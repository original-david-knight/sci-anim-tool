//! Runs `sci_anim frame` with and without `--gpu` on whatever GPUs the machine has, and checks
//! that the test helper runs a drawing command under dibs only when it is installed.

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

/// Runs the binary through the test helper with the arguments in `RUN_THROUGH_THE_HELPER`, one a
/// line, on a scene file that is not there. Only `through_the_helper` runs it, in a process of its
/// own, since the helper reads the process's PATH and environment.
#[test]
#[ignore = "run by through_the_helper in a process of its own"]
fn run_through_the_helper() {
    let Ok(args) = std::env::var("RUN_THROUGH_THE_HELPER") else {
        return;
    };
    let args: Vec<&str> = args.lines().collect();

    let run = sci_anim(&args);

    assert_eq!(run.exit_code, 2, "{}", run.stdout);
    assert_eq!(run.json["errors"][0]["kind"], "io");
    assert_eq!(run.json["errors"][0]["file"], MISSING);
    assert_eq!(
        run.stderr, "",
        "the lines dibs prints while it waits are left out"
    );
}

const MISSING: &str = "/no/such/scene.json";

/// Runs `args` through the test helper in a process with `env` set, whose PATH holds only a `dibs`
/// that says it waits, records its arguments and runs the command, or nothing when
/// `dibs_installed` is false. Returns the arguments dibs was given, or none when the command ran
/// without it.
fn through_the_helper(args: &[&str], dibs_installed: bool, env: &[(&str, &str)]) -> Option<String> {
    let bin = tempfile::tempdir().unwrap();
    if dibs_installed {
        let dibs = bin.path().join("dibs");
        std::fs::write(
            &dibs,
            "#!/bin/sh\necho 'dibs: waiting: another program holds the GPU' >&2\nprintf '%s\\n' \"$@\" > \"${0%/*}/dibs-args\"\nwhile [ \"$1\" != -- ]; do shift; done\nshift\nDIBS_LEASE=fake GPU_LOCK_HELD=1 exec \"$@\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&dibs, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "run_through_the_helper", "--ignored"])
        .env_remove("GPU_LOCK_HELD")
        .env_remove("DIBS_LEASE")
        .env("PATH", bin.path())
        .env("RUN_THROUGH_THE_HELPER", args.join("\n"))
        .envs(env.iter().copied())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read_to_string(bin.path().join("dibs-args")).ok()
}

const FRAME: [&str; 6] = ["frame", MISSING, "-t", "0", "-o", "/no/such/f.png"];

#[test]
fn a_drawing_command_runs_under_dibs_when_it_is_installed() {
    let dibs_args = through_the_helper(&FRAME, true, &[]);

    let mut expected = vec![
        "run",
        "--gpu",
        "--priority",
        "10",
        "--label",
        "sci_anim test: frame",
        "--",
        env!("CARGO_BIN_EXE_sci_anim"),
    ];
    expected.extend(FRAME);
    assert_eq!(dibs_args, Some(expected.join("\n") + "\n"));
}

#[test]
fn a_command_that_does_not_draw_runs_without_dibs() {
    assert_eq!(through_the_helper(&["validate", MISSING], true, &[]), None);
}

#[test]
fn a_drawing_command_runs_directly_where_dibs_is_not_installed() {
    assert_eq!(through_the_helper(&FRAME, false, &[]), None);
}

/// A program that holds the older lock file sets `GPU_LOCK_HELD` alone, and dibs would wait on it.
#[test]
fn a_drawing_command_inside_a_holder_of_the_lock_file_runs_directly() {
    assert_eq!(
        through_the_helper(&FRAME, true, &[("GPU_LOCK_HELD", "1")]),
        None
    );
}

#[test]
fn a_drawing_command_inside_a_dibs_lease_runs_under_dibs() {
    let dibs_args = through_the_helper(
        &FRAME,
        true,
        &[("GPU_LOCK_HELD", "1"), ("DIBS_LEASE", "outer")],
    );

    assert!(dibs_args.is_some_and(|args| args.starts_with("run\n--gpu\n")));
}
