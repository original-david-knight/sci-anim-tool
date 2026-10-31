//! The project has no Python: no `.py` file, no script that runs under Python, and no text that
//! tells a reader to run it. Scripts and scene generators are written in Go.

use std::path::Path;
use std::process::Command;

const COMMANDS: [&str; 5] = ["python", "python3", "uv run", "pip", "pip3"];
const PROSE: [&str; 3] = ["Python program", "Python script", "Python file"];

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Where `word` stands alone in `line`: not inside a longer word such as `pipeline` or a name such
/// as `no_python` or `cpython-ext`.
fn has_word(line: &str, word: &str) -> bool {
    line.match_indices(word).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + word.len()..].chars().next();
        !before.is_some_and(|c| is_word_char(c) || c == '-')
            && !after.is_some_and(|c| is_word_char(c) || c == '-')
    })
}

/// A file name such as `spring_wave.py` in the line.
fn names_a_python_file(line: &str) -> bool {
    line.match_indices(".py").any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + 3..].chars().next();
        before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    })
}

/// Whether a file's text is read for instructions: Markdown, Rust, Go, CI workflows, shell
/// scripts, files in a `scripts` folder, and any file that starts with a shebang.
fn is_read_for_instructions(path: &str, text: &str) -> bool {
    let extension = Path::new(path).extension().and_then(|e| e.to_str());
    matches!(
        extension,
        Some("md" | "rs" | "go" | "sh" | "yml" | "yaml" | "toml")
    ) || path.starts_with("scripts/")
        || path.contains("/scripts/")
        || text.starts_with("#!")
}

/// Each way `text`, the contents of `path`, brings Python into the project, one per line.
fn offenses(path: &str, text: &str) -> Vec<String> {
    let mut found = Vec::new();
    if path.ends_with(".py") {
        found.push(format!("{path}: is a Python file"));
    }
    let first = text.lines().next().unwrap_or("");
    let python_shebang = first.starts_with("#!") && first.contains("python");
    if python_shebang {
        found.push(format!("{path}:1: runs under Python: {first}"));
    }
    if !is_read_for_instructions(path, text) {
        return found;
    }
    for (index, line) in text.lines().enumerate().skip(usize::from(python_shebang)) {
        let commands: Vec<&str> = COMMANDS
            .into_iter()
            .filter(|word| has_word(line, word))
            .chain(PROSE.into_iter().filter(|phrase| line.contains(phrase)))
            .collect();
        let mut said = commands.join(", ");
        if names_a_python_file(line) {
            said = if said.is_empty() {
                "a .py file".to_string()
            } else {
                format!("{said}, a .py file")
            };
        }
        if !said.is_empty() {
            found.push(format!("{path}:{}: names {said}", index + 1));
        }
    }
    found
}

#[test]
fn no_tracked_file_brings_in_python() {
    let repo = env!("CARGO_MANIFEST_DIR");
    let listed = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(repo)
        .output()
        .expect("git lists the tracked files");
    assert!(listed.status.success(), "git ls-files failed");
    let mut found = Vec::new();
    for path in String::from_utf8(listed.stdout)
        .unwrap()
        .split_terminator('\0')
    {
        // This file names the commands it looks for.
        if path == "tests/no_python.rs" {
            continue;
        }
        let Ok(bytes) = std::fs::read(Path::new(repo).join(path)) else {
            continue;
        };
        found.extend(offenses(path, &String::from_utf8_lossy(&bytes)));
    }
    assert!(
        found.is_empty(),
        "the project has no Python, and scripts and scene generators are written in Go, \
         but these bring Python in:\n{}",
        found.join("\n")
    );
}

#[test]
fn each_way_of_bringing_in_python_is_named() {
    let found: Vec<String> = [
        ("examples/wave.py", "print(1)\n"),
        ("scripts/make", "#!/usr/bin/env python3\nimport sys\n"),
        (
            "README.md",
            "Run `python3 -I make.py out.json`, or `uv run make`.\n",
        ),
        (
            "docs/setup.md",
            "Then pip install numpy.\nWrite a short Python program.\n",
        ),
        ("tests/gen.rs", "Command::new(\"python\")\n"),
        (
            "notes.md",
            "No Python here: no_python.rs, a pipeline, cpython-ext, snippets.\n",
        ),
        ("examples/scene.json", "{\"text\": \"run python3\"}\n"),
    ]
    .iter()
    .flat_map(|(path, text)| offenses(path, text))
    .collect();
    assert_eq!(
        found,
        [
            "examples/wave.py: is a Python file",
            "scripts/make:1: runs under Python: #!/usr/bin/env python3",
            "README.md:1: names python3, uv run, a .py file",
            "docs/setup.md:1: names pip",
            "docs/setup.md:2: names Python program",
            "tests/gen.rs:1: names python",
        ]
    );
}
