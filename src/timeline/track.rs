//! Tracks: one value's changes over scene time, each a segment set by an easing curve, a law or a
//! simulation's set, a carry or a law time, and the values an animation can carry.

use super::*;

/// A value an animation can carry from a start value to an end value.
pub trait Mix: Clone {
    fn mix(self, to: Self, progress: Progress) -> Self;
}

impl Mix for f32 {
    fn mix(self, to: f32, progress: Progress) -> f32 {
        self + (to - self) * progress.0
    }
}

/// In linear light, as the renderer blends, so a recolor passes through no darker color. Past
/// 1, as under an overshoot, the color stops at its end.
impl Mix for Color {
    fn mix(self, to: Color, progress: Progress) -> Color {
        Color::mix(self, to, progress.0.clamp(0.0, 1.0))
    }
}

/// One color per glyph, each mixed with its own.
impl Mix for Vec<Color> {
    fn mix(self, to: Vec<Color>, progress: Progress) -> Vec<Color> {
        self.into_iter()
            .zip(to)
            .map(|(from, to)| Mix::mix(from, to, progress))
            .collect()
    }
}

/// Law time, which a shape's track holds, and a readout's value.
impl Mix for f64 {
    fn mix(self, to: f64, progress: Progress) -> f64 {
        self + (to - self) * f64::from(progress.0)
    }
}

impl Mix for Vec3 {
    fn mix(self, to: Vec3, progress: Progress) -> Vec3 {
        self + (to - self) * progress.0
    }
}

/// A plane map as its track holds it. A segment's end carries the linear map that takes the
/// segment's start there, so a mix follows that map's matrix route; a plane map mixed entry by
/// entry would be the wrong route for a turn.
#[derive(Clone, Debug)]
pub(super) struct MapValue {
    pub(super) plane_map: PlaneMap,
    pub(super) carried_by: Option<Arc<LinearMap>>,
}

impl MapValue {
    pub(super) const IDENTITY: MapValue = MapValue {
        plane_map: PlaneMap::IDENTITY,
        carried_by: None,
    };
}

impl Mix for MapValue {
    fn mix(self, to: MapValue, progress: Progress) -> MapValue {
        match &to.carried_by {
            Some(linear_map) => MapValue {
                plane_map: self.plane_map.then(linear_map.at(progress.0)),
                carried_by: None,
            },
            None => to,
        }
    }
}

/// How a segment takes its value from its `from` to its `to`, named as `PlacedBy` is: by what.
#[derive(Debug)]
pub(super) enum SetBy<V> {
    /// `from` mixed toward `to` by the easing curve's progress.
    EasingCurve(EasingCurve),
    /// What a law or a simulation's set gives at each scene time, from `held`, the value when it
    /// began, and `to` is what it gives at the span's end, so the track holds exactly that from
    /// then on. A plain `fn` turns its three numbers into the track's value, which keeps
    /// `Segment` generic and `Debug`.
    Outright {
        outright: Outright,
        held: [f64; 3],
        value: fn([f64; 3]) -> V,
    },
    /// Resolved by `Timeline::stage_at` in its carried pass, because the value depends on where
    /// the `along` object is at that scene time. While it plays `Track::at` gives `to`, the
    /// value at the span's end, which only the check of an animation that already overlaps it
    /// reads. `reference` is the rotation held when the carry began.
    Carry {
        carry: Arc<Carry>,
        easing_curve: EasingCurve,
        reference: Vec3,
    },
    /// The law time at each scene time, counted from `law_start`: a follow of a shape.
    LawTime { law_start: f64, value: fn(f64) -> V },
}

impl<V> From<EasingCurve> for SetBy<V> {
    fn from(easing_curve: EasingCurve) -> SetBy<V> {
        SetBy::EasingCurve(easing_curve)
    }
}

/// A value a law sets: three numbers, a position's, a rotation's or a scale's, or one, a
/// readout's value or an object opacity.
pub(super) trait LawValue: Mix + Copy {
    fn numbers(self) -> [f64; 3];
    fn from_numbers(numbers: [f64; 3]) -> Self;
    /// The value as a scene file writes it.
    fn written(numbers: [f64; 3]) -> String;
}

impl LawValue for Vec3 {
    fn numbers(self) -> [f64; 3] {
        self.to_array().map(f64::from)
    }

    fn from_numbers([x, y, z]: [f64; 3]) -> Vec3 {
        Vec3::new(x as f32, y as f32, z as f32)
    }

