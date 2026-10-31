# Morph one object into another

A `morph` reshapes its object into the shape, colors and place of its `into` object over its span. Between two text or math objects, the glyphs that show the same characters slide into place, and each glyph with no partner fades out or in where it stands, so no glyph is bent into another. Any other two shapes, such as a square and a circle, or math and a circle, pair their paths in order and reshape point by point.

## Sub-features

- `morph-glyphs` pairs the glyphs of two math or text objects by the characters they show: in `A = \pi r \times r` into `A = \pi r^2`, `A`, `=`, `π` and the first `r` slide, `×` and the second `r` fade out, and `2` fades in.
- `morph-in-order` pairs a math object's glyphs with a shape's paths in order: `x^2` into a circle reshapes the `x` into the circle and shrinks the `2`.
- `morph-tubes` folds a tube through 76 curve points into the atom trace of ubiquitin's alpha carbons, curve point by curve point, with `inspect` reporting each mixed curve point halfway between its two ends, and refuses a tube through another count of curve points, a target that is not a tube, and a tube of expressions, each at its path (sci_anim 0.19.0 or later).

## How to get to it (user POV)

- A `morph` animation in a `play` step, with `object` and `into`. Read it with `frame` or `inspect` at a scene time inside the span. `sci_anim --version` names 0.3.2 or later for `morph-glyphs`.
- For `morph-tubes`, both objects are `tube`s whose `curve` is `points` or a `trace`, and `sci_anim --version` names 0.19.0 or later. `skills/sci-anim/examples/ubiquitin_fold.go` writes `ubiquitin_fold.json`, which folds a zig-zag chain into ubiquitin from scene time 3 to 9 while the camera pulls in, then moves the fold aside for the molecule.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set.

- **Glyphs.** Write the scene: `echo '{"objects":{"a":{"type":"math","tex":"A = \\pi r \\times r","size":1.2},"b":{"type":"math","tex":"A = \\pi r^2","size":1.2}},"timeline":[{"type":"wait","duration":0.5},{"type":"play","duration":2,"animations":[{"type":"morph","object":"a","into":"b","easing_curve":"linear"}]},{"type":"wait","duration":0.5}]}' > "$RUN/glyphs.json"`. `$SKILL/scripts/run "$RUN" morph-glyphs-validate validate "$RUN/glyphs.json"` exits 0 with `.duration` 3. Then `$SKILL/scripts/run "$RUN" morph-glyphs-before frame "$RUN/glyphs.json" --scene-time 0.5 -o "$RUN/glyphs-0.5.png"`, the same at 1.5 as `morph-glyphs-middle` and at 2.5 as `morph-glyphs-after`, each exit 0. View the three: at 1.5 the `×` and the second `r` show at half strength in their own shapes, a faint `2` sits beside the first `r`, and no glyph is a tangle of two. At 2.5 the frame shows `A = πr²`.
- **In order.** `echo '{"objects":{"t":{"type":"math","tex":"x^2","size":1.2},"ci":{"type":"circle","radius":1,"fill":{"color":"#58c4dd"}}},"timeline":[{"type":"play","duration":2,"animations":[{"type":"morph","object":"t","into":"ci","easing_curve":"linear"}]}]}' > "$RUN/in-order.json"`, then `$SKILL/scripts/run "$RUN" morph-in-order frame "$RUN/in-order.json" --scene-time 1 -o "$RUN/in-order-1.png"`. Exit 0. The frame shows one shape halfway between the `x` and the circle, and the `2` shrinking, not fading.
- **Tubes.** In `skills/sci-anim/examples/`, `go run ubiquitin_fold.go "$RUN/ubiquitin_fold.json"` and `cmp` it with the committed `ubiquitin_fold.json`: they match. Run the checks on the committed file, so its `structures/1ubq.pdb` resolves. `$SKILL/scripts/run "$RUN" morph-tubes-validate validate skills/sci-anim/examples/ubiquitin_fold.json` exits 0 with `.duration` 13.5. Draw `--scene-time 2.5`, `6` and `13.5` into `$RUN` as `morph-tubes-start`, `-middle` and `-end`. View them: at 2.5 a straight blue zig-zag spans the frame above "A chain of 76 amino acids"; at 6 a wavy chain about half as long, with no breaks; at 13.5 the folded tube on the left beside the ball-and-stick molecule on the right, the same shape seen the same way, with the molecule's helix lit and both captions. Run `inspect` at 2.9, 6 and 9 as `morph-tubes-inspect-*`. `.objects.chain.tube.curve_points` has 76 points at 2.9 and 6, and `.objects.fold.tube.curve_points` 76 at 9. Each point at 6 is the mean of its point at 2.9 and the fold's at 9, within 0.001: `jq -n --slurpfile a "$RUN/morph-tubes-inspect-2.9.stdout.json" --slurpfile m "$RUN/morph-tubes-inspect-6.stdout.json" --slurpfile b "$RUN/morph-tubes-inspect-9.stdout.json" '[range(76) as $k | range(3) as $i | (($a[0].objects.chain.tube.curve_points[$k][$i] + $b[0].objects.fold.tube.curve_points[$k][$i]) / 2 - $m[0].objects.chain.tube.curve_points[$k][$i]) | fabs] | max'` prints under 0.001. `check-layout` of it reports no problem, and its `stages.txt` names `fold (tube)` as `through the CA atoms of structures/1ubq.pdb`; remove the `ubiquitin_fold-check/` folder it writes beside the scene.
- **Tube errors.** In a scene file in `$RUN` with an arch tube through 3 `points`, a tube through 4, a circle, a sphere and a tube of expressions in `u`, a morph of the arch into the 4-point tube gives `wrong_length` at `/timeline/0/animations/0/into`, into the circle or the sphere `not_morphable` at `.../into`, of the circle into the arch `not_morphable` at `.../into`, and of the expressions tube into the arch `not_morphable` at `.../object`. Each exits 1.
- **Unchanged.** Draw `examples/plot_and_morph.json` and `skills/sci-anim/examples/derivative.json` at a scene time inside each morph's span (`validate` gives the steps' spans), with this binary and with the release before 0.19.0 (`SCI_ANIM=<path>`), and `cmp` the PNGs: a morph of paths draws the same pixels.
- **Proof.** The `.json` report of `validate`, the frames, the three inspect reports with the `jq` result, and the scene files in `$RUN`.

## Gotchas

- The source shows until the morph ends and the target from then, so the target is never written, and a frame after the span shows the target, not the source.
- A frame at the span's very start or end shows one side whole. Read the middle of the span to see the pairing.
- Inside single quotes the shell keeps `\\pi` as written, and JSON reads it as `\pi`. Check with `jq -r .objects.a.tex "$RUN/glyphs.json"`, which prints `A = \pi r \times r`.
