//! Plane maps and linear maps: the only module that does 2x2 math. A linear map carries objects
//! by one matrix about its map origin, and each object it names keeps a plane map, every linear
//! map so far composed into one matrix and shift. Pure, with no GPU and no timeline.

use glam::{Mat2, Vec2};
use schemars::JsonSchema;
use serde::Deserialize;

/// The largest entry or shift a composed plane map may reach. A matrix entry is at most 100, so
/// three linear maps in a row could reach 1e6; past this bound the linear map that went over is
/// a scene error. 1e4 keeps every mapped point far inside f32 and inside what lyon cuts well.
pub const PLANE_MAP_BOUND: f32 = 1e4;

/// Where every linear map so far has carried an object: a world point `p` of the xy plane lands
/// at `matrix * p + shift`. It acts after the object's transform and leaves z alone.
///
/// Every entry and the shift are finite: only `IDENTITY`, `LinearMap::at`, `LinearMap::whole`
/// and `then` make one, and the timeline refuses a composed end past `PLANE_MAP_BOUND`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneMap {
    matrix: Mat2,
    shift: Vec2,
}

impl PlaneMap {
    pub const IDENTITY: PlaneMap = PlaneMap {
        matrix: Mat2::IDENTITY,
        shift: Vec2::ZERO,
    };

    pub fn apply(self, point: Vec2) -> Vec2 {
        self.matrix * point + self.shift
    }

    /// `next` after `self`: a later linear map composes on the left, so B after A is B·A.
    pub fn then(self, next: PlaneMap) -> PlaneMap {
        PlaneMap {
            matrix: next.matrix * self.matrix,
            shift: next.matrix * self.shift + next.shift,
        }
    }

    pub fn matrix(self) -> Mat2 {
        self.matrix
    }

    pub fn shift(self) -> Vec2 {
        self.shift
    }

    /// The matrix row by row, as a scene file writes one: `[[a, b], [c, d]]`.
    pub fn rows(self) -> [[f32; 2]; 2] {
        let m = self.matrix;
        [[m.x_axis.x, m.y_axis.x], [m.x_axis.y, m.y_axis.y]]
    }

    pub fn determinant(self) -> f32 {
        self.matrix.determinant()
    }

    /// Every entry and the shift finite and at most `PLANE_MAP_BOUND` in size.
    pub fn within_bound(self) -> bool {
        self.matrix
            .to_cols_array()
            .into_iter()
            .chain(self.shift.to_array())
            .all(|v| v.abs() <= PLANE_MAP_BOUND)
    }
}

/// How a linear map's matrix goes from the identity to all of `matrix` as its progress runs:
/// the scene file's values of `route`. No variant doc comments, for the reason `FillRule` gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MatrixRoute {
    #[default]
    Entries,
    Turn,
}

/// One linear map, read once from the scene file: a matrix about a map origin, and the route
/// it takes there.
#[derive(Debug)]
pub struct LinearMap {
    origin: Vec2,
    matrix: Mat2,
    route: Route,
}

/// A matrix route with what it needs worked out once. Only `LinearMap::new` builds a `Turn`,
/// and only for a matrix with a determinant above 0, so a turning reflection cannot exist.
#[derive(Debug)]
enum Route {
    /// At progress p the matrix is (1 − p)·I + p·A.
    Entries,
    /// The polar decomposition A = R(angle)·stretch, with `stretch` symmetric and positive
    /// definite. At progress p the matrix is R(p·angle)·((1 − p)·I + p·stretch).
    Turn { angle: f32, stretch: Mat2 },
}

/// Why `route: turn` has no meaning for a matrix: its determinant is 0 or less, so it flips or
/// flattens the plane and has no rotation inside it to turn by.
#[derive(Debug, PartialEq)]
pub struct NoTurningRoute {
    pub determinant: f32,
}

impl LinearMap {
    /// `rows` is `[[a, b], [c, d]]` as the scene file writes it, sending `(x, y)` measured from
    /// `origin` to `(a·x + b·y, c·x + d·y)`.
    pub fn new(
        origin: Vec2,
        rows: [[f32; 2]; 2],
        route: MatrixRoute,
    ) -> Result<LinearMap, NoTurningRoute> {
        let [[a, b], [c, d]] = rows;
        let matrix = Mat2::from_cols(Vec2::new(a, c), Vec2::new(b, d));
        let route = match route {
            MatrixRoute::Entries => Route::Entries,
            MatrixRoute::Turn => {
                let determinant = matrix.determinant();
                if determinant <= 0.0 {
                    return Err(NoTurningRoute { determinant });
                }
                // The polar rotation of a 2x2 is along A + adj(A)ᵀ = [[a + d, b − c], [c − b, a + d]],
                // and (a + d)² + (c − b)² = |A|² + 2·det > 0, so the angle is defined. In (−π, π]:
                // the shorter way round, and counterclockwise at exactly half a turn. atan2 reads
                // the sign of a zero, so a −0 written in the scene file would turn clockwise.
                let across = if c == b { 0.0 } else { c - b };
                let angle = across.atan2(a + d);
                Route::Turn {
                    angle,
                    stretch: Mat2::from_angle(-angle) * matrix,
                }
            }
        };
        Ok(LinearMap {
            origin,
            matrix,
            route,
        })
    }

