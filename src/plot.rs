//! A plot's curve as a `Path` in its axes' own space.
//!
//! A function plot samples its expression, finer where the curve bends, and a point plot takes
//! the points it is given. Either way the samples are cut to the axes' box, so a curve that
//! leaves the box ends at its edge, and a curve that has no value, or jumps from one side of the
//! box to the other, leaves a gap. What is left becomes lines, one part per unbroken run, and
//! reaches `mesh` like any other flat shape.

use glam::{Mat2, Mat4, Vec2, Vec3};
use kurbo::{BezPath, Point, Rect};
use serde::Serialize;

use crate::axes::{Axes, Labelled};
use crate::expr::Expression;
use crate::path::{Path, PathError};
use crate::scene::{Camera, Glyph, ObjectId, SceneConfig};

#[derive(Debug, PartialEq)]
pub enum PlotError {
    /// The plot's `x_range` has no value inside the axes' x range.
    OutsideAxes,
    /// The curve has no value inside the axes' box: the expression is NaN or out of the box at
    /// every sample.
    NothingInside,
    /// The path past the cut cannot be drawn.
    Path(PathError),
}

/// How many samples a function starts with before it refines.
const FIRST_SAMPLES: usize = 64;
/// How many times an interval may be halved.
const MAX_DEPTH: u32 = 12;
/// The most samples a function plot makes.
const MAX_SAMPLES: usize = 20_000;
/// How far a curve may sit from its straight pieces, as a share of the axes' height.
const SAMPLE_TOLERANCE: f64 = 5e-4;
/// A step halved `MAX_DEPTH` times that still rises or falls by this share of the axes' height
/// is a jump, as `step(x)` makes at 0 and `mod(x, 1)` at each whole x, and the curve breaks there.
const JUMP_SHARE: f64 = 1e-2;

/// The graph of `function` over `x_range` (clipped to the axes' own x range), in the axes' local
/// space.
pub fn function_path(
    axes: &Axes,
    function: &Expression,
    x_range: (f64, f64),
) -> Result<Path, PlotError> {
    let (axis_min, axis_max) = axes.x.range();
    let (lo, hi) = (x_range.0.max(axis_min), x_range.1.min(axis_max));
    if lo >= hi {
        return Err(PlotError::OutsideAxes);
    }
    let (y_min, y_max) = axes.y.range();
    let height = y_max - y_min;
    let mut samples: Vec<(f64, f64)> = Vec::new();
    let steps = FIRST_SAMPLES;
    let at = |k: usize| lo + (hi - lo) * k as f64 / steps as f64;
    samples.push((at(0), function.eval(&[at(0)])));
    for k in 1..=steps {
        let b = (at(k), function.eval(&[at(k)]));
        refine(
            function,
            *samples.last().expect("a first sample"),
            b,
            0,
            (y_min, y_max),
            height,
            &mut samples,
        );
        samples.push(b);
    }
    runs_to_path(axes, clip(&samples, axes))
}

/// Subdivides `a` to `b` until the curve between them is straight to within the tolerance, a
/// whole box-crossing step is a jump, or the budget is spent. Pushes the samples strictly between,
/// and a NaN sample, which breaks the curve, where a step halved to the limit still jumps.
fn refine(
    f: &Expression,
    a: (f64, f64),
    b: (f64, f64),
    depth: u32,
    y_range: (f64, f64),
    height: f64,
    out: &mut Vec<(f64, f64)>,
) {
    let (a_known, b_known) = (a.1.is_finite(), b.1.is_finite());
    if !a_known && !b_known {
        return;
    }
    let above = |y: f64| y > y_range.1;
    let below = |y: f64| y < y_range.0;
    let same_side_outside =
        a_known && b_known && ((above(a.1) && above(b.1)) || (below(a.1) && below(b.1)));
    if same_side_outside || out.len() >= MAX_SAMPLES {
        return;
    }
    if depth >= MAX_DEPTH {
        if a_known && b_known && (b.1 - a.1).abs() > JUMP_SHARE * height {
            out.push(((a.0 + b.0) / 2.0, f64::NAN));
        }
        return;
    }
    let mid_x = (a.0 + b.0) / 2.0;
    let mid = (mid_x, f.eval(&[mid_x]));
    let opposite_outside =
        a_known && b_known && ((above(a.1) && below(b.1)) || (below(a.1) && above(b.1)));
    let straight = a_known
        && b_known
        && mid.1.is_finite()
        && (mid.1 - (a.1 + b.1) / 2.0).abs() <= SAMPLE_TOLERANCE * height;
    if straight && !opposite_outside {
        return;
    }
    refine(f, a, mid, depth + 1, y_range, height, out);
    out.push(mid);
    refine(f, mid, b, depth + 1, y_range, height, out);
}

