//! Matching two shapes so one can be mixed into the other, point by point.
//!
//! A path has any number of parts, each any number of pieces, and two paths almost never agree.
//! `MorphPlan::new` makes them agree once, when the scene file is read: it breaks each path into
//! cubic pieces, pairs the glyphs of two text or math objects by the characters they show, pairs
//! the parts, gives each pair the same number of pieces by splitting the longest, turns a closed
//! part to start where its partner starts, and keeps the result. Two tubes through as many curve
//! points need no matching: curve point k goes to curve point k.
//! `MorphPlan::at` is then a pure function of the progress: it mixes every control point and
//! each group's stroke and fill, or every curve point, the radius and the material, and returns
//! the mix to draw.
//!
//! Only kurbo is used for paths here, so the module is tested without a GPU.

use std::sync::Arc;

use glam::DVec3;
use kurbo::{
    BezPath, CubicBez, ParamCurve, ParamCurveArclen, ParamCurveArea, ParamCurveExtrema, PathEl,
    Point, Rect,
};

use crate::path::Path;
use crate::scene::{
    EdgeShade, Fill, Material, MixedPath, Positive, Shape, Solid, Stroke, UnitInterval,
};
use crate::space_curve::SpaceCurve;

/// What a morph reads of a shape: a flat shape's paths, or a tube's curve points.
pub enum MorphEnd {
    /// The paths of a flat shape, in the order a write draws them, each group with how it is
    /// stroked and filled. Text and math give one group per glyph, filled in the glyph's color,
    /// and every other flat shape gives one group, so a morph pairs glyph with glyph first and
    /// part with part within a glyph.
    Paths { groups: Vec<Group> },
    /// A tube whose curve is written through curve points, not on 3D axes.
    Curve(CurveEnd),
}

pub struct Group {
    parts: Vec<Part>,
    paint: Paint,
    /// The characters a glyph shows, and `None` for a shape that is not text or math.
    characters: Option<String>,
}

/// A tube as a morph reads it: its curve points in its own space, as written, and how it looks.
#[derive(Clone, Debug)]
pub struct CurveEnd {
    points: Vec<DVec3>,
    radius: Positive,
    material: Material,
    edges: EdgeShade,
}

impl MorphEnd {
    /// `None` for a shape a morph cannot reshape: a sphere, a prism, a revolution, a surface, a
    /// tube whose curve is expressions or that stands on 3D axes, a molecule, axes, a bar plot, a
    /// number line, 3D axes, a pattern, and a mix, which is a morph's own.
    pub fn of(shape: &Shape) -> Option<MorphEnd> {
        match shape {
            Shape::Flat { path, stroke, fill } => Some(MorphEnd::Paths {
                groups: vec![Group {
                    parts: parts(path),
                    paint: Paint {
                        stroke: *stroke,
                        fill: *fill,
                    },
                    characters: None,
                }],
            }),
            Shape::Glyphs { glyphs, .. } => Some(MorphEnd::Paths {
                groups: glyphs
                    .iter()
                    .map(|glyph| Group {
                        parts: parts(&glyph.path),
                        paint: Paint {
                            stroke: None,
                            fill: Some(Fill::solid(glyph.color)),
                        },
                        characters: Some(glyph.characters.clone()),
                    })
                    .collect(),
            }),
            Shape::Solid {
                solid:
                    Solid::Tube {
                        curve,
                        radius,
                        cut: None,
                    },
                material,
                edges,
            } => curve.curve_points().map(|points| {
                MorphEnd::Curve(CurveEnd {
                    points: points.to_vec(),
                    radius: *radius,
                    material: *material,
                    edges: *edges,
                })
            }),
            Shape::Sphere { .. }
            | Shape::Solid { .. }
            | Shape::Molecule { .. }
            | Shape::Axes(_)
            | Shape::BarPlot(_)
            | Shape::Table(_)
            | Shape::NumberLine(_)
            | Shape::Axes3d(_)
            | Shape::Mix(_)
            | Shape::Pattern(_)
            | Shape::Trail(_) => None,
        }
    }
}

/// One part of a path as cubic Béziers, each starting where the one before ended. A closed part
/// ends where it starts. Lines and quadratics are raised to cubics so every piece has one form.
#[derive(Clone, Debug)]
struct Part {
    closed: bool,
    cubics: Vec<CubicBez>,
}

/// A morph from one `MorphEnd` to another, matched once. Cheap to clone the way a track needs: it is
/// shared behind an `Arc` by the timeline.
#[derive(Debug)]
pub enum MorphPlan {
    Paths {
        groups: Vec<GroupPair>,
    },
    /// Two tubes through as many curve points, curve point k paired with curve point k.
    Curve {
        from: CurveEnd,
        to: CurveEnd,
    },
}

