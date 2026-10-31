//! Atom groups, the highlight that lights them and the captions attached to them, on PubChem's
//! caffeine, whose atom 1 is the oxygen of a carbonyl with atom 9, its carbon. Scenes are 400 by 400
//! pixels on black with the default camera, as in `tests/molecule.rs`.

mod common;

use common::{frame_at, inspect_at, pairs, pixel, scene, scene_errors, scene_file, sci_anim};
use serde_json::Value;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pubchem");

fn caffeine(more: &str) -> String {
    format!(
        r##""caf": {{ "type": "molecule", "file": "{FIXTURES}/caffeine.sdf", "groups": {{ "carbonyl": [1, 9] }}{more} }}"##
    )
}

fn light_in() -> &'static str {
    r#"{ "type": "play", "duration": 1, "animations": [ { "type": "highlight", "object": "caf", "group": "carbonyl", "to": 1, "easing_curve": "linear" } ] }"#
}

fn pair(value: &Value) -> (f64, f64) {
    (value[0].as_f64().unwrap(), value[1].as_f64().unwrap())
}

#[test]
fn a_highlighted_oxygen_glows_around_its_ball_and_shows_red_at_its_center() {
    let source = scene(&caffeine(""), light_in());
    let inspected = inspect_at(&source, "0");
    let atoms = inspected["objects"]["caf"]["atoms"].as_array().unwrap();
    let (oxygen, carbon) = (pair(&atoms[0]["pixels"]), pair(&atoms[8]["pixels"]));
    let length = ((oxygen.0 - carbon.0).powi(2) + (oxygen.1 - carbon.1).powi(2)).sqrt();
    let away = |pixels: f64| {
        (
            (oxygen.0 + (oxygen.0 - carbon.0) / length * pixels).round() as usize,
            (oxygen.1 + (oxygen.1 - carbon.1) / length * pixels).round() as usize,
        )
    };
    let center = (oxygen.0.round() as usize, oxygen.1.round() as usize);

    let (_a, plain) = frame_at(&source, "0");
    let (_b, lit) = frame_at(&source, "1");

    let beside = away(26.0);
    assert_eq!(
        pixel(&plain, beside.0, beside.1),
        [0, 0, 0, 255],
        "nothing is drawn beside the ball without the glow"
    );
    assert_ne!(pixel(&lit, beside.0, beside.1), [0, 0, 0, 255]);
    let [r, g, b, _] = pixel(&lit, center.0, center.1);
    assert!(
        r > 200 && g < 110 && b < 70,
        "still red at the center: {r} {g} {b}"
    );
    assert_ne!(
        pixel(&lit, center.0, center.1),
        pixel(&plain, center.0, center.1),
        "the film tints the ball"
    );
}

#[test]
fn a_glow_is_blended_once_where_two_shells_overlap() {
    let source = scene(&caffeine(""), light_in());
    let inspected = inspect_at(&source, "1");
    let atoms = inspected["objects"]["caf"]["atoms"].as_array().unwrap();
    let (oxygen, carbon) = (pair(&atoms[0]["pixels"]), pair(&atoms[8]["pixels"]));
    let (_dir, lit) = frame_at(&source, "1");
    let length = (oxygen.0 - carbon.0).hypot(oxygen.1 - carbon.1);
    let across = (
        -(oxygen.1 - carbon.1) / length,
        (oxygen.0 - carbon.0) / length,
    );
    let mid = ((oxygen.0 + carbon.0) / 2.0, (oxygen.1 + carbon.1) / 2.0);
    for side in [-14.0, 14.0] {
        let [r, g, b, _] = pixel(
            &lit,
            (mid.0 + across.0 * side).round() as usize,
            (mid.1 + across.1 * side).round() as usize,
        );
        assert!(
            r <= 118 && g <= 96 && b <= 24,
            "one film of #ffd23f at 0.18 on black is at most [118, 96, 24], facing the camera, and two are brighter: {r} {g} {b}"
        );
    }
}