/// The line through `points` in order, in data coordinates, clipped to the axes' box.
pub fn points_path(axes: &Axes, points: &[(f64, f64)]) -> Result<Path, PlotError> {
    runs_to_path(axes, clip(points, axes))
}

/// Splits the polyline `samples` into runs inside the axes' box. A NaN sample ends a run, and
/// so does a step that goes from one side of the box to the other with both ends outside it,
/// which is a jump and not a steep curve because refinement has already halved it to the limit.
fn clip(samples: &[(f64, f64)], axes: &Axes) -> Vec<Vec<(f64, f64)>> {
    let (x0, x1) = axes.x.range();
    let (y0, y1) = axes.y.range();
    let mut runs: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut run: Vec<(f64, f64)> = Vec::new();
    let end_run = |run: &mut Vec<(f64, f64)>, runs: &mut Vec<Vec<(f64, f64)>>| {
        if run.len() >= 2 {
            runs.push(std::mem::take(run));
        } else {
            run.clear();
        }
    };
    for pair in samples.windows(2) {
        let (p, q) = (pair[0], pair[1]);
        if !(p.0.is_finite() && p.1.is_finite() && q.0.is_finite() && q.1.is_finite()) {
            end_run(&mut run, &mut runs);
            continue;
        }
        let jump = (p.1 > y1 && q.1 < y0) || (p.1 < y0 && q.1 > y1);
        match (jump, clip_segment(p, q, (x0, y0, x1, y1))) {
            (false, Some((a, b))) => {
                if run.last() != Some(&a) {
                    end_run(&mut run, &mut runs);
                    run.push(a);
                }
                run.push(b);
                if b != q {
                    end_run(&mut run, &mut runs);
                }
            }
            _ => end_run(&mut run, &mut runs),
        }
    }
    end_run(&mut run, &mut runs);
    runs
}

/// Liang-Barsky: the part of the segment `p` to `q` inside `(x0, y0, x1, y1)`.
fn clip_segment(
    p: (f64, f64),
    q: (f64, f64),
    (x0, y0, x1, y1): (f64, f64, f64, f64),
) -> Option<((f64, f64), (f64, f64))> {
    let (dx, dy) = (q.0 - p.0, q.1 - p.1);
    let (mut enter, mut leave) = (0.0f64, 1.0f64);
    for (direction, distance) in [
        (-dx, p.0 - x0),
        (dx, x1 - p.0),
        (-dy, p.1 - y0),
        (dy, y1 - p.1),
    ] {
        if direction == 0.0 {
            if distance < 0.0 {
                return None;
            }
        } else {
            let along = distance / direction;
            if direction < 0.0 {
                enter = enter.max(along);
            } else {
                leave = leave.min(along);
            }
        }
    }
    (enter <= leave).then(|| {
        let at = |along: f64| {
            if along == 0.0 {
                p
            } else if along == 1.0 {
                q
            } else {
                (p.0 + dx * along, p.1 + dy * along)
            }
        };
        (at(enter), at(leave))
    })
}

fn runs_to_path(axes: &Axes, runs: Vec<Vec<(f64, f64)>>) -> Result<Path, PlotError> {
    let mut bez = BezPath::new();
    for run in &runs {
        let mut local = run
            .iter()
            .map(|&p| axes.to_local(p))
            .map(|v: Vec2| kurbo::Point::new(f64::from(v.x), f64::from(v.y)));
        bez.move_to(local.next().expect("a run has points"));
        local.for_each(|p| bez.line_to(p));
    }
    if runs.is_empty() {
        return Err(PlotError::NothingInside);
    }
    Path::from_bez(bez).map_err(PlotError::Path)
}

/// What `inspect` tells an agent about a plot: its axes, the box of data values its curve
/// spans, which is smaller than the axes' ranges where the curve is cut or does not reach, the
/// tick labels and axis labels of its axes that its stroke crosses, and the shown text and math
/// objects whose glyphs it crosses on the frame, none while the plot itself does not show.
#[derive(Debug, Serialize)]
pub struct PlotReport {
    pub axes: ObjectId,
    pub data_bounding_box: DataBox,
    pub crosses_labels: Vec<Labelled>,
    pub crosses_objects: Vec<String>,
}

