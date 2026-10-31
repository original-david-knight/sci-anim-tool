# Motion: steps, spans, follows and the camera

Read `instructional-videos.md` first. It says how long each change should take. This file says how to write it.

## Steps and spans

Every animation in a `play` step runs over the whole step unless it sets `start` and `end`, in seconds from the start of the step. Use consecutive steps for things that happen one after another. Use `start` and `end` inside one step when changes overlap or are timed to a word: `{ "type": "move", "object": "box4", "to": [2, 1, 0], "start": 0.5, "end": 1.2 }`. One object can move twice in a step if the two spans do not overlap, such as a lift from 0 to 0.4 s and then a slide from 0.4 to 1.2 s.

Overlap the end of one idea with the start of the next in one step: fade the old objects out from 0 to 0.6 s while the new one is created from 0.3 s. The frame then never stands empty between ideas.

An `easing_curve` shapes each change. `slow-both`, the default, suits most changes. Use `linear` for a steady rotation, a point that runs along a curve, or a count that keeps pace with a linear move. `slow-end` suits an object arriving and `slow-start` one leaving. Use `overshoot` rarely, for a playful arrival.

A value has one track: two animations whose spans overlap cannot change the same value of one object (`conflicting_animations`).

## Keep a motion going under several steps

A `wait` step holds the whole stage still, so a body that should keep moving while sentences come and go needs a `play` step with its follow in every step. Put the steps in one `chapter` and give each follow `"from": "<chapter id>"`: law time then counts from the chapter's start, and the motion runs on unbroken from step to step. A step with nothing else to do is a `play` with only the follow. `../examples/spring_wave.go` does this with a helper that adds the mass's follow to each step of its `bounce` chapter.

## Motion by a law or along a curve

A swing, an orbit, a pulse, a camera circling a solid or a point running along a plot is one `follow`, not a chain of moves. A follow sets a value at every scene time of its span from a law: `law` gives `x`, `y` and `z` as expressions in `t`, the law time in seconds, and `sets` names the value, `position` (the default), `rotation` in degrees, `scale` as a factor along each of the object's own axes, `camera-position` or `camera-target`. Leave out `object` for the camera. A rotation writes its unused angles as `"0"`.

```json
{ "type": "follow", "object": "rod", "sets": "rotation", "law": { "x": "0", "y": "0", "z": "30*cos(2.21*t)" } }
```

- The law gives the value outright, so it must start where the value is. At the span's start the law above gives 30, so declare the rod with `"rotation": [0, 0, 30]`. `validate` reports `law_jump` at `law`, with the law's value and the value the object has, when they differ by more than 0.001: declare the object where the law starts, or change the law.
- Law time counts from the span's start, or from the start of the chapter `from` names, as above.
- A law takes no `easing_curve`. The value stays at what the law gave at the span's end, and a later `move`, `rotate`, `scale` or `fade` eases from there.
- `validate` evaluates every law at every frame time of its span. An expression with no finite value there, such as `1/(t - 2)` at law time 2, or a position past 10000, is `out_of_range` at that expression, with the scene time.
- `"sets": "opacity"` and `"sets": "scale"` need sci_anim 0.7.0 or later. `"sets": "opacity"` sets the object opacity from one expression, `value`, with no `x`, `y` or `z`, as a readout's value is set: `"law": { "value": "1 - t/2" }` fades an object out over 2 s. The law must stay within 0 to 1 at every frame time of its span, or `validate` reports `out_of_range` at `value` with the scene time it leaves. At 0 the object is hidden, as after a fade to 0, and `check-layout` judges it as it judges an object a fade has faded.
- Pieces that pulse and fade as a material breaks down are two follows of one `pattern` in one step: `"sets": "scale"` with `{ "x": "1 + 0.3*sin(2*pi*t)", "y": "1 + 0.3*sin(2*pi*t)", "z": "1" }`, and `"sets": "opacity"` with `{ "value": "1 - t/4" }` over a 4 s span. A follow of a pattern's `scale` scales the whole pattern about its `position`, so a ring's radius grows and shrinks with its copies. To pulse each copy where it stands, write its `scale` in `copy` in `t` and run it with `"sets": "shape"`, as `physics.md` shows.
- A follow owns its value for its span as a `move` does, and a follow of `opacity` as a `fade` does. A plot (`placed_by_axes`), an attached object (`attached`) and an object a linear map names (`mapped_object`) cannot be followed, except in `opacity`, as they can be faded; follow what places them instead. An object attached to a followed object goes with it.
- A function plot, a tube, a surface or a pattern whose expressions read `t` changes shape over time: a follow with `"sets": "shape"` and no `law` runs it, as a travelling wave. `physics.md` says how. So does a readout's value, with `"sets": "value"`.
- Motion known only by its rates, such as a pendulum with drag, is a `simulate` animation, which `physics.md` teaches.

A follow with `along` instead of `law` carries `object` along another object's curve: the path of a flat shape or a plot, or a tube's space curve, from its start to its end by length over the span, shaped by its `easing_curve`. Use `linear` for a steady pace. `"faces": "tangent"` also turns the object so its own x axis points along the curve, so draw a triangle pointing right, toward +x:

```json
{ "type": "follow", "object": "triangle", "along": "parabola", "faces": "tangent", "easing_curve": "linear" }
```

- The carry goes wherever the `along` object is at each scene time: moved, scaled, under a linear map, or a plot whose axes move.
- A carry and a `create` of the same plot, with the same span and easing curve, keep the object on the pen's end at every frame.
- The object must stand at the curve's start, and with `faces` face along it there. `validate` reports `carry_jump` at `along` with the `position` to declare, and at `faces` with the `rotation` to declare. Copy each into the object. A tube through `points` starts along its first chord, so a first chord along +x starts the object at `rotation` `[0, 0, 0]` (`solids.md`).
- An `along` object cannot be carried itself, attached, or morphed, and a follow with `along` takes no `law`, `from` or `sets`. Text, math, axes, spheres, prisms and revolutions have no one curve (`no_curve`). The camera is not carried: circle it with a law.
- Carried along a tube with `faces`, an object's `rotation` in `inspect` can change fast where its own z axis points along the world's x axis, where Euler angles lose one of their three. The frame draws it turning smoothly.

To read a follow frame by frame, run `sci_anim stages s.json --from 2 --to 3`: it prints what `inspect` prints at every frame time from 2 to 3 s.

## The camera

At the default camera, `[0, 0, 10]` looking at the origin with a vertical field of view of 45 degrees, the frame shows x from -7.36 to 7.36 and y from -4.14 to 4.14 world units in the plane z = 0.

Three animations move the camera, each with `to` and the usual `start`, `end` and `easing_curve`. `camera-move` carries its position to `[x, y, z]` and keeps it looking at its target. `camera-look` turns it to look at `[x, y, z]`. `camera-zoom` sets its vertical field of view to `to` degrees, and a smaller value zooms in: `{ "type": "camera-zoom", "to": 30 }`. Set where the camera starts in the scene file's `camera`, with `position`, `target` and `fov`. A follow of `camera-position` circles it (`solids.md`).

The camera draws in perspective, so a sphere or an atom near the frame's edge looks stretched. When round things sit near the edges, move the camera back and narrow its view: `"position": [0, 0, 20]` with `"fov": 23.4` frames the same plane z = 0 as the default.

Move the camera only for a reason the viewer can name, such as showing a solid's depth or following the object to watch, and run `check-layout` after it: a camera move changes what the frame shows.
