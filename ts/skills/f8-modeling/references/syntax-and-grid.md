# Syntax and Grid Reference

2D coordinate grammar, container constructors, vector expansion, 3-tier attribute resolution, layout placement, and formatting presets.

---

## 1. 2D Coordinate Addressing Grammar

All formula cell references require explicit 2D bracket syntax:

`Table[RowSelector, ColSelector]` or `@table[RowSelector, ColSelector]`

### Coordinate Selectors

| Selector type              | Syntax                          | Resolution semantics                                                                          | Example                                          |
| :------------------------- | :------------------------------ | :-------------------------------------------------------------------------------------------- | :----------------------------------------------- |
| **Active table**           | `@table["row", @col]`           | Current evaluation table (avoids repeating table name intra-table)                            | `@table["revenue", @col] - @table["cogs", @col]` |
| **Active column**          | `Table["row", @col]`            | Current evaluation column                                                                     | `IS["revenue", @col] * 0.40`                     |
| **Active row**             | `Table[@row, "col"]`            | Current evaluation row                                                                        | `Assumptions[@row, "rate"] * 100`                |
| **Absolute point**         | `Table["row", "col"]`           | Fixed coordinate (lowers to `$Col$Row`)                                                       | `Assumptions["tax_rate", "val"]`                 |
| **Column lag / lead (1)**  | `Table["row", col.prev]`        | Prior (`prev`) or next (`next`) `measure` column                                              | `BS["cash", col.prev] + 10.0`                    |
| **Column lag / lead (k)**  | `Table["row", col.prev(k)]`     | `k` `measure` columns prior (`prev(k)`) or ahead (`next(k)`) (integer `1 <= k <= 2147483647`) | `Rev["sales", col.prev(2)]`                      |
| **Row lag / lead (1)**     | `Table[row.prev, "col"]`        | Prior (`prev`) or next (`next`) data row                                                      | `Schedule[row.prev, "col"] * 1.05`               |
| **Row lag / lead (k)**     | `Table[row.prev(k), "col"]`     | `k` data rows prior (`prev(k)`) or ahead (`next(k)`) (integer `1 <= k <= 2147483647`)         | `Schedule[row.next(2), "col"]`                   |
| **Column slice**           | `Table["row", "c1".."c2"]`      | Inclusive horizontal range across columns                                                     | `SUM(Rev["sales", "2023A".."2025E"])`            |
| **Dynamic column slice**   | `Table["row", "c1"..@col]`      | Inclusive horizontal range between any two column endpoints                                   | `SUM(Debt["amort", "2024E"..col.prev])`          |
| **Row slice (active col)** | `Table["r1".."r2", @col]`       | Inclusive vertical range across rows at active column                                         | `SUM(Opex["r_and_d".."g_and_a", @col])`          |
| **Dynamic row slice**      | `Table["r1"..@row, @col]`       | Inclusive vertical range between any two row endpoints                                        | `SUM(Cohort["c1"..row.prev, @col])`              |
| **Row slice (fixed col)**  | `Table["r1".."r2", "col"]`      | Inclusive vertical range across rows at fixed column                                          | `SUM(Comps["peer1".."peer5", "ev"])`             |
| **2D block slice**         | `Table["r1".."r2", "c1".."c2"]` | Inclusive 2D rectangular matrix range                                                         | `SUM(Matrix["r1".."r3", "2023A".."2025E"])`      |

### Lexical and Grammatical Rules

