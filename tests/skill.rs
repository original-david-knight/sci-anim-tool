//! Every object type, animation type, command and flag an agent can use is named in the sci-anim
//! skill, every command and flag in the command reference, and every object and animation type in
//! the verify-sci-anim feature map. The printed schema and `--help` are the lists, so a new one
//! fails here until its docs land with it.

mod common;

use std::path::Path;
use std::process::Command;

use common::sci_anim;

const SKILL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/skills/sci-anim");
const FEATURE_MAP: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/.claude/skills/verify-sci-anim/features"
);

/// Types no feature file drove when this test was written. Add no type here: write its recipe in
/// the feature map instead, and remove a type from here once a feature file names it.
const NOT_YET_IN_THE_FEATURE_MAP: &[&str] = &[
    "arc",
    "number-line",
    "point-plot",
    "rotate",
    "camera-move",
    "camera-look",
    "camera-zoom",
];

/// The text of every `.md` and `.json` file under `dir`.
fn text_under(dir: &Path) -> String {
    let mut text = String::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            text += &text_under(&path);
        } else if path.extension().is_some_and(|e| e == "md" || e == "json") {
            text += &std::fs::read_to_string(&path).unwrap();
        }
    }
    text
}

fn mentions_type(text: &str, value: &str) -> bool {
    [
        format!("`{value}`"),
        format!("\"type\": \"{value}\""),
        format!("\"type\":\"{value}\""),
    ]
    .iter()
    .any(|form| text.contains(form.as_str()))
}

/// Each object type and each animation type the printed schema accepts.
fn scene_types() -> Vec<String> {
    let schema = sci_anim(&["schema"]).json;
    ["ObjectFile", "AnimationFile"]
        .iter()
        .flat_map(|def| {
            schema["$defs"][def]["properties"]["type"]["enum"]
                .as_array()
                .unwrap_or_else(|| panic!("{def} lists its types"))
                .clone()
        })
        .map(|value| value.as_str().unwrap().to_string())
        .collect()
}

fn help(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_sci_anim"))
        .args(args)
        .arg("--help")
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap()
}

/// Each command of `sci_anim` under `parent`, from its `--help`, such as `validate` or
/// `skill install`: a command with subcommands of its own gives each of them instead.
fn commands(parent: &[&str]) -> Vec<String> {
    let text = help(parent);
    let Some(listed) = text.split("Commands:\n").nth(1) else {
        return vec![parent.join(" ")];
    };
    let listed = listed.split("\n\n").next().unwrap();
    listed
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|command| *command != "help")
        .flat_map(|command| commands(&[parent, &[command]].concat()))
        .collect()
}

#[test]
fn the_skill_mentions_every_object_and_animation_type() {
    let skill = text_under(Path::new(SKILL));
    let missing: Vec<String> = scene_types()
        .into_iter()
        .filter(|value| !mentions_type(&skill, value))
        .collect();
    assert!(
        missing.is_empty(),
        "skills/sci-anim never names these types in backticks or as a \"type\" value: {missing:?}. \
         Teach each one in the reference SKILL.md routes its topic to, in the same change that adds it."
    );
}

/// Each command and flag of `sci_anim` that `text` never names, a flag in any of its forms.
fn unnamed_commands_and_flags(text: &str) -> Vec<String> {
    let mut missing = Vec::new();
    for command in commands(&[]) {
        if !text.contains(&format!("sci_anim {command}")) {
            missing.push(format!("sci_anim {command}"));
        }
        // An option's line lists its short form and aliases too, and the skill may use any of them.
        for option in help(&command.split(' ').collect::<Vec<_>>())
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with('-') && !line.starts_with("-h"))
        {
            let forms: Vec<&str> = option
                .split([' ', ',', ']'])
                .filter(|word| word.starts_with('-'))
                .collect();
            let named = forms.iter().any(|form| {
                [' ', '`']
                    .iter()
                    .any(|after| text.contains(&format!("{form}{after}")))
            });
            if !named && !missing.iter().any(|m| m == forms[0]) {
                missing.push(forms.join(" or "));
            }
        }
    }
    missing
}

