//! Axes and number lines: where a data value lands in an object's own space, and the rulings and
//! tick labels that show it.
//!
//! Both draw the same way, as `Drawing`: one stroked path of axis lines and ticks, and the filled
//! glyphs of every tick label and axis label. A create draws the rulings along their length and
//! then fades the labels in, as it draws a stroke and then a fill, so `mesh` has no new curve
//! code. A plot reads where data lands along each axis, in data or in the axis scale, and nothing
//! else of an axes.

use glam::{Mat4, Vec2, Vec3};
use kurbo::{Affine, BezPath, Point, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::mesh::{BoundingBox, corners};
use crate::path::Path;
use crate::scene::{Align, Color, Font, Glyph, Stroke};
use crate::typeset::{self, Layout, TypesetError};

/// The longest tick list an axis draws. More is a spacing the agent did not mean.
pub const MAX_TICKS: usize = 100;

/// The length of a tick across its axis, in world units, centered on the axis.
pub(crate) const TICK_LENGTH: f32 = 0.16;
/// The space between a tick and the edge of its label.
pub(crate) const LABEL_GAP: f32 = 0.12;
/// Half the height of a digit as a share of the text size, to center a label on its tick.
const HALF_DIGIT_HEIGHT: f32 = 0.35;

/// One axis: the data values it shows, how it lays them along its length, the spacing of its
/// ticks, and how long it is in the object's own units. The axis runs across a box centered on
/// the object's position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    min: f64,
    max: f64,
    scale: AxisScale,
    /// The distance between ticks in the axis scale: data units on a linear axis, powers of ten
    /// on a log axis.
    spacing: f64,
    length: f32,
    /// Where the other axis crosses this one: 0 or the nearest end of the range, and the low
    /// end of a category axis, so the other axis runs beside the first category, not through it.
    crossing: f64,
}

/// How an axis lays data values along its length. `linear`, the default, puts equal steps of
/// value at equal steps along it. `log` puts each power of ten an equal step along it, so a
/// value 1000 times another sits three such steps past it. The variants carry no doc comments,
/// for the reason `FillRule` gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AxisScale {
    #[default]
    Linear,
    Log,
}

impl AxisScale {
    fn is_linear(&self) -> bool {
        *self == AxisScale::Linear
    }

    /// Where `value` lies in this scale: the value itself, or its power of ten.
    fn scaled(self, value: f64) -> f64 {
        match self {
            AxisScale::Linear => value,
            AxisScale::Log => value.log10(),
        }
    }

