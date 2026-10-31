//! Parses a command, runs it, and prints one JSON object on stdout for the agent.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use clap::{Args, Parser, Subcommand};
use serde::Serialize;

use crate::check_layout::{CheckLayoutFlags, check_layout};
use crate::frame_sequence::FrameSequence;
use crate::gpu::{Gpu, GpuError, GpuIdentity, GpuKind};
use crate::reaction::ReactionReport;
use crate::scene::{Camera, Scene, SceneTime, Sound};
use crate::scene_file::{ReadError, SceneError, read, schema};
use crate::timeline::{ChapterId, ChapterSpan, StepSpan};
use crate::video::VideoError;
use crate::{
    FrameSequenceRenderError, InspectedObject, RenderError, RenderUpdate, RenderUpdateKind,
};

#[derive(Parser)]
#[command(name = "sci_anim", version = concat!(env!("CARGO_PKG_VERSION"), " ", env!("SCI_ANIM_REVISION")), about = "Turns a scene file into an animation of math or science.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Lists every error in a scene file with the JSON path of the bad field.
    Validate { scene_file: PathBuf },
    /// Renders the frame at one scene time to a PNG.
    Frame {
        scene_file: PathBuf,
        /// Seconds since the start of the scene.
        #[arg(short = 't', long, alias = "t", allow_negative_numbers = true)]
        scene_time: f64,
        #[arg(short = 'o', long = "out")]
        png: PathBuf,
        #[command(flatten)]
        gpu: GpuFlag,
    },
    /// Prints each object's transform, opacity and bounding box at one scene time, and the camera.
    Inspect {
        scene_file: PathBuf,
        /// Seconds since the start of the scene.
        #[arg(short = 't', long, alias = "t", allow_negative_numbers = true)]
        scene_time: f64,
    },
    /// Prints what inspect prints at every frame time from one scene time to another, and at
    /// both ends, from one read of the scene file.
    Stages {
        scene_file: PathBuf,
        /// The first scene time, in seconds since the start of the scene.
        #[arg(long, allow_negative_numbers = true)]
        from: f64,
        /// The last scene time, in seconds since the start of the scene: `--from` or later.
        #[arg(long, allow_negative_numbers = true)]
        to: f64,
    },
    /// Renders the whole scene to an MP4 video with ffmpeg.
    Render {
        scene_file: PathBuf,
        #[arg(short = 'o', long = "out")]
        mp4: PathBuf,
        /// Prints a render update on stderr, one JSON object a line: about once a second while
        /// frames are drawn, then when ffmpeg is finishing the video, then when it is finished.
        #[arg(long)]
        updates: bool,
        #[command(flatten)]
        gpu: GpuFlag,
    },
    /// Renders every frame to numbered PNGs in a new directory, without sound.
    Frames {
        scene_file: PathBuf,
        #[arg(short = 'o', long = "out")]
        directory: PathBuf,
        /// Prints render updates on stderr.
        #[arg(long)]
        updates: bool,
        #[command(flatten)]
        gpu: GpuFlag,
    },
    /// Prints the JSON Schema of a scene file.
    Schema,
    /// Checks a scene file's layout at every step, animation span and follow's frame time, and
    /// prints each layout fault on a line of its own. Writes stages.txt, what shows at each
    /// checked scene time in words, and contact sheets of the step times. Exits 0 with no fault,
    /// 1 when it prints one, and 2 when it cannot check, with the error report on stderr.
    CheckLayout(CheckLayoutFlags),
    /// Installs the sci-anim skill this binary carries, the one that matches its version.
    Skill {
        #[command(subcommand)]
        command: SkillCommand,
    },
}

/// The GPU a command that draws asks for.
#[derive(Args)]
pub(crate) struct GpuFlag {
    /// The kind of GPU to draw on. With none, a discrete GPU when the machine has one, then an
    /// integrated one, then any other. The report names the GPU it drew on in `gpu`.
    #[arg(long, value_name = "KIND")]
    pub(crate) gpu: Option<GpuKind>,
}

