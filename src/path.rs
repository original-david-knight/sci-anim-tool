//! The path of a flat shape and the curve math a create needs. The only module that measures
//! curves: other modules hand it a kurbo `BezPath`, as `typeset` does a glyph's outline.
//!
//! Every flat shape becomes one `Path` when the scene file is read, so `mesh` has one way to make
//! a stroke and a fill, and a create has one way to find how far along a path it has drawn.

use std::f64::consts::{PI, TAU};
use std::sync::Arc as Shared;

use glam::Vec2;
use kurbo::{
    Arc, BezPath, Circle, ParamCurve, ParamCurveArclen, ParamCurveDeriv, PathEl, PathSeg, Point,
    Shape as _,
};

/// The curve of a flat shape in its own xy plane, in the object's units: straight and Bézier
/// pieces, open or closed, in one or more parts. The stroke follows it and the fill fills it.
/// Every constructor checks that it has length and that lyon can cut it, so code past the scene
/// file trusts both.
#[derive(Clone, Debug)]
pub struct Path(Shared<Measured>);

#[derive(Debug)]
struct Measured {
    kurbo_path: BezPath,
    length: f64,
    size: f32,
    bounding_box: kurbo::Rect,
    under_map: UnderMap,
}

/// What a plane map does to a path, which only the path's maker knows. Equality of paths
/// ignores it: a path is its curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UnderMap {
    /// Every point follows the map.
    EveryPoint,
    /// An arrow's path, drawn again between its two mapped ends, so its tip is never stretched.
    FromEnds(ArrowEnds),
    /// A dot's disc: its center follows the map and the disc keeps its size.
    CenterOnly,
}

/// What `Path::arrow` was given, in the object's own plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArrowEnds {
    pub from: Vec2,
    pub to: Vec2,
    pub tip_length: f32,
}

impl PartialEq for Path {
    fn eq(&self, other: &Path) -> bool {
        self.0.kurbo_path == other.0.kurbo_path
    }
}

/// Why a path cannot be drawn.
#[derive(Debug, PartialEq)]
pub enum PathError {
    /// `d` is not SVG path syntax. The message is kurbo's, such as `Unknown command, "X"`.
    Syntax(String),
    /// A coordinate is past what a 32-bit float holds, so lyon cannot cut the path.
    TooLarge,
    /// The path has no length, so there is nothing to stroke or fill.
    NoLength,
}

/// How far a length along a path may be off, in the object's units.
const LENGTH_ACCURACY: f64 = 1e-6;

/// The largest distance, as a share of the radius, between a circle or an arc and the cubic
/// Béziers made for it. kurbo makes a full circle from 4 cubics at this tolerance.
const CUBIC_TOLERANCE: f64 = 1e-3;

impl Path {
    fn new(kurbo_path: BezPath) -> Result<Path, PathError> {
        Path::made(kurbo_path, UnderMap::EveryPoint)
    }

    fn made(kurbo_path: BezPath, under_map: UnderMap) -> Result<Path, PathError> {
        let fits = |p: &Point| p.x.abs() <= f64::from(f32::MAX) && p.y.abs() <= f64::from(f32::MAX);
        let points_fit = kurbo_path.elements().iter().all(|element| match element {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => fits(p),
            PathEl::QuadTo(c, p) => fits(c) && fits(p),
            PathEl::CurveTo(c1, c2, p) => fits(c1) && fits(c2) && fits(p),
            PathEl::ClosePath => true,
        });
        if !points_fit {
            return Err(PathError::TooLarge);
        }
        let length: f64 = pieces(&kurbo_path)
            .map(|piece| piece.segment.arclen(LENGTH_ACCURACY))
            .sum();
        if length <= 0.0 {
            return Err(PathError::NoLength);
        }
        let box_ = kurbo_path.bounding_box();
        let size = box_.width().max(box_.height()) as f32;
        Ok(Path(Shared::new(Measured {
            kurbo_path,
            length,
            size,
            bounding_box: box_,
            under_map,
        })))
    }

