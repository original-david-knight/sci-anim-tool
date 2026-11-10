# Move bodies

A `simulate` with `bodies` moves objects as rigid bodies in the world's xy plane under `gravity`: free, turning about a `pivot`, or `fixed`. Collisions are settled pair by pair, with each contact's `restitution` and `friction`. Each moving body's position and rotation are set for the span, its quantities (`x`, `y`, `angle`, `vx`, `vy`, `speed`, `spin`, `px`, `py`, `ke`) are variables the `sets` read as `<object id>.<quantity>`, and `validate` lists every collision under `collisions`.

## Sub-features

- `bodies-carts` collides two carts elastically and perfectly inelastically, with readouts of the momentum and kinetic energy the simulation computes and velocity arrows set from its quantities.
- `bodies-cradle` sends a Newton's cradle's strike along a row of touching pivoted balls.
- `bodies-ramp` slides a block down a fixed ramp with friction, force arrows riding it.
- `bodies-projectile` launches a ball that bounces on a fixed line, its velocity arrow turning.
- `bodies-pendulum` swings a pivoted bob into a block on a floor.
- `bodies-seesaw` balances a pivoted plank under two blocks, then tips it onto a floor with a heavier block.
- `bodies-collisions` reads the `collisions` of `validate`.
- `bodies-same` runs one scene twice and compares the reports byte for byte.
- `bodies-errors` refuses a body that is not a circle, dot, sphere or convex polygon (a line only when fixed), a fixed body with motion, a moving one with no mass, an unknown object, `bodies` beside `variables`, a set of a moving body's position, and a scaled body.
- `bodies-layout` runs `check-layout` on each lesson.

## How to get to it (user POV)

- Write `{ "type": "simulate", "bodies": { "<id>": { "mass": 1, "velocity": [3, 0] }, "<ground id>": { "fixed": true } }, "sets": [...] }` in a play step, then run `validate`, `inspect`, `stages`, `frame`, `render`, `frames` or `check-layout`.
- `examples/collisions.json` and the lessons `skills/sci-anim/examples/newtons_cradle.json`, `block_on_ramp.json`, `projectile.json`, `pendulum_strikes_block.json` and `seesaw.json`.
- `sci_anim schema | jq '.["$defs"].BodyFile'` prints a body's fields.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok` for the frames.

- **Carts.** `validate examples/collisions.json`: exit 0, duration 4.5, and `.collisions` holds `["a", "b"]` and `["c", "d"]`, both at scene time 2.3167 (a gap of 3.95 closed at 3 m/s from scene time 1). `inspect -t 3.5`: `.objects.elastic_momentum.readout.text` `6.00` and `.objects.elastic_energy.readout.text` `9.00`, both as at the start; `.objects.inelastic_energy.readout.text` `6.00`, a third of the 9 J lost; `.objects.b.position` x about 5.23, moving at 4 m/s. Draw frames at 1.5 and 3.5 and view them: before, a long arrow on the 2 kg cart; after, a short arrow on it and a long one on the 1 kg cart above, and the two lower carts moving together.
- **Cradle.** `validate skills/sci-anim/examples/newtons_cradle.json`: the first four `.collisions` are `ball_0`→`ball_1`, `ball_1`→`ball_2`, `ball_2`→`ball_3`, `ball_3`→`ball_4`, all at scene time 1.8917. Draw frames at 1.85, 2.5 and 4 and view them: the blue ball meets the row; then only the red ball swings out; then the strike comes back and the blue ball swings out.
- **Ramp.** `inspect skills/sci-anim/examples/block_on_ramp.json -t 4.3`: `.objects.speed.readout.text` `5.44`, 2.3 s at g(sin 25° - 0.2 cos 25°) = 2.366 m/s². Draw frames at 2 and 4.3: the block lower on the slope with its three arrows on it and its trail behind.
- **Projectile.** `validate skills/sci-anim/examples/projectile.json`: the first collision of `ball` and `ground` at scene time 2.53, 1 + 2·7.5/9.8. Draw frames at 1.8, 2.53 and 4.2: the arc, the bounce, two lower arcs.
- **Pendulum.** `validate skills/sci-anim/examples/pendulum_strikes_block.json`: one collision of `block` and `bob` at 1.915. `inspect -t 4.5`: the block well right of its start at x -0.3, slowing on its friction. Draw frames at 1.5, 1.91 and 2.6.
- **Seesaw.** `inspect skills/sci-anim/examples/seesaw.json -t 3`: `.objects.tilt.readout.text` `0.0`, balanced; `-t 5.6`: tilt about `-19.6`, the plank resting on the floor. Draw frames at 2.5 and 5.6.
- **Same.** Run `stages skills/sci-anim/examples/seesaw.json --from 0 --to 5.6` twice into two files and `cmp` them: no difference.
- **Errors.** In `$RUN`, a simulation with `"bodies"` naming a `text` gives `invalid` at `.../bodies/<id>`; a concave polygon `invalid`; a moving `line` `invalid`; a body with no `mass` `missing_field` at `.../bodies/<id>/mass`; `"fixed": true` with a `mass` `invalid` at `.../mass`; an unknown id `unknown_object`; `bodies` beside `variables` `invalid` at `.../variables`; a set of a moving body's `position` `invalid` at `.../sets/0/position`; a body declared with `"scale": [2, 2, 1]` `invalid` at `.../bodies/<id>`; `"ball.velocity"` in a set `bad_format`. Each exits 1.
- **Layout.** `check-layout` each lesson with `--out "$RUN/layout-<name>"`: exit 0.
- **Proof.** The reports, frames and a sentence per frame in `$RUN`.

## Gotchas

- A body starts where its object stands when the span starts, at rest unless given a `velocity`, so declare a pendulum's bob already pulled aside, and a block touching its ramp.
- A collision takes the smaller restitution and the larger friction of its two bodies, so a coefficient written on one body decides.
- The pivot of a plank on a fulcrum is at the plank's middle; a fulcrum drawn under it is not a body, or the plank would rest on it.
