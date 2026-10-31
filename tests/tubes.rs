//! Space curves and tubes, read back from frames and reports as an agent reads them.
//!
//! The scenes are 400 by 400 pixels on black, with the camera at `[0, 0, 10]` looking at the
//! origin, so a unit of world at the origin is about 48 pixels.

mod common;

use common::{frame_at, inspect_at, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n.as_f64().unwrap() * 1000.0).round() / 1000.0)
        .collect()
}

fn validate(scene: &str) -> common::Run {
    let (_dir, path) = scene_file(scene);
    sci_anim(&["validate", path.to_str().unwrap()])
}

/// A red tube 0.2 thick along x from -2 to 2.
const ROD: &str = r##""rod": { "type": "tube", "radius": 0.1,
    "curve": { "x": "u", "y": "0", "z": "0", "u_range": [-2, 2] },
    "material": { "type": "standard", "color": "#ff0000" } }"##;

/// The columns of row `y` where red is lit, left first.
fn lit_columns(rgba: &[u8], y: usize) -> Vec<usize> {
    (0..400).filter(|&x| pixel(rgba, x, y)[0] > 100).collect()
}

#[test]
fn a_tube_along_a_line_is_a_lit_rod_with_its_ends_reported() {
    let (_dir, rgba) = frame_at(&scene(ROD, ""), "0");
    let inspected = inspect_at(&scene(ROD, ""), "0");

    let lit = lit_columns(&rgba, 200);
    assert!(
        lit.len() > 180 && lit.len() < 200,
        "4 units long is about 190 pixels: {}",
        lit.len()
    );
    let above_middle = (185..200)
        .filter(|&y| pixel(&rgba, 200, y)[0] > 100)
        .count();
    assert!(
        (3..=7).contains(&above_middle),
        "0.1 in radius is about 5 pixels: {above_middle}"
    );
    let rod = &inspected["objects"]["rod"];
    assert_eq!(numbers(&rod["tube"]["start"]), [-2.0, 0.0, 0.0]);
    assert_eq!(numbers(&rod["tube"]["end"]), [2.0, 0.0, 0.0]);
    assert!(
        (rod["tube"]["start_pixels"][0].as_f64().unwrap() - lit[0] as f64).abs() < 6.0,
        "{}",
        rod["tube"]
    );
    assert_eq!(
        numbers(&rod["bounding_box"]["world"]["min"]),
        [-2.0, -0.1, -0.1]
    );
    assert_eq!(
        numbers(&rod["bounding_box"]["world"]["max"]),
        [2.0, 0.1, 0.1]
    );
    assert_eq!(rod["colors"]["color"], "#ff0000");
}

#[test]
fn a_tubes_ends_move_with_its_transform() {
    let moved = ROD.replace(
        r#""radius": 0.1,"#,
        r#""radius": 0.1, "position": [0, 1, 0], "rotation": [0, 0, 90],"#,
    );

    let inspected = inspect_at(&scene(&moved, ""), "0");

    assert_eq!(
        numbers(&inspected["objects"]["rod"]["tube"]["start"]),
        [0.0, -1.0, 0.0]
    );
    assert_eq!(
        numbers(&inspected["objects"]["rod"]["tube"]["end"]),
        [0.0, 3.0, 0.0]
    );
}

#[test]
fn a_create_draws_a_tube_along_its_curve_by_length() {
    let scene = scene(
        ROD,
        r##"{ "type": "play", "duration": 2, "animations": [{ "type": "create", "object": "rod", "easing_curve": "linear" }] }"##,
    );

    let (_a, start) = frame_at(&scene, "0");
    let (_b, half) = frame_at(&scene, "1");
    let (_c, end) = frame_at(&scene, "2");
    let inspected = inspect_at(&scene, "1");

    assert!(
        lit_columns(&start, 200).is_empty(),
        "nothing shows before the create begins"
    );
    let (half, end) = (lit_columns(&half, 200), lit_columns(&end, 200));
    assert_eq!(half[0], end[0], "drawn from the start of `u_range`");
    let (half_length, end_length) = (half.last().unwrap() - half[0], end.last().unwrap() - end[0]);
    assert!(
        half_length.abs_diff(end_length / 2) <= 6,
        "half its length: {half_length} of {end_length}"
    );
    let rod = &inspected["objects"]["rod"];
    assert_eq!(rod["created"], 0.5);
    assert_eq!(
        numbers(&rod["bounding_box"]["world"]["max"]),
        [2.0, 0.1, 0.1],
        "the whole tube, as for any solid"
    );
    assert_eq!(numbers(&rod["tube"]["end"]), [2.0, 0.0, 0.0]);
}

