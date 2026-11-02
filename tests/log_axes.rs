//! Axes with a log axis, read back from reports and frames as an agent reads them.
//!
//! The scenes are 400 by 400 pixels with the default camera, so a world point (x, y) lands at
//! pixel (200 + 48.28 x, 200 - 48.28 y). The axes `a` put x from 0 to 4 over 4 and y from 10^-2
//! to 10^4 on a log axis over 6, so each power of ten of y is one world unit, and data y = 1 is
//! at world y = -1.

mod common;

use common::{frame_at, inspect_at, pairs, scene, scene_errors, scene_file, sci_anim};
use serde_json::{Value, json};

const RED: [u8; 4] = [255, 0, 0, 255];

const LOG_Y: &str = r##""a": { "type": "axes", "x": { "range": [0, 4] }, "y": { "range": [0.01, 10000], "scale": "log" }, "width": 4, "height": 6 }"##;

fn rounded(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n.as_f64().unwrap() * 1000.0).round() / 1000.0)
        .collect()
}

fn errors(objects: &str, timeline: &str) -> Vec<(String, String)> {
    let (_dir, path) = scene_file(&scene(objects, timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    scene_errors(&run)
}

#[test]
fn a_log_axis_ticks_each_power_of_ten_and_labels_it_as_a_power() {
    let inspected = inspect_at(&scene(LOG_Y, ""), "0");

    let axes = &inspected["objects"]["a"]["axes"];
    assert_eq!(
        axes["y"],
        json!({ "range": [0.01, 10000.0], "scale": "log", "spacing": 1.0 })
    );
    assert_eq!(
        axes["x"],
        json!({ "range": [0.0, 4.0], "spacing": 1.0 }),
        "a linear axis reports no scale"
    );
    let y_ticks: Vec<(f64, String, f64)> = axes["ticks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|tick| tick["axis"] == "y")
        .map(|tick| {
            (
                tick["value"].as_f64().unwrap(),
                tick["label_text"].as_str().unwrap_or("").to_string(),
                rounded(&tick["world"])[1],
            )
        })
        .collect();
    assert_eq!(
        y_ticks,
        [
            (0.01, "10^{-2}".to_string(), -3.0),
            (0.1, "10^{-1}".to_string(), -2.0),
            (1.0, "10^{0}".to_string(), -1.0),
            (10.0, "10^{1}".to_string(), 0.0),
            (100.0, "10^{2}".to_string(), 1.0),
            (1000.0, "10^{3}".to_string(), 2.0),
            (10000.0, "10^{4}".to_string(), 3.0),
        ]
    );
    assert_eq!(
        rounded(&axes["zero"]["world"]),
        [-2.0, -1.0, 0.0],
        "x = 0 and y = 1, the power of ten 10^0"
    );
    assert_eq!(
        rounded(&axes["y_unit"]),
        [0.0, 1.0, 0.0],
        "one power of ten of y"
    );
}

#[test]
fn a_data_point_at_1000_sits_at_the_tick_of_10_to_the_3() {
    let objects = format!(
        r##"{LOG_Y}, "p": {{ "type": "dot", "axes": "a", "radius": 0.1, "position": [2, 1000, 0], "fill": {{ "color": "#ff0000" }} }}"##
    );
    let inspected = inspect_at(&scene(&objects, ""), "0");

    let p = &inspected["objects"]["p"];
    assert_eq!(rounded(&p["position"]), [0.0, 2.0, 0.0]);
    assert_eq!(rounded(&p["data_point"]["position"]), [2.0, 1000.0, 0.0]);
    let tick = inspected["objects"]["a"]["axes"]["ticks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tick| tick["axis"] == "y" && tick["value"] == 1000.0)
        .unwrap()
        .clone();
    assert_eq!(rounded(&tick["world"])[1], rounded(&p["position"])[1]);

    let (_dir, frame) = frame_at(&scene(&objects, ""), "0");
    assert_eq!(common::pixel(&frame, 200, 103), RED, "world (0, 2)");
}

#[test]
fn a_move_on_a_log_axis_ends_at_its_data_value() {
    let objects = format!(
        r##"{LOG_Y}, "p": {{ "type": "dot", "axes": "a", "radius": 0.1, "position": [1, 0.1, 0], "fill": {{ "color": "#ff0000" }} }}"##
    );
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "move", "object": "p", "to": [3, 100, 0] }] }"#;
    let inspected = inspect_at(&scene(&objects, timeline), "1");

    let p = &inspected["objects"]["p"];
    assert_eq!(rounded(&p["position"]), [1.0, 1.0, 0.0]);
    assert_eq!(rounded(&p["data_point"]["position"]), [3.0, 100.0, 0.0]);
}

#[test]
fn a_bar_on_a_log_axis_reaches_its_value() {
    let objects = r##""a": { "type": "axes", "x": { "categories": ["A", "B"] }, "y": { "range": [1, 10000], "scale": "log" }, "width": 4, "height": 4 },
        "b": { "type": "bar-plot", "axes": "a", "values": [100, 10000], "fill": { "color": "#ff0000" } }"##;
    let inspected = inspect_at(&scene(objects, ""), "0");

    let bars = inspected["objects"]["b"]["bar_plot"]["bars"]
        .as_array()
        .unwrap();
    let top = |k: usize| rounded(&bars[k]["rectangle"]["world"]["max"])[1];
    assert_eq!(
        (top(0), top(1)),
        (0.0, 2.0),
        "10^2 halfway up, 10^4 at the top"
    );
}

#[test]
fn a_log_axis_range_at_or_below_0_is_refused_at_the_range() {
    let objects = r##""a": { "type": "axes", "x": { "range": [0, 4] }, "y": { "range": [0, 100], "scale": "log" }, "width": 4, "height": 4 }"##;
    assert_eq!(
        errors(objects, ""),
        pairs(&[("/objects/a/y/range", "out_of_range")])
    );
    let objects = r##""a": { "type": "axes", "x": { "range": [-1, 4], "scale": "log" }, "y": { "range": [1, 100] }, "width": 4, "height": 4 }"##;
    assert_eq!(
        errors(objects, ""),
        pairs(&[("/objects/a/x/range", "out_of_range")])
    );
    let objects = r##""a": { "type": "axes", "x": { "range": [0, 4] }, "y": { "range": [1, 100], "scale": "log", "spacing": 0.5 }, "width": 4, "height": 4 }"##;
    assert_eq!(
        errors(objects, ""),
        pairs(&[("/objects/a/y/spacing", "invalid")])
    );
}

#[test]
fn a_value_at_or_below_0_on_a_log_axis_is_refused_where_it_is_written() {
    let dot = |position: &str| {
        format!(
            r##"{LOG_Y}, "p": {{ "type": "dot", "axes": "a", "radius": 0.1, "position": {position}, "fill": {{ "color": "#ff0000" }} }}"##
        )
    };
    assert_eq!(
        errors(&dot("[1, 0, 0]"), ""),
        pairs(&[("/objects/p/position", "out_of_range")])
    );
    assert_eq!(
        errors(
            &dot("[1, 1, 0]"),
            r#"{ "type": "play", "duration": 1, "animations": [{ "type": "move", "object": "p", "to": [1, -5, 0] }] }"#
        ),
        pairs(&[("/timeline/0/animations/0/to", "out_of_range")])
    );
    assert_eq!(
        errors(
            &dot("[1, 1, 0]"),
            r#"{ "type": "play", "duration": 2, "animations": [{ "type": "follow", "object": "p", "law": { "x": "1", "y": "1 - t", "z": "0" } }] }"#
        ),
        pairs(&[("/timeline/0/animations/0/law/y", "out_of_range")])
    );
    let points = format!(
        r##"{LOG_Y}, "f": {{ "type": "point-plot", "axes": "a", "points": [[0, 1], [1, 10], [2, -3]] }}"##
    );
    assert_eq!(
        errors(&points, ""),
        pairs(&[("/objects/f/points/2", "out_of_range")])
    );
    let bars = r##""a": { "type": "axes", "x": { "categories": ["A", "B"] }, "y": { "range": [1, 100], "scale": "log" }, "width": 4, "height": 4 },
        "b": { "type": "bar-plot", "axes": "a", "values": [10, 0], "fill": { "color": "#ff0000" } }"##;
    assert_eq!(
        errors(bars, ""),
        pairs(&[("/objects/b/values/1", "out_of_range")])
    );
}
