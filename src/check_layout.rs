//! `check-layout`: checks a scene file's layout at every step and animation span, and draws a
//! contact sheet of the steps.
//!
//! It checks the middle and end of every play step, the end of every other step and chapter, the
//! stage just before a step's fades to 0 begin, the start, middle and end of every animation's
//! span, and every frame time of each follow's span, since a law or a carry can take an object
//! anywhere between three checked scene times. At each it reads the stage as `inspect` prints it
//! and prints each layout fault: an object the frame cuts off, text over another object, a plot
//! through a tick label or axis label, a caption over its molecule's atoms, an attached object
//! showing while its molecule is faded out, text too small or too faint to read, two texts too
//! close to read apart, two atoms that pass through each other in a reaction, two unbonded atoms
//! that rest so close at a step's end that they read as bonded, a heavy atom of a named group
//! hidden behind another atom or a bond of one pointing nearly at the camera on a ball-and-stick
//! molecule, a lit glow over text or another molecule's atoms, two glows on different molecules
//! that merge, a shown object in a reserved region, and an object that a create, write, morph or
//! reaction makes
//! but that never shows, because it is declared at opacity 0 and no fade or follow of its opacity raises it. Then it draws the frame at each step time, not each span
//! time, tiles them in order into contact sheets, and writes `stages.txt`: every shown object at
//! each checked scene time, in words, so an agent that cannot view images can still read the
//! video. A follow's frame times go neither on the sheet nor in `stages.txt`.
//!
//! The checks read the stage as JSON, exactly as `inspect` prints it, so what they compare is
//! what an agent reading the report sees.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use clap::Args;
use serde_json::Value;

use crate::cli::GpuFlag;
use crate::cli::{InspectedStage, ToolError};
use crate::gpu::Gpu;
use crate::scene::{Scene, SceneTime};
use crate::scene_file::read;

const COLUMNS: usize = 4;
const TILE_WIDTH: f64 = 480.0;
const PAGE_TILES: usize = 16;
/// A few pixels of touching boxes are antialiasing and glyph side bearings, not an overlap.
const TOLERANCE: f64 = 3.0;
/// The smallest readable em as a share of the frame's height: text size 0.3 at the default camera.
const SMALLEST_EM: f64 = 0.034;
/// The least contrast between text and background that reads at a glance, as WCAG measures it.
const LEAST_CONTRAST: f64 = 4.5;
/// How far an axis line's ticks reach across it, in pixels, with antialiasing.
const TICK_REACH: f64 = 8.0;
/// Two texts closer than this share of the frame's height read as one: about 13 pixels at 1080p.
const LEAST_GAP: f64 = 0.012;
/// Two atoms nearer than this many angstroms, at the molecule's scale, pass through each other.
const NEAREST_ATOMS: f64 = 0.5;
/// Two unbonded balls nearer than this many angstroms, at the molecule's scale, rest as close as
/// the balls of a bonded pair do. See `atoms_that_read_as_bonded`.
const BALLS_APART: f64 = 0.35;
/// A bond of a named group within about 30 degrees of the line of sight shows as a stub.
const END_ON: f64 = 0.866;
/// A ball-and-stick molecule of more shown atoms than this, such as a protein or a stretch of DNA
/// read from a PDB file, hides some atoms of any group behind others whichever way it turns, so
/// no rotation clears the hidden-atom and end-on-bond checks, and they skip it.
const CROWDED_ATOMS: usize = 200;
/// The edges of a reserved region on a contact sheet tile: pixels wide, in a color no lesson uses.
const RESERVED_COLOR: &str = "#ff00ff";
const RESERVED_EDGE_PIXELS: f64 = 2.0;
/// Scene times within a millisecond are one, so a span that ends at 0.3 reaches the end of steps
/// of 0.1 and 0.2 seconds, which sum to a hair over 0.3.
const SAME_SCENE_TIME: f64 = 0.001;
/// The solids, which a key light shades.
const SOLIDS: [&str; 5] = ["sphere", "prism", "revolution", "tube", "surface"];
/// A fault with a pattern names this many of its copies, then how many more.
const MOST_COPIES_NAMED: usize = 6;
const DEFAULT_TEXT_SIZE: f64 = 0.6;
const DEFAULT_COLOR: &str = "#ffffff";
const DEFAULT_BACKGROUND: &str = "#1e1e1e";
const DEFAULT_WIDTH: u64 = 1920;
const DEFAULT_HEIGHT: u64 = 1080;

#[derive(Args)]
pub struct CheckLayoutFlags {
    scene_file: PathBuf,
    /// The folder for stages.txt, the contact sheets and their frames. The default is
    /// `<scene file name>-check` beside the scene file.
    #[arg(long, value_name = "DIR")]
    out: Option<PathBuf>,
    /// Skips drawing the frames and the contact sheets.
    #[arg(long)]
    no_sheet: bool,
    /// Checks only the ends of steps and chapters: no play step's middle, no animation's span
    /// and no follow's frame times.
    #[arg(long)]
    ends_only: bool,
    /// Two object ids whose overlap is on purpose. Give it once for each pair.
    #[arg(long, value_name = "ID,ID")]
    allow: Vec<String>,
    /// A reserved region, a box of the frame in pixels from its top left that no shown object
    /// may enter. `@START-END` after the box holds it only from scene time START to END, in
    /// seconds; with no span it holds for the whole scene. Give it once for each.
    #[arg(long, value_name = "X0,Y0,X1,Y1[@START-END]", value_parser = reserved_region)]
    reserve: Vec<ReservedRegion>,
    /// An object id that may sit in the reserved regions, with the atoms a reaction moves from
    /// it. Give it once for each.
    #[arg(long, value_name = "ID")]
    reserve_exempt: Vec<String>,
    #[command(flatten)]
    gpu: GpuFlag,
}

/// A box on the frame in pixels, x from the left and y from the top, as `inspect` prints one.
#[derive(Clone, Copy, Debug)]
pub struct PixelBox {
    min: [f64; 2],
    max: [f64; 2],
}

impl PixelBox {
    fn of(value: &Value) -> PixelBox {
        let point = |key: &str| [number(&value[key][0]), number(&value[key][1])];
        PixelBox {
            min: point("min"),
            max: point("max"),
        }
    }
}

/// A reserved region: a box of the frame, and the span of scene time it holds for, or `None`
/// for the whole scene.
#[derive(Clone, Copy, Debug)]
pub struct ReservedRegion {
    pixels: PixelBox,
    span: Option<ReservedSpan>,
}

/// A reserved region's span: from `start` to `end` in scene seconds, both included, with `start`
/// less than `end`.
#[derive(Clone, Copy, Debug)]
struct ReservedSpan {
    start: f64,
    end: f64,
}

impl ReservedRegion {
    fn holds_at(&self, scene_time: f64) -> bool {
        self.span.is_none_or(|span| {
            span.start - SAME_SCENE_TIME <= scene_time && scene_time <= span.end + SAME_SCENE_TIME
        })
    }
}

impl std::fmt::Display for ReservedRegion {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "the reserved region {} to {} pixels",
            pixels_of(&self.pixels.min),
            pixels_of(&self.pixels.max)
        )?;
        match self.span {
            Some(span) => write!(
                f,
                ", from scene time {} to {}",
                general(span.start),
                general(span.end)
            ),
            None => Ok(()),
        }
    }
}

/// A reserved region from X0,Y0,X1,Y1 in frame pixels, with an optional @START-END in scene
/// seconds.
fn reserved_region(flag: &str) -> Result<ReservedRegion, String> {
    let (corners, span) = match flag.split_once('@') {
        Some((corners, span)) => (corners, Some(reserved_span(span)?)),
        None => (flag, None),
    };
    let values: Vec<Option<f64>> = corners
        .split(',')
        .map(|value| value.trim().parse().ok())
        .collect();
    let [Some(x0), Some(y0), Some(x1), Some(y1)] = values[..] else {
        return Err(format!("{corners} is not four numbers X0,Y0,X1,Y1"));
    };
    if !(x0 < x1 && y0 < y1) {
        return Err(format!(
            "{corners} needs X0 less than X1 and Y0 less than Y1"
        ));
    }
    Ok(ReservedRegion {
        pixels: PixelBox {
            min: [x0, y0],
            max: [x1, y1],
        },
        span,
    })
}

/// A reserved region's span from START-END in scene seconds.
fn reserved_span(span: &str) -> Result<ReservedSpan, String> {
    // From the second character, so a START of -1 reads as a number outside the scene.
    let dash = span
        .get(1..)
        .and_then(|rest| rest.find('-'))
        .map(|at| at + 1);
    let times = dash.and_then(|at| {
        let (start, end) = (&span[..at], &span[at + 1..]);
        Some((
            start.trim().parse::<f64>().ok()?,
            end.trim().parse::<f64>().ok()?,
        ))
    });
    let Some((start, end)) = times else {
        return Err(format!(
            "@{span} is not a span of scene time START-END in seconds"
        ));
    };
    if start.partial_cmp(&end) != Some(std::cmp::Ordering::Less) {
        return Err(format!("@{span} needs START less than END"));
    }
    Ok(ReservedSpan { start, end })
}

/// One checked scene time, described in words; `on_sheet` when its frame goes on the contact sheet,
/// and `ends_step` when it is the end of a step or chapter, where every animation of the step has
/// finished.
#[derive(Clone, Debug)]
struct CheckedTime {
    wording: String,
    scene_time: f64,
    on_sheet: bool,
    ends_step: bool,
}

/// Each layout fault, or each reason contrast is unverified, with every checked scene time it
/// was found at, in the order each was first found.
#[derive(Default)]
struct Findings {
    found: Vec<(String, Vec<CheckedTime>)>,
    index: HashMap<String, usize>,
}

impl Findings {
    fn add(&mut self, finding: String, at: CheckedTime) {
        let at_index = *self.index.entry(finding.clone()).or_insert_with(|| {
            self.found.push((finding, Vec::new()));
            self.found.len() - 1
        });
        self.found[at_index].1.push(at);
    }
}

/// What a scene says about itself: the scene file as written, and the scene as read.
struct Lesson<'a> {
    file: &'a Value,
    scene: &'a Scene,
}

