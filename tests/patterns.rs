//! Patterns, read back from reports and frames as an agent reads them.
//!
//! The scenes are 400 by 400 pixels on black, with the camera at `[0, 0, 10]` looking at the
//! origin, so a unit of world at the origin is about 48 pixels.

mod common;

use common::{frame_at, inspect_at, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

fn rounded(value: &Value) -> Vec<f64> {
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

/// A pattern `p` of a small dot at every copy of `layout`, with `copy` as written.
fn dots(layout: &str, copy: &str) -> String {
    format!(
        r##""p": {{ "type": "pattern", "shape": {{ "type": "dot", "radius": 0.1 }}, "layout": {layout}, "copy": {copy} }}"##
    )
}

fn copies(inspected: &Value) -> &Vec<Value> {
    inspected["objects"]["p"]["pattern"]["copies"]
        .as_array()
        .unwrap()
}

fn positions(inspected: &Value) -> Vec<Vec<f64>> {
    copies(inspected)
        .iter()
        .map(|copy| rounded(&copy["position"]))
        .collect()
}

#[test]
fn a_grid_centers_its_copies_on_the_patterns_position() {
    let inspected = inspect_at(
        &scene(
            &dots(
                r#"{ "type": "grid", "counts": [3, 1, 1], "spacing": 1 }"#,
                "{}",
            ),
            "",
        ),
        "0",
    );

    assert_eq!(inspected["objects"]["p"]["pattern"]["copy_count"], 3);
    assert_eq!(
        positions(&inspected),
        [[-1.0, 0.0, 0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]]
    );
}

#[test]
fn a_grids_copies_run_i_fastest_then_j_then_k() {
    let inspected = inspect_at(
        &scene(
            &dots(
                r#"{ "type": "grid", "counts": [2, 2, 2], "spacing": 2 }"#,
                "{}",
            ),
            "",
        ),
        "0",
    );

    let indices: Vec<&Value> = copies(&inspected)
        .iter()
        .map(|copy| &copy["index"])
        .collect();
    let expected: Vec<Value> = [
        [0, 0, 0],
        [1, 0, 0],
        [0, 1, 0],
        [1, 1, 0],
        [0, 0, 1],
        [1, 0, 1],
        [0, 1, 1],
        [1, 1, 1],
    ]
    .iter()
    .map(|i| serde_json::json!(i))
    .collect();
    assert_eq!(indices, expected.iter().collect::<Vec<_>>());
    assert_eq!(positions(&inspected)[5], [1.0, -1.0, 1.0]);
}

#[test]
fn a_ring_places_copy_i_at_its_angle_from_the_start_angle() {
    let inspected = inspect_at(
        &scene(
            &dots(
                r#"{ "type": "ring", "count": 4, "radius": 2, "start_angle": 90 }"#,
                "{}",
            ),
            "",
        ),
        "0",
    );

    assert_eq!(
        positions(&inspected),
        [
            [0.0, 2.0, 0.0],
            [-2.0, 0.0, 0.0],
            [0.0, -2.0, 0.0],
            [2.0, 0.0, 0.0]
        ]
    );
    assert_eq!(copies(&inspected)[3]["index"], serde_json::json!([3]));
}

#[test]
fn copies_along_an_open_curve_span_it_and_along_a_closed_one_leave_a_gap_at_the_start() {
    let open = inspect_at(
        &scene(
            &dots(
                r#"{ "type": "along", "count": 3, "curve": { "x": "u^2", "y": "0", "z": "0", "u_range": [0, 2] } }"#,
                "{}",
            ),
            "",
        ),
        "0",
    );
    let closed = inspect_at(
        &scene(
            &dots(
                r#"{ "type": "along", "count": 4, "curve": { "x": "cos(u)", "y": "sin(u)", "z": "0", "u_range": [0, 6.2832] } }"#,
                "{}",
            ),
            "",
        ),
        "0",
    );

    assert_eq!(
        positions(&open),
        [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0]],
        "by length, not by u"
    );
    let closed: Vec<Vec<f64>> = positions(&closed)
        .iter()
        .map(|p| p.iter().map(|c| (c * 100.0).round() / 100.0).collect())
        .collect();
    assert_eq!(
        closed,
        [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0]
        ]
    );
}

