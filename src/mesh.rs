//! Every shape becomes meshes here, so the GPU draws a flat shape and a sphere the same way.
//! The only module that uses lyon: a flat shape's path is cut into a fill and a stroke here.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::f32::consts::{PI, TAU};

use bytemuck::{Pod, Zeroable};
use kurbo::PathEl;
use lyon::math::point;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, LineCap, LineJoin, StrokeOptions,
    StrokeTessellator, StrokeVertex, VertexBuffers,
};

use glam::{Mat2, Mat3, Mat4, Vec2, Vec3};
use serde::Serialize;

use crate::axes::Drawing;
use crate::axes_3d::{Axes3d, Facing};
use crate::group::{BALL_AND_STICK_GLOW, Glow, GlowLook};
use crate::molecule::{Drawn, Representation};
use crate::path::{Path, UnderMap};
use crate::pattern::Pattern;
use crate::scene::{
    Camera, Color, DashPattern, EdgeShade, Fill, FillRule, FlatToWorld, Glyph, Material, ObjectId,
    Phase, Positive, Region, SceneConfig, Shape, Solid, Stage, StagedReaction, Stroke, Transform,
    UnitInterval,
};
use crate::space_curve::{CrossSection, CurveFrame, SpaceCurve};
use crate::surface::Surface;
use crate::table::{Cell, FLAT_GLOW_OPACITY, Table};

pub struct DrawList {
    pub camera: Camera,
    /// In object id order. An object with opacity 0 is listed, so `inspect` still measures it,
    /// and `Gpu::draw` skips it.
    pub objects: Vec<StagedObject>,
}

/// What a staged object's meshes draw.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Object(ObjectId),
    /// The atoms and bonds of the reaction at this index of the timeline, while it plays.
    Reaction(usize),
}

/// One object's meshes with the transform and object opacity they share. A reaction that is
/// playing is staged like an object at opacity 1 whose meshes are already in world space.
pub struct StagedObject {
    pub source: Source,
    pub transform: Transform,
    /// Places the meshes in the world. A flat shape's meshes already have the transform's x and
    /// y scale in them, or the whole matrix of its plane map, so a stroke keeps its width, and
    /// this is what is left of `FlatToWorld`. For a sphere it is the transform.
    pub mesh_to_world: Mat4,
    /// Multiplies the opacity of every mesh, after `Gpu::draw` has hidden the object's own far
    /// surfaces, so the object fades as one picture.
    pub opacity: f32,
    pub meshes: Vec<Mesh>,
    /// The shells of the molecule's lit groups, one per group, in group id order. A glow with
    /// share 0 is not staged. Each is blended once per pixel after the object's own meshes and
    /// at the object's opacity, so the atoms inside show through it.
    pub glows: Vec<StagedGlow>,
}

/// One group's shell as meshes in the molecule's own space, each already as opaque as the glow's
/// share makes it. See `GlowLook`.
pub struct StagedGlow {
    pub meshes: Vec<Mesh>,
}

/// The smallest box, lined up with the axes, around an object at one scene time.
#[derive(Debug, Serialize)]
pub struct BoundingBox {
    /// In world units.
    pub world: Corners<Vec3>,
    /// In pixels of the frame, x from the left edge and y from the top, as a PNG's rows run.
    /// Corners can lie outside the frame. `None` when the whole object is behind the camera.
    pub pixels: Option<Corners<Vec2>>,
}

#[derive(Debug, Serialize)]
pub struct Corners<P> {
    pub min: P,
    pub max: P,
}

/// How much of an object's bounding box the frame shows.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum InFrame {
    Whole,
    Part,
    /// Also an object with nothing to draw.
    None,
}

impl StagedObject {
    /// Measures the same meshes the GPU draws, so a box matches what the frame shows. An object
    /// with no meshes, such as a circle with neither stroke nor fill, has no box.
    pub fn bounding_box(
        &self,
        camera: &Camera,
        scene_config: &SceneConfig,
    ) -> (Option<BoundingBox>, InFrame) {
        let (width, height) = (
            scene_config.width.get() as f32,
            scene_config.height.get() as f32,
        );
        let points: Vec<Vec3> = self
            .meshes
            .iter()
            .flat_map(|mesh| &mesh.points)
            .map(|point| {
                self.mesh_to_world
                    .transform_point3(Vec3::from(point.position))
            })
            .collect();
        let Some(world) = corners(points.iter().copied(), Vec3::min, Vec3::max) else {
            return (None, InFrame::None);
        };
        let in_front: Vec<Vec2> = points
            .iter()
            .filter_map(|&point| camera.to_pixels(point, scene_config))
            .collect();
        let pixels = corners(in_front.iter().copied(), Vec2::min, Vec2::max);
        let in_frame = match &pixels {
            None => InFrame::None,
            Some(p) if p.max.x < 0.0 || p.max.y < 0.0 || p.min.x > width || p.min.y > height => {
                InFrame::None
            }
            Some(p)
                if in_front.len() == points.len()
                    && p.min.min_element() >= 0.0
                    && p.max.x <= width
                    && p.max.y <= height =>
            {
                InFrame::Whole
            }
            Some(_) => InFrame::Part,
        };
        (Some(BoundingBox { world, pixels }), in_frame)
    }
}

pub fn corners<P: Copy>(
    mut points: impl Iterator<Item = P>,
    min: fn(P, P) -> P,
    max: fn(P, P) -> P,
) -> Option<Corners<P>> {
    let first = points.next()?;
    Some(points.fold(
        Corners {
            min: first,
            max: first,
        },
        |c, p| Corners {
            min: min(c.min, p),
            max: max(c.max, p),
        },
    ))
}

pub struct Mesh {
    pub points: Vec<Point>,
    pub triangles: Vec<[u32; 3]>,
    pub material: Material,
    /// Drawn in front of the other meshes of its object where they share a plane, as a stroke is
    /// drawn over its fill.
    pub overlay: bool,
    pub lighting: Lighting,
    /// The color `Gpu::draw` draws the mesh's edges in, from `edge_mesh`, or `None` for a mesh
    /// with no edges. Only a prism's, a revolution's, a tube's or a surface's mesh has them.
    pub edges: Option<Color>,
}

/// How the GPU lights a mesh. The values are `lighting` in `draw.wgsl`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lighting {
    /// A light at the camera, with ambient and diffuse summing to 1, so a face toward the camera
    /// shows exactly its color from either side: flat shapes, text, math, axes and glows.
    Camera = 0,
    /// The key light from the camera's upper left, with a little light from the camera so no side
    /// goes black: spheres, molecules and solids.
    KeyLight = 1,
    /// Exactly its color: an edge.
    Unlit = 2,
    /// The key light on whichever side of the mesh the camera sees: a surface, a sheet with no
    /// inside. Its triangles turn counter-clockwise toward the side its normals point to.
    Sheet = 3,
}

impl Mesh {
    pub fn is_see_through(&self) -> bool {
        self.material.opacity() < 1.0
    }

    /// The triangles as `matrix` places them. A matrix that mirrors turns a triangle's corners the
    /// other way round, so a sheet's are put back in the other order to keep turning
    /// counter-clockwise toward the side its normals point to.
    pub fn triangles_placed_by(&self, matrix: Mat4) -> Cow<'_, [[u32; 3]]> {
        match self.lighting == Lighting::Sheet && matrix.determinant() < 0.0 {
            true => Cow::Owned(self.triangles.iter().map(|&[a, b, c]| [a, c, b]).collect()),
            false => Cow::Borrowed(&self.triangles),
        }
    }
}

/// One corner of a triangle, in the object's own space. Matches `PointIn` in `draw.wgsl`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Point {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

pub fn draw_list(stage: &Stage, scene_config: &SceneConfig) -> DrawList {
    let objects = stage
        .objects
        .iter()
        .map(|(object_id, object)| {
            let segments = ball_segments(
                &object.shape,
                &object.transform,
                &stage.camera,
                scene_config,
            );
            let (meshes, mesh_to_world) = match &object.shape {
                Shape::Molecule {
                    molecule,
                    representation,
                    ..
                } => (
                    drawn_meshes(&representation.draw(molecule), segments),
                    object.transform.object_to_world(),
                ),
                Shape::Axes3d(axes) => (
                    axes_3d_meshes(
                        axes,
                        object.transform.object_to_world(),
                        &stage.camera,
                        object.created.get(),
                    ),
                    Mat4::IDENTITY,
                ),
                Shape::Trail(trail) => (
                    trail_mesh(&trail.points, trail.stroke, &stage.camera)
                        .into_iter()
                        .collect(),
                    Mat4::IDENTITY,
                ),
                shape => shape.meshes(&object.transform, object.flat_to_world(), object.created),
            };
            StagedObject {
                source: Source::Object(object_id.clone()),
                transform: object.transform,
                mesh_to_world,
                opacity: object.opacity.get(),
                meshes,
                glows: object.shape.glows(segments),
            }
        })
        .chain(
            stage
                .reactions
                .iter()
                .enumerate()
                .filter(|(_, r)| r.phase == Phase::During)
                .map(|(k, r)| staged_reaction(k, r)),
        )
        .collect();
    DrawList {
        camera: stage.camera,
        objects,
    }
}

/// The blend of the reaction at index `k` of the timeline as one staged object: world-space
/// balls and sticks at opacity 1, with the shells of the glows it carries. `draw_list` stages it
/// only while the reaction plays, and `inspect` stages it at any scene time to measure its box.
pub fn staged_reaction(k: usize, reaction: &StagedReaction) -> StagedObject {
    let blend = &reaction.blend;
    let glows = reaction
        .glows
        .iter()
        .map(|carried| StagedGlow {
            meshes: glow_meshes(
                &blend.glow(carried),
                carried.glow,
                BALL_AND_STICK_GLOW,
                BALL_SEGMENTS,
            ),
        })
        .collect();
    StagedObject {
        source: Source::Reaction(k),
        transform: Transform::default(),
        mesh_to_world: Mat4::IDENTITY,
        opacity: 1.0,
        meshes: drawn_meshes(&blend.draw(), BALL_SEGMENTS),
        glows,
    }
}

impl Shape {
    /// The meshes and the matrix that places them. An object's scale, and its plane map, reshape
    /// a flat shape's path before its stroke is cut, not the stroke's width, so a stroke keeps
    /// its width in world units however the object is scaled, sheared or flattened. An arrow
    /// under a plane map is drawn again between its mapped ends, with the tip its own scale
    /// gives it, and it draws nothing when those ends meet.
    fn meshes(
        &self,
        transform: &Transform,
        to_world: FlatToWorld,
        created: UnitInterval,
    ) -> (Vec<Mesh>, Mat4) {
        let bake = Bake::of(to_world);
        let placed = to_world.meshes_to_world();
        match self {
            Shape::Flat { path, stroke, fill } => {
                let meshes = match (path.under_map(), bake) {
                    (UnderMap::FromEnds(ends), Bake::Mapped(matrix)) => Path::arrow(
                        matrix * ends.from,
                        matrix * ends.to,
                        ends.tip_length * own_tip_scale(transform),
                    )
                    .map_or_else(
                        |_| Vec::new(),
                        |arrow| {
                            flat_meshes(
                                &arrow,
                                Bake::Mapped(Mat2::IDENTITY),
                                *stroke,
                                *fill,
                                created.get(),
                            )
                        },
                    ),
                    _ => flat_meshes(path, bake, *stroke, *fill, created.get()),
                };
                (meshes, placed)
            }
            Shape::Glyphs { glyphs, size } => {
                (written_meshes(glyphs, *size, bake, created.get()), placed)
            }
            Shape::Table(table) => (table_meshes(table, bake, created.get()), placed),
            Shape::Mix(paths) => (
                paths
                    .iter()
                    .flat_map(|mixed| {
                        flat_meshes(&mixed.path, bake, mixed.stroke, mixed.fill, created.get())
                    })
                    .collect(),
                placed,
            ),
            Shape::Axes(axes) => (drawing_meshes(&axes.drawing, bake, created.get()), placed),
            Shape::NumberLine(line) => (drawing_meshes(&line.drawing, bake, created.get()), placed),
            Shape::BarPlot(bar_plot) => {
                let glows = bar_plot
                    .bars
                    .iter()
                    .filter(|bar| bar.glow.share.get() > 0.0)
                    .filter_map(|bar| Some((bar.glow_path()?, bar.glow)))
                    .flat_map(|(path, glow)| {
                        let fill = Fill {
                            color: glow.color,
                            opacity: UnitInterval::clamped(FLAT_GLOW_OPACITY * glow.share.get()),
                            rule: FillRule::EvenOdd,
                        };
                        path_meshes(&path, bake, None, 0.0, Some(fill), 1.0)
                    });
                let bars = bar_plot
                    .created_shares(created.get())
                    .filter_map(|(bar, share)| Some((bar, bar.path(share)?)))
                    .flat_map(|(bar, path)| {
                        let (fill, stroke) = bar_plot.paint(bar);
                        flat_meshes(&path, bake, stroke, Some(fill), 1.0)
                    });
                (merged(bars.chain(glows)), placed)
            }
            Shape::Axes3d(_) => {
                unreachable!("`draw_list` stages 3D axes, whose labels face its camera")
            }
            Shape::Trail(_) => unreachable!("`draw_list` stages a trail, which faces its camera"),
            Shape::Sphere { radius, material } if created.get() >= 1.0 => (
                vec![sphere(
                    radius.get(),
                    *material,
                    SPHERE_SEGMENTS,
                    SPHERE_RINGS,
                )],
                transform.object_to_world(),
            ),
            Shape::Sphere { radius, material } => (
                created_sphere(radius.get(), *material, created.get())
                    .into_iter()
                    .collect(),
                transform.object_to_world(),
            ),
            Shape::Solid {
                solid,
                material,
                edges,
            } => (
                solid_mesh(solid, *material, *edges, created.get())
                    .into_iter()
                    .collect(),
                transform.object_to_world(),
            ),
            Shape::Molecule { .. } => {
                unreachable!("`draw_list` cuts a molecule's balls as finely as its frame needs")
            }
            Shape::Pattern(pattern) => (
                merged(
                    copy_meshes(pattern, created.get())
                        .into_iter()
                        .flatten()
                        .chain(flat_copy_glows(pattern)),
                ),
                transform.object_to_world(),
            ),
        }
    }

