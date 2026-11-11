//! Vector fields: the electric field of charges and the magnetic field of wires, loops and
//! magnets, drawn as arrows or as field lines and worked out again at every scene time from where
//! each field source's object stands.
//!
//! The units take Coulomb's constant and μ0/4π as 1: a charge q gives q r̂/r², a straight wire of
//! current I gives 2I/ρ around it, and a circular loop of current I and radius a gives
//! 2πIa²/(a² + z²)^(3/2) on its axis. A loop off its axis is exact, by the complete elliptic
//! integrals, and a magnet is an ideal solenoid, the limit of a stack of loops, by the formula of
//! Derby and Olbert (Am. J. Phys. 78, 229, 2010), so its field lines run through its body and
//! close. Both evaluate Bulirsch's generalized complete elliptic integral, an arithmetic-geometric
//! mean iteration.

use std::f64::consts::PI;
use std::sync::Arc;

use glam::{DMat3, DQuat, DVec2, DVec3, EulerRot};
use serde::Serialize;

use crate::scene::{Object, ObjectId, Shape, Stroke, Transform};

/// Which field a vector field draws, and so which field sources it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FieldOf {
    Electric,
    Magnetic,
}

/// A field source and the object whose transform places it at every scene time.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldSource<S> {
    pub object: ObjectId,
    pub source: S,
}

/// A point charge at its object's position, signed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Charge {
    pub charge: f64,
}

/// What makes a magnetic field. Each lies in its object's own space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MagneticSource {
    /// An infinitely long straight wire through its object's position, along `direction`, a unit
    /// vector in its object's own space, carrying `current` that way.
    Wire { current: f64, direction: DVec3 },
    /// A circle of `radius` in its object's own xy plane about its position, carrying `current`
    /// counterclockwise seen from its own +z.
    Loop { radius: f64, current: f64 },
    /// A bar magnet `length` long and of `radius` along its object's own x axis, centered on its
    /// position, its north end toward +x: an ideal solenoid carrying `strength` of current per
    /// unit of its length.
    Magnet {
        length: f64,
        radius: f64,
        strength: f64,
    },
}

/// The field sources of a vector field: charges for an electric field, and wires, loops and
/// magnets for a magnetic one, so no field holds a source of the other kind.
#[derive(Clone, Debug, PartialEq)]
pub enum Sources {
    Electric(Arc<[FieldSource<Charge>]>),
    Magnetic(Arc<[FieldSource<MagneticSource>]>),
}

impl Sources {
    pub fn of(&self) -> FieldOf {
        match self {
            Sources::Electric(_) => FieldOf::Electric,
            Sources::Magnetic(_) => FieldOf::Magnetic,
        }
    }

    /// The object of each field source, in order.
    pub fn objects(&self) -> Vec<&ObjectId> {
        match self {
            Sources::Electric(sources) => sources.iter().map(|s| &s.object).collect(),
            Sources::Magnetic(sources) => sources.iter().map(|s| &s.object).collect(),
        }
    }
}

/// How the arrows of a vector field show how strong the field is where each stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowsStrength {
    Opacity,
    Length,
    None,
}

/// Where field lines start.
#[derive(Clone, Debug, PartialEq)]
pub enum Starts {
    /// Chosen from the field sources: this many lines a unit of charge, evenly around each
    /// charge, for an electric field, and this many lines a field source for a magnetic one.
    Automatic(f64),
    /// These points of the vector field's own plane, each traced both ways.
    Seeds(Arc<[DVec2]>),
}

/// What a vector field draws.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    /// Field lines in the vector field's own xy plane, inside the rectangle `width` by `height`
    /// centered on its position, each with a tip `tip_length` long halfway along it.
    Lines {
        width: f64,
        height: f64,
        starts: Starts,
        tip_length: f64,
    },
    /// An arrow at every place of a grid of `counts` places `spacing` apart, centered on the
    /// vector field's position, turned along the field there.
    Arrows {
        counts: [u32; 3],
        spacing: f64,
        length: f64,
        tip_length: f64,
        shows_strength: ShowsStrength,
    },
}

/// An object that draws a field from its field sources, again at every scene time.
#[derive(Clone, Debug)]
pub struct VectorField {
    pub sources: Sources,
    pub draw: Draw,
    pub stroke: Stroke,
    /// What it draws at the stage's scene time, in its own space.
    pub drawing: Arc<Drawing>,
}

/// A vector field's lines or arrows at one scene time, in its own space.
#[derive(Clone, Debug, Default)]
pub struct Drawing {
    pub lines: Vec<FieldLine>,
    pub arrows: Vec<FieldArrow>,
    /// The field at each of `seeds`, in the world, for `inspect`.
    pub at_seeds: Vec<DVec3>,
}

/// One field line in the vector field's own plane, its points in the field's direction.
#[derive(Clone, Debug)]
pub struct FieldLine {
    pub points: Vec<DVec2>,
    /// Whether it closes on itself, as a magnetic field line does. Its last point is its first.
    pub closed: bool,
    /// The object of the charge it ends on, if it ends on one.
    pub ends_on: Option<ObjectId>,
    /// How far along it its tip sits: at the point it was traced both ways from, or halfway
    /// along a line that starts on a charge.
    pub tip: f64,
}

/// One arrow, centered on its place in the vector field's own space.
#[derive(Clone, Copy, Debug)]
pub struct FieldArrow {
    pub place: DVec3,
    /// The unit direction it points in, in the vector field's own space.
    pub toward: DVec3,
    /// The size of the field it points along: in the plane for a flat grid, whole for a 3D one.
    pub strength: f64,
    /// From 0 for the weakest arrows to 1 for the strongest; see `strength_share`.
    pub strength_share: f64,
}

