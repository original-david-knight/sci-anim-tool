# Show readouts

A `readout` object shows a number typeset as math, to its `decimals`, with a `unit` after it. Its value is a track: a `count` eases it to a `to`, and a `follow` with `"sets": "value"` sets it from a law with one expression, `value`. Its digits are typeset once, and once more for each count of digits it shows. The number is lined up on its decimal point at the object's position, so its digits and its unit keep their places. `inspect` reports `readout.value` and `readout.text`.

## Sub-features

- `readout-count` eases a readout with `count`, with the same easing as a `move`.
- `readout-law` sets readouts from laws of `value`: a falling ball's speed, the time and the height.
- `readout-place` keeps the unit at one place as the number grows to the left.
- `readout-attach` attaches a readout to a moving ball.
- `readout-text` rounds to the decimals and shows no minus before a zero.
- `readout-errors` refuses a count or a follow of `value` of an object that is not a readout (`not_readout`), a law of `value` that does not start at the value (`law_jump`) or has no value at a frame time (`out_of_range` at `law/value`), `x` beside `value` (`invalid` at `law/x`), no `value` (`missing_field`), 7 decimals (`out_of_range`), and a unit that does not typeset (`bad_format` at `unit`).
- `readout-layout` runs `check-layout` on the readouts lesson.

## How to get to it (user POV)

- Write `{ "type": "readout", "decimals": 1, "unit": "\\text{ m/s}" }` among the objects, and a `count` of it or a `follow` with `"sets": "value"` and `"law": { "value": "..." }`, then run `validate`, `inspect`, `stages`, `frame`, `render`, `frames` or `check-layout`.
- `skills/sci-anim/examples/readouts.json` drops a ball in slow motion with its speed attached to it and its time and height beside it, then rolls a cart whose distance counts with the cart's easing.
- `sci_anim schema | jq '.["$defs"].ObjectFile.allOf[].then | select(.properties.type.const == "readout")'` prints the fields.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok` for the frames.

- **Law.** `validate skills/sci-anim/examples/readouts.json`: exit 0, duration about 14.53. `inspect -t 5.5305`, the landing: `.objects.speed.readout.text` is `9.9`, `.objects.time.readout.text` `1.01` and `.objects.height.readout.text` `0.00`, with `.objects.ball.position[1]` about -2.5. Draw frames at 4 and 5.53 and view them: the speed sits beside the ball each time, and grows from about 4.9 to 9.9 m/s.
- **Count.** `inspect -t 11.53` and `-t 13.03`: `.objects.distance.readout.value` equals `.objects.cart.position[0] + 5` each time, since the count and the move share their span and easing.
- **Place.** In those two reports, the world `bounding_box.max[0]` of `distance` is the same: the unit stays put while the number grows.
- **Text.** Write `$RUN/text.json` with a readout of `"value": -0.04, "decimals": 1` and one of `"value": 2.71828, "decimals": 3`: `inspect -t 0` gives the texts `0.0` and `2.718`.
- **Errors.** In `$RUN`: a count of a `dot` gives `not_readout` at `.../object`; a law `"value": "1 + t"` on a readout at 0 gives `law_jump` at `.../law`; `"value": "1/(t - 1) + 1"` over 2 s gives `out_of_range` at `.../law/value`; a law with `x` and `value` gives `invalid` at `.../law/x`; a law with only `x` gives `invalid` at `.../law/x` and `missing_field` at `.../law/value`; `"decimals": 7` gives `out_of_range` at `/objects/<id>/decimals`; `"unit": "\\frac{"` gives `bad_format` at `/objects/<id>/unit`. Each exits 1.
- **Cost.** Time `validate skills/sci-anim/examples/readouts.json`: well under a second, since a number that counts is not typeset again for each value.
- **Layout.** `check-layout skills/sci-anim/examples/readouts.json --out "$RUN/layout"` exits 0 and prints `No problem at any of 20 checked scene times and 183 frame times of follows.`
- **Proof.** The reports, frames and a sentence per frame in `$RUN`.

## Gotchas

- A readout grows to the left of its position. Leave room, and set its symbol in a math object aligned right beside it.
- A unit's leading space goes inside `\\text{...}`: math drops a bare space.
- A count's easing shapes the number as a move's shapes the place. Use the same easing on both when the number measures the motion.
