//! Time: animations, the easing curves that shape them, and the tracks they become.
//!
//! `scene_file` reads the steps in order and hands each to `Timeline::wait` or `Timeline::play`,
//! which give every value an animation changes its own track: one list of segments, each with
//! the value it starts from, the value it ends at, and when. A segment's start value is resolved
//! here, from the end of the track's previous segment or from the start stage, so
//! `Timeline::stage_at` needs no history and any scene time costs the same.
//!
//! `track` holds tracks and their segments, `follows` puts follows and simulations on them, and
//! `stage` builds the stage at a scene time from them.

use std::collections::BTreeMap;
use std::sync::Arc;

use glam::Vec3;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::data_point::DataToWorld;
use crate::follow::{self, Carry, Follow, Followed, Law, LawTimeStart, Outright};
use crate::group::Glow;
use crate::member::{Member, OwnColors};
use crate::morph::{MorphEnd, MorphPlan};
use crate::pattern;
use crate::plane_map::{LinearMap, PLANE_MAP_BOUND, PlaneMap};
use crate::reaction::{Blend, CarriedGlow, GroupRef, Reaction, Side};
use crate::scene::{
    Camera, Color, Duration, FieldOfView, FlatToWorld, FrameRate, Material, Object, ObjectId,
    Phase, PlacedBy, Shape, Stage, StagedMorph, StagedReaction, Transform, UnitInterval,
    frame_times_in,
};
use crate::scene_file::{JsonPath, SceneError, SceneErrorCode, ShapeFault, ShapeLaw};
use crate::simulate::{self, NoValue, Set, Simulate, Simulation};
use crate::surface::Sampling;

mod follows;
mod stage;
mod track;

use stage::*;
pub use track::Mix;
use track::*;

/// A change to one object or to the camera, with the easing curve that shapes it and its span.
#[derive(Debug)]
pub struct Animation {
    pub easing_curve: EasingCurve,
    pub change: Change,
    /// The span: when the animation starts and ends, in seconds from the start of its step.
    /// `scene_file` has checked that it lies inside the step and is not empty.
    pub span: (f64, f64),
}

/// What an animation changes, and the value it ends at. Each value is the one the object or
/// camera field of the same name takes, so `Move` ends at a `position` and `Fade` at an
/// `opacity`. An animation reads its start value from the stage at the start of its step, except
/// a create, whose created share runs from 0 to 1 and is 0 before the step.
#[derive(Debug)]
pub enum Change {
    /// Fades the object, or with a `member` only that copy of a pattern, whose own opacity the
    /// pattern's object opacity applies on top of.
    Fade {
        object_id: ObjectId,
        member: Option<Member>,
        to: UnitInterval,
    },
    /// Counts a readout's value to `to`. `scene_file` has checked that the object is a readout.
    Count {
        object_id: ObjectId,
        to: f64,
    },
    /// Moves the object, or with a `member` only that copy of a pattern, whose position is in
    /// the pattern's own space, so the pattern's own transform still carries it.
    Move {
        object_id: ObjectId,
        member: Option<Member>,
        to: Vec3,
    },
    Scale {
        object_id: ObjectId,
        to: Vec3,
    },
    /// Each angle moves straight from its old value to its new one, so `[0, 360, 0]` from
    /// `[0, 0, 0]` turns the object a full turn.
    Rotate {
        object_id: ObjectId,
        to: Vec3,
    },
    /// Draws the object's flat shape, grows its solid or a pattern's copies, or writes its
    /// glyphs: a `write` in the scene file is a create of text or math, and `scene_file` has
    /// checked which the object is. How a created share shows is `mesh::created_shares` for a
    /// flat shape, and `mesh::paced_shares` for glyphs and for a pattern's copies.
    Create {
        object_id: ObjectId,
    },
    /// Takes the object's stroke color, fill color, or the color of every glyph of text or math
    /// or of a solid, to the colors given, mixed in linear light. A pattern's are its shape's. A
    /// color left out keeps the color it has, except a fill in its stroke's color, as an arrow's
    /// tip is, which a recolor of the stroke takes along. `scene_file` has checked that the
    /// object has each color named. With a `member`, only that copy or bar is recolored, and the
    /// colors it is given are its own from then on, so a later recolor of the object leaves them.
    Recolor {
        object_id: ObjectId,
        member: Option<Member>,
        stroke: Option<Color>,
        fill: Option<Color>,
        color: Option<Color>,
    },
    CameraMove {
        to: Vec3,
    },
    CameraLook {
        to: Vec3,
    },
    CameraZoom {
        to: FieldOfView,
    },
    /// Lights the member of `object_id` to `to`, whose share is 0 for no glow and 1 for all of
    /// it. `scene_file` has checked that the object has the member. A color left out keeps the
    /// color the glow has.
    Highlight {
        object_id: ObjectId,
        member: Member,
        to: Glow,
        keeps_color: bool,
    },
    /// Moves every atom of the reactants to its product position, and owns the transforms and
    /// object opacities of every object it names for the step, so no other animation of the same
    /// step changes them. `scene_file` has checked the atom map.
    Reaction {
        reaction: Arc<Reaction>,
    },
    /// Reshapes `source` into the shape, stroke and fill of `target` over its span, moving it to
    /// where `target` is as it goes. The source shows until the morph ends and the target from
    /// then, with no fade written. `scene_file` has checked that both can be morphed, and that
    /// two plots share their axes when `between_plots`, and that two tubes pass through as many
    /// curve points.
    Morph {
        source: ObjectId,
        target: ObjectId,
        between_plots: bool,
    },
    /// Carries every point of each object in `object_ids` by `linear_map` over its span, after
    /// the plane map each already has. `scene_file` has checked that each is a flat shape, text,
    /// math, axes or a number line placed by its own transform, listed once.
    LinearMap {
        object_ids: Vec<ObjectId>,
        linear_map: Arc<LinearMap>,
    },
    /// Integrates its variables over its span and sets positions, rotations and readouts' values
    /// from them, each on the track of the value it sets. `scene_file` has checked its form and
    /// that each value it names is a readout's; `Timeline::play` integrates it.
    Simulate(Arc<Simulate>),
    /// Sets a position, a rotation, a scale, an object opacity, a readout's value, or the
    /// camera's position or target from a law at every
    /// scene time of its span, on the track of the value it sets. `scene_file` has checked the
    /// law's form; `Timeline::play` checks its values at every frame time of its span.
    Follow(Follow),
}

impl Change {
    /// The object whose position, rotation or scale this change sets in its own transform, if
    /// any: a move, scale, rotate or follow of an object. The one list that the `mapped_object`,
    /// `placed_by_axes` and `attached` guards read, so a change that sets a transform joins every
    /// guard by joining this match.
    fn transform_of(&self) -> Option<&ObjectId> {
        match self {
            Change::Move {
                object_id,
                member: None,
                ..
            }
            | Change::Scale { object_id, .. }
            | Change::Rotate { object_id, .. } => Some(object_id),
            Change::Follow(follow) => follow.object_id(),
            Change::Move {
                member: Some(_), ..
            }
            | Change::Fade { .. }
            | Change::Count { .. }
            | Change::Create { .. }
            | Change::Recolor { .. }
            | Change::CameraMove { .. }
            | Change::CameraLook { .. }
            | Change::CameraZoom { .. }
            | Change::Highlight { .. }
            | Change::Reaction { .. }
            | Change::Morph { .. }
            | Change::Simulate(_)
            | Change::LinearMap { .. } => None,
        }
    }

    /// Every object this change names, in any role.
    fn object_ids(&self) -> Vec<&ObjectId> {
        match self {
            Change::Fade { object_id, .. }
            | Change::Count { object_id, .. }
            | Change::Move { object_id, .. }
            | Change::Scale { object_id, .. }
            | Change::Rotate { object_id, .. }
            | Change::Create { object_id }
            | Change::Recolor { object_id, .. }
            | Change::Highlight { object_id, .. } => vec![object_id],
            Change::CameraMove { .. } | Change::CameraLook { .. } | Change::CameraZoom { .. } => {
                Vec::new()
            }
            Change::Reaction { reaction } => reaction
                .reactants
                .iter()
                .chain(&reaction.products)
                .map(|side| &side.object_id)
                .collect(),
            Change::Morph { source, target, .. } => vec![source, target],
            Change::LinearMap { object_ids, .. } => object_ids.iter().collect(),
            Change::Follow(Follow::Carry { object_id, carry }) => vec![object_id, &carry.along],
            Change::Follow(Follow::Law {
                followed: Followed::Value(object_id) | Followed::Opacity(object_id),
                ..
            }) => vec![object_id],
            Change::Follow(follow @ Follow::Law { .. }) => follow.object_id().into_iter().collect(),
            Change::Follow(Follow::Shape { object_id, .. }) => vec![object_id],
            Change::Simulate(simulate) => simulate.sets.iter().map(Set::object_id).collect(),
        }
    }
}

