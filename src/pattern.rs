//! A pattern: one shape drawn at every copy of a layout, each copy with its own transform, color
//! share and color to mix toward, from expressions in its index and place. Every copy is
//! evaluated once, when the scene file is read, so a stage holds only numbers.

use std::fmt;
use std::sync::Arc;

use glam::Vec3;
use serde::Serialize;

use crate::expr::Expression;
use crate::follow::{self, Faces};
use crate::group::Glow;
use crate::member::OwnColors;
use crate::scene::{Color, Shape, Transform};
use crate::space_curve::{CurveFrame, SpaceCurve};

/// The most copies a pattern holds.
pub const MAX_COPIES: u64 = 10_000;

/// What `Shape::Pattern` holds. Shared parts in `Arc`, because a stage is cloned every frame.
#[derive(Clone, Debug)]
pub struct Pattern {
    /// The shape every copy draws, in its own space: `Shape::Flat`, `Shape::Sphere` or
    /// `Shape::Solid`, never another. It holds the pattern's colors, which a recolor changes.
    pub shape: Box<Shape>,
    /// In index order.
    pub copies: Arc<[Copy]>,
}

/// One place where a pattern draws its shape.
#[derive(Clone, Debug)]
pub struct Copy {
    pub index: CopyIndex,
    /// In the pattern's own space.
    pub transform: Transform,
    /// How far the copy's colors are mixed toward its `to_color`, from 0 to 1.
    pub color_share: f32,
    /// The pattern's `to_color`, or the color of its palette that the copy's palette index picks.
    pub to_color: Color,
    /// The copy's own opacity, from 0 to 1, which a fade of the copy changes. The pattern's
    /// object opacity applies on top of it.
    pub opacity: f32,
    /// The colors a recolor of the copy gave it, drawn in place of its mixed colors.
    pub own_colors: OwnColors,
    pub glow: Glow,
}

/// What each copy's colors are mixed toward by its color share.
pub enum ToColor {
    /// One color for every copy.
    One(Color),
    /// The color of `palette` that each copy's palette index picks.
    Palette {
        palette: Vec<Color>,
        index: PaletteIndex,
    },
}

/// Which color of a palette each copy picks, counting from 0.
pub enum PaletteIndex {
    /// An expression read under the layout's variables, as a `CopyExpressions` is.
    Expression(Expression),
    /// One index a copy, in index order, each within the palette: the scene file is checked for
    /// that when it is read.
    Listed(Vec<u32>),
}

/// The field of a pattern's palette index, under the object.
pub const PALETTE_INDEX_FIELD: &str = "copy/palette_index";

/// A grid's copy is numbered along x, y and z, and a ring's or an along's by one number, each
/// from 0. It serializes as `[i, j, k]` or `[i]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(untagged)]
pub enum CopyIndex {
    Grid([u32; 3]),
    Line([u32; 1]),
}

impl fmt::Display for CopyIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CopyIndex::Grid([i, j, k]) => write!(f, "[{i}, {j}, {k}]"),
            CopyIndex::Line([i]) => write!(f, "[{i}]"),
        }
    }
}

/// Where a pattern's copies go, in the pattern's own space.
pub enum Layout {
    /// `counts` copies along x, y and z, `spacing` apart, centered on the pattern's position.
    Grid { counts: [u32; 3], spacing: f32 },
    /// `count` copies around a circle of `radius` in the pattern's xy plane, the first at
    /// `start_angle` degrees counterclockwise from +x.
    Ring {
        count: u32,
        radius: f32,
        start_angle: f32,
    },
    /// `count` copies along `curve`, evenly spaced by length, each facing its tangent when
    /// `faces` says so.
    Along {
        count: u32,
        curve: SpaceCurve,
        faces: Option<Faces>,
    },
}

impl Layout {
    pub fn copy_count(&self) -> usize {
        match self {
            Layout::Grid { counts, .. } => counts.iter().map(|&n| n as usize).product(),
            Layout::Ring { count, .. } | Layout::Along { count, .. } => *count as usize,
        }
    }

