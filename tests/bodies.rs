//! Bodies: a simulation that moves objects as rigid bodies, read back from reports as an agent
//! reads them.

mod common;

use common::{inspect_at, pairs, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

/// A 2 kg cart at 3 m/s toward a 1 kg cart at rest, 1 apart, readouts of their total momentum
/// and kinetic energy, and the dot at the tip of the first cart's velocity arrow.
const CARTS: &str = r#"
    "a": { "type": "polygon", "points": [[-0.5, -0.3], [0.5, -0.3], [0.5, 0.3], [-0.5, 0.3]], "position": [-2, 0, 0] },
    "b": { "type": "polygon", "points": [[-0.5, -0.3], [0.5, -0.3], [0.5, 0.3], [-0.5, 0.3]], "position": [0, 0, 0] },
    "a_tip": { "type": "dot", "radius": 0.05, "position": [-0.5, 0, 0] },
    "momentum": { "type": "readout", "value": 6, "decimals": 6 },
    "energy": { "type": "readout", "value": 9, "decimals": 6 }"#;

fn carts(restitution: &str) -> String {
    let simulate = format!(
        r#"{{ "type": "play", "duration": 2, "animations": [ {{ "type": "simulate", "gravity": 0,
            "bodies": {{ "a": {{ "mass": 2, "velocity": [3, 0] }}, "b": {{ "mass": 1, "restitution": {restitution} }} }},
            "sets": [
              {{ "object": "a_tip", "position": ["a.x + 0.5*a.vx", "a.y", "0"] }},
              {{ "object": "momentum", "value": "a.px + b.px" }},
              {{ "object": "energy", "value": "a.ke + b.ke" }}
            ] }} ] }}"#
    );
    scene(CARTS, &simulate)
}

fn value(stage: &Value, readout: &str) -> f64 {
    stage["objects"][readout]["readout"]["value"]
        .as_f64()
        .unwrap()
}

fn x(stage: &Value, object: &str) -> f64 {
    stage["objects"][object]["position"][0].as_f64().unwrap()
}

fn errors(objects: &str, simulate: &str) -> Vec<(String, String)> {
    let timeline = format!(
        r#"{{ "type": "play", "duration": 1, "animations": [ {{ "type": "simulate", {simulate} }} ] }}"#
    );
    let (_dir, path) = scene_file(&scene(objects, &timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    scene_errors(&run)
}

#[test]
fn an_elastic_collision_keeps_the_momentum_and_kinetic_energy_the_readouts_show() {
    let scene = carts("1");

    let after = inspect_at(&scene, "2");

    // The gap of 1 closes at 3 m/s; then a moves at 1 m/s and b at 4 m/s.
    assert!((value(&after, "momentum") - 6.0).abs() < 1e-9);
    assert!((value(&after, "energy") - 9.0).abs() < 1e-9);
    let (a, b) = (x(&after, "a"), x(&after, "b"));
    assert!(
        (a - (-2.0 + 1.0 + (2.0 - 1.0 / 3.0) * 1.0)).abs() < 0.01,
        "{a}"
    );
    assert!((b - (2.0 - 1.0 / 3.0) * 4.0).abs() < 0.01, "{b}");
    assert!(
        (x(&after, "a_tip") - (a + 0.5)).abs() < 1e-6,
        "the tip sits at a.x + 0.5·1"
    );
}

#[test]
fn a_perfectly_inelastic_collision_loses_a_third_of_the_kinetic_energy() {
    let after = inspect_at(&carts("0"), "2");

    assert!((value(&after, "momentum") - 6.0).abs() < 1e-9);
    // m1·m2/(2(m1 + m2))·v² = 3 J of the 9 J.
    assert!((value(&after, "energy") - 6.0).abs() < 1e-9);
}

#[test]
fn validate_names_each_collision_and_its_scene_time() {
    let (_dir, path) = scene_file(&carts("1"));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    let collisions = run.json["collisions"].as_array().unwrap();
    assert_eq!(collisions.len(), 1, "{}", run.stdout);
    assert_eq!(collisions[0]["bodies"], serde_json::json!(["a", "b"]));
    assert_eq!(collisions[0]["path"], "/timeline/0/animations/0");
    let scene_time = collisions[0]["scene_time"].as_f64().unwrap();
    assert!((scene_time - 1.0 / 3.0).abs() < 1.0 / 60.0, "{scene_time}");
}

#[test]
fn the_same_scene_gives_the_same_stages_on_every_run() {
    let objects = r#"
        "ball": { "type": "circle", "radius": 0.2, "position": [-3, 2, 0] },
        "crate": { "type": "polygon", "points": [[-0.4, -0.4], [0.4, -0.4], [0.4, 0.4], [-0.4, 0.4]], "position": [0.5, -1.1, 0], "rotation": [0, 0, 20] },
        "ground": { "type": "line", "from": [-6, 0], "to": [6, 0], "position": [0, -1.6, 0] }"#;
    let timeline = r#"{ "type": "play", "duration": 3, "animations": [ { "type": "simulate",
        "bodies": { "ball": { "mass": 1, "velocity": [4, 1], "restitution": 0.7 }, "crate": { "mass": 3, "friction": 0.4 }, "ground": { "fixed": true } } } ] }"#;
    let (_dir, path) = scene_file(&scene(objects, timeline));
    let path = path.to_str().unwrap();

    let first = sci_anim(&["stages", path, "--from", "0", "--to", "3"]);
    let second = sci_anim(&["stages", path, "--from", "0", "--to", "3"]);

    assert_eq!(first.exit_code, 0, "{}", first.stdout);
    assert_eq!(first.stdout, second.stdout);
}

