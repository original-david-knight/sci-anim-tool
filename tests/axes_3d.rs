//! 3D axes, read back from frames and reports as an agent reads them.
//!
//! The scenes are 400 by 400 pixels on black, with the camera at `[0, 0, 10]` looking at the
//! origin unless a scene sets its own.

mod common;

use common::{frame_at, inspect_at, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
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

/// 3D axes `a` with x from -3 to 3 over 6, y from 0 to 4 over 2 and z from 0 to 4 over 4, each
/// labeled, with `more` fields.
fn axes(more: &str) -> String {
    format!(
        r##""a": {{ "type": "3d-axes", {more} "x": {{ "range": [-3, 3], "length": 6, "label": "x" }}, "y": {{ "range": [0, 4], "length": 2, "label": "y" }}, "z": {{ "range": [0, 4], "length": 4, "label": "z" }} }}"##
    )
}

/// `objects` seen from a camera at `position`, looking at the origin.
fn seen_from(position: &str, objects: &str, timeline: &str) -> String {
    scene(objects, timeline).replacen(
        r#""objects""#,
        &format!(r#""camera": {{ "position": {position}, "target": [0, 0, 0] }}, "objects""#),
        1,
    )
}

fn axes_report(inspected: &Value) -> &Value {
    &inspected["objects"]["a"]["axes_3d"]
}

/// The tick of `axis` at `value`.
fn tick<'a>(report: &'a Value, axis: &str, value: f64) -> &'a Value {
    report["ticks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["axis"] == axis && t["value"] == value)
        .unwrap_or_else(|| panic!("no {axis} tick at {value}"))
}

#[test]
fn data_lands_with_x_right_z_up_and_y_away_each_range_filling_its_length() {
    let report = inspect_at(&scene(&axes(""), ""), "0");
    let report = axes_report(&report);

    assert_eq!(
        numbers(&report["zero"]["world"]),
        [0.0, -2.0, 1.0],
        "the box is centered on the position, so data 0 of y and z lies at the near, low corner"
    );
    assert_eq!(numbers(&report["x_unit"]), [1.0, 0.0, 0.0]);
    assert_eq!(
        numbers(&report["y_unit"]),
        [0.0, 0.0, -0.5],
        "y runs away from the default camera, 4 data units over 2"
    );
    assert_eq!(numbers(&report["z_unit"]), [0.0, 1.0, 0.0]);
    assert_eq!(
        numbers(&tick(report, "y", 4.0)["world"]),
        [0.0, -2.0, -1.0],
        "each axis crosses the others at 0, or the nearest end of their range"
    );
    assert_eq!(numbers(&tick(report, "z", 3.0)["world"]), [0.0, 1.0, 1.0]);
    assert_eq!(report["z"]["range"], serde_json::json!([0.0, 4.0]));
    assert_eq!(report["z"]["spacing"], 1.0);
}

#[test]
fn tick_labels_sit_below_x_and_y_and_left_of_z_and_none_where_the_axes_cross() {
    let report = inspect_at(&seen_from("[6, 5, 10]", &axes(""), ""), "0");
    let report = axes_report(&report);

    let x2 = tick(report, "x", 2.0);
    assert_eq!(
        (x2["label_text"].as_str(), x2["label_side"].as_str()),
        (Some("2"), Some("below"))
    );
    assert_eq!(
        (
            tick(report, "y", 2.0)["label_side"].as_str(),
            tick(report, "z", 2.0)["label_side"].as_str()
        ),
        (Some("below"), Some("left"))
    );
    let below = &x2["label_bounding_box"]["world"];
    assert!(
        below["max"][1].as_f64().unwrap() < -2.08,
        "under the tick, which reaches 0.08 below the axis: {below}"
    );
    for (axis, value) in [("x", 0.0), ("y", 0.0), ("z", 0.0)] {
        assert!(
            tick(report, axis, value).get("label_text").is_none(),
            "the {axis} tick at {value} is where the axes cross: {}",
            tick(report, axis, value)
        );
    }
    let z_label = &report["z"]["label_bounding_box"]["world"];
    assert!(
        z_label["min"][1].as_f64().unwrap() > 2.1,
        "the z label sits above the top of z: {z_label}"
    );
}

#[test]
fn a_label_faces_the_camera_wherever_it_stands() {
    let from_front = inspect_at(&scene(&axes(""), ""), "0");
    let from_side = inspect_at(&seen_from("[10, 0, 0]", &axes(""), ""), "0");

    let width = |report: &Value| {
        let world = &tick(axes_report(report), "x", 2.0)["label_bounding_box"]["world"];
        [0, 2].map(|k| {
            ((world["max"][k].as_f64().unwrap() - world["min"][k].as_f64().unwrap()) * 1000.0)
                .round()
                / 1000.0
        })
    };
    let (front, side) = (width(&from_front), width(&from_side));
    assert!(
        front[0] > 0.1 && front[1] == 0.0,
        "from the front it spans x: {front:?}"
    );
    assert!(
        side[0] == 0.0 && side[1] > 0.1 && (side[1] - front[0]).abs() < 1e-3,
        "from +x it spans z as wide: {side:?}"
    );
}

#[test]
fn a_solid_in_front_hides_a_label() {
    let bare = scene(&axes(""), "");
    let inspected = inspect_at(&bare, "0");
    let label = &tick(axes_report(&inspected), "x", 2.0)["label_bounding_box"];
    let middle = |of: &str, k: usize| {
        (label[of]["min"][k].as_f64().unwrap() + label[of]["max"][k].as_f64().unwrap()) / 2.0
    };
    // Halfway along the line from the camera at [0, 0, 10] to the label's middle.
    let between = [
        middle("world", 0) / 2.0,
        middle("world", 1) / 2.0,
        (middle("world", 2) + 10.0) / 2.0,
    ];
    let covered = scene(
        &format!(
            r##"{}, "s": {{ "type": "sphere", "radius": 0.3, "position": {between:?}, "material": {{ "type": "standard", "color": "#ff0000" }} }}"##,
            axes("")
        ),
        "",
    );

    let (x, y) = (middle("pixels", 0) as usize, middle("pixels", 1) as usize);
    let (_a, bare_frame) = frame_at(&bare, "0");
    let (_b, covered_frame) = frame_at(&covered, "0");
    let white = |rgba: &[u8]| {
        (y - 4..=y + 4)
            .flat_map(|row| (x - 4..=x + 4).map(move |column| (column, row)))
            .filter(|&(c, r)| pixel(rgba, c, r)[1] > 128)
            .count()
    };
    assert!(
        white(&bare_frame) > 3,
        "the 2 shows white around ({x}, {y})"
    );
    assert_eq!(
        white(&covered_frame),
        0,
        "a red sphere nearer the camera hides it"
    );
}

#[test]
fn a_create_draws_the_lines_and_ticks_then_fades_the_labels_in() {
    let created = scene(
        &axes(""),
        r#"{ "type": "play", "duration": 2, "animations": [{ "type": "create", "object": "a", "easing_curve": "linear" }] }"#,
    );

    let inspected = inspect_at(&created, "1");
    let report = axes_report(&inspected);
    let pixels = &tick(report, "x", 2.0)["label_bounding_box"]["pixels"];
    let (x, y) = (
        ((pixels["min"][0].as_f64().unwrap() + pixels["max"][0].as_f64().unwrap()) / 2.0) as usize,
        ((pixels["min"][1].as_f64().unwrap() + pixels["max"][1].as_f64().unwrap()) / 2.0) as usize,
    );
    let lit = |rgba: &[u8]| {
        (y - 4..=y + 4)
            .flat_map(|row| (x - 4..=x + 4).map(move |column| (column, row)))
            .filter(|&(c, r)| pixel(rgba, c, r)[1] > 128)
            .count()
    };
    let (_a, start) = frame_at(&created, "0");
    let (_b, half) = frame_at(&created, "1");
    let (_c, end) = frame_at(&created, "2");

    assert_eq!(inspected["objects"]["a"]["created"], 0.5);
    assert!(
        start.chunks(4).all(|p| p[1] == 0),
        "nothing before the create"
    );
    let x_axis_row = tick(report, "x", -3.0)["pixels"][1].as_f64().unwrap() as usize;
    assert!(
        pixel(&half, 200, x_axis_row)[1] > 128,
        "the lines are drawn halfway through"
    );
    assert_eq!(lit(&half), 0, "the labels have not begun to fade in");
    assert!(lit(&end) > 3, "the labels show at the end");
}

#[test]
fn the_axes_move_scale_and_turn_as_one_object() {
    let moved = scene(
        &axes(""),
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "move", "object": "a", "to": [1, 0, 0] }, { "type": "scale", "object": "a", "to": [2, 2, 2] }] }"#,
    );

    let report = inspect_at(&moved, "1");
    let report = axes_report(&report);
    assert_eq!(numbers(&report["zero"]["world"]), [1.0, -4.0, 2.0]);
    assert_eq!(numbers(&report["x_unit"]), [2.0, 0.0, 0.0]);
}

