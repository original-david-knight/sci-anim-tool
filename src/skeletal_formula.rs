//! A skeletal formula: a molecule drawn flat as a textbook draws it, from a 2D structure file.
//!
//! The whole drawing is made once, when the scene file is read, in the object's own space and
//! world units: carbons as corners, other atoms as their atom symbols, bonds as strokes, stereo
//! bonds as wedges and hashes. A write draws the bonds along their path, filling each wedge
//! as the stroke passes it, then writes the atom symbols glyph by glyph. A highlight lights an atom
//! group with a see-through fill over its atoms and the bonds between them.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;

use glam::Vec2;
use kurbo::{BezPath, Rect};
use serde::Serialize;

use crate::element::Element;
use crate::group::{Glow, GroupId, Groups};
use crate::molecule::{AtomNumber, BondOrder, Molecule};
use crate::path::Path;
use crate::scene::{Align, Color, Font, Glyph, MirrorPlane, Positive};
use crate::structure_file::{Bond2d, Stereo, Structure2d};
use crate::typeset::{self, Layout, TypesetError};

/// The proportions of a textbook drawing, as shares of the bond length, after the ACS style that
/// chemistry journals print in.
const DOUBLE_SPACING: f32 = 0.18;
/// How much shorter, at each end, the second stroke of a ring's double bond is than the first.
const INNER_TRIM: f32 = 0.16;
const WEDGE_WIDTH: f32 = 0.16;
const HASH_SPACING: f32 = 0.11;
/// How far a glow spreads from each of its atoms and the bonds between them.
const GLOW_RADIUS: f32 = 0.28;
/// How wide a glow's band along a bond is, as a share of its discs' width. Narrower than the
/// discs, so no corner of a band lies on a disc's edge, where lyon cuts overlapping parts wrongly.
const BAND_SHARE: f32 = 0.8;
/// The default text size of the atom symbols, and the default stroke width.
pub const DEFAULT_SIZE: f32 = 0.6;
pub const DEFAULT_STROKE_WIDTH: f32 = 0.04;
/// How far a bond stops short of an atom symbol's element_symbol, as a share of the text size.
const CLEARANCE: f32 = 0.14;
/// The share of a write that draws the bonds, before the atom symbols are written.
const BONDS_SHARE: f32 = 0.6;

/// How a skeletal formula looks, as its scene file sets it.
#[derive(Clone, Copy, Debug)]
pub struct Look {
    /// The length its bonds between atoms other than hydrogen are drawn at, as their median.
    pub bond_length: Positive,
    /// The text size of its atom symbols.
    pub size: Positive,
    pub stroke_width: Positive,
    pub color: Color,
    pub font: Font,
}

/// How a skeletal formula shows one atom.
#[derive(Clone, Debug)]
pub enum AtomForm {
    /// A carbon with a bond drawn, shown only by where its bonds meet.
    Corner,
    Symbol(AtomSymbol),
    /// A hydrogen on a carbon, or one written in its atom's symbol, with no stereo bond.
    Hidden,
}

/// The element_symbol a skeletal formula writes at an atom: its element with the hydrogens it holds and
/// its charge, such as `NH₂`, `HO` or `O⁻`.
#[derive(Clone, Debug)]
pub struct AtomSymbol {
    pub text: String,
    /// Its glyphs in the formula's glyphs.
    pub glyphs: Range<usize>,
    /// The box of the element's element_symbol, which a bond stops short of.
    clear: Rect,
}

#[derive(Clone, Debug)]
pub struct DrawnAtom {
    pub number: AtomNumber,
    pub element: Element,
    /// Where it lies in the formula's own space.
    pub position: Vec2,
    pub form: AtomForm,
}

/// How a skeletal formula draws one bond. No variant doc comments, for the reason `FillRule`
/// gives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BondForm {
    Line,
    DoubleCentered,
    DoubleInRing,
    Triple,
    Wedge,
    Hash,
    Hidden,
}

#[derive(Clone, Debug)]
pub struct DrawnBond {
    pub bond: Bond2d,
    pub form: BondForm,
    /// How many strokes a write draws for it: one a line, two a double bond, a wedge's outline,
    /// one each stroke of a hash.
    pub strokes: usize,
}

/// A wedge's fill, which shows as the bonds' stroke passes its outline: `drawn` is where the outline lies
/// along the bond path, as shares of its length.
#[derive(Clone, Debug)]
pub struct WedgeFill {
    pub path: Path,
    pub drawn: (f32, f32),
}

