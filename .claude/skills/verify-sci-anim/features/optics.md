# Trace rays and images

A `lens` (a thin lens of `focal_length`), a `mirror` (flat, concave or convex), and a `polygon` or `circle` with a `refractive_index` are optical elements. A `ray` names them in `optics` and is traced through them at every scene time from `from`, aimed by `principal`, `through` or `angle`: it bends at a lens toward its source's image by the thin lens equation, reflects off a mirror toward the image the mirror equation gives, refracts at each edge of a medium by Snell's law and reflects whole past the critical angle. `back_trace` dashes it back to a virtual image. An `image` draws the image of the arrow from `from` to `to` through lenses and mirrors, solid when real and dashed when virtual.

## Sub-features

- `optics-converging` draws a converging lens's principal rays, which meet at the image the thin lens equation gives, then moves the object inside the focal length.
- `optics-diverging` draws a diverging lens's rays and its virtual image.
- `optics-mirror` draws a concave mirror's rays and image, real then virtual.
- `optics-refraction` bends a ray into glass by Snell's law, then sweeps a ray inside the glass past the critical angle.
- `optics-telescope` passes parallel rays through two lenses.
- `optics-errors` refuses a ray with no aim or two, an unknown element, one named twice, one that does nothing to light, a principal ray with no lens or curved mirror, a medium in an image, a ray with a `position`, and a move of a ray.
- `optics-layout` runs `check-layout` on each lesson.

## How to get to it (user POV)

- Write a `lens`, `mirror` or `polygon` with a `refractive_index`, then `ray` and `image` objects that name it in `optics`, and run `validate`, `inspect`, `frame`, `render` or `check-layout`.
- The lessons `skills/sci-anim/examples/converging_lens.json`, `diverging_lens.json`, `concave_mirror.json`, `refraction.json` and `telescope.json`.
- `sci_anim schema | jq '.["$defs"].ObjectFile'` and search for `"ray"`, `"image"`, `"lens"` and `"mirror"`.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok` for the frames.

- **Converging.** `inspect skills/sci-anim/examples/converging_lens.json -t 7.5`: `.objects.image.image` has `to` `[3.333, -0.533]`, `magnification` -0.667 and `real` true: 1/5 + 1/di = 1/2. Each of `.objects.parallel.ray`, `center` and `focal` passes through that tip. `-t 13`: the foot at -1.2, the image `to` `[-3, 2]`, `magnification` 2.5, `real` false, and each ray's `virtual_image` true. Draw frames at 3.75, 7.5 and 13 and view them: the parallel ray half drawn; three rays meeting at an inverted image; three rays spreading from the lens with dashed lines back to a larger upright dashed image.
- **Diverging.** `inspect skills/sci-anim/examples/diverging_lens.json -t 7.5`: the image `to` `[-1.667, 0.4]`, `magnification` 0.333, `real` false. Draw the frame at 7.5: the parallel ray spreads as if from F, dashed back to the small upright image.
- **Mirror.** `inspect skills/sci-anim/examples/concave_mirror.json -t 7.5`: the image `to` `[0.2, -0.4]`, `real` true (do 7, f 2); the center ray's points go out to the mirror and back through C at `[-1, 0]`. `-t 13.5`: the image at x 6, behind the mirror, `magnification` 2.5, `real` false. Draw both and view them.
- **Refraction.** `inspect skills/sci-anim/examples/refraction.json -t 4`: `.objects.into_glass.ray.points` bends at `[-3, 0]` from 40.2° to 25.5° off the normal, sin 40.2° = 1.5 sin 25.5°. `-t 8`, `-t 10` and `-t 13`: `.objects.from_glass.ray.points[2]` above the surface while the aim's incidence is under 41.8°, and below it, reflected, at 13. Draw frames at 4, 10 and 13.
- **Telescope.** `inspect skills/sci-anim/examples/telescope.json -t 6`: the middle ray's last segment climbs 4 times as steeply as its first falls, tan 3°. Draw the frame at 6.
- **Errors.** `cargo test --release --test optics` holds each refusal; in `$RUN`, a ray with neither `principal`, `through` nor `angle` gives `missing_field` at `/objects/<id>`, two of them `invalid`, an element that is a plain circle `invalid` at `.../optics/0`, an unknown one `unknown_object`, one named twice `duplicate_object`, `principal` with only glass `invalid` at `.../principal`, a glass in an image's `optics` `invalid`, a ray with a `position` `placed_by_light` at `.../position`, and a move of a ray `placed_by_light` at `.../object`. Each exits 1.
- **Layout.** `check-layout` each lesson with `--out "$RUN/layout-<name>"`: exit 0.
- **Proof.** The reports, frames and a sentence per frame in `$RUN`.

## Gotchas

- A ray that misses a lens's `height` passes it unbent. Make the lens tall enough for every principal ray of the object's whole move.
- `check-layout` judges a ray by the text its stroke runs through, reported as "runs through", not by its box, and lets it run off the frame.
- An image hides while the object stands on a focal point, where it lies at infinity.
