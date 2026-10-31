# Example scenes

Each scene file here passes `sci_anim validate`, and the tests check that it still does. Render any of them to see what it draws:

```sh
sci_anim render examples/showcase.json -o showcase.mp4
```

Each scene below shows part of what a scene file can do. To copy a feature into your own scene, find the scene that shows it, then look up the fields in the [scene file reference](../docs/scene-reference.md).

The sci-anim skill has more in [`skills/sci-anim/examples/`](../skills/sci-anim/examples/): whole lessons for an agent to copy, scene generators written in Go, and short demos of single features. The skill's `SKILL.md` lists them.

## Shapes, text and math

- `curves.json` creates a line, an arrow, a polygon, an arc and an SVG path.
- `text_and_math.json` writes text in the sans and serif fonts, and LaTeX math.
- `table.json` writes a table of derivatives with math in its cells and one ruling under its header row, then lights one cell and moves the light to another.
- `timeline.json` moves, scales, rotates and fades objects, and moves the camera.

## Plots

- `plot_and_morph.json` creates axes, a number line and a function plot, and morphs one shape into another.
- `bar_plot.json` grows a bar plot of the plastic made in the first year of each decade from 1950 to 2020, one bar at a time, on axes whose x is a category axis.
- `laws.json` orbits a planet by a law continued across a chapter with `from`, circles the camera around a cut-open cone with a law, and carries a triangle along a plot as the plot is created, facing along it.

## Solids and 3D

- `sphere_and_circle.json` has a flat circle and a lit sphere, with no timeline.
- `solids.json` raises a cylinder and a box, turns a cone and a cut-open planet into being, and moves the camera around them.
- `tubes.json` draws a spring along its helix and squashes it under a box, then draws a trefoil knot that closes on itself and turns it.
- `surfaces.json` raises a potential well and a standing wave from flat, then circles the camera around them.
- `axes_3d.json` draws 3D axes with z up, raises the paraboloid z = x² + y² on them, cut at the top of their box, then walks a data point down it by a law while the camera circles.

## Patterns

- `patterns.json` grows a crystal lattice of spheres and turns it, sets 12 ticks in a ring around a dial, threads beads along a helix, and draws a vector field of arrows, each one `pattern`.
- `facing_copies.json` turns the copies of three patterns to face along their curves: cones and tilted prisms along a helix, and arrows along a flat wave.

## Chemistry

- `molecules.json` fades in caffeine and benzene from PubChem structure files, hides benzene's hydrogens, and moves the camera and turns it to a new target.
- `reaction.json` turns ethylene and hydrogen chloride into chloroethane with an atom map.
- `hydration.json` turns ethylene and water into ethanol under a reaction scheme written in math.
- `functional_groups.json` highlights atom groups of a molecule and attaches captions and arrows to them.

## Narration and overlays

- `narrated_swap.json` swaps two numbers of a row to a voice, one chapter for each voice clip in `voice/`. Each number is attached to its box, a recolor marks the pair on the words that name it, and spans lift, slide and settle both boxes in one step. The clips were spoken with `espeak-ng -s 150`.
- `transparent_overlay.json` moves and fades text, math and a circle over a transparent background, for another tool to lay over footage. Render it with `sci_anim frames`, not `render`.

## Everything at once

- `showcase.json` is about 75 seconds long, with chapters linked by camera moves. It morphs a function plot and its math from y = x² into y = x⁴, shears and turns a circle and its basis vectors, captions an atom group of caffeine, carries a glow through a reaction to its product, grows a crystal lattice as one pattern, swings a pendulum by a law, shows a cut-open revolution and a tube along a knot, and rolls a data point down a surface on 3D axes while the camera circles. The GIF at the top of the README is this scene.
