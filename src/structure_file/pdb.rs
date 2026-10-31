//! Reading a PDB file's fixed columns into a `Molecule` with its residues and chains, and the
//! secondary structure its `HELIX` and `SHEET` records give.
//!
//! What it keeps: every `ATOM` and `HETATM` record of the first model, except waters and the
//! atoms of any alternate location but the first a residue gives. What it bonds: each `CONECT`
//! record's atoms, and the atoms a standard residue's distances show are bonded, inside the
//! residue and to the residue after it in its chain. Every bond is single, because a distance
//! does not tell a double bond from a single one reliably at a crystal structure's precision.

use std::collections::{BTreeMap, BTreeSet};

use glam::DVec3;

use crate::element::Element;
use crate::molecule::{
    BondOrder, ChainId, Molecule, Residue, ResidueNumber, SecondaryStructure,
    SecondaryStructureKind, StandardResidue,
};

/// Two atoms of a standard residue are bonded when they lie no farther apart than the sum of their
/// covalent radii and this, in angstroms. A crystal structure's bonds come within about 0.1 of
/// that sum, and the nearest two unbonded atoms of a residue, two bonds apart, lie 2.2
/// or more apart, where the longest sum of two radii a residue bonds, carbon to sulfur, plus this
/// is 2.18.
pub const BOND_TOLERANCE: f64 = 0.4;

/// The residue names of water, which a crystal structure gives as lone oxygens around the
/// molecule. They are left out.
const WATERS: [&str; 4] = ["HOH", "WAT", "DOD", "H2O"];

struct PdbAtom {
    serial: u32,
    element: Element,
    position: DVec3,
}

