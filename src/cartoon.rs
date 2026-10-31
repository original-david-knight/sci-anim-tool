//! The cartoon representation: a protein's backbone as one tube through its alpha carbons, a
//! nucleic acid's through its phosphorus atoms, whose cross-section is a thin circle along coil,
//! a flat ribbon along each helix and a flat arrow along each β-strand, from the secondary
//! structure its PDB file gives. Everything here is a pure function of the molecule, made once.
//!
//! Residue `k` of a run of trace atoms sits at `s = k`, and the tube is the centripetal
//! Catmull-Rom curve through the trace atoms, as a tube through curve points is.
//!
//! The ribbon's flat side does not follow the rotation-minimizing frame a tube's circle is carried
//! on: it is set at each trace atom from the atoms around it, and turned between them. Along a
//! helix it faces out from the helix's axis, toward the trace atom from the middle of its two
//! neighbors. Elsewhere it lies in the plane of the residue's peptide, along its `C` to `O`,
//! which in a β-sheet lies in the sheet's plane, across the strand. Each is flipped where needed
//! to stay within a right angle of the one before it, so the ribbon twists only as the chain
//! does.

use glam::{DVec3, Vec3};

use crate::group::{AtomGroup, CARTOON_GLOW};
use crate::molecule::{AtomNumber, Molecule, SecondaryStructureKind, StandardResidue};
use crate::scene::Color;
use crate::space_curve::{CrossSection, Spans};

/// The radius of the tube along coil, in angstroms.
const COIL_RADIUS: f32 = 0.3;
/// A helix's ribbon is this wide and thick on either side of the trace, in angstroms.
const HELIX_HALF_WIDTH: f32 = 1.1;
const HELIX_HALF_THICKNESS: f32 = 0.25;
/// A strand's arrow, the same, and the half width its tip steps out to.
const STRAND_HALF_WIDTH: f32 = 0.9;
const STRAND_HALF_THICKNESS: f32 = 0.3;
const TIP_HALF_WIDTH: f32 = 1.5;
/// How far in `s` the tube takes to widen from coil into a ribbon or an arrow, and to narrow
/// back, so it changes smoothly.
const RAMP: f64 = 0.5;
/// The cross-sections a span between two trace atoms is cut into.
const SECTIONS_PER_SPAN: usize = 8;
/// Two trace atoms farther apart than this, in angstroms, are not neighbors in the chain: the
/// file leaves out the residues between them, so the cartoon breaks there. Neighboring alpha
/// carbons lie 3.8 apart, and neighboring phosphorus atoms about 7.
const AMINO_ACID_GAP: f32 = 4.5;
const NUCLEOTIDE_GAP: f32 = 8.0;

/// The colors of the three parts, a textbook's: a red helix, a blue strand and gray coil.
const HELIX_COLOR: Color = Color {
    r: 0xd9,
    g: 0x4a,
    b: 0x5a,
};
const STRAND_COLOR: Color = Color {
    r: 0x3b,
    g: 0x8f,
    b: 0xd9,
};
const COIL_COLOR: Color = Color {
    r: 0xc8,
    g: 0xc8,
    b: 0xc8,
};

/// Which part of a cartoon a stretch of residues is drawn as. A residue in no stretch is coil.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    Helix,
    Strand,
}

/// Every unbroken run of trace atoms of a molecule, in file order. Empty when the molecule has no
/// chain of two amino acids or nucleotides, so a cartoon of it would draw nothing.
#[derive(Debug)]
pub struct Cartoon {
    runs: Vec<TraceRun>,
}

/// The trace atoms of one chain, or of the part of one between two gaps.
#[derive(Debug)]
struct TraceRun {
    /// The trace atom of each residue, in chain order.
    atoms: Vec<AtomNumber>,
    /// The stretches of helix and strand, in chain order, each from one residue index to another,
    /// both included. A residue in none is coil.
    stretches: Vec<(Part, usize, usize)>,
    spans: Spans,
    /// The direction of the ribbon's width at each trace atom.
    across: Vec<Vec3>,
}