/// A text or math object that shows at a scene time, at an object opacity and a created share
/// above 0: its glyphs, and the matrix that takes its own plane to the world.
pub struct ShownText<'a> {
    pub object_id: String,
    pub glyphs: &'a [Glyph],
    pub object_to_world: Mat4,
}

/// Where a plot is on the frame: the matrix that takes its axes' plane to the world, and the
/// camera that takes the world to pixels.
pub struct OnFrame<'a> {
    pub plot_to_world: Mat4,
    pub camera: Camera,
    pub scene_config: &'a SceneConfig,
}

#[derive(Debug, Serialize)]
pub struct DataBox {
    pub min: [f64; 2],
    pub max: [f64; 2],
}

/// `stroke_width` is in world units, and `matrix` takes the axes' own plane to world lengths in
/// it: the axes' scale, or with their plane map, which the plot shares.
pub fn report(
    axes_id: &ObjectId,
    axes: &Axes,
    path: &Path,
    stroke_width: f32,
    matrix: Mat2,
    on_frame: &OnFrame,
    texts: &[ShownText],
) -> PlotReport {
    let bounding_box = path.bounding_box();
    let data = |x: f64, y: f64| {
        let (x, y) = axes.to_data(Vec2::new(x as f32, y as f32));
        [x, y]
    };
    PlotReport {
        axes: axes_id.clone(),
        data_bounding_box: DataBox {
            min: data(bounding_box.x0, bounding_box.y0),
            max: data(bounding_box.x1, bounding_box.y1),
        },
        crosses_labels: crossed_labels(axes, path, stroke_width, matrix),
        crosses_objects: crossed_objects(path, stroke_width, matrix, on_frame, texts),
    }
}

/// The object ids of `texts` whose glyphs the stroke along `path` enters on the frame, in the
/// order of `texts`. The test runs in pixels, where a plot and a text in different planes, or
/// placed by different objects, meet as the frame shows them. A flat object facing the camera
/// maps onto pixels by one scale and a shift, so there the test is exact: each straight piece
/// of the stroke reaches half the stroke's width at its depth, and each glyph is its box. A
/// curve can pass between two glyphs without crossing the object.
fn crossed_objects(
    path: &Path,
    stroke_width: f32,
    matrix: Mat2,
    on_frame: &OnFrame,
    texts: &[ShownText],
) -> Vec<String> {
    let OnFrame {
        plot_to_world,
        camera,
        scene_config,
    } = *on_frame;
    let to_pixels = |world: Vec3| {
        camera
            .to_pixels(world, scene_config)
            .map(|p| Point::new(f64::from(p.x), f64::from(p.y)))
    };
    let half_width = stroke_width / 2.0;
    let mut pieces: Vec<(Point, Point, f64)> = Vec::new();
    let mut last = Vec3::ZERO;
    let mut piece_to = |from: Vec3, to: Vec3| {
        let radius =
            f64::from(half_width * camera.pixels_per_unit((from + to) / 2.0, scene_config));
        if let (Some(p), Some(q)) = (to_pixels(from), to_pixels(to)) {
            pieces.push((p, q, radius));
        }
    };
    let world = |p: Point| plot_to_world.transform_point3(Vec3::new(p.x as f32, p.y as f32, 0.0));
    kurbo::flatten(
        path.elements().iter().copied(),
        CROSSING_TOLERANCE / largest_entry(matrix).max(f64::MIN_POSITIVE),
        |element| match element {
            kurbo::PathEl::MoveTo(p) => last = world(p),
            kurbo::PathEl::LineTo(p) => {
                let p = world(p);
                piece_to(last, p);
                last = p;
            }
            _ => {}
        },
    );
    texts
        .iter()
        .filter(|text| {
            text.glyphs.iter().any(|glyph| {
                let b = glyph.path.bounding_box();
                let corners =
                    [(b.x0, b.y0), (b.x1, b.y0), (b.x1, b.y1), (b.x0, b.y1)].map(|(x, y)| {
                        to_pixels(
                            text.object_to_world
                                .transform_point3(Vec3::new(x as f32, y as f32, 0.0)),
                        )
                    });
                let Some(quad) = corners.into_iter().collect::<Option<Vec<Point>>>() else {
                    return false;
                };
                pieces
                    .iter()
                    .any(|&(p, q, radius)| piece_meets_quad((p, q), &quad, radius))
            })
        })
        .map(|text| text.object_id.clone())
        .collect()
}

