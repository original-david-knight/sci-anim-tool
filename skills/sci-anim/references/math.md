# Math: text, equations, shapes and plots

Read `instructional-videos.md` first. `../examples/derivative.json` is a complete lesson built from what this file describes: a secant line slides into a tangent while its slope morphs from 3.5 to 2. Copy its shape for a new video.

## Text and math

- A `text` object is plain characters. A `math` object is LaTeX math in `tex`. Use `math` for anything with a symbol, an exponent or a fraction, and `\text{...}` for words inside it.
- JSON doubles every backslash: `"tex": "\\frac{a}{b}"`.
- Plain text draws `–`, `→`, `×`, `·`, `≈`, `°`, `²`, `₂`, Greek and Cyrillic, so "CO₂ has two C=O bonds" can be a `text`. `validate` reports a character the fonts lack.
- `\frac`, `\sqrt`, `\sum`, `\int`, `\vec`, `\hat`, `\mathbb`, `\text`, `\mathrm`, `pmatrix`, `cases` and `aligned` all work. `validate` reports any command that does not, at its `tex`.
- To color one term, wrap it in `\textcolor`: `"tex": "y = \\textcolor{#58c4dd}{x}^2"`. `{\color{#58c4dd} x + y}` colors the rest of its group. Every other glyph takes the object's `color`.
- A color in `tex` is `#rrggbb` or a LaTeX name such as `red` or `blue`. Use the hex colors of `instructional-videos.md`, so x in the equation matches x on the plot. `validate` reports any other color, such as `[HTML]{58C4DD}`, at its `tex`.
- A colored term morphs into its partner's color, glyph by glyph.
- `align` sets which point of the box sits at `position`: the middle of the left edge, the center, or the middle of the right edge.

## Algebra on screen

A derivation reads best as one equation that changes in place.

- Declare each line of the derivation as its own math object, all at the same `position`.
- `write` the first. Then `morph` each into the next, one step each, 1.5 to 2.5 s. The target needs no write and no fade.
- A morph pairs glyphs that show the same characters, in order, and fades the rest: in `A = πr × r` into `A = πr²` the `A`, `=`, `π` and first `r` slide into place, the `×` and second `r` fade out, and the `2` fades in. So a line that keeps most of its glyphs morphs cleanly. A line whose glyphs are rearranged, such as `a + b` into `b + a`, keeps few partners and mostly fades: show a rearrangement by moving separate objects, or fade the old line out and write the new one.
- To keep the history visible, put the next line below the last (y lower by about 1.3 × `size`) and `write` it, rather than morphing.
- To point at one term, draw a `polygon` rectangle or an `arc` around it with a `create`, or `move` it out of the line. Read the term's box from `inspect`.

## Axes and plots

- `axes` takes `x` and `y` (each a `range`, optional `spacing` and `label`) and a `width` and `height` in world units, centered on its `position`. `create` draws it. An `x` of `categories` instead is a category axis for a bar plot (below).
- `function-plot` names its `axes` and gives an `expression` in `x`. Write `2*x`, not `2x`. `ln` is the natural log and there is no `log`. Angles are radians. `atan2(y, x)` takes y first, `mod(a, b)` has the sign of b, and `step(a)` is 1 from 0 on and 0 below, for a piecewise function such as `step(x)*x^2`. The plot breaks where the function jumps, with no stroke up the jump. The schema lists every function: `sci_anim schema | jq -r '.["$defs"].ObjectFile.allOf[].then | select(.properties.type.const == "function-plot") | .properties.expression.description'`.
- `point-plot` draws straight pieces through `points` in data units, for data or a curve an expression cannot write.
- A plot is placed by its axes. It has no position of its own, and it is cut at the axes' box.
- A plot morphs only into another plot on the same axes, each x moving straight up or down. Use it to show a parameter changing: `sin(x)` into `2*sin(x)`.
- A function of two variables, such as a potential well or a wave's height, is not a plot: it is a `surface`, a lit sheet whose height `z` is an expression in `x` and `y`, which `solids.md` covers. `solids.md` also covers `3d-axes`, three axes with z up.
- A curve's ends often pass the edge of the axes box. Set `x_range` to stop the curve where it leaves the box, so its end sits inside.

A point on a plot, such as the point of tangency, a root or a maximum, is a `dot` that names the axes in `axes`. It is a data point: its `position`, and every `move`, `follow` and carry of it, is in the axes' data coordinates, `[x, y, 0]` as the plot writes them. The third number is the distance off the axes' plane in world units; leave it 0. The axes carry it wherever they go, through a move, a scale, a rotate or a linear map of the axes, and its `radius` stays in world units, so it stays round however the axes stretch.

