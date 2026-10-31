//! A follow: an animation that sets a value at every scene time of its span from a law, three
//! expressions in law time, or carries an object along another object's curve, instead of
//! easing it to a `to`.
//!
//! Everything a follow computes lives here: a law's value at a scene time, where a law has no
//! value or reaches too far at a frame time, and where a carry puts its object and how it turns
//! it to face the tangent. A simulation's set gives its value outright as a law does, so
//! `Outright` holds both and every rule about a value given outright is written once.
//! `Timeline` puts a follow on the track of each value it sets, as a segment set by its law or
//! its carry, so every rule that reads tracks covers it.

use std::sync::Arc;

use glam::{EulerRot, Mat3, Quat, Vec2, Vec3};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::expr::Expression;
use crate::path::Path;
use crate::scene::{FlatToWorld, ObjectId, Shape, Solid};
use crate::scene_file::{JsonPath, SceneError, SceneErrorCode};
use crate::simulate::Simulation;
use crate::space_curve::{CurveFrame, SpaceCurve};
use crate::timeline::ChapterId;

/// How far from the origin a position a law sets may reach, in world units.
pub const FARTHEST: f64 = 1e4;

/// How far a follow's first value may lie from the value its track holds when it begins, so a
/// value never pops: farther is `law_jump`.
pub const LEAST_JUMP: f64 = 0.001;

/// The three expressions in law time that a follow sets its value from, read and checked for
/// form when the scene file is read.
#[derive(Debug)]
pub struct Law {
    /// x, y and z, in that order.
    coordinates: [Expression; 3],
    kind: LawKind,
}

const COORDINATES: [&str; 3] = ["x", "y", "z"];

/// What a law's numbers are, which decides how far they may reach: a position, the camera's
/// position or target, is bounded by `FARTHEST`, a rotation's angles may wind on, a scale may
/// grow, shrink or turn over as a scale animation's `to` may, a readout's value is its one
/// number, `value`, which may reach anywhere, and an object opacity is one number, `value`,
/// from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LawKind {
    Position,
    Rotation,
    Scale,
    Value,
    Opacity,
}

/// How far past 0 or 1 an object opacity law may reach at a frame time before it is
/// `out_of_range`: only rounding, so `1 - t/2` at the end of a 2 second span is 0.
const OPACITY_ROUNDING: f64 = 1e-9;

impl LawKind {
    /// How many numbers the law gives: three, or one, `value`.
    fn numbers(self) -> usize {
        match self {
            LawKind::Value | LawKind::Opacity => 1,
            LawKind::Position | LawKind::Rotation | LawKind::Scale => 3,
        }
    }

    /// Whether `value` is a number this kind may take at a frame time.
    fn takes(self, value: f64) -> bool {
        value.is_finite()
            && match self {
                LawKind::Position => value.abs() <= FARTHEST,
                LawKind::Opacity => (-OPACITY_ROUNDING..=1.0 + OPACITY_ROUNDING).contains(&value),
                LawKind::Rotation | LawKind::Scale | LawKind::Value => true,
            }
    }

    /// `value` brought within what this kind may take, once it is finite.
    fn bounded(self, value: f64) -> f64 {
        match self {
            LawKind::Position => value.clamp(-FARTHEST, FARTHEST),
            LawKind::Opacity => value.clamp(0.0, 1.0),
            LawKind::Rotation | LawKind::Scale | LawKind::Value => value,
        }
    }
}

/// One number of a law or a set that has no finite value, a position's that reaches past
/// `FARTHEST`, or an object opacity's outside 0 to 1, at a frame time of its span.
#[derive(Debug, PartialEq)]
pub struct OutrightFault {
    /// 0 for x, 1 for y and 2 for z, or 0 for a law's one number, `value`.
    pub coordinate: usize,
    pub scene_time: f64,
    pub law_time: f64,
    /// NaN or infinite for a number with no value.
    pub value: f64,
}

