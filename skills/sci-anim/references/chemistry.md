# Chemistry: molecules, atom groups and reactions

Read `instructional-videos.md` first. `../examples/hydration.json` is a complete lesson built from what this file describes: ethylene's double bond lights up, water joins it, and the reaction makes ethanol while the glow moves with the two carbons. Its structure files are in `../examples/structures/`.

## Fetch structure files

The tool never uses the network. Download each molecule's 3D structure file from PubChem before you write the scene, into the scene file's folder:

```sh
curl -fo caffeine.sdf "https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/name/caffeine/SDF?record_type=3d"
curl -fo water.sdf "https://pubchem.ncbi.nlm.nih.gov/rest/pug/compound/cid/962/SDF?record_type=3d"
```

- Always add `?record_type=3d`. The tool refuses a 2D file.
- Use the CID form, `compound/cid/<cid>`, when a name could mean more than one compound.
- A 404 means PubChem has no 3D model. Proteins, DNA and H₂ have none. Fetch a protein or DNA as a PDB file instead ("Proteins and DNA from PDB files" below). For anything else, choose another example, and tell the user why.
- `file` in the scene file is relative to the scene file.

## Find the atoms

Every atom is known by its number in the structure file. To list them, write a scene with only the molecule and inspect it:

```sh
sci_anim inspect m.json -t 0 | jq -c '.objects.caffeine | (.atoms[] | [.number, .element]), (.bonds[] | [.atoms, .order])'
```

Find the atoms of a group by their elements and bonds: a carbonyl is a C double-bonded to an O.

Each atom also has `pixels`, its center on the frame, and from sci_anim 0.11.0 each shown atom has `ball_radius`, the radius in pixels of the disc its ball shows, at the molecule's scale and the camera. A text covers an atom when that disc reaches the text's box, even with the center outside it. To keep a text clear of a molecule, keep every ball's center at least its `ball_radius` from the text's `bounding_box.pixels`:

```sh
sci_anim inspect s.json -t 4 | jq -c '.objects.ethylene.atoms[] | select(.hidden | not) | [.number, .pixels, .ball_radius]'
```

## Molecules on screen

- A molecule's `position` is its centroid. One world unit is one ångström, so a small molecule is 3 to 5 units across. `scale` 1.3 to 1.6 makes a small molecule easier to see. Keep a large one like caffeine at 1.
- `hydrogens: false` hides the hydrogens. Hide them on a big molecule when the hydrogens are not the point. Show them when a reaction moves one.
- Two molecules of one compound, such as the two O₂ of a combustion, are two molecule objects with the same `file`.
- A molecule that will get a caption above it sits at y = 0 or lower, so the caption stays clear of the heading.
- A molecule can fade, move, scale and rotate. It cannot be created. Bring it in with a fade from `opacity` 0.
- `rotate` turns a molecule to the rotation in its `to`, not by it. To turn a molecule once and end where it started, read its `rotation` from `inspect` and add 360 to the first angle only: from `[80, 15, -100]`, rotate to `[440, 15, -100]`. That turns it about the frame's horizontal axis, with `linear` easing over 4 to 6 s, and shows its 3D shape. Adding 360 to another angle turns it about a tilted axis.
- A molecule seen edge-on hides atoms behind atoms. On a molecule of 200 shown atoms or fewer, `check-layout` lists a heavy atom of a named group hidden behind another atom, and a bond between heavy atoms of a named group that points nearly at the camera. Turn the molecule with `rotation` until neither shows.

## Point at atoms