    fn written(numbers: [f64; 3]) -> String {
        follow::triple(numbers)
    }
}

impl LawValue for f32 {
    fn numbers(self) -> [f64; 3] {
        [f64::from(self), 0.0, 0.0]
    }

    fn from_numbers([value, _, _]: [f64; 3]) -> f32 {
        value as f32
    }

    fn written([value, _, _]: [f64; 3]) -> String {
        follow::short(value)
    }
}

impl LawValue for f64 {
    fn numbers(self) -> [f64; 3] {
        [self, 0.0, 0.0]
    }

    fn from_numbers([value, _, _]: [f64; 3]) -> f64 {
        value
    }

    fn written([value, _, _]: [f64; 3]) -> String {
        follow::short(value)
    }
}

/// A law or a simulation's set about to go on the track of the value it sets.
pub(super) struct OutrightOn {
    pub(super) outright: Outright,
    pub(super) span: (f64, f64),
    /// The animation, which a conflicting animation names.
    pub(super) animation: JsonPath,
    /// Where `law_jump` is reported: a law's `law`, or a set's expressions.
    pub(super) jump_at: JsonPath,
}

impl OutrightOn {
    /// Puts it on `track`, starting from where the track holds the value, or `start_value`, and
    /// names the value `value` in a message. Its errors: the track's `conflicting_animations`,
    /// and `law_jump` when it does not start where the value is.
    pub(super) fn push<V: LawValue>(
        self,
        track: &mut Track<V>,
        value: String,
        start_value: V,
    ) -> Vec<SceneError> {
        let OutrightOn {
            outright,
            span,
            animation,
            jump_at,
        } = self;
        let held = track.at(span.0).unwrap_or(start_value).numbers();
        let first = outright.at(span.0, held);
        let to = V::from_numbers(outright.at(span.1, held));
        let wording = match outright {
            Outright::Law { .. } => &JumpWording::LAW,
            Outright::Set { .. } => &JumpWording::SIMULATION,
        };
        let law_start = outright.law_start();
        if let Err(conflict) = track.push(
            &value,
            span,
            start_value,
            to,
            SetBy::Outright {
                outright,
                held,
                value: V::from_numbers,
            },
            animation,
        ) {
            return vec![conflict];
        }
        law_jump::<V>(first, held, &value, span.0, law_start, wording, &jump_at)
            .into_iter()
            .collect()
    }
}

/// How a `law_jump` message names what set the value and what to do instead.
pub(super) struct JumpWording {
    pub(super) gives: &'static str,
    pub(super) outright: &'static str,
    pub(super) fix: &'static str,
    pub(super) when: &'static str,
}

impl JumpWording {
    const LAW: JumpWording = JumpWording {
        gives: "the law gives",
        outright: "a follow sets its value outright, so its law must start within",
        fix: "change the law, set `from` to continue an earlier follow's law time, or bring the value to",
        when: "before this follow",
    };
    pub(super) const SHAPE: JumpWording = JumpWording {
        gives: "this follow draws its shape at law time",
        outright: "a follow of a shape sets its law time outright, so it must start within",
        fix: "set `from` so law time goes on from where the shape was, leave it out to start at 0, or end an earlier follow of the shape at law time",
        when: "before this follow",
    };
    const SIMULATION: JumpWording = JumpWording {
        gives: "this set gives",
        outright: "a simulation sets its values outright, so each set must start within",
        fix: "change the starting values or the expression, or bring the value to",
        when: "before this simulation",
    };
}

/// `law_jump` at `at` when `first`, what a law or a simulation gives at `begins`, lies farther
/// than `follow::LEAST_JUMP` from `held`, the value there.
pub(super) fn law_jump<V: LawValue>(
    first: [f64; 3],
    held: [f64; 3],
    value: &str,
    begins: f64,
    law_start: f64,
    wording: &JumpWording,
    at: &JsonPath,
) -> Option<SceneError> {
    let apart = first
        .iter()
        .zip(held)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f64>()
        .sqrt();
    let JumpWording {
        gives,
        outright,
        fix,
        when,
    } = wording;
    (apart > follow::LEAST_JUMP).then(|| {
        let message = format!(
            "at scene time {} (law time {}) {gives} {}, and {value} is {} there; {outright} {} of it: {fix} {} {when}",
            follow::short(begins),
            follow::short(begins - law_start),
            V::written(first),
            V::written(held),
            follow::LEAST_JUMP,
            V::written(first),
        );
        SceneError::new(at.clone(), SceneErrorCode::LawJump, message)
    })
}

