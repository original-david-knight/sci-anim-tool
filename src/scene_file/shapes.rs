//! How an object's shape is read from its file: a path, a solid, axes, a plot, a pattern, and
//! a shape law that a follow redraws at other law times.

use super::*;

impl ObjectFile {
    /// The `Object`, with its shape as a path or a sphere, or each field at fault with its scene
    /// error code and message. `placed` holds the objects read so far, which a plot reads its axes
    /// from: `into_scene` reads every plot after every other object.
    pub(super) fn into_object(
        self,
        base: &std::path::Path,
        placed: &BTreeMap<ObjectId, Object>,
    ) -> Result<Object, Faults> {
        let shape_law =
            reads_law_time(&self.shape).then(|| ShapeLaw::new(self.shape.clone(), base, placed));
        let (shape, placed_by, readout) = match self.shape {
            ShapeFile::Readout {
                value,
                decimals,
                unit,
                size,
                color,
            } => {
                let readout = Readout::new(value.get(), decimals.get(), unit, size, color)
                    .map_err(typeset_error("unit"))?;
                (
                    Shape::Glyphs {
                        glyphs: readout.glyphs(value.get()),
                        size,
                    },
                    PlacedBy::Itself,
                    Some(Arc::new(readout)),
                )
            }
            file => {
                let (shape, placed_by) = shape(file, base, placed, ShapeAt::START)?;
                (shape, placed_by, None)
            }
        };
        if matches!(placed_by, PlacedBy::Axes(_)) && self.transform != Transform::default() {
            let message = format!(
                "{} is placed by its axes and has no position, rotation or scale of its own; place the axes instead",
                shape.what_axes_place()
            );
            return Err(("position", SceneErrorCode::PlacedByAxes, message).into());
        }
        Ok(Object {
            transform: self.transform,
            opacity: self.opacity,
            created: UnitInterval::one(),
            placed_by,
            shape,
            plane_map: None,
            shape_law: shape_law.map(Arc::new),
            readout,
        })
    }
}

/// The law time a shape is read at, and the points a surface whose height reads it is sampled at.
#[derive(Clone, Copy)]
pub(super) struct ShapeAt<'a> {
    pub(super) law_time: f64,
    pub(super) sampling: Option<&'a Sampling>,
}

impl ShapeAt<'_> {
    /// How the scene file declares a shape: at `t` of 0, before any follow runs it.
    const START: ShapeAt<'static> = ShapeAt {
        law_time: 0.0,
        sampling: None,
    };
}

/// A shape whose expressions read `t`, law time: everything needed to read it again at another
/// law time, so a follow with `sets: shape` can redraw it at every scene time. The expressions are
/// read once more at each law time, which `Timeline::play` checks at every frame time of the
/// follow's span.
pub struct ShapeLaw {
    pub(super) file: ShapeFile,
    pub(super) base: PathBuf,
    /// The axes and 3D axes of the scene, as declared, which a plot, a surface or a tube reads.
    pub(super) axes: BTreeMap<ObjectId, Object>,
    /// The points a surface is sampled at at every scene time, set by
    /// `Timeline::fix_shape_samplings` once the timeline is read.
    pub sampling: Option<Sampling>,
}

impl fmt::Debug for ShapeLaw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ShapeLaw")
    }
}

/// A shape's fault at a law time: its field under the object, its code and its message.
pub type ShapeFault = (String, SceneErrorCode, String);

impl ShapeLaw {
    pub(super) fn new(
        file: ShapeFile,
        base: &std::path::Path,
        placed: &BTreeMap<ObjectId, Object>,
    ) -> ShapeLaw {
        let axes = placed
            .iter()
            .filter(|(_, o)| matches!(o.shape, Shape::Axes(_) | Shape::Axes3d(_)))
            .map(|(id, o)| (id.clone(), o.clone()))
            .collect();
        ShapeLaw {
            file,
            base: base.to_path_buf(),
            axes,
            sampling: None,
        }
    }

    /// The shape at `law_time`, or each fault its expressions have there.
    pub fn at(&self, law_time: f64) -> Result<Shape, Vec<ShapeFault>> {
        shape(
            self.file.clone(),
            &self.base,
            &self.axes,
            ShapeAt {
                law_time,
                sampling: self.sampling.as_ref(),
            },
        )
        .map(|(shape, _)| shape)
        .map_err(|Faults(faults)| faults)
    }

    /// The sampling of the surface a shape draws, itself or as a pattern's copies.
    pub fn sampling_of(shape: &Shape) -> Option<&Sampling> {
        match shape {
            Shape::Solid {
                solid: Solid::Surface { surface },
                ..
            } => Some(surface.sampling()),
            Shape::Pattern(pattern) => ShapeLaw::sampling_of(&pattern.shape),
            _ => None,
        }
    }
}

/// Whether a shape's expressions read `t`, law time: a function plot's, a space curve's, a
/// surface's, or a pattern's, its shape's included.
fn reads_law_time(file: &ShapeFile) -> bool {
    let reads = |source: &str, variables| {
        Expression::parse(source, variables).is_ok_and(|expression| expression.reads_law_time())
    };
    let curve = |curve: &SpaceCurveFile| match curve {
        SpaceCurveFile::Expressions { x, y, z, .. } => [x, y, z]
            .into_iter()
            .any(|source| reads(source, expr::SPACE_CURVE)),
        SpaceCurveFile::Points { .. } | SpaceCurveFile::Trace { .. } => false,
    };
    match file {
        ShapeFile::FunctionPlot { expression, .. } => reads(expression, expr::FUNCTION_PLOT),
        ShapeFile::Tube { curve: written, .. } => curve(written),
        ShapeFile::Surface { z, .. } => reads(z, expr::SURFACE),
        ShapeFile::Pattern {
            shape,
            layout,
            copy,
        } => {
            let variables = match layout {
                LayoutFile::Grid { .. } => expr::GRID_COPY,
                LayoutFile::Ring { .. } | LayoutFile::Along { .. } => expr::COPY,
            };
            let copy_reads = copy.as_ref().is_some_and(|copy| {
                let [px, py, pz] = copy.position.as_ref().map_or([None, None, None], |c| {
                    [c.x.as_ref(), c.y.as_ref(), c.z.as_ref()]
                });
                let [rx, ry, rz] = copy.rotation.as_ref().map_or([None, None, None], |c| {
                    [c.x.as_ref(), c.y.as_ref(), c.z.as_ref()]
                });
                let [sx, sy, sz] = copy.scale.as_ref().map_or([None, None, None], |c| {
                    [c.x.as_ref(), c.y.as_ref(), c.z.as_ref()]
                });
                [
                    px,
                    py,
                    pz,
                    rx,
                    ry,
                    rz,
                    sx,
                    sy,
                    sz,
                    copy.color_share.as_ref(),
                ]
                .into_iter()
                .flatten()
                .any(|source| reads(source, variables))
            });
            reads_law_time(shape)
                || matches!(layout, LayoutFile::Along { curve: written, .. } if curve(written))
                || copy_reads
        }
        _ => false,
    }
}