    /// The path moved by `by` in its own plane, as a readout lines its number up on its decimal
    /// point.
    pub fn moved(&self, by: Vec2) -> Path {
        let shift = kurbo::Affine::translate((f64::from(by.x), f64::from(by.y)));
        let measured = &self.0;
        Path(Shared::new(Measured {
            kurbo_path: shift * measured.kurbo_path.clone(),
            length: measured.length,
            size: measured.size,
            bounding_box: shift.transform_rect_bbox(measured.bounding_box),
            under_map: measured.under_map,
        }))
    }

    /// Any kurbo path, such as a glyph's outline.
    pub fn from_bez(kurbo_path: BezPath) -> Result<Path, PathError> {
        Path::new(kurbo_path)
    }

    /// A path in SVG path syntax, such as `M 0 0 L 1 0 A 1 1 0 0 1 0 1 Z`.
    pub fn from_svg(d: &str) -> Result<Path, PathError> {
        Path::new(BezPath::from_svg(d).map_err(|e| PathError::Syntax(e.to_string()))?)
    }

    pub fn line(from: Vec2, to: Vec2) -> Result<Path, PathError> {
        let mut path = BezPath::new();
        path.move_to(point(from));
        path.line_to(point(to));
        Path::new(path)
    }

    /// A shaft from `from` to the base of the tip, then the tip, a closed triangle as wide as it
    /// is long with its apex at `to`. The tip is `tip_length` long, or a quarter of the arrow
    /// when the arrow is shorter than four tips.
    pub fn arrow(from: Vec2, to: Vec2, tip_length: f32) -> Result<Path, PathError> {
        let length = from.distance(to);
        if length == 0.0 {
            return Err(PathError::NoLength);
        }
        let ends = ArrowEnds {
            from,
            to,
            tip_length,
        };
        let tip_length = tip_length.min(length / 4.0);
        let along = (to - from) / length;
        let across = along.perp() * (tip_length / 2.0);
        let base = to - along * tip_length;
        let mut path = BezPath::new();
        path.move_to(point(from));
        path.line_to(point(base));
        path.move_to(point(base - across));
        path.line_to(point(to));
        path.line_to(point(base + across));
        path.close_path();
        Path::made(path, UnderMap::FromEnds(ends))
    }

    /// A closed path through `points` in order. `scene_file` guarantees at least three.
    pub fn polygon(points: &[Vec2]) -> Result<Path, PathError> {
        let mut path = BezPath::new();
        let (first, rest) = points.split_first().expect("a polygon has points");
        path.move_to(point(*first));
        rest.iter().for_each(|p| path.line_to(point(*p)));
        path.close_path();
        Path::new(path)
    }

    /// A circle about the origin, starting at `(radius, 0)` and going counterclockwise.
    pub fn circle(radius: f32) -> Result<Path, PathError> {
        Path::circle_at(Vec2::ZERO, radius)
    }

    /// A circle about `center`, starting at its right and going counterclockwise.
    pub fn circle_at(center: Vec2, radius: f32) -> Result<Path, PathError> {
        let radius = f64::from(radius);
        Path::new(Circle::new(point(center), radius).to_path(CUBIC_TOLERANCE * radius))
    }

    /// A dot's disc: a circle about the origin that a plane map moves and never reshapes.
    pub fn dot(radius: f32) -> Result<Path, PathError> {
        let radius = f64::from(radius);
        Path::made(
            Circle::new(Point::ZERO, radius).to_path(CUBIC_TOLERANCE * radius),
            UnderMap::CenterOnly,
        )
    }

    /// An arc of a circle about the origin, from `start_angle` degrees counterclockwise from +x,
    /// sweeping `sweep` degrees counterclockwise, or clockwise when `sweep` is negative.
    pub fn arc(radius: f32, start_angle: f32, sweep: f32) -> Result<Path, PathError> {
        let radius = f64::from(radius);
        let arc = Arc::new(
            Point::ZERO,
            (radius, radius),
            f64::from(start_angle).to_radians(),
            f64::from(sweep).to_radians(),
            0.0,
        );
        Path::new(arc.to_path(CUBIC_TOLERANCE * radius))
    }

