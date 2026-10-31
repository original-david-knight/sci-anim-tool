//! The timeline a scene file declares, steps and animations as `sci_anim schema` prints them,
//! and how each animation is read into a `Change`.

use super::*;

/// A step either waits with nothing changing or plays animations, each over its span. Both last their
/// `duration`. A chapter on the timeline names a run of steps.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(super) enum StepFile {
    /// Holds the stage as it is for `duration` seconds.
    Wait {
        /// How long the step lasts, in seconds, more than 0.
        duration: Duration,
    },
    /// A named run of steps, which `validate` and `inspect` report by its `id`, and which a
    /// sound can start at. It lasts as long as its steps, or `min_duration` when that is longer,
    /// holding the stage as its last step left it until then. Its steps are wait and play
    /// steps, not chapters.
    Chapter {
        /// The chapter's id, its own in the scene.
        id: String,
        /// The least the chapter lasts, in seconds, such as the length of the narration it
        /// carries. The default is no least: the sum of its steps.
        min_duration: Option<Duration>,
        /// The chapter's steps, played one after another from its start. At least one.
        #[schemars(length(min = 1))]
        steps: Vec<StepFile>,
    },
    /// Plays every animation in `animations`, each over its span, which is the whole step
    /// unless the animation sets `start` or `end`.
    Play {
        /// How long the step lasts, in seconds, more than 0.
        duration: Duration,
        /// Each plays over its span: the whole step, unless it sets `start` or `end`. Two
        /// animations whose spans overlap cannot change one value of one object, or of the
        /// camera, but one can play after the other in the same step.
        #[schemars(length(min = 1))]
        animations: Vec<AnimationFile>,
    },
}

/// The file form of an `Animation`. The change's fields sit beside `easing_curve`, tagged by
/// `type`.
#[derive(Deserialize, JsonSchema)]
pub(super) struct AnimationFile {
    /// How the animation's progress follows the elapsed share of its span: `linear`,
    /// `slow-start`, `slow-end`, `slow-both` or `overshoot`. The default is `slow-both`. A follow
    /// with a `law` takes none, because its law sets its value at every scene time.
    pub(super) easing_curve: Option<EasingCurve>,
    /// When the animation starts, in seconds from the start of its step, such as the time a
    /// narrator says the word it shows. The default is 0.
    pub(super) start: Option<SecondsIntoStep>,
    /// When the animation ends, in seconds from the start of its step: more than `start`, and at
    /// most the step's `duration`, which is the default.
    pub(super) end: Option<Duration>,
    #[serde(flatten)]
    pub(super) change: ChangeFile,
}

fn default_fit() -> bool {
    true
}

