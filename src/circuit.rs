//! Circuits: batteries, resistors, bulbs, capacitors, inductors, switches and meters on the edges
//! of a grid, joined where their ends meet, solved by modified nodal analysis.
//!
//! A circuit's state is three numbers a component: the charge that has passed through it, the
//! energy it has delivered or turned to heat, and its own state, a capacitor's voltage, an
//! inductor's current or a switch's 1 for closed. Everything else, every node voltage and every
//! component's current, voltage and power, is solved from that state at any instant: each
//! capacitor holds its voltage as a battery would, each inductor drives its current as a source
//! of current would, and the rest is a network of resistances and sources. A simulation steps the
//! state by fourth-order Runge-Kutta, solving the network at every stage of every step.

use std::collections::BTreeMap;

use glam::IVec2;

pub mod drawing;

/// What a component is, with the values that make it so, in SI units.
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    /// Drives its `to` end `emf` volts above its `from` end, less its internal resistance's drop.
    Battery {
        emf: f64,
        internal_resistance: f64,
    },
    Resistor {
        resistance: f64,
    },
    /// A resistor that glows by the power it draws.
    Bulb {
        resistance: f64,
    },
    Capacitor {
        capacitance: f64,
    },
    Inductor {
        inductance: f64,
    },
    Switch,
    /// Passes current with no voltage across it, and reads that current.
    Ammeter,
    /// Passes no current, and reads the voltage across it.
    Voltmeter,
    Wire,
}

impl Kind {
    /// The word the scene file writes for it.
    pub fn word(&self) -> &'static str {
        match self {
            Kind::Battery { .. } => "battery",
            Kind::Resistor { .. } => "resistor",
            Kind::Bulb { .. } => "bulb",
            Kind::Capacitor { .. } => "capacitor",
            Kind::Inductor { .. } => "inductor",
            Kind::Switch => "switch",
            Kind::Ammeter => "ammeter",
            Kind::Voltmeter => "voltmeter",
            Kind::Wire => "wire",
        }
    }

    /// Whether it holds the voltage across it outright whatever passes, so a loop of these alone
    /// has no current the solver can find: a battery with no internal resistance, a capacitor, and
    /// anything with no resistance at all.
    fn holds_voltage(&self, closed: bool) -> bool {
        match self {
            Kind::Battery {
                internal_resistance,
                ..
            } => *internal_resistance == 0.0,
            Kind::Capacitor { .. } | Kind::Wire | Kind::Ammeter => true,
            Kind::Switch => closed,
            _ => false,
        }
    }

    /// Whether current can pass it in a network of resistances and sources at an instant, which
    /// joins its ends into one island.
    fn conducts(&self, closed: bool) -> bool {
        match self {
            Kind::Switch => closed,
            Kind::Inductor { .. } | Kind::Voltmeter => false,
            _ => true,
        }
    }

    /// Whether its current is an unknown of the solve, beside the node voltages.
    fn has_current_unknown(&self, closed: bool) -> bool {
        match self {
            Kind::Battery { .. } | Kind::Capacitor { .. } | Kind::Wire | Kind::Ammeter => true,
            Kind::Switch => closed,
            _ => false,
        }
    }

    /// Whether its `energy` quantity is what it has delivered or turned to heat over time, kept
    /// in its state, rather than what it stores now.
    fn keeps_energy(&self) -> bool {
        matches!(
            self,
            Kind::Battery { .. } | Kind::Resistor { .. } | Kind::Bulb { .. }
        )
    }

    fn has_energy(&self) -> bool {
        self.keeps_energy() || matches!(self, Kind::Capacitor { .. } | Kind::Inductor { .. })
    }
}

/// A component as the scene file writes it, its fields checked one by one.
#[derive(Clone, Debug)]
pub struct Component {
    /// Its id, which names its quantities; a wire may have none.
    pub id: Option<String>,
    pub kind: Kind,
    pub from: IVec2,
    pub to: IVec2,
    /// Its own state when the circuit is first drawn: a capacitor's voltage, an inductor's
    /// current, or a switch's 1 for closed and 0 for open.
    pub start: f64,
    /// The LaTeX of its label, or `None` for none.
    pub label: Option<String>,
}

/// One thing wrong with a circuit, at the field of the component it names.
#[derive(Debug, PartialEq)]
pub struct Fault {
    /// The component's index, or `None` for the circuit's own `ground`.
    pub component: Option<usize>,
    /// The field under the component, or `None` for the component itself.
    pub field: Option<&'static str>,
    pub message: String,
}