impl FieldLine {
    pub fn length(&self) -> f64 {
        self.points.windows(2).map(|p| p[0].distance(p[1])).sum()
    }

    /// The point `along` the line from its start, and the unit direction there.
    pub fn at(&self, along: f64) -> (DVec2, DVec2) {
        let mut walked = 0.0;
        for pair in self.points.windows(2) {
            let step = pair[0].distance(pair[1]);
            if step > 0.0 && walked + step >= along {
                let direction = (pair[1] - pair[0]) / step;
                return (pair[0] + direction * (along - walked), direction);
            }
            walked += step;
        }
        let n = self.points.len();
        let last = (self.points[n - 1] - self.points[n.saturating_sub(2)]).normalize_or_zero();
        (self.points[n - 1], last)
    }
}

impl FieldArrow {
    /// The share of its full length an arrow draws, and its opacity, by `shows`.
    pub fn look(&self, shows: ShowsStrength) -> (f64, f64) {
        let w = self.strength_share;
        match shows {
            ShowsStrength::Opacity => (1.0, 0.15 + 0.85 * w),
            ShowsStrength::Length => (0.25 + 0.75 * w, 1.0),
            ShowsStrength::None => (1.0, 1.0),
        }
    }
}

/// Where a field source stands in the world at a scene time, with how far its body reaches.
#[derive(Clone, Copy, Debug)]
enum Placed {
    Charge {
        at: DVec3,
        charge: f64,
        radius: f64,
    },
    Wire {
        through: DVec3,
        along: DVec3,
        current: f64,
        radius: f64,
    },
    Loop {
        center: DVec3,
        axis: DVec3,
        x_axis: DVec3,
        radius: f64,
        current: f64,
    },
    Magnet {
        center: DVec3,
        axis: DVec3,
        y_axis: DVec3,
        half_length: f64,
        radius: f64,
        strength: f64,
    },
}

/// The world position and turn of an object, and the radius of its body: half the larger side
/// of a flat shape's box with its stroke, so a circle's is its radius, or how far any other
/// shape reaches.
fn standing(object: &Object) -> (DVec3, DMat3, f64) {
    let [x, y, z] = object
        .transform
        .rotation
        .as_dvec3()
        .to_array()
        .map(f64::to_radians);
    let turn = DMat3::from_quat(DQuat::from_euler(EulerRot::XYZ, x, y, z));
    let radius = match &object.shape {
        Shape::Flat { path, stroke, .. } => {
            let b = path.bounding_box();
            [b.x0, b.x1, b.y0, b.y1]
                .into_iter()
                .map(f64::abs)
                .fold(0.0, f64::max)
                + stroke.map_or(0.0, |s| f64::from(s.width.get()) / 2.0)
        }
        shape => f64::from(shape.reach()),
    };
    let scale = f64::from(object.transform.scale.abs().max_element());
    (object.transform.position.as_dvec3(), turn, radius * scale)
}

fn place_charge(
    source: &FieldSource<Charge>,
    objects: &std::collections::BTreeMap<ObjectId, Object>,
) -> Placed {
    let (at, _, reach) = standing(&objects[&source.object]);
    Placed::Charge {
        at,
        charge: source.source.charge,
        radius: reach,
    }
}

fn place_magnetic(
    source: &FieldSource<MagneticSource>,
    objects: &std::collections::BTreeMap<ObjectId, Object>,
) -> Placed {
    let (at, turn, reach) = standing(&objects[&source.object]);
    match source.source {
        MagneticSource::Wire { current, direction } => Placed::Wire {
            through: at,
            along: (turn * direction).normalize(),
            current,
            radius: reach,
        },
        MagneticSource::Loop { radius, current } => Placed::Loop {
            center: at,
            axis: turn.z_axis,
            x_axis: turn.x_axis,
            radius,
            current,
        },
        MagneticSource::Magnet {
            length,
            radius,
            strength,
        } => Placed::Magnet {
            center: at,
            axis: turn.x_axis,
            y_axis: turn.y_axis,
            half_length: length / 2.0,
            radius,
            strength,
        },
    }
}

impl Placed {
    /// This source's field at the world point `p`.
    fn field(&self, p: DVec3) -> DVec3 {
        match *self {
            Placed::Charge { at, charge, .. } => {
                let r = p - at;
                let d2 = r.length_squared();
                if d2 == 0.0 {
                    return DVec3::ZERO;
                }
                r * (charge / (d2 * d2.sqrt()))
            }
            Placed::Wire {
                through,
                along,
                current,
                ..
            } => {
                let d = p - through;
                let rho = d - along * d.dot(along);
                let r2 = rho.length_squared();
                if r2 == 0.0 {
                    return DVec3::ZERO;
                }
                along.cross(rho) * (2.0 * current / r2)
            }
            Placed::Loop {
                center,
                axis,
                radius,
                current,
                ..
            } => axial(p - center, axis, |rho, z| {
                loop_field(radius, current, rho, z)
            }),
            Placed::Magnet {
                center,
                axis,
                half_length,
                radius,
                strength,
                ..
            } => axial(p - center, axis, |rho, z| {
                solenoid_field(radius, half_length, strength, rho, z)
            }),
        }
    }

    /// Whether the world point `p` lies inside the source's own body, where no arrow is drawn:
    /// within a charge's or a wire's object, or inside a magnet.
    fn holds(&self, p: DVec3) -> bool {
        match *self {
            Placed::Charge { at, radius, .. } => p.distance(at) < radius,
            Placed::Wire {
                through,
                along,
                radius,
                ..
            } => {
                let d = p - through;
                (d - along * d.dot(along)).length() < radius
            }
            Placed::Loop { .. } => false,
            Placed::Magnet {
                center,
                axis,
                half_length,
                radius,
                ..
            } => {
                let d = p - center;
                let z = d.dot(axis);
                z.abs() < half_length && (d - axis * z).length() < radius
            }
        }
    }
}

