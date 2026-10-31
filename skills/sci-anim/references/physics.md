# Physics: waves, forces, paths, readouts and motion by its rates

Read `instructional-videos.md` and `motion.md` first, and `math.md` for the plots and equations a physics lesson shows beside its picture. A physics lesson shows something changing over time, so most of what it draws is a `follow`, whose laws, `from` and carries `motion.md` covers.

`../examples/spring_wave.go` is a whole lesson: a mass bounces on a spring while a point plots its height, and the viewer reads amplitude and period off the wave it draws. Each feature below also has a demo in `../examples/`, named in `SKILL.md`: copy its syntax, not its pacing or framing.

## A shape that changes over time

A function plot's `expression`, a tube's `curve`, a surface's `z` and a pattern's `copy` expressions and `along` curve may read `t`, law time in seconds. The shape is then a shape law, and a follow with `"sets": "shape"` runs it:

```json
"wave": { "type": "function-plot", "axes": "ax", "expression": "sin(2*x - 3*t)" }
```

```json
{ "type": "follow", "object": "wave", "sets": "shape" }
```

- Before its first follow, the shape is drawn at `t` of 0, so a `create` draws the wave as it stands at 0.
- Law time counts from the follow's span, or from the start of the chapter `from` names, as a law's does. Write `"from"` on each follow of a motion that runs across several steps of a chapter.
- From the span's end on, the shape holds as it was at the span's end.
- A follow sets the shape's law time outright, so its law time must start where the shape is drawn: at 0 before its first follow, then where the last follow of it ended. A follow with no `from` starts at 0, so only the first follow of a shape leaves `from` out. A second follow names the chapter the first began in, and a chapter whose shape law starts again from 0 gets a shape of its own. `validate` reports `law_jump` at the follow with both law times.
- A follow of a shape takes no `law`, `along`, `faces` or `easing_curve`.
- `validate` evaluates the shape at every frame time of the follow's span. A space curve, surface or copy with no finite value there is `out_of_range` at that expression, and the message begins `at scene time 1 (law time 1)`. A plot draws a gap there, as it always does.
- A shape law cannot be morphed, and nothing can be carried `along` it.

| Picture | Write |
|---|---|
| A wave travelling right | `"expression": "sin(2*x - 3*t)"` |
| A standing wave | `"expression": "2*sin(2*x)*cos(3*t)"`, or two travelling waves and their sum as three plots |
| A term fading into a series over a second | `"expression": "sin(x) + min(max(t - 1, 0), 1)*sin(3*x)/3"` |
| A vibrating string | a `tube` with `"y": "0.3*sin(u)*cos(4*t)"` over `"u_range": [0, 3.1416]` |
| Ripples on a pond | a `surface` with `"z": "0.4*cos(3*sqrt(x^2 + y^2) - 4*t)*exp(-0.2*(x^2 + y^2))"` |
| A row of masses as a longitudinal wave | a `pattern` on a `grid` with `"copy": { "position": { "x": "x + 0.2*sin(x - 3*t)" } }` |

A surface that a follow runs is sampled at the same points at every frame, every point any of its frame times needs, so its triangles and edges stay put while it moves.

`check-layout` checks every frame time of the follow, so a wave that runs through a tick label is reported however briefly it does. A wave crosses the x axis by nature: give that axis a `spacing` wider than its range, so only the tick at 0 is labeled, rather than move the wave off its axis.

## Forces on a body: ends on objects

A line's or an arrow's `from` or `to` may sit on an object: `{ "object": "block", "offset": [0, -1.5, 0] }` is the object's world position plus an offset in world units along the world's axes. The shape is then placed by its ends, and lies between them at every scene time wherever the object goes:

```json
"gravity": { "type": "arrow", "from": { "object": "block" }, "to": { "object": "block", "offset": [0, -1.6, 0] } }
```