#[test]
fn the_skill_mentions_every_command_and_flag() {
    let skill = std::fs::read_to_string(format!("{SKILL}/SKILL.md")).unwrap();
    let missing = unnamed_commands_and_flags(&skill);
    assert!(
        missing.is_empty(),
        "skills/sci-anim/SKILL.md never names {missing:?}. Add each to the table in \"The tool\"."
    );
}

#[test]
fn the_command_reference_names_every_command_and_flag() {
    let reference = include_str!("../docs/cli.md");
    let missing = unnamed_commands_and_flags(reference);
    assert!(
        missing.is_empty(),
        "docs/cli.md never names {missing:?}. Describe each in its command's section."
    );
}

/// Each `.md` file under `dir`, with its text.
fn markdown_under(dir: &Path) -> Vec<(std::path::PathBuf, String)> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(markdown_under(&path));
        } else if path.extension().is_some_and(|e| e == "md") {
            let text = std::fs::read_to_string(&path).unwrap();
            files.push((path, text));
        }
    }
    files
}

#[test]
fn every_file_the_skill_names_is_in_the_skill() {
    // The skill is installed as its folder alone, so a path it names must resolve from the file
    // that names it, without the rest of the repository.
    // A path in backticks, or a bare file name such as `motion.md`, which may also name a file at
    // the skill's root, such as `SKILL.md`.
    let is_path = |token: &str| {
        ["../", "references/", "examples/", "scripts/"]
            .iter()
            .any(|s| token.starts_with(s))
    };
    let is_bare_file = |token: &str| !token.contains('/') && token.ends_with(".md");
    let mut missing = Vec::new();
    for (path, text) in markdown_under(Path::new(SKILL)) {
        let here = path.parent().unwrap();
        for token in text
            .split('`')
            .skip(1)
            .step_by(2)
            .filter(|token| !token.contains(' '))
        {
            let found = if is_path(token) {
                here.join(token).exists()
            } else if is_bare_file(token) {
                here.join(token).exists() || Path::new(SKILL).join(token).exists()
            } else {
                true
            };
            if !found {
                missing.push(format!(
                    "{}: {token}",
                    path.strip_prefix(SKILL).unwrap().display()
                ));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "skills/sci-anim names files it does not hold: {missing:?}"
    );
}

/// Runs `go run` with `args` in `dir`, as the skill tells an agent to.
fn go_run(dir: &Path, args: &[&str]) {
    let ran = Command::new("go")
        .arg("run")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap_or_else(|e| {
            panic!(
                "could not run `go` ({e}): install Go, which runs the skill's scene generators and \
                 scripts/straight_chain.go"
            )
        });
    assert!(
        ran.success(),
        "`go run {}` in {} failed",
        args.join(" "),
        dir.display()
    );
}

#[test]
fn each_scene_generator_example_writes_its_scene_file() {
    let examples = Path::new(SKILL).join("examples");
    let generators: Vec<_> = std::fs::read_dir(&examples)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "go"))
        .collect();
    assert!(
        generators.len() >= 3,
        "the skill's examples hold the three scene generators"
    );
    let out_dir = tempfile::tempdir().unwrap();
    for generator in generators {
        let written = out_dir
            .path()
            .join(generator.with_extension("json").file_name().unwrap());
        go_run(
            &examples,
            &[
                generator.file_name().unwrap().to_str().unwrap(),
                written.to_str().unwrap(),
            ],
        );
        let committed = std::fs::read_to_string(generator.with_extension("json")).unwrap();
        assert_eq!(
            std::fs::read_to_string(&written).unwrap(),
            committed,
            "{} no longer writes the committed scene file: run it and commit what it writes",
            generator.display()
        );
    }
}

