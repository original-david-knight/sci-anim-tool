//! Tables, driven as an agent drives them: their layout, their write, a highlight of one cell,
//! `inspect`'s `table`, `check-layout` reading each cell as text, and their scene errors.

mod common;

use std::process::Command;

use common::{frame_at, inspect_at, pairs, scene, scene_errors, scene_file, sci_anim};

fn errors(objects: &str, timeline: &str) -> Vec<(String, String)> {
    let (_dir, path) = scene_file(&scene(objects, timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    scene_errors(&run)
}

fn world(rectangle: &serde_json::Value) -> Vec<f64> {
    ["min", "max"]
        .iter()
        .flat_map(|end| (0..2).map(move |i| rectangle["world"][end][i].as_f64().unwrap()))
        .map(|v| (v * 1000.0).round() / 1000.0)
        .collect()
}

#[test]
fn empty_cells_are_as_wide_and_tall_as_their_padding_and_the_table_is_centered() {
    let report = inspect_at(
        &scene(
            r#""t": { "type": "table", "padding": 0.5, "position": [1, 0, 0], "rows": [["", ""], ["", ""]] }"#,
            "",
        ),
        "0",
    );
    let table = &report["objects"]["t"]["table"];
    assert_eq!(
        (table["rows"].as_u64(), table["columns"].as_u64()),
        (Some(2), Some(2))
    );
    let cells = table["cells"].as_array().unwrap();
    assert_eq!(cells[1]["cell"], serde_json::json!([0, 1]));
    assert_eq!(world(&cells[0]["rectangle"]), [0.0, 0.0, 1.0, 1.0]);
    assert_eq!(world(&cells[1]["rectangle"]), [1.0, 0.0, 2.0, 1.0]);
    assert_eq!(world(&cells[2]["rectangle"]), [0.0, -1.0, 1.0, 0.0]);
    assert!(cells[3]["bounding_box"].is_null());
}

#[test]
fn a_cell_reports_its_text_or_tex_and_its_glyphs_lie_inside_its_rectangle() {
    let report = inspect_at(
        &scene(
            r#""t": { "type": "table", "size": 0.5, "rows": [["Codon", { "tex": "x^2" }], ["AUG", "Met"]] }"#,
            "",
        ),
        "0",
    );
    let cells = report["objects"]["t"]["table"]["cells"].as_array().unwrap();
    assert_eq!(cells[0]["text"], "Codon");
    assert_eq!(cells[1]["tex"], "x^2");
    assert_eq!(cells[3]["text"], "Met");
    for cell in cells {
        let (glyphs, rectangle) = (world(&cell["bounding_box"]), world(&cell["rectangle"]));
        assert!(
            rectangle[0] < glyphs[0]
                && rectangle[1] < glyphs[1]
                && glyphs[2] < rectangle[2]
                && glyphs[3] < rectangle[3],
            "{cell}"
        );
    }
}

#[test]
fn a_write_draws_the_rulings_then_the_cells_in_reading_order() {
    let table = scene(
        r#""t": { "type": "table", "rows": [["a", "b"], ["c", "d"], ["e", "f"]] }"#,
        r#"{ "type": "play", "duration": 5, "animations": [{ "type": "write", "object": "t", "easing_curve": "linear" }] }"#,
    );
    let created = |scene_time: &str| -> Vec<f64> {
        inspect_at(&table, scene_time)["objects"]["t"]["table"]["cells"]
            .as_array()
            .unwrap()
            .iter()
            .map(|cell| cell["created"].as_f64().unwrap())
            .collect()
    };
    assert_eq!(created("0.5"), [0.0; 6]);
    let halfway = created("3");
    assert_eq!(halfway[0], 1.0);
    assert!(
        halfway[5] < 0.001,
        "the last cell has barely begun: {halfway:?}"
    );
    assert_eq!(created("5"), [1.0; 6]);
}

#[test]
fn a_highlight_lights_one_cell_behind_its_text() {
    let lit = scene(
        r#""t": { "type": "table", "padding": 0.5, "rulings": "none", "position": [0, 0, 0], "rows": [["", ""]] }"#,
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "highlight", "object": "t", "cell": [0, 1], "to": 1 }] }"#,
    );
    let report = inspect_at(&lit, "1");
    let cells = &report["objects"]["t"]["table"]["cells"];
    assert_eq!(
        (cells[0]["glow"].as_f64(), cells[1]["glow"].as_f64()),
        (Some(0.0), Some(1.0))
    );
    assert_eq!(cells[1]["glow_color"], "#ffd23f");
    let (_dir, rgba) = frame_at(&lit, "1");
    let pixel = |x: usize, y: usize| &rgba[(y * 400 + x) * 4..(y * 400 + x) * 4 + 3];
    assert_eq!(pixel(180, 200), [0, 0, 0], "the unlit cell, left of center");
    assert_eq!(
        pixel(220, 200),
        [137, 112, 29],
        "the lit cell, #ffd23f at 0.25 in linear light"
    );
}

