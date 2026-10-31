//! Atom groups, the glow that highlights them, and the objects attached beside them.
//!
//! A group is named data on a molecule object: atom numbers checked against the structure file
//! when the scene file is read. Everything derived from it, the glow's shells, the center an
//! attachment is measured from and the boxes `inspect` reports, is a pure function of the group, the
//! representation and the stage, so no group state lives anywhere but on the molecule object.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use glam::{Quat, Vec2, Vec3};
use serde::Serialize;

use crate::molecule::{
    AtomNumber, ChainId, Molecule, Representation, Residue, StandardResidue, in_nucleobase,
};
use crate::scene::{Camera, Color, FlatToWorld, ObjectId, Transform, UnitInterval};
use crate::scene_file::{JsonPath, SceneError, SceneErrorCode};
use crate::timeline::{Mix, Progress};

/// The name a molecule gives a group: a key of its `groups`. Only `Groups::read` makes one, so
/// holding a `GroupId` together with its `Groups` means the group exists.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GroupId(Arc<str>);

impl std::borrow::Borrow<str> for GroupId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl Serialize for GroupId {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl fmt::Display for GroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Some atoms of one molecule, in atom number order.
#[derive(Clone, Debug)]
pub struct AtomGroup {
    pub atoms: Vec<AtomNumber>,
    /// The point an attachment is measured from, in the molecule's own space: the mean of the group's
    /// atoms the representation shows, or of all of them when it shows none. Hidden atoms do not
    /// pull the center toward nothing.
    pub center: Vec3,
}

/// How a scene file writes one group's atoms, before they are checked against the molecule.
#[derive(Clone, Debug)]
pub enum GroupAtoms {
    Numbers(Vec<u32>),
    /// Every atom of the residues the picks name, or only their nucleobases' atoms.
    Residues {
        picks: Vec<ResiduePick>,
        nucleobases: bool,
    },
}

/// Some residues of one chain of a PDB file, as an atom group names them.
#[derive(Clone, Debug)]
pub struct ResiduePick {
    pub chain: ChainId,
    pub residues: ResidueRun,
}

/// Which residues of its chain a residue pick names.
#[derive(Clone, Copy, Debug)]
pub enum ResidueRun {
    /// Every residue of this residue number, whatever its insertion code.
    One(i32),
    /// From one residue number to another, both included.
    Between {
        from: i32,
        to: i32,
    },
    WholeChain,
}

/// Every group of one molecule object. Shared, because a stage is cloned for every frame.
#[derive(Clone, Debug, Default)]
pub struct Groups(Arc<BTreeMap<GroupId, AtomGroup>>);

impl Groups {
    pub fn get(&self, id: &GroupId) -> Option<&AtomGroup> {
        self.0.get(id)
    }

