//! Prisms, revolutions and the create of a sphere, read back from frames and reports as an agent
//! reads them.
//!
//! The scenes are 400 by 400 pixels on black. Where the camera stands at `[0, 4, 10]` it looks
//! down on the solids, so a box shows its top above its front face.

mod common;

use common::{frame_at, inspect_at, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

const BLACK: [u8; 4] = [0, 0, 0, 255];

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n.as_f64().unwrap() * 1000.0).round() / 1000.0)
        .collect()
}

/// A 2 by 2 by 2 red box standing on y = -1, seen from above.
const BOX: &str = r##""b": { "type": "prism", "position": [0, -1, 0],
    "base": { "type": "polygon", "points": [[-1, -1], [1, -1], [1, 1], [-1, 1]] }, "height": 2,
    "material": { "type": "standard", "color": "#ff0000" } }"##;

fn from_above(objects: &str, timeline: &str) -> String {
    scene(objects, timeline).replacen(
        r#""objects""#,
        r#""camera": { "position": [0, 4, 10] }, "objects""#,
        1,
    )
}

/// The rows of the frame's middle column where `channel` is lit, top first.
fn lit_rows(rgba: &[u8], channel: usize) -> Vec<usize> {
    (0..400)
        .filter(|&y| pixel(rgba, 200, y)[channel] > 100)
        .collect()
}

fn validate(scene: &str) -> common::Run {
    let (_dir, path) = scene_file(scene);
    sci_anim(&["validate", path.to_str().unwrap()])
}

#[test]
fn a_box_shows_a_lighter_top_than_front_with_a_dark_edge_between() {
    let (_dir, rgba) = frame_at(&from_above(BOX, ""), "0");

    let column: Vec<[u8; 4]> = (0..400).map(|y| pixel(&rgba, 200, y)).collect();
    let red = lit_rows(&rgba, 0);
    let (top, front) = (column[red[0] + 5], column[*red.last().unwrap() - 5]);
    assert!(
        top[0] > front[0],
        "the key light lights the top more than the front: top {top:?}, front {front:?}"
    );
    let between = (red[0] + 5..*red.last().unwrap() - 5)
        .map(|y| column[y])
        .find(|p| u16::from(p[0]) + 60 < u16::from(front[0]));
    assert!(
        between.is_some(),
        "an edge where the top meets the front: {:?}",
        &column[red[0]..=*red.last().unwrap()]
    );
}

