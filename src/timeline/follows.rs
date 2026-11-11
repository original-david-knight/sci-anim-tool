//! Putting values set at every scene time of a span on their tracks: a follow's law, shape law or
//! carry, and a simulation's sets.

use super::*;
use crate::circuit::{self, CircuitMotion};

impl Timeline {
    /// Samples each surface a follow runs at the same points at every scene time: every point it
    /// needs at `t` of 0 and at each frame time of its follows, so its triangles stay put. Its
    /// shape at 0 and at each of those frame times is read again on those points, which a frame
    /// time's own points do not all hold, and its errors are each fault there, as `push_shape`
    /// reports one.
    pub fn fix_shape_samplings(&self, start: &mut Stage) -> Vec<SceneError> {
        let mut errors = Vec::new();
        for (
            id,
            ShapeSampling {
                sampling,
                frame_times,
            },
        ) in &self.shape_samplings
        {
            let object = start
                .objects
                .get_mut(id)
                .expect("a follow names an object of the start stage");
            let declared = ShapeLaw::sampling_of(&object.shape)
                .map_or_else(|| sampling.clone(), |declared| declared.union(sampling));
            let law = object
                .shape_law
                .as_mut()
                .expect("a follow of a shape names a shape law");
            Arc::get_mut(law)
                .expect("only the start stage holds a shape law while the scene file is read")
                .sampling = Some(declared);
            for &(scene_time, law_time) in std::iter::once(&(0.0, 0.0)).chain(frame_times) {
                match law.at(law_time) {
                    Ok(shape) if law_time == 0.0 && scene_time == 0.0 => object.shape = shape,
                    Ok(_) => {}
                    Err(faults) => shape_faults(id, scene_time, law_time, faults, &mut errors),
                }
            }
        }
        errors
    }

    /// Adds a follow to the track of the value it sets, and returns every scene error it has at
    /// once. `guard` has already refused a follow of an object placed by another or carried by
    /// a linear map.
    pub(super) fn push_follow(
        &mut self,
        follow: &Follow,
        span: (f64, f64),
        start: &Stage,
        easing_curve: EasingCurve,
        path: &JsonPath,
    ) -> Vec<SceneError> {
        match follow {
            Follow::Law {
                followed,
                law,
                law_time_start,
            } => self.push_law(followed, law, law_time_start, span, start, path),
            Follow::Carry { object_id, carry } => {
                self.push_carry(object_id, carry, span, start, easing_curve, path)
            }
            Follow::Shape {
                object_id,
                law_time_start,
            } => self.push_shape(object_id, law_time_start, span, start, path),
        }
    }

    /// Where a follow's law time counts from in scene time, or `unknown_chapter` at `from` for a
    /// chapter that has not begun.
    pub(super) fn law_start(
        &self,
        law_time_start: &LawTimeStart,
        span: (f64, f64),
        path: &JsonPath,
    ) -> Result<f64, SceneError> {
        match law_time_start {
            LawTimeStart::Span => Ok(span.0),
            LawTimeStart::Chapter(id) => self.chapter_start(id).ok_or_else(|| {
                let begun: Vec<String> = self.chapters.iter().map(|chapter| &chapter.id).chain(self.open_chapter.iter().map(|(id, _)| id)).map(|id| format!("`{}`", &**id)).collect();
                let known = if begun.is_empty() { "no chapter has begun by this step; put the steps that continue one motion in a `chapter`, and name it here".to_string() } else { format!("the chapters begun by this step are {}", begun.join(", ")) };
                let message = format!("law time counts from the start of the chapter `from` names, and no chapter that has begun by this step has the id `{}`; {known}", &**id);
                SceneError::new(path.field("from"), SceneErrorCode::UnknownChapter, message)
            }),
        }
    }

