mod common;

use common::{pairs, scene_errors, scene_file, sci_anim};
use serde_json::json;

const GOOD: &str = r##"{
  "objects": {
    "disc": { "type": "circle", "radius": 2, "fill": { "color": "#1c758a" } },
    "orb": { "type": "sphere", "radius": 0.8, "position": [0.6, 0.4, 1] }
  }
}"##;

#[test]
fn validate_lists_every_error_at_once_at_its_field() {
    let (_dir, path) = scene_file(
        r##"{
  "scene_config": { "fps": 0, "background": "1e1e1e" },
  "objects": {
    "disc": { "type": "circle", "radius": 2, "radious": 3, "stroke": { "color": "#58c4dd", "width": -1 } },
    "orb": { "type": "sphear", "radius": 1 },
    "ball": { "type": "sphere", "material": { "type": "standard", "color": "red", "opacity": 1.5 } }
  }
}"##,
    );

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    assert_eq!(run.json["status"], "error");
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/ball/material/color", "bad_format"),
            ("/objects/ball/material/opacity", "out_of_range"),
            ("/objects/ball/radius", "missing_field"),
            ("/objects/disc/radious", "unknown_field"),
            ("/objects/disc/stroke/width", "out_of_range"),
            ("/objects/orb/type", "unknown_value"),
            ("/scene_config/background", "bad_format"),
            ("/scene_config/fps", "out_of_range"),
        ]),
        "{}",
        run.stdout
    );
    let messages: Vec<&str> = run.json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap())
        .collect();
    assert_eq!(messages[2], "missing field `radius`");
    assert_eq!(
        messages[3],
        "unknown field `radious`, expected one of `type`, `radius`, `stroke`, `fill`, `position`, `rotation`, `scale`, `opacity`, `attach`"
    );
    assert_eq!(
        messages[5],
        r#""sphear" is not one of "circle", "dot", "sphere", "prism", "revolution", "tube", "surface", "line", "arrow", "spring", "trail", "readout", "polygon", "arc", "path", "text", "math", "table", "axes", "3d-axes", "number-line", "function-plot", "point-plot", "bar-plot", "molecule", "pattern""#
    );
    assert!(
        run.json["errors"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["kind"] == "scene")
    );
}

#[test]
fn validate_reports_an_object_with_no_type_once() {
    let (_dir, path) = scene_file(
        r##"{ "objects": { "x": { "radius": 1, "stroke": { "color": "#ffffff" } } } }"##,
    );

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/x/type", "missing_field")]),
        "{}",
        run.stdout
    );
}

#[test]
fn validate_reports_a_camera_that_looks_at_its_own_position() {
    let (_dir, path) = scene_file(
        r##"{ "camera": { "position": [1, 2, 3], "target": [1, 2, 3] }, "objects": {} }"##,
    );

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1);
    assert_eq!(scene_errors(&run), pairs(&[("/camera/target", "invalid")]));
}

#[test]
fn validate_reports_broken_json_at_the_root() {
    let (_dir, path) = scene_file("{\n  \"objects\": {\n");

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1);
    assert_eq!(scene_errors(&run), pairs(&[("", "invalid_json")]));
    assert_eq!(
        run.json["errors"][0]["message"],
        "EOF while parsing an object at line 3 column 0"
    );
}

#[test]
fn validate_accepts_a_good_scene() {
    let (_dir, path) = scene_file(GOOD);

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    assert_eq!(
        (run.json["status"].as_str(), run.json["command"].as_str()),
        (Some("ok"), Some("validate"))
    );
}

#[test]
fn validate_reports_a_missing_scene_file_as_an_io_error() {
    let run = sci_anim(&["validate", "/nonexistent/scene.json"]);

    assert_eq!(run.exit_code, 2);
    assert_eq!(
        run.json,
        json!({ "status": "error", "errors": [
            { "kind": "io", "file": "/nonexistent/scene.json", "message": "No such file or directory (os error 2)" }
        ] })
    );
}

#[test]
fn an_unknown_command_is_a_usage_error_in_json() {
    let run = sci_anim(&["valdate", "scene.json"]);

    assert_eq!(run.exit_code, 2);
    assert_eq!(run.json["status"], "error");
    assert_eq!(run.json["errors"][0]["kind"], "usage");
}

