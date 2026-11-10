//! Bodies: objects a simulation moves as rigid bodies in the world's xy plane, under gravity,
//! pushing on each other where they touch. A body is free, turns about a pivot, or is fixed.
//!
//! The world is stepped at a fixed step, single-threaded, in object id order, with sines and
//! cosines from `libm`, so the same scene gives the same motion on every machine. Each step
//! moves the bodies by velocity Verlet under gravity, then settles the contacts where they touch:
//! first every collision, one pair at a time, the pair that closes fastest first, as two bodies
//! colliding alone would; then the resting contacts, with friction, by sequential impulses; then
//! it pushes apart what still overlaps. Resolving collisions pair by pair is what keeps an
//! elastic collision's kinetic energy and sends a Newton's cradle's last ball out alone.

use std::sync::Arc;

use glam::DVec2;
use serde::Serialize;

use crate::scene::ObjectId;

/// What a body collides with, in its object's own plane at scale 1, measured from the object's
/// position.
#[derive(Clone, Debug, PartialEq)]
pub enum Collider {
    /// A circle's or a dot's disc.
    Disc(f64),
    /// A sphere, which turns as a solid ball does.
    Ball(f64),
    /// A convex polygon's corners counterclockwise, or a line's two ends.
    Corners(Arc<[DVec2]>),
}

impl Collider {
    /// The polygon or line through `points`, turned counterclockwise.
    pub fn corners(points: impl IntoIterator<Item = DVec2>) -> Collider {
        let mut points: Vec<DVec2> = points.into_iter().collect();
        if signed_area(&points) < 0.0 {
            points.reverse();
        }
        Collider::Corners(points.into())
    }

    /// Whether a body may have it: a disc, a ball, or a polygon with area that bends the same
    /// way at every corner. A body's collider is convex, so two bodies meet along one edge.
    pub fn is_convex(&self) -> bool {
        match self {
            Collider::Disc(_) | Collider::Ball(_) => true,
            Collider::Corners(points) => {
                let n = points.len();
                n > 2
                    && signed_area(points) > 0.0
                    && (0..n).all(|i| {
                        let (a, b, c) = (points[i], points[(i + 1) % n], points[(i + 2) % n]);
                        (b - a).perp_dot(c - b) >= 0.0
                    })
            }
        }
    }

    /// Whether it is a line's two ends, which only a fixed body may have, since a line has no
    /// area to give it a mass.
    pub fn is_line(&self) -> bool {
        matches!(self, Collider::Corners(points) if points.len() == 2)
    }

    /// Where the body's mass is centered, in its object's own plane, and its moment of inertia
    /// about that point for `mass`, at uniform density. `None` for a line, which has no area.
    fn mass_center_and_inertia(&self, mass: f64) -> Option<(DVec2, f64)> {
        match self {
            Collider::Disc(radius) => Some((DVec2::ZERO, mass * radius * radius / 2.0)),
            Collider::Ball(radius) => Some((DVec2::ZERO, 0.4 * mass * radius * radius)),
            Collider::Corners(points) if points.len() < 3 => None,
            Collider::Corners(points) => {
                let area = signed_area(points);
                let n = points.len();
                let (mut center, mut second) = (DVec2::ZERO, 0.0);
                for i in 0..n {
                    let (a, b) = (points[i], points[(i + 1) % n]);
                    let cross = a.perp_dot(b);
                    center += (a + b) * cross;
                    second += cross * (a.dot(a) + a.dot(b) + b.dot(b));
                }
                let center = center / (6.0 * area);
                let about_origin = mass * second / (12.0 * area);
                Some((center, about_origin - mass * center.length_squared()))
            }
        }
    }
}

fn signed_area(points: &[DVec2]) -> f64 {
    let n = points.len();
    (0..n)
        .map(|i| points[i].perp_dot(points[(i + 1) % n]))
        .sum::<f64>()
        / 2.0
}

/// One body as the simulation declares it.
#[derive(Clone, Debug)]
pub struct Body {
    pub object_id: ObjectId,
    pub collider: Collider,
    pub motion: Motion,
    /// How much of its closing speed a collision gives back, from 0 to 1. A contact takes the
    /// smaller of its two bodies', so 1, the default, leaves the choice to the other body.
    pub restitution: f64,
    /// The coefficient of friction, 0 or more. A contact takes the larger of its two bodies', so
    /// 0, the default, leaves the choice to the other body.
    pub friction: f64,
}

/// How a body may move.
#[derive(Clone, Debug)]
pub enum Motion {
    /// It moves and turns freely, or about `pivot`, a point of the world that the body's point
    /// there stays on, as a pendulum or a seesaw does.
    Moving {
        /// In kilograms.
        mass: f64,
        /// Of its center of mass at the span's start, in world units a second.
        velocity: DVec2,
        /// Counterclockwise, in degrees a second.
        spin: f64,
        pivot: Option<DVec2>,
    },
    /// It never moves: the ground, a wall, a ramp.
    Fixed,
}