```json
"ax": { "type": "axes", "x": { "range": [-1, 3] }, "y": { "range": [0, 9] }, "width": 6, "height": 5 },
"parabola": { "type": "function-plot", "axes": "ax", "expression": "x^2", "x_range": [1, 3] },
"p": { "type": "dot", "axes": "ax", "position": [1, 1, 0], "radius": 0.08, "opacity": 0 }
```

- A `move` of `p` to `[2, 4, 0]` slides it to the data point (2, 4). A `follow` of `p` with a law writes `x`, `y` and `z` in `t` in data coordinates too, so a projectile's `(v*t, h - 4.9*t^2)` goes in as it is.
- To run it along the curve, carry it `along` the plot: `{ "type": "follow", "object": "p", "along": "parabola", "easing_curve": "linear" }`. Declare it at the curve's start in data coordinates; `carry_jump` names the data position to declare.
- `inspect` reports its world `position` and `dot.center` as for any dot, and `data_point` with its `axes` and its data `position`. An object `attach`ed to it sits where the frame shows it.
- A linear map names the axes, not a dot on them, a dot on axes is not morphed, and it is no curve to carry another object along. A dot that names 3D axes, a number line or any other object is `invalid` at `axes`: the data point on `3d-axes` is a `sphere`, which `solids.md` covers.

Everything else on a plot, such as a tangent line, a text label or a shaded area, is an ordinary object in world units. Convert a point in data units to world units with the axes' own numbers:

```
world_x = position_x + (x - (x_low + x_high) / 2) * width / (x_high - x_low)
world_y = position_y + (y - (y_low + y_high) / 2) * height / (y_high - y_low)
```

`inspect` reports the axes' `zero`, `x_unit` and `y_unit` in world units, and each tick's world position, so you can check your numbers.

- A point is a `dot` with `radius` about 0.08, on the axes as above. It has no stroke, so fade it in from `opacity` 0 rather than creating it.
- A line through two points on the curve is a `line` from world point to world point. Morph it into another line to slide it.
- An area under a curve is a `polygon` whose points run along the curve and back along the axis, with a `fill` of `opacity` about 0.35.
- To run a shape that is not a dot along a curve, carry it with a `follow` whose `along` is the plot, and declare it at the curve's start in world units; `validate` gives the `position` to declare in `carry_jump`. Add `"faces": "tangent"` to turn a triangle drawn toward +x, such as `"points": [[0.24, 0], [-0.13, 0.13], [-0.13, -0.13]]`, along the curve. See "Motion by a law or along a curve" in `motion.md`.
- To draw a curve in step with a point running along it, `create` the plot and carry the point `along` it in one step, with one span and one easing curve: the point stays on the pen's end. A carry and a create both go by length, not by x.
- A shape that is not a dot and moves by a formula is a `follow` with a `law` in world units, so convert the formula's data units with the axes' numbers above.
- Tick labels are plain numbers, with no unit, under or beside their ticks, and they cannot be moved. Where the axes cross, one label shows the crossing's value. A curve that crosses below the x axis can run through the labels there. `check-layout` lists every tick label and axis label a plot runs through. Widen the axis range, stop the plot with `x_range`, or set the y range so the curve stays clear.
- `inspect` gives each tick its `label_text` (the words it shows), `label_side` (`below`, `above`, `left`, `right`, or `below-left` for the one label where the axes cross) and `label_bounding_box`. Labels show as many decimals as the spacing has, so a spacing of π shows `3.14159265`. For ticks at π and 2π, give the axis a round spacing or none, and draw your own short `line` ticks with `math` labels such as `\pi`.
- `inspect` lists on each plot the shown text and math objects its stroke runs through, in `crosses_objects`, glyph by glyph. So a text set inside a curve's hump is clear when it is not listed. `check-layout` reports each one.
- An axis `label` sits at the high end of its axis. Extend the axis `range` a little past the data and stop the plot with `x_range`, so the label clears the curve's end.

## Bar plots

A quantity counted in groups, such as the plastic made in each of several years or the share of each polymer, is a `bar-plot`. Needs sci_anim 0.16.0 or later. Its axes' `x` is a category axis: `categories` in place of `range` and `spacing`, one label each, set in plain text below the axes' box. The plastic made in the first year of each decade, in million tonnes:

