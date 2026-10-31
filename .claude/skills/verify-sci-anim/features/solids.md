# Draw solids

A scene file's `prism` pushes a `base` region (a `circle`, `polygon` or `path`) straight up by `height`, and a `revolution` turns a `profile` region around the object's y axis through `angle` degrees. Both show as lit solids with dark edges, and a `create` grows them: a prism rises and a revolution turns. A sphere is created the same way. Spheres, molecules and solids are lit by a key light from the camera's upper left in a few stepped tones. Flat shapes, text and math draw exactly as before.

## Sub-features

- `solids-draw` draws a box, cylinder, cone and a cut-open planet, each lit, with edges.
- `solids-create` raises a prism, turns a revolution, and turns a sphere into being over a create's span.
- `solids-fade` fades a solid with its edges.
- `solids-errors` refuses a profile left of the y axis, a base with no inside, a see-through material, and a type such as `box` with a hint naming the solid to write.
- `solids-flat-unchanged` draws flat shapes, text and math byte for byte as before the key light.

## How to get to it (user POV)

- Write an object with `"type": "prism"` or `"type": "revolution"` in a scene file, then run `validate`, `frame`, `inspect`, `render`, `frames` or `check-layout` on it.
- `examples/solids.json` raises a cylinder and a box, turns a cone and a cut-open planet, and moves the camera.
- `sci_anim schema | jq '.["$defs"].ObjectFile.allOf[].then | select(.properties.type.const == "prism")'` prints the fields.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.

- **Draw.** Run `$SKILL/scripts/run "$RUN" solids-draw-5 frame examples/solids.json --scene-time 5 -o "$RUN/solids-5.png"`. Exit 0. View the PNG. A blue cylinder, a red cone, a green box and a yellow sphere cut open to a red core, each lighter on its upper left, with thin dark edges.
- **Create.** Draw `--scene-time 1`, `2` and `3` into `$RUN`, and run `inspect` at 2. At 1 no solid shows. At 2 `.objects.cylinder.created` is 0.5 and the frame shows a cylinder half its height, standing on the same base. At 3 both are whole. `inspect` gives the whole solid's `bounding_box` even mid-create.
- **Fade.** Copy `examples/solids.json` into `$RUN`, give the box `"opacity": 0.5`, and draw it at 5. The box is half see-through and still shows its edges.
- **Errors.** Write a scene file into `$RUN` with a `revolution` whose profile is `{ "type": "circle", "radius": 1 }`, a `prism` on `[[0,0],[1,0],[2,0]]`, and a prism whose material has `"opacity": 0.5`. `validate` exits 1 with `out_of_range` at `/objects/<id>/profile`, `invalid` at `/objects/<id>/base/points` and `invalid` at `/objects/<id>/material/opacity`. In a separate file, an object of `"type": "box"` gives `unknown_value` at `/objects/<id>/type`, whose message names the `prism` to write. Schema errors come alone, so test them in their own file.
- **Flat shapes unchanged.** Build the parent commit's binary in a worktree, then draw `examples/text_and_math.json` at 1 with both binaries (`SCI_ANIM=<old binary>` for the run helper). The RGBA pixels match exactly. `examples/sphere_and_circle.json` at 0 differs only inside the sphere.
- **Video and layout.** `render examples/solids.json -o "$RUN/solids.mp4"` writes 660 frames per `ffprobe`. `check-layout examples/solids.json --out "$RUN/layout"` reports no problem at its 19 scene times.
- **Proof.** The PNGs, reports, the pixel comparison and a sentence per PNG in `$RUN`.

## Gotchas

- A base's y runs along -z, away from the default camera, so a base drawn upward on paper recedes into the scene.
- A revolution under 360 degrees opens toward the camera's front right. Turn the object about y to aim the opening.
- At 400 pixels tall an edge is about one pixel wide, so a pixel check for an edge needs a looser threshold than at 1080p.
- `check-layout` reads a text's contrast against a solid behind it in the solid's color and its key-lit shade, so white text on a white prism gives a fault for each lit tone it cannot be read against.
