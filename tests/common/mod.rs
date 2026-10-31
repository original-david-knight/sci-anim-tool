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

/// Runs the built binary the way an agent does. Panics unless stdout is exactly one JSON value.
pub fn sci_anim(args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .args(args)
        .output()
        .expect("the binary runs");
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
