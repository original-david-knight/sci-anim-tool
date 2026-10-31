//! Morphs read back from frames and reports as an agent reads them.
//!
//! The scenes are 400 by 400 pixels with the default camera, so a world point (x, y) lands at
//! pixel (200 + 48.28 x, 200 - 48.28 y).

mod common;

use common::{frame_at, inspect_at, near, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::json;

const BLACK: [u8; 4] = [0, 0, 0, 255];
const RED: [u8; 4] = [255, 0, 0, 255];

const SQUARE: &str = r##""sq": { "type": "polygon", "points": [[-1, -1], [1, -1], [1, 1], [-1, 1]], "fill": { "color": "#ff0000" } }"##;
const CIRCLE: &str = r##""ci": { "type": "circle", "radius": 1, "fill": { "color": "#0000ff" } }"##;
const MORPH: &str = r##"{ "type": "play", "duration": 2, "animations": [{ "type": "morph", "object": "sq", "into": "ci", "easing_curve": "linear" }] }"##;

fn square_into_circle() -> String {
    scene(&format!("{SQUARE}, {CIRCLE}"), MORPH)
}

#[test]
fn halfway_from_a_square_to_a_circle_the_corners_round_off_without_turning() {
    let scene = square_into_circle();

    let (_a, start) = frame_at(&scene, "0");
    let (_b, half) = frame_at(&scene, "1");
    let (_c, end) = frame_at(&scene, "2");

    // World (0.8, -0.8) is pixel (239, 239) and (0.9, -0.9) is (243, 243). Halfway the square's
    // corner (1, -1) has come to (0.8536, -0.8536), pixel (241, 241).
    assert_eq!(pixel(&start, 243, 243), RED, "inside the square's corner");
    assert_eq!(
        pixel(&half, 239, 239),
        [188, 0, 188, 255],
        "inside the rounded corner, in the mix of red and blue in linear light"
    );
    assert_eq!(pixel(&half, 243, 243), BLACK, "past the rounded corner");
    assert_eq!(pixel(&end, 239, 239), BLACK, "outside the circle");
    assert!(
        near(pixel(&end, 200, 247), [0, 0, 255, 255]),
        "inside the circle's bottom, in blue: {:?}",
        pixel(&end, 200, 247)
    );
}

#[test]
fn the_source_shows_until_the_step_ends_and_the_target_from_then() {
    let scene = scene(
        &format!("{SQUARE}, {CIRCLE}"),
        &format!(r##"{{ "type": "wait", "duration": 1 }}, {MORPH}"##),
    );

    let before = inspect_at(&scene, "0.5");
    let during = inspect_at(&scene, "2");
    let after = inspect_at(&scene, "3");

    assert_eq!(
        (
            before["objects"]["sq"]["opacity"].clone(),
            before["objects"]["ci"]["opacity"].clone()
        ),
        (json!(1.0), json!(0.0))
    );
    assert_eq!(
        (
            during["objects"]["sq"]["opacity"].clone(),
            during["objects"]["ci"]["opacity"].clone()
        ),
        (json!(1.0), json!(0.0))
    );
    assert_eq!(
        (
            after["objects"]["sq"]["opacity"].clone(),
            after["objects"]["ci"]["opacity"].clone()
        ),
        (json!(0.0), json!(1.0))
    );
    assert_eq!(
        during["objects"]["sq"]["morph"],
        json!({ "into": "ci", "phase": "during", "progress": 0.5 })
    );
    assert_eq!(before["objects"]["sq"]["morph"]["phase"], "before");
    assert_eq!(after["objects"]["sq"]["morph"]["phase"], "after");
    assert!(before["objects"]["ci"].get("morph").is_none());
}

#[test]
fn the_source_moves_to_where_the_target_is() {
    let scene = scene(
        &format!(
            r##"{SQUARE}, "ci": {{ "type": "circle", "radius": 1, "position": [3, 0, 0], "fill": {{ "color": "#0000ff" }} }}"##
        ),
        MORPH,
    );

    let half = inspect_at(&scene, "1");

    assert_eq!(half["objects"]["sq"]["position"], json!([1.5, 0.0, 0.0]));
}

#[test]
fn text_morphs_into_a_shape_glyph_by_glyph() {
    let scene = scene(
        r##""t": { "type": "math", "tex": "x^2" }, "ci": { "type": "circle", "radius": 1, "fill": { "color": "#0000ff" } }"##,
        r##"{ "type": "play", "duration": 2, "animations": [{ "type": "morph", "object": "t", "into": "ci" }] }"##,
    );

    let half = inspect_at(&scene, "1");

    assert_eq!(half["objects"]["t"]["morph"]["phase"], "during");
    assert!(
        half["objects"]["t"]["bounding_box"].is_object(),
        "the mix has a shape"
    );
    assert_eq!(
        half["objects"]["t"]["glyphs"], 2,
        "x and the superscript 2, though mid-morph the text is drawn as a path"
    );
}

fn morph_errors(objects: &str, timeline: &str) -> Vec<(String, String)> {
    let (_dir, path) = scene_file(&scene(objects, timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    scene_errors(&run)
}

fn play(animations: &str) -> String {
    format!(r##"{{ "type": "play", "duration": 1, "animations": [{animations}] }}"##)
}

#[test]
fn a_morph_names_what_it_cannot_reshape() {
    let objects = format!(r##"{SQUARE}, {CIRCLE}, "ball": {{ "type": "sphere", "radius": 1 }}"##);

    assert_eq!(
        morph_errors(
            &objects,
            &play(r##"{ "type": "morph", "object": "ball", "into": "ci" }"##)
        ),
        pairs(&[("/timeline/0/animations/0/object", "not_morphable")])
    );
    assert_eq!(
        morph_errors(
            &objects,
            &play(r##"{ "type": "morph", "object": "sq", "into": "ball" }"##)
        ),
        pairs(&[("/timeline/0/animations/0/into", "not_morphable")])
    );
    assert_eq!(
        morph_errors(
            &objects,
            &play(r##"{ "type": "morph", "object": "sq", "into": "sq" }"##)
        ),
        pairs(&[("/timeline/0/animations/0/into", "duplicate_object")])
    );
    assert_eq!(
        morph_errors(
            &objects,
            &play(r##"{ "type": "morph", "object": "sq", "into": "nope" }"##)
        ),
        pairs(&[("/timeline/0/animations/0/into", "unknown_object")])
    );
}

#[test]
fn an_object_is_morphed_once_and_made_once() {
    let objects = format!(
        r##"{SQUARE}, {CIRCLE}, "tri": {{ "type": "polygon", "points": [[0, 0], [1, 0], [0, 1]], "fill": {{ "color": "#00ff00" }} }}"##
    );
    let twice = format!(
        "{},{}",
        play(r##"{ "type": "morph", "object": "sq", "into": "ci" }"##),
        play(r##"{ "type": "morph", "object": "sq", "into": "tri" }"##)
    );
    let made_twice = format!(
        "{},{}",
        play(r##"{ "type": "morph", "object": "sq", "into": "ci" }"##),
        play(r##"{ "type": "morph", "object": "tri", "into": "ci" }"##)
    );
    let created_target = format!(
        "{},{}",
        play(r##"{ "type": "morph", "object": "sq", "into": "ci" }"##),
        play(r##"{ "type": "create", "object": "ci" }"##)
    );
    let objects_with_stroke = objects.replace(
        r##""radius": 1, "fill""##,
        r##""radius": 1, "stroke": { "color": "#ffffff" }, "fill""##,
    );

    assert_eq!(
        morph_errors(&objects, &twice),
        pairs(&[("/timeline/1/animations/0/object", "morph_order")])
    );
    assert_eq!(
        morph_errors(&objects, &made_twice),
        pairs(&[("/timeline/1/animations/0/into", "morph_order")])
    );
    assert_eq!(
        morph_errors(&objects_with_stroke, &created_target),
        pairs(&[("/timeline/1/animations/0/object", "morph_order")])
    );
}

#[test]
fn a_morph_chains_and_conflicts_with_other_animations_of_its_objects() {
    let objects = format!(
        r##"{SQUARE}, {CIRCLE}, "tri": {{ "type": "polygon", "points": [[0, 0], [1, 0], [0, 1]], "fill": {{ "color": "#00ff00" }} }}"##
    );
    let chain = format!(
        "{},{}",
        play(r##"{ "type": "morph", "object": "sq", "into": "ci" }"##),
        play(r##"{ "type": "morph", "object": "ci", "into": "tri" }"##)
    );
    let (_dir, path) = scene_file(&scene(&objects, &chain));
    assert_eq!(sci_anim(&["validate", path.to_str().unwrap()]).exit_code, 0);

    let clash = play(
        r##"{ "type": "morph", "object": "sq", "into": "ci" }, { "type": "move", "object": "sq", "to": [1, 0, 0] }"##,
    );
    assert_eq!(
        morph_errors(&objects, &clash),
        pairs(&[("/timeline/0/animations/1", "conflicting_animations")])
    );
    let clash_target = play(
        r##"{ "type": "morph", "object": "sq", "into": "ci" }, { "type": "fade", "object": "ci", "to": 0.5 }"##,
    );
    assert_eq!(
        morph_errors(&objects, &clash_target),
        pairs(&[("/timeline/0/animations/1", "conflicting_animations")])
    );
}

#[test]
fn a_source_that_is_yet_to_be_created_is_not_ready() {
    let objects = format!(r##"{SQUARE}, {CIRCLE}"##);
    let stroked = objects.replace(
        r##""fill": { "color": "#ff0000" }"##,
        r##""stroke": { "color": "#ff0000" }"##,
    );
    let both = play(
        r##"{ "type": "create", "object": "sq" }, { "type": "morph", "object": "sq", "into": "ci" }"##,
    );

    let errors = morph_errors(&stroked, &both);

    assert!(
        errors.contains(&(
            "/timeline/0/animations/1/object".to_string(),
            "not_ready".to_string()
        )),
        "{errors:?}"
    );
}
