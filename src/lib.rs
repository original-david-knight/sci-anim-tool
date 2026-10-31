//! A frame is drawn in three steps, each a call in `frame` below:
//!
//! ```text
//! Scene x SceneTime  --Scene::at-------->  Stage      the camera and objects at that time
//! Stage              --mesh::draw_list--->  DrawList   every object as meshes
//! DrawList           --Gpu::draw--------->  Frame      pixels, the only GPU step
//! ```
//!
//! `render` draws every frame that way, in scene time order, into one `Encoder`. `inspect` runs
//! the first two steps and measures the draw list, with no GPU.

mod axes;
mod axes_3d;
mod bar_plot;
mod cartoon;
mod check_layout;
mod cli;
mod data_point;
mod element;
mod ends;
mod expr;
mod follow;
mod frame;
mod frame_sequence;
mod gpu;
mod group;
mod member;
mod mesh;
mod molecule;
mod morph;
mod path;
mod pattern;
mod plane_map;
mod plot;
mod reaction;
mod readout;
mod scene;
mod scene_file;
mod simulate;
mod skill;
mod space_curve;
mod structure_file;
mod surface;
mod table;
mod timeline;
mod trail;
mod typeset;
mod video;

use std::collections::{BTreeMap, VecDeque};
use std::thread::JoinHandle;

pub use cli::run;

use axes::{AxesReport, NumberLineReport};
use axes_3d::Axes3dReport;
use frame::Frame;
use frame_sequence::{FrameSequence, FrameSequenceError, FrameSequenceWriter};
use glam::{Vec2, Vec3};
use gpu::{Drawing, Gpu, GpuError, GpuIdentity, GpuKind, Readback};
use mesh::{BoundingBox, InFrame, Source};
use molecule::MoleculeReport;
use path::UnderMap;
use plot::PlotReport;
use reaction::ReactionReport;
use scene::{
    Background, Camera, ObjectId, Phase, Scene, SceneTime, Shape, Transform, UnitInterval,
};
use serde::Serialize;
use video::{Encoder, Video, VideoError};

fn frame(gpu: &Gpu, scene: &Scene, scene_time: SceneTime) -> Result<Frame, GpuError> {
    readback(gpu, scene, scene_time)?.frame()
}

fn readback(gpu: &Gpu, scene: &Scene, scene_time: SceneTime) -> Result<Readback, GpuError> {
    gpu.readback(draw(gpu, scene, scene_time)?)
}

fn draw(gpu: &Gpu, scene: &Scene, scene_time: SceneTime) -> Result<Drawing, GpuError> {
    let stage = scene.at(scene_time);
    gpu.draw(
        &scene.scene_config,
        &mesh::draw_list(&stage, &scene.scene_config),
    )
}

enum RenderError {
    Gpu(GpuError),
    Video(VideoError),
    TransparentBackground,
}

enum FrameSequenceRenderError {
    Gpu(GpuError),
    Files(FrameSequenceError),
}

/// How far a render has got, for an agent waiting on a long one.
#[derive(Debug, Clone, Copy, Serialize)]
struct RenderUpdate {
    update: RenderUpdateKind,
    frames_drawn: u32,
    frames: u32,
}

/// ffmpeg encodes each frame as it arrives, so `Finishing` is only ffmpeg's tail and the move
/// of the video into place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RenderUpdateKind {
    Drawing,
    Finishing,
    Finished,
}

