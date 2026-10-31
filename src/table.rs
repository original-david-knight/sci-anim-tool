//! A table: text and math in cells, in rows and columns, with rulings between them, in the
//! table's own space and centered on its position.
//!
//! Each cell is typeset on its own when the scene file is read, then placed: every column is as
//! wide as its widest cell and every row as tall as its tallest, plus the cell padding on every
//! side. A write draws the rulings along their length, then writes every cell's glyphs in reading
//! order, as one text object's.

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;

use glam::{Mat4, Vec2, Vec3};
use kurbo::{BezPath, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::group::Glow;
use crate::mesh::{BoundingBox, corners};
use crate::path::Path;
use crate::scene::{Align, Glyph, Positive, Stroke};
use crate::typeset::Typeset;

/// How opaque a flat glow, a lit cell's, bar's or flat copy's, is at glow share 1: enough to see at a glance, and little enough
/// that white text on it keeps a contrast of 4.5 on a dark background in the default color.
pub const FLAT_GLOW_OPACITY: f32 = 0.25;

/// The share of a write that draws the rulings, before the cells are written.
const RULINGS_SHARE: f32 = 0.2;

/// Which rulings a table draws: between every row and column and around the table, once under
/// the first row, or none. No variant doc comments, for the reason `FillRule` gives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Rulings {
    #[default]
    All,
    Header,
    None,
}

/// A cell's row and column, each counted from 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CellAddress {
    pub row: usize,
    pub column: usize,
}

impl fmt::Display for CellAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.row, self.column)
    }
}

impl Serialize for CellAddress {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        [self.row, self.column].serialize(serializer)
    }
}

/// What a cell holds, as the scene file wrote it.
#[derive(Clone, Debug, PartialEq)]
pub enum CellContent {
    Empty,
    Text(String),
    Math(String),
}

/// One cell: its glyphs in the table's glyphs, and its rectangle in the table's own space.
#[derive(Clone, Debug)]
pub struct Cell {
    pub address: CellAddress,
    pub content: CellContent,
    pub glyphs: Range<usize>,
    pub rectangle: Rect,
}

#[derive(Clone, Debug)]
pub struct Table {
    /// Every cell's glyphs, in reading order, placed in the table's own space.
    pub glyphs: Vec<Glyph>,
    /// Every cell, in reading order.
    pub cells: Vec<Cell>,
    pub rows: usize,
    pub columns: usize,
    /// `None` when the table draws none.
    pub rulings: Option<Path>,
    pub stroke: Stroke,
    /// The text size, which sets a write's pen width.
    pub size: Positive,
    /// The glow of each cell that a highlight has touched. A cell not listed has none. The
    /// timeline's tracks replace the values here at every scene time.
    pub glows: BTreeMap<CellAddress, Glow>,
}