#[derive(Clone, Debug)]
pub struct SkeletalFormula {
    /// The atoms and bonds of the 2D structure file, which its atom groups are checked against.
    pub molecule: Arc<Molecule>,
    /// Indexed as the molecule's atoms.
    pub atoms: Vec<DrawnAtom>,
    /// In file order.
    pub bonds: Vec<DrawnBond>,
    /// The bond path: every bond's strokes, a hash's strokes and wedge outlines, in the order a write
    /// draws them. `None` for a formula with no bond drawn.
    pub bond_path: Option<Path>,
    pub wedges: Vec<WedgeFill>,
    /// Every atom symbol's glyphs, in the order a write writes them.
    pub glyphs: Vec<Glyph>,
    pub look: Look,
    pub groups: Groups,
    /// The glow of each group that a highlight has touched. A group not listed has none. The
    /// timeline's tracks replace the values here at every scene time.
    pub glows: BTreeMap<GroupId, Glow>,
}

/// One straight stroke of a bond, from `from` to `to`, and whether each end lies on its atom's
/// center, where it can join the next stroke at a corner.
#[derive(Clone, Copy)]
struct Stroke {
    from: Vec2,
    to: Vec2,
    joins: [bool; 2],
}

impl Stroke {
    fn reversed(self) -> Stroke {
        Stroke {
            from: self.to,
            to: self.from,
            joins: [self.joins[1], self.joins[0]],
        }
    }
}

/// What one bond draws, from its first atom to its second.
enum Drawing {
    Strokes(Vec<Stroke>),
    Wedge([Vec2; 3]),
}

impl SkeletalFormula {
    /// The formula of `structure` drawn as `look` says, its atoms reflected through `mirror`: `x`
    /// and `y` flip the drawing across that axis through its middle, and `z` keeps every atom
    /// where it is and swaps its wedges and hashes. Fails only when an atom symbol cannot be
    /// typeset, which the bundled font rules out for every element.
    pub fn new(
        structure: Structure2d,
        look: Look,
        mirror: Option<MirrorPlane>,
    ) -> Result<SkeletalFormula, TypesetError> {
        let Structure2d {
            molecule,
            bonds,
            charges,
        } = structure;
        let elements: Vec<Element> = molecule
            .numbers()
            .map(|n| molecule.atom(n).element)
            .collect();
        let is_hydrogen = |i: usize| elements[i] == Element::HYDROGEN;
        let bonds: Vec<Bond2d> = bonds
            .into_iter()
            .map(|bond| Bond2d {
                stereo: bond
                    .stereo
                    .filter(|_| bond.order == BondOrder::Single)
                    .map(|stereo| match mirror {
                        Some(MirrorPlane::Z) => stereo.through_the_page(),
                        _ => stereo,
                    }),
                ..bond
            })
            .collect();
        let factor = look.bond_length.get() / median_length(&molecule, &bonds, &is_hydrogen);
        let mut positions: Vec<Vec2> = molecule
            .numbers()
            .map(|n| {
                let flat = molecule.atom(n).position * factor;
                mirror.map_or(flat, |plane| plane.reflect(flat)).truncate()
            })
            .collect();
        let count = elements.len();
        let mut neighbors: Vec<Vec<(usize, usize)>> = vec![Vec::new(); count];
        for (k, bond) in bonds.iter().enumerate() {
            let (a, b) = (bond.from.index(), bond.to.index());
            neighbors[a].push((b, k));
            neighbors[b].push((a, k));
        }
        let has_stereo = |i: usize| neighbors[i].iter().any(|&(_, k)| bonds[k].stereo.is_some());
        let hidden: Vec<bool> = (0..count)
            .map(|i| {
                is_hydrogen(i)
                    && !has_stereo(i)
                    && matches!(neighbors[i][..], [(j, _)] if !is_hydrogen(j))
            })
            .collect();
        let shown: Vec<Vec2> = (0..count)
            .filter(|&i| !hidden[i])
            .map(|i| positions[i])
            .collect();
        let middle = middle(&shown);
        positions.iter_mut().for_each(|p| *p -= middle);
        let drawn_bond = |k: usize| !hidden[bonds[k].from.index()] && !hidden[bonds[k].to.index()];
        let drawn_neighbors = |i: usize| {
            neighbors[i]
                .iter()
                .filter(|&&(_, k)| drawn_bond(k))
                .map(|&(j, _)| j)
                .collect::<Vec<usize>>()
        };
        let (bond_order, atom_order) = traversal(count, &neighbors, &hidden, &drawn_bond);

        let mut glyphs = Vec::new();
        let mut forms: Vec<AtomForm> = (0..count)
            .map(|i| match hidden[i] {
                true => AtomForm::Hidden,
                false => AtomForm::Corner,
            })
            .collect();
        for &i in &atom_order {
            let bonded = drawn_neighbors(i);
            if elements[i] == Element::CARBON && !bonded.is_empty() && charges[i] == 0 {
                continue;
            }
            let hydrogens = neighbors[i].iter().filter(|&&(j, _)| hidden[j]).count();
            let away = -bonded
                .iter()
                .map(|&j| (positions[j] - positions[i]).normalize_or_zero())
                .sum::<Vec2>();
            let symbol = atom_symbol(
                elements[i],
                hydrogens,
                charges[i],
                Side::away(away, bonded.len()),
                positions[i],
                &look,
                &mut glyphs,
            )?;
            forms[i] = AtomForm::Symbol(symbol);
        }
        let atoms: Vec<DrawnAtom> = molecule
            .numbers()
            .zip(forms)
            .map(|(number, form)| DrawnAtom {
                number,
                element: elements[number.index()],
                position: positions[number.index()],
                form,
            })
            .collect();

        let rings = Rings {
            neighbors: &neighbors,
            drawn_bond: &drawn_bond,
        };
        let mut drawn_bonds = Vec::with_capacity(bonds.len());
        let mut drawings = Vec::with_capacity(bonds.len());
        for (k, bond) in bonds.iter().enumerate() {
            let (form, drawing) = if drawn_bond(k) {
                bond_drawing(bond, k, &atoms, &rings, &look)
            } else {
                (BondForm::Hidden, Drawing::Strokes(Vec::new()))
            };
            let strokes = match &drawing {
                Drawing::Strokes(strokes) => strokes.len(),
                Drawing::Wedge(_) => 1,
            };
            drawn_bonds.push(DrawnBond {
                bond: *bond,
                form,
                strokes,
            });
            drawings.push(drawing);
        }
        let (drawn_path, wedges) = bond_path(&bond_order, &bonds, &drawings);
        Ok(SkeletalFormula {
            molecule: Arc::new(molecule),
            atoms,
            bonds: drawn_bonds,
            bond_path: drawn_path,
            wedges,
            glyphs,
            look,
            groups: Groups::default(),
            glows: BTreeMap::new(),
        })
    }

