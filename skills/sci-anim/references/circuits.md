# Circuits: schematics whose currents sci_anim computes

Read `instructional-videos.md`, `motion.md` and `physics.md` first. From sci_anim 0.27.0 a circuit is computed, not drawn by hand: you lay its components on a grid, and a `simulate` solves it by modified nodal analysis, so every current, voltage and power comes from the components' values. Dots of current move by the computed current, bulbs glow by the power they draw, switches open and close at scene times, and capacitors charge through their resistors. `../examples/series_resistors.json`, `parallel_resistors.json`, `switch_bulb.json`, `voltage_divider.json`, `rc_charging.json` and `mixed_network.json` are whole lessons.

## Lay out the schematic

```json
"circuit": { "type": "circuit", "position": [-6, -2, 0], "spacing": 1.2, "current_dots": { "speed": 0.6 },
  "components": [
    { "id": "B", "kind": "battery", "emf": 12, "from": [0, 0], "to": [0, 3] },
    { "id": "R1", "kind": "resistor", "resistance": 2, "from": [0, 3], "to": [2, 3], "label": "R_1 = 2\\,\\Omega" },
    { "id": "R2", "kind": "resistor", "resistance": 4, "from": [2, 3], "to": [4, 3] },
    { "id": "A", "kind": "ammeter", "from": [4, 3], "to": [4, 0] },
    { "kind": "wire", "from": [4, 0], "to": [0, 0] }
  ] }
```

- Each component runs along one grid line from grid point `from` to grid point `to`, whole numbers. Grid point `[0, 0]` sits at the object's `position`, and `spacing` world units separate neighbouring grid points. The symbols are drawn in proportion to `spacing`.
- Ends on the same grid point are joined, and a dot marks a point where three or more meet. Components meet only at their ends: an end partway along a wire joins nothing, so split the wire there.
- Kinds and the values they take, in SI units: `battery` (`emf` volts, `internal_resistance` ohms, its `from` the negative end, its `to` the positive end, the long plate), `resistor` and `bulb` (`resistance` ohms), `capacitor` (`capacitance` farads, `voltage` at the start), `inductor` (`inductance` henries, `current` at the start), `switch` (`closed`, open by default), `ammeter`, `voltmeter` and `wire`. Every component but a wire needs an `id`.
- A label is LaTeX beside the symbol, on the outer side of the loop unless something is there, never over a symbol, a wire or another label. The default is the value with its unit, as `4\,\Omega` or `1\,\text{mF}`; write `label` to name it, as `R_1 = 4\,\Omega`, or `""` for none. A label that meets something on both sides is a scene error: give the circuit a larger `spacing` or a smaller `size`.
- `ground` puts a ground symbol at a grid point and holds it at 0 volts. With none, the battery's negative end is 0 volts.
- Draw it with `create`: the symbols and wires are drawn along their length, then the labels fade in.

## Solve it

```json
{ "type": "play", "duration": 8, "animations": [
  { "type": "simulate", "circuit": "circuit",
    "switches": [{ "switch": "S", "closed": true, "at": 2 }],
    "sets": [{ "object": "current", "value": "circuit.A.current" }] }
] }
```

- `simulate` with `circuit` solves the circuit at every step of its span: each capacitor charges and each inductor's current builds by fourth-order Runge-Kutta, eight steps a frame. It starts where the circuit's last simulation left it, so two simulations in a row continue one circuit, with the narration's pauses between.
- `switches` close and open switches `at` seconds into the span, and the circuit is solved again from that moment.
- Each component's quantities are variables a set reads as `<circuit id>.<component id>.<quantity>`: `current` in amperes from `from` to `to`; `voltage`, its `from` end above its `to` end, or a battery's positive end above its negative end; `power`, what a battery delivers or anything else takes; `charge` passed through it, or a capacitor's charge; `energy` a battery has delivered, a resistor or bulb has turned to heat and light, or a capacitor or inductor holds; and `closed` of a switch, 1 or 0.
- A readout set from a quantity must be declared with its value at the span's start, or `validate` reports `law_jump` with both values. `validate` lists every quantity at each simulation's start and end under `circuits`, so read the starting values there.
- To plot a quantity against time, put a `dot` on `axes` and set its `position` to `["t", "circuit.C.voltage", "0"]`, with a `trail` of the dot: the trail draws the curve as it charges. `t` counts from the span's start.
- `current_dots` draws dots along every wire and component, moving in the conventional direction, from a battery's positive end round to its negative end, at `speed` world units a second for each ampere. Set `speed` so the largest current moves about 1 to 3 units a second: about 300 for a few milliamperes.
- Each bulb glows by its share of the most power any bulb of the circuit draws in any of its simulations, so two bulbs in one circuit compare by brightness.
- Before its first simulation the circuit stands as declared, its dots still and its bulbs dark, though `inspect` already reports its currents; after a simulation ends it holds where it was left. Run the simulation until the scene ends for current that keeps moving.

| Lesson | Write |
|---|---|
| Resistors in series, R = R1 + R2 | a battery and two resistors in one loop, an ammeter, readouts of `A.current` and `R1.voltage + R2.voltage` |
| Resistors in parallel | two resistors between the same two rails of wire, readouts of each `current` and the battery's |
| A switch lights a bulb | a `switch` and a `bulb`, `switches` closing it, a readout of the bulb's `power` |
| A voltage divider | two resistors in series, a `voltmeter` across the lower one through two wires, a readout of its `voltage` |
| RC charging | a switch, a resistor and a capacitor, the switch closed `at` 0, a dot on axes set to `t` and the capacitor's `voltage` with a trail |
| A series-parallel network | a resistor in series with two in parallel, readouts of each `current` |
| RL rise | an `inductor` in place of the capacitor: its `current` rises to emf/R with time constant L/R |

## What validate refuses

Each message names what is wrong and what would pass:

- a component whose ends are one point or not on one grid line; one lying along another; an end partway along another;
- an end that joins nothing;
- a loop with no resistance in it: a battery shorted by wires, a closed switch or an ammeter, a capacitor straight across a battery, a loop of wires; a switching that would make one is refused at the simulation's `switches`;
- a voltmeter in series, so the only loop through some component closes through it;
- an ammeter across a component;
- a capacitor or inductor whose time constant is shorter than four of the simulation's steps;
- a value a kind needs missing, or one it does not take, as `emf` on a resistor.

## Inspect and check

- `inspect` gives `.objects.<circuit>.circuit`: `nodes`, each grid point with its `voltage` and place in pixels; `components`, by id, each with its ends, `current`, `voltage`, `power`, `charge`, `energy` and `closed`, and its label's box; and `wires`, each with its ends and `current`.
- `check-layout` judges other objects against the circuit's whole box, labels included, so put readouts, equations and plots beside the circuit, not inside its loops.

## Limits

- Components lie on grid lines; there is no automatic layout of series and parallel groups.
- Meters are ideal: an ammeter has no resistance and a voltmeter passes no current.
- A capacitor or inductor needs a time constant of at least four steps, about 0.0083 s at 60 frames a second.
