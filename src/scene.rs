//! The domain model of a scene. Everything here is validated data: `scene_file` is the only
//! module that builds a `Scene`.

use std::borrow::{Borrow, Cow};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use glam::{EulerRot, Mat2, Mat4, Quat, Vec2, Vec3, Vec4};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use crate::axes::{Axes, NumberLine};
use crate::axes_3d::Axes3d;
use crate::bar_plot::BarPlot;
use crate::ends::Ends;
pub use crate::group::Attachment;
use crate::group::{Glow, GroupId, Groups};
use crate::molecule::{Molecule, Representation};
use crate::path::{Path, UnderMap};
use crate::pattern::Pattern;
use crate::plane_map::PlaneMap;
use crate::reaction::{Blend, CarriedGlow};
use crate::readout::Readout;
use crate::scene_file::{JsonPath, SceneError, SceneErrorCode, ShapeLaw};
use crate::space_curve::SpaceCurve;
use crate::surface::Surface;
use crate::table::Table;
use crate::timeline::{ChapterSpan, StepSpan, Timeline};
use crate::trail::Trail;

pub struct Scene {
    pub(crate) scene_config: SceneConfig,
    /// The camera and objects as the scene file declares them, before the timeline plays.
    pub(crate) start: Stage,
    /// Built by `scene_file` from the scene file's steps against `start`, so every value it holds
    /// was resolved once, when the scene was read.
    pub(crate) timeline: Timeline,
    /// The sounds a render mixes under the video, in the scene file's order.
    pub(crate) sounds: Vec<Sound>,
}

/// A sound file that plays from the start of a chapter. `scene_file` has checked that the file
/// can be read and that the chapter exists.
#[derive(Clone, Debug, Serialize)]
pub struct Sound {
    /// The file as the scene file writes it, relative to the scene file.
    pub file: String,
    /// The file found from the scene file's folder.
    #[serde(skip)]
    pub path: std::path::PathBuf,
    pub at: crate::timeline::ChapterId,
    /// The scene time the sound starts at: its chapter's start.
    pub start: f64,
}

impl Scene {
    /// The sum of every step's duration. A scene with no steps is one instant long.
    pub fn duration(&self) -> f64 {
        self.timeline.duration()
    }

    pub fn steps(&self) -> &[StepSpan] {
        self.timeline.steps()
    }

    pub fn chapters(&self) -> &[ChapterSpan] {
        self.timeline.chapters()
    }

    pub fn sounds(&self) -> &[Sound] {
        &self.sounds
    }

    /// The chapter a scene time falls in: the later of two where one ends as the next starts.
    pub fn chapter_at(&self, scene_time: SceneTime) -> Option<&ChapterSpan> {
        self.chapters()
            .iter()
            .rev()
            .find(|chapter| (chapter.start..=chapter.end).contains(&scene_time.seconds()))
    }

    /// The scene time of every rendered frame, in order: frame `i` shows scene time
    /// `i / fps`. A video holds `round(duration * fps)` frames, and at least one, so it lasts the
    /// scene's duration to the nearest frame. These are the only scene times made without
    /// `scene_time`, and each lies inside the scene because `(n - 1) / fps < duration`.
    pub fn frame_scene_times(&self) -> impl ExactSizeIterator<Item = SceneTime> + use<> {
        let fps = f64::from(self.scene_config.fps.get());
        frame_times(self.duration(), fps).map(SceneTime)
    }

    /// `from`, every frame time after it before `to`, and `to`: the scene times `stages` reports,
    /// so a checker sees every frame a render draws between the two and both ends.
    pub fn stage_times(&self, from: SceneTime, to: SceneTime) -> Vec<SceneTime> {
        let fps = f64::from(self.scene_config.fps.get());
        let mut times: Vec<SceneTime> = std::iter::once(from.0)
            .chain(frame_times_in((from.0, to.0), fps))
            .map(SceneTime)
            .collect();
        times.dedup();
        times
    }

    /// The only way to make a `SceneTime`, so holding one means it lies inside the scene.
    pub fn scene_time(&self, seconds: f64) -> Result<SceneTime, SceneError> {
        if (0.0..=self.duration()).contains(&seconds) {
            Ok(SceneTime(seconds))
        } else {
            let message = format!(
                "scene time {seconds} is outside the scene, which runs from 0 to {} seconds",
                self.duration()
            );
            Err(SceneError::new(
                JsonPath::root(),
                SceneErrorCode::SceneTimeOutOfRange,
                message,
            ))
        }
    }

    /// The stage at a scene time. Pure: no GPU, no IO, and no memory of earlier calls. Each
    /// animated value is looked up on its own track, so the cost does not grow with scene time.
    pub fn at(&self, scene_time: SceneTime) -> Stage {
        self.timeline.stage_at(&self.start, scene_time.seconds())
    }
}

/// The scene time of every frame a scene of `duration` renders at `fps`: `round(duration * fps)`
/// frames, and at least one, frame `i` at `i / fps`.
pub(crate) fn frame_times(duration: f64, fps: f64) -> impl ExactSizeIterator<Item = f64> {
    let frames = ((duration * fps).round() as u32).max(1);
    (0..frames).map(move |i| frame_time(i, fps))
}

/// The scene time of frame `i` at `fps` frames per second: the one formula every frame time
/// comes from, so a law is checked at exactly the scene times a render draws.
pub(crate) fn frame_time(i: u32, fps: f64) -> f64 {
    f64::from(i) / fps
}

/// Every frame time from `span`'s start up to but not at its end, then the end itself, which a
/// follow holds from then on even when no frame falls on it. It needs no scene duration, so a
/// step can be checked the moment it is read.
pub(crate) fn frame_times_in(span: (f64, f64), fps: f64) -> impl Iterator<Item = f64> {
    let first = (span.0 * fps).floor().max(0.0) as u32;
    (first..)
        .map(move |i| frame_time(i, fps))
        .skip_while(move |t| *t < span.0)
        .take_while(move |t| *t < span.1)
        .chain(std::iter::once(span.1))
}

