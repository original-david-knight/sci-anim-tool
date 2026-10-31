//! `sci_anim check-layout`, run as an agent runs it. The scenes are 400 by 400 pixels with the
//! default camera.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{read_rgba, scene, scene_file, sci_anim};

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/pubchem");

struct Checked {
    exit_code: i32,
    stdout: String,
    stderr: String,
    stages: String,
    out: PathBuf,
    _dir: tempfile::TempDir,
}

fn check_layout(scene: &str, flags: &[&str]) -> Checked {
    let (dir, path) = scene_file(scene);
    let out = dir.path().join("check");
    let run = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .arg("check-layout")
        .arg(&path)
        .arg("--out")
        .arg(&out)
        .args(flags)
        .output()
        .expect("the binary runs");
    let stages: PathBuf = out.join("stages.txt");
    Checked {
        exit_code: run.status.code().expect("an exit code"),
        stdout: String::from_utf8(run.stdout).unwrap(),
        stderr: String::from_utf8(run.stderr).unwrap(),
        stages: std::fs::read_to_string(stages).unwrap_or_default(),
        out,
        _dir: dir,
    }
}

fn fault_lines(checked: &Checked) -> Vec<&str> {
    checked
        .stdout
        .lines()
        .filter(|line| line.starts_with("t="))
        .collect()
}

fn unverified_lines(checked: &Checked) -> Vec<&str> {
    checked
        .stdout
        .lines()
        .filter(|line| line.starts_with("Contrast unverified at "))
        .collect()
}

fn transparent_scene(objects: &str, timeline: &str) -> String {
    scene(objects, timeline).replace(
        "\"background\": \"#000000\"",
        "\"background\": \"transparent\"",
    )
}

fn panel_and_text(fill: &str, panel_z: f32, text: &str) -> String {
    let objects = format!(
        r##""panel": {{ "type": "polygon", "points": [[-3, -1], [3, -1], [3, 1], [-3, 1]], "position": [0, 0, {panel_z}], "fill": {{ "color": "{fill}" }} }},
        "t": {{ "type": "text", "text": "Read me", "color": "#ffffff", {text} }}"##
    );
    transparent_scene(&objects, r#"{ "type": "wait", "duration": 1 }"#)
}

#[test]
fn transparent_text_on_a_covering_dark_panel_passes() {
    let checked = check_layout(
        &panel_and_text("#1d4a5a", -1.0, "\"position\": [0, 0, 0]"),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert_eq!(
        checked.stdout.lines().next(),
        Some("No problem at any of 1 checked scene times.")
    );
    assert!(unverified_lines(&checked).is_empty(), "{}", checked.stdout);
}

#[test]
fn transparent_text_on_a_covering_light_panel_fails_verified_contrast() {
    let checked = check_layout(
        &panel_and_text("#eeeeee", -1.0, "\"position\": [0, 0, 0]"),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (end of step 0): "t" is hard to read: #ffffff on the fill behind it, which shows as #eeeeee has contrast 1.2, and needs 4.5"#
        ]
    );
    assert!(unverified_lines(&checked).is_empty(), "{}", checked.stdout);
}

#[test]
fn attached_text_uses_its_inspected_position_over_an_opaque_panel() {
    let checked = check_layout(
        &panel_and_text(
            "#1d4a5a",
            -1.0,
            "\"attach\": {\"object\": \"panel\", \"offset\": [0, 0]}",
        ),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert_eq!(
        checked.stdout.lines().next(),
        Some("No problem at any of 1 checked scene times.")
    );
    assert!(unverified_lines(&checked).is_empty(), "{}", checked.stdout);
}

#[test]
fn transparent_panel_recolor_uses_the_color_at_each_checked_scene_time() {
    let objects = r##""panel": { "type": "polygon", "points": [[-3, -1], [3, -1], [3, 1], [-3, 1]], "position": [0, 0, -1], "fill": { "color": "#1d4a5a" } },
        "t": { "type": "text", "text": "Read me", "color": "#ffffff" }"##;
    let timeline = r##"{ "type": "play", "duration": 1, "animations": [{ "type": "recolor", "object": "panel", "fill": "#eeeeee" }] }"##;
    let checked = check_layout(&transparent_scene(objects, timeline), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(fault_lines(&checked).len(), 2, "{}", checked.stdout);
    assert!(
        fault_lines(&checked)[1].contains("#eeeeee has contrast 1.2"),
        "{}",
        checked.stdout
    );
    assert!(unverified_lines(&checked).is_empty(), "{}", checked.stdout);
}

#[test]
fn fading_and_partly_created_panels_leave_transparent_contrast_unverified() {
    let panel = r##""panel": { "type": "polygon", "points": [[-3, -1], [3, -1], [3, 1], [-3, 1]], "position": [0, 0, -1], "fill": { "color": "#eeeeee" } },
        "t": { "type": "text", "text": "Read me", "color": "#ffffff" }"##;
    let fade = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "fade", "object": "panel", "to": 0.5 }] }"#;
    let checked = check_layout(
        &transparent_scene(&panel.replace("#eeeeee", "#1d4a5a"), fade),
        &["--no-sheet"],
    );
    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert!(
        unverified_lines(&checked)
            .iter()
            .any(|line| line.contains("t=0.5 to 1")),
        "{}",
        checked.stdout
    );
    assert!(
        checked.stages.contains("contrast unverified"),
        "{}",
        checked.stages
    );

    let created = panel.replace(
        "\"fill\":",
        "\"stroke\": {\"color\": \"#eeeeee\"}, \"fill\":",
    );
    let create = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "create", "object": "panel" }] }"#;
    let checked = check_layout(&transparent_scene(&created, create), &["--no-sheet"]);
    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert!(
        checked.stdout.contains("Contrast unverified for"),
        "{}",
        checked.stdout
    );
    assert!(
        checked.stages.contains("contrast unverified"),
        "{}",
        checked.stages
    );
}

#[test]
fn partly_written_text_has_unknown_contrast_until_its_fill_is_complete() {
    let objects = r##""panel": { "type": "polygon", "points": [[-3, -1], [3, -1], [3, 1], [-3, 1]], "position": [0, 0, -1], "fill": { "color": "#eeeeee" } },
        "t": { "type": "text", "text": "Read me", "color": "#ffffff" }"##;
    let write =
        r#"{ "type": "play", "duration": 1, "animations": [{ "type": "write", "object": "t" }] }"#;
    let checked = check_layout(&transparent_scene(objects, write), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (end of step 0): "t" is hard to read: #ffffff on the fill behind it, which shows as #eeeeee has contrast 1.2, and needs 4.5"#
        ]
    );
    assert!(
        unverified_lines(&checked)
            .iter()
            .any(|line| line.contains("t=0.5")),
        "{}",
        checked.stdout
    );
}

#[test]
fn text_a_fade_leaves_faded_on_a_covering_panel_is_judged_where_it_rests() {
    let objects = r##""panel": { "type": "polygon", "points": [[-3, -1], [3, -1], [3, 1], [-3, 1]], "position": [0, 0, -1], "fill": { "color": "#eeeeee" } },
        "t": { "type": "text", "text": "Read me", "color": "#ffffff", "opacity": 0 }"##;
    let fade = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "fade", "object": "t", "to": 0.5 }] }"#;
    let checked = check_layout(&transparent_scene(objects, fade), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (end of step 0): "t" is hard to read: #ffffff at object opacity 0.5 shows as #f7f7f7, and on the fill behind it, which shows as #eeeeee has contrast 1.1, and needs 4.5"#
        ]
    );
    assert!(unverified_lines(&checked).is_empty(), "{}", checked.stdout);
}