/// One animation's hold on one value, from `start` to `end` in scene time.
#[derive(Debug)]
pub(super) struct Segment<V> {
    pub(super) start: f64,
    pub(super) end: f64,
    /// The value before the segment starts: the previous segment's `to`, or the start stage's.
    pub(super) from: V,
    /// The value from the segment's end on.
    pub(super) to: V,
    pub(super) set_by: SetBy<V>,
    /// The animation that made this segment, so a second animation of the same value in the same
    /// step can name it.
    pub(super) animation: JsonPath,
}

/// Every change to one value, in scene time order, never overlapping.
#[derive(Debug)]
pub(super) struct Track<V> {
    pub(super) segments: Vec<Segment<V>>,
}

impl<V> Default for Track<V> {
    fn default() -> Self {
        Track {
            segments: Vec::new(),
        }
    }
}

impl<V: Mix> Track<V> {
    /// The value at a scene time, or `None` until the first segment starts: a member's own
    /// color, which its object's colors hold until its own recolor begins.
    pub(super) fn since_first(&self, scene_time: f64) -> Option<V> {
        self.segments.first().filter(|s| s.start <= scene_time)?;
        self.at(scene_time)
    }

    /// The value at a scene time, or `None` for a track with no segments, where the start
    /// stage's value holds. Before the first segment its start value holds, which is the start
    /// stage's value for every track but a created share's, whose first segment starts from 0.
    /// After a segment ends its end value holds until the next one starts.
    pub(super) fn at(&self, scene_time: f64) -> Option<V> {
        let after = self.segments.partition_point(|s| s.start <= scene_time);
        let segment = self.segments.get(after.saturating_sub(1))?;
        if scene_time < segment.start {
            return Some(segment.from.clone());
        }
        if scene_time >= segment.end {
            return Some(segment.to.clone());
        }
        let share =
            ElapsedShare(((scene_time - segment.start) / (segment.end - segment.start)) as f32);
        match &segment.set_by {
            SetBy::EasingCurve(easing_curve) => Some(
                segment
                    .from
                    .clone()
                    .mix(segment.to.clone(), easing_curve.progress(share)),
            ),
            SetBy::Outright {
                outright,
                held,
                value,
            } => Some(value(outright.at(scene_time, *held))),
            SetBy::Carry { .. } => Some(segment.to.clone()),
            SetBy::LawTime { law_start, value } => Some(value(scene_time - law_start)),
        }
    }

    /// The carry playing at a scene time, from its span's start up to but not at its end, with
    /// its progress clamped to 0 to 1, so an overshoot waits at the curve's end.
    pub(super) fn carry_at(&self, scene_time: f64) -> Option<(&Carry, f32, Vec3)> {
        let after = self.segments.partition_point(|s| s.start <= scene_time);
        let segment = self
            .segments
            .get(after.checked_sub(1)?)
            .filter(|segment| scene_time < segment.end)?;
        let SetBy::Carry {
            carry,
            easing_curve,
            reference,
        } = &segment.set_by
        else {
            return None;
        };
        let share =
            ElapsedShare(((scene_time - segment.start) / (segment.end - segment.start)) as f32);
        Some((
            carry,
            easing_curve.progress(share).get().clamp(0.0, 1.0),
            *reference,
        ))
    }

    /// Adds a change from `start` to `end`, starting from wherever the track's last change left
    /// the value, or from `start_value` if it has none. Two animations in one step that change
    /// the same value overlap, which is a scene error at the second.
    /// `value` names the value for a message, such as "the position of `orb`".
    pub(super) fn push(
        &mut self,
        value: &str,
        step: (f64, f64),
        start_value: V,
        to: V,
        set_by: impl Into<SetBy<V>>,
        animation: JsonPath,
    ) -> Result<(), SceneError> {
        if let Some(last) = self.segments.last()
            && last.end > step.0
        {
            let message = format!(
                "this animation and the one at {} both change {value} at once; give them spans one after the other with `start` and `end`, or put them in separate steps",
                last.animation
            );
            return Err(SceneError::new(
                animation,
                SceneErrorCode::ConflictingAnimations,
                message,
            ));
        }
        let from = self
            .segments
            .last()
            .map_or(start_value, |last| last.to.clone());
        self.segments.push(Segment {
            start: step.0,
            end: step.1,
            from,
            to,
            set_by: set_by.into(),
            animation,
        });
        Ok(())
    }
}
