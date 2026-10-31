# How scene files work

A scene file is a JSON file that describes one animation: the objects on the stage, the camera that looks at them, and a timeline of steps that change them. sci_anim reads the file and draws every frame from it.

A scene file is data, not a program. It has no loops, no variables and no arithmetic. That choice is what makes sci_anim work well for agents:

- `sci_anim validate` checks the whole file before anything draws, and reports each scene error with the JSON path of the bad field, so an agent fixes the field it names.
- Every frame is a pure function of the scene file and the scene time. `inspect` and `frame` show any moment without playing the ones before it.
- The file is easy to write from another program. For a big scene, an agent writes a [scene generator](#write-a-scene-generator-for-big-scenes) and runs it to write the scene file.

This page explains the parts of a scene file and how they fit together. The [scene file reference](scene-reference.md) lists every field of every type, and `sci_anim schema` prints the same facts as JSON Schema.

## A whole scene

This scene draws axes and the curve y = x², fades in a dot, carries the dot along the curve, and fades in an equation while the dot moves:

```json
{
  "scene_config": { "width": 1280, "height": 720, "fps": 30 },
  "objects": {
    "title": { "type": "text", "text": "A dot on y = x²", "size": 0.45, "position": [0, 3.4, 0] },
    "ax": { "type": "axes", "width": 7, "height": 5, "position": [0, -0.4, 0],
            "x": { "range": [-3, 3], "spacing": 1, "label": "x" },
            "y": { "range": [-1, 9], "spacing": 2, "label": "y" } },
    "curve": { "type": "function-plot", "axes": "ax", "expression": "x^2",
               "stroke": { "color": "#58c4dd", "width": 0.06 } },
    "ball": { "type": "dot", "axes": "ax", "position": [-2, 4, 0], "radius": 0.12,
              "fill": { "color": "#fc6255" }, "opacity": 0 },
    "rule": { "type": "math", "tex": "y' = 2x", "size": 0.5, "position": [4.6, 1.5, 0], "opacity": 0 }
  },
  "timeline": [
    { "type": "play", "duration": 1.5, "animations": [
      { "type": "write", "object": "title" },
      { "type": "create", "object": "ax" }
    ] },
    { "type": "play", "duration": 2, "animations": [
      { "type": "create", "object": "curve" },
      { "type": "fade", "object": "ball", "to": 1, "start": 1.5 }
    ] },
    { "type": "play", "duration": 3, "animations": [
      { "type": "follow", "object": "ball",
        "law": { "x": "-2 + 4*t/3", "y": "(-2 + 4*t/3)^2", "z": "0" } },
      { "type": "fade", "object": "rule", "to": 1, "start": 1, "end": 2 }
    ] },
    { "type": "wait", "duration": 1 }
  ]
}
```

`sci_anim validate` reports a `duration` of 7.5 seconds: four steps of 1.5, 2, 3 and 1 seconds. The rest of this page explains each part.

## The top level has five keys

| Key | Holds | When you leave it out |
|---|---|---|
| `objects` | Every object, keyed by its object id. | Required. |
| `timeline` | The steps, played one after another from scene time 0. | No steps: a scene one frame long. |
| `scene_config` | The frame `width` and `height` in pixels, the `fps`, and the `background`. | 1920 by 1080 at 60 frames per second on `#1e1e1e`. |
| `camera` | Where the scene is seen from at scene time 0. | A camera at `[0, 0, 10]` that looks at the origin. |
| `sounds` | Sound files, each started at a chapter. | A silent video. |

The `background` is an opaque `#rrggbb` color or `transparent`. A transparent scene renders to PNG frames with `frames`, for another tool to lay over footage. An MP4 cannot carry transparency.

## Positions are in world units

The stage is a 3D world. x points right, y points up, and z points toward the default camera. Positions, sizes and stroke widths are in world units, not pixels.

The default camera stands at `[0, 0, 10]`, looks at the origin, and has a vertical field of view of 45 degrees. It shows about 8.3 world units from bottom to top in the plane z = 0, and 14.7 across at 16:9. So `[0, 3.4, 0]` sits near the top of the frame, and `[-7, 0, 0]` near its left edge. A flat shape faces the default camera unless you rotate it.

Objects on axes are the exception. A dot on axes, or a function plot, gives its values in the axes' data coordinates, and the axes carry them into the world. In the scene above, the dot's `[-2, 4, 0]` is the point x = -2, y = 4 on the axes `ax`, wherever those axes are.

The reports speak both languages. `inspect` gives each object's bounding box in world units and in pixels, and `check-layout` describes faults in pixels.

## Objects

Each entry in `objects` is one object, written under its object id, which the timeline uses to name it. Its `type` picks the shape, and the type's own fields describe it:

```json
"disc": { "type": "circle", "radius": 1.5, "stroke": { "color": "#58c4dd" }, "fill": { "color": "#58c4dd", "opacity": 0.3 } }
```

Every object also takes these fields:

| Field | Meaning | Default |
|---|---|---|
| `position` | Where the object sits, as `[x, y, z]`. | The origin. |
| `rotation` | Angles in degrees about the object's x, then y, then z axis. | `[0, 0, 0]` |
| `scale` | A factor along each of the object's own axes. A stroke keeps its width under any scale. | `[1, 1, 1]` |
| `opacity` | 1 shows the object and 0 hides it. | 1 |
| `attach` | Places the object beside another object, or beside an atom group of a molecule, so it moves with it. A caption is a text or math object with an attachment. | None. |

The order of keys in a scene file does not matter.

sci_anim has 26 object types. This list groups them by what they draw. The [scene file reference](scene-reference.md#objects) has the fields of each.

- Flat shapes: `circle`, `dot`, `line`, `arrow`, `polygon`, `arc`, and `path`.
- Text and math: `text`, `math` typeset from LaTeX, `table`, and `readout`, a number that changes.
- Plots: `axes`, `number-line`, `function-plot`, `point-plot`, and `bar-plot`.
- Solids and 3D: `sphere`, `prism`, `revolution`, `tube`, `surface`, `spring`, and `3d-axes`.
- Chemistry: `molecule`, read from a structure file or a PDB file.
- Repetition: `pattern`, one shape drawn at many places, such as a crystal lattice or the ticks of a clock.
- Motion: `trail`, the path an object leaves behind it.

## The timeline is a list of steps

The timeline plays its steps one after another. A step is one of three types:

- `play` plays its `animations` over its `duration` in seconds.
- `wait` holds the stage still for its `duration`.
- `chapter` names a run of steps with an `id`. A chapter lasts as long as its steps, or its `min_duration` when that is longer, and holds the stage as its last step left it until then.

The scene lasts the sum of its steps. Scene time is the number of seconds since the start of the scene, and every command that looks at one moment takes it as `--scene-time`. `validate` reports when each step and each chapter starts and ends, so you can check the timing against your plan before you draw anything.

## Animations change one thing over a span

An animation changes one object, or the camera, over its span. The span is the whole step unless the animation sets `start` or `end`, in seconds from the start of its step. In the scene above, the dot fades in over the last half second of a 2-second step because its fade sets `"start": 1.5`.

An animation's `easing_curve` shapes its progress over its span: `linear`, `slow-start`, `slow-end`, `slow-both` or `overshoot`. The default is `slow-both`.

Animations in one step play at the same time. Two animations whose spans overlap cannot change the same value of the same object, but one can follow the other in the same step.

sci_anim has 17 animation types. The [scene file reference](scene-reference.md#animations) has the fields of each.

- Bring an object in: `create` draws a shape or grows a solid, and `write` writes text, math and tables.
- Ease a value to a `to`: `move`, `scale`, `rotate`, `fade`, `recolor`, and `count`, which counts a readout to a value.
- Reshape: `morph` turns one shape into another, `reaction` turns reactant molecules into products, and `linear-map` carries objects through a 2x2 matrix.
- Light one part: `highlight` lights an atom group of a molecule, a cell of a table, a copy of a pattern, or a bar of a bar plot.
- Move the camera: `camera-move`, `camera-look`, and `camera-zoom`.
- Set a value at every scene time: `follow` and `simulate`.

### When an object shows

An object shows from scene time 0 as declared, unless an animation brings it in:

- An object that the timeline creates or writes, or that a morph or a reaction makes, shows nothing until that animation begins. It enters at opacity 1.
- An object that enters by a fade is declared with `"opacity": 0` and faded to 1, like the dot and the equation in the scene above. A dot has no stroke to create, so a fade brings it in.
- An object with no create, write, morph, reaction or fade shows for the whole video.

A create or a write does not raise opacity. An object declared at `"opacity": 0` and then only created stays hidden, and `check-layout` reports that it never shows.

### A follow sets a value from a law

Most animations ease a value from where it is to a `to`. A `follow` sets a value at every scene time instead, from a law: expressions in `t`, the law time in seconds since the span's start. A pendulum swings by a follow of `rotation`, a planet orbits by a follow of `position`, and the camera circles a solid by a follow of `camera-position`.

The follow in the scene above carries the dot along the curve. Its law gives x = -2 + 4t/3 and y = x², so the dot runs from x = -2 to x = 2 over 3 seconds and stays on the plot.

A law gives the value outright. At the start of its span it must give the value the object already has, or `validate` reports a `law_jump` error with both values. A follow can also carry an object along another object's curve with `along`, and a `simulate` animation integrates rates, such as a pendulum with drag.

## Chapters carry narration

A chapter is a named run of steps. `validate` and `inspect` report chapters by their `id`, and a sound can start at one:

```json
"sounds": [{ "file": "voice/intro.mp3", "at": "intro" }]
```

A narrated video has one chapter for each voice clip, with the clip's length as the chapter's `min_duration`, so the picture waits for the voice. `render` mixes the sounds under the video. The sound file's path is relative to the scene file.

## Write a scene generator for big scenes

A scene file cannot loop or compute. When a scene repeats a shape with small changes, cuts a shape into many pieces, or places objects by arithmetic, write a scene generator instead: a short program that builds the objects and the timeline and writes the scene file. The sci-anim skill's examples include scene generators in Go, such as `skills/sci-anim/examples/spring_wave.go`. Edit the generator and run it again, never the scene file it writes.

One shape at many places needs no generator. A `pattern` object draws it at every copy of a layout, such as a lattice, a ring or a row.

## Check a scene before you render it

Each command shows a different view of the same scene file:

1. `sci_anim validate scene.json` reports every scene error, or the timing of a good file.
2. `sci_anim inspect scene.json -t 4.5` prints where every object is at scene time 4.5, with no GPU.
3. `sci_anim frame scene.json -t 4.5 -o frame.png` draws that moment.
4. `sci_anim check-layout scene.json --out check` finds clipped objects, overlapping text, text that is too small or too faint, and more, at every step, and draws contact sheets to look at.
5. `sci_anim render scene.json -o scene.mp4` renders the video.

The [command reference](cli.md) describes each command, its flags and its report.

## Find every field

- The [scene file reference](scene-reference.md) lists every field of every object, animation and step type, with its unit, default and range.
- `sci_anim schema` prints the JSON Schema that `validate` checks against.
- `skills/sci-anim/scripts/fields <type>` prints the fields of one type, such as `fields arrow`. It needs `jq`.
- `examples/` and `skills/sci-anim/examples/` hold working scenes. [examples/README.md](../examples/README.md) says what each one shows.
- `GLOSSARY.md` defines each word the scene file uses, such as span, follow and attachment.