/// A circuit's components, the nodes their ends make and the node held at 0 volts.
#[derive(Debug)]
pub struct Network {
    pub components: Vec<Component>,
    /// Each node's grid point, in the order components first reach them.
    pub nodes: Vec<IVec2>,
    /// Each component's `from` and `to` node.
    pub ends: Vec<[usize; 2]>,
    /// The grid point of the ground symbol, if the circuit draws one.
    pub ground: Option<IVec2>,
}

impl Network {
    /// The network of `components`, grounded at `ground`, or every fault in how they join: a
    /// component whose ends are one point or off one grid line; one lying along another, or with
    /// an end partway along another; an end that joins nothing; a ground on no end; a loop with
    /// no resistance in it; a voltmeter in series; an ammeter across a component; and a
    /// voltmeter whose ends no path joins.
    pub fn new(components: Vec<Component>, ground: Option<IVec2>) -> Result<Network, Vec<Fault>> {
        let mut faults = Vec::new();
        let fault = |i: usize, field: Option<&'static str>, message: String| Fault {
            component: Some(i),
            field,
            message,
        };
        for (i, c) in components.iter().enumerate() {
            let name = describe(c, i);
            if c.from == c.to {
                faults.push(fault(
                    i,
                    Some("to"),
                    format!(
                        "{name} starts and ends at {}; give `to` another grid point",
                        point(c.from)
                    ),
                ));
            } else if c.from.x != c.to.x && c.from.y != c.to.y {
                faults.push(fault(i, Some("to"), format!("{name} runs from {} to {}, and a component runs along a grid line; make its `from` and `to` share an x or a y, as {} does", point(c.from), point(c.to), point(IVec2::new(c.to.x, c.from.y)))));
            }
        }
        if !faults.is_empty() {
            return Err(faults);
        }
        for (i, a) in components.iter().enumerate() {
            for (j, b) in components.iter().enumerate().skip(i + 1) {
                if let Some((start, end)) = shared_run(a, b) {
                    faults.push(fault(j, None, format!("{} lies along {} from {} to {}, and components meet only at their ends; split the longer one at the other's ends", describe(b, j), describe(a, i), point(start), point(end))));
                }
            }
            for (j, b) in components.iter().enumerate() {
                for (end, field) in [(b.from, "from"), (b.to, "to")] {
                    if i != j && strictly_inside(a, end) {
                        faults.push(fault(j, Some(field), format!("{}'s end {} lies partway along {}, where nothing joins; split {} at {} so they meet there", describe(b, j), point(end), describe(a, i), describe(a, i), point(end))));
                    }
                }
            }
        }
        let mut nodes: Vec<IVec2> = Vec::new();
        let mut node_of = |p: IVec2| match nodes.iter().position(|q| *q == p) {
            Some(n) => n,
            None => {
                nodes.push(p);
                nodes.len() - 1
            }
        };
        let ends: Vec<[usize; 2]> = components
            .iter()
            .map(|c| [node_of(c.from), node_of(c.to)])
            .collect();
        let mut degree = vec![0; nodes.len()];
        for [a, b] in &ends {
            degree[*a] += 1;
            degree[*b] += 1;
        }
        for (i, c) in components.iter().enumerate() {
            for (k, field) in [(0, "from"), (1, "to")] {
                if degree[ends[i][k]] == 1 {
                    faults.push(fault(i, Some(field), format!("{}'s end {} joins nothing, so no current can pass it; end another component or a wire at {}", describe(c, i), point(nodes[ends[i][k]]), point(nodes[ends[i][k]]))));
                }
            }
        }
        if let Some(g) = ground
            && !nodes.contains(&g)
        {
            faults.push(Fault {
                component: None,
                field: None,
                message: format!(
                    "the ground at {} is on no component's end; put it where ends meet, such as {}",
                    point(g),
                    point(nodes[0])
                ),
            });
        }
        let network = Network {
            components,
            nodes,
            ends,
            ground,
        };
        if faults.is_empty() {
            faults.extend(network.meter_faults());
            let closed: Vec<bool> = network.components.iter().map(|c| c.start == 1.0).collect();
            faults.extend(
                network
                    .loop_fault(&closed)
                    .map(|(i, message)| fault(i, None, message)),
            );
        }
        if faults.is_empty() {
            Ok(network)
        } else {
            Err(faults)
        }
    }

    /// The voltmeters in series, which no current could pass, and whose ends no other path
    /// joins; and the ammeters straight across another component, which they would short.
    fn meter_faults(&self) -> Vec<Fault> {
        let mut faults = Vec::new();
        for (i, c) in self.components.iter().enumerate() {
            let [a, b] = self.ends[i];
            match c.kind {
                Kind::Voltmeter => {
                    // In series, it closes the only loop some other component is on, and a
                    // voltmeter passes no current round it.
                    let voltmeter = |j: usize| matches!(self.components[j].kind, Kind::Voltmeter);
                    let cut_off = (0..self.components.len()).find(|&x| {
                        !matches!(
                            self.components[x].kind,
                            Kind::Voltmeter | Kind::Wire | Kind::Switch
                        ) && self.on_a_loop(x, |j| voltmeter(j) && j != i)
                            && !self.on_a_loop(x, voltmeter)
                    });
                    if let Some(x) = cut_off {
                        let message = format!(
                            "{} is in series with {}: the only loop through it closes through the voltmeter, and a voltmeter passes no current; connect it across the component it measures, its `from` and `to` on that component's ends",
                            describe(c, i),
                            describe(&self.components[x], x)
                        );
                        faults.push(Fault {
                            component: Some(i),
                            field: None,
                            message,
                        });
                    }
                }
                Kind::Ammeter => {
                    // Ends joined by wires alone are one point to the current.
                    let mut wired = Islands::new(self.nodes.len());
                    for (j, other) in self.components.iter().enumerate() {
                        if other.kind == Kind::Wire {
                            wired.join(self.ends[j][0], self.ends[j][1]);
                        }
                    }
                    let (a, b) = (wired.root(a), wired.root(b));
                    if let Some(j) = (0..self.components.len()).find(|&j| {
                        j != i
                            && !matches!(
                                self.components[j].kind,
                                Kind::Voltmeter | Kind::Wire | Kind::Ammeter
                            )
                            && {
                                let [c0, c1] = self.ends[j].map(|n| wired.root(n));
                                (c0 == a && c1 == b) || (c0 == b && c1 == a)
                            }
                    }) {
                        let message = format!(
                            "{} is across {}, so it shorts it and reads all its current; put it in series, in a gap you open in the wire beside it",
                            describe(c, i),
                            describe(&self.components[j], j)
                        );
                        faults.push(Fault {
                            component: Some(i),
                            field: None,
                            message,
                        });
                    }
                }
                _ => {}
            }
        }
        faults
    }

    /// Whether component `x` lies on a loop of components, leaving out those `left_out` names.
    fn on_a_loop(&self, x: usize, left_out: impl Fn(usize) -> bool) -> bool {
        let mut joined = Islands::new(self.nodes.len());
        for j in (0..self.components.len()).filter(|&j| j != x && !left_out(j)) {
            joined.join(self.ends[j][0], self.ends[j][1]);
        }
        joined.root(self.ends[x][0]) == joined.root(self.ends[x][1])
    }

    /// The first loop of components that hold their voltage outright, with `closed` saying which
    /// switches are closed, as the index of one of them and the message: a battery shorted, a
    /// capacitor straight across a source, or a loop of wires.
    pub fn loop_fault(&self, closed: &[bool]) -> Option<(usize, String)> {
        let mut joined = Islands::new(self.nodes.len());
        let mut tree: Vec<usize> = Vec::new();
        for (i, c) in self.components.iter().enumerate() {
            if !c.kind.holds_voltage(closed[i]) {
                continue;
            }
            let [a, b] = self.ends[i];
            if joined.root(a) != joined.root(b) {
                joined.join(a, b);
                tree.push(i);
                continue;
            }
            let mut members = self.path_between(&tree, a, b);
            members.push(i);
            let names: Vec<String> = members
                .iter()
                .map(|&j| describe(&self.components[j], j))
                .collect();
            let listed = list(&names);
            let battery = members
                .iter()
                .find(|&&j| matches!(self.components[j].kind, Kind::Battery { .. }));
            let capacitor = members
                .iter()
                .find(|&&j| matches!(self.components[j].kind, Kind::Capacitor { .. }));
            let message = match (battery, capacitor) {
                (Some(&j), None) => format!(
                    "{} is shorted: {listed} make a loop with no resistance in it, so its current would have no limit; put a resistor or a bulb in that loop, or give {} an `internal_resistance`",
                    describe(&self.components[j], j),
                    self.components[j].id.as_deref().unwrap_or("the battery")
                ),
                (Some(_), Some(&j)) | (None, Some(&j)) => format!(
                    "{} is straight across a source with no resistance between them ({listed}), so it would charge in no time; put a resistor in series with it",
                    describe(&self.components[j], j)
                ),
                (None, None) => format!(
                    "{listed} make a loop with no component in it, which carries any current at all; leave one of them out"
                ),
            };
            return Some((i, message));
        }
        None
    }

    /// The components of `tree`, a forest of components, on the path from node `a` to node `b`.
    fn path_between(&self, tree: &[usize], a: usize, b: usize) -> Vec<usize> {
        fn search(
            network: &Network,
            tree: &[usize],
            at: usize,
            to: usize,
            seen: &mut Vec<usize>,
        ) -> Option<Vec<usize>> {
            if at == to {
                return Some(Vec::new());
            }
            for &i in tree {
                if seen.contains(&i) {
                    continue;
                }
                let [p, q] = network.ends[i];
                let next = if p == at {
                    q
                } else if q == at {
                    p
                } else {
                    continue;
                };
                seen.push(i);
                if let Some(mut path) = search(network, tree, next, to, seen) {
                    path.insert(0, i);
                    return Some(path);
                }
            }
            None
        }
        search(self, tree, a, b, &mut Vec::new()).unwrap_or_default()
    }

    /// The node voltages and each component's current and voltage, with each capacitor at the
    /// voltage, each inductor at the current, and each switch as `state` holds them.
    pub fn solve(&self, state: &[f64]) -> Solution {
        let count = self.components.len();
        let own = |i: usize| state[3 * i + 2];
        let closed = |i: usize| own(i) == 1.0;
        let mut islands = Islands::new(self.nodes.len());
        for (i, c) in self.components.iter().enumerate() {
            if c.kind.conducts(closed(i)) {
                islands.join(self.ends[i][0], self.ends[i][1]);
            }
        }
        let roots: Vec<usize> = (0..self.nodes.len()).map(|n| islands.root(n)).collect();
        // Each island's reference node: the ground, the first battery's negative end, or its
        // first node.
        let grounded = self
            .ground
            .and_then(|g| self.nodes.iter().position(|p| *p == g));
        let batteries = self
            .components
            .iter()
            .enumerate()
            .filter(|(_, c)| matches!(c.kind, Kind::Battery { .. }))
            .map(|(i, _)| self.ends[i][0]);
        let mut reference: BTreeMap<usize, usize> = BTreeMap::new();
        for node in grounded
            .into_iter()
            .chain(batteries)
            .chain(0..self.nodes.len())
        {
            reference.entry(roots[node]).or_insert(node);
        }
        let is_reference = |n: usize| reference[&roots[n]] == n;
        let mut unknown_of_node: Vec<Option<usize>> = vec![None; self.nodes.len()];
        let mut size = 0;
        for (n, unknown) in unknown_of_node.iter_mut().enumerate() {
            if !is_reference(n) {
                *unknown = Some(size);
                size += 1;
            }
        }
        let mut unknown_of_current: Vec<Option<usize>> = vec![None; count];
        for (i, c) in self.components.iter().enumerate() {
            if c.kind.has_current_unknown(closed(i)) {
                unknown_of_current[i] = Some(size);
                size += 1;
            }
        }
        let mut matrix = vec![vec![0.0; size]; size];
        let mut rhs = vec![0.0; size];
        for (i, c) in self.components.iter().enumerate() {
            let [a, b] = self.ends[i].map(|n| unknown_of_node[n]);
            let mut conductance = |g: f64| {
                for (p, q, sign) in [(a, a, 1.0), (b, b, 1.0), (a, b, -1.0), (b, a, -1.0)] {
                    if let (Some(p), Some(q)) = (p, q) {
                        matrix[p][q] += sign * g;
                    }
                }
            };
            match c.kind {
                Kind::Resistor { resistance } | Kind::Bulb { resistance } => {
                    conductance(1.0 / resistance)
                }
                Kind::Inductor { .. } => {
                    // A source of its current from `from` to `to`, unless nothing else joins its ends.
                    let [na, nb] = self.ends[i];
                    if roots[na] == roots[nb] {
                        if let Some(a) = a {
                            rhs[a] -= own(i);
                        }
                        if let Some(b) = b {
                            rhs[b] += own(i);
                        }
                    }
                }
                _ => {}
            }
            if let Some(k) = unknown_of_current[i] {
                // Its current leaves `from` and reaches `to`; its row says what holds its voltage.
                if let Some(a) = a {
                    matrix[a][k] += 1.0;
                    matrix[k][a] += 1.0;
                }
                if let Some(b) = b {
                    matrix[b][k] -= 1.0;
                    matrix[k][b] -= 1.0;
                }
                match c.kind {
                    Kind::Battery {
                        emf,
                        internal_resistance,
                    } => {
                        // V(from) - V(to) = r I - emf.
                        matrix[k][k] -= internal_resistance;
                        rhs[k] = -emf;
                    }
                    Kind::Capacitor { .. } => rhs[k] = own(i),
                    _ => {}
                }
            }
        }
        let solved = gauss(matrix, rhs);
        let voltage_of = |n: usize| unknown_of_node[n].map_or(0.0, |k| solved[k]);
        let node_voltages: Vec<f64> = (0..self.nodes.len()).map(voltage_of).collect();
        let mut currents = vec![0.0; count];
        let mut drops = vec![0.0; count];
        for (i, c) in self.components.iter().enumerate() {
            let [a, b] = self.ends[i];
            let drop = node_voltages[a] - node_voltages[b];
            drops[i] = drop;
            currents[i] = match c.kind {
                Kind::Resistor { resistance } | Kind::Bulb { resistance } => drop / resistance,
                Kind::Inductor { .. } => own(i),
                _ => unknown_of_current[i].map_or(0.0, |k| solved[k]),
            };
            if matches!(c.kind, Kind::Inductor { .. }) && roots[a] != roots[b] {
                // Nothing else joins its ends, so its current holds at what it was.
                drops[i] = 0.0;
            }
        }
        Solution {
            node_voltages,
            currents,
            drops,
        }
    }

    /// How fast each number of `state` changes.
    fn rates(&self, state: &[f64]) -> Vec<f64> {
        let solution = self.solve(state);
        let mut rates = vec![0.0; state.len()];
        for (i, c) in self.components.iter().enumerate() {
            let (current, drop) = (solution.currents[i], solution.drops[i]);
            rates[3 * i] = current;
            rates[3 * i + 1] = if c.kind.keeps_energy() {
                power(&c.kind, current, drop)
            } else {
                0.0
            };
            rates[3 * i + 2] = match c.kind {
                Kind::Capacitor { capacitance } => current / capacitance,
                Kind::Inductor { inductance } => drop / inductance,
                _ => 0.0,
            };
        }
        rates
    }

    /// The state before any simulation of the circuit: no charge passed and no energy spent,
    /// each capacitor, inductor and switch as declared.
    pub fn rest(&self) -> Vec<f64> {
        self.components
            .iter()
            .flat_map(|c| [0.0, 0.0, c.start])
            .collect()
    }

    /// Each component's quantities that a simulation names, in order, as `(component, quantity)`:
    /// `current`, `voltage`, `power` and `charge` of every component with an id, `energy` of a
    /// battery, resistor, bulb, capacitor or inductor, and `closed` of a switch.
    pub fn quantities(&self) -> Vec<(usize, Quantity)> {
        let mut quantities = Vec::new();
        for (i, c) in self.components.iter().enumerate() {
            if c.id.is_none() {
                continue;
            }
            quantities.extend(
                [
                    Quantity::Current,
                    Quantity::Voltage,
                    Quantity::Power,
                    Quantity::Charge,
                ]
                .map(|q| (i, q)),
            );
            if c.kind.has_energy() {
                quantities.push((i, Quantity::Energy));
            }
            if c.kind == Kind::Switch {
                quantities.push((i, Quantity::Closed));
            }
        }
        quantities
    }

    /// Each quantity's value in `state`, in the order of `quantities`.
    pub fn values(&self, quantities: &[(usize, Quantity)], state: &[f64]) -> Vec<f64> {
        let solution = self.solve(state);
        quantities
            .iter()
            .map(|&(i, q)| self.value(&solution, state, i, q))
            .collect()
    }

    pub fn value(&self, solution: &Solution, state: &[f64], i: usize, q: Quantity) -> f64 {
        let kind = &self.components[i].kind;
        let (current, drop) = (solution.currents[i], solution.drops[i]);
        // Adding 0 turns -0, as a current of no charge in a loop comes out, into 0.
        0.0 + match q {
            Quantity::Current => current,
            Quantity::Voltage => match kind {
                Kind::Battery { .. } => -drop,
                _ => drop,
            },
            Quantity::Power => power(kind, current, drop),
            Quantity::Charge => match kind {
                Kind::Capacitor { capacitance } => capacitance * state[3 * i + 2],
                _ => state[3 * i],
            },
            Quantity::Energy => match kind {
                Kind::Capacitor { capacitance } => 0.5 * capacitance * state[3 * i + 2].powi(2),
                Kind::Inductor { inductance } => 0.5 * inductance * state[3 * i + 2].powi(2),
                _ => state[3 * i + 1],
            },
            Quantity::Closed => state[3 * i + 2],
        }
    }

    /// `state` stepped `dt` seconds by fourth-order Runge-Kutta, the switches held.
    pub fn step(&self, state: &[f64], dt: f64) -> Vec<f64> {
        let k1 = self.rates(state);
        let k2 = self.rates(&along(state, &k1, dt / 2.0));
        let k3 = self.rates(&along(state, &k2, dt / 2.0));
        let k4 = self.rates(&along(state, &k3, dt));
        (0..state.len())
            .map(|i| state[i] + dt / 6.0 * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]))
            .collect()
    }

    /// The shortest time constant of any capacitor or inductor with the switches of `state`: the
    /// capacitance times, or the inductance over, the resistance it meets with every source at
    /// zero. `None` for a circuit with neither.
    pub fn shortest_time_constant(&self, state: &[f64]) -> Option<(usize, f64)> {
        let mut zeroed = state.to_vec();
        for (i, c) in self.components.iter().enumerate() {
            if matches!(c.kind, Kind::Capacitor { .. } | Kind::Inductor { .. }) {
                zeroed[3 * i + 2] = 0.0;
            }
        }
        let quiet = Network {
            components: self
                .components
                .iter()
                .map(|c| Component {
                    kind: match c.kind {
                        Kind::Battery {
                            internal_resistance,
                            ..
                        } => Kind::Battery {
                            emf: 0.0,
                            internal_resistance,
                        },
                        ref kind => kind.clone(),
                    },
                    ..c.clone()
                })
                .collect(),
            nodes: self.nodes.clone(),
            ends: self.ends.clone(),
            ground: self.ground,
        };
        let mut shortest: Option<(usize, f64)> = None;
        for (i, c) in self.components.iter().enumerate() {
            let tau = match c.kind {
                // A capacitor at 1 volt with every source at zero: R = 1 / |I|.
                Kind::Capacitor { capacitance } => {
                    let mut trial = zeroed.clone();
                    trial[3 * i + 2] = 1.0;
                    let current = quiet.solve(&trial).currents[i].abs();
                    (current > 0.0).then(|| capacitance / current)
                }
                // An inductor at 1 ampere: R = |V|.
                Kind::Inductor { inductance } => {
                    let mut trial = zeroed.clone();
                    trial[3 * i + 2] = 1.0;
                    let drop = quiet.solve(&trial).drops[i].abs();
                    (drop > 0.0).then(|| inductance / drop)
                }
                _ => None,
            };
            if let Some(tau) = tau
                && shortest.is_none_or(|(_, s)| tau < s)
            {
                shortest = Some((i, tau));
            }
        }
        shortest
    }
}