#[test]
fn shapes_without_proved_coverage_or_depth_leave_contrast_unverified() {
    let circle = r##""panel": { "type": "circle", "radius": 2, "position": [0, 0, -1], "fill": { "color": "#eeeeee" } },
        "t": { "type": "text", "text": "Read me", "color": "#ffffff" }"##;
    let checked = check_layout(
        &transparent_scene(circle, r#"{ "type": "wait", "duration": 1 }"#),
        &["--no-sheet"],
    );
    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert_eq!(fault_lines(&checked), [] as [&str; 0]);
    assert_eq!(unverified_lines(&checked).len(), 1, "{}", checked.stdout);

    let checked = check_layout(
        &panel_and_text("#eeeeee", 0.0, "\"position\": [0, 0, -1]"),
        &["--no-sheet"],
    );
    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert_eq!(unverified_lines(&checked).len(), 1, "{}", checked.stdout);

    let narrow = r##""panel": { "type": "polygon", "points": [[-0.5, -1], [0.5, -1], [0.5, 1], [-0.5, 1]], "position": [0, 0, -1], "fill": { "color": "#eeeeee" } },
        "t": { "type": "text", "text": "Read me", "color": "#ffffff" }"##;
    let checked = check_layout(
        &transparent_scene(narrow, r#"{ "type": "wait", "duration": 1 }"#),
        &["--no-sheet", "--allow", "panel,t"],
    );
    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert_eq!(unverified_lines(&checked).len(), 1, "{}", checked.stdout);
}

#[test]
fn an_object_between_panel_and_text_makes_contrast_unverified() {
    let objects = r##""panel": { "type": "polygon", "points": [[-3, -1], [3, -1], [3, 1], [-3, 1]], "position": [0, 0, -1], "fill": { "color": "#eeeeee" } },
        "blocker": { "type": "circle", "radius": 0.4, "position": [0, 0, -0.5], "fill": { "color": "#1d4a5a" } },
        "t": { "type": "text", "text": "Read me", "color": "#ffffff" }"##;
    let checked = check_layout(
        &transparent_scene(objects, r#"{ "type": "wait", "duration": 1 }"#),
        &["--no-sheet", "--allow", "blocker,t"],
    );

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert!(fault_lines(&checked).is_empty(), "{}", checked.stdout);
    assert_eq!(unverified_lines(&checked).len(), 1, "{}", checked.stdout);
}

#[test]
fn a_glow_reaching_text_makes_contrast_unverified() {
    let objects = format!(
        r##""panel": {{ "type": "polygon", "points": [[-3, -1], [3, -1], [3, 1], [-3, 1]], "position": [0, 0, -2], "fill": {{ "color": "#1d4a5a" }} }},
        "t": {{ "type": "text", "text": "Read me", "position": [0, 0, 1] }},
        "m": {{ "type": "molecule", "file": "{FIXTURES}/ethanol.sdf", "position": [2.2, 0, 0], "scale": [0.5, 0.5, 0.5], "groups": {{ "heavy": [1] }} }}"##
    );
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "highlight", "object": "m", "group": "heavy", "to": 1 }] }"#;
    let checked = check_layout(
        &transparent_scene(&objects, timeline),
        &["--no-sheet", "--allow", "m/heavy,t"],
    );

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert!(fault_lines(&checked).is_empty(), "{}", checked.stdout);
    assert_eq!(unverified_lines(&checked).len(), 1, "{}", checked.stdout);
    assert!(
        unverified_lines(&checked)[0].contains("t=0.5 to 1"),
        "{}",
        checked.stdout
    );
}

#[test]
fn tilted_text_leaves_transparent_contrast_unverified() {
    let checked = check_layout(
        &panel_and_text("#1d4a5a", -1.0, "\"rotation\": [20, 0, 0]"),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert_eq!(unverified_lines(&checked).len(), 1, "{}", checked.stdout);
    assert!(
        checked.stages.contains("contrast unverified"),
        "{}",
        checked.stages
    );
}

#[test]
fn a_frame_cutoff_fails_when_transparent_contrast_is_unverified() {
    let objects = r##""t": { "type": "text", "text": "Off frame", "position": [0, -5, 0] }"##;
    let checked = check_layout(
        &transparent_scene(objects, r#"{ "type": "wait", "duration": 1 }"#),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert!(
        fault_lines(&checked)
            .iter()
            .any(|line| line.contains("is cut off by the frame")),
        "{}",
        checked.stdout
    );
    assert_eq!(unverified_lines(&checked).len(), 1, "{}", checked.stdout);
    assert!(
        checked.stages.contains("contrast unverified"),
        "{}",
        checked.stages
    );
}

const TWO_TEXTS: &str = r##""a": { "type": "text", "text": "A" }, "b": { "type": "text", "text": "B", "position": [-3, 0, 0] }"##;

fn ten_second_step_moving_b_through(waypoint: &str) -> String {
    let timeline = format!(
        r##"{{ "type": "play", "duration": 10, "animations": [
          {{ "type": "move", "object": "b", "to": {waypoint}, "start": 0.5, "end": 1.5 }},
          {{ "type": "move", "object": "b", "to": [-3, 0, 0], "start": 1.5, "end": 2.5 }}
        ] }}"##
    );
    scene(TWO_TEXTS, &timeline)
}

#[test]
fn a_text_moved_through_another_over_an_early_span_of_a_long_step_is_reported_at_that_span() {
    let checked = check_layout(
        &ten_second_step_moving_b_through("[0, 0, 0]"),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    let lines = fault_lines(&checked);
    assert_eq!(lines.len(), 1, "{}", checked.stdout);
    assert!(
        lines[0].starts_with(
            r#"t=1.5 (end of animation 0 (move of "b") of step 0): "a" and "b" overlap by "#
        ),
        "{}",
        lines[0]
    );
}

#[test]
fn a_path_around_the_other_text_passes() {
    let checked = check_layout(
        &ten_second_step_moving_b_through("[0, 2, 0]"),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert_eq!(
        checked.stdout.lines().next(),
        Some("No problem at any of 7 checked scene times.")
    );
}

#[test]
fn text_recolored_to_the_background_is_hard_to_read() {
    let scene = r##"{
      "scene_config": { "background": "#101824" },
      "objects": { "label": { "type": "text", "text": "This should fail contrast", "color": "#ffffff", "size": 0.6 } },
      "timeline": [
        { "type": "play", "duration": 0.7, "animations": [{ "type": "write", "object": "label" }] },
        { "type": "play", "duration": 0.5, "animations": [{ "type": "recolor", "object": "label", "color": "#101824" }] },
        { "type": "wait", "duration": 1 }
      ]
    }"##;

    let checked = check_layout(scene, &["--no-sheet"]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1.2 to 2.2 (end of step 1 to end of step 2, 2 checks): "label" is hard to read: #101824 on the background #101824 has contrast 1.0, and needs 4.5"#
        ]
    );
}

fn panel_recolored_to(fill: &str) -> String {
    let objects = r##""panel": { "type": "polygon", "points": [[-3, -1], [3, -1], [3, 1], [-3, 1]], "fill": { "color": "#333333" } },
      "t": { "type": "text", "text": "Read me", "color": "#ffffff" }"##;
    let timeline = format!(
        r##"{{ "type": "play", "duration": 1, "animations": [{{ "type": "recolor", "object": "panel", "fill": "{fill}" }}] }}"##
    );
    scene(objects, &timeline)
}

#[test]
fn a_panel_recolored_light_behind_white_text_is_reported() {
    let checked = check_layout(&panel_recolored_to("#eeeeee"), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=0.5 (middle of step 0): "t" is hard to read: #ffffff on the fill behind it, which shows as #b2b2b2 has contrast 2.1, and needs 4.5"#,
            r#"t=1 (end of step 0): "t" is hard to read: #ffffff on the fill behind it, which shows as #eeeeee has contrast 1.2, and needs 4.5"#,
        ]
    );
}