    fn unscaled(self, scaled: f64) -> f64 {
        match self {
            AxisScale::Linear => scaled,
            AxisScale::Log => 10f64.powf(scaled),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum AxisError {
    /// `range` does not have its lowest value first, or has a value that is not a number.
    EmptyRange,
    /// A log axis' range starts at 0 or below, where no power of ten lies.
    NotAboveZero,
    /// A log axis' spacing is not a whole number of powers of ten.
    NotWholePowers,
    /// The spacing gives more than `MAX_TICKS` ticks.
    TooManyTicks(usize),
}

impl Axis {
    /// `spacing` is the distance between ticks in the axis scale, or `None` for a round number
    /// that gives about five: data units on a linear axis, and on a log axis a whole number of
    /// powers of ten, one when that gives five or fewer.
    pub fn new(
        min: f64,
        max: f64,
        scale: AxisScale,
        spacing: Option<f64>,
        length: f32,
    ) -> Result<Axis, AxisError> {
        if !(min.is_finite() && max.is_finite() && min < max) {
            return Err(AxisError::EmptyRange);
        }
        if scale == AxisScale::Log && min <= 0.0 {
            return Err(AxisError::NotAboveZero);
        }
        let span = scale.scaled(max) - scale.scaled(min);
        let spacing = match (scale, spacing) {
            (_, Some(spacing)) => spacing,
            (AxisScale::Linear, None) => nice_spacing(span),
            (AxisScale::Log, None) => nice_spacing(span).max(1.0),
        };
        if scale == AxisScale::Log && spacing.fract() != 0.0 {
            return Err(AxisError::NotWholePowers);
        }
        let axis = Axis {
            min,
            max,
            scale,
            spacing,
            length,
            crossing: 0.0f64.clamp(min, max),
        };
        let count = axis.ticks().count();
        if count > MAX_TICKS {
            return Err(AxisError::TooManyTicks(count));
        }
        Ok(axis)
    }

    /// A category axis of `count` categories: category k is at data value k, one data unit wide,
    /// with a tick at each, and the other axis crosses at the low end.
    pub fn categories(count: usize, length: f32) -> Axis {
        let min = -0.5;
        Axis {
            min,
            max: count as f64 - 0.5,
            scale: AxisScale::Linear,
            spacing: 1.0,
            length,
            crossing: min,
        }
    }

    pub fn range(&self) -> (f64, f64) {
        (self.min, self.max)
    }

    pub fn scale(&self) -> AxisScale {
        self.scale
    }

    pub fn spacing(&self) -> f64 {
        self.spacing
    }

    /// Whether the axis can place `value`: any value on a linear axis, and a value above 0 on a
    /// log axis.
    pub fn places(&self, value: f64) -> bool {
        self.scale == AxisScale::Linear || value > 0.0
    }

    /// Where `value` lies in the axis scale, along which data lands evenly: the value itself on
    /// a linear axis and its power of ten on a log axis.
    pub fn scaled(&self, value: f64) -> f64 {
        self.scale.scaled(value)
    }

    pub fn unscaled(&self, scaled: f64) -> f64 {
        self.scale.unscaled(scaled)
    }

    /// The range in the axis scale.
    pub fn scaled_range(&self) -> (f64, f64) {
        (self.scaled(self.min), self.scaled(self.max))
    }

    /// The own length one step of the axis scale spans along the axis, before the object's
    /// scale: one data unit on a linear axis, one power of ten on a log axis.
    pub fn unit(&self) -> f64 {
        let (low, high) = self.scaled_range();
        f64::from(self.length) / (high - low)
    }

    /// Where a data value lands along the axis: `-length / 2` at the lowest value and
    /// `length / 2` at the highest, and beyond them outside the range. On a log axis a value at
    /// or below 0 has no place, and lands at NaN.
    pub fn to_local(self, value: f64) -> f32 {
        self.scaled_to_local(self.scaled(value))
    }

    /// Where a value in the axis scale lands along the axis.
    pub fn scaled_to_local(self, scaled: f64) -> f32 {
        let (low, high) = self.scaled_range();
        (((scaled - low) / (high - low) - 0.5) * f64::from(self.length)) as f32
    }

    pub fn to_data(self, local: f32) -> f64 {
        let (low, high) = self.scaled_range();
        self.unscaled((f64::from(local) / f64::from(self.length) + 0.5) * (high - low) + low)
    }

    /// Every multiple of the spacing in the range of the axis scale, so ticks sit on round
    /// values whatever the range starts at: on a log axis, at powers of ten.
    pub fn ticks(&self) -> impl Iterator<Item = f64> + '_ {
        let (low, high) = self.scaled_range();
        let first = (low / self.spacing - 1e-9).ceil() as i64;
        let last = (high / self.spacing + 1e-9).floor() as i64;
        (first..=last).map(|k| match self.scale {
            AxisScale::Linear => k as f64 * self.spacing,
            // Read from its decimal form, so 10^-6 is the double nearest 0.000001, as a range
            // written 1e-6 is.
            AxisScale::Log => format!("1e{}", k * self.spacing as i64)
                .parse()
                .expect("a power of ten in decimal form parses"),
        })
    }

    pub(crate) fn crossing(&self) -> f64 {
        self.crossing
    }

    /// A tick's value as LaTeX math: on a linear axis with as many decimals as the spacing has,
    /// such as `-1.5` or `20`, and on a log axis as a power of ten, such as `10^{-6}`.
    pub(crate) fn label(&self, value: f64) -> String {
        match self.scale {
            AxisScale::Linear => self.number(value),
            AxisScale::Log => format!("10^{{{}}}", self.number(value.log10())),
        }
    }

    /// `value` with as many decimals as the spacing has, and no negative zero.
    fn number(&self, value: f64) -> String {
        let decimals = (0..=8).find(|d| {
            let scaled = self.spacing * 10f64.powi(*d);
            (scaled - scaled.round()).abs() <= 1e-9 * scaled.abs().max(1.0)
        });
        let text = format!("{value:.*}", decimals.unwrap_or(8) as usize);
        if text
            .trim_start_matches('-')
            .chars()
            .all(|c| c == '0' || c == '.')
        {
            text.trim_start_matches('-').to_string()
        } else {
            text
        }
    }
}

/// The round spacing (1, 2 or 5 times a power of ten) that gives about five ticks over `span`.
fn nice_spacing(span: f64) -> f64 {
    let raw = span / 5.0;
    let magnitude = 10f64.powf(raw.log10().floor());
    let share = raw / magnitude;
    magnitude
        * if share < 1.5 {
            1.0
        } else if share < 3.5 {
            2.0
        } else if share < 7.5 {
            5.0
        } else {
            10.0
        }
}

/// What is drawn for an axes or a number line, in the object's own units.
#[derive(Clone, Debug)]
pub struct Drawing {
    /// The axis lines, then every tick: open parts, so a create draws the lines first.
    pub rulings: Path,
    pub stroke: Stroke,
    /// Every tick label and axis label, each with its glyphs filled in their own colors under
    /// the non-zero rule: the appearance's color, unless LaTeX in an axis label colors them.
    pub labels: Vec<Label>,
}

/// One tick label or axis label: what it labels, the LaTeX it typesets, the side of what it
/// labels it sits on, its glyphs, and the box around them in the object's own units.
#[derive(Clone, Debug)]
pub struct Label {
    /// One tick, or an axis. The label where the axes cross labels both ticks there when they
    /// say the same, as both say 0 where the axes cross at 0.
    pub of: Vec<Labelled>,
    pub text: String,
    pub side: Side,
    pub glyphs: Vec<Glyph>,
    pub bounding_box: Rect,
}

/// Where a label sits beside its tick, or beside the end of its axis, in the object's own plane:
/// below is toward its -y.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Side {
    Below,
    Above,
    Left,
    Right,
    /// The corner below and left of where the axes cross.
    BelowLeft,
}

/// What a label labels, as `inspect` names it: a tick by its axis and value, or an axis by its
/// axis and the LaTeX of its label.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Labelled {
    Tick { axis: &'static str, value: f64 },
    Axis { axis: &'static str, label: String },
}

/// How an axes or a number line looks.
#[derive(Clone, Copy, Debug)]
pub struct Appearance {
    pub stroke: Stroke,
    pub color: Color,
    /// The text size of the labels.
    pub label_size: f32,
}

/// Why an axis label would not typeset, with the axis it is on.
#[derive(Debug)]
pub struct LabelError {
    pub axis: &'static str,
    pub error: TypesetError,
}

#[derive(Clone, Debug)]
pub struct Axes {
    pub x: Axis,
    pub y: Axis,
    /// The categories of a category x axis, in order, or `None` when x shows numbers.
    pub categories: Option<Vec<String>>,
    pub drawing: Drawing,
}

#[derive(Clone, Debug)]
pub struct NumberLine {
    pub axis: Axis,
    pub drawing: Drawing,
}

struct Builder {
    rulings: BezPath,
    labels: Vec<Label>,
    size: f32,
    color: Color,
}

impl Builder {
    fn new(appearance: Appearance) -> Builder {
        Builder {
            rulings: BezPath::new(),
            labels: Vec::new(),
            size: appearance.label_size,
            color: appearance.color,
        }
    }

    fn segment(&mut self, from: Vec2, to: Vec2) {
        self.rulings.move_to(point(from));
        self.rulings.line_to(point(to));
    }

    /// Typesets `tex` and puts the point of its box that `align` names at `at`.
    fn label(
        &mut self,
        tex: &str,
        align: Align,
        at: Vec2,
        side: Side,
        of: Vec<Labelled>,
    ) -> Result<(), TypesetError> {
        let layout = Layout {
            size: self.size,
            align,
        };
        let glyphs = typeset::math(tex, self.color, layout)?;
        self.place(glyphs, tex, at, side, of);
        Ok(())
    }

    /// Typesets a category's label as plain text, centered under `at`.
    fn category_label(
        &mut self,
        category: &str,
        at: Vec2,
        of: Labelled,
    ) -> Result<(), TypesetError> {
        let layout = Layout {
            size: self.size,
            align: Align::Center,
        };
        let glyphs = typeset::text(category, Font::Serif, self.color, layout)?;
        self.place(glyphs, category, at, Side::Below, vec![of]);
        Ok(())
    }

    fn place(&mut self, glyphs: Vec<Glyph>, text: &str, at: Vec2, side: Side, of: Vec<Labelled>) {
        let glyphs: Vec<Glyph> = glyphs
            .into_iter()
            .map(|glyph| {
                let mut moved = BezPath::from_vec(glyph.path.elements().to_vec());
                moved.apply_affine(Affine::translate((f64::from(at.x), f64::from(at.y))));
                Glyph {
                    path: Path::from_bez(moved).expect("a moved glyph has the length it had"),
                    ..glyph
                }
            })
            .collect();
        if let Some(bounding_box) = glyphs
            .iter()
            .map(|glyph| glyph.path.bounding_box())
            .reduce(|a, b| a.union(b))
        {
            self.labels.push(Label {
                of,
                text: text.to_string(),
                side,
                glyphs,
                bounding_box,
            });
        }
    }

    fn finish(self, appearance: Appearance) -> Drawing {
        Drawing {
            rulings: Path::from_bez(self.rulings).expect("an axis has length"),
            stroke: appearance.stroke,
            labels: self.labels,
        }
    }
}

impl Drawing {
    /// The label of what `is_labelled` picks out, or `None` when nothing labels it.
    fn label_of(&self, is_labelled: impl Fn(&Labelled) -> bool) -> Option<&Label> {
        self.labels
            .iter()
            .find(|label| label.of.iter().any(&is_labelled))
    }

    /// What the tick of `axis` at `value` shows of its label, or `None` when it shows none.
    fn tick_label(
        &self,
        axis: &'static str,
        value: f64,
        object_to_world: Mat4,
        to_pixels: impl Fn(Vec3) -> Option<Vec2>,
    ) -> Option<TickLabel> {
        let label = self.label_of(|of| *of == Labelled::Tick { axis, value })?;
        Some(TickLabel {
            label_text: label.text.clone(),
            label_side: label.side,
            label_bounding_box: label.bounding_box_in(object_to_world, to_pixels),
        })
    }

    /// The bounding box of the label of the axis `which`, or `None` when it has none.
    fn axis_label_bounding_box(
        &self,
        which: &str,
        object_to_world: Mat4,
        to_pixels: impl Fn(Vec3) -> Option<Vec2>,
    ) -> Option<BoundingBox> {
        let label =
            self.label_of(|of| matches!(of, Labelled::Axis { axis, .. } if *axis == which))?;
        Some(label.bounding_box_in(object_to_world, to_pixels))
    }
}

impl Label {
    /// The bounding box in world units and pixels.
    fn bounding_box_in(
        &self,
        object_to_world: Mat4,
        to_pixels: impl Fn(Vec3) -> Option<Vec2>,
    ) -> BoundingBox {
        let r = self.bounding_box;
        let world: Vec<Vec3> = [(r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)]
            .into_iter()
            .map(|(x, y)| object_to_world.transform_point3(Vec3::new(x as f32, y as f32, 0.0)))
            .collect();
        let pixels = corners(
            world.iter().filter_map(|&corner| to_pixels(corner)),
            Vec2::min,
            Vec2::max,
        );
        BoundingBox {
            world: corners(world.into_iter(), Vec3::min, Vec3::max)
                .expect("a box has four corners"),
            pixels,
        }
    }
}

fn point(v: Vec2) -> Point {
    Point::new(f64::from(v.x), f64::from(v.y))
}

impl Axes {
    /// `x_label` and `y_label` are LaTeX math, set at the high end of their axis.
    pub fn new(
        x: Axis,
        y: Axis,
        x_label: Option<&str>,
        y_label: Option<&str>,
        appearance: Appearance,
    ) -> Result<Axes, LabelError> {
        Axes::drawn(x, None, y, [x_label, y_label], appearance)
    }

    /// Axes whose x is a category axis `width` long: a tick at each category, labeled below the
    /// axes' box with the category in plain text.
    pub fn with_categories(
        categories: Vec<String>,
        width: f32,
        y: Axis,
        x_label: Option<&str>,
        y_label: Option<&str>,
        appearance: Appearance,
    ) -> Result<Axes, LabelError> {
        let x = Axis::categories(categories.len(), width);
        Axes::drawn(x, Some(categories), y, [x_label, y_label], appearance)
    }

    fn drawn(
        x: Axis,
        categories: Option<Vec<String>>,
        y: Axis,
        [x_label, y_label]: [Option<&str>; 2],
        appearance: Appearance,
    ) -> Result<Axes, LabelError> {
        let mut b = Builder::new(appearance);
        let x_axis_y = y.to_local(y.crossing());
        let y_axis_x = x.to_local(x.crossing());
        b.segment(
            Vec2::new(x.to_local(x.min), x_axis_y),
            Vec2::new(x.to_local(x.max), x_axis_y),
        );
        b.segment(
            Vec2::new(y_axis_x, y.to_local(y.min)),
            Vec2::new(y_axis_x, y.to_local(y.max)),
        );
        // The crossing's two tick labels show once, in the corner below and left of the
        // crossing, when they say the same, as both say 0 where the axes cross at 0. Otherwise
        // a crossing label that the other axis runs through, under the x axis where the y axis
        // runs below or left of the y axis where the x axis runs left, moves to the other side
        // of its own axis, or to the corner when the other axis runs both ways. When the label's
        // own axis starts at the crossing, the other crossing label sits beside the corner, so the
        // label moves along its own axis past the other axis instead: an x label right of the y
        // axis, a y label above the x axis.
        let crossing_text = |axis: &Axis| {
            axis.ticks()
                .any(|value| value == axis.crossing())
                .then(|| axis.label(axis.crossing()))
        };
        let shared = crossing_text(&x).filter(|text| crossing_text(&y).as_ref() == Some(text));
        let tick = |axis: &'static str, value: f64| Labelled::Tick { axis, value };
        let mut corner_label = shared
            .clone()
            .map(|text| (text, vec![tick("x", x.crossing()), tick("y", y.crossing())]));
        let tick_label = |b: &mut Builder,
                          which: &'static str,
                          axis: &Axis,
                          value: f64,
                          align: Align,
                          at: Vec2,
                          side: Side|
         -> Result<(), LabelError> {
            b.label(
                &axis.label(value),
                align,
                at,
                side,
                vec![tick(which, value)],
            )
            .map_err(|error| LabelError { axis: which, error })
        };
        let half_digit = HALF_DIGIT_HEIGHT * appearance.label_size;
        for value in x.ticks() {
            let along = x.to_local(value);
            b.segment(
                Vec2::new(along, x_axis_y - TICK_LENGTH / 2.0),
                Vec2::new(along, x_axis_y + TICK_LENGTH / 2.0),
            );
            let offset = TICK_LENGTH / 2.0 + LABEL_GAP + half_digit;
            if let Some(categories) = &categories {
                // Below the box, not the axis line, so a bar hanging below the axis is clear.
                let bottom = y.to_local(y.min);
                b.category_label(
                    &categories[value as usize],
                    Vec2::new(along, bottom - offset),
                    tick("x", value),
                )
                .map_err(|error| LabelError { axis: "x", error })?;
                continue;
            }
            let at_crossing = value == x.crossing();
            if at_crossing && shared.is_some() {
                continue;
            }
            match (at_crossing && y.min < y.crossing(), y.max > y.crossing()) {
                (false, _) => tick_label(
                    &mut b,
                    "x",
                    &x,
                    value,
                    Align::Center,
                    Vec2::new(along, x_axis_y - offset),
                    Side::Below,
                )?,
                (true, false) => tick_label(
                    &mut b,
                    "x",
                    &x,
                    value,
                    Align::Center,
                    Vec2::new(along, x_axis_y + offset),
                    Side::Above,
                )?,
                (true, true) if x.min == x.crossing() => tick_label(
                    &mut b,
                    "x",
                    &x,
                    value,
                    Align::Left,
                    Vec2::new(y_axis_x + LABEL_GAP, x_axis_y - offset),
                    Side::Below,
                )?,
                (true, true) => corner_label = Some((x.label(value), vec![tick("x", value)])),
            }
        }
        for value in y.ticks() {
            let along = y.to_local(value);
            b.segment(
                Vec2::new(y_axis_x - TICK_LENGTH / 2.0, along),
                Vec2::new(y_axis_x + TICK_LENGTH / 2.0, along),
            );
            let offset = TICK_LENGTH / 2.0 + LABEL_GAP;
            let at_crossing = value == y.crossing();
            if at_crossing && shared.is_some() {
                continue;
            }
            match (at_crossing && x.min < x.crossing(), x.max > x.crossing()) {
                (false, _) => tick_label(
                    &mut b,
                    "y",
                    &y,
                    value,
                    Align::Right,
                    Vec2::new(y_axis_x - offset, along),
                    Side::Left,
                )?,
                (true, false) => tick_label(
                    &mut b,
                    "y",
                    &y,
                    value,
                    Align::Left,
                    Vec2::new(y_axis_x + offset, along),
                    Side::Right,
                )?,
                (true, true) if y.min == y.crossing() => tick_label(
                    &mut b,
                    "y",
                    &y,
                    value,
                    Align::Right,
                    Vec2::new(y_axis_x - offset, x_axis_y + LABEL_GAP + half_digit),
                    Side::Left,
                )?,
                (true, true) => corner_label = Some((y.label(value), vec![tick("y", value)])),
            }
        }
        if let Some((text, of)) = corner_label {
            let corner = Vec2::new(y_axis_x - LABEL_GAP, x_axis_y - LABEL_GAP - half_digit);
            b.label(&text, Align::Right, corner, Side::BelowLeft, of)
                .map_err(|error| LabelError { axis: "x", error })?;
        }
        let axis_label = |axis: &'static str, tex: &str| {
            vec![Labelled::Axis {
                axis,
                label: tex.to_string(),
            }]
        };
        if let Some(tex) = x_label {
            b.label(
                tex,
                Align::Left,
                Vec2::new(x.to_local(x.max) + LABEL_GAP, x_axis_y),
                Side::Right,
                axis_label("x", tex),
            )
            .map_err(|error| LabelError { axis: "x", error })?;
        }
        if let Some(tex) = y_label {
            b.label(
                tex,
                Align::Center,
                Vec2::new(y_axis_x, y.to_local(y.max) + LABEL_GAP + half_digit),
                Side::Above,
                axis_label("y", tex),
            )
            .map_err(|error| LabelError { axis: "y", error })?;
        }
        Ok(Axes {
            x,
            y,
            categories,
            drawing: b.finish(appearance),
        })
    }

    pub fn to_local(&self, data: (f64, f64)) -> Vec2 {
        Vec2::new(self.x.to_local(data.0), self.y.to_local(data.1))
    }

    pub fn to_data(&self, local: Vec2) -> (f64, f64) {
        (self.x.to_data(local.x), self.y.to_data(local.y))
    }

    /// The first coordinate of `data` that its axis cannot place, a value at or below 0 on a log
    /// axis, as the axis' name and the value, or `None` when both axes place theirs.
    pub fn unplaced(&self, (x, y): (f64, f64)) -> Option<(&'static str, f64)> {
        [("x", &self.x, x), ("y", &self.y, y)]
            .into_iter()
            .find(|(_, axis, value)| !axis.places(*value))
            .map(|(name, _, value)| (name, value))
    }

    /// The first two neighboring category labels that come nearer than a quarter of the label
    /// size, which the frame would show run together as one word, or `None`.
    pub fn crowded_categories(&self, label_size: f32) -> Option<(&str, &str)> {
        self.categories.as_ref()?;
        let category_labels: Vec<&Label> = self
            .drawing
            .labels
            .iter()
            .filter(|label| matches!(label.of.as_slice(), [Labelled::Tick { axis: "x", .. }]))
            .collect();
        category_labels.windows(2).find_map(|pair| {
            let gap = pair[1].bounding_box.x0 - pair[0].bounding_box.x1;
            (gap < f64::from(label_size) / 4.0)
                .then(|| (pair[0].text.as_str(), pair[1].text.as_str()))
        })
    }

    /// What an agent needs to put a data point on a frame: the world position of the data zero
    /// and of one data unit along each axis, and every tick, in world units and pixels.
    /// `object_to_world` is the axes' transform, and `to_pixels` is the camera's.
    pub fn report(
        &self,
        object_to_world: Mat4,
        to_pixels: impl Fn(Vec3) -> Option<Vec2>,
    ) -> AxesReport {
        let world =
            |data: (f64, f64)| object_to_world.transform_point3(self.to_local(data).extend(0.0));
        let unit = |axis: &Axis, along: Vec3| {
            object_to_world.transform_vector3(along * axis.unit() as f32)
        };
        let spot = |data| Spot {
            world: world(data),
            pixels: to_pixels(world(data)),
        };
        let ticks = self
            .x
            .ticks()
            .map(|value| ("x", value, (value, self.y.crossing())))
            .chain(
                self.y
                    .ticks()
                    .map(|value| ("y", value, (self.x.crossing(), value))),
            )
            .map(|(axis, value, data)| Tick {
                axis,
                value,
                at: spot(data),
                label: self
                    .drawing
                    .tick_label(axis, value, object_to_world, &to_pixels),
            })
            .collect();
        let axis_report = |which: &'static str, axis: &Axis| AxisReport {
            range: [axis.min, axis.max],
            scale: axis.scale,
            spacing: axis.spacing,
            label_bounding_box: self.drawing.axis_label_bounding_box(
                which,
                object_to_world,
                &to_pixels,
            ),
            categories: (which == "x").then(|| self.categories.clone()).flatten(),
        };
        AxesReport {
            x: axis_report("x", &self.x),
            y: axis_report("y", &self.y),
            zero: spot((self.x.unscaled(0.0), self.y.unscaled(0.0))),
            x_unit: unit(&self.x, Vec3::X),
            y_unit: unit(&self.y, Vec3::Y),
            ticks,
        }
    }
}