- Table identifiers: Bare ASCII identifiers (`[A-Za-z0-9_]+` containing at least one ASCII letter or `_`) or `@table` active-table token before `[`. Quoted table names (`"Table"[...]`), pure numbers (`123[...]`), or dot access (`Table.row`) throw `FormulaParseError`.
- Coordinate keys: Explicit row and column IDs inside `[...]` must be double-quoted strings (`"revenue"`, `"2024E"`). Unquoted keys (`Table[revenue, @col]`, `Table["revenue", 2024E]`) throw `FormulaParseError`.
- Numeric and string literals: Numeric literals support integers, decimals, and scientific notation (`[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?`, e.g., `42`, `0.25`, `1e-9`, `1.5E+6`) within finite IEEE-754 `f64` bounds (non-finite overflow such as `1e500` or malformed tokens such as `1.5e-` throw `FormulaParseError`; finite magnitudes outside `[1e-6, 1e15)` lower to compact uppercase `E` notation in Excel). String literals and quoted coordinate keys use double quotes with `\"` and `\\` backslash escapes.
- Active axis tokens: Must be `@col`, `@row`, or `@table` (strictly before `[`). Bare `@` throws `FormulaParseError`.
- Uppercase identifiers: Function names (`[A-Z][A-Z0-9_]*`, single uppercase identifiers such as `SUM` or `XLOOKUP`) and boolean literals (`TRUE`, `FALSE`) must be uppercase (table identifiers before `[` may be mixed-case). Lowercase functions/booleans, dotted function names (`ERROR.TYPE`), `null`, or standalone identifiers throw `FormulaParseError`.
- Omitted arguments (`_`): Syntactically confined to direct function call arguments (`PMT(rate, nper, pv, _, type)`); using `_` in arithmetic or coordinate selectors throws `FormulaParseError`. Semantically valid only on intermediate optional parameters followed by at least one explicit argument (omitting a required or trailing parameter with `_` throws `F8Error`).
- Operators and precedence (highest to lowest):
  1. Exponentiation: `^` (tier 5, right-associative, binds tighter than unary `-` so `-2 ^ 2` evaluates to `-4` and lowers to `=-(2^2)`; numeric operands required; `0 ^ 0` and negative base with non-integer exponent return `#NUM!`, `0 ^ -n` returns `#DIV/0!`)
  2. Unary negation: `-` (tier 4, right-associative; numeric operand required; unary `+` is not supported and throws `FormulaParseError`)
  3. Multiplicative: `*`, `/` (tier 3, left-associative; numeric operands required)
  4. Additive: `+`, `-` (tier 2, left-associative; numeric operands required)
  5. Relational comparisons: `=`, `<>`, `<`, `<=`, `>`, `>=` (tier 1, non-associative unless parenthesized as `(a = b) = c`; unparenthesized chained comparisons throw `FormulaParseError`). Operands must be non-`Empty` values of identical type (`Number`, `Text` via case-insensitive ASCII comparison, or `Boolean`; cross-type or `Empty` operands throw `F8Error`, and runtime Excel errors bubble). Note: `==`, `!=`, and `&` are not supported and throw `FormulaParseError`.
- Slice endpoints, ordering, and contiguity: Range endpoints (`start..end`) accept quoted keys, `@col`/`@row`, or `prev`/`next` offsets on either side. Resolved indices must follow declared table order (`start_idx <= end_idx`; inverted ranges throw `ReferenceResolutionError`). Row range slices (`"r1".."r2"`) must span contiguous data rows with no intervening row gutters or section headers, and horizontal column range slices (`"c1".."c2"`) must span contiguous `measure` columns with no `descriptor` or gutter columns (violations throw `ReferenceResolutionError`); 1D vertical slices on a single `descriptor` column (`Table["r1".."r2", "line_item"]`) remain valid in `XLOOKUP`, `XMATCH`, and `INDEX` (numeric and financial aggregations reject `descriptor` columns with `F8Error`).
- Offset boundaries: `col.prev` / `col.next` anchor `origin.col` in the target table and step across `role: "measure"` columns in the target table (skipping `descriptor` and gutter columns); `row.prev` / `row.next` anchor `origin.row` in the target table and step across data rows in the target table (skipping section headers and gutter rows). If `origin.col` / `origin.row` is not declared in the target table, resolution throws `ReferenceResolutionError` (`ActiveColumnMismatch` / `ActiveRowMismatch`, matching `@col` / `@row`); if `origin.col` is declared as `role: "descriptor"` in the target table or stepping exceeds the target table's axis bounds, resolution throws `ReferenceResolutionError` (`TemporalOffsetOutOfBounds`).
- Excel A1 lowering and `$` pinning:
  - Single cell: Static `Table["r", "c"]` lowers to `$B$5`; any dynamic axis (`@col`, `@row`, `prev`, `next`) lowers to relative `B5`. Cross-sheet references in lowered formulas and sheet-qualified inspection metadata (`physicalAddress`, `sheetAddress`) always single-quote the sheet name (`'Sheet1'!$B$5`, `'Sheet1'!C14`, escaping `'` as `''`).
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
    showGridlines?: boolean; // default false
    freezePanes?: FreezePanesInput; // { row: number, col: number }
    layout?: SheetLayoutMode; // "auto" (default) | "grid"
    colWidth?: number; // finite 1..=255, only valid with layout: "grid"
  },
);

