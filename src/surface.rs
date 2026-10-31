//! A surface: its height z as an expression in `x` and `y` over a rectangle, sampled once when the
//! scene file is read on a tensor grid, finer where it bends. The rectangle lies on the object's
//! floor, its x and y read as x and -z as a prism's base is, and the height rises along +y.
//!
//! On 3D axes the surface is written in their data coordinates, which they scale per axis, and it
//! is cut at their box: where the sheet leaves the z range, `mesh` cuts it along the range's planes.
//!
//! A tensor grid is one sorted list of x and one of y, with a height at every pair. An interval is
//! split along its whole line, so neighboring cells always share their corners: the sheet has no
//! cracks and no T-junctions, which a quadtree would have where a fine cell meets a coarse one.

use glam::{Vec2, Vec3};

use crate::axes_3d::DataToOwn;
use crate::expr::Expression;
use crate::space_curve::FARTHEST;

/// A surface's heights on its grid, as the scene file writes them, with the slope at each point,
/// and how they land in the object's own space.
#[derive(Debug)]
pub struct Surface {
    /// Low first.
    xs: Vec<f32>,
    /// Low first.
    ys: Vec<f32>,
    /// The height at `(xs[i], ys[j])` is `heights[j * xs.len() + i]`. On 3D axes a height far past
    /// the z range, or with no end, is held a little past it, where the cut takes it away.
    heights: Vec<f32>,
    /// dz/dx and dz/dy at each point, in the order of `heights`.
    slopes: Vec<Vec2>,
    place: Placing,
    sampling: Sampling,
}

/// The x and the y a surface's heights are sampled at, each low first. A surface whose height
/// reads law time is sampled at the same points at every scene time, so its triangles do not change
/// from frame to frame and its edges do not shimmer.
#[derive(Clone, Debug, PartialEq)]
pub struct Sampling {
    xs: Vec<f64>,
    ys: Vec<f64>,
}

impl Sampling {
    /// Every x and every y of both samplings, which share their ranges. An axis past `MAX_INTERVALS`
    /// intervals is split evenly into that many instead.
    pub fn union(&self, other: &Sampling) -> Sampling {
        let merge = |a: &[f64], b: &[f64]| {
            let mut values: Vec<f64> = a.iter().chain(b).copied().collect();
            values.sort_by(f64::total_cmp);
            values.dedup();
            if values.len() > MAX_INTERVALS + 1 {
                let (lo, hi) = (values[0], values[values.len() - 1]);
                values = (0..=MAX_INTERVALS)
                    .map(|k| {
                        if k == MAX_INTERVALS {
                            hi
                        } else {
                            lo + (hi - lo) * k as f64 / MAX_INTERVALS as f64
                        }
                    })
                    .collect();
            }
            values
        };
        Sampling {
            xs: merge(&self.xs, &other.xs),
            ys: merge(&self.ys, &other.ys),
        }
    }
}

/// Where a surface's data coordinates land in the object's own space, and on 3D axes the z range
/// it is cut at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placing {
    pub to_own: DataToOwn,
    /// The data z range of 3D axes, low first. A create rises the sheet from 0 in it, or from its
    /// nearest end.
    pub cut: Option<(f64, f64)>,
}

impl Placing {
    /// A surface placed by its own transform: data x along x, z up, y along -z, in world units.
    pub const OWN: Placing = Placing {
        to_own: DataToOwn::IDENTITY,
        cut: None,
    };

    /// The data z a create rises the sheet from.
    fn base(&self) -> f32 {
        self.cut
            .map_or(0.0, |(low, high)| 0.0f64.clamp(low, high) as f32)
    }
}

/// How far past each end of the z range of 3D axes a height is held, in shares of the range: far
/// enough that the cut between a held height and one inside lands within a tenth of a cell of
/// where the sheet leaves the box.
const HELD_PAST: f64 = 10.0;

#[derive(Debug, PartialEq)]
pub enum SurfaceError {
    /// The height has no finite value at `(x, y)`.
    NoValue { x: f64, y: f64 },
    /// The height lies farther than `FARTHEST` from the object's position at `(x, y)`.
    TooFar { x: f64, y: f64, value: f64 },
}