/// A field symmetric about `axis`, from its parts `(along ρ, along axis)` at distance ρ from the
/// axis and height z along it.
fn axial(d: DVec3, axis: DVec3, parts: impl Fn(f64, f64) -> (f64, f64)) -> DVec3 {
    let z = d.dot(axis);
    let rho_vector = d - axis * z;
    let rho = rho_vector.length();
    let (b_rho, b_z) = parts(rho, z);
    let out = if rho > 0.0 {
        rho_vector / rho
    } else {
        DVec3::ZERO
    };
    let field = out * b_rho + axis * b_z;
    if field.is_finite() {
        field
    } else {
        DVec3::ZERO
    }
}

/// Bulirsch's generalized complete elliptic integral, the integral over φ from 0 to π/2 of
/// (c cos²φ + s sin²φ) / ((cos²φ + p sin²φ) √(cos²φ + kc² sin²φ)), as Derby and Olbert give it.
pub fn cel(kc: f64, p: f64, c: f64, s: f64) -> f64 {
    if kc == 0.0 {
        return f64::NAN;
    }
    let tolerance = 1e-14;
    let mut k = kc.abs();
    let (mut pp, mut cc, mut ss) = (p, c, s);
    let mut em = 1.0;
    if p > 0.0 {
        pp = p.sqrt();
        ss = s / pp;
    } else {
        let mut f = kc * kc;
        let mut q = 1.0 - f;
        let g = 1.0 - pp;
        f -= pp;
        q *= ss - c * pp;
        pp = (f / g).sqrt();
        cc = (c - ss) / g;
        ss = -q / (g * g * pp) + cc * pp;
    }
    let mut f = cc;
    cc += ss / pp;
    let mut g = k / pp;
    ss = 2.0 * (ss + f * g);
    pp += g;
    g = em;
    em += k;
    let mut kk = k;
    for _ in 0..64 {
        if (g - k).abs() <= g * tolerance {
            break;
        }
        k = 2.0 * kk.sqrt();
        kk = k * em;
        f = cc;
        cc += ss / pp;
        g = kk / pp;
        ss = 2.0 * (ss + f * g);
        pp += g;
        g = em;
        em += k;
    }
    (PI / 2.0) * (ss + cc * em) / (em * (em + pp))
}

/// The field of a circular loop of `radius` a and `current` I at distance ρ from its axis and
/// height z above its plane: (B along ρ, B along the axis), from the complete elliptic integrals
/// K and E of k² = 1 − near²/far², where near² = (ρ − a)² + z² and far² = (ρ + a)² + z² are the
/// squared distances to the loop's nearest and farthest points.
fn loop_field(a: f64, current: f64, rho: f64, z: f64) -> (f64, f64) {
    let r2 = rho * rho + z * z;
    let near2 = a * a + r2 - 2.0 * a * rho;
    let far2 = a * a + r2 + 2.0 * a * rho;
    if near2 <= 0.0 {
        return (0.0, 0.0);
    }
    let far = far2.sqrt();
    let kc = (near2 / far2).sqrt();
    let k_integral = cel(kc, 1.0, 1.0, 1.0);
    let e_integral = cel(kc, 1.0, 1.0, kc * kc);
    // μ0 I / π with μ0 = 4π.
    let c = 4.0 * current;
    let b_z = c / (2.0 * near2 * far) * ((a * a - r2) * e_integral + near2 * k_integral);
    let b_rho = if rho > 1e-9 * a {
        c * z / (2.0 * near2 * far * rho) * ((a * a + r2) * e_integral - near2 * k_integral)
    } else {
        0.0
    };
    (b_rho, b_z)
}

/// The field of an ideal solenoid of `radius` a and half length b carrying `strength` n·I of
/// current per unit length, at distance ρ from its axis and height z from its middle, by Derby and
/// Olbert's formula.
fn solenoid_field(a: f64, b: f64, strength: f64, rho: f64, z: f64) -> (f64, f64) {
    // μ0 n I / π with μ0 = 4π.
    let b0 = 4.0 * strength;
    let gamma = (a - rho) / (a + rho);
    let mut b_rho = 0.0;
    let mut b_z = 0.0;
    for (sign, zz) in [(1.0, z + b), (-1.0, z - b)] {
        let across = (zz * zz + (rho + a) * (rho + a)).sqrt();
        let rim = a / across;
        let end = zz / across;
        let k = ((zz * zz + (a - rho) * (a - rho)) / (zz * zz + (a + rho) * (a + rho))).sqrt();
        b_rho += sign * rim * cel(k, 1.0, 1.0, -1.0);
        b_z += sign * end * cel(k, gamma * gamma, 1.0, gamma);
    }
    (b0 * b_rho, b0 * a / (a + rho) * b_z)
}

/// The vector field's own xy plane in the world, at a scene time.
#[derive(Clone, Copy)]
struct Plane {
    origin: DVec3,
    x: DVec3,
    y: DVec3,
    normal: DVec3,
}

impl Plane {
    fn of(transform: &Transform) -> Plane {
        let [x, y, z] = transform
            .rotation
            .as_dvec3()
            .to_array()
            .map(f64::to_radians);
        let turn = DMat3::from_quat(DQuat::from_euler(EulerRot::XYZ, x, y, z));
        Plane {
            origin: transform.position.as_dvec3(),
            x: turn.x_axis,
            y: turn.y_axis,
            normal: turn.z_axis,
        }
    }

    fn world(&self, p: DVec2) -> DVec3 {
        self.origin + self.x * p.x + self.y * p.y
    }

    fn world_3d(&self, p: DVec3) -> DVec3 {
        self.world(p.truncate()) + self.normal * p.z
    }

