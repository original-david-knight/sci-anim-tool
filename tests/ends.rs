//! Ends on objects: lines, arrows and springs placed by their ends, read back from reports as an
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

fn errors(objects: &str, timeline: &str) -> (Vec<(String, String)>, Vec<String>) {
    let (_dir, path) = scene_file(&scene(objects, timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    let messages = run.json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap().to_string())
        .collect();
    (scene_errors(&run), messages)
}

const BLOCK: &str = r##""block": { "type": "polygon", "points": [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]], "fill": { "color": "#58c4dd" } }"##;

const GRAVITY: &str = r##""gravity": { "type": "arrow", "from": { "object": "block" }, "to": { "object": "block", "offset": [0, -1.5, 0] } }"##;

const SLIDE: &str = r#"{ "type": "play", "duration": 2, "animations": [ { "type": "move", "object": "block", "to": [2, 1, 0], "easing_curve": "linear" } ] }"#;

#[test]
fn an_arrow_with_its_ends_on_a_block_goes_with_the_block() {
    let scene = scene(&format!("{BLOCK}, {GRAVITY}"), SLIDE);
    let halfway = inspect_at(&scene, "1");
    let gravity = &halfway["objects"]["gravity"];

    assert_eq!(numbers(&gravity["ends"]["from"]), [1.0, 0.5, 0.0]);
    assert_eq!(numbers(&gravity["ends"]["to"]), [1.0, -1.0, 0.0]);
    assert_eq!(
        numbers(&gravity["position"]),
        [1.0, 0.5, 0.0],
        "placed at its `from`"
    );
    assert_eq!(
        numbers(&gravity["rotation"]),
        [0.0, 0.0, -90.0],
        "pointing down, turned only in the frame's plane"
    );
    assert_eq!(
        numbers(&gravity["scale"]),
        [1.0, 1.0, 1.0],
        "drawn again between its ends, not stretched"
    );
}

#[test]
fn an_arrow_s_tip_keeps_its_length_as_its_ends_move_apart() {
    let objects = format!(
        r##"{BLOCK}, "pull": {{ "type": "arrow", "from": [-3, 0], "to": {{ "object": "block" }}, "tip_length": 0.3 }}"##
    );
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "move", "object": "block", "to": [3, 0, 0] } ] }"#;
    let scene = scene(&objects, timeline);
    let width = |inspected: &Value| {
        let world = &inspected["objects"]["pull"]["bounding_box"]["world"];
        numbers(&world["max"])[1] - numbers(&world["min"])[1]
    };

    assert_eq!(
        numbers(&inspect_at(&scene, "1")["objects"]["pull"]["ends"]["to"]),
        [3.0, 0.0, 0.0]
    );
    assert!(
        (width(&inspect_at(&scene, "0")) - width(&inspect_at(&scene, "1"))).abs() < 1e-3,
        "the tip is as wide when the arrow is twice as long"
    );
}

#[test]
fn a_spring_between_a_point_and_a_block_stretches_with_it() {
    let objects = format!(
        r##"{BLOCK}, "spring": {{ "type": "spring", "from": [-4, 0, 0], "to": {{ "object": "block", "offset": [-0.5, 0, 0] }}, "turns": 6, "radius": 0.2 }}"##
    );
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "move", "object": "block", "to": [2, 0, 0] } ] }"#;
    let scene = scene(&objects, timeline);
    let spring = |scene_time| inspect_at(&scene, scene_time)["objects"]["spring"].clone();

    assert_eq!(numbers(&spring("0")["ends"]["to"]), [-0.5, 0.0, 0.0]);
    assert_eq!(numbers(&spring("1")["ends"]["to"]), [1.5, 0.0, 0.0]);
    let width_of = |spring: &Value| {
        numbers(&spring["bounding_box"]["world"]["max"])[0]
            - numbers(&spring["bounding_box"]["world"]["min"])[0]
    };
    assert!(
        (width_of(&spring("1")) - 5.5).abs() < 0.05,
        "the coil runs the whole way between its ends: {}",
        width_of(&spring("1"))
    );
}