- Name atom groups on the molecule: `"groups": { "carbonyl": [1, 9] }`.
- A `highlight` lights a group's glow to `to` (1 on, 0 off), in its `color`. Use one color per kind of group and keep it.
- A glow is yellow, `#ffd23f`, unless a highlight gives it a `color`. A glow that is off lights up in its highlight's color from the start, and a lit glow moves from its color to the next highlight's.
- A glow covers a group's atoms and the bonds between them. To point at one bond, make a group of its two atoms.
- Pick glow colors that stand apart from the element colors. Carbon is gray, hydrogen white, oxygen red, nitrogen blue, chlorine and fluorine green, sulfur yellow, phosphorus orange, bromine dark red and iodine purple. Yellow `#ffd23f`, green `#83c167` and purple `#9a72ac` read well on carbon, hydrogen, oxygen and nitrogen. Use purple, not green, on chlorine. A caption in purple takes `#b189c6`, which text can be read in.
- A glow reaches about one atom's radius past its atoms. Glows on two molecules side by side merge into one blob, so light them in separate steps or move the molecules apart. `check-layout` lists glows that merge, and glows that cover text or another molecule's atoms.
- Unlight a group before a step that turns its molecule far, or its glow and caption swing into other things.
- A caption is a `text` or `math` with `attach`: it stays beside its group, `offset` right and up of the group's center on the frame, as the molecule turns. A leader `arrow` with the same `attach` points from the caption to the group. Its `from` and `to` are measured from the group center too.
- Put a caption on the side of the group with no atoms, past the glow: about 1.5 units from the group's center at `scale` 1, more at a larger scale. `check-layout` lists a caption that covers atoms or sits in a glow.
- Declare captions and leaders with `opacity` 0 and fade them in with the highlight.
- Fading a molecule out does not fade its captions or leaders. Fade them in the same step. `check-layout` lists a caption left behind.

## Reactions

A `reaction` turns `reactants` into `products` over its step. Each atom moves in a straight line from where it is in a reactant to where it is in a product. Broken bonds thin away, formed bonds grow. The reactants show until the step begins and the products from its end, so neither needs a fade.

The products need an `atom_map`: one row per atom, pairing each reactant atom with the product atom it becomes. Build it like this:

1. List the atoms and bonds of every reactant and product, as above.
2. Write the reaction as a chemist would. Which bonds break, and which form? When the balanced equation allows more than one path, use the textbook one, and say on screen that the video shows the overall change, not each step of the mechanism.
3. Map the heavy atoms first, by element and by what they are bonded to.
4. Map each hydrogen by the heavy atom it is bonded to. A hydrogen that moves to a new atom maps to a hydrogen on that atom in the product.
5. Run `validate`. It lists every unmapped atom, every atom mapped twice and every pair of two elements, at the row at fault.
6. Check the chemistry. Inspect at any scene time and list every bond the reaction does not keep as it was:

```sh
sci_anim inspect s.json -t 0 | jq -c '.reactions[0].bonds[] | select(.fate.type != "kept") | [(.atoms[] | "\(.object) \(.atom)"), .fate]'
```

The broken, formed and changed bonds must be exactly the ones the chemistry says. For ethylene + water: the C=C bond changes to single, one C–O and one C–H bond form, and one O–H bond breaks. Any other bond there means a wrong row. Swapping two hydrogens on the same atom changes nothing chemically but can make them cross. Swap them if `check-layout` lists them passing through each other.

- Place the reactants apart, with room for the products between them. Put each product near the reactants that give it most of its atoms. Atoms move in straight lines, so a product on the far side of another molecule sends atoms flying through it. `check-layout` lists two unbonded atoms that pass within 0.5 Å of each other in the middle of a reaction, and two whose balls rest less than 0.35 Å apart at the end of a step, such as a product set down against a molecule beside it.
- Each product ends at its own `position`, turned by the fit and then by its own `rotation`. The fit is one rotation for all the products together, the one that lines them up best with the reactants, so two products of one compound with the same `rotation` face the same way. Moving a reactant changes the fit, so run `check-layout` again after you move one. To show a product from a better side, give it a `rotation`: it turns the product about the world's axes after the fit, so `[0, 180, 0]` turns it half round the vertical and keeps whatever tilt the fit gave it. Draw a `frame` at the reaction's end to see which way each product faces before you choose. `inspect` reports the product's rotation once the reaction ends, which is where a later `rotate` starts from.
- To place a product by hand instead, give the reaction `"fit": false`: each product then ends at exactly its own `position` and `rotation`, and `inspect` reports that `rotation` after the reaction. `fit_rmsd` then says how far the products, as their rotations turn them, lie from the reactants.
- A reactant must be at `opacity` 1 and at a scale the same on every axis when its reaction begins.
- A lit group keeps its glow through a reaction. The glow moves with the group's atoms, and at the step's end the product group of the same atoms glows in the same color. Use it to show where an atom goes: light its group before the reaction, and unlight the product group after it.
- Each product that receives atoms of a lit group needs a group of exactly those atoms. Find them in the atom map: the product atoms the group's atoms pair with. `validate` names the product and the atom numbers to declare. A group split across two products needs a group in each.
- Two lit groups cannot pass their glow to one product group. Do not highlight a reactant or product group in the reaction's own step.
- `inspect` lists a reaction's carried glows in `reactions[].glows`, each with the product groups it `passes_to`.
- Give the reaction 3 to 4 s, and wait 1.5 s after it.
- A product can be a reactant in a later reaction, for a mechanism in several steps.

