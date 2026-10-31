//! Follows: values set from a law at every scene time, read back from reports as an agent reads
//! them.
//!
//! The scenes are 400 by 400 pixels on black, with the camera at `[0, 0, 10]` looking at the
//! origin.

mod common;

use common::{inspect_at, pairs, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n.as_f64().unwrap() * 1000.0).round() / 1000.0 + 0.0)
        .collect()
}

fn validate(scene: &str) -> common::Run {
    let (_dir, path) = scene_file(scene);
    sci_anim(&["validate", path.to_str().unwrap()])
}

fn errors(objects: &str, timeline: &str) -> Vec<(String, String)> {
    let run = validate(&scene(objects, timeline));
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    scene_errors(&run)
}

fn message(objects: &str, timeline: &str) -> String {
    let run = validate(&scene(objects, timeline));
    run.json["errors"][0]["message"]
        .as_str()
        .unwrap_or_else(|| panic!("{}", run.stdout))
        .to_string()
}

/// A play step of `duration` seconds with `animations`.
fn play(duration: f64, animations: &str) -> String {
    format!(r#"{{ "type": "play", "duration": {duration}, "animations": [{animations}] }}"#)
}

/// A rod hung from `[0, 2]` and a bob at its end, both swung 30 degrees right at rest.
const PENDULUM: &str = r##""rod": { "type": "line", "from": [0, 0], "to": [0, -2], "position": [0, 2, 0], "rotation": [0, 0, 30] },
    "bob": { "type": "dot", "radius": 0.18, "position": [1, 0.268, 0], "fill": { "color": "#fc6255" } }"##;

const SWING: &str = r#"{ "type": "follow", "object": "rod", "sets": "rotation", "law": { "x": "0", "y": "0", "z": "30*cos(2.21*t)" } },
    { "type": "follow", "object": "bob", "law": { "x": "2*sin(pi/6*cos(2.21*t))", "y": "2 - 2*cos(pi/6*cos(2.21*t))", "z": "0" } }"#;

const DOT: &str = r##""p": { "type": "dot", "radius": 0.1 }"##;

#[test]
fn a_pendulum_is_where_its_law_puts_it_at_any_scene_time() {
    let scene = scene(PENDULUM, &play(2.0, SWING));

    let swinging = inspect_at(&scene, "0.7");
    let held = inspect_at(&scene, "2");

    assert_eq!(
        numbers(&swinging["objects"]["rod"]["rotation"]),
        [0.0, 0.0, 0.714],
        "30*cos(2.21*0.7)"
    );
    assert_eq!(
        numbers(&swinging["objects"]["bob"]["position"]),
        [0.025, 0.0, 0.0]
    );
    assert_eq!(
        numbers(&held["objects"]["rod"]["rotation"]),
        [0.0, 0.0, -8.647],
        "30*cos(4.42), held from the span's end"
    );
}

#[test]
fn an_orbit_continued_across_two_steps_with_from_is_continuous() {
    let planet = r##""earth": { "type": "sphere", "radius": 0.3, "position": [4, 0, 0] }"##;
    let orbit = r#"{ "type": "follow", "object": "earth", "from": "orbit", "law": { "x": "4*cos(tau*t/8)", "y": "0", "z": "-4*sin(tau*t/8)" } }"#;
    let chapter = |second: &str| {
        format!(
            r#"{{ "type": "chapter", "id": "orbit", "steps": [{}, {}] }}"#,
            play(4.0, orbit),
            play(4.0, second)
        )
    };

    let continued = scene(planet, &chapter(orbit));
    let restarted = chapter(&orbit.replace(r#""from": "orbit", "#, ""));

    assert_eq!(validate(&continued).exit_code, 0);
    assert_eq!(
        numbers(&inspect_at(&continued, "4")["objects"]["earth"]["position"]),
        [-4.0, 0.0, 0.0]
    );
    assert_eq!(
        numbers(&inspect_at(&continued, "6")["objects"]["earth"]["position"]),
        [0.0, 0.0, 4.0],
        "law time 6 in the second step"
    );
    let before = numbers(&inspect_at(&continued, "3.9833")["objects"]["earth"]["position"]);
    let after = numbers(&inspect_at(&continued, "4.0167")["objects"]["earth"]["position"]);
    assert!(
        (before[2] - -after[2]).abs() < 0.001 && (before[0] - after[0]).abs() < 0.001,
        "one frame either side mirror each other: {before:?} {after:?}"
    );
    assert_eq!(
        errors(planet, &restarted),
        pairs(&[("/timeline/0/steps/1/animations/0/law", "law_jump")])
    );
    let jump = message(planet, &restarted);
    assert!(
        jump.contains("the law gives [4, 0, 0]")
            && jump.contains("the position of `earth` is [-4, 0, 0]"),
        "{jump}"
    );
}

#[test]
fn a_follow_then_a_move_eases_from_the_held_value() {
    let timeline = format!(
        "{}, {}",
        play(
            1.0,
            r#"{ "type": "follow", "object": "p", "law": { "x": "2*t", "y": "0", "z": "0" } }"#
        ),
        play(
            1.0,
            r#"{ "type": "move", "object": "p", "to": [4, 0, 0], "easing_curve": "linear" }"#
        )
    );
    let scene = scene(DOT, &timeline);

    assert_eq!(
        numbers(&inspect_at(&scene, "1")["objects"]["p"]["position"]),
        [2.0, 0.0, 0.0]
    );
    assert_eq!(
        numbers(&inspect_at(&scene, "1.5")["objects"]["p"]["position"]),
        [3.0, 0.0, 0.0]
    );
}

#[test]
fn an_object_attached_to_a_followed_object_goes_with_it() {
    let objects = r##""box": { "type": "circle", "radius": 0.5, "stroke": { "color": "#ffffff" } },
        "n": { "type": "text", "text": "1", "attach": { "object": "box" } }"##;
    let scene = scene(
        objects,
        &play(
            1.0,
            r#"{ "type": "follow", "object": "box", "law": { "x": "t", "y": "0", "z": "0" } }"#,
        ),
    );

    let inspected = inspect_at(&scene, "1");

    assert_eq!(
        numbers(&inspected["objects"]["box"]["position"]),
        [1.0, 0.0, 0.0]
    );
    assert_eq!(
        numbers(&inspected["objects"]["n"]["position"]),
        [0.927, 0.0, 0.727],
        "on the line from the camera to the box, nearer by the circle's reach"
    );
}

#[test]
fn a_camera_follow_circles_the_target() {
    let camera = play(
        4.0,
        r#"{ "type": "follow", "sets": "camera-position", "law": { "x": "10*sin(tau*t/4)", "y": "0", "z": "10*cos(tau*t/4)" } }"#,
    );
    let scene = scene(DOT, &camera);

    let quarter = inspect_at(&scene, "1");

    assert_eq!(numbers(&quarter["camera"]["position"]), [10.0, 0.0, 0.0]);
    assert_eq!(numbers(&quarter["camera"]["target"]), [0.0, 0.0, 0.0]);
    let through = play(
        2.0,
        r#"{ "type": "follow", "sets": "camera-position", "law": { "x": "0", "y": "0", "z": "10 - 10*t" } }"#,
    );
    assert_eq!(
        errors(DOT, &through),
        pairs(&[("/timeline/0/animations/0", "invalid")]),
        "the camera reaches its target at scene time 1"
    );
}

#[test]
fn every_mistake_in_a_laws_expressions_is_an_error_at_its_expression() {
    let unknown_names = play(
        2.0,
        r#"{ "type": "follow", "object": "p", "law": { "x": "cos(u)", "y": "a*t", "z": "0" } }"#,
    );
    let no_value = play(
        2.0,
        r#"{ "type": "follow", "object": "p", "law": { "x": "exp(5*t) - 1", "y": "0", "z": "1/(t - 1) + 1" } }"#,
    );

    assert_eq!(
        errors(DOT, &unknown_names),
        pairs(&[
            ("/timeline/0/animations/0/law/x", "bad_format"),
            ("/timeline/0/animations/0/law/y", "bad_format")
        ])
    );
    let run = validate(&scene(DOT, &no_value));
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/0/animations/0/law/x", "out_of_range"),
            ("/timeline/0/animations/0/law/z", "out_of_range")
        ])
    );
    let messages: Vec<&str> = run.json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap())
        .collect();
    assert!(messages[0].starts_with("`x` reaches 10403.5657 at scene time 1.85 (law time 1.85), past the 10000 a position may reach"), "{messages:?}");
    assert!(
        messages[1].starts_with("`z` has no finite value at scene time 1 (law time 1)"),
        "{messages:?}"
    );
}

