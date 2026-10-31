//! Reading a structure file into a `Molecule`: an SDF file, or a PDB file, which `pdb` reads.
//! The only module that uses sdfrust, whose types stop here: the molecule, its elements and its
//! bond orders are ours, and everything sdfrust accepts that a molecule cannot be is refused here
//! with the reason.
//!
//! Reading happens once, when the scene file is read. The tool never goes online to find a
//! structure file.

mod pdb;

use std::io;
use std::path::Path;

use sdfrust::BondOrder as SdfBondOrder;

use crate::element::Element;
use crate::molecule::{BondOrder, Molecule};

#[derive(Debug)]
pub enum StructureFileError {
    /// The file could not be read from disk.
    Unreadable(io::Error),
    /// The file was read, and is not a structure file that makes a molecule. The message says
    /// why, and `first_line` is the first line that has any text, which shows an error page that
    /// curl saved in place of a structure file.
    Invalid { message: String, first_line: String },
}

/// Whether `file` is a PDB file, by its name: it ends in `.pdb`, or in `.ent` as the Protein Data
/// Bank's archive names them. Any other file is SDF.
pub fn is_pdb(file: &Path) -> bool {
    file.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdb") || e.eq_ignore_ascii_case("ent"))
}

/// The molecule of `file`: of a PDB file's first model, or of the one SDF record, V2000 or
/// V3000, of any other file.
pub fn read(file: &Path) -> Result<Molecule, StructureFileError> {
    let bytes = std::fs::read(file).map_err(StructureFileError::Unreadable)?;
    let text = String::from_utf8_lossy(&bytes);
    let first_line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .chars()
        .take(80)
        .collect::<String>();
    let invalid = |message: String| StructureFileError::Invalid {
        message,
        first_line: first_line.clone(),
    };
    if is_pdb(file) {
        return pdb::read(&text).map_err(invalid);
    }
    let mut records =
        sdfrust::parse_sdf_auto_string_multi(&text).map_err(|e| invalid(e.to_string()))?;
    let record = match records.len() {
        0 => return Err(invalid("it has no molecule".to_string())),
        1 => records.remove(0),
        n => {
            return Err(invalid(format!(
                "it holds {n} molecules, and a structure file holds one shape of one compound; fetch it with `record_type=3d`, which gives the first conformer"
            )));
        }
    };
    // Columns 21 and 22 of the header's second line give the dimensions, as the SDF format defines.
    if record
        .program_line
        .as_deref()
        .and_then(|line| line.get(20..22))
        == Some("2D")
    {
        return Err(invalid("it is a 2D structure file, whose atoms all lie flat; fetch the 3D one by adding `?record_type=3d` to the PubChem URL".to_string()));
    }
    let atoms = record
        .atoms
        .iter()
        .enumerate()
        .map(|(i, atom)| {
            let element = Element::from_symbol(&atom.element).ok_or_else(|| invalid(format!("atom {} has the element symbol {:?}, which is not an element sci_anim draws; it draws elements 1 (H) to 109 (Mt)", i + 1, atom.element)))?;
            Ok((element, [atom.x, atom.y, atom.z]))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bonds = record
        .bonds
        .iter()
        .map(|bond| {
            let order = match bond.order {
                SdfBondOrder::Single => BondOrder::Single,
                SdfBondOrder::Double => BondOrder::Double,
                SdfBondOrder::Triple => BondOrder::Triple,
                other => {
                    let message = format!(
                        "the bond from atom {} to atom {} has the SDF bond code {}, and sci_anim draws only single (1), double (2) and triple (3) bonds; PubChem writes an aromatic ring with alternating single and double bonds",
                        bond.atom1 + 1,
                        bond.atom2 + 1,
                        other.to_sdf()
                    );
                    return Err(invalid(message));
                }
            };
            Ok((bond.atom1 as u32 + 1, bond.atom2 as u32 + 1, order))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Molecule::new(atoms, bonds).map_err(|e| invalid(e.to_string()))
}
