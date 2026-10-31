//! Simulations: values set from variables integrated by their rates, read back from reports as an
//! agent reads them.

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

/// A mass on a spring of period 2 pi, from x = 1 at rest, and a readout of its speed.
const OBJECTS: &str = r#""mass": { "type": "dot", "radius": 0.1, "position": [1, 0, 0] }, "speed": { "type": "readout", "decimals": 3 }"#;

fn simulate(fields: &str) -> String {
    format!(
        r#"{{ "type": "play", "duration": 6.2832, "animations": [ {{ "type": "simulate", {fields} }} ] }}"#
    )
}

const SPRING: &str = r#""variables": { "x": 1, "v": 0 }, "rates": { "x": "v", "v": "-x" },
    "sets": [ { "object": "mass", "position": ["x", "0", "0"] }, { "object": "speed", "value": "abs(v)" } ]"#;

#[test]
fn a_simulation_sets_its_objects_from_its_variables_at_any_scene_time() {
    let scene = scene(OBJECTS, &simulate(SPRING));
    let at = |scene_time| inspect_at(&scene, scene_time);

    assert_eq!(
        numbers(&at("1.5708")["objects"]["mass"]["position"]),
        [0.0, 0.0, 0.0],
        "a quarter period on, at rest's middle"
    );
    assert_eq!(at("1.5708")["objects"]["speed"]["readout"]["text"], "1.000");
    assert_eq!(
        numbers(&at("3.14159")["objects"]["mass"]["position"]),
        [-1.0, 0.0, 0.0]
    );
    assert_eq!(
        numbers(&at("0.12345")["objects"]["mass"]["position"])[0],
        0.992,
        "cos(0.12345), between two steps"
    );
}

#[test]
fn a_simulation_holds_what_it_set_from_its_span_s_end() {
    let timeline = format!(
        r#"{}, {{ "type": "wait", "duration": 1 }}"#,
        simulate(SPRING)
    );
    let scene = scene(OBJECTS, &timeline);

    assert_eq!(
        inspect_at(&scene, "7")["objects"]["mass"]["position"],
        inspect_at(&scene, "6.2832")["objects"]["mass"]["position"]
    );
}

#[test]
fn rates_read_law_time() {
    let fields = r#""variables": { "x": 1 }, "rates": { "x": "2*t" }, "sets": [ { "object": "mass", "position": ["x", "0", "0"] } ]"#;

    assert_eq!(
        numbers(
            &inspect_at(&scene(OBJECTS, &simulate(fields)), "2")["objects"]["mass"]["position"]
        )[0],
        5.0,
        "1 + t^2 at t = 2"
    );
}

#[test]
fn a_simulation_has_at_least_one_variable() {
    let fields = r#""variables": {}, "rates": {}, "sets": [ { "object": "mass", "position": ["1", "0", "0"] } ]"#;

    assert_eq!(
        errors(OBJECTS, &simulate(fields)),
        pairs(&[("/timeline/0/animations/0/variables", "wrong_length")])
    );
}

#[test]
fn a_set_starts_where_its_value_is() {
    let fields = r#""variables": { "x": 2, "v": 0 }, "rates": { "x": "v", "v": "-x" }, "sets": [ { "object": "mass", "position": ["x", "0", "0"] } ]"#;

    assert_eq!(
        errors(OBJECTS, &simulate(fields)),
        pairs(&[("/timeline/0/animations/0/sets/0/position", "law_jump")])
    );
}

#[test]
fn a_variable_with_no_value_is_out_of_range_at_its_rate() {
    let fields = r#""variables": { "x": 1 }, "rates": { "x": "x*x" }, "sets": [ { "object": "mass", "position": ["1", "0", "0"] } ]"#;

    assert_eq!(
        errors(OBJECTS, &simulate(fields)),
        pairs(&[("/timeline/0/animations/0/rates/x", "out_of_range")])
    );
}

#[test]
fn a_set_with_no_value_at_a_frame_time_is_out_of_range_at_its_expression() {
    let fields = r#""variables": { "x": 1, "v": 0 }, "rates": { "x": "v", "v": "-x" }, "sets": [ { "object": "mass", "position": ["x", "sqrt(x)", "0"] } ]"#;
    let objects = r#""mass": { "type": "dot", "radius": 0.1, "position": [1, 1, 0] }"#;

    assert_eq!(
        errors(objects, &simulate(fields)),
        pairs(&[("/timeline/0/animations/0/sets/0/position/1", "out_of_range")])
    );
}

#[test]
fn every_variable_has_one_rate_and_a_name_an_expression_reads() {
    let fields = r#""variables": { "x": 1, "sin": 0, "y": 0 }, "rates": { "x": "x", "sin": "0", "z": "1" }, "sets": [ { "object": "mass", "position": ["x", "0", "0"] } ]"#;

    assert_eq!(
        errors(OBJECTS, &simulate(fields)),
        pairs(&[
            ("/timeline/0/animations/0/variables/sin", "invalid"),
            ("/timeline/0/animations/0/rates/y", "missing_field"),
            ("/timeline/0/animations/0/rates/z", "invalid")
        ])
    );
}