/// How many even intervals each axis starts with before it refines.
const FIRST_INTERVALS: usize = 32;
/// The most intervals an axis is split into.
const MAX_INTERVALS: usize = 256;
/// How far the sheet may sit from the middle of an interval's two ends, as a share of the
/// diagonal of its box.
const SAMPLE_TOLERANCE: f64 = 5e-4;

impl Surface {
    /// Samples `height`, z in `x` and `y`, over `x_range` and `y_range`, each low first. The
    /// sheet is sampled finer where it bends as `place` draws it, in the object's own units.
    pub fn sample(
        height: &Expression,
        x_range: (f64, f64),
        y_range: (f64, f64),
        place: Placing,
    ) -> Result<Surface, SurfaceError> {
        let at = height_at(height, place);
        let even = |(lo, hi): (f64, f64)| -> Vec<f64> {
            (0..=FIRST_INTERVALS)
                .map(|k| {
                    if k == FIRST_INTERVALS {
                        hi
                    } else {
                        lo + (hi - lo) * k as f64 / FIRST_INTERVALS as f64
                    }
                })
                .collect()
        };
        let (mut xs, mut ys) = (even(x_range), even(y_range));
        let mut heights = heights_at(&at, &xs, &ys)?;
        let (lo, hi) = heights
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), &z| (lo.min(z), hi.max(z)));
        let (lo, hi) = place.cut.map_or((lo, hi), |(low, high)| {
            (lo.max(low), hi.min(high).max(lo.max(low)))
        });
        let [sx, sy, sz] = place.to_own.scale;
        let size = (((x_range.1 - x_range.0) * sx).powi(2)
            + ((hi - lo) * sz).powi(2)
            + ((y_range.1 - y_range.0) * sy).powi(2))
        .sqrt();
        let tolerance = size * SAMPLE_TOLERANCE;
        loop {
            let x_splits = splits(
                &xs,
                &ys,
                &at,
                |i, j| heights[j * xs.len() + i],
                sz,
                tolerance,
            )?;
            let y_splits = splits(
                &ys,
                &xs,
                |y, x| at(x, y),
                |j, i| heights[j * xs.len() + i],
                sz,
                tolerance,
            )?;
            if x_splits.is_empty() && y_splits.is_empty() {
                break;
            }
            xs = split(&xs, &x_splits);
            ys = split(&ys, &y_splits);
            heights = heights_at(&at, &xs, &ys)?;
        }
        Ok(Surface::on(Sampling { xs, ys }, heights, place))
    }

    /// Samples `height` at every point of `sampling`, with no refinement, so two scene times of
    /// one surface share their triangles.
    pub fn sample_on(
        height: &Expression,
        sampling: &Sampling,
        place: Placing,
    ) -> Result<Surface, SurfaceError> {
        let at = height_at(height, place);
        let heights = heights_at(&at, &sampling.xs, &sampling.ys)?;
        Ok(Surface::on(sampling.clone(), heights, place))
    }

    fn on(sampling: Sampling, heights: Vec<f64>, place: Placing) -> Surface {
        let slopes = slopes(&sampling.xs, &sampling.ys, &heights);
        let narrow = |values: &[f64]| values.iter().map(|&v| v as f32).collect();
        Surface {
            xs: narrow(&sampling.xs),
            ys: narrow(&sampling.ys),
            heights: narrow(&heights),
            slopes,
            place,
            sampling,
        }
    }

    /// The x and y the surface is sampled at.
    pub fn sampling(&self) -> &Sampling {
        &self.sampling
    }

    /// How many points the grid has along x and along y.
    pub fn counts(&self) -> (usize, usize) {
        (self.xs.len(), self.ys.len())
    }

    /// The point at `(i, j)` of the grid in the object's own space, with its height risen `rise`
    /// of the way from the height a create starts at, and its normal there, toward +y: data x
    /// along x, the height along y, and data y along -z.
    pub fn point(&self, i: usize, j: usize, rise: f32) -> (Vec3, Vec3) {
        let k = j * self.xs.len() + i;
        let slope = self.slopes[k] * rise;
        let base = self.place.base();
        let height = base + (self.heights[k] - base) * rise;
        let [sx, sy, sz] = self.place.to_own.scale;
        let normal =
            Vec3::new(-slope.x * (sz / sx) as f32, 1.0, slope.y * (sz / sy) as f32).normalize();
        (
            self.place.to_own.own([
                f64::from(self.xs[i]),
                f64::from(self.ys[j]),
                f64::from(height),
            ]),
            normal,
        )
    }

    /// On 3D axes, the lowest and highest y of the object's own space the sheet is cut at.
    pub fn cut(&self) -> Option<(f32, f32)> {
        self.place.cut.map(|(low, high)| {
            (
                self.place.to_own.own_height(low),
                self.place.to_own.own_height(high),
            )
        })
    }

    /// How far the farthest corner of the sheet's box lies from the object's position.
    pub fn reach(&self) -> f32 {
        let (lo, hi) = self
            .heights
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), &z| (lo.min(z), hi.max(z)));
        let ends = |values: &[f32]| [values[0], values[values.len() - 1]].map(f64::from);
        let ([x0, x1], [y0, y1]) = (ends(&self.xs), ends(&self.ys));
        let (a, b) = (
            self.place.to_own.own([x0, y0, f64::from(lo)]),
            self.place.to_own.own([x1, y1, f64::from(hi)]),
        );
        a.abs().max(b.abs()).length()
    }
}

