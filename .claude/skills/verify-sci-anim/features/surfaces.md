# Draw surfaces

A scene file's `surface` is a lit sheet whose height `z` is an expression in `x` and `y`, over the rectangle `x_range` by `y_range` of the object's floor, with y running away from the default camera and the height rising straight up. It is sampled finer where it bends, lit by the key light on whichever side the camera sees, and drawn with edges along its rim and where it folds away from the camera. A create rises it from the flat rectangle to its heights. `inspect` reports its bounding box, and every mistake in `z` or its ranges is a scene error at its field, and a surface written with the old `y` over `x` and `z` is told to write `z`.

## Sub-features

- `surfaces-draw` draws a potential well and a standing wave, lit, with edges along their rims and folds.
- `surfaces-both-sides` lights the side the camera sees, from above and from below, and under a mirroring scale.
- `surfaces-create` rises a surface from flat to its heights over a create's span.
- `surfaces-inspect` reports a surface's bounding box, the whole sheet's even mid-create.
- `surfaces-errors` refuses an unknown name in `z` at its column, a range high first, a height with no value or past 10000 at some x and y, the old fields `y` and `z_range` with a hint naming `z` and `y_range`, and a type such as `heightmap` with a hint naming the surface.
- `surfaces-layout` runs `check-layout` on a scene with surfaces, and `stages.txt` names a surface by its `z`.

## How to get to it (user POV)

- Write an object with `"type": "surface"` in a scene file, then run `validate`, `frame`, `inspect`, `render`, `frames` or `check-layout` on it.
- `examples/surfaces.json` rises a potential well and a standing wave, then circles the camera around them.
- `sci_anim schema | jq '.["$defs"].ObjectFile.allOf[].then | select(.properties.type.const == "surface")'` prints the fields.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.

- **Draw.** Run `$SKILL/scripts/run "$RUN" surfaces-draw-5 frame examples/surfaces.json --scene-time 5 -o "$RUN/surfaces-5.png"`. Exit 0. View the PNG: a blue sheet with a round well sunk in it on the left and a red sheet of ridges on the right, each lit from the upper left, with thin dark edges along their rims and along the lip of the well, the math of each under it, written `z = ...` in `x` and `y`, and the title above.
- **Both sides.** Write `$RUN/work/below.json`: a 400 by 400 scene on `#000000` with the camera at `[0, -10, 0.5]` looking at the origin and one surface with `z` `0`, ranges `[-1, 1]`, color `#ff0000`. Draw it, then again with the camera at `[0, 10, 0.5]`, then again from below with the surface's `scale` `[1, -1, 1]`. The middle pixel's red is about 213 in all three, not the 155 or so of a side lit only by the ambient.
- **Create.** Draw `--scene-time 0`, `1` and `2` into `$RUN`. At 0 no blue shows. At 1 the well is half as deep and the sheet still flat at its rim. At 2 it is whole.
- **Inspect.** `inspect examples/surfaces.json --scene-time 1`: `.objects.well.created` is 0.5 and `.bounding_box.world` runs from `[-5.7, -2, -2.5]` to about `[-0.7, -0.004, 2.5]`, the whole sheet's.
- **Away from the camera.** Write a surface with `z` `0`, `x_range` `[-1, 1]` and `y_range` `[0, 2]`. `inspect` gives `.bounding_box.world` from `[-1, 0, -2]` to `[1, 0, 0]`: data y runs along -z.
- **Errors.** Write a scene file into `$RUN` with a surface whose `z` is `a*x + sin(u, y)`, `x_range` `[1, -1]` and `y_range` `[2, 2]`. `validate` exits 1 with three `bad_format` errors at `/objects/<id>/z`, for `a`, `u` and `sin` given two arguments, each message naming its column, and `invalid` at `/x_range` and `/y_range`. A surface with `z` of `sqrt(x)` over `x_range` `[-1, 1]` gives `out_of_range` at `/z` naming `x = -1`, and one with `exp(y)` over `y_range` `[0, 20]` gives `out_of_range` at `/z` with the value and its x and y. A surface written the old way, `y` of `x*z` over `x_range` and `z_range`, gives `unknown_field` at `/y` whose message says the height is `z`, `unknown_field` at `/z_range` naming `y_range`, and `missing_field` at `/z`. In a separate file, an object of `"type": "heightmap"` gives `unknown_value` at `/objects/<id>/type`, whose message names the `surface` to write.
- **Video and layout.** `render examples/surfaces.json -o "$RUN/surfaces.mp4"` writes 990 frames per `ffprobe`. `$SKILL/scripts/run "$RUN" surfaces-layout check-layout examples/surfaces.json --out "$RUN/layout"` reports no problem, and `$RUN/layout/stages.txt` names `well (surface)` with its `z =` expression and ranges.
- **Proof.** The PNGs, reports, and a sentence per PNG in `$RUN`.

## Gotchas

- A surface seen straight from the front, from the default camera, is a thin line. Put the camera above and to the side.
- `x_range` and `y_range` are numbers, not expressions, low first.
- A surface that is not symmetric in y looks mirrored front to back against a scene written for the old `y` in `x` and `z`: data y is the old -z.
- The key light turns with the camera, so a sheet seen from below at a slant shows a darker side than from above. Compare square-on views to judge which side is lit.
- `check-layout` compares text with a surface's whole box, which spans its lowest and highest heights over the whole rectangle.