#[test]
fn a_shape_placed_by_its_ends_is_hidden_while_an_end_s_object_is() {
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "fade", "object": "block", "to": 0 } ] }"#;
    let scene = scene(&format!("{BLOCK}, {GRAVITY}"), timeline);

    assert_eq!(
        inspect_at(&scene, "0.5")["objects"]["gravity"]["opacity"],
        1.0
    );
    assert_eq!(
        inspect_at(&scene, "1")["objects"]["gravity"]["opacity"],
        0.0
    );
}

#[test]
fn a_shape_placed_by_its_ends_is_hidden_until_an_end_s_object_is_created() {
    let objects = r##""block": { "type": "polygon", "points": [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]], "stroke": { "color": "#58c4dd" } },
        "spring": { "type": "spring", "from": [-4, 0, 0], "to": { "object": "block" } }"##;
    let timeline = r#"{ "type": "wait", "duration": 1 }, { "type": "play", "duration": 1, "animations": [ { "type": "create", "object": "block" } ] }"#;
    let scene = scene(objects, timeline);

    assert_eq!(
        inspect_at(&scene, "0.5")["objects"]["spring"]["opacity"],
        0.0,
        "before the block's create"
    );
    assert_eq!(
        inspect_at(&scene, "1.5")["objects"]["spring"]["opacity"],
        1.0,
        "while it is drawn"
    );
}

#[test]
fn a_spring_makes_at_most_100_turns() {
    let (found, _) = errors(
        r#""s": { "type": "spring", "from": [-2, 0], "to": [2, 0], "turns": 1e30 }"#,
        "",
    );

    assert_eq!(found, pairs(&[("/objects/s/turns", "out_of_range")]));
}

#[test]
fn an_end_on_a_data_point_sits_where_the_axes_put_it() {
    let objects = r##""ax": { "type": "axes", "x": { "range": [0, 4] }, "y": { "range": [0, 4] }, "width": 4, "height": 4 },
        "p": { "type": "dot", "radius": 0.1, "axes": "ax", "position": [1, 1, 0] },
        "pointer": { "type": "line", "from": [3, 3], "to": { "object": "p" } }"##;

    let inspected = inspect_at(&scene(objects, ""), "0");

    assert_eq!(
        numbers(&inspected["objects"]["pointer"]["ends"]["to"]),
        [-1.0, -1.0, 0.0],
        "data (1, 1) of axes centered on the origin"
    );
}

#[test]
fn a_shape_placed_by_its_ends_has_no_transform_of_its_own() {
    let objects = format!(
        r##"{BLOCK}, "gravity": {{ "type": "arrow", "from": {{ "object": "block" }}, "to": [0, -2], "position": [1, 0, 0] }}, "line": {{ "type": "line", "from": {{ "object": "block" }}, "to": [3, 3] }}"##
    );
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "rotate", "object": "line", "to": [0, 0, 90] } ] }"#;

    assert_eq!(
        errors(&objects, timeline).0,
        pairs(&[("/objects/gravity/position", "placed_by_ends")])
    );
    let objects = format!(
        r##"{BLOCK}, "line": {{ "type": "line", "from": {{ "object": "block" }}, "to": [3, 3] }}"##
    );
    assert_eq!(
        errors(&objects, timeline).0,
        pairs(&[("/timeline/0/animations/0/object", "placed_by_ends")])
    );
}

#[test]
fn an_end_names_an_object_with_a_place_of_its_own() {
    let objects = r##""ax": { "type": "axes", "x": { "range": [0, 4] }, "y": { "range": [0, 4] }, "width": 4, "height": 4 },
        "curve": { "type": "function-plot", "axes": "ax", "expression": "x" },
        "a": { "type": "line", "from": { "object": "nowhere" }, "to": [1, 1] },
        "b": { "type": "spring", "from": { "object": "curve" }, "to": [1, 1] },
        "p": { "type": "dot", "radius": 0.1 }, "path": { "type": "trail", "object": "p" },
        "c": { "type": "arrow", "from": { "object": "path" }, "to": [1, 1] }"##;

    assert_eq!(
        errors(objects, "").0,
        pairs(&[
            ("/objects/a/from/object", "unknown_object"),
            ("/objects/b/from/object", "invalid"),
            ("/objects/c/from/object", "invalid")
        ])
    );
}