/// Turns the share of a step's duration that has passed into progress. `linear` moves at a
/// constant speed, `slow-start` and `slow-end` ease one end, `slow-both` eases both, and
/// `overshoot` passes its end value and settles back. The default is `slow-both`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EasingCurve {
    Linear,
    SlowStart,
    SlowEnd,
    #[default]
    SlowBoth,
    Overshoot,
}

/// The share of a step's duration that has passed, from 0 to 1. Only a segment makes one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ElapsedShare(f32);

/// How far an animation has gone. 0 at the start of its step and 1 at the end. Between the two
/// an overshoot curve goes past 1. Only `EasingCurve::progress` makes one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Progress(f32);

impl Progress {
    pub fn get(self) -> f32 {
        self.0
    }
}

impl EasingCurve {
    pub fn progress(self, share: ElapsedShare) -> Progress {
        const OVERSHOOT: f32 = 1.70158;
        let s = share.0;
        Progress(match self {
            EasingCurve::Linear => s,
            EasingCurve::SlowStart => s * s * s,
            EasingCurve::SlowEnd => 1.0 - (1.0 - s).powi(3),
            EasingCurve::SlowBoth => s * s * (3.0 - 2.0 * s),
            EasingCurve::Overshoot => {
                1.0 + (OVERSHOOT + 1.0) * (s - 1.0).powi(3) + OVERSHOOT * (s - 1.0).powi(2)
            }
        })
    }
}

#[derive(Debug, Default)]
struct ObjectTracks {
    position: Track<Vec3>,
    rotation: Track<Vec3>,
    scale: Track<Vec3>,
    opacity: Track<f32>,
    created: Track<f32>,
    /// A flat shape's stroke color and fill color, which a recolor changes.
    stroke_color: Track<Color>,
    fill_color: Track<Color>,
    /// The color of each glyph of text or math, or the one color of a sphere.
    color: Track<Vec<Color>>,
    /// The tracks of each member an animation names, apart from the object's own.
    members: BTreeMap<Member, MemberTracks>,
    /// Set on the source of a morph: how far the morph has gone, and the plan that turns that
    /// into a shape.
    morph: Option<MorphTrack>,
    /// Where linear maps carry the object, one segment per linear map that names it.
    plane_map: Track<MapValue>,
    /// The law time a shape law is drawn at, one segment per follow of the shape.
    shape: Track<f64>,
    /// A readout's value, which a count, a follow or a simulation changes.
    value: Track<f64>,
}

/// One member's own values: a copy's position in its pattern's own space and its own opacity, a
/// copy's or a bar's own colors, and any member's glow.
#[derive(Debug, Default)]
struct MemberTracks {
    position: Track<Vec3>,
    opacity: Track<f32>,
    stroke_color: Track<Color>,
    fill_color: Track<Color>,
    color: Track<Color>,
    glow: Track<Glow>,
}

impl MemberTracks {
    /// The colors the member's own recolors have given it by `scene_time`.
    fn own_colors_at(&self, scene_time: f64) -> OwnColors {
        OwnColors {
            stroke: self.stroke_color.since_first(scene_time),
            fill: self.fill_color.since_first(scene_time),
            color: self.color.since_first(scene_time),
        }
    }
}

#[derive(Debug)]
struct MorphTrack {
    target: ObjectId,
    span: (f64, f64),
    progress: Track<f32>,
    plan: Arc<MorphPlan>,
}

#[derive(Debug, Default)]
struct CameraTracks {
    position: Track<Vec3>,
    target: Track<Vec3>,
    fov: Track<f32>,
}

/// A scene's steps turned into one track per value they change, built one step at a time by
/// `wait` and `play` as `scene_file` reads the steps. Only animated objects have tracks.
#[derive(Debug)]
pub struct Timeline {
    /// Frames per second, from the scene config, so a step can be checked at its frame times the
    /// moment it is read, before the scene's duration is known.
    fps: f64,
    duration: f64,
    /// When each step starts and ends, in timeline order.
    steps: Vec<StepSpan>,
    /// When each chapter starts and ends, in timeline order, once its last step is read.
    chapters: Vec<ChapterSpan>,
    /// The chapter whose steps are being read, and its start, which is final once it opens.
    open_chapter: Option<(ChapterId, f64)>,
    /// Each carry so far: the object carried, the `along` object, the axes of an `along` plot,
    /// and the carry's path, so a carry along a carried object, or of an object another carry
    /// goes along, is refused at whichever is read later.
    carries: Vec<(ObjectId, ObjectId, Option<ObjectId>, JsonPath)>,
    camera: CameraTracks,
    objects: BTreeMap<ObjectId, ObjectTracks>,
    /// One per reaction, in timeline order. Its track holds the reaction's blend: the reactants'
    /// atoms before the step, the products' after, and between them the straight-line mix.
    reactions: Vec<ReactionTrack>,
    /// When a reaction made or used up each object it names, so `stage_at` shows an object
    /// exactly while it exists.
    lifetimes: BTreeMap<ObjectId, Lifetime>,
    /// For each object whose shape law draws a surface and a follow runs: every point the surface
    /// is sampled at, at any frame time of those follows, and those frame times.
    shape_samplings: BTreeMap<ObjectId, ShapeSampling>,
}

/// The points a surface a follow runs is sampled at, and the scene time and law time of each
/// frame time of its follows, where `fix_shape_samplings` checks it on those points.
#[derive(Debug)]
struct ShapeSampling {
    sampling: Sampling,
    frame_times: Vec<(f64, f64)>,
}

/// When a step starts and ends, in scene time, and the JSON path of the step.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepSpan {
    pub path: JsonPath,
    pub start: f64,
    pub end: f64,
}

/// The name a scene gives a chapter. Only `scene_file` makes one, from a chapter's `id`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChapterId(Arc<str>);

impl ChapterId {
    pub(crate) fn new(id: &str) -> Self {
        Self(Arc::from(id))
    }
}

impl std::ops::Deref for ChapterId {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl Serialize for ChapterId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

/// When a chapter starts and ends, in scene time. Its end is its last step's, or later when its
/// least duration holds the stage past that.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChapterSpan {
    pub id: ChapterId,
    pub start: f64,
    pub end: f64,
}

#[derive(Debug)]
struct ReactionTrack {
    span: (f64, f64),
    blend: Track<Blend>,
    fit_rmsd: f32,
    glows: Arc<[CarriedGlow]>,
}

/// An object a reaction makes exists from the end of that reaction, and one it uses up exists
/// until that reaction begins. An object is made at most once and used up at most once.
#[derive(Debug, Default, Clone, Copy)]
struct Lifetime {
    made_at: Option<f64>,
    used_up_at: Option<f64>,
}

impl Lifetime {
    fn at(self, scene_time: f64) -> bool {
        self.made_at.is_none_or(|made| scene_time >= made)
            && self.used_up_at.is_none_or(|used| scene_time < used)
    }

    /// The one order rule for reactions and morphs: an object is made at most once, used up at
    /// most once, never made after it is used up, and not used up by a step that begins before
    /// it is made. Returns a message for each way `claim` on a step beginning at `begins` breaks it.
    fn claim(self, object_id: &ObjectId, claim: Claim, begins: f64) -> Vec<String> {
        let mut messages = Vec::new();
        match claim {
            Claim::UsesUp => {
                if let Some(used) = self.used_up_at {
                    messages.push(format!("`{object_id}` was used up at scene time {used}, and an object is used up once"));
                }
                if let Some(made) = self.made_at.filter(|made| *made > begins) {
                    messages.push(format!(
                        "`{object_id}` is not made until scene time {made}, after this step begins"
                    ));
                }
            }
            Claim::Makes => {
                if let Some(made) = self.made_at {
                    messages.push(format!("`{object_id}` was already made at scene time {made}, and an object is made once"));
                }
                if let Some(used) = self.used_up_at {
                    messages.push(format!("`{object_id}` was used up at scene time {used}, before this step makes it; an object shows only after the reaction or morph that makes it, so use it up in a later step"));
                }
            }
        }
        messages
    }
}