#[test]
fn a_panel_recolored_dark_behind_white_text_passes() {
    let checked = check_layout(&panel_recolored_to("#1d4a5a"), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
}

#[test]
fn a_molecule_is_spared_the_hidden_atom_check_only_while_a_rotate_turns_it() {
    let objects = format!(
        r##""m": {{ "type": "molecule", "file": "{FIXTURES}/ethanol.sdf", "groups": {{ "heavy": [1, 2, 3] }} }}"##
    );
    let timeline = r#"{ "type": "play", "duration": 10, "animations": [
        { "type": "rotate", "object": "m", "to": [0, 180, 0], "easing_curve": "linear", "start": 0.5, "end": 1.5 },
        { "type": "rotate", "object": "m", "to": [0, 90, 0], "easing_curve": "linear", "start": 2, "end": 3 }
      ] }"#;

    let checked = check_layout(&scene(&objects, timeline), &["--no-sheet"]);

    assert!(
        checked
            .stages
            .contains("t=1, middle of animation 0 (rotate of \"m\") of step 0\n"),
        "turned through 90 degrees here: {}",
        checked.stages
    );
    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=3 to 10 (end of animation 1 (rotate of "m") of step 0 to end of step 0, 3 checks): atom 3 of "m" is hidden behind atom 1: turn the molecule with `rotation` so it shows"#
        ]
    );
}

#[test]
fn a_molecule_of_more_than_200_shown_atoms_is_spared_the_hidden_atom_check() {
    let structures = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/sci-anim/examples/structures"
    );
    let protein = |file: &str| {
        format!(
            r#""m": {{ "type": "molecule", "file": "{file}", "scale": [0.07, 0.07, 0.07], "groups": {{ "helix": {{ "residues": [{{ "chain": "A", "from": 23, "to": 34 }}] }} }} }}"#
        )
    };
    let whole = check_layout(
        &scene(&protein(&format!("{structures}/1ubq.pdb")), ""),
        &["--no-sheet"],
    );

    assert_eq!(whole.exit_code, 0, "{}", whole.stdout);

    let dir = tempfile::tempdir().unwrap();
    let helix_only: String = std::fs::read_to_string(format!("{structures}/1ubq.pdb"))
        .unwrap()
        .lines()
        .filter(|l| {
            l.starts_with("ATOM") && (23..=34).contains(&l[22..26].trim().parse::<i32>().unwrap())
        })
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(dir.path().join("helix.pdb"), helix_only).unwrap();
    let alone = check_layout(
        &scene(&protein(dir.path().join("helix.pdb").to_str().unwrap()), ""),
        &["--no-sheet"],
    );

    assert_eq!(alone.exit_code, 1, "{}", alone.stdout);
    assert!(
        fault_lines(&alone)
            .iter()
            .any(|l| l.contains("of \"m\" is hidden behind atom")),
        "the helix's 98 atoms alone are few enough to turn: {}",
        alone.stdout
    );
}

#[test]
fn text_an_opacity_law_fades_on_a_covering_panel_is_judged_where_it_rests_as_after_a_fade() {
    let objects = r##""panel": { "type": "polygon", "points": [[-3, -1], [3, -1], [3, 1], [-3, 1]], "position": [0, 0, -1], "fill": { "color": "#eeeeee" } },
        "t": { "type": "text", "text": "Read me", "color": "#ffffff", "opacity": 0 }"##;
    let law = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "follow", "object": "t", "sets": "opacity", "law": { "value": "t/2" } }] }"#;
    let checked = check_layout(&transparent_scene(objects, law), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 to 1 (end of step 0 to frame at 1 of follow 0 (opacity of "t") of step 0, 2 checks): "t" is hard to read: #ffffff at object opacity 0.5 shows as #f7f7f7, and on the fill behind it, which shows as #eeeeee has contrast 1.1, and needs 4.5"#
        ]
    );
    assert!(unverified_lines(&checked).is_empty(), "{}", checked.stdout);
}

fn label_on_a_dark_background(label: &str, timeline: &str) -> String {
    scene(
        &format!(r#""label": {{ "type": "text", "text": "Faint", {label} }}"#),
        timeline,
    )
    .replace("#000000", "#101824")
}

#[test]
fn text_faded_to_0_2_and_held_is_hard_to_read_where_it_rests() {
    let fade = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "fade", "object": "label", "to": 0.2 }] },
        { "type": "wait", "duration": 1 }"#;
    let checked = check_layout(
        &label_on_a_dark_background(r##""color": "#ffffff""##, fade),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 to 2 (end of step 0 to end of step 1, 2 checks): "label" is hard to read: #ffffff at object opacity 0.2 shows as #7d7e80, and on the background #101824 has contrast 4.4, and needs 4.5"#
        ]
    );
}

#[test]
fn text_mid_fade_in_is_not_judged_at_the_faint_opacities_it_passes() {
    let fade = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "fade", "object": "label", "to": 1, "easing_curve": "linear" }] }"#;
    let gray = r##""color": "#8a8a8a", "opacity": 0"##;
    let checked = check_layout(&label_on_a_dark_background(gray, fade), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);

    let held = fade.replace(r#""to": 1"#, r#""to": 0.5"#);
    let checked = check_layout(&label_on_a_dark_background(gray, &held), &["--no-sheet"]);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (end of step 0): "label" is hard to read: #8a8a8a at object opacity 0.5 shows as #656667, and on the background #101824 has contrast 3.1, and needs 4.5"#
        ]
    );
}

#[test]
fn an_opacity_law_that_holds_text_at_0_2_is_hard_to_read_from_its_follow_s_end() {
    let law = r#"{ "type": "play", "duration": 2, "animations": [{ "type": "follow", "object": "label", "sets": "opacity", "law": { "value": "max(0.2, 1 - t)" } }] },
        { "type": "wait", "duration": 1 }"#;
    let checked = check_layout(
        &label_on_a_dark_background(r##""color": "#ffffff""##, law),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=2 to 3 (end of step 0 to end of step 1, 3 checks): "label" is hard to read: #ffffff at object opacity 0.2 shows as #7d7e80, and on the background #101824 has contrast 4.4, and needs 4.5"#
        ]
    );
}

#[test]
fn text_at_opacity_0_6_with_enough_contrast_on_a_dark_background_passes() {
    let checked = check_layout(
        &label_on_a_dark_background(
            r##""color": "#ffffff", "opacity": 0.6"##,
            r#"{ "type": "wait", "duration": 1 }"#,
        ),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert_eq!(
        checked.stdout.lines().next(),
        Some("No problem at any of 1 checked scene times.")
    );
}

#[test]
fn text_faded_to_opacity_0_is_hidden_and_not_judged() {
    let fade = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "fade", "object": "label", "to": 0 }] },
        { "type": "wait", "duration": 1 }"#;
    let checked = check_layout(
        &label_on_a_dark_background(r##""color": "#ffffff""##, fade),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert_eq!(
        checked.stdout.lines().next(),
        Some("No problem at any of 4 checked scene times.")
    );
}

#[test]
fn text_in_a_corner_of_a_circle_s_box_off_its_disc_is_read_against_the_background() {
    let objects = |position: &str| {
        format!(
            r##""disc": {{ "type": "circle", "radius": 1.5, "fill": {{ "color": "#eeeeee" }} }},
        "t": {{ "type": "text", "text": "x", "color": "#ffffff", "size": 0.3, "position": {position} }}"##
        )
    };
    let wait = r#"{ "type": "wait", "duration": 1 }"#;

    let corner = check_layout(&scene(&objects("[1.3, 1.3, 0]"), wait), &["--no-sheet"]);
    assert_eq!(corner.exit_code, 0, "{}", corner.stdout);

    let center = check_layout(&scene(&objects("[0, 0, 0]"), wait), &["--no-sheet"]);
    assert_eq!(
        fault_lines(&center),
        [
            r#"t=1 (end of step 0): "t" is hard to read: #ffffff on the fill behind it, which shows as #eeeeee has contrast 1.2, and needs 4.5"#
        ]
    );
}

