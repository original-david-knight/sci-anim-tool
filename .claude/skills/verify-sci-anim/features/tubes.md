# Draw tubes along space curves

A scene file's `tube` carries a circle of `radius` along a space curve, whose `curve` writes `x`, `y` and `z` as expressions in `u` over `u_range`, or lists the `points` it passes through, smooth between them, or reads them as the atom `trace` of a PDB file. It shows as a lit solid with edges along its outline and a disc at each open end, and a curve that ends where it starts closes on itself with no ends. A create draws it along its curve by length. `inspect` reports its bounding box and, under `tube`, the world point and pixel at each end of its curve, and for a curve through points the world point of each curve point. Expressions, in a tube's curve and in a function plot, also take `atan2(y, x)`, `mod(a, b)` and `step(a)`, and every unknown name is a scene error at its own column.

## Sub-features

- `tubes-draw` draws a spring along a helix and a trefoil knot that closes with no seam or end discs.
- `tubes-create` draws a tube along its curve by length over a create's span.
- `tubes-inspect` reports `tube.start`, `tube.end`, their pixels, and the whole tube's bounding box mid-create.
- `tubes-errors` refuses an unknown name in each expression at its column, a curve with no value at some u, a `u_range` high first, a curve that is one point, and a type such as `helix` with a hint naming the tube.
- `tubes-functions` plots `step`, `mod` and `atan2` in a function plot, breaking the curve at each jump, and the schema describes each expression in its own variable.
- `tubes-layout` runs `check-layout` on a scene with tubes, and `stages.txt` names a tube by its curve.
- `tubes-points` draws a tube through curve points written as `points` (sci_anim 0.12.0 or later): a generated polymer coil, a carry along it with `faces: tangent`, copies `along` points, every bad points list at its path, and expression curves drawing as before.
- `tubes-trace` draws a tube along the atom trace of ubiquitin's alpha carbons (sci_anim 0.19.0 or later), through each alpha carbon where the molecule of the same file draws it, `inspect` reporting its 76 `curve_points`, and refuses an atom name no residue has, a chain the file lacks, an SDF file and a missing file, each at its path under `curve/trace`.

## How to get to it (user POV)

- Write an object with `"type": "tube"` in a scene file, then run `validate`, `frame`, `inspect`, `render`, `frames` or `check-layout` on it.
- `examples/tubes.json` draws a spring, squashes it under a box, then draws a trefoil knot and turns it while the camera moves.
- `skills/sci-anim/examples/polymer_coil.go` writes `polymer_coil.json`: a tangled chain of 300 curve points drawn as a tube, then an arrowhead carried along it from scene time 4.6 to 12.6.
- `sci_anim schema | jq '.["$defs"].ObjectFile.allOf[].then | select(.properties.type.const == "tube")'` prints the fields, and `jq '.["$defs"].SpaceCurveFile'` the curve's.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.

