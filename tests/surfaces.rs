//! Surfaces, read back from frames and reports as an agent reads them.
//!
//! The scenes are 400 by 400 pixels on black, with the camera at `[0, 0, 10]` looking at the
//! origin unless a scene sets its own, so a unit of world at the origin is about 48 pixels.

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

/// A red surface `s` of height `z` over the square from -1 to 1, with `more` fields.
fn sheet(z: &str, more: &str) -> String {
    format!(
        r##""s": {{ "type": "surface", "z": "{z}", "x_range": [-1, 1], "y_range": [-1, 1], {more} "material": {{ "type": "standard", "color": "#ff0000" }} }}"##
    )
}

/// `objects` seen from a camera at `position`, looking at the origin.
fn seen_from(position: &str, objects: &str) -> String {
    scene(objects, "").replacen(
        r#""objects""#,
        &format!(r#""camera": {{ "position": {position}, "target": [0, 0, 0] }}, "objects""#),
        1,
    )
}

/// The red of the middle pixel.
fn middle_red(scene: &str) -> u8 {
    let (_dir, rgba) = frame_at(scene, "0");
    pixel(&rgba, 200, 200)[0]
}

#[test]
fn a_surface_is_lit_on_the_side_the_camera_sees() {
    let flat = sheet("0", "");

    // Square on, the side toward the camera shows two thirds of its color in linear light, as
    // every solid does, where a side lit only by the ambient would show about 155.
    let above = middle_red(&seen_from("[0, 10, 0.5]", &flat));
    let below = middle_red(&seen_from("[0, -10, 0.5]", &flat));
    let mirrored_below = middle_red(&seen_from(
        "[0, -10, 0.5]",
        &sheet("0", r#""scale": [1, -1, 1],"#),
    ));

    assert!(above.abs_diff(213) <= 4, "from above: {above}");
    assert!(below.abs_diff(213) <= 4, "from below: {below}");
    assert!(
        mirrored_below.abs_diff(213) <= 4,
        "a mirrored sheet from below: {mirrored_below}"
    );
}

#[test]
fn a_surfaces_box_spans_its_heights_and_a_create_rises_it_from_flat() {
    let tilted = sheet("-y", "");
    let scene = scene(
        &tilted,
        r#"{ "type": "play", "duration": 2, "animations": [{ "type": "create", "object": "s", "easing_curve": "linear" }] }"#,
    );

    let (_a, start) = frame_at(&scene, "0");
    let (_b, half) = frame_at(&scene, "1");
    let (_c, end) = frame_at(&scene, "2");
    let inspected = inspect_at(&scene, "1");

    // The sheet z = -y rises toward the camera, so it faces it as a band, as tall on the frame as
    // it rises.
    let lit_rows = |rgba: &[u8]| (0..400).filter(|&y| pixel(rgba, 200, y)[0] > 60).count();
    assert_eq!(
        lit_rows(&start),
        0,
        "nothing shows before the create begins"
    );
    let (half, end) = (lit_rows(&half), lit_rows(&end));
    assert!(
        end > 90 && end < 110,
        "2 units tall is about 100 pixels: {end}"
    );
    assert!(
        half.abs_diff(end / 2) <= 4,
        "half its height: {half} of {end}"
    );
    let s = &inspected["objects"]["s"];
    assert_eq!(s["created"], 0.5);
    assert_eq!(
        numbers(&s["bounding_box"]["world"]["min"]),
        [-1.0, -1.0, -1.0],
        "the whole surface, as for any solid"
    );
    assert_eq!(numbers(&s["bounding_box"]["world"]["max"]), [1.0, 1.0, 1.0]);
}

#[test]
fn a_surfaces_box_is_its_sampled_heights_over_its_rectangle() {
    let inspected = inspect_at(&scene(&sheet("x", r#""position": [0, 1, 0],"#), ""), "0");

    let s = &inspected["objects"]["s"];
    assert_eq!(
        numbers(&s["bounding_box"]["world"]["min"]),
        [-1.0, 0.0, -1.0]
    );
    assert_eq!(numbers(&s["bounding_box"]["world"]["max"]), [1.0, 2.0, 1.0]);
    assert_eq!(s["colors"]["color"], "#ff0000");
    assert!(
        s.get("tube").is_none() && s.get("surface").is_none(),
        "no detail beyond the box: {s}"
    );
}

#[test]
fn data_y_runs_away_from_the_default_camera() {
    let inspected = inspect_at(
        &scene(
            &sheet("0", r#""y_range": [0, 2],"#).replace(r#""y_range": [-1, 1], "#, ""),
            "",
        ),
        "0",
    );

    let s = &inspected["objects"]["s"];
    assert_eq!(
        numbers(&s["bounding_box"]["world"]["min"]),
        [-1.0, 0.0, -2.0],
        "y from 0 to 2 lies along z from 0 to -2"
    );
    assert_eq!(numbers(&s["bounding_box"]["world"]["max"]), [1.0, 0.0, 0.0]);
}

#[test]
fn each_unknown_name_in_z_is_a_scene_error_at_its_column_with_each_range_high_first() {
    let run = validate(&scene(
        r##""s": { "type": "surface", "z": "a*x + sin(u, y)", "x_range": [1, -1], "y_range": [2, 2] }"##,
        "",
    ));

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/s/z", "bad_format"),
            ("/objects/s/z", "bad_format"),
            ("/objects/s/z", "bad_format"),
            ("/objects/s/x_range", "invalid"),
            ("/objects/s/y_range", "invalid")
        ])
    );
    let messages = messages(&run);
    assert!(
        messages.contains(&"`z` is not an expression sci_anim can read: unknown name `a`; the variables are `x`, `y` and `t`, and the constants are pi, e and tau (at column 1 of \"a*x + sin(u, y)\")".to_string()),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("unknown name `u`") && m.contains("at column 11 of")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("`sin` takes 1 argument") && m.contains("at column 7 of")),
        "{messages:?}"
    );
    assert!(
        messages.contains(
            &"`x_range` must give its lowest value first, and the two must differ".to_string()
        ),
        "{messages:?}"
    );
}

#[test]
fn a_height_with_no_value_or_too_far_is_a_scene_error_at_z_naming_x_and_y() {
    let no_value = validate(&scene(
        r##""s": { "type": "surface", "z": "sqrt(x)", "x_range": [-1, 1], "y_range": [0, 1] }"##,
        "",
    ));
    let too_far = validate(&scene(
        r##""s": { "type": "surface", "z": "exp(y)", "x_range": [0, 1], "y_range": [0, 20] }"##,
        "",
    ));

    assert_eq!(
        scene_errors(&no_value),
        pairs(&[("/objects/s/z", "out_of_range")])
    );
    assert_eq!(
        messages(&no_value)[0],
        "`z` has no finite value at x = -1, y = 0, and a surface needs one at every point of `x_range` by `y_range`; narrow the ranges or change the expression"
    );
    assert_eq!(
        scene_errors(&too_far),
        pairs(&[("/objects/s/z", "out_of_range")])
    );
    assert_eq!(
        messages(&too_far)[0],
        "`z` is 1.1789917546552919e4 at x = 0, y = 9.375, and a surface stays within 10000 of the object's position"
    );
}

#[test]
fn a_surface_written_with_y_over_x_and_z_is_told_its_height_is_z() {
    let run = validate(&scene(
        r##""s": { "type": "surface", "y": "x*z", "x_range": [-1, 1], "z_range": [-1, 1] }"##,
        "",
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/s/y", "unknown_field"),
            ("/objects/s/z", "missing_field"),
            ("/objects/s/z_range", "unknown_field")
        ])
    );
    let messages = messages(&run);
    assert!(messages[0].ends_with("; a surface's height is `z`, an expression in `x` and `y`, as a textbook writes z = f(x, y)"), "{messages:?}");
    assert!(messages[2].ends_with("; a surface's rectangle is `x_range` by `y_range`, and its height `z` rises along the object's own up"), "{messages:?}");
}

#[test]
fn a_range_past_the_farthest_point_is_a_scene_error_at_that_range() {
    let wide = validate(&scene(
        r##""s": { "type": "surface", "z": "0", "x_range": [-1e20, 1], "y_range": [0, 1] }"##,
        "",
    ));

    assert_eq!(
        scene_errors(&wide),
        pairs(&[("/objects/s/x_range", "out_of_range")])
    );
    assert_eq!(
        messages(&wide)[0],
        "`x_range` reaches past 10000, and a surface stays within that of the object's position"
    );
}

#[test]
fn a_surface_is_a_solid_that_cannot_be_see_through_morph_be_mapped_or_be_carried_along() {
    let see_through = sheet("0", "").replace(
        r##""color": "#ff0000""##,
        r##""color": "#ff0000", "opacity": 0.5"##,
    );
    assert_eq!(
        scene_errors(&validate(&scene(&see_through, ""))),
        pairs(&[("/objects/s/material/opacity", "invalid")])
    );

    let objects = format!(
        r##"{}, "c": {{ "type": "circle", "radius": 1, "stroke": {{ "color": "#ffffff" }} }}, "d": {{ "type": "dot", "radius": 0.1 }}"##,
        sheet("0", "")
    );
    let run = validate(&scene(
        &objects,
        r##"{ "type": "play", "duration": 1, "animations": [{ "type": "morph", "object": "s", "into": "c" }] },
            { "type": "play", "duration": 1, "animations": [{ "type": "linear-map", "objects": ["s"], "matrix": [[2, 0], [0, 1]] }] },
            { "type": "play", "duration": 1, "animations": [{ "type": "follow", "object": "d", "along": "s" }] }"##,
    ));
    let codes: Vec<String> = scene_errors(&run)
        .into_iter()
        .map(|(_, code)| code)
        .collect();
    assert_eq!(
        codes,
        ["not_morphable", "not_mappable", "no_curve"],
        "{}",
        run.stdout
    );
    assert!(messages(&run).contains(&"`s` is a surface, which has no one curve to carry along; carry along a flat shape, a plot or a tube".to_string()), "{}", run.stdout);
}

#[test]
fn a_pattern_draws_a_surface_at_every_copy() {
    let ramps = r##""p": { "type": "pattern",
        "shape": { "type": "surface", "z": "0.2*x", "x_range": [-0.5, 0.5], "y_range": [-0.5, 0.5], "material": { "type": "standard", "color": "#ff0000" } },
        "layout": { "type": "grid", "counts": [3, 1, 1], "spacing": 1.5 } }"##;

    assert_eq!(validate(&scene(ramps, "")).exit_code, 0);
    let inspected = inspect_at(&scene(ramps, ""), "0");
    let copies = inspected["objects"]["p"]["pattern"]["copies"]
        .as_array()
        .unwrap();
    assert_eq!(copies.len(), 3);
    assert_eq!(
        numbers(&copies[2]["bounding_box"]["world"]["min"]),
        [1.0, -0.1, -0.5]
    );
    assert_eq!(
        numbers(&copies[2]["bounding_box"]["world"]["max"]),
        [2.0, 0.1, 0.5]
    );
}

#[test]
fn a_heightmap_or_parametric_surface_type_names_the_surface_to_write() {
    let run = validate(&scene(
        r##""h": { "type": "heightmap" }, "p": { "type": "parametric-surface" }"##,
        "",
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/h/type", "unknown_value"),
            ("/objects/p/type", "unknown_value")
        ])
    );
    let messages = messages(&run);
    assert!(messages[0].ends_with("; a sheet of heights is a `surface`, whose `z` is an expression in `x` and `y` over `x_range` and `y_range`"), "{messages:?}");
    assert!(
        messages[1].contains("a `surface` writes only a height"),
        "{messages:?}"
    );
}

#[test]
fn the_schema_describes_z_in_x_and_y() {
    let text = sci_anim(&["schema"]).json.to_string();

    assert!(text.contains("An expression in `x` and `y`, such as `-2*exp(-(x^2 + y^2)/2)`, which gives the sheet's height z over the point (x, y)"));
}
