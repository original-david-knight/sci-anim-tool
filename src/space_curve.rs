//! A space curve: three expressions in `u` for its x, y and z over a range of `u`, or the curve
//! points it passes through in order, sampled once when the scene file is read, finer where it
//! bends, with the frame a tube carries its circle on at each sample.
//!
//! Between curve points the curve is centripetal Catmull-Rom (Yuksel, Schaefer and Keyser,
//! "Parameterization and applications of Catmull-Rom curves", 2011): each span is a cubic whose
//! parameter runs over the square root of the distance between its points, so it never loops or
//! overshoots between two points, as a curve whose parameter runs 1 a span does where the spacing
//! changes.
//!
//! The frames turn no more than the curve does: each is the one before it carried along the
//! curve by two reflections (Wang, Jüttler, Zheng and Liu, "Computation of rotation minimizing
//! frames", 2008), so a tube never twists at an inflection or along a straight run, where a
//! Frenet frame flips or is undefined. A closed curve's frames are turned a little along its
//! length so the last meets the first.

use glam::{DVec3, Quat, Vec3};

use crate::axes_3d::DataToOwn;
use crate::expr::Expression;

/// A space curve's samples in the object's own space, in order of `u`, with their frames.
#[derive(Debug)]
pub struct SpaceCurve {
    frames: Vec<CurveFrame>,
    /// The length along the curve from its start to each sample.
    lengths: Vec<f32>,
    /// The u of each sample, rising from the first to the last.
    us: Vec<f64>,
    /// Whether the curve ends where it starts. Its last sample is then its first.
    closed: bool,
    /// The curve points a curve written through points passes through, in the object's own
    /// space, as written: one per point, equal neighbors kept. `None` for a curve of expressions
    /// and a coil.
    curve_points: Option<Vec<DVec3>>,
}

/// Where a sample lies, the way the curve runs there, and a direction square to that way. The
/// third direction is `tangent × normal`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveFrame {
    pub point: Vec3,
    pub tangent: Vec3,
    pub normal: Vec3,
}

/// The outline a tube carries at one point of its curve: an ellipse about `point`, square to
/// `tangent`, reaching `half_width` along `across` and `half_thickness` along
/// `tangent × across`. A round tube's is a circle, both halves its radius, and a cartoon's is a
/// flat ribbon along a helix and an arrow along a strand. Two cross-sections in a row at one
/// point and of different sizes are a step, which a flat ring joins.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrossSection {
    pub point: Vec3,
    pub tangent: Vec3,
    pub across: Vec3,
    pub half_width: f32,
    pub half_thickness: f32,
}

#[derive(Debug, PartialEq)]
pub enum SpaceCurveError {
    /// The coordinate at this index, 0 for x, 1 for y and 2 for z, has no finite value at `u`.
    NoValue { coordinate: usize, u: f64 },
    /// The coordinate at this index lies farther than `FARTHEST` from the object's position at `u`.
    TooFar {
        coordinate: usize,
        u: f64,
        value: f64,
    },
    /// Every sample is one point.
    NoLength,
}

/// How far from the object's position a space curve may reach, in world units.
pub const FARTHEST: f64 = 1e4;

/// A curve whose ends lie this share of the size of its box apart, or closer, is closed: its end
/// is moved onto its start. A `u_range` of `[0, 6.2832]` for a full turn closes, though 6.2832
/// is not quite 2 pi.
const CLOSING_SHARE: f64 = 1e-3;

/// How many samples a space curve starts with before it refines, evenly spaced in `u`.
const FIRST_SAMPLES: usize = 256;
/// How many times a step in `u` may be halved.
const MAX_DEPTH: u32 = 10;
/// The most samples a space curve makes.
const MAX_SAMPLES: usize = 8_000;
/// How far the curve may sit from its straight pieces, as a share of the size of its box. At this
/// share a circle is cut every 5 degrees or so.
const SAMPLE_TOLERANCE: f64 = 5e-4;
/// How many even pieces each span between two curve points starts with before it refines. A
/// span may bend one way and then the other, and its middle then lies on its chord.
const SPAN_PIECES: usize = 4;

