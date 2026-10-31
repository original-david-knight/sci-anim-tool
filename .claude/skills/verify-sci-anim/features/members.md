# Animate one member of an object

A `move`, `fade`, `recolor` or `highlight` that names a `copy` beside its `object` changes that copy of a pattern alone, and a `recolor` or `highlight` that names a `bar` changes that bar of a bar plot alone. A copy's move is in the pattern's own space, so a move of the whole pattern still carries it. `inspect` reports each copy's `position`, `opacity`, `colors`, `glow` and `glow_color`, and each bar's `fill`, `stroke`, `glow` and `glow_color`. `check-layout` checks a moved copy where it has moved.

## Sub-features

- `member-copy-move` moves copies apart one after another, each to its `to` in the pattern's own space, while the others stay.
- `member-copy-highlight` lights copies with a see-through box around each.
- `member-bar` lights one bar and recolors it, the other bars unchanged.
- `member-errors` refuses a copy index past the copies, a bar index past the bars, a `copy` of an object that is no pattern and a `bar` of one that is no bar plot, each with exit 1 at its field.
- `member-same` every scene without a member draws as before.

## How to get to it (user POV)

- Write `"copy": [i, j, k]` (a grid) or `"copy": [i]` (a ring or an along) in a `move`, `fade`, `recolor` or `highlight` of a pattern, or `"bar": k` in a `recolor` or `highlight` of a bar plot. `sci_anim validate`, `inspect`, `frame` and `check-layout` read it.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. `skills/sci-anim/examples/unzipping_ladder.json` unzips its seven rungs from scene time 3.5 to 7.5, the top rung first, then lights copies `[0, 1, 0]` to `[0, 3, 0]` of `left_nucleobases` from 7.5 to 8.5.

- **Unzip.** `$SKILL/scripts/run "$RUN" member-copy-move-validate validate skills/sci-anim/examples/unzipping_ladder.json`, exit `0`. Then `frame` it at `--scene-time 3.4`, `5.5` and `10` into `$RUN`. View them: whole rungs at 3.4, the top rungs split and the bottom ones whole at 5.5, every rung split at 10.
- **Moved and unmoved copies.** `$SKILL/scripts/run "$RUN" member-copy-move-inspect-5.5 inspect skills/sci-anim/examples/unzipping_ladder.json --scene-time 5.5`. `jq -c '[.objects.left_nucleobases.pattern.copies[] | .position[0]]'` gives about `-2.0` for the top rungs, which have moved, and `-0.8` for the bottom ones, which have not.
- **Lit codon.** At scene time 9.5, `jq -c '[.objects.left_nucleobases.pattern.copies[] | .glow]'` is `[0,1,1,1,0,0,0]`, and the frame shows a box around each of the three.
- **One bar.** Copy `examples/bar_plot.json` into `$RUN` and add a step `{ "type": "play", "duration": 1, "animations": [ { "type": "highlight", "object": "plastic", "bar": 7, "to": 1 }, { "type": "recolor", "object": "plastic", "bar": 7, "fill": "#fc6255" } ] }`. `inspect` at its end: `jq -c '[.objects.plastic.bar_plot.bars[] | [.fill, .glow]]'` has `["#fc6255",1]` last and `["#58c4dd",0]` for every other bar. The frame shows 2020's bar red in a yellow frame.
- **Layout.** `$SKILL/scripts/run "$RUN" member-copy-move-check check-layout skills/sci-anim/examples/unzipping_ladder.json --no-sheet`, exit `0`. Remove the `unzipping_ladder-check/` folder it writes beside the scene.
- **Errors.** In scenes in `$RUN`: a `highlight` of `left_nucleobases` with `"copy": [0, 7, 0]` is `unknown_copy`, a `highlight` of `plastic` with `"bar": 8` is `unknown_bar`, a `move` of `title` with `"copy": [0]` is `not_pattern`, and a `recolor` of `title` with `"bar": 0` is `not_bar_plot`, each exit `1` at `/timeline/<step>/animations/<k>/copy` or `/bar`.
- **Same as before.** `frame` every example in `examples/` and `skills/sci-anim/examples/` at a few scene times with this build and with the `sci_anim` on PATH, and `cmp` the PNGs: every pair is byte-identical.
- **Proof.** The `.stdout.json` and `.exit` files and the frames in `$RUN`.

## Gotchas

- A copy's `to` is in the pattern's own space: a grid copy's place leaves out the pattern's `position`. Read the place from the scene file's layout, not from `inspect`, which reports the world position.
- A `bar` in a `move` or `fade` is `unknown_field`: a bar is placed by its value.
- `check-layout` writes `<scene>-check/` beside the scene. Remove it from `skills/sci-anim/examples/` after the run.