impl Law {
    pub fn new(coordinates: [Expression; 3], kind: LawKind) -> Law {
        Law { coordinates, kind }
    }

    /// The law as it is written, at a law time: NaN or infinite where a coordinate has no value.
    fn written(&self, law_time: f64) -> [f64; 3] {
        self.coordinates
            .each_ref()
            .map(|expression| expression.eval(&[law_time]))
    }
}

/// What sets a value outright at every scene time of a span, rather than easing it to a `to`: a
/// follow's law, or one set of a simulation. The value is what it gives, so it must start where
/// the value is, and the value holds what it gave at the span's end.
#[derive(Debug)]
pub enum Outright {
    Law {
        law: Arc<Law>,
        law_start: f64,
    },
    Set {
        simulation: Arc<Simulation>,
        set: usize,
    },
}

impl Outright {
    /// The scene time law time counts from.
    pub fn law_start(&self) -> f64 {
        match self {
            Outright::Law { law_start, .. } => *law_start,
            Outright::Set { simulation, .. } => simulation.law_start(),
        }
    }

    fn kind(&self) -> LawKind {
        match self {
            Outright::Law { law, .. } => law.kind,
            Outright::Set { simulation, set } => simulation.set_kind(*set),
        }
    }

    /// The numbers as written at a scene time: NaN or infinite where one has no value.
    fn written(&self, scene_time: f64) -> [f64; 3] {
        match self {
            Outright::Law { law, law_start } => law.written(scene_time - law_start),
            Outright::Set { simulation, set } => simulation.set_at(*set, scene_time),
        }
    }

    /// The value a track holds at a scene time. `validate` refuses a number with no value at a
    /// frame time, but `frame` and `inspect` may ask any scene time, so a number with no finite
    /// value there keeps `held`'s, the value when the span began, a position's stays within
    /// `FARTHEST`, and an object opacity's within 0 to 1. Every scene time is then drawable, and
    /// still a pure function of the scene time.
    pub fn at(&self, scene_time: f64, held: [f64; 3]) -> [f64; 3] {
        let kind = self.kind();
        let mut value = self.written(scene_time);
        for (number, held) in value.iter_mut().zip(held) {
            *number = if number.is_finite() {
                kind.bounded(*number)
            } else {
                held
            };
        }
        value
    }

    /// The first scene time of `scene_times` at which each number has no finite value, a
    /// position's lies past `FARTHEST`, or an object opacity's outside 0 to 1, so there are at
    /// most three faults, each reported at once.
    pub fn faults(&self, scene_times: impl Iterator<Item = f64>) -> Vec<OutrightFault> {
        let kind = self.kind();
        let numbers = kind.numbers();
        let mut faults: Vec<OutrightFault> = Vec::new();
        for scene_time in scene_times {
            for (coordinate, value) in self
                .written(scene_time)
                .into_iter()
                .enumerate()
                .take(numbers)
            {
                if !kind.takes(value) && !faults.iter().any(|fault| fault.coordinate == coordinate)
                {
                    faults.push(OutrightFault {
                        coordinate,
                        scene_time,
                        law_time: scene_time - self.law_start(),
                        value,
                    });
                }
            }
            if faults.len() == numbers {
                break;
            }
        }
        faults.sort_by_key(|fault| fault.coordinate);
        faults
    }

    /// The `out_of_range` error of `fault` at `expression`, the path of its expression.
    pub fn fault_error(&self, fault: &OutrightFault, expression: JsonPath) -> SceneError {
        let kind = self.kind();
        let name = if kind.numbers() == 1 {
            "value"
        } else {
            COORDINATES[fault.coordinate]
        };
        let (needs, change) = match self {
            Outright::Law { .. } => (
                "a law needs one at every frame time of its follow's span",
                "the follow's span",
            ),
            Outright::Set { .. } => (
                "a simulation sets its values at every frame time of its span",
                "the simulation",
            ),
        };
        let when = format!(
            "scene time {} (law time {})",
            short(fault.scene_time),
            short(fault.law_time)
        );
        let message = if kind == LawKind::Opacity && fault.value.is_finite() {
            format!(
                "`{name}` reaches {} at {when}, outside the 0 to 1 an object opacity takes; change the expression or {change}",
                short(fault.value)
            )
        } else if fault.value.is_finite() {
            format!(
                "`{name}` reaches {} at {when}, past the {FARTHEST} a position may reach; change the expression or {change}",
                short(fault.value)
            )
        } else {
            format!(
                "`{name}` has no finite value at {when}, and {needs}; change the expression or {change}"
            )
        };
        SceneError::new(expression, SceneErrorCode::OutOfRange, message)
    }
}