/// The file form of a `Change`. `object` is a string here and an `ObjectId` past the boundary,
/// which is the one reason this enum is not `Change` itself.
#[derive(Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(super) enum ChangeFile {
    /// Changes the object opacity to `to`. With `copy`, fades only that copy of a pattern: its own
    /// opacity, which the pattern's object opacity applies on top of, so a copy faded to 0 stays
    /// hidden while the pattern fades in.
    Fade {
        /// The object id of the object to fade.
        object: String,
        /// The copy index of one copy of the pattern `object` to change alone, as `inspect`
        /// reports it: `[i, j, k]` for a grid and `[i]` for a ring or an along, each from 0.
        /// Needs sci_anim 0.20.0 or later.
        #[schemars(length(min = 1, max = 3))]
        copy: Option<Vec<u32>>,
        /// The object opacity to end at, from 0 (hidden) to 1 (shown). To fade an object in,
        /// declare it with `opacity` 0 and fade it to 1.
        to: UnitInterval,
    },
    /// Counts a readout's value to `to` over its span, shaped by its easing curve, from the value
    /// it has when the count begins.
    Count {
        /// The object id of a `readout`.
        object: String,
        /// The value to end at.
        to: FiniteNumber,
    },
    /// Moves the object's position to `to`. With `copy`, moves only that copy of a pattern, and
    /// `to` is the copy's position in the pattern's own space, as its layout places it, so a move,
    /// rotate or scale of the whole pattern still carries the moved copy. A copy of a pattern whose
    /// copy expressions read `t` is placed by them and cannot be moved.
    Move {
        /// The object id of the object to move.
        object: String,
        /// The copy index of one copy of the pattern `object` to change alone, as `inspect`
        /// reports it: `[i, j, k]` for a grid and `[i]` for a ring or an along, each from 0.
        /// Needs sci_anim 0.20.0 or later.
        #[schemars(length(min = 1, max = 3))]
        copy: Option<Vec<u32>>,
        /// The position to end at, as `[x, y, z]` in world units, or in data coordinates for a
        /// dot on axes or a sphere on 3D axes.
        #[schemars(with = "[f32; 3]")]
        to: Vec3,
    },
    /// Scales the object to `to` along its own axes.
    Scale {
        /// The object id of the object to scale.
        object: String,
        /// The scale to end at, as a factor along each of the object's own x, y and z axes.
        /// `[1, 1, 1]` is the declared size.
        #[schemars(with = "[f32; 3]")]
        to: Vec3,
    },
    /// Turns the object to the rotation `to`, in degrees. Each angle moves straight to its new
    /// value, so `[0, 360, 0]` from `[0, 0, 0]` turns the object a full turn.
    Rotate {
        /// The object id of the object to turn.
        object: String,
        /// The rotation to end at, as angles in degrees about x, y and z, as in `rotation`.
        /// Values past 360 turn more than once.
        #[schemars(with = "[f32; 3]")]
        to: Vec3,
    },
    /// Draws a flat shape over its span: its stroke along its path by length, then its fill
    /// fading in. A solid grows, and a pattern's copies are created in index order, each starting
    /// a little after the one before. The object shows nothing before the create starts. A molecule cannot be created, and
    /// text and math are written.
    Create {
        /// The object id of a flat shape with a stroke, a solid, a pattern, axes, 3D axes or a number line.
        object: String,
    },
    /// Draws text or math glyph by glyph over its span, a few at a time: each glyph's outline
    /// along its path, then its fill fading in as the outline thins away. The object shows nothing
    /// before the write starts.
    Write {
        /// The object id of a text or math object.
        object: String,
    },
    /// Turns the molecule objects in `reactants` into the molecule objects in `products` over its
    /// span. `atom_map` pairs every reactant atom with the product atom it becomes, each named by
    /// its object id and its atom number in that object's structure file. Each atom moves in a
    /// straight line from its reactant position to its product position, a broken bond thins
    /// away over the first half, and a formed bond thickens over the second. Each product ends at
    /// its own `position` and `scale`, turned by the fit, the rotation that lines it up best with
    /// the reactants, and then by its own `rotation`, so `[0, 0, 0]` keeps the fit. With `fit`
    /// false, each product ends at exactly its own `rotation`. The
    /// reactants show until the reaction begins and the products from its end, so neither needs a
    /// fade. A reactant group lit when the reaction begins keeps its glow on its atoms as they move,
    /// and passes it at the reaction's end to the group of each product that holds exactly the atoms
    /// that product receives from it. Declare that group in the product's `groups`, or it is a
    /// scene error. Two lit groups cannot pass their glow to one product group.
    Reaction {
        /// The object ids of the molecule objects the reaction uses up. At least one.
        #[schemars(length(min = 1))]
        reactants: Vec<String>,
        /// The object ids of the molecule objects the reaction makes. At least one.
        #[schemars(length(min = 1))]
        products: Vec<String>,
        /// The atom map: one pair for every atom of the reactants, hidden hydrogens included,
        /// such as `{ "reactant": { "object": "ethylene", "atom": 1 }, "product": { "object":
        /// "chloroethane", "atom": 2 } }`. Every reactant atom and every product atom appears in
        /// exactly one pair, and the two atoms of a pair are one element. `sci_anim inspect`
        /// lists each molecule's atoms with their numbers, elements and positions.
        #[schemars(length(min = 1))]
        atom_map: Vec<AtomPairFile>,
        /// Whether each product is turned by the fit, the one rotation that lines the products up
        /// best with the reactants, before its own `rotation`. The default is `true`. With
        /// `false`, each product ends at exactly its own `position` and `rotation`, as `inspect`
        /// reports the object after the reaction: for products placed by hand, such as loose
        /// atoms set beside each other, which the fit would turn about their common middle.
        #[serde(default = "default_fit")]
        fit: bool,
    },
    /// Reshapes the flat shape, text, math, plot or tube `object` into `into`'s shape, stroke and
    /// fill, moving it to where `into` is. The source shows until the morph ends and `into` from
    /// then, so neither needs a fade, and `into` is not created or written. The parts of the two
    /// are matched in path order, text glyph to glyph, and a part with no partner grows from or
    /// shrinks to a point. A plot morphs only into a plot on the same axes, and each x of it
    /// moves straight up or down. A tube whose curve is `points` or a `trace` morphs only into
    /// another such tube through as many curve points, each curve point moving straight to its
    /// partner, the k-th to the k-th, and the curve made again through them at every scene time;
    /// its radius and material mix too. Morphing a tube needs sci_anim 0.19.0 or later.
    Morph {
        /// The object id of the source: the flat shape, text, math, plot or tube that is reshaped.
        object: String,
        /// The object id of the target, whose shape, stroke, fill and place the source takes.
        into: String,
    },
    /// Lights the atom group `group` of the molecule `object` with a glow, a see-through shell
    /// around its atoms and the bonds between them, to the share `to`: 1 is all of the glow and 0
    /// is none. It starts from the glow the steps before left, none at first. `color` is the
    /// glow's color, `#ffd23f` unless an earlier highlight set another, and a highlight that
    /// leaves it out keeps the color it has. A glow that is off lights up in `color` from the
    /// start of the span, and only its share changes. Light a group in and out with two
    /// highlights. A group whose atoms are all hidden has no glow to light. A reaction carries a
    /// lit group's glow and passes it to a product group, so a highlight of a group of a
    /// reaction's molecule in the reaction's own step is a conflict. On a table it lights the
    /// cell at `cell` instead: a see-through fill of the cell's rectangle, behind its text and
    /// rulings, to the share `to` (sci_anim 0.18.0 or later). On a pattern it lights the copy at
    /// `copy`, and on a bar plot the bar at `bar` (sci_anim 0.20.0 or later): a see-through box
    /// a little larger than a flat copy, behind its stroke and around its fill, a see-through frame
    /// around a bar as wide as half the gap between two bars, or a see-through shell around a
    /// solid copy.
    Highlight {
        /// The object id of a molecule, a table, a pattern or a bar plot.
        object: String,
        /// The group id of one of the molecule's `groups`. Give it for a molecule, `cell` for a
        /// table, `copy` for a pattern and `bar` for a bar plot.
        group: Option<String>,
        /// The cell address of one cell of the table, `[row, column]`, each counted from 0, such
        /// as `[2, 1]` for the second cell of the third row. Needs sci_anim 0.18.0 or later.
        cell: Option<[u32; 2]>,
        /// The copy index of one copy of the pattern `object` to change alone, as `inspect`
        /// reports it: `[i, j, k]` for a grid and `[i]` for a ring or an along, each from 0.
        /// Needs sci_anim 0.20.0 or later.
        #[schemars(length(min = 1, max = 3))]
        copy: Option<Vec<u32>>,
        /// The index of one bar of the bar plot `object` to change alone, in category order from
        /// 0. Needs sci_anim 0.20.0 or later.
        bar: Option<u32>,
        /// The glow share to end at, from 0 (no glow) to 1 (all of it).
        to: UnitInterval,
        /// The glow's color, written `#rrggbb`. The default is the color the glow already has,
        /// `#ffd23f` at first.
        color: Option<Color>,
    },
    /// Takes the object's colors to new ones over its span, mixed in linear light: a flat shape's
    /// or plot's `stroke` and `fill`, and the `color` of text, math or a solid. Give at least one.
    /// A color left out keeps the color it has, except a fill in the stroke's color, as an arrow's
    /// tip is, which a recolor of the stroke takes along. A `color` recolors every glyph, those that
    /// `\textcolor` colors too. A pattern's colors are its shape's, and each copy still mixes them
    /// toward `to_color`, or its pick of `palette`, by its color share. Axes, number lines and
    /// molecules cannot be recolored. With `copy` or `bar`, recolors only that copy of a pattern or
    /// bar of a bar plot, from the colors it shows when the recolor begins, and the colors it ends
    /// at are its own from then on: a later recolor of the whole object leaves it as it is.
    Recolor {
        /// The object id of a flat shape, plot, text, math, solid or pattern.
        object: String,
        /// The copy index of one copy of the pattern `object` to change alone, as `inspect`
        /// reports it: `[i, j, k]` for a grid and `[i]` for a ring or an along, each from 0.
        /// Needs sci_anim 0.20.0 or later.
        #[schemars(length(min = 1, max = 3))]
        copy: Option<Vec<u32>>,
        /// The index of one bar of the bar plot `object` to change alone, in category order from
        /// 0. Needs sci_anim 0.20.0 or later.
        bar: Option<u32>,
        /// The stroke color to end at, written `#rrggbb`. The object needs a stroke.
        stroke: Option<Color>,
        /// The fill color to end at, written `#rrggbb`. The object needs a fill.
        fill: Option<Color>,
        /// The color of text, math or a sphere to end at, written `#rrggbb`.
        color: Option<Color>,
    },
    /// Moves the camera's position to `to`. It keeps looking at its target.
    CameraMove {
        /// The camera position to end at, as `[x, y, z]` in world units.
        #[schemars(with = "[f32; 3]")]
        to: Vec3,
    },
    /// Turns the camera to look at `to`.
    CameraLook {
        /// The camera target to end at, as `[x, y, z]` in world units.
        #[schemars(with = "[f32; 3]")]
        to: Vec3,
    },
    /// Changes the camera's vertical field of view to `to` degrees. Smaller zooms in.
    CameraZoom {
        /// The vertical field of view to end at, in degrees, more than 0 and less than 180.
        to: FieldOfView,
    },
    /// Carries every point of each object in `objects` by one 2x2 `matrix` about `origin`, in the
    /// world's xy plane, in world units with y up, over its span: a point `p` goes to
    /// `origin + matrix · (p − origin)`. Every point of every object listed follows the same
    /// matrix at every scene time, so a circle and the radii drawn to it stay together
    /// throughout, which a morph does not promise. A later linear map acts after an earlier one,
    /// so B after A is B·A, and `inspect` reports each named object's `plane_map`. A stroke keeps
    /// its declared width under any matrix. An arrow is drawn between its two mapped ends with its
    /// tip unsheared, as long as its own scale makes it, and draws nothing while they meet. A dot
    /// moves to its mapped center and keeps its size, so it stays visible under the zero matrix.
    /// Text, math and axes are mapped like any path, so a shear slants their glyphs. A plot is
    /// placed by its axes, so name the axes, and an attached object sits beside the object it is
    /// attached to. Solids, molecules, patterns, plots, attached objects and objects turned about
    /// x or y when the map begins cannot be named. From the start of the
    /// first linear map that names an object, a move, scale, rotate or morph of it is a scene
    /// error, and so is one that is still playing when the map begins: the map keeps the
    /// object's transform from then on. A fade, recolor, create or write of it is fine.
    LinearMap {
        /// The object ids of the flat shapes, dots, text, math, axes and number lines to carry.
        /// At least one, each once.
        #[schemars(length(min = 1))]
        objects: Vec<String>,
        /// The map origin, the point the matrix acts about, as `[x, y]` in world units. It stays
        /// where it is. The default is `[0, 0]`.
        origin: Option<[OriginCoordinate; 2]>,
        /// The matrix, row by row: `[[a, b], [c, d]]` sends a point `(x, y)` measured from
        /// `origin` to `(a·x + b·y, c·x + d·y)`. Each entry is from -100 to 100.
        /// `[[0, -1], [1, 0]]` turns a quarter turn counterclockwise, `[[1, 1], [0, 1]]` shears
        /// along x, `[[1, 0], [0, -1]]` reflects across the x axis, and `[[1, 0], [0, 0]]`
        /// flattens onto it.
        matrix: [[MatrixEntry; 2]; 2],
        /// How the matrix goes from the identity to all of `matrix` as the progress runs.
        /// `entries`, the default, moves each entry straight from the identity's, so a turn
        /// shrinks along the chord and a half turn passes through the zero matrix. `turn` turns
        /// by the rotation inside the matrix, the shorter way and counterclockwise at exactly
        /// half a turn, while it stretches by the rest, so a rotation keeps every length at every
        /// frame. `turn` needs a matrix whose determinant is more than 0: no reflection and no
        /// flattening.
        #[serde(default)]
        route: MatrixRoute,
    },
    /// Integrates `variables` from their starting values by their `rates` over its span, and
    /// sets each value in `sets` from expressions in the variables at every scene time of it: a
    /// pendulum with drag, a projectile with air resistance, a charging capacitor, predator and
    /// prey, an epidemic, or two masses on springs that share their variables. The rates read the
    /// variables and `t`, law time, which counts from the span or from the chapter `from` names,
    /// as a follow's does. It is integrated once, when the scene file is read, by fourth-order
    /// Runge-Kutta at eight steps a frame, over at most 216000 frames, an hour at 60 frames a
    /// second. A variable with no finite value at a step is
    /// `out_of_range` at its rate, and a set that does not start where its value is, is
    /// `law_jump`. Each value it sets is held from its span's end on, and no other animation
    /// changes it while the simulation plays. A simulation takes no `easing_curve`.
    Simulate {
        /// Each variable's name and its value at the span's start, such as
        /// `{ "angle": 1.2, "spin": 0 }`. A name is letters, digits and `_`, starts with a letter,
        /// and is not `t`, `pi`, `e`, `tau` or a function's name. At least one.
        #[schemars(extend("minProperties" = 1))]
        variables: BTreeMap<String, FiniteNumber>,
        /// How fast each variable changes, per second of law time: one expression in the
        /// variables and `t` for every variable, by its name, such as
        /// `{ "angle": "spin", "spin": "-9.8*sin(angle) - 0.3*spin" }`.
        rates: BTreeMap<String, String>,
        /// The values the simulation sets, each value of an object once. At least one.
        #[schemars(length(min = 1))]
        sets: Vec<SetFile>,
        /// The id of a chapter: law time counts from that chapter's start instead of from this
        /// simulation's span. The simulation's own chapter, or one before it.
        from: Option<String>,
    },
    /// Sets a value at every scene time of its span from a law, or carries an object along
    /// another object's curve, instead of easing it to a `to`. Give `law` or `along`.
    ///
    /// With `law`, `x`, `y` and `z` are expressions in `t`, law time in seconds, or `value` is
    /// one for a readout's value or an object opacity, and `sets` names the value, an object's
    /// `position` by default. A pendulum is a follow of `rotation`, an orbit a follow of
    /// `position`, a pulse a follow of `scale`, and a camera circling a solid a follow of
    /// `camera-position`. The law gives the value outright, so at the span's start it must give
    /// the value the object or camera already has, within 0.001, or the scene file has a
    /// `law_jump` error with both values. The value stays at what the law gives at the span's
    /// end, and a later animation starts from there. Law time counts from the span's start, or
    /// from the start of the chapter `from` names, so follows in several steps continue one
    /// motion. Every expression is evaluated at every frame time of the span, and its end, when
    /// the scene file is read. A law takes no `easing_curve`.
    ///
    /// With `along`, the follow carries `object` along the path of a flat shape or plot, or the
    /// space curve of a tube, by length in that object's own space, over the span and shaped by
    /// `easing_curve`, wherever that object is at each scene time. A carry and a create of a plot
    /// with the same span and easing curve keep the object on the pen's end. `faces: tangent`
    /// also turns the object so its own x axis points along the curve. The object must stand at
    /// the curve's start, and face its tangent there when `faces` is set, or the scene file has a
    /// `carry_jump` error with the position or rotation to declare.
    ///
    /// A follow of an object's position, rotation or scale joins its moves, scales and rotates:
    /// no other animation changes the value it sets while it plays, and a plot, an attached
    /// object and an object a linear map names cannot be followed. A follow of `opacity` joins
    /// its fades the same way, and any object a fade can fade can be followed.
    Follow {
        /// The object id of the object whose value the law sets. Leave it out when `sets` is
        /// `camera-position` or `camera-target`.
        object: Option<String>,
        /// What the law sets: `position`, the object's position in world units, or in data
        /// coordinates for a dot on axes or a sphere on 3D axes, the default;
        /// `rotation`, the object's rotation in degrees about its x, its turned y and its turned
        /// z axis, as in `rotation`; `scale`, the factor along each of the object's own x, y and
        /// z axes, as in `scale`; `opacity`, the object opacity, from the law's one expression,
        /// `value`; `value`, a readout's value, from `value` too; `camera-position`, where the
        /// camera stands; or `camera-target`, the point the camera looks at. A follow with `along` sets the
        /// position, so leave this out. `shape` takes no `law`: it runs the `t` of the object's
        /// own expressions, a function plot's `expression`, a tube's `curve`, a surface's `z`
        /// or a pattern's `copy` and `along` curve, counting law time over the span or from the
        /// chapter `from` names, and holds the shape at the span's end. Its law time must start
        /// at the one the shape is drawn at: 0 before its first follow, then where the last
        /// follow of it ended.
        sets: Option<FollowedFile>,
        /// The law: `x`, `y` and `z` as expressions in `t`, law time in seconds, or `value` alone
        /// for `opacity` and a readout's `value`. A rotation's unused angles are written `"0"`.
        /// Give `law` or `along`, not both.
        law: Option<LawFile>,
        /// The id of a chapter: law time counts from that chapter's start instead of from this
        /// follow's span, so follows in several steps of a chapter continue one motion. The
        /// follow's own chapter, or one before it. Only with `law`.
        from: Option<String>,
        /// The object id of a flat shape, plot or tube to carry `object` along, by length over
        /// the span, from the start of its curve to its end. A pattern has many curves, so it
        /// cannot be named. It must not be carried itself,
        /// attached, or morphed. Give `law` or `along`, not both.
        along: Option<String>,
        /// `tangent` turns the carried object so its own x axis points along the curve at every
        /// scene time. Leave it out to keep the object's rotation. Only with `along`.
        faces: Option<Faces>,
    },
}

