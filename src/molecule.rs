//! The domain model of a molecule and of its representations, ball-and-stick and space-filling.
//! Everything here is validated data: `structure_file` is the only module that builds a
//! `Molecule`, and `mesh` turns the balls and sticks a representation draws into triangles.
//!
//! The units are angstroms, which are world units until the object's scale says otherwise, and
//! the atoms are placed about their centroid, so the object's position is the molecule's middle
//! and a rotation turns it in place.

use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroU32;
use std::sync::OnceLock;

use glam::{Mat3, Mat4, Vec2, Vec3};
use serde::Serialize;

use crate::cartoon::{Cartoon, CartoonTube};
use crate::element::Element;
use crate::group::{GroupId, GroupReport};
use crate::scene::Color;

/// An atom's number in the structure file: its place in an SDF file, counting from 1, or its
/// serial number in a PDB file. Only `Molecule::numbered` makes one, so holding an `AtomNumber`
/// means the atom exists. It carries the atom's place in the molecule, so finding the atom needs
/// no search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AtomNumber {
    number: NonZeroU32,
    index: u32,
}

impl AtomNumber {
    pub fn get(self) -> u32 {
        self.number.get()
    }

    fn index(self) -> usize {
        self.index as usize
    }
}

impl fmt::Display for AtomNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.number.fmt(f)
    }
}

impl Serialize for AtomNumber {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u32(self.get())
    }
}

/// The letter a PDB file names a chain by, such as `A`. A file that names none gives a space.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChainId(pub char);

impl Serialize for ChainId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

impl fmt::Display for ChainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A residue's number in its chain, with the insertion code that tells apart two residues a file
/// gives one number, as `52` and `52A`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ResidueNumber {
    pub number: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub insertion_code: Option<char>,
}

impl fmt::Display for ResidueNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.number)?;
        self.insertion_code
            .map_or(Ok(()), |code| write!(f, "{code}"))
    }
}

/// A residue whose bonds a PDB file leaves out and sci_anim finds from its atoms' distances: one
/// of the 20 amino acids, or a nucleotide of DNA or RNA.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandardResidue {
    AminoAcid,
    Nucleotide,
}

impl StandardResidue {
    const AMINO_ACIDS: [&str; 20] = [
        "ALA", "ARG", "ASN", "ASP", "CYS", "GLN", "GLU", "GLY", "HIS", "ILE", "LEU", "LYS", "MET",
        "PHE", "PRO", "SER", "THR", "TRP", "TYR", "VAL",
    ];
    const NUCLEOTIDES: [&str; 9] = ["DA", "DC", "DG", "DT", "DU", "A", "C", "G", "U"];

    /// What a residue of the residue name `name` is, if it is standard.
    pub fn of(name: &str) -> Option<StandardResidue> {
        if Self::AMINO_ACIDS.contains(&name) {
            Some(StandardResidue::AminoAcid)
        } else if Self::NUCLEOTIDES.contains(&name) {
            Some(StandardResidue::Nucleotide)
        } else {
            None
        }
    }

    /// The atom names of the bond from one residue to the next in its chain: the peptide bond
    /// from an amino acid's `C` to the next one's `N`, and the phosphodiester bond from a
    /// nucleotide's `O3'` to the next one's `P`.
    pub fn next_bond(self) -> (&'static str, &'static str) {
        match self {
            StandardResidue::AminoAcid => ("C", "N"),
            StandardResidue::Nucleotide => ("O3'", "P"),
        }
    }
}

/// Whether the atom of atom name `atom_name` in a nucleotide is of its nucleobase: not of its
/// sugar, whose atom names carry a prime, as `C1'`, and not of its phosphate, `P` and the
/// oxygens `OP1` to `OP3`, which older files write `O1P` to `O3P`.
pub fn in_nucleobase(atom_name: &str) -> bool {
    const PHOSPHATE: [&str; 7] = ["P", "OP1", "OP2", "OP3", "O1P", "O2P", "O3P"];
    !atom_name.contains('\'') && !PHOSPHATE.contains(&atom_name)
}