    /// The length along the path, over all its parts.
    pub fn length(&self) -> f64 {
        self.0.length
    }

    /// The first `share` of the path by length along it, as a create draws it. A part that is
    /// cut is left open at the cut. At a share of 1 the path is whole, with its closed parts
    /// still closed. At a share of 0 it has no length, so it is a kurbo `BezPath` and not a `Path`.
    pub fn partial(&self, share: f32) -> BezPath {
        if share >= 1.0 {
            return self.0.kurbo_path.clone();
        }
        let target = f64::from(share.max(0.0)) * self.length();
        let mut partial = BezPath::new();
        let mut walked = 0.0;
        for Piece {
            segment,
            starts_part,
            closes_part,
        } in pieces(&self.0.kurbo_path)
        {
            if starts_part {
                partial.move_to(segment.start());
            }
            let length = segment.arclen(LENGTH_ACCURACY);
            if walked + length <= target {
                walked += length;
                if closes_part {
                    partial.close_path()
                } else {
                    partial.push(segment.as_path_el())
                }
            } else {
                let remaining = target - walked;
                if remaining > 0.0 {
                    let cut = segment.inv_arclen(remaining, LENGTH_ACCURACY);
                    partial.push(segment.subsegment(0.0..cut).as_path_el());
                }
                break;
            }
        }
        partial
    }

    /// The point at `share` of the path's length, by the same walk over the same pieces as
    /// `partial`, so a carry at a share sits at the end of a create at that share. A share at the
    /// end of a part sits there, and one just past it at the start of the next part, so a carry
    /// crosses the gap between two parts in one frame.
    ///
    /// Along a straight piece the tangent turns evenly from the bisector of the joint at its
    /// start to the bisector of the joint at its end, wherever two straight pieces of one part
    /// meet at a gentle joint. A plot's curve is straight pieces, so without this a carry facing
    /// it would turn in steps, one per piece; a corner sharper than a gentle joint still turns at
    /// once, as a polygon's should.
    pub fn point_at(&self, share: f32) -> PathPoint {
        let target = f64::from(share.clamp(0.0, 1.0)) * self.length();
        let mut walked = 0.0;
        let mut angle = None;
        let mut last = None;
        let pieces = pieces_with_length(&self.0.kurbo_path);
        for (index, piece) in pieces.iter().enumerate() {
            let segment = piece.segment;
            let length = segment.arclen(LENGTH_ACCURACY);
            let cut = (walked + length >= target)
                .then(|| segment.inv_arclen(target - walked, LENGTH_ACCURACY));
            let until = cut.unwrap_or(1.0);
            // A Bézier turns less than half a turn over each of these steps, so the angle can
            // be counted on through it.
            let steps = if matches!(segment, PathSeg::Line(_)) {
                1
            } else {
                TURNING_STEPS
            };
            let mut tangent = kurbo::Vec2::ZERO;
            for k in 0..=steps {
                tangent = tangent_of(&segment, until * f64::from(k) / f64::from(steps));
                angle = Some(turned_on(angle, tangent));
            }
            if cut.is_some() {
                let (before, after) = (
                    gentle_turn(&pieces, index, Joint::Start),
                    gentle_turn(&pieces, index, Joint::End),
                );
                if before != 0.0 || after != 0.0 {
                    let blended = angle.expect("a piece was counted") - before / 2.0
                        + (before + after) / 2.0 * until;
                    angle = Some(blended);
                    tangent = kurbo::Vec2::new(blended.cos(), blended.sin());
                }
            }
            last = Some((segment.eval(until), tangent));
            if cut.is_some() {
                break;
            }
            walked += length;
        }
        let (point, tangent) = last.expect("a path has length");
        let unit = tangent.normalize();
        PathPoint {
            point: Vec2::new(point.x as f32, point.y as f32),
            tangent: Vec2::new(unit.x as f32, unit.y as f32),
            angle: angle.expect("a path has length"),
        }
    }