#[test]
fn a_set_names_one_value_of_an_object_that_has_it() {
    let fields = r#""variables": { "x": 1 }, "rates": { "x": "0" }, "sets": [
        { "object": "nowhere", "position": ["x", "0", "0"] },
        { "object": "mass", "position": ["x", "0", "0"], "value": "x" },
        { "object": "mass", "value": "x" },
        { "object": "speed", "value": "w" } ]"#;

    assert_eq!(
        errors(OBJECTS, &simulate(fields)),
        pairs(&[
            ("/timeline/0/animations/0/sets/0/object", "unknown_object"),
            ("/timeline/0/animations/0/sets/1", "invalid"),
            ("/timeline/0/animations/0/sets/2/object", "not_readout"),
            ("/timeline/0/animations/0/sets/3/value", "bad_format"),
        ])
    );
}

#[test]
fn a_simulation_owns_its_values_and_takes_no_easing_curve() {
    let objects = r#""ax": { "type": "axes", "x": { "range": [0, 1] }, "y": { "range": [0, 1] }, "width": 2, "height": 2 },
        "curve": { "type": "function-plot", "axes": "ax", "expression": "x" }, "mass": { "type": "dot", "radius": 0.1, "position": [1, 0, 0] }"#;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [
        { "type": "simulate", "easing_curve": "linear", "variables": { "x": 1 }, "rates": { "x": "0" }, "sets": [ { "object": "curve", "position": ["x", "0", "0"] } ] },
        { "type": "simulate", "variables": { "x": 1 }, "rates": { "x": "0" }, "sets": [ { "object": "mass", "position": ["x", "0", "0"] } ] },
        { "type": "move", "object": "mass", "to": [2, 0, 0] } ] }"#;

    assert_eq!(
        errors(objects, timeline),
        pairs(&[
            ("/timeline/0/animations/0/easing_curve", "invalid"),
            ("/timeline/0/animations/2", "conflicting_animations")
        ])
    );
}

#[test]
fn a_simulation_sets_a_rotation() {
    let fields = r#""variables": { "angle": 0 }, "rates": { "angle": "10" }, "sets": [ { "object": "mass", "rotation": ["0", "0", "angle"] } ]"#;

    assert_eq!(
        numbers(
            &inspect_at(&scene(OBJECTS, &simulate(fields)), "1")["objects"]["mass"]["rotation"]
        ),
        [0.0, 0.0, 10.0]
    );
}

#[test]
fn law_time_counts_from_the_chapter_from_names() {
    let objects = r#""mass": { "type": "dot", "radius": 0.1, "position": [1, 0, 0] }"#;
    let timeline = r#"{ "type": "chapter", "id": "fall", "steps": [ { "type": "wait", "duration": 1 }, { "type": "play", "duration": 1, "animations": [
        { "type": "simulate", "from": "fall", "variables": { "x": 0 }, "rates": { "x": "0" }, "sets": [ { "object": "mass", "position": ["x + t", "0", "0"] } ] } ] } ] }"#;

    assert_eq!(
        numbers(&inspect_at(&scene(objects, timeline), "1.5")["objects"]["mass"]["position"]),
        [1.5, 0.0, 0.0],
        "law time 1.5 at scene time 1.5"
    );
    assert_eq!(
        errors(objects, &timeline.replace(r#""from": "fall", "#, "")),
        pairs(&[(
            "/timeline/0/steps/1/animations/0/sets/0/position",
            "law_jump"
        )]),
        "law time 0 without it"
    );
}

#[test]
fn a_set_places_an_object_with_a_place_of_its_own() {
    let objects = format!(r#"{OBJECTS}, "path": {{ "type": "trail", "object": "mass" }}"#);
    let fields = r#""variables": { "x": 1 }, "rates": { "x": "0" }, "sets": [ { "object": "path", "position": ["x", "0", "0"] } ]"#;

    assert_eq!(
        errors(&objects, &simulate(fields)),
        pairs(&[("/timeline/0/animations/0/sets/0/object", "placed_by_trail")])
    );
}

#[test]
fn a_simulation_sets_each_value_once() {
    let fields = r#""variables": { "x": 1 }, "rates": { "x": "0" }, "sets": [
        { "object": "mass", "position": ["x", "0", "0"] }, { "object": "mass", "rotation": ["0", "0", "0"] }, { "object": "mass", "position": ["x", "0", "0"] } ]"#;
    let (_dir, path) = scene_file(&scene(OBJECTS, &simulate(fields)));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/0/animations/0/sets/2", "invalid")])
    );
    assert!(
        run.json["errors"][0]["message"]
            .as_str()
            .unwrap()
            .starts_with(
                "the set at /timeline/0/animations/0/sets/0 already sets the position of `mass`"
            ),
        "{}",
        run.stdout
    );
}

#[test]
fn a_simulation_spans_at_most_an_hour_of_frames_at_60_fps() {
    let timeline = r#"{ "type": "play", "duration": 10000, "animations": [
        { "type": "simulate", "variables": { "x": 1 }, "rates": { "x": "0" }, "sets": [ { "object": "mass", "position": ["x", "0", "0"] } ] } ] }"#;

    assert_eq!(
        errors(OBJECTS, timeline),
        pairs(&[("/timeline/0/animations/0", "out_of_range")])
    );
}

#[test]
fn an_expression_error_names_the_coordinate_by_its_axis() {
    let fields = r#""variables": { "x": 1 }, "rates": { "x": "0" }, "sets": [ { "object": "mass", "position": ["x", "w", "0"] } ]"#;
    let (_dir, path) = scene_file(&scene(OBJECTS, &simulate(fields)));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/0/animations/0/sets/0/position/1", "bad_format")])
    );
    assert!(
        run.json["errors"][0]["message"]
            .as_str()
            .unwrap()
            .starts_with("`y` is not an expression"),
        "{}",
        run.stdout
    );
}
