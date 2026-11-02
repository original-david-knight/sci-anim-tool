//! Skeletal formulas, driven as an agent drives them: L-alanine's 2D record read into corners,
//! atom symbols, a hash and a double bond, as `inspect` reports them; its mirror images; caffeine's
//! ring double bonds; a wedge and a hash against a golden frame; a write; a highlight of a group;
//! and the scene errors of a file a skeletal formula cannot draw.
//!
//! Scenes are 400 by 400 pixels on black with the default camera, so a world point (x, y) in the
//! plane z = 0 lands at pixel (200 + 48.28 x, 200 - 48.28 y).

mod common;

use common::{
    frame_at, inspect_at, near, pixel, read_rgba, scene, scene_errors, scene_file, sci_anim,
};
use serde_json::Value;

const ALANINE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/skills/sci-anim/examples/structures/l_alanine_2d.sdf"
);
const PUBCHEM: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pubchem");
const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/skeletal_formula"
);

fn alanine(more: &str) -> String {
    format!(r#""f": {{ "type": "skeletal-formula", "file": "{ALANINE}"{more} }}"#)
}

fn formula(inspected: &Value) -> &Value {
    &inspected["objects"]["f"]["skeletal_formula"]
}

/// Each atom's number, how it is drawn and its atom symbol, as `inspect` reports them.
fn drawn_atoms(inspected: &Value) -> Vec<(u64, String, Option<String>)> {
    formula(inspected)["atoms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|atom| {
            (
                atom["atom"].as_u64().unwrap(),
                atom["drawn"].as_str().unwrap().to_string(),
                atom["symbol"].as_str().map(str::to_string),
            )
        })
        .collect()
}

