//! Flat shapes and the create animation, read back from frames and reports as an agent reads them.
//!
//! The scenes are 400 by 400 pixels with the default camera, at z = 10 with a 45 degree field of
//! view, so one world unit in the plane z = 0 is 200 / (10 tan 22.5) = 48.28 pixels, and a world
//! point (x, y) lands at pixel (200 + 48.28 x, 200 - 48.28 y).

mod common;

use common::{frame_at, inspect_at, near, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

const BLACK: [u8; 4] = [0, 0, 0, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n.as_f64().unwrap() * 1000.0).round() / 1000.0)
        .collect()
}

const LINE: &str = r##""l": { "type": "line", "from": [-2, 0], "to": [2, 0], "stroke": { "color": "#ff0000", "width": 0.2 } }"##;
const CREATE_L: &str = r##"{ "type": "play", "duration": 2, "animations": [{ "type": "create", "object": "l", "easing_curve": "linear" }] }"##;

#[test]
fn a_line_at_half_create_covers_half_its_path() {
    let scene = scene(LINE, CREATE_L);

    let (_a, start) = frame_at(&scene, "0");
    let (_b, half) = frame_at(&scene, "1");
    let (_c, end) = frame_at(&scene, "2");
    let inspected = inspect_at(&scene, "1");

    assert_eq!(
        pixel(&start, 152, 200),
        BLACK,
        "nothing is drawn before the create begins"
    );
    assert_eq!(
        pixel(&half, 152, 200),
        RED,
        "one unit left of center is drawn at half"
    );
    assert_eq!(pixel(&half, 195, 200), RED, "just left of the cut at x = 0");
    assert_eq!(pixel(&half, 205, 200), BLACK, "just right of the cut");
    assert_eq!(
        pixel(&half, 248, 200),
        BLACK,
        "one unit right of center is not yet drawn"
    );
    assert_eq!(pixel(&end, 248, 200), RED);
    let l = &inspected["objects"]["l"];
    assert_eq!(l["created"], 0.5);
    assert_eq!(
        numbers(&l["bounding_box"]["world"]["min"]),
        [-2.0, -0.1, 0.0]
    );
    assert_eq!(
        numbers(&l["bounding_box"]["world"]["max"]),
        [2.0, 0.1, 0.0],
        "the whole line, as for an object at opacity 0"
    );
}

#[test]
fn an_object_the_timeline_creates_shows_nothing_before_its_step() {
    let scene = scene(
        LINE,
        &format!(r##"{{ "type": "wait", "duration": 1 }}, {CREATE_L}"##),
    );

    let (_a, waiting) = frame_at(&scene, "0.5");
    let inspected = inspect_at(&scene, "0.5");

    assert_eq!(pixel(&waiting, 200, 200), BLACK);
    let l = &inspected["objects"]["l"];
    assert_eq!(l["created"], 0.0);
    assert_eq!(
        numbers(&l["bounding_box"]["world"]["min"]),
        [-2.0, -0.1, 0.0],
        "the whole line, which the create will draw"
    );
    assert_eq!(l["in_frame"], "whole");
}

const SQUARE: &str = r##""s": { "type": "polygon", "points": [[-1, -1], [1, -1], [1, 1], [-1, 1]],
    "stroke": { "color": "#ff0000", "width": 0.2 }, "fill": { "color": "#00ff00" } }"##;

#[test]
fn a_create_draws_the_stroke_over_the_first_half_then_fades_the_fill_in() {
    let scene = scene(
        SQUARE,
        r##"{ "type": "play", "duration": 2, "animations": [{ "type": "create", "object": "s", "easing_curve": "linear" }] }"##,
    );

    let (_a, quarter) = frame_at(&scene, "0.5");
    let (_b, three_quarters) = frame_at(&scene, "1.5");
    let (_c, end) = frame_at(&scene, "2");

    assert_eq!(
        pixel(&quarter, 200, 248),
        RED,
        "the bottom side is drawn first"
    );
    assert_eq!(
        pixel(&quarter, 200, 152),
        BLACK,
        "the top side is not drawn yet"
    );
    assert_eq!(
        pixel(&quarter, 200, 200),
        BLACK,
        "no fill while the stroke draws"
    );
    assert_eq!(
        pixel(&three_quarters, 200, 152),
        RED,
        "the whole stroke is drawn by half the step"
    );
    assert!(
        near(pixel(&three_quarters, 200, 200), [0, 188, 0, 255]),
        "the fill at half its opacity over black: {:?}",
        pixel(&three_quarters, 200, 200)
    );
    assert_eq!(pixel(&end, 200, 200), GREEN);
    assert_eq!(
        pixel(&end, 248, 200),
        RED,
        "the stroke is drawn over the fill"
    );
}

