//! A readout: a number typeset as math with a unit after it, whose value is a track like any
//! other. The digits are typeset once, when the scene file is read, with how far each moves the
//! next glyph, and so is each layout of a number: its sign, digits, point and unit, with every
//! digit a 0. A number is its layout with each 0 swapped for its digit, and every glyph after a
//! digit moved by how much wider or narrower that digit is than a 0. So a number is typeset at
//! most once per sign and count of digits, and its baseline never moves, where a number typeset
//! whole sits higher or lower as its digits change how tall it is. The number is lined up on its
//! decimal point, or on the end of its last digit with no decimals, at the object's position, so
//! its digits keep their places as it grows.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use glam::Vec2;
use serde::Serialize;

use crate::scene::{Align, Color, Glyph, Positive};
use crate::typeset::{self, Layout, TypesetError};

/// How many decimals a readout may show.
pub const MOST_DECIMALS: u8 = 6;

#[derive(Debug)]
pub struct Readout {
    /// The value the scene file declares, which a readout shows until an animation changes it.
    pub value: f64,
    pub decimals: u8,
    /// LaTeX math set after the number, such as `\text{ m/s}`.
    pub unit: String,
    pub size: Positive,
    pub color: Color,
    /// Each digit from 0 to 9, placed as a 0 whose lower left corner is at the origin.
    digits: Vec<Digit>,
    /// The glyphs of each layout typeset so far, by the number's text with every digit a 0.
    layouts: Mutex<BTreeMap<String, Arc<[Glyph]>>>,
}

#[derive(Debug)]
struct Digit {
    glyph: Glyph,
    /// How much farther than a 0 the digit moves the glyph after it.
    wider: f32,
}

impl Readout {
    /// Typesets the digits and the layout of the declared value, so a unit that cannot be
    /// typeset is found when the scene file is read.
    pub fn new(
        value: f64,
        decimals: u8,
        unit: String,
        size: Positive,
        color: Color,
    ) -> Result<Readout, TypesetError> {
        // Each digit between two 0s: the 0s give where the digit starts and how far it moves the
        // next glyph.
        let line = typeset::math(
            "00102030405060708090",
            color,
            Layout {
                size: size.get(),
                align: Align::Left,
            },
        )?;
        let corner = |glyph: &Glyph| {
            let corner = glyph.path.bounding_box();
            Vec2::new(corner.x0 as f32, corner.y0 as f32)
        };
        let zero = corner(&line[0]);
        let advance = corner(&line[1]).x - zero.x;
        let digits = (0..10)
            .map(|d| {
                if d == 0 {
                    return Digit {
                        glyph: moved(&line[0], -zero),
                        wider: 0.0,
                    };
                }
                let starts = Vec2::new(corner(&line[2 * d - 1]).x + advance, zero.y);
                Digit {
                    glyph: moved(&line[2 * d], -starts),
                    wider: corner(&line[2 * d + 1]).x - starts.x - advance,
                }
            })
            .collect();
        let readout = Readout {
            value,
            decimals,
            unit,
            size,
            color,
            digits,
            layouts: Mutex::new(BTreeMap::new()),
        };
        readout.layout(&readout.text(value))?;
        Ok(readout)
    }

    /// The number as the readout shows it, to its decimals, with no minus sign before a zero.
    pub fn text(&self, value: f64) -> String {
        let text = format!("{value:.*}", usize::from(self.decimals));
        match text.strip_prefix('-') {
            Some(magnitude) if magnitude.chars().all(|c| c == '0' || c == '.') => {
                magnitude.to_string()
            }
            _ => text,
        }
    }

    /// The glyphs that show `value`: its layout with each 0 swapped for its digit, lined up.
    pub fn glyphs(&self, value: f64) -> Vec<Glyph> {
        let text = self.text(value);
        // `new` typeset a layout with this unit, and every other layout only adds or drops digits.
        let layout = self
            .layout(&text)
            .expect("the unit was typeset when the scene file was read");
        let mut wider = 0.0;
        let mut chars = text.chars();
        let glyphs = layout
            .iter()
            .map(|slot| {
                let by = Vec2::new(wider, 0.0);
                match chars.next().and_then(|c| c.to_digit(10)) {
                    Some(d) => {
                        let digit = &self.digits[d as usize];
                        wider += digit.wider;
                        let corner = slot.path.bounding_box();
                        moved(
                            &digit.glyph,
                            Vec2::new(corner.x0 as f32, corner.y0 as f32) + by,
                        )
                    }
                    None => moved(slot, by),
                }
            })
            .collect();
        lined_up(glyphs, &text, self.decimals)
    }

    /// The glyphs of the layout of the number `text`, typeset the first time it is asked for.
    fn layout(&self, text: &str) -> Result<Arc<[Glyph]>, TypesetError> {
        let zeros: String = text
            .chars()
            .map(|c| if c.is_ascii_digit() { '0' } else { c })
            .collect();
        if let Some(layout) = self
            .layouts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&zeros)
        {
            return Ok(Arc::clone(layout));
        }
        let layout: Arc<[Glyph]> = Arc::from(typeset::math(
            &format!("{zeros}{}", self.unit),
            self.color,
            Layout {
                size: self.size.get(),
                align: Align::Left,
            },
        )?);
        self.layouts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(zeros, Arc::clone(&layout));
        Ok(layout)
    }
}