/// ffmpeg starts before the GPU, so a machine with no ffmpeg fails in milliseconds.
/// `on_update` hears a drawing update for 0 frames and after every frame, then finishing, then
/// finished only once the video is in place.
fn render(
    scene: &Scene,
    mp4: &std::path::Path,
    gpu_kind: Option<GpuKind>,
    on_update: &mut dyn FnMut(RenderUpdate),
) -> Result<(Video, GpuIdentity), RenderError> {
    if scene.scene_config.background == Background::Transparent {
        return Err(RenderError::TransparentBackground);
    }
    let frames = scene.frame_scene_times().len() as u32;
    let mut update = |update, frames_drawn| {
        on_update(RenderUpdate {
            update,
            frames_drawn,
            frames,
        })
    };
    let encoder = Encoder::start(&scene.scene_config, scene.sounds(), frames, mp4)
        .map_err(RenderError::Video)?;
    let gpu = Gpu::new(scene.scene_config.background, gpu_kind).map_err(RenderError::Gpu)?;
    update(RenderUpdateKind::Drawing, 0);
    // ffmpeg is fed on a thread of its own, so the GPU draws while ffmpeg takes the frames before.
    let encoder = std::thread::scope(|scope| {
        let (sender, writing) = std::sync::mpsc::sync_channel::<Readback>(WRITING_FRAMES);
        let writer = scope.spawn(move || write_frames(encoder, writing));
        let drawing = (|| {
            for (drawn, readback) in (1..).zip(readbacks(&gpu, scene)) {
                if sender.send(readback?).is_err() {
                    // The writer stopped on an error, which its join reports.
                    break;
                }
                update(RenderUpdateKind::Drawing, drawn);
            }
            Ok(())
        })();
        drop(sender);
        let encoder = match writer.join() {
            Ok(encoder) => encoder?,
            Err(panic) => std::panic::resume_unwind(panic),
        };
        drawing.map_err(RenderError::Gpu)?;
        Ok(encoder)
    })?;
    update(RenderUpdateKind::Finishing, frames);
    let video = encoder.finish().map_err(RenderError::Video)?;
    update(RenderUpdateKind::Finished, frames);
    Ok((video, gpu.identity))
}

/// How many drawn frames wait for the writer at once. Each holds its readback, 33 MB at 3840 by
/// 2160.
const WRITING_FRAMES: usize = 2;

fn write_frames(
    mut encoder: Encoder,
    writing: std::sync::mpsc::Receiver<Readback>,
) -> Result<Encoder, RenderError> {
    for readback in writing {
        let frame = readback.frame().map_err(RenderError::Gpu)?;
        encoder.push(&frame).map_err(RenderError::Video)?;
    }
    Ok(encoder)
}

/// Every frame's readback in scene time order. The processor stages each frame while the GPU
/// still draws the one before.
fn readbacks<'a>(
    gpu: &'a Gpu,
    scene: &'a Scene,
) -> impl Iterator<Item = Result<Readback, GpuError>> + 'a {
    let mut scene_times = scene.frame_scene_times();
    let mut drawing: Option<Drawing> = None;
    std::iter::from_fn(move || {
        let mut next = || {
            scene_times
                .next()
                .map(|scene_time| draw(gpu, scene, scene_time))
                .transpose()
        };
        if drawing.is_none() {
            drawing = match next() {
                Ok(first) => Some(first?),
                Err(e) => return Some(Err(e)),
            };
        }
        let next = match next() {
            Ok(next) => next,
            Err(e) => return Some(Err(e)),
        };
        let previous = std::mem::replace(&mut drawing, next).expect("a frame is drawing");
        Some(gpu.readback(previous))
    })
}

/// How many frames `frames` turns into PNGs at once. Each holds its readback and its frame, 99 MB
/// at 3840 by 2160.
const ENCODING_FRAMES: usize = 8;

type Encoding = JoinHandle<Result<Vec<u8>, FrameSequenceRenderError>>;

fn write_oldest(
    encoding: &mut VecDeque<Encoding>,
    writer: &mut FrameSequenceWriter,
) -> Result<(), FrameSequenceRenderError> {
    let png = match encoding.pop_front().expect("a frame is encoding").join() {
        Ok(png) => png?,
        Err(panic) => std::panic::resume_unwind(panic),
    };
    writer
        .push_png(&png)
        .map_err(FrameSequenceRenderError::Files)
}