#[test]
fn a_text_covers_an_atom_whose_ball_reaches_its_box_though_its_center_lies_outside() {
    let objects = |label_y: f32| {
        format!(
            r#""m": {{ "type": "molecule", "file": "{FIXTURES}/hydrogen-chloride.sdf", "scale": [2, 2, 2] }},
        "label": {{ "type": "text", "text": "HCl", "size": 0.3, "position": [0, {label_y}, 0] }}"#
        )
    };
    let wait = r#"{ "type": "wait", "duration": 1 }"#;

    let over = check_layout(&scene(&objects(1.0), wait), &["--no-sheet"]);
    assert_eq!(
        fault_lines(&over),
        [r#"t=1 (end of step 0): "label" covers atoms [1] of "m""#],
        "the chlorine's ball, of radius 49 pixels at scale 2, reaches the label's box from 22 pixels left of it and 32 below"
    );

    let clear = check_layout(&scene(&objects(2.0), wait), &["--no-sheet"]);
    assert_eq!(clear.exit_code, 0, "{}", clear.stdout);
}

#[test]
fn a_ball_whose_disc_reaches_a_reserved_region_is_in_it_though_its_center_lies_outside() {
    let objects = format!(
        r#""m": {{ "type": "molecule", "file": "{FIXTURES}/hydrogen-chloride.sdf", "scale": [2, 2, 2] }}"#
    );
    let scene = scene(&objects, r#"{ "type": "wait", "duration": 1 }"#);

    let reached = check_layout(&scene, &["--no-sheet", "--reserve", "0,0,150,400"]);
    assert_eq!(
        fault_lines(&reached),
        [
            r#"t=1 (end of step 0): "m" has atoms [1] in the reserved region [0, 0] to [150, 400] pixels"#
        ],
        "the chlorine's center is 17 pixels right of the region and its ball's radius is 49"
    );

    let clear = check_layout(&scene, &["--no-sheet", "--reserve", "0,0,110,400"]);
    assert_eq!(clear.exit_code, 0, "{}", clear.stdout);
}

#[test]
fn a_follow_of_opacity_raises_an_object_declared_at_opacity_0() {
    let objects = r#""eq": { "type": "math", "tex": "x^2", "size": 0.6, "opacity": 0 }"#;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "write", "object": "eq" }] },
        { "type": "play", "duration": 1, "animations": [{ "type": "follow", "object": "eq", "sets": "opacity", "law": { "value": "t" } }] }"#;

    let checked = check_layout(&scene(objects, timeline), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
}

#[test]
fn a_step_that_fades_everything_out_at_its_end_puts_the_completed_diagram_on_the_sheet() {
    let objects = r##""a": { "type": "text", "text": "A" }, "c": { "type": "circle", "radius": 1, "position": [0, 2, 0], "fill": { "color": "#58c4dd" } }"##;
    let timeline = r#"{ "type": "play", "duration": 3, "animations": [
        { "type": "fade", "object": "a", "to": 0, "start": 2 },
        { "type": "fade", "object": "c", "to": 0, "start": 2 }
      ] }"#;

    let checked = check_layout(&scene(objects, timeline), &[]);

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    let times: Vec<&str> = checked
        .stages
        .lines()
        .filter(|line| line.starts_with("t="))
        .collect();
    assert_eq!(
        times,
        [
            "t=1.5, middle of step 0, frame-000.png",
            "t=2, before the fades of step 0, frame-001.png",
            "t=2.5, middle of animation 0 (fade of \"a\") of step 0",
            "t=3, end of step 0, frame-002.png",
        ]
    );
}

const SUBTITLES: &str = "0,320,400,400";

fn math_at(position: &str) -> String {
    scene(
        &format!(r#""eq": {{ "type": "math", "tex": "E = mc^2", "position": {position} }}"#),
        r#"{ "type": "wait", "duration": 1 }"#,
    )
}

#[test]
fn math_in_a_reserved_region_is_reported() {
    let checked = check_layout(
        &math_at("[0, -3, 0]"),
        &["--no-sheet", "--reserve", SUBTITLES],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (end of step 0): "eq" is in the reserved region [0, 320] to [400, 400] pixels: its box runs from [144, 333] to [254, 357] pixels"#
        ]
    );
}

#[test]
fn math_moved_above_a_reserved_region_passes() {
    let checked = check_layout(
        &math_at("[0, -2, 0]"),
        &["--no-sheet", "--reserve", SUBTITLES],
    );

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert_eq!(
        checked.stdout.lines().next(),
        Some("No problem at any of 1 checked scene times.")
    );
}

#[test]
fn an_exempt_object_may_sit_in_a_reserved_region() {
    let checked = check_layout(
        &math_at("[0, -3, 0]"),
        &[
            "--no-sheet",
            "--reserve",
            SUBTITLES,
            "--reserve-exempt",
            "eq",
        ],
    );

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert_eq!(
        checked.stdout.lines().next(),
        Some("No problem at any of 1 checked scene times.")
    );
}

#[test]
fn a_text_moved_through_a_reserved_region_over_an_early_span_is_reported_at_that_span() {
    let checked = check_layout(
        &ten_second_step_moving_b_through("[-3, -3, 0]"),
        &["--no-sheet", "--reserve", SUBTITLES],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1.5 (end of animation 0 (move of "b") of step 0): "b" is in the reserved region [0, 320] to [400, 400] pixels: its box runs from [49, 336] to [62, 354] pixels"#
        ]
    );
}

#[test]
fn a_reserved_region_outside_the_frame_or_with_its_min_past_its_max_is_a_usage_error() {
    for (region, message) in [
        (
            "0,320,401,400",
            "--reserve [0, 320] to [401, 400] runs outside the frame, which is 400 by 400 pixels",
        ),
        (
            "0,400,400,320",
            "0,400,400,320 needs X0 less than X1 and Y0 less than Y1",
        ),
        ("0,320,400", "0,320,400 is not four numbers X0,Y0,X1,Y1"),
    ] {
        let checked = check_layout(&math_at("[0, 0, 0]"), &["--no-sheet", "--reserve", region]);

        assert_eq!(checked.exit_code, 2, "{}", checked.stdout);
        assert_eq!(checked.stdout, "", "stdout keeps to layout faults");
        let report: serde_json::Value = serde_json::from_str(&checked.stderr)
            .unwrap_or_else(|e| panic!("stderr is one error report: {e}\n{}", checked.stderr));
        assert_eq!(report["errors"][0]["kind"], "usage", "{report}");
        assert!(
            report["errors"][0]["message"]
                .as_str()
                .unwrap()
                .contains(message),
            "{report}"
        );
    }
}

/// Math in the subtitles region for a 1-second wait, then fading out over a 2-second step: it
/// is checked at scene times 1, 2 (half faded) and 3 (faded out).
fn math_fading_out_in_the_subtitles_region() -> String {
    scene(
        r#""eq": { "type": "math", "tex": "E = mc^2", "position": [0, -3, 0] }"#,
        r#"{ "type": "wait", "duration": 1 },
        { "type": "play", "duration": 2, "animations": [{ "type": "fade", "object": "eq", "to": 0 }] }"#,
    )
}