/// Whether a disc of `radius` moved along the piece meets the convex quadrilateral `quad`:
/// whether the piece starts inside it or passes within the radius of one of its edges.
fn piece_meets_quad(piece: (Point, Point), quad: &[Point], radius: f64) -> bool {
    let edges = || (0..quad.len()).map(|k| (quad[k], quad[(k + 1) % quad.len()]));
    let turns: Vec<f64> = edges().map(|(a, b)| (b - a).cross(piece.0 - a)).collect();
    let has_area = turns.iter().any(|&turn| turn != 0.0);
    let inside = has_area
        && (turns.iter().all(|&turn| turn >= 0.0) || turns.iter().all(|&turn| turn <= 0.0));
    inside || edges().any(|edge| segment_distance(piece, edge) <= radius)
}

/// The shortest distance between two straight segments: 0 where they cross, and otherwise from
/// an end of one to the other.
fn segment_distance((p, q): (Point, Point), (a, b): (Point, Point)) -> f64 {
    let side = |from: Point, to: Point, point: Point| (to - from).cross(point - from);
    let crosses = side(p, q, a) * side(p, q, b) < 0.0 && side(a, b, p) * side(a, b, q) < 0.0;
    let to_segment = |point: Point, (from, to): (Point, Point)| {
        let along = to - from;
        let share = if along.hypot2() == 0.0 {
            0.0
        } else {
            ((point - from).dot(along) / along.hypot2()).clamp(0.0, 1.0)
        };
        (from + along * share).distance(point)
    };
    if crosses {
        0.0
    } else {
        [
            to_segment(p, (a, b)),
            to_segment(q, (a, b)),
            to_segment(a, (p, q)),
            to_segment(b, (p, q)),
        ]
        .into_iter()
        .fold(f64::INFINITY, f64::min)
    }
}

/// How far a curve may sit from the straight pieces it is measured by, in world units.
const CROSSING_TOLERANCE: f64 = 1e-3;

/// The largest size of an entry of `matrix`, which for the axes' scale is its largest factor.
fn largest_entry(matrix: Mat2) -> f64 {
    f64::from(matrix.abs().to_cols_array().into_iter().fold(0.0, f32::max))
}

/// What the labels of `axes` label, for each label whose box the stroke along `path` enters, in
/// the order the axes lay their labels out. The test runs in the axes' plane taken by `matrix`,
/// where lengths are world lengths whatever the rotation, so the stroke is `stroke_width` wide
/// there. It treats each straight piece of the stroke as reaching half the width from it in
/// every direction, which overstates only the square cut at the two ends of a part. A label's
/// box is the box around its four corners in that plane, which a shear widens.
fn crossed_labels(axes: &Axes, path: &Path, stroke_width: f32, matrix: Mat2) -> Vec<Labelled> {
    let [a, c, b, d] = matrix.to_cols_array().map(f64::from);
    let to_world_lengths = |p: Point| Point::new(a * p.x + b * p.y, c * p.x + d * p.y);
    let mut pieces: Vec<(Point, Point)> = Vec::new();
    let mut last = Point::ZERO;
    kurbo::flatten(
        path.elements().iter().copied(),
        CROSSING_TOLERANCE / largest_entry(matrix).max(f64::MIN_POSITIVE),
        |element| match element {
            kurbo::PathEl::MoveTo(p) => last = to_world_lengths(p),
            kurbo::PathEl::LineTo(p) => {
                let p = to_world_lengths(p);
                pieces.push((last, p));
                last = p;
            }
            _ => {}
        },
    );
    let half_width = f64::from(stroke_width) / 2.0;
    axes.drawing
        .labels
        .iter()
        .filter(|label| {
            let b = label.bounding_box;
            let corners = [(b.x0, b.y0), (b.x1, b.y0), (b.x0, b.y1), (b.x1, b.y1)]
                .map(|(x, y)| to_world_lengths(Point::new(x, y)));
            let (x0, x1) = corners
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                    (lo.min(p.x), hi.max(p.x))
                });
            let (y0, y1) = corners
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), p| {
                    (lo.min(p.y), hi.max(p.y))
                });
            pieces
                .iter()
                .any(|&piece| piece_reaches(piece, Rect::new(x0, y0, x1, y1), half_width))
        })
        .flat_map(|label| label.of.iter().cloned())
        .collect()
}

