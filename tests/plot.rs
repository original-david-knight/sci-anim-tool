//! Axes, number lines and plots, read back from frames and reports as an agent reads them.
//!
//! The scenes are 400 by 400 pixels with the default camera, so a world point (x, y) lands at
//! pixel (200 + 48.28 x, 200 - 48.28 y).

mod common;

use common::{frame_at, inspect_at, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::{Value, json};

const BLACK: [u8; 4] = [0, 0, 0, 255];
const RED: [u8; 4] = [255, 0, 0, 255];

const AXES: &str = r##""ax": { "type": "axes", "x": { "range": [-2, 2], "spacing": 1 }, "y": { "range": [0, 4], "spacing": 1 }, "width": 4, "height": 4 }"##;
const X_SQUARED: &str = r##""f": { "type": "function-plot", "axes": "ax", "expression": "x^2", "stroke": { "color": "#ff0000", "width": 0.1 } }"##;

fn rounded(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n.as_f64().unwrap() * 1000.0).round() / 1000.0)
        .collect()
}

fn errors(objects: &str, timeline: &str) -> Vec<(String, String)> {
    let (_dir, path) = scene_file(&scene(objects, timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    scene_errors(&run)
}

#[test]
fn x_squared_on_known_axes_passes_through_the_pixel_of_data_1_1() {
    let (_dir, frame) = frame_at(&scene(&format!("{AXES}, {X_SQUARED}"), ""), "0");

    // Data (1, 1) is local (1, -1) in the 4 by 4 box, so world (1, -1): pixel (248.3, 248.3).
    assert_eq!(pixel(&frame, 248, 248), RED);
    assert_eq!(pixel(&frame, 152, 248), RED, "data (-1, 1) by symmetry");
    assert_eq!(pixel(&frame, 248, 152), BLACK, "data (1, 3) is on no curve");
    assert_eq!(
        pixel(&frame, 120, 296),
        [255, 255, 255, 255],
        "the x axis, at data y = 0, local y = -2"
    );
}

#[test]
fn inspect_gives_the_axes_to_world_map_and_the_plot_bounding_box() {
    let inspected = inspect_at(&scene(&format!("{AXES}, {X_SQUARED}"), ""), "0");

    let axes = &inspected["objects"]["ax"]["axes"];
    assert_eq!(axes["x"], json!({ "range": [-2.0, 2.0], "spacing": 1.0 }));
    assert_eq!(
        rounded(&axes["zero"]["world"]),
        [0.0, -2.0, 0.0],
        "data (0, 0), the zero, is the bottom middle of the box"
    );
    assert_eq!(rounded(&axes["zero"]["pixels"]), [200.0, 296.569]);
    assert_eq!(
        (rounded(&axes["x_unit"]), rounded(&axes["y_unit"])),
        (vec![1.0, 0.0, 0.0], vec![0.0, 1.0, 0.0])
    );
    let ticks = axes["ticks"].as_array().unwrap();
    assert_eq!(ticks.len(), 5 + 5);
    assert_eq!(
        (
            ticks[0]["axis"].clone(),
            ticks[0]["value"].clone(),
            rounded(&ticks[0]["world"])
        ),
        (json!("x"), json!(-2.0), vec![-2.0, -2.0, 0.0])
    );
    let plot = &inspected["objects"]["f"]["plot"];
    assert_eq!(plot["axes"], "ax");
    assert_eq!(
        (
            rounded(&plot["data_bounding_box"]["min"]),
            rounded(&plot["data_bounding_box"]["max"])
        ),
        (vec![-2.0, 0.0], vec![2.0, 4.0])
    );
}

#[test]
fn an_expression_that_does_not_parse_is_a_scene_error_at_its_field_with_the_column() {
    let (_dir, path) = scene_file(&scene(
        &format!("{AXES}, {}", X_SQUARED.replace("x^2", "x^")),
        "",
    ));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        run.json["errors"],
        json!([{
            "kind": "scene",
            "path": "/objects/f/expression",
            "code": "bad_format",
            "message": "`expression` is not an expression sci_anim can read: the expression ends after `^`, where a value is needed (at column 3 of \"x^\")"
        }])
    );
}

#[test]
fn each_way_to_misname_axes_or_a_range_has_its_own_error() {
    let number_line = r##""n": { "type": "number-line", "range": [0, 10], "length": 8 }"##;

    assert_eq!(
        errors(
            &format!("{AXES}, {}", X_SQUARED.replace("\"ax\"", "\"nope\"")),
            ""
        ),
        pairs(&[("/objects/f/axes", "unknown_object")])
    );
    assert_eq!(
        errors(&format!("{number_line}, {}", X_SQUARED), ""),
        pairs(&[("/objects/f/axes", "unknown_object")])
    );
    assert_eq!(
        errors(&AXES.replace("[-2, 2]", "[2, -2]"), ""),
        pairs(&[("/objects/ax/x/range", "invalid")])
    );
    assert_eq!(
        errors(
            &AXES.replace("\"spacing\": 1 }, \"y\"", "\"spacing\": 0.001 }, \"y\""),
            ""
        ),
        pairs(&[("/objects/ax/x/spacing", "invalid")])
    );
    assert_eq!(
        errors(
            &format!("{AXES}, {}", X_SQUARED.replace("x^2", "sqrt(x) + y")),
            ""
        ),
        pairs(&[("/objects/f/expression", "bad_format")])
    );
    assert_eq!(
        errors(
            &format!("{AXES}, {}", X_SQUARED.replace("x^2", "5 + x*0")),
            ""
        ),
        pairs(&[("/objects/f/expression", "invalid")]),
        "a line at y = 5 never enters a box that ends at 4"
    );
    assert_eq!(
        errors(
            &format!(
                "{AXES}, {}",
                X_SQUARED.replace("\"stroke\"", "\"position\": [1, 0, 0], \"stroke\"")
            ),
            ""
        ),
        pairs(&[("/objects/f/position", "placed_by_axes")])
    );
}

#[test]
fn a_plot_is_placed_by_its_axes_and_cannot_be_moved_alone() {
    let move_axes = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "move", "object": "ax", "to": [1, 0, 0], "easing_curve": "linear" }] }"##;
    let moved = scene(&format!("{AXES}, {X_SQUARED}"), move_axes);

    let (_dir, frame) = frame_at(&moved, "1");
    assert_eq!(
        pixel(&frame, 296, 248),
        RED,
        "data (1, 1) is now at world (2, -1)"
    );
    let inspected = inspect_at(&moved, "1");
    assert_eq!(
        inspected["objects"]["f"]["position"],
        json!([1.0, 0.0, 0.0])
    );

    let move_plot = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "move", "object": "f", "to": [1, 0, 0] }] }"##;
    assert_eq!(
        errors(&format!("{AXES}, {X_SQUARED}"), move_plot),
        pairs(&[("/timeline/0/animations/0/object", "placed_by_axes")])
    );
}

