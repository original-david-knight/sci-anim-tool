//! Chapters: named runs of steps that last at least their least duration, as a narrated video
//! times each run of steps to its voice clip.

mod common;

use common::{inspect_at, pairs, scene, scene_errors, scene_file, sci_anim};
use serde_json::json;

const OBJECTS: &str = r##""orb": { "type": "sphere", "radius": 0.5, "opacity": 0 }"##;

fn validate(timeline: &str) -> common::Run {
    let (_dir, path) = scene_file(&scene(OBJECTS, timeline));
    sci_anim(&["validate", path.to_str().unwrap()])
}

const NARRATED: &str = r##"{ "type": "wait", "duration": 1 },
  { "type": "chapter", "id": "intro", "min_duration": 4.4, "steps": [
    { "type": "play", "duration": 1, "animations": [{ "type": "fade", "object": "orb", "to": 1 }] },
    { "type": "play", "duration": 2, "animations": [{ "type": "move", "object": "orb", "to": [2, 0, 0] }] }
  ] },
  { "type": "chapter", "id": "pivot", "min_duration": 1, "steps": [
    { "type": "play", "duration": 2, "animations": [{ "type": "move", "object": "orb", "to": [0, 0, 0] }] }
  ] }"##;

#[test]
fn validate_reports_each_chapter_lasting_at_least_its_least_duration() {
    let run = validate(NARRATED);

    assert_eq!(
        run.json,
        json!({
            "status": "ok",
            "command": "validate",
            "duration": 7.4,
            "steps": [
                { "path": "/timeline/0", "start": 0.0, "end": 1.0 },
                { "path": "/timeline/1/steps/0", "start": 1.0, "end": 2.0 },
                { "path": "/timeline/1/steps/1", "start": 2.0, "end": 4.0 },
                { "path": "/timeline/2/steps/0", "start": 5.4, "end": 7.4 }
            ],
            "chapters": [{ "id": "intro", "start": 1.0, "end": 5.4 }, { "id": "pivot", "start": 5.4, "end": 7.4 }]
        })
    );
}

#[test]
fn a_chapter_holds_the_stage_until_its_least_duration_and_inspect_reports_it() {
    let scene = scene(OBJECTS, NARRATED);

    let held = inspect_at(&scene, "5");
    let boundary = inspect_at(&scene, "5.4");

    assert_eq!(held["chapter"], json!("intro"));
    assert_eq!(
        held["objects"]["orb"]["position"],
        json!([2.0, 0.0, 0.0]),
        "held where the last step left it"
    );
    assert_eq!(
        boundary["chapter"],
        json!("pivot"),
        "where one chapter ends and the next starts, the next"
    );
    assert_eq!(
        inspect_at(&scene, "0.5").get("chapter"),
        None,
        "the wait before any chapter"
    );
}

#[test]
fn a_chapter_id_repeated_after_another_chapter_is_a_duplicate() {
    let run = validate(
        r##"{ "type": "chapter", "id": "a", "steps": [{ "type": "wait", "duration": 1 }] },
      { "type": "chapter", "id": "b", "steps": [{ "type": "wait", "duration": 1 }] },
      { "type": "chapter", "id": "a", "steps": [{ "type": "wait", "duration": 1 }] }"##,
    );

    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/2/id", "duplicate_chapter")])
    );
}

#[test]
fn a_repeated_chapter_id_a_chapter_in_a_chapter_and_a_fault_in_a_chapters_step_are_errors_at_their_fields()
 {
    let run = validate(
        r##"{ "type": "chapter", "id": "a", "steps": [{ "type": "wait", "duration": 1 }] },
      { "type": "chapter", "id": "a", "steps": [
        { "type": "chapter", "id": "b", "steps": [{ "type": "wait", "duration": 1 }] },
        { "type": "play", "duration": 1, "animations": [{ "type": "fade", "object": "ghost", "to": 1 }] }
      ] }"##,
    );

    assert_eq!(run.exit_code, 1);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/1/id", "duplicate_chapter"),
            ("/timeline/1/steps/0/type", "invalid"),
            ("/timeline/1/steps/1/animations/0/object", "unknown_object"),
        ])
    );
}
