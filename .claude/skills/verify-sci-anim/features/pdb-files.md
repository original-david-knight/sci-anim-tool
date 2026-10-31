# Read PDB files

A molecule whose `file` ends in `.pdb` or `.ent` is read as a PDB file: its first model's atoms without waters, numbered by their serial numbers, its `CONECT` bonds, and the bonds of amino acids and nucleotides found from their atoms' distances, all single. `inspect` lists each chain with its residues and each residue's atom names. An atom group may be written by residue picks, and with `"part": "nucleobase"` holds only the nucleobases. Needs sci_anim 0.17.0 or later.

## Sub-features

- `pdb-read` reads 1BNA as 486 atoms, 544 bonds and chains A and B of 12 residues, and 1UBQ as 602 atoms, 608 bonds and chain A of 76 residues.
- `pdb-bonds` no bond is longer than its atoms' covalent radii and 0.4 Å, and every residue is bonded to the next one of its chain.
- `pdb-groups` a group of one residue, a run, a whole chain, or two residues' nucleobases holds exactly those atoms.
- `pdb-turn` the two example scenes turn their molecule with one nucleobase pair and one helix lit, and pass `check-layout`.
- `pdb-errors` a PDB file with no atoms, a malformed coordinate line, a chain or residue the file does not have, and residues on an SDF molecule are scene errors at the right path.
- `pdb-sdf-unchanged` every scene with an SDF molecule draws the same frame, byte for byte, as the build before the change.

## How to get to it (user POV)

- Write a `molecule` whose `file` is a PDB file, such as `skills/sci-anim/examples/structures/1ubq.pdb`, and atom groups as `{ "residues": [...] }`. `sci_anim validate` checks them, `sci_anim inspect` lists `chains` and `groups`, `sci_anim frame` draws the molecule and its glow, and `sci_anim check-layout` checks the scene.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. `skills/sci-anim/examples/dna_double_helix.json` lights the nucleobase pair of residue 6 of chain A and residue 19 of chain B from scene time 2.3 to 3.5 as the dodecamer turns from 1.5 to 9.5. `skills/sci-anim/examples/ubiquitin.json` lights residues 23 to 34 over the same times.
- A scene of both molecules at scale 1, `$RUN/both.json`: `{ "objects": { "dna": { "type": "molecule", "file": "<repo>/skills/sci-anim/examples/structures/1bna.pdb" }, "ub": { "type": "molecule", "file": "<repo>/skills/sci-anim/examples/structures/1ubq.pdb" } }, "timeline": [] }`.

- **Read.** `$SKILL/scripts/run "$RUN" pdb-read-inspect inspect "$RUN/both.json" -t 0`. Exit `0`. `jq -c '.objects | to_entries[] | [.key, (.value.atoms | length), (.value.bonds | length), [.value.chains[] | [.chain, (.residues | length)]]]'` prints `["dna",486,544,[["A",12],["B",12]]]` and `["ub",602,608,[["A",76]]]`. `.objects.dna.chains[1].residues[0].atoms["O5'"]` is `245`: the `TER` record after chain A takes serial 244.
- **Bonds.** From the same report, with `r='{"C":0.75,"N":0.71,"O":0.63,"P":1.11,"S":1.03}'`, the covalent radii of the elements in both files: `jq -c --argjson r "$r" '.objects[] | (.atoms | map({key: (.number | tostring), value: .}) | from_entries) as $a | [.bonds[] | [$a[(.atoms[0] | tostring)], $a[(.atoms[1] | tostring)]] | ((.[0].position as $p | .[1].position as $q | [range(3)] | map(($p[.] - $q[.]) * ($p[.] - $q[.])) | add | sqrt) - $r[.[0].element] - $r[.[1].element])] | max'` prints how far the longest bond passes its atoms' two covalent radii: below `0.4` for each molecule, about `0.07` and `0.11`. `jq -c '.objects[] | .bonds as $b | [.chains[] | .residues | range(1; length) as $i | [.[$i - 1].atoms[]] as $x | [.[$i].atoms[]] as $y | any($b[]; ((.atoms[0] | IN($x[])) and (.atoms[1] | IN($y[]))) or ((.atoms[1] | IN($x[])) and (.atoms[0] | IN($y[]))))] | [(map(select(.)) | length), length]'` prints `[22,22]` and `[75,75]`: every residue is bonded to the next of its chain.
- **Groups.** Write `$RUN/groups.json` with `dna` holding `"pair": { "residues": [{ "chain": "A", "number": 6 }, { "chain": "B", "number": 19 }], "part": "nucleobase" }` and `"strand": { "residues": [{ "chain": "B" }] }`, and `ub` holding `"helix": { "residues": [{ "chain": "A", "from": 23, "to": 34 }] }`. `$SKILL/scripts/run "$RUN" pdb-groups-inspect inspect "$RUN/groups.json" -t 0`. `jq -c '[.objects.dna.groups.pair.atoms | length], [.objects.dna.groups.strand.atoms | length], [.objects.ub.groups.helix.atoms | length]'` prints `[19]`, `[243]` and `[98]`: adenine's nucleobase has 10 atoms and thymine's 9, with no hydrogens.
- **Turn.** For each example scene, `$SKILL/scripts/run "$RUN" pdb-turn-<name>-validate validate <scene>`, then `frame` at `-t 1.5`, `-t 5` and `-t 8.5` into `$RUN/<name>-<t>.png`. View each: the molecule turns between them, the lit pair or helix glows yellow at 5 and 8.5, and nothing glows at 1.5. Then `$SKILL/scripts/run "$RUN" pdb-turn-<name>-layout check-layout <scene> --out "$RUN/work/<name>-check"`. Exit `0`, printing `No problem at any of 9 checked scene times.`
- **Errors.** In `$RUN/bad/`: `empty.pdb` holding `HEADER    NOTHING` and `END`; `bad.pdb`, four atom lines of 1UBQ with the third's x coordinate `26.9x3`; and a scene naming both, `ub` with groups `{ "z": { "residues": [{ "chain": "Z" }] }, "far": { "residues": [{ "chain": "A", "number": 99 }] } }`, and caffeine with `{ "a": { "residues": [{ "chain": "A" }] } }`. `$SKILL/scripts/run "$RUN" pdb-errors-validate validate "$RUN/bad/scene.json"`. Exit `1`. `jq -c '[.errors[] | [.path, .code]]'` lists `bad_structure_file` at `/objects/bad/file` and `/objects/empty/file`, `no_such_residue` at `/objects/caf/groups/a/residues/0`, `/objects/ub/groups/far/residues/0/number` and `/objects/ub/groups/z/residues/0/chain`. The message at `/objects/bad/file` names `bad.pdb` and `line 3`.
- **SDF unchanged.** For each scene in `examples/` and `skills/sci-anim/examples/` with a molecule, draw a frame at `-t 2` with this build and with the build before the change (`SCI_ANIM=$(command -v sci_anim)` when that is the installed one), and `cmp` the PNGs. Every pair is identical.
- **Proof.** The `.stdout.json` and `.exit` files, the frames and the `cmp` output in `$RUN`.

## Gotchas

- A PDB file named anything but `.pdb` or `.ent` is read as SDF and fails with the SDF reader's message.
- `check-layout` does not list hidden atoms or end-on bonds of groups on a molecule of more than 200 shown atoms. Look at the frames to see that the lit part shows.
- A frame in the first instants of the highlight's span shows almost no glow. Frame mid-span or after.
- Atom numbers are serial numbers, so `atoms[n - 1]` is not atom `n` past the first gap. Select atoms by `.number`.
