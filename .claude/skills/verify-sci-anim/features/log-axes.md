# Draw log axes

An axis of an `axes` object takes `"scale": "log"`: each power of ten is an equal step along it, its ticks sit at powers of ten, and each tick label is the power typeset in math, such as `10^{-6}`. Either axis can be log, or both for a log-log plot. A `function-plot`, a `"type": "point-plot"`, a `bar-plot` and a data point all take data values, so a dot at y = 1000 sits on the tick `10^{3}`. A function plot on a log x axis samples x evenly along the axis, and a power law on log-log axes is a straight line. A log axis' `range` starts above 0, its `spacing` is a whole number of powers of ten, and a value at or below 0 placed on it is a scene error where it is written. `inspect` reports `"scale": "log"` on the axis, `zero` at 1 on a log axis, and the unit vector of one power of ten. A number line and 3D axes stay linear. Needs sci_anim 0.25.0 or later.

## Sub-features

- `log-axes-ticks` puts a tick at each power of ten of a log axis, evenly spaced, labeled `10^{k}`, every few powers over a wide range.
- `log-axes-data` places a data point, a point plot's points and a bar's top at their data values on a log axis, as `inspect` and the frame show.
- `log-axes-loglog` draws y = x² on log-log axes as a straight line of slope 2.
- `log-axes-sampling` draws a curve on a log x axis as smoothly at its low end as at its high end.
- `log-axes-layout` passes `check-layout`: no tick label runs into another or into the axes.
- `log-axes-errors` refuses a log range reaching 0 or below, a fractional spacing, and a value at or below 0 as a data point's `position`, a `move`'s `to`, a follow's law, a point plot's point and a bar's value.
- `log-axes-unchanged` reports linear axes as before, with no `scale` field.

## How to get to it (user POV)

- Give an axis of an `axes` object `"scale": "log"` and a `range` above 0, add plots or data points that name the axes, then run `validate`, `frame`, `inspect`, `stages`, `render`, `frames` or `check-layout`.
- `skills/sci-anim/examples/log_growth.json` is an excess growing from 10^-6 to 1 on a log y axis, the plot created over 5 s from 5.5 s with a dot following `10^(1.2*t - 6)` at the pen's end.
- `sci_anim schema | jq '.["$defs"].AxisFile'` prints the fields, and `jq '.["$defs"].AxisScale'` the two scales.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.

- **Ticks.** `inspect skills/sci-anim/examples/log_growth.json --scene-time 12`: `.objects.ax.axes.y` is `{ "range": [1e-6, 1], "scale": "log", "spacing": 1 }`, and the y ticks have `value` 1e-6 to 1 with `label_text` `10^{-6}` to `10^{0}`, their world y one step of 5/6 apart. `frame` at 12 and view it: seven y ticks evenly up the axis, each labeled as a power of ten, the blue line straight from the bottom left corner to the top right, the yellow dot at its top end.
- **Data.** Write `$RUN/log-y.json`: axes x 0 to 4 over 4, y log 0.01 to 10^4 over 6, a red `dot` on them at `[2, 1000, 0]`, a `"type": "point-plot"` through `[[0.5, 0.02], [1.5, 3], [3.5, 5000]]`, and a function plot of `10^x`. `inspect` at 0: the dot's `position` is world y 2, the world y of the tick of value 1000. `frame` at 0 and view it: the dot on the `10^{3}` tick's level, the function plot straight from data (0, 1) to (4, 10^4), the point plot's three straight pieces. Copy it with a category x axis and a `bar-plot` of values 100 and 10000 on a log y from 1 to 10^4: `inspect` gives the first bar's `rectangle` top halfway up the box and the second's at the top.
- **Log-log.** Write `$RUN/log-log.json`: axes x log 0.01 to 100 and y log 10^-4 to 10^4 with `spacing` 2, 6 by 6, and a function plot of `x^2`. `frame` at 0 and view it: one straight line from corner to corner, the y ticks at every other power of ten. `inspect` gives the plot's `data_bounding_box` from (0.01, 10^-4) to (100, 10^4).
- **Sampling.** Write `$RUN/log-x-sin.json`: a log x axis 0.01 to 100, a linear y -1.2 to 1.2, and a function plot of `sin(ln(x)*3)`. `frame` at 0 and view it: even waves along the whole axis, as round at 10^-2 as at 10^2.
- **Layout.** `check-layout` on the example and on `$RUN/log-log.json`, each with `--out` into `$RUN`, exits 0. View the last frame of each: no `10^{k}` label touches its neighbor or an axis.
- **Errors.** With `jq`, write variants into `$RUN`: the example's y `range` `[0, 1]`, its y `spacing` 0.5, the dot `p` at `[0, 0, 0]`, a `move` of `p` to `[1, -1, 0]`, a follow of `p` with `"y": "1 - t"`, a point plot with the point `[1, 0]`, and a bar value of 0 on a log y. `validate` exits 1 with `out_of_range` at `/objects/ax/y/range`, `invalid` at `/objects/ax/y/spacing`, and `out_of_range` at `/objects/p/position`, at the move's `to`, at the follow's `law/y`, at the point plot's `points/<k>` and at the bar plot's `values/<k>`, each message saying the axis is a log axis that places only values above 0.
- **Unchanged.** `inspect` `examples/plot_and_morph.json` at 0: its axes report `range` and `spacing` and no `scale`.
- **Proof.** The PNGs, reports, and a sentence per PNG in `$RUN`.

## Gotchas

- A `move` of a data point goes straight in data values, so on a log axis it crosses most of the powers of ten in its first moments. A steady climb is a follow of a law that is a power of ten in `t`.
- The x axis crosses a log y axis at the low end of its range, and both crossing labels show, as at any crossing whose labels differ.
- A `sci_anim` before 0.25.0 gives `unknown_field` at the axis' `scale`. Drive `target/release/sci_anim`.