/// A piece of a cartoon's tube, in the molecule's own space: one part in its color, or the shell
/// of a lit stretch in the glow's color. It is closed at both ends.
#[derive(Clone, Debug)]
pub struct CartoonTube {
    pub sections: Vec<CrossSection>,
    pub color: Color,
}

impl Cartoon {
    pub fn new(molecule: &Molecule) -> Cartoon {
        let residues = molecule.residues();
        let mut runs = Vec::new();
        for chain in residues.chunk_by(|a, b| a.chain == b.chain) {
            let traced: Vec<(&crate::molecule::Residue, AtomNumber, f32)> = chain
                .iter()
                .filter_map(|residue| {
                    let (atom_name, gap) = match StandardResidue::of(&residue.name)? {
                        StandardResidue::AminoAcid => ("CA", AMINO_ACID_GAP),
                        StandardResidue::Nucleotide => ("P", NUCLEOTIDE_GAP),
                    };
                    let &(_, atom) = residue.atoms.iter().find(|(n, _)| n == atom_name)?;
                    Some((residue, atom, gap))
                })
                .collect();
            let position = |atom: AtomNumber| molecule.atom(atom).position;
            for run in traced.chunk_by(|a, b| position(a.1).distance(position(b.1)) <= a.2.max(b.2))
            {
                if run.len() < 2 {
                    continue;
                }
                let part = |residue: &crate::molecule::Residue| {
                    molecule
                        .secondary_structure()
                        .iter()
                        .enumerate()
                        .find(|(_, s)| {
                            s.chain == residue.chain
                                && s.from <= residue.number
                                && residue.number <= s.to
                        })
                        .map(|(element, s)| match s.kind {
                            SecondaryStructureKind::Helix { .. } => (Part::Helix, element),
                            SecondaryStructureKind::Strand { .. } => (Part::Strand, element),
                        })
                };
                let parts: Vec<Option<(Part, usize)>> = run.iter().map(|(r, ..)| part(r)).collect();
                let mut stretches: Vec<(Part, usize, usize)> = Vec::new();
                let mut k = 0;
                for same in parts.chunk_by(|a, b| a == b) {
                    if let Some((part, _)) = same[0] {
                        stretches.push((part, k, k + same.len() - 1));
                    }
                    k += same.len();
                }
                let points: Vec<DVec3> = run.iter().map(|r| position(r.1).as_dvec3()).collect();
                let spans = Spans::new(points.clone(), false);
                let in_helix = |k: usize| {
                    stretches
                        .iter()
                        .any(|&(part, a, b)| part == Part::Helix && (a..=b).contains(&k))
                };
                let n = run.len();
                let mut across: Vec<Option<Vec3>> = (0..n)
                    .map(|k| {
                        let tangent = spans.at(k as f64).1.as_vec3().normalize_or_zero();
                        let width = if in_helix(k) && k > 0 && k + 1 < n {
                            let outward = points[k] - (points[k - 1] + points[k + 1]) / 2.0;
                            outward.as_vec3().cross(tangent)
                        } else {
                            let named = |atom_name: &str| {
                                run[k]
                                    .0
                                    .atoms
                                    .iter()
                                    .find(|(n, _)| n == atom_name)
                                    .map(|&(_, atom)| position(atom))
                            };
                            match (named("C"), named("O")) {
                                (Some(c), Some(o)) => o - c,
                                _ => Vec3::ZERO,
                            }
                        };
                        width.reject_from(tangent).try_normalize()
                    })
                    .collect();
                for k in 1..n {
                    if across[k].is_none() {
                        across[k] = across[k - 1];
                    }
                }
                for k in (0..n - 1).rev() {
                    if across[k].is_none() {
                        across[k] = across[k + 1];
                    }
                }
                let mut across: Vec<Vec3> = (0..n)
                    .map(|k| {
                        across[k].unwrap_or_else(|| {
                            spans.at(k as f64).1.as_vec3().any_orthonormal_vector()
                        })
                    })
                    .collect();
                for k in 1..n {
                    if across[k].dot(across[k - 1]) < 0.0 {
                        across[k] = -across[k];
                    }
                }
                runs.push(TraceRun {
                    atoms: run.iter().map(|r| r.1).collect(),
                    stretches,
                    spans,
                    across,
                });
            }
        }
        Cartoon { runs }
    }