#[test]
fn the_chain_generator_names_an_unknown_monomer_and_the_ones_it_knows() {
    let out_dir = tempfile::tempdir().unwrap();
    let written = out_dir.path().join("nylon.sdf");
    let ran = Command::new("go")
        .args(["run", "straight_chain.go", "nylon-6", "4"])
        .arg(&written)
        .current_dir(Path::new(SKILL).join("scripts"))
        .output()
        .expect("could not run `go`: install Go, which runs scripts/straight_chain.go");
    let stderr = String::from_utf8(ran.stderr).unwrap();
    // `go run` exits 1 when the program fails, and names the program's own exit status last.
    assert_eq!(ran.status.code(), Some(1), "stderr: {stderr}");
    assert_eq!(stderr.lines().last(), Some("exit status 2"));
    let lines: Vec<&str> = stderr.lines().take(3).collect();
    assert_eq!(
        lines,
        [
            "straight_chain.go knows no monomer \"nylon-6\"",
            "usage: go run straight_chain.go [-radical] <monomer> <length> <file.sdf>",
            "monomers: ethylene, propylene, vinyl-chloride, tetrafluoroethylene, styrene, \
             ethylene-terephthalate",
        ]
    );
    assert!(!written.exists(), "it wrote {}", written.display());
}

#[test]
fn each_command_in_the_notice_writes_its_structure_file() {
    // The NOTICE says how each structure file that is not PubChem's was written, one `go run` a
    // line, run in its folder, with the file it writes last.
    let folder = Path::new(SKILL).join("examples/structures");
    let notice = std::fs::read_to_string(folder.join("NOTICE")).unwrap();
    let commands: Vec<Vec<&str>> = notice
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("go run "))
        .map(|command| command.split_whitespace().collect())
        .collect();
    assert_eq!(
        commands.len(),
        8,
        "the NOTICE gives the command that wrote each of the four radicals, the two nucleobase \
         pairs, the polystyrene chain and the PET chain"
    );
    let out_dir = tempfile::tempdir().unwrap();
    for command in commands {
        let (file, args) = command.split_last().unwrap();
        let written = out_dir.path().join(file);
        go_run(&folder, &[args, &[written.to_str().unwrap()]].concat());
        assert_eq!(
            std::fs::read_to_string(&written).unwrap(),
            std::fs::read_to_string(folder.join(file)).unwrap(),
            "`go run {}` no longer writes the committed {file}: run it and commit what it writes",
            command.join(" ")
        );
    }
}

#[test]
fn the_feature_map_mentions_every_object_and_animation_type() {
    let map = text_under(Path::new(FEATURE_MAP));
    let types = scene_types();
    let missing: Vec<&String> = types
        .iter()
        .filter(|value| {
            !mentions_type(&map, value) && !NOT_YET_IN_THE_FEATURE_MAP.contains(&value.as_str())
        })
        .collect();
    assert!(
        missing.is_empty(),
        "no file in .claude/skills/verify-sci-anim/features names these types: {missing:?}. \
         Add a recipe that drives each one, and list it in features/README.md."
    );
    let mapped: Vec<&&str> = NOT_YET_IN_THE_FEATURE_MAP
        .iter()
        .filter(|value| mentions_type(&map, value))
        .collect();
    assert!(
        mapped.is_empty(),
        "the feature map now names {mapped:?}: remove them from NOT_YET_IN_THE_FEATURE_MAP in tests/skill.rs"
    );
    let unknown: Vec<&&str> = NOT_YET_IN_THE_FEATURE_MAP
        .iter()
        .filter(|value| !types.iter().any(|t| t == **value))
        .collect();
    assert!(
        unknown.is_empty(),
        "the schema has no types {unknown:?}: remove them from NOT_YET_IN_THE_FEATURE_MAP in tests/skill.rs"
    );
}
