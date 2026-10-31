//! A dot on 2D axes is a data point, read back from reports as an agent reads them.
//!
//! The scenes are 400 by 400 pixels on black, with the camera at `[0, 0, 10]` looking at the
//! origin. The axes `a` put x from -1 to 3 over 4 and y from 0 to 8 over 4, so data `(1, 1)` lands
//! at world `(0, -1.5)`.

mod common;

use common::{inspect_at, pairs, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n.as_f64().unwrap() * 1000.0).round() / 1000.0)
        .collect()
}

fn validate(scene: &str) -> common::Run {
    let (_dir, path) = scene_file(scene);
    sci_anim(&["validate", path.to_str().unwrap()])
}

fn messages(run: &common::Run) -> Vec<String> {
    run.json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap().to_string())
        .collect()
}

/// Axes `a`, the plot `f` of x² on them from x = 1, and a yellow dot `p` of radius 0.08 on them
/// at data `position`.
fn on_axes(position: &str) -> String {
    format!(
        r##""a": {{ "type": "axes", "x": {{ "range": [-1, 3] }}, "y": {{ "range": [0, 8] }}, "width": 4, "height": 4 }},
           "f": {{ "type": "function-plot", "axes": "a", "expression": "x^2", "x_range": [1, 2.8] }},
           "p": {{ "type": "dot", "axes": "a", "radius": 0.08, "position": {position}, "fill": {{ "color": "#ffd23f" }} }}"##
    )
}

fn world_box(inspected: &Value, id: &str) -> (Vec<f64>, Vec<f64>) {
    let world = &inspected["objects"][id]["bounding_box"]["world"];
    (numbers(&world["min"]), numbers(&world["max"]))
}

#[test]
fn a_dot_on_axes_stands_where_the_plot_puts_the_same_data_point() {
    let inspected = inspect_at(&scene(&on_axes("[1, 1, 0]"), ""), "0");

    let p = &inspected["objects"]["p"];
    assert_eq!(numbers(&p["position"]), [0.0, -1.5, 0.0]);
    assert_eq!(numbers(&p["dot"]["center"]), [0.0, -1.5, 0.0]);
    assert_eq!(
        numbers(&p["data_point"]["position"]),
        [1.0, 1.0, 0.0],
        "the position as the scene file declares it"
    );
    assert_eq!(p["data_point"]["axes"], "a");
    assert_eq!(
        world_box(&inspected, "p"),
        (vec![-0.08, -1.58, 0.0], vec![0.08, -1.42, 0.0])
    );

    // A dot in world units carried along the plot is told where the plot's curve starts: x² at
    // x = 1, by the plot's own mapping.
    let world_dot = format!(
        r##"{}, "w": {{ "type": "dot", "radius": 0.08 }}"##,
        on_axes("[1, 1, 0]")
    );
    let carried = validate(&scene(
        &world_dot,
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "follow", "object": "w", "along": "f" }] }"#,
    ));
    assert_eq!(
        scene_errors(&carried),
        pairs(&[("/timeline/0/animations/0/along", "carry_jump")])
    );
    assert!(
        messages(&carried)[0].contains("declare `w`'s `position` as [0, -1.5, 0]"),
        "{:?}",
        messages(&carried)
    );
}

#[test]
fn a_move_and_a_follow_of_a_dot_on_axes_are_in_data_coordinates() {
    let moved = scene(
        &on_axes("[1, 1, 0]"),
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "move", "object": "p", "to": [2, 4, 0.5] }] },
           { "type": "play", "duration": 1, "animations": [{ "type": "follow", "object": "p", "law": { "x": "2 - t", "y": "(2 - t)^2", "z": "0.5" } }] }"#,
    );

    let after_move = inspect_at(&moved, "1");
    assert_eq!(
        numbers(&after_move["objects"]["p"]["position"]),
        [1.0, 0.0, 0.5],
        "z is off the axes' plane, in world units"
    );
    let mid_follow = inspect_at(&moved, "1.5");
    assert_eq!(
        numbers(&mid_follow["objects"]["p"]["data_point"]["position"]),
        [1.5, 2.25, 0.5]
    );
    assert_eq!(
        numbers(&mid_follow["objects"]["p"]["position"]),
        [0.5, -0.875, 0.5]
    );
}

#[test]
fn the_axes_carry_a_dot_on_them_and_it_stays_round() {
    let carried = scene(
        &on_axes("[1, 1, 0.5]"),
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "move", "object": "a", "to": [1, 0, 0] }, { "type": "scale", "object": "a", "to": [2, 3, 2] }] }"#,
    );

    let inspected = inspect_at(&carried, "1");
    assert_eq!(
        numbers(&inspected["objects"]["p"]["position"]),
        [1.0, -4.5, 0.5],
        "z stays 0.5 off the plane, in world units, though the axes doubled in z"
    );
    assert_eq!(
        world_box(&inspected, "p"),
        (vec![0.92, -4.58, 0.5], vec![1.08, -4.42, 0.5]),
        "still 0.08 round, though the axes stretched unevenly"
    );
    assert_eq!(
        numbers(&inspected["objects"]["p"]["data_point"]["position"]),
        [1.0, 1.0, 0.5]
    );
}

#[test]
fn a_linear_map_of_the_axes_carries_a_dot_on_them() {
    let mapped = scene(
        &on_axes("[1, 1, 0]"),
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "linear-map", "objects": ["a"], "matrix": [[1, 1], [0, 1]] }] }"#,
    );

    let inspected = inspect_at(&mapped, "1");
    assert_eq!(
        numbers(&inspected["objects"]["p"]["dot"]["center"]),
        [-1.5, -1.5, 0.0],
        "the shear takes (0, -1.5) to (-1.5, -1.5), as it takes the plot"
    );
    assert_eq!(
        numbers(&inspected["objects"]["p"]["data_point"]["position"]),
        [1.0, 1.0, 0.0]
    );
}