#[test]
fn a_spring_fits_the_box_of_its_helix() {
    let spring = r##""s": { "type": "tube", "radius": 0.05,
        "curve": { "x": "cos(u)", "y": "0.1*u", "z": "sin(u)", "u_range": [0, 31.4159] } }"##;

    let inspected = inspect_at(&scene(spring, ""), "0");

    let world = &inspected["objects"]["s"]["bounding_box"]["world"];
    let (min, max) = (numbers(&world["min"]), numbers(&world["max"]));
    assert!(
        min[0] >= -1.051 && min[0] <= -1.04 && max[0] <= 1.051 && max[0] >= 1.04,
        "{min:?} {max:?}"
    );
    assert!(
        (min[1] + 0.05).abs() < 0.01 && (max[1] - 3.192).abs() < 0.01,
        "{min:?} {max:?}"
    );
}

#[test]
fn a_closed_curve_shows_no_ends() {
    // A circle of tube in the xy plane facing the camera, lit across where its two ends would meet.
    let round_tube = |range: &str| {
        format!(
            r##""r": {{ "type": "tube", "radius": 0.3, "curve": {{ "x": "2*cos(u)", "y": "2*sin(u)", "z": "0", "u_range": {range} }},
                "material": {{ "type": "standard", "color": "#ff0000" }} }}"##
        )
    };

    let (_a, closed) = frame_at(&scene(&round_tube("[0, 6.2832]"), ""), "0");
    let (_b, open) = frame_at(&scene(&round_tube("[0, 6.25]"), ""), "0");

    // The seam is at +x, about 96 pixels right of the middle. Across a closed tube the red never
    // drops to an edge's dark shade, and across an open one the ends' rims show.
    let darkest = |rgba: &[u8]| (190..211).map(|y| pixel(rgba, 296, y)[0]).min().unwrap();
    assert!(
        darkest(&closed) > 120,
        "no edge across the seam: {}",
        darkest(&closed)
    );
    assert!(
        darkest(&open) < darkest(&closed),
        "the open tube's ends show: {} against {}",
        darkest(&open),
        darkest(&closed)
    );
}

#[test]
fn each_unknown_name_in_a_space_curve_is_a_scene_error_at_its_expression_and_column() {
    let run = validate(&scene(
        r##""s": { "type": "tube", "radius": 0.1, "curve": { "x": "cos(s)", "y": "a*u + b", "z": "sin(u)", "u_range": [0, 1] } }"##,
        "",
    ));

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/s/curve/x", "bad_format"),
            ("/objects/s/curve/y", "bad_format"),
            ("/objects/s/curve/y", "bad_format")
        ])
    );
    let messages: Vec<&str> = run.json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap())
        .collect();
    assert!(messages.contains(&"`x` is not an expression sci_anim can read: unknown name `s`; the variables are `u` and `t`, and the constants are pi, e and tau (at column 5 of \"cos(s)\")"), "{messages:?}");
    assert!(
        messages
            .iter()
            .any(|m| m.contains("unknown name `a`") && m.contains("at column 1 of")),
        "{messages:?}"
    );
    assert!(
        messages
            .iter()
            .any(|m| m.contains("unknown name `b`") && m.contains("at column 7 of")),
        "{messages:?}"
    );
}

