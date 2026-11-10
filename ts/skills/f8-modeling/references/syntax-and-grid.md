# Syntax and Grid Reference

2D coordinate grammar, container constructors, vector expansion, 3-tier attribute resolution, layout placement, and formatting presets.

---

## 1. 2D Coordinate Addressing Grammar

All formula cell references require explicit 2D bracket syntax:

`Table[RowSelector, ColSelector]` or `@table[RowSelector, ColSelector]`

### Coordinate Selectors

| Selector type              | Syntax                          | Resolution semantics                                                    | Example                                          |
| :------------------------- | :------------------------------ | :---------------------------------------------------------------------- | :----------------------------------------------- |
| **Active table**           | `@table["row", @col]`           | Current evaluation table (avoids repeating table name intra-table)      | `@table["revenue", @col] - @table["cogs", @col]` |
| **Active column**          | `Table["row", @col]`            | Current evaluation column                                               | `IS["revenue", @col] * 0.40`                     |
| **Active row**             | `Table[@row, "col"]`            | Current evaluation row                                                  | `Assumptions[@row, "rate"] * 100`                |
| **Absolute point**         | `Table["row", "col"]`           | Fixed coordinate (lowers to `$Col$Row`)                                 | `Assumptions["tax_rate", "val"]`                 |
| **Column lag / lead (1)**  | `Table["row", col.prev]`        | Prior (`prev`) or next (`next`) `measure` column                        | `BS["cash", col.prev] + 10.0`                    |
| **Column lag / lead (k)**  | `Table["row", col.prev(k)]`     | `k` `measure` columns prior (`prev(k)`) or ahead (`next(k)`) (`k >= 0`) | `Rev["sales", col.prev(2)]`                      |
| **Row lag / lead (1)**     | `Table[row.prev, "col"]`        | Prior (`prev`) or next (`next`) data row                                | `Schedule[row.prev, "col"] * 1.05`               |
| **Row lag / lead (k)**     | `Table[row.prev(k), "col"]`     | `k` data rows prior (`prev(k)`) or ahead (`next(k)`) (`k >= 0`)         | `Schedule[row.next(2), "col"]`                   |
| **Column slice**           | `Table["row", "c1".."c2"]`      | Inclusive horizontal range across columns                               | `SUM(Rev["sales", "2023A".."2025E"])`            |
| **Dynamic column slice**   | `Table["row", "c1"..@col]`      | Inclusive horizontal range between any two column endpoints             | `SUM(Debt["amort", "2024E"..col.prev])`          |
| **Row slice (active col)** | `Table["r1".."r2", @col]`       | Inclusive vertical range across rows at active column                   | `SUM(Opex["r_and_d".."g_and_a", @col])`          |
| **Dynamic row slice**      | `Table["r1"..@row, @col]`       | Inclusive vertical range between any two row endpoints                  | `SUM(Cohort["c1"..row.prev, @col])`              |
| **Row slice (fixed col)**  | `Table["r1".."r2", "col"]`      | Inclusive vertical range across rows at fixed column                    | `SUM(Comps["peer1".."peer5", "ev"])`             |
| **2D block slice**         | `Table["r1".."r2", "c1".."c2"]` | Inclusive 2D rectangular matrix range                                   | `SUM(Matrix["r1".."r3", "2023A".."2025E"])`      |

### Lexical and Grammatical Rules

