//! The one boundary between a scene file and a `Scene`.
//!
//! `read` validates the JSON against `schema()`, the same document `sci_anim schema` prints, so
//! every schema violation is reported at once with its JSON path. Only a document that passed
//! is deserialized, and then checked for what a schema cannot say: a camera that looks nowhere,
//! a shape with no length, a `d` that is not SVG path syntax, text or math that does not
//! typeset, a structure file that cannot be read, an animation of an unknown object, and a create
//! of a shape with no stroke. A structure file's path is relative to the scene file, and the file
//! is read here, once.
//!
//! This module checks the document and reads it into a `Scene`. `objects` and `animations` hold
//! the two halves of the file format, `shapes` reads an object's shape, `placing` places what is
//! placed by another object, and `animations` also reads each animation into a `Change`.

use std::collections::BTreeMap;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use jsonschema::error::ValidationErrorKind;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use glam::{Vec2, Vec3};

use crate::axes::{Appearance, Axes, Axis, AxisError, LabelError, NumberLine};
use crate::axes_3d::Axes3d;
use crate::bar_plot::{BarPlot, BarPlotError};
use crate::ends::{End, Ends};
use crate::expr::{self, Expression, ExpressionError};
use crate::follow::{Carry, Faces, Follow, Followed, Law, LawKind, LawTimeStart};
use crate::group::{Attachment, Glow, GroupAtoms, Groups};
use crate::member::Member;
use crate::molecule::{Representation, StandardResidue};
use crate::morph::MorphEnd;
use crate::path::{Path, PathError, UnderMap};
use crate::pattern::{self, CopyExpressions, CopyIndex, PaletteIndex, Pattern, ToColor};
use crate::plane_map::{LinearMap, MatrixRoute, NoTurningRoute};
use crate::plot::{self, PlotError};
use crate::reaction::{AtomPairFile, Reaction};
use crate::readout::Readout;
use crate::scene::{
    Align, Camera, CellPadding, Color, CopyCount, Decimals, Duration, EdgeShade, FieldOfView, Fill,
    FillRule, FiniteNumber, Font, Material, MatrixEntry, Object, ObjectId, OriginCoordinate,
    PlacedBy, Positive, Region, RevolutionAngle, Scene, SceneConfig, SecondsIntoStep, Shape, Solid,
    Sound, Stage, Stroke, Sweep, Transform, Turns, UnitInterval,
};
use crate::simulate::{Set, Simulate};
use crate::space_curve::{self, SpaceCurve, SpaceCurveError};
use crate::structure_file::{self, StructureFileError};
use crate::surface::{Placing, Sampling, Surface, SurfaceError};
use crate::table::{CellAddress, CellContent, Rulings, Table};
use crate::timeline::{Animation, Change, ChapterId, EasingCurve, Timeline};
use crate::trail::Trail;
use crate::typeset::{self, Layout, TypesetError};

mod animations;
mod members;
mod objects;
mod placing;
mod shapes;

use animations::*;
use members::*;
use objects::*;
use placing::*;
use shapes::*;
pub use shapes::{ShapeFault, ShapeLaw};

pub fn read(scene_file_path: &std::path::Path) -> Result<Scene, ReadError> {
    let text = std::fs::read_to_string(scene_file_path).map_err(|source| ReadError::Io {
        file: scene_file_path.to_path_buf(),
        source,
    })?;
    let value: Value = serde_json::from_str(&text).map_err(|e| {
        ReadError::Invalid(vec![SceneError::new(
            JsonPath::root(),
            SceneErrorCode::InvalidJson,
            e.to_string(),
        )])
    })?;
    let errors = schema_errors(&value);
    if !errors.is_empty() {
        return Err(ReadError::Invalid(errors));
    }
    let scene_file: SceneFile = serde_json::from_value(value).map_err(|e| {
        let message = format!(
            "the scene file passed the schema but did not parse, which is a sci_anim bug: {e}"
        );
        ReadError::Invalid(vec![SceneError::new(
            JsonPath::root(),
            SceneErrorCode::Internal,
            message,
        )])
    })?;
    scene_file
        .into_scene(scene_file_path.parent().unwrap_or(std::path::Path::new("")))
        .map_err(ReadError::Invalid)
}

/// The JSON Schema of a scene file, derived from the types serde parses it into.
pub fn schema() -> &'static Value {
    static SCHEMA: OnceLock<Value> = OnceLock::new();
    SCHEMA.get_or_init(|| {
        let mut schema =
            serde_json::to_value(schemars::schema_for!(SceneFile)).expect("a schema is JSON");
        drop_null_branches(&mut schema);
        discriminate(&mut schema);
        strict(&mut schema);
        schema
    })
}