/// The node voltages and each component's current, from its `from` end to its `to` end, and the
/// voltage of its `from` end above its `to` end.
#[derive(Debug)]
pub struct Solution {
    pub node_voltages: Vec<f64>,
    pub currents: Vec<f64>,
    pub drops: Vec<f64>,
}

/// One number of a component that a simulation names `<circuit>.<component id>.<quantity>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quantity {
    /// Amperes through it from `from` to `to`.
    Current,
    /// Volts across it: its `from` end above its `to` end, or a battery's positive end above its
    /// negative end.
    Voltage,
    /// Watts: what a battery delivers to the rest of the circuit, and what any other component
    /// takes from it.
    Power,
    /// Coulombs passed through it from `from` to `to`, or a capacitor's charge.
    Charge,
    /// Joules a battery has delivered, a resistor or bulb has turned to heat and light, or a
    /// capacitor or inductor holds.
    Energy,
    /// A switch's 1 when closed and 0 when open.
    Closed,
}

impl Quantity {
    pub fn word(self) -> &'static str {
        match self {
            Quantity::Current => "current",
            Quantity::Voltage => "voltage",
            Quantity::Power => "power",
            Quantity::Charge => "charge",
            Quantity::Energy => "energy",
            Quantity::Closed => "closed",
        }
    }
}