impl SpaceCurve {
    /// Samples `coordinates`, the curve's x, y and z in `u`, over `u_range`, low first. On 3D axes
    /// they are data coordinates that `on_axes` carries into the object's own space, and the curve
    /// is sampled finer where it bends there.
    pub fn sample(
        coordinates: &[Expression; 3],
        u_range: (f64, f64),
        on_axes: Option<DataToOwn>,
    ) -> Result<SpaceCurve, SpaceCurveError> {
        let at = |u: f64| -> Result<[f64; 3], SpaceCurveError> {
            let mut point = [0.0; 3];
            for (coordinate, expression) in coordinates.iter().enumerate() {
                let value = expression.eval(&[u]);
                if !value.is_finite() {
                    return Err(SpaceCurveError::NoValue { coordinate, u });
                }
                if value.abs() > FARTHEST {
                    return Err(SpaceCurveError::TooFar {
                        coordinate,
                        u,
                        value,
                    });
                }
                point[coordinate] = value;
            }
            Ok(on_axes.map_or(point, |to_own| to_own.own_f64(point)))
        };
        let (lo, hi) = u_range;
        let first: Vec<(f64, [f64; 3])> = (0..=FIRST_SAMPLES)
            .map(|k| {
                let u = if k == FIRST_SAMPLES {
                    hi
                } else {
                    lo + (hi - lo) * k as f64 / FIRST_SAMPLES as f64
                };
                at(u).map(|point| (u, point))
            })
            .collect::<Result<_, _>>()?;
        let size = diagonal(first.iter().map(|(_, p)| *p));
        if size == 0.0 {
            return Err(SpaceCurveError::NoLength);
        }
        let mut samples = vec![first[0]];
        for pair in first.windows(2) {
            refine(
                &at,
                pair[0],
                pair[1],
                0,
                size * SAMPLE_TOLERANCE,
                &mut samples,
            )?;
            samples.push(pair[1]);
        }
        let mut sampled: Vec<(f64, Vec3)> = samples
            .iter()
            .map(|(u, [x, y, z])| (*u, Vec3::new(*x as f32, *y as f32, *z as f32)))
            .collect();
        sampled.dedup_by(|b, a| b.1 == a.1);
        let (us, mut points): (Vec<f64>, Vec<Vec3>) = sampled.into_iter().unzip();
        let closed = points.len() > 3
            && points[0].distance(*points.last().expect("samples"))
                <= (CLOSING_SHARE * size) as f32;
        if closed {
            *points.last_mut().expect("samples") = points[0];
        }
        if points.len() < 2 {
            return Err(SpaceCurveError::NoLength);
        }
        let mut lengths = vec![0.0];
        for pair in points.windows(2) {
            lengths.push(lengths.last().expect("a first length") + pair[0].distance(pair[1]));
        }
        let tangents = chord_tangents(&points, closed);
        let frames = frames(&points, &tangents, &lengths, closed);
        Ok(SpaceCurve {
            frames,
            lengths,
            us,
            closed,
            curve_points: None,
        })
    }

