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

- Table identifiers: Bare ASCII identifiers (`[A-Za-z0-9_]+` containing at least one ASCII letter or `_`, excluding reserved tokens `TRUE`, `FALSE`, `_`, and scientific-notation-shaped tokens like `1e5`) or `@table` before `[`. Invalid table IDs throw `SpecificationError`; quoted table names (`"Table"[...]`), pure numbers (`123[...]`), or dot access (`Table.row`) in formulas throw `FormulaParseError`.
- Coordinate keys: Explicit row and column IDs inside `[...]` must be double-quoted strings (`"revenue"`, `"2024E"`). Unquoted keys (`Table[revenue, @col]`, `Table["revenue", 2024E]`) throw `FormulaParseError`.
- Numeric and string literals: Numeric literals support integers, decimals, and scientific notation (`[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?`, e.g., `42`, `0.25`, `1e-9`, `1.5E+6`) within finite IEEE-754 `f64` bounds (overflow such as `1e500` or malformed tokens such as `1.5e-` throw `FormulaParseError`; finite magnitudes outside `[1e-6, 1e15)` lower to compact uppercase `E` notation in Excel). String literals and quoted coordinate keys use double quotes with `\"` and `\\` escapes.
- Active axis tokens: Must be `@col`, `@row`, or `@table` (before `[`). Bare `@` throws `FormulaParseError`.
- Uppercase identifiers: Function names (`[A-Z][A-Z0-9_]*`, e.g., `SUM`, `XLOOKUP`) and boolean literals (`TRUE`, `FALSE`) must be uppercase (table identifiers before `[` may be mixed-case). Lowercase functions/booleans, dotted function names (`ERROR.TYPE`), `null`, or standalone identifiers throw `FormulaParseError`.
- Omitted arguments (`_`): Syntactically confined to direct function call arguments (`PMT(rate, nper, pv, _, type)`); using `_` in arithmetic or coordinate selectors throws `FormulaParseError`. Valid only on intermediate optional parameters followed by at least one explicit argument (omitting a required or trailing parameter with `_` throws `F8Error`).
- Operators and precedence (highest to lowest):
  1. Exponentiation: `^` (right-associative, binds tighter than unary `-` so `-2 ^ 2` evaluates to `-4` and lowers to `=-(2^2)`; numeric operands required; `0 ^ 0` and negative base with non-integer exponent return `#NUM!`, `0 ^ -n` returns `#DIV/0!`).
  2. Unary negation: `-` (right-associative; numeric operand required; unary `+` throws `FormulaParseError`).
  3. Multiplicative: `*`, `/` (left-associative; numeric operands required).
  4. Additive: `+`, `-` (left-associative; numeric operands required).
  5. Relational comparisons: `=`, `<>`, `<`, `<=`, `>`, `>=` (non-associative unless parenthesized as `(a = b) = c`; unparenthesized chained comparisons throw `FormulaParseError`). Operands must be non-empty values of identical type (`Number`, `Text` via ASCII case-insensitive comparison, or `Boolean`; cross-type or empty operands throw `F8Error`, and runtime Excel errors bubble). `==`, `!=`, and `&` throw `FormulaParseError`.
- Slice endpoints, ordering, and contiguity: Range endpoints (`start..end`) accept quoted keys, `@col`/`@row`, or `prev`/`next` offsets on either side. Resolved indices must follow declared table order (`start <= end`; inverted ranges throw `ReferenceResolutionError`). Row range slices must span contiguous data rows with no intervening row gutters or section headers, and horizontal column range slices must span contiguous `measure` columns with no `descriptor` or gutter columns (violations throw `ReferenceResolutionError`). 1D vertical slices on a single `descriptor` column (`Table["r1".."r2", "line_item"]`) are valid in `XLOOKUP`, `XMATCH`, and `INDEX`, while numeric and financial aggregations reject `descriptor` columns (`F8Error`).
- Offset boundaries: `col.prev` / `col.next` anchor the origin column ID in the target table and step across `role: "measure"` columns (skipping `descriptor` and gutter columns); `row.prev` / `row.next` anchor the origin row ID in the target table and step across data rows (skipping section headers and gutter rows). Missing origin IDs, `descriptor`-anchored column offsets, or steps past axis bounds throw `ReferenceResolutionError`.
- Excel A1 lowering and `$` pinning:
  - Single cell: Static `Table["r", "c"]` lowers to `$B$5`; any dynamic axis (`@col`, `@row`, `prev`, `next`) lowers to relative `B5`. Cross-sheet references in lowered formulas and sheet-qualified inspection addresses (`physicalAddress`, `sheetAddress`) single-quote the sheet name (`'Sheet1'!$B$5`, `'Sheet1'!C14`, escaping `'` as `''`).
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