/// What `kind` delivers, for a battery, or takes, for anything else, at `current` and `drop`.
fn power(kind: &Kind, current: f64, drop: f64) -> f64 {
    match kind {
        Kind::Battery { .. } => -drop * current,
        _ => drop * current,
    }
}

/// A switch set closed or open `at` seconds into a simulation's span.
#[derive(Clone, Debug, PartialEq)]
pub struct Switching {
    pub component: usize,
    pub closed: bool,
    pub at: f64,
}

/// A simulation of a circuit: its network, the switchings in its span, and the variables it
/// names: each component's quantities, then the state, three numbers a component.
#[derive(Debug)]
pub struct CircuitMotion {
    /// The circuit object it solves.
    pub object: crate::scene::ObjectId,
    pub network: std::sync::Arc<Network>,
    pub switchings: Vec<Switching>,
    pub quantities: Vec<(usize, Quantity)>,
}

impl CircuitMotion {
    /// The variables' names: `<circuit id>.<component id>.<quantity>` for each quantity, then
    /// names an expression cannot read for the state.
    pub fn names(&self, circuit: &str) -> Vec<String> {
        let network = &self.network;
        self.quantities
            .iter()
            .map(|&(i, q)| {
                let id = network.components[i].id.as_deref().unwrap_or_default();
                format!("{circuit}.{id}.{}", q.word())
            })
            .chain((0..network.components.len()).flat_map(|i| {
                ["passed", "spent", "own"].map(|part| format!("{circuit}.#{i}.{part}"))
            }))
            .collect()
    }

