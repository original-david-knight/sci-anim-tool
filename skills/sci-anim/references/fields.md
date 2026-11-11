# Fields: electric and magnetic fields drawn from their sources

Read `instructional-videos.md`, `motion.md` and `math.md` first. From sci_anim 0.29.0 a field is computed, not drawn by hand: you place the charges, wires, loops and magnets as ordinary objects, and a `vector-field` that names them as its field sources works out the field and draws it as field lines or as arrows at every scene time. Move a charge or turn a magnet with any animation, and the lines bend and the arrows swing on their own. `../examples/electric_dipole.json`, `bar_magnet.json`, `wire_arrows.json` and `faraday_magnet_loop.json` are whole lessons.

## The parts of a field

```json
"plus": { "type": "circle", "radius": 0.3, "position": [-2, 0, 0], "fill": { "color": "#c0392b" } },
"minus": { "type": "circle", "radius": 0.3, "position": [2, 0, 0], "fill": { "color": "#2e6db4" } },
"E": { "type": "vector-field", "of": "electric",
  "sources": [
    { "type": "charge", "object": "plus", "charge": 1 },
    { "type": "charge", "object": "minus", "charge": -1 }
  ],
  "draw": { "type": "lines", "width": 14, "height": 6.4, "per_charge": 12 },
  "stroke": { "color": "#ffff00", "width": 0.03 } }
```

- `of` is `electric` or `magnetic`, and sets which field sources the field takes: `charge` for an electric field, `wire`, `loop` and `magnet` for a magnetic one. A field source of the other field is `invalid` at its `type`. For both fields at once, write two vector fields.
- Each field source names an `object`, drawn however you like, which must be placed by its own transform (not attached, not a plot, not a ray). That object's position and rotation place the source at every scene time, so the field follows every `move`, `rotate`, `follow`, carry and simulation of it.
- The units take Coulomb's constant and μ0/4π as 1, so only the shape of the field and the arrows' relative strengths matter on screen: a charge q gives q/r², a wire of current I gives 2I/ρ, a loop of radius a gives 2πIa²/(a² + z²)^(3/2) on its axis, and a long magnet 4π times its `strength` inside.

| Field source | Where it stands | Its numbers |
|---|---|---|
| `charge` | at its object's position; field lines start and end on the edge of its object's drawing, a circle's radius, and no arrow is drawn inside it | `charge`, signed |
| `wire` | an infinitely long straight wire through its object's position, along `direction` in its object's own space, by default `[0, 0, 1]`: seen end on, its current toward the camera, so its field circles it counterclockwise | `current`, signed; `direction` |
| `loop` | a circle of `radius` in its object's own xy plane, its current counterclockwise seen from its own +z, so its field runs through it along +z. Turn its object `[0, 90, 0]` to see it side on, its axis along the world's x | `radius`, `current` |
| `magnet` | a bar `length` long along its object's own x, centered on its position, of `radius`, north toward +x. It is an ideal solenoid, so draw a solenoid or an electromagnet with it too | `length`, `radius`, `strength`: current per unit of length, default 1 |

- The vector field's own `position` and `rotation` place its rectangle and its plane. Its `scale` stays 1 (`invalid`); give it a bigger rectangle or grid instead.
- Field lines and arrows draw over objects at the same depth in any order. Give the vector field a `position` z of 0.01 to draw it over a magnet's fill, as `bar_magnet.json` does, so its lines show running through the body.

## Field lines

`"draw": { "type": "lines", "width": 14, "height": 6.4 }` traces field lines in the vector field's own xy plane, inside the rectangle `width` by `height` centered on its position. Each has a tip showing the field's direction.

- Lines follow the field's part in the vector field's plane. Set the field sources in that plane, or symmetric about it: charges in it, a wire square to it, a loop's or a magnet's axis in it. The field of anything out of the plane is drawn as its shadow in the plane, which is not a true field line. For another plane, add a second vector field turned about an axis, such as `"rotation": [90, 0, 0]` for the xz plane.
- An electric field starts `per_charge` lines (default 8) on each unit of positive charge, evenly around it from its own +x, so a charge of 2 starts twice as many. Each ends on a negative charge, on the rectangle's edge, or after running three times round the rectangle. A negative charge that fewer lines reach than its own share starts the rest itself, at its even angles farthest from those that reach it, so a dipole of +1 and -1 shows as many lines on each charge, symmetric above and below, and a lone negative charge shows lines coming in from the edge.
- A magnetic field starts `count` lines (default 6) on each loop or magnet, evenly across its middle along the line where its middle crosses the plane, and `count` out from each wire along the plane's +x. Each is traced both ways. Inside a magnet the lines run as evenly as its field there is strong; out of its north end they fan out, and the lines seeded near its wall close tightly round it while those near its axis close far outside it, so they often run off the rectangle edge to edge, as in a textbook drawing. A wire's lines are circles.
- `seeds`, a list of points `[x, y]` in the plane, starts the lines there instead, each traced both ways. Use them to pick exactly which loops of a magnet or circles around a wire to draw. `inspect` gives the field at each seed under `at_seeds`.
- A line's tip sits halfway along it when it starts on a charge or closes, and halfway along its forward half otherwise, so it never hides inside a magnet. `tip_length` sets its size (default 0.25).
- A magnet's lines are only as far apart inside it as its radius over `count`, so give a thin magnet few lines, or draw it thicker.
- `create` draws each line from its start along its length, all at once: out from a positive charge, in from the edge toward a lone negative charge, and round from its start for a magnetic line.