    fn own(&self, p: DVec3) -> DVec2 {
        let d = p - self.origin;
        DVec2::new(d.dot(self.x), d.dot(self.y))
    }

    fn own_direction(&self, v: DVec3) -> DVec2 {
        DVec2::new(v.dot(self.x), v.dot(self.y))
    }

    fn own_3d(&self, v: DVec3) -> DVec3 {
        DVec3::new(v.dot(self.x), v.dot(self.y), v.dot(self.normal))
    }
}

/// Every field source of `sources` where `objects` stand.
fn placed(
    sources: &Sources,
    objects: &std::collections::BTreeMap<ObjectId, Object>,
) -> Vec<Placed> {
    match sources {
        Sources::Electric(charges) => charges.iter().map(|s| place_charge(s, objects)).collect(),
        Sources::Magnetic(magnetic) => magnetic
            .iter()
            .map(|s| place_magnetic(s, objects))
            .collect(),
    }
}

fn field_at(placed: &[Placed], p: DVec3) -> DVec3 {
    placed.iter().map(|s| s.field(p)).sum()
}

/// A field's arrows grow and fade over this range of field strengths about their median, a
/// hundredfold: one tenth of the median is the weakest and ten times it the strongest.
const STRENGTH_DECADES: f64 = 2.0;

impl VectorField {
    /// What the field draws with its field sources' objects as `objects` stand and its own
    /// `transform`.
    pub fn drawing_at(
        &self,
        transform: &Transform,
        objects: &std::collections::BTreeMap<ObjectId, Object>,
    ) -> Drawing {
        let plane = Plane::of(transform);
        let placed = placed(&self.sources, objects);
        match &self.draw {
            Draw::Arrows {
                counts, spacing, ..
            } => Drawing {
                arrows: arrows(&placed, &plane, *counts, *spacing),
                ..Drawing::default()
            },
            Draw::Lines {
                width,
                height,
                starts,
                ..
            } => {
                let tracer = Tracer {
                    placed: &placed,
                    plane,
                    half: DVec2::new(width / 2.0, height / 2.0),
                    step: width.max(*height) / 500.0,
                };
                let sources = self.sources.objects();
                let lines = match (starts, &self.sources) {
                    (Starts::Seeds(seeds), _) => seeds
                        .iter()
                        .filter_map(|seed| tracer.both_ways(*seed, &sources))
                        .collect(),
                    (Starts::Automatic(per_charge), Sources::Electric(_)) => {
                        tracer.electric(*per_charge, &sources)
                    }
                    (Starts::Automatic(count), Sources::Magnetic(_)) => {
                        tracer.magnetic(count.round().max(0.0) as usize, &sources)
                    }
                };
                let at_seeds = match starts {
                    Starts::Seeds(seeds) => seeds
                        .iter()
                        .map(|seed| field_at(&placed, plane.world(*seed)))
                        .collect(),
                    Starts::Automatic(_) => Vec::new(),
                };
                Drawing {
                    lines,
                    arrows: Vec::new(),
                    at_seeds,
                }
            }
        }
    }

    /// How far its drawing reaches from its position, in its own space.
    pub fn reach(&self) -> f32 {
        let lines = self
            .drawing
            .lines
            .iter()
            .flat_map(|line| &line.points)
            .map(|p| p.length());
        let arrows = self.drawing.arrows.iter().map(|arrow| arrow.place.length());
        let (length, tip) = match self.draw {
            Draw::Lines { tip_length, .. } => (0.0, tip_length),
            Draw::Arrows {
                length, tip_length, ..
            } => (length / 2.0, tip_length),
        };
        (lines.chain(arrows).fold(0.0, f64::max) + length + tip) as f32
            + self.stroke.width.get() / 2.0
    }
}

/// An arrow at every place of the grid not inside a field source's body, turned along the field
/// there: in the vector field's own plane for a flat grid, in space for one of more than one
/// layer.
fn arrows(placed: &[Placed], plane: &Plane, counts: [u32; 3], spacing: f64) -> Vec<FieldArrow> {
    let flat = counts[2] == 1;
    let mut drawn = Vec::new();
    for k in 0..counts[2] {
        for j in 0..counts[1] {
            for i in 0..counts[0] {
                let place = DVec3::new(
                    (f64::from(i) - f64::from(counts[0] - 1) / 2.0) * spacing,
                    (f64::from(j) - f64::from(counts[1] - 1) / 2.0) * spacing,
                    (f64::from(k) - f64::from(counts[2] - 1) / 2.0) * spacing,
                );
                let world = plane.world_3d(place);
                if placed.iter().any(|s| s.holds(world)) {
                    continue;
                }
                let own = plane.own_3d(field_at(placed, world));
                let own = if flat {
                    own.truncate().extend(0.0)
                } else {
                    own
                };
                let strength = own.length();
                if !(strength > 0.0 && strength.is_finite()) {
                    continue;
                }
                drawn.push(FieldArrow {
                    place,
                    toward: own / strength,
                    strength,
                    strength_share: 0.0,
                });
            }
        }
    }
    let mut strengths: Vec<f64> = drawn.iter().map(|a| a.strength).collect();
    strengths.sort_by(f64::total_cmp);
    if let Some(&median) = strengths.get(strengths.len() / 2) {
        for arrow in &mut drawn {
            arrow.strength_share = strength_share(arrow.strength, median);
        }
    }
    drawn
}

/// How strong an arrow shows, from 0 to 1: ½ + log₁₀(strength / median) / `STRENGTH_DECADES`,
/// clamped, so the median arrow is halfway and a field a hundred times stronger at one end of the
/// grid than the other still shows every arrow.
pub fn strength_share(strength: f64, median: f64) -> f64 {
    (0.5 + (strength / median).log10() / STRENGTH_DECADES).clamp(0.0, 1.0)
}

