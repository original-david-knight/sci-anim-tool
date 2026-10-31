//! Text, math and the write animation, read back from frames and reports as an agent reads them.
//!
//! The scenes are 400 by 400 pixels with the default camera, so one world unit in the plane
//! z = 0 is 48.28 pixels, and a world point (x, y) lands at pixel (200 + 48.28 x, 200 - 48.28 y).
//! Most scenes set `"font": "serif"`, and their glyph positions come from Libertinus Serif's
//! outlines at 10 points, where an `H` spans x 0.145 to 7.095 and is 7.3 wide, its stems run
//! x 1.06 to 1.90 and 5.33 to 6.17, and its crossbar crosses y 3.3, measured down from its cap
//! height at 0 to its baseline at 6.58.

mod common;

use common::{frame_at, inspect_at, near, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

const BLACK: [u8; 4] = [0, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n.as_f64().unwrap() * 100.0).round() / 100.0)
        .collect()
}

#[test]
fn hello_at_size_1_is_centered_on_its_box_from_cap_height_to_baseline() {
    let hello = r##""t": { "type": "text", "text": "Hello", "font": "serif", "size": 1 }"##;

    let inspected = inspect_at(&scene(hello, ""), "0");

    let world = &inspected["objects"]["t"]["bounding_box"]["world"];
    assert_eq!(
        numbers(&world["min"]),
        [-1.09, -0.34, 0.0],
        "the o dips 0.01 below the baseline at -0.329"
    );
    assert_eq!(
        numbers(&world["max"]),
        [1.06, 0.37, 0.0],
        "the l rises 0.04 above the cap height at 0.329"
    );
}

#[test]
fn text_is_sans_unless_its_font_is_serif() {
    let hellos = r##""default": { "type": "text", "text": "Hello", "size": 1 }, "sans": { "type": "text", "text": "Hello", "font": "sans", "size": 1 }"##;

    let inspected = inspect_at(&scene(hellos, ""), "0");

    let default = &inspected["objects"]["default"]["bounding_box"]["world"];
    let sans = &inspected["objects"]["sans"]["bounding_box"]["world"];
    assert_eq!(
        numbers(&default["min"]),
        [-1.0, -0.34, 0.0],
        "narrower than Libertinus Serif's -1.09"
    );
    assert_eq!(numbers(&default["max"]), [1.05, 0.37, 0.0]);
    assert_eq!(default, sans);
}

#[test]
fn left_aligned_text_starts_at_its_position() {
    let hello = r##""t": { "type": "text", "text": "Hello", "font": "serif", "size": 1, "align": "left", "position": [-2, 0, 0] }"##;

    let inspected = inspect_at(&scene(hello, ""), "0");

    let world = &inspected["objects"]["t"]["bounding_box"]["world"];
    assert_eq!(
        numbers(&world["min"]),
        [-1.99, -0.34, 0.0],
        "the H's serif starts 0.0145 in from the box's left edge"
    );
}

const BIG_H: &str = r##""h": { "type": "text", "text": "H", "font": "serif", "size": 4 }"##;

#[test]
fn a_glyph_fills_its_stems_and_crossbar_and_not_the_space_between() {
    let (_a, frame) = frame_at(&scene(BIG_H, ""), "0");

    assert_eq!(
        pixel(&frame, 158, 165),
        WHITE,
        "the left stem, at 10 points x 1.48 y 1.5"
    );
    assert_eq!(
        pixel(&frame, 199, 165),
        BLACK,
        "between the stems, x 3.6 y 1.5"
    );
    assert_eq!(pixel(&frame, 199, 200), WHITE, "the crossbar, x 3.6 y 3.3");
}

const FIVE_HS: &str = r##""h": { "type": "text", "text": "HHHHH", "font": "serif", "size": 2 }"##;
const WRITE_H: &str = r##"{ "type": "play", "duration": 2, "animations": [{ "type": "write", "object": "h", "easing_curve": "linear" }] }"##;

#[test]
fn a_half_written_text_has_filled_its_first_glyphs_and_only_begun_its_last() {
    let scene = scene(FIVE_HS, WRITE_H);

    let (_a, before) = frame_at(&scene, "0");
    let (_b, half) = frame_at(&scene, "1");
    let (_c, written) = frame_at(&scene, "2");
    let inspected = inspect_at(&scene, "1");

    assert_eq!(inspected["objects"]["h"]["created"], 0.5);
    assert_eq!(
        pixel(&before, 58, 198),
        BLACK,
        "nothing shows before the write"
    );
    assert!(
        near(pixel(&half, 58, 198), [231, 231, 231, 255]),
        "the first H's crossbar, its glyph 0.9 written, so its fill at 0.8: {:?}",
        pixel(&half, 58, 198)
    );
    assert_eq!(
        pixel(&half, 340, 198),
        BLACK,
        "the last H's crossbar, its glyph 0.1 written, so only a fifth of its stroke"
    );
    assert_eq!(pixel(&written, 58, 198), WHITE);
    assert_eq!(pixel(&written, 340, 198), WHITE);
}

