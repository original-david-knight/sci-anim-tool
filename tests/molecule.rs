//! Molecules read from PubChem structure files, drawn and inspected as an agent sees them.
//!
//! The fixtures under `tests/fixtures/pubchem` are PubChem's 3D records for caffeine (24 atoms,
//! 25 bonds), benzene (12 atoms, 9 single and 3 double bonds), ethanol (9 atoms, 8 single
//! bonds) and acetylene (4 atoms, one triple bond), and the page curl saved for insulin, which
//! has no 3D model. Scenes are 400 by 400 pixels on black with the default camera, so a world
//! point (x, y) in the plane z = 0 lands at pixel (200 + 48.28 x, 200 - 48.28 y).

mod common;

use common::{frame_at, inspect_at, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pubchem");

fn molecule(id: &str, sdf: &str, more: &str) -> String {
    format!(r##""{id}": {{ "type": "molecule", "file": "{FIXTURES}/{sdf}.sdf"{more} }}"##)
}

/// The bonds of an inspected molecule with the given bond order.
fn bonds_of(order: &str, inspected: &Value, id: &str) -> usize {
    inspected["objects"][id]["bonds"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|b| b["order"] == order)
        .count()
}

fn atoms<'a>(inspected: &'a Value, id: &str) -> &'a Vec<Value> {
    inspected["objects"][id]["atoms"].as_array().unwrap()
}

fn hidden(rows: &[Value]) -> usize {
    rows.iter().filter(|row| row["hidden"] == true).count()
}

fn pixel_of(atom: &Value) -> (usize, usize) {
    let pixels = &atom["pixels"];
    (
        pixels[0].as_f64().unwrap().round() as usize,
        pixels[1].as_f64().unwrap().round() as usize,
    )
}

fn within(actual: [u8; 4], expected: [u8; 3], tolerance: u8) -> bool {
    actual[..3]
        .iter()
        .zip(expected)
        .all(|(a, e)| a.abs_diff(e) <= tolerance)
        && actual[3] == 255
}

#[test]
fn caffeine_has_24_atoms_and_25_bonds_four_of_them_double() {
    let inspected = inspect_at(&scene(&molecule("caf", "caffeine", ""), ""), "0");

    assert_eq!(atoms(&inspected, "caf").len(), 24);
    let bonds = inspected["objects"]["caf"]["bonds"].as_array().unwrap();
    assert_eq!(bonds.len(), 25);
    assert_eq!(
        (
            bonds_of("single", &inspected, "caf"),
            bonds_of("double", &inspected, "caf")
        ),
        (21, 4)
    );
    let first = &atoms(&inspected, "caf")[0];
    assert_eq!(
        (first["number"].clone(), first["element"].clone()),
        (1.into(), "O".into()),
        "atoms are numbered as the file numbers them"
    );
    assert_eq!(bonds[0]["atoms"], serde_json::json!([1, 9]));
}

#[test]
fn benzene_has_12_atoms_9_single_bonds_and_3_double_bonds_as_pubchem_writes_it() {
    let inspected = inspect_at(&scene(&molecule("ring", "benzene", ""), ""), "0");

    assert_eq!(atoms(&inspected, "ring").len(), 12);
    assert_eq!(
        (
            bonds_of("single", &inspected, "ring"),
            bonds_of("double", &inspected, "ring")
        ),
        (9, 3)
    );
}

#[test]
fn acetylene_has_one_triple_bond_and_ethanol_no_multiple_bond() {
    let inspected = inspect_at(
        &scene(
            &format!(
                "{}, {}",
                molecule("alkyne", "acetylene", ""),
                molecule("alcohol", "ethanol", "")
            ),
            "",
        ),
        "0",
    );

    assert_eq!(
        (
            atoms(&inspected, "alkyne").len(),
            bonds_of("triple", &inspected, "alkyne")
        ),
        (4, 1)
    );
    assert_eq!(
        (
            atoms(&inspected, "alcohol").len(),
            bonds_of("single", &inspected, "alcohol")
        ),
        (9, 8)
    );
}