/// Traces field lines in the vector field's own plane, by fourth-order Runge-Kutta steps of a
/// fixed length along the unit direction of the field's part in that plane.
struct Tracer<'a> {
    placed: &'a [Placed],
    plane: Plane,
    /// Half the rectangle's width and height.
    half: DVec2,
    step: f64,
}

/// How a traced line stopped.
enum Stop {
    /// It reached the rectangle's edge, at its last point.
    Edge,
    /// It reached the body of the charge at this index.
    Charge(usize),
    /// It came back to where it started.
    Closed,
    /// It ran its longest length, or reached a point with no field in the plane.
    Spent,
}

impl Tracer<'_> {
    fn direction(&self, p: DVec2, sign: f64) -> Option<DVec2> {
        let f = self
            .plane
            .own_direction(field_at(self.placed, self.plane.world(p)));
        let length = f.length();
        (length > 0.0 && length.is_finite()).then(|| f * (sign / length))
    }

    fn rk4(&self, p: DVec2, sign: f64, h: f64) -> Option<DVec2> {
        let k1 = self.direction(p, sign)?;
        let k2 = self.direction(p + k1 * (h / 2.0), sign)?;
        let k3 = self.direction(p + k2 * (h / 2.0), sign)?;
        let k4 = self.direction(p + k3 * h, sign)?;
        Some(p + (k1 + k2 * 2.0 + k3 * 2.0 + k4) * (h / 6.0))
    }

    /// The points one step of length `h` from `p` passes through: the step's end, or, where the
    /// field turns too sharply for one step to follow it to a millionth of the rectangle, as at a
    /// magnet's rim, the ends of each half step, halved again as often as needed, down to a 64th.
    fn advance(&self, p: DVec2, sign: f64, h: f64, into: &mut Vec<DVec2>) -> Option<()> {
        let whole = self.rk4(p, sign, h)?;
        let middle = self.rk4(p, sign, h / 2.0)?;
        let halves = self.rk4(middle, sign, h / 2.0)?;
        if whole.distance(halves) <= 1e-6 * self.half.max_element() || h <= self.step / 64.0 {
            into.push(halves);
            return Some(());
        }
        self.advance(p, sign, h / 2.0, into)?;
        let at = *into.last().expect("pushed above");
        self.advance(at, sign, h / 2.0, into)
    }

    fn inside(&self, p: DVec2) -> bool {
        p.x.abs() <= self.half.x && p.y.abs() <= self.half.y
    }

    /// The charge at index `i`'s center in the plane and its body's radius there, or `None` for a
    /// source that is not a charge.
    fn charge(&self, i: usize) -> Option<(DVec2, f64, f64)> {
        match self.placed[i] {
            Placed::Charge { at, charge, radius } => {
                Some((self.plane.own(at), charge, radius.max(2.0 * self.step)))
            }
            _ => None,
        }
    }

    /// The longest a line runs: three times round the rectangle.
    fn longest(&self) -> usize {
        (12.0 * (self.half.x + self.half.y) / self.step).ceil() as usize
    }

    /// The line from `start` with the field (`sign` 1) or against it (-1), until it leaves the
    /// rectangle, reaches a charge other than `from`, comes back to its start, or is spent. It
    /// comes back when, having gone more than three steps away, it crosses the line through its
    /// start square to its first direction, the way it first left, within two steps of the start.
    fn trace(
        &self,
        start: DVec2,
        sign: f64,
        from: Option<usize>,
        closes: bool,
    ) -> (Vec<DVec2>, Stop) {
        let mut points = vec![start];
        let Some(first) = self.direction(start, sign) else {
            return (points, Stop::Spent);
        };
        let mut went_away = false;
        let mut stepped = Vec::new();
        for _ in 0..self.longest() {
            let p = points[points.len() - 1];
            stepped.clear();
            if self.advance(p, sign, self.step, &mut stepped).is_none() {
                return (points, Stop::Spent);
            }
            for &next in &stepped {
                let p = points[points.len() - 1];
                if !self.inside(next) {
                    points.push(edge_point(p, next, self.half));
                    return (points, Stop::Edge);
                }
                for i in 0..self.placed.len() {
                    if Some(i) == from {
                        continue;
                    }
                    if let Some((center, _, radius)) = self.charge(i)
                        && next.distance(center) < radius
                    {
                        let toward = (next - center).normalize_or_zero();
                        points.push(center + toward * radius);
                        return (points, Stop::Charge(i));
                    }
                }
                went_away |= next.distance(start) > 3.0 * self.step;
                let (before, after) = ((p - start).dot(first), (next - start).dot(first));
                if closes && went_away && before < 0.0 && after >= 0.0 {
                    let crossing = p + (next - p) * (-before / (after - before));
                    if crossing.distance(start) < 2.0 * self.step {
                        points.push(start);
                        return (points, Stop::Closed);
                    }
                }
                points.push(next);
            }
        }
        (points, Stop::Spent)
    }

    /// The line through `seed`, traced both ways, its points in the field's direction: closed if
    /// it comes back to the seed, or else from wherever its backward half stopped to wherever its
    /// forward half did. `None` where the plane has no field.
    fn both_ways(&self, seed: DVec2, sources: &[&ObjectId]) -> Option<FieldLine> {
        self.direction(seed, 1.0)?;
        let (forward, stop) = self.trace(seed, 1.0, None, true);
        if matches!(stop, Stop::Closed) {
            return Some(FieldLine {
                points: forward,
                closed: true,
                ends_on: None,
                tip: 0.0,
            });
        }
        let (mut backward, _) = self.trace(seed, -1.0, None, false);
        let tip = backward.windows(2).map(|p| p[0].distance(p[1])).sum();
        backward.reverse();
        backward.pop();
        backward.extend(forward);
        let ends_on = match stop {
            Stop::Charge(i) => Some(sources[i].clone()),
            _ => None,
        };
        (backward.len() > 1).then_some(FieldLine {
            points: backward,
            closed: false,
            ends_on,
            tip,
        })
    }

    /// Electric field lines: `per_charge` lines a unit of charge evenly around each positive
    /// charge, traced with the field; then, for each negative charge that fewer lines reach than
    /// its own count, lines traced against the field from its even angles farthest from those
    /// that reach it.
    fn electric(&self, per_charge: f64, sources: &[&ObjectId]) -> Vec<FieldLine> {
        let mut lines = Vec::new();
        let count = |charge: f64| (per_charge * charge.abs()).round() as usize;
        let mut arrivals: Vec<Vec<f64>> = vec![Vec::new(); self.placed.len()];
        for i in 0..self.placed.len() {
            let Some((center, charge, radius)) = self.charge(i) else {
                continue;
            };
            let n = count(charge);
            if charge <= 0.0 || n == 0 {
                continue;
            }
            for k in 0..n {
                let angle = 2.0 * PI * k as f64 / n as f64;
                let start = center + DVec2::from_angle(angle) * radius;
                if !self.inside(start) {
                    continue;
                }
                let (points, stop) = self.trace(start, 1.0, Some(i), false);
                let ends_on = match stop {
                    Stop::Charge(j) => {
                        let (to, ..) = self.charge(j).expect("a line stops on a charge");
                        let end = points[points.len() - 1] - to;
                        arrivals[j].push(end.y.atan2(end.x));
                        Some(sources[j].clone())
                    }
                    _ => None,
                };
                if points.len() > 1 {
                    lines.push(halfway(FieldLine {
                        points,
                        closed: false,
                        ends_on,
                        tip: 0.0,
                    }));
                }
            }
        }
        for (i, reached) in arrivals.iter().enumerate() {
            let Some((center, charge, radius)) = self.charge(i) else {
                continue;
            };
            let n = count(charge);
            if charge >= 0.0 || reached.len() >= n {
                continue;
            }
            for angle in free_angles(reached, n - reached.len(), n) {
                let start = center + DVec2::from_angle(angle) * radius;
                if !self.inside(start) {
                    continue;
                }
                let (mut points, _) = self.trace(start, -1.0, Some(i), false);
                points.reverse();
                if points.len() > 1 {
                    lines.push(halfway(FieldLine {
                        points,
                        closed: false,
                        ends_on: Some(sources[i].clone()),
                        tip: 0.0,
                    }));
                }
            }
        }
        lines
    }

    /// Magnetic field lines: `count` lines on each side of a loop or a magnet, and `count` out
    /// from a wire, each traced both ways from its seed.
    fn magnetic(&self, count: usize, sources: &[&ObjectId]) -> Vec<FieldLine> {
        let mut seeds = Vec::new();
        for placed in self.placed {
            match *placed {
                Placed::Wire {
                    through,
                    along,
                    radius,
                    ..
                } => {
                    let across = along.dot(self.plane.normal);
                    if across.abs() < 1e-6 {
                        continue;
                    }
                    // Where the wire crosses the plane.
                    let t = (self.plane.origin - through).dot(self.plane.normal) / across;
                    let at = self.plane.own(through + along * t);
                    let room = [
                        self.half.x - at.x,
                        self.half.x + at.x,
                        self.half.y - at.y,
                        self.half.y + at.y,
                    ]
                    .into_iter()
                    .fold(f64::INFINITY, f64::min);
                    let room = if room > 0.0 {
                        room
                    } else {
                        self.half.min_element()
                    };
                    for k in 1..=count {
                        let out = room * k as f64 / (count + 1) as f64;
                        if out > radius {
                            seeds.push(at + DVec2::X * out);
                        }
                    }
                }
                Placed::Loop {
                    center,
                    axis,
                    x_axis,
                    radius,
                    ..
                } => {
                    seeds.extend(self.outward(center, axis, x_axis, radius, count));
                }
                Placed::Magnet {
                    center,
                    axis,
                    y_axis,
                    radius,
                    ..
                } => {
                    seeds.extend(self.outward(center, axis, y_axis, radius, count));
                }
                Placed::Charge { .. } => unreachable!("a magnetic field has no charges"),
            }
        }
        seeds
            .into_iter()
            .filter(|seed| self.inside(*seed))
            .filter_map(|seed| self.both_ways(seed, sources))
            .collect()
    }

    /// `count` points on each side of the disc of `radius` about `center`, square to `axis`,
    /// along the line where its plane meets the vector field's, or along `fallback` when the two
    /// planes are parallel: at even steps from its rim out toward the rectangle's nearer edge along
    /// that line, each the middle of its share. Every field line of a loop or a magnet passes
    /// through it and crosses this line outside it, so a line seeded farther out closes wider.
    fn outward(
        &self,
        center: DVec3,
        axis: DVec3,
        fallback: DVec3,
        radius: f64,
        count: usize,
    ) -> Vec<DVec2> {
        let meet = axis.cross(self.plane.normal);
        let along = if meet.length() > 1e-6 { meet } else { fallback };
        let along = self.plane.own_direction(along).normalize_or_zero();
        let middle = self.plane.own(center);
        let room = [along, -along]
            .into_iter()
            .map(|way| {
                [0, 1]
                    .into_iter()
                    .filter(|&a| way[a] != 0.0)
                    .map(|a| (self.half[a] * way[a].signum() - middle[a]) / way[a])
                    .fold(f64::INFINITY, f64::min)
            })
            .fold(f64::INFINITY, f64::min);
        let room = if room > radius { room } else { 2.0 * radius };
        (1..=count)
            .flat_map(|k| {
                let out = radius + (room - radius) * (k as f64 - 0.5) / count as f64;
                [middle + along * out, middle - along * out]
            })
            .collect()
    }
}

