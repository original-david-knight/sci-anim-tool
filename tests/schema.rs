mod common;

use common::{scene_errors, scene_file, sci_anim};
use serde_json::{Value, json};

fn printed_schema() -> jsonschema::Validator {
    let run = sci_anim(&["schema"]);
    assert_eq!(run.exit_code, 0);
    jsonschema::validator_for(&run.json).expect("the printed schema is a valid JSON Schema")
}

fn good_scene() -> Value {
    json!({
        "scene_config": { "width": 320, "height": 180, "fps": 30, "background": "#000000" },
        "camera": { "position": [0, 0, 10], "target": [0, 0, 0], "up": [0, 1, 0], "fov": 45 },
        "objects": {
            "disc": { "type": "circle", "radius": 2, "rotation": [0, 30, 0],
                      "stroke": { "color": "#58c4dd", "width": 0.1 }, "fill": { "color": "#1c758a", "opacity": 0.5 } },
            "orb": { "type": "sphere", "radius": 1, "position": [0, 0, 1], "scale": [1, 2, 1],
                     "material": { "type": "standard", "color": "#fc6255", "roughness": 0.2, "metallic": 0, "opacity": 1 } }
        },
        "timeline": [
            { "type": "wait", "duration": 0.5 },
            { "type": "play", "duration": 1, "animations": [
                { "type": "fade", "object": "orb", "to": 0.5, "easing_curve": "slow-start" },
                { "type": "camera-move", "to": [2, 1, 9] },
                { "type": "camera-zoom", "to": 30, "easing_curve": "overshoot" }
            ] }
        ]
    })
}

#[test]
fn the_printed_schema_and_validate_agree_on_every_scene() {
    let printed = printed_schema();
    let mut scenes = vec![(good_scene(), true)];
    for (pointer, value) in [
        ("/objects/disc/radious", json!(1)),
        ("/objects/disc/type", json!("square")),
        ("/objects/orb/material/type", json!("glass")),
        ("/objects/orb/material/shine", json!(1)),
        ("/objects/disc/fill/color", json!("teal")),
        ("/objects/orb/position", json!([0, 0])),
        ("/camera/fov", json!(180)),
        ("/scene_config/width", json!(8193)),
        ("/scene_config/name", json!("x")),
        ("/timeline", json!({})),
        (
            "/timeline",
            json!([{ "type": "wait", "duration": 1, "animations": [] }]),
        ),
        (
            "/timeline",
            json!([{ "type": "play", "duration": 1, "animations": [{ "type": "fade", "object": "orb", "to": 2 }] }]),
        ),
        (
            "/timeline",
            json!([{ "type": "play", "duration": 1, "animations": [{ "type": "camera-zoom", "to": 0 }] }]),
        ),
        ("/timeline", json!([{}])),
    ] {
        let mut scene = good_scene();
        let (parent, field) = pointer.rsplit_once('/').unwrap();
        scene.pointer_mut(parent).unwrap()[field] = value;
        scenes.push((scene, false));
    }

    for (scene, valid) in scenes {
        let (_dir, path) = scene_file(&scene.to_string());
        let run = sci_anim(&["validate", path.to_str().unwrap()]);
        assert_eq!(printed.is_valid(&scene), valid, "printed schema on {scene}");
        assert_eq!(
            run.exit_code == 0,
            valid,
            "validate on {scene}\n{}",
            run.stdout
        );
    }
}

#[test]
fn the_printed_schema_reports_errors_at_the_same_paths_as_validate() {
    let printed = printed_schema();
    let mut scene = good_scene();
    scene["objects"]["orb"]["radius"] = json!(-1);
    scene["objects"]["disc"]["stroke"]["colour"] = json!("#ffffff");

    let mut paths: Vec<String> = printed
        .iter_errors(&scene)
        .map(|e| e.instance_path().to_string())
        .collect();
    paths.sort();
    let (_dir, path) = scene_file(&scene.to_string());
    let run = sci_anim(&["validate", path.to_str().unwrap()]);

    assert_eq!(paths, vec!["/objects/disc/stroke", "/objects/orb/radius"]);
    assert_eq!(
        scene_errors(&run),
        common::pairs(&[
            ("/objects/disc/stroke/colour", "unknown_field"),
            ("/objects/orb/radius", "out_of_range")
        ])
    );
}

/// Every place in `schema` that an agent reads with no `description`, as JSON Pointers into the
/// schema. A property set to `true` is a field flattened beside a tagged enum, which the outer
/// `properties` describes. A branch's `type` const is the tag, which the branch's own description
/// covers. An `if` only tests the tag.
fn undocumented(schema: &Value, at: &str, missing: &mut Vec<String>) {
    match schema {
        Value::Object(map) => {
            for (name, property) in map
                .get("properties")
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
            {
                let is_type_const = name == "type" && property.get("const").is_some();
                if property.is_object() && !is_type_const && property.get("description").is_none() {
                    missing.push(format!("{at}/properties/{name}"));
                }
            }
            for (key, child) in map {
                if key == "if" {
                    continue;
                }
                let child_at = format!("{at}/{key}");
                let is_type = matches!(key.as_str(), "then") || at == "/$defs";
                if is_type && child.get("description").is_none() {
                    missing.push(child_at.clone());
                }
                undocumented(child, &child_at, missing);
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                undocumented(item, &format!("{at}/{i}"), missing);
            }
        }
        _ => {}
    }
}

#[test]
fn every_field_and_type_of_the_printed_schema_has_a_description() {
    let schema = sci_anim(&["schema"]).json;
    let mut missing = Vec::new();
    if schema.get("description").is_none() {
        missing.push("(the whole schema)".to_string());
    }
    undocumented(&schema, "", &mut missing);
    assert!(
        missing.is_empty(),
        "{} places in the schema have no description:\n{}",
        missing.len(),
        missing.join("\n")
    );
}

#[test]
fn a_reader_that_stops_early_leaves_no_panic() {
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .arg("schema")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("the binary runs");
    // Closing the pipe before reading is what `head` does once it has its lines.
    drop(child.stdout.take());
    let out = child.wait_with_output().expect("the binary exits");

    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stderr), "");
}