/// Where a copy's own x and z axes point, from its `rotation` in degrees: about its x axis, then
/// its turned y, then its turned z.
fn axes(rotation: &Value) -> [[f64; 3]; 2] {
    let [a, b, c] = [0, 1, 2].map(|k| rotation[k].as_f64().unwrap().to_radians());
    let turn = |[x, y, z]: [f64; 3]| {
        let [x, y, z] = [x * c.cos() - y * c.sin(), x * c.sin() + y * c.cos(), z];
        let [x, y, z] = [x * b.cos() + z * b.sin(), y, -x * b.sin() + z * b.cos()];
        [x, y * a.cos() - z * a.sin(), y * a.sin() + z * a.cos()]
    };
    [turn([1.0, 0.0, 0.0]), turn([0.0, 0.0, 1.0])]
}

/// Whether `direction` points the way of the unit vector `unit`, within the 1 or 2 degrees a
/// curve's sampled tangent can be off at its ends.
fn along(direction: [f64; 3], unit: [f64; 3]) -> bool {
    let length = direction.iter().map(|d| d * d).sum::<f64>().sqrt();
    direction
        .iter()
        .zip(unit)
        .all(|(d, u)| (d / length - u).abs() < 0.03)
}

const FACING_RING: &str = r#"{ "type": "along", "count": 4, "faces": "tangent", "curve": { "x": "cos(u)", "y": "sin(u)", "z": "0", "u_range": [0, 6.2832] } }"#;

#[test]
fn copies_that_face_a_flat_curve_turn_their_x_axis_along_it_and_keep_z_on_z() {
    let inspected = inspect_at(&scene(&dots(FACING_RING, "{}"), ""), "0");

    for (i, copy) in copies(&inspected).iter().enumerate() {
        let angle = std::f64::consts::FRAC_PI_2 * i as f64;
        let [x_axis, z_axis] = axes(&copy["rotation"]);
        assert!(
            along(x_axis, [-angle.sin(), angle.cos(), 0.0]),
            "copy {i} faces along the circle: {}",
            copy["rotation"]
        );
        assert!(
            along(z_axis, [0.0, 0.0, 1.0]),
            "copy {i} keeps its z axis on z: {}",
            copy["rotation"]
        );
    }
}