    /// The smooth curve through `curve_points` in order, centripetal Catmull-Rom between each two,
    /// sampled finer where it bends. On 3D axes they are data coordinates that `on_axes` carries
    /// into the object's own space, and the curve is made there. A point equal to the one before
    /// it adds nothing. A curve whose last point lies on its first, with at least two between
    /// them, is closed and runs smoothly through that point. An open curve starts and ends along its
    /// first and last chord.
    pub fn smooth_through(
        curve_points: &[[f64; 3]],
        on_axes: Option<DataToOwn>,
    ) -> Result<SpaceCurve, SpaceCurveError> {
        let own: Vec<DVec3> = curve_points
            .iter()
            .map(|&point| DVec3::from(on_axes.map_or(point, |to_own| to_own.own_f64(point))))
            .collect();
        let mut points: Vec<DVec3> = Vec::with_capacity(curve_points.len());
        // The index in `curve_points` of the last of each run of equal points, which the span
        // after it leaves from.
        let mut last_of: Vec<usize> = Vec::with_capacity(curve_points.len());
        for (index, &point) in own.iter().enumerate() {
            if points.last() == Some(&point) {
                *last_of.last_mut().expect("a point before") = index;
            } else {
                points.push(point);
                last_of.push(index);
            }
        }
        let n = points.len();
        if n < 2 {
            return Err(SpaceCurveError::NoLength);
        }
        let size = diagonal(points.iter().map(|point| point.to_array()));
        let closed = n > 3 && points[0].distance(points[n - 1]) <= CLOSING_SHARE * size;
        if closed {
            points[n - 1] = points[0];
        }
        let spans = Spans::new(points, closed);
        let at = |g: f64| Ok(spans.at(g).0.to_array());
        let mut samples = vec![(0.0, spans.points[0].to_array())];
        for span in 0..n - 1 {
            for piece in 0..SPAN_PIECES {
                let g = |k: usize| span as f64 + k as f64 / SPAN_PIECES as f64;
                let (a, b) = (g(piece), g(piece + 1));
                let a = (a, at(a)?);
                let b = (b, at(b)?);
                refine(&at, a, b, 0, size * SAMPLE_TOLERANCE, &mut samples)?;
                samples.push(b);
            }
        }
        let last_u = (curve_points.len() - 1) as f64;
        let u_of = |g: f64| {
            let span = (g.floor() as usize).min(n - 2);
            ((last_of[span] as f64 + g - span as f64) / last_u).min(1.0)
        };
        let mut sampled: Vec<(Vec3, Vec3, f64)> = samples
            .iter()
            .map(|&(g, _)| {
                let (point, derivative) = spans.at(g);
                (point.as_vec3(), derivative.as_vec3(), u_of(g))
            })
            .collect();
        sampled.dedup_by(|b, a| b.0 == a.0);
        let (points, derivatives): (Vec<Vec3>, Vec<Vec3>) = sampled
            .iter()
            .map(|&(point, derivative, _)| (point, derivative))
            .unzip();
        let us = sampled.iter().map(|&(_, _, u)| u).collect();
        let chords = chord_tangents(&points, closed);
        let tangents = derivatives
            .iter()
            .zip(chords)
            .map(|(derivative, chord)| derivative.try_normalize().unwrap_or(chord))
            .collect::<Vec<_>>();
        let mut lengths = vec![0.0];
        for pair in points.windows(2) {
            lengths.push(lengths.last().expect("a first length") + pair[0].distance(pair[1]));
        }
        let frames = frames(&points, &tangents, &lengths, closed);
        Ok(SpaceCurve {
            frames,
            lengths,
            us,
            closed,
            curve_points: Some(own),
        })
    }

    /// The open curve through `points` in order, with no sampling of its own: a spring's coil,
    /// whose turns give every point. Its u runs from 0 at the first point to 1 at the last, by
    /// point. `None` for fewer than two distinct points.
    pub fn through(mut points: Vec<Vec3>) -> Option<SpaceCurve> {
        points.dedup();
        if points.len() < 2 {
            return None;
        }
        let mut lengths = vec![0.0];
        for pair in points.windows(2) {
            lengths.push(lengths.last().expect("a first length") + pair[0].distance(pair[1]));
        }
        let tangents = chord_tangents(&points, false);
        let frames = frames(&points, &tangents, &lengths, false);
        let last = (points.len() - 1) as f64;
        Some(SpaceCurve {
            frames,
            lengths,
            us: (0..points.len()).map(|k| k as f64 / last).collect(),
            closed: false,
            curve_points: None,
        })
    }

    pub fn length(&self) -> f32 {
        *self.lengths.last().expect("at least two samples")
    }