#[test]
fn a_fading_box_keeps_its_edges() {
    let faded = BOX.replace(r#""height": 2,"#, r#""height": 2, "opacity": 0.6,"#);
    let (_dir, rgba) = frame_at(&from_above(&faded, ""), "0");

    let column: Vec<[u8; 4]> = (0..400).map(|y| pixel(&rgba, 200, y)).collect();
    let red = lit_rows(&rgba, 0);
    let front = column[*red.last().unwrap() - 5];
    let between = (red[0] + 5..*red.last().unwrap() - 5)
        .map(|y| column[y])
        .find(|p| u16::from(p[0]) + 40 < u16::from(front[0]));
    assert!(
        between.is_some(),
        "an edge where the top meets the front: {:?}",
        &column[red[0]..=*red.last().unwrap()]
    );
}

#[test]
fn a_mirrored_box_is_lit_as_the_box_is() {
    let mirrored = BOX.replace(
        r#""position": [0, -1, 0]"#,
        r#""position": [0, -1, 0], "scale": [-1, 1, 1]"#,
    );

    let (_a, plain) = frame_at(&from_above(BOX, ""), "0");
    let (_b, flipped) = frame_at(&from_above(&mirrored, ""), "0");

    assert_eq!(
        pixel(&flipped, 200, 150),
        pixel(&plain, 200, 150),
        "a negative scale keeps the normals pointing out"
    );
}

#[test]
fn a_create_raises_a_prism_from_its_base() {
    let scene = from_above(
        BOX,
        r##"{ "type": "play", "duration": 2, "animations": [{ "type": "create", "object": "b", "easing_curve": "linear" }] }"##,
    );

    let (_a, start) = frame_at(&scene, "0");
    let (_b, half) = frame_at(&scene, "1");
    let (_c, end) = frame_at(&scene, "2");
    let inspected = inspect_at(&scene, "1");

    assert!(
        lit_rows(&start, 0).is_empty(),
        "nothing shows before the create begins"
    );
    let (half_rows, end_rows) = (lit_rows(&half, 0), lit_rows(&end, 0));
    assert_eq!(
        half_rows.last(),
        end_rows.last(),
        "it stands on its base throughout"
    );
    assert!(
        half_rows[0] > end_rows[0] + 20,
        "half risen, its top lower: {} against {}",
        half_rows[0],
        end_rows[0]
    );
    let b = &inspected["objects"]["b"];
    assert_eq!(b["created"], 0.5);
    assert_eq!(
        numbers(&b["bounding_box"]["world"]["min"]),
        [-1.0, -1.0, -1.0]
    );
    assert_eq!(
        numbers(&b["bounding_box"]["world"]["max"]),
        [1.0, 1.0, 1.0],
        "the whole box, which the create will raise, as for a flat shape"
    );
    assert_eq!(b["colors"]["color"], "#ff0000");
}

#[test]
fn a_revolution_through_a_quarter_turn_lies_between_plus_x_and_minus_z() {
    let cylinder = r##""c": { "type": "revolution", "angle": 90,
        "profile": { "type": "polygon", "points": [[0, 0], [1, 0], [1, 2], [0, 2]] } }"##;

    let inspected = inspect_at(&scene(cylinder, ""), "0");

    let world = &inspected["objects"]["c"]["bounding_box"]["world"];
    assert_eq!(numbers(&world["min"]), [0.0, 0.0, -1.0]);
    assert_eq!(numbers(&world["max"]), [1.0, 2.0, 0.0]);
}

#[test]
fn a_torus_is_the_revolution_of_a_circle_beside_the_axis() {
    let torus = r##""t": { "type": "revolution", "profile": { "type": "circle", "radius": 0.25, "center": [1, 0] } }"##;

    let inspected = inspect_at(&scene(torus, ""), "0");

    let world = &inspected["objects"]["t"]["bounding_box"]["world"];
    assert_eq!(numbers(&world["min"]), [-1.25, -0.25, -1.25]);
    assert_eq!(numbers(&world["max"]), [1.25, 0.25, 1.25]);
}

#[test]
fn a_sphere_can_be_created_and_shows_nothing_before() {
    let orb = r##""orb": { "type": "sphere", "radius": 1, "material": { "type": "standard", "color": "#ff0000" } }"##;
    let scene = scene(
        orb,
        r##"{ "type": "play", "duration": 2, "animations": [{ "type": "create", "object": "orb", "easing_curve": "linear" }] }"##,
    );

    let (_a, start) = frame_at(&scene, "0");
    let (_b, end) = frame_at(&scene, "2");
    let half = inspect_at(&scene, "1");

    assert_eq!(pixel(&start, 200, 200), BLACK);
    assert_eq!(
        pixel(&end, 200, 200),
        [213, 0, 0, 255],
        "whole, and lit as a sphere is"
    );
    assert_eq!(half["objects"]["orb"]["created"], 0.5);
}

#[test]
fn a_recolor_changes_a_solids_color() {
    let scene = from_above(
        BOX,
        r##"{ "type": "play", "duration": 1, "animations": [{ "type": "recolor", "object": "b", "color": "#00ff00" }] }"##,
    );

    let inspected = inspect_at(&scene, "1");
    let (_dir, rgba) = frame_at(&scene, "1");

    assert_eq!(inspected["objects"]["b"]["colors"]["color"], "#00ff00");
    let front = pixel(&rgba, 200, *lit_rows(&rgba, 1).last().unwrap() - 5);
    assert!(front[1] > 100 && front[0] == 0, "{front:?}");
}