```json
"decades": { "type": "axes",
  "x": { "categories": ["1950", "1960", "1970", "1980", "1990", "2000", "2010", "2020"] },
  "y": { "range": [0, 500], "spacing": 100, "label": "\\text{Mt}" },
  "width": 10, "height": 5, "position": [0, -0.3, 0] },
"plastic": { "type": "bar-plot", "axes": "decades",
  "values": [2, 8, 35, 70, 120, 213, 313, 435], "fill": { "color": "#58c4dd" } }
```

- `values` gives one value for each category, in the order of `categories`, in the y axis' data units. A different count is `wrong_length` at `values`, and a bar plot on axes whose `x` has a `range` is `invalid` at `axes`.
- Category labels must clear each other: two that come nearer than a quarter of the axes' `label_size` are `invalid` at `x/categories`. Four-digit years fit about 8 to a 10-unit-wide axes at a `label_size` of 0.3. In a 1080×1920 frame write `’50`, `’60` and so on, and name the years in the title.
- Each bar is 0.7 of its category's width and stands on the x axis. A negative value hangs below it, so give `y` a `range` below 0 for one. The part of a bar outside the axes' box is cut, as a curve is, with no error, so a value past the top of the y range draws to the top.
- `create` grows the bars from the x axis one at a time, in category order, each taking the same share of the span. Give the span about half a second a bar.
- Put the units in the y axis `label`, such as `\text{Mt}`, and the title in a `text` above the axes. Cite the data's source in a small `text` below the category labels, or in the narration.
- A bar plot is placed by its axes, as a plot is. A `fade` fades every bar and a `recolor` changes every bar's `fill` and `stroke`. To point at one bar, name it in `bar`, beside `object`, by its index in category order from 0 (sci_anim 0.20.0 or later): a `highlight` lights a see-through frame around it that fills its category's width, and a `recolor` gives it colors of its own, which a later `recolor` of the whole plot leaves. `{ "type": "highlight", "object": "plastic", "bar": 7, "to": 1 }` lights 2020's bar. A bar index past the bars is `unknown_bar`, and a `bar` of an object that is not a bar plot is `not_bar_plot`, each at `bar`.
- `inspect` reports `bar_plot.bars`, each with its `category`, `value`, own `created` share, its whole `rectangle` in world units and pixels, `null` for a bar with no height, its `fill` and `stroke` colors, and its `glow` and `glow_color`. The axes report `x.categories`, and each category's tick with its `label_text`.
- Category k, counted from 0, is at data x = k, so a `point-plot` or a `dot` on the same axes at x = k sits on its bar: a trend line over the bars is a `point-plot` through `[k, value]`.

## Tables

Rows and columns of words, numbers or math, such as a table of derivatives or the genetic code, are one `table`. Needs sci_anim 0.18.0 or later. `rows` lists the rows from the top, each a list of its cells from the left. A cell is a string of plain text, `{ "tex": "..." }` for LaTeX math, or `""` for an empty cell:

```json
"rules": { "type": "table", "size": 0.45, "rulings": "header", "align": "left", "position": [0, -0.3, 0],
  "rows": [
    ["Function", "Derivative"],
    [{ "tex": "\\sin x" }, { "tex": "\\cos x" }],
    [{ "tex": "\\ln x" }, { "tex": "\\frac{1}{x}" }] ] }
```