#[test]
fn a_create_draws_the_axes_and_the_plot_along_their_paths() {
    let create = r##"{ "type": "play", "duration": 2, "animations": [
        { "type": "create", "object": "ax", "easing_curve": "linear" }, { "type": "create", "object": "f", "easing_curve": "linear" }] }"##;
    let created = scene(&format!("{AXES}, {X_SQUARED}"), create);

    let (_a, start) = frame_at(&created, "0");
    let (_b, quarter) = frame_at(&created, "0.5");
    let (_c, end) = frame_at(&created, "2");

    assert_eq!(pixel(&start, 248, 248), BLACK);
    assert_eq!(
        pixel(&quarter, 152, 248),
        BLACK,
        "x^2 has gone a quarter of its length from the left, to about data x = -1.4, so data (-1, 1) is not drawn"
    );
    assert_eq!(
        pixel(&quarter, 118, 157),
        RED,
        "data (-1.7, 2.89), local (-1.7, 0.89), is"
    );
    assert_eq!(pixel(&end, 248, 248), RED);
}

#[test]
fn a_number_line_reports_its_ticks_and_a_point_plot_joins_its_points() {
    let objects = r##""n": { "type": "number-line", "range": [0, 10], "length": 8, "position": [0, -3, 0] },
        "ax": { "type": "axes", "x": { "range": [0, 4] }, "y": { "range": [0, 4] }, "width": 4, "height": 4 },
        "p": { "type": "point-plot", "axes": "ax", "points": [[0, 0], [2, 2], [3, 6], [4, 2]], "stroke": { "color": "#ff0000", "width": 0.1 } }"##;
    let scene = scene(objects, "");

    let inspected = inspect_at(&scene, "0");

    let line = &inspected["objects"]["n"]["number_line"];
    assert_eq!(
        line["axis"],
        json!({ "range": [0.0, 10.0], "spacing": 2.0 })
    );
    assert_eq!(rounded(&line["unit"]), [0.8, 0.0, 0.0]);
    assert_eq!(line["ticks"].as_array().unwrap().len(), 6);
    assert_eq!(rounded(&line["ticks"][5]["world"]), [4.0, -3.0, 0.0]);
    let plot = &inspected["objects"]["p"]["plot"];
    assert_eq!(
        (
            rounded(&plot["data_bounding_box"]["min"]),
            rounded(&plot["data_bounding_box"]["max"])
        ),
        (vec![0.0, 0.0], vec![4.0, 4.0]),
        "the point (3, 6) is cut at the top of the box"
    );
}