    /// Each copy's index and place, in index order: a grid's `i` fastest, then `j`, then `k`, and
    /// the curve's frame at the place for a copy that faces its tangent. The values a copy's
    /// expressions read are the index and place, in the order of `expr::GRID_COPY` or
    /// `expr::COPY`. Copies along an open curve run from its start to its end, and along a
    /// closed one stop one spacing short of the start, so no two meet.
    fn places(&self) -> Vec<(CopyIndex, Vec3, Option<CurveFrame>)> {
        match self {
            Layout::Grid {
                counts: [nx, ny, nz],
                spacing,
            } => {
                let middle = |n: u32| (n as f32 - 1.0) / 2.0;
                let mut places = Vec::new();
                for k in 0..*nz {
                    for j in 0..*ny {
                        for i in 0..*nx {
                            let place = Vec3::new(
                                i as f32 - middle(*nx),
                                j as f32 - middle(*ny),
                                k as f32 - middle(*nz),
                            ) * *spacing;
                            places.push((CopyIndex::Grid([i, j, k]), place, None));
                        }
                    }
                }
                places
            }
            Layout::Ring {
                count,
                radius,
                start_angle,
            } => (0..*count)
                .map(|i| {
                    let (sin, cos) = (start_angle + 360.0 * i as f32 / *count as f32)
                        .to_radians()
                        .sin_cos();
                    (
                        CopyIndex::Line([i]),
                        Vec3::new(radius * cos, radius * sin, 0.0),
                        None,
                    )
                })
                .collect(),
            Layout::Along {
                count,
                curve,
                faces,
            } => {
                let gaps = if curve.is_closed() {
                    *count
                } else {
                    (*count - 1).max(1)
                };
                (0..*count)
                    .map(|i| {
                        let frame = curve.frame_at(curve.length() * i as f32 / gaps as f32);
                        (
                            CopyIndex::Line([i]),
                            frame.point,
                            faces.map(|Faces::Tangent| frame),
                        )
                    })
                    .collect()
            }
        }
    }
}

/// The field of each value a pattern's `copy` sets, under the object, in the order of a
/// `CopyExpressions`.
pub const COPY_FIELDS: [&str; 10] = [
    "copy/position/x",
    "copy/position/y",
    "copy/position/z",
    "copy/rotation/x",
    "copy/rotation/y",
    "copy/rotation/z",
    "copy/scale/x",
    "copy/scale/y",
    "copy/scale/z",
    "copy/color_share",
];

/// The expression of each value of `COPY_FIELDS` that a pattern's `copy` writes, read under its
/// layout's variables. A value with none takes its default: the place for a position, 0 for a
/// rotation, 1 for a scale, and for a color share 1 with a palette and 0 without.
pub struct CopyExpressions(pub [Option<Expression>; 10]);

/// An expression of `copy` with no value a copy can take, at the first such copy in index order:
/// no finite value, or for the palette index no whole number within the palette.
#[derive(Debug)]
pub struct CopyFault {
    /// One of `COPY_FIELDS`, or `PALETTE_INDEX_FIELD`.
    pub field: &'static str,
    pub index: CopyIndex,
    pub place: Vec3,
    pub value: f64,
}