impl Table {
    /// Places each row's typeset cells, `None` for an empty one. Every row holds as many cells
    /// as the first, and there is at least one: `scene_file` has checked both. Each cell was
    /// typeset with `align`.
    pub fn new(
        rows: Vec<Vec<(CellContent, Option<Typeset>)>>,
        align: Align,
        padding: f32,
        rulings: Rulings,
        stroke: Stroke,
        size: Positive,
    ) -> Table {
        let columns = rows[0].len();
        let box_of =
            |set: &Option<Typeset>| set.as_ref().map_or((0.0, 0.0), |s| (s.width, s.height));
        let widths: Vec<f32> = (0..columns)
            .map(|c| {
                rows.iter()
                    .map(|row| box_of(&row[c].1).0)
                    .fold(0.0, f32::max)
                    + 2.0 * padding
            })
            .collect();
        let heights: Vec<f32> = rows
            .iter()
            .map(|row| row.iter().map(|cell| box_of(&cell.1).1).fold(0.0, f32::max) + 2.0 * padding)
            .collect();
        let (width, height): (f32, f32) = (widths.iter().sum(), heights.iter().sum());
        let lefts: Vec<f32> = running(-width / 2.0, &widths, 1.0);
        let tops: Vec<f32> = running(height / 2.0, &heights, -1.0);
        let row_count = rows.len();
        let mut glyphs = Vec::new();
        let mut cells = Vec::new();
        for (r, row) in rows.into_iter().enumerate() {
            for (c, (content, set)) in row.into_iter().enumerate() {
                let rectangle = Rect::new(
                    f64::from(lefts[c]),
                    f64::from(tops[r] - heights[r]),
                    f64::from(lefts[c] + widths[c]),
                    f64::from(tops[r]),
                );
                let aligned_at = Vec2::new(
                    match align {
                        Align::Left => lefts[c] + padding,
                        Align::Center => lefts[c] + widths[c] / 2.0,
                        Align::Right => lefts[c] + widths[c] - padding,
                    },
                    tops[r] - heights[r] / 2.0,
                );
                let first = glyphs.len();
                glyphs.extend(
                    set.into_iter()
                        .flat_map(|set| set.glyphs)
                        .map(|glyph| Glyph {
                            path: glyph.path.moved(aligned_at),
                            ..glyph
                        }),
                );
                cells.push(Cell {
                    address: CellAddress { row: r, column: c },
                    content,
                    glyphs: first..glyphs.len(),
                    rectangle,
                });
            }
        }
        let (half_width, half_height) = (f64::from(width) / 2.0, f64::from(height) / 2.0);
        let mut path = BezPath::new();
        let mut rule = |from: (f64, f64), to: (f64, f64)| {
            path.move_to(from);
            path.line_to(to);
        };
        match rulings {
            Rulings::All => {
                for y in tops.iter().map(|&y| f64::from(y)).chain([-half_height]) {
                    rule((-half_width, y), (half_width, y));
                }
                for x in lefts.iter().map(|&x| f64::from(x)).chain([half_width]) {
                    rule((x, half_height), (x, -half_height));
                }
            }
            Rulings::Header => {
                let y = f64::from(tops[0] - heights[0]);
                rule((-half_width, y), (half_width, y));
            }
            Rulings::None => {}
        }
        Table {
            glyphs,
            cells,
            rows: row_count,
            columns,
            rulings: Path::from_bez(path).ok(),
            stroke,
            size,
            glows: BTreeMap::new(),
        }
    }

    pub fn cell(&self, address: CellAddress) -> Option<&Cell> {
        (address.row < self.rows && address.column < self.columns)
            .then(|| &self.cells[address.row * self.columns + address.column])
    }

    /// How far a write at `created` has drawn the rulings and written the cells: the rulings over
    /// the first fifth, the cells over the rest, or over all of it when there are no rulings.
    pub fn written_shares(&self, created: f32) -> (f32, f32) {
        if self.rulings.is_none() {
            return (0.0, created);
        }
        (
            (created / RULINGS_SHARE).min(1.0),
            ((created - RULINGS_SHARE) / (1.0 - RULINGS_SHARE)).max(0.0),
        )
    }

    /// The whole table's box in its own space, rulings or not.
    pub fn rectangle(&self) -> Rect {
        let first = self.cells[0].rectangle;
        let last = self.cells[self.cells.len() - 1].rectangle;
        Rect::new(first.x0, last.y0, last.x1, first.y1)
    }

    /// How far its farthest corner lies from its position.
    pub fn reach(&self) -> f32 {
        let r = self.rectangle();
        r.x1.hypot(r.y1) as f32 + self.stroke.width.get() / 2.0
    }
}

/// Where each of `lengths` starts, laid end to end from `from` in `direction`.
fn running(from: f32, lengths: &[f32], direction: f32) -> Vec<f32> {
    lengths
        .iter()
        .scan(from, |at, length| {
            let start = *at;
            *at += direction * length;
            Some(start)
        })
        .collect()
}

/// What `inspect` tells an agent about a table: its text size, its row and column counts, and
/// each cell in reading order.
#[derive(Debug, Serialize)]
pub struct TableReport {
    pub size: f32,
    pub rows: usize,
    pub columns: usize,
    pub cells: Vec<CellReport>,
}

/// One cell: its address, its text or math, how far a write has written it, its glow, its
/// rectangle, and the box of its glyphs, none for an empty cell.
#[derive(Debug, Serialize)]
pub struct CellReport {
    pub cell: CellAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tex: Option<String>,
    pub created: f32,
    pub glow: f32,
    pub glow_color: String,
    pub rectangle: BoundingBox,
    pub bounding_box: Option<BoundingBox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colors: Option<crate::ColorsReport>,
}

/// A rectangle of the table's own space in the world and on the frame.
pub fn rectangle_report(
    r: Rect,
    object_to_world: Mat4,
    to_pixels: impl Fn(Vec3) -> Option<Vec2>,
) -> BoundingBox {
    let world: Vec<Vec3> = [(r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)]
        .into_iter()
        .map(|(x, y)| object_to_world.transform_point3(Vec3::new(x as f32, y as f32, 0.0)))
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
}
