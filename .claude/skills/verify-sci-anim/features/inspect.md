# Inspect a stage

`sci_anim inspect <scene file> --scene-time <seconds>` prints the stage at one scene time as JSON, with no GPU. It gives the `camera` (`position`, `target`, `up`, `fov`) and, for each object id, its `position`, `rotation`, `scale`, `opacity`, `created` share, `colors`, `bounding_box` (`world` and `pixels`), `in_frame` (`whole`, `part` or `none`) and, for an object a linear map names, its `plane_map`. It also gives `reactions`, and the `chapter` the scene time falls in.

## Sub-features

- `inspect-transform` reports each object's position, rotation and scale as the timeline has left them.
- `inspect-created` reports created share and opacity before, during and after a create, write or fade.
- `inspect-in-frame` reports `in_frame` and the pixel bounding box.
- `inspect-plane-map` reports the plane map of each object a linear map names.
- `inspect-chapter` names the chapter at the scene time.
- `inspect-ball-radius` reports, for each shown atom of a molecule and of a reaction, its `ball_radius`: the radius in pixels of the disc its ball shows, after the molecule's scale (its largest factor), the camera and the frame size. A hidden atom has none.
- `inspect-stages` prints, with `sci_anim stages <scene file> --from <seconds> --to <seconds>`, the same report body at every frame time between the two scene times and at both, under `stages`, with the scene's `fps`.

## How to get to it (user POV)

- Run `sci_anim inspect <scene file> --scene-time <seconds>` (or `-t`).
- `check-layout` reads the stage `inspect` prints at every checked scene time and turns it into `stages.txt`.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. The GPU is not needed.

- **Transform at a scene time.** Run `$SKILL/scripts/run "$RUN" inspect-transform inspect examples/timeline.json --scene-time 1`. Exit 0. `jq -c '.objects.disc.position, .objects.disc.in_frame' "$RUN/inspect-transform.stdout.json"` prints `[-2.5,0.0,0.0]` and `"whole"`.
- **Created.** Run `inspect examples/timeline.json` at `-t 0`, `0.5` and `1`, as checks `inspect-created-<t>`. `.objects.orb.opacity` is 0, 0.5 and 1 over its fade, and `.objects.disc.opacity` stays 1.
- **Out of range.** `$SKILL/scripts/run "$RUN" inspect-out-of-range inspect examples/timeline.json -t 7`: exit 1, `.errors[0].code` `scene_time_out_of_range` at path `""`.
- **Before and after a step.** Inspect the same scene at each step's `start` and `end` from `validate`. Each value a step's animations change moves from its old value to its new one, and nothing else changes.
- **Plane map.** Write a shear scene: `printf '%s' '{"objects":{"ring":{"type":"circle","radius":1,"stroke":{"color":"#58c4dd"}},"tip":{"type":"dot","radius":0.08,"position":[1,0,0]},"title":{"type":"text","text":"A shear","position":[0,2.5,0]}},"timeline":[{"type":"play","duration":2,"animations":[{"type":"linear-map","objects":["ring","tip"],"matrix":[[1,1],[0,1]],"origin":[0,0]}]}]}' > "$RUN/shear.json"`. Run `$SKILL/scripts/run "$RUN" inspect-plane-map inspect "$RUN/shear.json" -t 1`. `jq -c '.objects.ring.plane_map.matrix, (.objects.title|has("plane_map"))'` prints `[[1.0,0.5],[0.0,1.0]]` (halfway under the default easing curve) and `false`. At `-t 2` the matrix is `[[1.0,1.0],[0.0,1.0]]` with `determinant` 1.0.
- **Ball radius.** Write `$RUN/hcl.json` as `check-layout.md`'s covered atoms recipe does, and run `$SKILL/scripts/run "$RUN" inspect-ball-radius inspect "$RUN/hcl.json" -t 1`. `jq -c '.objects.m.atoms[] | [.element, .ball_radius]'` prints about `["Cl",49.04]` and `["H",26.01]`. View `frame` at scene time 1: the chlorine's disc is about 98 pixels across. Set the scale to `[1, 1, 1]` as `inspect-ball-radius-half`: about `["Cl",23.04]` and `["H",13.71]`, not exact halves, since the scale also moves each atom nearer or farther from the camera. Mid-reaction, as at `-t 14` of `skills/sci-anim/examples/polyethylene.json`, each shown atom of `.reactions[]` has a `ball_radius` too. Add `"hydrogens": false` to `m` as `inspect-ball-radius-hidden`: the hydrogen, now hidden, has no `ball_radius`.
- **Chapter.** Run `$SKILL/scripts/run "$RUN" inspect-chapter inspect examples/narrated_swap.json -t 5`. `.chapter` is `swap`.
- **Stages.** Run `$SKILL/scripts/run "$RUN" inspect-stages stages examples/timeline.json --from 0.99 --to 1.02`. Exit 0. `jq -c '[.command, .fps, [.stages[].scene_time]]'` prints `["stages",30,[0.99,1.0,1.02]]`, since the scene runs at 30 frames a second, and `jq '.stages[1]'` matches the report of `inspect --scene-time 1` without its `status` and `command`. `--from 2 --to 1` exits 2 with a `usage` error.
- **Proof.** The reports at each scene time in `$RUN`, and for a visual claim the frame at the same scene time.

## Gotchas

- Object ids are the keys of `.objects`, sorted. `jq '.objects | keys'` lists them.
- `bounding_box.pixels` uses frame pixels from the top-left, y down. `world` uses world units, y up.
- A text or math object before its write shows `created` 0 and still has a bounding box. Use `created` and `opacity` to tell whether it shows, not the box.
