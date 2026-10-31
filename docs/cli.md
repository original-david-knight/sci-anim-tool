# Command reference

`sci_anim` is a command-line tool. Each command reads one scene file and prints one JSON report on stdout, so an agent or a script can read the result without parsing text. `check-layout` and `--version` are the two exceptions, and their sections say what they print instead.

For what goes inside a scene file, read [How scene files work](scene-files.md) and the [scene file reference](scene-reference.md).

| Command | What it does | Needs a GPU | Needs `ffmpeg` |
|---|---|---|---|
| [`validate`](#validate) | Checks a scene file and reports its timing. | no | no |
| [`inspect`](#inspect) | Prints the camera and every object's state at one scene time. | no | no |
| [`stages`](#stages) | Prints what `inspect` prints at every frame time between two scene times. | no | no |
| [`frame`](#frame) | Renders one frame to a PNG. | yes | no |
| [`frames`](#frames) | Renders every frame to numbered PNGs, with transparency. | yes | no |
| [`render`](#render) | Renders the scene to an MP4 video with its sounds. | yes | yes |
| [`check-layout`](#check-layout) | Finds layout faults and draws contact sheets. | yes | for the contact sheets |
| [`schema`](#schema) | Prints the JSON Schema of a scene file. | no | no |
| [`skill install`](#skill-install) | Installs the sci-anim skill for your agents. | no | no |

`sci_anim --help` and `sci_anim <command> --help` print the same flags with a line about each.

## Exit codes and reports

Every command exits with one of three codes:

- `0` when the command did what it was asked.
- `1` when the scene file is at fault. The report lists the scene errors.
- `2` for anything else, such as a file that cannot be read or written, no GPU, a missing `ffmpeg`, or a bad command line.

A report that worked has `"status": "ok"` and names its `command`. A report that failed has `"status": "error"` and an `errors` list. Each error has a `kind`, a `code` and a `message`, and a scene error also has the JSON path of the bad field in `path`:

```json
{
  "status": "error",
  "errors": [
    {
      "kind": "scene",
      "path": "/objects/d/radius",
      "code": "out_of_range",
      "message": "-1 is less than or equal to the minimum of 0"
    }
  ]
}
```

## Choose a GPU

`frame`, `frames`, `render` and `check-layout` draw on the GPU. They take a discrete GPU when the machine has one, then an integrated one, then any other. Mesa's lavapipe software driver works on a Linux machine with no GPU, at a lower frame rate. Lavapipe is Linux only, so on macOS `--gpu cpu` is a `gpu` error. A Mac's GPU draws through Metal and reports its kind as `integrated`.

`--gpu <KIND>` draws on the first GPU of one kind instead. A kind the machine has no GPU of is a `gpu` error that lists the GPUs it has.

| `KIND` | The GPU |
|---|---|
| `discrete` | A GPU of its own, apart from the processor, such as an NVIDIA RTX. |
| `integrated` | A GPU built into the processor, such as an Intel Iris Xe or an AMD Radeon 610M. |
| `other` | A driver that names no kind, as OpenGL drivers do. |
| `virtual` | A GPU a virtual machine is given. |
| `cpu` | A driver that draws on the processor, such as lavapipe. |

The reports of `frame`, `frames` and `render` name the GPU they drew on in `gpu`:

```json
"gpu": { "name": "NVIDIA GeForce RTX 4090", "kind": "discrete", "backend": "vulkan", "driver": "NVIDIA" }
```

Two GPUs may not draw byte-identical frames, so the `gpu` field explains a difference between two renders.

## validate

```sh
sci_anim validate <SCENE_FILE>
```

Checks the scene file against the schema and the rules the schema cannot state, such as an animation that names an object id the scene does not have. It lists the scene errors it finds, each at the JSON path of the bad field, and exits 1 when there is one.

For a good file the report gives the scene's `duration` in seconds, each step's `path`, `start` and `end` in `steps`, and each chapter's `id`, `start` and `end` in `chapters` when the scene has chapters:

```json
{
  "status": "ok",
  "command": "validate",
  "duration": 2.0,
  "steps": [
    { "path": "/timeline/0", "start": 0.0, "end": 1.0 },
    { "path": "/timeline/1", "start": 1.0, "end": 2.0 }
  ]
}
```

Every other command validates the scene file first and reports its scene errors the same way.

## inspect

```sh
sci_anim inspect <SCENE_FILE> --scene-time <SECONDS>
```

Prints the scene at one scene time, with no GPU:

- `scene_time` and the chapter the scene time falls in.
- `camera`, with its `position`, `target`, `up` and `fov`.
- `objects`, keyed by object id. Each has its `position`, `rotation`, `scale`, `opacity`, the share drawn so far in `created`, its `colors`, its `bounding_box` in world units and in pixels, and `in_frame`, which is `whole`, `part` or `none`. Some object types add fields of their own, such as `glyphs` for text, each shown atom's `ball_radius` for a molecule, and the `plane_map` of each object a linear map carries.

| Flag | Meaning |
|---|---|
| `-t`, `--scene-time <SECONDS>` | Seconds since the start of the scene. Required. |

## stages

```sh
sci_anim stages <SCENE_FILE> --from <SECONDS> --to <SECONDS>
```

Prints what `inspect` prints at every frame time from `--from` to `--to`, and at both, from one read of the scene file. The report holds the scene's `fps` and a `stages` list. Use it to follow an object that a follow moves frame by frame.

| Flag | Meaning |
|---|---|
| `--from <SECONDS>` | The first scene time. Required. |
| `--to <SECONDS>` | The last scene time, `--from` or later. Required. |

## frame

```sh
sci_anim frame <SCENE_FILE> --scene-time <SECONDS> --out <PNG>
```

Renders the frame at one scene time to a PNG. The report names the `png`, its `width` and `height`, the `scene_time` and the `gpu`.

| Flag | Meaning |
|---|---|
| `-t`, `--scene-time <SECONDS>` | Seconds since the start of the scene. Required. |
| `-o`, `--out <PNG>` | The PNG to write. Required. |
| `--gpu <KIND>` | The kind of GPU to draw on. See [Choose a GPU](#choose-a-gpu). |

A frame of a scene with `"background": "transparent"` keeps its transparency, as [frames](#frames) describes.

## frames

```sh
sci_anim frames <SCENE_FILE> --out <DIRECTORY>
```

Renders every frame to numbered PNGs in a new directory, with no sound and no `ffmpeg`. Use it for a scene with a transparent background, which an MP4 cannot carry, or to hand the frames to another tool.

| Flag | Meaning |
|---|---|
| `-o`, `--out <DIRECTORY>` | The directory to create. Its parent must exist, and the directory itself must not. Required. |
| `--updates` | Prints render updates on stderr, as [render](#render) does. |
| `--gpu <KIND>` | The kind of GPU to draw on. See [Choose a GPU](#choose-a-gpu). |

The report:

```json
{
  "status": "ok", "command": "frames", "directory": "fr", "pattern": "frame-%06d.png",
  "start_number": 0, "width": 1920, "height": 1080, "fps": 60, "frames": 120, "duration": 2.0,
  "alpha": "straight", "color_space": "srgb",
  "gpu": { "name": "NVIDIA GeForce RTX 4090", "kind": "discrete", "backend": "vulkan", "driver": "NVIDIA" }
}
```

Frame `i` shows scene time `i / fps`. A scene of duration `d` gives `max(1, round(d * fps))` frames, and the reported `duration` is `frames / fps`. Empty frames and chapter holds stay in the sequence. The PNGs keep the scene's width and height, odd sizes included.

Each PNG stores straight alpha with sRGB color channels and linear alpha. Composite in linear light to get the same colors as an opaque render.

The directory holds an `.incomplete` marker until the render succeeds. A handled failure removes the paths the command created. A render that is killed can leave a marked directory behind. Use the sequence only after an `ok` report, and give a retry a new directory. Do not change the directory while the render runs.

## render

```sh
sci_anim render <SCENE_FILE> --out <MP4>
```

Renders the whole scene to an H.264 MP4 and mixes the scene file's `sounds` under it. It needs `ffmpeg` on `PATH`. A scene with a transparent background cannot render to MP4, because H.264 drops alpha. Use [frames](#frames) for it.

| Flag | Meaning |
|---|---|
| `-o`, `--out <MP4>` | The video to write. Required. |
| `--updates` | Prints render updates on stderr. |
| `--gpu <KIND>` | The kind of GPU to draw on. See [Choose a GPU](#choose-a-gpu). |

The report names the `mp4`, its `width`, `height`, `fps`, `frames`, `duration` and `gpu`.

With `--updates`, the command prints one JSON line on stderr about once a second while it draws frames, then one when `ffmpeg` is finishing the video, then one when the video is written:

```json
{"update":"drawing","frames_drawn":61,"frames":270}
```

The report on stdout is the same with or without `--updates`, and only the report says whether the render worked.

## check-layout

```sh
sci_anim check-layout <SCENE_FILE> [--out <DIR>]
```

Checks the layout of the scene at the middle and end of every step, at the start, middle and end of every animation's span, and at every frame time of a follow or a simulation. It prints each layout fault on a line of its own, such as:

```text
t=0 (the only frame): "c" is cut off by the frame: its box runs from [2000, 407] to [2266, 673] pixels
t=0 (the only frame): "a" and "b" overlap by 235 x 73 pixels
```

It finds objects the frame cuts off, text over another object, text that is too small or too faint against what is behind it, objects in a reserved region, and more.

It writes into `--out`:

- `stages.txt`, what shows at each checked scene time in words. The first scene time lists every object that shows, top of the frame first. Each later one lists only changes, marked `+` for an object that now shows, `~` for one that changed and `-` for one that no longer shows. Each `t=` value is a scene time that `inspect` and `frame` take back.
- `contact-sheet-1.png` and on, 16 tiles a page, one tile for each checked step time, and a full-size PNG of each tile. These need `ffmpeg`.

It exits 0 with no fault, 1 when it prints one, and 2 when it cannot check. On exit 2 the JSON error report goes to stderr, not stdout.

| Flag | Meaning |
|---|---|
| `--out <DIR>` | The folder for `stages.txt` and the contact sheets. Each run empties it first. The default is `<scene file name>-check` beside the scene file. |
| `--no-sheet` | Skips the frames and the contact sheets. |
| `--ends-only` | Checks only the ends of steps and chapters. Use it for fast reruns while you fix faults, then run the full check again. |
| `--allow <ID,ID>` | Two object ids whose overlap is on purpose. Give it once for each pair. |
| `--reserve <X0,Y0,X1,Y1[@START-END]>` | A reserved region, a box of the frame in pixels from its top left that no shown object may enter, such as space kept for a caption another tool adds. With `@START-END` it holds only from scene time START to END in seconds. Give it once for each region. |
| `--reserve-exempt <ID>` | An object id that may sit in the reserved regions. Give it once for each object. |
| `--gpu <KIND>` | The kind of GPU to draw on. See [Choose a GPU](#choose-a-gpu). |

For a scene with a transparent background, `check-layout` cannot know what will sit behind the text, so it reports contrast against that background as unverified. It still checks clipping, overlaps, text size and reserved regions, and it checks contrast against an opaque rectangular panel when it can prove where the panel covers. An unverified contrast does not fail the check.

A layout check covers only its checked scene times, not every frame between them. It cannot tell you whether the video teaches well, so look at the contact sheets too.

## schema

```sh
sci_anim schema
```

Prints the JSON Schema of a scene file, about 230 KB. Every field has a description with its unit, default and range. `validate` checks scene files against this same schema. The [scene file reference](scene-reference.md) is this schema as a readable page.

## skill install

```sh
sci_anim skill install [--user | --project | --to <SKILLS_FOLDER>...]
```

Writes the sci-anim skill that this binary carries into the `sci-anim` folder of one or more skills folders. The skill teaches an agent to use sci_anim. See [the README](../README.md#install-the-skill-or-your-agent-will-guess) for why your agent needs it.

| Flag | Where it installs |
|---|---|
| `--user` | User level: `~/.claude/skills` and `~/.agents/skills`, for each of `~/.claude` and `~/.agents` that exists, or `~/.agents/skills` when neither does. |
| `--project` | Project level: the same folders under the root of the git repository that holds the current directory. |
| `--to <SKILLS_FOLDER>` | That skills folder, such as `.claude/skills`. Give it once for each folder. |

Give at most one of the three. With none, a person at a terminal inside a git repository is asked which level, and the question names the folders each answer writes. If stdin ends with no answer, nothing is installed and the command exits 1. An agent, a script, or a run outside a repository gets user level with no question.

The command replaces a sci-anim skill installed there before, so files you added to that folder are lost. It refuses a link, or a folder that is not a sci-anim skill. The report lists the folders it wrote in `installed`, with the skill's `version`.

Run it again after you upgrade sci_anim, so the skill matches the binary.

## --version

```sh
sci_anim --version
```

Prints plain text, not a report: the version and the git commit the binary was built from, such as `sci_anim 0.24.0 36e2a8ec816b96379a78f4b3f5f3ed9d1c31fd44`. ` modified` follows when the work tree had uncommitted changes, and `unknown` stands in for the commit in a build without git.
