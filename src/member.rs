//! Members: the parts of an object an animation can change apart from the rest of it. A
//! highlight lights an atom group of a molecule, a cell of a table, a copy of a pattern or a bar
//! of a bar plot; a move and a fade change one copy; a recolor changes one copy or one bar.
//!
//! A member is named in the scene file beside the animation's `object`, by `group`, `cell`, `copy`
//! or `bar`, and `scene_file` checks it against the object once, when it reads the animation. Its
//! values at a scene time are computed from its tracks, as an object's are, so a moved copy's
//! place is never stored.

use std::fmt;

use crate::group::GroupId;
use crate::pattern::CopyIndex;
use crate::scene::Color;
use crate::table::CellAddress;

/// One member of an object, checked against it: holding a `Member` with the object it was read
/// for means the object has it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Member {
    Group(GroupId),
    Cell(CellAddress),
    Copy(CopyIndex),
    /// A bar of a bar plot, by its index in category order, from 0.
    Bar(usize),
}

impl fmt::Display for Member {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Member::Group(group) => write!(f, "group `{group}`"),
            Member::Cell(address) => write!(f, "cell {address}"),
            Member::Copy(index) => write!(f, "copy {index}"),
            Member::Bar(index) => write!(f, "bar {index}"),
        }
    }
}

/// The colors a recolor of one copy or one bar gave it, which it draws in place of the colors it
/// takes from its object. A color no recolor of the member named is `None`, and follows the
/// object's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OwnColors {
    pub stroke: Option<Color>,
    pub fill: Option<Color>,
    pub color: Option<Color>,
}

impl OwnColors {
    pub fn is_empty(&self) -> bool {
        *self == OwnColors::default()
    }
}
