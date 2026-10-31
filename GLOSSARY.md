# sci_anim

sci_anim turns a scene file into an animation of math or science, such as a plotted function or a molecule reacting. An agent writes the scene file and drives the command-line tool.

## Language

### Scenes

**Scene**:
The complete description of one animation. It holds the scene config, the camera, the objects and the timeline.
_Avoid_: project (for a scene; a project is a repository, as in project level), composition, script, construct

**Scene file**:
The file a scene is written in. It is data, not a program.
_Avoid_: script, manifest, config file

**Scene config**:
The settings that apply to the whole scene, such as resolution, frame rate and background.
_Avoid_: settings, options, config (alone)

**Background**:
What lies behind every object in a frame: an opaque color, or nothing when the frame is transparent.
_Avoid_: backdrop

**Stage**:
The camera and objects of a scene as they stand at one scene time. A scene declares its start stage, and the timeline changes it.
_Avoid_: state, scene graph

**Agent**:
The AI program that writes scene files and runs the tool. A person can do the same things, but the tool is designed for an agent first.
_Avoid_: client, bot, user (when the agent is meant)

### Objects

**Object**:
A named thing in a scene that can be seen and animated, such as a circle, an equation, a set of axes or a molecule.
_Avoid_: mobject, entity, node, item, element (an element is chemical)

