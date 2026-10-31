//! A simulation: an animation that integrates its variables from their starting values by their
//! rates over its span, and sets positions, rotations and readouts' values from expressions in
//! the variables. It is integrated once, when the scene file is read, by fourth-order Runge-Kutta
//! at a fixed step of a fraction of a frame, and the variables at every step are kept. The stage
//! at any scene time takes the step before it and integrates the rest of the way from there, so
//! a frame stays a pure function of the scene and the scene time.

use crate::expr::Expression;
use crate::follow::{LawKind, LawTimeStart};
use crate::scene::ObjectId;

/// How many integration steps each frame of the span is cut into.
const STEPS_PER_FRAME: f64 = 8.0;

/// The most frames a simulation spans, an hour at 60 frames a second, which bounds the steps it
/// integrates and keeps when the scene file is read.
pub const MOST_FRAMES: f64 = 216_000.0;

/// A simulation as the scene file writes it, read and checked for form.
#[derive(Debug)]
pub struct Simulate {
    /// The variables' names, in the order their values are kept and the expressions read them,
    /// then `t`.
    pub variables: Vec<String>,
    /// Each variable's value at the start of the span.
    pub starts: Vec<f64>,
    /// How fast each variable changes, per second of law time, in the variables and `t`.
    pub rates: Vec<Expression>,
    pub sets: Vec<Set>,
    pub law_time_start: LawTimeStart,
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

/// A simulation integrated over its span: its variables at every step.
#[derive(Debug)]
pub struct Simulation {
    simulate: std::sync::Arc<Simulate>,
    /// The span in scene time.
    span: (f64, f64),
    /// The scene time law time counts from.
    law_start: f64,
    step: f64,
    /// The variables at step `k`, `k` from 0 to the step count, one after another.
    steps: Vec<f64>,
}

/// A variable that has no finite value at a step of the span.
#[derive(Debug, PartialEq)]
pub struct NoValue {
    pub variable: usize,
    pub scene_time: f64,
    pub law_time: f64,
}

impl Simulation {
    /// Integrates `simulate` over `span`, at `STEPS_PER_FRAME` steps a frame of `fps`, with law
    /// time counting from `law_start`. The step is the span cut evenly, so the last step ends
    /// exactly at the span's end.
    pub fn integrate(
        simulate: &std::sync::Arc<Simulate>,
        span: (f64, f64),
        law_start: f64,
        fps: f64,
    ) -> Result<Simulation, NoValue> {
        let count = ((span.1 - span.0) * fps * STEPS_PER_FRAME).ceil().max(1.0) as usize;
        let step = (span.1 - span.0) / count as f64;
        let mut simulation = Simulation {
            simulate: std::sync::Arc::clone(simulate),
            span,
            law_start,
            step,
            steps: simulate.starts.clone(),
        };
        let size = simulate.starts.len();
        for k in 0..count {
            let scene_time = span.0 + step * k as f64;
            let values = simulation.steps[k * size..(k + 1) * size].to_vec();
            let next = simulation.advance(&values, scene_time, step);
            if let Some(variable) = next.iter().position(|value| !value.is_finite()) {
                let scene_time = span.0 + step * (k + 1) as f64;
                return Err(NoValue {
                    variable,
                    scene_time,
                    law_time: scene_time - law_start,
                });
            }
            simulation.steps.extend(next);
        }
        Ok(simulation)
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
        let values: Vec<f64> = values
            .iter()
            .copied()
            .chain(std::iter::once(law_time))
            .collect();
        self.simulate
            .rates
            .iter()
            .map(|rate| rate.eval(&values))
            .collect()
    }

    /// The variables at a scene time, held at the span's ends outside it: the step before it,
    /// integrated the rest of the way.
    pub fn variables_at(&self, scene_time: f64) -> Vec<f64> {
        let size = self.simulate.starts.len();
        let count = self.steps.len() / size - 1;
        let into = ((scene_time - self.span.0) / self.step).clamp(0.0, count as f64);
        let k = (into.floor() as usize).min(count);
        let values = &self.steps[k * size..(k + 1) * size];
        let rest = (into - k as f64) * self.step;
        if rest <= 0.0 {
            values.to_vec()
        } else {
            self.advance(values, self.span.0 + self.step * k as f64, rest)
        }
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
            starts: starts.to_vec(),
            rates,
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
            Simulation::integrate(&simulate(&["x"], &[1.0]), (0.0, 1.0), 0.0, 10.0).unwrap();

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
            Simulation::integrate(&simulate(&["x*x"], &[1.0]), (0.0, 2.0), 0.0, 60.0).unwrap_err();

        assert_eq!(found.variable, 0);
        assert!(
            found.scene_time > 0.99 && found.scene_time < 1.01,
            "1/(1 - t) blows up at 1: {}",
            found.scene_time
        );
    }
}