#[test]
fn a_space_curve_with_no_value_too_far_or_no_length_is_a_scene_error() {
    let tube = |x: &str, u_range: &str| {
        format!(
            r##""s": {{ "type": "tube", "radius": 0.1, "curve": {{ "x": "{x}", "y": "u", "z": "0", "u_range": {u_range} }} }}"##
        )
    };

    let no_value = validate(&scene(&tube("sqrt(u)", "[-1, 1]"), ""));
    assert_eq!(
        scene_errors(&no_value),
        pairs(&[("/objects/s/curve/x", "out_of_range")])
    );
    assert!(
        no_value.json["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("no finite value at u = -1"),
        "{}",
        no_value.stdout
    );
    assert_eq!(
        scene_errors(&validate(&scene(&tube("exp(u)", "[0, 20]"), ""))),
        pairs(&[("/objects/s/curve/x", "out_of_range")])
    );
    assert_eq!(
        scene_errors(&validate(&scene(&tube("u", "[1, 0]"), ""))),
        pairs(&[("/objects/s/curve/u_range", "invalid")])
    );
    let point = r##""s": { "type": "tube", "radius": 0.1, "curve": { "x": "1", "y": "2", "z": "3", "u_range": [0, 1] } }"##;
    assert_eq!(
        scene_errors(&validate(&scene(point, ""))),
        pairs(&[("/objects/s/curve", "invalid")])
    );
}

/// A tube through three points, an arch from x = -2 to 2 with its top at y = 1.
const ARCH: &str = r##""arch": { "type": "tube", "radius": 0.1,
    "curve": { "points": [[-2, 0, 0], [0, 1, 0], [2, 0, 0]] },
    "material": { "type": "standard", "color": "#ff0000" } }"##;

#[test]
fn a_tube_through_points_passes_through_each_and_reports_its_first_and_last() {
    let inspected = inspect_at(&scene(ARCH, ""), "0");
    let (_dir, rgba) = frame_at(&scene(ARCH, ""), "0");

    let arch = &inspected["objects"]["arch"];
    assert_eq!(numbers(&arch["tube"]["start"]), [-2.0, 0.0, 0.0]);
    assert_eq!(numbers(&arch["tube"]["end"]), [2.0, 0.0, 0.0]);
    let top = arch["bounding_box"]["world"]["max"][1].as_f64().unwrap();
    assert!(
        (top - 1.1).abs() < 0.002,
        "the middle point is the top, with no overshoot past it: {top}"
    );
    // y = 1 lands about 48 pixels above the middle row, and the tube's top about 53.
    let lit_in_middle_column: Vec<usize> = (0..400)
        .filter(|&y| pixel(&rgba, 200, y)[0] > 100)
        .collect();
    assert!(
        (145..=150).contains(&lit_in_middle_column[0])
            && (153..=158).contains(lit_in_middle_column.last().unwrap()),
        "{lit_in_middle_column:?}"
    );
}

#[test]
fn a_carry_along_a_tube_through_points_leaves_along_its_first_chord() {
    let objects = format!(
        r##"{ARCH}, "bead": {{ "type": "sphere", "radius": 0.15, "position": [-2, 0, 0] }},
        "flat": {{ "type": "tube", "radius": 0.05, "curve": {{ "points": [[0, -2, 0], [1, -2, 0], [2, -1, 0]] }} }},
        "arrow": {{ "type": "polygon", "points": [[0.2, 0], [-0.1, 0.1], [-0.1, -0.1]], "position": [0, -2, 0] }}"##
    );
    let scene = scene(
        &objects,
        r#"{ "type": "play", "duration": 1, "animations": [
            { "type": "follow", "object": "bead", "along": "arch", "easing_curve": "linear" },
            { "type": "follow", "object": "arrow", "along": "flat", "faces": "tangent", "easing_curve": "linear" }
        ] }"#,
    );

    assert_eq!(
        validate(&scene).exit_code,
        0,
        "the arrow stands unturned at the first point, its chord along +x: {}",
        validate(&scene).stdout
    );
    let end = inspect_at(&scene, "1");
    assert_eq!(
        numbers(&end["objects"]["bead"]["position"]),
        [2.0, 0.0, 0.0]
    );
    assert_eq!(
        numbers(&end["objects"]["arrow"]["position"]),
        [2.0, -1.0, 0.0]
    );
    assert_eq!(
        numbers(&end["objects"]["arrow"]["rotation"]),
        [0.0, 0.0, 45.0],
        "reaches the last point along the last chord"
    );
}