    /// The variables for `state`, with the switchings at 0 seconds into the span applied.
    pub fn start(&self, state: &[f64]) -> Vec<f64> {
        let mut state = state.to_vec();
        for s in self.switchings.iter().filter(|s| s.at == 0.0) {
            state[3 * s.component + 2] = f64::from(u8::from(s.closed));
        }
        self.variables(state)
    }

    fn variables(&self, state: Vec<f64>) -> Vec<f64> {
        let mut variables = self.network.values(&self.quantities, &state);
        variables.extend(state);
        variables
    }

    /// The state part of `variables`.
    pub fn state<'a>(&self, variables: &'a [f64]) -> &'a [f64] {
        &variables[self.quantities.len()..]
    }

    /// `variables` at `into` seconds into the span, `dt` seconds on, each switching between set
    /// as it falls.
    pub fn advance(&self, variables: &[f64], into: f64, dt: f64) -> Vec<f64> {
        let mut state = self.state(variables).to_vec();
        let mut at = into;
        let end = into + dt;
        for s in self
            .switchings
            .iter()
            .filter(|s| s.at > into && s.at <= end)
        {
            if s.at > at {
                state = self.network.step(&state, s.at - at);
                at = s.at;
            }
            state[3 * s.component + 2] = f64::from(u8::from(s.closed));
        }
        if end > at {
            state = self.network.step(&state, end - at);
        }
        self.variables(state)
    }
}

