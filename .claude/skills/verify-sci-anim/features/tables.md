# Draw tables

A `"type": "table"` sets text and math in cells: `rows` from the top, each a list of cells from the left, every row as long as the first. A cell is a string of plain text, `""` for an empty one, or `{ "tex": "..." }` for math. Each column is as wide as its widest cell and each row as tall as its tallest, plus `padding` on every side, and the table is centered on its `position`. `rulings` draws strokes between the cells: `all`, `header` or `none`. A `write` draws the rulings along their length over the first fifth of its span, then writes the cells glyph by glyph in reading order. A `highlight` with `cell: [row, column]` lights one cell with a see-through fill behind its text. `inspect` reports `table` with every cell's address, text or tex, created share, glow, rectangle and glyphs' box, and `check-layout` checks each cell's text as a text object named `table/row/column`. Needs sci_anim 0.18.0 or later.

## Sub-features

- `tables-draw` lays out the cells in rows and columns with their rulings, text and math alike.
- `tables-write` draws the rulings, then writes the cells row by row: nothing before the write, some rows part way through it, every cell after.
- `tables-highlight` lights one cell by its cell address, behind its text, and moves the light to another.
- `tables-inspect` reports each cell's address, text or tex, created share, glow, rectangle and glyphs' box.
- `tables-layout` passes `check-layout` at 1920×1080 with all 64 codons, and reports a cell's text that is too small, too faint on its glow, or covered, by its `table/row/column`.
- `tables-errors` refuses a row with another cell count, an empty table, and a lit cell outside the table, each at its field.
- `tables-unchanged` draws the existing text and math examples byte for byte as the `sci_anim` before tables did.

## How to get to it (user POV)

- Add a `table` object, `write` it, and `highlight` a `cell`, then run `validate`, `frame`, `inspect`, `stages`, `render`, `frames` or `check-layout`.
- `skills/sci-anim/examples/codon_table.json`, written by `codon_table.go` beside it, is the standard genetic code: 17 rows of 6 cells, written from 1.5 s to 5.5 s, with AUG, cell `[12, 1]`, lit from 6.5 s to 8 s. `examples/table.json` is a table of derivatives with math cells and a `header` ruling.
- `sci_anim schema | jq '.["$defs"].ObjectFile.allOf[].then | select(.properties.type.const == "table")'` prints the fields, and `jq '.["$defs"].CellFile'` a cell.

## Driving it with the run helper

Preconditions:

- Baseline preconditions from the README hold, with `RUN` set. Doctor's GPU line is `ok`.

- **Draw.** `validate skills/sci-anim/examples/codon_table.json` exits 0 with a duration of 10. `frame` at 6 and view the PNG: a ruled table, `1st U C A G 3rd` along the top, 16 rows of codons, `UUU Phe` at the top left and `GGG Gly` at the bottom right, no cell lit. `frame examples/table.json` at 7: `Function` and `Derivative` over one ruling, math in the cells, `cos x` lit blue.
- **Write.** `frame` the codon table at 3.5 and view it: every ruling drawn, the first rows written and the rest empty.
- **Highlight.** `frame` at 9 and view it: the `AUG Met` cell filled yellow behind white text, the caption beside it. `frame examples/table.json` at 5 has `1/x` lit, and at 7 `cos x`.
- **Inspect.** `inspect skills/sci-anim/examples/codon_table.json --scene-time 9`: `.objects.codons.table` has `rows` 17 and `columns` 6, and `.cells[73]` has `cell` `[12, 1]`, `text` `"AUG Met"`, `glow` 1 and a `rectangle` that holds its `bounding_box`. At 3.5 the late cells have `created` 0.
- **Layout.** `check-layout skills/sci-anim/examples/codon_table.json --out "$RUN/layout"` exits 0, and `stages.txt` lists cells such as `codons/12/1 (text): "AUG Met"`. Write `$RUN/faults.json`: a table at `size` 0.1, a cell lit `#ffffff`, and a text over a cell. `check-layout` exits 1 naming `too small to read` on the small table's cells, `hard to read` on the cell lit white, and `covers cells` on the text.
- **Errors.** With `jq`, write variants of the codon table into `$RUN`: row 3 with one cell dropped, `rows` `[]`, and the highlight's `cell` `[17, 1]`. `validate` exits 1 with `wrong_length` at `/objects/codons/rows/3`, `wrong_length` at `/objects/codons/rows`, and `unknown_cell` at `/timeline/3/animations/0/cell`.
- **Unchanged.** `frame` `examples/text_and_math.json` and `skills/sci-anim/examples/derivative.json` at a few scene times with `target/release/sci_anim` and with the `sci_anim` on PATH, and `cmp` each pair of PNGs: they match byte for byte.
- **Proof.** The PNGs, reports, and a sentence per PNG in `$RUN`.

## Gotchas

- A table is written, not created: `create` on a table is `use_write`.
- A cell's glow is see-through: white text on a lit cell passes at the default yellow on a dark background, and a light glow color fails `check-layout`'s contrast check.
- Rows are not spanned: the first base of the codon table stands on the first row of its block of four.
- A `sci_anim` before 0.18.0 gives `unknown_value` at the table's `type`. Drive `target/release/sci_anim`.