/// The bodies of one simulation, in object id order, and the gravity they fall under.
#[derive(Clone, Debug)]
pub struct Bodies {
    pub bodies: Vec<Body>,
    /// Downward, along the world's -y, in world units a second squared.
    pub gravity: f64,
}

/// The quantities a simulation reads of each moving body, `<object id>.<quantity>` in an
/// expression: its position and rotation in degrees as its object's transform has them, the
/// velocity of its center of mass, its spin in degrees a second, its momentum, its kinetic
/// energy, turning included, and its speed.
pub const QUANTITIES: [&str; 10] = [
    "x", "y", "angle", "vx", "vy", "spin", "px", "py", "ke", "speed",
];

/// Where a body's object stands when the simulation starts: its position in the xy plane, and
/// its rotation about z in degrees.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    pub position: DVec2,
    pub angle: f64,
}

/// Two bodies colliding at a scene time.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Collision {
    pub scene_time: f64,
    /// The two bodies' object ids, in object id order.
    pub bodies: [ObjectId; 2],
}

/// A closing speed under this, in world units a second, is a body coming to rest on another,
/// not a collision: it gives back no speed, and is not reported.
const COLLISION_SPEED: f64 = 0.05;

/// Two bodies this close, in world units, touch: a collision passes from one to the next along a
/// row of touching balls in one step.
const TOUCHING: f64 = 1e-6;

/// How far apart two bodies may be, in world units, and still hold each other up in a step.
const CONTACT_REACH: f64 = 1e-3;

/// How far two bodies may overlap, in world units, before they are pushed apart.
const SLOP: f64 = 1e-4;

const RESTING_ITERATIONS: usize = 24;
const PUSH_ITERATIONS: usize = 4;

/// The bodies' motion over a simulation: their quantities at every kept step.
pub struct Run {
    /// `QUANTITIES` of every moving body, in body order, at each kept step.
    pub kept: Vec<f64>,
    pub collisions: Vec<Collision>,
}

impl Bodies {
    /// The moving bodies, in order.
    pub fn moving(&self) -> impl Iterator<Item = &Body> {
        self.bodies
            .iter()
            .filter(|body| matches!(body.motion, Motion::Moving { .. }))
    }

    /// Steps the bodies `count` times by `step` seconds from `placed`, one for each body, at
    /// `span_start` in scene time, and keeps their quantities at the start and after every
    /// `keep_every` steps and the last.
    pub fn run(
        &self,
        placed: &[Placed],
        span_start: f64,
        step: f64,
        count: usize,
        keep_every: usize,
    ) -> Run {
        let mut world = World::new(self, placed);
        let mut run = Run {
            kept: Vec::new(),
            collisions: Vec::new(),
        };
        world.keep(&mut run.kept);
        for k in 1..=count {
            let pairs = world.step(step);
            let scene_time = span_start + step * k as f64;
            run.collisions
                .extend(pairs.into_iter().map(|(a, b)| Collision {
                    scene_time,
                    bodies: [
                        self.bodies[a].object_id.clone(),
                        self.bodies[b].object_id.clone(),
                    ],
                }));
            if k % keep_every == 0 || k == count {
                world.keep(&mut run.kept);
            }
        }
        run
    }
}

/// A body as the world steps it. A free body turns about its center of mass and a pivoted one
/// about its pivot, its turning center; a pivoted body's center of mass moves only by turning,
/// so its `linear` velocity stays 0 and its inverse mass is 0.
struct Rigid {
    /// The collider about the center of mass: a radius, or the corners measured from it.
    shape: Shape,
    /// The center of mass in its object's own plane, which the object's position is drawn from.
    mass_center: DVec2,
    inverse_mass: f64,
    /// About the turning center.
    inverse_inertia: f64,
    mass: f64,
    /// About the center of mass.
    inertia: f64,
    pivot: Option<DVec2>,
    center: DVec2,
    /// In radians.
    angle: f64,
    linear: DVec2,
    /// In radians a second.
    spin: f64,
    restitution: f64,
    friction: f64,
    moving: bool,
}

#[derive(Clone)]
enum Shape {
    Round(f64),
    Corners(Arc<[DVec2]>),
}

impl Rigid {
    fn turning_center(&self) -> DVec2 {
        self.pivot.unwrap_or(self.center)
    }

    fn velocity_at(&self, point: DVec2) -> DVec2 {
        self.linear + (point - self.turning_center()).perp() * self.spin
    }

    fn push(&mut self, point: DVec2, impulse: DVec2) {
        self.linear += impulse * self.inverse_mass;
        self.spin += (point - self.turning_center()).perp_dot(impulse) * self.inverse_inertia;
    }

    /// The inverse of the mass an impulse along `direction` at `point` meets.
    fn give(&self, point: DVec2, direction: DVec2) -> f64 {
        let lever = (point - self.turning_center()).perp_dot(direction);
        self.inverse_mass + lever * lever * self.inverse_inertia
    }

    /// Moves the body as `push` would its velocity, by a shift of position instead.
    fn shift(&mut self, point: DVec2, shift: DVec2) {
        let turn = (point - self.turning_center()).perp_dot(shift) * self.inverse_inertia;
        match self.pivot {
            Some(pivot) => self.turn_about(pivot, turn),
            None => {
                self.center += shift * self.inverse_mass;
                self.angle += turn;
            }
        }
    }

