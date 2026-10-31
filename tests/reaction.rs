//! Reactions between molecules read from PubChem structure files.
//!
//! The reaction is ethylene (CID 6325) and hydrogen chloride (CID 313) to chloroethane (CID 6337),
//! all PubChem 3D records in `tests/fixtures/pubchem`. Ethylene's C1=C2 becomes chloroethane's
//! C2-C3, C1 gets the chlorine, and hydrogen chloride's hydrogen goes to C2. Scenes are 400 by
//! 400 pixels on black with the default camera, so a world point (x, y) in the plane z = 0 lands
//! at pixel (200 + 48.28 x, 200 - 48.28 y).

mod common;

use common::{frame_at, inspect_at, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pubchem");

fn molecule(id: &str, sdf: &str, more: &str) -> String {
    format!(r##""{id}": {{ "type": "molecule", "file": "{FIXTURES}/{sdf}.sdf"{more} }}"##)
}

fn atom_map(pairs: &[(&str, u32, &str, u32)]) -> String {
    let rows: Vec<String> = pairs.iter().map(|(r, a, p, b)| format!(r#"{{ "reactant": {{ "object": "{r}", "atom": {a} }}, "product": {{ "object": "{p}", "atom": {b} }} }}"#)).collect();
    format!("[{}]", rows.join(", "))
}

const HYDROCHLORINATION: [(&str, u32, &str, u32); 8] = [
    ("ethylene", 1, "chloroethane", 2),
    ("ethylene", 2, "chloroethane", 3),
    ("ethylene", 3, "chloroethane", 4),
    ("ethylene", 4, "chloroethane", 5),
    ("ethylene", 5, "chloroethane", 6),
    ("ethylene", 6, "chloroethane", 7),
    ("hcl", 1, "chloroethane", 1),
    ("hcl", 2, "chloroethane", 8),
];

/// Objects for the reaction with the reactants at x = -2.6 and 2.6, and the product at the origin.
fn objects() -> String {
    [
        molecule("ethylene", "ethylene", r#", "position": [-2.6, 0, 0]"#),
        molecule("hcl", "hydrogen-chloride", r#", "position": [2.6, 0, 0]"#),
        molecule("chloroethane", "chloroethane", ""),
    ]
    .join(", ")
}

/// A linear reaction over scene times 1 to 4, so progress at scene time t is (t - 1) / 3.
fn reaction(map: &str) -> String {
    reaction_with(map, "")
}

/// `reaction`, with `more` animations after the reaction in its step.
fn reaction_with(map: &str, more: &str) -> String {
    format!(
        r#"{{ "type": "wait", "duration": 1 }},
           {{ "type": "play", "duration": 3, "animations": [ {{ "type": "reaction", "easing_curve": "linear",
              "reactants": ["ethylene", "hcl"], "products": ["chloroethane"], "atom_map": {map} }}{more} ] }},
           {{ "type": "wait", "duration": 1 }}"#
    )
}

fn hydrochlorination() -> String {
    scene(&objects(), &reaction(&atom_map(&HYDROCHLORINATION)))
}

fn atoms(inspected: &Value) -> &Vec<Value> {
    inspected["reactions"][0]["atoms"].as_array().unwrap()
}

fn position(atom: &Value) -> [f64; 3] {
    [0, 1, 2].map(|i| atom["position"][i].as_f64().unwrap())
}

fn close(actual: [f64; 3], expected: [f64; 3], tolerance: f64) -> bool {
    actual
        .iter()
        .zip(expected)
        .all(|(a, e)| (a - e).abs() <= tolerance)
}

#[test]
fn the_reaction_validates_and_calls_each_atom_by_reactant_object_and_number() {
    let (_dir, path) = scene_file(&hydrochlorination());
    assert_eq!(sci_anim(&["validate", path.to_str().unwrap()]).exit_code, 0);

    let inspected = inspect_at(&hydrochlorination(), "0");

    let reaction = &inspected["reactions"][0];
    assert_eq!(reaction["phase"], "before");
    assert_eq!(atoms(&inspected).len(), 8);
    let cl = &atoms(&inspected)[6];
    assert_eq!(
        (
            cl["reactant"].clone(),
            cl["product"].clone(),
            cl["element"].clone()
        ),
        (
            serde_json::json!({"object": "hcl", "atom": 1}),
            serde_json::json!({"object": "chloroethane", "atom": 1}),
            "Cl".into()
        )
    );
}

#[test]
fn before_the_step_the_reactants_show_and_after_it_the_product_does() {
    let opacities = |t: &str| {
        let inspected = inspect_at(&hydrochlorination(), t);
        ["ethylene", "hcl", "chloroethane"]
            .map(|id| inspected["objects"][id]["opacity"].as_f64().unwrap())
    };

    assert_eq!(
        opacities("0.99"),
        [1.0, 1.0, 0.0],
        "the product shows nothing before its reaction"
    );
    assert_eq!(
        opacities("1"),
        [0.0, 0.0, 0.0],
        "the blend draws from the step's first instant"
    );
    assert_eq!(opacities("2.5"), [0.0, 0.0, 0.0]);
    assert_eq!(
        opacities("4"),
        [0.0, 0.0, 1.0],
        "the product shows from the step's end"
    );
}

#[test]
fn each_atom_moves_in_a_straight_line_from_its_reactant_position_to_its_product_position() {
    let at = |t: &str| position(&atoms(&inspect_at(&hydrochlorination(), t))[6]);

    let (start, middle, end) = (at("1"), at("2.5"), at("4"));

    assert!(
        close(start, [2.2971, 0.1027, 0.5692], 1e-3),
        "the Cl of hydrogen chloride (0, 0, 0 in its file) less the centroid (0.3029, -0.1027, -0.5692), at 2.6 right of the middle: {start:?}"
    );
    assert!(
        close(middle, [0, 1, 2].map(|i| (start[i] + end[i]) / 2.0), 1e-4),
        "linear easing: halfway at half the step: {middle:?}"
    );
    let product = inspect_at(&hydrochlorination(), "4");
    assert!(
        close(
            end,
            position(&product["objects"]["chloroethane"]["atoms"][0]),
            1e-4
        ),
        "it ends where the product object's atom 1 is"
    );
}

#[test]
fn the_product_keeps_its_own_shape_so_its_carbon_chlorine_bond_is_1_777_angstroms() {
    let inspected = inspect_at(&hydrochlorination(), "4");

    let [cl, c] = [0, 1].map(|i| position(&inspected["objects"]["chloroethane"]["atoms"][i]));

    let distance =
        ((cl[0] - c[0]).powi(2) + (cl[1] - c[1]).powi(2) + (cl[2] - c[2]).powi(2)).sqrt();
    assert!(
        (distance - 1.7767).abs() < 1e-3,
        "{distance}: Cl (1.4165, 0.2652, 0) to C2 (-0.1260, -0.6165, 0) in the file"
    );
}

#[test]
fn each_bond_is_kept_broken_formed_or_changed_by_comparing_the_two_sides_through_the_atom_map() {
    let inspected = inspect_at(&hydrochlorination(), "0");
    let bonds = inspected["reactions"][0]["bonds"].as_array().unwrap();

    let count = |fate: &str| bonds.iter().filter(|b| b["fate"]["type"] == fate).count();

    assert_eq!(
        [
            count("kept"),
            count("changed"),
            count("broken"),
            count("formed")
        ],
        [4, 1, 1, 2]
    );
    assert_eq!(
        bonds[0]["atoms"],
        serde_json::json!([{"object": "ethylene", "atom": 1}, {"object": "ethylene", "atom": 2}])
    );
    assert_eq!(
        bonds[0]["fate"],
        serde_json::json!({"type": "changed", "from": "double", "to": "single"}),
        "the double bond becomes single"
    );
    assert_eq!(
        bonds[1]["fate"],
        serde_json::json!({"type": "kept", "order": "single"}),
        "a carbon-hydrogen bond stays single"
    );
}

#[test]
fn a_broken_bond_thins_over_the_first_half_and_a_formed_bond_thickens_over_the_second() {
    let weights = |t: &str| {
        let inspected = inspect_at(&hydrochlorination(), t);
        let bonds = inspected["reactions"][0]["bonds"]
            .as_array()
            .unwrap()
            .clone();
        let weight = |kind: &str| {
            bonds.iter().find(|b| b["fate"]["type"] == kind).unwrap()["weight"]
                .as_f64()
                .unwrap()
        };
        (weight("broken"), weight("formed"), weight("changed"))
    };

    assert_eq!(weights("0"), (1.0, 0.0, 1.0));
    assert_eq!(weights("1.75"), (0.5, 0.0, 1.0), "a quarter of the way");
    assert_eq!(
        weights("2.5"),
        (0.0, 0.0, 1.0),
        "half way: the bond is gone and the new ones not yet there"
    );
    assert_eq!(weights("3.25"), (0.0, 0.5, 1.0));
    assert_eq!(weights("5"), (0.0, 1.0, 1.0));
}

/// Chloroethane's record with every atom turned a quarter turn about z: (x, y) becomes (-y, x).
fn quarter_turned_chloroethane() -> String {
    let sdf = std::fs::read_to_string(format!("{FIXTURES}/chloroethane.sdf")).unwrap();
    sdf.lines()
        .enumerate()
        .map(|(i, line)| {
            if (4..12).contains(&i) {
                let (x, y): (f64, f64) = (
                    line[0..10].trim().parse().unwrap(),
                    line[10..20].trim().parse().unwrap(),
                );
                format!("{:10.4}{:10.4}{}", -y, x, &line[20..])
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_product_file_turned_a_quarter_turn_fits_back_to_the_reactant_with_an_rmsd_near_0() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("turned.sdf"), quarter_turned_chloroethane()).unwrap();
    let identity: Vec<(&str, u32, &str, u32)> =
        (1..=8).map(|n| ("before", n, "after", n)).collect();
    let scene = scene(
        &format!(
            r#"{}, "after": {{ "type": "molecule", "file": "{}/turned.sdf" }}"#,
            molecule("before", "chloroethane", ""),
            dir.path().display()
        ),
        &format!(
            r#"{{ "type": "play", "duration": 1, "animations": [ {{ "type": "reaction", "reactants": ["before"], "products": ["after"], "atom_map": {} }} ] }}"#,
            atom_map(&identity)
        ),
    );

    let start = inspect_at(&scene, "0");
    let end = inspect_at(&scene, "1");

    assert!(
        start["reactions"][0]["fit_rmsd"].as_f64().unwrap() < 1e-4,
        "{}",
        start["reactions"][0]["fit_rmsd"]
    );
    assert!(
        close(position(&atoms(&end)[0]), [2.0521, 0.3552, 0.0], 1e-3),
        "chlorine ends where it started, 2.0521 right of the centroid: {:?}",
        position(&atoms(&end)[0])
    );
    assert!(close(
        position(&atoms(&start)[0]),
        position(&atoms(&end)[0]),
        1e-3
    ));
    let rotation = &end["objects"]["after"]["rotation"];
    assert!(
        close(
            [0, 1, 2].map(|i| rotation[i].as_f64().unwrap()),
            [0.0, 0.0, -90.0],
            1e-2
        ),
        "the fit turns the product back a quarter turn: {rotation}"
    );
}

/// Chloroethane reacting into its quarter-turned copy with the fit off, the copy declared at
/// `rotation`.
fn unfitted_quarter_turn(rotation: &str) -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("turned.sdf"), quarter_turned_chloroethane()).unwrap();
    let identity: Vec<(&str, u32, &str, u32)> =
        (1..=8).map(|n| ("before", n, "after", n)).collect();
    let scene = scene(
        &format!(
            r#"{}, "after": {{ "type": "molecule", "file": "{}/turned.sdf", "rotation": {rotation} }}"#,
            molecule("before", "chloroethane", ""),
            dir.path().display()
        ),
        &format!(
            r#"{{ "type": "play", "duration": 1, "animations": [ {{ "type": "reaction", "fit": false, "reactants": ["before"], "products": ["after"], "atom_map": {} }} ] }}"#,
            atom_map(&identity)
        ),
    );
    (dir, scene)
}

#[test]
fn with_the_fit_off_each_product_ends_at_exactly_its_declared_rotation() {
    let (_dir, kept) = unfitted_quarter_turn("[0, 0, 0]");
    let (_dir2, undone) = unfitted_quarter_turn("[0, 0, -90]");

    let (kept_end, undone_end) = (inspect_at(&kept, "1"), inspect_at(&undone, "1"));

    assert_eq!(
        kept_end["objects"]["after"]["rotation"],
        serde_json::json!([0.0, 0.0, 0.0]),
        "no fit turns it back"
    );
    assert!(
        close(position(&atoms(&kept_end)[0]), [-0.3552, 2.0521, 0.0], 1e-3),
        "chlorine ends a quarter turn from where it started: {:?}",
        position(&atoms(&kept_end)[0])
    );
    let rmsd = kept_end["reactions"][0]["fit_rmsd"].as_f64().unwrap();
    assert!(
        (rmsd - 1.8980).abs() < 1e-3,
        "how far the quarter-turned product lies from the reactant: {rmsd}"
    );
    assert_eq!(
        undone_end["objects"]["after"]["rotation"],
        serde_json::json!([0.0, 0.0, -90.0])
    );
    assert!(
        close(
            position(&atoms(&undone_end)[0]),
            [2.0521, 0.3552, 0.0],
            1e-3
        ),
        "its own rotation undoes the quarter turn: {:?}",
        position(&atoms(&undone_end)[0])
    );
    assert!(undone_end["reactions"][0]["fit_rmsd"].as_f64().unwrap() < 1e-4);
}

#[test]
fn an_atom_map_with_three_mistakes_reports_all_three_and_the_atoms_they_leave_unmapped() {
    let mut wrong = HYDROCHLORINATION.to_vec();
    wrong[0].3 = 1; // C -> Cl
    wrong[1].3 = 99; // no atom 99
    wrong[3].1 = 3; // ethylene atom 3 mapped twice
    wrong.pop(); // hcl atom 2 left out
    let invalid = scene(&objects(), &reaction(&atom_map(&wrong)));
    let (_dir, path) = scene_file(&invalid);

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    let mut expected = pairs(&[
        (
            "/timeline/1/animations/0/atom_map/0/product",
            "element_changed",
        ),
        (
            "/timeline/1/animations/0/atom_map/1/product/atom",
            "no_such_atom",
        ),
        (
            "/timeline/1/animations/0/atom_map/3/reactant",
            "atom_mapped_twice",
        ),
        ("/timeline/1/animations/0/atom_map", "unmapped_atom"),
        ("/timeline/1/animations/0/atom_map", "unmapped_atom"),
        ("/timeline/1/animations/0/atom_map", "unmapped_atom"),
    ]);
    expected.sort();
    let mut actual = scene_errors(&run);
    actual.sort();
    assert_eq!(actual, expected);
}

#[test]
fn a_reaction_that_loses_an_atom_says_which_atoms_each_side_holds() {
    let missing_hcl = scene(
        &objects(),
        &reaction(&atom_map(&HYDROCHLORINATION[..6]))
            .replace(r#"["ethylene", "hcl"]"#, r#"["ethylene"]"#),
    );
    let (_dir, path) = scene_file(&missing_hcl);

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1);
    let message = run.json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["code"] == "atoms_not_conserved")
        .unwrap()["message"]
        .as_str()
        .unwrap();
    assert!(
        message.starts_with("the reactants hold C2 H4 and the products C2 H5 Cl"),
        "{message}"
    );
}

#[test]
fn another_animation_of_a_reactant_in_the_reaction_step_is_a_conflict() {
    let move_ethylene = r#", { "type": "move", "object": "ethylene", "to": [0, 1, 0] }"#;
    let scene = scene(
        &objects(),
        &reaction_with(&atom_map(&HYDROCHLORINATION), move_ethylene),
    );
    let (_dir, path) = scene_file(&scene);

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/1/animations/1", "conflicting_animations")])
    );
}

#[test]
fn mid_reaction_the_chlorine_is_drawn_green_where_inspect_says_it_is() {
    let scene = hydrochlorination();
    let inspected = inspect_at(&scene, "2.5");
    let (_dir, rgba) = frame_at(&scene, "2.5");

    let cl = &atoms(&inspected)[6];
    let pixels = (
        cl["pixels"][0].as_f64().unwrap().round() as usize,
        cl["pixels"][1].as_f64().unwrap().round() as usize,
    );

    let seen = pixel(&rgba, pixels.0, pixels.1);
    // Chlorine green #1ff01f at two thirds, as the key light shades a ball's middle.
    assert!(
        seen[..3]
            .iter()
            .zip([0x18, 0xc9, 0x18])
            .all(|(a, e): (&u8, u8)| a.abs_diff(e) <= 12),
        "{seen:?}"
    );
}

#[test]
fn a_product_can_be_the_reactant_of_a_later_reaction_and_starts_where_the_first_left_it() {
    let back: Vec<(&str, u32, &str, u32)> = HYDROCHLORINATION
        .iter()
        .map(|&(r, a, _, b)| {
            (
                "chloroethane",
                b,
                if r == "hcl" {
                    "hcl_again"
                } else {
                    "ethylene_again"
                },
                a,
            )
        })
        .collect();
    let objects = format!(
        "{}, {}, {}",
        objects(),
        molecule(
            "ethylene_again",
            "ethylene",
            r#", "position": [-2.6, -2, 0]"#
        ),
        molecule(
            "hcl_again",
            "hydrogen-chloride",
            r#", "position": [2.6, -2, 0]"#
        )
    );
    let elimination = format!(
        r#", {{ "type": "play", "duration": 2, "animations": [ {{ "type": "reaction", "reactants": ["chloroethane"], "products": ["ethylene_again", "hcl_again"], "atom_map": {} }} ] }}"#,
        atom_map(&back)
    );
    let scene = scene(
        &objects,
        &(reaction(&atom_map(&HYDROCHLORINATION)) + &elimination),
    );
    let opacities = |t: &str| {
        let inspected = inspect_at(&scene, t);
        ["chloroethane", "ethylene_again"]
            .map(|id| inspected["objects"][id]["opacity"].as_f64().unwrap())
    };

    assert_eq!(
        opacities("4.5"),
        [1.0, 0.0],
        "between the reactions the product of the first shows"
    );
    assert_eq!(
        opacities("6"),
        [0.0, 0.0],
        "the second reaction's blend draws"
    );
    assert_eq!(
        opacities("7"),
        [0.0, 1.0],
        "the second reaction's product shows from its end"
    );
    let between = inspect_at(&scene, "4.5");
    let second = inspect_at(&scene, "5");
    assert_eq!(second["reactions"][1]["phase"], "during");
    let cl_first = position(&between["objects"]["chloroethane"]["atoms"][0]);
    let cl_second = position(&second["reactions"][1]["atoms"][0]);
    assert!(
        close(cl_first, cl_second, 1e-4),
        "the chlorine starts the second reaction where the first put it: {cl_first:?} and {cl_second:?}"
    );
}

#[test]
fn a_faded_reactant_and_a_stretched_product_are_both_reported_not_ready() {
    let objects = [
        molecule("ethylene", "ethylene", r#", "position": [-2.6, 0, 0]"#),
        molecule(
            "hcl",
            "hydrogen-chloride",
            r#", "position": [2.6, 0, 0], "opacity": 0.5"#,
        ),
        molecule("chloroethane", "chloroethane", r#", "scale": [1, 2, 1]"#),
    ]
    .join(", ");
    let (_dir, path) = scene_file(&scene(&objects, &reaction(&atom_map(&HYDROCHLORINATION))));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/1/animations/0/reactants/1", "not_ready"),
            ("/timeline/1/animations/0/products/0", "not_ready")
        ])
    );
}

#[test]
fn reacting_the_same_objects_twice_reports_each_used_up_reactant_and_made_product() {
    let again = r#", { "type": "play", "duration": 1, "animations": [ { "type": "reaction", "reactants": ["ethylene", "hcl"], "products": ["chloroethane"], "atom_map": MAP } ] }"#.replace("MAP", &atom_map(&HYDROCHLORINATION));
    let (_dir, path) = scene_file(&scene(
        &objects(),
        &(reaction(&atom_map(&HYDROCHLORINATION)) + &again),
    ));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/3/animations/0/reactants/0", "reaction_order"),
            ("/timeline/3/animations/0/reactants/1", "reaction_order"),
            ("/timeline/3/animations/0/products/0", "reaction_order")
        ])
    );
}

