//! Ends on objects: a line, an arrow or a spring whose `from` or `to` sits on an object, at the
//! object's world position plus an offset in world units, on a tube's or a plot's curve, or on
//! an atom of a molecule, so it goes with that object. Such a shape is placed by its ends: at
//! every scene time the stage reads where both ends are, after shape laws, carries and data
//! points, and lays the shape between them.
//!
//! The shape is drawn again between its ends at every scene time rather than scaled, so an
//! arrow's tip keeps its length and a spring keeps its turns as its ends move.

use std::f32::consts::TAU;

use glam::{EulerRot, Mat3, Quat, Vec2, Vec3};
use serde::Serialize;

use crate::molecule::AtomNumber;
use crate::path::Path;
use crate::scene::{Camera, Object, ObjectId, Shape, Solid, Transform, own_to_world};
use crate::space_curve::SpaceCurve;

/// What places a line, an arrow or a spring: its two ends, and how it is drawn between them.
#[derive(Clone, Debug, PartialEq)]
pub struct Ends {
    pub from: End,
    pub to: End,
    pub drawn: Drawn,
}

/// One end, which `End::world` finds in the world at a scene time.
#[derive(Clone, Debug, PartialEq)]
pub enum End {
    /// A point in the world.
    Point(Vec3),
    /// An object's world position plus an offset in world units.
    On { object: ObjectId, offset: Vec3 },
    /// The point at `u` of a tube's space curve, within the curve's u range.
    OnCurve { object: ObjectId, u: f64 },
    /// The first point of a plot's curve at `x` in its axes' own plane, which is the plot's own.
    OnPlot { object: ObjectId, x: f32 },
    /// The center of an atom of a molecule object.
    OnAtom { object: ObjectId, atom: AtomNumber },
}

/// How a shape placed by its ends is drawn between them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Drawn {
    Line,
    /// An arrow's tip is `tip_length` long, or a quarter of the arrow when that is shorter.
    Arrow {
        tip_length: f32,
    },
    /// A coil of `turns` turns of `radius` about the line between the ends.
    Spring {
        turns: f32,
        radius: f32,
    },
}

/// How many points a spring's coil has per turn.
const POINTS_PER_TURN: f32 = 48.0;

/// The most turns a spring's coil makes, which keeps the coil it draws again at every scene
/// time to at most a few thousand points.
pub const MOST_TURNS: f32 = 100.0;

/// Ends closer than this, in world units, meet, and the shape between them draws nothing.
const LEAST_LENGTH: f32 = 1e-5;

impl End {
    /// Where the end is in the world, with `objects` as they stand at a scene time.
    /// A curve is read as the object's shape stands then, so an end on a curve follows its shape
    /// law, and an end on a curve or an atom goes with the object as it moves, turns and scales.
    pub fn world(&self, objects: &std::collections::BTreeMap<ObjectId, Object>) -> Vec3 {
        let in_own_space = |object: &ObjectId, own: Vec3| {
            own_to_world(objects, object)
                .to_world()
                .transform_point3(own)
        };
        match self {
            End::Point(point) => *point,
            End::On { object, offset } => objects[object].flat_to_world().position() + *offset,
            End::OnCurve { object, u } => match &objects[object].shape {
                Shape::Solid {
                    solid: Solid::Tube { curve, .. },
                    ..
                } => in_own_space(object, curve.point_at_u(*u)),
                _ => unreachable!("`scene_file` sits an end on a curve only on a tube"),
            },
            End::OnPlot { object, x } => match &objects[object].shape {
                Shape::Flat { path, .. } => in_own_space(object, path.point_at_x(*x).extend(0.0)),
                _ => unreachable!("`scene_file` refuses a morph of a plot an end sits on"),
            },
            End::OnAtom { object, atom } => match &objects[object].shape {
                Shape::Molecule { molecule, .. } => {
                    in_own_space(object, molecule.atom(*atom).position)
                }
                _ => unreachable!("`scene_file` sits an end on an atom only of a molecule"),
            },
        }
    }

    pub fn object(&self) -> Option<&ObjectId> {
        match self {
            End::Point(_) => None,
            End::On { object, .. }
            | End::OnCurve { object, .. }
            | End::OnPlot { object, .. }
            | End::OnAtom { object, .. } => Some(object),
        }
    }
}

impl Ends {
    /// The objects the ends sit on.
    pub fn objects(&self) -> impl Iterator<Item = &ObjectId> {
        self.from.object().into_iter().chain(self.to.object())
    }

