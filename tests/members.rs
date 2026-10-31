//! Members, driven as an agent drives them: a copy of a pattern moved, faded, recolored and lit
//! apart from the others, a bar of a bar plot lit and recolored, what `inspect` reports of each,
//! and the scene errors of a member the object does not have.

mod common;

use common::{inspect_at, pairs, scene, scene_errors, scene_file, sci_anim};

const ROW: &str = r##""row": { "type": "pattern", "shape": { "type": "circle", "radius": 0.3, "fill": { "color": "#ffffff" } },
    "layout": { "type": "grid", "counts": [3, 1, 1], "spacing": 2 }, "position": [0, 1, 0] }"##;

const BARS: &str = r##""axes": { "type": "axes", "x": { "categories": ["a", "b", "c"] }, "y": { "range": [0, 10] }, "width": 6, "height": 4 },
    "bars": { "type": "bar-plot", "axes": "axes", "values": [2, 4, 6], "fill": { "color": "#58c4dd" } }"##;

fn rounded(value: &serde_json::Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| (v.as_f64().unwrap() * 1000.0).round() / 1000.0)
        .collect()
}

fn copies() -> String {
    scene(
        ROW,
        r##"{ "type": "play", "duration": 2, "animations": [
            { "type": "move", "object": "row", "copy": [2, 0, 0], "to": [2, -2, 0], "easing_curve": "linear" },
            { "type": "fade", "object": "row", "copy": [0, 0, 0], "to": 0, "easing_curve": "linear" },
            { "type": "move", "object": "row", "to": [1, 1, 0], "easing_curve": "linear" } ] },
          { "type": "play", "duration": 1, "animations": [
            { "type": "recolor", "object": "row", "copy": [1, 0, 0], "fill": "#ff0000" },
            { "type": "highlight", "object": "row", "copy": [1, 0, 0], "to": 1, "color": "#00ff00" } ] },
          { "type": "play", "duration": 1, "animations": [
            { "type": "recolor", "object": "row", "fill": "#0000ff" } ] }"##,
    )
}

#[test]
fn a_moved_copy_moves_in_its_pattern_whose_own_move_carries_it_too() {
    let report = inspect_at(&copies(), "1");
    let copies = report["objects"]["row"]["pattern"]["copies"]
        .as_array()
        .unwrap();
    assert_eq!(rounded(&copies[0]["position"]), [-1.5, 1.0, 0.0]);
    assert_eq!(rounded(&copies[1]["position"]), [0.5, 1.0, 0.0]);
    assert_eq!(rounded(&copies[2]["position"]), [2.5, 0.0, 0.0]);
    assert_eq!(copies[0]["opacity"], 0.5);
    assert_eq!(copies[1]["opacity"], 1.0);
}

#[test]
fn a_recolored_copy_keeps_its_own_color_when_the_whole_pattern_is_recolored_later() {
    let before = inspect_at(&copies(), "2");
    let copies = before["objects"]["row"]["pattern"]["copies"]
        .as_array()
        .unwrap();
    assert_eq!(copies[1]["colors"]["fill"], "#ffffff");
    assert_eq!(copies[1]["glow"], 0.0);

    let after = inspect_at(&self::copies(), "4");
    let copies = after["objects"]["row"]["pattern"]["copies"]
        .as_array()
        .unwrap();
    assert_eq!(copies[1]["colors"]["fill"], "#ff0000");
    assert_eq!(
        (&copies[1]["glow"], &copies[1]["glow_color"]),
        (&serde_json::json!(1.0), &serde_json::json!("#00ff00"))
    );
    assert_eq!(copies[2]["colors"]["fill"], "#0000ff");
    assert_eq!(copies[2]["glow"], 0.0);
}

