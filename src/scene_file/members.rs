//! The member an animation names beside its `object`, read and checked against that object.

use super::*;

/// The member fields an animation writes, each `None` when it is left out or when the animation
/// has no such field.
#[derive(Default)]
pub(super) struct MemberFields {
    pub(super) group: Option<String>,
    pub(super) cell: Option<[u32; 2]>,
    pub(super) copy: Option<Vec<u32>>,
    pub(super) bar: Option<u32>,
}

/// The member `written` names on `object_id`, `None` when it names none, or the scene error at
/// the field that names one the object does not have: a member of another kind of object, an
/// atom group, cell, copy or bar the object lacks, a copy index of the wrong length for its
/// pattern's layout, or an atom group whose atoms are all hidden.
pub(super) fn read_member(
    object_id: &ObjectId,
    written: MemberFields,
    start: &Stage,
    path: &JsonPath,
) -> Result<Option<Member>, SceneError> {
    let shape = &start.objects[object_id].shape;
    let error =
        |field: &str, code, message: String| Err(SceneError::new(path.field(field), code, message));
    let given: Vec<&str> = [
        ("group", written.group.is_some()),
        ("cell", written.cell.is_some()),
        ("copy", written.copy.is_some()),
        ("bar", written.bar.is_some()),
    ]
    .into_iter()
    .filter_map(|(field, is_given)| is_given.then_some(field))
    .collect();
    if let [first, second, ..] = given[..] {
        return error(
            second,
            SceneErrorCode::Invalid,
            format!(
                "an animation names one member of its object, and this one names `{first}` and `{second}`; give one of them"
            ),
        );
    }
    if let Some(group) = written.group {
        return match shape {
            Shape::Molecule {
                molecule,
                representation,
                groups,
                ..
            } => {
                let Some((group_id, atom_group)) = groups.find(&group) else {
                    let known: Vec<String> = groups.iter().map(|(g, _)| format!("`{g}`")).collect();
                    let known = if known.is_empty() {
                        "it has no groups; declare them in its `groups`".to_string()
                    } else {
                        format!("its groups are {}", known.join(", "))
                    };
                    return error(
                        "group",
                        SceneErrorCode::UnknownGroup,
                        format!("`{object_id}` has no group `{group}`; {known}"),
                    );
                };
                if !representation.lights(molecule, atom_group) {
                    let message = match representation {
                        Representation::Cartoon => format!(
                            "group `{group_id}` of `{object_id}` holds no alpha carbon `CA` of an amino acid or phosphorus `P` of a nucleotide, and a cartoon lights the tube along each residue whose `CA` or `P` the group holds, so its glow would draw nothing; write the group by its residues, as `{{ \"residues\": [{{ \"chain\": \"A\", \"from\": 23, \"to\": 34 }}] }}`"
                        ),
                        Representation::BallAndStick { .. }
                        | Representation::SpaceFilling { .. } => format!(
                            "every atom of group `{group_id}` of `{object_id}` is hidden, so its glow would draw nothing; show its hydrogens with `\"hydrogens\": true` or add a shown atom to the group"
                        ),
                    };
                    return error("group", SceneErrorCode::GroupHidden, message);
                }
                Ok(Some(Member::Group(group_id.clone())))
            }
            Shape::Table(_) => error(
                "group",
                SceneErrorCode::UnknownGroup,
                format!(
                    "`{object_id}` is a table, whose cells a highlight names by `cell`, `[row, column]`, not by `group`"
                ),
            ),
            _ => error(
                "object",
                SceneErrorCode::NotMolecule,
                format!(
                    "`group` names an atom group of a molecule, and `{object_id}` is not a molecule; {}",
                    what_members(shape)
                ),
            ),
        };
    }
    if let Some([row, column]) = written.cell {
        let Shape::Table(table) = shape else {
            return error(
                "cell",
                SceneErrorCode::NotTable,
                format!(
                    "`cell` names a cell of a table, and `{object_id}` is not a table; {}",
                    what_members(shape)
                ),
            );
        };
        let address = CellAddress {
            row: row as usize,
            column: column as usize,
        };
        if table.cell(address).is_none() {
            return error(
                "cell",
                SceneErrorCode::UnknownCell,
                format!(
                    "`{object_id}` has no cell {address}: it has {} rows and {} columns, so a cell is from [0, 0] to [{}, {}]",
                    table.rows,
                    table.columns,
                    table.rows - 1,
                    table.columns - 1
                ),
            );
        }
        return Ok(Some(Member::Cell(address)));
    }
    if let Some(written) = written.copy {
        let Shape::Pattern(pattern) = shape else {
            return error(
                "copy",
                SceneErrorCode::NotPattern,
                format!(
                    "`copy` names a copy of a pattern by its index, and `{object_id}` is not a pattern; {}",
                    what_members(shape)
                ),
            );
        };
        let last = pattern.copies.last().expect("a pattern has a copy").index;
        let index = match (written.as_slice(), last) {
            (&[i, j, k], CopyIndex::Grid(_)) => CopyIndex::Grid([i, j, k]),
            (&[i], CopyIndex::Line(_)) => CopyIndex::Line([i]),
            _ => {
                let numbered = match last {
                    CopyIndex::Grid(_) => "a grid's copy by three numbers, `[i, j, k]`",
                    CopyIndex::Line(_) => "a ring's or an along's copy by one number, `[i]`",
                };
                return error(
                    "copy",
                    SceneErrorCode::WrongLength,
                    format!("`{object_id}` numbers {numbered}, each from 0"),
                );
            }
        };
        if pattern.ordinal(index).is_none() {
            return error(
                "copy",
                SceneErrorCode::UnknownCopy,
                format!(
                    "`{object_id}` has no copy {index}: its copies run from {} to {last}",
                    pattern.copies[0].index
                ),
            );
        }
        return Ok(Some(Member::Copy(index)));
    }
    if let Some(k) = written.bar {
        let Shape::BarPlot(bar_plot) = shape else {
            return error(
                "bar",
                SceneErrorCode::NotBarPlot,
                format!(
                    "`bar` names a bar of a bar plot by its index, and `{object_id}` is not a bar plot; {}",
                    what_members(shape)
                ),
            );
        };
        let count = bar_plot.bars.len();
        if k as usize >= count {
            return error(
                "bar",
                SceneErrorCode::UnknownBar,
                format!(
                    "`{object_id}` has {count} bars, one for each category, so a bar is from 0 to {}",
                    count - 1
                ),
            );
        }
        return Ok(Some(Member::Bar(k as usize)));
    }
    Ok(None)
}