#[test]
fn a_reserved_region_with_a_span_reports_an_object_inside_the_span_and_not_outside_it() {
    let fading = math_fading_out_in_the_subtitles_region();
    let whole_scene = check_layout(&fading, &["--no-sheet", "--reserve", SUBTITLES]);
    let during_the_fade = check_layout(&fading, &["--no-sheet", "--reserve", "0,320,400,400@2-3"]);
    let before_it_shows =
        check_layout(&fading, &["--no-sheet", "--reserve", "0,320,400,400@0-0.5"]);

    assert_eq!(
        fault_lines(&whole_scene),
        [
            r#"t=1 to 2 (end of step 0 to middle of step 1, 2 checks): "eq" is in the reserved region [0, 320] to [400, 400] pixels: its box runs from [144, 333] to [254, 357] pixels"#
        ]
    );
    assert_eq!(during_the_fade.exit_code, 1, "{}", during_the_fade.stdout);
    assert_eq!(
        fault_lines(&during_the_fade),
        [
            r#"t=2 (middle of step 1): "eq" is in the reserved region [0, 320] to [400, 400] pixels, from scene time 2 to 3: its box runs from [144, 333] to [254, 357] pixels"#
        ],
        "half faded at 2 it still shows, and at 3 it has faded out"
    );
    assert_eq!(before_it_shows.exit_code, 0, "{}", before_it_shows.stdout);
    assert_eq!(
        before_it_shows.stdout.lines().next(),
        Some("No problem at any of 3 checked scene times.")
    );
}

#[test]
fn a_span_written_to_the_scene_end_reaches_steps_whose_durations_sum_a_hair_off_it() {
    // 0.1 + 0.2 sums to a hair over 0.3, and 0.7 + 0.1 to a hair under 0.8.
    for (first, second, start, end) in [("0.1", "0.2", "0.2", "0.3"), ("0.7", "0.1", "0.75", "0.8")]
    {
        let checked = check_layout(
            &scene(
                r#""eq": { "type": "math", "tex": "E = mc^2", "position": [0, -3, 0] }"#,
                &format!(
                    r#"{{ "type": "wait", "duration": {first} }}, {{ "type": "wait", "duration": {second} }}"#
                ),
            ),
            &[
                "--no-sheet",
                "--reserve",
                &format!("0,320,400,400@{start}-{end}"),
            ],
        );

        assert_eq!(checked.exit_code, 1, "{}", checked.stderr);
        assert_eq!(
            fault_lines(&checked),
            [format!(
                r#"t={end} (end of step 1): "eq" is in the reserved region [0, 320] to [400, 400] pixels, from scene time {start} to {end}: its box runs from [144, 333] to [254, 357] pixels"#
            )]
        );
    }
}

#[test]
fn every_scene_time_check_layout_writes_is_one_inspect_and_frame_take_back() {
    // Six significant digits round 51.995999 up to 51.996, past the scene's end.
    let lesson = scene(
        r##""disc": { "type": "circle", "radius": 0.5, "position": [-2, 0, 0], "fill": { "color": "#3fa7ff" } },
        "edge": { "type": "text", "text": "Cut off", "color": "#ffffff", "position": [6, 2, 0] }"##,
        r#"{ "type": "play", "duration": 51.995999, "animations": [ { "type": "move", "object": "disc", "to": [2, 0, 0] } ] }"#,
    );
    let checked = check_layout(&lesson, &["--no-sheet"]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stderr);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=0 to 51.995999 (start of animation 0 (move of "disc") of step 0 to end of step 0, 3 checks): "edge" is cut off by the frame: its box runs from [449, 92] to [533, 113] pixels"#
        ]
    );
    let stage_lines: Vec<&str> = checked
        .stages
        .lines()
        .filter(|line| line.starts_with("t="))
        .collect();
    assert_eq!(
        stage_lines,
        [
            r#"t=0, start of animation 0 (move of "disc") of step 0"#,
            "t=25.998, middle of step 0",
            "t=51.995999, end of step 0",
        ]
    );
    let (_dir, path) = scene_file(&lesson);
    let png = checked.out.join("end.png");
    let inspected = sci_anim(&[
        "inspect",
        path.to_str().unwrap(),
        "--scene-time",
        "51.995999",
    ]);
    let drawn = sci_anim(&[
        "frame",
        path.to_str().unwrap(),
        "--scene-time",
        "51.995999",
        "-o",
        png.to_str().unwrap(),
    ]);
    assert_eq!(
        (
            inspected.exit_code,
            inspected.json["objects"]["disc"]["position"].clone(),
            drawn.exit_code
        ),
        (0, serde_json::json!([2.0, 0.0, 0.0]), 0),
        "{}{}",
        inspected.stdout,
        drawn.stdout
    );
}

#[test]
fn a_reserved_region_with_end_before_start_or_outside_the_scene_is_a_usage_error() {
    for (region, message) in [
        ("0,320,400,400@3-2", "@3-2 needs START less than END"),
        (
            "0,320,400,400@0-4",
            "--reserve from scene time 0 to 4 runs outside the scene, which runs from 0 to 3 seconds",
        ),
        (
            "0,320,400,400@-1-2",
            "--reserve from scene time -1 to 2 runs outside the scene, which runs from 0 to 3 seconds",
        ),
        (
            "0,320,400,400@2",
            "@2 is not a span of scene time START-END in seconds",
        ),
    ] {
        let checked = check_layout(
            &math_fading_out_in_the_subtitles_region(),
            &["--no-sheet", "--reserve", region],
        );

        assert_eq!(checked.exit_code, 2, "{region}: {}", checked.stdout);
        assert_eq!(checked.stdout, "", "stdout keeps to layout faults");
        let report: serde_json::Value = serde_json::from_str(&checked.stderr)
            .unwrap_or_else(|e| panic!("stderr is one error report: {e}\n{}", checked.stderr));
        assert_eq!(report["errors"][0]["kind"], "usage", "{report}");
        assert!(
            report["errors"][0]["message"]
                .as_str()
                .unwrap()
                .contains(message),
            "{report}"
        );
    }
}

#[test]
fn the_contact_sheet_shows_a_reserved_region_only_on_tiles_inside_its_span() {
    let checked = check_layout(
        &math_fading_out_in_the_subtitles_region(),
        &["--reserve", "50,300,350,380@2-3"],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    let sheet = checked.out.join("contact-sheet-1.png");
    // Tiles are 480 pixels wide with 6 between, so the box's top edge at y 300 lands at 361 on each.
    let top_edge_on_tile = |tile: usize| png_pixel(&sheet, 240 + tile * 486, 361);
    assert_eq!(top_edge_on_tile(0), [0, 0, 0, 255], "at scene time 1");
    assert_eq!(top_edge_on_tile(1), [255, 0, 255, 255], "at scene time 2");
    assert_eq!(top_edge_on_tile(2), [255, 0, 255, 255], "at scene time 3");
}

#[test]
fn a_scene_file_that_cannot_be_checked_prints_its_error_report_on_stderr_and_exits_2() {
    let checked = check_layout(
        &scene(
            r#""c": { "type": "circle", "radius": -1 }"#,
            r#"{ "type": "wait", "duration": 1 }"#,
        ),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 2, "{}", checked.stderr);
    assert_eq!(checked.stdout, "");
    let report: serde_json::Value = serde_json::from_str(&checked.stderr).unwrap();
    assert_eq!(
        (
            report["status"].as_str(),
            report["errors"][0]["kind"].as_str(),
            report["errors"][0]["path"].as_str()
        ),
        (Some("error"), Some("scene"), Some("/objects/c/radius"))
    );
}

/// One RGBA pixel of a PNG of any size.
fn png_pixel(png: &Path, x: usize, y: usize) -> [u8; 4] {
    let width = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(png).unwrap()))
        .read_info()
        .unwrap()
        .info()
        .width as usize;
    let start = (y * width + x) * 4;
    read_rgba(png)[start..start + 4].try_into().unwrap()
}

