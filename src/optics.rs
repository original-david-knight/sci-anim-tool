//! Ray optics: lenses, mirrors and media, and the rays and images drawn through them.
//!
//! A lens or a curved mirror forms images by the thin-element law a ray diagram teaches: a point
//! and its image are conjugate, so a lens of focal length f at its own origin sends a point
//! (x, y) to (f·x, f·y)/(f + x), which is 1/do + 1/di = 1/f with magnification -di/do, and a
//! mirror does the same and reflects the result back across itself. Each ray carries the point
//! it came from as a point of the projective plane, so a point at infinity is a direction, and
//! leaves a lens or mirror toward that point's image: every ray from an object's tip then meets
//! at the image's tip, real or virtual. A ray bends at a lens's middle plane, as a thin lens
//! does, and reflects where it meets a mirror's drawn arc. A medium refracts it exactly, by
//! Snell's law, at every edge it crosses, and reflects it whole past the critical angle.

use std::sync::Arc;

use glam::{DVec2, DVec3, Vec2};
use serde::Serialize;

use crate::body::Collider;
use crate::ends::End;
use crate::path::Path;
use crate::scene::{Object, ObjectId};

/// What an optical element does to the light that meets it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Optic {
    /// A thin lens across its object's own y axis, `height` long: converging for a positive
    /// focal length, diverging for a negative one. Its optical axis is its object's own x axis.
    Lens { focal_length: f64, height: f64 },
    /// A mirror across its object's own y axis, its reflecting side toward its own -x: flat with
    /// no focal length, concave for a positive one and convex for a negative one, curved on a
    /// circle of twice the focal length's radius through its object's position.
    Mirror {
        focal_length: Option<f64>,
        height: f64,
    },
    /// Glass, water or another clear region, its object's collider, at a refractive index of 1
    /// or more, in air of index 1.
    Medium { refractive_index: f64 },
}

/// An optical element: an object and what it does to light.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub object: ObjectId,
    pub optic: Optic,
}

/// Which way a ray leaves its source.
#[derive(Clone, Debug, PartialEq)]
pub enum Aim {
    /// Through a point, toward the elements.
    Through(End),
    /// At an angle in degrees, counterclockwise from the world's +x.
    Angle(f64),
    /// A principal ray of the first lens or curved mirror it meets.
    Principal(Principal),
}

/// The rays a ray diagram draws from an object's tip, each of which a lens or a curved mirror
/// sends somewhere it is easy to draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Principal {
    /// Along the optical axis: it leaves through the far focal point of a converging lens or a
    /// concave mirror, or as if from the near one of a diverging lens or a convex mirror.
    Parallel,
    /// Through, or toward, the focal point on the object's side: it leaves parallel to the axis.
    Focal,
    /// Through a lens's middle, which it passes straight, or toward a mirror's center of
    /// curvature, from which it reflects back on itself.
    Center,
}

/// A shape traced through optical elements at every scene time: a ray, or the image of an arrow.
#[derive(Clone, Debug, PartialEq)]
pub enum Light {
    /// A ray from `from`, bent by every element of `elements` it meets, ending `reach` past the
    /// last. With `back_trace`, a dashed line runs back from where it leaves the last lens or
    /// mirror to the virtual image its source has there.
    Ray {
        from: End,
        aim: Aim,
        elements: Arc<[Element]>,
        reach: f64,
        back_trace: bool,
    },
    /// The image of the arrow from `from` to `to` through the lenses and mirrors of `elements`,
    /// in the order the light meets them: an arrow between the two ends' images, dashed when
    /// virtual.
    Image {
        from: End,
        to: End,
        elements: Arc<[Element]>,
        tip_length: f32,
    },
}

/// A ray or image at a scene time: its path in the world's xy plane, and what `inspect` reports.
pub struct Traced {
    pub path: Path,
    pub report: LightReport,
}

/// What `inspect` reports of a ray or an image.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case", untagged)]
pub enum LightReport {
    Ray {
        /// The world points the ray passes through: its source, each point it bends at, and its
        /// end.
        points: Vec<[f64; 2]>,
        /// Where the ray's source is imaged by the last lens or mirror it met, if anywhere
        /// finite.
        image: Option<[f64; 2]>,
        /// Whether that image lies behind the ray, so its last part only seems to come from it.
        #[serde(skip_serializing_if = "Option::is_none")]
        virtual_image: Option<bool>,
    },
    Image {
        from: [f64; 2],
        to: [f64; 2],
        /// The image's height over the object's, negative when it is upside down.
        magnification: f64,
        /// Whether light reaches the image, which a screen could show, rather than only seeming
        /// to come from it.
        real: bool,
    },
}