- Table identifiers: Bare ASCII identifiers (`[A-Za-z0-9_]+` containing at least one ASCII letter or `_`) or `@table` active-table token before `[`. Quoted table names (`"Table"[...]`), pure numbers (`123[...]`), or dot access (`Table.row`) throw `FormulaParseError`.
- Coordinate keys: Explicit row and column IDs inside `[...]` must be double-quoted strings (`"revenue"`, `"2024E"`). Unquoted keys (`Table[revenue, @col]`) throw `FormulaParseError`.
- Active axis tokens: Must be `@col`, `@row`, or `@table` (strictly before `[`). Bare `@` throws `FormulaParseError`.
- Uppercase identifiers: Function names (`[A-Z][A-Z0-9_]*`, starting with an ASCII uppercase letter) and boolean literals (`TRUE`, `FALSE`) must be uppercase (table identifiers before `[` may be mixed-case). Lowercase functions/booleans, `null`, or standalone identifiers throw `FormulaParseError`.
- Omitted arguments (`_`): Valid only as a direct argument in a function call (`PMT(rate, nper, pv, _, type)`). Using `_` in arithmetic or coordinate selectors throws `FormulaParseError`.
- Operators and precedence (highest to lowest):
  1. Exponentiation: `^` (tier 5, right-associative)
  2. Unary prefix: `+`, `-` (tier 4, right-associative)
  3. Multiplicative: `*`, `/` (tier 3, left-associative)
  4. Additive: `+`, `-` (tier 2, left-associative)
  5. Relational comparisons: `=`, `<>`, `<`, `<=`, `>`, `>=` (tier 1, left-associative; cross-type rank order `Number < Text < Boolean`, case-insensitive ASCII string comparison, error bubbling, and `Empty` coerced to `0.0` vs. `Number`, `""` vs. `Text`, equal to `Empty`, and ordered less than `Boolean` so `Empty < FALSE < TRUE` and `Empty = FALSE` is `FALSE`). Note: `==`, `!=`, and `&` are not supported and throw `FormulaParseError`.
- Slice endpoints and ordering: Range endpoints (`start..end`) accept quoted keys, `@col`/`@row`, or `prev`/`next` offsets on either side. Resolved indices must follow declared table order (`start_idx <= end_idx`); inverted ranges throw `ReferenceResolutionError`. Note: column range slices (`"c1".."c2"`) span all data columns between the endpoints in declaration order (including any intermediate `descriptor` columns, skipping gutters), whereas `col.prev`/`col.next` step across `measure` columns only.
- Offset boundaries: `col.prev` / `col.next` step across `role: "measure"` columns in the origin table (skipping `descriptor` and gutter columns) from `origin.col` to resolve a target column ID, then look up that ID in the target table; `row.prev` / `row.next` step across data rows in the origin table (skipping section headers and gutter rows) from `origin.row` to resolve a target row ID, then look up that ID in the target table. Stepping outside the origin table's axis (or evaluating `col.prev`/`col.next` from a `descriptor` column) throws `ReferenceResolutionError` (`TemporalOffsetOutOfBounds`); if the offset-resolved ID is absent in the target table, it throws `ReferenceResolutionError` (`UnknownColumn` / `UnknownRow`).
- Excel A1 lowering and `$` pinning:
  - Single cell: Static `Table["r", "c"]` lowers to `$B$5`; any dynamic axis (`@col`, `@row`, `prev`, `next`) lowers to relative `B5`. Cross-sheet references always single-quote the sheet name in lowered formulas (`'Sheet1'!$B$5`, escaping `'` as `''`), whereas inspection metadata (`physicalAddress`, `sheetAddress`) uses unquoted `Sheet1!C14`.
  - Range slice: Static key endpoints pin their respective axis on each endpoint (`Table["r", "c1"..@col]` -> `$C$5:E$5`; `Table[@row, "c1"..@col]` -> `$C5:E5`), except 1D horizontal static-column ranges on a dynamic row (`Table[@row, "c1".."c2"]` -> `C5:E5`).

---

## 2. Container Constructors (`Model`, `Sheet`, `Table`)

```ts
import { Cell, Model, Sheet, Table, compile, defaultTheme, init } from "f8-engine";

const model = new Model({
  name?: string, // default "Model"
  theme?: ThemeInput, // default "corporate_navy"
});

const sheet: Sheet = model.addSheet(
  name: string,
  options?: {
    tabColor?: string;
    showGridlines?: boolean; // default true
    freezePanes?: FreezePanesInput; // { row: number, col: number }
  },
);

const table: Table = sheet.addTable(
  id: string,
  title: string,
  options?: {
    subtitle?: string;
    currency?: CurrencyCode;
    collapsible?: boolean; // default false
    rightOf?: string | Table;
    below?: string | Table;
    gap?: number;
  },
);
```