#[test]
fn each_mistake_in_an_axis_is_a_scene_error_at_its_field() {
    let run = validate(&scene(
        r##""a": { "type": "3d-axes", "x": { "range": [1, -1], "length": 2 }, "y": { "range": [0, 1], "length": 2 }, "z": { "range": [0, 1], "length": 2 } }"##,
        "",
    ));

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/a/x/range", "invalid")])
    );
    let label = validate(&scene(
        r##""a": { "type": "3d-axes", "x": { "range": [0, 1], "length": 2 }, "y": { "range": [0, 1], "length": 2 }, "z": { "range": [0, 1], "length": 2, "label": "\\boxed{z}" } }"##,
        "",
    ));
    assert_eq!(
        scene_errors(&label),
        pairs(&[("/objects/a/z/label", "invalid")])
    );
    let ticks = validate(&scene(
        r##""a": { "type": "3d-axes", "x": { "range": [0, 1], "length": 2 }, "y": { "range": [0, 1], "length": 2 }, "z": { "range": [0, 1], "spacing": 0.001, "length": 2 } }"##,
        "",
    ));
    assert_eq!(
        scene_errors(&ticks),
        pairs(&[("/objects/a/z/spacing", "invalid")])
    );
    let missing = validate(&scene(
        r##""a": { "type": "3d-axes", "x": { "range": [0, 1] }, "y": { "range": [0, 1], "length": 2 } }"##,
        "",
    ));
    assert_eq!(
        scene_errors(&missing),
        pairs(&[
            ("/objects/a/x/length", "missing_field"),
            ("/objects/a/z", "missing_field")
        ])
    );
}

