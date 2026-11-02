//! A molecule's `mirror`: its structure file's atoms reflected through a plane across the
//! centroid, inspected, reacted and drawn as an agent sees them.
//!
//! The molecule is L-alanine, the skill's `examples/structures/alanine.sdf`: 13 atoms, with its
//! stereocenter at atom 4, so it is not its own mirror image. Scenes are 400 by 400 pixels on
//! black with the default camera, which looks down -z at the origin, so a scene that is its own
//! reflection across x = 0 draws a frame that is its own reflection across the middle column.

mod common;

use common::{frame_at, inspect_at, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

const ALANINE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/skills/sci-anim/examples/structures/alanine.sdf"
);

fn molecule(id: &str, more: &str) -> String {
    format!(r##""{id}": {{ "type": "molecule", "file": "{ALANINE}"{more} }}"##)
}

/// Each atom's element and position as the SDF file's atom block gives them.
fn file_atoms() -> Vec<(String, [f64; 3])> {
    let text = std::fs::read_to_string(ALANINE).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let count: usize = lines[3][..3].trim().parse().unwrap();
    lines[4..4 + count]
        .iter()
        .map(|line| {
            let fields: Vec<&str> = line.split_whitespace().collect();
            let at = |i: usize| fields[i].parse::<f64>().unwrap();
            (fields[3].to_string(), [at(0), at(1), at(2)])
        })
        .collect()
}

fn inspected_atoms(inspected: &Value, id: &str) -> Vec<(String, [f64; 3])> {
    inspected["objects"][id]["atoms"]
        .as_array()
        .unwrap()
        .iter()
        .map(|atom| {
            let p = &atom["position"];
            (
                atom["element"].as_str().unwrap().to_string(),
                [0, 1, 2].map(|i| p[i].as_f64().unwrap()),
            )
        })
        .collect()
}

#[test]
fn inspect_places_each_atom_at_its_file_position_reflected_through_the_centroid() {
    let file = file_atoms();
    let centroid = [0, 1, 2].map(|i| file.iter().map(|(_, p)| p[i]).sum::<f64>() / 13.0);
    for (axis, plane) in ["x", "y", "z"].iter().enumerate() {
        let inspected = inspect_at(
            &scene(
                &molecule("m", &format!(r#", "mirror": "{plane}""#)),
                r#"{ "type": "wait", "duration": 1 }"#,
            ),
            "0",
        );

        let atoms = inspected_atoms(&inspected, "m");

        assert_eq!(atoms.len(), file.len());
        for (number, ((element, at), (file_element, in_file))) in (1..).zip(atoms.iter().zip(&file))
        {
            let mut reflected = [0, 1, 2].map(|i| in_file[i] - centroid[i]);
            reflected[axis] = -reflected[axis];
            assert_eq!(element, file_element, "atom {number} keeps its element");
            assert!(
                (0..3).all(|i| (at[i] - reflected[i]).abs() < 1e-3),
                "mirror {plane}: atom {number} at {at:?}, its file position reflected is {reflected:?}"
            );
        }
    }
}

#[test]
fn a_mirrored_molecule_keeps_its_atom_numbers_bonds_groups_and_captions() {
    let objects = [
        molecule(
            "m",
            r#", "mirror": "x", "groups": { "stereocenter": [4], "methyl": [5, 8, 9, 10] }"#,
        ),
        r#""label": { "type": "text", "text": "S", "size": 0.4, "attach": { "object": "m", "group": "stereocenter", "offset": [0, 1.5] } }"#.to_string(),
    ]
    .join(",");
    let steps = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "highlight", "object": "m", "group": "methyl", "to": 1 }] }"#;

    let plain = inspect_at(&scene(&molecule("m", ""), ""), "0");
    let mirrored = inspect_at(&scene(&objects, steps), "1");

    assert_eq!(
        mirrored["objects"]["m"]["bonds"], plain["objects"]["m"]["bonds"],
        "the same bonds between the same atom numbers"
    );
    let number = |atom: &Value| atom["number"].clone();
    let numbers = |v: &Value| -> Vec<Value> {
        v["objects"]["m"]["atoms"]
            .as_array()
            .unwrap()
            .iter()
            .map(number)
            .collect()
    };
    assert_eq!(numbers(&mirrored), numbers(&plain));
    let groups = &mirrored["objects"]["m"]["groups"];
    assert_eq!(groups["methyl"]["glow"], 1.0, "{groups}");
    let center = &groups["stereocenter"]["center"];
    let carbon = &mirrored["objects"]["m"]["atoms"][3]["position"];
    assert_eq!(center, carbon, "the group of atom 4 is centered on atom 4");
}