    /// The map this linear map has made by `progress`, about its origin: p ↦ G·(p − o) + o. A
    /// progress past 1, as under an overshoot, goes past the matrix and stays finite.
    pub fn at(&self, progress: f32) -> PlaneMap {
        let toward = |end: Mat2| Mat2::IDENTITY * (1.0 - progress) + end * progress;
        let matrix = match self.route {
            Route::Entries => toward(self.matrix),
            Route::Turn { angle, stretch } => Mat2::from_angle(progress * angle) * toward(stretch),
        };
        self.about_origin(matrix)
    }

    /// The whole map, with the matrix exactly as written rather than R·P multiplied back, so a
    /// point ends at its numerical matrix product whichever the route.
    pub fn whole(&self) -> PlaneMap {
        self.about_origin(self.matrix)
    }

    fn about_origin(&self, matrix: Mat2) -> PlaneMap {
        PlaneMap {
            matrix,
            shift: self.origin - matrix * self.origin,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Mat2, b: [[f32; 2]; 2], tolerance: f32) -> bool {
        let rows = PlaneMap {
            matrix: a,
            shift: Vec2::ZERO,
        }
        .rows();
        rows.iter()
            .flatten()
            .zip(b.iter().flatten())
            .all(|(x, y)| (x - y).abs() <= tolerance)
    }

    fn turn(rows: [[f32; 2]; 2]) -> (f32, Mat2) {
        match LinearMap::new(Vec2::ZERO, rows, MatrixRoute::Turn)
            .unwrap()
            .route
        {
            Route::Turn { angle, stretch } => (angle.to_degrees(), stretch),
            Route::Entries => panic!("a turning route"),
        }
    }

    #[test]
    fn a_shear_splits_into_a_turn_and_a_symmetric_stretch() {
        let (angle, stretch) = turn([[1.0, 1.0], [0.0, 1.0]]);

        assert!((angle - -26.565052).abs() < 1e-4, "{angle}");
        assert!(
            close(stretch, [[0.894427, 0.447214], [0.447214, 1.341641]], 1e-5),
            "{stretch}"
        );
    }

    #[test]
    fn a_rotation_splits_into_itself_and_no_stretch() {
        let (angle, stretch) = turn([[0.8660254, -0.5], [0.5, 0.8660254]]);

        assert!((angle - 30.0).abs() < 1e-4, "{angle}");
        assert!(close(stretch, [[1.0, 0.0], [0.0, 1.0]], 1e-6), "{stretch}");
    }

    #[test]
    fn a_matrix_that_flips_or_flattens_has_no_turning_route() {
        let refused = |rows| LinearMap::new(Vec2::ZERO, rows, MatrixRoute::Turn).unwrap_err();

        assert_eq!(
            refused([[1.0, 0.0], [0.0, -1.0]]),
            NoTurningRoute { determinant: -1.0 }
        );
        assert_eq!(
            refused([[2.0, 0.0], [0.0, 0.0]]),
            NoTurningRoute { determinant: 0.0 }
        );
        assert_eq!(
            refused([[1.0, 1.0], [1.0, 1.0]]),
            NoTurningRoute { determinant: 0.0 }
        );
        assert!(
            LinearMap::new(Vec2::ZERO, [[1.0, 0.0], [0.0, -1.0]], MatrixRoute::Entries).is_ok()
        );
    }

    #[test]
    fn halfway_through_a_quarter_turn_the_entries_route_shrinks_and_the_turn_route_does_not() {
        let quarter = [[0.0, -1.0], [1.0, 0.0]];
        let entries = LinearMap::new(Vec2::ZERO, quarter, MatrixRoute::Entries).unwrap();
        let turning = LinearMap::new(Vec2::ZERO, quarter, MatrixRoute::Turn).unwrap();

        assert_eq!(entries.at(0.5).rows(), [[0.5, -0.5], [0.5, 0.5]]);
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!(close(turning.at(0.5).matrix(), [[h, -h], [h, h]], 1e-6));
        assert!(close(
            turning.at(0.25).matrix(),
            [[0.923880, -0.382683], [0.382683, 0.923880]],
            1e-6
        ));
        assert_eq!(turning.at(0.0), PlaneMap::IDENTITY);
    }

    #[test]
    fn a_half_turn_passes_through_zero_by_entries_and_turns_counterclockwise_by_turn() {
        let half = [[-1.0, 0.0], [0.0, -1.0]];

        assert_eq!(
            LinearMap::new(Vec2::ZERO, half, MatrixRoute::Entries)
                .unwrap()
                .at(0.5)
                .rows(),
            [[0.0, 0.0], [0.0, 0.0]]
        );
        let turning = LinearMap::new(Vec2::ZERO, half, MatrixRoute::Turn).unwrap();
        assert!(
            close(turning.at(0.5).matrix(), [[0.0, -1.0], [1.0, 0.0]], 1e-6),
            "{}",
            turning.at(0.5).matrix()
        );
    }

    #[test]
    fn a_turn_with_a_stretch_turns_while_it_grows() {
        let turning =
            LinearMap::new(Vec2::ZERO, [[0.0, -2.0], [2.0, 0.0]], MatrixRoute::Turn).unwrap();

        let h = 1.5 * std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            close(turning.at(0.5).matrix(), [[h, -h], [h, h]], 1e-5),
            "turned 45° at 1.5 times"
        );
    }