#[test]
fn hiding_hydrogens_hides_ten_atoms_and_their_ten_bonds_and_deletes_none() {
    let shown = inspect_at(
        &scene(&molecule("caf", "caffeine", r#", "hydrogens": true"#), ""),
        "0",
    );
    let hidden_h = inspect_at(
        &scene(&molecule("caf", "caffeine", r#", "hydrogens": false"#), ""),
        "0",
    );

    assert_eq!(
        (
            hidden(atoms(&shown, "caf")),
            hidden(shown["objects"]["caf"]["bonds"].as_array().unwrap())
        ),
        (0, 0)
    );
    assert_eq!(
        atoms(&hidden_h, "caf").len(),
        24,
        "a hidden hydrogen is still an atom"
    );
    assert_eq!(hidden(atoms(&hidden_h, "caf")), 10);
    assert_eq!(
        hidden(hidden_h["objects"]["caf"]["bonds"].as_array().unwrap()),
        10
    );
    let hydrogens: Vec<&Value> = atoms(&hidden_h, "caf")
        .iter()
        .filter(|a| a["element"] == "H")
        .collect();
    assert_eq!(
        (
            hydrogens.len(),
            hydrogens.iter().all(|a| a["hidden"] == true)
        ),
        (10, true),
        "exactly the hydrogens"
    );
    assert_eq!(
        atoms(&shown, "caf")[14]["position"],
        atoms(&hidden_h, "caf")[14]["position"],
        "hiding moves nothing"
    );
}

#[test]
fn hidden_hydrogens_shrink_the_bounding_box_to_what_is_drawn() {
    let x_range = |hydrogens: &str| {
        let inspected = inspect_at(
            &scene(
                &molecule("caf", "caffeine", &format!(r#", "hydrogens": {hydrogens}"#)),
                "",
            ),
            "0",
        );
        let world = &inspected["objects"]["caf"]["bounding_box"]["world"];
        (
            world["min"][0].as_f64().unwrap(),
            world["max"][0].as_f64().unwrap(),
        )
    };

    let (shown_min, shown_max) = x_range("true");
    let (hidden_min, hidden_max) = x_range("false");

    assert!(
        (shown_max - 4.436).abs() < 1e-3,
        "H19 at x 4.1992 less the centroid's 0.0636, plus its ball of 0.3: {shown_max}"
    );
    assert!(
        (hidden_max - 3.554).abs() < 1e-3,
        "then C13 at x 3.1926 and its ball of 0.425: {hidden_max}"
    );
    assert!(
        (shown_min - -3.578).abs() < 1e-3 && (hidden_min - -3.578).abs() < 1e-3,
        "the oxygen O2, with its ball of 0.3875, is leftmost either way: {shown_min} {hidden_min}"
    );
}

#[test]
fn the_pixel_inspect_reports_for_an_oxygen_shows_oxygen_red() {
    let scene = scene(&molecule("caf", "caffeine", ""), "");

    let inspected = inspect_at(&scene, "0");
    let (_dir, rgba) = frame_at(&scene, "0");

    let oxygen = &atoms(&inspected, "caf")[0];
    let (x, y) = pixel_of(oxygen);
    // A ball's middle faces the camera, which the key light shades to two thirds of its color
    // in linear light.
    assert!(
        within(pixel(&rgba, x, y), [0xd5, 0x09, 0x09], 12),
        "{:?} at ({x}, {y})",
        pixel(&rgba, x, y)
    );
    let nitrogen = pixel_of(&atoms(&inspected, "caf")[2]);
    assert!(
        within(pixel(&rgba, nitrogen.0, nitrogen.1), [0x26, 0x41, 0xcf], 12),
        "N3 is blue: {:?}",
        pixel(&rgba, nitrogen.0, nitrogen.1)
    );
}

#[test]
fn hiding_hydrogens_draws_nothing_where_a_hydrogen_was() {
    let with = scene(&molecule("caf", "caffeine", r#", "hydrogens": true"#), "");
    let without = scene(&molecule("caf", "caffeine", r#", "hydrogens": false"#), "");

    let inspected = inspect_at(&with, "0");
    let (_a, shown) = frame_at(&with, "0");
    let (_b, hidden_frame) = frame_at(&without, "0");

    let hydrogen = pixel_of(&atoms(&inspected, "caf")[14]);
    assert!(
        within(pixel(&shown, hydrogen.0, hydrogen.1), [255, 255, 255], 12),
        "H15 is white: {:?}",
        pixel(&shown, hydrogen.0, hydrogen.1)
    );
    assert_eq!(
        pixel(&hidden_frame, hydrogen.0, hydrogen.1),
        [0, 0, 0, 255],
        "nothing is drawn there once hydrogens are hidden"
    );
    let oxygen = pixel_of(&atoms(&inspected, "caf")[0]);
    assert_eq!(
        pixel(&shown, oxygen.0, oxygen.1),
        pixel(&hidden_frame, oxygen.0, oxygen.1),
        "the oxygen does not change"
    );
}

#[test]
fn a_double_bond_is_two_sticks_with_a_gap_between_them() {
    let scene = format!(
        r##"{{ "scene_config": {{ "width": 400, "height": 400, "background": "#000000" }}, "camera": {{ "position": [0, 0, 20] }},
              "objects": {{ {} }} }}"##,
        molecule(
            "alkene",
            "benzene",
            r#", "hydrogens": false, "scale": [4, 4, 4]"#
        )
    );

    let inspected = inspect_at(&scene, "0");
    let (_dir, rgba) = frame_at(&scene, "0");

    let double = inspected["objects"]["alkene"]["bonds"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["order"] == "double")
        .unwrap();
    let [a, b] = [0, 1].map(|i| double["atoms"][i].as_u64().unwrap() as usize - 1);
    let (a, b) = (
        pixel_of(&atoms(&inspected, "alkene")[a]),
        pixel_of(&atoms(&inspected, "alkene")[b]),
    );
    let midpoint = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
    let covered = |x: usize, y: usize| pixel(&rgba, x, y) != [0, 0, 0, 255];
    let (dx, dy) = (b.0 as f64 - a.0 as f64, b.1 as f64 - a.1 as f64);
    let length = dx.hypot(dy);
    let across = |pixels: f64| {
        (
            (midpoint.0 as f64 - dy / length * pixels).round() as usize,
            (midpoint.1 as f64 + dx / length * pixels).round() as usize,
        )
    };
    assert!(
        !covered(midpoint.0, midpoint.1),
        "the gap between the two sticks, on the bond's axis"
    );
    let (left, right) = (across(-0.09 * 4.0 * 24.14), across(0.09 * 4.0 * 24.14));
    assert!(
        covered(left.0, left.1) && covered(right.0, right.1),
        "a stick on each side"
    );
}

#[test]
fn a_structure_file_path_is_relative_to_the_scene_file_not_to_where_the_tool_runs() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("structures")).unwrap();
    std::fs::copy(
        format!("{FIXTURES}/ethanol.sdf"),
        dir.path().join("structures/ethanol.sdf"),
    )
    .unwrap();
    let scene_path = dir.path().join("scene.json");
    std::fs::write(
        &scene_path,
        r#"{ "objects": { "e": { "type": "molecule", "file": "structures/ethanol.sdf" } } }"#,
    )
    .unwrap();

    let run = sci_anim(&["validate", scene_path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 0, "{}", run.stdout);
}

#[test]
fn a_missing_structure_file_is_a_scene_error_at_its_file_that_says_where_it_looked() {
    let (dir, path) =
        scene_file(r#"{ "objects": { "caf": { "type": "molecule", "file": "caffeine.sdf" } } }"#);

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        run.exit_code, 1,
        "a scene file at fault, not the tool: {}",
        run.stdout
    );
    assert_eq!(
        scene_errors(&run),
        common::pairs(&[("/objects/caf/file", "unreadable_file")])
    );
    let message = run.json["errors"][0]["message"].as_str().unwrap();
    assert!(
        message
            .starts_with("`caffeine.sdf` cannot be read (No such file or directory (os error 2))"),
        "{message}"
    );
    assert!(
        message.contains(&format!("looked for {}/caffeine.sdf", dir.path().display())),
        "{message}"
    );
    assert!(
        message.ends_with("download it first, such as from PubChem with `curl -fo caffeine.sdf \"https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/<name>/SDF?record_type=3d\"`"),
        "{message}"
    );
}

#[test]
fn a_2d_structure_file_is_a_scene_error_that_says_how_to_fetch_the_3d_one() {
    let flat = std::fs::read_to_string(format!("{FIXTURES}/caffeine-2d.sdf")).unwrap();

    let message = bad_file_message(&flat);

    assert_eq!(
        message,
        "`bad.sdf` is not a structure file sci_anim can draw: it is a 2D structure file, whose atoms all lie flat; fetch the 3D one by adding `?record_type=3d` to the PubChem URL; its first line is \"2519\""
    );
}

fn bad_file_message(sdf: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("bad.sdf"), sdf).unwrap();
    let scene_path = dir.path().join("scene.json");
    std::fs::write(
        &scene_path,
        r#"{ "objects": { "m": { "type": "molecule", "file": "bad.sdf" } } }"#,
    )
    .unwrap();
    let run = sci_anim(&["validate", scene_path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    assert_eq!(
        scene_errors(&run),
        common::pairs(&[("/objects/m/file", "bad_structure_file")])
    );
    run.json["errors"][0]["message"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn a_page_curl_saved_in_place_of_a_structure_file_is_named_for_what_it_is() {
    let insulin = std::fs::read_to_string(format!("{FIXTURES}/../bad/insulin-404.sdf")).unwrap();

    let message = bad_file_message(&insulin);

    assert_eq!(
        message,
        "`bad.sdf` is not a structure file sci_anim can draw: Missing required section: counts line; its first line is \"Status: 404\""
    );
}

#[test]
fn a_structure_file_that_sci_anim_cannot_draw_says_which_atom_or_bond() {
    let ethanol = std::fs::read_to_string(format!("{FIXTURES}/ethanol.sdf")).unwrap();
    let caffeine = std::fs::read_to_string(format!("{FIXTURES}/caffeine.sdf")).unwrap();
    let benzene = std::fs::read_to_string(format!("{FIXTURES}/benzene.sdf")).unwrap();

    assert!(bad_file_message(&ethanol.replacen(" O   0", " Xx  0", 1)).contains("atom 1 has the element symbol \"Xx\", which is not an element sci_anim draws; it draws elements 1 (H) to 109 (Mt)"));
    assert!(
        bad_file_message(&ethanol.replacen(" O   0", " Ds  0", 1)).contains(
            "atom 1 has the element symbol \"Ds\", which is not an element sci_anim draws"
        )
    );
    assert!(
        bad_file_message(&caffeine.replacen("  1  9  2  0", "  1  9  4  0", 1))
            .contains("the bond from atom 1 to atom 9 has the SDF bond code 4")
    );
    assert!(
        bad_file_message(&caffeine.replacen("  1  9  2  0", "  1 99  2  0", 1))
            .contains("Invalid atom index 99 in bond (molecule has 24 atoms)")
    );
    assert!(
        bad_file_message(&caffeine.replacen("  1  9  2  0", "  9  9  2  0", 1))
            .contains("a bond joins atom 9 to itself")
    );
    assert!(bad_file_message(&format!("{ethanol}{benzene}")).contains("it holds 2 molecules"));
    assert!(bad_file_message("").contains("it has no molecule"));
}

#[test]
fn a_molecule_fades_moves_scales_and_rotates_about_its_centroid() {
    let steps = r##"{ "type": "play", "duration": 2, "animations": [
        { "type": "fade", "object": "a", "to": 0.5, "easing_curve": "linear" },
        { "type": "move", "object": "a", "to": [1, 2, 0], "easing_curve": "linear" },
        { "type": "scale", "object": "a", "to": [2, 2, 2], "easing_curve": "linear" },
        { "type": "rotate", "object": "a", "to": [0, 0, 180], "easing_curve": "linear" }
    ] }"##;
    let at = |t: &str| inspect_at(&scene(&molecule("a", "acetylene", ""), steps), t);

    let (start, end) = (at("0"), at("2"));

    let x = |inspected: &Value, i: usize| {
        inspected["objects"]["a"]["atoms"][i]["position"][0]
            .as_f64()
            .unwrap()
    };
    assert!(
        (x(&start, 0) + 0.6).abs() < 1e-3 && (x(&start, 1) - 0.6).abs() < 1e-3,
        "the two carbons are 0.6 either side of the centroid"
    );
    assert!(
        (x(&end, 0) - (1.0 + 1.2)).abs() < 1e-3 && (x(&end, 1) - (1.0 - 1.2)).abs() < 1e-3,
        "scaled by 2 and turned half a turn about z, then at 1"
    );
    assert_eq!(
        (
            end["objects"]["a"]["opacity"].clone(),
            end["objects"]["a"]["created"].clone()
        ),
        (0.5.into(), 1.0.into())
    );
}

#[test]
fn caffeine_turns_about_its_centroid() {
    let steps = r##"{ "type": "play", "duration": 2, "animations": [
        { "type": "rotate", "object": "caf", "to": [0, 0, 180], "easing_curve": "linear" },
        { "type": "move", "object": "caf", "to": [1, 0, 0], "easing_curve": "linear" }
    ] }"##;

    let inspected = inspect_at(&scene(&molecule("caf", "caffeine", ""), steps), "2");

    let position: Vec<f64> = atoms(&inspected, "caf")[0]["position"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (c.as_f64().unwrap() * 1000.0).round() / 1000.0)
        .collect();
    assert_eq!(
        position,
        [0.594, -2.526, 0.0],
        "atom 1 at (0.406, 2.526) about the centroid, turned half a turn, then moved 1 along x"
    );
}

#[test]
fn a_molecule_cannot_be_created_or_written() {
    let steps = r##"{ "type": "play", "duration": 1, "animations": [
        { "type": "create", "object": "a" }, { "type": "write", "object": "a" } ] }"##;
    let (_dir, path) = scene_file(&scene(&molecule("a", "acetylene", ""), steps));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        common::pairs(&[
            ("/timeline/0/animations/0/object", "not_flat"),
            ("/timeline/0/animations/1/object", "not_text")
        ])
    );
    assert!(
        run.json["errors"][0]["message"]
            .as_str()
            .unwrap()
            .contains("`a` is a molecule; fade it in instead")
    );
}

#[test]
fn space_filling_draws_each_atom_at_its_van_der_waals_radius_and_no_bond() {
    let drawn = |representation: &str| {
        scene(
            &molecule(
                "a",
                "acetylene",
                &format!(r#", "representation": "{representation}""#),
            ),
            "",
        )
    };
    let radii = |inspected: &Value| -> Vec<f64> {
        atoms(inspected, "a")
            .iter()
            .map(|atom| (atom["ball_radius"].as_f64().unwrap() * 100.0).round() / 100.0)
            .collect()
    };

    let filled = inspect_at(&drawn("space-filling"), "0");
    let sticks = inspect_at(&drawn("ball-and-stick"), "0");

    assert_eq!(filled["objects"]["a"]["representation"], "space-filling");
    assert_eq!(
        radii(&filled),
        [82.08, 82.08, 57.94, 57.94],
        "carbon 1.7 and hydrogen 1.2 angstroms at 48.28 pixels a unit"
    );
    assert_eq!(radii(&sticks), [20.52, 20.52, 14.49, 14.49]);
    assert_eq!(
        hidden(filled["objects"]["a"]["bonds"].as_array().unwrap()),
        3,
        "space-filling draws no bond"
    );
    assert_eq!(hidden(atoms(&filled, "a")), 0);

    let (_a, filled_frame) = frame_at(&drawn("space-filling"), "0");
    let (_b, sticks_frame) = frame_at(&drawn("ball-and-stick"), "0");
    let above_the_triple_bond = (200, 128);
    assert!(
        within(
            pixel(
                &filled_frame,
                above_the_triple_bond.0,
                above_the_triple_bond.1
            ),
            [0x90, 0x90, 0x90],
            12
        ),
        "1.5 angstroms above the bond lies inside both carbons' balls: {:?}",
        pixel(
            &filled_frame,
            above_the_triple_bond.0,
            above_the_triple_bond.1
        )
    );
    assert_eq!(
        pixel(
            &sticks_frame,
            above_the_triple_bond.0,
            above_the_triple_bond.1
        ),
        [0, 0, 0, 255],
        "and outside every ball and stick"
    );
}

#[test]
fn an_unknown_representation_is_a_scene_error_at_its_field() {
    let (_dir, path) = scene_file(&scene(
        &molecule("a", "acetylene", r#", "representation": "wireframe""#),
        "",
    ));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        common::pairs(&[("/objects/a/representation", "unknown_value")])
    );
}

#[test]
fn the_printed_schema_says_how_to_fetch_a_structure_file_and_refuses_an_empty_file() {
    let schema = sci_anim(&["schema"]);
    let described = schema.json.to_string();
    assert!(
        described.contains(
            "https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/caffeine/SDF?record_type=3d"
        ),
        "{described}"
    );

    let (_dir, path) = scene_file(r#"{ "objects": { "a": { "type": "molecule", "file": "" } } }"#);
    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(
        scene_errors(&run),
        common::pairs(&[("/objects/a/file", "invalid")])
    );
}

#[test]
fn the_molecules_example_validates_and_shows_caffeine_whole_and_benzene_without_hydrogens() {
    let example = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/molecules.json");

    let validated = sci_anim(&["validate", example]);
    let halfway = sci_anim(&["inspect", example, "--scene-time", "3"]);

    assert_eq!(validated.exit_code, 0, "{}", validated.stdout);
    assert_eq!(
        halfway.json["objects"]["caffeine"]["rotation"],
        serde_json::json!([0.0, 180.0, 0.0])
    );
    assert_eq!(
        (
            atoms(&halfway.json, "caffeine").len(),
            hidden(atoms(&halfway.json, "caffeine"))
        ),
        (24, 0)
    );
    assert_eq!(
        (
            atoms(&halfway.json, "benzene").len(),
            hidden(atoms(&halfway.json, "benzene"))
        ),
        (12, 6)
    );
}