/// The shape a `ShapeFile` writes at a law time, with what places it: a plot is placed by its
/// axes, and every other shape by its object's own transform.
pub(super) fn shape(
    file: ShapeFile,
    base: &std::path::Path,
    placed: &BTreeMap<ObjectId, Object>,
    at_time: ShapeAt,
) -> Result<(Shape, PlacedBy), Faults> {
    let mut placed_by = PlacedBy::Itself;
    let shape = match file {
        ShapeFile::Sphere {
            radius,
            material,
            axes,
        } => {
            if let Some(axes) = axes {
                placed_by = PlacedBy::DataPoint(find_data_point_axes(&axes, placed, false)?);
            }
            Shape::Sphere { radius, material }
        }
        ShapeFile::Prism {
            base,
            height,
            material,
            edges,
        } => {
            let base = region(base, ["base/radius", "base/points", "base/d"])?;
            Shape::Solid {
                solid: Solid::Prism { base, height },
                material: opaque(material)?,
                edges,
            }
        }
        ShapeFile::Revolution {
            profile,
            angle,
            material,
            edges,
        } => {
            let profile = region(profile, ["profile/radius", "profile/points", "profile/d"])?;
            let leftmost = profile.path.bounding_box().x0;
            if leftmost < -1e-6 * f64::from(profile.path.size()) {
                let message = format!(
                    "the profile reaches x = {leftmost:.4}, and a profile lies at x of 0 or more, on one side of the axis it turns around; move it right of the y axis"
                );
                return Err(("profile", SceneErrorCode::OutOfRange, message).into());
            }
            Shape::Solid {
                solid: Solid::Revolution { profile, angle },
                material: opaque(material)?,
                edges,
            }
        }
        ShapeFile::Tube {
            curve,
            axes,
            radius,
            material,
            edges,
        } => {
            let on = axes.map(|axes| find_axes_3d(&axes, placed)).transpose()?;
            let curve = space_curve(
                curve,
                base,
                on.as_ref().map(|(_, axes)| axes.data_to_own()),
                at_time.law_time,
            )?;
            let cut = on.as_ref().map(|(_, axes)| axes.own_box());
            if let Some((axes_id, _)) = on {
                placed_by = PlacedBy::Axes(axes_id);
            }
            Shape::Solid {
                solid: Solid::Tube {
                    curve: Arc::new(curve),
                    radius,
                    cut,
                },
                material: opaque(material)?,
                edges,
            }
        }
        ShapeFile::Surface {
            z,
            x_range,
            y_range,
            axes,
            material,
            edges,
        } => {
            let on = axes.map(|axes| find_axes_3d(&axes, placed)).transpose()?;
            let surface = read_surface(
                &z,
                x_range,
                y_range,
                on.as_ref().map(|(_, axes)| *axes),
                at_time,
            )?;
            if let Some((axes_id, _)) = on {
                placed_by = PlacedBy::Axes(axes_id);
            }
            Shape::Solid {
                solid: Solid::Surface {
                    surface: Arc::new(surface),
                },
                material: opaque(material)?,
                edges,
            }
        }
        ShapeFile::Dot { radius, fill, axes } => {
            if let Some(axes) = axes {
                placed_by = PlacedBy::DataPoint(find_data_point_axes(&axes, placed, true)?);
            }
            Shape::Flat {
                path: Path::dot(radius.get())
                    .map_err(at("radius", "the radius is too small to draw"))?,
                stroke: None,
                fill: Some(fill),
            }
        }
        ShapeFile::Circle {
            radius,
            stroke,
            fill,
        } => Shape::Flat {
            path: Path::circle(radius.get())
                .map_err(at("radius", "the radius is too small to draw"))?,
            stroke,
            fill,
        },
        ShapeFile::Line { from, to, stroke } => {
            let (from, to) = (
                from.in_plane().unwrap_or(Vec2::ZERO),
                to.in_plane().unwrap_or(Vec2::X),
            );
            let path = Path::line(from, to).map_err(at(
                if from.is_finite() { "to" } else { "from" },
                "the line's `to` is its `from`, so it has no length",
            ))?;
            Shape::Flat {
                path,
                stroke: Some(stroke),
                fill: None,
            }
        }
        ShapeFile::Arrow {
            from,
            to,
            tip_length,
            stroke,
        } => {
            let (from, to) = (
                from.in_plane().unwrap_or(Vec2::ZERO),
                to.in_plane().unwrap_or(Vec2::X),
            );
            let path = Path::arrow(from, to, tip_length.get()).map_err(at(
                if from.is_finite() { "to" } else { "from" },
                "the arrow's `to` is its `from`, so it points nowhere",
            ))?;
            Shape::Flat {
                path,
                stroke: Some(stroke),
                fill: Some(Fill::solid(stroke.color)),
            }
        }
        ShapeFile::Spring {
            turns,
            radius,
            tube_radius,
            material,
            edges,
            ..
        } => {
            let curve = crate::ends::helix(1.0, turns.get(), radius.get());
            Shape::Solid {
                solid: Solid::Tube {
                    curve: Arc::new(curve),
                    radius: tube_radius,
                    cut: None,
                },
                material: opaque(material)?,
                edges,
            }
        }
        ShapeFile::Trail {
            object,
            length,
            stroke,
        } => Shape::Trail(Trail {
            object: ObjectId::new(&object),
            length: length.map(Duration::get),
            stroke: undashed(stroke, "a trail's")?,
            passed: Arc::from([]),
            fps: 1.0,
            since: 0.0,
            points: Vec::new(),
        }),
        ShapeFile::Readout { .. } => {
            unreachable!("`into_object` reads a readout, and a pattern draws none")
        }
        ShapeFile::Polygon {
            points,
            stroke,
            fill,
        } => Shape::Flat {
            path: Path::polygon(&points).map_err(at(
                "points",
                "every point of the polygon is the same, so it has no length",
            ))?,
            stroke,
            fill,
        },
        ShapeFile::Arc {
            radius,
            start_angle,
            sweep,
            stroke,
            fill,
        } => {
            let field = if sweep.get() == 0.0 {
                "sweep"
            } else {
                "radius"
            };
            Shape::Flat {
                path: Path::arc(radius.get(), start_angle, sweep.get()).map_err(at(
                    field,
                    "a `sweep` of 0 has no length, so there is nothing to draw",
                ))?,
                stroke,
                fill,
            }
        }
        ShapeFile::Path { d, stroke, fill } => Shape::Flat {
            path: Path::from_svg(&d)
                .map_err(at("d", "`d` has no length, so there is nothing to draw"))?,
            stroke,
            fill,
        },
        ShapeFile::Text {
            text,
            font,
            size,
            color,
            align,
        } => {
            let glyphs = typeset::text(
                &text,
                font,
                color,
                Layout {
                    size: size.get(),
                    align,
                },
            )
            .map_err(typeset_error("text"))?;
            Shape::Glyphs { glyphs, size }
        }
        ShapeFile::Math {
            tex,
            size,
            color,
            align,
        } => {
            let glyphs = typeset::math(
                &tex,
                color,
                Layout {
                    size: size.get(),
                    align,
                },
            )
            .map_err(typeset_error("tex"))?;
            Shape::Glyphs { glyphs, size }
        }
        ShapeFile::Table {
            rows,
            size,
            color,
            font,
            align,
            padding,
            rulings,
            stroke,
        } => Shape::Table(table(
            rows, size, color, font, align, padding, rulings, stroke,
        )?),
        ShapeFile::Axes {
            x,
            y,
            width,
            height,
            stroke,
            color,
            label_size,
        } => {
            let appearance = Appearance {
                stroke,
                color,
                label_size: label_size.get(),
            };
            let y_axis = axis("y", &y, height)?;
            let axes = match &x {
                XAxisFile::Numbers(file) => Axes::new(
                    axis("x", file, width)?,
                    y_axis,
                    x.label(),
                    y.label.as_deref(),
                    appearance,
                ),
                XAxisFile::Categories(file) => Axes::with_categories(
                    file.categories.clone(),
                    width.get(),
                    y_axis,
                    x.label(),
                    y.label.as_deref(),
                    appearance,
                ),
            };
            let axes = axes.map_err(label_error)?;
            if let Some((left, right)) = axes.crowded_categories(label_size.get()) {
                let message = format!(
                    "the labels `{left}` and `{right}` run into each other, each category being {:.2} wide; make the axes wider, the `label_size` smaller or the categories shorter",
                    width.get() / axes.categories.as_ref().map_or(1, Vec::len) as f32
                );
                return Err(("x/categories", SceneErrorCode::Invalid, message).into());
            }
            Shape::Axes(axes)
        }
        ShapeFile::Axes3d {
            x,
            y,
            z,
            stroke,
            color,
            label_size,
        } => {
            let appearance = Appearance {
                stroke: undashed(stroke, "3D axes'")?,
                color,
                label_size: label_size.get(),
            };
            let mut axes = Vec::new();
            for (which, file) in [("x", &x), ("y", &y), ("z", &z)] {
                let one = AxisFile {
                    range: file.range,
                    spacing: file.spacing,
                    label: None,
                };
                axes.push(axis(which, &one, file.length)?);
            }
            let axes: [Axis; 3] = axes
                .try_into()
                .unwrap_or_else(|_| unreachable!("one axis each for x, y and z"));
            Shape::Axes3d(
                Axes3d::new(
                    axes,
                    [x.label.as_deref(), y.label.as_deref(), z.label.as_deref()],
                    appearance,
                )
                .map_err(label_error)?,
            )
        }
        ShapeFile::NumberLine {
            range,
            spacing,
            length,
            label,
            stroke,
            color,
            label_size,
        } => {
            let appearance = Appearance {
                stroke,
                color,
                label_size: label_size.get(),
            };
            let one = AxisFile {
                range,
                spacing,
                label: None,
            };
            let line = axis("", &one, length)?;
            Shape::NumberLine(
                NumberLine::new(line, label.as_deref(), appearance).map_err(label_error)?,
            )
        }
        ShapeFile::FunctionPlot {
            axes,
            expression,
            x_range,
            stroke,
        } => {
            let (axes_id, axes) = find_axes(&axes, placed)?;
            let expression = Expression::parse(&expression, expr::FUNCTION_PLOT)
                .map_err(expression_errors("expression", &expression))?
                .at_law_time(at_time.law_time);
            let range = x_range.unwrap_or_else(|| axes.x.range().into());
            if range[0] >= range[1] {
                return Err((
                    "x_range",
                    SceneErrorCode::Invalid,
                    "`x_range` must give its lowest value first".to_string(),
                )
                    .into());
            }
            let path = plot::function_path(axes, &expression, (range[0], range[1]))
                .map_err(plot_error("expression"))?;
            placed_by = PlacedBy::Axes(axes_id);
            Shape::Flat {
                path,
                stroke: Some(stroke),
                fill: None,
            }
        }
        ShapeFile::PointPlot {
            axes,
            points,
            stroke,
        } => {
            let (axes_id, axes) = find_axes(&axes, placed)?;
            let points: Vec<(f64, f64)> = points.into_iter().map(|[x, y]| (x, y)).collect();
            let path = plot::points_path(axes, &points).map_err(plot_error("points"))?;
            placed_by = PlacedBy::Axes(axes_id);
            Shape::Flat {
                path,
                stroke: Some(stroke),
                fill: None,
            }
        }
        ShapeFile::BarPlot {
            axes,
            values,
            fill,
            stroke,
        } => {
            let (axes_id, found) = find_axes(&axes, placed)?;
            let bar_plot =
                BarPlot::new(found, &values, fill, stroke).map_err(|error| match error {
                    BarPlotError::NoCategories => (
                        "axes",
                        SceneErrorCode::Invalid,
                        format!(
                            "`{axes}` shows numbers along x, and a bar plot stands on a category axis; give `{axes}` an `x` of `{{ \"categories\": [...] }}`, one label for each bar"
                        ),
                    ),
                    BarPlotError::ValueCount { values, categories } => (
                        "values",
                        SceneErrorCode::WrongLength,
                        format!(
                            "`values` gives {values} values and `{axes}` has {categories} categories; give one value for each category, in the order of its `categories`"
                        ),
                    ),
                })?;
            placed_by = PlacedBy::Axes(axes_id);
            Shape::BarPlot(bar_plot)
        }
        ShapeFile::Molecule {
            file,
            representation,
            hydrogens,
            ..
        } => {
            let molecule = structure_file::read(&base.join(&file))
                .map_err(structure_file_error(&file, base))?;
            let representation = match representation {
                RepresentationFile::BallAndStick => Representation::BallAndStick { hydrogens },
                RepresentationFile::SpaceFilling => Representation::SpaceFilling { hydrogens },
                RepresentationFile::Cartoon if molecule.residues().is_empty() => {
                    return Err((
                        "representation",
                        SceneErrorCode::Invalid,
                        format!(
                            "`{file}` is read as an SDF file, which has no chains or secondary structure to draw as a cartoon; a cartoon draws a protein, DNA or RNA read from a PDB file, whose name ends in `.pdb` or `.ent`, so draw this molecule as `ball-and-stick`"
                        ),
                    )
                        .into());
                }
                RepresentationFile::Cartoon if molecule.cartoon().is_empty() => {
                    return Err((
                        "representation",
                        SceneErrorCode::Invalid,
                        format!(
                            "no chain of `{file}` has two amino acids with alpha carbons `CA`, or two nucleotides with phosphorus atoms `P`, in a row, so a cartoon would draw nothing; draw this molecule as `ball-and-stick`"
                        ),
                    )
                        .into());
                }
                RepresentationFile::Cartoon => Representation::Cartoon,
            };
            Shape::Molecule {
                molecule: Arc::new(molecule),
                representation,
                groups: Groups::default(),
                glows: BTreeMap::new(),
            }
        }
        ShapeFile::Pattern {
            shape: one,
            layout,
            copy,
        } => Shape::Pattern(read_pattern(
            *one,
            layout,
            copy.map(|copy| *copy).unwrap_or_default(),
            base,
            placed,
            at_time,
        )?),
    };
    Ok((shape, placed_by))
}