/// The line with its tip halfway along it.
fn halfway(line: FieldLine) -> FieldLine {
    FieldLine {
        tip: line.length() / 2.0,
        ..line
    }
}

/// The point where the step from `inside` to `outside` leaves the rectangle of half sizes `half`.
fn edge_point(inside: DVec2, outside: DVec2, half: DVec2) -> DVec2 {
    let d = outside - inside;
    let mut t: f64 = 1.0;
    for axis in 0..2 {
        if d[axis] != 0.0 {
            let wall = half[axis] * d[axis].signum();
            t = t.min((wall - inside[axis]) / d[axis]);
        }
    }
    inside + d * t.clamp(0.0, 1.0)
}

/// `n` of the `total` even angles from 0 around a negative charge, for the lines it lacks: those
/// farthest from every angle a line `reached` it at, so a scene symmetric about the charge keeps
/// its lines symmetric.
fn free_angles(reached: &[f64], n: usize, total: usize) -> Vec<f64> {
    let even: Vec<f64> = (0..total)
        .map(|k| 2.0 * PI * k as f64 / total as f64)
        .collect();
    let clear = |angle: f64| {
        reached
            .iter()
            .map(|a| {
                let apart = (angle - a).rem_euclid(2.0 * PI);
                apart.min(2.0 * PI - apart)
            })
            .fold(PI, f64::min)
    };
    let mut order: Vec<usize> = (0..total).collect();
    // Clearances within a millionth of a radian tie, and a tie keeps the even angles' order.
    order.sort_by(|&a, &b| {
        let (ca, cb) = (
            (clear(even[a]) * 1e6).round(),
            (clear(even[b]) * 1e6).round(),
        );
        cb.total_cmp(&ca).then(a.cmp(&b))
    });
    order.into_iter().take(n).map(|k| even[k]).collect()
}