/// Seconds since the start of a scene, known to lie inside it.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct SceneTime(f64);

impl SceneTime {
    pub fn seconds(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Debug)]
pub struct Stage {
    pub(crate) camera: Camera,
    /// Ordered by object id, so draw order does not depend on the order of the scene file.
    pub(crate) objects: BTreeMap<ObjectId, Object>,
    /// Every reaction of the timeline at this scene time, in timeline order. Empty in the start
    /// stage.
    pub(crate) reactions: Vec<StagedReaction>,
    /// Every object a morph reshapes, by its source's object id, at this scene time. Empty in the
    /// start stage.
    pub(crate) morphs: BTreeMap<ObjectId, StagedMorph>,
    /// Every readout's value at this scene time, by its object id.
    pub(crate) values: BTreeMap<ObjectId, f64>,
}

/// A morph at one scene time. During its step the source object's shape is the mix of the two
/// looks, so the source's `shape` already shows it, and this says where the morph stands.
#[derive(Clone, Debug)]
pub struct StagedMorph {
    pub target: ObjectId,
    pub phase: Phase,
    /// The morph's progress, past 1 for a moment under an overshoot.
    pub progress: f32,
}

/// A reaction at one scene time.
#[derive(Clone, Debug)]
pub struct StagedReaction {
    pub phase: Phase,
    pub blend: Blend,
    /// See `ReactionBlends::fit_rmsd`.
    pub fit_rmsd: f32,
    /// The glows of the reactant groups lit when the step begins, drawn on the blend while it plays.
    pub glows: Arc<[CarriedGlow]>,
}

/// Where a scene time falls against a reaction's step. The reactants show before the step, the
/// blend draws during it, and the products show after.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Before,
    During,
    After,
}

/// The name a scene gives an object. Only `scene_file` makes one, from a key of the scene file's
/// `objects`, so holding an `ObjectId` means the object exists.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ObjectId(Arc<str>);

impl ObjectId {
    pub(crate) fn new(object_id: &str) -> Self {
        Self(Arc::from(object_id))
    }
}

/// Lets a track or a test look an object up by the object id the scene file wrote.
impl Borrow<str> for ObjectId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl Serialize for ObjectId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A shape placed by a transform. `scene_file` builds one from an `ObjectFile`, so a `Shape`
/// here is already a path or a sphere, never the file's circle or arrow.
#[derive(Clone, Debug)]
pub struct Object {
    pub transform: Transform,
    /// The object opacity: 1 shows the object, 0 hides it, and between the two the object shows
    /// as one faded picture of itself. A fade animates it. A fade in starts from an object
    /// declared with opacity 0.
    pub opacity: UnitInterval,
    /// The created share: how far a create has gone. 1 for every object the file declares, and 0
    /// from the start of the scene for an object the timeline creates, until its create begins.
    /// A molecule's stays 1, because a molecule cannot be created.
    pub created: UnitInterval,
    /// What gives this object its transform. A plot or an attached object has no position,
    /// rotation or scale of its own: the stage computes its transform at every scene time.
    pub placed_by: PlacedBy,
    pub shape: Shape,
    /// Where every linear map so far has carried the object: `None` for an object no linear map
    /// names, which then draws exactly as it would with no linear maps in the scene. A plot has
    /// its axes'. The timeline's track replaces it at every scene time.
    pub plane_map: Option<PlaneMap>,
    /// For a shape whose expressions read `t`, law time: how to read it again at another law
    /// time. `shape` is it at `t` of 0 in the start stage, and the timeline's shape track
    /// replaces it at every scene time a follow of the shape has begun by.
    pub shape_law: Option<Arc<ShapeLaw>>,
    /// For a readout: its decimals, unit and the glyphs of each number it shows. `shape` is the
    /// glyphs of its value, which the timeline's value track replaces at every scene time.
    pub readout: Option<Arc<Readout>>,
}

impl Object {
    /// Where the object's own plane lands in the world, plane map included. A dot keeps its own
    /// transform at its mapped center, so its disc keeps its size.
    pub fn flat_to_world(&self) -> FlatToWorld {
        let Some(plane_map) = self.plane_map else {
            return FlatToWorld::Declared(self.transform);
        };
        let Transform {
            position,
            rotation,
            scale,
        } = self.transform;
        let position = plane_map.apply(position.truncate()).extend(position.z);
        match &self.shape {
            Shape::Flat { path, .. } if path.under_map() == UnderMap::CenterOnly => {
                FlatToWorld::Declared(Transform {
                    position,
                    ..self.transform
                })
            }
            _ => {
                let own = Mat2::from_angle(rotation.z.to_radians())
                    * Mat2::from_diagonal(scale.truncate());
                FlatToWorld::Mapped {
                    matrix: plane_map.matrix() * own,
                    position,
                    z_scale: scale.z,
                }
            }
        }
    }
}

/// Where a flat object's own plane lands in the world at one scene time: the one definition that
/// meshes, `inspect`'s reports and attachments all read. A sphere or a molecule is always
/// `Declared`, because no linear map names one.
#[derive(Clone, Copy, Debug)]
pub enum FlatToWorld {
    /// The transform alone places the object, as it did before linear maps existed.
    Declared(Transform),
    /// A plane map carries every point: a point `q` of the object's own plane lands at
    /// `position + matrix · q`. `matrix` is the plane map's matrix times the object's z rotation
    /// and its x and y scale. Linear maps refuse an object turned about x or y, so that is the
    /// whole of its rotation.
    Mapped {
        matrix: Mat2,
        position: Vec3,
        z_scale: f32,
    },
}