#[test]
fn check_layout_reads_each_cell_as_a_text_object() {
    let (dir, path) = scene_file(&scene(
        r#""t": { "type": "table", "size": 0.1, "rows": [["a", "b"]] },
        "u": { "type": "table", "size": 0.6, "position": [0, -2, 0], "rows": [["lit"]] }"#,
        r##"{ "type": "play", "duration": 1, "animations": [{ "type": "highlight", "object": "u", "cell": [0, 0], "to": 1, "color": "#ffffff" }] }"##,
    ));
    let out = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .arg("check-layout")
        .arg(&path)
        .arg("--no-sheet")
        .arg("--out")
        .arg(dir.path().join("check"))
        .output()
        .unwrap();
    let printed = String::from_utf8(out.stdout).unwrap();
    assert_eq!(out.status.code(), Some(1), "{printed}");
    assert!(
        printed.contains(r#""t/0/0" is too small to read"#),
        "{printed}"
    );
    assert!(
        printed.contains(r#""t/0/1" is too small to read"#),
        "{printed}"
    );
    assert!(
        printed.contains(r#""u/0/0" is hard to read: #ffffff on the fill behind it"#),
        "{printed}"
    );
}

#[test]
fn validate_refuses_a_bad_table_or_cell_at_its_field() {
    assert_eq!(
        errors(
            r#""t": { "type": "table", "rows": [["a", "b"], ["c"]] }"#,
            ""
        ),
        pairs(&[("/objects/t/rows/1", "wrong_length")])
    );
    assert_eq!(
        errors(r#""t": { "type": "table", "rows": [] }"#, ""),
        pairs(&[("/objects/t/rows", "wrong_length")])
    );
    let lit = |animation: &str| {
        errors(
            r#""t": { "type": "table", "rows": [["a", "b"]] }, "c": { "type": "circle", "radius": 1 }"#,
            &format!(r#"{{ "type": "play", "duration": 1, "animations": [{animation}] }}"#),
        )
    };
    assert_eq!(
        lit(r#"{ "type": "highlight", "object": "t", "cell": [1, 0], "to": 1 }"#),
        pairs(&[("/timeline/0/animations/0/cell", "unknown_cell")])
    );
    assert_eq!(
        lit(r#"{ "type": "highlight", "object": "c", "cell": [0, 0], "to": 1 }"#),
        pairs(&[("/timeline/0/animations/0/cell", "not_table")])
    );
    assert_eq!(
        lit(r#"{ "type": "highlight", "object": "t", "group": "a", "to": 1 }"#),
        pairs(&[("/timeline/0/animations/0/group", "unknown_group")])
    );
    assert_eq!(
        lit(r#"{ "type": "create", "object": "t" }"#),
        pairs(&[("/timeline/0/animations/0/object", "use_write")])
    );
}