#[test]
fn a_copys_own_rotation_turns_it_about_its_axes_turned_to_the_tangent() {
    let inspected = inspect_at(
        &scene(&dots(FACING_RING, r#"{ "rotation": { "x": "90" } }"#), ""),
        "0",
    );

    let [x_axis, z_axis] = axes(&copies(&inspected)[0]["rotation"]);
    assert!(
        along(x_axis, [0.0, 1.0, 0.0]),
        "still along the tangent at (1, 0, 0)"
    );
    assert!(
        along(z_axis, [1.0, 0.0, 0.0]),
        "a quarter turn about the tangent tips its z axis outward"
    );
}

#[test]
fn copies_that_face_a_helix_turn_their_x_axis_along_it() {
    let helix = r#"{ "type": "along", "count": 9, "faces": "tangent", "curve": { "x": "cos(u)", "y": "0.3*u", "z": "sin(u)", "u_range": [0, 12] } }"#;
    let inspected = inspect_at(&scene(&dots(helix, "{}"), ""), "0");

    for (i, copy) in copies(&inspected).iter().enumerate() {
        // A helix runs at one speed in u, so copy i is at u = 1.5*i.
        let u = 1.5 * i as f64;
        let tangent = [-u.sin(), 0.3, u.cos()];
        let [x_axis, _] = axes(&copy["rotation"]);
        assert!(
            along(tangent, x_axis),
            "copy {i}: x axis {x_axis:?}, tangent {tangent:?}"
        );
    }
}

#[test]
fn a_flat_copy_facing_a_vertical_curve_is_drawn_turned_upright() {
    let lines = |faces: &str| {
        format!(
            r##""p": {{ "type": "pattern", "shape": {{ "type": "line", "from": [-0.6, 0], "to": [0.6, 0], "stroke": {{ "color": "#ffffff", "width": 0.1 }} }},
                "layout": {{ "type": "along", "count": 3, {faces} "curve": {{ "x": "0", "y": "u", "z": "0", "u_range": [-2, 2] }} }} }}"##
        )
    };
    let (_dir, unturned) = frame_at(&scene(&lines(""), ""), "0");
    let (_dir, turned) = frame_at(&scene(&lines(r#""faces": "tangent","#), ""), "0");

    assert_eq!(pixel(&unturned, 220, 200), [255, 255, 255, 255]);
    assert_eq!(pixel(&unturned, 200, 215), [0, 0, 0, 255]);
    assert_eq!(
        pixel(&turned, 220, 200),
        [0, 0, 0, 255],
        "the line no longer runs across"
    );
    assert_eq!(
        pixel(&turned, 200, 215),
        [255, 255, 255, 255],
        "it runs up the curve"
    );
}

#[test]
fn copies_facing_a_curve_that_reads_law_time_turn_as_it_turns() {
    let spoke = r#"{ "type": "along", "count": 2, "faces": "tangent", "curve": { "x": "u*cos(t)", "y": "u*sin(t)", "z": "0", "u_range": [0, 2] } }"#;
    let follow = r#"{ "type": "play", "duration": 2, "animations": [ { "type": "follow", "object": "p", "sets": "shape" } ] }"#;
    let scene = scene(&dots(spoke, "{}"), follow);

    for (scene_time, degrees) in [("0", 0.0), ("1", 1f64.to_degrees())] {
        let inspected = inspect_at(&scene, scene_time);
        for copy in copies(&inspected) {
            let rotation = rounded(&copy["rotation"]);
            assert!(
                rotation[0].abs() < 1e-2
                    && rotation[1].abs() < 1e-2
                    && (rotation[2] - degrees).abs() < 1e-2,
                "at {scene_time}: {rotation:?}"
            );
        }
    }
}

#[test]
fn a_position_expression_in_x_lifts_a_row_into_a_wave() {
    let inspected = inspect_at(
        &scene(
            &dots(
                r#"{ "type": "grid", "counts": [5, 1, 1], "spacing": 1 }"#,
                r#"{ "position": { "y": "sin(x)" } }"#,
            ),
            "",
        ),
        "0",
    );

    let ys: Vec<f64> = positions(&inspected).iter().map(|p| p[1]).collect();
    assert_eq!(ys, [-0.909, -0.841, 0.0, 0.841, 0.909]);
}

/// Three filled circles 1 apart, mixed from red toward blue by `i/2`.
const RED_TO_BLUE: &str = r##""p": { "type": "pattern",
    "shape": { "type": "circle", "radius": 0.2, "stroke": { "color": "#ff0000" }, "fill": { "color": "#ff0000" } },
    "layout": { "type": "grid", "counts": [3, 1, 1], "spacing": 1 },
    "copy": { "color_share": "i/2", "to_color": "#0000ff" } }"##;

#[test]
fn a_color_share_mixes_each_copys_colors_toward_to_color_in_linear_light() {
    let inspected = inspect_at(&scene(RED_TO_BLUE, ""), "0");

    let strokes: Vec<&Value> = copies(&inspected)
        .iter()
        .map(|copy| &copy["colors"]["stroke"])
        .collect();
    assert_eq!(strokes, ["#ff0000", "#bc00bc", "#0000ff"]);
    assert_eq!(copies(&inspected)[1]["colors"]["fill"], "#bc00bc");
    assert_eq!(
        inspected["objects"]["p"]["colors"]["stroke"], "#ff0000",
        "the object's colors are its shape's"
    );
}

#[test]
fn each_copy_is_drawn_in_its_mixed_color_where_its_position_pixels_say() {
    let (_dir, rgba) = frame_at(&scene(RED_TO_BLUE, ""), "0");
    let inspected = inspect_at(&scene(RED_TO_BLUE, ""), "0");

    let at = |copy: &Value| {
        let [x, y] = [0, 1].map(|k| copy["position_pixels"][k].as_f64().unwrap().round() as usize);
        pixel(&rgba, x, y)
    };
    let copies = copies(&inspected);
    assert_eq!(at(&copies[0]), [255, 0, 0, 255]);
    assert_eq!(at(&copies[1]), [188, 0, 188, 255]);
    assert_eq!(at(&copies[2]), [0, 0, 255, 255]);
    assert_eq!(
        pixel(&rgba, 224, 200),
        [0, 0, 0, 255],
        "half way between two copies is background"
    );
}

#[test]
fn a_lattice_of_spheres_is_lit_at_each_copy_and_dark_between() {
    let lattice = r##""p": { "type": "pattern",
        "shape": { "type": "sphere", "radius": 0.3, "material": { "type": "standard", "color": "#ff0000" } },
        "layout": { "type": "grid", "counts": [2, 2, 1], "spacing": 2 } }"##;

    let (_dir, rgba) = frame_at(&scene(lattice, ""), "0");
    let inspected = inspect_at(&scene(lattice, ""), "0");

    for copy in copies(&inspected) {
        let [x, y] = [0, 1].map(|k| copy["position_pixels"][k].as_f64().unwrap().round() as usize);
        assert!(
            pixel(&rgba, x, y)[0] > 150,
            "a sphere at {}: {:?}",
            copy["index"],
            pixel(&rgba, x, y)
        );
    }
    assert_eq!(
        pixel(&rgba, 200, 200),
        [0, 0, 0, 255],
        "the middle of the lattice is empty"
    );
    let world = &copies(&inspected)[0]["bounding_box"]["world"];
    assert_eq!(
        (rounded(&world["min"]), rounded(&world["max"])),
        (vec![-1.3, -1.3, -0.3], vec![-0.7, -0.7, 0.3]),
        "the box of one copy"
    );
}

#[test]
fn a_rotate_of_the_pattern_turns_every_copy_about_its_position() {
    let pattern = dots(
        r#"{ "type": "grid", "counts": [3, 1, 1], "spacing": 1 }"#,
        "{}",
    )
    .replace(
        r#""type": "pattern","#,
        r#""type": "pattern", "position": [0, 1, 0],"#,
    );
    let scene = scene(
        &pattern,
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "rotate", "object": "p", "to": [0, 0, 90] }] }"#,
    );

    let inspected = inspect_at(&scene, "1");

    assert_eq!(
        positions(&inspected),
        [[0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 2.0, 0.0]]
    );
}

