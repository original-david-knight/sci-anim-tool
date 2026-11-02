# Draw mirror images of molecules

A molecule with `"mirror": "x"`, `"y"` or `"z"` draws its structure file's atoms reflected through the plane across its centroid at right angles to that axis of its own space, before its `rotation`. Atom numbers, bonds, groups, highlights, captions and `inspect` are as for the file. A reaction from a molecule to its mirror image with the identity atom map validates, and with `"fit": false` every atom lies in the mirror plane halfway. Molecules without `mirror` draw as before. Needs sci_anim 0.25.0 or later.

## Sub-features

- `mirror-inspect` reports each atom of a mirrored molecule at its file position minus the centroid, with the mirrored coordinate negated, for each of `x`, `y` and `z`.
- `mirror-pair` draws L-alanine and its mirror image side by side as each other's reflection across the frame's middle column, with turns that mirror each other.
- `mirror-groups` lights an atom group and attaches a caption to it on a mirrored molecule, by the file's atom numbers.
- `mirror-reaction` validates a reaction from a molecule to its mirror image, and halfway reports every atom in the plane.
- `mirror-error` refuses an unknown plane with `unknown_value` at `/objects/<id>/mirror`.
- `mirror-unchanged` a scene with no `mirror` draws the same frame, byte for byte, as the build before the change.

## How to get to it (user POV)

- Give a `molecule` a `mirror`, as `skills/sci-anim/examples/mirror_images.json` does for its right-hand alanine. `sci_anim validate` checks it, `inspect` reports its atoms, `frame` and `render` draw it, and `check-layout` checks the scene.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. `skills/sci-anim/examples/mirror_images.json` runs 28.3 s: L-alanine fades in by 2.7, the glass pane by 5.2 and the mirror image by 7.2; the mirror image turns over from 11.3 to 13.8, slides up beside the first by 15.8, and both methyls light from 16.6 to 17.8; it goes back by 24.1, and both stereocenters light with S and R captions by 25.3.
- Variants that read `alanine.sdf` sit beside a copy of the structures: `mkdir -p "$RUN/mirror" && cp -r skills/sci-anim/examples/structures "$RUN/mirror/"`.

- **Inspect.** Write `$RUN/mirror/plain.json`, one molecule `m` of `structures/alanine.sdf` at the origin and a one-second `wait`, and `$RUN/mirror/x.json`, `y.json` and `z.json`, the same with `"mirror"` set to that axis. `$SKILL/scripts/run "$RUN" mirror-inspect-plain inspect "$RUN/mirror/plain.json" -t 0`, then `mirror-inspect-x`, `-y` and `-z` the same. Each atom of `x` is at its `plain` position with x negated, and likewise for `y` and `z`, within 1e-3: `jq -s '[.[0].objects.m.atoms, .[1].objects.m.atoms] | transpose | map(.[0].position[0] + .[1].position[0] | fabs) | max' "$RUN/mirror-inspect-plain.stdout.json" "$RUN/mirror-inspect-x.stdout.json"` prints a number below `0.001`, and the y and z differences are 0. Elements and numbers are in the same order in all four.
- **Pair.** `$SKILL/scripts/run "$RUN" mirror-pair frame skills/sci-anim/examples/mirror_images.json -t 8 -o "$RUN/mirror-pair.png"` and view it: two alanines either side of a pale vertical pane, each the other's reflection, carboxyl outward, amino groups toward the pane, methyls down.
- **Groups.** `frame` at `-t 19` into `$RUN/mirror-methyls.png`: the turned mirror image beside the first with its methyl up and lit purple, the first's down and lit yellow. `frame` at `-t 27` into `$RUN/mirror-stereocenters.png`: both middle carbons lit yellow, S above the left and R above the right.
- **Layout.** `$SKILL/scripts/run "$RUN" mirror-layout check-layout skills/sci-anim/examples/mirror_images.json --out "$RUN/work/mirror-check"`: exit `0`, `No problem at any of 31 checked scene times.`
- **Reaction.** Write `$RUN/mirror/pass.json` with molecules `l` and `d` of `structures/alanine.sdf` at the origin, `d` with `"mirror": "x"`, and a one-second `reaction` from `l` to `d` with `"fit": false`, `"easing_curve": "linear"` and the 13 pairs of the identity atom map. `validate` exits `0`. `inspect -t 0.5`: `jq '[.reactions[0].atoms[].position[0] | fabs] | max'` is below `1e-4`. A `frame` at `-t 0.5` shows the balls flattened onto one vertical line.
- **Error.** A copy with `"mirror": "w"`: `validate` exits `1` with `.code` `unknown_value` at `.path` `/objects/right/mirror`.
- **Unchanged.** For `skills/sci-anim/examples/hydration.json`, draw a frame at `-t 16.5` with this build and with the build before the change (`SCI_ANIM=$(command -v sci_anim)` when that is the installed one), and `cmp` the PNGs. They are identical.
- **Proof.** The reports, frames, variant scenes and `cmp` output in `$RUN`.

## Gotchas

- Equal rotations do not make a mirror pair. The mirror image of a molecule at `rotation` `[a, b, c]` with `"mirror": "x"` takes `[a, -b, -c]`.
- The plane is in the molecule's own space. A molecule turned a quarter turn about y with `"mirror": "x"` looks mirrored front to back in the frame.
- `tests/mirror.rs` checks the pair frame by its outline, not against a stored PNG, since lavapipe and Metal shade differently.
