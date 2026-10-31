//! The objects a scene file declares: every shape's fields, as `sci_anim schema` prints them.

use super::*;

/// One object: its shape, picked by `type`, with the shape's fields, beside its `position`,
/// `rotation`, `scale` and `opacity`.
#[derive(Deserialize, JsonSchema)]
pub(super) struct ObjectFile {
    #[serde(flatten)]
    pub(super) transform: Transform,
    /// The object opacity: 1 shows the object, 0 hides it, and between the two the object shows
    /// as one faded picture of itself. A fade animates it. A fade in starts from an object
    /// declared with opacity 0.
    #[serde(default = "UnitInterval::one")]
    pub(super) opacity: UnitInterval,
    /// Places this object beside another object, or beside an atom group of a molecule, so it
    /// stays there through that object's moves, rotates and scales and through camera moves: one
    /// `move` of a box carries the number attached to it. A caption is a `text` or `math` with
    /// this, and a leader line is an `arrow` or `line` with it. The object then has no
    /// `position`, `rotation` or `scale` of its own: it sits at the other object's position, or
    /// the group's center, on the frame, moved by `offset`, faces the camera, shows in front of
    /// all of the other object, and exists while it does.
    pub(super) attach: Option<AttachFile>,
    #[serde(flatten)]
    pub(super) shape: ShapeFile,
}

/// Where an attached object sits.
#[derive(Deserialize, JsonSchema)]
pub(super) struct AttachFile {
    /// The object id of the object to sit beside. It must be placed by its own transform: not a
    /// plot, and not attached itself.
    pub(super) object: String,
    /// The group id of one of that object's `groups`, when it is a molecule and the attached
    /// object sits beside the group. Leave it out to sit beside the object's position.
    pub(super) group: Option<String>,
    /// How far right and up of the object's position, or the group's center, on the frame the
    /// attached object's position lies, in world units measured in the plane through the
    /// object's position that faces the camera. It is along the screen's axes, not the world's,
    /// so the attached object stays beside while the object turns. The default is `[0, 0]`.
    #[serde(default)]
    #[schemars(with = "[f32; 2]")]
    pub(super) offset: [f32; 2],
}

/// The atoms of one group: their atom numbers, or the residues of a PDB file that hold them.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(super) enum GroupFile {
    /// The atom numbers of the group's atoms: at least one, each 1 or more, as the structure file
    /// numbers them, such as `[1, 9]`.
    Atoms(#[schemars(length(min = 1), inner(range(min = 1)))] Vec<u32>),
    Residues(ResiduesFile),
}

/// The atoms of some residues of a PDB file, such as one helix,
/// `{ "residues": [{ "chain": "A", "from": 23, "to": 34 }] }`, or one nucleobase pair, the
/// nucleobases of two nucleotides across from each other,
/// `{ "residues": [{ "chain": "A", "number": 4 }, { "chain": "B", "number": 21 }], "part": "nucleobase" }`.
/// `inspect` lists each chain's residues. Needs sci_anim 0.17.0 or later.
#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct ResiduesFile {
    /// The residues, each by its chain id and residue number, or a run of them, or a whole chain.
    /// At least one.
    #[schemars(length(min = 1))]
    pub(super) residues: Vec<ResiduePickFile>,
    /// Which atoms of each residue the group holds. Leave it out for all of them.
    /// `nucleobase` holds only a nucleotide's nucleobase, without its sugar and phosphate, and
    /// every residue named must then be a nucleotide.
    pub(super) part: Option<ResiduePartFile>,
}

/// The values of a residue pick's `part`.
#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ResiduePartFile {
    Nucleobase,
}

/// One residue, a run of residues, or a whole chain, of a PDB file.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(super) enum ResiduePickFile {
    /// The residue of residue number `number` in chain `chain`, as `{ "chain": "A", "number": 4 }`.
    /// A number with insertion codes, as `52` and `52A`, names every residue of that number.
    One {
        /// The chain id, the one letter the PDB file names the chain by.
        #[schemars(length(min = 1, max = 1))]
        chain: String,
        /// The residue number, as the PDB file gives it.
        number: i32,
    },
    /// The residues of chain `chain` numbered `from` to `to`, both included, as
    /// `{ "chain": "A", "from": 23, "to": 34 }`.
    Run {
        /// The chain id, the one letter the PDB file names the chain by.
        #[schemars(length(min = 1, max = 1))]
        chain: String,
        /// The residue number the run starts at. The chain has a residue of this number.
        from: i32,
        /// The residue number the run ends at, `from` or more. The chain has a residue of this
        /// number.
        to: i32,
    },
    /// Every residue of chain `chain`, as `{ "chain": "B" }`.
    Chain {
        /// The chain id, the one letter the PDB file names the chain by.
        #[schemars(length(min = 1, max = 1))]
        chain: String,
    },
}

impl GroupFile {
    pub(super) fn into_group_atoms(self) -> crate::group::GroupAtoms {
        use crate::group::{GroupAtoms, ResiduePick, ResidueRun};
        use crate::molecule::ChainId;
        let chain_id = |chain: &str| ChainId(chain.chars().next().expect("the schema's length"));
        match self {
            GroupFile::Atoms(numbers) => GroupAtoms::Numbers(numbers),
            GroupFile::Residues(ResiduesFile { residues, part }) => GroupAtoms::Residues {
                nucleobases: matches!(part, Some(ResiduePartFile::Nucleobase)),
                picks: residues
                    .into_iter()
                    .map(|pick| match pick {
                        ResiduePickFile::One { chain, number } => ResiduePick {
                            chain: chain_id(&chain),
                            residues: ResidueRun::One(number),
                        },
                        ResiduePickFile::Run { chain, from, to } => ResiduePick {
                            chain: chain_id(&chain),
                            residues: ResidueRun::Between { from, to },
                        },
                        ResiduePickFile::Chain { chain } => ResiduePick {
                            chain: chain_id(&chain),
                            residues: ResidueRun::WholeChain,
                        },
                    })
                    .collect(),
            },
        }
    }
}

/// How a molecule is drawn. The values of the scene file's `representation`.
#[derive(Clone, Copy, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RepresentationFile {
    #[default]
    BallAndStick,
    SpaceFilling,
    Cartoon,
}

/// Only these characters: SVG path commands, numbers and separators. kurbo then checks the
/// grammar, and reports the error at `d`.
pub(super) const SVG_PATH_PATTERN: &str = r"^[MmZzLlHhVvCcSsQqTtAa0-9eE+\-., \t\r\n]*$";