/// The scene error of a highlight that names no member of `object_id`, at the field that names
/// the kind of member it has, or at `object` when it has none.
pub(super) fn no_member(object_id: &ObjectId, start: &Stage, path: &JsonPath) -> SceneError {
    let shape = &start.objects[object_id].shape;
    let missing = |field: &str, message: String| {
        SceneError::new(path.field(field), SceneErrorCode::MissingField, message)
    };
    match shape {
        Shape::Molecule { .. } => missing(
            "group",
            format!(
                "a highlight of the molecule `{object_id}` names the atom group it lights in `group`"
            ),
        ),
        Shape::Table(_) => missing(
            "cell",
            format!(
                "a highlight of the table `{object_id}` names the cell it lights in `cell`, `[row, column]`, each counted from 0"
            ),
        ),
        Shape::Pattern(_) => missing(
            "copy",
            format!(
                "a highlight of the pattern `{object_id}` names the copy it lights in `copy`, by its index"
            ),
        ),
        Shape::BarPlot(_) => missing(
            "bar",
            format!(
                "a highlight of the bar plot `{object_id}` names the bar it lights in `bar`, by its index from 0"
            ),
        ),
        _ => SceneError::new(
            path.field("object"),
            SceneErrorCode::NotMolecule,
            format!(
                "`highlight` lights an atom group of a molecule, a cell of a table, a copy of a pattern or a bar of a bar plot, and `{object_id}` is none of them"
            ),
        ),
    }
}

/// What members `shape` has, and the field that names one, for a message about a member field
/// of the wrong kind.
fn what_members(shape: &Shape) -> &'static str {
    match shape {
        Shape::Molecule { .. } => "a molecule's atom group is named by `group`",
        Shape::Table(_) => "a table's cell is named by `cell`",
        Shape::Pattern(_) => "a pattern's copy is named by `copy`",
        Shape::BarPlot(_) => "a bar plot's bar is named by `bar`",
        _ => "it has no members, so name only the `object`",
    }
}
