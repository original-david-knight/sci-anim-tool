//! Runs `sci_anim inspect` the way an agent checks a scene time without looking at a frame.

mod common;

use common::{scene_file, sci_anim};
use serde_json::{Value, json};

const SCENE: &str = r##"{
  "scene_config": { "width": 400, "height": 400 },
  "objects": {
    "orb": { "type": "sphere", "radius": 0.5 },
    "far": { "type": "sphere", "radius": 0.5, "position": [0, 0, 20], "opacity": 0 }
  },
  "timeline": [
    { "type": "play", "duration": 2, "animations": [
      { "type": "move", "object": "orb", "to": [4, 0, 0], "easing_curve": "linear" },
      { "type": "fade", "object": "far", "to": 1, "easing_curve": "linear" }
    ] }
  ]
}"##;

fn numbers(value: &Value) -> Vec<f64> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n.as_f64().unwrap() * 1000.0).round() / 1000.0)
        .collect()
}

#[test]
fn inspect_gives_each_object_where_the_timeline_has_put_it() {
    let (_dir, path) = scene_file(SCENE);

    let run = sci_anim(&["inspect", path.to_str().unwrap(), "--scene-time", "1"]);

    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    let orb = &run.json["objects"]["orb"];
    assert_eq!(
        numbers(&orb["bounding_box"]["world"]["min"]),
        [1.5, -0.5, -0.5]
    );
    assert_eq!(
        numbers(&orb["bounding_box"]["world"]["max"]),
        [2.5, 0.5, 0.5]
    );
    assert_eq!(orb["in_frame"], "whole");
    assert_eq!(orb["opacity"], 1.0);
    assert_eq!(
        (
            orb["position"].clone(),
            orb["rotation"].clone(),
            orb["scale"].clone()
        ),
        (
            json!([2.0, 0.0, 0.0]),
            json!([0.0, 0.0, 0.0]),
            json!([1.0, 1.0, 1.0])
        )
    );
    let far = &run.json["objects"]["far"];
    assert_eq!(far["opacity"], 0.5);
    assert_eq!(
        (
            far["bounding_box"]["pixels"].clone(),
            far["in_frame"].clone()
        ),
        (Value::Null, json!("none")),
        "behind the camera"
    );
}

#[test]
fn stages_reports_every_frame_time_between_two_scene_times_and_both_ends() {
    let (_dir, path) = scene_file(SCENE);
    let path = path.to_str().unwrap();

    let stages = sci_anim(&["stages", path, "--from", "0.99", "--to", "1.02"]);
    let inspected = sci_anim(&["inspect", path, "--scene-time", "1"]);

    assert_eq!(stages.exit_code, 0, "{}", stages.stdout);
    assert_eq!(
        (stages.json["command"].clone(), stages.json["fps"].clone()),
        (json!("stages"), json!(60))
    );
    let times: Vec<f64> = stages.json["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|stage| stage["scene_time"].as_f64().unwrap())
        .collect();
    assert_eq!(
        times,
        [0.99, 1.0, 61.0 / 60.0, 1.02],
        "both ends, and the frame times 60/60 and 61/60 between them"
    );
    let at_one = &stages.json["stages"][1];
    let mut alone = inspected.json.clone();
    alone
        .as_object_mut()
        .unwrap()
        .retain(|field, _| field != "status" && field != "command");
    assert_eq!(
        *at_one, alone,
        "each stage is what inspect prints at its scene time"
    );
}

#[test]
fn stages_refuses_an_end_before_its_start_or_outside_the_scene() {
    let (_dir, path) = scene_file(SCENE);
    let path = path.to_str().unwrap();

    let backwards = sci_anim(&["stages", path, "--from", "1", "--to", "0.5"]);
    let outside = sci_anim(&["stages", path, "--from", "1", "--to", "3"]);

    assert_eq!(
        (
            backwards.exit_code,
            backwards.json["errors"][0]["kind"].clone()
        ),
        (2, json!("usage"))
    );
    assert_eq!(
        (outside.exit_code, outside.json["errors"][0]["code"].clone()),
        (1, json!("scene_time_out_of_range"))
    );
}

#[test]
fn inspect_reports_an_object_with_nothing_to_draw() {
    let (_dir, path) = scene_file(
        r##"{ "objects": { "ring": { "type": "circle", "radius": 1, "position": [1, 2, 3] } } }"##,
    );

    let run = sci_anim(&["inspect", path.to_str().unwrap(), "--scene-time", "0"]);

    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    assert_eq!(
        run.json["objects"],
        json!({ "ring": {
            "position": [1.0, 2.0, 3.0], "rotation": [0.0, 0.0, 0.0], "scale": [1.0, 1.0, 1.0],
            "opacity": 1.0, "created": 1.0, "bounding_box": null, "in_frame": "none"
        } })
    );
}