/// Whether a disc of `radius` moved along the piece meets `bounding_box`: whether the piece enters
/// the box widened by the radius across or along, or passes within the radius of a corner.
fn piece_reaches((p, q): (Point, Point), bounding_box: Rect, radius: f64) -> bool {
    let enters = |r: Rect| clip_segment((p.x, p.y), (q.x, q.y), (r.x0, r.y0, r.x1, r.y1)).is_some();
    let near_corner = |corner: Point| {
        let along = q - p;
        let share = if along.hypot2() == 0.0 {
            0.0
        } else {
            ((corner - p).dot(along) / along.hypot2()).clamp(0.0, 1.0)
        };
        (p + along * share).distance(corner) <= radius
    };
    enters(Rect::new(
        bounding_box.x0 - radius,
        bounding_box.y0,
        bounding_box.x1 + radius,
        bounding_box.y1,
    )) || enters(Rect::new(
        bounding_box.x0,
        bounding_box.y0 - radius,
        bounding_box.x1,
        bounding_box.y1 + radius,
    )) || [
        Point::new(bounding_box.x0, bounding_box.y0),
        Point::new(bounding_box.x1, bounding_box.y0),
        Point::new(bounding_box.x0, bounding_box.y1),
        Point::new(bounding_box.x1, bounding_box.y1),
    ]
    .into_iter()
    .any(near_corner)
}

#[cfg(test)]
mod tests {
    use kurbo::PathEl;

    use super::*;
    use crate::axes::{Appearance, Axis};
    use crate::scene::{Color, Stroke};

    fn axes(x: (f64, f64), y: (f64, f64), width: f32, height: f32) -> Axes {
        let appearance = Appearance {
            stroke: Stroke::default(),
            color: Color {
                r: 255,
                g: 255,
                b: 255,
            },
            label_size: 0.3,
        };
        Axes::new(
            Axis::new(x.0, x.1, Some(1.0), width).unwrap(),
            Axis::new(y.0, y.1, Some(1.0), height).unwrap(),
            None,
            None,
            appearance,
        )
        .unwrap()
    }

    fn function(source: &str, axes: &Axes) -> Path {
        let range = axes.x.range();
        function_path(
            axes,
            &Expression::parse(source, crate::expr::FUNCTION_PLOT).unwrap(),
            range,
        )
        .unwrap()
    }

    fn lines(path: &Path) -> Vec<Vec<(f64, f64)>> {
        let mut parts: Vec<Vec<(f64, f64)>> = Vec::new();
        for element in path.elements() {
            match element {
                PathEl::MoveTo(p) => parts.push(vec![(p.x, p.y)]),
                PathEl::LineTo(p) => parts.last_mut().unwrap().push((p.x, p.y)),
                other => panic!("{other:?}"),
            }
        }
        parts
    }

    #[test]
    fn x_squared_passes_through_the_local_point_of_data_1_1() {
        let axes = axes((-2.0, 2.0), (0.0, 4.0), 4.0, 4.0);

        let parts = lines(&function("x^2", &axes));

        assert_eq!(parts.len(), 1);
        let near = |target: (f64, f64)| {
            parts[0]
                .iter()
                .any(|p| (p.0 - target.0).abs() < 1e-9 && (p.1 - target.1).abs() < 1e-9)
        };
        assert!(near((1.0, -1.0)), "data (1, 1) is local (1, -1)");
        assert!(
            near((-2.0, 2.0)) && near((2.0, 2.0)),
            "the ends are the corners where x^2 reaches 4"
        );
        assert!(near((0.0, -2.0)), "the vertex sits on the bottom edge");
    }

    #[test]
    fn a_plot_that_leaves_the_box_ends_at_its_side_and_returns_as_a_new_part() {
        let axes = axes((-3.0, 3.0), (-1.0, 1.0), 6.0, 2.0);

        let parts = lines(&function("2*sin(3*x)", &axes));

        assert!(parts.len() > 3, "{} parts", parts.len());
        for part in &parts {
            assert!(
                part.iter().all(|p| p.1.abs() <= 1.0 + 1e-9),
                "never above the box"
            );
        }
        assert!(
            parts
                .iter()
                .any(|part| (part.last().unwrap().1 - 1.0).abs() < 1e-9),
            "a part ends on the top edge"
        );
    }