#[test]
fn copies_along_points_sit_at_the_ends_and_between_by_length() {
    let objects = r##""p": { "type": "pattern", "shape": { "type": "dot", "radius": 0.05 },
        "layout": { "type": "along", "count": 3, "curve": { "points": [[0, 0, 0], [1, 0, 0], [4, 0, 0]] } } }"##;

    let copies = inspect_at(&scene(objects, ""), "0")["objects"]["p"]["pattern"]["copies"].clone();

    let xs: Vec<f64> = copies
        .as_array()
        .unwrap()
        .iter()
        .map(|copy| numbers(&copy["position"])[0])
        .collect();
    assert_eq!(xs, [0.0, 2.0, 4.0]);
}

#[test]
fn a_bad_points_list_is_a_scene_error_at_its_point() {
    let tube = |points: &str| {
        format!(r##""s": {{ "type": "tube", "radius": 0.1, "curve": {{ "points": {points} }} }}"##)
    };
    let errors = |points: &str| scene_errors(&validate(&scene(&tube(points), "")));

    assert_eq!(
        errors("[[0, 0, 0]]"),
        pairs(&[("/objects/s/curve/points", "wrong_length")])
    );
    assert_eq!(
        errors("[[0, 0, 0], [1, 2]]"),
        pairs(&[("/objects/s/curve/points/1", "wrong_length")])
    );
    assert_eq!(
        errors("[[0, 0, 0], [1, 2, 3], [1, 20000, 3]]"),
        pairs(&[("/objects/s/curve/points/2/1", "out_of_range")])
    );
    assert_eq!(
        errors(r#"[[0, 0, 0], [1, "a", 3]]"#),
        pairs(&[("/objects/s/curve/points/1/1", "wrong_type")])
    );
    assert_eq!(
        errors("[[1, 2, 3], [1, 2, 3]]"),
        pairs(&[("/objects/s/curve/points", "invalid")])
    );
    let not_a_number = validate(&scene(&tube("[[0, 0, 0], [1, NaN, 3]]"), ""));
    assert_eq!(not_a_number.exit_code, 1);
    assert_eq!(
        scene_errors(&not_a_number),
        pairs(&[("", "invalid_json")]),
        "JSON has no NaN, so the file is not JSON"
    );
    let along = r##""p": { "type": "pattern", "shape": { "type": "dot", "radius": 0.05 },
        "layout": { "type": "along", "count": 3, "curve": { "points": [[0, 0, 0], [1, 0]] } } }"##;
    assert_eq!(
        scene_errors(&validate(&scene(along, ""))),
        pairs(&[("/objects/p/layout/curve/points/1", "wrong_length")])
    );
}

#[test]
fn a_tube_is_a_solid_that_cannot_be_see_through_morph_or_be_mapped() {
    let see_through = ROD.replace(
        r##""color": "#ff0000""##,
        r##""color": "#ff0000", "opacity": 0.5"##,
    );
    assert_eq!(
        scene_errors(&validate(&scene(&see_through, ""))),
        pairs(&[("/objects/rod/material/opacity", "invalid")])
    );

    let objects = format!(
        r##"{ROD}, "c": {{ "type": "circle", "radius": 1, "stroke": {{ "color": "#ffffff" }} }}"##
    );
    let run = validate(&scene(
        &objects,
        r##"{ "type": "play", "duration": 1, "animations": [{ "type": "morph", "object": "rod", "into": "c" }] },
            { "type": "play", "duration": 1, "animations": [{ "type": "linear-map", "objects": ["rod"], "matrix": [[2, 0], [0, 1]] }] }"##,
    ));
    let codes: Vec<String> = scene_errors(&run)
        .into_iter()
        .map(|(_, code)| code)
        .collect();
    assert_eq!(codes, ["not_morphable", "not_mappable"], "{}", run.stdout);
}

#[test]
fn a_helix_type_names_the_spring_and_the_tube_that_draw_one() {
    let run = validate(&scene(r##""h": { "type": "helix" }"##, ""));

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/h/type", "unknown_value")])
    );
    let message = run.json["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("a coil between two points is a `spring`")
            && message.contains("a helix on its own is a `tube`"),
        "{message}"
    );
}

#[test]
fn the_new_functions_draw_in_a_function_plot() {
    // step(x) is 0 left of 0 and 1 from 0 on, so the plot's data box runs from y = 0 to 1.
    let objects = r##""ax": { "type": "axes", "x": { "range": [-2, 2] }, "y": { "range": [-2, 2] }, "width": 4, "height": 4 },
        "f": { "type": "function-plot", "axes": "ax", "expression": "step(x) + mod(x, 1)*0 + atan2(0, 1)" }"##;

    let inspected = inspect_at(&scene(objects, ""), "0");

    let data = &inspected["objects"]["f"]["plot"]["data_bounding_box"];
    assert_eq!(
        numbers(&data["min"]),
        [-2.0, 0.0],
        "{}",
        inspected["objects"]["f"]
    );
    assert_eq!(numbers(&data["max"]), [2.0, 1.0]);
}

#[test]
fn the_schema_describes_each_expression_in_its_own_variables() {
    let schema = sci_anim(&["schema"]).json;
    let text = schema.to_string();

    assert!(text.contains("An expression in `x`, such as `x^2 - 3*sin(2*x)`, which gives y for each x in data units. It may also read `t`, law time"));
    assert!(text.contains(
        "An expression in `u`, such as `cos(u)`, which gives the curve's z in world units"
    ));
    assert!(text.contains("`atan2(y, x)` is the angle of the point (x, y)"));
}

/// The arch, red and 0.1 thick, and a blue valley 0.3 thick through `valley_points`, and a morph
/// of the arch into the valley over scene time 0 to 1 at one speed.
fn arch_into_valley(valley_points: &str) -> String {
    let objects = format!(
        r##"{ARCH}, "valley": {{ "type": "tube", "radius": 0.3,
        "curve": {{ "points": {valley_points} }},
        "material": {{ "type": "standard", "color": "#0000ff" }} }}"##
    );
    scene(
        &objects,
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "morph", "object": "arch", "into": "valley", "easing_curve": "linear" }] }"#,
    )
}