#[test]
fn axes_3d_cannot_be_morphed_mapped_recolored_or_carried_along() {
    let objects = format!(
        r##"{}, "c": {{ "type": "circle", "radius": 1, "stroke": {{ "color": "#ffffff" }} }}, "d": {{ "type": "dot", "radius": 0.1 }}"##,
        axes("")
    );
    let run = validate(&scene(
        &objects,
        r##"{ "type": "play", "duration": 1, "animations": [{ "type": "morph", "object": "a", "into": "c" }] },
            { "type": "play", "duration": 1, "animations": [{ "type": "linear-map", "objects": ["a"], "matrix": [[2, 0], [0, 1]] }] },
            { "type": "play", "duration": 1, "animations": [{ "type": "recolor", "object": "a", "color": "#ff0000" }] },
            { "type": "play", "duration": 1, "animations": [{ "type": "follow", "object": "d", "along": "a" }] }"##,
    ));

    let codes: Vec<String> = scene_errors(&run)
        .into_iter()
        .map(|(_, code)| code)
        .collect();
    assert_eq!(
        codes,
        [
            "not_morphable",
            "not_mappable",
            "not_recolorable",
            "no_curve"
        ],
        "{}",
        run.stdout
    );
    let messages = messages(&run);
    assert!(messages.contains(&"`a` is 3D axes, which stand in space, not in the world's xy plane; move, scale or rotate them instead".to_string()), "{messages:?}");
    assert!(messages.contains(&"`a` is 3D axes, which has no one curve to carry along; carry along a flat shape, a plot or a tube".to_string()), "{messages:?}");
}

