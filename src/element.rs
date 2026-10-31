//! The chemical elements, with the color and size a representation draws each with and the
//! covalent radius a PDB file's bonds are found by. One table owns these facts, so a new column
//! (a mass, a name) is one edit.

use serde::{Serialize, Serializer};

use crate::scene::Color;

/// A chemical element, known by its atomic number, 1 to 109. Only `from_symbol` and the
/// constants make one, so holding an `Element` means it has a row in the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Element(u8);

struct Row {
    symbol: &'static str,
    /// The element color: Jmol's, which most viewers use, so a molecule looks as an agent expects.
    color: Color,
    /// The van der Waals radius in angstroms, from the Blue Obelisk Data Repository.
    radius: f32,
    /// The covalent radius in angstroms, from the same repository, whose values match Pyykkö and
    /// Atsumi's single-bond radii (2009).
    covalent_radius: f32,
}

const fn row(symbol: &'static str, rgb: u32, radius: f32, covalent_radius: f32) -> Row {
    Row {
        symbol,
        color: Color {
            r: (rgb >> 16) as u8,
            g: (rgb >> 8) as u8,
            b: rgb as u8,
        },
        radius,
        covalent_radius,
    }
}

/// Every element both sources give, in atomic number order. Jmol's colors stop at meitnerium,
/// so an element after it is not one sci_anim draws. `scripts/element-data` prints these rows,
/// and a test checks them against `tests/fixtures/elements`.
static ROWS: [Row; 109] = [
    row("H", 0xffffff, 1.2, 0.32),
    row("He", 0xd9ffff, 1.4, 0.46),
    row("Li", 0xcc80ff, 2.2, 1.33),
    row("Be", 0xc2ff00, 1.9, 1.02),
    row("B", 0xffb5b5, 1.8, 0.85),
    row("C", 0x909090, 1.7, 0.75),
    row("N", 0x3050f8, 1.6, 0.71),
    row("O", 0xff0d0d, 1.55, 0.63),
    row("F", 0x90e050, 1.5, 0.64),
    row("Ne", 0xb3e3f5, 1.54, 0.67),
    row("Na", 0xab5cf2, 2.4, 1.55),
    row("Mg", 0x8aff00, 2.2, 1.39),
    row("Al", 0xbfa6a6, 2.1, 1.26),
    row("Si", 0xf0c8a0, 2.1, 1.16),
    row("P", 0xff8000, 1.95, 1.11),
    row("S", 0xffff30, 1.8, 1.03),
    row("Cl", 0x1ff01f, 1.8, 0.99),
    row("Ar", 0x80d1e3, 1.88, 0.96),
    row("K", 0x8f40d4, 2.8, 1.96),
    row("Ca", 0x3dff00, 2.4, 1.71),
    row("Sc", 0xe6e6e6, 2.3, 1.48),
    row("Ti", 0xbfc2c7, 2.15, 1.36),
    row("V", 0xa6a6ab, 2.05, 1.34),
    row("Cr", 0x8a99c7, 2.05, 1.22),
    row("Mn", 0x9c7ac7, 2.05, 1.19),
    row("Fe", 0xe06633, 2.05, 1.16),
    row("Co", 0xf090a0, 2.0, 1.11),
    row("Ni", 0x50d050, 2.0, 1.1),
    row("Cu", 0xc88033, 2.0, 1.12),
    row("Zn", 0x7d80b0, 2.1, 1.18),
    row("Ga", 0xc28f8f, 2.1, 1.24),
    row("Ge", 0x668f8f, 2.1, 1.21),
    row("As", 0xbd80e3, 2.05, 1.21),
    row("Se", 0xffa100, 1.9, 1.16),
    row("Br", 0xa62929, 1.9, 1.14),
    row("Kr", 0x5cb8d1, 2.02, 1.17),
    row("Rb", 0x702eb0, 2.9, 2.1),
    row("Sr", 0x00ff00, 2.55, 1.85),
    row("Y", 0x94ffff, 2.4, 1.63),
    row("Zr", 0x94e0e0, 2.3, 1.54),
    row("Nb", 0x73c2c9, 2.15, 1.47),
    row("Mo", 0x54b5b5, 2.1, 1.38),
    row("Tc", 0x3b9e9e, 2.05, 1.28),
    row("Ru", 0x248f8f, 2.05, 1.25),
    row("Rh", 0x0a7d8c, 2.0, 1.25),
    row("Pd", 0x006985, 2.05, 1.2),
    row("Ag", 0xc0c0c0, 2.1, 1.28),
    row("Cd", 0xffd98f, 2.2, 1.36),
    row("In", 0xa67573, 2.2, 1.42),
    row("Sn", 0x668080, 2.25, 1.4),
    row("Sb", 0x9e63b5, 2.2, 1.4),
    row("Te", 0xd47a00, 2.1, 1.36),
    row("I", 0x940094, 2.1, 1.33),
    row("Xe", 0x429eb0, 2.16, 1.31),
    row("Cs", 0x57178f, 3.0, 2.32),
    row("Ba", 0x00c900, 2.7, 1.96),
    row("La", 0x70d4ff, 2.5, 1.8),
    row("Ce", 0xffffc7, 2.48, 1.63),
    row("Pr", 0xd9ffc7, 2.47, 1.76),
    row("Nd", 0xc7ffc7, 2.45, 1.74),
    row("Pm", 0xa3ffc7, 2.43, 1.73),
    row("Sm", 0x8fffc7, 2.42, 1.72),
    row("Eu", 0x61ffc7, 2.4, 1.68),
    row("Gd", 0x45ffc7, 2.38, 1.69),
    row("Tb", 0x30ffc7, 2.37, 1.68),
    row("Dy", 0x1fffc7, 2.35, 1.67),
    row("Ho", 0x00ff9c, 2.33, 1.66),
    row("Er", 0x00e675, 2.32, 1.65),
    row("Tm", 0x00d452, 2.3, 1.64),
    row("Yb", 0x00bf38, 2.28, 1.7),
    row("Lu", 0x00ab24, 2.27, 1.62),
    row("Hf", 0x4dc2ff, 2.25, 1.52),
    row("Ta", 0x4da6ff, 2.2, 1.46),
    row("W", 0x2194d6, 2.1, 1.37),
    row("Re", 0x267dab, 2.05, 1.31),
    row("Os", 0x266696, 2.0, 1.29),
    row("Ir", 0x175487, 2.0, 1.22),
    row("Pt", 0xd0d0e0, 2.05, 1.23),
    row("Au", 0xffd123, 2.1, 1.24),
    row("Hg", 0xb8b8d0, 2.05, 1.33),
    row("Tl", 0xa6544d, 2.2, 1.44),
    row("Pb", 0x575961, 2.3, 1.44),
    row("Bi", 0x9e4fb5, 2.3, 1.51),
    row("Po", 0xab5c00, 2.0, 1.45),
    row("At", 0x754f45, 2.0, 1.47),
    row("Rn", 0x428296, 2.0, 1.42),
    row("Fr", 0x420066, 2.0, 2.23),
    row("Ra", 0x007d00, 2.0, 2.01),
    row("Ac", 0x70abfa, 2.0, 1.86),
    row("Th", 0x00baff, 2.4, 1.75),
    row("Pa", 0x00a1ff, 2.0, 1.69),
    row("U", 0x008fff, 2.3, 1.7),
    row("Np", 0x0080ff, 2.0, 1.71),
    row("Pu", 0x006bff, 2.0, 1.72),
    row("Am", 0x545cf2, 2.0, 1.66),
    row("Cm", 0x785ce3, 2.0, 1.66),
    row("Bk", 0x8a4fe3, 2.0, 1.68),
    row("Cf", 0xa136d4, 2.0, 1.68),
    row("Es", 0xb31fd4, 2.0, 1.65),
    row("Fm", 0xb31fba, 2.0, 1.67),
    row("Md", 0xb30da6, 2.0, 1.73),
    row("No", 0xbd0d87, 2.0, 1.76),
    row("Lr", 0xc70066, 2.0, 1.61),
    row("Rf", 0xcc0059, 2.0, 1.57),
    row("Db", 0xd1004f, 2.0, 1.49),
    row("Sg", 0xd90045, 2.0, 1.43),
    row("Bh", 0xe00038, 2.0, 1.41),
    row("Hs", 0xe6002e, 2.0, 1.34),
    row("Mt", 0xeb0026, 2.0, 1.29),
];