    pub fn find(&self, id: &str) -> Option<(&GroupId, &AtomGroup)> {
        self.0.get_key_value(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&GroupId, &AtomGroup)> {
        self.0.iter()
    }

    /// Checks `file`, a molecule's `groups` as written, against the molecule at `path`
    /// (`/objects/<id>/groups`): every atom number exists and no atom is listed twice in one group.
    /// An empty group is the schema's error, not this function's. Returns every error found.
    pub fn read(
        path: &JsonPath,
        file: &BTreeMap<String, GroupAtoms>,
        molecule: &Molecule,
        representation: Representation,
    ) -> Result<Groups, Vec<SceneError>> {
        let mut errors = Vec::new();
        let mut groups = BTreeMap::new();
        for (id, written) in file {
            let at = path.field(id);
            let mut atoms: Vec<AtomNumber> = Vec::new();
            match written {
                GroupAtoms::Numbers(numbers) => {
                    for (i, &n) in numbers.iter().enumerate() {
                        match molecule.atom_number(n) {
                            None => {
                                let message = format!(
                                    "the molecule has {} atoms, and none is numbered {n}",
                                    molecule.atom_count()
                                );
                                errors.push(SceneError::new(
                                    at.index(i),
                                    SceneErrorCode::NoSuchAtom,
                                    message,
                                ));
                            }
                            Some(number) if atoms.contains(&number) => {
                                errors.push(SceneError::new(
                                    at.index(i),
                                    SceneErrorCode::Invalid,
                                    format!("atom {n} is already in group `{id}`"),
                                ));
                            }
                            Some(number) => atoms.push(number),
                        }
                    }
                }
                GroupAtoms::Residues { picks, nucleobases } => {
                    for (i, pick) in picks.iter().enumerate() {
                        match residue_atoms(molecule, pick, *nucleobases) {
                            Ok(found) => atoms.extend(found),
                            Err((field, code, message)) => {
                                let at = at.field("residues").index(i);
                                let at = field.map_or(at.clone(), |f| at.field(f));
                                errors.push(SceneError::new(at, code, message));
                            }
                        }
                    }
                }
            }
            atoms.sort();
            atoms.dedup();
            let shown: Vec<Vec3> = atoms
                .iter()
                .map(|&n| molecule.atom(n))
                .filter(|a| representation.shows(a))
                .map(|a| a.position)
                .collect();
            let positions: Vec<Vec3> = if shown.is_empty() {
                atoms.iter().map(|&n| molecule.atom(n).position).collect()
            } else {
                shown
            };
            let center = positions.iter().copied().sum::<Vec3>() / positions.len().max(1) as f32;
            groups.insert(GroupId(Arc::from(id.as_str())), AtomGroup { atoms, center });
        }
        if errors.is_empty() {
            Ok(Groups(Arc::new(groups)))
        } else {
            Err(errors)
        }
    }
}

/// A fault in one residue pick: the field of the pick it is at, if one, its code and its message.
type PickFault = (Option<&'static str>, SceneErrorCode, String);

/// Every atom of the residues `pick` names, or with `nucleobases` every atom of their
/// nucleobases, or why it names none.
fn residue_atoms(
    molecule: &Molecule,
    pick: &ResiduePick,
    nucleobases: bool,
) -> Result<Vec<AtomNumber>, PickFault> {
    let residues = molecule.residues();
    if residues.is_empty() {
        return Err((
            None,
            SceneErrorCode::NoSuchResidue,
            "the molecule is read from an SDF file, which has no residues; name the group's atoms by their atom numbers, as `[1, 9]`".to_string(),
        ));
    }
    let chain: Vec<&Residue> = residues.iter().filter(|r| r.chain == pick.chain).collect();
    if chain.is_empty() {
        let mut chains: Vec<String> = residues.iter().map(|r| format!("`{}`", r.chain)).collect();
        chains.dedup();
        return Err((
            Some("chain"),
            SceneErrorCode::NoSuchResidue,
            format!(
                "the molecule has no chain `{}`; its chains are {}",
                pick.chain,
                chains.join(", ")
            ),
        ));
    }
    let numbered = |field: &'static str, n: i32| {
        if chain.iter().any(|r| r.number.number == n) {
            return Ok(());
        }
        let (first, last) = (chain[0].number, chain[chain.len() - 1].number);
        Err((
            Some(field),
            SceneErrorCode::NoSuchResidue,
            format!(
                "chain `{}` has no residue numbered {n}; its {} residues are numbered {first} to {last}",
                pick.chain,
                chain.len()
            ),
        ))
    };
    let (from, to) = match pick.residues {
        ResidueRun::One(n) => {
            numbered("number", n)?;
            (n, n)
        }
        ResidueRun::Between { from, to } => {
            if from > to {
                return Err((
                    Some("to"),
                    SceneErrorCode::Invalid,
                    format!(
                        "`to` is {to}, before `from`, {from}; give the lower residue number first"
                    ),
                ));
            }
            numbered("from", from)?;
            numbered("to", to)?;
            (from, to)
        }
        ResidueRun::WholeChain => (i32::MIN, i32::MAX),
    };
    let picked = chain
        .iter()
        .filter(|r| (from..=to).contains(&r.number.number));
    if !nucleobases {
        return Ok(picked
            .flat_map(|r| r.atoms.iter().map(|&(_, n)| n))
            .collect());
    }
    if let Some(other) = picked
        .clone()
        .find(|r| StandardResidue::of(&r.name) != Some(StandardResidue::Nucleotide))
    {
        return Err((
            None,
            SceneErrorCode::Invalid,
            format!(
                "residue {} of chain `{}` is {}, which is not a nucleotide and has no nucleobase; leave out `part` to hold all its atoms",
                other.number, pick.chain, other.name
            ),
        ));
    }
    Ok(picked
        .flat_map(|r| r.atoms.iter())
        .filter(|(atom_name, _)| in_nucleobase(atom_name))
        .map(|&(_, n)| n)
        .collect())
}

/// How lit one group is: `share` is 0 for no glow and 1 for all of it, in `color`. A highlight
/// animation's track carries it, as a fade's carries an opacity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glow {
    pub share: UnitInterval,
    pub color: Color,
}

impl Glow {
    pub const DEFAULT_COLOR: Color = Color {
        r: 0xff,
        g: 0xd2,
        b: 0x3f,
    };