    /// The first point of the path at `x`, along the straight pieces a plot's curve is drawn as,
    /// or the point nearest `x` where no piece reaches it, as where the curve is cut.
    pub fn point_at_x(&self, x: f32) -> Vec2 {
        let x = f64::from(x);
        let mut nearest: Option<(f64, Point)> = None;
        for Piece { segment, .. } in pieces(&self.0.kurbo_path) {
            let (a, b) = (segment.start(), segment.end());
            let (left, right) = if a.x <= b.x { (a, b) } else { (b, a) };
            let (miss, at) = if x < left.x {
                (left.x - x, left)
            } else if x > right.x {
                (x - right.x, right)
            } else if left.x == right.x {
                (0.0, left)
            } else {
                (0.0, left.lerp(right, (x - left.x) / (right.x - left.x)))
            };
            if nearest.is_none_or(|(least, _)| miss < least) {
                nearest = Some((miss, at));
            }
            if miss == 0.0 {
                break;
            }
        }
        let (_, at) = nearest.expect("a path has length");
        Vec2::new(at.x as f32, at.y as f32)
    }

    /// The longest side of the box around the path, for a tolerance relative to the path's size.
    /// More than 0, because the path has length.
    pub fn size(&self) -> f32 {
        self.0.size
    }

    /// The box around the path, for a plot to read back the data values it spans.
    pub fn bounding_box(&self) -> kurbo::Rect {
        self.0.bounding_box
    }

    pub fn under_map(&self) -> UnderMap {
        self.0.under_map
    }

    pub(crate) fn elements(&self) -> &[PathEl] {
        self.0.kurbo_path.elements()
    }
}

/// Each drawn piece of a kurbo path in order. A `MoveTo` draws nothing, so it is not a piece: it
/// marks the piece after it as the start of a part.
fn pieces(kurbo_path: &BezPath) -> impl Iterator<Item = Piece> + '_ {
    let mut current = Point::ZERO;
    let mut part_start = Point::ZERO;
    let mut starts_part = true;
    kurbo_path.elements().iter().filter_map(move |element| {
        let (segment, closes_part) = match *element {
            PathEl::MoveTo(p) => {
                current = p;
                part_start = p;
                starts_part = true;
                return None;
            }
            PathEl::LineTo(p) => (PathSeg::Line(kurbo::Line::new(current, p)), false),
            PathEl::QuadTo(c, p) => (PathSeg::Quad(kurbo::QuadBez::new(current, c, p)), false),
            PathEl::CurveTo(c1, c2, p) => (
                PathSeg::Cubic(kurbo::CubicBez::new(current, c1, c2, p)),
                false,
            ),
            PathEl::ClosePath => (PathSeg::Line(kurbo::Line::new(current, part_start)), true),
        };
        current = segment.end();
        let piece = Piece {
            segment,
            starts_part,
            closes_part,
        };
        starts_part = closes_part;
        Some(piece)
    })
}

/// The pieces that have length, in order. A piece with none hands on that it starts a part to
/// the piece after it, and that it closes one to the piece before it, as a `Z` back to a part's
/// start does.
fn pieces_with_length(kurbo_path: &BezPath) -> Vec<Piece> {
    let mut kept: Vec<Piece> = Vec::new();
    let mut starts_part = false;
    for piece in pieces(kurbo_path) {
        if piece.segment.arclen(LENGTH_ACCURACY) > 0.0 {
            kept.push(Piece {
                starts_part: piece.starts_part || starts_part,
                ..piece
            });
            starts_part = false;
        } else {
            starts_part |= piece.starts_part;
            if piece.closes_part
                && !piece.starts_part
                && let Some(before) = kept.last_mut()
            {
                before.closes_part = true;
            }
        }
    }
    kept
}