/// One group of the source and its partner in the target, with the paint each side gives it. A
/// group with no partner keeps its own paint at both ends, so it grows or shrinks in its color.
#[derive(Debug)]
pub struct GroupPair {
    pairs: Vec<Pair>,
    from: Paint,
    to: Paint,
}

#[derive(Debug)]
struct Pair {
    from: Vec<CubicBez>,
    to: Vec<CubicBez>,
    closed: (bool, bool),
}

#[derive(Debug, Clone, Copy)]
struct Paint {
    stroke: Option<Stroke>,
    fill: Option<Fill>,
}

impl MorphPlan {
    /// Between two text or math objects, glyphs that show the same characters pair, as many as
    /// keep their order, and a glyph with no partner fades out or in where it stands, so no glyph
    /// is bent into another. Any other two flat shapes pair their groups in order, and a group
    /// left over grows or shrinks. Two tubes pair their curve points in order, which `scene_file`
    /// has checked are as many.
    pub fn new(from: &MorphEnd, to: &MorphEnd) -> MorphPlan {
        let (from, to) = match (from, to) {
            (MorphEnd::Paths { groups: from }, MorphEnd::Paths { groups: to }) => (from, to),
            (MorphEnd::Curve(from), MorphEnd::Curve(to)) => {
                return MorphPlan::Curve {
                    from: from.clone(),
                    to: to.clone(),
                };
            }
            _ => unreachable!("`scene_file` morphs a tube only into a tube"),
        };
        let both_glyphs = from
            .iter()
            .chain(to)
            .all(|group| group.characters.is_some());
        if both_glyphs {
            return MorphPlan::Paths {
                groups: by_characters(from, to),
            };
        }
        let groups = (0..from.len().max(to.len()))
            .map(|k| match (from.get(k), to.get(k)) {
                (Some(a), Some(b)) => GroupPair {
                    pairs: pair_parts(&a.parts, &b.parts, equalize),
                    from: a.paint,
                    to: b.paint,
                },
                (Some(a), None) => GroupPair {
                    pairs: pair_parts(&a.parts, &collapsed(&a.parts), equalize),
                    from: a.paint,
                    to: a.paint,
                },
                (None, Some(b)) => GroupPair {
                    pairs: pair_parts(&collapsed(&b.parts), &b.parts, equalize),
                    from: b.paint,
                    to: b.paint,
                },
                (None, None) => unreachable!("k is below the longer list"),
            })
            .collect();
        MorphPlan::Paths { groups }
    }

    /// A morph between two plots on the same axes. Each pair of parts is sampled at the x values
    /// of both, so the mix moves every x straight up or down and adds no corner of its own.
    pub fn between_plots(from: &MorphEnd, to: &MorphEnd) -> MorphPlan {
        let (MorphEnd::Paths { groups: from }, MorphEnd::Paths { groups: to }) = (from, to) else {
            unreachable!("a plot's morph end is its paths")
        };
        let (a, b) = (&from[0], &to[0]);
        let pairs = pair_parts(&a.parts, &b.parts, |a, b| {
            along_x(a, b).unwrap_or_else(|| equalize(a, b))
        });
        MorphPlan::Paths {
            groups: vec![GroupPair {
                pairs,
                from: a.paint,
                to: b.paint,
            }],
        }
    }

    #[cfg(test)]
    fn groups(&self) -> &[GroupPair] {
        match self {
            MorphPlan::Paths { groups } => groups,
            MorphPlan::Curve { .. } => &[],
        }
    }

    /// The number of cubic pieces each side has after matching, over all parts.
    #[cfg(test)]
    pub fn pieces(&self) -> usize {
        self.groups()
            .iter()
            .flat_map(|group| &group.pairs)
            .map(|pair| pair.from.len())
            .sum()
    }

    /// The number of matched parts.
    #[cfg(test)]
    pub fn parts(&self) -> usize {
        self.groups().iter().map(|group| group.pairs.len()).sum()
    }

    /// The mix `progress` of the way from the first look to the second: for paths, one path per
    /// group, each in its own mixed stroke and fill; for tubes, the tube through each pair of
    /// curve points mixed, its curve made again through them. Points are mixed without limit, so
    /// an overshoot carries the shape past the second look. Colors, widths, radii and opacities
    /// stay between the two. A group whose every piece has collapsed to a point is left out, and
    /// `None` means every group was, or every mixed curve point is one point.
    pub fn at(&self, progress: f32) -> Option<Shape> {
        let mixed = progress.clamp(0.0, 1.0);
        let groups = match self {
            MorphPlan::Paths { groups } => groups,
            MorphPlan::Curve { from, to } => return mix_curves(from, to, progress),
        };
        let paths: Vec<MixedPath> = groups
            .iter()
            .filter_map(|group| {
                let path = mix_pairs(&group.pairs, progress)?;
                Some(MixedPath {
                    path,
                    stroke: mix_stroke(group.from.stroke, group.to.stroke, mixed),
                    fill: mix_fill(group.from.fill, group.to.fill, mixed),
                })
            })
            .collect();
        (!paths.is_empty()).then_some(Shape::Mix(paths))
    }
}

