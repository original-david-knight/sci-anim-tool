# Draw vector fields

A `vector-field` draws the electric field of `charge` field sources, or the magnetic field of `wire`, `loop` and `magnet` field sources, each placed by an object it names. It works the field out again at every scene time from where those objects stand, and draws it as field lines in its own plane, inside a rectangle `width` by `height`, or as arrows at the places of a grid. Moving or turning a source's object moves the field.

## Sub-features

- `fields-dipole` traces a dipole's field lines from the plus to the minus or the edge, symmetric above and below, and follows the minus as it moves.
- `fields-magnet` traces a bar magnet's field lines, every one closed through its body.
- `fields-wire` draws arrows circling a wire seen end on, none inside it, and the summed field of two wires of opposite current.
- `fields-faraday` moves a magnet and its field lines through a loop.
- `fields-loop` reads a loop's field on its axis at `seeds` against 2πIa²/(a² + z²)^(3/2).
- `fields-create` draws each line out from its start under `create`.
- `fields-errors` refuses an unknown object, an empty `sources`, a field source of the other field, a source on an object placed by another, a wire with no direction, a rectangle of no height, a line count of the other field, a `scale` of the field and a `scale` or `morph` of it.
- `fields-layout` runs `check-layout` on each lesson.

## How to get to it (user POV)

- Write objects drawn as the charges, wires, loops or magnets, then a `vector-field` with `of`, `sources` naming them and `draw`, and run `validate`, `inspect`, `frame`, `render` or `check-layout`.
- The lessons `skills/sci-anim/examples/electric_dipole.json`, `bar_magnet.json`, `wire_arrows.json` and `faraday_magnet_loop.json`.
- `sci_anim schema | jq '.["$defs"].ObjectFile'` and search for `"vector-field"`, or `skills/sci-anim/scripts/fields vector-field`.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok` for the frames.

- **Dipole.** `inspect skills/sci-anim/examples/electric_dipole.json -t 3.5`: `.objects.field.vector_field.line_count` 19, of which 12 have `ends_on` `minus`; every line's `to` is on the minus's edge, 0.3 from `[2, 0]`, or on the rectangle's edge (|x| 7 or |y| 3.2); for every line ending at `[x, y]` another ends at `[x, -y]`. `-t 6`: the lines ending on the minus end 0.3 from `[1, 1.2]`. Draw frames at 2, 3.5 and 6 and view them: lines half drawn out of the plus; every line drawn, tips toward the minus; the lines bent up toward the moved minus.
- **Magnet.** `inspect skills/sci-anim/examples/bar_magnet.json -t 4`: `line_count` 8, every `closed` true and `from` equal to `to`, each `from` at x 0 outside the bar (|y| > 0.4). Draw the frame at 4 and view it: nested loops, tips pointing from N round to S outside, lines visible through the bar.
- **Wire.** `inspect skills/sci-anim/examples/wire_arrows.json -t 3`: `.objects.one.vector_field.arrow_count` 104 of the grid's 105 places; the arrow at `place` `[-0.9, 0, 0]` has `toward` `[0, 1, 0]` and `strength` 1.818 (2I/ρ at ρ 1.1). `-t 8`: `.objects.two` has the arrow at `[0, 0, 0]` with `toward` `[0, 1, 0]` and `strength` 2, the two wires' fields added. Draw frames at 3 and 8 and view them.
- **Faraday.** `inspect skills/sci-anim/examples/faraday_magnet_loop.json` at 3.5, 5, 7.5 and 9: `.objects.bar.position` x -4.6, -2.3, 0.307 and 2.607, and `.objects.field.vector_field.lines[0].from` at the same x each time, with all 6 lines `closed`. Draw frames at 3.5, 5, 7.5 and 9: the lines go with the magnet through the loop, with no line popping in or out between frames.
- **Loop.** A scene of a `circle` of radius 1.5 turned `[0, 90, 0]` named by a `loop` of radius 1.5 and current 2, with `seeds` `[[0, 0], [1, 0], [3, 0]]`: `at_seeds` x 8.377, 4.826 and 0.749, y and z 0: 2πIa²/(a² + z²)^(3/2) at z 0, 1 and 3.
- **Create.** `frame skills/sci-anim/examples/electric_dipole.json -t 2`: the lines reach partway from the plus; at 3.5 they are whole.
- **Errors.** `cargo test --release --test vector_fields` holds each refusal; in `$RUN`, a charge on an unknown object gives `unknown_object` at `/objects/<id>/sources/0/object`, a `loop` in an electric field `invalid` at `.../sources/1/type`, a charge on a ray `invalid` at `.../object`, a wire `direction` of `[0, 0, 0]` `invalid` at `.../direction`, `"sources": []` `wrong_length` at `.../sources`, `"height": 0` `out_of_range` at `.../draw/height`, `count` on an electric field `invalid` at `.../draw/count`, a `scale` of the field `invalid` at `.../scale`, a `scale` animation of it `invalid` and a `morph` `not_morphable` at `.../object`. Each exits 1.
- **Layout.** `check-layout` each lesson with `--out "$RUN/layout-<name>"`: exit 0.
- **Proof.** The reports, frames and a sentence per frame in `$RUN`.

## Gotchas

- Field lines lie in the vector field's own plane and follow the field's part in it. A source out of the plane draws its shadow there, not a true field line.
- `inspect` measures an object's box whole while it is being created, so prove a `create` with frames, not with the bounding box.
- `check-layout` judges a vector field by the text its strokes run through, reported as "runs through", and lets it run off the frame, as it does a ray. In a scene with a vector field it checks every frame time of every animation.
- A field draws over objects at its own depth in no fixed order; the lessons set its z to 0.01 to draw over a magnet's fill.
