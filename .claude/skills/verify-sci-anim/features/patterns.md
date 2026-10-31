# Draw patterns

A scene file's `pattern` draws one `shape`, a flat shape or a solid, at every copy of a `layout`: a `grid` of `counts` at a `spacing`, centered on its position, a `ring` of `count` at a `radius` from a `start_angle`, or `along` a space curve by length, where `"faces": "tangent"` turns each copy's own x axis along the curve. `copy` gives each copy its own position, rotation, scale and color share from expressions in its index (`i`, and `j` and `k` in a grid) and its place (`x`, `y`, `z`), and `to_color` is what a color share mixes toward, or, from sci_anim 0.15.0, each copy picks a color of a `palette` by its `palette_index`, an expression or a list of one index a copy. A pattern is one object: one rotate turns every copy, a create grows the copies in index order, and `inspect` reports `pattern.copy_count` and each copy's `index`, world `position`, `position_pixels`, `rotation`, `created`, `colors` and `bounding_box`. `check-layout` checks text against each shown copy and reads a text's contrast against a copy, a solid or a sphere behind it.

## Sub-features

- `patterns-draw` draws a lattice of spheres, a ring of ticks, beads along a helix and an arrow field, each in its mixed colors.
- `patterns-face` turns copies along a helix and along a flat curve to face the tangent, with each copy's own `rotation` on top, and along a curve that reads `t` as a follow of its shape runs it.
- `patterns-palette` colors each half rung of a ladder by its nucleobase in the sequence ATGCGTA and its partner strand, from a palette, and refuses an index that is no whole number within the palette, a list of the wrong length and a bad color, each at its path.
- `patterns-create` grows a pattern's copies in index order over a create's span.
- `patterns-inspect` reports the copy count and each copy's index, position, created share, colors and box, and `stages` reports them at every frame time.
- `patterns-errors` refuses a shape a pattern does not draw, a bad field inside the shape, more than 10,000 copies, an unknown variable, an expression with no value at a copy, `color_share` without `to_color`, and a linear map, morph or carry of a pattern, each at its path.
- `patterns-layout` runs `check-layout` on a scene with patterns, and its tests cover text between copies, text on a copy, a reserved region and text over a solid.
- `patterns-video` renders the example to MP4 and to a frame sequence.

## How to get to it (user POV)

- Write an object with `"type": "pattern"` in a scene file, then run `validate`, `frame`, `inspect`, `stages`, `render`, `frames` or `check-layout` on it.
- `examples/facing_copies.json` sets cones and prisms tilted 20 degrees along two helices and arrows along a flat wave, each `along` layout with `"faces": "tangent"`, then turns the helices.
- `examples/patterns.json` grows a 4×4×4 lattice and turns it, sets 12 ticks around a dial, threads 24 beads along a helix, and draws the field (-y, x) as 52 arrows, then recolors the arrows.
- `sci_anim schema | jq '.["$defs"].ObjectFile.allOf[].then | select(.properties.type.const == "pattern")'` prints the fields, and `jq '.["$defs"].LayoutFile'` and `jq '.["$defs"].CopyFile'` the layout's and copy's.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.