/// Each bond's two atoms, its form and its strokes, as `inspect` reports them.
fn drawn_bonds(inspected: &Value) -> Vec<([u64; 2], String, u64)> {
    formula(inspected)["bonds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|bond| {
            (
                [
                    bond["atoms"][0].as_u64().unwrap(),
                    bond["atoms"][1].as_u64().unwrap(),
                ],
                bond["form"].as_str().unwrap().to_string(),
                bond["strokes"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn positions(inspected: &Value) -> Vec<[f64; 2]> {
    formula(inspected)["atoms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|atom| {
            let p = &atom["position"];
            [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]
        })
        .collect()
}

fn symbol_of(inspected: &Value, atom: usize) -> String {
    formula(inspected)["atoms"][atom - 1]["symbol"]
        .as_str()
        .unwrap()
        .to_string()
}

fn form_of(inspected: &Value, atoms: [u64; 2]) -> String {
    drawn_bonds(inspected)
        .into_iter()
        .find(|(a, _, _)| *a == atoms)
        .map(|(_, form, _)| form)
        .unwrap()
}

#[test]
fn l_alanine_draws_three_corners_three_atom_symbols_a_hash_and_a_double_bond() {
    let inspected = inspect_at(&scene(&alanine(""), ""), "0");

    let symbol = |s: &str| Some(s.to_string());
    let mut expected = vec![
        (1, "symbol".to_string(), symbol("OH")),
        (2, "symbol".to_string(), symbol("O")),
        (3, "symbol".to_string(), symbol("H₂N")),
        (4, "corner".to_string(), None),
        (5, "corner".to_string(), None),
        (6, "corner".to_string(), None),
    ];
    expected.extend((7..=13).map(|n| (n, "hidden".to_string(), None)));
    assert_eq!(drawn_atoms(&inspected), expected);

    let shown: Vec<([u64; 2], String, u64)> = drawn_bonds(&inspected)
        .into_iter()
        .filter(|(_, form, _)| form != "hidden")
        .collect();
    let (hash, rest): (Vec<_>, Vec<_>) = shown.into_iter().partition(|(_, f, _)| f == "hash");
    assert_eq!(
        rest,
        vec![
            ([1, 6], "line".to_string(), 1),
            ([2, 6], "double-centered".to_string(), 2),
            ([4, 5], "line".to_string(), 1),
            ([4, 6], "line".to_string(), 1),
        ]
    );
    assert_eq!(hash.len(), 1);
    let ([from, to], _, marks) = hash[0];
    assert_eq!(
        [from, to],
        [4, 3],
        "the hash's narrow end is at the stereocenter"
    );
    assert!(marks >= 5, "{marks} hash marks");
    let bonds = formula(&inspected)["bonds"].as_array().unwrap();
    assert_eq!(bonds.iter().filter(|b| b["form"] == "hidden").count(), 7);
    assert_eq!(bonds[3]["stereo"], "hash");
}

#[test]
fn bonds_between_atoms_other_than_hydrogen_are_drawn_bond_length_long() {
    let inspected = inspect_at(&scene(&alanine(r#", "bond_length": 0.75"#), ""), "0");
    let p = positions(&inspected);
    let length = |a: usize, b: usize| {
        let (pa, pb) = (p[a - 1], p[b - 1]);
        (pa[0] - pb[0]).hypot(pa[1] - pb[1])
    };
    for (a, b) in [(4, 5), (4, 6), (1, 6), (2, 6), (3, 4)] {
        assert!(
            (length(a, b) - 0.75).abs() < 1e-3,
            "bond {a}-{b} is {}",
            length(a, b)
        );
    }
    assert_eq!(formula(&inspected)["bond_length"], 0.75);
    assert!((formula(&inspected)["size"].as_f64().unwrap() - 0.45).abs() < 1e-6);
}

#[test]
fn every_bond_stops_short_of_an_atom_symbol() {
    let (_dir, rgba) = frame_at(&scene(&alanine(""), ""), "0");
    let inspected = inspect_at(&scene(&alanine(""), ""), "0");
    let atoms = formula(&inspected)["atoms"].as_array().unwrap();
    let n = &atoms[2];
    let (px, py) = (
        n["position_pixels"][0].as_f64().unwrap(),
        n["position_pixels"][1].as_f64().unwrap(),
    );
    let c = &atoms[3];
    let (cx, cy) = (
        c["position_pixels"][0].as_f64().unwrap(),
        c["position_pixels"][1].as_f64().unwrap(),
    );
    let ink_max_x = n["bounding_box"]["pixels"]["max"][0].as_f64().unwrap();
    let lit = |x: f64, y: f64| {
        let p = pixel(&rgba, x.round() as usize, y.round() as usize);
        p[0] > 60 || p[1] > 60 || p[2] > 60
    };
    let gap = (0..=20)
        .map(|i| f64::from(i) / 20.0)
        .map(|t| (px + (cx - px) * t, py + (cy - py) * t))
        .filter(|&(x, _)| x > ink_max_x + 0.5)
        .take_while(|&(x, y)| !lit(x, y))
        .count();
    assert!(
        gap >= 1,
        "the line from N to its carbon leaves a gap after the letters' box"
    );
}

#[test]
fn mirror_x_and_y_flip_the_drawing_and_mirror_z_swaps_the_hash_for_a_wedge() {
    let plain = inspect_at(&scene(&alanine(""), ""), "0");
    let flipped = |plane: &str| {
        inspect_at(
            &scene(&alanine(&format!(r#", "mirror": "{plane}""#)), ""),
            "0",
        )
    };
    let (x, y, z) = (flipped("x"), flipped("y"), flipped("z"));
    let close = |a: f64, b: f64| (a - b).abs() < 1e-4;
    for (k, p) in positions(&plain).into_iter().enumerate() {
        let (px, py, pz) = (positions(&x)[k], positions(&y)[k], positions(&z)[k]);
        assert!(
            close(px[0], -p[0]) && close(px[1], p[1]),
            "atom {} under x",
            k + 1
        );
        assert!(
            close(py[0], p[0]) && close(py[1], -p[1]),
            "atom {} under y",
            k + 1
        );
        assert!(
            close(pz[0], p[0]) && close(pz[1], p[1]),
            "atom {} under z",
            k + 1
        );
    }
    assert_eq!(form_of(&plain, [4, 3]), "hash");
    assert_eq!(form_of(&x, [4, 3]), "hash");
    assert_eq!(form_of(&y, [4, 3]), "hash");
    assert_eq!(form_of(&z, [4, 3]), "wedge");
    assert_eq!(formula(&z)["bonds"][3]["stereo"], "wedge");
    assert_eq!(
        (symbol_of(&plain, 3), symbol_of(&x, 3)),
        ("H₂N".to_string(), "NH₂".to_string()),
        "the hydrogens stay on the side away from the bonds, and the letters are not reversed"
    );
    assert_eq!(
        (symbol_of(&plain, 1), symbol_of(&x, 1)),
        ("OH".to_string(), "HO".to_string())
    );
}

#[test]
fn caffeine_draws_its_ring_double_bonds_inside_the_rings_and_its_carbonyls_centered() {
    let objects =
        format!(r#""f": {{ "type": "skeletal-formula", "file": "{PUBCHEM}/caffeine-2d.sdf" }}"#);
    let inspected = inspect_at(&scene(&objects, ""), "0");
    let forms: Vec<String> = drawn_bonds(&inspected)
        .into_iter()
        .map(|(_, form, _)| form)
        .collect();
    let count = |form: &str| forms.iter().filter(|f| *f == form).count();
    assert_eq!(count("double-in-ring"), 2, "{forms:?}");
    assert_eq!(count("double-centered"), 2, "{forms:?}");
    let symbols: Vec<(String, String)> = formula(&inspected)["atoms"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["drawn"] == "symbol")
        .map(|a| {
            (
                a["element"].as_str().unwrap().to_string(),
                a["symbol"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(
        symbols.iter().filter(|(e, s)| e == "N" && s == "N").count(),
        4,
        "caffeine's four nitrogens hold no hydrogen: {symbols:?}"
    );
    assert_eq!(symbols.iter().filter(|(e, _)| e == "O").count(), 2);
    assert!(
        formula(&inspected)["atoms"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["element"] == "H")
            .all(|a| a["drawn"] == "hidden")
    );
}

/// How many of the 400 by 400 pixels differ from `golden` by more than 48 in any channel.
fn pixels_off(actual: &[u8], golden: &[u8]) -> usize {
    actual
        .chunks(4)
        .zip(golden.chunks(4))
        .filter(|(a, g)| a.iter().zip(g.iter()).any(|(a, g)| a.abs_diff(*g) > 48))
        .count()
}

/// The golden frame is `wedge_and_hash.json` beside it, drawn by `sci_anim frame` and checked by
/// eye against a textbook's L-alanine and its reflection through the page. Two GPUs smooth edges a
/// little differently, so a few edge pixels may differ.
#[test]
fn a_wedge_and_a_hash_draw_as_the_golden_frame() {
    let png = tempfile::tempdir().unwrap();
    let out = png.path().join("f.png");
    let run = sci_anim(&[
        "frame",
        &format!("{GOLDEN}/wedge_and_hash.json"),
        "--scene-time",
        "0",
        "-o",
        out.to_str().unwrap(),
    ]);
    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    let (actual, golden) = (
        read_rgba(&out),
        read_rgba(std::path::Path::new(&format!(
            "{GOLDEN}/wedge_and_hash.png"
        ))),
    );
    assert_eq!(actual.len(), golden.len());
    let off = pixels_off(&actual, &golden);
    assert!(
        off < 200,
        "{off} of 160000 pixels differ from the golden frame"
    );
    let lit = golden.chunks(4).filter(|p| p[0] > 128).count();
    assert!(
        lit > 2000,
        "the golden frame shows the formulas: {lit} lit pixels"
    );
}

#[test]
fn a_write_draws_the_bonds_then_writes_the_atom_symbols() {
    let written = scene(
        &alanine(""),
        r#"{ "type": "wait", "duration": 1 },
           { "type": "play", "duration": 2, "animations": [{ "type": "write", "object": "f", "easing_curve": "linear" }] }"#,
    );
    let lit = |scene_time: &str| {
        let (_dir, rgba) = frame_at(&written, scene_time);
        rgba.chunks(4).filter(|p| p[0] > 128).count()
    };
    let shown: Vec<usize> = ["0.5", "1.3", "1.9", "2.5", "3"].map(lit).to_vec();
    assert_eq!(shown[0], 0, "nothing shows before the write starts");
    assert!(
        shown.windows(2).all(|w| w[0] < w[1]),
        "each frame shows more: {shown:?}"
    );
    let (_dir, whole) = frame_at(&scene(&alanine(""), ""), "0");
    assert_eq!(
        shown[4],
        whole.chunks(4).filter(|p| p[0] > 128).count(),
        "a written formula is the formula as declared"
    );
    let created = inspect_at(&written, "1.9")["objects"]["f"]["created"]
        .as_f64()
        .unwrap();
    assert!((created - 0.45).abs() < 1e-6, "{created}");
}

#[test]
fn a_highlight_lights_a_group_with_a_see_through_fill_over_its_atoms() {
    let lit = scene(
        &alanine(r#", "groups": { "stereocenter": [4, 7], "methyl_hydrogens": [8, 9, 10] }"#),
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "highlight", "object": "f", "group": "stereocenter", "to": 1 }] }"#,
    );
    let inspected = inspect_at(&lit, "1");
    let groups = &formula(&inspected)["groups"];
    assert_eq!(groups["stereocenter"]["atoms"], serde_json::json!([4, 7]));
    assert_eq!(groups["stereocenter"]["glow"], 1.0);
    assert_eq!(groups["stereocenter"]["color"], "#ffd23f");
    assert_eq!(groups["methyl_hydrogens"]["glow"], 0.0);
    let stereocenter = &formula(&inspected)["atoms"][3]["position_pixels"];
    let (x, y) = (
        stereocenter[0].as_f64().unwrap().round() as usize,
        stereocenter[1].as_f64().unwrap().round() as usize,
    );
    let (_dir, rgba) = frame_at(&lit, "1");
    assert!(
        near(pixel(&rgba, x, y - 9), [137, 112, 29, 255]),
        "above the stereocenter, between its bonds, the glow is #ffd23f at 0.25 in linear light: {:?}",
        pixel(&rgba, x, y - 9)
    );
    let (_dir, unlit) = frame_at(&lit, "0");
    assert_eq!(pixel(&unlit, x, y - 9), [0, 0, 0, 255]);
}

fn errors(objects: &str, timeline: &str) -> Vec<(String, String)> {
    let (_dir, path) = scene_file(&scene(objects, timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    scene_errors(&run)
}

fn first_message(objects: &str, timeline: &str) -> String {
    let (_dir, path) = scene_file(&scene(objects, timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    run.json["errors"][0]["message"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn a_group_of_left_out_hydrogens_and_a_create_are_scene_errors() {
    let hydrogens = alanine(r#", "groups": { "methyl_hydrogens": [8, 9, 10] }"#);
    let highlight = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "highlight", "object": "f", "group": "methyl_hydrogens", "to": 1 }] }"#;
    assert_eq!(
        errors(&hydrogens, highlight),
        common::pairs(&[("/timeline/0/animations/0/group", "group_hidden")])
    );
    let create =
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "create", "object": "f" }] }"#;
    assert_eq!(
        errors(&alanine(""), create),
        common::pairs(&[("/timeline/0/animations/0/object", "use_write")])
    );
    assert_eq!(
        errors(&alanine(r#", "groups": { "x": [14] }"#), ""),
        common::pairs(&[("/objects/f/groups/x/0", "no_such_atom")])
    );
}

#[test]
fn a_3d_structure_file_is_a_scene_error_at_its_file_that_says_how_to_fetch_the_2d_one() {
    let objects =
        format!(r#""f": {{ "type": "skeletal-formula", "file": "{PUBCHEM}/caffeine.sdf" }}"#);
    assert_eq!(
        errors(&objects, ""),
        common::pairs(&[("/objects/f/file", "bad_structure_file")])
    );
    let message = first_message(&objects, "");
    assert!(
        message.contains("it is a 3D structure file, whose atoms do not lie flat as a skeletal formula draws them; fetch the 2D one by adding `?record_type=2d` to the PubChem URL"),
        "{message}"
    );
}

#[test]
fn a_missing_file_is_a_scene_error_at_its_file_that_says_how_to_fetch_it_in_2d() {
    let objects = r#""f": { "type": "skeletal-formula", "file": "alanine.sdf" }"#;
    assert_eq!(
        errors(objects, ""),
        common::pairs(&[("/objects/f/file", "unreadable_file")])
    );
    let message = first_message(objects, "");
    assert!(
        message.ends_with("such as from PubChem with `curl -fo alanine.sdf \"https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/<name>/SDF?record_type=2d\"`"),
        "{message}"
    );
}

#[test]
fn a_pdb_file_is_a_scene_error_at_its_file() {
    let pdb = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/sci-anim/examples/structures/1bna.pdb"
    );
    let objects = format!(r#""f": {{ "type": "skeletal-formula", "file": "{pdb}" }}"#);
    assert_eq!(
        errors(&objects, ""),
        common::pairs(&[("/objects/f/file", "bad_structure_file")])
    );
    assert!(first_message(&objects, "").contains("it is a PDB file, which gives 3D positions"));
}