fn frames(
    scene: &Scene,
    directory: &std::path::Path,
    gpu_kind: Option<GpuKind>,
    on_update: &mut dyn FnMut(RenderUpdate),
) -> Result<(FrameSequence, GpuIdentity), FrameSequenceRenderError> {
    let frames = scene.frame_scene_times().len() as u32;
    let mut update = |update, frames_drawn| {
        on_update(RenderUpdate {
            update,
            frames_drawn,
            frames,
        })
    };
    let mut writer = FrameSequenceWriter::start(&scene.scene_config, directory)
        .map_err(FrameSequenceRenderError::Files)?;
    let gpu =
        Gpu::new(scene.scene_config.background, gpu_kind).map_err(FrameSequenceRenderError::Gpu)?;
    // Each frame is turned into a PNG on a thread of its own while the GPU draws the next ones,
    // at most `ENCODING_FRAMES` at once, and written in frame order.
    let mut encoding: VecDeque<Encoding> = VecDeque::with_capacity(ENCODING_FRAMES);
    let mut drawn = 0;
    update(RenderUpdateKind::Drawing, 0);
    for readback in readbacks(&gpu, scene) {
        let readback = readback.map_err(FrameSequenceRenderError::Gpu)?;
        if encoding.len() == ENCODING_FRAMES {
            write_oldest(&mut encoding, &mut writer)?;
        }
        let file = writer.file(drawn);
        drawn += 1;
        encoding.push_back(std::thread::spawn(move || {
            let frame = readback.frame().map_err(FrameSequenceRenderError::Gpu)?;
            frame.png().map_err(|source| {
                FrameSequenceRenderError::Files(FrameSequenceError { file, source })
            })
        }));
        update(RenderUpdateKind::Drawing, drawn);
    }
    while !encoding.is_empty() {
        write_oldest(&mut encoding, &mut writer)?;
    }
    update(RenderUpdateKind::Finishing, frames);
    let frame_sequence = writer.finish().map_err(FrameSequenceRenderError::Files)?;
    update(RenderUpdateKind::Finished, frames);
    Ok((frame_sequence, gpu.identity))
}

struct Inspection {
    camera: Camera,
    objects: BTreeMap<ObjectId, InspectedObject>,
    /// Every reaction of the timeline, in timeline order, at any scene time: before its step
    /// its atoms are the reactants', and after it the products'.
    reactions: Vec<ReactionReport>,
}

/// One object of an inspection, with its transform fields beside its opacity as in a scene file,
/// the created share a create or a write has reached, and what only its shape has: for text or
/// math, how many glyphs a write paces, for a molecule, its atoms and bonds, and for a pattern,
/// its copies.
#[derive(Debug, Serialize)]
struct InspectedObject {
    #[serde(flatten)]
    transform: Transform,
    opacity: f32,
    created: f32,
    /// The colors a recolor changes, as they are at this scene time.
    #[serde(skip_serializing_if = "ColorsReport::is_empty")]
    colors: ColorsReport,
    #[serde(flatten)]
    detail: Option<ShapeDetail>,
    /// On a dot on axes or a sphere on 3D axes: the axes, and its position in their data
    /// coordinates.
    #[serde(skip_serializing_if = "Option::is_none")]
    data_point: Option<DataPointReport>,
    /// On the source of a morph, from before its step to after: where the morph stands.
    #[serde(skip_serializing_if = "Option::is_none")]
    morph: Option<MorphReport>,
    /// On an attached object: what it is attached to, and on a molecule the atoms it covers on
    /// the frame.
    #[serde(skip_serializing_if = "Option::is_none")]
    attached: Option<group::AttachedReport>,
    /// On an object a linear map names, at every scene time: where linear maps have carried it.
    #[serde(skip_serializing_if = "Option::is_none")]
    plane_map: Option<PlaneMapReport>,
    bounding_box: Option<BoundingBox>,
    in_frame: InFrame,
}

/// A flat shape's stroke and fill colors, the color of a solid or of text or math whose glyphs
/// all share one, and otherwise the distinct colors of its glyphs in glyph order, each written
/// `#rrggbb`.
#[derive(Debug, Default, Serialize)]
pub(crate) struct ColorsReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    stroke: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    glyph_colors: Option<Vec<String>>,
}

impl ColorsReport {
    pub(crate) fn of_glyphs(glyphs: &[scene::Glyph]) -> ColorsReport {
        let mut glyph_colors: Vec<String> = Vec::new();
        for glyph in glyphs {
            let hex = glyph.color.hex();
            if !glyph_colors.contains(&hex) {
                glyph_colors.push(hex);
            }
        }
        match glyph_colors.as_slice() {
            [one] => ColorsReport {
                color: Some(one.clone()),
                ..ColorsReport::default()
            },
            _ => ColorsReport {
                glyph_colors: Some(glyph_colors),
                ..ColorsReport::default()
            },
        }
    }