/// Where a follow's law time counts from.
#[derive(Clone, Debug, PartialEq)]
pub enum LawTimeStart {
    /// The start of the follow's own span: the default.
    Span,
    /// The start of the chapter `from` names: the follow's own chapter or one before it.
    /// `Timeline` resolves it, because only the timeline knows when chapters begin.
    Chapter(ChapterId),
}

/// The value a follow with a law sets, which picks the one track it holds for its span.
#[derive(Clone, Debug, PartialEq)]
pub enum Followed {
    Position(ObjectId),
    Rotation(ObjectId),
    CameraPosition,
    CameraTarget,
    Scale(ObjectId),
    /// A readout's value.
    Value(ObjectId),
    /// The object opacity.
    Opacity(ObjectId),
}

/// A follow past the scene file: a law or a carry, never both and never neither, which
/// `scene_file` reports at the field at fault. A follow of the camera names no object, and one of
/// an object names it in what it sets.
#[derive(Debug)]
pub enum Follow {
    Law {
        followed: Followed,
        law: Arc<Law>,
        law_time_start: LawTimeStart,
    },
    /// Sets the object's position, and its rotation too when it faces the tangent, shaped by the
    /// animation's easing curve.
    Carry {
        object_id: ObjectId,
        carry: Arc<Carry>,
    },
    /// Runs the shape law of the object: its shape at every scene time of the span is its shape
    /// at that law time. `scene_file` has checked that its shape reads `t`.
    Shape {
        object_id: ObjectId,
        law_time_start: LawTimeStart,
    },
}

impl Follow {
    /// The object whose transform this follow sets, if it sets an object's. A follow of a shape,
    /// a readout's value or an object opacity sets none.
    pub fn object_id(&self) -> Option<&ObjectId> {
        match self {
            Follow::Law {
                followed:
                    Followed::Position(object_id)
                    | Followed::Rotation(object_id)
                    | Followed::Scale(object_id),
                ..
            }
            | Follow::Carry { object_id, .. } => Some(object_id),
            Follow::Law {
                followed:
                    Followed::CameraPosition
                    | Followed::CameraTarget
                    | Followed::Value(_)
                    | Followed::Opacity(_),
                ..
            }
            | Follow::Shape { .. } => None,
        }
    }

    /// Whether this follow moves the camera's position or target.
    pub fn turns_the_camera(&self) -> bool {
        matches!(
            self,
            Follow::Law {
                followed: Followed::CameraPosition | Followed::CameraTarget,
                ..
            }
        )
    }
}

/// How a carried object turns as it goes, or each copy of an `along` layout turns at its place.
/// Leaving `faces` out keeps the rotation it has. `tangent` turns the object so its own x axis
/// points along the curve's tangent, toward the curve's end, and its own z axis stays square to
/// the plane of a flat curve, or along the third direction of a space curve's frame, which starts
/// as near the curve's own +z as it can and never twists about the curve. A copy's own rotation
/// then turns it further, about its turned axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Faces {
    Tangent,
}