/// The tube `progress` of the way from `from` to `to`: each curve point mixed with its partner
/// and the curve made through them again, so it stays smooth at every progress. Edges take the
/// first look's shade for the first half and the second look's for the rest.
fn mix_curves(from: &CurveEnd, to: &CurveEnd, progress: f32) -> Option<Shape> {
    let weight = f64::from(progress);
    let points: Vec<[f64; 3]> = from
        .points
        .iter()
        .zip(&to.points)
        .map(|(a, b)| (*a + (*b - *a) * weight).to_array())
        .collect();
    let curve = SpaceCurve::smooth_through(&points, None).ok()?;
    let mixed = progress.clamp(0.0, 1.0);
    let radius = from.radius.get() + (to.radius.get() - from.radius.get()) * mixed;
    Some(Shape::Solid {
        solid: Solid::Tube {
            curve: Arc::new(curve),
            radius: Positive::try_from(f64::from(radius)).ok()?,
            cut: None,
        },
        material: mix_material(from.material, to.material, mixed),
        edges: if mixed < 0.5 { from.edges } else { to.edges },
    })
}

fn mix_material(from: Material, to: Material, weight: f32) -> Material {
    let (
        Material::Standard {
            color: a,
            roughness: a_roughness,
            metallic: a_metallic,
            opacity: a_opacity,
        },
        Material::Standard {
            color: b,
            roughness: b_roughness,
            metallic: b_metallic,
            opacity: b_opacity,
        },
    ) = (from, to);
    let share = |a: UnitInterval, b: UnitInterval| {
        UnitInterval::clamped(a.get() + (b.get() - a.get()) * weight)
    };
    Material::Standard {
        color: a.mix(b, weight),
        roughness: share(a_roughness, b_roughness),
        metallic: share(a_metallic, b_metallic),
        opacity: share(a_opacity, b_opacity),
    }
}

/// The path `progress` of the way between each pair's two sides. `None` if every piece has
/// collapsed to a point.
fn mix_pairs(pairs: &[Pair], progress: f32) -> Option<Path> {
    let weight = f64::from(progress);
    let mut bez = BezPath::new();
    for pair in pairs {
        let mut pieces = pair.from.iter().zip(&pair.to).map(|(a, b)| {
            CubicBez::new(
                mix_point(a.p0, b.p0, weight),
                mix_point(a.p1, b.p1, weight),
                mix_point(a.p2, b.p2, weight),
                mix_point(a.p3, b.p3, weight),
            )
        });
        let first = pieces.next().expect("a pair has pieces");
        bez.move_to(first.p0);
        bez.curve_to(first.p1, first.p2, first.p3);
        for piece in pieces {
            bez.curve_to(piece.p1, piece.p2, piece.p3);
        }
        if if progress < 0.5 {
            pair.closed.0
        } else {
            pair.closed.1
        } {
            bez.close_path();
        }
    }
    Path::from_bez(bez).ok()
}

fn mix_point(a: Point, b: Point, weight: f64) -> Point {
    a + (b - a) * weight
}

/// A dash pattern cannot be mixed, so the first look's holds for the first half and the second
/// look's for the rest, as a fill rule does.
fn mix_stroke(from: Option<Stroke>, to: Option<Stroke>, weight: f32) -> Option<Stroke> {
    let dash = match weight < 0.5 {
        true => from.or(to).and_then(|s| s.dash),
        false => to.or(from).and_then(|s| s.dash),
    };
    let (color, width) = match (from, to) {
        (None, None) => return None,
        (Some(a), None) => (a.color, a.width.get() * (1.0 - weight)),
        (None, Some(b)) => (b.color, b.width.get() * weight),
        (Some(a), Some(b)) => (
            a.color.mix(b.color, weight),
            a.width.get() + (b.width.get() - a.width.get()) * weight,
        ),
    };
    Positive::try_from(f64::from(width))
        .ok()
        .map(|width| Stroke { color, width, dash })
}

/// A fill that is absent on one side is that side's fill at opacity 0. The fill rule cannot be
/// mixed, so the first look's holds for the first half and the second look's for the rest.
fn mix_fill(from: Option<Fill>, to: Option<Fill>, weight: f32) -> Option<Fill> {
    let (color, opacity, rule) = match (from, to) {
        (None, None) => return None,
        (Some(a), None) => (a.color, a.opacity.get() * (1.0 - weight), a.rule),
        (None, Some(b)) => (b.color, b.opacity.get() * weight, b.rule),
        (Some(a), Some(b)) => (
            a.color.mix(b.color, weight),
            a.opacity.get() + (b.opacity.get() - a.opacity.get()) * weight,
            if weight < 0.5 { a.rule } else { b.rule },
        ),
    };
    Some(Fill {
        color,
        opacity: UnitInterval::clamped(opacity),
        rule,
    })
}