fn moved(glyph: &Glyph, by: Vec2) -> Glyph {
    Glyph {
        path: glyph.path.moved(by),
        color: glyph.color,
        characters: glyph.characters.clone(),
    }
}

/// `glyphs` moved so the decimal point of the number `text`, or the end of its last digit, sits
/// at the origin. Each character of a number is one glyph in New Computer Modern Math, in order.
fn lined_up(glyphs: Vec<Glyph>, text: &str, decimals: u8) -> Vec<Glyph> {
    let point_at = match text.chars().position(|c| c == '.').filter(|_| decimals > 0) {
        Some(point) => {
            let point = glyphs[point].path.bounding_box();
            (point.x0 + point.x1) / 2.0
        }
        None => glyphs[text.chars().count() - 1].path.bounding_box().x1,
    };
    let by = Vec2::new(-point_at as f32, 0.0);
    glyphs.iter().map(|glyph| moved(glyph, by)).collect()
}

/// What `inspect` says of a readout: its value at the scene time, and the number it shows.
#[derive(Debug, Serialize)]
pub struct ReadoutReport {
    pub value: f64,
    pub text: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn readout(decimals: u8, unit: &str) -> Readout {
        Readout::new(
            0.0,
            decimals,
            unit.to_string(),
            Positive::try_from(1.0).unwrap(),
            Color {
                r: 255,
                g: 255,
                b: 255,
            },
        )
        .unwrap()
    }

    /// The glyphs of `value` typeset whole, as the layouts are.
    fn typeset_whole(readout: &Readout, value: f64) -> Vec<Glyph> {
        let text = readout.text(value);
        let glyphs = typeset::math(
            &format!("{text}{}", readout.unit),
            readout.color,
            Layout {
                size: readout.size.get(),
                align: Align::Left,
            },
        )
        .unwrap();
        lined_up(glyphs, &text, readout.decimals)
    }

    #[test]
    fn a_number_shows_its_decimals_and_no_minus_before_zero() {
        assert_eq!(readout(1, "").text(9.81), "9.8");
        assert_eq!(readout(1, "").text(-0.04), "0.0");
        assert_eq!(readout(0, "").text(-2.6), "-3");
    }

    #[test]
    fn each_digit_point_and_sign_of_a_number_is_one_glyph() {
        assert_eq!(
            readout(2, "").glyphs(-12.5).len(),
            6,
            "minus, 1, 2, point, 5, 0"
        );
    }

    #[test]
    fn a_number_from_its_layout_runs_across_as_the_number_typeset_whole() {
        for (decimals, unit, value) in [
            (2, "\\text{ m/s}", -12.5),
            (0, "", 9876543210.0),
            (3, "\\,\\mathrm{kg}", 0.1234),
            (1, "\\text{ s}", 77.7),
        ] {
            let readout = readout(decimals, unit);
            let (from_layout, whole) = (readout.glyphs(value), typeset_whole(&readout, value));
            assert_eq!(from_layout.len(), whole.len(), "{value}");
            for (a, b) in from_layout.iter().zip(&whole) {
                let (a, b) = (a.path.bounding_box(), b.path.bounding_box());
                assert!(
                    (a.x0 - b.x0).abs() < 1e-4 && (a.x1 - b.x1).abs() < 1e-4,
                    "{value}: {a:?} and {b:?}"
                );
                assert!(
                    (a.height() - b.height()).abs() < 1e-4 && (a.y0 - b.y0).abs() < 0.01,
                    "{value}: the same glyph, as high within a hundredth: {a:?} and {b:?}"
                );
            }
        }
    }

    #[test]
    fn a_number_keeps_its_baseline_as_its_digits_change() {
        let readout = readout(1, "\\text{ m}");
        let unit_bottom = |value| readout.glyphs(value).last().unwrap().path.bounding_box().y0;

        assert_eq!(unit_bottom(10.0), unit_bottom(77.7));
    }

    #[test]
    fn the_decimal_point_keeps_its_place_as_the_number_grows() {
        let unit = |value| {
            readout(1, "\\text{ m}")
                .glyphs(value)
                .last()
                .unwrap()
                .path
                .bounding_box()
                .x1
        };
        let first_digit = |value| readout(1, "").glyphs(value)[0].path.bounding_box().x0;

        assert!((unit(3.2) - unit(123.4)).abs() < 1e-4, "the unit stays put");
        assert!(
            first_digit(123.4) < first_digit(3.2),
            "a longer number grows to the left"
        );
    }

    #[test]
    fn a_unit_that_cannot_be_typeset_is_found_when_the_readout_is_made() {
        assert!(
            Readout::new(
                0.0,
                1,
                "\\frac{".to_string(),
                Positive::try_from(1.0).unwrap(),
                Color {
                    r: 255,
                    g: 255,
                    b: 255
                }
            )
            .is_err()
        );
    }
}
