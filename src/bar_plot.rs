//! A bar plot: one bar for each category of its axes' category axis, standing on the x axis and
//! reaching its value, in the axes' own space.
//!
//! A bar is a rectangle cut at the axes' box, as a curve is. A create grows the bars from the x
//! axis one at a time, in category order.

use glam::{Mat4, Vec2, Vec3};
use kurbo::{BezPath, Rect};
use serde::Serialize;

use crate::axes::Axes;
use crate::group::Glow;
use crate::member::OwnColors;
use crate::mesh::{BoundingBox, corners};
use crate::path::Path;
use crate::scene::{Fill, ObjectId, Stroke};

/// How much of its category's width a bar fills, so neighbors stand apart.
const BAR_SHARE: f32 = 0.7;

#[derive(Clone, Debug)]
pub struct BarPlot {
    pub bars: Vec<Bar>,
    pub fill: Fill,
    pub stroke: Option<Stroke>,
}

/// One bar in the axes' own space: its sides, the x axis it stands on and the end it reaches,
/// which lies above the x axis for a value above it and below for a value below.
#[derive(Clone, Debug)]
pub struct Bar {
    pub category: String,
    pub value: f64,
    left: f32,
    right: f32,
    foot: f32,
    reach: f32,
    /// The colors a recolor of the bar gave it, drawn in place of the plot's.
    pub own_colors: OwnColors,
    pub glow: Glow,
}

#[derive(Debug, PartialEq)]
pub enum BarPlotError {
    /// The axes' x axis shows numbers, not categories.
    NoCategories,
    /// The plot gives `values` values for `categories` categories.
    ValueCount { values: usize, categories: usize },
}

impl BarPlot {
    pub fn new(
        axes: &Axes,
        values: &[f64],
        fill: Fill,
        stroke: Option<Stroke>,
    ) -> Result<BarPlot, BarPlotError> {
        let categories = axes.categories.as_ref().ok_or(BarPlotError::NoCategories)?;
        if values.len() != categories.len() {
            return Err(BarPlotError::ValueCount {
                values: values.len(),
                categories: categories.len(),
            });
        }
        let (low, high) = axes.y.range();
        let foot = axes.y.to_local(axes.y.crossing());
        let half = (axes.x.to_local(1.0) - axes.x.to_local(0.0)) * BAR_SHARE / 2.0;
        let bars = categories
            .iter()
            .zip(values)
            .enumerate()
            .map(|(k, (category, &value))| {
                let center = axes.x.to_local(k as f64);
                Bar {
                    category: category.clone(),
                    value,
                    left: center - half,
                    right: center + half,
                    foot,
                    reach: axes.y.to_local(value.clamp(low, high)),
                    own_colors: OwnColors::default(),
                    glow: Glow::off(),
                }
            })
            .collect();
        Ok(BarPlot { bars, fill, stroke })
    }

    /// Each bar's own created share at the plot's created share, in category order. Each bar
    /// takes the same share of the create and starts when the one before it is whole, so the
    /// bars grow one at a time.
    pub fn created_shares(&self, created: f32) -> impl Iterator<Item = (&Bar, f32)> {
        let count = self.bars.len() as f32;
        self.bars.iter().enumerate().map(move |(k, bar)| {
            let share = if created >= 1.0 {
                1.0
            } else {
                (created * count - k as f32).clamp(0.0, 1.0)
            };
            (bar, share)
        })
    }

    /// How far the farthest corner of a bar lies from the axes' center.
    pub fn reach(&self) -> f32 {
        self.bars
            .iter()
            .filter_map(|bar| bar.rectangle(1.0))
            .flat_map(|r| [(r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)])
            .map(|(x, y)| x.hypot(y) as f32)
            .fold(0.0, f32::max)
            + self.stroke.map_or(0.0, |s| s.width.get() / 2.0)
    }
}

impl BarPlot {
    /// The fill and stroke `bar` draws: the plot's, or the bar's own colors in their place.
    pub fn paint(&self, bar: &Bar) -> (Fill, Option<Stroke>) {
        let fill = Fill {
            color: bar.own_colors.fill.unwrap_or(self.fill.color),
            ..self.fill
        };
        let stroke = self.stroke.map(|stroke| Stroke {
            color: bar.own_colors.stroke.unwrap_or(stroke.color),
            ..stroke
        });
        (fill, stroke)
    }
}

