//! Runs `sci_anim frame` and reads the PNG it wrote, the way an agent would.
//!
//! The probe scene is 401 by 401 pixels with a 90 degree field of view, so pixel 200 sits on the
//! camera's axis, and a point `x` across at depth `d` from the camera lands `200.5 * x / d`
//! pixels from pixel 200. The camera is at z = 10. A red sphere of radius 2 sits at the origin,
//! and a disc of radius 4 lies in the plane z = 1.5, which cuts through the sphere.
//!
//! Along a row through the center, a ray `p` pixels out meets the sphere in front of that plane
//! for p < 31.2, and behind it for 31.2 < p < 40.9, the sphere's outline. The disc ends at
//! 200.5 * 4 / 8.5 = 94.4 pixels. A painter's algorithm, drawing whole objects far to near or
//! near to far, gets either p = 0 or p = 36 wrong.

mod common;

use std::path::{Path, PathBuf};

use common::{pairs, scene_errors, scene_file, sci_anim, without_gpu};
use serde_json::json;

const BACKGROUND: [u8; 4] = [0x1e, 0x1e, 0x1e, 255];
const FILL: [u8; 4] = [0x1c, 0x75, 0x8a, 255];

fn probe_scene(first: (&str, &str), second: (&str, &str)) -> String {
    format!(
        r##"{{
  "scene_config": {{ "width": 401, "height": 401, "background": "#1e1e1e" }},
  "camera": {{ "position": [0, 0, 10], "target": [0, 0, 0], "fov": 90 }},
  "objects": {{
    "{}": {},
    "{}": {}
  }}
}}"##,
        first.0, first.1, second.0, second.1
    )
}

const SPHERE: &str = r##"{ "type": "sphere", "radius": 2, "material": { "type": "standard", "color": "#ff0000" } }"##;
const DISC: &str = r##"{ "type": "circle", "radius": 4, "position": [0, 0, 1.5], "fill": { "color": "#1c758a" } }"##;

struct Pixels {
    width: u32,
    rgba: Vec<u8>,
}

impl Pixels {
    fn at(&self, x: u32, y: u32) -> [u8; 4] {
        let start = ((y * self.width + x) * 4) as usize;
        self.rgba[start..start + 4].try_into().unwrap()
    }
}

fn read_png(png: &Path) -> (u32, u32, Pixels) {
    let mut reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(png).unwrap()))
        .read_info()
        .unwrap();
    let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut rgba).unwrap();
    assert_eq!(
        (info.color_type, info.bit_depth),
        (png::ColorType::Rgba, png::BitDepth::Eight)
    );
    (
        info.width,
        info.height,
        Pixels {
            width: info.width,
            rgba,
        },
    )
}

/// Renders a scene at scene time 0 and returns the PNG's path, kept alive by the temp dir.
fn render(scene: &str) -> (tempfile::TempDir, PathBuf) {
    let (dir, path) = scene_file(scene);
    let png = dir.path().join("f.png");
    let run = sci_anim(&[
        "frame",
        path.to_str().unwrap(),
        "--scene-time",
        "0",
        "-o",
        png.to_str().unwrap(),
    ]);
    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    (dir, png)
}

fn is_red(pixel: [u8; 4]) -> bool {
    matches!(pixel, [r, 0, 0, 255] if r > 0)
}

#[test]
fn frame_reports_what_it_wrote() {
    let (dir, path) = scene_file(&probe_scene(("orb", SPHERE), ("disc", DISC)));
    let png = dir.path().join("f.png");

    let run = sci_anim(&[
        "frame",
        path.to_str().unwrap(),
        "--scene-time",
        "0",
        "-o",
        png.to_str().unwrap(),
    ]);

    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    assert_eq!(
        without_gpu(&run.json),
        json!({ "status": "ok", "command": "frame", "png": png, "width": 401, "height": 401, "scene_time": 0.0 })
    );
    assert_eq!(read_png(&png).0, 401);
}

#[test]
fn frame_shows_the_background_and_the_exact_fill_color() {
    let (_dir, png) = render(&probe_scene(("orb", SPHERE), ("disc", DISC)));
    let (width, height, pixels) = read_png(&png);

    assert_eq!((width, height), (401, 401));
    assert_eq!(pixels.at(0, 0), BACKGROUND);
    assert_eq!(pixels.at(400, 400), BACKGROUND);
    assert_eq!(
        pixels.at(270, 200),
        FILL,
        "70 pixels out the disc faces the camera, clear of the sphere"
    );
    assert_eq!(pixels.at(200, 130), FILL, "and 70 pixels up");
    assert_eq!(
        pixels.at(300, 200),
        BACKGROUND,
        "100 pixels out is past the disc's edge"
    );
}