#[test]
fn an_unknown_mirror_plane_is_a_scene_error_at_mirror() {
    let (_dir, path) = scene_file(&scene(&molecule("m", r#", "mirror": "w""#), ""));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    assert_eq!(
        scene_errors(&run)[0].0,
        "/objects/m/mirror",
        "{}",
        run.stdout
    );
}

#[test]
fn a_reaction_from_a_molecule_to_its_mirror_image_moves_each_atom_through_the_mirror() {
    let identity: Vec<String> = (1..=13)
        .map(|n| {
            format!(r#"{{ "reactant": {{ "object": "l", "atom": {n} }}, "product": {{ "object": "d", "atom": {n} }} }}"#)
        })
        .collect();
    let objects = [molecule("l", ""), molecule("d", r#", "mirror": "x""#)].join(",");
    let steps = format!(
        r#"{{ "type": "play", "duration": 1, "animations": [{{ "type": "reaction", "reactants": ["l"], "products": ["d"], "fit": false, "easing_curve": "linear", "atom_map": [{}] }}] }}"#,
        identity.join(",")
    );
    let (_dir, path) = scene_file(&scene(&objects, &steps));

    let validated = sci_anim(&["validate", path.to_str().unwrap()]);
    let halfway = inspect_at(&scene(&objects, &steps), "0.5");

    assert_eq!(validated.exit_code, 0, "{}", validated.stdout);
    let atoms = halfway["reactions"][0]["atoms"].as_array().unwrap();
    assert_eq!(atoms.len(), 13);
    for atom in atoms {
        let x = atom["position"][0].as_f64().unwrap();
        assert!(
            x.abs() < 1e-4,
            "halfway every atom is in the mirror plane x = 0: {atom}"
        );
    }
}

/// How many of a frame's drawn pixels have no drawn pixel at their reflection across the middle
/// column, and how many pixels are drawn.
fn unreflected(rgba: &[u8]) -> (usize, usize) {
    let drawn = |x: usize, y: usize| rgba[(y * 400 + x) * 4..][..3].iter().any(|&c| c > 24);
    let mut count = (0, 0);
    for y in 0..400 {
        for x in 0..400 {
            if drawn(x, y) {
                count.1 += 1;
                if !drawn(399 - x, y) {
                    count.0 += 1;
                }
            }
        }
    }
    count
}

#[test]
fn a_molecule_and_its_mirror_image_side_by_side_draw_a_frame_that_is_its_own_reflection() {
    let pair = |right: &str| {
        let objects = [
            molecule(
                "l",
                r#", "position": [-2, 0.3, 0], "rotation": [20, 30, 40], "scale": [0.8, 0.8, 0.8]"#,
            ),
            molecule(
                "d",
                &format!(r#", "position": [2, 0.3, 0], "scale": [0.8, 0.8, 0.8]{right}"#),
            ),
        ]
        .join(",");
        frame_at(&scene(&objects, ""), "0").1
    };

    let (mirrored_off, mirrored_drawn) =
        unreflected(&pair(r#", "rotation": [20, -30, -40], "mirror": "x""#));
    let (copied_off, copied_drawn) = unreflected(&pair(r#", "rotation": [20, -30, -40]"#));

    assert!(mirrored_drawn > 10_000, "{mirrored_drawn} pixels drawn");
    assert!(
        mirrored_off * 100 < mirrored_drawn,
        "the mirror pair's outline is its own reflection, but for antialiased edges: {mirrored_off} of {mirrored_drawn} drawn pixels are not"
    );
    assert!(
        copied_off * 10 > copied_drawn,
        "without `mirror`, the turned copy is not the reflection: {copied_off} of {copied_drawn} drawn pixels are off"
    );
}
