//! The domain model of a reaction: the atom map between reactant and product molecules, the bonds
//! it breaks and forms, the best-fit rotation that lines the products up with the reactants, and
//! the blend that draws every atom on its way from its reactant position to its product position.
//!
//! Three stages, each a plain value:
//!
//! ```text
//! ReactionFile --Reaction::read-->  Reaction   the atom map and bonds, checked against the molecules;
//!                                               knows no transforms, so it is valid at any scene time
//! Reaction x transforms --> ReactionBlends  start and end blend of one reaction, with the product
//!                                               rotations; made once, when the timeline reads the step
//! ReactionBlends  --Mix, progress-->        Blend      every atom and bond at one progress; `Blend::draw`
//! ```
//!
//! Kabsch's problem is solved with Horn's quaternion method: the best rotation is the eigenvector
//! of a symmetric 4 by 4 matrix, so the result is always a proper rotation (never a reflection)
//! and needs no linear algebra crate.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use glam::{EulerRot, Quat, Vec3};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::element::Element;
use crate::group::{BALL_AND_STICK_GLOW, GLOW_BOND_RADIUS, Glow, GroupId, Groups};
use crate::molecule::{
    AtomNumber, BALL_SHARE, Ball, Bond, BondOrder, Drawn, Molecule, Representation, STICK_SPACING,
    Stick,
};
use crate::scene::{Object, ObjectId, Shape, Stage, Transform};
use crate::scene_file::{JsonPath, SceneError, SceneErrorCode};
use crate::timeline::{Mix, Progress};

// ---------------------------------------------------------------------------------------------
// The file form.

/// An atom named the way an agent finds it: the molecule object's id and the atom's number in
/// that object's structure file.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct AtomRefFile {
    /// The object id of a molecule object.
    pub object: String,
    /// The atom's number in that object's structure file, counting from 1 in the order the file
    /// lists its atoms, or in a PDB file the serial number the file gives it.
    #[schemars(range(min = 1))]
    pub atom: u32,
}

/// One pair of the atom map: the reactant atom and the product atom it becomes.
#[derive(Clone, Debug, Deserialize, JsonSchema)]
pub struct AtomPairFile {
    /// The atom before the reaction: an atom of one of the reaction's `reactants`.
    pub reactant: AtomRefFile,
    /// The atom it becomes: an atom of one of the reaction's `products`, of the same element.
    pub product: AtomRefFile,
}

// ---------------------------------------------------------------------------------------------
// The validated reaction.

/// One molecule object on a side of a reaction, with what the blend needs of it.
#[derive(Clone, Debug)]
pub struct Side {
    pub object_id: ObjectId,
    molecule: Arc<Molecule>,
    representation: Representation,
    groups: Groups,
}

/// An atom by the index of its molecule in `Reaction::reactants` or `Reaction::products`, and
/// its number there.
type SideAtom = (usize, AtomNumber);

/// An atom of the reaction: one reactant atom and the product atom it becomes.
#[derive(Clone, Debug, Serialize)]
pub struct MappedAtom {
    pub reactant: AtomRef,
    pub product: AtomRef,
    pub element: Element,
    pub hidden: bool,
    /// Where the atom is in `Reaction::reactants` and `Reaction::products`.
    #[serde(skip)]
    sides: (usize, usize),
}

/// What `inspect` and the atom map call an atom: its molecule object and its number there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AtomRef {
    pub object: ObjectId,
    pub atom: AtomNumber,
}

/// What a reaction does to a bond, found by comparing the two sides through the atom map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum BondFate {
    Kept {
        order: BondOrder,
    },
    Broken {
        order: BondOrder,
    },
    Formed {
        order: BondOrder,
    },
    /// Joins the same two atoms before and after, with another bond order after.
    Changed {
        from: BondOrder,
        to: BondOrder,
    },
}

/// How a bond's sticks look at one progress of its reaction.
#[derive(Clone, Debug, PartialEq)]
pub struct BondSticks {
    /// Where each stick lies across the bond, in multiples of `STICK_SPACING`.
    pub offsets: Vec<f32>,
    /// The radius of every stick at full bond weight, in angstroms.
    pub radius: f32,
    pub weight: f32,
}