/// The pattern a `pattern` object writes, or every fault in it at once: its shape's under
/// `shape`, its curve's under `layout`, and each expression of `copy` that cannot be read; then
/// each expression with no finite value at some copy, at its first such copy. A shape a pattern
/// does not draw is refused before it is read, so a molecule's file is never opened.
fn read_pattern(
    one: ShapeFile,
    layout: LayoutFile,
    copy: CopyFile,
    base: &std::path::Path,
    placed: &BTreeMap<ObjectId, Object>,
    at_time: ShapeAt,
) -> Result<Pattern, Faults> {
    let variables = match layout {
        LayoutFile::Grid { .. } => expr::GRID_COPY,
        LayoutFile::Ring { .. } | LayoutFile::Along { .. } => expr::COPY,
    };
    let one = match not_one_shape(&one) {
        None if one.names_axes() => Err(("shape/axes", SceneErrorCode::Invalid, "a pattern places every copy itself, so its shape cannot name `axes`; write a separate object for each shape on the axes".to_string()).into()),
        Some(why) => {
            let message = format!("a pattern draws one `circle`, `dot`, `line`, `arrow`, `polygon`, `arc`, `path`, `sphere`, `prism`, `revolution`, `tube` or `surface` at every copy, and {why}");
            Err(("shape/type", SceneErrorCode::Invalid, message).into())
        }
        None => shape(one, base, placed, at_time).map_err(|faults| faults.under("shape")),
    };
    let layout =
        read_layout(layout, base, at_time.law_time).map_err(|faults| faults.under("layout"));
    let copy = copy_expressions(copy, variables, at_time.law_time);
    let ((one, _), layout, (expressions, to_color)) = match (one, layout, copy) {
        (Ok(one), Ok(layout), Ok(copy)) => (one, layout, copy),
        (one, layout, copy) => {
            return Err(Faults(
                [one.err(), layout.err(), copy.err()]
                    .into_iter()
                    .flatten()
                    .flat_map(|faults| faults.0)
                    .collect(),
            ));
        }
    };
    let palette_length = match &to_color {
        ToColor::One(_) => 0,
        ToColor::Palette { palette, index } => {
            if let PaletteIndex::Listed(listed) = index
                && listed.len() != layout.copy_count()
            {
                let message = format!(
                    "`palette_index` lists {} indices and the layout has {} copies, and it needs one index a copy, in index order",
                    listed.len(),
                    layout.copy_count()
                );
                return Err((
                    pattern::PALETTE_INDEX_FIELD,
                    SceneErrorCode::WrongLength,
                    message,
                )
                    .into());
            }
            palette.len()
        }
    };
    Pattern::new(one, &layout, &expressions, &to_color).map_err(|found| {
        let faults: Vec<Fault> = found
            .into_iter()
            .map(|n| {
                let [x, y, z] = n.place.to_array();
                let needs = if n.field == pattern::PALETTE_INDEX_FIELD {
                    format!(
                        "a whole number from 0 to {}, one of the palette's {palette_length} colors; change the expression or the palette",
                        palette_length - 1
                    )
                } else {
                    "a finite value; change the expression or the layout".to_string()
                };
                let message = format!(
                    "`{}` is {} at copy {}, whose place is ({x}, {y}, {z}), and every copy needs {needs}",
                    n.field.strip_prefix("copy/").unwrap_or(n.field),
                    n.value,
                    n.index
                );
                (n.field, SceneErrorCode::OutOfRange, message)
            })
            .collect();
        faults.into()
    })
}