/// What a reaction or a morph does to the lifetime of an object it names.
#[derive(Clone, Copy)]
enum Claim {
    /// A reactant, or a morph's source.
    UsesUp,
    /// A product, or a morph's target.
    Makes,
}

/// The colors a recolor can change on one shape: a flat shape's stroke and fill, and the color
/// of each glyph of text or math, or of a sphere or another solid. A pattern's are its shape's. A
/// shape with none of a kind has `None` or an empty list.
#[derive(Clone, Debug, PartialEq)]
struct Colors {
    stroke: Option<Color>,
    fill: Option<Color>,
    color: Vec<Color>,
}

impl Colors {
    fn of(shape: &Shape) -> Colors {
        match shape {
            Shape::Flat { stroke, fill, .. } => Colors {
                stroke: stroke.map(|s| s.color),
                fill: fill.map(|f| f.color),
                color: Vec::new(),
            },
            Shape::Glyphs { glyphs, .. } => Colors {
                stroke: None,
                fill: None,
                color: glyphs.iter().map(|g| g.color).collect(),
            },
            Shape::Table(table) => Colors {
                stroke: table.rulings.as_ref().map(|_| table.stroke.color),
                fill: None,
                color: table.glyphs.iter().map(|g| g.color).collect(),
            },
            Shape::Sphere {
                material: Material::Standard { color, .. },
                ..
            }
            | Shape::Solid {
                material: Material::Standard { color, .. },
                ..
            } => Colors {
                stroke: None,
                fill: None,
                color: vec![*color],
            },
            Shape::BarPlot(bar_plot) => Colors {
                stroke: bar_plot.stroke.map(|s| s.color),
                fill: Some(bar_plot.fill.color),
                color: Vec::new(),
            },
            Shape::Pattern(pattern) => Colors::of(&pattern.shape),
            Shape::Trail(trail) => Colors {
                stroke: Some(trail.stroke.color),
                fill: None,
                color: Vec::new(),
            },
            Shape::Mix(_)
            | Shape::Molecule { .. }
            | Shape::Axes(_)
            | Shape::NumberLine(_)
            | Shape::Axes3d(_) => Colors {
                stroke: None,
                fill: None,
                color: Vec::new(),
            },
        }
    }

    fn paint(self, shape: &mut Shape) {
        match shape {
            Shape::Flat { stroke, fill, .. } => {
                if let (Some(stroke), Some(color)) = (stroke, self.stroke) {
                    stroke.color = color;
                }
                if let (Some(fill), Some(color)) = (fill, self.fill) {
                    fill.color = color;
                }
            }
            // A readout shows more or fewer glyphs as its number grows, all in its one color.
            Shape::Glyphs { glyphs, .. } if glyphs.len() != self.color.len() => glyphs
                .iter_mut()
                .zip(self.color.first().into_iter().cycle())
                .for_each(|(glyph, color)| glyph.color = *color),
            Shape::Glyphs { glyphs, .. } => glyphs
                .iter_mut()
                .zip(self.color)
                .for_each(|(glyph, color)| glyph.color = color),
            Shape::Table(table) => {
                if let Some(color) = self.stroke {
                    table.stroke.color = color;
                }
                table
                    .glyphs
                    .iter_mut()
                    .zip(self.color)
                    .for_each(|(glyph, color)| glyph.color = color);
            }
            Shape::Sphere {
                material: Material::Standard { color, .. },
                ..
            }
            | Shape::Solid {
                material: Material::Standard { color, .. },
                ..
            } => {
                if let Some(to) = self.color.first() {
                    *color = *to;
                }
            }
            Shape::BarPlot(bar_plot) => {
                if let (Some(stroke), Some(color)) = (&mut bar_plot.stroke, self.stroke) {
                    stroke.color = color;
                }
                if let Some(color) = self.fill {
                    bar_plot.fill.color = color;
                }
            }
            Shape::Pattern(pattern) => self.paint(&mut pattern.shape),
            Shape::Trail(trail) => {
                if let Some(color) = self.stroke {
                    trail.stroke.color = color;
                }
            }
            Shape::Mix(_)
            | Shape::Molecule { .. }
            | Shape::Axes(_)
            | Shape::NumberLine(_)
            | Shape::Axes3d(_) => {}
        }
    }
}

impl ObjectTracks {
    /// The object's colors at a scene time, from its declared colors and its recolors.
    fn colors_at(&self, declared: &Colors, scene_time: f64) -> Colors {
        Colors {
            stroke: self.stroke_color.at(scene_time).or(declared.stroke),
            fill: self.fill_color.at(scene_time).or(declared.fill),
            color: self
                .color
                .at(scene_time)
                .unwrap_or_else(|| declared.color.clone()),
        }
    }
}

impl CameraTracks {
    fn at(&self, start: &Camera, scene_time: f64) -> Camera {
        let mut camera = *start;
        camera.position = self.position.at(scene_time).unwrap_or(camera.position);
        camera.target = self.target.at(scene_time).unwrap_or(camera.target);
        if let Some(fov) = self.fov.at(scene_time) {
            camera.fov = FieldOfView::clamped(fov);
        }
        camera
    }
}

impl Timeline {
    pub fn new(fps: FrameRate) -> Timeline {
        Timeline {
            fps: f64::from(fps.get()),
            duration: 0.0,
            steps: Vec::new(),
            chapters: Vec::new(),
            open_chapter: None,
            carries: Vec::new(),
            camera: CameraTracks::default(),
            objects: BTreeMap::new(),
            reactions: Vec::new(),
            lifetimes: BTreeMap::new(),
            shape_samplings: BTreeMap::new(),
        }
    }

    pub fn wait(&mut self, duration: Duration, path: JsonPath) {
        let start = self.duration;
        self.duration = to_the_nanosecond(self.duration + duration.get());
        self.steps.push(StepSpan {
            path,
            start,
            end: self.duration,
        });
    }

    /// Opens a chapter at the end of the steps so far, before its own steps are read.
    pub fn open_chapter(&mut self, id: ChapterId) {
        self.open_chapter = Some((id, self.duration));
    }

    /// Closes the open chapter, holding the stage until `min_duration` has passed when its steps
    /// end sooner.
    pub fn close_chapter(&mut self, min_duration: Option<Duration>) {
        let (id, start) = self.open_chapter.take().expect("a chapter is open");
        if let Some(min_duration) = min_duration {
            self.duration = self
                .duration
                .max(to_the_nanosecond(start + min_duration.get()));
        }
        self.chapters.push(ChapterSpan {
            id,
            start,
            end: self.duration,
        });
    }

    /// The start of the chapter with this id, among the chapters closed and the one open.
    pub fn chapter_start(&self, id: &str) -> Option<f64> {
        let open = self.open_chapter.iter().map(|(id, start)| (id, *start));
        self.chapters
            .iter()
            .map(|chapter| (&chapter.id, chapter.start))
            .chain(open)
            .find(|(chapter, _)| ***chapter == *id)
            .map(|(_, start)| start)
    }