    /// The curve points of a curve written through points, in the object's own space, one per
    /// point as written. `None` for a curve of expressions and a coil.
    pub fn curve_points(&self) -> Option<&[DVec3]> {
        self.curve_points.as_deref()
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    pub fn start(&self) -> Vec3 {
        self.frames[0].point
    }

    pub fn end(&self) -> Vec3 {
        self.frames.last().expect("at least two samples").point
    }

    /// The lowest and the highest u, at the curve's start and its end.
    pub fn u_range(&self) -> (f64, f64) {
        (self.us[0], *self.us.last().expect("at least two samples"))
    }

    /// The point at `u`, between the two samples on either side of it, or the start or the end
    /// for a u outside `u_range`.
    pub fn point_at_u(&self, u: f64) -> Vec3 {
        let after = self
            .us
            .partition_point(|&sample| sample <= u)
            .clamp(1, self.us.len() - 1);
        let (a, b) = (self.us[after - 1], self.us[after]);
        let share = ((u - a) / (b - a)).clamp(0.0, 1.0) as f32;
        self.frames[after - 1]
            .point
            .lerp(self.frames[after].point, share)
    }

    /// How far the curve's farthest sample lies from the object's position.
    pub fn reach(&self) -> f32 {
        self.frames
            .iter()
            .map(|frame| frame.point.length())
            .fold(0.0, f32::max)
    }

    /// The frames from the start up to `length` along the curve, the last one at exactly that
    /// length, between two samples.
    pub fn frames_up_to(&self, length: f32) -> Vec<CurveFrame> {
        if length >= self.length() {
            return self.frames.clone();
        }
        let after = self.after(length);
        let mut frames = self.frames[..after].to_vec();
        frames.push(self.frame_at(length));
        frames
    }

    /// The frame at `length` along the curve, between two samples: the last of `frames_up_to`.
    pub fn frame_at(&self, length: f32) -> CurveFrame {
        if length >= self.length() {
            return *self.frames.last().expect("at least two samples");
        }
        let after = self.after(length);
        let (a, b) = (self.frames[after - 1], self.frames[after]);
        let share =
            (length - self.lengths[after - 1]) / (self.lengths[after] - self.lengths[after - 1]);
        let tangent = a.tangent.lerp(b.tangent, share).normalize_or(a.tangent);
        let normal = a
            .normal
            .lerp(b.normal, share)
            .reject_from_normalized(tangent)
            .normalize_or(a.normal);
        CurveFrame {
            point: a.point.lerp(b.point, share),
            tangent,
            normal,
        }
    }

    /// The index of the first sample past `length`, and never the first sample.
    fn after(&self, length: f32) -> usize {
        self.lengths
            .partition_point(|&l| l <= length)
            .clamp(1, self.lengths.len() - 1)
    }
}

/// The length of the diagonal of the box around `points`.
fn diagonal(points: impl Iterator<Item = [f64; 3]>) -> f64 {
    let (mut min, mut max) = ([f64::MAX; 3], [f64::MIN; 3]);
    for point in points {
        for k in 0..3 {
            min[k] = min[k].min(point[k]);
            max[k] = max[k].max(point[k]);
        }
    }
    (0..3)
        .map(|k| (max[k] - min[k]).powi(2))
        .sum::<f64>()
        .sqrt()
}

type Sample = (f64, [f64; 3]);

/// Halves the step from `a` to `b` until the curve's middle lies within `tolerance` of the chord's,
/// or the budget is spent. Pushes the samples strictly between.
fn refine(
    at: &impl Fn(f64) -> Result<[f64; 3], SpaceCurveError>,
    a: Sample,
    b: Sample,
    depth: u32,
    tolerance: f64,
    out: &mut Vec<Sample>,
) -> Result<(), SpaceCurveError> {
    if depth >= MAX_DEPTH || out.len() >= MAX_SAMPLES {
        return Ok(());
    }
    let u = (a.0 + b.0) / 2.0;
    let middle = (u, at(u)?);
    let off = (0..3)
        .map(|k| (middle.1[k] - (a.1[k] + b.1[k]) / 2.0).powi(2))
        .sum::<f64>()
        .sqrt();
    if off <= tolerance {
        return Ok(());
    }
    refine(at, a, middle, depth + 1, tolerance, out)?;
    out.push(middle);
    refine(at, middle, b, depth + 1, tolerance, out)
}

/// The normal of a curve's first frame: the one that leaves its third direction as near the
/// curve's own +z as it can be, or +y on a curve that starts along z. The reflections keep a flat
/// curve's third direction square to its plane, so a curve in the xy plane keeps it on +z.
fn first_normal(tangent: Vec3) -> Vec3 {
    let up = if tangent.cross(Vec3::Z).length_squared() > 1e-6 {
        Vec3::Z
    } else {
        Vec3::Y
    };
    up.cross(tangent).normalize()
}

/// The spans of a centripetal Catmull-Rom curve through its points, with the derivative it has at
/// each point per unit of the parameter the module names, which the spans on either side share.
#[derive(Debug)]
pub(crate) struct Spans {
    points: Vec<DVec3>,
    derivatives: Vec<DVec3>,
}

impl Spans {
    /// An open curve's ends take a point mirrored past them as their outer neighbor, so each end
    /// leaves along its chord. A closed curve's last point is its first.
    pub(crate) fn new(points: Vec<DVec3>, closed: bool) -> Spans {
        let n = points.len();
        let spacing = |a: DVec3, b: DVec3| a.distance(b).sqrt();
        let derivatives = (0..n)
            .map(|i| {
                let here = points[i];
                let before = match i {
                    0 if closed => points[n - 2],
                    0 => 2.0 * here - points[1],
                    _ => points[i - 1],
                };
                let after = match i {
                    _ if i + 1 < n => points[i + 1],
                    _ if closed => points[1],
                    _ => 2.0 * here - points[n - 2],
                };
                let (d0, d1) = (spacing(before, here), spacing(here, after));
                (here - before) / d0 - (after - before) / (d0 + d1) + (after - here) / d1
            })
            .collect();
        Spans {
            points,
            derivatives,
        }
    }