#[derive(Subcommand)]
enum SkillCommand {
    /// Writes the sci-anim skill into the `sci-anim` folder of each skills folder at user level,
    /// at project level, or given with --to, making the skills folder if needed. User level is
    /// ~/.claude/skills and ~/.agents/skills, for each of ~/.claude and ~/.agents that exists, or
    /// ~/.agents/skills when neither does. Project level is the same folders under the root of
    /// the git repository holding the current directory. With no flag, inside a repository,
    /// with stdin and stderr a terminal, it asks which level and names the folders each answer
    /// writes; it installs nothing and exits 1 if stdin ends first. Otherwise, with no flag,
    /// it installs at user level. It replaces an earlier sci-anim skill there, so files you
    /// added to that folder are lost. It refuses a link, or a folder that is not a sci-anim
    /// skill.
    Install {
        /// A skills folder to install into, such as .claude/skills for one project. Give it once
        /// per skills folder.
        #[arg(long = "to", value_name = "SKILLS_FOLDER", conflicts_with_all = ["user", "project"])]
        skills_folders: Vec<PathBuf>,
        /// Installs at user level, without asking.
        #[arg(long, conflicts_with = "project")]
        user: bool,
        /// Installs at project level, into the root of the git repository holding the current
        /// directory, without asking.
        #[arg(long)]
        project: bool,
    },
}

/// What every command but `schema` prints. `schema` prints the bare schema document, so that
/// `sci_anim schema > scene.schema.json` gives a file other tools can read.
#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Report {
    Ok(Outcome),
    Error { errors: Vec<ToolError> },
}

#[derive(Debug, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
enum Outcome {
    /// The scene's length, and when each step and each chapter starts and ends, so sound made
    /// elsewhere can be laid against the video.
    Validate {
        duration: f64,
        steps: Vec<StepSpan>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        chapters: Vec<ChapterSpan>,
        /// Each sound with the chapter and scene time it starts at.
        #[serde(skip_serializing_if = "Vec::is_empty")]
        sounds: Vec<Sound>,
    },
    Frame {
        png: PathBuf,
        width: u32,
        height: u32,
        scene_time: f64,
        gpu: GpuIdentity,
    },
    Inspect(InspectedStage),
    /// The stage at every frame time from `--from` to `--to`, and at both, in scene time order.
    Stages {
        fps: u32,
        stages: Vec<InspectedStage>,
    },
    Render {
        mp4: PathBuf,
        width: u32,
        height: u32,
        fps: u32,
        frames: u32,
        duration: f64,
        gpu: GpuIdentity,
    },
    Frames {
        directory: PathBuf,
        #[serde(flatten)]
        frame_sequence: FrameSequence,
        gpu: GpuIdentity,
    },
    /// Each skill folder the skill was written to, and the version of sci_anim it matches.
    SkillInstall {
        installed: Vec<PathBuf>,
        version: &'static str,
    },
}

/// What `inspect` prints at one scene time, and `stages` at each.
#[derive(Debug, Serialize)]
pub(crate) struct InspectedStage {
    scene_time: f64,
    /// The chapter the scene time falls in, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    chapter: Option<ChapterId>,
    camera: Camera,
    objects: BTreeMap<String, InspectedObject>,
    reactions: Vec<ReactionReport>,
}