#[test]
fn the_contact_sheet_shows_a_reserved_region_and_the_full_size_frame_does_not() {
    let checked = check_layout(&math_at("[0, 2, 0]"), &["--reserve", "50,300,350,380"]);

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    let sheet = checked.out.join("contact-sheet-1.png");
    // A 400-pixel frame shrinks to a 480-pixel tile, so the box's top edge at y 300 lands at 360 and its left edge at x 50 at 60.
    assert_eq!(png_pixel(&sheet, 240, 361), [255, 0, 255, 255], "top edge");
    assert_eq!(png_pixel(&sheet, 61, 400), [255, 0, 255, 255], "left edge");
    assert_eq!(
        png_pixel(&sheet, 240, 400),
        [0, 0, 0, 255],
        "inside the region"
    );
    assert_eq!(
        png_pixel(&checked.out.join("frame-000.png"), 200, 301),
        [0, 0, 0, 255],
        "the full-size frame at the top edge"
    );
}

#[test]
fn atoms_a_reaction_moves_through_a_reserved_region_are_reported_mid_reaction() {
    let molecule = |id: &str, sdf: &str, position: &str| {
        format!(
            r#""{id}": {{ "type": "molecule", "file": "{FIXTURES}/{sdf}.sdf", "position": {position}, "scale": [0.5, 0.5, 0.5] }}"#
        )
    };
    let objects = [
        molecule("ethylene", "ethylene", "[-2.6, 1.2, 0]"),
        molecule("hcl", "hydrogen-chloride", "[-2.6, -1.2, 0]"),
        molecule("chloroethane", "chloroethane", "[2.4, 0, 0]"),
    ]
    .join(", ");
    let atom_map: Vec<String> = [("ethylene", 1, 2), ("ethylene", 2, 3), ("ethylene", 3, 4), ("ethylene", 4, 5), ("ethylene", 5, 6), ("ethylene", 6, 7), ("hcl", 1, 1), ("hcl", 2, 8)]
        .iter()
        .map(|(reactant, atom, product_atom)| format!(r#"{{ "reactant": {{ "object": "{reactant}", "atom": {atom} }}, "product": {{ "object": "chloroethane", "atom": {product_atom} }} }}"#))
        .collect();
    let timeline = format!(
        r#"{{ "type": "play", "duration": 2, "animations": [{{ "type": "reaction", "easing_curve": "linear",
            "reactants": ["ethylene", "hcl"], "products": ["chloroethane"], "atom_map": [{}] }}] }}"#,
        atom_map.join(", ")
    );

    // The reactants sit left of the strip and the product right of it; only atoms in transit cross it, each by its ball: mid-reaction a carbon, two hydrogens and the chlorine have their centers just outside it and their discs in it.
    let checked = check_layout(
        &scene(&objects, &timeline),
        &["--no-sheet", "--reserve", "180,0,220,400"],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (middle of step 0): in reaction 0, "ethylene" has atoms [1, 2, 5, 6] in the reserved region [180, 0] to [220, 400] pixels"#,
            r#"t=1 (middle of step 0): in reaction 0, "hcl" has atoms [1, 2] in the reserved region [180, 0] to [220, 400] pixels"#,
        ]
    );
}

#[test]
fn a_line_thinner_than_the_tolerance_wholly_inside_a_reserved_region_is_reported() {
    let rule = scene(
        r#""rule": { "type": "line", "from": [-2, -3], "to": [2, -3] }"#,
        r#"{ "type": "wait", "duration": 1 }"#,
    );

    let checked = check_layout(&rule, &["--no-sheet", "--reserve", SUBTITLES]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (end of step 0): "rule" is in the reserved region [0, 320] to [400, 400] pixels: its box runs from [103, 344] to [297, 346] pixels"#
        ]
    );
}

#[test]
fn a_linear_map_that_carries_a_text_onto_another_is_reported_at_its_end() {
    let objects = r##""a": { "type": "text", "text": "A" }, "b": { "type": "text", "text": "B", "position": [-1.5, 0, 0] }"##;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "linear-map", "objects": ["b"], "matrix": [[2, 0], [0, 2]], "origin": [-3, 0] }] }"#;

    let checked = check_layout(&scene(objects, timeline), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    let lines = fault_lines(&checked);
    assert_eq!(lines.len(), 1, "{}", checked.stdout);
    assert!(
        lines[0].starts_with(r#"t=1 (end of step 0): "a" and "b" overlap by "#),
        "{}",
        lines[0]
    );
}

#[test]
fn a_text_a_linear_map_shrinks_below_the_smallest_em_is_too_small_at_its_end() {
    let objects = r##""t": { "type": "text", "text": "Hello", "size": 0.6 }"##;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "linear-map", "objects": ["t"], "matrix": [[0.1, 0], [0, 0.1]] }] }"#;

    let checked = check_layout(&scene(objects, timeline), &["--no-sheet"]);

    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (end of step 0): "t" is too small to read: its em is 3 pixels in a frame 400 pixels high, and needs 14"#
        ],
        "0.6 times 0.1 is 0.06 units, 2.9 pixels; halfway, at 0.55 times, it is 15.9: {}",
        checked.stdout
    );
}

#[test]
fn a_text_a_linear_map_flattens_across_its_baseline_is_too_small_and_a_sheared_one_is_not() {
    let objects = r##""flat": { "type": "text", "text": "Hi", "size": 0.6, "position": [0, 1.5, 0] }, "slanted": { "type": "text", "text": "Hi", "size": 0.6, "position": [0, -1.5, 0] }"##;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [
      { "type": "linear-map", "objects": ["flat"], "matrix": [[1, 1], [0.1, 0.12]] },
      { "type": "linear-map", "objects": ["slanted"], "matrix": [[1, 1], [0, 1]] }
    ] }"#;

    let checked = check_layout(&scene(objects, timeline), &["--no-sheet"]);

    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (end of step 0): "flat" is too small to read: its em is 1 pixels in a frame 400 pixels high, and needs 14"#
        ],
        "an em stands 0.6 times 0.02 / 1.005 units across the baseline, where the determinant is 0.02 and the baseline 1.005 long, though the up direction is 1.007 long: {}",
        checked.stdout
    );
}

/// A dot carried once round a circle of radius 3 over a 10 second step. It passes the top of the
/// circle at scene time 2.5, between the step's start, middle and end.
fn dot_round_a_circle() -> String {
    let objects = r##""ring": { "type": "circle", "radius": 3, "stroke": { "color": "#888888" } }, "p": { "type": "dot", "radius": 0.1, "position": [3, 0, 0] }"##;
    scene(
        objects,
        r#"{ "type": "play", "duration": 10, "animations": [{ "type": "follow", "object": "p", "along": "ring", "easing_curve": "linear" }] }"#,
    )
}