### Container Methods and Properties

| Container | Member                             | Return type        | Behavior                                                                                                      |
| :-------- | :--------------------------------- | :----------------- | :------------------------------------------------------------------------------------------------------------ |
| `Model`   | `model.addSheet(name, options?)`   | `Sheet`            | Appends worksheet; duplicate `name` or negative/non-integer `freezePanes` `{ row, col }` throws `Error`       |
| `Model`   | `model.getSheet(name)`             | `Sheet`            | Looks up worksheet by `name` (throws `Error` if absent)                                                       |
| `Model`   | `model.getTable(id)`               | `Table`            | Looks up table by `id` across all worksheets (throws `Error` if absent)                                       |
| `Model`   | `model.sheets`                     | `readonly Sheet[]` | Ordered array of worksheets                                                                                   |
| `Model`   | `model.tables`                     | `readonly Table[]` | Ordered array of all tables across all worksheets                                                             |
| `Model`   | `model.compile()`                  | `CompiledModel`    | Validates, expands, evaluates, lays out, and lowers model                                                     |
| `Model`   | `model.toSpec()`                   | `ModelSpec`        | Serializes model into `{ manifest: ProjectManifestInput, tables: readonly TableSpecInput[] }`                 |
| `Sheet`   | `sheet.addTable(id, title, opts?)` | `Table`            | Appends table; duplicate `id` across model, both `rightOf` and `below`, or `gap < 0` throws `Error`           |
| `Sheet`   | `sheet.getTable(id)`               | `Table`            | Looks up table on this worksheet by `id` (throws `Error` if absent)                                           |
| `Sheet`   | `sheet.tables`                     | `readonly Table[]` | Ordered array of tables on this worksheet                                                                     |
| `Sheet`   | `sheet.toSpec()`                   | `SheetSpecInput`   | Serializes worksheet into raw specification object                                                            |
| `Table`   | `table.toSpec()`                   | `TableSpecInput`   | Serializes table into raw specification object                                                                |

- Low-level compilation & WASM lifecycle:
  - `compile(manifest, tables) => CompiledModel` compiles raw `ProjectManifestInput` and `readonly TableSpecInput[]` payloads directly.
  - Server/CLI runtimes (Node.js, Bun, Deno) auto-initialize WASM synchronously on module import; browser environments call `await init(moduleOrPath?)` once before calling `compile()`, `model.compile()`, or `defaultTheme()`.