#[test]
fn validate_reports_each_step_error_at_its_field() {
    let (_dir, path) = scene_file(
        r##"{
  "objects": { "orb": { "type": "sphere", "radius": 1 } },
  "timeline": [
    {},
    { "type": "wait" },
    { "type": "wait", "duration": 1, "animations": [] },
    { "type": "play", "duration": 1, "animatons": [] },
    { "type": "play", "duration": 0, "animations": [
      { "type": "spin", "object": "orb" },
      { "type": "move", "object": "orb", "to": [1, 2], "easing_curve": "smooth" },
      { "type": "camera-zoom", "to": 200 },
      { "type": "camera-move", "to": [0, 0, 5], "object": "orb" },
      { "type": "camera-look" }
    ] },
    { "type": "pause", "duration": 1 }
  ]
}"##,
    );

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/0/type", "missing_field"),
            ("/timeline/1/duration", "missing_field"),
            ("/timeline/2/animations", "unknown_field"),
            ("/timeline/3/animations", "missing_field"),
            ("/timeline/3/animatons", "unknown_field"),
            ("/timeline/4/animations/0/type", "unknown_value"),
            ("/timeline/4/animations/1/easing_curve", "unknown_value"),
            ("/timeline/4/animations/1/to", "wrong_length"),
            ("/timeline/4/animations/2/to", "out_of_range"),
            ("/timeline/4/animations/3/object", "unknown_field"),
            ("/timeline/4/animations/4/to", "missing_field"),
            ("/timeline/4/duration", "out_of_range"),
            ("/timeline/5/type", "unknown_value"),
        ]),
        "{}",
        run.stdout
    );
    let message = |i: usize| {
        run.json["errors"][i]["message"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(
        message(6),
        r#""smooth" is not one of "linear", "slow-start", "slow-end", "slow-both", "overshoot""#
    );
    assert_eq!(
        message(9),
        "unknown field `object`, expected one of `type`, `to`, `easing_curve`, `start`, `end`"
    );
}

#[test]
fn validate_lists_the_object_ids_an_animation_could_mean() {
    let (_dir, path) = scene_file(
        r##"{
  "objects": { "orb": { "type": "sphere", "radius": 1 }, "disc": { "type": "circle", "radius": 1 } },
  "timeline": [{ "type": "play", "duration": 1, "animations": [
    { "type": "fade", "object": "orbb", "to": 0 },
    { "type": "move", "object": "camera", "to": [1, 0, 0] }
  ] }]
}"##,
    );

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/0/animations/0/object", "unknown_object"),
            ("/timeline/0/animations/1/object", "unknown_object")
        ])
    );
    assert_eq!(
        run.json["errors"][0]["message"],
        "no object has the id `orbb`; the objects are `disc`, `orb`"
    );
    assert_eq!(
        run.json["errors"][1]["message"],
        "no object has the id `camera`; the objects are `disc`, `orb`; to change the camera, use an animation of type `camera-move`, `camera-look` or `camera-zoom`, or a `follow` whose `sets` is `camera-position` or `camera-target`"
    );
}

#[test]
fn validate_refuses_two_animations_of_one_value_in_one_step() {
    let (_dir, path) = scene_file(
        r##"{
  "objects": { "orb": { "type": "sphere", "radius": 1 } },
  "timeline": [
    { "type": "play", "duration": 1, "animations": [
      { "type": "move", "object": "orb", "to": [1, 0, 0] },
      { "type": "rotate", "object": "orb", "to": [0, 90, 0] },
      { "type": "camera-move", "to": [0, 1, 10] },
      { "type": "move", "object": "orb", "to": [2, 0, 0] },
      { "type": "camera-move", "to": [0, 2, 10] }
    ] },
    { "type": "play", "duration": 1, "animations": [{ "type": "move", "object": "orb", "to": [3, 0, 0] }] }
  ]
}"##,
    );

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/0/animations/3", "conflicting_animations"),
            ("/timeline/0/animations/4", "conflicting_animations"),
        ]),
        "{}",
        run.stdout
    );
    assert_eq!(
        run.json["errors"][0]["message"],
        "this animation and the one at /timeline/0/animations/0 both change the position of `orb` at once; give them spans one after the other with `start` and `end`, or put them in separate steps"
    );
}

#[test]
fn validate_reports_a_camera_moved_onto_its_target() {
    let (_dir, path) = scene_file(
        r##"{
  "objects": {},
  "timeline": [
    { "type": "play", "duration": 1, "animations": [{ "type": "camera-zoom", "to": 30 }] },
    { "type": "play", "duration": 1, "animations": [{ "type": "camera-zoom", "to": 45 }, { "type": "camera-move", "to": [0, 0, 0] }] }
  ]
}"##,
    );

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/1/animations/1", "invalid")]),
        "{}",
        run.stdout
    );
}