    /// The glow shells of a molecule's lit groups, in group id order, their balls cut into
    /// `segments`, or of a pattern's lit solid copies, in index order.
    fn glows(&self, segments: u32) -> Vec<StagedGlow> {
        if let Shape::Pattern(pattern) = self {
            return solid_copy_glows(pattern);
        }
        let Shape::Molecule {
            molecule,
            representation,
            groups,
            glows,
        } = self
        else {
            return Vec::new();
        };
        glows
            .iter()
            .filter(|(_, glow)| glow.share.get() > 0.0)
            .filter_map(|(id, glow)| {
                groups.get(id).map(|group| StagedGlow {
                    meshes: glow_meshes(
                        &representation.glow(molecule, group, glow.color),
                        *glow,
                        representation.glow_look(),
                        segments,
                    ),
                })
            })
            .collect()
    }
}

/// A trail's points as a ribbon of its stroke's width in the world, square to the camera's
/// direction so it shows exactly its color, mitered at each point. `None` for fewer than two
/// points, or a trail that runs straight along the camera's direction.
fn trail_mesh(points: &[Vec3], stroke: Stroke, camera: &Camera) -> Option<Mesh> {
    let toward_camera = camera.direction();
    let sides: Vec<Vec3> = points
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).cross(toward_camera).normalize_or_zero())
        .collect();
    if points.len() < 2 || sides.iter().all(|side| *side == Vec3::ZERO) {
        return None;
    }
    let half = stroke.width.get() / 2.0;
    let mut builder = MeshBuilder::default();
    for (k, point) in points.iter().enumerate() {
        let (before, after) = (
            sides.get(k.wrapping_sub(1)).copied().unwrap_or(Vec3::ZERO),
            sides.get(k).copied().unwrap_or(Vec3::ZERO),
        );
        let own = if after == Vec3::ZERO { before } else { after };
        let side = (before + after).normalize_or_zero();
        let side = if side == Vec3::ZERO { own } else { side };
        let reach = half / side.dot(own).max(0.25);
        builder.push(*point + side * reach, toward_camera);
        builder.push(*point - side * reach, toward_camera);
    }
    for k in 0..points.len() as u32 - 1 {
        let (a, b, c, d) = (2 * k, 2 * k + 1, 2 * k + 2, 2 * k + 3);
        builder.triangles.push([a, b, c]);
        builder.triangles.push([c, b, d]);
    }
    Some(builder.finish(stroke.material(), Lighting::Camera))
}

/// The object placed as it is, drawing `meshes` instead, for measuring a part of it: a group's
/// shell, whatever its glow share, so an agent can lay out a caption before the highlight, or one
/// copy of a pattern.
pub fn with_meshes(object: &StagedObject, meshes: Vec<Mesh>) -> StagedObject {
    StagedObject {
        source: object.source.clone(),
        transform: object.transform,
        mesh_to_world: object.mesh_to_world,
        opacity: 1.0,
        meshes,
        glows: Vec::new(),
    }
}

/// A group's shell, whatever its glow share, for measuring.
pub fn shell_meshes(drawn: &Drawn, color: Color) -> Vec<Mesh> {
    meshes_by_color(
        drawn,
        |_| Material::solid(color),
        Lighting::Camera,
        BALL_SEGMENTS,
    )
}

/// The shell as a see-through film, as opaque as `look` says at share 1.
pub fn glow_meshes(drawn: &Drawn, glow: Glow, look: GlowLook, segments: u32) -> Vec<Mesh> {
    let film = |color| Material::Standard {
        color,
        roughness: crate::scene::UnitInterval::one(),
        metallic: crate::scene::UnitInterval::clamped(0.0),
        opacity: crate::scene::UnitInterval::clamped(look.opacity * glow.share.get()),
    };
    let lighting = match look.key_light {
        true => Lighting::KeyLight,
        false => Lighting::Camera,
    };
    meshes_by_color(drawn, film, lighting, segments)
}

/// How much an arrow's own scale lengthens the tip it is drawn again with under a plane map:
/// the scale for an even scale, so the identity map draws the arrow as it is drawn unmapped,
/// and the geometric mean of the x and y scale for an uneven one, so the tip covers the area
/// the two would stretch it to.
fn own_tip_scale(transform: &Transform) -> f32 {
    (transform.scale.x * transform.scale.y).abs().sqrt()
}

/// What a flat shape's path points are multiplied by before lyon cuts them.
#[derive(Clone, Copy, Debug)]
enum Bake {
    /// The object's x and y scale, one factor per coordinate, as before linear maps existed.
    Scaled(Vec2),
    /// The whole `matrix` of `FlatToWorld::Mapped`.
    Mapped(Mat2),
}

impl Bake {
    fn of(to_world: FlatToWorld) -> Bake {
        match to_world {
            FlatToWorld::Declared(declared) => Bake::Scaled(declared.scale.truncate()),
            FlatToWorld::Mapped { matrix, .. } => Bake::Mapped(matrix),
        }
    }

    /// How much the bake can stretch a length, for a flattening tolerance relative to size.
    fn stretch(self) -> f32 {
        match self {
            Bake::Scaled(scale) => scale.abs().max_element(),
            Bake::Mapped(matrix) => matrix.x_axis.length().max(matrix.y_axis.length()),
        }
    }

    /// The least tolerance lyon is asked to cut to. A path a plane map has squeezed below a
    /// millionth of a world unit draws nothing, so lyon never meets a near-zero matrix.
    fn least_tolerance(self) -> f32 {
        match self {
            Bake::Scaled(_) => 0.0,
            Bake::Mapped(_) => FLATTEN_TOLERANCE * 1e-6,
        }
    }
}

impl From<Vec2> for Bake {
    fn from(scale: Vec2) -> Bake {
        Bake::Scaled(scale)
    }
}

/// The largest distance, as a share of the path's size, between a curve and the straight pieces
/// it is cut into.
const FLATTEN_TOLERANCE: f32 = 1e-3;

/// What a created share shows of a stroke and of a fill, each from 0 to 1. A create draws the
/// stroke along the path over the first half of the created share and fades the fill in over the
/// second half. With only one of the two, that one takes the whole create. An absent one is 0.
fn created_shares(created: f32, has_stroke: bool, has_fill: bool) -> (f32, f32) {
    match (has_stroke, has_fill) {
        (true, true) => ((created * 2.0).min(1.0), (created * 2.0 - 1.0).max(0.0)),
        (true, false) => (created, 0.0),
        (false, true) => (0.0, created),
        (false, false) => (0.0, 0.0),
    }
}

/// The meshes a created share shows of a flat shape, by `created_shares`.
fn flat_meshes(
    path: &Path,
    bake: impl Into<Bake>,
    stroke: Option<Stroke>,
    fill: Option<Fill>,
    created: f32,
) -> Vec<Mesh> {
    let (stroke_share, fill_share) = created_shares(created, stroke.is_some(), fill.is_some());
    path_meshes(path, bake.into(), stroke, stroke_share, fill, fill_share)
}

/// The fill of the whole path at `fill_share` of its opacity, then the stroke of the first
/// `stroke_share` of the path's length, as an overlay. A fill or stroke whose share is 0 has no
/// mesh, so `inspect` measures only what the frame shows. The path is baked by `bake` first, after
/// a create's cut, so every point drawn is a point of the path carried by the bake. A path baked
/// to a point has no meshes. Every flat shape and every glyph reaches the GPU here.
fn path_meshes(
    path: &Path,
    bake: Bake,
    stroke: Option<Stroke>,
    stroke_share: f32,
    fill: Option<Fill>,
    fill_share: f32,
) -> Vec<Mesh> {
    let tolerance = FLATTEN_TOLERANCE * path.size() * bake.stretch();
    if tolerance <= bake.least_tolerance() {
        return Vec::new();
    }
    let fill = fill.filter(|_| fill_share > 0.0).and_then(|fill| {
        fill_mesh(
            path.elements(),
            bake,
            tolerance,
            fill.rule,
            fill.material(fill_share),
        )
    });
    let stroke = stroke.filter(|_| stroke_share > 0.0).and_then(|stroke| {
        let drawn = path.partial(stroke_share);
        let (elements, bake) = match stroke.dash {
            None => (Cow::Borrowed(drawn.elements()), bake),
            Some(dash) => (
                Cow::Owned(dashed(
                    drawn.elements(),
                    bake,
                    dash,
                    matches!(path.under_map(), UnderMap::FromEnds(_)),
                )),
                Bake::Scaled(Vec2::ONE),
            ),
        };
        stroke_mesh(
            &elements,
            bake,
            tolerance,
            stroke.width.get(),
            stroke.material(),
        )
    });
    fill.into_iter().chain(stroke).collect()
}

/// The meshes of axes or a number line at a created share: the rulings drawn along their length
/// over the first half, then the labels fading in over the second, as a create does a stroke and
/// a fill.
fn drawing_meshes(drawing: &Drawing, bake: Bake, created: f32) -> Vec<Mesh> {
    let (ruling_share, label_share) = created_shares(created, true, !drawing.labels.is_empty());
    let mut meshes = path_meshes(
        &drawing.rulings,
        bake,
        Some(drawing.stroke),
        ruling_share,
        None,
        0.0,
    );
    meshes.extend(
        drawing
            .labels
            .iter()
            .flat_map(|label| &label.glyphs)
            .flat_map(|glyph| {
                path_meshes(
                    &glyph.path,
                    bake,
                    None,
                    0.0,
                    Some(Fill::solid(glyph.color)),
                    label_share,
                )
            }),
    );
    meshes
}

/// The meshes of 3D axes at a created share, in world units: the axis lines and ticks drawn
/// along their length over the first half, as unlit sticks a stroke wide placed by
/// `object_to_world`, then the labels fading in over the second, each facing `camera`. A stroke
/// keeps its width in world units under the object's scale, as a flat shape's does.
fn axes_3d_meshes(
    axes: &Axes3d,
    object_to_world: Mat4,
    camera: &Camera,
    created: f32,
) -> Vec<Mesh> {
    let (ruling_share, label_share) = created_shares(created, true, !axes.labels.is_empty());
    let facing = Facing::new(object_to_world, camera);
    let rulings_in_world = axes.rulings_in_world(&facing);
    let mut left = ruling_share
        * rulings_in_world
            .iter()
            .map(|(from, to)| from.distance(*to))
            .sum::<f32>();
    let mut rulings = MeshBuilder::default();
    for (from, to) in rulings_in_world {
        let length = from.distance(to);
        if left <= 0.0 || length == 0.0 {
            break;
        }
        rulings.stick(
            from,
            from.lerp(to, (left / length).min(1.0)),
            axes.stroke.width.get() / 2.0,
            STICK_SIDES,
        );
        left -= length;
    }
    let mut meshes: Vec<Mesh> = (!rulings.triangles.is_empty())
        .then(|| rulings.finish(axes.stroke.material(), Lighting::Unlit))
        .into_iter()
        .collect();
    for label in &axes.labels {
        let to_world = label.to_world(&facing);
        let normal = camera.direction();
        for glyph in &label.glyphs {
            for mut mesh in path_meshes(
                &glyph.path,
                Bake::Scaled(Vec2::ONE),
                None,
                0.0,
                Some(Fill::solid(glyph.color)),
                label_share,
            ) {
                mesh.points.iter_mut().for_each(|point| {
                    *point = Point {
                        position: to_world
                            .transform_point3(Vec3::from(point.position))
                            .to_array(),
                        normal: normal.to_array(),
                    }
                });
                meshes.push(mesh);
            }
        }
    }
    meshes
}

/// How far a write has drawn each of `count` glyphs at a created share, or a create each of
/// `count` copies of a pattern: each one's own created share, in order. Each takes the same share
/// of the whole, and each starts a lag after the one before, so a few overlap at a time, as
/// manim's Write staggers glyphs. The lag is a fifth of one's share, or less when there are more
/// than 20, so that each still takes about a fifth of the whole rather than flashing by. A whole
/// created share draws every one whole: in f32 the last one's share can fall just short of 1,
/// which would leave a glyph's pen a few billionths of a unit wide, the one mesh of a glyph a
/// singular plane map has flattened.
pub fn paced_shares(count: usize, created: f32) -> impl Iterator<Item = f32> {
    let lag = (4.0 / count.max(1) as f32).min(0.2);
    let each = 1.0 / (1.0 + (count.max(1) - 1) as f32 * lag);
    (0..count).map(move |i| {
        if created >= 1.0 {
            1.0
        } else {
            ((created - i as f32 * lag * each) / each).clamp(0.0, 1.0)
        }
    })
}

