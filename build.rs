//! Sets SCI_ANIM_REVISION, which `--version` prints after the package version: the git commit
//! the binary was built from, or `unknown`, and ` modified` when the work tree had uncommitted
//! changes.
//!
//! Writes `skill_files.rs` into OUT_DIR: every file of `skills/sci-anim/`, sorted by its path in
//! the skill, so the binary carries the skill that matches it for `sci_anim skill install`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|out| out.status.success())?;
    String::from_utf8(out.stdout).ok()
}

fn files_under(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
    {
        let path = entry.unwrap().path();
        // Hidden files are left by tools, not written for the skill.
        let name = path.file_name().unwrap().to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            files_under(&path, files);
        } else {
            files.push(path);
        }
    }
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn is_executable(_: &Path) -> bool {
    false
}

fn write_skill_files() {
    let skill = Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("skills/sci-anim");
    let mut files = Vec::new();
    files_under(&skill, &mut files);
    let mut entries: Vec<(String, PathBuf)> = files
        .into_iter()
        .map(|file| {
            let in_skill = file
                .strip_prefix(&skill)
                .unwrap()
                .components()
                .map(|part| {
                    part.as_os_str()
                        .to_str()
                        .expect("a skill file's path is UTF-8")
                })
                .collect::<Vec<_>>()
                .join("/");
            (in_skill, file)
        })
        .collect();
    entries.sort();
    let mut table = String::from("&[\n");
    for (in_skill, file) in entries {
        let source = file.to_str().expect("a skill file's path is UTF-8");
        writeln!(table, "    SkillFile {{ path: {in_skill:?}, bytes: include_bytes!({source:?}), executable: {} }},", is_executable(&file)).unwrap();
    }
    table.push_str("]\n");
    std::fs::write(
        Path::new(&std::env::var("OUT_DIR").unwrap()).join("skill_files.rs"),
        table,
    )
    .unwrap();
}

fn main() {
    for path in [
        ".git/HEAD",
        ".git/index",
        ".git/refs",
        ".git/packed-refs",
        "src",
        "skills",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let revision = match git(&["rev-parse", "HEAD"]) {
        Some(commit) => {
            let modified =
                git(&["status", "--porcelain"]).is_some_and(|status| !status.trim().is_empty());
            format!(
                "{}{}",
                commit.trim(),
                if modified { " modified" } else { "" }
            )
        }
        None => "unknown".to_string(),
    };
    println!("cargo:rustc-env=SCI_ANIM_REVISION={revision}");
    write_skill_files();
}