impl NumberLine {
    pub fn new(
        axis: Axis,
        label: Option<&str>,
        appearance: Appearance,
    ) -> Result<NumberLine, LabelError> {
        let mut b = Builder::new(appearance);
        b.segment(
            Vec2::new(axis.to_local(axis.min), 0.0),
            Vec2::new(axis.to_local(axis.max), 0.0),
        );
        let half_digit = HALF_DIGIT_HEIGHT * appearance.label_size;
        for value in axis.ticks() {
            let along = axis.to_local(value);
            b.segment(
                Vec2::new(along, -TICK_LENGTH / 2.0),
                Vec2::new(along, TICK_LENGTH / 2.0),
            );
            let of = vec![Labelled::Tick { axis: "x", value }];
            b.label(
                &axis.label(value),
                Align::Center,
                Vec2::new(along, -TICK_LENGTH / 2.0 - LABEL_GAP - half_digit),
                Side::Below,
                of,
            )
            .map_err(|error| LabelError { axis: "", error })?;
        }
        if let Some(tex) = label {
            let of = vec![Labelled::Axis {
                axis: "x",
                label: tex.to_string(),
            }];
            b.label(
                tex,
                Align::Left,
                Vec2::new(axis.to_local(axis.max) + LABEL_GAP, 0.0),
                Side::Right,
                of,
            )
            .map_err(|error| LabelError { axis: "", error })?;
        }
        Ok(NumberLine {
            axis,
            drawing: b.finish(appearance),
        })
    }
}