/// Which island of nodes joined by components each node is in.
struct Islands(Vec<usize>);

impl Islands {
    fn new(count: usize) -> Islands {
        Islands((0..count).collect())
    }

    fn root(&mut self, mut n: usize) -> usize {
        while self.0[n] != n {
            self.0[n] = self.0[self.0[n]];
            n = self.0[n];
        }
        n
    }

    fn join(&mut self, a: usize, b: usize) {
        let (a, b) = (self.root(a), self.root(b));
        self.0[a.max(b)] = a.min(b);
    }
}

/// The solution of `matrix x = rhs` by Gaussian elimination with partial pivoting. The checks
/// when the scene file is read leave no loop of held voltages and no node unjoined to its
/// island's reference, so the matrix is never singular.
fn gauss(mut matrix: Vec<Vec<f64>>, mut rhs: Vec<f64>) -> Vec<f64> {
    let size = rhs.len();
    for col in 0..size {
        let pivot = (col..size)
            .max_by(|&p, &q| matrix[p][col].abs().total_cmp(&matrix[q][col].abs()))
            .expect("a column to pivot on");
        matrix.swap(col, pivot);
        rhs.swap(col, pivot);
        let pivot_row = matrix[col].clone();
        for row in col + 1..size {
            let factor = matrix[row][col] / pivot_row[col];
            if factor == 0.0 {
                continue;
            }
            for (entry, pivot) in matrix[row][col..].iter_mut().zip(&pivot_row[col..]) {
                *entry -= factor * pivot;
            }
            rhs[row] -= factor * rhs[col];
        }
    }
    let mut solved = vec![0.0; size];
    for row in (0..size).rev() {
        let rest: f64 = (row + 1..size).map(|k| matrix[row][k] * solved[k]).sum();
        solved[row] = (rhs[row] - rest) / matrix[row][row];
    }
    solved
}