/// A stretch of one chain that a PDB file's `HELIX` or `SHEET` record says folds into a regular
/// shape: residues `from` to `to` of `chain`, both included.
#[derive(Clone, Debug, PartialEq)]
pub struct SecondaryStructure {
    pub kind: SecondaryStructureKind,
    pub chain: ChainId,
    pub from: ResidueNumber,
    pub to: ResidueNumber,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SecondaryStructureKind {
    /// A `HELIX` record: its helix id and its class, 1 for an α-helix and 5 for a 3₁₀ helix.
    Helix { id: String, class: u8 },
    /// A `SHEET` record: one β-strand, by the sheet id of its β-sheet and its strand number there.
    Strand { sheet: String, strand: u32 },
}

/// One residue of a chain in a PDB file: an amino acid or a nucleotide, or a ligand or ion the
/// file names as a residue. `A` is how its atoms are known: by `AtomNumber` in a molecule, and by
/// the number the file gives each while the molecule is made.
#[derive(Clone, Debug, PartialEq)]
pub struct Residue<A = AtomNumber> {
    pub chain: ChainId,
    pub number: ResidueNumber,
    /// The residue name, such as `DG` or `ALA`.
    pub name: String,
    /// Its atoms in file order, each with its atom name, such as `CA` or `N1`.
    pub atoms: Vec<(String, A)>,
}

#[derive(Clone, Copy, Debug)]
pub struct Atom {
    pub number: AtomNumber,
    pub element: Element,
    /// In angstroms, from the molecule's centroid.
    pub position: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BondOrder {
    Single,
    Double,
    Triple,
}

/// The sticks the ball-and-stick representation draws a bond of an order with: where each lies
/// across the bond, in multiples of `STICK_SPACING`, and how thick each is, in angstroms.
pub(crate) struct Spread {
    pub(crate) offsets: &'static [f32],
    pub(crate) radius: f32,
}

impl BondOrder {
    pub(crate) fn spread(self) -> Spread {
        match self {
            BondOrder::Single => Spread {
                offsets: &[0.0],
                radius: 0.12,
            },
            BondOrder::Double => Spread {
                offsets: &[-0.5, 0.5],
                radius: 0.075,
            },
            BondOrder::Triple => Spread {
                offsets: &[-1.0, 0.0, 1.0],
                radius: 0.06,
            },
        }
    }
}

/// A link between two atoms, with the lower number first. Two atoms have at most one bond.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Bond {
    pub atoms: [AtomNumber; 2],
    pub order: BondOrder,
}

/// The atoms and bonds of one structure file, and the residues of a PDB file.
#[derive(Debug)]
pub struct Molecule {
    /// In atom number order, which is file order.
    atoms: Vec<Atom>,
    /// In structure file order.
    bonds: Vec<Bond>,
    /// Every atom's bonded atoms in number order, indexed like `atoms`. Which way a double bond
    /// lies is decided from these.
    neighbors: Vec<Vec<AtomNumber>>,
    /// In file order. Empty for an SDF file, which has none.
    residues: Vec<Residue>,
    /// In file order: the `HELIX` records, then the `SHEET` records. Empty for an SDF file.
    secondary_structure: Vec<SecondaryStructure>,
    /// The cartoon representation's tubes, made the first time a cartoon draws the molecule.
    cartoon: OnceLock<Cartoon>,
}

#[derive(Debug, PartialEq)]
pub enum MoleculeError {
    NoAtoms,
    NumbersNotIncreasing { atom: u32, after: u32 },
    NotFinite { atom: u32 },
    BondToMissingAtom { atom: u32, atoms: usize },
    BondToSelf { atom: u32 },
    BondRepeated { atoms: [u32; 2] },
}

impl fmt::Display for MoleculeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoleculeError::NoAtoms => write!(f, "it has no atoms, so there is nothing to draw"),
            MoleculeError::NumbersNotIncreasing { atom, after } => write!(
                f,
                "atom {atom} comes after atom {after}, and atom numbers must increase through the file"
            ),
            MoleculeError::NotFinite { atom } => write!(
                f,
                "atom {atom} has a coordinate that is not a finite number"
            ),
            MoleculeError::BondToMissingAtom { atom, atoms } => write!(
                f,
                "a bond joins atom {atom}, but the file has {atoms} atoms"
            ),
            MoleculeError::BondToSelf { atom } => write!(f, "a bond joins atom {atom} to itself"),
            MoleculeError::BondRepeated { atoms: [a, b] } => {
                write!(f, "atoms {a} and {b} are bonded twice")
            }
        }
    }
}