/// The file form of a `Shape`. Every flat kind names its own dimensions, in the object's xy
/// plane and about the object's position, and becomes one `Path` when the file is read.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(super) enum ShapeFile {
    /// A flat circle about the object's position. No stroke when `stroke` is absent, and no fill
    /// when `fill` is absent.
    Circle {
        /// The circle's radius in world units, more than 0.
        radius: Positive,
        /// The circle's outline. With none, the circle has no stroke and cannot be created.
        stroke: Option<Stroke>,
        /// The circle's inside. With none, the circle has no fill.
        fill: Option<Fill>,
    },
    /// A filled disc about the object's position that marks a point: the tip of a vector, the
    /// map origin of a linear map, a point on a curve. Its own scale and rotation reshape it as a
    /// circle's would. A linear map moves its center to the center's image and keeps its size,
    /// so it stays visible even when the matrix flattens everything around it. It has no
    /// stroke, so fade it in rather than create it. `inspect` reports its world `center`. On
    /// `axes` it is a data point, the point of tangency, root or maximum of a plot.
    Dot {
        /// The disc's radius in world units, more than 0, on axes too, so it stays round however
        /// they stretch.
        radius: Positive,
        /// The object id of an `axes` object. The dot is then a data point: its `position`, and
        /// every move, follow and carry of it, is in their data coordinates, `[x, y, z]` with x
        /// and y as a plot on them writes them and z the distance off their plane in world
        /// units, normally 0. The axes carry it wherever they go, and a linear map names the
        /// axes, not the dot. The default is none: its position is in world units.
        axes: Option<String>,
        /// The disc's color and opacity. The default is solid white.
        #[serde(default = "ShapeFile::default_dot_fill")]
        fill: Fill,
    },
    /// A solid sphere about the object's position, lit as every solid is. It can fade, move,
    /// scale and rotate, and a create turns it into being as the revolution of a half circle, but
    /// it cannot be morphed.
    Sphere {
        /// The sphere's radius in world units, more than 0, on 3D axes too.
        radius: Positive,
        /// The object id of a `3d-axes` object. The sphere is then a data point: its `position`,
        /// and every move, follow and carry of it, is in their data coordinates, and the axes
        /// carry it wherever they go. The default is none: its position is in world units.
        axes: Option<String>,
        /// How the sphere's surface looks. The default is white, `roughness` 0.5 and `metallic` 0.
        #[serde(default)]
        material: Material,
    },
    /// A solid made by pushing `base` straight up by `height`, with a face the shape of the base
    /// at each end. A box is a prism on a square, a cylinder a prism on a circle, and a pipe a
    /// prism on a path with a hole in it under `even-odd`. The base lies flat in the object's xz
    /// plane: its x is the world's x, and its y runs along -z, away from the default camera. The
    /// prism stands on the object's position and rises along +y. A create raises it from its
    /// base over the create's span. A solid is lit by a key light from the camera's upper left
    /// and drawn with edges where its faces meet at a sharp angle and along its outline. It can
    /// fade, move, scale, rotate and recolor its `color`, but it cannot morph or be named by a
    /// linear map. `inspect` reports its bounding box.
    Prism {
        /// The region pushed up: a `circle`, `polygon` or `path` in the object's xz plane.
        base: RegionFile,
        /// How far the base is pushed up, in world units, more than 0.
        height: Positive,
        /// How the prism's faces look. The default is white. The `opacity` must be 1, since a
        /// see-through solid cannot be drawn yet: fade the object's `opacity` instead.
        #[serde(default)]
        material: Material,
        /// `dark` for edges in a darker shade of the color, or `light` for a lighter one, for a
        /// dark solid whose dark edges vanish against the background. The default is `dark`.
        #[serde(default)]
        edges: EdgeShade,
    },
    /// A solid made by turning `profile` around the object's y axis through `angle` degrees. The
    /// profile is half of the solid's outline as the default camera sees it, drawn in the
    /// object's xy plane with x of 0 or more: a cone is the revolution of a right triangle with
    /// one side on the axis, a cylinder of a rectangle, a torus of a circle whose `center` lies
    /// beside the axis, and a flask or a lens of a path. With an `angle` under 360 the solid is
    /// cut open and closed by its profile at both cut faces, so a 270 degree planet shows its
    /// layers. It turns from +x toward -z, away from the default camera. A create turns it from
    /// 0 to its `angle`. It is lit, drawn with edges, animated and inspected as a prism is.
    Revolution {
        /// The region turned: a `circle`, `polygon` or `path` in the object's xy plane, at x of 0
        /// or more.
        profile: RegionFile,
        /// How far the profile turns, in degrees, more than 0 and at most 360. The default is
        /// 360, a whole solid.
        #[serde(default = "RevolutionAngle::full")]
        angle: RevolutionAngle,
        /// How the revolution's faces look. The default is white. The `opacity` must be 1, since a
        /// see-through solid cannot be drawn yet: fade the object's `opacity` instead.
        #[serde(default)]
        material: Material,
        /// `dark` for edges in a darker shade of the color, or `light` for a lighter one, for a
        /// dark solid whose dark edges vanish against the background. The default is `dark`.
        #[serde(default)]
        edges: EdgeShade,
    },
    /// A solid made by carrying a circle of `radius` along the space curve `curve`, always square
    /// to the curve, on a frame that turns no more than the curve does, so the tube never twists.
    /// A spring is a tube along a helix, and an orbit, a field line or a strand of DNA is a tube
    /// too. Each open end is closed by a disc, and a curve that ends where it starts closes on
    /// itself with no ends. A create draws it along the curve by length, from the start of
    /// `u_range` or the first point. It is lit, drawn with edges, animated and inspected as a
    /// prism is, and `inspect` adds the world point at each end of its curve and, for a curve
    /// through points or an atom trace, of each curve point. A tube through curve points morphs
    /// into another through as many, curve point by curve point. With `axes`, the
    /// curve is written in the data coordinates of those 3D axes, which place the tube, and the
    /// tube is cut where its curve leaves their box, each piece closed by a disc.
    Tube {
        /// The curve the tube runs along: x, y and z as expressions in `u`, or the points it passes
        /// through, in world units from the object's position, or in data units with z up on
        /// `axes`, or an atom trace of a PDB file, in angstroms.
        curve: SpaceCurveFile,
        /// The object id of a `3d-axes` object whose data coordinates the curve is written in.
        /// The tube then has no `position`, `rotation` or `scale` of its own: the axes place it.
        /// The default is none: the tube is placed by its own transform.
        axes: Option<String>,
        /// The radius of the tube's circle, in world units, more than 0. Keep it under the
        /// tightest bend's radius, or the tube folds into itself there.
        radius: Positive,
        /// How the tube's faces look. The default is white. The `opacity` must be 1, since a
        /// see-through solid cannot be drawn yet: fade the object's `opacity` instead.
        #[serde(default)]
        material: Material,
        /// `dark` for edges in a darker shade of the color, or `light` for a lighter one, for a
        /// dark solid whose dark edges vanish against the background. The default is `dark`.
        #[serde(default)]
        edges: EdgeShade,
    },
    /// A solid that is a sheet: its height `z`, an expression in `x` and `y`, over the rectangle
    /// `x_range` by `y_range`, as a textbook writes z = f(x, y). The rectangle lies on the
    /// object's floor, read as a prism's base is: x along the object's x, and y along its -z,
    /// away from the default camera. The height rises along the object's own up, +y, so z is
    /// vertical. Everything is in world units from the object's position. A potential well, a
    /// wave's height and a function of two variables are surfaces. It is sampled finer where it
    /// bends and lit on both sides, so the camera may look at it from above or below. A create
    /// rises it from the flat rectangle at height 0 to its heights. It is drawn with edges along
    /// its rim and where it folds away from the camera, and is animated and inspected as a prism is.
    /// With `axes`, x, y and z are the data coordinates of those 3D axes, which place the
    /// surface, the ranges default to theirs, and the sheet is cut where it leaves their z range,
    /// so a function with a pole stops at the box. A create then rises it from data z of 0, or
    /// the nearest end of the z range.
    Surface {
        #[schemars(description = crate::expr::surface_description())]
        z: String,
        /// The lowest and highest x, as `[low, high]`, low first, in world units from the
        /// object's position, or in data units on `axes`, where the default is the axes' x range
        /// and the rectangle is cut to it. Required without `axes`.
        #[schemars(with = "Option<[f64; 2]>")]
        x_range: Option<[f64; 2]>,
        /// The lowest and highest y, as `[low, high]`, low first, in world units from the
        /// object's position, or in data units on `axes`, where the default is the axes' y range.
        /// y runs along the object's -z, away from the default camera. Required without `axes`.
        #[schemars(with = "Option<[f64; 2]>")]
        y_range: Option<[f64; 2]>,
        /// The object id of a `3d-axes` object whose data coordinates the surface is written in.
        /// The surface then has no `position`, `rotation` or `scale` of its own: the axes place
        /// it. The default is none: the surface is placed by its own transform.
        axes: Option<String>,
        /// How the sheet looks, on both sides. The default is white. The `opacity` must be 1,
        /// since a see-through solid cannot be drawn yet: fade the object's `opacity` instead.
        #[serde(default)]
        material: Material,
        /// `dark` for edges in a darker shade of the color, or `light` for a lighter one, for a
        /// dark solid whose dark edges vanish against the background. The default is `dark`.
        #[serde(default)]
        edges: EdgeShade,
    },
    /// A straight stroke from `from` to `to`. With no `stroke` it is white and 0.04 wide, and a
    /// `stroke` that is written sets its own `color`. An end may sit on an object, as
    /// `{ "object": "block", "offset": [0, -1, 0] }`: the line is then placed by its ends, lies
    /// between them at every scene time wherever those objects go, has no `position`,
    /// `rotation` or `scale` of its own, and is hidden while either object is. `inspect` reports
    /// each end's world point under `ends`.
    Line {
        /// The start: `[x, y]` in world units in the object's plane, measured from its position,
        /// or an end on an object, a curve or an atom. On a line placed by its ends, `[x, y]` or `[x, y, z]` is a
        /// point in the world.
        from: EndFile,
        /// The end, written as `from` is. It must differ from `from`.
        to: EndFile,
        /// The line's color and width. The default is white and 0.04 wide.
        #[serde(default)]
        stroke: Stroke,
    },
    /// A line from `from` to `to` with a filled tip at `to`, both in the stroke's color. The tip
    /// is a triangle `tip_length` long and as wide, or a quarter of the arrow when the arrow is
    /// shorter than four tips. With no `stroke` it is white and 0.04 wide. An end may sit on an
    /// object, as a line's may: a force arrow then stays on its block as the block moves, and
    /// its tip keeps its length as the arrow stretches.
    Arrow {
        /// The tail: `[x, y]` in world units in the object's plane, measured from its position,
        /// or an end on an object, a curve or an atom. On an arrow placed by its ends, `[x, y]` or `[x, y, z]` is a
        /// point in the world.
        from: EndFile,
        /// The point of the tip, written as `from` is. It must differ from `from`.
        to: EndFile,
        /// The length of the tip in world units, more than 0. The tip is as wide as it is long.
        /// The default is 0.35.
        #[serde(default = "ShapeFile::default_tip_length")]
        tip_length: Positive,
        /// The color and width of the shaft. The tip is filled in the same color. The default is
        /// white and 0.04 wide.
        #[serde(default)]
        stroke: Stroke,
    },
    /// A coil of `turns` turns of `radius` about the straight line from `from` to `to`, drawn as a
    /// `tube` of `tube_radius` on a helix, so it stretches and squashes as its ends move and keeps
    /// its turns. Each end is a point in the world, `[x, y]` or `[x, y, z]`, or an end on an
    /// object, as `{ "object": "mass", "offset": [0, 0.3, 0] }`. A spring is placed by its ends:
    /// it has no `position`, `rotation` or `scale` of its own, and is hidden while either
    /// object is. It is lit, created, faded and recolored as a tube is, and `inspect` reports
    /// each end's world point under `ends`.
    Spring {
        /// Where the coil starts.
        from: EndFile,
        /// Where the coil ends.
        to: EndFile,
        /// How many turns the coil makes between its ends, more than 0 and at most 100. The
        /// default is 8.
        #[serde(default = "ShapeFile::default_turns")]
        turns: Turns,
        /// The radius of the coil about the line between its ends, in world units, more than 0.
        /// The default is 0.2.
        #[serde(default = "ShapeFile::default_spring_radius")]
        radius: Positive,
        /// The radius of the tube the coil is drawn as, in world units, more than 0. The default
        /// is 0.03.
        #[serde(default = "ShapeFile::default_tube_radius")]
        tube_radius: Positive,
        /// How the coil's faces look. The default is white. The `opacity` must be 1.
        #[serde(default)]
        material: Material,
        /// `dark` or `light` edges, as for a tube. The default is `dark`.
        #[serde(default)]
        edges: EdgeShade,
    },
    /// The world points `object` passed through over the last `length` seconds of scene time,
    /// or since it began to show, drawn as a stroke that faces the camera. The points are its
    /// world position at every frame time in that time, and the newest is where it stands, so a
    /// trail follows a law, a carry, a move or a data point alike: an orbit, a pendulum's arc
    /// and a projectile's path are trails. A trail has no `position`, `rotation` or `scale` of
    /// its own. It is not created: it grows as its object moves, so fade it in, or declare it
    /// shown from the start. `inspect` reports its `point_count`, its `newest` point and the
    /// pixel that lands on, and its bounding box.
    Trail {
        /// The object id of the object whose path the trail draws: not a plot, and not a trail.
        object: String,
        /// How many seconds of scene time back the trail reaches, more than 0. The default is
        /// every second since the object began to show: from the start of its create, the end
        /// of the reaction or morph that makes it, or scene time 0.
        length: Option<Duration>,
        /// The trail's color and width. The default is white and 0.04 wide.
        #[serde(default)]
        stroke: Stroke,
    },
    /// A number typeset as math, with `unit` after it, such as a speed that counts as a ball
    /// falls. Its `value` changes as a track does: a `count` eases it to a `to`, a follow with
    /// `"sets": "value"` sets it from a law with one expression, `value`, and a simulation sets
    /// it. Its digits and the layout of each count of digits are typeset once, so a number
    /// that counts costs no more than one that stands still. The number is lined up on its decimal point, or on the end of its last digit with no
    /// decimals, at the object's position, so its digits keep their places as it grows; give it
    /// room to the left. It is written, faded, attached and recolored as math is. `inspect`
    /// reports its `value` and the `text` it shows under `readout`.
    Readout {
        /// The value shown until an animation changes it. The default is 0.
        #[serde(default = "FiniteNumber::zero")]
        value: FiniteNumber,
        /// How many decimals the number shows, from 0 to 6. The default is 0.
        #[serde(default = "Decimals::none")]
        decimals: Decimals,
        /// LaTeX math set after the number, as in a math object's `tex`, such as
        /// `\\text{ m/s}` as JSON writes it, whose space keeps the unit off the number. The default is none.
        #[serde(default)]
        unit: String,
        /// The text size: the height of one em, in world units, more than 0. The default is 0.6.
        #[serde(default = "ShapeFile::default_size")]
        size: Positive,
        /// The color of every glyph, written `#rrggbb`. The default is white.
        #[serde(default = "ShapeFile::default_color")]
        color: Color,
    },
    /// A closed path through `points` in order, at least three.
    Polygon {
        /// The corners in order, each `[x, y]` in world units in the object's plane, measured from
        /// its position. At least three. The path closes from the last back to the first.
        #[schemars(with = "Vec<[f32; 2]>", length(min = 3))]
        points: Vec<Vec2>,
        /// The polygon's outline. With none, it has no stroke and cannot be created.
        stroke: Option<Stroke>,
        /// The polygon's inside. With none, it has no fill.
        fill: Option<Fill>,
    },
    /// Part of a circle about the object's position, from `start_angle` degrees counterclockwise
    /// from +x, sweeping `sweep` degrees, counterclockwise when positive. A fill fills the
    /// region between the arc and its chord.
    Arc {
        /// The radius of the circle the arc is part of, in world units, more than 0.
        radius: Positive,
        /// Where the arc starts, in degrees counterclockwise from +x. The default is 0.
        #[serde(default)]
        start_angle: f32,
        /// How far the arc runs from `start_angle`, in degrees from -360 to 360, counterclockwise
        /// when positive. It must not be 0.
        sweep: Sweep,
        /// The arc's outline. With none, it has no stroke and cannot be created.
        stroke: Option<Stroke>,
        /// The region between the arc and its chord. With none, it has no fill.
        fill: Option<Fill>,
    },
    /// Any path, written in SVG path syntax in `d`, such as
    /// `M -1 -1 L 1 -1 L 1 1 Z M 0 0 A 1 1 0 1 0 0 0.01 Z`. A path can have several parts,
    /// and `fill.rule` says whether a part inside another is a hole.
    Path {
        /// SVG path data: the commands `M L H V C S Q T A Z`, upper case for absolute points and
        /// lower case for relative ones, with numbers in world units measured from the object's
        /// position. Unlike SVG, y points up. `M` starts a new part.
        #[schemars(regex(pattern = SVG_PATH_PATTERN))]
        d: String,
        /// The path's outline. With none, it has no stroke and cannot be created.
        stroke: Option<Stroke>,
        /// The path's inside. With none, it has no fill.
        fill: Option<Fill>,
    },
    /// Plain text in the `font` it names, Libertinus Sans unless it is `serif`, filled in `color`. A `\n` breaks a line. Nothing else in
    /// it is markup. `size` is the em in world units. `align` lines up the lines and picks the
    /// point of the text's box at the object's position: the middle of its left edge, its
    /// center, or the middle of its right edge. The box runs from the first line's cap height
    /// to the last line's baseline. A write draws it glyph by glyph.
    Text {
        /// The characters to set, as plain text. `\n` starts a new line, and nothing else is
        /// markup. The bundled fonts cover Latin, Greek and Cyrillic, and a character with no
        /// glyph in them is a scene error.
        text: String,
        /// `sans` for Libertinus Sans or `serif` for Libertinus Serif. The default is `sans`.
        #[serde(default)]
        font: Font,
        /// The text size: the height of one em, in world units, more than 0. The default is 0.6.
        #[serde(default = "ShapeFile::default_size")]
        size: Positive,
        /// The color every glyph is filled in, written `#rrggbb`. The default is white.
        #[serde(default = "ShapeFile::default_color")]
        color: Color,
        /// `left`, `center` or `right`: how the lines line up, and which point of the text's box
        /// sits at the object's position. The default is `center`.
        #[serde(default)]
        align: Align,
    },
    /// An equation in LaTeX math syntax, such as `e^{i\\pi} + 1 = 0`, set in display appearance in New
    /// Computer Modern Math. Each glyph is filled in the color LaTeX gives it with `\textcolor`
    /// or `\color`, or else in `color`. `size`, `align` and the write are as for text.
    Math {
        /// LaTeX math, such as `\frac{a}{b} + \sqrt{x}`. JSON needs each backslash written
        /// twice, so the scene file holds `"\\frac{a}{b} + \\sqrt{x}"`. mitex turns it into
        /// Typst math, so it takes the commands and environments mitex knows, such as `\text`,
        /// `\mathbb`, `pmatrix`, `cases` and `aligned`. A command mitex does not know, such as
        /// mhchem's `\ce`, is a scene error. `\textcolor{#58c4dd}{x}` colors one term, and
        /// `{\color{#58c4dd} x + y}` colors the rest of its group. A color is `#rrggbb` or one of
        /// red, green, blue, cyan, magenta, yellow, black, white, gray, lightgray, darkgray,
        /// brown, orange, pink, purple, teal and olive, in any case, and any other color is a
        /// scene error. These commands mitex knows are a scene error too: `\boxed` and `\fbox`,
        /// which draw a box sci_anim cannot draw yet, `\Box`, whose glyph the bundled font
        /// lacks, `\limits` and `\nolimits` with no operator before them, and the commands of a
        /// document rather than an equation, `\bottomrule`, `\cite`, `\eqref`, `\footnote`,
        /// `\iffalse`, `\includegraphics`, `\item`, `\midrule`, `\newenvironment`,
        /// `\newenvironment*`, `\ref`, `\renewenvironment`, `\renewenvironment*`,
        /// `\textwidth` and `\toprule`, and the environment `tabular`.
        tex: String,
        /// The text size: the height of one em, in world units, more than 0. The default is 0.6.
        #[serde(default = "ShapeFile::default_size")]
        size: Positive,
        /// The color of every glyph that `tex` does not color, written `#rrggbb`. The default is
        /// white.
        #[serde(default = "ShapeFile::default_color")]
        color: Color,
        /// `left`, `center` or `right`: how the lines line up, and which point of the math
        /// object's box sits at the object's position. The default is `center`.
        #[serde(default)]
        align: Align,
    },
    /// Text and math in cells, in `rows` of cells from the top, each row's cells from the left,
    /// with rulings between them, centered on the object's position. Each column is as wide as
    /// its widest cell and each row as tall as its tallest, plus `padding` on every side of each
    /// cell. A write draws the rulings along their length over the first fifth of its span, then
    /// writes the cells in reading order, row by row. A `highlight` with a `cell` of
    /// `[row, column]` lights one cell, and a `recolor` changes every cell's `color` and the
    /// rulings' `stroke`. `inspect` reports `table`: its `size`, `rows`, `columns` and each
    /// cell's `cell` address, `text` or `tex`, own `created` share, `glow`, `rectangle` and the
    /// `bounding_box` of its glyphs, which `check-layout` checks as it checks a text object's,
    /// naming the cell `table/row/column`. Needs sci_anim 0.18.0 or later.
    Table {
        /// The rows from the top, each a list of its cells from the left, every row as long as
        /// the first. A cell is a string of plain text, set as a `text` object's `text` is, an
        /// object `{ "tex": "..." }` of LaTeX math, set as a `math` object's `tex` is, or `""`
        /// for an empty cell. Such as `[["Codon", "Amino acid"], ["AUG", "Met"]]`.
        #[schemars(length(min = 1), inner(length(min = 1)))]
        rows: Vec<Vec<CellFile>>,
        /// The text size of every cell: the height of one em, in world units, more than 0. The
        /// default is 0.6.
        #[serde(default = "ShapeFile::default_size")]
        size: Positive,
        /// The color of every glyph that a cell's `tex` does not color, written `#rrggbb`. The
        /// default is white.
        #[serde(default = "ShapeFile::default_color")]
        color: Color,
        /// The font of the text cells: `sans` for Libertinus Sans or `serif` for Libertinus
        /// Serif. Math is set in New Computer Modern Math. The default is `sans`.
        #[serde(default)]
        font: Font,
        /// `left`, `center` or `right`: where each cell's text or math sits across its cell, `padding` from its left
        /// edge, in its middle, or `padding` from its right edge. It sits in the middle of its
        /// row's height. The default is `center`.
        #[serde(default)]
        align: Align,
        /// The cell padding: the space between each cell's edges and its text or math, on every
        /// side, in world units, 0 or more. The default is 0.4 of `size`.
        padding: Option<CellPadding>,
        /// Which rulings to draw: `all` between every row and column and around the table,
        /// `header` one under the first row, or `none`. The default is `all`.
        #[serde(default)]
        rulings: Rulings,
        /// The color and width of the rulings. The default is gray, `#888888`, and 0.02 wide.
        #[serde(default = "ShapeFile::default_ruling_stroke")]
        stroke: Stroke,
    },
    /// Axes: `x` and `y` each give the `range` of data values they show, the `spacing` of their
    /// ticks (a round number giving about five when left out) and a `label` in LaTeX math, set at
    /// the high end. `width` and `height` are the lengths of the axes in world units, in a box
    /// centered on the object's position. The x axis crosses the y axis at 0, or at the nearest
    /// end of the y range. Tick labels are set in math. A create draws the axis lines and ticks
    /// along their length, then fades the labels in. Plots name these axes and are placed by them.
    /// `inspect` reports the text, side and bounding box of each tick label, the bounding box of
    /// each axis label, and on each plot the labels its stroke crosses.
    Axes {
        /// The horizontal axis: its range, tick spacing and label, or its `categories` and label,
        /// a category axis for a bar plot.
        x: XAxisFile,
        /// The vertical axis: its range, tick spacing and label.
        y: AxisFile,
        /// The length of the x axis in world units, more than 0.
        width: Positive,
        /// The length of the y axis in world units, more than 0.
        height: Positive,
        /// The color and width of the axis lines and ticks. The default is white and 0.04 wide.
        #[serde(default)]
        stroke: Stroke,
        /// The color of the tick labels and axis labels, written `#rrggbb`. The default is white.
        #[serde(default = "ShapeFile::default_color")]
        color: Color,
        /// The text size of the tick labels and axis labels: one em, in world units, more than 0.
        /// The default is 0.3.
        #[serde(default = "ShapeFile::default_label_size")]
        label_size: Positive,
    },
    /// Three axes with ticks, tick labels and axis labels, z up as a textbook draws them: data
    /// `(x, y, z)` lands at x to the right, z up and y away from the default camera, each axis
    /// scaled so its `range` fills its `length`, in a box centered on the object's position.
    /// Each axis crosses the others at 0, or at the nearest end of their range, and the tick
    /// there has no label. An x or y tick label sits below its tick, or above where the axis runs
    /// over the box's middle, and a z tick label sits left of its tick on the frame, from wherever
    /// the camera stands. An axis label sits past the high end of its axis. Every label faces the camera at every frame and is hidden by what stands in
    /// front of it. The lines and ticks keep the stroke's width from every side. A create draws
    /// the lines and ticks along their length, then fades the labels in. It is moved, scaled,
    /// rotated and faded as any object, and cannot be morphed, recolored or named by a linear
    /// map. `inspect` reports where data lands in the world and each tick with its label's text,
    /// side and bounding box.
    #[serde(rename = "3d-axes")]
    Axes3d {
        /// The axis to the right: its range, tick spacing, length and label.
        x: Axis3dFile,
        /// The axis away from the default camera, along -z of the world: its range, tick
        /// spacing, length and label.
        y: Axis3dFile,
        /// The axis up: its range, tick spacing, length and label.
        z: Axis3dFile,
        /// The color and width of the axis lines and ticks. The default is white and 0.04 wide.
        #[serde(default)]
        stroke: Stroke,
        /// The color of the tick labels and axis labels, written `#rrggbb`. The default is white.
        #[serde(default = "ShapeFile::default_color")]
        color: Color,
        /// The text size of the tick labels and axis labels: one em, in world units, more than 0.
        /// The default is 0.3.
        #[serde(default = "ShapeFile::default_label_size")]
        label_size: Positive,
    },
    /// One axis with ticks and labels, `length` long in world units and centered on the object's
    /// position, drawn, created and inspected as axes are.
    NumberLine {
        /// The lowest and highest value the line shows, as `[low, high]`, low first.
        #[schemars(with = "[f64; 2]")]
        range: [f64; 2],
        /// The distance in data units between ticks, more than 0, giving at most 100 ticks. The
        /// default is a round number that gives about five.
        spacing: Option<f64>,
        /// The length of the line in world units, more than 0.
        length: Positive,
        /// LaTeX math set at the high end of the line, as in `tex`. The default is no label.
        label: Option<String>,
        /// The color and width of the line and its ticks. The default is white and 0.04 wide.
        #[serde(default)]
        stroke: Stroke,
        /// The color of the tick labels and the label, written `#rrggbb`. The default is white.
        #[serde(default = "ShapeFile::default_color")]
        color: Color,
        /// The text size of the tick labels and the label: one em, in world units, more than 0.
        /// The default is 0.3.
        #[serde(default = "ShapeFile::default_label_size")]
        label_size: Positive,
    },
    /// The graph of `expression`, an expression in `x`, over `x_range` (the axes' own x range
    /// when left out), in the coordinates of the axes object `axes`. It has no position, rotation
    /// or scale of its own and is placed by its axes. Where the function has no value, or leaves the
    /// axes, the curve is cut. `inspect` names the labels of its axes and the shown text and math
    /// objects that its stroke crosses.
    FunctionPlot {
        /// The object id of the `axes` object whose data coordinates the plot is drawn in.
        axes: String,
        #[schemars(description = crate::expr::function_plot_description())]
        expression: String,
        /// The lowest and highest x to plot, as `[low, high]` in data units, low first. The
        /// default is the axes' x range.
        #[schemars(with = "Option<[f64; 2]>")]
        x_range: Option<[f64; 2]>,
        /// The curve's color and width. The default is white and 0.04 wide.
        #[serde(default)]
        stroke: Stroke,
    },
    /// The line through `points` in order, in the coordinates of the axes object `axes`, cut at
    /// the axes' box. For data and for anything an expression cannot say. `inspect` reports it as
    /// it reports a function plot.
    PointPlot {
        /// The object id of the `axes` object whose data coordinates the plot is drawn in.
        axes: String,
        /// The points the curve runs through in order, each `[x, y]` in data units. At least two.
        #[schemars(with = "Vec<[f64; 2]>", length(min = 2))]
        points: Vec<[f64; 2]>,
        /// The curve's color and width. The default is white and 0.04 wide.
        #[serde(default)]
        stroke: Stroke,
    },
    /// One bar for each category of the axes object `axes`, whose `x` is a category axis: a
    /// filled rectangle 0.7 of its category's width, standing on the x axis and reaching its
    /// value. A negative value's bar hangs below the x axis, and the part of a bar outside the
    /// axes' box is cut, as a curve is. It has no position, rotation or scale of its own and is
    /// placed by its axes. A create grows the bars from the x axis one after another, in category
    /// order. A fade fades every bar, and a recolor changes every bar's `fill` and `stroke`; one
    /// bar cannot be animated alone. `inspect` reports `bar_plot`: its `axes`, and for each bar
    /// its `category`, `value`, own `created` share and its whole `rectangle` in world units and
    /// pixels. Needs sci_anim 0.16.0 or later.
    BarPlot {
        /// The object id of the `axes` object whose category axis the bars stand on.
        axes: String,
        /// Each category's value in the y axis' data units, in the order of the axes'
        /// `categories`, one value for each category.
        values: Vec<f64>,
        /// The bars' color and opacity.
        fill: Fill,
        /// The bars' outline. The default is none.
        stroke: Option<Stroke>,
    },
    /// The atoms and bonds of the structure file `file`, a path relative to the scene file, drawn
    /// as `representation` says. Fetch a structure file with PubChem before the render, for
    /// example `curl -fo caffeine.sdf "https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/caffeine/SDF?record_type=3d"`.
    /// A compound with no 3D model, such as a protein, answers 404, and `-f` stops curl saving
    /// the error as a file; fetch a protein, DNA or RNA as a PDB file from the Protein Data Bank
    /// instead, as `curl -fo 1ubq.pdb https://files.rcsb.org/download/1UBQ.pdb`. Atoms are in
    /// angstroms, one world unit each, placed about their centroid, so the object's `position`
    /// is the middle of the molecule, and its `rotation` turns it in place. `scale` sizes the
    /// whole molecule. `hydrogens: false` leaves every hydrogen and its bonds undrawn, and
    /// `inspect` still reports them as hidden atoms. A molecule can fade, move, scale and rotate,
    /// but not be created or written.
    Molecule {
        /// The path of a structure file, relative to the folder of the scene file: a PDB file when
        /// its name ends in `.pdb` or `.ent`, and an SDF file otherwise. sci_anim never downloads
        /// one. Fetch PubChem's 3D record first, from
        /// `https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/<name>/SDF?record_type=3d`.
        /// Without `record_type=3d`, PubChem serves a 2D record whose atoms all lie in one plane.
        /// Fetch a PDB file from `https://files.rcsb.org/download/<PDB id>.pdb`. sci_anim reads
        /// its first model and leaves out its waters, and of an atom the file gives alternate
        /// locations, it keeps the first, usually `A`. Its bonds are its `CONECT` records, and
        /// in amino acids and nucleotides the bonds their atoms' distances show, within a
        /// residue and to the next residue of its chain; every bond draws single. A PDB file
        /// usually has no hydrogens, and the molecule draws without them. Atoms keep the serial
        /// numbers the file gives them. Reading a PDB file needs sci_anim 0.17.0 or later.
        #[schemars(length(min = 1))]
        file: String,
        /// How the molecule is drawn. `ball-and-stick`, the default, draws each atom a ball a
        /// quarter of its element's van der Waals radius, and each bond sticks between them, one
        /// for a single bond, two for a double and three for a triple: for a molecule of up to a
        /// few hundred atoms, whose bonds a lesson talks about. `space-filling` draws each atom a
        /// ball at its element's van der Waals radius, overlapping its bonded neighbors, and no
        /// sticks: the space the molecule fills, for a molecule too big to read as balls and
        /// sticks, such as a stretch of DNA or a protein of thousands of atoms. A lit group's
        /// atoms take its glow's color, and `inspect` reports every bond hidden. A reaction cannot
        /// turn a space-filling molecule, since it draws bonds breaking and forming.
        /// `space-filling` needs sci_anim 0.21.0 or later. `cartoon` draws a protein, DNA or RNA
        /// read from a PDB file as a textbook draws its fold: each chain's backbone as one tube
        /// through its amino acids' alpha carbons, `CA`, or its nucleotides' phosphorus atoms,
        /// `P`. The tube is a thin gray circle along coil, widens to a flat red ribbon along each
        /// helix of a `HELIX` record, its flat side facing out from the helix's axis, and to a
        /// flat blue arrow along each β-strand of a `SHEET` record, lying in its β-sheet's plane,
        /// with a tip that steps out one residue before the strand's end and narrows to its point
        /// there. It changes shape smoothly over half a residue at each end of a stretch. A file
        /// with no `HELIX` or `SHEET` records draws as coil. The tube breaks where the file leaves
        /// residues out. No atom is drawn as a ball, so `inspect` reports every atom as hidden
        /// and `hydrogens` does nothing. A highlight of an atom group lights the tube along each
        /// residue whose `CA` or `P` the group holds, so a group written by residues lights that
        /// stretch of the ribbon. A molecule read from an SDF file cannot be a cartoon, and
        /// neither can a reactant or a product. `cartoon` needs sci_anim 0.22.0 or later.
        #[serde(default)]
        representation: RepresentationFile,
        /// Whether hydrogen atoms and their bonds are drawn, in either representation. With
        /// `false` they are hidden atoms: still part of the molecule, its atom groups and its
        /// reactions. The default is `true`.
        #[serde(default = "ShapeFile::default_hydrogens")]
        hydrogens: bool,
        /// Named sets of atoms, by the atoms' numbers in the structure file, such as
        /// `{ "carbonyl": [1, 9] }`. A group can be highlighted and a caption attached to it. A
        /// group may include atoms that `hydrogens: false` hides. A product's group receives the
        /// glow of a reactant group whose atoms become exactly its atoms. A molecule read from a
        /// PDB file can name a group's atoms by residue instead, as
        /// `{ "helix": { "residues": [{ "chain": "A", "from": 23, "to": 34 }] } }`.
        #[serde(default)]
        groups: BTreeMap<String, GroupFile>,
    },
    /// One shape drawn at every copy of a layout. A crystal lattice is a pattern of a `sphere` on
    /// a `grid`, a ruler's ticks are a pattern of a `line` on a grid one copy high, a ring of
    /// atoms is a pattern on a `ring`, and a vector field is a pattern of an `arrow` on a grid,
    /// each copy turned and scaled by expressions in its place. `layout` says where the copies go,
    /// and `copy` gives each copy its own position, rotation, scale and color share from
    /// expressions in its index and place. A pattern holds at most 10000 copies.
    ///
    /// A pattern is one object, placed by its own `position`, `rotation` and `scale`: one rotate
    /// turns the whole lattice, a fade fades every copy, a recolor changes the shape's colors, and
    /// a create grows the copies in index order, each starting a little after the one before. A copy is not animated alone; to
    /// point at one copy, draw a separate object over it. A copy's scale and the pattern's size a
    /// flat copy's stroke with the rest of it. `inspect` reports `copy_count`, and for each copy
    /// its `index`, its world `position` and the pixel it lands on, its `rotation` in the
    /// pattern's own space, its own `created` share, its
    /// `colors` and its `bounding_box`.
    Pattern {
        /// The shape every copy draws: a `circle`, `dot`, `line`, `arrow`, `polygon`, `arc`,
        /// `path`, `sphere`, `prism`, `revolution`, `tube` or `surface`, with that shape's own fields and no
        /// `position`, `rotation`, `scale` or `opacity`. Its coordinates are in world units from
        /// the copy's position. Text, math, axes, number lines, plots, molecules and patterns are
        /// scene errors.
        shape: Box<ShapeFile>,
        /// Where the copies go, in the pattern's own space: a `grid`, a `ring`, or `along` a space
        /// curve.
        layout: LayoutFile,
        /// Each copy's own position, rotation, scale, color share and pick of a palette, as
        /// expressions in its index and place. Leave it out, or any field of it, for copies at
        /// their places, unturned, at scale 1 and in the shape's own colors.
        copy: Option<Box<CopyFile>>,
    },
}

