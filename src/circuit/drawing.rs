//! How a circuit is drawn: each component's symbol on its grid edge with leads to its ends, a dot
//! where three or more ends meet, a ground symbol, and each label beside its symbol on the side
//! where it meets nothing else. At each scene time the stage gives it the state a simulation left,
//! so its switches stand open or closed, dots of current move along every component and wire by
//! the charge that has passed, and each bulb glows by the power it draws.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use glam::{Vec2, Vec3};
use kurbo::{BezPath, Rect};
use serde::Serialize;

use super::{Kind, Network, Quantity, describe};
use crate::path::Path;
use crate::scene::{Align, Color, Font, Glyph, Positive};
use crate::typeset::{self, Layout};

/// The size of a component's symbol, as shares of the circuit's spacing.
const BODY: f32 = 0.25;
const ZIGZAG: f32 = 0.12;
const BULB: f32 = 0.2;
const METER: f32 = 0.22;
const PLATE_GAP: f32 = 0.07;
const CONTACT: f32 = 0.04;
const JUNCTION: f32 = 0.055;
/// How far a lit bulb's halo reaches, in bulb radii.
const GLOW_REACH: f32 = 1.6;
/// The gap between a label and what it labels, as a share of the spacing.
const LABEL_GAP: f32 = 0.08;
/// A cubic's handle for a quarter circle, as a share of its radius.
const QUARTER: f64 = 0.552_284_75;
const GLOW: Color = Color {
    r: 0xff,
    g: 0xd8,
    b: 0x4d,
};

/// The look of a circuit: its color, the width of its strokes, its labels' text size, and how far
/// apart its grid points are, in world units.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    pub color: Color,
    pub stroke_width: Positive,
    pub size: Positive,
    pub spacing: f32,
}

/// Dots of current: how far they move a second for each ampere, how far apart they are, in world
/// units, and their color.
#[derive(Clone, Copy, Debug)]
pub struct CurrentDots {
    pub speed: f32,
    pub spacing: f32,
    pub color: Color,
}

/// One label: the component it labels, its glyphs and the box they fill.
#[derive(Clone, Debug)]
pub struct Label {
    pub component: usize,
    pub glyphs: Range<usize>,
    pub area: Rect,
}

/// The straight run of one component in the circuit's plane, and the stretch of it its symbol
/// fills, along it from `from`, where no dot of current is drawn.
#[derive(Clone, Debug)]
struct Run {
    from: Vec2,
    to: Vec2,
    body: Option<(f32, f32)>,
}

#[derive(Clone, Debug)]
pub struct Circuit {
    pub network: Arc<Network>,
    pub look: Look,
    pub dots: Option<CurrentDots>,
    /// Every symbol's strokes, leads and wires, the ground symbol, and the meters' circles: all
    /// but the switches' levers.
    strokes: BezPath,
    /// Battery and capacitor plates, switch contacts and junction dots.
    pub fills: Option<Path>,
    /// The meters' letters, then every label's glyphs.
    pub glyphs: Vec<Glyph>,
    pub labels: Vec<Label>,
    /// Each switch's lever, closed and open.
    levers: Vec<(usize, [(Vec2, Vec2); 2])>,
    /// Each bulb's center and radius.
    bulbs: Vec<(usize, Vec2, f32)>,
    runs: Vec<Run>,
    /// The circuit's state at this stage, three numbers a component, as `circuit` describes.
    pub state: Vec<f64>,
    /// The power at which a bulb glows its brightest: the most any bulb of the circuit draws in
    /// any of its simulations.
    pub full_power: f64,
}

/// Why a circuit cannot be drawn: its component's index and the message.
pub type DrawFault = (usize, String);