#[test]
fn a_polygon_fill_covers_its_inside_and_not_its_notch() {
    let c_shape = r##""c": { "type": "polygon", "points": [[-2, -2], [2, -2], [2, 2], [-2, 2], [-2, 1], [1, 1], [1, -1], [-2, -1]], "fill": { "color": "#00ff00" } }"##;

    let (_a, frame) = frame_at(&scene(c_shape, ""), "0");

    assert_eq!(
        pixel(&frame, 272, 200),
        GREEN,
        "1.5 units right, inside the back of the C"
    );
    assert_eq!(
        pixel(&frame, 200, 128),
        GREEN,
        "1.5 units up, inside the top arm"
    );
    assert_eq!(pixel(&frame, 200, 200), BLACK, "the center is in the notch");
    assert_eq!(
        pixel(&frame, 152, 200),
        BLACK,
        "one unit left is in the notch"
    );
}

#[test]
fn a_path_fill_has_a_hole_under_even_odd_and_none_under_nonzero() {
    let ring = |rule: &str| {
        scene(
            &format!(
                r##""r": {{ "type": "path", "d": "M -2 0 A 2 2 0 1 0 2 0 A 2 2 0 1 0 -2 0 Z M -1 0 A 1 1 0 1 0 1 0 A 1 1 0 1 0 -1 0 Z", "fill": {{ "color": "#00ff00", "rule": "{rule}" }} }}"##
            ),
            "",
        )
    };

    let (_a, even_odd) = frame_at(&ring("even-odd"), "0");
    let (_b, non_zero) = frame_at(&ring("non-zero"), "0");

    assert_eq!(
        pixel(&even_odd, 272, 200),
        GREEN,
        "1.5 units out is in the ring"
    );
    assert_eq!(pixel(&even_odd, 200, 200), BLACK, "the center is the hole");
    assert_eq!(pixel(&non_zero, 272, 200), GREEN);
    assert_eq!(
        pixel(&non_zero, 200, 200),
        GREEN,
        "both parts wind the same way, so nonzero fills the center"
    );
}

#[test]
fn strokes_meet_at_mitered_corners_and_end_with_square_caps() {
    let square = r##""s": { "type": "polygon", "points": [[-1, -1], [1, -1], [1, 1], [-1, 1]], "stroke": { "color": "#ff0000", "width": 0.4 } }"##;
    let line = r##""l": { "type": "line", "from": [-2, 0], "to": [2, 0], "stroke": { "color": "#ff0000", "width": 0.4 } }"##;

    let (_a, cornered) = frame_at(&scene(square, ""), "0");
    let (_b, capped) = frame_at(&scene(line, ""), "0");

    assert_eq!(
        pixel(&cornered, 256, 143),
        RED,
        "world (1.17, 1.17): inside the mitered corner, 0.24 from the corner point, which a round join would leave black"
    );
    assert_eq!(
        pixel(&cornered, 262, 137),
        BLACK,
        "world (1.29, 1.29): past the miter, which ends at 1.2"
    );
    assert_eq!(pixel(&capped, 291, 200), RED, "world (1.9, 0): on the line");
    assert_eq!(
        pixel(&capped, 301, 200),
        BLACK,
        "world (2.1, 0): a square cap ends at 2, a round one would reach 2.2"
    );
}

