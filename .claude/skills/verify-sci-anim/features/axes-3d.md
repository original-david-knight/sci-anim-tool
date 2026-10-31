# Draw 3D axes

A scene file's `3d-axes` draws three axes with ticks, tick labels and axis labels, z up as a textbook draws them: data x to the right, z up and y away from the default camera, each axis's `range` filling its `length` in a box centered on the object's position. Each axis crosses the others at 0, or the nearest end of its range, and the tick there has no label. Tick labels sit below the x and y axes and left of the z axis, every label faces the camera at every frame, and an object in front of a label hides it. A create draws the lines and ticks, then fades the labels in. `inspect` reports `axes_3d`: where data lands in the world and each tick with its label's text, side and box. A `surface` or `tube` that names the axes in its `"axes"` field is written in their data coordinates, goes with them, and is cut at their box: a surface where it leaves the z range, a tube into pieces each closed by a disc. A `sphere` that names them is a data point, whose position, moves, follows and carries are in their data coordinates, with its radius in world units.

## Sub-features

- `axes-3d-draw` draws three labeled axes with ticks from a camera above and to the side.
- `axes-3d-facing` turns every label to face the camera as the camera moves.
- `axes-3d-depth` hides a label behind a solid that stands in front of it.
- `axes-3d-create` draws the lines and ticks over the first half of a create, then fades the labels in.
- `axes-3d-inspect` reports `zero`, `x_unit`, `y_unit`, `z_unit`, each axis's range and label box, and each tick with its label, none at the crossing.
- `axes-3d-errors` refuses a range high first, an unreadable label, a missing `length`, a morph, linear map, recolor or carry along the axes, and a type such as `three-d-axes` with a hint naming `3d-axes`.
- `axes-3d-layout` runs `check-layout`, which checks text against the lines, ticks and labels, and `stages.txt` lists each axis's tick labels.
- `axes-3d-surface` draws a surface on the axes in their data coordinates, with the axes' ranges, cut where it leaves the z range, a pole stopping at the box, and a create rising it from data z of 0.
- `axes-3d-tube` draws a tube on the axes in their data coordinates, cut into pieces at their box.
- `axes-3d-data-point` places a sphere on the axes by its data position, walks it by a law in data coordinates, keeps its radius in world units as the axes scale, carries it along a tube on the axes, and sits a caption on it where the frame shows it. A dot on 2D axes is the 2D data point, in `data-points.md`.
- `axes-3d-placed` moves a surface with its axes, and refuses a position or a rotate of it with `placed_by_axes`, a plot naming 3D axes and a surface naming 2D axes with `invalid` at `/objects/<id>/axes`.

## How to get to it (user POV)

- Write an object with `"type": "3d-axes"` in a scene file, then run `validate`, `frame`, `inspect`, `stages`, `render`, `frames` or `check-layout` on it.
- Give a `surface` or a `tube` an `"axes"` field that names the 3D axes.
- `examples/axes_3d.json` creates 3D axes, rises the paraboloid z = x² + y² on them, and walks a data point down it by a law while the camera circles.
- `sci_anim schema | jq '.["$defs"].ObjectFile.allOf[].then | select(.properties.type.const == "3d-axes")'` prints the fields, and `jq '.["$defs"].Axis3dFile'` an axis's.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.