/// Where a line, an arrow or a spring starts or stops: a point, an end on an object, an end on a
/// tube's curve at `u`, an end on a plot's curve at `x`, or an end on an atom of a molecule.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(super) enum EndFile {
    /// A point, `[x, y]` or, on a shape placed by its ends, `[x, y, z]`.
    Point(#[schemars(length(min = 2, max = 3))] Vec<f32>),
    OnCurve(OnCurveFile),
    OnPlot(OnPlotFile),
    OnAtom(OnAtomFile),
    On(OnObjectFile),
}

/// An end on an atom's center, named by an atom reference as an atom map names it, as
/// `{ "object": "at_pair", "atom": 13 }`. It goes with the molecule as it moves, turns and
/// scales, so a hydrogen bond stays on its two atoms. A molecule that a reaction uses up hides
/// the shape when the reaction begins, and one that a reaction makes shows it from the
/// reaction's end. Needs sci_anim 0.14.0 or later.
#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct OnAtomFile {
    /// The object id of the molecule the end sits on.
    pub(super) object: String,
    /// The atom's number in that molecule's structure file, counting from 1 in the order the file
    /// lists its atoms, or in a PDB file the serial number the file gives it, as an atom map
    /// numbers it.
    #[schemars(range(min = 1))]
    pub(super) atom: u32,
}