    pub fn off() -> Glow {
        Glow {
            share: UnitInterval::clamped(0.0),
            color: Glow::DEFAULT_COLOR,
        }
    }
}

impl Mix for Glow {
    fn mix(self, to: Glow, progress: Progress) -> Glow {
        let p = progress.get();
        let share = self.share.get() + (to.share.get() - self.share.get()) * p;
        // A glow that is off shows no color, so it lights up in the color it is going to.
        let color = if self.share.get() == 0.0 {
            to.color
        } else {
            self.color.mix(to.color, p.clamp(0.0, 1.0))
        };
        Glow {
            share: UnitInterval::clamped(share),
            color,
        }
    }
}

/// How a representation draws a lit group's glow: a shell `padding` angstroms larger than each
/// of the group's balls, how opaque the shell is at glow share 1, and whether the key light shades
/// it as it does the balls or it shows its color evenly. The atoms inside keep their element
/// colors under it.
#[derive(Clone, Copy, Debug)]
pub struct GlowLook {
    pub padding: f32,
    pub opacity: f32,
    pub key_light: bool,
}

/// The radius of the tube a glow lays along each bond a representation draws as sticks between
/// two atoms of the group, in angstroms.
pub const GLOW_BOND_RADIUS: f32 = 0.3;

/// A ball-and-stick glow is a faint film well clear of its small balls, so it shows around them
/// and the sticks between them.
pub const BALL_AND_STICK_GLOW: GlowLook = GlowLook {
    padding: 0.3,
    opacity: 0.18,
    key_light: false,
};

/// A space-filling glow lies close over balls that touch their neighbors, so a padding as wide as
/// ball-and-stick's would spill over the atoms around the group, and it is opaque enough to color
/// the group's atoms against the thousands unlit around them.
pub const SPACE_FILLING_GLOW: GlowLook = GlowLook {
    padding: 0.05,
    opacity: 0.75,
    key_light: true,
};

/// A cartoon's glow is a faint film standing off its tube, as ball-and-stick's stands off its
/// balls and sticks.
pub const CARTOON_GLOW: GlowLook = BALL_AND_STICK_GLOW;

/// An object placed beside another object, or beside an atom group of a molecule, instead of by
/// a transform of its own: its position is that object's position, or the group's center, as the
/// frame shows it, plus `offset` along the screen's right and up, and it faces the camera. The
/// caption of a group is a text or math object with one of these, a leader line is an arrow with
/// one, and a number attached to its box moves with the box.
#[derive(Clone, Debug, PartialEq)]
pub struct Attachment {
    pub object: ObjectId,
    /// Set when `object` is a molecule and the attachment is to one of its atom groups.
    pub group: Option<GroupId>,
    /// Along the camera's right and up, in world units measured in the plane through the
    /// object's position that faces the camera.
    pub offset: Vec2,
}

impl Attachment {
    /// The transform that places an attached object: a pure function of where the object it is
    /// attached to lands in the world, plane map included, and the camera at one scene time.
    /// `center` is the point it sits at in that object's own space: the origin, or a group's
    /// center. `reach` is how far the object's farthest drawn point lies from its position, in
    /// its own space.
    ///
    /// The attached object is drawn in the plane facing the camera just nearer than that reach,
    /// so nothing of the object covers it, and scaled by that plane's depth over the object's,
    /// so it shows the size and pixel offset it would have in the object's own depth plane.
    /// Those stay the same while the object turns. The center is carried along its ray from the
    /// camera into the plane, so it keeps its pixel, and the offset is added along the frame's
    /// right and up.
    pub fn transform(
        &self,
        object: &FlatToWorld,
        center: Vec3,
        reach: f32,
        camera: &Camera,
    ) -> Transform {
        let forward = (camera.target - camera.position).normalize();
        let right = forward.cross(camera.up).normalize();
        let up = right.cross(forward);
        let center_world = object.to_world().transform_point3(center);
        let center_depth = (center_world - camera.position).dot(forward);
        let object_depth = (object.position() - camera.position).dot(forward);
        let (carried, shrink) = if center_depth > NEAREST_DEPTH && object_depth > NEAREST_DEPTH {
            let depth = (object_depth - reach * object.stretch()).max(NEAREST_DEPTH);
            (
                camera.position + (center_world - camera.position) * (depth / center_depth),
                depth / object_depth,
            )
        } else {
            (center_world, 1.0)
        };
        let facing = Quat::from_mat3(&glam::Mat3::from_cols(right, up, -forward));
        let (x, y, z) = facing.to_euler(glam::EulerRot::XYZ);
        let offset = (right * self.offset.x + up * self.offset.y) * shrink;
        Transform {
            position: carried + offset,
            rotation: Vec3::new(x, y, z) * (180.0 / std::f32::consts::PI),
            scale: Vec3::splat(shrink),
        }
    }
}

/// An attached object nearer the camera than this, in world units, would be cut by the frame's
/// near plane.
const NEAREST_DEPTH: f32 = 0.2;

/// What `inspect` says of one group.
#[derive(Debug, Serialize)]
pub struct GroupReport {
    pub atoms: Vec<AtomNumber>,
    /// The center in world units, and the pixel it lands on.
    pub center: Vec3,
    /// Where the center lands in the frame, in pixels from the top left.
    pub center_pixels: Option<Vec2>,
    /// The box around the group's glow shell, which is the group's atoms and bonds the
    /// representation shows plus their padding, as the frame would show them lit. `None` for a
    /// group of hidden atoms, which has nothing to draw.
    pub bounding_box: Option<crate::mesh::BoundingBox>,
    /// The glow share at this scene time and its color.
    pub glow: f32,
    pub color: String,
}

/// What `inspect` says of an attached object beyond its box: what it is attached to, and on a
/// molecule the atoms its pixel box covers, so an agent can move a caption that sits on atoms.
#[derive(Debug, Serialize)]
pub struct AttachedReport {
    pub object: ObjectId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupId>,
    /// The shown atoms of the molecule whose ball's pixel box meets this object's pixel box.
    /// Empty when the object has no pixel box, and left out when it is attached to no molecule.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub covers_atoms: Option<Vec<AtomNumber>>,
}

impl Representation {
    /// How far the farthest shown ball surface, or the cartoon's tube, of `molecule` lies from its
    /// centroid, in its own space.
    pub fn reach(self, molecule: &Molecule) -> f32 {
        if self == Representation::Cartoon {
            return molecule.cartoon().reach();
        }
        molecule
            .numbers()
            .map(|n| molecule.atom(n))
            .filter_map(|a| Some(a.position.length() + self.ball_radius(a)?))
            .fold(0.0, f32::max)
    }