/// Each copy's meshes at a created share of the pattern, in index order and in the pattern's own
/// space, with none for a copy whose own share is 0. The shape is cut into triangles at its own
/// size once for each distinct share, and each copy carries that cut by its transform, so a
/// copy's scale sizes a flat copy's stroke with the rest of it. A whole sphere is cut as an atom's
/// ball is, so a lattice of thousands stays cheap. A copy with its own colors is cut on its own,
/// in them, and a copy's own opacity makes each of its meshes as see-through.
pub fn copy_meshes(pattern: &Pattern, created: f32) -> Vec<Vec<Mesh>> {
    let mut cut: HashMap<u32, Vec<Mesh>> = HashMap::new();
    let edges = match *pattern.shape {
        Shape::Solid { edges, .. } => Some(edges),
        _ => None,
    };
    pattern
        .copies
        .iter()
        .zip(paced_shares(pattern.copies.len(), created))
        .map(|(copy, share)| {
            if share <= 0.0 {
                return Vec::new();
            }
            let own_cut;
            let (own, color_share) = if copy.own_colors.is_empty() {
                let own = cut
                    .entry(share.to_bits())
                    .or_insert_with(|| cut_copy(&pattern.shape, share));
                (&*own, copy.color_share)
            } else {
                own_cut = cut_copy(&pattern.copy_shape(copy), share);
                (&own_cut, 0.0)
            };
            let to_pattern = copy.transform.object_to_world();
            own.iter()
                .map(|mesh| {
                    let Material::Standard {
                        color,
                        roughness,
                        metallic,
                        opacity,
                    } = mesh.material;
                    let color = if color_share > 0.0 {
                        color.mix(copy.to_color, color_share)
                    } else {
                        color
                    };
                    let opacity = if copy.opacity < 1.0 {
                        UnitInterval::clamped(opacity.get() * copy.opacity)
                    } else {
                        opacity
                    };
                    let material = Material::Standard {
                        color,
                        roughness,
                        metallic,
                        opacity,
                    };
                    Mesh {
                        material,
                        edges: mesh.edges.and(edges).map(|shade| shade.of(color)),
                        ..placed_in_pattern(mesh, to_pattern)
                    }
                })
                .collect()
        })
        .collect()
}

/// A copy's shape cut into triangles at its own size and a created share.
fn cut_copy(shape: &Shape, share: f32) -> Vec<Mesh> {
    match shape {
        Shape::Sphere { radius, material } if share >= 1.0 => {
            vec![sphere(radius.get(), *material, BALL_SEGMENTS, BALL_RINGS)]
        }
        shape => {
            shape
                .meshes(
                    &Transform::default(),
                    FlatToWorld::Declared(Transform::default()),
                    UnitInterval::clamped(share),
                )
                .0
        }
    }
}

/// `mesh`, in a copy's own space, carried into the pattern's own space by `to_pattern`, the
/// copy's transform.
fn placed_in_pattern(mesh: &Mesh, to_pattern: Mat4) -> Mesh {
    let normal_to_pattern = crate::gpu::cofactor(Mat3::from_mat4(to_pattern));
    Mesh {
        points: mesh
            .points
            .iter()
            .map(|point| Point {
                position: to_pattern
                    .transform_point3(Vec3::from(point.position))
                    .to_array(),
                normal: (normal_to_pattern * Vec3::from(point.normal))
                    .normalize_or_zero()
                    .to_array(),
            })
            .collect(),
        triangles: mesh.triangles_placed_by(to_pattern).into_owned(),
        material: mesh.material,
        overlay: mesh.overlay,
        lighting: mesh.lighting,
        edges: mesh.edges,
    }
}

/// How far a flat copy's glow reaches past the copy's box on every side, as a share of the
/// box's longer side.
const FLAT_COPY_GLOW_SPREAD: f64 = 0.15;

/// How far a solid copy's glow shell reaches from the copy's center, as a share of the farthest
/// point of its shape.
const SOLID_COPY_GLOW_REACH: f32 = 1.25;

/// The glow of each lit copy of a pattern of a flat shape, in index order and in the pattern's own
/// space: a see-through fill of the copy's box, stroke included, and a little around it. Its
/// stroke is an overlay, which the glow lies behind, and a filled shape's glow leaves out the box
/// of its fill, so it does not tint it. A copy's own opacity fades its glow too.
fn flat_copy_glows(pattern: &Pattern) -> impl Iterator<Item = Mesh> + '_ {
    let frame = match &*pattern.shape {
        Shape::Flat { path, stroke, fill } => {
            let half = stroke.map_or(0.0, |s| f64::from(s.width.get()) / 2.0);
            let filled = path.bounding_box();
            let inner = filled.inflate(half, half);
            let spread = FLAT_COPY_GLOW_SPREAD * inner.width().max(inner.height());
            let hole = fill.map(|_| filled);
            let mut frame = kurbo::BezPath::new();
            for r in std::iter::once(inner.inflate(spread, spread)).chain(hole) {
                frame.move_to((r.x0, r.y0));
                frame.line_to((r.x1, r.y0));
                frame.line_to((r.x1, r.y1));
                frame.line_to((r.x0, r.y1));
                frame.close_path();
            }
            Path::from_bez(frame).ok()
        }
        _ => None,
    };
    pattern
        .copies
        .iter()
        .filter(|copy| copy.glow.share.get() * copy.opacity > 0.0)
        .filter_map(move |copy| Some((copy, frame.clone()?)))
        .flat_map(|(copy, frame)| {
            let fill = Fill {
                color: copy.glow.color,
                opacity: UnitInterval::clamped(
                    FLAT_GLOW_OPACITY * copy.glow.share.get() * copy.opacity,
                ),
                rule: FillRule::EvenOdd,
            };
            let to_pattern = copy.transform.object_to_world();
            path_meshes(&frame, Bake::Scaled(Vec2::ONE), None, 0.0, Some(fill), 1.0)
                .into_iter()
                .map(move |mesh| placed_in_pattern(&mesh, to_pattern))
        })
}

/// The glow shell of each lit copy of a pattern of a solid, in index order and in the pattern's
/// own space: a see-through sphere about the copy's center, `SOLID_COPY_GLOW_REACH` times as far
/// out as its shape reaches, as a group's glow is a shell about its atoms.
fn solid_copy_glows(pattern: &Pattern) -> Vec<StagedGlow> {
    if matches!(*pattern.shape, Shape::Flat { .. }) {
        return Vec::new();
    }
    let radius = pattern.shape.reach() * SOLID_COPY_GLOW_REACH;
    pattern
        .copies
        .iter()
        .filter(|copy| copy.glow.share.get() * copy.opacity > 0.0)
        .map(|copy| {
            let film = Material::Standard {
                color: copy.glow.color,
                roughness: UnitInterval::one(),
                metallic: UnitInterval::clamped(0.0),
                opacity: UnitInterval::clamped(
                    BALL_AND_STICK_GLOW.opacity * copy.glow.share.get() * copy.opacity,
                ),
            };
            let shell = Mesh {
                lighting: Lighting::Camera,
                ..sphere(radius, film, BALL_SEGMENTS, BALL_RINGS)
            };
            StagedGlow {
                meshes: vec![placed_in_pattern(&shell, copy.transform.object_to_world())],
            }
        })
        .collect()
}

/// The meshes with one material, overlay, lighting and edge color each, in the order each first
/// appears, so a pattern of one color is one or two draws.
fn merged(meshes: impl Iterator<Item = Mesh>) -> Vec<Mesh> {
    let key = |mesh: &Mesh| {
        let Material::Standard {
            color,
            roughness,
            metallic,
            opacity,
        } = mesh.material;
        (
            [color.r, color.g, color.b],
            [roughness.get(), metallic.get(), opacity.get()].map(f32::to_bits),
            mesh.overlay,
            mesh.lighting as u8,
            mesh.edges.map(|c| [c.r, c.g, c.b]),
        )
    };
    let mut merged: Vec<Mesh> = Vec::new();
    let mut by_key = HashMap::new();
    for mesh in meshes {
        match by_key.get(&key(&mesh)) {
            Some(&k) => {
                let into: &mut Mesh = &mut merged[k];
                let first = into.points.len() as u32;
                into.points.extend(mesh.points);
                into.triangles
                    .extend(mesh.triangles.iter().map(|t| t.map(|i| first + i)));
            }
            None => {
                by_key.insert(key(&mesh), merged.len());
                merged.push(mesh);
            }
        }
    }
    merged
}

/// Glyphs as a write draws them at a created share, each at its own paced share, in order.
fn written_meshes(glyphs: &[Glyph], size: Positive, bake: Bake, created: f32) -> Vec<Mesh> {
    glyphs
        .iter()
        .zip(paced_shares(glyphs.len(), created))
        .flat_map(|(glyph, written)| {
            glyph_meshes(
                &glyph.path,
                bake,
                Fill::solid(glyph.color),
                Stroke::pen(glyph.color, size),
                written,
            )
        })
        .collect()
}

/// A table at a write's created share: the glow of each lit cell, its rulings drawn along their
/// length, then its cells written in reading order. Its glyphs are overlays, as strokes are, so a
/// see-through glow, which the GPU blends after every opaque mesh, lies behind them and does not
/// tint them.
fn table_meshes(table: &Table, bake: Bake, created: f32) -> Vec<Mesh> {
    let (ruling_share, cell_share) = table.written_shares(created);
    let mut meshes: Vec<Mesh> = table
        .glows
        .iter()
        .filter(|(_, glow)| glow.share.get() > 0.0)
        .filter_map(|(address, glow)| Some((table.cell(*address)?, glow)))
        .flat_map(|(cell, glow)| {
            let r = cell.rectangle;
            let corners = [(r.x0, r.y0), (r.x1, r.y0), (r.x1, r.y1), (r.x0, r.y1)]
                .map(|(x, y)| Vec2::new(x as f32, y as f32));
            let fill = Fill {
                color: glow.color,
                opacity: UnitInterval::clamped(FLAT_GLOW_OPACITY * glow.share.get()),
                rule: FillRule::default(),
            };
            Path::polygon(&corners)
                .map(|path| path_meshes(&path, bake, None, 0.0, Some(fill), 1.0))
                .unwrap_or_default()
        })
        .collect();
    if let Some(rulings) = &table.rulings {
        meshes.extend(path_meshes(
            rulings,
            bake,
            Some(table.stroke),
            ruling_share,
            None,
            0.0,
        ));
    }
    meshes.extend(
        written_meshes(&table.glyphs, table.size, bake, cell_share)
            .into_iter()
            .map(|mesh| Mesh {
                overlay: true,
                ..mesh
            }),
    );
    meshes
}

/// The glyphs of one cell of a table, written whole, placed as the table is, for measuring.
pub fn cell_meshes(table: &Table, cell: &Cell, to_world: FlatToWorld) -> Vec<Mesh> {
    table.glyphs[cell.glyphs.clone()]
        .iter()
        .flat_map(|glyph| {
            path_meshes(
                &glyph.path,
                Bake::of(to_world),
                None,
                0.0,
                Some(Fill::solid(glyph.color)),
                1.0,
            )
        })
        .collect()
}

/// One glyph as a write draws it: its pen along its path over the first half of its share, then
/// its fill fading in over the second half while the pen thins to nothing, so a written glyph is
/// its fill alone. The same stroke and fill meshes a create makes of a flat shape.
fn glyph_meshes(
    glyph: &Path,
    bake: impl Into<Bake>,
    fill: Fill,
    pen: Stroke,
    written: f32,
) -> Vec<Mesh> {
    let (stroke_share, fill_share) = created_shares(written, true, true);
    let thinned = Positive::try_from(f64::from(pen.width.get() * (1.0 - fill_share)))
        .ok()
        .map(|width| Stroke { width, ..pen });
    path_meshes(
        glyph,
        bake.into(),
        thinned,
        stroke_share,
        Some(fill),
        fill_share,
    )
}

/// `None` when lyon cannot cut the path, which takes more triangles than a `u32` indexes, or
/// when the path encloses no area. Such a shape draws nothing rather than stopping the render.
fn fill_mesh(
    path: &[PathEl],
    bake: Bake,
    tolerance: f32,
    rule: FillRule,
    material: Material,
) -> Option<Mesh> {
    let rule = match rule {
        FillRule::NonZero => lyon::tessellation::FillRule::NonZero,
        FillRule::EvenOdd => lyon::tessellation::FillRule::EvenOdd,
    };
    let mut buffers: VertexBuffers<Point, u32> = VertexBuffers::new();
    FillTessellator::new()
        .tessellate_path(
            &lyon_path(path, bake),
            &FillOptions::tolerance(tolerance).with_fill_rule(rule),
            &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| flat_point(v.position())),
        )
        .ok()?;
    mesh(buffers, material, false)
}

