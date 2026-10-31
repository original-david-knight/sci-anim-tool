//! Linear maps and dots, read back from inspect reports and frames as an agent reads them.
//!
//! The scenes are 400 by 400 pixels with the default camera, so one world unit in the plane
//! z = 0 is 48.28 pixels and a world point (x, y) lands at pixel (200 + 48.28 x, 200 - 48.28 y).
//! Every expected number is worked out by hand from the matrices, not read from a run.

mod common;

use common::{frame_at, inspect_at, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::{Value, json};

const BLACK: [u8; 4] = [0, 0, 0, 255];
const RED: [u8; 4] = [255, 0, 0, 255];

/// Each number of a JSON array, rounded to 3 decimals.
fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("an array, not {value}"))
        .iter()
        .map(|n| (n.as_f64().unwrap() * 1000.0).round() / 1000.0)
        .collect()
}

fn rows(value: &Value) -> Vec<Vec<f64>> {
    value.as_array().unwrap().iter().map(numbers).collect()
}

/// Whether each number is within `tolerance` of the expected.
fn near(value: &Value, expected: &[f64], tolerance: f64) -> bool {
    let actual: Vec<f64> = value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_f64().unwrap())
        .collect();
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (a - e).abs() <= tolerance)
}

fn validate(scene: &str) -> common::Run {
    let (_dir, path) = scene_file(scene);
    sci_anim(&["validate", path.to_str().unwrap()])
}

