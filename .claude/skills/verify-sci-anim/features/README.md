# sci_anim verification map

This folder is the maintained source for verifying what an agent can do with sci_anim. Read this index before driving the tool, then use the matching feature file as the recipe.

## Baseline preconditions

- `cargo build --release` has finished in the checkout under test.
- `$SKILL/scripts/doctor` prints `OK`. It requires the binary to be built from HEAD and newer than every source file, ffmpeg on PATH, and a GPU that draws a frame.
- `RUN=$($SKILL/scripts/new-run <feature-id>)` gave a fresh run folder. Never reuse one from another run.
- Commands run from the repo root, so `examples/...` paths resolve.

## Driving conventions

- Run every sci_anim command as `$SKILL/scripts/run "$RUN" <check> <args...>`. It keeps cmd, report, stderr and exit code.
- Assert on `.status`, `.command`, `.errors[].kind`, `.errors[].path`, `.errors[].code` and the exit code, with `jq`. Never on message wording alone or on line positions.
- Write files a proof cites into `$RUN`, and throwaway files into `$RUN/work/`.
- Check each file the tool writes with a second tool: view the PNG, `ffprobe` the video, list the frame sequence.
- `sci_anim schema` is the field reference. Search it with `jq`; it is about 200 KB.

## Proof and skip reporting

- CLI proof is the command, the report on stdout, stderr and the exit code, plus the files written and a second tool's reading of them.
- A visual proof has frames on both sides of the change: before and after an animation's span, or the old and new behavior.
- Record the feature id and the scene file with every artifact. The check name in `run` does this when it starts with the sub-feature id.
- When ffmpeg or the GPU is missing, report the feature as not verified, with the doctor line that failed.
- Do not report an entry point as verified through another one. `frame` passing does not prove `frames`, and `render` passing does not prove `frames`.

## Feature entry contract

Each feature file starts with an H1 title and one paragraph describing what the agent sees. It then uses exactly four H2 sections in this order.

1. `Sub-features` lists short ids with one line for each behavior.
2. `How to get to it (user POV)` lists every entry point an agent uses.
3. `Driving it with the run helper` starts with `Preconditions:` and uses labeled bullets that pair each action with an exact command and an observable result.
4. `Gotchas` lists traps that can waste or invalidate a run.

Keep implementation details out of the map. Name only commands, flags, report fields, files and observable proof.

## Features

