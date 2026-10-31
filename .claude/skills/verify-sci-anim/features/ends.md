# Place lines, arrows and springs by their ends, and dash strokes

A line's or an arrow's `from` or `to` may sit on an object, as `{ "object": "block", "offset": [0, -1.5, 0] }`: the object's world position plus an offset in world units. It may instead sit on a tube's curve at a `u`, as `{ "object": "chain", "u": 1.6 }`, on a plot's curve at an `x`, as `{ "object": "wave", "x": 1 }`, or on an atom of a molecule, as `{ "object": "at_pair", "atom": 13 }`. The shape is then placed by its ends and lies between them at every scene time wherever the object goes, an arrow's tip keeping its length. A `spring` is a coil of `turns` turns of `radius` between two ends written the same way, which stretches and squashes and keeps its turns. A shape placed by its ends has no transform of its own, is hidden while either end's object is, and `inspect` reports each end's world point under `ends`. A stroke may have a dash pattern, `"dash": [0.08, 0.06]`, so a hydrogen bond is a dashed line between two atoms.

## Sub-features

- `ends-arrow` keeps a force arrow on a block that moves, its tip unstretched.
- `ends-spring` stretches a spring from a fixed point to a mass that a law moves.
- `ends-hidden` hides a shape placed by its ends while an end's object is at opacity 0 or waiting for its create.
- `ends-data-point` sits an end on a dot on axes, where the axes put it.
- `ends-errors` refuses a `position` of a shape placed by its ends (`placed_by_ends` at `position`), a move or rotate of one (`placed_by_ends` at `object`), an end on no object (`unknown_object` at `.../from/object`) or on a plot (`invalid`), a linear map (`not_mappable`), morph (`placed_by_ends`) or carry along one (`invalid`), an end that is one number (`wrong_length` at the end), an end on a trail (`invalid` at `.../from/object`), two fixed ends at one point (`invalid` at `to`), and a spring of more than 100 `turns` (`out_of_range` at `turns`).
- `ends-curve` sits an end on a tube's space curve at a `u` (sci_anim 0.13.0 or later): cross-links between two chains stay on their `u` as the chains' shape laws run and the tubes move and turn.
- `ends-plot` sits an end on a plot's curve at an `x` of its axes, following the plot's shape law and its axes.
- `ends-curve-errors` refuses a `u` outside the curve's u range (`out_of_range` at `.../from/u`), an `x` outside the axes' x range (`out_of_range` at `.../to/x`), a `u` on an object with no space curve, such as a sphere, or on a plot, and an `x` on a tube (`invalid` at `.../from/object`), and a morph of a plot an end sits on (`not_morphable` at `.../object`).
- `ends-atom` sits an end on an atom of a molecule by its atom reference (sci_anim 0.14.0 or later): hydrogen bonds stay on their atoms as nucleobase pairs turn, and on two molecules as one moves away.
- `ends-atom-reaction` hides an end on a reactant's atom from the reaction's start and shows one on a product's atom from its end.
- `ends-atom-errors` refuses an atom past the molecule's atoms (`no_such_atom` at `.../from/atom`), atom 0 (`out_of_range` at `.../atom`) and an `atom` on an object that is not a molecule (`invalid` at `.../from/object`).
- `dash` draws a stroke with a dash pattern `[dash, gap]` in world units, its dashes anchored at the start of each part of the path, an arrow's tip whole.
- `dash-errors` refuses a dash or gap of 0 or less (`out_of_range` at `.../stroke/dash/0` or `/1`), one length (`wrong_length` at `.../stroke/dash`), and a dashed trail (`invalid` at `.../stroke/dash`).
- `ends-hint` answers the type `coil` with a hint to `spring`.
- `ends-layout` runs `check-layout` on the forces lesson.

## How to get to it (user POV)