impl FlatToWorld {
    /// The object's own space to the world. Singular when `matrix` is.
    pub fn to_world(self) -> Mat4 {
        match self {
            FlatToWorld::Declared(transform) => transform.object_to_world(),
            FlatToWorld::Mapped {
                matrix,
                position,
                z_scale,
            } => Mat4::from_cols(
                matrix.x_axis.extend(0.0).extend(0.0),
                matrix.y_axis.extend(0.0).extend(0.0),
                Vec4::new(0.0, 0.0, z_scale, 0.0),
                position.extend(1.0),
            ),
        }
    }

    /// What places a flat shape's meshes, whose points already hold the x and y scale, or the
    /// whole `matrix`.
    pub fn meshes_to_world(self) -> Mat4 {
        match self {
            FlatToWorld::Declared(transform) => transform.unscaled(),
            FlatToWorld::Mapped {
                position, z_scale, ..
            } => Mat4::from_scale_rotation_translation(
                Vec3::new(1.0, 1.0, z_scale),
                Quat::IDENTITY,
                position,
            ),
        }
    }

    /// The 2x2 that takes the object's own plane to world lengths in it, whatever its turn: the
    /// x and y scale, or `matrix`.
    pub fn matrix(self) -> Mat2 {
        match self {
            FlatToWorld::Declared(transform) => Mat2::from_diagonal(transform.scale.truncate()),
            FlatToWorld::Mapped { matrix, .. } => matrix,
        }
    }

    /// Where the object's position lands in the world.
    pub fn position(self) -> Vec3 {
        match self {
            FlatToWorld::Declared(transform) => transform.position,
            FlatToWorld::Mapped { position, .. } => position,
        }
    }

    /// How much the object's own lengths can grow in the world: its largest scale, or the
    /// longer column of `matrix`.
    pub fn stretch(self) -> f32 {
        match self {
            FlatToWorld::Declared(transform) => transform.scale.max_element(),
            FlatToWorld::Mapped { matrix, .. } => {
                matrix.x_axis.length().max(matrix.y_axis.length())
            }
        }
    }
}

/// Where the own space of the object `id` lands in the world, with `objects` as they stand at a
/// scene time: a plot, or a tube on 3D axes, is where its axes are, before the stage copies
/// their transform onto it.
pub fn own_to_world(objects: &BTreeMap<ObjectId, Object>, id: &ObjectId) -> FlatToWorld {
    let object = &objects[id];
    match &object.placed_by {
        PlacedBy::Axes(axes) => {
            let axes = &objects[axes];
            Object {
                transform: axes.transform,
                plane_map: axes.plane_map,
                ..object.clone()
            }
            .flat_to_world()
        }
        PlacedBy::Itself | PlacedBy::Attachment(_) | PlacedBy::DataPoint(_) | PlacedBy::Ends(_) => {
            object.flat_to_world()
        }
    }
}

/// What places an object. An object placed by itself has the transform it declares, and the
/// timeline animates it. The others have no transform of their own: the stage computes it at
/// every scene time from the object that places them.
#[derive(Clone, Debug, PartialEq)]
pub enum PlacedBy {
    Itself,
    /// A plot sits where its axes sit, and a surface or a tube on 3D axes in their data
    /// coordinates.
    Axes(ObjectId),
    /// A data point: a dot on these axes or a sphere on these 3D axes, whose own transform,
    /// which the timeline animates, holds its position in their data coordinates. The stage
    /// carries it to the world.
    DataPoint(ObjectId),
    /// A caption or leader line sits beside an atom group of a molecule, and any flat shape, text
    /// or math can sit beside another object.
    Attachment(Attachment),
    /// A line, an arrow or a spring with an end on an object, or a spring, lies between its two
    /// ends, and the stage draws it again between them at every scene time.
    Ends(Ends),
}

/// What an object is, after the scene file's shapes have become paths.
#[derive(Clone, Debug)]
pub enum Shape {
    /// A flat shape in its own xy plane, facing +z: a stroke along its path, a fill of its
    /// inside, or both. No stroke when `stroke` is `None`, and no fill when `fill` is `None`.
    Flat {
        path: Path,
        stroke: Option<Stroke>,
        fill: Option<Fill>,
    },
    /// Text or math, typeset when the scene file was read: its glyphs in Typst's layout order,
    /// each filled in its own color under the non-zero fill rule, as fonts expect. It is a flat
    /// shape with no stroke. A write outlines each glyph with its pen, which thins away as the
    /// glyph's fill comes in. `size` is the text size, which sets the pen's width.
    Glyphs {
        glyphs: Vec<Glyph>,
        size: Positive,
    },
    /// A morph's source while the morph plays: the mix of its two morph ends, one path per
    /// glyph or flat shape, each with its own mixed stroke and fill.
    Mix(Vec<MixedPath>),
    Sphere {
        radius: Positive,
        material: Material,
    },
    /// A prism, a revolution, a tube or a surface, lit and outlined with edges.
    Solid {
        solid: Solid,
        material: Material,
        edges: EdgeShade,
    },
    /// The atoms and bonds of a structure file, read when the scene file was read, drawn as the
    /// representation says. Shared, because a stage is cloned for every frame.
    Molecule {
        molecule: Arc<Molecule>,
        representation: Representation,
        /// The molecule's named atom groups, checked against its atoms.
        groups: Groups,
        /// The glow of each group that a highlight has touched. A group not listed has none. The
        /// timeline's tracks replace the values here at every scene time.
        glows: BTreeMap<GroupId, Glow>,
    },
    /// Two axes with ticks and labels, in a box centered on the object's position. A plot is a
    /// `Flat` shape in these axes' local space that they place.
    Axes(Axes),
    /// One bar for each category of its axes' category axis, in those axes' local space,
    /// which they place as they place a plot.
    BarPlot(BarPlot),
    /// Text and math in cells, in rows and columns, with rulings between them, centered on the
    /// object's position: every cell's glyphs in reading order, which a write draws after the
    /// rulings, and the glow of each lit cell.
    Table(Table),
    NumberLine(NumberLine),
    /// Three axes with ticks and labels, z up, in a box centered on the object's position. Each
    /// label faces the camera at every frame.
    Axes3d(Axes3d),
    /// One flat shape or solid drawn at every copy of a layout, each copy placed by its own
    /// transform in the object's space.
    Pattern(Pattern),
    /// The world points an object passed through, drawn as a stroke that faces the camera. It
    /// has no transform of its own: its points are in the world.
    Trail(Trail),
}