/// What a follow with `along` carries its object along: the curve of a flat shape, a plot or a
/// tube, as that object declares it. Where that object is at a scene time comes from the stage,
/// so an `along` object that moves, a plot placed by its axes and a flat shape under a plane map
/// all carry correctly with nothing more here.
#[derive(Debug)]
pub struct Carry {
    /// The object whose curve this is. Never the carried object, never attached, never itself
    /// carried, and never morphed: `Timeline` and `scene_file` refuse each.
    pub along: ObjectId,
    curve: CarryCurve,
    pub faces: Option<Faces>,
}

#[derive(Debug)]
enum CarryCurve {
    /// A flat shape's path in its own plane; a plot's lies in its axes' own space.
    Flat(Path),
    /// A tube's space curve in its object space.
    Space(Arc<SpaceCurve>),
}

/// Where a carry puts its object at one share of the curve, in the world.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CarryPoint {
    pub position: Vec3,
    /// Only when the carry faces the tangent.
    pub rotation: Option<Vec3>,
}

impl Carry {
    /// `None` for a shape with no one curve to carry along: text, math, axes, a number line, a
    /// sphere, a prism, a revolution or a molecule.
    pub fn of(along: ObjectId, shape: &Shape, faces: Option<Faces>) -> Option<Carry> {
        let curve = match shape {
            Shape::Flat { path, .. } => CarryCurve::Flat(path.clone()),
            Shape::Solid {
                solid: Solid::Tube { curve, .. },
                ..
            } => CarryCurve::Space(Arc::clone(curve)),
            _ => return None,
        };
        Some(Carry {
            along,
            curve,
            faces,
        })
    }

    /// The world point at `share` of the curve by length in the `along` object's own space, and,
    /// facing the tangent, the rotation, with that object where `flat_to_world` puts it at this scene
    /// time. A share outside 0 to 1, as an overshoot gives, waits at the curve's end.
    ///
    /// Own-space length, not world length, because a create cuts by own-space length too: a
    /// carry and a create of a plot with one span and easing curve keep the carried object on the
    /// pen's end at every frame, and the carry's speed does not change while the curve moves or
    /// is mapped.
    ///
    /// `reference` is the rotation the carried object held when the carry began. Of the Euler
    /// angles that give one orientation, this gives those that turn on from it with the curve, so
    /// they never jump by a turn from one frame to the next, and they equal the held rotation at
    /// the start whenever the orientation does.
    pub fn at(&self, share: f32, flat_to_world: FlatToWorld, reference: Vec3) -> CarryPoint {
        let share = share.clamp(0.0, 1.0);
        match &self.curve {
            CarryCurve::Flat(path) => {
                let at = path.point_at(share);
                let position = flat_to_world
                    .to_world()
                    .transform_point3(at.point.extend(0.0));
                let rotation = self.faces.map(|Faces::Tangent| {
                    let start = flat_rotation(
                        path.point_at(0.0).tangent,
                        path.point_at(0.0).angle,
                        flat_to_world,
                    );
                    let turns = ((reference.z - start.z) / 360.0).round() * 360.0;
                    flat_rotation(at.tangent, at.angle, flat_to_world) + Vec3::new(0.0, 0.0, turns)
                });
                CarryPoint { position, rotation }
            }
            CarryCurve::Space(curve) => {
                let to_world = flat_to_world.to_world();
                let length = share * curve.length();
                let position = to_world.transform_point3(curve.frame_at(length).point);
                let rotation = self.faces.map(|Faces::Tangent| {
                    // Counted on sample by sample from the start, as a flat path's angle is.
                    let frames = curve.frames_up_to(length);
                    frames
                        .iter()
                        .fold(None, |before: Option<Vec3>, frame| {
                            Some(nearest_equivalent(
                                space_rotation(frame, &to_world),
                                before.unwrap_or(reference),
                            ))
                        })
                        .expect("a curve has frames")
                });
                CarryPoint { position, rotation }
            }
        }
    }
}

