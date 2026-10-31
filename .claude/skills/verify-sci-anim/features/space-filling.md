# Draw molecules space-filling

A molecule with `"representation": "space-filling"` draws each shown atom as a ball at its element's van der Waals radius and no sticks, for DNA or a protein of thousands of atoms. A lit group's glow lies close over its balls and colors them. `inspect` reports `representation`, each ball's `ball_radius` at the whole radius and every bond hidden. `check-layout` judges text against the balls' discs and skips the touching-atoms, hidden-atom and end-on-bond checks. A reaction refuses a space-filling molecule. Ball-and-stick molecules draw as before. Needs sci_anim 0.21.0 or later.

## Sub-features

- `space-filling-draw` draws the nucleosome's 6,021 DNA atoms as touching balls, turning, with one strand lit in the glow's color.
- `space-filling-report` reports `"representation": "space-filling"`, a carbon's `ball_radius` four times its ball-and-stick one, and every bond hidden.
- `space-filling-frame-time` renders the example at 1920×1080 in a time a render can take, with its balls cut into fewer triangles than ball-and-stick's.
- `space-filling-layout` passes `check-layout` on the example, and lists a text laid over the discs of the molecule's balls.
- `space-filling-errors` refuses an unknown representation with `unknown_value` at `.../representation`, and a space-filling reactant with `invalid` at its place in `reactants`.
- `space-filling-ball-and-stick-unchanged` every scene with a ball-and-stick molecule draws the same frame, byte for byte, as the build before the change.

## How to get to it (user POV)

- Give a `molecule` `"representation": "space-filling"`, as `skills/sci-anim/examples/nucleosome_dna.json` does. `sci_anim validate` checks it, `inspect` reports it, `frame` and `render` draw it, and `check-layout` checks the scene.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. `skills/sci-anim/examples/nucleosome_dna.json` fades the DNA in over 1.5 s, turns it once about the vertical from 1.5 to 9.5 with chain J lit from 2.3 to 3.5, and holds until 10.5.

- **Draw.** `$SKILL/scripts/run "$RUN" space-filling-draw-validate validate skills/sci-anim/examples/nucleosome_dna.json`: exit `0`, `.duration` `10.5`. Then `frame` at `-t 1.5`, `-t 5` and `-t 8.5` into `$RUN/nucleosome-<t>.png`, and view each: touching balls in element colors with no sticks, the superhelix turned between them, one strand yellow at 5 and 8.5 and nothing lit at 1.5.
- **Report.** `$SKILL/scripts/run "$RUN" space-filling-report inspect skills/sci-anim/examples/nucleosome_dna.json -t 5`. `jq -c '.objects.dna | [.representation, (.atoms | length), (.bonds | length), ([.bonds[] | select(.hidden | not)] | length), (.groups.strand.atoms | length)]'` prints `["space-filling",6021,6754,0,3010]`. The same scene with `"representation": "ball-and-stick"` gives each atom a `ball_radius` a quarter of this one's.
- **Frame time.** `ffprobe`-check a `render` of the example into `$RUN/nucleosome.mp4` with `--updates`: 630 frames. Divide its wall time by 630 for the frame time, and compare it with the same render of `skills/sci-anim/examples/dna_double_helix.json`.
- **Layout.** `$SKILL/scripts/run "$RUN" space-filling-layout check-layout skills/sci-anim/examples/nucleosome_dna.json --out "$RUN/work/nucleosome-check"`: exit `0`, `No problem at any of 9 checked scene times.` Copy the scene to `$RUN/over.json` with `say_strand` at `[0, 2, 0]`: `check-layout` exits `1` and lists `"say_strand" covers atoms [...] of "dna"`.
- **Errors.** A copy with `"representation": "wireframe"`: `validate` exits `1` with `unknown_value` at `/objects/dna/representation`. A reaction whose reactant is drawn space-filling: `invalid` at `/timeline/<i>/animations/<j>/reactants/<k>`, and the message says to give it `ball-and-stick`.
- **Ball-and-stick unchanged.** For each scene in `examples/` and `skills/sci-anim/examples/` with a ball-and-stick molecule, draw a frame at `-t 2` with this build and with the build before the change (`SCI_ANIM=$(command -v sci_anim)` when that is the installed one), and `cmp` the PNGs. Every pair is identical.
- **Proof.** The reports, frames, the render's wall time and the `cmp` output in `$RUN`.

## Gotchas

- `inspect` of the example prints about 3 MB, an entry for every atom and bond.
- A frame in the first instants of the highlight's span shows almost no glow. Frame mid-span or after.
- A ball's disc in `inspect` is measured at its center's depth, so a ball near the frame's edge shows a few pixels wider than its `ball_radius`, as perspective stretches it.
