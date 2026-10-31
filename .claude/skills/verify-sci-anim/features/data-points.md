# Place data points on axes

A `dot` that names 2D axes in its `"axes"` field is a data point: its `position`, and every `move`, `follow` and carry of it, is in the axes' data coordinates, `[x, y, z]` with x and y as a plot on the axes writes them and z the distance off their plane in world units. The axes carry it wherever they go, and its radius stays in world units, so it stays round however they stretch. `inspect` reports its world `position` and `dot.center`, and `data_point` with its `axes` and data `position`. An object attached to it sits on it on the frame. A dot that names 3D axes, a number line or another object, and a sphere that names 2D axes, are refused at `axes`. The sphere on 3D axes is the 3D data point, whose recipe is in `axes-3d.md`.

## Sub-features

- `data-points-place` stands a dot at its data position, where the plot on the same axes has that point.
- `data-points-axes-move` moves and scales the axes, and the dot goes with them and stays the same size.
- `data-points-carry` carries a dot along a plot on its axes, and `carry_jump` names the data position to declare.
- `data-points-attach` centers a text attached to a dot on axes on the dot's pixels.
- `data-points-inspect` reports `data_point.axes` and `data_point.position` beside `dot.center`.
- `data-points-errors` refuses a dot on 3D axes, on a number line or on another object, a sphere on 2D axes, and a dot with `axes` in a pattern's shape, with `invalid` at `axes`, all in one report; and a linear map, morph or carry along a dot on axes.

## How to get to it (user POV)

- Give a `dot` an `"axes"` field that names an `"type": "axes"` object, then run `validate`, `frame`, `inspect`, `stages`, `render`, `frames` or `check-layout` on the scene file.
- `skills/sci-anim/examples/derivative.json` puts the point of tangency and the sliding second point on the axes as data points, and moves the second one in data coordinates.
- `sci_anim schema | jq '.["$defs"].ObjectFile.allOf[].then | select(.properties.type.const == "dot") | .properties.axes'` prints the field's description.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.

- **Place.** Write `$RUN/dot.json`: `"ax": { "type": "axes", "x": { "range": [-1, 3] }, "y": { "range": [0, 9] }, "width": 6, "height": 5 }`, `"parabola": { "type": "function-plot", "axes": "ax", "expression": "x^2", "x_range": [1, 3] }` and `"p": { "type": "dot", "axes": "ax", "position": [1, 1, 0], "radius": 0.08, "fill": { "color": "#ffd23f" } }`, with no steps. `validate` exits 0. `frame` at 0 and view the PNG: the yellow dot sits on the parabola's left end. `inspect` gives `.objects.p.position` `[0, -1.944, 0]`, x 1 at the middle of x's range and y 1 at 5/9 of a unit per data unit below y's middle. Add a world dot `w` and a follow of `w` `along` `parabola`: `validate` gives `carry_jump` naming `[0, -1.9444, 0]`, the plot's own world point at x = 1.
- **Axes move.** Add a play of 1 s with a `move` of `ax` to `[1, 0, 0]` and a scale of `ax` to `[2, 2, 1]`. `inspect` at 1: `.objects.p.position` is `[1, -3.889, 0]`, the axes' new middle plus twice the offset from it, and `.bounding_box.world` is still 0.16 across in x and in y. Draw frames at 0 and 1 and view them: the dot stays on the stretched parabola's end and stays round.
- **Carry.** Replace the steps with a follow of `p` `along` `parabola` with `"easing_curve": "linear"` over 2 s. `validate` exits 0. `inspect` at 1, 1.5 and 2: `.objects.p.data_point.position` has y = x² at each, and `[3, 9, 0]` at 2. Draw frames at 0, 1 and 2 and view them: the dot runs up the curve. Declare `p` at `[0, 0, 0]`: `validate` gives `carry_jump` at `/timeline/0/animations/0/along` whose message says to declare `p`'s position as `[1, 1, 0]`.
- **Attach.** Add `"n": { "type": "text", "text": "P", "size": 0.3, "attach": { "object": "p", "offset": [0, 0] } }`. `inspect` at 0: the centers of `.objects.n.bounding_box.pixels` and `.objects.p.bounding_box.pixels` are within about a pixel, the P's ink sitting a little right of its middle, and `.objects.n.attached.object` is `"p"`.
- **Inspect.** `inspect $RUN/dot.json --scene-time 0`: `.objects.p.data_point` is `{ "axes": "ax", "position": [1, 1, 0] }`, and `.objects.p.dot.center` equals `.objects.p.position`.
- **Lesson.** `validate skills/sci-anim/examples/derivative.json` exits 0, and `frame` at 8, 15, 21 and 24.5 shows the yellow point at (1, 1) and the red one on the curve at (2.5, 6.25), (1.5, 2.25) and nearer (1, 1). `$SKILL/scripts/run "$RUN" data-points-layout check-layout skills/sci-anim/examples/derivative.json --ends-only --no-sheet --out "$RUN/work/layout"` reports no problem at 22 checked scene times.
- **Errors.** Write `$RUN/errors.json` with 2D axes `flat`, 3D axes `space`, a number line `line`, a circle `ring`, dots naming `space`, `line` and `ring` in `axes`, a sphere naming `flat`, and a pattern whose shape is a dot naming `flat`. `validate` exits 1 with `invalid` at each dot's and the sphere's `/objects/<id>/axes` and at `/objects/<pattern>/shape/axes`, five errors in one report. With a second dot `q` on `ax`, a linear map naming `p` gives `not_mappable`, a morph of `p` into `q` gives `not_morphable` at `object`, and a follow of a world dot along `p` gives `invalid` at `along`.
- **Proof.** The PNGs, reports, and a sentence per PNG in `$RUN`.

## Gotchas

- A data point's position is in data coordinates, but `.position` and `dot.center` in `inspect` are in the world. Read the data position from `data_point.position`.
- Scale the axes along x and y only to see the plot stretch: z of a data point is in world units, so a z scale does not lift it.
- Caption a data point with a capital letter, as `P`, to check that its attachment centers on it. A letter with a descender sets its box lower.
- A `sci_anim` built before data points on axes gives `unknown_field` at `/objects/p/axes`. Drive `target/release/sci_anim`.