/// One value a simulation sets: an object's `position` or `rotation` from three expressions, or a
/// readout's `value` from one. Give exactly one of the three.
#[derive(Deserialize, JsonSchema)]
pub(super) struct SetFile {
    /// The object id of the object whose value this sets.
    pub(super) object: String,
    /// The object's position as three expressions, x, y and z, in the simulation's variables
    /// and `t`, in world units, or in data coordinates for a data point, such as
    /// `["2*sin(angle)", "-2*cos(angle)", "0"]`. It must start where the object is.
    pub(super) position: Option<[String; 3]>,
    /// The object's rotation as three expressions, angles in degrees about its x, y and z axes, in
    /// the simulation's variables and `t`, such as `["0", "0", "angle*180/pi"]`. It must start
    /// where the object is turned.
    pub(super) rotation: Option<[String; 3]>,
    /// A readout's value as one expression in the simulation's variables and `t`, such as
    /// `abs(2*spin)`. It must start at the readout's value.
    pub(super) value: Option<String>,
}

/// The value a follow's law sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum FollowedFile {
    #[default]
    Position,
    Rotation,
    CameraPosition,
    CameraTarget,
    Shape,
    Value,
    Scale,
    Opacity,
}

/// A law: three expressions in `t`, law time in seconds, one for each coordinate of the value a
/// follow sets, such as `{ "x": "0", "y": "0", "z": "30*cos(2.21*t)" }` for a pendulum's swing,
/// or for a readout's value or an object opacity the one expression `value`, such as
/// `{ "value": "9.8*t" }`.
#[derive(Deserialize, JsonSchema)]
pub(super) struct LawFile {
    #[schemars(description = crate::expr::law_description("x"))]
    pub(super) x: Option<String>,
    #[schemars(description = crate::expr::law_description("y"))]
    pub(super) y: Option<String>,
    #[schemars(description = crate::expr::law_description("z"))]
    pub(super) z: Option<String>,
    /// An expression in the one variable `t`, law time in seconds, such as `9.8*t`, which gives a
    /// readout's value, for a follow of `value`, or the object opacity, for a follow of
    /// `opacity`, such as `1 - t/2`. Such a law has no `x`, `y` or `z`. It is evaluated at every
    /// frame time of the follow's span when the scene file is read, and must have a finite value
    /// at each, and an object opacity's must lie within 0 to 1 at each, or the scene file has an
    /// `out_of_range` error. Between frame times an opacity is held within 0 to 1.
    pub(super) value: Option<String>,
}

