//! The sci-anim skill this binary was built with, and `sci_anim skill install`, which writes it
//! into skills folders at project level, at user level, or given with `--to`.

use std::fs;
use std::io::{BufRead, ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::cli::ToolError;

/// One file of the skill: its path in the skill folder with forward slashes, and its bytes.
struct SkillFile {
    path: &'static str,
    bytes: &'static [u8],
    executable: bool,
}

const SKILL_FILES: &[SkillFile] = include!(concat!(env!("OUT_DIR"), "/skill_files.rs"));

const SKILL_FOLDER: &str = "sci-anim";
const INSTALLING_FOLDER: &str = ".sci-anim.installing";

/// The level `--user` or `--project` names, or the person at the terminal answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Level {
    User,
    Project,
}

/// The skills folders an install writes into, chosen from its flags, the repository it runs in and whether a
/// person is at the terminal.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum SkillsFolders {
    /// The skills folders given with `--to`.
    Given(Vec<PathBuf>),
    User,
    Project {
        repository: PathBuf,
    },
    /// Ask the person at the terminal whether to install at project level in this repository or
    /// at user level.
    Ask {
        repository: PathBuf,
    },
}

/// Chooses the skills folders an install writes into. `--to`, `--user` and `--project` are never given together.
/// With none of them it asks only inside a repository with a person at the terminal, so a
/// script or an agent gets user level and never waits on a question.
pub(crate) fn choose(
    given: Vec<PathBuf>,
    level: Option<Level>,
    repository: Option<PathBuf>,
    terminal: bool,
) -> Result<SkillsFolders, ToolError> {
    match (level, repository) {
        _ if !given.is_empty() => Ok(SkillsFolders::Given(given)),
        (Some(Level::User), _) => Ok(SkillsFolders::User),
        (Some(Level::Project), Some(repository)) => Ok(SkillsFolders::Project { repository }),
        (Some(Level::Project), None) => Err(ToolError::Usage {
            message: "--project installs into the git repository holding the current directory, and there is none; run it inside a repository, or pass --user or --to <skills folder>".into(),
        }),
        (None, Some(repository)) if terminal => Ok(SkillsFolders::Ask { repository }),
        (None, _) => Ok(SkillsFolders::User),
    }
}

/// The root of the git work tree holding `dir`: the nearest folder, from `dir` up, with an entry
/// named `.git`, which is a file in a worktree.
pub(crate) fn repository_holding(dir: &Path) -> Option<PathBuf> {
    dir.ancestors()
        .find(|folder| fs::symlink_metadata(folder.join(".git")).is_ok())
        .map(Path::to_path_buf)
}

/// The skills folders under `root`, the home folder at user level or a repository's root at
/// project level: `skills` in each of `<root>/.claude` and `<root>/.agents` that exists, or
/// `<root>/.agents/skills` when neither does.
pub(crate) fn skills_folders_under(root: &Path) -> Vec<PathBuf> {
    let folders: Vec<PathBuf> = [".claude", ".agents"]
        .iter()
        .map(|agent_home| root.join(agent_home))
        .filter(|agent_home| agent_home.is_dir())
        .map(|agent_home| agent_home.join("skills"))
        .collect();
    if folders.is_empty() {
        vec![root.join(".agents/skills")]
    } else {
        folders
    }
}

/// The skills folders the chosen skills folders are, asking on `writer` and reading the answer from
/// `reader` for [`SkillsFolders::Ask`].
pub(crate) fn resolve(
    chosen: SkillsFolders,
    reader: impl BufRead,
    writer: impl Write,
) -> Result<Vec<PathBuf>, ToolError> {
    let home = || {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| ToolError::Usage {
                message: "HOME is not set, so there is no user level to install at; pass --project or --to <skills folder>".into(),
            })
    };
    match chosen {
        SkillsFolders::Given(folders) => Ok(folders),
        SkillsFolders::User => Ok(skills_folders_under(&home()?)),
        SkillsFolders::Project { repository } => Ok(skills_folders_under(&repository)),
        SkillsFolders::Ask { repository } => {
            let home = home()?;
            let project_folders = skills_folders_under(&repository);
            let user_folders = skills_folders_under(&home);
            let answer = ask(
                &repository,
                &project_folders,
                &home,
                &user_folders,
                reader,
                writer,
            )
            .map_err(|e| ToolError::Usage {
                message: format!("could not ask which level to install at: {e}"),
            })?;
            match answer {
                Some(Level::Project) => Ok(project_folders),
                Some(Level::User) => Ok(user_folders),
                None => Err(ToolError::Unanswered {
                    message: "stdin ended before an answer to project level or user level, so nothing was installed; pass --project, --user or --to <skills folder>".into(),
                }),
            }
        }
    }
}