    /// Whether the cartoon draws anything: whether the molecule has a chain of at least two
    /// amino acids or nucleotides with their trace atoms.
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// Whether `atom` is a trace atom: one the tube runs through.
    pub fn traces(&self, atom: AtomNumber) -> bool {
        self.runs.iter().any(|run| run.atoms.contains(&atom))
    }

    /// The tube: each part of each run in its color.
    pub fn tubes(&self) -> Vec<CartoonTube> {
        let mut tubes = Vec::new();
        for run in &self.runs {
            let last = run.last();
            let mut from = 0.0;
            for &(part, a, b) in &run.stretches {
                let (start, end) = run.reach(part, a, b);
                if start > from {
                    tubes.push(run.tube(from, start, 0.0, COIL_COLOR));
                }
                let color = match part {
                    Part::Helix => HELIX_COLOR,
                    Part::Strand => STRAND_COLOR,
                };
                if end > start {
                    tubes.push(run.tube(start, end, 0.0, color));
                }
                from = end;
            }
            if from < last {
                tubes.push(run.tube(from, last, 0.0, COIL_COLOR));
            }
        }
        tubes
    }

    /// The shell of `group`'s glow: the tube a padding larger along each stretch of residues
    /// whose trace atom is in the group, from halfway to the residue before it to halfway to the
    /// one after.
    pub fn glow(&self, group: &AtomGroup, color: Color) -> Vec<CartoonTube> {
        let mut tubes = Vec::new();
        for run in &self.runs {
            let lit: Vec<bool> = run
                .atoms
                .iter()
                .map(|atom| group.atoms.binary_search(atom).is_ok())
                .collect();
            let mut k = 0;
            for same in lit.chunk_by(|a, b| a == b) {
                if same[0] {
                    let from = (k as f64 - 0.5).max(0.0);
                    let to = ((k + same.len() - 1) as f64 + 0.5).min(run.last());
                    tubes.push(run.tube(from, to, CARTOON_GLOW.padding, color));
                }
                k += same.len();
            }
        }
        tubes
    }

    /// How far the farthest point of the tube lies from the molecule's centroid, in its own space.
    pub fn reach(&self) -> f32 {
        self.runs
            .iter()
            .flat_map(|run| run.tube(0.0, run.last(), 0.0, COIL_COLOR).sections)
            .map(|section| section.point.length() + section.half_width.max(section.half_thickness))
            .fold(0.0, f32::max)
    }
}

impl TraceRun {
    fn last(&self) -> f64 {
        (self.atoms.len() - 1) as f64
    }

    /// Where a stretch of `part` from residue `a` to residue `b` changes the tube: from where it
    /// starts to widen to where it is back to coil, within the run.
    fn reach(&self, part: Part, a: usize, b: usize) -> (f64, f64) {
        let end = match part {
            Part::Strand => b as f64,
            _ => b as f64 + RAMP,
        };
        ((a as f64 - RAMP).max(0.0), end.min(self.last()))
    }

    /// Where a strand's tip starts: one residue before its last, or where the strand starts
    /// to widen if it is shorter.
    fn tip(a: usize, b: usize) -> f64 {
        (b as f64 - 1.0).max(a as f64 - RAMP)
    }

    /// The half width and half thickness of the cross-section at `s`. Where a tip starts,
    /// `before` gives the size just before the step and otherwise just after it.
    fn size(&self, s: f64, before: bool) -> (f32, f32) {
        let smooth = |t: f64| {
            let t = t.clamp(0.0, 1.0);
            (t * t * (3.0 - 2.0 * t)) as f32
        };
        let toward = |(w, t): (f32, f32), share: f32| {
            (
                COIL_RADIUS + (w - COIL_RADIUS) * share,
                COIL_RADIUS + (t - COIL_RADIUS) * share,
            )
        };
        for &(part, a, b) in &self.stretches {
            let (a, b) = (a as f64, b as f64);
            match part {
                Part::Helix if (a - RAMP..=b + RAMP).contains(&s) => {
                    let share = smooth((s - (a - RAMP)) / RAMP).min(smooth((b + RAMP - s) / RAMP));
                    return toward((HELIX_HALF_WIDTH, HELIX_HALF_THICKNESS), share);
                }
                Part::Strand if (a - RAMP..=b).contains(&s) => {
                    let tip = Self::tip(a as usize, b as usize);
                    if s < tip || (s == tip && before) {
                        let share = smooth((s - (a - RAMP)) / RAMP);
                        return toward((STRAND_HALF_WIDTH, STRAND_HALF_THICKNESS), share);
                    }
                    let share = ((b - s) / (b - tip)) as f32;
                    return toward((TIP_HALF_WIDTH, STRAND_HALF_THICKNESS), share);
                }
                _ => {}
            }
        }
        (COIL_RADIUS, COIL_RADIUS)
    }

