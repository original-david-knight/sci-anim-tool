# Write polymer chains

The skill's `scripts/straight_chain.go` writes the structure file of a straight polymer chain from ideal bond lengths and angles, for a monomer it knows and a length in backbone atoms, a ring in the backbone counting as one. A polystyrene chain carries a benzene ring on every second carbon, on alternate sides. A PET chain's backbone runs through a benzene ring between two ester groups. sci_anim draws each ring's alternating single and double bonds, so it reads as benzene, and the two example scenes light the rings, ester groups, units and end groups of each.

## Sub-features

- `chain-same` the four older monomers, `ethylene`, `propylene`, `vinyl-chloride` and `tetrafluoroethylene`, write the same bytes as main's generator.
- `chain-phenyl` `styrene` writes a ring side group on every second carbon, square to the backbone, on alternate sides.
- `chain-ring-backbone` `ethylene-terephthalate` writes a flat PET chain whose backbone passes through each ring, entered at one ring carbon and left at the one across from it.
- `chain-layout` the two example scenes, `skills/sci-anim/examples/polystyrene.json` and `skills/sci-anim/examples/pet.json`, pass `check-layout`, and a polystyrene chain seen from the default camera does not.
- `chain-unknown` a monomer the generator does not know writes no file and names the ones it knows.

## How to get to it (user POV)

- `go run straight_chain.go [-radical] <monomer> <length> <file.sdf>` in `skills/sci-anim/scripts/`, as `skills/sci-anim/references/chemistry.md` teaches under "Polymers".
- A scene file names the written file as a molecule object's `file`. `sci_anim validate`, `check-layout` and `frame` drive it.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Go is on PATH. `G=skills/sci-anim/scripts/straight_chain.go`.

- **Same as main.** `git show main:$G > "$RUN/work/main_chain.go"`. For each of the four older monomers, each length 1, 2, 3, 4, 5, 7, 10 and 13, with and without `-radical`, run `go run "$RUN/work/main_chain.go"` into `$RUN/work/main/` and `go run $G` into `$RUN/work/new/`, each file named the same, as `ethylene-7-r.sdf`. `diff -r "$RUN/work/main" "$RUN/work/new"` prints nothing and exits `0`, over 64 files.
- **Polystyrene.** `go run $G styrene 8 "$RUN/polystyrene.sdf"`, then `cmp` it with `skills/sci-anim/examples/structures/polystyrene.sdf`: the same. Its counts line is ` 66 69`, and 12 of its bond lines end in `  2  0`, three double bonds on each of its four rings. `$SKILL/scripts/run "$RUN" chain-layout-polystyrene check-layout skills/sci-anim/examples/polystyrene.json --out "$RUN/polystyrene-check"`. Exit `0`, `No problem at any of 24 checked scene times`. `$SKILL/scripts/run "$RUN" chain-phenyl-7.0 frame skills/sci-anim/examples/polystyrene.json --scene-time 7.0 -o "$RUN/polystyrene-7.0.png"` and the same at `7.7`. View both: the first ring is lit below the backbone, then the second above it, each a hexagon of six carbons with five hydrogens.
- **Seen edge on.** `jq '.objects.chain.rotation = [0, 0, 0]' skills/sci-anim/examples/polystyrene.json > skills/sci-anim/examples/edge-on.json`, then `$SKILL/scripts/run "$RUN" chain-layout-edge-on check-layout skills/sci-anim/examples/edge-on.json --ends-only --no-sheet --out "$RUN/work/edge-on"` and remove `edge-on.json`. Exit `1`, with lines such as `the bond between atoms 11 and 12 of "chain" points nearly at the camera`.
- **PET.** `go run $G ethylene-terephthalate 15 "$RUN/pet.sdf"`, then `cmp` it with `skills/sci-anim/examples/structures/pet.sdf`: the same. Its counts line is ` 47 48`, and 10 of its bond lines end in `  2  0`, three on each ring and the four C=O. `$SKILL/scripts/run "$RUN" chain-layout-pet check-layout skills/sci-anim/examples/pet.json --out "$RUN/pet-check"`. Exit `0`, `No problem at any of 21 checked scene times`. Draw `frame` at scene times `2.7` and `11.3` and view them: both rings lit yellow in the backbone, then one repeating unit lit green from its ester oxygen to its second CH₂.
- **Unknown monomer.** `go run $G nylon-6 4 "$RUN/work/nylon.sdf" 2> "$RUN/chain-unknown.stderr"`. `go run` exits `1` and its last line is `exit status 2`. The first line is `straight_chain.go knows no monomer "nylon-6"`, the third lists the six monomers, and `$RUN/work/nylon.sdf` does not exist.
- **Proof.** The `.stdout.json` and `.exit` files, the written structure files, the contact sheets and the frames in `$RUN`.

## Gotchas

- `check-layout` says an atom hides behind another only for atoms in an atom group, so a chain with no groups can pass at a turn where the example's lit units fail.
- A variant scene must sit beside `structures/`, so write it in `skills/sci-anim/examples/` and remove it after, or copy `structures/` beside it.
- `go run` turns the generator's exit status 2 into its own 1. Read the last line of stderr for the generator's own.