/// An end on a tube's space curve, at the curve's point at `u`. It follows the curve as the tube
/// moves, turns and scales, and as its shape law runs. Needs sci_anim 0.13.0 or later.
#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct OnCurveFile {
    /// The object id of the tube the end sits on. It may stand on 3D axes, but must not be a
    /// spring, which is placed by its own ends.
    pub(super) object: String,
    /// The u of the curve's point, within the curve's `u_range`, or from 0 at the first curve
    /// point to 1 at the last for a curve through `points`, so the end keeps to the point at that
    /// u wherever the curve goes.
    pub(super) u: f64,
}

/// An end on a plot's curve, at its first point at `x`, in its axes' data coordinates. It goes
/// with the axes and follows the curve as its shape law runs; where the curve is cut, it waits
/// at the curve's point nearest that x. Needs sci_anim 0.13.0 or later.
#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct OnPlotFile {
    /// The object id of the function plot or point plot the end sits on.
    pub(super) object: String,
    /// The x in the axes' data coordinates, within their x range.
    pub(super) x: f64,
}

/// An end on an object: the object's world position, where the frame draws its center, plus
/// `offset`.
#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct OnObjectFile {
    /// The object id of the object the end sits on. It must be placed by its own transform or be
    /// a data point: not a plot, an attached object or another shape placed by its ends.
    pub(super) object: String,
    /// How far from the object's position the end lies, as `[x, y, z]` in world units along the
    /// world's axes, so it does not turn with the object. The default is `[0, 0, 0]`.
    #[serde(default)]
    #[schemars(with = "[f32; 3]")]
    pub(super) offset: [f32; 3],
}