/// How far a ray runs, at most, through elements before it is cut, so a ray caught between two
/// mirrors ends.
const MOST_BENDS: usize = 64;

/// A ray this close past a point it bent at does not meet that point's element again.
const LEAST_STEP: f64 = 1e-7;

/// A dash and the gap after it on a back trace or a virtual image, in world units.
const DASH: (f64, f64) = (0.14, 0.1);

/// Where an element stands in the world: its position, its turn about z in radians, and its
/// collider there for a medium.
struct Placed<'a> {
    element: &'a Element,
    origin: DVec2,
    angle: f64,
    region: Option<Region>,
}

/// A medium's region in the world.
enum Region {
    Disc(DVec2, f64),
    Corners(Vec<DVec2>),
}

impl Region {
    fn contains(&self, point: DVec2) -> bool {
        match self {
            Region::Disc(center, radius) => point.distance(*center) < *radius,
            // Inside when a line from the point toward +x crosses an odd number of edges.
            Region::Corners(corners) => {
                let n = corners.len();
                (0..n)
                    .filter(|&i| {
                        let (a, b) = (corners[i], corners[(i + 1) % n]);
                        (a.y > point.y) != (b.y > point.y)
                            && point.x < a.x + (point.y - a.y) * (b.x - a.x) / (b.y - a.y)
                    })
                    .count()
                    % 2
                    == 1
            }
        }
    }
}

impl Placed<'_> {
    fn local(&self, point: DVec2) -> DVec2 {
        DVec2::from_angle(-self.angle).rotate(point - self.origin)
    }

    fn local_direction(&self, direction: DVec2) -> DVec2 {
        DVec2::from_angle(-self.angle).rotate(direction)
    }

    fn world(&self, point: DVec2) -> DVec2 {
        self.origin + DVec2::from_angle(self.angle).rotate(point)
    }

    fn world_direction(&self, direction: DVec2) -> DVec2 {
        DVec2::from_angle(self.angle).rotate(direction)
    }

    /// A point of the projective plane, in the element's own frame.
    fn local_projective(&self, point: DVec3) -> DVec3 {
        let turned = DVec2::from_angle(-self.angle).rotate(point.truncate());
        let shifted = turned - DVec2::from_angle(-self.angle).rotate(self.origin) * point.z;
        shifted.extend(point.z)
    }

    fn world_projective(&self, point: DVec3) -> DVec3 {
        (DVec2::from_angle(self.angle).rotate(point.truncate()) + self.origin * point.z)
            .extend(point.z)
    }
}

fn place<'a>(
    element: &'a Element,
    objects: &std::collections::BTreeMap<ObjectId, Object>,
) -> Placed<'a> {
    let object = &objects[&element.object];
    let transform = object.transform;
    let origin = object.flat_to_world().position().truncate().as_dvec2();
    let angle = f64::from(transform.rotation.z).to_radians();
    let scale = transform.scale.truncate().as_dvec2();
    let region = match (&element.optic, &object.collider) {
        (Optic::Medium { .. }, Some(Collider::Disc(radius) | Collider::Ball(radius))) => {
            Some(Region::Disc(origin, radius * scale.x.abs()))
        }
        (Optic::Medium { .. }, Some(Collider::Corners(corners))) => Some(Region::Corners(
            corners
                .iter()
                .map(|corner| origin + DVec2::from_angle(angle).rotate(*corner * scale))
                .collect(),
        )),
        _ => None,
    };
    Placed {
        element,
        origin,
        angle,
        region,
    }
}

/// How a ray meets an element: where, and what happens there.
struct Met {
    distance: f64,
    point: DVec2,
    meets: Meets,
}

enum Meets {
    /// A lens or a mirror's reflecting side.
    Front,
    /// A mirror's back, which stops the ray.
    Back,
    /// A medium's edge, with its outward normal.
    Edge(DVec2),
}