impl Circuit {
    pub fn new(
        network: Arc<Network>,
        look: Look,
        dots: Option<CurrentDots>,
    ) -> Result<Circuit, DrawFault> {
        let s = look.spacing;
        let at = |p: glam::IVec2| p.as_vec2() * s;
        let mut strokes = BezPath::new();
        let mut fills = BezPath::new();
        let mut glyphs = Vec::new();
        let mut levers = Vec::new();
        let mut bulbs = Vec::new();
        let mut runs = Vec::new();
        // Each component's box in the plane, for placing labels clear of it.
        let mut taken: Vec<(Rect, String)> = Vec::new();
        for (i, c) in network.components.iter().enumerate() {
            let (from, to) = (at(c.from), at(c.to));
            let length = from.distance(to);
            let d = (to - from) / length;
            let n = d.perp();
            let mid = length / 2.0;
            let local = |u: f32, v: f32| from + d * u + n * v;
            let half = match c.kind {
                Kind::Wire => 0.0,
                Kind::Resistor { .. } => BODY * s,
                Kind::Bulb { .. } => BULB * s,
                Kind::Battery { .. } | Kind::Capacitor { .. } => PLATE_GAP * s,
                Kind::Inductor { .. } | Kind::Switch => BODY * s,
                Kind::Ammeter | Kind::Voltmeter => METER * s,
            };
            if half > 0.0 {
                line(&mut strokes, from, local(mid - half, 0.0));
                line(&mut strokes, local(mid + half, 0.0), to);
            } else {
                line(&mut strokes, from, to);
            }
            let reach = match c.kind {
                Kind::Wire => 0.0,
                Kind::Resistor { .. } => ZIGZAG * s,
                Kind::Bulb { .. } => GLOW_REACH * BULB * s,
                Kind::Battery { .. } => 0.3 * s,
                Kind::Capacitor { .. } => 0.25 * s,
                Kind::Inductor { .. } => BODY * s / 4.0,
                Kind::Switch => BODY * s,
                Kind::Ammeter | Kind::Voltmeter => METER * s,
            };
            match c.kind {
                Kind::Resistor { .. } => {
                    let mut points = vec![local(mid - half, 0.0)];
                    for k in 1..12 {
                        let v = if k % 2 == 1 { ZIGZAG * s } else { -ZIGZAG * s };
                        points.push(local(mid - half + k as f32 * half / 6.0, v));
                    }
                    points.push(local(mid + half, 0.0));
                    strokes_through(&mut strokes, &points);
                }
                Kind::Bulb { .. } => {
                    let center = local(mid, 0.0);
                    circle(&mut strokes, center, half);
                    let k = 0.7 * half / std::f32::consts::SQRT_2;
                    line(&mut strokes, center - (d + n) * k, center + (d + n) * k);
                    line(&mut strokes, center - (d - n) * k, center + (d - n) * k);
                    bulbs.push((i, center, half));
                }
                Kind::Battery { .. } => {
                    // The long thin plate is the positive end, toward `to`.
                    plate(&mut fills, local(mid + half, 0.0), d, n, 0.015 * s, 0.3 * s);
                    plate(&mut fills, local(mid - half, 0.0), d, n, 0.04 * s, 0.15 * s);
                }
                Kind::Capacitor { .. } => {
                    plate(&mut fills, local(mid + half, 0.0), d, n, 0.02 * s, 0.25 * s);
                    plate(&mut fills, local(mid - half, 0.0), d, n, 0.02 * s, 0.25 * s);
                }
                Kind::Inductor { .. } => {
                    let radius = half / 4.0;
                    for k in 0..4 {
                        let center = local(mid - half + (2 * k + 1) as f32 * radius, 0.0);
                        hump(&mut strokes, center, d, n, radius);
                    }
                }
                Kind::Switch => {
                    let (pivot, contact) = (local(mid - half, 0.0), local(mid + half, 0.0));
                    circle(&mut fills, pivot, CONTACT * s);
                    circle(&mut fills, contact, CONTACT * s);
                    let (cos, sin) = (30f32.to_radians().cos(), 30f32.to_radians().sin());
                    let open = pivot + (d * cos + n * sin) * 2.0 * half;
                    levers.push((i, [(pivot, contact), (pivot, open)]));
                }
                Kind::Ammeter | Kind::Voltmeter => {
                    let center = local(mid, 0.0);
                    circle(&mut strokes, center, half);
                    let text = if c.kind == Kind::Ammeter { "A" } else { "V" };
                    let set = typeset::text_box(
                        text,
                        Font::Sans,
                        look.color,
                        Layout {
                            size: 1.3 * half,
                            align: Align::Center,
                        },
                    )
                    .map_err(|e| (i, format!("{e:?}")))?;
                    glyphs.extend(moved(set.glyphs, center));
                }
                Kind::Wire => {}
            }
            let body = (half > 0.0).then_some((mid - half, mid + half));
            runs.push(Run { from, to, body });
            let area = Rect::from_points(point(local(0.0, -reach)), point(local(length, reach)));
            taken.push((
                area.inflate(
                    f64::from(look.stroke_width.get()) / 2.0,
                    f64::from(look.stroke_width.get()) / 2.0,
                ),
                describe(c, i),
            ));
        }
        if let Some(ground) = network.ground {
            let g = at(ground);
            let (stem, widths) = (0.2 * s, [0.18 * s, 0.12 * s, 0.06 * s]);
            line(&mut strokes, g, g - Vec2::Y * stem);
            for (k, w) in widths.into_iter().enumerate() {
                let y = g.y - stem - k as f32 * 0.07 * s;
                line(&mut strokes, Vec2::new(g.x - w, y), Vec2::new(g.x + w, y));
            }
            taken.push((
                Rect::new(
                    (g.x - widths[0]) as f64,
                    (g.y - stem - 0.14 * s) as f64,
                    (g.x + widths[0]) as f64,
                    g.y as f64,
                ),
                "the ground".to_string(),
            ));
        }
        let mut degree = vec![0; network.nodes.len()];
        for [a, b] in &network.ends {
            degree[*a] += 1;
            degree[*b] += 1;
        }
        for (node, p) in network.nodes.iter().enumerate() {
            if degree[node] >= 3 {
                circle(&mut fills, at(*p), JUNCTION * s);
            }
        }
        let drawn = taken
            .iter()
            .map(|(r, _)| *r)
            .reduce(|a, b| a.union(b))
            .unwrap_or_default();
        let middle = drawn.center();
        let mut labels = Vec::new();
        for (i, c) in network.components.iter().enumerate() {
            let Some(tex) = &c.label else { continue };
            let set = typeset::math_box(
                tex,
                look.color,
                Layout {
                    size: look.size.get(),
                    align: Align::Center,
                },
            )
            .map_err(|e| (i, format!("its label cannot be typeset: {e:?}")))?;
            let run = &runs[i];
            let d = (run.to - run.from).normalize();
            let n = d.perp();
            let mid = (run.from + run.to) / 2.0;
            let reach = (taken[i].0.height().min(taken[i].0.width()) / 2.0) as f32;
            let half = Vec2::new(set.width, set.height) / 2.0;
            let away = n.abs().dot(half) + reach + LABEL_GAP * s;
            let toward_outside = n.dot(mid - Vec2::new(middle.x as f32, middle.y as f32));
            let sides = if toward_outside < 0.0 {
                [-1.0, 1.0]
            } else {
                [1.0, -1.0]
            };
            let mut placed = None;
            let mut met: Vec<String> = Vec::new();
            for side in sides {
                let center = mid + n * away * side;
                let area = Rect::from_center_size(
                    point(center),
                    (f64::from(set.width), f64::from(set.height)),
                );
                let hits: Vec<&String> = taken
                    .iter()
                    .filter(|(r, _)| r.intersect(area).area() > 1e-12)
                    .map(|(_, name)| name)
                    .collect();
                if hits.is_empty() {
                    placed = Some((center, area));
                    break;
                }
                met.extend(hits.into_iter().cloned());
            }
            let Some((center, area)) = placed else {
                met.dedup();
                return Err((
                    i,
                    format!(
                        "{}'s label meets {} on both sides of it; give the circuit a larger `spacing`, a smaller `size`, or a shorter label",
                        describe(c, i),
                        super::list(&met)
                    ),
                ));
            };
            let start = glyphs.len();
            glyphs.extend(moved(set.glyphs, center));
            labels.push(Label {
                component: i,
                glyphs: start..glyphs.len(),
                area,
            });
            let gap = f64::from(LABEL_GAP * s) / 2.0;
            taken.push((
                area.inflate(gap, gap),
                format!("{}'s label", describe(c, i)),
            ));
        }
        let state = network.rest();
        Ok(Circuit {
            network,
            look,
            dots,
            strokes,
            fills: (!fills.elements().is_empty())
                .then(|| Path::from_bez(fills).ok())
                .flatten(),
            glyphs,
            labels,
            levers,
            bulbs,
            runs,
            state,
            full_power: 0.0,
        })
    }