    /// Adds a step that plays `animations`, each over its span and with the JSON path it was read
    /// from. Returns every scene error the step has: two animations of one value whose spans
    /// overlap, and a camera that looks nowhere, or along its up, at any scene time of the step.
    pub fn play(
        &mut self,
        duration: Duration,
        path: JsonPath,
        animations: &[(JsonPath, Animation)],
        start: &Stage,
    ) -> Vec<SceneError> {
        let step = (
            self.duration,
            to_the_nanosecond(self.duration + duration.get()),
        );
        self.duration = step.1;
        self.steps.push(StepSpan {
            path,
            start: step.0,
            end: step.1,
        });
        // A track takes its segments in scene time order, so animations that change one value
        // one after another in a step are pushed in the order they start. A carry is pushed
        // after every animation of the step that names its `along` object, or an `along` plot's
        // axes, and starts before the carry ends, and after anything else starting with it: the
        // `along` object's tracks are then whole up to the carry's end, so the carry's end value
        // and start are where that object really is. Its key stays below its own end, so the
        // carried object's own track still takes its segments in order.
        let push_order = |animation: &Animation| match &animation.change {
            Change::Follow(Follow::Carry { carry, .. }) => {
                let axes = match &start.objects[&carry.along].placed_by {
                    PlacedBy::Axes(axes) => Some(axes),
                    _ => None,
                };
                let places_along = |other: &Animation| {
                    other
                        .change
                        .object_ids()
                        .into_iter()
                        .any(|id| *id == carry.along || Some(id) == axes)
                };
                let latest = animations.iter().map(|(_, other)| other).filter(|other| {
                    other.span.0 < animation.span.1
                        && places_along(other)
                        && !std::ptr::eq(*other, animation)
                });
                (
                    latest
                        .map(|other| other.span.0)
                        .fold(animation.span.0, f64::max),
                    true,
                )
            }
            _ => (animation.span.0, false),
        };
        type PushOrder = (f64, bool);
        let mut in_order: Vec<(&(JsonPath, Animation), PushOrder)> = animations
            .iter()
            .map(|entry| (entry, push_order(&entry.1)))
            .collect();
        in_order.sort_by(|(_, a), (_, b)| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let in_order: Vec<&(JsonPath, Animation)> =
            in_order.into_iter().map(|(entry, _)| entry).collect();
        let mut errors: Vec<SceneError> = in_order
            .into_iter()
            .map(|(path, animation)| (path, animation, scene_span(step.0, animation.span)))
            .flat_map(|(path, animation, span)| match &animation.change {
                Change::Reaction { reaction } => {
                    self.push_reaction(reaction, span, start, animation.easing_curve, path.clone())
                }
                Change::Morph {
                    source,
                    target,
                    between_plots,
                } => self.push_morph(
                    source,
                    target,
                    *between_plots,
                    span,
                    start,
                    animation.easing_curve,
                    path.clone(),
                ),
                Change::Recolor {
                    object_id,
                    member: Some(member),
                    stroke,
                    fill,
                    color,
                } => self.push_member_recolor(
                    object_id,
                    member,
                    (*stroke, *fill, *color),
                    span,
                    start,
                    animation.easing_curve,
                    path,
                ),
                Change::Recolor {
                    object_id,
                    member: None,
                    stroke,
                    fill,
                    color,
                } => self.push_recolor(
                    object_id,
                    (*stroke, *fill, *color),
                    span,
                    start,
                    animation.easing_curve,
                    path,
                ),
                Change::LinearMap {
                    object_ids,
                    linear_map,
                } => self.push_linear_map(
                    object_ids,
                    linear_map,
                    span,
                    start,
                    animation.easing_curve,
                    path,
                ),
                Change::Simulate(simulate) => self.push_simulate(simulate, span, start, path),
                Change::Follow(follow) => match self.guard(&animation.change, start, path) {
                    Ok(()) => self.push_follow(follow, span, start, animation.easing_curve, path),
                    Err(error) => vec![error],
                },
                _ => self
                    .push(animation, span, start, path.clone())
                    .err()
                    .into_iter()
                    .collect(),
            })
            .collect();
        // Each instant the camera turns lies in the span of an animation that turns it, so
        // checking every such span checks the whole step, at the sampling each span deserves.
        let turns_the_camera = animations.iter().filter(|(_, a)| match &a.change {
            Change::CameraMove { .. } | Change::CameraLook { .. } => true,
            Change::Follow(follow) => follow.turns_the_camera(),
            _ => false,
        });
        let defect = turns_the_camera
            .filter(|_| errors.is_empty())
            .find_map(|(path, a)| {
                self.sight_defect(&start.camera, scene_span(step.0, a.span))
                    .map(|message| (path, message))
            });
        if let Some((path, message)) = defect {
            errors.push(SceneError::new(
                path.clone(),
                SceneErrorCode::Invalid,
                message,
            ));
        }
        errors
    }

    /// Refuses a change that sets the transform of an object a linear map has carried, or of an
    /// object placed by another, at its `object`: the guards every change in
    /// `Change::transform_of` shares.
    fn guard(&self, change: &Change, start: &Stage, path: &JsonPath) -> Result<(), SceneError> {
        let Some(object_id) = change.transform_of() else {
            return Ok(());
        };
        self.guard_object(object_id, start, &path.field("object"))
    }

    /// The guard for one object whose transform an animation sets, with the error at `at`.
    fn guard_object(
        &self,
        object_id: &ObjectId,
        start: &Stage,
        at: &JsonPath,
    ) -> Result<(), SceneError> {
        let path = at;
        if let Shape::Trail(trail) = &start.objects[object_id].shape {
            let message = format!(
                "`{object_id}` is a trail, drawn where `{}` has been, so move, follow or carry `{}` instead",
                trail.object, trail.object
            );
            return Err(SceneError::new(
                path.clone(),
                SceneErrorCode::PlacedByTrail,
                message,
            ));
        }
        if let Some(map) = self.mapped_by(object_id) {
            let message = format!(
                "`{object_id}` is carried by the linear map at {map}, which keeps the position, rotation and scale of every object it names from when it begins; move, scale, rotate or follow `{object_id}` before that, or carry it by another linear map"
            );
            return Err(SceneError::new(
                path.clone(),
                SceneErrorCode::MappedObject,
                message,
            ));
        }
        match &start.objects[object_id].placed_by {
            PlacedBy::Itself | PlacedBy::DataPoint(_) => Ok(()),
            PlacedBy::Axes(axes) => {
                let message = format!(
                    "`{object_id}` is {}, which is placed by its axes `{axes}`, so move, scale, rotate or follow `{axes}` instead",
                    start.objects[object_id].shape.what_axes_place()
                );
                Err(SceneError::new(
                    path.clone(),
                    SceneErrorCode::PlacedByAxes,
                    message,
                ))
            }
            PlacedBy::Ends(_) => {
                let message = format!(
                    "`{object_id}` is placed by its ends and lies between them, so move, follow or carry the objects its `from` and `to` sit on instead, or change their `offset`"
                );
                Err(SceneError::new(
                    path.clone(),
                    SceneErrorCode::PlacedByEnds,
                    message,
                ))
            }
            PlacedBy::Attachment(attachment) => {
                let to = attachment
                    .group
                    .as_ref()
                    .map_or(format!("`{}`", attachment.object), |group| {
                        format!("group `{group}` of `{}`", attachment.object)
                    });
                let message = format!(
                    "`{object_id}` is attached to {to} and sits beside it, so move, scale, rotate or follow `{}` instead, or change the `offset` in its `attach`",
                    attachment.object
                );
                Err(SceneError::new(
                    path.clone(),
                    SceneErrorCode::Attached,
                    message,
                ))
            }
        }
    }

    fn push(
        &mut self,
        animation: &Animation,
        span: (f64, f64),
        start: &Stage,
        path: JsonPath,
    ) -> Result<(), SceneError> {
        let easing_curve = animation.easing_curve;
        let object = |id: &ObjectId| &start.objects[id];
        self.guard(&animation.change, start, &path)?;
        match &animation.change {
            Change::Fade {
                object_id,
                member: None,
                to,
            } => {
                let value = format!("the opacity of `{object_id}`");
                let track = &mut self.objects.entry(object_id.clone()).or_default().opacity;
                track.push(
                    &value,
                    span,
                    object(object_id).opacity.get(),
                    to.get(),
                    easing_curve,
                    path,
                )
            }
            Change::Fade {
                object_id,
                member: Some(member),
                to,
            } => {
                let value = format!("the opacity of {member} of `{object_id}`");
                let start_opacity = copy_of(&object(object_id).shape, member).opacity;
                self.member_tracks(object_id, member).opacity.push(
                    &value,
                    span,
                    start_opacity,
                    to.get(),
                    easing_curve,
                    path,
                )
            }
            Change::Highlight {
                object_id,
                member,
                to,
                keeps_color,
            } => {
                let value = format!("the glow of {member} of `{object_id}`");
                let start_glow = glow_of(&object(object_id).shape, member);
                let track = &mut self.member_tracks(object_id, member).glow;
                let before = track.segments.last().map_or(start_glow, |s| s.to);
                let to = Glow {
                    color: if *keeps_color { before.color } else { to.color },
                    ..*to
                };
                track.push(&value, span, start_glow, to, easing_curve, path)
            }
            Change::Count { object_id, to } => {
                let value = format!("the value of `{object_id}`");
                let start_value = object(object_id)
                    .readout
                    .as_ref()
                    .expect("`scene_file` checked that the object is a readout")
                    .value;
                let track = &mut self.objects.entry(object_id.clone()).or_default().value;
                track.push(&value, span, start_value, *to, easing_curve, path)
            }
            Change::Move {
                object_id,
                member: None,
                to,
            } => {
                let value = format!("the position of `{object_id}`");
                let track = &mut self.objects.entry(object_id.clone()).or_default().position;
                track.push(
                    &value,
                    span,
                    object(object_id).transform.position,
                    *to,
                    easing_curve,
                    path,
                )
            }
            Change::Move {
                object_id,
                member: Some(member),
                to,
            } => {
                let value = format!("the position of {member} of `{object_id}`");
                let start_position = copy_of(&object(object_id).shape, member).transform.position;
                self.member_tracks(object_id, member).position.push(
                    &value,
                    span,
                    start_position,
                    *to,
                    easing_curve,
                    path,
                )
            }
            Change::Scale { object_id, to } => {
                let value = format!("the scale of `{object_id}`");
                let track = &mut self.objects.entry(object_id.clone()).or_default().scale;
                track.push(
                    &value,
                    span,
                    object(object_id).transform.scale,
                    *to,
                    easing_curve,
                    path,
                )
            }
            Change::Rotate { object_id, to } => {
                let value = format!("the rotation of `{object_id}`");
                let track = &mut self.objects.entry(object_id.clone()).or_default().rotation;
                track.push(
                    &value,
                    span,
                    object(object_id).transform.rotation,
                    *to,
                    easing_curve,
                    path,
                )
            }
            Change::Create { object_id } => {
                // Not the start stage's 1: an object the timeline creates shows nothing until its
                // first create, and `Track::at` holds this 0 before the segment.
                let value = format!("the created share of `{object_id}`");
                if let Some(made) = self.lifetimes.get(object_id).and_then(|l| l.made_at) {
                    let message = format!(
                        "`{object_id}` is made whole by a morph or a reaction at scene time {made}, so it has nothing left to create or write"
                    );
                    return Err(SceneError::new(
                        path.field("object"),
                        SceneErrorCode::MorphOrder,
                        message,
                    ));
                }
                let track = &mut self.objects.entry(object_id.clone()).or_default().created;
                if let Some(earlier) = track.segments.last().filter(|s| s.end <= span.0) {
                    // `scene_file` lets only a write draw glyphs, so the shape names the verb the agent used.
                    let verb = if matches!(object(object_id).shape, Shape::Glyphs { .. }) {
                        "written"
                    } else {
                        "created"
                    };
                    let message = format!(
                        "`{object_id}` is already {verb} by {}, and an object is {verb} once",
                        earlier.animation
                    );
                    return Err(SceneError::new(
                        path.field("object"),
                        SceneErrorCode::CreatedTwice,
                        message,
                    ));
                }
                track.push(&value, span, 0.0, 1.0, easing_curve, path)
            }
            Change::CameraMove { to } => self.camera.position.push(
                "the position of the camera",
                span,
                start.camera.position,
                *to,
                easing_curve,
                path,
            ),
            Change::CameraLook { to } => self.camera.target.push(
                "the target of the camera",
                span,
                start.camera.target,
                *to,
                easing_curve,
                path,
            ),
            Change::Reaction { .. } => unreachable!("`play` pushes a reaction"),
            Change::Recolor { .. } => unreachable!("`play` pushes a recolor"),
            Change::Morph { .. } => unreachable!("`play` pushes a morph"),
            Change::LinearMap { .. } => unreachable!("`play` pushes a linear map"),
            Change::Follow(_) => unreachable!("`play` pushes a follow"),
            Change::Simulate(_) => unreachable!("`play` pushes a simulation"),
            Change::CameraZoom { to } => self.camera.fov.push(
                "the fov of the camera",
                span,
                start.camera.fov.get(),
                to.get(),
                easing_curve,
                path,
            ),
        }
    }

    /// The JSON path of a morph that reshapes the object, as its source or its target.
    fn morph_of(&self, object_id: &ObjectId) -> Option<&JsonPath> {
        let morph = self.objects.iter().find_map(|(id, tracks)| {
            tracks
                .morph
                .as_ref()
                .filter(|morph| id == object_id || morph.target == *object_id)
        })?;
        morph
            .progress
            .segments
            .first()
            .map(|segment| &segment.animation)
    }

    /// Adds a recolor of each color named, from the color the object has when the step begins.
    /// A fill in the stroke's color goes along with the stroke unless the fill is named too.
    fn push_recolor(
        &mut self,
        object_id: &ObjectId,
        (stroke, fill, color): (Option<Color>, Option<Color>, Option<Color>),
        span: (f64, f64),
        start: &Stage,
        easing_curve: EasingCurve,
        path: &JsonPath,
    ) -> Vec<SceneError> {
        let declared = Colors::of(&start.objects[object_id].shape);
        let tracks = self.objects.entry(object_id.clone()).or_default();
        let now = tracks.colors_at(&declared, span.0);
        let fill = fill.or(stroke.filter(|_| now.fill.is_some() && now.fill == now.stroke));
        let value = |what: &str| format!("the {what} of `{object_id}`");
        let mut pushed = Vec::new();
        if let (Some(to), Some(from)) = (stroke, declared.stroke) {
            pushed.push(tracks.stroke_color.push(
                &value("stroke color"),
                span,
                from,
                to,
                easing_curve,
                path.clone(),
            ));
        }
        if let (Some(to), Some(from)) = (fill, declared.fill) {
            pushed.push(tracks.fill_color.push(
                &value("fill color"),
                span,
                from,
                to,
                easing_curve,
                path.clone(),
            ));
        }
        if let Some(to) = color {
            let to = vec![to; declared.color.len()];
            pushed.push(tracks.color.push(
                &value("color"),
                span,
                declared.color,
                to,
                easing_curve,
                path.clone(),
            ));
        }
        pushed.into_iter().filter_map(Result::err).collect()
    }

    /// The tracks of one member of an object, made empty the first time an animation names it.
    fn member_tracks(&mut self, object_id: &ObjectId, member: &Member) -> &mut MemberTracks {
        self.objects
            .entry(object_id.clone())
            .or_default()
            .members
            .entry(member.clone())
            .or_default()
    }

    /// Adds a recolor of one copy or bar, from the colors it draws when the recolor begins: its
    /// own, or else its object's then, a copy's mixed by its color share. A fill in the stroke's
    /// color goes along with the stroke unless the fill is named too.
    #[allow(clippy::too_many_arguments)]
    fn push_member_recolor(
        &mut self,
        object_id: &ObjectId,
        member: &Member,
        (stroke, fill, color): (Option<Color>, Option<Color>, Option<Color>),
        span: (f64, f64),
        start: &Stage,
        easing_curve: EasingCurve,
        path: &JsonPath,
    ) -> Vec<SceneError> {
        let mut shape = start.objects[object_id].shape.clone();
        let tracks = self.objects.entry(object_id.clone()).or_default();
        tracks
            .colors_at(&Colors::of(&shape), span.0)
            .paint(&mut shape);
        let own = tracks
            .members
            .get(member)
            .map_or_else(OwnColors::default, |m| m.own_colors_at(span.0));
        let now = match (&shape, member) {
            (Shape::Pattern(pattern), Member::Copy(_)) => {
                let copy = pattern::Copy {
                    own_colors: own,
                    ..copy_of(&shape, member).clone()
                };
                Colors::of(&pattern.copy_shape(&copy))
            }
            (Shape::BarPlot(bar_plot), Member::Bar(k)) => {
                let mut bar = bar_plot.bars[*k].clone();
                bar.own_colors = own;
                let (fill, stroke) = bar_plot.paint(&bar);
                Colors {
                    stroke: stroke.map(|s| s.color),
                    fill: Some(fill.color),
                    color: Vec::new(),
                }
            }
            _ => unreachable!(
                "`scene_file` recolors only a copy of a pattern or a bar of a bar plot"
            ),
        };
        let fill = fill.or(stroke.filter(|_| now.fill.is_some() && now.fill == now.stroke));
        let tracks = self.member_tracks(object_id, member);
        let value = |what: &str| format!("the {what} of {member} of `{object_id}`");
        let mut pushed = Vec::new();
        if let (Some(to), Some(from)) = (stroke, now.stroke) {
            pushed.push(tracks.stroke_color.push(
                &value("stroke color"),
                span,
                from,
                to,
                easing_curve,
                path.clone(),
            ));
        }
        if let (Some(to), Some(from)) = (fill, now.fill) {
            pushed.push(tracks.fill_color.push(
                &value("fill color"),
                span,
                from,
                to,
                easing_curve,
                path.clone(),
            ));
        }
        if let (Some(to), Some(&from)) = (color, now.color.first()) {
            pushed.push(tracks.color.push(
                &value("color"),
                span,
                from,
                to,
                easing_curve,
                path.clone(),
            ));
        }
        pushed.into_iter().filter_map(Result::err).collect()
    }

    /// Keeps every color of an object where it is for the step, so a recolor of it in the same
    /// step is a conflict.
    fn hold_colors(
        &mut self,
        object_id: &ObjectId,
        span: (f64, f64),
        start: &Stage,
        path: &JsonPath,
    ) -> Vec<SceneError> {
        let declared = Colors::of(&start.objects[object_id].shape);
        let tracks = self.objects.entry(object_id.clone()).or_default();
        let now = tracks.colors_at(&declared, span.0);
        let value = |what: &str| format!("the {what} of `{object_id}`");
        let linear = EasingCurve::Linear;
        let mut pushed = Vec::new();
        if let (Some(from), Some(to)) = (declared.stroke, now.stroke) {
            pushed.push(tracks.stroke_color.push(
                &value("stroke color"),
                span,
                from,
                to,
                linear,
                path.clone(),
            ));
        }
        if let (Some(from), Some(to)) = (declared.fill, now.fill) {
            pushed.push(tracks.fill_color.push(
                &value("fill color"),
                span,
                from,
                to,
                linear,
                path.clone(),
            ));
        }
        if !declared.color.is_empty() {
            pushed.push(tracks.color.push(
                &value("color"),
                span,
                declared.color,
                now.color,
                linear,
                path.clone(),
            ));
        }
        pushed.into_iter().filter_map(Result::err).collect()
    }

    /// Adds a reaction: checks that its objects are ready and in order, claims their tracks for
    /// the step, resolves their transforms at the step's start into a `ReactionBlends`, and gives each
    /// product the rotation that lines it up with the reactants. Returns every scene error found,
    /// at the reaction's own JSON paths.
    fn push_reaction(
        &mut self,
        reaction: &Arc<Reaction>,
        span: (f64, f64),
        start: &Stage,
        easing_curve: EasingCurve,
        path: JsonPath,
    ) -> Vec<SceneError> {
        let now = self.stage_at(start, span.0);
        let mut errors = Vec::new();
        let mut transforms = |sides: &[Side], field: &str| -> Vec<Transform> {
            sides
                .iter()
                .enumerate()
                .map(|(i, side)| {
                    let (id, at) = (&side.object_id, path.field(field).index(i));
                    let Object { transform, opacity, .. } = &now.objects[id];
                    let lifetime = self.lifetimes.get(id).copied().unwrap_or_default();
                    let claim = if field == "reactants" { Claim::UsesUp } else { Claim::Makes };
                    errors.extend(lifetime.claim(id, claim, span.0).into_iter().map(|message| SceneError::new(at.clone(), SceneErrorCode::ReactionOrder, message)));
                    if opacity.get() != 1.0 && lifetime.at(span.0) {
                        let message = format!("`{id}` has object opacity {} when this reaction begins, and a reaction needs 1; leave `opacity` out, because a reactant shows until the reaction and a product from its end", opacity.get());
                        errors.push(SceneError::new(at.clone(), SceneErrorCode::NotReady, message));
                    }
                    if transform.scale.x != transform.scale.y || transform.scale.y != transform.scale.z {
                        let message = format!("`{id}` has scale {}, and a reaction draws every atom at one size, so its scale must be the same along every axis", transform.scale);
                        errors.push(SceneError::new(at, SceneErrorCode::NotReady, message));
                    }
                    *transform
                })
                .collect()
        };
        let (reactant_transforms, product_transforms) = (
            transforms(&reaction.reactants, "reactants"),
            transforms(&reaction.products, "products"),
        );
        if !errors.is_empty() {
            return errors;
        }
        let glows = match self.carry_glows(reaction, &now, &path) {
            Ok(glows) => glows,
            Err(errors) => return errors,
        };
        let motion = reaction.motion(&reactant_transforms, &product_transforms);
        let rotations = reaction
            .reactants
            .iter()
            .map(|_| None)
            .chain(motion.product_rotations.iter().map(|r| Some(*r)));
        for (side, rotation) in reaction
            .reactants
            .iter()
            .chain(&reaction.products)
            .zip(rotations)
        {
            errors.extend(self.hold(&side.object_id, span, start, &path, rotation));
            errors.extend(self.hold_glows(&side.object_id, span, start, &path, &glows));
        }
        if !errors.is_empty() {
            return errors;
        }
        for side in &reaction.reactants {
            self.lifetimes
                .entry(side.object_id.clone())
                .or_default()
                .used_up_at = Some(span.0);
        }
        for side in &reaction.products {
            self.lifetimes
                .entry(side.object_id.clone())
                .or_default()
                .made_at = Some(span.1);
        }
        let mut blend = Track::default();
        blend
            .push(
                "the reaction",
                span,
                motion.from,
                motion.to,
                easing_curve,
                path,
            )
            .expect("a new track has nothing to conflict with");
        self.reactions.push(ReactionTrack {
            span,
            blend,
            fit_rmsd: motion.fit_rmsd,
            glows: glows.into(),
        });
        errors
    }

    /// The glow of every reactant group lit when the reaction begins, as the reaction carries it,
    /// or every scene error: a product with no group for the atoms a lit group gives it, and two
    /// lit groups that pass their glow to one product group.
    fn carry_glows(
        &self,
        reaction: &Reaction,
        now: &Stage,
        path: &JsonPath,
    ) -> Result<Vec<CarriedGlow>, Vec<SceneError>> {
        let mut errors = Vec::new();
        let mut carried: Vec<CarriedGlow> = Vec::new();
        for (r, side) in reaction.reactants.iter().enumerate() {
            let Shape::Molecule { glows, .. } = &now.objects[&side.object_id].shape else {
                unreachable!("a reactant is a molecule")
            };
            for (group, glow) in glows.iter().filter(|(_, glow)| glow.share.get() > 0.0) {
                match reaction.carry(r, group, *glow, path) {
                    Ok(glow) => carried.push(glow),
                    Err(more) => errors.extend(more),
                }
            }
        }
        let mut taken: BTreeMap<&GroupRef, &GroupRef> = BTreeMap::new();
        for glow in &carried {
            for to in &glow.passes_to {
                if let Some(first) = taken.insert(to, &glow.from) {
                    let product = reaction
                        .products
                        .iter()
                        .position(|p| p.object_id == to.object)
                        .expect("a glow passes to a product");
                    let from = if first.object == glow.from.object {
                        format!(
                            "groups `{}` and `{}` of `{}`",
                            first.group, glow.from.group, first.object
                        )
                    } else {
                        format!(
                            "group `{}` of `{}` and group `{}` of `{}`",
                            first.group, first.object, glow.from.group, glow.from.object
                        )
                    };
                    let message = format!(
                        "{from} both pass their glow to group `{}` of `{}`, and a group takes one glow; highlight one of them to 0 in an earlier step",
                        to.group, to.object
                    );
                    errors.push(SceneError::new(
                        path.field("products").index(product),
                        SceneErrorCode::ConflictingAnimations,
                        message,
                    ));
                }
            }
        }
        if errors.is_empty() {
            Ok(carried)
        } else {
            Err(errors)
        }
    }

    /// Keeps the glow of every group of a reaction's molecule where it is for the step, so a
    /// highlight of one in the same step is a conflict, except a product group that `carried`
    /// passes a glow to: its glow goes to that glow, invisibly, because a product shows only
    /// after the step.
    fn hold_glows(
        &mut self,
        object_id: &ObjectId,
        span: (f64, f64),
        start: &Stage,
        path: &JsonPath,
        carried: &[CarriedGlow],
    ) -> Vec<SceneError> {
        let Shape::Molecule { groups, glows, .. } = &start.objects[object_id].shape else {
            unreachable!("a reaction's object is a molecule")
        };
        let tracks = self.objects.entry(object_id.clone()).or_default();
        groups
            .iter()
            .filter_map(|(group, _)| {
                let start_glow = glows.get(group).copied().unwrap_or_else(Glow::off);
                let track = &mut tracks
                    .members
                    .entry(Member::Group(group.clone()))
                    .or_default()
                    .glow;
                let passed = carried.iter().find(|c| {
                    c.passes_to
                        .iter()
                        .any(|to| to.object == *object_id && to.group == *group)
                });
                let to = passed.map_or_else(|| track.at(span.0).unwrap_or(start_glow), |c| c.glow);
                let value = format!("the glow of group `{group}` of `{object_id}`");
                track
                    .push(
                        &value,
                        span,
                        start_glow,
                        to,
                        EasingCurve::Linear,
                        path.clone(),
                    )
                    .err()
            })
            .collect()
    }

    /// Adds a morph: checks that the source exists whole when the morph begins and the target is
    /// not yet made, then matches the two as they look when it begins, recolored or not, and
    /// gives the source tracks that carry its transform and object opacity to the target's values
    /// at its end, a progress track for the shape, and the source's and target's lifetimes. The
    /// target's tracks are held for the morph's span, so another animation of them then is
    /// `conflicting_animations`.
    #[allow(clippy::too_many_arguments)]
    fn push_morph(
        &mut self,
        source: &ObjectId,
        target: &ObjectId,
        between_plots: bool,
        span: (f64, f64),
        start: &Stage,
        easing_curve: EasingCurve,
        path: JsonPath,
    ) -> Vec<SceneError> {
        let mut errors = Vec::new();
        let (source_life, target_life) = (
            self.lifetimes.get(source).copied().unwrap_or_default(),
            self.lifetimes.get(target).copied().unwrap_or_default(),
        );
        let mut refuse = |field: &str, code, message: String| {
            errors.push(SceneError::new(path.field(field), code, message))
        };
        source_life
            .claim(source, Claim::UsesUp, span.0)
            .into_iter()
            .for_each(|message| refuse("object", SceneErrorCode::MorphOrder, message));
        target_life
            .claim(target, Claim::Makes, span.0)
            .into_iter()
            .for_each(|message| refuse("into", SceneErrorCode::MorphOrder, message));
        for (id, field) in [(source, "object"), (target, "into")] {
            if let Some(map) = self.mapped_by(id) {
                refuse(
                    field,
                    SceneErrorCode::MappedObject,
                    format!(
                        "`{id}` is carried by the linear map at {map}, and a morph reshapes declared shapes, which a plane map no longer shows; morph a copy that no linear map names"
                    ),
                );
            }
            if let Some((carried, _, _, follow)) =
                self.carries.iter().find(|(_, along, _, _)| along == id)
            {
                refuse(
                    field,
                    SceneErrorCode::Invalid,
                    format!(
                        "the follow at {follow} carries `{carried}` along `{id}`, and a carry goes along the curve an object declares, which a morph would reshape; morph a copy of `{id}` instead"
                    ),
                );
            }
        }
        if let Some(created) = self
            .objects
            .get(target)
            .and_then(|t| t.created.segments.first())
        {
            refuse(
                "into",
                SceneErrorCode::MorphOrder,
                format!(
                    "`{target}` is created or written by {}, and a morph makes its target whole at the end of the step, so leave that out",
                    created.animation
                ),
            );
        }
        let created = self
            .objects
            .get(source)
            .and_then(|t| t.created.at(span.0))
            .unwrap_or(1.0);
        if created < 1.0 {
            refuse(
                "object",
                SceneErrorCode::NotReady,
                format!(
                    "`{source}` is only {created} created when this morph begins, and a morph needs the whole source; create or write it in an earlier step"
                ),
            );
        }
        if !errors.is_empty() {
            return errors;
        }
        let now = self.stage_at(start, span.0);
        let end = |id: &ObjectId| {
            MorphEnd::of(&now.objects[id].shape)
                .expect("`scene_file` checked that both ends can be morphed")
        };
        let (from, to) = (end(source), end(target));
        let plan = Arc::new(if between_plots {
            MorphPlan::between_plots(&from, &to)
        } else {
            MorphPlan::new(&from, &to)
        });
        let after = self.stage_at(start, span.1);
        let end = &after.objects[target];
        let declared = &start.objects[source];
        let value = |what: &str| format!("the {what} of `{source}`");
        let tracks = self.objects.entry(source.clone()).or_default();
        errors.extend(
            [
                tracks.position.push(
                    &value("position"),
                    span,
                    declared.transform.position,
                    end.transform.position,
                    easing_curve,
                    path.clone(),
                ),
                tracks.rotation.push(
                    &value("rotation"),
                    span,
                    declared.transform.rotation,
                    end.transform.rotation,
                    easing_curve,
                    path.clone(),
                ),
                tracks.scale.push(
                    &value("scale"),
                    span,
                    declared.transform.scale,
                    end.transform.scale,
                    easing_curve,
                    path.clone(),
                ),
                tracks.opacity.push(
                    &value("opacity"),
                    span,
                    declared.opacity.get(),
                    end.opacity.get(),
                    easing_curve,
                    path.clone(),
                ),
            ]
            .into_iter()
            .filter_map(Result::err),
        );
        let mut progress = Track::default();
        progress
            .push("the morph", span, 0.0, 1.0, easing_curve, path.clone())
            .expect("a new track has nothing to conflict with");
        tracks.morph = Some(MorphTrack {
            target: target.clone(),
            span,
            progress,
            plan,
        });
        errors.extend(self.hold(target, span, start, &path, None));
        errors.extend(self.hold_colors(source, span, start, &path));
        errors.extend(self.hold_colors(target, span, start, &path));
        self.lifetimes.entry(source.clone()).or_default().used_up_at = Some(span.1);
        self.lifetimes.entry(target.clone()).or_default().made_at = Some(span.1);
        errors
    }

    /// The JSON path of the first linear map that names the object, if one does.
    fn mapped_by(&self, object_id: &ObjectId) -> Option<&JsonPath> {
        self.objects
            .get(object_id)?
            .plane_map
            .segments
            .first()
            .map(|segment| &segment.animation)
    }

    /// Adds a linear map of each object, after the plane map it already has. Refuses, at the
    /// object's place in `objects`, an object turned about x or y when the map begins, and one
    /// whose move, scale, rotate or morph is still playing when the map begins: from then on its
    /// transform stays as it is. Also refuses a composed plane map past `PLANE_MAP_BOUND`, at
    /// `matrix`, and two linear maps of one object at once.
    fn push_linear_map(
        &mut self,
        object_ids: &[ObjectId],
        linear_map: &Arc<LinearMap>,
        span: (f64, f64),
        start: &Stage,
        easing_curve: EasingCurve,
        path: &JsonPath,
    ) -> Vec<SceneError> {
        let mut errors = Vec::new();
        for (i, object_id) in object_ids.iter().enumerate() {
            let at = path.field("objects").index(i);
            let tracks = self.objects.entry(object_id.clone()).or_default();
            let rotation = tracks
                .rotation
                .at(span.0)
                .unwrap_or(start.objects[object_id].transform.rotation);
            if rotation.x != 0.0 || rotation.y != 0.0 {
                let message = format!(
                    "`{object_id}` is turned to rotation {rotation} when this linear map begins, and a linear map acts in the world's xy plane, so an object it names turns only about z"
                );
                errors.push(SceneError::new(at, SceneErrorCode::NotMappable, message));
                continue;
            }
            let playing = [&tracks.position, &tracks.rotation, &tracks.scale]
                .into_iter()
                .filter_map(|track| track.segments.last())
                .find(|segment| segment.end > span.0);
            if let Some(segment) = playing {
                let message = format!(
                    "the animation at {} holds or changes the transform of `{object_id}` until scene time {}, after this linear map begins, and a linear map keeps the position, rotation and scale of every object it names from when it begins; end that animation first",
                    segment.animation, segment.end
                );
                errors.push(SceneError::new(at, SceneErrorCode::MappedObject, message));
                continue;
            }
            let before = tracks
                .plane_map
                .segments
                .last()
                .map_or(PlaneMap::IDENTITY, |segment| segment.to.plane_map);
            let end = before.then(linear_map.whole());
            if !end.within_bound() {
                let message = format!(
                    "with the linear maps before it, this one carries `{object_id}` by the matrix {:?} and shift {}, past the {PLANE_MAP_BOUND} a plane map may reach; use smaller entries, or a map origin nearer the objects and (0, 0), because the shift grows with the origin's distance from (0, 0)",
                    end.rows(),
                    end.shift()
                );
                errors.push(SceneError::new(
                    path.field("matrix"),
                    SceneErrorCode::OutOfRange,
                    message,
                ));
                continue;
            }
            let value = format!("the plane map of `{object_id}`");
            let to = MapValue {
                plane_map: end,
                carried_by: Some(Arc::clone(linear_map)),
            };
            errors.extend(
                tracks
                    .plane_map
                    .push(
                        &value,
                        span,
                        MapValue::IDENTITY,
                        to,
                        easing_curve,
                        path.clone(),
                    )
                    .err(),
            );
        }
        errors
    }

    /// Keeps an object's transform and opacity where they are for the step, so an animation of
    /// the same step that changes one is a conflict. A product's rotation is the one value that
    /// changes: it goes to `rotation`, invisibly, because a product shows only after the step.
    fn hold(
        &mut self,
        object_id: &ObjectId,
        span: (f64, f64),
        start: &Stage,
        path: &JsonPath,
        rotation: Option<Vec3>,
    ) -> Vec<SceneError> {
        let declared = &start.objects[object_id];
        let tracks = self.objects.entry(object_id.clone()).or_default();
        let value = |what: &str| format!("the {what} of `{object_id}`");
        let (position, scale) = (
            tracks
                .position
                .at(span.0)
                .unwrap_or(declared.transform.position),
            tracks.scale.at(span.0).unwrap_or(declared.transform.scale),
        );
        let current_rotation = tracks
            .rotation
            .at(span.0)
            .unwrap_or(declared.transform.rotation);
        let opacity = tracks.opacity.at(span.0).unwrap_or(declared.opacity.get());
        let linear = EasingCurve::Linear;
        [
            tracks.position.push(
                &value("position"),
                span,
                declared.transform.position,
                position,
                linear,
                path.clone(),
            ),
            tracks.scale.push(
                &value("scale"),
                span,
                declared.transform.scale,
                scale,
                linear,
                path.clone(),
            ),
            tracks.rotation.push(
                &value("rotation"),
                span,
                declared.transform.rotation,
                rotation.unwrap_or(current_rotation),
                linear,
                path.clone(),
            ),
            tracks.opacity.push(
                &value("opacity"),
                span,
                declared.opacity.get(),
                opacity,
                linear,
                path.clone(),
            ),
        ]
        .into_iter()
        .filter_map(Result::err)
        .collect()
    }

    /// Why the camera cannot look anywhere at some scene time of `span`, if it cannot.
    ///
    /// The camera's position and target can each follow a different easing curve, and an
    /// overshoot is not monotonic, so the direction the camera looks is not a straight line in
    /// any one progress and has no simple closed form. Instead the two lengths that reach 0 at a
    /// defect are sampled along the span, and each sampled low point is refined by
    /// golden-section search.
    fn sight_defect(&self, start: &Camera, span: (f64, f64)) -> Option<&'static str> {
        const SAMPLES: usize = 256;
        const TOLERANCE: f32 = 1e-4;
        let camera_at = |share: f64| self.camera.at(start, span.0 + share * (span.1 - span.0));
        let sight = |share: f64| {
            let camera = camera_at(share);
            (camera.target - camera.position, camera.up.normalize())
        };
        let scale = sight(0.0).0.length().max(sight(1.0).0.length());
        let looks_nowhere = |share: f64| sight(share).0.length();
        let looks_along_up = |share: f64| {
            let (sight, up) = sight(share);
            sight.cross(up).length()
        };
        let reaches_zero = |length: &dyn Fn(f64) -> f32| {
            let lengths: Vec<f32> = (0..=SAMPLES)
                .map(|k| length(k as f64 / SAMPLES as f64))
                .collect();
            (0..=SAMPLES).any(|k| {
                let low_point = (k == 0 || lengths[k] <= lengths[k - 1])
                    && (k == SAMPLES || lengths[k] <= lengths[k + 1]);
                let bracket = (
                    k.saturating_sub(1) as f64 / SAMPLES as f64,
                    (k + 1).min(SAMPLES) as f64 / SAMPLES as f64,
                );
                low_point && least(length, bracket) <= TOLERANCE * scale
            })
        };
        if reaches_zero(&looks_nowhere) {
            Some(
                "during this step the camera's target reaches its position, so the camera looks nowhere",
            )
        } else if reaches_zero(&looks_along_up) {
            Some("during this step the camera looks along its up, so up is undefined")
        } else {
            None
        }
    }

    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// When an object begins to show: the start of its first create, the end of the reaction or
    /// morph that makes it, or scene time 0.
    pub fn shows_from(&self, object_id: &ObjectId) -> f64 {
        let created = self
            .objects
            .get(object_id)
            .and_then(|tracks| tracks.created.segments.first())
            .map(|segment| segment.start);
        let made = self
            .lifetimes
            .get(object_id)
            .and_then(|lifetime| lifetime.made_at);
        created.into_iter().chain(made).fold(0.0, f64::max)
    }