- [Validate a scene file](./validate.md) covers the ok report with step and chapter spans, every scene error with its path, and exit codes.
- [Render one frame](./frame.md) covers `frame` at a scene time, opaque and transparent backgrounds, and an out-of-range scene time.
- [Inspect a stage](./inspect.md) covers `inspect` at a scene time: camera, each object's transform, opacity, created share, bounding box and plane map, with no GPU, and `stages`, the same at every frame time between two scene times.
- [Render a video](./render.md) covers `render` to MP4, sounds, `--updates` and refusing a transparent scene.
- [Render a frame sequence](./frames.md) covers `frames` to numbered PNGs with straight alpha, and refusing an existing directory.
- [Choose the GPU](./gpu.md) covers drawing on the discrete GPU of a machine with two, `--gpu KIND` in `frame`, `frames`, `render` and `check-layout`, the report's `gpu`, a missing kind's `gpu` error, and a machine with one GPU.
- [Draw solids](./solids.md) covers prisms and revolutions, their create, fade and scene errors, the key light, and flat shapes drawing as before.
- [Draw tubes along space curves](./tubes.md) covers tubes, their create, `inspect`'s tube ends, curves through points with a carry along them, atom traces of a PDB file, expression errors at each column, and the functions `atan2`, `mod` and `step`.
- [Draw surfaces](./surfaces.md) covers surfaces, lit on the side the camera sees, their create, `inspect`'s bounding box, every scene error in `z` and its ranges, the old `y` and `z_range` refused with a hint, and `check-layout` naming a surface by its `z`.
- [Draw 3D axes](./axes-3d.md) covers `3d-axes`: three labeled axes with z up, labels that face the camera and hide behind solids, their create, `inspect`'s `axes_3d` map and ticks, their scene errors, `check-layout` against their labels, and surfaces and tubes written in their data coordinates and cut at their box.
- [Draw bar plots](./bar-plots.md) covers `bar-plot` on a category axis: bars at their values with each category labeled below the box, the create growing them one at a time, negative and cut bars, `inspect`'s `bar_plot`, `check-layout` at both orientations, every scene error, and plots drawing as before.
- [Draw tables](./tables.md) covers `table`: text and math cells in rows and columns with their rulings, the write drawing rulings then cells row by row, a `highlight` lighting one cell by its cell address, `inspect`'s `table`, `check-layout` reading each cell as text, every scene error, and text and math drawing as before.
- [Place data points on axes](./data-points.md) covers a `dot` that names 2D axes: placed, moved, followed and carried in their data coordinates, carried by the axes and staying round, an attachment on it, `inspect`'s `data_point`, and its scene errors.
- [Follow a law or carry along a curve](./follow.md) covers `follow`: a pendulum's rotation, an orbit continued across a chapter with `from`, the camera circling, a pattern pulsing in scale and fading out by laws, a carry along a plot as it is created with `faces: tangent`, `law_jump`, `carry_jump` and the other scene errors, `sci_anim stages`, and `check-layout` at every frame time of a follow.
- [Run a shape law](./shape-laws.md) covers a plot, tube, surface or pattern whose expressions read `t`, run by a `follow` with `sets: shape`: law time over the span or from a chapter, the shape held at the span's end, one sampling for a moving surface, every scene error, and `check-layout` at its frame times.
- [Place lines, arrows and springs by their ends, and dash strokes](./ends.md) covers ends on objects, on curves and on atoms: hydrogen bonds as dashed lines that stay on their atoms as nucleobase pairs turn, an end on a reactant's or a product's atom, dash patterns and their errors, cross-links that stay on two wiggling chains, an arrow on a travelling wave, force arrows that stay on a moving block with their tips unstretched, a `spring` that stretches between a fixed point and a mass, hiding with an end's object, ends on data points, `inspect`'s `ends`, every scene error, and the `coil` hint.
- [Draw trails](./trails.md) covers `trail`: the world points an object passed through over a `length` or since it began to show, behind a law, a move and a data point, its recolor, `inspect`'s `trail`, and every scene error.
- [Show readouts](./readouts.md) covers `readout` and `count`: a number with a unit counted by a `count`, set by a follow with `sets: value`, lined up on its decimal point, attached to a moving ball, `inspect`'s `readout`, and every scene error.
- [Run simulations](./simulations.md) covers `simulate`: variables integrated by their rates, positions and readouts set from them at any scene time and held after, rates that read law time, `law_jump`, every scene error, and `check-layout` at its frame times.
- [Draw patterns](./patterns.md) covers `pattern`: grids, rings and copies along a curve, facing its tangent or not, each copy's expressions, color share and pick of a palette, the create in index order, `inspect`'s copies, every pattern scene error, and `check-layout` against copies and solids.
- [Morph one object into another](./morph.md) covers `morph`: math and text glyphs paired by the characters they show, with each glyph that has no partner fading where it stands, and other shapes paired in order, and a tube folded into another curve point by curve point.
- [Run reactions](./reactions.md) covers `reaction`: products turned by the fit, `"fit": false` ending each product at its declared rotation, and a bad `fit` refused.
- [Write polymer chains](./polymer-chains.md) covers the skill's chain generator: the older monomers unchanged, polystyrene's ring side groups, PET's rings in the backbone, both example scenes passing `check-layout`, a polystyrene chain seen edge on failing it, and an unknown monomer.
- [Read PDB files](./pdb-files.md) covers a molecule read from a PDB file: its atoms by serial number, its found bonds, `inspect`'s chains and residues, atom groups written by residue picks and by nucleobase, the dodecamer and ubiquitin turning with a pair and a helix lit, every scene error, and SDF molecules drawing as before.
- [Draw molecules space-filling](./space-filling.md) covers `representation: space-filling`: the nucleosome's DNA of 6,021 atoms as touching balls turning with one strand lit, `inspect`'s representation and hidden bonds, its frame time at 1920×1080, `check-layout` by the discs of its balls, an unknown representation and a space-filling reactant refused, and ball-and-stick molecules drawing as before.
- [Draw cartoon proteins](./cartoons.md) covers `"representation": "cartoon"`: ubiquitin as coil, helix ribbons and strand arrows from its `HELIX` and `SHEET` records, `inspect`'s `secondary_structure`, its helix and then its β-sheet lit in turn, a PDB file with no records drawing as coil, every scene error, and tubes and ball-and-stick molecules drawing as before.
- [Light atom groups](./highlights.md) covers `highlight`: a glow that is off lighting up in the highlight's color from the start of its span, and a highlight with no `color` keeping the glow's.
- [Check a layout](./check-layout.md) covers `sci_anim check-layout`: layout faults, contrast at a faded text's object opacity, `stages.txt`, contact sheets, reserved regions for the whole scene or a span of scene time, and its exit codes.
- [Install the skill](./skill-install.md) covers `sci_anim skill install`: the skill written byte for byte into a skills folder given with `--to`, found under `HOME` (user level) or under the repository root (project level), the question at a terminal and its answers, user level with no terminal or no repository, conflicting flags refused, a second install replacing the first, and refusing a link or a foreign folder.
- [Animate one member of an object](./members.md) covers a `move`, `fade`, `recolor` or `highlight` of one copy of a pattern by `copy`, and a `recolor` or `highlight` of one bar of a bar plot by `bar`: a ladder unzipping rung by rung, a lit codon and a lit bar, `inspect`'s copies and bars, `check-layout` following a moved copy, every scene error, and every other scene drawing as before.