/// The film of a lone oxygen's shell, between the balls and sticks and the shell's rim, shades
/// smoothly from one pixel to the next. When the shell's far side showed wherever its triangles
/// were drawn first, it cut dark teeth and streaks into the film, 35 levels of green deep.
#[test]
fn a_lone_atoms_glow_shows_only_the_near_side_of_its_shell() {
    let water = format!(
        r#""water": {{ "type": "molecule", "file": "{FIXTURES}/water.sdf", "scale": [2.5, 2.5, 2.5], "groups": {{ "oxygen": [1] }} }}"#
    );
    let light = r##"{ "type": "play", "duration": 1, "animations": [ { "type": "highlight", "object": "water", "group": "oxygen", "to": 1, "color": "#00ff00" } ] }"##;
    let source = scene(&water, light);
    let (_a, plain) = frame_at(&source, "0");
    let (_b, lit) = frame_at(&source, "1");

    let black_around = |rgba: &[u8], x: usize, y: usize| {
        (x - 2..=x + 2)
            .flat_map(|i| (y - 2..=y + 2).map(move |j| pixel(rgba, i, j)[..3] == [0, 0, 0]))
            .collect::<Vec<bool>>()
    };
    let mut film = 0;
    let mut steepest = (0, 0, 0);
    for y in 2..398 {
        for x in 2..398 {
            let only_film = black_around(&plain, x, y).iter().all(|&black| black)
                && black_around(&lit, x, y).iter().all(|&black| !black);
            if !only_film {
                continue;
            }
            film += 1;
            let green = |x, y| i16::from(pixel(&lit, x, y)[1]);
            let step = (green(x, y) - green(x + 1, y))
                .abs()
                .max((green(x, y) - green(x, y + 1)).abs());
            if step > steepest.0 {
                steepest = (step, x, y);
            }
        }
    }
    assert!(
        film > 5000,
        "the shell shows as a film around the ball: {film} pixels"
    );
    assert!(
        steepest.0 <= 6,
        "the film's green steps by {} at pixel ({}, {})",
        steepest.0,
        steepest.1,
        steepest.2
    );
}

#[test]
fn a_caption_stays_beside_its_group_while_the_molecule_turns_180_degrees() {
    let caption = r##""note": { "type": "text", "text": "C=O", "size": 0.5, "align": "left", "attach": { "object": "caf", "group": "carbonyl", "offset": [1.9, 0] } }"##;
    let turn = r#"{ "type": "play", "duration": 2, "animations": [ { "type": "rotate", "object": "caf", "to": [0, 180, 0], "easing_curve": "linear" } ] }"#;
    let source = scene(&format!("{}, {caption}", caffeine("")), turn);

    let mut seen = Vec::new();
    for t in ["0", "0.5", "1", "1.5", "2"] {
        let inspected = inspect_at(&source, t);
        let group = &inspected["objects"]["caf"]["groups"]["carbonyl"];
        let note = &inspected["objects"]["note"];
        let (center, group_box) = (
            pair(&group["center_pixels"]),
            &group["bounding_box"]["pixels"],
        );
        let note_box = &note["bounding_box"]["pixels"];
        let (left, top, bottom) = (
            note_box["min"][0].as_f64().unwrap(),
            note_box["min"][1].as_f64().unwrap(),
            note_box["max"][1].as_f64().unwrap(),
        );
        let gap = left - group_box["max"][0].as_f64().unwrap();
        seen.push((left - center.0, (top + bottom) / 2.0 - center.1, gap));
    }
    for (dx, dy, gap) in &seen {
        assert!(
            (dx - 93.0).abs() < 0.05 && (dy - 0.12).abs() < 0.05,
            "the caption keeps its pixel offset from the group's center: {seen:?}"
        );
        assert!(
            *gap > 0.0,
            "the caption never meets the group's glow box: {seen:?}"
        );
    }
}