- A free-body diagram is a block with an arrow per force, each with both ends on the block. Move, follow or carry the block, and every arrow slides with it.
- An arrow's tip keeps its `tip_length` as the arrow stretches. An arrow from a body to a fixed point, as `"to": [0.7, -0.5]`, grows and shrinks with the body's distance from that point, so it shows a restoring force. It draws nothing while its ends meet.
- The offset does not turn with the object: an arrow on a block turned by 30 degrees still points where its offset says.
- An end that is a point, `[x, y]` or `[x, y, z]`, is a point in the world.
- An arrow that shows a changing vector, such as a ball's velocity, has its `from` on the body and its `to` on a small `dot` that a follow moves by its own law: the body's law plus the vector, scaled to world units. The dot must show, since an end on a hidden object hides the arrow, so give it the arrow's color and a radius about the stroke's width.
- A shape placed by its ends has no `position`, `rotation` or `scale` (`placed_by_ends`), and is not moved, followed, mapped, morphed or carried along. Fade, create and recolor it as any line.
- It is hidden while the object either end sits on is at opacity 0, or waiting for its `create`. A caption attached to the block is not: fade the captions with the block.
- An end with an `offset` sits on an object placed by its own transform, or on a data point: not on a plot, an attached object or another shape placed by its ends (`invalid`). A plot takes an end on its curve instead, below. A molecule takes an end on an atom too, `{ "object": "water", "atom": 2 }`, from sci_anim 0.14.0, which goes with the molecule as it turns (`chemistry.md`).
- `inspect` reports each end's world point under `ends`, as `from` and `to`, with the pixels they land on.

## Ends on a curve: cross-links and a point of a wave

From sci_anim 0.13.0 an end may sit on a curve. `{ "object": "chain", "u": 1.6 }` is the point of a tube's space curve at that `u`, and `{ "object": "wave", "x": 1 }` is the first point of a plot's curve at that `x` of its axes. The end keeps to its `u` or `x` while the tube or the axes move, turn and scale, and while a `follow` with `sets: shape` runs the curve's shape law, so a cross-link stays on the two chains it joins however they wiggle:

```json
"cross_link_1": { "type": "line", "from": { "object": "chain_a", "u": -4 }, "to": { "object": "chain_b", "u": -4.3 } }
```