#[test]
fn making_a_product_that_an_earlier_reaction_used_up_is_out_of_order() {
    let back: Vec<(&str, u32, &str, u32)> = HYDROCHLORINATION
        .iter()
        .map(|&(r, a, _, b)| {
            (
                "chloroethane",
                b,
                if r == "hcl" {
                    "hcl_again"
                } else {
                    "ethylene_again"
                },
                a,
            )
        })
        .collect();
    let objects = format!(
        "{}, {}, {}",
        objects(),
        molecule("ethylene_again", "ethylene", ""),
        molecule("hcl_again", "hydrogen-chloride", "")
    );
    let elimination = format!(
        r#"{{ "type": "play", "duration": 1, "animations": [ {{ "type": "reaction", "reactants": ["chloroethane"], "products": ["ethylene_again", "hcl_again"], "atom_map": {} }} ] }}, "#,
        atom_map(&back)
    );
    let (_dir, path) = scene_file(&scene(
        &objects,
        &(elimination + &reaction(&atom_map(&HYDROCHLORINATION))),
    ));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/2/animations/0/products/0", "reaction_order")])
    );
}

#[test]
fn the_examples_validate_and_the_hydration_fits_as_the_right_map_does() {
    let example = |name: &str| format!("{}/examples/{name}.json", env!("CARGO_MANIFEST_DIR"));

    let reaction = sci_anim(&["validate", &example("reaction")]);
    let hydration = sci_anim(&["inspect", &example("hydration"), "--scene-time", "2"]);

    assert_eq!(reaction.exit_code, 0, "{}", reaction.stdout);
    assert_eq!(hydration.exit_code, 0, "{}", hydration.stdout);
    let fit_rmsd = hydration.json["reactions"][0]["fit_rmsd"].as_f64().unwrap();
    assert!((fit_rmsd - 1.3697).abs() < 1e-3, "{fit_rmsd}");
    assert_eq!(hydration.json["reactions"][0]["phase"], "during");
}