impl Molecule {
    /// A molecule from atoms in file order, with the position the file gives each in angstroms,
    /// and bonds as the file numbers their two atoms. The atoms are moved so that their centroid,
    /// counting every atom, is the origin.
    pub fn new(
        atoms: Vec<(Element, [f64; 3])>,
        bonds: Vec<(u32, u32, BondOrder)>,
    ) -> Result<Molecule, MoleculeError> {
        let atoms = (1..).zip(atoms).map(|(n, (e, p))| (n, e, p)).collect();
        Molecule::numbered(atoms, bonds, Vec::new())
    }

    /// A molecule from atoms in file order, each with the number the file gives it, which must
    /// increase through the file, and residues that name their atoms by those numbers. Otherwise
    /// as `new`.
    pub fn numbered(
        atoms: Vec<(u32, Element, [f64; 3])>,
        bonds: Vec<(u32, u32, BondOrder)>,
        residues: Vec<Residue<u32>>,
    ) -> Result<Molecule, MoleculeError> {
        if atoms.is_empty() {
            return Err(MoleculeError::NoAtoms);
        }
        let mut after = 0;
        for &(atom, ..) in &atoms {
            if atom <= after {
                return Err(MoleculeError::NumbersNotIncreasing { atom, after });
            }
            after = atom;
        }
        if let Some((atom, ..)) = atoms
            .iter()
            .find(|(_, _, position)| position.iter().any(|c| !c.is_finite()))
        {
            return Err(MoleculeError::NotFinite { atom: *atom });
        }
        let centroid = atoms
            .iter()
            .fold([0.0; 3], |sum, (_, _, p)| {
                [sum[0] + p[0], sum[1] + p[1], sum[2] + p[2]]
            })
            .map(|s| s / atoms.len() as f64);
        let count = atoms.len();
        let atoms: Vec<Atom> = atoms
            .into_iter()
            .enumerate()
            .map(|(i, (n, element, p))| Atom {
                number: AtomNumber {
                    number: NonZeroU32::new(n).expect("numbers increase from above 0"),
                    index: i as u32,
                },
                element,
                position: Vec3::new(
                    (p[0] - centroid[0]) as f32,
                    (p[1] - centroid[1]) as f32,
                    (p[2] - centroid[2]) as f32,
                ),
            })
            .collect();
        let mut neighbors = vec![Vec::new(); count];
        let mut made: Vec<Bond> = Vec::with_capacity(bonds.len());
        for (a, b, order) in bonds {
            let in_file = |n: u32| {
                find(&atoms, n).ok_or(MoleculeError::BondToMissingAtom {
                    atom: n,
                    atoms: count,
                })
            };
            let (a, b) = (in_file(a)?, in_file(b)?);
            if a == b {
                return Err(MoleculeError::BondToSelf { atom: a.get() });
            }
            let atoms = if a < b { [a, b] } else { [b, a] };
            if made.iter().any(|bond| bond.atoms == atoms) {
                return Err(MoleculeError::BondRepeated {
                    atoms: atoms.map(AtomNumber::get),
                });
            }
            neighbors[a.index()].push(b);
            neighbors[b.index()].push(a);
            made.push(Bond { atoms, order });
        }
        neighbors.iter_mut().for_each(|n| n.sort());
        let residues = residues
            .into_iter()
            .map(|residue| Residue {
                chain: residue.chain,
                number: residue.number,
                name: residue.name,
                atoms: residue
                    .atoms
                    .into_iter()
                    .map(|(atom_name, n)| {
                        (
                            atom_name,
                            find(&atoms, n).expect("a residue's atoms are atoms"),
                        )
                    })
                    .collect(),
            })
            .collect();
        Ok(Molecule {
            atoms,
            bonds: made,
            neighbors,
            residues,
            secondary_structure: Vec::new(),
            cartoon: OnceLock::new(),
        })
    }

    /// The molecule with the secondary structure its PDB file gives.
    pub fn with_secondary_structure(
        self,
        secondary_structure: Vec<SecondaryStructure>,
    ) -> Molecule {
        Molecule {
            secondary_structure,
            ..self
        }
    }

    /// The `HELIX` records of a PDB file, then its `SHEET` records, each in file order. None for
    /// an SDF file, or a PDB file that gives none.
    pub fn secondary_structure(&self) -> &[SecondaryStructure] {
        &self.secondary_structure
    }

    /// The tubes a cartoon draws of the molecule, made once.
    pub fn cartoon(&self) -> &Cartoon {
        self.cartoon.get_or_init(|| Cartoon::new(self))
    }

    /// The atom number `n` names, if the molecule has an atom of that number.
    pub fn atom_number(&self, n: u32) -> Option<AtomNumber> {
        find(&self.atoms, n)
    }

