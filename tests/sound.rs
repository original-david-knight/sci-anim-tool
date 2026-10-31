//! Sounds in a scene file: checked by validate, mixed under the video by render, and read back
//! with ffprobe and ffmpeg as an agent checks a video.

mod common;

use std::path::Path;
use std::process::Command;

use common::{pairs, scene_errors, sci_anim};
use serde_json::json;

/// 2 seconds at 20 frames per second: a wait, then a chapter the tone starts at.
const SCENE: &str = r##"{
  "scene_config": { "width": 160, "height": 90, "fps": 20 },
  "objects": { "orb": { "type": "sphere", "radius": 1 } },
  "timeline": [
    { "type": "wait", "duration": 0.75 },
    { "type": "chapter", "id": "pivot", "min_duration": 1.25, "steps": [{ "type": "wait", "duration": 0.5 }] }
  ],
  "sounds": [{ "file": "sounds/tone.wav", "at": "pivot" }]
}"##;

/// A scene file in a fresh temp dir beside a `sounds/tone.wav` of `seconds` of a 440 Hz tone.
fn scene_with_tone(scene: &str, seconds: f64) -> (tempfile::TempDir, std::path::PathBuf) {
    let (dir, path) = common::scene_file(scene);
    std::fs::create_dir(dir.path().join("sounds")).unwrap();
    let tone = format!("sine=frequency=440:sample_rate=48000:duration={seconds}");
    let made = Command::new("ffmpeg")
        .args(["-v", "error", "-f", "lavfi", "-i", &tone])
        .arg(dir.path().join("sounds/tone.wav"))
        .status()
        .expect("ffmpeg runs");
    assert!(made.success());
    (dir, path)
}

/// When the sound first rises above silence, and how long the MP4's sound stream is, in seconds.
fn sound_start_and_length(mp4: &Path) -> (f64, f64) {
    let detect = Command::new("ffmpeg")
        .arg("-i")
        .arg(mp4)
        .args(["-af", "silencedetect=noise=-40dB:d=0.1", "-f", "null", "-"])
        .output()
        .expect("ffmpeg runs");
    let log = String::from_utf8_lossy(&detect.stderr);
    let start = log
        .split("silence_end: ")
        .nth(1)
        .and_then(|rest| rest.split_whitespace().next())
        .expect("a silence that ends")
        .parse()
        .unwrap();
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=codec_name,duration",
            "-of",
            "json",
        ])
        .arg(mp4)
        .output()
        .expect("ffprobe runs");
    let probed: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
    assert_eq!(probed["streams"][0]["codec_name"], "aac");
    (
        start,
        probed["streams"][0]["duration"]
            .as_str()
            .unwrap()
            .parse()
            .unwrap(),
    )
}

#[test]
fn validate_reports_each_sound_at_its_chapters_start() {
    let (_dir, path) = scene_with_tone(SCENE, 0.5);

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        run.json["sounds"],
        json!([{ "file": "sounds/tone.wav", "at": "pivot", "start": 0.75 }])
    );
}

#[test]
fn render_places_a_sound_at_its_chapters_start_and_cuts_it_at_the_videos_end() {
    let (dir, path) = scene_with_tone(SCENE, 5.0);
    let mp4 = dir.path().join("out.mp4");

    let run = sci_anim(&[
        "render",
        path.to_str().unwrap(),
        "-o",
        mp4.to_str().unwrap(),
    ]);

    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    let (start, length) = sound_start_and_length(&mp4);
    assert!(
        (start - 0.75).abs() < 0.03,
        "the tone starts with the chapter at 0.75 s, not at {start}"
    );
    assert!(
        (length - 2.0).abs() < 0.03,
        "a 5 s tone is cut at the video's end, 2 s, not {length}"
    );
}

#[test]
fn a_missing_sound_file_and_an_unknown_chapter_are_errors_at_their_fields() {
    let scene = SCENE.replace(r#""sounds": [{ "file": "sounds/tone.wav", "at": "pivot" }]"#, r#""sounds": [{ "file": "sounds/tone.wav", "at": "pivot" }, { "file": "voice.mp3", "at": "outro" }]"#);
    let (_dir, path) = scene_with_tone(&scene, 0.5);

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/sounds/1/file", "unreadable_file"),
            ("/sounds/1/at", "unknown_chapter")
        ])
    );
}

#[test]
fn the_narrated_example_starts_each_voice_clip_at_its_chapter() {
    let example = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/narrated_swap.json");

    let run = sci_anim(&["validate", example]);

    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    let starts: Vec<(String, f64)> = run.json["sounds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["at"].as_str().unwrap().to_string(),
                s["start"].as_f64().unwrap(),
            )
        })
        .collect();
    let chapters: Vec<(String, f64)> = run.json["chapters"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["id"].as_str().unwrap().to_string(),
                c["start"].as_f64().unwrap(),
            )
        })
        .collect();
    assert_eq!(starts, chapters);
}

#[test]
fn render_mixes_every_sound_as_48_khz_stereo_and_keeps_a_colon_in_a_file_path() {
    let scene = SCENE.replace(r#""sounds": [{ "file": "sounds/tone.wav", "at": "pivot" }]"#, r#""sounds": [{ "file": "sounds/take:1.wav", "at": "pivot" }, { "file": "sounds/tone.wav", "at": "pivot" }]"#);
    let (dir, path) = scene_with_tone(&scene, 0.5);
    let mono = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=sample_rate=22050:duration=0.5",
            "-ac",
            "1",
        ])
        .arg(dir.path().join("sounds/take:1.wav"))
        .status()
        .expect("ffmpeg runs");
    assert!(mono.success());
    let mp4 = dir.path().join("out.mp4");

    let run = sci_anim(&[
        "render",
        path.to_str().unwrap(),
        "-o",
        mp4.to_str().unwrap(),
    ]);

    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_entries",
            "stream=sample_rate,channels",
            "-of",
            "json",
        ])
        .arg(&mp4)
        .output()
        .expect("ffprobe runs");
    let probed: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
    assert_eq!(
        probed["streams"][0],
        json!({ "sample_rate": "48000", "channels": 2 }),
        "a 22.05 kHz mono sound listed first does not set the format"
    );
}
