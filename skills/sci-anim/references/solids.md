# Solids: prisms, revolutions, tubes, surfaces, spheres and 3D axes

Read `instructional-videos.md` and `motion.md` first.

A solid has depth. Use one when the lesson is about a thing in space: a volume, a cross section, a planet's layers, a lens. A diagram that lies flat on the page stays a flat shape.

## Make a solid from a flat shape

A `sphere` needs only a `radius`, and stands about its `position`: `{ "type": "sphere", "radius": 1, "material": { "type": "standard", "color": "#58c4dd" } }`. It can fade, move, scale, rotate and be created, but not morphed. Every other solid is a region given depth. The region is written as a `circle`, `polygon` or `path`, with no stroke or fill, in world units from the object's position.

- A `prism` pushes its `base` straight up by `height`. The base lies flat on the floor: its x is the world's x, and its y runs away from the camera, along -z. The prism stands on the object's `position` and rises along +y.
- A `revolution` turns its `profile` around the object's y axis through `angle` degrees, 360 by default. Draw the profile as the right half of the solid's outline seen from the front, with x of 0 or more. `validate` reports a profile that crosses to negative x.

| Solid | Write |
|---|---|
| Box | `prism` on a square `polygon` |
| Cylinder | `prism` on a `circle`, or `revolution` of a rectangle `[[0, 0], [r, 0], [r, h], [0, h]]` |
| Cone | `revolution` of `[[0, 0], [r, 0], [0, h]]` |
| Torus | `revolution` of a `circle` whose `center` is `[R, 0]` |
| Pipe | `prism` on a `path` of two circles with `"rule": "even-odd"` |
| Planet with layers | two `revolution`s at one position: a shell from a half-disc `path` with a hole, and a core from a half disc, both with `angle` 270 |

A `revolution` under 360 degrees is cut open and closed by its profile at both cut faces. It turns from +x toward -z, away from the camera, so the opening faces the camera at the front right. Turn the object about y with `rotation` to aim the opening.

## Carry a circle along a space curve

A `tube` carries a circle of `radius` along a space curve, square to the curve, so a spring, a coil, an orbit, a field line or a strand of DNA is one object. Its `curve` writes x, y and z as expressions in `u`, in world units from the object's position, over `u_range`, low first:

| Curve | `x` | `y` | `z` | `u_range` |
|---|---|---|---|---|
| Spring, 5 turns, 2.5 tall | `0.8*cos(u)` | `0.08*u` | `0.8*sin(u)` | `[0, 31.4159]` |
| Circle in the floor | `2*cos(u)` | `0` | `2*sin(u)` | `[0, 6.283185]` |
| Ellipse orbit, sun at a focus | `2*cos(u) - 1.2` | `0` | `1.6*sin(u)` | `[0, 6.283185]` |
| Trefoil knot | `sin(u) + 2*sin(2*u)` | `cos(u) - 2*cos(2*u)` | `-sin(3*u)` | `[0, 6.283185]` |
| Straight rod from a to b | `u` | `0` | `0` | `[a, b]` |

- A curve that ends where it starts closes on itself with no ends. Write a full turn as `6.283185`, or at least `6.2832`: `6.28` leaves a gap and two end discs.
- Keep `radius` under the radius of the tightest bend, or the tube folds into itself there. A wire is 0.05 to 0.1, a pipe 0.2 or more.
- Every expression needs a value at every u of `u_range`: `validate` reports `out_of_range` at the expression with the u where it has none.
- The expressions take the functions a function plot's do, with `atan2`, `mod` and `step`.
- A `scale` of the object squashes the tube's circle too. A spring between two objects is a `spring`, not a tube: it stretches and squashes as its ends move and keeps its turns and its round wire (`physics.md`).

### A curve through points

A curve no formula gives, such as a random coil, a tangled polymer chain or a backbone from real coordinates, is written as the curve points it passes through, with sci_anim 0.12.0 or later. Write them with a scene generator, never by hand: `../examples/polymer_coil.go` walks a seeded random chain of 300 curve points that never comes back near itself and draws it as a tube with an arrowhead riding it.

```json
"chain": { "type": "tube", "radius": 0.06,
           "curve": { "points": [[-4, -0.3, 0], [-3.73, -0.3, 0], [-3.6, -0.15, 0.2], [-3.4, 0.1, 0.3]] },
           "material": { "type": "standard", "color": "#58c4dd" } }
```