impl VectorField {
    /// Its field lines, or the shafts of its arrows in a grid one place deep, as one path in its
    /// own plane, for finding the text they run through. `None` for arrows in space.
    pub fn stroke_path(&self) -> Option<crate::path::Path> {
        let mut path = kurbo::BezPath::new();
        let at = |p: DVec2| kurbo::Point::new(p.x, p.y);
        match self.draw {
            Draw::Lines { .. } => {
                for line in &self.drawing.lines {
                    path.move_to(at(line.points[0]));
                    for p in &line.points[1..] {
                        path.line_to(at(*p));
                    }
                }
            }
            Draw::Arrows {
                counts,
                length,
                shows_strength,
                ..
            } if counts[2] == 1 => {
                for arrow in &self.drawing.arrows {
                    let half =
                        arrow.toward.truncate() * (length * arrow.look(shows_strength).0 / 2.0);
                    let place = arrow.place.truncate();
                    path.move_to(at(place - half));
                    path.line_to(at(place + half));
                }
            }
            Draw::Arrows { .. } => return None,
        }
        crate::path::Path::from_bez(path).ok()
    }
}

/// What `inspect` reports of a vector field.
#[derive(Debug, Serialize)]
pub struct VectorFieldReport {
    pub of: FieldOf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines: Option<Vec<LineReport>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrow_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrows: Option<Vec<ArrowReport>>,
    /// The field in the world at each of `seeds`, in order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_seeds: Option<Vec<[f64; 3]>>,
    /// The text and math objects its lines or arrows run through on the frame, which
    /// `check-layout` reports, as it does a ray's.
    pub crosses_objects: Vec<String>,
}

/// One field line's ends in the world, and whether it closes.
#[derive(Debug, Serialize)]
pub struct LineReport {
    pub from: [f64; 3],
    pub to: [f64; 3],
    pub closed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ends_on: Option<String>,
}

/// One arrow: its place and direction in the world, and the field's strength there.
#[derive(Debug, Serialize)]
pub struct ArrowReport {
    pub place: [f64; 3],
    pub toward: [f64; 3],
    pub strength: f64,
}