- **Draw.** Write `$RUN/axes.json`: the camera at `[5, 6, 12]` looking at `[0, 0, 0]`, and `"a": { "type": "3d-axes", "x": { "range": [-3, 3], "length": 6, "label": "x" }, "y": { "range": [-3, 3], "length": 6, "label": "y" }, "z": { "range": [0, 4], "length": 4, "label": "z" } }`, with a 2 s play that creates `a` and a 6 s play whose `follow` of `camera-position` circles the camera, `"x": "13*sin(0.394791 + tau*t/6)", "y": "6", "z": "13*cos(0.394791 + tau*t/6)"`. Draw `--scene-time 2`. View the PNG: x runs to the lower right, y to the upper right away from the camera and z straight up, each with ticks and number labels, `x`, `y` and `z` past their ends, and no label where the axes cross.
- **Facing.** Draw `--scene-time 3.5` and `5`. View both: the camera has moved around the box, every label still reads upright and square to the frame, not slanted or edge-on, and the z tick labels stay left of the z axis on the frame, clear of it, with each z tick across the axis.
- **Depth.** Add a red sphere of radius 0.3 halfway between the camera and the label of the x tick at 2, using that label's world box from `inspect`. Draw it. The 2 is hidden behind the sphere, and the other labels show.
- **Create.** Draw `--scene-time 0.5` and `1`, and `inspect` at 1. At 0.5 part of the lines shows and no label. At 1 `.objects.a.created` is 0.5, every line and tick shows, and no label yet.
- **Inspect.** `inspect $RUN/axes.json --scene-time 2`: `.objects.a.axes_3d.zero.world` is `[0, -2, 0]`, `x_unit` `[1, 0, 0]`, `y_unit` `[0, 0, -1]` and `z_unit` `[0, 1, 0]`. The ticks of all three axes are in one list, `.axes_3d.ticks[]`, each with `axis` and `value`. The x tick at 2 has `label_text` `"2"` and `label_side` `"below"`, the z tick at 2 `"left"`, and the ticks at the crossing, x 0, y 0 and z 0, have no `label_text`. `.axes_3d.z.label_bounding_box` lies above the z axis's top at world y 2.
- **Errors.** Write scene files into `$RUN`: an x `range` of `[1, -1]` gives `invalid` at `/objects/a/x/range`; a z `label` of `\\boxed{z}` gives `invalid` at `/objects/a/z/label`; an x with no `length` gives `missing_field` at `/objects/a/x/length`. With a circle `c` and a dot `d` beside the axes, a morph of `a` into `c`, a linear map of `a`, a recolor of `a` and a follow of `d` along `a` give `not_morphable`, `not_mappable`, `not_recolorable` and `no_curve`. An object of `"type": "three-d-axes"` gives `unknown_value` at `/objects/<id>/type`, whose message names `3d-axes`.
- **Video and layout.** `render $RUN/axes.json -o "$RUN/axes.mp4"` writes 480 frames per `ffprobe`. `$SKILL/scripts/run "$RUN" axes-3d-layout check-layout $RUN/axes.json --out "$RUN/layout"` reports no problem, and `$RUN/layout/stages.txt` lists `a (3d-axes)` with its x, y and z tick labels. A text set on a tick label is reported as covering `the tick label 2 of "a"`.
- **Surface.** Run `frame examples/axes_3d.json --scene-time 5` into `$RUN`. View the PNG: a blue bowl standing on the axes' floor, its rim cut round and flat at the top of the box, z = 8, with a dark edge along the cut; the z axis shows above the bowl's inside, and the axes and labels behind the bowl are hidden by it. `inspect` at 5 gives `.objects.bowl.bounding_box.world` from about `[-2.36, -1.25, -2.36]` to `[2.36, 1.25, 2.36]`: x² + y² = 8 at 5/6 of a unit per data unit, and the z range of 0 to 8 standing 2.5 tall. Draw `--scene-time 3`: the bowl half risen from the floor, its corners already cut.
- **Pole.** Write a surface on the axes with `z` `1/(x^2 + y^2)`. `validate` exits 0, and `inspect` gives its box's top at the box's top.
- **Tube.** Write a red tube on 3D axes with z from 0 to 8, whose curve is `x` `u`, `y` `0`, `z` `4 + 6*sin(u)` over `[-3, 3]`, and draw it from `[0, 0, 12]`. View the PNG: the wave shows in three pieces, each ending in a flat disc at the top or floor of the box. A rod `x` `u` over `[-5, 5]` on x from -3 to 3 inspects with its box from x -3 to 3, and `tube.end` still at x 5 of data, the curve's own end.
- **Data point.** Draw `examples/axes_3d.json` at `--scene-time 5`, `7`, `9` and `13`. View them: a red ball at the bowl's rim at 5, lower and nearer the middle at 7 and 9, and at the bottom by 13, round in every frame while the camera circles. `inspect` at 9 gives `.objects.point.data_point.position` about `[-0.343, -0.397, 0.576]`, the law at law time 4, and `.position` the world point the axes carry it to. Write a sphere `p` of radius 0.2 on axes with x from -3 to 3 over 6, y from 0 to 4 over 2 and z from 0 to 4 over 4, at data `[1, 2, 4]`: `inspect` gives `.position` `[1, 2, 0]` and a box 0.4 across; scale the axes to 2 and the box is still 0.4 across. A text `attach`ed to `p` with offset `[0, 0]` has its pixel box centered on `p`'s. A follow of `p` along a tube on the axes from data `[-2, 2, -1]` validates, and a `p` declared elsewhere gets `carry_jump` naming the data position `[-2, 2, -1]` to declare.
- **Placed.** Move the axes and `inspect` the surface: its box moves with them. A `position` on the surface gives `placed_by_axes` at `/objects/<id>/position`, and a rotate of it `placed_by_axes` at its animation's `object`. A function plot whose `"axes"` names 3D axes, and a surface whose `"axes"` names 2D axes, give `invalid` at `/objects/<id>/axes`.
- **Proof.** The PNGs, reports, and a sentence per PNG in `$RUN`.

## Gotchas

- From the default camera the y axis points at the camera and shows as a dot with its labels stacked. Put the camera above and to the side.
- Caption a data point with a capital letter, as `P`, to check that its attachment centers on it. A letter with a descender, as `p`, sets its box about 18 pixels lower at 1080p.
- A label's box changes with the camera, because the label faces it. Read boxes from `inspect` at the scene time you check.
- The box is centered on the position, so data 0 lies at the middle of a range only when the range is symmetric: z from 0 to 4 puts data z 0 at the box's floor.
- A data point inside a steep bowl is hidden by the bowl's near wall from a low camera. Raise the camera above the wall's slope.
- A surface on a square of x and y rises from flat mid-create, so its corners reach the top of the box first and the half-risen sheet shows the square's corners cut.