    pub fn atom_count(&self) -> usize {
        self.atoms.len()
    }

    /// How many atoms there are and how they are numbered, as an error message says it: "24
    /// atoms, numbered 1 to 24", or for a PDB file whose numbers skip some, "486 atoms, numbered
    /// from 1 to 487 with gaps".
    pub fn numbering(&self) -> String {
        let count = self.atoms.len();
        let (first, last) = (self.atoms[0].number, self.atoms[count - 1].number);
        if first.get() == 1 && last.get() as usize == count {
            format!("{count} atoms, numbered 1 to {count}")
        } else {
            format!("{count} atoms, numbered from {first} to {last} with gaps")
        }
    }

    /// The residues of a PDB file in file order. None for an SDF file.
    pub fn residues(&self) -> &[Residue] {
        &self.residues
    }

    /// Every atom's number, in order.
    pub fn numbers(&self) -> impl Iterator<Item = AtomNumber> + '_ {
        self.atoms.iter().map(|a| a.number)
    }

    pub fn bonds(&self) -> &[Bond] {
        &self.bonds
    }

    pub fn atom(&self, number: AtomNumber) -> &Atom {
        &self.atoms[number.index()]
    }

    /// The unit vector across `bond` along which its sticks are spread. It lies in the plane of
    /// the bond and the first other atom bonded to either end, as a chemist draws a double bond
    /// in a flat ring, and it never depends on which atoms a representation shows. A bond with no
    /// such atom, or whose others lie along it, as in acetylene, gets any direction across it.
    pub(crate) fn across(&self, bond: &Bond) -> Vec3 {
        let [a, b] = bond.atoms.map(|n| self.atom(n).position);
        let along = (b - a).normalize_or_zero();
        let others = bond.atoms.into_iter().flat_map(|n| {
            self.neighbors[n.index()]
                .iter()
                .map(move |&other| (n, other))
        });
        others
            .filter(|(_, other)| !bond.atoms.contains(other))
            .map(|(end, other)| self.atom(other).position - self.atom(end).position)
            .map(|toward| toward - along * toward.dot(along))
            .find(|across| across.length() > 1e-3)
            .map_or_else(|| along.any_orthonormal_vector(), Vec3::normalize)
    }

    /// The sticks that draw `bond`: for each of the sticks its order spreads across it, one from
    /// each atom to the bond's midpoint, in that atom's element color.
    fn sticks(&self, bond: &Bond) -> impl Iterator<Item = Stick> {
        let ends = bond.atoms.map(|n| *self.atom(n));
        let midpoint = (ends[0].position + ends[1].position) / 2.0;
        let across = self.across(bond);
        let spread = bond.order.spread();
        spread.offsets.iter().flat_map(move |&offset| {
            let shift = across * offset * STICK_SPACING;
            ends.map(|atom| Stick {
                from: atom.position + shift,
                to: midpoint + shift,
                radius: spread.radius,
                color: atom.element.color(),
            })
        })
    }
}

/// The number of the atom numbered `n` in `atoms`, which are in number order.
fn find(atoms: &[Atom], n: u32) -> Option<AtomNumber> {
    atoms
        .binary_search_by_key(&n, |atom| atom.number.get())
        .ok()
        .map(|i| atoms[i].number)
}

/// The distance between the sticks of a double or triple bond in ball-and-stick, in angstroms.
pub(crate) const STICK_SPACING: f32 = 0.18;

/// A ball of the ball-and-stick representation is this share of its element's van der Waals
/// radius, so the sticks between balls show.
pub(crate) const BALL_SHARE: f32 = 0.25;

/// The way a molecule is drawn, and whether its hydrogens are shown. It changes how a molecule
/// looks and never which atoms and bonds it has: a hidden hydrogen is still an atom, with a
/// position that `inspect` reports. Each representation owns what it draws of the molecule: the
/// size of its balls, whether it draws sticks, and the look of a lit group's glow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Representation {
    /// Each shown atom a ball a quarter of its van der Waals radius, and each bond between two
    /// shown atoms the sticks of its bond order, so a reader sees which atoms are bonded.
    BallAndStick { hydrogens: bool },
    /// Each shown atom a ball at its van der Waals radius, overlapping its bonded neighbors, and
    /// no sticks, so a molecule of thousands of atoms reads as the shape it fills.
    SpaceFilling { hydrogens: bool },
    /// The backbone of each chain as one tube, shaped by the secondary structure of a PDB file. It
    /// draws no atom as a ball, so every atom is hidden.
    Cartoon,
}