// ---- breaking a path into cubics -------------------------------------------------------------

fn parts(path: &Path) -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();
    let mut current = Part {
        closed: false,
        cubics: Vec::new(),
    };
    let mut at = Point::ZERO;
    let mut start = Point::ZERO;
    let finish = |part: &mut Part, closed: bool, parts: &mut Vec<Part>| {
        if !part.cubics.is_empty() {
            parts.push(Part {
                closed,
                cubics: std::mem::take(&mut part.cubics),
            });
        }
    };
    for element in path.elements() {
        match *element {
            PathEl::MoveTo(p) => {
                finish(&mut current, false, &mut parts);
                at = p;
                start = p;
            }
            PathEl::LineTo(p) => push(
                &mut current,
                CubicBez::new(at, at.lerp(p, 1.0 / 3.0), at.lerp(p, 2.0 / 3.0), p),
                &mut at,
            ),
            PathEl::QuadTo(c, p) => {
                push(&mut current, kurbo::QuadBez::new(at, c, p).raise(), &mut at)
            }
            PathEl::CurveTo(c1, c2, p) => push(&mut current, CubicBez::new(at, c1, c2, p), &mut at),
            PathEl::ClosePath => {
                push(
                    &mut current,
                    CubicBez::new(
                        at,
                        at.lerp(start, 1.0 / 3.0),
                        at.lerp(start, 2.0 / 3.0),
                        start,
                    ),
                    &mut at,
                );
                finish(&mut current, true, &mut parts);
                at = start;
            }
        }
    }
    finish(&mut current, false, &mut parts);
    parts
}

/// Adds a piece unless it is a point, which a morph cannot split.
fn push(part: &mut Part, cubic: CubicBez, at: &mut Point) {
    *at = cubic.p3;
    if !(cubic.p0 == cubic.p1 && cubic.p1 == cubic.p2 && cubic.p2 == cubic.p3) {
        part.cubics.push(cubic);
    }
}

/// Each part of `group` shrunk to a point at the middle of its box, for a part with no partner
/// to grow from or shrink to. A part with no partner keeps its place.
fn collapsed(group: &[Part]) -> Vec<Part> {
    group
        .iter()
        .map(|part| {
            let c = box_of(&part.cubics).center();
            Part {
                closed: part.closed,
                cubics: vec![CubicBez::new(c, c, c, c)],
            }
        })
        .collect()
}

fn box_of(cubics: &[CubicBez]) -> Rect {
    cubics
        .iter()
        .map(|c| c.bounding_box())
        .reduce(|a, b| a.union(b))
        .expect("a part has pieces")
}

// ---- pairing ---------------------------------------------------------------------------------

/// Pairs the glyphs of two text or math objects: the longest run of glyphs, in order on both
/// sides, that show the same characters, the earliest where there is a choice. A glyph left out
/// keeps its shape and place and fades out, or fades in, over the morph.
fn by_characters(from: &[Group], to: &[Group]) -> Vec<GroupPair> {
    let same = |i: usize, j: usize| from[i].characters == to[j].characters;
    // longest[i][j]: how many glyphs of from[i..] and to[j..] can pair.
    let mut longest = vec![vec![0usize; to.len() + 1]; from.len() + 1];
    for i in (0..from.len()).rev() {
        for j in (0..to.len()).rev() {
            longest[i][j] = if same(i, j) {
                longest[i + 1][j + 1] + 1
            } else {
                longest[i + 1][j].max(longest[i][j + 1])
            };
        }
    }
    let fade = |group: &Group, out: bool| {
        let gone = Paint {
            stroke: None,
            fill: group.paint.fill.map(|fill| Fill {
                opacity: UnitInterval::clamped(0.0),
                ..fill
            }),
        };
        let (from, to) = if out {
            (group.paint, gone)
        } else {
            (gone, group.paint)
        };
        GroupPair {
            pairs: pair_parts(&group.parts, &group.parts, equalize),
            from,
            to,
        }
    };
    let (mut i, mut j, mut groups) = (0, 0, Vec::new());
    while i < from.len() || j < to.len() {
        if i < from.len()
            && j < to.len()
            && same(i, j)
            && longest[i][j] == longest[i + 1][j + 1] + 1
        {
            groups.push(GroupPair {
                pairs: pair_parts(&from[i].parts, &to[j].parts, equalize),
                from: from[i].paint,
                to: to[j].paint,
            });
            (i, j) = (i + 1, j + 1);
        } else if j == to.len() || (i < from.len() && longest[i + 1][j] >= longest[i][j + 1]) {
            groups.push(fade(&from[i], true));
            i += 1;
        } else {
            groups.push(fade(&to[j], false));
            j += 1;
        }
    }
    groups
}