impl Shape {
    /// What an object of this shape is, said of one placed by axes: a plot on 2D axes, or a
    /// surface or a tube on 3D axes.
    pub fn what_axes_place(&self) -> &'static str {
        match self {
            Shape::Solid {
                solid: Solid::Surface { .. },
                ..
            } => "a surface on 3D axes",
            Shape::Solid {
                solid: Solid::Tube { .. },
                ..
            } => "a tube on 3D axes",
            Shape::BarPlot(_) => "a bar plot",
            _ => "a plot",
        }
    }

    /// How far the shape's farthest drawn point lies from the object's position, in its own
    /// space: what an object attached to it is drawn nearer than, so nothing of it covers that
    /// object.
    pub fn reach(&self) -> f32 {
        let of_box = |rect: kurbo::Rect| {
            [
                (rect.x0, rect.y0),
                (rect.x1, rect.y0),
                (rect.x0, rect.y1),
                (rect.x1, rect.y1),
            ]
            .into_iter()
            .map(|(x, y)| x.hypot(y) as f32)
            .fold(0.0, f32::max)
        };
        let of_path = |path: &Path, stroke: &Option<Stroke>| {
            of_box(path.bounding_box()) + stroke.map_or(0.0, |s| s.width.get() / 2.0)
        };
        match self {
            Shape::Flat { path, stroke, .. } => of_path(path, stroke),
            Shape::Glyphs { glyphs, .. } => glyphs
                .iter()
                .map(|g| of_box(g.path.bounding_box()))
                .fold(0.0, f32::max),
            Shape::Mix(paths) => paths
                .iter()
                .map(|p| of_path(&p.path, &p.stroke))
                .fold(0.0, f32::max),
            Shape::Sphere { radius, .. } => radius.get(),
            Shape::Solid { solid, .. } => solid.reach(),
            Shape::Molecule {
                molecule,
                representation,
                ..
            } => representation.reach(molecule),
            Shape::Axes(Axes { drawing, .. }) | Shape::NumberLine(NumberLine { drawing, .. }) => {
                drawing
                    .labels
                    .iter()
                    .map(|l| of_box(l.bounding_box))
                    .fold(of_path(&drawing.rulings, &Some(drawing.stroke)), f32::max)
            }
            Shape::Axes3d(axes) => axes.reach(),
            Shape::BarPlot(bar_plot) => bar_plot.reach(),
            Shape::Table(table) => table.reach(),
            Shape::Pattern(pattern) => pattern.reach(),
            Shape::Trail(trail) => {
                trail
                    .points
                    .iter()
                    .map(|point| point.length())
                    .fold(0.0, f32::max)
                    + trail.stroke.width.get() / 2.0
            }
        }
    }
}

/// A solid made from a region or a space curve, in the object's own space.
#[derive(Clone, Debug)]
pub enum Solid {
    /// The base lies in the xz plane, its x and y read as x and -z, and is pushed up along +y.
    Prism { base: Region, height: Positive },
    /// The profile lies in the xy plane at x of 0 or more and turns about the y axis, from +x
    /// toward -z, through `angle`.
    Revolution {
        profile: Region,
        angle: RevolutionAngle,
    },
    /// A circle of `radius` carried along the curve, square to it. Shared, because a stage is
    /// cloned for every frame. On 3D axes it is cut to their box, low corner first.
    Tube {
        curve: Arc<SpaceCurve>,
        radius: Positive,
        cut: Option<[Vec3; 2]>,
    },
    /// A sheet of heights over a rectangle of the xz plane, lit on both sides. Shared, because a
    /// stage is cloned for every frame.
    Surface { surface: Arc<Surface> },
}

impl Solid {
    fn reach(&self) -> f32 {
        let bounding_box = match self {
            Solid::Prism { base, .. } | Solid::Revolution { profile: base, .. } => {
                base.path.bounding_box()
            }
            Solid::Tube { curve, radius, .. } => return curve.reach() + radius.get(),
            Solid::Surface { surface } => return surface.reach(),
        };
        let (x, y) = (
            bounding_box.x0.abs().max(bounding_box.x1.abs()) as f32,
            bounding_box.y0.abs().max(bounding_box.y1.abs()) as f32,
        );
        match self {
            Solid::Prism { height, .. } => Vec3::new(x, height.get(), y).length(),
            Solid::Revolution { .. } => x.hypot(y),
            Solid::Tube { .. } | Solid::Surface { .. } => unreachable!("returned above"),
        }
    }
}

/// What a fill of `path` under `rule` would fill: a base or a profile.
#[derive(Clone, Debug)]
pub struct Region {
    pub path: Path,
    pub rule: FillRule,
}

/// How a solid's edges are drawn: a darker shade of its color, or a lighter one for a dark solid
/// whose dark edges would vanish against the background. No variant doc comments, for the reason
/// `FillRule` gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeShade {
    #[default]
    Dark,
    Light,
}