    fn closed(&self, component: usize) -> bool {
        self.state[3 * component + 2] == 1.0
    }

    /// Every stroke: the symbols, leads and wires, and each switch's lever as it stands.
    pub fn stroke_path(&self) -> Path {
        let mut strokes = self.strokes.clone();
        for (i, [closed, open]) in &self.levers {
            let (a, b) = if self.closed(*i) { *closed } else { *open };
            line(&mut strokes, a, b);
        }
        Path::from_bez(strokes).expect("a circuit has at least one component, with length")
    }

    /// The dots of current, each where the charge that has passed through its component has
    /// carried it: none inside a symbol.
    pub fn dot_path(&self) -> Option<Path> {
        let dots = self.dots?;
        let radius = 0.045 * self.look.spacing;
        let mut path = BezPath::new();
        for (i, run) in self.runs.iter().enumerate() {
            let length = run.from.distance(run.to);
            let d = (run.to - run.from) / length;
            let passed =
                (self.state[3 * i] * f64::from(dots.speed)).rem_euclid(f64::from(dots.spacing));
            let mut u = passed as f32;
            while u <= length {
                let inside = run
                    .body
                    .is_some_and(|(a, b)| u > a - radius && u < b + radius);
                if !inside && u >= radius && u <= length - radius {
                    circle(&mut path, run.from + d * u, radius);
                }
                u += dots.spacing;
            }
        }
        (!path.elements().is_empty())
            .then(|| Path::from_bez(path).ok())
            .flatten()
    }