#[test]
fn a_faded_solid_blends_with_the_background_edges_and_all() {
    let faded = BOX.replace(r#""height": 2,"#, r#""height": 2, "opacity": 0.5,"#);

    let (_a, opaque) = frame_at(&from_above(BOX, ""), "0");
    let (_b, half) = frame_at(&from_above(&faded, ""), "0");

    let front = *lit_rows(&opaque, 0).last().unwrap() - 5;
    let (seen, whole) = (pixel(&half, 200, front), pixel(&opaque, 200, front));
    assert!(
        seen[0] > 0 && seen[0] < whole[0],
        "half {seen:?}, whole {whole:?}"
    );
}

#[test]
fn a_profile_left_of_the_axis_is_a_scene_error() {
    let run = validate(&scene(
        r##""c": { "type": "revolution", "profile": { "type": "circle", "radius": 1 } }"##,
        "",
    ));

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/c/profile", "out_of_range")])
    );
    assert!(
        run.json["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("x = -1.0000"),
        "{}",
        run.stdout
    );
}

#[test]
fn a_base_with_no_inside_is_a_scene_error() {
    let run = validate(&scene(
        r##""b": { "type": "prism", "height": 1, "base": { "type": "polygon", "points": [[0, 0], [1, 0], [2, 0]] } }"##,
        "",
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/b/base/points", "invalid")])
    );
}

#[test]
fn a_see_through_solid_is_a_scene_error() {
    let run = validate(&scene(
        r##""b": { "type": "prism", "height": 1, "base": { "type": "circle", "radius": 1 }, "material": { "type": "standard", "color": "#ffffff", "opacity": 0.5 } }"##,
        "",
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/b/material/opacity", "invalid")])
    );
}

#[test]
fn a_box_or_a_cone_type_names_the_solid_that_draws_it() {
    let run = validate(&scene(
        r##""b": { "type": "box" }, "k": { "type": "cone" }"##,
        "",
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/b/type", "unknown_value"),
            ("/objects/k/type", "unknown_value")
        ])
    );
    let messages: Vec<&str> = run.json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap())
        .collect();
    assert!(
        messages[0].ends_with("a box is a `prism` whose `base` is a square `polygon`"),
        "{}",
        messages[0]
    );
    assert!(
        messages[1].contains("a cone is a `revolution`"),
        "{}",
        messages[1]
    );
}

#[test]
fn a_line_is_not_a_base() {
    let run = validate(&scene(
        r##""b": { "type": "prism", "height": 1, "base": { "type": "line", "from": [0, 0], "to": [1, 0] } }"##,
        "",
    ));

    assert_eq!(
        scene_errors(&run)[0],
        (
            "/objects/b/base/type".to_string(),
            "unknown_value".to_string()
        ),
        "{}",
        run.stdout
    );
}

#[test]
fn a_solid_cannot_morph_be_mapped_or_take_a_stroke_color() {
    let objects = format!(
        r##"{BOX}, "s": {{ "type": "circle", "radius": 1, "stroke": {{ "color": "#ffffff" }} }}"##
    );
    let run = validate(&scene(
        &objects,
        r##"{ "type": "play", "duration": 1, "animations": [{ "type": "morph", "object": "b", "into": "s" }] },
            { "type": "play", "duration": 1, "animations": [{ "type": "linear-map", "objects": ["b"], "matrix": [[2, 0], [0, 1]] }] },
            { "type": "play", "duration": 1, "animations": [{ "type": "recolor", "object": "b", "stroke": "#00ff00" }] }"##,
    ));

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/0/animations/0/object", "not_morphable"),
            ("/timeline/1/animations/0/objects/0", "not_mappable"),
            ("/timeline/2/animations/0/stroke", "not_recolorable"),
        ])
    );
}
