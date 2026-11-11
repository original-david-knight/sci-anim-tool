//! A simulation: an animation that integrates its variables over its span, and sets positions,
//! rotations and readouts' values from expressions in the variables. It is integrated once,
//! when the scene file is read, and its variables are kept, so a frame stays a pure function of
//! the scene and the scene time.
//!
//! Its variables move one of two ways. By their rates: fourth-order Runge-Kutta at a fixed step
//! of a fraction of a frame, every step kept, and the stage at any scene time takes the step
//! before it and integrates the rest of the way. Or as bodies, which `body` steps under gravity
//! and their contacts, its variables each moving body's quantities, kept at every frame and
//! drawn straight between two.

use crate::body::{Bodies, Collision, Placed};
use crate::expr::Expression;
use crate::follow::{LawKind, LawTimeStart};
use crate::scene::ObjectId;

/// How many integration steps each frame of the span is cut into.
pub const STEPS_PER_FRAME: f64 = 8.0;

/// How many steps bodies take a frame: a collision closes no more than this share of a frame's
/// travel before it is settled.
const BODY_STEPS_PER_FRAME: usize = 16;

/// The most frames a simulation spans, an hour at 60 frames a second, which bounds the steps it
/// integrates and keeps when the scene file is read.
pub const MOST_FRAMES: f64 = 216_000.0;

/// A simulation as the scene file writes it, read and checked for form.
#[derive(Debug)]
pub struct Simulate {
    /// The variables' names, in the order their values are kept and the expressions read them,
    /// then `t`: the scene file's variables, or each moving body's quantities as
    /// `<object id>.<quantity>`.
    pub variables: Vec<String>,
    pub motion: Motion,
    /// Each moving body's position and rotation first, then the scene file's sets.
    pub sets: Vec<Set>,
    pub law_time_start: LawTimeStart,
}

/// How a simulation's variables move.
#[derive(Debug)]
pub enum Motion {
    /// From each variable's value at the start of the span, by how fast it changes, per second
    /// of law time, in the variables and `t`.
    Rates {
        starts: Vec<f64>,
        rates: Vec<Expression>,
    },
    Bodies(Bodies),
    /// A circuit, stepped by `CircuitMotion::advance` from where its last simulation left it.
    Circuit(crate::circuit::CircuitMotion),
}

impl Simulate {
    /// How many of `sets` are bodies' own, before the scene file's.
    pub fn body_sets(&self) -> usize {
        match &self.motion {
            Motion::Rates { .. } => 0,
            Motion::Bodies(bodies) => 2 * bodies.moving().count(),
            Motion::Circuit(_) => 0,
        }
    }
}

/// One value a simulation sets, from expressions in its variables and `t`.
#[derive(Debug)]
pub enum Set {
    Position(ObjectId, [Expression; 3]),
    Rotation(ObjectId, [Expression; 3]),
    /// A readout's value.
    Value(ObjectId, Expression),
}

impl Set {
    pub fn object_id(&self) -> &ObjectId {
        match self {
            Set::Position(object_id, _)
            | Set::Rotation(object_id, _)
            | Set::Value(object_id, _) => object_id,
        }
    }

    /// What the set's numbers are, which decides how far they may reach.
    pub fn kind(&self) -> LawKind {
        match self {
            Set::Position(..) => LawKind::Position,
            Set::Rotation(..) => LawKind::Rotation,
            Set::Value(..) => LawKind::Value,
        }
    }

    /// The field of the set's expressions under its entry of `sets`.
    pub fn field(&self) -> &'static str {
        match self {
            Set::Position(..) => "position",
            Set::Rotation(..) => "rotation",
            Set::Value(..) => "value",
        }
    }
}

/// A simulation integrated over its span: its variables at every kept step.
#[derive(Debug)]
pub struct Simulation {
    simulate: std::sync::Arc<Simulate>,
    /// The span in scene time.
    span: (f64, f64),
    /// The scene time law time counts from.
    law_start: f64,
    /// The scene time between two kept steps.
    step: f64,
    /// The variables at kept step `k`, `k` from 0 to the count, one after another.
    steps: Vec<f64>,
    /// Every collision of its bodies, in scene time order.
    pub collisions: Vec<Collision>,
}

/// A variable that has no finite value at a step of the span.
#[derive(Debug, PartialEq)]
pub struct NoValue {
    pub variable: usize,
    pub scene_time: f64,
    pub law_time: f64,
}