impl EndFile {
    /// The point in the object's plane, for a line or an arrow placed by its own transform.
    pub(super) fn in_plane(&self) -> Option<Vec2> {
        match self {
            EndFile::Point(point) if point.len() == 2 => Some(Vec2::new(point[0], point[1])),
            _ => None,
        }
    }
}

/// The region of a prism's base or a revolution's profile: what a circle's, polygon's or path's
/// fill would fill. It has no stroke, fill or position of its own, and its coordinates are in
/// world units from the object's position.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(super) enum RegionFile {
    /// A disc of `radius` about `center`.
    Circle {
        /// The disc's radius in world units, more than 0.
        radius: Positive,
        /// The disc's center, as `[x, y]` in world units. The default is `[0, 0]`.
        #[serde(default)]
        #[schemars(with = "[f32; 2]")]
        center: Vec2,
    },
    /// The inside of the closed path through `points` in order.
    Polygon {
        /// The corners in order, each `[x, y]` in world units. At least three.
        #[schemars(with = "Vec<[f32; 2]>", length(min = 3))]
        points: Vec<Vec2>,
    },
    /// The inside of any path in SVG path syntax, each part closed back to its start.
    Path {
        /// SVG path data, as a `path` object's `d`. Unlike SVG, y points up.
        #[schemars(regex(pattern = SVG_PATH_PATTERN))]
        d: String,
        /// `non-zero` or `even-odd`, as a fill's `rule`: under `even-odd` a part inside another is
        /// a hole. The default is `non-zero`.
        #[serde(default)]
        rule: FillRule,
    },
}