#[test]
fn a_finished_write_draws_exactly_what_the_text_with_no_write_draws() {
    let (_a, written) = frame_at(&scene(FIVE_HS, WRITE_H), "2");
    let (_b, declared) = frame_at(&scene(FIVE_HS, ""), "0");

    assert!(
        written == declared,
        "a write ends at a created share of exactly 1, as a declared text has"
    );
}

#[test]
fn inspect_counts_the_glyphs_a_write_paces() {
    let objects = r##""euler": { "type": "math", "tex": "e^{i\\pi} + 1 = 0" },
        "roots": { "type": "math", "tex": "x = \\frac{-b \\pm \\sqrt{b^2 - 4ac}}{2a}" },
        "hello": { "type": "text", "text": "Hello, world!" },
        "c": { "type": "circle", "radius": 1 }"##;

    let inspected = inspect_at(&scene(objects, ""), "0");

    assert_eq!(
        inspected["objects"]["euler"]["glyphs"], 7,
        "e, i, π, +, 1, =, 0"
    );
    assert_eq!(
        inspected["objects"]["roots"]["glyphs"], 16,
        "x = - b ± √ b 2 - 4 a c 2 a, the root's bar and the fraction bar"
    );
    assert_eq!(
        inspected["objects"]["hello"]["glyphs"], 12,
        "the space draws nothing"
    );
    assert_eq!(
        inspected["objects"]["c"].get("glyphs"),
        None,
        "a circle has no glyphs"
    );
}

#[test]
fn a_second_write_of_one_object_is_a_scene_error_that_says_written() {
    let objects = r##""eq": { "type": "math", "tex": "x^2" }"##;
    let write = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "write", "object": "eq" }] }"##;
    let (_dir, path) = scene_file(&scene(objects, &format!("{write}, {write}")));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/1/animations/0/object", "created_twice")]),
        "{}",
        run.stdout
    );
    assert_eq!(
        run.json["errors"][0]["message"],
        "`eq` is already written by /timeline/0/animations/0, and an object is written once"
    );
}

#[test]
fn bad_tex_is_a_scene_error_at_its_tex_with_the_message_of_mitex_or_typst() {
    let (_a, unknown) = scene_file(&scene(
        r##""eq": { "type": "math", "tex": "\\foo{x}" }"##,
        "",
    ));
    let (_b, unclosed) = scene_file(&scene(
        r##""eq": { "type": "math", "tex": "\\left( x" }"##,
        "",
    ));
    let (_c, empty) = scene_file(&scene(r##""t": { "type": "text", "text": " " }"##, ""));

    let unknown = sci_anim(&["validate", unknown.to_str().unwrap()]);
    let unclosed = sci_anim(&["validate", unclosed.to_str().unwrap()]);
    let empty = sci_anim(&["validate", empty.to_str().unwrap()]);

    assert_eq!(unknown.exit_code, 1);
    assert_eq!(
        scene_errors(&unknown),
        pairs(&[("/objects/eq/tex", "bad_format")])
    );
    assert_eq!(
        unknown.json["errors"][0]["message"],
        "`tex` is not LaTeX math that sci_anim can typeset: unknown command: \\foo"
    );
    assert_eq!(
        unclosed.json["errors"][0]["message"],
        "`tex` is not LaTeX math that sci_anim can typeset: unclosed delimiter, in the Typst math `lr(\\(  x ` that mitex wrote for it"
    );
    assert_eq!(
        scene_errors(&empty),
        pairs(&[("/objects/t/text", "invalid")])
    );
}

#[test]
fn write_is_for_text_and_math_and_create_for_other_flat_shapes() {
    let objects = r##""eq": { "type": "math", "tex": "x^2" }, "c": { "type": "circle", "radius": 1, "stroke": { "color": "#ffffff" } }"##;
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "create", "object": "eq" }, { "type": "write", "object": "c" }] }"##;
    let (_dir, path) = scene_file(&scene(objects, timeline));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/0/animations/0/object", "use_write"),
            ("/timeline/0/animations/1/object", "not_text")
        ]),
        "{}",
        run.stdout
    );
}

/// Every pixel of one half of a 400 by 400 frame: `true` for the left half.
fn half(frame: &[u8], left: bool) -> Vec<[u8; 4]> {
    (0..400)
        .flat_map(|y| (0..200).map(move |x| (x, y)))
        .map(|(x, y)| pixel(frame, if left { x } else { x + 200 }, y))
        .collect()
}