impl ShapeFile {
    /// Whether the shape is placed by axes it names, so it is read after every other object: a
    /// plot, a surface or tube on 3D axes, or a data point.
    pub(super) fn names_axes(&self) -> bool {
        matches!(
            self,
            ShapeFile::FunctionPlot { .. }
                | ShapeFile::PointPlot { .. }
                | ShapeFile::BarPlot { .. }
                | ShapeFile::Surface { axes: Some(_), .. }
                | ShapeFile::Tube { axes: Some(_), .. }
                | ShapeFile::Sphere { axes: Some(_), .. }
                | ShapeFile::Dot { axes: Some(_), .. }
        )
    }
}

/// Why a pattern does not draw `shape` at every copy, or `None` for a shape it draws.
fn not_one_shape(shape: &ShapeFile) -> Option<&'static str> {
    Some(match shape {
        ShapeFile::Text { .. } => {
            "`text` would set one string at every copy; write a separate `text` object for each label"
        }
        ShapeFile::Math { .. } => {
            "`math` would set one equation at every copy; write a separate `math` object for each"
        }
        ShapeFile::Table { .. } => {
            "a `table` is many cells and rulings, not one shape; write a separate `table` object for each"
        }
        ShapeFile::Molecule { .. } => {
            "a `molecule` is many atoms, not one shape; draw atoms as a pattern of a `sphere`, or write a separate `molecule` object for each"
        }
        ShapeFile::Axes { .. } | ShapeFile::NumberLine { .. } | ShapeFile::Axes3d { .. } => {
            "axes and a number line are many lines and labels, not one shape; write a separate object for each"
        }
        ShapeFile::FunctionPlot { .. } | ShapeFile::PointPlot { .. } => {
            "a plot is placed by its axes, not one shape a copy can place; write a separate plot for each curve"
        }
        ShapeFile::BarPlot { .. } => {
            "a bar plot is placed by its axes, not one shape a copy can place; write one `bar-plot` with a value for each category"
        }
        ShapeFile::Pattern { .. } => {
            "a pattern does not nest; give one pattern a `grid` with more `counts`, or write a second pattern"
        }
        ShapeFile::Spring { .. } => {
            "a spring lies between its two ends, which a copy cannot place; write a separate `spring` for each"
        }
        ShapeFile::Trail { .. } => {
            "a trail is drawn where its object has been, which a copy cannot place; write a separate `trail` for each"
        }
        ShapeFile::Readout { .. } => {
            "a `readout` sets one number at every copy; write a separate `readout` for each"
        }
        ShapeFile::Line { from, to, .. } | ShapeFile::Arrow { from, to, .. }
            if from.in_plane().is_none() || to.in_plane().is_none() =>
        {
            "a line or an arrow with an end on an object, or at a point `[x, y, z]`, lies between its ends, which a copy cannot place; give the copies' shape ends `[x, y]` in its plane"
        }
        ShapeFile::Circle { .. }
        | ShapeFile::Dot { .. }
        | ShapeFile::Line { .. }
        | ShapeFile::Arrow { .. }
        | ShapeFile::Polygon { .. }
        | ShapeFile::Arc { .. }
        | ShapeFile::Path { .. }
        | ShapeFile::Sphere { .. }
        | ShapeFile::Prism { .. }
        | ShapeFile::Revolution { .. }
        | ShapeFile::Tube { .. }
        | ShapeFile::Surface { .. } => return None,
    })
}

/// The layout a pattern writes, with a grid of more than `pattern::MAX_COPIES` copies at its
/// `counts` and a curve's faults under `curve`.
fn read_layout(
    file: LayoutFile,
    base: &std::path::Path,
    law_time: f64,
) -> Result<pattern::Layout, Faults> {
    Ok(match file {
        LayoutFile::Grid { counts, spacing } => {
            let counts = counts.map(CopyCount::get);
            let total: u64 = counts.iter().map(|&n| u64::from(n)).product();
            if total > pattern::MAX_COPIES {
                let [nx, ny, nz] = counts;
                let message = format!(
                    "a grid of {nx} by {ny} by {nz} is {total} copies, and a pattern holds at most {}; use smaller `counts`",
                    pattern::MAX_COPIES
                );
                return Err(("counts", SceneErrorCode::OutOfRange, message).into());
            }
            pattern::Layout::Grid {
                counts,
                spacing: spacing.get(),
            }
        }
        LayoutFile::Ring {
            count,
            radius,
            start_angle,
        } => pattern::Layout::Ring {
            count: count.get(),
            radius: radius.get(),
            start_angle,
        },
        LayoutFile::Along {
            count,
            curve,
            faces,
        } => pattern::Layout::Along {
            count: count.get(),
            curve: space_curve(curve, base, None, law_time)?,
            faces,
        },
    })
}