    /// How far a write at `created` has drawn the bonds and written the atom symbols: the bonds
    /// over the first 60%, the symbols over the rest, or either over all of it when the formula
    /// has nothing of the other.
    pub fn written_shares(&self, created: f32) -> (f32, f32) {
        match (self.bond_path.is_some(), !self.glyphs.is_empty()) {
            (true, true) => (
                (created / BONDS_SHARE).min(1.0),
                ((created - BONDS_SHARE) / (1.0 - BONDS_SHARE)).max(0.0),
            ),
            (true, false) => (created, 0.0),
            (false, _) => (0.0, created),
        }
    }

    /// How much of a wedge's fill shows when the bonds' stroke has drawn `bond_share` of the bond path.
    pub fn wedge_share(wedge: &WedgeFill, bond_share: f32) -> f32 {
        let (from, to) = wedge.drawn;
        if bond_share >= 1.0 {
            return 1.0;
        }
        ((bond_share - from) / (to - from).max(f32::EPSILON)).clamp(0.0, 1.0)
    }

    /// Whether a glow of the group of `atoms` draws anything: whether it holds an atom the formula
    /// shows.
    pub fn lights(&self, atoms: &[AtomNumber]) -> bool {
        atoms
            .iter()
            .any(|n| !matches!(self.atoms[n.index()].form, AtomForm::Hidden))
    }

