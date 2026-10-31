# Contributing to sci_anim

To send a change, fork the repository, make the change on a branch of your fork, and open a pull request against `main`. For a large change, open an issue first so we can agree on the shape before you build it.

If an agent makes the change, point it at `AGENTS.md`. It holds the same rules as this file, written for an agent.

## Build and test

You need the tools from the README's install section, Go on `PATH` to run the tests and the scripts in `scripts/`, and `jq` on `PATH` for the scripts below. The tests run the skill's scene generators, which are Go programs, and each script in `scripts/` builds and runs the Go file beside it. CI uses Go 1.27.3. The project has no Python, and a script or scene generator you add is written in Go.

```sh
cargo build --release --locked
cargo test --release --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

CI runs all four. The code uses rustfmt's default layout, so run `cargo fmt` before you commit.

Many tests draw frames, so they need a GPU with a Vulkan driver. Mesa's lavapipe software driver also works, and CI uses it. Under lavapipe, `png_write_failure_cleans_owned_files_and_never_reports_finished` fails because lavapipe cannot create a device under `ulimit -f 0`, so CI skips that one test.

## Use the glossary's words

`GLOSSARY.md` defines the words this project uses. Use them for every name you add: types, fields, functions, scene-file keys, CLI flags, error messages, and docs. Each term lists the words to avoid for it. Many of those come from manim, so write `EasingCurve`, not `RateFunction`, and `progress`, not `alpha`.

If you need a concept the glossary does not have, add the term to `GLOSSARY.md` in the same pull request.

Before you open the pull request, run:

```sh
scripts/glossary-check
```

It lists each name in your change that uses an avoided word. Rename the name, or say in the pull request why it means something else.

## Prove the change on the release binary

`cargo test` passing is not enough for a change to `src/`, `assets/` or the scene-file schema. Drive the release binary on real scene files with the steps in `.claude/skills/verify-sci-anim/SKILL.md`:

1. Build with `cargo build --release`.
2. Run `.claude/skills/verify-sci-anim/scripts/doctor`. It checks that the binary is current and can draw a frame.
3. Follow the recipe in `.claude/skills/verify-sci-anim/features/` for each feature your change touches.
4. Put the run folder's path, and what you saw, in the pull request's description.

## Keep the skill and the feature map current

`skills/sci-anim/` is the skill that teaches an agent to use the tool. The binary carries it. A change that adds an object type, an animation type, a command or a flag also teaches it in the skill. A new object or animation type also gets a recipe in `.claude/skills/verify-sci-anim/features/`. `tests/skill.rs` fails when either is missing.

Every scene in `examples/`, the README's first scene, and each whole scene in `docs/scene-files.md` must pass `sci_anim validate`. The tests check this.

## Keep the docs current

The docs in `docs/` are for people who use sci_anim. Change them in the same pull request as the behavior they describe:

- `docs/cli.md` describes every command and flag. A new command or flag gets a line in its command's section. `tests/skill.rs` fails when one is missing.
- `docs/scene-reference.md` is generated from the schema's descriptions, so edit the descriptions in `src/`, not the page. After a change to the schema, run `scripts/scene-reference` and commit the page it writes. `tests/docs.rs` fails when the page and the schema differ.
- `docs/scene-files.md` explains how a scene file works. Change it when a change alters a rule it states, such as when an object shows.
- `examples/README.md` says what each scene in `examples/` shows. A new example scene gets a line there.

## License of contributions

Unless you state otherwise, any contribution you submit for inclusion in sci_anim, as defined in the Apache-2.0 license, is dual licensed under the Apache License, Version 2.0 and the MIT license, without any additional terms or conditions.
