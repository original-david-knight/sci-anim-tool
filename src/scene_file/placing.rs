//! What places an object once every object is read: the object a trail follows, the ends a
//! line, arrow or spring lies between, and the object an attachment sits beside.

use super::*;

/// Checks each trail's object once every object is read, and that the trail has no transform of
/// its own. Errors are at the trail's `object` or `position`.
pub(super) fn check_trails(objects: &mut BTreeMap<ObjectId, Object>, errors: &mut Vec<SceneError>) {
    let trails: Vec<(ObjectId, ObjectId, Transform)> = objects
        .iter()
        .filter_map(|(id, o)| match &o.shape {
            Shape::Trail(trail) => Some((id.clone(), trail.object.clone(), o.transform)),
            _ => None,
        })
        .collect();
    for (id, of, transform) in trails {
        let at = JsonPath::root().field("objects").field(&id.to_string());
        let why = match objects.get_key_value(&*of.to_string()) {
            None => {
                let ids: Vec<String> = objects.keys().map(|k| format!("`{k}`")).collect();
                errors.push(SceneError::new(
                    at.field("object"),
                    SceneErrorCode::UnknownObject,
                    format!(
                        "no object has the id `{of}`; the objects are {}",
                        ids.join(", ")
                    ),
                ));
                continue;
            }
            Some((found, _)) if *found == id => Some("itself".to_string()),
            Some((
                found,
                Object {
                    shape: Shape::Trail(_),
                    ..
                },
            )) => Some(format!(
                "`{found}`, another trail, which has no position of its own"
            )),
            Some((
                found,
                Object {
                    placed_by: PlacedBy::Axes(axes),
                    ..
                },
            )) => Some(format!(
                "`{found}`, which its axes `{axes}` place, so it has no position of its own; draw the trail of a data point on `{axes}`"
            )),
            Some(_) => None,
        };
        if let Some(why) = why {
            errors.push(SceneError::new(
                at.field("object"),
                SceneErrorCode::Invalid,
                format!("a trail draws where its object has been, and `{id}` cannot draw {why}"),
            ));
        } else if transform != Transform::default() {
            let message = format!(
                "`{id}` is drawn where `{of}` has been, in the world, so it has no position, rotation or scale of its own"
            );
            errors.push(SceneError::new(
                at.field("position"),
                SceneErrorCode::PlacedByTrail,
                message,
            ));
        }
    }
}