#[test]
fn a_shape_placed_by_its_ends_is_not_mapped_morphed_or_carried_along() {
    let objects = format!(
        r##"{BLOCK}, "line": {{ "type": "line", "from": {{ "object": "block" }}, "to": [3, 3] }}, "other": {{ "type": "line", "from": [0, 0], "to": [1, 0] }}, "p": {{ "type": "dot", "radius": 0.1 }}"##
    );
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [
        { "type": "linear-map", "objects": ["line"], "matrix": [[1, 1], [0, 1]] },
        { "type": "morph", "object": "other", "into": "line" },
        { "type": "follow", "object": "p", "along": "line" } ] }"#;

    assert_eq!(
        errors(&objects, timeline).0,
        pairs(&[
            ("/timeline/0/animations/0/objects/0", "not_mappable"),
            ("/timeline/0/animations/1/into", "placed_by_ends"),
            ("/timeline/0/animations/2/along", "invalid")
        ])
    );
}

#[test]
fn a_coil_named_by_its_old_names_points_to_the_spring() {
    let (found, messages) = errors(r#""c": { "type": "coil" }"#, "");

    assert_eq!(found, pairs(&[("/objects/c/type", "unknown_value")]));
    assert!(
        messages[0].contains("a coil between two points is a `spring`"),
        "{messages:?}"
    );
}

#[test]
fn two_fixed_ends_at_one_point_have_no_length() {
    let objects = r#""s": { "type": "spring", "from": [1, 1, 0], "to": [1, 1, 0] }, "a": { "type": "arrow", "from": [0, 0, 1], "to": [0, 0, 1] }"#;

    assert_eq!(
        errors(objects, "").0,
        pairs(&[("/objects/a/to", "invalid"), ("/objects/s/to", "invalid")])
    );
}

const WAVY: &str = r##""chain": { "type": "tube", "position": [1, 0, 0], "radius": 0.05, "curve": { "x": "u", "y": "sin(u + t)", "z": "0", "u_range": [0, 2] } }"##;

#[test]
fn an_end_on_a_tube_s_curve_keeps_to_its_u_as_the_shape_law_runs() {
    let objects = format!(
        r##"{WAVY}, "cross_link": {{ "type": "line", "from": {{ "object": "chain", "u": 1 }}, "to": [0, 3, 0] }}"##
    );
    let timeline = r#"{ "type": "play", "duration": 2, "animations": [ { "type": "follow", "object": "chain", "sets": "shape" } ] }"#;
    let scene = scene(&objects, timeline);
    let from = |scene_time| {
        numbers(&inspect_at(&scene, scene_time)["objects"]["cross_link"]["ends"]["from"])
    };

    assert_eq!(from("0"), [2.0, 0.841, 0.0], "x = 1 + u, y = sin(1)");
    assert_eq!(from("1"), [2.0, 0.909, 0.0], "y = sin(1 + 1)");
}

#[test]
fn an_end_on_a_curve_through_points_goes_with_the_tube_as_it_moves_and_turns() {
    let objects = r##""chain": { "type": "tube", "radius": 0.05, "curve": { "points": [[0, 0, 0], [1, 0, 0], [2, 0, 0]] } },
        "a": { "type": "line", "from": { "object": "chain", "u": 0.5 }, "to": [0, 0, 3] },
        "b": { "type": "spring", "from": { "object": "chain", "u": 1 }, "to": [0, 0, 3] }"##;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [
        { "type": "rotate", "object": "chain", "to": [0, 0, 90] },
        { "type": "move", "object": "chain", "to": [1, 0, 0] } ] }"#;
    let scene = scene(objects, timeline);
    let from = |scene_time, id: &str| {
        numbers(&inspect_at(&scene, scene_time)["objects"][id]["ends"]["from"])
    };

    assert_eq!(
        from("0", "a"),
        [1.0, 0.0, 0.0],
        "the middle curve point is at u = 0.5"
    );
    assert_eq!(from("0", "b"), [2.0, 0.0, 0.0]);
    assert_eq!(from("1", "a"), [1.0, 1.0, 0.0]);
    assert_eq!(from("1", "b"), [1.0, 2.0, 0.0]);
}

