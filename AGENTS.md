# sci_anim

sci_anim turns a scene file into an animation of math or science. Read `GLOSSARY.md` before you write code, docs, or commit messages here. Read `docs/plan.md` for the work planned next.

## Use the glossary while you build

`GLOSSARY.md` defines the words this project uses. Use those words for every name you create: types, fields, functions, variables, scene-file keys, CLI flags, error messages, docs, and commit messages.

- A word under a term's _Avoid_ line never names that term's concept. Agents know manim, so manim's words leak in easily. Write `EasingCurve`, not `RateFunction`. Write `progress`, not `alpha`. Write `ObjectId`, not `name`.
- A word in parentheses after an avoided word limits when it is avoided. "line (a line is an object)" means `line` is wrong for a stroke, not wrong everywhere. Give an avoided word a note whenever it has proper uses too. Leave the note off only when every use of the word is wrong.
- If you need a concept the glossary does not have, add the term to `GLOSSARY.md` in the same change. Use the `domain-modeling` skill to write it. Do not invent a synonym for an existing term.
- If you change what a term means, update `GLOSSARY.md` and every use of the term in the same change.

## Check the glossary before review and before shipping

Run `scripts/glossary-check` before you commit, before you open a PR, and before you give a review or ship verdict. In pstack this covers the Feature playbook's verify step, `interrogate`, and each verifier in the Shipping playbook.

The script lists names in `.rs` and `.json` lines added since the branch left main, plus uncommitted and untracked files. Pass `--base <ref>` to compare with another ref, or `--all` to check every tracked file. Each hit names the avoided word and the term it is avoided for.

The script does not read prose or catch a new concept with no glossary term. It also skips every avoided word that has a note, because only a reader can tell the word's senses apart. Read the diff for all three yourself.

For each hit, either rename the name to the glossary term, or confirm the name does not mean that term's concept. A `line_count` that counts lines of a file is not a stroke.

A review or ship verdict follows these rules:

- `FAIL` if a name, doc, or message uses an avoided word for its term, or the change adds a concept with no glossary term.
- `PASS+NOTES` if every hit is a different meaning of the word. List each such hit and its meaning in the notes.
- `PASS` only if the glossary gives no reason for either of the above.

## Prove the change on the real binary before it reaches main

Every change reaches main through a pull request, the maintainer's too. The maintainer pushes a branch, opens the pull request with `gh pr create`, and turns on auto-merge with `gh pr merge --auto --squash`. GitHub merges it once the Linux and macOS checks pass. A contributor opens the pull request from a fork. Before you open a pull request with a change to `src/`, `assets/` or the scene-file schema, and before you give a review or ship verdict on one, follow `.claude/skills/verify-sci-anim/SKILL.md`. Build the release binary, run its `doctor`, and drive every entry point the change touches with the matching recipe in `features/`. In pstack this covers the Feature playbook's verify step and each verifier in the Shipping playbook. Passing `cargo test` is not this proof.

Put the run folder's path in the pull request's description, or in the verdict. The description becomes the body of the squashed commit. A ship verdict is `FAIL` if a touched feature was not driven, unless the run reports it as not verified with the doctor line that failed. A change that adds a user-facing behavior updates the feature map in the same change.

## Rules and what enforces them

When the operator corrects you, fix the mistake and add its rule here. If the rule is already here with nothing to enforce it, the correction is a repeat: enforce it with a test, a check or a hook in the same change. Drop a rule once its mistake can no longer happen.

| Rule | Enforced by |
|---|---|
| A name uses the glossary term, never an avoided word for it. | `scripts/glossary-check` lists the hits for avoided words with no note, and you decide each one. Prose, avoided words with a note and concepts with no glossary term are judged by reading the diff. |
| The sci-anim skill teaches every object type, animation type, command and flag in the same change that adds it. | `tests/skill.rs` reads the types from `sci_anim schema` and the flags from `--help`. |
| The feature map has a recipe for every new object or animation type. | `tests/skill.rs`. Its `NOT_YET_IN_THE_FEATURE_MAP` lists the older types that had none. The list can only shrink. |
| Every example scene, the README's scene and each whole scene in `docs/scene-files.md` validate. | `every_example_scene_validates` in `tests/frame.rs`, and `tests/readme.rs`. |
| `docs/cli.md` describes every command and flag in the same change that adds it. | `the_command_reference_names_every_command_and_flag` in `tests/skill.rs`. |
| `docs/scene-reference.md` is the schema as written by `scripts/scene-reference`. Edit the schema's descriptions, never the page, and run the script after a schema change. | `tests/docs.rs` |
| Every file the sci-anim skill names is in the skill, since it is installed as its folder alone. | `every_file_the_skill_names_is_in_the_skill` in `tests/skill.rs`. |
| A scene generator in the skill's examples writes the scene file committed beside it. | `each_scene_generator_example_writes_its_scene_file` in `tests/skill.rs`. |
| Every schema field and type has a description. | `tests/schema.rs` |
| The `sci_anim` on PATH is the build the skill describes. After a change to `src/` lands on main, run `cargo install --path . --locked`. Bump the version in `Cargo.toml` when a change adds a command or flag the skill teaches, and name it in the skill as the least version it needs. | Nothing checks the build on PATH since `check-layout` became a command of the binary. `sci_anim --version` names the version and the commit it was built from. |
| Prove a change on the release binary before it reaches main, and cite the run folder. | `doctor` refuses a binary older than HEAD. Driving the recipes and citing the run folder are judgment calls. |
| No Python in the project. Scripts and scene generators are written in Go. A script in `scripts/` is a shell script that builds and runs the Go file beside it, which uses only Go's standard library. | `tests/no_python.rs` fails on a tracked `.py` file, a Python shebang, or text that tells a reader to run Python, and names each offender. |
| Every change reaches main through a pull request whose `test` and `test-macos` checks pass. Nothing force-pushes or deletes main. | A ruleset on main in the GitHub repository. It has no bypass, so an agent pushing with the maintainer's credentials is held to it too. |
| No commit names a private word, such as one of the maintainer's machines or a tool built on sci_anim. | `.githooks/pre-push` blocks a push whose new commits name a word from `~/.config/sci_anim/private-words`, which lives outside the repository. Turn it on in a clone with `git config core.hooksPath .githooks`. Without the word list it does nothing. |