/// Pairs parts in path order, so the agent can tell which part becomes which. A part left over
/// pairs with a point at its own middle, so it grows or shrinks in place.
fn pair_parts(a: &[Part], b: &[Part], pair: impl Fn(&Part, &Part) -> Pair) -> Vec<Pair> {
    let (collapsed_a, collapsed_b) = (collapsed(a), collapsed(b));
    (0..a.len().max(b.len()))
        .map(|k| {
            pair(
                a.get(k).unwrap_or_else(|| &collapsed_b[k]),
                b.get(k).unwrap_or_else(|| &collapsed_a[k]),
            )
        })
        .collect()
}

/// Two open parts whose knots both run left to right, as a function plot's do, as straight pieces
/// at the union of their knots' x values. Each part's x span is scaled onto the other's, so parts
/// over different x ranges still meet end to end. `None` for a part that turns back in x, such as
/// a point plot of a circle.
fn along_x(from: &Part, to: &Part) -> Option<Pair> {
    let knots = |part: &Part| -> Option<Vec<Point>> {
        let knots: Vec<Point> = std::iter::once(part.cubics[0].p0)
            .chain(part.cubics.iter().map(|c| c.p3))
            .collect();
        let rising = !part.closed && knots.windows(2).all(|w| w[0].x < w[1].x);
        rising.then_some(knots)
    };
    let (a, b) = (knots(from)?, knots(to)?);
    let share =
        |knots: &[Point], x: f64| (x - knots[0].x) / (knots[knots.len() - 1].x - knots[0].x);
    let mut shares: Vec<f64> = a
        .iter()
        .map(|p| share(&a, p.x))
        .chain(b.iter().map(|p| share(&b, p.x)))
        .collect();
    shares.sort_by(f64::total_cmp);
    shares.dedup_by(|x, y| (*x - *y).abs() < 1e-12);
    let lines = |knots: &[Point]| -> Vec<CubicBez> {
        let (x0, x1) = (knots[0].x, knots[knots.len() - 1].x);
        let at = |share: f64| {
            let x = x0 + (x1 - x0) * share;
            let k = knots.partition_point(|p| p.x < x).clamp(1, knots.len() - 1);
            let (p, q) = (knots[k - 1], knots[k]);
            p.lerp(q, ((x - p.x) / (q.x - p.x)).clamp(0.0, 1.0))
        };
        shares
            .windows(2)
            .map(|w| {
                let (p, q) = (at(w[0]), at(w[1]));
                CubicBez::new(p, p.lerp(q, 1.0 / 3.0), p.lerp(q, 2.0 / 3.0), q)
            })
            .collect()
    };
    Some(Pair {
        from: lines(&a),
        to: lines(&b),
        closed: (false, false),
    })
}

/// Gives two parts the same number of pieces, then lines the second up with the first. Twice the
/// larger count lets a corner of one meet the middle of a piece of the other, so a square's
/// corners meet a circle's diagonals and the shape does not turn.
fn equalize(from: &Part, to: &Part) -> Pair {
    let n = 2 * from.cubics.len().max(to.cubics.len());
    let from_cubics = split_to(&from.cubics, n);
    let mut to_cubics = split_to(&to.cubics, n);
    if from.closed && to.closed {
        if let (Some(a), Some(b)) = (signed_area(&from_cubics), signed_area(&to_cubics))
            && a.signum() != b.signum()
        {
            to_cubics = reversed(&to_cubics);
        }
        let shift = best_shift(&from_cubics, &to_cubics);
        to_cubics.rotate_left(shift);
    } else if reversing_is_nearer(&from_cubics, &to_cubics) {
        to_cubics = reversed(&to_cubics);
    }
    Pair {
        from: from_cubics,
        to: to_cubics,
        closed: (from.closed, to.closed),
    }
}

/// `cubics` cut into `n` pieces by halving the longest piece, by arc length, until there are
/// enough. A single point is `n` copies of itself.
fn split_to(cubics: &[CubicBez], n: usize) -> Vec<CubicBez> {
    if cubics.len() == 1
        && cubics[0].p0 == cubics[0].p3
        && cubics[0].p0 == cubics[0].p1
        && cubics[0].p0 == cubics[0].p2
    {
        return vec![cubics[0]; n];
    }
    let mut pieces: Vec<(CubicBez, f64)> = cubics
        .iter()
        .map(|c| (*c, c.arclen(ARCLEN_ACCURACY)))
        .collect();
    while pieces.len() < n {
        let (k, _) = pieces
            .iter()
            .enumerate()
            .fold(
                (0, f64::MIN),
                |best, (k, (_, len))| if *len > best.1 { (k, *len) } else { best },
            );
        let (cubic, len) = pieces[k];
        let middle = cubic.inv_arclen(len / 2.0, ARCLEN_ACCURACY);
        let (first, second) = (cubic.subsegment(0.0..middle), cubic.subsegment(middle..1.0));
        pieces[k] = (first, len / 2.0);
        pieces.insert(k + 1, (second, len / 2.0));
    }
    pieces.into_iter().map(|(c, _)| c).collect()
}