fn one_step(duration: f64, animations: &str) -> String {
    format!(r#"{{ "type": "play", "duration": {duration}, "animations": [{animations}] }}"#)
}

/// Every place a report holds `null`, as JSON Pointers. serde writes a NaN as `null`.
fn nulls(value: &Value, at: &str, found: &mut Vec<String>) {
    match value {
        Value::Null => found.push(at.to_string()),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .for_each(|(i, item)| nulls(item, &format!("{at}/{i}"), found)),
        Value::Object(map) => map
            .iter()
            .for_each(|(key, item)| nulls(item, &format!("{at}/{key}"), found)),
        _ => {}
    }
}

const SVD_POINTS: &str = r##""p1": { "type": "dot", "radius": 0.05, "position": [0.7071, 0.7071, 0], "fill": { "color": "#ff0000" } },
  "p2": { "type": "dot", "radius": 0.05, "position": [-0.7071, 0.7071, 0], "fill": { "color": "#00ff00" } },
  "v1": { "type": "arrow", "from": [0, 0], "to": [0.7071, 0.7071], "tip_length": 0.2 }"##;

fn svd_steps(first_route: &str) -> String {
    let map = |matrix: &str, route: &str| {
        one_step(
            2.0,
            &format!(
                r#"{{ "type": "linear-map", "objects": ["p1", "p2", "v1"], "matrix": {matrix}, {route} "easing_curve": "linear" }}"#
            ),
        )
    };
    [
        map("[[0.7071, 0.7071], [-0.7071, 0.7071]]", first_route),
        map("[[2, 0], [0, 0.5]]", ""),
        map("[[0.8660, -0.5], [0.5, 0.8660]]", r#""route": "turn","#),
    ]
    .join(", ")
}

#[test]
fn dots_and_an_arrow_tip_end_at_the_product_of_every_matrix_before_them() {
    let report = inspect_at(&scene(SVD_POINTS, &svd_steps(r#""route": "turn","#)), "6");

    let objects = &report["objects"];
    assert_eq!(
        numbers(&objects["p1"]["dot"]["center"]),
        [1.732, 1.0, 0.0],
        "U·Σ·Vᵀ of (0.7071, 0.7071)"
    );
    assert_eq!(
        numbers(&objects["p2"]["dot"]["center"]),
        [-0.25, 0.433, 0.0]
    );
    assert_eq!(
        numbers(&objects["v1"]["arrow"]["to"]),
        [1.732, 1.0, 0.0],
        "the arrow's tip is p1"
    );
    assert_eq!(numbers(&objects["v1"]["arrow"]["from"]), [0.0, 0.0, 0.0]);
    assert_eq!(
        rows(&objects["p1"]["plane_map"]["matrix"]),
        [[1.401, 1.048], [0.401, 1.013]],
        "the later map on the left: U·Σ·Vᵀ"
    );
    assert_eq!(numbers(&objects["p1"]["plane_map"]["shift"]), [0.0, 0.0]);
    assert_eq!(
        numbers(&objects["p1"]["position"]),
        [0.707, 0.707, 0.0],
        "the transform is as declared"
    );
    assert_eq!(
        numbers(&objects["p1"]["dot"]["center_pixels"]),
        [283.627, 151.717],
        "48.284 pixels a unit from the middle"
    );
}

#[test]
fn halfway_through_v_transpose_a_turn_keeps_the_length_and_entries_take_the_chord() {
    let turning = inspect_at(&scene(SVD_POINTS, &svd_steps(r#""route": "turn","#)), "1");
    let straight = inspect_at(&scene(SVD_POINTS, &svd_steps("")), "1");

    assert_eq!(
        numbers(&turning["objects"]["p1"]["dot"]["center"]),
        [0.924, 0.383, 0.0],
        "22.5° round at length 1"
    );
    assert_eq!(
        numbers(&straight["objects"]["p1"]["dot"]["center"]),
        [0.854, 0.354, 0.0],
        "the chord's middle, at length 0.924"
    );
}

const CIRCLE_RADII_AND_DOT: &str = r##""c": { "type": "circle", "radius": 1, "stroke": { "color": "#58c4dd", "width": 0.04 } },
  "ex": { "type": "arrow", "from": [0, 0], "to": [1, 0], "tip_length": 0.2 },
  "ey": { "type": "arrow", "from": [0, 0], "to": [0, 1], "tip_length": 0.2 },
  "d": { "type": "dot", "radius": 0.06, "position": [0, 1, 0] }"##;

/// Where a world point is on the unit circle that `plane_map` carries: 1 when on it.
fn on_mapped_unit_circle(plane_map: &Value, point: &Value) -> f64 {
    let exact = |v: &Value| {
        v.as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_f64().unwrap())
            .collect::<Vec<f64>>()
    };
    let m: Vec<Vec<f64>> = plane_map["matrix"]
        .as_array()
        .unwrap()
        .iter()
        .map(exact)
        .collect();
    let shift = exact(&plane_map["shift"]);
    let (x, y) = (
        point[0].as_f64().unwrap() - shift[0],
        point[1].as_f64().unwrap() - shift[1],
    );
    let determinant = m[0][0] * m[1][1] - m[0][1] * m[1][0];
    let (u, v) = (
        (m[1][1] * x - m[0][1] * y) / determinant,
        (-m[1][0] * x + m[0][0] * y) / determinant,
    );
    u.hypot(v)
}

#[test]
fn under_a_shear_the_circle_its_radii_and_the_dot_at_a_tip_stay_together_at_every_sampled_time() {
    let animation = r#"{ "type": "linear-map", "objects": ["c", "ex", "ey", "d"], "matrix": [[1, 1], [0, 1]], "easing_curve": "linear" }"#;
    let scene = scene(CIRCLE_RADII_AND_DOT, &one_step(2.0, animation));

    for (time, p, half_width) in [
        ("0.5", 0.25, 1.0308),
        ("1", 0.5, 1.1180),
        ("1.5", 0.75, 1.25),
        ("2", 1.0, std::f64::consts::SQRT_2),
    ] {
        let objects = &inspect_at(&scene, time)["objects"];
        let map = &objects["c"]["plane_map"];
        assert_eq!(rows(&map["matrix"]), [[1.0, p], [0.0, 1.0]], "at {time}");
        for id in ["ex", "ey", "d"] {
            assert_eq!(
                &objects[id]["plane_map"], map,
                "{id} follows the circle's map at {time}"
            );
        }
        assert_eq!(
            numbers(&objects["ey"]["arrow"]["to"]),
            [p, 1.0, 0.0],
            "at {time}"
        );
        assert_eq!(
            numbers(&objects["d"]["dot"]["center"]),
            [p, 1.0, 0.0],
            "the dot sits on ey's tip at {time}"
        );
        assert_eq!(
            numbers(&objects["ex"]["arrow"]["to"]),
            [1.0, 0.0, 0.0],
            "a shear along x leaves x alone, at {time}"
        );
        for tip in [
            &objects["ex"]["arrow"]["to"],
            &objects["ey"]["arrow"]["to"],
            &objects["d"]["dot"]["center"],
        ] {
            assert!(
                (on_mapped_unit_circle(map, tip) - 1.0).abs() < 1e-5,
                "{tip} lies on the mapped circle at {time}"
            );
        }
        let world = &objects["c"]["bounding_box"]["world"];
        assert!(
            near(&world["max"], &[half_width + 0.02, 1.02, 0.0], 0.005),
            "the ellipse, √(1 + p²) wide, plus half the stroke at {time}: {world}"
        );
    }
}

#[test]
fn under_a_turning_stretch_the_radii_turn_at_the_circles_growing_radius() {
    let animation = r#"{ "type": "linear-map", "objects": ["c", "ex", "ey", "d"], "matrix": [[0, -2], [2, 0]], "route": "turn", "easing_curve": "linear" }"#;
    let scene = scene(CIRCLE_RADII_AND_DOT, &one_step(2.0, animation));

    for (time, radius, ex_to) in [
        ("0.5", 1.25, [1.155, 0.478]),
        ("1", 1.5, [1.061, 1.061]),
        ("1.5", 1.75, [0.67, 1.617]),
        ("2", 2.0, [0.0, 2.0]),
    ] {
        let objects = &inspect_at(&scene, time)["objects"];
        let map = &objects["c"]["plane_map"];
        assert_eq!(
            numbers(&objects["ex"]["arrow"]["to"]),
            [ex_to[0], ex_to[1], 0.0],
            "at {time}"
        );
        assert_eq!(
            numbers(&objects["ey"]["arrow"]["to"]),
            [-ex_to[1], ex_to[0], 0.0],
            "a quarter turn ahead of ex at {time}"
        );
        assert_eq!(
            objects["d"]["dot"]["center"], objects["ey"]["arrow"]["to"],
            "at {time}"
        );
        for tip in [&objects["ex"]["arrow"]["to"], &objects["ey"]["arrow"]["to"]] {
            assert!(
                (on_mapped_unit_circle(map, tip) - 1.0).abs() < 1e-5,
                "{tip} lies on the mapped circle at {time}"
            );
        }
        let world = &objects["c"]["bounding_box"]["world"];
        assert!(
            near(&world["max"], &[radius + 0.02, radius + 0.02, 0.0], 0.005),
            "still a circle, of radius {radius}, at {time}: {world}"
        );
    }
}

#[test]
fn a_shear_or_a_stretch_keeps_a_stroke_at_its_declared_width() {
    let line = r##""l": { "type": "line", "from": [0, -1], "to": [0, 1], "stroke": { "color": "#ffffff", "width": 0.2 } }"##;
    let mapped = |matrix: &str| {
        let animation =
            format!(r#"{{ "type": "linear-map", "objects": ["l"], "matrix": {matrix} }}"#);
        inspect_at(&scene(line, &one_step(1.0, &animation)), "1")["objects"]["l"]["bounding_box"]["world"].clone()
    };

    let sheared = mapped("[[1, 1], [0, 1]]");
    assert_eq!(
        numbers(&sheared["max"]),
        [1.071, 1.071, 0.0],
        "the 45° segment's square ends, 1 + 0.1/√2; a sheared stroke would reach 1.1 in x"
    );
    assert_eq!(numbers(&sheared["min"]), [-1.071, -1.071, 0.0]);
    let stretched = mapped("[[3, 0], [0, 1]]");
    assert_eq!(
        numbers(&stretched["max"]),
        [0.1, 1.0, 0.0],
        "a stretch across the line does not widen it to 0.3"
    );
}

#[test]
fn a_reflection_flips_a_triangle_its_dot_and_its_arrow() {
    let objects = r##""t": { "type": "polygon", "points": [[0, 0], [1, 0], [0, 1]], "fill": { "color": "#ff0000" } },
      "d": { "type": "dot", "radius": 0.05, "position": [0.5, 0.8, 0], "fill": { "color": "#00ff00" } },
      "a": { "type": "arrow", "from": [0, 0], "to": [1, 1], "stroke": { "color": "#0000ff" } }"##;
    let scene = scene(
        objects,
        &one_step(
            1.0,
            r#"{ "type": "linear-map", "objects": ["t", "d", "a"], "matrix": [[1, 0], [0, -1]] }"#,
        ),
    );

    let report = inspect_at(&scene, "1");
    let (_dir, frame) = frame_at(&scene, "1");

    let objects = &report["objects"];
    assert_eq!(
        numbers(&objects["t"]["bounding_box"]["world"]["min"]),
        [0.0, -1.0, 0.0]
    );
    assert_eq!(
        numbers(&objects["t"]["bounding_box"]["world"]["max"]),
        [1.0, 0.0, 0.0]
    );
    assert_eq!(numbers(&objects["d"]["dot"]["center"]), [0.5, -0.8, 0.0]);
    assert_eq!(numbers(&objects["a"]["arrow"]["to"]), [1.0, -1.0, 0.0]);
    assert_eq!(objects["t"]["plane_map"]["determinant"], json!(-1.0));
    assert_eq!(
        pixel(&frame, 210, 219),
        RED,
        "world (0.2, -0.4), inside the flipped triangle"
    );
    assert_eq!(
        pixel(&frame, 210, 181),
        BLACK,
        "world (0.2, 0.4), where the triangle was"
    );
}

#[test]
fn a_rank_one_matrix_flattens_a_circle_flat_and_an_arrow_across_it_to_nothing() {
    let objects = r##""c": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff", "width": 0.04 }, "fill": { "color": "#0000ff" } },
      "ex": { "type": "arrow", "from": [0, 0], "to": [1, 0] },
      "ey": { "type": "arrow", "from": [0, 0], "to": [0, 1] },
      "d": { "type": "dot", "radius": 0.05, "position": [0, 1, 0], "fill": { "color": "#ff0000" } }"##;
    let animation = r#"{ "type": "linear-map", "objects": ["c", "ex", "ey", "d"], "matrix": [[2, 0], [0, 0]] }"#;

    let objects = &inspect_at(&scene(objects, &one_step(1.0, animation)), "1")["objects"];

    let circle = &objects["c"]["bounding_box"]["world"];
    assert_eq!(
        (numbers(&circle["min"]), numbers(&circle["max"])),
        (vec![-2.0, -0.02, 0.0], vec![2.0, 0.02, 0.0]),
        "the segment from -2 to 2 with square ends, its stroke still 0.04 wide; the fill has no area"
    );
    assert_eq!(numbers(&objects["ex"]["arrow"]["to"]), [2.0, 0.0, 0.0]);
    assert!(objects["ex"]["bounding_box"].is_object());
    assert_eq!(
        (
            numbers(&objects["ey"]["arrow"]["from"]),
            numbers(&objects["ey"]["arrow"]["to"])
        ),
        (vec![0.0, 0.0, 0.0], vec![0.0, 0.0, 0.0])
    );
    assert_eq!(
        objects["ey"]["bounding_box"],
        Value::Null,
        "an arrow whose ends meet draws nothing"
    );
    assert_eq!(objects["ey"]["in_frame"], "none");
    assert_eq!(numbers(&objects["d"]["dot"]["center"]), [0.0, 0.0, 0.0]);
    assert_eq!(
        numbers(&objects["d"]["bounding_box"]["world"]["max"]),
        [0.05, 0.05, 0.0],
        "the dot keeps its size"
    );
}

#[test]
fn a_rank_one_matrix_leaves_text_math_and_fills_nothing_to_draw_and_no_box() {
    let objects = r##""t": { "type": "text", "text": "Shear", "size": 0.8 },
      "turned": { "type": "text", "text": "Shear", "size": 0.8, "rotation": [0, 0, 30], "position": [0, 1.5, 0] },
      "m": { "type": "math", "tex": "e^{i\\pi}", "size": 0.8, "position": [0, -1.5, 0] },
      "tri": { "type": "polygon", "points": [[0, 0], [1, 0], [0, 1]], "fill": { "color": "#0000ff" } },
      "outlined": { "type": "polygon", "points": [[0, 0], [1, 0], [0, 1]], "fill": { "color": "#0000ff" }, "stroke": { "color": "#ffffff", "width": 0.04 } }"##;
    for matrix in ["[[1, 1], [1, 1]]", "[[1, 3], [0.5, 1.5]]"] {
        let animation = format!(
            r#"{{ "type": "linear-map", "objects": ["t", "turned", "m", "tri", "outlined"], "matrix": {matrix} }}"#
        );
        let objects = &inspect_at(&scene(objects, &one_step(1.0, &animation)), "1")["objects"];

        for id in ["t", "turned", "m", "tri"] {
            assert_eq!(
                (&objects[id]["bounding_box"], &objects[id]["in_frame"]),
                (&Value::Null, &json!("none")),
                "{id} under {matrix}"
            );
        }
        assert!(
            objects["outlined"]["bounding_box"].is_object(),
            "a stroke keeps its width under {matrix}"
        );
    }
}

#[test]
fn consecutive_maps_compose_on_the_left_about_their_own_origins() {
    let objects = r##""d": { "type": "dot", "radius": 0.05, "position": [3, 1, 0] }, "e": { "type": "dot", "radius": 0.05, "position": [1, 0, 0] }"##;
    let timeline = [
        one_step(1.0, r#"{ "type": "linear-map", "objects": ["d"], "matrix": [[1, 1], [0, 1]], "origin": [1, 0] }, { "type": "linear-map", "objects": ["e"], "matrix": [[0, -1], [1, 0]], "route": "turn" }"#),
        one_step(1.0, r#"{ "type": "linear-map", "objects": ["d"], "matrix": [[2, 0], [0, 2]], "origin": [0, 2] }, { "type": "linear-map", "objects": ["e"], "matrix": [[2, 0], [0, 1]] }"#),
    ]
    .join(", ");
    let scene = scene(objects, &timeline);

    let after_first = &inspect_at(&scene, "1")["objects"];
    let after_both = &inspect_at(&scene, "2")["objects"];

    assert_eq!(
        numbers(&after_first["d"]["dot"]["center"]),
        [4.0, 1.0, 0.0],
        "sheared about (1, 0)"
    );
    assert_eq!(
        numbers(&after_both["d"]["dot"]["center"]),
        [8.0, 0.0, 0.0],
        "then doubled about (0, 2)"
    );
    assert_eq!(
        rows(&after_both["d"]["plane_map"]["matrix"]),
        [[2.0, 2.0], [0.0, 2.0]]
    );
    assert_eq!(numbers(&after_both["d"]["plane_map"]["shift"]), [0.0, -2.0]);
    assert_eq!(
        numbers(&after_both["e"]["dot"]["center"]),
        [0.0, 1.0, 0.0],
        "B·A turns (1, 0) to (0, 1), which the x stretch leaves; A·B would give (0, 2)"
    );
    assert_eq!(
        rows(&after_both["e"]["plane_map"]["matrix"]),
        [[0.0, -2.0], [1.0, 0.0]]
    );
}

#[test]
fn the_zero_matrix_keeps_only_the_dot_and_never_reports_a_nan() {
    let objects = r##""c": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff" }, "fill": { "color": "#0000ff" } },
      "a": { "type": "arrow", "from": [0, 0], "to": [1, 0] },
      "t": { "type": "text", "text": "x" },
      "d": { "type": "dot", "radius": 0.05, "position": [1, 0, 0], "fill": { "color": "#ff0000" } }"##;
    let animation = r#"{ "type": "linear-map", "objects": ["c", "a", "t", "d"], "matrix": [[0, 0], [0, 0]], "origin": [0.5, 0.5], "easing_curve": "overshoot" }"#;
    let scene = scene(objects, &one_step(1.0, animation));

    for time in ["0", "0.25", "0.5", "0.75", "0.9", "1"] {
        let report = inspect_at(&scene, time);
        let mut found = Vec::new();
        nulls(&report, "", &mut found);
        assert!(
            found.iter().all(|at| at.ends_with("/bounding_box")),
            "only a box may be null at {time}: {found:?}"
        );
        frame_at(&scene, time);
    }
    let objects = &inspect_at(&scene, "1")["objects"];
    for id in ["c", "a", "t"] {
        assert_eq!(objects[id]["bounding_box"], Value::Null, "{id}");
        assert_eq!(objects[id]["in_frame"], "none", "{id}");
    }
    assert_eq!(numbers(&objects["a"]["arrow"]["to"]), [0.5, 0.5, 0.0]);
    assert_eq!(numbers(&objects["d"]["dot"]["center"]), [0.5, 0.5, 0.0]);
    assert_eq!(
        numbers(&objects["d"]["bounding_box"]["world"]["min"]),
        [0.45, 0.45, 0.0]
    );
    assert_eq!(
        objects["c"]["plane_map"],
        json!({ "matrix": [[0.0, 0.0], [0.0, 0.0]], "shift": [0.5, 0.5], "determinant": 0.0 })
    );
    let overshot = &inspect_at(&scene, "0.75")["objects"]["d"]["dot"]["center"];
    assert_eq!(
        numbers(overshot),
        [0.468, 0.532, 0.0],
        "progress 1.064 is -0.064 times the plane about (0.5, 0.5)"
    );
    let (_dir, frame) = frame_at(&scene, "1");
    assert_eq!(pixel(&frame, 224, 176), RED, "the dot at the map origin");
    assert_eq!(
        pixel(&frame, 248, 200),
        BLACK,
        "where the circle's right edge was"
    );
}

#[test]
fn a_matrix_near_zero_draws_nothing_but_the_dot() {
    let animation = r#"{ "type": "linear-map", "objects": ["c", "ex", "ey", "d"], "matrix": [[1e-9, 0], [0, 1e-9]] }"#;
    let scene = scene(CIRCLE_RADII_AND_DOT, &one_step(1.0, animation));

    let objects = &inspect_at(&scene, "1")["objects"];
    frame_at(&scene, "1");

    assert_eq!(objects["c"]["bounding_box"], Value::Null);
    assert_eq!(numbers(&objects["d"]["dot"]["center"]), [0.0, 0.0, 0.0]);
    assert_eq!(
        numbers(&objects["d"]["bounding_box"]["world"]["max"]),
        [0.06, 0.06, 0.0]
    );
}

#[test]
fn a_half_turn_passes_through_zero_by_entries_and_turns_counterclockwise_by_turn() {
    let halfway = |route: &str| {
        let animation = format!(
            r#"{{ "type": "linear-map", "objects": ["c", "ex", "ey", "d"], "matrix": [[-1, 0], [0, -1]], {route} "easing_curve": "linear" }}"#
        );
        inspect_at(
            &scene(CIRCLE_RADII_AND_DOT, &one_step(2.0, &animation)),
            "1",
        )["objects"]
            .clone()
    };

    let entries = halfway("");
    let turning = halfway(r#""route": "turn","#);

    assert_eq!(entries["c"]["bounding_box"], Value::Null, "the zero matrix");
    assert_eq!(
        numbers(&turning["d"]["dot"]["center"]),
        [-1.0, 0.0, 0.0],
        "(0, 1) a quarter turn on"
    );
    assert_eq!(numbers(&turning["ex"]["arrow"]["to"]), [0.0, 1.0, 0.0]);
    assert_eq!(
        numbers(&turning["c"]["bounding_box"]["world"]["max"]),
        [1.02, 1.02, 0.0]
    );
}

#[test]
fn a_half_turn_written_with_a_negative_zero_turns_counterclockwise() {
    for matrix in [
        "[[-1, 0], [-0, -1]]",
        "[[-1, -0.0], [0, -1]]",
        "[[-1, 0], [0, -1]]",
    ] {
        let animation = format!(
            r#"{{ "type": "linear-map", "objects": ["d"], "matrix": {matrix}, "route": "turn", "easing_curve": "linear" }}"#
        );
        let scene = scene(
            r##""d": { "type": "dot", "radius": 0.1, "position": [1, 0, 0] }"##,
            &one_step(2.0, &animation),
        );

        let objects = &inspect_at(&scene, "1")["objects"];

        assert_eq!(
            numbers(&objects["d"]["dot"]["center"]),
            [0.0, 1.0, 0.0],
            "(1, 0) a quarter turn on, with {matrix}"
        );
    }
}

#[test]
fn a_mapped_arrow_keeps_its_declared_tip_and_an_unmapped_one_is_scaled_as_before() {
    let objects = r##""a": { "type": "arrow", "from": [0, 0], "to": [2, 0], "stroke": { "color": "#ff0000" } },
      "sheared": { "type": "arrow", "from": [0, 0], "to": [0, 1], "tip_length": 0.2, "position": [0, -2, 0] },
      "declared": { "type": "arrow", "from": [0, 0], "to": [1, 1], "tip_length": 0.2, "position": [0, -2, 0] },
      "scaled": { "type": "arrow", "from": [0, 0], "to": [1, 0], "tip_length": 0.2, "scale": [1, 3, 1], "position": [-3, 0, 0] }"##;
    let animations = r#"{ "type": "linear-map", "objects": ["a"], "matrix": [[1, 0], [0, 3]] }, { "type": "linear-map", "objects": ["sheared"], "matrix": [[1, 1], [0, 1]], "origin": [0, -2] }"#;
    let scene = scene(objects, &one_step(1.0, animations));

    let objects = &inspect_at(&scene, "1")["objects"];
    let (_dir, frame) = frame_at(&scene, "1");

    assert_eq!(numbers(&objects["a"]["arrow"]["to"]), [2.0, 0.0, 0.0]);
    assert_eq!(
        pixel(&frame, 282, 195),
        RED,
        "world (1.7, 0.1), inside the 0.35 tip, whose half width there is 0.15"
    );
    assert_eq!(
        pixel(&frame, 282, 184),
        BLACK,
        "world (1.7, 0.33), which a tip stretched 3 times, 0.45 there, would cover"
    );
    assert_eq!(
        objects["sheared"]["bounding_box"]["world"], objects["declared"]["bounding_box"]["world"],
        "a sheared arrow is the arrow declared between its mapped ends"
    );
    assert!(
        objects["declared"].get("arrow").is_none()
            && objects["declared"].get("plane_map").is_none(),
        "an object no map names reports what it did"
    );
    let scaled = numbers(&objects["scaled"]["bounding_box"]["world"]["max"]);
    assert!(
        (0.31..=0.34).contains(&scaled[1]),
        "with no linear map, a scale still stretches the tip: {scaled:?}"
    );
}

#[test]
fn a_mapped_arrow_keeps_the_tip_its_own_scale_gives_it() {
    let objects = r##""mapped": { "type": "arrow", "from": [0, 0], "to": [1, 0], "tip_length": 0.35, "scale": [2, 2, 1], "position": [-2, 1, 0] },
      "plain": { "type": "arrow", "from": [0, 0], "to": [1, 0], "tip_length": 0.35, "scale": [2, 2, 1], "position": [-2, -1, 0] },
      "even": { "type": "arrow", "from": [0, 0], "to": [1, 0], "tip_length": 0.1, "scale": [2, 2, 1], "position": [0, 2, 0] },
      "uneven": { "type": "arrow", "from": [0, 0], "to": [2, 0], "tip_length": 0.1, "scale": [1, 4, 1], "position": [0, 2, 0] }"##;
    let scene = scene(
        objects,
        &one_step(
            1.0,
            r#"{ "type": "linear-map", "objects": ["mapped", "even", "uneven"], "matrix": [[1, 0], [0, 1]] }"#,
        ),
    );

    let objects = &inspect_at(&scene, "1")["objects"];

    let world = |id: &str, corner: &str| numbers(&objects[id]["bounding_box"]["world"][corner]);
    assert_eq!(
        (world("plain", "min"), world("plain", "max")),
        (vec![-2.0, -1.282, 0.0], vec![0.009, -0.718, 0.0]),
        "a tip 0.25 long, twice over"
    );
    assert_eq!(
        (world("mapped", "min"), world("mapped", "max")),
        (vec![-2.0, 0.718, 0.0], vec![0.009, 1.282, 0.0]),
        "the same tip under the identity"
    );
    assert_eq!(
        objects["uneven"]["bounding_box"], objects["even"]["bounding_box"],
        "a scale of 1 by 4 gives the tip their geometric mean, 2"
    );
}

#[test]
fn a_create_under_a_running_map_draws_mapped_points_of_the_share_it_has_drawn() {
    let objects = r##""ring": { "type": "circle", "radius": 1, "stroke": { "color": "#58c4dd", "width": 0.04 } }"##;
    let animations = r#"{ "type": "create", "object": "ring", "easing_curve": "linear" }, { "type": "linear-map", "objects": ["ring"], "matrix": [[1, 2], [0, 1]], "easing_curve": "linear" }"#;
    let scene = scene(objects, &one_step(2.0, animations));

    let (_dir, frame) = frame_at(&scene, "0.5");

    assert_ne!(
        pixel(&frame, 245, 158),
        BLACK,
        "the circle at 60°, (0.5, 0.866), sheared by [[1, 0.5], [0, 1]] to (0.933, 0.866)"
    );
    assert_eq!(
        pixel(&frame, 215, 152),
        BLACK,
        "the circle at 100°, past the quarter the create has drawn by its own length"
    );
}

#[test]
fn a_create_of_a_mapped_arrow_draws_along_the_arrow_drawn_between_its_mapped_ends() {
    let objects = r##""v": { "type": "arrow", "from": [-0.5, 0], "to": [0.5, 0], "tip_length": 0.2, "stroke": { "color": "#ff0000", "width": 0.06 } }"##;
    let timeline = [
        one_step(
            1.0,
            r#"{ "type": "linear-map", "objects": ["v"], "matrix": [[3, 0], [0, 3]] }"#,
        ),
        one_step(
            1.0,
            r#"{ "type": "create", "object": "v", "easing_curve": "linear" }"#,
        ),
    ];

    let (_dir, frame) = frame_at(&scene(objects, &timeline.join(", ")), "1.4");

    assert_eq!(
        pixel(&frame, 258, 200),
        RED,
        "world (1.2, 0): 0.8 of the stroke is 2.758 of the 3.447 the mapped arrow's shaft and tip run, inside its 2.8 shaft"
    );
    assert_eq!(
        pixel(&frame, 268, 200),
        BLACK,
        "world (1.4, 0), past where the shaft has reached, with no tip yet"
    );
}

#[test]
fn a_plot_is_placed_by_its_mapped_axes() {
    let objects = r##""ax": { "type": "axes", "x": { "range": [-1, 1] }, "y": { "range": [-1, 1] }, "width": 4, "height": 2 },
      "p": { "type": "function-plot", "axes": "ax", "expression": "x", "stroke": { "color": "#ffffff", "width": 0.04 } }"##;
    let scene = scene(
        objects,
        &one_step(
            1.0,
            r#"{ "type": "linear-map", "objects": ["ax"], "matrix": [[2, 0], [0, 1]] }"#,
        ),
    );

    let objects = &inspect_at(&scene, "1")["objects"];

    assert_eq!(
        objects["p"]["plane_map"],
        json!({ "matrix": [[2.0, 0.0], [0.0, 1.0]], "shift": [0.0, 0.0], "determinant": 2.0 })
    );
    assert_eq!(objects["p"]["plane_map"], objects["ax"]["plane_map"]);
    let world = &objects["p"]["bounding_box"]["world"];
    assert_eq!(
        numbers(&world["max"]),
        [4.005, 1.019, 0.0],
        "the line from (-4, -1) to (4, 1) with square ends of a 0.04 stroke"
    );
}

#[test]
fn a_rank_one_map_of_axes_and_math_draws_and_reports_finite_numbers() {
    let objects = r##""ax": { "type": "axes", "x": { "range": [-1, 1], "label": "x" }, "y": { "range": [-1, 1] }, "width": 4, "height": 2 },
      "p": { "type": "function-plot", "axes": "ax", "expression": "x^2" },
      "m": { "type": "math", "tex": "\\frac{a}{b}", "position": [0, 2, 0] }"##;
    let scene = scene(
        objects,
        &one_step(
            1.0,
            r#"{ "type": "linear-map", "objects": ["ax", "m"], "matrix": [[1, 1], [1, 1]] }"#,
        ),
    );

    for time in ["0", "0.5", "1"] {
        let report = inspect_at(&scene, time);
        let mut found = Vec::new();
        nulls(&report, "", &mut found);
        assert!(
            found.iter().all(|at| at.ends_with("/bounding_box")),
            "at {time}: {found:?}"
        );
        frame_at(&scene, time);
    }
    let objects = &inspect_at(&scene, "1")["objects"];
    assert_eq!(
        objects["p"]["plane_map"],
        json!({ "matrix": [[1.0, 1.0], [1.0, 1.0]], "shift": [0.0, 0.0], "determinant": 0.0 })
    );
    assert_eq!(objects["m"]["glyphs"], 3);
}

#[test]
fn a_caption_attached_to_a_mapped_dot_sits_beside_its_mapped_center() {
    let objects = r##""d": { "type": "dot", "radius": 0.05, "position": [1, 0, 0] },
      "cap": { "type": "text", "text": "P", "size": 0.5, "attach": { "object": "d", "offset": [0, 0.4] } }"##;
    let scene = scene(
        objects,
        &one_step(
            1.0,
            r#"{ "type": "linear-map", "objects": ["d"], "matrix": [[2, 0], [0, 1]] }"#,
        ),
    );

    let objects = &inspect_at(&scene, "1")["objects"];

    assert_eq!(numbers(&objects["d"]["dot"]["center"]), [2.0, 0.0, 0.0]);
    assert_eq!(
        numbers(&objects["cap"]["position"]),
        [1.986, 0.397, 0.071],
        "drawn just nearer than the dot's reach of 0.0707, as every caption is, from (2, 0)"
    );
    assert!(
        objects["cap"].get("plane_map").is_none(),
        "the caption sits beside the dot; no map names it"
    );
}

#[test]
fn a_fade_recolor_or_create_of_a_mapped_object_plays_with_the_map() {
    let objects = r##""c": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff" }, "fill": { "color": "#0000ff" }, "opacity": 0 }"##;
    let animations = r##"{ "type": "linear-map", "objects": ["c"], "matrix": [[1, 1], [0, 1]] }, { "type": "fade", "object": "c", "to": 1 },
      { "type": "recolor", "object": "c", "fill": "#ff0000" }, { "type": "create", "object": "c" }"##;

    assert_eq!(
        validate(&scene(objects, &one_step(1.0, animations))).exit_code,
        0
    );
}

#[test]
fn a_linear_map_refuses_what_it_cannot_carry_and_lists_every_error_at_once() {
    let objects = r##""c": { "type": "circle", "radius": 1 }, "orb": { "type": "sphere", "radius": 1 },
      "ax": { "type": "axes", "x": { "range": [-1, 1] }, "y": { "range": [-1, 1] }, "width": 4, "height": 2 },
      "p": { "type": "function-plot", "axes": "ax", "expression": "x" },
      "cap": { "type": "text", "text": "c", "attach": { "object": "c" } },
      "tilted": { "type": "circle", "radius": 1, "rotation": [30, 0, 0] }"##;
    let animation = r#"{ "type": "linear-map", "objects": ["orb", "p", "cap", "c", "c", "nope", "tilted"], "matrix": [[1, 0], [0, 1]] }"#;

    let run = validate(&scene(objects, &one_step(1.0, animation)));

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/0/animations/0/objects/0", "not_mappable"),
            ("/timeline/0/animations/0/objects/1", "not_mappable"),
            ("/timeline/0/animations/0/objects/2", "not_mappable"),
            ("/timeline/0/animations/0/objects/4", "duplicate_object"),
            ("/timeline/0/animations/0/objects/5", "unknown_object"),
        ]),
        "{}",
        run.stdout
    );
    assert!(
        run.json["errors"][1]["message"]
            .as_str()
            .unwrap()
            .contains("name `ax` instead"),
        "{}",
        run.stdout
    );
    assert_eq!(
        run.json["errors"][2]["message"],
        "`cap` is attached to `c` and sits beside it; name `c` instead, and `cap` stays beside it wherever the map carries it"
    );
    let tilted = validate(&scene(
        objects,
        &one_step(
            1.0,
            r#"{ "type": "linear-map", "objects": ["tilted"], "matrix": [[1, 0], [0, 1]] }"#,
        ),
    ));
    assert_eq!(
        scene_errors(&tilted),
        pairs(&[("/timeline/0/animations/0/objects/0", "not_mappable")])
    );
}

#[test]
fn a_map_refuses_an_object_turned_about_y_when_it_begins_and_not_one_turned_back_or_about_z() {
    let objects = r##""c": { "type": "circle", "radius": 1 }"##;
    let after = |to: &str| {
        let timeline = [
            one_step(
                1.0,
                &format!(r#"{{ "type": "rotate", "object": "c", "to": {to} }}"#),
            ),
            one_step(
                1.0,
                r#"{ "type": "linear-map", "objects": ["c"], "matrix": [[1, 1], [0, 1]] }"#,
            ),
        ]
        .join(", ");
        validate(&scene(objects, &timeline))
    };

    assert_eq!(
        scene_errors(&after("[0, 30, 0]")),
        pairs(&[("/timeline/1/animations/0/objects/0", "not_mappable")])
    );
    assert_eq!(after("[0, 0, 30]").exit_code, 0);
    let tilted_then_flat = [
        one_step(
            1.0,
            r#"{ "type": "rotate", "object": "c", "to": [30, 0, 0] }"#,
        ),
        one_step(
            1.0,
            r#"{ "type": "rotate", "object": "c", "to": [0, 0, 0] }"#,
        ),
        one_step(
            1.0,
            r#"{ "type": "linear-map", "objects": ["c"], "matrix": [[1, 1], [0, 1]] }"#,
        ),
    ];
    let run = validate(&scene(objects, &tilted_then_flat.join(", ")));
    assert_eq!(
        run.exit_code, 0,
        "flat again when the map begins: {}",
        run.stdout
    );
}

#[test]
fn a_move_scale_rotate_or_morph_of_a_mapped_object_is_refused_in_either_order() {
    let objects = r##""m": { "type": "polygon", "points": [[0, 0], [1, 0], [0, 1]], "stroke": { "color": "#ffffff" } }, "ci": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff" } }"##;
    let map = r#"{ "type": "linear-map", "objects": ["m"], "matrix": [[1, 1], [0, 1]] }"#;
    let after_map = |animation: &str| {
        scene_errors(&validate(&scene(
            objects,
            &[one_step(1.0, map), one_step(1.0, animation)].join(", "),
        )))
    };

    for animation in [
        r#"{ "type": "move", "object": "m", "to": [1, 0, 0] }"#,
        r#"{ "type": "scale", "object": "m", "to": [2, 2, 1] }"#,
        r#"{ "type": "rotate", "object": "m", "to": [0, 0, 90] }"#,
        r#"{ "type": "morph", "object": "m", "into": "ci" }"#,
    ] {
        assert_eq!(
            after_map(animation),
            pairs(&[("/timeline/1/animations/0/object", "mapped_object")]),
            "{animation}"
        );
    }
    assert_eq!(
        after_map(r#"{ "type": "morph", "object": "ci", "into": "m" }"#),
        pairs(&[("/timeline/1/animations/0/into", "mapped_object")])
    );
    let run = validate(&scene(
        objects,
        &[
            one_step(1.0, map),
            one_step(1.0, r#"{ "type": "move", "object": "m", "to": [1, 0, 0] }"#),
        ]
        .join(", "),
    ));
    assert!(
        run.json["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("the linear map at /timeline/0/animations/0"),
        "{}",
        run.stdout
    );
    let overlapping = one_step(
        2.0,
        r#"{ "type": "move", "object": "m", "to": [1, 0, 0], "end": 1 }, { "type": "linear-map", "objects": ["m"], "matrix": [[1, 1], [0, 1]], "start": 0.5 }"#,
    );
    assert_eq!(
        scene_errors(&validate(&scene(objects, &overlapping))),
        pairs(&[("/timeline/0/animations/1/objects/0", "mapped_object")])
    );
    let one_after_the_other = one_step(
        2.0,
        r#"{ "type": "move", "object": "m", "to": [1, 0, 0], "end": 1 }, { "type": "linear-map", "objects": ["m"], "matrix": [[1, 1], [0, 1]], "start": 1 }"#,
    );
    assert_eq!(
        validate(&scene(objects, &one_after_the_other)).exit_code,
        0,
        "a move that ends as the map begins"
    );
    let target_held = one_step(
        2.0,
        r#"{ "type": "morph", "object": "ci", "into": "m", "end": 1 }, { "type": "linear-map", "objects": ["m"], "matrix": [[1, 1], [0, 1]], "start": 0.5 }"#,
    );
    let run = validate(&scene(objects, &target_held));
    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/0/animations/1/objects/0", "mapped_object")])
    );
    assert!(run.json["errors"][0]["message"].as_str().unwrap().starts_with("the animation at /timeline/0/animations/0 holds or changes the transform of `m` until scene time 1,"), "a morph holds its target: {}", run.stdout);
}

#[test]
fn two_linear_maps_of_one_object_at_once_conflict() {
    let animations = r#"{ "type": "linear-map", "objects": ["c"], "matrix": [[2, 0], [0, 1]] }, { "type": "linear-map", "objects": ["c"], "matrix": [[1, 0], [0, 2]] }"#;

    let run = validate(&scene(
        r##""c": { "type": "circle", "radius": 1 }"##,
        &one_step(1.0, animations),
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/0/animations/1", "conflicting_animations")])
    );
}

#[test]
fn turn_is_refused_where_the_matrix_has_no_rotation_to_turn_by() {
    for (matrix, determinant) in [
        ("[[1, 0], [0, -1]]", "determinant -1"),
        ("[[2, 0], [0, 0]]", "determinant 0"),
        ("[[1, 1], [1, 1]]", "determinant 0"),
    ] {
        let animation = format!(
            r#"{{ "type": "linear-map", "objects": ["c"], "matrix": {matrix}, "route": "turn" }}"#
        );
        let run = validate(&scene(
            r##""c": { "type": "circle", "radius": 1 }"##,
            &one_step(1.0, &animation),
        ));
        assert_eq!(
            scene_errors(&run),
            pairs(&[("/timeline/0/animations/0/route", "no_turning_route")]),
            "{matrix}"
        );
        assert!(
            run.json["errors"][0]["message"]
                .as_str()
                .unwrap()
                .contains(determinant),
            "{}",
            run.stdout
        );
    }
}

#[test]
fn huge_entries_origins_and_composed_maps_are_out_of_range() {
    let c = r##""c": { "type": "circle", "radius": 1 }"##;
    let errors = |timeline: &str| scene_errors(&validate(&scene(c, timeline)));

    assert_eq!(
        errors(&one_step(
            1.0,
            r#"{ "type": "linear-map", "objects": ["c"], "matrix": [[1000, 0], [0, 1]] }"#
        )),
        pairs(&[("/timeline/0/animations/0/matrix/0/0", "out_of_range")])
    );
    assert_eq!(
        errors(&one_step(
            1.0,
            r#"{ "type": "linear-map", "objects": ["c"], "matrix": [[1, 0], [0, 1]], "origin": [100000, 0] }"#
        )),
        pairs(&[("/timeline/0/animations/0/origin/0", "out_of_range")])
    );
    assert_eq!(
        errors(&one_step(
            1.0,
            r#"{ "type": "linear-map", "objects": [], "matrix": [[1, 0], [0, 1]] }"#
        )),
        pairs(&[("/timeline/0/animations/0/objects", "wrong_length")])
    );
    let hundredfold = one_step(
        1.0,
        r#"{ "type": "linear-map", "objects": ["c"], "matrix": [[100, 0], [0, 1]] }"#,
    );
    assert_eq!(
        validate(&scene(
            c,
            &[hundredfold.clone(), hundredfold.clone()].join(", ")
        ))
        .exit_code,
        0,
        "1e4 is at the bound"
    );
    assert_eq!(
        errors(&[hundredfold.clone(), hundredfold.clone(), hundredfold].join(", ")),
        pairs(&[("/timeline/2/animations/0/matrix", "out_of_range")])
    );
    let far_origin = validate(&scene(
        c,
        &one_step(
            1.0,
            r#"{ "type": "linear-map", "objects": ["c"], "matrix": [[4, 0], [0, 1]], "origin": [5000, 0] }"#,
        ),
    ));
    assert_eq!(
        scene_errors(&far_origin),
        pairs(&[("/timeline/0/animations/0/matrix", "out_of_range")])
    );
    assert!(
        far_origin.json["errors"][0]["message"].as_str().unwrap().ends_with("shift [-15000, 0], past the 10000 a plane map may reach; use smaller entries, or a map origin nearer the objects and (0, 0), because the shift grows with the origin's distance from (0, 0)"),
        "{}",
        far_origin.stdout
    );
}

#[test]
fn a_dot_is_faded_in_not_created() {
    let run = validate(&scene(
        r##""d": { "type": "dot", "radius": 0.1 }"##,
        &one_step(1.0, r#"{ "type": "create", "object": "d" }"#),
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/0/animations/0/object", "no_stroke")])
    );
    assert!(
        run.json["errors"][0]["message"]
            .as_str()
            .unwrap()
            .ends_with("`d` is a dot, which has no stroke; fade it in instead"),
        "{}",
        run.stdout
    );
}

#[test]
fn an_unmapped_dot_is_a_white_disc_of_its_radius_with_its_center_reported() {
    let scene = scene(
        r##""d": { "type": "dot", "radius": 0.5, "position": [1, 0, 0] }"##,
        "",
    );

    let objects = &inspect_at(&scene, "0")["objects"];
    let (_dir, frame) = frame_at(&scene, "0");

    assert_eq!(numbers(&objects["d"]["dot"]["center"]), [1.0, 0.0, 0.0]);
    assert_eq!(
        numbers(&objects["d"]["bounding_box"]["world"]["max"]),
        [1.5, 0.5, 0.0]
    );
    assert!(objects["d"].get("plane_map").is_none());
    assert_eq!(pixel(&frame, 248, 200), [255, 255, 255, 255], "its center");
}