#[test]
fn validate_reports_a_camera_that_looks_nowhere_partway_through_a_step() {
    let scene = |target_to: &str| {
        format!(
            r##"{{
  "camera": {{ "position": [0, 0, 10], "target": [0, 0, 0] }},
  "objects": {{}},
  "timeline": [{{ "type": "play", "duration": 2, "animations": [
    {{ "type": "camera-move", "to": [0, 0, -10], "easing_curve": "slow-end" }},
    {{ "type": "camera-look", "to": {target_to}, "easing_curve": "linear" }}
  ] }}]
}}"##
        )
    };
    let (_a, crossing) = scene_file(&scene("[0, 0, -5]"));
    let (_b, passing_beside) = scene_file(&scene("[1, 0, -5]"));

    let crossing = sci_anim(&["validate", crossing.to_str().unwrap()]);
    let passing_beside = sci_anim(&["validate", passing_beside.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&crossing),
        pairs(&[("/timeline/0/animations/0", "invalid")]),
        "the camera looks along z at both ends of the step, and its position passes its target between them\n{}",
        crossing.stdout
    );
    assert_eq!(
        crossing.json["errors"][0]["message"],
        "during this step the camera's target reaches its position, so the camera looks nowhere"
    );
    assert_eq!(
        (
            passing_beside.json["status"].as_str(),
            passing_beside.json["command"].as_str()
        ),
        (Some("ok"), Some("validate")),
        "a target one unit aside is never reached"
    );
}

#[test]
fn validate_reports_a_camera_that_looks_along_its_up_partway_through_a_step() {
    let (_dir, path) = scene_file(
        r##"{
  "objects": {},
  "timeline": [{ "type": "play", "duration": 1, "animations": [
    { "type": "camera-move", "to": [0, 10, -10], "easing_curve": "linear" }
  ] }]
}"##,
    );

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/0/animations/0", "invalid")]),
        "straight above the target halfway\n{}",
        run.stdout
    );
    assert_eq!(
        run.json["errors"][0]["message"],
        "during this step the camera looks along its up, so up is undefined"
    );
}

/// A camera move over a short span of a long step still passes its target, and the check
/// samples the span, not the step.
#[test]
fn validate_reports_a_camera_that_looks_nowhere_in_a_short_span_of_a_long_step() {
    let scene = r##"{
  "camera": { "position": [0, 0, 10], "target": [0, 0, 0] },
  "objects": {},
  "timeline": [{ "type": "play", "duration": 1000, "animations": [
    { "type": "camera-move", "to": [0, 0, -10], "easing_curve": "linear", "start": 333.3, "end": 333.31 }
  ] }]
}"##;
    let (_dir, path) = scene_file(scene);

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/0/animations/0", "invalid")]),
        "{}",
        run.stdout
    );
}

#[test]
fn version_prints_the_commit_the_binary_was_built_from() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .arg("--version")
        .output()
        .unwrap();
    let printed = String::from_utf8(out.stdout).unwrap();

    let revision = printed
        .strip_prefix(concat!("sci_anim ", env!("CARGO_PKG_VERSION"), " "))
        .and_then(|rest| rest.strip_suffix('\n'))
        .unwrap_or_else(|| panic!("{printed:?}"));
    let commit = revision.strip_suffix(" modified").unwrap_or(revision);
    assert!(
        commit == "unknown"
            || (commit.len() == 40
                && commit
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())),
        "{printed:?}"
    );
    let checked_out = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .filter(|out| out.status.success());
    if let Some(checked_out) = checked_out {
        assert_eq!(commit, String::from_utf8_lossy(&checked_out.stdout).trim());
    }
}

#[test]
fn a_scene_lasts_its_step_durations_as_written_not_their_float_sum() {
    // These add, one after another as f64, to 37.099999999999994.
    let durations = [
        1.5, 1.5, 1.5, 1.0, 1.2, 2.5, 0.5, 1.5, 2.0, 3.0, 0.5, 1.2, 2.5, 2.0, 0.5, 3.0, 0.4, 3.0,
        0.8, 2.0, 1.5, 3.5,
    ];
    assert_eq!(durations.iter().sum::<f64>(), 37.099999999999994);
    let steps: Vec<String> = durations
        .iter()
        .map(|duration| format!(r#"{{ "type": "wait", "duration": {duration} }}"#))
        .collect();
    let (_dir, path) = scene_file(&format!(
        r##"{{ "objects": {{ "disc": {{ "type": "circle", "radius": 1 }} }}, "timeline": [{}] }}"##,
        steps.join(", ")
    ));
    let path = path.to_str().unwrap();

    let validated = sci_anim(&["validate", path]);
    let inspected = sci_anim(&["inspect", path, "--scene-time", "37.1"]);

    assert_eq!(
        (
            validated.json["duration"].clone(),
            validated.json["steps"][21]["end"].clone(),
            inspected.exit_code,
            inspected.json["scene_time"].clone()
        ),
        (json!(37.1), json!(37.1), 0, json!(37.1)),
        "{}",
        inspected.stdout
    );
}