    fn turn_about(&mut self, pivot: DVec2, turn: f64) {
        self.center = pivot + rotate(self.center - pivot, turn);
        self.angle += turn;
    }

    fn center_velocity(&self) -> DVec2 {
        self.velocity_at(self.center)
    }

    /// The acceleration gravity gives the body: of its center of mass when free, or of its turn
    /// when pivoted.
    fn fall(&self, gravity: f64) -> (DVec2, f64) {
        match self.pivot {
            _ if !self.moving => (DVec2::ZERO, 0.0),
            None => (DVec2::new(0.0, -gravity), 0.0),
            Some(pivot) => {
                let torque = (self.center - pivot).perp_dot(DVec2::new(0.0, -gravity * self.mass));
                (DVec2::ZERO, torque * self.inverse_inertia)
            }
        }
    }

    fn kick(&mut self, gravity: f64, dt: f64) {
        let (linear, spin) = self.fall(gravity);
        self.linear += linear * dt;
        self.spin += spin * dt;
    }

    fn drift(&mut self, dt: f64) {
        match self.pivot {
            Some(pivot) => self.turn_about(pivot, self.spin * dt),
            None => {
                self.center += self.linear * dt;
                self.angle += self.spin * dt;
            }
        }
    }

    /// The collider where the body stands: a radius about its center, or its corners.
    fn placed(&self) -> InWorld {
        match &self.shape {
            Shape::Round(radius) => InWorld::Round(self.center, *radius),
            Shape::Corners(corners) => InWorld::Corners(
                corners
                    .iter()
                    .map(|corner| self.center + rotate(*corner, self.angle))
                    .collect(),
            ),
        }
    }
}

fn rotate(v: DVec2, angle: f64) -> DVec2 {
    let (sin, cos) = (libm::sin(angle), libm::cos(angle));
    DVec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos)
}

enum InWorld {
    Round(DVec2, f64),
    Corners(Vec<DVec2>),
}

/// Where two bodies touch or overlap: the normal from `a` toward `b`, and up to two points, each
/// with how deep the bodies overlap there, less than 0 for a gap.
struct Contact {
    a: usize,
    b: usize,
    normal: DVec2,
    points: Vec<(DVec2, f64)>,
}

impl Contact {
    fn middle(&self) -> DVec2 {
        self.points.iter().map(|(point, _)| *point).sum::<DVec2>() / self.points.len() as f64
    }

    fn deepest(&self) -> (DVec2, f64) {
        self.points
            .iter()
            .copied()
            .fold((DVec2::ZERO, f64::NEG_INFINITY), |deepest, point| {
                if point.1 > deepest.1 { point } else { deepest }
            })
    }
}

struct World {
    rigids: Vec<Rigid>,
    gravity: f64,
}

impl World {
    fn new(bodies: &Bodies, placed: &[Placed]) -> World {
        let rigids = bodies
            .bodies
            .iter()
            .zip(placed)
            .map(|(body, placed)| {
                let angle = placed.angle.to_radians();
                let (mass, velocity, spin, pivot, moving) = match &body.motion {
                    Motion::Moving {
                        mass,
                        velocity,
                        spin,
                        pivot,
                    } => (*mass, *velocity, spin.to_radians(), *pivot, true),
                    Motion::Fixed => (0.0, DVec2::ZERO, 0.0, None, false),
                };
                let (mass_center, inertia) = body
                    .collider
                    .mass_center_and_inertia(mass.max(1.0))
                    .unwrap_or((DVec2::ZERO, 0.0));
                let center = placed.position + rotate(mass_center, angle);
                let shape = match &body.collider {
                    Collider::Disc(radius) | Collider::Ball(radius) => Shape::Round(*radius),
                    Collider::Corners(corners) => {
                        Shape::Corners(corners.iter().map(|c| *c - mass_center).collect())
                    }
                };
                let (inverse_mass, inverse_inertia, linear, spin) = match (moving, pivot) {
                    (false, _) => (0.0, 0.0, DVec2::ZERO, 0.0),
                    (true, None) => (1.0 / mass, 1.0 / inertia, velocity, spin),
                    (true, Some(pivot)) => {
                        let arm = center - pivot;
                        let about_pivot = inertia + mass * arm.length_squared();
                        // A pivoted body only turns: the turn whose velocity at its center of
                        // mass is nearest the one declared, plus its own spin.
                        let turn = if arm.length_squared() > 0.0 {
                            arm.perp_dot(velocity) / arm.length_squared()
                        } else {
                            spin
                        };
                        (0.0, 1.0 / about_pivot, DVec2::ZERO, turn)
                    }
                };
                Rigid {
                    shape,
                    mass_center,
                    inverse_mass,
                    inverse_inertia,
                    mass,
                    inertia,
                    pivot,
                    center,
                    angle,
                    linear,
                    spin,
                    restitution: body.restitution,
                    friction: body.friction,
                    moving,
                }
            })
            .collect();
        World {
            rigids,
            gravity: bodies.gravity,
        }
    }