#[test]
fn an_end_on_a_plot_s_curve_keeps_to_its_x_as_the_axes_move_and_the_shape_law_runs() {
    let objects = r##""ax": { "type": "axes", "x": { "range": [0, 4] }, "y": { "range": [0, 4] }, "width": 4, "height": 4 },
        "level": { "type": "function-plot", "axes": "ax", "expression": "1 + t" },
        "rise": { "type": "arrow", "from": [1, -3, 0], "to": { "object": "level", "x": 3 } }"##;
    let timeline = r#"{ "type": "play", "duration": 2, "animations": [
        { "type": "follow", "object": "level", "sets": "shape" },
        { "type": "move", "object": "ax", "to": [1, 0, 0], "easing_curve": "linear" } ] }"#;
    let scene = scene(objects, timeline);
    let to = |scene_time| numbers(&inspect_at(&scene, scene_time)["objects"]["rise"]["ends"]["to"]);

    assert_eq!(
        to("0"),
        [1.0, -1.0, 0.0],
        "data (3, 1) on axes centered at the origin"
    );
    assert_eq!(
        to("1"),
        [1.5, 0.0, 0.0],
        "data (3, 2) on axes moved halfway"
    );
}

#[test]
fn an_end_on_a_curve_names_a_curve_and_a_place_on_it() {
    let objects = format!(
        r##"{WAVY}, "ax": {{ "type": "axes", "x": {{ "range": [0, 4] }}, "y": {{ "range": [0, 4] }}, "width": 4, "height": 4 }},
        "level": {{ "type": "function-plot", "axes": "ax", "expression": "1" }},
        "ball": {{ "type": "sphere", "radius": 0.2 }},
        "a": {{ "type": "line", "from": {{ "object": "chain", "u": 2.5 }}, "to": [0, 3, 0] }},
        "b": {{ "type": "line", "from": {{ "object": "ball", "u": 0.5 }}, "to": [0, 3, 0] }},
        "c": {{ "type": "line", "from": {{ "object": "level", "u": 0.5 }}, "to": [0, 3, 0] }},
        "d": {{ "type": "line", "from": {{ "object": "chain", "x": 1 }}, "to": [0, 3, 0] }},
        "e": {{ "type": "line", "from": [0, 3, 0], "to": {{ "object": "level", "x": 5 }} }},
        "f": {{ "type": "line", "from": [0, 3, 0], "to": {{ "object": "level", "x": 2 }} }}"##
    );
    let (found, messages) = errors(&objects, "");

    assert_eq!(
        found,
        pairs(&[
            ("/objects/a/from/u", "out_of_range"),
            ("/objects/b/from/object", "invalid"),
            ("/objects/c/from/object", "invalid"),
            ("/objects/d/from/object", "invalid"),
            ("/objects/e/to/x", "out_of_range")
        ])
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("u 2.5 lies outside `chain`'s u range, 0 to 2")),
        "{messages:?}"
    );
}

#[test]
fn a_plot_an_end_sits_on_is_not_morphed() {
    let objects = r##""ax": { "type": "axes", "x": { "range": [0, 4] }, "y": { "range": [0, 4] }, "width": 4, "height": 4 },
        "level": { "type": "function-plot", "axes": "ax", "expression": "1" },
        "other": { "type": "function-plot", "axes": "ax", "expression": "2" },
        "f": { "type": "line", "from": [0, 3, 0], "to": { "object": "level", "x": 2 } }"##;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [
        { "type": "morph", "object": "level", "into": "other" } ] }"#;

    assert_eq!(
        errors(objects, timeline).0,
        pairs(&[("/timeline/0/animations/0/object", "not_morphable")])
    );
}

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pubchem");