const RED: [u8; 4] = [255, 0, 0, 255];
const YELLOW: [u8; 4] = [255, 255, 0, 255];
const TWO_HS: &str = r"\\textcolor{red}{\\mathrm{H}} \\quad \\mathrm{H}";

#[test]
fn a_glyph_latex_colors_is_filled_in_its_color_and_the_rest_in_the_objects() {
    let eq = format!(
        r##""eq": {{ "type": "math", "tex": "{TWO_HS}", "size": 2, "color": "#ffff00" }}"##
    );

    let (_a, frame) = frame_at(&scene(&eq, ""), "0");

    let (left, right) = (half(&frame, true), half(&frame, false));
    assert!(
        left.contains(&RED) && !left.contains(&YELLOW),
        "the left H is red"
    );
    assert!(
        right.contains(&YELLOW) && !right.contains(&RED),
        "the right H is the object's yellow"
    );
}

#[test]
fn a_hex_color_a_named_color_and_a_color_to_the_end_of_a_group_each_color_their_glyphs() {
    let eq = r##""eq": { "type": "math", "tex": "{\\color{#ff0000} \\mathrm{H}} \\quad \\textcolor{Yellow}{\\mathrm{H}}", "size": 2, "color": "#0000ff" }"##;

    let (_a, frame) = frame_at(&scene(eq, ""), "0");

    assert!(half(&frame, true).contains(&RED));
    assert!(half(&frame, false).contains(&YELLOW));
    assert!(
        !frame.chunks(4).any(|p| p == [0, 0, 255, 255]),
        "no glyph is left in the object's blue"
    );
}

#[test]
fn a_write_outlines_each_glyph_with_a_pen_in_the_glyphs_own_color() {
    let eq = format!(r##""eq": {{ "type": "math", "tex": "{TWO_HS}", "size": 2 }}"##);
    let write = r##"{ "type": "play", "duration": 2, "animations": [{ "type": "write", "object": "eq", "easing_curve": "linear" }] }"##;

    let (_a, frame) = frame_at(&scene(&eq, write), "0.2");

    let drawn: Vec<&[u8]> = frame.chunks(4).filter(|p| *p != BLACK).collect();
    assert!(drawn.iter().any(|p| p[0] > 200), "the first H's pen shows");
    assert!(
        drawn.iter().all(|p| p[1] == 0 && p[2] == 0),
        "only the red H has begun, and its pen is red, not the object's white"
    );
}

#[test]
fn a_morph_mixes_each_glyphs_own_color_with_its_partners() {
    let objects = format!(
        r##""a": {{ "type": "math", "tex": "{TWO_HS}", "size": 2 }}, "b": {{ "type": "math", "tex": "{}", "size": 2 }}"##,
        TWO_HS.replace("red", "blue")
    );
    let morph = r##"{ "type": "play", "duration": 2, "animations": [{ "type": "morph", "object": "a", "into": "b", "easing_curve": "linear" }] }"##;

    let (_a, frame) = frame_at(&scene(&objects, morph), "1");

    let (left, right) = (half(&frame, true), half(&frame, false));
    assert!(
        left.contains(&[188, 0, 188, 255]) && !left.contains(&WHITE),
        "red and blue meet at #bc00bc in linear light"
    );
    assert!(right.contains(&WHITE), "white stays white");
}

#[test]
fn a_color_sci_anim_cannot_read_is_a_scene_error_at_tex_that_names_the_forms_that_work() {
    let (_a, unknown) = scene_file(&scene(
        r##""eq": { "type": "math", "tex": "\\textcolor{nocolor}{x}" }"##,
        "",
    ));
    let (_b, bracketed) = scene_file(&scene(
        r##""eq": { "type": "math", "tex": "\\color[HTML]{FF0000} x" }"##,
        "",
    ));

    let unknown = sci_anim(&["validate", unknown.to_str().unwrap()]);
    let bracketed = sci_anim(&["validate", bracketed.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&unknown),
        pairs(&[("/objects/eq/tex", "bad_format")])
    );
    assert_eq!(
        unknown.json["errors"][0]["message"],
        "`tex` gives the color `nocolor`, which sci_anim cannot read; write a color in braces as `#rrggbb`, as in `\\textcolor{#58c4dd}{x}`, or as one of red, green, blue, cyan, magenta, yellow, black, white, gray, lightgray, darkgray, brown, orange, pink, purple, teal and olive, with no `[HTML]` or `[rgb]` before the braces"
    );
    assert_eq!(
        scene_errors(&bracketed),
        pairs(&[("/objects/eq/tex", "bad_format")])
    );
}
