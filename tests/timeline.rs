//! Frames in the middle of steps, read back as an agent reads them.
//!
//! The scenes are 400 by 400 pixels with the default camera, at z = 10 with a 45 degree field of
//! view, so one world unit in the plane z = 0 is 200 / (10 tan 22.5) = 48.28 pixels.

mod common;

use common::{frame_at, near, pixel};

const MOVING: &str = r##"{
  "scene_config": { "width": 400, "height": 400, "background": "#000000" },
  "objects": { "orb": { "type": "sphere", "radius": 0.5, "material": { "type": "standard", "color": "#ff0000" } } },
  "timeline": [
    { "type": "wait", "duration": 1 },
    { "type": "play", "duration": 2, "animations": [{ "type": "move", "object": "orb", "to": [4, 0, 0], "easing_curve": "linear" }] },
    { "type": "play", "duration": 2, "animations": [{ "type": "move", "object": "orb", "to": [0, 0, 0], "easing_curve": "slow-both" }] }
  ]
}"##;

#[test]
fn a_frame_mid_move_shows_the_object_where_the_easing_curve_puts_it() {
    let red = |rgba: &[u8], x| matches!(pixel(rgba, x, 200), [r, 0, 0, 255] if r > 200);

    let (_a, waiting) = frame_at(MOVING, "0.5");
    let (_b, halfway) = frame_at(MOVING, "2");
    let (_c, quarter_back) = frame_at(MOVING, "3.5");

    assert!(red(&waiting, 200), "nothing moves during the wait");
    assert!(
        red(&halfway, 297) && !red(&halfway, 200),
        "linear: x = 2 at half the step, 200 + 2 * 48.28"
    );
    assert!(
        red(&quarter_back, 363) && !red(&quarter_back, 335),
        "slow-both at a quarter: x = 4 - 4 * 0.15625 = 3.375, 200 + 3.375 * 48.28 = 363"
    );
}

#[test]
fn a_faded_sphere_blends_its_color_with_the_background() {
    let scene = r##"{
  "scene_config": { "width": 400, "height": 400, "background": "#1e1e1e" },
  "objects": { "orb": { "type": "sphere", "radius": 1, "opacity": 0, "material": { "type": "standard", "color": "#ff0000" } } },
  "timeline": [{ "type": "play", "duration": 2, "animations": [{ "type": "fade", "object": "orb", "to": 1, "easing_curve": "linear" }] }]
}"##;

    let (_a, start) = frame_at(scene, "0");
    let (_b, halfway) = frame_at(scene, "1");
    let (_c, end) = frame_at(scene, "2");

    assert_eq!(
        pixel(&start, 200, 200),
        [0x1e, 0x1e, 0x1e, 255],
        "a fade in starts hidden"
    );
    assert!(
        near(pixel(&halfway, 200, 200), [158, 19, 19, 255]),
        "{:?}",
        pixel(&halfway, 200, 200)
    );
    assert_eq!(
        pixel(&end, 200, 200),
        [213, 0, 0, 255],
        "the middle faces the camera, which the key light shades to two thirds in linear light"
    );
}

#[test]
fn a_faded_circle_shows_its_stroke_over_the_background_not_over_its_fill() {
    let scene = r##"{
  "scene_config": { "width": 400, "height": 400, "background": "#0000ff" },
  "objects": { "c": { "type": "circle", "radius": 3, "opacity": 0.5,
                      "stroke": { "color": "#ff0000", "width": 0.6 }, "fill": { "color": "#00ff00" } } }
}"##;

    let (_a, faded) = frame_at(scene, "0");

    assert!(
        near(pixel(&faded, 200, 200), [0, 188, 188, 255]),
        "fill at half over blue: {:?}",
        pixel(&faded, 200, 200)
    );
    assert!(
        near(pixel(&faded, 335, 200), [188, 0, 188, 255]),
        "stroke at half over blue, 2.8 units out where the fill lies under it: {:?}",
        pixel(&faded, 335, 200)
    );
}

