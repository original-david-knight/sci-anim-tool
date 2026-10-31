//! Molecules read from PDB files: the Dickerson dodecamer, 1BNA, and ubiquitin, 1UBQ, as the
//! Protein Data Bank serves them, and atom groups written by their residues.

mod common;

use common::{inspect_at, pairs, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

const STRUCTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/skills/sci-anim/examples/structures"
);

fn molecule(id: &str, file: &str, groups: &str) -> String {
    format!(
        r#""{id}": {{ "type": "molecule", "file": "{STRUCTURES}/{file}", "scale": [0.1, 0.1, 0.1], "groups": {{ {groups} }} }}"#
    )
}

fn validate(objects: &str) -> common::Run {
    let (_dir, path) = scene_file(&scene(objects, ""));
    sci_anim(&["validate", path.to_str().unwrap()])
}

fn numbers(value: &Value) -> Vec<u64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_u64().unwrap())
        .collect()
}

#[test]
fn inspect_reports_each_chain_with_its_residues_and_their_atom_names() {
    let inspected = inspect_at(&scene(&molecule("dna", "1bna.pdb", ""), ""), "0");
    let dna = &inspected["objects"]["dna"];

    assert_eq!(dna["atoms"].as_array().unwrap().len(), 486);
    assert_eq!(dna["bonds"].as_array().unwrap().len(), 544);
    let chains: Vec<(&str, usize)> = dna["chains"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            (
                c["chain"].as_str().unwrap(),
                c["residues"].as_array().unwrap().len(),
            )
        })
        .collect();
    assert_eq!(chains, [("A", 12), ("B", 12)]);
    let guanine = &dna["chains"][0]["residues"][3];
    assert_eq!(guanine["number"], 4);
    assert_eq!(guanine["name"], "DG");
    assert_eq!(guanine["atoms"]["P"], 58);
    assert_eq!(guanine["atoms"]["N1"], 75);
    assert_eq!(
        dna["chains"][1]["residues"][0]["atoms"]["O5'"], 245,
        "atoms keep the serial numbers the file gives them"
    );
}

#[test]
fn a_group_holds_every_atom_of_its_residues_or_only_their_nucleobases() {
    let groups = r#""pair": { "residues": [{ "chain": "A", "number": 4 }, { "chain": "B", "number": 21 }], "part": "nucleobase" }, "strand": { "residues": [{ "chain": "B" }] }, "guanine": { "residues": [{ "chain": "A", "number": 4 }] }"#;
    let inspected = inspect_at(&scene(&molecule("dna", "1bna.pdb", groups), ""), "0");
    let dna = &inspected["objects"]["dna"];

    let pair = numbers(&dna["groups"]["pair"]["atoms"]);
    let guanine = numbers(&dna["groups"]["guanine"]["atoms"]);
    assert_eq!(
        pair.len(),
        11 + 8,
        "guanine's nucleobase has 11 atoms and cytosine's 8, without hydrogens"
    );
    assert_eq!(
        guanine.len(),
        22,
        "a nucleotide of guanine, with its sugar and phosphate"
    );
    assert_eq!(
        &pair[..11],
        &guanine[11..],
        "the nucleobase is the residue's atoms after its phosphate and sugar"
    );
    assert_eq!(numbers(&dna["groups"]["strand"]["atoms"]).len(), 243);
}

#[test]
fn a_run_of_residues_holds_ubiquitins_helix() {
    let groups = r#""helix": { "residues": [{ "chain": "A", "from": 23, "to": 34 }] }"#;
    let inspected = inspect_at(&scene(&molecule("ub", "1ubq.pdb", groups), ""), "0");
    let ub = &inspected["objects"]["ub"];

    let helix = numbers(&ub["groups"]["helix"]["atoms"]);
    let residues = ub["chains"][0]["residues"].as_array().unwrap();
    let first = residues[22]["atoms"]["N"].as_u64().unwrap();
    let last = residues[33]["atoms"]
        .as_object()
        .unwrap()
        .values()
        .map(|n| n.as_u64().unwrap())
        .max()
        .unwrap();
    assert_eq!(
        (residues[22]["name"].as_str(), residues[33]["name"].as_str()),
        (Some("ILE"), Some("GLU"))
    );
    assert_eq!(helix, (first..=last).collect::<Vec<_>>());
    assert_eq!(helix.len(), 98);
}

