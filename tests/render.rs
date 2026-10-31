//! Runs `sci_anim render` and reads the MP4 back with ffprobe, the way an agent checks a video.

mod common;

use std::path::Path;
use std::process::Command;

use common::{scene_file, sci_anim, without_gpu};
use serde_json::{Value, json};

/// 1.5 seconds at 20 frames per second, at an odd width that the video pads to an even one.
const SCENE: &str = r##"{
  "scene_config": { "width": 161, "height": 90, "fps": 20 },
  "objects": { "orb": { "type": "sphere", "radius": 1, "opacity": 0 } },
  "timeline": [
    { "type": "play", "duration": 1, "animations": [{ "type": "fade", "object": "orb", "to": 1 }] },
    { "type": "wait", "duration": 0.5 }
  ]
}"##;

fn probe(mp4: &Path) -> Value {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-count_frames",
            "-select_streams",
            "v:0",
            "-of",
            "json",
        ])
        .args([
            "-show_entries",
            "stream=codec_name,width,height,r_frame_rate,nb_read_frames,pix_fmt:format=duration",
        ])
        .arg(mp4)
        .output()
        .expect("ffprobe runs");
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn render_writes_one_frame_per_frame_time_and_reports_it() {
    let (dir, path) = scene_file(SCENE);
    let mp4 = dir.path().join("out.mp4");

    let run = sci_anim(&[
        "render",
        path.to_str().unwrap(),
        "-o",
        mp4.to_str().unwrap(),
    ]);

    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    assert_eq!(
        without_gpu(&run.json),
        json!({ "status": "ok", "command": "render", "mp4": mp4, "width": 162, "height": 90, "fps": 20, "frames": 30, "duration": 1.5 })
    );
    let probed = probe(&mp4);
    assert_eq!(
        probed["streams"][0],
        json!({ "codec_name": "h264", "width": 162, "height": 90, "pix_fmt": "yuv420p", "r_frame_rate": "20/1", "nb_read_frames": "30" })
    );
    assert_eq!(probed["format"]["duration"], "1.500000");
    assert!(!dir.path().join(".out.mp4.partial").exists());
    assert_eq!(run.stderr, "");
}

/// Each line of stderr as JSON.
fn render_updates(stderr: &str) -> Vec<Value> {
    stderr
        .lines()
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|e| panic!("a render update is one JSON line: {e}\n{line}"))
        })
        .collect()
}

#[test]
fn render_with_updates_prints_the_same_report_and_its_updates_on_stderr() {
    let (dir, path) = scene_file(SCENE);
    let mp4 = dir.path().join("out.mp4");

    let run = sci_anim(&[
        "render",
        path.to_str().unwrap(),
        "-o",
        mp4.to_str().unwrap(),
        "--updates",
    ]);

    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    assert_eq!(
        without_gpu(&run.json),
        json!({ "status": "ok", "command": "render", "mp4": mp4, "width": 162, "height": 90, "fps": 20, "frames": 30, "duration": 1.5 })
    );
    let updates = render_updates(&run.stderr);
    let (drawing, rest) = updates.split_at(updates.len() - 2);
    assert_eq!(
        drawing[0],
        json!({ "update": "drawing", "frames_drawn": 0, "frames": 30 })
    );
    assert_eq!(
        drawing.last().unwrap(),
        &json!({ "update": "drawing", "frames_drawn": 30, "frames": 30 })
    );
    assert!(
        drawing
            .iter()
            .all(|update| update["update"] == "drawing" && update["frames"] == 30),
        "{drawing:?}"
    );
    assert!(
        drawing
            .windows(2)
            .all(|pair| pair[0]["frames_drawn"].as_u64() <= pair[1]["frames_drawn"].as_u64()),
        "{drawing:?}"
    );
    assert_eq!(
        rest,
        [
            json!({ "update": "finishing", "frames_drawn": 30, "frames": 30 }),
            json!({ "update": "finished", "frames_drawn": 30, "frames": 30 })
        ]
    );
}

