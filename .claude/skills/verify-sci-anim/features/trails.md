# Draw trails

A `trail` object draws the world points its `object` passed through over the last `length` seconds of scene time, or since that object began to show, as a stroke that faces the camera and ends where the object stands. The points are the object's world position at every frame time, so a trail follows a law, a carry, a move or a data point. `inspect` reports the trail's `point_count`, its `newest` point with `newest_pixels`, and its bounding box.

## Sub-features

- `trail-length` keeps the last `length` seconds of a moving object's path, its newest point where the object stands.
- `trail-whole` with no `length` draws the whole path since the object began to show: a projectile's parabola and a phase portrait's spiral.
- `trail-data-point` follows a dot on axes in the world.
- `trail-recolor` recolors a trail's stroke.
- `trail-attached` follows an object attached to a shape whose shape law moves it.
- `trail-scene-end` ends at its object, not past it, when the scene ends between two frame times.
- `trail-errors` refuses a trail of no object (`unknown_object`), of a plot or of itself (`invalid`), with a `position` (`placed_by_trail`), and a move (`placed_by_trail`), create (`invalid`) or fill recolor (`not_recolorable`) of a trail. An end of a line, arrow or spring on a trail is refused in `ends.md`.
- `trail-layout` runs `check-layout` on the trails lesson.

## How to get to it (user POV)

- Write `{ "type": "trail", "object": "<id>", "length": 4 }` among the objects, move the object, then run `validate`, `inspect`, `stages`, `frame`, `render`, `frames` or `check-layout`.
- `skills/sci-anim/examples/paths.json` leaves the four-second trail of an orbit, the whole path of a thrown ball, and the spiral a damped swing traces on its phase portrait.
- `sci_anim schema | jq '.["$defs"].ObjectFile.allOf[].then | select(.properties.type.const == "trail")'` prints the fields.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok` for the frames.

- **Length.** `validate skills/sci-anim/examples/paths.json`: exit 0, duration about 31.4. `inspect -t 6.5` and `-t 9.5`: `.objects.orbit.trail.newest` equals `.objects.planet.position` both times, and `.trail.point_count` is 241, four seconds of frame times and the newest point. Draw both frames and view them: an arc of the ellipse about the sun ends at the planet, and has moved on around it.
- **Whole.** `inspect -t 14`: `.objects.flight.trail.newest` is the ball's position at the end of its flight, about `[5.43, -2.5, 0]`. Draw the frame and view it: the whole parabola from the left of the ground to the ball. `inspect -t 31` gives a spiral `phase` trail; draw the frame and view it: a spiral into the axes' origin.
- **Data point.** Axes of x and y range `[0, 4]`, 4 by 4, a dot on them moved from data `[1, 1]` to `[3, 3]`, and a trail of the dot: `inspect -t 1` gives `.trail.newest` `[1, 1, 0]`, the world point of data (3, 3).
- **Attached.** A surface of `"z": "t"` over x and y `[-1, 1]` followed with `"sets": "shape"` over 2 s, a text attached to it with offset `[1, 0]`, and a trail of the text with `length` 0.5: at `-t 2` the trail's world `bounding_box.min[2]` is the text's `position[2]` at `-t 1.5`, so the trail follows the caption out as the sheet rises.
- **Scene end.** A dot followed by `"x": "t"` over 1.01 s at `"fps": 30`, so the scene ends between frame times, and a trail of it: `inspect -t 1.005` gives `.objects.<trail>.bounding_box.world.max[0]` 1.005, where the dot stands, and `.trail.newest` `[1.005, 0, 0]`.
- **Recolor.** A trail with a white stroke recolored to `#fc6255`: `inspect` at the recolor's end gives `.colors.stroke` `#fc6255`.
- **Errors.** In `$RUN`: a trail of `nowhere` gives `unknown_object` at `/objects/<id>/object`; of a `function-plot` and of itself, `invalid` there; one with a `position`, `placed_by_trail` at `.../position`; a move of a trail, `placed_by_trail` at `/timeline/<n>/animations/<m>/object`; a create, `invalid` there; a recolor of its `fill`, `not_recolorable` at `.../fill`. Each exits 1.
- **Layout.** `check-layout skills/sci-anim/examples/paths.json --out "$RUN/layout"` exits 0 and prints `No problem at any of 29 checked scene times and 1290 frame times of follows.`
- **Proof.** The reports, frames and a sentence per frame in `$RUN`.

## Gotchas

- A trail shows nothing until its object moves. Fade it in with its object, or declare it shown.
- A trail's points stay in the world: moving the axes of a data point leaves the trail where the point was.
- The ends of a trail are cut square, as a stroke's are, so its box starts at its oldest point.