### Every bond broken at once

To count the atoms, or to set the energy of the bonds broken against that of the bonds formed, show the reactants coming apart into loose atoms, then the loose atoms joining into the products: two reactions, through one molecule object that has every atom and no bond. Write its structure file by hand, in the scene file's folder. Copy a fetched file's first three lines, keeping `3D` on the second, then a counts line with the atom count and 0 bonds, one line per atom at the position it should rest at, and `M  END`:

```
loose atoms
  sci_anim-hand-made-3D

  3  0  0     0  0  0  0  0  0999 V2000
   -2.0000    0.0000    0.0000 O   0  0  0  0  0  0  0  0  0  0  0  0
    0.0000    0.0000    0.2000 C   0  0  0  0  0  0  0  0  0  0  0  0
    2.0000    0.0000    0.0000 O   0  0  0  0  0  0  0  0  0  0  0  0
M  END
$$$$
```

Give both reactions `"fit": false`. The fit would turn the loose atoms about their middle to lie close to the reactants, and then the products to lie close to the loose atoms, so the atoms would not rest where the file puts them, and a ring laid flat would rest tilted. With the fit off, the loose atoms rest at their own `position` and `rotation`, as the file lays them out, and each product ends at its own.

Rest each loose atom away from both where it starts and where it ends, such as spread on a wide ring around the middle, so the viewer sees the atoms come apart in the first reaction and gather to their new partners in the second. Atoms that rest where the products will be make the second reaction a still frame whose bonds just appear. Keep loose atoms at least 1.5 Å apart in the file: two oxygens 1 Å apart look like an O₂ molecule with no stick. `check-layout` lists two atoms with no bond between them, in one molecule or two, whose balls rest less than 0.35 Å apart at the end of a step, since they read as bonded. Give each of the two reactions 3 to 4 s.

Say on screen that a real reaction does not pull every atom apart: the loose atoms show which bonds break and which form, not a step of the mechanism.

## Polymers

PubChem has no 3D model of a polymer chain, and sci_anim makes no structure. Write the chain's structure file yourself with `../scripts/straight_chain.go` before you write the scene. `../examples/polyethylene.go` is a complete lesson: a methyl radical opens one ethylene after another, the chain grows by three repeating units, and the first unit stays lit while it grows. `../examples/polystyrene.json` lights a polystyrene chain's rings one after another, and `../examples/pet.json` lights the rings, ester groups, one repeating unit and the end groups of a PET chain.

### Write a short chain

Name the monomer and how many backbone atoms the chain has, and run `straight_chain.go` in its folder, `../scripts/`. It needs Go and nothing else:

```sh
cd <skill folder>/scripts && go run straight_chain.go ethylene 10 <scene folder>/decane.sdf
```

| Monomer | Plastic | Repeating unit | Backbone atoms a unit |
|---|---|---|---|
| `ethylene` | polyethylene | CH₂–CH₂, no side groups | 2 |
| `propylene` | polypropylene | a CH₃ on every second carbon | 2 |
| `vinyl-chloride` | PVC | a Cl on every second carbon | 2 |
| `tetrafluoroethylene` | PTFE | two F on every carbon | 2 |
| `styrene` | polystyrene | a benzene ring on every second carbon | 2 |
| `ethylene-terephthalate` | PET | –O–CO–C₆H₄–CO–O–CH₂–CH₂–, a benzene ring in the backbone | 7, the ring counting as one |