#[test]
fn a_create_grows_copies_in_index_order_and_the_frame_shows_only_those_begun() {
    let row = r##""p": { "type": "pattern",
        "shape": { "type": "circle", "radius": 0.15, "stroke": { "color": "#ffffff" }, "fill": { "color": "#ffffff" } },
        "layout": { "type": "grid", "counts": [10, 1, 1], "spacing": 0.4 } }"##;
    let scene = scene(
        row,
        r#"{ "type": "play", "duration": 2, "animations": [{ "type": "create", "object": "p", "easing_curve": "linear" }] }"#,
    );

    let inspected = inspect_at(&scene, "1");
    let (_dir, rgba) = frame_at(&scene, "1");

    let created: Vec<f64> = copies(&inspected)
        .iter()
        .map(|copy| (copy["created"].as_f64().unwrap() * 100.0).round() / 100.0)
        .collect();
    assert_eq!(created, [1.0, 1.0, 1.0, 0.8, 0.6, 0.4, 0.2, 0.0, 0.0, 0.0]);
    for (copy, created) in copies(&inspected).iter().zip(&created) {
        let [x, y] = [0, 1].map(|k| copy["position_pixels"][k].as_f64().unwrap().round() as usize);
        let lit = (x - 9..=x + 9).any(|x| (y - 9..=y + 9).any(|y| pixel(&rgba, x, y)[0] > 0));
        assert_eq!(
            lit,
            *created > 0.0,
            "copy {} at created share {created}",
            copy["index"]
        );
    }
    assert_eq!(
        rounded(&inspected["objects"]["p"]["bounding_box"]["world"]["max"])[0],
        1.97,
        "the whole pattern's box"
    );
}