    /// The see-through fill a glow of the group of `atoms` lays: a disc about each shown atom,
    /// a box about each atom symbol, and a band along each bond drawn between two of them, all
    /// one path that the non-zero fill rule fills once where they overlap. `None` when the group
    /// holds no shown atom.
    pub fn glow_path(&self, atoms: &[AtomNumber]) -> Option<Path> {
        let radius = GLOW_RADIUS * self.look.bond_length.get();
        let in_group = |n: &AtomNumber| atoms.binary_search(n).is_ok();
        let mut path = BezPath::new();
        for &n in atoms {
            let atom = &self.atoms[n.index()];
            match &atom.form {
                AtomForm::Hidden => {}
                AtomForm::Corner => circle(&mut path, atom.position, radius),
                AtomForm::Symbol(symbol) => {
                    let ink = self.glyph_box(symbol.glyphs.clone());
                    let r = ink.inflate(f64::from(radius) * 0.5, f64::from(radius) * 0.5);
                    counterclockwise(
                        &mut path,
                        &[
                            Vec2::new(r.x0 as f32, r.y0 as f32),
                            Vec2::new(r.x1 as f32, r.y0 as f32),
                            Vec2::new(r.x1 as f32, r.y1 as f32),
                            Vec2::new(r.x0 as f32, r.y1 as f32),
                        ],
                    );
                }
            }
        }
        for drawn in &self.bonds {
            let [a, b] = [drawn.bond.from, drawn.bond.to];
            if drawn.form == BondForm::Hidden || !in_group(&a) || !in_group(&b) {
                continue;
            }
            let (p, q) = (
                self.atoms[a.index()].position,
                self.atoms[b.index()].position,
            );
            let across = (q - p).normalize_or_zero().perp() * radius * BAND_SHARE;
            counterclockwise(&mut path, &[p - across, q - across, q + across, p + across]);
        }
        Path::from_bez(path).ok()
    }

    /// The box around the ink of `glyphs`, in the formula's own space.
    pub fn glyph_box(&self, glyphs: Range<usize>) -> Rect {
        ink_box(&self.glyphs[glyphs])
    }

    /// How far its farthest drawn point lies from its position.
    pub fn reach(&self) -> f32 {
        let half_stroke = self.look.stroke_width.get() / 2.0;
        let corners = |r: Rect| {
            [(r.x0, r.y0), (r.x1, r.y0), (r.x0, r.y1), (r.x1, r.y1)]
                .into_iter()
                .map(|(x, y)| x.hypot(y) as f32)
                .fold(0.0, f32::max)
        };
        let bonds = self
            .bond_path
            .as_ref()
            .map_or(0.0, |path| corners(path.bounding_box()) + half_stroke);
        let glyphs = self
            .glyphs
            .iter()
            .map(|g| corners(g.path.bounding_box()))
            .fold(0.0, f32::max);
        bonds.max(glyphs)
    }
}

/// The median length of the bonds between two atoms other than hydrogen, or of every bond when
/// there is none, or 1 for a lone atom, in the file's units.
fn median_length(
    molecule: &Molecule,
    bonds: &[Bond2d],
    is_hydrogen: &dyn Fn(usize) -> bool,
) -> f32 {
    let length = |bond: &Bond2d| {
        (molecule.atom(bond.from).position - molecule.atom(bond.to).position).length()
    };
    let heavy: Vec<f32> = bonds
        .iter()
        .filter(|b| !is_hydrogen(b.from.index()) && !is_hydrogen(b.to.index()))
        .map(length)
        .collect();
    let mut lengths = if heavy.is_empty() {
        bonds.iter().map(length).collect()
    } else {
        heavy
    };
    lengths.retain(|&l| l > 0.0);
    lengths.sort_by(f32::total_cmp);
    lengths.get(lengths.len() / 2).copied().unwrap_or(1.0)
}