/// The molecule of `text`, a PDB file, or what is wrong with it. A message about one line says
/// which, counting from 1.
pub fn read(text: &str) -> Result<Molecule, String> {
    let mut atoms: Vec<PdbAtom> = Vec::new();
    let mut residues: Vec<Residue<u32>> = Vec::new();
    // Every serial number an atom record gives, kept or not, so a `CONECT` to a water is skipped
    // and a `CONECT` to no atom at all is an error.
    let mut serials: BTreeSet<u32> = BTreeSet::new();
    let mut first_alternate: BTreeMap<(char, i32, Option<char>), char> = BTreeMap::new();
    let mut conects: Vec<(usize, u32, u32)> = Vec::new();
    let mut waters = false;
    let mut helices: Vec<SecondaryStructure> = Vec::new();
    let mut strands: Vec<SecondaryStructure> = Vec::new();
    let mut model_ended = false;
    for (i, line) in text.lines().enumerate() {
        let line_number = i + 1;
        let at = |message: String| format!("line {line_number}: {message}");
        match columns(line, 1, 6).trim_end() {
            "ATOM" | "HETATM" => {
                let serial = whole(line, 7, 11, "atom serial number").map_err(at)?;
                serials.insert(serial);
                if model_ended {
                    continue;
                }
                let residue_name = columns(line, 18, 20).trim().to_string();
                if WATERS.contains(&residue_name.as_str()) {
                    waters = true;
                    continue;
                }
                let atom_name = columns(line, 13, 16).trim().to_string();
                let alternate = character(line, 17);
                let chain = ChainId(character(line, 22).unwrap_or(' '));
                let number = ResidueNumber {
                    number: whole(line, 23, 26, "residue number").map_err(at)?,
                    insertion_code: character(line, 27),
                };
                if let Some(alternate) = alternate {
                    let first = first_alternate
                        .entry((chain.0, number.number, number.insertion_code))
                        .or_insert(alternate);
                    if *first != alternate {
                        continue;
                    }
                }
                let position = DVec3::new(
                    decimal(line, 31, 38, "x coordinate").map_err(at)?,
                    decimal(line, 39, 46, "y coordinate").map_err(at)?,
                    decimal(line, 47, 54, "z coordinate").map_err(at)?,
                );
                let symbol = match columns(line, 77, 78).trim() {
                    "" => atom_name
                        .trim_start_matches(|c: char| c.is_ascii_digit())
                        .chars()
                        .take(1)
                        .collect(),
                    written => written.to_string(),
                };
                let element = element(&symbol).ok_or_else(|| {
                    at(format!(
                        "atom {serial} has the element symbol {symbol:?}, which is not an element sci_anim draws; it draws elements 1 (H) to 109 (Mt)"
                    ))
                })?;
                let same_residue = residues.last().is_some_and(|r| {
                    r.chain == chain && r.number == number && r.name == residue_name
                });
                if !same_residue {
                    residues.push(Residue {
                        chain,
                        number,
                        name: residue_name,
                        atoms: Vec::new(),
                    });
                }
                let residue = residues.last_mut().expect("pushed above if absent");
                residue.atoms.push((atom_name, serial));
                atoms.push(PdbAtom {
                    serial,
                    element,
                    position,
                });
            }
            "HELIX" => {
                let kind = SecondaryStructureKind::Helix {
                    id: columns(line, 12, 14).trim().to_string(),
                    class: match columns(line, 39, 40).trim() {
                        "" => 1,
                        _ => whole(line, 39, 40, "helix class").map_err(at)?,
                    },
                };
                helices
                    .extend(stretch(line, kind, [20, 22, 25, 26], [32, 34, 37, 38]).map_err(at)?);
            }
            "SHEET" => {
                let kind = SecondaryStructureKind::Strand {
                    sheet: columns(line, 12, 14).trim().to_string(),
                    strand: whole(line, 8, 10, "strand number").map_err(at)?,
                };
                strands
                    .extend(stretch(line, kind, [22, 23, 26, 27], [33, 34, 37, 38]).map_err(at)?);
            }
            "ENDMDL" => model_ended = true,
            "CONECT" => {
                let from = whole(line, 7, 11, "atom serial number").map_err(at)?;
                for (first, last) in [(12, 16), (17, 21), (22, 26), (27, 31)] {
                    if !columns(line, first, last).trim().is_empty() {
                        let to =
                            whole(line, first, last, "bonded atom serial number").map_err(at)?;
                        conects.push((line_number, from, to));
                    }
                }
            }
            _ => {}
        }
    }
    if atoms.is_empty() && waters {
        return Err("its only atoms are waters, which sci_anim leaves out".to_string());
    }

    let kept: BTreeMap<u32, usize> = atoms
        .iter()
        .enumerate()
        .map(|(i, a)| (a.serial, i))
        .collect();
    let mut bonds: BTreeSet<(u32, u32)> = BTreeSet::new();
    for (line_number, from, to) in conects {
        for serial in [from, to] {
            if !serials.contains(&serial) {
                return Err(format!(
                    "line {line_number}: a CONECT record names atom {serial}, which no ATOM or HETATM record gives"
                ));
            }
        }
        if from != to && kept.contains_key(&from) && kept.contains_key(&to) {
            bonds.insert((from.min(to), from.max(to)));
        }
    }
    let atom = |serial: u32| &atoms[kept[&serial]];
    let bonded = |a: u32, b: u32| {
        let (a, b) = (atom(a), atom(b));
        let reach = f64::from(a.element.covalent_radius() + b.element.covalent_radius());
        a.position.distance(b.position) <= reach + BOND_TOLERANCE
    };
    for (k, residue) in residues.iter().enumerate() {
        let Some(standard) = StandardResidue::of(&residue.name) else {
            continue;
        };
        for (i, &(_, a)) in residue.atoms.iter().enumerate() {
            for &(_, b) in &residue.atoms[i + 1..] {
                if bonded(a, b) {
                    bonds.insert((a.min(b), a.max(b)));
                }
            }
        }
        let Some(next) = residues.get(k + 1) else {
            continue;
        };
        if next.chain != residue.chain || StandardResidue::of(&next.name) != Some(standard) {
            continue;
        }
        let (from, to) = standard.next_bond();
        let named = |r: &Residue<u32>, atom_name: &str| {
            r.atoms
                .iter()
                .find(|(n, _)| n == atom_name)
                .map(|&(_, s)| s)
        };
        if let (Some(a), Some(b)) = (named(residue, from), named(next, to))
            && bonded(a, b)
        {
            bonds.insert((a.min(b), a.max(b)));
        }
    }

    Molecule::numbered(
        atoms
            .iter()
            .map(|a| (a.serial, a.element, a.position.to_array()))
            .collect(),
        bonds
            .into_iter()
            .map(|(a, b)| (a, b, BondOrder::Single))
            .collect(),
        residues,
    )
    .map(|molecule| molecule.with_secondary_structure(helices.into_iter().chain(strands).collect()))
    .map_err(|e| e.to_string())
}