impl VectorField {
    pub fn report(&self, transform: &Transform, crosses_objects: Vec<String>) -> VectorFieldReport {
        let plane = Plane::of(transform);
        let drawing = &self.drawing;
        let (lines, arrows) = match self.draw {
            Draw::Lines { .. } => (
                Some(
                    drawing
                        .lines
                        .iter()
                        .map(|line| LineReport {
                            from: plane.world(line.points[0]).to_array(),
                            to: plane.world(line.points[line.points.len() - 1]).to_array(),
                            closed: line.closed,
                            ends_on: line.ends_on.as_ref().map(ToString::to_string),
                        })
                        .collect::<Vec<_>>(),
                ),
                None,
            ),
            Draw::Arrows { .. } => (
                None,
                Some(
                    drawing
                        .arrows
                        .iter()
                        .map(|arrow| ArrowReport {
                            place: plane.world_3d(arrow.place).to_array(),
                            toward: (plane.world_3d(arrow.toward) - plane.origin).to_array(),
                            strength: arrow.strength,
                        })
                        .collect::<Vec<_>>(),
                ),
            ),
        };
        let at_seeds = match &self.draw {
            Draw::Lines {
                starts: Starts::Seeds(_),
                ..
            } => Some(drawing.at_seeds.iter().map(|f| f.to_array()).collect()),
            _ => None,
        };
        VectorFieldReport {
            of: self.sources.of(),
            line_count: lines.as_ref().map(Vec::len),
            lines,
            arrow_count: arrows.as_ref().map(Vec::len),
            arrows,
            at_seeds,
            crosses_objects,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The integral `cel` stands for, by Simpson's rule on 20,000 pieces.
    fn cel_by_sum(kc: f64, p: f64, c: f64, s: f64) -> f64 {
        let n = 20_000;
        let h = (PI / 2.0) / n as f64;
        let f = |phi: f64| {
            let (cos2, sin2) = (phi.cos().powi(2), phi.sin().powi(2));
            (c * cos2 + s * sin2) / ((cos2 + p * sin2) * (cos2 + kc * kc * sin2).sqrt())
        };
        (0..=n)
            .map(|i| {
                let weight = if i == 0 || i == n {
                    1.0
                } else if i % 2 == 1 {
                    4.0
                } else {
                    2.0
                };
                weight * f(i as f64 * h)
            })
            .sum::<f64>()
            * h
            / 3.0
    }

    #[test]
    fn bulirsch_s_integral_is_the_integral_it_stands_for() {
        for (kc, p, c, s) in [
            (0.3, 1.0, 1.0, 1.0),
            (0.3, 1.0, 1.0, 0.09),
            (0.8, 1.0, 1.0, -1.0),
            (0.5, 0.25, 1.0, 0.5),
            (0.9, 0.04, 1.0, 0.2),
        ] {
            let exact = cel(kc, p, c, s);
            let summed = cel_by_sum(kc, p, c, s);
            assert!(
                (exact - summed).abs() < 1e-9 * summed.abs().max(1.0),
                "{kc} {p} {c} {s}: {exact} {summed}"
            );
        }
        // K(0) = E(0) = π/2.
        assert!((cel(1.0, 1.0, 1.0, 1.0) - PI / 2.0).abs() < 1e-14);
    }

    /// The Biot-Savart sum, with μ0/4π = 1, over a loop of `radius` and `current` in the xy plane
    /// cut into `pieces` straight pieces.
    fn biot_savart(radius: f64, current: f64, p: DVec3, pieces: usize) -> DVec3 {
        (0..pieces)
            .map(|i| {
                let (a, b) = (
                    2.0 * PI * i as f64 / pieces as f64,
                    2.0 * PI * (i + 1) as f64 / pieces as f64,
                );
                let from = DVec3::new(a.cos(), a.sin(), 0.0) * radius;
                let to = DVec3::new(b.cos(), b.sin(), 0.0) * radius;
                let r = p - (from + to) / 2.0;
                (to - from).cross(r) * (current / r.length().powi(3))
            })
            .sum()
    }

    fn a_loop(radius: f64, current: f64) -> Placed {
        Placed::Loop {
            center: DVec3::ZERO,
            axis: DVec3::Z,
            x_axis: DVec3::X,
            radius,
            current,
        }
    }

    #[test]
    fn a_loop_off_its_axis_gives_the_field_of_the_biot_savart_sum() {
        let ring = a_loop(1.3, 2.0);
        for p in [
            DVec3::new(0.4, 0.2, 0.5),
            DVec3::new(2.0, -1.0, 0.3),
            DVec3::new(0.0, 1.2, -0.8),
            DVec3::new(3.0, 3.0, 2.0),
        ] {
            let exact = ring.field(p);
            let summed = biot_savart(1.3, 2.0, p, 200_000);
            assert!(
                (exact - summed).length() < 1e-6 * summed.length(),
                "{p}: {exact} {summed}"
            );
        }
    }

    #[test]
    fn a_magnet_is_the_field_of_a_stack_of_loops() {
        let (length, radius, strength) = (2.0, 0.3, 1.5);
        let magnet = Placed::Magnet {
            center: DVec3::ZERO,
            axis: DVec3::X,
            y_axis: DVec3::Y,
            half_length: length / 2.0,
            radius,
            strength,
        };
        let loops = 4000;
        let stack = |p: DVec3| -> DVec3 {
            (0..loops)
                .map(|i| {
                    let x = -length / 2.0 + length * (i as f64 + 0.5) / loops as f64;
                    let turn = Placed::Loop {
                        center: DVec3::new(x, 0.0, 0.0),
                        axis: DVec3::X,
                        x_axis: DVec3::Y,
                        radius,
                        current: strength * length / loops as f64,
                    };
                    turn.field(p)
                })
                .sum()
        };
        for p in [
            DVec3::new(0.0, 0.0, 0.0),
            DVec3::new(0.3, 0.1, 0.0),
            DVec3::new(1.5, 0.5, 0.0),
            DVec3::new(-0.4, 0.9, 0.2),
        ] {
            let exact = magnet.field(p);
            let summed = stack(p);
            assert!(
                (exact - summed).length() < 1e-4 * summed.length(),
                "{p}: {exact} {summed}"
            );
        }
        // Its middle: μ0 n I b / √(b² + a²), with μ0 = 4π.
        let middle = magnet.field(DVec3::ZERO).x;
        let expected = 4.0 * PI * strength * 1.0 / (1.0f64 + 0.09).sqrt();
        assert!(
            (middle - expected).abs() < 1e-9 * expected,
            "{middle} {expected}"
        );
    }

    #[test]
    fn a_line_leaving_the_rectangle_stops_on_its_edge() {
        let half = DVec2::new(2.0, 1.0);
        let edge = edge_point(DVec2::new(1.9, 0.0), DVec2::new(2.1, 0.5), half);
        assert!((edge - DVec2::new(2.0, 0.25)).length() < 1e-12, "{edge}");
    }

    #[test]
    fn a_negative_charge_starts_its_missing_lines_farthest_from_those_that_reach_it() {
        // Lines reach it from the left, around π, as in a dipole.
        let reached = [PI - 0.4, PI, PI + 0.4];
        let mut angles: Vec<f64> = free_angles(&reached, 5, 12)
            .into_iter()
            .map(|a| (a * 6.0 / PI).round())
            .collect();
        angles.sort_by(f64::total_cmp);
        // In twelfths of a turn: 0, ±1 and ±2, symmetric about the axis.
        assert_eq!(angles, [0.0, 1.0, 2.0, 10.0, 11.0]);
    }
}