/// How far a mitered corner may reach past its corner, in stroke widths as lyon counts it,
/// before it is beveled: 1.1 widths, so corners sharper than about 54 degrees bevel. A 60
/// degree corner keeps its point, and a thin triangle's 29 degree corner grows no spike two
/// widths long.
const MITER_LIMIT: f32 = 1.1;

/// A ribbon `width` wide centered on the path, with mitered corners, beveled past
/// `MITER_LIMIT`, and square-cut ends.
fn stroke_mesh(
    path: &[PathEl],
    bake: Bake,
    tolerance: f32,
    width: f32,
    material: Material,
) -> Option<Mesh> {
    let mut buffers: VertexBuffers<Point, u32> = VertexBuffers::new();
    StrokeTessellator::new()
        .tessellate_path(
            &lyon_path(path, bake),
            &StrokeOptions::tolerance(tolerance)
                .with_line_width(width)
                .with_line_join(LineJoin::Miter)
                .with_miter_limit(MITER_LIMIT)
                .with_line_cap(LineCap::Butt),
            &mut BuffersBuilder::new(&mut buffers, |v: StrokeVertex| flat_point(v.position())),
        )
        .ok()?;
    mesh(buffers, material, true)
}

/// The dashes `dash` cuts `path` into, baked by `bake` first, so dashes are measured in world
/// units as the stroke's width is. Each part of the path starts with a dash, so a dashed line
/// keeps its dashes where they are as it grows from its start. An arrow's tip, every part after
/// its shaft, is kept whole.
fn dashed(path: &[PathEl], bake: Bake, dash: DashPattern, arrow: bool) -> Vec<PathEl> {
    let affine = match bake {
        Bake::Scaled(scale) => kurbo::Affine::scale_non_uniform(scale.x.into(), scale.y.into()),
        Bake::Mapped(m) => kurbo::Affine::new(
            [m.x_axis.x, m.x_axis.y, m.y_axis.x, m.y_axis.y, 0.0, 0.0].map(f64::from),
        ),
    };
    let baked: Vec<PathEl> = path.iter().map(|&element| affine * element).collect();
    let tip = match arrow {
        true => baked
            .iter()
            .skip(1)
            .position(|element| matches!(element, PathEl::MoveTo(_)))
            .map_or(baked.len(), |k| k + 1),
        false => baked.len(),
    };
    let (shaft, tip) = baked.split_at(tip);
    let lengths = [dash.0.get(), dash.1.get()].map(f64::from);
    kurbo::dash(shaft.iter().copied(), 0.0, &lengths)
        .chain(tip.iter().copied())
        .collect()
}

/// lyon's path from ours, baked by `bake`. lyon wants every part ended before the next begins,
/// and a part that our path leaves open, as a create's cut does, stays open here, so it gets
/// caps and no fill.
fn lyon_path(path: &[PathEl], bake: Bake) -> lyon::path::Path {
    let pt = |p: kurbo::Point| {
        let (x, y) = (p.x as f32, p.y as f32);
        match bake {
            Bake::Scaled(scale) => point(x * scale.x, y * scale.y),
            Bake::Mapped(m) => point(
                m.x_axis.x * x + m.y_axis.x * y,
                m.x_axis.y * x + m.y_axis.y * y,
            ),
        }
    };
    let mut builder = lyon::path::Path::builder();
    let mut open = false;
    for element in path {
        match *element {
            PathEl::MoveTo(p) => {
                if open {
                    builder.end(false);
                }
                builder.begin(pt(p));
                open = true;
            }
            PathEl::LineTo(p) => {
                builder.line_to(pt(p));
            }
            PathEl::QuadTo(c, p) => {
                builder.quadratic_bezier_to(pt(c), pt(p));
            }
            PathEl::CurveTo(c1, c2, p) => {
                builder.cubic_bezier_to(pt(c1), pt(c2), pt(p));
            }
            PathEl::ClosePath => {
                builder.end(true);
                open = false;
            }
        }
    }
    if open {
        builder.end(false);
    }
    builder.build()
}

fn flat_point(position: lyon::math::Point) -> Point {
    Point {
        position: [position.x, position.y, 0.0],
        normal: [0.0, 0.0, 1.0],
    }
}

/// `None` for no triangles: the start of a create or write cuts slivers of path whose fill has
/// no area, and the GPU cannot draw an empty buffer. `None` too for a point that is not finite,
/// which a join of a path a singular plane map has folded could leave: such a mesh is dropped
/// rather than drawn or measured with NaN.
fn mesh(buffers: VertexBuffers<Point, u32>, material: Material, overlay: bool) -> Option<Mesh> {
    let triangles: Vec<[u32; 3]> = buffers
        .indices
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    let finite = buffers
        .vertices
        .iter()
        .all(|p| p.position.iter().all(|c| c.is_finite()));
    (finite && !triangles.is_empty()).then_some(Mesh {
        points: buffers.vertices,
        triangles,
        material,
        overlay,
        lighting: Lighting::Camera,
        edges: None,
    })
}

const SPHERE_SEGMENTS: u32 = 64;
const SPHERE_RINGS: u32 = 32;

fn sphere(radius: f32, material: Material, segments: u32, rings: u32) -> Mesh {
    let mut builder = MeshBuilder::default();
    builder.sphere(Vec3::ZERO, radius, &UnitSphere::new(segments, rings));
    builder.finish(material, Lighting::KeyLight)
}

/// A sphere a create has turned `created` of the way round: the revolution of a half disc, its
/// cut faces showing. `None` before it begins.
fn created_sphere(radius: f32, material: Material, created: f32) -> Option<Mesh> {
    if created <= 0.0 {
        return None;
    }
    let half_disc = Region {
        path: Path::arc(radius, -90.0, 180.0).expect("a radius more than 0"),
        rule: FillRule::NonZero,
    };
    let mut mesh = revolution(&half_disc, TAU * created, material)?;
    mesh.edges = None;
    Some(mesh)
}

/// How many pieces a full revolution is cut into around its axis. Neighbors turn 5.6 degrees,
/// under `CREASE_DEGREES`, so a revolution is smooth around and shows no edges between them.
const REVOLUTION_PIECES: f32 = 64.0;

/// Where two faces of a solid that turn more than this many degrees meet, their normals are not
/// shared, so the corner shows sharp, and `edge_mesh` draws an edge along it.
const CREASE_DEGREES: f32 = 30.0;

/// The mesh of a solid that a create has run `created` of its operation: a prism risen that share
/// of its height, a revolution turned that share of its angle, a tube drawn that share of its
/// curve's length, or a surface risen that share of its heights. `None` before it begins.
fn solid_mesh(solid: &Solid, material: Material, edges: EdgeShade, created: f32) -> Option<Mesh> {
    if created <= 0.0 {
        return None;
    }
    let mut mesh = match solid {
        Solid::Prism { base, height } => prism(base, height.get() * created, material),
        Solid::Revolution { profile, angle } => {
            revolution(profile, angle.get().to_radians() * created, material)
        }
        Solid::Tube { curve, radius, cut } => tube(curve, radius.get(), *cut, created, material),
        Solid::Surface { surface } => Some(sheet(surface, created, material)),
    }?;
    let Material::Standard { color, .. } = material;
    mesh.edges = Some(edges.of(color));
    Some(mesh)
}

/// The base pushed up by `height`: the base's face at the bottom and the top, and a wall along
/// each piece of its outline. A base point `(x, y)` lands at `(x, 0, -y)`.
fn prism(base: &Region, height: f32, material: Material) -> Option<Mesh> {
    let at = |p: Vec2, y: f32| Vec3::new(p.x, y, -p.y);
    let normal = |n: Vec2| Vec3::new(n.x, 0.0, -n.y);
    let straight_parts = straight_parts(&base.path);
    let (face_points, face_triangles) = region_triangles(&straight_parts, base.rule)?;
    let mut builder = MeshBuilder::default();
    for (y, facing) in [(0.0, Vec3::NEG_Y), (height, Vec3::Y)] {
        let first = builder.points.len() as u32;
        face_points
            .iter()
            .for_each(|&p| _ = builder.push(at(p, y), facing));
        builder
            .triangles
            .extend(face_triangles.iter().map(|t| t.map(|i| first + i)));
    }
    for wall in walls(base, &straight_parts) {
        let corners = [
            (wall.from, wall.normals[0], 0.0),
            (wall.to, wall.normals[1], 0.0),
            (wall.to, wall.normals[1], height),
            (wall.from, wall.normals[0], height),
        ];
        let [a, b, c, d] = corners.map(|(p, n, y)| builder.push(at(p, y), normal(n)));
        builder.triangles.extend([[a, b, c], [a, c, d]]);
    }
    Some(builder.finish(material, Lighting::KeyLight))
}

/// The profile turned about the y axis through `turned` radians, from +x toward -z, closed by
/// the profile's face at both cut ends when it turns less than a full circle.
fn revolution(profile: &Region, turned: f32, material: Material) -> Option<Mesh> {
    let pieces = (REVOLUTION_PIECES * turned / TAU).ceil().max(1.0) as u32;
    // Within half a piece of a whole turn is whole, so the two cut faces never nearly meet.
    let whole = turned >= TAU * (1.0 - 0.5 / REVOLUTION_PIECES);
    // The last angle is `turned` itself, so the wall's end and the cut face meet bit for bit, or 0
    // for a whole revolution, so its seam welds and shows no edge.
    let turns: Vec<(f32, f32)> = (0..=pieces)
        .map(|k| match k == pieces {
            true if whole => 0.0,
            true => turned,
            false => turned * k as f32 / pieces as f32,
        })
        .map(f32::sin_cos)
        .collect();
    let at =
        |p: Vec2, (sin, cos): (f32, f32)| Vec3::new(p.x.max(0.0) * cos, p.y, -p.x.max(0.0) * sin);
    let normal = |n: Vec2, (sin, cos): (f32, f32)| Vec3::new(n.x * cos, n.y, -n.x * sin);
    let straight_parts = straight_parts(&profile.path);
    let mut builder = MeshBuilder::default();
    for wall in walls(profile, &straight_parts)
        .into_iter()
        .filter(|wall| wall.from.x > 0.0 || wall.to.x > 0.0)
    {
        let first = builder.points.len() as u32;
        for &turn in &turns {
            builder.push(at(wall.from, turn), normal(wall.normals[0], turn));
            builder.push(at(wall.to, turn), normal(wall.normals[1], turn));
        }
        for k in 0..pieces {
            let a = first + 2 * k;
            builder
                .triangles
                .extend([[a, a + 1, a + 2], [a + 2, a + 1, a + 3]]);
        }
    }
    if !whole {
        let (face_points, face_triangles) = region_triangles(&straight_parts, profile.rule)?;
        let (first_turn, last_turn) = (turns[0], turns[pieces as usize]);
        for (turn, facing) in [
            (first_turn, Vec3::Z),
            (last_turn, Vec3::new(-last_turn.0, 0.0, -last_turn.1)),
        ] {
            let first = builder.points.len() as u32;
            face_points
                .iter()
                .for_each(|&p| _ = builder.push(at(p, turn), facing));
            builder
                .triangles
                .extend(face_triangles.iter().map(|t| t.map(|i| first + i)));
        }
    }
    (!builder.triangles.is_empty()).then(|| builder.finish(material, Lighting::KeyLight))
}

/// The sides of a tube. Neighbors turn 15 degrees, under `CREASE_DEGREES`, so a tube is smooth
/// around and shows no edges along it but its outline.
const TUBE_SIDES: u32 = 24;

/// A circle of `radius` carried along `created` of the curve's length, square to it on each of its
/// frames, closed by a disc at each end. A closed curve drawn whole has no ends: its last circle is
/// its first. On 3D axes the tube is cut where its curve leaves `cut`, the axes' box, into pieces
/// each closed by a disc at each end.
fn tube(
    curve: &SpaceCurve,
    radius: f32,
    cut: Option<[Vec3; 2]>,
    created: f32,
    material: Material,
) -> Option<Mesh> {
    let frames = curve.frames_up_to(curve.length() * created);
    let mut builder = MeshBuilder::default();
    match cut {
        None => tube_piece(
            &mut builder,
            &frames,
            radius,
            curve.is_closed() && created >= 1.0,
        ),
        Some(cut) => pieces_inside(&frames, cut)
            .iter()
            .for_each(|piece| tube_piece(&mut builder, piece, radius, false)),
    }
    (!builder.triangles.is_empty()).then(|| builder.finish(material, Lighting::KeyLight))
}