#[test]
fn a_three_d_axes_type_names_the_3d_axes_to_write() {
    let run = validate(&scene(r##""a": { "type": "three-d-axes" }"##, ""));

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/a/type", "unknown_value")])
    );
    assert!(messages(&run)[0].ends_with("; three axes with z up are `3d-axes`, whose `x`, `y` and `z` each give a `range` and a `length`"), "{:?}", messages(&run));
}

/// 3D axes `a` with x and y from -3 to 3 over 6 and z from `z_low` to `z_high` over 4, and
/// `objects` beside them.
fn on_axes(z_range: &str, objects: &str) -> String {
    format!(
        r##""a": {{ "type": "3d-axes", "x": {{ "range": [-3, 3], "length": 6 }}, "y": {{ "range": [-3, 3], "length": 6 }}, "z": {{ "range": {z_range}, "length": 4 }} }}, {objects}"##
    )
}

fn world_box(inspected: &Value, id: &str) -> (Vec<f64>, Vec<f64>) {
    let world = &inspected["objects"][id]["bounding_box"]["world"];
    (numbers(&world["min"]), numbers(&world["max"]))
}

#[test]
fn a_surface_on_3d_axes_is_written_in_their_data_and_cut_at_their_z_range() {
    let bowl = on_axes(
        "[0, 8]",
        r##""s": { "type": "surface", "axes": "a", "z": "x^2 + y^2" }"##,
    );

    let inspected = inspect_at(&scene(&bowl, ""), "0");

    // The ranges are the axes', and the bowl, which reaches 18 at the corners, stops at z = 8,
    // the top of the box, 2 above its middle, where x^2 + y^2 = 8.
    assert_eq!(
        world_box(&inspected, "s"),
        (vec![-2.828, -2.0, -2.828], vec![2.828, 2.0, 2.828])
    );
}

#[test]
fn a_surface_s_ranges_are_cut_to_the_axes_and_a_pole_stops_at_the_box() {
    let wide = on_axes(
        "[0, 4]",
        r##""s": { "type": "surface", "axes": "a", "z": "1/(x^2 + y^2)", "x_range": [-10, 1], "y_range": [0, 10] }"##,
    );

    assert_eq!(
        validate(&scene(&wide, "")).exit_code,
        0,
        "a pole that 1/0 makes infinite is cut, not refused"
    );
    let inspected = inspect_at(&scene(&wide, ""), "0");
    let (min, max) = world_box(&inspected, "s");
    assert_eq!(
        (min[0], max[0]),
        (-3.0, 1.0),
        "x from -10 to 1 is cut to the axes' -3"
    );
    assert_eq!(
        (min[2], max[2]),
        (-3.0, 0.0),
        "y from 0 to 10 is cut to 3, which lies 3 away from the camera"
    );
    assert_eq!(max[1], 2.0, "the pole stops at the top of the box");
}

#[test]
fn a_create_rises_a_surface_on_3d_axes_from_data_z_of_0_or_the_nearest_end() {
    // Seen level from the front, a flat sheet is a line at its height.
    let line_row = |z_range: &str, scene_time: &str| {
        let flat = on_axes(
            z_range,
            r##""s": { "type": "surface", "axes": "a", "z": "2", "material": { "type": "standard", "color": "#ff0000" } }"##,
        );
        let created = scene(
            &flat,
            r#"{ "type": "play", "duration": 2, "animations": [{ "type": "create", "object": "s", "easing_curve": "linear" }] }"#,
        );
        let (_dir, rgba) = frame_at(&created, scene_time);
        let rows: Vec<usize> = (0..400)
            .filter(|&y| pixel(&rgba, 200, y)[0] > 40 && pixel(&rgba, 200, y)[1] < 30)
            .collect();
        (rows.iter().sum::<usize>() as f64 / rows.len() as f64).round()
    };

    let (half, whole) = (line_row("[-4, 4]", "1"), line_row("[-4, 4]", "2"));
    assert!(
        (200.0 - half - (200.0 - whole) / 2.0).abs() <= 1.5,
        "from z = 0 at the box's middle, halfway is z = 1: row {half} against {whole}"
    );
    // With z from 1 to 5 the box's middle is z = 3, and the sheet at z = 2 lies 1 below it; risen
    // halfway from z = 1, it lies at 1.5, half again as far below.
    let (half, whole) = (line_row("[1, 5]", "1"), line_row("[1, 5]", "2"));
    assert!(
        (half - 200.0 - 1.5 * (whole - 200.0)).abs() <= 1.5,
        "from z = 1, the low end: row {half} against {whole}"
    );
}

#[test]
fn a_tube_on_3d_axes_is_written_in_their_data_and_cut_into_pieces_at_their_box() {
    let rod = on_axes(
        "[0, 8]",
        r##""t": { "type": "tube", "axes": "a", "radius": 0.1, "curve": { "x": "u", "y": "0", "z": "4", "u_range": [-5, 5] } }"##,
    );
    let wave = on_axes(
        "[0, 8]",
        r##""t": { "type": "tube", "axes": "a", "radius": 0.1, "curve": { "x": "u", "y": "0", "z": "4 + 6*sin(u)", "u_range": [-3, 3] }, "material": { "type": "standard", "color": "#ff0000" } }"##,
    );

    let inspected = inspect_at(&scene(&rod, ""), "0");
    assert_eq!(
        world_box(&inspected, "t"),
        (vec![-3.0, -0.1, -0.1], vec![3.0, 0.1, 0.1]),
        "a rod from x = -5 to 5 is cut at -3 and 3, at z = 4, the box's middle"
    );
    assert_eq!(
        numbers(&inspected["objects"]["t"]["tube"]["end"]),
        [5.0, 0.0, 0.0],
        "the curve's own end lies past the box"
    );
    let (_dir, rgba) = frame_at(&seen_from("[0, 0, 12]", &wave, ""), "0");
    let lit_columns: Vec<usize> = (0..400)
        .filter(|&x| (0..400).any(|y| pixel(&rgba, x, y)[0] > 100 && pixel(&rgba, x, y)[1] < 60))
        .collect();
    let pieces = lit_columns.windows(2).filter(|w| w[1] > w[0] + 1).count() + 1;
    assert_eq!(
        pieces, 3,
        "the wave leaves the box over its crests and troughs, so it shows in three pieces: {lit_columns:?}"
    );
}

#[test]
fn a_surface_or_tube_on_3d_axes_goes_with_them_and_has_no_transform_of_its_own() {
    let bowl = on_axes(
        "[0, 8]",
        r##""s": { "type": "surface", "axes": "a", "z": "x^2 + y^2" }"##,
    );
    let moved = scene(
        &bowl,
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "move", "object": "a", "to": [1, 0, 0] }] }"#,
    );

    assert_eq!(
        world_box(&inspect_at(&moved, "1"), "s"),
        (vec![-1.828, -2.0, -2.828], vec![3.828, 2.0, 2.828])
    );
    let placed = validate(&scene(
        &on_axes(
            "[0, 8]",
            r##""s": { "type": "surface", "axes": "a", "z": "0", "position": [1, 0, 0] }"##,
        ),
        "",
    ));
    assert_eq!(
        scene_errors(&placed),
        pairs(&[("/objects/s/position", "placed_by_axes")])
    );
    assert_eq!(
        messages(&placed)[0],
        "a surface on 3D axes is placed by its axes and has no position, rotation or scale of its own; place the axes instead"
    );
    let rotated = validate(&scene(
        &bowl,
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "rotate", "object": "s", "to": [0, 90, 0] }] }"#,
    ));
    assert_eq!(
        scene_errors(&rotated),
        pairs(&[("/timeline/0/animations/0/object", "placed_by_axes")])
    );
    assert_eq!(
        messages(&rotated)[0],
        "`s` is a surface on 3D axes, which is placed by its axes `a`, so move, scale, rotate or follow `a` instead"
    );
}