#[test]
fn one_bar_is_lit_and_another_recolored() {
    let report = inspect_at(
        &scene(
            BARS,
            r##"{ "type": "play", "duration": 1, "animations": [
                { "type": "highlight", "object": "bars", "bar": 1, "to": 1 },
                { "type": "recolor", "object": "bars", "bar": 2, "fill": "#fc6255" } ] }"##,
        ),
        "1",
    );
    let bars = report["objects"]["bars"]["bar_plot"]["bars"]
        .as_array()
        .unwrap();
    let seen: Vec<(&str, f64, &str)> = bars
        .iter()
        .map(|bar| {
            (
                bar["fill"].as_str().unwrap(),
                bar["glow"].as_f64().unwrap(),
                bar["glow_color"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        seen,
        [
            ("#58c4dd", 0.0, "#ffd23f"),
            ("#58c4dd", 1.0, "#ffd23f"),
            ("#fc6255", 0.0, "#ffd23f")
        ]
    );
}

fn errors(animation: &str) -> Vec<(String, String)> {
    let objects = format!(
        r#"{ROW}, {BARS}, "c": {{ "type": "circle", "radius": 1 }},
        "ring": {{ "type": "pattern", "shape": {{ "type": "circle", "radius": 0.2 }}, "layout": {{ "type": "ring", "count": 4, "radius": 1 }} }}"#
    );
    let (_dir, path) = scene_file(&scene(
        &objects,
        &format!(r#"{{ "type": "play", "duration": 1, "animations": [{animation}] }}"#),
    ));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    scene_errors(&run)
}

#[test]
fn a_member_the_object_does_not_have_is_refused_at_its_field() {
    let at =
        |field: &str, code: &str| pairs(&[(&format!("/timeline/0/animations/0/{field}"), code)]);
    assert_eq!(
        errors(r#"{ "type": "highlight", "object": "row", "copy": [3, 0, 0], "to": 1 }"#),
        at("copy", "unknown_copy")
    );
    assert_eq!(
        errors(r##"{ "type": "recolor", "object": "bars", "bar": 3, "fill": "#ffffff" }"##),
        at("bar", "unknown_bar")
    );
    assert_eq!(
        errors(r#"{ "type": "move", "object": "c", "copy": [0], "to": [0, 0, 0] }"#),
        at("copy", "not_pattern")
    );
    assert_eq!(
        errors(r#"{ "type": "highlight", "object": "c", "bar": 0, "to": 1 }"#),
        at("bar", "not_bar_plot")
    );
    assert_eq!(
        errors(r#"{ "type": "fade", "object": "ring", "copy": [0, 0, 0], "to": 0 }"#),
        at("copy", "wrong_length")
    );
    assert_eq!(
        errors(r#"{ "type": "highlight", "object": "ring", "to": 1 }"#),
        at("copy", "missing_field")
    );
    assert_eq!(
        errors(r#"{ "type": "highlight", "object": "row", "copy": [0, 0, 0], "bar": 0, "to": 1 }"#),
        at("bar", "invalid")
    );
    assert_eq!(
        errors(r#"{ "type": "move", "object": "bars", "bar": 0, "to": [0, 0, 0] }"#),
        at("bar", "unknown_field")
    );
}

#[test]
fn two_moves_of_one_copy_at_once_conflict_and_moves_of_two_copies_do_not() {
    assert_eq!(
        errors(
            r#"{ "type": "move", "object": "row", "copy": [0, 0, 0], "to": [0, 0, 0] },
               { "type": "move", "object": "row", "copy": [0, 0, 0], "to": [1, 0, 0] }"#
        ),
        pairs(&[("/timeline/0/animations/1", "conflicting_animations")])
    );
    let (_dir, path) = scene_file(&scene(
        ROW,
        r#"{ "type": "play", "duration": 1, "animations": [
            { "type": "move", "object": "row", "copy": [0, 0, 0], "to": [0, 0, 0] },
            { "type": "move", "object": "row", "copy": [1, 0, 0], "to": [1, 0, 0] } ] }"#,
    ));
    assert_eq!(sci_anim(&["validate", path.to_str().unwrap()]).exit_code, 0);
}