/// The expressions `copy` writes, read under `variables`, with what its color share mixes toward,
/// or a fault at each mistake in each expression, at a `color_share` with no color to mix toward,
/// a `to_color` with no `color_share`, a `palette` with `to_color` or without `palette_index`, a
/// `palette_index` without `palette`, and each listed palette index past the palette's end.
fn copy_expressions(
    copy: CopyFile,
    variables: expr::Variables,
    law_time: f64,
) -> Result<(CopyExpressions, ToColor), Faults> {
    let mut faults: Vec<Fault> = Vec::new();
    let mut listed_faults: Vec<(String, SceneErrorCode, String)> = Vec::new();
    match (&copy.color_share, copy.to_color, &copy.palette) {
        (_, Some(_), Some(_)) => faults.push(("copy/palette", SceneErrorCode::Invalid, "each copy's colors are mixed toward `to_color` or toward its pick of `palette`, not both; leave one out".to_string())),
        (Some(_), None, None) => faults.push(("copy/to_color", SceneErrorCode::MissingField, "`color_share` mixes each copy's colors toward `to_color`, or toward its pick of a `palette`, so write one of them too".to_string())),
        (None, Some(_), None) => faults.push(("copy/color_share", SceneErrorCode::MissingField, "`to_color` is what each copy's colors are mixed toward by its `color_share`, so write `color_share` too, such as `\"i/9\"`".to_string())),
        _ => {}
    }
    let palette_index = match (&copy.palette, copy.palette_index) {
        (Some(_), None) => {
            faults.push(("copy/palette_index", SceneErrorCode::MissingField, "`palette` is the colors each copy picks one of by its `palette_index`, so write `palette_index` too, such as `\"mod(i, 4)\"` or `[0, 1, 2, 3]`".to_string()));
            None
        }
        (None, Some(_)) => {
            faults.push(("copy/palette", SceneErrorCode::MissingField, "`palette_index` picks one of the colors of `palette` for each copy, so write `palette` too, such as `[\"#fc6255\", \"#58c4dd\"]`".to_string()));
            None
        }
        (Some(palette), Some(PaletteIndexFile::Listed(listed))) => {
            for (entry, &index) in listed.iter().enumerate() {
                if index as usize >= palette.len() {
                    listed_faults.push((
                        format!("{}/{entry}", pattern::PALETTE_INDEX_FIELD),
                        SceneErrorCode::OutOfRange,
                        format!(
                            "palette index {index} picks no color: the palette has {} colors, so an index runs from 0 to {}",
                            palette.len(),
                            palette.len() - 1
                        ),
                    ));
                }
            }
            Some(PaletteIndex::Listed(listed))
        }
        (Some(_), Some(PaletteIndexFile::Expression(source))) => {
            Expression::parse(&source, variables)
                .map(|expression| PaletteIndex::Expression(expression.at_law_time(law_time)))
                .map_err(|errors| {
                    faults.extend(expression_errors(pattern::PALETTE_INDEX_FIELD, &source)(
                        errors,
                    ))
                })
                .ok()
        }
        (None, None) => None,
    };
    let [px, py, pz] = copy
        .position
        .map_or([None, None, None], |c| [c.x, c.y, c.z]);
    let [rx, ry, rz] = copy
        .rotation
        .map_or([None, None, None], |c| [c.x, c.y, c.z]);
    let [sx, sy, sz] = copy.scale.map_or([None, None, None], |c| [c.x, c.y, c.z]);
    let sources = [px, py, pz, rx, ry, rz, sx, sy, sz, copy.color_share];
    let expressions: Vec<Option<Expression>> = pattern::COPY_FIELDS
        .into_iter()
        .zip(sources)
        .map(|(field, source)| {
            let source = source?;
            Expression::parse(&source, variables)
                .map(|expression| expression.at_law_time(law_time))
                .map_err(|errors| faults.extend(expression_errors(field, &source)(errors)))
                .ok()
        })
        .collect();
    if !faults.is_empty() || !listed_faults.is_empty() {
        let mut all = Faults::from(faults);
        all.0.extend(listed_faults);
        return Err(all);
    }
    let expressions = CopyExpressions(
        expressions
            .try_into()
            .unwrap_or_else(|_| unreachable!("one source for each field")),
    );
    let to_color = match (copy.palette, palette_index) {
        (Some(palette), Some(index)) => ToColor::Palette { palette, index },
        _ => ToColor::One(copy.to_color.unwrap_or_else(ShapeFile::default_color)),
    };
    Ok((expressions, to_color))
}