    /// The tube from `s` of `from` to `to`, its halves `pad` larger, in `color`.
    fn tube(&self, from: f64, to: f64, pad: f32, color: Color) -> CartoonTube {
        let mut stops: Vec<f64> = (0..=(self.last() as usize * SECTIONS_PER_SPAN))
            .map(|k| k as f64 / SECTIONS_PER_SPAN as f64)
            .collect();
        let mut tips = Vec::new();
        for &(part, a, b) in &self.stretches {
            let (af, bf) = (a as f64, b as f64);
            stops.extend([af - RAMP, af, bf, bf + RAMP]);
            if part == Part::Strand {
                tips.push(Self::tip(a, b));
            }
        }
        stops.extend(tips.iter().copied());
        stops.extend([from, to]);
        stops.retain(|s| (from..=to).contains(s));
        stops.sort_by(f64::total_cmp);
        stops.dedup_by(|b, a| (*b - *a).abs() < 1e-9);
        let mut sections = Vec::with_capacity(stops.len() + tips.len());
        for &s in &stops {
            let step = tips.contains(&s);
            if step && s > from {
                sections.push(self.section(s, true, pad));
            }
            if !step || s < to {
                sections.push(self.section(s, false, pad));
            }
        }
        CartoonTube { sections, color }
    }

