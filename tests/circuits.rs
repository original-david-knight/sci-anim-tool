//! Circuits: schematics whose currents and voltages sci_anim solves, read back from `validate`
//! and `inspect` as an agent reads them.

mod common;

use std::path::PathBuf;

use common::{scene, scene_file, sci_anim};
use serde_json::Value;

const LESSONS: [&str; 6] = [
    "series_resistors",
    "parallel_resistors",
    "switch_bulb",
    "voltage_divider",
    "rc_charging",
    "mixed_network",
];

fn lesson(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("skills/sci-anim/examples/{name}.json"))
}

fn inspect(path: &std::path::Path, scene_time: f64) -> Value {
    let run = sci_anim(&[
        "inspect",
        path.to_str().unwrap(),
        "--scene-time",
        &scene_time.to_string(),
    ]);
    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    run.json
}

fn circuit(stage: &Value) -> &Value {
    &stage["objects"]["circuit"]["circuit"]
}

fn number(value: &Value) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| panic!("a number: {value}"))
}

/// A component's or a wire's ends, current, and drop: its `from` end above its `to` end, none for a
/// wire.
type Conductor = ([i64; 2], [i64; 2], f64, Option<f64>);

/// Every component and wire.
fn currents_and_drops(circuit: &Value) -> Vec<Conductor> {
    let at = |v: &Value| [v[0].as_i64().unwrap(), v[1].as_i64().unwrap()];
    let components = circuit["components"]
        .as_object()
        .unwrap()
        .values()
        .map(|c| {
            let voltage = number(&c["voltage"]);
            let drop = if c["kind"] == "battery" {
                -voltage
            } else {
                voltage
            };
            (
                at(&c["from"]),
                at(&c["to"]),
                number(&c["current"]),
                Some(drop),
            )
        });
    let wires = circuit["wires"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| (at(&w["from"]), at(&w["to"]), number(&w["current"]), None));
    components.chain(wires).collect()
}

#[test]
fn kirchhoffs_laws_hold_in_every_lesson_at_every_second() {
    for name in LESSONS {
        let path = lesson(name);
        let duration = number(&sci_anim(&["validate", path.to_str().unwrap()]).json["duration"]);
        let mut scene_time = 0.0;
        while scene_time <= duration {
            let stage = inspect(&path, scene_time);
            let circuit = circuit(&stage);
            let voltage_at: Vec<([i64; 2], f64)> = circuit["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|n| {
                    (
                        [n["at"][0].as_i64().unwrap(), n["at"][1].as_i64().unwrap()],
                        number(&n["voltage"]),
                    )
                })
                .collect();
            let voltage = |p: [i64; 2]| voltage_at.iter().find(|(q, _)| *q == p).unwrap().1;
            let currents_and_drops = currents_and_drops(circuit);
            // Current: what leaves each node is what reaches it.
            for (node, _) in &voltage_at {
                let leaving: f64 = currents_and_drops
                    .iter()
                    .map(|(from, to, current, _)| {
                        (if from == node { *current } else { 0.0 })
                            - (if to == node { *current } else { 0.0 })
                    })
                    .sum();
                assert!(
                    leaving.abs() < 1e-9,
                    "{name} at {scene_time}: {leaving} A leaves {node:?}"
                );
            }
            // Voltage: every drop is the difference of its ends' node voltages, so the drops
            // round any loop sum to 0.
            for (from, to, _, drop) in &currents_and_drops {
                let between = voltage(*from) - voltage(*to);
                let drop = drop.unwrap_or(0.0);
                assert!(
                    (drop - between).abs() < 1e-9,
                    "{name} at {scene_time}: {drop} V across {from:?} to {to:?}, nodes differ by {between}"
                );
            }
            scene_time += 1.0;
        }
    }
}

#[test]
fn equivalent_resistances_are_the_hand_values() {
    // 12 V over the battery's current: 2 + 4; 3 and 6 in parallel; 2 then 6 and 3 in parallel.
    for (name, resistance) in [
        ("series_resistors", 6.0),
        ("parallel_resistors", 2.0),
        ("mixed_network", 4.0),
    ] {
        let stage = inspect(&lesson(name), 6.0);
        let battery = &circuit(&stage)["components"]["B"];
        let equivalent = number(&battery["voltage"]) / number(&battery["current"]);
        assert!(
            (equivalent - resistance).abs() < 1e-9,
            "{name}: {equivalent} ohms"
        );
    }
    let mixed = inspect(&lesson("mixed_network"), 6.0);
    let components = &circuit(&mixed)["components"];
    let (i2, i3) = (
        number(&components["R2"]["current"]),
        number(&components["R3"]["current"]),
    );
    assert!(
        (i2 - 1.0).abs() < 1e-9 && (i3 - 2.0).abs() < 1e-9,
        "{i2} A and {i3} A"
    );
}

#[test]
fn a_divider_gives_its_resistance_ratio_of_the_battery() {
    let stage = inspect(&lesson("voltage_divider"), 5.0);

    let reading = number(&circuit(&stage)["components"]["V"]["voltage"]);

    assert!(
        (reading - 9.0 * 2000.0 / 3000.0).abs() < 1e-9,
        "{reading} V"
    );
    assert_eq!(
        number(&stage["objects"]["reading"]["readout"]["value"]),
        reading
    );
}