- Theme resolution (`ThemeInput = string | Readonly<Record<string, unknown>> | URL`):
  - `string` (default `"corporate_navy"`): Resolves strictly to a compiled-in preset in `f8-core` (does not read disk files; unknown preset name throws `SpecificationError`).
  - `URL` (e.g. `pathToFileURL("theme.json")` or `new URL("./theme.json", import.meta.url)`): Reads and parses a JSON theme file into `model.theme` via `node:fs`. Use `f8 theme init` (or `defaultTheme()`) to inspect or scaffold the default `corporate_navy` schema.
  - `Readonly<Record<string, unknown>>`: Inline custom theme object. Compiler validates hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA`, or `"none"`/`"transparent"` on fills), positive finite metrics, `{dec}`/`{sym}` format placeholders, and default decimals `<= 14`.
- Worksheet options: `tabColor` accepts hex color strings (invalid hex throws `EmissionError` on workbook emission). `freezePanes: { row, col }` sets 0-indexed integer split coordinates.

---

## 3. Column and Row Specifications

### `table.addColumn`

```ts
table.addColumn(
  id: string,
  options?: {
    role?: AxisRole; // default "measure"
    header?: string;
    headers?: readonly string[];
    group?: string;
    style?: SemanticStyle;
    highlight?: boolean;
    shade?: Shade;
    format?: FormatPreset;
    decimals?: number;
    currency?: CurrencyCode;
    expr?: string;
  },
): this
```

- Column roles (`AxisRole`):
  - `"measure"` (default): Calculable data column. Participates in horizontal `row.expr` broadcasting and `col.prev`/`col.next` navigation.
  - `"descriptor"`: Metadata/label column. Isolated from vector formula broadcasting (`row.expr` and `col.expr` never expand into descriptor columns) and isolated from row-level `format`, `decimals`, and `currency`.
- Headers and group banners:
  - Pass `header: "Title"` (1 tier) or `headers: ["Tier 1", "Tier 2"]` (stacked tiers within the column); passing both throws `Error`.
  - Pass `group: "Banner"` for a Tier 0 horizontal banner spanning contiguous columns with the same `group` string (rendered with `CenterAcross` and `super_header_format`; excluded from single-column width calculation). Empty or whitespace-only `group` throws `Error`.
  - Per-column header depth is `Math.max(headers.length, 1) + 1` when `group` is set, or `headers.length` (`0` if omitted) when ungrouped. Table header depth is `max(col.header_depth())` across all columns (`0` header rows when ungrouped and no column sets `header`/`headers`). When header rows exist (`depth > 0`), columns with fewer lines are bottom-aligned; if a column specifies neither `header` nor `headers`, `.xlsx` emission writes a blank header cell while `inspectSheet` and `TableAuditor.toMarkdown()` fall back to `col.id`.
- Vertical vector formula (`expr`): Evaluates top-to-bottom across all data rows in `measure` columns (ignored on `descriptor` columns, which require explicit Tier 1 `cells` entries).

### `table.addRow`

```ts
table.addRow(
  id: string,
  options?: {
    cells?: Readonly<Record<string, CellInputValue>>;
    expr?: string;
    style?: SemanticStyle;
    highlight?: boolean;
    shade?: Shade;
    format?: FormatPreset;
    decimals?: number;
    currency?: CurrencyCode;
  },
): this
```

- Horizontal vector formula (`expr`): Evaluates left-to-right across all `measure` columns. When `expr` uses `col.prev` on the first `measure` column, supply an explicit 0D base value for that column in `cells`.
- Duplicate `id` in `addColumn` or `addRow` throws `Error`. Tables with an empty `columns` or `rows` spec list throw `SpecificationError`.

---

## 4. 0D Cell Overrides and Dynamic Updates (`Cell`, `setCell`, `setCells`)

### `Cell` Class and `DetailedCellInput`

```ts
export interface DetailedCellInput {
  readonly value?: CellValueLiteral;
  readonly expr?: string;
  readonly style?: SemanticStyle;
  readonly highlight?: boolean;
  readonly shade?: Shade;
  readonly format?: FormatPreset;
  readonly decimals?: number;
  readonly currency?: CurrencyCode;
  readonly indent?: number;
  readonly note?: string;
}

const cell = new Cell(options: DetailedCellInput);
```

`CellInputValue = CellValueLiteral | Cell | DetailedCellInput` where `CellValueLiteral = number | string | boolean | null`.

- Payload exclusivity: `new Cell(options)` (and `DetailedCellInput` on new cells) must specify either `value` or `expr`, never both (`Error` at construction, `SpecificationError` at compile time). Note: `value: null` (or literal `null`) is a valid explicit empty payload (`CellValue::Empty`).
- Constraint checks (violations throw `SpecificationError`):
  - `indent`: Non-negative integer (`0..=255`); allowed only on `role: "descriptor"` cells (`indent > 0` on a `measure` cell is rejected).
  - `currency`: Rejects pairing `currency` with a non-`"currency"` `format` on the same spec level (`cell`, `row`, or `col`). During grid expansion, if the cell's resolved effective `format` is not `"currency"` (or the column is `role: "descriptor"`), effective `currency` resolves to `null`.
  - `decimals`: Requires `0 <= decimals <= 14`. Pairing `decimals` with `"integer"`, `"date"`, or `"text"` on the same spec level (`cell`, `row`, or `col`) throws `SpecificationError`. During grid expansion, if the cell's resolved effective `format` is `"integer"`, `"date"`, or `"text"`, effective `decimals` resolves to `null` (when effective `format` is `null`, explicit `cell`/`row`/`col` `decimals` is retained for in-memory formatting).
  - Unknown column key in `row.cells` throws `SpecificationError`.

### `table.setCell` and `table.setCells`

```ts
export interface AxisSlice {
  readonly start: string;
  readonly stop: string;
}