impl Representation {
    /// Whether `atom` is drawn. The one place that decides it: `draw` and the report use it.
    pub fn shows(self, atom: &Atom) -> bool {
        match self {
            Representation::BallAndStick { hydrogens }
            | Representation::SpaceFilling { hydrogens } => {
                hydrogens || atom.element != Element::HYDROGEN
            }
            Representation::Cartoon => false,
        }
    }

    /// Whether `bond` is drawn as sticks: in ball-and-stick, when both its atoms are shown.
    /// Space-filling draws no bond.
    pub fn shows_bond(self, molecule: &Molecule, bond: &Bond) -> bool {
        matches!(self, Representation::BallAndStick { .. })
            && bond.atoms.iter().all(|&n| self.shows(molecule.atom(n)))
    }

    /// The radius of `atom`'s ball in angstroms, in the molecule's own space. `None` when the atom
    /// is hidden. The one place that sizes a ball: `draw`, the glow and the report use it.
    pub fn ball_radius(self, atom: &Atom) -> Option<f32> {
        let share = match self {
            Representation::BallAndStick { .. } => BALL_SHARE,
            Representation::SpaceFilling { .. } => 1.0,
            Representation::Cartoon => return None,
        };
        self.shows(atom).then(|| atom.element.radius() * share)
    }

    /// The value of the scene file's `representation` that names it, which `inspect` reports.
    pub fn name(self) -> &'static str {
        match self {
            Representation::BallAndStick { .. } => "ball-and-stick",
            Representation::SpaceFilling { .. } => "space-filling",
            Representation::Cartoon => "cartoon",
        }
    }

    pub fn draw(self, molecule: &Molecule) -> Drawn {
        if self == Representation::Cartoon {
            return Drawn {
                balls: Vec::new(),
                sticks: Vec::new(),
                tubes: molecule.cartoon().tubes(),
            };
        }
        Drawn {
            balls: molecule
                .atoms
                .iter()
                .filter_map(|atom| {
                    Some(Ball {
                        center: atom.position,
                        radius: self.ball_radius(atom)?,
                        color: atom.element.color(),
                    })
                })
                .collect(),
            sticks: molecule
                .bonds
                .iter()
                .filter(|bond| self.shows_bond(molecule, bond))
                .flat_map(|bond| molecule.sticks(bond))
                .collect(),
            tubes: Vec::new(),
        }
    }

    /// What `inspect` says of a molecule placed by `object_to_world`: each atom and bond, and
    /// whether it is hidden. `to_pixels` maps a world point to the pixel it lands on, and
    /// `pixels_per_unit` says how many pixels one world unit spans there.
    pub fn report(
        self,
        molecule: &Molecule,
        object_to_world: Mat4,
        to_pixels: impl Fn(Vec3) -> Option<Vec2>,
        pixels_per_unit: impl Fn(Vec3) -> f32,
    ) -> MoleculeReport {
        let stretch = largest_stretch(object_to_world);
        let atoms = molecule
            .atoms
            .iter()
            .map(|atom| {
                let position = object_to_world.transform_point3(atom.position);
                let pixels = to_pixels(position);
                AtomReport {
                    number: atom.number,
                    element: atom.element,
                    position,
                    pixels,
                    ball_radius: pixels
                        .and(self.ball_radius(atom))
                        .map(|radius| radius * stretch * pixels_per_unit(position)),
                    hidden: !self.shows(atom),
                }
            })
            .collect();
        let bonds = molecule
            .bonds
            .iter()
            .map(|bond| BondReport {
                atoms: bond.atoms,
                order: bond.order,
                hidden: !self.shows_bond(molecule, bond),
            })
            .collect();
        let mut chains: Vec<ChainReport> = Vec::new();
        for residue in &molecule.residues {
            let report = ResidueReport {
                number: residue.number.number,
                insertion_code: residue.number.insertion_code,
                name: residue.name.clone(),
                atoms: AtomNames(residue.atoms.clone()),
            };
            match chains.iter_mut().find(|c| c.chain == residue.chain) {
                Some(chain) => chain.residues.push(report),
                None => chains.push(ChainReport {
                    chain: residue.chain,
                    residues: vec![report],
                }),
            }
        }
        let secondary_structure = molecule
            .secondary_structure
            .iter()
            .map(|stretch| SecondaryStructureReport {
                kind: stretch.kind.clone(),
                chain: stretch.chain,
                from: stretch.from.number,
                from_insertion_code: stretch.from.insertion_code,
                to: stretch.to.number,
                to_insertion_code: stretch.to.insertion_code,
            })
            .collect();
        MoleculeReport {
            representation: self.name(),
            atoms,
            bonds,
            chains,
            secondary_structure,
            groups: BTreeMap::new(),
        }
    }
}