    /// Integrates a simulation over its span and puts each value it sets on that value's track.
    /// Its errors: `unknown_chapter` at `from`; `out_of_range` at the simulation when it spans
    /// more than `simulate::MOST_FRAMES` frames; `out_of_range` at the rate of the first variable
    /// with no finite value, and at each set expression with none, or a position's past
    /// `follow::FARTHEST`, at a frame time of the span or its end; the guards of each object
    /// whose transform it sets, at that set's `object`; the tracks' `conflicting_animations`; and
    /// `law_jump` at a set that does not start where its value is.
    pub(super) fn push_simulate(
        &mut self,
        simulate: &Arc<Simulate>,
        span: (f64, f64),
        start: &Stage,
        path: &JsonPath,
    ) -> Vec<SceneError> {
        let law_start = match self.law_start(&simulate.law_time_start, span, path) {
            Ok(law_start) => law_start,
            Err(error) => return vec![error],
        };
        let frames = (span.1 - span.0) * self.fps;
        if frames > simulate::MOST_FRAMES {
            let message = format!(
                "this simulation spans {} frames, and a simulation spans at most {}, an hour at 60 frames a second; shorten its span or lower the scene's fps",
                frames.round(),
                simulate::MOST_FRAMES
            );
            return vec![SceneError::new(
                path.clone(),
                SceneErrorCode::OutOfRange,
                message,
            )];
        }
        let placed = match self.placed_bodies(simulate, span, start, path) {
            Ok(placed) => placed,
            Err(errors) => return errors,
        };
        let circuit_state = match &simulate.motion {
            Motion::Circuit(motion) => match self.circuit_start(motion, span, path) {
                Ok(state) => state,
                Err(errors) => return errors,
            },
            _ => Vec::new(),
        };
        let simulation = match Simulation::integrate(
            simulate,
            span,
            law_start,
            self.fps,
            &placed,
            &circuit_state,
        ) {
            Ok(simulation) => Arc::new(simulation),
            Err(NoValue {
                variable,
                scene_time,
                law_time,
            }) => {
                let name = &simulate.variables[variable];
                let message = format!(
                    "`{name}` has no finite value at scene time {} (law time {}), and a simulation needs one at every step of its span; change its rates, its starting values or the span",
                    follow::short(scene_time),
                    follow::short(law_time)
                );
                return vec![SceneError::new(
                    path.field("rates").field(name),
                    SceneErrorCode::OutOfRange,
                    message,
                )];
            }
        };
        if let Motion::Circuit(motion) = &simulate.motion {
            self.push_circuit_run(motion, &simulation, span, path);
        }
        self.collisions.extend(
            simulation
                .collisions
                .iter()
                .map(|collision| CollisionReport {
                    collision: collision.clone(),
                    path: path.clone(),
                }),
        );
        let body_sets = simulate.body_sets();
        let mut errors = Vec::new();
        for (i, set) in simulate.sets.iter().enumerate() {
            // A body's own sets are not in the scene file, so their faults are the body's.
            let body =
                (i < body_sets).then(|| path.field("bodies").field(&set.object_id().to_string()));
            let at = body
                .clone()
                .unwrap_or_else(|| path.field("sets").index(i - body_sets));
            let field_at = |field: JsonPath| body.clone().unwrap_or(field);
            let outright = Outright::Set {
                simulation: Arc::clone(&simulation),
                set: i,
            };
            errors.extend(
                outright
                    .faults(frame_times_in(span, self.fps))
                    .iter()
                    .map(|fault| {
                        let expression = match set {
                            Set::Value(..) => at.field("value"),
                            _ => field_at(at.field(set.field()).index(fault.coordinate)),
                        };
                        outright.fault_error(fault, expression)
                    }),
            );
            let object_id = set.object_id();
            if !matches!(set, Set::Value(..))
                && let Err(error) =
                    self.guard_object(object_id, start, &field_at(at.field("object")))
            {
                errors.push(error);
                continue;
            }
            if matches!(set, Set::Position(..)) {
                errors.extend(self.unplaced_error(&outright, object_id, start, span, |k| {
                    field_at(at.field(set.field()).index(k))
                }));
            }
            let on = OutrightOn {
                outright,
                span,
                animation: field_at(at.field(set.field())),
                jump_at: field_at(at.field(set.field())),
            };
            let tracks = self.objects.entry(object_id.clone()).or_default();
            let declared = &start.objects[object_id];
            errors.extend(match set {
                Set::Position(..) => on.push(
                    &mut tracks.position,
                    format!("the position of `{object_id}`"),
                    declared.transform.position,
                ),
                Set::Rotation(..) => on.push(
                    &mut tracks.rotation,
                    format!("the rotation of `{object_id}`"),
                    declared.transform.rotation,
                ),
                Set::Value(..) => {
                    let start_value = declared
                        .readout
                        .as_ref()
                        .expect("`scene_file` checked that the object is a readout")
                        .value;
                    on.push(
                        &mut tracks.value,
                        format!("the value of `{object_id}`"),
                        start_value,
                    )
                }
            });
        }
        errors
    }