impl EdgeShade {
    /// The color of the edges of a solid of `color`, mixed in linear light.
    pub fn of(self, color: Color) -> Color {
        match self {
            EdgeShade::Dark => color.mix(Color { r: 0, g: 0, b: 0 }, 0.88),
            EdgeShade::Light => color.mix(
                Color {
                    r: 0xff,
                    g: 0xff,
                    b: 0xff,
                },
                0.7,
            ),
        }
    }
}

/// One drawn mark of a text or math object: its path, in the object's own plane, the color it is
/// filled in, and the characters it shows. A math glyph's color is the one LaTeX gives it, or the
/// object's `color`.
#[derive(Clone, Debug)]
pub struct Glyph {
    pub path: Path,
    pub color: Color,
    /// The characters the glyph shows, such as `r`, `π` or a ligature's `ffi`, and empty for a
    /// rule. A morph between two text or math objects pairs glyphs that show the same characters.
    pub characters: String,
}

/// One path of a mix, with the stroke and fill mixed for it.
#[derive(Clone, Debug)]
pub struct MixedPath {
    pub path: Path,
    pub stroke: Option<Stroke>,
    pub fill: Option<Fill>,
}

/// The scene config: the frame size in pixels, frame rate and background.
/// Every field has a default, so the scene file can leave out any of them or all.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SceneConfig {
    /// The width of each frame in pixels, from 1 to 8192. The default is 1920.
    pub width: FrameDimension,
    /// The height of each frame in pixels, from 1 to 8192. The default is 1080.
    pub height: FrameDimension,
    /// The frame rate, in frames per second, from 1 to 240. The default is 60.
    pub fps: FrameRate,
    /// An opaque `#rrggbb` color or `transparent`. The default is `#1e1e1e`.
    pub background: Background,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            width: FrameDimension(1920),
            height: FrameDimension(1080),
            fps: FrameRate(60),
            background: Background::Opaque(Color {
                r: 0x1e,
                g: 0x1e,
                b: 0x1e,
            }),
        }
    }
}

/// The viewpoint the scene is seen from, with perspective. World space has x to the right, y up
/// and z toward the default camera, in world units. The default camera stands at `[0, 0, 10]` and
/// looks at the origin with a field of view of 45 degrees, so the frame shows about 8.3 world
/// units from bottom to top in the plane z = 0, and 14.7 across at 16:9.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Camera {
    /// Where the camera stands, as `[x, y, z]` in world units. The default is `[0, 0, 10]`.
    #[schemars(with = "[f32; 3]")]
    pub position: Vec3,
    /// The point the camera looks at, as `[x, y, z]` in world units. It must differ from
    /// `position`. The default is the origin.
    #[schemars(with = "[f32; 3]")]
    pub target: Vec3,
    /// The direction that is up in the frame, as `[x, y, z]`. Any length works, but it must not
    /// point along the line from `position` to `target`. The default is `[0, 1, 0]`.
    #[schemars(with = "[f32; 3]")]
    pub up: Vec3,
    /// The vertical field of view in degrees, more than 0 and less than 180. Smaller shows less
    /// and so zooms in. The default is 45.
    pub fov: FieldOfView,
}

/// The camera's near plane: nothing nearer is drawn or has a pixel.
const NEAR: f32 = 0.1;

impl Camera {
    /// wgpu's clip space has depth from 0 to 1 and y up, which glam calls the DirectX convention.
    pub fn world_to_clip(&self, aspect: f32) -> Mat4 {
        glam::camera::rh::proj::directx::perspective(
            self.fov.get().to_radians(),
            aspect,
            NEAR,
            1000.0,
        ) * glam::camera::rh::view::look_at_mat4(self.position, self.target, self.up)
    }

    /// The field and the reason when this camera cannot look anywhere. `scene_file` asks this of
    /// the start camera. `Timeline::play` checks the camera along every step that moves or turns
    /// it.
    pub fn check(&self) -> Option<(&'static str, &'static str)> {
        let looking = self.target - self.position;
        if looking.length() == 0.0 {
            Some((
                "target",
                "the camera's target is its position, so the camera looks nowhere",
            ))
        } else if looking.cross(self.up).length() <= 1e-6 * looking.length() * self.up.length() {
            Some((
                "up",
                "the camera's up points along the direction it looks, so up is undefined",
            ))
        } else {
            None
        }
    }

    /// The pixel of a frame that a world point lands on, x from the left edge and y from the top,
    /// as a PNG's rows run. It can lie outside the frame. `None` when the point is behind the
    /// camera's near plane.
    pub fn to_pixels(self, point: Vec3, scene_config: &SceneConfig) -> Option<Vec2> {
        let (width, height) = (
            scene_config.width.get() as f32,
            scene_config.height.get() as f32,
        );
        let clip = self.world_to_clip(width / height) * point.extend(1.0);
        (clip.w > NEAR).then(|| {
            Vec2::new(
                (clip.x / clip.w + 1.0) / 2.0 * width,
                (1.0 - clip.y / clip.w) / 2.0 * height,
            )
        })
    }

    /// How many pixels one world unit spans at `point`, across a plane that faces the camera.
    pub fn pixels_per_unit(&self, point: Vec3, scene_config: &SceneConfig) -> f32 {
        let depth = (point - self.position).dot(-self.direction());
        scene_config.height.get() as f32 / 2.0 / ((self.fov.get().to_radians() / 2.0).tan() * depth)
    }

    /// The unit vector from the target to the camera.
    pub fn direction(&self) -> Vec3 {
        (self.position - self.target).normalize()
    }