- A ring in the backbone counts as one backbone atom in the length, so a chain never ends inside a ring. A PET chain of n units plus one, such as 15, ends in an acid, –COOH, and an alcohol, –OH, as a real PET chain does. PET is made from two monomers, terephthalic acid and ethylene glycol, and `ethylene-terephthalate` names the repeating unit they give.
- The file comes from ideal bond lengths and angles, not from a force field's minimum: C–C 1.54 Å, C–H 1.09 Å, C–F 1.35 Å, C–Cl 1.77 Å, 1.39 Å round a benzene ring with its C–H 1.08 Å, 1.51 Å from a chain carbon to its ring, C=O 1.21 Å, 1.48 Å from a ring to a carbonyl carbon, 1.34 Å from a carbonyl carbon to its ester oxygen, 1.44 Å from that oxygen to CH₂, and O–H 0.96 Å. The angle is a tetrahedral 109.5° at a CH₂ or CH, 120° at a carbonyl carbon and 117° at an ester oxygen. Every backbone dihedral is 180° (all anti), so the backbone is a straight zig-zag. A real chain coils, and a real PTFE chain twists into a slow helix, since its fluorines crowd each other on a straight backbone. Say so when the lesson is about a chain's real shape.
- It lays the backbone along x, its first atom on the left, with the zig-zag in the plane z = 0, so the default camera sees the zig-zag face on. A ring in the backbone lies in that plane too, as PET's rings lie flat with their ester groups in a PET crystal.
- A benzene ring is written with alternating single and double bonds, which sci_anim draws as one stick and two sticks side by side in the ring's plane, so it reads as benzene. sci_anim refuses the SDF code for an aromatic bond.
- The side groups alternate sides along the chain (syndiotactic). Two side groups on one side of a straight backbone would lie 2.5 Å apart. Most polystyrene sold puts each ring on a random side (atactic), so say on screen that the chain shown alternates.
- A polystyrene ring lies square to the backbone, as in polystyrene's zig-zag crystal: turned toward the backbone, its hydrogens would come within 0.7 Å of the backbone's or the next ring's. The default camera therefore sees each ring edge on, and `check-layout` says its bonds point nearly at the camera. Turn the chain with `rotation` `[75, 0, -30]`, as `../examples/polystyrene.json` does: the rings show as hexagons, taking turns above and below the backbone. Turned further, as `[90, 0, -40]`, backbone carbons hide behind each other, and turned less, as `[75, 0, -25]`, the rings are edge on again.
- Each end atom's last bond takes a hydrogen, so a polyethylene chain ends in two CH₃ and its end groups are those two hydrogens, and a chain that ends on an oxygen ends in –OH. `-radical` leaves the last backbone atom with an unpaired electron in place of that hydrogen. A real chain has thousands of units, and its end groups come from how it was made, such as the initiator. Say on screen that the chain shown is short, and what its end groups are.
- A monomer missing from the table is not supported. `straight_chain.go` stops, writes no file, and names the monomers it knows.
- The same arguments write the same file. Commit the structure file beside the scene file.

### Lay the chain out

A straight polyethylene chain is about 1.3 Å long per backbone carbon, plus 1 Å with its hydrogens shown. This is the longest chain at each resolution and `scale` that passes `check-layout` between a heading and a sentence at the default camera:

| Resolution | Chain along | `scale` | Hydrogens shown | Hydrogens hidden |
|---|---|---|---|---|
| 1920×1080 | x, as the file lays it | 1 | 10 carbons, 5 units | 10 carbons, 5 units |
| 1920×1080 | x | 1.4 | 6 carbons, 3 units | 8 carbons, 4 units |
| 1080×1920 | y, `rotation` `[0, 0, 90]` | 0.7 | 6 carbons, 3 units | 8 carbons, 4 units |
| 1080×1920 | y | 1 | none | 6 carbons, 3 units |

- Three units is the fewest that reads as a unit repeated. Four or five read better when they fit.
- A 1080×1920 frame at the default camera shows y from −4.14 to 4.14, as a 1920×1080 frame does, but x only from −2.33 to 2.33. Lay the chain along y there.
- Show the hydrogens when the lesson is about the units, so each reads as CH₂–CH₂. With them hidden the chain reads as a zig-zag of carbons, which fits a longer chain.
- The table is for polyethylene. A side group, such as polypropylene's CH₃, widens the chain and moves its centroid off the backbone, so run `check-layout` on a chain with side groups before you settle its length.

Polystyrene's outermost ring hydrogens lie 9.2 Å apart across the backbone, and a PET unit is 11.1 Å long. The longest of each that passes `check-layout` between a heading and a sentence, in backbone atoms with a backbone ring counting as one:

| Monomer | Resolution | `rotation` | `scale` | Hydrogens shown | Hydrogens hidden |
|---|---|---|---|---|---|
| `styrene` | 1920×1080 | `[75, 0, -30]` | 0.5 | 17 carbons | 19 carbons |
| `styrene` | 1080×1920 | `[0, 75, 60]`, along y | 0.35 | 13 carbons | 13 carbons |
| `ethylene-terephthalate` | 1920×1080 | none | 1 | 8, one unit | 9 |
| `ethylene-terephthalate` | 1920×1080 | none | 0.55 | 16, two units | 16 |
| `ethylene-terephthalate` | 1080×1920 | `[0, 0, 90]`, along y | 0.4 | 9 | 9 |