    /// Where each of a simulation's bodies stands when its span starts, as the timeline so far
    /// leaves it, or the `invalid` error at each body that is scaled or turned about x or y
    /// then, since a body moves in the world's xy plane at its drawn size. None for a simulation
    /// by rates.
    fn placed_bodies(
        &self,
        simulate: &Simulate,
        span: (f64, f64),
        start: &Stage,
        path: &JsonPath,
    ) -> Result<Vec<Placed>, Vec<SceneError>> {
        let Motion::Bodies(bodies) = &simulate.motion else {
            return Ok(Vec::new());
        };
        let stage = self.stage(start, span.0, Staging::BeforeTrails);
        let mut errors = Vec::new();
        let placed = bodies
            .bodies
            .iter()
            .map(|body| {
                let id = &body.object_id;
                let Transform {
                    position,
                    rotation,
                    scale,
                } = stage.objects[id].transform;
                if rotation.x != 0.0 || rotation.y != 0.0 || scale.x != 1.0 || scale.y != 1.0 {
                    let message = format!(
                        "`{id}` is turned by {rotation} and scaled by {scale} when the simulation starts, and a body moves in the world's xy plane at the size it is drawn: turned only about z, at scale 1; draw it at its size instead"
                    );
                    errors.push(SceneError::new(
                        path.field("bodies").field(&id.to_string()),
                        SceneErrorCode::Invalid,
                        message,
                    ));
                }
                Placed {
                    position: position.truncate().as_dvec2(),
                    angle: f64::from(rotation.z),
                }
            })
            .collect();
        if errors.is_empty() {
            Ok(placed)
        } else {
            Err(errors)
        }
    }

    /// Adds a follow of a shape to the object's shape track. Its errors: `unknown_chapter` at
    /// `from`; each fault the shape's expressions have at a frame time of the span or at its end,
    /// at the first such scene time, at the expression under the object, as `out_of_range` for a
    /// value that is not finite; the track's `conflicting_animations`; and `law_jump` at the
    /// follow when its law time does not start at the one the shape is drawn at.
    fn push_shape(
        &mut self,
        object_id: &ObjectId,
        law_time_start: &LawTimeStart,
        span: (f64, f64),
        start: &Stage,
        path: &JsonPath,
    ) -> Vec<SceneError> {
        let law_start = match self.law_start(law_time_start, span, path) {
            Ok(law_start) => law_start,
            Err(error) => return vec![error],
        };
        let law = start.objects[object_id]
            .shape_law
            .as_ref()
            .expect("`scene_file` checked that the shape reads `t`");
        let mut errors: Vec<SceneError> = Vec::new();
        let mut sampling: Option<Sampling> = None;
        let frame_times: Vec<(f64, f64)> = frame_times_in(span, self.fps)
            .map(|scene_time| (scene_time, scene_time - law_start))
            .collect();
        for &(scene_time, law_time) in &frame_times {
            match law.at(law_time) {
                Ok(shape) => {
                    if let Some(at) = ShapeLaw::sampling_of(&shape) {
                        sampling = Some(
                            sampling.map_or_else(|| at.clone(), |sampling| sampling.union(at)),
                        );
                    }
                }
                Err(faults) => shape_faults(object_id, scene_time, law_time, faults, &mut errors),
            }
        }
        if let Some(sampling) = sampling {
            match self.shape_samplings.get_mut(object_id) {
                Some(known) => {
                    known.sampling = known.sampling.union(&sampling);
                    known.frame_times.extend(frame_times);
                }
                None => {
                    _ = self.shape_samplings.insert(
                        object_id.clone(),
                        ShapeSampling {
                            sampling,
                            frame_times,
                        },
                    )
                }
            }
        }
        let value = format!("the shape of `{object_id}`");
        let track = &mut self.objects.entry(object_id.clone()).or_default().shape;
        let held = track.at(span.0).unwrap_or(0.0);
        if let Err(conflict) = track.push(
            &value,
            span,
            0.0,
            span.1 - law_start,
            SetBy::LawTime {
                law_start,
                value: |law_time| law_time,
            },
            path.clone(),
        ) {
            errors.push(conflict);
            return errors;
        }
        let law_time = format!("the law time of the shape of `{object_id}`");
        errors.extend(law_jump::<f64>(
            (span.0 - law_start).numbers(),
            held.numbers(),
            &law_time,
            span.0,
            law_start,
            &JumpWording::SHAPE,
            path,
        ));
        errors
    }

