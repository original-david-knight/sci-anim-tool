# Run reactions

A `reaction` turns reactant molecule objects into product molecule objects over its span: each atom moves straight from its reactant position to its product position, broken bonds thin away and formed bonds grow. By default each product is turned by the fit, one rotation that lines the products up with the reactants, and then by its own `rotation`. With `"fit": false` each product ends at exactly its own `position` and `rotation`. `inspect` reports the product's rotation after the reaction and the reaction's `fit_rmsd`.

## Sub-features

- `reaction-fit` turns each product by the fit, then by its own `rotation`, so `inspect` reports a rotation the scene file never wrote.
- `reaction-fit-off` with `"fit": false` ends each product at exactly its declared `rotation`, and loose atoms rest as their structure file lays them out.
- `reaction-fit-bad` refuses a `fit` that is not a boolean, at its JSON path.

## How to get to it (user POV)

- Write a `reaction` animation in a scene file, with `"fit": false` to place products by hand. `sci_anim inspect` after the reaction's span reports each product object's `rotation`, and `reactions[].fit_rmsd` at any scene time.
- `sci_anim frame` after the reaction shows the products as they rest.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. The lesson's structure files are relative to the scene file, so copy them beside each variant: `mkdir -p "$RUN/hydration" && cp -r skills/sci-anim/examples/structures "$RUN/hydration/"`.

- **Fit.** `$SKILL/scripts/run "$RUN" reaction-fit inspect skills/sci-anim/examples/hydration.json --t 16.5`. Exit `0`. `jq -c '.objects.ethanol.rotation' "$RUN/reaction-fit.stdout.json"` prints about `[98.53,15.57,-162.01]`, though the scene declares `[0, 60, 0]`: the fit turned it first.
- **Fit off.** `jq '.timeline[9].animations[0].fit = false' skills/sci-anim/examples/hydration.json > "$RUN/hydration/fit-off.json"`, then `$SKILL/scripts/run "$RUN" reaction-fit-off inspect "$RUN/hydration/fit-off.json" --t 16.5`. Exit `0`. `.objects.ethanol.rotation` is `[0.0,60.0,0.0]`, and `.reactions[0].fit_rmsd` is about `3.41`, how far the ethanol as declared lies from the reactants.
- **Loose atoms rest flat.** For a structure file of loose atoms laid out flat (every z 0) as a product, inspect after the reaction: with `"fit": false` every `.objects.<id>.atoms[].position[2]` is `0` and its `rotation` is `[0,0,0]`; with the fit on the atoms tilt. Draw a `frame` at that scene time for each and view both: the fit-off ring is symmetric about the middle.
- **Bad fit.** `jq '.timeline[9].animations[0].fit = "no"' skills/sci-anim/examples/hydration.json > "$RUN/hydration/fit-bad.json"`, then `$SKILL/scripts/run "$RUN" reaction-fit-bad validate "$RUN/hydration/fit-bad.json"`. Exit `1`, with an error at `.path` `/timeline/9/animations/0/fit`, `.code` `wrong_type`.
- **Proof.** The `.stdout.json` and `.exit` files, the variant scene files and the frames in `$RUN`.

## Gotchas

- A variant of a lesson written outside its folder loses its structure files. Copy `structures/` beside it.
- A product's `rotation` reported by `inspect` before the reaction ends is the declared one either way. Inspect after the reaction's span.
- With the fit on, moving a reactant changes the fit, so the products' reported rotations change too.