    /// The point and the derivative at `g`, which runs from 0 at the first point by 1 a span.
    pub(crate) fn at(&self, g: f64) -> (DVec3, DVec3) {
        let span = (g.floor() as usize).min(self.points.len() - 2);
        let s = g - span as f64;
        let (p0, p1) = (self.points[span], self.points[span + 1]);
        let spacing = p0.distance(p1).sqrt();
        let (m0, m1) = (
            self.derivatives[span] * spacing,
            self.derivatives[span + 1] * spacing,
        );
        let (s2, s3) = (s * s, s * s * s);
        let point = (2.0 * s3 - 3.0 * s2 + 1.0) * p0
            + (s3 - 2.0 * s2 + s) * m0
            + (3.0 * s2 - 2.0 * s3) * p1
            + (s3 - s2) * m1;
        let derivative = (6.0 * s2 - 6.0 * s) * p0
            + (3.0 * s2 - 4.0 * s + 1.0) * m0
            + (6.0 * s - 6.0 * s2) * p1
            + (3.0 * s2 - 2.0 * s) * m1;
        (point, derivative)
    }
}

/// The way the curve runs at each of `points`, between the pieces on either side of it.
fn chord_tangents(points: &[Vec3], closed: bool) -> Vec<Vec3> {
    let n = points.len();
    let piece = |i: usize| (points[i + 1] - points[i]).normalize();
    (0..n)
        .map(|i| {
            let before = if i > 0 {
                Some(piece(i - 1))
            } else if closed {
                Some(piece(n - 2))
            } else {
                None
            };
            let after = if i + 1 < n {
                Some(piece(i))
            } else if closed {
                Some(piece(0))
            } else {
                None
            };
            match (before, after) {
                // A cusp, where the curve turns straight back, takes the way it came.
                (Some(b), Some(a)) => (a + b).try_normalize().unwrap_or(b),
                (Some(one), None) | (None, Some(one)) => one,
                (None, None) => unreachable!("at least two points"),
            }
        })
        .collect()
}

/// The frame at each of `points`, running along `tangents`, carried from the first by double
/// reflection.
fn frames(points: &[Vec3], tangents: &[Vec3], lengths: &[f32], closed: bool) -> Vec<CurveFrame> {
    let n = points.len();
    let mut frames = Vec::with_capacity(n);
    frames.push(CurveFrame {
        point: points[0],
        tangent: tangents[0],
        normal: first_normal(tangents[0]),
    });
    for i in 0..n - 1 {
        let here = frames[i];
        let reflect =
            |v: Vec3, across: Vec3| v - across * (2.0 * across.dot(v) / across.length_squared());
        let step = points[i + 1] - points[i];
        let (normal, tangent) = (reflect(here.normal, step), reflect(here.tangent, step));
        let turn = tangents[i + 1] - tangent;
        let normal = if turn.length_squared() > 1e-12 {
            reflect(normal, turn)
        } else {
            normal
        };
        frames.push(CurveFrame {
            point: points[i + 1],
            tangent: tangents[i + 1],
            normal: normal
                .reject_from_normalized(tangents[i + 1])
                .normalize_or(here.normal),
        });
    }
    if closed {
        // The frames came round turned by `missing` about the tangent at the seam, so each is
        // turned back by its share of the length.
        let (first, last) = (frames[0], frames[n - 1]);
        let missing = last
            .normal
            .cross(first.normal)
            .dot(first.tangent)
            .atan2(last.normal.dot(first.normal));
        let total = lengths[n - 1];
        for (frame, &length) in frames.iter_mut().zip(lengths) {
            frame.normal =
                Quat::from_axis_angle(frame.tangent, missing * length / total) * frame.normal;
        }
        frames[n - 1] = frames[0];
    }
    frames
}

#[cfg(test)]
mod tests {
    use std::f32::consts::TAU;

