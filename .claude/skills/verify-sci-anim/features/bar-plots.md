# Draw bar plots

A `"type": "bar-plot"` names 2D axes whose `x` is a category axis, `{ "categories": [...] }`, and gives one value per category in `values`. Each bar stands on the x axis at its category, 0.7 of the category's width, and reaches its value: a negative value hangs below the x axis, and a bar past the axes' box is cut at its edge. Each category's label is set in plain text below the axes' box, and the y axis stands at the category axis' low end. A `create` grows the bars from the x axis one at a time, in category order. `inspect` reports `bar_plot.bars`, each bar's `category`, `value`, own `created` share and whole `rectangle`, and the axes report `x.categories` and a tick for each category with its `label_text`. Needs sci_anim 0.16.0 or later.

## Sub-features

- `bar-plots-draw` draws the bars at their values on the category axis, with each category's label below the box.
- `bar-plots-create` grows the bars one at a time: none before the create, some part way through it, all after.
- `bar-plots-inspect` reports each bar's category, value, created share and rectangle, and the axes' categories.
- `bar-plots-cut` hangs a negative value's bar below the x axis and cuts a value past the y range at the box's edge.
- `bar-plots-layout` passes `check-layout` at 1920×1080 and at 1080×1920.
- `bar-plots-errors` refuses an empty `categories`, a value that is not a number, a value count that differs from the category count, a bar plot on axes whose `x` has a `range`, and category labels that run into each other.
- `bar-plots-unchanged` draws the existing plot examples byte for byte as the `sci_anim` before bar plots did.

## How to get to it (user POV)

- Give an `axes` object an `x` of `{ "categories": [...] }`, add a `bar-plot` that names it in `axes`, then run `validate`, `frame`, `inspect`, `stages`, `render`, `frames` or `check-layout`.
- `examples/bar_plot.json` is the plastic made in the first year of each decade from 1950 to 2020, its bars created over 4 s from 1.5 s.
- `sci_anim schema | jq '.["$defs"].ObjectFile.allOf[].then | select(.properties.type.const == "bar-plot")'` prints the fields, and `jq '.["$defs"].CategoryAxisFile'` the category axis.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.

- **Draw.** `validate examples/bar_plot.json` exits 0 with a duration of 8.5. `frame` at 8 and view the PNG: eight blue bars rising left to right, the last near the 400 tick, labels 1950 to 2020 under the box, `Mt` over the y axis.
- **Create.** `frame` at 1.4, 3.3 and 6 and view them: no bars at 1.4, the bars of 1950 to 1970 whole and 1980's part grown at 3.3, all eight at 6.
- **Inspect.** `inspect examples/bar_plot.json --scene-time 3.3`: `.objects.plastic.bar_plot.bars[3]` has `category` `"1980"`, `value` 70 and a `created` between 0 and 1, and the bars after it have `created` 0. `.bars[7].rectangle.world` runs from y -2.5 to 1.85, whole though the bar has not begun, the x axis at -2.5 and 435 of 500 up a height of 5. `.objects.decades.axes.x.categories` lists the eight labels, and `.ticks[0].label_text` is `"1950"`.
- **Cut.** Copy the example to `$RUN/cut.json` with a y `range` of `[-100, 300]` and the first value -50. `frame` at 8 and view it: the 1950 bar hangs below the x axis, which now crosses at 0, and the bars of 2000 to 2020 stop at the box's top. `inspect` gives `.bars[0].rectangle.world.min[1]` below the x axis and `.bars[7].rectangle.world.max[1]` at the box's top.
- **Layout.** `check-layout examples/bar_plot.json --out "$RUN/layout-landscape"` exits 0. Write `$RUN/portrait.json`: the example at 1080×1920 with the camera at `[0, 0, 18]`, axes 6.6 wide and 7 high at a `label_size` of 0.45, categories `’50` to `’20`, and the title and source text broken onto more lines at sizes 0.8 and 0.52. `check-layout` exits 0 there too. View the last frame of each: no category label runs into another. Remove any `examples/bar_plot-check/` a run without `--out` leaves.
- **Errors.** With `jq`, write variants of the example into `$RUN`: `categories` `[]`, `values[2]` `"35"`, seven values, an `x` of `{ "range": [0, 8] }`, and an axes `width` of 3. `validate` exits 1 with `wrong_length` at `/objects/decades/x/categories`, `wrong_type` at `/objects/plastic/values/2`, `wrong_length` at `/objects/plastic/values`, `invalid` at `/objects/plastic/axes`, and `invalid` at `/objects/decades/x/categories` naming `1950` and `1960`, whose labels run into each other.
- **Unchanged.** `frame` `examples/plot_and_morph.json`, `examples/curves.json`, `examples/laws.json` and `skills/sci-anim/examples/derivative.json` at a few scene times with `target/release/sci_anim` and with the `sci_anim` on PATH, and `cmp` each pair of PNGs: they match byte for byte.
- **Proof.** The PNGs, reports, and a sentence per PNG in `$RUN`.

## Gotchas

- The 1950 bar is 2 Mt of 500, about two pixels high, under the x axis line. It is there: `inspect` gives its rectangle.
- A bar plot has no curve, so it cannot be morphed, carried along, or hold an end. One bar is lit or recolored alone by naming it in `bar` (see `members.md`).
- A `sci_anim` before 0.16.0 gives `unknown_value` at the bar plot's `type`. Drive `target/release/sci_anim`.