#[test]
fn naming_the_wrong_axes_is_invalid_at_axes() {
    let objects = r##""flat": { "type": "axes", "x": { "range": [0, 1] }, "y": { "range": [0, 1] }, "width": 2, "height": 2 },
        "s": { "type": "surface", "axes": "flat", "z": "0" },
        "p": { "type": "function-plot", "axes": "a", "expression": "x" },
        "t": { "type": "tube", "axes": "nowhere", "radius": 0.1, "curve": { "x": "u", "y": "0", "z": "0", "u_range": [0, 1] } }"##;
    let run = validate(&scene(&on_axes("[0, 8]", objects), ""));

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/p/axes", "invalid"),
            ("/objects/s/axes", "invalid"),
            ("/objects/t/axes", "unknown_object")
        ])
    );
    let messages = messages(&run);
    assert!(
        messages[0].starts_with("`a` is 3D axes, and a plot is drawn on 2D `axes`"),
        "{messages:?}"
    );
    assert!(
        messages[1].starts_with("`flat` is 2D axes, which hold plots"),
        "{messages:?}"
    );
    assert_eq!(
        messages[2],
        "no 3D axes object has the id `nowhere`; the 3D axes are `a`"
    );
}

#[test]
fn a_surface_off_axes_needs_its_ranges_and_a_pattern_s_shape_names_no_axes() {
    let run = validate(&scene(r##""s": { "type": "surface", "z": "0" }"##, ""));
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/s/x_range", "missing_field"),
            ("/objects/s/y_range", "missing_field")
        ])
    );

    let pattern = on_axes(
        "[0, 8]",
        r##""p": { "type": "pattern", "shape": { "type": "surface", "axes": "a", "z": "0" }, "layout": { "type": "ring", "count": 3, "radius": 1 } }"##,
    );
    assert_eq!(
        scene_errors(&validate(&scene(&pattern, ""))),
        pairs(&[("/objects/p/shape/axes", "invalid")])
    );
}