#[test]
fn a_residue_the_file_does_not_have_is_a_scene_error_at_its_field() {
    let groups = r#""z": { "residues": [{ "chain": "Z", "number": 1 }] }, "far": { "residues": [{ "chain": "A", "number": 99 }] }, "back": { "residues": [{ "chain": "A", "from": 30, "to": 20 }] }, "past": { "residues": [{ "chain": "A", "from": 70, "to": 80 }] }, "base": { "residues": [{ "chain": "A", "number": 1 }], "part": "nucleobase" }"#;
    let caffeine = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/pubchem/caffeine.sdf"
    );
    let objects = format!(
        r#"{}, "caf": {{ "type": "molecule", "file": "{caffeine}", "groups": {{ "a": {{ "residues": [{{ "chain": "A" }}] }} }} }}"#,
        molecule("ub", "1ubq.pdb", groups)
    );

    let run = validate(&objects);

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/caf/groups/a/residues/0", "no_such_residue"),
            ("/objects/ub/groups/back/residues/0/to", "invalid"),
            ("/objects/ub/groups/base/residues/0", "invalid"),
            (
                "/objects/ub/groups/far/residues/0/number",
                "no_such_residue"
            ),
            ("/objects/ub/groups/past/residues/0/to", "no_such_residue"),
            ("/objects/ub/groups/z/residues/0/chain", "no_such_residue"),
        ])
    );
    let message = |path: &str| {
        run.json["errors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["path"] == path)
            .unwrap()["message"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(
        message("/objects/ub/groups/far/residues/0/number"),
        "chain `A` has no residue numbered 99; its 76 residues are numbered 1 to 76"
    );
    assert_eq!(
        message("/objects/ub/groups/z/residues/0/chain"),
        "the molecule has no chain `Z`; its chains are `A`"
    );
    assert_eq!(
        message("/objects/ub/groups/base/residues/0"),
        "residue 1 of chain `A` is MET, which is not a nucleotide and has no nucleobase; leave out `part` to hold all its atoms"
    );
}

#[test]
fn a_residue_pick_with_fields_of_two_forms_is_a_schema_error() {
    let groups = r#""mixed": { "residues": [{ "chain": "A", "number": 4, "to": 9 }] }, "long": { "residues": [{ "chain": "AB" }] }, "none": { "residues": [] }"#;

    let run = validate(&molecule("ub", "1ubq.pdb", groups));

    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/ub/groups/long/residues/0/chain", "invalid"),
            ("/objects/ub/groups/mixed/residues/0", "invalid"),
            ("/objects/ub/groups/none/residues", "wrong_length"),
        ])
    );
}

#[test]
fn an_end_sits_on_an_atom_by_its_serial_number() {
    let objects = format!(
        r#"{}, "a": {{ "type": "line", "from": {{ "object": "dna", "atom": 245 }}, "to": {{ "object": "dna", "atom": 1 }} }}, "b": {{ "type": "line", "from": {{ "object": "dna", "atom": 244 }}, "to": [0, 0] }}"#,
        molecule("dna", "1bna.pdb", "")
    );

    let run = validate(&objects);

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/b/from/atom", "no_such_atom")])
    );
    assert_eq!(
        run.json["errors"][0]["message"],
        "`dna` has 486 atoms, numbered from 1 to 487 with gaps, and an end of `b` names atom 244"
    );
}

#[test]
fn a_pdb_file_that_makes_no_molecule_names_the_file_and_line() {
    let dir = tempfile::tempdir().unwrap();
    let lines: Vec<&str> = std::fs::read_to_string(format!("{STRUCTURES}/1ubq.pdb"))
        .unwrap()
        .leak()
        .lines()
        .filter(|l| l.starts_with("ATOM"))
        .take(4)
        .collect();
    let bad = [
        lines[0],
        lines[1],
        &lines[2].replace("26.913", "26.9x3"),
        lines[3],
    ]
    .join("\n");
    std::fs::write(dir.path().join("bad.pdb"), bad).unwrap();
    std::fs::write(dir.path().join("empty.pdb"), "HEADER    NOTHING\nEND\n").unwrap();
    let scene_path = dir.path().join("scene.json");
    let objects = r#""bad": { "type": "molecule", "file": "bad.pdb" }, "empty": { "type": "molecule", "file": "empty.pdb" }, "gone": { "type": "molecule", "file": "gone.pdb" }"#;
    std::fs::write(&scene_path, scene(objects, "")).unwrap();

    let run = sci_anim(&["validate", scene_path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/bad/file", "bad_structure_file"),
            ("/objects/empty/file", "bad_structure_file"),
            ("/objects/gone/file", "unreadable_file"),
        ])
    );
    let messages: Vec<&str> = run.json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["message"].as_str().unwrap())
        .collect();
    assert_eq!(
        messages[0],
        "`bad.pdb` is not a structure file sci_anim can draw: line 3: the x coordinate in columns 31 to 38 is \"26.9x3\", which is not a number; its first line is \"ATOM      1  N   MET A   1      27.340  24.430   2.614  1.00  9.67           N\""
    );
    assert_eq!(
        messages[1],
        "`empty.pdb` is not a structure file sci_anim can draw: it has no atoms, so there is nothing to draw; its first line is \"HEADER    NOTHING\""
    );
    assert!(
        messages[2].contains("curl -fo gone.pdb https://files.rcsb.org/download/<PDB id>.pdb"),
        "{}",
        messages[2]
    );
}