fn meet(placed: &Placed, from: DVec2, direction: DVec2) -> Option<Met> {
    let (p, d) = (placed.local(from), placed.local_direction(direction));
    let on_plane = |height: f64| -> Option<(f64, DVec2)> {
        if d.x.abs() < 1e-12 {
            return None;
        }
        let t = -p.x / d.x;
        let point = p + d * t;
        (t > LEAST_STEP && point.y.abs() <= height / 2.0).then_some((t, point))
    };
    match placed.element.optic {
        Optic::Lens { height, .. } => on_plane(height).map(|(t, point)| Met {
            distance: t,
            point: placed.world(point),
            meets: Meets::Front,
        }),
        Optic::Mirror {
            focal_length: None,
            height,
        } => on_plane(height).map(|(t, point)| Met {
            distance: t,
            point: placed.world(point),
            meets: if d.x > 0.0 { Meets::Front } else { Meets::Back },
        }),
        Optic::Mirror {
            focal_length: Some(focal_length),
            height,
        } => {
            let center = DVec2::new(-2.0 * focal_length, 0.0);
            let radius = 2.0 * focal_length.abs();
            let to = p - center;
            let (b, c) = (to.dot(d), to.length_squared() - radius * radius);
            let discriminant = b * b - c;
            if discriminant < 0.0 {
                return None;
            }
            let root = discriminant.sqrt();
            [-b - root, -b + root]
                .into_iter()
                .filter(|t| *t > LEAST_STEP)
                .map(|t| (t, p + d * t))
                .find(|(_, point)| {
                    point.y.abs() <= height / 2.0 && (*point - center).dot(-center) > 0.0
                })
                .map(|(t, point)| {
                    // The reflecting side faces -x: toward the center of a concave mirror.
                    let front = if focal_length > 0.0 {
                        (center - point) / radius
                    } else {
                        (point - center) / radius
                    };
                    Met {
                        distance: t,
                        point: placed.world(point),
                        meets: if d.dot(front) < 0.0 {
                            Meets::Front
                        } else {
                            Meets::Back
                        },
                    }
                })
        }
        Optic::Medium { .. } => meet_edge(
            placed.region.as_ref().expect("a medium has a region"),
            from,
            direction,
        ),
    }
}

/// The nearest edge of `region` the ray from `from` along `direction` crosses.
fn meet_edge(region: &Region, from: DVec2, direction: DVec2) -> Option<Met> {
    match region {
        Region::Disc(center, radius) => {
            let to = from - *center;
            let (b, c) = (to.dot(direction), to.length_squared() - radius * radius);
            let discriminant = b * b - c;
            if discriminant < 0.0 {
                return None;
            }
            let root = discriminant.sqrt();
            [-b - root, -b + root]
                .into_iter()
                .find(|t| *t > LEAST_STEP)
                .map(|t| {
                    let point = from + direction * t;
                    Met {
                        distance: t,
                        point,
                        meets: Meets::Edge((point - *center) / *radius),
                    }
                })
        }
        Region::Corners(corners) => {
            let n = corners.len();
            (0..n)
                .filter_map(|i| {
                    let (a, b) = (corners[i], corners[(i + 1) % n]);
                    let edge = b - a;
                    let across = direction.perp_dot(edge);
                    if across.abs() < 1e-12 {
                        return None;
                    }
                    let t = (a - from).perp_dot(edge) / across;
                    let share = (a - from).perp_dot(direction) / across;
                    (t > LEAST_STEP && (0.0..=1.0).contains(&share)).then(|| Met {
                        distance: t,
                        point: from + direction * t,
                        meets: Meets::Edge(DVec2::new(edge.y, -edge.x).normalize()),
                    })
                })
                .min_by(|x, y| x.distance.total_cmp(&y.distance))
        }
    }
}

/// `point` of the projective plane imaged by a thin lens of `focal_length` at the origin, its
/// axis along x.
fn through_lens(point: DVec3, focal_length: f64) -> DVec3 {
    DVec3::new(
        focal_length * point.x,
        focal_length * point.y,
        point.x + focal_length * point.z,
    )
}

/// `point` imaged by a mirror at the origin facing -x: a lens's image reflected back across it.
fn off_mirror(point: DVec3, focal_length: Option<f64>) -> DVec3 {
    let image = match focal_length {
        Some(focal_length) => through_lens(point, focal_length),
        None => point,
    };
    DVec3::new(-image.x, image.y, image.z)
}

/// The finite point a projective point stands for, if it is finite.
fn finite(point: DVec3) -> Option<DVec2> {
    (point.z.abs() > 1e-12 * point.truncate().length().max(1.0)).then(|| point.truncate() / point.z)
}