- `u` is the curve's own: within its `u_range` for expressions, and from 0 at the first curve point to 1 at the last for `points`, so an end at the k-th of n points is at u = k/(n - 1) and stays on that point. A `u` outside the range is `out_of_range` at the end's `u`.
- `x` is in the axes' data coordinates, within their x range (`out_of_range` at the end's `x`). Where the plot is cut, as where it leaves the box, the end waits at the curve's point nearest that x.
- A `u` sits only on a tube, and an `x` only on a function plot or a point plot (`invalid` at the end's `object`, with a hint to the other form). A spring is placed by its own ends, so no end sits on its coil.
- An end on a curve sits on the curve's middle, inside a tube, so a line between two tubes shows from one surface to the other.
- A plot an end sits on is not morphed (`not_morphable`): fade it out and the other in.
- A cross-link joins two chains' tubes, as `../examples/cross_links.json` shows. A vertical arrow from a data point on the x axis to a travelling wave at the same `x` shows that one point of the medium only rises and falls.

## A spring between two ends

A `spring` is a coil of `turns` turns, at most 100, of `radius` about the line between its `from` and `to`, drawn as a tube of `tube_radius`. Its ends are written as a line's are, so a spring between a ceiling and a mass stretches and squashes as the mass moves, and keeps its turns:

```json
"spring": { "type": "spring", "from": [0, 2.6], "to": { "object": "mass", "offset": [0, 0.4, 0] }, "turns": 9, "radius": 0.25 }
```

Move the mass with a `follow`, as `"y": "-0.5 + 0.8*cos(3*t)"`, and the spring follows. A spring is lit, faded and created as a tube is.

`check-layout` reports a caption that sits on what its arrow points into, such as `mg` over the incline its block rests on. Pass `--allow gravity_caption,incline` when the overlap is the picture, as `../examples/forces.json` needs.

## The path a body leaves: trails

A `trail` draws the world points its `object` passed through, as a stroke that faces the camera and ends where the object stands:

```json
"orbit": { "type": "trail", "object": "planet", "length": 4, "stroke": { "color": "#58c4dd", "width": 0.04 } }
```

- `length` is how many seconds back it reaches. Leave it out for the whole path since the object began to show: from scene time 0, the start of its create, or the end of the reaction or morph that makes it. An orbit takes a `length`, and a projectile's or a phase portrait's path is whole.
- The points are the object's world position at every frame time, so a trail follows a `follow`, a carry, a `move` or a data point alike. A trail of a data point is a phase portrait: put a `dot` on `axes` and follow its position with the state's two coordinates.
- A trail is not created. It grows as its object moves, so it shows nothing before the object moves. Fade it in and out, and recolor its `stroke`.
- A trail has no `position`, `rotation` or `scale` (`placed_by_trail`). Its object is not a plot or another trail (`invalid`).
- Its points stay where the object was in the world: a trail of a data point does not move with its axes.
- `inspect` reports `trail.point_count`, `trail.newest`, the point the object stands at, and `trail.newest_pixels`, with the trail's bounding box.

## A number that counts: readouts

A `readout` shows a number typeset as math, with a `unit` after it:

```json
"speed": { "type": "readout", "decimals": 1, "unit": "\\text{ m/s}", "attach": { "object": "ball", "offset": [1.1, 0.4] } }
```

- `value` is the number shown until an animation changes it, 0 by default, `decimals` how many decimals it shows, from 0 to 6, and `size` and `color` as for math. Write the space before the unit inside `\\text{...}`.
- A `count` eases the value to a `to` over its span: `{ "type": "count", "object": "distance", "to": 9, "easing_curve": "slow-start" }`. Give it the same span and easing curve as the `move` it measures, and the number stays with the motion.
- A follow with `"sets": "value"` sets it from a law with one expression: `"law": { "value": "9.8*t" }`, and no `x`, `y` or `z`. The law starts where the value is, or `validate` reports `law_jump`, as for any law.
- The number is lined up on its decimal point at the object's position, or on the end of its last digit with no decimals. It grows to the left, so leave room left of the position, and put its symbol, such as `v =`, in a `math` object with `"align": "right"` left of it.
- A readout is written, faded, recolored and attached as math is: attach a speed to its ball with an `offset` that clears the ball.
- `inspect` reports `readout.value` and `readout.text`, the number as it is shown, under the readout. Only a readout has a value: a count or a follow of `value` of another object is `not_readout`.
- A readout typesets its digits once, and once more for each count of digits it shows, so a number that counts through thousands of values costs `validate` no more than a few typesettings.

## Motion known only by its rates: simulations

When a lesson knows how fast things change but no formula for where they are, write a `simulate` animation. It integrates its `variables` from their starting values by their `rates` over its span, and `sets` positions, rotations and readouts' values from expressions in the variables:

```json
{ "type": "simulate",
  "variables": { "angle": 1.2, "spin": 0 },
  "rates": { "angle": "spin", "spin": "-9.8/2.5*sin(angle) - 0.3*spin" },
  "sets": [
    { "object": "bob", "position": ["-3 + 2.5*sin(angle)", "2.5 - 2.5*cos(angle)", "0"] },
    { "object": "speed", "value": "abs(2.5*spin)" }
  ] }
```

- A variable's name is letters, digits and `_`, starting with a letter, and not `t`, `pi`, `e`, `tau` or a function's name. Write one rate for every variable, by its name. A rate reads every variable and `t`, law time, which counts from the span or from the chapter `from` names.
- A second-order law, such as a pendulum's or a spring's, is two variables: the position and its rate of change, as `angle` and `spin` above.
- Each set gives one value: `position` or `rotation` as three expressions, or a readout's `value` as one. A simulation sets each value once, so a second set of the same object's `position` is `invalid`. Several objects can share the variables: two masses on springs, three bodies in orbit, or a state point on axes and the readouts of its coordinates.
- A set gives its value outright, so it must start where the value is. Declare the object where the starting values put it, or `validate` reports `law_jump` at the set with both values.
- A variable that runs away to no finite value is `out_of_range` at its rate, with the scene time, and so is a set with no value at a frame time, at its expression.
- The simulation owns each value it sets for its span, as a follow does: a move of the same object then is `conflicting_animations`. Each value holds from the span's end on. A simulation takes no `easing_curve`.
- An end on a simulated object, a trail of it and a readout of its speed all go with it, so a rod from the pivot to the bob, the arc the bob leaves and its speed are one simulation and three objects.
- `check-layout` checks every frame time of a simulation's span, as it does a follow's.
- A simulation spans at most 216000 frames, an hour at 60 frames a second, or it is `out_of_range`.

| Lesson | Variables | Rates |
|---|---|---|
| Pendulum with drag | `angle`, `spin` | `spin`, `-9.8/L*sin(angle) - c*spin` |
| Projectile with air resistance | `x`, `y`, `vx`, `vy` | `vx`, `vy`, `-k*vx`, `-9.8 - k*vy` |
| Charging capacitor | `q` | `(V - q/C)/R` |
| Predator and prey | `rabbits`, `foxes` | `a*rabbits - b*rabbits*foxes`, `d*rabbits*foxes - g*foxes` |
| Epidemic | `s`, `i`, `r` | `-b*s*i`, `b*s*i - g*i`, `g*i` |