#[test]
fn an_arrow_has_a_filled_tip_in_its_stroke_color() {
    let arrow = r##""a": { "type": "arrow", "from": [-2, 0], "to": [2, 0], "stroke": { "color": "#ff0000", "width": 0.1 } }"##;

    let (_a, frame) = frame_at(&scene(arrow, ""), "0");

    assert_eq!(pixel(&frame, 200, 200), RED, "the shaft");
    assert_eq!(
        pixel(&frame, 248, 195),
        BLACK,
        "world (1, 0.1): beside the shaft, which is 0.05 wide each side"
    );
    assert_eq!(
        pixel(&frame, 284, 196),
        RED,
        "world (1.75, 0.07): inside the tip, whose half width there is 0.125"
    );
    assert_eq!(pixel(&frame, 296, 200), RED, "world (2, 0): the apex");
    assert_eq!(
        pixel(&frame, 306, 200),
        BLACK,
        "world (2.2, 0): past the apex and its miter"
    );
}

#[test]
fn an_arc_sweeps_counterclockwise_from_its_start_angle() {
    let arc = r##""a": { "type": "arc", "radius": 2, "start_angle": 0, "sweep": 90, "stroke": { "color": "#ff0000", "width": 0.2 } }"##;

    let (_a, frame) = frame_at(&scene(arc, ""), "0");

    assert_eq!(pixel(&frame, 268, 131), RED, "45 degrees round, on the arc");
    assert_eq!(
        pixel(&frame, 268, 268),
        BLACK,
        "45 degrees the other way, where the arc is not"
    );
    assert_eq!(
        pixel(&frame, 131, 131),
        BLACK,
        "135 degrees, past the sweep"
    );
}