/// The most `object_to_world` stretches any direction, so a ball it places, which a scale of
/// unequal factors draws as an ellipsoid, fits in a sphere of its radius times this.
pub fn largest_stretch(object_to_world: Mat4) -> f32 {
    let turn_and_scale = Mat3::from_mat4(object_to_world);
    [
        turn_and_scale.x_axis,
        turn_and_scale.y_axis,
        turn_and_scale.z_axis,
    ]
    .map(Vec3::length)
    .into_iter()
    .fold(0.0, f32::max)
}

/// Whether a ball the frame shows as a disc of `radius` pixels about `center` reaches into the
/// pixel box from `min` to `max`: its center is in the box or nearer it than `radius`.
pub fn ball_meets_box(center: Vec2, radius: f32, min: Vec2, max: Vec2) -> bool {
    let nearest = center.clamp(min, max);
    nearest == center || center.distance(nearest) < radius
}

/// A sphere: how a representation draws an atom.
#[derive(Clone, Copy, Debug)]
pub struct Ball {
    pub center: Vec3,
    pub radius: f32,
    pub color: Color,
}

/// A cylinder: how the ball-and-stick representation draws half of one stick of a bond. It runs
/// from an atom's center to the midpoint of the bond, in the atom's element color.
#[derive(Clone, Copy, Debug)]
pub struct Stick {
    pub from: Vec3,
    pub to: Vec3,
    pub radius: f32,
    pub color: Color,
}

/// Everything a representation draws of a molecule, in the molecule's own space: the balls and
/// sticks of ball-and-stick, and the tubes of a cartoon.
#[derive(Debug)]
pub struct Drawn {
    pub balls: Vec<Ball>,
    pub sticks: Vec<Stick>,
    pub tubes: Vec<CartoonTube>,
}

#[derive(Debug, Serialize)]
pub struct MoleculeReport {
    /// `ball-and-stick` or `space-filling`.
    representation: &'static str,
    atoms: Vec<AtomReport>,
    bonds: Vec<BondReport>,
    /// The chains of a PDB file in file order, each with its residues. Left out for an SDF file.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    chains: Vec<ChainReport>,
    /// Each helix and β-strand a PDB file's `HELIX` and `SHEET` records give, with its residue
    /// range. Left out when there are none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    secondary_structure: Vec<SecondaryStructureReport>,
    /// Each atom group with its center and box, in group id order. Left out when there are none.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) groups: BTreeMap<GroupId, GroupReport>,
}

/// One atom as `inspect` reports it, so an agent can put a label beside it.
#[derive(Debug, Serialize)]
pub struct AtomReport {
    number: AtomNumber,
    element: Element,
    /// In world units.
    position: Vec3,
    /// Where the atom's center lands in the frame, in pixels from the top left. `None` when it
    /// is behind the camera.
    pixels: Option<Vec2>,
    /// The radius of the atom's ball in pixels, as the frame draws it at the molecule's scale.
    /// Left out when the atom is hidden or behind the camera.
    #[serde(skip_serializing_if = "Option::is_none")]
    ball_radius: Option<f32>,
    hidden: bool,
}

/// One chain as `inspect` reports it, so an agent can write an atom group by its residues.
#[derive(Debug, Serialize)]
pub struct ChainReport {
    chain: ChainId,
    residues: Vec<ResidueReport>,
}

/// One stretch of secondary structure as `inspect` reports it, so an agent can write an atom group
/// of its residues: `{ "kind": "helix", "id": "H1", "class": 1, "chain": "A", "from": 23, "to": 34 }`
/// or `{ "kind": "strand", "sheet": "BET", "strand": 1, "chain": "A", "from": 10, "to": 17 }`.
#[derive(Debug, Serialize)]
pub struct SecondaryStructureReport {
    #[serde(flatten)]
    kind: SecondaryStructureKind,
    chain: ChainId,
    from: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    from_insertion_code: Option<char>,
    to: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    to_insertion_code: Option<char>,
}