#[test]
fn an_end_on_an_atom_goes_with_the_molecule_as_it_moves_turns_and_scales() {
    let objects = format!(
        r##""water": {{ "type": "molecule", "file": "{FIXTURES}/water.sdf", "position": [2, 0, 0], "scale": [2, 2, 2] }},
        "bond": {{ "type": "line", "from": {{ "object": "water", "atom": 1 }}, "to": [0, 3, 0] }}"##
    );
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [
        { "type": "rotate", "object": "water", "to": [0, 0, 90] },
        { "type": "move", "object": "water", "to": [0, -1, 0] } ] }"#;
    let scene = scene(&objects, timeline);
    let from =
        |scene_time| numbers(&inspect_at(&scene, scene_time)["objects"]["bond"]["ends"]["from"]);

    assert_eq!(
        from("0"),
        [1.411, -0.436, 0.308],
        "the oxygen, 0.295, 0.218 and -0.154 from water's centroid, scaled twice"
    );
    assert_eq!(
        from("1"),
        [0.436, -1.589, 0.308],
        "turned a quarter turn about z"
    );
}

#[test]
fn an_end_on_an_atom_hides_while_a_reaction_uses_its_molecule_up_and_shows_on_a_product() {
    let molecule = |id: &str, sdf: &str, position: &str| {
        format!(
            r##""{id}": {{ "type": "molecule", "file": "{FIXTURES}/{sdf}.sdf", "position": {position} }}"##
        )
    };
    let objects = [
        molecule("ethylene", "ethylene", "[-2.6, 0, 0]"),
        molecule("hcl", "hydrogen-chloride", "[2.6, 0, 0]"),
        molecule("chloroethane", "chloroethane", "[0, 0, 0]"),
        r#""on_reactant": { "type": "line", "from": { "object": "hcl", "atom": 1 }, "to": [0, 3, 0] }"#.to_string(),
        r#""on_product": { "type": "line", "from": { "object": "chloroethane", "atom": 1 }, "to": [0, 3, 0] }"#.to_string(),
    ]
    .join(", ");
    let atom_map: Vec<String> = [
        ("ethylene", 1, 2),
        ("ethylene", 2, 3),
        ("ethylene", 3, 4),
        ("ethylene", 4, 5),
        ("ethylene", 5, 6),
        ("ethylene", 6, 7),
        ("hcl", 1, 1),
        ("hcl", 2, 8),
    ]
    .iter()
    .map(|(r, a, b)| format!(r#"{{ "reactant": {{ "object": "{r}", "atom": {a} }}, "product": {{ "object": "chloroethane", "atom": {b} }} }}"#))
    .collect();
    let timeline = format!(
        r#"{{ "type": "wait", "duration": 1 }},
           {{ "type": "play", "duration": 3, "animations": [ {{ "type": "reaction",
              "reactants": ["ethylene", "hcl"], "products": ["chloroethane"], "atom_map": [{}] }} ] }},
           {{ "type": "wait", "duration": 1 }}"#,
        atom_map.join(", ")
    );
    let scene = scene(&objects, &timeline);
    let opacities = |scene_time| {
        let objects = &inspect_at(&scene, scene_time)["objects"];
        [&objects["on_reactant"], &objects["on_product"]].map(|o| o["opacity"].as_f64().unwrap())
    };

    assert_eq!(opacities("0.5"), [1.0, 0.0], "before the reaction");
    assert_eq!(opacities("2.5"), [0.0, 0.0], "while it plays");
    assert_eq!(opacities("4.5"), [0.0, 1.0], "after it");
}

#[test]
fn an_end_on_an_atom_names_a_molecule_and_one_of_its_atoms() {
    let objects = format!(
        r##""water": {{ "type": "molecule", "file": "{FIXTURES}/water.sdf" }},
        "ball": {{ "type": "sphere", "radius": 0.2 }},
        "a": {{ "type": "line", "from": {{ "object": "water", "atom": 4 }}, "to": [0, 3, 0] }},
        "b": {{ "type": "line", "from": {{ "object": "ball", "atom": 1 }}, "to": [0, 3, 0] }},
        "d": {{ "type": "line", "from": [0, 3, 0], "to": {{ "object": "nowhere", "atom": 1 }} }}"##
    );
    let (found, messages) = errors(&objects, "");
    let atom_0 = format!(
        r##""water": {{ "type": "molecule", "file": "{FIXTURES}/water.sdf" }},
        "c": {{ "type": "line", "from": [0, 3, 0], "to": {{ "object": "water", "atom": 0 }} }}"##
    );

    assert_eq!(
        found,
        pairs(&[
            ("/objects/a/from/atom", "no_such_atom"),
            ("/objects/b/from/object", "invalid"),
            ("/objects/d/to/object", "unknown_object")
        ])
    );
    assert_eq!(
        errors(&atom_0, "").0,
        pairs(&[("/objects/c/to/atom", "out_of_range")]),
        "atoms are numbered from 1"
    );
    assert!(
        messages
            .iter()
            .any(|m| m
                .contains("`water` has 3 atoms, numbered 1 to 3, and an end of `a` names atom 4")),
        "{messages:?}"
    );
}

#[test]
fn a_dash_pattern_is_two_lengths_of_at_least_a_hundredth() {
    let objects = r##""a": { "type": "line", "from": [0, 0], "to": [1, 0], "stroke": { "color": "#ffffff", "dash": [0, 0.1] } },
        "b": { "type": "line", "from": [0, 0], "to": [1, 0], "stroke": { "color": "#ffffff", "dash": [0.1, -0.1] } },
        "c": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff", "dash": [0.1] } }"##;
    let trail = r##""ball": { "type": "sphere", "radius": 0.2 },
        "d": { "type": "trail", "object": "ball", "stroke": { "color": "#ffffff", "dash": [0.1, 0.1] } }"##;

    assert_eq!(
        errors(objects, "").0,
        pairs(&[
            ("/objects/a/stroke/dash/0", "out_of_range"),
            ("/objects/b/stroke/dash/1", "out_of_range"),
            ("/objects/c/stroke/dash", "wrong_length")
        ])
    );
    assert_eq!(
        errors(trail, "").0,
        pairs(&[("/objects/d/stroke/dash", "invalid")]),
        "a trail faces the camera and is drawn unbroken"
    );
}

#[test]
fn a_dashed_line_keeps_its_dashes_where_they_are_as_its_ends_move_apart() {
    let objects = r##""post": { "type": "dot", "radius": 0.01, "position": [2, 0, 0] },
        "bond": { "type": "line", "from": [-2, 0, 0], "to": { "object": "post" }, "stroke": { "color": "#ffffff", "width": 0.1, "dash": [0.5, 0.5] } }"##;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "move", "object": "post", "to": [3, 0, 0] } ] }"#;
    let scene = scene(objects, timeline);
    let ends = &inspect_at(&scene, "0")["objects"]["bond"]["ends"];
    let (left, right) = (numbers(&ends["from_pixels"]), numbers(&ends["to_pixels"]));
    let pixel_at = |x: f64| (left[0] + (x + 2.0) / 4.0 * (right[0] - left[0])).round() as usize;
    let row = left[1].round() as usize;
    let lit = |rgba: &[u8], x: f64| common::pixel(rgba, pixel_at(x), row)[0] > 128;

    for scene_time in ["0", "1"] {
        let (_dir, rgba) = common::frame_at(&scene, scene_time);
        assert_eq!(
            [-1.75, -1.25, -0.75, -0.25, 1.25].map(|x| lit(&rgba, x)),
            [true, false, true, false, true],
            "dashes from -2 to -1.5, -1 to -0.5, 0 to 0.5 and 1 to 1.5 at scene time {scene_time}"
        );
    }
}