/// The height at `(x, y)`, or why there is none. On 3D axes a height far past the z range is held
/// a little past it.
fn height_at(
    height: &Expression,
    place: Placing,
) -> impl Fn(f64, f64) -> Result<f64, SurfaceError> + '_ {
    move |x, y| {
        let value = height.eval(&[x, y]);
        match place.cut {
            _ if value.is_nan() => Err(SurfaceError::NoValue { x, y }),
            Some((low, high)) => Ok(value.clamp(
                low - HELD_PAST * (high - low),
                high + HELD_PAST * (high - low),
            )),
            None if !value.is_finite() => Err(SurfaceError::NoValue { x, y }),
            None if value.abs() > FARTHEST => Err(SurfaceError::TooFar { x, y, value }),
            None => Ok(value),
        }
    }
}

/// The height at every `(x, y)` of the grid, rows of x one y at a time.
fn heights_at(
    at: &impl Fn(f64, f64) -> Result<f64, SurfaceError>,
    xs: &[f64],
    ys: &[f64],
) -> Result<Vec<f64>, SurfaceError> {
    ys.iter()
        .flat_map(|&y| xs.iter().map(move |&x| at(x, y)))
        .collect()
}

/// The intervals of `along` to split at their middles, by index: each one whose middle, on some
/// line of `across`, lies farther than `tolerance` from the mean of its two ends, in heights
/// scaled by `height_scale`, worst first, as many as `MAX_INTERVALS` allows. `at(a, c)` is the
/// height at `a` along and `c` across, and `known(i, j)` the height already sampled at
/// `along[i]`, `across[j]`.
fn splits(
    along: &[f64],
    across: &[f64],
    at: impl Fn(f64, f64) -> Result<f64, SurfaceError>,
    known: impl Fn(usize, usize) -> f64,
    height_scale: f64,
    tolerance: f64,
) -> Result<Vec<usize>, SurfaceError> {
    let room = MAX_INTERVALS.saturating_sub(along.len() - 1);
    if room == 0 {
        return Ok(Vec::new());
    }
    let mut off: Vec<(f64, usize)> = Vec::new();
    for i in 0..along.len() - 1 {
        let middle = (along[i] + along[i + 1]) / 2.0;
        let mut worst = 0.0f64;
        for (j, &c) in across.iter().enumerate() {
            worst = worst
                .max((at(middle, c)? - (known(i, j) + known(i + 1, j)) / 2.0).abs() * height_scale);
        }
        if worst > tolerance {
            off.push((worst, i));
        }
    }
    off.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut chosen: Vec<usize> = off.into_iter().take(room).map(|(_, i)| i).collect();
    chosen.sort_unstable();
    Ok(chosen)
}

/// `values` with the middle of each interval in `intervals`, sorted, added after its start.
fn split(values: &[f64], intervals: &[usize]) -> Vec<f64> {
    let mut out = Vec::with_capacity(values.len() + intervals.len());
    let mut next = intervals.iter().peekable();
    for (i, &value) in values.iter().enumerate() {
        out.push(value);
        if next.next_if_eq(&&i).is_some() {
            out.push((value + values[i + 1]) / 2.0);
        }
    }
    out
}

