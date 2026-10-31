//! Shape laws: a plot, a tube, a surface or a pattern whose expressions read `t`, law time, run
//! by a follow with `sets: shape`, read back from reports as an agent reads them.

mod common;

use common::{inspect_at, pairs, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{value}"))
        .iter()
        .map(|n| (n.as_f64().unwrap() * 1000.0).round() / 1000.0 + 0.0)
        .collect()
}

fn errors(objects: &str, timeline: &str) -> (Vec<(String, String)>, Vec<String>) {
    let (_dir, path) = scene_file(&scene(objects, timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    let messages = run.json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap().to_string())
        .collect();
    (scene_errors(&run), messages)
}

const AXES: &str = r#""ax": { "type": "axes", "x": { "range": [0, 1] }, "y": { "range": [-5, 5] }, "width": 4, "height": 4 }"#;

/// A wait of 1 s, then a follow of `object`'s shape over 2 s, then a wait of 1 s.
fn followed(object: &str, extra: &str) -> String {
    format!(
        r#"{{ "type": "wait", "duration": 1 }},
        {{ "type": "play", "duration": 2, "animations": [ {{ "type": "follow", "object": "{object}", "sets": "shape"{extra} }} ] }},
        {{ "type": "wait", "duration": 1 }}"#
    )
}

#[test]
fn a_plot_is_drawn_at_law_time_0_until_its_follow_then_at_its_law_time_then_held() {
    let objects = format!(
        r#"{AXES}, "line": {{ "type": "function-plot", "axes": "ax", "expression": "x + t" }}"#
    );
    let scene = scene(&objects, &followed("line", ""));
    let low = |scene_time| {
        numbers(
            &inspect_at(&scene, scene_time)["objects"]["line"]["plot"]["data_bounding_box"]["min"],
        )[1]
    };

    assert_eq!(low("0.5"), 0.0, "before its follow");
    assert_eq!(low("2"), 1.0, "law time 1, a second into the span");
    assert_eq!(low("3"), 2.0);
    assert_eq!(low("3.5"), 2.0, "held from the span's end");
}

#[test]
fn law_time_counts_from_the_chapter_from_names() {
    let objects = format!(
        r#"{AXES}, "line": {{ "type": "function-plot", "axes": "ax", "expression": "x + t" }}"#
    );
    let follow = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "follow", "object": "line", "sets": "shape", "from": "wave" } ] }"#;
    let timeline = format!(
        r#"{{ "type": "wait", "duration": 1 }}, {{ "type": "chapter", "id": "wave", "steps": [ {follow}, {follow} ] }}"#
    );
    let inspected = inspect_at(&scene(&objects, &timeline), "2.5");

    assert_eq!(
        numbers(&inspected["objects"]["line"]["plot"]["data_bounding_box"]["min"])[1],
        1.5,
        "law time 1.5 at scene time 2.5, in the second follow"
    );
}

#[test]
fn a_follow_of_a_shape_starts_at_the_law_time_the_shape_is_drawn_at() {
    let objects = format!(
        r#"{AXES}, "line": {{ "type": "function-plot", "axes": "ax", "expression": "x + t" }}"#
    );
    let follow = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "follow", "object": "line", "sets": "shape" } ] }"#;
    let from_later = format!(
        r#"{{ "type": "chapter", "id": "wave", "steps": [ {{ "type": "wait", "duration": 1 }}, {} ] }}"#,
        follow.replace(r#""sets": "shape""#, r#""sets": "shape", "from": "wave""#)
    );

    let (found, messages) = errors(&objects, &format!("{follow}, {follow}"));
    assert_eq!(
        found,
        pairs(&[("/timeline/1/animations/0", "law_jump")]),
        "the second starts again from 0"
    );
    assert!(messages[0].contains("this follow draws its shape at law time 0, and the law time of the shape of `line` is 1 there"), "{messages:?}");
    assert_eq!(
        errors(&objects, &from_later).0,
        pairs(&[("/timeline/0/steps/1/animations/0", "law_jump")]),
        "law time 1 after a shape drawn at 0"
    );
}

