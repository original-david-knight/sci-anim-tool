# Run simulations

A `simulate` animation integrates its `variables` from their starting values by their `rates` over its span, and `sets` positions, rotations and readouts' values from expressions in the variables at every scene time of it. Its rates read the variables and `t`, law time. It is integrated once when the scene file is read, so `inspect` at any scene time reads where it put each object, and each value holds from the span's end on.

## Sub-features

- `simulate-pendulum` swings a bob with drag, with a rod whose end sits on it, a trail of it and a readout of its speed.
- `simulate-shared` runs predator and prey, whose two variables place a state point on axes and set two readouts.
- `simulate-any-time` reads a spring's mass between two integration steps and after the span.
- `simulate-law-time` integrates a rate that reads `t`.
- `simulate-errors` refuses a set that does not start where its value is (`law_jump`), a variable that runs away (`out_of_range` at its rate), a set with no value at a frame time (`out_of_range` at its expression), no variable (`wrong_length` at `variables`), a variable's name an expression cannot read (`invalid`), a missing rate (`missing_field`) or one for no variable (`invalid`), a set of no object (`unknown_object`), of two values (`invalid`) or of a value of an object that is not a readout (`not_readout`), an unknown name in a set (`bad_format`), an `easing_curve` (`invalid`), a second set of one value (`invalid` at the later set), a span of more than 216000 frames (`out_of_range` at the simulation), a set of a trail's position (`placed_by_trail` at its `object`), and a move of a simulated value in its span (`conflicting_animations`).
- `simulate-layout` runs `check-layout`, which checks every frame time of a simulation.

## How to get to it (user POV)

- Write `{ "type": "simulate", "variables": {...}, "rates": {...}, "sets": [...] }` in a play step, then run `validate`, `inspect`, `stages`, `frame`, `render`, `frames` or `check-layout`.
- `skills/sci-anim/examples/simulations.json` swings a pendulum with drag, then runs predator and prey round their cycle on a phase portrait.
- `sci_anim schema | jq '.["$defs"].AnimationFile.allOf[].then | select(.properties.type.const == "simulate")'` prints the fields, and `jq '.["$defs"].SetFile'` a set's.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok` for the frames.

- **Pendulum.** `validate skills/sci-anim/examples/simulations.json`: exit 0, duration 30.5. `inspect -t 2.5` gives `.objects.bob.position` about `[-0.67, 1.59, 0.01]`, the declared start, and `-t 14.5` a bob near the bottom, below `[-3, 2.5]` by about 2.5, with `.objects.speed.readout.text` under 1. Draw frames at 3.5 and 14.5 and view them: a wide swing with its arc behind it, then a small one, the rod from the pivot to the bob each time.
- **Shared.** `inspect -t 20` and `-t 25`: `.objects.rabbits.readout.value` and `.objects.foxes.readout.value` are the state point's data `position` x and y, each time different. Draw the frame at 29 and view it: a closed loop of the cycle on the axes.
- **Any time.** Write `$RUN/spring.json`: a dot `mass` at `[1, 0, 0]` and a readout `speed` with 3 decimals, and a simulation over 6.2832 s of `x` from 1 and `v` from 0 with rates `v` and `-x`, setting the mass's position `["x", "0", "0"]` and the speed's value `abs(v)`, then a wait of 1 s. `inspect -t 1.5708` gives the mass at `[0, 0, 0]` and the speed `1.000`; `-t 0.12345` gives x about 0.992, cos(0.12345), between two steps; `-t 7` gives the position at `-t 6.2832`.
- **Law time.** A rate `"x": "2*t"` from 1 puts the mass at x 5 at scene time 2. In a chapter `fall` holding a wait of 1 s and a simulation over 1 s with `"from": "fall"`, a set `["x + t", "0", "0"]` of `x` from 0 with rate `0`, its mass declared at `[1, 0, 0]`, puts the mass at x 1.5 at scene time 1.5; without `from` it is `law_jump`.
- **Rotation.** A set `"rotation": ["0", "0", "angle"]` of `angle` from 0 with rate `10` gives `.objects.mass.rotation` `[0, 0, 10]` at scene time 1.
- **Errors.** In `$RUN`: starting `x` at 2 for a mass declared at 1 gives `law_jump` at `.../sets/0/position`; a rate `x*x` gives `out_of_range` at `.../rates/x`; a set `["x", "sqrt(x)", "0"]` of that spring, its mass declared at `[1, 1, 0]`, gives `out_of_range` at `.../sets/0/position/1`; `"variables": {}` with `"rates": {}` gives `wrong_length` at `.../variables`; a variable `sin` gives `invalid` at `.../variables/sin`; a variable with no rate gives `missing_field` at `.../rates/<name>`, and a rate for no variable `invalid` there; a set of `nowhere` gives `unknown_object`, one with `position` and `value` `invalid` at the set, a `value` of a dot `not_readout`, and an unknown name `bad_format` at `.../value`; an unknown name in a position's second expression gives `bad_format` at `.../position/1` whose message starts `` `y` is not an expression``; an `easing_curve` gives `invalid`; a second set of the mass's position gives `invalid` at that set, naming the first; a span of 10000 s gives `out_of_range` at the simulation; a set of a trail's position gives `placed_by_trail` at `.../sets/0/object`; a move of a simulated mass in the span gives `conflicting_animations`. Each exits 1.
- **Layout.** `check-layout skills/sci-anim/examples/simulations.json --out "$RUN/layout"` exits 0 and prints `No problem at any of 23 checked scene times and 1442 frame times of simulations.`
- **Proof.** The reports, frames and a sentence per frame in `$RUN`.

## Gotchas

- A set gives its value outright. Declare each object where the starting values put it.
- A second-order law is two variables, the position and its rate of change.
- A stiff or fast rate needs a short span or gentler numbers: the integration takes eight steps a frame, whatever the rates.