/// The middle of the box around `points`, the shown atoms, which centers the formula on its
/// position.
fn middle(points: &[Vec2]) -> Vec2 {
    let (min, max) = points.iter().fold(
        (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
        |(min, max), &p| (min.min(p), max.max(p)),
    );
    (min + max) / 2.0
}

/// The order a write draws the bonds in, each with the atom it is drawn from, and the order of
/// the shown atoms, by a walk that goes as deep as it can before it turns back, so the stroke draws
/// long runs. Each piece of the molecule starts at its first atom with one bond, or its first
/// atom.
fn traversal(
    count: usize,
    neighbors: &[Vec<(usize, usize)>],
    hidden: &[bool],
    drawn_bond: &dyn Fn(usize) -> bool,
) -> (Vec<(usize, usize)>, Vec<usize>) {
    let degree = |i: usize| neighbors[i].iter().filter(|&&(_, k)| drawn_bond(k)).count();
    let mut starts: Vec<usize> = (0..count).filter(|&i| !hidden[i]).collect();
    starts.sort_by_key(|&i| (degree(i) != 1, i));
    let mut visited = vec![false; count];
    let mut used = vec![
        false;
        neighbors
            .iter()
            .flatten()
            .map(|&(_, k)| k + 1)
            .max()
            .unwrap_or(0)
    ];
    let (mut bond_order, mut atom_order) = (Vec::new(), Vec::new());
    for start in starts {
        if visited[start] {
            continue;
        }
        visited[start] = true;
        atom_order.push(start);
        let mut stack = vec![start];
        while let Some(&at) = stack.last() {
            let next = neighbors[at]
                .iter()
                .find(|&&(_, k)| drawn_bond(k) && !used[k])
                .copied();
            let Some((to, k)) = next else {
                stack.pop();
                continue;
            };
            used[k] = true;
            bond_order.push((k, at));
            if !visited[to] {
                visited[to] = true;
                atom_order.push(to);
                stack.push(to);
            }
        }
    }
    (bond_order, atom_order)
}

/// Where an atom symbol writes the hydrogens it holds beside its element: on the side away from
/// its bonds, right unless they come from the right, and above or below only for an atom whose
/// bonds come from both sides.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Side {
    Right,
    Left,
    Above,
    Below,
}

impl Side {
    fn away(away: Vec2, bonds: usize) -> Side {
        let length = away.length();
        if bonds >= 2 && length > 0.1 && away.x.abs() < 0.3 * length {
            return if away.y > 0.0 {
                Side::Above
            } else {
                Side::Below
            };
        }
        if away.x < -0.1 * length.max(1e-6) && length > 1e-3 {
            Side::Left
        } else {
            Side::Right
        }
    }
}

const SUBSCRIPTS: [char; 10] = ['₀', '₁', '₂', '₃', '₄', '₅', '₆', '₇', '₈', '₉'];
const SUPERSCRIPTS: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];

/// `H`, `H₂` and on for `count` hydrogens, and nothing for none.
fn hydrogens_text(count: usize) -> String {
    match count {
        0 => String::new(),
        1 => "H".to_string(),
        n => std::iter::once('H')
            .chain(
                n.to_string()
                    .chars()
                    .map(|d| SUBSCRIPTS[d as usize - '0' as usize]),
            )
            .collect(),
    }
}

/// `⁺`, `²⁻` and on for a formal charge, and nothing for none.
fn charge_text(charge: i8) -> String {
    let sign = if charge > 0 { '⁺' } else { '⁻' };
    match charge.unsigned_abs() {
        0 => String::new(),
        1 => sign.to_string(),
        n => n
            .to_string()
            .chars()
            .map(|d| SUPERSCRIPTS[d as usize - '0' as usize])
            .chain([sign])
            .collect(),
    }
}

/// Typesets the atom symbol of an atom of `element` at `at`, holding `hydrogens` and carrying
/// `charge`, with its hydrogens on `side`, and appends its glyphs to `glyphs`. The element's
/// element_symbol are centered on the atom.
fn atom_symbol(
    element: Element,
    hydrogens: usize,
    charge: i8,
    side: Side,
    at: Vec2,
    look: &Look,
    glyphs: &mut Vec<Glyph>,
) -> Result<AtomSymbol, TypesetError> {
    let layout = Layout {
        size: look.size.get(),
        align: Align::Left,
    };
    let (element_symbol, hs, sign) = (
        element.symbol(),
        hydrogens_text(hydrogens),
        charge_text(charge),
    );
    let (line, element_first) = match side {
        Side::Right => (format!("{element_symbol}{hs}{sign}"), 0),
        Side::Left => (format!("{hs}{element_symbol}{sign}"), hs.chars().count()),
        Side::Above | Side::Below => (format!("{element_symbol}{sign}"), 0),
    };
    let mut set = typeset::text(&line, look.font, look.color, layout)?;
    let element_glyphs = glyphs_showing(&set, element_first, element_symbol);
    let clear = ink_box(&set[element_glyphs]);
    let shift = at - Vec2::new(clear.center().x as f32, clear.center().y as f32);
    let mut text = line;
    if hydrogens > 0 && matches!(side, Side::Above | Side::Below) {
        let below = typeset::text(&hs, look.font, look.color, layout)?;
        let hs_box = ink_box(&below);
        let gap = f64::from(0.18 * look.size.get());
        let dy = match side {
            Side::Above => clear.y1 + gap - hs_box.y0,
            _ => clear.y0 - gap - hs_box.y1,
        };
        let dx = clear.center().x - hs_box.center().x;
        set.extend(below.into_iter().map(|glyph| Glyph {
            path: glyph.path.moved(Vec2::new(dx as f32, dy as f32)),
            ..glyph
        }));
        text = format!("{element_symbol}{hs}{sign}");
    }
    let first = glyphs.len();
    glyphs.extend(set.into_iter().map(|glyph| Glyph {
        path: glyph.path.moved(shift),
        ..glyph
    }));
    let moved = kurbo::Vec2::new(f64::from(shift.x), f64::from(shift.y));
    Ok(AtomSymbol {
        text,
        glyphs: first..glyphs.len(),
        clear: clear + moved,
    })
}

