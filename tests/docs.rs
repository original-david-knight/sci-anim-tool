mod common;

use std::io::Write;
use std::process::{Command, Stdio};

use common::sci_anim;

/// docs/scene-reference.md is what scripts/scene-reference.go writes from `sci_anim schema`.
#[test]
fn the_scene_reference_matches_the_schema() {
    let schema = sci_anim(&["schema"]).stdout;
    let mut generator = Command::new("go")
        .args(["run", "scripts/scene-reference.go"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| {
            panic!("could not run `go` ({e}): install Go, which runs scripts/scene-reference.go")
        });
    generator
        .stdin
        .take()
        .unwrap()
        .write_all(schema.as_bytes())
        .unwrap();
    let out = generator.wait_with_output().unwrap();
    assert!(out.status.success(), "scripts/scene-reference.go failed");
    let committed = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/scene-reference.md"),
    )
    .unwrap();
    assert!(
        String::from_utf8(out.stdout).unwrap() == committed,
        "docs/scene-reference.md differs from the schema: run scripts/scene-reference and commit \
         what it writes"
    );
}
