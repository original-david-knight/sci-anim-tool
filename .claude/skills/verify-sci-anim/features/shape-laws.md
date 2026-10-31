# Run a shape law

A function plot's `expression`, a tube's `curve`, a surface's `z` and a pattern's `copy` expressions and `along` curve may read `t`, law time. A `follow` with `"sets": "shape"` then redraws the shape at every scene time of its span as its expressions give it at that law time, and holds it from the span's end on. Before its first follow the shape is drawn at `t` of 0. `inspect` reports the shape as it stands at the scene time asked, and `validate` checks the shape at every frame time of each such follow.

## Sub-features

- `shape-plot` runs a travelling wave on axes: its curve moves between frames, and `inspect`'s `data_bounding_box` follows `t`.
- `shape-from` counts law time from a chapter's start with `from`, so a second follow goes on from where the first ended.
- `shape-solids` runs a tube's curve, a surface's height and a pattern's copies, each as `inspect` reports them.
- `shape-surface-sampling` samples a moving surface at the same points at every frame, so the still part of the sheet does not change between frames, and checks it on all of those points at every frame time.
- `shape-errors` refuses a follow of a shape that reads no `t` (`invalid` at `object`), a `law` beside `sets: shape` (`invalid` at `law`), a space curve with no value at a frame time (`out_of_range` at the expression, with the scene time), a morph of a shape law and a carry along one (`invalid`), two follows of one shape at once (`conflicting_animations`), and a follow whose law time does not start at the one the shape is drawn at (`law_jump` at the follow).
- `shape-layout` runs `check-layout`, which checks every frame time of each follow of a shape.

## How to get to it (user POV)

- Write `t` into a shape's expression and add `{ "type": "follow", "object": "<id>", "sets": "shape" }` to a play step, then run `validate`, `inspect`, `stages`, `frame`, `render`, `frames` or `check-layout`.
- `skills/sci-anim/examples/waves.json` runs a travelling wave, a standing wave as the sum of two, and a Fourier series that gains a term each second.
- `sci_anim schema | jq '.["$defs"].FollowedFile'` lists `shape` among the values of `sets`.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok` for the frames.

- **Plot.** `validate skills/sci-anim/examples/waves.json`: exit 0, duration 31. Draw frames at 4 and 4.5 into `$RUN` and view them: the blue wave's crests have moved right by about 0.75 between the two, an eighth of the axis. Frames 2 s apart look alike, since the wave moves one wavelength in about 2.1 s. `inspect -t 4` gives `.objects.travel.plot.data_bounding_box` from about -1 to 1 in y.
- **From.** Write `$RUN/plot.json`: axes with x range `[0, 1]` and y range `[-5, 5]`, a `function-plot` `line` of `"x + t"`, a wait of 1 s, then a chapter `wave` holding two follows of `line`'s shape with `"from": "wave"`, each over 1 s, and a wait of 1 s. `inspect -t 0.5` gives `.objects.line.plot.data_bounding_box.min[1]` 0, before the follows; `-t 2.5` gives 1.5, law time counted from the chapter in the second follow; `-t 3.5` gives 2, held.
- **Solids.** Write `$RUN/solids.json` with a tube whose `curve` has `"y": "t"`, a surface with `"z": "t"`, and a pattern of two dots with `"copy": { "position": { "x": "x + t" } }`, each followed over 2 s. `inspect -t 1.5` gives `.objects.<tube>.tube.start` `[0, 1.5, 0]`, the surface's `bounding_box.world.max[1]` 1.5, and the first copy's `position` `[1, 0, 0]`.
- **Surface sampling.** Write a surface `"z": "0.6*sin(3*y - 4*t)*exp(-2*x^2)"` over x `[-5, 5]` and y `[-3, 3]`, followed over 4 s at 30 fps, seen from `[0, 6, 9]`. Draw frames at 1, 1.0333 and 1.0667: in the right quarter of each, where the sheet is flat, no pixel changes between neighboring frames, and the middle shows the wave moving. Then write a surface over x and y `[0, 1]`, followed over 1 s, of `"(1 - step(t - 0.5))*exp(-400*(x - 0.3)^2) + (x - 0.298828125)/(x - 0.298828125 + 1 - step(t - 0.5))"`: a bump before 0.5 s, whose points reach x = 0.298828125, and a flat sheet after with no value there alone. `validate` gives `out_of_range` at `/objects/<id>/z` whose message starts `at scene time 0.5 (law time 0.5)` and names x = 0.298828125.
- **Errors.** In `$RUN`: a follow of the shape of a plot of `"x"` gives `invalid` at `.../object`; the same follow with a `law` adds `invalid` at `.../law`; a tube with `"y": "1/(t - 1)"` followed over 2 s gives `out_of_range` at `/objects/<id>/curve/y` whose message starts `at scene time 1 (law time 1)`; a morph `into` a shape law gives `invalid` at `.../into`, and a carry `along` one `invalid` at `.../along`; two follows of one shape in one span give `conflicting_animations`; in `plot.json`, leaving `from` off the second follow gives `law_jump` at that follow, whose message names law time 0 and law time 1. Each exits 1.
- **Layout.** `check-layout skills/sci-anim/examples/waves.json --out "$RUN/layout"` exits 0 and prints `No problem at any of 32 checked scene times and 1383 frame times of follows.`
- **Proof.** The reports, the frames and a sentence per frame in `$RUN`.

## Gotchas

- With no `from`, a follow counts law time from its own span, so it starts at `t` of 0. Only the first follow of a shape can leave `from` out; a later one names the chapter the first began in, or `validate` reports `law_jump`.
- A wave crosses its x axis, so `check-layout` reports it running through x tick labels. Give that axis a `spacing` wider than its range.
- A shape law follow costs `validate` one reading of the shape per frame time of its span.