/// The direction a ray leaves `at` in, along the line toward `image` (a projective point),
/// with its x of the sign `ahead` in the element's frame.
fn toward(image: DVec3, at: DVec2, ahead: f64) -> Option<DVec2> {
    let along = match finite(image) {
        Some(point) => point - at,
        None => image.truncate(),
    };
    if along.x.abs() < 1e-12 {
        return None;
    }
    Some((along * along.x.signum() * ahead).normalize())
}

/// What a lens or mirror does to a ray: the direction it leaves `at` in, and its source's image,
/// both in the element's frame. `source` is the ray's source as a projective point, and `going`
/// the way it travels there.
fn bend(optic: Optic, source: DVec3, at: DVec2, going: DVec2) -> Option<(DVec2, DVec3)> {
    match optic {
        Optic::Lens { focal_length, .. } => {
            // A lens works the same from either side: flip x so the ray travels +x.
            let side = going.x.signum();
            let flip = |p: DVec3| DVec3::new(p.x * side, p.y, p.z);
            let image = flip(through_lens(flip(source), focal_length));
            let leaves = toward(image, at, side)?;
            Some((leaves, image))
        }
        Optic::Mirror { focal_length, .. } => {
            let image = off_mirror(source, focal_length);
            Some((toward(image, at, -1.0)?, image))
        }
        Optic::Medium { .. } => unreachable!("a medium refracts"),
    }
}

/// Snell's law at an edge of outward normal `normal`, from index `from` to `to`: the direction
/// the ray goes on in, or the reflected direction past the critical angle.
pub fn refract(direction: DVec2, normal: DVec2, from: f64, to: f64) -> (DVec2, bool) {
    let facing = if direction.dot(normal) > 0.0 {
        -normal
    } else {
        normal
    };
    let cos_in = -direction.dot(facing);
    let ratio = from / to;
    let sin_out_squared = ratio * ratio * (1.0 - cos_in * cos_in);
    if sin_out_squared > 1.0 {
        return (direction + facing * (2.0 * cos_in), true);
    }
    let cos_out = (1.0 - sin_out_squared).sqrt();
    (
        (direction * ratio + facing * (ratio * cos_in - cos_out)).normalize(),
        false,
    )
}

impl Light {
    /// The objects its ends sit on, which hide it while they are hidden.
    pub fn end_objects(&self) -> impl Iterator<Item = &ObjectId> {
        let ends: Vec<&End> = match self {
            Light::Ray { from, aim, .. } => std::iter::once(from)
                .chain(match aim {
                    Aim::Through(end) => Some(end),
                    _ => None,
                })
                .collect(),
            Light::Image { from, to, .. } => vec![from, to],
        };
        ends.into_iter().filter_map(End::object)
    }

    /// The ray or image as `objects` stand at a scene time, at the height of its source, or
    /// `None` when it draws nothing: an aim with no direction, or an image at infinity.
    pub fn trace(
        &self,
        objects: &std::collections::BTreeMap<ObjectId, Object>,
    ) -> Option<(f32, Traced)> {
        match self {
            Light::Ray {
                from,
                aim,
                elements,
                reach,
                back_trace,
            } => {
                let source = from.world(objects);
                let placed: Vec<Placed> = elements.iter().map(|e| place(e, objects)).collect();
                let start = source.truncate().as_dvec2();
                let direction = match aim {
                    Aim::Through(end) => {
                        let through = end.world(objects).truncate().as_dvec2() - start;
                        (through.length() > 1e-12).then(|| through.normalize())?
                    }
                    Aim::Angle(degrees) => DVec2::from_angle(degrees.to_radians()),
                    Aim::Principal(principal) => {
                        let reference = placed.iter().find(|p| {
                            matches!(
                                p.element.optic,
                                Optic::Lens { .. }
                                    | Optic::Mirror {
                                        focal_length: Some(_),
                                        ..
                                    }
                            )
                        })?;
                        principal_direction(reference, *principal, start)?
                    }
                };
                let path = trace_ray(&placed, start, direction, *reach, *back_trace);
                Some((source.z, path))
            }
            Light::Image {
                from,
                to,
                elements,
                tip_length,
            } => {
                let (a, b) = (from.world(objects), to.world(objects));
                let placed: Vec<Placed> = elements.iter().map(|e| place(e, objects)).collect();
                let (from_image, real) = image_of(&placed, a.truncate().as_dvec2())?;
                let (to_image, _) = image_of(&placed, b.truncate().as_dvec2())?;
                let path = arrow(from_image, to_image, f64::from(*tip_length), !real)?;
                let object = (b - a).truncate().as_dvec2();
                let magnification = (to_image - from_image).dot(object) / object.length_squared();
                Some((
                    a.z,
                    Traced {
                        path,
                        report: LightReport::Image {
                            from: from_image.to_array(),
                            to: to_image.to_array(),
                            magnification,
                            real,
                        },
                    },
                ))
            }
        }
    }
}

