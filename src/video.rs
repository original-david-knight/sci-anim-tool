//! The one owner of the ffmpeg process, and the only module that runs a program.
//!
//! `Encoder` writes to a partial file beside the video and renames it over the video only when
//! ffmpeg has finished cleanly. A render that fails or is killed leaves any earlier video at
//! that path as it was, and a dropped `Encoder` kills ffmpeg and deletes its partial file.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::thread::JoinHandle;

use crate::frame::Frame;
use crate::scene::{Background, Color, SceneConfig, Sound};

/// What a render wrote, for the report.
#[derive(Debug)]
pub struct Video {
    pub frames: u32,
    /// The video's size, which is the frame's size rounded up to even numbers, because the
    /// pixel format every player reads needs even sizes. The added row or column is the
    /// background color.
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    /// `frames / fps`, in seconds.
    pub duration: f64,
}

#[derive(Debug)]
pub enum VideoError {
    /// No program named `ffmpeg` could be started.
    Missing(io::Error),
    /// ffmpeg stopped with an error, or stopped reading frames. `stderr` is what it printed.
    Failed { message: String, stderr: String },
    /// The video's directory cannot take the partial file, or moving the finished partial file
    /// onto the video's path failed. `file` is the video's path.
    Io { file: PathBuf, source: io::Error },
}

pub struct Encoder {
    ffmpeg: Child,
    stdin: Option<ChildStdin>,
    stderr: Option<JoinHandle<String>>,
    partial: PathBuf,
    mp4: PathBuf,
    frames: u32,
    scene_config: SceneConfig,
}

impl Encoder {
    /// `frames` is how many frames the video will hold, which is where its sounds are cut.
    pub fn start(
        scene_config: &SceneConfig,
        sounds: &[Sound],
        frames: u32,
        mp4: &Path,
    ) -> Result<Encoder, VideoError> {
        let partial = partial_path(mp4);
        // Created here rather than by ffmpeg, so a missing or read-only directory is reported
        // against the video's path, not ffmpeg's message about a hidden file.
        std::fs::File::create(&partial).map_err(|source| VideoError::Io {
            file: mp4.to_path_buf(),
            source,
        })?;
        let ffmpeg = Command::new("ffmpeg")
            .args(ffmpeg_args(scene_config, sounds, frames, &partial))
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn();
        let mut ffmpeg = ffmpeg.map_err(|e| {
            let _ = std::fs::remove_file(&partial);
            VideoError::Missing(e)
        })?;
        let stdin = ffmpeg.stdin.take();
        let mut stderr = ffmpeg.stderr.take().expect("stderr is piped");
        // Read in its own thread so ffmpeg never blocks on a full stderr pipe while we block on
        // its full stdin.
        let stderr = std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text);
            text
        });
        Ok(Encoder {
            ffmpeg,
            stdin,
            stderr: Some(stderr),
            partial,
            mp4: mp4.to_path_buf(),
            frames: 0,
            scene_config: *scene_config,
        })
    }

    /// Frames must come in scene time order, one per `Scene::frame_scene_times`.
    pub fn push(&mut self, frame: &Frame) -> Result<(), VideoError> {
        let stdin = self.stdin.as_mut().expect("stdin is open until finish");
        match stdin.write_all(&frame.rgba) {
            Ok(()) => {
                self.frames += 1;
                Ok(())
            }
            Err(e) => Err(self.failure(format!(
                "ffmpeg stopped reading frames after {} of them: {e}",
                self.frames
            ))),
        }
    }

    pub fn finish(mut self) -> Result<Video, VideoError> {
        drop(self.stdin.take());
        let status = self
            .ffmpeg
            .wait()
            .map_err(|e| self.failure(format!("waiting for ffmpeg failed: {e}")))?;
        if !status.success() {
            return Err(self.failure(format!("ffmpeg exited with {status}")));
        }
        std::fs::rename(&self.partial, &self.mp4).map_err(|source| VideoError::Io {
            file: self.mp4.clone(),
            source,
        })?;
        let even = |n: u32| n.next_multiple_of(2);
        let fps = self.scene_config.fps.get();
        Ok(Video {
            frames: self.frames,
            width: even(self.scene_config.width.get()),
            height: even(self.scene_config.height.get()),
            fps,
            duration: f64::from(self.frames) / f64::from(fps),
        })
    }

    fn failure(&mut self, message: String) -> VideoError {
        drop(self.stdin.take());
        let _ = self.ffmpeg.wait();
        let stderr = self
            .stderr
            .take()
            .and_then(|thread| thread.join().ok())
            .unwrap_or_default();
        VideoError::Failed {
            message,
            stderr: stderr.trim_end().to_string(),
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        let _ = self.ffmpeg.kill();
        let _ = self.ffmpeg.wait();
        let _ = std::fs::remove_file(&self.partial);
    }
}