/// The runs of `frames` that lie inside the box `[low, high]`, each starting and ending where the
/// curve crosses the box's side, or at an end of `frames` inside it.
fn pieces_inside(frames: &[CurveFrame], [low, high]: [Vec3; 2]) -> Vec<Vec<CurveFrame>> {
    let between = |a: &CurveFrame, b: &CurveFrame, share: f32| {
        let tangent = a.tangent.lerp(b.tangent, share).normalize_or(a.tangent);
        CurveFrame {
            point: a.point.lerp(b.point, share),
            tangent,
            normal: a
                .normal
                .lerp(b.normal, share)
                .reject_from_normalized(tangent)
                .normalize_or(a.normal),
        }
    };
    let mut pieces: Vec<Vec<CurveFrame>> = Vec::new();
    let mut open = false;
    for pair in frames.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        // The shares of the step from a to b that lie inside the box, by its six sides.
        let step = b.point - a.point;
        let (mut enter, mut leave) = (0.0f32, 1.0f32);
        for k in 0..3 {
            if step[k] == 0.0 {
                if a.point[k] < low[k] || a.point[k] > high[k] {
                    (enter, leave) = (1.0, 0.0);
                }
                continue;
            }
            let (t0, t1) = (
                (low[k] - a.point[k]) / step[k],
                (high[k] - a.point[k]) / step[k],
            );
            enter = enter.max(t0.min(t1));
            leave = leave.min(t0.max(t1));
        }
        if enter > leave {
            open = false;
            continue;
        }
        if !open || enter > 0.0 {
            pieces.push(vec![between(a, b, enter)]);
        }
        let piece = pieces.last_mut().expect("a piece is open");
        piece.push(if leave < 1.0 {
            between(a, b, leave)
        } else {
            *b
        });
        open = leave >= 1.0;
    }
    pieces.retain(|piece| {
        piece.len() >= 2 && piece[0].point.distance(piece[piece.len() - 1].point) > 0.0
    });
    pieces
}

/// A circle of `radius` along `frames`, closed by a disc at each end unless `closed`, when the
/// last frame is the first.
fn tube_piece(builder: &mut MeshBuilder, frames: &[CurveFrame], radius: f32, closed: bool) {
    let sections: Vec<CrossSection> = frames
        .iter()
        .map(|frame| CrossSection {
            point: frame.point,
            tangent: frame.tangent,
            across: frame.normal,
            half_width: radius,
            half_thickness: radius,
        })
        .collect();
    sweep(builder, &sections, closed);
}

/// The ellipses of `sections` joined in order into a tube, closed by a flat face at each end
/// unless `closed`, when the last section is the first. A step, two sections in a row at one
/// point and of different sizes, is joined by a flat ring that faces back along the tube where it
/// widens and on along it where it narrows, as the shoulder of a cartoon's strand arrow does.
fn sweep(builder: &mut MeshBuilder, sections: &[CrossSection], closed: bool) {
    // The angles of the sides, computed once, so the rings' points are where the ends' are.
    let around: Vec<(f32, f32)> = (0..TUBE_SIDES)
        .map(|side| (TAU * side as f32 / TUBE_SIDES as f32).sin_cos())
        .collect();
    // Written so that a circle, whose halves are equal and whose ratios are exactly 1, gets the
    // same bits a circle always got.
    let ring = |section: &CrossSection, (sin, cos): (f32, f32)| {
        let (width, thickness) = (section.half_width, section.half_thickness);
        let side = section.tangent.cross(section.across);
        let offset = (section.across * cos + side * (sin * (thickness / width))) * width;
        let normal = section.across * cos + side * (sin * (width / thickness));
        let normal = if width == thickness {
            normal
        } else {
            normal.normalize()
        };
        (section.point + offset, normal)
    };
    let is_step = |a: &CrossSection, b: &CrossSection| {
        a.point == b.point && (a.half_width != b.half_width || a.half_thickness != b.half_thickness)
    };
    let mut pieces: Vec<&[CrossSection]> = Vec::new();
    let mut start = 0;
    for k in 1..sections.len() {
        if is_step(&sections[k - 1], &sections[k]) {
            pieces.push(&sections[start..k]);
            start = k;
        }
    }
    pieces.push(&sections[start..]);
    let closed = closed && pieces.len() == 1;
    let strip = |builder: &mut MeshBuilder, first_ring: u32, rings: usize, joins: usize| {
        let at = |ring: usize, side: u32| {
            first_ring + (ring as u32 % rings as u32) * TUBE_SIDES + side % TUBE_SIDES
        };
        for ring in 0..joins {
            for side in 0..TUBE_SIDES {
                let (a, b, c, d) = (
                    at(ring, side),
                    at(ring, side + 1),
                    at(ring + 1, side),
                    at(ring + 1, side + 1),
                );
                builder.triangles.extend([[a, b, c], [c, b, d]]);
            }
        }
    };
    for piece in &pieces {
        let rings = if closed { piece.len() - 1 } else { piece.len() };
        let first_ring = builder.points.len() as u32;
        for section in &piece[..rings] {
            for &angle in &around {
                let (point, normal) = ring(section, angle);
                builder.push(point, normal);
            }
        }
        strip(builder, first_ring, rings, piece.len() - 1);
    }
    for pair in pieces.windows(2) {
        let (from, to) = (pair[0][pair[0].len() - 1], pair[1][0]);
        let widens =
            to.half_width.max(to.half_thickness) > from.half_width.max(from.half_thickness);
        let facing = if widens { -to.tangent } else { to.tangent };
        let first_ring = builder.points.len() as u32;
        for section in [from, to] {
            for &angle in &around {
                builder.push(ring(&section, angle).0, facing);
            }
        }
        strip(builder, first_ring, 2, 1);
    }
    if !closed {
        let (first, last) = (sections[0], sections[sections.len() - 1]);
        for (section, facing) in [(first, -first.tangent), (last, last.tangent)] {
            let center = builder.push(section.point, facing);
            let first = builder.points.len() as u32;
            around
                .iter()
                .for_each(|&angle| _ = builder.push(ring(&section, angle).0, facing));
            builder.triangles.extend(
                (0..TUBE_SIDES).map(|side| [center, first + side, first + (side + 1) % TUBE_SIDES]),
            );
        }
    }
}

/// The surface risen `rise` of the way to its heights, two triangles a cell, counter-clockwise seen
/// from +y. Its rows run from the far end of its y range to the near, so they run along +z. On 3D
/// axes it is cut where it leaves their z range.
fn sheet(surface: &Surface, rise: f32, material: Material) -> Mesh {
    let (nx, ny) = surface.counts();
    let mut builder = MeshBuilder::default();
    for j in (0..ny).rev() {
        for i in 0..nx {
            let (point, normal) = surface.point(i, j, rise);
            builder.push(point, normal);
        }
    }
    let at = |i: usize, j: usize| (j * nx + i) as u32;
    for j in 0..ny - 1 {
        for i in 0..nx - 1 {
            let (a, b, c, d) = (at(i, j), at(i + 1, j), at(i, j + 1), at(i + 1, j + 1));
            builder.triangles.extend([[a, c, b], [b, c, d]]);
        }
    }
    if let Some((low, high)) = surface.cut() {
        cut_to_band(&mut builder, low, high);
    }
    builder.finish(material, Lighting::Sheet)
}

/// Cuts every triangle to the part of it whose y lies from `low` to `high`, keeping the order its
/// corners turn in, and drops the points no triangle keeps. A cut on a side two triangles share is
/// one point, found from the side's ends in one order, so the cut sheet has no cracks and its
/// edges run along the cut.
fn cut_to_band(builder: &mut MeshBuilder, low: f32, high: f32) {
    let points = std::mem::take(&mut builder.points);
    let y = |i: u32| points[i as usize].position[1];
    let inside = |i: u32| (low..=high).contains(&y(i));
    let mut kept: Vec<Point> = Vec::new();
    let mut index: HashMap<(u32, u32, u8), u32> = HashMap::new();
    let mut keep = |key: (u32, u32, u8), point: &dyn Fn() -> Point| {
        *index.entry(key).or_insert_with(|| {
            kept.push(point());
            kept.len() as u32 - 1
        })
    };
    let mut triangles = Vec::new();
    for triangle in std::mem::take(&mut builder.triangles) {
        let mut corners: Vec<u32> = Vec::new();
        for (from, to) in [
            (triangle[0], triangle[1]),
            (triangle[1], triangle[2]),
            (triangle[2], triangle[0]),
        ] {
            if inside(from) {
                corners.push(keep((from, from, 0), &|| points[from as usize]));
            }
            let (a, b) = (from.min(to), from.max(to));
            let mut cuts: Vec<(f32, u32)> = [(low, 1u8), (high, 2u8)]
                .into_iter()
                .filter(|&(plane, _)| (y(a) - plane) * (y(b) - plane) < 0.0)
                .map(|(plane, which)| {
                    let share = (plane - y(a)) / (y(b) - y(a));
                    let point = || {
                        let (p, q) = (points[a as usize], points[b as usize]);
                        let position = Vec3::from(p.position).lerp(Vec3::from(q.position), share);
                        let normal = Vec3::from(p.normal)
                            .lerp(Vec3::from(q.normal), share)
                            .normalize_or_zero();
                        Point {
                            position: [position.x, plane, position.z],
                            normal: normal.to_array(),
                        }
                    };
                    (
                        if from == a { share } else { 1.0 - share },
                        keep((a, b, which), &point),
                    )
                })
                .collect();
            cuts.sort_by(|p, q| p.0.total_cmp(&q.0));
            corners.extend(cuts.into_iter().map(|(_, k)| k));
        }
        triangles.extend(
            (1..corners.len().saturating_sub(1)).map(|k| [corners[0], corners[k], corners[k + 1]]),
        );
    }
    builder.points = kept;
    builder.triangles = triangles;
}

/// A region's outline cut into straight pieces, part by part. Each part is a ring of points: the
/// piece from its last point back to its first closes it, as a fill closes it.
fn straight_parts(path: &Path) -> Vec<Vec<Vec2>> {
    let mut parts: Vec<Vec<Vec2>> = Vec::new();
    let point = |p: kurbo::Point| Vec2::new(p.x as f32, p.y as f32);
    kurbo::flatten(
        path.elements().iter().copied(),
        f64::from(FLATTEN_TOLERANCE * path.size()),
        |element| match element {
            PathEl::MoveTo(p) => parts.push(vec![point(p)]),
            PathEl::LineTo(p) => parts
                .last_mut()
                .expect("a part starts with a move")
                .push(point(p)),
            _ => {}
        },
    );
    for part in &mut parts {
        part.dedup();
        if part.len() > 1 && part.first() == part.last() {
            part.pop();
        }
    }
    parts.retain(|part| part.len() > 1);
    split_at_crossings(parts)
}

/// The parts with a point added wherever two pieces cross, so each piece lies wholly inside the
/// region or wholly on its outline, and `walls` can judge it at its middle. Two squares that
/// overlap under `non-zero` then have walls all along their joint outline.
fn split_at_crossings(parts: Vec<Vec<Vec2>>) -> Vec<Vec<Vec2>> {
    let pieces: Vec<(Vec2, Vec2)> = parts
        .iter()
        .flat_map(|part| (0..part.len()).map(move |i| (part[i], part[(i + 1) % part.len()])))
        .collect();
    parts
        .iter()
        .map(|part| {
            let mut split = Vec::with_capacity(part.len());
            for i in 0..part.len() {
                let (from, to) = (part[i], part[(i + 1) % part.len()]);
                let mut crossings: Vec<f32> = pieces
                    .iter()
                    .filter_map(|&(a, b)| crossing(from, to, a, b))
                    .collect();
                crossings.sort_by(f32::total_cmp);
                split.push(from);
                split.extend(crossings.into_iter().map(|t| from + (to - from) * t));
            }
            split.dedup();
            split
        })
        .collect()
}

/// How far along the piece from `from` to `to` the piece from `a` to `b` crosses it, strictly
/// inside both, or `None` where they do not cross or only touch.
fn crossing(from: Vec2, to: Vec2, a: Vec2, b: Vec2) -> Option<f32> {
    let (along, other) = (to - from, b - a);
    let denominator = along.perp_dot(other);
    if denominator.abs() <= f32::EPSILON * along.length() * other.length() {
        return None;
    }
    let t = (a - from).perp_dot(other) / denominator;
    let u = (a - from).perp_dot(along) / denominator;
    let inside = 1e-5..1.0 - 1e-5;
    (inside.contains(&t) && inside.contains(&u)).then_some(t)
}

/// One straight piece of a region's outline, with the normal out of the region at each end.
/// Where the next piece turns less than `CREASE_DEGREES`, the two share the normal halfway
/// between theirs, so a circle's wall is smooth and a square's corners are sharp.
struct Wall {
    from: Vec2,
    to: Vec2,
    normals: [Vec2; 2],
}

/// The walls of a region's outline. A piece with the region on both sides, or on neither, as
/// where two parts overlap under the non-zero rule, has no wall.
fn walls(region: &Region, straight_parts: &[Vec<Vec2>]) -> Vec<Wall> {
    let nudge = region.path.size() * 1e-4;
    let contains = |point: Vec2| {
        let winding = winding(straight_parts, point);
        match region.rule {
            FillRule::NonZero => winding != 0,
            FillRule::EvenOdd => winding % 2 != 0,
        }
    };
    let crease = CREASE_DEGREES.to_radians().cos();
    let mut walls = Vec::new();
    for part in straight_parts {
        let n = part.len();
        let outward: Vec<Option<Vec2>> = (0..n)
            .map(|i| {
                let (from, to) = (part[i], part[(i + 1) % n]);
                let (middle, left) = ((from + to) / 2.0, (to - from).perp().normalize_or_zero());
                match (
                    contains(middle + left * nudge),
                    contains(middle - left * nudge),
                ) {
                    (false, true) => Some(left),
                    (true, false) => Some(-left),
                    _ => None,
                }
            })
            .collect();
        for i in 0..n {
            let Some(own) = outward[i] else { continue };
            let shared = |other: Option<Vec2>| {
                other
                    .filter(|o| o.dot(own) > crease)
                    .map_or(own, |o| (o + own).normalize())
            };
            walls.push(Wall {
                from: part[i],
                to: part[(i + 1) % n],
                normals: [
                    shared(outward[(i + n - 1) % n]),
                    shared(outward[(i + 1) % n]),
                ],
            });
        }
    }
    walls
}