    /// The transform that lays the shape from `from` to `to`, its own x axis along them and its
    /// own z axis as near the camera's direction as it can be, and `shape` drawn again that long.
    /// `None` while the ends meet.
    pub fn place(
        &self,
        from: Vec3,
        to: Vec3,
        camera: &Camera,
        shape: &Shape,
    ) -> Option<(Transform, Shape)> {
        let length = from.distance(to);
        if length < LEAST_LENGTH {
            return None;
        }
        let along = (to - from) / length;
        let facing = [camera.direction(), Vec3::Z, Vec3::Y]
            .into_iter()
            .map(|toward| toward.reject_from_normalized(along))
            .find(|square| square.length() > 1e-3)
            .expect("one of z and y is not along a line");
        let facing = facing.normalize();
        let (x, y, z) = Quat::from_mat3(&Mat3::from_cols(along, facing.cross(along), facing))
            .to_euler(EulerRot::XYZ);
        let transform = Transform {
            position: from,
            rotation: Vec3::new(x, y, z) * (180.0 / std::f32::consts::PI),
            scale: Vec3::ONE,
        };
        let mut shape = shape.clone();
        match (&mut shape, self.drawn) {
            (Shape::Flat { path, .. }, Drawn::Line) => {
                *path = Path::line(Vec2::ZERO, Vec2::new(length, 0.0)).ok()?
            }
            (Shape::Flat { path, .. }, Drawn::Arrow { tip_length }) => {
                *path = Path::arrow(Vec2::ZERO, Vec2::new(length, 0.0), tip_length).ok()?
            }
            (
                Shape::Solid {
                    solid: Solid::Tube { curve, .. },
                    ..
                },
                Drawn::Spring { turns, radius },
            ) => *curve = std::sync::Arc::new(helix(length, turns, radius)),
            _ => unreachable!(
                "`scene_file` gives a line or an arrow a flat shape and a spring a tube"
            ),
        }
        Some((transform, shape))
    }
}

/// A coil `length` long along x from the origin, of `turns` turns of `radius` about the x axis.
/// Its radius rises from 0 over the first quarter turn and falls to 0 over the last, so both ends
/// lie on the axis, where the spring's ends are.
pub fn helix(length: f32, turns: f32, radius: f32) -> SpaceCurve {
    let count = (turns * POINTS_PER_TURN).ceil().max(8.0) as usize;
    let points = (0..=count)
        .map(|k| {
            let share = k as f32 / count as f32;
            let rise = (4.0 * turns * share)
                .min(4.0 * turns * (1.0 - share))
                .min(1.0);
            let angle = TAU * turns * share;
            Vec3::new(
                length * share,
                radius * rise * angle.sin(),
                radius * rise * angle.cos(),
            )
        })
        .collect();
    SpaceCurve::through(points).expect("a coil of some length has distinct points")
}

/// Where each end of a shape placed by its ends is in the world, and the pixels they land on.
#[derive(Debug, Serialize)]
pub struct EndsReport {
    pub from: Vec3,
    pub to: Vec3,
    pub from_pixels: Option<Vec2>,
    pub to_pixels: Option<Vec2>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_spring_s_helix_keeps_its_turns_and_ends_on_its_axis() {
        let short = helix(1.0, 6.0, 0.2);
        let long = helix(3.0, 6.0, 0.2);

        assert_eq!(short.start(), Vec3::ZERO);
        assert!(
            long.end().distance(Vec3::new(3.0, 0.0, 0.0)) < 1e-5,
            "{}",
            long.end()
        );
        assert!(long.length() > short.length(), "the same turns stretched");
        assert!(
            (short.reach() - (1.0f32 + 0.04).sqrt()).abs() < 0.05,
            "the coil keeps its radius: {}",
            short.reach()
        );
    }

    #[test]
    fn a_shape_in_the_plane_of_the_frame_turns_only_about_z() {
        let ends = Ends {
            from: End::Point(Vec3::ZERO),
            to: End::Point(Vec3::new(1.0, 1.0, 0.0)),
            drawn: Drawn::Arrow { tip_length: 0.35 },
        };
        let arrow = Shape::Flat {
            path: Path::line(Vec2::ZERO, Vec2::X).unwrap(),
            stroke: None,
            fill: None,
        };

        let (transform, _) = ends
            .place(
                Vec3::ZERO,
                Vec3::new(1.0, 1.0, 0.0),
                &Camera::default(),
                &arrow,
            )
            .unwrap();

        assert!(
            transform.rotation.distance(Vec3::new(0.0, 0.0, 45.0)) < 1e-3,
            "{}",
            transform.rotation
        );
        assert!(
            ends.place(Vec3::ONE, Vec3::ONE, &Camera::default(), &arrow)
                .is_none(),
            "ends that meet draw nothing"
        );
    }
}
