//! The stage at a scene time: the start stage with every value its track's value then, and the
//! objects placed by others placed after.

use super::*;

impl Timeline {
    /// The world position of each of `object_ids` at each of `scene_times`, the places a trail
    /// draws, from one stage at each scene time.
    pub fn positions<'a>(
        &self,
        start: &Stage,
        object_ids: impl Iterator<Item = &'a ObjectId>,
        scene_times: &[f64],
    ) -> BTreeMap<ObjectId, Vec<Vec3>> {
        let mut positions: BTreeMap<ObjectId, Vec<Vec3>> = object_ids
            .map(|id| (id.clone(), Vec::with_capacity(scene_times.len())))
            .collect();
        if positions.is_empty() {
            return positions;
        }
        for &scene_time in scene_times {
            let stage = self.stage(start, scene_time, Staging::BeforeTrails);
            for (id, passed) in &mut positions {
                passed.push(stage.objects[id].flat_to_world().position());
            }
        }
        positions
    }

    /// The start stage with every animated value replaced by its track's value at `scene_time`.
    /// An opacity, created share or field of view an overshoot carries out of range is clamped
    /// into it.
    pub fn stage_at(&self, start: &Stage, scene_time: f64) -> Stage {
        self.stage(start, scene_time, Staging::Whole)
    }

    pub(super) fn stage(&self, start: &Stage, scene_time: f64, staging: Staging) -> Stage {
        let mut stage = start.clone();
        stage.camera = self.camera.at(&start.camera, scene_time);
        for (id, tracks) in &self.objects {
            let object = stage
                .objects
                .get_mut(id)
                .expect("a track's object id names an object of the start stage");
            let transform = &mut object.transform;
            transform.position = tracks.position.at(scene_time).unwrap_or(transform.position);
            transform.rotation = tracks.rotation.at(scene_time).unwrap_or(transform.rotation);
            transform.scale = tracks.scale.at(scene_time).unwrap_or(transform.scale);
            if let Some(opacity) = tracks.opacity.at(scene_time) {
                object.opacity = UnitInterval::clamped(opacity);
            }
            if let Some(created) = tracks.created.at(scene_time) {
                object.created = UnitInterval::clamped(created);
            }
            // A law time with no shape, off the frame times `validate` checked, keeps the shape at 0.
            if let (Some(law_time), Some(law)) = (tracks.shape.at(scene_time), &object.shape_law)
                && let Ok(shape) = law.at(law_time)
            {
                object.shape = shape;
            }
            if let (Some(value), Some(readout)) = (tracks.value.at(scene_time), &object.readout) {
                stage.values.insert(id.clone(), value);
                if let Shape::Glyphs { glyphs, .. } = &mut object.shape {
                    *glyphs = readout.glyphs(value);
                }
            }
            let declared = Colors::of(&object.shape);
            tracks
                .colors_at(&declared, scene_time)
                .paint(&mut object.shape);
            object.plane_map = tracks.plane_map.at(scene_time).map(|value| value.plane_map);
        }
        for (id, tracks) in &self.objects {
            let Some(morph) = &tracks.morph else { continue };
            let phase = if scene_time < morph.span.0 {
                Phase::Before
            } else if scene_time < morph.span.1 {
                Phase::During
            } else {
                Phase::After
            };
            let progress = morph
                .progress
                .at(scene_time)
                .expect("a morph's track has its one segment");
            if phase == Phase::During
                && progress != 0.0
                && let Some(shape) = morph.plan.at(progress)
            {
                stage
                    .objects
                    .get_mut(id)
                    .expect("a track's object id names an object of the start stage")
                    .shape = shape;
            }
            stage.morphs.insert(
                id.clone(),
                StagedMorph {
                    target: morph.target.clone(),
                    phase,
                    progress,
                },
            );
        }
        for (id, tracks) in self.objects.iter().filter(|(_, t)| !t.members.is_empty()) {
            let shape = &mut stage
                .objects
                .get_mut(id)
                .expect("a track's object id names an object of the start stage")
                .shape;
            stage_members(shape, &tracks.members, scene_time);
        }
        // Carried objects next: each reads its `along` object where the tracks put it, and an
        // `along` object is never carried itself, so one pass places them all.
        let carried: Vec<(ObjectId, follow::CarryPoint)> = self
            .objects
            .iter()
            .filter_map(|(id, tracks)| {
                let (carry, progress, reference) = tracks.position.carry_at(scene_time)?;
                Some((
                    id.clone(),
                    carry.at(progress, along_to_world(&stage, &carry.along), reference),
                ))
            })
            .collect();
        for (id, point) in carried {
            let position = data_of(&stage, &id, point.position);
            let transform = &mut stage
                .objects
                .get_mut(&id)
                .expect("a track's object id names an object of the start stage")
                .transform;
            transform.position = position.unwrap_or(transform.position);
            transform.rotation = point.rotation.unwrap_or(transform.rotation);
        }
        // Data points next: each is placed by axes or 3D axes, which have their own transforms,
        // and an object attached to a data point reads where it lands.
        let data_points: Vec<(ObjectId, Vec3)> = stage
            .objects
            .iter()
            .filter_map(|(id, o)| match &o.placed_by {
                PlacedBy::DataPoint(axes) => Some((
                    id.clone(),
                    DataToWorld::of(&stage.objects[axes]).world(o.transform.position),
                )),
                _ => None,
            })
            .collect();
        for (id, world) in data_points {
            stage
                .objects
                .get_mut(&id)
                .expect("listed above")
                .transform
                .position = world;
        }
        // Shapes placed by their ends next: an end sits on an object placed by its own transform
        // or a data point, which have their places now. Ends that meet draw nothing.
        let ended: Vec<(ObjectId, Option<(Transform, Shape)>)> = stage
            .objects
            .iter()
            .filter_map(|(id, o)| match &o.placed_by {
                PlacedBy::Ends(ends) => Some((
                    id.clone(),
                    ends.place(
                        ends.from.world(&stage.objects),
                        ends.to.world(&stage.objects),
                        &stage.camera,
                        &o.shape,
                    ),
                )),
                _ => None,
            })
            .collect();
        for (id, placed) in ended {
            let object = stage.objects.get_mut(&id).expect("listed above");
            match placed {
                Some((transform, shape)) => (object.transform, object.shape) = (transform, shape),
                None => object.opacity = UnitInterval::clamped(0.0),
            }
        }
        // Placed objects last: they read the transforms, plane maps and the camera this scene
        // time ended at, so a plot of carried axes and an object attached to a carried object go
        // with it. A plot has its axes' plane map, and an attached object has none.
        let placed: Vec<(ObjectId, Transform, Option<PlaneMap>)> = stage
            .objects
            .iter()
            .filter_map(|(id, o)| match &o.placed_by {
                PlacedBy::Itself | PlacedBy::DataPoint(_) | PlacedBy::Ends(_) => None,
                PlacedBy::Axes(axes) => Some((
                    id.clone(),
                    stage.objects[axes].transform,
                    stage.objects[axes].plane_map,
                )),
                PlacedBy::Attachment(attachment) => {
                    let object = &stage.objects[&attachment.object];
                    let center = match (&attachment.group, &object.shape) {
                        (None, _) => Vec3::ZERO,
                        (Some(group), Shape::Molecule { groups, .. }) => {
                            groups
                                .get(group)
                                .expect("`scene_file` checked the group")
                                .center
                        }
                        (Some(_), _) => unreachable!(
                            "`scene_file` checked that an object with groups is a molecule"
                        ),
                    };
                    Some((
                        id.clone(),
                        attachment.transform(
                            &object.flat_to_world(),
                            center,
                            object.shape.reach(),
                            &stage.camera,
                        ),
                        None,
                    ))
                }
            })
            .collect();
        for (id, transform, plane_map) in placed {
            let object = stage.objects.get_mut(&id).expect("listed above");
            object.transform = transform;
            object.plane_map = plane_map;
        }
        let gone: Vec<ObjectId> = stage
            .objects
            .iter()
            .filter(|(id, o)| {
                let lives_by = match &o.placed_by {
                    PlacedBy::Attachment(attachment) => &attachment.object,
                    _ => *id,
                };
                self.lifetimes
                    .get(lives_by)
                    .is_some_and(|lifetime| !lifetime.at(scene_time))
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in gone {
            stage.objects.get_mut(&id).expect("listed above").opacity = UnitInterval::clamped(0.0);
        }
        // A shape placed by its ends is hidden while the object either end sits on is, by its
        // opacity or before its create.
        let hidden: Vec<ObjectId> = stage
            .objects
            .iter()
            .filter(|(_, o)| match &o.placed_by {
                PlacedBy::Ends(ends) => ends.objects().any(|on| {
                    stage.objects[on].opacity.get() == 0.0 || stage.objects[on].created.get() == 0.0
                }),
                _ => false,
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in hidden {
            stage.objects.get_mut(&id).expect("listed above").opacity = UnitInterval::clamped(0.0);
        }
        // Trails last: each draws its object where the passes above left it.
        let trails: Vec<(ObjectId, Vec<Vec3>)> = stage
            .objects
            .iter()
            .filter(|_| staging == Staging::Whole)
            .filter_map(|(id, o)| match &o.shape {
                Shape::Trail(trail) => Some((
                    id.clone(),
                    trail.points_at(
                        scene_time,
                        stage.objects[&trail.object].flat_to_world().position(),
                    ),
                )),
                _ => None,
            })
            .collect();
        for (id, points) in trails {
            if let Shape::Trail(trail) =
                &mut stage.objects.get_mut(&id).expect("listed above").shape
            {
                trail.points = points;
            }
        }
        stage.reactions = self
            .reactions
            .iter()
            .map(|r| {
                let phase = if scene_time < r.span.0 {
                    Phase::Before
                } else if scene_time < r.span.1 {
                    Phase::During
                } else {
                    Phase::After
                };
                StagedReaction {
                    phase,
                    blend: r
                        .blend
                        .at(scene_time)
                        .expect("a reaction's track has its one segment"),
                    fit_rmsd: r.fit_rmsd,
                    glows: Arc::clone(&r.glows),
                }
            })
            .collect();
        stage
    }
}

/// How much of a stage `Timeline::stage` works out: all of it, or all but its trails, which are
/// drawn from where objects were and cannot move one.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Staging {
    Whole,
    BeforeTrails,
}

/// Where an `along` object's own plane, or a tube's space, lands in the world on `stage`: a plot
/// is where its axes are, before the placed pass of `stage_at` copies them onto it.
pub(super) fn along_to_world(stage: &Stage, along: &ObjectId) -> FlatToWorld {
    crate::scene::own_to_world(&stage.objects, along)
}

/// The position a carry's `world` point gives the object `id` on `stage`: the point itself, or
/// for a data point its data coordinates on its axes, or `None` while those axes are scaled flat.
pub(super) fn data_of(stage: &Stage, id: &ObjectId, world: Vec3) -> Option<Vec3> {
    match &stage.objects[id].placed_by {
        PlacedBy::DataPoint(axes) => DataToWorld::of(&stage.objects[axes]).data(world),
        _ => Some(world),
    }
}

/// Each member's values at `scene_time` set on its object's shape: a group's or a cell's glow, a
/// copy's position, own opacity, own colors and glow, and a bar's own colors and glow.
fn stage_members(shape: &mut Shape, members: &BTreeMap<Member, MemberTracks>, scene_time: f64) {
    match shape {
        Shape::Molecule { glows, .. } => {
            for (member, tracks) in members {
                if let (Member::Group(group), Some(glow)) = (member, tracks.glow.at(scene_time)) {
                    glows.insert(group.clone(), glow);
                }
            }
        }
        Shape::Table(table) => {
            for (member, tracks) in members {
                if let (Member::Cell(address), Some(glow)) = (member, tracks.glow.at(scene_time)) {
                    table.glows.insert(*address, glow);
                }
            }
        }
        Shape::Pattern(pattern) => {
            let mut copies = pattern.copies.to_vec();
            for (member, tracks) in members {
                let Some(k) = (match member {
                    Member::Copy(index) => pattern.ordinal(*index),
                    _ => None,
                }) else {
                    continue;
                };
                let copy = &mut copies[k];
                if let Some(position) = tracks.position.at(scene_time) {
                    copy.transform.position = position;
                }
                if let Some(opacity) = tracks.opacity.at(scene_time) {
                    copy.opacity = opacity.clamp(0.0, 1.0);
                }
                copy.own_colors = tracks.own_colors_at(scene_time);
                if let Some(glow) = tracks.glow.at(scene_time) {
                    copy.glow = glow;
                }
            }
            pattern.copies = copies.into();
        }
        Shape::BarPlot(bar_plot) => {
            for (member, tracks) in members {
                let Member::Bar(k) = member else { continue };
                let bar = &mut bar_plot.bars[*k];
                bar.own_colors = tracks.own_colors_at(scene_time);
                if let Some(glow) = tracks.glow.at(scene_time) {
                    bar.glow = glow;
                }
            }
        }
        _ => {}
    }
}