impl AnimationFile {
    /// The `Animation` read from `path` in a step `duration` long, or the scene errors for a span
    /// that is empty or runs past the step, and for an `object` that names no object or that a
    /// create cannot draw.
    pub(super) fn into_animation(
        self,
        start: &Stage,
        path: &JsonPath,
        duration: Duration,
    ) -> Result<Animation, Vec<SceneError>> {
        let span = (
            self.start.map_or(0.0, SecondsIntoStep::get),
            self.end.map_or(duration.get(), Duration::get),
        );
        let mut errors = Vec::new();
        if span.1 > duration.get() {
            let message = format!(
                "`end` is {} s, and the step lasts {} s; make the step's `duration` at least {}",
                span.1,
                duration.get(),
                span.1
            );
            errors.push(SceneError::new(
                path.field("end"),
                SceneErrorCode::OutOfRange,
                message,
            ));
        } else if span.0 >= span.1 {
            let message = format!(
                "`start` is {} s and the animation ends at {} s, so it would play over no time; `start` must come before `end`",
                span.0, span.1
            );
            errors.push(SceneError::new(
                path.field("start"),
                SceneErrorCode::Invalid,
                message,
            ));
        }
        let change =
            match self.change {
                ChangeFile::Reaction {
                    reactants,
                    products,
                    atom_map,
                    fit,
                } => Reaction::read(path, &reactants, &products, &atom_map, fit, start).map(
                    |reaction| Change::Reaction {
                        reaction: Arc::new(reaction),
                    },
                ),
                ChangeFile::LinearMap {
                    objects,
                    origin,
                    matrix,
                    route,
                } => {
                    let origin = origin.map_or(Vec2::ZERO, |[x, y]| Vec2::new(x.get(), y.get()));
                    read_linear_map(
                        path,
                        &objects,
                        origin,
                        matrix.map(|row| row.map(MatrixEntry::get)),
                        route,
                        start,
                    )
                }
                ChangeFile::Follow {
                    object,
                    sets,
                    law,
                    from,
                    along,
                    faces,
                } => read_follow(
                    path,
                    FollowFields {
                        object,
                        sets,
                        law,
                        from,
                        along,
                        faces,
                    },
                    self.easing_curve.is_some(),
                    start,
                ),
                ChangeFile::Simulate {
                    variables,
                    rates,
                    sets,
                    from,
                } => read_simulate(
                    path,
                    variables,
                    rates,
                    sets,
                    from,
                    self.easing_curve.is_some(),
                    start,
                ),
                change => into_change(change, start, path).map_err(|error| vec![error]),
            };
        match change {
            Ok(change) if errors.is_empty() => Ok(Animation {
                easing_curve: self.easing_curve.unwrap_or_default(),
                change,
                span,
            }),
            Ok(_) => Err(errors),
            Err(found) => Err(errors.into_iter().chain(found).collect()),
        }
    }
}

/// The linear map read from `path`, or every scene error in its `objects` and `route` at once,
/// each object's at its place in `objects`.
fn read_linear_map(
    path: &JsonPath,
    objects: &[String],
    origin: Vec2,
    rows: [[f32; 2]; 2],
    route: MatrixRoute,
    start: &Stage,
) -> Result<Change, Vec<SceneError>> {
    let mut errors = Vec::new();
    let mut object_ids: Vec<ObjectId> = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        let at = path.field("objects").index(i);
        let Some((object_id, declared)) = start.objects.get_key_value(object.as_str()) else {
            errors.push(unknown_object(at, object, start));
            continue;
        };
        if let Some(first) = objects[..i].iter().position(|earlier| earlier == object) {
            let message = format!(
                "`{object_id}` is already at {}, and a linear map names each object once",
                path.field("objects").index(first)
            );
            errors.push(SceneError::new(
                at,
                SceneErrorCode::DuplicateObject,
                message,
            ));
            continue;
        }
        let why = match (&declared.shape, &declared.placed_by) {
            (Shape::Sphere { .. } | Shape::Solid { .. }, _) => Some("is a solid, and a linear map carries flat shapes, dots, text, math and axes in the world's xy plane; move, scale or rotate it instead".to_string()),
            (Shape::Molecule { .. }, _) => Some("is a molecule, and a linear map carries flat shapes, dots, text, math and axes in the world's xy plane; move, scale or rotate it instead".to_string()),
            (Shape::Pattern(_), _) => Some("is a pattern, whose copies may be solids; move, scale or rotate it instead".to_string()),
            (Shape::Trail(trail), _) => Some(format!("is a trail, drawn where `{}` has been; name `{}` instead", trail.object, trail.object)),
            (Shape::Axes3d(_), _) => Some("is 3D axes, which stand in space, not in the world's xy plane; move, scale or rotate them instead".to_string()),
            (_, PlacedBy::Axes(axes)) => Some(format!("is a plot, which is placed by its axes `{axes}`; name `{axes}` instead, and the plot goes with them")),
            (_, PlacedBy::Attachment(attachment)) => Some(format!("is attached to `{}` and sits beside it; name `{}` instead, and `{object_id}` stays beside it wherever the map carries it", attachment.object, attachment.object)),
            (_, PlacedBy::DataPoint(axes)) => Some(format!("is a data point, placed in the data coordinates of `{axes}`; name `{axes}` instead, and the point goes with them")),
            (_, PlacedBy::Ends(_)) => Some("is placed by its ends and lies between them; name the objects its ends sit on instead".to_string()),
            _ => None,
        };
        match why {
            Some(why) => errors.push(SceneError::new(
                at,
                SceneErrorCode::NotMappable,
                format!("`{object_id}` {why}"),
            )),
            None => object_ids.push(object_id.clone()),
        }
    }
    match LinearMap::new(origin, rows, route) {
        Err(NoTurningRoute { determinant }) => {
            let message = format!(
                "`route: turn` turns by the rotation inside the matrix, and this matrix has determinant {determinant}, so it flips or flattens the plane and has no rotation to turn by; leave out `route` to move each entry straight"
            );
            errors.push(SceneError::new(
                path.field("route"),
                SceneErrorCode::NoTurningRoute,
                message,
            ));
        }
        Ok(linear_map) if errors.is_empty() => {
            return Ok(Change::LinearMap {
                object_ids,
                linear_map: Arc::new(linear_map),
            });
        }
        Ok(_) => {}
    }
    Err(errors)
}

