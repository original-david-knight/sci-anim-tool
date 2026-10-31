# Validate a scene file

`sci_anim validate <scene file>` tells an agent whether a scene file is good before it renders anything. A good file gets an ok report with the scene's `duration`, each step's `path`, `start` and `end`, each chapter's `id`, `start` and `end`, and each sound's `file`, `at` and `start`. A bad file gets every scene error at once, each with the JSON pointer of the bad field.

## Sub-features

- `validate-ok` reports `duration` and the scene time each step starts and ends.
- `validate-chapters` reports chapter spans, with a chapter's least duration holding the stage.
- `validate-sounds` reports each sound with the chapter and scene time it starts at.
- `validate-errors` lists every scene error with `.path` and `.code`, exit 1.
- `validate-io` reports an unreadable file as an `io` error, exit 2.

## How to get to it (user POV)

- Run `sci_anim validate <scene file>`.
- `sci_anim frame`, `inspect`, `stages`, `render` and `frames` validate first and give the same scene errors, so a bad file fails them the same way.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set.

- **Ok report.** Validate a timeline example. Run `$SKILL/scripts/run "$RUN" validate-ok validate examples/timeline.json`. Exit 0. `jq '.duration, (.steps|length)' "$RUN/validate-ok.stdout.json"` prints `6.0` and `5`. Each step's `start` is the previous step's `end`.
- **Chapters and sounds.** Validate the narrated example. Run `$SKILL/scripts/run "$RUN" validate-sounds validate examples/narrated_swap.json`. Exit 0. `.chapters[].id` are `compare`, `swap`, `sorted`. `.sounds[]` names `voice/compare.mp3` at `compare`, with `.start` equal to that chapter's `.start` (1.5).
- **Every error at once.** Write a scene file with two unknown object ids: `echo '{"objects":{"a":{"type":"circle","radius":1}},"timeline":[{"type":"play","duration":1,"animations":[{"type":"fade","object":"nope","to":1},{"type":"fade","object":"ghost","to":1}]}]}' > "$RUN/two-errors.json"`. Run `$SKILL/scripts/run "$RUN" validate-errors validate "$RUN/two-errors.json"`. Exit 1. `jq -c '.errors[]|[.kind,.path,.code]'` prints `["scene","/timeline/0/animations/0/object","unknown_object"]` and the same for `/animations/1/object`.
- **Schema error.** Change `"radius":1` to `"radius":-1` in a copy. Exit 1, one error at `/objects/a/radius` with code `out_of_range`.
- **Not a scene file.** Run `$SKILL/scripts/run "$RUN" validate-not-json validate tests/fixtures/bad/insulin-404.sdf`. Exit 1, code `invalid_json` at path `""`.
- **Missing file.** Run `$SKILL/scripts/run "$RUN" validate-io validate "$RUN/work/none.json"`. Exit 2, `.errors[0].kind` is `io`.
- **Proof.** The `.stdout.json` and `.exit` files of each step, plus `two-errors.json`, in `$RUN`.

## Gotchas

- Schema errors come first. While the file breaks the schema, such as a negative radius, the reference errors, such as an unknown object id, are not listed yet. Prove "every error at once" with errors of one kind.
- Chapter and step times are sums of floats. Compare them with a tolerance, since `6.3999999999999995` is 6.4.
- A sound's file path is relative to the scene file's folder. Copying `narrated_swap.json` elsewhere breaks its sounds unless `voice/` comes too.