- `[0, 75, 60]` is `[75, 0, -30]` turned a quarter turn in the frame's plane. Rotation angles turn about the molecule's own axes, x first, so the two do not differ in one angle.
- These chains light no atom group. `check-layout` also says when an atom of a group hides behind another, which it does not check for atoms in no group. An 8-carbon chain at `scale` 0.55 that lights each unit and ring passes, as in `../examples/polystyrene.json`.
- Past about 9 carbons, a turned polystyrene chain's far rings lie over each other. Show four rings, or fewer, when the lesson is about them.
- A PET chain of 15, two units with an acid end and an alcohol end, fits at `scale` 0.55, as in `../examples/pet.json`.

### Point at the repeating units

`straight_chain.go` numbers the backbone atoms first, left to right, then the side groups' heavy atoms, then every hydrogen, grouped by the atom it is on, in the order of that atom's number. A ring in the backbone gives its six carbons round the ring from the one the backbone enters by, so the backbone leaves by the fourth. A phenyl gives its six carbons round the ring from the one bonded to the backbone. To list each heavy atom of a chain with its hydrogens:

```sh
sci_anim inspect m.json -t 0 | jq -c '.objects.chain | (.atoms | map(select(.element == "H") | .number)) as $h | .bonds[].atoms | select(.[1] | IN($h[]))' | jq -sc 'group_by(.[0]) | map([.[0][0], map(.[1])])[]'
```

For a polyethylene chain of 10 carbons it prints `[1,[11,12,13]]`, then `[2,[14,15]]`, and so on, so unit k is carbons 2k − 1 and 2k with their hydrogens and side groups.

For the polystyrene chain of 8 carbons in `../examples/structures/polystyrene.sdf`, atoms 1 to 8 are the backbone, then each phenyl's six carbons in turn, 9 to 14 on carbon 2 up to 27 to 32 on carbon 8, then the hydrogens: 33 to 46 on the backbone, and five on each ring, 47 to 51 on the first. Ring k is atoms 6k + 3 to 6k + 8 with hydrogens 5k + 42 to 5k + 46.

For the PET chain of 15 in `../examples/structures/pet.sdf`, a unit is seven backbone atoms, the ring counting as one, numbered 1 to 12: O 1, the carbonyl carbon 2, the ring 3 to 8, entered at 3 and left at 6, the carbonyl carbon 9, the ester oxygen 10, and CH₂ 11 and 12. The second unit is 13 to 24, and the last oxygen 25. The four carbonyl oxygens come next, 26 to 29, one on each of carbons 2, 9, 14 and 21. Then the hydrogens: 30 on oxygen 1, 31 to 34 on the first ring, 35 to 38 on its CH₂s, and on.

- Give each unit an atom group, `"unit_1": [1, 2, 11, 12, 13, 14, 15]` and on. An end carbon's three hydrogens go in its unit, since no frame shows which of them is the end group.
- Light the units one after another. Each step lights the next unit to 1 and the last to 0, in 0.6 to 0.8 s. Light one unit at a time, since the glows of two units side by side touch.
- Give each end group an atom group of its own, and light the ends in another color when the sentence names them.

### Polymerization as reactions

Every atom of the reactants is an atom of the products, so five ethylenes cannot become decane: C₁₀H₂₀ is not C₁₀H₂₂. Show the textbook chain growth instead. An initiator radical opens one monomer's double bond, and each new radical end opens the next. Each step is one reaction, of the chain so far and one monomer, whose product is the reactant of the next.

- Write every chain with `straight_chain.go -radical`, as `../examples/structures/NOTICE` shows: ethylene chains of 1, 3, 5 and 7 carbons are the methyl, propyl, pentyl and heptyl radicals. A chain's first atoms lie at the same file positions whatever its length, so each chain holds the atoms it shares with the next at the same place.
- A molecule's `position` is its centroid, and each chain's centroid is somewhere else. Give each chain the position its own centroid has in the shared file positions, times its `scale`, plus one origin for all of them, and give every reaction `"fit": false`. The atoms the chain already has stay where they are, and only the monomer's atoms move.
- Map each atom of the chain so far to the atom at the same place in the next chain's file. Map the monomer's carbon nearer the radical end to the next chain's first new carbon, and the other to the second. Map each hydrogen by its carbon, to the nearer hydrogen of its partner. A scene generator can read the structure files and write these rows, as the example's does.
- Each step's bond fates are one formed single C–C bond, from the radical end to the monomer's near carbon, and the monomer's C=C changed to single. Check every reaction, not only the first:

```sh
sci_anim inspect s.json -t 0 | jq -c '.reactions[] | [.bonds[] | select(.fate.type != "kept") | [(.atoms[] | "\(.object) \(.atom)"), .fate]]'
```

- Set the monomer beside the radical end, turned with `rotation` `[0, 0, angle]` so its C=C lies along the bond it becomes, about 1.2 Å out along the chain and 1.3 Å up from the place it will take. Keep it inside the frame and clear of the heading.
- The tool draws no unpaired electron. Give each chain a caption `•` on its radical-end group, out from the end carbon along the way the next bond goes, about 1.3 units at `scale` 1.2, past the glow. A caption lives as long as its molecule, so the unpaired electron moves to the new end as each reaction ends.
- Light the first unit after the first reaction, and it stays lit through every later one: each later chain needs the same group, which `validate` names.
- Say on screen what the chain ends in, that a real chain grows to thousands of units, and that it stops when its radical end meets another radical, which the scene does not show.

### Thermoplastics and thermosets

A thermoplastic's chains only tangle, so heat lets them slide past each other. A thermoset's chains are joined by cross-links, so they cannot. Draw the chains as tubes, not molecules: a few hundred atoms per chain is too many to read. Wiggle each chain with a shape law, and draw each cross-link as a `line` whose ends sit on the two tubes at a `u`, with sci_anim 0.13.0 or later. `../examples/cross_links.json` joins two wiggling chains with five cross-links that stay on their two atoms as the chains move and turn. `physics.md` covers ends on a curve. For the thermoplastic, draw the same chains with no cross-links and pull them apart with a `move`.

## Hydrogen bonds and nucleobase pairs

A hydrogen bond is not one of a molecule's bonds, so draw it as a dashed `line` whose two ends sit on atoms, with sci_anim 0.14.0 or later: `{ "object": "at_pair", "atom": 13 }` is atom 13 of that molecule, numbered as an atom map numbers atoms. The line stays on its two atoms as the molecule moves, turns and scales:

```json
"at_bond_1": { "type": "line", "from": { "object": "at_pair", "atom": 13 }, "to": { "object": "at_pair", "atom": 21 },
  "stroke": { "color": "#ffd23f", "width": 0.05, "dash": [0.08, 0.06] } }
```

- Draw it from the hydrogen to the atom it points to, the nitrogen or oxygen across the gap, about 1.9 Å. At `scale` 0.45 dashes of 0.08 and gaps of 0.06 give about six dashes.
- PubChem has a structure file of each nucleobase but not of a pair. Write a Watson-Crick pair with `../scripts/nucleobase_pair.go`, which needs Go and nothing else: `cd <skill folder>/scripts && go run nucleobase_pair.go AT <scene folder>/at_pair.sdf`, or `GC`. The pair lies flat, its hydrogen bonds across the middle, the purine's atoms first. The file's third line lists each hydrogen bond as the atom numbers of its hydrogen and the atom it points to, such as `hydrogen bonds 13-21 26-7`: A-T has two and G-C three.
- One file holds both nucleobases, so one `rotate` turns the pair about its middle and the hydrogen bonds turn with it. `../examples/nucleobase_pairs.json` turns an A-T pair and a G-C pair with their hydrogen bonds on, and `../examples/structures/` holds both files.
- An end may sit on an atom of two molecules, such as a water molecule's hydrogen and an ethanol's oxygen. Move them apart and the bond stretches with dashes of the same length.
- An atom past the molecule's atoms is `no_such_atom` at the end's `atom`, and an `atom` on an object that is not a molecule is `invalid` at its `object`. The end is hidden while its molecule is: a reaction that uses the molecule up hides it from the reaction's start, and an end on a product shows from the reaction's end.

## Proteins and DNA from PDB files

PubChem has no 3D model of a protein or of DNA. The Protein Data Bank has the structures experiments found, as PDB files, which sci_anim 0.17.0 or later reads. Fetch one by its four-character PDB id:

```sh
curl -fo 1ubq.pdb https://files.rcsb.org/download/1UBQ.pdb
```