    /// The unit vector toward the key light: up and to the left of the camera and a little
    /// toward the viewer. It turns with the camera, so a camera that circles a solid keeps the
    /// light where the viewer expects it.
    pub fn key_light(&self) -> Vec3 {
        let forward = -self.direction();
        let right = forward.cross(self.up).normalize();
        let up = right.cross(forward);
        (-0.55 * right + 0.65 * up - 0.5 * forward).normalize()
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 0.0, 10.0),
            target: Vec3::ZERO,
            up: Vec3::Y,
            fov: FieldOfView(45.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Transform {
    /// Where the object sits, as `[x, y, z]` in world units, or in data coordinates for a dot on
    /// axes or a sphere on 3D axes. A flat shape's own coordinates are measured from here. The
    /// default is the origin.
    #[schemars(with = "[f32; 3]")]
    pub position: Vec3,
    /// Angles in degrees. The object turns about its x axis, then its turned y axis, then its
    /// turned z axis. The default is `[0, 0, 0]`, which leaves a flat shape facing the default
    /// camera.
    #[schemars(with = "[f32; 3]")]
    pub rotation: Vec3,
    /// A factor along each of the object's own x, y and z axes. A stroke keeps its width under any
    /// scale. The default is `[1, 1, 1]`.
    #[schemars(with = "[f32; 3]")]
    pub scale: Vec3,
}

impl Transform {
    pub fn object_to_world(&self) -> Mat4 {
        let [x, y, z] = self.rotation.to_array().map(f32::to_radians);
        Mat4::from_scale_rotation_translation(
            self.scale,
            Quat::from_euler(EulerRot::XYZ, x, y, z),
            self.position,
        )
    }

    /// The transform with its x and y scale set to 1, for a flat shape whose meshes already
    /// have them.
    pub fn unscaled(&self) -> Mat4 {
        Transform {
            scale: Vec3::new(1.0, 1.0, self.scale.z),
            ..*self
        }
        .object_to_world()
    }
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Vec3::ZERO,
            scale: Vec3::ONE,
        }
    }
}

/// How a surface looks under light. A solid, such as a sphere, a prism or a molecule, is lit by
/// a key light from the camera's upper left, so its side toward that light shows exactly its
/// color and its far side is darker. A flat shape is lit from the camera, so it shows exactly
/// its color where it faces the camera.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Material {
    /// A surface with a color and how rough and how metallic it is.
    Standard {
        /// The surface's color, written `#rrggbb`.
        color: Color,
        /// From 0, a glossy surface with a sharp highlight, to 1, a matte one. The default is 0.5.
        #[serde(default = "UnitInterval::half")]
        roughness: UnitInterval,
        /// From 0, a plastic-like surface, to 1, a metal one. The default is 0.
        #[serde(default = "UnitInterval::zero")]
        metallic: UnitInterval,
        /// How much the surface hides what is behind it, from 0 (clear) to 1 (solid). A
        /// see-through surface shows its own far side. To fade the whole object, use the
        /// object's `opacity` instead. The default is 1.
        #[serde(default = "UnitInterval::one")]
        opacity: UnitInterval,
    },
}

impl Material {
    pub fn opacity(&self) -> f32 {
        match self {
            Material::Standard { opacity, .. } => opacity.get(),
        }
    }

    pub fn linear_rgba(&self) -> [f32; 4] {
        match self {
            Material::Standard { color, opacity, .. } => color.linear(opacity.get()),
        }
    }

    /// A solid surface of one color, lit like a sphere's.
    pub fn solid(color: Color) -> Material {
        Material::Standard {
            color,
            roughness: UnitInterval::half(),
            metallic: UnitInterval::zero(),
            opacity: UnitInterval::one(),
        }
    }

    /// A matte surface of one color, as a stroke or a fill is.
    fn flat(color: Color, opacity: UnitInterval) -> Material {
        Material::Standard {
            color,
            roughness: UnitInterval::one(),
            metallic: UnitInterval::zero(),
            opacity,
        }
    }
}

impl Default for Material {
    fn default() -> Self {
        Material::Standard {
            color: Color {
                r: 0xff,
                g: 0xff,
                b: 0xff,
            },
            roughness: UnitInterval::half(),
            metallic: UnitInterval::zero(),
            opacity: UnitInterval::one(),
        }
    }
}

/// The outline of a flat shape. Its corners are mitered, a corner sharper than about 54 degrees
/// is beveled, and its ends are cut square. It may be dashed. The default, for a line or an arrow
/// that sets none, is white and 0.04 wide.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, JsonSchema)]
pub struct Stroke {
    /// The stroke's color, written `#rrggbb`.
    pub color: Color,
    /// The stroke's width in world units, more than 0. The object's scale does not change it.
    /// The default is 0.04.
    #[serde(default = "Stroke::default_width")]
    pub width: Positive,
    /// A dash pattern, `[dash, gap]`: the stroke is drawn as dashes `dash` long with gaps `gap`
    /// long between them, in world units along the stroke, measured as its width is. The dashes
    /// start at the start of each part of the path, so a line placed by its ends keeps the
    /// length of its dashes as it stretches, and the last dash is cut where the stroke ends. An
    /// arrow's tip is drawn whole. A trail's and 3D axes' strokes are not dashed. Needs sci_anim
    /// 0.14.0 or later. The default is none: one unbroken stroke.
    #[serde(default)]
    pub dash: Option<DashPattern>,
}

/// A dash pattern: a dash's length, then the length of the gap after it, in world units, each
/// from 0.01 to 100.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, JsonSchema)]
pub struct DashPattern(pub DashLength, pub DashLength);

impl Stroke {
    fn default_width() -> Positive {
        Positive(0.04)
    }

    /// The pen a write outlines a glyph with: in the glyph's color, `PEN_EMS` of an em wide.
    pub fn pen(color: Color, size: Positive) -> Stroke {
        Stroke {
            color,
            width: Positive(size.0 * PEN_EMS),
            dash: None,
        }
    }

    pub fn material(&self) -> Material {
        Material::flat(self.color, UnitInterval::one())
    }
}

