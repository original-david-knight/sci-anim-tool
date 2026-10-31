---
name: sci-anim
description: Make instructional videos and animations of math, physics and chemistry with the sci_anim command-line tool, from a JSON scene file. Use whenever someone asks for an explainer, lesson, or animation of an equation, a derivation, a function plot, geometry, a wave, a force, a moving body, a molecule, or a chemical reaction, or mentions sci_anim or a scene file.
---

# sci_anim

sci_anim turns a scene file into an MP4 or a frame sequence of PNGs. A scene file is JSON data: the objects, the camera, and a timeline of steps that animate them. You plan the lesson, write the scene file, check it, look at its frames, and render.

## Read the files for your video

Read every file the video needs before you write any JSON. Each path is relative to this file.

| The video has | Read |
|---|---|
| anything (always) | `references/instructional-videos.md`, then `references/motion.md` |
| text, equations, shapes, plots, bar plots of data, tables such as the genetic code, geometry, or matrices and linear maps | `references/math.md` |
| molecules, atom groups, reactions, polymers, hydrogen bonds and nucleobase pairs, or proteins and DNA from PDB files | `references/chemistry.md` |
| solids: boxes, cylinders, cones, spheres, planets, orbits, knots, surfaces and functions of two variables, 3D axes, or anything with depth | `references/solids.md` |
| repetition: a crystal lattice, ruler or clock ticks, a ring of atoms or charges, beads along a curve, a vector field of arrows, or rungs colored by a sequence | `references/patterns.md` |
| physics: waves, forces on a body, springs, the path a body leaves, a number that counts as it changes, or motion known by its rates | `references/physics.md` |
| a narrator's voice, music, or any sound | `references/narration.md` |
| frames to lay over other footage or images | `references/overlays.md` |

A chemistry video with an equation or a plot reads `references/math.md` too.

## The tool

Check the tool first with `sci_anim --version`. This skill needs `sci_anim 0.5.0` or later, for `-t`, a reaction's `fit` and the touching-atoms check. `sci_anim skill install` arrived in 0.4.0. `--gpu` and the report's `gpu` arrived in 0.9.0. From 0.10.0 every scene time `check-layout` prints or writes to `stages.txt` lies inside the scene, so `inspect` and `frame` take it back. From 0.11.0 `inspect` reports each shown atom's `ball_radius` in pixels, and `check-layout` finds the atoms a text covers, and the atoms in a reserved region, by their balls' discs, not their centers. From 0.12.0 a space curve may be written as the `points` it passes through (`references/solids.md`). From 0.13.0 a line's, an arrow's or a spring's end may sit on a tube's curve at a `u` or on a plot's curve at an `x` (`references/physics.md`). From 0.14.0 an end may sit on an atom, and a stroke may be dashed (`references/chemistry.md`, `references/math.md`). From 0.15.0 a pattern's copies may pick their colors from a `palette` (`references/patterns.md`). From 0.16.0 axes may have a category axis, and a `bar-plot` stands one bar on each category (`references/math.md`). From 0.17.0 a molecule may be read from a PDB file, and an atom group written by its residues (`references/chemistry.md`). From 0.18.0 a `table` sets text and math in rows and columns, and a `highlight` lights one of its cells (`references/math.md`). From 0.19.0 a space curve may be the atom trace of a PDB file, and a tube through curve points may morph into another through as many (`references/solids.md`). From 0.20.0 a `move`, `fade`, `recolor` or `highlight` may change one copy of a pattern, named by `copy` (`references/patterns.md`), and a `recolor` or `highlight` one bar of a bar plot, named by `bar` (`references/math.md`). From 0.21.0 a molecule may be drawn `space-filling`, each atom a ball at its van der Waals radius, for DNA or a protein of thousands of atoms (`references/chemistry.md`). From 0.22.0 a molecule read from a PDB file may be drawn as a `cartoon`, and `inspect` lists its `secondary_structure` (`references/chemistry.md`). From 0.23.1 `check-layout` judges a text over a cartoon by the cartoon's box. From 0.23.0 `sci_anim skill install` takes `--project` and `--user`. After you upgrade sci_anim, run `sci_anim skill install` again, so the skill matches the binary. For MP4 rendering, also check `ffmpeg -version`. `frame` and `frames` need no ffmpeg.