- Every row has as many cells as the first. A row with another count is `wrong_length` at `rows/<row>`, and a table with no rows, or a row with no cells, is `wrong_length` at `rows` or `rows/<row>`.
- The table sizes itself from its cells: each column as wide as its widest cell and each row as tall as its tallest, plus `padding` on every side of each cell (0.4 of `size` unless set). It is centered on its `position`. `size`, `color` and `font` set every cell, as they set a `text`, and `align` puts each cell's text at the left, center or right of its cell.
- `rulings` draws strokes between the cells: `all` (the default) between every row and column and around the table, `header` one under the first row, or `none`. `stroke` sets their color and width, gray and 0.02 wide unless set.
- `write` draws a table: its rulings along their length over the first fifth of the span, then its cells glyph by glyph in reading order, row by row. A table is not created. Give it about a second for every three rows.
- `highlight` lights one cell: `{ "type": "highlight", "object": "rules", "cell": [2, 1], "to": 1 }` names the cell by its cell address, `[row, column]`, each counted from 0, the header row being row 0. The cell's rectangle fills see-through in the glow's `color`, `#ffd23f` unless set, behind its text and rulings. Light another cell and put the first back to `"to": 0` in the same step to move the light. A cell outside the table is `unknown_cell` at `cell`, and `cell` on an object that is not a table is `not_table`.
- White text on a lit cell keeps enough contrast in the default yellow on a dark background. A lighter glow color can make it hard to read, and `check-layout` says so.
- `recolor` with `color` changes every cell's text, and with `stroke` the rulings.
- `inspect` reports `table`: its `size`, `rows`, `columns` and every cell in reading order, with its `cell` address, `text` or `tex`, own `created` share, `glow`, `glow_color`, its `rectangle` and the `bounding_box` of its glyphs in world units and pixels. `check-layout` checks each cell's text as it checks a `text` object, naming it `table/row/column` such as `codons/12/1`: too small, too faint on its glow or the background, too close to another text, or covered by one. A text over a table's cells is reported as covering them. `--allow codons,caption` lets every cell of `codons` meet `caption`.
- A table must fit the frame at a readable size. At 1920×1080 with the default camera, `size` 0.3 is about the smallest text `check-layout` passes, and 17 rows of it fit at a `padding` of 0.1. Show a part of a bigger table, or split it into two.
- `../examples/codon_table.go` writes the whole genetic code, 64 codons, as a table and lights AUG.

## Shapes and geometry

- `circle`, `dot`, `line`, `arrow`, `polygon`, `arc` and `path` are flat shapes. Their points are `[x, y]` measured from the object's `position`.
- A shape needs a `stroke` to be created. `create` draws the stroke along the path, then fades the fill in.
- A stroke's `width` is in world units: 0.04 is thin, 0.06 to 0.08 reads well for the main shape. `scale` never widens a stroke.
- A dashed stroke has a dash pattern, `"stroke": { "color": "#888888", "width": 0.03, "dash": [0.1, 0.1] }`: dashes 0.1 long with gaps 0.1 long, in world units along the stroke as its width is, each from 0.01 to 100. Needs sci_anim 0.14.0 or later. Any flat shape's stroke and a plot's may be dashed, not a trail's or 3D axes' (`invalid`). Each part of the path starts with a dash and its last dash is cut where it ends, so a line placed by its ends keeps its dashes' length as it stretches, and a create draws the dashes one after another. An arrow's tip is drawn whole. A dash pattern of a morph's two looks switches halfway.
- Mark an angle with an `arc` of `radius` about 0.5 at the vertex, from the first side's angle sweeping to the second's.
- A `path` takes SVG path syntax in `d`, with y up.
- A `line` needs `from` and `to` to differ. For a length that shrinks to nothing, such as the height of a point on the x axis, morph it into a line 0.002 long, or fade it out.
- `rotate` is in degrees about x, y and z. A flat shape turns in its plane about z, around its `position`. Put a shape's `position` where it should pivot, and measure its points from there.
- A pendulum, a spring's bob or any swing is a `follow` of `rotation` with a law, such as `"z": "30*cos(2.21*t)"`, which swings about the object's `position`. `../examples/pendulum.json` swings a rod and its bob, each by its own law.
- To move a point around a circle, carry it `along` the circle with a `follow`, or put its `position` at the circle's center and draw it as a `path` disc away from that center, then `rotate` it about z with `linear` easing. A radius `line` from `[0, 0]` turns the same way. A disc of radius 0.12 at distance 2 is `"d": "M 2.12 0 A 0.12 0.12 0 1 0 1.88 0 A 0.12 0.12 0 1 0 2.12 0 Z"` with a `fill`.
- To show a copy of an object, declare a second object just like it with `opacity` 0, fade it in, and move it. Where the two overlap, a see-through fill shows darker, so move the copy away in the same step as the fade or give it a solid fill.
- To keep a number or label on a shape that moves, such as a value in its box in a sort, give the text `"attach": { "object": "box4" }` and move only the box: one `move` carries both. Add `"offset": [0, 0.8]` to sit above it instead. The attached text has no `position` of its own, faces the camera, shows in front of the box, and is gone when the box is. Fade and recolor it on its own.
- A `number-line` is one axis with ticks, for integers, fractions or a point moving along a line.
- A vector field, a row of ticks or a ring of marks is one `pattern` of an `arrow`, `line` or `dot`, with each copy's rotation and scale written as expressions in its place. `patterns.md` covers it.

## Linear maps