- Each curve point is `[x, y, z]` in world units from the object's position, or in data units on `axes`. Give 2 to 10000, each coordinate within 10000. `validate` reports a point with two numbers as `wrong_length` at its own path, such as `/objects/chain/curve/points/1`, and a coordinate too far as `out_of_range` at it. JSON has no NaN, so a generator that writes one makes a file that is not JSON (`invalid_json`): check your numbers before you write them.
- The curve passes through every point and is smooth between them (centripetal Catmull-Rom), so it never loops or overshoots between two points however unevenly they are spaced. A bend sharper than the tube is thick folds the tube into itself there, so keep neighboring points a few radii apart.
- `u` runs from 0 at the first point to 1 at the last: the k-th of n points, counted from 0, is at u = k/(n - 1).
- A point equal to the one before it adds nothing. A last point back on the first, with at least two between, closes the curve smoothly, as a ring polymer, with no ends.
- An open curve leaves its first point straight toward the second. A carry with `faces: tangent` starts facing that chord, so a generator whose first two points differ only in x starts the carried object unturned, at `rotation` `[0, 0, 0]`.
- A curve through points does not read `t`, so it is not a shape law. Everything that takes a space curve takes one: a tube, a carry along the tube, and a pattern's `along` layout with its `faces`.

### A curve through the atoms of a PDB file

An atom trace reads its curve points from a PDB file, one atom of each amino acid or nucleotide by its atom name, with sci_anim 0.19.0 or later. A protein's chain is the trace through its alpha carbons, `CA`, and a chain of DNA the trace through its phosphorus atoms, `P`:

```json
"fold": { "type": "tube", "radius": 1.2, "scale": [0.1, 0.1, 0.1],
          "curve": { "trace": { "file": "structures/1ubq.pdb", "atom_name": "CA" } },
          "material": { "type": "standard", "color": "#58c4dd" } }
```

- Each curve point is where a `molecule` of the same file draws that atom, in angstroms from the molecule's centroid. So give the tube the molecule's `scale`, about 0.1, and write its `radius` in angstroms: 1 to 1.5 reads as a chain. A tube with the molecule's `position`, `rotation` and `scale` runs through the molecule's atoms. Two alpha carbons in a row lie 3.8 Å apart.
- `file` is relative to the scene file, as a molecule's is, and must end in `.pdb` or `.ent` (`invalid` at `curve/trace/file` otherwise). Ions and ligands add nothing, so a calcium ion, whose atom name is also `CA`, stays off the curve. A residue without the atom adds no curve point.
- `chain`, such as `"A"`, keeps one chain. Leave it out for every chain in file order, which joins the end of one chain to the start of the next with a straight span. A chain the file does not have is `invalid` at `curve/trace/chain`, and an atom name no residue has is `invalid` at `curve/trace/atom_name`.
- Everything that takes curve points takes a trace: `u` runs from 0 at the first atom to 1 at the last, a carry rides it, and a morph pairs its curve points.

### Fold a chain: morph one tube into another

A tube whose curve is `points` or a `trace` morphs into another such tube through as many curve points, with sci_anim 0.19.0 or later. Curve point k moves in a straight line to the target's curve point k, and the curve is made again through the moving points at every frame, so it stays smooth. Its radius, color and place move to the target's too. `../examples/ubiquitin_fold.go` writes `ubiquitin_fold.json`, which folds a zig-zag of 76 curve points into ubiquitin's atom trace while the camera pulls in, then shows the molecule beside it with its α-helix lit:

```json
{ "type": "morph", "object": "chain", "into": "fold" }
```

- Give both tubes as many curve points: `validate` reports `wrong_length` at the morph's `into` with both counts. Write the straight chain with a scene generator, one curve point per residue, 3.8 Å apart, as `ubiquitin_fold.go` does.
- Write both in the same units at the same `scale`, so the straight chain is as long as the real one. 76 amino acids stretch to about 285 Å, 28.5 world units at scale 0.1: start the camera far enough back to see it whole, about `[0, 0, 21]`, and `camera-move` it in during the morph.
- Each curve point travels straight, so chains pass through themselves on the way. A morph shows the fold's start and end shapes, not the way a protein folds. Say so if the lesson is about how it folds.
- A tube morphs only into a tube and only from one (`not_morphable` at the other end). A tube of expressions in `u` and a tube on 3D axes are not morphed (`not_morphable`): write the curve as `points` instead.