## Arrows

`"draw": { "type": "arrows", "layout": { "type": "grid", "counts": [15, 7, 1], "spacing": 0.9 } }` draws an arrow at every place of a grid in the vector field's own space, centered on its place, written as a pattern's `grid` is (`patterns.md`), at most 4000 places.

- In a grid one place deep, each arrow lies in the vector field's plane along the field's part in it. In a deeper grid, such as `[5, 5, 5]`, each points along the field in space and turns about itself to face the camera.
- No arrow is drawn inside a charge's or a wire's object, inside a magnet, or where the field is 0, so `arrow_count` may be less than the grid's places.
- `shows_strength` is `opacity` (the default), `length` or `none`. Each arrow's strength share is ½ + log₁₀(strength / median) / 2, clamped to 0 to 1, where the median is of every arrow's strength at that scene time: the median arrow is halfway, and arrows from a tenth of it to ten times it span the range, so a field that falls off as 1/r² still shows every arrow. Under `opacity` an arrow's opacity is 0.15 + 0.85 × share; under `length` it is 0.25 + 0.75 × share of `length` long.
- `length` is the full arrow's length (default 0.8 of `spacing`), and `tip_length` its tip's (default a quarter of `length`).
- `create` grows every arrow at once, shaft then tip.

## Animating a field

- Move, rotate or follow the field sources' objects, not the field. Turning a magnet turns its field, and moving a magnet through a loop drags its lines through it, as `faraday_magnet_loop.json` does.
- A field source's numbers are fixed. To change a current or a charge, cross-fade two vector fields, as `wire_arrows.json` does when a second wire joins.
- `create`, `fade` and `recolor` of `stroke` work on a vector field; its tips are filled in the stroke's color. A `morph` is `not_morphable`, a `scale` is `invalid`, and a linear map refuses it.
- `check-layout` judges a vector field by the text its lines or arrows run through, reported as "runs through", not by its box, and lets it run off the frame, as it does a ray. In a scene with a vector field it checks every frame time of every animation, so keep labels outside the rectangle, or on a charge's drawing, where no line goes.

## Reading it back

`inspect` reports under `vector_field`: `of`; for lines, `line_count` and each line's `from` and `to` in the world, `closed`, and `ends_on`, the object of the charge it ends on; for arrows, `arrow_count` and each arrow's `place` and `toward`, a unit direction, in the world, and the field's `strength` there; and `at_seeds` for lines started at `seeds`.

| Lesson | Write |
|---|---|
| Field of a point charge | one `charge` and `per_charge` lines: they run straight out at even angles |
| Electric dipole | +1 and -1, `per_charge` 12, then `move` one charge and watch the lines bend |
| Two like charges | two +1 charges: every line runs to the edge, and none crosses the middle |
| Bar magnet | a `magnet` whose object is the bar's outline, `count` 8, the vector field at z 0.01 |
| Solenoid | a `magnet` as long and as wide as the coil drawn; its field is uniform inside |
| A current loop seen side on | a `loop` whose object is turned `[0, 90, 0]`, with dots where its wire crosses the plane |
| Wire seen end on | a `wire` and `arrows` in a flat grid; a second wire of opposite current in a second vector field faded in |
| Faraday's law | a `magnet` moved through a loop drawn as an ellipse, with the induced current's direction shown on the loop |
| A charge's field in space | `arrows` in a `[5, 5, 5]` grid under a turned camera |

## Limits

- The fields are static: each scene time draws the field of the sources where they stand then. No induced field, no radiation, no field inside matter.
- Field lines lie in the vector field's plane. Only arrows show a field in space.
- A field line's spacing shows how the field runs, not how strong it is, except around charges, whose lines are shared out by charge.