/// The glyphs of `set` that show `element_symbol`, starting after the glyphs of its first `skip`
/// characters, or every glyph when they cannot be found.
fn glyphs_showing(set: &[Glyph], skip: usize, element_symbol: &str) -> Range<usize> {
    let mut characters = 0;
    let mut start = None;
    for (i, glyph) in set.iter().enumerate() {
        if characters == skip && start.is_none() {
            start = Some(i);
        }
        characters += glyph.characters.chars().count();
        if let Some(s) = start {
            let shown: String = set[s..=i].iter().map(|g| g.characters.as_str()).collect();
            if shown == element_symbol {
                return s..i + 1;
            }
        }
    }
    0..set.len()
}

/// The box around the ink of `glyphs`.
fn ink_box(glyphs: &[Glyph]) -> Rect {
    glyphs
        .iter()
        .map(|g| g.path.bounding_box())
        .reduce(|a, b| a.union(b))
        .unwrap_or_default()
}

/// What finds a bond's ring: the atoms' bonds and which bonds are drawn.
struct Rings<'a> {
    neighbors: &'a [Vec<(usize, usize)>],
    drawn_bond: &'a dyn Fn(usize) -> bool,
}

impl Rings<'_> {
    /// The atoms of the smallest ring through bond `k` from `a` to `b`, by the shortest way from
    /// `a` to `b` over the other drawn bonds, or `None` for a bond in no ring.
    fn smallest(&self, k: usize, a: usize, b: usize) -> Option<Vec<usize>> {
        let mut came_from = vec![usize::MAX; self.neighbors.len()];
        came_from[a] = a;
        let mut queue = std::collections::VecDeque::from([a]);
        while let Some(at) = queue.pop_front() {
            if at == b {
                let mut ring = vec![b];
                let mut step = b;
                while step != a {
                    step = came_from[step];
                    ring.push(step);
                }
                return Some(ring);
            }
            for &(to, j) in &self.neighbors[at] {
                if j != k && (self.drawn_bond)(j) && came_from[to] == usize::MAX {
                    came_from[to] = at;
                    queue.push_back(to);
                }
            }
        }
        None
    }
}