**Object id**:
The name a scene gives an object. The timeline refers to objects by their object ids.
_Avoid_: name (for an object's id), key, label (a label is visible text)

**Shape**:
What kind of thing an object is, with its dimensions, such as a circle with a radius. An object is a shape placed by a transform.
_Avoid_: geometry, primitive, kind (for a shape)

**Transform**:
An object's position, rotation and scale.
_Avoid_: pose, placement, transform (for changing one shape into another, which is a morph), transform (for what a linear map does, which changes a plane map)

**Material**:
How an object's surface looks under light, such as its color, roughness, metalness and opacity.
_Avoid_: shader, texture, style

**Object opacity**:
How much an object shows as a whole, from 0 (hidden) to 1 (shown). An object between the two shows as one faded picture of itself: only its nearest surface shows, unlike a see-through material, whose own far side shows through it.
_Avoid_: alpha, visibility, transparency

**Flat shape**:
A shape with no depth: a path in the object's own plane, drawn as a stroke along the path, a fill of its inside, or both. A circle, dot, line, arrow, polygon, arc, path, text and math are flat shapes. A sphere is not.
_Avoid_: 2D shape, vector shape, vmobject, figure

**Path**:
The curve of a flat shape in its own plane, made of straight and Bézier pieces, open or closed, in one or more parts. Every flat shape becomes one path when the scene file is read, and the stroke follows it and the fill fills it. A `path` shape writes its path in SVG path syntax.
_Avoid_: outline (the stroke is the outline), contour, polyline, bezpath, geometry

**Line**:
A flat shape that is a straight stroke between two points in its plane, or between its two ends.
_Avoid_: stroke (a stroke is the outline of any flat shape), connector

**Arrow**:
A line with a filled tip at its `to` end, both in its stroke's color. The tip is part of the arrow's path, so a create draws the shaft and then the tip. Under a plane map an arrow is drawn between its two mapped ends with its tip unsheared, as long as its own scale makes it, or the geometric mean of its x and y scale when they differ, and draws nothing while they meet.
_Avoid_: head, arrowhead

**End**:
Where a line, an arrow or a spring starts or stops: a point; a point on an object, at that object's world position plus an offset in world units along the world's axes; the point of a tube's space curve at a `u` within its u range; the first point of a plot's curve at an `x` of its axes; or an atom's center, named by an atom reference. An end on a curve keeps to that u or x as the curve's object moves, turns and scales and as its shape law runs, so a cross-link stays on the two chains it joins. An end on an atom goes with its molecule the same way, so a hydrogen bond stays on its two atoms; it is hidden while a reaction uses its molecule up, and one on a product shows from the reaction's end. A shape with an end on an object, and every spring, is placed by its ends: at every scene time it is drawn again between them, wherever their objects are, so an arrow keeps its tip's length and a spring its turns. It has no position, rotation or scale of its own, and is hidden while the object either end sits on is. A force arrow on a sliding block has both ends on the block. `inspect` reports each end's world point.
_Avoid_: anchor, endpoint, terminal (for an end; a terminal is also where a person types), pin, connector, attachment (an attachment places text beside an object)

**Polygon**:
A flat shape that is a closed path of straight pieces through its points in order. A triangle, a rectangle and a square are polygons with three or four points.
_Avoid_: poly, polyline

**Arc**:
A flat shape that is part of a circle: from a start angle, sweeping an angle counterclockwise when positive. Its fill, if it has one, fills the region between the arc and its chord.
_Avoid_: sector (which would also fill to the center), circle segment

**Dot**:
A flat shape that is a filled disc of `radius` about its position, which marks a point: the tip of a vector, a map origin, a point on a curve. Its own scale and rotation reshape it as a circle's would, but a plane map moves only its center, so it stays on the frame when a matrix flattens everything around it. It has a fill and no stroke, so it is faded in, not created. `inspect` reports its world `center`. A dot that names axes is a data point.
_Avoid_: marker, marked point

**Text**:
A flat shape that is a string of plain characters typeset in its font, line by line, as glyphs all filled in the object's color. Nothing in it is markup.
_Avoid_: caption (a caption is a text or math object with an attachment), textbox, typography

**Font**:
The typeface a text object is set in: `sans`, the default, is Libertinus Sans, and `serif` is Libertinus Serif. Math is always set in New Computer Modern Math. Every font is bundled into the tool, and none is read from the system.
_Avoid_: typeface, font family, font face

**Math**:
A flat shape that is a formula written in LaTeX math syntax in its `tex` field, typeset in display style in New Computer Modern Math as glyphs. A glyph that `\textcolor` or `\color` colors is filled in that color, and every other glyph in the object's color.
_Avoid_: equation (alone, for the object type), formula (for the object type), MathTex, LaTeX object

**Readout**:
An object that shows a number typeset as math, to its `decimals`, with a `unit` after it, such as a speed that counts up as a ball falls. Its value changes as a track does: a count eases it, a follow sets it from a law, and a simulation sets it. The number is lined up on its decimal point, so its digits keep their places as it grows, and on one baseline, whatever its digits. It is written, faded, attached and recolored as math is.
_Avoid_: counter, number display, DecimalNumber, label (a label is visible text of axes), value tracker, ValueTracker

**Glyph**:
One drawn mark of a text or math object, with its own path filled in its own color under the `non-zero` fill rule: a character's outline, a ligature of several characters such as `ffi`, or a rule such as a fraction bar or the bar of a root. A space has no glyph. Glyphs are in the order Typst lays them out, which is the order a write draws them in. Each glyph keeps the characters it shows, none for a rule, and a morph between two text or math objects pairs glyphs by them.
_Avoid_: character (one glyph can show several, as a ligature does), letter, symbol (for a glyph), submobject

**Typesetting**:
Turning a text or math object's string into its glyphs, placed in the object's plane. It happens once, when the scene file is read, with the bundled fonts, and for math after mitex has turned the LaTeX into Typst math. A string that cannot be typeset is a scene error at its field.
_Avoid_: shaping, rendering (for text), compiling (for text)

**Text size**:
The height of one em of a text or math object's font, in world units. The field is `size`.
_Avoid_: font size, font_size

**Alignment**:
How a text or math object's lines line up with each other, and which point of its box sits at its position: the middle of the left edge, the center, or the middle of the right edge. The field is `align`.
_Avoid_: anchor, justification, text-anchor

**Stroke**:
The outline of a flat shape, with its color and width. Its corners are mitered, except that a corner sharper than about 54 degrees is beveled, and its ends are cut square. Its width is in world units, and neither an object's scale nor its plane map widens it, though either reshapes the path under it. A pattern is the exception: it draws its copies as one picture, so its scale and each copy's scale size a copy's stroke with the rest of it. A stroke may have a dash pattern.
_Avoid_: border, line (a line is an object)

**Dash pattern**:
How a stroke is broken into dashes: a dash's length and the gap's after it, in world units along the stroke, measured as its width is. Each part of the path starts with a dash, so a line placed by its ends keeps its dashes' length as it stretches and the last dash is cut where the stroke ends. An arrow's tip is drawn whole. A hydrogen bond is a dashed line. The field is the stroke's `dash`.
_Avoid_: dasharray, dotted, stipple, line style

**Fill**:
The inside of a flat shape, with its color, opacity and fill rule.
_Avoid_: background (the scene config has a background)

**Fill rule**:
How a fill decides what is inside a path that crosses itself or has parts inside parts. `non-zero`, the default, fills where the path winds around a point. `even-odd` fills where an odd number of parts enclose a point, so a part inside another is a hole.
_Avoid_: winding rule, fill-rule, clip rule

**Camera**:
The viewpoint the scene is seen from. Every scene has one camera, and the timeline can animate it. A flat scene is a scene whose camera looks straight at objects that have no depth.
_Avoid_: view, viewport, eye

### Solids

**Solid**:
A shape with depth, drawn as lit faces in its material. A prism, a revolution, a tube, a surface and a sphere are solids.
_Avoid_: mesh (a mesh is triangles), body, model (for a solid), 3D shape, primitive, volume

**Prism**:
A solid made by pushing a region, its base, straight up by a height, with a face the shape of the base at each end. A box is a prism on a square, and a cylinder a prism on a circle.
_Avoid_: extrusion, extrude, slab, box (for the shape type), depth (depth is distance from the camera)

**Region**:
The inside of a circle, polygon or path, as its fill would fill it under its fill rule: a prism's base or a revolution's profile. A path with a hole under `even-odd` is a region with a hole.
_Avoid_: area (an area is a size), footprint, shape (alone)

**Base**:
The region a prism stands on and pushes up, written as a circle, polygon or path.
_Avoid_: footprint, plan (for a base), profile (a profile is a revolution's), base (in chemistry, an acid's partner or a nucleobase)

**Revolution**:
A solid made by turning a region, its profile, around the object's y axis through an angle, a full circle by default. A cone is the revolution of a right triangle, and a torus of a circle drawn beside the axis.
_Avoid_: lathe, turned part, spin, solid of rotation, revolution (in astronomy, an orbit, which a follow draws)

**Profile**:
The region a revolution turns around its axis, written as a circle, polygon or path on one side of the axis: half of the solid's outline.
_Avoid_: outline, silhouette, base (a base is a prism's)

**Space curve**:
A curve through space, written as three expressions in `u` for its x, y and z over a range of `u`, its `u_range`, or as the curve points it passes through in order, which a scene generator writes or an atom trace reads from a PDB file. A helix, an orbit, a field line, a random coil and a backbone from real coordinates are space curves. One whose end lies within a thousandth of its size of its start is closed, and its end is moved onto its start.
_Avoid_: path (a path is flat), spline, 3D path, trajectory

**Curve point**:
One of the points a space curve written as `points` passes through, or an atom trace reads, in order, smooth between each two and never overshooting them. Of n curve points, the k-th, counted from 0, sits at u = k/(n - 1), so `u` runs from 0 at the first to 1 at the last and names a curve point however far apart they lie.
_Avoid_: control point (the curve passes through each), knot (a knot is a curve that closes on itself tangled), vertex, waypoint

**Atom trace**:
A space curve whose curve points are one atom of each amino acid or nucleotide of a PDB file, in file order, of one chain or of every chain, by atom name, such as a protein's alpha carbons, `CA`, or a nucleic acid's phosphorus atoms, `P`. Each curve point is where a molecule object of the same file draws that atom, in angstroms from the molecule's centroid, so a tube along it with the molecule's transform runs through those atoms as the molecule draws them.
_Avoid_: CA trace (for one of another atom name), backbone (a polymer chain's atoms), ribbon, cartoon (a representation)

**Tube**:
A solid made by carrying a circle of a radius along a space curve, always square to the curve, on a frame that turns no more than the curve does, so it never twists. A disc closes each end, and a tube along a closed space curve closes on itself with no ends. A spring is drawn as a tube along a helix. Its cross-section is a circle; a cartoon carries one that changes along the curve. On 3D axes its curve is in their data coordinates, and it is cut into pieces where the curve leaves their box, each closed by a disc. `inspect` reports the world point at each end of its curve, and of each curve point of a curve through curve points.
_Avoid_: pipe, cable, wire, sweep (an arc's sweep is its angle), extrusion

**Spring**:
A solid that is a coil of `turns` turns of a `radius` about the straight line between its two ends, drawn as a tube along a helix, so it stretches and squashes as its ends move and keeps its turns.
_Avoid_: coil (for the object type), helix (a helix is a space curve), tube (for a spring, which a tube draws)

**Trail**:
An object that draws the world points another object passed through over the last `length` seconds of scene time, or since that object began to show, as a stroke that faces the camera, ending where the object stands. The points are the object's world position at every frame time in that time, so a trail follows a law, a carry, a move or a data point alike. An orbit, a pendulum's arc, a projectile's path and a point tracing a phase portrait are trails. A trail has no position, rotation or scale of its own, and is not created: it grows as its object moves.
_Avoid_: trace (an atom trace is a space curve through a PDB file's atoms), path (a path is a flat shape's curve), tail, streak, motion blur, TracedPath

**Pattern**:
An object that draws one shape, a flat shape or a solid, at every copy of a layout. Each copy may take its own position, rotation, scale and color share from expressions in its index and place, and its pick of a palette. A pattern is one object, so one animation changes every copy, and it draws its copies as one picture: its scale and a copy's scale size the whole copy, stroke included.
_Avoid_: array, loop, repeat, instances, clones, group (an atom group is a molecule's)

**Layout**:
Where a pattern's copies go: a `grid` of counts along x, y and z at one spacing, centered on the pattern's position, a `ring` of copies around a circle in the pattern's own plane, or `along` a space curve, spaced evenly by length, where each copy may face the curve's tangent.
_Avoid_: arrangement, distribution, grid (alone, for the axes' lines, which axes avoid), ring (alone, for a molecule's ring of atoms)

**Copy**:
One place where a pattern draws its shape, numbered by its index: `i`, or `i`, `j` and `k` in a grid, each from 0. A copy is a member of its pattern: a move, fade, recolor or highlight that names it by its index changes it alone.
_Avoid_: instance, clone, item, element (an element is chemical), copy (for a prism's top face)

**Place**:
Where a pattern's layout puts a copy, as `x`, `y` and `z` in the pattern's own space. A copy stands at its place unless its position expressions move it.
_Avoid_: location, slot, cell (for a copy's place)

**Color share**:
How far a copy's colors are mixed toward the pattern's `to_color`, or toward the copy's pick of its palette, from 0 (its shape's own colors) to 1 (all that color), in linear light. With a palette it is 1 unless written.
_Avoid_: tint, gradient, weight (for a color share), color progress

**Palette**:
The list of colors a pattern's copies pick from, one color a copy by its palette index, such as one color for each nucleobase so each rung of a ladder takes its nucleobase's color.
_Avoid_: colormap, color map, color scheme, swatches, color list

**Palette index**:
Which color of a palette a copy picks, counted from 0: an expression in the copy's index and place, or a list of one whole number a copy, in index order, such as a sequence of nucleobases written as their palette indices. Every copy's palette index is a whole number within the palette.
_Avoid_: color index, color id, lookup, color_map

**Edge**:
A line drawn where a solid's faces meet at a sharp angle, or along its outline against what is behind it, a few pixels wide whatever the distance. Prisms, revolutions, tubes and surfaces show their edges, and spheres and molecules do not. A surface's edges are its rim and where it folds away from the camera.
_Avoid_: stroke (a stroke is a flat shape's outline), border, wireframe, silhouette (alone)

**Key light**:
The light that lights every solid, spheres and molecules included, from the camera's upper left, so a solid's side toward it shows exactly its color and its far side is darker, stepping through a few tones as an illustration does. It turns with the camera. Flat shapes, text and math are lit from the camera instead, so they show exactly their colors.
_Avoid_: sun, lamp, directional light, headlight, shading (alone)

**Surface**:
A solid that is a sheet: its height `z`, an expression in `x` and `y`, over the rectangle that its `x_range` and `y_range` bound, sampled finer where it bends. The rectangle lies on the object's floor, read as a prism's base is, and the height rises along the object's own up, so `z` is vertical as a textbook draws it. On 3D axes all three are data coordinates, and the sheet is cut at the axes' box. It is lit on both sides, each side by the key light as its own, so the camera may see it from above or below. A potential well and a wave's height are surfaces.
_Avoid_: surface plot, mesh (a mesh is triangles), heightmap, terrain

**3D axes**:
An object that shows three axes, x, y and z, with ticks, tick labels and axis labels, in a box centered on its position, with z up, x to the right and y away from the default camera, as a textbook draws them. Each label faces the camera at every frame and is hidden by what stands in front of it. A surface, a tube or a sphere that names them is written in their data coordinates and goes with them.
_Avoid_: 3D grid, coordinate system, ThreeDAxes, axes (alone, for 3D axes)

**Data coordinates**:
Where a value lies in the units an axes object's axes show, rather than in world units: `[x, y]` on axes, with z for the distance off their plane in world units, and `[x, y, z]` on 3D axes. A plot and a dot on axes, and a surface, tube or sphere on 3D axes, write their values in data coordinates, and the axes carry them to the world.
_Avoid_: data space, chart coordinates, local coordinates

**Data point**:
A dot that names axes, or a sphere that names 3D axes: its `position` is in their data coordinates, so a move, a follow or a carry takes it along a plot or across a surface on those axes, the axes carry it wherever they go, and its radius stays in world units so it stays round however the axes stretch.
_Avoid_: marker, scatter point

### Time

**Timeline**:
The ordered list of steps, some of them in chapters, that makes up a scene's animation.
_Avoid_: script, sequence, storyboard

**Step**:
One entry in a timeline. It either plays one or more animations over its duration, each over its span, or waits for its duration with nothing changing.
_Avoid_: beat, cue, keyframe, section, scene (a scene holds the whole timeline)

**Chapter**:
A named run of steps on a timeline, which `validate` and `inspect` report by its chapter id and a sound can start at. It lasts as long as its steps, or its least duration when that is longer, holding the stage as its last step left it until then. A narrated video makes one chapter per voice clip, with the clip's length as its least duration.
_Avoid_: section, beat, part (for a chapter), segment (for a chapter), scene (a scene holds the whole timeline), cue

**Chapter id**:
The name a scene gives a chapter, in its `id`. No two chapters share one.
_Avoid_: name (for a chapter's id), key, label (a label is visible text)

**Animation**:
A change to one object, or to the camera, over its span of the step that plays it.
_Avoid_: transition, tween, effect

**Member**:
One part of an object that an animation can change apart from the rest of the object, named in a field beside the animation's `object`: an atom group of a molecule by `group`, a cell of a table by `cell`, a copy of a pattern by `copy`, and a bar of a bar plot by `bar`. A highlight lights any member. A move and a fade change a copy, and a recolor a copy or a bar. A copy's moved position is in its pattern's own space, so the pattern's own move still carries it, and a copy's own opacity shows under the pattern's object opacity. The colors a recolor gives a copy or a bar are its own: a later recolor of the whole object leaves them.
_Avoid_: part (a path's part is one run of it), piece (a path's piece), element (an element is chemical), child, sub-object, selection, target (a target is a morph's or the camera's), item

**Change**:
What one animation does to one value of an object or of the camera, and the value it ends at. A fade changes an object opacity, a count a readout's value, a move its position, a scale its scale, a rotate its rotation, and a create or a write its created share. A recolor changes a stroke color, a fill color, or the color of text, math or a solid. A highlight changes the glow of a member. A move, fade or recolor that names a member changes that member's position, opacity or colors alone. A camera move changes the camera's position, a camera look its target and a camera zoom its field of view. A linear map changes the plane map of each object it names. A follow sets a position, rotation, scale, object opacity, camera position, camera target or readout's value from a law, runs a shape law, or carries a position along a curve. A simulation sets positions, rotations and readouts' values from its variables.
_Avoid_: property, update (a render update is not a change), mutation, pan, dolly

**Count**:
An animation that eases a readout's value to its `to` over its span, shaped by its easing curve, from the value it has when the count begins. A count and a move with the same span and easing curve keep a distance readout with the object it measures.
_Avoid_: tween (for a count), increment, ChangeDecimalToValue, value change

**Create**:
An animation that draws a flat shape or grows a solid over its span. A flat shape's stroke is drawn along its path by length over the first half, then its fill fades in over the second half, and a shape with a stroke and no fill draws its stroke over the whole span. A solid's operation runs from nothing to all of it: a prism rises to its height, a revolution and a sphere turn through their angle, a tube is drawn along its space curve by length, and a surface rises from its flat rectangle to its heights. Axes, a number line and 3D axes draw their lines and ticks along their length, then fade their labels in. A pattern's copies are created in index order, each starting a little after the one before. An object with a create shows nothing before the create starts, and is created once. A pattern's copy whose shape has no stroke fades its fill in over its share. A molecule, any other flat shape with no stroke, and text, math and tables, which are written, cannot be created.
_Avoid_: reveal, show creation, draw border

**Write**:
An animation that draws a text or math object glyph by glyph over its span, a few glyphs at a time: each glyph's pen along its path, then its fill fading in while the pen thins away. A table's rulings are drawn along their length over the first fifth of the span, and its cells written over the rest. A write changes the created share, as a create does. An object with a write shows nothing before the write starts, and is written once. Only text, math and tables are written, and they are never created.
_Avoid_: create (for text and math), typewriter, reveal

**Pen**:
The stroke a write outlines each glyph with: in the glyph's color, 0.025 of its text size wide, thinning to nothing as the glyph's fill comes in, so a written object ends as it was declared.
_Avoid_: write stroke, border, temporary stroke

**Created share**:
How far a create or a write has gone, from 0 (nothing of the object shows) to 1 (all of it shows): for a solid, how far its operation has run. Every object the scene file declares has 1, and an object the timeline creates has 0 until its create begins.
_Avoid_: progress (which is an animation's), drawn share, completion

**Morph**:
An animation that reshapes one object, its source, into the shape, stroke and fill of another, its target, and moves it to where the target is. The source and target are flat shapes, text, math or plots, or two tubes through as many curve points, which morph curve point by curve point, the k-th to the k-th, so a chain folds from straight into its folded shape. The source shows until the morph ends and the target from then, so the agent writes no fade, and the target is never created or written. A morph's progress mixes every point of the two paths, their colors, stroke widths and fill opacities. A molecule is not morphed: a reaction turns one molecule into another, and an object a linear map names is not morphed from that map's start on.
_Avoid_: transform (which is position, rotation and scale), transformation, replace, tween

**Recolor**:
An animation that takes an object's colors to new ones over its span, mixed in linear light: a flat shape's stroke color and fill color, or the color of every glyph of text, math or a table, or of a solid, and a table's rulings by its stroke color. A color it leaves out keeps the color it has, except a fill in its stroke's color, such as an arrow's tip, which a recolor of the stroke takes along. A recolor of a pattern recolors its shape, and each copy still mixes that color toward the pattern's `to_color`, or its pick of the palette, by its color share. A recolor that names one copy or one bar recolors that member alone, from the colors it shows, to colors of its own. Axes, number lines and molecules are not recolored: a highlight lights a member, such as an atom group or a cell.
_Avoid_: tint, colorize, color change, highlight (a highlight lights an atom group)

**Linear map**:
An animation that carries every point of the objects in its `objects` by one 2x2 `matrix` about its map origin, in the world's xy plane, over its span. `[[a, b], [c, d]]` sends a point `(x, y)` from the origin to `(a·x + b·y, c·x + d·y)` from it, and every listed point follows the same matrix at every scene time, so a circle and the radii drawn to it stay together at every frame. A later linear map acts after an earlier one, so B after A is B·A. From its start on, an object it names is not moved, scaled, rotated or morphed.
_Avoid_: apply matrix, ApplyMatrix, apply_matrix, ApplyPointwiseFunction, matrix transform, transform (which is position, rotation and scale), warp

**Map origin**:
The point a linear map's matrix acts about, in its `origin`, in world units: it stays where it is. Two linear maps of one object may have different map origins.
_Avoid_: pivot, anchor (alignment avoids it), fixed point

**Matrix route**:
How a linear map's matrix goes from the identity to all of the matrix as its progress runs, in its `route`. `entries`, the default, moves each entry straight, so a turn shrinks along the chord and a half turn passes through the zero matrix. `turn` turns by the rotation inside the matrix, the shorter way, while it stretches by the rest, so a rotation keeps every length at every frame; only a matrix whose determinant is more than 0 has one.
_Avoid_: interpolation, tween, path (a path is a curve), easing (an easing curve shapes the progress, not the matrix)

**Plane map**:
Where every linear map so far has carried an object: a matrix and a shift of the world's xy plane, applied after the object's transform. An object a linear map names has one from scene time 0, the identity until that map begins, and a plot has its axes'. An object no linear map names has none.
_Avoid_: transform (which is position, rotation and scale), pose, deformation, world transform

**Follow**:
An animation that sets an object's position, rotation, scale or object opacity, a readout's value, or the camera's position or target, at every scene time in its span from a law, runs an object's shape law, or carries an object along another object's curve, instead of easing it to a `to`, and holds what it gave from its span's end on. A pendulum swings, a planet orbits, a wave travels, pieces of plastic pulse and fade, and the camera circles a solid by a follow.
_Avoid_: updater, driver (a GPU's driver is one), always_redraw, tween, keyframes, move (a move eases to one place)

**Simulation**:
An animation that integrates its variables, named values such as an angle and its spin, from their starting values by their rates over its span, and sets positions, rotations and readouts' values from expressions in them at every scene time of it. Its rates read the variables and law time. It is integrated once, when the scene file is read, by fourth-order Runge-Kutta at eight steps a frame, and the stage at any scene time takes the step before it and integrates the rest of the way. A pendulum with drag, a projectile with air resistance, a charging capacitor, predator and prey, and an epidemic are each one simulation, and objects that share its variables, such as two masses on springs, move together. Each value it sets is held from its span's end on.
_Avoid_: physics engine, solver, integrator, ODE (for the animation), updater, simulation (for any other animation)

**Variable**:
A named value a simulation integrates, such as a pendulum's angle and its spin, from the starting value the simulation gives it. Its name is what its rate, every other rate and every set read it by.
_Avoid_: state variable, unknown (for a simulation's variable), degree of freedom, parameter (for a simulation's variable)

**Set**:
One value a simulation sets at every scene time of its span, a position, a rotation or a readout's value, from expressions in its variables and `t`. A simulation sets each value with one set.
_Avoid_: output, mapping, assignment (for a simulation's set)

**Rate**:
The expression that gives how fast one of a simulation's variables changes, per second of law time, in the variables and `t`. A simulation writes one rate for every variable.
_Avoid_: derivative (alone), velocity (a rate is any variable's), slope (for a rate), delta

**Carry**:
What a follow with `along` does: it moves an object along the path of a flat shape or a plot, or the space curve of a tube, by length in that object's own space over its span, shaped by its easing curve, wherever that object is at each scene time. A carry and a create of a plot with the same span and easing curve keep the object on the pen's end.
_Avoid_: MoveAlongPath, motion path, path animation, ride, glide

**Tangent**:
The way a curve runs at a point, toward its end. A carry with `faces: tangent` turns its object so that the object's own x axis points along the tangent, and an `along` layout with `faces: tangent` turns each copy the same way at its place, before the copy's own rotation turns it further. The object's own z axis stays square to the plane of a flat curve; on a space curve it starts as near the curve's own +z as it can, or +y on a curve that starts along z, and never twists about the curve, so on a space curve in the xy plane it stays on +z. Along straight pieces of a path that meet at joints turning less than 15 degrees, as a plot's do, the tangent turns evenly from the bisector of one joint to the next, so it follows the curve the pieces stand for; a sharper corner turns it at once.
_Avoid_: velocity, bearing, direction of travel

**Law**:
The three expressions in law time, `t`, that a follow sets its value from, or for a readout's value or an object opacity the one expression `value`. An object opacity's law must stay within 0 to 1 at every frame time of its span. The value is what the law gives, not a change added to what the follow found, so a law must start where the value is, and the value stays at what the law gave at the span's end.
_Avoid_: formula (alone), function (alone), equation of motion, rule (for a law), law (in a video's prose, such as Hooke's law, which is physics, not this)

**Law time**:
The seconds a law or a shape law reads as `t`: from the start of its follow's span, or from the start of the chapter its follow names, so follows in several steps can continue one motion.
_Avoid_: scene time (which counts from the scene's start), elapsed share, local time, phase (for law time)

**Shape law**:
The expressions of a shape that read `t`, law time: a function plot's, a space curve's and so a tube's, a surface's, or a pattern's copy expressions and `along` curve. A follow with `sets: shape` draws the shape at every scene time of its span as its expressions give it at that law time, and holds it at the span's end. Before its first follow the shape is drawn at `t` of 0. A follow's law time must start at the one the shape is drawn at, as a law must start where its value is. A travelling wave, a standing wave and a Fourier series that gains a term each chapter are shape laws. A shape law is not morphed and not carried along.
_Avoid_: animated shape, dynamic shape, time-dependent shape, updater, always_redraw

**Source**:
The object a morph reshapes, named by `object`. It is drawn as the mix of the two shapes while the morph plays.
_Avoid_: start object, original

**Target**:
The object whose shape a morph's source becomes, named by `into`.
_Avoid_: destination, end object

**Morph end**:
A flat shape's paths, stroke and fill as a morph reads them: the source's at progress 0 and the target's at progress 1. Text and math give one group of parts per glyph, filled in the glyph's color. A tube gives its curve points as written, its radius and its material.
_Avoid_: snapshot

**Mix**:
What a morph's source is drawn as while the morph plays: each group of its morph end mixed with its partner group at the morph's progress, each in its own mixed stroke and fill. A glyph with no partner keeps its shape and place and fades out, or in, over the morph. Any other group with no partner keeps its own stroke and fill as it grows or shrinks. A tube's mix is the tube through each curve point mixed with its partner, its curve made again through them at every scene time, with its radius and material mixed.
_Avoid_: blend (a blend is a reaction's), interpolation, tween

**Path match**:
The pairing of the parts and pieces of a morph's two paths, so that a mix has a partner for every point. Between two text or math objects, glyphs that show the same characters pair, as many as keep their order on both sides, the earliest where there is a choice, so `πr × r` into `πr²` pairs `π` with `π` and the first `r` with `r`, and no glyph is bent into another. Any other two shapes pair their groups in order. Within a group, parts pair in path order. Each side's longest piece is halved until both have twice the larger piece count, so a corner can meet the middle of a piece. A closed part is turned to start at its partner's nearest corner, and a part with no partner grows from or shrinks to a point at its own middle. Two plots instead pair each part's points at the x values of both, so every x of the mix moves straight up or down. A path match keeps no point's place, where a linear map keeps every point's.
_Avoid_: alignment (alignment is text's), correspondence, registration

**Morph plan**:
Everything a morph needs from its two ends, worked out once when the scene file is read: the path match, and the strokes and fills it mixes, or the two tubes' curve points, radii and materials.
_Avoid_: morph (the animation), interpolation

**Mesh**:
The triangles one surface of an object is cut into, with the material they are drawn with. An object becomes one or more meshes that share its transform. A circle's fill and stroke are two meshes.
_Avoid_: geometry, model (for a mesh), drawable

**Draw list**:
Every mesh of every object on a stage, each with its transform, plus the camera. It is everything the GPU draws for one frame.
_Avoid_: render list, batch, command list

**Scene time**:
The number of seconds since the start of a scene. The command-line flag is `--scene-time`, and `-t` is its short form.
_Avoid_: t (alone, except as the `-t` flag; a law's `t` is law time), timestamp, frame number

**Span**:
The part of its step an animation plays over, from its `start` to its `end` in seconds from the start of the step. The default is the whole step. A narrated step sets a span to start on the word that names the change. A reserved region's span is instead a run of scene time, from START to END in scene seconds, both included, given as `@START-END` after its box. Its default is the whole scene.
_Avoid_: window, interval (for a span), range (an axis range is an axes'), offset (an attachment's)

**Progress**:
How far an animation has gone, from 0 at the start of its span to 1 at the end.
_Avoid_: alpha, percent, scene time

**Elapsed share**:
The share of an animation's span that has passed at a scene time, from 0 at the start of the span to 1 at the end.
_Avoid_: progress (what an easing curve makes of it), alpha, t (alone; a law's `t` is law time)

**Easing curve**:
The curve that turns an elapsed share into progress. A linear easing curve moves at a constant speed. Others start slow, end slow, or overshoot.
_Avoid_: rate function, timing function, interpolation

**Track**:
Every change one value goes through, in scene time order, such as the position of one object or the target of the camera. A value has one track, so two animations whose spans overlap cannot change the same value, and two in one step can change it one after the other.
_Avoid_: channel, lane, keyframes, curve (an easing curve is a curve)

**Sound**:
A sound file on disk, such as a narrator's voice clip, that plays under the video from the start of a chapter, as the scene file's `sounds` lists it. A render mixes every sound at the loudness it has and cuts it at the video's end. A frame has no sound.
_Avoid_: audio, soundtrack, track (a track is one value's changes), sample (for a sound)

**Frame**:
One image of a scene at one scene time.
_Avoid_: still, snapshot, screenshot

**Frame time**:
A scene time a frame is drawn at: frame `i` at `i / fps`. `validate` evaluates a law at every frame time of its span and at its end, and `stages` reports every frame time between two scene times.
_Avoid_: frame number, tick, timestamp

**Pixel alpha**:
How much a frame's pixel covers the image behind it, from 0 for an empty pixel to 1 for an opaque pixel. It includes both surface opacity and partial coverage at an object's edge.
_Avoid_: object opacity (which applies to one object)

**Straight alpha**:
A frame representation whose RGB colors are independent of its pixel alpha.
_Avoid_: unassociated alpha

**Premultiplied alpha**:
A frame representation whose linear RGB colors have already been multiplied by its pixel alpha.
_Avoid_: associated alpha

**Frame sequence**:
Every frame of a scene in scene time order, stored as numbered images at the scene config's frame rate.
_Avoid_: image sequence, PNG sequence

**Readback**:
A frame the GPU has drawn, copied into memory the processor can read, still in the GPU's color format with its rows padded. `frames` turns each readback into a frame and a PNG on a thread of its own while the GPU draws the next frames.
_Avoid_: download, staging buffer, capture

**Render**:
Turning a whole scene into a video file or a frame sequence.
_Avoid_: export, build, compile, frame (one image, not a video)

**Video**:
One file that holds every frame of a scene in scene time order at the scene config's frame rate, with its sounds when present.
_Avoid_: movie, output, export

**GPU**:
The graphics processor a command that draws, `frame`, `frames`, `render` or `check-layout`, draws its frames on, as one driver offers it. A machine can have several: a laptop may have an NVIDIA GPU and an Intel one, and lavapipe offers the processor as one. The report of `frame`, `frames` and `render` names the GPU in `gpu`, with its `name`, GPU kind, `backend` and `driver`.
_Avoid_: adapter (except wgpu's `Adapter`, in the module that uses wgpu), graphics card, device (for a GPU)

**GPU kind**:
What sort of GPU a driver says it offers: `discrete`, a GPU of its own; `integrated`, one built into the processor; `other`, one whose driver names no kind, as OpenGL's do; `virtual`, one a virtual machine is given; or `cpu`, a driver that draws on the processor. A command draws on a discrete GPU when the machine has one, then an integrated one, and so on in that order. `--gpu KIND` draws on the first GPU of that kind instead, and is a GPU error when the machine has none.
_Avoid_: power preference (except wgpu's `PowerPreference`), device type (except wgpu's `DeviceType`), adapter type

### Plots

**Axes**:
An object that shows two axes with ticks and tick labels in a box of its `width` and `height`, centered on its position. Data coordinates map linearly onto the box, and each axis crosses the other at 0, or at the nearest end of the other's range, except that the y axis stands at the low end of a category axis. A create draws the axis lines and ticks along their length, then fades the labels in.
_Avoid_: grid (for axes), coordinate system, plane (for axes), chart, graph

**Number line**:
An object that shows one axis with ticks and tick labels, `length` long and centered on its position. It has no y axis, so no plot goes on it.
_Avoid_: ruler

**Axis range**:
The lowest and highest data value an axis shows, in the field `range`.
_Avoid_: domain, limits, extent

**Category axis**:
An x axis of axes that shows `categories` in order instead of a range of numbers. Category k, counted from 0, sits at data x = k and is one data unit wide, and the y axis stands at the category axis' low end.
_Avoid_: categorical axis, discrete axis, ordinal axis, band scale

**Category**:
One named place along a category axis, with a tick and its label in plain text below the axes' box.
_Avoid_: bin (for a category), bucket, class (for a category), group (a group is atoms)

**Tick**:
A short stroke across an axis at a multiple of its `spacing`, the distance in data units between ticks, or at each category of a category axis.
_Avoid_: mark, notch, step (a step is the timeline's)

**Tick label**:
The number typeset in math beside a tick, with as many decimals as the spacing has, or on a category axis the category's label in plain text. It sits below, above, left or right of its tick, or in the corner below and left of where the axes cross. On 3D axes it faces the camera, and the tick where the axes cross has none. An axis label is LaTeX math in the field `label`, set at the high end of its axis.
_Avoid_: tick text

**Crossed label**:
A tick label or axis label of a plot's axes that the plot's stroke, at its full width, runs into the bounding box of. `inspect` names each one on the plot in `crosses_labels`, so an agent that cannot see the frame can move the plot or the label.
_Avoid_: collision, overlap, clash, intersection (for a label)

**Crossed object**:
A text or math object whose glyphs a plot's stroke, at its full width, runs into on the frame at one scene time, while both show. `inspect` names each one on the plot in `crosses_objects`. A stroke that passes between two glyphs crosses neither.
_Avoid_: collision, overlap, clash, intersection (for an object)

**Plot**:
A flat shape written in the data coordinates of an axes object, which the plot names in `axes`: a stroke along a curve, or a bar plot. A plot has no position, rotation or scale of its own: it is placed by its axes and is always where they are. A curve is cut at the axes' box, where it has no value, and where it jumps, as `step(x)` does at 0.
_Avoid_: graph, chart, line (a line is an object)

**Function plot**:
A plot whose curve is the graph of an expression in `x`, sampled finer where the curve bends.
_Avoid_: graph of a function, get_graph

**Point plot**:
A plot whose curve is the straight pieces through the points the scene file lists, in order. It is how an agent plots data, or a function an expression cannot write.
_Avoid_: scatter plot, line plot, data series

**Bar plot**:
A plot of one bar for each category of its axes' category axis, given as one value per category. A create grows its bars one at a time, in category order.
_Avoid_: bar chart, column chart, histogram

**Bar**:
A filled rectangle of a bar plot, 0.7 of its category's width, standing on the x axis and reaching its category's value, below the x axis for a value below it. The part outside the axes' box is cut, as a curve is. A bar is a member of its bar plot, named by its index in category order from 0, so a highlight lights it and a recolor recolors it alone.
_Avoid_: column (for a bar), rect (for a bar)

**Expression**:
A formula in infix math, such as `x^2 - 3*sin(2*x)`. Its variables are the ones its field names: `x` in a function plot, `u` in a space curve, `t`, law time, in a law, `x` and `y` in a surface, in a pattern a copy's index, `i` or `i`, `j` and `k`, and its place, `x`, `y` and `z`, and in a simulation's rates and sets its variables and `t`. A function plot's, a space curve's, a surface's and a copy's expressions may read `t` too, which makes them a shape law. Its constants are `pi`, `e` and `tau`, and its operators and functions are a fixed list that `sci_anim schema` prints. It has no assignment, loop or definition, so it is data, read and checked when the scene file is read. In a plot, a value outside a function's domain is a gap in the curve; in a space curve, a surface, a law or a copy it is a scene error.
_Avoid_: lambda, equation (alone), formula (for the object type)

### Tables

**Table**:
An object that shows text and math in cells, in rows and columns, with rulings between them, centered on its position. Each column is as wide as its widest cell and each row as tall as its tallest, plus the cell padding on every side. A write draws its rulings along their length, then writes its cells in reading order, row by row and each row left to right.
_Avoid_: grid (for a table), matrix (a matrix is math or a linear map's), spreadsheet, tabular, chart

**Row**:
One run of a table's cells from left to right, numbered from 0 at the top. Every row of a table has the same number of cells.
_Avoid_: record, line (a line is an object)

**Column**:
One run of a table's cells from top to bottom, numbered from 0 at the left.
_Avoid_: field (for a column; a field is a key of the scene file), column (for a bar)

**Cell**:
The place in a table where one row and one column meet: a rectangle that holds plain text, math, or nothing, set in the table's text size and lined up in it by the table's alignment.
_Avoid_: entry (for a cell), item, cell (for a copy's place)

**Cell address**:
The row and the column that name one cell of a table, each counted from 0, written `[row, column]`. A highlight names a cell by its table's object id and its cell address, as it names an atom group by its molecule's object id and its group id.
_Avoid_: cell id, coordinates, position (an object's), index (alone, for a cell; an index is a copy's)

**Ruling**:
A stroke a table draws between its rows or columns, as its `rulings` choose: `all` between every row and column and around the table, `header` once under the first row, or `none`.
_Avoid_: line (a line is an object), border, grid line, rule (a rule is a glyph, such as a fraction bar)

**Cell padding**:
The space between a cell's rectangle and the box of its text or math, on every side, in world units. The field is `padding`.
_Avoid_: margin, inset, spacing (a tick spacing is an axis')

### The tool

**Report**:
The JSON object a command prints for the agent. It holds either what the command did or every error that stopped it.
_Avoid_: output, response, result (for the whole object)

**Render update**:
A JSON object on a line of its own that a render can print while it runs, apart from its report. It says how many frames are drawn, and whether the render is drawing frames, finishing, or finished. It is not a report, and the report alone says whether the render worked.
_Avoid_: progress (an animation's), phase (a morph's or reaction's), status (a report's), event, stage (the camera and objects at a scene time)

**Scene error**:
One thing wrong with a scene file, or with the scene time a command asks for, with the JSON path of the bad field and a message. `validate` lists every scene error at once.
_Avoid_: validation error, issue, diagnostic, problem

**JSON path**:
Where a field sits in a scene file, written as a JSON Pointer such as `/objects/disc/radius`. The empty path is the whole file.
_Avoid_: location, pointer (alone), key path, field path

**Bounding box**:
The smallest box, lined up with the axes, around an object at one scene time, in world units and in pixels of the frame. It is the whole object's box even while the object is hidden, faded or not yet created, and under a plane map the box of the shape the map makes. An object with nothing to draw has none. A tick label or axis label of an axes object has its own, around its glyphs.
_Avoid_: bounds, extent, aabb, rect

**Layout check**:
The `check-layout` command: it reads the stage at each checked scene time and prints each layout fault, then writes `stages.txt`, what shows at each checked scene time in words, and the contact sheets.
_Avoid_: lint, layout validation, checker script

**Layout fault**:
One thing wrong with the picture that the layout check prints, with the run of checked scene times it lasts: an object the frame cuts off, text over another object or too small or faint to read, an object in a reserved region, and the like. It is not a scene error: the scene file is valid, and the picture is wrong.
_Avoid_: problem (the word the printed lines use, kept for their readers), issue, warning, scene error

**Checked scene time**:
A scene time the layout check reads the stage at: the middle and end of a step, the end of a chapter, the moment before a step's fades to 0 begin, the start, middle and end of an animation's span, and every frame time of a follow's span. Times within a millisecond are one. Step ends, chapter ends and the scene's duration are kept to the nanosecond, so steps of 0.7 and 0.1 seconds end at 0.8, not at their float sum. The layout check writes each checked scene time with six significant digits, or more where six would round past the scene's end, so `inspect` and `frame` take back every one it writes.
_Avoid_: sample (a sound's), probe, checkpoint

**Contact sheet**:
A PNG of tiles, one frame per checked scene time that ends or halves a step, in scene time order, with each reserved region outlined in magenta on the tiles inside its span. The frames under it are written full size beside it.
_Avoid_: storyboard, thumbnail grid, montage

**Reserved region**:
A box of the frame, in pixels from its top left, that the layout check keeps clear of every shown object except those the agent exempts, such as where a player shows a video's subtitles. It holds for the whole scene, or only over its span, such as where a photo is laid over the video for one chapter. An object still fading counts as shown. A molecule enters it by a ball whose disc reaches into the box, even with the atom's center outside. It is never drawn in the video.
_Avoid_: caption region (a caption is a text or math object with an attachment), safe area, subtitle area, mask

**Video clip**:
An MP4 or WebM file that another tool lays over or under sci_anim's picture as a layer, played muted from the layer's start between two times of the clip, and held at its last frame if the layer lasts longer. sci_anim never reads one.
_Avoid_: clip (alone; a narrator's voice clip is a sound), footage, movie

**Skill**:
The instructions that teach an agent to make an instructional video with the tool: one entry file that says which other files to read for the subject, such as math or chemistry.
_Avoid_: guide, manual, prompt, playbook

**Skills folder**:
The folder an agent loads skills from, such as `~/.claude/skills` for Claude Code, `~/.agents/skills` for other agents, or `.claude/skills` for one project. `sci_anim skill install` writes the skill into a `sci-anim` folder in it.
_Avoid_: skills directory, skill path, plugin folder, install location

**User level**:
The skills folders in the user's home folder, which agents load in every project, such as `~/.claude/skills`.
_Avoid_: global (for a level), system-wide, home install

**Project level**:
The skills folders at the root of one git repository, which agents load only in that repository, such as `.claude/skills`.
_Avoid_: repo level, repository level, local (for a level), workspace (for a level)

**Scene generator**:
A short program an agent writes that builds a scene's objects and timeline and writes its scene file, for a scene too big or too repetitive to write by hand, such as a circle cut into sixty slices or a mass that bounces under every step. The agent edits the generator and runs it again, never the scene file it writes. The scene file stays data: the tool never runs a generator.
_Avoid_: script (alone, which the scene file is not), build script, template, macro

### Molecules

**Compound**:
PubChem's record for one chemical substance. Not every compound has a structure file, because PubChem has no 3D model for large or flexible compounds such as proteins.
_Avoid_: molecule (the object drawn in a scene), chemical, substance

**Structure file**:
A file that gives a molecule's atoms with their 3D positions and the bonds between them: an SDF file, as PubChem serves it, or a PDB file. An SDF file holds one shape of the compound, which PubChem calls a conformer. PubChem serves it, or the agent writes it for a molecule PubChem has no 3D model of, such as a polymer chain.
_Avoid_: molfile, model (for a structure file; a PDB file's models are its sets of atoms), conformer file, coordinates (for a structure file)

**PDB file**:
A structure file in the format of the Protein Data Bank, which serves the structures of DNA, RNA and proteins that experiments found. Its atoms come in residues and chains, and it gives the bonds of standard residues by their atom names alone, not as bonds. It may give several models of one structure, as an NMR experiment does; a molecule is read from the first.
_Avoid_: PDB entry (the Protein Data Bank's record, which a PDB file is one form of), mmCIF, protein file

**Residue**:
One unit of a chain in a PDB file: an amino acid of a protein, a nucleotide of DNA or RNA, or a ligand or ion the file names as one. It has a residue name, such as `ALA` or `DG`, a residue number, and its atoms, each with an atom name.
_Avoid_: monomer (the molecule before it joins), repeating unit (a polymer chain's), amino acid or nucleotide (for a residue of either kind)

**Residue number**:
The number a PDB file gives a residue in its chain, with an insertion code, a letter such as the A of 52A, when the file gives two residues one number. Residue numbers rise along a chain, but need not start at 1 or be unbroken.
_Avoid_: sequence number, position (a residue number is not a place), index (for a residue number), residue id

**Residue name**:
The three letters or fewer a PDB file names a residue's kind by, such as `ALA` for alanine, `DG` for a nucleotide of guanine in DNA and `HOH` for water.
_Avoid_: residue type, code (for a residue name; an insertion code is part of a residue number), name (alone, for a residue name)

**Atom name**:
The name a PDB file gives an atom within its residue, such as `CA` for an amino acid's alpha carbon or `N1` for a nucleobase's first nitrogen. No two atoms of a residue share one. A sugar's atoms carry a prime, as `C1'`.
_Avoid_: atom label, atom type, atom id

**Chain**:
The residues of one strand of DNA or RNA or one protein in a PDB file, bonded one to the next, named by its chain id, a letter such as `A`. The Dickerson dodecamer is two chains, A and B, one for each strand.
_Avoid_: strand (for a chain of a PDB file), subunit, segment, polymer chain (a molecule of repeating units the agent writes)

**Standard residue**:
A residue whose bonds sci_anim finds from its atoms' distances, because a PDB file leaves them out: one of the 20 amino acids, or a nucleotide of DNA or RNA. Two of its atoms are bonded when they lie no farther apart than their covalent radii together and 0.4 Å. One is also bonded to the next residue of its chain, by the peptide bond from an amino acid's `C` to the next one's `N`, or by the phosphodiester bond from a nucleotide's `O3'` to the next one's `P`, when those two atoms are as near. Every bond found this way is single.
_Avoid_: standard amino acid (for a nucleotide too), canonical residue, known residue

**Covalent radius**:
An element's share of the length of a single bond, so two bonded atoms lie about their two covalent radii apart, such as 0.75 Å for carbon and 0.71 Å for nitrogen.
_Avoid_: bond radius, radius (alone, which is the van der Waals radius a ball is sized by)

**Alternate location**:
One of two or more positions a PDB file gives an atom that the experiment found in more than one place, marked `A`, `B` and on. A molecule keeps the first location the file gives each residue, usually `A`, and leaves out the rest.
_Avoid_: altLoc, conformer (an SDF file's shape), disorder

**Nucleobase**:
The ring part of a nucleotide that pairs across DNA's two strands: adenine, thymine, guanine, cytosine, or uracil in RNA. In a PDB file it is a nucleotide's atoms whose atom names carry no prime and are not of its phosphate. An atom group written by residues with `part` `nucleobase` holds only the nucleobases.
_Avoid_: base (a base is a prism's), nucleotide (the nucleobase with its sugar and phosphate)

**Molecule**:
An object made of the atoms and bonds read from one structure file.
_Avoid_: compound (PubChem's record), structure (for a molecule), model (for a molecule)

**Atom**:
One atom in a molecule, with its element and its position. An atom is known by its number in the structure file: its place in an SDF file, counting from 1, or the serial number a PDB file gives it. In a reaction it is known by its reactant's atom reference.
_Avoid_: node, vertex, ball (a ball is how a representation draws an atom)

**Element**:
The chemical element of an atom, such as carbon or oxygen, written as its symbol: `C`, `O` or `Cl`.
_Avoid_: element (for anything that is not chemical), atom type

**Element color**:
The color an atom gets from its element unless the scene sets another, such as gray for carbon, red for oxygen, blue for nitrogen and white for hydrogen.
_Avoid_: CPK color, atom color (a color the scene sets for one atom)

**Bond**:
A link between exactly two atoms in the same molecule, with a bond order. A bond has no position of its own. It always runs between its two atoms.
_Avoid_: edge, link, stick (a stick is how a representation draws a bond)

**Bond order**:
Whether a bond is single, double or triple. An aromatic ring, such as benzene's, has alternating single and double bonds, as PubChem's structure files give it. Drawing such a ring as aromatic is a choice of representation.
_Avoid_: bond type, multiplicity, aromatic (as a bond order)

**Polymer chain**:
A molecule made of the same repeating unit many times over, such as polyethylene. A real one has thousands of units, so a scene shows a short chain of a few, with an end group at each end.
_Avoid_: polymer (alone, for one chain), oligomer, strand (DNA's)

**Cross-link**:
A bond that joins one polymer chain to another, as the chains of a thermoset are joined, so they cannot slide apart when it is heated, as a thermoplastic's do. A scene draws one as a line whose two ends sit on the two chains' tubes, each at a `u`.
_Avoid_: bridge, tie, rung (a ladder's), link (alone)

**Hydrogen bond**:
The pull between a hydrogen bonded to a nitrogen or an oxygen and another nitrogen or oxygen nearby, weaker than a bond and not one of a molecule's bonds. Hydrogen bonds hold the two nucleobases of a nucleobase pair together. A scene draws one as a dashed line from the hydrogen's atom to the other atom, each end on its atom.
_Avoid_: H-bond, hydrogen bridge, bond (alone, for a hydrogen bond)

**Nucleobase pair**:
Two nucleobases across from each other on DNA's two strands, held together by hydrogen bonds: adenine with thymine by two, guanine with cytosine by three. A scene reads one from a structure file of both nucleobases, or lights one in a PDB file of DNA as an atom group of the nucleobases of two residues.
_Avoid_: base pair (a base is a prism's), nucleotide pair, rung (a ladder's)

**Monomer**:
The small molecule that polymerization joins into a polymer chain, such as ethylene for polyethylene.
_Avoid_: building block, subunit

**Repeating unit**:
The atoms of a polymer chain that one monomer gave it, such as the CH₂–CH₂ of each ethylene in polyethylene. A scene points at one with an atom group.
_Avoid_: monomer (the molecule before it joins), mer, segment, residue (a PDB file's unit of a chain)

**End group**:
The atoms that end a polymer chain past its last repeating unit, such as the CH₃ left by an initiator or the hydrogen that caps a short chain.
_Avoid_: terminus, terminal group, chain end (for its atoms)

**Backbone**:
The atoms of a polymer chain that run from one end group to the other, bonded one to the next, such as polyethylene's carbons. A backbone may pass through a ring, as PET's passes through a benzene ring between two ester groups. A chain's length is counted in backbone atoms, a ring in the backbone counting as one.
_Avoid_: main chain, spine, skeleton

**Side group**:
An atom or small group bonded to a backbone atom in place of a hydrogen, such as polypropylene's CH₃, PVC's Cl or polystyrene's benzene ring.
_Avoid_: substituent, pendant group, branch

**Ring**:
Atoms of a molecule bonded one to the next in a closed loop, such as benzene's six carbons. A benzene ring's bonds alternate single and double. A ring can be a side group, as polystyrene's is, or lie in a backbone, as PET's does. A benzene ring that is a side group is a phenyl.
_Avoid_: cycle, aromatic (as a name for the ring)

**Centroid**:
The mean position of every atom of a molecule, hidden atoms too, so no representation moves it. A molecule is placed with its centroid at the object's position, and a rotation turns it about its centroid.
_Avoid_: center of mass, origin (the structure file's origin is not the object's)

**Ball**:
How a representation draws an atom: a sphere in the atom's element color, with a radius a quarter of its element's van der Waals radius in ball-and-stick and the whole of it in space-filling. The frame shows it as a disc, and `inspect` reports each shown atom's `ball_radius`, the disc's radius in pixels after the molecule's scale and the camera. The layout check finds the atoms a text covers by their discs, which together cover what a space-filling molecule covers in the frame.
_Avoid_: atom (an atom has no size)

**Stick**:
How the ball-and-stick representation draws a bond: a cylinder from each of its two atoms to the middle of the bond, in that atom's element color. A single bond has one stick at each atom, a double bond two side by side and a triple bond three, spread across the bond in the plane of the bond and the first other atom bonded to either end.
_Avoid_: cylinder, line (a line is an object), bond (a bond is the link, not how it looks)

**Hidden atom**:
An atom that the representation does not draw, such as a hydrogen when the scene hides hydrogens, or any atom of a cartoon. It is still an atom of its molecule, with its number, element and position, and `inspect` reports it with `"hidden": true`. A bond is hidden when either of its atoms is, and in space-filling, which draws no stick, every bond is. An object at object opacity 0 is not hidden in this sense, and neither is its bounding box.
_Avoid_: removed, deleted, invisible

**Representation**:
The way a molecule is drawn, ball-and-stick, space-filling or cartoon, and whether its hydrogens are shown. It changes how a molecule looks, never which atoms and bonds it has. A hidden hydrogen is still an atom of its molecule. Each representation owns what it draws: the size of its balls, whether it draws sticks, and how its glow looks.
_Avoid_: style, display mode, model (for a representation), rendering

**Ball-and-stick**:
The representation that draws each shown atom as a ball a quarter of its element's van der Waals radius and each bond between two shown atoms as its sticks, so a reader sees which atoms are bonded. It is the default, and the one a reaction draws. The layout check finds its unbonded atoms that read as bonded, and on a molecule of up to 200 shown atoms, a group's atoms hidden behind others and its bonds that point at the camera.
_Avoid_: ball and stick model, stick model (sticks alone are another representation)

**Space-filling**:
The representation that draws each shown atom as a ball at its element's van der Waals radius, overlapping its bonded neighbors, and no sticks, so a molecule of thousands of atoms reads as the space it fills. Its glow lies close over its group's balls and colors them. A reaction cannot turn a space-filling molecule.
_Avoid_: CPK model (CPK is a color scheme too), van der Waals representation, surface (a surface is an object), spacefill

### Proteins

**Secondary structure**:
The stretches of a chain that fold into a regular shape held by hydrogen bonds along the backbone, as a PDB file's `HELIX` and `SHEET` records give them, each by its chain and the residue numbers of its first and last residue: a helix, such as an α-helix, or a β-strand. `inspect` lists each stretch with its residue range, so an atom group can be written by its residues.
_Avoid_: fold (the whole chain's shape), motif, domain, SSE

**α-helix**:
A stretch of a protein chain wound into a right-handed helix of 3.6 residues a turn, as a `HELIX` record of class 1 gives it. A `HELIX` record of another class, such as the 3₁₀ helix of class 5, is a helix of secondary structure too, and a cartoon draws every helix the same; `inspect` gives each one's `class`.
_Avoid_: alpha helix (write α-helix), helix (alone, which is also a space curve, such as a spring's), spiral, coil (the parts of a chain in no stretch)

**β-strand**:
A stretch of a protein chain drawn out nearly straight, lying beside other β-strands in a β-sheet, as a `SHEET` record gives it by its sheet id and its strand number.
_Avoid_: strand (alone, which is DNA's or a chain's), beta chain, extended segment

**β-sheet**:
β-strands lying side by side, held by hydrogen bonds between neighbors, as the `SHEET` records that share a sheet id give them. Ubiquitin's five β-strands make one β-sheet.
_Avoid_: pleated sheet, sheet (alone; a surface is a sheet), layer

**Coil**:
The parts of a chain in no helix and no β-strand, which a cartoon draws as a thin round tube.
_Avoid_: loop (one stretch of coil between two others), random coil (an unfolded chain), turn (a rotate turns)

**Cartoon**:
The representation that draws a protein, DNA or RNA read from a PDB file as a textbook draws its fold: each chain's backbone as one tube through its amino acids' alpha carbons, `CA`, or its nucleotides' phosphorus atoms, `P`, whose cross-section is a thin circle along coil, a flat ribbon along each helix, its flat side facing out from the helix's axis, and a flat arrow along each β-strand, lying in its β-sheet's plane. It draws no atom as a ball, so every atom is hidden. A highlight lights the tube along each residue whose `CA` or `P` the atom group holds.
_Avoid_: ribbon diagram, ribbon (one part of a cartoon), schematic, atom trace (a space curve)

**Cross-section**:
The outline a tube carries along its space curve, square to it: a circle of the tube's radius for a tube object, and for a cartoon an ellipse whose width and thickness change along the curve, steps included, as the tip of a strand arrow steps out. Its width lies along a direction it carries at each point, which a cartoon sets from the chain's atoms rather than letting the frame turn no more than the curve does.
_Avoid_: profile (a revolution's), section (alone), slice, outline (a stroke)

### Atom groups

**Atom group**:
Some atoms of one molecule object, named in the molecule's `groups` by their atom numbers, or on a molecule read from a PDB file by their residues, so a highlight or a caption can point at them. A group may hold hidden atoms. It changes nothing about how the molecule draws.
_Avoid_: functional group (a chemist's meaning, which the tool does not check), selection, residue (a group may hold residues' atoms, but is not one), fragment

**Residue pick**:
One entry of an atom group written by residues: one residue of a chain by its residue number, the residues of a chain from one residue number to another, or a whole chain.
_Avoid_: selection (an atom group is avoided as one), residue range, residue filter

**Group id**:
The name a molecule object gives one of its atom groups. A highlight and an attachment name a group by its molecule's object id and its group id.
_Avoid_: name (for an atom group's id), key, label (a label is visible text)

**Group center**:
The mean position of an atom group's shown atoms, or of all its atoms when none is shown. `inspect` reports it as the group's `center`, and an attachment is measured from where the frame shows it.
_Avoid_: anchor (alignment avoids it), centroid (the whole molecule's), pivot

**Highlight**:
An animation that takes the glow of a member, an atom group, a table's cell, a pattern's copy or a bar plot's bar, to a glow share over its span, in a color the scene sets or the color the glow already has. A glow at glow share 0 shows no color, so it lights up in the highlight's color from the start of the span. Lighting a member in and out takes two highlights, as a fade in and out takes two fades.
_Avoid_: select, emphasize, indicate, flash

**Glow**:
The see-through shell around a lit atom group's shown atoms and the bonds between them, or the see-through fill of a lit cell's rectangle behind its text and rulings, a see-through box a little larger than a lit flat copy, behind its stroke and around its fill, a see-through frame around a lit bar that fills its category's width, or a see-through shell around a lit solid copy, in one color. A highlight lights it, and a reaction carries a group's glow to the products. The element colors of the atoms inside show through it, and where two shells overlap it is no brighter. A group's representation sets how its glow looks: in ball-and-stick a faint film standing well off the balls and around the sticks, in space-filling a film lying close over the balls, shaded by the key light and opaque enough to color the group's atoms against the thousands unlit around them, and in a cartoon a faint film around the tube along the group's residues.
_Avoid_: halo, aura, outline (the stroke is the outline), atom color

**Glow share**:
How far a member's glow shows, from 0 (none) to 1 (all of it).
_Avoid_: progress (an animation's), glow opacity, intensity

**Attachment**:
Placing a flat shape, text or math object beside another object, or beside an atom group of a molecule, instead of by a transform of its own: at that object's position, or the group center, as the frame shows it, moved by an offset along the frame's right and up, facing the camera, and in front of all of that object. One move of a box carries the number attached to it. An attached object, like a plot, has no position, rotation or scale of its own, and the object it is attached to must have its own.
_Avoid_: anchor, pin, link (a bond is a link), follow (a follow is an animation that sets a value from a law or a curve)

**Caption**:
A text or math object with an attachment. A line or arrow with an attachment to the same group is its leader line.
_Avoid_: annotation, callout, tag, label (a tick label is an axes')

### Reactions

**Reaction**:
An animation that turns reactant molecules into product molecules. Each atom moves from its reactant position to its product position, while broken bonds disappear and formed bonds appear. Its reactants show until it begins and its products from its end. A reactant group lit when the reaction begins keeps its glow while it plays, as a carried glow. A scene time falls before, during or after a reaction's span, which `inspect` calls its phase.
_Avoid_: morph (which reshapes any object), transformation, conversion

**Reactant**:
A molecule as it is before a reaction, and the molecule object that shows it.
_Avoid_: input, start molecule, substrate

**Product**:
A molecule as it is after a reaction, and the molecule object that shows it.
_Avoid_: output, result, end molecule

**Atom map**:
The pairing of every reactant atom with the product atom it becomes. Every atom on each side has exactly one partner on the other, so the reactants and products have the same atoms. Structure files fetched separately number their atoms independently, so a reaction needs an atom map.
_Avoid_: atom mapping, correspondence, matching

**Broken bond**:
A bond that joins two atoms in the reactants but not in the products.
_Avoid_: removed bond, deleted bond

**Formed bond**:
A bond that joins two atoms in the products but not in the reactants.
_Avoid_: new bond, added bond, created bond

**Bond fate**:
What a reaction does to a bond, found by comparing its reactants and products through the atom map: the bond is kept, broken, formed or changed.
_Avoid_: bond change, bond status, bond diff

**Changed bond**:
A bond that joins the same two atoms in the reactants and the products, with another bond order in the products, such as ethylene's double bond becoming a single bond.
_Avoid_: kept bond (a kept bond keeps its bond order), reordered bond, broken and formed bond

**Atom reference**:
An object id and an atom number that name one atom of one molecule object, written `{ "object": "ethylene", "atom": 1 }`. An atom map pairs atom references, `inspect` names a reaction's atoms by them, and an end on an atom is one.
_Avoid_: atom id, atom address, atom name (a PDB file's name of an atom within its residue)

**Lifetime**:
The scene times an object exists in, from when it is made to when it is used up. A product's lifetime starts at the end of the reaction that makes it, and a reactant's lifetime ends at the start of the reaction that uses it up. A target's lifetime starts at the end of the morph that makes it, and a source's lifetime ends at the end of that morph, because the source is what the morph draws. An object is made at most once and used up at most once, by a reaction or a morph, and never made after it is used up. An attached object's lifetime is that of the object it is attached to. Outside its lifetime an object is not drawn, and the agent writes no fade for it.
_Avoid_: existence, presence, visibility

**Blend**:
Every atom and bond of a reaction at one scene time: each atom on its straight line between its reactant position and its product position, and each bond at its bond weight. A reaction's blend is drawn while the reaction plays.
_Avoid_: interpolation, intermediate, frame (a frame is an image)

**Carried glow**:
The glow of a reactant's atom group that is lit when its reaction begins, drawn around the group's atoms and bonds in the blend while the reaction plays. At the reaction's end it passes to the product group that holds exactly the atoms each product receives, at the same glow share and color.
_Avoid_: followed glow, glow transfer, moving highlight

**Fit**:
The rotation that turns a reaction's products to lie as close as possible to its reactants over every mapped atom, found by Kabsch's method, and how far apart the two still lie, as `fit_rmsd`. A reaction with `fit` false has none: each product ends at its own rotation, and `fit_rmsd` is how far apart the products, as those rotations turn them, lie from the reactants.
_Avoid_: alignment (alignment is text's), registration, superposition

**Bond weight**:
How thick a bond's sticks are during a reaction, as a share of full, from 0 to 1. A broken bond's weight falls to 0 over the first half of the progress, and a formed bond's rises from 0 over the second half.
_Avoid_: bond opacity, bond strength, thickness