    /// Each lit bulb's glow: its disc and its halo, each with its opacity.
    pub fn glows(&self) -> Vec<(Path, f32)> {
        if self.full_power <= 0.0 || self.bulbs.is_empty() {
            return Vec::new();
        }
        let solution = self.network.solve(&self.state);
        self.bulbs
            .iter()
            .filter_map(|&(i, center, radius)| {
                let power = solution.drops[i] * solution.currents[i];
                let share = (power / self.full_power).clamp(0.0, 1.0) as f32;
                (share > 0.0).then_some((center, radius, share))
            })
            .flat_map(|(center, radius, share)| {
                [(radius, 0.9 * share), (GLOW_REACH * radius, 0.3 * share)]
                    .into_iter()
                    .filter_map(move |(r, opacity)| {
                        Some((Path::circle_at(center, r).ok()?, opacity))
                    })
            })
            .collect()
    }

    pub const GLOW: Color = GLOW;

    /// The box of everything the circuit draws, in its own plane.
    pub fn bounding_box(&self) -> Rect {
        let mut area = self.stroke_path().bounding_box();
        for (_, [_, (a, b)]) in &self.levers {
            area = area.union_pt(point(*a)).union_pt(point(*b));
        }
        for &(_, center, radius) in &self.bulbs {
            let r = f64::from(GLOW_REACH * radius);
            area = area.union(Rect::from_center_size(point(center), (2.0 * r, 2.0 * r)));
        }
        for label in &self.labels {
            area = area.union(label.area);
        }
        area.inflate(
            f64::from(self.look.stroke_width.get()) / 2.0,
            f64::from(self.look.stroke_width.get()) / 2.0,
        )
    }

    /// How far its farthest drawn point lies from the object's position.
    pub fn reach(&self) -> f32 {
        let r = self.bounding_box();
        [(r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)]
            .into_iter()
            .map(|(x, y)| x.hypot(y) as f32)
            .fold(0.0, f32::max)
    }
}

fn point(v: Vec2) -> kurbo::Point {
    kurbo::Point::new(f64::from(v.x), f64::from(v.y))
}

fn line(path: &mut BezPath, a: Vec2, b: Vec2) {
    path.move_to(point(a));
    path.line_to(point(b));
}

fn strokes_through(path: &mut BezPath, points: &[Vec2]) {
    path.move_to(point(points[0]));
    for p in &points[1..] {
        path.line_to(point(*p));
    }
}

fn circle(path: &mut BezPath, center: Vec2, radius: f32) {
    let circle = kurbo::Circle::new(point(center), f64::from(radius));
    path.extend(kurbo::Shape::path_elements(&circle, 1e-4));
}