#[test]
fn a_plot_morphs_into_a_plot_on_the_same_axes_and_no_other() {
    let g = X_SQUARED.replace("\"f\"", "\"g\"").replace("x^2", "2*x");
    let other_axes = AXES.replace("\"ax\"", "\"bx\"");
    let h = X_SQUARED
        .replace("\"f\"", "\"h\"")
        .replace("\"ax\"", "\"bx\"");
    let morph = |from: &str, to: &str| {
        format!(
            r##"{{ "type": "play", "duration": 2, "animations": [{{ "type": "morph", "object": "{from}", "into": "{to}", "easing_curve": "linear" }}] }}"##
        )
    };

    let ok = scene(&format!("{AXES}, {X_SQUARED}, {g}"), &morph("f", "g"));
    let half = inspect_at(&ok, "1");
    assert_eq!(half["objects"]["f"]["morph"]["phase"], "during");
    let (_dir, frame) = frame_at(&ok, "1");
    // At data x = 1 the curves are at y = 1 and y = 2, so halfway at 1.5: local y = -0.5.
    assert_eq!(
        pixel(&frame, 248, 200 + 24),
        RED,
        "x^2 and 2x halfway pass through data (1, 1.5)"
    );
    assert_eq!(
        errors(
            &format!("{AXES}, {other_axes}, {X_SQUARED}, {h}"),
            &morph("f", "h")
        ),
        pairs(&[("/timeline/0/animations/0/into", "not_morphable")])
    );
}

#[test]
fn every_frame_at_the_start_of_a_create_and_a_write_draws() {
    // A create or write begins with slivers of path whose fill has no area; these two objects
    // once made such a sliver into an empty mesh that stopped the frame.
    let scene = r##"{
      "scene_config": { "width": 1920, "height": 1080 },
      "objects": {
        "ax": { "type": "axes", "x": { "range": [-3, 3], "spacing": 1 }, "y": { "range": [-1, 9], "spacing": 2 }, "width": 8, "height": 5 },
        "f": { "type": "function-plot", "axes": "ax", "expression": "x^2", "stroke": { "color": "#fc6255", "width": 0.07 } },
        "t": { "type": "text", "text": "Molecules from PubChem structure files", "size": 0.45, "position": [0, 3.2, 0] }
      },
      "timeline": [
        { "type": "play", "duration": 1.5, "animations": [{ "type": "create", "object": "ax" }, { "type": "create", "object": "f" }] },
        { "type": "play", "duration": 1, "animations": [{ "type": "write", "object": "t" }] }
      ]
    }"##;
    let (dir, path) = scene_file(scene);
    let png = dir.path().join("f.png");

    let failed: Vec<&str> = ["0.016667", "1.716667"]
        .into_iter()
        .filter(|scene_time| {
            sci_anim(&[
                "frame",
                path.to_str().unwrap(),
                "--scene-time",
                scene_time,
                "-o",
                png.to_str().unwrap(),
            ])
            .exit_code
                != 0
        })
        .collect();

    assert_eq!(failed, Vec::<&str>::new());
}

const DEGREES_AXES: &str = r##""ax": { "type": "axes", "x": { "range": [0, 390], "spacing": 90 }, "y": { "range": [-1.2, 1.2], "spacing": 1 }, "width": 7, "height": 3 }"##;

fn crosses_labels(inspected: &Value, plot: &str) -> Value {
    inspected["objects"][plot]["plot"]["crosses_labels"].clone()
}