fn validator() -> &'static jsonschema::Validator {
    static VALIDATOR: OnceLock<jsonschema::Validator> = OnceLock::new();
    VALIDATOR
        .get_or_init(|| jsonschema::validator_for(schema()).expect("the derived schema compiles"))
}

fn schema_errors(value: &Value) -> Vec<SceneError> {
    let mut errors: Vec<SceneError> = validator()
        .iter_errors(value)
        .flat_map(|e| scene_errors(&e))
        .collect();
    errors.sort();
    errors.dedup();
    errors
}

/// A `required` or `additionalProperties` error names the object, so it moves to the field.
fn scene_errors(error: &jsonschema::ValidationError<'_>) -> Vec<SceneError> {
    let path = JsonPath(error.instance_path().to_string());
    let one = |code| vec![SceneError::new(path.clone(), code, error.to_string())];
    match error.kind() {
        ValidationErrorKind::Required { property } => {
            let field = property.as_str().unwrap_or_default();
            vec![SceneError::new(
                path.field(field),
                SceneErrorCode::MissingField,
                format!("missing field `{field}`"),
            )]
        }
        ValidationErrorKind::AdditionalProperties { unexpected } => {
            let expected = allowed_fields(error.schema_path().as_str());
            unexpected
                .iter()
                .map(|field| {
                    let mut message =
                        format!("unknown field `{field}`, expected one of {expected}");
                    if let Some(hint) = field_hint(error.instance(), field) {
                        message.push_str("; ");
                        message.push_str(hint);
                    }
                    SceneError::new(path.field(field), SceneErrorCode::UnknownField, message)
                })
                .collect()
        }
        ValidationErrorKind::Type { .. } => one(SceneErrorCode::WrongType),
        ValidationErrorKind::Minimum { .. }
        | ValidationErrorKind::Maximum { .. }
        | ValidationErrorKind::ExclusiveMinimum { .. }
        | ValidationErrorKind::ExclusiveMaximum { .. } => one(SceneErrorCode::OutOfRange),
        ValidationErrorKind::MinItems { .. }
        | ValidationErrorKind::MaxItems { .. }
        | ValidationErrorKind::MinProperties { .. } => one(SceneErrorCode::WrongLength),
        ValidationErrorKind::Pattern { .. } => one(SceneErrorCode::BadFormat),
        ValidationErrorKind::Enum { options } => {
            let allowed: Vec<String> = options
                .as_array()
                .into_iter()
                .flatten()
                .map(Value::to_string)
                .collect();
            let mut message = format!("{} is not one of {}", error.instance(), allowed.join(", "));
            if let Some(hint) = type_hint(&path, error.instance()) {
                message.push_str("; ");
                message.push_str(hint);
            }
            vec![SceneError::new(path, SceneErrorCode::UnknownValue, message)]
        }
        ValidationErrorKind::Constant { .. } => one(SceneErrorCode::UnknownValue),
        // An end is a point or an end on an object: the branch of the value's own JSON type says
        // what is wrong with it. A space curve is expressions or points, both objects: the branch
        // whose fields the object has says.
        ValidationErrorKind::AnyOf { context } => {
            let fails_here =
                |branch: &Vec<jsonschema::ValidationError<'static>>,
                 wrong: fn(&ValidationErrorKind) -> bool| {
                    branch
                        .iter()
                        .any(|e| wrong(e.kind()) && e.instance_path() == error.instance_path())
                };
            let of_its_type: Vec<&Vec<jsonschema::ValidationError<'static>>> = context
                .iter()
                .filter(|branch| {
                    !fails_here(branch, |kind| {
                        matches!(kind, ValidationErrorKind::Type { .. })
                    })
                })
                .collect();
            let with_its_fields: Vec<_> = of_its_type
                .iter()
                .copied()
                .filter(|branch| {
                    !fails_here(branch, |kind| {
                        matches!(
                            kind,
                            ValidationErrorKind::Required { .. }
                                | ValidationErrorKind::AdditionalProperties { .. }
                        )
                    })
                })
                .collect();
            match (of_its_type.as_slice(), with_its_fields.as_slice()) {
                ([branch], _) | (_, [branch]) => branch.iter().flat_map(scene_errors).collect(),
                _ => one(SceneErrorCode::Invalid),
            }
        }
        _ => one(SceneErrorCode::Invalid),
    }
}

