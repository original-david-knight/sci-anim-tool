//! Readouts: a number typeset as math whose value is a track, read back from reports as an agent
//! reads them.

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

const SPEED: &str = r#""speed": { "type": "readout", "decimals": 1, "unit": "\\text{ m/s}" }"#;

fn play(animations: &str) -> String {
    format!(r#"{{ "type": "play", "duration": 2, "animations": [ {animations} ] }}"#)
}

#[test]
fn a_count_eases_a_readout_to_its_to() {
    let scene = scene(
        SPEED,
        &play(r#"{ "type": "count", "object": "speed", "to": 120, "easing_curve": "linear" }"#),
    );
    let readout = |scene_time| inspect_at(&scene, scene_time)["objects"]["speed"].clone();

    assert_eq!(
        readout("0")["readout"],
        serde_json::json!({ "value": 0.0, "text": "0.0" })
    );
    assert_eq!(readout("0.5")["readout"]["text"], "30.0");
    assert_eq!(readout("2")["readout"]["text"], "120.0");
    assert_eq!(
        readout("2")["glyphs"],
        8,
        "1, 2, 0, point, 0, then m, / and s"
    );
}

#[test]
fn a_follow_sets_a_readout_from_a_law_of_its_value() {
    let scene = scene(
        SPEED,
        &play(
            r#"{ "type": "follow", "object": "speed", "sets": "value", "law": { "value": "9.8*t" } }"#,
        ),
    );

    assert_eq!(
        inspect_at(&scene, "1")["objects"]["speed"]["readout"]["text"],
        "9.8"
    );
    assert_eq!(
        inspect_at(&scene, "2")["objects"]["speed"]["readout"]["text"],
        "19.6"
    );
}

#[test]
fn a_negative_zero_shows_as_zero_and_the_decimals_round() {
    let objects = r#""a": { "type": "readout", "value": -0.04, "decimals": 1 }, "b": { "type": "readout", "value": 2.71828, "decimals": 3 }"#;
    let inspected = inspect_at(&scene(objects, ""), "0");

    assert_eq!(inspected["objects"]["a"]["readout"]["text"], "0.0");
    assert_eq!(inspected["objects"]["b"]["readout"]["text"], "2.718");
}

#[test]
fn the_unit_keeps_its_place_as_the_number_grows() {
    let scene = scene(
        SPEED,
        &play(r#"{ "type": "count", "object": "speed", "to": 123.4 }"#),
    );
    let right = |scene_time| {
        numbers(&inspect_at(&scene, scene_time)["objects"]["speed"]["bounding_box"]["world"]["max"])
            [0]
    };
    let left = |scene_time| {
        numbers(&inspect_at(&scene, scene_time)["objects"]["speed"]["bounding_box"]["world"]["min"])
            [0]
    };

    assert_eq!(right("0"), right("2"), "the unit ends at one place");
    assert!(left("2") < left("0"), "the number grows to the left");
}

#[test]
fn a_law_of_value_starts_where_the_value_is_and_has_a_value_at_every_frame_time() {
    let jump = play(
        r#"{ "type": "follow", "object": "speed", "sets": "value", "law": { "value": "1 + t" } }"#,
    );
    let gap = play(
        r#"{ "type": "follow", "object": "speed", "sets": "value", "law": { "value": "1/(t - 1) + 1" } }"#,
    );

    assert_eq!(
        errors(SPEED, &jump),
        pairs(&[("/timeline/0/animations/0/law", "law_jump")])
    );
    assert_eq!(
        errors(SPEED, &gap),
        pairs(&[("/timeline/0/animations/0/law/value", "out_of_range")])
    );
}

#[test]
fn a_law_of_value_has_one_expression() {
    let three = play(
        r#"{ "type": "follow", "object": "speed", "sets": "value", "law": { "x": "t", "value": "t" } }"#,
    );
    let none =
        play(r#"{ "type": "follow", "object": "speed", "sets": "value", "law": { "x": "t" } }"#);

    assert_eq!(
        errors(SPEED, &three),
        pairs(&[("/timeline/0/animations/0/law/x", "invalid")])
    );
    assert_eq!(
        errors(SPEED, &none),
        pairs(&[
            ("/timeline/0/animations/0/law/x", "invalid"),
            ("/timeline/0/animations/0/law/value", "missing_field")
        ])
    );
}

#[test]
fn only_a_readout_has_a_value() {
    let objects = format!(r#"{SPEED}, "p": {{ "type": "dot", "radius": 0.1 }}"#);
    let timeline = play(
        r#"{ "type": "count", "object": "p", "to": 1 }, { "type": "follow", "object": "p", "sets": "value", "law": { "value": "t" } }"#,
    );

    assert_eq!(
        errors(&objects, &timeline),
        pairs(&[
            ("/timeline/0/animations/0/object", "not_readout"),
            ("/timeline/0/animations/1/object", "not_readout")
        ])
    );
}

#[test]
fn a_readout_s_fields_are_checked() {
    let objects = r#""a": { "type": "readout", "decimals": 7 }, "b": { "type": "readout", "unit": "\\frac{" }"#;

    assert_eq!(
        errors(objects, ""),
        pairs(&[("/objects/a/decimals", "out_of_range")])
    );
    assert_eq!(
        errors(r#""b": { "type": "readout", "unit": "\\frac{" }"#, ""),
        pairs(&[("/objects/b/unit", "bad_format")])
    );
}