/// The tick of `axis` at `value` in an axes report.
fn tick<'a>(axes: &'a Value, axis: &str, value: f64) -> &'a Value {
    axes["ticks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tick| tick["axis"] == axis && tick["value"] == value)
        .unwrap()
}

#[test]
fn a_plot_reports_the_tick_labels_its_stroke_crosses() {
    let sine = r##""s": { "type": "function-plot", "axes": "ax", "expression": "sin(x*pi/180)", "x_range": [0, 360], "stroke": { "color": "#ff0000", "width": 0.06 } }"##;

    let inspected = inspect_at(&scene(&format!("{DEGREES_AXES}, {sine}"), ""), "0");

    // The frame shows sin(x) running down through the right of "180" and up through the left of
    // "360", and passing clear of "90" and "270".
    assert_eq!(
        crosses_labels(&inspected, "s"),
        json!([{ "axis": "x", "value": 180.0 }, { "axis": "x", "value": 360.0 }])
    );

    let derivative_axes = r##""ax": { "type": "axes", "width": 7, "height": 5.5, "x": { "range": [-1, 3], "spacing": 1, "label": "x" }, "y": { "range": [-1, 9], "spacing": 2, "label": "y" } }"##;
    let parabola = r##""p": { "type": "function-plot", "axes": "ax", "expression": "x^2", "x_range": [-1, 2.9], "stroke": { "color": "#ff0000", "width": 0.06 } }"##;
    let above = inspect_at(&scene(&format!("{derivative_axes}, {parabola}"), ""), "0");
    assert_eq!(
        crosses_labels(&above, "p"),
        json!([]),
        "x^2 stays above the x axis and right of the y tick labels"
    );
}

#[test]
fn each_tick_reports_its_label_box_and_a_stroke_0_05_clear_of_it_does_not_cross() {
    let inspected = inspect_at(&scene(DEGREES_AXES, ""), "0");
    let axes = &inspected["objects"]["ax"]["axes"];
    let ninety = &tick(axes, "x", 90.0)["label_bounding_box"];

    // The frame lights the glyphs of "90" in pixel columns 102 to 115 and rows 209 to 219,
    // antialiased edges included.
    let pixels: Vec<f64> = [&ninety["pixels"]["min"], &ninety["pixels"]["max"]]
        .iter()
        .flat_map(|corner| rounded(corner))
        .map(f64::round)
        .collect();
    assert_eq!(pixels, [102.0, 210.0, 116.0, 220.0]);

    // A level point plot whose stroke edge, 0.03 from its middle, lies 0.05 below the label's
    // box, and one whose edge lies 0.05 inside it.
    let bottom = ninety["world"]["min"][1].as_f64().unwrap();
    let to_data = |world_y: f64| {
        (world_y - axes["zero"]["world"][1].as_f64().unwrap()) / axes["y_unit"][1].as_f64().unwrap()
    };
    let level = |id: &str, world_y: f64| {
        format!(
            r##""{id}": {{ "type": "point-plot", "axes": "ax", "points": [[60, {y}], [120, {y}]], "stroke": {{ "color": "#ff0000", "width": 0.06 }} }}"##,
            y = to_data(world_y)
        )
    };
    let near = inspect_at(
        &scene(
            &format!(
                "{DEGREES_AXES}, {}, {}",
                level("clear", bottom - 0.03 - 0.05),
                level("into", bottom - 0.03 + 0.05)
            ),
            "",
        ),
        "0",
    );
    assert_eq!(crosses_labels(&near, "clear"), json!([]));
    assert_eq!(
        crosses_labels(&near, "into"),
        json!([{ "axis": "x", "value": 90.0 }])
    );
}

#[test]
fn a_plot_reports_the_axis_label_its_stroke_crosses() {
    let labelled = DEGREES_AXES.replace(
        "\"spacing\": 90 }",
        r#""spacing": 90, "label": "\\theta" }"#,
    );
    // A wide stroke up the right edge of the box reaches past the gap to the label beyond it.
    let wide = r##""e": { "type": "point-plot", "axes": "ax", "points": [[390, -1], [390, 1]], "stroke": { "color": "#ff0000", "width": 0.4 } }"##;

    let inspected = inspect_at(&scene(&format!("{labelled}, {wide}"), ""), "0");

    assert_eq!(
        crosses_labels(&inspected, "e"),
        json!([{ "axis": "x", "label": "\\theta" }])
    );
    let label_box = &inspected["objects"]["ax"]["axes"]["x"]["label_bounding_box"]["world"];
    assert!(
        label_box["min"][0].as_f64().unwrap() > 3.5,
        "the label starts right of the box's right edge, at world x 3.5: {label_box}"
    );
}