/// Places each line, arrow and spring with an end on an object, or a spring, by its ends, once
/// every object is read. Errors are at the end at fault, or at `position` for a transform of its
/// own.
pub(super) fn place_by_ends(
    objects: &mut BTreeMap<ObjectId, Object>,
    ended: Vec<(String, (EndFile, EndFile, crate::ends::Drawn))>,
    attached: &[&str],
    camera: &Camera,
    errors: &mut Vec<SceneError>,
) {
    let ended_ids: Vec<String> = ended.iter().map(|(id, _)| id.clone()).collect();
    for (id, (from, to, drawn)) in ended {
        let at = JsonPath::root().field("objects").field(&id);
        let mut end = |file: EndFile, field: &str| -> Option<End> {
            match file {
                EndFile::Point(point) => match point.as_slice() {
                    [x, y] => Some(End::Point(Vec3::new(*x, *y, 0.0))),
                    [x, y, z] => Some(End::Point(Vec3::new(*x, *y, *z))),
                    _ => unreachable!("the schema allows two or three numbers"),
                },
                EndFile::OnCurve(OnCurveFile { object: on, .. })
                | EndFile::OnPlot(OnPlotFile { object: on, .. })
                | EndFile::OnAtom(OnAtomFile { object: on, .. })
                | EndFile::On(OnObjectFile { object: on, .. })
                    if !objects.contains_key(on.as_str()) =>
                {
                    let ids: Vec<String> = objects.keys().map(|k| format!("`{k}`")).collect();
                    errors.push(SceneError::new(
                        at.field(field).field("object"),
                        SceneErrorCode::UnknownObject,
                        format!(
                            "no object has the id `{on}`; the objects are {}",
                            ids.join(", ")
                        ),
                    ));
                    None
                }
                EndFile::OnCurve(on) => {
                    let (object, declared) = objects.get_key_value(on.object.as_str())?;
                    let curve = match &declared.shape {
                        Shape::Solid {
                            solid: Solid::Tube { curve, .. },
                            ..
                        } if !ended_ids.contains(&on.object) => Ok(curve),
                        Shape::Solid {
                            solid: Solid::Tube { .. },
                            ..
                        } => Err(format!(
                            "`{object}`, a spring, which is placed by its own ends and drawn again between them"
                        )),
                        Shape::Flat { .. } if matches!(declared.placed_by, PlacedBy::Axes(_)) => {
                            Err(format!(
                                "`{object}`, a plot, whose curve is read at an `x` in its axes' data coordinates; write `{{ \"object\": \"{object}\", \"x\": ... }}`"
                            ))
                        }
                        _ => Err(format!(
                            "`{object}`, which is not a tube and has no space curve; an end on an object's position is `{{ \"object\": \"{object}\", \"offset\": [0, 0, 0] }}`"
                        )),
                    };
                    let curve = match curve {
                        Ok(curve) => curve,
                        Err(why) => {
                            errors.push(SceneError::new(at.field(field).field("object"), SceneErrorCode::Invalid, format!("an end of `{id}` at a `u` sits on a tube's space curve, and cannot sit on {why}")));
                            return None;
                        }
                    };
                    let (lo, hi) = curve.u_range();
                    if !(lo..=hi).contains(&on.u) {
                        errors.push(SceneError::new(
                            at.field(field).field("u"),
                            SceneErrorCode::OutOfRange,
                            format!(
                                "u {} lies outside `{object}`'s u range, {lo} to {hi}; write a u within it",
                                on.u
                            ),
                        ));
                        return None;
                    }
                    Some(End::OnCurve {
                        object: object.clone(),
                        u: on.u,
                    })
                }
                EndFile::OnPlot(on) => {
                    let (object, declared) = objects.get_key_value(on.object.as_str())?;
                    let axes = match (&declared.placed_by, &declared.shape) {
                        (PlacedBy::Axes(axes), Shape::Flat { .. }) => match &objects[axes].shape {
                            Shape::Axes(axes) => Some(axes),
                            _ => None,
                        },
                        _ => None,
                    };
                    let Some(axes) = axes else {
                        let hint = if matches!(
                            declared.shape,
                            Shape::Solid {
                                solid: Solid::Tube { .. },
                                ..
                            }
                        ) {
                            format!(
                                "a tube, whose space curve is read at a `u`; write `{{ \"object\": \"{object}\", \"u\": ... }}`"
                            )
                        } else {
                            format!(
                                "not a plot; an end on an object's position is `{{ \"object\": \"{object}\", \"offset\": [0, 0, 0] }}`"
                            )
                        };
                        errors.push(SceneError::new(
                            at.field(field).field("object"),
                            SceneErrorCode::Invalid,
                            format!(
                                "an end of `{id}` at an `x` sits on a plot's curve, and `{object}` is {hint}"
                            ),
                        ));
                        return None;
                    };
                    let (lo, hi) = axes.x.range();
                    if !(lo..=hi).contains(&on.x) {
                        errors.push(SceneError::new(
                            at.field(field).field("x"),
                            SceneErrorCode::OutOfRange,
                            format!(
                                "x {} lies outside the x range of `{object}`'s axes, {lo} to {hi}; write an x within it",
                                on.x
                            ),
                        ));
                        return None;
                    }
                    Some(End::OnPlot {
                        object: object.clone(),
                        x: axes.x.to_local(on.x),
                    })
                }
                EndFile::OnAtom(on) => {
                    let (object, declared) = objects.get_key_value(on.object.as_str())?;
                    let Shape::Molecule { molecule, .. } = &declared.shape else {
                        errors.push(SceneError::new(
                            at.field(field).field("object"),
                            SceneErrorCode::Invalid,
                            format!(
                                "an end of `{id}` at an `atom` sits on an atom of a molecule, and `{object}` is not a molecule; an end on an object's position is `{{ \"object\": \"{object}\", \"offset\": [0, 0, 0] }}`"
                            ),
                        ));
                        return None;
                    };
                    let Some(atom) = molecule.atom_number(on.atom) else {
                        errors.push(SceneError::new(
                            at.field(field).field("atom"),
                            SceneErrorCode::NoSuchAtom,
                            format!(
                                "`{object}` has {}, and an end of `{id}` names atom {}",
                                molecule.numbering(),
                                on.atom
                            ),
                        ));
                        return None;
                    };
                    Some(End::OnAtom {
                        object: object.clone(),
                        atom,
                    })
                }
                EndFile::On(on) => {
                    let (object, declared) = objects.get_key_value(on.object.as_str())?;
                    let why = if on.object == id {
                        Some("itself".to_string())
                    } else if attached.contains(&on.object.as_str())
                        || ended_ids.contains(&on.object)
                        || matches!(declared.shape, Shape::Trail(_))
                        || !matches!(
                            declared.placed_by,
                            PlacedBy::Itself | PlacedBy::DataPoint(_)
                        )
                    {
                        Some(format!(
                            "`{object}`, which is a plot, a trail, attached or placed by its own ends, and so has no position of its own"
                        ))
                    } else {
                        None
                    };
                    if let Some(why) = why {
                        let instead = if matches!(declared.placed_by, PlacedBy::Axes(_))
                            && matches!(declared.shape, Shape::Flat { .. })
                        {
                            format!(
                                "; an end on a plot's curve is `{{ \"object\": \"{object}\", \"x\": ... }}`, at an x of its axes"
                            )
                        } else {
                            String::new()
                        };
                        errors.push(SceneError::new(at.field(field).field("object"), SceneErrorCode::Invalid, format!("an end of `{id}` cannot sit on {why}; sit it on an object placed by its own transform, or on a data point{instead}")));
                        return None;
                    }
                    Some(End::On {
                        object: object.clone(),
                        offset: Vec3::from_array(on.offset),
                    })
                }
            }
        };
        let (Some(from), Some(to)) = (end(from, "from"), end(to, "to")) else {
            continue;
        };
        // Ends on objects may meet as the objects move, and the shape draws nothing then; two
        // fixed points that are one point never draw it.
        if let (End::Point(a), End::Point(b)) = (&from, &to)
            && a == b
        {
            errors.push(SceneError::new(
                at.field("to"),
                SceneErrorCode::Invalid,
                format!("`{id}`'s `to` is its `from`, so it has no length; move one end"),
            ));
            continue;
        }
        let object = objects.get(id.as_str()).expect("read above");
        if object.transform != Transform::default() {
            let message = format!(
                "`{id}` is placed by its ends and lies between them, with no position, rotation or scale of its own; set each end's `offset` instead"
            );
            errors.push(SceneError::new(
                at.field("position"),
                SceneErrorCode::PlacedByEnds,
                message,
            ));
            continue;
        }
        let ends = Ends { from, to, drawn };
        let (from_world, to_world) = (ends.from.world(objects), ends.to.world(objects));
        let placed = ends.place(from_world, to_world, camera, &object.shape);
        let object = objects.get_mut(id.as_str()).expect("read above");
        if let Some((transform, shape)) = placed {
            (object.transform, object.shape) = (transform, shape);
        }
        object.placed_by = PlacedBy::Ends(ends);
    }
}