## Draw a height over a rectangle

A `surface` is a sheet whose height `z` is an expression in `x` and `y`, over the rectangle `x_range` by `y_range`, as a textbook writes z = f(x, y), in world units from its `position`. The rectangle lies on the object's floor as a prism's base does: x to the right and y away from the camera. The height rises straight up, so `z` is vertical on the frame. A potential well, a wave's height, a saddle and any function of two variables is one:

```json
"well": { "type": "surface", "z": "-2*exp(-(x^2 + y^2)/2)", "x_range": [-3, 3], "y_range": [-3, 3],
          "material": { "type": "standard", "color": "#58c4dd" } }
```

| Sheet | `z` |
|---|---|
| Potential well | `-2*exp(-(x^2 + y^2)/2)` |
| Standing wave | `0.4*sin(2*x)*cos(2*y)` |
| Saddle | `0.3*(x^2 - y^2)` |
| Ripple from a point | `0.3*cos(3*sqrt(x^2 + y^2))` |

- Each range is low first. `z` needs a finite value at every point of the rectangle: `validate` reports `out_of_range` at `z` with the x and y where it has none, so `sqrt(x)` needs an `x_range` from 0.
- Write the height as `z`, not `y`: `validate` reports `unknown_field` at `y` and says so.
- It is sampled finer where it bends, so a narrow well keeps its bottom. Keep the heights within a few units of the rectangle's size, or the sheet towers over the frame.
- It is lit on both sides, so the camera may look from above or below. A camera above and to the side, such as `[5, 5, 10]`, shows the heights best; straight from the front, a sheet is a thin line.
- A surface writes only a height. A sheet that folds back over itself, such as a sphere's or an orbital's, cannot be drawn yet.
- Write the function beside it as `math`, such as `z = -2e^{-(x^2 + y^2)/2}`, `attach`ed to the surface or its axes so it faces the camera.
- To graph it on axes, name `3d-axes` in `axes`, as below.

## Draw 3D axes

`3d-axes` draws three axes with ticks, tick labels and axis labels, z up as a textbook draws them: x to the right, z up and y away from the camera. Each of `x`, `y` and `z` gives a `range` in data units, a `length` in world units that the range fills, an optional `spacing` of its ticks and an optional `label` in LaTeX math. The box of the three lengths is centered on the object's `position`.

```json
"axes": { "type": "3d-axes",
          "x": { "range": [-3, 3], "length": 6, "label": "x" },
          "y": { "range": [-3, 3], "length": 6, "label": "y" },
          "z": { "range": [0, 8], "length": 4, "label": "z" } }
```

- Each axis crosses the others at 0, or at the nearest end of its range. The tick there has no label, so a range from 0 shows no 0.
- Tick labels sit below the x and y axes, or above an axis that runs over the middle of the box, and left of the z axis on the frame from wherever the camera stands. An axis label sits past the high end of its axis. Every label faces the camera at every frame, and an object in front of a label hides it.
- `stroke`, `color` and `label_size` set the lines and labels as for `axes`. The lines keep the stroke's width from every side.
- A camera above and to the side shows all three axes, such as `[7, 6, 11]` looking at the middle of the box. From straight in front, the y axis points at the camera.
- `create` draws the lines and ticks along their length, then fades the labels in. Move, scale, rotate and fade them as any object. They cannot be morphed, recolored or named by a linear map.
- A `surface` or a `tube` that names the axes in `axes` is written in their data coordinates and goes with them. It has no `position`, `rotation` or `scale` of its own: `validate` reports `placed_by_axes` for one, and for a move, scale, rotate or follow of it. Move the axes instead.
- A surface on 3D axes takes the axes' x and y ranges unless it gives its own, which are cut to the axes'. It is cut where it leaves the z range, so `x^2 + y^2` on a z range from 0 to 8 shows a round bowl cut at 8, and a pole such as `1/(x^2 + y^2)` stops at the top of the box instead of being refused. A create rises it from data z of 0, or the nearest end of the z range.
- A tube on 3D axes writes its curve in data units, and is cut into pieces where the curve leaves the box, each closed by a disc. Its `radius` is in world units.
- A `sphere` that names the axes in `axes` is a data point: its `position`, and every `move`, `follow` and carry of it, is in their data coordinates, so a follow whose law writes x, y and z = f(x, y) walks it across a surface on the axes, as gradient descent does. The axes carry it wherever they go, and its `radius` stays in world units, so it stays round however the axes stretch. Lift it by its radius over the sheet, or it sinks half in. `inspect` reports its world `position` and box, and `data_point.position` in data coordinates. A caption `attach`ed to it sits where the frame shows it.
- A plot that names 3D axes, and a surface, tube or sphere that names 2D `axes`, is `invalid` at `axes`. The data point on 2D axes is a `dot`, which `math.md` covers.
- To see a point inside a bowl, raise the camera above the bowl's rim: a camera lower than the slope of the bowl's wall sees only its outside.