    pub fn steps(&self) -> &[StepSpan] {
        &self.steps
    }

    pub fn chapters(&self) -> &[ChapterSpan] {
        &self.chapters
    }
}

/// The copy a member names on a pattern's shape. `scene_file` has checked that the shape is a
/// pattern with that copy.
fn copy_of<'a>(shape: &'a Shape, member: &Member) -> &'a pattern::Copy {
    match (shape, member) {
        (Shape::Pattern(pattern), Member::Copy(index)) => pattern
            .ordinal(*index)
            .map(|k| &pattern.copies[k])
            .expect("`scene_file` checked that the pattern has the copy"),
        _ => unreachable!("`scene_file` checked that the member is a copy of a pattern"),
    }
}

/// The glow a member has on the start stage.
fn glow_of(shape: &Shape, member: &Member) -> Glow {
    match (shape, member) {
        (Shape::Molecule { glows, .. }, Member::Group(group)) => {
            glows.get(group).copied().unwrap_or_else(Glow::off)
        }
        (Shape::Table(table), Member::Cell(address)) => {
            table.glows.get(address).copied().unwrap_or_else(Glow::off)
        }
        (Shape::Pattern(_), Member::Copy(_)) => copy_of(shape, member).glow,
        (Shape::BarPlot(bar_plot), Member::Bar(k)) => bar_plot.bars[*k].glow,
        _ => unreachable!("`scene_file` checked that the object has the member"),
    }
}