/// The field at fault, several fixed names joined by `/`, with its scene error code and message.
pub(super) type Fault = (&'static str, SceneErrorCode, String);

/// Every fault of one object, such as each unknown name in each of a space curve's expressions.
/// Each owns its field, so the faults of a shape inside another can be put under that shape's field.
pub(super) struct Faults(pub(super) Vec<(String, SceneErrorCode, String)>);

impl From<Fault> for Faults {
    fn from((field, code, message): Fault) -> Faults {
        Faults(vec![(field.to_string(), code, message)])
    }
}

impl From<Vec<Fault>> for Faults {
    fn from(faults: Vec<Fault>) -> Faults {
        Faults(
            faults
                .into_iter()
                .map(|(field, code, message)| (field.to_string(), code, message))
                .collect(),
        )
    }
}

impl Faults {
    /// The same faults with `field` and a `/` before each field.
    pub(super) fn under(self, field: &str) -> Faults {
        Faults(
            self.0
                .into_iter()
                .map(|(inner, code, message)| (format!("{field}/{inner}"), code, message))
                .collect(),
        )
    }
}

/// A fault at `field` for each mistake in `source`, the expression written there.
pub(super) fn expression_errors(
    field: &'static str,
    source: &str,
) -> impl Fn(Vec<ExpressionError>) -> Vec<Fault> {
    move |errors| {
        errors
            .into_iter()
            .map(|e| (field, SceneErrorCode::BadFormat, format!("`{}` is not an expression sci_anim can read: {} (at column {} of {source:?})", field.rsplit('/').next().unwrap_or(field), e.message, e.column)))
            .collect()
    }
}

/// The space curve a tube writes, with a fault at each expression that cannot be read, or at the
/// expression and the u where the curve has no value or reaches too far, or at `points` when they
/// are all one point.
pub(super) fn space_curve(
    file: SpaceCurveFile,
    base: &std::path::Path,
    on_axes: Option<crate::axes_3d::DataToOwn>,
    law_time: f64,
) -> Result<SpaceCurve, Faults> {
    const FIELDS: [&str; 3] = ["curve/x", "curve/y", "curve/z"];
    let (x, y, z, u_range) = match file {
        SpaceCurveFile::Expressions { x, y, z, u_range } => (x, y, z, u_range),
        SpaceCurveFile::Points { points } => {
            let points: Vec<[f64; 3]> = points.iter().map(|point| point.0).collect();
            return SpaceCurve::smooth_through(&points, on_axes).map_err(|_| {
                (
                    "curve/points",
                    SceneErrorCode::Invalid,
                    "every point is one point, so the curve has no length to draw along"
                        .to_string(),
                )
                    .into()
            });
        }
        SpaceCurveFile::Trace { trace } => {
            let points = atom_trace(&trace, base)
                .map_err(|fault| Faults::from(fault).under("curve/trace"))?;
            return SpaceCurve::smooth_through(&points, on_axes).map_err(|_| {
                (
                    "curve/trace",
                    SceneErrorCode::Invalid,
                    "every atom of the atom trace is at one point, so the curve has no length to draw along"
                        .to_string(),
                )
                    .into()
            });
        }
    };
    let mut faults = Vec::new();
    let mut coordinates = Vec::new();
    for (field, source) in FIELDS.into_iter().zip([&x, &y, &z]) {
        match Expression::parse(source, expr::SPACE_CURVE) {
            Ok(expression) => coordinates.push(expression.at_law_time(law_time)),
            Err(errors) => faults.extend(expression_errors(field, source)(errors)),
        }
    }
    let [lo, hi] = u_range;
    if lo >= hi {
        faults.push((
            "curve/u_range",
            SceneErrorCode::Invalid,
            "`u_range` must give its lowest value first, and the two must differ".to_string(),
        ));
    }
    let Ok(coordinates) = <[Expression; 3]>::try_from(coordinates) else {
        return Err(faults.into());
    };
    if !faults.is_empty() {
        return Err(faults.into());
    }
    SpaceCurve::sample(&coordinates, (lo, hi), on_axes).map_err(|error| {
        let which = ["x", "y", "z"];
        match error {
            SpaceCurveError::NoValue { coordinate, u } => {
                (FIELDS[coordinate], SceneErrorCode::OutOfRange, format!("`{}` has no finite value at u = {u}, and a space curve needs one at every u of `u_range`; narrow `u_range` or change the expression", which[coordinate]))
            }
            SpaceCurveError::TooFar { coordinate, u, value } => (
                FIELDS[coordinate],
                SceneErrorCode::OutOfRange,
                format!("`{}` is {value:e} at u = {u}, and a space curve stays within {} of the object's position", which[coordinate], space_curve::FARTHEST),
            ),
            SpaceCurveError::NoLength => ("curve", SceneErrorCode::Invalid, "the curve is one point at every u of `u_range`, so it has no length to draw along".to_string()),
        }
        .into()
    })
}

/// The surface a `surface` writes, with a fault at each mistake in `z`, at a range left out, not
/// low first, or outside the axes, or at `z` and the x and y where the height has no value or,
/// off 3D axes, reaches too far. On 3D axes a range left out is theirs.
fn read_surface(
    z: &str,
    x_range: Option<[f64; 2]>,
    y_range: Option<[f64; 2]>,
    on: Option<&Axes3d>,
    at_time: ShapeAt,
) -> Result<Surface, Faults> {
    let mut faults = Vec::new();
    let height = Expression::parse(z, expr::SURFACE)
        .map(|height| height.at_law_time(at_time.law_time))
        .map_err(|errors| faults.extend(expression_errors("z", z)(errors)))
        .ok();
    let mut ranges = [(0.0, 0.0); 2];
    for (k, (field, written)) in [("x_range", x_range), ("y_range", y_range)]
        .into_iter()
        .enumerate()
    {
        let Some([lo, hi]) = written.or(on.map(|axes| axes.axes[k].range().into())) else {
            faults.push((field, SceneErrorCode::MissingField, format!("a surface not on 3D axes needs `{field}`, its lowest and highest value as `[low, high]`; or name 3D axes in `axes`, whose ranges it then takes")));
            continue;
        };
        if lo >= hi {
            faults.push((
                field,
                SceneErrorCode::Invalid,
                format!("`{field}` must give its lowest value first, and the two must differ"),
            ));
        } else if lo.abs().max(hi.abs()) > space_curve::FARTHEST {
            faults.push((field, SceneErrorCode::OutOfRange, format!("`{field}` reaches past {}, and a surface stays within that of the object's position", space_curve::FARTHEST)));
        } else if let Some(axes) = on {
            let (low, high) = axes.axes[k].range();
            if hi <= low || lo >= high {
                faults.push((field, SceneErrorCode::Invalid, format!("`{field}` lies outside the axes' {} range, from {low} to {high}, so there is nothing to draw", ["x", "y"][k])));
            }
            ranges[k] = (lo.max(low), hi.min(high));
        } else {
            ranges[k] = (lo, hi);
        }
    }
    let Some(height) = height.filter(|_| faults.is_empty()) else {
        return Err(faults.into());
    };
    let place = on.map_or(Placing::OWN, |axes| Placing {
        to_own: axes.data_to_own(),
        cut: Some(axes.axes[2].range()),
    });
    let sampled = match at_time.sampling {
        Some(sampling) => Surface::sample_on(&height, sampling, place),
        None => Surface::sample(&height, ranges[0], ranges[1], place),
    };
    sampled.map_err(|error| {
        let message = match error {
            SurfaceError::NoValue { x, y } => format!("`z` has no finite value at x = {x}, y = {y}, and a surface needs one at every point of `x_range` by `y_range`; narrow the ranges or change the expression"),
            SurfaceError::TooFar { x, y, value } => format!("`z` is {value:e} at x = {x}, y = {y}, and a surface stays within {} of the object's position", space_curve::FARTHEST),
        };
        ("z", SceneErrorCode::OutOfRange, message).into()
    })
}

/// The region a base or a profile writes, with its faults at `fields`: its radius, points and `d`.
pub(super) fn region(
    file: RegionFile,
    [radius_field, points_field, d_field]: [&'static str; 3],
) -> Result<Region, Fault> {
    let (region, field) = match file {
        RegionFile::Circle { radius, center } => {
            let path = Path::circle_at(center, radius.get())
                .map_err(at(radius_field, "the radius is too small to draw"))?;
            (
                Region {
                    path,
                    rule: FillRule::NonZero,
                },
                radius_field,
            )
        }
        RegionFile::Polygon { points } => (
            Region {
                path: Path::polygon(&points).map_err(at(
                    points_field,
                    "every point of the polygon is the same, so it has no length",
                ))?,
                rule: FillRule::NonZero,
            },
            points_field,
        ),
        RegionFile::Path { d, rule } => (
            Region {
                path: Path::from_svg(&d).map_err(at(
                    d_field,
                    "`d` has no length, so there is nothing to draw",
                ))?,
                rule,
            },
            d_field,
        ),
    };
    if !crate::mesh::has_inside(&region) {
        return Err((
            field,
            SceneErrorCode::Invalid,
            "the region encloses no area, so there is no solid to make".to_string(),
        ));
    }
    Ok(region)
}

/// A solid's material, which must hide what is behind it until see-through solids sort their
/// own faces.
fn opaque(material: Material) -> Result<Material, Fault> {
    if material.opacity() < 1.0 {
        let message = "a solid's material must have `opacity` 1, since a see-through solid cannot be drawn yet; set the object's `opacity` to fade the whole solid";
        return Err((
            "material/opacity",
            SceneErrorCode::Invalid,
            message.to_string(),
        ));
    }
    Ok(material)
}

/// A stroke that faces the camera, a trail's or 3D axes', which is not cut into dashes.
fn undashed(stroke: Stroke, what: &str) -> Result<Stroke, Fault> {
    if stroke.dash.is_some() {
        let message = format!(
            "{what} stroke faces the camera and is drawn unbroken, so it cannot be dashed; remove `dash`"
        );
        return Err(("stroke/dash", SceneErrorCode::Invalid, message));
    }
    Ok(stroke)
}

pub(super) fn axis(which: &str, file: &AxisFile, length: Positive) -> Result<Axis, Fault> {
    let (range_field, spacing_field) = match which {
        "x" => ("x/range", "x/spacing"),
        "y" => ("y/range", "y/spacing"),
        "z" => ("z/range", "z/spacing"),
        _ => ("range", "spacing"),
    };
    if file.spacing.is_some_and(|s| s <= 0.0) {
        return Err((
            spacing_field,
            SceneErrorCode::OutOfRange,
            "`spacing` must be more than 0".to_string(),
        ));
    }
    Axis::new(file.range[0], file.range[1], file.spacing, length.get()).map_err(|error| match error {
        AxisError::EmptyRange => (range_field, SceneErrorCode::Invalid, "`range` must give its lowest value first, and the two must differ".to_string()),
        AxisError::TooManyTicks(count) => {
            (spacing_field, SceneErrorCode::Invalid, format!("this spacing gives {count} ticks over the range, and an axis draws at most {}; use a larger `spacing`", crate::axes::MAX_TICKS))
        }
    })
}

fn label_error(error: LabelError) -> Fault {
    let field = match error.axis {
        "x" => "x/label",
        "y" => "y/label",
        "z" => "z/label",
        _ => "label",
    };
    typeset_error(field)(error.error)
}

/// The axes object a plot names, read before the plot, with its object id.
fn find_axes<'a>(
    object: &str,
    placed: &'a BTreeMap<ObjectId, Object>,
) -> Result<(ObjectId, &'a Axes), Fault> {
    match placed.get_key_value(object) {
        Some((
            id,
            Object {
                shape: Shape::Axes(axes),
                ..
            },
        )) => return Ok((id.clone(), axes)),
        Some((
            _,
            Object {
                shape: Shape::Axes3d(_),
                ..
            },
        )) => {
            let message = format!(
                "`{object}` is 3D axes, and a plot is drawn on 2D `axes`; draw a function of two variables as a `surface` and a curve in space as a `tube` on 3D axes"
            );
            return Err(("axes", SceneErrorCode::Invalid, message));
        }
        _ => {}
    }
    let axes: Vec<String> = placed
        .iter()
        .filter(|(_, o)| matches!(o.shape, Shape::Axes(_)))
        .map(|(id, _)| format!("`{id}`"))
        .collect();
    let known = if axes.is_empty() {
        "the scene has no axes object".to_string()
    } else {
        format!("the axes are {}", axes.join(", "))
    };
    let number_line = matches!(
        placed.get(object).map(|o| &o.shape),
        Some(Shape::NumberLine(_))
    );
    let why = if number_line {
        format!("`{object}` is a number line, which has one axis and no y to plot against; ")
    } else {
        String::new()
    };
    Err((
        "axes",
        SceneErrorCode::UnknownObject,
        format!("{why}no axes object has the id `{object}`; {known}"),
    ))
}

/// The axes a data point names, read before it: 2D axes for a dot, which is flat, and 3D axes
/// for a sphere.
fn find_data_point_axes(
    object: &str,
    placed: &BTreeMap<ObjectId, Object>,
    flat: bool,
) -> Result<ObjectId, Fault> {
    let invalid = |message: String| Err(("axes", SceneErrorCode::Invalid, message));
    match (placed.get_key_value(object), flat) {
        (
            Some((
                id,
                Object {
                    shape: Shape::Axes(_),
                    ..
                },
            )),
            true,
        )
        | (
            Some((
                id,
                Object {
                    shape: Shape::Axes3d(_),
                    ..
                },
            )),
            false,
        ) => Ok(id.clone()),
        (
            Some((
                _,
                Object {
                    shape: Shape::Axes3d(_),
                    ..
                },
            )),
            true,
        ) => invalid(format!(
            "`{object}` is 3D axes, and a dot is flat; the data point on 3D axes is a `sphere`, whose `radius` stays in world units as a dot's does"
        )),
        (
            Some((
                _,
                Object {
                    shape: Shape::Axes(_),
                    ..
                },
            )),
            false,
        ) => invalid(format!(
            "`{object}` is 2D axes, and a sphere is the data point on `3d-axes`; mark a point on 2D axes with a `dot`, whose `axes` names them"
        )),
        (
            Some((
                _,
                Object {
                    shape: Shape::NumberLine(_),
                    ..
                },
            )),
            true,
        ) => invalid(format!(
            "`{object}` is a number line, which has one axis and no y to place a point by; mark a point on `axes`, or place the dot in world units"
        )),
        (Some(_), true) => invalid(format!(
            "`{object}` is not axes, so it has no data coordinates to place a dot by; a dot's `axes` names an `axes` object"
        )),
        (_, false) => find_axes_3d(object, placed).map(|(id, _)| id),
        (None, true) => {
            let axes: Vec<String> = placed
                .iter()
                .filter(|(_, o)| matches!(o.shape, Shape::Axes(_)))
                .map(|(id, _)| format!("`{id}`"))
                .collect();
            let known = if axes.is_empty() {
                "the scene has no axes object".to_string()
            } else {
                format!("the axes are {}", axes.join(", "))
            };
            Err((
                "axes",
                SceneErrorCode::UnknownObject,
                format!("no axes object has the id `{object}`; {known}"),
            ))
        }
    }
}

/// The 3D axes a surface or a tube names, read before it, with their object id.
fn find_axes_3d<'a>(
    object: &str,
    placed: &'a BTreeMap<ObjectId, Object>,
) -> Result<(ObjectId, &'a Axes3d), Fault> {
    match placed.get_key_value(object) {
        Some((
            id,
            Object {
                shape: Shape::Axes3d(axes),
                ..
            },
        )) => Ok((id.clone(), axes)),
        Some((
            _,
            Object {
                shape: Shape::Axes(_),
                ..
            },
        )) => {
            let message = format!(
                "`{object}` is 2D axes, which hold plots; a surface or a tube stands on `3d-axes`, whose x, y and z each give a `range` and a `length`"
            );
            Err(("axes", SceneErrorCode::Invalid, message))
        }
        _ => {
            let axes: Vec<String> = placed
                .iter()
                .filter(|(_, o)| matches!(o.shape, Shape::Axes3d(_)))
                .map(|(id, _)| format!("`{id}`"))
                .collect();
            let known = if axes.is_empty() {
                "the scene has no `3d-axes` object".to_string()
            } else {
                format!("the 3D axes are {}", axes.join(", "))
            };
            Err((
                "axes",
                SceneErrorCode::UnknownObject,
                format!("no 3D axes object has the id `{object}`; {known}"),
            ))
        }
    }
}

