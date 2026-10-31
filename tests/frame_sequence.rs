mod common;

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

use common::{read_rgba, scene_file, sci_anim, without_gpu};
use serde_json::{Value, json};

fn scene(timeline: Value) -> Value {
    json!({
        "scene_config": { "width": 81, "height": 61, "fps": 4, "background": "transparent" },
        "objects": { "disc": { "type": "circle", "radius": 1, "opacity": 0, "fill": { "color": "#268cb5" } } },
        "timeline": timeline
    })
}

fn files(directory: &Path) -> Vec<String> {
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    files.sort();
    files
}

#[test]
fn every_numbered_frame_matches_its_scene_time_including_empty_waits_and_chapter_holds() {
    let scene = scene(json!([
        { "type": "wait", "duration": 0.5 },
        { "type": "chapter", "id": "appear", "min_duration": 1.5, "steps": [
            { "type": "play", "duration": 0.5, "animations": [{ "type": "fade", "object": "disc", "to": 1, "easing_curve": "linear" }] }
        ] },
        { "type": "play", "duration": 0.5, "animations": [{ "type": "fade", "object": "disc", "to": 0, "easing_curve": "linear" }] },
        { "type": "wait", "duration": 0.5 }
    ]));
    let (dir, path) = scene_file(&scene.to_string());
    let directory = dir.path().join("frames");
    let run = sci_anim(&[
        "frames",
        path.to_str().unwrap(),
        "-o",
        directory.to_str().unwrap(),
        "--updates",
    ]);
    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    assert_eq!(
        without_gpu(&run.json),
        json!({
            "status": "ok", "command": "frames", "directory": directory, "pattern": "frame-%06d.png", "start_number": 0,
            "width": 81, "height": 61, "fps": 4, "frames": 12, "duration": 3.0, "alpha": "straight", "color_space": "srgb"
        })
    );
    assert_eq!(
        files(&directory),
        (0..12)
            .map(|index| format!("frame-{index:06}.png"))
            .collect::<Vec<_>>()
    );
    let render_updates: Vec<Value> = run
        .stderr
        .lines()
        .map(|text| serde_json::from_str(text).unwrap())
        .collect();
    assert_eq!(
        render_updates.first().unwrap(),
        &json!({ "update": "drawing", "frames_drawn": 0, "frames": 12 })
    );
    assert_eq!(
        &render_updates[render_updates.len() - 3..],
        &[
            json!({ "update": "drawing", "frames_drawn": 12, "frames": 12 }),
            json!({ "update": "finishing", "frames_drawn": 12, "frames": 12 }),
            json!({ "update": "finished", "frames_drawn": 12, "frames": 12 })
        ]
    );
    for index in 0..12 {
        let frame_time = (f64::from(index) / 4.0).to_string();
        let png = dir.path().join("single.png");
        let run = sci_anim(&[
            "frame",
            path.to_str().unwrap(),
            "--scene-time",
            &frame_time,
            "-o",
            png.to_str().unwrap(),
        ]);
        assert_eq!(run.exit_code, 0, "{}", run.stdout);
        let numbered = directory.join(format!("frame-{index:06}.png"));
        assert_eq!(
            std::fs::read(&numbered).unwrap(),
            std::fs::read(&png).unwrap(),
            "frame {index} at {frame_time}"
        );
        let rgba = read_rgba(&numbered);
        if [0, 1, 2, 10, 11].contains(&index) {
            assert!(
                rgba.iter().all(|byte| *byte == 0),
                "empty frame {index} is present"
            );
        }
        if [4, 5, 6, 7, 8].contains(&index) {
            assert_eq!(rgba[(30 * 81 + 40) * 4 + 3], 255, "held frame {index}");
        }
    }
}

#[test]
fn an_empty_timeline_has_one_frame_and_fractional_duration_rounds_to_the_nearest_frame() {
    for (timeline, count, duration) in [
        (json!([]), 1, 0.25),
        (json!([{ "type": "wait", "duration": 0.36 }]), 1, 0.25),
        (json!([{ "type": "wait", "duration": 0.38 }]), 2, 0.5),
    ] {
        let (dir, path) = scene_file(&scene(timeline).to_string());
        let directory = dir.path().join("frames");
        let run = sci_anim(&[
            "frames",
            path.to_str().unwrap(),
            "-o",
            directory.to_str().unwrap(),
        ]);
        assert_eq!(run.exit_code, 0, "{}", run.stdout);
        assert_eq!(run.json["frames"], count);
        assert_eq!(run.json["duration"], duration);
        assert_eq!(files(&directory).len(), count as usize);
        assert_eq!(run.stderr, "");
        assert_eq!(
            read_rgba(&directory.join("frame-000000.png")).len(),
            81 * 61 * 4
        );
    }
}