| Command | Use it to |
|---|---|
| `scripts/fields <type>` | Print every field of one object or animation type, such as `arrow` or `follow`, with its unit, default and range. With no type it lists every type. It reads `sci_anim schema` and needs `jq`. |
| `sci_anim schema` | Print the JSON Schema of a scene file, about 230 KB. Search it with `jq` or `grep` rather than reading it whole, or use `scripts/fields`. |
| `sci_anim validate s.json` | List every scene error at once, each at the JSON path of the bad field. For a good file, print the `duration` and each step's `path`, `start` and `end` in seconds, and each chapter's `id`, `start` and `end`. |
| `sci_anim inspect s.json -t 4.5` | Print the chapter the scene time falls in, the camera, and every object's position, opacity, created share, bounding box in pixels, and `in_frame` (`whole`, `part` or `none`) at one scene time, with the `plane_map` of each object a linear map carries. No GPU. `-t` is short for `--scene-time`. |
| `sci_anim stages s.json --from 2 --to 3` | Print what `inspect` prints at every frame time from `--from` to `--to`, and at both, under `stages`, with the scene's `fps`. Use it to follow an object a `follow` moves frame by frame. |
| `sci_anim frame s.json -t 4.5 -o f.png` | Render one frame to a PNG. |
| `sci_anim frames s.json -o frames` | Render numbered PNGs into a new directory, preserving transparency. Add `--updates` for render updates. `references/overlays.md` says how to use them. |
| `sci_anim render s.json -o out.mp4` | Render the video, with the scene file's `sounds` mixed under it. Add `--updates` on a long render to read how far it has got on stderr: one JSON line about once a second, `{"update":"drawing","frames_drawn":61,"frames":270}`, then `finishing`, then `finished` once the video is written. The report on stdout is the same either way and alone says whether the render worked. |
| `sci_anim check-layout s.json --out check` | Check the layout at every step, animation span and frame time of a follow or a simulation, print each layout fault on a line of its own, and write `stages.txt` and the contact sheets into `--out`, by default a folder beside the scene file. `--no-sheet` skips the frames and contact sheets, `--ends-only` checks only the ends of steps and chapters, `--allow a,b` lets two object ids overlap on purpose, `--reserve x0,y0,x1,y1` keeps a reserved region in frame pixels clear for the whole scene, `--reserve x0,y0,x1,y1@start-end` (sci_anim 0.6.0 or later) keeps it clear from scene time start to end in seconds, and `--reserve-exempt id` lets one object sit in it. Give each of the last three once per pair, region or object. Each run empties `--out` first, so give a run you want to keep its own folder. |
| `sci_anim skill install` | Write the skill this binary carries into the `sci-anim` folder of each skills folder at user level, `~/.claude/skills` and `~/.agents/skills` for each of `~/.claude` and `~/.agents` that exists, replacing the sci-anim skill there. `--project` (sci_anim 0.23.0 or later) installs at project level instead, into the same skills folders under the root of the git repository holding the current directory. `--user` installs at user level. `--to .claude/skills` installs into that skills folder. Give at most one of the three. With none, inside a repository with stdin and stderr a terminal, it asks `Install the sci-anim skill for this repository (<root>: .claude/skills) or for your user (~/.claude/skills, ~/.agents/skills)? [p/u]` on stderr and installs nothing, with exit 1, if stdin ends unanswered. An agent's run is no terminal, so it never asks: pass `--project` or `--user` to choose. It refuses a link at `sci-anim`, so a skill linked from a checkout stays as it is. |