/// Checks the layout, prints each layout fault, and writes `stages.txt` and the contact sheets.
/// Ok(true) when it printed a layout fault.
pub fn check_layout(flags: &CheckLayoutFlags) -> Result<bool, Vec<ToolError>> {
    let io = |file: &Path, error: &dyn std::fmt::Display| {
        vec![ToolError::Io {
            file: file.to_path_buf(),
            message: error.to_string(),
        }]
    };
    let text = std::fs::read_to_string(&flags.scene_file).map_err(|e| io(&flags.scene_file, &e))?;
    let file: Value = serde_json::from_str(&text).map_err(|e| io(&flags.scene_file, &e))?;
    let scene_config = &file["scene_config"];
    let width = scene_config
        .get("width")
        .cloned()
        .unwrap_or(Value::from(DEFAULT_WIDTH));
    let height = scene_config
        .get("height")
        .cloned()
        .unwrap_or(Value::from(DEFAULT_HEIGHT));
    for region in flags.reserve.iter().map(|region| region.pixels) {
        if region.min[0] < 0.0
            || region.min[1] < 0.0
            || region.max[0] > number(&width)
            || region.max[1] > number(&height)
        {
            let message = format!(
                "--reserve {} to {} runs outside the frame, which is {} by {} pixels",
                pixels_of(&region.min),
                pixels_of(&region.max),
                as_written(&width),
                as_written(&height)
            );
            return Err(vec![ToolError::Usage { message }]);
        }
    }
    let scene = read(&flags.scene_file)?;
    for span in flags.reserve.iter().filter_map(|region| region.span) {
        if span.start < 0.0 || span.end > scene.duration() + SAME_SCENE_TIME {
            let message = format!(
                "--reserve from scene time {} to {} runs outside the scene, which runs from 0 to {} seconds",
                general(span.start),
                general(span.end),
                general(scene.duration())
            );
            return Err(vec![ToolError::Usage { message }]);
        }
    }
    let lesson = Lesson {
        file: &file,
        scene: &scene,
    };
    let checked_times = checked_times(&lesson, !flags.ends_only);
    let rotations = rotations(&lesson);
    let opacity_changes = opacity_changes(&lesson);
    let allowed: Vec<BTreeSet<&str>> = flags
        .allow
        .iter()
        .map(|pair| pair.split(',').collect())
        .collect();
    let exempt: HashSet<&str> = flags.reserve_exempt.iter().map(String::as_str).collect();
    let transparent = scene_config
        .get("background")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_BACKGROUND)
        == "transparent";

    // The checks at one stage: its layout faults, and the reasons its contrast is unverified.
    let faults_at = |report: &Value, at: &CheckedTime| -> (Vec<String>, Vec<String>) {
        let with = with_cells(report, &file);
        let (report, file) = with.as_ref().map_or((report, &file), |(r, f)| (r, f));
        let fading = changing(&opacity_changes, at.scene_time);
        let mut found = layout_faults(
            report,
            file,
            &allowed,
            &changing(&rotations, at.scene_time),
            &fading,
        );
        if at.ends_step {
            found.extend(atoms_that_read_as_bonded(report));
        }
        found.extend(reserved_faults(
            report,
            at.scene_time,
            &flags.reserve,
            &exempt,
        ));
        let mut unknown = Vec::new();
        if transparent {
            let (contrast_faults, reasons) = transparent_contrast(report, file, &fading);
            found.extend(contrast_faults);
            unknown = reasons;
        }
        (found, unknown)
    };
    let mut faults = Findings::default();
    let mut unverified = Findings::default();
    // A followed object moves a little every frame, so one fault across a follow's frames is one
    // line, worded as at its first frame, whatever pixels it reaches at the others.
    let mut first_wording: HashMap<String, String> = HashMap::new();
    let mut record = |(found, unknown): (Vec<String>, Vec<String>),
                      at: &CheckedTime,
                      moving: bool|
     -> Vec<String> {
        for fault in found {
            let fault = if moving {
                first_wording
                    .entry(without_pixels(&fault))
                    .or_insert(fault)
                    .clone()
            } else {
                fault
            };
            faults.add(fault, at.clone());
        }
        for reason in &unknown {
            unverified.add(reason.clone(), at.clone());
        }
        unknown
    };

    let mut stages = Vec::new();
    let mut stage_unverified = Vec::new();
    let checked = each_in_parallel(&checked_times, |at| {
        let report = stage_report(&scene, at.scene_time)?;
        let with = with_cells(&report, &file);
        let (shown_report, shown_file) = with.as_ref().map_or((&report, &file), |(r, f)| (r, f));
        Ok::<_, Vec<ToolError>>((describe(shown_report, shown_file), faults_at(&report, at)))
    });
    for (at, result) in checked_times.iter().zip(checked) {
        let (described, found) = result?;
        stages.push(described);
        stage_unverified.push(record(found, at, false));
    }
    let frames = if flags.ends_only {
        Vec::new()
    } else {
        frame_times(&scene, &frame_spans(&lesson))?
    };
    let checked = each_in_parallel(&frames, |frame| {
        let report = stage_report(&scene, frame.stage_time.seconds())?;
        let seconds = number(&report["scene_time"]);
        let at = CheckedTime {
            wording: format!(
                "frame at {} of {}",
                written_scene_time(seconds, scene.duration()),
                frame.wording
            ),
            scene_time: seconds,
            on_sheet: false,
            ends_step: false,
        };
        let found = faults_at(&report, &at);
        Ok::<_, Vec<ToolError>>((at, found))
    });
    for result in checked {
        let (at, found) = result?;
        record(found, &at, true);
    }
    let follow_frames = frames.iter().filter(|frame| frame.of_follow).count();
    let simulation_frames = frames.iter().filter(|frame| frame.of_simulation).count();

    for (fault, at) in never_shown(&lesson) {
        faults.add(fault, at);
    }
    for (fault, at) in &faults.found {
        say(&format!("{}: {fault}", when(at, scene.duration())));
    }
    for (reason, at) in &unverified.found {
        say(&format!(
            "Contrast unverified at {}: {reason}",
            when(at, scene.duration())
        ));
    }
    let any_fault = !faults.found.is_empty();
    let frame_times = match (follow_frames, simulation_frames) {
        (0, 0) => String::new(),
        (follows, 0) => format!(" and {follows} frame times of follows"),
        (0, simulations) => format!(" and {simulations} frame times of simulations"),
        (follows, simulations) => {
            format!(", {follows} frame times of follows and {simulations} of simulations")
        }
    };
    let checked = format!("{} checked scene times", checked_times.len()) + &frame_times;
    if !any_fault && unverified.found.is_empty() {
        say(&format!("No problem at any of {checked}."));
    } else if !any_fault {
        say(&format!("No verified problem at any of {checked}."));
    }
    if !unverified.found.is_empty() {
        let checks: usize = unverified.found.iter().map(|(_, at)| at.len()).sum();
        let times: HashSet<(&str, u64, bool)> = unverified
            .found
            .iter()
            .flat_map(|(_, at)| at)
            .map(|at| {
                (
                    at.wording.as_str(),
                    (at.scene_time + 0.0).to_bits(),
                    at.on_sheet,
                )
            })
            .collect();
        say(&format!(
            "Contrast unverified for {checks} text checks at {} checked scene times; check the final composite.",
            times.len()
        ));
    }

    let out = tidy(
        &flags
            .out
            .clone()
            .unwrap_or_else(|| beside(&flags.scene_file)),
    );
    std::fs::create_dir_all(&out).map_err(|e| io(&out, &e))?;
    remove_old_sheets(&out).map_err(|e| io(&out, &e))?;
    let stages_file = out.join("stages.txt");
    let sheet: Vec<&CheckedTime> = checked_times.iter().filter(|at| at.on_sheet).collect();
    let tile_number = |at: &CheckedTime| sheet.iter().position(|tile| std::ptr::eq(*tile, at));
    let mut stages_text = String::from(
        "The stage at each checked scene time. The first lists every shown object, top of the frame first;\n\
         each later one lists only changes: + now shows, ~ changed, - no longer shows.\n\n",
    );
    for (number, at) in checked_times.iter().enumerate() {
        stages_text += &format!(
            "t={}, {}",
            written_scene_time(at.scene_time, scene.duration()),
            at.wording
        );
        if let (false, Some(tile)) = (flags.no_sheet, tile_number(at)) {
            stages_text += &format!(", frame-{tile:03}.png");
        }
        stages_text += "\n";
        let before = number.checked_sub(1).map(|before| &stages[before]);
        let mut lines = changes(before, &stages[number]);
        lines.extend(
            stage_unverified[number]
                .iter()
                .map(|reason| format!("  contrast unverified: {reason}")),
        );
        stages_text += &(lines.join("\n") + "\n\n");
    }
    std::fs::write(&stages_file, stages_text).map_err(|e| io(&stages_file, &e))?;
    say(&format!(
        "What shows at each checked scene time, in words: {}",
        stages_file.display()
    ));
    if !flags.no_sheet {
        let sheets = contact_sheets(
            &scene,
            &sheet,
            &out,
            &flags.reserve,
            number(&width),
            &flags.gpu,
        )?;
        say(&format!(
            "Contact sheets, {PAGE_TILES} tiles a page, one tile per checked step time, left to right then down:"
        ));
        for sheet in sheets {
            say(&format!("  {}", sheet.display()));
        }
        say(&format!(
            "Each tile is also a full-size PNG in {}, named in stages.txt.",
            out.display()
        ));
    }
    Ok(any_fault)
}

/// Prints one line on stdout. A reader that stops early, such as `head`, is no fault of the check.
fn say(line: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stdout().lock(), "{line}");
}

fn scene_time(scene: &Scene, seconds: f64) -> Result<crate::scene::SceneTime, Vec<ToolError>> {
    scene
        .scene_time(seconds)
        .map_err(|e| vec![ToolError::Scene(e)])
}

/// The stage at a scene time as `inspect` prints it.
fn stage_report(scene: &Scene, seconds: f64) -> Result<Value, Vec<ToolError>> {
    let stage = InspectedStage::at(scene, scene_time(scene, seconds)?);
    // Printed and read back, so each number is what an agent reads from the report.
    let printed = serde_json::to_string(&stage).expect("a stage is plain data");
    Ok(serde_json::from_str(&printed).expect("a printed stage reads back"))
}

/// The default folder: `<scene file name>-check` beside the scene file.
fn beside(scene_file: &Path) -> PathBuf {
    let stem = scene_file.file_stem().unwrap_or_default().to_string_lossy();
    scene_file.with_file_name(format!("{stem}-check"))
}

/// A path without `.` parts, so the paths it prints read as the folder given.
fn tidy(path: &Path) -> PathBuf {
    let tidied: PathBuf = path
        .components()
        .filter(|part| *part != Component::CurDir)
        .collect();
    if tidied.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        tidied
    }
}