```json
"bowl": { "type": "surface", "axes": "axes", "z": "x^2 + y^2", "material": { "type": "standard", "color": "#58c4dd" } }
```

- `inspect` reports `axes_3d` with `zero`, the world point of data (0, 0, 0), and `x_unit`, `y_unit` and `z_unit`, the world vector one data unit adds, so data (x, y, z) lands at `zero + x*x_unit + y*y_unit + z*z_unit`. Each tick has its world point and pixel, and its `label_text`, `label_side` and `label_bounding_box` as the frame shows it at that scene time. `check-layout` checks text against the lines, ticks and labels, not the box.

## Look

A solid is lit by a key light from the camera's upper left, so a face toward the light shows exactly its `material` color and the far side is darker. The shade steps in a few tones, as an illustration does. Prisms, revolutions, tubes and surfaces are drawn with dark edges where faces meet at a sharp angle and along the outline, and a surface along its rim. A dark solid on the default background loses its outline: set `"edges": "light"`.

A solid is opaque. Its material's `opacity` must be 1. To fade a solid, fade the object's `opacity`.

## Animate

- `create` grows a solid over its span: a prism rises from its base, a revolution or a sphere turns from 0 to its angle, a tube is drawn along its curve by length from the low end of `u_range`, and a surface rises from flat to its heights. 1.5 to 2 s reads well, and `linear` draws a tube at one speed along it.
- `fade`, `move`, `scale`, `rotate` and a `recolor` of `color` work as on any object. A camera move shows depth best: move the camera a few units sideways and up.
- To circle the camera around a solid, `camera-move` it to the circle's start, then give it a `follow` of `camera-position` with a law, which keeps it looking at its target: `{ "type": "follow", "sets": "camera-position", "law": { "x": "9*sin(tau*t/6)", "y": "3", "z": "9*cos(tau*t/6)" } }` circles once in 6 s from `[0, 3, 9]`. A title that should stay on the frame while the camera circles is `attach`ed to the solid, so it faces the camera. A solid that looks the same from every side, such as a whole cone, shows no turn: cut it open with `angle` under 360.
- A planet's orbit is a `follow` of its `position` with a law, such as `"x": "3*cos(tau*t/8)", "z": "-3*sin(tau*t/8)"`, and a moon's is a law that adds its planet's. Draw the orbit itself as a thin `tube` along the same curve.
- To run a bead along a tube, carry it with a `follow` whose `along` is the tube, and `"faces": "tangent"` to turn its own x axis along the curve. `validate` gives the bead's start `position` and `rotation` to declare in `carry_jump`.
- A solid cannot morph, except a tube through curve points into another (above), and a linear map cannot name one. `validate` reports both.
- A crystal lattice, a row of identical solids or beads along a curve is one `pattern`, which `patterns.md` covers.

## Layout

`inspect` reports a solid's bounding box, the whole solid even while a create is growing it, and a tube's `tube.start` and `tube.end`, the world points and pixels where its curve begins and ends. A tube through curve points also reports `tube.curve_points`, the world point of each, mixed while a morph plays. A surface's box spans its lowest and highest heights over its whole rectangle. A box seen from the side is larger on the frame than its base suggests, so check with `check-layout`, especially after a camera move, which brings near solids closer to text below them.