/// The direction of a principal ray from `start` for the element at `reference`, toward it.
fn principal_direction(reference: &Placed, principal: Principal, start: DVec2) -> Option<DVec2> {
    let source = reference.local(start);
    let side = source.x.signum();
    let (focal_length, mirror) = match reference.element.optic {
        Optic::Lens { focal_length, .. } => (focal_length, false),
        Optic::Mirror {
            focal_length: Some(focal_length),
            ..
        } => (focal_length, true),
        _ => unreachable!("the reference is a lens or a curved mirror"),
    };
    // The focal point a focal length out on the source's side, which for a diverging lens or a
    // convex mirror lies on the far side; a mirror's source is always on its -x side.
    let through = match principal {
        Principal::Parallel => return Some(reference.world_direction(DVec2::X * -side)),
        Principal::Focal => DVec2::new(side * focal_length, 0.0),
        Principal::Center if mirror => DVec2::new(side * 2.0 * focal_length, 0.0),
        Principal::Center => DVec2::ZERO,
    };
    let toward_element = -side;
    let along = through - source;
    if along.x.abs() < 1e-12 {
        return None;
    }
    Some(reference.world_direction((along * along.x.signum() * toward_element).normalize()))
}

/// Where the lenses and mirrors of `placed` image `point`, in the order listed, and whether the
/// image is real: on the side the light leaves the last of them toward.
fn image_of(placed: &[Placed], point: DVec2) -> Option<(DVec2, bool)> {
    let mut source = point.extend(1.0);
    let mut real = true;
    // The way the light travels, from the point toward the first element, then reversed by
    // each mirror: a lens's image is real on the side the light goes on toward.
    let mut going = placed.first()?.origin - point;
    for element in placed {
        let local = element.local_projective(source);
        let side = element.local_direction(going).x.signum();
        let (image, ahead) = match element.element.optic {
            Optic::Lens { focal_length, .. } => {
                let flip = |p: DVec3| DVec3::new(p.x * side, p.y, p.z);
                (flip(through_lens(flip(local), focal_length)), side)
            }
            Optic::Mirror { focal_length, .. } => {
                let back = element.local_direction(going) * DVec2::new(-1.0, 1.0);
                going = element.world_direction(back);
                (off_mirror(local, focal_length), -1.0)
            }
            Optic::Medium { .. } => unreachable!("an image is formed by lenses and mirrors"),
        };
        real = finite(image).is_some_and(|p| p.x * ahead > 0.0);
        source = element.world_projective(image);
    }
    finite(source).map(|point| (point, real))
}

fn trace_ray(
    placed: &[Placed],
    start: DVec2,
    direction: DVec2,
    reach: f64,
    back_trace: bool,
) -> Traced {
    let mut points = vec![start];
    let (mut at, mut direction) = (start, direction);
    let mut source = start.extend(1.0);
    // A ray that starts inside a medium starts at its index.
    let mut index = placed
        .iter()
        .find_map(|p| match (p.element.optic, &p.region) {
            (Optic::Medium { refractive_index }, Some(region)) if region.contains(start) => {
                Some(refractive_index)
            }
            _ => None,
        })
        .unwrap_or(1.0);
    let mut imaged: Option<DVec3> = None;
    for _ in 0..MOST_BENDS {
        let nearest = placed
            .iter()
            .filter_map(|p| meet(p, at, direction).map(|met| (p, met)))
            .min_by(|x, y| x.1.distance.total_cmp(&y.1.distance));
        let Some((element, met)) = nearest else {
            points.push(at + direction * reach);
            return ray(points, imaged, back_trace);
        };
        points.push(met.point);
        at = met.point;
        match (element.element.optic, met.meets) {
            (_, Meets::Back) => return ray(points, imaged, back_trace),
            (Optic::Medium { refractive_index }, Meets::Edge(normal)) => {
                let entering = direction.dot(normal) < 0.0;
                let to = if entering { refractive_index } else { 1.0 };
                let (onward, reflected) = refract(direction, normal, index, to);
                if !reflected {
                    index = to;
                }
                direction = onward;
                source = at.extend(1.0);
                imaged = None;
            }
            (optic, _) => {
                let local = element.local_projective(source);
                let Some((leaves, image)) = bend(
                    optic,
                    local,
                    element.local(at),
                    element.local_direction(direction),
                ) else {
                    points.push(at + direction * reach);
                    return ray(points, imaged, back_trace);
                };
                direction = element.world_direction(leaves);
                source = element.world_projective(image);
                imaged = Some(source);
            }
        }
    }
    ray(points, imaged, back_trace)
}