#[test]
fn group_and_attachment_mistakes_are_scene_errors_at_their_fields() {
    let bad_groups = r##""caf": { "type": "molecule", "file": "FIX/caffeine.sdf", "groups": { "empty": [], "zero": [0], "far": [1, 99], "twice": [2, 2] } }"##.replace("FIX", FIXTURES);
    let run = {
        let (_dir, path) = scene_file(&scene(&bad_groups, ""));
        sci_anim(&["validate", path.to_str().unwrap()])
    };
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/caf/groups/empty", "wrong_length"),
            ("/objects/caf/groups/zero/0", "out_of_range")
        ]),
        "the schema's errors come first, as for every field"
    );
    let bad_atoms = bad_groups.replace(r#""empty": [], "zero": [0], "#, "");
    let (_dir, path) = scene_file(&scene(&bad_atoms, ""));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/caf/groups/far/1", "no_such_atom"),
            ("/objects/caf/groups/twice/1", "invalid")
        ])
    );

    let objects = format!(
        r##"{}, "disc": {{ "type": "circle", "radius": 1 }}, "note": {{ "type": "text", "text": "x", "position": [1, 0, 0], "attach": {{ "object": "caf", "group": "carbonyl" }} }}, "lost": {{ "type": "text", "text": "x", "attach": {{ "object": "caf", "group": "nitro" }} }}, "wrong": {{ "type": "text", "text": "x", "attach": {{ "object": "disc", "group": "a" }} }}"##,
        caffeine("")
    );
    let (_dir, path) = scene_file(&scene(&objects, ""));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/objects/lost/attach/group", "unknown_group"),
            ("/objects/note/attach/offset", "attached"),
            ("/objects/wrong/attach/group", "not_molecule")
        ])
    );
}

#[test]
fn highlights_and_moves_of_attached_objects_are_refused() {
    let objects = format!(
        r##"{}, "disc": {{ "type": "circle", "radius": 1 }}, "note": {{ "type": "text", "text": "x", "attach": {{ "object": "caf", "group": "carbonyl" }} }}"##,
        caffeine("")
    );
    let steps = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "highlight", "object": "disc", "group": "carbonyl", "to": 1 }, { "type": "highlight", "object": "caf", "group": "nitro", "to": 1 }, { "type": "move", "object": "note", "to": [0, 0, 0] } ] }"#;
    let (_dir, path) = scene_file(&scene(&objects, steps));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(
        scene_errors(&run),
        pairs(&[
            ("/timeline/0/animations/0/object", "not_molecule"),
            ("/timeline/0/animations/1/group", "unknown_group"),
            ("/timeline/0/animations/2/object", "attached")
        ])
    );
}

#[test]
fn a_group_of_hidden_hydrogens_has_no_box_and_the_group_with_a_carbon_boxes_only_the_carbon() {
    let molecule = format!(
        r##""caf": {{ "type": "molecule", "file": "{FIXTURES}/caffeine.sdf", "hydrogens": false, "groups": {{ "only_h": [17, 18], "mixed": [9, 17] }} }}"##
    );
    let inspected = inspect_at(&scene(&molecule, ""), "0");
    let groups = &inspected["objects"]["caf"]["groups"];
    let carbon = &inspected["objects"]["caf"]["atoms"][8]["position"];
    assert!(
        groups["only_h"]["bounding_box"].is_null(),
        "two hidden hydrogens draw nothing: {}",
        groups["only_h"]
    );
    assert_eq!(
        groups["mixed"]["center"], *carbon,
        "the hidden hydrogen does not pull the center off the carbon"
    );
    let world = &groups["mixed"]["bounding_box"]["world"];
    let width = world["max"][0].as_f64().unwrap() - world["min"][0].as_f64().unwrap();
    assert!(
        (width - 1.45).abs() < 1e-4,
        "the carbon's ball, 1.7 / 4, plus 0.3 of glow on each side: {width}"
    );
}