#[test]
fn a_tilted_polygon_draws_its_stroke_over_its_fill_whatever_the_tilt() {
    for rotation in ["[0, 0, 0]", "[63, 27, 11]", "[80, 0, 0]", "[85, 10, 0]"] {
        let polygon = |fill: &str| {
            scene(
                &format!(
                    r##""s": {{ "type": "polygon", "points": [[-3, -2], [3, -2], [2, 2], [-2, 2]], "rotation": {rotation}, "stroke": {{ "color": "#ff0000", "width": 0.6 }}{fill} }}"##
                ),
                "",
            )
        };

        let (_a, alone) = frame_at(&polygon(""), "0");
        let (_b, over_fill) = frame_at(&polygon(r##", "fill": { "color": "#00ff00" }"##), "0");

        // The tilted stroke is lit at one shade, so its brightest red is a pixel it covers whole. A
        // pixel it covers in part is a blend with whatever is beside it, the fill or the background.
        let full_red = (0..400 * 400)
            .map(|i| pixel(&alone, i % 400, i / 400)[0])
            .max()
            .unwrap();
        let mut covered = 0;
        for y in 0..400 {
            for x in 0..400 {
                if pixel(&alone, x, y) == [full_red, 0, 0, 255] {
                    covered += 1;
                    assert_eq!(
                        pixel(&over_fill, x, y),
                        [full_red, 0, 0, 255],
                        "the fill shows through the stroke at ({x}, {y}), rotation {rotation}"
                    );
                }
            }
        }
        assert!(
            covered > 100,
            "the stroke covers {covered} pixels whole at rotation {rotation}"
        );
    }
}

#[test]
fn a_faded_stroke_shows_no_fill_through_it() {
    let circle = r##""c": { "type": "circle", "radius": 3, "opacity": 0.5, "stroke": { "color": "#ff0000", "width": 0.6 }, "fill": { "color": "#00ff00" } }"##;

    let (_a, frame) = frame_at(&scene(circle, ""), "0");

    assert!(
        near(pixel(&frame, 337, 200), [188, 0, 0, 255]),
        "world x 2.85, the inner half of the stroke, over the fill: {:?}",
        pixel(&frame, 337, 200)
    );
    assert!(
        near(pixel(&frame, 352, 200), [188, 0, 0, 255]),
        "world x 3.15, the outer half: {:?}",
        pixel(&frame, 352, 200)
    );
    assert!(
        near(pixel(&frame, 200, 200), [0, 188, 0, 255]),
        "the fill at half over black: {:?}",
        pixel(&frame, 200, 200)
    );
}

/// How many pixels' worth of red a frame over black shows, in linear light, so a pixel the
/// stroke covers by half counts as half.
fn covered_pixels(rgba: &[u8]) -> f64 {
    let linear = |c: u8| {
        let c = f64::from(c) / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    rgba.chunks_exact(4).map(|p| linear(p[0])).sum()
}

#[test]
fn a_stroke_keeps_its_width_all_the_way_round_and_half_a_create_covers_half() {
    let ring = r##""c": { "type": "circle", "radius": 3, "stroke": { "color": "#ff0000", "width": 0.4 } }"##;
    let creating = scene(
        ring,
        r##"{ "type": "play", "duration": 1, "animations": [{ "type": "create", "object": "c", "easing_curve": "linear" }] }"##,
    );

    let (_a, whole) = frame_at(&scene(ring, ""), "0");
    let (_b, half) = frame_at(&creating, "0.5");

    let expected = 2.0 * std::f64::consts::PI * 3.0 * 0.4 * 48.28 * 48.28;
    let whole = covered_pixels(&whole);
    assert!(
        (whole - expected).abs() < expected * 0.01,
        "covered {whole}, a ring 0.4 wide covers {expected}"
    );
    let half = covered_pixels(&half);
    assert!(
        (half / whole - 0.5).abs() < 0.01,
        "half {half} of whole {whole}"
    );
}

#[test]
fn a_stroke_is_anti_aliased_along_its_sides() {
    let diagonal = r##""d": { "type": "line", "from": [-2, -2], "to": [2, 2], "stroke": { "color": "#ff0000", "width": 0.5 } }"##;

    let (_a, frame) = frame_at(&scene(diagonal, ""), "0");

    let blended = (0..400 * 400)
        .filter(|i| matches!(pixel(&frame, i % 400, i / 400), [r, 0, 0, 255] if r > 0 && r < 255))
        .count();
    assert!(
        blended >= 50,
        "{blended} pixels along the edges are a blend of red and black"
    );
    assert_eq!(pixel(&frame, 200, 200), RED);
}

#[test]
fn an_arrow_that_points_nowhere_is_a_scene_error() {
    let (_dir, path) = scene_file(&scene(
        r##""a": { "type": "arrow", "from": [1, 1], "to": [1, 1] }"##,
        "",
    ));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1);
    assert_eq!(scene_errors(&run), pairs(&[("/objects/a/to", "invalid")]));
    assert_eq!(
        run.json["errors"][0]["message"],
        "the arrow's `to` is its `from`, so it points nowhere"
    );
}

#[test]
fn a_d_that_is_not_svg_path_syntax_is_a_scene_error_at_d() {
    let (_a, cut_short) = scene_file(&scene(r##""p": { "type": "path", "d": "M 0 0 L 1" }"##, ""));
    let (_b, odd_character) = scene_file(&scene(
        r##""p": { "type": "path", "d": "M 0 0 L 1 0 # L 2 0" }"##,
        "",
    ));

    let cut_short = sci_anim(&["validate", cut_short.to_str().unwrap()]);
    let odd_character = sci_anim(&["validate", odd_character.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&cut_short),
        pairs(&[("/objects/p/d", "bad_format")])
    );
    assert_eq!(
        cut_short.json["errors"][0]["message"],
        "`d` is not SVG path syntax: Unexpected EOF"
    );
    assert_eq!(
        scene_errors(&odd_character),
        pairs(&[("/objects/p/d", "bad_format")])
    );
}

#[test]
fn a_shape_with_no_length_is_a_scene_error_at_its_field() {
    let (_dir, path) = scene_file(&scene(
        r##""a": { "type": "arc", "radius": 1, "sweep": 0, "stroke": { "color": "#ffffff" } },
    "e": { "type": "path", "d": "", "fill": { "color": "#ffffff" } },
    "l": { "type": "line", "from": [1, 1], "to": [1, 1] },
    "m": { "type": "path", "d": "M 1 1", "stroke": { "color": "#ffffff" } },
    "q": { "type": "polygon", "points": [[2, 2], [2, 2], [2, 2]], "fill": { "color": "#ffffff" } }"##,
        "",
    ));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/a/sweep", "invalid"),
            ("/objects/e/d", "invalid"),
            ("/objects/l/to", "invalid"),
            ("/objects/m/d", "invalid"),
            ("/objects/q/points", "invalid"),
        ]),
        "{}",
        run.stdout
    );
    assert_eq!(
        run.json["errors"][2]["message"],
        "the line's `to` is its `from`, so it has no length"
    );
    assert_eq!(
        run.json["errors"][3]["message"],
        "`d` has no length, so there is nothing to draw"
    );
}

fn create_errors(objects: &str, timeline: &str) -> common::Run {
    let (_dir, path) = scene_file(&scene(objects, timeline));
    sci_anim(&["validate", path.to_str().unwrap()])
}

const CREATE_ORB: &str =
    r##"{ "type": "play", "duration": 1, "animations": [{ "type": "create", "object": "orb" }] }"##;

#[test]
fn a_create_of_a_shape_with_no_stroke_is_a_scene_error() {
    let run = create_errors(
        r##""orb": { "type": "circle", "radius": 1, "fill": { "color": "#ffffff" } }"##,
        CREATE_ORB,
    );

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/0/animations/0/object", "no_stroke")])
    );
    assert_eq!(
        run.json["errors"][0]["message"],
        "`create` draws a stroke along a flat shape's path or grows a solid, and `orb` has no stroke; give it one, or fade it in instead"
    );
}