export type AxisSelector = string | Iterable<string> | AxisSlice;

table.setCell(row: string, col: string, cell: CellInputValue): this;
table.setCells(rows: AxisSelector, cols: AxisSelector, cell: CellInputValue): this;
```

- Create vs. merge semantics:
  - **Existing 0D cell**: If `(row, col)` already has an entry in `row.cells`, `setCell` / `setCells` merges any defined attributes from a `DetailedCellInput` object (`style`, `highlight`, `shade`, `format`, `decimals`, `currency`, `indent`, `note`) while preserving its existing `value` or `expr` (unless a new `value`, `expr`, or `new Cell(...)` payload is provided, which replaces the payload).
  - **New 0D override**: If `(row, col)` does not yet have a 0D entry in `row.cells` (including coordinates currently covered only by `row.expr` or `col.expr`), `setCell` / `setCells` must provide `value` (or a scalar literal / `null`) or `expr`; omitting both `value` and `expr` throws `Error`.
- Axis selectors (`AxisSelector`):
  - Accepts a single ID (`string`), a non-empty `Iterable<string>` (e.g. `string[]`), or an inclusive `AxisSlice` (`{ start: "start_id", stop: "stop_id" }`) across declared data rows or data columns (skipping gutters and section headers).
  - Unknown row or column IDs, inverted slices (`startIdx > stopIdx`), or empty iterables throw `Error`.
  - `setCells` pre-validates all target `(row, col)` coordinates atomically before mutating `row.cells`.

```ts
// Create 0D overrides across a column array or 2D AxisSlice
table.setCells("amort", ["standalone", "adj"], null);
table.setCells(
  { start: "r1", stop: "r2" },
  { start: "c1", stop: "c2" },
  { expr: 'Table[@row, "base"] * Table["mult", @col]' },
);

