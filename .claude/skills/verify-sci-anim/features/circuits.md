# Solve circuits

A `circuit` lays batteries, resistors, bulbs, capacitors, inductors, switches, ammeters, voltmeters and wires on the edges of a grid, joined where their ends meet, each drawn with its standard symbol and a label placed clear of everything else. A `simulate` with `circuit` solves it by modified nodal analysis at every step, charges its capacitors and builds its inductors' currents, sets its `switches` at scene times, moves `current_dots` by the charge that has passed, and lights each bulb by its power. Each component's quantities are variables the `sets` read as `<circuit>.<component>.<quantity>`.

## Sub-features

- `circuits-series` solves two resistors and an ammeter in one loop: one current, drops that add to the emf.
- `circuits-parallel` solves two resistors between two rails: one voltage, currents that add.
- `circuits-switch` closes and then opens a switch on a bulb: no current, then the bulb lit, then dark.
- `circuits-divider` reads a voltmeter across the lower of two resistors.
- `circuits-rc` charges a capacitor through a resistor, its voltage plotted by a dot and a trail.
- `circuits-mixed` solves a resistor in series with two in parallel.
- `circuits-report` reads `validate`'s `circuits` and `inspect`'s `circuit`.
- `circuits-same` runs one scene twice and compares the reports byte for byte.
- `circuits-errors` refuses a component off a grid line, an end that joins nothing, a battery shorted, a voltmeter in series, an ammeter across a component, a switching that shorts the battery, and a switching of something that is not a switch.
- `circuits-layout` runs `check-layout` on each lesson.

## How to get to it (user POV)

- Write a `circuit` object with `components`, then `{ "type": "simulate", "circuit": "<id>", "switches": [...], "sets": [...] }` in a play step, and run `validate`, `inspect`, `stages`, `frame`, `render` or `check-layout`.
- The lessons `skills/sci-anim/examples/series_resistors.json`, `parallel_resistors.json`, `switch_bulb.json`, `voltage_divider.json`, `rc_charging.json` and `mixed_network.json`.
- `sci_anim schema | jq '.["$defs"].ComponentFile'` prints a component's fields.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok` for the frames.

- **Series.** `inspect skills/sci-anim/examples/series_resistors.json -t 6`: `.objects.circuit.circuit.components` gives `A.current` 2, `R1.voltage` 4 and `R2.voltage` 8, 12 V over 6 Ω. Draw frames at 2 and 9 and view them: the schematic half drawn; the dots moving, the readouts at 2.00 A and 12.00 V.
- **Parallel.** `inspect skills/sci-anim/examples/parallel_resistors.json -t 6`: `R1.current` 4, `R2.current` 2, `B.current` 6. Draw the frame at 10: twice the dots' speed through R1.
- **Switch.** `inspect skills/sci-anim/examples/switch_bulb.json` at 4, 6 and 10: `L.power` 0, 12, 0, and `S.closed` false, true, false. Draw frames at 4 and 7: the lever up and the bulb dark; the lever down, the bulb glowing and the dots moving.
- **Divider.** `inspect skills/sci-anim/examples/voltage_divider.json -t 5`: `V.voltage` 6, 9 V times 2/3. Draw the frame at 8.
- **RC.** `inspect skills/sci-anim/examples/rc_charging.json -t 5`: `C.voltage` 6.3212, 10(1 - 1/e), one time constant after the switch closes at 3. Draw frames at 5 and 13: the trail through 63% at t = 2 s on the axes; the curve flattening near 10 V.
- **Mixed.** `inspect skills/sci-anim/examples/mixed_network.json -t 6`: `R1.current` 3, `R2.current` 1, `R3.current` 2. Draw the frame at 11.
- **Report.** `validate skills/sci-anim/examples/rc_charging.json`: `.circuits[0]` has `circuit` `circuit`, `start` 3, `end` 13, and `at_end` with `circuit.B.energy` equal to `circuit.R.energy` plus `circuit.C.energy` to 1e-9.
- **Same.** Run `stages skills/sci-anim/examples/rc_charging.json --from 3 --to 4` twice into two files and `cmp` them: no difference.
- **Errors.** `cargo test --release --test circuits` holds each refusal; in `$RUN`, a wire from `[0, 2]` to `[1, 3]` gives `invalid` at `.../components/<i>/to`; a resistor end at a free grid point `invalid` at its `to`, the message naming the point; wires round a battery `invalid`, "is shorted"; a voltmeter in the loop `invalid`, "is in series with"; an ammeter across a resistor through two wires `invalid`, "is across"; `switches` closing a switch onto a short `invalid` at `.../switches`, "when `S` closes at 1 s"; a switching of a resistor `invalid` at `.../switches/0/switch`. Each exits 1.
- **Layout.** `check-layout` each lesson with `--out "$RUN/layout-<name>"`: exit 0.
- **Proof.** The reports, frames and a sentence per frame in `$RUN`.

## Gotchas

- A readout set from a quantity is declared at the quantity's value when the simulation starts, or `validate` reports `law_jump`; read the starting values in `validate`'s `circuits`.
- `check-layout` judges other objects against the circuit's whole box, so a readout inside the loop is reported as overlapping it.
- A circuit before its first simulation shows no moving dots and no glow, though `inspect` already reports its currents.