    fn keep(&self, kept: &mut Vec<f64>) {
        for rigid in self.rigids.iter().filter(|rigid| rigid.moving) {
            let position = rigid.center - rotate(rigid.mass_center, rigid.angle);
            let velocity = rigid.center_velocity();
            let speed_squared = velocity.length_squared();
            kept.extend([
                position.x,
                position.y,
                rigid.angle.to_degrees(),
                velocity.x,
                velocity.y,
                rigid.spin.to_degrees(),
                rigid.mass * velocity.x,
                rigid.mass * velocity.y,
                0.5 * rigid.mass * speed_squared + 0.5 * rigid.inertia * rigid.spin * rigid.spin,
                speed_squared.sqrt(),
            ]);
        }
    }

    /// One step: gravity by velocity Verlet, then the contacts. Returns each pair of bodies that
    /// collided in it, once.
    fn step(&mut self, dt: f64) -> Vec<(usize, usize)> {
        let gravity = self.gravity;
        for rigid in &mut self.rigids {
            rigid.kick(gravity, dt / 2.0);
            rigid.drift(dt);
            rigid.kick(gravity, dt / 2.0);
        }
        let contacts = self.contacts();
        let collided = self.collide(&contacts, dt);
        self.rest(&contacts, dt);
        for _ in 0..PUSH_ITERATIONS {
            self.push_apart();
        }
        collided
    }

    /// Every pair of bodies, not both fixed, within `CONTACT_REACH` of each other.
    fn contacts(&self) -> Vec<Contact> {
        let placed: Vec<InWorld> = self.rigids.iter().map(Rigid::placed).collect();
        let mut contacts = Vec::new();
        for a in 0..self.rigids.len() {
            for b in a + 1..self.rigids.len() {
                if !self.rigids[a].moving && !self.rigids[b].moving {
                    continue;
                }
                if let Some((normal, points)) = touch(&placed[a], &placed[b]) {
                    contacts.push(Contact {
                        a,
                        b,
                        normal,
                        points,
                    });
                }
            }
        }
        contacts
    }

    /// How fast the bodies of `contact` close along its normal at `point`: less than 0 when they
    /// close.
    fn closing(&self, contact: &Contact, point: DVec2, direction: DVec2) -> f64 {
        direction.dot(
            self.rigids[contact.b].velocity_at(point) - self.rigids[contact.a].velocity_at(point),
        )
    }

    fn impulse(&mut self, contact: &Contact, point: DVec2, impulse: DVec2) {
        self.rigids[contact.a].push(point, -impulse);
        self.rigids[contact.b].push(point, impulse);
    }

    fn give(&self, contact: &Contact, point: DVec2, direction: DVec2) -> f64 {
        self.rigids[contact.a].give(point, direction)
            + self.rigids[contact.b].give(point, direction)
    }

    /// Settles every collision, the pair that closes fastest first, each as the two bodies
    /// colliding alone would, until no pair closes faster than `COLLISION_SPEED`. A pair
    /// collides once it touches, or would close its gap within the step.
    fn collide(&mut self, contacts: &[Contact], dt: f64) -> Vec<(usize, usize)> {
        let mut collided: Vec<(usize, usize)> = Vec::new();
        for _ in 0..64 * contacts.len().max(1) {
            let fastest = contacts
                .iter()
                .map(|contact| {
                    (
                        contact,
                        self.closing(contact, contact.middle(), contact.normal),
                    )
                })
                .filter(|(contact, closing)| {
                    *closing < -COLLISION_SPEED
                        && -contact.deepest().1 <= TOUCHING.max(-closing * dt)
                })
                .min_by(|x, y| x.1.total_cmp(&y.1));
            let Some((contact, closing)) = fastest else {
                break;
            };
            let point = contact.middle();
            let (a, b) = (&self.rigids[contact.a], &self.rigids[contact.b]);
            let restitution = a.restitution.min(b.restitution);
            let friction = a.friction.max(b.friction);
            let normal = -(1.0 + restitution) * closing / self.give(contact, point, contact.normal);
            self.impulse(contact, point, contact.normal * normal);
            let tangent = contact.normal.perp();
            let sliding = self.closing(contact, point, tangent);
            let across = (-sliding / self.give(contact, point, tangent))
                .clamp(-friction * normal, friction * normal);
            self.impulse(contact, point, tangent * across);
            if !collided.contains(&(contact.a, contact.b)) {
                collided.push((contact.a, contact.b));
            }
        }
        collided
    }