/// Whether a region has any inside to make a solid of, as a polygon whose points lie on one line
/// does not.
pub fn has_inside(region: &Region) -> bool {
    region_triangles(&straight_parts(&region.path), region.rule).is_some()
}

/// How many times the straight_parts wind around `point`, counterclockwise positive. They are tested
/// rather than the region's curves, which bulge past the straight pieces a wall is built on.
fn winding(straight_parts: &[Vec<Vec2>], point: Vec2) -> i32 {
    let mut winding = 0;
    for part in straight_parts {
        for (i, &from) in part.iter().enumerate() {
            let to = part[(i + 1) % part.len()];
            let left = (to - from).perp_dot(point - from);
            if from.y <= point.y && to.y > point.y && left > 0.0 {
                winding += 1;
            } else if from.y > point.y && to.y <= point.y && left < 0.0 {
                winding -= 1;
            }
        }
    }
    winding
}

/// The region's face, cut by lyon from the same straight pieces its walls are made of, so the
/// face's rim and the walls' ends share their points.
fn region_triangles(
    straight_parts: &[Vec<Vec2>],
    rule: FillRule,
) -> Option<(Vec<Vec2>, Vec<[u32; 3]>)> {
    let mut builder = lyon::path::Path::builder();
    for part in straight_parts {
        builder.begin(point(part[0].x, part[0].y));
        part[1..]
            .iter()
            .for_each(|p| _ = builder.line_to(point(p.x, p.y)));
        builder.end(true);
    }
    let rule = match rule {
        FillRule::NonZero => lyon::tessellation::FillRule::NonZero,
        FillRule::EvenOdd => lyon::tessellation::FillRule::EvenOdd,
    };
    let mut buffers: VertexBuffers<Vec2, u32> = VertexBuffers::new();
    FillTessellator::new()
        .tessellate_path(
            &builder.build(),
            &FillOptions::default().with_fill_rule(rule),
            &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| {
                Vec2::new(v.position().x, v.position().y)
            }),
        )
        .ok()?;
    let triangles: Vec<[u32; 3]> = buffers
        .indices
        .chunks_exact(3)
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    (!triangles.is_empty()).then_some((buffers.vertices, triangles))
}

/// An edge is this share of the frame's height wide, 3 pixels at 1080p, whatever its distance.
const EDGE_WIDTH: f32 = 3.0 / 1080.0;

/// How far an edge lies toward the camera from the faces it runs along, as a share of the frame's
/// height, so the depth test draws it over them and still hides it behind other objects.
const EDGE_LIFT: f32 = 2.0 / 1080.0;

/// The edges of a mesh that has them, as the camera sees it placed by `mesh_to_world`: ribbons
/// that face the camera, along each of `edge_lines`.
pub fn edge_mesh(
    mesh: &Mesh,
    mesh_to_world: Mat4,
    camera: &Camera,
    scene_config: &SceneConfig,
) -> Option<Mesh> {
    let color = mesh.edges?;
    let height = scene_config.height.get() as f32;
    let forward = -camera.direction();
    let depth = |p: Vec3| (p - camera.position).dot(forward);
    let mut builder = MeshBuilder::default();
    for (from, to) in edge_lines(mesh, mesh_to_world, camera.position) {
        let Some((from, to)) = in_front(from, to, depth) else {
            continue;
        };
        // A ribbon's width is set at its middle, so a line from near the camera to far from it
        // is cut into pieces whose ends differ in depth by at most a quarter.
        let (near, far) = (depth(from).min(depth(to)), depth(from).max(depth(to)));
        let pieces = ((far / near).ln() / 1.25f32.ln()).ceil().clamp(1.0, 32.0) as u32;
        for k in 0..pieces {
            let (from, to) = (
                from.lerp(to, k as f32 / pieces as f32),
                from.lerp(to, (k + 1) as f32 / pieces as f32),
            );
            let middle = (from + to) / 2.0;
            let per_pixel = 1.0 / camera.pixels_per_unit(middle, scene_config);
            let (along, to_eye) = (
                (to - from).normalize_or_zero(),
                (camera.position - middle).normalize(),
            );
            let width = per_pixel * EDGE_WIDTH * height;
            let side = along.cross(to_eye).normalize_or_zero() * width / 2.0;
            let lift = to_eye * per_pixel * EDGE_LIFT * height;
            let (from, to) = (
                from - along * width / 2.0 + lift,
                to + along * width / 2.0 + lift,
            );
            let [p, q, r, s] = [from - side, to - side, to + side, from + side]
                .map(|corner| builder.push(corner, to_eye));
            builder.triangles.extend([[p, q, r], [p, r, s]]);
        }
    }
    (!builder.triangles.is_empty()).then(|| builder.finish(Material::solid(color), Lighting::Unlit))
}

/// The nearest depth an edge is drawn at, in world units in front of the camera: a little past
/// the camera's near plane, so a ribbon's width there stays finite.
const EDGE_NEAREST: f32 = 0.15;

/// The part of the line from `from` to `to` at least `EDGE_NEAREST` in front of the camera, by
/// `depth`, or `None` when none of it is.
fn in_front(from: Vec3, to: Vec3, depth: impl Fn(Vec3) -> f32) -> Option<(Vec3, Vec3)> {
    let (d_from, d_to) = (depth(from), depth(to));
    let cut = |behind: Vec3, d_behind: f32, ahead: Vec3, d_ahead: f32| {
        behind.lerp(ahead, (EDGE_NEAREST - d_behind) / (d_ahead - d_behind))
    };
    match (d_from >= EDGE_NEAREST, d_to >= EDGE_NEAREST) {
        (true, true) => Some((from, to)),
        (false, false) => None,
        (false, true) => Some((cut(from, d_from, to, d_to), to)),
        (true, false) => Some((from, cut(to, d_to, from, d_from))),
    }
}

/// The world lines a mesh shows as edges from `camera_position`: each crease sharper than
/// `CREASE_DEGREES`, each rim no other face shares, and the outline, where a face toward the
/// camera meets one away from it. They are found in the mesh's own space, where its normals were
/// shared, so an uneven scale draws no edge down a side it shades smooth. Points with the same
/// bits are one point, so faces that meet share their sides. A mesh its matrix flattens has none.
fn edge_lines(mesh: &Mesh, mesh_to_world: Mat4, camera_position: Vec3) -> Vec<(Vec3, Vec3)> {
    if mesh_to_world.determinant() == 0.0 {
        return Vec::new();
    }
    let camera_position = mesh_to_world.inverse().transform_point3(camera_position);
    let mut welded: HashMap<[u32; 3], u32> = HashMap::new();
    let mut points: Vec<Vec3> = Vec::new();
    let ids: Vec<u32> = mesh
        .points
        .iter()
        .map(|p| {
            *welded
                .entry(p.position.map(f32::to_bits))
                .or_insert_with(|| {
                    points.push(Vec3::from(p.position));
                    points.len() as u32 - 1
                })
        })
        .collect();
    let mut faces: Vec<(Vec3, Vec3)> = Vec::new();
    let mut sides: BTreeMap<(u32, u32), Vec<usize>> = BTreeMap::new();
    for triangle in &mesh.triangles {
        let [a, b, c] = triangle.map(|i| ids[i as usize]);
        let (pa, pb, pc) = (points[a as usize], points[b as usize], points[c as usize]);
        let Some(normal) = (pb - pa).cross(pc - pa).try_normalize() else {
            continue;
        };
        let point_normals: Vec3 = triangle
            .iter()
            .map(|&i| Vec3::from(mesh.points[i as usize].normal))
            .sum();
        faces.push((
            if normal.dot(point_normals) < 0.0 {
                -normal
            } else {
                normal
            },
            pa,
        ));
        for (x, y) in [(a, b), (b, c), (c, a)] {
            sides
                .entry((x.min(y), x.max(y)))
                .or_default()
                .push(faces.len() - 1);
        }
    }
    let crease = CREASE_DEGREES.to_radians().cos();
    let toward = |face: usize| faces[face].0.dot(camera_position - faces[face].1) > 0.0;
    sides
        .into_iter()
        .filter(|(_, shared)| match shared.as_slice() {
            [_] => true,
            &[one, other] => {
                faces[one].0.dot(faces[other].0) < crease || toward(one) != toward(other)
            }
            _ => false,
        })
        .map(|((a, b), _)| {
            (
                mesh_to_world.transform_point3(points[a as usize]),
                mesh_to_world.transform_point3(points[b as usize]),
            )
        })
        .collect()
}

/// The sphere of an atom is cut into fewer triangles than a lone sphere object: 1024, so a
/// molecule of a few hundred atoms stays cheap to build and to draw each frame. At 1080p a
/// ball a half unit across is about 130 pixels, and 32 segments keep its silhouette within a
/// quarter pixel of round.
const BALL_SEGMENTS: u32 = 32;
const BALL_RINGS: u32 = 16;

/// The fewest segments a space-filling ball is cut into, however small it shows.
const FEWEST_BALL_SEGMENTS: u32 = 8;

/// How far the edge of a space-filling ball's disc may stray inside round, in pixels.
const ROUND_PIXELS: f32 = 0.3;

/// The sides of a stick.
const STICK_SIDES: u32 = 12;

/// How many segments around an object's balls are cut into, with half as many rings: a pure
/// function of the stage, so a frame still draws on its own. Ball-and-stick and a pattern of
/// spheres cut every ball into `BALL_SEGMENTS`. Space-filling draws thousands of balls four times
/// as wide, so it cuts them only as finely as its largest ball needs where the molecule reaches
/// nearest the camera: the fewest segments, a multiple of 4, whose disc strays less than
/// `ROUND_PIXELS` inside round, from `FEWEST_BALL_SEGMENTS` to `BALL_SEGMENTS`.
fn ball_segments(
    shape: &Shape,
    transform: &Transform,
    camera: &Camera,
    scene_config: &SceneConfig,
) -> u32 {
    let Shape::Molecule {
        molecule,
        representation: representation @ Representation::SpaceFilling { .. },
        ..
    } = shape
    else {
        return BALL_SEGMENTS;
    };
    let to_world = transform.object_to_world();
    let stretch = crate::molecule::largest_stretch(to_world);
    let nearest =
        transform.position + camera.direction() * representation.reach(molecule) * stretch;
    let largest = molecule
        .numbers()
        .filter_map(|n| representation.ball_radius(molecule.atom(n)))
        .fold(0.0, f32::max);
    let pixels = largest * stretch * camera.pixels_per_unit(nearest, scene_config);
    if !pixels.is_finite() || pixels <= 0.0 {
        return BALL_SEGMENTS;
    }
    let turn = (1.0 - ROUND_PIXELS / pixels).max(-1.0).acos();
    let segments = (PI / turn).ceil() as u32;
    segments
        .next_multiple_of(4)
        .clamp(FEWEST_BALL_SEGMENTS, BALL_SEGMENTS)
}

/// One mesh for every ball and stick of an element color, so a molecule of any size is as many
/// draws as it has colors. Every vertex is already in the molecule's own space.
fn drawn_meshes(drawn: &Drawn, segments: u32) -> Vec<Mesh> {
    meshes_by_color(drawn, Material::solid, Lighting::KeyLight, segments)
}

fn meshes_by_color(
    drawn: &Drawn,
    material: impl Fn(Color) -> Material,
    lighting: Lighting,
    segments: u32,
) -> Vec<Mesh> {
    let unit = UnitSphere::new(segments, segments / 2);
    let mut by_color: BTreeMap<[u8; 3], (Color, MeshBuilder)> = BTreeMap::new();
    fn of(
        by_color: &mut BTreeMap<[u8; 3], (Color, MeshBuilder)>,
        color: Color,
    ) -> &mut MeshBuilder {
        &mut by_color
            .entry([color.r, color.g, color.b])
            .or_insert_with(|| (color, MeshBuilder::default()))
            .1
    }
    for ball in &drawn.balls {
        of(&mut by_color, ball.color).sphere(ball.center, ball.radius, &unit);
    }
    for stick in &drawn.sticks {
        of(&mut by_color, stick.color).stick(stick.from, stick.to, stick.radius, STICK_SIDES);
    }
    for tube in &drawn.tubes {
        sweep(of(&mut by_color, tube.color), &tube.sections, false);
    }
    by_color
        .into_values()
        .map(|(color, builder)| builder.finish(material(color), lighting))
        .collect()
}

/// The points of a sphere of radius 1 about the origin, which are its normals: rings from the +y
/// pole to the -y pole, `segments` around, with the seam repeated so that it can be an edge.
/// Worked out once and placed at every ball, so a molecule of thousands of balls takes no sine.
struct UnitSphere {
    segments: u32,
    rings: u32,
    normals: Vec<Vec3>,
}