- The file's name must end in `.pdb` or `.ent`. Any other name is read as SDF.
- sci_anim reads the first model, leaves out the waters, and keeps the first alternate location of an atom the experiment found in two places. It bonds the atoms of the `CONECT` records, and in amino acids and nucleotides the atoms whose distances show a bond, within a residue and to the next residue of the chain. Every bond draws single. A PDB file from an X-ray structure has no hydrogens, and the molecule draws without them.
- An atom's number is the serial number the file gives it, which skips some numbers: a `TER` record between chains takes one, and the waters are left out. Atom maps and ends on atoms use these numbers.
- `../examples/dna_double_helix.json` turns the Dickerson dodecamer, `1bna.pdb`, with one nucleobase pair lit, and `../examples/ubiquitin.json` turns ubiquitin, `1ubq.pdb`, with its α-helix lit. Both files are in `../examples/structures/`.
- A protein of hundreds of atoms reads better as a cartoon ("Draw a protein as a cartoon" below), or as its chain alone: a `tube` whose `curve` is the atom trace through its alpha carbons, `{ "trace": { "file": "structures/1ubq.pdb", "atom_name": "CA" } }`, with sci_anim 0.19.0 or later. A tube with the molecule's `position`, `rotation` and `scale` runs through its alpha carbons, so the two can stand side by side or one over the other. A straight chain morphs into the trace to fold: `../examples/ubiquitin_fold.go` folds 76 curve points into ubiquitin. `solids.md` covers both.

### Find the residues

A PDB file names its atoms by chain, residue and atom name. `inspect` lists each chain's residues, each with its residue number, residue name and an object from atom name to atom number:

```sh
sci_anim inspect m.json -t 0 | jq -c '.objects.dna.chains[] | .chain as $c | .residues[] | [$c, .number, .name]'
sci_anim inspect m.json -t 0 | jq -c '.objects.dna.chains[0].residues[3].atoms'
```

The second prints `{"P":58,"OP1":59,...,"N1":75,...}`, so an end on that guanine's N1 is `{ "object": "dna", "atom": 75 }`. The file says where its helices and strands lie, by chain and residue number: `grep -E '^(HELIX|SHEET)' 1ubq.pdb` prints `HELIX 1 H1 ILE A 23 GLU A 34`, the helix from residue 23 to residue 34 of chain A, and from sci_anim 0.22.0 `inspect` lists them as `secondary_structure`. A DNA duplex's two chains pair end to end: in the dodecamer, residue k of chain A pairs with residue 25 − k of chain B.

### Light residues

Write an atom group by its residues instead of its atom numbers, as a list of residue picks:

```json
"groups": {
  "helix": { "residues": [{ "chain": "A", "from": 23, "to": 34 }] },
  "pair": { "residues": [{ "chain": "A", "number": 6 }, { "chain": "B", "number": 19 }], "part": "nucleobase" },
  "strand": { "residues": [{ "chain": "B" }] }
}
```

- A pick is one residue, `number`; a run, `from` and `to`, both included; or a whole chain, `chain` alone.
- `"part": "nucleobase"` holds only each nucleotide's nucleobase, without its sugar and phosphate. Light a nucleobase pair that way: lit whole, its two nucleotides light a stretch of the backbone too.
- A chain or residue number the file does not have is `no_such_residue` at the pick's field, and the message lists the chains or the chain's residue numbers. A molecule read from an SDF file has no residues.
- A glow over a hundred atoms in a crowded molecule reads as a tint. Light one helix or one pair at a time, in a color that stands apart, and say on screen what is lit.

### Draw a protein as a cartoon

A protein of hundreds of atoms reads best as a textbook draws it: `"representation": "cartoon"`, with sci_anim 0.22.0 or later. Each chain's backbone is one tube through its alpha carbons, a thin gray circle along coil, a flat red ribbon along each helix, its flat side facing out from the helix's axis, and a flat blue arrow along each β-strand, lying in its β-sheet's plane and pointing to the strand's end. DNA and RNA draw as a tube through their phosphorus atoms.

```json
"protein": { "type": "molecule", "file": "structures/1ubq.pdb", "representation": "cartoon", "scale": [0.16, 0.16, 0.16],
  "groups": { "helix": { "residues": [{ "chain": "A", "from": 23, "to": 34 }] } } }
```

