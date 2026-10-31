# Light atom groups

A `highlight` takes an atom group's glow to the glow share in its `to`, in its `color` or the color the glow already has. A glow that is off lights up in the highlight's color from the start of its span, so only its share changes. A lit glow moves from its color to the next highlight's. `inspect` reports each group's `glow` and `color` under `objects.<id>.groups`.

## Sub-features

- `highlight-color` lights a glow that is off in the highlight's `color` at every scene time of its span, never through the default yellow `#ffd23f`.
- `highlight-keeps-color` a highlight that leaves out `color` keeps the color the glow has.

## How to get to it (user POV)

- Write a `highlight` animation naming a molecule object's group. `sci_anim inspect` at any scene time reports the group's `glow` and `color`. `sci_anim frame` shows the glow.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. `skills/sci-anim/examples/hydration.json` lights its `hydroxyl` group first in `#58c4dd`, in the step from scene time 18.5 to 19.7.

- **Color from the start.** `$SKILL/scripts/run "$RUN" highlight-color-inspect-18.6 inspect skills/sci-anim/examples/hydration.json --t 18.6`, and the same at `19.1` and `19.7`. Exit `0` each. `jq -c '.objects.ethanol.groups.hydroxyl | [.glow, .color]'` prints about `[0.02,"#58c4dd"]`, `[0.5,"#58c4dd"]` and `[1.0,"#58c4dd"]`. A build before the change printed `#fdd247` and `#c3cba7` for the first two.
- **See it.** `$SKILL/scripts/run "$RUN" highlight-color-frame-19.1 frame skills/sci-anim/examples/hydration.json --t 19.1 -o "$RUN/after-19.1.png"`. View it: the glow around the OH is blue, the color of its caption. To compare, drive main's build with `SCI_ANIM=<path to it>` into `$RUN/before-19.1.png`: its glow is olive.
- **Keeps its color.** At scene time 23, after `hydroxyl` is unlit, `.objects.ethanol.groups.hydroxyl.color` is still `#58c4dd` at glow `0`.
- **Proof.** The `.stdout.json` and `.exit` files and the frames in `$RUN`.

## Gotchas

- In the first instants of a span the glow share is near 0, so a frame there shows almost no glow. Frame mid-span to see its color.
- `inspect` before a highlight's span reports the glow's color as it was, which no frame shows while the share is 0.
