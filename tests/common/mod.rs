#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

pub struct Run {
    pub exit_code: i32,
    pub stdout: String,
    pub json: serde_json::Value,
    pub stderr: String,
}

/// Set for everything run under the machine's GPU lock, so a program started by one that holds it
/// does not wait on its own parent. Every caller on the machine uses this name, video builds and
/// the tests of other programs that draw included.
pub const GPU_HELD_ENV: &str = "GPU_LOCK_HELD";

/// The machine's GPU lock: `gpu.lock` in the user's cache folder. A machine draws on its GPU one
/// command at a time, since a video build sizes its frames to the GPU memory free when it starts,
/// and a test drawing beside it leaves it too little. A test holds the lock only for one command.
pub fn gpu_lock_file() -> PathBuf {
    let home = || PathBuf::from(std::env::var_os("HOME").expect("HOME is set"));
    if cfg!(target_os = "macos") {
        return home().join("Library/Caches/gpu.lock");
    }
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| home().join(".cache"));
    cache.join("gpu.lock")
}

/// The machine's GPU lock, held while it lives, or nothing when this test runs inside a holder.
pub struct GpuTurn(Option<std::fs::File>);

/// Waits for the machine's GPU lock, saying so on stderr, and holds it while the turn lives. Every
/// test that runs a command that draws (`frame`, `frames`, `render`, `check-layout`) takes it for
/// that command, and sets [`GPU_HELD_ENV`] on it.
pub fn gpu_turn() -> GpuTurn {
    gpu_turn_at(
        &gpu_lock_file(),
        std::env::var_os(GPU_HELD_ENV).is_some_and(|held| !held.is_empty()),
    )
}

/// Waits for the lock file `lock`, unless `held` says a parent holds it already.
pub fn gpu_turn_at(lock: &Path, held: bool) -> GpuTurn {
    if held {
        return GpuTurn(None);
    }
    if let Some(dir) = lock.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock)
        .unwrap_or_else(|e| panic!("open the GPU lock {}: {e}", lock.display()));
    match file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            eprintln!("waiting for the GPU: another program is drawing on this machine");
            file.lock().unwrap();
        }
        Err(std::fs::TryLockError::Error(e)) => panic!("lock the GPU {}: {e}", lock.display()),
    }
    GpuTurn(Some(file))
}

/// Whether `args` run a command that draws on the GPU.
fn draws(args: &[&str]) -> bool {
    matches!(
        args.first(),
        Some(&("frame" | "frames" | "render" | "check-layout"))
    )
}

/// Runs the built binary the way an agent does. Panics unless stdout is exactly one JSON value.
/// A command that draws holds the machine's GPU lock while it runs.
pub fn sci_anim(args: &[&str]) -> Run {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sci_anim"));
    let _turn = draws(args).then(|| {
        command.env(GPU_HELD_ENV, "1");
        gpu_turn()
    });
    let out = command.args(args).output().expect("the binary runs");
    let stdout = String::from_utf8(out.stdout).expect("utf-8 stdout");
    let json = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("stdout is one JSON value: {e}\n{stdout}"));
    let stderr = String::from_utf8(out.stderr).expect("utf-8 stderr");
    Run {
        exit_code: out.status.code().expect("an exit code"),
        stdout,
        json,
        stderr,
    }
}

/// The report of a command that draws, without the GPU it names, which differs between machines.
/// Panics unless it names one GPU of a kind `--gpu` takes.
pub fn without_gpu(report: &Value) -> Value {
    let mut report = report.clone();
    let gpu = report
        .as_object_mut()
        .and_then(|fields| fields.remove("gpu"))
        .expect("the report names its GPU");
    assert!(gpu["name"].is_string() && gpu["driver"].is_string() && gpu["backend"].is_string());
    assert!(
        ["discrete", "integrated", "other", "virtual", "cpu"]
            .contains(&gpu["kind"].as_str().unwrap()),
        "{gpu}"
    );
    report
}

/// Writes a scene file into a fresh temp dir, which lives as long as the returned guard.
pub fn scene_file(json: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("scene.json");
    std::fs::write(&path, json).unwrap();
    (dir, path)
}

pub fn scene_errors(run: &Run) -> Vec<(String, String)> {
    run.json["errors"]
        .as_array()
        .unwrap_or_else(|| panic!("a report with errors\n{}", run.stdout))
        .iter()
        .map(|e| {
            (
                e["path"].as_str().unwrap().to_string(),
                e["code"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

pub fn pairs(expected: &[(&str, &str)]) -> Vec<(String, String)> {
    expected
        .iter()
        .map(|(p, c)| (p.to_string(), c.to_string()))
        .collect()
}

/// A 400 by 400 scene on black of `objects` and the `timeline` steps.
pub fn scene(objects: &str, timeline: &str) -> String {
    format!(
        r##"{{
  "scene_config": {{ "width": 400, "height": 400, "background": "#000000" }},
  "objects": {{ {objects} }},
  "timeline": [ {timeline} ]
}}"##
    )
}

/// The frame at a scene time, as RGBA bytes, with the temp dir that holds its PNG.
pub fn frame_at(scene: &str, scene_time: &str) -> (tempfile::TempDir, Vec<u8>) {
    let (dir, path) = scene_file(scene);
    let png = dir.path().join("f.png");
    let run = sci_anim(&[
        "frame",
        path.to_str().unwrap(),
        "--scene-time",
        scene_time,
        "-o",
        png.to_str().unwrap(),
    ]);
    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    let rgba = read_rgba(&png);
    (dir, rgba)
}

/// The report of `inspect` at a scene time.
pub fn inspect_at(scene: &str, scene_time: &str) -> Value {
    let (_dir, path) = scene_file(scene);
    let run = sci_anim(&[
        "inspect",
        path.to_str().unwrap(),
        "--scene-time",
        scene_time,
    ]);
    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    run.json
}

pub fn read_rgba(png: &Path) -> Vec<u8> {
    let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(png).unwrap()))
        .read_info()
        .unwrap();
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut rgba).unwrap();
    rgba
}

/// One pixel of a 400 by 400 frame.
pub fn pixel(rgba: &[u8], x: usize, y: usize) -> [u8; 4] {
    let start = (y * 400 + x) * 4;
    rgba[start..start + 4].try_into().unwrap()
}

/// Each channel within 1 of the expected, for colors blended in linear light.
pub fn near(actual: [u8; 4], expected: [u8; 4]) -> bool {
    actual.iter().zip(expected).all(|(a, e)| a.abs_diff(e) <= 1)
}