impl Default for Stroke {
    fn default() -> Self {
        Stroke {
            color: Color {
                r: 0xff,
                g: 0xff,
                b: 0xff,
            },
            width: Stroke::default_width(),
            dash: None,
        }
    }
}

/// A pen is this share of an em wide, thin enough that a glyph does not look bold
/// while it is drawn.
const PEN_EMS: f32 = 0.025;

/// The inside of a flat shape.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize, JsonSchema)]
pub struct Fill {
    /// The fill's color, written `#rrggbb`.
    pub color: Color,
    /// How much the fill hides what is behind it, from 0 (clear) to 1 (solid). The default is 1.
    #[serde(default = "UnitInterval::one")]
    pub opacity: UnitInterval,
    /// Which parts of a path that crosses itself, or has parts inside parts, are filled:
    /// `non-zero` or `even-odd`. The default is `non-zero`.
    #[serde(default)]
    pub rule: FillRule,
}

impl Fill {
    /// A solid fill in `color`, as an arrow's tip is filled in its stroke's color.
    pub fn solid(color: Color) -> Fill {
        Fill {
            color,
            opacity: UnitInterval::one(),
            rule: FillRule::default(),
        }
    }

    /// The fill's material at `shown` of its opacity, which is less than 1 while a create fades
    /// the fill in.
    pub fn material(&self, shown: f32) -> Material {
        Material::flat(
            self.color,
            UnitInterval::clamped(self.opacity.get() * shown),
        )
    }
}

/// How a fill decides what is inside a path that crosses itself or has parts inside parts.
/// `non-zero`, the default, fills where the path winds around a point, so a part inside another
/// is a hole only when it runs the other way round. `even-odd` fills where an odd number of
/// parts enclose a point, so a part inside another is always a hole. The variants carry no doc
/// comments because schemars would then write a `oneOf`, which hides the list of values from
/// the error an agent gets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FillRule {
    #[default]
    NonZero,
    EvenOdd,
}

/// How the lines of a text or math object line up, and which point of its box sits at the
/// object's position: the middle of its left edge, its center, or the middle of its right edge.
/// No variant doc comments, for the reason `FillRule` gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Align {
    Left,
    #[default]
    Center,
    Right,
}

/// The typeface a text object is set in: Libertinus Sans or Libertinus Serif. Math is always set
/// in New Computer Modern Math. No variant doc comments, for the reason `FillRule` gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Font {
    #[default]
    Sans,
    Serif,
}

pub const HEX_COLOR_PATTERN: &str = "^#[0-9a-fA-F]{6}$";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub enum Background {
    Opaque(Color),
    Transparent,
}

impl TryFrom<String> for Background {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        match text.as_str() {
            "transparent" => Ok(Self::Transparent),
            _ => Color::try_from(text).map(Self::Opaque),
        }
    }
}

impl JsonSchema for Background {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "Background".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": "^(#[0-9a-fA-F]{6}|transparent)$",
            "description": "An opaque sRGB color written #rrggbb, or transparent for an empty background.",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    /// The color as a scene file writes it: `#rrggbb`.
    pub fn hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    pub fn linear(self, opacity: f32) -> [f32; 4] {
        let decode = |byte: u8| {
            let c = byte as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        [decode(self.r), decode(self.g), decode(self.b), opacity]
    }

    /// The color `weight` of the way from `self` to `to`, mixed in linear light as the renderer
    /// blends, so `#ff0000` and `#0000ff` meet at `#bc00bc` and not at a darker `#800080`.
    pub fn mix(self, to: Color, weight: f32) -> Color {
        let (a, b) = (self.linear(1.0), to.linear(1.0));
        let encode = |k: usize| {
            let c = (a[k] + (b[k] - a[k]) * weight).clamp(0.0, 1.0);
            let c = if c <= 0.0031308 {
                c * 12.92
            } else {
                1.055 * c.powf(1.0 / 2.4) - 0.055
            };
            (c * 255.0).round() as u8
        };
        Color {
            r: encode(0),
            g: encode(1),
            b: encode(2),
        }
    }
}

impl TryFrom<String> for Color {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        let digits = text
            .strip_prefix('#')
            .filter(|d| d.len() == 6 && d.bytes().all(|b| b.is_ascii_hexdigit()));
        let Some(digits) = digits else {
            return Err(format!("{text:?} does not match {HEX_COLOR_PATTERN}"));
        };
        let channel = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).expect("two hex digits");
        Ok(Color {
            r: channel(0),
            g: channel(2),
            b: channel(4),
        })
    }
}

impl JsonSchema for Color {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "Color".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": HEX_COLOR_PATTERN,
            "description": "An sRGB color written #rrggbb.",
        })
    }
}