#[test]
fn a_recolor_of_a_pattern_recolors_its_shape_and_each_copy_still_mixes() {
    let arrows = r##""p": { "type": "pattern",
        "shape": { "type": "arrow", "from": [0, 0], "to": [0.5, 0], "stroke": { "color": "#ff0000" } },
        "layout": { "type": "ring", "count": 2, "radius": 1 },
        "copy": { "color_share": "i", "to_color": "#0000ff" } }"##;
    let scene = scene(
        arrows,
        r##"{ "type": "play", "duration": 1, "animations": [{ "type": "recolor", "object": "p", "stroke": "#ffffff" }] }"##,
    );

    let inspected = inspect_at(&scene, "1");

    assert_eq!(inspected["objects"]["p"]["colors"]["stroke"], "#ffffff");
    assert_eq!(
        inspected["objects"]["p"]["colors"]["fill"], "#ffffff",
        "an arrow's tip goes with its stroke"
    );
    assert_eq!(copies(&inspected)[0]["colors"]["stroke"], "#ffffff");
    assert_eq!(copies(&inspected)[1]["colors"]["stroke"], "#0000ff");
}

#[test]
fn a_caption_attached_to_a_pattern_sits_beside_its_position() {
    let captioned = format!(
        r#"{}, "caption": {{ "type": "text", "text": "row", "size": 0.3, "attach": {{ "object": "p", "offset": [0, -1] }} }}"#,
        dots(
            r#"{ "type": "grid", "counts": [3, 1, 1], "spacing": 1 }"#,
            "{}"
        )
        .replace(
            r#""type": "pattern","#,
            r#""type": "pattern", "position": [1, 1, 0],"#
        )
    );

    let inspected = inspect_at(&scene(&captioned, ""), "0");

    let caption = &inspected["objects"]["caption"];
    assert_eq!(caption["attached"]["object"], "p");
    let (min, max) = (
        rounded(&caption["bounding_box"]["pixels"]["min"]),
        rounded(&caption["bounding_box"]["pixels"]["max"]),
    );
    let middle = rounded(&copies(&inspected)[1]["position_pixels"]);
    assert_eq!(
        rounded(&copies(&inspected)[1]["position"]),
        [1.0, 1.0, 0.0],
        "the middle copy sits at the pattern's position"
    );
    assert!(
        ((min[0] + max[0]) / 2.0 - middle[0]).abs() < 2.0,
        "centered under it: {min:?} {max:?} {middle:?}"
    );
    assert!(
        ((min[1] + max[1]) / 2.0 - middle[1] - 48.0).abs() < 5.0,
        "one unit below it: {min:?} {max:?} {middle:?}"
    );
}

#[test]
fn a_shape_a_pattern_does_not_draw_is_refused_at_its_type_and_its_file_is_not_read() {
    let run = validate(&scene(
        r#""p": { "type": "pattern", "shape": { "type": "molecule", "file": "missing.sdf" }, "layout": { "type": "ring", "count": 3, "radius": 1 } },
           "q": { "type": "pattern", "shape": { "type": "text", "text": "x" }, "layout": { "type": "ring", "count": 3, "radius": 1 } }"#,
        "",
    ));

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/p/shape/type", "invalid"),
            ("/objects/q/shape/type", "invalid")
        ])
    );
    let message = run.json["errors"][1]["message"].as_str().unwrap();
    assert!(
        message.contains("`tube` or `surface` at every copy")
            && message.contains("one string at every copy"),
        "{message}"
    );
}