/// A filled plate: `along` half as thick along the run, `across` half as long across it.
fn plate(path: &mut BezPath, center: Vec2, d: Vec2, n: Vec2, along: f32, across: f32) {
    let corners = [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .map(|(a, b)| center + d * along * a + n * across * b);
    strokes_through(path, &corners);
    path.close_path();
}

/// One hump of an inductor's coil: the half circle of `radius` about `center` on the `n` side.
fn hump(path: &mut BezPath, center: Vec2, d: Vec2, n: Vec2, radius: f32) {
    let (c, d, n, r) = (
        point(center),
        point(d).to_vec2(),
        point(n).to_vec2(),
        f64::from(radius),
    );
    let (start, top, end) = (c - d * r, c + n * r, c + d * r);
    path.move_to(start);
    path.curve_to(start + n * r * QUARTER, top - d * r * QUARTER, top);
    path.curve_to(top + d * r * QUARTER, end + n * r * QUARTER, end);
}

fn moved(glyphs: Vec<Glyph>, by: Vec2) -> impl Iterator<Item = Glyph> {
    glyphs.into_iter().map(move |glyph| Glyph {
        path: glyph.path.moved(by),
        ..glyph
    })
}

/// What `inspect` tells an agent about a circuit at a scene time: each node's grid point, voltage
/// and place, each component with an id by its id, with its ends and quantities, and each wire
/// with its ends and current.
#[derive(Debug, Serialize)]
pub struct CircuitReport {
    pub nodes: Vec<NodeReport>,
    pub components: BTreeMap<String, ComponentReport>,
    pub wires: Vec<WireReport>,
}

#[derive(Debug, Serialize)]
pub struct WireReport {
    pub from: [i32; 2],
    pub to: [i32; 2],
    pub current: f64,
}

#[derive(Debug, Serialize)]
pub struct NodeReport {
    pub at: [i32; 2],
    pub voltage: f64,
    pub position: Vec3,
    pub position_pixels: Option<Vec2>,
}

#[derive(Debug, Serialize)]
pub struct ComponentReport {
    pub kind: &'static str,
    pub from: [i32; 2],
    pub to: [i32; 2],
    pub current: f64,
    pub voltage: f64,
    pub power: f64,
    pub charge: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub energy: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<bool>,
    /// The box of its label's glyphs, which nothing else of the circuit enters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_bounding_box: Option<crate::mesh::BoundingBox>,
}

impl Circuit {
    /// The report, with `to_world` placing a point of the circuit's plane in the world, and
    /// `label_box` measuring a label.
    pub fn report(
        &self,
        to_world: impl Fn(Vec2) -> Vec3,
        to_pixels: impl Fn(Vec3) -> Option<Vec2>,
        label_box: impl Fn(&Label) -> Option<crate::mesh::BoundingBox>,
    ) -> CircuitReport {
        let network = &self.network;
        let solution = network.solve(&self.state);
        let nodes = network
            .nodes
            .iter()
            .zip(&solution.node_voltages)
            .map(|(p, &voltage)| {
                let position = to_world(p.as_vec2() * self.look.spacing);
                NodeReport {
                    at: p.to_array(),
                    voltage: voltage + 0.0,
                    position,
                    position_pixels: to_pixels(position),
                }
            })
            .collect();
        let quantities = network.quantities();
        let components = network
            .components
            .iter()
            .enumerate()
            .filter_map(|(i, c)| Some((i, c, c.id.clone()?)))
            .map(|(i, c, id)| {
                let value = |q| network.value(&solution, &self.state, i, q);
                let has = |q| quantities.contains(&(i, q));
                let report = ComponentReport {
                    kind: c.kind.word(),
                    from: c.from.to_array(),
                    to: c.to.to_array(),
                    current: value(Quantity::Current),
                    voltage: value(Quantity::Voltage),
                    power: value(Quantity::Power),
                    charge: value(Quantity::Charge),
                    energy: has(Quantity::Energy).then(|| value(Quantity::Energy)),
                    closed: has(Quantity::Closed).then(|| self.closed(i)),
                    label_bounding_box: self
                        .labels
                        .iter()
                        .find(|label| label.component == i)
                        .and_then(&label_box),
                };
                (id, report)
            })
            .collect();
        let wires = network
            .components
            .iter()
            .enumerate()
            .filter(|(_, c)| c.id.is_none())
            .map(|(i, c)| WireReport {
                from: c.from.to_array(),
                to: c.to.to_array(),
                current: solution.currents[i] + 0.0,
            })
            .collect();
        CircuitReport {
            nodes,
            components,
            wires,
        }
    }
}