    /// Adds a carry to the position track of the object it carries, and to its rotation track
    /// when it faces the tangent. Its errors: `invalid` at `along` for an `along` object that is
    /// carried or morphed, or for a carried object another carry goes along; the tracks'
    /// `conflicting_animations`; and `carry_jump` at `along` or `faces` when the curve does not
    /// start where the object stands or turns it another way, with the value to declare.
    #[allow(clippy::too_many_arguments)]
    fn push_carry(
        &mut self,
        object_id: &ObjectId,
        carry: &Arc<Carry>,
        span: (f64, f64),
        start: &Stage,
        easing_curve: EasingCurve,
        path: &JsonPath,
    ) -> Vec<SceneError> {
        let along = &carry.along;
        let along_axes = match &start.objects[along].placed_by {
            PlacedBy::Axes(axes) => Some(axes.clone()),
            _ => None,
        };
        let refuse = |message: String| {
            vec![SceneError::new(
                path.field("along"),
                SceneErrorCode::Invalid,
                message,
            )]
        };
        for (carried, earlier_along, earlier_axes, earlier) in &self.carries {
            if carried == along || Some(carried) == along_axes.as_ref() {
                return refuse(format!(
                    "`{along}` is carried by the follow at {earlier}, and a carry goes along an object that is not carried itself; write the motion of `{object_id}` as a law instead"
                ));
            }
            if earlier_along == object_id || earlier_axes.as_ref() == Some(object_id) {
                return refuse(format!(
                    "the follow at {earlier} carries an object along `{object_id}`, and a carried object is not what another carry goes along; write the motion of `{object_id}` as a law instead"
                ));
            }
        }
        if let Some(morph) = self.morph_of(along) {
            return refuse(format!(
                "`{along}` is reshaped by the morph at {morph}, and a carry goes along the curve an object declares; carry `{object_id}` along an object no morph reshapes"
            ));
        }
        let (before, after) = (self.stage_at(start, span.0), self.stage_at(start, span.1));
        let declared = start.objects[object_id].transform;
        let tracks = self.objects.entry(object_id.clone()).or_default();
        let held = Transform {
            position: tracks.position.at(span.0).unwrap_or(declared.position),
            rotation: tracks.rotation.at(span.0).unwrap_or(declared.rotation),
            ..declared
        };
        let in_data = |stage: &Stage, point: follow::CarryPoint| follow::CarryPoint {
            position: data_of(stage, object_id, point.position).unwrap_or(point.position),
            ..point
        };
        let first = in_data(
            &before,
            carry.at(0.0, along_to_world(&before, along), held.rotation),
        );
        let last = in_data(
            &after,
            carry.at(1.0, along_to_world(&after, along), held.rotation),
        );
        let set_by = || SetBy::Carry {
            carry: Arc::clone(carry),
            easing_curve,
            reference: held.rotation,
        };
        let value = |what: &str| format!("the {what} of `{object_id}`");
        let mut errors: Vec<SceneError> = tracks
            .position
            .push(
                &value("position"),
                span,
                declared.position,
                last.position,
                set_by(),
                path.clone(),
            )
            .err()
            .into_iter()
            .collect();
        if let Some(rotation) = last.rotation {
            errors.extend(
                tracks
                    .rotation
                    .push(
                        &value("rotation"),
                        span,
                        declared.rotation,
                        rotation,
                        set_by(),
                        path.clone(),
                    )
                    .err(),
            );
        }
        if !errors.is_empty() {
            return errors;
        }
        self.carries
            .push((object_id.clone(), along.clone(), along_axes, path.clone()));
        let to_triple = |v: Vec3| follow::triple(v.to_array().map(f64::from));
        if f64::from(first.position.distance(held.position)) > follow::LEAST_JUMP {
            let message = format!(
                "`{along}`'s curve starts at {} at scene time {}, and `{object_id}` stands at {} there; a carry sets the position outright, so declare `{object_id}`'s `position` as {}, or move it there before this follow",
                to_triple(first.position),
                follow::short(span.0),
                to_triple(held.position),
                to_triple(first.position)
            );
            errors.push(SceneError::new(
                path.field("along"),
                SceneErrorCode::CarryJump,
                message,
            ));
        }
        if let Some(rotation) = first
            .rotation
            .filter(|rotation| f64::from(rotation.distance(held.rotation)) > follow::LEAST_JUMP)
        {
            let message = format!(
                "facing the tangent at the start of `{along}`'s curve turns `{object_id}` to {} at scene time {}, and it is turned to {} there; declare `{object_id}`'s `rotation` as {}, or rotate it there before this follow",
                to_triple(rotation),
                follow::short(span.0),
                to_triple(held.rotation),
                to_triple(rotation)
            );
            errors.push(SceneError::new(
                path.field("faces"),
                SceneErrorCode::CarryJump,
                message,
            ));
        }
        errors
    }