#[test]
fn a_capacitor_reaches_1_minus_1_over_e_of_the_battery_one_time_constant_after_its_switch_closes() {
    // The switch closes as the simulation starts, at scene time 3; RC is 2 kΩ times 1 mF.
    let path = lesson("rc_charging");

    let at_rc = number(&circuit(&inspect(&path, 5.0))["components"]["C"]["voltage"]);
    let before = number(&circuit(&inspect(&path, 3.0))["components"]["C"]["voltage"]);

    let expected = 10.0 * (1.0 - (-1.0f64).exp());
    assert!(
        (at_rc - expected).abs() < 1e-6,
        "{at_rc} V, not {expected} V"
    );
    assert!((at_rc / 10.0 - 0.632).abs() < 0.001);
    assert_eq!(before, 0.0);
}

#[test]
fn the_battery_delivers_the_energy_the_resistor_turns_to_heat_plus_what_the_capacitor_stores() {
    let validated = sci_anim(&["validate", lesson("rc_charging").to_str().unwrap()]);
    let end = &validated.json["circuits"][0]["at_end"];

    let delivered = number(&end["circuit.B.energy"]);
    let heat = number(&end["circuit.R.energy"]);
    let stored = number(&end["circuit.C.energy"]);

    assert!(delivered > 0.09, "{delivered} J");
    assert!(
        (delivered - heat - stored).abs() < 1e-6 * delivered,
        "{delivered} J delivered, {heat} J heat, {stored} J stored"
    );
}

#[test]
fn a_bulb_draws_power_only_while_its_switch_is_closed() {
    // The switch closes 2 s and opens 6.5 s into the simulation, which starts at scene time 3.
    let path = lesson("switch_bulb");
    let power = |t: f64| number(&circuit(&inspect(&path, t))["components"]["L"]["power"]);

    assert_eq!(power(4.0), 0.0);
    assert!((power(6.0) - 12.0).abs() < 1e-9, "{}", power(6.0));
    assert_eq!(power(10.0), 0.0);
}

#[test]
fn a_circuit_solves_the_same_to_the_bit_on_every_run() {
    for name in ["rc_charging", "switch_bulb"] {
        let path = lesson(name);
        let path = path.to_str().unwrap();
        let first = sci_anim(&["stages", path, "--from", "3", "--to", "4"]);
        let second = sci_anim(&["stages", path, "--from", "3", "--to", "4"]);
        assert_eq!(first.exit_code, 0, "{}", first.stdout);
        assert_eq!(first.stdout, second.stdout, "{name}");
    }
}

fn faults(components: &str, simulate: &str) -> Vec<(String, String, String)> {
    let objects = format!(r#""c": {{ "type": "circuit", "components": [{components}] }}"#);
    let timeline = if simulate.is_empty() {
        String::new()
    } else {
        format!(
            r#"{{ "type": "play", "duration": 2, "animations": [ {{ "type": "simulate", "circuit": "c", {simulate} }} ] }}"#
        )
    };
    let (_dir, path) = scene_file(&scene(&objects, &timeline));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(run.exit_code, 1, "{}", run.stdout);
    run.json["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            let field = |k: &str| e[k].as_str().unwrap().to_string();
            (field("path"), field("code"), field("message"))
        })
        .collect()
}

const LOOP: &str = r#"
    { "id": "B", "kind": "battery", "emf": 6, "from": [0, 0], "to": [0, 2] },
    { "id": "R", "kind": "resistor", "resistance": 3, "from": [0, 2], "to": [2, 2] },
    { "kind": "wire", "from": [2, 2], "to": [2, 0] },
    { "kind": "wire", "from": [2, 0], "to": [0, 0] }"#;