impl Element {
    pub const HYDROGEN: Element = Element(1);

    /// The element whose symbol is written exactly `symbol`, such as `Cl`.
    pub fn from_symbol(symbol: &str) -> Option<Element> {
        ROWS.iter()
            .position(|row| row.symbol == symbol)
            .map(|i| Element(i as u8 + 1))
    }

    pub fn symbol(self) -> &'static str {
        self.row().symbol
    }

    /// The element color.
    pub fn color(self) -> Color {
        self.row().color
    }

    /// The van der Waals radius in angstroms.
    pub fn radius(self) -> f32 {
        self.row().radius
    }

    /// The covalent radius in angstroms. Two bonded atoms lie about the sum of theirs apart.
    pub fn covalent_radius(self) -> f32 {
        self.row().covalent_radius
    }

    fn row(self) -> &'static Row {
        &ROWS[usize::from(self.0) - 1]
    }
}

impl Serialize for Element {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.symbol())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rows of a source file in `tests/fixtures/elements`: atomic number, symbol, value.
    fn source(text: &str) -> Vec<(usize, &str, &str)> {
        text.lines()
            .map(|line| {
                let cells: Vec<&str> = line.split('\t').collect();
                (cells[0].parse().unwrap(), cells[1], cells[2])
            })
            .collect()
    }

    #[test]
    fn every_element_color_is_jmols() {
        let jmol = source(include_str!("../tests/fixtures/elements/jmol_colors.tsv"));

        assert_eq!(jmol.len(), ROWS.len());
        for (number, symbol, hex) in jmol {
            let element =
                Element::from_symbol(symbol).unwrap_or_else(|| panic!("no row for {symbol}"));
            let rgb = u32::from_str_radix(hex, 16).unwrap();
            assert_eq!(element, Element(number as u8), "{symbol}");
            assert_eq!(
                element.color(),
                row(symbol, rgb, 0.0, 0.0).color,
                "{symbol}"
            );
        }
    }

    #[test]
    fn every_radius_is_the_blue_obelisk_van_der_waals_radius() {
        let bodr = source(include_str!("../tests/fixtures/elements/bodr_radii.tsv"));

        assert_eq!(bodr.len(), ROWS.len());
        for (number, symbol, radius) in bodr {
            assert_eq!(Element(number as u8).symbol(), symbol);
            assert_eq!(
                Element(number as u8).radius(),
                radius.parse::<f32>().unwrap(),
                "{symbol}"
            );
        }
    }

    #[test]
    fn every_covalent_radius_is_the_blue_obelisk_one() {
        let bodr = source(include_str!(
            "../tests/fixtures/elements/bodr_covalent_radii.tsv"
        ));

        assert_eq!(bodr.len(), ROWS.len());
        for (number, symbol, radius) in bodr {
            assert_eq!(Element(number as u8).symbol(), symbol);
            assert_eq!(
                Element(number as u8).covalent_radius(),
                radius.parse::<f32>().unwrap(),
                "{symbol}"
            );
        }
        assert_eq!(Element::from_symbol("P").unwrap().covalent_radius(), 1.11);
    }

    #[test]
    fn symbols_are_exact_and_unique() {
        assert_eq!(Element::from_symbol("CL"), None);
        assert_eq!(Element::from_symbol("Xx"), None);
        assert_eq!(
            Element::from_symbol("Ds"),
            None,
            "element 110 has no Jmol color"
        );
        for (i, row) in ROWS.iter().enumerate() {
            assert_eq!(
                Element::from_symbol(row.symbol),
                Some(Element(i as u8 + 1)),
                "{} appears twice",
                row.symbol
            );
        }
    }
}