    /// Holds bodies up on each other, with friction, by sequential impulses at every contact
    /// point, so no point closes faster than its gap closes in one step.
    fn rest(&mut self, contacts: &[Contact], dt: f64) {
        let mut held: Vec<Vec<(f64, f64)>> = contacts
            .iter()
            .map(|contact| vec![(0.0, 0.0); contact.points.len()])
            .collect();
        for _ in 0..RESTING_ITERATIONS {
            for (contact, held) in contacts.iter().zip(&mut held) {
                let friction = self.rigids[contact.a]
                    .friction
                    .max(self.rigids[contact.b].friction);
                for (&(point, depth), (normal_held, tangent_held)) in
                    contact.points.iter().zip(held.iter_mut())
                {
                    let allowed = (-depth).max(0.0) / dt;
                    let closing = self.closing(contact, point, contact.normal);
                    let more = -(closing + allowed) / self.give(contact, point, contact.normal);
                    let total = (*normal_held + more).max(0.0);
                    self.impulse(contact, point, contact.normal * (total - *normal_held));
                    *normal_held = total;
                    let tangent = contact.normal.perp();
                    let sliding = self.closing(contact, point, tangent);
                    let limit = friction * *normal_held;
                    let total = (*tangent_held - sliding / self.give(contact, point, tangent))
                        .clamp(-limit, limit);
                    self.impulse(contact, point, tangent * (total - *tangent_held));
                    *tangent_held = total;
                }
            }
        }
    }

    /// Moves overlapping bodies most of the way apart, each by the share its mass and turning
    /// give, at the deepest point of each contact.
    fn push_apart(&mut self) {
        for contact in self.contacts() {
            let (point, depth) = contact.deepest();
            if depth <= SLOP {
                continue;
            }
            let amount = 0.8 * (depth - SLOP) / self.give(&contact, point, contact.normal);
            self.rigids[contact.a].shift(point, -contact.normal * amount);
            self.rigids[contact.b].shift(point, contact.normal * amount);
        }
    }
}

/// Where `a` and `b` touch, if they lie within `CONTACT_REACH`: the normal from `a` toward `b`
/// and the contact points with their depths.
fn touch(a: &InWorld, b: &InWorld) -> Option<(DVec2, Vec<(DVec2, f64)>)> {
    match (a, b) {
        (InWorld::Round(ca, ra), InWorld::Round(cb, rb)) => {
            let between = *cb - *ca;
            let distance = between.length();
            let depth = ra + rb - distance;
            if depth < -CONTACT_REACH {
                return None;
            }
            let normal = if distance > 0.0 {
                between / distance
            } else {
                DVec2::Y
            };
            Some((normal, vec![(*ca + normal * (ra - depth / 2.0), depth)]))
        }
        (InWorld::Corners(corners), InWorld::Round(center, radius)) => {
            corners_and_round(corners, *center, *radius)
        }
        (InWorld::Round(center, radius), InWorld::Corners(corners)) => {
            corners_and_round(corners, *center, *radius).map(|(normal, points)| (-normal, points))
        }
        (InWorld::Corners(a), InWorld::Corners(b)) => corners_and_corners(a, b),
    }
}

/// The outward normal of each edge of a counterclockwise polygon, edge `i` from corner `i` to
/// the next. A line's two edges face both ways.
fn edge_normals(corners: &[DVec2]) -> Vec<DVec2> {
    let n = corners.len();
    (0..n)
        .map(|i| {
            let edge = corners[(i + 1) % n] - corners[i];
            DVec2::new(edge.y, -edge.x).normalize()
        })
        .collect()
}

fn corners_and_round(
    corners: &[DVec2],
    center: DVec2,
    radius: f64,
) -> Option<(DVec2, Vec<(DVec2, f64)>)> {
    let n = corners.len();
    let normals = edge_normals(corners);
    let (face, separation) = (0..n)
        .map(|i| (i, normals[i].dot(center - corners[i])))
        .max_by(|x, y| x.1.total_cmp(&y.1))
        .expect("a collider has corners");
    if separation > radius + CONTACT_REACH {
        return None;
    }
    if separation <= 0.0 && n > 2 {
        let normal = normals[face];
        return Some((
            normal,
            vec![(center - normal * radius, radius - separation)],
        ));
    }
    let (nearest, distance) = (0..n)
        .map(|i| {
            let (start, end) = (corners[i], corners[(i + 1) % n]);
            let along = end - start;
            let share = ((center - start).dot(along) / along.length_squared()).clamp(0.0, 1.0);
            let point = start + along * share;
            (point, center.distance(point))
        })
        .min_by(|x, y| x.1.total_cmp(&y.1))
        .expect("a collider has corners");
    let depth = radius - distance;
    if depth < -CONTACT_REACH {
        return None;
    }
    let normal = if distance > 1e-12 {
        (center - nearest) / distance
    } else {
        normals[face]
    };
    Some((normal, vec![(nearest, depth)]))
}

/// The edge of `of` that `other` lies farthest outside of, and how far.
fn farthest_outside(of: &[DVec2], other: &[DVec2]) -> (usize, f64) {
    let normals = edge_normals(of);
    (0..of.len())
        .map(|i| {
            let deepest = other
                .iter()
                .map(|corner| normals[i].dot(*corner - of[i]))
                .fold(f64::INFINITY, f64::min);
            (i, deepest)
        })
        .max_by(|x, y| x.1.total_cmp(&y.1))
        .expect("a collider has corners")
}