#[test]
fn render_with_updates_prints_no_finished_update_when_ffmpeg_fails() {
    let (dir, path) = scene_file(SCENE);
    let mp4 = dir.path().join("out.mp4");
    let ffmpeg = dir.path().join("ffmpeg");
    std::fs::write(&ffmpeg, "#!/bin/sh\ncat > /dev/null\nexit 1\n").unwrap();
    std::fs::set_permissions(&ffmpeg, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let search_path = format!(
        "{}:{}",
        dir.path().display(),
        std::env::var("PATH").unwrap()
    );

    let out = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .args([
            "render",
            path.to_str().unwrap(),
            "-o",
            mp4.to_str().unwrap(),
            "--updates",
        ])
        .env("PATH", search_path)
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    let updates = render_updates(&String::from_utf8(out.stderr).unwrap());

    assert_eq!(out.status.code(), Some(2));
    assert_eq!(report["errors"][0]["kind"], "ffmpeg");
    assert_eq!(
        report["errors"][0]["message"],
        "ffmpeg exited with exit status: 1"
    );
    assert_eq!(
        updates.last().unwrap(),
        &json!({ "update": "finishing", "frames_drawn": 30, "frames": 30 })
    );
    assert!(
        updates.iter().all(|update| update["update"] != "finished"),
        "{updates:?}"
    );
    assert!(!mp4.exists());
}

#[test]
fn render_stops_with_an_ffmpeg_error_when_ffmpeg_stops_reading_frames() {
    let (dir, path) = scene_file(SCENE);
    let mp4 = dir.path().join("out.mp4");
    let ffmpeg = dir.path().join("ffmpeg");
    let two_frames = 161 * 90 * 4 * 2;
    std::fs::write(
        &ffmpeg,
        format!("#!/bin/sh\nhead -c {two_frames} > /dev/null\necho gave up >&2\nexit 1\n"),
    )
    .unwrap();
    std::fs::set_permissions(&ffmpeg, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let search_path = format!(
        "{}:{}",
        dir.path().display(),
        std::env::var("PATH").unwrap()
    );

    let out = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .args([
            "render",
            path.to_str().unwrap(),
            "-o",
            mp4.to_str().unwrap(),
        ])
        .env("PATH", search_path)
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();

    assert_eq!(out.status.code(), Some(2));
    assert_eq!(report["errors"][0]["kind"], "ffmpeg");
    assert_eq!(report["errors"][0]["stderr"], "gave up");
    assert!(
        report["errors"][0]["message"]
            .as_str()
            .unwrap()
            .starts_with("ffmpeg stopped reading frames after "),
        "{report}"
    );
    assert!(!mp4.exists());
    assert!(!dir.path().join(".out.mp4.partial").exists());
}

/// A datagram socket keeps each write(2) as its own message, so a line split across two writes,
/// which another process writing to the same pipe could land between, shows as two messages.
#[test]
fn render_writes_each_render_update_whole_in_one_write() {
    let (dir, path) = scene_file(SCENE);
    let mp4 = dir.path().join("out.mp4");
    let (ours, theirs) = std::os::unix::net::UnixDatagram::pair().unwrap();

    let exit = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .args([
            "render",
            path.to_str().unwrap(),
            "-o",
            mp4.to_str().unwrap(),
            "--updates",
        ])
        .stdout(std::process::Stdio::null())
        .stderr(std::os::fd::OwnedFd::from(theirs))
        .status()
        .unwrap();
    ours.set_nonblocking(true).unwrap();
    let mut writes = Vec::new();
    let mut buffer = [0; 4096];
    while let Ok(length) = ours.recv(&mut buffer) {
        writes.push(String::from_utf8(buffer[..length].to_vec()).unwrap());
    }

    assert_eq!(exit.code(), Some(0));
    assert_eq!(
        writes.first().map(String::as_str),
        Some("{\"update\":\"drawing\",\"frames_drawn\":0,\"frames\":30}\n")
    );
    assert_eq!(
        writes.last().map(String::as_str),
        Some("{\"update\":\"finished\",\"frames_drawn\":30,\"frames\":30}\n")
    );
    assert!(
        writes
            .iter()
            .all(|write| write.ends_with("}\n") && write.matches('\n').count() == 1),
        "{writes:?}"
    );
}

#[test]
fn render_gives_the_same_bytes_twice_with_or_without_updates() {
    let (dir, path) = scene_file(SCENE);
    let (first, second) = (dir.path().join("a.mp4"), dir.path().join("b.mp4"));

    sci_anim(&[
        "render",
        path.to_str().unwrap(),
        "-o",
        first.to_str().unwrap(),
    ]);
    sci_anim(&[
        "render",
        path.to_str().unwrap(),
        "-o",
        second.to_str().unwrap(),
        "--updates",
    ]);

    assert_eq!(
        std::fs::read(&first).unwrap(),
        std::fs::read(&second).unwrap()
    );
}

#[test]
fn render_without_ffmpeg_is_an_ffmpeg_error_and_writes_nothing() {
    let (dir, path) = scene_file(SCENE);
    let mp4 = dir.path().join("out.mp4");

    let out = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .args([
            "render",
            path.to_str().unwrap(),
            "-o",
            mp4.to_str().unwrap(),
        ])
        .env("PATH", dir.path())
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();

    assert_eq!(out.status.code(), Some(2));
    assert_eq!(report["errors"][0]["kind"], "ffmpeg");
    assert_eq!(report["errors"][0]["stderr"], "");
    assert!(
        report["errors"][0]["message"]
            .as_str()
            .unwrap()
            .starts_with("ffmpeg could not be started")
    );
    assert!(!mp4.exists());
    assert!(!dir.path().join(".out.mp4.partial").exists());
}

#[test]
fn render_into_a_missing_directory_is_an_io_error_naming_the_video() {
    let (_dir, path) = scene_file(SCENE);

    let run = sci_anim(&[
        "render",
        path.to_str().unwrap(),
        "-o",
        "/nonexistent/out.mp4",
    ]);

    assert_eq!(run.exit_code, 2, "{}", run.stdout);
    assert_eq!(
        run.json,
        json!({ "status": "error", "errors": [
            { "kind": "io", "file": "/nonexistent/out.mp4", "message": "No such file or directory (os error 2)" }
        ] })
    );
}

#[test]
fn a_render_of_a_bad_scene_file_is_a_scene_error() {
    let (dir, path) = scene_file(
        r##"{ "objects": {}, "timeline": [{ "type": "play", "duration": 1, "animations": [{ "type": "fade", "object": "orb", "to": 1 }] }] }"##,
    );
    let mp4 = dir.path().join("out.mp4");

    let run = sci_anim(&[
        "render",
        path.to_str().unwrap(),
        "-o",
        mp4.to_str().unwrap(),
    ]);

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        common::scene_errors(&run),
        common::pairs(&[("/timeline/0/animations/0/object", "unknown_object")])
    );
    assert!(!mp4.exists());
}
