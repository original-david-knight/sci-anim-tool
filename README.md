# sci_anim

sci_anim is a command-line tool that turns a scene file into an animation of math or science, such as a plotted function, an equation being written, or a molecule reacting. A scene file is JSON data that declares the objects, the camera and a timeline of steps. sci_anim checks it, inspects it and renders it to an MP4 or to PNG frames.

![Frames from examples/showcase.json: a plot morphing from y = x² to y = x⁴, a shear acting on a circle, caffeine with its carbonyl group highlighted, ethylene and hydrogen chloride reacting, a crystal lattice, a pendulum moved by a law, and a paraboloid on 3D axes](docs/media/showcase.gif)

The animation above is [`examples/showcase.json`](examples/showcase.json), rendered with `sci_anim render` and converted to a GIF.

## Inspired by manim

sci_anim was inspired by [manim](https://www.manim.community/), the animation engine behind 3Blue1Brown's videos. It draws the same kind of animation without manim's Python dependency, and it is shaped for an AI agent rather than for a person writing code. sci_anim is one Rust binary. It typesets LaTeX math itself, so it needs no Python and no TeX install. A scene is a JSON file of data rather than code, and every command answers in JSON an agent can read.

## sci_anim is built for agents

sci_anim is made for a coding agent, such as Claude Code or Codex, to drive. You ask your agent for a video. The agent plans the lesson, writes the scene file, checks it, looks at its frames, fixes what is wrong, and renders. You can write scene files by hand, but most people will never open one.

Every part of the tool is shaped for that loop:

- A scene file is data with no loops or code, so `validate` checks all of it before anything draws, and any frame draws on its own.
- Every command prints one JSON report, and exits with 0, 1 for a fault in the scene file, or 2 for anything else.
- `validate` lists every scene error it finds with the JSON path of the bad field, such as `/objects/disc/radius`, so the agent fixes the field it names.
- `inspect` gives the agent numbers instead of a picture: where every object is, in world units and in pixels, at any scene time, with no GPU.
- `check-layout` finds what an agent misses when it looks at frames: text cut off by the frame, text over another object, text too small or too faint to read. It draws contact sheets for the agent to look at too.

### Install the skill, or your agent will guess

The tool alone is not enough. An agent that has never seen sci_anim does not know the scene file's 26 object types and 17 animation types, how to pace a lesson, or the check-and-fix loop. The **sci-anim skill** teaches it all of that. The skill is a folder of Markdown files and examples that agents load when you ask for a video.

The binary carries the skill that matches its version, and installs it with one command:

```sh
sci_anim skill install
```

Without the skill, an agent writes scene files by trial and error against `validate`, and makes videos that pass the checks but teach badly. With it, the agent follows a tested process. It plans each step's duration, reads the reference for its subject, copies from working examples, runs `check-layout`, and looks at every contact sheet before it renders.

The skill holds:

- `SKILL.md`, the shape of a scene file, the commands, and the loop the agent follows.
- `references/`, one file for each subject: how an instructional video teaches, motion and the camera, math, chemistry, solids, patterns, physics, narration, and overlays. The agent reads the ones its video needs.
- `examples/`, whole lessons to copy, scene generators written in Go, and short demos of single features.
- `scripts/fields <type>`, which prints every field of one object or animation type.

## Get started

1. Install the requirements:
    - Rust 1.92 or later, to build sci_anim.
    - A GPU to draw frames. On Linux that is a GPU with a Vulkan driver, or Mesa's lavapipe software driver on a machine with no GPU. On macOS it is the Mac's own GPU, through Metal. `validate`, `inspect`, `stages` and `schema` need no GPU.
    - `ffmpeg` on `PATH`, for `render` and for the contact sheets of `check-layout`.
    - `jq` on `PATH`, for the skill's `scripts/fields`.

    sci_anim is developed and used on Linux. macOS is not fully tested. CI builds sci_anim and runs its tests on an Apple Silicon virtual machine, but it has not been used on a real Mac, and Intel Macs are untested. On a Mac, install Rust with rustup, then run `brew install ffmpeg jq`. If something breaks, please open an issue.

2. Install sci_anim:

    ```sh
    cargo install --git https://github.com/original-david-knight/sci-anim-tool --locked
    ```

    Run `sci_anim --version` to check that the `sci_anim` on your `PATH` is the one you built.

3. Install the skill:

    ```sh
    sci_anim skill install
    ```

    Inside a git repository, it asks whether to install the skill for that repository or for your user. See [Install the skill for a project or for your user](#install-the-skill-for-a-project-or-for-your-user).

4. Ask your agent for a video. If the agent was already running, start a new session so it loads the skill:

    > Make a 60-second video that shows why the derivative of x² is 2x, with a secant line that slides into the tangent.

    The agent writes a scene file, checks it and renders an MP4. It tells you the video's path, its duration, and what it teaches.

## Write a scene by hand

To see how a scene file works without an agent, save this scene as `scene.json`. It draws a circle and writes a title over one second, then holds for one more:

```json
{
  "objects": {
    "disc": { "type": "circle", "radius": 1.5, "stroke": { "color": "#58c4dd" } },
    "title": { "type": "text", "text": "A circle", "position": [0, 2.5, 0] }
  },
  "timeline": [
    { "type": "play", "duration": 1, "animations": [
      { "type": "create", "object": "disc" },
      { "type": "write", "object": "title" }
    ] },
    { "type": "wait", "duration": 1 }
  ]
}
```

Check it, then render it:

```sh
sci_anim validate scene.json
sci_anim render scene.json -o scene.mp4
```

`validate` prints `"status": "ok"` and a `duration` of 2 seconds. `render` writes `scene.mp4` and prints a report that names the GPU it drew on.

[How scene files work](docs/scene-files.md) explains objects, world units, the timeline and animations, and builds a larger scene.

## Documentation

| Read | To learn |
|---|---|
| [How scene files work](docs/scene-files.md) | The parts of a scene file and how they fit together, with a worked example. |
| [Scene file reference](docs/scene-reference.md) | Every field of every object, animation and step type, with its unit, default and range. It is generated from `sci_anim schema`. |
| [Command reference](docs/cli.md) | Every command, its flags, its JSON report and its exit codes. |
| [Example scenes](examples/README.md) | What each scene in `examples/` shows. |
| [The sci-anim skill](skills/sci-anim/SKILL.md) | What your agent reads: the loop it follows and the references for each subject. |
| [Glossary](GLOSSARY.md) | The words the project uses, in code, docs and the scene file. |
| [Plan](docs/plan.md) | The known limits and the work planned next. |

## What it can draw

Each video below is one scene, narrated, with sound.

Text in two fonts, math typeset from LaTeX, an equation that morphs into the next, and a table with a highlighted cell:

https://github.com/user-attachments/assets/c17b0882-e19d-410e-b7f2-f5b6164057df

Axes and a number line, a function plot that morphs into another, a triangle that follows a plot as it is created, and a bar plot on a category axis:

https://github.com/user-attachments/assets/43ab87d3-1c54-4d4a-9499-1f16abf2b927

A linear map that carries a circle, a square and the basis vectors through a shear and a quarter turn:

https://github.com/user-attachments/assets/9dab2fc2-b05f-4353-beff-50e05ba253d0

Caffeine read from a PubChem structure file, an atom group highlighted and captioned, and a reaction that moves each atom into its product:

https://github.com/user-attachments/assets/df0d5152-dd39-4f6d-831c-266227d6c23b

Lit solids made from flat shapes, a revolution cut open, tubes along a helix and a knot, and a surface on 3D axes while the camera circles it:

https://github.com/user-attachments/assets/c825d930-d6ef-4170-b473-1e56381ebfbb

Moves, turns, scales, recolors and fades on a timeline, a pendulum swung by a law, three patterns of copies, and a change that starts on a spoken word:

https://github.com/user-attachments/assets/8b5f5993-afc2-48ad-a9d9-8d9204a881d1

## Commands

Each command is described in full in the [command reference](docs/cli.md).

| Command | What it does |
|---|---|
| `sci_anim validate <scene_file>` | Lists every scene error with the JSON path of the bad field, or reports the scene's duration and the timing of each step and chapter. |
| `sci_anim inspect <scene_file> -t <seconds>` | Prints the camera and every object's position, opacity and bounding box at one scene time, with no GPU. |
| `sci_anim stages <scene_file> --from <seconds> --to <seconds>` | Prints what `inspect` prints at every frame time between two scene times. |
| `sci_anim frame <scene_file> -t <seconds> -o <png>` | Renders the frame at one scene time to a PNG. |
| `sci_anim frames <scene_file> -o <directory>` | Renders every frame to numbered PNGs that keep transparency, with no `ffmpeg`. |
| `sci_anim render <scene_file> -o <mp4>` | Renders the whole scene to an MP4 with its sounds. |
| `sci_anim check-layout <scene_file> --out <folder>` | Prints each layout fault, and writes `stages.txt` and contact sheets. |
| `sci_anim schema` | Prints the JSON Schema of a scene file. |
| `sci_anim skill install` | Installs the sci-anim skill this binary carries. |
| `sci_anim --version` | Prints the version and the git commit the binary was built from. |

## Install the skill for a project or for your user

`sci_anim skill install` writes the skill into the `sci-anim` folder of each skills folder it installs into:

- `--user` installs for your user, into `~/.claude/skills` and `~/.agents/skills`, for each of `~/.claude` and `~/.agents` that exists. With neither, it uses `~/.agents/skills`.
- `--project` installs for one repository, into the same folders under its root. Commit them, and everyone who clones the repository gets the skill.
- `--to <skills folder>` installs into that folder. Give it once for each folder.

With no flag, a person at a terminal inside a git repository is asked which to do. An agent, a script, or a run outside a repository installs for the user.

The skill must match the binary. After you upgrade sci_anim, run `sci_anim skill install` again. It replaces the skill it installed before, so do not keep your own files in that folder.

To work on the skill itself, link the folder from your checkout instead, for example for Claude Code:

```sh
ln -s "$PWD/skills/sci-anim" ~/.claude/skills/sci-anim
```

`sci_anim skill install` refuses to replace a link, so the checkout's skill stays in place.

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md) says how to build, test and send a change. If an agent makes the change, point it at [AGENTS.md](AGENTS.md).

## License

sci_anim is licensed under either of the [Apache License, Version 2.0](LICENSE-APACHE) or the [MIT license](LICENSE-MIT), at your option. The fonts, MiTeX files and structure files it bundles keep their own licenses, named in the `NOTICE` file beside each.

Unless you state otherwise, any contribution you submit for inclusion in sci_anim, as defined in the Apache-2.0 license, is dual licensed as above, without any additional terms or conditions.