#[test]
fn frame_draws_whichever_surface_is_nearest_at_each_pixel() {
    let (_dir, png) = render(&probe_scene(("orb", SPHERE), ("disc", DISC)));
    let (_, _, pixels) = read_png(&png);

    assert!(
        is_red(pixels.at(200, 200)),
        "the sphere is in front of the disc's plane on the axis: {:?}",
        pixels.at(200, 200)
    );
    for (x, y) in [(236, 200), (164, 200), (200, 236), (200, 164)] {
        assert_eq!(
            pixels.at(x, y),
            FILL,
            "36 pixels out the sphere is behind the disc's plane, at ({x}, {y})"
        );
    }
}

#[test]
fn frame_lights_the_sphere_from_the_cameras_upper_left() {
    let (_dir, png) = render(&probe_scene(("orb", SPHERE), ("disc", DISC)));
    let (_, _, pixels) = read_png(&png);

    let (upper_left, center, lower_right) = (
        pixels.at(184, 184),
        pixels.at(200, 200),
        pixels.at(216, 216),
    );
    assert_eq!(
        center,
        [213, 0, 0, 255],
        "the middle faces the camera, which the key light shades to two thirds in linear light"
    );
    assert!(
        is_red(upper_left) && is_red(lower_right),
        "{upper_left:?} {lower_right:?}"
    );
    assert!(
        upper_left[0] > center[0] && center[0] >= lower_right[0],
        "the shade steps in bands, so the lower right may share the middle's: upper left {upper_left:?}, center {center:?}, lower right {lower_right:?}"
    );
}

#[test]
fn frame_does_not_depend_on_object_order() {
    let sphere_first = render(&probe_scene(("orb", SPHERE), ("disc", DISC)));
    let disc_first = render(&probe_scene(("disc", DISC), ("orb", SPHERE)));
    let disc_drawn_first = render(&probe_scene(("z_orb", SPHERE), ("a_disc", DISC)));

    let bytes = std::fs::read(&sphere_first.1).unwrap();
    assert_eq!(
        bytes,
        std::fs::read(&disc_first.1).unwrap(),
        "swapping the objects in the file"
    );
    assert_eq!(
        bytes,
        std::fs::read(&disc_drawn_first.1).unwrap(),
        "renaming them so the disc is drawn first"
    );
}

#[test]
fn frame_draws_a_stroke_over_its_fill_on_a_tilted_circle() {
    let stroke_only = r##"{ "type": "circle", "radius": 3, "rotation": [63, 27, 11], "stroke": { "color": "#ff0000", "width": 0.6 } }"##;
    let with_fill = r##"{ "type": "circle", "radius": 3, "rotation": [63, 27, 11], "stroke": { "color": "#ff0000", "width": 0.6 }, "fill": { "color": "#00ff00" } }"##;
    let scene = |circle: &str| {
        format!(
            r##"{{ "scene_config": {{ "width": 400, "height": 400, "background": "#0000ff" }}, "objects": {{ "c": {circle} }} }}"##
        )
    };
    let (_a, alone) = render(&scene(stroke_only));
    let (_b, over_fill) = render(&scene(with_fill));
    let (_, _, alone) = read_png(&alone);
    let (_, _, over_fill) = read_png(&over_fill);

    let mut covered = 0;
    for y in 0..400 {
        for x in 0..400 {
            if is_red(alone.at(x, y)) {
                covered += 1;
                assert_eq!(
                    over_fill.at(x, y),
                    alone.at(x, y),
                    "the fill shows through the stroke at ({x}, {y})"
                );
            }
        }
    }
    assert!(covered > 5000, "the stroke covers {covered} pixels");
}

#[test]
fn frame_rejects_a_scene_time_outside_the_scene() {
    let (dir, path) = scene_file(&probe_scene(("orb", SPHERE), ("disc", DISC)));
    let png = dir.path().join("f.png");

    for scene_time in ["0.5", "-1"] {
        let run = sci_anim(&[
            "frame",
            path.to_str().unwrap(),
            "--scene-time",
            scene_time,
            "-o",
            png.to_str().unwrap(),
        ]);

        assert_eq!(run.exit_code, 1, "{}", run.stdout);
        assert_eq!(
            scene_errors(&run),
            pairs(&[("", "scene_time_out_of_range")])
        );
        assert_eq!(
            run.json["errors"][0]["message"],
            format!("scene time {scene_time} is outside the scene, which runs from 0 to 0 seconds")
        );
    }
    assert!(!png.exists());
}