/// The traced ray through `points`, and the dashed line back from its last bend to the image
/// its source has there, when that image is virtual and `back_trace` asks for it.
fn ray(points: Vec<DVec2>, imaged: Option<DVec3>, back_trace: bool) -> Traced {
    let n = points.len();
    let image = imaged.and_then(finite);
    let virtual_image = image.map(|image| {
        let (bent, end) = (points[n - 2], points[n - 1]);
        (image - bent).dot(end - bent) < 0.0
    });
    let mut path = kurbo::BezPath::new();
    path.move_to(point(points[0]));
    for k in 1..n {
        // A ray that turns back on itself, as one through a mirror's center of curvature does,
        // starts a part of its own there: a stroke folded onto itself draws thin.
        if k > 1 && (points[k] - points[k - 1]).dot(points[k - 1] - points[k - 2]) < 0.0 {
            path.move_to(point(points[k - 1]));
        }
        path.line_to(point(points[k]));
    }
    if let (true, Some(true), Some(image)) = (back_trace, virtual_image, image) {
        dashes(&mut path, points[n - 2], image);
    }
    Traced {
        path: Path::from_bez(path).expect("a ray has length"),
        report: LightReport::Ray {
            points: points.iter().map(|p| p.to_array()).collect(),
            image: image.map(|p| p.to_array()),
            virtual_image,
        },
    }
}

fn point(p: DVec2) -> kurbo::Point {
    kurbo::Point::new(p.x, p.y)
}

/// Dashes from `from` to `to`, each its own part of `path`.
fn dashes(path: &mut kurbo::BezPath, from: DVec2, to: DVec2) {
    let length = from.distance(to);
    if length < 1e-9 {
        return;
    }
    let along = (to - from) / length;
    let mut s = 0.0;
    while s < length {
        path.move_to(point(from + along * s));
        path.line_to(point(from + along * (s + DASH.0).min(length)));
        s += DASH.0 + DASH.1;
    }
}

/// An arrow from `from` to `to` with a filled tip `tip_length` long, its shaft dashed when
/// `dashed`. `None` when it has no length.
fn arrow(from: DVec2, to: DVec2, tip_length: f64, dashed: bool) -> Option<Path> {
    if from.distance(to) < 1e-9 {
        return None;
    }
    let drawn = Path::arrow(from.as_vec2(), to.as_vec2(), tip_length as f32).ok()?;
    if !dashed {
        return Some(drawn);
    }
    let tip = tip_length.min(from.distance(to) / 4.0);
    let along = (to - from).normalize();
    let base = to - along * tip;
    let mut path = kurbo::BezPath::new();
    dashes(&mut path, from, base);
    let side = along.perp() * (tip / 2.0);
    path.move_to(point(to));
    path.line_to(point(base + side));
    path.line_to(point(base - side));
    path.close_path();
    Path::from_bez(path).ok()
}

/// A lens drawn about its object's position: a biconvex outline for a converging lens or a
/// biconcave one for a diverging lens, `height` tall, with a tick at each focal point.
pub fn lens_path(focal_length: f64, height: f64, focal_points: bool) -> Path {
    let (h, w) = (height / 2.0, 0.07 * height);
    let mut path = kurbo::BezPath::new();
    if focal_length > 0.0 {
        path.move_to((0.0, -h));
        path.quad_to((2.0 * w, 0.0), (0.0, h));
        path.quad_to((-2.0 * w, 0.0), (0.0, -h));
    } else {
        path.move_to((-w, -h));
        path.line_to((w, -h));
        path.quad_to((0.0, 0.0), (w, h));
        path.line_to((-w, h));
        path.quad_to((0.0, 0.0), (-w, -h));
    }
    path.close_path();
    if focal_points {
        for x in [-focal_length, focal_length] {
            ticks(&mut path, x);
        }
    }
    Path::from_bez(path).expect("a lens has a size")
}