/// Gives each attached object what places it, once every object and group is read. Errors are at
/// the `attach` field at fault.
pub(super) fn attach_all(
    objects: &mut BTreeMap<ObjectId, Object>,
    attaches: Vec<(String, AttachFile)>,
    errors: &mut Vec<SceneError>,
) {
    let attached: Vec<String> = attaches.iter().map(|(id, _)| id.clone()).collect();
    for (id, attach) in attaches {
        let at = JsonPath::root().field("objects").field(&id).field("attach");
        let object_id = ObjectId::new(&id);
        let Some((to_id, to)) = objects.get_key_value(attach.object.as_str()) else {
            let ids: Vec<String> = objects.keys().map(|k| format!("`{k}`")).collect();
            errors.push(SceneError::new(
                at.field("object"),
                SceneErrorCode::UnknownObject,
                format!(
                    "no object has the id `{}`; the objects are {}",
                    attach.object,
                    ids.join(", ")
                ),
            ));
            continue;
        };
        if *to_id == object_id
            || !matches!(to.placed_by, PlacedBy::Itself | PlacedBy::DataPoint(_))
            || attached.contains(&attach.object)
        {
            let why = if *to_id == object_id {
                "itself".to_string()
            } else {
                format!(
                    "`{to_id}`, which is a plot or attached itself, and so has no transform of its own to sit beside"
                )
            };
            errors.push(SceneError::new(
                at.field("object"),
                SceneErrorCode::NotAttachable,
                format!("`{id}` cannot be attached to {why}"),
            ));
            continue;
        }
        let group = match (&attach.group, &to.shape) {
            (None, _) => None,
            (Some(group), Shape::Molecule { groups, .. }) => match groups.find(group) {
                Some((group, _)) => Some(group.clone()),
                None => {
                    let known: Vec<String> = groups.iter().map(|(g, _)| format!("`{g}`")).collect();
                    let known = if known.is_empty() {
                        "it has no groups; declare them in its `groups`".to_string()
                    } else {
                        format!("its groups are {}", known.join(", "))
                    };
                    errors.push(SceneError::new(
                        at.field("group"),
                        SceneErrorCode::UnknownGroup,
                        format!("`{}` has no group `{group}`; {known}", attach.object),
                    ));
                    continue;
                }
            },
            (Some(_), _) => {
                let message = format!(
                    "`{}` is not a molecule, and only a molecule has atom groups; leave out `group` to sit beside the object",
                    attach.object
                );
                errors.push(SceneError::new(
                    at.field("group"),
                    SceneErrorCode::NotMolecule,
                    message,
                ));
                continue;
            }
        };
        let target = Attachment {
            object: to_id.clone(),
            group,
            offset: attach.offset.into(),
        };
        let object = objects.get_mut(&object_id).expect("read above");
        if !matches!(object.shape, Shape::Flat { .. } | Shape::Glyphs { .. })
            || object.placed_by != PlacedBy::Itself
        {
            errors.push(SceneError::new(at, SceneErrorCode::NotAttachable, format!("`{id}` is not a flat shape, text or math that sits on its own, and only those can be attached")));
        } else if object.transform != Transform::default() {
            let message = format!(
                "`{id}` is attached to `{}` and sits beside it, with no position, rotation or scale of its own; set `offset` in `attach` instead",
                target.object
            );
            errors.push(SceneError::new(
                at.field("offset"),
                SceneErrorCode::Attached,
                message,
            ));
        } else {
            object.placed_by = PlacedBy::Attachment(target);
        }
    }
}