const table: Table = sheet.addTable(
  id: string,
  title: string,
  options?: {
    subtitle?: string | readonly string[];
    currency?: CurrencyCode;
    collapsible?: boolean; // default false
  },
);

export type TableRef = string | Table;
export interface LaneInput {
  readonly tables: readonly TableRef[];
  readonly tableGap?: number;
}
export type LaneInputItem = LaneInput | TableRef;
export interface BandInput {
  readonly lanes: readonly LaneInputItem[];
  readonly gap?: number;
  readonly laneGap?: number;
}
export type BandInputItem = BandInput | TableRef;

sheet.setBands(bands: readonly BandInputItem[]): Sheet;
```

### Container Methods and Properties

| Container | Member                             | Return type                | Behavior                                                                                                                                               |
| :-------- | :--------------------------------- | :------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Model`   | `model.addSheet(name, options?)`   | `Sheet`                    | Appends worksheet; duplicate `name`, invalid `freezePanes`, invalid `layout`, `colWidth` with `"auto"`, or `colWidth` outside `1..=255` throws `Error` |
| `Model`   | `model.getSheet(name)`             | `Sheet`                    | Looks up worksheet by `name` (throws `Error` if absent)                                                                                                |
| `Model`   | `model.getTable(id)`               | `Table`                    | Looks up table by `id` across all worksheets (throws `Error` if absent)                                                                                |
| `Model`   | `model.sheets`                     | `readonly Sheet[]`         | Ordered array of worksheets                                                                                                                            |
| `Model`   | `model.tables`                     | `readonly Table[]`         | Ordered array of all tables across all worksheets                                                                                                      |
| `Model`   | `model.compile()`                  | `CompiledModel`            | Validates, expands, evaluates, lays out, and lowers model                                                                                              |
| `Model`   | `model.toSpec()`                   | `ModelSpec`                | Serializes model into `{ manifest: ProjectManifestInput, tables: readonly TableSpecInput[] }`                                                          |
| `Sheet`   | `sheet.addTable(id, title, opts?)` | `Table`                    | Appends table; duplicate `id` across model throws `Error`                                                                                              |
| `Sheet`   | `sheet.setBands(bands)`            | `Sheet`                    | Replaces worksheet 2D band/lane layout (exact partition of tables on this sheet; accepts `BandInput` or bare `TableRef` shorthand; returns `this`)     |
| `Sheet`   | `sheet.bands`                      | `readonly BandSpecInput[]` | Normalized band layout (`tables: readonly string[]`); defaults to one 1-table band per table in `addTable` order                                       |
| `Sheet`   | `sheet.getTable(id)`               | `Table`                    | Looks up table on this worksheet by `id` (throws `Error` if absent)                                                                                    |
| `Sheet`   | `sheet.tables`                     | `readonly Table[]`         | Tables on this worksheet in `addTable` declaration order (compiled outputs follow canonical `bands -> lanes -> tables` order)                          |
| `Sheet`   | `sheet.toSpec()`                   | `SheetSpecInput`           | Serializes worksheet into raw specification object (re-validates exact partition if `addTable` followed `setBands`)                                    |
| `Table`   | `table.toSpec()`                   | `TableSpecInput`           | Serializes table into raw specification object                                                                                                         |