/// A mirror drawn about its object's position, facing its own -x: a line, or an arc of the
/// circle of twice its focal length's radius, `height` tall, short strokes along its back, and
/// with `focal_points` a tick at its focal point and at its center of curvature.
pub fn mirror_path(focal_length: Option<f64>, height: f64, focal_points: bool) -> Path {
    let h = height / 2.0;
    let surface = |y: f64| match focal_length {
        None => DVec2::new(0.0, y),
        Some(f) => {
            let radius = 2.0 * f;
            DVec2::new(
                -radius + radius.signum() * (radius * radius - y * y).sqrt(),
                y,
            )
        }
    };
    let mut path = kurbo::BezPath::new();
    let steps = 32;
    path.move_to(point(surface(-h)));
    for k in 1..=steps {
        path.line_to(point(surface(-h + height * k as f64 / steps as f64)));
    }
    let hatches = (height / 0.3).floor().max(1.0) as usize;
    for k in 0..=hatches {
        let on = surface(-h + height * k as f64 / hatches as f64);
        path.move_to(point(on));
        path.line_to(point(on + DVec2::new(0.18, -0.12)));
    }
    if let (true, Some(f)) = (focal_points, focal_length) {
        ticks(&mut path, -f);
        ticks(&mut path, -2.0 * f);
    }
    Path::from_bez(path).expect("a mirror has a size")
}

/// A short tick across the optical axis at `x`.
fn ticks(path: &mut kurbo::BezPath, x: f64) {
    path.move_to((x, -0.12));
    path.line_to((x, 0.12));
}