/// The reaction of `reaction`, after a first step of `first` animations, with a group on ethylene's
/// double bond and on chloroethane's chlorine, each with a caption.
fn captioned(first: &str) -> String {
    let objects = [
        molecule("ethylene", "ethylene", r#", "position": [-2.6, 0, 0], "groups": { "double": [1, 2] }"#),
        molecule("hcl", "hydrogen-chloride", r#", "position": [2.6, 0, 0]"#),
        molecule("chloroethane", "chloroethane", r#", "groups": { "chlorine": [1] }"#),
        r#""alkene": { "type": "text", "text": "C=C", "size": 0.4, "attach": { "object": "ethylene", "group": "double", "offset": [0, 1] } }"#.to_string(),
        r#""halide": { "type": "text", "text": "Cl", "size": 0.4, "attach": { "object": "chloroethane", "group": "chlorine", "offset": [0, 1] } }"#.to_string(),
    ]
    .join(", ");
    let steps = reaction(&atom_map(&HYDROCHLORINATION)).replacen(
        r#"{ "type": "wait", "duration": 1 }"#,
        &format!(r#"{{ "type": "play", "duration": 1, "animations": [ {first} ] }}"#),
        1,
    );
    scene(&objects, &steps)
}

#[test]
fn a_caption_shows_exactly_while_its_molecule_exists() {
    let source = captioned(r#"{ "type": "fade", "object": "hcl", "to": 1 }"#);
    let opacities: Vec<(f64, f64)> = ["0.5", "2", "4.5"]
        .iter()
        .map(|t| {
            let inspected = inspect_at(&source, t);
            (
                inspected["objects"]["alkene"]["opacity"].as_f64().unwrap(),
                inspected["objects"]["halide"]["opacity"].as_f64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        opacities,
        [(1.0, 0.0), (0.0, 0.0), (0.0, 1.0)],
        "the reactant's caption until the reaction, the product's from its end"
    );
}

/// The hydrochlorination with ethylene's carbons in the group `double`, lit to 1 in `#00ffff`
/// over the first second, and `product_groups` on chloroethane, whose atoms 2 and 3 those carbons
/// become. `more` animations play in the reaction's step.
fn lit_double(product_groups: &str, more: &str) -> String {
    let objects = [
        molecule(
            "ethylene",
            "ethylene",
            r#", "position": [-2.6, 0, 0], "groups": { "double": [1, 2] }"#,
        ),
        molecule("hcl", "hydrogen-chloride", r#", "position": [2.6, 0, 0]"#),
        molecule(
            "chloroethane",
            "chloroethane",
            &format!(r#", "groups": {{ {product_groups} }}"#),
        ),
    ]
    .join(", ");
    let light = r##"{ "type": "highlight", "object": "ethylene", "group": "double", "to": 1, "color": "#00ffff", "easing_curve": "linear" }"##;
    let steps = reaction_with(&atom_map(&HYDROCHLORINATION), more).replacen(
        r#"{ "type": "wait", "duration": 1 }"#,
        &format!(r#"{{ "type": "play", "duration": 1, "animations": [ {light} ] }}"#),
        1,
    );
    scene(&objects, &steps)
}

#[test]
fn a_lit_reactant_group_needs_a_product_group_of_exactly_the_atoms_it_becomes() {
    let (_dir, path) = scene_file(&lit_double(r#""chlorine": [1]"#, ""));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/1/animations/0/products/0", "unknown_group")])
    );
    assert_eq!(
        run.json["errors"][0]["message"],
        "group `double` of `ethylene` glows when this reaction begins, and its atoms become atoms 2, 3 of `chloroethane`, which has no group of exactly those atoms; add one to the `groups` of `chloroethane`, such as `\"double\": [2, 3]`, or highlight `double` to 0 in an earlier step"
    );

    let (_dir, path) = scene_file(&lit_double(r#""carbons": [2, 3]"#, ""));
    assert_eq!(sci_anim(&["validate", path.to_str().unwrap()]).exit_code, 0);
}

#[test]
fn the_reaction_carries_a_lit_group_through_its_step_and_the_product_group_glows_from_its_end() {
    let source = lit_double(r#""carbons": [2, 3], "chlorine": [1]"#, "");
    let glows = |t: &str| {
        let inspected = inspect_at(&source, t);
        let groups = &inspected["objects"]["chloroethane"]["groups"];
        (
            inspected["reactions"][0]["glows"].clone(),
            (
                groups["carbons"]["glow"].as_f64().unwrap(),
                groups["carbons"]["color"].as_str().unwrap().to_string(),
            ),
            groups["chlorine"]["glow"].as_f64().unwrap(),
        )
    };

    let (during, _, _) = glows("2.5");
    let (_, after, chlorine) = glows("4");

    assert_eq!(
        during,
        serde_json::json!([{ "object": "ethylene", "group": "double", "glow": 1.0, "color": "#00ffff", "passes_to": [{ "object": "chloroethane", "group": "carbons" }] }])
    );
    assert_eq!(
        (after, chlorine),
        ((1.0, "#00ffff".to_string()), 0.0),
        "the carbons glow from the step's end, and the chlorine, which no lit group becomes, does not"
    );
}

#[test]
fn a_later_highlight_of_the_product_group_starts_from_the_glow_the_reaction_passed() {
    let fade = r#"{ "type": "play", "duration": 2, "animations": [ { "type": "highlight", "object": "chloroethane", "group": "carbons", "to": 0, "easing_curve": "linear" } ] }"#;
    let source = lit_double(r#""carbons": [2, 3]"#, "").replacen(
        r#"{ "type": "wait", "duration": 1 }"#,
        fade,
        1,
    );

    let carbons = inspect_at(&source, "5")["objects"]["chloroethane"]["groups"]["carbons"].clone();

    assert_eq!(
        (carbons["glow"].as_f64(), carbons["color"].as_str()),
        (Some(0.5), Some("#00ffff")),
        "halfway from 1 to 0, in the passed color"
    );
}

#[test]
fn a_lit_group_split_across_two_products_needs_a_group_in_each() {
    let back: Vec<(&str, u32, &str, u32)> = HYDROCHLORINATION
        .iter()
        .map(|&(r, a, p, b)| (p, b, r, a))
        .collect();
    let elimination = |ethylene_groups: &str, hcl_groups: &str| {
        let objects = [
            molecule(
                "chloroethane",
                "chloroethane",
                r#", "groups": { "c_cl": [1, 2] }"#,
            ),
            molecule(
                "ethylene",
                "ethylene",
                &format!(r#", "position": [-2.6, 0, 0], "groups": {{ {ethylene_groups} }}"#),
            ),
            molecule(
                "hcl",
                "hydrogen-chloride",
                &format!(r#", "position": [2.6, 0, 0], "groups": {{ {hcl_groups} }}"#),
            ),
        ]
        .join(", ");
        let steps = format!(
            r#"{{ "type": "play", "duration": 1, "animations": [ {{ "type": "highlight", "object": "chloroethane", "group": "c_cl", "to": 1 }} ] }},
               {{ "type": "play", "duration": 2, "animations": [ {{ "type": "reaction", "reactants": ["chloroethane"], "products": ["ethylene", "hcl"], "atom_map": {} }} ] }}"#,
            atom_map(&back)
        );
        scene(&objects, &steps)
    };

    let (_dir, path) = scene_file(&elimination("", ""));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/1/animations/0/products/0", "unknown_group"),
            ("/timeline/1/animations/0/products/1", "unknown_group")
        ])
    );

    let passes_to = inspect_at(&elimination(r#""carbon": [1]"#, r#""chlorine": [1]"#), "2")["reactions"][0]["glows"][0]["passes_to"].clone();
    assert_eq!(
        passes_to,
        serde_json::json!([{ "object": "ethylene", "group": "carbon" }, { "object": "hcl", "group": "chlorine" }])
    );
}

#[test]
fn two_lit_groups_of_the_same_atoms_and_a_highlight_of_a_product_group_in_the_step_are_conflicts() {
    let two = lit_double(r#""carbons": [2, 3]"#, "").replace(r#""groups": { "double": [1, 2] }"#, r#""groups": { "double": [1, 2], "pair": [1, 2] }"#).replace(
        r##""to": 1, "color": "#00ffff", "easing_curve": "linear" }"##,
        r##""to": 1, "color": "#00ffff", "easing_curve": "linear" }, { "type": "highlight", "object": "ethylene", "group": "pair", "to": 1 }"##,
    );
    let (_dir, path) = scene_file(&two);
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(
        scene_errors(&run),
        pairs(&[(
            "/timeline/1/animations/0/products/0",
            "conflicting_animations"
        )])
    );
    assert_eq!(
        run.json["errors"][0]["message"],
        "groups `double` and `pair` of `ethylene` both pass their glow to group `carbons` of `chloroethane`, and a group takes one glow; highlight one of them to 0 in an earlier step"
    );

    let in_step =
        r#", { "type": "highlight", "object": "chloroethane", "group": "carbons", "to": 1 }"#;
    let (_dir, path) = scene_file(&lit_double(r#""carbons": [2, 3]"#, in_step));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/1/animations/1", "conflicting_animations")])
    );
}

#[test]
fn mid_reaction_the_carried_glow_tints_the_carbons_and_leaves_the_chlorine_as_it_was() {
    let lit = lit_double(r#""carbons": [2, 3]"#, "");
    let unlit = lit.replace(r#""to": 1, "color""#, r#""to": 0, "color""#);
    let inspected = inspect_at(&lit, "2.5");
    let at = |i: usize| {
        let pixels = &atoms(&inspected)[i]["pixels"];
        (
            pixels[0].as_f64().unwrap().round() as usize,
            pixels[1].as_f64().unwrap().round() as usize,
        )
    };
    let (carbon, chlorine) = (at(0), at(6));

    let (_a, lit_frame) = frame_at(&lit, "2.5");
    let (_b, unlit_frame) = frame_at(&unlit, "2.5");

    assert_ne!(
        pixel(&lit_frame, carbon.0, carbon.1),
        pixel(&unlit_frame, carbon.0, carbon.1),
        "the film tints the carbon"
    );
    assert_eq!(
        pixel(&lit_frame, chlorine.0, chlorine.1),
        pixel(&unlit_frame, chlorine.0, chlorine.1),
        "the chlorine is in no lit group"
    );
}

#[test]
fn a_reaction_refuses_a_space_filling_reactant_or_product_at_its_place_in_the_list() {
    let objects = [
        molecule(
            "ethylene",
            "ethylene",
            r#", "position": [-2.6, 0, 0], "representation": "space-filling""#,
        ),
        molecule("hcl", "hydrogen-chloride", r#", "position": [2.6, 0, 0]"#),
        molecule(
            "chloroethane",
            "chloroethane",
            r#", "representation": "space-filling""#,
        ),
    ]
    .join(", ");
    let (_dir, path) = scene_file(&scene(&objects, &reaction(&atom_map(&HYDROCHLORINATION))));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/1/animations/0/reactants/0", "invalid"),
            ("/timeline/1/animations/0/products/0", "invalid"),
        ])
    );
    let messages: Vec<&str> = run.json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap())
        .collect();
    assert_eq!(
        messages[1],
        "`chloroethane` is drawn `space-filling`, and a reaction draws bonds breaking and forming as sticks; give `chloroethane` the `representation` `ball-and-stick`"
    );
}