- Low-level compilation & WASM lifecycle:
  - `compile(manifest, tables) => CompiledModel` compiles raw `ProjectManifestInput` and `readonly TableSpecInput[]` payloads directly.
  - Server/CLI runtimes (Node.js, Bun, Deno) auto-initialize WASM synchronously on module import; browser environments call `await init(moduleOrPath?)` once before calling `compile()`, `model.compile()`, or `defaultTheme()`.
- Theme resolution (`ThemeInput = string | Readonly<Record<string, unknown>> | URL`):
  - `string` (default `"corporate_navy"`): Resolves strictly to a compiled-in preset in `f8-core` (does not read disk files; unknown preset name throws `SpecificationError`).
  - `URL` (e.g. `pathToFileURL("theme.json")` or `new URL("./theme.json", import.meta.url)`): Reads and parses a JSON theme file into `model.theme` via `node:fs`. Use `f8 theme init` (or `defaultTheme()`) to inspect or scaffold the default `corporate_navy` schema.
  - `Readonly<Record<string, unknown>>`: Inline custom theme object. Compiler validates hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA`, or `"none"`/`"transparent"` on fills), positive finite metrics, `{dec}`/`{sym}` format placeholders, and default decimals `<= 14`.
- Worksheet options: `name` enforces Excel worksheet rules (`1..=31` UTF-16 code units, no `: \ / ? * [ ]`, no leading/trailing `'`, not `"History"`, case-insensitive uniqueness; violations throw `SpecificationError`). `tabColor` accepts `#RGB`, `#RRGGBB`, or `#RRGGBBAA` hex color strings (invalid hex throws `SpecificationError`). `freezePanes: { row, col }` sets 0-indexed integer split coordinates within Excel grid bounds (`row < 1_048_576`, `col < 16_384`). `layout: SheetLayoutMode` (`"auto"` default | `"grid"`) selects per-column auto-fit vs. uniform grid layout; `colWidth` (finite `1..=255`, valid only when `layout: "grid"`, defaulting to `theme.metrics.min_col_width`) sets the base grid column width.

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
  - `"descriptor"`: Metadata/label column. Isolated from vector formula broadcasting (`row.expr` and `col.expr` never expand into descriptor columns), isolated from row-level `format`, `decimals`, and `currency`, and rejected in numeric and financial aggregations (`F8Error` `DescriptorReferenceNotAllowed`; valid in `XLOOKUP`, `XMATCH`, and `INDEX`).
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

- Payload exclusivity: `new Cell(options)` (and `DetailedCellInput` on new cells) must specify either `value` or `expr`, never both (`Error` at construction, `SpecificationError` at compile time). Note: `value: null` (or literal `null`) is a valid explicit empty payload (`CellValue::Empty`). Formula expressions evaluating to `Empty` at the cell root throw `F8Error` (`EmptyFormulaResult`); scalar parameters, `1x1` single-cell references, evaluated branch arms (`IF`, `CHOOSE`, `IFERROR`, `XLOOKUP`), and selected lookup/index elements (`XLOOKUP`, `INDEX`) resolving to `Empty` throw `F8Error` (`EmptyArgument`).
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
table.setCells("amort", ["standalone", "adj"], null)
table.setCells(
  { start: "r1", stop: "r2" },
  { start: "c1", stop: "c2" },
  { expr: 'Table[@row, "base"] * Table["mult", @col]' }
)