/// What to write for an object `type` that names a solid or a pattern sci_anim makes another way.
fn type_hint(path: &JsonPath, value: &Value) -> Option<&'static str> {
    let fields: Vec<&str> = path.0.split('/').collect();
    if !matches!(fields.as_slice(), ["", "objects", _, "type"]) {
        return None;
    }
    Some(match value.as_str()? {
        "box" | "cube" | "cuboid" => "a box is a `prism` whose `base` is a square `polygon`",
        "cylinder" => "a cylinder is a `prism` whose `base` is a `circle`",
        "cone" => {
            "a cone is a `revolution` whose `profile` is a right-triangle `polygon` with one side on the y axis, such as `[[0, 0], [1, 0], [0, 2]]`"
        }
        "torus" => {
            "a torus is a `revolution` whose `profile` is a `circle` with its `center` beside the y axis"
        }
        "helix" | "coil" => {
            "a coil between two points is a `spring`, whose `from` and `to` may sit on objects; a helix on its own is a `tube` whose `curve` is `{ \"x\": \"cos(u)\", \"y\": \"0.1*u\", \"z\": \"sin(u)\", \"u_range\": [0, 25] }`"
        }
        "space-curve" | "curve" | "parametric-curve" | "curve-3d" => {
            "a space curve is drawn as a `tube` whose `curve` writes x, y and z as expressions in `u`, or the `points` it passes through"
        }
        "pipe" | "wire" | "cable" => {
            "a bent pipe or wire is a `tube` along a space curve, and a straight pipe a `prism` on a ring of two circles under `even-odd`"
        }
        "surface-plot" | "heightmap" | "height-map" | "terrain" | "plot-3d" | "plot3d"
        | "graph-3d" => {
            "a sheet of heights is a `surface`, whose `z` is an expression in `x` and `y` over `x_range` and `y_range`"
        }
        "parametric-surface" => {
            "a `surface` writes only a height: its `z` is an expression in `x` and `y` over `x_range` and `y_range`, and a sheet that is not a height over a rectangle cannot be drawn yet"
        }
        "ball" => "a ball is a `sphere`",
        "lattice" | "crystal" => "a lattice is a `pattern` of a `sphere` with a `grid` layout",
        "array" | "repeat" => {
            "copies of one shape are a `pattern`: its `shape` at every copy of a `grid`, a `ring` or `along` a curve"
        }
        "vector-field" => {
            "a vector field is a `pattern` of an `arrow` with a `grid` layout, each copy turned and scaled by expressions in its place in `copy`"
        }
        "three-d-axes" | "axes-3d" | "3d-axis" | "axes3d" | "threedaxes" | "coordinate-system" => {
            "three axes with z up are `3d-axes`, whose `x`, `y` and `z` each give a `range` and a `length`"
        }
        _ => return None,
    })
}

/// What to write for a field that an object of its type once had, or that names the wrong axis.
fn field_hint(object: &Value, field: &str) -> Option<&'static str> {
    match (object.get("type")?.as_str()?, field) {
        ("surface", "y") => Some(
            "a surface's height is `z`, an expression in `x` and `y`, as a textbook writes z = f(x, y)",
        ),
        ("surface", "z_range") => Some(
            "a surface's rectangle is `x_range` by `y_range`, and its height `z` rises along the object's own up",
        ),
        _ => None,
    }
}

fn allowed_fields(schema_path: &str) -> String {
    let subschema = schema_path
        .rsplit_once('/')
        .and_then(|(parent, _)| schema().pointer(parent));
    let fields: Vec<String> = subschema
        .and_then(|s| s["properties"].as_object())
        .map(|properties| properties.keys().map(|k| format!("`{k}`")).collect())
        .unwrap_or_default();
    fields.join(", ")
}

/// schemars writes `Option<T>` as `anyOf [T, null]`, which hides an error inside `T` behind the
/// `anyOf`, or as `"type": [T, "null"]` when `T` is inline. A scene file leaves out a field it
/// does not set rather than writing `null`, so either form becomes `T`.
fn drop_null_branches(subschema: &mut Value) {
    match subschema {
        Value::Object(map) => {
            let non_null = match map
                .get("anyOf")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
            {
                Some([branch, null]) | Some([null, branch])
                    if null == &json!({ "type": "null" }) =>
                {
                    branch.as_object().cloned()
                }
                _ => None,
            };
            if let Some(branch) = non_null {
                map.remove("anyOf");
                // The field's own description, beside the `anyOf`, says more than its type's.
                for (key, value) in branch {
                    map.entry(key).or_insert(value);
                }
            }
            if let Some(Value::Array(types)) = map.get("type")
                && types.len() == 2
                && let Some(one) = types.iter().find(|t| *t != "null")
                && types.iter().any(|t| t == "null")
            {
                let one = one.clone();
                map.insert("type".into(), one);
            }
            map.values_mut().for_each(drop_null_branches);
        }
        Value::Array(items) => items.iter_mut().for_each(drop_null_branches),
        _ => {}
    }
}