    /// The `out_of_range` error at the first frame time of `span` at which `outright`, setting
    /// the position of the data point `object_id`, gives it a coordinate that a log axis of its
    /// axes cannot place, a value at or below 0, or `None`. `expression` gives the path of the
    /// expression of a coordinate, 0 for x and 1 for y.
    fn unplaced_error(
        &self,
        outright: &Outright,
        object_id: &ObjectId,
        start: &Stage,
        span: (f64, f64),
        expression: impl Fn(usize) -> JsonPath,
    ) -> Option<SceneError> {
        let PlacedBy::DataPoint(axes_id) = &start.objects[object_id].placed_by else {
            return None;
        };
        let on = DataToWorld::of(&start.objects[axes_id]);
        let (scene_time, axis, value) = frame_times_in(span, self.fps).find_map(|scene_time| {
            let position = Vec3::from_array(outright.at(scene_time, [1.0; 3]).map(|v| v as f32));
            on.unplaced(position)
                .map(|(axis, value)| (scene_time, axis, value))
        })?;
        let message = format!(
            "`{axis}` reaches {} at scene time {} (law time {}), and the {axis} axis of `{axes_id}` is a log axis, which places only values above 0; change the expression so it stays above 0",
            follow::short(value),
            follow::short(scene_time),
            follow::short(scene_time - outright.law_start())
        );
        let coordinate = usize::from(axis == "y");
        Some(SceneError::new(
            expression(coordinate),
            SceneErrorCode::OutOfRange,
            message,
        ))
    }

