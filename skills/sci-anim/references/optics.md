# Optics: lenses, mirrors, refraction and ray diagrams

Read `instructional-videos.md`, `motion.md` and `math.md` first. From sci_anim 0.26.0 a ray diagram is computed, not drawn: you place a `lens`, a `mirror` or a piece of glass, an object's arrow, and `ray` and `image` objects that name them, and sci_anim traces every ray and finds every image at every scene time. Move the object or the lens, and the rays swing and the image grows, shrinks, flips and turns virtual on its own. `../examples/converging_lens.json`, `diverging_lens.json`, `concave_mirror.json`, `refraction.json` and `telescope.json` are whole lessons.

## The parts of a ray diagram

```json
"axis": { "type": "line", "from": [-7, 0], "to": [7, 0], "stroke": { "color": "#666666", "width": 0.02 } },
"lens": { "type": "lens", "focal_length": 2, "height": 5, "fill": { "color": "#58c4dd", "opacity": 0.25 } },
"foot": { "type": "dot", "radius": 0.05, "position": [-5, 0, 0] },
"candle": { "type": "arrow", "from": { "object": "foot" }, "to": { "object": "foot", "offset": [0, 0.8, 0] } },
"parallel": { "type": "ray", "from": { "object": "foot", "offset": [0, 0.8, 0] }, "principal": "parallel", "optics": ["lens"], "length": 5, "back_trace": true },
"image": { "type": "image", "from": { "object": "foot" }, "to": { "object": "foot", "offset": [0, 0.8, 0] }, "optics": ["lens"] }
```

- A `lens` is a thin lens across its own y axis, with its optical axis along its own x. A positive `focal_length` converges and draws biconvex, a negative one diverges and draws biconcave. It ticks the axis at each focal point; label them with math `F` and `F'` attached to the lens at an `offset` of the focal length.
- A `mirror` reflects on its own -x side, so light comes from the left unless you turn it. No `focal_length` is a flat mirror, a positive one concave, a negative one convex. A curved mirror ticks its focal point and its center of curvature, two focal lengths out.
- Stand the object on a small `dot`, its foot, and draw it as an arrow with both ends on the dot. Every ray and the image read the same ends, so one `move` of the dot moves them all.
- A `ray` leaves `from` one of three ways. `principal` is a principal ray of the first lens or curved mirror in `optics`: `parallel` along the axis, `focal` through the focal point on the object's side (toward the far one for a diverging lens), and `center` through a lens's middle or toward a mirror's center of curvature. `through` is any point or end it passes through. `angle` is a direction in degrees from +x, as parallel rays from a distant star need.
- A ray bends at a lens's middle, as the thin-lens model draws it, reflects where it meets a mirror's arc, and runs `length` world units past its last bend. Give each ray a `length` that ends inside the frame, or `check-layout` reports it cut off.
- `back_trace: true` adds a dashed line from where the ray leaves the lens or mirror back to the virtual image it seems to come from. It draws nothing while the image is real, so leave it on every ray of a lesson whose object crosses a focal point.
- An `image` draws the arrow's image through the lenses and mirrors of `optics`, in the order the light meets them: solid while real, dashed while virtual, upside down when inverted. It hides while the image is at infinity, at the instant the object stands on a focal point.
- A ray or an image has no `position` (`placed_by_light`). Create a ray to draw it from its source along its path: three creates one after another build the diagram ray by ray. Fade an image in once its rays meet.
- `inspect` gives each ray's `points` and the `image` its source has at the last lens or mirror, with `virtual_image`, and each image's `from`, `to`, `magnification` and `real`. Use them to place labels such as "real, inverted" beside the image.

| Lesson | Write |
|---|---|
| A converging lens, object beyond 2F, between F and 2F, inside F | one lens, `parallel`, `center` and `focal` rays with `back_trace`, an image, and a `move` of the foot across the focal point |
| A diverging lens | `"focal_length": -2.5`, the same three rays with `back_trace` on `parallel` and `focal` |
| A magnifying glass | a converging lens with the foot inside F: the image is virtual, upright and larger |
| A concave mirror | `"type": "mirror"` with a positive focal length, the object on its -x side, rays `parallel`, `focal` and `center` |
| A convex mirror | a negative focal length; every image is virtual, behind the mirror |
| A plane mirror | no `focal_length`; aim rays with `through`, and the image stands as far behind |
| A refracting telescope | two lenses whose focal points meet, both in `optics` in order, and parallel rays by `angle` |

## Refraction: glass and water

A `polygon` or a `circle` with a `refractive_index` is glass or water to every ray that names it. The ray refracts at each edge it crosses by Snell's law, and past the critical angle it reflects whole and stays inside:

```json
"glass": { "type": "polygon", "points": [[-7.2, -4.2], [7.2, -4.2], [7.2, 0], [-7.2, 0]], "refractive_index": 1.5, "fill": { "color": "#58c4dd", "opacity": 0.18 } },
"emitter": { "type": "dot", "radius": 0.1, "position": [-5.2, 2.6, 0] },
"into_glass": { "type": "ray", "from": { "object": "emitter" }, "through": [-3, 0], "optics": ["glass"], "length": 3 }
```

- 1.33 is water, 1.5 glass, 2.42 diamond. Air outside is 1. A ray that starts inside the region starts at its index.
- A flat interface is a wide rectangle whose top edge is the surface. A plate shifts a ray without turning it, a triangle of glass bends it twice toward its base, and a circle is a drop of water.
- To sweep the angle of incidence past the critical angle, aim the ray `through` a small dot on the surface and `move` the dot: the ray refracts out, grazes the surface, then reflects whole.
- A ray meets at most 64 edges, lenses and mirrors, so a ray trapped in glass ends.
- An `image` is formed by lenses and mirrors only; a medium in its `optics` is `invalid`.

## Limits

- A lens bends rays at its middle plane, as every ray diagram does, so the thin lens equation holds exactly. For a thick lens, whose rays bend at both faces, write a `polygon` with a `refractive_index` through many points along its two arcs.
- A curved mirror reflects each ray toward the image the mirror equation gives, from where it meets the drawn arc, so rays from a point meet exactly at its image, as in a textbook. Spherical aberration is not shown.
- Every ray lies in the world's xy plane, at the height of its source.