/// The stretch of secondary structure of `kind` that a `HELIX` or `SHEET` record gives, from the
/// residue whose chain id, residue number and insertion code are in the columns `first` to the
/// one in the columns `last`, each written `[chain, number from, number to, insertion code]`.
/// `None` for a record whose two ends lie on different chains, which no stretch does.
fn stretch(
    line: &str,
    kind: SecondaryStructureKind,
    first: [usize; 4],
    last: [usize; 4],
) -> Result<Option<SecondaryStructure>, String> {
    let end = |[chain, from, to, code]: [usize; 4]| -> Result<_, String> {
        Ok((
            ChainId(character(line, chain).unwrap_or(' ')),
            ResidueNumber {
                number: whole(line, from, to, "residue number")?,
                insertion_code: character(line, code),
            },
        ))
    };
    let ((chain, from), (end_chain, to)) = (end(first)?, end(last)?);
    Ok((chain == end_chain).then_some(SecondaryStructure {
        kind,
        chain,
        from,
        to,
    }))
}

/// Columns `first` to `last` of `line`, counting from 1 as the PDB format does, or as many of
/// them as the line has.
fn columns(line: &str, first: usize, last: usize) -> &str {
    let end = last.min(line.len());
    line.get(first - 1..end).unwrap_or("")
}

fn character(line: &str, column: usize) -> Option<char> {
    columns(line, column, column)
        .chars()
        .next()
        .filter(|c| *c != ' ')
}

fn whole<T: std::str::FromStr>(
    line: &str,
    first: usize,
    last: usize,
    what: &str,
) -> Result<T, String> {
    let text = columns(line, first, last).trim();
    text.parse().map_err(|_| {
        format!("the {what} in columns {first} to {last} is {text:?}, which is not a whole number")
    })
}

fn decimal(line: &str, first: usize, last: usize, what: &str) -> Result<f64, String> {
    let text = columns(line, first, last).trim();
    text.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| {
            format!("the {what} in columns {first} to {last} is {text:?}, which is not a number")
        })
}