/// How many steps a Bézier piece's turning is counted in, so its tangent turns less than half a
/// turn over each.
const TURNING_STEPS: u32 = 16;

/// The way a piece runs at `t`, toward its end. Where a Bézier's control point sits on its end
/// the derivative there is zero, so the chord across a little of the piece stands in for it.
fn tangent_of(segment: &PathSeg, parameter: f64) -> kurbo::Vec2 {
    let derivative = match segment {
        PathSeg::Line(line) => line.p1 - line.p0,
        PathSeg::Quad(quad) => quad.deriv().eval(parameter).to_vec2(),
        PathSeg::Cubic(cubic) => cubic.deriv().eval(parameter).to_vec2(),
    };
    if derivative.hypot2() > 1e-18 {
        return derivative;
    }
    segment.eval((parameter + 1e-3).min(1.0)) - segment.eval((parameter - 1e-3).max(0.0))
}

/// The largest turn at a joint of two straight pieces that a carry's tangent turns through
/// evenly rather than at once: more than a plot's curve turns between two of its samples, and
/// less than a polygon's corner turns, unless it has more than 24 sides.
const GENTLE_JOINT: f64 = 15.0 * PI / 180.0;

/// Which end of a piece a joint is at.
#[derive(Clone, Copy)]
enum Joint {
    Start,
    End,
}

/// The signed turn in radians at the joint at one end of the straight piece `pieces[index]`, when
/// a straight piece of the same part meets it there by a gentle joint, and 0 otherwise: at a
/// Bézier, at a sharper corner, and at an open part's ends. The pieces have length, so the joint
/// of a closed part's last piece and its first is found even when its `Z` draws nothing.
fn gentle_turn(pieces: &[Piece], index: usize, joint: Joint) -> f64 {
    let part_start = (0..=index)
        .rev()
        .find(|&i| pieces[i].starts_part)
        .expect("the first piece starts a part");
    let part_end = (index..pieces.len())
        .find(|&i| i + 1 == pieces.len() || pieces[i + 1].starts_part)
        .expect("the last piece ends a part");
    let closed = pieces[part_end].closes_part;
    let neighbour = match joint {
        Joint::Start if index > part_start => Some(index - 1),
        Joint::Start => closed.then_some(part_end),
        Joint::End if index < part_end => Some(index + 1),
        Joint::End => closed.then_some(part_start),
    };
    let (PathSeg::Line(here), Some(PathSeg::Line(there))) =
        (pieces[index].segment, neighbour.map(|i| pieces[i].segment))
    else {
        return 0.0;
    };
    let (from, to) = match joint {
        Joint::Start => (there.p1 - there.p0, here.p1 - here.p0),
        Joint::End => (here.p1 - here.p0, there.p1 - there.p0),
    };
    let turn = from.cross(to).atan2(from.dot(to));
    if turn.abs() < GENTLE_JOINT { turn } else { 0.0 }
}

/// The angle of `tangent` from +x, counted on from `before` by less than half a turn, so it never
/// jumps by a whole turn.
fn turned_on(before: Option<f64>, tangent: kurbo::Vec2) -> f64 {
    let angle = tangent.y.atan2(tangent.x);
    match before {
        None => angle,
        Some(before) => before + (angle - before + PI).rem_euclid(TAU) - PI,
    }
}

/// A point of a path at a share of its length, as a carry reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PathPoint {
    pub point: Vec2,
    /// Unit length, toward the path's end. At a corner, the way the piece before it runs, and
    /// along a straight piece between gentle joints, turned evenly between their bisectors.
    pub tangent: Vec2,
    /// The tangent's angle from +x in radians, counted on from the path's start through every
    /// piece, corner and part, so it never jumps by a whole turn.
    pub angle: f64,
}

/// One drawn piece of a path: a line or a Bézier, and where it sits in its part.
struct Piece {
    segment: PathSeg,
    starts_part: bool,
    /// The line back to the part's start that a `Z` draws.
    closes_part: bool,
}

