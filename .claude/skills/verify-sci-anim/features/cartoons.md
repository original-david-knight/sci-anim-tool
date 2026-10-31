# Draw cartoon proteins

A molecule read from a PDB file with `"representation": "cartoon"` draws each chain's backbone as one tube through its alpha carbons, `CA` (or its nucleotides' `P`): a thin gray circle along coil, a flat red ribbon along each helix of a `HELIX` record, facing out from the helix's axis, and a flat blue arrow along each β-strand of a `SHEET` record, whose tip steps out one residue before the strand's end. No atom draws as a ball. `inspect` lists the file's `secondary_structure`, each stretch with its residue range, and a highlight of an atom group lights the tube along each residue whose `CA` the group holds. Needs sci_anim 0.22.0 or later.

## Sub-features

- `cartoon-draw` ubiquitin draws as coil, two helix ribbons and five strand arrows, turning, and reads as a textbook cartoon.
- `cartoon-inspect` `inspect` lists two helices and five β-strands with their residue ranges, and every atom as hidden with no `ball_radius`.
- `cartoon-light` the example scene lights the α-helix, then the β-sheet, and passes `check-layout`.
- `cartoon-layout` a text over a cartoon is a layout fault, judged by the cartoon's box (sci_anim 0.23.1 or later), and a text clear of the box is none.
- `cartoon-coil` a PDB file with no `HELIX` or `SHEET` records validates and draws as one coil tube.
- `cartoon-errors` a cartoon of an SDF molecule, a highlight of a group with no `CA`, and a cartoon reactant are scene errors at the right path.
- `cartoon-unchanged` every existing scene with a tube or a molecule draws the same frame, byte for byte, as the build before the change.

## How to get to it (user POV)

- Write a `molecule` whose `file` is a PDB file and whose `representation` is `cartoon`, with atom groups written by the residue ranges `inspect` lists. `sci_anim validate` checks it, `sci_anim inspect` lists `secondary_structure`, `sci_anim frame` draws it and its glow, and `sci_anim check-layout` checks the scene. `skills/sci-anim/examples/ubiquitin_cartoon.json` is the example.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. `skills/sci-anim/examples/ubiquitin_cartoon.json` turns ubiquitin over scene times 1.5 to 11.5, lights residues 23 to 34 from 2 to 3 and back off from 6 to 6.5, and lights the five strands of the sheet from 7 to 8.

- **Draw and light.** `$SKILL/scripts/run "$RUN" cartoon-light-validate validate skills/sci-anim/examples/ubiquitin_cartoon.json`, exit `0`. Then `frame` at `-t 1.5`, `-t 4` and `-t 9` into `$RUN/cartoon-<t>.png`. View each: red helix ribbons, blue strand arrows and gray coil at every time, turned between them; nothing lit at 1.5, the long helix lit at 4 with its sentence, the five strands lit at 9 with theirs and the helix unlit.
- **Layout.** `$SKILL/scripts/run "$RUN" cartoon-light-layout check-layout skills/sci-anim/examples/ubiquitin_cartoon.json --out "$RUN/work/check"`. Exit `0`, printing `No problem at any of 14 checked scene times.`
- **Inspect.** `$SKILL/scripts/run "$RUN" cartoon-inspect inspect skills/sci-anim/examples/ubiquitin_cartoon.json -t 4`. `jq -c '.objects.protein.secondary_structure[] | [.kind, .from, .to]'` prints `["helix",23,34]`, `["helix",56,59]`, then `["strand",10,17]`, `["strand",1,7]`, `["strand",64,72]`, `["strand",40,45]` and `["strand",48,50]`. `jq '[.objects.protein.atoms[] | select(.hidden | not)] | length'` prints `0`.
- **Text over the cartoon.** Write `$RUN/over.json`, ubiquitin as a cartoon at scale 0.12 with the text `label`, "Ubiquitin", size 0.5, at `[0, 0, 2]`, and `$RUN/clear.json`, the same with the text at `[0, -3, 2]`. `check-layout --no-sheet` on `over.json` exits `1`, printing one fault, `"label" and "protein" overlap by` its pixels. On `clear.json` it exits `0`. Draw `over.json` at `-t 0` and view it: the text sits on the ribbons.
- **Coil.** Write `$RUN/coil.pdb`, the `ATOM` lines of `1ubq.pdb` alone (`grep '^ATOM' skills/sci-anim/examples/structures/1ubq.pdb`), and `$RUN/coil.json` drawing it as a cartoon at scale 0.16. `validate` exits `0`, and a `frame` at `-t 0` shows one gray tube with no ribbons or arrows.
- **Errors.** `$RUN/bad.json` with `caf`, `tests/fixtures/pubchem/caffeine.sdf` as a cartoon: `validate` exits `1` with `invalid` at `/objects/caf/representation`. `$RUN/bad2.json` with ubiquitin as a cartoon whose group `n` is `[1]`, highlighted, and a reaction from 1BNA as a cartoon to it: `validate` exits `1` with `group_hidden` at `/timeline/0/animations/0/group` and `invalid` at `/timeline/1/animations/0/reactants/0` and `/timeline/1/animations/0/products/0`.
- **Unchanged.** For each scene in `examples/` and `skills/sci-anim/examples/` with a `tube` or a `molecule` but no cartoon, draw a frame at `-t 2` with this build and with the installed `sci_anim` (`SCI_ANIM=$(command -v sci_anim)`), and `cmp` the PNGs. Every pair is identical.
- **Proof.** The `.stdout.json` and `.exit` files, the frames and the `cmp` output in `$RUN`.

## Gotchas

- A highlight's `start` and `end` are seconds into its step, not shares of it.
- The glow on a blue strand reads pale. View the frame mid-span or after it, not at the start.
- `hydrogens` does nothing on a cartoon, and the group center of a caption is the middle of all the group's atoms, side chains included.