// Patch metadata on an existing 0D cell (already in row.cells)
table.setCell("r1", "c1", { highlight: true, note: "Patched existing 0D cell" });
```

---

## 5. 3-Tier Specificity Lattice

Every `(row, col)` data coordinate resolves its payload and formatting through a 3-tier specificity lattice (`Tier 1: 0D Cell > Tier 2: 1D Row/Col > Tier 3: Default`):

| Attribute       | Tier 1 (0D Cell)                         | Tier 2 (1D Row vs. Column on `measure` cols)                              | Descriptor Column (`role: "descriptor"`)                    | Tier 3 (Fallback Default)                                                                |
| :-------------- | :--------------------------------------- | :------------------------------------------------------------------------ | :---------------------------------------------------------- | :--------------------------------------------------------------------------------------- |
| **Payload**     | `cell.value` or `cell.expr`              | `row.expr` XOR `col.expr` (both -> `SpecificationError`)                  | Requires Tier 1 entry (`cells`), else `SpecificationError`  | None (unassigned `measure` cell -> `SpecificationError`)                                 |
| **`format`**    | `cell.format`                            | `row.format` XOR `col.format` (differing -> `SpecificationError`)         | `cell.format` else `col.format` (row isolated)              | `null` (unformatted)                                                                     |
| **`decimals`**  | `cell.decimals`                          | `row.decimals` XOR `col.decimals` (differing -> `SpecificationError`)     | `cell.decimals` else `col.decimals` (row isolated)          | `theme.number_formats.defaults` for active preset (`null` if non-decimal preset)         |
| **`currency`**  | `cell.currency`                          | `row.currency` XOR `col.currency` (differing -> `SpecificationError`)     | `null` (descriptor columns ignore `currency`)               | `table.currency` else `"USD"` (only when effective `format === "currency"`, else `null`) |
| **`shade`**     | `cell.shade` (`"none"` clears to `null`) | `row.shade` XOR `col.shade` (differing non-`"none"` -> `SpecificationError`) | Same as `measure` (row `shade` spans descriptors)        | `null`                                                                                   |
| **`style`**     | `"data"` resets both axes to `"data"`; `"subtotal"`/`"total"` overrides horizontal axis | Orthogonal 2D composition: `(horizontal=row.style, vertical=col.style)` | Same as `measure` (row `style` spans descriptors) | `("data", "data")`                                                                       |
| **`highlight`** | `cell.highlight` (`false` clears inherited highlight) | `Boolean(row.highlight \|\| col.highlight)`                  | Same as `measure`                                           | `false`                                                                                  |
| **`indent`**    | `cell.indent`                            | N/A                                                                       | `cell.indent` else `0`                                      | `0` (`indent > 0` on `measure` cell -> `SpecificationError`)                             |
| **`note`**      | `cell.note`                              | N/A                                                                       | `cell.note` else `null`                                     | `null`                                                                                   |

---

## 6. Layout Elements and Worksheet Placement

### Section Headers and Spacing Gutters

```ts
table.addSectionHeader(title: string, options?: { indent?: number; note?: string }): this;
table.addRowGutter(): this;
table.addColumnGutter(): this;
```

- `addSectionHeader`: Consumes 1 physical row, writes `title` at `table.start_col` with `section_header_format(indent)` and optional Excel note, and allocates zero calculation cells.
- `addRowGutter` / `addColumnGutter`: Consumes 1 blank physical row or column (`gutter_col_width` = `3.0` chars by default) with zero calculation cells. Skipped by vector formulas, `prev`/`next` offsets, and `AxisSlice` selectors.

### Table Placement (`stack`, `below`, `rightOf`, `gap`)

- Worksheet margins: Physical Row 0 and Column 0 (Column A) are reserved as top and left margins. The first table on a sheet starts at `B2` (`col=1, row=1`).
- Placement modes on `sheet.addTable(...)`:
  - Default (`stack`): Places table at `start_col = 1` (Column B) below the lowest `end_row` of all preceding tables on the sheet, separated by `gap: 2` gutter rows.
  - `below: "TargetTable"` (or `Table` instance): Aligns `start_col = target.start_col` and places table at `target.end_row + 1 + gap` (default `gap: 2`).
  - `rightOf: "TargetTable"` (or `Table` instance): Aligns `start_row = target.start_row` and places table at `target.end_col + 1 + gap` (default `gap: 1` gutter column).
  - Placement validation: `rightOf` and `below` are mutually exclusive (`Error`), and `gap < 0` throws `Error`. Target table must be declared earlier on the same worksheet, and table bounding boxes cannot overlap (violations throw `SpecificationError`).
- Table banners and collapsing:
  - Non-empty `title` emits a full-width title banner row (`start_col..=end_col`). `subtitle` emits a subtitle row beneath `title` (passing an empty or whitespace-only `subtitle: ""` throws `SpecificationError`; omit `subtitle` when unused). If either `title` or `subtitle` is present, a 1-row buffer gutter is inserted before column headers; passing `title: ""` (with no `subtitle`) omits both banner and buffer gutter so column headers start at `start_row`.
  - `collapsible: true`: Groups physical rows `start_row + 1..=end_row` in Excel outline with summary symbols above (`group_symbols_above(true)`).
- Column width arbitration & text wrapping: Each physical worksheet column takes the maximum width across all tables spanning that column, computed as `round(clamp(max_chars * char_width_multiplier + col_padding, min_col_width, max_col_width), 2)` (default `col_padding=0.0`, `min_col_width=12.0`, `max_col_width=75.0`, where `max_chars` spans per-column header lines—excluding spanning `group`—and `\n`-delimited data cell segments with `indent * 2` plus `+1` alignment-space character on formatted values with trailing alignment spaces; gutter-only columns use `gutter_col_width=3.0`). Cells whose effective width exceeds `max_col_width` or that contain `\n` newlines enable OpenXML `wrapText="1"` (`FormatAlign::VerticalCenter` on all data cells) and scale `SheetLayout.row_heights` to `lines * data_row_height` while leaving `.xlsx` row heights automatic for Excel font auto-fitting.

---

## 7. Formatting Presets, Styles, Shades, and Currencies

### Format Presets (`FormatPreset`)

| Preset           | Default `decimals` | Supports `decimals` | Supports `currency` | Sample inputs (`pos`, `neg`, `zero`) | In-memory formatted output (`formatted()`, `toMarkdown()`)  |
| :--------------- | :----------------- | :------------------ | :------------------ | :----------------------------------- | :---------------------------------------------------------- |
| `"currency"`     | `0`                | Yes (`0..=14`)      | Yes (default `USD`) | `1250`, `-1250`, `0`                 | `"$1,250"` / `"($1,250)"` / `"$0"` (Excel zero mask: `$-`)  |
| `"number"`       | `0`                | Yes (`0..=14`)      | No                  | `1250`, `-1250`, `0`                 | `"1,250"` / `"(1,250)"` / `"0"` (Excel zero mask: `-`)      |
| `"percentage"`   | `1`                | Yes (`0..=14`)      | No                  | `0.15`, `-0.15`, `0`                 | `"15.0%"` / `"(15.0%)"` / `"0.0%"` (Excel zero mask: `-`)   |
| `"multiple"`     | `1`                | Yes (`0..=14`)      | No                  | `12.5`, `-12.5`, `0`                 | `"12.5x"` / `"(12.5x)"` / `"0.0x"` (Excel neg: `-12.5x`)    |
| `"basis_points"` | `0`                | Yes (`0..=14`)      | No                  | `0.015`, `-0.015`, `0`               | `"150 bps"` / `"(150 bps)"` / `"0 bps"` (scales `10,000x` in-memory; Excel `#,##0" bps"` does not scale) |
| `"integer"`      | `null`             | No                  | No                  | `2024`, `-2024`, `0`                 | `"2024"` / `"(2024)"` / `"0"` (Excel: `2024` / `(-2024)` / `-`) |
| `"date"`         | `null`             | No                  | No                  | `45657` (Excel serial)               | `"2024-12-31"`                                              |
| `"text"`         | `null`             | No                  | No                  | `"Sample"`                           | `"Sample"`                                                  |
| `null` (default) | `null`             | Yes (in-memory)     | No                  | `1250`, `-12.5`, `0`                 | `"1,250"` / `"(12.5)"` / `"0"` (uses explicit `decimals` if set, else `0` if integer and `1` if fractional; Excel `General`) |