impl Bar {
    /// The glow of a lit bar: a frame around the whole bar but its foot on the x axis, as wide as
    /// half the gap between two bars, so it fills its category's width and lights the space around
    /// the bar without tinting it, filled by the even-odd rule. `None` while the bar has no height.
    pub fn glow_path(&self) -> Option<Path> {
        let bar = self.rectangle(1.0)?;
        let spread =
            f64::from(self.right - self.left) * f64::from((1.0 - BAR_SHARE) / BAR_SHARE) / 2.0;
        let mut path = BezPath::new();
        let (below, above) = if self.reach >= self.foot {
            (0.0, spread)
        } else {
            (spread, 0.0)
        };
        let around = Rect::new(
            bar.x0 - spread,
            bar.y0 - below,
            bar.x1 + spread,
            bar.y1 + above,
        );
        for r in [around, bar] {
            path.move_to((r.x0, r.y0));
            path.line_to((r.x1, r.y0));
            path.line_to((r.x1, r.y1));
            path.line_to((r.x0, r.y1));
            path.close_path();
        }
        Path::from_bez(path).ok()
    }

    /// The bar grown `share` of the way from the x axis to its end, or `None` while it has no
    /// height: before it starts to grow, or when its value lies on the x axis or past it outside
    /// the box.
    pub fn rectangle(&self, share: f32) -> Option<Rect> {
        let end = self.foot + (self.reach - self.foot) * share.clamp(0.0, 1.0);
        (end != self.foot).then(|| {
            Rect::new(
                f64::from(self.left),
                f64::from(self.foot.min(end)),
                f64::from(self.right),
                f64::from(self.foot.max(end)),
            )
        })
    }

    pub fn path(&self, share: f32) -> Option<Path> {
        let rectangle = self.rectangle(share)?;
        let mut path = BezPath::new();
        path.move_to((rectangle.x0, rectangle.y0));
        path.line_to((rectangle.x1, rectangle.y0));
        path.line_to((rectangle.x1, rectangle.y1));
        path.line_to((rectangle.x0, rectangle.y1));
        path.close_path();
        Path::from_bez(path).ok()
    }
}

/// What `inspect` tells an agent about a bar plot: its axes, and each bar in category order.
#[derive(Debug, Serialize)]
pub struct BarPlotReport {
    pub axes: ObjectId,
    pub bars: Vec<BarReport>,
}

#[derive(Debug, Serialize)]
pub struct BarReport {
    pub category: String,
    pub value: f64,
    /// How far the create has grown this bar, from 0 to 1.
    pub created: f32,
    /// The whole bar, as cut at the axes' box, in world units and pixels, at any created share.
    /// `None` when it has no height: its value lies on the x axis, or past it outside the box.
    pub rectangle: Option<BoundingBox>,
    /// The bar's fill color, and its stroke color when the plot has a stroke, written `#rrggbb`:
    /// the plot's, or the bar's own after a recolor of the bar.
    pub fill: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    /// The glow share of the bar's glow, and its color.
    pub glow: f32,
    pub glow_color: String,
}

