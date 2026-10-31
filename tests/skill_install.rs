//! `sci_anim skill install` writes the skill the binary was built with into a skills folder,
//! replaces an earlier install, and never writes through a link.

mod common;

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::sci_anim;
use serde_json::json;

const SKILL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/skills/sci-anim");

/// Every file under `dir`, by its path from `dir`, with its bytes.
fn files_under(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, files: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    std::fs::read(&path).unwrap(),
                );
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(dir, dir, &mut files);
    files
}

fn entries(dir: &Path) -> Vec<String> {
    let mut listed: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    listed.sort();
    listed
}

#[test]
fn installs_the_skill_byte_for_byte_and_again_over_an_earlier_install() {
    let dir = tempfile::tempdir().unwrap();
    let skills_folder = dir.path().join("project/.claude/skills");
    let skill = skills_folder.join("sci-anim");

    let first = sci_anim(&["skill", "install", "--to", skills_folder.to_str().unwrap()]);

    assert_eq!(first.exit_code, 0, "{}", first.stdout);
    assert_eq!(
        first.json,
        json!({ "status": "ok", "command": "skill_install", "installed": [skill], "version": env!("CARGO_PKG_VERSION") })
    );
    assert_eq!(
        files_under(&skill),
        files_under(Path::new(SKILL)),
        "the installed skill is the skill folder, file for file"
    );
    assert!(files_under(&skill).contains_key(Path::new("SKILL.md")));
    assert_eq!(
        std::fs::metadata(skill.join("scripts/fields"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
    assert_eq!(
        std::fs::metadata(skill.join("SKILL.md"))
            .unwrap()
            .permissions()
            .mode()
            & 0o111,
        0
    );

    std::fs::write(skill.join("mine.md"), "added by hand").unwrap();
    std::fs::remove_file(skill.join("references/math.md")).unwrap();
    let second = sci_anim(&["skill", "install", "--to", skills_folder.to_str().unwrap()]);

    assert_eq!(second.exit_code, 0, "{}", second.stdout);
    assert_eq!(second.json, first.json);
    assert_eq!(
        files_under(&skill),
        files_under(Path::new(SKILL)),
        "a second install puts back the skill as it was built"
    );
    assert_eq!(entries(&skills_folder), ["sci-anim"]);
}

#[test]
fn installs_into_each_skills_folder_given() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (dir.path().join("a"), dir.path().join("b"));

    let run = sci_anim(&[
        "skill",
        "install",
        "--to",
        a.to_str().unwrap(),
        "--to",
        b.to_str().unwrap(),
    ]);

    assert_eq!(run.exit_code, 0, "{}", run.stdout);
    assert_eq!(
        run.json["installed"],
        json!([a.join("sci-anim"), b.join("sci-anim")])
    );
    assert_eq!(
        files_under(&b.join("sci-anim")),
        files_under(Path::new(SKILL))
    );
}

#[test]
fn refuses_a_symlink_and_leaves_its_target_alone() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("checkout/skills/sci-anim");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(
        target.join("SKILL.md"),
        "---\nname: sci-anim\n---\nworking copy\n",
    )
    .unwrap();
    let skills_folder = dir.path().join("skills");
    std::fs::create_dir(&skills_folder).unwrap();
    std::os::unix::fs::symlink(&target, skills_folder.join("sci-anim")).unwrap();

    let run = sci_anim(&["skill", "install", "--to", skills_folder.to_str().unwrap()]);

    assert_eq!(run.exit_code, 2, "{}", run.stdout);
    assert_eq!(
        run.json,
        json!({ "status": "error", "errors": [{
            "kind": "io",
            "file": skills_folder.join("sci-anim"),
            "message": format!("the skill folder is a link to {}, which sci_anim skill install never writes through; remove the link, or pass --to <skills folder> to install somewhere else", target.display()),
        }]})
    );
    assert_eq!(entries(&target), ["SKILL.md"]);
    assert_eq!(
        std::fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "---\nname: sci-anim\n---\nworking copy\n"
    );
    assert_eq!(entries(&skills_folder), ["sci-anim"]);
}

#[test]
fn refuses_a_folder_that_is_not_the_sci_anim_skill() {
    let dir = tempfile::tempdir().unwrap();
    let other = dir.path().join("sci-anim");
    std::fs::create_dir(&other).unwrap();
    std::fs::write(other.join("SKILL.md"), "---\nname: something-else\n---\n").unwrap();

    let run = sci_anim(&["skill", "install", "--to", dir.path().to_str().unwrap()]);

    assert_eq!(run.exit_code, 2, "{}", run.stdout);
    assert_eq!(run.json["errors"][0]["kind"], "io");
    assert_eq!(run.json["errors"][0]["file"], json!(other));
    assert_eq!(entries(&other), ["SKILL.md"]);
    assert_eq!(
        std::fs::read_to_string(other.join("SKILL.md")).unwrap(),
        "---\nname: something-else\n---\n"
    );
}

/// Runs `sci_anim skill install` with no `--to` and HOME at `home`.
fn install_at_home(home: &Path) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .args(["skill", "install"])
        .env("HOME", home)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

#[test]
fn with_no_to_installs_for_each_agent_home_that_exists() {
    let home = tempfile::tempdir().unwrap();
    std::fs::create_dir(home.path().join(".claude")).unwrap();

    assert_eq!(
        install_at_home(home.path())["installed"],
        json!([home.path().join(".claude/skills/sci-anim")])
    );
    assert!(!home.path().join(".agents").exists());

    std::fs::create_dir(home.path().join(".agents")).unwrap();
    assert_eq!(
        install_at_home(home.path())["installed"],
        json!([
            home.path().join(".claude/skills/sci-anim"),
            home.path().join(".agents/skills/sci-anim")
        ])
    );
    assert_eq!(
        files_under(&home.path().join(".agents/skills/sci-anim")),
        files_under(Path::new(SKILL))
    );
}

#[test]
fn with_no_to_and_no_agent_home_installs_into_agents_skills() {
    let home = tempfile::tempdir().unwrap();

    assert_eq!(
        install_at_home(home.path())["installed"],
        json!([home.path().join(".agents/skills/sci-anim")])
    );
    assert_eq!(entries(home.path()), [".agents"]);
}

/// Runs `sci_anim skill install` with `args` in `dir`, HOME at `home`, and stdin and stdout not
/// a terminal, so it never asks.
fn install_in(dir: &Path, home: &Path, args: &[&str]) -> (Option<i32>, serde_json::Value) {
    let out = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .args(["skill", "install"])
        .args(args)
        .current_dir(dir)
        .env("HOME", home)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    (
        out.status.code(),
        serde_json::from_slice(&out.stdout).unwrap(),
    )
}

#[test]
fn project_installs_at_the_root_of_the_repository_holding_the_current_directory() {
    let dir = tempfile::tempdir().unwrap();
    let (home, repo) = (dir.path().join("home"), dir.path().join("repo"));
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(repo.join(".claude")).unwrap();
    std::fs::create_dir_all(repo.join("src/deep")).unwrap();
    std::fs::create_dir_all(&home).unwrap();

    let (exit, report) = install_in(&repo.join("src/deep"), &home, &["--project"]);

    assert_eq!(exit, Some(0), "{report}");
    assert_eq!(
        report["installed"],
        json!([repo.join(".claude/skills/sci-anim")])
    );
    assert_eq!(entries(&home), Vec::<String>::new());
}

#[test]
fn with_no_flag_and_no_terminal_installs_at_user_level_inside_a_repository() {
    let dir = tempfile::tempdir().unwrap();
    let (home, repo) = (dir.path().join("home"), dir.path().join("repo"));
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    std::fs::create_dir_all(home.join(".claude")).unwrap();

    let (exit, report) = install_in(&repo, &home, &[]);

    assert_eq!(exit, Some(0), "{report}");
    assert_eq!(
        report["installed"],
        json!([home.join(".claude/skills/sci-anim")])
    );
    assert_eq!(entries(&repo), [".git"]);
}

#[test]
fn project_outside_a_repository_installs_nothing() {
    let dir = tempfile::tempdir().unwrap();

    let (exit, report) = install_in(dir.path(), dir.path(), &["--project"]);

    assert_eq!(exit, Some(2), "{report}");
    assert_eq!(report["errors"][0]["kind"], "usage");
    assert_eq!(entries(dir.path()), Vec::<String>::new());
}

#[test]
fn to_user_and_project_refuse_each_other() {
    let dir = tempfile::tempdir().unwrap();
    let to = dir.path().join("skills");
    let to = to.to_str().unwrap();
    for pair in [
        &["--to", to, "--user"][..],
        &["--to", to, "--project"],
        &["--user", "--project"],
    ] {
        let (exit, report) = install_in(dir.path(), dir.path(), pair);
        assert_eq!(exit, Some(2), "{pair:?}: {report}");
        assert_eq!(report["errors"][0]["kind"], "usage", "{pair:?}");
        assert!(
            report["errors"][0]["message"]
                .as_str()
                .unwrap()
                .contains("cannot be used with"),
            "{pair:?}: {report}"
        );
    }
    assert_eq!(entries(dir.path()), Vec::<String>::new());
}