impl NumberLine {
    /// See `Axes::report`.
    pub fn report(
        &self,
        object_to_world: Mat4,
        to_pixels: impl Fn(Vec3) -> Option<Vec2>,
    ) -> NumberLineReport {
        let world = |value: f64| {
            object_to_world.transform_point3(Vec3::new(self.axis.to_local(value), 0.0, 0.0))
        };
        let spot = |value| Spot {
            world: world(value),
            pixels: to_pixels(world(value)),
        };
        NumberLineReport {
            axis: AxisReport {
                range: [self.axis.min, self.axis.max],
                scale: self.axis.scale,
                spacing: self.axis.spacing,
                label_bounding_box: self.drawing.axis_label_bounding_box(
                    "x",
                    object_to_world,
                    &to_pixels,
                ),
                categories: None,
            },
            zero: spot(0.0),
            unit: object_to_world.transform_vector3(Vec3::X * self.axis.unit() as f32),
            ticks: self
                .axis
                .ticks()
                .map(|value| Tick {
                    axis: "x",
                    value,
                    at: spot(value),
                    label: self
                        .drawing
                        .tick_label("x", value, object_to_world, &to_pixels),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct NumberLineReport {
    pub axis: AxisReport,
    /// The value 0, even where it lies outside the range.
    pub zero: Spot,
    /// The world vector that one unit of value adds.
    pub unit: Vec3,
    pub ticks: Vec<Tick>,
}

#[derive(Debug, Serialize)]
pub struct AxesReport {
    pub x: AxisReport,
    pub y: AxisReport,
    /// The data point (0, 0), the zero of both axes, even where it lies outside the ranges. On a
    /// log axis its coordinate is 1 instead, the power of ten 10^0.
    pub zero: Spot,
    /// The world vector that one data unit along x adds, so data (x, y) is at
    /// `zero.world + x * x_unit + y * y_unit`. On a log axis it is the vector one power of ten
    /// adds, and log10 of the value takes the value's place in the sum.
    pub x_unit: Vec3,
    pub y_unit: Vec3,
    pub ticks: Vec<Tick>,
}

#[derive(Debug, Serialize)]
pub struct AxisReport {
    pub range: [f64; 2],
    /// How the axis lays values along its length: `log`, or left out for `linear`.
    #[serde(skip_serializing_if = "AxisScale::is_linear")]
    pub scale: AxisScale,
    /// The distance between ticks: data units on a linear axis, powers of ten on a log axis.
    pub spacing: f64,
    /// The bounding box of the axis label. Left out when the axis has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label_bounding_box: Option<BoundingBox>,
    /// The categories of a category axis in order, category k at data value k and at the tick
    /// of value k. Left out when the axis shows numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub categories: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct Spot {
    pub world: Vec3,
    /// `None` behind the camera.
    pub pixels: Option<Vec2>,
}

#[derive(Debug, Serialize)]
pub struct Tick {
    pub axis: &'static str,
    pub value: f64,
    #[serde(flatten)]
    pub at: Spot,
    /// Left out when no label shows the tick's value: where both axes run past their crossing
    /// and the two crossing labels differ, the corner shows only the y tick's.
    #[serde(flatten)]
    pub label: Option<TickLabel>,
}

/// The tick label that shows a tick's value. Where the axes cross and both crossing labels say
/// the same, one label in the corner shows both ticks' values, and both ticks report it.
#[derive(Debug, Serialize)]
pub struct TickLabel {
    /// What the label shows, exactly as typeset, such as `-1.5`, `20` or `3.14159265`.
    pub label_text: String,
    pub label_side: Side,
    pub label_bounding_box: BoundingBox,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis(min: f64, max: f64, spacing: Option<f64>, length: f32) -> Axis {
        Axis::new(min, max, AxisScale::Linear, spacing, length).unwrap()
    }

    #[test]
    fn data_lands_in_a_box_centered_on_the_object() {
        let y = axis(0.0, 4.0, None, 4.0);

        assert_eq!(y.to_local(0.0), -2.0);
        assert_eq!(y.to_local(1.0), -1.0);
        assert_eq!(y.to_local(4.0), 2.0);
        assert_eq!(y.to_local(5.0), 3.0, "outside the range, on the same line");
        assert_eq!(y.to_data(-1.0), 1.0);
    }

    #[test]
    fn ticks_sit_on_multiples_of_the_spacing() {
        let x = axis(-1.5, 3.0, Some(1.0), 6.0);

        assert_eq!(x.ticks().collect::<Vec<_>>(), [-1.0, 0.0, 1.0, 2.0, 3.0]);
        assert_eq!(axis(0.0, 1.0, Some(0.25), 1.0).ticks().count(), 5);
    }

    #[test]
    fn the_default_spacing_is_a_round_value_giving_about_five_ticks() {
        assert_eq!(axis(-3.0, 3.0, None, 6.0).spacing, 1.0);
        assert_eq!(axis(0.0, 9.0, None, 4.0).spacing, 2.0);
        assert_eq!(axis(0.0, 100.0, None, 4.0).spacing, 20.0);
        assert_eq!(axis(0.0, 1.0, None, 4.0).spacing, 0.2);
    }

    #[test]
    fn labels_have_the_decimals_of_the_spacing_and_no_negative_zero() {
        let x = axis(-1.0, 1.0, Some(0.5), 4.0);

        assert_eq!(
            x.ticks().map(|v| x.label(v)).collect::<Vec<_>>(),
            ["-1.0", "-0.5", "0.0", "0.5", "1.0"]
        );
        assert_eq!(axis(-10.0, 10.0, Some(5.0), 4.0).label(-0.0), "0");
        assert_eq!(
            axis(0.0, 1.0, Some(0.1), 1.0).label(0.30000000000000004),
            "0.3"
        );
    }

    /// No tick label may lie across an axis line or run into another label, wherever the axes
    /// cross and whatever their axis scales, and the crossing's value still shows once.
    #[test]
    fn no_tick_label_lies_across_an_axis_or_another_label() {
        let appearance = Appearance {
            stroke: Stroke::default(),
            color: Color {
                r: 255,
                g: 255,
                b: 255,
            },
            label_size: 0.3,
        };
        let linear = |min, max, spacing, length| axis(min, max, Some(spacing), length);
        let log = |min, max, length| Axis::new(min, max, AxisScale::Log, None, length).unwrap();
        for (x, y) in [
            (linear(0.0, 390.0, 90.0, 8.0), linear(-1.2, 1.2, 1.0, 5.0)),
            (linear(-3.0, 3.0, 1.0, 8.0), linear(-2.0, 2.0, 1.0, 5.0)),
            (linear(2.0, 5.0, 1.0, 8.0), linear(-1.0, 1.0, 1.0, 5.0)),
            (linear(-3.0, 3.0, 1.0, 8.0), linear(2.0, 5.0, 1.0, 5.0)),
            (linear(-5.0, -1.0, 1.0, 8.0), linear(-3.0, 3.0, 1.0, 5.0)),
            (linear(0.0, 4.0, 1.0, 8.0), linear(0.0, 4.0, 1.0, 5.0)),
            (log(1e-2, 1e2, 8.0), linear(-1.2, 1.2, 1.0, 5.0)),
            (linear(-2.0, 2.0, 1.0, 8.0), log(1.0, 1e4, 5.0)),
            (linear(0.0, 10.0, 2.0, 8.0), log(1e-6, 1.0, 5.0)),
            (log(1e-2, 1e2, 6.0), log(1e-4, 1e4, 6.0)),
        ] {
            let axes = Axes::new(x, y, None, None, appearance).unwrap();
            let (x_axis_y, y_axis_x) = (
                f64::from(y.to_local(y.crossing())),
                f64::from(x.to_local(x.crossing())),
            );
            let (x_span, y_span) = (
                (f64::from(x.to_local(x.min)), f64::from(x.to_local(x.max))),
                (f64::from(y.to_local(y.min)), f64::from(y.to_local(y.max))),
            );
            let labels = &axes.drawing.labels;
            for glyph in labels.iter().flat_map(|label| &label.glyphs) {
                let b = glyph.path.bounding_box();
                let across_x_axis =
                    b.y0 < x_axis_y && x_axis_y < b.y1 && b.x1 > x_span.0 && b.x0 < x_span.1;
                let across_y_axis =
                    b.x0 < y_axis_x && y_axis_x < b.x1 && b.y1 > y_span.0 && b.y0 < y_span.1;
                assert!(
                    !across_x_axis && !across_y_axis,
                    "x {x:?}, y {y:?}: a label glyph at {b:?} lies across an axis"
                );
            }
            for (k, a) in labels.iter().enumerate() {
                for b in &labels[k + 1..] {
                    let gap = a.bounding_box.inflate(0.02, 0.02).intersect(b.bounding_box);
                    assert!(
                        gap.width() <= 0.0 || gap.height() <= 0.0,
                        "x {x:?}, y {y:?}: the labels {} and {} run into each other",
                        a.text,
                        b.text
                    );
                }
            }
            let zeros = labels
                .iter()
                .flat_map(|label| &label.glyphs)
                .filter(|glyph| {
                    glyph
                        .path
                        .bounding_box()
                        .center()
                        .distance((y_axis_x, x_axis_y).into())
                        < 0.4
                })
                .count();
            if x.min <= 0.0 && y.min <= 0.0 && 0.0 <= x.max && 0.0 <= y.max {
                assert_eq!(
                    zeros, 1,
                    "x {x:?}, y {y:?}: the axes cross at 0, and one 0 shows it"
                );
            }
        }
    }

    #[test]
    fn a_bad_range_or_spacing_is_refused() {
        assert_eq!(
            Axis::new(1.0, 1.0, AxisScale::Linear, None, 1.0).unwrap_err(),
            AxisError::EmptyRange
        );
        assert_eq!(
            Axis::new(2.0, 1.0, AxisScale::Linear, None, 1.0).unwrap_err(),
            AxisError::EmptyRange
        );
        assert_eq!(
            Axis::new(f64::NAN, 1.0, AxisScale::Linear, None, 1.0).unwrap_err(),
            AxisError::EmptyRange
        );
        assert_eq!(
            Axis::new(0.0, 1000.0, AxisScale::Linear, Some(1.0), 1.0).unwrap_err(),
            AxisError::TooManyTicks(1001)
        );
    }

    #[test]
    fn a_log_axis_ticks_each_power_of_ten_evenly_and_labels_it_as_a_power() {
        let y = Axis::new(1e-6, 1.0, AxisScale::Log, None, 6.0).unwrap();

        assert_eq!(
            y.ticks().collect::<Vec<_>>(),
            [1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1, 1.0]
        );
        assert_eq!(
            y.ticks().map(|v| y.label(v)).collect::<Vec<_>>(),
            [
                "10^{-6}", "10^{-5}", "10^{-4}", "10^{-3}", "10^{-2}", "10^{-1}", "10^{0}"
            ]
        );
        let along: Vec<f32> = y.ticks().map(|v| y.to_local(v)).collect();
        for (local, expected) in along.iter().zip([-3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0]) {
            assert!((local - expected).abs() < 1e-6, "{along:?}");
        }
        assert!(
            (y.to_local(0.002) - 2f32.log10()).abs() < 1e-5,
            "0.002 lies log10(2) of a power of ten past 10^-3, at the middle of the axis"
        );
        assert!((y.to_data(1.0) - 0.01).abs() < 1e-12);
    }

    #[test]
    fn a_log_axis_over_many_powers_of_ten_ticks_every_few() {
        let y = Axis::new(1e-30, 1.0, AxisScale::Log, None, 6.0).unwrap();
        assert_eq!(y.spacing(), 5.0);
        assert_eq!(
            y.ticks().map(|v| y.label(v)).collect::<Vec<_>>(),
            [
                "10^{-30}", "10^{-25}", "10^{-20}", "10^{-15}", "10^{-10}", "10^{-5}", "10^{0}"
            ]
        );
        let every_other = Axis::new(1.0, 1e4, AxisScale::Log, Some(2.0), 4.0).unwrap();
        assert_eq!(every_other.ticks().collect::<Vec<_>>(), [1.0, 1e2, 1e4]);
    }

    #[test]
    fn a_log_axis_refuses_a_range_reaching_0_and_a_fraction_of_a_power() {
        for low in [0.0, -1.0] {
            assert_eq!(
                Axis::new(low, 10.0, AxisScale::Log, None, 1.0).unwrap_err(),
                AxisError::NotAboveZero
            );
        }
        assert_eq!(
            Axis::new(1.0, 100.0, AxisScale::Log, Some(0.5), 1.0).unwrap_err(),
            AxisError::NotWholePowers
        );
        let axes = Axes::new(
            Axis::new(1.0, 10.0, AxisScale::Log, None, 1.0).unwrap(),
            Axis::new(-1.0, 1.0, AxisScale::Linear, None, 1.0).unwrap(),
            None,
            None,
            Appearance {
                stroke: Stroke::default(),
                color: Color { r: 0, g: 0, b: 0 },
                label_size: 0.3,
            },
        )
        .unwrap();
        assert_eq!(axes.unplaced((5.0, -1.0)), None, "y is linear");
        assert_eq!(axes.unplaced((0.0, 0.5)), Some(("x", 0.0)));
        assert_eq!(axes.unplaced((-2.0, 0.5)), Some(("x", -2.0)));
    }
}