    pub fn glow_look(self) -> GlowLook {
        match self {
            Representation::BallAndStick { .. } => BALL_AND_STICK_GLOW,
            Representation::SpaceFilling { .. } => SPACE_FILLING_GLOW,
            Representation::Cartoon => CARTOON_GLOW,
        }
    }

    /// Whether a glow of `group` draws anything: whether it holds a shown atom, or on a cartoon
    /// the trace atom of a residue, whose stretch of the tube it lights.
    pub fn lights(self, molecule: &Molecule, group: &AtomGroup) -> bool {
        match self {
            Representation::Cartoon => {
                let cartoon = molecule.cartoon();
                group.atoms.iter().any(|&n| cartoon.traces(n))
            }
            _ => group.atoms.iter().any(|&n| self.shows(molecule.atom(n))),
        }
    }

    /// The shell of a group's glow in the molecule's own space, as `glow_look` says: a ball a
    /// padding larger around each shown atom of the group, and in ball-and-stick a thicker stick
    /// along each bond between two of them. Hidden atoms and their bonds get no shell, as they
    /// get no ball or stick. On a cartoon, the tube a padding larger along the residues whose
    /// trace atom the group holds.
    pub fn glow(
        self,
        molecule: &Molecule,
        group: &AtomGroup,
        color: Color,
    ) -> crate::molecule::Drawn {
        use crate::molecule::{Ball, Stick};
        if self == Representation::Cartoon {
            return crate::molecule::Drawn {
                balls: Vec::new(),
                sticks: Vec::new(),
                tubes: molecule.cartoon().glow(group, color),
            };
        }
        let look = self.glow_look();
        let balls = group
            .atoms
            .iter()
            .filter_map(|&n| {
                let atom = molecule.atom(n);
                Some(Ball {
                    center: atom.position,
                    radius: self.ball_radius(atom)? + look.padding,
                    color,
                })
            })
            .collect();
        let in_group = |n: &AtomNumber| group.atoms.binary_search(n).is_ok();
        let sticks = molecule
            .bonds()
            .iter()
            .filter(|bond| bond.atoms.iter().all(in_group) && self.shows_bond(molecule, bond))
            .map(|bond| Stick {
                from: molecule.atom(bond.atoms[0]).position,
                to: molecule.atom(bond.atoms[1]).position,
                radius: GLOW_BOND_RADIUS,
                color,
            })
            .collect();
        crate::molecule::Drawn {
            balls,
            sticks,
            tubes: Vec::new(),
        }
    }
}