/// Asks on `writer` whether to install for `repository` (project level) or for the user (user
/// level), naming the skills folders each answer writes, until `reader` gives `p` or `u`. `None`
/// when `reader` ends first.
fn ask(
    repository: &Path,
    project_folders: &[PathBuf],
    home: &Path,
    user_folders: &[PathBuf],
    mut reader: impl BufRead,
    mut writer: impl Write,
) -> std::io::Result<Option<Level>> {
    let shown = |folders: &[PathBuf], root: &Path, prefix: &str| {
        folders
            .iter()
            .map(|folder| match folder.strip_prefix(root) {
                Ok(under) => format!("{prefix}{}", under.display()),
                Err(_) => folder.display().to_string(),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let question = format!(
        "Install the sci-anim skill for this repository ({}: {}) or for your user ({})? [p/u] ",
        repository.display(),
        shown(project_folders, repository, ""),
        shown(user_folders, home, "~/"),
    );
    loop {
        write!(writer, "{question}")?;
        writer.flush()?;
        let mut answer = String::new();
        if reader.read_line(&mut answer)? == 0 {
            writeln!(writer)?;
            return Ok(None);
        }
        match answer.trim().to_ascii_lowercase().as_str() {
            "p" | "project" => return Ok(Some(Level::Project)),
            "u" | "user" => return Ok(Some(Level::User)),
            _ => {}
        }
    }
}

/// Writes the skill into `<skills folder>/sci-anim` of each skills folder, and returns those
/// skill folders. It replaces an earlier sci-anim skill there and refuses anything else.
pub(crate) fn install(skills_folders: &[PathBuf]) -> Result<Vec<PathBuf>, Vec<ToolError>> {
    let mut installed = Vec::new();
    let mut errors = Vec::new();
    for skills_folder in skills_folders {
        match install_into(skills_folder) {
            Ok(skill) => installed.push(skill),
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Ok(installed)
    } else {
        Err(errors)
    }
}

fn install_into(skills_folder: &Path) -> Result<PathBuf, ToolError> {
    let skill = skills_folder.join(SKILL_FOLDER);
    let refuse = |message: String| ToolError::Io {
        file: skill.clone(),
        message,
    };
    let earlier_install = match fs::symlink_metadata(&skill) {
        Err(e) if e.kind() == ErrorKind::NotFound => false,
        Err(e) => return Err(io_error(&skill, e)),
        Ok(meta) if meta.file_type().is_symlink() => {
            let target = fs::read_link(&skill).map_err(|e| io_error(&skill, e))?;
            return Err(refuse(format!("the skill folder is a link to {}, which sci_anim skill install never writes through; remove the link, or pass --to <skills folder> to install somewhere else", target.display())));
        }
        Ok(meta) if meta.is_dir() && is_sci_anim_skill(&skill) => true,
        Ok(_) => return Err(refuse("the skill folder is in the way and is not a sci-anim skill, since it has no SKILL.md that says `name: sci-anim`; move it, or pass --to <skills folder> to install somewhere else".into())),
    };
    let installing = skills_folder.join(INSTALLING_FOLDER);
    remove(&installing)?;
    for file in SKILL_FILES {
        write_skill_file(&installing, file)?;
    }
    if earlier_install {
        fs::remove_dir_all(&skill).map_err(|e| io_error(&skill, e))?;
    }
    fs::rename(&installing, &skill).map_err(|e| io_error(&skill, e))?;
    Ok(skill)
}

/// Whether the folder's SKILL.md front matter names the sci-anim skill.
fn is_sci_anim_skill(folder: &Path) -> bool {
    let Ok(text) = fs::read_to_string(folder.join("SKILL.md")) else {
        return false;
    };
    let mut lines = text.lines();
    lines.next() == Some("---")
        && lines
            .take_while(|line| *line != "---")
            .any(|line| line.trim() == "name: sci-anim")
}

/// Removes what a crashed install left, without following a link.
fn remove(path: &Path) -> Result<(), ToolError> {
    let removing = match fs::symlink_metadata(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => Err(e),
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
    };
    removing.map_err(|e| io_error(path, e))
}

fn write_skill_file(installing: &Path, file: &SkillFile) -> Result<(), ToolError> {
    let path = installing.join(file.path);
    fs::create_dir_all(path.parent().expect("a skill file is in a folder"))
        .map_err(|e| io_error(&path, e))?;
    fs::write(&path, file.bytes).map_err(|e| io_error(&path, e))?;
    #[cfg(unix)]
    if file.executable {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .map_err(|e| io_error(&path, e))?;
    }
    Ok(())
}

fn io_error(file: &Path, error: std::io::Error) -> ToolError {
    ToolError::Io {
        file: file.to_path_buf(),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folders(paths: &[&str]) -> Vec<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn skills_folders_from_flags_repository_and_terminal() {
        let repo = || Some(PathBuf::from("/work/repo"));
        let cases = [
            (
                folders(&["/tmp/skills"]),
                None,
                repo(),
                true,
                SkillsFolders::Given(folders(&["/tmp/skills"])),
            ),
            (vec![], Some(Level::User), repo(), true, SkillsFolders::User),
            (
                vec![],
                Some(Level::Project),
                repo(),
                false,
                SkillsFolders::Project {
                    repository: "/work/repo".into(),
                },
            ),
            (
                vec![],
                None,
                repo(),
                true,
                SkillsFolders::Ask {
                    repository: "/work/repo".into(),
                },
            ),
            (vec![], None, repo(), false, SkillsFolders::User),
            (vec![], None, None, true, SkillsFolders::User),
            (vec![], None, None, false, SkillsFolders::User),
        ];
        for (given, level, repository, terminal, expected) in cases {
            assert_eq!(
                choose(given, level, repository, terminal).unwrap(),
                expected
            );
        }
        assert!(matches!(
            choose(vec![], Some(Level::Project), None, true),
            Err(ToolError::Usage { .. })
        ));
    }

    #[test]
    fn the_repository_is_the_nearest_folder_with_a_git_entry() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        let worktree = repo.join(".claude/worktrees/w");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir_all(worktree.join("src")).unwrap();
        fs::write(worktree.join(".git"), "gitdir: elsewhere\n").unwrap();

        assert_eq!(
            repository_holding(&repo.join(".claude")),
            Some(repo.clone())
        );
        assert_eq!(repository_holding(&worktree.join("src")), Some(worktree));
        assert_eq!(repository_holding(dir.path()), None);
    }

    #[test]
    fn project_level_follows_the_user_level_rule_under_the_repository() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert_eq!(skills_folders_under(root), [root.join(".agents/skills")]);
        fs::create_dir(root.join(".claude")).unwrap();
        assert_eq!(skills_folders_under(root), [root.join(".claude/skills")]);
        fs::create_dir(root.join(".agents")).unwrap();
        assert_eq!(
            skills_folders_under(root),
            [root.join(".claude/skills"), root.join(".agents/skills")]
        );
    }

    fn asked(typed: &str) -> (Option<Level>, String) {
        let mut shown = Vec::new();
        let answer = ask(
            Path::new("/work/repo"),
            &folders(&["/work/repo/.claude/skills"]),
            Path::new("/home/me"),
            &folders(&["/home/me/.claude/skills", "/home/me/.agents/skills"]),
            typed.as_bytes(),
            &mut shown,
        )
        .unwrap();
        (answer, String::from_utf8(shown).unwrap())
    }

    const QUESTION: &str = "Install the sci-anim skill for this repository (/work/repo: .claude/skills) or for your user (~/.claude/skills, ~/.agents/skills)? [p/u] ";

    #[test]
    fn asks_until_it_gets_p_or_u() {
        assert_eq!(asked("p\n"), (Some(Level::Project), QUESTION.to_string()));
        assert_eq!(asked("u\n"), (Some(Level::User), QUESTION.to_string()));
        assert_eq!(
            asked("yes\n\np\n"),
            (Some(Level::Project), QUESTION.repeat(3))
        );
        assert_eq!(asked(""), (None, format!("{QUESTION}\n")));
        assert_eq!(
            asked("maybe\n"),
            (None, format!("{}\n", QUESTION.repeat(2)))
        );
    }
}