- Write `"from": { "object": "<id>", "offset": [x, y, z] }` or the same for `to` on a `line` or an `arrow`, or a `spring` with `from` and `to`, then run `validate`, `inspect`, `stages`, `frame`, `render`, `frames` or `check-layout`.
- `skills/sci-anim/examples/forces.json` slides a block down an incline with its weight, normal and friction arrows on it, then bounces a mass on a spring with an arrow from the mass to its rest point.
- `sci_anim schema | jq '.["$defs"].EndFile, .["$defs"].OnObjectFile, .["$defs"].OnCurveFile, .["$defs"].OnPlotFile, .["$defs"].OnAtomFile'` prints the end's forms, and `.["$defs"].Stroke.properties.dash` the dash pattern.
- `skills/sci-anim/examples/nucleobase_pairs.json` turns an A-T pair and a G-C pair, from `structures/at_pair.sdf` and `gc_pair.sdf`, joined by dashed hydrogen bonds.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok` for the frames.

- **Arrow.** `validate skills/sci-anim/examples/forces.json`: exit 0, duration 19.5. `inspect -t 4.5` and `-t 7.5`: `.objects.gravity.ends.from` is the block's `position` each time, about `[-4.3, 1.21, 0.01]` and `[-1.3, -0.52, 0.01]`, and `.ends.to` is 1.6 below it. Draw frames at 4.5 and 7.5 and view them: the three arrows and their captions sit on the block at the top of the slope, then lower down it.
- **Tip.** Write `$RUN/pull.json`: a square `block` moved from the origin to `[3, 0, 0]`, and an arrow `pull` from `[-3, 0]` to `{ "object": "block" }` with `tip_length` 0.3. `inspect -t 0` and `-t 1` give `.objects.pull.ends.to` `[0, 0, 0]` and `[3, 0, 0]`, `.scale` `[1, 1, 1]` both times, and a world `bounding_box` as tall at both, since the tip keeps its size.
- **Spring.** In forces.json, `inspect -t 10.5` and `-t 11.55`: `.objects.spring.ends.to` is the mass's position plus `[0, 0.4, 0]`, about `[0, 0.7, 0.01]` and `[0, -0.9, 0.01]`. Draw both frames and view them: the coil runs from the ceiling to the mass's top, short and dense, then long and open, with nine turns each time.
- **Hidden.** In `$RUN/pull.json`, add a fade of `block` to 0 over 1 s: `inspect -t 1` gives `.objects.pull.opacity` 0. Write `$RUN/uncreated.json`: a stroked square `block` created in a play from 1 s to 2 s, after a wait, and a spring from `[-4, 0, 0]` to `{ "object": "block" }`: `inspect -t 0.5` gives `.objects.spring.opacity` 0 and `-t 1.5` gives 1. Draw both frames and view them: no spring before the block, then the spring to the square as it is drawn.
- **Data point.** Axes of x and y range `[0, 4]`, 4 by 4, a dot `p` on them at `[1, 1, 0]`, and a line from `[3, 3]` to `{ "object": "p" }`: `inspect -t 0` gives `.ends.to` `[-1, -1, 0]`.
- **Errors.** In `$RUN`: an arrow with an end on an object and a `position` gives `placed_by_ends` at `/objects/<id>/position`; a rotate of such a line gives `placed_by_ends` at `.../object`; an end on `nowhere` gives `unknown_object` at `/objects/<id>/from/object`; a spring from a `function-plot`, and an arrow from a `trail`, give `invalid` there; a `linear-map`, a `morph` into, and a follow `along` such a line give `not_mappable`, `placed_by_ends` and `invalid`; `"from": [0]` gives `wrong_length` at `/objects/<id>/from`; a spring with `"turns": 1e30` gives `out_of_range` at `/objects/<id>/turns`; a spring or an arrow from `[1, 1, 0]` to `[1, 1, 0]` gives `invalid` at `/objects/<id>/to`. Each exits 1.
- **Curve.** `validate skills/sci-anim/examples/cross_links.json`: exit 0, duration 17.5. `inspect -t 2.5`, `-t 5.5` and `-t 8`: `.objects.cross_link_1.ends.from` is `chain_a`'s position plus its curve at u = -4 and that law time, `[-4, 1.3 + 0.3 sin(-7.6 - 2.4 t), 0.3 cos(-5.2 + 1.7 t)]` before the move, with `t` the scene time less 2.5. Draw the three frames and view them: the five cross-links run from one chain to the other at each, as the chains wiggle, `chain_a` rises and `chain_b` turns about x. A tube through `points` `[[0,0,0],[1,0,0],[2,0,0]]` with an end at u 0.5 gives `.ends.from` `[1, 0, 0]`, the middle curve point.
- **Plot.** In cross_links.json, `inspect -t 12` and `-t 13`: `.objects.rise.ends.to` is the data point (1, 1.5 sin(2 - 3 t)) of `ax`, with `t` the scene time less 10.5, and `.ends.from` is `foot`, on the x axis below it. Draw the frame at 13 and view it: the arrow runs straight up or down from the x axis to the wave.
- **Curve errors.** In `$RUN`: an end at u 2.5 on a tube of `u_range` `[0, 2]` gives `out_of_range` at `/objects/<id>/from/u`; an end at `x` 5 on a plot of axes with x range `[0, 4]` gives `out_of_range` at `.../to/x`; a `u` on a sphere or on a plot, and an `x` on a tube, give `invalid` at `.../from/object`; a morph of a plot an end sits on gives `not_morphable` at `/timeline/0/animations/0/object`. Each exits 1.
- **Atom.** `validate skills/sci-anim/examples/nucleobase_pairs.json`: exit 0, duration 14. `inspect -t 3.5`, `-t 6` and `-t 12`: `.objects.at_bond_1.ends.from` and `.ends.to` equal `.objects.at_pair.atoms[12].position` and `.atoms[20].position`, atoms 13 and 21, and each G-C bond's ends equal its two atoms the same way. Draw the three frames and view them: two dashed bonds across A-T and three across G-C, face on, then edge on, then tilted, each from a white hydrogen to a blue nitrogen or a red oxygen.
- **Two molecules.** In `$RUN`, a water molecule and an ethanol from `tests/fixtures/pubchem`, a dashed line from water's atom 2 to ethanol's atom 1 and a move of the water away: `inspect` at the move's start and end puts `.ends.from` and `.ends.to` on those atoms' positions. Draw both frames and view them: the dashes keep their length as the line stretches.
- **Reaction.** The hydrochlorination of ethylene from `tests/ends.rs` with a line on `hcl`'s atom 1 and one on `chloroethane`'s atom 1: `inspect` before, during and after the reaction gives their opacities `[1, 0]`, `[0, 0]` and `[0, 1]`.
- **Atom errors.** In `$RUN`: an end at atom 4 of water gives `no_such_atom` at `/objects/<id>/from/atom`; an `atom` on a sphere gives `invalid` at `.../from/object`; atom 0 gives `out_of_range` at `.../to/atom`; a dash `[0, 0.1]` gives `out_of_range` at `/objects/<id>/stroke/dash/0`, `[0.1, -0.1]` at `.../dash/1`, `[0.1]` `wrong_length` at `.../stroke/dash`, and a dashed trail `invalid` at `.../stroke/dash`. Each exits 1.
- **Atom layout.** `check-layout skills/sci-anim/examples/nucleobase_pairs.json --out "$RUN/layout"` exits 0 and prints `No problem at any of 13 checked scene times.`
- **Same as before.** Draw `skills/sci-anim/examples/forces.json` at 4.5 and 10.5 and `examples/curves.json` at its end with this build and with the `sci_anim` on PATH: `cmp` finds each pair identical.
- **Hint.** An object of type `coil` gives `unknown_value` at `/objects/<id>/type`, whose message says `a coil between two points is a \`spring\``.
- **Layout.** `check-layout skills/sci-anim/examples/forces.json --allow gravity_caption,incline --out "$RUN/layout"` exits 0 and prints `No problem at any of 31 checked scene times and 481 frame times of follows.`
- **Proof.** The reports, frames and a sentence per frame in `$RUN`.

## Gotchas

- An offset is along the world's axes, not the object's: it does not turn with the object. An end on a curve or an atom does turn with it.
- An end on an atom sits on its center, inside its ball, so a hydrogen bond shows between the two balls.
- An end on a tube's curve sits on the curve's middle, inside the tube, so the line shows only between the two surfaces.
- A caption attached to a block is not hidden with its arrows. Fade it with the block.
- `check-layout` flags a caption over the shape its arrow points into. Pass `--allow` for that pair when the overlap is the picture.