// Patch metadata on an existing 0D cell (already in row.cells)
table.setCell("r1", "c1", { highlight: true, note: "Patched existing 0D cell" })
```

---

## 5. 3-Tier Specificity Lattice

Every `(row, col)` data coordinate resolves its payload and formatting through a 3-tier specificity lattice (`Tier 1: 0D Cell > Tier 2: 1D Row/Col > Tier 3: Default`):

| Attribute       | Tier 1 (0D Cell)                                                                                 | Tier 2 (1D Row vs. Column on `measure` cols)                                 | Descriptor Column (`role: "descriptor"`)                   | Tier 3 (Fallback Default)                                                                |
| :-------------- | :----------------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------- | :--------------------------------------------------------- | :--------------------------------------------------------------------------------------- |
| **Payload**     | `cell.value` or `cell.expr`                                                                      | `row.expr` XOR `col.expr` (both -> `SpecificationError`)                     | Requires Tier 1 entry (`cells`), else `SpecificationError` | None (unassigned `measure` cell -> `SpecificationError`)                                 |
| **`format`**    | `cell.format`                                                                                    | `row.format` XOR `col.format` (differing -> `SpecificationError`)            | `cell.format` else `col.format` (row isolated)             | `null` (unformatted)                                                                     |
| **`decimals`**  | `cell.decimals`                                                                                  | `row.decimals` XOR `col.decimals` (differing -> `SpecificationError`)        | `cell.decimals` else `col.decimals` (row isolated)         | `theme.number_formats.defaults` for active preset (`null` if non-decimal preset)         |
| **`currency`**  | `cell.currency`                                                                                  | `row.currency` XOR `col.currency` (differing -> `SpecificationError`)        | `null` (descriptor columns ignore `currency`)              | `table.currency` else `"USD"` (only when effective `format === "currency"`, else `null`) |
| **`shade`**     | `cell.shade` (`"none"` clears to `null`)                                                         | `row.shade` XOR `col.shade` (differing non-`"none"` -> `SpecificationError`) | Same as `measure` (row `shade` spans descriptors)          | `null`                                                                                   |
| **`style`**     | `"data"` resets both axes to `"data"`; `"bold"`/`"subtotal"`/`"total"` overrides horizontal axis | Orthogonal 2D composition: `(horizontal=row.style, vertical=col.style)`      | Same as `measure` (row `style` spans descriptors)          | `("data", "data")`                                                                       |
| **`highlight`** | `cell.highlight` (`false` clears inherited highlight)                                            | `Boolean(row.highlight \|\| col.highlight)`                                  | Same as `measure`                                          | `false`                                                                                  |
| **`indent`**    | `cell.indent`                                                                                    | N/A                                                                          | `cell.indent` else `0`                                     | `0` (`indent > 0` on `measure` cell -> `SpecificationError`)                             |
| **`note`**      | `cell.note`                                                                                      | N/A                                                                          | `cell.note` else `null`                                    | `null`                                                                                   |

---

## 6. Layout Elements and Worksheet Placement

### Section Headers and Spacing Gutters

```ts
table.addSectionHeader(title: string, options?: { indent?: number; note?: string }): this;
table.addRowGutter(): this;
table.addColumnGutter(): this;
```

- `addSectionHeader`: Consumes 1 physical row, writes `title` at `table.start_col` with `section_header_format(indent)` and optional Excel note, and allocates zero calculation cells.
- `addRowGutter` / `addColumnGutter`: Consumes 1 blank physical row or column (`gutter_col_width` = `2.0` chars by default) with zero calculation cells; row formatting continues across column gutters while row gutters remain blank. Skipped by vector formulas, `prev`/`next` offsets, and `Table.setCells(...)` `AxisSlice` selectors; formula `start..end` range slices cannot span gutters or section headers (`ReferenceResolutionError`).

### Worksheet Layout (`BandInput`, `LaneInput`, `setBands`)

- Worksheet margins: Physical Row 0 and Column 0 (Column A) are reserved as top and left margins. Band `0` starts at `B2` (`col=1, row=1`).
- Default vertical stacking: Without `sheet.setBands(...)`, each table on the worksheet is placed in its own 1-lane, 1-table band at Column `B` separated by `2` blank gutter rows in `addTable` order.
- Explicit 2D band/lane layout via `sheet.setBands(bands: readonly BandInputItem[]): this`:
  - `BandInput` (`{ lanes: readonly LaneInputItem[], gap?: number, laneGap?: number }`): Horizontal row of 1 or more vertical lanes (`LaneInput` or bare `TableRef` 1-table lane shorthand). Band `b > 0` starts at `prev_band_max_end_row + 1 + (gap ?? 2)`; Lane `l > 0` starts at `prev_lane_max_end_col + 1 + (laneGap ?? 1)`.
  - `LaneInput` (`{ tables: readonly TableRef[], tableGap?: number }`): Vertical stack of 1 or more tables (`Table` or `string` ID, normalized to `readonly string[]`). Table `0` starts at `band_start_row`; Table `t > 0` starts at `prev_table_end_row + 1 + (tableGap ?? 2)`.
  - Bare `TableRef` shorthand: Pass a `Table` or `string` directly inside `setBands([...])` for a 1-lane, 1-table full-width band, or directly inside `lanes: [...]` for a 1-table lane (`{ lanes: [left, right] }`, `{ lanes: [{ tables: [tl, bl] }, tr] }`, or `{ lanes: [card], gap: 1 }`). Multi-table vertical lanes still require explicit `{ tables: [...] }` objects (nested `TableRef[]` arrays are rejected).
  - Non-overlap & ordering: Tables cannot overlap by construction (each lane's width is `max(end_col)` across its tables, so a wider lower-left card cleanly shifts Lane 1 right). Compiled outputs (`summary()`, `errors()`, `inspectSheet()`, `toMarkdown()`) follow canonical `bands -> lanes -> tables` order.
  - Strict validation: Throws `Error` on empty `lanes`/`tables`, nested arrays or invalid lane items, negative or non-integer gaps, dead gaps (`gap` on Band `0`, `laneGap` on a 1-lane band, `tableGap` on a 1-table lane), foreign-sheet or duplicate tables, and missing tables (exact partition enforced on `setBands` and re-validated on `toSpec()` if `addTable` is called later). `setBands([])` is valid on an empty sheet; re-calling `setBands` replaces the layout idempotently.

```ts
sheet.setBands([
  { lanes: [{ tables: [tl, bl] }, tr], laneGap: 1 }, // bare TableRef shorthand for 1-table lane
  timeline, // bare TableRef shorthand for a 1-table full-width band
])
```

- Table banners and collapsing:
  - Non-empty `title` emits a band/lane-aware title banner row (`start_col..=title_end_col`): titled tables in the last lane of any band (including single-lane bands) extend to `sheet_max_end_col`, while titled tables in any non-last lane extend to that lane's `lane_max_end_col` (preserving `band.laneGap` cleanly; subtitles, column headers, section headers, body borders, and `col_widths` stay bounded to `start_col..=end_col`). `subtitle` accepts a single string or an array of strings (`subtitle: ["($ in Millions)", "Audited FY22–FY24"]`, normalized to `table.subtitles: readonly string[]`), emitting one subtitle row per line beneath `title` (on narrow 1–2 column tables, split long subtitles into an array of shorter lines so text does not overflow far to the right; passing an empty or whitespace-only line `""` throws `SpecificationError`; omit `subtitle` or pass `[]` when unused). If either `title` or `subtitle` is present, a 1-row buffer gutter is inserted before column headers; passing `title: ""` (with no `subtitle`) omits both banner and buffer gutter so column headers start at `start_row`.
  - `collapsible: true`: Groups physical rows `start_row + 1..=end_row` in Excel outline with summary symbols above (`group_symbols_above(true)`).
- Column width arbitration & text wrapping (`layout: "auto"`, default): Each physical worksheet column takes the maximum width across all tables spanning that column, computed as `round(clamp(max_chars * char_width_multiplier + col_padding, min_col_width, max_col_width), 2)` (default `col_padding=0.0`, `min_col_width=12.0`, `max_col_width=75.0`, where `max_chars` spans per-column header lines—excluding spanning `group`—and `\n`-delimited data cell segments with `indent * 2` plus `+1` alignment-space character on non-empty `measure` headers, `date`/numeric/unformatted `measure` numbers not ending in `)`, and non-empty text cells under trailing-`_)` formats while leaving `descriptor` headers and unformatted `descriptor` cells unpadded; gutter-only columns use `gutter_col_width=2.0`). Cells whose effective width exceeds `max_col_width` or that contain `\n` newlines enable OpenXML `wrapText="1"` (`FormatAlign::VerticalCenter` on all data cells) while leaving `.xlsx` row heights automatic for Excel font auto-fitting.
- Uniform grid layout & descriptor overflow (`layout: "grid"`): Designed for multi-table dashboards and financial statements where multi-lane card bands sit above wide timelines without stretching measure columns. Every physical column covered by any data column span shares uniform width `W = round(max(base_w, max_measure_cell_width, max_non_text_descriptor_cell_width), 2)` (floored at `base_w = round(colWidth ?? theme.metrics.min_col_width, 2)` in `1..=255`, ignoring `max_col_width` and capped at `255`; column `0` and gutter/gap-only columns retain `gutter_col_width=2.0`). `descriptor` columns automatically span `span >= 1` contiguous physical columns (`ceil(req_w / W)` from non-empty descriptor headers/text with `+1` breathing space and `indent * 2`) with zero merged cells—anchor cells stay at `phys_start` (preserving vertical `XLOOKUP`/`XMATCH`/`INDEX` slices and contiguous `measure` ranges) while styled blank runway cells carry continuous shading/borders and move vertical right borders to `phys_end`. Finalizing lanes in ascending `lane_idx` (`0..=max_lane_idx`), tables with a leading `descriptor` before their first `measure` align their leading descriptor block (through the last leading `descriptor`, excluding trailing `addColumnGutter()` spacers after it so forecast-only schedules skipping earlier periods align vertically with stacked timelines) within each `(lane_idx, start_col)` group by extending their last leading `descriptor` (descriptor-only tables skip alignment; span growth in Lane `k` automatically shifts Lane `k+1`). Strict `grid` rules (violations throw `SpecificationError`): `wrap_text` is always `false`, `\n`/`\r` is forbidden in data cells, column `headers`, and `group` banners, and every non-empty `measure` column header line must fit within static `base_w` (split long headers into multi-tier `headers: [...]`, abbreviate, or raise `colWidth`).

---

## 7. Formatting Presets, Styles, Shades, and Currencies

### Format Presets (`FormatPreset`)

Default `corporate_navy` Excel masks reserve a uniform trailing alignment space (`_)`) across right-aligned `measure` elements: measure column headers use `theme.number_formats.text` (`@_)`), numeric presets append a 4th `;@_)` text section, `date` uses `yyyy-mm-dd_)`, and unformatted (`null`) measure cells use `theme.number_formats.general` (`General_);(General);0_);@_)`). Setting any template to `""` or `"General"` in a custom theme emits native Excel `numFmtId="0"`; left-aligned `descriptor` headers and unformatted `descriptor` cells always use native `General`.

| Preset           | Default `decimals` | Supports `decimals` | Supports `currency` | Sample inputs (`pos`, `neg`, `zero`) | In-memory formatted output (`formatted()`, `toMarkdown()`)                                                                                                                                              |
| :--------------- | :----------------- | :------------------ | :------------------ | :----------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `"currency"`     | `0`                | Yes (`0..=14`)      | Yes (default `USD`) | `1250`, `-1250`, `0`                 | `"$1,250"` / `"($1,250)"` / `"$0"` (Excel zero mask: `$-`)                                                                                                                                              |
| `"number"`       | `0`                | Yes (`0..=14`)      | No                  | `1250`, `-1250`, `0`                 | `"1,250"` / `"(1,250)"` / `"0"` (Excel zero mask: `-`)                                                                                                                                                  |
| `"percentage"`   | `1`                | Yes (`0..=14`)      | No                  | `0.15`, `-0.15`, `0`                 | `"15.0%"` / `"(15.0%)"` / `"0.0%"` (Excel zero mask: `-`)                                                                                                                                               |
| `"multiple"`     | `1`                | Yes (`0..=14`)      | No                  | `12.5`, `-12.5`, `0`                 | `"12.5x"` / `"(12.5x)"` / `"0.0x"` (Excel neg: `(12.5x)`)                                                                                                                                               |
| `"basis_points"` | `0`                | Yes (`0..=14`)      | No                  | `0.015`, `-0.015`, `0`               | `"150 bps"` / `"(150 bps)"` / `"0 bps"` (scales `10,000x` in-memory; Excel `#,##0" bps"` does not scale)                                                                                                |
| `"integer"`      | `null`             | No                  | No                  | `2024`, `-2024`, `0`                 | `"2024"` / `"(2024)"` / `"0"` (Excel: `2024` / `(-2024)` / `-`)                                                                                                                                         |
| `"date"`         | `null`             | No                  | No                  | `45657` (Excel serial)               | `"2024-12-31"` (Excel: `yyyy-mm-dd_)`)                                                                                                                                                                  |
| `"text"`         | `null`             | No                  | No                  | `"Sample"`                           | `"Sample"` (Excel: `@_)`)                                                                                                                                                                               |
| `null` (default) | `null`             | Yes (in-memory)     | No                  | `1250`, `-12.5`, `0`                 | `"1,250"` / `"(12.5)"` / `"0"` (uses explicit `decimals` if set, else rounds to `1` decimal place and trims trailing `.0`; Excel measure: `theme.number_formats.general`, descriptor: native `General`) |