fn along(state: &[f64], rates: &[f64], dt: f64) -> Vec<f64> {
    state.iter().zip(rates).map(|(s, r)| s + r * dt).collect()
}

/// The grid points two straight components share along their length, if they overlap.
fn shared_run(a: &Component, b: &Component) -> Option<(IVec2, IVec2)> {
    let (a0, a1) = (a.from.min(a.to), a.from.max(a.to));
    let (b0, b1) = (b.from.min(b.to), b.from.max(b.to));
    let same_line = (a0.x == a1.x && b0.x == b1.x && a0.x == b0.x)
        || (a0.y == a1.y && b0.y == b1.y && a0.y == b0.y);
    let (start, end) = (a0.max(b0), a1.min(b1));
    (same_line && start != end && start.cmple(end).all()).then_some((start, end))
}

/// Whether `p` lies on `c` between its ends.
fn strictly_inside(c: &Component, p: IVec2) -> bool {
    let (lo, hi) = (c.from.min(c.to), c.from.max(c.to));
    p != c.from
        && p != c.to
        && lo.cmple(p).all()
        && p.cmple(hi).all()
        && (lo.x == hi.x || lo.y == hi.y)
}

/// A component as messages name it: `resistor R1`, or `the wire from [0, 0] to [2, 0]`.
pub fn describe(c: &Component, i: usize) -> String {
    match &c.id {
        Some(id) => format!("{} `{id}`", c.kind.word()),
        None => format!(
            "the {} from {} to {} (components/{i})",
            c.kind.word(),
            point(c.from),
            point(c.to)
        ),
    }
}