sheet.setBands(bands: readonly BandInputItem[]): this;
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
| `Sheet`   | `sheet.setBands(bands)`            | `this`                     | Replaces worksheet 2D band/lane layout (exact partition of tables on this sheet; accepts `BandInput` or bare `TableRef` shorthand; returns `this`)     |
| `Sheet`   | `sheet.bands`                      | `readonly BandSpecInput[]` | Normalized band layout (`tables: readonly string[]`); defaults to one 1-table band per table in `addTable` order                                       |
| `Sheet`   | `sheet.getTable(id)`               | `Table`                    | Looks up table on this worksheet by `id` (throws `Error` if absent)                                                                                    |
| `Sheet`   | `sheet.tables`                     | `readonly Table[]`         | Tables on this worksheet in `addTable` declaration order (compiled outputs follow canonical `bands -> lanes -> tables` order)                          |
| `Sheet`   | `sheet.toSpec()`                   | `SheetSpecInput`           | Serializes worksheet into raw specification object (re-validates exact partition if `addTable` followed `setBands`)                                    |
| `Table`   | `table.toSpec()`                   | `TableSpecInput`           | Serializes table into raw specification object                                                                                                         |

- Low-level compilation & WASM lifecycle:
  - `compile(manifest, tables) => CompiledModel` compiles raw `ProjectManifestInput` and `readonly TableSpecInput[]` payloads directly.
  - Server/CLI runtimes (Node.js, Bun, Deno) auto-initialize WASM synchronously on module import; browser environments call `await init(moduleOrPath?)` once before calling `compile()`, `model.compile()`, or `defaultTheme()`.
- Theme resolution (`ThemeInput = string | Readonly<Record<string, unknown>> | URL`):
  - `string` (default `"corporate_navy"`): Resolves to a compiled-in preset (does not read disk files; unknown preset throws `SpecificationError`).
  - `URL` (e.g. `pathToFileURL("theme.json")` or `new URL("./theme.json", import.meta.url)`): Reads and parses a JSON theme file into `model.theme` via `node:fs`. Use `f8 theme init` or `defaultTheme()` to inspect or scaffold the default `corporate_navy` schema.
  - `Readonly<Record<string, unknown>>`: Inline custom theme object. Compiler validates hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA`, or `"none"`/`"transparent"` on fills), positive finite metrics (`col_padding >= 0`, `min_col_width <= max_col_width`), `{dec}`/`{sym}` format placeholders, and default decimals `<= 14`.
- Worksheet options: `name` enforces Excel worksheet rules (`1..=31` UTF-16 code units, no `: \ / ? * [ ]`, no leading/trailing `'`, not `"History"`, case-insensitive uniqueness; violations throw `SpecificationError`). `tabColor` accepts `#RGB`, `#RRGGBB`, or `#RRGGBBAA` hex strings (invalid hex throws `SpecificationError`). `freezePanes: { row, col }` sets 0-indexed integer split coordinates within Excel grid bounds (`row < 1_048_576`, `col < 16_384`). `layout: SheetLayoutMode` (`"auto"` default | `"grid"`) selects per-column auto-fit vs. uniform grid layout; `colWidth` (finite `1..=255`, valid only when `layout: "grid"`, defaulting to `theme.metrics.min_col_width`) sets the base grid column width.

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
  - `"descriptor"`: Label/metadata column. Isolated from vector formula broadcasting (`row.expr` and `col.expr` never expand into descriptor columns) and row-level `format`, `decimals`, and `currency`; rejected in numeric and financial aggregations (`F8Error`; valid in `XLOOKUP`, `XMATCH`, and `INDEX`).