### Semantic Styles (`SemanticStyle`)

- `"data"` (default): Standard body typography and borders.
- `"subtotal"`: Bold typography; thin top border on rows (`subtotal_border_top`), thin left border on columns (`col_subtotal_border_left`).
- `"total"`: Bold typography; thin top and double bottom border on rows (`total_border_top`, `total_border_bottom`), thin left and double right border on columns (`col_total_border_left`, `col_total_border_right`).

### Background Shades (`Shade`)

Applies theme palette background fill to body cells without changing font color (fill resolution: `highlight_fill` if highlighted; else `total_fill.or(shade_fill)` if `total`; else `subtotal_fill.or(shade_fill)` if `subtotal`; else `shade_fill.or(data_fill)`; font color precedence: `input_text` on `measure` literal inputs `> highlight_text > total_text > subtotal_text > data_text` on `descriptor` cells `> formula_text`):

- `"none"`: No background shade; clears inherited row/column shade when set on a 0D cell override.
- `"muted"`: Maps to `theme.palette.shade_muted_fill` (`#F8FAFC` in `corporate_navy`).
- `"alt"`: Maps to `theme.palette.shade_alt_fill` (`#F1F5F9` in `corporate_navy`).

### Currency Codes (`CurrencyCode`)

Case-insensitive 3-letter ISO codes (normalized to lowercase in spec payloads):
`USD`, `EUR`, `GBP`, `JPY`, `CHF`, `CAD`, `AUD`, `CNY`, `HKD`, `SGD`, `SEK`, `NOK`, `NZD`, `INR`, `BRL`.