fn curve_points(tube: &Value) -> Vec<Vec<f64>> {
    tube["tube"]["curve_points"]
        .as_array()
        .unwrap_or_else(|| panic!("a tube through points reports its curve points: {tube}"))
        .iter()
        .map(numbers)
        .collect()
}

#[test]
fn a_tube_morphs_curve_point_by_curve_point_into_another() {
    let scene = arch_into_valley("[[-2, -2, 0], [0, -3, 0], [2, -2, 0]]");

    let start = inspect_at(&scene, "0");
    let halfway = inspect_at(&scene, "0.5");
    let end = inspect_at(&scene, "1");

    assert_eq!(
        curve_points(&start["objects"]["arch"]),
        [[-2.0, 0.0, 0.0], [0.0, 1.0, 0.0], [2.0, 0.0, 0.0]]
    );
    assert_eq!(
        curve_points(&halfway["objects"]["arch"]),
        [[-2.0, -1.0, 0.0], [0.0, -1.0, 0.0], [2.0, -1.0, 0.0]],
        "each curve point halfway to its partner, so the curve is a straight rod"
    );
    let rod = &halfway["objects"]["arch"]["bounding_box"]["world"];
    assert_eq!(
        (numbers(&rod["min"]), numbers(&rod["max"])),
        (vec![-2.0, -1.2, -0.2], vec![2.0, -0.8, 0.2]),
        "the radius is halfway from 0.1 to 0.3"
    );
    assert_eq!(halfway["objects"]["arch"]["colors"]["color"], "#bc00bc");
    assert_eq!(
        curve_points(&end["objects"]["valley"]),
        [[-2.0, -2.0, 0.0], [0.0, -3.0, 0.0], [2.0, -2.0, 0.0]]
    );
}