fn corners_and_corners(a: &[DVec2], b: &[DVec2]) -> Option<(DVec2, Vec<(DVec2, f64)>)> {
    let (edge_a, outside_a) = farthest_outside(a, b);
    let (edge_b, outside_b) = farthest_outside(b, a);
    if outside_a.max(outside_b) > CONTACT_REACH {
        return None;
    }
    // Prefer `a`'s face unless `b`'s is clearly better, so a tie never flips between steps.
    let (reference, incident, edge, flipped) = if outside_b > 0.98 * outside_a + 1e-6 {
        (b, a, edge_b, true)
    } else {
        (a, b, edge_a, false)
    };
    let n = reference.len();
    let (start, end) = (reference[edge], reference[(edge + 1) % n]);
    let normal = edge_normals(reference)[edge];
    let incident_normals = edge_normals(incident);
    let m = incident.len();
    let facing = (0..m)
        .min_by(|i, j| {
            normal
                .dot(incident_normals[*i])
                .total_cmp(&normal.dot(incident_normals[*j]))
        })
        .expect("a collider has corners");
    let mut points = vec![incident[facing], incident[(facing + 1) % m]];
    let tangent = (end - start).normalize();
    points = clip(&points, -tangent, -tangent.dot(start));
    points = clip(&points, tangent, tangent.dot(end));
    let points: Vec<(DVec2, f64)> = points
        .into_iter()
        .map(|point| (point, -normal.dot(point - start)))
        .filter(|(_, depth)| *depth >= -CONTACT_REACH)
        .collect();
    if points.is_empty() {
        return None;
    }
    Some((if flipped { -normal } else { normal }, points))
}