/// How `bond` is drawn, from its first atom to its second, each end stopping short of an atom
/// symbol there.
fn bond_drawing(
    bond: &Bond2d,
    k: usize,
    atoms: &[DrawnAtom],
    rings: &Rings,
    look: &Look,
) -> (BondForm, Drawing) {
    let length = look.bond_length.get();
    let (a, b) = (&atoms[bond.from.index()], &atoms[bond.to.index()]);
    let (p, q) = (a.position, b.position);
    let along = (q - p).normalize_or_zero();
    let across = along.perp();
    let clearance = f64::from(CLEARANCE * look.size.get());
    let cleared = |from: Vec2, to: Vec2| -> Option<Stroke> {
        let mut stroke = Stroke {
            from,
            to,
            joins: [from == p, to == q],
        };
        if let AtomForm::Symbol(symbol) = &a.form {
            let t = leaves(
                stroke.from,
                stroke.to,
                symbol.clear.inflate(clearance, clearance),
            )?;
            stroke.from = stroke.from.lerp(stroke.to, t);
            stroke.joins[0] = false;
        }
        if let AtomForm::Symbol(symbol) = &b.form {
            let reversed = stroke.reversed();
            let t = leaves(
                reversed.from,
                reversed.to,
                symbol.clear.inflate(clearance, clearance),
            )?;
            stroke.to = reversed.from.lerp(reversed.to, t);
            stroke.joins[1] = false;
        }
        Some(stroke)
    };
    let offset = |by: f32, trim: f32| {
        let shift = across * by * length;
        let trim = along * trim * length;
        cleared(p + shift + trim, q + shift - trim)
    };
    let strokes =
        |list: Vec<Option<Stroke>>| Drawing::Strokes(list.into_iter().flatten().collect());
    match (bond.order, bond.stereo) {
        (BondOrder::Single, Some(stereo)) => {
            let Some(line) = cleared(p, q) else {
                return (BondForm::Line, Drawing::Strokes(Vec::new()));
            };
            let wide = WEDGE_WIDTH * length / 2.0;
            match stereo {
                Stereo::Wedge => (
                    BondForm::Wedge,
                    Drawing::Wedge([line.from, line.to + across * wide, line.to - across * wide]),
                ),
                Stereo::Hash => {
                    let run = (line.to - line.from).length();
                    let hash_strokes = ((run / (HASH_SPACING * length)).round() as usize).max(3);
                    let narrow = look.stroke_width.get();
                    let hashes = (0..hash_strokes)
                        .map(|i| {
                            let t = 0.08 + 0.92 * i as f32 / (hash_strokes - 1) as f32;
                            let at = line.from.lerp(line.to, t);
                            let half = (narrow + (wide - narrow) * t).max(narrow / 2.0);
                            Stroke {
                                from: at - across * half,
                                to: at + across * half,
                                joins: [false, false],
                            }
                        })
                        .collect();
                    (BondForm::Hash, Drawing::Strokes(hashes))
                }
            }
        }
        (BondOrder::Single, None) => (BondForm::Line, strokes(vec![cleared(p, q)])),
        (BondOrder::Double, _) => {
            let (i, j) = (bond.from.index(), bond.to.index());
            match rings.smallest(k, i, j) {
                Some(ring) => {
                    let center =
                        ring.iter().map(|&r| atoms[r].position).sum::<Vec2>() / ring.len() as f32;
                    let inside = if (center - p).dot(across) >= 0.0 {
                        1.0
                    } else {
                        -1.0
                    };
                    (
                        BondForm::DoubleInRing,
                        strokes(vec![
                            cleared(p, q),
                            offset(inside * DOUBLE_SPACING, INNER_TRIM),
                        ]),
                    )
                }
                None => (
                    BondForm::DoubleCentered,
                    strokes(vec![
                        offset(DOUBLE_SPACING / 2.0, 0.0),
                        offset(-DOUBLE_SPACING / 2.0, 0.0),
                    ]),
                ),
            }
        }
        (BondOrder::Triple, _) => (
            BondForm::Triple,
            strokes(vec![
                offset(DOUBLE_SPACING, 0.0),
                cleared(p, q),
                offset(-DOUBLE_SPACING, 0.0),
            ]),
        ),
    }
}

/// How far along the stroke from `from` to `to`, from 0 to 1, it leaves `area`, a box about its
/// start, or 0 when it starts outside it, or `None` when it ends inside it.
fn leaves(from: Vec2, to: Vec2, area: Rect) -> Option<f32> {
    let step = to - from;
    let inside = |p: Vec2| area.contains(kurbo::Point::new(f64::from(p.x), f64::from(p.y)));
    if !inside(from) {
        return Some(0.0);
    }
    if inside(to) {
        return None;
    }
    let exit = |start: f32, along: f32, low: f64, high: f64| {
        if along > 0.0 {
            (high as f32 - start) / along
        } else if along < 0.0 {
            (low as f32 - start) / along
        } else {
            f32::INFINITY
        }
    };
    Some(
        exit(from.x, step.x, area.x0, area.x1)
            .min(exit(from.y, step.y, area.y0, area.y1))
            .clamp(0.0, 1.0),
    )
}