impl Pattern {
    /// The pattern of `shape` at every copy of `layout`, each copy's values evaluated from
    /// `expressions` and its color to mix toward from `to_color`, or the first copy at which each
    /// expression has no value the copy can take.
    pub fn new(
        shape: Shape,
        layout: &Layout,
        expressions: &CopyExpressions,
        to_color: &ToColor,
    ) -> Result<Pattern, Vec<CopyFault>> {
        let mut faults: Vec<CopyFault> = Vec::new();
        let mut fault = |field: &'static str, index, place, value| {
            if !faults.iter().any(|f| f.field == field) {
                faults.push(CopyFault {
                    field,
                    index,
                    place,
                    value,
                });
            }
        };
        let default_share = match to_color {
            ToColor::One(_) => 0.0,
            ToColor::Palette { .. } => 1.0,
        };
        let mut copies = Vec::new();
        for (ordinal, (index, place, facing)) in layout.places().into_iter().enumerate() {
            let values: Vec<f64> = match index {
                CopyIndex::Grid([i, j, k]) => vec![
                    i.into(),
                    j.into(),
                    k.into(),
                    place.x.into(),
                    place.y.into(),
                    place.z.into(),
                ],
                CopyIndex::Line([i]) => {
                    vec![i.into(), place.x.into(), place.y.into(), place.z.into()]
                }
            };
            let mut set = [
                place.x,
                place.y,
                place.z,
                0.0,
                0.0,
                0.0,
                1.0,
                1.0,
                1.0,
                default_share,
            ];
            for (slot, expression) in expressions.0.iter().enumerate() {
                let Some(expression) = expression else {
                    continue;
                };
                let value = expression.eval(&values);
                if (value as f32).is_finite() {
                    set[slot] = value as f32;
                } else {
                    fault(COPY_FIELDS[slot], index, place, value);
                }
            }
            let copy_to_color = match to_color {
                ToColor::One(color) => *color,
                ToColor::Palette {
                    palette,
                    index: PaletteIndex::Listed(listed),
                } => palette[listed[ordinal] as usize],
                ToColor::Palette {
                    palette,
                    index: PaletteIndex::Expression(expression),
                } => {
                    let value = expression.eval(&values);
                    let whole = value.round();
                    // A sum of fractions such as `3*(i/3)` lands a rounding error off its whole.
                    if (value - whole).abs() <= 1e-6 && (0.0..palette.len() as f64).contains(&whole)
                    {
                        palette[whole as usize]
                    } else {
                        fault(PALETTE_INDEX_FIELD, index, place, value);
                        palette[0]
                    }
                }
            };
            let own = Vec3::new(set[3], set[4], set[5]);
            let transform = Transform {
                position: Vec3::new(set[0], set[1], set[2]),
                rotation: facing.map_or(own, |frame| follow::faced_copy(&frame, own)),
                scale: Vec3::new(set[6], set[7], set[8]),
            };
            copies.push(Copy {
                index,
                transform,
                color_share: set[9].clamp(0.0, 1.0),
                to_color: copy_to_color,
                opacity: 1.0,
                own_colors: OwnColors::default(),
                glow: Glow::off(),
            });
        }
        if !faults.is_empty() {
            faults.sort_by_key(|f| COPY_FIELDS.iter().position(|field| *field == f.field));
            return Err(faults);
        }
        Ok(Pattern {
            shape: Box::new(shape),
            copies: copies.into(),
        })
    }

    /// How far the farthest point of any copy lies from the pattern's position, in its own space.
    pub fn reach(&self) -> f32 {
        let shape = self.shape.reach();
        self.copies
            .iter()
            .map(|copy| {
                copy.transform.position.length() + shape * copy.transform.scale.abs().max_element()
            })
            .fold(0.0, f32::max)
    }

    /// Where the copy of `index` sits in `copies`, if the pattern has it.
    pub fn ordinal(&self, index: CopyIndex) -> Option<usize> {
        self.copies.iter().position(|copy| copy.index == index)
    }

    /// The shape `copy` draws: the pattern's shape with each of its colors mixed toward
    /// the copy's `to_color` by its color share, or the copy's own color in its place.
    pub fn copy_shape(&self, copy: &Copy) -> Shape {
        let OwnColors {
            stroke: own_stroke,
            fill: own_fill,
            color: own_color,
        } = copy.own_colors;
        let mix = |color: &mut Color, own: Option<Color>| {
            *color = own.unwrap_or(color.mix(copy.to_color, copy.color_share));
        };
        let mut shape = (*self.shape).clone();
        match &mut shape {
            Shape::Flat { stroke, fill, .. } => {
                stroke
                    .iter_mut()
                    .for_each(|stroke| mix(&mut stroke.color, own_stroke));
                fill.iter_mut()
                    .for_each(|fill| mix(&mut fill.color, own_fill));
            }
            Shape::Sphere {
                material: crate::scene::Material::Standard { color, .. },
                ..
            }
            | Shape::Solid {
                material: crate::scene::Material::Standard { color, .. },
                ..
            } => mix(color, own_color),
            Shape::Glyphs { .. }
            | Shape::Mix(_)
            | Shape::Molecule { .. }
            | Shape::Axes(_)
            | Shape::BarPlot(_)
            | Shape::Table(_)
            | Shape::NumberLine(_)
            | Shape::Axes3d(_)
            | Shape::Pattern(_)
            | Shape::Trail(_) => {
                unreachable!("`scene_file` reads a pattern's shape as a flat shape or a solid")
            }
        }
        shape
    }
}