/// The axis, value, `label_text` and `label_side` of each tick of an axes or number line report.
fn tick_labels(ticks: &Value) -> Vec<Value> {
    ticks
        .as_array()
        .unwrap()
        .iter()
        .map(|t| json!([t["axis"], t["value"], t["label_text"], t["label_side"]]))
        .collect()
}

#[test]
fn each_tick_reports_the_text_its_label_shows_and_the_side_of_the_tick_it_is_on() {
    let objects = r##""ax": { "type": "axes", "x": { "range": [0, 7], "spacing": 3.141592653589793 }, "y": { "range": [-1.5, 1.5], "spacing": 0.5 }, "width": 7, "height": 3 },
        "n": { "type": "number-line", "range": [-1, 1], "spacing": 0.5, "length": 4, "position": [0, -2.6, 0] },
        "both": { "type": "axes", "x": { "range": [-1, 1], "spacing": 1 }, "y": { "range": [-1, 1], "spacing": 1 }, "width": 1.6, "height": 1.6, "position": [0, 2.6, 0] }"##;

    let inspected = inspect_at(&scene(objects, ""), "0");

    // The frame shows "3.14159265" and "6.28318531" under the x ticks, "0.00000000" in the corner
    // below and left of the crossing, running off the left edge, and "-1.5" to "1.5" left of the
    // y axis.
    let ax = tick_labels(&inspected["objects"]["ax"]["axes"]["ticks"]);
    assert_eq!(
        ax[..4],
        [
            json!(["x", 0.0, "0.00000000", "below-left"]),
            json!(["x", std::f64::consts::PI, "3.14159265", "below"]),
            json!(["x", std::f64::consts::TAU, "6.28318531", "below"]),
            json!(["y", -1.5, "-1.5", "left"]),
        ]
    );
    assert_eq!(ax[6], json!(["y", 0.0, "0.0", "left"]));

    // The number line shows "-1.0" to "1.0" under its ticks.
    assert_eq!(
        tick_labels(&inspected["objects"]["n"]["number_line"]["ticks"]),
        [
            json!(["x", -1.0, "-1.0", "below"]),
            json!(["x", -0.5, "-0.5", "below"]),
            json!(["x", 0.0, "0.0", "below"]),
            json!(["x", 0.5, "0.5", "below"]),
            json!(["x", 1.0, "1.0", "below"]),
        ]
    );

    // Where both crossing labels say "0", the frame shows one "0" below and left of the
    // crossing, lighting pixel columns 187 to 194 and rows 80 to 90, and both ticks report it.
    let both = &inspected["objects"]["both"]["axes"];
    for axis in ["x", "y"] {
        let zero = tick(both, axis, 0.0);
        let pixels: Vec<f64> = [
            &zero["label_bounding_box"]["pixels"]["min"],
            &zero["label_bounding_box"]["pixels"]["max"],
        ]
        .iter()
        .flat_map(|corner| rounded(corner))
        .map(f64::round)
        .collect();
        assert_eq!(
            (
                zero["label_text"].clone(),
                zero["label_side"].clone(),
                pixels
            ),
            (
                json!("0"),
                json!("below-left"),
                vec![188.0, 80.0, 194.0, 90.0]
            ),
            "the {axis} tick at 0"
        );
    }
}

/// Axes whose data units are world units, with sin(x) over them: the hump peaks at world
/// (-1.93, 1) and the trough bottoms at world (1.21, -1).
const SINE_AXES: &str = r##""ax": { "type": "axes", "x": { "range": [0, 7], "spacing": 1 }, "y": { "range": [-1.5, 1.5], "spacing": 0.5 }, "width": 7, "height": 3 },
    "s": { "type": "function-plot", "axes": "ax", "expression": "sin(x)", "x_range": [0, 6.5], "stroke": { "color": "#ff0000", "width": 0.06 } }"##;
const IN_THE_HUMP: &str =
    r##""inside": { "type": "text", "text": "+2", "size": 0.5, "position": [-1.929, 0.45, 0] }"##;
