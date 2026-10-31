# Scene file reference

A scene file is the JSON file a scene is written in: its scene config, its camera, its objects and its timeline. This page lists every field a scene file takes. `scripts/scene-reference` writes it from the JSON Schema that `sci_anim schema` prints, so change a field's description in `src/`, where the schema comes from, and run the script, rather than editing this page. [How scene files work](scene-files.md) explains how the parts fit together.

## Contents

- [Top level](#top-level)
- [Scene config](#scene-config)
- [Camera](#camera)
- [Steps](#steps): [`wait`](#wait), [`chapter`](#chapter), [`play`](#play)
- [Objects](#objects): [`circle`](#circle), [`dot`](#dot), [`sphere`](#sphere), [`prism`](#prism), [`revolution`](#revolution), [`tube`](#tube), [`surface`](#surface), [`line`](#line), [`arrow`](#arrow), [`spring`](#spring), [`trail`](#trail), [`readout`](#readout), [`polygon`](#polygon), [`arc`](#arc), [`path`](#path), [`text`](#text), [`math`](#math), [`table`](#table), [`axes`](#axes), [`3d-axes`](#3d-axes), [`number-line`](#number-line), [`function-plot`](#function-plot), [`point-plot`](#point-plot), [`bar-plot`](#bar-plot), [`molecule`](#molecule), [`pattern`](#pattern)
- [Animations](#animations): [`fade`](#fade), [`count`](#count), [`move`](#move), [`scale`](#scale), [`rotate`](#rotate), [`create`](#create), [`write`](#write), [`reaction`](#reaction), [`morph`](#morph), [`highlight`](#highlight), [`recolor`](#recolor), [`camera-move`](#camera-move), [`camera-look`](#camera-look), [`camera-zoom`](#camera-zoom), [`linear-map`](#linear-map), [`simulate`](#simulate), [`follow`](#follow)
- [Shared types](#shared-types): [Align](#align), [Atom pair](#atom-pair), [Atom ref](#atom-ref), [Atom trace](#atom-trace), [Attach](#attach), [Axis](#axis), [Axis 3d](#axis-3d), [Category axis](#category-axis), [Cell](#cell), [Copy](#copy), [Copy position](#copy-position), [Copy rotation](#copy-rotation), [Copy scale](#copy-scale), [Curve coordinate](#curve-coordinate), [Curve point](#curve-point), [Dash pattern](#dash-pattern), [Easing curve](#easing-curve), [Edge shade](#edge-shade), [End](#end), [Faces](#faces), [Fill](#fill), [Fill rule](#fill-rule), [Followed](#followed), [Font](#font), [Group](#group), [Law](#law), [Layout](#layout), [Material](#material), [Math cell](#math-cell), [Matrix route](#matrix-route), [On atom](#on-atom), [On curve](#on-curve), [On object](#on-object), [On plot](#on-plot), [Palette index](#palette-index), [Region](#region), [Representation](#representation), [Residue part](#residue-part), [Residue pick](#residue-pick), [Residues](#residues), [Rulings](#rulings), [Set](#set), [Shape](#shape), [Sound](#sound), [Space curve](#space-curve), [Stroke](#stroke), [X axis](#x-axis)

## Top level

A scene file: one JSON object that describes one animation of math or science. It is data, not a program. `scene_config` sets the frame size, frame rate and background. `camera` sets the viewpoint. `objects` names every object by its object id and gives its shape and transform: flat shapes, text, math, axes, plots, solids, molecules and patterns. `timeline` lists the steps, each a wait or a play of animations that change objects or the camera. Only `objects` is required.

World space has x to the right, y up and z toward the default camera, in world units. A flat shape lies in its own xy plane, facing +z until it is rotated. Angles are in degrees, colors are `#rrggbb`, and durations are in seconds. Every field below says its unit, default and range. An unknown field is an error.

Run `sci_anim validate` to list every scene error with its JSON path, and `sci_anim inspect` to see where each object lands at a scene time. The `examples` folder of the sci_anim repository holds example scenes.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `scene_config` | [Scene config](#scene-config) |  | The frame size, frame rate and background of the scene. Leave it out for 1920 by 1080 pixels at 60 frames per second on `#1e1e1e`. The background can be `transparent` for PNG frames. |
| `camera` | [Camera](#camera) |  | Where the scene is seen from at scene time 0. Leave it out for a camera at `[0, 0, 10]` looking at the origin. |
| `objects` | map of object id to [Object](#objects) | yes | Every object of the scene, keyed by its object id, which animations use to name it. The key order does not matter. |
| `timeline` | array of [Step](#steps) |  | The steps, played one after another from scene time 0. The scene lasts the sum of their durations. The default is no steps: a scene one frame long. |
| `sounds` | array of [Sound](#sound) |  | Sound files, such as a narrator's voice clips, each played from the start of a chapter. `render` mixes them at full volume under the video and cuts them at its end, and `frame` and `inspect` ignore them. Set their loudness before, with ffmpeg or another tool. The default is none: a silent video. |

## Scene config

The scene config: the frame size in pixels, frame rate and background. Every field has a default, so the scene file can leave out any of them or all.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `width` | integer |  | The width of each frame in pixels, from 1 to 8192. The default is 1920. |
| `height` | integer |  | The height of each frame in pixels, from 1 to 8192. The default is 1080. |
| `fps` | integer |  | The frame rate, in frames per second, from 1 to 240. The default is 60. |
| `background` | string |  | An opaque `#rrggbb` color or `transparent`. The default is `#1e1e1e`. |

## Camera

The viewpoint the scene is seen from, with perspective. World space has x to the right, y up and z toward the default camera, in world units. The default camera stands at `[0, 0, 10]` and looks at the origin with a field of view of 45 degrees, so the frame shows about 8.3 world units from bottom to top in the plane z = 0, and 14.7 across at 16:9.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `position` | [number, number, number] |  | Where the camera stands, as `[x, y, z]` in world units. The default is `[0, 0, 10]`. |
| `target` | [number, number, number] |  | The point the camera looks at, as `[x, y, z]` in world units. It must differ from `position`. The default is the origin. |
| `up` | [number, number, number] |  | The direction that is up in the frame, as `[x, y, z]`. Any length works, but it must not point along the line from `position` to `target`. The default is `[0, 1, 0]`. |
| `fov` | number |  | The vertical field of view in degrees, more than 0 and less than 180. Smaller shows less and so zooms in. The default is 45. |

## Steps

A step either waits with nothing changing or plays animations, each over its span. Both last their `duration`. A chapter on the timeline names a run of steps.

### `wait`

Holds the stage as it is for `duration` seconds.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `wait` | yes |  |
| `duration` | number | yes | How long the step lasts, in seconds, more than 0. |

### `chapter`

A named run of steps, which `validate` and `inspect` report by its `id`, and which a sound can start at. It lasts as long as its steps, or `min_duration` when that is longer, holding the stage as its last step left it until then. Its steps are wait and play steps, not chapters.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `chapter` | yes |  |
| `id` | string | yes | The chapter's id, its own in the scene. |
| `min_duration` | number |  | The least the chapter lasts, in seconds, such as the length of the narration it carries. The default is no least: the sum of its steps. |
| `steps` | array of [Step](#steps) | yes | The chapter's steps, played one after another from its start. At least one. |

### `play`

Plays every animation in `animations`, each over its span, which is the whole step unless the animation sets `start` or `end`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `play` | yes |  |
| `duration` | number | yes | How long the step lasts, in seconds, more than 0. |
| `animations` | array of [Animation](#animations) | yes | Each plays over its span: the whole step, unless it sets `start` or `end`. Two animations whose spans overlap cannot change one value of one object, or of the camera, but one can play after the other in the same step. |

## Objects

One object: its shape, picked by `type`, with the shape's fields, beside its `position`, `rotation`, `scale` and `opacity`.

### Fields every object takes

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `position` | [number, number, number] |  | Where the object sits, as `[x, y, z]` in world units, or in data coordinates for a dot on axes or a sphere on 3D axes. A flat shape's own coordinates are measured from here. The default is the origin. |
| `rotation` | [number, number, number] |  | Angles in degrees. The object turns about its x axis, then its turned y axis, then its turned z axis. The default is `[0, 0, 0]`, which leaves a flat shape facing the default camera. |
| `scale` | [number, number, number] |  | A factor along each of the object's own x, y and z axes. A stroke keeps its width under any scale. The default is `[1, 1, 1]`. |
| `opacity` | number |  | The object opacity: 1 shows the object, 0 hides it, and between the two the object shows as one faded picture of itself. A fade animates it. A fade in starts from an object declared with opacity 0. |
| `attach` | [Attach](#attach) |  | Places this object beside another object, or beside an atom group of a molecule, so it stays there through that object's moves, rotates and scales and through camera moves: one `move` of a box carries the number attached to it. A caption is a `text` or `math` with this, and a leader line is an `arrow` or `line` with it. The object then has no `position`, `rotation` or `scale` of its own: it sits at the other object's position, or the group's center, on the frame, moved by `offset`, faces the camera, shows in front of all of the other object, and exists while it does. |

### `circle`

A flat circle about the object's position. No stroke when `stroke` is absent, and no fill when `fill` is absent.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `circle` | yes |  |
| `radius` | number | yes | The circle's radius in world units, more than 0. |
| `stroke` | [Stroke](#stroke) |  | The circle's outline. With none, the circle has no stroke and cannot be created. |
| `fill` | [Fill](#fill) |  | The circle's inside. With none, the circle has no fill. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `dot`

A filled disc about the object's position that marks a point: the tip of a vector, the map origin of a linear map, a point on a curve. Its own scale and rotation reshape it as a circle's would. A linear map moves its center to the center's image and keeps its size, so it stays visible even when the matrix flattens everything around it. It has no stroke, so fade it in rather than create it. `inspect` reports its world `center`. On `axes` it is a data point, the point of tangency, root or maximum of a plot.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `dot` | yes |  |
| `radius` | number | yes | The disc's radius in world units, more than 0, on axes too, so it stays round however they stretch. |
| `axes` | string |  | The object id of an `axes` object. The dot is then a data point: its `position`, and every move, follow and carry of it, is in their data coordinates, `[x, y, z]` with x and y as a plot on them writes them and z the distance off their plane in world units, normally 0. The axes carry it wherever they go, and a linear map names the axes, not the dot. The default is none: its position is in world units. |
| `fill` | [Fill](#fill) |  | The disc's color and opacity. The default is solid white. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `sphere`

A solid sphere about the object's position, lit as every solid is. It can fade, move, scale and rotate, and a create turns it into being as the revolution of a half circle, but it cannot be morphed.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `sphere` | yes |  |
| `radius` | number | yes | The sphere's radius in world units, more than 0, on 3D axes too. |
| `axes` | string |  | The object id of a `3d-axes` object. The sphere is then a data point: its `position`, and every move, follow and carry of it, is in their data coordinates, and the axes carry it wherever they go. The default is none: its position is in world units. |
| `material` | [Material](#material) |  | How the sphere's surface looks. The default is white, `roughness` 0.5 and `metallic` 0. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `prism`

A solid made by pushing `base` straight up by `height`, with a face the shape of the base at each end. A box is a prism on a square, a cylinder a prism on a circle, and a pipe a prism on a path with a hole in it under `even-odd`. The base lies flat in the object's xz plane: its x is the world's x, and its y runs along -z, away from the default camera. The prism stands on the object's position and rises along +y. A create raises it from its base over the create's span. A solid is lit by a key light from the camera's upper left and drawn with edges where its faces meet at a sharp angle and along its outline. It can fade, move, scale, rotate and recolor its `color`, but it cannot morph or be named by a linear map. `inspect` reports its bounding box.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `prism` | yes |  |
| `base` | [Region](#region) | yes | The region pushed up: a `circle`, `polygon` or `path` in the object's xz plane. |
| `height` | number | yes | How far the base is pushed up, in world units, more than 0. |
| `material` | [Material](#material) |  | How the prism's faces look. The default is white. The `opacity` must be 1, since a see-through solid cannot be drawn yet: fade the object's `opacity` instead. |
| `edges` | [Edge shade](#edge-shade) |  | `dark` for edges in a darker shade of the color, or `light` for a lighter one, for a dark solid whose dark edges vanish against the background. The default is `dark`. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `revolution`

A solid made by turning `profile` around the object's y axis through `angle` degrees. The profile is half of the solid's outline as the default camera sees it, drawn in the object's xy plane with x of 0 or more: a cone is the revolution of a right triangle with one side on the axis, a cylinder of a rectangle, a torus of a circle whose `center` lies beside the axis, and a flask or a lens of a path. With an `angle` under 360 the solid is cut open and closed by its profile at both cut faces, so a 270 degree planet shows its layers. It turns from +x toward -z, away from the default camera. A create turns it from 0 to its `angle`. It is lit, drawn with edges, animated and inspected as a prism is.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `revolution` | yes |  |
| `profile` | [Region](#region) | yes | The region turned: a `circle`, `polygon` or `path` in the object's xy plane, at x of 0 or more. |
| `angle` | number |  | How far the profile turns, in degrees, more than 0 and at most 360. The default is 360, a whole solid. |
| `material` | [Material](#material) |  | How the revolution's faces look. The default is white. The `opacity` must be 1, since a see-through solid cannot be drawn yet: fade the object's `opacity` instead. |
| `edges` | [Edge shade](#edge-shade) |  | `dark` for edges in a darker shade of the color, or `light` for a lighter one, for a dark solid whose dark edges vanish against the background. The default is `dark`. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `tube`

A solid made by carrying a circle of `radius` along the space curve `curve`, always square to the curve, on a frame that turns no more than the curve does, so the tube never twists. A spring is a tube along a helix, and an orbit, a field line or a strand of DNA is a tube too. Each open end is closed by a disc, and a curve that ends where it starts closes on itself with no ends. A create draws it along the curve by length, from the start of `u_range` or the first point. It is lit, drawn with edges, animated and inspected as a prism is, and `inspect` adds the world point at each end of its curve and, for a curve through points or an atom trace, of each curve point. A tube through curve points morphs into another through as many, curve point by curve point. With `axes`, the curve is written in the data coordinates of those 3D axes, which place the tube, and the tube is cut where its curve leaves their box, each piece closed by a disc.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `tube` | yes |  |
| `curve` | [Space curve](#space-curve) | yes | The curve the tube runs along: x, y and z as expressions in `u`, or the points it passes through, in world units from the object's position, or in data units with z up on `axes`, or an atom trace of a PDB file, in angstroms. |
| `axes` | string |  | The object id of a `3d-axes` object whose data coordinates the curve is written in. The tube then has no `position`, `rotation` or `scale` of its own: the axes place it. The default is none: the tube is placed by its own transform. |
| `radius` | number | yes | The radius of the tube's circle, in world units, more than 0. Keep it under the tightest bend's radius, or the tube folds into itself there. |
| `material` | [Material](#material) |  | How the tube's faces look. The default is white. The `opacity` must be 1, since a see-through solid cannot be drawn yet: fade the object's `opacity` instead. |
| `edges` | [Edge shade](#edge-shade) |  | `dark` for edges in a darker shade of the color, or `light` for a lighter one, for a dark solid whose dark edges vanish against the background. The default is `dark`. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `surface`

A solid that is a sheet: its height `z`, an expression in `x` and `y`, over the rectangle `x_range` by `y_range`, as a textbook writes z = f(x, y). The rectangle lies on the object's floor, read as a prism's base is: x along the object's x, and y along its -z, away from the default camera. The height rises along the object's own up, +y, so z is vertical. Everything is in world units from the object's position. A potential well, a wave's height and a function of two variables are surfaces. It is sampled finer where it bends and lit on both sides, so the camera may look at it from above or below. A create rises it from the flat rectangle at height 0 to its heights. It is drawn with edges along its rim and where it folds away from the camera, and is animated and inspected as a prism is. With `axes`, x, y and z are the data coordinates of those 3D axes, which place the surface, the ranges default to theirs, and the sheet is cut where it leaves their z range, so a function with a pole stops at the box. A create then rises it from data z of 0, or the nearest end of the z range.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `surface` | yes |  |
| `z` | string | yes | An expression in `x` and `y`, such as `-2*exp(-(x^2 + y^2)/2)`, which gives the sheet's height z over the point (x, y), for each x of `x_range` and y of `y_range`, as a textbook writes z = f(x, y). The height rises along the object's own up, in world units from its position. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It must have a finite value within 10000 at every point of the rectangle: `sqrt(x)` over an `x_range` below 0 is a scene error, with the x and y where the value is missing. |
| `x_range` | [number, number] |  | The lowest and highest x, as `[low, high]`, low first, in world units from the object's position, or in data units on `axes`, where the default is the axes' x range and the rectangle is cut to it. Required without `axes`. |
| `y_range` | [number, number] |  | The lowest and highest y, as `[low, high]`, low first, in world units from the object's position, or in data units on `axes`, where the default is the axes' y range. y runs along the object's -z, away from the default camera. Required without `axes`. |
| `axes` | string |  | The object id of a `3d-axes` object whose data coordinates the surface is written in. The surface then has no `position`, `rotation` or `scale` of its own: the axes place it. The default is none: the surface is placed by its own transform. |
| `material` | [Material](#material) |  | How the sheet looks, on both sides. The default is white. The `opacity` must be 1, since a see-through solid cannot be drawn yet: fade the object's `opacity` instead. |
| `edges` | [Edge shade](#edge-shade) |  | `dark` for edges in a darker shade of the color, or `light` for a lighter one, for a dark solid whose dark edges vanish against the background. The default is `dark`. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `line`

A straight stroke from `from` to `to`. With no `stroke` it is white and 0.04 wide, and a `stroke` that is written sets its own `color`. An end may sit on an object, as `{ "object": "block", "offset": [0, -1, 0] }`: the line is then placed by its ends, lies between them at every scene time wherever those objects go, has no `position`, `rotation` or `scale` of its own, and is hidden while either object is. `inspect` reports each end's world point under `ends`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `line` | yes |  |
| `from` | [End](#end) | yes | The start: `[x, y]` in world units in the object's plane, measured from its position, or an end on an object, a curve or an atom. On a line placed by its ends, `[x, y]` or `[x, y, z]` is a point in the world. |
| `to` | [End](#end) | yes | The end, written as `from` is. It must differ from `from`. |
| `stroke` | [Stroke](#stroke) |  | The line's color and width. The default is white and 0.04 wide. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `arrow`

A line from `from` to `to` with a filled tip at `to`, both in the stroke's color. The tip is a triangle `tip_length` long and as wide, or a quarter of the arrow when the arrow is shorter than four tips. With no `stroke` it is white and 0.04 wide. An end may sit on an object, as a line's may: a force arrow then stays on its block as the block moves, and its tip keeps its length as the arrow stretches.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `arrow` | yes |  |
| `from` | [End](#end) | yes | The tail: `[x, y]` in world units in the object's plane, measured from its position, or an end on an object, a curve or an atom. On an arrow placed by its ends, `[x, y]` or `[x, y, z]` is a point in the world. |
| `to` | [End](#end) | yes | The point of the tip, written as `from` is. It must differ from `from`. |
| `tip_length` | number |  | The length of the tip in world units, more than 0. The tip is as wide as it is long. The default is 0.35. |
| `stroke` | [Stroke](#stroke) |  | The color and width of the shaft. The tip is filled in the same color. The default is white and 0.04 wide. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `spring`

A coil of `turns` turns of `radius` about the straight line from `from` to `to`, drawn as a `tube` of `tube_radius` on a helix, so it stretches and squashes as its ends move and keeps its turns. Each end is a point in the world, `[x, y]` or `[x, y, z]`, or an end on an object, as `{ "object": "mass", "offset": [0, 0.3, 0] }`. A spring is placed by its ends: it has no `position`, `rotation` or `scale` of its own, and is hidden while either object is. It is lit, created, faded and recolored as a tube is, and `inspect` reports each end's world point under `ends`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `spring` | yes |  |
| `from` | [End](#end) | yes | Where the coil starts. |
| `to` | [End](#end) | yes | Where the coil ends. |
| `turns` | number |  | How many turns the coil makes between its ends, more than 0 and at most 100. The default is 8. |
| `radius` | number |  | The radius of the coil about the line between its ends, in world units, more than 0. The default is 0.2. |
| `tube_radius` | number |  | The radius of the tube the coil is drawn as, in world units, more than 0. The default is 0.03. |
| `material` | [Material](#material) |  | How the coil's faces look. The default is white. The `opacity` must be 1. |
| `edges` | [Edge shade](#edge-shade) |  | `dark` or `light` edges, as for a tube. The default is `dark`. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `trail`

The world points `object` passed through over the last `length` seconds of scene time, or since it began to show, drawn as a stroke that faces the camera. The points are its world position at every frame time in that time, and the newest is where it stands, so a trail follows a law, a carry, a move or a data point alike: an orbit, a pendulum's arc and a projectile's path are trails. A trail has no `position`, `rotation` or `scale` of its own. It is not created: it grows as its object moves, so fade it in, or declare it shown from the start. `inspect` reports its `point_count`, its `newest` point and the pixel that lands on, and its bounding box.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `trail` | yes |  |
| `object` | string | yes | The object id of the object whose path the trail draws: not a plot, and not a trail. |
| `length` | number |  | How many seconds of scene time back the trail reaches, more than 0. The default is every second since the object began to show: from the start of its create, the end of the reaction or morph that makes it, or scene time 0. |
| `stroke` | [Stroke](#stroke) |  | The trail's color and width. The default is white and 0.04 wide. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `readout`

A number typeset as math, with `unit` after it, such as a speed that counts as a ball falls. Its `value` changes as a track does: a `count` eases it to a `to`, a follow with `"sets": "value"` sets it from a law with one expression, `value`, and a simulation sets it. Its digits and the layout of each count of digits are typeset once, so a number that counts costs no more than one that stands still. The number is lined up on its decimal point, or on the end of its last digit with no decimals, at the object's position, so its digits keep their places as it grows; give it room to the left. It is written, faded, attached and recolored as math is. `inspect` reports its `value` and the `text` it shows under `readout`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `readout` | yes |  |
| `value` | number |  | The value shown until an animation changes it. The default is 0. |
| `decimals` | integer |  | How many decimals the number shows, from 0 to 6. The default is 0. |
| `unit` | string |  | LaTeX math set after the number, as in a math object's `tex`, such as `\\text{ m/s}` as JSON writes it, whose space keeps the unit off the number. The default is none. |
| `size` | number |  | The text size: the height of one em, in world units, more than 0. The default is 0.6. |
| `color` | string |  | The color of every glyph, written `#rrggbb`. The default is white. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `polygon`

A closed path through `points` in order, at least three.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `polygon` | yes |  |
| `points` | array of [number, number] | yes | The corners in order, each `[x, y]` in world units in the object's plane, measured from its position. At least three. The path closes from the last back to the first. |
| `stroke` | [Stroke](#stroke) |  | The polygon's outline. With none, it has no stroke and cannot be created. |
| `fill` | [Fill](#fill) |  | The polygon's inside. With none, it has no fill. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `arc`

Part of a circle about the object's position, from `start_angle` degrees counterclockwise from +x, sweeping `sweep` degrees, counterclockwise when positive. A fill fills the region between the arc and its chord.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `arc` | yes |  |
| `radius` | number | yes | The radius of the circle the arc is part of, in world units, more than 0. |
| `start_angle` | number |  | Where the arc starts, in degrees counterclockwise from +x. The default is 0. |
| `sweep` | number | yes | How far the arc runs from `start_angle`, in degrees from -360 to 360, counterclockwise when positive. It must not be 0. |
| `stroke` | [Stroke](#stroke) |  | The arc's outline. With none, it has no stroke and cannot be created. |
| `fill` | [Fill](#fill) |  | The region between the arc and its chord. With none, it has no fill. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `path`

Any path, written in SVG path syntax in `d`, such as `M -1 -1 L 1 -1 L 1 1 Z M 0 0 A 1 1 0 1 0 0 0.01 Z`. A path can have several parts, and `fill.rule` says whether a part inside another is a hole.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `path` | yes |  |
| `d` | string | yes | SVG path data: the commands `M L H V C S Q T A Z`, upper case for absolute points and lower case for relative ones, with numbers in world units measured from the object's position. Unlike SVG, y points up. `M` starts a new part. |
| `stroke` | [Stroke](#stroke) |  | The path's outline. With none, it has no stroke and cannot be created. |
| `fill` | [Fill](#fill) |  | The path's inside. With none, it has no fill. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `text`

Plain text in the `font` it names, Libertinus Sans unless it is `serif`, filled in `color`. A `\n` breaks a line. Nothing else in it is markup. `size` is the em in world units. `align` lines up the lines and picks the point of the text's box at the object's position: the middle of its left edge, its center, or the middle of its right edge. The box runs from the first line's cap height to the last line's baseline. A write draws it glyph by glyph.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `text` | yes |  |
| `text` | string | yes | The characters to set, as plain text. `\n` starts a new line, and nothing else is markup. The bundled fonts cover Latin, Greek and Cyrillic, and a character with no glyph in them is a scene error. |
| `font` | [Font](#font) |  | `sans` for Libertinus Sans or `serif` for Libertinus Serif. The default is `sans`. |
| `size` | number |  | The text size: the height of one em, in world units, more than 0. The default is 0.6. |
| `color` | string |  | The color every glyph is filled in, written `#rrggbb`. The default is white. |
| `align` | [Align](#align) |  | `left`, `center` or `right`: how the lines line up, and which point of the text's box sits at the object's position. The default is `center`. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `math`

An equation in LaTeX math syntax, such as `e^{i\\pi} + 1 = 0`, set in display appearance in New Computer Modern Math. Each glyph is filled in the color LaTeX gives it with `\textcolor` or `\color`, or else in `color`. `size`, `align` and the write are as for text.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `math` | yes |  |
| `tex` | string | yes | LaTeX math, such as `\frac{a}{b} + \sqrt{x}`. JSON needs each backslash written twice, so the scene file holds `"\\frac{a}{b} + \\sqrt{x}"`. mitex turns it into Typst math, so it takes the commands and environments mitex knows, such as `\text`, `\mathbb`, `pmatrix`, `cases` and `aligned`. A command mitex does not know, such as mhchem's `\ce`, is a scene error. `\textcolor{#58c4dd}{x}` colors one term, and `{\color{#58c4dd} x + y}` colors the rest of its group. A color is `#rrggbb` or one of red, green, blue, cyan, magenta, yellow, black, white, gray, lightgray, darkgray, brown, orange, pink, purple, teal and olive, in any case, and any other color is a scene error. These commands mitex knows are a scene error too: `\boxed` and `\fbox`, which draw a box sci_anim cannot draw yet, `\Box`, whose glyph the bundled font lacks, `\limits` and `\nolimits` with no operator before them, and the commands of a document rather than an equation, `\bottomrule`, `\cite`, `\eqref`, `\footnote`, `\iffalse`, `\includegraphics`, `\item`, `\midrule`, `\newenvironment`, `\newenvironment*`, `\ref`, `\renewenvironment`, `\renewenvironment*`, `\textwidth` and `\toprule`, and the environment `tabular`. |
| `size` | number |  | The text size: the height of one em, in world units, more than 0. The default is 0.6. |
| `color` | string |  | The color of every glyph that `tex` does not color, written `#rrggbb`. The default is white. |
| `align` | [Align](#align) |  | `left`, `center` or `right`: how the lines line up, and which point of the math object's box sits at the object's position. The default is `center`. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `table`

Text and math in cells, in `rows` of cells from the top, each row's cells from the left, with rulings between them, centered on the object's position. Each column is as wide as its widest cell and each row as tall as its tallest, plus `padding` on every side of each cell. A write draws the rulings along their length over the first fifth of its span, then writes the cells in reading order, row by row. A `highlight` with a `cell` of `[row, column]` lights one cell, and a `recolor` changes every cell's `color` and the rulings' `stroke`. `inspect` reports `table`: its `size`, `rows`, `columns` and each cell's `cell` address, `text` or `tex`, own `created` share, `glow`, `rectangle` and the `bounding_box` of its glyphs, which `check-layout` checks as it checks a text object's, naming the cell `table/row/column`. Needs sci_anim 0.18.0 or later.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `table` | yes |  |
| `rows` | array of array of [Cell](#cell) | yes | The rows from the top, each a list of its cells from the left, every row as long as the first. A cell is a string of plain text, set as a `text` object's `text` is, an object `{ "tex": "..." }` of LaTeX math, set as a `math` object's `tex` is, or `""` for an empty cell. Such as `[["Codon", "Amino acid"], ["AUG", "Met"]]`. |
| `size` | number |  | The text size of every cell: the height of one em, in world units, more than 0. The default is 0.6. |
| `color` | string |  | The color of every glyph that a cell's `tex` does not color, written `#rrggbb`. The default is white. |
| `font` | [Font](#font) |  | The font of the text cells: `sans` for Libertinus Sans or `serif` for Libertinus Serif. Math is set in New Computer Modern Math. The default is `sans`. |
| `align` | [Align](#align) |  | `left`, `center` or `right`: where each cell's text or math sits across its cell, `padding` from its left edge, in its middle, or `padding` from its right edge. It sits in the middle of its row's height. The default is `center`. |
| `padding` | number |  | The cell padding: the space between each cell's edges and its text or math, on every side, in world units, 0 or more. The default is 0.4 of `size`. |
| `rulings` | [Rulings](#rulings) |  | Which rulings to draw: `all` between every row and column and around the table, `header` one under the first row, or `none`. The default is `all`. |
| `stroke` | [Stroke](#stroke) |  | The color and width of the rulings. The default is gray, `#888888`, and 0.02 wide. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `axes`

Axes: `x` and `y` each give the `range` of data values they show, the `spacing` of their ticks (a round number giving about five when left out) and a `label` in LaTeX math, set at the high end. `width` and `height` are the lengths of the axes in world units, in a box centered on the object's position. The x axis crosses the y axis at 0, or at the nearest end of the y range. Tick labels are set in math. A create draws the axis lines and ticks along their length, then fades the labels in. Plots name these axes and are placed by them. `inspect` reports the text, side and bounding box of each tick label, the bounding box of each axis label, and on each plot the labels its stroke crosses.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `axes` | yes |  |
| `x` | [X axis](#x-axis) | yes | The horizontal axis: its range, tick spacing and label, or its `categories` and label, a category axis for a bar plot. |
| `y` | [Axis](#axis) | yes | The vertical axis: its range, tick spacing and label. |
| `width` | number | yes | The length of the x axis in world units, more than 0. |
| `height` | number | yes | The length of the y axis in world units, more than 0. |
| `stroke` | [Stroke](#stroke) |  | The color and width of the axis lines and ticks. The default is white and 0.04 wide. |
| `color` | string |  | The color of the tick labels and axis labels, written `#rrggbb`. The default is white. |
| `label_size` | number |  | The text size of the tick labels and axis labels: one em, in world units, more than 0. The default is 0.3. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `3d-axes`

Three axes with ticks, tick labels and axis labels, z up as a textbook draws them: data `(x, y, z)` lands at x to the right, z up and y away from the default camera, each axis scaled so its `range` fills its `length`, in a box centered on the object's position. Each axis crosses the others at 0, or at the nearest end of their range, and the tick there has no label. An x or y tick label sits below its tick, or above where the axis runs over the box's middle, and a z tick label sits left of its tick on the frame, from wherever the camera stands. An axis label sits past the high end of its axis. Every label faces the camera at every frame and is hidden by what stands in front of it. The lines and ticks keep the stroke's width from every side. A create draws the lines and ticks along their length, then fades the labels in. It is moved, scaled, rotated and faded as any object, and cannot be morphed, recolored or named by a linear map. `inspect` reports where data lands in the world and each tick with its label's text, side and bounding box.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `3d-axes` | yes |  |
| `x` | [Axis 3d](#axis-3d) | yes | The axis to the right: its range, tick spacing, length and label. |
| `y` | [Axis 3d](#axis-3d) | yes | The axis away from the default camera, along -z of the world: its range, tick spacing, length and label. |
| `z` | [Axis 3d](#axis-3d) | yes | The axis up: its range, tick spacing, length and label. |
| `stroke` | [Stroke](#stroke) |  | The color and width of the axis lines and ticks. The default is white and 0.04 wide. |
| `color` | string |  | The color of the tick labels and axis labels, written `#rrggbb`. The default is white. |
| `label_size` | number |  | The text size of the tick labels and axis labels: one em, in world units, more than 0. The default is 0.3. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `number-line`

One axis with ticks and labels, `length` long in world units and centered on the object's position, drawn, created and inspected as axes are.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `number-line` | yes |  |
| `range` | [number, number] | yes | The lowest and highest value the line shows, as `[low, high]`, low first. |
| `spacing` | number |  | The distance in data units between ticks, more than 0, giving at most 100 ticks. The default is a round number that gives about five. |
| `length` | number | yes | The length of the line in world units, more than 0. |
| `label` | string |  | LaTeX math set at the high end of the line, as in `tex`. The default is no label. |
| `stroke` | [Stroke](#stroke) |  | The color and width of the line and its ticks. The default is white and 0.04 wide. |
| `color` | string |  | The color of the tick labels and the label, written `#rrggbb`. The default is white. |
| `label_size` | number |  | The text size of the tick labels and the label: one em, in world units, more than 0. The default is 0.3. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `function-plot`

The graph of `expression`, an expression in `x`, over `x_range` (the axes' own x range when left out), in the coordinates of the axes object `axes`. It has no position, rotation or scale of its own and is placed by its axes. Where the function has no value, or leaves the axes, the curve is cut. `inspect` names the labels of its axes and the shown text and math objects that its stroke crosses.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `function-plot` | yes |  |
| `axes` | string | yes | The object id of the `axes` object whose data coordinates the plot is drawn in. |
| `expression` | string | yes | An expression in `x`, such as `x^2 - 3*sin(2*x)`, which gives y for each x in data units. It may also read `t`, law time in seconds, as in the travelling wave `sin(2*x - 3*t)`: a `follow` with `"sets": "shape"` then runs it, and before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. Where the expression has no value, such as `sqrt(x)` for x below 0, the curve has a gap, and where it jumps, such as `step(x)` at 0, the curve breaks. |
| `x_range` | [number, number] |  | The lowest and highest x to plot, as `[low, high]` in data units, low first. The default is the axes' x range. |
| `stroke` | [Stroke](#stroke) |  | The curve's color and width. The default is white and 0.04 wide. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `point-plot`

The line through `points` in order, in the coordinates of the axes object `axes`, cut at the axes' box. For data and for anything an expression cannot say. `inspect` reports it as it reports a function plot.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `point-plot` | yes |  |
| `axes` | string | yes | The object id of the `axes` object whose data coordinates the plot is drawn in. |
| `points` | array of [number, number] | yes | The points the curve runs through in order, each `[x, y]` in data units. At least two. |
| `stroke` | [Stroke](#stroke) |  | The curve's color and width. The default is white and 0.04 wide. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `bar-plot`

One bar for each category of the axes object `axes`, whose `x` is a category axis: a filled rectangle 0.7 of its category's width, standing on the x axis and reaching its value. A negative value's bar hangs below the x axis, and the part of a bar outside the axes' box is cut, as a curve is. It has no position, rotation or scale of its own and is placed by its axes. A create grows the bars from the x axis one after another, in category order. A fade fades every bar, and a recolor changes every bar's `fill` and `stroke`; one bar cannot be animated alone. `inspect` reports `bar_plot`: its `axes`, and for each bar its `category`, `value`, own `created` share and its whole `rectangle` in world units and pixels. Needs sci_anim 0.16.0 or later.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `bar-plot` | yes |  |
| `axes` | string | yes | The object id of the `axes` object whose category axis the bars stand on. |
| `values` | array of number | yes | Each category's value in the y axis' data units, in the order of the axes' `categories`, one value for each category. |
| `fill` | [Fill](#fill) | yes | The bars' color and opacity. |
| `stroke` | [Stroke](#stroke) |  | The bars' outline. The default is none. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `molecule`

The atoms and bonds of the structure file `file`, a path relative to the scene file, drawn as `representation` says. Fetch a structure file with PubChem before the render, for example `curl -fo caffeine.sdf "https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/caffeine/SDF?record_type=3d"`. A compound with no 3D model, such as a protein, answers 404, and `-f` stops curl saving the error as a file; fetch a protein, DNA or RNA as a PDB file from the Protein Data Bank instead, as `curl -fo 1ubq.pdb https://files.rcsb.org/download/1UBQ.pdb`. Atoms are in angstroms, one world unit each, placed about their centroid, so the object's `position` is the middle of the molecule, and its `rotation` turns it in place. `scale` sizes the whole molecule. `hydrogens: false` leaves every hydrogen and its bonds undrawn, and `inspect` still reports them as hidden atoms. A molecule can fade, move, scale and rotate, but not be created or written.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `molecule` | yes |  |
| `file` | string | yes | The path of a structure file, relative to the folder of the scene file: a PDB file when its name ends in `.pdb` or `.ent`, and an SDF file otherwise. sci_anim never downloads one. Fetch PubChem's 3D record first, from `https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/<name>/SDF?record_type=3d`. Without `record_type=3d`, PubChem serves a 2D record whose atoms all lie in one plane. Fetch a PDB file from `https://files.rcsb.org/download/<PDB id>.pdb`. sci_anim reads its first model and leaves out its waters, and of an atom the file gives alternate locations, it keeps the first, usually `A`. Its bonds are its `CONECT` records, and in amino acids and nucleotides the bonds their atoms' distances show, within a residue and to the next residue of its chain; every bond draws single. A PDB file usually has no hydrogens, and the molecule draws without them. Atoms keep the serial numbers the file gives them. Reading a PDB file needs sci_anim 0.17.0 or later. |
| `representation` | [Representation](#representation) |  | How the molecule is drawn. `ball-and-stick`, the default, draws each atom a ball a quarter of its element's van der Waals radius, and each bond sticks between them, one for a single bond, two for a double and three for a triple: for a molecule of up to a few hundred atoms, whose bonds a lesson talks about. `space-filling` draws each atom a ball at its element's van der Waals radius, overlapping its bonded neighbors, and no sticks: the space the molecule fills, for a molecule too big to read as balls and sticks, such as a stretch of DNA or a protein of thousands of atoms. A lit group's atoms take its glow's color, and `inspect` reports every bond hidden. A reaction cannot turn a space-filling molecule, since it draws bonds breaking and forming. `space-filling` needs sci_anim 0.21.0 or later. `cartoon` draws a protein, DNA or RNA read from a PDB file as a textbook draws its fold: each chain's backbone as one tube through its amino acids' alpha carbons, `CA`, or its nucleotides' phosphorus atoms, `P`. The tube is a thin gray circle along coil, widens to a flat red ribbon along each helix of a `HELIX` record, its flat side facing out from the helix's axis, and to a flat blue arrow along each β-strand of a `SHEET` record, lying in its β-sheet's plane, with a tip that steps out one residue before the strand's end and narrows to its point there. It changes shape smoothly over half a residue at each end of a stretch. A file with no `HELIX` or `SHEET` records draws as coil. The tube breaks where the file leaves residues out. No atom is drawn as a ball, so `inspect` reports every atom as hidden and `hydrogens` does nothing. A highlight of an atom group lights the tube along each residue whose `CA` or `P` the group holds, so a group written by residues lights that stretch of the ribbon. A molecule read from an SDF file cannot be a cartoon, and neither can a reactant or a product. `cartoon` needs sci_anim 0.22.0 or later. |
| `hydrogens` | boolean |  | Whether hydrogen atoms and their bonds are drawn, in either representation. With `false` they are hidden atoms: still part of the molecule, its atom groups and its reactions. The default is `true`. |
| `groups` | map of name to [Group](#group) |  | Named sets of atoms, by the atoms' numbers in the structure file, such as `{ "carbonyl": [1, 9] }`. A group can be highlighted and a caption attached to it. A group may include atoms that `hydrogens: false` hides. A product's group receives the glow of a reactant group whose atoms become exactly its atoms. A molecule read from a PDB file can name a group's atoms by residue instead, as `{ "helix": { "residues": [{ "chain": "A", "from": 23, "to": 34 }] } }`. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

### `pattern`

One shape drawn at every copy of a layout. A crystal lattice is a pattern of a `sphere` on a `grid`, a ruler's ticks are a pattern of a `line` on a grid one copy high, a ring of atoms is a pattern on a `ring`, and a vector field is a pattern of an `arrow` on a grid, each copy turned and scaled by expressions in its place. `layout` says where the copies go, and `copy` gives each copy its own position, rotation, scale and color share from expressions in its index and place. A pattern holds at most 10000 copies.

A pattern is one object, placed by its own `position`, `rotation` and `scale`: one rotate turns the whole lattice, a fade fades every copy, a recolor changes the shape's colors, and a create grows the copies in index order, each starting a little after the one before. A copy is not animated alone; to point at one copy, draw a separate object over it. A copy's scale and the pattern's size a flat copy's stroke with the rest of it. `inspect` reports `copy_count`, and for each copy its `index`, its world `position` and the pixel it lands on, its `rotation` in the pattern's own space, its own `created` share, its `colors` and its `bounding_box`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `pattern` | yes |  |
| `shape` | [Shape](#shape) | yes | The shape every copy draws: a `circle`, `dot`, `line`, `arrow`, `polygon`, `arc`, `path`, `sphere`, `prism`, `revolution`, `tube` or `surface`, with that shape's own fields and no `position`, `rotation`, `scale` or `opacity`. Its coordinates are in world units from the copy's position. Text, math, axes, number lines, plots, molecules and patterns are scene errors. |
| `layout` | [Layout](#layout) | yes | Where the copies go, in the pattern's own space: a `grid`, a `ring`, or `along` a space curve. |
| `copy` | [Copy](#copy) |  | Each copy's own position, rotation, scale, color share and pick of a palette, as expressions in its index and place. Leave it out, or any field of it, for copies at their places, unturned, at scale 1 and in the shape's own colors. |
| `position` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `rotation` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `scale` | [number, number, number] |  | See [Fields every object takes](#fields-every-object-takes). |
| `opacity` | number |  | See [Fields every object takes](#fields-every-object-takes). |
| `attach` | [Attach](#attach) |  | See [Fields every object takes](#fields-every-object-takes). |

## Animations

The file form of an `Animation`. The change's fields sit beside `easing_curve`, tagged by `type`.

### Fields every animation takes

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `easing_curve` | [Easing curve](#easing-curve) |  | How the animation's progress follows the elapsed share of its span: `linear`, `slow-start`, `slow-end`, `slow-both` or `overshoot`. The default is `slow-both`. A follow with a `law` takes none, because its law sets its value at every scene time. |
| `start` | number |  | When the animation starts, in seconds from the start of its step, such as the time a narrator says the word it shows. The default is 0. |
| `end` | number |  | When the animation ends, in seconds from the start of its step: more than `start`, and at most the step's `duration`, which is the default. |

### `fade`

Changes the object opacity to `to`. With `copy`, fades only that copy of a pattern: its own opacity, which the pattern's object opacity applies on top of, so a copy faded to 0 stays hidden while the pattern fades in.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `fade` | yes |  |
| `object` | string | yes | The object id of the object to fade. |
| `copy` | [integer] or [integer, integer] or [integer, integer, integer] |  | The copy index of one copy of the pattern `object` to change alone, as `inspect` reports it: `[i, j, k]` for a grid and `[i]` for a ring or an along, each from 0. Needs sci_anim 0.20.0 or later. |
| `to` | number | yes | The object opacity to end at, from 0 (hidden) to 1 (shown). To fade an object in, declare it with `opacity` 0 and fade it to 1. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `count`

Counts a readout's value to `to` over its span, shaped by its easing curve, from the value it has when the count begins.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `count` | yes |  |
| `object` | string | yes | The object id of a `readout`. |
| `to` | number | yes | The value to end at. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `move`

Moves the object's position to `to`. With `copy`, moves only that copy of a pattern, and `to` is the copy's position in the pattern's own space, as its layout places it, so a move, rotate or scale of the whole pattern still carries the moved copy. A copy of a pattern whose copy expressions read `t` is placed by them and cannot be moved.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `move` | yes |  |
| `object` | string | yes | The object id of the object to move. |
| `copy` | [integer] or [integer, integer] or [integer, integer, integer] |  | The copy index of one copy of the pattern `object` to change alone, as `inspect` reports it: `[i, j, k]` for a grid and `[i]` for a ring or an along, each from 0. Needs sci_anim 0.20.0 or later. |
| `to` | [number, number, number] | yes | The position to end at, as `[x, y, z]` in world units, or in data coordinates for a dot on axes or a sphere on 3D axes. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `scale`

Scales the object to `to` along its own axes.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `scale` | yes |  |
| `object` | string | yes | The object id of the object to scale. |
| `to` | [number, number, number] | yes | The scale to end at, as a factor along each of the object's own x, y and z axes. `[1, 1, 1]` is the declared size. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `rotate`

Turns the object to the rotation `to`, in degrees. Each angle moves straight to its new value, so `[0, 360, 0]` from `[0, 0, 0]` turns the object a full turn.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `rotate` | yes |  |
| `object` | string | yes | The object id of the object to turn. |
| `to` | [number, number, number] | yes | The rotation to end at, as angles in degrees about x, y and z, as in `rotation`. Values past 360 turn more than once. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `create`

Draws a flat shape over its span: its stroke along its path by length, then its fill fading in. A solid grows, and a pattern's copies are created in index order, each starting a little after the one before. The object shows nothing before the create starts. A molecule cannot be created, and text and math are written.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `create` | yes |  |
| `object` | string | yes | The object id of a flat shape with a stroke, a solid, a pattern, axes, 3D axes or a number line. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `write`

Draws text or math glyph by glyph over its span, a few at a time: each glyph's outline along its path, then its fill fading in as the outline thins away. The object shows nothing before the write starts.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `write` | yes |  |
| `object` | string | yes | The object id of a text or math object. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `reaction`

Turns the molecule objects in `reactants` into the molecule objects in `products` over its span. `atom_map` pairs every reactant atom with the product atom it becomes, each named by its object id and its atom number in that object's structure file. Each atom moves in a straight line from its reactant position to its product position, a broken bond thins away over the first half, and a formed bond thickens over the second. Each product ends at its own `position` and `scale`, turned by the fit, the rotation that lines it up best with the reactants, and then by its own `rotation`, so `[0, 0, 0]` keeps the fit. With `fit` false, each product ends at exactly its own `rotation`. The reactants show until the reaction begins and the products from its end, so neither needs a fade. A reactant group lit when the reaction begins keeps its glow on its atoms as they move, and passes it at the reaction's end to the group of each product that holds exactly the atoms that product receives from it. Declare that group in the product's `groups`, or it is a scene error. Two lit groups cannot pass their glow to one product group.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `reaction` | yes |  |
| `reactants` | array of string | yes | The object ids of the molecule objects the reaction uses up. At least one. |
| `products` | array of string | yes | The object ids of the molecule objects the reaction makes. At least one. |
| `atom_map` | array of [Atom pair](#atom-pair) | yes | The atom map: one pair for every atom of the reactants, hidden hydrogens included, such as `{ "reactant": { "object": "ethylene", "atom": 1 }, "product": { "object": "chloroethane", "atom": 2 } }`. Every reactant atom and every product atom appears in exactly one pair, and the two atoms of a pair are one element. `sci_anim inspect` lists each molecule's atoms with their numbers, elements and positions. |
| `fit` | boolean |  | Whether each product is turned by the fit, the one rotation that lines the products up best with the reactants, before its own `rotation`. The default is `true`. With `false`, each product ends at exactly its own `position` and `rotation`, as `inspect` reports the object after the reaction: for products placed by hand, such as loose atoms set beside each other, which the fit would turn about their common middle. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `morph`

Reshapes the flat shape, text, math, plot or tube `object` into `into`'s shape, stroke and fill, moving it to where `into` is. The source shows until the morph ends and `into` from then, so neither needs a fade, and `into` is not created or written. The parts of the two are matched in path order, text glyph to glyph, and a part with no partner grows from or shrinks to a point. A plot morphs only into a plot on the same axes, and each x of it moves straight up or down. A tube whose curve is `points` or a `trace` morphs only into another such tube through as many curve points, each curve point moving straight to its partner, the k-th to the k-th, and the curve made again through them at every scene time; its radius and material mix too. Morphing a tube needs sci_anim 0.19.0 or later.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `morph` | yes |  |
| `object` | string | yes | The object id of the source: the flat shape, text, math, plot or tube that is reshaped. |
| `into` | string | yes | The object id of the target, whose shape, stroke, fill and place the source takes. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `highlight`

Lights the atom group `group` of the molecule `object` with a glow, a see-through shell around its atoms and the bonds between them, to the share `to`: 1 is all of the glow and 0 is none. It starts from the glow the steps before left, none at first. `color` is the glow's color, `#ffd23f` unless an earlier highlight set another, and a highlight that leaves it out keeps the color it has. A glow that is off lights up in `color` from the start of the span, and only its share changes. Light a group in and out with two highlights. A group whose atoms are all hidden has no glow to light. A reaction carries a lit group's glow and passes it to a product group, so a highlight of a group of a reaction's molecule in the reaction's own step is a conflict. On a table it lights the cell at `cell` instead: a see-through fill of the cell's rectangle, behind its text and rulings, to the share `to` (sci_anim 0.18.0 or later). On a pattern it lights the copy at `copy`, and on a bar plot the bar at `bar` (sci_anim 0.20.0 or later): a see-through box a little larger than a flat copy, behind its stroke and around its fill, a see-through frame around a bar as wide as half the gap between two bars, or a see-through shell around a solid copy.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `highlight` | yes |  |
| `object` | string | yes | The object id of a molecule, a table, a pattern or a bar plot. |
| `group` | string |  | The group id of one of the molecule's `groups`. Give it for a molecule, `cell` for a table, `copy` for a pattern and `bar` for a bar plot. |
| `cell` | [integer, integer] |  | The cell address of one cell of the table, `[row, column]`, each counted from 0, such as `[2, 1]` for the second cell of the third row. Needs sci_anim 0.18.0 or later. |
| `copy` | [integer] or [integer, integer] or [integer, integer, integer] |  | The copy index of one copy of the pattern `object` to change alone, as `inspect` reports it: `[i, j, k]` for a grid and `[i]` for a ring or an along, each from 0. Needs sci_anim 0.20.0 or later. |
| `bar` | integer |  | The index of one bar of the bar plot `object` to change alone, in category order from 0. Needs sci_anim 0.20.0 or later. |
| `to` | number | yes | The glow share to end at, from 0 (no glow) to 1 (all of it). |
| `color` | string |  | The glow's color, written `#rrggbb`. The default is the color the glow already has, `#ffd23f` at first. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `recolor`

Takes the object's colors to new ones over its span, mixed in linear light: a flat shape's or plot's `stroke` and `fill`, and the `color` of text, math or a solid. Give at least one. A color left out keeps the color it has, except a fill in the stroke's color, as an arrow's tip is, which a recolor of the stroke takes along. A `color` recolors every glyph, those that `\textcolor` colors too. A pattern's colors are its shape's, and each copy still mixes them toward `to_color`, or its pick of `palette`, by its color share. Axes, number lines and molecules cannot be recolored. With `copy` or `bar`, recolors only that copy of a pattern or bar of a bar plot, from the colors it shows when the recolor begins, and the colors it ends at are its own from then on: a later recolor of the whole object leaves it as it is.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `recolor` | yes |  |
| `object` | string | yes | The object id of a flat shape, plot, text, math, solid or pattern. |
| `copy` | [integer] or [integer, integer] or [integer, integer, integer] |  | The copy index of one copy of the pattern `object` to change alone, as `inspect` reports it: `[i, j, k]` for a grid and `[i]` for a ring or an along, each from 0. Needs sci_anim 0.20.0 or later. |
| `bar` | integer |  | The index of one bar of the bar plot `object` to change alone, in category order from 0. Needs sci_anim 0.20.0 or later. |
| `stroke` | string |  | The stroke color to end at, written `#rrggbb`. The object needs a stroke. |
| `fill` | string |  | The fill color to end at, written `#rrggbb`. The object needs a fill. |
| `color` | string |  | The color of text, math or a sphere to end at, written `#rrggbb`. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `camera-move`

Moves the camera's position to `to`. It keeps looking at its target.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `camera-move` | yes |  |
| `to` | [number, number, number] | yes | The camera position to end at, as `[x, y, z]` in world units. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `camera-look`

Turns the camera to look at `to`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `camera-look` | yes |  |
| `to` | [number, number, number] | yes | The camera target to end at, as `[x, y, z]` in world units. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `camera-zoom`

Changes the camera's vertical field of view to `to` degrees. Smaller zooms in.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `camera-zoom` | yes |  |
| `to` | number | yes | The vertical field of view to end at, in degrees, more than 0 and less than 180. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `linear-map`

Carries every point of each object in `objects` by one 2x2 `matrix` about `origin`, in the world's xy plane, in world units with y up, over its span: a point `p` goes to `origin + matrix · (p − origin)`. Every point of every object listed follows the same matrix at every scene time, so a circle and the radii drawn to it stay together throughout, which a morph does not promise. A later linear map acts after an earlier one, so B after A is B·A, and `inspect` reports each named object's `plane_map`. A stroke keeps its declared width under any matrix. An arrow is drawn between its two mapped ends with its tip unsheared, as long as its own scale makes it, and draws nothing while they meet. A dot moves to its mapped center and keeps its size, so it stays visible under the zero matrix. Text, math and axes are mapped like any path, so a shear slants their glyphs. A plot is placed by its axes, so name the axes, and an attached object sits beside the object it is attached to. Solids, molecules, patterns, plots, attached objects and objects turned about x or y when the map begins cannot be named. From the start of the first linear map that names an object, a move, scale, rotate or morph of it is a scene error, and so is one that is still playing when the map begins: the map keeps the object's transform from then on. A fade, recolor, create or write of it is fine.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `linear-map` | yes |  |
| `objects` | array of string | yes | The object ids of the flat shapes, dots, text, math, axes and number lines to carry. At least one, each once. |
| `origin` | [number, number] |  | The map origin, the point the matrix acts about, as `[x, y]` in world units. It stays where it is. The default is `[0, 0]`. |
| `matrix` | [[number, number], [number, number]] | yes | The matrix, row by row: `[[a, b], [c, d]]` sends a point `(x, y)` measured from `origin` to `(a·x + b·y, c·x + d·y)`. Each entry is from -100 to 100. `[[0, -1], [1, 0]]` turns a quarter turn counterclockwise, `[[1, 1], [0, 1]]` shears along x, `[[1, 0], [0, -1]]` reflects across the x axis, and `[[1, 0], [0, 0]]` flattens onto it. |
| `route` | [Matrix route](#matrix-route) |  | How the matrix goes from the identity to all of `matrix` as the progress runs. `entries`, the default, moves each entry straight from the identity's, so a turn shrinks along the chord and a half turn passes through the zero matrix. `turn` turns by the rotation inside the matrix, the shorter way and counterclockwise at exactly half a turn, while it stretches by the rest, so a rotation keeps every length at every frame. `turn` needs a matrix whose determinant is more than 0: no reflection and no flattening. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `simulate`

Integrates `variables` from their starting values by their `rates` over its span, and sets each value in `sets` from expressions in the variables at every scene time of it: a pendulum with drag, a projectile with air resistance, a charging capacitor, predator and prey, an epidemic, or two masses on springs that share their variables. The rates read the variables and `t`, law time, which counts from the span or from the chapter `from` names, as a follow's does. It is integrated once, when the scene file is read, by fourth-order Runge-Kutta at eight steps a frame, over at most 216000 frames, an hour at 60 frames a second. A variable with no finite value at a step is `out_of_range` at its rate, and a set that does not start where its value is, is `law_jump`. Each value it sets is held from its span's end on, and no other animation changes it while the simulation plays. A simulation takes no `easing_curve`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `simulate` | yes |  |
| `variables` | map of name to number | yes | Each variable's name and its value at the span's start, such as `{ "angle": 1.2, "spin": 0 }`. A name is letters, digits and `_`, starts with a letter, and is not `t`, `pi`, `e`, `tau` or a function's name. At least one. |
| `rates` | map of name to string | yes | How fast each variable changes, per second of law time: one expression in the variables and `t` for every variable, by its name, such as `{ "angle": "spin", "spin": "-9.8*sin(angle) - 0.3*spin" }`. |
| `sets` | array of [Set](#set) | yes | The values the simulation sets, each value of an object once. At least one. |
| `from` | string |  | The id of a chapter: law time counts from that chapter's start instead of from this simulation's span. The simulation's own chapter, or one before it. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

### `follow`

Sets a value at every scene time of its span from a law, or carries an object along another object's curve, instead of easing it to a `to`. Give `law` or `along`.

With `law`, `x`, `y` and `z` are expressions in `t`, law time in seconds, or `value` is one for a readout's value or an object opacity, and `sets` names the value, an object's `position` by default. A pendulum is a follow of `rotation`, an orbit a follow of `position`, a pulse a follow of `scale`, and a camera circling a solid a follow of `camera-position`. The law gives the value outright, so at the span's start it must give the value the object or camera already has, within 0.001, or the scene file has a `law_jump` error with both values. The value stays at what the law gives at the span's end, and a later animation starts from there. Law time counts from the span's start, or from the start of the chapter `from` names, so follows in several steps continue one motion. Every expression is evaluated at every frame time of the span, and its end, when the scene file is read. A law takes no `easing_curve`.

With `along`, the follow carries `object` along the path of a flat shape or plot, or the space curve of a tube, by length in that object's own space, over the span and shaped by `easing_curve`, wherever that object is at each scene time. A carry and a create of a plot with the same span and easing curve keep the object on the pen's end. `faces: tangent` also turns the object so its own x axis points along the curve. The object must stand at the curve's start, and face its tangent there when `faces` is set, or the scene file has a `carry_jump` error with the position or rotation to declare.

A follow of an object's position, rotation or scale joins its moves, scales and rotates: no other animation changes the value it sets while it plays, and a plot, an attached object and an object a linear map names cannot be followed. A follow of `opacity` joins its fades the same way, and any object a fade can fade can be followed.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `follow` | yes |  |
| `object` | string |  | The object id of the object whose value the law sets. Leave it out when `sets` is `camera-position` or `camera-target`. |
| `sets` | [Followed](#followed) |  | What the law sets: `position`, the object's position in world units, or in data coordinates for a dot on axes or a sphere on 3D axes, the default; `rotation`, the object's rotation in degrees about its x, its turned y and its turned z axis, as in `rotation`; `scale`, the factor along each of the object's own x, y and z axes, as in `scale`; `opacity`, the object opacity, from the law's one expression, `value`; `value`, a readout's value, from `value` too; `camera-position`, where the camera stands; or `camera-target`, the point the camera looks at. A follow with `along` sets the position, so leave this out. `shape` takes no `law`: it runs the `t` of the object's own expressions, a function plot's `expression`, a tube's `curve`, a surface's `z` or a pattern's `copy` and `along` curve, counting law time over the span or from the chapter `from` names, and holds the shape at the span's end. Its law time must start at the one the shape is drawn at: 0 before its first follow, then where the last follow of it ended. |
| `law` | [Law](#law) |  | The law: `x`, `y` and `z` as expressions in `t`, law time in seconds, or `value` alone for `opacity` and a readout's `value`. A rotation's unused angles are written `"0"`. Give `law` or `along`, not both. |
| `from` | string |  | The id of a chapter: law time counts from that chapter's start instead of from this follow's span, so follows in several steps of a chapter continue one motion. The follow's own chapter, or one before it. Only with `law`. |
| `along` | string |  | The object id of a flat shape, plot or tube to carry `object` along, by length over the span, from the start of its curve to its end. A pattern has many curves, so it cannot be named. It must not be carried itself, attached, or morphed. Give `law` or `along`, not both. |
| `faces` | [Faces](#faces) |  | `tangent` turns the carried object so its own x axis points along the curve at every scene time. Leave it out to keep the object's rotation. Only with `along`. |
| `easing_curve` | [Easing curve](#easing-curve) |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `start` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |
| `end` | number |  | See [Fields every animation takes](#fields-every-animation-takes). |

## Shared types

The types that the fields above refer to.

### Align

How the lines of a text or math object line up, and which point of its box sits at the object's position: the middle of its left edge, its center, or the middle of its right edge. No variant doc comments, for the reason `FillRule` gives.

One of `left`, `center`, `right`.

### Atom pair

One pair of the atom map: the reactant atom and the product atom it becomes.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `reactant` | [Atom ref](#atom-ref) | yes | The atom before the reaction: an atom of one of the reaction's `reactants`. |
| `product` | [Atom ref](#atom-ref) | yes | The atom it becomes: an atom of one of the reaction's `products`, of the same element. |

### Atom ref

An atom named the way an agent finds it: the molecule object's id and the atom's number in that object's structure file.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `object` | string | yes | The object id of a molecule object. |
| `atom` | integer | yes | The atom's number in that object's structure file, counting from 1 in the order the file lists its atoms, or in a PDB file the serial number the file gives it. |

### Atom trace

The atoms an atom trace passes through: in each amino acid or nucleotide of the PDB file `file`, in file order, the atom named `atom_name`. A residue without that atom adds no curve point. Each curve point is the atom's position in angstroms from the molecule's centroid, where a `molecule` object of the same file draws it, so a tube with the molecule's `position`, `rotation` and `scale` runs through those atoms, and its `radius` is in angstroms too. Curve point k is the k-th atom found, so `u` = k/(n - 1) as for `points`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `file` | string | yes | The path of a PDB file, its name ending in `.pdb` or `.ent`, relative to the folder of the scene file, as a molecule's `file` is. sci_anim reads its first model and the first alternate location of each atom. |
| `atom_name` | string | yes | The atom name to pass through in each residue, such as `CA` for an amino acid's alpha carbon or `P` for a nucleotide's phosphorus. Ions and ligands are left out, so a calcium ion, whose atom name is also `CA`, adds nothing. |
| `chain` | string |  | The chain id of the one chain to pass through, such as `A`. The default is every chain in file order, which joins the last residue of one chain to the first of the next. |

### Attach

Where an attached object sits.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `object` | string | yes | The object id of the object to sit beside. It must be placed by its own transform: not a plot, and not attached itself. |
| `group` | string |  | The group id of one of that object's `groups`, when it is a molecule and the attached object sits beside the group. Leave it out to sit beside the object's position. |
| `offset` | [number, number] |  | How far right and up of the object's position, or the group's center, on the frame the attached object's position lies, in world units measured in the plane through the object's position that faces the camera. It is along the screen's axes, not the world's, so the attached object stays beside while the object turns. The default is `[0, 0]`. |

### Axis

One axis of an `axes` object.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `range` | [number, number] | yes | The lowest and highest data value the axis shows, as `[low, high]`, low first. |
| `spacing` | number |  | The distance in data units between ticks, more than 0, giving at most 100 ticks. The default is a round number that gives about five. |
| `label` | string |  | LaTeX math set at the high end of the axis, as in a math object's `tex`. The default is no label. |

### Axis 3d

One axis of a `3d-axes` object.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `range` | [number, number] | yes | The lowest and highest data value the axis shows, as `[low, high]`, low first. |
| `spacing` | number |  | The distance in data units between ticks, more than 0, giving at most 100 ticks. The default is a round number that gives about five. |
| `length` | number | yes | The length of the axis in world units, more than 0, which its range fills. |
| `label` | string |  | LaTeX math set past the high end of the axis, as in a math object's `tex`, such as `x`. The default is no label. |

### Category axis

A category axis: the categories in order, each with a tick and its label in plain text below the axes' box. Category k, counted from 0, is at data x = k and one data unit wide, so a point plot or a data point at x = k stands on it. The y axis stands at the axis' low end. Needs sci_anim 0.16.0 or later.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `categories` | array of string | yes | Each category's label, in order along the axis, set in plain text below its tick, such as `["1950", "1960"]` or `["PE", "PP", "PVC"]`. From 1 to 100 of them. Two neighboring labels nearer than a quarter of `label_size` are a scene error: widen the axes, lower `label_size` or shorten the labels, such as `’50` for 1950. |
| `label` | string |  | LaTeX math set at the high end of the axis, as in a math object's `tex`. The default is no label. |

### Cell

One cell of a table: a string of plain text, `""` for an empty cell, or `{ "tex": "..." }` for LaTeX math.

It is one of these.

- string.
- [Math cell](#math-cell).

### Copy

Each copy's own values, as expressions in its index and place: in a `grid`, `i`, `j` and `k` and `x`, `y` and `z`, and in a `ring` or `along`, `i` and `x`, `y` and `z`. Every expression is evaluated at every copy when the scene file is read.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `position` | [Copy position](#copy-position) |  | The copy's position in the pattern's own space, in world units. A coordinate left out is the place's, so `{ "y": "y + 0.2*sin(x)" }` lifts a row of copies into a wave. |
| `rotation` | [Copy rotation](#copy-rotation) |  | The copy's rotation, in degrees about its own x, y and z axes, turned as an object's `rotation` turns. An angle left out is 0. |
| `scale` | [Copy scale](#copy-scale) |  | The copy's scale along its own x, y and z axes. A factor left out is 1. |
| `color_share` | string |  | An expression in the copy's index and place, which gives how far the copy's colors are mixed toward `to_color`, or toward its pick of `palette`, in linear light, from 0 for the shape's own colors to 1 for that color, such as `i/9`. A value below 0 is 0, and above 1 is 1. Write it with `to_color` or `palette`. The default is 1 with a `palette`, and 0 without. In a `grid` it reads `i`, `j` and `k`, the copy's index along x, y and z from 0, and in a `ring` or `along` it reads `i`, the copy's index from 0. In every layout it reads `x`, `y` and `z`, the copy's place in the pattern's own space. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every copy when the scene file is read, and must have a finite value at each: the first copy where it has none is a scene error, with its index and place. |
| `to_color` | string |  | The color each copy's colors are mixed toward by its `color_share`, written `#rrggbb`. Write it with `color_share`, and not with `palette`. |
| `palette` | array of string |  | The colors each copy picks one of by its `palette_index`, each written `#rrggbb`, at least one. Each copy's colors are mixed toward its pick by its `color_share`, which is 1 unless written, so each copy is drawn in its pick: a palette of four nucleobases' colors colors each rung of a ladder by its nucleobase. Write it with `palette_index`, and not with `to_color`. Needs sci_anim 0.15.0 or later. |
| `palette_index` | [Palette index](#palette-index) |  | Which color of `palette` each copy picks, counting from 0: an expression in the copy's index and place, such as `"mod(i, 4)"`, or a list of one whole number a copy in index order, such as `[0, 1, 2, 3, 2, 1, 0]` for the sequence ATGCGTA of a palette of A, T, G and C. Every copy's index must be a whole number within the palette, and a list must have one entry a copy; either fault is a scene error at the copy or entry. Write it with `palette`. Needs sci_anim 0.15.0 or later. |

### Copy position

A copy's position, a coordinate at a time.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `x` | string |  | An expression in the copy's index and place, which gives the x of the copy's position in world units, in the pattern's own space. The default is the place's x, so `x + 0.2*sin(i)` moves each copy off its place. In a `grid` it reads `i`, `j` and `k`, the copy's index along x, y and z from 0, and in a `ring` or `along` it reads `i`, the copy's index from 0. In every layout it reads `x`, `y` and `z`, the copy's place in the pattern's own space. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every copy when the scene file is read, and must have a finite value at each: the first copy where it has none is a scene error, with its index and place. |
| `y` | string |  | An expression in the copy's index and place, which gives the y of the copy's position in world units, in the pattern's own space. The default is the place's y, so `y + 0.2*sin(i)` moves each copy off its place. In a `grid` it reads `i`, `j` and `k`, the copy's index along x, y and z from 0, and in a `ring` or `along` it reads `i`, the copy's index from 0. In every layout it reads `x`, `y` and `z`, the copy's place in the pattern's own space. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every copy when the scene file is read, and must have a finite value at each: the first copy where it has none is a scene error, with its index and place. |
| `z` | string |  | An expression in the copy's index and place, which gives the z of the copy's position in world units, in the pattern's own space. The default is the place's z, so `z + 0.2*sin(i)` moves each copy off its place. In a `grid` it reads `i`, `j` and `k`, the copy's index along x, y and z from 0, and in a `ring` or `along` it reads `i`, the copy's index from 0. In every layout it reads `x`, `y` and `z`, the copy's place in the pattern's own space. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every copy when the scene file is read, and must have a finite value at each: the first copy where it has none is a scene error, with its index and place. |

### Copy rotation

A copy's rotation, an angle at a time.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `x` | string |  | An expression in the copy's index and place, which gives the copy's angle in degrees about its own x axis, turned as an object's `rotation` turns, such as `30*i`. The default is 0. In a `grid` it reads `i`, `j` and `k`, the copy's index along x, y and z from 0, and in a `ring` or `along` it reads `i`, the copy's index from 0. In every layout it reads `x`, `y` and `z`, the copy's place in the pattern's own space. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every copy when the scene file is read, and must have a finite value at each: the first copy where it has none is a scene error, with its index and place. |
| `y` | string |  | An expression in the copy's index and place, which gives the copy's angle in degrees about its own y axis, turned as an object's `rotation` turns, such as `30*i`. The default is 0. In a `grid` it reads `i`, `j` and `k`, the copy's index along x, y and z from 0, and in a `ring` or `along` it reads `i`, the copy's index from 0. In every layout it reads `x`, `y` and `z`, the copy's place in the pattern's own space. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every copy when the scene file is read, and must have a finite value at each: the first copy where it has none is a scene error, with its index and place. |
| `z` | string |  | An expression in the copy's index and place, which gives the copy's angle in degrees about its own z axis, turned as an object's `rotation` turns, such as `30*i`. The default is 0. In a `grid` it reads `i`, `j` and `k`, the copy's index along x, y and z from 0, and in a `ring` or `along` it reads `i`, the copy's index from 0. In every layout it reads `x`, `y` and `z`, the copy's place in the pattern's own space. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every copy when the scene file is read, and must have a finite value at each: the first copy where it has none is a scene error, with its index and place. |

### Copy scale

A copy's scale, a factor at a time.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `x` | string |  | An expression in the copy's index and place, which gives the copy's factor along its own x axis, such as `1 + 0.1*i`. A copy's scale and the pattern's size a flat copy's stroke with the rest of it. The default is 1. In a `grid` it reads `i`, `j` and `k`, the copy's index along x, y and z from 0, and in a `ring` or `along` it reads `i`, the copy's index from 0. In every layout it reads `x`, `y` and `z`, the copy's place in the pattern's own space. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every copy when the scene file is read, and must have a finite value at each: the first copy where it has none is a scene error, with its index and place. |
| `y` | string |  | An expression in the copy's index and place, which gives the copy's factor along its own y axis, such as `1 + 0.1*i`. A copy's scale and the pattern's size a flat copy's stroke with the rest of it. The default is 1. In a `grid` it reads `i`, `j` and `k`, the copy's index along x, y and z from 0, and in a `ring` or `along` it reads `i`, the copy's index from 0. In every layout it reads `x`, `y` and `z`, the copy's place in the pattern's own space. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every copy when the scene file is read, and must have a finite value at each: the first copy where it has none is a scene error, with its index and place. |
| `z` | string |  | An expression in the copy's index and place, which gives the copy's factor along its own z axis, such as `1 + 0.1*i`. A copy's scale and the pattern's size a flat copy's stroke with the rest of it. The default is 1. In a `grid` it reads `i`, `j` and `k`, the copy's index along x, y and z from 0, and in a `ring` or `along` it reads `i`, the copy's index from 0. In every layout it reads `x`, `y` and `z`, the copy's place in the pattern's own space. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every copy when the scene file is read, and must have a finite value at each: the first copy where it has none is a scene error, with its index and place. |

### Curve coordinate

One coordinate of a curve point, from -10000 to 10000 world units.

Written as number.

### Curve point

One curve point, `[x, y, z]` in world units, each coordinate from -10000 to 10000.

Written as [[Curve coordinate](#curve-coordinate), [Curve coordinate](#curve-coordinate), [Curve coordinate](#curve-coordinate)].

### Dash pattern

A dash pattern: a dash's length, then the length of the gap after it, in world units, each from 0.01 to 100.

Written as [number, number].

### Easing curve

Turns the share of a step's duration that has passed into progress. `linear` moves at a constant speed, `slow-start` and `slow-end` ease one end, `slow-both` eases both, and `overshoot` passes its end value and settles back. The default is `slow-both`.

One of `linear`, `slow-start`, `slow-end`, `slow-both`, `overshoot`.

### Edge shade

How a solid's edges are drawn: a darker shade of its color, or a lighter one for a dark solid whose dark edges would vanish against the background. No variant doc comments, for the reason `FillRule` gives.

One of `dark`, `light`.

### End

Where a line, an arrow or a spring starts or stops: a point, an end on an object, an end on a tube's curve at `u`, an end on a plot's curve at `x`, or an end on an atom of a molecule.

It is one of these.

- [number, number] or [number, number, number]. A point, `[x, y]` or, on a shape placed by its ends, `[x, y, z]`.
- [On curve](#on-curve).
- [On plot](#on-plot).
- [On atom](#on-atom).
- [On object](#on-object).

### Faces

How a carried object turns as it goes, or each copy of an `along` layout turns at its place. Leaving `faces` out keeps the rotation it has. `tangent` turns the object so its own x axis points along the curve's tangent, toward the curve's end, and its own z axis stays square to the plane of a flat curve, or along the third direction of a space curve's frame, which starts as near the curve's own +z as it can and never twists about the curve. A copy's own rotation then turns it further, about its turned axes.

One of `tangent`.

### Fill

The inside of a flat shape.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `color` | string | yes | The fill's color, written `#rrggbb`. |
| `opacity` | number |  | How much the fill hides what is behind it, from 0 (clear) to 1 (solid). The default is 1. |
| `rule` | [Fill rule](#fill-rule) |  | Which parts of a path that crosses itself, or has parts inside parts, are filled: `non-zero` or `even-odd`. The default is `non-zero`. |

### Fill rule

How a fill decides what is inside a path that crosses itself or has parts inside parts. `non-zero`, the default, fills where the path winds around a point, so a part inside another is a hole only when it runs the other way round. `even-odd` fills where an odd number of parts enclose a point, so a part inside another is always a hole. The variants carry no doc comments because schemars would then write a `oneOf`, which hides the list of values from the error an agent gets.

One of `non-zero`, `even-odd`.

### Followed

The value a follow's law sets.

One of `position`, `rotation`, `camera-position`, `camera-target`, `shape`, `value`, `scale`, `opacity`.

### Font

The typeface a text object is set in: Libertinus Sans or Libertinus Serif. Math is always set in New Computer Modern Math. No variant doc comments, for the reason `FillRule` gives.

One of `sans`, `serif`.

### Group

The atoms of one group: their atom numbers, or the residues of a PDB file that hold them.

It is one of these.

- array of integer. The atom numbers of the group's atoms: at least one, each 1 or more, as the structure file numbers them, such as `[1, 9]`.
- [Residues](#residues).

### Law

A law: three expressions in `t`, law time in seconds, one for each coordinate of the value a follow sets, such as `{ "x": "0", "y": "0", "z": "30*cos(2.21*t)" }` for a pendulum's swing, or for a readout's value or an object opacity the one expression `value`, such as `{ "value": "9.8*t" }`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `x` | string |  | An expression in the one variable `t`, law time in seconds, such as `30*cos(2.21*t)`, which gives the x of the value the follow sets: a position's x in world units, a rotation's angle in degrees about the object's x axis, as in `rotation`, or a scale's factor along the object's own x axis, as in `scale`. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every frame time of the follow's span when the scene file is read, and must have a finite value at each, and a position's must stay within 10000. |
| `y` | string |  | An expression in the one variable `t`, law time in seconds, such as `30*cos(2.21*t)`, which gives the y of the value the follow sets: a position's y in world units, a rotation's angle in degrees about the object's y axis, as in `rotation`, or a scale's factor along the object's own y axis, as in `scale`. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every frame time of the follow's span when the scene file is read, and must have a finite value at each, and a position's must stay within 10000. |
| `z` | string |  | An expression in the one variable `t`, law time in seconds, such as `30*cos(2.21*t)`, which gives the z of the value the follow sets: a position's z in world units, a rotation's angle in degrees about the object's z axis, as in `rotation`, or a scale's factor along the object's own z axis, as in `scale`. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every frame time of the follow's span when the scene file is read, and must have a finite value at each, and a position's must stay within 10000. |
| `value` | string |  | An expression in the one variable `t`, law time in seconds, such as `9.8*t`, which gives a readout's value, for a follow of `value`, or the object opacity, for a follow of `opacity`, such as `1 - t/2`. Such a law has no `x`, `y` or `z`. It is evaluated at every frame time of the follow's span when the scene file is read, and must have a finite value at each, and an object opacity's must lie within 0 to 1 at each, or the scene file has an `out_of_range` error. Between frame times an opacity is held within 0 to 1. |

### Layout

Where a pattern's copies go, in the pattern's own space. Each copy has an index, counted from 0, and a place, the point the layout puts it at, which is its position unless `copy` moves it.

#### `grid` layout

`nx` copies along x by `ny` along y by `nz` along z, `spacing` apart and centered on the pattern's position. Copy `i`, `j`, `k` has the place `((i - (nx-1)/2)*spacing, (j - (ny-1)/2)*spacing, (k - (nz-1)/2)*spacing)`, so `[3, 1, 1]` at spacing 1 places copies at x of -1, 0 and 1. The copies are in index order with `i` fastest, then `j`, then `k`, the order a create grows them in.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `grid` | yes |  |
| `counts` | [integer, integer, integer] | yes | How many copies along x, y and z, as `[nx, ny, nz]`, each 1 or more. Their product is at most 10000. `[5, 1, 1]` is a row, `[5, 5, 1]` a square in the xy plane, and `[5, 5, 5]` a cube. |
| `spacing` | number | yes | The distance between neighboring copies along each axis, in world units, more than 0. |

#### `ring` layout

`count` copies evenly around a circle of `radius` in the pattern's xy plane, centered on its position. Copy `i` has the place at `start_angle + 360*i/count` degrees counterclockwise from +x, as an arc's angles run.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `ring` | yes |  |
| `count` | integer | yes | How many copies, from 1 to 10000. |
| `radius` | number | yes | The circle's radius in world units, more than 0. |
| `start_angle` | number |  | Where copy 0 sits, in degrees counterclockwise from +x. The default is 0. |

#### `along` layout

`count` copies along a space curve, evenly spaced by length. On an open curve the first is at its start and the last at its end; on a closed curve the last stops one spacing short of the start, so no two copies meet. One copy sits at the start. To set copies along a tube, write the tube's `curve` here too. With `faces`, each copy turns to face along the curve.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `along` | yes |  |
| `count` | integer | yes | How many copies, from 1 to 10000. |
| `curve` | [Space curve](#space-curve) | yes | The curve: x, y and z as expressions in `u`, or the points it passes through, in world units from the pattern's position, as a tube's `curve` is written. |
| `faces` | [Faces](#faces) |  | `tangent` turns each copy so its own x axis points along the curve at its place, toward the curve's end, as a carry's `faces` turns its object. Its own z axis starts as near the pattern's own +z as it can, or +y on a curve that starts along z, and the copies never twist about the curve, so along a curve in the pattern's xy plane every copy's z axis is +z. The copy's `rotation` then turns it further, about its turned axes, so a copy can tilt from its strand. Leave it out for copies turned only by their `rotation`. |

### Material

How a surface looks under light. A solid, such as a sphere, a prism or a molecule, is lit by a key light from the camera's upper left, so its side toward that light shows exactly its color and its far side is darker. A flat shape is lit from the camera, so it shows exactly its color where it faces the camera.

#### `standard` material

A surface with a color and how rough and how metallic it is.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `standard` | yes |  |
| `color` | string | yes | The surface's color, written `#rrggbb`. |
| `roughness` | number |  | From 0, a glossy surface with a sharp highlight, to 1, a matte one. The default is 0.5. |
| `metallic` | number |  | From 0, a plastic-like surface, to 1, a metal one. The default is 0. |
| `opacity` | number |  | How much the surface hides what is behind it, from 0 (clear) to 1 (solid). A see-through surface shows its own far side. To fade the whole object, use the object's `opacity` instead. The default is 1. |

### Math cell

A cell of LaTeX math.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `tex` | string | yes | LaTeX math, set as a `math` object's `tex` is, such as `\\frac{1}{2}`. |

### Matrix route

How a linear map's matrix goes from the identity to all of `matrix` as its progress runs: the scene file's values of `route`. No variant doc comments, for the reason `FillRule` gives.

One of `entries`, `turn`.

### On atom

An end on an atom's center, named by an atom reference as an atom map names it, as `{ "object": "at_pair", "atom": 13 }`. It goes with the molecule as it moves, turns and scales, so a hydrogen bond stays on its two atoms. A molecule that a reaction uses up hides the shape when the reaction begins, and one that a reaction makes shows it from the reaction's end. Needs sci_anim 0.14.0 or later.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `object` | string | yes | The object id of the molecule the end sits on. |
| `atom` | integer | yes | The atom's number in that molecule's structure file, counting from 1 in the order the file lists its atoms, or in a PDB file the serial number the file gives it, as an atom map numbers it. |

### On curve

An end on a tube's space curve, at the curve's point at `u`. It follows the curve as the tube moves, turns and scales, and as its shape law runs. Needs sci_anim 0.13.0 or later.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `object` | string | yes | The object id of the tube the end sits on. It may stand on 3D axes, but must not be a spring, which is placed by its own ends. |
| `u` | number | yes | The u of the curve's point, within the curve's `u_range`, or from 0 at the first curve point to 1 at the last for a curve through `points`, so the end keeps to the point at that u wherever the curve goes. |

### On object

An end on an object: the object's world position, where the frame draws its center, plus `offset`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `object` | string | yes | The object id of the object the end sits on. It must be placed by its own transform or be a data point: not a plot, an attached object or another shape placed by its ends. |
| `offset` | [number, number, number] |  | How far from the object's position the end lies, as `[x, y, z]` in world units along the world's axes, so it does not turn with the object. The default is `[0, 0, 0]`. |

### On plot

An end on a plot's curve, at its first point at `x`, in its axes' data coordinates. It goes with the axes and follows the curve as its shape law runs; where the curve is cut, it waits at the curve's point nearest that x. Needs sci_anim 0.13.0 or later.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `object` | string | yes | The object id of the function plot or point plot the end sits on. |
| `x` | number | yes | The x in the axes' data coordinates, within their x range. |

### Palette index

Which color of a pattern's palette each copy picks: an expression or a list.

It is one of these.

- string. An expression in the copy's index and place, which gives which color of `palette` the copy picks, counting from 0, such as `mod(i, 4)`. Its value at each copy must be a whole number from 0 to one less than the palette's length. In a `grid` it reads `i`, `j` and `k`, the copy's index along x, y and z from 0, and in a `ring` or `along` it reads `i`, the copy's index from 0. In every layout it reads `x`, `y` and `z`, the copy's place in the pattern's own space. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It is evaluated at every copy when the scene file is read, and must have a finite value at each: the first copy where it has none is a scene error, with its index and place.
- array of integer. One palette index a copy, in index order: a grid's `i` fastest, then `j`, then `k`.

### Region

The region of a prism's base or a revolution's profile: what a circle's, polygon's or path's fill would fill. It has no stroke, fill or position of its own, and its coordinates are in world units from the object's position.

#### `circle` region

A disc of `radius` about `center`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `circle` | yes |  |
| `radius` | number | yes | The disc's radius in world units, more than 0. |
| `center` | [number, number] |  | The disc's center, as `[x, y]` in world units. The default is `[0, 0]`. |

#### `polygon` region

The inside of the closed path through `points` in order.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `polygon` | yes |  |
| `points` | array of [number, number] | yes | The corners in order, each `[x, y]` in world units. At least three. |

#### `path` region

The inside of any path in SVG path syntax, each part closed back to its start.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `type` | `path` | yes |  |
| `d` | string | yes | SVG path data, as a `path` object's `d`. Unlike SVG, y points up. |
| `rule` | [Fill rule](#fill-rule) |  | `non-zero` or `even-odd`, as a fill's `rule`: under `even-odd` a part inside another is a hole. The default is `non-zero`. |

### Representation

How a molecule is drawn. The values of the scene file's `representation`.

One of `ball-and-stick`, `space-filling`, `cartoon`.

### Residue part

The values of a residue pick's `part`.

One of `nucleobase`.

### Residue pick

One residue, a run of residues, or a whole chain, of a PDB file.

It is one of these.

An object with the fields below. The residue of residue number `number` in chain `chain`, as `{ "chain": "A", "number": 4 }`. A number with insertion codes, as `52` and `52A`, names every residue of that number.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `chain` | string | yes | The chain id, the one letter the PDB file names the chain by. |
| `number` | integer | yes | The residue number, as the PDB file gives it. |

An object with the fields below. The residues of chain `chain` numbered `from` to `to`, both included, as `{ "chain": "A", "from": 23, "to": 34 }`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `chain` | string | yes | The chain id, the one letter the PDB file names the chain by. |
| `from` | integer | yes | The residue number the run starts at. The chain has a residue of this number. |
| `to` | integer | yes | The residue number the run ends at, `from` or more. The chain has a residue of this number. |

An object with the fields below. Every residue of chain `chain`, as `{ "chain": "B" }`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `chain` | string | yes | The chain id, the one letter the PDB file names the chain by. |

### Residues

The atoms of some residues of a PDB file, such as one helix, `{ "residues": [{ "chain": "A", "from": 23, "to": 34 }] }`, or one nucleobase pair, the nucleobases of two nucleotides across from each other, `{ "residues": [{ "chain": "A", "number": 4 }, { "chain": "B", "number": 21 }], "part": "nucleobase" }`. `inspect` lists each chain's residues. Needs sci_anim 0.17.0 or later.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `residues` | array of [Residue pick](#residue-pick) | yes | The residues, each by its chain id and residue number, or a run of them, or a whole chain. At least one. |
| `part` | [Residue part](#residue-part) |  | Which atoms of each residue the group holds. Leave it out for all of them. `nucleobase` holds only a nucleotide's nucleobase, without its sugar and phosphate, and every residue named must then be a nucleotide. |

### Rulings

Which rulings a table draws: between every row and column and around the table, once under the first row, or none. No variant doc comments, for the reason `FillRule` gives.

One of `all`, `header`, `none`.

### Set

One value a simulation sets: an object's `position` or `rotation` from three expressions, or a readout's `value` from one. Give exactly one of the three.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `object` | string | yes | The object id of the object whose value this sets. |
| `position` | [string, string, string] |  | The object's position as three expressions, x, y and z, in the simulation's variables and `t`, in world units, or in data coordinates for a data point, such as `["2*sin(angle)", "-2*cos(angle)", "0"]`. It must start where the object is. |
| `rotation` | [string, string, string] |  | The object's rotation as three expressions, angles in degrees about its x, y and z axes, in the simulation's variables and `t`, such as `["0", "0", "angle*180/pi"]`. It must start where the object is turned. |
| `value` | string |  | A readout's value as one expression in the simulation's variables and `t`, such as `abs(2*spin)`. It must start at the readout's value. |

### Shape

The file form of a `Shape`. Every flat kind names its own dimensions, in the object's xy plane and about the object's position, and becomes one `Path` when the file is read.

#### `circle` shape

The same as [`circle`](#circle), without the [fields every object takes](#fields-every-object-takes).

#### `dot` shape

The same as [`dot`](#dot), without the [fields every object takes](#fields-every-object-takes).

#### `sphere` shape

The same as [`sphere`](#sphere), without the [fields every object takes](#fields-every-object-takes).

#### `prism` shape

The same as [`prism`](#prism), without the [fields every object takes](#fields-every-object-takes).

#### `revolution` shape

The same as [`revolution`](#revolution), without the [fields every object takes](#fields-every-object-takes).

#### `tube` shape

The same as [`tube`](#tube), without the [fields every object takes](#fields-every-object-takes).

#### `surface` shape

The same as [`surface`](#surface), without the [fields every object takes](#fields-every-object-takes).

#### `line` shape

The same as [`line`](#line), without the [fields every object takes](#fields-every-object-takes).

#### `arrow` shape

The same as [`arrow`](#arrow), without the [fields every object takes](#fields-every-object-takes).

#### `spring` shape

The same as [`spring`](#spring), without the [fields every object takes](#fields-every-object-takes).

#### `trail` shape

The same as [`trail`](#trail), without the [fields every object takes](#fields-every-object-takes).

#### `readout` shape

The same as [`readout`](#readout), without the [fields every object takes](#fields-every-object-takes).

#### `polygon` shape

The same as [`polygon`](#polygon), without the [fields every object takes](#fields-every-object-takes).

#### `arc` shape

The same as [`arc`](#arc), without the [fields every object takes](#fields-every-object-takes).

#### `path` shape

The same as [`path`](#path), without the [fields every object takes](#fields-every-object-takes).

#### `text` shape

The same as [`text`](#text), without the [fields every object takes](#fields-every-object-takes).

#### `math` shape

The same as [`math`](#math), without the [fields every object takes](#fields-every-object-takes).

#### `table` shape

The same as [`table`](#table), without the [fields every object takes](#fields-every-object-takes).

#### `axes` shape

The same as [`axes`](#axes), without the [fields every object takes](#fields-every-object-takes).

#### `3d-axes` shape

The same as [`3d-axes`](#3d-axes), without the [fields every object takes](#fields-every-object-takes).

#### `number-line` shape

The same as [`number-line`](#number-line), without the [fields every object takes](#fields-every-object-takes).

#### `function-plot` shape

The same as [`function-plot`](#function-plot), without the [fields every object takes](#fields-every-object-takes).

#### `point-plot` shape

The same as [`point-plot`](#point-plot), without the [fields every object takes](#fields-every-object-takes).

#### `bar-plot` shape

The same as [`bar-plot`](#bar-plot), without the [fields every object takes](#fields-every-object-takes).

#### `molecule` shape

The same as [`molecule`](#molecule), without the [fields every object takes](#fields-every-object-takes).

#### `pattern` shape

The same as [`pattern`](#pattern), without the [fields every object takes](#fields-every-object-takes).

### Sound

One sound file and the chapter it starts at.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `file` | string | yes | The sound file's path, relative to the scene file, in any format ffmpeg reads, such as MP3 or WAV. sci_anim never fetches one, so it must be on disk. |
| `at` | string | yes | The `id` of the chapter the sound starts at. |

### Space curve

A curve through space, sampled finer where it bends: its x, y and z as expressions in `u`, or the `points` it passes through, or the atom `trace` of a PDB file. A helix is `{ "x": "cos(u)", "y": "0.1*u", "z": "sin(u)", "u_range": [0, 25] }`, a circle in the xz plane is `{ "x": "2*cos(u)", "y": "0", "z": "2*sin(u)", "u_range": [0, 6.2832] }`, and a chain through three beads is `{ "points": [[0, 0, 0], [1, 0.5, 0], [2, 0, 0.5]] }`.

It is one of these.

An object with the fields below. x, y and z as expressions in `u` over `u_range`. Each expression must have a finite value within 10000 of the object's position at every u of `u_range`.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `x` | string | yes | An expression in `u`, such as `cos(u)`, which gives the curve's x in world units, from the object's position, for each u of `u_range`. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It must have a finite value at every u of `u_range`: `sqrt(u)` over a range below 0 is a scene error, with the u where the value is missing. |
| `y` | string | yes | An expression in `u`, such as `cos(u)`, which gives the curve's y in world units, from the object's position, for each u of `u_range`. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It must have a finite value at every u of `u_range`: `sqrt(u)` over a range below 0 is a scene error, with the u where the value is missing. |
| `z` | string | yes | An expression in `u`, such as `cos(u)`, which gives the curve's z in world units, from the object's position, for each u of `u_range`. It may also read `t`, law time in seconds, so the shape changes over time while a `follow` with `"sets": "shape"` runs it, and is evaluated again at every frame time of that follow's span when the scene file is read; before that follow it is drawn at `t` of 0. The operators are `+`, `-`, `*`, `/` and `^` for a power, with parentheses. `^` binds tightest and groups to the right, so `-a^2` is `-(a^2)`. A product needs its `*`: write `2*a`, not `2a`. The constants are `pi`, `e` and `tau`. The functions of one argument are `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `sinh`, `cosh`, `tanh`, `exp`, `ln`, `log2`, `log10`, `sqrt`, `cbrt`, `abs`, `floor`, `ceil`, `sign` and `step`, and `min`, `max`, `atan2` and `mod` take two, as in `max(a, 0)`. `ln` is the natural logarithm, and there is no `log`. `atan2(y, x)` is the angle of the point (x, y) from the +x axis, from -pi to pi. `mod(a, b)` is what is left of a after taking away whole b's, with the sign of b, so `mod(-1, 3)` is 2. `step(a)` is 1 where a is 0 or more and 0 below it. Angles are in radians. At most 500 characters. It must have a finite value at every u of `u_range`: `sqrt(u)` over a range below 0 is a scene error, with the u where the value is missing. |
| `u_range` | [number, number] | yes | The lowest and highest u, as `[low, high]`, low first. The curve starts at low. |

An object with the fields below. The curve points it passes through, in order, smooth between them, as a scene generator writes a random coil or a backbone from real coordinates. Needs sci_anim 0.12.0 or later.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `points` | array of [Curve point](#curve-point) | yes | The curve points, each `[x, y, z]` in world units from the object's position, from 2 to 10000 of them, each coordinate within 10000. The curve passes through each in order and is centripetal Catmull-Rom between each two, so it never loops or overshoots between them, and an open curve leaves its first point toward the second and reaches its last from the one before. `u` runs from 0 at the first point to 1 at the last, the k-th of n points, counted from 0, at u = k/(n - 1), so a point keeps its u however far apart the points lie. A point equal to the one before it adds nothing to the curve. A last point within a thousandth of the curve's size of the first, with at least two points between them, closes the curve, which then runs smoothly through it with no ends. |

An object with the fields below. An atom trace: the curve through one atom of each amino acid or nucleotide of a PDB file, its curve points read as a `points` curve's are, such as a protein's alpha carbons, `{ "trace": { "file": "structures/1ubq.pdb", "atom_name": "CA" } }`. Needs sci_anim 0.19.0 or later.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `trace` | [Atom trace](#atom-trace) | yes | Which PDB file, which atom name and which chain the atom trace reads. |

### Stroke

The outline of a flat shape. Its corners are mitered, a corner sharper than about 54 degrees is beveled, and its ends are cut square. It may be dashed. The default, for a line or an arrow that sets none, is white and 0.04 wide.

| Field | Type | Required | Description |
| --- | --- | --- | --- |
| `color` | string | yes | The stroke's color, written `#rrggbb`. |
| `width` | number |  | The stroke's width in world units, more than 0. The object's scale does not change it. The default is 0.04. |
| `dash` | [Dash pattern](#dash-pattern) |  | A dash pattern, `[dash, gap]`: the stroke is drawn as dashes `dash` long with gaps `gap` long between them, in world units along the stroke, measured as its width is. The dashes start at the start of each part of the path, so a line placed by its ends keeps the length of its dashes as it stretches, and the last dash is cut where the stroke ends. An arrow's tip is drawn whole. A trail's and 3D axes' strokes are not dashed. Needs sci_anim 0.14.0 or later. The default is none: one unbroken stroke. |

### X axis

The x axis of an `axes` object: a `range` of numbers, or `categories`, which a bar plot stands on.

It is one of these.

- [Axis](#axis).
- [Category axis](#category-axis).