#[test]
fn a_carry_through_a_reserved_region_between_samples_is_reported_at_its_frame_times() {
    let checked = check_layout(
        &dot_round_a_circle(),
        &[
            "--no-sheet",
            "--reserve",
            "180,30,220,80",
            "--reserve-exempt",
            "ring",
        ],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    let lines = fault_lines(&checked);
    assert_eq!(lines.len(), 1, "{}", checked.stdout);
    assert_eq!(
        lines[0],
        r#"t=2.26667 to 2.73333 (frame at 2.26667 of follow 0 ("p" along "ring") of step 0 to frame at 2.73333 of follow 0 ("p" along "ring") of step 0, 29 checks): "p" is in the reserved region [180, 30] to [220, 80] pixels: its box runs from [216, 52] to [226, 62] pixels"#,
        "{}",
        checked.stdout
    );
    assert!(
        !checked.stages.contains("frame at"),
        "a follow's frame times stay out of stages.txt: {}",
        checked.stages
    );
}

#[test]
fn every_frame_time_of_a_follow_is_checked_unless_ends_only() {
    let passes = check_layout(&dot_round_a_circle(), &["--no-sheet"]);
    let ends_only = check_layout(
        &dot_round_a_circle(),
        &[
            "--no-sheet",
            "--ends-only",
            "--reserve",
            "180,30,220,80",
            "--reserve-exempt",
            "ring",
        ],
    );

    assert_eq!(
        passes.stdout.lines().next(),
        Some("No problem at any of 3 checked scene times and 601 frame times of follows."),
        "the step's middle, its end, and the follow's start: {}",
        passes.stdout
    );
    assert_eq!(ends_only.exit_code, 0, "{}", ends_only.stdout);
    assert_eq!(
        ends_only.stdout.lines().next(),
        Some("No problem at any of 1 checked scene times.")
    );
}

#[test]
fn follows_that_run_at_once_check_each_frame_time_once() {
    let objects = r##""ring": { "type": "circle", "radius": 3, "stroke": { "color": "#888888" } }, "p": { "type": "dot", "radius": 0.1, "position": [3, 0, 0] }, "q": { "type": "dot", "radius": 0.1, "position": [3, 0, 0] }"##;
    let follow = |object: &str| {
        format!(
            r#"{{ "type": "follow", "object": "{object}", "along": "ring", "easing_curve": "linear" }}"#
        )
    };
    let two_follows = scene(
        objects,
        &format!(
            r#"{{ "type": "play", "duration": 10, "animations": [{}, {}] }}"#,
            follow("p"),
            follow("q")
        ),
    );

    let passes = check_layout(&two_follows, &["--no-sheet"]);
    let reserved = check_layout(
        &two_follows,
        &[
            "--no-sheet",
            "--reserve",
            "180,30,220,80",
            "--reserve-exempt",
            "ring",
            "--reserve-exempt",
            "q",
        ],
    );

    assert_eq!(
        passes.stdout.lines().next(),
        Some("No problem at any of 3 checked scene times and 601 frame times of follows."),
        "{}",
        passes.stdout
    );
    assert_eq!(
        fault_lines(&reserved),
        [
            r#"t=2.26667 to 2.73333 (frame at 2.26667 of follow 0 ("p" along "ring") of step 0 to frame at 2.73333 of follow 0 ("p" along "ring") of step 0, 29 checks): "p" is in the reserved region [180, 30] to [220, 80] pixels: its box runs from [216, 52] to [226, 62] pixels"#
        ],
        "{}",
        reserved.stdout
    );
}

/// Five discs 0.6 across in a row at x = -3, -1.5, 0, 1.5 and 3, filled `#eeeeee`, and a white
/// text at `text_x`.
fn row_and_text(text_x: f32) -> String {
    let objects = format!(
        r##""row": {{ "type": "pattern", "shape": {{ "type": "circle", "radius": 0.3, "fill": {{ "color": "#eeeeee" }} }},
            "layout": {{ "type": "grid", "counts": [5, 1, 1], "spacing": 1.5 }} }},
        "t": {{ "type": "text", "text": "A", "size": 0.4, "color": "#ffffff", "position": [{text_x}, 0, 0] }}"##
    );
    scene(&objects, r#"{ "type": "wait", "duration": 1 }"#)
}

#[test]
fn a_moved_copy_is_checked_where_it_has_moved() {
    let objects = r##""row": { "type": "pattern", "shape": { "type": "circle", "radius": 0.3, "fill": { "color": "#eeeeee" } },
            "layout": { "type": "grid", "counts": [5, 1, 1], "spacing": 1.5 } },
        "t": { "type": "text", "text": "A", "size": 0.4, "color": "#ffffff", "position": [0, 2, 0] }"##;
    let moved = scene(
        objects,
        r#"{ "type": "play", "duration": 1, "animations": [ { "type": "move", "object": "row", "copy": [2, 0, 0], "to": [0, 2, 0] } ] }"#,
    );
    let checked = check_layout(&moved, &["--no-sheet"]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (end of step 0): "t" is hard to read: #ffffff on copy [2, 0, 0] of "row" behind it, which shows as #eeeeee has contrast 1.2, and needs 4.5"#
        ]
    );
}