#[test]
fn a_fault_inside_the_shape_is_reported_under_shape() {
    let run = validate(&scene(
        r#""p": { "type": "pattern", "shape": { "type": "sphere", "radius": -1, "position": [1, 0, 0] }, "layout": { "type": "ring", "count": 3, "radius": 1 } },
           "q": { "type": "pattern", "shape": { "type": "line", "from": [0, 0], "to": [0, 0] }, "layout": { "type": "ring", "count": 3, "radius": 1 } }"#,
        "",
    ));
    let read = validate(&scene(
        r#""q": { "type": "pattern", "shape": { "type": "line", "from": [0, 0], "to": [0, 0] }, "layout": { "type": "ring", "count": 3, "radius": 1 } }"#,
        "",
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/p/shape/position", "unknown_field"),
            ("/objects/p/shape/radius", "out_of_range")
        ])
    );
    assert_eq!(
        scene_errors(&read),
        pairs(&[("/objects/q/shape/to", "invalid")])
    );
}

#[test]
fn more_than_ten_thousand_copies_is_out_of_range_at_the_layout() {
    let run = validate(&scene(
        &format!(
            "{}, {}",
            dots(
                r#"{ "type": "grid", "counts": [100, 101, 1], "spacing": 1 }"#,
                "{}"
            ),
            dots(r#"{ "type": "ring", "count": 10001, "radius": 1 }"#, "{}")
                .replace(r#""p":"#, r#""q":"#)
        ),
        "",
    ));
    let grid = validate(&scene(
        &dots(
            r#"{ "type": "grid", "counts": [100, 101, 1], "spacing": 1 }"#,
            "{}",
        ),
        "",
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/q/layout/count", "out_of_range")]),
        "the schema reports a count"
    );
    assert_eq!(
        scene_errors(&grid),
        pairs(&[("/objects/p/layout/counts", "out_of_range")]),
        "reading reports a grid's product"
    );
    assert!(
        grid.json["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("10100 copies")
    );
}

#[test]
fn a_j_in_a_ring_is_an_unknown_name_at_its_column() {
    let run = validate(&scene(
        &dots(
            r#"{ "type": "ring", "count": 3, "radius": 1 }"#,
            r#"{ "rotation": { "z": "30*j" } }"#,
        ),
        "",
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/p/copy/rotation/z", "bad_format")])
    );
    let message = run.json["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("unknown name `j`") && message.contains("column 4"),
        "{message}"
    );
}

#[test]
fn an_expression_with_no_value_at_a_copy_names_its_first_such_copy() {
    let run = validate(&scene(
        &dots(
            r#"{ "type": "grid", "counts": [5, 1, 1], "spacing": 1 }"#,
            r#"{ "scale": { "x": "sqrt(i - 2)" } }"#,
        ),
        "",
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/p/copy/scale/x", "out_of_range")])
    );
    let message = run.json["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("NaN at copy [0, 0, 0], whose place is (-2, 0, 0)"),
        "{message}"
    );
}

#[test]
fn a_to_color_without_a_color_share_is_a_missing_field() {
    let run = validate(&scene(
        &dots(
            r#"{ "type": "ring", "count": 3, "radius": 1 }"#,
            r##"{ "to_color": "#ff0000" }"##,
        ),
        "",
    ));
    let other = validate(&scene(
        &dots(
            r#"{ "type": "ring", "count": 3, "radius": 1 }"#,
            r#"{ "color_share": "i/2" }"#,
        ),
        "",
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/p/copy/color_share", "missing_field")])
    );
    assert_eq!(
        scene_errors(&other),
        pairs(&[("/objects/p/copy/to_color", "missing_field")])
    );
}

#[test]
fn a_pattern_cannot_be_morphed_mapped_or_carried_along() {
    let objects = format!(
        r##"{}, "c": {{ "type": "circle", "radius": 1, "stroke": {{ "color": "#ffffff" }} }}"##,
        dots(r#"{ "type": "ring", "count": 3, "radius": 1 }"#, "{}")
    );
    let play = |animation: &str| {
        validate(&scene(
            &objects,
            &format!(r#"{{ "type": "play", "duration": 1, "animations": [{animation}] }}"#),
        ))
    };

    let morph = play(r#"{ "type": "morph", "object": "p", "into": "c" }"#);
    let map = play(r#"{ "type": "linear-map", "objects": ["p"], "matrix": [[1, 1], [0, 1]] }"#);
    let carry = play(r#"{ "type": "follow", "object": "c", "along": "p" }"#);

    assert_eq!(
        scene_errors(&morph),
        pairs(&[("/timeline/0/animations/0/object", "not_morphable")])
    );
    assert_eq!(
        scene_errors(&map),
        pairs(&[("/timeline/0/animations/0/objects/0", "not_mappable")])
    );
    assert_eq!(
        scene_errors(&carry),
        pairs(&[("/timeline/0/animations/0/along", "no_curve")])
    );
    assert!(
        carry.json["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("a pattern, whose copies are many curves")
    );
}

/// A pattern `p` of filled gray circles 1 apart in a row of `count`, with `copy` as written.
fn gray_circles(count: u32, copy: &str) -> String {
    format!(
        r##""p": {{ "type": "pattern",
    "shape": {{ "type": "circle", "radius": 0.2, "stroke": {{ "color": "#888888" }}, "fill": {{ "color": "#888888" }} }},
    "layout": {{ "type": "grid", "counts": [{count}, 1, 1], "spacing": 1 }},
    "copy": {copy} }}"##
    )
}

/// Adenine, thymine, guanine and cytosine.
const NUCLEOBASE_PALETTE: &str = r##"["#fc6255", "#ffd23f", "#83c167", "#58c4dd"]"##;

fn fills(inspected: &Value) -> Vec<&Value> {
    copies(inspected)
        .iter()
        .map(|copy| &copy["colors"]["fill"])
        .collect()
}

#[test]
fn a_listed_palette_index_colors_each_copy_by_its_entry() {
    let copy =
        format!(r#"{{ "palette": {NUCLEOBASE_PALETTE}, "palette_index": [0, 1, 2, 3, 2, 1, 0] }}"#);
    let inspected = inspect_at(&scene(&gray_circles(7, &copy), ""), "0");

    assert_eq!(
        fills(&inspected),
        [
            "#fc6255", "#ffd23f", "#83c167", "#58c4dd", "#83c167", "#ffd23f", "#fc6255"
        ]
    );
    assert_eq!(copies(&inspected)[3]["colors"]["stroke"], "#58c4dd");
}

#[test]
fn a_palette_index_expression_picks_by_the_copys_index() {
    let copy = r##"{ "palette": ["#ff0000", "#0000ff"], "palette_index": "mod(i, 2)" }"##;
    let inspected = inspect_at(&scene(&gray_circles(4, copy), ""), "0");

    assert_eq!(
        fills(&inspected),
        ["#ff0000", "#0000ff", "#ff0000", "#0000ff"]
    );
}

#[test]
fn a_color_share_mixes_each_copy_toward_its_pick_of_the_palette() {
    let copy = r##"{ "palette": ["#ff0000", "#0000ff"], "palette_index": [0, 1, 1], "color_share": "i/2" }"##;
    let inspected = inspect_at(&scene(&gray_circles(3, copy), ""), "0");

    assert_eq!(fills(&inspected), ["#888888", "#6262cf", "#0000ff"]);
}

#[test]
fn each_copy_is_drawn_in_its_pick_of_the_palette() {
    let copy = r##"{ "palette": ["#ff0000", "#00ff00", "#0000ff"], "palette_index": [2, 0, 1] }"##;
    let scene = scene(&gray_circles(3, copy), "");
    let (_dir, rgba) = frame_at(&scene, "0");
    let inspected = inspect_at(&scene, "0");

    let at = |copy: &Value| {
        let [x, y] = [0, 1].map(|k| copy["position_pixels"][k].as_f64().unwrap().round() as usize);
        pixel(&rgba, x, y)
    };
    let drawn: Vec<[u8; 4]> = copies(&inspected).iter().map(at).collect();
    assert_eq!(
        drawn,
        [[0, 0, 255, 255], [255, 0, 0, 255], [0, 255, 0, 255]]
    );
}

#[test]
fn a_listed_palette_index_past_the_palette_is_out_of_range_at_its_entry() {
    let run = validate(&scene(
        &gray_circles(
            3,
            r##"{ "palette": ["#ff0000", "#0000ff"], "palette_index": [0, 1, 2] }"##,
        ),
        "",
    ));

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/p/copy/palette_index/2", "out_of_range")])
    );
    let message = run.json["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("palette index 2 picks no color: the palette has 2 colors"),
        "{message}"
    );
}

#[test]
fn a_listed_palette_index_needs_one_entry_a_copy() {
    let run = validate(&scene(
        &gray_circles(
            3,
            r##"{ "palette": ["#ff0000", "#0000ff"], "palette_index": [0, 1] }"##,
        ),
        "",
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/p/copy/palette_index", "wrong_length")])
    );
    let message = run.json["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("lists 2 indices and the layout has 3 copies"),
        "{message}"
    );
}

#[test]
fn a_palette_index_expression_that_is_not_a_whole_number_within_the_palette_names_its_copy() {
    let fraction = validate(&scene(
        &gray_circles(
            3,
            r##"{ "palette": ["#ff0000", "#0000ff"], "palette_index": "i/2" }"##,
        ),
        "",
    ));
    let past = validate(&scene(
        &gray_circles(
            3,
            r##"{ "palette": ["#ff0000", "#0000ff"], "palette_index": "i" }"##,
        ),
        "",
    ));

    assert_eq!(
        scene_errors(&fraction),
        pairs(&[("/objects/p/copy/palette_index", "out_of_range")])
    );
    let message = fraction.json["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("`palette_index` is 0.5 at copy [1, 0, 0], whose place is (0, 0, 0), and every copy needs a whole number from 0 to 1"),
        "{message}"
    );
    let message = past.json["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("is 2 at copy [2, 0, 0]"), "{message}");
}

#[test]
fn a_bad_palette_or_palette_index_is_refused_at_its_path() {
    let bad_color = validate(&scene(
        &gray_circles(
            2,
            r##"{ "palette": ["#ff0000", "red"], "palette_index": [0, 1] }"##,
        ),
        "",
    ));
    let empty = validate(&scene(
        &gray_circles(2, r#"{ "palette": [], "palette_index": [0, 0] }"#),
        "",
    ));
    let negative = validate(&scene(
        &gray_circles(
            2,
            r##"{ "palette": ["#ff0000"], "palette_index": [0, -1] }"##,
        ),
        "",
    ));
    let unknown_name = validate(&scene(
        &gray_circles(2, r##"{ "palette": ["#ff0000"], "palette_index": "n" }"##),
        "",
    ));

    assert_eq!(bad_color.exit_code, 1);
    assert_eq!(
        scene_errors(&bad_color),
        pairs(&[("/objects/p/copy/palette/1", "bad_format")])
    );
    assert_eq!(
        scene_errors(&empty),
        pairs(&[("/objects/p/copy/palette", "wrong_length")])
    );
    assert_eq!(
        scene_errors(&negative),
        pairs(&[("/objects/p/copy/palette_index/1", "out_of_range")])
    );
    assert_eq!(
        scene_errors(&unknown_name),
        pairs(&[("/objects/p/copy/palette_index", "bad_format")])
    );
}

#[test]
fn a_palette_goes_with_a_palette_index_and_not_with_to_color() {
    let both = validate(&scene(
        &gray_circles(
            2,
            r##"{ "palette": ["#ff0000"], "palette_index": [0, 0], "to_color": "#0000ff", "color_share": "1" }"##,
        ),
        "",
    ));
    let no_index = validate(&scene(
        &gray_circles(2, r##"{ "palette": ["#ff0000"] }"##),
        "",
    ));
    let no_palette = validate(&scene(
        &gray_circles(2, r#"{ "palette_index": [0, 0] }"#),
        "",
    ));

    assert_eq!(
        scene_errors(&both),
        pairs(&[("/objects/p/copy/palette", "invalid")])
    );
    assert_eq!(
        scene_errors(&no_index),
        pairs(&[("/objects/p/copy/palette_index", "missing_field")])
    );
    assert_eq!(
        scene_errors(&no_palette),
        pairs(&[("/objects/p/copy/palette", "missing_field")])
    );
}