/// `out.mp4` becomes `.out.mp4.partial` in the same directory, so the rename cannot cross
/// file systems.
fn partial_path(mp4: &Path) -> PathBuf {
    let mut file_name = OsString::from(".");
    file_name.push(mp4.file_name().unwrap_or_default());
    file_name.push(".partial");
    mp4.with_file_name(file_name)
}

/// Raw RGBA frames in on stdin, H.264 in an MP4 out, with each sound delayed to its start,
/// mixed at full volume and cut at the video's end as 48 kHz stereo AAC. A scene with no sounds gives a video
/// with no sound stream. `-f mp4` because the partial file's name does not end in `.mp4`.
/// `bitexact` keeps ffmpeg's version out of the file, so one scene gives the same bytes on every
/// run with the same ffmpeg.
fn ffmpeg_args(
    scene_config: &SceneConfig,
    sounds: &[Sound],
    frames: u32,
    partial: &Path,
) -> Vec<OsString> {
    let size = format!("{}x{}", scene_config.width.get(), scene_config.height.get());
    let fps = scene_config.fps.get().to_string();
    let Background::Opaque(Color { r, g, b }) = scene_config.background else {
        unreachable!("render rejects transparent backgrounds before starting ffmpeg")
    };
    let pad = format!("[0:v]pad=ceil(iw/2)*2:ceil(ih/2)*2:color=0x{r:02x}{g:02x}{b:02x}[video]");
    let mut args: Vec<OsString> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-s",
        &size,
        "-r",
        &fps,
        "-i",
        "-",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    let mut filters = vec![pad];
    if !sounds.is_empty() {
        for sound in sounds {
            // `file:` so a name such as `take:1.wav` is not read as a protocol.
            let mut file = OsString::from("file:");
            file.push(std::path::absolute(&sound.path).unwrap_or_else(|_| sound.path.clone()));
            args.extend([OsString::from("-i"), file]);
        }
        // Every sound in one format first, so the mix does not take whichever the first sound has.
        let delayed: Vec<String> = sounds
            .iter()
            .enumerate()
            .map(|(i, sound)| format!("[{}:a]aformat=sample_rates=48000:channel_layouts=stereo,adelay={}:all=1[sound{i}]", i + 1, (sound.start * 1000.0).round()))
            .collect();
        let streams: String = (0..sounds.len()).map(|i| format!("[sound{i}]")).collect();
        let end = f64::from(frames) / f64::from(scene_config.fps.get());
        filters.extend(delayed);
        filters.push(format!(
            "{streams}amix=inputs={}:normalize=0:duration=longest,atrim=end={end}[sound]",
            sounds.len()
        ));
    }
    args.extend([
        "-filter_complex".into(),
        filters.join(";").into(),
        "-map".into(),
        "[video]".into(),
    ]);
    args.extend(
        [
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-crf",
            "18",
            "-flags:v",
            "+bitexact",
        ]
        .map(OsString::from),
    );
    if !sounds.is_empty() {
        args.extend(
            [
                "-map",
                "[sound]",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "-flags:a",
                "+bitexact",
            ]
            .map(OsString::from),
        );
    }
    args.extend(
        [
            "-fflags",
            "+bitexact",
            "-map_metadata",
            "-1",
            "-movflags",
            "+faststart",
            "-f",
            "mp4",
        ]
        .map(OsString::from),
    );
    args.push(partial.as_os_str().to_owned());
    args
}