/// dz/dx and dz/dy at every point: central differences across its two neighbors, and one-sided at
/// the first and last point of each line.
fn slopes(xs: &[f64], ys: &[f64], heights: &[f64]) -> Vec<Vec2> {
    let (nx, ny) = (xs.len(), ys.len());
    let difference = |values: &[f64], n: usize, k: usize, height: &dyn Fn(usize) -> f64| {
        let (a, b) = (k.saturating_sub(1), (k + 1).min(n - 1));
        ((height(b) - height(a)) / (values[b] - values[a])) as f32
    };
    (0..ny)
        .flat_map(|j| (0..nx).map(move |i| (i, j)))
        .map(|(i, j)| {
            Vec2::new(
                difference(xs, nx, i, &|i| heights[j * nx + i]),
                difference(ys, ny, j, &|j| heights[j * nx + i]),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::SURFACE;

    fn surface(z: &str, x_range: (f64, f64), y_range: (f64, f64)) -> Result<Surface, SurfaceError> {
        Surface::sample(
            &Expression::parse(z, SURFACE).unwrap(),
            x_range,
            y_range,
            Placing::OWN,
        )
    }

    #[test]
    fn a_flat_slope_needs_no_more_than_its_first_samples() {
        let slope = surface("x + 2*y", (-1.0, 1.0), (0.0, 4.0)).unwrap();

        assert_eq!(slope.counts(), (33, 33));
        let (point, normal) = slope.point(32, 0, 1.0);
        assert_eq!(point, Vec3::new(1.0, 1.0, 0.0));
        let (far, _) = slope.point(0, 32, 1.0);
        assert_eq!(
            far,
            Vec3::new(-1.0, 7.0, -4.0),
            "data y runs along -z, away from the default camera"
        );
        assert!(
            (normal - Vec3::new(-1.0, 1.0, 2.0).normalize()).length() < 1e-6,
            "{normal}"
        );
    }

    #[test]
    fn a_narrow_well_is_sampled_finer_near_its_bottom_on_both_axes() {
        let well = surface("-2*exp(-(x^2 + y^2)/0.02)", (-3.0, 3.0), (-3.0, 3.0)).unwrap();

        let (nx, ny) = well.counts();
        assert!(nx > 33 && ny > 33 && nx <= 257 && ny <= 257, "{nx} by {ny}");
        let near = |values: &[f32]| {
            values
                .windows(2)
                .filter(|w| w[0].abs() < 0.3 && w[1].abs() < 0.3)
                .count()
        };
        assert!(
            near(&well.xs) > 8 && near(&well.ys) > 8,
            "fine near 0: {:?}",
            well.xs
        );
        assert!(well.xs.windows(2).all(|w| w[0] < w[1]) && well.ys.windows(2).all(|w| w[0] < w[1]));
        let bottom = well.heights.iter().copied().fold(f32::MAX, f32::min);
        assert!((bottom + 2.0).abs() < 0.01, "{bottom}");
    }

    #[test]
    fn an_axis_stops_splitting_at_its_most_intervals() {
        let comb = surface("sin(200*x)", (0.0, 10.0), (0.0, 1.0)).unwrap();

        assert_eq!(comb.counts(), (MAX_INTERVALS + 1, 33));
    }

    #[test]
    fn the_slopes_are_central_differences_and_one_sided_at_each_end() {
        let bowl = surface("x^2", (0.0, 1.0), (0.0, 1.0)).unwrap();

        let (_, inside) = bowl.point(16, 0, 1.0);
        assert!(
            (inside - Vec3::new(-1.0, 1.0, 0.0).normalize()).length() < 1e-4,
            "dz/dx is 2x = 1 at x = 0.5: {inside}"
        );
        let (_, flat) = bowl.point(16, 0, 0.0);
        assert_eq!(flat, Vec3::Y, "a sheet risen none is flat");
    }

    #[test]
    fn a_height_with_no_value_or_too_far_is_an_error_at_its_x_and_y() {
        assert_eq!(
            surface("sqrt(x)", (-1.0, 1.0), (0.0, 1.0)).unwrap_err(),
            SurfaceError::NoValue { x: -1.0, y: 0.0 }
        );
        assert_eq!(
            surface("exp(y)", (0.0, 1.0), (0.0, 20.0)).unwrap_err(),
            SurfaceError::TooFar {
                x: 0.0,
                y: 20.0 * 15.0 / 32.0,
                value: (20.0f64 * 15.0 / 32.0).exp()
            }
        );
    }
}