const ARCLEN_ACCURACY: f64 = 1e-6;

/// `None` for a part with no area, such as a point or a line, which has no orientation.
fn signed_area(cubics: &[CubicBez]) -> Option<f64> {
    let area: f64 = cubics.iter().map(|c| c.signed_area()).sum();
    (area.abs() > 1e-12).then_some(area)
}

fn reversed(cubics: &[CubicBez]) -> Vec<CubicBez> {
    cubics
        .iter()
        .rev()
        .map(|c| CubicBez::new(c.p3, c.p2, c.p1, c.p0))
        .collect()
}

/// The turn of a closed part, in pieces, that puts the nearest starting points together: the
/// least total squared distance between the start of each piece and the start of its partner. The
/// least turn wins a tie, so the same two shapes always match the same way.
fn best_shift(from: &[CubicBez], to: &[CubicBez]) -> usize {
    let n = from.len();
    let cost = |shift: usize| -> f64 {
        (0..n)
            .map(|i| (from[i].p0 - to[(i + shift) % n].p0).hypot2())
            .sum()
    };
    let costs: Vec<f64> = (0..n).map(cost).collect();
    let least = costs.iter().copied().fold(f64::INFINITY, f64::min);
    costs
        .iter()
        .position(|&c| c <= least + 1e-9 * (1.0 + least))
        .expect("a shift has the least cost")
}