/// The element a PDB file's symbol names. The file writes symbols in capitals, as `FE`.
fn element(symbol: &str) -> Option<Element> {
    let mut chars = symbol.chars();
    let first = chars.next()?.to_ascii_uppercase();
    let rest: String = chars.map(|c| c.to_ascii_lowercase()).collect();
    Element::from_symbol(&format!("{first}{rest}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const UBIQUITIN: &str = include_str!("../../skills/sci-anim/examples/structures/1ubq.pdb");
    const DODECAMER: &str = include_str!("../../skills/sci-anim/examples/structures/1bna.pdb");

    /// `line` with `column`, counting from 1, set to `c`.
    fn with_column(line: &str, column: usize, c: char) -> String {
        let mut chars: Vec<char> = format!("{line:<80}").chars().collect();
        chars[column - 1] = c;
        chars.into_iter().collect::<String>().trim_end().to_string()
    }

    /// The atom records of ubiquitin's residues `numbers`.
    fn ubiquitin_residues(numbers: &[i32]) -> Vec<String> {
        UBIQUITIN
            .lines()
            .filter(|l| l.starts_with("ATOM"))
            .filter(|l| numbers.contains(&columns(l, 23, 26).trim().parse().unwrap()))
            .map(str::to_string)
            .collect()
    }

    fn bond_serials(molecule: &Molecule) -> Vec<[u32; 2]> {
        molecule
            .bonds()
            .iter()
            .map(|b| b.atoms.map(|n| n.get()))
            .collect()
    }

    /// The most any bond is longer than the sum of its atoms' covalent radii.
    fn longest_stretch(molecule: &Molecule) -> f32 {
        molecule
            .bonds()
            .iter()
            .map(|b| {
                let [a, z] = b.atoms.map(|n| molecule.atom(n));
                a.position.distance(z.position)
                    - a.element.covalent_radius()
                    - z.element.covalent_radius()
            })
            .fold(f32::MIN, f32::max)
    }

    /// Whether every residue is bonded to the one after it in its chain.
    fn every_residue_bonded_to_the_next(molecule: &Molecule) -> bool {
        molecule.residues().windows(2).all(|pair| {
            let [this, next] = pair else { unreachable!() };
            next.chain != this.chain
                || molecule.bonds().iter().any(|b| {
                    this.atoms.iter().any(|&(_, n)| b.atoms.contains(&n))
                        && next.atoms.iter().any(|&(_, n)| b.atoms.contains(&n))
                })
        })
    }

    #[test]
    fn ubiquitin_is_one_chain_of_76_residues_bonded_into_one_piece() {
        let molecule = read(UBIQUITIN).unwrap();

        assert_eq!(molecule.atom_count(), 602, "the 58 waters are left out");
        assert_eq!(
            molecule.bonds().len(),
            602 - 1 + 7,
            "one piece with seven rings: two phenylalanines, a tyrosine, a histidine and three prolines"
        );
        assert_eq!(molecule.residues().len(), 76);
        let residue = &molecule.residues()[22];
        assert_eq!((residue.name.as_str(), residue.number.number), ("ILE", 23));
        assert!(every_residue_bonded_to_the_next(&molecule));
        assert!(longest_stretch(&molecule) <= BOND_TOLERANCE as f32);
        assert!(
            molecule
                .bonds()
                .iter()
                .all(|b| b.order == BondOrder::Single)
        );
    }

    #[test]
    fn the_dodecamer_is_two_chains_of_twelve_nucleotides_numbered_by_serial() {
        let molecule = read(DODECAMER).unwrap();

        assert_eq!(molecule.atom_count(), 486, "the 80 waters are left out");
        assert_eq!(
            molecule.bonds().len(),
            2 * (243 - 1 + 6 * 3 + 6 * 2),
            "each strand is one piece of 243 atoms, with a sugar ring in every nucleotide and two more rings in each of its six purines and one in each pyrimidine"
        );
        let chains: Vec<(char, i32, i32)> = molecule
            .residues()
            .chunk_by(|a, b| a.chain == b.chain)
            .map(|c| {
                (
                    c[0].chain.0,
                    c[0].number.number,
                    c[c.len() - 1].number.number,
                )
            })
            .collect();
        assert_eq!(chains, [('A', 1, 12), ('B', 13, 24)]);
        assert_eq!(
            molecule.residues()[12].atoms[0],
            ("O5'".to_string(), molecule.atom_number(245).unwrap()),
            "chain A's TER record takes serial 244, so chain B starts at 245"
        );
        assert_eq!(molecule.atom_number(244), None);
        assert!(every_residue_bonded_to_the_next(&molecule));
        assert!(longest_stretch(&molecule) <= BOND_TOLERANCE as f32);
    }

    #[test]
    fn ubiquitin_has_two_helices_and_a_sheet_of_five_strands() {
        let molecule = read(UBIQUITIN).unwrap();

        let stretches: Vec<(SecondaryStructureKind, char, i32, i32)> = molecule
            .secondary_structure()
            .iter()
            .map(|s| (s.kind.clone(), s.chain.0, s.from.number, s.to.number))
            .collect();
        let helix = |id: &str, class| SecondaryStructureKind::Helix {
            id: id.to_string(),
            class,
        };
        let strand = |strand| SecondaryStructureKind::Strand {
            sheet: "BET".to_string(),
            strand,
        };
        assert_eq!(
            stretches,
            [
                (helix("H1", 1), 'A', 23, 34),
                (helix("H2", 5), 'A', 56, 59),
                (strand(1), 'A', 10, 17),
                (strand(2), 'A', 1, 7),
                (strand(3), 'A', 64, 72),
                (strand(4), 'A', 40, 45),
                (strand(5), 'A', 48, 50),
            ]
        );
        assert!(
            read(DODECAMER).unwrap().secondary_structure().is_empty(),
            "the dodecamer's file has no HELIX or SHEET records"
        );
    }

    #[test]
    fn a_stretch_between_two_chains_is_left_out_and_a_bad_number_names_its_line() {
        let mut lines = vec![
            "HELIX    1  H1 ILE A   23  GLU B   34  1                                  12"
                .to_string(),
        ];
        lines.extend(ubiquitin_residues(&[1]));
        assert!(
            read(&lines.join("\n"))
                .unwrap()
                .secondary_structure()
                .is_empty()
        );

        lines[0] = "SHEET    1 BET 5 GLY A  1x  VAL A  17  0".to_string();
        assert_eq!(
            read(&lines.join("\n")).unwrap_err(),
            "line 1: the residue number in columns 23 to 26 is \"1x\", which is not a whole number"
        );
    }

    #[test]
    fn residues_with_a_gap_between_them_are_not_bonded() {
        let text = ubiquitin_residues(&[1, 3]).join("\n");

        let molecule = read(&text).unwrap();

        assert_eq!(molecule.residues().len(), 2);
        assert!(!every_residue_bonded_to_the_next(&molecule));
        assert_eq!(
            molecule.bonds().len(),
            7 + 7,
            "methionine's 8 atoms and isoleucine's 8 each make a chain of 7 bonds"
        );
    }

    #[test]
    fn only_the_first_alternate_location_and_the_first_model_are_kept() {
        let lines = ubiquitin_residues(&[1]);
        let a = with_column(&lines[7], 17, 'A');
        let b = with_column(&with_column(&lines[7], 17, 'B'), 31, '9');
        let mut text = vec!["MODEL        1".to_string()];
        text.extend(lines[..7].iter().cloned());
        text.extend([a, b, "ENDMDL".to_string(), "MODEL        2".to_string()]);
        text.extend(lines.iter().cloned());
        text.push("ENDMDL".to_string());

        let molecule = read(&text.join("\n")).unwrap();

        assert_eq!(molecule.atom_count(), 8);
        assert_eq!(molecule.bonds().len(), 7, "CE bonds to SD at location A");
    }

    #[test]
    fn a_conect_record_bonds_atoms_once_and_skips_a_water() {
        let mut lines = ubiquitin_residues(&[1]);
        lines.push(
            "HETATM  604  O   HOH A  77      45.747  30.081  19.708  1.00 12.43           O"
                .to_string(),
        );
        lines.push("CONECT    1    8  604".to_string());
        lines.push("CONECT    8    1".to_string());

        let molecule = read(&lines.join("\n")).unwrap();

        assert_eq!(molecule.atom_count(), 8);
        assert!(bond_serials(&molecule).contains(&[1, 8]));
        assert_eq!(molecule.bonds().len(), 8);
    }

    #[test]
    fn a_ligand_is_bonded_only_by_its_conect_records() {
        let lines = [
            "HETATM    1  C1  EOH A   1       0.000   0.000   0.000  1.00  0.00           C",
            "HETATM    2  C2  EOH A   1       1.520   0.000   0.000  1.00  0.00           C",
            "HETATM    3  O   EOH A   1       2.000   1.350   0.000  1.00  0.00           O",
            "CONECT    2    3",
        ];

        let molecule = read(&lines.join("\n")).unwrap();

        assert_eq!(bond_serials(&molecule), [[2, 3]]);
    }

    #[test]
    fn a_file_that_cannot_make_a_molecule_says_which_line() {
        let mut lines = ubiquitin_residues(&[1]);
        lines[2].replace_range(30..38, "  26.9x3");
        assert_eq!(
            read(&lines.join("\n")).unwrap_err(),
            "line 3: the x coordinate in columns 31 to 38 is \"26.9x3\", which is not a number"
        );

        assert_eq!(
            read("HEADER    DNA\nEND\n").unwrap_err(),
            "it has no atoms, so there is nothing to draw"
        );
        assert_eq!(
            read("HETATM  604  O   HOH A  77      45.747  30.081  19.708  1.00 12.43           O")
                .unwrap_err(),
            "its only atoms are waters, which sci_anim leaves out"
        );
        let mut lines = ubiquitin_residues(&[1]);
        lines.push("CONECT    1  900".to_string());
        assert_eq!(
            read(&lines.join("\n")).unwrap_err(),
            "line 9: a CONECT record names atom 900, which no ATOM or HETATM record gives"
        );
        let lines = ubiquitin_residues(&[1]);
        assert_eq!(
            read(&with_column(&lines[0], 77, 'X')).unwrap_err(),
            "line 1: atom 1 has the element symbol \"XN\", which is not an element sci_anim draws; it draws elements 1 (H) to 109 (Mt)"
        );
    }

    #[test]
    fn an_element_symbol_is_read_in_capitals_or_from_the_atom_name() {
        let lines = [
            "HETATM    1 FE   HEM A   1       0.000   0.000   0.000  1.00  0.00          FE",
            "HETATM    2  CA  HEM A   1       1.520   0.000   0.000  1.00  0.00",
        ];

        let molecule = read(&lines.join("\n")).unwrap();

        let symbols: Vec<&str> = molecule
            .numbers()
            .map(|n| molecule.atom(n).element.symbol())
            .collect();
        assert_eq!(symbols, ["Fe", "C"]);
    }
}