### Semantic Styles (`SemanticStyle`)

Structural summary and typographic emphasis roles for rows, columns, and 0D cell overrides. Use `"bold"` when you only want bold typography (e.g. KPI/ratio rows, `LTM` or primary scenario columns, or descriptor labels); reserve `"subtotal"` and `"total"` strictly for true accounting summary rows and terminal summary/aggregation columns (`Total`, `Cumulative`):

- `"data"` (default): Standard body typography and borders.
- `"bold"`: Bold typography only; no borders and standard fill/text color precedence (`highlight` > `shade` > `data`, preserving blue `input_text`).
- `"subtotal"`: Bold typography; thin top border on rows (`subtotal_border_top`), dashed left border on columns (`col_subtotal_border_left`).
- `"total"`: Bold typography; thin top and double bottom border on rows (`total_border_top`, `total_border_bottom`), thin left border on columns (`col_total_border_left`; optional `col_total_border_right` defaults to `"none"`).

### Background Shades (`Shade`)

Applies theme palette background fill to body cells without changing font color (fill resolution: `highlight_fill` if highlighted; else `total_fill.or(shade_fill)` if `total`; else `subtotal_fill.or(shade_fill)` if `subtotal`; else `shade_fill.or(data_fill)`; font color precedence: `input_text` on `measure` literal inputs `> highlight_text > total_text > subtotal_text > data_text` on `descriptor` cells `> formula_text`):

- `"none"`: No background shade; clears inherited row/column shade when set on a 0D cell override.
- `"muted"`: Maps to `theme.palette.shade_muted_fill`.
- `"alt"`: Maps to `theme.palette.shade_alt_fill`.

### Currency Codes (`CurrencyCode`)

Case-insensitive 3-letter ISO codes (normalized to lowercase in spec payloads):
`USD`, `EUR`, `GBP`, `JPY`, `CHF`, `CAD`, `AUD`, `CNY`, `HKD`, `SGD`, `SEK`, `NOK`, `NZD`, `INR`, `BRL`.