impl UnitSphere {
    fn new(segments: u32, rings: u32) -> UnitSphere {
        let normals = (0..=rings)
            .flat_map(|ring| {
                let polar = PI * ring as f32 / rings as f32;
                (0..=segments).map(move |segment| {
                    let azimuth = TAU * segment as f32 / segments as f32;
                    Vec3::new(
                        polar.sin() * azimuth.sin(),
                        polar.cos(),
                        polar.sin() * azimuth.cos(),
                    )
                })
            })
            .collect();
        UnitSphere {
            segments,
            rings,
            normals,
        }
    }
}

/// Triangles of one material, added a solid at a time. Each solid's normals point out of it.
#[derive(Default)]
struct MeshBuilder {
    points: Vec<Point>,
    triangles: Vec<[u32; 3]>,
}

impl MeshBuilder {
    fn finish(self, material: Material, lighting: Lighting) -> Mesh {
        Mesh {
            points: self.points,
            triangles: self.triangles,
            material,
            overlay: false,
            lighting,
            edges: None,
        }
    }

    /// Adding 0 turns a -0 into 0, so two points that meet have the same bits and `edge_mesh`
    /// welds them.
    fn push(&mut self, position: Vec3, normal: Vec3) -> u32 {
        self.points.push(Point {
            position: (position + 0.0).to_array(),
            normal: normal.to_array(),
        });
        self.points.len() as u32 - 1
    }

    /// `unit` placed at `center` and grown to `radius`.
    fn sphere(&mut self, center: Vec3, radius: f32, unit: &UnitSphere) {
        let first = self.points.len() as u32;
        self.points.extend(unit.normals.iter().map(|&normal| Point {
            position: (center + normal * radius).to_array(),
            normal: normal.to_array(),
        }));
        let (segments, rings) = (unit.segments, unit.rings);
        let corner = |ring: u32, segment: u32| first + ring * (segments + 1) + segment;
        for ring in 0..rings {
            for segment in 0..segments {
                let (a, b) = (corner(ring, segment), corner(ring, segment + 1));
                let (c, d) = (corner(ring + 1, segment), corner(ring + 1, segment + 1));
                self.triangles.push([a, c, b]);
                self.triangles.push([b, c, d]);
            }
        }
    }

