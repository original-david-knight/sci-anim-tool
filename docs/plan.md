# Plan

This is the plan for sci_anim. It lists the work not yet built, in the order to build it. What is built is described by the skill, `sci_anim schema` and the code. Terms follow `GLOSSARY.md`.

The agents that make videos cannot ask for features, so this plan adds what the next videos will reach for.

## Rules every step keeps

- Each step lands on main, proven on the real binary with `.claude/skills/verify-sci-anim/` as `AGENTS.md` says, before the next step starts. The step adds its feature to that skill's feature map.
- No Python. Scripts and scene generators are written in Go.
- Drawing a frame is a pure function of the scene and the scene time, so any frame renders on its own.
- The tool validates the scene file once, when it reads it. Code past that point trusts its types.
- Every object turns into triangles plus a material and a transform before drawing, so every kind of object reaches the renderer the same way.
- A new object type, animation type, field or flag gets a glossary term, schema descriptions, the skill's teaching, a feature map recipe and an example scene that validates, in the same change. Bump the minor version when an item lands, and name it in the skill as the least version the feature needs. After it lands, run `cargo install --path . --locked`.

## Next

Each item names what an agent can then do, the shape to build, and the proof beyond the rules above. An item keeps its number when the items before it land, so a reference such as "once item 2 lands" stays true.

Nothing is planned. Add work here as videos need it.

## Known limits

- A carry along a tube with `faces: tangent` reports Euler angles that swing fast where the carried object's z axis lies along the world's x axis, where Euler angles lose one of their three, though the frame turns it smoothly. A copy that faces a space curve reports the principal Euler angles of its own rotation, each copy on its own, so two neighbors' angles can differ by far more than the copies turn, and under a shape law a copy near that lock swings the same way.
- Copies and carries that face a space curve never twist about it, so on a helix their y axis turns against the helix's axis, 53 degrees a turn on a helix of radius 0.8 rising 0.12 per radian. Rungs that point at the axis are turned from their place instead. Copies turned to keep a named direction square to the tangent could face both the curve and the axis.
- `inspect` of a pattern of 8,000 spheres prints about 8 MB, one entry a copy, measured on a 20×20×20 lattice, and `stages` prints that again at every frame time of a follow.
- A morph of a tube moves each curve point in a straight line, so a folding chain passes through itself on the way, and a tube of expressions in `u` or on 3D axes is not morphed. A tube of expressions could be sampled at as many u as the other tube has curve points.
- A shape law is not morphed, and nothing is carried along it: a carry copies the curve its `along` object declares. A surfer riding a travelling wave needs a carry that reads the curve at each scene time.
- A follow of a shape that begins at a law time away from where the last left it, as a second follow with no `from` does, jumps the shape there with no scene error, unlike a law's `law_jump`, because no measure of how far two shapes lie apart is defined yet.
- A trail keeps its points in the world, so moving the axes of a data point leaves the trail where the point was. A trail in the axes' data coordinates would move with them.
- A tube object's cross-section is always a circle. A cartoon carries one that changes along its curve, flat and with steps, which a `tube` could take as a field to draw a ribbon of its own. A cartoon colors each part by its kind, with no colors a scene sets, and draws a helix of every `HELIX` class as the same ribbon. Its ribbon is turned at each alpha carbon by the atoms around it, unlike copies facing a curve, so it twists as the chain does.
- A PDB file of more than 99,999 atoms writes its serial numbers in hybrid-36, which sci_anim does not read. A residue pick names residues by number alone, so `52` holds both 52 and 52A. A residue that is not an amino acid or a nucleotide is bonded only by `CONECT` records, which a file may leave out for a ligand.
- A ball-and-stick molecule cuts every ball into 1,024 triangles however small it shows, so its frames stay byte for byte what they were. Space-filling cuts its balls only as finely as its largest ball shows. The same rule would speed a ball-and-stick molecule of thousands of atoms.
- `inspect` measures a ball's disc at its center's depth, so a ball near the frame's edge shows up to about 7% wider than its `ball_radius`, as perspective stretches it. `check-layout` judges text against these discs.
- A simulation is integrated at eight steps a frame whatever its rates, so a stiff system, such as a stiff spring beside a slow one, needs a short span or gentler numbers. An adaptive step would lift that.
- A scene rendered on two GPUs may not give byte-identical video. Nothing in sci_anim relies on it, and the reports of `frame`, `frames` and `render` name the GPU that drew the picture in `gpu`, so a difference can be explained.
- `frames` turns each 16-bit frame of a transparent scene into 8-bit straight alpha on the processor, about 55 ms of processor time a frame at 3840 by 2160, spread over 8 threads. On a machine with few cores that bounds a 4K frame sequence, so converting on the GPU would speed it there.

## Later

Later, as videos need them: see-through solids that sort their own faces, a plane that cuts a solid open, laws for color, parametric surfaces such as orbitals, and textures on solids.

## Out of scope for now

- A live preview window in sci_anim.
- Network access from sci_anim.
- Generating 3D structure from SMILES, and physically accurate reaction paths.
- Molecular surfaces, and folding a protein from its sequence. Proteins are drawn from their PDB files.
- Compositing images, footage and narration with sci_anim's picture. Another tool does it from the render or the frame sequence.