/// schemars writes an enum tagged by `type` as a `oneOf`, and a validator can only say that a
/// `oneOf` matched no branch, at the object's path. This rewrites the `oneOf` so that `type` is
/// checked once against the list of tags, and the branch the tag picks is checked with `if` and
/// `then`, which reports each error at its own field.
///
/// Fields flattened beside the enum, such as an object's transform, stay in the outer
/// `properties` and are listed as `true` in each branch, so `strict` does not call them unknown.
fn discriminate(subschema: &mut Value) {
    if let Some(map) = subschema.as_object_mut()
        && let Some(branches) = map.get("oneOf").and_then(Value::as_array)
        && let Some(tags) = branches
            .iter()
            .map(|b| b.pointer("/properties/type/const").cloned())
            .collect::<Option<Vec<_>>>()
    {
        let outer_fields: Vec<String> = map
            .get("properties")
            .and_then(Value::as_object)
            .map(|p| p.keys().cloned().collect())
            .unwrap_or_default();
        let cases: Vec<Value> = branches
            .iter()
            .zip(&tags)
            .map(|(branch, tag)| {
                let mut case = branch.clone();
                for field in &outer_fields {
                    case["properties"][field] = Value::Bool(true);
                }
                json!({ "if": { "required": ["type"], "properties": { "type": { "const": tag } } }, "then": case })
            })
            .collect();
        map.remove("oneOf");
        map.insert("type".into(), json!("object"));
        let description = "Picks the entry of `allOf` whose `if` names this value. That entry's `then` describes the value and lists the fields that go with it.";
        map.entry("properties").or_insert_with(|| json!({}))["type"] =
            json!({ "description": description, "enum": tags });
        let required = map.entry("required").or_insert_with(|| json!([]));
        required
            .as_array_mut()
            .expect("required is a list")
            .push(json!("type"));
        map.insert("allOf".into(), Value::Array(cases));
    }
    match subschema {
        Value::Object(map) => map.values_mut().for_each(discriminate),
        Value::Array(items) => items.iter_mut().for_each(discriminate),
        _ => {}
    }
}

/// Makes every object schema that lists its fields reject any other field. A `discriminate`
/// wrapper is left open because each of its branches is strict, and an `if` only tests the tag.
fn strict(subschema: &mut Value) {
    match subschema {
        Value::Object(map) => {
            if map.contains_key("properties") && !map.contains_key("allOf") {
                map.entry("additionalProperties")
                    .or_insert(Value::Bool(false));
            }
            map.iter_mut()
                .filter(|(k, _)| k.as_str() != "if")
                .for_each(|(_, child)| strict(child));
        }
        Value::Array(items) => items.iter_mut().for_each(strict),
        _ => {}
    }
}

/// Where a field sits in a scene file, as a JSON Pointer (RFC 6901). `""` is the whole file.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct JsonPath(String);

impl JsonPath {
    pub fn root() -> Self {
        Self(String::new())
    }

    pub fn field(&self, field: &str) -> Self {
        Self(format!(
            "{}/{}",
            self.0,
            field.replace('~', "~0").replace('/', "~1")
        ))
    }

    pub fn index(&self, index: usize) -> Self {
        Self(format!("{}/{index}", self.0))
    }
}