    fn section(&self, s: f64, before: bool, pad: f32) -> CrossSection {
        let (point, derivative) = self.spans.at(s);
        let k = (s.floor() as usize).min(self.atoms.len() - 2);
        let share = (s - k as f64) as f32;
        let chord = (self.spans.at(k as f64 + 1.0).0 - self.spans.at(k as f64).0).as_vec3();
        let tangent = derivative
            .as_vec3()
            .try_normalize()
            .unwrap_or_else(|| chord.normalize());
        let across = self.across[k]
            .lerp(self.across[k + 1], share)
            .reject_from_normalized(tangent)
            .try_normalize()
            .unwrap_or_else(|| tangent.any_orthonormal_vector());
        let (half_width, half_thickness) = self.size(s, before);
        CrossSection {
            point: point.as_vec3(),
            tangent,
            across,
            half_width: half_width + pad,
            half_thickness: half_thickness + pad,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WHITE: Color = Color {
        r: 0xff,
        g: 0xff,
        b: 0xff,
    };

    fn ubiquitin() -> Molecule {
        let file = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("skills/sci-anim/examples/structures/1ubq.pdb");
        crate::structure_file::read(&file).unwrap()
    }

    /// The `CA` atoms of ubiquitin's residues `from` to `to`, as an atom group.
    fn ca_atoms(molecule: &Molecule, from: i32, to: i32) -> AtomGroup {
        let atoms = molecule
            .residues()
            .iter()
            .filter(|r| (from..=to).contains(&r.number.number))
            .flat_map(|r| r.atoms.iter().filter(|(n, _)| n == "CA").map(|&(_, a)| a))
            .collect();
        AtomGroup {
            atoms,
            center: Vec3::ZERO,
        }
    }

    #[test]
    fn ubiquitin_is_one_run_of_76_ca_atoms_in_seven_stretches() {
        let cartoon = Cartoon::new(&ubiquitin());

        assert_eq!(cartoon.runs.len(), 1);
        let run = &cartoon.runs[0];
        assert_eq!(run.atoms.len(), 76);
        assert_eq!(
            run.stretches,
            [
                (Part::Strand, 0, 6),
                (Part::Strand, 9, 16),
                (Part::Helix, 22, 33),
                (Part::Strand, 39, 44),
                (Part::Strand, 47, 49),
                (Part::Helix, 55, 58),
                (Part::Strand, 63, 71),
            ],
            "residues 1 to 7, 10 to 17, 23 to 34, 40 to 45, 48 to 50, 56 to 59 and 64 to 72"
        );
    }

    #[test]
    fn each_part_is_a_tube_in_its_color_and_each_strand_ends_in_a_tip() {
        let tubes = Cartoon::new(&ubiquitin()).tubes();

        let colors: Vec<&str> = tubes
            .iter()
            .map(|t| match t.color {
                HELIX_COLOR => "helix",
                STRAND_COLOR => "strand",
                _ => "coil",
            })
            .collect();
        assert_eq!(
            colors,
            [
                "strand", "coil", "strand", "coil", "helix", "coil", "strand", "coil", "strand",
                "coil", "helix", "coil", "strand", "coil"
            ]
        );
        for tube in tubes.iter().filter(|t| t.color == STRAND_COLOR) {
            let steps: Vec<(f32, f32)> = tube
                .sections
                .windows(2)
                .filter(|pair| pair[0].point == pair[1].point)
                .map(|pair| (pair[0].half_width, pair[1].half_width))
                .collect();
            assert_eq!(
                steps,
                [(STRAND_HALF_WIDTH, TIP_HALF_WIDTH)],
                "one step, out to the tip"
            );
            let tip = tube.sections[tube.sections.len() - 1];
            assert_eq!(
                (tip.half_width, tip.half_thickness),
                (COIL_RADIUS, COIL_RADIUS)
            );
        }
    }

    #[test]
    fn a_helix_ribbon_faces_out_from_its_axis() {
        let molecule = ubiquitin();
        let cartoon = Cartoon::new(&molecule);
        let run = &cartoon.runs[0];

        for k in 23..=32 {
            let section = run.section(k as f64, false, 0.0);
            let facing = section.tangent.cross(section.across);
            let p = |k: usize| molecule.atom(run.atoms[k]).position;
            let outward = (p(k) - (p(k - 1) + p(k + 1)) / 2.0).normalize();
            assert!(
                facing.dot(outward).abs() > 0.95,
                "residue {}: {}",
                k + 1,
                facing.dot(outward)
            );
            assert_eq!(
                (section.half_width, section.half_thickness),
                (HELIX_HALF_WIDTH, HELIX_HALF_THICKNESS)
            );
        }
    }

    #[test]
    fn a_glow_lights_the_tube_from_halfway_before_its_first_residue_to_halfway_after_its_last() {
        let molecule = ubiquitin();
        let cartoon = Cartoon::new(&molecule);
        let run = &cartoon.runs[0];

        let shells = cartoon.glow(&ca_atoms(&molecule, 23, 34), WHITE);

        assert_eq!(shells.len(), 1);
        let sections = &shells[0].sections;
        assert_eq!(sections[0].point, run.section(21.5, false, 0.0).point);
        assert_eq!(
            sections[sections.len() - 1].point,
            run.section(33.5, false, 0.0).point
        );
        let widest = sections.iter().map(|s| s.half_width).fold(0.0, f32::max);
        assert_eq!(widest, HELIX_HALF_WIDTH + CARTOON_GLOW.padding);
        assert!(cartoon.glow(&ca_atoms(&molecule, 0, 0), WHITE).is_empty());
    }

    #[test]
    fn a_file_with_no_secondary_structure_draws_one_coil_tube() {
        let molecule = ubiquitin().with_secondary_structure(Vec::new());

        let tubes = Cartoon::new(&molecule).tubes();

        assert_eq!(tubes.len(), 1);
        assert_eq!(tubes[0].color, COIL_COLOR);
        assert!(
            tubes[0]
                .sections
                .iter()
                .all(|s| (s.half_width, s.half_thickness) == (COIL_RADIUS, COIL_RADIUS))
        );
    }
}