#[test]
fn no_atom_covers_a_caption_on_the_near_or_the_far_side_of_the_molecule() {
    let tilted = |opacity: &str| {
        let molecule = format!(
            r##""caf": {{ "type": "molecule", "file": "{FIXTURES}/caffeine.sdf", "rotation": [70, 0, 0], "opacity": {opacity}, "groups": {{ "carbonyl": [1, 9], "methyl": [12, 16, 17, 18] }} }}"##
        );
        let captions = r##""near": { "type": "text", "text": "C=O", "size": 0.5, "color": "#00ff00", "attach": { "object": "caf", "group": "carbonyl" } }, "far": { "type": "text", "text": "CH3", "size": 0.5, "color": "#00ff00", "attach": { "object": "caf", "group": "methyl" } }"##;
        scene(&format!("{molecule}, {captions}"), "")
    };
    let inspected = inspect_at(&tilted("1"), "0");
    let atoms = inspected["objects"]["caf"]["atoms"].as_array().unwrap();
    assert!(
        atoms[0]["position"][2].as_f64().unwrap() > 2.0
            && atoms[11]["position"][2].as_f64().unwrap() < -2.0,
        "the carbonyl's oxygen faces the camera and the methyl's carbon faces away"
    );
    let (_a, shown) = frame_at(&tilted("1"), "0");
    let (_b, alone) = frame_at(&tilted("0"), "0");
    let glyph_pixels = |rgba: &[u8], id: &str| {
        let pixels = &inspected["objects"][id]["bounding_box"]["pixels"];
        let (min, max) = (pair(&pixels["min"]), pair(&pixels["max"]));
        let mut inside = Vec::new();
        for y in min.1.floor() as usize..=max.1.ceil() as usize {
            for x in min.0.floor() as usize..=max.0.ceil() as usize {
                if pixel(rgba, x, y) == [0, 255, 0, 255] {
                    inside.push((x, y));
                }
            }
        }
        inside
    };
    let counts: Vec<(usize, usize)> = ["near", "far"]
        .iter()
        .map(|id| {
            let (alone, shown) = (glyph_pixels(&alone, id), glyph_pixels(&shown, id));
            (
                alone.len(),
                alone.iter().filter(|p| !shown.contains(p)).count(),
            )
        })
        .collect();
    assert_eq!(
        counts,
        [(92, 0), (107, 0)],
        "each caption's glyph pixels, and how many of them an atom covers"
    );
}

#[test]
fn a_highlight_of_a_group_of_hidden_atoms_is_a_scene_error() {
    let molecule = format!(
        r##""caf": {{ "type": "molecule", "file": "{FIXTURES}/caffeine.sdf", "hydrogens": false, "groups": {{ "only_h": [17, 18], "mixed": [9, 17] }} }}"##
    );
    let steps = r#"{ "type": "play", "duration": 1, "animations": [ { "type": "highlight", "object": "caf", "group": "only_h", "to": 1 }, { "type": "highlight", "object": "caf", "group": "mixed", "to": 1 } ] }"#;
    let (_dir, path) = scene_file(&scene(&molecule, steps));
    let run = sci_anim(&["validate", path.to_str().unwrap()]);
    assert_eq!(
        scene_errors(&run),
        pairs(&[("/timeline/0/animations/0/group", "group_hidden")])
    );
}