/// The rotation that turns a flat object's own x axis along `tangent`, at the lifted `angle` in
/// radians, with its own z axis square to the curve's plane, in degrees. The `along` object's own
/// rotation comes first: its turn about z then the tangent's are one turn about the same axis, so
/// the Euler angles are exact and need no conversion. Its scale or plane map turns the tangent by
/// less than a quarter turn while it has no reflection, so the angle stays continuous.
fn flat_rotation(tangent: Vec2, angle: f64, flat_to_world: FlatToWorld) -> Vec3 {
    let principal = |angle: f64| {
        (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
    };
    match flat_to_world {
        FlatToWorld::Declared(transform) => {
            let turned = transform.scale.truncate() * tangent;
            let world = angle + principal(f64::from(turned.y).atan2(f64::from(turned.x)) - angle);
            transform.rotation + Vec3::new(0.0, 0.0, world.to_degrees() as f32)
        }
        FlatToWorld::Mapped { matrix, .. } => {
            // The turn of the plane map's matrix, as in its polar form.
            let turn = f64::from(matrix.x_axis.y - matrix.y_axis.x)
                .atan2(f64::from(matrix.x_axis.x + matrix.y_axis.y));
            let turned = matrix * tangent;
            let world = angle
                + turn
                + principal(f64::from(turned.y).atan2(f64::from(turned.x)) - angle - turn);
            Vec3::new(0.0, 0.0, world.to_degrees() as f32)
        }
    }
}

/// The principal Euler angles, in degrees, of the rotation that turns an object's own x axis along
/// a frame's tangent in the world and its own y axis along the frame's normal.
fn space_rotation(frame: &CurveFrame, to_world: &glam::Mat4) -> Vec3 {
    let tangent = to_world.transform_vector3(frame.tangent).normalize();
    let normal = to_world
        .transform_vector3(frame.normal)
        .reject_from_normalized(tangent)
        .normalize();
    degrees(facing(tangent, normal))
}

/// The principal Euler angles, in degrees, of a copy that faces the tangent of `frame`, in the
/// pattern's own space, then turns by `own`, its own rotation in degrees, about its turned axes.
pub fn faced_copy(frame: &CurveFrame, own: Vec3) -> Vec3 {
    let [x, y, z] = own.to_array().map(f32::to_radians);
    degrees(facing(frame.tangent, frame.normal) * Quat::from_euler(EulerRot::XYZ, x, y, z))
}

/// The turn of an object's own x axis onto `tangent` and its own y axis onto `normal`, each a
/// unit vector, square to the other.
fn facing(tangent: Vec3, normal: Vec3) -> Quat {
    Quat::from_mat3(&Mat3::from_cols(tangent, normal, tangent.cross(normal)))
}

fn degrees(rotation: Quat) -> Vec3 {
    let (x, y, z) = rotation.to_euler(EulerRot::XYZ);
    Vec3::new(x, y, z) * (180.0 / std::f32::consts::PI)
}

/// Of the Euler angles that give the orientation of `principal`, those nearest `reference` angle
/// by angle: `(a, b, c)` and `(a + 180, 180 - b, c + 180)`, each turned by whole turns. Only a
/// quarter turn about the turned y axis, where the first and third angles turn about one axis,
/// can still jump.
fn nearest_equivalent(principal: Vec3, reference: Vec3) -> Vec3 {
    let near = |angle: f32, reference: f32| angle + ((reference - angle) / 360.0).round() * 360.0;
    let nearest = |angles: Vec3| {
        Vec3::new(
            near(angles.x, reference.x),
            near(angles.y, reference.y),
            near(angles.z, reference.z),
        )
    };
    let (first, second) = (
        nearest(principal),
        nearest(Vec3::new(
            principal.x + 180.0,
            180.0 - principal.y,
            principal.z + 180.0,
        )),
    );
    if first.distance_squared(reference) <= second.distance_squared(reference) {
        first
    } else {
        second
    }
}

/// A value of three coordinates as a scene file writes one, such as `[1, 0.268, 0]`.
pub fn triple(value: [f64; 3]) -> String {
    format!("[{}]", value.map(short).join(", "))
}

/// A number to four decimals, as an agent reads and writes one, such as a scene time.
pub fn short(value: f64) -> String {
    let rounded = (value * 1e4).round() / 1e4;
    format!("{}", if rounded == 0.0 { 0.0 } else { rounded })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expr::LAW;

    fn law(x: &str, y: &str, z: &str, kind: LawKind, law_start: f64) -> Outright {
        let parse = |source| Expression::parse(source, LAW).unwrap();
        Outright::Law {
            law: Arc::new(Law::new([parse(x), parse(y), parse(z)], kind)),
            law_start,
        }
    }

    #[test]
    fn each_coordinate_faults_once_at_its_first_bad_scene_time() {
        let faults = law("1/(t - 2)", "exp(t)", "0", LawKind::Position, 0.0)
            .faults((0..=40).map(|i| f64::from(i) / 4.0));

        assert_eq!(faults.len(), 2);
        assert_eq!(
            (
                faults[0].coordinate,
                faults[0].scene_time,
                faults[0].law_time
            ),
            (0, 2.0, 2.0)
        );
        assert_eq!(
            (faults[1].coordinate, faults[1].scene_time),
            (1, 9.25),
            "e^9.25 is past 10000, e^9 is not"
        );
    }

    #[test]
    fn a_rotation_may_wind_past_any_bound() {
        assert!(
            law("0", "0", "1000*t", LawKind::Rotation, 0.0)
                .faults([0.0, 100.0].into_iter())
                .is_empty()
        );
        assert_eq!(
            law("0", "0", "1000*t", LawKind::Position, 0.0)
                .faults([0.0, 100.0].into_iter())
                .len(),
            1
        );
    }

    #[test]
    fn a_value_law_has_one_number() {
        assert_eq!(
            law("1/t", "0", "0", LawKind::Value, 0.0)
                .faults([0.0].into_iter())
                .len(),
            1
        );
    }

    #[test]
    fn an_opacity_law_faults_outside_0_to_1_and_is_held_within_it() {
        let fading = law("1 - t", "0", "0", LawKind::Opacity, 0.0);

        assert!(
            fading.faults([0.0, 0.5, 1.0].into_iter()).is_empty(),
            "1 - t reaches 0 at law time 1"
        );
        assert_eq!(fading.faults([1.5].into_iter()).len(), 1);
        assert_eq!(fading.at(1.5, [1.0, 0.0, 0.0]), [0.0, 0.0, 0.0]);
        assert_eq!(fading.at(-1.0, [1.0, 0.0, 0.0]), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn law_time_counts_from_the_law_start() {
        let faults =
            law("sqrt(t)", "0", "0", LawKind::Position, 3.5).faults([3.0, 4.0].into_iter());

        assert_eq!((faults[0].scene_time, faults[0].law_time), (3.0, -0.5));
    }

    #[test]
    fn a_coordinate_with_no_value_keeps_the_held_value_and_a_position_stops_at_farthest() {
        let law = law("1/t", "exp(t)", "t", LawKind::Position, 0.0);

        assert_eq!(
            law.at(0.0, [7.0, 8.0, 9.0]),
            [7.0, 1.0, 0.0],
            "1/0 has no finite value"
        );
        assert_eq!(law.at(20.0, [7.0, 8.0, 9.0]), [0.05, FARTHEST, 20.0]);
    }

    fn orientation(degrees: Vec3) -> Quat {
        let [x, y, z] = degrees.to_array().map(f32::to_radians);
        Quat::from_euler(EulerRot::XYZ, x, y, z)
    }

    fn turns_alike(a: Vec3, b: Vec3) -> bool {
        [Vec3::X, Vec3::Y]
            .iter()
            .all(|axis| (orientation(a) * *axis).distance(orientation(b) * *axis) < 1e-4)
    }

    #[test]
    fn the_nearest_equivalent_angles_give_the_same_orientation() {
        for (principal, reference) in [
            (Vec3::new(10.0, 20.0, 30.0), Vec3::new(370.0, 20.0, -330.0)),
            (Vec3::new(10.0, 20.0, 30.0), Vec3::new(190.0, 160.0, 210.0)),
            (Vec3::new(-170.0, 5.0, 175.0), Vec3::new(185.0, 0.0, -180.0)),
        ] {
            let nearest = nearest_equivalent(principal, reference);

            assert!(
                turns_alike(nearest, principal),
                "{nearest} turns as {principal} does"
            );
            assert!(
                nearest.distance(reference) < 20.0,
                "{nearest} near {reference}"
            );
        }
    }

    #[test]
    fn a_carry_faces_the_tangent_of_a_circle_and_turns_on_past_a_whole_turn() {
        let circle = Carry::of(
            ObjectId::new("ring"),
            &Shape::Flat {
                path: Path::circle(1.0).unwrap(),
                stroke: None,
                fill: None,
            },
            Some(Faces::Tangent),
        )
        .unwrap();
        let flat_to_world = FlatToWorld::Declared(crate::scene::Transform::default());

        let start = circle.at(0.0, flat_to_world, Vec3::new(0.0, 0.0, 450.0));
        let rotations: Vec<f32> = (0..=200)
            .map(|k| {
                circle
                    .at(k as f32 / 200.0, flat_to_world, Vec3::new(0.0, 0.0, 450.0))
                    .rotation
                    .unwrap()
                    .z
            })
            .collect();

        assert_eq!(start.position, Vec3::X);
        assert!(
            (start.rotation.unwrap().z - 450.0).abs() < 0.01,
            "90 degrees, a turn on from 0, as the held rotation is: {:?}",
            start.rotation
        );
        assert!(
            rotations
                .windows(2)
                .all(|pair| (pair[1] - pair[0]).abs() < 3.0),
            "no jump between shares"
        );
        assert!(
            (rotations[200] - 810.0).abs() < 0.1,
            "one whole turn on: {}",
            rotations[200]
        );
    }

    #[test]
    fn a_carry_along_a_tube_turns_on_with_no_jump() {
        // A wave whose tangent always runs partly along x, so the carried object's z axis, square
        // to it, never lies along the world's x, where Euler angles in this order lose one and
        // only the orientation stays smooth.
        let parse = |source| Expression::parse(source, crate::expr::SPACE_CURVE).unwrap();
        let wave = SpaceCurve::sample(
            &[parse("u"), parse("sin(u)"), parse("0.5*cos(u)")],
            (-6.0, 6.0),
            None,
        )
        .unwrap();
        let shape = Shape::Solid {
            solid: Solid::Tube {
                curve: Arc::new(wave),
                radius: crate::scene::Positive::try_from(0.1).unwrap(),
                cut: None,
            },
            material: Default::default(),
            edges: Default::default(),
        };
        let carry = Carry::of(ObjectId::new("spring"), &shape, Some(Faces::Tangent)).unwrap();
        let flat_to_world = FlatToWorld::Declared(crate::scene::Transform::default());
        let held = carry.at(0.0, flat_to_world, Vec3::ZERO).rotation.unwrap();

        let rotations: Vec<Vec3> = (0..=600)
            .map(|k| {
                carry
                    .at(k as f32 / 600.0, flat_to_world, held)
                    .rotation
                    .unwrap()
            })
            .collect();

        assert!(
            rotations
                .windows(2)
                .all(|pair| (pair[1] - pair[0]).abs().max_element() < 10.0),
            "no Euler angle jumps between shares"
        );
        for (k, rotation) in rotations.iter().enumerate().take(600).step_by(50) {
            let tangent = orientation(*rotation) * Vec3::X;
            let step = carry
                .at(k as f32 / 600.0 + 1e-4, flat_to_world, held)
                .position
                - carry.at(k as f32 / 600.0, flat_to_world, held).position;
            assert!(
                tangent.angle_between(step) < 0.1,
                "its x axis runs along the wave at share {k}/600"
            );
        }
    }
}