fn point(p: IVec2) -> String {
    format!("[{}, {}]", p.x, p.y)
}

fn list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => one.clone(),
        [most @ .., last] => format!("{} and {last}", most.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(id: &str, kind: Kind, from: [i32; 2], to: [i32; 2]) -> Component {
        Component {
            id: (!id.is_empty()).then(|| id.to_string()),
            kind,
            from: IVec2::from(from),
            to: IVec2::from(to),
            start: 0.0,
            label: None,
        }
    }

    fn battery(emf: f64) -> Kind {
        Kind::Battery {
            emf,
            internal_resistance: 0.0,
        }
    }

    #[test]
    fn a_battery_drives_ohms_law_through_a_resistor() {
        let network = Network::new(
            vec![
                component("B", battery(12.0), [0, 0], [0, 2]),
                component("R", Kind::Resistor { resistance: 4.0 }, [0, 2], [2, 2]),
                component("", Kind::Wire, [2, 2], [2, 0]),
                component("", Kind::Wire, [2, 0], [0, 0]),
            ],
            None,
        )
        .unwrap();

        let solution = network.solve(&network.rest());

        assert!((solution.currents[0] - 3.0).abs() < 1e-12, "{solution:?}");
        assert!((solution.currents[1] - 3.0).abs() < 1e-12, "{solution:?}");
        assert!((solution.drops[1] - 12.0).abs() < 1e-12, "{solution:?}");
    }

    #[test]
    fn a_wire_across_a_battery_is_named_with_what_would_pass() {
        let faults = Network::new(
            vec![
                component("B", battery(6.0), [0, 0], [0, 1]),
                component("", Kind::Wire, [0, 1], [1, 1]),
                component("", Kind::Wire, [1, 1], [1, 0]),
                component("", Kind::Wire, [1, 0], [0, 0]),
            ],
            None,
        )
        .unwrap_err();

        assert!(
            faults[0].message.starts_with("battery `B` is shorted"),
            "{faults:?}"
        );
        assert!(
            faults[0].message.contains("internal_resistance"),
            "{faults:?}"
        );
    }
}
