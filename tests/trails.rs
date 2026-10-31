//! Trails: the world points an object passed through, read back from reports as an agent reads
//! them.

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

fn errors(objects: &str, timeline: &str) -> Vec<(String, String)> {
    let (_dir, path) = scene_file(&scene(objects, timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    scene_errors(&run)
}

const DOT: &str = r#""p": { "type": "dot", "radius": 0.1 }"#;

/// The dot moves from 0 to 4 along x over 4 s at a steady pace, 60 frames a second.
const SLIDE: &str = r#"{ "type": "play", "duration": 4, "animations": [ { "type": "follow", "object": "p", "law": { "x": "t", "y": "0", "z": "0" } } ] }"#;

#[test]
fn a_trail_draws_its_object_s_last_seconds_and_ends_where_it_stands() {
    let objects = format!(r#"{DOT}, "path": {{ "type": "trail", "object": "p", "length": 1 }}"#);
    let scene = scene(&objects, SLIDE);
    let at_3 = inspect_at(&scene, "3.5");
    let trail = &at_3["objects"]["path"];

    assert_eq!(numbers(&trail["trail"]["newest"]), [3.5, 0.0, 0.0]);
    assert_eq!(
        trail["trail"]["point_count"], 61,
        "the frame times from 2.5 up to 3.5, then 3.5"
    );
    assert_eq!(
        numbers(&trail["bounding_box"]["world"]["min"])[0],
        2.5,
        "from 2.5, its end cut square as a stroke's is"
    );
}

#[test]
fn a_trail_with_no_length_reaches_back_to_its_object_s_create() {
    let objects = r##""c": { "type": "circle", "radius": 0.2, "stroke": { "color": "#ffffff" } }, "path": { "type": "trail", "object": "c" }"##;
    let timeline = r#"{ "type": "wait", "duration": 1 },
        { "type": "play", "duration": 2, "animations": [ { "type": "create", "object": "c", "end": 0.5 }, { "type": "move", "object": "c", "to": [2, 0, 0], "easing_curve": "linear" } ] }"#;
    let scene = scene(objects, timeline);

    assert_eq!(
        inspect_at(&scene, "0.5")["objects"]["path"]["trail"]["point_count"],
        0,
        "nothing before the create"
    );
    assert_eq!(
        numbers(&inspect_at(&scene, "3")["objects"]["path"]["bounding_box"]["world"]["min"])[0],
        0.0,
        "from where it was created"
    );
}

#[test]
fn a_trail_follows_a_data_point_in_the_world() {
    let objects = r#""ax": { "type": "axes", "x": { "range": [0, 4] }, "y": { "range": [0, 4] }, "width": 4, "height": 4 },
        "p": { "type": "dot", "radius": 0.1, "axes": "ax", "position": [1, 1, 0] },
        "path": { "type": "trail", "object": "p" }"#;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "move", "object": "p", "to": [3, 3, 0] } ] }"#;

    let inspected = inspect_at(&scene(objects, timeline), "1");

    assert_eq!(
        numbers(&inspected["objects"]["path"]["trail"]["newest"]),
        [1.0, 1.0, 0.0],
        "data (3, 3) on axes centered on the origin"
    );
}

#[test]
fn a_trail_s_stroke_is_recolored() {
    let objects = format!(
        r##"{DOT}, "path": {{ "type": "trail", "object": "p", "stroke": {{ "color": "#ffffff" }} }}"##
    );
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [ { "type": "recolor", "object": "path", "stroke": "#fc6255" } ] }"##;

    assert_eq!(
        inspect_at(&scene(&objects, timeline), "1")["objects"]["path"]["colors"]["stroke"],
        "#fc6255"
    );
}

#[test]
fn a_trail_names_an_object_with_a_place_and_has_none_of_its_own() {
    let objects = r#""ax": { "type": "axes", "x": { "range": [0, 4] }, "y": { "range": [0, 4] }, "width": 4, "height": 4 },
        "curve": { "type": "function-plot", "axes": "ax", "expression": "x" },
        "a": { "type": "trail", "object": "nowhere" },
        "b": { "type": "trail", "object": "curve" },
        "c": { "type": "trail", "object": "c" },
        "d": { "type": "trail", "object": "ax", "position": [1, 0, 0] }"#;

    assert_eq!(
        errors(objects, ""),
        pairs(&[
            ("/objects/a/object", "unknown_object"),
            ("/objects/b/object", "invalid"),
            ("/objects/c/object", "invalid"),
            ("/objects/d/position", "placed_by_trail")
        ])
    );
}

#[test]
fn a_trail_is_not_moved_created_or_filled() {
    let objects = format!(r#"{DOT}, "path": {{ "type": "trail", "object": "p" }}"#);
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [
        { "type": "move", "object": "path", "to": [1, 0, 0] },
        { "type": "create", "object": "path" },
        { "type": "recolor", "object": "path", "fill": "#ffffff" } ] }"##;

    assert_eq!(
        errors(&objects, timeline),
        pairs(&[
            ("/timeline/0/animations/1/object", "invalid"),
            ("/timeline/0/animations/2/fill", "not_recolorable"),
            ("/timeline/0/animations/0/object", "placed_by_trail")
        ])
    );
}

#[test]
fn a_trail_follows_an_object_attached_to_a_shape_that_grows() {
    let objects = r#""sheet": { "type": "surface", "z": "t", "x_range": [-1, 1], "y_range": [-1, 1] },
        "n": { "type": "text", "text": "N", "size": 0.3, "attach": { "object": "sheet", "offset": [1, 0] } },
        "path": { "type": "trail", "object": "n", "length": 0.5 }"#;
    let timeline = r#"{ "type": "play", "duration": 2, "animations": [ { "type": "follow", "object": "sheet", "sets": "shape" } ] }"#;
    let scene = scene(objects, timeline);
    let back = numbers(&inspect_at(&scene, "1.5")["objects"]["n"]["position"]);

    assert_eq!(
        numbers(&inspect_at(&scene, "2")["objects"]["path"]["bounding_box"]["world"]["min"])[2],
        back[2],
        "back to where the caption was half a second before, beside the risen sheet"
    );
}

#[test]
fn a_trail_reaches_no_further_than_where_its_object_stands() {
    // 1.01 s at 30 frames a second is 30 frames, the last at 0.9667, so the scene ends between
    // frame times.
    let objects = format!(r#"{DOT}, "path": {{ "type": "trail", "object": "p" }}"#);
    let timeline = r#"{ "type": "play", "duration": 1.01, "animations": [ { "type": "follow", "object": "p", "law": { "x": "t", "y": "0", "z": "0" } } ] }"#;
    let scene = scene(&objects, timeline).replace(r#""background""#, r#""fps": 30, "background""#);

    assert_eq!(
        numbers(&inspect_at(&scene, "1.005")["objects"]["path"]["bounding_box"]["world"]["max"])[0],
        1.005
    );
}
