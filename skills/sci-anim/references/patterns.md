# Patterns: lattices, rings, rows and fields of copies

Read `instructional-videos.md` first, and `solids.md` when the copies are solids.

A `pattern` draws one shape at every copy of a layout. Use one when repetition is the point of the picture: the atoms of a crystal, the ticks of a ruler, a ring of charges, beads on a strand, an arrow at every point of a vector field. A pattern is one object, so one `rotate` turns the whole lattice, one `fade` fades every copy, and `inspect` reports it once, with each copy inside it.

## Write a pattern

```json
"lattice": {
  "type": "pattern",
  "shape": { "type": "sphere", "radius": 0.2, "material": { "type": "standard", "color": "#58c4dd" } },
  "layout": { "type": "grid", "counts": [4, 4, 4], "spacing": 0.8 },
  "copy": { "color_share": "k/3", "to_color": "#fc6255" },
  "rotation": [20, 30, 0]
}
```

- `shape` is the shape every copy draws, written as an object's shape with no `position`, `rotation`, `scale` or `opacity`: a `circle`, `dot`, `line`, `arrow`, `polygon`, `arc`, `path`, `sphere`, `prism`, `revolution`, `tube` or `surface`. Text, math, plots, axes and molecules are not drawn by a pattern: write separate objects for labels that differ.
- `layout` says where the copies go:

| Layout | Write | Copies stand |
|---|---|---|
| Grid | `{ "type": "grid", "counts": [nx, ny, nz], "spacing": s }` | in rows `s` apart along x, y and z, centered on the pattern's `position` |
| Ring | `{ "type": "ring", "count": n, "radius": r, "start_angle": 90 }` | evenly around a circle in the pattern's own xy plane, the first at `start_angle` degrees from +x, counterclockwise |
| Along | `{ "type": "along", "count": n, "curve": { "x": ..., "y": ..., "z": ..., "u_range": [a, b] } }` | evenly by length along a space curve, written as a `tube`'s `curve`, expressions or, with sci_anim 0.12.0 or later, `{ "points": [[x, y, z], ...] }`; both ends on an open curve, and no two meeting on a closed one. Add `"faces": "tangent"`, with sci_anim 0.7.0 or later, to turn each copy along the curve |

- A pattern has at most 10,000 copies.
- The object's own `position`, `rotation`, `scale` and `opacity` place and show the whole pattern.

## Give each copy its own values

`copy` holds expressions that every copy is evaluated with, when the scene file is read. Each reads the copy's index, `i` (and `j` and `k` in a grid), each from 0, and its place, `x`, `y` and `z`, where the layout puts it in the pattern's own space.

- `position`: `{ "x", "y", "z" }`, each left out stays at the place. `"y": "0.3*sin(2*x)"` lifts a row of dots into a wave.
- `rotation`: degrees, each left out is 0. `"z": "30*i"` turns each copy of a ring a little more.
- `scale`: each left out is 1. The pattern draws its copies as one picture, so a copy's scale sizes its stroke with it, and so does the pattern's scale.
- `color_share` with `to_color`: each copy's colors are mixed toward `to_color` by `color_share`, clamped to 0 to 1. `"color_share": "i/11", "to_color": "#fc6255"` runs a ring of 12 from its shape's color to red.
- `palette` with `palette_index` (sci_anim 0.15.0 or later): each copy picks one color of `palette` by its `palette_index`, counted from 0, and is drawn in it. The index is an expression, such as `"mod(i, 4)"`, or a list of one whole number a copy in index order, as a scene generator writes a sequence. `color_share` still mixes toward the pick, and is 1 unless you write it. Write `palette` or `to_color`, not both.

Color each rung of a ladder by its nucleobase, for the sequence ATGCGTA, with a palette of A, T, G and C:

```json
"copy": {
  "palette": ["#fc6255", "#ffd23f", "#83c167", "#58c4dd"],
  "palette_index": [0, 1, 2, 3, 2, 1, 0]
}
```

A pattern draws no text, so label the copies with separate `text` objects, and add a legend of the palette's meanings, one `text` in each color. `../examples/sequence_ladder.json` draws the two strands this way, the partner strand's half rungs from the complement's indices. `validate` refuses an index that is not a whole number within the palette, at the copy or list entry, and a list whose length is not the copy count. Keep the palette as a named constant in a scene generator when several patterns share it.

| Picture | Write |
|---|---|
| Dashed line | not a pattern: a `line` whose `stroke` has `"dash": [0.1, 0.1]` (`math.md`, sci_anim 0.14.0 or later) |
| Ruler ticks every 0.5 | `line` from `[0, 0]` to `[0, 0.2]` in a grid of `[11, 1, 1]` at 0.5, with `"scale": { "y": "1 + step(-mod(i, 2))" }` for long ticks on whole numbers |
| Clock hour ticks | `line` from `[0, -0.15]` to `[0, 0.15]` in a ring of 12, `"start_angle": 90`, `"rotation": { "z": "30*i" }` |
| Vector field of v = (-y, x) | `arrow` from `[-0.2, 0]` to `[0.2, 0]` in a grid of `[9, 5, 1]`, `"rotation": { "z": "atan2(x, -y)*180/pi" }`, `"scale": { "x": "0.4 + 0.15*sqrt(x^2 + y^2)", "y": "0.4 + 0.15*sqrt(x^2 + y^2)" }` |
| Beads on a helix | `sphere` along a helix curve, `count` 20 |
| Units pointing along a strand | a cone `revolution` along a helix curve with `"faces": "tangent"` and `"rotation": { "z": "-90" }` |
| Simple cubic crystal | `sphere` in a grid of `[3, 3, 3]` |