    use super::*;
    use crate::expr::SPACE_CURVE;

    fn curve(
        x: &str,
        y: &str,
        z: &str,
        u_range: (f64, f64),
    ) -> Result<SpaceCurve, SpaceCurveError> {
        let parse = |source| Expression::parse(source, SPACE_CURVE).unwrap();
        SpaceCurve::sample(&[parse(x), parse(y), parse(z)], u_range, None)
    }

    #[test]
    fn a_helix_is_sampled_finely_enough_to_look_round_and_measures_its_length() {
        let helix = curve(
            "cos(u)",
            "0.1*u",
            "sin(u)",
            (0.0, 4.0 * std::f64::consts::PI),
        )
        .unwrap();

        let expected = 4.0 * std::f32::consts::PI * (1.0f32 + 0.01).sqrt();
        assert!(
            (helix.length() - expected).abs() < expected * 1e-3,
            "length {}",
            helix.length()
        );
        assert!(
            helix
                .frames
                .windows(2)
                .all(|pair| pair[0].tangent.angle_between(pair[1].tangent) < 6f32.to_radians()),
            "no piece turns more than about 5 degrees"
        );
        assert!(!helix.is_closed());
        assert_eq!(helix.start(), Vec3::new(1.0, 0.0, 0.0));
    }

    #[test]
    fn every_frame_is_square_to_the_curve() {
        let knot = curve(
            "sin(u) + 2*sin(2*u)",
            "cos(u) - 2*cos(2*u)",
            "-sin(3*u)",
            (0.0, std::f64::consts::TAU),
        )
        .unwrap();

        for frame in &knot.frames {
            assert!(
                (frame.normal.length() - 1.0).abs() < 1e-4
                    && (frame.tangent.length() - 1.0).abs() < 1e-4
            );
            assert!(frame.normal.dot(frame.tangent).abs() < 1e-4, "{frame:?}");
        }
    }