#[derive(Debug, Serialize)]
pub struct ResidueReport {
    number: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    insertion_code: Option<char>,
    /// The residue name, such as `DG` or `ALA`.
    name: String,
    /// Each atom's atom name with its atom number, in file order.
    atoms: AtomNames,
}

/// A residue's atoms, written as an object from atom name to atom number, as `{ "N1": 70 }`.
#[derive(Debug)]
struct AtomNames(Vec<(String, AtomNumber)>);

impl Serialize for AtomNames {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_map(self.0.iter().map(|(atom_name, n)| (atom_name, n)))
    }
}

#[derive(Debug, Serialize)]
pub struct BondReport {
    atoms: [AtomNumber; 2],
    order: BondOrder,
    hidden: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(symbol: &str) -> Element {
        Element::from_symbol(symbol).unwrap()
    }

    /// Ethene, flat in the xy plane: C1=C2 along x, two hydrogens on each carbon.
    fn ethene() -> Molecule {
        let atoms = vec![
            (element("C"), [-0.67, 0.0, 0.0]),
            (element("C"), [0.67, 0.0, 0.0]),
            (element("H"), [-1.24, 0.92, 0.0]),
            (element("H"), [-1.24, -0.92, 0.0]),
            (element("H"), [1.24, 0.92, 0.0]),
            (element("H"), [1.24, -0.92, 0.0]),
        ];
        Molecule::new(
            atoms,
            vec![
                (1, 2, BondOrder::Double),
                (1, 3, BondOrder::Single),
                (1, 4, BondOrder::Single),
                (2, 5, BondOrder::Single),
                (2, 6, BondOrder::Single),
            ],
        )
        .unwrap()
    }

    #[test]
    fn the_atoms_are_moved_to_their_centroid() {
        let molecule = Molecule::new(
            vec![
                (element("C"), [1.0, 2.0, 3.0]),
                (element("O"), [3.0, 2.0, 3.0]),
            ],
            vec![(2, 1, BondOrder::Double)],
        )
        .unwrap();

        let positions: Vec<Vec3> = molecule.atoms.iter().map(|a| a.position).collect();

        assert_eq!(
            positions,
            [Vec3::new(-1.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)]
        );
        assert_eq!(
            molecule.bonds[0].atoms.map(AtomNumber::get),
            [1, 2],
            "the lower number comes first"
        );
    }

    #[test]
    fn a_bond_to_no_atom_to_itself_or_twice_is_refused() {
        let two = || vec![(element("C"), [0.0; 3]), (element("C"), [1.0, 0.0, 0.0])];

        assert_eq!(
            Molecule::new(two(), vec![(1, 3, BondOrder::Single)]).unwrap_err(),
            MoleculeError::BondToMissingAtom { atom: 3, atoms: 2 }
        );
        assert_eq!(
            Molecule::new(two(), vec![(0, 1, BondOrder::Single)]).unwrap_err(),
            MoleculeError::BondToMissingAtom { atom: 0, atoms: 2 }
        );
        assert_eq!(
            Molecule::new(two(), vec![(2, 2, BondOrder::Single)]).unwrap_err(),
            MoleculeError::BondToSelf { atom: 2 }
        );
        assert_eq!(
            Molecule::new(
                two(),
                vec![(1, 2, BondOrder::Single), (2, 1, BondOrder::Double)]
            )
            .unwrap_err(),
            MoleculeError::BondRepeated { atoms: [1, 2] }
        );
        assert_eq!(
            Molecule::new(vec![], vec![]).unwrap_err(),
            MoleculeError::NoAtoms
        );
        assert_eq!(
            Molecule::new(vec![(element("C"), [f64::NAN, 0.0, 0.0])], vec![]).unwrap_err(),
            MoleculeError::NotFinite { atom: 1 }
        );
    }

    #[test]
    fn a_double_bond_is_two_sticks_a_side_in_the_plane_of_its_neighbors() {
        let molecule = ethene();
        let carbons = molecule.bonds[0];

        let across = molecule.across(&carbons);
        let sticks: Vec<Stick> = molecule.sticks(&carbons).collect();

        assert!(
            (across - Vec3::Y).length() < 1e-5,
            "H3 lies +y of C1, so across is +y: {across}"
        );
        assert_eq!(
            sticks.len(),
            4,
            "two parallel sticks, each a half from each carbon to the midpoint"
        );
        let ys: Vec<f32> = sticks
            .iter()
            .map(|s| (s.from.y * 1000.0).round() / 1000.0)
            .collect();
        assert_eq!(ys, [-0.09, -0.09, 0.09, 0.09]);
        assert!(
            sticks.iter().all(|s| s.from.z == 0.0 && s.to.z == 0.0),
            "no stick leaves the plane"
        );
        assert_eq!(
            (sticks[0].to.x, sticks[1].to.x),
            (0.0, 0.0),
            "they meet at the bond's midpoint"
        );
    }