## Face along the curve

An `along` layout with `"faces": "tangent"` turns each copy so its own x axis points along the curve at its place, toward the curve's end, as a carry's `faces` turns its object. Draw the shape pointing right, toward +x, as an arrow from `[-0.2, 0]` to `[0.2, 0]`:

```json
"units": {
  "type": "pattern",
  "shape": { "type": "arrow", "from": [-0.2, 0], "to": [0.2, 0], "stroke": { "color": "#ffd23f", "width": 0.04 } },
  "layout": { "type": "along", "count": 9, "faces": "tangent", "curve": { "x": "1.6*u", "y": "0.9*sin(pi*u)", "z": "0", "u_range": [-1, 1] } }
}
```

- A copy's own z axis stays on +z along a curve in the pattern's xy plane, so flat copies keep facing the default camera. On any other curve it starts as near +z as it can, or +y on a curve that starts along z, and the copies never twist about the curve, as a tube does not.
- The copy's `rotation` then turns it further, about its own axes as the tangent left them: `"rotation": { "x": "20" }` tilts each copy 20 degrees about the curve, and `"z": "90"` turns each copy square to the curve.
- Copies that never twist turn slowly about the curve against a helix's axis: on a helix of radius 0.8 rising 0.12 per radian of `u`, each copy's y axis turns 53 degrees about its tangent for each turn of the helix. A flat copy lies in the plane of the tangent and its own y axis, so flat copies along a helix are seen edge on in places. Draw copies that read from every side instead, such as a cone. A `revolution` of the polygon `[[0, -0.15], [0.09, -0.15], [0, 0.18]]` has its tip on its own +y, and `"rotation": { "z": "-90" }` turns that tip along the curve.
- For copies that point at a helix's axis, such as a ladder's rungs, leave `faces` out and turn each from its place: `"rotation": { "y": "180 - atan2(z, x)*180/pi" }` points each copy's x axis at the axis of a helix about y.
- A curve that reads `t` turns the copies with it as a follow with `"sets": "shape"` runs it.

`validate` evaluates every copy. An expression with no finite value at some copy, such as `sqrt(i - 2)` at copy 0, is `out_of_range` at that expression with the copy's index and place. An unknown name, such as `j` in a ring, is `bad_format` at its column.

## Animate

- `create` grows the copies in index order, each starting a little after the one before, so a few dozen copies sweep in one by one and thousands fill in like a wave: a grid row by row along x, then up y, then back along z. Each copy draws as its shape does: a stroke along its path, a fill fading in, a solid rising or turning. A pattern of dots, which have no stroke, fades each dot in.
- `fade`, `move`, `scale`, `rotate` and `follow` with a law work on the whole pattern. A `recolor` changes its shape's colors, `stroke` and `fill` for a flat shape and `color` for a solid, and each copy still mixes toward `to_color` or its pick of `palette`.
- One copy is animated alone by naming it in `copy`, beside `object`, by its `index` as `inspect` reports it: `[i, j, k]` in a grid, `[i]` in a ring or an along. Needs sci_anim 0.20.0 or later. A `move`, `fade`, `recolor` or `highlight` with `copy` changes that copy and no other, so a ladder unzips rung by rung and one codon lights up:

```json
{ "type": "move", "object": "left_nucleobases", "copy": [0, 6, 0], "to": [-1.2, 2.4, 0], "start": 0, "end": 1 },
{ "type": "highlight", "object": "left_nucleobases", "copy": [0, 2, 0], "to": 1 }
```

  - A copy's `move` takes its `to` in the pattern's own space, where its layout placed it, not in the world: a grid copy's own place is its `x`, `y` and `z` from the layout, the pattern's `position` left out. A move, scale or rotate of the whole pattern still carries a moved copy, so the two add. A copy of a pattern whose copy expressions read `t` cannot be moved.
  - A copy's `fade` sets its own opacity, which the pattern's object opacity applies on top of. A copy faded to 0 shows nothing, and `check-layout` skips it.
  - A copy's `recolor` starts from the colors it shows and ends at colors of its own. From then on a `recolor` of the whole pattern leaves that copy as it is.
  - A copy's `highlight` lights a see-through glow around it: a box a little larger than a flat copy, behind its stroke and around its fill, or a shell around a solid copy. It fades with the copy.
  - A copy index the pattern does not have is `unknown_copy`, one of the wrong length is `wrong_length`, and a `copy` on an object that is not a pattern is `not_pattern`, each at `copy`. Two animations of one value of one copy at once conflict, as for an object, and two copies animate at once freely.
- Things that each move on their own all the time, such as the cards of a sort, are still better as separate objects a scene generator writes.
- A pattern cannot morph, be named by a linear map or be carried along. `validate` reports each.

## Layout

`inspect` reports a pattern's `bounding_box` and, under `pattern`, its `copy_count` and each copy's `index`, world `position` with its pixel, `rotation` in the pattern's own space, `created`, own `opacity`, `colors`, `glow` and `glow_color`, and `bounding_box`. A moved copy is reported where it has moved, and `check-layout` checks it there. `check-layout` compares a text beside a pattern with each shown copy, not the pattern's whole box, so a label between the atoms of a lattice passes, and checks a text's contrast against the copy or solid behind it.