fn reversing_is_nearer(from: &[CubicBez], to: &[CubicBez]) -> bool {
    let (a, b) = (
        (from[0].p0 - to[0].p0).hypot2() + (from[from.len() - 1].p3 - to[to.len() - 1].p3).hypot2(),
        (from[0].p0 - to[to.len() - 1].p3).hypot2() + (from[from.len() - 1].p3 - to[0].p0).hypot2(),
    );
    b < a
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use glam::Vec2;

    use super::*;
    use crate::axes::{Appearance, Axes, Axis};
    use crate::expr::Expression;
    use crate::plot;
    use crate::scene::{Color, FillRule};

    fn square() -> Path {
        Path::polygon(&[
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ])
        .unwrap()
    }

    fn flat(path: Path, stroke: Option<Stroke>, fill: Option<Fill>) -> MorphEnd {
        MorphEnd::of(&Shape::Flat { path, stroke, fill }).unwrap()
    }

    fn color(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b }
    }

    fn stroke(c: Color, width: f32) -> Stroke {
        Stroke {
            color: c,
            width: Positive::try_from(f64::from(width)).unwrap(),
            dash: None,
        }
    }

    /// The one path of a mix of two shapes that have one group each.
    fn only(mix: Shape) -> MixedPath {
        let Shape::Mix(mut paths) = mix else {
            panic!("a morph draws a mix")
        };
        assert_eq!(paths.len(), 1);
        paths.remove(0)
    }

    fn knots(shape: &Shape) -> Vec<Point> {
        let MixedPath { path, .. } = only(shape.clone());
        path.elements()
            .iter()
            .filter_map(|e| match e {
                PathEl::MoveTo(p) | PathEl::CurveTo(_, _, p) => Some(*p),
                _ => None,
            })
            .collect()
    }

    fn assert_knots(actual: &[Point], expected: &[(f64, f64)]) {
        assert_eq!(actual.len(), expected.len(), "{actual:?}");
        for (a, &e) in actual.iter().zip(expected) {
            assert!(
                (*a - Point::from(e)).hypot() < 1e-3,
                "{actual:?} is not {expected:?}"
            );
        }
    }

    #[test]
    fn halfway_between_a_square_and_a_circle_corners_meet_the_diagonals_so_nothing_turns() {
        let plan = MorphPlan::new(
            &flat(square(), None, None),
            &flat(Path::circle(1.0).unwrap(), None, None),
        );

        assert_eq!((plan.parts(), plan.pieces()), (1, 8));
        assert_knots(
            &knots(&plan.at(0.5).unwrap()),
            &[
                (-0.8536, -0.8536),
                (0.0, -1.0),
                (0.8536, -0.8536),
                (1.0, 0.0),
                (0.8536, 0.8536),
                (0.0, 1.0),
                (-0.8536, 0.8536),
                (-1.0, 0.0),
                (-0.8536, -0.8536),
            ],
        );
    }

    #[test]
    fn parts_pair_in_path_order_not_by_size() {
        let small_then_big = Path::from_svg(
            "M -3.5 -0.5 L -2.5 -0.5 L -2.5 0.5 L -3.5 0.5 Z M 2 -1 L 4 -1 L 4 1 L 2 1 Z",
        )
        .unwrap();
        let big_then_small = Path::from_svg(
            "M -4 -1 L -2 -1 L -2 1 L -4 1 Z M 2.5 -0.5 L 3.5 -0.5 L 3.5 0.5 L 2.5 0.5 Z",
        )
        .unwrap();
        let plan = MorphPlan::new(
            &flat(small_then_big, None, None),
            &flat(big_then_small, None, None),
        );

        let halfway = knots(&plan.at(0.5).unwrap());
        assert_knots(&[halfway[0], halfway[9]], &[(-3.75, -0.75), (2.25, -0.75)]);
    }

    #[test]
    fn a_part_with_no_partner_grows_from_its_own_middle() {
        let hole =
            Path::from_svg("M -3 -3 L 3 -3 L 3 3 L -3 3 Z M -1 -1 L -1 1 L 1 1 L 1 -1 Z").unwrap();
        let plan = MorphPlan::new(
            &flat(hole, None, None),
            &flat(Path::circle(1.0).unwrap(), None, None),
        );

        assert_eq!(plan.parts(), 2);
        let MixedPath { path, .. } = only(plan.at(1.0).unwrap());
        let moves: Vec<_> = path
            .elements()
            .iter()
            .filter_map(|e| {
                if let PathEl::MoveTo(p) = e {
                    Some(*p)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(moves.len(), 2);
        assert_eq!(
            moves[1],
            Point::new(0.0, 0.0),
            "the second part ends as a point at the middle of the hole it was"
        );
    }

    #[test]
    fn both_parts_are_split_by_length_to_twice_the_larger_count() {
        let line = Path::line(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0)).unwrap();
        let zigzag = Path::from_svg("M 0 0 L 1 0 L 4 0 L 4 3").unwrap();
        let plan = MorphPlan::new(&flat(line, None, None), &flat(zigzag, None, None));

        assert_eq!(plan.pieces(), 6);
        assert_knots(
            &knots(&plan.at(0.0).unwrap()),
            &[
                (0.0, 0.0),
                (0.5, 0.0),
                (1.0, 0.0),
                (1.5, 0.0),
                (2.0, 0.0),
                (3.0, 0.0),
                (4.0, 0.0),
            ],
        );
    }

    #[test]
    fn colors_widths_and_opacities_mix_and_an_absent_side_fades_from_nothing() {
        let a = flat(square(), Some(stroke(color(255, 0, 0), 0.2)), None);
        let b = flat(
            Path::circle(1.0).unwrap(),
            Some(stroke(color(0, 0, 255), 0.1)),
            Some(Fill {
                color: color(0, 255, 0),
                opacity: UnitInterval::clamped(0.8),
                rule: FillRule::EvenOdd,
            }),
        );
        let plan = MorphPlan::new(&a, &b);

        let MixedPath { stroke, fill, .. } = only(plan.at(0.5).unwrap());
        let stroke = stroke.unwrap();
        assert_eq!(stroke.color, color(188, 0, 188), "halfway in linear light");
        assert!((stroke.width.get() - 0.15).abs() < 1e-6);
        let fill = fill.unwrap();
        assert_eq!(
            (fill.color, fill.rule),
            (color(0, 255, 0), FillRule::EvenOdd)
        );
        assert!(
            (fill.opacity.get() - 0.4).abs() < 1e-6,
            "from no fill to 0.8: {}",
            fill.opacity.get()
        );
        let MixedPath { fill, .. } = only(plan.at(0.0).unwrap());
        assert_eq!(fill.unwrap().opacity.get(), 0.0);
    }

    #[test]
    fn a_stroke_that_the_target_lacks_thins_away() {
        let plan = MorphPlan::new(
            &flat(square(), Some(stroke(color(255, 255, 255), 0.2)), None),
            &flat(
                Path::circle(1.0).unwrap(),
                None,
                Some(Fill::solid(color(1, 2, 3))),
            ),
        );

        let MixedPath { stroke, .. } = only(plan.at(0.25).unwrap());
        assert!((stroke.unwrap().width.get() - 0.15).abs() < 1e-6);
        let MixedPath { stroke, .. } = only(plan.at(1.0).unwrap());
        assert!(stroke.is_none());
    }

    #[test]
    fn opposite_windings_are_turned_to_agree_so_the_shape_does_not_pass_through_itself() {
        let clockwise = Path::polygon(&[
            Vec2::new(-1.0, -1.0),
            Vec2::new(-1.0, 1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(1.0, -1.0),
        ])
        .unwrap();
        let plan = MorphPlan::new(&flat(square(), None, None), &flat(clockwise, None, None));

        let MixedPath { path, .. } = only(plan.at(0.5).unwrap());
        let size = path.size();
        assert!(
            (size - 2.0).abs() < 1e-9,
            "the same square at every progress, not a bow tie: {size}"
        );
    }

    #[test]
    fn halfway_between_two_plots_each_x_is_at_the_mean_of_their_values() {
        let appearance = Appearance {
            stroke: Stroke::default(),
            color: color(255, 255, 255),
            label_size: 0.3,
        };
        let axes = Axes::new(
            Axis::new(-3.0, 3.0, None, 6.0).unwrap(),
            Axis::new(-1.0, 9.0, None, 10.0).unwrap(),
            None,
            None,
            appearance,
        )
        .unwrap();
        let plot = |source: &str| {
            flat(
                plot::function_path(
                    &axes,
                    &Expression::parse(source, crate::expr::FUNCTION_PLOT).unwrap(),
                    (-3.0, 3.0),
                )
                .unwrap(),
                None,
                None,
            )
        };
        let plan = MorphPlan::between_plots(&plot("x^2"), &plot("4 + 4*sin(2*x)"));

        let mix = knots(&plan.at(0.5).unwrap());
        assert!(
            mix.windows(2).all(|w| w[0].x <= w[1].x),
            "no sample is out of x order"
        );
        for x in [-0.6, 1.0, 1.6] {
            let k = mix.iter().position(|p| p.x >= x).unwrap();
            let (a, b) = (mix[k - 1], mix[k]);
            let local_y = a.y + (b.y - a.y) * (x - a.x) / (b.x - a.x);
            let mean = (x * x + 4.0 + 4.0 * (2.0 * x).sin()) / 2.0;
            assert!(
                (local_y + 4.0 - mean).abs() < 0.01,
                "at x = {x} the mix is at {} and the mean is {mean}",
                local_y + 4.0
            );
        }
    }

    fn math(tex: &str) -> MorphEnd {
        let layout = crate::typeset::Layout {
            size: 1.0,
            align: crate::scene::Align::Center,
        };
        let glyphs = crate::typeset::math(tex, color(255, 255, 255), layout).unwrap();
        MorphEnd::of(&Shape::Glyphs {
            glyphs,
            size: Positive::try_from(1.0).unwrap(),
        })
        .unwrap()
    }

    /// Each path of a mix with its fill's opacity, in the order the mix draws them.
    fn opacities(mix: Shape) -> Vec<f32> {
        let Shape::Mix(paths) = mix else {
            panic!("a morph draws a mix")
        };
        paths
            .iter()
            .map(|path| path.fill.unwrap().opacity.get())
            .collect()
    }

    #[test]
    fn glyphs_pair_by_the_characters_they_show_and_a_glyph_with_no_partner_fades_where_it_stands() {
        let (times, squared) = (math(r"A = \pi r \times r"), math(r"A = \pi r^2"));
        let plan = MorphPlan::new(&times, &squared);

        assert_eq!(
            opacities(plan.at(0.5).unwrap()),
            [1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5],
            "A, =, π and r pair; ×, the second r and the 2 fade"
        );
        let shape = |end: &MorphEnd, k: usize| {
            let MorphEnd::Paths { groups } = end else {
                unreachable!("math gives paths")
            };
            groups[k]
                .parts
                .iter()
                .flat_map(|part| part.cubics.clone())
                .collect::<Vec<_>>()
        };
        let Shape::Mix(halfway) = plan.at(0.5).unwrap() else {
            unreachable!()
        };
        let times_sign: Vec<Point> = halfway[4]
            .path
            .elements()
            .iter()
            .filter_map(|e| {
                if let PathEl::CurveTo(_, _, p) = e {
                    Some(*p)
                } else {
                    None
                }
            })
            .collect();
        assert!(
            shape(&times, 4)
                .iter()
                .all(|c| times_sign.iter().any(|p| (*p - c.p3).hypot() < 1e-6)),
            "the × keeps its own shape as it fades"
        );
    }

    #[test]
    fn glyphs_that_line_up_pair_in_order_and_the_new_one_fades_in() {
        let plan = MorphPlan::new(&math("x + 1"), &math("x + 12"));

        assert_eq!(opacities(plan.at(0.25).unwrap()), [1.0, 1.0, 1.0, 0.25]);
    }

    #[test]
    fn pairing_and_mixing_a_thousand_pieces_is_fast_enough_to_do_every_frame() {
        let wave = |amplitude: f64| {
            let mut d = String::from("M 0 0");
            (1..=1000).for_each(|i| {
                d.push_str(&format!(
                    " L {} {}",
                    f64::from(i) * 0.01,
                    amplitude * (f64::from(i) * 0.05).sin()
                ))
            });
            Path::from_svg(&d).unwrap()
        };
        let (a, b) = (flat(wave(1.0), None, None), flat(wave(2.0), None, None));

        let started = Instant::now();
        let plan = MorphPlan::new(&a, &b);
        let pairing = started.elapsed();
        let started = Instant::now();
        (0..10).for_each(|k| drop(plan.at(f32::from(k as u8) / 10.0)));
        let mixing = started.elapsed() / 10;

        eprintln!("pairing 1000 pieces: {pairing:?}; one frame's mix: {mixing:?}");
        assert_eq!(plan.pieces(), 2000);
    }
}