/// What `trace` gives before the stage places a light: a short line, replaced at every scene
/// time.
pub fn unplaced() -> Path {
    Path::line(Vec2::ZERO, Vec2::X).expect("a line of length 1")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(optic: Optic) -> Element {
        Element {
            object: ObjectId::new("element"),
            optic,
        }
    }

    fn at_origin(element: &Element) -> Placed<'_> {
        Placed {
            element,
            origin: DVec2::ZERO,
            angle: 0.0,
            region: None,
        }
    }

    fn lens(focal_length: f64) -> Element {
        element(Optic::Lens {
            focal_length,
            height: 4.0,
        })
    }

    fn image(placed: &[Placed], point: [f64; 2]) -> (DVec2, bool) {
        image_of(placed, DVec2::from(point)).unwrap()
    }

    #[test]
    fn a_converging_lens_images_by_the_thin_lens_equation_on_both_sides_of_its_focal_point() {
        let lens = lens(2.0);
        let placed = [at_origin(&lens)];

        // Beyond F: 1/3 + 1/di = 1/2 gives di = 6, m = -2, real and upside down.
        let (beyond, real) = image(&placed, [-3.0, 1.0]);
        assert!(
            (beyond - DVec2::new(6.0, -2.0)).length() < 1e-12,
            "{beyond}"
        );
        assert!(real);

        // Inside F: 1/1 + 1/di = 1/2 gives di = -2, m = 2, virtual and upright on the object's side.
        let (inside, real) = image(&placed, [-1.0, 1.0]);
        assert!(
            (inside - DVec2::new(-2.0, 2.0)).length() < 1e-12,
            "{inside}"
        );
        assert!(!real);
    }

    #[test]
    fn a_diverging_lens_makes_a_smaller_upright_virtual_image() {
        let lens = lens(-2.0);

        // 1/4 + 1/di = -1/2 gives di = -4/3, m = 1/3.
        let (image, real) = image(&[at_origin(&lens)], [-4.0, 1.5]);

        assert!(
            (image - DVec2::new(-4.0 / 3.0, 0.5)).length() < 1e-12,
            "{image}"
        );
        assert!(!real);
    }

    #[test]
    fn a_concave_mirror_images_in_front_by_the_mirror_equation() {
        let mirror = element(Optic::Mirror {
            focal_length: Some(2.0),
            height: 4.0,
        });

        // 1/6 + 1/di = 1/2 gives di = 3 in front, m = -1/2.
        let (image, real) = image(&[at_origin(&mirror)], [-6.0, 1.0]);

        assert!((image - DVec2::new(-3.0, -0.5)).length() < 1e-12, "{image}");
        assert!(real);
    }

    #[test]
    fn a_flat_mirror_images_as_far_behind_it_upright() {
        let mirror = element(Optic::Mirror {
            focal_length: None,
            height: 4.0,
        });

        let (image, real) = image(&[at_origin(&mirror)], [-3.0, 1.0]);

        assert!((image - DVec2::new(3.0, 1.0)).length() < 1e-12);
        assert!(!real);
    }

    fn traced(placed: &[Placed], from: [f64; 2], direction: DVec2) -> Vec<DVec2> {
        match trace_ray(placed, DVec2::from(from), direction.normalize(), 5.0, true).report {
            LightReport::Ray { points, .. } => points.into_iter().map(DVec2::from).collect(),
            LightReport::Image { .. } => unreachable!(),
        }
    }

    /// Where the line through the last two points crosses x.
    fn y_at(points: &[DVec2], x: f64) -> f64 {
        let (a, b) = (points[points.len() - 2], points[points.len() - 1]);
        a.y + (b.y - a.y) * (x - a.x) / (b.x - a.x)
    }

    #[test]
    fn the_principal_rays_of_a_lens_meet_at_its_image() {
        let lens = lens(2.0);
        let placed = [at_origin(&lens)];
        let tip = DVec2::new(-3.0, 1.0);
        let reference = &placed[0];

        let rays: Vec<Vec<DVec2>> = [Principal::Parallel, Principal::Focal, Principal::Center]
            .into_iter()
            .map(|principal| {
                let direction = principal_direction(reference, principal, tip).unwrap();
                traced(&placed, tip.to_array(), direction)
            })
            .collect();

        for points in &rays {
            assert!((y_at(points, 6.0) + 2.0).abs() < 1e-9, "{points:?}");
        }
        // The parallel ray leaves through F' at (2, 0), and the focal ray leaves parallel.
        assert!(y_at(&rays[0], 2.0).abs() < 1e-12);
        assert!((rays[1][2].y - rays[1][1].y).abs() < 1e-12);
    }

    #[test]
    fn a_ray_refracts_into_glass_by_snell_s_law() {
        let incoming = DVec2::from_angle((-60f64).to_radians());

        let (into, reflected) = refract(incoming, DVec2::Y, 1.0, 1.5);

        // sin 30° = 1.5 sin θ: θ = 19.47° from the normal.
        let angle = into.x.atan2(-into.y);
        assert!(!reflected);
        assert!((angle.sin() * 1.5 - 30f64.to_radians().sin()).abs() < 1e-12);
    }

    #[test]
    fn past_the_critical_angle_glass_reflects_the_whole_ray() {
        // Glass of 1.5 into air: the critical angle is asin(1/1.5) = 41.81°.
        let critical = (1.0f64 / 1.5).asin();
        let under = DVec2::new((critical - 0.01).sin(), (critical - 0.01).cos());
        let over = DVec2::new((critical + 0.01).sin(), (critical + 0.01).cos());

        let (_, under_reflected) = refract(under, DVec2::Y, 1.5, 1.0);
        let (back, over_reflected) = refract(over, DVec2::Y, 1.5, 1.0);

        assert!(!under_reflected);
        assert!(over_reflected);
        assert!((back - DVec2::new(over.x, -over.y)).length() < 1e-12);
    }

    #[test]
    fn a_plate_of_glass_shifts_a_ray_without_turning_it() {
        let plate = element(Optic::Medium {
            refractive_index: 1.5,
        });
        let placed = [Placed {
            element: &plate,
            origin: DVec2::ZERO,
            angle: 0.0,
            region: Some(Region::Corners(vec![
                DVec2::new(-1.0, -1.0),
                DVec2::new(1.0, -1.0),
                DVec2::new(1.0, 1.0),
                DVec2::new(-1.0, 1.0),
            ])),
        }];
        let direction = DVec2::new(1.0, -0.5).normalize();

        let points = traced(&placed, [-3.0, 1.5], direction);

        assert_eq!(points.len(), 4);
        let out = (points[3] - points[2]).normalize();
        assert!((out - direction).length() < 1e-12, "{out}");
    }

    #[test]
    fn a_telescope_of_two_lenses_sends_parallel_rays_out_parallel_and_steeper() {
        // Objective of 4 and eyepiece of 1, 5 apart, their focal points meeting.
        let objective = lens(4.0);
        let eyepiece = lens(1.0);
        let placed = [
            at_origin(&objective),
            Placed {
                element: &eyepiece,
                origin: DVec2::new(5.0, 0.0),
                angle: 0.0,
                region: None,
            },
        ];
        let tilt = 0.05f64;

        let points = traced(&placed, [-2.0, 0.3], DVec2::new(1.0, tilt));

        let out = points[3] - points[2];
        // The angular magnification is -fo/fe = -4.
        assert!((out.y / out.x + 4.0 * tilt).abs() < 1e-12, "{out}");
    }
}