/// A curve through space, sampled finer where it bends: its x, y and z as expressions in `u`, or
/// the `points` it passes through, or the atom `trace` of a PDB file. A helix is
/// `{ "x": "cos(u)", "y": "0.1*u", "z": "sin(u)", "u_range": [0, 25] }`, a circle in the xz plane
/// is `{ "x": "2*cos(u)", "y": "0", "z": "2*sin(u)", "u_range": [0, 6.2832] }`, and a chain
/// through three beads is `{ "points": [[0, 0, 0], [1, 0.5, 0], [2, 0, 0.5]] }`.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(super) enum SpaceCurveFile {
    /// x, y and z as expressions in `u` over `u_range`. Each expression must have a finite value
    /// within 10000 of the object's position at every u of `u_range`.
    Expressions {
        #[schemars(description = crate::expr::space_curve_description("x"))]
        x: String,
        #[schemars(description = crate::expr::space_curve_description("y"))]
        y: String,
        #[schemars(description = crate::expr::space_curve_description("z"))]
        z: String,
        /// The lowest and highest u, as `[low, high]`, low first. The curve starts at low.
        #[schemars(with = "[f64; 2]")]
        u_range: [f64; 2],
    },
    /// The curve points it passes through, in order, smooth between them, as a scene generator
    /// writes a random coil or a backbone from real coordinates. Needs sci_anim 0.12.0 or later.
    Points {
        /// The curve points, each `[x, y, z]` in world units from the object's position, from 2
        /// to 10000 of them, each coordinate within 10000. The curve passes through each in
        /// order and is centripetal Catmull-Rom between each two, so it never loops or overshoots
        /// between them, and an open curve leaves its first point toward the second and reaches
        /// its last from the one before. `u` runs from 0 at the first point to 1 at the last, the
        /// k-th of n points, counted from 0, at u = k/(n - 1), so a point keeps its u however far
        /// apart the points lie. A point equal to the one before it adds nothing to the curve. A
        /// last point within a thousandth of the curve's size of the first, with at least two
        /// points between them, closes the curve, which then runs smoothly through it with no
        /// ends.
        #[schemars(length(min = 2, max = 10000))]
        points: Vec<CurvePoint>,
    },
    /// An atom trace: the curve through one atom of each amino acid or nucleotide of a PDB file,
    /// its curve points read as a `points` curve's are, such as a protein's alpha carbons, `{
    /// "trace": { "file": "structures/1ubq.pdb", "atom_name": "CA" } }`. Needs sci_anim 0.19.0 or
    /// later.
    Trace {
        /// Which PDB file, which atom name and which chain the atom trace reads.
        trace: AtomTraceFile,
    },
}