#[test]
fn a_body_starts_where_the_timeline_left_it() {
    let objects = r#""ball": { "type": "circle", "radius": 0.2, "position": [0, 0, 0] }"#;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "move", "object": "ball", "to": [2, 0, 0] } ] },
        { "type": "play", "duration": 1, "animations": [ { "type": "simulate", "gravity": 0, "bodies": { "ball": { "mass": 1, "velocity": [1, 0] } } } ] }"#;

    let after = inspect_at(&scene(objects, timeline), "2");

    assert!((x(&after, "ball") - 3.0).abs() < 1e-9);
}

#[test]
fn a_body_is_a_drawn_convex_shape_and_a_fixed_body_has_no_motion() {
    let objects = r#"
        "ball": { "type": "circle", "radius": 0.2 },
        "label": { "type": "text", "text": "hi" },
        "dent": { "type": "polygon", "points": [[0, 0], [2, 0], [1, 0.2], [1, 2]] },
        "rail": { "type": "line", "from": [0, 0], "to": [1, 0] },
        "ground": { "type": "line", "from": [-5, -1], "to": [5, -1] }"#;
    let simulate = r#""bodies": { "ball": {}, "label": { "mass": 1 }, "dent": { "mass": 1 }, "rail": { "mass": 1 }, "ground": { "fixed": true, "mass": 1 }, "ghost": { "fixed": true } }"#;

    assert_eq!(
        errors(objects, simulate),
        pairs(&[
            ("/timeline/0/animations/0/bodies/ball/mass", "missing_field"),
            ("/timeline/0/animations/0/bodies/dent", "invalid"),
            ("/timeline/0/animations/0/bodies/ghost", "unknown_object"),
            ("/timeline/0/animations/0/bodies/ground/mass", "invalid"),
            ("/timeline/0/animations/0/bodies/label", "invalid"),
            ("/timeline/0/animations/0/bodies/rail", "invalid"),
        ])
    );
}

#[test]
fn a_simulation_moves_bodies_or_integrates_variables_and_owns_each_body_s_place() {
    let objects = r#""ball": { "type": "circle", "radius": 0.2 }"#;

    assert_eq!(
        errors(
            objects,
            r#""variables": { "x": 0 }, "rates": { "x": "1" }, "bodies": { "ball": { "mass": 1 } }"#
        ),
        pairs(&[("/timeline/0/animations/0/variables", "invalid")])
    );
    assert_eq!(
        errors(
            objects,
            r#""bodies": { "ball": { "mass": 1 } }, "sets": [ { "object": "ball", "position": ["0", "0", "0"] } ]"#
        ),
        pairs(&[("/timeline/0/animations/0/sets/0/position", "invalid")])
    );
    assert_eq!(
        errors(
            objects,
            r#""bodies": { "ball": { "mass": 1 } }, "sets": [ { "object": "ball", "rotation": ["0", "0", "ball.speed + ball.mass"] } ]"#
        ),
        pairs(&[("/timeline/0/animations/0/sets/0/rotation", "invalid")])
    );
}

#[test]
fn a_body_moves_at_its_drawn_size_in_the_xy_plane() {
    let objects = r#""ball": { "type": "circle", "radius": 0.2, "scale": [2, 2, 1] }"#;

    assert_eq!(
        errors(objects, r#""bodies": { "ball": { "mass": 1 } }"#),
        pairs(&[("/timeline/0/animations/0/bodies/ball", "invalid")])
    );
}

#[test]
fn a_set_reads_a_body_s_quantities_by_name() {
    let objects = r#""ball": { "type": "circle", "radius": 0.2 }, "speed": { "type": "readout" }"#;

    let found = errors(
        objects,
        r#""bodies": { "ball": { "mass": 1 } }, "sets": [ { "object": "speed", "value": "ball.velocity" } ]"#,
    );

    assert_eq!(
        found,
        pairs(&[("/timeline/0/animations/0/sets/0/value", "bad_format")])
    );
}