#[test]
fn a_second_create_of_one_object_is_a_scene_error() {
    let run = create_errors(
        &format!(
            "{LINE}, {}",
            r##""orb": { "type": "sphere", "radius": 1 }"##
        ),
        &format!("{CREATE_L}, {CREATE_L}"),
    );

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/1/animations/0/object", "created_twice")]),
        "{}",
        run.stdout
    );
    assert_eq!(
        run.json["errors"][0]["message"],
        "`l` is already created by /timeline/0/animations/0, and an object is created once"
    );
}

#[test]
fn the_schema_checks_each_new_shape_at_its_field() {
    let (_dir, path) = scene_file(&scene(
        r##""p": { "type": "polygon", "points": [[0, 0], [1, 1]] },
    "a": { "type": "arc", "radius": 1, "sweep": 400 },
    "l": { "type": "line", "from": [0], "to": [1, 1] },
    "f": { "type": "circle", "radius": 1, "fill": { "color": "#ffffff", "rule": "winding" } }"##,
        "",
    ));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/a/sweep", "out_of_range"),
            ("/objects/f/fill/rule", "unknown_value"),
            ("/objects/l/from", "wrong_length"),
            ("/objects/p/points", "wrong_length"),
        ]),
        "{}",
        run.stdout
    );
}

#[test]
fn a_stroke_sets_its_color_and_a_line_with_no_stroke_is_white() {
    let (_dir, path) = scene_file(&scene(
        r##""c": { "type": "circle", "radius": 1, "stroke": { "width": 0.1 } },
    "l": { "type": "line", "from": [0, 0], "to": [1, 0], "stroke": { "width": 0.1 } }"##,
        "",
    ));
    let white_line = scene(
        r##""l": { "type": "line", "from": [-2, 0], "to": [2, 0] }"##,
        "",
    );

    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    let (_a, frame) = frame_at(&white_line, "0");

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/c/stroke/color", "missing_field"),
            ("/objects/l/stroke/color", "missing_field")
        ])
    );
    assert_eq!(pixel(&frame, 200, 200), [255, 255, 255, 255]);
}

/// The pixels of row `y` at `xs` that are more red than not.
fn red_run(rgba: &[u8], y: usize, xs: std::ops::Range<usize>) -> usize {
    xs.filter(|&x| pixel(rgba, x, y)[0] > 127).count()
}

#[test]
fn a_scale_reshapes_the_path_and_keeps_the_stroke_width() {
    let ring = |radius: &str, scale: &str| {
        scene(
            &format!(
                r##""c": {{ "type": "circle", "radius": {radius}, "scale": {scale}, "stroke": {{ "color": "#ff0000", "width": 0.2 }} }}"##
            ),
            "",
        )
    };

    let (_a, unscaled) = frame_at(&ring("2", "[1, 1, 1]"), "0");
    let (_b, scaled) = frame_at(&ring("1", "[2, 2, 1]"), "0");
    let (_c, stretched) = frame_at(&ring("1", "[2, 1, 1]"), "0");
    let inspected = inspect_at(&ring("1", "[2, 2, 1]"), "0");

    assert_eq!(
        red_run(&unscaled, 200, 250..350),
        11,
        "0.2 units is 9.7 pixels, and the edge pixels it half covers are more red than not"
    );
    assert_eq!(
        red_run(&scaled, 200, 250..350),
        11,
        "a circle scaled by 2 keeps a stroke 0.2 wide"
    );
    assert_eq!(red_run(&stretched, 200, 250..350), 11, "the stretched side");
    assert_eq!(
        (0..200)
            .filter(|&y| pixel(&stretched, 200, y)[0] > 127)
            .count(),
        10,
        "the unstretched side is as wide, to the pixel its edges fall on"
    );
    let c = &inspected["objects"]["c"];
    assert_eq!(
        numbers(&c["scale"]),
        [2.0, 2.0, 1.0],
        "inspect reports the object's scale"
    );
    assert_eq!(
        numbers(&c["bounding_box"]["world"]["max"]),
        [2.1, 2.1, 0.0],
        "the box is what is drawn: radius 2 plus half the stroke"
    );
}