fn remove_old_sheets(out: &Path) -> std::io::Result<()> {
    let matches = |file_name: &str, prefix: &str, suffix: &str| {
        file_name.len() >= prefix.len() + suffix.len()
            && file_name.starts_with(prefix)
            && file_name.ends_with(suffix)
    };
    for entry in std::fs::read_dir(out)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if matches(&file_name, "frame-", ".png")
            || matches(&file_name, "contact-sheet", ".png")
            || file_name == "tiles.txt"
        {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

/// Where each step is, in words, with the step as the scene file writes it and its start and end
/// in scene time.
fn steps<'a>(lesson: &Lesson<'a>) -> Vec<(String, &'a Value, f64, f64)> {
    lesson
        .scene
        .steps()
        .iter()
        .map(|span| {
            let path = span.path.to_string();
            let segments: Vec<&str> = path.split('/').skip(2).collect();
            let mut step =
                &lesson.file["timeline"][segments[0].parse::<usize>().unwrap_or_default()];
            let mut wording = format!("step {}", segments[0]);
            if segments.len() == 3 {
                wording = format!(
                    "step {} of chapter \"{}\"",
                    segments[2],
                    as_written(&step["id"])
                );
                step = &step["steps"][segments[2].parse::<usize>().unwrap_or_default()];
            }
            (wording, step, span.start, span.end)
        })
        .collect()
}

/// Each animation of a step, with its span in scene time.
fn animation_spans(step: &Value, start: f64, end: f64) -> Vec<(&Value, f64, f64)> {
    let animations = step["animations"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    animations
        .iter()
        .map(|animation| {
            let span = (
                animation.get("start").map_or(0.0, number),
                animation.get("end").map_or(end - start, number),
            );
            let (first, last) = crate::timeline::scene_span(start, span);
            (animation, first, last)
        })
        .collect()
}

/// Whether an animation turns its object: a rotate, a follow that sets a rotation, or a carry
/// that faces the tangent.
fn turns(animation: &Value) -> bool {
    if animation["type"] == "follow" {
        return animation.get("sets").is_some_and(|sets| sets == "rotation")
            || animation
                .get("faces")
                .is_some_and(|faces| faces == "tangent");
    }
    if animation["type"] == "simulate" {
        return animation["sets"]
            .as_array()
            .is_some_and(|sets| sets.iter().any(|set| set.get("rotation").is_some()));
    }
    animation["type"] == "rotate"
}

/// The object id and span of each animation that turns its object.
fn rotations(lesson: &Lesson) -> Vec<(String, f64, f64)> {
    steps(lesson)
        .into_iter()
        .flat_map(|(_, step, start, end)| animation_spans(step, start, end))
        .filter(|(animation, _, _)| turns(animation))
        .flat_map(|(animation, first, last)| {
            let turned: Vec<String> = match animation["sets"]
                .as_array()
                .filter(|_| animation["type"] == "simulate")
            {
                Some(sets) => sets
                    .iter()
                    .filter(|set| set.get("rotation").is_some())
                    .map(|set| as_written(&set["object"]))
                    .collect(),
                None => vec![as_written(&animation["object"])],
            };
            turned.into_iter().map(move |object| (object, first, last))
        })
        .collect()
}

/// Each object a fade or a follow of its opacity changes the object opacity of, with the span.
fn opacity_changes(lesson: &Lesson) -> Vec<(String, f64, f64)> {
    steps(lesson)
        .into_iter()
        .flat_map(|(_, step, start, end)| animation_spans(step, start, end))
        .filter(|(animation, _, _)| animation["type"] == "fade" || sets_opacity(animation))
        .map(|(animation, first, last)| (as_written(&animation["object"]), first, last))
        .collect()
}

/// Whether an animation is a follow whose law sets an object opacity, which may fade the object
/// out or in as a fade does.
fn sets_opacity(animation: &Value) -> bool {
    animation["type"] == "follow" && animation.get("sets").is_some_and(|sets| sets == "opacity")
}

/// Each object that a create, write, morph or reaction makes but that never shows: the scene file
/// declares it at object opacity 0, none of those raises it, and no fade or follow of its opacity
/// does. Each is found at the
/// end of the first animation that makes it.
fn never_shown(lesson: &Lesson) -> Vec<(String, CheckedTime)> {
    let mut made: Vec<(String, &str, CheckedTime)> = Vec::new();
    let mut faded_in: HashSet<String> = HashSet::new();
    for (wording, step, start, end) in steps(lesson) {
        for (animation, _, last) in animation_spans(step, start, end) {
            let animation_type = animation["type"].as_str().unwrap_or_default();
            let objects: Vec<String> = match animation_type {
                "create" | "write" => vec![as_written(&animation["object"])],
                "morph" => vec![as_written(&animation["into"])],
                "reaction" => animation["products"]
                    .as_array()
                    .map(|products| products.iter().map(as_written).collect())
                    .unwrap_or_default(),
                "fade" if number(&animation["to"]) > 0.0 => {
                    faded_in.insert(as_written(&animation["object"]));
                    Vec::new()
                }
                "follow" if sets_opacity(animation) => {
                    faded_in.insert(as_written(&animation["object"]));
                    Vec::new()
                }
                _ => Vec::new(),
            };
            for object_id in objects {
                let what = if animation_type == "morph" {
                    format!("the morph into \"{object_id}\"")
                } else {
                    format!("the {animation_type} of \"{object_id}\"")
                };
                made.push((
                    object_id,
                    animation_type,
                    CheckedTime {
                        wording: format!("end of {what} in {wording}"),
                        scene_time: last,
                        on_sheet: false,
                        ends_step: false,
                    },
                ));
            }
        }
    }
    let declared_hidden = |object_id: &str| {
        lesson.file["objects"][object_id]
            .get("opacity")
            .is_some_and(|opacity| number(opacity) == 0.0)
    };
    let mut reported = HashSet::new();
    made.into_iter()
        .filter(|(object_id, _, _)| declared_hidden(object_id) && !faded_in.contains(object_id) && reported.insert(object_id.clone()))
        .map(|(object_id, animation_type, at)| (format!("\"{object_id}\" never shows: it is declared at opacity 0, which a {animation_type} does not raise, and no fade or follow of its opacity raises it"), at))
        .collect()
}

/// The span of a follow or a simulation, every frame time of which `check-layout` checks, with
/// it in words.
struct FrameSpan {
    wording: String,
    first: f64,
    last: f64,
    of: SpanOf,
}

enum SpanOf {
    Follow,
    Simulation,
}

/// Each follow and simulation in words, with its span.
fn frame_spans(lesson: &Lesson) -> Vec<FrameSpan> {
    let what = |animation: &Value| {
        if animation.get("along").is_some() {
            return format!(
                "{} along {}",
                json_text(&animation["object"], true),
                json_text(&animation["along"], true)
            );
        }
        let of = if animation.get("object").is_some() {
            format!(" of {}", json_text(&animation["object"], true))
        } else {
            String::new()
        };
        animation
            .get("sets")
            .map_or("position".to_string(), as_written)
            + &of
    };
    let mut spans = Vec::new();
    for (wording, step, start, end) in steps(lesson) {
        for (number, (animation, first, last)) in
            animation_spans(step, start, end).into_iter().enumerate()
        {
            if animation["type"] == "follow" {
                spans.push(FrameSpan {
                    wording: format!("follow {number} ({}) of {wording}", what(animation)),
                    first,
                    last,
                    of: SpanOf::Follow,
                });
            }
            if animation["type"] == "simulate" {
                let variables: Vec<String> = animation["variables"]
                    .as_object()
                    .map(|variables| variables.keys().cloned().collect())
                    .unwrap_or_default();
                spans.push(FrameSpan {
                    wording: format!(
                        "simulation {number} (of {}) of {wording}",
                        variables.join(", ")
                    ),
                    first,
                    last,
                    of: SpanOf::Simulation,
                });
            }
        }
    }
    spans
}

/// A frame time of one or more follows or simulations, worded as the first of them in the lesson.
struct FrameTime {
    stage_time: SceneTime,
    wording: String,
    of_follow: bool,
    of_simulation: bool,
}

/// Every frame time of every follow's and simulation's span, in order and each once. Follows that
/// run at once share their frame times, and a stage is the same whichever follow it is checked for.
fn frame_times(scene: &Scene, spans: &[FrameSpan]) -> Result<Vec<FrameTime>, Vec<ToolError>> {
    let mut times: BTreeMap<u64, FrameTime> = BTreeMap::new();
    for FrameSpan {
        wording,
        first,
        last,
        of,
    } in spans
    {
        let (from, to) = (scene_time(scene, *first)?, scene_time(scene, *last)?);
        for stage_time in scene.stage_times(from, to) {
            // Scene times are never negative, so their bits sort as the times do.
            let frame = times
                .entry(stage_time.seconds().to_bits())
                .or_insert_with(|| FrameTime {
                    stage_time,
                    wording: wording.clone(),
                    of_follow: false,
                    of_simulation: false,
                });
            match of {
                SpanOf::Follow => frame.of_follow = true,
                SpanOf::Simulation => frame.of_simulation = true,
            }
        }
    }
    Ok(times.into_values().collect())
}

/// `check` of each of `work`, in its order, worked out on every core. Each stage is read and
/// checked on its own, so only the recording of what was found needs the order.
fn each_in_parallel<T: Sync, R: Send>(work: &[T], check: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism()
        .map_or(1, usize::from)
        .clamp(1, work.len().max(1));
    let mut done: Vec<Option<R>> = std::iter::repeat_with(|| None).take(work.len()).collect();
    std::thread::scope(|scope| {
        let check = &check;
        let workers: Vec<_> = (0..threads)
            .map(|first| {
                scope.spawn(move || {
                    work.iter()
                        .enumerate()
                        .skip(first)
                        .step_by(threads)
                        .map(|(index, piece)| (index, check(piece)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for worker in workers {
            for (index, checked) in worker
                .join()
                .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
            {
                done[index] = Some(checked);
            }
        }
    });
    done.into_iter()
        .map(|checked| checked.expect("every piece is checked"))
        .collect()
}

/// The object ids that one of `spans` is changing at a scene time, strictly inside its span, such
/// as the ids a rotate is turning.
fn changing(spans: &[(String, f64, f64)], scene_time: f64) -> HashSet<&str> {
    spans
        .iter()
        .filter(|(_, first, last)| *first < scene_time && scene_time < *last)
        .map(|(id, _, _)| id.as_str())
        .collect()
}

/// The scene times to check, in order: the middle of each play step, the end of each step and
/// chapter, and the moment before a step's fades to 0 and follows of opacity begin, all on the contact sheet, and the
/// start, middle and end of each animation's span, which are not; a follow's middle is among
/// its frame times, which `frame_spans` lists. Times within a millisecond are one checked scene
/// time, the sheet's if any of them is.
fn checked_times(lesson: &Lesson, middles: bool) -> Vec<CheckedTime> {
    let at = |wording: String, scene_time: f64, on_sheet: bool| CheckedTime {
        wording,
        scene_time,
        on_sheet,
        ends_step: false,
    };
    let step_end = |wording: String, scene_time: f64| CheckedTime {
        wording,
        scene_time,
        on_sheet: true,
        ends_step: true,
    };
    let mut times = Vec::new();
    for (wording, step, start, end) in steps(lesson) {
        if step["type"] == "play" {
            let spans = animation_spans(step, start, end);
            if middles {
                times.push(at(
                    format!("middle of {wording}"),
                    (start + end) / 2.0,
                    true,
                ));
                for (number, (animation, first, last)) in spans.iter().enumerate() {
                    let animation_type = as_written(&animation["type"]);
                    let what = if animation.get("object").is_some() {
                        format!(
                            "{animation_type} of \"{}\"",
                            as_written(&animation["object"])
                        )
                    } else {
                        animation_type
                    };
                    let moments: &[(&str, f64)] =
                        if animation["type"] == "follow" || animation["type"] == "simulate" {
                            &[("start", *first), ("end", *last)]
                        } else {
                            &[
                                ("start", *first),
                                ("middle", (first + last) / 2.0),
                                ("end", *last),
                            ]
                        };
                    for (moment, scene_time) in moments {
                        times.push(at(
                            format!("{moment} of animation {number} ({what}) of {wording}"),
                            *scene_time,
                            false,
                        ));
                    }
                }
            }
            let fades = spans
                .iter()
                .filter(|(animation, _, _)| {
                    (animation["type"] == "fade" && number(&animation["to"]) == 0.0)
                        || sets_opacity(animation)
                })
                .map(|(_, first, _)| *first);
            if let Some(earliest) = fades.reduce(f64::min)
                && earliest > start
            {
                times.push(at(format!("before the fades of {wording}"), earliest, true));
            }
        }
        times.push(step_end(format!("end of {wording}"), end));
    }
    for chapter in lesson.scene.chapters() {
        times.push(step_end(
            format!(
                "end of chapter \"{}\", held to its min_duration",
                &*chapter.id
            ),
            chapter.end,
        ));
    }
    let millisecond = |scene_time: f64| {
        format!("{scene_time:.3}")
            .parse::<f64>()
            .unwrap_or(scene_time)
    };
    times.sort_by(|a, b| {
        millisecond(a.scene_time)
            .total_cmp(&millisecond(b.scene_time))
            .then(b.on_sheet.cmp(&a.on_sheet))
    });
    times.dedup_by(|later, earlier| {
        let one = millisecond(later.scene_time) == millisecond(earlier.scene_time);
        earlier.ends_step |= one && later.ends_step;
        one
    });
    if times.is_empty() {
        times.push(at("the only frame".to_string(), 0.0, true));
    }
    times
}

/// The report and the scene file with each table's cells beside it as text and math objects of
/// their own, `table/row/column`, and each lit cell's glow as a filled rectangle under its text,
/// `table/row/column/glow`, so every check reads a cell's text as it reads any text, and its
/// contrast against the glow. `None` when the report has no table.
fn with_cells(report: &Value, file: &Value) -> Option<(Value, Value)> {
    let tables: Vec<(&String, &Value)> = report["objects"]
        .as_object()?
        .iter()
        .filter(|(_, inspected)| inspected.get("table").is_some())
        .collect();
    if tables.is_empty() {
        return None;
    }
    let (mut report_with, mut file_with) = (report.clone(), file.clone());
    for (table_id, inspected) in tables {
        let table = &inspected["table"];
        let placed = |fields: serde_json::Map<String, Value>| {
            let mut fields = fields;
            for key in ["position", "rotation", "scale", "plane_map"] {
                if let Some(value) = inspected.get(key) {
                    fields.insert(key.to_string(), value.clone());
                }
            }
            Value::Object(fields)
        };
        for cell in table["cells"].as_array().into_iter().flatten() {
            if !truthy(cell.get("bounding_box")) {
                continue;
            }
            let cell_id = format!(
                "{table_id}/{}/{}",
                as_written(&cell["cell"][0]),
                as_written(&cell["cell"][1])
            );
            let (kind, words) = match cell.get("tex") {
                Some(tex) => ("math", ("tex", tex.clone())),
                None => ("text", ("text", cell["text"].clone())),
            };
            report_with["objects"][&cell_id] = placed(serde_json::Map::from_iter([
                ("cell_of".to_string(), Value::from(table_id.as_str())),
                ("glyphs".to_string(), Value::from(1)),
                ("opacity".to_string(), inspected["opacity"].clone()),
                ("created".to_string(), cell["created"].clone()),
                ("colors".to_string(), cell["colors"].clone()),
                ("bounding_box".to_string(), cell["bounding_box"].clone()),
                ("in_frame".to_string(), Value::from("whole")),
            ]));
            file_with["objects"][&cell_id] = serde_json::json!({
                "type": kind, words.0: words.1, "size": table["size"]
            });
            let glow = number(&cell["glow"]);
            if glow > 0.0 {
                let glow_id = format!("{cell_id}/glow");
                report_with["objects"][&glow_id] = placed(serde_json::Map::from_iter([
                    ("cell_of".to_string(), Value::from(table_id.as_str())),
                    ("glow_of".to_string(), Value::from(cell_id.as_str())),
                    (
                        "opacity".to_string(),
                        Value::from(number(&inspected["opacity"]) * glow),
                    ),
                    ("created".to_string(), Value::from(1)),
                    (
                        "colors".to_string(),
                        serde_json::json!({ "fill": cell["glow_color"] }),
                    ),
                    ("bounding_box".to_string(), cell["rectangle"].clone()),
                    ("in_frame".to_string(), Value::from("whole")),
                ]));
                file_with["objects"][&glow_id] = serde_json::json!({
                    "type": "polygon",
                    "fill": { "color": cell["glow_color"], "opacity": crate::table::FLAT_GLOW_OPACITY }
                });
            }
        }
    }
    Some((report_with, file_with))
}

/// The table a cell or a cell's glow stands in for, or the object itself.
fn table_of<'a>(object_id: &'a str, inspected: &'a Value) -> &'a str {
    inspected
        .get("cell_of")
        .and_then(Value::as_str)
        .unwrap_or(object_id)
}

/// Each object that shows at this scene time and has a bounding box, in the report's order.
fn shown(report: &Value) -> Vec<(&str, &Value)> {
    report["objects"]
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(_, inspected)| {
            number(&inspected["opacity"]) > 0.0
                && number(&inspected["created"]) > 0.0
                && truthy(inspected.get("bounding_box"))
        })
        .map(|(id, inspected)| (id.as_str(), inspected))
        .collect()
}

fn pixel_box(inspected: &Value) -> PixelBox {
    PixelBox::of(&inspected["bounding_box"]["pixels"])
}

/// How far two objects' boxes overlap, in whole pixels, when they do by more than the tolerance.
fn overlap(a: &Value, b: &Value) -> Option<(i64, i64)> {
    let (a, b) = (pixel_box(a), pixel_box(b));
    let width = a.max[0].min(b.max[0]) - a.min[0].max(b.min[0]);
    let height = a.max[1].min(b.max[1]) - a.min[1].max(b.min[1]);
    (width > TOLERANCE && height > TOLERANCE).then(|| {
        (
            width.round_ties_even() as i64,
            height.round_ties_even() as i64,
        )
    })
}

fn channels(color: &str) -> [f64; 3] {
    [1, 3, 5].map(|i| {
        color
            .get(i..i + 2)
            .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            .map_or(0.0, f64::from)
    })
}

fn to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(color: &str) -> f64 {
    let [r, g, b] = channels(color).map(|c| to_linear(c / 255.0));
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn contrast(a: &str, b: &str) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    let (light, dark) = if a >= b { (a, b) } else { (b, a) };
    (light + 0.05) / (dark + 0.05)
}

/// The colors a text or math object's glyphs are filled in at this scene time, as inspect
/// reports them. Mid-morph it reports none, so they are the declared color and every hex color
/// its LaTeX gives a term.
fn text_colors(inspected: &Value, declared: &Value) -> BTreeSet<String> {
    let colors = &inspected["colors"];
    if let Some(color) = colors.get("color") {
        return BTreeSet::from([as_written(color)]);
    }
    if let Some(glyph_colors) = colors.get("glyph_colors") {
        return glyph_colors
            .as_array()
            .into_iter()
            .flatten()
            .map(as_written)
            .collect();
    }
    let mut found = BTreeSet::from([declared
        .get("color")
        .map_or(DEFAULT_COLOR.to_string(), as_written)
        .to_lowercase()]);
    found.extend(
        tex_colors(declared["tex"].as_str().unwrap_or_default())
            .into_iter()
            .map(|color| color.to_lowercase()),
    );
    found
}

/// Each `#rrggbb` that `\color{...}` or `\textcolor{...}` names in LaTeX.
fn tex_colors(tex: &str) -> Vec<&str> {
    let mut found = Vec::new();
    let mut rest = tex;
    while let Some(at) = rest.find('\\') {
        rest = &rest[at + 1..];
        let after = rest
            .strip_prefix("textcolor{")
            .or_else(|| rest.strip_prefix("color{"));
        if let Some(after) = after
            && let Some(color) = after.get(..7)
            && color.starts_with('#')
            && color[1..].chars().all(|c| c.is_ascii_hexdigit())
            && after[7..].starts_with('}')
        {
            found.push(color);
            rest = &after[8..];
        }
    }
    found
}

/// The pixels between two boxes that do not overlap, along the axis that separates them.
fn gap(a: &Value, b: &Value) -> Option<f64> {
    let (a, b) = (pixel_box(a), pixel_box(b));
    let apart = [0, 1].map(|axis| (b.min[axis] - a.max[axis]).max(a.min[axis] - b.max[axis]));
    let (most, least) = (apart[0].max(apart[1]), apart[0].min(apart[1]));
    (most >= 0.0 && least < 0.0).then_some(most)
}

/// A color shown at a share of its opacity over the background, mixed in linear light as the
/// renderer blends it.
fn over(color: &str, background: &str, share: f64) -> String {
    let (color, background) = (channels(color), channels(background));
    let mixed = [0, 1, 2].map(|i| {
        to_byte(
            to_linear(color[i] / 255.0) * share + to_linear(background[i] / 255.0) * (1.0 - share),
        )
    });
    format!("#{:02x}{:02x}{:02x}", mixed[0], mixed[1], mixed[2])
}

/// A channel in linear light as an sRGB byte.
fn to_byte(c: f64) -> i64 {
    let c = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (c.clamp(0.0, 1.0) * 255.0).round_ties_even() as i64
}

/// A solid's color as the key light shows it: exactly on the side toward the light, and two
/// thirds of it in linear light on the side toward the camera, where a text over it usually sits.
fn lit(color: &str) -> [String; 2] {
    let shade = channels(color).map(|c| to_byte(to_linear(c / 255.0) * 2.0 / 3.0));
    [
        color.to_string(),
        format!("#{:02x}{:02x}{:02x}", shade[0], shade[1], shade[2]),
    ]
}

fn holds_center(pixels: PixelBox, center: [f64; 2]) -> bool {
    (0..2).all(|axis| pixels.min[axis] <= center[axis] && center[axis] <= pixels.max[axis])
}

/// The colors a surface shows a text, each with its opacity and what it is: a declared fill in
/// its color at this scene time, or a solid's color as the key light shades it.
fn surface_colors(colors: &Value, declared: &Value) -> Vec<(String, f64, String)> {
    if truthy(declared.get("fill")) {
        let fill = &declared["fill"];
        let color = as_written(colors.get("fill").unwrap_or(&fill["color"]));
        return vec![(
            color,
            fill.get("opacity").map_or(1.0, number),
            "the fill".to_string(),
        )];
    }
    if declared
        .get("type")
        .and_then(Value::as_str)
        .is_some_and(|kind| SOLIDS.contains(&kind))
    {
        let color = colors
            .get("color")
            .or_else(|| declared["material"].get("color"))
            .map_or("#ffffff".to_string(), as_written);
        return lit(&color)
            .into_iter()
            .map(|shade| (shade, 1.0, "the solid".to_string()))
            .collect();
    }
    Vec::new()
}

/// The colors a text is read against, each with what shows it: the background, and each fill,
/// solid or shown copy of a pattern whose box holds the text's center, in its color at this scene
/// time, mixed by its opacity.
fn behind(
    text: &Value,
    objects: &[(&str, &Value)],
    declared_objects: &Value,
    background: &str,
) -> Vec<(String, String)> {
    let pixels = pixel_box(text);
    let center = [0, 1].map(|axis| (pixels.min[axis] + pixels.max[axis]) / 2.0);
    let mut colors = vec![(background.to_string(), "the background".to_string())];
    for (object_id, inspected) in objects {
        if std::ptr::eq(*inspected, text) {
            continue;
        }
        let declared = &declared_objects[*object_id];
        let mut surfaces = Vec::new();
        if inspected.get("pattern").is_some() {
            let shape = &declared["shape"];
            for copy in shown_copies(inspected) {
                if truthy(copy["bounding_box"].get("pixels"))
                    && holds_center(PixelBox::of(&copy["bounding_box"]["pixels"]), center)
                {
                    let what = format!(
                        "copy {} of \"{object_id}\"",
                        json_text(&copy["index"], true)
                    );
                    surfaces.extend(
                        surface_colors(&copy["colors"], shape)
                            .into_iter()
                            .map(|(color, opacity, _)| (color, opacity, what.clone())),
                    );
                }
            }
        } else if holds_center(pixel_box(inspected), center)
            && !off_the_disc(text, inspected, declared)
        {
            surfaces.extend(surface_colors(&inspected["colors"], declared));
        }
        let opacity = number(&inspected["opacity"]);
        colors.extend(
            surfaces
                .into_iter()
                .map(|(color, share, what)| (over(&color, background, share * opacity), what)),
        );
    }
    colors
}

/// Whether a text's center, in the world, lies off the disc of a circle with no x or y turn whose
/// box holds it, as in a corner of the box or beside a disc a plane map shears.
fn off_the_disc(text: &Value, circle: &Value, declared: &Value) -> bool {
    let rotation = &circle["rotation"];
    if declared["type"] != "circle" || number(&rotation[0]) != 0.0 || number(&rotation[1]) != 0.0 {
        return false;
    }
    let world = &text["bounding_box"]["world"];
    let mut point =
        [0, 1].map(|axis| (number(&world["min"][axis]) + number(&world["max"][axis])) / 2.0);
    let plane_map = &circle["plane_map"];
    if !plane_map.is_null() {
        let matrix = &plane_map["matrix"];
        let (a, b, c, d) = (
            number(&matrix[0][0]),
            number(&matrix[0][1]),
            number(&matrix[1][0]),
            number(&matrix[1][1]),
        );
        let determinant = a * d - b * c;
        if determinant == 0.0 {
            return false;
        }
        let [x, y] = [0, 1].map(|axis| point[axis] - number(&plane_map["shift"][axis]));
        point = [(d * x - b * y) / determinant, (a * y - c * x) / determinant];
    }
    let [x, y] = [0, 1].map(|axis| point[axis] - number(&circle["position"][axis]));
    let turn = number(&rotation[2]).to_radians();
    let local = [
        (x * turn.cos() + y * turn.sin()) / number(&circle["scale"][0]),
        (y * turn.cos() - x * turn.sin()) / number(&circle["scale"][1]),
    ];
    local[0].hypot(local[1]) > number(&declared["radius"])
}

/// The copies of a pattern that show at this scene time, each with a box on the frame.
fn shown_copies(inspected: &Value) -> Vec<&Value> {
    let copies = inspected["pattern"]["copies"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    copies
        .iter()
        .filter(|copy| {
            number(&copy["created"]) > 0.0
                && number(&copy["opacity"]) > 0.0
                && truthy(copy.get("bounding_box"))
        })
        .collect()
}

/// Copies by their index, the first few of them.
fn copy_list(copies: &[&Value]) -> String {
    let named: Vec<String> = copies
        .iter()
        .take(MOST_COPIES_NAMED)
        .map(|copy| json_text(&copy["index"], true))
        .collect();
    let more = if copies.len() > MOST_COPIES_NAMED {
        format!(" and {} more", copies.len() - MOST_COPIES_NAMED)
    } else {
        String::new()
    };
    named.join(", ") + &more
}

/// How tall one em of a text or math object stands in the world: its text size times its y scale,
/// or under a plane map its height across the line its baseline lands on. The map, the object's z
/// turn and its scale make one matrix A, and an em stands |det A| / |A (1, 0)| across the mapped
/// baseline, so a shear along the baseline keeps it and a map that flattens the glyphs onto their
/// baseline takes it to 0, where the length of A (0, 1) would grow under both.
fn em_height(size: f64, inspected: &Value) -> f64 {
    let plane_map = &inspected["plane_map"];
    if plane_map.is_null() {
        return size * number(&inspected["scale"][1]);
    }
    let matrix = &plane_map["matrix"];
    let (a, b, c, d) = (
        number(&matrix[0][0]),
        number(&matrix[0][1]),
        number(&matrix[1][0]),
        number(&matrix[1][1]),
    );
    let turn = number(&inspected["rotation"][2]).to_radians();
    let (sx, sy) = (
        number(&inspected["scale"][0]),
        number(&inspected["scale"][1]),
    );
    let baseline =
        ((a * turn.cos() + b * turn.sin()) * sx).hypot((c * turn.cos() + d * turn.sin()) * sx);
    if baseline != 0.0 {
        size * ((a * d - b * c) * sx * sy).abs() / baseline
    } else {
        0.0
    }
}

fn text_faults(
    object_id: &str,
    inspected: &Value,
    declared: &Value,
    file: &Value,
    objects: &[(&str, &Value)],
) -> Vec<String> {
    let mut found = Vec::new();
    let scene_config = &file["scene_config"];
    let world = PixelBox::of(&inspected["bounding_box"]["world"]);
    let pixels = pixel_box(inspected);
    let (world_height, pixel_height) = (world.max[1] - world.min[1], pixels.max[1] - pixels.min[1]);
    if world_height > 0.0 {
        let em = em_height(
            declared.get("size").map_or(DEFAULT_TEXT_SIZE, number),
            inspected,
        ) * pixel_height
            / world_height;
        let frame_height = scene_config
            .get("height")
            .cloned()
            .unwrap_or(Value::from(DEFAULT_HEIGHT));
        let least = SMALLEST_EM * number(&frame_height);
        if em < least {
            found.push(format!(
                "\"{object_id}\" is too small to read: its em is {em:.0} pixels in a frame {} pixels high, and needs {least:.0}",
                as_written(&frame_height)
            ));
        }
    }
    let background = scene_config
        .get("background")
        .map_or(DEFAULT_BACKGROUND.to_string(), as_written);
    if background == "transparent" {
        return found;
    }
    let opacity = number(&inspected["opacity"]);
    for (under, surface) in behind(inspected, objects, &file["objects"], &background) {
        let what = if surface == "the background" {
            surface.clone()
        } else {
            format!("{surface} behind it, which shows as")
        };
        found.extend(
            text_colors(inspected, declared)
                .into_iter()
                .filter_map(|color| hard_to_read(object_id, &color, opacity, &what, &under)),
        );
    }
    found
}

/// The layout fault of a text in `color` at an object opacity over `under`, which `what` names,
/// when its contrast is below the least: a faded text shows as its color mixed toward what is
/// behind it.
fn hard_to_read(
    object_id: &str,
    color: &str,
    opacity: f64,
    what: &str,
    under: &str,
) -> Option<String> {
    let shows = over(color, under, opacity);
    let ratio = contrast(&shows, under);
    let faded = if opacity < 1.0 {
        format!(
            " at object opacity {} shows as {shows}, and",
            general(opacity)
        )
    } else {
        String::new()
    };
    (ratio < LEAST_CONTRAST).then(|| format!("\"{object_id}\" is hard to read: {color}{faded} on {what} {under} has contrast {ratio:.1}, and needs {LEAST_CONTRAST}"))
}

fn axis_aligned_rectangle(points: &Value) -> bool {
    let points = points.as_array().map(Vec::as_slice).unwrap_or_default();
    if points.len() != 4 {
        return false;
    }
    let distinct = |axis: usize| {
        let mut values: Vec<f64> = Vec::new();
        for point in points {
            let value = number(&point[axis]);
            if !values.contains(&value) {
                values.push(value);
            }
        }
        values
    };
    let (xs, ys) = (distinct(0), distinct(1));
    let corner = |point: &Value| point.as_array().is_some_and(|point| point.len() == 2);
    let corners: Vec<[f64; 2]> = points
        .iter()
        .map(|point| [number(&point[0]), number(&point[1])])
        .collect();
    xs.len() == 2
        && ys.len() == 2
        && points.iter().all(corner)
        && xs
            .iter()
            .all(|x| ys.iter().all(|y| corners.contains(&[*x, *y])))
        && (0..4).all(|i| {
            let (a, b) = (corners[i], corners[(i + 1) % 4]);
            (a[0] == b[0]) != (a[1] == b[1])
        })
}

fn crosses(a: PixelBox, b: PixelBox) -> bool {
    (0..2).all(|axis| a.max[axis].min(b.max[axis]) > a.min[axis].max(b.min[axis]))
}

fn covers_with_margin(outer: PixelBox, inner: PixelBox) -> bool {
    (0..2).all(|axis| {
        outer.min[axis] + TOLERANCE < inner.min[axis]
            && inner.max[axis] < outer.max[axis] - TOLERANCE
    })
}

fn transparent_panel_color(
    text_id: &str,
    text: &Value,
    report: &Value,
    declared_objects: &Value,
) -> Option<String> {
    let camera = &report["camera"];
    let (position, target, up) = (&camera["position"], &camera["target"], &camera["up"]);
    let same = |a: &Value, b: &Value| (0..2).all(|i| number(&a[i]) == number(&b[i]));
    if !same(position, target)
        || number(&position[2]) <= number(&target[2])
        || number(&up[0]) != 0.0
        || number(&up[1]) <= 0.0
    {
        return None;
    }
    if report["reactions"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|reaction| reaction["phase"] == "during")
    {
        return None;
    }
    let text_box = &text["bounding_box"];
    let objects = shown(report);
    for (panel_id, panel) in &objects {
        if *panel_id == text_id {
            continue;
        }
        let declared = &declared_objects[*panel_id];
        let fill = &declared["fill"];
        let unturned = (0..3).all(|i| number(&panel["rotation"][i]) == 0.0)
            && panel["rotation"].as_array().is_some_and(|r| r.len() == 3);
        if declared["type"] != "polygon"
            || !axis_aligned_rectangle(&declared["points"])
            || declared
                .get("stroke")
                .is_some_and(|stroke| !stroke.is_null())
            || !truthy(declared.get("fill"))
            || fill.get("opacity").map_or(1.0, number) != 1.0
            || number(&panel["opacity"]) != 1.0
            || number(&panel["created"]) != 1.0
            || truthy(panel.get("morph"))
            || truthy(panel.get("plane_map"))
            || !unturned
            || panel["in_frame"] != "whole"
        {
            continue;
        }
        let panel_box = &panel["bounding_box"];
        if !covers_with_margin(
            PixelBox::of(&panel_box["pixels"]),
            PixelBox::of(&text_box["pixels"]),
        ) {
            continue;
        }
        if number(&text_box["world"]["min"][2]) <= number(&panel_box["world"]["max"][2]) {
            continue;
        }
        let obscured = objects.iter().any(|(other_id, other)| {
            *other_id != *panel_id
                && *other_id != text_id
                && crosses(PixelBox::of(&text_box["pixels"]), pixel_box(other))
        });
        if obscured {
            continue;
        }
        let glowing = report["objects"]
            .as_object()
            .into_iter()
            .flatten()
            .flat_map(|(_, other)| other["groups"].as_object().into_iter().flatten())
            .any(|(_, group)| {
                number(&group["glow"]) > 0.0
                    && (!truthy(group.get("bounding_box"))
                        || !truthy(group["bounding_box"].get("pixels"))
                        || crosses(
                            PixelBox::of(&text_box["pixels"]),
                            PixelBox::of(&group["bounding_box"]["pixels"]),
                        ))
            });
        if glowing {
            continue;
        }
        return Some(as_written(
            panel["colors"].get("fill").unwrap_or(&fill["color"]),
        ));
    }
    None
}

/// The contrast faults of a transparent scene, and each text whose contrast is unverified.
fn transparent_contrast(
    report: &Value,
    file: &Value,
    fading: &HashSet<&str>,
) -> (Vec<String>, Vec<String>) {
    let (mut faults, mut unverified) = (Vec::new(), Vec::new());
    let declared_objects = &file["objects"];
    for (object_id, inspected) in shown(report) {
        if inspected.get("glyphs").is_none() || !at_rest(object_id, inspected, fading) {
            continue;
        }
        let tilted = !(0..2).all(|i| number(&inspected["rotation"][i]) == 0.0);
        if number(&inspected["created"]) != 1.0
            || truthy(inspected.get("morph"))
            || truthy(inspected.get("plane_map"))
            || tilted
        {
            unverified.push(format!("\"{object_id}\" contrast unverified: its text is partly written, morphed, mapped, or tilted"));
            continue;
        }
        let Some(under) = transparent_panel_color(object_id, inspected, report, declared_objects)
        else {
            unverified.push(format!("\"{object_id}\" contrast unverified: no fully covering opaque rectangle is proved behind it"));
            continue;
        };
        let opacity = number(&inspected["opacity"]);
        faults.extend(
            text_colors(inspected, &declared_objects[object_id])
                .into_iter()
                .filter_map(|color| {
                    hard_to_read(
                        object_id,
                        &color,
                        opacity,
                        "the fill behind it, which shows as",
                        &under,
                    )
                }),
        );
    }
    (faults, unverified)
}

/// Whether a shown text's object opacity is at rest, so its size and contrast are judged: it is
/// at 1, or no fade or follow of its opacity is playing. A fade in passes through faint opacities
/// on its way to 1, which is not a fault.
fn at_rest(object_id: &str, inspected: &Value, fading: &HashSet<&str>) -> bool {
    number(&inspected["opacity"]) == 1.0 || !fading.contains(table_of(object_id, inspected))
}

fn reaction_faults(report: &Value) -> Vec<String> {
    let mut found = Vec::new();
    for (number_of, reaction) in report["reactions"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        if reaction["phase"] != "during" {
            continue;
        }
        let atoms: Vec<&Value> = reaction["atoms"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|atom| !truthy(Some(&atom["hidden"])))
            .collect();
        let reference_of = |reference: &Value| {
            (
                as_written(&reference["object"]),
                as_written(&reference["atom"]),
            )
        };
        // Atoms that share a bond on either side are meant to be close.
        let bonded: HashSet<BTreeSet<(String, String)>> = reaction["bonds"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|bond| {
                bond["atoms"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(reference_of)
                    .collect()
            })
            .collect();
        for (i, a) in atoms.iter().enumerate() {
            let scale = number(
                &report["objects"][a["reactant"]["object"].as_str().unwrap_or_default()]["scale"]
                    [0],
            );
            for b in &atoms[i + 1..] {
                if bonded.contains(&BTreeSet::from([
                    reference_of(&a["reactant"]),
                    reference_of(&b["reactant"]),
                ])) {
                    continue;
                }
                let distance = sum((0..3).map(|axis| {
                    (number(&a["position"][axis]) - number(&b["position"][axis])).powi(2)
                }))
                .sqrt();
                if distance < NEAREST_ATOMS * scale {
                    found.push(format!(
                        "in reaction {number_of}, atom {} of \"{}\" and atom {} of \"{}\" pass {:.2} angstroms apart: move a reactant or a product so their paths do not cross",
                        as_written(&a["reactant"]["atom"]),
                        as_written(&a["reactant"]["object"]),
                        as_written(&b["reactant"]["atom"]),
                        as_written(&b["reactant"]["object"]),
                        distance / scale
                    ));
                }
            }
        }
    }
    found
}

/// Two shown atoms with no bond between them, in one molecule or two, whose balls rest nearly
/// touching once a step's animations have finished. A ball is a quarter of its element's van der
/// Waals radius, so the balls of a bonded pair stand 0.14 (H-H) to 0.7 (C-C) angstroms apart with
/// a stick between them, 0.37 for C-H, the bond drawn most. Two unbonded balls nearer than
/// `BALLS_APART`, under that, nearly touch and read as a bonded pair with its stick left out; two
/// loose oxygens 1.03 apart (0.25 between balls) read as O2, and a loose hydrogen and oxygen with
/// 0.45 between their balls read as two atoms. Atoms two bonds apart, such as a methyl's hydrogens,
/// rest more than 0.9 apart, so the molecules PubChem serves do not trip it.
fn atoms_that_read_as_bonded(report: &Value) -> Vec<String> {
    struct Placed<'a> {
        object_id: &'a str,
        number: u64,
        center: [f64; 3],
        radius: f64,
        scale: f64,
    }
    let mut balls = Vec::new();
    let mut bonded: HashSet<(&str, u64, u64)> = HashSet::new();
    for (object_id, inspected) in shown(report) {
        let Some(atoms) = inspected["atoms"].as_array() else {
            continue;
        };
        if !draws_sticks(inspected) {
            continue;
        }
        let scale = number(&inspected["scale"][0]);
        for atom in atoms.iter().filter(|atom| !truthy(Some(&atom["hidden"]))) {
            let Some(element) = atom["element"]
                .as_str()
                .and_then(crate::element::Element::from_symbol)
            else {
                continue;
            };
            let center = [0, 1, 2].map(|axis| number(&atom["position"][axis]));
            let radius = f64::from(element.radius() * crate::molecule::BALL_SHARE) * scale;
            balls.push(Placed {
                object_id,
                number: atom["number"].as_u64().unwrap_or_default(),
                center,
                radius,
                scale,
            });
        }
        for bond in inspected["bonds"].as_array().into_iter().flatten() {
            let [a, b] = [0, 1].map(|end| bond["atoms"][end].as_u64().unwrap_or_default());
            bonded.insert((object_id, a.min(b), a.max(b)));
        }
    }
    let mut found = Vec::new();
    for (i, a) in balls.iter().enumerate() {
        for b in &balls[i + 1..] {
            if a.object_id == b.object_id
                && bonded.contains(&(a.object_id, a.number.min(b.number), a.number.max(b.number)))
            {
                continue;
            }
            let scale = a.scale.min(b.scale);
            let gap = sum((0..3).map(|axis| (a.center[axis] - b.center[axis]).powi(2))).sqrt()
                - a.radius
                - b.radius;
            if gap < BALLS_APART * scale {
                found.push(format!(
                    "atom {} of \"{}\" and atom {} of \"{}\" have no bond, and their balls rest {:.2} angstroms apart, so they read as bonded: move them until their balls are at least {BALLS_APART} angstroms apart",
                    a.number,
                    a.object_id,
                    b.number,
                    b.object_id,
                    gap / scale
                ));
            }
        }
    }
    found
}

/// Whether an inspected molecule is drawn ball-and-stick, whose small balls and sticks the
/// bonded-looking, hidden-atom and end-on-bond checks judge. A space-filling molecule's balls
/// overlap their neighbors and hide one another by design, and it draws no bond, so only the
/// discs of its balls are checked against text.
fn draws_sticks(inspected: &Value) -> bool {
    inspected["representation"] == "ball-and-stick"
}

fn vector(value: &Value) -> Vec<f64> {
    value.as_array().into_iter().flatten().map(number).collect()
}

/// Heavy atoms of a named group that sit nearly straight behind another shown atom, as seen
/// from the camera, on a molecule no rotate is turning. A hydrogen behind its neighbor, as in
/// any methyl group, a molecule caught mid-turn, and a molecule too crowded for any turn to clear
/// are not worth turning for.
fn hidden_group_atoms(
    report: &Value,
    objects: &[(&str, &Value)],
    turning: &HashSet<&str>,
) -> Vec<String> {
    let mut found = Vec::new();
    let camera = &report["camera"];
    let sight: Vec<f64> = vector(&camera["target"])
        .iter()
        .zip(vector(&camera["position"]))
        .map(|(t, p)| t - p)
        .collect();
    let length = sum(sight.iter().map(|c| c * c)).sqrt();
    let sight: Vec<f64> = sight.iter().map(|c| c / length).collect();
    let dot = |a: &[f64], b: &[f64]| sum(a.iter().zip(b).map(|(a, b)| a * b));
    for (object_id, inspected) in objects {
        if turning.contains(object_id) || !draws_sticks(inspected) {
            continue;
        }
        let mut atoms: Vec<(i64, &Value)> = Vec::new();
        for atom in inspected["atoms"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|atom| !truthy(Some(&atom["hidden"])))
        {
            let atom_number = atom["number"].as_i64().unwrap_or_default();
            match atoms.iter_mut().find(|(n, _)| *n == atom_number) {
                Some(entry) => entry.1 = atom,
                None => atoms.push((atom_number, atom)),
            }
        }
        if atoms.len() > CROWDED_ATOMS {
            continue;
        }
        let atom = |atom_number: i64| {
            atoms
                .iter()
                .find(|(n, _)| *n == atom_number)
                .map(|(_, atom)| *atom)
        };
        let groups: Vec<BTreeSet<i64>> = inspected["groups"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(_, group)| {
                group["atoms"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_i64)
                    .collect()
            })
            .collect();
        let talked_about: BTreeSet<i64> = groups.iter().flatten().copied().collect();
        let scale = number(&inspected["scale"][0]);
        for bond in inspected["bonds"].as_array().into_iter().flatten() {
            let (first, second) = (
                bond["atoms"][0].as_i64().unwrap_or_default(),
                bond["atoms"][1].as_i64().unwrap_or_default(),
            );
            if truthy(Some(&bond["hidden"])) {
                continue;
            }
            let (Some(first_atom), Some(second_atom)) = (atom(first), atom(second)) else {
                continue;
            };
            if first_atom["element"] == "H" || second_atom["element"] == "H" {
                continue;
            }
            if !groups
                .iter()
                .any(|group| group.contains(&first) && group.contains(&second))
            {
                continue;
            }
            let apart: Vec<f64> = vector(&second_atom["position"])
                .iter()
                .zip(vector(&first_atom["position"]))
                .map(|(p, q)| p - q)
                .collect();
            let along = dot(&apart, &sight).abs() / sum(apart.iter().map(|d| d * d)).sqrt();
            if along > END_ON {
                found.push(format!("the bond between atoms {first} and {second} of \"{object_id}\" points nearly at the camera: turn the molecule with `rotation` so it shows its length"));
            }
        }
        for atom_number in talked_about.iter().filter(|n| atom(**n).is_some()) {
            let this = atom(*atom_number).expect("filtered to shown atoms");
            if this["element"] == "H" {
                continue;
            }
            for (other_number, other) in &atoms {
                if std::ptr::eq(*other, this) {
                    continue;
                }
                let apart: Vec<f64> = vector(&other["position"])
                    .iter()
                    .zip(vector(&this["position"]))
                    .map(|(o, a)| o - a)
                    .collect();
                let along = dot(&apart, &sight);
                let across = sum(apart
                    .iter()
                    .zip(&sight)
                    .map(|(d, v)| (d - along * v).powi(2)))
                .sqrt();
                if along < 0.0 && across < NEAREST_ATOMS * scale {
                    found.push(format!("atom {atom_number} of \"{object_id}\" is hidden behind atom {other_number}: turn the molecule with `rotation` so it shows"));
                    break;
                }
            }
        }
    }
    found
}

/// The report of axes, a number line or 3D axes, or None for any other object.
fn ruling(inspected: &Value) -> Option<&Value> {
    ["axes", "number_line", "axes_3d"]
        .into_iter()
        .map(|key| inspected.get(key))
        .find(|report| truthy(*report))
        .flatten()
}

/// The parts of axes, a number line or 3D axes in pixels: each axis line with its ticks, each
/// tick label and each axis label.
fn axis_parts(inspected: &Value) -> Vec<(String, PixelBox)> {
    let Some(report) = ruling(inspected) else {
        return Vec::new();
    };
    let ticks = report["ticks"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut parts = Vec::new();
    for tick in ticks {
        if tick.get("label_bounding_box").is_some()
            && truthy(tick["label_bounding_box"].get("pixels"))
        {
            parts.push((
                format!("tick label {}", as_written(&tick["label_text"])),
                PixelBox::of(&tick["label_bounding_box"]["pixels"]),
            ));
        }
    }
    for axis in ["x", "y", "z"] {
        let own = report
            .get(axis)
            .filter(|own| truthy(Some(own)))
            .and_then(|own| own.get("label_bounding_box"))
            .filter(|label| truthy(Some(label)));
        let label = own.or_else(|| {
            report
                .get("label_bounding_box")
                .filter(|_| report.get("axis").is_some_and(|named| named == axis))
        });
        if truthy(label) {
            parts.push((
                format!("{axis} axis label"),
                PixelBox::of(&label.expect("truthy")["pixels"]),
            ));
        }
        let points: Vec<[f64; 2]> = ticks
            .iter()
            .filter(|tick| tick["axis"] == axis)
            .map(|tick| [number(&tick["pixels"][0]), number(&tick["pixels"][1])])
            .collect();
        if points.len() >= 2 {
            let low = [0, 1]
                .map(|i| points.iter().map(|p| p[i]).fold(f64::INFINITY, f64::min) - TICK_REACH);
            let high = [0, 1].map(|i| {
                points
                    .iter()
                    .map(|p| p[i])
                    .fold(f64::NEG_INFINITY, f64::max)
                    + TICK_REACH
            });
            parts.push((
                format!("{axis} axis"),
                PixelBox {
                    min: low,
                    max: high,
                },
            ));
        }
    }
    parts
}

fn inside(inner: &Value, outer: &Value) -> bool {
    let (a, b) = (pixel_box(inner), pixel_box(outer));
    (0..2).all(|i| b.min[i] <= a.min[i] && a.max[i] <= b.max[i])
}

fn boxes_overlap(a: PixelBox, b: PixelBox) -> bool {
    (0..2).all(|i| a.max[i].min(b.max[i]) - a.min[i].max(b.min[i]) > TOLERANCE)
}

/// Whether an inspected atom's ball, as the disc the frame shows, reaches into a pixel box. A
/// hidden atom has no ball and reaches nothing.
fn ball_reaches(atom: &Value, pixels: PixelBox) -> bool {
    let point = |at: [f64; 2]| glam::Vec2::new(at[0] as f32, at[1] as f32);
    atom.get("ball_radius").is_some_and(|radius| {
        crate::molecule::ball_meets_box(
            point([number(&atom["pixels"][0]), number(&atom["pixels"][1])]),
            number(radius) as f32,
            point(pixels.min),
            point(pixels.max),
        )
    })
}

/// The shown atoms whose balls reach into a text's box.
fn covered_atoms(text: &Value, molecule: &Value) -> Vec<Value> {
    let pixels = pixel_box(text);
    molecule["atoms"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|atom| ball_reaches(atom, pixels))
        .map(|atom| atom["number"].clone())
        .collect()
}

fn allows(allowed: &[BTreeSet<&str>], first: &str, second: &str) -> bool {
    let pair = BTreeSet::from([first, second]);
    allowed.contains(&pair)
}

fn attached_to(inspected: &Value) -> Option<&str> {
    inspected
        .get("attached")
        .and_then(|attached| attached.get("object"))
        .and_then(Value::as_str)
}

fn layout_faults(
    report: &Value,
    file: &Value,
    allowed: &[BTreeSet<&str>],
    turning: &HashSet<&str>,
    fading: &HashSet<&str>,
) -> Vec<String> {
    let objects = shown(report);
    let declared_objects = &file["objects"];
    let mut found = reaction_faults(report);
    found.extend(hidden_group_atoms(report, &objects, turning));
    for (object_id, inspected) in &objects {
        let plot = &inspected["plot"];
        for text_id in plot["crosses_objects"].as_array().into_iter().flatten() {
            let text_id = as_written(text_id);
            if !allows(allowed, object_id, &text_id)
                && !allows(
                    allowed,
                    object_id,
                    table_of(&text_id, &report["objects"][&text_id]),
                )
            {
                found.push(format!("\"{object_id}\" runs through \"{text_id}\""));
            }
        }
        for label in plot["crosses_labels"].as_array().into_iter().flatten() {
            let what = match label.get("value") {
                Some(value) => format!("tick label {}", general(number(value))),
                None => format!("axis label {}", json_text(&label["label"], true)),
            };
            found.push(format!(
                "\"{object_id}\" runs through the {} {what} of \"{}\": change the axis range or the plot's x_range, or move the axes",
                as_written(&label["axis"]),
                as_written(&plot["axes"])
            ));
        }
        if inspected.get("glyphs").is_some() && at_rest(object_id, inspected, fading) {
            found.extend(text_faults(
                object_id,
                inspected,
                &declared_objects[*object_id],
                file,
                &objects,
            ));
        }
        if inspected["in_frame"] != "whole" {
            let pixels = pixel_box(inspected);
            found.push(format!(
                "\"{object_id}\" is cut off by the frame: its box runs from {} to {} pixels",
                pixels_of(&pixels.min),
                pixels_of(&pixels.max)
            ));
        }
        if truthy(inspected.get("attached")) {
            let attached = &inspected["attached"];
            let to = as_written(&attached["object"]);
            if number(&report["objects"][&to]["opacity"]) == 0.0 {
                found.push(format!("\"{object_id}\" shows while \"{to}\", which it is attached to, is faded out: fade it with \"{to}\""));
            }
            if truthy(attached.get("covers_atoms")) && inspected.get("glyphs").is_some() {
                found.push(format!(
                    "\"{object_id}\" covers atoms {} of \"{to}\"",
                    as_written(&attached["covers_atoms"])
                ));
            }
        }
    }
    let mut ids: Vec<(&str, &Value)> = objects.clone();
    ids.sort_by(|a, b| a.0.cmp(b.0));
    let height = number(
        file["scene_config"]
            .get("height")
            .unwrap_or(&Value::from(DEFAULT_HEIGHT)),
    );
    for (i, (first, a)) in ids.iter().enumerate() {
        for (second, b) in &ids[i + 1..] {
            let (first, second, a, b) = (*first, *second, *a, *b);
            if (a.get("glyphs").is_none() && b.get("glyphs").is_none())
                || allows(allowed, first, second)
                || allows(allowed, table_of(first, a), table_of(second, b))
            {
                continue;
            }
            let other = |id: &str| if id == first { (second, b) } else { (first, a) };
            if let Some((table_id, table)) = [(first, a), (second, b)]
                .into_iter()
                .find(|(_, o)| o.get("table").is_some())
            {
                // Each cell's text is a text object of its own, checked against the other text; here the other text is checked against the cells' rectangles.
                let (text_id, text) = other(table_id);
                if text.get("glyphs").is_some() && table_of(text_id, text) != table_id {
                    let covered: Vec<String> = table["table"]["cells"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|cell| {
                            overlap(
                                &serde_json::json!({ "bounding_box": cell["rectangle"] }),
                                text,
                            )
                            .is_some()
                        })
                        .map(|cell| json_text(&cell["cell"], true))
                        .collect();
                    if !covered.is_empty() {
                        found.push(format!(
                            "\"{text_id}\" covers cells {} of \"{table_id}\"",
                            covered.join(", ")
                        ));
                    }
                }
                continue;
            }
            if let Some((molecule_id, molecule)) = [(first, a), (second, b)]
                .into_iter()
                .find(|(_, o)| o.get("atoms").is_some() && o["representation"] != "cartoon")
            {
                // Atoms have exact positions, so a text beside a molecule is checked against them, not its box. A cartoon shows no atom, so it is checked by its box below.
                let (text_id, text) = other(molecule_id);
                let covered = covered_atoms(text, molecule);
                if !covered.is_empty() && attached_to(text) != Some(molecule_id) {
                    found.push(format!(
                        "\"{text_id}\" covers atoms {} of \"{molecule_id}\"",
                        as_written(&Value::Array(covered))
                    ));
                }
                continue;
            }
            if let Some((ruled_id, ruled)) = [(first, a), (second, b)]
                .into_iter()
                .find(|(_, o)| ruling(o).is_some())
            {
                // Axes are mostly empty, so a text is checked against their lines, ticks and labels, not their box.
                let (text_id, text) = other(ruled_id);
                if text.get("glyphs").is_some() {
                    let hit: Vec<String> = axis_parts(ruled)
                        .into_iter()
                        .filter(|(_, part)| boxes_overlap(*part, pixel_box(text)))
                        .map(|(name, _)| name)
                        .collect();
                    if !hit.is_empty() {
                        found.push(format!(
                            "\"{text_id}\" covers the {} of \"{ruled_id}\"",
                            hit.join(", the ")
                        ));
                    }
                    continue;
                }
            }
            if let Some((patterned_id, patterned)) = [(first, a), (second, b)]
                .into_iter()
                .find(|(_, o)| o.get("pattern").is_some())
            {
                // A pattern is mostly empty between its copies, so a text is checked against each shown copy, not the pattern's box.
                let (text_id, text) = other(patterned_id);
                if text.get("glyphs").is_some() {
                    let filled = truthy(declared_objects[patterned_id]["shape"].get("fill"));
                    let text_box = pixel_box(text);
                    let hit: Vec<&Value> = shown_copies(patterned)
                        .into_iter()
                        .filter(|copy| {
                            if !truthy(copy["bounding_box"].get("pixels")) {
                                return false;
                            }
                            let copy_box = PixelBox::of(&copy["bounding_box"]["pixels"]);
                            boxes_overlap(copy_box, text_box)
                                && !(filled
                                    && (0..2).all(|i| {
                                        copy_box.min[i] <= text_box.min[i]
                                            && text_box.max[i] <= copy_box.max[i]
                                    }))
                        })
                        .collect();
                    if !hit.is_empty() {
                        found.push(format!(
                            "\"{text_id}\" overlaps copies {} of \"{patterned_id}\"",
                            copy_list(&hit)
                        ));
                    }
                    continue;
                }
            }
            if a.get("plot").is_some() || b.get("plot").is_some() {
                // A plot reports the text its stroke runs through, in crosses_objects, above.
                continue;
            }
            let filled = [(first, a), (second, b)].into_iter().find(|(id, o)| {
                truthy(declared_objects[*id].get("fill")) && o.get("glyphs").is_none()
            });
            if let Some((filled_id, filled)) = filled
                && inside(other(filled_id).1, filled)
            {
                // A text wholly inside a filled shape is set on the fill; its contrast is checked above.
                continue;
            }
            let on = [((first, a), (second, b)), ((second, b), (first, a))]
                .into_iter()
                .find(|((_, inner), (outer_id, _))| attached_to(inner) == Some(*outer_id));
            if let Some(((_, inner), (_, outer))) = on
                && inside(inner, outer)
            {
                // An object attached to another and wholly inside it, such as a number in its box, sits there on purpose.
                continue;
            }
            if let Some((width, height)) = overlap(a, b) {
                found.push(format!(
                    "\"{first}\" and \"{second}\" overlap by {width} x {height} pixels"
                ));
            } else if a.get("glyphs").is_some() && b.get("glyphs").is_some() {
                let least = LEAST_GAP * height;
                if let Some(apart) = gap(a, b)
                    && apart < least
                {
                    found.push(format!("\"{first}\" and \"{second}\" are only {apart:.0} pixels apart, and text needs {least:.0} to read as separate"));
                }
            }
        }
    }
    // A group's box is its glow's shell, which reaches well past its atoms.
    let glows: Vec<(String, &str, &Value)> =
        objects
            .iter()
            .flat_map(|(molecule, inspected)| {
                inspected["groups"].as_object().into_iter().flatten().map(
                    move |(group_id, group)| (format!("{molecule}/{group_id}"), *molecule, group),
                )
            })
            .filter(|(_, _, group)| number(&group["glow"]) > 0.0)
            .collect();
    for (glow_id, _, group) in &glows {
        for (object_id, inspected) in &ids {
            if inspected.get("glyphs").is_some()
                && !allows(allowed, glow_id, object_id)
                && let Some((width, height)) = overlap(group, inspected)
            {
                found.push(format!("the glow of \"{glow_id}\" and \"{object_id}\" overlap by {width} x {height} pixels"));
            }
        }
    }
    for (glow_id, molecule, group) in &glows {
        let glow_box = pixel_box(group);
        for (other_id, other) in &objects {
            let covered: Vec<Value> = other["atoms"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|atom| {
                    other_id != molecule
                        && !truthy(Some(&atom["hidden"]))
                        && holds(
                            glow_box,
                            [number(&atom["pixels"][0]), number(&atom["pixels"][1])],
                        )
                })
                .map(|atom| atom["number"].clone())
                .collect();
            if !covered.is_empty() {
                found.push(format!(
                    "the glow of \"{glow_id}\" covers atoms {} of \"{other_id}\"",
                    as_written(&Value::Array(covered))
                ));
            }
        }
    }
    for (i, (first_id, first_molecule, first)) in glows.iter().enumerate() {
        for (second_id, second_molecule, second) in &glows[i + 1..] {
            if first_molecule != second_molecule
                && let Some((width, height)) = overlap(first, second)
            {
                found.push(format!("the glows of \"{first_id}\" and \"{second_id}\" merge by {width} x {height} pixels"));
            }
        }
    }
    found
}

fn holds(region: PixelBox, point: [f64; 2]) -> bool {
    (0..2).all(|axis| region.min[axis] <= point[axis] && point[axis] <= region.max[axis])
}

/// Whether a box enters a region by more than the tolerance on each axis, or, along an axis it is
/// too thin for that, lies wholly within it.
fn enters(pixels: PixelBox, region: PixelBox) -> bool {
    (0..2).all(|i| {
        pixels.max[i].min(region.max[i]) - pixels.min[i].max(region.min[i]) > TOLERANCE
            || (region.min[i] <= pixels.min[i] && pixels.max[i] <= region.max[i])
    })
}

/// Each shown object, other than the exempt, in a reserved region that holds at this scene time:
/// axes by their parts, a molecule by its shown atoms' balls, anything else by its box, and each
/// shown atom a reaction is moving, by its ball and named by its reactant.
fn reserved_faults(
    report: &Value,
    scene_time: f64,
    reserved: &[ReservedRegion],
    exempt: &HashSet<&str>,
) -> Vec<String> {
    let mut found = Vec::new();
    for holding in reserved.iter().filter(|region| region.holds_at(scene_time)) {
        let place = holding.to_string();
        let region = &holding.pixels;
        for (number_of, reaction) in report["reactions"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            if reaction["phase"] != "during" {
                continue;
            }
            let mut moving: Vec<(String, Vec<i64>)> = Vec::new();
            for atom in reaction["atoms"].as_array().into_iter().flatten() {
                let reactant = as_written(&atom["reactant"]["object"]);
                if !exempt.contains(reactant.as_str()) && ball_reaches(atom, *region) {
                    let atom_number = atom["reactant"]["atom"].as_i64().unwrap_or_default();
                    match moving.iter_mut().find(|(id, _)| *id == reactant) {
                        Some((_, atoms)) => atoms.push(atom_number),
                        None => moving.push((reactant, vec![atom_number])),
                    }
                }
            }
            moving.sort_by(|a, b| a.0.cmp(&b.0));
            for (object_id, mut atoms) in moving {
                atoms.sort();
                found.push(format!(
                    "in reaction {number_of}, \"{object_id}\" has atoms {} in {place}",
                    number_list(&atoms)
                ));
            }
        }
        let mut objects = shown(report);
        objects.sort_by(|a, b| a.0.cmp(b.0));
        for (object_id, inspected) in objects {
            if exempt.contains(object_id) || inspected.get("cell_of").is_some() {
                continue;
            }
            let pixels = pixel_box(inspected);
            if inspected.get("atoms").is_some() {
                let atoms: Vec<Value> = inspected["atoms"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|atom| ball_reaches(atom, *region))
                    .map(|atom| atom["number"].clone())
                    .collect();
                if !atoms.is_empty() {
                    found.push(format!(
                        "\"{object_id}\" has atoms {} in {place}",
                        as_written(&Value::Array(atoms))
                    ));
                }
            } else if inspected.get("pattern").is_some() {
                let inside_region: Vec<&Value> = shown_copies(inspected)
                    .into_iter()
                    .filter(|copy| {
                        truthy(copy["bounding_box"].get("pixels"))
                            && enters(PixelBox::of(&copy["bounding_box"]["pixels"]), *region)
                    })
                    .collect();
                if !inside_region.is_empty() {
                    found.push(format!(
                        "\"{object_id}\" has copies {} in {place}",
                        copy_list(&inside_region)
                    ));
                }
            } else if ruling(inspected).is_some() {
                let mut hit: Vec<String> = Vec::new();
                for (name, part) in axis_parts(inspected) {
                    if enters(part, *region) && !hit.contains(&name) {
                        hit.push(name);
                    }
                }
                if !hit.is_empty() {
                    found.push(format!(
                        "\"{object_id}\" has its {} in {place}",
                        hit.join(", ")
                    ));
                }
            } else if enters(pixels, *region) {
                found.push(format!(
                    "\"{object_id}\" is in {place}: its box runs from {} to {} pixels",
                    pixels_of(&pixels.min),
                    pixels_of(&pixels.max)
                ));
            }
        }
    }
    found
}

/// The stage at one scene time in words: each shown object, top of the frame first, keyed by
/// object id.
fn describe(report: &Value, file: &Value) -> Vec<(String, String)> {
    let declared_objects = &file["objects"];
    let mut objects = shown(report);
    objects.sort_by(|a, b| {
        pixel_box(a.1).min[1]
            .partial_cmp(&pixel_box(b.1).min[1])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut lines = Vec::new();
    for (object_id, inspected) in objects {
        if inspected.get("glow_of").is_some() {
            continue;
        }
        let declared = &declared_objects[object_id];
        let object_type = declared
            .get("type")
            .map_or("object".to_string(), as_written);
        let mut words = ["text", "tex", "expression", "file"]
            .into_iter()
            .map(|key| declared.get(key))
            .find(|words| truthy(*words))
            .flatten()
            .cloned();
        let curve = &declared["curve"];
        if let (None, Some(points)) = (&words, curve.get("points").and_then(Value::as_array)) {
            let point = |at: Option<&Value>| at.map_or("?".to_string(), Value::to_string);
            words = Some(Value::from(format!(
                "through {} points, from {} to {}",
                points.len(),
                point(points.first()),
                point(points.last())
            )));
        } else if let (None, Some(trace)) = (&words, curve.get("trace")) {
            let field = |key: &str| trace.get(key).map_or("?".to_string(), as_written);
            words = Some(Value::from(format!(
                "through the {} atoms of {}{}",
                field("atom_name"),
                field("file"),
                trace.get("chain").map_or(String::new(), |chain| format!(
                    ", chain {}",
                    as_written(chain)
                ))
            )));
        } else if words.is_none() && curve.is_object() {
            let u_range = curve
                .get("u_range")
                .cloned()
                .unwrap_or(serde_json::json!(["?", "?"]));
            let field = |key: &str| curve.get(key).map_or("None".to_string(), as_written);
            words = Some(Value::from(format!(
                "x = {}, y = {}, z = {}, for u from {} to {}",
                field("x"),
                field("y"),
                field("z"),
                as_written(&u_range[0]),
                as_written(&u_range[1])
            )));
        }
        if object_type == "surface" {
            let axes = declared
                .get("axes")
                .and_then(Value::as_str)
                .map_or(&Value::Null, |axes| &declared_objects[axes]);
            let range = |own: &str, axis: &str| {
                declared
                    .get(own)
                    .filter(|range| truthy(Some(range)))
                    .or_else(|| axes[axis].get("range"))
                    .cloned()
                    .unwrap_or(serde_json::json!(["?", "?"]))
            };
            let (x_range, y_range) = (range("x_range", "x"), range("y_range", "y"));
            let mut surface = format!(
                "z = {}, for x from {} to {} and y from {} to {}",
                declared.get("z").map_or("None".to_string(), as_written),
                as_written(&x_range[0]),
                as_written(&x_range[1]),
                as_written(&y_range[0]),
                as_written(&y_range[1])
            );
            if let Some(axes) = declared.get("axes") {
                surface += &format!(" on \"{}\"", as_written(axes));
            }
            words = Some(Value::from(surface));
        }
        if inspected.get("pattern").is_some() {
            let layout = declared["layout"]
                .get("type")
                .map_or("layout".to_string(), as_written);
            let shape = declared["shape"]
                .get("type")
                .map_or("a shape".to_string(), as_written);
            words = Some(Value::from(format!(
                "{} copies of {shape} in a {layout}",
                as_written(&inspected["pattern"]["copy_count"])
            )));
        }
        if let Some(ruled) = ruling(inspected) {
            let ticks = ruled["ticks"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default();
            let axes: Vec<String> = ["x", "y", "z"]
                .into_iter()
                .filter(|axis| ticks.iter().any(|tick| tick["axis"] == *axis))
                .map(|axis| {
                    let labels: Vec<String> = ticks
                        .iter()
                        .filter(|tick| tick["axis"] == axis && tick.get("label_text").is_some())
                        .map(|tick| as_written(&tick["label_text"]))
                        .collect();
                    format!("{axis} tick labels {}", labels.join(", "))
                })
                .collect();
            words = Some(Value::from(axes.join("; ")));
        }
        let table = &inspected["table"];
        if table.is_object() {
            words = Some(Value::from(format!(
                "{} rows of {} cells",
                as_written(&table["rows"]),
                as_written(&table["columns"])
            )));
        }
        let mut line = format!("  {object_id} ({object_type})");
        if let Some(words) = words.filter(|words| truthy(Some(words))) {
            line += &format!(": {}", json_text(&words, false));
        }
        let mut notes = Vec::new();
        let opacity = number(&inspected["opacity"]);
        if opacity < 1.0 {
            notes.push(format!("opacity {opacity:.2}"));
        }
        let created = number(&inspected["created"]);
        if created < 1.0 {
            notes.push(format!("{:.0}% drawn", created * 100.0));
        }
        let lit: Vec<String> = inspected["groups"]
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(_, group)| number(&group["glow"]) > 0.0)
            .map(|(group_id, _)| group_id.clone())
            .chain(
                table["cells"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|cell| number(&cell["glow"]) > 0.0)
                    .map(|cell| format!("cell {}", json_text(&cell["cell"], true))),
            )
            .collect();
        if !lit.is_empty() {
            notes.push(format!("glowing {}", lit.join(", ")));
        }
        if truthy(inspected.get("morph")) {
            notes.push("morphing".to_string());
        }
        let pixels = pixel_box(inspected);
        notes.push(format!(
            "pixels {} to {}",
            pixels_of(&pixels.min),
            pixels_of(&pixels.max)
        ));
        lines.push((
            object_id.to_string(),
            format!("{line} [{}]", notes.join("; ")),
        ));
    }
    for (number_of, reaction) in report["reactions"]
        .as_array()
        .into_iter()
        .flatten()
        .enumerate()
    {
        if reaction["phase"] == "during" {
            lines.push((
                format!("reaction {number_of}"),
                format!("  reaction {number_of}: atoms moving, bonds breaking and forming"),
            ));
        }
    }
    lines
}

/// The lines of one stage as changes from the stage before: + shows now, - no longer shows, ~ changed.
fn changes(before: Option<&Vec<(String, String)>>, after: &[(String, String)]) -> Vec<String> {
    let Some(before) = before else {
        let lines: Vec<String> = after.iter().map(|(_, line)| line.clone()).collect();
        return if lines.is_empty() {
            vec!["  nothing shows".to_string()]
        } else {
            lines
        };
    };
    let find = |lines: &[(String, String)], shown_id: &str| {
        lines
            .iter()
            .find(|(id, _)| id == shown_id)
            .map(|(_, line)| line.clone())
    };
    let rest = |line: &str| line.chars().skip(1).collect::<String>();
    let mut lines: Vec<String> = after
        .iter()
        .filter(|(shown_id, _)| find(before, shown_id).is_none())
        .map(|(_, line)| format!("+{}", rest(line)))
        .collect();
    lines.extend(
        after
            .iter()
            .filter(|(shown_id, line)| find(before, shown_id).is_some_and(|old| old != *line))
            .map(|(_, line)| format!("~{}", rest(line))),
    );
    lines.extend(
        before
            .iter()
            .filter(|(shown_id, _)| find(after, shown_id).is_none())
            .map(|(_, line)| format!("-{}", rest(line))),
    );
    let unchanged = after
        .iter()
        .filter(|(shown_id, line)| find(before, shown_id).as_ref() == Some(line))
        .count();
    if unchanged > 0 {
        lines.push(format!("  ({unchanged} more, unchanged)"));
    }
    if lines.is_empty() {
        vec!["  nothing shows".to_string()]
    } else {
        lines
    }
}

/// The scene times a finding was found at, in words, in a scene `duration` seconds long.
fn when(times: &[CheckedTime], duration: f64) -> String {
    let mut times = times.to_vec();
    times.sort_by(|a, b| {
        a.scene_time
            .partial_cmp(&b.scene_time)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let (first, last) = (&times[0], &times[times.len() - 1]);
    let written = |at: &CheckedTime| written_scene_time(at.scene_time, duration);
    if times.len() == 1 {
        return format!("t={} ({})", written(first), first.wording);
    }
    format!(
        "t={} to {} ({} to {}, {} checks)",
        written(first),
        written(last),
        first.wording,
        last.wording,
        times.len()
    )
}

/// Draws the frame at each checked scene time on the sheet, and tiles them in pages of a few
/// rows, so a sheet stays readable when it is shown shrunk.
fn contact_sheets(
    scene: &Scene,
    sheet: &[&CheckedTime],
    out: &Path,
    reserved: &[ReservedRegion],
    frame_width: f64,
    gpu: &GpuFlag,
) -> Result<Vec<PathBuf>, Vec<ToolError>> {
    // Drawn on the full-size frame before it shrinks to a tile, so its edges are this wide on the tile.
    let edge_width =
        ((RESERVED_EDGE_PIXELS * frame_width / TILE_WIDTH).round_ties_even() as i64).max(1);
    // Each region's edges on the page's tiles inside its span, which ffmpeg numbers from 0.
    let boxes = |page: &[&CheckedTime]| -> String {
        let mut boxes = String::new();
        for region in reserved {
            let held: Vec<String> = (0..page.len())
                .filter(|&tile| region.holds_at(page[tile].scene_time))
                .map(|tile| format!("eq(n\\,{tile})"))
                .collect();
            if held.is_empty() {
                continue;
            }
            let pixels = region.pixels;
            boxes += &format!(
                "drawbox=x={:.0}:y={:.0}:w={:.0}:h={:.0}:color={RESERVED_COLOR}:t={edge_width}:enable={},",
                pixels.min[0],
                pixels.min[1],
                pixels.max[0] - pixels.min[0],
                pixels.max[1] - pixels.min[1],
                held.join("+")
            );
        }
        boxes
    };
    if !sheet.is_empty() {
        let gpu = Gpu::new(scene.scene_config.background, gpu.gpu)?;
        for (number_of, at) in sheet.iter().enumerate() {
            let frame = crate::frame(&gpu, scene, scene_time(scene, at.scene_time)?)?;
            let png = out.join(format!("frame-{number_of:03}.png"));
            frame.write_png(&png).map_err(|e| {
                vec![ToolError::Io {
                    file: png.clone(),
                    message: e.to_string(),
                }]
            })?;
        }
    }
    let mut sheets = Vec::new();
    for (page, first) in (0..sheet.len()).step_by(PAGE_TILES).enumerate() {
        let count = PAGE_TILES.min(sheet.len() - first);
        let path = out.join(format!("contact-sheet-{}.png", page + 1));
        let filter = format!(
            "{}scale={TILE_WIDTH}:-1,tile={COLUMNS}x{}:padding=6:color=white",
            boxes(&sheet[first..first + count]),
            count.div_ceil(COLUMNS)
        );
        let ran = Command::new("ffmpeg")
            .args([
                "-loglevel",
                "error",
                "-y",
                "-framerate",
                "1",
                "-start_number",
                &first.to_string(),
                "-i",
            ])
            .arg(out.join("frame-%03d.png"))
            .args(["-vf", &filter, "-frames:v", "1"])
            .arg(&path)
            .output();
        match ran {
            Err(e) => {
                let message = format!(
                    "ffmpeg could not be started, so no contact sheet was drawn; install ffmpeg and put it on PATH ({e})"
                );
                return Err(vec![ToolError::Ffmpeg {
                    message,
                    stderr: String::new(),
                }]);
            }
            Ok(ran) if !ran.status.success() => {
                let message = format!("ffmpeg failed drawing {}", path.display());
                return Err(vec![ToolError::Ffmpeg {
                    message,
                    stderr: String::from_utf8_lossy(&ran.stderr).into_owned(),
                }]);
            }
            Ok(_) => sheets.push(path),
        }
    }
    Ok(sheets)
}

/// A fault with its pixel coordinates and overlap sizes taken out, so a fault that moves with a
/// followed object reads as one.
fn without_pixels(fault: &str) -> String {
    let mut kept = String::new();
    let mut rest = fault;
    'scan: while let Some(c) = rest.chars().next() {
        if c == '[' {
            let inner =
                rest[1..].trim_start_matches(|c: char| c.is_ascii_digit() || "-., ".contains(c));
            if inner.len() < rest.len() - 1 && inner.starts_with(']') {
                kept.push('#');
                rest = &inner[1..];
                continue 'scan;
            }
        } else if c.is_ascii_digit() {
            let after = rest.trim_start_matches(|c: char| c.is_ascii_digit());
            if let Some(after) = after.strip_prefix(" x ") {
                let tail = after.trim_start_matches(|c: char| c.is_ascii_digit());
                if tail.len() < after.len() {
                    kept.push('#');
                    rest = tail;
                    continue 'scan;
                }
            }
        }
        kept.push(c);
        rest = &rest[c.len_utf8()..];
    }
    kept
}

/// The number a report or scene file value holds, or NaN where it holds none.
fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(f64::NAN)
}

/// Whether a value counts as given: not missing, null, false, 0, or empty.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

/// Neumaier's compensated sum, so a sum of many small terms keeps its last digits.
fn sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let (mut total, mut compensation) = (0.0_f64, 0.0_f64);
    for value in values {
        let next = total + value;
        compensation += if total.abs() >= value.abs() {
            (total - next) + value
        } else {
            (value - next) + total
        };
        total = next;
    }
    if compensation != 0.0 && compensation.is_finite() {
        total + compensation
    } else {
        total
    }
}

/// A pixel point as `[x, y]` in whole pixels.
fn pixels_of(point: &[f64]) -> String {
    format!(
        "[{}]",
        point
            .iter()
            .map(|value| format!("{value:.0}"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn number_list(values: &[i64]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// A number with six significant digits and no trailing zeros, in exponent form when it is very
/// large or small: 1.5, 0.1, 37.1333, 1e-05.
fn general(value: f64) -> String {
    significant(value, 6)
}

/// A scene time as the layout check writes it: six significant digits, or as many more as keep
/// it inside a scene `duration` seconds long, so `inspect` and `frame` take it back. The end of a
/// scene 51.995999 seconds long is 51.995999, not 51.996.
fn written_scene_time(scene_time: f64, duration: f64) -> String {
    (6..=17)
        .map(|digits| significant(scene_time, digits))
        .find(|written| written.parse::<f64>().is_ok_and(|read| read <= duration))
        .unwrap_or_else(|| shortest(scene_time))
}

/// A number with `digits` significant digits and no trailing zeros, in exponent form when it is
/// very large or small.
fn significant(value: f64, digits: usize) -> String {
    if !value.is_finite() {
        return if value.is_nan() {
            "nan".to_string()
        } else if value > 0.0 {
            "inf".to_string()
        } else {
            "-inf".to_string()
        };
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.to_string();
    }
    let scientific = format!("{value:.*e}", digits - 1);
    let (mantissa, exponent) = scientific.split_once('e').expect("exponent form");
    let exponent: i32 = exponent.parse().expect("an exponent");
    let trim = |written: &str| {
        if written.contains('.') {
            written
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        } else {
            written.to_string()
        }
    };
    let digits = digits as i32;
    if (-4..digits).contains(&exponent) {
        trim(&format!("{value:.*}", (digits - 1 - exponent) as usize))
    } else {
        format!(
            "{}e{}{:02}",
            trim(mantissa),
            if exponent < 0 { '-' } else { '+' },
            exponent.abs()
        )
    }
}

/// The shortest decimal that reads back as the same number, with `.0` on a whole number and in
/// exponent form past 1e16 or below 1e-4.
fn shortest(value: f64) -> String {
    if !value.is_finite() {
        return if value.is_nan() {
            "nan".to_string()
        } else if value > 0.0 {
            "inf".to_string()
        } else {
            "-inf".to_string()
        };
    }
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0"
        } else {
            "0.0"
        }
        .to_string();
    }
    let scientific = format!("{value:e}");
    let (mantissa, exponent) = scientific.split_once('e').expect("exponent form");
    let exponent: i32 = exponent.parse().expect("an exponent");
    let (sign, mantissa) = mantissa
        .strip_prefix('-')
        .map_or(("", mantissa), |m| ("-", m));
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let point = exponent + 1;
    if point <= -4 || point > 16 {
        let fraction = if digits.len() > 1 {
            format!(".{}", &digits[1..])
        } else {
            String::new()
        };
        return format!(
            "{sign}{}{fraction}e{}{:02}",
            &digits[..1],
            if exponent < 0 { '-' } else { '+' },
            exponent.abs()
        );
    }
    let unsigned = if point <= 0 {
        format!("0.{}{digits}", "0".repeat((-point) as usize))
    } else if point as usize >= digits.len() {
        format!("{digits}{}.0", "0".repeat(point as usize - digits.len()))
    } else {
        format!(
            "{}.{}",
            &digits[..point as usize],
            &digits[point as usize..]
        )
    };
    format!("{sign}{unsigned}")
}

/// A value in the report's wording: a string bare, and any other value as `literal` writes it.
fn as_written(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => literal(other),
    }
}

fn literal(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        Value::Number(n) if n.is_f64() => shortest(n.as_f64().expect("a float")),
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            let quote = if s.contains('\'') && !s.contains('"') {
                '"'
            } else {
                '\''
            };
            let escaped = s
                .replace('\\', "\\\\")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            let escaped = if quote == '\'' {
                escaped.replace('\'', "\\'")
            } else {
                escaped
            };
            format!("{quote}{escaped}{quote}")
        }
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(literal).collect::<Vec<_>>().join(", ")
        ),
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{}: {}", literal(&Value::from(k.as_str())), literal(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// A value as JSON with a space after each comma and colon, and with every character past ASCII
/// escaped when `ascii` is set.
fn json_text(value: &Value, ascii: bool) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) if n.is_f64() => {
            let value = n.as_f64().expect("a float");
            if value.is_nan() {
                "NaN".to_string()
            } else if value.is_infinite() {
                if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string()
            } else {
                shortest(value)
            }
        }
        Value::Number(n) => n.to_string(),
        Value::String(s) => {
            let mut quoted = String::from("\"");
            for c in s.chars() {
                match c {
                    '"' => quoted += "\\\"",
                    '\\' => quoted += "\\\\",
                    '\n' => quoted += "\\n",
                    '\r' => quoted += "\\r",
                    '\t' => quoted += "\\t",
                    '\u{8}' => quoted += "\\b",
                    '\u{c}' => quoted += "\\f",
                    c if (c as u32) < 0x20 || (ascii && (c as u32) > 0x7e) => {
                        let mut units = [0u16; 2];
                        for unit in c.encode_utf16(&mut units) {
                            quoted += &format!("\\u{unit:04x}");
                        }
                    }
                    c => quoted.push(c),
                }
            }
            quoted + "\""
        }
        Value::Array(items) => format!(
            "[{}]",
            items
                .iter()
                .map(|item| json_text(item, ascii))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Object(map) => {
            format!(
                "{{{}}}",
                map.iter()
                    .map(|(k, v)| format!(
                        "{}: {}",
                        json_text(&Value::from(k.as_str()), ascii),
                        json_text(v, ascii)
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_are_written_as_the_report_words_them() {
        let cases = [
            0.0,
            1.0,
            1.5,
            0.1,
            37.13333333,
            123456.7,
            1234567.0,
            0.0001,
            0.00001,
            -2.5,
            1e16,
            1e15,
        ];
        let written: Vec<(String, String)> = cases
            .iter()
            .map(|value| (general(*value), shortest(*value)))
            .collect();
        assert_eq!(
            written,
            [
                ("0", "0.0"),
                ("1", "1.0"),
                ("1.5", "1.5"),
                ("0.1", "0.1"),
                ("37.1333", "37.13333333"),
                ("123457", "123456.7"),
                ("1.23457e+06", "1234567.0"),
                ("0.0001", "0.0001"),
                ("1e-05", "1e-05"),
                ("-2.5", "-2.5"),
                ("1e+16", "1e+16"),
                ("1e+15", "1000000000000000.0"),
            ]
            .map(|(g, s)| (g.to_string(), s.to_string()))
        );
    }

    #[test]
    fn a_scene_time_is_written_with_more_digits_only_where_six_would_round_past_the_scene() {
        let cases = [
            (51.995999, 51.995999),
            (51.9959996, 52.0),
            (25.9979995, 51.995999),
            (0.1 + 0.7, 0.1 + 0.7),
            (0.0, 0.8),
            (1.0 / 3.0, 1.0),
        ];
        let written: Vec<String> = cases
            .iter()
            .map(|(scene_time, duration)| written_scene_time(*scene_time, *duration))
            .collect();
        assert_eq!(
            written,
            [
                "51.995999",
                "51.996",
                "25.998",
                "0.7999999999999999",
                "0",
                "0.333333"
            ]
        );
    }

    #[test]
    fn a_fault_a_follow_carries_across_frames_reads_as_one() {
        assert_eq!(
            without_pixels(
                r#""a" and "b" overlap by 12 x 30 pixels; box [10, -2.5] to [3, 4]; [x] 7 x"#
            ),
            r#""a" and "b" overlap by # pixels; box # to #; [x] 7 x"#
        );
    }

    #[test]
    fn text_is_quoted_as_json_with_spaces_after_commas() {
        let value = serde_json::json!([1, "é\"", 2.0]);
        assert_eq!(json_text(&value, true), r#"[1, "\u00e9\"", 2.0]"#);
        assert_eq!(json_text(&value, false), r#"[1, "é\"", 2.0]"#);
    }
}