const ON_THE_TROUGH: &str =
    r##""on": { "type": "text", "text": "+2", "size": 0.5, "position": [1.212, -1, 0] }"##;
/// The curve runs down between the "a" and the "b", through the box of the whole text.
const ASTRIDE: &str = r##""between": { "type": "text", "text": "a        b", "size": 0.5, "position": [-0.358, 0.35, 0] }"##;

fn crosses_objects(inspected: &Value, plot: &str) -> Value {
    inspected["objects"][plot]["plot"]["crosses_objects"].clone()
}

#[test]
fn a_plot_reports_the_shown_text_objects_whose_glyphs_its_stroke_crosses() {
    let inspected = inspect_at(
        &scene(
            &format!("{SINE_AXES}, {IN_THE_HUMP}, {ON_THE_TROUGH}, {ASTRIDE}"),
            "",
        ),
        "0",
    );

    // The frame shows the curve through the "+2" on the trough, above the "+2" in the hump, and
    // between the "a" and the "b".
    assert_eq!(crosses_objects(&inspected, "s"), json!(["on"]));

    let hidden = ON_THE_TROUGH.replace("\"size\"", "\"opacity\": 0, \"size\"");
    assert_eq!(
        crosses_objects(
            &inspect_at(&scene(&format!("{SINE_AXES}, {hidden}"), ""), "0"),
            "s"
        ),
        json!([]),
        "a text at opacity 0 does not show"
    );
    let written_later = r##"{ "type": "wait", "duration": 1 }, { "type": "play", "duration": 1, "animations": [{ "type": "write", "object": "on" }] }"##;
    let later = scene(&format!("{SINE_AXES}, {ON_THE_TROUGH}"), written_later);
    assert_eq!(
        crosses_objects(&inspect_at(&later, "0.5"), "s"),
        json!([]),
        "a text not yet written does not show"
    );
    assert_eq!(
        crosses_objects(&inspect_at(&later, "1.5"), "s"),
        json!(["on"]),
        "a text half written shows"
    );
    let hidden_plot = SINE_AXES.replace("\"stroke\"", "\"opacity\": 0, \"stroke\"");
    assert_eq!(
        crosses_objects(
            &inspect_at(&scene(&format!("{hidden_plot}, {ON_THE_TROUGH}"), ""), "0"),
            "s"
        ),
        json!([]),
        "a plot at opacity 0 crosses nothing on the frame"
    );
}

#[test]
fn a_stroke_0_05_clear_of_a_glyph_does_not_cross_it() {
    let inspected = inspect_at(&scene(&format!("{SINE_AXES}, {IN_THE_HUMP}"), ""), "0");
    let top = inspected["objects"]["inside"]["bounding_box"]["world"]["max"][1]
        .as_f64()
        .unwrap();

    // Level point plots across the "+2" whose stroke edge, 0.03 from the middle, lies 0.05 above
    // the top of its tallest glyph, and 0.05 inside it. Data y is world y on these axes.
    let level = |id: &str, y: f64| {
        format!(
            r##""{id}": {{ "type": "point-plot", "axes": "ax", "points": [[1.2, {y}], [1.95, {y}]], "stroke": {{ "color": "#ff0000", "width": 0.06 }} }}"##
        )
    };
    let near = inspect_at(
        &scene(
            &format!(
                "{SINE_AXES}, {IN_THE_HUMP}, {}, {}",
                level("clear", top + 0.03 + 0.05),
                level("into", top + 0.03 - 0.05)
            ),
            "",
        ),
        "0",
    );

    assert_eq!(crosses_objects(&near, "clear"), json!([]));
    assert_eq!(crosses_objects(&near, "into"), json!(["inside"]));
}

#[test]
fn a_text_nearer_the_camera_crosses_a_plot_where_the_frame_shows_it_on_the_curve() {
    // At z = 3 the camera, at z = 10, shows this "+2" exactly where the trough's "+2" shows at
    // z = 0, so the frame shows the curve through it, though in the plane z = 0 it sits clear
    // above the curve.
    let nearer = r##""nearer": { "type": "text", "text": "+2", "size": 0.35, "position": [0.8484, -0.7, 3] }"##;

    let inspected = inspect_at(&scene(&format!("{SINE_AXES}, {nearer}"), ""), "0");

    assert_eq!(crosses_objects(&inspected, "s"), json!(["nearer"]));
}