impl BondFate {
    /// The one rule for how a bond looks mid-reaction, read by both `Blend::draw` and `inspect`.
    /// A broken bond thins over the first half of the progress and a formed one thickens over
    /// the second, so a bond is gone before its atoms are far apart and forms as they arrive. A
    /// changed bond keeps its weight and moves its sticks across the bond, so a double bond's
    /// two sticks close into one and a single bond's stick opens into two.
    pub fn look_at(self, progress: f32) -> BondSticks {
        let share = progress.clamp(0.0, 1.0);
        let (from, to, weight) = match self {
            BondFate::Kept { order } => (order, order, 1.0),
            BondFate::Broken { order } => (order, order, (1.0 - 2.0 * progress).clamp(0.0, 1.0)),
            BondFate::Formed { order } => (order, order, (2.0 * progress - 1.0).clamp(0.0, 1.0)),
            BondFate::Changed { from, to } => (from, to, 1.0),
        };
        let (from, to) = (from.spread(), to.spread());
        let sticks = from.offsets.len().max(to.offsets.len());
        let offset = |offsets: &[f32], j: usize| offsets[j * offsets.len() / sticks];
        let offsets = (0..sticks)
            .map(|j| {
                offset(from.offsets, j) + (offset(to.offsets, j) - offset(from.offsets, j)) * share
            })
            .collect();
        BondSticks {
            offsets,
            radius: from.radius + (to.radius - from.radius) * share,
            weight,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ReactionBond {
    /// Indices into `Reaction::atoms`, the lower first.
    pub atoms: [usize; 2],
    pub fate: BondFate,
}

#[derive(Debug)]
pub struct Reaction {
    pub reactants: Vec<Side>,
    pub products: Vec<Side>,
    /// In reactant order: the reactants as listed, each by atom number.
    pub atoms: Vec<MappedAtom>,
    /// Ordered by the atom indices.
    pub bonds: Vec<ReactionBond>,
    /// Whether the products are turned by the fit before their own rotations.
    fit: bool,
}

impl Reaction {
    /// The reaction the scene file describes, or every scene error in it. `path` is the JSON path
    /// of the animation, and `start` holds the molecule objects the sides name. Checks that every
    /// side is a molecule object, that every atom of each side is mapped exactly once, that
    /// mapped atoms are one element and are shown or hidden together, and that the two sides hold
    /// the same atoms. Nothing here depends on a scene time.
    pub fn read(
        path: &JsonPath,
        reactants: &[String],
        products: &[String],
        atom_map: &[AtomPairFile],
        fit: bool,
        start: &Stage,
    ) -> Result<Reaction, Vec<SceneError>> {
        let mut errors = Vec::new();
        let mut error = |at: JsonPath, code: SceneErrorCode, message: String| {
            errors.push(SceneError::new(at, code, message))
        };
        let mut side_of = |field: &str, ids: &[String], other: &[String]| -> Vec<Option<Side>> {
            ids.iter()
                .enumerate()
                .map(|(i, id)| {
                    let at = path.field(field).index(i);
                    let Some((object_id, Object { shape: Shape::Molecule { molecule, representation, groups, .. }, .. })) = start.objects.get_key_value(id.as_str()) else {
                        let (code, why) = if start.objects.contains_key(id.as_str()) {
                            (SceneErrorCode::NotMolecule, format!("`{id}` is not a molecule; a reaction turns molecule objects into molecule objects"))
                        } else {
                            (SceneErrorCode::UnknownObject, format!("no object has the id `{id}`; the objects are {}", start.objects.keys().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ")))
                        };
                        error(at, code, why);
                        return None;
                    };
                    if ids[..i].contains(id) || other.contains(id) {
                        let message = if other.contains(id) { format!("`{id}` is both a reactant and a product; a reaction needs separate objects for the two") } else { format!("`{id}` is listed twice in `{field}`") };
                        error(at, SceneErrorCode::DuplicateObject, message);
                        return None;
                    }
                    if !matches!(representation, Representation::BallAndStick { .. }) {
                        error(at, SceneErrorCode::Invalid, format!("`{id}` is drawn `{}`, and a reaction draws bonds breaking and forming as sticks; give `{id}` the `representation` `ball-and-stick`", representation.name()));
                    }
                    Some(Side { object_id: object_id.clone(), molecule: molecule.clone(), representation: *representation, groups: groups.clone() })
                })
                .collect()
        };
        let reactant_sides = side_of("reactants", reactants, products);
        let product_sides = side_of("products", products, reactants);

        // Resolve each pair to (side, atom number) on both ends, with an error per bad end.
        let resolve = |ref_path: JsonPath,
                       atom_ref: &AtomRefFile,
                       ids: &[String],
                       sides: &[Option<Side>],
                       role: &str,
                       errors: &mut Vec<SceneError>|
         -> Option<SideAtom> {
            let Some(side) = ids.iter().position(|id| *id == atom_ref.object) else {
                let listed: Vec<String> = ids.iter().map(|id| format!("`{id}`")).collect();
                let message = format!(
                    "`{}` is not one of this reaction's {role}s ({}); an atom map names atoms of the objects listed in `{role}s`",
                    atom_ref.object,
                    listed.join(", ")
                );
                errors.push(SceneError::new(
                    ref_path.field("object"),
                    SceneErrorCode::NotInReaction,
                    message,
                ));
                return None;
            };
            let molecule = &sides[side].as_ref()?.molecule;
            match molecule.atom_number(atom_ref.atom) {
                Some(number) => Some((side, number)),
                None => {
                    let message = format!(
                        "`{}` has {}, and the atom map names atom {}",
                        atom_ref.object,
                        molecule.numbering(),
                        atom_ref.atom
                    );
                    errors.push(SceneError::new(
                        ref_path.field("atom"),
                        SceneErrorCode::NoSuchAtom,
                        message,
                    ));
                    None
                }
            }
        };
        let mut pairs: Vec<(SideAtom, SideAtom)> = Vec::new();
        let mut mapped_reactant: BTreeMap<SideAtom, usize> = BTreeMap::new();
        let mut mapped_product: BTreeMap<SideAtom, usize> = BTreeMap::new();
        for (k, pair) in atom_map.iter().enumerate() {
            let at = path.field("atom_map").index(k);
            let r = resolve(
                at.field("reactant"),
                &pair.reactant,
                reactants,
                &reactant_sides,
                "reactant",
                &mut errors,
            );
            let p = resolve(
                at.field("product"),
                &pair.product,
                products,
                &product_sides,
                "product",
                &mut errors,
            );
            let (Some(r), Some(p)) = (r, p) else { continue };
            let (er, ep) = (
                reactant_sides[r.0]
                    .as_ref()
                    .map(|s| s.molecule.atom(r.1).element),
                product_sides[p.0]
                    .as_ref()
                    .map(|s| s.molecule.atom(p.1).element),
            );
            if let (Some(er), Some(ep)) = (er, ep)
                && er != ep
            {
                let message = format!(
                    "atom {} of `{}` is {} and atom {} of `{}` is {}; an atom keeps its element through a reaction",
                    pair.reactant.atom,
                    pair.reactant.object,
                    er.symbol(),
                    pair.product.atom,
                    pair.product.object,
                    ep.symbol()
                );
                errors.push(SceneError::new(
                    at.field("product"),
                    SceneErrorCode::ElementChanged,
                    message,
                ));
                continue;
            }
            let mut twice = |seen: &mut BTreeMap<SideAtom, usize>,
                             key,
                             side: &str,
                             who: &AtomRefFile| {
                if let Some(first) = seen.insert(key, k) {
                    let message = format!(
                        "atom {} of `{}` is already mapped by /atom_map/{first}; every {side} atom has exactly one partner",
                        who.atom, who.object
                    );
                    errors.push(SceneError::new(
                        at.field(side),
                        SceneErrorCode::AtomMappedTwice,
                        message,
                    ));
                    false
                } else {
                    true
                }
            };
            let fresh_r = twice(&mut mapped_reactant, r, "reactant", &pair.reactant);
            let fresh_p = twice(&mut mapped_product, p, "product", &pair.product);
            if !(fresh_r && fresh_p) {
                continue;
            }
            pairs.push((r, p));
        }
        // Atoms no pair names, and whether the two sides hold the same atoms.
        for (field, sides, mapped) in [
            ("reactant", &reactant_sides, &mapped_reactant),
            ("product", &product_sides, &mapped_product),
        ] {
            for (s, side) in sides.iter().enumerate() {
                let Some(side) = side else { continue };
                let missing: Vec<String> = side
                    .molecule
                    .numbers()
                    .filter(|n| !mapped.contains_key(&(s, *n)))
                    .map(|n| n.to_string())
                    .collect();
                if !missing.is_empty() {
                    let message = format!(
                        "atoms {} of {field} `{}` are not in the atom map; every atom needs a partner",
                        missing.join(", "),
                        side.object_id
                    );
                    errors.push(SceneError::new(
                        path.field("atom_map"),
                        SceneErrorCode::UnmappedAtom,
                        message,
                    ));
                }
            }
        }
        let (held_r, held_p) = (
            atoms_held(reactant_sides.iter().flatten()),
            atoms_held(product_sides.iter().flatten()),
        );
        if held_r != held_p
            && reactant_sides
                .iter()
                .chain(&product_sides)
                .all(Option::is_some)
        {
            let message = format!(
                "the reactants hold {held_r} and the products {held_p}; a reaction keeps its atoms, so add the missing molecule to one side"
            );
            errors.push(SceneError::new(
                path.field("products"),
                SceneErrorCode::AtomsNotConserved,
                message,
            ));
        }
        for (k, (r, p)) in pairs.iter().enumerate() {
            let (sr, sp) = (
                reactant_sides[r.0].as_ref().expect("resolved"),
                product_sides[p.0].as_ref().expect("resolved"),
            );
            let (hr, hp) = (
                !sr.representation.shows(sr.molecule.atom(r.1)),
                !sp.representation.shows(sp.molecule.atom(p.1)),
            );
            if hr != hp {
                let message = format!(
                    "atom {} of `{}` is {} and its partner atom {} of `{}` is {}; hide hydrogens on both sides or neither",
                    r.1,
                    sr.object_id,
                    if hr { "hidden" } else { "shown" },
                    p.1,
                    sp.object_id,
                    if hp { "hidden" } else { "shown" }
                );
                errors.push(SceneError::new(
                    path.field("atom_map").index(k).field("product"),
                    SceneErrorCode::Invalid,
                    message,
                ));
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let (reactants, products): (Vec<Side>, Vec<Side>) = (
            reactant_sides.into_iter().flatten().collect(),
            product_sides.into_iter().flatten().collect(),
        );
        Ok(Reaction::new(reactants, products, &pairs, fit))
    }

    /// The reaction of `pairs`, already checked: one pair per atom of each side. The atoms are
    /// put in reactant order and the bonds sorted into kept, broken and formed.
    fn new(
        reactants: Vec<Side>,
        products: Vec<Side>,
        pairs: &[(SideAtom, SideAtom)],
        fit: bool,
    ) -> Reaction {
        let by_reactant: BTreeMap<SideAtom, SideAtom> = pairs.iter().copied().collect();
        let mut atoms = Vec::new();
        let mut index_of_reactant = BTreeMap::new();
        let mut index_of_product = BTreeMap::new();
        for (s, side) in reactants.iter().enumerate() {
            for number in side.molecule.numbers() {
                let (ps, pn) = by_reactant[&(s, number)];
                let atom = side.molecule.atom(number);
                index_of_reactant.insert((s, number), atoms.len());
                index_of_product.insert((ps, pn), atoms.len());
                atoms.push(MappedAtom {
                    reactant: AtomRef {
                        object: side.object_id.clone(),
                        atom: number,
                    },
                    product: AtomRef {
                        object: products[ps].object_id.clone(),
                        atom: pn,
                    },
                    element: atom.element,
                    hidden: !side.representation.shows(atom),
                    sides: (s, ps),
                });
            }
        }
        let mut changes: BTreeMap<[usize; 2], (Option<BondOrder>, Option<BondOrder>)> =
            BTreeMap::new();
        for (sides, index, from) in [
            (&reactants, &index_of_reactant, true),
            (&products, &index_of_product, false),
        ] {
            for (s, side) in sides.iter().enumerate() {
                for bond in side.molecule.bonds() {
                    let [a, b] = bond.atoms.map(|n| index[&(s, n)]);
                    let entry = changes.entry([a.min(b), a.max(b)]).or_default();
                    if from {
                        entry.0 = Some(bond.order)
                    } else {
                        entry.1 = Some(bond.order)
                    }
                }
            }
        }
        let bonds = changes
            .into_iter()
            .map(|(atoms, orders)| ReactionBond {
                atoms,
                fate: match orders {
                    (Some(from), Some(to)) if from == to => BondFate::Kept { order: from },
                    (Some(from), Some(to)) => BondFate::Changed { from, to },
                    (Some(order), None) => BondFate::Broken { order },
                    (None, Some(order)) => BondFate::Formed { order },
                    (None, None) => unreachable!("a bond comes from one side at least"),
                },
            })
            .collect();
        Reaction {
            reactants,
            products,
            atoms,
            bonds,
            fit,
        }
    }

    /// The reaction's motion, given each side's transform as the timeline has it when the step
    /// begins. The products are lined up with the reactants by the best-fit rotation about their
    /// own middles, then turned by their own rotation field, and put at their own position:
    ///
    /// ```text
    /// reactant atom world position  q_i = reactant transform x atom position
    /// product atom, scaled          y_i = product scale x atom position          (about its centroid)
    /// fit                           R   = best rotation of y_i onto q_i, each about its centroid,
    ///                                     or the identity when the reaction's `fit` is off
    /// product ends                  p_i = product position + product rotation x R x y_i
    /// ```
    ///
    /// `reactants` and `products` are in the order of `self.reactants` and `self.products`, and
    /// each scale is uniform: the caller has checked it.
    pub fn motion(
        self: &Arc<Self>,
        reactants: &[Transform],
        products: &[Transform],
    ) -> ReactionBlends {
        let q: Vec<Vec3> = self
            .atoms
            .iter()
            .map(|a| {
                reactants[a.sides.0].object_to_world().transform_point3(
                    self.reactants[a.sides.0]
                        .molecule
                        .atom(a.reactant.atom)
                        .position,
                )
            })
            .collect();
        let y: Vec<Vec3> = self
            .atoms
            .iter()
            .map(|a| {
                products[a.sides.1].scale.x
                    * self.products[a.sides.1]
                        .molecule
                        .atom(a.product.atom)
                        .position
            })
            .collect();
        let (rotations, fit_rmsd) = if self.fit {
            let fit = best_rotation(&y, &q);
            (
                products
                    .iter()
                    .map(|t| euler(t.rotation) * fit.rotation)
                    .collect::<Vec<Quat>>(),
                fit.rmsd,
            )
        } else {
            let rotations: Vec<Quat> = products.iter().map(|t| euler(t.rotation)).collect();
            let turned: Vec<Vec3> = self
                .atoms
                .iter()
                .zip(&y)
                .map(|(a, y)| rotations[a.sides.1] * *y)
                .collect();
            (rotations, rmsd(&turned, &q))
        };
        let p: Vec<Vec3> = self
            .atoms
            .iter()
            .zip(&y)
            .map(|(a, y)| products[a.sides.1].position + rotations[a.sides.1] * *y)
            .collect();
        let start_rotations: Vec<Quat> = reactants.iter().map(|t| euler(t.rotation)).collect();
        let across_start: Vec<Option<Vec3>> = self
            .bonds
            .iter()
            .map(|b| {
                self.across(
                    b,
                    &self.reactants,
                    &start_rotations,
                    |a| (a.sides.0, a.reactant.atom),
                    matches!(b.fate, BondFate::Formed { .. }),
                )
            })
            .collect();
        let across_end: Vec<Option<Vec3>> = self
            .bonds
            .iter()
            .map(|b| {
                self.across(
                    b,
                    &self.products,
                    &rotations,
                    |a| (a.sides.1, a.product.atom),
                    matches!(b.fate, BondFate::Broken { .. }),
                )
            })
            .collect();
        let across = |own: &Option<Vec3>, other: &Option<Vec3>| {
            own.or(*other).expect("a bond is on one side at least")
        };
        let blend =
            |positions: Vec<Vec3>, scales: Vec<f32>, across: Vec<Vec3>, progress: f32| Blend {
                reaction: self.clone(),
                positions,
                scales,
                across,
                progress,
            };
        ReactionBlends {
            fit_rmsd,
            product_rotations: if self.fit {
                rotations.iter().map(|q| degrees(*q)).collect()
            } else {
                products.iter().map(|t| t.rotation).collect()
            },
            from: blend(
                q,
                self.atoms
                    .iter()
                    .map(|a| reactants[a.sides.0].scale.x)
                    .collect(),
                across_start
                    .iter()
                    .zip(&across_end)
                    .map(|(s, e)| across(s, e))
                    .collect(),
                0.0,
            ),
            to: blend(
                p,
                self.atoms
                    .iter()
                    .map(|a| products[a.sides.1].scale.x)
                    .collect(),
                across_end
                    .iter()
                    .zip(&across_start)
                    .map(|(e, s)| across(e, s))
                    .collect(),
                1.0,
            ),
        }
    }

    /// The world direction the sticks of `bond` spread across on one side, or `None` when the
    /// bond is not on that side (`absent`). `rotations` are the sides' rotations on that side.
    fn across(
        &self,
        bond: &ReactionBond,
        sides: &[Side],
        rotations: &[Quat],
        atom_of: impl Fn(&MappedAtom) -> SideAtom,
        absent: bool,
    ) -> Option<Vec3> {
        if absent {
            return None;
        }
        let [(side, a), (_, b)] = bond.atoms.map(|i| atom_of(&self.atoms[i]));
        Some(
            rotations[side]
                * sides[side].molecule.across(&Bond {
                    atoms: [a.min(b), a.max(b)],
                    order: BondOrder::Single,
                }),
        )
    }

    /// The glow of group `group_id` of the reactant at `reactant`, lit at `glow` when the step
    /// begins, as the reaction carries it, or a scene error at `path`'s `products` for each
    /// product that receives some of its shown atoms and has no group of exactly the atoms it
    /// receives. A product that receives only hidden atoms of the group receives no glow, because
    /// there would be nothing to draw.
    pub fn carry(
        &self,
        reactant: usize,
        group_id: &GroupId,
        glow: Glow,
        path: &JsonPath,
    ) -> Result<CarriedGlow, Vec<SceneError>> {
        let side = &self.reactants[reactant];
        let group = side
            .groups
            .get(group_id)
            .expect("a glow's group is one of its molecule's groups");
        let atoms: Vec<usize> = (0..self.atoms.len())
            .filter(|&i| {
                self.atoms[i].sides.0 == reactant
                    && group.atoms.contains(&self.atoms[i].reactant.atom)
            })
            .collect();
        let mut passes_to = Vec::new();
        let mut errors = Vec::new();
        for (p, product) in self.products.iter().enumerate() {
            let received: Vec<&MappedAtom> = atoms
                .iter()
                .map(|&i| &self.atoms[i])
                .filter(|a| a.sides.1 == p)
                .collect();
            if received.iter().all(|a| a.hidden) {
                continue;
            }
            let mut numbers: Vec<AtomNumber> = received.iter().map(|a| a.product.atom).collect();
            numbers.sort();
            match product.groups.iter().find(|(_, g)| g.atoms == numbers) {
                Some((id, _)) => passes_to.push(GroupRef {
                    object: product.object_id.clone(),
                    group: id.clone(),
                }),
                None => {
                    let listed = numbers
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ");
                    let (reactant, product) = (&side.object_id, &product.object_id);
                    let message = format!(
                        "group `{group_id}` of `{reactant}` glows when this reaction begins, and its atoms become atoms {listed} of `{product}`, which has no group of exactly those atoms; add one to the `groups` of `{product}`, such as `\"{group_id}\": [{listed}]`, or highlight `{group_id}` to 0 in an earlier step"
                    );
                    errors.push(SceneError::new(
                        path.field("products").index(p),
                        SceneErrorCode::UnknownGroup,
                        message,
                    ));
                }
            }
        }
        if errors.is_empty() {
            Ok(CarriedGlow {
                from: GroupRef {
                    object: side.object_id.clone(),
                    group: group_id.clone(),
                },
                glow,
                atoms,
                passes_to,
            })
        } else {
            Err(errors)
        }
    }
}

/// A group named by its molecule object's id and its group id.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct GroupRef {
    pub object: ObjectId,
    pub group: GroupId,
}

/// The glow of a reactant group that is lit when its reaction begins, as the reaction carries it:
/// drawn around the group's atoms in the blend while the step plays, and passed at the step's end
/// to the product group each product's share of those atoms makes up.
#[derive(Clone, Debug)]
pub struct CarriedGlow {
    pub from: GroupRef,
    pub glow: Glow,
    /// Indices into `Reaction::atoms`.
    atoms: Vec<usize>,
    /// One group per product that receives a shown atom of the group, in `Reaction::products` order.
    pub passes_to: Vec<GroupRef>,
}

/// `C2 H6 Cl` style: each element with its count, carbon first, then hydrogen, then the rest
/// alphabetically, as chemists write a formula.
fn atoms_held<'a>(sides: impl Iterator<Item = &'a Side>) -> String {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for side in sides {
        for number in side.molecule.numbers() {
            *counts
                .entry(side.molecule.atom(number).element.symbol())
                .or_default() += 1;
        }
    }
    let rank = |symbol: &str| match symbol {
        "C" => 0,
        "H" => 1,
        _ => 2,
    };
    let mut ordered: Vec<_> = counts.into_iter().collect();
    ordered.sort_by_key(|(symbol, _)| (rank(symbol), *symbol));
    ordered
        .iter()
        .map(|(symbol, n)| {
            if *n == 1 {
                symbol.to_string()
            } else {
                format!("{symbol}{n}")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn euler(degrees: Vec3) -> Quat {
    let [x, y, z] = degrees.to_array().map(f32::to_radians);
    Quat::from_euler(EulerRot::XYZ, x, y, z)
}

fn degrees(rotation: Quat) -> Vec3 {
    let (x, y, z) = rotation.to_euler(EulerRot::XYZ);
    Vec3::new(x, y, z) * (180.0 / std::f32::consts::PI)
}

// ---------------------------------------------------------------------------------------------
// Kabsch.

/// The best rotation of one set of points onto another, and how well it fits.
#[derive(Clone, Copy, Debug)]
pub struct Fit {
    pub rotation: Quat,
    /// The root mean square distance between the rotated sources and the targets, each taken
    /// about its own centroid, in the points' units.
    pub rmsd: f32,
}

/// The rotation `R` that minimizes the sum of `|R (source_i - sources' centroid) - (target_i -
/// targets' centroid)|^2`, over proper rotations only. When the points are collinear or
/// coincident the rotation is not unique, and any best one is returned.
pub fn best_rotation(sources: &[Vec3], targets: &[Vec3]) -> Fit {
    debug_assert_eq!(sources.len(), targets.len());
    let about_centroid = |points: &[Vec3]| {
        let centroid = points.iter().sum::<Vec3>() / points.len().max(1) as f32;
        points.iter().map(|p| *p - centroid).collect::<Vec<_>>()
    };
    let (sources, targets) = (about_centroid(sources), about_centroid(targets));
    let mut s = [[0.0f64; 3]; 3];
    for (a, b) in sources.iter().zip(&targets) {
        let (a, b) = (a.as_dvec3().to_array(), b.as_dvec3().to_array());
        for i in 0..3 {
            for j in 0..3 {
                s[i][j] += a[i] * b[j];
            }
        }
    }
    let n = [
        [
            s[0][0] + s[1][1] + s[2][2],
            s[1][2] - s[2][1],
            s[2][0] - s[0][2],
            s[0][1] - s[1][0],
        ],
        [
            s[1][2] - s[2][1],
            s[0][0] - s[1][1] - s[2][2],
            s[0][1] + s[1][0],
            s[2][0] + s[0][2],
        ],
        [
            s[2][0] - s[0][2],
            s[0][1] + s[1][0],
            -s[0][0] + s[1][1] - s[2][2],
            s[1][2] + s[2][1],
        ],
        [
            s[0][1] - s[1][0],
            s[2][0] + s[0][2],
            s[1][2] + s[2][1],
            -s[0][0] - s[1][1] + s[2][2],
        ],
    ];
    let q = largest_eigenvector(n);
    let rotation = Quat::from_xyzw(q[1] as f32, q[2] as f32, q[3] as f32, q[0] as f32).normalize();
    let turned: Vec<Vec3> = sources.iter().map(|a| rotation * *a).collect();
    Fit {
        rotation,
        rmsd: rmsd(&turned, &targets),
    }
}

/// The root mean square distance between `sources` and `targets`, each taken about its own
/// centroid.
fn rmsd(sources: &[Vec3], targets: &[Vec3]) -> f32 {
    let centroid = |points: &[Vec3]| points.iter().sum::<Vec3>() / points.len().max(1) as f32;
    let (a, b) = (centroid(sources), centroid(targets));
    let sum: f32 = sources
        .iter()
        .zip(targets)
        .map(|(s, t)| ((*s - a) - (*t - b)).length_squared())
        .sum();
    (sum / sources.len().max(1) as f32).sqrt()
}

/// The unit eigenvector of a symmetric 4 by 4 matrix with the largest eigenvalue, by cyclic
/// Jacobi rotations.
fn largest_eigenvector(mut a: [[f64; 4]; 4]) -> [f64; 4] {
    let mut v = [[0.0; 4]; 4];
    (0..4).for_each(|i| v[i][i] = 1.0);
    for _ in 0..64 {
        let off: f64 = (0..4)
            .flat_map(|i| (i + 1..4).map(move |j| (i, j)))
            .map(|(i, j)| a[i][j] * a[i][j])
            .sum();
        if off < 1e-30 {
            break;
        }
        for p in 0..3 {
            for q in p + 1..4 {
                if a[p][q].abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let tangent = theta.signum() / (theta.abs() + (theta * theta + 1.0).sqrt());
                let (c, s) = (
                    1.0 / (tangent * tangent + 1.0).sqrt(),
                    tangent / (tangent * tangent + 1.0).sqrt(),
                );
                for row in a.iter_mut().chain(v.iter_mut()) {
                    let (rp, rq) = (row[p], row[q]);
                    row[p] = c * rp - s * rq;
                    row[q] = s * rp + c * rq;
                }
                let (rp, rq) = (a[p], a[q]);
                a[p] = std::array::from_fn(|k| c * rp[k] - s * rq[k]);
                a[q] = std::array::from_fn(|k| s * rp[k] + c * rq[k]);
            }
        }
    }
    let best = (0..4)
        .max_by(|&i, &j| a[i][i].total_cmp(&a[j][j]))
        .expect("four eigenvalues");
    [v[0][best], v[1][best], v[2][best], v[3][best]]
}

// ---------------------------------------------------------------------------------------------
// ReactionBlends and blend.

/// A reaction placed in a scene: where every atom starts and ends, and how the products turn.
/// Made once, when the timeline reads the step, from the transforms the sides have then.
#[derive(Clone, Debug)]
pub struct ReactionBlends {
    pub from: Blend,
    pub to: Blend,
    /// How well the best-fit rotation lines the products up with the reactants, each about its
    /// centroid, in world units. Where the reactants are placed enters it, so it compares atom
    /// maps for one placement: the lower, the closer the map pairs atoms that lie alike. With the
    /// fit off, it is how far apart the products turned by their own rotations lie from the
    /// reactants, the same way.
    pub fit_rmsd: f32,
    /// The rotation each product ends with, in degrees, in `Reaction::products` order.
    pub product_rotations: Vec<Vec3>,
}

/// Every atom of a reaction in world space at one progress. Two blends mix by straight-line
/// interpolation, so `Track<Blend>` is a reaction's track.
#[derive(Clone, Debug)]
pub struct Blend {
    reaction: Arc<Reaction>,
    positions: Vec<Vec3>,
    scales: Vec<f32>,
    /// Per bond: the world direction its sticks spread across.
    across: Vec<Vec3>,
    progress: f32,
}

impl Mix for Blend {
    fn mix(self, to: Blend, progress: Progress) -> Blend {
        let p = progress.get();
        let lerp = |a: Vec3, b: Vec3| a + (b - a) * p;
        Blend {
            reaction: self.reaction,
            positions: self
                .positions
                .iter()
                .zip(&to.positions)
                .map(|(a, b)| lerp(*a, *b))
                .collect(),
            scales: self
                .scales
                .iter()
                .zip(&to.scales)
                .map(|(a, b)| a + (b - a) * p)
                .collect(),
            across: self
                .across
                .iter()
                .zip(&to.across)
                .map(|(a, b)| lerp(*a, *b))
                .collect(),
            progress: self.progress + (to.progress - self.progress) * p,
        }
    }
}

impl Blend {
    /// Balls and sticks in world space, as the ball-and-stick representation draws them, with
    /// each bond as `BondFate::look_at` says.
    pub fn draw(&self) -> Drawn {
        let reaction = &self.reaction;
        let mut drawn = Drawn {
            balls: Vec::new(),
            sticks: Vec::new(),
            tubes: Vec::new(),
        };
        for (i, atom) in reaction.atoms.iter().enumerate().filter(|(_, a)| !a.hidden) {
            drawn.balls.push(Ball {
                center: self.positions[i],
                radius: atom.element.radius() * BALL_SHARE * self.scales[i],
                color: atom.element.color(),
            });
        }
        for (bond, across) in reaction.bonds.iter().zip(&self.across) {
            let [a, b] = bond.atoms;
            if reaction.atoms[a].hidden || reaction.atoms[b].hidden {
                continue;
            }
            let look = bond.fate.look_at(self.progress);
            let scale = (self.scales[a] + self.scales[b]) / 2.0;
            let radius = look.radius * look.weight * scale;
            let [pa, pb] = [self.positions[a], self.positions[b]];
            let midpoint = (pa + pb) / 2.0;
            for offset in &look.offsets {
                let shift = *across * *offset * STICK_SPACING * scale;
                for (end, atom) in [(pa, a), (pb, b)] {
                    drawn.sticks.push(Stick {
                        from: end + shift,
                        to: midpoint + shift,
                        radius,
                        color: reaction.atoms[atom].element.color(),
                    });
                }
            }
        }
        drawn
    }

    /// The shell of a carried glow in world space, as `Representation::glow` makes it on a
    /// molecule: a ball a padding larger around each shown atom of the group, and a tube along
    /// each bond between two of them, at the bond's weight. At progress 0 it is the reactant
    /// group's shell and at progress 1 the product groups'.
    pub fn glow(&self, carried: &CarriedGlow) -> Drawn {
        let reaction = &self.reaction;
        let color = carried.glow.color;
        let shown = |i: &usize| !reaction.atoms[*i].hidden;
        let balls = carried
            .atoms
            .iter()
            .filter(|i| shown(i))
            .map(|&i| Ball {
                center: self.positions[i],
                radius: (reaction.atoms[i].element.radius() * BALL_SHARE
                    + BALL_AND_STICK_GLOW.padding)
                    * self.scales[i],
                color,
            })
            .collect();
        let sticks = reaction
            .bonds
            .iter()
            .filter(|bond| {
                bond.atoms
                    .iter()
                    .all(|i| carried.atoms.contains(i) && shown(i))
            })
            .map(|bond| {
                let [a, b] = bond.atoms;
                let radius = GLOW_BOND_RADIUS
                    * bond.fate.look_at(self.progress).weight
                    * (self.scales[a] + self.scales[b])
                    / 2.0;
                Stick {
                    from: self.positions[a],
                    to: self.positions[b],
                    radius,
                    color,
                }
            })
            .filter(|stick| stick.radius > 0.0)
            .collect();
        Drawn {
            balls,
            sticks,
            tubes: Vec::new(),
        }
    }
}

impl fmt::Display for AtomRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.object, self.atom)
    }
}

// ---------------------------------------------------------------------------------------------
// What `inspect` says.

/// One reaction as `inspect` reports it at a scene time. Atoms are named as the atom map names
/// them: by the reactant's object id and atom number, with the product atom it becomes beside it.
#[derive(Debug, Serialize)]
pub struct ReactionReport {
    pub phase: crate::scene::Phase,
    /// How well the best-fit rotation lines the products up with the reactants, in world units.
    /// With the fit off, how far apart the products turned by their own rotations lie from them.
    pub fit_rmsd: f32,
    pub atoms: Vec<BlendAtomReport>,
    pub bonds: Vec<BlendBondReport>,
    /// The glows the reaction carries through its step, one per reactant group lit when the step
    /// begins, at any scene time.
    pub glows: Vec<CarriedGlowReport>,
    pub bounding_box: Option<crate::mesh::BoundingBox>,
    pub in_frame: crate::mesh::InFrame,
}

#[derive(Debug, Serialize)]
pub struct BlendAtomReport {
    pub reactant: AtomRef,
    pub product: AtomRef,
    pub element: Element,
    /// In world units, where the atom is at this scene time.
    pub position: Vec3,
    /// `None` when the atom is behind the camera.
    pub pixels: Option<glam::Vec2>,
    /// The radius of the atom's ball in pixels, as the frame draws it at this scene time, its
    /// scale moving from its reactant's to its product's. Left out when the atom is hidden or
    /// behind the camera.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ball_radius: Option<f32>,
    pub hidden: bool,
}

#[derive(Debug, Serialize)]
pub struct CarriedGlowReport {
    #[serde(flatten)]
    pub from: GroupRef,
    /// The glow share the group has when the step begins, and its color.
    pub glow: f32,
    pub color: String,
    /// The product groups that glow from the step's end.
    pub passes_to: Vec<GroupRef>,
}

impl CarriedGlow {
    pub fn report(&self) -> CarriedGlowReport {
        CarriedGlowReport {
            from: self.from.clone(),
            glow: self.glow.share.get(),
            color: self.glow.color.hex(),
            passes_to: self.passes_to.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct BlendBondReport {
    /// The reactant atoms at its two ends, lower index first.
    pub atoms: [AtomRef; 2],
    pub fate: BondFate,
    /// How thick its sticks are, as a share of full: 1 for a kept or changed bond, falling to 0
    /// for a broken one and rising from 0 for a formed one.
    pub weight: f32,
    pub hidden: bool,
}

impl Blend {
    pub fn report(
        &self,
        to_pixels: impl Fn(Vec3) -> Option<glam::Vec2>,
        pixels_per_unit: impl Fn(Vec3) -> f32,
    ) -> (Vec<BlendAtomReport>, Vec<BlendBondReport>) {
        let reaction = &self.reaction;
        let atoms = reaction
            .atoms
            .iter()
            .zip(&self.positions)
            .zip(&self.scales)
            .map(|((a, &position), &scale)| {
                let pixels = to_pixels(position);
                BlendAtomReport {
                    reactant: a.reactant.clone(),
                    product: a.product.clone(),
                    element: a.element,
                    position,
                    pixels,
                    ball_radius: pixels.filter(|_| !a.hidden).map(|_| {
                        a.element.radius() * BALL_SHARE * scale * pixels_per_unit(position)
                    }),
                    hidden: a.hidden,
                }
            })
            .collect();
        let bonds = reaction
            .bonds
            .iter()
            .map(|b| BlendBondReport {
                atoms: b.atoms.map(|i| reaction.atoms[i].reactant.clone()),
                fate: b.fate,
                weight: b.fate.look_at(self.progress).weight,
                hidden: b.atoms.iter().any(|&i| reaction.atoms[i].hidden),
            })
            .collect();
        (atoms, bonds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn side(id: &str, file: &str) -> Side {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/pubchem")
            .join(file);
        Side {
            object_id: ObjectId::new(id),
            molecule: Arc::new(crate::structure_file::read(&path).unwrap()),
            representation: Representation::BallAndStick { hydrogens: true },
            groups: Groups::default(),
        }
    }

    #[test]
    fn points_away_from_their_centroid_fit_onto_a_turned_and_moved_copy() {
        let sources = [
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(1.0, 2.0, 0.0),
            Vec3::new(1.0, 0.0, 1.0),
        ];
        let turn = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let targets = sources.map(|p| turn * p + Vec3::new(5.0, -2.0, 1.0));

        let fit = best_rotation(&sources, &targets);

        assert!(fit.rmsd < 1e-4, "{}", fit.rmsd);
        assert!(
            fit.rotation.angle_between(turn) < 1e-3,
            "{:?}",
            fit.rotation
        );
    }

    /// Ethylene at (0, -0.5, 0) and water at (-1, 2, 0) turn into ethanol.
    fn hydration_fit_rmsd(pairs: &[(usize, u32, u32)]) -> f32 {
        let reactants = vec![side("ethylene", "ethylene.sdf"), side("water", "water.sdf")];
        let products = vec![side("ethanol", "ethanol.sdf")];
        let number = |s: &Side, n: u32| s.molecule.atom_number(n).unwrap();
        let pairs: Vec<(SideAtom, SideAtom)> = pairs
            .iter()
            .map(|&(r, a, b)| ((r, number(&reactants[r], a)), (0, number(&products[0], b))))
            .collect();
        let at = |x: f32, y: f32| Transform {
            position: Vec3::new(x, y, 0.0),
            ..Transform::default()
        };
        Arc::new(Reaction::new(reactants, products, &pairs, true))
            .motion(&[at(0.0, -0.5), at(-1.0, 2.0)], &[Transform::default()])
            .fit_rmsd
    }

    #[test]
    fn the_right_atom_map_fits_closer_than_one_that_swaps_a_carbon_with_the_oxygen() {
        let right = [
            (0, 1, 2),
            (0, 2, 3),
            (0, 3, 4),
            (0, 4, 5),
            (0, 5, 6),
            (0, 6, 7),
            (1, 1, 1),
            (1, 2, 9),
            (1, 3, 8),
        ];
        let mut swapped = right;
        (swapped[0].2, swapped[6].2) = (1, 2);

        let (right, swapped) = (hydration_fit_rmsd(&right), hydration_fit_rmsd(&swapped));

        assert!((right - 1.3697).abs() < 1e-3, "{right}");
        assert!((swapped - 1.5878).abs() < 1e-3, "{swapped}");
    }
}
