# Install the skill

`sci_anim skill install` writes the sci-anim skill the binary was built with into the `sci-anim` folder of a skills folder, so an agent with no checkout gets the skill that matches its binary. At user level it installs into `~/.claude/skills` and `~/.agents/skills`, for each of `~/.claude` and `~/.agents` that exists, or into `~/.agents/skills` when neither does. At project level it installs into the same folders under the root of the git repository holding the current directory. `--user`, `--project` and `--to` choose, at most one of them. With none, a person at a terminal inside a repository is asked which level; a script, an agent, or a run outside a repository gets user level. It prints one ok report with each skill folder it wrote, under `installed`, and the `version`. It replaces a sci-anim skill installed there before, and refuses a link or any other folder in the way with exit 2.

## Sub-features

- `skill-install-to` writes every file of `skills/sci-anim/` byte for byte into `<skills folder>/sci-anim`, with `scripts/fields` executable, making the skills folder if needed.
- `skill-install-again` replaces an earlier install, dropping files added to it and putting back files removed from it, with the same report.
- `skill-install-home` installs, with no `--to`, into the skills folder of each agent home under `HOME` that exists, or into `~/.agents/skills` when none does.
- `skill-install-project` installs, with `--project`, into the skills folders under the root of the repository holding the current directory, found by the nearest `.git` entry, a file in a worktree.
- `skill-install-ask` asks, with no flag, inside a repository, with stdin and stderr a terminal, which level, naming the repository root and the folders each answer writes; `p` installs at project level, `u` at user level, any other answer asks again, and the end of stdin installs nothing with exit 1 and an `unanswered` error.
- `skill-install-no-terminal` installs at user level, with no question, with no flag inside a repository when stdin or stderr is not a terminal, and anywhere outside a repository.
- `skill-install-conflict` refuses any two of `--to`, `--user` and `--project` with a `usage` error, exit 2, and installs nothing.
- `skill-install-link` refuses a link at `<skills folder>/sci-anim` with an `io` error naming the link's target, exit 2, and leaves the target untouched.
- `skill-install-foreign` refuses a folder there whose `SKILL.md` does not say `name: sci-anim`, exit 2, and leaves it untouched.

## How to get to it (user POV)

- Run `sci_anim skill install` at a terminal and answer `p` or `u` inside a repository, or `sci_anim skill install --project` or `--user`, or `sci_anim skill install --to <skills folder>` once per skills folder. `--project` and `--user` need 0.23.0 or later. The sci-anim skill and the README tell agents to run it after every upgrade. `sci_anim --version` names 0.4.0 or later.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set.
- Never drive the default install against your real home folder: a developer's `~/.claude/skills/sci-anim` may be a link into this checkout. Give `HOME` a folder under `$RUN/work/`.

- **Project install.** Run `$SKILL/scripts/run "$RUN" skill-install-to skill install --to "$RUN/work/project/.claude/skills"`. Exit 0. `jq -c .` prints `{"status":"ok","command":"skill_install","installed":["<RUN>/work/project/.claude/skills/sci-anim"],"version":"<the version in Cargo.toml>"}`. `diff -r skills/sci-anim "$RUN/work/project/.claude/skills/sci-anim"` prints nothing. `stat -c %a "$RUN/work/project/.claude/skills/sci-anim/scripts/fields"` prints `755`.
- **The installed skill works.** Run `"$RUN/work/project/.claude/skills/sci-anim/scripts/fields" arrow | head -3` with `target/release` first on `PATH`. It prints arrow's fields.
- **Again.** Add a file with `echo mine > "$RUN/work/project/.claude/skills/sci-anim/mine.md"` and remove `references/math.md` from the install. Run `$SKILL/scripts/run "$RUN" skill-install-again skill install --to "$RUN/work/project/.claude/skills"`. Exit 0, the same report. `diff -r` against `skills/sci-anim` prints nothing, and `ls -A "$RUN/work/project/.claude/skills"` prints only `sci-anim`.
- **Default skills folders.** `mkdir -p "$RUN/work/home/.claude"`. Run `HOME="$RUN/work/home" $SKILL/scripts/run "$RUN" skill-install-home skill install`. Exit 0, `.installed` is `["<RUN>/work/home/.claude/skills/sci-anim"]` and no `.agents` folder exists. With an empty `HOME` folder, `.installed` is `["<home>/.agents/skills/sci-anim"]`.
- **Project level.** `mkdir -p "$RUN/work/repo/.git" "$RUN/work/repo/.claude" "$RUN/work/repo/src" "$RUN/work/home/.claude"`. From `$RUN/work/repo/src`, with `HOME="$RUN/work/home"`, run `skill install --project` through the run helper (give it the absolute path of `run`). Exit 0, `.installed` is `["<RUN>/work/repo/.claude/skills/sci-anim"]`, and `$RUN/work/home/.claude` has no `skills`.
- **The question.** The run helper's stdout is no terminal, so drive the question in a pseudo-terminal with `script`, from `$RUN/work/repo` with `HOME="$RUN/work/home"`: `printf 'p\n' | script -qec "<repo>/target/release/sci_anim skill install" "$RUN/skill-install-ask-p.typescript"; echo $?`. The typescript shows `Install the sci-anim skill for this repository (<RUN>/work/repo: .claude/skills) or for your user (~/.claude/skills)? [p/u]`, then the ok report naming the project level folder, exit 0. With `u`, the report names `<RUN>/work/home/.claude/skills/sci-anim`. With `printf 'maybe\np\n'` the question shows twice, then project level. With `printf ''` (the end of stdin) the question shows once, the report is an `unanswered` error, exit 1, and no skill folder is written.
- **No terminal.** From `$RUN/work/repo`, `run ... skill install` with no flag: no question, exit 0, user level. From a folder with no `.git` above it, such as a `mktemp -d` outside the checkout, the same: user level, and inside a terminal too (`script -qec`) no question.
- **Conflicting flags.** Each of `--to X --user`, `--to X --project` and `--user --project`: exit 2, `.errors[0].kind` is `usage` and the message says `cannot be used with`, and nothing is written. `--project` outside a repository: exit 2, `usage`.
- **Link refused.** Make a folder with a `SKILL.md` that says `name: sci-anim` and link `<skills folder>/sci-anim` to it. Run `$SKILL/scripts/run "$RUN" skill-install-link skill install --to <skills folder>`. Exit 2. `jq -c '.errors[]|[.kind,.file]'` prints `["io","<skills folder>/sci-anim"]`, and `.message` names the target. The target's files are unchanged (`sha256sum` before and after), and the link is still a link.
- **Foreign folder refused.** Put a folder `sci-anim` with `SKILL.md` saying `name: other` in a skills folder and install into it. Exit 2, kind `io`, the folder unchanged.
- **Proof.** The `.stdout.json` and `.exit` files of each check, the `diff -r` and `stat` output, and the checksums of the link's target, in `$RUN`.

## Gotchas

- Inside this checkout, the repository is the worktree or checkout you run in, so a `--project` or a `p` answer from there writes into the checkout's own `.claude/skills`. Drive project level in a repository under `$RUN/work/`, never in the checkout, and never commit an installed skill.
- The question goes to stderr, so the report on stdout stays one JSON value. `script` merges both into its typescript.

- The report's paths are as `--to` gave them, so a relative `--to` gives a relative path.
- The binary carries the skill from its build. After editing `skills/sci-anim/`, rebuild before you install, or the install is the old skill. doctor flags a skill file newer than the binary.
- An install that stops part way leaves `<skills folder>/.sci-anim.installing`. The next install removes it first.