impl fmt::Display for JsonPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Stable and machine-matchable. Add codes freely; rename one only when docs/plan.md asks for it,
/// in the same change as every test and doc that names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SceneErrorCode {
    InvalidJson,
    MissingField,
    UnknownField,
    WrongType,
    OutOfRange,
    WrongLength,
    /// A string that is not in the form its field takes, such as a color that is not `#rrggbb`
    /// or a `d` that is not SVG path syntax.
    BadFormat,
    UnknownValue,
    /// A value of the right type that the scene cannot use, such as a camera that looks at its
    /// own position, or a line whose `to` is its `from`.
    Invalid,
    SceneTimeOutOfRange,
    Internal,
    /// An animation names an object id that no object has.
    UnknownObject,
    /// A create names a sphere or a molecule, which have no path to draw along.
    NotFlat,
    /// A create names a flat shape with no stroke to draw.
    NoStroke,
    /// A write names an object that is not text or math.
    NotText,
    /// A create names text or math, which a write draws.
    UseWrite,
    /// A create or a write names an object that an earlier step already creates or writes.
    CreatedTwice,
    /// A molecule's or a sound's `file` cannot be read from disk: it is missing, or not a file,
    /// or not allowed.
    UnreadableFile,
    /// A molecule's `file` was read and is not a structure file that makes a molecule: it is not
    /// SDF, or names an element or a bond that sci_anim cannot draw.
    BadStructureFile,
    /// Two animations whose spans overlap change the same value of the same object or of the
    /// camera, such as a highlight of a reaction's group in the reaction's step. Also two lit groups
    /// whose glow a reaction passes to one product group.
    ConflictingAnimations,
    /// A reaction's reactant or product names an object that is not a molecule.
    NotMolecule,
    /// A reaction lists an object twice, or on both sides.
    DuplicateObject,
    /// An atom map names an object that is not a reactant (on the reactant side) or a product
    /// (on the product side) of its reaction.
    NotInReaction,
    /// An atom map names an atom number the molecule does not have.
    NoSuchAtom,
    /// An atom group names a chain or a residue number that the molecule's PDB file does not
    /// have, or names residues of a molecule read from an SDF file, which has none.
    NoSuchResidue,
    /// An atom map gives an atom a second partner.
    AtomMappedTwice,
    /// An atom map pairs two atoms of different elements.
    ElementChanged,
    /// An atom map leaves an atom without a partner.
    UnmappedAtom,
    /// The reactants and products hold different atoms.
    AtomsNotConserved,
    /// A reactant that an earlier reaction already used up, or a product that one already made or
    /// already used up.
    ReactionOrder,
    /// A reaction's molecule does not have a single uniform scale, or is not shown at object
    /// opacity 1, at the moment the reaction needs it. Also a morph's source that is not whole.
    NotReady,
    /// A morph names an object that is not a flat shape, text, math or a plot, such as a solid, 3D
    /// axes or a pattern, or two plots on different axes, or a plot and a shape that is not one.
    NotMorphable,
    /// A morph's source that an earlier morph or reaction already used up, or is not made until
    /// later, or a target that was already made or used up, or has a create or write.
    MorphOrder,
    /// A plot's position, rotation or scale, or a move, scale, rotate or follow of a plot, or of a
    /// surface or a tube on 3D axes: the axes place it. It was `follows_axes` until follow named
    /// an animation.
    PlacedByAxes,
    /// A highlight or an attach names a group the molecule does not have, or a reaction passes a
    /// lit group's glow to a product with no group of exactly the atoms it receives.
    UnknownGroup,
    /// An attach on an object that cannot be attached: only flat shapes, text and math can, and
    /// not a plot. Also an attach to an object that is not placed by its own transform, such as
    /// a plot or another attached object, or to itself.
    NotAttachable,
    /// A position, rotation or scale, or a move, scale, rotate or follow, of an attached object:
    /// it sits beside the object it is attached to.
    Attached,
    /// A highlight of a group whose atoms are all hidden, so its glow would draw nothing.
    GroupHidden,
    /// A highlight names a cell address outside its table.
    UnknownCell,
    /// A highlight names a `cell` of an object that is not a table.
    NotTable,
    /// An animation names a copy index its pattern does not have.
    UnknownCopy,
    /// An animation names a `copy` of an object that is not a pattern.
    NotPattern,
    /// An animation names a bar index past its bar plot's bars.
    UnknownBar,
    /// An animation names a `bar` of an object that is not a bar plot.
    NotBarPlot,
    /// A recolor names a color the object does not have: a stroke or fill of text, math or a
    /// sphere, a `color` of a flat shape, a stroke or fill the shape lacks, or any color of axes,
    /// a number line, 3D axes or a molecule.
    NotRecolorable,
    /// A chapter has the id of an earlier chapter.
    DuplicateChapter,
    /// A sound's `at` names no chapter, or a follow's `from` names no chapter that has begun by
    /// the follow's step.
    UnknownChapter,
    /// A linear map names a solid, a molecule, a pattern, 3D axes, a plot, an attached object, or
    /// an object turned about x or y, which it cannot carry in the world's xy plane.
    NotMappable,
    /// A linear map with `route: turn` whose matrix has a determinant of 0 or less, which has no
    /// rotation inside it to turn by.
    NoTurningRoute,
    /// A move, scale, rotate, follow or morph of an object a linear map names, from the start of
    /// that map on, or one that is still playing when the map begins.
    MappedObject,
    /// A follow whose law, at its first scene time, lies farther than 0.001 from the value its
    /// track holds there. The message gives both values.
    LawJump,
    /// A position, rotation or scale, or a move, scale, rotate, follow, morph or linear map, of a
    /// line, an arrow or a spring placed by its ends: it lies between them.
    PlacedByEnds,
    /// A position, rotation or scale, or a move, scale, rotate or follow, of a trail: it is
    /// drawn where its object has been.
    PlacedByTrail,
    /// A count, or a follow of `value`, names an object that is not a readout.
    NotReadout,
    /// A follow with `along` whose curve starts farther than 0.001 from where the object stands
    /// when it begins, or, facing the tangent, turns it farther than that from its rotation. The
    /// message gives the position or rotation to declare.
    CarryJump,
    /// A follow's `along` names an object with no one curve to carry along: text, math, axes, a
    /// number line, 3D axes, a sphere, a prism, a revolution, a surface, a molecule or a pattern.
    NoCurve,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct SceneError {
    pub path: JsonPath,
    pub code: SceneErrorCode,
    pub message: String,
}