impl Simulation {
    /// Integrates `simulate` over `span`, at `STEPS_PER_FRAME` steps a frame of `fps`, or its
    /// bodies from `placed`, one for each body, at `BODY_STEPS_PER_FRAME`, with law time counting
    /// from `law_start`. The step is the span cut evenly, so the last step ends exactly at the
    /// span's end.
    pub fn integrate(
        simulate: &std::sync::Arc<Simulate>,
        span: (f64, f64),
        law_start: f64,
        fps: f64,
        placed: &[Placed],
        circuit_state: &[f64],
    ) -> Result<Simulation, NoValue> {
        let simulation = |step: f64, steps: Vec<f64>, collisions: Vec<Collision>| Simulation {
            simulate: std::sync::Arc::clone(simulate),
            span,
            law_start,
            step,
            steps,
            collisions,
        };
        match &simulate.motion {
            Motion::Rates { starts, .. } => {
                let count = ((span.1 - span.0) * fps * STEPS_PER_FRAME).ceil().max(1.0) as usize;
                let step = (span.1 - span.0) / count as f64;
                let mut integrated = simulation(step, starts.clone(), Vec::new());
                let size = starts.len();
                for k in 0..count {
                    let scene_time = span.0 + step * k as f64;
                    let values = integrated.steps[k * size..(k + 1) * size].to_vec();
                    let next = integrated.advance(&values, scene_time, step);
                    if let Some(variable) = next.iter().position(|value| !value.is_finite()) {
                        let scene_time = span.0 + step * (k + 1) as f64;
                        return Err(NoValue {
                            variable,
                            scene_time,
                            law_time: scene_time - law_start,
                        });
                    }
                    integrated.steps.extend(next);
                }
                Ok(integrated)
            }
            Motion::Circuit(motion) => {
                let count = ((span.1 - span.0) * fps * STEPS_PER_FRAME).ceil().max(1.0) as usize;
                let step = (span.1 - span.0) / count as f64;
                let mut steps = motion.start(circuit_state);
                let size = steps.len();
                for k in 0..count {
                    let values = steps[k * size..(k + 1) * size].to_vec();
                    let next = motion.advance(&values, step * k as f64, step);
                    if let Some(variable) = next.iter().position(|value| !value.is_finite()) {
                        let scene_time = span.0 + step * (k + 1) as f64;
                        return Err(NoValue {
                            variable,
                            scene_time,
                            law_time: scene_time - law_start,
                        });
                    }
                    steps.extend(next);
                }
                Ok(simulation(step, steps, Vec::new()))
            }
            Motion::Bodies(bodies) => {
                let frames = ((span.1 - span.0) * fps).ceil().max(1.0) as usize;
                let count = frames * BODY_STEPS_PER_FRAME;
                let step = (span.1 - span.0) / count as f64;
                let run = bodies.run(placed, span.0, step, count, BODY_STEPS_PER_FRAME);
                Ok(simulation(
                    step * BODY_STEPS_PER_FRAME as f64,
                    run.kept,
                    run.collisions,
                ))
            }
        }
    }

    /// The variables `dt` seconds after `values` at `scene_time`, by one step of fourth-order
    /// Runge-Kutta.
    fn advance(&self, values: &[f64], scene_time: f64, dt: f64) -> Vec<f64> {
        let law_time = scene_time - self.law_start;
        let k1 = self.rates(values, law_time);
        let k2 = self.rates(&along(values, &k1, dt / 2.0), law_time + dt / 2.0);
        let k3 = self.rates(&along(values, &k2, dt / 2.0), law_time + dt / 2.0);
        let k4 = self.rates(&along(values, &k3, dt), law_time + dt);
        values
            .iter()
            .enumerate()
            .map(|(i, value)| value + dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]))
            .collect()
    }

    fn rates(&self, values: &[f64], law_time: f64) -> Vec<f64> {
        let Motion::Rates { rates, .. } = &self.simulate.motion else {
            unreachable!("only a simulation by rates advances by them")
        };
        let values: Vec<f64> = values
            .iter()
            .copied()
            .chain(std::iter::once(law_time))
            .collect();
        rates.iter().map(|rate| rate.eval(&values)).collect()
    }

    /// The variables at a scene time, held at the span's ends outside it: the kept step before
    /// it, integrated the rest of the way by its rates, or for bodies drawn straight toward the
    /// next kept step.
    pub fn variables_at(&self, scene_time: f64) -> Vec<f64> {
        let size = self.simulate.variables.len();
        let count = self.steps.len() / size - 1;
        let into = ((scene_time - self.span.0) / self.step).clamp(0.0, count as f64);
        let k = (into.floor() as usize).min(count);
        let values = &self.steps[k * size..(k + 1) * size];
        let rest = into - k as f64;
        if rest <= 0.0 {
            return values.to_vec();
        }
        match &self.simulate.motion {
            Motion::Rates { .. } => {
                self.advance(values, self.span.0 + self.step * k as f64, rest * self.step)
            }
            Motion::Circuit(motion) => {
                motion.advance(values, self.step * k as f64, rest * self.step)
            }
            Motion::Bodies(_) => {
                let next = &self.steps[(k + 1) * size..(k + 2) * size];
                values
                    .iter()
                    .zip(next)
                    .map(|(value, next)| value + (next - value) * rest)
                    .collect()
            }
        }
    }
    /// The span in scene time.
    pub fn span(&self) -> (f64, f64) {
        self.span
    }

    /// The variables' names.
    pub fn names(&self) -> &[String] {
        &self.simulate.variables
    }

    /// A circuit simulation's state at a scene time, held at the span's ends outside it.
    pub fn circuit_state_at(&self, scene_time: f64) -> Option<Vec<f64>> {
        match &self.simulate.motion {
            Motion::Circuit(motion) => Some(motion.state(&self.variables_at(scene_time)).to_vec()),
            _ => None,
        }
    }

    /// The variables at every kept step.
    pub fn kept(&self) -> impl Iterator<Item = &[f64]> {
        self.steps.chunks(self.simulate.variables.len())
    }

    /// The scene time law time counts from.
    pub fn law_start(&self) -> f64 {
        self.law_start
    }

    pub fn set_kind(&self, set: usize) -> LawKind {
        self.simulate.sets[set].kind()
    }

    /// What set `set` gives at a scene time: three numbers for a position or a rotation, or a
    /// readout's value then two zeros. NaN or infinite where an expression has no value.
    pub fn set_at(&self, set: usize, scene_time: f64) -> [f64; 3] {
        let law_time = scene_time.clamp(self.span.0, self.span.1) - self.law_start;
        let values: Vec<f64> = self
            .variables_at(scene_time)
            .into_iter()
            .chain(std::iter::once(law_time))
            .collect();
        match &self.simulate.sets[set] {
            Set::Position(_, coordinates) | Set::Rotation(_, coordinates) => coordinates
                .each_ref()
                .map(|expression| expression.eval(&values)),
            Set::Value(_, expression) => [expression.eval(&values), 0.0, 0.0],
        }
    }
}