#[test]
fn frame_reports_a_png_it_cannot_write_as_an_io_error() {
    let (_dir, path) = scene_file(&probe_scene(("orb", SPHERE), ("disc", DISC)));

    let run = sci_anim(&[
        "frame",
        path.to_str().unwrap(),
        "--scene-time",
        "0",
        "-o",
        "/nonexistent/f.png",
    ]);

    assert_eq!(run.exit_code, 2);
    assert_eq!(
        run.json["errors"],
        json!([{ "kind": "io", "file": "/nonexistent/f.png", "message": "No such file or directory (os error 2)" }])
    );
}

#[test]
fn the_example_scene_validates_and_renders() {
    let example = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sphere_and_circle.json"
    );
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("example.png");

    let validated = sci_anim(&["validate", example]);
    let framed = sci_anim(&[
        "frame",
        example,
        "--scene-time",
        "0",
        "-o",
        png.to_str().unwrap(),
    ]);

    assert_eq!(
        (
            validated.json["status"].as_str(),
            validated.json["command"].as_str()
        ),
        (Some("ok"), Some("validate"))
    );
    assert_eq!(framed.exit_code, 0, "{}", framed.stdout);
    let (width, height, pixels) = read_png(&png);
    assert_eq!((width, height), (1920, 1080));
    assert_eq!(pixels.at(0, 0), BACKGROUND);
}

#[test]
fn the_short_flag_is_another_way_to_write_scene_time() {
    let example = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/sphere_and_circle.json"
    );
    let dir = tempfile::tempdir().unwrap();
    let png = |name: &str| dir.path().join(name).to_str().unwrap().to_string();

    let long_run = sci_anim(&[
        "frame",
        example,
        "--scene-time",
        "0",
        "-o",
        &png("long.png"),
    ]);
    let short_run = sci_anim(&["frame", example, "-t", "0", "-o", &png("short.png")]);
    // The alias the skill taught before `-t`, kept for the scripts that use it.
    let alias_run = sci_anim(&["frame", example, "--t", "0", "-o", &png("alias.png")]);
    let negative = sci_anim(&["frame", example, "-t", "-1", "-o", &png("negative.png")]);

    assert_eq!(
        (long_run.exit_code, long_run.json["scene_time"].clone()),
        (0, json!(0.0)),
        "{}",
        long_run.stdout
    );
    assert_eq!(
        (short_run.exit_code, short_run.json["scene_time"].clone()),
        (0, json!(0.0)),
        "{}",
        short_run.stdout
    );
    assert_eq!(
        (alias_run.exit_code, alias_run.json["scene_time"].clone()),
        (0, json!(0.0)),
        "{}",
        alias_run.stdout
    );
    assert_eq!(
        std::fs::read(png("short.png")).unwrap(),
        std::fs::read(png("long.png")).unwrap()
    );
    assert_eq!(
        std::fs::read(png("alias.png")).unwrap(),
        std::fs::read(png("long.png")).unwrap()
    );
    assert_eq!(negative.exit_code, 1, "{}", negative.stdout);
    assert_eq!(
        scene_errors(&negative),
        pairs(&[("", "scene_time_out_of_range")])
    );
    assert!(!dir.path().join("negative.png").exists());
}

#[test]
fn the_timeline_example_validates_and_lasts_six_seconds() {
    let example = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/timeline.json");

    let validated = sci_anim(&["validate", example]);
    let end = sci_anim(&["inspect", example, "--scene-time", "6"]);
    let after = sci_anim(&["inspect", example, "--scene-time", "6.01"]);

    assert_eq!(
        (
            validated.json["status"].as_str(),
            validated.json["command"].as_str()
        ),
        (Some("ok"), Some("validate"))
    );
    assert_eq!(end.json["camera"]["position"], json!([5.0, 4.0, 8.0]));
    assert_eq!(after.json["errors"][0]["code"], "scene_time_out_of_range");
    assert_eq!(sci_anim(&["inspect", example, "-t", "6"]).json, end.json);
}

#[test]
fn every_example_scene_validates() {
    let folders = [
        concat!(env!("CARGO_MANIFEST_DIR"), "/examples"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/skills/sci-anim/examples"),
    ];
    let mut failed: Vec<String> = folders
        .iter()
        .flat_map(|folder| std::fs::read_dir(folder).unwrap())
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .filter(|path| sci_anim(&["validate", path.to_str().unwrap()]).exit_code != 0)
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    failed.sort();

    assert_eq!(failed, Vec::<String>::new());
}