    #[test]
    fn the_sticks_of_a_triple_bond_between_collinear_atoms_lie_across_it() {
        let molecule = Molecule::new(
            vec![
                (element("C"), [-0.6, 0.0, 0.0]),
                (element("C"), [0.6, 0.0, 0.0]),
                (element("H"), [-1.66, 0.0, 0.0]),
                (element("H"), [1.66, 0.0, 0.0]),
            ],
            vec![
                (1, 2, BondOrder::Triple),
                (1, 3, BondOrder::Single),
                (2, 4, BondOrder::Single),
            ],
        )
        .unwrap();

        let across = molecule.across(&molecule.bonds[0]);

        assert!(
            (across.length() - 1.0).abs() < 1e-5 && across.x.abs() < 1e-5,
            "{across}"
        );
        assert_eq!(
            molecule.sticks(&molecule.bonds[0]).count(),
            6,
            "three parallel sticks"
        );
    }

    #[test]
    fn hiding_hydrogens_leaves_the_atoms_and_draws_only_the_carbons_and_their_bond() {
        let molecule = ethene();

        let shown = Representation::BallAndStick { hydrogens: true }.draw(&molecule);
        let hidden = Representation::BallAndStick { hydrogens: false }.draw(&molecule);

        assert_eq!((shown.balls.len(), shown.sticks.len()), (6, 4 + 8));
        assert_eq!((hidden.balls.len(), hidden.sticks.len()), (2, 4));
        assert_eq!(
            molecule.atoms.len(),
            6,
            "a hidden hydrogen is still an atom"
        );
        let double_bond = |drawn: &Drawn| {
            drawn
                .sticks
                .iter()
                .filter(|s| s.radius == 0.075)
                .map(|s| (s.from, s.to))
                .collect::<Vec<_>>()
        };
        assert_eq!(double_bond(&shown).len(), 4);
        assert_eq!(
            double_bond(&hidden)[0].0,
            double_bond(&shown)[0].0,
            "the double bond does not turn when hydrogens go"
        );
    }

    #[test]
    fn a_bond_is_hidden_when_either_of_its_atoms_is() {
        let molecule = ethene();
        let report = Representation::BallAndStick { hydrogens: false }.report(
            &molecule,
            Mat4::IDENTITY,
            |_| None,
            |_| 1.0,
        );

        let hidden: Vec<bool> = report.bonds.iter().map(|b| b.hidden).collect();
        assert_eq!(hidden, [false, true, true, true, true]);
        assert_eq!(report.atoms.iter().filter(|a| a.hidden).count(), 4);
    }

    #[test]
    fn each_shown_ball_has_its_radius_in_pixels_at_the_largest_scale() {
        let report = Representation::BallAndStick { hydrogens: false }.report(
            &ethene(),
            Mat4::from_scale(Vec3::new(1.0, 2.0, 0.5)),
            |point| Some(point.truncate()),
            |_| 10.0,
        );

        let radii: Vec<Option<f32>> = report.atoms.iter().map(|a| a.ball_radius).collect();
        assert_eq!(
            radii,
            [Some(8.5), Some(8.5), None, None, None, None],
            "a carbon's ball is 1.7 × 0.25 angstroms, at scale 2 and 10 pixels a unit; a hidden hydrogen has none"
        );
    }

    #[test]
    fn a_ball_meets_a_box_by_its_disc_not_its_corner() {
        let (min, max) = (Vec2::new(0.0, 0.0), Vec2::new(100.0, 20.0));
        assert!(ball_meets_box(Vec2::new(50.0, 28.0), 10.0, min, max));
        assert!(!ball_meets_box(Vec2::new(108.0, 28.0), 10.0, min, max));
        assert!(ball_meets_box(Vec2::new(106.0, 26.0), 10.0, min, max));
        assert!(ball_meets_box(Vec2::new(50.0, 10.0), 0.0, min, max));
    }
}