#[test]
fn a_highlight_that_leaves_out_its_color_keeps_the_color_the_glow_has() {
    let molecule = format!(
        r##""caf": {{ "type": "molecule", "file": "{FIXTURES}/caffeine.sdf", "groups": {{ "carbonyl": [1, 9], "methyl": [12, 16, 17, 18] }} }}"##
    );
    let highlight = |group: &str, to: u8, color: &str| {
        format!(
            r#"{{ "type": "play", "duration": 1, "animations": [ {{ "type": "highlight", "object": "caf", "group": "{group}", "to": {to}{color} }} ] }}"#
        )
    };
    let steps = [
        highlight("carbonyl", 1, r##", "color": "#00ffff""##),
        highlight("carbonyl", 0, ""),
        highlight("carbonyl", 1, ""),
        highlight("methyl", 1, ""),
    ]
    .join(", ");
    let inspected = inspect_at(&scene(&molecule, &steps), "4");
    let groups = &inspected["objects"]["caf"]["groups"];
    let seen = |group: &str| {
        (
            groups[group]["glow"].as_f64().unwrap(),
            groups[group]["color"].as_str().unwrap().to_string(),
        )
    };
    assert_eq!(
        (seen("carbonyl"), seen("methyl")),
        ((1.0, "#00ffff".to_string()), (1.0, "#ffd23f".to_string()))
    );
}

#[test]
fn a_glow_that_is_off_lights_up_in_the_highlight_color_from_the_start_of_its_span() {
    let green = r##"{ "type": "play", "duration": 2, "animations": [ { "type": "highlight", "object": "caf", "group": "carbonyl", "to": 1, "color": "#83c167", "easing_curve": "linear" } ] }"##;
    let purple = r##"{ "type": "play", "duration": 2, "animations": [ { "type": "highlight", "object": "caf", "group": "carbonyl", "to": 0.5, "color": "#9a72ac", "easing_curve": "linear" } ] }"##;
    let source = scene(&caffeine(""), &[green, purple].join(", "));
    let seen = |t: &str| {
        let inspected = inspect_at(&source, t);
        let group = &inspected["objects"]["caf"]["groups"]["carbonyl"];
        (
            group["glow"].as_f64().unwrap(),
            group["color"].as_str().unwrap().to_string(),
        )
    };

    assert_eq!(
        seen("0.1"),
        (0.05, "#83c167".to_string()),
        "no yellow on the way in"
    );
    assert_eq!(seen("1"), (0.5, "#83c167".to_string()));
    assert_ne!(
        seen("3").1,
        "#83c167",
        "a lit glow mixes toward its next color"
    );
}

#[test]
fn in_the_example_each_caption_shows_at_full_glow_and_covers_no_atom() {
    let example = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/examples/functional_groups.json"
    );
    for (scene_time, group) in [
        ("2.25", "carbonyl_a"),
        ("5.75", "carbonyl_b"),
        ("9.25", "methyl"),
    ] {
        let run = sci_anim(&["inspect", example, "--scene-time", scene_time]);
        let objects = &run.json["objects"];
        let caption = &objects[format!("{group}_caption").as_str()];
        assert_eq!(
            (
                objects["caffeine"]["groups"][group]["glow"].as_f64(),
                caption["opacity"].as_f64(),
                caption["attached"]["covers_atoms"].as_array().map(Vec::len)
            ),
            (Some(1.0), Some(1.0), Some(0)),
            "at scene time {scene_time}"
        );
    }
}

#[test]
fn a_space_filling_glow_colors_its_atoms_and_spills_no_wider_than_their_balls() {
    let source = scene(
        &caffeine(r#", "representation": "space-filling", "scale": [0.5, 0.5, 0.5]"#),
        light_in(),
    );
    let inspected = inspect_at(&source, "0");
    let atoms = inspected["objects"]["caf"]["atoms"].as_array().unwrap();
    let (oxygen, carbon) = (pair(&atoms[0]["pixels"]), pair(&atoms[8]["pixels"]));
    let length = ((oxygen.0 - carbon.0).powi(2) + (oxygen.1 - carbon.1).powi(2)).sqrt();
    let away = |pixels: f64| {
        (
            (oxygen.0 + (oxygen.0 - carbon.0) / length * pixels).round() as usize,
            (oxygen.1 + (oxygen.1 - carbon.1) / length * pixels).round() as usize,
        )
    };
    let center = away(0.0);

    let (_a, plain) = frame_at(&source, "0");
    let (_b, lit) = frame_at(&source, "1");

    let [r, g, b, _] = pixel(&plain, center.0, center.1);
    assert!(r > 200 && g < 20 && b < 20, "red unlit: {r} {g} {b}");
    let [r, g, b, _] = pixel(&lit, center.0, center.1);
    assert!(
        r > 200 && g > 150 && b < 90,
        "the glow's yellow over the oxygen's red: {r} {g} {b}"
    );
    let ball = atoms[0]["ball_radius"].as_f64().unwrap();
    let edge = (0..40)
        .map(|past| ball + f64::from(past))
        .find(|&distance| {
            let beside = away(distance);
            pixel(&plain, beside.0, beside.1) == [0, 0, 0, 255]
        })
        .unwrap();
    let beside = away(edge + 4.0);
    assert_eq!(
        pixel(&lit, beside.0, beside.1),
        [0, 0, 0, 255],
        "the shell stands 0.05 angstroms off the ball, under 2 pixels, so 4 pixels past the edge of the oxygen's disc, {edge} pixels from its center, is black"
    );
}