fn plot_error(field: &'static str) -> impl Fn(PlotError) -> Fault {
    move |error| {
        match error {
        PlotError::OutsideAxes => ("x_range", SceneErrorCode::Invalid, "`x_range` lies outside the axes' x range, so there is nothing to draw".to_string()),
        PlotError::NothingInside => (field, SceneErrorCode::Invalid, "the curve never enters the axes' box, so there is nothing to draw; check the ranges of the axes".to_string()),
        PlotError::Path(error) => at(field, "the curve has no length")(error),
    }
    }
}

/// The scene error at `field` for a path that cannot be drawn, with `no_length` as the message
/// when the path has no length.
pub(super) fn at(
    field: &'static str,
    no_length: &'static str,
) -> impl Fn(PathError) -> (&'static str, SceneErrorCode, String) {
    move |error| match error {
        PathError::Syntax(message) => (
            field,
            SceneErrorCode::BadFormat,
            format!("`d` is not SVG path syntax: {message}"),
        ),
        PathError::TooLarge => (
            field,
            SceneErrorCode::OutOfRange,
            "a coordinate is past what a 32-bit float holds".to_string(),
        ),
        PathError::NoLength => (field, SceneErrorCode::Invalid, no_length.to_string()),
    }
}

/// A table of `rows`, each cell typeset on its own, or a fault at each row whose length differs
/// from the first's and at each cell that cannot be typeset.
#[allow(clippy::too_many_arguments)]
fn table(
    rows: Vec<Vec<CellFile>>,
    size: Positive,
    color: Color,
    font: Font,
    align: Align,
    padding: Option<CellPadding>,
    rulings: Rulings,
    stroke: Stroke,
) -> Result<Table, Faults> {
    let columns = rows[0].len();
    let layout = Layout {
        size: size.get(),
        align,
    };
    let mut faults = Vec::new();
    let mut placed = Vec::new();
    for (r, row) in rows.into_iter().enumerate() {
        if row.len() != columns {
            let cells = |n: usize| {
                if n == 1 {
                    "1 cell".to_string()
                } else {
                    format!("{n} cells")
                }
            };
            let message = format!(
                "row {r} has {} and row 0 has {}; give every row one cell for each column, `\"\"` for an empty one",
                cells(row.len()),
                cells(columns)
            );
            faults.push((format!("rows/{r}"), SceneErrorCode::WrongLength, message));
            continue;
        }
        let mut cells = Vec::new();
        for (c, cell) in row.into_iter().enumerate() {
            let (set, content, field) = match cell {
                CellFile::Text(text) if text.trim().is_empty() => {
                    cells.push((CellContent::Empty, None));
                    continue;
                }
                CellFile::Text(text) => (
                    typeset::text_box(&text, font, color, layout),
                    CellContent::Text(text),
                    "text",
                ),
                CellFile::Math(MathCellFile { tex }) => (
                    typeset::math_box(&tex, color, layout),
                    CellContent::Math(tex),
                    "tex",
                ),
            };
            match set {
                Ok(set) => cells.push((content, Some(set))),
                Err(error) => {
                    let (_, code, message) = typeset_error(field)(error);
                    let at = match field {
                        "tex" => format!("rows/{r}/{c}/tex"),
                        _ => format!("rows/{r}/{c}"),
                    };
                    faults.push((at, code, message));
                }
            }
        }
        placed.push(cells);
    }
    if !faults.is_empty() {
        return Err(Faults(faults));
    }
    let padding = padding.map_or(0.4 * size.get(), CellPadding::get);
    Ok(Table::new(placed, align, padding, rulings, stroke, size))
}