/// The part of the segment through `points` where `direction · p <= limit`.
fn clip(points: &[DVec2], direction: DVec2, limit: f64) -> Vec<DVec2> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let (p, q) = (points[0], points[1]);
    let (dp, dq) = (direction.dot(p) - limit, direction.dot(q) - limit);
    let mut kept = Vec::new();
    if dp <= 0.0 {
        kept.push(p);
    }
    if dq <= 0.0 {
        kept.push(q);
    }
    if dp * dq < 0.0 {
        kept.push(p + (q - p) * (dp / (dp - dq)));
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(id: &str, collider: Collider, motion: Motion) -> Body {
        Body {
            object_id: ObjectId::new(id),
            collider,
            motion,
            restitution: 1.0,
            friction: 0.0,
        }
    }

    fn moving(mass: f64, velocity: [f64; 2]) -> Motion {
        Motion::Moving {
            mass,
            velocity: DVec2::from(velocity),
            spin: 0.0,
            pivot: None,
        }
    }

    fn rectangle(half_width: f64, half_height: f64) -> Collider {
        Collider::corners([
            DVec2::new(-half_width, -half_height),
            DVec2::new(half_width, -half_height),
            DVec2::new(half_width, half_height),
            DVec2::new(-half_width, half_height),
        ])
    }

    fn at(x: f64, y: f64) -> Placed {
        Placed {
            position: DVec2::new(x, y),
            angle: 0.0,
        }
    }

    /// The quantities of `bodies` stepped for `seconds` at 960 steps a second, kept at every step.
    fn run(bodies: &Bodies, placed: &[Placed], seconds: f64) -> (Vec<Vec<f64>>, Vec<Collision>) {
        let count = (seconds * 960.0).round() as usize;
        let run = bodies.run(placed, 0.0, 1.0 / 960.0, count, 1);
        let size = bodies.moving().count() * QUANTITIES.len();
        (
            run.kept.chunks(size).map(<[f64]>::to_vec).collect(),
            run.collisions,
        )
    }

    fn quantity(keep: &[f64], body: usize, name: &str) -> f64 {
        let k = QUANTITIES.iter().position(|q| *q == name).unwrap();
        keep[body * QUANTITIES.len() + k]
    }

    fn two_carts(restitution: f64) -> (Vec<f64>, Vec<Collision>) {
        let mut b = body("cart_b", rectangle(0.5, 0.3), moving(1.0, [0.0, 0.0]));
        b.restitution = restitution;
        let bodies = Bodies {
            bodies: vec![
                body("cart_a", rectangle(0.5, 0.3), moving(2.0, [3.0, 0.0])),
                b,
            ],
            gravity: 0.0,
        };
        let (kept, collisions) = run(&bodies, &[at(-2.0, 0.0), at(0.0, 0.0)], 1.0);
        (kept.last().unwrap().clone(), collisions)
    }

    #[test]
    fn an_elastic_collision_keeps_momentum_and_kinetic_energy() {
        let (after, collisions) = two_carts(1.0);

        // 2 kg at 3 m/s into 1 kg at rest: (m1 - m2)/(m1 + m2)·3 and 2·m1/(m1 + m2)·3.
        let (va, vb) = (quantity(&after, 0, "vx"), quantity(&after, 1, "vx"));
        assert!(
            (va - 1.0).abs() < 1e-9 && (vb - 4.0).abs() < 1e-9,
            "{va} {vb}"
        );
        let momentum = quantity(&after, 0, "px") + quantity(&after, 1, "px");
        let energy = quantity(&after, 0, "ke") + quantity(&after, 1, "ke");
        assert!((momentum - 6.0).abs() < 1e-9, "{momentum}");
        assert!((energy - 9.0).abs() < 1e-9, "{energy}");
        assert_eq!(quantity(&after, 0, "spin"), 0.0);
        assert_eq!(collisions.len(), 1);
        assert_eq!(
            collisions[0].bodies.each_ref().map(|id| id.to_string()),
            ["cart_a", "cart_b"]
        );
        // The gap of 1 closes at 3 m/s.
        assert!((collisions[0].scene_time - 1.0 / 3.0).abs() < 2.0 / 960.0);
    }

    #[test]
    fn a_perfectly_inelastic_collision_keeps_momentum_and_loses_the_textbook_energy() {
        let (after, _) = two_carts(0.0);

        let (va, vb) = (quantity(&after, 0, "vx"), quantity(&after, 1, "vx"));
        assert!(
            (va - 2.0).abs() < 1e-9 && (vb - 2.0).abs() < 1e-9,
            "{va} {vb}"
        );
        // Lost: m1·m2/(2(m1 + m2))·3² = 3 J of the 9 J.
        let energy = quantity(&after, 0, "ke") + quantity(&after, 1, "ke");
        assert!((energy - 6.0).abs() < 1e-9, "{energy}");
    }

    #[test]
    fn a_partly_elastic_collision_closes_at_its_restitution_times_the_speed_it_met() {
        let (after, _) = two_carts(0.5);

        let parting = quantity(&after, 1, "vx") - quantity(&after, 0, "vx");
        let momentum = quantity(&after, 0, "px") + quantity(&after, 1, "px");
        assert!((parting - 1.5).abs() < 1e-9, "{parting}");
        assert!((momentum - 6.0).abs() < 1e-9, "{momentum}");
    }

    fn pivoted(mass: f64, pivot: [f64; 2]) -> Motion {
        Motion::Moving {
            mass,
            velocity: DVec2::ZERO,
            spin: 0.0,
            pivot: Some(DVec2::from(pivot)),
        }
    }

    #[test]
    fn a_newtons_cradle_sends_out_only_its_last_ball_at_the_first_balls_speed() {
        let (length, radius, pulled) = (2.0, 0.25, 0.5f64);
        let bodies = Bodies {
            bodies: (0..5)
                .map(|k| {
                    let x = 2.0 * radius * k as f64;
                    body(
                        &format!("ball_{k}"),
                        Collider::Ball(radius),
                        pivoted(1.0, [x, length]),
                    )
                })
                .collect(),
            gravity: 9.8,
        };
        let placed: Vec<Placed> = (0..5)
            .map(|k| {
                let x = 2.0 * radius * k as f64;
                if k == 0 {
                    at(x - length * pulled.sin(), length - length * pulled.cos())
                } else {
                    at(x, 0.0)
                }
            })
            .collect();

        let (kept, collisions) = run(&bodies, &placed, 1.2);

        // A ball that only turns about its pivot: m·g·h = ½·(I + m·L²)·ω², and v = ω·L.
        let inertia = 0.4 * radius * radius + length * length;
        let height = length * (1.0 - pulled.cos());
        let fastest = (2.0 * 9.8 * height * length * length / inertia).sqrt();
        let after = &kept[(1.0 * 960.0) as usize];
        // The first ball strikes a hair before the bottom, so it keeps a trace of its speed.
        assert!(quantity(after, 0, "speed") < 1e-3 * 2.5, "{after:?}");
        for k in 1..4 {
            assert!(
                quantity(after, k, "speed") < 1e-9,
                "ball {k} moves: {after:?}"
            );
        }
        let last = kept
            .iter()
            .map(|keep| quantity(keep, 4, "speed"))
            .fold(0.0, f64::max);
        assert!((last - fastest).abs() < 1e-3 * fastest, "{last} {fastest}");
        let first: Vec<[String; 2]> = collisions
            .iter()
            .map(|c| c.bodies.each_ref().map(|id| id.to_string()))
            .collect();
        assert_eq!(
            first[..4],
            [
                ["ball_0", "ball_1"],
                ["ball_1", "ball_2"],
                ["ball_2", "ball_3"],
                ["ball_3", "ball_4"]
            ]
            .map(|pair| pair.map(String::from))
        );
        assert!(
            collisions[..4]
                .iter()
                .all(|c| c.scene_time == collisions[0].scene_time)
        );
    }

    fn block_on_ramp(friction: f64) -> f64 {
        let slope = 30f64.to_radians();
        let normal = DVec2::new(-slope.sin(), slope.cos());
        let mut block = body("block", rectangle(0.25, 0.25), moving(1.0, [0.0, 0.0]));
        block.friction = friction;
        let bodies = Bodies {
            bodies: vec![block, body("ramp", rectangle(10.0, 0.1), Motion::Fixed)],
            gravity: 9.8,
        };
        let on_ramp = normal * 0.35;
        let placed = [
            Placed {
                position: on_ramp,
                angle: 30.0,
            },
            Placed {
                position: DVec2::ZERO,
                angle: 30.0,
            },
        ];
        let (kept, _) = run(&bodies, &placed, 1.0);
        quantity(kept.last().unwrap(), 0, "speed")
    }

    #[test]
    fn a_block_slides_down_a_ramp_at_g_times_sin_less_friction_times_cos() {
        let slope = 30f64.to_radians();
        let want = 9.8 * (slope.sin() - 0.3 * slope.cos());

        let speed = block_on_ramp(0.3);

        assert!((speed - want).abs() < 2e-3 * want, "{speed} {want}");
    }

    #[test]
    fn friction_past_the_tangent_of_the_slope_holds_a_block_at_rest() {
        assert!(block_on_ramp(0.7) < 1e-3);
    }

    #[test]
    fn a_projectile_lands_where_the_range_formula_says() {
        let bodies = Bodies {
            bodies: vec![body("ball", Collider::Disc(0.1), moving(1.0, [7.0, 7.0]))],
            gravity: 9.8,
        };

        let (kept, _) = run(&bodies, &[at(0.0, 0.0)], 1.0);

        let after = kept.last().unwrap();
        assert!((quantity(after, 0, "x") - 7.0).abs() < 1e-9);
        assert!((quantity(after, 0, "y") - (7.0 - 4.9)).abs() < 1e-9);
        assert!((quantity(after, 0, "vy") - (7.0 - 9.8)).abs() < 1e-9);
    }

    #[test]
    fn a_pendulum_strikes_a_block_as_a_body_of_its_moment_about_the_pivot() {
        let (length, radius, pulled) = (2.0, 0.15, 0.6f64);
        let bodies = Bodies {
            bodies: vec![
                body("block", rectangle(0.2, 0.2), moving(1.0, [0.0, 0.0])),
                body("bob", Collider::Ball(radius), pivoted(1.0, [0.0, length])),
                body("ground", rectangle(5.0, 0.1), Motion::Fixed),
            ],
            gravity: 9.8,
        };
        let placed = [
            at(0.4, 0.0),
            at(-length * pulled.sin(), length - length * pulled.cos()),
            at(0.0, -0.3),
        ];

        let (kept, collisions) = run(&bodies, &placed, 1.5);

        // At the bottom the bob meets the block as a mass of I/L² along x at its center's height.
        let inertia = 0.4 * radius * radius + length * length;
        let spin = (2.0 * 9.8 * length * (1.0 - pulled.cos()) / inertia).sqrt();
        let meets = inertia / (length * length);
        let want = 2.0 * meets / (meets + 1.0) * spin * length;
        let block = quantity(kept.last().unwrap(), 0, "vx");
        assert!((block - want).abs() < 5e-3 * want, "{block} {want}");
        assert_eq!(collisions[0].bodies[0].to_string(), "block");
    }

    fn seesaw(right_mass: f64) -> f64 {
        let mut left = body("left", rectangle(0.2, 0.2), moving(2.0, [0.0, 0.0]));
        let mut right = body("right", rectangle(0.2, 0.2), moving(right_mass, [0.0, 0.0]));
        left.friction = 0.6;
        right.friction = 0.6;
        let bodies = Bodies {
            bodies: vec![
                left,
                body("plank", rectangle(2.5, 0.1), pivoted(1.0, [0.0, 0.0])),
                right,
            ],
            gravity: 9.8,
        };
        let (kept, _) = run(&bodies, &[at(-1.0, 0.3), at(0.0, 0.0), at(2.0, 0.3)], 2.0);
        quantity(kept.last().unwrap(), 1, "angle")
    }

    #[test]
    fn a_seesaw_balances_when_the_moments_about_its_pivot_are_equal() {
        // 2 kg at 1 m on the left, 1 kg at 2 m on the right.
        let level = seesaw(1.0);
        let tipped = seesaw(1.5);

        assert!(level.abs() < 0.1, "{level}");
        assert!(tipped < -5.0, "{tipped}");
    }

    #[test]
    fn the_same_bodies_give_the_same_motion_to_the_bit() {
        let bodies = Bodies {
            bodies: (0..6)
                .map(|k| {
                    let mut box_body = body(
                        &format!("box_{k}"),
                        rectangle(0.3, 0.2),
                        moving(1.0, [1.0, 0.0]),
                    );
                    box_body.restitution = 0.4;
                    box_body.friction = 0.5;
                    box_body
                })
                .chain([body("ground", rectangle(20.0, 0.1), Motion::Fixed)])
                .collect(),
            gravity: 9.8,
        };
        let placed: Vec<Placed> = (0..6)
            .map(|k| Placed {
                position: DVec2::new((k % 3) as f64 * 0.7, 1.0 + (k / 3) as f64 * 0.6),
                angle: k as f64 * 17.0,
            })
            .chain([at(0.0, -0.1)])
            .collect();

        let first = bodies.run(&placed, 0.0, 1.0 / 960.0, 1920, 16);
        let second = bodies.run(&placed, 0.0, 1.0 / 960.0, 1920, 16);

        let bits = |kept: &[f64]| kept.iter().map(|v| v.to_bits()).collect::<Vec<u64>>();
        assert_eq!(bits(&first.kept), bits(&second.kept));
        assert_eq!(first.collisions, second.collisions);
    }
}