Every command but `check-layout` prints one JSON report. Exit code 0 is success, 1 is a fault in the scene file, 2 is anything else. `check-layout` prints its faults as lines instead and exits 0 with none, 1 when it prints one, and 2 when it cannot check, with the JSON report of why on stderr.

The commands that draw, `frame`, `frames`, `render` and `check-layout`, draw on a discrete GPU when the machine has one, then on an integrated one. The report of `frame`, `frames` and `render` names the GPU it drew on in `gpu`, such as `{"name":"NVIDIA RTX 4000 Ada Generation Laptop GPU","kind":"discrete","backend":"vulkan","driver":"NVIDIA"}`. `--gpu integrated` draws on an integrated GPU instead, and `--gpu discrete` or `--gpu cpu` (lavapipe) names the others. A kind the machine has no GPU of is a `gpu` error that lists the GPUs it has. A 4K render on one laptop's discrete GPU draws about three times as many frames a second as on its integrated one, so leave `--gpu` out unless you are told to keep the discrete GPU free.

## A scene file

```json
{
  "scene_config": { "width": 1920, "height": 1080, "fps": 60, "background": "#1e1e1e" },
  "camera": { "position": [0, 0, 10], "target": [0, 0, 0], "fov": 45 },
  "objects": {
    "title": { "type": "text", "text": "Why is the sky blue?", "size": 0.5, "position": [0, 3.3, 0] },
    "sun": { "type": "circle", "radius": 1, "position": [-3, 0, 0],
             "stroke": { "color": "#ffd23f", "width": 0.06 }, "fill": { "color": "#ffd23f", "opacity": 0.35 } },
    "ray": { "type": "arrow", "from": [-1.8, 0], "to": [2.5, 0], "stroke": { "color": "#58c4dd", "width": 0.05 }, "opacity": 0 }
  },
  "timeline": [
    { "type": "play", "duration": 1.5, "animations": [{ "type": "write", "object": "title" }] },
    { "type": "play", "duration": 1.5, "animations": [{ "type": "create", "object": "sun" }] },
    { "type": "play", "duration": 1, "animations": [{ "type": "fade", "object": "ray", "to": 1 }] },
    { "type": "wait", "duration": 2 }
  ]
}
```

- `scene_config` and `camera` may be left out, for the values above. `sounds` lists sound files (`references/narration.md`).
- Each object shows from scene time 0, as declared, unless the timeline creates or writes it, or a morph or reaction makes it: then it shows nothing until that animation begins.
- An object that enters by a create, a write, a morph or a reaction keeps opacity 1. None of them raises opacity, so one declared at `"opacity": 0` stays hidden, and `check-layout` reports that it never shows.
- An object that enters by a fade is declared with `"opacity": 0`, such as a dot, which has no stroke to create. An object with no create, write, morph, reaction or fade at all shows for the whole video.
- A step is a `play` of `animations` over its `duration`, a `wait` that holds the stage still for its `duration`, or a `chapter` with an `id`, its `steps`, and an optional `min_duration`. `references/motion.md` covers spans, easing, follows and the camera.

## Write a scene generator for anything big

A scene file has no loops, variables or arithmetic. Write it by hand only for a short scene. For most lessons, write a scene generator: a short Go program, one file with only the standard library, that builds the objects and the timeline and writes the scene file. Edit the generator and run it again, never the JSON it writes. Write one when:

- a shape is cut into pieces that each move, such as the slices of a circle or the cards of a sort, which needs one object per piece;
- a motion keeps going under many steps, such as a bouncing mass, which needs the same follow in each step;
- positions come from arithmetic, such as data units turned into world units, or text widths;
- the same object repeats with small changes, such as one caption per idea.