/// `values` moved `dt` along `rates`.
fn along(values: &[f64], rates: &[f64], dt: f64) -> Vec<f64> {
    values
        .iter()
        .zip(rates)
        .map(|(value, rate)| value + rate * dt)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn simulate(rates: &[&str], starts: &[f64]) -> Arc<Simulate> {
        let variables: Vec<String> = ["x", "v"]
            .iter()
            .take(starts.len())
            .map(|name| name.to_string())
            .collect();
        let names: Vec<&str> = variables.iter().map(String::as_str).chain(["t"]).collect();
        let rates = rates
            .iter()
            .map(|rate| Expression::parse(rate, &names).unwrap())
            .collect();
        Arc::new(Simulate {
            variables,
            motion: Motion::Rates {
                starts: starts.to_vec(),
                rates,
            },
            sets: Vec::new(),
            law_time_start: LawTimeStart::Span,
        })
    }

    #[test]
    fn a_spring_s_mass_comes_back_after_one_period() {
        let simulation = Simulation::integrate(
            &simulate(&["v", "-x"], &[1.0, 0.0]),
            (2.0, 2.0 + std::f64::consts::TAU),
            2.0,
            30.0,
            &[],
            &[],
        )
        .unwrap();

        let back = simulation.variables_at(2.0 + std::f64::consts::TAU);
        let quarter = simulation.variables_at(2.0 + std::f64::consts::FRAC_PI_2);

        assert!(
            (back[0] - 1.0).abs() < 1e-9 && back[1].abs() < 1e-9,
            "{back:?}"
        );
        assert!(
            quarter[0].abs() < 1e-9 && (quarter[1] + 1.0).abs() < 1e-9,
            "{quarter:?}"
        );
    }

    #[test]
    fn a_scene_time_between_steps_is_integrated_from_the_step_before() {
        let simulation =
            Simulation::integrate(&simulate(&["x"], &[1.0]), (0.0, 1.0), 0.0, 10.0, &[], &[])
                .unwrap();

        let between = simulation.variables_at(0.123_456)[0];

        assert!((between - 0.123_456f64.exp()).abs() < 1e-10, "{between}");
        assert_eq!(
            simulation.variables_at(5.0),
            simulation.variables_at(1.0),
            "held from the span's end"
        );
    }

    #[test]
    fn a_variable_that_runs_away_has_no_value_at_its_first_step() {
        let found =
            Simulation::integrate(&simulate(&["x*x"], &[1.0]), (0.0, 2.0), 0.0, 60.0, &[], &[])
                .unwrap_err();

        assert_eq!(found.variable, 0);
        assert!(
            found.scene_time > 0.99 && found.scene_time < 1.01,
            "1/(1 - t) blows up at 1: {}",
            found.scene_time
        );
    }
}