#[test]
fn frames_need_no_ffmpeg_and_ignore_validated_sound_content() {
    let mut scene = scene(
        json!([{ "type": "chapter", "id": "voice", "min_duration": 0.25, "steps": [{ "type": "wait", "duration": 0.25 }] }]),
    );
    scene["sounds"] = json!([{ "file": "voice.mp3", "at": "voice" }]);
    let (dir, path) = scene_file(&scene.to_string());
    std::fs::write(
        dir.path().join("voice.mp3"),
        b"readable file, not encoded sound",
    )
    .unwrap();
    let directory = dir.path().join("frames");
    let out = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .args([
            "frames",
            path.to_str().unwrap(),
            "-o",
            directory.to_str().unwrap(),
        ])
        .env("PATH", dir.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(files(&directory), ["frame-000000.png"]);
    std::fs::remove_file(dir.path().join("voice.mp3")).unwrap();
    let missing = dir.path().join("missing-sound");
    let run = sci_anim(&[
        "frames",
        path.to_str().unwrap(),
        "-o",
        missing.to_str().unwrap(),
    ]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    assert_eq!(run.json["errors"][0]["code"], "unreadable_file");
    assert!(!missing.exists());
}

#[test]
fn frames_accept_opaque_backgrounds_and_keep_exact_odd_dimensions() {
    let mut scene = scene(json!([]));
    scene["scene_config"]["background"] = json!("#123456");
    let (dir, path) = scene_file(&scene.to_string());
    let directory = dir.path().join("frames");
    let run = sci_anim(&[
        "frames",
        path.to_str().unwrap(),
        "-o",
        directory.to_str().unwrap(),
    ]);
    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    assert_eq!(run.json["width"], 81);
    assert_eq!(run.json["height"], 61);
    let rgba = read_rgba(&directory.join("frame-000000.png"));
    assert_eq!(rgba.len(), 81 * 61 * 4);
    assert!(
        rgba.chunks_exact(4)
            .all(|pixel| pixel == [0x12, 0x34, 0x56, 255])
    );
}

#[test]
fn any_existing_destination_is_refused_without_changing_it() {
    let (dir, path) = scene_file(&scene(json!([])).to_string());
    let existing_file = dir.path().join("file");
    let existing_directory = dir.path().join("directory");
    let existing_symlink = dir.path().join("link");
    std::fs::write(&existing_file, b"keep").unwrap();
    std::fs::create_dir(&existing_directory).unwrap();
    std::fs::write(existing_directory.join("keep"), b"keep").unwrap();
    std::os::unix::fs::symlink(dir.path().join("absent"), &existing_symlink).unwrap();
    for destination in [&existing_file, &existing_directory, &existing_symlink] {
        let run = sci_anim(&[
            "frames",
            path.to_str().unwrap(),
            "-o",
            destination.to_str().unwrap(),
        ]);
        assert_eq!(run.exit_code, 2, "{}", run.stdout);
        assert_eq!(run.json["errors"][0]["kind"], "io");
        assert_eq!(run.json["errors"][0]["file"], json!(destination));
        assert_eq!(run.stderr, "");
    }
    assert_eq!(std::fs::read(&existing_file).unwrap(), b"keep");
    assert_eq!(files(&existing_directory), ["keep"]);
    assert!(
        existing_symlink
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
    let missing_parent = dir.path().join("absent").join("frames");
    let run = sci_anim(&[
        "frames",
        path.to_str().unwrap(),
        "-o",
        missing_parent.to_str().unwrap(),
    ]);
    assert_eq!(run.exit_code, 2);
    assert!(!missing_parent.parent().unwrap().exists());
}

#[test]
fn png_write_failure_cleans_owned_files_and_never_reports_finished() {
    let (dir, path) = scene_file(&scene(json!([])).to_string());
    let directory = dir.path().join("frames");
    let out = Command::new("/bin/bash")
        .args([
            "-c",
            "ulimit -f 0\ntrap '' XFSZ\nexec \"$1\" frames \"$2\" -o \"$3\" --updates",
            "--",
        ])
        .arg(env!("CARGO_BIN_EXE_sci_anim"))
        .arg(&path)
        .arg(&directory)
        .env("MESA_SHADER_CACHE_DISABLE", "true")
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(2), "{report}");
    assert_eq!(report["errors"][0]["kind"], "io");
    assert_eq!(
        report["errors"][0]["file"],
        json!(directory.join("frame-000000.png"))
    );
    assert!(
        !directory.exists(),
        "the partial PNG, incomplete marker and empty directory are removed"
    );
    let render_updates = String::from_utf8(out.stderr).unwrap();
    assert!(
        render_updates.contains("\"update\":\"drawing\""),
        "{render_updates}"
    );
    assert!(
        !render_updates.contains("\"update\":\"finished\""),
        "{render_updates}"
    );
}

#[test]
fn an_abrupt_stop_leaves_an_incomplete_directory_that_cannot_be_reused() {
    let (dir, path) = scene_file(&scene(json!([{ "type": "wait", "duration": 1000 }])).to_string());
    let directory = dir.path().join("frames");
    let mut child = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .args([
            "frames",
            path.to_str().unwrap(),
            "-o",
            directory.to_str().unwrap(),
            "--updates",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut first_update = String::new();
    BufReader::new(child.stderr.take().unwrap())
        .read_line(&mut first_update)
        .unwrap();
    assert!(
        first_update.contains("\"frames_drawn\":0"),
        "{first_update}"
    );
    assert!(directory.join(".incomplete").exists());
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(directory.join(".incomplete").exists());
    let run = sci_anim(&[
        "frames",
        path.to_str().unwrap(),
        "-o",
        directory.to_str().unwrap(),
    ]);
    assert_eq!(run.exit_code, 2);
    assert_eq!(run.json["errors"][0]["kind"], "io");
    assert!(directory.join(".incomplete").exists());
}

#[test]
fn transparent_mp4_is_rejected_before_gpu_ffmpeg_or_destination_writes() {
    let (dir, path) = scene_file(&scene(json!([])).to_string());
    let mp4 = dir.path().join("out.mp4");
    std::fs::write(&mp4, b"earlier video").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .args([
            "render",
            path.to_str().unwrap(),
            "-o",
            mp4.to_str().unwrap(),
            "--updates",
        ])
        .env("PATH", dir.path())
        .env("WGPU_BACKEND", "noop")
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(report["errors"][0]["kind"], "usage");
    assert!(
        report["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("sci_anim frames")
    );
    assert_eq!(std::fs::read(mp4).unwrap(), b"earlier video");
    assert!(!dir.path().join(".out.mp4.partial").exists());
    assert!(out.stderr.is_empty());
}