    #[test]
    fn one_over_x_has_a_gap_at_the_pole_and_no_vertical_line_through_it() {
        let axes = axes((-2.0, 2.0), (-3.0, 3.0), 4.0, 6.0);

        let parts = lines(&function("1/x", &axes));

        assert_eq!(parts.len(), 2, "one part each side of x = 0");
        for part in &parts {
            let xs: Vec<f64> = part.iter().map(|p| p.0).collect();
            assert!(
                xs.iter().all(|x| (*x - 0.0).abs() > 1e-6),
                "no sample at the pole"
            );
        }
        assert!(parts[0].last().unwrap().0 < 0.0 && parts[1][0].0 > 0.0);
    }

    #[test]
    fn tan_has_a_gap_at_each_pole() {
        let axes = axes((-3.0, 3.0), (-4.0, 4.0), 6.0, 8.0);

        let parts = lines(&function("tan(x)", &axes));

        assert_eq!(
            parts.len(),
            3,
            "poles at -pi/2 and pi/2 make three branches"
        );
    }

    #[test]
    fn a_jump_inside_the_box_breaks_the_curve() {
        let axes = axes((-2.0, 2.0), (-2.0, 2.0), 4.0, 4.0);

        let step = lines(&function("step(x)", &axes));
        let saw = lines(&function("mod(x, 1)", &axes));

        assert_eq!(step.len(), 2, "no stroke up the jump at 0: {step:?}");
        assert!(
            step[0].iter().all(|p| p.1.abs() < 1e-9)
                && step[1].iter().all(|p| (p.1 - 1.0).abs() < 1e-9)
        );
        assert_eq!(saw.len(), 4, "a tooth for each whole x from -2 to 2");
        assert_eq!(
            lines(&function("x^3", &axes)).len(),
            1,
            "a steep curve is not a jump"
        );
    }

    #[test]
    fn sqrt_has_no_plot_where_it_has_no_value_and_starts_at_its_first_value() {
        let axes = axes((-4.0, 4.0), (0.0, 3.0), 8.0, 3.0);

        let parts = lines(&function("sqrt(x)", &axes));

        assert_eq!(parts.len(), 1);
        assert!(
            parts[0][0].0 > 0.0 - 1e-3 && parts[0][0].0 < 0.01,
            "starts at x = 0: {:?}",
            parts[0][0]
        );
    }

    #[test]
    fn a_steep_line_that_crosses_the_box_in_one_coarse_step_is_drawn_all_the_same() {
        let axes = axes((-3.0, 3.0), (-4.0, 4.0), 6.0, 8.0);

        let parts = lines(&function("1000*x", &axes));

        assert_eq!(parts.len(), 1, "a continuous line, not a jump");
    }

    #[test]
    fn a_function_with_no_value_in_the_box_has_nothing_to_draw() {
        let axes = axes((-3.0, -1.0), (0.0, 1.0), 2.0, 1.0);

        let drawn = function_path(
            &axes,
            &Expression::parse("sqrt(x)", crate::expr::FUNCTION_PLOT).unwrap(),
            (-3.0, -1.0),
        );

        assert_eq!(drawn.unwrap_err(), PlotError::NothingInside);
        assert_eq!(
            function_path(
                &axes,
                &Expression::parse("x", crate::expr::FUNCTION_PLOT).unwrap(),
                (5.0, 6.0)
            )
            .unwrap_err(),
            PlotError::OutsideAxes
        );
    }

    #[test]
    fn points_are_joined_in_order_and_cut_at_the_box() {
        let axes = axes((0.0, 4.0), (0.0, 4.0), 4.0, 4.0);

        let path = points_path(&axes, &[(0.0, 0.0), (2.0, 2.0), (3.0, 6.0), (4.0, 2.0)]).unwrap();

        let parts = lines(&path);
        assert_eq!(
            parts.len(),
            2,
            "the step from (3, 6) leaves the box and the next re-enters"
        );
        assert_eq!(
            parts[0].last().unwrap(),
            &(0.5, 2.0),
            "cut where the segment from data (2, 2) to (3, 6) meets y = 4"
        );
    }
}
