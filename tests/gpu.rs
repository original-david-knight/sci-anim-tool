//! Runs `sci_anim frame` with and without `--gpu` on whatever GPUs the machine has.

mod common;

use common::{scene_file, sci_anim};

const SCENE: &str = r##"{ "scene_config": { "width": 64, "height": 64 }, "objects": { "c": { "type": "circle", "radius": 1, "fill": { "color": "#ff0000" } } } }"##;

const KINDS: [&str; 5] = ["discrete", "integrated", "other", "virtual", "cpu"];

fn frame(gpu: Option<&str>) -> common::Run {
    let (dir, path) = scene_file(SCENE);
    let png = dir.path().join("f.png");
    let mut args = vec![
        "frame",
        path.to_str().unwrap(),
        "-t",
        "0",
        "-o",
        png.to_str().unwrap(),
    ];
    if let Some(kind) = gpu {
        args.extend(["--gpu", kind]);
    }
    sci_anim(&args)
}

#[test]
fn naming_the_kind_of_the_gpu_drawn_on_draws_on_the_same_gpu() {
    let default = frame(None);
    assert_eq!(default.exit_code, 0, "{}", default.stdout);
    let kind = default.json["gpu"]["kind"].as_str().unwrap();

    let chosen = frame(Some(kind));

    assert_eq!(chosen.exit_code, 0, "{}", chosen.stdout);
    assert_eq!(chosen.json["gpu"], default.json["gpu"]);
}

#[test]
fn a_kind_no_gpu_has_is_a_gpu_error_that_lists_the_gpus_there_are() {
    let default = frame(None);
    let name = default.json["gpu"]["name"].as_str().unwrap();
    let (kind, run) = KINDS
        .iter()
        .map(|kind| (kind, frame(Some(kind))))
        // A GPU of a kind can be there and still fail, as an RTX 4090's OpenGL GPU does on one desktop.
        .find(|(_, run)| {
            run.json["errors"][0]["message"]
                .as_str()
                .is_some_and(|message| message.starts_with("--gpu"))
        })
        .expect("no machine has a GPU of every kind");

    assert_eq!(run.exit_code, 2, "{}", run.stdout);
    assert_eq!(run.json["errors"][0]["kind"], "gpu");
    let message = run.json["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.starts_with(&format!(
            "--gpu {kind} names no GPU of this machine, which has "
        )),
        "{message}"
    );
    assert!(message.contains(name), "{message}");
}

#[test]
fn a_kind_gpu_does_not_take_is_a_usage_error() {
    let run = frame(Some("nvidia"));

    assert_eq!(run.exit_code, 2, "{}", run.stdout);
    assert_eq!(run.json["errors"][0]["kind"], "usage");
    let message = run.json["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("discrete, integrated, other, virtual, cpu"),
        "{message}"
    );
}