impl InspectedStage {
    pub(crate) fn at(scene: &Scene, scene_time: SceneTime) -> InspectedStage {
        let inspection = crate::inspect(scene, scene_time);
        let objects = inspection
            .objects
            .into_iter()
            .map(|(id, object)| (id.to_string(), object))
            .collect();
        let chapter = scene
            .chapter_at(scene_time)
            .map(|chapter| chapter.id.clone());
        InspectedStage {
            scene_time: scene_time.seconds(),
            chapter,
            camera: inspection.camera,
            objects,
            reactions: inspection.reactions,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ToolError {
    Scene(SceneError),
    Io {
        file: PathBuf,
        message: String,
    },
    Gpu {
        message: String,
    },
    /// ffmpeg is missing, or failed. `stderr` is what ffmpeg printed, empty when it never ran.
    Ffmpeg {
        message: String,
        stderr: String,
    },
    Usage {
        message: String,
    },
    /// `skill install` asked which level to install at, and stdin ended before an answer.
    Unanswered {
        message: String,
    },
}

impl Report {
    /// 0 when ok, 1 when the scene file is at fault or a question went unanswered, 2 when
    /// anything else is at fault.
    fn exit_code(&self) -> ExitCode {
        match self {
            Report::Ok(_) => ExitCode::SUCCESS,
            Report::Error { errors }
                if errors
                    .iter()
                    .any(|e| matches!(e, ToolError::Scene(_) | ToolError::Unanswered { .. })) =>
            {
                ExitCode::from(1)
            }
            Report::Error { .. } => ExitCode::from(2),
        }
    }
}

pub fn run() -> ExitCode {
    let command = match Cli::try_parse() {
        Ok(cli) => cli.command,
        Err(e) if !e.use_stderr() => {
            let _ = e.print();
            return ExitCode::SUCCESS;
        }
        // check-layout keeps stdout for its layout faults.
        Err(e)
            if std::env::args_os()
                .nth(1)
                .is_some_and(|command| command == "check-layout") =>
        {
            return failed(vec![ToolError::Usage {
                message: e.render().to_string(),
            }]);
        }
        Err(e) => {
            return report(Err(vec![ToolError::Usage {
                message: e.render().to_string(),
            }]));
        }
    };
    let outcome = match command {
        Command::Schema => return print(schema(), ExitCode::SUCCESS),
        Command::CheckLayout(flags) => {
            return match check_layout(&flags) {
                Ok(any_fault) => ExitCode::from(u8::from(any_fault)),
                Err(errors) => failed(errors),
            };
        }
        Command::Validate { scene_file } => validate(&scene_file),
        Command::Frame {
            scene_file,
            scene_time,
            png,
            gpu,
        } => frame(&scene_file, scene_time, png, gpu.gpu),
        Command::Inspect {
            scene_file,
            scene_time,
        } => inspect(&scene_file, scene_time),
        Command::Stages {
            scene_file,
            from,
            to,
        } => stages(&scene_file, from, to),
        Command::Render {
            scene_file,
            mp4,
            updates,
            gpu,
        } => render(&scene_file, mp4, updates, gpu.gpu),
        Command::Frames {
            scene_file,
            directory,
            updates,
            gpu,
        } => frames(&scene_file, directory, updates, gpu.gpu),
        Command::Skill {
            command:
                SkillCommand::Install {
                    skills_folders,
                    user,
                    project,
                },
        } => skill_install(skills_folders, user, project),
    };
    report(outcome)
}

fn validate(scene_file: &Path) -> Result<Outcome, Vec<ToolError>> {
    let scene = read(scene_file)?;
    Ok(Outcome::Validate {
        duration: scene.duration(),
        steps: scene.steps().to_vec(),
        chapters: scene.chapters().to_vec(),
        sounds: scene.sounds().to_vec(),
    })
}

fn frame(
    scene_file: &Path,
    seconds: f64,
    png: PathBuf,
    gpu_kind: Option<GpuKind>,
) -> Result<Outcome, Vec<ToolError>> {
    let scene = read(scene_file)?;
    let scene_time = scene
        .scene_time(seconds)
        .map_err(|e| vec![ToolError::Scene(e)])?;
    let gpu = Gpu::new(scene.scene_config.background, gpu_kind)?;
    let frame = crate::frame(&gpu, &scene, scene_time)?;
    frame.write_png(&png).map_err(|e| {
        vec![ToolError::Io {
            file: png.clone(),
            message: e.to_string(),
        }]
    })?;
    Ok(Outcome::Frame {
        png,
        width: frame.width,
        height: frame.height,
        scene_time: scene_time.seconds(),
        gpu: gpu.identity,
    })
}

fn inspect(scene_file: &Path, seconds: f64) -> Result<Outcome, Vec<ToolError>> {
    let scene = read(scene_file)?;
    let scene_time = scene
        .scene_time(seconds)
        .map_err(|e| vec![ToolError::Scene(e)])?;
    Ok(Outcome::Inspect(InspectedStage::at(&scene, scene_time)))
}

fn stages(scene_file: &Path, from: f64, to: f64) -> Result<Outcome, Vec<ToolError>> {
    if from > to {
        return Err(vec![ToolError::Usage {
            message: format!(
                "--from {from} is later than --to {to}; give the earlier scene time first"
            ),
        }]);
    }
    let scene = read(scene_file)?;
    let errors: Vec<ToolError> = [scene.scene_time(from), scene.scene_time(to)]
        .into_iter()
        .filter_map(Result::err)
        .map(ToolError::Scene)
        .collect();
    let (Ok(from), Ok(to)) = (scene.scene_time(from), scene.scene_time(to)) else {
        return Err(errors);
    };
    let stages = scene
        .stage_times(from, to)
        .into_iter()
        .map(|scene_time| InspectedStage::at(&scene, scene_time))
        .collect();
    Ok(Outcome::Stages {
        fps: scene.scene_config.fps.get(),
        stages,
    })
}

fn render(
    scene_file: &Path,
    mp4: PathBuf,
    updates: bool,
    gpu_kind: Option<GpuKind>,
) -> Result<Outcome, Vec<ToolError>> {
    let scene = read(scene_file)?;
    let mut last_printed = None;
    let mut print = |update| {
        print_update(
            update,
            &mut last_printed,
            Instant::now(),
            &mut std::io::stderr(),
        )
    };
    let on_update: &mut dyn FnMut(RenderUpdate) = if updates { &mut print } else { &mut |_| {} };
    let (video, gpu) = crate::render(&scene, &mp4, gpu_kind, on_update)?;
    Ok(Outcome::Render {
        mp4,
        width: video.width,
        height: video.height,
        fps: video.fps,
        frames: video.frames,
        duration: video.duration,
        gpu,
    })
}

fn frames(
    scene_file: &Path,
    directory: PathBuf,
    updates: bool,
    gpu_kind: Option<GpuKind>,
) -> Result<Outcome, Vec<ToolError>> {
    let scene = read(scene_file)?;
    let mut last_printed = None;
    let mut print = |update| {
        print_update(
            update,
            &mut last_printed,
            Instant::now(),
            &mut std::io::stderr(),
        )
    };
    let on_update: &mut dyn FnMut(RenderUpdate) = if updates { &mut print } else { &mut |_| {} };
    let (frame_sequence, gpu) = crate::frames(&scene, &directory, gpu_kind, on_update)?;
    Ok(Outcome::Frames {
        directory,
        frame_sequence,
        gpu,
    })
}

fn skill_install(
    given: Vec<PathBuf>,
    user: bool,
    project: bool,
) -> Result<Outcome, Vec<ToolError>> {
    use crate::skill::{Level, choose, repository_holding, resolve};
    use std::io::IsTerminal;
    let level = match (user, project) {
        (true, _) => Some(Level::User),
        (_, true) => Some(Level::Project),
        _ => None,
    };
    let repository = std::env::current_dir()
        .ok()
        .and_then(|dir| repository_holding(&dir));
    let terminal = std::io::stdin().is_terminal() && std::io::stderr().is_terminal();
    let chosen = choose(given, level, repository, terminal).map_err(|e| vec![e])?;
    // The question goes to stderr, so stdout stays one JSON report.
    let skills_folders =
        resolve(chosen, std::io::stdin().lock(), std::io::stderr().lock()).map_err(|e| vec![e])?;
    let installed = crate::skill::install(&skills_folders)?;
    Ok(Outcome::SkillInstall {
        installed,
        version: env!("CARGO_PKG_VERSION"),
    })
}

/// A render update comes for every frame drawn. This prints the first and the last drawing
/// update and at most one a second between them, and every other update.
fn print_update(
    update: RenderUpdate,
    last_printed: &mut Option<Instant>,
    now: Instant,
    out: &mut impl Write,
) {
    let between = update.update == RenderUpdateKind::Drawing
        && update.frames_drawn != 0
        && update.frames_drawn != update.frames;
    if between && last_printed.is_some_and(|at| now - at < Duration::from_secs(1)) {
        return;
    }
    *last_printed = Some(now);
    let mut line = serde_json::to_string(&update).expect("a render update is plain data");
    line.push('\n');
    // One write(2), so another process writing to the same pipe cannot split the line.
    let _ = out.write_all(line.as_bytes());
}

fn report(outcome: Result<Outcome, Vec<ToolError>>) -> ExitCode {
    let report = match outcome {
        Ok(outcome) => Report::Ok(outcome),
        Err(errors) => Report::Error { errors },
    };
    print(&report, report.exit_code())
}

/// Prints the error report of a check-layout that could not check on stderr, and exits 2.
fn failed(errors: Vec<ToolError>) -> ExitCode {
    let text =
        serde_json::to_string_pretty(&Report::Error { errors }).expect("a report is plain data");
    let _ = writeln!(std::io::stderr().lock(), "{text}");
    ExitCode::from(2)
}

fn print(json: &impl Serialize, exit_code: ExitCode) -> ExitCode {
    let text = serde_json::to_string_pretty(json).expect("a report is plain data");
    // A reader that stops early, such as `head`, closes the pipe; that is no fault of the command.
    match writeln!(std::io::stdout().lock(), "{text}") {
        Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            panic!("failed printing to stdout: {e}")
        }
        _ => exit_code,
    }
}

impl From<ReadError> for Vec<ToolError> {
    fn from(error: ReadError) -> Self {
        match error {
            ReadError::Io { file, source } => vec![ToolError::Io {
                file,
                message: source.to_string(),
            }],
            ReadError::Invalid(errors) => errors.into_iter().map(ToolError::Scene).collect(),
        }
    }
}

impl From<RenderError> for Vec<ToolError> {
    fn from(error: RenderError) -> Self {
        match error {
            RenderError::TransparentBackground => vec![ToolError::Usage {
                message: "MP4 cannot preserve a transparent background; use sci_anim frames to write a transparent frame sequence".into(),
            }],
            RenderError::Gpu(error) => error.into(),
            RenderError::Video(VideoError::Missing(e)) => {
                let message = format!("ffmpeg could not be started, so no video was written; install ffmpeg and put it on PATH ({e})");
                vec![ToolError::Ffmpeg { message, stderr: String::new() }]
            }
            RenderError::Video(VideoError::Failed { message, stderr }) => vec![ToolError::Ffmpeg { message, stderr }],
            RenderError::Video(VideoError::Io { file, source }) => vec![ToolError::Io { file, message: source.to_string() }],
        }
    }
}

impl From<FrameSequenceRenderError> for Vec<ToolError> {
    fn from(error: FrameSequenceRenderError) -> Self {
        match error {
            FrameSequenceRenderError::Gpu(error) => error.into(),
            FrameSequenceRenderError::Files(error) => vec![ToolError::Io {
                file: error.file,
                message: error.source.to_string(),
            }],
        }
    }
}

impl From<GpuError> for Vec<ToolError> {
    fn from(error: GpuError) -> Self {
        vec![ToolError::Gpu {
            message: error.message(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawing_updates_between_the_first_and_the_last_print_at_most_once_a_second() {
        let start = Instant::now();
        let at = |seconds: f64| start + Duration::from_secs_f64(seconds);
        let drawing = |frames_drawn| RenderUpdate {
            update: RenderUpdateKind::Drawing,
            frames_drawn,
            frames: 5,
        };
        let updates = [
            (drawing(0), at(0.0)),
            (drawing(1), at(0.5)),
            (drawing(2), at(1.2)),
            (drawing(3), at(1.9)),
            (drawing(4), at(2.3)),
            (drawing(5), at(2.4)),
            (
                RenderUpdate {
                    update: RenderUpdateKind::Finishing,
                    frames_drawn: 5,
                    frames: 5,
                },
                at(2.4),
            ),
            (
                RenderUpdate {
                    update: RenderUpdateKind::Finished,
                    frames_drawn: 5,
                    frames: 5,
                },
                at(2.5),
            ),
        ];
        let mut last_printed = None;
        let mut out = Vec::new();

        for (update, now) in updates {
            print_update(update, &mut last_printed, now, &mut out);
        }

        assert_eq!(
            String::from_utf8(out).unwrap(),
            concat!(
                r#"{"update":"drawing","frames_drawn":0,"frames":5}"#,
                "\n",
                r#"{"update":"drawing","frames_drawn":2,"frames":5}"#,
                "\n",
                r#"{"update":"drawing","frames_drawn":4,"frames":5}"#,
                "\n",
                r#"{"update":"drawing","frames_drawn":5,"frames":5}"#,
                "\n",
                r#"{"update":"finishing","frames_drawn":5,"frames":5}"#,
                "\n",
                r#"{"update":"finished","frames_drawn":5,"frames":5}"#,
                "\n",
            )
        );
    }
}
