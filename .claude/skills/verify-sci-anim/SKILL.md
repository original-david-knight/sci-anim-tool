---
name: verify-sci-anim
description: Drive the real sci_anim command-line tool in this repo (validate, frame, inspect, stages, render, frames, check-layout and skill install) on real scene files and keep proof of what it did. Use when you need to show that a change to sci_anim works the way an agent uses it, beyond cargo test: a new scene-file field, a rendering fix, a report change, an exit code, a frame sequence, a video with sounds.
---

# Verify sci_anim

sci_anim is a short-lived CLI. Each command reads one scene file, prints one JSON report on stdout, and exits 0 (did it), 1 (the scene file is at fault) or 2 (anything else). `check-layout` instead prints a layout fault a line and exits 0 (none), 1 (it printed one) or 2 (it could not check, with the error report on stderr). There is no server. The primary surface is the `sci_anim` binary. The secondary surface is `skills/sci-anim/`, the skill that teaches agents to use it, which the binary carries and `sci_anim skill install` writes into a skills folder.

Use the project's words in anything you write from a run (see `GLOSSARY.md`): a *frame*, not a screenshot; a *video*, not an output; a *render*, not an export; *scene time*, not t; *object id*, not name; *sound*, not audio.

The feature map is in `features/README.md`. Read it, then the file for the feature you are proving. Paths below are relative to the repo root unless they start with `$SKILL`.

```sh
SKILL=.claude/skills/verify-sci-anim   # in a worktree, the same path under the worktree root
```

## Launch

Build the binary once per change. The helpers drive `target/release/sci_anim` of the checkout they sit in.

```sh
cargo build --release      # ready when it prints "Finished `release` profile"
```

Then make a run folder for this verification. It prints the folder's path; keep it in `RUN`.

```sh
RUN=$($SKILL/scripts/new-run <feature-id>)    # e.g. new-run frames-transparent
```

`new-run` records HEAD, the binary's `--version` and how many source paths are uncommitted in `$RUN/run.txt`. Evidence goes in `$RUN`. Scratch files you will not cite (scene variants, throwaway frames) go in `$RUN/work/`.

The run folder root is `$SCI_ANIM_EVIDENCE`, default `~/.cache/sci_anim-verify/`. It is outside the repo, so `cargo clean` and `git clean` never eat it.

Teardown: there is nothing to stop after a foreground command. For a background render, see Cleanup.

## Doctor

Run this first, and again whenever a result looks wrong:

```sh
$SKILL/scripts/doctor
```

It is read-only for the repo. It checks that the binary exists, that `--version` names the HEAD commit, that no file in `src/`, `skills/`, `build.rs`, `Cargo.*` or `assets/` is newer than the binary, that `ffmpeg`, `ffprobe` and `jq` are on PATH, and that a 64x64 scene validates and draws a frame on the GPU (in a `mktemp -d` folder it removes). It ends with `OK` (exit 0) or `NOT OK` (exit 1). A stale binary is the usual cause of a wrong result: rebuild and re-run doctor.

Uncommitted changes do not fail doctor. The binary then says `modified` after the commit, and the source-newer check is what tells you it is current.

## Drive

Run every sci_anim command through the `run` helper, so the command, report, stderr and exit code are all kept:

```sh
$SKILL/scripts/run "$RUN" <check> <sci_anim args...>
```

It writes `$RUN/<check>.cmd`, `<check>.stdout.json`, `<check>.stderr` and `<check>.exit`, appends a line to `$RUN/transcript.txt`, prints the report, and exits with sci_anim's exit code. Give each check a unique name, starting with the sub-feature id it proves. Write files the command makes (`-o`) into `$RUN` when they are evidence, into `$RUN/work/` when not.

```sh
$SKILL/scripts/run "$RUN" validate validate examples/timeline.json
$SKILL/scripts/run "$RUN" frame-1s frame examples/timeline.json --scene-time 1 -o "$RUN/timeline-1s.png"
$SKILL/scripts/run "$RUN" inspect-1s inspect examples/timeline.json --scene-time 1
jq '.objects.disc.position' "$RUN/inspect-1s.stdout.json"
```

