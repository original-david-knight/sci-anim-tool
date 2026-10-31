//! Recolors, read back as an agent reads them: frames, inspect and validate.
//!
//! The scenes are 400 by 400 pixels with the default camera, so one world unit in the plane
//! z = 0 is 48.28 pixels and the origin is pixel (200, 200).

mod common;

use common::{frame_at, inspect_at, near, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::json;

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

#[test]
fn a_fill_recolored_halfway_is_the_mix_in_linear_light() {
    let objects = r##""c": { "type": "circle", "radius": 2, "fill": { "color": "#ff0000" } }"##;
    let timeline = r##"{ "type": "play", "duration": 2, "animations": [{ "type": "recolor", "object": "c", "fill": "#0000ff", "easing_curve": "linear" }] }"##;
    let scene = scene(objects, timeline);

    let (_a, start) = frame_at(&scene, "0");
    let (_b, halfway) = frame_at(&scene, "1");
    let (_c, end) = frame_at(&scene, "2");

    assert_eq!(pixel(&start, 200, 200), RED);
    assert!(
        near(pixel(&halfway, 200, 200), [0xbc, 0, 0xbc, 255]),
        "{:?}",
        pixel(&halfway, 200, 200)
    );
    assert_eq!(pixel(&end, 200, 200), BLUE);
}

#[test]
fn a_recolor_of_an_arrows_stroke_recolors_its_tip_too() {
    let objects = r##""a": { "type": "arrow", "from": [-2, 0], "to": [2, 0], "stroke": { "color": "#ff0000", "width": 0.1 } }"##;
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "recolor", "object": "a", "stroke": "#0000ff" }] }"##;

    let (_a, end) = frame_at(&scene(objects, timeline), "1");

    assert_eq!(pixel(&end, 200, 200), BLUE, "the shaft");
    assert_eq!(pixel(&end, 284, 196), BLUE, "inside the tip");
}

#[test]
fn a_fill_in_another_color_keeps_it_when_only_the_stroke_is_recolored() {
    let objects = r##""c": { "type": "circle", "radius": 2, "stroke": { "color": "#00ff00", "width": 0.2 }, "fill": { "color": "#ff0000" } }"##;
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "recolor", "object": "c", "stroke": "#0000ff" }] }"##;

    let report = inspect_at(&scene(objects, timeline), "1");

    assert_eq!(
        report["objects"]["c"]["colors"],
        json!({ "stroke": "#0000ff", "fill": "#ff0000" })
    );
}

#[test]
fn a_text_recolor_colors_every_glyph_and_inspect_reports_it() {
    let objects =
        r##""t": { "type": "math", "tex": "\\textcolor{#00ff00}{6} + 1", "color": "#ffffff" }"##;
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "recolor", "object": "t", "color": "#58c4dd" }] }"##;
    let scene = scene(objects, timeline);

    assert_eq!(
        inspect_at(&scene, "0")["objects"]["t"]["colors"],
        json!({ "glyph_colors": ["#00ff00", "#ffffff"] }),
        "two glyph colors have no one color"
    );
    assert_eq!(
        inspect_at(&scene, "1")["objects"]["t"]["colors"],
        json!({ "color": "#58c4dd" })
    );
}

#[test]
fn inspect_reports_each_glyph_color_of_mixed_color_math_until_a_recolor_ends() {
    let objects =
        r##""t": { "type": "math", "tex": "\\textcolor{#ff0000}{x}+y", "color": "#ffffff" }"##;
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "recolor", "object": "t", "color": "#0000ff", "easing_curve": "linear" }] }"##;
    let scene = scene(objects, timeline);

    let midway = &inspect_at(&scene, "0.5")["objects"]["t"]["colors"];

    assert_eq!(
        inspect_at(&scene, "0")["objects"]["t"]["colors"],
        json!({ "glyph_colors": ["#ff0000", "#ffffff"] })
    );
    assert_eq!(midway, &json!({ "glyph_colors": ["#bc00bc", "#bcbcff"] }));
    assert_eq!(
        inspect_at(&scene, "1")["objects"]["t"]["colors"],
        json!({ "color": "#0000ff" })
    );
}

#[test]
fn a_sphere_recolors_its_material() {
    let objects = r##""s": { "type": "sphere", "radius": 1, "material": { "type": "standard", "color": "#ff0000" } }"##;
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "recolor", "object": "s", "color": "#0000ff" }] }"##;

    let report = inspect_at(&scene(objects, timeline), "1");

    assert_eq!(
        report["objects"]["s"]["colors"],
        json!({ "color": "#0000ff" })
    );
}