A `linear-map` carries every point of the objects in `objects` by a 2x2 `matrix` about `origin`, in world units: `[[a, b], [c, d]]` sends a point `(x, y)` measured from `origin` to `(a·x + b·y, c·x + d·y)`. Use it for whatever a matrix does to the plane: a shear, a reflection, a stretch along a turned axis, a rotation, a matrix that flattens the plane. Every point stays the same point of its object at every frame, so a circle, its radii and the dots at their tips stay together, which a morph does not promise.

- Draw a vector as an `arrow` and a point as a `dot`, and name them in the same map as the shape they belong to. An arrow is drawn between its mapped ends with its tip unsheared, as long as its own scale makes it, and draws nothing while its ends meet. A dot moves to its mapped center and keeps its size, so it still shows when a matrix flattens everything else.
- A stroke keeps its `width` under any matrix. Text, math and axes are mapped like any path, so a shear slants their glyphs. To keep text upright beside a point, leave it out of the map and attach it to a dot. As a caption it stays upright beside the dot's mapped center.
- A later map acts after an earlier one: maps Vᵀ, Σ and U played in that order leave each object at U·Σ·Vᵀ. Each map can have its own `origin`.
- The default `route`, `entries`, moves each entry straight from the identity, so a rotation shrinks on the way and a half turn passes through nothing. For a rotation, or any factor with a determinant above 0, give `"route": "turn"`: it turns by the rotation inside the matrix, the shorter way, while it stretches by the rest. `turn` is a scene error for a reflection or a flattening matrix.
- A plot is placed by its axes, so name the axes. A caption sits beside the object it is attached to, wherever the map carries it. A sphere or a molecule cannot be named.
- From the start of an object's first map, it cannot be moved, scaled, rotated or morphed: place it before, or carry it by another map. A fade, recolor, create or write of it plays as usual.
- `inspect` reports `plane_map` on every object a map names, with `matrix`, `shift` and `determinant`: a world point `p` lands at `matrix · p + shift`. A mapped arrow reports its world `arrow.from` and `arrow.to`, and a dot its `dot.center`. Check a dot's center against the matrix product you expect.

An SVD lesson shows A = U·Σ·Vᵀ as three maps of one circle, its two radii along the right singular vectors and the dots at their tips. Here Vᵀ turns -45°, Σ stretches by 2 and 0.5, and U turns 30°, so at the end the radii lie along 30° and 120°, 3 and 0.75 long, on the ellipse's axes:

```json
{
  "objects": {
    "circle": { "type": "circle", "radius": 1.5, "stroke": { "color": "#58c4dd", "width": 0.05 } },
    "v1": { "type": "arrow", "from": [0, 0], "to": [1.0607, 1.0607], "tip_length": 0.2, "stroke": { "color": "#fc6255", "width": 0.05 } },
    "v2": { "type": "arrow", "from": [0, 0], "to": [-1.0607, 1.0607], "tip_length": 0.2, "stroke": { "color": "#83c167", "width": 0.05 } },
    "tip1": { "type": "dot", "radius": 0.08, "position": [1.0607, 1.0607, 0], "fill": { "color": "#fc6255" } },
    "tip2": { "type": "dot", "radius": 0.08, "position": [-1.0607, 1.0607, 0], "fill": { "color": "#83c167" } }
  },
  "timeline": [
    { "type": "play", "duration": 2, "animations": [
      { "type": "linear-map", "objects": ["circle", "v1", "v2", "tip1", "tip2"], "matrix": [[0.7071, 0.7071], [-0.7071, 0.7071]], "route": "turn" }
    ] },
    { "type": "play", "duration": 2, "animations": [
      { "type": "linear-map", "objects": ["circle", "v1", "v2", "tip1", "tip2"], "matrix": [[2, 0], [0, 0.5]] }
    ] },
    { "type": "play", "duration": 2, "animations": [
      { "type": "linear-map", "objects": ["circle", "v1", "v2", "tip1", "tip2"], "matrix": [[0.866, -0.5], [0.5, 0.866]], "route": "turn" }
    ] }
  ]
}
```

At scene time 6, `inspect` reports `tip1`'s `dot.center` as `[2.598, 1.5, 0]` and the circle's `plane_map.matrix` as U·Σ·Vᵀ, `[[1.401, 1.048], [0.401, 1.013]]`. Put each factor in its own chapter with its formula, and keep the formula out of the map.

## Morph rules

A morph's source and target are flat shapes, text, math or plots, or two tubes through as many curve points (`solids.md`). The source shows until the step ends, then the target. The target is never created or written, and an object is made by a morph at most once and used up at most once. To morph A into B and back, morph B into a third object declared like A.