impl SceneError {
    pub fn new(path: JsonPath, code: SceneErrorCode, message: impl Into<String>) -> Self {
        Self {
            path,
            code,
            message: message.into(),
        }
    }
}

#[derive(Debug)]
pub enum ReadError {
    Io { file: PathBuf, source: io::Error },
    Invalid(Vec<SceneError>),
}

/// A scene file: one JSON object that describes one animation of math or science. It is data,
/// not a program. `scene_config` sets the frame size, frame rate and background. `camera` sets
/// the viewpoint. `objects` names every object by its object id and gives its shape and
/// transform: flat shapes, text, math, axes, plots, solids, molecules and patterns. `timeline` lists the
/// steps, each a wait or a play of animations that change objects or the camera. Only `objects`
/// is required.
///
/// World space has x to the right, y up and z toward the default camera, in world units. A flat
/// shape lies in its own xy plane, facing +z until it is rotated. Angles are in degrees,
/// colors are `#rrggbb`, and durations are in seconds. Every field below says its unit, default
/// and range. An unknown field is an error.
///
/// Run `sci_anim validate` to list every scene error with its JSON path, and `sci_anim inspect`
/// to see where each object lands at a scene time. The `examples` folder of the sci_anim
/// repository holds example scenes.
#[derive(Deserialize, JsonSchema)]
struct SceneFile {
    /// The frame size, frame rate and background of the scene. Leave it out for 1920 by 1080
    /// pixels at 60 frames per second on `#1e1e1e`. The background can be `transparent` for PNG frames.
    #[serde(default)]
    scene_config: SceneConfig,
    /// Where the scene is seen from at scene time 0. Leave it out for a camera at `[0, 0, 10]`
    /// looking at the origin.
    #[serde(default)]
    camera: Camera,
    /// Every object of the scene, keyed by its object id, which animations use to name it. The
    /// key order does not matter.
    objects: BTreeMap<String, ObjectFile>,
    /// The steps, played one after another from scene time 0. The scene lasts the sum of their
    /// durations. The default is no steps: a scene one frame long.
    #[serde(default)]
    timeline: Vec<StepFile>,
    /// Sound files, such as a narrator's voice clips, each played from the start of a chapter.
    /// `render` mixes them at full volume under the video and cuts them at its end, and `frame`
    /// and `inspect` ignore them. Set their loudness before, with ffmpeg or another tool. The
    /// default is none: a silent video.
    #[serde(default)]
    sounds: Vec<SoundFile>,
}

/// One sound file and the chapter it starts at.
#[derive(Deserialize, JsonSchema)]
struct SoundFile {
    /// The sound file's path, relative to the scene file, in any format ffmpeg reads, such as
    /// MP3 or WAV. sci_anim never fetches one, so it must be on disk.
    file: String,
    /// The `id` of the chapter the sound starts at.
    at: String,
}