pub fn report(
    axes_id: &ObjectId,
    bar_plot: &BarPlot,
    created: f32,
    object_to_world: Mat4,
    to_pixels: impl Fn(Vec3) -> Option<Vec2>,
) -> BarPlotReport {
    let bars = bar_plot
        .created_shares(created)
        .map(|(bar, created)| {
            let rectangle = bar.rectangle(1.0).map(|r| {
                let world: Vec<Vec3> = [(r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)]
                    .into_iter()
                    .map(|(x, y)| {
                        object_to_world.transform_point3(Vec3::new(x as f32, y as f32, 0.0))
                    })
                    .collect();
                BoundingBox {
                    pixels: corners(
                        world.iter().filter_map(|&corner| to_pixels(corner)),
                        Vec2::min,
                        Vec2::max,
                    ),
                    world: corners(world.into_iter(), Vec3::min, Vec3::max)
                        .expect("a rectangle has four corners"),
                }
            });
            let (fill, stroke) = bar_plot.paint(bar);
            BarReport {
                category: bar.category.clone(),
                value: bar.value,
                created,
                rectangle,
                fill: fill.color.hex(),
                stroke: stroke.map(|s| s.color.hex()),
                glow: bar.glow.share.get(),
                glow_color: bar.glow.color.hex(),
            }
        })
        .collect();
    BarPlotReport {
        axes: axes_id.clone(),
        bars,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::axes::{Appearance, Axis, Side};
    use crate::scene::Color;

    const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
    };

    /// Four categories over 4 world units, so each is 1 wide, and y from -10 to 10 over 4.
    fn axes() -> Axes {
        let appearance = Appearance {
            stroke: Stroke::default(),
            color: WHITE,
            label_size: 0.3,
        };
        let names = ["A", "B", "C", "D"].map(String::from).to_vec();
        let y = Axis::new(-10.0, 10.0, Some(5.0), 4.0).unwrap();
        Axes::with_categories(names, 4.0, y, None, None, appearance).unwrap()
    }

    fn bars(values: &[f64]) -> BarPlot {
        BarPlot::new(&axes(), values, Fill::solid(WHITE), None).unwrap()
    }

    fn corners(rectangle: Option<Rect>) -> Option<[f64; 4]> {
        rectangle.map(|r| [r.x0, r.y0, r.x1, r.y1].map(|v| (v * 1e4).round() / 1e4))
    }

    #[test]
    fn a_bar_stands_on_the_x_axis_at_its_category_and_reaches_its_value() {
        let plot = bars(&[5.0, -5.0, 20.0, 0.0]);

        let whole: Vec<Option<[f64; 4]>> = plot
            .bars
            .iter()
            .map(|bar| corners(bar.rectangle(1.0)))
            .collect();

        assert_eq!(
            whole,
            [
                Some([-1.85, 0.0, -1.15, 1.0]),
                Some([-0.85, -1.0, -0.15, 0.0]),
                Some([0.15, 0.0, 0.85, 2.0]),
                None,
            ],
            "5 rises to 1, -5 hangs to -1, 20 is cut at the box's top at 2, and 0 has no height"
        );
        assert_eq!(
            corners(plot.bars[0].rectangle(0.5)),
            Some([-1.85, 0.0, -1.15, 0.5])
        );
    }

    #[test]
    fn a_create_grows_the_bars_one_at_a_time_in_category_order() {
        let plot = bars(&[1.0, 2.0, 3.0, 4.0]);
        let shares = |created: f32| -> Vec<f32> {
            plot.created_shares(created)
                .map(|(_, share)| share)
                .collect()
        };

        assert_eq!(shares(0.0), [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(shares(0.375), [1.0, 0.5, 0.0, 0.0]);
        assert_eq!(shares(0.75), [1.0, 1.0, 1.0, 0.0]);
        assert_eq!(shares(1.0), [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn a_bar_plot_needs_a_category_axis_and_one_value_for_each_category() {
        let appearance = Appearance {
            stroke: Stroke::default(),
            color: WHITE,
            label_size: 0.3,
        };
        let numbers = Axes::new(
            Axis::new(0.0, 4.0, None, 4.0).unwrap(),
            Axis::new(0.0, 4.0, None, 4.0).unwrap(),
            None,
            None,
            appearance,
        )
        .unwrap();

        assert_eq!(
            BarPlot::new(&numbers, &[1.0], Fill::solid(WHITE), None).unwrap_err(),
            BarPlotError::NoCategories
        );
        assert_eq!(
            BarPlot::new(&axes(), &[1.0, 2.0, 3.0], Fill::solid(WHITE), None).unwrap_err(),
            BarPlotError::ValueCount {
                values: 3,
                categories: 4
            }
        );
    }

    #[test]
    fn labels_wider_than_their_categories_are_crowded() {
        let appearance = Appearance {
            stroke: Stroke::default(),
            color: WHITE,
            label_size: 0.3,
        };
        let years = ["1950", "1960", "1970", "1980"].map(String::from).to_vec();
        let y = || Axis::new(0.0, 1.0, None, 4.0).unwrap();
        let narrow = Axes::with_categories(years.clone(), 2.0, y(), None, None, appearance);
        let wide = Axes::with_categories(years, 8.0, y(), None, None, appearance);

        assert_eq!(
            narrow.unwrap().crowded_categories(0.3),
            Some(("1950", "1960")),
            "four digits at 0.3 are wider than a category half a unit wide"
        );
        assert_eq!(wide.unwrap().crowded_categories(0.3), None);
    }

    #[test]
    fn each_category_is_labeled_below_the_box_and_the_y_axis_stands_at_the_low_end() {
        let axes = axes();

        let below: Vec<(&str, Side)> = axes
            .drawing
            .labels
            .iter()
            .filter(|label| label.bounding_box.y1 < -2.0)
            .map(|label| (label.text.as_str(), label.side))
            .collect();

        assert_eq!(
            below,
            [
                ("A", Side::Below),
                ("B", Side::Below),
                ("C", Side::Below),
                ("D", Side::Below)
            ],
            "each label lies below the box's bottom at y = -2, under the bar hanging lowest"
        );
        assert_eq!(axes.crowded_categories(0.3), None);
        assert_eq!(axes.x.to_local(axes.x.crossing()), -2.0);
        assert_eq!(axes.x.ticks().collect::<Vec<_>>(), [0.0, 1.0, 2.0, 3.0]);
    }
}