#[test]
fn each_fault_names_what_is_wrong_and_what_would_pass() {
    let diagonal = faults(
        &format!(r#"{LOOP}, {{ "kind": "wire", "from": [0, 2], "to": [1, 3] }}"#),
        "",
    );
    assert_eq!(diagonal[0].0, "/objects/c/components/4/to");
    assert!(diagonal[0].2.contains("share an x or a y"), "{diagonal:?}");

    let shorted = faults(
        &format!(
            r#"{LOOP}, {{ "kind": "wire", "from": [0, 0], "to": [-1, 0] }}, {{ "kind": "wire", "from": [-1, 0], "to": [-1, 2] }}, {{ "kind": "wire", "from": [-1, 2], "to": [0, 2] }}"#
        ),
        "",
    );
    assert!(
        shorted.iter().any(|(_, code, message)| code == "invalid"
            && message.starts_with("battery `B` is shorted")
            && message.contains("put a resistor or a bulb in that loop")),
        "{shorted:?}"
    );

    let floating = faults(
        &format!(
            r#"{LOOP}, {{ "id": "R2", "kind": "resistor", "resistance": 1, "from": [2, 2], "to": [4, 2] }}"#
        ),
        "",
    );
    assert_eq!(floating[0].0, "/objects/c/components/4/to");
    assert!(
        floating[0].2.contains("joins nothing")
            && floating[0]
                .2
                .contains("end another component or a wire at [4, 2]"),
        "{floating:?}"
    );

    let series = faults(
        r#"
        { "id": "B", "kind": "battery", "emf": 6, "from": [0, 0], "to": [0, 2] },
        { "id": "R", "kind": "resistor", "resistance": 3, "from": [0, 2], "to": [2, 2] },
        { "id": "V", "kind": "voltmeter", "from": [2, 2], "to": [2, 0] },
        { "kind": "wire", "from": [2, 0], "to": [0, 0] }"#,
        "",
    );
    assert_eq!(series[0].0, "/objects/c/components/2");
    assert!(
        series[0].2.starts_with("voltmeter `V` is in series with")
            && series[0].2.contains("connect it across"),
        "{series:?}"
    );

    let across = faults(
        r#"{ "id": "B", "kind": "battery", "emf": 6, "from": [0, 0], "to": [0, 2] }, { "id": "R", "kind": "resistor", "resistance": 3, "from": [0, 2], "to": [2, 2] }, { "id": "R2", "kind": "resistor", "resistance": 3, "from": [2, 2], "to": [2, 0] }, { "kind": "wire", "from": [2, 0], "to": [0, 0] }, { "kind": "wire", "from": [0, 2], "to": [0, 3] }, { "id": "A", "kind": "ammeter", "from": [0, 3], "to": [2, 3] }, { "kind": "wire", "from": [2, 3], "to": [2, 2] }"#,
        "",
    );
    assert!(
        across
            .iter()
            .any(|(path, _, message)| path == "/objects/c/components/5"
                && message.starts_with("ammeter `A` is across resistor `R`")
                && message.contains("put it in series")),
        "{across:?}"
    );

    let missing = faults(
        r#"{ "id": "R", "kind": "resistor", "from": [0, 0], "to": [1, 0] }"#,
        "",
    );
    assert_eq!(
        (missing[0].0.as_str(), missing[0].1.as_str()),
        ("/objects/c/components/0", "missing_field")
    );
}

#[test]
fn a_switch_that_would_short_the_battery_is_refused_at_its_switching() {
    let errors = faults(
        &format!(
            r#"{LOOP}, {{ "id": "S", "kind": "switch", "from": [0, 2], "to": [0, 3] }}, {{ "kind": "wire", "from": [0, 3], "to": [-1, 3] }}, {{ "kind": "wire", "from": [-1, 3], "to": [-1, 0] }}, {{ "kind": "wire", "from": [-1, 0], "to": [0, 0] }}"#
        ),
        r#""switches": [{ "switch": "S", "closed": true, "at": 1 }]"#,
    );

    assert_eq!(errors[0].0, "/timeline/0/animations/0/switches");
    assert!(
        errors[0]
            .2
            .starts_with("when `S` closes at 1 s, battery `B` is shorted"),
        "{errors:?}"
    );
}

#[test]
fn a_switching_names_a_switch_of_the_circuit() {
    let errors = faults(LOOP, r#""switches": [{ "switch": "R", "closed": true }]"#);

    assert_eq!(errors[0].0, "/timeline/0/animations/0/switches/0/switch");
    assert!(
        errors[0].2.contains("has no switch `R`; it has none"),
        "{errors:?}"
    );
}

#[test]
fn an_inductors_current_rises_to_1_minus_1_over_e_of_its_final_value_in_l_over_r() {
    let objects = r#""c": { "type": "circuit", "components": [
        { "id": "B", "kind": "battery", "emf": 10, "from": [0, 0], "to": [0, 2] },
        { "id": "S", "kind": "switch", "from": [0, 2], "to": [2, 2] },
        { "id": "R", "kind": "resistor", "resistance": 2, "from": [2, 2], "to": [4, 2] },
        { "id": "L", "kind": "inductor", "inductance": 1, "from": [4, 2], "to": [4, 0] },
        { "kind": "wire", "from": [4, 0], "to": [0, 0] }
    ] }"#;
    let timeline = r#"{ "type": "play", "duration": 3, "animations": [{ "type": "simulate", "circuit": "c", "switches": [{ "switch": "S", "closed": true }] }] }"#;
    let (_dir, path) = scene_file(&scene(objects, timeline));

    let inductor = &inspect(&path, 0.5)["objects"]["c"]["circuit"]["components"]["L"];
    let end = &sci_anim(&["validate", path.to_str().unwrap()]).json["circuits"][0]["at_end"];

    let current = number(&inductor["current"]);
    assert!(
        (current - 5.0 * (1.0 - (-1.0f64).exp())).abs() < 1e-6,
        "{current} A"
    );
    let (delivered, heat, stored) = (
        number(&end["c.B.energy"]),
        number(&end["c.R.energy"]),
        number(&end["c.L.energy"]),
    );
    assert!(
        (delivered - heat - stored).abs() < 1e-6 * delivered,
        "{delivered} J, {heat} J, {stored} J"
    );
}