#[test]
fn a_recolor_holds_its_color_into_later_steps_and_a_second_starts_from_it() {
    let objects = r##""c": { "type": "circle", "radius": 2, "fill": { "color": "#ff0000" } }"##;
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "recolor", "object": "c", "fill": "#0000ff" }] },
      { "type": "wait", "duration": 1 },
      { "type": "play", "duration": 2, "animations": [{ "type": "recolor", "object": "c", "fill": "#ff0000", "easing_curve": "linear" }] }"##;
    let scene = scene(objects, timeline);

    let (_a, waiting) = frame_at(&scene, "1.5");
    let (_b, halfway_back) = frame_at(&scene, "3");

    assert_eq!(pixel(&waiting, 200, 200), BLUE);
    assert!(
        near(pixel(&halfway_back, 200, 200), [0xbc, 0, 0xbc, 255]),
        "{:?}",
        pixel(&halfway_back, 200, 200)
    );
}

#[test]
fn a_morph_starts_from_the_colors_a_recolor_left() {
    let objects = r##""c": { "type": "circle", "radius": 1.5, "fill": { "color": "#ff0000" } },
      "sq": { "type": "polygon", "points": [[-1.5, -1.5], [1.5, -1.5], [1.5, 1.5], [-1.5, 1.5]], "fill": { "color": "#0000ff" } }"##;
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "recolor", "object": "c", "fill": "#0000ff" }] },
      { "type": "play", "duration": 1, "animations": [{ "type": "morph", "object": "c", "into": "sq" }] }"##;

    let (_a, early) = frame_at(&scene(objects, timeline), "1.1");

    assert_eq!(
        pixel(&early, 200, 200),
        BLUE,
        "blue to blue, with no flash of the declared red"
    );
}

#[test]
fn a_recolor_of_a_color_the_object_lacks_is_an_error_at_that_field() {
    let objects = r##""c": { "type": "circle", "radius": 1, "fill": { "color": "#ff0000" } },
      "t": { "type": "text", "text": "6" },
      "ax": { "type": "axes", "x": { "range": [0, 1] }, "y": { "range": [0, 1] }, "width": 2, "height": 2 }"##;
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [
        { "type": "recolor", "object": "c", "color": "#0000ff" },
        { "type": "recolor", "object": "t", "fill": "#0000ff" },
        { "type": "recolor", "object": "ax", "color": "#0000ff" }
      ] },
      { "type": "play", "duration": 1, "animations": [
        { "type": "recolor", "object": "c", "stroke": "#0000ff" },
        { "type": "recolor", "object": "t" }
      ] }"##;
    let (_dir, path) = scene_file(&scene(objects, timeline));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/0/animations/0/color", "not_recolorable"),
            ("/timeline/0/animations/1/fill", "not_recolorable"),
            ("/timeline/0/animations/2/object", "not_recolorable"),
            ("/timeline/1/animations/0/stroke", "not_recolorable"),
            ("/timeline/1/animations/1/stroke", "missing_field"),
        ])
    );
}

#[test]
fn two_recolors_of_one_color_in_a_step_and_a_recolor_of_a_morphs_source_conflict() {
    let objects = r##""c": { "type": "circle", "radius": 1, "fill": { "color": "#ff0000" } },
      "d": { "type": "circle", "radius": 2, "fill": { "color": "#00ff00" } }"##;
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [
        { "type": "recolor", "object": "c", "fill": "#0000ff" },
        { "type": "recolor", "object": "c", "fill": "#00ff00" }
      ] },
      { "type": "play", "duration": 1, "animations": [
        { "type": "morph", "object": "c", "into": "d" },
        { "type": "recolor", "object": "c", "fill": "#ffffff" }
      ] }"##;
    let (_dir, path) = scene_file(&scene(objects, timeline));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/0/animations/1", "conflicting_animations"),
            ("/timeline/1/animations/1", "conflicting_animations")
        ])
    );
}

#[test]
fn a_morph_starts_from_a_recolor_earlier_in_its_own_step() {
    let objects = r##""c": { "type": "circle", "radius": 1.5, "fill": { "color": "#ff0000" } },
      "sq": { "type": "polygon", "points": [[-1.5, -1.5], [1.5, -1.5], [1.5, 1.5], [-1.5, 1.5]], "fill": { "color": "#0000ff" } }"##;
    let timeline = r##"{ "type": "play", "duration": 2, "animations": [
        { "type": "morph", "object": "c", "into": "sq", "start": 1 },
        { "type": "recolor", "object": "c", "fill": "#0000ff", "end": 1 }
      ] }"##;

    let (_a, early) = frame_at(&scene(objects, timeline), "1.02");

    assert_eq!(
        pixel(&early, 200, 200),
        BLUE,
        "the morph mixes blue into blue, with no snap back to the declared red"
    );
}