/// A red data point `p` of radius 0.2 on the axes of `axes("")`, at data `position`.
fn data_point(position: &str) -> String {
    format!(
        r##"{}, "p": {{ "type": "sphere", "axes": "a", "radius": 0.2, "position": {position}, "material": {{ "type": "standard", "color": "#ff0000" }} }}"##,
        axes("")
    )
}

#[test]
fn a_data_point_stands_at_its_data_position_with_its_radius_in_world_units() {
    let inspected = inspect_at(&scene(&data_point("[1, 2, 4]"), ""), "0");

    let p = &inspected["objects"]["p"];
    assert_eq!(
        numbers(&p["position"]),
        [1.0, 2.0, 0.0],
        "x 1, z 4 at the top of the box, y 2 at the middle of its range"
    );
    assert_eq!(
        numbers(&p["data_point"]["position"]),
        [1.0, 2.0, 4.0],
        "the position as the scene file declares it"
    );
    assert_eq!(p["data_point"]["axes"], "a");
    assert_eq!(
        world_box(&inspected, "p"),
        (vec![0.8, 1.8, -0.2], vec![1.2, 2.2, 0.2])
    );
}

#[test]
fn a_move_and_a_follow_of_a_data_point_are_in_data_coordinates() {
    let moved = scene(
        &data_point("[1, 2, 4]"),
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "move", "object": "p", "to": [0, 4, 0] }] },
           { "type": "play", "duration": 1, "animations": [{ "type": "follow", "object": "p", "law": { "x": "t", "y": "4", "z": "2*t" } }] }"#,
    );

    let after_move = inspect_at(&moved, "1");
    assert_eq!(
        numbers(&after_move["objects"]["p"]["position"]),
        [0.0, -2.0, -1.0],
        "data (0, 4, 0) is the far, low corner's middle"
    );
    let after_follow = inspect_at(&moved, "2");
    assert_eq!(
        numbers(&after_follow["objects"]["p"]["data_point"]["position"]),
        [1.0, 4.0, 2.0]
    );
    assert_eq!(
        numbers(&after_follow["objects"]["p"]["position"]),
        [1.0, 0.0, -1.0]
    );
}