    #[test]
    fn a_linear_map_acts_about_its_origin_and_ends_at_the_matrix_as_written() {
        let origin = Vec2::new(-1.4, -0.2);
        let stretch =
            LinearMap::new(origin, [[2.0, 0.0], [0.0, 0.5]], MatrixRoute::Entries).unwrap();

        assert_eq!(stretch.whole().apply(origin), origin);
        assert!(
            stretch
                .whole()
                .apply(Vec2::new(-0.4, 0.8))
                .distance(Vec2::new(0.6, 0.3))
                < 1e-6
        );
        assert_eq!(stretch.whole().rows(), [[2.0, 0.0], [0.0, 0.5]]);
        assert_eq!(stretch.at(0.5).rows(), [[1.5, 0.0], [0.0, 0.75]]);
        let rotation = [[0.8660254, -0.5], [0.5, 0.8660254]];
        assert_eq!(
            LinearMap::new(origin, rotation, MatrixRoute::Turn)
                .unwrap()
                .whole()
                .rows(),
            rotation,
            "not R·P multiplied back"
        );
    }

    #[test]
    fn a_later_map_composes_on_the_left_with_its_own_origin() {
        let shear = LinearMap::new(
            Vec2::new(1.0, 0.0),
            [[1.0, 1.0], [0.0, 1.0]],
            MatrixRoute::Entries,
        )
        .unwrap();
        let double = LinearMap::new(
            Vec2::new(0.0, 2.0),
            [[2.0, 0.0], [0.0, 2.0]],
            MatrixRoute::Entries,
        )
        .unwrap();

        let both = shear.whole().then(double.whole());

        assert_eq!(both.rows(), [[2.0, 2.0], [0.0, 2.0]]);
        let point = Vec2::new(3.0, 1.0);
        assert_eq!(shear.whole().apply(point), Vec2::new(4.0, 1.0));
        assert_eq!(
            both.apply(point),
            Vec2::new(8.0, 0.0),
            "(4, 1) doubled about (0, 2)"
        );
        assert_eq!(both.shift(), Vec2::new(0.0, -2.0));
    }

    #[test]
    fn an_overshoot_past_the_zero_matrix_stays_finite() {
        let zero = LinearMap::new(
            Vec2::new(0.2, 0.1),
            [[0.0, 0.0], [0.0, 0.0]],
            MatrixRoute::Entries,
        )
        .unwrap();

        let past = zero.at(1.064137);

        assert!((past.matrix().x_axis.x - -0.064137).abs() < 1e-6);
        assert_eq!(
            zero.whole().apply(Vec2::new(5.0, -3.0)),
            Vec2::new(0.2, 0.1)
        );
        assert!(past.within_bound());
    }

    #[test]
    fn a_plane_map_past_the_bound_or_not_finite_is_out_of_bound() {
        let big = LinearMap::new(Vec2::ZERO, [[100.0, 0.0], [0.0, 1.0]], MatrixRoute::Entries)
            .unwrap()
            .whole();

        assert!(big.then(big).within_bound(), "1e4 is at the bound");
        assert!(!big.then(big).then(big).within_bound());
        assert!(
            !PlaneMap {
                matrix: Mat2::IDENTITY,
                shift: Vec2::new(f32::NAN, 0.0)
            }
            .within_bound()
        );
        assert_eq!(big.determinant(), 100.0);
    }
}