#[test]
fn each_field_a_follow_cannot_take_is_an_error_at_that_field() {
    let follow = |fields: &str| play(1.0, &format!(r#"{{ "type": "follow", {fields} }}"#));
    let law = r#""law": { "x": "0", "y": "0", "z": "0" }"#;

    assert_eq!(
        errors(
            DOT,
            &follow(&format!(
                r#""object": "p", "easing_curve": "linear", {law}"#
            ))
        ),
        pairs(&[("/timeline/0/animations/0/easing_curve", "invalid")])
    );
    assert_eq!(
        errors(DOT, &follow(r#""object": "p""#)),
        pairs(&[("/timeline/0/animations/0/law", "missing_field")])
    );
    assert_eq!(
        errors(
            DOT,
            &follow(&format!(r#""object": "p", "sets": "camera-target", {law}"#))
        ),
        pairs(&[("/timeline/0/animations/0/object", "invalid")])
    );
    assert_eq!(
        errors(DOT, &follow(law)),
        pairs(&[("/timeline/0/animations/0/object", "missing_field")])
    );
    assert_eq!(
        errors(DOT, &follow(&format!(r#""object": "q", {law}"#))),
        pairs(&[("/timeline/0/animations/0/object", "unknown_object")])
    );
    assert_eq!(
        errors(
            DOT,
            &follow(&format!(r#""object": "p", "sets": "size", {law}"#))
        ),
        pairs(&[("/timeline/0/animations/0/sets", "unknown_value")])
    );
}

#[test]
fn from_names_a_chapter_that_has_begun() {
    let follow = r#"{ "type": "follow", "object": "p", "from": "later", "law": { "x": "0", "y": "0", "z": "0" } }"#;
    let timeline = format!(
        r#"{}, {{ "type": "chapter", "id": "later", "steps": [{{ "type": "wait", "duration": 1 }}] }}"#,
        play(1.0, follow)
    );

    assert_eq!(
        errors(DOT, &timeline),
        pairs(&[("/timeline/0/animations/0/from", "unknown_chapter")])
    );
    assert!(
        message(DOT, &timeline).contains("no chapter has begun by this step"),
        "{}",
        message(DOT, &timeline)
    );
}

#[test]
fn a_law_must_start_where_its_value_is() {
    let jump = play(
        1.0,
        r#"{ "type": "follow", "object": "p", "law": { "x": "1 + t", "y": "0", "z": "0" } }"#,
    );

    assert_eq!(
        errors(DOT, &jump),
        pairs(&[("/timeline/0/animations/0/law", "law_jump")])
    );
    let message = message(DOT, &jump);
    assert!(message.starts_with("at scene time 0 (law time 0) the law gives [1, 0, 0], and the position of `p` is [0, 0, 0] there"), "{message}");
}

#[test]
fn a_follow_joins_move_and_rotate_in_every_rule_that_refuses_them() {
    let objects = r##""p": { "type": "dot", "radius": 0.1 },
        "ax": { "type": "axes", "x": { "range": [-2, 2] }, "y": { "range": [-2, 2] }, "width": 4, "height": 4 },
        "f": { "type": "function-plot", "axes": "ax", "expression": "x" },
        "c": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff" } },
        "n": { "type": "text", "text": "1", "attach": { "object": "c" } }"##;
    let follow = |object: &str| {
        format!(
            r#"{{ "type": "follow", "object": "{object}", "law": {{ "x": "0", "y": "0", "z": "0" }} }}"#
        )
    };
    let map = r#"{ "type": "linear-map", "objects": ["c"], "matrix": [[2, 0], [0, 1]] }"#;
    let moved = r#"{ "type": "move", "object": "p", "to": [1, 0, 0] }"#;
    let timeline = format!(
        "{}, {}",
        play(1.0, &[follow("f"), follow("n"), map.to_string()].join(", ")),
        play(
            1.0,
            &[follow("c"), follow("p"), moved.to_string()].join(", ")
        )
    );

    assert_eq!(
        errors(objects, &timeline),
        pairs(&[
            ("/timeline/0/animations/0/object", "placed_by_axes"),
            ("/timeline/0/animations/1/object", "attached"),
            ("/timeline/1/animations/0/object", "mapped_object"),
            ("/timeline/1/animations/2", "conflicting_animations"),
        ])
    );
}

/// A ring of six white pieces.
const PIECES: &str = r##""bits": { "type": "pattern", "shape": { "type": "circle", "radius": 0.2, "fill": { "color": "#ffffff" } }, "layout": { "type": "ring", "count": 6, "radius": 1.5 } }"##;

const PULSE_AND_FADE: &str = r#"{ "type": "follow", "object": "bits", "sets": "scale", "law": { "x": "1 + 0.5*sin(pi*t)", "y": "1 + 0.5*sin(pi*t)", "z": "1" } },
    { "type": "follow", "object": "bits", "sets": "opacity", "law": { "value": "1 - t/2" } }"#;

#[test]
fn a_pattern_of_pieces_pulses_in_scale_and_fades_out_by_laws() {
    let scene = scene(
        PIECES,
        &format!(
            r#"{}, {{ "type": "wait", "duration": 1 }}"#,
            play(2.0, PULSE_AND_FADE)
        ),
    );

    let swelling = inspect_at(&scene, "0.5");
    let shrinking = inspect_at(&scene, "1.5");
    let held = inspect_at(&scene, "2.5");

    assert_eq!(
        numbers(&swelling["objects"]["bits"]["scale"]),
        [1.5, 1.5, 1.0]
    );
    assert_eq!(swelling["objects"]["bits"]["opacity"], 0.75);
    assert_eq!(
        numbers(&shrinking["objects"]["bits"]["scale"]),
        [0.5, 0.5, 1.0]
    );
    assert_eq!(shrinking["objects"]["bits"]["opacity"], 0.25);
    assert_eq!(numbers(&held["objects"]["bits"]["scale"]), [1.0, 1.0, 1.0]);
    assert_eq!(
        held["objects"]["bits"]["opacity"], 0.0,
        "held from the span's end"
    );
}

#[test]
fn an_opacity_law_stays_within_0_to_1_and_starts_where_the_opacity_is() {
    let follow = |law: &str| {
        play(
            2.0,
            &format!(r#"{{ "type": "follow", "object": "p", "sets": "opacity", "law": {law} }}"#),
        )
    };

    assert_eq!(
        errors(DOT, &follow(r#"{ "value": "1 - t" }"#)),
        pairs(&[("/timeline/0/animations/0/law/value", "out_of_range")])
    );
    assert!(
        message(DOT, &follow(r#"{ "value": "1 - t" }"#)).starts_with("`value` reaches -0.0167 at scene time 1.0167 (law time 1.0167), outside the 0 to 1 an object opacity takes"),
        "{}",
        message(DOT, &follow(r#"{ "value": "1 - t" }"#))
    );
    assert_eq!(
        errors(DOT, &follow(r#"{ "value": "0.5" }"#)),
        pairs(&[("/timeline/0/animations/0/law", "law_jump")])
    );
    assert!(
        message(DOT, &follow(r#"{ "value": "0.5" }"#)).starts_with(
            "at scene time 0 (law time 0) the law gives 0.5, and the opacity of `p` is 1 there"
        ),
        "{}",
        message(DOT, &follow(r#"{ "value": "0.5" }"#))
    );
    assert_eq!(
        errors(DOT, &follow(r#"{ "x": "1", "value": "1" }"#)),
        pairs(&[("/timeline/0/animations/0/law/x", "invalid")])
    );
    assert_eq!(
        errors(DOT, &follow(r#"{ "x": "1", "y": "1", "z": "1" }"#)),
        pairs(&[
            ("/timeline/0/animations/0/law/x", "invalid"),
            ("/timeline/0/animations/0/law/y", "invalid"),
            ("/timeline/0/animations/0/law/z", "invalid"),
            ("/timeline/0/animations/0/law/value", "missing_field"),
        ])
    );
}

#[test]
fn a_follow_of_scale_joins_scales_and_one_of_opacity_joins_fades() {
    let objects = r##""c": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff" } },
        "n": { "type": "text", "text": "1", "attach": { "object": "c" } }"##;
    let law = |object: &str, sets: &str| {
        let law = if sets == "opacity" {
            r#"{ "value": "1 - t/2" }"#
        } else {
            r#"{ "x": "1", "y": "1", "z": "1" }"#
        };
        format!(r#"{{ "type": "follow", "object": "{object}", "sets": "{sets}", "law": {law} }}"#)
    };
    let fade = r#"{ "type": "fade", "object": "c", "to": 0 }"#;
    let scale = r#"{ "type": "scale", "object": "c", "to": [2, 2, 2] }"#;
    let timeline = [
        play(1.0, &[law("n", "scale"), law("n", "opacity")].join(", ")),
        play(1.0, &[law("c", "opacity"), fade.to_string()].join(", ")),
        play(1.0, &[law("c", "scale"), scale.to_string()].join(", ")),
    ]
    .join(", ");

    assert_eq!(
        errors(objects, &timeline),
        pairs(&[
            ("/timeline/0/animations/0/object", "attached"),
            ("/timeline/1/animations/1", "conflicting_animations"),
            ("/timeline/2/animations/1", "conflicting_animations"),
        ])
    );
}

#[test]
fn the_schema_describes_a_laws_expressions_in_t() {
    let text = sci_anim(&["schema"]).json.to_string();

    assert!(text.contains("An expression in the one variable `t`, law time in seconds, such as `30*cos(2.21*t)`, which gives the z of the value the follow sets"));
}

/// Axes whose data units are world units, centered on the origin, so data `(x, y)` is world
/// `(x, y - 2)`, with the parabola `y = x^2/2` on them and a triangle at its start facing along it.
const PARABOLA: &str = r##""ax": { "type": "axes", "x": { "range": [-3, 3] }, "y": { "range": [-1, 5] }, "width": 6, "height": 6 },
    "f": { "type": "function-plot", "axes": "ax", "expression": "x^2/2", "stroke": { "color": "#58c4dd" } },
    "m": { "type": "polygon", "points": [[0.18, 0], [-0.1, 0.1], [-0.1, -0.1]], "fill": { "color": "#ffd23f" }, "position": [-3, 2.5, 0], "rotation": [0, 0, -71.2926] }"##;

const CARRY: &str = r#"{ "type": "follow", "object": "m", "along": "f", "faces": "tangent", "easing_curve": "linear" }"#;

fn near(actual: &[f64], expected: &[f64], within: f64) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(a, e)| (a - e).abs() <= within)
}

#[test]
fn a_carry_sits_on_the_pens_end_of_a_create_of_a_plot() {
    let scene = scene(
        PARABOLA,
        &play(
            3.0,
            &[
                r#"{ "type": "create", "object": "f", "easing_curve": "linear" }"#,
                CARRY,
            ]
            .join(", "),
        ),
    );

    assert_eq!(validate(&scene).exit_code, 0);
    // A quarter of the parabola's length from x = -3 ends at x = -1.94, where its slope is -1.94.
    for (scene_time, position, turned) in [
        ("0.75", [-1.94, -0.118, 0.0], -62.73),
        ("1.5", [0.0, -2.0, 0.0], 0.0),
        ("2.25", [1.94, -0.118, 0.0], 62.73),
        ("3", [3.0, 2.5, 0.0], 71.29),
    ] {
        let inspected = inspect_at(&scene, scene_time);
        let carried = &inspected["objects"]["m"];
        assert!(
            near(&numbers(&carried["position"]), &position, 0.01),
            "{scene_time}: {} against {position:?}",
            carried["position"]
        );
        assert!(
            near(&numbers(&carried["rotation"]), &[0.0, 0.0, turned], 3.0),
            "{scene_time}: {} faces along the drawn curve",
            carried["rotation"]
        );
    }
}

#[test]
fn a_carry_along_a_plot_turns_a_little_at_every_frame_time() {
    let (_dir, path) = scene_file(&scene(PARABOLA, &play(3.0, CARRY)));

    let stages = sci_anim(&[
        "stages",
        path.to_str().unwrap(),
        "--from",
        "1.4",
        "--to",
        "1.6",
    ]);

    assert_eq!(stages.exit_code, 0, "{}", stages.stdout);
    let turned: Vec<f64> = stages.json["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|stage| stage["objects"]["m"]["rotation"][2].as_f64().unwrap())
        .collect();
    let expected = [
        -20.193, -17.111, -13.905, -10.591, -7.106, -3.576, 0.0, 3.576, 7.106, 10.591, 13.905,
        17.111, 20.193,
    ];
    assert!(
        near(&turned, &expected, 0.01),
        "the plot's curve is straight pieces, and the triangle still turns at every frame time: {turned:?}"
    );
    assert!(
        turned
            .windows(2)
            .all(|pair| (3.0..3.6).contains(&(pair[1] - pair[0]))),
        "{turned:?}"
    );
}

#[test]
fn a_carry_goes_with_an_along_object_that_moves() {
    let objects = r##""ring": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff" } }, "p": { "type": "dot", "radius": 0.1, "position": [1, 0, 0] }"##;
    let animations = [
        r#"{ "type": "move", "object": "ring", "to": [2, 0, 0], "easing_curve": "linear" }"#,
        r#"{ "type": "follow", "object": "p", "along": "ring", "easing_curve": "linear" }"#,
    ];
    let scene = scene(objects, &play(2.0, &animations.join(", ")));

    assert!(
        near(
            &numbers(&inspect_at(&scene, "0.5")["objects"]["p"]["position"]),
            &[0.5, 1.0, 0.0],
            0.002
        ),
        "a quarter round a ring moved by 0.5"
    );
    assert!(
        near(
            &numbers(&inspect_at(&scene, "1")["objects"]["p"]["position"]),
            &[0.0, 0.0, 0.0],
            0.002
        ),
        "half round a ring moved by 1"
    );
    assert!(
        near(
            &numbers(&inspect_at(&scene, "2")["objects"]["p"]["position"]),
            &[3.0, 0.0, 0.0],
            0.002
        ),
        "held at the end of the ring where it ends"
    );
}

#[test]
fn a_carry_along_a_plot_goes_with_its_axes() {
    let animations = [
        r#"{ "type": "move", "object": "ax", "to": [1, 1, 0], "easing_curve": "linear" }"#,
        CARRY,
    ];
    let scene = scene(PARABOLA, &play(3.0, &animations.join(", ")));

    assert!(
        near(
            &numbers(&inspect_at(&scene, "1.5")["objects"]["m"]["position"]),
            &[0.5, -1.5, 0.0],
            0.01
        ),
        "the parabola's lowest point, with the axes half way to [1, 1, 0]"
    );
    assert!(near(
        &numbers(&inspect_at(&scene, "3")["objects"]["m"]["position"]),
        &[4.0, 3.5, 0.0],
        0.01
    ));
}

#[test]
fn a_carry_along_a_mapped_shape_follows_its_plane_map() {
    let objects = r##""l": { "type": "line", "from": [0, 0], "to": [2, 0] }, "p": { "type": "dot", "radius": 0.1 }"##;
    let animations = [
        r#"{ "type": "linear-map", "objects": ["l"], "matrix": [[1, 0], [1, 1]], "easing_curve": "linear" }"#,
        r#"{ "type": "follow", "object": "p", "along": "l", "faces": "tangent", "easing_curve": "linear" }"#,
    ];
    let scene = scene(objects, &play(2.0, &animations.join(", ")));

    let halfway = inspect_at(&scene, "1");
    let end = inspect_at(&scene, "2");

    assert_eq!(validate(&scene).exit_code, 0);
    assert!(
        near(
            &numbers(&halfway["objects"]["p"]["position"]),
            &[1.0, 0.5, 0.0],
            0.002
        ),
        "(1, 0) under [[1, 0], [0.5, 1]]: {}",
        halfway["objects"]["p"]["position"]
    );
    assert!(
        near(
            &numbers(&halfway["objects"]["p"]["rotation"]),
            &[0.0, 0.0, 26.565],
            0.01
        ),
        "facing (1, 0.5)"
    );
    assert!(near(
        &numbers(&end["objects"]["p"]["position"]),
        &[2.0, 2.0, 0.0],
        0.002
    ));
    assert!(near(
        &numbers(&end["objects"]["p"]["rotation"]),
        &[0.0, 0.0, 45.0],
        0.01
    ));
}

#[test]
fn a_carry_along_a_tube_facing_the_tangent_turns_with_no_jump_between_frames() {
    let objects = r##""wave": { "type": "tube", "radius": 0.05, "curve": { "x": "u", "y": "sin(u)", "z": "0.5*cos(u)", "u_range": [-3, 3] } },
        "bead": { "type": "prism", "base": { "type": "polygon", "points": [[-0.2, -0.1], [0.2, -0.1], [0.2, 0.1], [-0.2, 0.1]] }, "height": 0.2,
                  "position": [-3, -0.1411, -0.495], "rotation": [-2.1862, -2.2105, -44.7035] }"##;
    let scene = scene(objects, &play(1.0, r#"{ "type": "follow", "object": "bead", "along": "wave", "faces": "tangent", "easing_curve": "linear" }"#)).replace(r#""background""#, r#""fps": 30, "background""#);

    let rotations: Vec<Vec<f64>> =
        (0..=30)
            .map(|frame| {
                numbers(
                    &inspect_at(&scene, &format!("{}", f64::from(frame) / 30.0))["objects"]["bead"]
                        ["rotation"],
                )
            })
            .collect();

    assert_eq!(
        validate(&scene).exit_code,
        0,
        "the bead is declared where the carry starts"
    );
    assert!(
        rotations
            .windows(2)
            .all(|pair| near(&pair[1], &pair[0], 30.0)),
        "each angle turns on by less than a twelfth of a turn a frame, never by a turn: {rotations:?}"
    );
    assert!(
        near(
            &numbers(&inspect_at(&scene, "1")["objects"]["bead"]["position"]),
            &[3.0, 0.141, -0.495],
            0.002
        ),
        "the curve's end"
    );
}

#[test]
fn a_carry_then_a_move_in_one_step_play_one_after_the_other() {
    let objects = r##""ring": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff" } }, "p": { "type": "dot", "radius": 0.1, "position": [1, 0, 0] }"##;
    let animations = |move_start: f64| {
        format!(
            r#"{{ "type": "follow", "object": "p", "along": "ring", "end": 3 }}, {{ "type": "move", "object": "p", "to": [0, 2, 0], "start": {move_start}, "end": 4 }}"#
        )
    };

    assert_eq!(
        validate(&scene(objects, &play(4.0, &animations(3.0)))).exit_code,
        0
    );
    assert_eq!(
        errors(objects, &play(4.0, &animations(2.0))),
        pairs(&[("/timeline/0/animations/1", "conflicting_animations")])
    );
}

#[test]
fn each_mistake_in_a_carry_is_an_error_at_its_field() {
    let objects = r##""ring": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff" } },
        "sq": { "type": "polygon", "points": [[-1, -1], [1, -1], [1, 1]], "stroke": { "color": "#ffffff" } },
        "p": { "type": "dot", "radius": 0.1, "position": [1, 0, 0] },
        "q": { "type": "dot", "radius": 0.1, "position": [1, 0, 0] },
        "title": { "type": "text", "text": "Ring", "position": [0, 3, 0] },
        "n": { "type": "text", "text": "1", "attach": { "object": "ring" } }"##;
    let follow = |fields: &str| play(1.0, &format!(r#"{{ "type": "follow", {fields} }}"#));
    let at = |field: &str| format!("/timeline/0/animations/0/{field}");

    assert_eq!(
        errors(
            objects,
            &follow(r#""object": "p", "along": "ring", "law": { "x": "1", "y": "0", "z": "0" }"#)
        ),
        pairs(&[(&at("along"), "invalid")])
    );
    assert_eq!(
        errors(
            objects,
            &follow(
                r#""object": "p", "faces": "tangent", "law": { "x": "1", "y": "0", "z": "0" }"#
            )
        ),
        pairs(&[(&at("faces"), "invalid")])
    );
    assert_eq!(
        errors(
            objects,
            &follow(r#""object": "p", "along": "ring", "from": "intro""#)
        ),
        pairs(&[(&at("from"), "invalid")])
    );
    assert_eq!(
        errors(
            objects,
            &follow(r#""sets": "camera-position", "along": "ring""#)
        ),
        pairs(&[(&at("sets"), "invalid")])
    );
    assert_eq!(
        errors(objects, &follow(r#""object": "p", "along": "title""#)),
        pairs(&[(&at("along"), "no_curve")])
    );
    assert_eq!(
        errors(objects, &follow(r#""object": "ring", "along": "ring""#)),
        pairs(&[(&at("along"), "invalid")])
    );
    assert_eq!(
        errors(objects, &follow(r#""object": "p", "along": "n""#)),
        pairs(&[(&at("along"), "invalid")])
    );
    assert_eq!(
        errors(objects, &follow(r#""object": "p", "along": "rings""#)),
        pairs(&[(&at("along"), "unknown_object")])
    );
    let chain = format!(
        "{}, {}",
        follow(r#""object": "p", "along": "ring""#),
        follow(r#""object": "q", "along": "p""#)
    );
    assert_eq!(
        errors(objects, &chain),
        pairs(&[("/timeline/1/animations/0/along", "invalid")]),
        "a carry along a carried object"
    );
    let morph = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "morph", "object": "ring", "into": "sq" }] }"#;
    assert_eq!(
        errors(
            objects,
            &format!("{morph}, {}", follow(r#""object": "p", "along": "ring""#))
        ),
        pairs(&[("/timeline/1/animations/0/along", "invalid")]),
        "a carry along a morphed object"
    );
    assert_eq!(
        errors(
            objects,
            &format!("{}, {morph}", follow(r#""object": "p", "along": "ring""#))
        ),
        pairs(&[("/timeline/1/animations/0/object", "invalid")]),
        "a morph of an object a carry goes along"
    );
}

#[test]
fn a_carry_must_start_where_its_object_stands_and_faces() {
    let objects = r##""ring": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff" } }, "p": { "type": "dot", "radius": 0.1 }"##;
    let timeline = play(
        1.0,
        r#"{ "type": "follow", "object": "p", "along": "ring", "faces": "tangent" }"#,
    );

    let run = validate(&scene(objects, &timeline));

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/0/animations/0/along", "carry_jump"),
            ("/timeline/0/animations/0/faces", "carry_jump")
        ])
    );
    let messages: Vec<&str> = run.json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap())
        .collect();
    assert!(
        messages[0].contains("declare `p`'s `position` as [1, 0, 0]"),
        "{messages:?}"
    );
    assert!(
        messages[1].contains("declare `p`'s `rotation` as [0, 0, 90]"),
        "{messages:?}"
    );
}