impl SceneFile {
    /// `base` is the folder of the scene file, which structure files are relative to.
    fn into_scene(self, base: &std::path::Path) -> Result<Scene, Vec<SceneError>> {
        let mut errors = Vec::new();
        if let Some((field, message)) = self.camera.check() {
            errors.push(SceneError::new(
                JsonPath::root().field("camera").field(field),
                SceneErrorCode::Invalid,
                message,
            ));
        }
        let mut objects = BTreeMap::new();
        // A plot reads its axes, so every plot is read after every other object.
        let (plots, others): (Vec<_>, Vec<_>) = self
            .objects
            .into_iter()
            .partition(|(_, o)| o.shape.names_axes());
        let mut attaches = Vec::new();
        let mut ended = Vec::new();
        for (id, mut object) in others.into_iter().chain(plots) {
            let groups_file: BTreeMap<String, GroupAtoms> = match &mut object.shape {
                ShapeFile::Molecule { groups, .. } => std::mem::take(groups)
                    .into_iter()
                    .map(|(id, atoms)| (id, atoms.into_group_atoms()))
                    .collect(),
                _ => BTreeMap::new(),
            };
            let attach = object.attach.take();
            let ends = object.shape.take_ends();
            match object.into_object(base, &objects) {
                Ok(mut object) => {
                    if let Shape::Molecule {
                        molecule,
                        representation,
                        groups,
                        ..
                    } = &mut object.shape
                    {
                        match Groups::read(
                            &JsonPath::root().field("objects").field(&id).field("groups"),
                            &groups_file,
                            molecule,
                            *representation,
                        ) {
                            Ok(read) => *groups = read,
                            Err(found) => errors.extend(found),
                        }
                    }
                    attaches.extend(attach.map(|a| (id.clone(), a)));
                    ended.extend(ends.map(|e| (id.clone(), e)));
                    objects.insert(ObjectId::new(&id), object);
                }
                Err(Faults(faults)) => {
                    for (field, code, message) in faults {
                        errors.push(SceneError::new(
                            fields(&JsonPath::root().field("objects").field(&id), &field),
                            code,
                            message,
                        ));
                    }
                }
            }
        }
        check_trails(&mut objects, &mut errors);
        let attached: Vec<&str> = attaches
            .iter()
            .map(|(id, _): &(String, AttachFile)| id.as_str())
            .collect();
        place_by_ends(&mut objects, ended, &attached, &self.camera, &mut errors);
        attach_all(&mut objects, attaches, &mut errors);
        // The timeline resolves object ids against the start stage, so it needs every object.
        if !errors.is_empty() {
            return Err(errors);
        }
        let values = objects
            .iter()
            .filter_map(|(id, o)| {
                o.readout
                    .as_ref()
                    .map(|readout| (id.clone(), readout.value))
            })
            .collect();
        let mut start = Stage {
            camera: self.camera,
            objects,
            reactions: Vec::new(),
            morphs: BTreeMap::new(),
            values,
        };
        let mut timeline = Timeline::new(self.scene_config.fps);
        for (i, step) in self.timeline.into_iter().enumerate() {
            let path = JsonPath::root().field("timeline").index(i);
            match step {
                StepFile::Chapter {
                    id,
                    min_duration,
                    steps,
                } => {
                    if timeline.chapter_start(&id).is_some() {
                        errors.push(SceneError::new(
                            path.field("id"),
                            SceneErrorCode::DuplicateChapter,
                            format!(
                                "an earlier chapter has the id `{id}`, and each chapter has its own"
                            ),
                        ));
                    }
                    timeline.open_chapter(ChapterId::new(&id));
                    for (j, step) in steps.into_iter().enumerate() {
                        errors.extend(read_step(
                            step,
                            path.field("steps").index(j),
                            &start,
                            &mut timeline,
                        ));
                    }
                    timeline.close_chapter(min_duration);
                }
                step => errors.extend(read_step(step, path, &start, &mut timeline)),
            }
        }
        let fps = f64::from(self.scene_config.fps.get());
        let frames: Vec<f64> = crate::scene::frame_times(timeline.duration(), fps).collect();
        // A trail draws its object's world position at every frame time, worked out once here.
        let trailed: Vec<(ObjectId, ObjectId)> = start
            .objects
            .iter()
            .filter_map(|(id, o)| match &o.shape {
                Shape::Trail(trail) => Some((id.clone(), trail.object.clone())),
                _ => None,
            })
            .collect();
        let passed = timeline.positions(&start, trailed.iter().map(|(_, of)| of), &frames);
        for (id, of) in trailed {
            let since = timeline.shows_from(&of);
            let Shape::Trail(trail) = &mut start.objects.get_mut(&id).expect("listed above").shape
            else {
                unreachable!("listed above")
            };
            (trail.passed, trail.fps, trail.since) = (passed[&of].as_slice().into(), fps, since);
        }
        // A surface checked on its own points at each frame time is checked again on all of them,
        // unless a fault there was already found.
        if errors.is_empty() {
            errors.extend(timeline.fix_shape_samplings(&mut start));
        }
        let sounds = self
            .sounds
            .into_iter()
            .enumerate()
            .filter_map(|(i, sound)| {
                let at = JsonPath::root().field("sounds").index(i);
                let path = base.join(&sound.file);
                let chapter = timeline.chapters().iter().find(|chapter| *chapter.id == *sound.at);
                if !path.is_file() {
                    let looked = std::path::absolute(&path).unwrap_or_else(|_| path.clone());
                    let message = format!("`{}` cannot be read; a sound file's path is relative to the scene file, so sci_anim looked for {}", sound.file, looked.display());
                    errors.push(SceneError::new(at.field("file"), SceneErrorCode::UnreadableFile, message));
                }
                if chapter.is_none() {
                    let ids: Vec<String> = timeline.chapters().iter().map(|chapter| format!("`{}`", &*chapter.id)).collect();
                    let known = if ids.is_empty() { "the timeline has no chapters; put the steps the sound goes with in a `chapter`".to_string() } else { format!("the chapters are {}", ids.join(", ")) };
                    errors.push(SceneError::new(at.field("at"), SceneErrorCode::UnknownChapter, format!("no chapter has the id `{}`; {known}", sound.at)));
                }
                let chapter = chapter?;
                Some(Sound { file: sound.file, path, at: chapter.id.clone(), start: chapter.start })
            })
            .collect();
        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(Scene {
            scene_config: self.scene_config,
            start,
            timeline,
            sounds,
        })
    }
}