    #[test]
    fn a_frame_does_not_turn_along_a_straight_run_or_flip_at_an_inflection() {
        let s_curve = curve("u", "u^3", "0", (-1.0, 1.0)).unwrap();

        // A flat curve's frames turn only about the plane's normal, so each normal keeps its
        // angle with that normal, where a Frenet frame flips at the inflection.
        let first = s_curve.frames[0].normal.z;
        assert!(
            s_curve
                .frames
                .iter()
                .all(|frame| (frame.normal.z - first).abs() < 1e-4),
            "{:?}",
            s_curve
                .frames
                .iter()
                .map(|f| f.normal.z)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    #[allow(
        clippy::approx_constant,
        reason = "6.2832 is what an agent writes for 2 pi"
    )]
    fn a_closed_curve_meets_its_own_first_frame() {
        let trefoil = curve(
            "sin(u) + 2*sin(2*u)",
            "cos(u) - 2*cos(2*u)",
            "-sin(3*u)",
            (0.0, std::f64::consts::TAU),
        )
        .unwrap();

        assert!(trefoil.is_closed());
        assert_eq!(trefoil.frames[0], *trefoil.frames.last().unwrap());
        let circle = curve("cos(u)", "sin(u)", "0", (0.0, std::f64::consts::TAU)).unwrap();
        // A flat ring's frames turn only about the plane's normal, so it needs no correction and
        // each normal keeps its angle with the plane.
        let first = circle.frames[0].normal.z;
        assert!(
            circle
                .frames
                .iter()
                .all(|frame| (frame.normal.z - first).abs() < 1e-4),
            "{:?}",
            circle.frames.iter().map(|f| f.normal.z).collect::<Vec<_>>()
        );
        assert!((circle.length() - TAU).abs() < 1e-3);
        assert!(
            curve("cos(u)", "sin(u)", "0", (0.0, 6.2832))
                .unwrap()
                .is_closed(),
            "6.2832 for 2 pi closes"
        );
        assert!(
            !curve("cos(u)", "sin(u)", "0", (0.0, 6.2))
                .unwrap()
                .is_closed()
        );
    }

    #[test]
    fn a_curve_in_the_xy_plane_keeps_its_third_direction_on_z() {
        for flat in [
            curve("cos(u)", "sin(u)", "0", (0.0, 5.0)).unwrap(),
            curve("u", "u^3", "0", (-1.0, 1.0)).unwrap(),
        ] {
            assert!(
                flat.frames
                    .iter()
                    .all(|frame| frame.tangent.cross(frame.normal).distance(Vec3::Z) < 1e-4),
                "{:?}",
                flat.frames
                    .iter()
                    .map(|f| f.tangent.cross(f.normal))
                    .collect::<Vec<_>>()
            );
        }
        let up_z = curve("0", "0", "u", (0.0, 1.0)).unwrap();
        assert_eq!(
            up_z.frames[0].tangent.cross(up_z.frames[0].normal),
            Vec3::Y,
            "a curve along z takes +y"
        );
    }

    #[test]
    fn frames_up_to_a_length_end_exactly_there() {
        let line = curve("2*u", "0", "0", (0.0, 1.0)).unwrap();

        let frames = line.frames_up_to(0.5);
        assert_eq!(frames.last().unwrap().point, Vec3::new(0.5, 0.0, 0.0));
        assert_eq!(line.frames_up_to(5.0).len(), line.frames.len());
        assert_eq!(line.frame_at(0.5), *frames.last().unwrap());
        assert_eq!(line.frame_at(0.0).point, Vec3::ZERO);
        assert_eq!(
            line.frame_at(5.0).point,
            Vec3::new(2.0, 0.0, 0.0),
            "past the end, the end"
        );
    }

    #[test]
    fn a_coordinate_with_no_value_or_too_far_is_an_error_at_its_u() {
        assert_eq!(
            curve("u", "sqrt(u)", "0", (-1.0, 1.0)).unwrap_err(),
            SpaceCurveError::NoValue {
                coordinate: 1,
                u: -1.0
            }
        );
        assert_eq!(
            curve("u", "0", "1/(u - 1)", (0.0, 1.0)).unwrap_err(),
            SpaceCurveError::NoValue {
                coordinate: 2,
                u: 1.0
            }
        );
        assert!(matches!(
            curve("exp(u)", "0", "0", (0.0, 20.0)).unwrap_err(),
            SpaceCurveError::TooFar { coordinate: 0, .. }
        ));
        assert_eq!(
            curve("1", "2", "3", (0.0, 1.0)).unwrap_err(),
            SpaceCurveError::NoLength
        );
    }