A generator keeps the palette and the layout as named constants at the top, has a helper for each object and step it repeats, and checks what the timing must keep, with a panic when a check fails. `examples/spring_wave.go` is a whole lesson written this way: run `go run spring_wave.go out.json` in `examples/`. It needs no `go.mod`. The tool reads a scene file's keys in any order, so `encoding/json` and `map[string]any` are enough. The examples keep each object's keys in the order they set them, with a small ordered object type, so the scene file reads like the generator. One shape at many places, such as a lattice or ticks, needs no generator: it is one `pattern` (`references/patterns.md`).

## The loop

1. Write the lesson plan, with a duration for every step, and add them up (`references/instructional-videos.md`). Cut ideas until the sum fits the length you were asked for.
2. Write the scene file, or the generator, and run `validate` until it reports `ok`. Check its `duration` against the plan.
3. Run `sci_anim check-layout <scene file> --out <folder>`. It checks clipping, overlaps, text size and contrast (at a text's object opacity, wherever a fade leaves it), plot crossings, attachments, glows, atoms, and reserved regions, at the middle and end of every step, before every fade to 0, at the start, middle and end of every animation's span, and at every frame time of every follow and simulation. Fix each layout fault. A scene with many long follows can take a minute or more to check in full, so while you fix faults, rerun with `--ends-only --no-sheet`, which checks only the ends of steps and chapters in seconds, and run the full check again before you render.
4. Look at every contact sheet it draws, 16 tiles a page, one tile per step time. Open a full-size frame where the text on a sheet is too small. Ask the questions in "Check your work" in `references/instructional-videos.md`, and fix what fails them. A layout check with no fault says nothing about whether the video teaches.
5. Read `stages.txt` beside the sheets: every object that shows at the first checked scene time, top of the frame first, with its words, how far it is drawn, its opacity and its place in pixels, then only the changes at each later one, marked `+` for an object that now shows, `~` for one that changed, and `-` for one that no longer shows. With many objects, `grep -v` the ids of repeated pieces to read the rest. It leaves out a follow's frame times. Each `t=` is a scene time `inspect --scene-time` and `frame --scene-time` take back: six significant digits, or more at the scene's end when six would round past it, such as `t=51.995999` in a scene that long.
6. Render. Take a frame from the video at the end of each idea, `ffmpeg -ss 12.5 -i out.mp4 -frames:v 1 f.png`, and look at it as the viewer will.
7. Tell the user the video's path, its duration, and what it teaches.

A layout check covers only its checked scene times, not every frame between them, and says nothing about how narration sounds.

## Examples

Each validates and passes `check-layout`. `examples/spring_wave.go` writes `examples/spring_wave.json`, `examples/polyethylene.go` writes `examples/polyethylene.json`, `examples/polymer_coil.go` writes `examples/polymer_coil.json`, `examples/codon_table.go` writes `examples/codon_table.json`, `examples/ubiquitin_fold.go` writes `examples/ubiquitin_fold.json`, and `examples/unzipping_ladder.go` writes `examples/unzipping_ladder.json`. The lessons are models to copy. The demos are working syntax for one feature: copy their objects and animations, not their pacing or framing.

| File | What it is | Copy it for |
|---|---|---|
| `examples/derivative.json` | lesson: a secant line slides into a tangent while its slope morphs from 3.5 to 2 | a math lesson on a plot, with an equation that changes in place |
| `examples/hydration.json` | lesson: water adds across ethylene's double bond, with the glow following the carbons | a reaction, atom groups, captions on molecules |
| `examples/polyethylene.go` | lesson, as a generator: three ethylenes join a growing radical chain, with the first unit lit as it grows | a polymer chain, reactions chained one into the next, a generator that reads structure files |
| `examples/spring_wave.go` | lesson, as a generator: a bouncing mass plots its own height and the viewer reads off amplitude and period | a scene generator, a motion that keeps going under many steps, a dashed line (sci_anim 0.14.0 or later), a readout as a stopwatch |
| `examples/polymer_coil.go` | demo, as a generator: a seeded random walk of 300 curve points drawn as a tangled tube, with an arrowhead carried along it | a space curve through points, a carry with `faces: tangent` along it, a generator that computes geometry (sci_anim 0.12.0 or later) |
| `examples/polystyrene.json` | demo: a polystyrene chain turned to show its benzene rings, one on every second carbon, lit one after another above and below the backbone | a ring side group, a chain turned so its rings show, structure files written by `scripts/straight_chain.go` |
| `examples/pet.json` | demo: a PET chain of two units, with its backbone rings, ester groups, one repeating unit and its acid and alcohol ends lit in turn | a ring in the backbone, atom groups for parts of a repeating unit |
| `examples/pendulum.json` | demo: a pendulum's rod and bob swing by laws | a swing by a law |
| `examples/waves.json` | demo: a travelling wave, a standing wave, a Fourier series | shape laws |
| `examples/forces.json` | demo: force arrows riding a sliding block, and a mass on a spring | ends on objects, springs |
| `examples/cross_links.json` | demo: cross-links between two wiggling chains, then an arrow that rides one point of a travelling wave | ends on a tube's curve at a `u` and on a plot's curve at an `x` (sci_anim 0.13.0 or later) |
| `examples/nucleobase_pairs.json` | demo: an A-T nucleobase pair and a G-C nucleobase pair turning, joined by two and three dashed hydrogen bonds | ends on atoms, dashed strokes, structure files written by `scripts/nucleobase_pair.go` (sci_anim 0.14.0 or later) |
| `examples/dna_double_helix.json` | demo: the Dickerson dodecamer, read from its PDB file, turning about its axis with one nucleobase pair lit | a PDB file, an atom group of two residues' nucleobases (sci_anim 0.17.0 or later) |
| `examples/ubiquitin.json` | demo: ubiquitin, read from its PDB file, turning with its α-helix lit | a PDB file, an atom group of a run of residues (sci_anim 0.17.0 or later) |
| `examples/ubiquitin_cartoon.json` | demo: ubiquitin drawn as a cartoon, turning, with its α-helix and then its β-sheet lit | a protein as a `cartoon` of ribbons and arrows, atom groups of the residue ranges `inspect` lists in `secondary_structure` (sci_anim 0.22.0 or later) |
| `examples/ubiquitin_fold.go` | demo, as a generator: a straight chain of 76 curve points folds into ubiquitin's alpha carbons while the camera pulls in, then steps aside for the molecule with its α-helix lit | an atom trace of a PDB file, a tube morphed into another curve point by curve point, a camera move during a morph (sci_anim 0.19.0 or later) |
| `examples/nucleosome_dna.json` | demo: the 147 nucleobase pairs of DNA that wrap a nucleosome, 6,021 atoms read from PDB entry 1KX5 without its histones, drawn space-filling and turning with one strand lit | `representation: space-filling`, an atom group of a whole chain (sci_anim 0.21.0 or later) |
| `examples/sequence_ladder.json` | demo: a ladder whose half rungs take the colors of the sequence ATGCGTA and its partner strand, with a legend | a pattern whose copies pick their colors from a palette (sci_anim 0.15.0 or later) |
| `examples/unzipping_ladder.go` | demo, as a generator: the ladder of ATGCGTA unzips rung by rung from the top, each half rung sliding apart with its base's letter, then the codon TGC lights up | moving and lighting one copy of a pattern, animations staggered by `start` and `end` (sci_anim 0.20.0 or later) |
| `examples/codon_table.go` | demo, as a generator: the genetic code's 64 codons as a table, each with its amino acid, then AUG lit where a ribosome starts reading | a table of text, a table written from data a generator checks, lighting one cell (sci_anim 0.18.0 or later) |
| `examples/paths.json` | demo: trails of an orbit, a thrown ball and a phase portrait | trails |
| `examples/readouts.json` | demo: speed, time and height counting beside a falling ball | readouts and counts |
| `examples/simulations.json` | demo: a pendulum with drag, and predator and prey | simulations |