/// The atoms an atom trace passes through: in each amino acid or nucleotide of the PDB file
/// `file`, in file order, the atom named `atom_name`. A residue without that atom adds no curve
/// point. Each curve point is the atom's position in angstroms from the molecule's centroid,
/// where a `molecule` object of the same file draws it, so a tube with the molecule's
/// `position`, `rotation` and `scale` runs through those atoms, and its `radius` is in
/// angstroms too. Curve point k is the k-th atom found, so `u` = k/(n - 1) as for `points`.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct AtomTraceFile {
    /// The path of a PDB file, its name ending in `.pdb` or `.ent`, relative to the folder of the
    /// scene file, as a molecule's `file` is. sci_anim reads its first model and the first
    /// alternate location of each atom.
    #[schemars(length(min = 1))]
    pub(super) file: String,
    /// The atom name to pass through in each residue, such as `CA` for an amino acid's alpha
    /// carbon or `P` for a nucleotide's phosphorus. Ions and ligands are left out, so a calcium
    /// ion, whose atom name is also `CA`, adds nothing.
    #[schemars(length(min = 1, max = 4))]
    pub(super) atom_name: String,
    /// The chain id of the one chain to pass through, such as `A`. The default is every chain in
    /// file order, which joins the last residue of one chain to the first of the next.
    #[schemars(length(min = 1, max = 1))]
    pub(super) chain: Option<String>,
}