/// Adds a wait or a play step read from `path` to the timeline, and returns every scene error it
/// has. A chapter holds only these, so a chapter inside a chapter is an error here.
fn read_step(
    step: StepFile,
    path: JsonPath,
    start: &Stage,
    timeline: &mut Timeline,
) -> Vec<SceneError> {
    match step {
        StepFile::Wait { duration } => {
            timeline.wait(duration, path);
            Vec::new()
        }
        StepFile::Play {
            duration,
            animations,
        } => {
            let mut errors = Vec::new();
            let animations: Vec<(JsonPath, Animation)> = animations
                .into_iter()
                .enumerate()
                .filter_map(|(j, animation)| {
                    let at = path.field("animations").index(j);
                    match animation.into_animation(start, &at, duration) {
                        Ok(animation) => Some((at, animation)),
                        Err(found) => {
                            errors.extend(found);
                            None
                        }
                    }
                })
                .collect();
            errors.extend(timeline.play(duration, path, &animations, start));
            errors
        }
        StepFile::Chapter { .. } => vec![SceneError::new(
            path.field("type"),
            SceneErrorCode::Invalid,
            "a chapter holds wait and play steps, and not another chapter",
        )],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::{Color, HEX_COLOR_PATTERN};

    #[test]
    fn the_color_pattern_and_the_color_parser_agree() {
        let pattern = jsonschema::validator_for(&json!({ "pattern": HEX_COLOR_PATTERN })).unwrap();
        for text in [
            "#58c4dd", "#FFFFFF", "#000000", "58c4dd", "#58c4d", "#58c4ddd", "#58c4dg", "#+1+1+1",
            "#ééé", "",
        ] {
            let parsed = Color::try_from(text.to_string()).is_ok();
            assert_eq!(parsed, pattern.is_valid(&json!(text)), "{text:?}");
        }
        assert_eq!(
            Color::try_from("#58c4dd".to_string()),
            Ok(Color {
                r: 0x58,
                g: 0xc4,
                b: 0xdd
            })
        );
    }

    #[test]
    fn the_svg_path_pattern_admits_every_path_kurbo_reads() {
        let pattern = jsonschema::validator_for(&json!({ "pattern": SVG_PATH_PATTERN })).unwrap();
        for d in [
            "M 0 0 L 1 0",
            "M-1,-1L1,-1 1,1Z",
            "M 0 0 C 0 1 1 1 1 0 Q 2 -1 3 0 T 5 0 A 1 1 0 0 1 7 0 z",
            "m1e-3 .5h2v-1",
            "",
        ] {
            assert!(pattern.is_valid(&json!(d)), "{d:?}");
            assert!(
                !matches!(Path::from_svg(d), Err(PathError::Syntax(_))),
                "{d:?}"
            );
        }
        assert!(!pattern.is_valid(&json!("M 0 0 L 1 0 # a comment")));
    }
}