- **Draw.** Run `$SKILL/scripts/run "$RUN" tubes-draw-11 frame examples/tubes.json --scene-time 11 -o "$RUN/tubes-11.png"`. Exit 0. View the PNG: a blue spring under a green box, and a red trefoil knot, each lit from the upper left with thin dark outlines. The knot shows no line across it where its curve starts and ends.
- **Create.** Draw `--scene-time 1`, `2` and `3` into `$RUN`, and run `inspect` at 2. At 1 the spring shows nothing. At 2 `.objects.spring.created` is 0.5 and the frame shows the lower half of the spring, with a disc at its drawn end. At 3 it is whole.
- **Inspect.** At 2, `.objects.spring.tube.start` is `[-2.4, -2.4, 0]` and `.tube.end` is about `[-2.4, 0.11, 0]`, with `start_pixels` and `end_pixels` on the frame. `.bounding_box.world` spans the whole spring, though half is drawn.
- **Errors.** Write a scene file into `$RUN` with a tube whose `x` is `cos(s)` and whose `y` is `a*u + b`. (`t` is law time, which a curve may read, so it is not an unknown name.) `validate` exits 1 with three `bad_format` errors, one at `/objects/<id>/curve/x` and two at `/objects/<id>/curve/y`, each message naming the unknown name and its column. A tube with `x` of `sqrt(u)` over `[-1, 1]` gives `out_of_range` at `/curve/x` with `u = -1`, one with `u_range` `[1, 0]` gives `invalid` at `/curve/u_range`, and one with `x`, `y` and `z` all constants gives `invalid` at `/curve`. In a separate file, an object of `"type": "helix"` gives `unknown_value` at `/objects/<id>/type`, whose message names the `tube` to write. A `spring` is an object type of its own (see `ends.md`).
- **Functions.** A function plot with `expression` `step(x)` on axes from -2 to 2 inspects with `.plot.data_bounding_box` from y 0 to 1. Draw it beside `mod(x, 1) - 1` and `atan2(x, 1)` and view the frame: the step is flat at 0 and at 1 with no stroke up at x = 0, the sawtooth has no vertical strokes, and the arctangent is smooth. `sci_anim schema | grep -c 'atan2(y, x)'` is at least 1.
- **Video and layout.** `render examples/tubes.json -o "$RUN/tubes.mp4"` writes 714 frames per `ffprobe`. `$SKILL/scripts/run "$RUN" tubes-layout check-layout examples/tubes.json --out "$RUN/layout"` reports no problem, and `$RUN/layout/stages.txt` names `knot (tube)` with its `x =` expression.
- **Points.** Run `go run polymer_coil.go "$RUN/polymer_coil.json"` in `skills/sci-anim/examples/` and `cmp` it with the committed `polymer_coil.json`: they match. `validate` it: exit 0, so the arrowhead stands unturned at the first point, its first chord along +x. Draw `--scene-time 4.6`, `8.6` and `12.6`: a smooth blue tangle with no corners at its points, and the yellow arrowhead at the chain's left end, partway along, and at its last point, pointing along the chain each time. `inspect` at 12.6: `.objects.rider.position` is the last curve point. `check-layout` of it reports no problem, and `stages.txt` names `chain (tube)` as `through 300 points`. In a scene file in `$RUN`, a tube whose `points` hold one point gives `wrong_length` at `/objects/<id>/curve/points`, one point of two numbers `wrong_length` at `.../points/1`, a coordinate of 20000 `out_of_range` at `.../points/2/1`, all points equal `invalid` at `.../points`, and a `NaN` `invalid_json` at the empty path, since JSON has no NaN; each exits 1. A `pattern` `along` `{ "points": [[0, 0, 0], [1, 0, 0], [4, 0, 0]] }` with `count` 3 inspects with copies at x 0, 2 and 4. Last, draw `examples/tubes.json` at 11 with this binary and with the release before 0.12.0 (`SCI_ANIM=<path>`), and `cmp` the PNGs: expression curves draw the same pixels.
- **Trace.** Copy `skills/sci-anim/examples/structures/1ubq.pdb` into `$RUN/structures/`. Write `$RUN/trace.json` with a tube `backbone` of `radius` 0.4 in yellow and a `molecule` `ub` of `structures/1ubq.pdb`, both at `scale` `[0.12, 0.12, 0.12]` and the same `position`, the tube's `curve` `{ "trace": { "file": "structures/1ubq.pdb", "atom_name": "CA" } }`. `validate` exits 0. Draw it at 0 as `tubes-trace-frame` and view it: the yellow tube runs through the molecule's atoms along its chain, its ends at the molecule's two ends. `inspect` it as `tubes-trace-inspect`: `.objects.backbone.tube.curve_points` holds 76 points, the first equal to the `.objects.ub.atoms[]` entry of number 2 (MET 1's CA) and the last to number 599 (GLY 76's CA), and neighbors lie about 0.456 apart, 3.8 Å at scale 0.12. In another file in `$RUN`, a trace with `atom_name` `P` gives `invalid` at `/objects/<id>/curve/trace/atom_name`, `chain` `B` `invalid` at `.../trace/chain`, a `.sdf` file `invalid` at `.../trace/file`, and a missing `.pdb` `unreadable_file` at `.../trace/file`; it exits 1.
- **Unchanged.** Draw `examples/tubes.json` at 11 and `skills/sci-anim/examples/polymer_coil.json` at 8.6 with this binary and with the release before 0.19.0 (`SCI_ANIM=<path>`), and `cmp` the PNGs: they match.
- **Proof.** The PNGs, reports, and a sentence per PNG in `$RUN`.

## Gotchas

- A carry along a tube through points must start facing its first chord: when it does not run along +x, copy the `rotation` the `carry_jump` error at `faces` names.
- `u_range` is numbers, not expressions. A full turn written `6.28` leaves the ends apart and shows two end discs; write `6.283185`.
- A `scale` that is not even squashes the tube's circle, so a spring scaled in y has a flattened wire.
- A tube thicker than its tightest bend folds into itself there; that is the scene's fault, not a rendering bug.
- `check-layout` compares text beside a tube with the tube's box, so text inside a helix's box is reported even where the frame shows it in the gap between turns.