fn cartoon(id: &str, file: &str, groups: &str) -> String {
    format!(
        r#""{id}": {{ "type": "molecule", "file": "{file}", "representation": "cartoon", "scale": [0.1, 0.1, 0.1], "groups": {{ {groups} }} }}"#
    )
}

#[test]
fn a_cartoon_hides_every_atom_and_inspect_lists_each_helix_and_strand() {
    let inspected = inspect_at(
        &scene(&cartoon("ub", &format!("{STRUCTURES}/1ubq.pdb"), ""), ""),
        "0",
    );
    let ub = &inspected["objects"]["ub"];

    let stretches: Vec<String> = ub["secondary_structure"]
        .as_array()
        .unwrap()
        .iter()
        .map(Value::to_string)
        .collect();
    assert_eq!(
        stretches,
        [
            r#"{"kind":"helix","id":"H1","class":1,"chain":"A","from":23,"to":34}"#,
            r#"{"kind":"helix","id":"H2","class":5,"chain":"A","from":56,"to":59}"#,
            r#"{"kind":"strand","sheet":"BET","strand":1,"chain":"A","from":10,"to":17}"#,
            r#"{"kind":"strand","sheet":"BET","strand":2,"chain":"A","from":1,"to":7}"#,
            r#"{"kind":"strand","sheet":"BET","strand":3,"chain":"A","from":64,"to":72}"#,
            r#"{"kind":"strand","sheet":"BET","strand":4,"chain":"A","from":40,"to":45}"#,
            r#"{"kind":"strand","sheet":"BET","strand":5,"chain":"A","from":48,"to":50}"#,
        ]
    );
    let atoms = ub["atoms"].as_array().unwrap();
    assert_eq!(
        (
            atoms.len(),
            atoms.iter().filter(|a| a["hidden"] == true).count(),
            atoms
                .iter()
                .filter(|a| a.get("ball_radius").is_some())
                .count()
        ),
        (602, 602, 0),
        "a cartoon draws no atom as a ball"
    );
}

#[test]
fn a_pdb_file_with_no_helix_or_sheet_records_draws_as_coil() {
    let dir = tempfile::tempdir().unwrap();
    let atoms: String = std::fs::read_to_string(format!("{STRUCTURES}/1ubq.pdb"))
        .unwrap()
        .lines()
        .filter(|l| l.starts_with("ATOM"))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(dir.path().join("coil.pdb"), atoms).unwrap();
    let scene_path = dir.path().join("scene.json");
    std::fs::write(&scene_path, scene(&cartoon("ub", "coil.pdb", ""), "")).unwrap();

    let run = sci_anim(&["validate", scene_path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 0, "{}", run.stdout);
}

#[test]
fn a_cartoon_of_an_sdf_molecule_a_group_with_no_alpha_carbon_and_a_cartoon_reactant_are_scene_errors()
 {
    let caffeine = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/pubchem/caffeine.sdf"
    );
    let run = validate(&cartoon("caf", caffeine, ""));
    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[("/objects/caf/representation", "invalid")])
    );

    let objects = format!(
        r#"{}, {}"#,
        cartoon(
            "ub",
            &format!("{STRUCTURES}/1ubq.pdb"),
            r#""nitrogen": [1], "helix": { "residues": [{ "chain": "A", "from": 23, "to": 34 }] }"#
        ),
        cartoon("dna", &format!("{STRUCTURES}/1bna.pdb"), "")
    );
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [
        { "type": "highlight", "object": "ub", "group": "nitrogen", "to": 1 },
        { "type": "highlight", "object": "ub", "group": "helix", "to": 1 } ] },
      { "type": "play", "duration": 1, "animations": [
        { "type": "reaction", "reactants": ["dna"], "products": ["ub"],
          "atom_map": [{ "reactant": { "object": "dna", "atom": 1 }, "product": { "object": "ub", "atom": 1 } }] } ] }"#;
    let (_dir, path) = scene_file(&scene(&objects, timeline));

    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(run.exit_code, 1);
    let errors = scene_errors(&run);
    for expected in pairs(&[
        ("/timeline/0/animations/0/group", "group_hidden"),
        ("/timeline/1/animations/0/reactants/0", "invalid"),
        ("/timeline/1/animations/0/products/0", "invalid"),
    ]) {
        assert!(errors.contains(&expected), "{expected:?} in {errors:?}");
    }
    assert!(
        !errors
            .iter()
            .any(|(path, _)| path.starts_with("/timeline/0/animations/1")),
        "a group of residues lights a cartoon: {errors:?}"
    );
}