    fn of(shape: &Shape) -> ColorsReport {
        match shape {
            Shape::Flat { stroke, fill, .. } => ColorsReport {
                stroke: stroke.map(|s| s.color.hex()),
                fill: fill.map(|f| f.color.hex()),
                ..ColorsReport::default()
            },
            Shape::Glyphs { glyphs, .. } => ColorsReport::of_glyphs(glyphs),
            Shape::Table(table) => ColorsReport {
                stroke: table.rulings.as_ref().map(|_| table.stroke.color.hex()),
                ..ColorsReport::of_glyphs(&table.glyphs)
            },
            Shape::Sphere {
                material: scene::Material::Standard { color, .. },
                ..
            }
            | Shape::Solid {
                material: scene::Material::Standard { color, .. },
                ..
            } => ColorsReport {
                color: Some(color.hex()),
                ..ColorsReport::default()
            },
            Shape::BarPlot(bar_plot) => ColorsReport {
                stroke: bar_plot.stroke.map(|s| s.color.hex()),
                fill: Some(bar_plot.fill.color.hex()),
                ..ColorsReport::default()
            },
            Shape::Pattern(pattern) => ColorsReport::of(&pattern.shape),
            Shape::Trail(trail) => ColorsReport {
                stroke: Some(trail.stroke.color.hex()),
                ..ColorsReport::default()
            },
            Shape::Mix(_)
            | Shape::Molecule { .. }
            | Shape::Axes(_)
            | Shape::NumberLine(_)
            | Shape::Axes3d(_) => ColorsReport::default(),
        }
    }