impl ShapeFile {
    pub(super) fn default_tip_length() -> Positive {
        Positive::try_from(0.35).expect("positive")
    }

    pub(super) fn default_size() -> Positive {
        Positive::try_from(0.6).expect("positive")
    }

    pub(super) fn default_color() -> Color {
        Color {
            r: 0xff,
            g: 0xff,
            b: 0xff,
        }
    }

    pub(super) fn default_hydrogens() -> bool {
        true
    }

    pub(super) fn default_ruling_stroke() -> Stroke {
        Stroke {
            color: Color {
                r: 0x88,
                g: 0x88,
                b: 0x88,
            },
            width: Positive::try_from(0.02).expect("positive"),
            dash: None,
        }
    }

    pub(super) fn default_label_size() -> Positive {
        Positive::try_from(0.3).expect("positive")
    }

    pub(super) fn default_dot_fill() -> Fill {
        Fill::solid(ShapeFile::default_color())
    }

    pub(super) fn default_turns() -> Turns {
        Turns::try_from(8.0).expect("within the turns a spring makes")
    }

    pub(super) fn default_spring_radius() -> Positive {
        Positive::try_from(0.2).expect("positive")
    }

    pub(super) fn default_tube_radius() -> Positive {
        Positive::try_from(0.03).expect("positive")
    }

    /// A line's or an arrow's ends when either sits on an object or is a point in space, and a
    /// spring's always, with how they are drawn, taken out of the shape and left as `(0, 0)` to
    /// `(1, 0)` there, so the shape reads as a placeholder the stage replaces. `None` for a
    /// shape placed by its own transform.
    pub(super) fn take_ends(&mut self) -> Option<(EndFile, EndFile, crate::ends::Drawn)> {
        let placeholder = || {
            (
                EndFile::Point(vec![0.0, 0.0]),
                EndFile::Point(vec![1.0, 0.0]),
            )
        };
        let (from, to, drawn) = match self {
            ShapeFile::Line { from, to, .. }
                if from.in_plane().is_none() || to.in_plane().is_none() =>
            {
                (from, to, crate::ends::Drawn::Line)
            }
            ShapeFile::Arrow {
                from,
                to,
                tip_length,
                ..
            } if from.in_plane().is_none() || to.in_plane().is_none() => (
                from,
                to,
                crate::ends::Drawn::Arrow {
                    tip_length: tip_length.get(),
                },
            ),
            ShapeFile::Spring {
                from,
                to,
                turns,
                radius,
                ..
            } => (
                from,
                to,
                crate::ends::Drawn::Spring {
                    turns: turns.get(),
                    radius: radius.get(),
                },
            ),
            _ => return None,
        };
        let (start, end) = placeholder();
        Some((
            std::mem::replace(from, start),
            std::mem::replace(to, end),
            drawn,
        ))
    }
}