    /// Adds a law to its value's track. Its errors: `unknown_chapter` at `from` for a chapter that
    /// has not begun; `out_of_range` at each expression with no finite value, a position's
    /// past `follow::FARTHEST`, or an object opacity's outside 0 to 1, at a frame time of the
    /// span or at its end; the track's
    /// `conflicting_animations`; and `law_jump` at `law` when the law does not start where the
    /// value is.
    #[allow(clippy::too_many_arguments)]
    fn push_law(
        &mut self,
        followed: &Followed,
        law: &Arc<Law>,
        law_time_start: &LawTimeStart,
        span: (f64, f64),
        start: &Stage,
        path: &JsonPath,
    ) -> Vec<SceneError> {
        let law_start = match self.law_start(law_time_start, span, path) {
            Ok(law_start) => law_start,
            Err(error) => return vec![error],
        };
        let outright = Outright::Law {
            law: Arc::clone(law),
            law_start,
        };
        let mut errors: Vec<SceneError> = outright
            .faults(frame_times_in(span, self.fps))
            .iter()
            .map(|fault| {
                let field = if matches!(followed, Followed::Value(_) | Followed::Opacity(_)) {
                    "value"
                } else {
                    ["x", "y", "z"][fault.coordinate]
                };
                outright.fault_error(fault, path.field("law").field(field))
            })
            .collect();
        if let Followed::Position(id) = followed {
            errors.extend(self.unplaced_error(&outright, id, start, span, |k| {
                path.field("law").field(["x", "y"][k])
            }));
        }
        let on = OutrightOn {
            outright,
            span,
            animation: path.clone(),
            jump_at: path.field("law"),
        };
        errors.extend(match followed {
            Followed::Position(id) => on.push(
                &mut self.objects.entry(id.clone()).or_default().position,
                format!("the position of `{id}`"),
                start.objects[id].transform.position,
            ),
            Followed::Rotation(id) => on.push(
                &mut self.objects.entry(id.clone()).or_default().rotation,
                format!("the rotation of `{id}`"),
                start.objects[id].transform.rotation,
            ),
            Followed::Scale(id) => on.push(
                &mut self.objects.entry(id.clone()).or_default().scale,
                format!("the scale of `{id}`"),
                start.objects[id].transform.scale,
            ),
            Followed::Opacity(id) => on.push(
                &mut self.objects.entry(id.clone()).or_default().opacity,
                format!("the opacity of `{id}`"),
                start.objects[id].opacity.get(),
            ),
            Followed::CameraPosition => on.push(
                &mut self.camera.position,
                "the position of the camera".to_string(),
                start.camera.position,
            ),
            Followed::CameraTarget => on.push(
                &mut self.camera.target,
                "the target of the camera".to_string(),
                start.camera.target,
            ),
            Followed::Value(id) => {
                let start_value = start.objects[id]
                    .readout
                    .as_ref()
                    .expect("`scene_file` checked that the object is a readout")
                    .value;
                on.push(
                    &mut self.objects.entry(id.clone()).or_default().value,
                    format!("the value of `{id}`"),
                    start_value,
                )
            }
        });
        errors
    }
}

/// Each of a shape law's `faults` at a frame time, at its expression under the object, unless
/// `errors` already has one there: so each expression reports its first bad frame time.
fn shape_faults(
    object_id: &ObjectId,
    scene_time: f64,
    law_time: f64,
    faults: Vec<ShapeFault>,
    errors: &mut Vec<SceneError>,
) {
    let object = JsonPath::root()
        .field("objects")
        .field(&object_id.to_string());
    for (field, code, message) in faults {
        let at = field
            .split('/')
            .fold(object.clone(), |path, name| path.field(name));
        if !errors.iter().any(|error| error.path == at) {
            let message = format!(
                "at scene time {} (law time {}), {message}",
                follow::short(scene_time),
                follow::short(law_time)
            );
            errors.push(SceneError::new(at, code, message));
        }
    }
}

