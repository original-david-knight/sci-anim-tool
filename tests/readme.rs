mod common;

use common::{scene_file, sci_anim};

/// The first scene a reader copies from the README must pass `validate` as written.
#[test]
fn the_readme_scene_file_validates() {
    let readme = include_str!("../README.md");
    let scene = readme
        .split("```json\n")
        .nth(1)
        .and_then(|rest| rest.split("```").next())
        .expect("the README has a ```json block");
    let (_dir, path) = scene_file(scene);
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(
        (run.exit_code, run.json["status"].as_str()),
        (0, Some("ok")),
        "{}",
        run.stdout
    );
}

/// Each whole scene in docs/scene-files.md must pass `validate` as written. A ```json block
/// that is not an object with `objects` is a fragment, such as one object, and is not checked.
#[test]
fn every_scene_in_how_scene_files_work_validates() {
    let page = include_str!("../docs/scene-files.md");
    let scenes: Vec<&str> = page
        .split("```json\n")
        .skip(1)
        .filter_map(|rest| rest.split("```").next())
        .filter(|block| {
            serde_json::from_str::<serde_json::Value>(block)
                .is_ok_and(|value| value.get("objects").is_some())
        })
        .collect();
    assert!(!scenes.is_empty(), "docs/scene-files.md has a whole scene");
    for scene in scenes {
        let (_dir, path) = scene_file(scene);
        let run = sci_anim(&["validate", path.to_str().unwrap()]);
        assert_eq!(
            (run.exit_code, run.json["status"].as_str()),
            (0, Some("ok")),
            "{scene}\n{}",
            run.stdout
        );
    }
}