#[test]
fn a_text_between_the_copies_of_a_pattern_passes() {
    let checked = check_layout(&row_and_text(0.75), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert!(
        checked
            .stages
            .contains(r#"row (pattern): "5 copies of circle in a grid""#),
        "{}",
        checked.stages
    );
}

#[test]
fn a_text_on_a_copy_is_read_against_that_copy() {
    let checked = check_layout(&row_and_text(0.0), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (end of step 0): "t" is hard to read: #ffffff on copy [2, 0, 0] of "row" behind it, which shows as #eeeeee has contrast 1.2, and needs 4.5"#
        ]
    );
}

#[test]
fn a_text_across_copies_names_them() {
    let checked = check_layout(
        &row_and_text(0.0).replace(r#""text": "A""#, r#""text": "A much wider label""#),
        &["--no-sheet"],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert!(
        fault_lines(&checked).contains(
            &r#"t=1 (end of step 0): "t" overlaps copies [1, 0, 0], [2, 0, 0], [3, 0, 0] of "row""#
        ),
        "{}",
        checked.stdout
    );
}

#[test]
fn a_reserved_region_holds_the_copies_that_enter_it() {
    let checked = check_layout(
        &row_and_text(0.75),
        &["--no-sheet", "--reserve", "320,170,399,230"],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (end of step 0): "row" has copies [4, 0, 0] in the reserved region [320, 170] to [399, 230] pixels"#
        ]
    );
}

#[test]
fn a_text_over_a_solid_is_read_against_its_lit_color() {
    let objects = r##""ball": { "type": "sphere", "radius": 1.2, "material": { "type": "standard", "color": "#eeeeee" } },
        "t": { "type": "text", "text": "Na", "size": 0.4, "color": "#ffffff", "position": [0, 0, 1.5] }"##;
    let checked = check_layout(
        &scene(objects, r#"{ "type": "wait", "duration": 1 }"#),
        &["--no-sheet", "--allow", "t,ball"],
    );

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            r#"t=1 (end of step 0): "t" is hard to read: #ffffff on the solid behind it, which shows as #eeeeee has contrast 1.2, and needs 4.5"#,
            r#"t=1 (end of step 0): "t" is hard to read: #ffffff on the solid behind it, which shows as #c7c7c7 has contrast 1.7, and needs 4.5"#,
        ]
    );
}

/// 3D axes `a`, x from -3 to 3, y and z from 0 to 4, seen from the default camera, and a white
/// text at `text_position`.
fn axes_3d_and_text(text_position: &str) -> String {
    let objects = format!(
        r##""a": {{ "type": "3d-axes", "x": {{ "range": [-3, 3], "length": 6 }}, "y": {{ "range": [0, 4], "length": 2 }}, "z": {{ "range": [0, 4], "length": 4, "label": "z" }} }},
        "t": {{ "type": "text", "text": "A", "size": 0.3, "color": "#ffffff", "position": {text_position} }}"##
    );
    scene(&objects, r#"{ "type": "wait", "duration": 1 }"#)
}

#[test]
fn a_text_inside_the_box_of_3d_axes_passes_and_stages_lists_their_tick_labels() {
    let checked = check_layout(&axes_3d_and_text("[1.5, 0.5, 0]"), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
    assert!(checked.stages.contains(r#"a (3d-axes): "x tick labels -3, -2, -1, 1, 2, 3; y tick labels 1, 2, 3, 4; z tick labels 1, 2, 3, 4""#), "{}", checked.stages);
}

#[test]
fn a_text_on_a_tick_label_of_3d_axes_names_it() {
    let checked = check_layout(&axes_3d_and_text("[2, -2.33, 1]"), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [r#"t=1 (end of step 0): "t" covers the tick label 2 of "a""#]
    );
}

#[test]
fn math_written_at_opacity_0_and_never_faded_in_is_reported() {
    let objects = r#""eq": { "type": "math", "tex": "x^2", "size": 0.6, "opacity": 0 }"#;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "write", "object": "eq" }] }, { "type": "wait", "duration": 1 }"#;

    let checked = check_layout(&scene(objects, timeline), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 1, "{}", checked.stdout);
    assert_eq!(
        fault_lines(&checked),
        [
            "t=1 (end of the write of \"eq\" in step 0): \"eq\" never shows: it is declared at opacity 0, which a write does not raise, and no fade or follow of its opacity raises it"
        ]
    );
}

#[test]
fn a_morph_target_at_opacity_0_and_never_faded_in_is_reported() {
    let objects = r#""a": { "type": "math", "tex": "x+1", "size": 0.6 }, "b": { "type": "math", "tex": "x+2", "size": 0.6, "opacity": 0 }"#;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "morph", "object": "a", "into": "b" }] }, { "type": "wait", "duration": 1 }"#;

    let checked = check_layout(&scene(objects, timeline), &["--no-sheet"]);

    assert_eq!(
        fault_lines(&checked),
        [
            "t=1 (end of the morph into \"b\" in step 0): \"b\" never shows: it is declared at opacity 0, which a morph does not raise, and no fade or follow of its opacity raises it"
        ]
    );
}

#[test]
fn text_written_at_opacity_0_and_faded_in_later_passes() {
    let objects = r#""eq": { "type": "math", "tex": "x^2", "size": 0.6, "opacity": 0 }"#;
    let timeline = r#"{ "type": "play", "duration": 1, "animations": [{ "type": "write", "object": "eq" }] },
        { "type": "play", "duration": 1, "animations": [{ "type": "fade", "object": "eq", "to": 1 }] }"#;

    let checked = check_layout(&scene(objects, timeline), &["--no-sheet"]);

    assert_eq!(checked.exit_code, 0, "{}", checked.stdout);
}

/// A hand-written structure file of loose atoms with no bonds, each `(x, y, element)` at z 0.
fn loose_atoms(dir: &Path, file: &str, atoms: &[(f32, f32, &str)]) -> PathBuf {
    let lines: Vec<String> = atoms
        .iter()
        .map(|(x, y, element)| {
            format!(
                "{x:10.4}{y:10.4}{:10.4} {element:<3} 0  0  0  0  0  0  0  0  0  0  0  0",
                0.0
            )
        })
        .collect();
    let text = format!(
        "loose atoms\n  sci_anim-hand-made-3D\n\n{:3}  0  0     0  0  0  0  0  0999 V2000\n{}\nM  END\n$$$$\n",
        atoms.len(),
        lines.join("\n")
    );
    let path = dir.join(file);
    std::fs::write(&path, text).unwrap();
    path
}

#[test]
fn unbonded_atoms_whose_balls_rest_nearly_touching_read_as_bonded_and_bonded_ones_do_not() {
    let dir = tempfile::tempdir().unwrap();
    let pair = loose_atoms(
        dir.path(),
        "pair.sdf",
        &[(-0.515, 0.0, "O"), (0.515, 0.0, "O"), (0.0, 3.0, "C")],
    );
    let one = loose_atoms(dir.path(), "one.sdf", &[(0.0, 0.0, "N")]);
    let objects = format!(
        r##""loose": {{ "type": "molecule", "file": "{}" }}, "beside": {{ "type": "molecule", "file": "{}", "position": [0, 3.025, 0] }}, "water": {{ "type": "molecule", "file": "{FIXTURES}/water.sdf", "position": [0, -3, 0] }}"##,
        pair.display(),
        one.display()
    );

    let checked = check_layout(
        &scene(&objects, r#"{ "type": "wait", "duration": 1 }"#),
        &["--no-sheet"],
    );

    let bonded: Vec<&str> = fault_lines(&checked)
        .into_iter()
        .filter(|line| line.contains("read as bonded"))
        .collect();
    assert_eq!(
        bonded,
        [
            r#"t=1 (end of step 0): atom 1 of "beside" and atom 3 of "loose" have no bond, and their balls rest 0.20 angstroms apart, so they read as bonded: move them until their balls are at least 0.35 angstroms apart"#,
            r#"t=1 (end of step 0): atom 1 of "loose" and atom 2 of "loose" have no bond, and their balls rest 0.26 angstroms apart, so they read as bonded: move them until their balls are at least 0.35 angstroms apart"#,
        ],
        "water's O-H balls, 0.27 apart, share a bond: {}",
        checked.stdout
    );
}

#[test]
fn a_space_filling_molecule_is_judged_by_its_balls_discs_not_its_hidden_atoms_or_touching_balls() {
    let dir = tempfile::tempdir().unwrap();
    let pair = loose_atoms(
        dir.path(),
        "pair.sdf",
        &[(-0.515, 0.0, "O"), (0.515, 0.0, "O")],
    );
    let objects = |representation: &str| {
        format!(
            r##""ethanol": {{ "type": "molecule", "file": "{FIXTURES}/ethanol.sdf", "rotation": [0, 90, 0], "representation": "{representation}", "groups": {{ "heavy": [1, 2, 3] }} }},
            "pair": {{ "type": "molecule", "file": "{}", "position": [0, -2.2, 0], "representation": "{representation}" }},
            "label": {{ "type": "text", "text": "ethanol", "size": 0.3, "position": [0, 2.2, 0] }}"##,
            pair.display()
        )
    };
    let wait = r#"{ "type": "wait", "duration": 1 }"#;

    let sticks = check_layout(&scene(&objects("ball-and-stick"), wait), &["--no-sheet"]);
    let filled = check_layout(&scene(&objects("space-filling"), wait), &["--no-sheet"]);

    assert_eq!(
        fault_lines(&sticks),
        [
            r#"t=1 (end of step 0): atom 3 of "ethanol" is hidden behind atom 1: turn the molecule with `rotation` so it shows"#,
            r#"t=1 (end of step 0): atom 1 of "pair" and atom 2 of "pair" have no bond, and their balls rest 0.26 angstroms apart, so they read as bonded: move them until their balls are at least 0.35 angstroms apart"#,
        ]
    );
    assert_eq!(
        fault_lines(&filled),
        [r#"t=1 (end of step 0): "label" covers atoms [1, 9] of "ethanol""#],
        "the oxygen and its hydrogen at their van der Waals radii reach the label, and the two loose oxygens overlap"
    );
}

#[test]
fn a_text_over_a_cartoon_is_judged_by_the_cartoons_box() {
    let structures = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/sci-anim/examples/structures"
    );
    let objects = |y: f64| {
        format!(
            r#""protein": {{ "type": "molecule", "file": "{structures}/1ubq.pdb", "representation": "cartoon", "scale": [0.12, 0.12, 0.12] }},
            "label": {{ "type": "text", "text": "Ubiquitin", "size": 0.5, "position": [0, {y}, 2] }}"#
        )
    };
    let wait = r#"{ "type": "wait", "duration": 1 }"#;

    let over = check_layout(&scene(&objects(0.0), wait), &["--no-sheet"]);
    let clear = check_layout(&scene(&objects(-3.0), wait), &["--no-sheet"]);

    assert_eq!(
        fault_lines(&over).len(),
        1,
        "the label sits on the ribbons: {}",
        over.stdout
    );
    assert!(
        fault_lines(&over)[0].contains(r#""label" and "protein" overlap by"#)
            || fault_lines(&over)[0].contains(r#""protein" and "label" overlap by"#),
        "{}",
        over.stdout
    );
    assert_eq!(clear.exit_code, 0, "{}", clear.stdout);
}