/// An agent lays sound made elsewhere against the video at each step's start, so `validate`
/// reports every step's span in scene time, summed in order as the frames are.
#[test]
fn validate_reports_when_each_step_starts_and_ends() {
    let (_dir, path) = common::scene_file(MOVING);

    let run = common::sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        run.json,
        serde_json::json!({
            "status": "ok",
            "command": "validate",
            "duration": 5.0,
            "steps": [
                { "path": "/timeline/0", "start": 0.0, "end": 1.0 },
                { "path": "/timeline/1", "start": 1.0, "end": 3.0 },
                { "path": "/timeline/2", "start": 3.0, "end": 5.0 }
            ]
        })
    );
}

/// A 4 second step whose animations each play over their own span, as a narrated step times
/// each change to the word that names it.
fn spans(animations: &str) -> String {
    common::scene(
        r##""orb": { "type": "sphere", "radius": 0.5, "opacity": 0 },
        "c": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff" } },
        "sq": { "type": "polygon", "points": [[-1, -1], [1, -1], [1, 1], [-1, 1]], "stroke": { "color": "#ffffff" } }"##,
        &format!(r#"{{ "type": "play", "duration": 4, "animations": [{animations}] }}"#),
    )
}

#[test]
fn an_animation_with_a_start_and_an_end_plays_over_that_span_of_its_step() {
    let scene = spans(
        r#"{ "type": "fade", "object": "orb", "to": 1, "start": 1, "end": 2, "easing_curve": "linear" }"#,
    );
    let opacity = |t| {
        common::inspect_at(&scene, t)["objects"]["orb"]["opacity"]
            .as_f64()
            .unwrap()
    };

    assert_eq!(
        [opacity("0.5"), opacity("1.5"), opacity("3")],
        [0.0, 0.5, 1.0]
    );
}

#[test]
fn two_moves_of_one_object_can_play_one_after_the_other_in_one_step_in_any_order() {
    let scene = spans(
        r#"{ "type": "move", "object": "orb", "to": [2, 1, 0], "start": 2, "easing_curve": "linear" },
           { "type": "move", "object": "orb", "to": [0, 1, 0], "end": 2, "easing_curve": "linear" }"#,
    );
    let position = |t| common::inspect_at(&scene, t)["objects"]["orb"]["position"].clone();

    assert_eq!(position("1"), serde_json::json!([0.0, 0.5, 0.0]), "lifting");
    assert_eq!(
        position("3"),
        serde_json::json!([1.0, 1.0, 0.0]),
        "sliding from where the lift ended"
    );
}

#[test]
fn a_create_shows_nothing_before_its_start_and_a_morph_shows_its_target_from_its_end() {
    let scene = spans(
        r#"{ "type": "create", "object": "c", "start": 1, "end": 2 },
           { "type": "morph", "object": "c", "into": "sq", "start": 2, "end": 3 }"#,
    );
    let object = |t, id: &str| common::inspect_at(&scene, t)["objects"][id].clone();

    assert_eq!(object("0.5", "c")["created"], serde_json::json!(0.0));
    assert_eq!(
        object("2.5", "c")["morph"]["phase"],
        serde_json::json!("during")
    );
    assert_eq!(
        (
            object("3.5", "c")["opacity"].clone(),
            object("3.5", "sq")["opacity"].clone()
        ),
        (serde_json::json!(0.0), serde_json::json!(1.0))
    );
}

#[test]
fn overlapping_spans_of_one_value_and_spans_outside_their_step_are_errors_at_their_fields() {
    let scene = spans(
        r#"{ "type": "move", "object": "orb", "to": [1, 0, 0], "end": 2.5 },
           { "type": "move", "object": "orb", "to": [2, 0, 0], "start": 2 },
           { "type": "fade", "object": "orb", "to": 1, "end": 5 },
           { "type": "scale", "object": "orb", "to": [2, 2, 2], "start": 3, "end": 3 },
           { "type": "rotate", "object": "orb", "to": [0, 0, 90], "start": -1 }"#,
    );
    let (_dir, path) = common::scene_file(&scene);

    let run = common::sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        common::scene_errors(&run),
        common::pairs(&[("/timeline/0/animations/4/start", "out_of_range")])
    );
    let fixed = scene.replace(r#", "start": -1"#, "");
    let (_dir, path) = common::scene_file(&fixed);
    let run = common::sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(
        common::scene_errors(&run),
        common::pairs(&[
            ("/timeline/0/animations/2/end", "out_of_range"),
            ("/timeline/0/animations/3/start", "invalid"),
            ("/timeline/0/animations/1", "conflicting_animations"),
        ])
    );
}