/// The bond path through every bond in `order`, each drawn from the atom given with it, and each
/// wedge's fill with where its outline lies along the path. A stroke that starts where the last
/// one ended, at an atom's center, carries on from it, so a chain of corners is mitered rather
/// than notched.
fn bond_path(
    order: &[(usize, usize)],
    bonds: &[Bond2d],
    drawings: &[Drawing],
) -> (Option<Path>, Vec<WedgeFill>) {
    let mut path = BezPath::new();
    let mut walked = 0.0f32;
    let mut open_end: Option<Vec2> = None;
    let mut wedge_spans = Vec::new();
    let point = |p: Vec2| kurbo::Point::new(f64::from(p.x), f64::from(p.y));
    for &(k, from) in order {
        let forward = bonds[k].from.index() == from;
        match &drawings[k] {
            Drawing::Strokes(strokes) => {
                for stroke in strokes {
                    let stroke = if forward { *stroke } else { stroke.reversed() };
                    if !(stroke.joins[0] && open_end == Some(stroke.from)) {
                        path.move_to(point(stroke.from));
                    }
                    path.line_to(point(stroke.to));
                    walked += (stroke.to - stroke.from).length();
                    open_end = stroke.joins[1].then_some(stroke.to);
                }
            }
            Drawing::Wedge(corners) => {
                let start = walked;
                path.move_to(point(corners[0]));
                for i in 1..=3 {
                    let corner = corners[i % 3];
                    if i < 3 {
                        path.line_to(point(corner));
                    }
                    walked += (corner - corners[i - 1]).length();
                }
                path.close_path();
                wedge_spans.push((corners, start, walked));
                open_end = None;
            }
        }
    }
    let Ok(bond_path) = Path::from_bez(path) else {
        return (None, Vec::new());
    };
    let total = walked.max(f32::EPSILON);
    let wedges = wedge_spans
        .into_iter()
        .filter_map(|(corners, start, end)| {
            Some(WedgeFill {
                path: Path::polygon(&counterclockwise_points(*corners)).ok()?,
                drawn: (start / total, end / total),
            })
        })
        .collect();
    (Some(bond_path), wedges)
}

/// `points` in counterclockwise order, so overlapping parts of one path fill once under the
/// non-zero fill rule.
fn counterclockwise_points<const N: usize>(mut points: [Vec2; N]) -> [Vec2; N] {
    let twice_area: f32 = (0..N)
        .map(|i| points[i].perp_dot(points[(i + 1) % N]))
        .sum();
    if twice_area < 0.0 {
        points.reverse();
    }
    points
}

fn counterclockwise(path: &mut BezPath, points: &[Vec2; 4]) {
    let points = counterclockwise_points(*points);
    let point = |p: Vec2| kurbo::Point::new(f64::from(p.x), f64::from(p.y));
    path.move_to(point(points[0]));
    points[1..].iter().for_each(|&p| path.line_to(point(p)));
    path.close_path();
}

/// A counterclockwise circle about `center`, as `kurbo` makes one.
fn circle(path: &mut BezPath, center: Vec2, radius: f32) {
    use kurbo::Shape as _;
    let circle = kurbo::Circle::new(
        kurbo::Point::new(f64::from(center.x), f64::from(center.y)),
        f64::from(radius),
    );
    path.extend(circle.path_elements(1e-3 * f64::from(radius)));
}

/// What `inspect` tells an agent about a skeletal formula: its bond length and text size, every
/// atom with how it is drawn, every bond with its form, and each atom group.
#[derive(Debug, Serialize)]
pub struct SkeletalFormulaReport {
    pub bond_length: f32,
    pub size: f32,
    pub atoms: Vec<FormulaAtomReport>,
    pub bonds: Vec<FormulaBondReport>,
    pub groups: BTreeMap<GroupId, FormulaGroupReport>,
}

/// One atom: its number and element, `corner`, `symbol` or `hidden`, its atom symbol's text, its
/// world position and pixel, and the box of its atom symbol.
#[derive(Debug, Serialize)]
pub struct FormulaAtomReport {
    pub atom: AtomNumber,
    pub element: Element,
    pub drawn: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    pub position: glam::Vec3,
    pub position_pixels: Option<Vec2>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounding_box: Option<crate::mesh::BoundingBox>,
}

/// One bond: its atoms as the file writes them, the first a stereo bond's narrow end, its bond/// order, whether it is a wedge or a hash, how it is drawn, and how many strokes a write draws for it.
#[derive(Debug, Serialize)]
pub struct FormulaBondReport {
    pub atoms: [AtomNumber; 2],
    pub order: BondOrder,
    pub stereo: Option<Stereo>,
    pub form: BondForm,
    pub strokes: usize,
}

/// One atom group: its atoms, its glow share and color, and the box of its glow.
#[derive(Debug, Serialize)]
pub struct FormulaGroupReport {
    pub atoms: Vec<AtomNumber>,
    pub glow: f32,
    pub color: String,
    pub bounding_box: Option<crate::mesh::BoundingBox>,
}

impl AtomForm {
    pub fn name(&self) -> &'static str {
        match self {
            AtomForm::Corner => "corner",
            AtomForm::Symbol(_) => "symbol",
            AtomForm::Hidden => "hidden",
        }
    }
}