Stable handles to assert on, never on line positions in the pretty-printed JSON:

- `.status` is `"ok"` or `"error"`. `.command` names the command on success.
- `.errors[]` has `.kind` (`scene`, `io`, `gpu`, `ffmpeg`, `usage`). A scene error also has `.path` (a JSON pointer such as `/objects/a/radius`), `.code` (such as `out_of_range`, `unknown_object`, `invalid_json`) and `.message`.
- The exit code: 1 if any error is a scene error, else 2.

Scenes to drive: `examples/*.json` (listed with what each shows in `README.md`), `skills/sci-anim/examples/*.json` (whole lessons), `tests/fixtures/` (structure files and a bad one). For a new feature, write a small scene file into `$RUN` (it is evidence of what you drove), or copy and change an example. `sci_anim schema` is the reference for every field: search it with `jq`, it is about 200 KB.

Concurrency: commands share nothing but the GPU and the paths you give them, so two agents can drive at once if their run folders differ. Two `cargo build`s in one checkout wait on each other's lock. A worktree under `.claude/worktrees/` has its own `target/`, so build there and run that worktree's helpers.

## Evidence

A proof keeps, in `$RUN`:

- every command with its report, stderr and exit code (the `run` helper does this), and `transcript.txt` in the order you ran them;
- the files the command made, checked by a second tool, not by trusting the report: `ffprobe` for a video's streams and frame count, `ls`/`file` for a frame sequence's PNGs, viewing a frame PNG (Read it if you can view images);
- for a visual claim, the frame before the change and the frame after (or at the scene times either side of an animation's span), not only the final one;
- the scene file you drove, if it is not a tracked example.

Proof standards:

- Drive the real binary with a scene file, the way an agent does. Calling a Rust function or a test helper is not proof of the user path; `cargo test` is a separate, earlier check.
- A report saying `"status": "ok"` is not proof that a file is right. Check the file: open the frame, `ffprobe` the video, count the PNGs.
- Prove the failure path too when the change touches it: the wrong scene file gives exit 1 and the error at the right `.path`; a missing or existing destination path gives exit 2 and leaves nothing behind.
- No mocks. ffmpeg and the GPU are real here. If either is missing, report the feature as not verified, not as passing.

## Cleanup

```sh
$SKILL/scripts/cleanup "$RUN"
```

It kills only the PIDs you wrote to `$RUN/pids`, and only while each is still a `sci_anim` or `ffmpeg` process, then removes `$RUN/work/` and lists what is kept. Start a background render like this so cleanup can find it:

```sh
target/release/sci_anim render s.json -o "$RUN/v.mp4" --updates > "$RUN/render.stdout.json" 2> "$RUN/render.stderr" &
echo $! >> "$RUN/pids"
```

Never `pkill sci_anim` or `killall ffmpeg`: another agent may be rendering. Cleanup never removes `$RUN` itself. After cleanup, give the run folder's path in your report; it is where the proof lives.

Run cleanup after a failed attempt too, then make a new run folder for the retry.

## Helpers

All in `$SKILL/scripts/`, all executable:

| Helper | Invocation | Does |
|---|---|---|
| `doctor` | `doctor` | Read-only health check of this checkout's binary, tools and GPU. |
| `new-run` | `RUN=$(new-run <feature-id>)` | Makes `$SCI_ANIM_EVIDENCE/<time>-<feature-id>-<pid>/` with `work/` and `run.txt`. |
| `run` | `run "$RUN" <check> <sci_anim args...>` | Runs sci_anim and keeps cmd, report, stderr, exit code. `SCI_ANIM=<path>` drives another binary. |
| `cleanup` | `cleanup "$RUN"` | Kills the run's recorded PIDs, removes `work/`, keeps the evidence. |