    /// A tube of `radius` around the segment from `from` to `to`, open at both ends. A stick
    /// ends inside a ball or against its other half, so it needs no caps. A stick of no length
    /// adds nothing.
    fn stick(&mut self, from: Vec3, to: Vec3, radius: f32, sides: u32) {
        let axis = (to - from).normalize_or_zero();
        if axis == Vec3::ZERO {
            return;
        }
        let (u, v) = axis.any_orthonormal_pair();
        let first = self.points.len() as u32;
        for side in 0..=sides {
            let angle = TAU * side as f32 / sides as f32;
            let normal = u * angle.cos() + v * angle.sin();
            for end in [from, to] {
                self.points.push(Point {
                    position: (end + normal * radius).to_array(),
                    normal: normal.to_array(),
                });
            }
        }
        for side in 0..sides {
            let (a, b) = (first + side * 2, first + side * 2 + 2);
            self.triangles.push([a, a + 1, b]);
            self.triangles.push([b, a + 1, b + 1]);
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::Vec3;

    use super::*;
    use crate::scene::{Color, Positive};

    #[test]
    fn a_dashed_arrow_cuts_its_shaft_in_world_units_and_keeps_its_tip_whole() {
        let arrow = Path::arrow(Vec2::ZERO, Vec2::new(1.0, 0.0), 0.2).unwrap();
        let dash = DashPattern(
            crate::scene::DashLength::try_from(0.3).unwrap(),
            crate::scene::DashLength::try_from(0.2).unwrap(),
        );
        let round = |v: f64| (v * 1e3).round() / 1e3 + 0.0;
        let starts = |elements: &[PathEl]| -> Vec<(f64, f64)> {
            elements
                .iter()
                .filter_map(|e| match e {
                    PathEl::MoveTo(p) => Some((round(p.x), round(p.y))),
                    _ => None,
                })
                .collect()
        };

        let doubled = dashed(arrow.elements(), Bake::Scaled(Vec2::splat(2.0)), dash, true);
        let mut shaft = starts(&doubled);
        let tip = shaft.pop();
        shaft.sort_by(|a, b| a.0.total_cmp(&b.0));

        assert_eq!(
            shaft,
            [(0.0, 0.0), (0.5, 0.0), (1.0, 0.0), (1.5, 0.0)],
            "dashes from 0, 0.5, 1 and 1.5 along a shaft 1.6 long, the arrow baked twice its size"
        );
        assert_eq!(tip, Some((1.6, -0.2)), "then the tip from its corner");
        assert_eq!(
            doubled.last(),
            Some(&PathEl::ClosePath),
            "the tip is closed"
        );
    }

    fn area(mesh: &Mesh) -> f32 {
        let at = |i: u32| Vec3::from(mesh.points[i as usize].position);
        mesh.triangles
            .iter()
            .map(|&[a, b, c]| (at(b) - at(a)).cross(at(c) - at(a)).length() / 2.0)
            .sum()
    }

    fn white() -> Color {
        Color {
            r: 0xff,
            g: 0xff,
            b: 0xff,
        }
    }

    fn stroke(width: f32) -> Stroke {
        Stroke {
            width: crate::scene::Positive::try_from(f64::from(width)).unwrap(),
            ..Stroke::default()
        }
    }

    #[test]
    fn a_circle_fill_is_a_flat_disc_of_area_pi_r_squared() {
        let meshes = flat_meshes(
            &Path::circle(2.0).unwrap(),
            Vec2::ONE,
            None,
            Some(Fill::solid(white())),
            1.0,
        );

        let [mesh] = meshes.as_slice() else {
            panic!("one mesh")
        };
        assert!(
            (area(mesh) - 4.0 * PI).abs() < 4.0 * PI * 3e-3,
            "area {}",
            area(mesh)
        );
        assert!(
            mesh.points
                .iter()
                .all(|p| p.position[2] == 0.0 && p.normal == [0.0, 0.0, 1.0])
        );
        assert!(!mesh.overlay);
    }

    #[test]
    fn a_circle_stroke_is_a_ring_of_its_width_centered_on_the_radius() {
        let meshes = flat_meshes(
            &Path::circle(2.0).unwrap(),
            Vec2::ONE,
            Some(stroke(0.5)),
            None,
            1.0,
        );

        let [mesh] = meshes.as_slice() else {
            panic!("one mesh")
        };
        let expected = PI * (2.25f32.powi(2) - 1.75f32.powi(2));
        assert!(
            (area(mesh) - expected).abs() < expected * 1e-2,
            "area {}",
            area(mesh)
        );
        let radii = mesh.points.iter().map(|p| Vec3::from(p.position).length());
        assert!(
            radii.clone().all(|r| (1.74..=2.26).contains(&r)),
            "{:?}",
            radii.collect::<Vec<_>>()
        );
        assert!(mesh.overlay);
    }

    #[test]
    fn a_create_draws_the_stroke_over_the_first_half_and_fades_the_fill_over_the_second() {
        assert_eq!(created_shares(0.25, true, true), (0.5, 0.0));
        assert_eq!(created_shares(0.5, true, true), (1.0, 0.0));
        assert_eq!(created_shares(0.75, true, true), (1.0, 0.5));
        assert_eq!(created_shares(0.3, true, false), (0.3, 0.0));
        assert_eq!(created_shares(0.3, false, true), (0.0, 0.3));
        assert_eq!(created_shares(1.0, false, false), (0.0, 0.0));
    }

    #[test]
    fn a_half_created_line_is_a_ribbon_over_half_the_line() {
        let line = Path::line(Vec2::new(-2.0, 0.0), Vec2::new(2.0, 0.0)).unwrap();

        let meshes = flat_meshes(&line, Vec2::ONE, Some(stroke(0.2)), None, 0.5);

        let [mesh] = meshes.as_slice() else {
            panic!("one mesh")
        };
        let xs = mesh.points.iter().map(|p| p.position[0]);
        let (min_x, max_x) = xs.fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
        assert_eq!(
            (min_x, max_x),
            (-2.0, 0.0),
            "butt caps end exactly at the line's ends"
        );
        assert!(
            (area(mesh) - 0.4).abs() < 1e-5,
            "2 long by 0.2 wide, area {}",
            area(mesh)
        );
        assert!(
            flat_meshes(&line, Vec2::ONE, Some(stroke(0.2)), None, 0.0).is_empty(),
            "nothing drawn at 0"
        );
    }

    #[test]
    fn a_fill_that_has_not_begun_to_show_has_no_mesh() {
        let square = Path::polygon(&[
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ])
        .unwrap();

        let half = flat_meshes(
            &square,
            Vec2::ONE,
            Some(stroke(0.1)),
            Some(Fill::solid(white())),
            0.5,
        );
        let three_quarters = flat_meshes(
            &square,
            Vec2::ONE,
            Some(stroke(0.1)),
            Some(Fill::solid(white())),
            0.75,
        );

        assert_eq!(half.len(), 1, "the stroke only");
        assert!(half[0].overlay);
        assert_eq!(
            three_quarters.len(),
            2,
            "the fill at half its opacity, then the stroke"
        );
        assert_eq!(three_quarters[0].material.opacity(), 0.5);
        assert!((area(&three_quarters[0]) - 4.0).abs() < 1e-5);
    }

    #[test]
    fn a_half_written_math_of_seven_glyphs_has_drawn_its_first_and_not_its_last() {
        let shares: Vec<f32> = paced_shares(7, 0.5)
            .map(|s| (s * 1000.0).round() / 1000.0)
            .collect();

        assert_eq!(shares, [1.0, 0.9, 0.7, 0.5, 0.3, 0.1, 0.0]);
        assert!(paced_shares(7, 0.0).all(|s| s == 0.0));
        assert!(paced_shares(7, 1.0).all(|s| s == 1.0));
        assert!(
            paced_shares(5, 1.0).all(|s| s == 1.0),
            "five glyphs, where the last falls short of 1 in f32"
        );
        assert_eq!(
            paced_shares(1, 0.25).collect::<Vec<_>>(),
            [0.25],
            "one glyph takes the whole write"
        );
    }

    #[test]
    fn a_written_glyph_is_its_fill_alone_and_a_half_written_one_is_its_whole_stroke() {
        let square = Path::polygon(&[
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ])
        .unwrap();
        let (fill, pen) = (Fill::solid(white()), stroke(0.1));

        let half = glyph_meshes(&square, Vec2::ONE, fill, pen, 0.5);
        let three_quarters = glyph_meshes(&square, Vec2::ONE, fill, pen, 0.75);
        let written = glyph_meshes(&square, Vec2::ONE, fill, pen, 1.0);

        let [stroke_only] = half.as_slice() else {
            panic!("the stroke only")
        };
        assert!(
            (area(stroke_only) - 8.0 * 0.1).abs() < 1e-4,
            "the whole path, 0.1 wide: {}",
            area(stroke_only)
        );
        let [faded_fill, thinned] = three_quarters.as_slice() else {
            panic!("fill and stroke")
        };
        assert_eq!(faded_fill.material.opacity(), 0.5);
        assert!(
            (area(thinned) - 8.0 * 0.05).abs() < 1e-4,
            "half as wide while the fill is half in: {}",
            area(thinned)
        );
        let [fill_only] = written.as_slice() else {
            panic!("the fill only")
        };
        assert!(!fill_only.overlay && (area(fill_only) - 4.0).abs() < 1e-5);
    }

    fn region(path: Path, rule: FillRule) -> Region {
        Region { path, rule }
    }

    fn square(half: f32) -> Path {
        Path::polygon(&[
            Vec2::new(-half, -half),
            Vec2::new(half, -half),
            Vec2::new(half, half),
            Vec2::new(-half, half),
        ])
        .unwrap()
    }

    #[test]
    fn a_prism_on_a_square_is_a_box_standing_on_its_base_with_outward_normals() {
        let mesh = prism(
            &region(square(1.0), FillRule::NonZero),
            3.0,
            Material::default(),
        )
        .unwrap();

        assert!(
            (area(&mesh) - (2.0 * 4.0 + 4.0 * 2.0 * 3.0)).abs() < 1e-4,
            "two 2 by 2 faces and four 2 by 3 walls: {}",
            area(&mesh)
        );
        let ys = mesh.points.iter().map(|p| p.position[1]);
        assert_eq!(
            ys.clone().fold(f32::MAX, f32::min),
            0.0,
            "it stands on its base"
        );
        assert_eq!(ys.fold(f32::MIN, f32::max), 3.0);
        let middle = Vec3::new(0.0, 1.5, 0.0);
        for point in &mesh.points {
            assert!(
                Vec3::from(point.normal).dot(Vec3::from(point.position) - middle) > 0.0,
                "{point:?} points out"
            );
        }
    }

    #[test]
    fn a_base_point_lands_at_its_x_and_minus_its_y() {
        let triangle = Path::polygon(&[
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(0.0, 2.0),
        ])
        .unwrap();

        let mesh = prism(
            &region(triangle, FillRule::NonZero),
            1.0,
            Material::default(),
        )
        .unwrap();

        let far = mesh
            .points
            .iter()
            .map(|p| p.position[2])
            .fold(f32::MAX, f32::min);
        assert_eq!(
            far, -2.0,
            "y 2 of the base runs away from the default camera"
        );
    }

    #[test]
    fn a_prism_on_a_ring_under_even_odd_is_a_pipe_whose_inner_wall_faces_the_hole() {
        let annulus = Path::from_svg("M 1 0 A 1 1 0 1 1 -1 0 A 1 1 0 1 1 1 0 Z M 0.5 0 A 0.5 0.5 0 1 1 -0.5 0 A 0.5 0.5 0 1 1 0.5 0 Z").unwrap();

        let mesh = prism(
            &region(annulus, FillRule::EvenOdd),
            2.0,
            Material::default(),
        )
        .unwrap();

        let expected = 2.0 * PI * (1.0 - 0.25) + 2.0 * PI * 2.0 + 2.0 * PI * 0.5 * 2.0;
        assert!(
            (area(&mesh) - expected).abs() < expected * 1e-2,
            "two annuli and two walls: {} against {expected}",
            area(&mesh)
        );
        let inner = mesh
            .points
            .iter()
            .find(|p| p.normal[1] == 0.0 && Vec2::new(p.position[0], p.position[2]).length() < 0.6)
            .unwrap();
        assert!(
            Vec3::from(inner.normal).dot(Vec3::from(inner.position).with_y(0.0)) < 0.0,
            "{inner:?} faces the axis"
        );
    }

    #[test]
    fn a_revolution_of_a_rectangle_against_the_axis_is_a_closed_cylinder() {
        let rectangle = Path::polygon(&[
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(0.0, 2.0),
        ])
        .unwrap();

        let mesh = revolution(
            &region(rectangle, FillRule::NonZero),
            TAU,
            Material::default(),
        )
        .unwrap();

        let expected = 2.0 * PI * 2.0 + 2.0 * PI;
        assert!(
            (area(&mesh) - expected).abs() < expected * 1e-2,
            "a side and two discs: {}",
            area(&mesh)
        );
        let radii = mesh
            .points
            .iter()
            .map(|p| Vec2::new(p.position[0], p.position[2]).length());
        assert!(
            radii.clone().all(|r| r < 1.0 + 1e-5) && (radii.fold(0.0, f32::max) - 1.0).abs() < 1e-5,
            "around the y axis at radius 1"
        );
    }

    #[test]
    fn a_half_revolution_is_closed_by_its_profile_at_both_cut_faces() {
        let rectangle = Path::polygon(&[
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(0.0, 2.0),
        ])
        .unwrap();

        let mesh = revolution(
            &region(rectangle, FillRule::NonZero),
            PI,
            Material::default(),
        )
        .unwrap();

        let expected = PI * 2.0 + PI + 2.0 * 2.0;
        assert!(
            (area(&mesh) - expected).abs() < expected * 1e-2,
            "half a side, two half discs and two 1 by 2 cut faces: {}",
            area(&mesh)
        );
        assert!(
            mesh.points.iter().all(|p| p.position[2] <= 1e-6),
            "it turns from +x toward -z"
        );
    }

    #[test]
    fn a_create_raises_a_prism_and_turns_a_revolution() {
        let solid = |created| {
            let prism = Solid::Prism {
                base: region(square(1.0), FillRule::NonZero),
                height: Positive::try_from(2.0).unwrap(),
            };
            solid_mesh(&prism, Material::default(), EdgeShade::Dark, created)
        };

        assert!(solid(0.0).is_none(), "nothing before the create begins");
        let half = solid(0.5).unwrap();
        assert_eq!(
            half.points
                .iter()
                .map(|p| p.position[1])
                .fold(f32::MIN, f32::max),
            1.0,
            "half its height"
        );
        assert_eq!(
            half.edges,
            Some(EdgeShade::Dark.of(Color {
                r: 0xff,
                g: 0xff,
                b: 0xff
            }))
        );
    }

    #[test]
    fn a_box_shows_its_twelve_edges_and_no_line_across_a_face() {
        let mesh = prism(
            &region(square(1.0), FillRule::NonZero),
            2.0,
            Material::default(),
        )
        .unwrap();

        let lines = edge_lines(&mesh, Mat4::IDENTITY, Vec3::new(4.0, 5.0, 6.0));

        assert_eq!(lines.len(), 12, "{lines:?}");
        for (from, to) in lines {
            assert_eq!(
                (to - from)
                    .abs()
                    .cmpgt(Vec3::splat(1e-6))
                    .bitmask()
                    .count_ones(),
                1,
                "{from} to {to} runs along one axis"
            );
        }
    }

    #[test]
    fn a_whole_revolution_shows_no_seam() {
        let rectangle = Path::polygon(&[
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(0.0, 2.0),
        ])
        .unwrap();
        let mesh = revolution(
            &region(rectangle, FillRule::NonZero),
            TAU,
            Material::default(),
        )
        .unwrap();

        let lines = edge_lines(&mesh, Mat4::IDENTITY, Vec3::new(10.0, 5.0, 0.0));

        let seam = lines.iter().filter(|(from, to)| {
            (from.y - to.y).abs() > 1.0 && from.x > 0.9 && from.z.abs() < 1e-3
        });
        assert_eq!(
            seam.count(),
            0,
            "the side at +x faces the camera_position, and its first and last pieces meet"
        );
        let rims = lines.iter().filter(|(from, to)| from.y == to.y).count();
        assert_eq!(rims, 128, "the bottom and top rims, 64 pieces each");
    }

    #[test]
    fn two_overlapping_squares_have_walls_all_around_their_union() {
        let squares =
            Path::from_svg("M 0 0 L 2 0 L 2 2 L 0 2 Z M 1 1 L 2.5 1 L 2.5 2.5 L 1 2.5 Z").unwrap();
        let region = region(squares, FillRule::NonZero);

        let length: f32 = walls(&region, &straight_parts(&region.path))
            .iter()
            .map(|wall| wall.from.distance(wall.to))
            .sum();

        assert!(
            (length - 10.0).abs() < 1e-4,
            "8 + 6 less the 4 of the overlap's square: {length}"
        );
    }

    #[test]
    fn a_turn_within_half_a_piece_of_whole_is_whole() {
        let rectangle = Path::polygon(&[
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(0.0, 2.0),
        ])
        .unwrap();

        let nearly = revolution(
            &region(rectangle.clone(), FillRule::NonZero),
            359.99f32.to_radians(),
            Material::default(),
        )
        .unwrap();
        let whole = revolution(
            &region(rectangle, FillRule::NonZero),
            TAU,
            Material::default(),
        )
        .unwrap();

        assert_eq!(
            nearly.triangles.len(),
            whole.triangles.len(),
            "no cut faces"
        );
    }

    #[test]
    fn a_squashed_revolution_shows_no_edges_down_its_smooth_side() {
        let rectangle = Path::polygon(&[
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 2.0),
            Vec2::new(0.0, 2.0),
        ])
        .unwrap();
        let mesh = revolution(
            &region(rectangle, FillRule::NonZero),
            TAU,
            Material::default(),
        )
        .unwrap();

        let lines = edge_lines(
            &mesh,
            Mat4::from_scale(Vec3::new(1.0, 1.0, 0.1)),
            Vec3::new(0.0, 5.0, 10.0),
        );

        let down_the_side = lines
            .iter()
            .filter(|(from, to)| (from.y - to.y).abs() > 1.0)
            .count();
        assert_eq!(
            down_the_side, 2,
            "only the outline at each side: {down_the_side}"
        );
    }

    #[test]
    fn an_edge_that_runs_behind_the_camera_is_cut_where_it_passes_it() {
        let depth = |p: Vec3| 10.0 - p.z;

        let cut = in_front(Vec3::new(1.0, 0.0, 20.0), Vec3::new(1.0, 0.0, 0.0), depth);

        assert_eq!(
            cut,
            Some((
                Vec3::new(1.0, 0.0, 10.0 - EDGE_NEAREST),
                Vec3::new(1.0, 0.0, 0.0)
            ))
        );
        assert_eq!(
            in_front(Vec3::new(0.0, 0.0, 11.0), Vec3::new(0.0, 0.0, 12.0), depth),
            None
        );
    }

    #[test]
    fn a_sphere_half_created_is_half_turned_and_shows_no_edges() {
        let mesh = created_sphere(1.0, Material::default(), 0.5).unwrap();

        assert!(mesh.edges.is_none());
        assert!(
            mesh.points.iter().all(|p| p.position[2] <= 1e-6),
            "half a turn from +x toward -z"
        );
        let expected = 2.0 * PI + PI;
        assert!(
            (area(&mesh) - expected).abs() < expected * 1e-2,
            "half the surface and two half discs: {}",
            area(&mesh)
        );
    }

    #[test]
    fn a_sphere_has_its_points_on_the_surface_with_outward_normals() {
        let mesh = sphere(1.5, Material::default(), SPHERE_SEGMENTS, SPHERE_RINGS);

        for point in &mesh.points {
            let position = Vec3::from(point.position);
            assert!((position.length() - 1.5).abs() < 1e-5);
            assert!((Vec3::from(point.normal) - position / 1.5).length() < 1e-5);
        }
        let expected = 4.0 * PI * 1.5 * 1.5;
        assert!(
            (area(&mesh) - expected).abs() < expected * 1e-2,
            "area {}",
            area(&mesh)
        );
    }

    #[test]
    fn a_space_filling_molecule_cuts_its_balls_as_finely_as_its_largest_ball_shows() {
        let carbon = std::sync::Arc::new(
            crate::molecule::Molecule::new(
                vec![(crate::element::Element::from_symbol("C").unwrap(), [0.0; 3])],
                vec![],
            )
            .unwrap(),
        );
        let segments = |representation, scale: f32| {
            let shape = Shape::Molecule {
                molecule: carbon.clone(),
                representation,
                groups: crate::group::Groups::default(),
                glows: BTreeMap::new(),
            };
            let transform = Transform {
                scale: Vec3::splat(scale),
                ..Transform::default()
            };
            ball_segments(
                &shape,
                &transform,
                &Camera::default(),
                &SceneConfig::default(),
            )
        };
        let filling = Representation::SpaceFilling { hydrogens: true };

        assert_eq!(
            [0.01, 0.02, 0.05, 0.1, 0.2, 0.5].map(|scale| segments(filling, scale)),
            [8, 12, 16, 20, 28, 32],
            "a carbon of 2.2, 4.5, 11, 23, 46 and 115 pixels at 1080p"
        );
        assert_eq!(
            segments(Representation::BallAndStick { hydrogens: true }, 0.01),
            BALL_SEGMENTS
        );
    }

    #[test]
    fn a_flat_sweep_with_a_step_is_an_arrow_whose_shoulder_faces_back() {
        let at = |x: f32, half_width: f32| CrossSection {
            point: Vec3::new(x, 0.0, 0.0),
            tangent: Vec3::X,
            across: Vec3::Y,
            half_width,
            half_thickness: 0.25,
        };
        let mut builder = MeshBuilder::default();
        sweep(
            &mut builder,
            &[at(0.0, 1.0), at(1.0, 1.0), at(1.0, 2.0), at(2.0, 0.25)],
            false,
        );

        let points: Vec<(Vec3, Vec3)> = builder
            .points
            .iter()
            .map(|p| (Vec3::from(p.position), Vec3::from(p.normal)))
            .collect();
        assert_eq!(
            points.len(),
            4 * 24 + 2 * 24 + 2 * 25,
            "two rings either side of the step, the shoulder's two, and a cap at each end"
        );
        let widest = |axis: usize| {
            points
                .iter()
                .map(|(p, _)| p[axis].abs())
                .fold(0.0, f32::max)
        };
        assert_eq!((widest(1), widest(2)), (2.0, 0.25));
        let shoulder = points
            .iter()
            .filter(|(p, n)| p.x == 1.0 && *n == Vec3::NEG_X)
            .count();
        assert_eq!(shoulder, 48, "both rings of the shoulder face back");
        assert!(
            points.iter().all(|(_, n)| (n.length() - 1.0).abs() < 1e-5),
            "every normal of an ellipse is a unit vector"
        );
    }

    #[test]
    fn a_round_sweep_is_the_tube_a_circle_always_drew() {
        let frames = [
            CurveFrame {
                point: Vec3::ZERO,
                tangent: Vec3::X,
                normal: Vec3::Y,
            },
            CurveFrame {
                point: Vec3::new(1.0, 0.3, 0.0),
                tangent: Vec3::new(0.8, 0.6, 0.0),
                normal: Vec3::new(-0.6, 0.8, 0.0),
            },
        ];
        let mut builder = MeshBuilder::default();
        tube_piece(&mut builder, &frames, 0.4, false);

        let (sin, cos) = (TAU * 5.0 / 24.0).sin_cos();
        let outward = Vec3::Y * cos + Vec3::X.cross(Vec3::Y) * sin;
        assert_eq!(
            Vec3::from(builder.points[5].position),
            outward * 0.4,
            "the sixth point of the first ring, by a circle's own sum, bit for bit"
        );
        assert_eq!(Vec3::from(builder.points[5].normal), outward);
    }
}
