# Set text in mono

`"font": "mono"` sets a `text`, a `table`'s text cells or a molecule's atom symbols in JetBrains Mono, whose characters are all 0.6 of `size` wide, spaces included, so columns of code or terminal output line up. A mono text draws no ligatures (`!=` and `->` stay two signs), covers the box drawing characters `tree` prints and `✓ ✗ ❯ ●`, and takes no glyph from another font, so a character JetBrains Mono lacks is a scene error. Sans and serif text fall back through the fonts they did before, so they draw as before. Needs sci_anim 0.28.0 or later.

## Sub-features

- `mono-cells` sets every character in one cell: `iiii`, `mmmm`, `->!=` and four spaces are each 2.4 wide at `size` 1, and a column of code lines up under the column above.
- `mono-symbols` draws `├──`, `│`, `✓`, `✗`, `❯` and `●`, and `!=` as two signs.
- `mono-write` writes a mono text glyph by glyph, as any text.
- `mono-table` sets a table's text cells in mono.
- `mono-missing` refuses a character JetBrains Mono lacks, such as `日`, at the object's `text`, exit 1.
- `mono-unchanged` draws sans and serif text as before: the snapshot tests in `tests/` pass unchanged.

## How to get to it (user POV)

- Give a `text` `"font": "mono"`, then run `validate`, `inspect` or `frame`.
- `sci_anim schema | jq '.["$defs"].Font'` lists `sans`, `serif` and `mono`.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.

- **Cells.** Write a scene with left-aligned mono texts at `size` 1, `iiii`, `mmmm`, `->!=` and `    ab`, and a Go function of five lines with tab-free indentation. `inspect` at 0: `iiii` and `mmmm` have the same bounding box width to within the glyphs' side bearings, and `    ab` starts 2.4 right of `ab`. `frame` it and view the PNG: the code's columns line up.
- **Symbols.** In the same scene, a text `├── a\n│   └── b\n✓ ✗ ❯ ●\nif a != b -> c`. View the frame: the tree's lines join, `!=` is `!` and `=`.
- **Write.** Write the code with a `write` over 2 s and `frame` at its middle: the first lines drawn, the rest not.
- **Table.** A `table` with `"font": "mono"` and cells `drwxr-xr-x`, `4096`: `frame` and view it.
- **Missing.** A mono text `x日`: `validate` exits 1 with an error at `/objects/<id>/text`.
- **Proof.** The scene, the reports and the PNGs in `$RUN`, and a sentence per PNG.