/// One curve point, `[x, y, z]` in world units, each coordinate from -10000 to 10000.
#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(transparent)]
pub(super) struct CurvePoint(#[schemars(with = "[CurveCoordinate; 3]")] pub(super) [f64; 3]);

/// One coordinate of a curve point, from -10000 to 10000 world units.
#[derive(JsonSchema)]
#[schemars(transparent)]
#[allow(dead_code, reason = "only its schema is used")]
struct CurveCoordinate(#[schemars(range(min = -10000.0, max = 10000.0))] f64);

/// Where a pattern's copies go, in the pattern's own space. Each copy has an index, counted from
/// 0, and a place, the point the layout puts it at, which is its position unless `copy` moves it.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub(super) enum LayoutFile {
    /// `nx` copies along x by `ny` along y by `nz` along z, `spacing` apart and centered on the
    /// pattern's position. Copy `i`, `j`, `k` has the place `((i - (nx-1)/2)*spacing,
    /// (j - (ny-1)/2)*spacing, (k - (nz-1)/2)*spacing)`, so `[3, 1, 1]` at spacing 1 places copies
    /// at x of -1, 0 and 1. The copies are in index order with `i` fastest, then `j`, then `k`,
    /// the order a create grows them in.
    Grid {
        /// How many copies along x, y and z, as `[nx, ny, nz]`, each 1 or more. Their product is
        /// at most 10000. `[5, 1, 1]` is a row, `[5, 5, 1]` a square in the xy plane, and
        /// `[5, 5, 5]` a cube.
        counts: [CopyCount; 3],
        /// The distance between neighboring copies along each axis, in world units, more than 0.
        spacing: Positive,
    },
    /// `count` copies evenly around a circle of `radius` in the pattern's xy plane, centered on
    /// its position. Copy `i` has the place at `start_angle + 360*i/count` degrees counterclockwise
    /// from +x, as an arc's angles run.
    Ring {
        /// How many copies, from 1 to 10000.
        count: CopyCount,
        /// The circle's radius in world units, more than 0.
        radius: Positive,
        /// Where copy 0 sits, in degrees counterclockwise from +x. The default is 0.
        #[serde(default)]
        start_angle: f32,
    },
    /// `count` copies along a space curve, evenly spaced by length. On an open curve the first is
    /// at its start and the last at its end; on a closed curve the last stops one spacing short of
    /// the start, so no two copies meet. One copy sits at the start. To set copies along a tube,
    /// write the tube's `curve` here too. With `faces`, each copy turns to face along the curve.
    Along {
        /// How many copies, from 1 to 10000.
        count: CopyCount,
        /// The curve: x, y and z as expressions in `u`, or the points it passes through, in world
        /// units from the pattern's position, as a tube's `curve` is written.
        curve: SpaceCurveFile,
        /// `tangent` turns each copy so its own x axis points along the curve at its place,
        /// toward the curve's end, as a carry's `faces` turns its object. Its own z axis starts as
        /// near the pattern's own +z as it can, or +y on a curve that starts along z, and the
        /// copies never twist about the curve, so along a curve in the pattern's xy plane every
        /// copy's z axis is +z. The copy's `rotation` then turns it further, about its turned
        /// axes, so a copy can tilt from its strand. Leave it out for copies turned only by their
        /// `rotation`.
        faces: Option<Faces>,
    },
}

/// Each copy's own values, as expressions in its index and place: in a `grid`, `i`, `j` and `k`
/// and `x`, `y` and `z`, and in a `ring` or `along`, `i` and `x`, `y` and `z`. Every expression
/// is evaluated at every copy when the scene file is read.
#[derive(Clone, Default, Deserialize, JsonSchema)]
pub(super) struct CopyFile {
    /// The copy's position in the pattern's own space, in world units. A coordinate left out is
    /// the place's, so `{ "y": "y + 0.2*sin(x)" }` lifts a row of copies into a wave.
    pub(super) position: Option<CopyPositionFile>,
    /// The copy's rotation, in degrees about its own x, y and z axes, turned as an object's
    /// `rotation` turns. An angle left out is 0.
    pub(super) rotation: Option<CopyRotationFile>,
    /// The copy's scale along its own x, y and z axes. A factor left out is 1.
    pub(super) scale: Option<CopyScaleFile>,
    #[schemars(description = crate::expr::color_share_description())]
    pub(super) color_share: Option<String>,
    /// The color each copy's colors are mixed toward by its `color_share`, written `#rrggbb`.
    /// Write it with `color_share`, and not with `palette`.
    pub(super) to_color: Option<Color>,
    /// The colors each copy picks one of by its `palette_index`, each written `#rrggbb`, at
    /// least one. Each copy's colors are mixed toward its pick by its `color_share`, which is 1
    /// unless written, so each copy is drawn in its pick: a palette of four nucleobases' colors
    /// colors each rung of a ladder by its nucleobase. Write it with `palette_index`, and not
    /// with `to_color`. Needs sci_anim 0.15.0 or later.
    #[schemars(length(min = 1))]
    pub(super) palette: Option<Vec<Color>>,
    /// Which color of `palette` each copy picks, counting from 0: an expression in the copy's
    /// index and place, such as `"mod(i, 4)"`, or a list of one whole number a copy in index
    /// order, such as `[0, 1, 2, 3, 2, 1, 0]` for the sequence ATGCGTA of a palette of A, T, G
    /// and C. Every copy's index must be a whole number within the palette, and a list must have
    /// one entry a copy; either fault is a scene error at the copy or entry. Write it with
    /// `palette`. Needs sci_anim 0.15.0 or later.
    pub(super) palette_index: Option<PaletteIndexFile>,
}

/// Which color of a pattern's palette each copy picks: an expression or a list.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(super) enum PaletteIndexFile {
    #[schemars(description = crate::expr::palette_index_description())]
    Expression(String),
    /// One palette index a copy, in index order: a grid's `i` fastest, then `j`, then `k`.
    Listed(Vec<u32>),
}

/// A copy's position, a coordinate at a time.
#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct CopyPositionFile {
    #[schemars(description = crate::expr::copy_position_description("x"))]
    pub(super) x: Option<String>,
    #[schemars(description = crate::expr::copy_position_description("y"))]
    pub(super) y: Option<String>,
    #[schemars(description = crate::expr::copy_position_description("z"))]
    pub(super) z: Option<String>,
}

/// A copy's rotation, an angle at a time.
#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct CopyRotationFile {
    #[schemars(description = crate::expr::copy_rotation_description("x"))]
    pub(super) x: Option<String>,
    #[schemars(description = crate::expr::copy_rotation_description("y"))]
    pub(super) y: Option<String>,
    #[schemars(description = crate::expr::copy_rotation_description("z"))]
    pub(super) z: Option<String>,
}

/// A copy's scale, a factor at a time.
#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct CopyScaleFile {
    #[schemars(description = crate::expr::copy_scale_description("x"))]
    pub(super) x: Option<String>,
    #[schemars(description = crate::expr::copy_scale_description("y"))]
    pub(super) y: Option<String>,
    #[schemars(description = crate::expr::copy_scale_description("z"))]
    pub(super) z: Option<String>,
}

/// One axis of a `3d-axes` object.
#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct Axis3dFile {
    /// The lowest and highest data value the axis shows, as `[low, high]`, low first.
    #[schemars(with = "[f64; 2]")]
    pub(super) range: [f64; 2],
    /// The distance in data units between ticks, more than 0, giving at most 100 ticks. The
    /// default is a round number that gives about five.
    pub(super) spacing: Option<f64>,
    /// The length of the axis in world units, more than 0, which its range fills.
    pub(super) length: Positive,
    /// LaTeX math set past the high end of the axis, as in a math object's `tex`, such as `x`.
    /// The default is no label.
    pub(super) label: Option<String>,
}

/// One axis of an `axes` object.
#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct AxisFile {
    /// The lowest and highest data value the axis shows, as `[low, high]`, low first.
    #[schemars(with = "[f64; 2]")]
    pub(super) range: [f64; 2],
    /// The distance in data units between ticks, more than 0, giving at most 100 ticks. The
    /// default is a round number that gives about five.
    pub(super) spacing: Option<f64>,
    /// LaTeX math set at the high end of the axis, as in a math object's `tex`. The default is
    /// no label.
    pub(super) label: Option<String>,
}

/// The x axis of an `axes` object: a `range` of numbers, or `categories`, which a bar plot stands
/// on.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(super) enum XAxisFile {
    Numbers(AxisFile),
    Categories(CategoryAxisFile),
}

impl XAxisFile {
    pub(super) fn label(&self) -> Option<&str> {
        match self {
            XAxisFile::Numbers(file) => file.label.as_deref(),
            XAxisFile::Categories(file) => file.label.as_deref(),
        }
    }
}

/// A category axis: the categories in order, each with a tick and its label in plain text below
/// the axes' box. Category k, counted from 0, is at data x = k and one data unit wide, so a point
/// plot or a data point at x = k stands on it. The y axis stands at the axis' low end. Needs
/// sci_anim 0.16.0 or later.
#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct CategoryAxisFile {
    /// Each category's label, in order along the axis, set in plain text below its tick, such as
    /// `["1950", "1960"]` or `["PE", "PP", "PVC"]`. From 1 to 100 of them. Two neighboring labels
    /// nearer than a quarter of `label_size` are a scene error: widen the axes, lower
    /// `label_size` or shorten the labels, such as `’50` for 1950.
    #[schemars(length(min = 1, max = 100), inner(length(min = 1)))]
    pub(super) categories: Vec<String>,
    /// LaTeX math set at the high end of the axis, as in a math object's `tex`. The default is
    /// no label.
    pub(super) label: Option<String>,
}

/// One cell of a table: a string of plain text, `""` for an empty cell, or `{ "tex": "..." }`
/// for LaTeX math.
#[derive(Clone, Deserialize, JsonSchema)]
#[serde(untagged)]
pub(super) enum CellFile {
    Text(String),
    Math(MathCellFile),
}

/// A cell of LaTeX math.
#[derive(Clone, Deserialize, JsonSchema)]
pub(super) struct MathCellFile {
    /// LaTeX math, set as a `math` object's `tex` is, such as `\\frac{1}{2}`.
    pub(super) tex: String,
}