/// The scene error at `field` for text or math that cannot be typeset.
fn typeset_error(
    field: &'static str,
) -> impl Fn(TypesetError) -> (&'static str, SceneErrorCode, String) {
    move |error| match error {
        TypesetError::Tex(message) => (
            field,
            SceneErrorCode::BadFormat,
            format!("`{field}` is not LaTeX math that sci_anim can typeset: {message}"),
        ),
        TypesetError::Typst {
            message,
            typst_form: Some(form),
        } => (
            field,
            SceneErrorCode::BadFormat,
            format!(
                "`{field}` is not LaTeX math that sci_anim can typeset: {message}, in the Typst math `{form}` that mitex wrote for it"
            ),
        ),
        TypesetError::Typst {
            message,
            typst_form: None,
        } => (
            field,
            SceneErrorCode::BadFormat,
            format!("`{field}` could not be typeset: {message}"),
        ),
        TypesetError::MissingGlyph(c) => (
            field,
            SceneErrorCode::Invalid,
            format!(
                "the bundled fonts have no glyph for {c:?} (U+{:04X})",
                c as u32
            ),
        ),
        TypesetError::NoGlyphs => (
            field,
            SceneErrorCode::Invalid,
            format!("`{field}` has no glyphs, so there is nothing to draw"),
        ),
        TypesetError::Unsupported(what) => (
            field,
            SceneErrorCode::Invalid,
            format!("`{field}` lays out {what}, which sci_anim cannot draw yet"),
        ),
        TypesetError::Color(color) => {
            let (names, last) = typeset::NAMED_COLORS.split_at(typeset::NAMED_COLORS.len() - 1);
            let message = format!(
                "`{field}` gives the color `{color}`, which sci_anim cannot read; write a color in braces as `#rrggbb`, as in `\\textcolor{{#58c4dd}}{{x}}`, or as one of {} and {}, with no `[HTML]` or `[rgb]` before the braces",
                names.join(", "),
                last[0]
            );
            (field, SceneErrorCode::BadFormat, message)
        }
    }
}

/// The curve points of an atom trace, in angstroms from the molecule's centroid, or the fault
/// under `trace` that stops it.
fn atom_trace(trace: &AtomTraceFile, base: &std::path::Path) -> Result<Vec<[f64; 3]>, Fault> {
    let file = &trace.file;
    if !structure_file::is_pdb(std::path::Path::new(file)) {
        return Err((
            "file",
            SceneErrorCode::Invalid,
            format!(
                "an atom trace reads the residues and atom names of a PDB file, whose name ends in `.pdb` or `.ent`, and `{file}` is read as an SDF file, which has neither"
            ),
        ));
    }
    let molecule =
        structure_file::read(&base.join(file)).map_err(structure_file_error(file, base))?;
    let chains: Vec<char> = molecule
        .residues()
        .iter()
        .map(|residue| residue.chain.0)
        .fold(Vec::new(), |mut chains, chain| {
            if !chains.contains(&chain) {
                chains.push(chain);
            }
            chains
        });
    let chain = match &trace.chain {
        Some(chain) => {
            let id = chain
                .chars()
                .next()
                .expect("the schema gives a chain id one character");
            if !chains.contains(&id) {
                let named: Vec<String> = chains.iter().map(|c| format!("`{c}`")).collect();
                return Err((
                    "chain",
                    SceneErrorCode::Invalid,
                    format!(
                        "`{file}` has no chain `{id}`; its chains are {}",
                        named.join(", ")
                    ),
                ));
            }
            Some(id)
        }
        None => None,
    };
    let points: Vec<[f64; 3]> = molecule
        .residues()
        .iter()
        .filter(|residue| chain.is_none_or(|id| residue.chain.0 == id))
        .filter(|residue| StandardResidue::of(&residue.name).is_some())
        .filter_map(|residue| {
            residue
                .atoms
                .iter()
                .find(|(atom_name, _)| *atom_name == trace.atom_name)
                .map(|&(_, number)| molecule.atom(number).position.as_dvec3().to_array())
        })
        .collect();
    if points.len() < 2 {
        let atom_name = &trace.atom_name;
        return Err((
            "atom_name",
            SceneErrorCode::Invalid,
            format!(
                "{} amino acid or nucleotide of `{file}`{} has an atom named `{atom_name}`, and an atom trace needs at least two; an amino acid's alpha carbon is `CA` and a nucleotide's phosphorus is `P`",
                if points.is_empty() { "no" } else { "only one" },
                chain.map_or(String::new(), |id| format!(" in chain `{id}`"))
            ),
        ));
    }
    if points.len() > 10_000 {
        return Err((
            "atom_name",
            SceneErrorCode::OutOfRange,
            format!(
                "`{file}` has {} atoms named `{}`, and a space curve passes through at most 10000; name one `chain`",
                points.len(),
                trace.atom_name
            ),
        ));
    }
    Ok(points)
}

/// The scene error at `file` for a structure file that cannot make a molecule. The message names
/// the file as the scene wrote it and where it was looked for, because a path relative to the
/// scene file is easy to get wrong from another folder.
fn structure_file_error<'a>(
    file: &'a str,
    base: &'a std::path::Path,
) -> impl Fn(StructureFileError) -> (&'static str, SceneErrorCode, String) + 'a {
    move |error| match error {
        StructureFileError::Unreadable(source) => {
            let looked = std::path::absolute(base.join(file)).unwrap_or_else(|_| base.join(file));
            let fetch = if structure_file::is_pdb(std::path::Path::new(file)) {
                format!(
                    "from the Protein Data Bank with `curl -fo {file} https://files.rcsb.org/download/<PDB id>.pdb`"
                )
            } else {
                format!(
                    "from PubChem with `curl -fo {file} \"https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/<name>/SDF?record_type=3d\"`"
                )
            };
            let message = format!(
                "`{file}` cannot be read ({source}); a structure file's path is relative to the scene file, so sci_anim looked for {}; sci_anim never fetches a structure file, so download it first, such as {fetch}",
                looked.display()
            );
            ("file", SceneErrorCode::UnreadableFile, message)
        }
        StructureFileError::Invalid {
            message,
            first_line,
        } => {
            let message = format!(
                "`{file}` is not a structure file sci_anim can draw: {message}; its first line is {first_line:?}"
            );
            ("file", SceneErrorCode::BadStructureFile, message)
        }
    }
}