    #[test]
    fn a_curve_through_points_passes_through_each_and_leaves_along_its_first_chord() {
        let points = [
            [0.0, 0.0, 0.0],
            [0.5, 0.0, 0.0],
            [1.0, 1.0, 0.5],
            [2.0, 0.5, -1.0],
        ];
        let chain = SpaceCurve::smooth_through(&points, None).unwrap();

        for [x, y, z] in points {
            let point = Vec3::new(x as f32, y as f32, z as f32);
            assert!(
                chain.frames.iter().any(|frame| frame.point == point),
                "passes through {point}"
            );
        }
        assert_eq!(chain.start(), Vec3::ZERO);
        assert_eq!(chain.end(), Vec3::new(2.0, 0.5, -1.0));
        assert_eq!(chain.frames[0].tangent, Vec3::X);
        assert!(
            chain
                .frames
                .last()
                .unwrap()
                .tangent
                .distance(Vec3::new(1.0, -0.5, -1.5).normalize())
                < 1e-6,
            "reaches its last point from the one before"
        );
        assert!(!chain.is_closed());
    }

    #[test]
    fn a_curve_through_unevenly_spaced_points_does_not_overshoot() {
        // A parameter that runs 1 a span would carry the curve past 1.1 between 1 and 1.1, and back.
        let line = SpaceCurve::smooth_through(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.1, 0.0, 0.0],
                [5.0, 0.0, 0.0],
            ],
            None,
        )
        .unwrap();

        assert!(
            line.frames
                .windows(2)
                .all(|pair| pair[1].point.x > pair[0].point.x),
            "{:?}",
            line.frames.iter().map(|f| f.point.x).collect::<Vec<_>>()
        );
        assert!(
            line.frames
                .iter()
                .all(|frame| frame.point.y == 0.0 && frame.point.z == 0.0)
        );
        assert!(
            (line.length() - 5.0).abs() < 1e-5,
            "length {}",
            line.length()
        );
    }

    #[test]
    fn a_curve_back_at_its_first_point_closes_smoothly() {
        let square = SpaceCurve::smooth_through(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
            ],
            None,
        )
        .unwrap();

        assert!(square.is_closed());
        assert_eq!(square.frames[0], *square.frames.last().unwrap());
        assert!(
            square.frames[0]
                .tangent
                .distance(Vec3::new(1.0, -1.0, 0.0).normalize())
                < 1e-6,
            "turns through its first point as through every other: {}",
            square.frames[0].tangent
        );
    }

    #[test]
    fn a_repeated_point_adds_nothing_and_one_point_has_no_length() {
        let once =
            SpaceCurve::smooth_through(&[[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 1.0]], None)
                .unwrap();
        let twice = SpaceCurve::smooth_through(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [2.0, 0.0, 1.0],
            ],
            None,
        )
        .unwrap();

        assert_eq!(once.frames, twice.frames);
        assert_eq!(
            SpaceCurve::smooth_through(&[[1.0, 2.0, 3.0], [1.0, 2.0, 3.0]], None).unwrap_err(),
            SpaceCurveError::NoLength
        );
    }

    #[test]
    fn a_point_at_u_is_the_expressions_at_u_and_a_curve_point_keeps_its_u() {
        let parabola = curve("u", "u^2", "0", (0.0, 2.0)).unwrap();
        let through = SpaceCurve::smooth_through(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [1.0, 1.0, 0.0],
                [5.0, 0.0, 0.0],
            ],
            None,
        )
        .unwrap();

        assert_eq!(parabola.u_range(), (0.0, 2.0));
        assert!(
            parabola.point_at_u(1.5).distance(Vec3::new(1.5, 2.25, 0.0)) < 1e-3,
            "{}",
            parabola.point_at_u(1.5)
        );
        assert_eq!(through.u_range(), (0.0, 1.0));
        assert_eq!(through.point_at_u(0.0), Vec3::ZERO);
        assert_eq!(
            through.point_at_u(2.0 / 3.0),
            Vec3::new(1.0, 1.0, 0.0),
            "the repeated point is at the u of the last of its copies"
        );
        assert_eq!(through.point_at_u(1.0), Vec3::new(5.0, 0.0, 0.0));
    }
}
