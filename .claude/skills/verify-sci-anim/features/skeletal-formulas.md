# Draw skeletal formulas

A `"type": "skeletal-formula"` draws a molecule flat, as a textbook draws it, from PubChem's 2D record (`record_type=2d`): carbons as corners with no letter, every other atom as its atom symbol with the hydrogens it holds written on the side away from its bonds (`OH`, `HO`, `H₂N`), hydrogens on carbon left out, a double bond as two strokes, inside its ring for a ring bond and centered otherwise, a triple bond as three, and each stereo bond as a wedge or a hash with its narrow end at the file's first atom of the bond. `bond_length` sets how long its bonds are drawn. `mirror` draws its mirror image: `x` and `y` flip the drawing, and `z` keeps every atom where it is and swaps wedges and hashes. A `write` draws the bonds along their path, filling each wedge as the stroke passes it, then writes the atom symbols. A `highlight` lights an atom group with a see-through fill over its atoms and the bonds between them. `inspect` reports `skeletal_formula` with every atom's `drawn` and `symbol` and every bond's `form` and `strokes`. Needs sci_anim 0.25.0 or later.

## Sub-features

- `skeletal-draw` draws L-alanine from its 2D record: `H₂N`, `O`, `OH`, three corners, a hash from the stereocenter to the nitrogen and a centered double bond to the oxygen, bonds stopping short of the atom symbols.
- `skeletal-rings` draws ring double bonds inside their ring, and the atom symbols and wedges of glucose and caffeine as a textbook does.
- `skeletal-mirror` draws `mirror` `x` and `y` flipped, with the hydrogens still written away from the bonds, and `z` in place with the hash turned into a wedge.
- `skeletal-write` draws the bonds, then writes the atom symbols: nothing before the write, bonds part way through it, the whole formula after.
- `skeletal-highlight` lights an atom group behind the formula's strokes and atom symbols.
- `skeletal-inspect` reports each atom's `drawn`, `symbol` and `position`, each bond's `atoms`, `stereo`, `form` and `strokes`, and each group's glow and box.
- `skeletal-layout` passes `check-layout` with the skill's example.
- `skeletal-errors` refuses a 3D structure file, a PDB file and a missing file at `file`, a `create`, and a highlight of a group of left-out hydrogens.
- `skeletal-unchanged` draws molecules, and the scenes of the skill's chemistry examples, as before.

## How to get to it (user POV)

- Fetch a 2D record with `curl -fo <name>_2d.sdf "https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/CID/<cid>/record/SDF?record_type=2d"`, add a `skeletal-formula` object with `file`, `write` it and `highlight` a `group`, then run `validate`, `frame`, `inspect`, `stages`, `render`, `frames` or `check-layout`.
- `skills/sci-anim/examples/alanine_formulas.json` writes L-alanine from 1.5 s to 4 s, lights its amino group from 5 s to 6.5 s, draws a mirror pane, writes D-alanine as `mirror: "x"` from 9 s to 11.5 s, and lights both stereocenters from 12.5 s to 14 s. Its file is `skills/sci-anim/examples/structures/l_alanine_2d.sdf`. `tests/fixtures/pubchem/caffeine-2d.sdf` is caffeine's 2D record.
- `sci_anim schema | jq '.["$defs"].ObjectFile.allOf[].then | select(.properties.type.const == "skeletal-formula")'` prints the fields.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.
- For the rings, fetch glucose's and ibuprofen's 2D records into `$RUN/work/` with `curl -fo glucose_2d.sdf "https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/CID/5793/record/SDF?record_type=2d"` and CID 3672. Without the network, use caffeine's fixture alone.

- **Draw.** Write `$RUN/pair.json`, 1280×720 on white, with L-alanine at `[-3, 0.5, 0]`, its `mirror: "x"` image at `[3, 0.5, 0]` and its `mirror: "z"` image at `[0, -2.2, 0]`, all `"color": "#000000"`. `frame` it at 0 and view the PNG against a textbook's L-alanine: `H₂N` on the left joined by a hash that widens toward the N, a corner at the stereocenter, a methyl stroke down, `O` above a double bond and `OH` to the right; every bond stops short of the letters, and the corners join with no notch.
- **Rings.** Write `$RUN/rings.json` with glucose and ibuprofen side by side, black on white, and `frame` it: glucose's ring `O` clears its two bonds, its `HO` on the left reads left to right, its wedges and hashes widen away from the ring carbons; ibuprofen's benzene ring has its three second strokes inside the ring and shorter at both ends, and its `C=O` is two centered strokes.
- **Mirror.** In `$RUN/pair.json`'s frame, the `x` image has `HO` on the left and `NH₂` on the right with the hash kept; the `z` image is placed as L-alanine with a filled wedge to `H₂N`. `inspect` it: the `x` image's atom positions are L-alanine's with x negated about its own position, and `.bonds[3].form` is `hash` there and `wedge` on the `z` image.
- **Write.** `frame skills/sci-anim/examples/alanine_formulas.json` at 1.4, 2.5 and 4 and view them: nothing of L-alanine, some bonds drawn with no atom symbols, then the whole formula.
- **Highlight.** `frame` the example at 7 and view it: a yellow box behind `H₂N`. At 14 a blue disc lies behind each stereocenter's corner.
- **Inspect.** `inspect skills/sci-anim/examples/alanine_formulas.json --scene-time 7`: `.objects.l_alanine.skeletal_formula.atoms[2]` has `drawn` `symbol` and `symbol` `"H₂N"`, atoms 7 to 13 are `hidden`, `.bonds[3]` has `atoms` `[4, 3]`, `stereo` `hash`, `form` `hash` and `strokes` 5 or more, `.bonds[2]` has `form` `double-centered`, and `.groups.amino.glow` is 1. For caffeine's 2D record, two bonds have `form` `double-in-ring` and two `double-centered`.
- **Layout.** `check-layout skills/sci-anim/examples/alanine_formulas.json --out "$RUN/layout"` exits 0, and the contact sheets show each step.
- **Errors.** With `jq`, write variants into `$RUN`: the formula's `file` set to `tests/fixtures/pubchem/caffeine.sdf` (3D), to `skills/sci-anim/examples/structures/1bna.pdb`, and to `missing.sdf`; a `create` of `l_alanine`; and the `amino` group set to its hydrogens alone, `[11, 12]`. `validate` exits 1 with `bad_structure_file`, `bad_structure_file` and `unreadable_file` at `/objects/l_alanine/file`, the last naming `record_type=2d`; `use_write` at the create's `object`; and `group_hidden` at each highlight's `group`.
- **Unchanged.** `frame` `examples/molecules.json` and `skills/sci-anim/examples/hydration.json` at a few scene times with `target/release/sci_anim` and with the `sci_anim` on PATH, and `cmp` each pair of PNGs: they match byte for byte.
- **Proof.** The PNGs, reports, and a sentence per PNG in `$RUN`.

## Gotchas

- A skeletal formula reads the 2D record, and a molecule the 3D one. The two records of one compound number their atoms differently: find a formula's atom numbers with `inspect`.
- A formula is written, not created: `create` on one is `use_write`.
- `scale` does not widen strokes, so a formula scaled up looks thin. Set `bond_length` instead.
- A glow at 0.25 opacity is faint on a white background. Judge a highlight on the dark default background.
- A `sci_anim` before 0.25.0 gives `unknown_value` at the formula's `type`. Drive `target/release/sci_anim`.