fn point(v: Vec2) -> Point {
    Point::new(f64::from(v.x), f64::from(v.y))
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    fn end_of(kurbo_path: &BezPath) -> Point {
        match kurbo_path.elements().last() {
            Some(PathEl::LineTo(p) | PathEl::QuadTo(_, p) | PathEl::CurveTo(_, _, p)) => *p,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_circle_is_four_cubics_two_pi_r_long() {
        let circle = Path::circle(2.0).unwrap();

        assert_eq!(
            circle
                .elements()
                .iter()
                .filter(|e| matches!(e, PathEl::CurveTo(..)))
                .count(),
            4
        );
        assert!(
            (circle.length() - 4.0 * PI).abs() < 1e-3,
            "{}",
            circle.length()
        );
    }

    #[test]
    fn half_a_square_by_length_ends_at_the_opposite_corner() {
        let square = Path::polygon(&[
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ])
        .unwrap();

        assert_eq!(square.length(), 8.0);
        let half = square.partial(0.5);
        assert_eq!(
            half.elements(),
            &[
                PathEl::MoveTo((-1.0, -1.0).into()),
                PathEl::LineTo((1.0, -1.0).into()),
                PathEl::LineTo((1.0, 1.0).into())
            ]
        );
        assert_eq!(
            end_of(&square.partial(0.875)),
            (-1.0, 0.0).into(),
            "seven eighths is halfway down the closing side"
        );
        assert_eq!(
            square.partial(1.0).elements(),
            square.elements(),
            "whole, and still closed"
        );
        assert_eq!(
            square.partial(0.0).elements(),
            &[PathEl::MoveTo((-1.0, -1.0).into())],
            "nothing drawn yet"
        );
    }

    #[test]
    fn a_quarter_of_a_circle_by_length_turns_a_quarter_turn() {
        let quarter = Path::circle(2.0).unwrap().partial(0.25);

        let end = end_of(&quarter);
        let turned = end.y.atan2(end.x).to_degrees();
        assert!(
            (turned - 90.0).abs() < 0.05,
            "turned {turned} degrees, ending at {end:?}"
        );
    }

    #[test]
    fn an_arrow_is_a_shaft_then_a_closed_tip_and_points_somewhere() {
        let arrow = Path::arrow(Vec2::new(-2.0, 0.0), Vec2::new(2.0, 0.0), 0.35).unwrap();

        assert_eq!(arrow.elements()[0], PathEl::MoveTo((-2.0, 0.0).into()));
        assert!(
            matches!(arrow.elements()[1], PathEl::LineTo(p) if (p.x - 1.65).abs() < 1e-6 && p.y == 0.0),
            "the shaft ends at the tip's base"
        );
        assert!(matches!(arrow.elements()[2], PathEl::MoveTo(p) if (p.y - -0.175).abs() < 1e-6));
        assert!(matches!(arrow.elements()[3], PathEl::LineTo(p) if p == (2.0, 0.0).into()));
        assert_eq!(arrow.elements()[5], PathEl::ClosePath);
        assert!(
            (arrow.length() - (3.65 + 0.35 + 2.0 * (0.35f64.powi(2) + 0.175f64.powi(2)).sqrt()))
                .abs()
                < 1e-6
        );
        let short = Path::arrow(Vec2::ZERO, Vec2::new(0.8, 0.0), 0.35).unwrap();
        assert!(
            matches!(short.elements()[1], PathEl::LineTo(p) if (p.x - 0.6).abs() < 1e-6),
            "the tip is a quarter of a short arrow"
        );
        assert_eq!(
            Path::arrow(Vec2::ONE, Vec2::ONE, 0.35).unwrap_err(),
            PathError::NoLength
        );
    }

    #[test]
    fn an_arrow_keeps_its_ends_and_a_dot_its_center_under_a_map() {
        let arrow = Path::arrow(Vec2::ZERO, Vec2::new(0.8, 0.0), 0.35).unwrap();

        assert_eq!(
            arrow.under_map(),
            UnderMap::FromEnds(ArrowEnds {
                from: Vec2::ZERO,
                to: Vec2::new(0.8, 0.0),
                tip_length: 0.35
            }),
            "the declared tip, not the quarter it was cut to"
        );
        assert_eq!(Path::dot(0.05).unwrap().under_map(), UnderMap::CenterOnly);
        assert_eq!(
            Path::dot(0.05).unwrap(),
            Path::circle(0.05).unwrap(),
            "a dot's disc is a circle's path"
        );
        assert_eq!(
            Path::circle(0.05).unwrap().under_map(),
            UnderMap::EveryPoint
        );
        assert_eq!(
            Path::from_svg("M 0 0 L 1 0").unwrap().under_map(),
            UnderMap::EveryPoint
        );
    }

    #[test]
    fn an_arc_sweeps_counterclockwise_from_its_start_angle() {
        let arc = Path::arc(2.0, 0.0, 90.0).unwrap();

        assert_eq!(arc.elements()[0], PathEl::MoveTo((2.0, 0.0).into()));
        let end = end_of(&arc.partial(1.0));
        assert!(end.x.abs() < 1e-3 && (end.y - 2.0).abs() < 1e-3, "{end:?}");
        assert!((arc.length() - PI).abs() < 1e-3);
        assert!(
            (Path::arc(1.0, 90.0, -90.0).unwrap().length() - PI / 2.0).abs() < 1e-3,
            "clockwise from the top"
        );
    }

    #[test]
    fn the_point_at_a_share_is_the_end_of_the_create_at_that_share() {
        let paths = [
            Path::circle(2.0).unwrap(),
            Path::polygon(&[
                Vec2::new(-1.0, -1.0),
                Vec2::new(1.0, -1.0),
                Vec2::new(1.0, 1.0),
            ])
            .unwrap(),
            Path::arrow(Vec2::new(-2.0, 0.0), Vec2::new(2.0, 1.0), 0.35).unwrap(),
            Path::from_svg("M 0 0 C 1 2 2 -2 3 0 Q 4 1 5 0").unwrap(),
        ];

        for path in &paths {
            for share in [0.05, 0.3, 0.55, 0.8, 0.97] {
                let at = path.point_at(share).point;
                let end = end_of(&path.partial(share));
                assert!(
                    (f64::from(at.x) - end.x).abs() < 1e-5
                        && (f64::from(at.y) - end.y).abs() < 1e-5,
                    "{share}: {at} against {end:?}"
                );
            }
        }
    }

    #[test]
    fn the_tangent_runs_toward_the_paths_end() {
        let square = Path::polygon(&[
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ])
        .unwrap();

        assert_eq!(square.point_at(0.1).tangent, Vec2::X);
        assert_eq!(square.point_at(0.3).tangent, Vec2::Y);
        assert_eq!(square.point_at(0.0).point, Vec2::new(-1.0, -1.0));
        assert!(
            (square.point_at(0.9).angle - 1.5 * PI).abs() < 1e-9,
            "three quarter turns on along the closing side, not one back"
        );
    }

    #[test]
    fn the_tangent_turns_evenly_along_a_sampled_curve_and_at_once_at_a_corner() {
        let mut parabola = BezPath::new();
        parabola.move_to((-1.0, 0.5));
        (-9..=10)
            .map(|k| f64::from(k) / 10.0)
            .for_each(|x| parabola.line_to((x, x * x / 2.0)));
        let parabola = Path::from_bez(parabola).unwrap();
        let square = Path::polygon(&[
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ])
        .unwrap();

        let degrees: Vec<f64> = (92..=108)
            .map(|k| parabola.point_at(k as f32 / 200.0).angle.to_degrees())
            .collect();

        assert!(
            degrees
                .windows(2)
                .all(|pair| (pair[1] - pair[0] - 0.6529).abs() < 1e-3),
            "every 200th of the length turns 0.6529 degrees, across joints of 5.7: {degrees:?}"
        );
        assert!(
            parabola.point_at(0.5).angle.abs() < 1e-9,
            "flat at the lowest sample, the bisector of its joint"
        );
        assert!((parabola.point_at(0.5).tangent - Vec2::X).length() < 1e-6);
        assert!(
            (parabola.point_at(0.0).angle - (-0.095f64).atan2(0.1)).abs() < 1e-12,
            "an open end faces along its own piece"
        );
        assert_eq!(square.point_at(0.2499).angle, 0.0);
        assert_eq!(
            square.point_at(0.2501).angle,
            PI / 2.0,
            "a square turns its quarter turn at the corner, within a 5000th of its length"
        );
    }

    #[test]
    fn the_angle_never_jumps_by_a_turn_around_a_circle() {
        let circle = Path::circle(1.0).unwrap();

        let angles: Vec<f64> = (0..=1000)
            .map(|k| circle.point_at(k as f32 / 1000.0).angle)
            .collect();

        assert!(
            (angles[0] - PI / 2.0).abs() < 1e-3,
            "a circle starts at its right going up: {}",
            angles[0]
        );
        assert!(
            angles
                .windows(2)
                .all(|pair| (pair[1] - pair[0]).abs() < 0.01),
            "no step of a thousandth jumps"
        );
        assert!(
            (angles[1000] - angles[0] - 2.0 * PI).abs() < 1e-3,
            "one whole turn: {}",
            angles[1000] - angles[0]
        );
    }

    #[test]
    fn svg_path_syntax_is_read_by_kurbo_with_its_messages() {
        assert_eq!(Path::from_svg("M 0 0 L 3 4").unwrap().length(), 5.0);
        let syntax = |d| match Path::from_svg(d) {
            Err(PathError::Syntax(message)) => message,
            other => panic!("{other:?}"),
        };
        assert_eq!(syntax("M 0 0 L 1"), "Unexpected EOF");
        assert_eq!(
            syntax("L 1 1"),
            "Uninitialized path (missing moveto command)"
        );
        assert_eq!(syntax("M 0 0 X 1 1"), "Unknown command, \"X\"");
    }

    #[test]
    fn a_path_has_length_and_fits_in_f32() {
        assert_eq!(Path::from_svg("").unwrap_err(), PathError::NoLength);
        assert_eq!(Path::from_svg("M 1 1 Z").unwrap_err(), PathError::NoLength);
        assert_eq!(
            Path::line(Vec2::ONE, Vec2::ONE).unwrap_err(),
            PathError::NoLength
        );
        assert_eq!(Path::arc(1.0, 30.0, 0.0).unwrap_err(), PathError::NoLength);
        assert_eq!(
            Path::polygon(&[Vec2::ONE; 3]).unwrap_err(),
            PathError::NoLength
        );
        assert_eq!(
            Path::from_svg("M 0 0 L 1e39 0").unwrap_err(),
            PathError::TooLarge
        );
        assert_eq!(
            Path::circle(f32::INFINITY).unwrap_err(),
            PathError::TooLarge
        );
        assert_eq!(Path::from_svg("M 0 0 L 0 2").unwrap().size(), 2.0);
    }

    #[test]
    fn a_point_at_x_is_on_the_first_piece_there_or_nearest_it() {
        let cut = Path::from_svg("M 0 0 L 2 2 L 3 1 M 5 0 L 6 3").unwrap();

        assert_eq!(cut.point_at_x(1.0), Vec2::new(1.0, 1.0));
        assert_eq!(cut.point_at_x(2.5), Vec2::new(2.5, 1.5));
        assert_eq!(cut.point_at_x(4.5), Vec2::new(5.0, 0.0), "in the gap");
        assert_eq!(cut.point_at_x(-1.0), Vec2::new(0.0, 0.0));
    }
}