- **Draw.** Run `$SKILL/scripts/run "$RUN" patterns-draw-3 frame examples/patterns.json --scene-time 3 -o "$RUN/patterns-3.png"`. Exit 0. View the PNG: top left a cube of 64 lit spheres running from blue at the back to red at the front, top middle 12 white ticks inside a gray circle with longer ticks at 12, 3, 6 and 9 o'clock, top right yellow beads turning green up a gray helix, and below a field of arrows circling counterclockwise, short and blue in the middle and long and red at the edges.
- **Face.** Run `frame examples/facing_copies.json --scene-time 2 -o "$RUN/facing-2.png"`. Exit 0. View the PNG: on the left yellow cones whose tips point up the helix at every copy, in the middle blue to red prisms laid along the second helix, and on the right green arrows pointing along a gray wave, down where it falls and up where it rises. Delete `faces` from each layout with `jq` and draw it again: the arrows and the cones all point right. `inspect` at 2 gives `.objects.flow.pattern.copies[].rotation` as `[0, 0, a]` with `a` from -60.5 at copy 0 through 0 at copy 2 to 60.5 at copy 4, and `.objects.units.pattern.copies[0].rotation` is about `[80.56, 17.88, 2.92]`, the cone turned to the tangent and then by its own `-90` about z. `validate` of a `faces` of `"normal"` exits 1 with `unknown_value` at `/objects/flow/layout/faces`.
- **Face a moving curve.** Write a pattern of arrows `along` the spoke `{ "x": "u*cos(t)", "y": "u*sin(t)", "z": "0", "u_range": [0.5, 3] }` with `"faces": "tangent"` and a 3 s follow with `"sets": "shape"`. `inspect` at 0, 1.5 and 3 gives every copy's rotation as `[0, 0, 0]`, `[0, 0, 85.9437]` and `[0, 0, 171.8873]`, and the frames show the arrows pointing out along the spoke as it turns.
- **Palette.** Run `validate`, `frame --scene-time 6` and `inspect --scene-time 6` on `skills/sci-anim/examples/sequence_ladder.json`. Exit 0. View the PNG: two gray rails and seven rungs, each split into two halves; the left halves from the bottom red, yellow, green, blue, green, yellow, red, the right halves yellow, red, blue, green, blue, red, yellow; the letters A T G C G T A beside the left rail and their partners beside the right; and a legend of the four nucleobases, each in its color. `[.objects.left_nucleobases.pattern.copies[].colors.stroke]` is `["#fc6255", "#ffd23f", "#83c167", "#58c4dd", "#83c167", "#ffd23f", "#fc6255"]`. `check-layout` on it exits 0. With `jq`, set `palette_index` to `[0, 1, 2, 4, 2, 1, 0]` and `validate` exits 1 with `out_of_range` at `/objects/left_nucleobases/copy/palette_index/3`; to `"j/2"`, `out_of_range` at `/objects/left_nucleobases/copy/palette_index` naming copy `[0, 1, 0]` and 0.5; to `[0, 1]`, `wrong_length` there; to `true`, `invalid` there; and a palette color of `"red"` gives `bad_format` at `/objects/left_nucleobases/copy/palette/<n>`. `examples/patterns.json` and `examples/facing_copies.json` draw the same bytes as sci_anim 0.14.0 at a few scene times, so a pattern with `to_color` is unchanged.
- **Create.** Draw `--scene-time 1.5` and run `inspect` there. The lattice shows some spheres whole, some turning into being and the rest absent; `[.objects.lattice.pattern.copies[].created] | map(select(. > 0)) | length` is 40 of 64, and copy `[0, 0, 0]` is created before copy `[3, 3, 3]`.
- **Inspect.** At 1.5, `.objects.beads.pattern.copies[0].colors.color` is `#ffd23f` and `.copies[23].colors.color` is `#83c167`. `.objects.lattice.pattern.copy_count` is 64.
- **Stages.** `stages examples/patterns.json --from 1 --to 1.1` reports 7 stages, and the count of lattice copies with `created` above 0 rises from 27 to 29 across them.
- **Errors.** Write a scene file into `$RUN` with a pattern whose `shape` is a `text`, one whose shape is a `circle` of `radius` -1, a grid of `[101, 100, 1]`, and a ring whose `copy` writes `"rotation": { "z": "30*j" }` and a `color_share` with no `to_color`. `validate` exits 1 with `out_of_range` at `/objects/<id>/shape/radius` (the schema's); fix that and it gives `invalid` at `/shape/type`, `out_of_range` at `/layout/counts`, `bad_format` at `/copy/rotation/z` naming `j` and column 4, and `missing_field` at `/copy/to_color`. A ring of 6 with `"scale": { "x": "sqrt(i - 2)" }` gives `out_of_range` at `/copy/scale/x` naming copy `[0]`. A `linear-map` of a pattern is `not_mappable` at its `objects` entry, a morph into one `not_morphable` at `into`, and a follow `along` one `no_curve` at `along`.
- **Layout.** `$SKILL/scripts/run "$RUN" patterns-layout check-layout examples/patterns.json --out "$RUN/layout"` exits 0 with no problem, and `stages.txt` has `lattice (pattern): "64 copies of sphere in a grid"`. `cargo test --test check_layout` covers a label between copies passing, a label on a copy read against it, a label across copies naming `[1, 0, 0], [2, 0, 0], [3, 0, 0]`, copies in a reserved region, and text over a sphere read against `#eeeeee` and its shaded `#c7c7c7`.
- **Video.** `render examples/patterns.json -o "$RUN/patterns.mp4"` reports 480 frames, and `ffprobe` counts 480 frames at 1920x1080 and 60 fps. `frames` writes 480 PNGs.
- **Proof.** The PNGs, reports and a sentence per PNG in `$RUN`.

## Gotchas

- A pattern draws its copies as one picture, so a copy's `scale` thins a flat copy's stroke with it. That is intended, not a stroke bug.
- `color_share` is clamped to 0 to 1, so an expression that runs past 1 holds `to_color` rather than failing.
- A grid's `counts` are along the pattern's own axes, so a `rotation` of the pattern turns the rows.
- A lattice seen along one of its axes stacks its columns into streaks. Turn it about two axes, such as `[35, 45, 0]`, to show it as a cube.
- Copies that face a helix do not twist about it, so their y axes turn against the helix's axis, 53 degrees a turn on the example's helix. A flat copy along a helix shows edge on in places, and that is intended; the example draws cones for that reason.
- A copy's `rotation` in `inspect` is its principal Euler angles in the pattern's own space, each copy on its own, so two neighbors along a helix can report angles far apart though they turn a little.
- A sphere copy is cut more coarsely than a lone sphere, as an atom's ball is, so a large copy shows its facets at the outline.