impl Timeline {
    /// Where a simulation of a circuit starts: where the circuit's last simulation left it, or as
    /// declared. Its errors: `conflicting_animations` while another simulation of the circuit
    /// runs; `out_of_range` at a switching after the span; and `invalid` at `switches` for a
    /// switching that makes a loop with no resistance, or a time constant shorter than the
    /// simulation's step can follow.
    fn circuit_start(
        &self,
        motion: &CircuitMotion,
        span: (f64, f64),
        path: &JsonPath,
    ) -> Result<Vec<f64>, Vec<SceneError>> {
        let last = self
            .circuits
            .get(&motion.object)
            .and_then(|circuit| circuit.runs.last());
        if let Some(last) = last
            && last.span().1 > span.0 + 1e-9
        {
            let message = format!(
                "another simulation of the circuit `{}` runs until scene time {}, and a circuit is solved by one simulation at a time; start this one after it",
                motion.object,
                follow::short(last.span().1)
            );
            return Err(vec![SceneError::new(
                path.field("circuit"),
                SceneErrorCode::ConflictingAnimations,
                message,
            )]);
        }
        let network = &motion.network;
        let state = match last {
            Some(last) => motion.state(&last.variables_at(last.span().1)).to_vec(),
            None => network.rest(),
        };
        let length = span.1 - span.0;
        let mut errors = Vec::new();
        let step = 1.0 / (self.fps * crate::simulate::STEPS_PER_FRAME);
        let mut configuration = state.clone();
        let check = |configuration: &[f64], when: &str, errors: &mut Vec<SceneError>| {
            let closed: Vec<bool> = (0..network.components.len())
                .map(|i| {
                    network.components[i].kind == circuit::Kind::Switch
                        && configuration[3 * i + 2] == 1.0
                })
                .collect();
            if let Some((_, message)) = network.loop_fault(&closed) {
                errors.push(SceneError::new(
                    path.field("switches"),
                    SceneErrorCode::Invalid,
                    format!("{when}, {message}"),
                ));
            } else if let Some((i, tau)) = network.shortest_time_constant(configuration)
                && tau < 4.0 * step
            {
                let message = format!(
                    "{when}, {} has a time constant of {} s, and a simulation steps every {} s at this frame rate, too coarse to follow it; give it a larger value or more resistance, so its time constant is at least {} s",
                    circuit::describe(&network.components[i], i),
                    follow::short(tau),
                    follow::short(step),
                    follow::short(4.0 * step)
                );
                errors.push(SceneError::new(
                    path.clone(),
                    SceneErrorCode::OutOfRange,
                    message,
                ));
            }
        };
        check(&configuration, "when the simulation starts", &mut errors);
        for (k, switching) in motion.switchings.iter().enumerate() {
            if switching.at > length + 1e-9 {
                let message = format!(
                    "a switching at {} s falls after the span ends at {} s; set it within the span",
                    follow::short(switching.at),
                    follow::short(length)
                );
                errors.push(SceneError::new(
                    path.field("switches"),
                    SceneErrorCode::OutOfRange,
                    message,
                ));
            }
            configuration[3 * switching.component + 2] = f64::from(u8::from(switching.closed));
            let id = network.components[switching.component]
                .id
                .as_deref()
                .unwrap_or_default();
            let verb = if switching.closed { "closes" } else { "opens" };
            if motion.switchings[k + 1..]
                .iter()
                .all(|s| s.at != switching.at)
            {
                check(
                    &configuration,
                    &format!("when `{id}` {verb} at {} s", follow::short(switching.at)),
                    &mut errors,
                );
            }
        }
        if errors.is_empty() {
            Ok(state)
        } else {
            Err(errors)
        }
    }

    /// Keeps a simulation of a circuit for the stages after it, with what it reports and the most
    /// power its bulbs draw.
    fn push_circuit_run(
        &mut self,
        motion: &CircuitMotion,
        simulation: &Arc<Simulation>,
        span: (f64, f64),
        path: &JsonPath,
    ) {
        let bulb_powers: Vec<usize> = motion
            .quantities
            .iter()
            .enumerate()
            .filter(|(_, (i, q))| {
                *q == circuit::Quantity::Power
                    && matches!(
                        motion.network.components[*i].kind,
                        circuit::Kind::Bulb { .. }
                    )
            })
            .map(|(k, _)| k)
            .collect();
        let most = simulation
            .kept()
            .flat_map(|values| bulb_powers.iter().map(move |&k| values[k]))
            .fold(0.0, f64::max);
        let named = |values: Vec<f64>| -> BTreeMap<String, f64> {
            let names = &simulation.names();
            motion
                .quantities
                .iter()
                .enumerate()
                .map(|(k, _)| (names[k].clone(), values[k]))
                .collect()
        };
        self.circuit_reports.push(CircuitRunReport {
            circuit: motion.object.clone(),
            path: path.clone(),
            start: span.0,
            end: span.1,
            at_start: named(simulation.variables_at(span.0)),
            at_end: named(simulation.variables_at(span.1)),
        });
        let runs = self.circuits.entry(motion.object.clone()).or_default();
        runs.full_power = runs.full_power.max(most);
        runs.runs.push(Arc::clone(simulation));
    }
}