#[test]
fn a_morph_of_a_tube_needs_a_tube_through_as_many_curve_points() {
    let morph = |object: &str, into: &str| {
        format!(
            r#"{{ "type": "play", "duration": 1, "animations": [{{ "type": "morph", "object": "{object}", "into": "{into}" }}] }}"#
        )
    };
    let objects = format!(
        r##"{ARCH}, {ROD},
        "four": {{ "type": "tube", "radius": 0.1, "curve": {{ "points": [[0, 0, 0], [1, 0, 0], [2, 1, 0], [3, 0, 0]] }} }},
        "c": {{ "type": "circle", "radius": 1, "stroke": {{ "color": "#ffffff" }} }},
        "ball": {{ "type": "sphere", "radius": 1 }}"##
    );
    let errors =
        |object: &str, into: &str| scene_errors(&validate(&scene(&objects, &morph(object, into))));

    assert_eq!(
        errors("arch", "four"),
        pairs(&[("/timeline/0/animations/0/into", "wrong_length")])
    );
    assert_eq!(
        errors("arch", "c"),
        pairs(&[("/timeline/0/animations/0/into", "not_morphable")])
    );
    assert_eq!(
        errors("arch", "ball"),
        pairs(&[("/timeline/0/animations/0/into", "not_morphable")])
    );
    assert_eq!(
        errors("c", "arch"),
        pairs(&[("/timeline/0/animations/0/into", "not_morphable")])
    );
    assert_eq!(
        errors("rod", "arch"),
        pairs(&[("/timeline/0/animations/0/object", "not_morphable")]),
        "a curve of expressions has no curve points to pair"
    );
}

const STRUCTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/skills/sci-anim/examples/structures"
);

#[test]
fn an_atom_trace_passes_through_each_ca_atom_where_the_molecule_draws_it() {
    let objects = format!(
        r##""backbone": {{ "type": "tube", "radius": 0.5, "scale": [0.1, 0.1, 0.1],
            "curve": {{ "trace": {{ "file": "{STRUCTURES}/1ubq.pdb", "atom_name": "CA", "chain": "A" }} }} }},
        "ub": {{ "type": "molecule", "file": "{STRUCTURES}/1ubq.pdb", "scale": [0.1, 0.1, 0.1] }}"##
    );

    let inspected = inspect_at(&scene(&objects, ""), "0");

    let points = curve_points(&inspected["objects"]["backbone"]);
    assert_eq!(points.len(), 76, "ubiquitin has 76 amino acids");
    let atom = |number: u64| {
        let atoms = inspected["objects"]["ub"]["atoms"].as_array().unwrap();
        let found = atoms.iter().find(|a| a["number"] == number).unwrap();
        numbers(&found["position"])
    };
    assert_eq!(points[0], atom(2), "atom 2 is MET 1's CA");
    assert_eq!(points[75], atom(599), "atom 599 is GLY 76's CA");
}

#[test]
fn an_atom_trace_the_file_cannot_give_is_a_scene_error_at_its_field() {
    let trace = |id: &str, fields: &str| {
        format!(
            r##""{id}": {{ "type": "tube", "radius": 0.5, "curve": {{ "trace": {{ {fields} }} }} }}"##
        )
    };
    let caffeine = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/pubchem/caffeine.sdf"
    );
    let objects = [
        trace(
            "name",
            &format!(r#""file": "{STRUCTURES}/1ubq.pdb", "atom_name": "P""#),
        ),
        trace(
            "chain",
            &format!(r#""file": "{STRUCTURES}/1ubq.pdb", "atom_name": "CA", "chain": "B""#),
        ),
        trace(
            "sdf",
            &format!(r#""file": "{caffeine}", "atom_name": "CA""#),
        ),
        trace("missing", r#""file": "nowhere.pdb", "atom_name": "CA""#),
    ]
    .join(", ");

    let run = validate(&scene(&objects, ""));

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/chain/curve/trace/chain", "invalid"),
            ("/objects/missing/curve/trace/file", "unreadable_file"),
            ("/objects/name/curve/trace/atom_name", "invalid"),
            ("/objects/sdf/curve/trace/file", "invalid"),
        ])
    );
}