#[test]
fn the_axes_carry_a_data_point_and_its_radius_stays_in_world_units() {
    let carried = scene(
        &data_point("[1, 2, 4]"),
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "move", "object": "a", "to": [1, 0, 0] }, { "type": "scale", "object": "a", "to": [2, 2, 2] }] }"#,
    );

    let inspected = inspect_at(&carried, "1");
    assert_eq!(
        numbers(&inspected["objects"]["p"]["position"]),
        [3.0, 4.0, 0.0]
    );
    assert_eq!(
        world_box(&inspected, "p"),
        (vec![2.8, 3.8, -0.2], vec![3.2, 4.2, 0.2]),
        "still 0.2 round, though the axes doubled"
    );
}

#[test]
fn an_attachment_to_a_data_point_sits_where_the_frame_shows_it() {
    let captioned = format!(
        r##"{}, "n": {{ "type": "text", "text": "P", "size": 0.3, "attach": {{ "object": "p", "offset": [0, 0] }} }}"##,
        data_point("[1, 2, 4]")
    );

    let inspected = inspect_at(&seen_from("[6, 5, 10]", &captioned, ""), "0");
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
fn a_data_point_carried_along_a_tube_on_its_axes_stays_on_it() {
    let objects = format!(
        r##"{}, "t": {{ "type": "tube", "axes": "a", "radius": 0.05, "curve": {{ "x": "u", "y": "2", "z": "u + 1", "u_range": [-2, 2] }} }}"##,
        data_point("[-2, 2, -1]")
    );
    let carried = scene(
        &objects,
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "follow", "object": "p", "along": "t", "easing_curve": "linear" }] }"#,
    );

    assert_eq!(
        validate(&carried).exit_code,
        0,
        "{}",
        validate(&carried).stdout
    );
    let middle = inspect_at(&carried, "0.5");
    assert_eq!(
        numbers(&middle["objects"]["p"]["data_point"]["position"]),
        [0.0, 2.0, 1.0]
    );
    let wrong = validate(&scene(
        &objects.replace("[-2, 2, -1]", "[0, 0, 0]"),
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "follow", "object": "p", "along": "t" }] }"#,
    ));
    assert_eq!(
        scene_errors(&wrong),
        pairs(&[("/timeline/0/animations/0/along", "carry_jump")])
    );
    assert!(
        messages(&wrong)[0].contains("declare `p`'s `position` as [-2, 2, -1]"),
        "in data coordinates: {:?}",
        messages(&wrong)
    );
}

#[test]
fn a_data_point_on_2d_axes_is_invalid_and_in_a_pattern_too() {
    let flat = r##""flat": { "type": "axes", "x": { "range": [0, 1] }, "y": { "range": [0, 1] }, "width": 2, "height": 2 }, "p": { "type": "sphere", "axes": "flat", "radius": 0.1 }"##;
    assert_eq!(
        scene_errors(&validate(&scene(flat, ""))),
        pairs(&[("/objects/p/axes", "invalid")])
    );

    let pattern = format!(
        r##"{}, "q": {{ "type": "pattern", "shape": {{ "type": "sphere", "axes": "a", "radius": 0.1 }}, "layout": {{ "type": "ring", "count": 3, "radius": 1 }} }}"##,
        axes("")
    );
    assert_eq!(
        scene_errors(&validate(&scene(&pattern, ""))),
        pairs(&[("/objects/q/shape/axes", "invalid")])
    );
}