    fn is_empty(&self) -> bool {
        self.stroke.is_none()
            && self.fill.is_none()
            && self.color.is_none()
            && self.glyph_colors.is_none()
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ShapeDetail {
    Glyphs {
        glyphs: usize,
    },
    Molecule(MoleculeReport),
    Axes {
        axes: AxesReport,
    },
    NumberLine {
        number_line: NumberLineReport,
    },
    Axes3d {
        axes_3d: Axes3dReport,
    },
    Plot {
        plot: PlotReport,
    },
    BarPlot {
        bar_plot: bar_plot::BarPlotReport,
    },
    Table {
        table: table::TableReport,
    },
    Dot {
        dot: DotReport,
    },
    Arrow {
        arrow: ArrowReport,
    },
    Tube {
        tube: TubeReport,
    },
    Pattern {
        pattern: PatternReport,
    },
    Ends {
        ends: ends::EndsReport,
    },
    Trail {
        trail: trail::TrailReport,
    },
    Readout {
        glyphs: usize,
        readout: readout::ReadoutReport,
    },
}

/// A world point `p` of the xy plane lands at `matrix · p + shift`, with `matrix` row by row as a
/// scene file writes one.
#[derive(Debug, Serialize)]
struct PlaneMapReport {
    matrix: [[f32; 2]; 2],
    shift: Vec2,
    determinant: f32,
}

/// Where a dot's center is in the world, plane map included, and the pixel it lands on.
#[derive(Debug, Serialize)]
struct DotReport {
    center: Vec3,
    center_pixels: Option<Vec2>,
}

/// The two ends of an arrow a linear map names, in the world as its plane map carries them, and
/// the pixels they land on. The frame draws the arrow between them.
#[derive(Debug, Serialize)]
struct ArrowReport {
    from: Vec3,
    to: Vec3,
    from_pixels: Option<Vec2>,
    to_pixels: Option<Vec2>,
}

/// The world points where a tube's curve starts and ends, at the low and high end of its
/// `u_range` or at its first and last point, and the pixels they land on. They are the whole curve's ends, even while a create
/// is drawing it.
#[derive(Debug, Serialize)]
struct TubeReport {
    start: Vec3,
    end: Vec3,
    start_pixels: Option<Vec2>,
    end_pixels: Option<Vec2>,
    /// The world point of each curve point of a curve written through points or as an atom
    /// trace, in order: during a morph of the tube, the mixed curve points. Left out for a curve
    /// of expressions.
    #[serde(skip_serializing_if = "Option::is_none")]
    curve_points: Option<Vec<Vec3>>,
}

/// The axes or 3D axes a data point is placed by, and its position in their data coordinates at
/// this scene time, as its `position` was declared. Left out while the axes are scaled flat.
#[derive(Debug, Serialize)]
struct DataPointReport {
    axes: ObjectId,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<Vec3>,
}

/// A pattern's copies at a scene time, in index order.
#[derive(Debug, Serialize)]
struct PatternReport {
    copy_count: usize,
    copies: Vec<CopyReport>,
}

/// One copy of a pattern: its index, its position in the world with the pixel it lands on, its
/// rotation in the pattern's own space, its own created share and opacity, its colors mixed by its
/// color share or its own, its glow share and glow color, and the box of the whole copy, as an
/// object's box is whole while it is created.
#[derive(Debug, Serialize)]
struct CopyReport {
    index: pattern::CopyIndex,
    position: Vec3,
    position_pixels: Option<Vec2>,
    rotation: Vec3,
    created: f32,
    opacity: f32,
    colors: ColorsReport,
    glow: f32,
    glow_color: String,
    bounding_box: Option<BoundingBox>,
}

/// What an attached object is attached to, and when that is a molecule, the atoms of it whose
/// balls the object's pixel box meets, each ball as the disc the frame shows.
fn attached_report(
    attachment: &scene::Attachment,
    stage: &scene::Stage,
    camera: &Camera,
    scene_config: &scene::SceneConfig,
    bounding_box: Option<&BoundingBox>,
) -> group::AttachedReport {
    let object = &stage.objects[&attachment.object];
    let covers_atoms = match &object.shape {
        Shape::Molecule {
            molecule: atoms,
            representation,
            ..
        } => {
            let mut covers_atoms = Vec::new();
            if let Some(BoundingBox {
                pixels: Some(caption),
                ..
            }) = bounding_box
            {
                let to_world = object.transform.object_to_world();
                let stretch = molecule::largest_stretch(to_world);
                for number in atoms.numbers() {
                    let atom = atoms.atom(number);
                    let world = to_world.transform_point3(atom.position);
                    let (Some(center), Some(radius)) = (
                        camera.to_pixels(world, scene_config),
                        representation.ball_radius(atom),
                    ) else {
                        continue;
                    };
                    let radius = radius * stretch * camera.pixels_per_unit(world, scene_config);
                    if molecule::ball_meets_box(center, radius, caption.min, caption.max) {
                        covers_atoms.push(number);
                    }
                }
            }
            Some(covers_atoms)
        }
        _ => None,
    };
    group::AttachedReport {
        object: attachment.object.clone(),
        group: attachment.group.clone(),
        covers_atoms,
    }
}

#[derive(Debug, Serialize)]
struct MorphReport {
    into: ObjectId,
    phase: Phase,
    progress: f32,
}

/// Each object's transform, opacity, created share, glyph count or atoms and bonds, and bounding
/// box at a scene time, and the camera. The box is the whole object's, as if every create had finished, so an
/// agent can lay out an object before and while it is created, as it can one at opacity 0.
fn inspect(scene: &Scene, scene_time: SceneTime) -> Inspection {
    let stage = scene.at(scene_time);
    let mut whole = stage.clone();
    whole
        .objects
        .values_mut()
        .for_each(|object| object.created = UnitInterval::one());
    let draw_list = mesh::draw_list(&whole, &scene.scene_config);
    let shown_texts: Vec<plot::ShownText> = draw_list
        .objects
        .iter()
        .flat_map(|object| match &object.source {
            Source::Object(object_id) => match &stage.objects[object_id] {
                staged @ scene::Object {
                    shape: Shape::Glyphs { glyphs, .. },
                    ..
                } if staged.opacity.get() > 0.0 && staged.created.get() > 0.0 => {
                    vec![plot::ShownText {
                        object_id: object_id.to_string(),
                        glyphs,
                        object_to_world: staged.flat_to_world().to_world(),
                    }]
                }
                staged @ scene::Object {
                    shape: Shape::Table(table),
                    ..
                } if staged.opacity.get() > 0.0 && staged.created.get() > 0.0 => {
                    let (_, cell_share) = table.written_shares(staged.created.get());
                    let shares: Vec<f32> =
                        mesh::paced_shares(table.glyphs.len(), cell_share).collect();
                    table
                        .cells
                        .iter()
                        .filter(|cell| shares[cell.glyphs.clone()].iter().any(|&s| s > 0.0))
                        .map(|cell| plot::ShownText {
                            object_id: format!(
                                "{object_id}/{}/{}",
                                cell.address.row, cell.address.column
                            ),
                            glyphs: &table.glyphs[cell.glyphs.clone()],
                            object_to_world: staged.flat_to_world().to_world(),
                        })
                        .collect()
                }
                _ => Vec::new(),
            },
            Source::Reaction(_) => Vec::new(),
        })
        .collect();
    let objects = draw_list
        .objects
        .iter()
        .filter_map(|object| match &object.source {
            Source::Object(object_id) => Some((object_id, object)),
            Source::Reaction(_) => None,
        })
        .map(|(object_id, object)| {
            let (bounding_box, in_frame) =
                object.bounding_box(&draw_list.camera, &scene.scene_config);
            let staged = &stage.objects[object_id];
            let to_pixels = |point| draw_list.camera.to_pixels(point, &scene.scene_config);
            let to_world = staged.flat_to_world();
            let object_to_world = to_world.to_world();
            // Mid-morph a text source's shape is the mixed path, but it still has its declared glyphs.
            let shape = match &scene.start.objects[object_id].shape {
                declared @ Shape::Glyphs { .. } => declared,
                _ => &staged.shape,
            };
            let plot_detail = |path, stroke: &Option<scene::Stroke>| match &staged.placed_by {
                scene::PlacedBy::Axes(axes_id) => match &stage.objects[axes_id].shape {
                    Shape::Axes(axes) => {
                        let stroke_width = stroke.map_or(0.0, |stroke| stroke.width.get());
                        let on_frame = plot::OnFrame {
                            plot_to_world: object_to_world,
                            camera: draw_list.camera,
                            scene_config: &scene.scene_config,
                        };
                        let shows = staged.opacity.get() > 0.0 && staged.created.get() > 0.0;
                        let texts = if shows { shown_texts.as_slice() } else { &[] };
                        Some(ShapeDetail::Plot {
                            plot: plot::report(
                                axes_id,
                                axes,
                                path,
                                stroke_width,
                                to_world.matrix(),
                                &on_frame,
                                texts,
                            ),
                        })
                    }
                    _ => None,
                },
                _ => None,
            };
            let ends = match &staged.placed_by {
                scene::PlacedBy::Ends(ends) => Some(ends),
                _ => None,
            };
            let detail = if let Some(ends) = ends {
                let (from, to) = (
                    ends.from.world(&stage.objects),
                    ends.to.world(&stage.objects),
                );
                Some(ShapeDetail::Ends {
                    ends: ends::EndsReport {
                        from,
                        to,
                        from_pixels: to_pixels(from),
                        to_pixels: to_pixels(to),
                    },
                })
            } else {
                match shape {
                    // `shape` is the declared text; a readout shows the glyphs of its number now.
                    Shape::Glyphs { glyphs, .. } => {
                        match (&staged.readout, stage.values.get(object_id), &staged.shape) {
                            (Some(readout), Some(value), Shape::Glyphs { glyphs: shown, .. }) => {
                                Some(ShapeDetail::Readout {
                                    glyphs: shown.len(),
                                    readout: readout::ReadoutReport {
                                        value: *value,
                                        text: readout.text(*value),
                                    },
                                })
                            }
                            _ => Some(ShapeDetail::Glyphs {
                                glyphs: glyphs.len(),
                            }),
                        }
                    }
                    Shape::Trail(trail) => {
                        let newest = trail.points.last().copied();
                        Some(ShapeDetail::Trail {
                            trail: trail::TrailReport {
                                point_count: trail.points.len(),
                                newest,
                                newest_pixels: newest.and_then(to_pixels),
                            },
                        })
                    }
                    Shape::Molecule {
                        molecule,
                        representation,
                        groups,
                        glows,
                    } => {
                        let mut report =
                            representation.report(molecule, object_to_world, to_pixels, |point| {
                                draw_list.camera.pixels_per_unit(point, &scene.scene_config)
                            });
                        for (group_id, group) in groups.iter() {
                            let glow = glows
                                .get(group_id)
                                .copied()
                                .unwrap_or_else(group::Glow::off);
                            let shell = mesh::with_meshes(
                                object,
                                mesh::shell_meshes(
                                    &representation.glow(molecule, group, glow.color),
                                    glow.color,
                                ),
                            );
                            let center = object_to_world.transform_point3(group.center);
                            let entry = group::GroupReport {
                                atoms: group.atoms.clone(),
                                center,
                                center_pixels: to_pixels(center),
                                bounding_box: shell
                                    .bounding_box(&draw_list.camera, &scene.scene_config)
                                    .0,
                                glow: glow.share.get(),
                                color: glow.color.hex(),
                            };
                            report.groups.insert(group_id.clone(), entry);
                        }
                        Some(ShapeDetail::Molecule(report))
                    }
                    Shape::BarPlot(bars) => match &staged.placed_by {
                        scene::PlacedBy::Axes(axes_id) => Some(ShapeDetail::BarPlot {
                            bar_plot: bar_plot::report(
                                axes_id,
                                bars,
                                staged.created.get(),
                                object_to_world,
                                to_pixels,
                            ),
                        }),
                        _ => None,
                    },
                    Shape::Table(table) => {
                        let (_, cell_share) = table.written_shares(staged.created.get());
                        let shares: Vec<f32> =
                            mesh::paced_shares(table.glyphs.len(), cell_share).collect();
                        let cells = table
                            .cells
                            .iter()
                            .map(|cell| {
                                let written = &shares[cell.glyphs.clone()];
                                let created = if written.is_empty() {
                                    shares.get(cell.glyphs.start).copied().unwrap_or(cell_share)
                                } else {
                                    written.iter().sum::<f32>() / written.len() as f32
                                };
                                let glow = table
                                    .glows
                                    .get(&cell.address)
                                    .copied()
                                    .unwrap_or_else(group::Glow::off);
                                let (text, tex) = match &cell.content {
                                    table::CellContent::Empty => (None, None),
                                    table::CellContent::Text(text) => (Some(text.clone()), None),
                                    table::CellContent::Math(tex) => (None, Some(tex.clone())),
                                };
                                let glyphs = &table.glyphs[cell.glyphs.clone()];
                                table::CellReport {
                                    cell: cell.address,
                                    text,
                                    tex,
                                    created,
                                    glow: glow.share.get(),
                                    glow_color: glow.color.hex(),
                                    rectangle: table::rectangle_report(
                                        cell.rectangle,
                                        object_to_world,
                                        to_pixels,
                                    ),
                                    bounding_box: mesh::with_meshes(
                                        object,
                                        mesh::cell_meshes(table, cell, to_world),
                                    )
                                    .bounding_box(&draw_list.camera, &scene.scene_config)
                                    .0,
                                    colors: (!glyphs.is_empty())
                                        .then(|| ColorsReport::of_glyphs(glyphs)),
                                }
                            })
                            .collect();
                        Some(ShapeDetail::Table {
                            table: table::TableReport {
                                size: table.size.get(),
                                rows: table.rows,
                                columns: table.columns,
                                cells,
                            },
                        })
                    }
                    Shape::Axes(axes) => Some(ShapeDetail::Axes {
                        axes: axes.report(object_to_world, to_pixels),
                    }),
                    Shape::NumberLine(line) => Some(ShapeDetail::NumberLine {
                        number_line: line.report(object_to_world, to_pixels),
                    }),
                    Shape::Axes3d(axes) => Some(ShapeDetail::Axes3d {
                        axes_3d: axes.report(
                            &axes_3d::Facing::new(
                                staged.transform.object_to_world(),
                                &draw_list.camera,
                            ),
                            to_pixels,
                        ),
                    }),
                    Shape::Flat { path, stroke, .. } => match path.under_map() {
                        UnderMap::CenterOnly => {
                            let center = to_world.position();
                            Some(ShapeDetail::Dot {
                                dot: DotReport {
                                    center,
                                    center_pixels: to_pixels(center),
                                },
                            })
                        }
                        UnderMap::FromEnds(ends) if staged.plane_map.is_some() => {
                            let (from, to) = (
                                object_to_world.transform_point3(ends.from.extend(0.0)),
                                object_to_world.transform_point3(ends.to.extend(0.0)),
                            );
                            Some(ShapeDetail::Arrow {
                                arrow: ArrowReport {
                                    from,
                                    to,
                                    from_pixels: to_pixels(from),
                                    to_pixels: to_pixels(to),
                                },
                            })
                        }
                        _ => plot_detail(path, stroke),
                    },
                    // A plot's mix is its one path.
                    Shape::Mix(paths) => match paths.as_slice() {
                        [only] => plot_detail(&only.path, &only.stroke),
                        _ => None,
                    },
                    Shape::Solid {
                        solid: scene::Solid::Tube { curve, .. },
                        ..
                    } => {
                        let object_to_world = staged.transform.object_to_world();
                        let (start, end) = (
                            object_to_world.transform_point3(curve.start()),
                            object_to_world.transform_point3(curve.end()),
                        );
                        Some(ShapeDetail::Tube {
                            tube: TubeReport {
                                start,
                                end,
                                start_pixels: to_pixels(start),
                                end_pixels: to_pixels(end),
                                curve_points: curve.curve_points().map(|points| {
                                    points
                                        .iter()
                                        .map(|point| {
                                            object_to_world.transform_point3(point.as_vec3())
                                        })
                                        .collect()
                                }),
                            },
                        })
                    }
                    Shape::Sphere { .. } | Shape::Solid { .. } => None,
                    Shape::Pattern(pattern) => {
                        let whole = mesh::copy_meshes(pattern, 1.0);
                        let copies = pattern
                            .copies
                            .iter()
                            .zip(whole)
                            .zip(mesh::paced_shares(
                                pattern.copies.len(),
                                staged.created.get(),
                            ))
                            .map(|((copy, meshes), created)| {
                                let position =
                                    object_to_world.transform_point3(copy.transform.position);
                                let bounding_box = mesh::with_meshes(object, meshes)
                                    .bounding_box(&draw_list.camera, &scene.scene_config)
                                    .0;
                                CopyReport {
                                    index: copy.index,
                                    position,
                                    position_pixels: to_pixels(position),
                                    rotation: copy.transform.rotation,
                                    created,
                                    opacity: copy.opacity,
                                    colors: ColorsReport::of(&pattern.copy_shape(copy)),
                                    glow: copy.glow.share.get(),
                                    glow_color: copy.glow.color.hex(),
                                    bounding_box,
                                }
                            })
                            .collect();
                        Some(ShapeDetail::Pattern {
                            pattern: PatternReport {
                                copy_count: pattern.copies.len(),
                                copies,
                            },
                        })
                    }
                }
            };
            let data_point = match &staged.placed_by {
                scene::PlacedBy::DataPoint(axes) => {
                    let position = data_point::DataToWorld::of(&stage.objects[axes])
                        .data(staged.transform.position);
                    Some(DataPointReport {
                        axes: axes.clone(),
                        position,
                    })
                }
                _ => None,
            };
            let morph = stage.morphs.get(object_id).map(|m| MorphReport {
                into: m.target.clone(),
                phase: m.phase,
                progress: m.progress,
            });
            let attached = match &staged.placed_by {
                scene::PlacedBy::Attachment(a) => Some(attached_report(
                    a,
                    &stage,
                    &draw_list.camera,
                    &scene.scene_config,
                    bounding_box.as_ref(),
                )),
                _ => None,
            };
            let plane_map = staged.plane_map.map(|map| PlaneMapReport {
                matrix: map.rows(),
                shift: map.shift(),
                determinant: map.determinant(),
            });
            let inspected = InspectedObject {
                transform: object.transform,
                opacity: object.opacity,
                created: staged.created.get(),
                colors: ColorsReport::of(&staged.shape),
                detail,
                data_point,
                morph,
                attached,
                plane_map,
                bounding_box,
                in_frame,
            };
            (object_id.clone(), inspected)
        })
        .collect();
    let reactions = stage
        .reactions
        .iter()
        .enumerate()
        .map(|(k, reaction)| {
            let (bounding_box, in_frame) = mesh::staged_reaction(k, reaction)
                .bounding_box(&draw_list.camera, &scene.scene_config);
            let (atoms, bonds) = reaction.blend.report(
                |point| draw_list.camera.to_pixels(point, &scene.scene_config),
                |point| draw_list.camera.pixels_per_unit(point, &scene.scene_config),
            );
            let glows = reaction.glows.iter().map(|glow| glow.report()).collect();
            ReactionReport {
                phase: reaction.phase,
                fit_rmsd: reaction.fit_rmsd,
                atoms,
                bonds,
                glows,
                bounding_box,
                in_frame,
            }
        })
        .collect();
    Inspection {
        camera: draw_list.camera,
        objects,
        reactions,
    }
}
