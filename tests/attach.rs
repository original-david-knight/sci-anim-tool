//! Objects attached to objects that are not molecules, such as a number on its box.
//!
//! The scenes are 400 by 400 pixels with the default camera, so one world unit in the plane
//! z = 0 is 48.28 pixels and the origin is pixel (200, 200).

mod common;

use common::{frame_at, inspect_at, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::{Value, json};

const BOX_AND_DOT: &str = r##""box": { "type": "polygon", "points": [[-1, -1], [1, -1], [1, 1], [-1, 1]], "fill": { "color": "#ff0000" } },
  "dot": { "type": "circle", "radius": 0.3, "fill": { "color": "#0000ff" }, "attach": { "object": "box" } },
  "above": { "type": "circle", "radius": 0.2, "fill": { "color": "#00ff00" }, "attach": { "object": "box", "offset": [0, 1.5] } }"##;

fn center(object: &Value) -> [f64; 2] {
    let pixels = &object["bounding_box"]["pixels"];
    let mid =
        |k: usize| (pixels["min"][k].as_f64().unwrap() + pixels["max"][k].as_f64().unwrap()) / 2.0;
    [mid(0), mid(1)]
}

#[test]
fn one_move_of_a_box_carries_what_is_attached_to_it() {
    let timeline = r##"{ "type": "play", "duration": 2, "animations": [{ "type": "move", "object": "box", "to": [3, -1, 0], "easing_curve": "linear" }] }"##;
    let scene = scene(BOX_AND_DOT, timeline);

    let report = inspect_at(&scene, "1");
    let (_a, halfway) = frame_at(&scene, "1");

    let [box_x, box_y] = center(&report["objects"]["box"]);
    let [dot_x, dot_y] = center(&report["objects"]["dot"]);
    let [above_x, above_y] = center(&report["objects"]["above"]);
    assert!(
        (dot_x - box_x).abs() < 0.5 && (dot_y - box_y).abs() < 0.5,
        "the dot sits on the box's position: {dot_x}, {dot_y} against {box_x}, {box_y}"
    );
    assert!(
        (above_x - box_x).abs() < 0.5 && (box_y - above_y - 1.5 * 48.28).abs() < 0.5,
        "the circle above sits 1.5 above it on the frame: {above_x}, {above_y}"
    );
    assert_eq!(
        pixel(&halfway, 272, 224),
        [0, 0, 255, 255],
        "world (1.5, -0.5): the dot, drawn over the box it lies in the plane of"
    );
    assert_eq!(
        report["objects"]["dot"]["attached"],
        json!({ "object": "box" })
    );
}

#[test]
fn an_attached_object_exists_while_the_object_it_is_attached_to_does() {
    let objects = format!(
        r##"{BOX_AND_DOT},
  "ring": {{ "type": "circle", "radius": 1, "fill": {{ "color": "#ff0000" }} }}"##
    );
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "morph", "object": "box", "into": "ring" }] }"##;
    let scene = scene(&objects, timeline);

    assert_eq!(
        inspect_at(&scene, "0.5")["objects"]["dot"]["opacity"],
        json!(1.0)
    );
    assert_eq!(
        inspect_at(&scene, "1")["objects"]["dot"]["opacity"],
        json!(0.0),
        "the box is used up by its morph, and the dot with it"
    );
}

#[test]
fn an_attach_to_itself_a_plot_or_an_attached_object_or_to_a_group_of_a_shape_is_an_error() {
    let objects = r##""box": { "type": "polygon", "points": [[-1, -1], [1, -1], [1, 1], [-1, 1]], "fill": { "color": "#ff0000" } },
  "ax": { "type": "axes", "x": { "range": [0, 1] }, "y": { "range": [0, 1] }, "width": 2, "height": 2 },
  "curve": { "type": "function-plot", "axes": "ax", "expression": "x" },
  "self": { "type": "circle", "radius": 0.1, "fill": { "color": "#0000ff" }, "attach": { "object": "self" } },
  "on_plot": { "type": "circle", "radius": 0.1, "fill": { "color": "#0000ff" }, "attach": { "object": "curve" } },
  "first": { "type": "circle", "radius": 0.1, "fill": { "color": "#0000ff" }, "attach": { "object": "box" } },
  "chained": { "type": "circle", "radius": 0.1, "fill": { "color": "#0000ff" }, "attach": { "object": "first" } },
  "grouped": { "type": "circle", "radius": 0.1, "fill": { "color": "#0000ff" }, "attach": { "object": "box", "group": "corner" } }"##;
    let (_dir, path) = scene_file(&scene(objects, ""));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/chained/attach/object", "not_attachable"),
            ("/objects/grouped/attach/group", "not_molecule"),
            ("/objects/on_plot/attach/object", "not_attachable"),
            ("/objects/self/attach/object", "not_attachable"),
        ])
    );
}

#[test]
fn a_move_of_an_object_attached_to_a_box_says_to_move_the_box() {
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "move", "object": "dot", "to": [1, 0, 0] }] }"##;
    let (_dir, path) = scene_file(&scene(BOX_AND_DOT, timeline));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/0/animations/0/object", "attached")])
    );
    let message = run.json["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("move, scale, rotate or follow `box` instead"),
        "{message}"
    );
}