#[test]
fn a_dot_carried_along_a_plot_on_its_axes_stays_on_the_curve() {
    let carry = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "follow", "object": "p", "along": "f", "easing_curve": "linear" }] }"#;
    let carried = scene(&on_axes("[1, 1, 0]"), carry);

    assert_eq!(
        validate(&carried).exit_code,
        0,
        "{}",
        validate(&carried).stdout
    );
    for scene_time in ["0.3", "0.6", "1"] {
        let at =
            numbers(&inspect_at(&carried, scene_time)["objects"]["p"]["data_point"]["position"]);
        assert!(
            (at[1] - at[0] * at[0]).abs() < 0.01,
            "y = x² on the curve at {scene_time}: {at:?}"
        );
    }
    assert_eq!(
        numbers(&inspect_at(&carried, "1")["objects"]["p"]["data_point"]["position"]),
        [2.8, 7.84, 0.0]
    );

    let wrong = validate(&scene(&on_axes("[0, 0, 0]"), carry));
    assert_eq!(
        scene_errors(&wrong),
        pairs(&[("/timeline/0/animations/0/along", "carry_jump")])
    );
    assert!(
        messages(&wrong)[0].contains("declare `p`'s `position` as [1, 1, 0]"),
        "in data coordinates: {:?}",
        messages(&wrong)
    );
}

#[test]
fn an_attachment_to_a_dot_on_axes_centers_on_it() {
    let captioned = format!(
        r##"{}, "n": {{ "type": "text", "text": "P", "size": 0.3, "attach": {{ "object": "p", "offset": [0, 0] }} }}"##,
        on_axes("[2, 4, 0]")
    );

    let inspected = inspect_at(&scene(&captioned, ""), "0");
    let center = |id: &str| {
        let pixels = &inspected["objects"][id]["bounding_box"]["pixels"];
        [0, 1].map(|k| {
            (pixels["min"][k].as_f64().unwrap() + pixels["max"][k].as_f64().unwrap()) / 2.0
        })
    };
    let (point, caption) = (center("p"), center("n"));
    assert!(
        (point[0] - caption[0]).abs() < 1.0 && (point[1] - caption[1]).abs() < 1.0,
        "the P sits on the point's pixel: {point:?} and {caption:?}"
    );
    assert_eq!(inspected["objects"]["n"]["attached"]["object"], "p");
}

#[test]
fn a_dot_off_2d_axes_and_a_sphere_on_them_are_each_invalid_at_axes_at_once() {
    let objects = r##""flat": { "type": "axes", "x": { "range": [0, 1] }, "y": { "range": [0, 1] }, "width": 2, "height": 2 },
        "space": { "type": "3d-axes", "x": { "range": [0, 1], "length": 1 }, "y": { "range": [0, 1], "length": 1 }, "z": { "range": [0, 1], "length": 1 } },
        "line": { "type": "number-line", "range": [0, 1], "length": 2 },
        "ring": { "type": "circle", "radius": 1 },
        "on_space": { "type": "dot", "axes": "space", "radius": 0.1 },
        "on_line": { "type": "dot", "axes": "line", "radius": 0.1 },
        "on_ring": { "type": "dot", "axes": "ring", "radius": 0.1 },
        "ball": { "type": "sphere", "axes": "flat", "radius": 0.1 },
        "dots": { "type": "pattern", "shape": { "type": "dot", "axes": "flat", "radius": 0.1 }, "layout": { "type": "ring", "count": 3, "radius": 1 } }"##;

    let run = validate(&scene(objects, ""));
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/dots/shape/axes", "invalid"),
            ("/objects/ball/axes", "invalid"),
            ("/objects/on_line/axes", "invalid"),
            ("/objects/on_ring/axes", "invalid"),
            ("/objects/on_space/axes", "invalid")
        ])
    );
    let said = messages(&run).join("\n");
    for words in [
        "a dot is flat; the data point on 3D axes is a `sphere`",
        "no y to place a point by",
        "`ring` is not axes",
        "mark a point on 2D axes with a `dot`",
    ] {
        assert!(said.contains(words), "{words:?} in {said}");
    }
}

#[test]
fn a_dot_on_axes_is_not_mapped_morphed_or_carried_along() {
    let objects = format!(
        r##"{}, "q": {{ "type": "dot", "axes": "a", "radius": 0.08, "position": [2, 4, 0] }}, "w": {{ "type": "dot", "radius": 0.08 }}"##,
        on_axes("[1, 1, 0]")
    );
    let refused = |animation: &str| {
        scene_errors(&validate(&scene(
            &objects,
            &format!(r#"{{ "type": "play", "duration": 1, "animations": [{animation}] }}"#),
        )))
    };

    assert_eq!(
        refused(r#"{ "type": "linear-map", "objects": ["p"], "matrix": [[2, 0], [0, 1]] }"#),
        pairs(&[("/timeline/0/animations/0/objects/0", "not_mappable")])
    );
    assert_eq!(
        refused(r#"{ "type": "morph", "object": "p", "into": "q" }"#),
        pairs(&[("/timeline/0/animations/0/object", "not_morphable")])
    );
    assert_eq!(
        refused(r#"{ "type": "follow", "object": "w", "along": "p" }"#),
        pairs(&[("/timeline/0/animations/0/along", "invalid")])
    );
}