#[test]
fn a_tube_a_surface_and_a_pattern_each_follow_their_law_time() {
    let objects = r##""rope": { "type": "tube", "radius": 0.05, "curve": { "x": "u", "y": "t", "z": "0", "u_range": [0, 1] } },
        "sheet": { "type": "surface", "z": "t", "x_range": [-1, 1], "y_range": [-1, 1], "position": [0, 0, -3] },
        "beads": { "type": "pattern", "shape": { "type": "dot", "radius": 0.1 }, "layout": { "type": "grid", "counts": [2, 1, 1], "spacing": 1 }, "copy": { "position": { "x": "x + t" } } }"##;
    let timeline = r#"{ "type": "play", "duration": 2, "animations": [
        { "type": "follow", "object": "rope", "sets": "shape" },
        { "type": "follow", "object": "sheet", "sets": "shape" },
        { "type": "follow", "object": "beads", "sets": "shape" } ] }"#;
    let inspected = inspect_at(&scene(objects, timeline), "1.5");
    let objects = &inspected["objects"];

    assert_eq!(numbers(&objects["rope"]["tube"]["start"]), [0.0, 1.5, 0.0]);
    assert_eq!(
        numbers(&objects["sheet"]["bounding_box"]["world"]["max"])[1],
        1.5,
        "the sheet risen to its height t"
    );
    assert_eq!(
        numbers(&objects["beads"]["pattern"]["copies"][0]["position"]),
        [1.0, 0.0, 0.0],
        "place -0.5 moved by t"
    );
}

#[test]
fn a_space_curve_with_no_value_at_a_frame_time_is_out_of_range_with_the_scene_time() {
    let objects = r#""rope": { "type": "tube", "radius": 0.05, "curve": { "x": "u", "y": "1/(t - 1)", "z": "0", "u_range": [0, 1] } }"#;
    let timeline = r#"{ "type": "play", "duration": 2, "animations": [ { "type": "follow", "object": "rope", "sets": "shape" } ] }"#;

    let (found, messages) = errors(objects, timeline);

    assert_eq!(found, pairs(&[("/objects/rope/curve/y", "out_of_range")]));
    assert!(
        messages[0].starts_with("at scene time 1 (law time 1), `y` has no finite value"),
        "{messages:?}"
    );
}

#[test]
fn a_follow_of_a_shape_that_reads_no_law_time_or_with_a_law_is_invalid() {
    let objects = format!(
        r#"{AXES}, "flat": {{ "type": "function-plot", "axes": "ax", "expression": "x" }}"#
    );
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "follow", "object": "flat", "sets": "shape", "law": { "x": "0", "y": "0", "z": "0" } } ] }"#;

    assert_eq!(
        errors(&objects, timeline).0,
        pairs(&[
            ("/timeline/0/animations/0/law", "invalid"),
            ("/timeline/0/animations/0/object", "invalid")
        ])
    );
}

#[test]
fn a_shape_law_is_not_morphed_or_carried_along() {
    let objects = format!(
        r#"{AXES}, "wave": {{ "type": "function-plot", "axes": "ax", "expression": "sin(x - t)" }}, "flat": {{ "type": "function-plot", "axes": "ax", "expression": "x" }}, "p": {{ "type": "dot", "radius": 0.1 }}"#
    );
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "morph", "object": "flat", "into": "wave" }, { "type": "follow", "object": "p", "along": "wave" } ] }"#;

    assert_eq!(
        errors(&objects, timeline).0,
        pairs(&[
            ("/timeline/0/animations/0/into", "invalid"),
            ("/timeline/0/animations/1/along", "invalid")
        ])
    );
}

#[test]
fn two_follows_of_one_shape_at_once_conflict() {
    let objects = format!(
        r#"{AXES}, "wave": {{ "type": "function-plot", "axes": "ax", "expression": "sin(x - t)" }}"#
    );
    let follow = r#"{ "type": "follow", "object": "wave", "sets": "shape" }"#;
    let timeline =
        format!(r#"{{ "type": "play", "duration": 1, "animations": [ {follow}, {follow} ] }}"#);

    assert_eq!(
        errors(&objects, &timeline).0,
        pairs(&[("/timeline/0/animations/1", "conflicting_animations")])
    );
}

#[test]
fn a_moving_surface_is_checked_on_every_point_any_frame_time_samples() {
    // The bump at t < 0.5 samples x = 0.298828125, where the flat sheet after has no value; the
    // flat sheet's own points never reach it.
    let objects = r#""sheet": { "type": "surface", "z": "(1 - step(t - 0.5))*exp(-400*(x - 0.3)^2) + (x - 0.298828125)/(x - 0.298828125 + 1 - step(t - 0.5))", "x_range": [0, 1], "y_range": [0, 1] }"#;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "follow", "object": "sheet", "sets": "shape" } ] }"#;

    let (found, messages) = errors(objects, timeline);

    assert_eq!(found, pairs(&[("/objects/sheet/z", "out_of_range")]));
    assert!(
        messages[0].starts_with(
            "at scene time 0.5 (law time 0.5), `z` has no finite value at x = 0.298828125"
        ),
        "{messages:?}"
    );
}