#[test]
fn a_faded_stroke_that_crosses_itself_blends_once() {
    let crossing = r##""x": { "type": "path", "d": "M -2 -2 L 2 2 L 2 -2 L -2 2", "opacity": 0.5, "stroke": { "color": "#ff0000", "width": 0.4 } },
    "a": { "type": "arrow", "from": [-2, -3], "to": [2, -3], "opacity": 0.5, "stroke": { "color": "#ff0000", "width": 0.2 } }"##;

    let (_a, frame) = frame_at(&scene(crossing, ""), "0");

    assert!(
        near(pixel(&frame, 152, 248), [188, 0, 0, 255]),
        "world (-1, -1), one layer at half over black: {:?}",
        pixel(&frame, 152, 248)
    );
    assert!(
        near(pixel(&frame, 200, 200), [188, 0, 0, 255]),
        "the crossing at (0, 0) blends once: {:?}",
        pixel(&frame, 200, 200)
    );
    let twice: Vec<(usize, usize)> = (0..400 * 400)
        .map(|i| (i % 400, i / 400))
        .filter(|&(x, y)| pixel(&frame, x, y)[0] > 189)
        .collect();
    assert_eq!(
        twice,
        [],
        "pixels blended more than once, at the crossing, the corners or the arrow's tip"
    );
}

#[test]
fn the_create_example_validates_and_is_mid_create_at_three_seconds() {
    let example = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/curves.json");
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("example.png");

    let validated = sci_anim(&["validate", example]);
    let framed = sci_anim(&[
        "frame",
        example,
        "--scene-time",
        "3.2",
        "-o",
        png.to_str().unwrap(),
    ]);
    let inspected = sci_anim(&["inspect", example, "--scene-time", "3.2"]);

    assert_eq!(
        (
            validated.json["status"].as_str(),
            validated.json["command"].as_str()
        ),
        (Some("ok"), Some("validate"))
    );
    assert_eq!(framed.exit_code, 0, "{}", framed.stdout);
    let objects = inspected.json["objects"].as_object().unwrap();
    assert_eq!(objects.len(), 6);
    assert!(
        objects.values().all(|o| o["in_frame"] == "whole"),
        "{}",
        inspected.stdout
    );
    let box_created = objects["box"]["created"].as_f64().unwrap();
    assert!(
        box_created > 0.0 && box_created < 1.0,
        "the box is mid-create at 3.2 s: {box_created}"
    );
}

/// A sharp corner's miter would reach 0.39 past the corner for a 0.2 wide stroke, so it is
/// beveled instead, while the right angle keeps its square miter.
#[test]
fn a_sharp_corner_is_beveled_and_a_right_angle_stays_mitered() {
    let triangle = r##""t": { "type": "polygon", "points": [[0, 0], [4, 0], [0, 2.2]], "stroke": { "color": "#ff0000", "width": 0.2 } }"##;
    let inspected = inspect_at(&scene(triangle, ""), "0");

    let world = &inspected["objects"]["t"]["bounding_box"]["world"];
    assert_eq!(
        numbers(&world["min"])[..2],
        [-0.1, -0.1],
        "the right angle at the origin is mitered"
    );
    let sharp_tip = world["max"][0].as_f64().unwrap();
    assert!(
        (4.04..4.06).contains(&sharp_tip),
        "the 29 degree corner ends at the bevel, not the miter at 4.39: {sharp_tip}"
    );
}