/// The simulation read from `path`, or every scene error in it at once, each at its field: a
/// variable's name that an expression cannot read; a rate missing for a variable or written for
/// none; each mistake in each expression; a set's unknown object, a set with none or more than
/// one of `position`, `rotation` and `value`, and a value of an object that is not a readout; and
/// an `easing_curve`.
fn read_simulate(
    path: &JsonPath,
    variables: BTreeMap<String, FiniteNumber>,
    mut rates: BTreeMap<String, String>,
    sets: Vec<SetFile>,
    from: Option<String>,
    easing_curve: bool,
    start: &Stage,
) -> Result<Change, Vec<SceneError>> {
    let mut errors = Vec::new();
    if easing_curve {
        errors.push(SceneError::new(path.field("easing_curve"), SceneErrorCode::Invalid, "a simulation sets its values from its variables at every scene time, so it has no easing curve; leave out `easing_curve`"));
    }
    for name in variables
        .keys()
        .filter(|name| !expr::is_variable_name(name))
    {
        let message = format!(
            "`{name}` cannot name a variable: a name is letters, digits and `_`, starts with a letter, and is not `t`, `pi`, `e`, `tau` or a function's name, which an expression would read as those"
        );
        errors.push(SceneError::new(
            path.field("variables").field(name),
            SceneErrorCode::Invalid,
            message,
        ));
    }
    let names: Vec<&str> = variables
        .keys()
        .map(String::as_str)
        .chain([expr::LAW_TIME])
        .collect();
    // `name` is how a message names the expression: a rate by its variable, a coordinate by its axis.
    let parse = |source: &str, name: &str, at: JsonPath, errors: &mut Vec<SceneError>| {
        Expression::parse(source, &names)
            .map_err(|found| {
                errors.extend(found.into_iter().map(|e| SceneError::new(at.clone(), SceneErrorCode::BadFormat, format!("`{name}` is not an expression sci_anim can read: {} (at column {} of {source:?})", e.message, e.column))));
            })
            .ok()
    };
    let mut read_rates = Vec::new();
    for name in variables.keys() {
        match rates.remove(name) {
            Some(rate) => read_rates.push(parse(&rate, name, path.field("rates").field(name), &mut errors)),
            None => errors.push(SceneError::new(path.field("rates").field(name), SceneErrorCode::MissingField, format!("missing the rate of `{name}`: every variable needs an expression for how fast it changes"))),
        }
    }
    for name in rates.keys() {
        let known: Vec<String> = variables.keys().map(|name| format!("`{name}`")).collect();
        errors.push(SceneError::new(
            path.field("rates").field(name),
            SceneErrorCode::Invalid,
            format!(
                "no variable is named `{name}`; the variables are {}",
                known.join(", ")
            ),
        ));
    }
    let mut read_sets = Vec::new();
    // The set at which each object's position, rotation or value is first set.
    let mut first_sets: BTreeMap<(String, &'static str), usize> = BTreeMap::new();
    for (i, set) in sets.into_iter().enumerate() {
        let at = path.field("sets").index(i);
        let object_id = match start.objects.get_key_value(set.object.as_str()) {
            Some((object_id, _)) => Some(object_id.clone()),
            None => {
                errors.push(unknown_object(at.field("object"), &set.object, start));
                None
            }
        };
        let written = [
            set.position.is_some(),
            set.rotation.is_some(),
            set.value.is_some(),
        ]
        .into_iter()
        .filter(|written| *written)
        .count();
        if written != 1 {
            let (code, message) = if written == 0 {
                (
                    SceneErrorCode::MissingField,
                    "a set needs one of `position`, `rotation` and `value`",
                )
            } else {
                (
                    SceneErrorCode::Invalid,
                    "a set sets one value; give one of `position`, `rotation` and `value`, and write another set for the next",
                )
            };
            errors.push(SceneError::new(at.clone(), code, message));
            continue;
        }
        let field = if set.position.is_some() {
            "position"
        } else if set.rotation.is_some() {
            "rotation"
        } else {
            "value"
        };
        let first = *first_sets.entry((set.object.clone(), field)).or_insert(i);
        if first != i {
            let message = format!(
                "the set at {} already sets the {field} of `{}`, and a simulation sets each value once; write its expressions in one set",
                path.field("sets").index(first),
                set.object
            );
            errors.push(SceneError::new(
                at.clone(),
                SceneErrorCode::Invalid,
                message,
            ));
        }
        let three = |coordinates: [String; 3],
                     field: &str,
                     errors: &mut Vec<SceneError>|
         -> Option<[Expression; 3]> {
            let read: Vec<Option<Expression>> = coordinates
                .iter()
                .zip(["x", "y", "z"])
                .enumerate()
                .map(|(k, (source, axis))| parse(source, axis, at.field(field).index(k), errors))
                .collect();
            read.into_iter()
                .collect::<Option<Vec<Expression>>>()
                .map(|read| {
                    read.try_into()
                        .unwrap_or_else(|_| unreachable!("three expressions"))
                })
        };
        let read = match (set.position, set.rotation, set.value) {
            (Some(position), _, _) => three(position, "position", &mut errors)
                .zip(object_id)
                .map(|(coordinates, object_id)| Set::Position(object_id, coordinates)),
            (_, Some(rotation), _) => three(rotation, "rotation", &mut errors)
                .zip(object_id)
                .map(|(coordinates, object_id)| Set::Rotation(object_id, coordinates)),
            (_, _, Some(value)) => {
                let readout = object_id.and_then(|object_id| {
                    readout_id(&at, &object_id.to_string(), start)
                        .map_err(|error| errors.push(error))
                        .ok()
                });
                parse(&value, "value", at.field("value"), &mut errors)
                    .zip(readout)
                    .map(|(expression, object_id)| Set::Value(object_id, expression))
            }
            (None, None, None) => unreachable!("one is written"),
        };
        read_sets.push(read);
    }
    let rates: Option<Vec<Expression>> = read_rates.into_iter().collect();
    let sets: Option<Vec<Set>> = read_sets.into_iter().collect();
    match (rates, sets) {
        (Some(rates), Some(sets)) if errors.is_empty() => {
            let law_time_start = from.map_or(LawTimeStart::Span, |id| {
                LawTimeStart::Chapter(ChapterId::new(&id))
            });
            let (variables, starts) = variables
                .into_iter()
                .map(|(name, start)| (name, start.get()))
                .unzip();
            Ok(Change::Simulate(Arc::new(Simulate {
                variables,
                starts,
                rates,
                sets,
                law_time_start,
            })))
        }
        _ => Err(errors),
    }
}

/// The object id of the readout `object` names, or the scene error at `object`: an unknown
/// object, or one that is not a readout.
fn readout_id(path: &JsonPath, object: &str, start: &Stage) -> Result<ObjectId, SceneError> {
    let Some((object_id, declared)) = start.objects.get_key_value(object) else {
        return Err(unknown_object(path.field("object"), object, start));
    };
    if declared.readout.is_none() {
        let message = format!(
            "`{object_id}` is not a readout, and only a readout has a value to count or follow; write a `readout`, or move or follow `{object_id}`'s position"
        );
        return Err(SceneError::new(
            path.field("object"),
            SceneErrorCode::NotReadout,
            message,
        ));
    }
    Ok(object_id.clone())
}

/// The scene error at `at` for an `object` that names no object.
fn unknown_object(at: JsonPath, object: &str, start: &Stage) -> SceneError {
    let ids: Vec<String> = start.objects.keys().map(|id| format!("`{id}`")).collect();
    let mut message = format!(
        "no object has the id `{object}`; the objects are {}",
        ids.join(", ")
    );
    if object == "camera" {
        message.push_str("; to change the camera, use an animation of type `camera-move`, `camera-look` or `camera-zoom`, or a `follow` whose `sets` is `camera-position` or `camera-target`");
    }
    SceneError::new(at, SceneErrorCode::UnknownObject, message)
}

/// A follow's fields as the scene file writes them, before `read_follow` checks how they go
/// together.
pub(super) struct FollowFields {
    pub(super) object: Option<String>,
    pub(super) sets: Option<FollowedFile>,
    pub(super) law: Option<LawFile>,
    pub(super) from: Option<String>,
    pub(super) along: Option<String>,
    pub(super) faces: Option<Faces>,
}

/// The follow read from `path`, or every scene error in it at once, each at its field: an
/// `object` with the camera, or none without it; both `law` and `along`, or neither; `from` or an
/// `easing_curve` with no law, `faces` with no `along`, and `sets` with `along`; each mistake in
/// each of the law's expressions, at that expression with its column; and an `along` object that
/// is the object, attached, or has no one curve.
fn read_follow(
    path: &JsonPath,
    written: FollowFields,
    easing_curve_written: bool,
    start: &Stage,
) -> Result<Change, Vec<SceneError>> {
    let FollowFields {
        object,
        sets: sets_written,
        law,
        from,
        along,
        faces,
    } = written;
    let sets = sets_written.unwrap_or_default();
    if sets == FollowedFile::Shape {
        return read_shape_follow(
            path,
            object,
            law.is_some(),
            from,
            along.is_some(),
            faces.is_some(),
            easing_curve_written,
            start,
        );
    }
    let mut errors = Vec::new();
    let mut refuse = |field: &str, code, message: &str| {
        errors.push(SceneError::new(path.field(field), code, message))
    };
    match (&law, &along) {
        (Some(_), Some(_)) => refuse(
            "along",
            SceneErrorCode::Invalid,
            "a follow sets its value from `law` or carries its object `along` a curve; give one of them, not both",
        ),
        (None, None) => refuse(
            "law",
            SceneErrorCode::MissingField,
            "a follow needs `law`, `x`, `y` and `z` as expressions in `t`, law time in seconds, or `along`, the object id of a flat shape, plot or tube to carry its object along",
        ),
        (Some(_), None) => {
            if easing_curve_written {
                refuse(
                    "easing_curve",
                    SceneErrorCode::Invalid,
                    "a follow with a law sets its value from the law at every scene time, so it has no easing curve; leave out `easing_curve`",
                );
            }
            if faces.is_some() {
                refuse(
                    "faces",
                    SceneErrorCode::Invalid,
                    "`faces` turns an object a follow carries `along` a curve, and this follow has a law; set `rotation` with a second follow instead",
                );
            }
        }
        (None, Some(_)) => {
            if from.is_some() {
                refuse(
                    "from",
                    SceneErrorCode::Invalid,
                    "`from` says where a law's law time counts from, and a follow with `along` has no law; leave out `from`",
                );
            }
            if sets_written.is_some_and(|sets| sets != FollowedFile::Position) {
                refuse(
                    "sets",
                    SceneErrorCode::Invalid,
                    "a follow with `along` carries an object's position, and `faces: tangent` turns it too; leave out `sets`, and set a rotation or the camera with a law",
                );
            }
        }
    }
    let of_camera = matches!(
        sets,
        FollowedFile::CameraPosition | FollowedFile::CameraTarget
    );
    let object_id = match (object, of_camera) {
        (Some(_), true) => {
            errors.push(SceneError::new(path.field("object"), SceneErrorCode::Invalid, "a follow of `camera-position` or `camera-target` sets the camera, so leave out `object`"));
            None
        }
        (None, false) => {
            let message = "a follow of `position`, `rotation`, `scale`, `opacity` or `value` needs `object`, the object id of the object to set; to set the camera, write `\"sets\": \"camera-position\"` or `\"camera-target\"`";
            errors.push(SceneError::new(
                path.field("object"),
                SceneErrorCode::MissingField,
                message,
            ));
            None
        }
        (Some(object), false) if sets == FollowedFile::Value => {
            match readout_id(path, &object, start) {
                Ok(object_id) => Some(object_id),
                Err(error) => {
                    errors.push(error);
                    None
                }
            }
        }
        (Some(object), false) => match start.objects.get_key_value(object.as_str()) {
            Some((object_id, _)) => Some(object_id.clone()),
            None => {
                errors.push(unknown_object(path.field("object"), &object, start));
                None
            }
        },
        (None, true) => None,
    };
    if let (None, Some(along)) = (&law, along) {
        return match (
            read_carry(path, object_id.as_ref(), &along, faces, start),
            object_id,
        ) {
            (Ok(carry), Some(object_id)) if errors.is_empty() => {
                Ok(Change::Follow(Follow::Carry {
                    object_id,
                    carry: Arc::new(carry),
                }))
            }
            (Ok(_), _) => Err(errors),
            (Err(error), _) => Err(errors.into_iter().chain([error]).collect()),
        };
    }
    let Some(law) = law else { return Err(errors) };
    let zero = "0".to_string();
    let one_number = match sets {
        FollowedFile::Value => Some("a law of `value` sets a readout's one number"),
        FollowedFile::Opacity => Some("a law of `opacity` sets the object opacity"),
        _ => None,
    };
    let written: [(&str, Option<&String>); 3] = if let Some(sets_what) = one_number {
        for (field, extra) in [("law/x", &law.x), ("law/y", &law.y), ("law/z", &law.z)] {
            if extra.is_some() {
                let message = format!(
                    "{sets_what} from its one expression, `value`; leave out `x`, `y` and `z`"
                );
                errors.push(SceneError::new(
                    fields(path, field),
                    SceneErrorCode::Invalid,
                    message,
                ));
            }
        }
        [
            ("law/value", law.value.as_ref()),
            ("law/y", Some(&zero)),
            ("law/z", Some(&zero)),
        ]
    } else {
        if law.value.is_some() {
            errors.push(SceneError::new(fields(path, "law/value"), SceneErrorCode::Invalid, "`value` is the one expression of a law that sets a readout's value, with `\"sets\": \"value\"`, or an object opacity, with `\"sets\": \"opacity\"`; this law sets three numbers from `x`, `y` and `z`"));
        }
        [
            ("law/x", law.x.as_ref()),
            ("law/y", law.y.as_ref()),
            ("law/z", law.z.as_ref()),
        ]
    };
    let mut coordinates = Vec::new();
    for (field, source) in written {
        let Some(source) = source else {
            errors.push(SceneError::new(
                fields(path, field),
                SceneErrorCode::MissingField,
                format!("missing field `{}`", field.trim_start_matches("law/")),
            ));
            continue;
        };
        match Expression::parse(source, expr::LAW) {
            Ok(expression) => coordinates.push(expression),
            Err(found) => {
                errors.extend(expression_errors(field, source)(found).into_iter().map(
                    |(field, code, message)| SceneError::new(fields(path, field), code, message),
                ))
            }
        }
    }
    let (Ok(coordinates), true) = (<[Expression; 3]>::try_from(coordinates), errors.is_empty())
    else {
        return Err(errors);
    };
    let object_id = || {
        object_id
            .clone()
            .expect("an object follow without an object is refused above")
    };
    let (followed, kind) = match sets {
        FollowedFile::Position => (Followed::Position(object_id()), LawKind::Position),
        FollowedFile::Rotation => (Followed::Rotation(object_id()), LawKind::Rotation),
        FollowedFile::CameraPosition => (Followed::CameraPosition, LawKind::Position),
        FollowedFile::CameraTarget => (Followed::CameraTarget, LawKind::Position),
        FollowedFile::Shape => unreachable!("`read_shape_follow` reads a follow of a shape"),
        FollowedFile::Value => (Followed::Value(object_id()), LawKind::Value),
        FollowedFile::Scale => (Followed::Scale(object_id()), LawKind::Scale),
        FollowedFile::Opacity => (Followed::Opacity(object_id()), LawKind::Opacity),
    };
    let law_time_start = from.map_or(LawTimeStart::Span, |id| {
        LawTimeStart::Chapter(ChapterId::new(&id))
    });
    Ok(Change::Follow(Follow::Law {
        followed,
        law: Arc::new(Law::new(coordinates, kind)),
        law_time_start,
    }))
}

/// A follow of a shape, or every scene error in it at once: a `law`, `along`, `faces` or
/// `easing_curve`, which a follow of a shape has none of, no `object`, and an object whose shape
/// reads no `t`.
#[allow(clippy::too_many_arguments)]
fn read_shape_follow(
    path: &JsonPath,
    object: Option<String>,
    law: bool,
    from: Option<String>,
    along: bool,
    faces: bool,
    easing_curve: bool,
    start: &Stage,
) -> Result<Change, Vec<SceneError>> {
    let mut errors = Vec::new();
    for (written, field) in [
        (law, "law"),
        (along, "along"),
        (faces, "faces"),
        (easing_curve, "easing_curve"),
    ] {
        if written {
            let message = format!(
                "a follow of `shape` runs the `t` in the object's own expressions over its span, so it takes no `{field}`; leave it out"
            );
            errors.push(SceneError::new(
                path.field(field),
                SceneErrorCode::Invalid,
                message,
            ));
        }
    }
    let object_id = match object {
        None => {
            errors.push(SceneError::new(path.field("object"), SceneErrorCode::MissingField, "a follow of `shape` needs `object`, the object id of the object whose shape reads `t`"));
            None
        }
        Some(object) => match start.objects.get_key_value(object.as_str()) {
            None => {
                errors.push(unknown_object(path.field("object"), &object, start));
                None
            }
            Some((object_id, declared)) if declared.shape_law.is_none() => {
                let message = format!(
                    "`{object_id}` has no expression that reads `t`, law time, so a follow of its shape changes nothing; write `t` into a function plot's `expression`, a tube's `curve`, a surface's `z` or a pattern's `copy`, or follow its position or rotation instead"
                );
                errors.push(SceneError::new(
                    path.field("object"),
                    SceneErrorCode::Invalid,
                    message,
                ));
                None
            }
            Some((object_id, _)) => Some(object_id.clone()),
        },
    };
    match object_id {
        Some(object_id) if errors.is_empty() => {
            let law_time_start = from.map_or(LawTimeStart::Span, |id| {
                LawTimeStart::Chapter(ChapterId::new(&id))
            });
            Ok(Change::Follow(Follow::Shape {
                object_id,
                law_time_start,
            }))
        }
        _ => Err(errors),
    }
}

/// The carry along the object `along` names, of `object_id` when `object` named one, or the scene
/// error at `along`: an object that is the carried object, attached, or one with no one curve to
/// carry along.
fn read_carry(
    path: &JsonPath,
    object_id: Option<&ObjectId>,
    along: &str,
    faces: Option<Faces>,
    start: &Stage,
) -> Result<Carry, SceneError> {
    let at = path.field("along");
    let Some((along_id, along_object)) = start.objects.get_key_value(along) else {
        return Err(unknown_object(at, along, start));
    };
    let refuse = |message: String| {
        Err(SceneError::new(
            at.clone(),
            SceneErrorCode::Invalid,
            message,
        ))
    };
    if along_object.shape_law.is_some() {
        return refuse(format!(
            "`{along_id}`'s shape reads `t`, law time, and a carry goes along the curve an object declares, which a follow of its shape would change; carry along an object whose shape reads no `t`"
        ));
    }
    match &along_object.placed_by {
        _ if Some(along_id) == object_id => {
            return refuse(format!(
                "a follow carries `{along_id}` along another object's curve, not its own"
            ));
        }
        PlacedBy::Axes(axes) if Some(axes) == object_id => {
            return refuse(format!(
                "`{along_id}` is {} placed by `{axes}`, so carrying `{axes}` along it would move the curve it goes along",
                along_object.shape.what_axes_place()
            ));
        }
        PlacedBy::Attachment(attachment) => {
            return refuse(format!(
                "`{along_id}` is attached to `{}` and sits beside it on the frame, so it has no curve of its own in the world to carry along",
                attachment.object
            ));
        }
        PlacedBy::Ends(_) => {
            return refuse(format!(
                "`{along_id}` is placed by its ends and drawn again between them at every scene time, so it has no curve of its own to carry along; carry along a shape placed by its own transform"
            ));
        }
        PlacedBy::DataPoint(axes) if matches!(along_object.shape, Shape::Flat { .. }) => {
            return refuse(format!(
                "`{along_id}` is a data point on `{axes}`, whose disc marks a point rather than a curve to carry along; carry along a plot on `{axes}` instead"
            ));
        }
        PlacedBy::Itself | PlacedBy::Axes(_) | PlacedBy::DataPoint(_) => {}
    }
    Carry::of(along_id.clone(), &along_object.shape, faces).ok_or_else(|| {
        let what = match &along_object.shape {
            Shape::Glyphs { .. } => "text or math, whose glyphs are many curves",
            Shape::Trail(_) => "a trail, which grows as its object moves",
            Shape::Axes(_) => "axes",
            Shape::BarPlot(_) => "a bar plot, whose bars are many shapes",
            Shape::Table(_) => "a table, whose glyphs and rulings are many curves",
            Shape::NumberLine(_) => "a number line",
            Shape::Axes3d(_) => "3D axes",
            Shape::Sphere { .. } => "a sphere",
            Shape::Solid { solid: Solid::Surface { .. }, .. } => "a surface",
            Shape::Solid { .. } => "a prism or a revolution",
            Shape::Molecule { .. } => "a molecule",
            Shape::Pattern(_) => "a pattern, whose copies are many curves",
            Shape::Flat { .. } | Shape::Mix(_) => unreachable!("a flat shape has a curve, and a mix is staged only while a morph plays"),
        };
        SceneError::new(at, SceneErrorCode::NoCurve, format!("`{along_id}` is {what}, which has no one curve to carry along; carry along a flat shape, a plot or a tube"))
    })
}

/// The path of `field`, several fixed names joined by `/`, under `path`.
pub(super) fn fields(path: &JsonPath, field: &str) -> JsonPath {
    field
        .split('/')
        .fold(path.clone(), |path, name| path.field(name))
}

/// How many curve points `shape`, one end of a morph that names a tube, passes through, or the
/// scene error at `field` when it is not a tube a morph can reshape: one through curve points,
/// not on 3D axes.
fn tube_morph(
    shape: &Shape,
    id: &ObjectId,
    field: &str,
    path: &JsonPath,
) -> Result<usize, SceneError> {
    let refuse = |message: String| {
        Err(SceneError::new(
            path.field(field),
            SceneErrorCode::NotMorphable,
            message,
        ))
    };
    let Shape::Solid {
        solid: Solid::Tube { curve, cut, .. },
        ..
    } = shape
    else {
        let what = match shape {
            Shape::Flat { .. } | Shape::Glyphs { .. } => "a flat shape, text, math or a plot",
            Shape::Sphere { .. } | Shape::Solid { .. } => "another solid",
            Shape::Molecule { .. } => "a molecule",
            Shape::Pattern(_) => "a pattern",
            Shape::Table(_) => "a table",
            _ => "axes, 3D axes or a number line",
        };
        return refuse(format!(
            "a tube morphs only into a tube, and a tube is morphed only from a tube, curve point by curve point, and `{id}` is {what}; fade one out and the other in instead"
        ));
    };
    if cut.is_some() {
        return refuse(format!(
            "`{id}` is a tube on 3D axes, and a morph reshapes only a tube placed by its own transform; fade one out and the other in instead"
        ));
    }
    match curve.curve_points() {
        Some(points) => Ok(points.len()),
        None => refuse(format!(
            "`{id}`'s curve is written as expressions in `u`, and a tube morphs curve point by curve point; write its curve as `points` or a `trace`"
        )),
    }
}

/// The change of any animation but a reaction or a linear map, which
/// `AnimationFile::into_animation` reads.
fn into_change(change: ChangeFile, start: &Stage, path: &JsonPath) -> Result<Change, SceneError> {
    let object_id = |field: &str, object: String| {
        start
            .objects
            .get_key_value(object.as_str())
            .map(|(id, _)| id.clone())
            .ok_or_else(|| unknown_object(path.field(field), &object, start))
    };
    let change = match change {
        ChangeFile::Fade { object, copy, to } => {
            let object_id = object_id("object", object)?;
            let copy = MemberFields {
                copy,
                ..MemberFields::default()
            };
            Change::Fade {
                member: read_member(&object_id, copy, start, path)?,
                object_id,
                to,
            }
        }
        ChangeFile::Count { object, to } => Change::Count {
            object_id: readout_id(path, &object, start)?,
            to: to.get(),
        },
        ChangeFile::Move { object, copy, to } => {
            let object_id = object_id("object", object)?;
            let copy = MemberFields {
                copy,
                ..MemberFields::default()
            };
            let member = read_member(&object_id, copy, start, path)?;
            if member.is_some() && start.objects[&object_id].shape_law.is_some() {
                let message = format!(
                    "the copy expressions of `{object_id}` read `t`, so they place each copy at every scene time; move the whole pattern, or give the copy's motion in its expressions"
                );
                return Err(SceneError::new(
                    path.field("copy"),
                    SceneErrorCode::Invalid,
                    message,
                ));
            }
            Change::Move {
                object_id,
                member,
                to,
            }
        }
        ChangeFile::Scale { object, to } => Change::Scale {
            object_id: object_id("object", object)?,
            to,
        },
        ChangeFile::Rotate { object, to } => Change::Rotate {
            object_id: object_id("object", object)?,
            to,
        },
        ChangeFile::Create { object } => {
            let object_id = object_id("object", object)?;
            let cannot_create = match &start.objects[&object_id].shape {
                Shape::Molecule { .. } => {
                    Some((SceneErrorCode::NotFlat, "is a molecule; fade it in instead"))
                }
                Shape::Trail(_) => Some((
                    SceneErrorCode::Invalid,
                    "is a trail, which draws nothing new but grows as its object moves; fade it in instead",
                )),
                Shape::Flat { path, .. } if path.under_map() == UnderMap::CenterOnly => Some((
                    SceneErrorCode::NoStroke,
                    "is a dot, which has no stroke; fade it in instead",
                )),
                Shape::Flat { stroke: None, .. } => Some((
                    SceneErrorCode::NoStroke,
                    "has no stroke; give it one, or fade it in instead",
                )),
                Shape::Glyphs { .. } => Some((
                    SceneErrorCode::UseWrite,
                    "is text or math; draw it with `write` instead",
                )),
                Shape::Table(_) => Some((
                    SceneErrorCode::UseWrite,
                    "is a table; draw its rulings and write its cells with `write` instead",
                )),
                Shape::Flat {
                    stroke: Some(_), ..
                }
                | Shape::Axes(_)
                | Shape::BarPlot(_)
                | Shape::NumberLine(_)
                | Shape::Axes3d(_)
                | Shape::Sphere { .. }
                | Shape::Solid { .. }
                | Shape::Pattern(_) => None,
                Shape::Mix(_) => unreachable!("a mix is staged only while a morph plays"),
            };
            if let Some((code, why)) = cannot_create {
                let message = format!(
                    "`create` draws a stroke along a flat shape's path or grows a solid, and `{object_id}` {why}"
                );
                return Err(SceneError::new(path.field("object"), code, message));
            }
            Change::Create { object_id }
        }
        ChangeFile::Write { object } => {
            let object_id = object_id("object", object)?;
            if !matches!(
                start.objects[&object_id].shape,
                Shape::Glyphs { .. } | Shape::Table(_)
            ) {
                let message = format!(
                    "`write` draws text, math or a table glyph by glyph, and `{object_id}` is none of them; use `create` for a flat shape"
                );
                return Err(SceneError::new(
                    path.field("object"),
                    SceneErrorCode::NotText,
                    message,
                ));
            }
            Change::Create { object_id }
        }
        ChangeFile::Morph { object, into } => {
            let (source, target) = (object_id("object", object)?, object_id("into", into)?);
            if source == target {
                return Err(SceneError::new(
                    path.field("into"),
                    SceneErrorCode::DuplicateObject,
                    format!("`{source}` cannot morph into itself"),
                ));
            }
            let is_tube = |id: &ObjectId| {
                matches!(
                    start.objects[id].shape,
                    Shape::Solid {
                        solid: Solid::Tube { .. },
                        ..
                    }
                )
            };
            if is_tube(&source) || is_tube(&target) {
                // A shape that is not a tube can be a source, so a tube is the wrong target for it.
                let field = if is_tube(&source) { "object" } else { "into" };
                let from = tube_morph(&start.objects[&source].shape, &source, field, path)?;
                let to = tube_morph(&start.objects[&target].shape, &target, "into", path)?;
                if from != to {
                    let message = format!(
                        "`{source}` passes through {from} curve points and `{target}` through {to}, and a tube morphs curve point by curve point, the k-th to the k-th; give both as many"
                    );
                    return Err(SceneError::new(
                        path.field("into"),
                        SceneErrorCode::WrongLength,
                        message,
                    ));
                }
            }
            let end_of = |id: &ObjectId, field: &str| {
                MorphEnd::of(&start.objects[id].shape).ok_or_else(|| {
                    let what = match &start.objects[id].shape {
                        Shape::Sphere { .. } | Shape::Solid { .. } => "a solid; fade it out and the other in instead",
                        Shape::Molecule { .. } => "a molecule; a `reaction` turns a molecule into another",
                        Shape::Pattern(_) => "a pattern; fade it out and the other in instead",
                        _ => "axes, 3D axes or a number line; a morph reshapes flat shapes, text, math and plots",
                    };
                    SceneError::new(path.field(field), SceneErrorCode::NotMorphable, format!("a morph reshapes a flat shape, text, math, a plot or a tube through curve points, and `{id}` is {what}"))
                })
            };
            end_of(&source, "object")?;
            end_of(&target, "into")?;
            if let Some((id, field)) = [(&source, "object"), (&target, "into")]
                .into_iter()
                .find(|(id, _)| start.objects[*id].shape_law.is_some())
            {
                let message = format!(
                    "`{id}`'s shape reads `t`, law time, and a morph cannot reshape a shape that changes over time yet; fade one out and the other in instead"
                );
                return Err(SceneError::new(
                    path.field(field),
                    SceneErrorCode::Invalid,
                    message,
                ));
            }
            let (on_source, on_target) = (
                &start.objects[&source].placed_by,
                &start.objects[&target].placed_by,
            );
            if let Some((id, field)) = [(&source, "object"), (&target, "into")]
                .into_iter()
                .find(|(id, _)| matches!(start.objects[*id].placed_by, PlacedBy::Ends(_)))
            {
                let message = format!(
                    "`{id}` is placed by its ends and lies between them, and a morph moves its source to where its target is; fade one out and the other in instead"
                );
                return Err(SceneError::new(
                    path.field(field),
                    SceneErrorCode::PlacedByEnds,
                    message,
                ));
            }
            if let Some(ended) = start.objects.iter().find_map(|(id, o)| match &o.placed_by {
                PlacedBy::Ends(ends)
                    if [&ends.from, &ends.to].into_iter().any(
                        |end| matches!(end, End::OnPlot { object, .. } | End::OnCurve { object, .. } if *object == source),
                    ) =>
                {
                    Some(id)
                }
                _ => None,
            }) {
                let message = format!(
                    "an end of `{ended}` sits on `{source}`'s curve, which a morph would reshape into another shape; fade `{source}` out and `{target}` in instead"
                );
                return Err(SceneError::new(
                    path.field("object"),
                    SceneErrorCode::NotMorphable,
                    message,
                ));
            }
            if matches!(on_source, PlacedBy::Attachment(_))
                || matches!(on_target, PlacedBy::Attachment(_))
            {
                let message = "a morph moves its source to where its target is, and an attached object sits beside the object it is attached to, so neither can be a source or a target".to_string();
                return Err(SceneError::new(
                    path.field("into"),
                    SceneErrorCode::NotMorphable,
                    message,
                ));
            }
            if let Some((id, field)) = [(&source, "object"), (&target, "into")]
                .into_iter()
                .find(|(id, _)| matches!(start.objects[*id].placed_by, PlacedBy::DataPoint(_)))
            {
                let message = format!(
                    "`{id}` is a data point, whose position is in its axes' data coordinates, and a morph moves its source to where its target is; move the data point, or fade one out and the other in instead"
                );
                return Err(SceneError::new(
                    path.field(field),
                    SceneErrorCode::NotMorphable,
                    message,
                ));
            }
            if on_source != on_target {
                let message = format!(
                    "a plot morphs only into a plot on the same axes, and `{source}` and `{target}` are not on the same axes"
                );
                return Err(SceneError::new(
                    path.field("into"),
                    SceneErrorCode::NotMorphable,
                    message,
                ));
            }
            Change::Morph {
                source,
                target,
                between_plots: matches!(on_source, PlacedBy::Axes(_)),
            }
        }
        ChangeFile::Highlight {
            object,
            group,
            cell,
            copy,
            bar,
            to,
            color,
        } => {
            let object_id = object_id("object", object)?;
            let written = MemberFields {
                group,
                cell,
                copy,
                bar,
            };
            let Some(member) = read_member(&object_id, written, start, path)? else {
                return Err(no_member(&object_id, start, path));
            };
            Change::Highlight {
                object_id,
                member,
                to: Glow {
                    share: to,
                    color: color.unwrap_or(Glow::DEFAULT_COLOR),
                },
                keeps_color: color.is_none(),
            }
        }
        ChangeFile::Recolor {
            object,
            copy,
            bar,
            stroke,
            fill,
            color,
        } => {
            let object_id = object_id("object", object)?;
            let written = MemberFields {
                copy,
                bar,
                ..MemberFields::default()
            };
            let member = read_member(&object_id, written, start, path)?;
            if stroke.is_none() && fill.is_none() && color.is_none() {
                let message = "a recolor needs at least one of `stroke`, `fill` and `color`";
                return Err(SceneError::new(
                    path.field("stroke"),
                    SceneErrorCode::MissingField,
                    message,
                ));
            }
            let refuse = |field: &str, why: &str| {
                Err(SceneError::new(
                    path.field(field),
                    SceneErrorCode::NotRecolorable,
                    format!("`{object_id}` {why}"),
                ))
            };
            // A pattern's colors are its shape's.
            let shape = match &start.objects[&object_id].shape {
                Shape::Pattern(pattern) => &*pattern.shape,
                shape => shape,
            };
            match shape {
                Shape::Trail(_) => {
                    if let Some(field) = [("fill", fill), ("color", color)]
                        .into_iter()
                        .find_map(|(field, c)| c.map(|_| field))
                    {
                        return refuse(field, "is a trail, whose one color is its `stroke`");
                    }
                }
                Shape::Flat {
                    stroke: has_stroke,
                    fill: has_fill,
                    ..
                } => {
                    if color.is_some() {
                        return refuse(
                            "color",
                            "is a flat shape, whose colors are its `stroke` and `fill`",
                        );
                    }
                    if stroke.is_some() && has_stroke.is_none() {
                        return refuse("stroke", "has no stroke to recolor");
                    }
                    if fill.is_some() && has_fill.is_none() {
                        return refuse("fill", "has no fill to recolor");
                    }
                }
                Shape::BarPlot(bar_plot) => {
                    if color.is_some() {
                        return refuse(
                            "color",
                            "is a bar plot, whose colors are its bars' `stroke` and `fill`",
                        );
                    }
                    if stroke.is_some() && bar_plot.stroke.is_none() {
                        return refuse("stroke", "has no stroke to recolor");
                    }
                }
                Shape::Table(table) => {
                    if fill.is_some() {
                        return refuse(
                            "fill",
                            "is a table, whose colors are its cells' `color` and its rulings' `stroke`; a `highlight` lights one cell",
                        );
                    }
                    if stroke.is_some() && table.rulings.is_none() {
                        return refuse("stroke", "has no rulings to recolor");
                    }
                }
                Shape::Glyphs { .. } | Shape::Sphere { .. } | Shape::Solid { .. } => {
                    if let Some(field) = [("stroke", stroke), ("fill", fill)]
                        .into_iter()
                        .find_map(|(field, c)| c.map(|_| field))
                    {
                        return refuse(
                            field,
                            "is text, math or a solid, whose one color is `color`",
                        );
                    }
                }
                Shape::Molecule { .. }
                | Shape::Axes(_)
                | Shape::NumberLine(_)
                | Shape::Axes3d(_) => {
                    return refuse(
                        "object",
                        "is a molecule, axes, 3D axes or a number line, which cannot be recolored; a `highlight` lights an atom group of a molecule",
                    );
                }
                Shape::Mix(_) => unreachable!("a mix is staged only while a morph plays"),
                Shape::Pattern(_) => unreachable!("a pattern's shape is a flat shape or a solid"),
            }
            Change::Recolor {
                object_id,
                member,
                stroke,
                fill,
                color,
            }
        }
        ChangeFile::Reaction { .. } => unreachable!("`into_animation` reads a reaction"),
        ChangeFile::LinearMap { .. } => unreachable!("`into_animation` reads a linear map"),
        ChangeFile::Follow { .. } => unreachable!("`into_animation` reads a follow"),
        ChangeFile::Simulate { .. } => unreachable!("`into_animation` reads a simulation"),
        ChangeFile::CameraMove { to } => Change::CameraMove { to },
        ChangeFile::CameraLook { to } => Change::CameraLook { to },
        ChangeFile::CameraZoom { to } => Change::CameraZoom { to },
    };
    Ok(change)
}