- The helices and β-strands come from the file's `HELIX` and `SHEET` records. A file with none draws as coil, with no error. `inspect` lists them, each with its residue range: `jq -c '.objects.protein.secondary_structure[]'` prints `{"kind":"helix","id":"H1","class":1,"chain":"A","from":23,"to":34}` for ubiquitin's α-helix and `{"kind":"strand","sheet":"BET","strand":1,"chain":"A","from":10,"to":17}` for one of the five β-strands of its β-sheet. A helix of `class` 1 is an α-helix, and of class 5 a 3₁₀ helix.
- Light a helix or a β-sheet with a group of its residue ranges, copied from `secondary_structure`: a highlight lights the tube along each residue whose `CA` the group holds. A group with no `CA` is `group_hidden`.
- No atom draws as a ball, so `inspect` reports every atom as hidden, `hydrogens` does nothing, and a caption attached to a group sits at the middle of the group's atoms.
- From sci_anim 0.23.1 `check-layout` judges a text over a cartoon by the cartoon's box, as it judges a text beside a circle, and names it as `"label" and "protein" overlap`. The box holds the gaps between the ribbons too, so keep a heading or a sentence clear of the whole protein.
- A molecule read from an SDF file cannot be a cartoon: `invalid` at its `representation`. Nor can a reactant or a product of a reaction.
- `../examples/ubiquitin_cartoon.json` turns ubiquitin as a cartoon, with its α-helix and then its β-sheet lit.

### Proteins and DNA on screen

- One world unit is one ångström. The dodecamer is about 42 Å long and 25 Å across, and ubiquitin 30 to 37 Å across. At 1920×1080, between a heading and a sentence, the dodecamer fits at `scale` 0.21 laid along x with `rotation` `[0, 90, 0]`, and ubiquitin at `scale` 0.16.
- Turn the dodecamer about its own axis from `[0, 90, 0]` to `[360, 90, 0]`, and ubiquitin about the vertical from `[0, 0, 0]` to `[0, 360, 0]`, with `linear` easing over 6 to 8 s.
- In a molecule of more than 200 shown atoms some atoms of any group hide behind others whichever way it turns, so `check-layout` does not list hidden atoms or end-on bonds of its groups. Look at the frames to see that the lit part shows.

### Space-filling

A molecule of thousands of atoms reads as a tangle in balls and sticks. `"representation": "space-filling"`, from sci_anim 0.21.0, draws each shown atom as a ball at its element's van der Waals radius (carbon 1.7 Å, nitrogen 1.55, oxygen 1.52, phosphorus 1.8, hydrogen 1.2) and no sticks, so the molecule shows the space it fills: the grooves of DNA, the surface of a protein.

```json
"dna": { "type": "molecule", "file": "structures/1kx5_dna.pdb", "representation": "space-filling", "scale": [0.05, 0.05, 0.05],
  "groups": { "strand": { "residues": [{ "chain": "J" }] } } }
```

- Keep ball-and-stick, the default, for a molecule whose bonds the lesson talks about. Space-filling hides the bonds, and a reaction refuses a space-filling reactant or product.
- `hydrogens` works as in ball-and-stick. A PDB file from an X-ray structure has none. An SDF file's hydrogens show as balls of 1.2 Å; hide them with `"hydrogens": false` when the heavy atoms' shape is the point.
- A glow lies close over its group's balls, shaded like them, and colors them, so a whole strand or a run of residues reads among thousands of unlit atoms. Light a chain or a run, and say on screen what is lit.
- `inspect` reports `"representation": "space-filling"`, each shown atom's `ball_radius` at its whole van der Waals radius, and every bond hidden, since none is drawn.
- `check-layout` judges a text against the discs of the molecule's balls, which together cover what the molecule covers in the frame. It does not list unbonded atoms that touch, atoms hidden behind others or bonds that point at the camera: overlapping and hiding are what space-filling draws.
- `../examples/nucleosome_dna.json` turns the 147 nucleobase pairs of DNA that wrap a nucleosome, read from PDB entry 1KX5 without its histone proteins, with one strand lit. It is 6,021 atoms, about 108 Å across and 64 Å along its axis, and fits at `scale` 0.05 between a heading and two lines of text. It renders at about 65 ms a frame at 1920×1080, against about 20 for the dodecamer in ball-and-stick: a space-filling molecule's balls are cut into only as many triangles as its largest ball needs to look round at the size it shows.

## Equations for reactions

Write a reaction scheme as math with `\mathrm`: `"\\mathrm{C_2H_4} + \\mathrm{H_2O} \\rightarrow \\mathrm{C_2H_5OH}"`. mhchem's `\ce` does not work. `math.md` covers the rest of text and math.