/// A number type whose JSON Schema keywords sit beside the Rust check that enforces them.
macro_rules! value_rule {
    ($(#[$get:meta])* $name:ident($inner:ty) from $raw:ty, schema: $schema:tt, accepts: |$v:ident| $accepts:expr) => {
        #[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
        pub struct $name($inner);

        impl $name {
            $(#[$get])*
            pub fn get(self) -> $inner {
                self.0
            }
        }

        impl TryFrom<$raw> for $name {
            type Error = String;

            fn try_from($v: $raw) -> Result<Self, String> {
                if $accepts {
                    Ok($name($v as $inner))
                } else {
                    Err(format!("{} does not satisfy {}", $v, stringify!($schema)))
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                self.0.serialize(serializer)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = <$raw>::deserialize(deserializer)?;
                Self::try_from(raw).map_err(serde::de::Error::custom)
            }
        }

        impl JsonSchema for $name {
            fn inline_schema() -> bool {
                true
            }

            fn schema_name() -> Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_: &mut SchemaGenerator) -> Schema {
                json_schema!($schema)
            }
        }
    };
}

value_rule!(Positive(f32) from f64,
    schema: { "type": "number", "exclusiveMinimum": 0 },
    accepts: |v| v > 0.0 && v.is_finite());
value_rule!(
    /// A dash's or a gap's length in a dash pattern, in world units from 0.01 to 100, so a stroke
    /// the frame shows is cut into at most a few thousand dashes.
    DashLength(f32) from f64,
    schema: { "type": "number", "minimum": 0.01, "maximum": 100, "description": "World units, from 0.01 to 100." },
    accepts: |v| (0.01..=100.0).contains(&v));
value_rule!(UnitInterval(f32) from f64,
    schema: { "type": "number", "minimum": 0, "maximum": 1 },
    accepts: |v| (0.0..=1.0).contains(&v));
value_rule!(FrameDimension(u32) from u64,
    schema: { "type": "integer", "minimum": 1, "maximum": 8192, "description": "Pixels." },
    accepts: |v| (1..=8192).contains(&v));
value_rule!(FrameRate(u32) from u64,
    schema: { "type": "integer", "minimum": 1, "maximum": 240, "description": "Frames per second." },
    accepts: |v| (1..=240).contains(&v));
value_rule!(Duration(f64) from f64,
    schema: { "type": "number", "exclusiveMinimum": 0, "description": "Seconds." },
    accepts: |v| v > 0.0 && v.is_finite());
value_rule!(
    /// Seconds from the start of a step, 0 or more.
    SecondsIntoStep(f64) from f64,
    schema: { "type": "number", "minimum": 0, "description": "Seconds from the start of the step." },
    accepts: |v| v >= 0.0 && v.is_finite());
value_rule!(
    /// An entry of a linear map's matrix, from −100 to 100, so three linear maps in a row stay
    /// far inside what a 32-bit float holds.
    MatrixEntry(f32) from f64,
    schema: { "type": "number", "minimum": -100, "maximum": 100, "description": "A number from -100 to 100." },
    accepts: |v| (-100.0..=100.0).contains(&v));
value_rule!(
    /// A coordinate of a linear map's map origin, in world units from −10000 to 10000.
    OriginCoordinate(f32) from f64,
    schema: { "type": "number", "minimum": -10000, "maximum": 10000, "description": "World units, from -10000 to 10000." },
    accepts: |v| (-10000.0..=10000.0).contains(&v));
value_rule!(
    /// How many decimals a readout shows, from 0 to `readout::MOST_DECIMALS`.
    Decimals(u8) from u64,
    schema: { "type": "integer", "minimum": 0, "maximum": 6, "description": "A whole number of decimals, from 0 to 6." },
    accepts: |v| v <= u64::from(crate::readout::MOST_DECIMALS));
value_rule!(
    /// Any finite number: a readout's value, or a simulation's variable at the start of its span.
    FiniteNumber(f64) from f64,
    schema: { "type": "number", "description": "A number." },
    accepts: |v| v.is_finite());

impl Decimals {
    pub fn none() -> Self {
        Decimals(0)
    }
}

impl FiniteNumber {
    pub fn zero() -> Self {
        FiniteNumber(0.0)
    }
}

value_rule!(FieldOfView(f32) from f64,
    schema: { "type": "number", "exclusiveMinimum": 0, "exclusiveMaximum": 180 },
    accepts: |v| v > 0.0 && v < 180.0);
value_rule!(
    /// Degrees, counterclockwise when positive. At most one full turn either way.
    Sweep(f32) from f64,
    schema: { "type": "number", "minimum": -360, "maximum": 360, "description": "Degrees, counterclockwise when positive." },
    accepts: |v| (-360.0..=360.0).contains(&v));

value_rule!(
    /// A count of a pattern's copies: a ring's or an along's `count`, or one of a grid's
    /// `counts`, from 1 to `pattern::MAX_COPIES`.
    CopyCount(u32) from u64,
    schema: { "type": "integer", "minimum": 1, "maximum": 10000, "description": "A whole number of copies, from 1 to 10000." },
    accepts: |v| (1..=crate::pattern::MAX_COPIES).contains(&v));

value_rule!(
    /// How many turns a spring's coil makes: more than 0, and at most `ends::MOST_TURNS`.
    Turns(f32) from f64,
    schema: { "type": "number", "exclusiveMinimum": 0, "maximum": 100, "description": "Turns, more than 0 and at most 100." },
    accepts: |v| v > 0.0 && v <= f64::from(crate::ends::MOST_TURNS));

value_rule!(
    /// A table's cell padding, in world units from 0 to 100.
    CellPadding(f32) from f64,
    schema: { "type": "number", "minimum": 0, "maximum": 100, "description": "World units, from 0 to 100." },
    accepts: |v| (0.0..=100.0).contains(&v));

value_rule!(
    /// Degrees a revolution turns its profile through: more than 0, and at most a full turn.
    RevolutionAngle(f32) from f64,
    schema: { "type": "number", "exclusiveMinimum": 0, "maximum": 360, "description": "Degrees." },
    accepts: |v| v > 0.0 && v <= 360.0);

impl RevolutionAngle {
    pub fn full() -> Self {
        RevolutionAngle(360.0)
    }
}

impl FieldOfView {
    /// For a field of view an overshoot carries toward 0 or 180 degrees.
    pub fn clamped(degrees: f32) -> Self {
        FieldOfView(degrees.clamp(1.0, 179.0))
    }
}

impl UnitInterval {
    /// For a value an easing curve carried past 1 or below 0, such as an opacity under an
    /// overshoot.
    pub fn clamped(value: f32) -> Self {
        UnitInterval(value.clamp(0.0, 1.0))
    }

    fn zero() -> Self {
        UnitInterval(0.0)
    }

    fn half() -> Self {
        UnitInterval(0.5)
    }

    pub(crate) fn one() -> Self {
        UnitInterval(1.0)
    }
}