/// The least value of `length` on `bracket`, by golden-section search, assuming one low point.
/// A scene time kept to the nanosecond, so steps of 0.7 and 0.1 seconds end at 0.8, as written,
/// and not at the float sum 0.7999999999999999.
pub(crate) fn to_the_nanosecond(seconds: f64) -> f64 {
    (seconds * 1e9).round() / 1e9
}

/// An animation's span in scene time, for a step that starts at `step_start`, kept to the
/// nanosecond as step ends are, so a span to its step's end ends with the step.
pub(crate) fn scene_span(step_start: f64, span: (f64, f64)) -> (f64, f64) {
    (
        to_the_nanosecond(step_start + span.0),
        to_the_nanosecond(step_start + span.1),
    )
}

fn least(length: &dyn Fn(f64) -> f32, bracket: (f64, f64)) -> f32 {
    let ratio = (5f64.sqrt() - 1.0) / 2.0;
    let (mut a, mut b) = bracket;
    while b - a > 1e-9 {
        let (c, d) = (b - ratio * (b - a), a + ratio * (b - a));
        if length(c) <= length(d) { b = d } else { a = c }
    }
    length(a)
        .min(length(b))
        .min(length(bracket.0))
        .min(length(bracket.1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_easing_curve_starts_at_0_and_ends_at_1() {
        for easing_curve in [
            EasingCurve::Linear,
            EasingCurve::SlowStart,
            EasingCurve::SlowEnd,
            EasingCurve::SlowBoth,
            EasingCurve::Overshoot,
        ] {
            assert_eq!(
                easing_curve.progress(ElapsedShare(0.0)),
                Progress(0.0),
                "{easing_curve:?}"
            );
            assert!(
                (easing_curve.progress(ElapsedShare(1.0)).0 - 1.0).abs() < 1e-6,
                "{easing_curve:?}"
            );
        }
    }

    #[test]
    fn easing_curves_at_a_quarter_of_the_step() {
        let quarter = |easing_curve: EasingCurve| easing_curve.progress(ElapsedShare(0.25)).0;

        assert_eq!(quarter(EasingCurve::Linear), 0.25);
        assert_eq!(quarter(EasingCurve::SlowStart), 0.015625);
        assert_eq!(quarter(EasingCurve::SlowEnd), 0.578125);
        assert_eq!(quarter(EasingCurve::SlowBoth), 0.15625);
        assert!(
            (EasingCurve::Overshoot.progress(ElapsedShare(0.75)).0 - 1.064137).abs() < 1e-6,
            "past 1 at three quarters"
        );
    }

    #[test]
    fn a_track_holds_its_first_start_value_before_its_first_segment() {
        let mut created: Track<f32> = Track::default();
        created
            .push(
                "the created share of `c`",
                (2.0, 3.0),
                0.0,
                1.0,
                EasingCurve::Linear,
                JsonPath::root(),
            )
            .unwrap();

        assert_eq!(created.at(0.0), Some(0.0));
        assert_eq!(created.at(2.5), Some(0.5));
        assert_eq!(created.at(4.0), Some(1.0));
        assert_eq!(Track::<f32>::default().at(0.0), None);
    }
}