- Headers and group banners:
  - Pass `header: "Title"` (1 tier) or `headers: ["Tier 1", "Tier 2"]` (stacked tiers within the column); passing both throws `Error`.
  - Pass `group: "Banner"` for a Tier 0 horizontal banner spanning contiguous columns with the same `group` string (rendered with `CenterAcross` using the theme's super-header styling; excluded from single-column width calculation). Empty or whitespace-only `group` throws `Error`.
  - Per-column header depth is `Math.max(headers.length, 1) + 1` when `group` is set, or `headers.length` (`0` if omitted) when ungrouped. Table header depth is the maximum per-column depth across all columns (`0` header rows when ungrouped and no column sets `header`/`headers`). When header rows exist (`depth > 0`), columns with fewer lines are bottom-aligned; if a column sets neither `header` nor `headers`, `.xlsx` emission writes a blank header cell while `inspectSheet` and `TableAuditor.toMarkdown()` fall back to `col.id`.
- Vertical vector formula (`expr`): Evaluates top-to-bottom across data rows in `measure` columns (ignored on `descriptor` columns, which require explicit Tier 1 `cells` entries).

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

- Horizontal vector formula (`expr`): Evaluates left-to-right across `measure` columns. When `expr` uses `col.prev` on the first `measure` column, supply an explicit 0D override for that column in `cells`.
- Duplicate `id` in `addColumn` or `addRow` throws `Error`. Tables with empty `columns` or `rows` throw `SpecificationError`.

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

- Payload exclusivity: `new Cell(options)` (and `DetailedCellInput` on new cells) must specify either `value` or `expr`, never both (`Error` at construction, `SpecificationError` at compile time). `value: null` (or literal `null`) is a valid explicit empty cell; formula cells cannot evaluate to empty at the cell root, and scalar args, `1x1` cell refs, evaluated branch arms, and selected lookup/index elements reject empty (`F8Error`).
- Constraint checks (violations throw `SpecificationError`):
  - `indent`: Non-negative integer (`0..=255`); allowed only on `role: "descriptor"` cells (`indent > 0` on a `measure` cell is rejected).
  - `currency`: Rejects pairing `currency` with a non-`"currency"` `format` on the same spec level (`cell`, `row`, or `col`). If a cell's resolved effective `format` is not `"currency"` (or the column is `role: "descriptor"`), effective `currency` resolves to `null`.
  - `decimals`: Requires `0 <= decimals <= 14`. Pairing `decimals` with `"integer"`, `"date"`, or `"text"` on the same spec level (`cell`, `row`, or `col`) throws `SpecificationError`. If effective `format` is `"integer"`, `"date"`, or `"text"`, effective `decimals` resolves to `null` (when effective `format` is `null`, explicit `decimals` is retained for in-memory formatting).
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
  - **Existing 0D cell**: If `(row, col)` already has an entry in `row.cells`, `setCell` / `setCells` merges defined metadata from `DetailedCellInput` / `Cell` (`style`, `highlight`, `shade`, `format`, `decimals`, `currency`, `indent`, `note`) while preserving its existing `value` or `expr` (providing a new `value`, `expr`, or bare scalar/`null` replaces only the payload).
  - **New 0D override**: If `(row, col)` has no 0D entry in `row.cells` (including cells currently covered only by `row.expr` or `col.expr`), `setCell` / `setCells` must provide `value` (or a scalar literal / `null`) or `expr`; omitting payload throws `Error`.
- Axis selectors (`AxisSelector`):
  - Accepts a single ID (`string`), a non-empty `Iterable<string>` (e.g. `string[]`), or an inclusive `AxisSlice` (`{ start: "start_id", stop: "stop_id" }`) across declared data rows or data columns (skipping gutters and section headers).
  - Unknown IDs, inverted slices, or empty iterables throw `Error`.
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

Every `(row, col)` data coordinate resolves payload and formatting through a 3-tier specificity lattice (`Tier 1: 0D Cell > Tier 2: 1D Row/Col > Tier 3: Default`):

| Attribute       | Tier 1 (0D Cell)                                                                                 | Tier 2 (1D Row vs. Column on `measure` cols)                                 | Descriptor Column (`role: "descriptor"`)                   | Tier 3 (Fallback Default)                                                                |
| :-------------- | :----------------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------- | :--------------------------------------------------------- | :--------------------------------------------------------------------------------------- |
| **Payload**     | `cell.value` or `cell.expr`                                                                      | `row.expr` XOR `col.expr` (both -> `SpecificationError`)                     | Requires Tier 1 entry (`cells`), else `SpecificationError` | None (unassigned `measure` cell -> `SpecificationError`)                                 |
| **`format`**    | `cell.format`                                                                                    | `row.format` XOR `col.format` (differing -> `SpecificationError`)            | `cell.format` else `col.format` (row isolated)             | `null` (unformatted)                                                                     |
| **`decimals`**  | `cell.decimals`                                                                                  | `row.decimals` XOR `col.decimals` (differing -> `SpecificationError`)        | `cell.decimals` else `col.decimals` (row isolated)         | `theme.number_formats.defaults` at format/emission (`null` in inspection)                |
| **`currency`**  | `cell.currency`                                                                                  | `row.currency` XOR `col.currency` (differing -> `SpecificationError`)        | `null` (descriptor columns ignore `currency`)              | `table.currency` else `"usd"` (only when effective `format === "currency"`, else `null`) |
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

- `addSectionHeader`: Consumes 1 physical row, writes `title` at the table's starting column with the theme's section-header style, optional indentation, and optional Excel note, and allocates zero calculation cells.
- `addRowGutter` / `addColumnGutter`: Consumes 1 blank physical row or column (`gutter_col_width=2.0` chars by default) with zero calculation cells; row formatting continues across column gutters while row gutters remain blank. Skipped by vector formulas, `prev`/`next` offsets, and `Table.setCells(...)` `AxisSlice` selectors; formula `start..end` slices cannot span gutters or section headers (`ReferenceResolutionError`).

### Worksheet Layout (`BandInput`, `LaneInput`, `setBands`)

- Worksheet margins: Physical Row 0 (Row 1) and Column 0 (Column A) are reserved as blank margins. Band `0` starts at `B2` (`col=1, row=1`).
- Default vertical stacking: Without `sheet.setBands(...)`, each table is placed in its own 1-lane, 1-table band at Column `B` separated by `2` blank rows in `addTable` order.
- Explicit 2D band/lane layout via `sheet.setBands(bands: readonly BandInputItem[]): this`:
  - `BandInput` (`{ lanes: readonly LaneInputItem[], gap?: number, laneGap?: number }`): Horizontal row of 1 or more vertical lanes (`LaneInput` or bare `TableRef` 1-table lane shorthand). Band `b > 0` starts `gap` (default `2`) blank rows below the previous band's lowest row; Lane `l > 0` starts `laneGap` (default `1`) blank columns right of the previous lane's rightmost column.
  - `LaneInput` (`{ tables: readonly TableRef[], tableGap?: number }`): Vertical stack of 1 or more tables (`Table` or `string` ID, normalized to `readonly string[]`). Table `0` starts at the band's top row; Table `t > 0` starts `tableGap` (default `2`) blank rows below the previous table in the lane.
  - Bare `TableRef` shorthand: Pass a `Table` or `string` directly inside `setBands([...])` for a 1-lane, 1-table band, or directly inside `lanes: [...]` for a 1-table lane (`{ lanes: [left, right] }`, `{ lanes: [{ tables: [tl, bl] }, tr] }`, or `{ lanes: [card], gap: 1 }`). Multi-table vertical lanes require explicit `{ tables: [...] }` objects (nested `TableRef[]` arrays are rejected).
  - Non-overlap & ordering: Tables cannot overlap by construction (each lane's width is the maximum end column across its tables, shifting subsequent lanes right automatically). Compiled outputs (`summary()`, `errors()`, `inspectSheet()`, `toMarkdown()`) follow canonical `bands -> lanes -> tables` order.
  - Validation: Throws `Error` on empty `lanes`/`tables`, nested arrays or invalid lane items, negative or non-integer gaps, unused gaps (`gap` on Band `0`, `laneGap` on a 1-lane band, `tableGap` on a 1-table lane), foreign-sheet or duplicate tables, and missing tables (exact partition enforced on `setBands` and re-validated on `toSpec()` if `addTable` is called later). `setBands([])` is valid on an empty sheet; re-calling `setBands` replaces the layout idempotently.

```ts
sheet.setBands([
  { lanes: [{ tables: [tl, bl] }, tr], laneGap: 1 }, // bare TableRef shorthand for 1-table lane
  timeline, // bare TableRef shorthand for a 1-table band
])
```

### Table Banners and Collapsing

- Title banners: Non-empty `title` emits a title banner row spanning from the table's starting column to the sheet's rightmost column for tables in the last lane of any band (including single-lane bands), or to that lane's rightmost column for tables in non-last lanes. Subtitles, column headers, section headers, body borders, and column widths stay bounded to the table's own columns.
- Subtitles & buffer gutter: `subtitle` accepts a single string or an array of strings (normalized to `table.subtitles: readonly string[]`), emitting one row per entry beneath `title` (empty or whitespace-only entries throw `SpecificationError`; omit or pass `[]` when unused). When either `title` or `subtitle` is present, a 1-row blank buffer gutter is inserted before column headers; passing `title: ""` with no `subtitle` omits both banner and buffer gutter.
- Collapsing (`collapsible: true`): Groups rows below the table's first physical row in an Excel outline with summary symbols above.

### Column Width Arbitration (`layout: "auto"`, default)

- Per-column width: Each physical column takes the maximum width across all tables spanning it: `round(clamp(max_chars * char_width_multiplier + col_padding, min_col_width, max_col_width), 2)` (defaults: `char_width_multiplier=1.0`, `col_padding=0.0`, `min_col_width=12.0`, `max_col_width=75.0`). Column A and gutter/gap-only columns use `gutter_col_width` (`2.0` default).
- Character demand (`max_chars`): Longest line across per-column headers (excluding spanning `group` banners) and `\n`-delimited data cell segments, adding `indent * 2` plus `+1` trailing alignment character on non-empty `measure` headers, non-parenthesized numbers under `date`/numeric presets or unformatted `measure` cells, and non-empty text cells under trailing-`_)` formats (leaving `descriptor` headers, unformatted `descriptor` cells, and `text`-formatted numbers unpadded).
- Text wrapping: Cells whose width exceeds `max_col_width` or that contain `\n` enable OpenXML `wrapText="1"` (all data cells are vertically centered); `.xlsx` row heights remain automatic.

### Uniform Grid Layout (`layout: "grid"`)

- Uniform column width `W`: Every physical column covered by any data column span shares width `W = round(max(base_w, max_measure_cell_width, max_non_text_descriptor_cell_width), 2)`, where `base_w = round(colWidth ?? theme.metrics.min_col_width, 2)` in `1..=255`. `grid` ignores `max_col_width`; if `base_w` or computed `W` exceeds `255`, compilation fails with `SpecificationError` (it is not clamped). Column A and gutter/gap-only columns keep `gutter_col_width` (`2.0` default).
- Descriptor overflow spans: `descriptor` columns automatically span `ceil(req_w / W)` (`>= 1`) contiguous physical columns (from non-empty descriptor headers/text with `+1` breathing space and `indent * 2`) with zero merged cells. Anchor cells stay at the span's first column (preserving vertical `XLOOKUP`/`XMATCH`/`INDEX` slices and contiguous `measure` ranges), while styled blank runway cells carry continuous fills/borders and place vertical right borders on the span's final column.
- Leading descriptor alignment: Lanes finalize left-to-right. Within each `(laneIndex, startCol)` group, tables with at least one `descriptor` before their first `measure` column align their leading descriptor block (through the last leading `descriptor`, excluding trailing `addColumnGutter()` spacers after it) by extending their last leading `descriptor` span. Descriptor-only tables skip alignment; span growth in an earlier lane shifts later lanes right automatically.
- `grid` constraints (violations throw `SpecificationError`): Text wrapping is disabled, `\n`/`\r` line breaks are forbidden in data cells, column `headers`, and `group` banners, and every non-empty `measure` column header line must fit within static `base_w`.

---

## 7. Formatting Presets, Styles, Shades, and Currencies

### Format Presets (`FormatPreset`)

Default `corporate_navy` Excel masks reserve a uniform trailing alignment space (`_)`) on right-aligned `measure` elements: measure column headers use `theme.number_formats.text` (`@_)`), numeric presets append a 4th `;@_)` text section, `date` uses `yyyy-mm-dd_)`, and unformatted (`null`) measure cells use `theme.number_formats.general` (`General_);(General);0_);@_)`). Setting any template to `""` or `"General"` in a custom theme emits native Excel `numFmtId="0"`; left-aligned `descriptor` headers and unformatted `descriptor` cells use native `General`.

| Preset           | Default `decimals` | Supports `decimals` | Supports `currency` | Sample inputs (`pos`, `neg`, `zero`) | In-memory formatted output (`formatted()`, `toMarkdown()`)                                                                                                                                              |
| :--------------- | :----------------- | :------------------ | :------------------ | :----------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `"currency"`     | `0`                | Yes (`0..=14`)      | Yes (default `USD`) | `1250`, `-1250`, `0`                 | `"$1,250"` / `"($1,250)"` / `"$0"` (Excel zero mask: `-`)                                                                                                                                               |
| `"number"`       | `0`                | Yes (`0..=14`)      | No                  | `1250`, `-1250`, `0`                 | `"1,250"` / `"(1,250)"` / `"0"` (Excel zero mask: `-`)                                                                                                                                                  |
| `"percentage"`   | `1`                | Yes (`0..=14`)      | No                  | `0.15`, `-0.15`, `0`                 | `"15.0%"` / `"(15.0%)"` / `"0.0%"` (Excel zero mask: `-`)                                                                                                                                               |
| `"multiple"`     | `1`                | Yes (`0..=14`)      | No                  | `12.5`, `-12.5`, `0`                 | `"12.5x"` / `"(12.5x)"` / `"0.0x"` (Excel: `12.5x` / `(12.5x)` / `-`)                                                                                                                                   |
| `"basis_points"` | `0`                | Yes (`0..=14`)      | No                  | `0.015`, `-0.015`, `0`               | `"150 bps"` / `"(150 bps)"` / `"0 bps"` (scales `10,000x` in-memory; Excel `#,##0" bps"` does not scale; Excel zero mask: `-`)                                                                          |
| `"integer"`      | `null`             | No                  | No                  | `2024`, `-2024`, `0`                 | `"2024"` / `"(2024)"` / `"0"` (Excel: `2024` / `(-2024)` / `-`)                                                                                                                                         |
| `"date"`         | `null`             | No                  | No                  | `45657` (Excel serial)               | `"2024-12-31"` (Excel: `yyyy-mm-dd_)`)                                                                                                                                                                  |
| `"text"`         | `null`             | No                  | No                  | `"Sample"`                           | `"Sample"` (Excel: `@_)`)                                                                                                                                                                               |
| `null` (default) | `null`             | Yes (in-memory)     | No                  | `1250`, `-12.5`, `0`                 | `"1,250"` / `"(12.5)"` / `"0"` (uses explicit `decimals` if set, else rounds to `1` decimal place and trims trailing `.0`; Excel measure: `theme.number_formats.general`, descriptor: native `General`) |

### Semantic Styles (`SemanticStyle`)

Typography, border, and fill roles for rows, columns, and 0D cell overrides (borders and fills come from the active theme; defaults shown for `corporate_navy`):

- `"data"` (default): Regular typography, `data_fill`, and no summary borders.
- `"bold"`: Bold typography only; no borders, using standard fill/text color precedence (`highlight` > `shade` > `data`, preserving `input_text` on `measure` literals).
- `"subtotal"`: Bold typography; `subtotal_fill`/`subtotal_text`, thin top border on rows (`subtotal_border_top`), dashed left border on columns (`col_subtotal_border_left`).
- `"total"`: Bold typography; `total_fill`/`total_text`, thin top and double bottom border on rows (`total_border_top`, `total_border_bottom`), thin left border on columns (`col_total_border_left`; `col_total_border_right` defaults to `"none"`).

### Background Shades (`Shade`)

Applies theme palette background fill to body cells without changing font color (fill precedence: `highlight_fill` if highlighted; else `total_fill` or shade if `total`; else `subtotal_fill` or shade if `subtotal`; else shade or `data_fill`; font color precedence: `input_text` on `measure` literal inputs > `highlight_text` > `total_text` > `subtotal_text` > `data_text` on `descriptor` cells > `formula_text`):

- `"none"`: No background shade; clears inherited row/column shade when set on a 0D cell override.
- `"muted"`: Maps to `theme.palette.shade_muted_fill`.
- `"alt"`: Maps to `theme.palette.shade_alt_fill`.

### Currency Codes (`CurrencyCode`)

3-letter ISO codes normalized to lowercase by builder APIs and `compile(...)` (`CurrencyCode` types accept lowercase or uppercase):
`USD`, `EUR`, `GBP`, `JPY`, `CHF`, `CAD`, `AUD`, `CNY`, `HKD`, `SGD`, `SEK`, `NOK`, `NZD`, `INR`, `BRL`.
