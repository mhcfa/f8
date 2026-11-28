# Syntax and Grid Reference

2D coordinate grammar, core containers (`Model`, `Sheet`, `Table`, `compile`, `init`), column/row builders, 0D overrides, and the 3-tier specificity lattice.

## 1. 2D Coordinate Addressing Grammar

Every formula cell reference uses explicit 2D bracket syntax:

`Table[RowSelector, ColSelector]` or `@table[RowSelector, ColSelector]`

### Coordinate Selectors

| Selector            | Syntax                                      | Semantics                          | Example                                      |
| :------------------ | :------------------------------------------ | :--------------------------------- | :------------------------------------------- |
| Active table        | `@table["row", @col]`                       | Current table (intra-table)        | `@table["rev", @col] - @table["cogs", @col]` |
| Active column       | `Table["row", @col]`                        | Current evaluation column          | `IS["rev", @col] * 0.4`                      |
| Active row          | `Table[@row, "col"]`                        | Current evaluation row             | `Assump[@row, "rate"] * 100`                 |
| Absolute point      | `Table["row", "col"]`                       | Fixed cell (`$Col$Row`)            | `Assump["tax_rate", "val"]`                  |
| Column lag/lead (1) | `Table["row", col.prev]` / `col.next`       | Prior/next `measure` column        | `BS["cash", col.prev] + 10`                  |
| Column lag/lead (k) | `Table["row", col.prev(k)]` / `col.next(k)` | `k` `measure` cols (`1..=2^31-1`)  | `Rev["sales", col.prev(2)]`                  |
| Row lag/lead (1)    | `Table[row.prev, "col"]` / `row.next`       | Prior/next data row                | `Sched[row.prev, "col"] * 1.05`              |
| Row lag/lead (k)    | `Table[row.prev(k), "col"]` / `row.next(k)` | `k` data rows (`1..=2^31-1`)       | `Sched[row.next(2), "col"]`                  |
| Column slice        | `Table["row", "c1".."c2"]`                  | Inclusive horizontal range         | `SUM(Rev["sales", "2023A".."2025E"])`        |
| Dynamic col slice   | `Table["row", "c1"..@col]`                  | Any two column endpoints           | `SUM(Debt["amort", "2024E"..col.prev])`      |
| Row slice (active)  | `Table["r1".."r2", @col]`                   | Inclusive vertical range at `@col` | `SUM(Opex["rd".."ga", @col])`                |
| Dynamic row slice   | `Table["r1"..@row, @col]`                   | Any two row endpoints              | `SUM(Cohort["c1"..row.prev, @col])`          |
| Row slice (fixed)   | `Table["r1".."r2", "col"]`                  | Vertical range at fixed column     | `SUM(Comps["p1".."p5", "ev"])`               |
| 2D block slice      | `Table["r1".."r2", "c1".."c2"]`             | Inclusive 2D rectangular range     | `SUM(M["r1".."r3", "2023A".."2025E"])`       |

### Lexical, Operator, Slice, and Lowering Rules

- Table IDs & keys: Bare ASCII `[A-Za-z0-9_]+` with at least one letter or `_` (excluding `TRUE`, `FALSE`, `_`, and scientific tokens like `1e5`) or `@table` before `[`. Row/column keys inside `[...]` must be double-quoted (`\"` and `\\` escapes). Invalid table IDs throw `SpecificationError`; quoted table names, pure numeric table IDs, unquoted keys, or dot access (`Table.row`) throw `FormulaParseError`.
- Literals & tokens: Integers, decimals, and scientific notation (`[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?`) within finite `f64` bounds (overflow or malformed tokens throw `FormulaParseError`). Function names (`[A-Z][A-Z0-9_]*`) and booleans (`TRUE`, `FALSE`) must be uppercase; active tokens are `@col`, `@row`, `@table`. Lowercase functions/booleans, dotted function names, `null`, bare `@`, or standalone identifiers throw `FormulaParseError`.
- Omitted arguments (`_`): Syntactically confined to direct function call arguments (`PMT(r, n, pv, _, 1)`); `_` outside call arguments throws `FormulaParseError`. Only intermediate optional arguments are valid (see `references/functions.md`).
- Operators & precedence (high to low):
  1. `^`: Right-associative; binds tighter than unary `-` so `-2 ^ 2` evaluates to `-4` and lowers to `=-(2^2)`.
  2. Unary `-`: Right-associative (unary `+` throws `FormulaParseError`).
  3. `*`, `/`: Left-associative.
  4. `+`, `-`: Left-associative.
  5. `=`, `<>`, `<`, `<=`, `>`, `>=`: Non-associative unless parenthesized (`(a = b) = c`); unparenthesized chained comparisons, `==`, `!=`, and `&` throw `FormulaParseError`. Type/empty rules live in `references/functions.md`.
- Slices & contiguity: Range endpoints (`start..end`) accept quoted keys, `@col`/`@row`, or `prev`/`next` offsets and must satisfy `start <= end` in declared order (inverted ranges throw `ReferenceResolutionError`). Row slices must span contiguous data rows (no intervening gutters or section headers); horizontal column slices must span contiguous `measure` columns (no `descriptor` or gutter columns), else `ReferenceResolutionError`. Vertical 1D slices on a single `descriptor` column (`Table["r1".."r2", "item"]`) are valid in `XLOOKUP`/`XMATCH`/`INDEX` and `AND`/`OR`, while numeric/financial aggregations reject `descriptor` columns (`F8Error`).
- Offsets: `col.prev`/`col.next` anchor the origin column ID in the target table and step across `measure` columns (skipping `descriptor` and gutter columns); `row.prev`/`row.next` anchor the origin row ID and step across data rows (skipping section headers and row gutters). Missing origin IDs, `descriptor`-anchored column offsets, or steps past bounds throw `ReferenceResolutionError`.
- Excel A1 lowering & `$` pinning:
  - Single cell: Static `Table["r", "c"]` lowers to `$B$5`; any dynamic axis (`@col`, `@row`, `prev`, `next`) lowers to relative `B5`. Cross-sheet references single-quote the sheet name (`'Sheet1'!$B$5`, escaping `'` as `''`).
  - Range slice: Static key endpoints pin their axis per endpoint (`Table["r", "c1"..@col]` -> `$C$5:E$5`; `Table[@row, "c1"..@col]` -> `$C5:E5`), except 1D horizontal static-column ranges on a dynamic row (`Table[@row, "c1".."c2"]` -> `C5:E5`).

## 2. Core Containers (`Model`, `Sheet`, `Table`, `compile`, `init`)

```ts
import { Cell, Model, Sheet, Table, compile, defaultTheme, init } from "f8-engine"

const model = new Model({
  name?: string, // default "Model"
  theme?: ThemeInput, // default "corporate_navy"
  print?: SheetPrintSpecInput,
})
const sheet: Sheet = model.addSheet(name: string, options?: {
  tabColor?: string
  showGridlines?: boolean // default false
  freezePanes?: FreezePanesInput // { row: number; col: number }
  layout?: SheetLayoutMode // "auto" (default) | "grid"
  colWidth?: number // finite 1..=255, valid only with layout: "grid"
  print?: SheetPrintOverrideInput | "none"
})
const table: Table = sheet.addTable(id: string, title: string, options?: {
  subtitle?: string | readonly string[]
  currency?: CurrencyCode
  collapsible?: boolean // default false
})
const compiled = compile(manifest: ProjectManifestInput, tables: readonly TableSpecInput[])
```

- WASM lifecycle: Node.js, Bun, and Deno auto-initialize WASM synchronously on import; browsers call `await init(moduleOrPath?)` once before `compile()`, `model.compile()`, or `defaultTheme()`.
- `Model`:
  - `addSheet(name, options?) -> Sheet`: Appends worksheet (duplicate `name`, invalid `freezePanes`, invalid `layout`, `colWidth` with `"auto"`, or `colWidth` outside `1..=255` throws `Error`).
  - `getSheet(name) -> Sheet` / `getTable(id) -> Table`: Lookup across model (throws `Error` if absent).
  - `sheets: readonly Sheet[]` / `tables: readonly Table[]`: Ordered containers across model.
  - `compile() -> CompiledModel`: Runs all compiler passes and returns inspection/emission facade.
  - `toSpec() -> ModelSpec`: Serializes to `{ manifest: ProjectManifestInput, tables: readonly TableSpecInput[] }`.
- `Sheet`:
  - `addTable(id, title, options?) -> Table`: Appends table (duplicate `id` across model throws `Error`).
  - `setBands(bands) -> this` / `bands: readonly BandSpecInput[]`: Explicit 2D band/lane placement (defaults to 1-table vertical bands in `addTable` order; see `references/layout-and-styling.md`).
  - `getTable(id) -> Table`: Lookup on sheet (throws `Error` if absent).
  - `tables: readonly Table[]`: Tables in `addTable` order (compiled outputs follow `bands -> lanes -> tables` order).
  - `toSpec() -> SheetSpecInput`: Serializes sheet (re-validates band partition if `addTable` followed `setBands`).
- `Table`:
  - `toSpec() -> TableSpecInput`: Serializes table; `table.subtitles: readonly string[]` stores normalized subtitle lines.
- Sheet options: `name` follows Excel sheet rules (`1..=31` UTF-16 code units, no `: \ / ? * [ ]`, no leading/trailing `'`, not `"History"`, case-insensitive unique; else `SpecificationError`). `tabColor` accepts `#RGB`/`#RRGGBB`/`#RRGGBBAA`; `freezePanes: { row, col }` uses 0-indexed integer coordinates. For `layout`, `colWidth`, `print`, `theme`, banners, and bands see `references/layout-and-styling.md`.

## 3. Columns and Rows (`addColumn`, `addRow`)

```ts
table.addColumn(id: string, options?: {
  role?: AxisRole // default "measure"
  header?: string
  headers?: readonly string[]
  group?: string
  style?: SemanticStyle
  highlight?: boolean
  shade?: Shade
  format?: FormatPreset
  decimals?: number
  currency?: CurrencyCode
  expr?: string
}): this

table.addRow(id: string, options?: {
  cells?: Readonly<Record<string, CellInputValue>>
  expr?: string
  style?: SemanticStyle
  highlight?: boolean
  shade?: Shade
  format?: FormatPreset
  decimals?: number
  currency?: CurrencyCode
}): this
```

- Column roles (`AxisRole`):
  - `"measure"` (default): Calculable column participating in `row.expr` broadcasting and `col.prev`/`col.next` stepping.
  - `"descriptor"`: Label/metadata column isolated from `row.expr`, `col.expr`, and row-level `format`/`decimals`/`currency`; requires explicit Tier-1 `cells` entries; rejected by numeric/financial aggregations (`XLOOKUP`/`XMATCH`/`INDEX` allow 1D vertical descriptor slices).
- Headers & `group`: Pass `header: "Title"` or stacked `headers: ["T1", "T2"]` (not both, else `Error`). `group: "Banner"` renders a Tier-0 `CenterAcross` banner across contiguous columns with the same non-empty `group` (excluded from single-column width sizing). Columns with fewer header lines bottom-align to the table's max header depth (`0` header rows when ungrouped and no column sets headers); missing column headers emit blank in `.xlsx` while `inspectSheet` and `TableAuditor.toMarkdown()` fall back to `col.id`.
- Vector formulas (`expr`): `col.expr` broadcasts down data rows in `measure` columns; `row.expr` broadcasts across `measure` columns. When `row.expr` uses `col.prev` on the first `measure` column, provide an explicit 0D override in `cells`.
- Validation: Duplicate `id` throws `Error`; empty `columns` or `rows` throws `SpecificationError`.

## 4. 0D Cell Overrides (`Cell`, `setCell`, `setCells`)

```ts
export interface DetailedCellInput {
  readonly value?: CellValueLiteral
  readonly expr?: string
  readonly style?: SemanticStyle
  readonly highlight?: boolean
  readonly shade?: Shade
  readonly format?: FormatPreset
  readonly decimals?: number
  readonly currency?: CurrencyCode
  readonly indent?: number
  readonly note?: string
}

export type CellValueLiteral = number | string | boolean | null
export type CellInputValue = CellValueLiteral | Cell | DetailedCellInput
export interface AxisSlice { readonly start: string; readonly stop: string }
export type AxisSelector = string | Iterable<string> | AxisSlice

const cell = new Cell(options: DetailedCellInput)
table.setCell(row: string, col: string, cell: CellInputValue): this
table.setCells(rows: AxisSelector, cols: AxisSelector, cell: CellInputValue): this
```

- Payload exclusivity: `new Cell(options)` and `DetailedCellInput` on new cells must set `value` XOR `expr` (`Error` at construction, `SpecificationError` at compile). `value: null` (or literal `null`) creates an explicit empty cell.
- Attribute constraints (`SpecificationError`):
  - `indent`: Integer `0..=255`; `> 0` valid only on `descriptor` cells (rejected on `measure`).
  - `currency`: Cannot pair with non-`"currency"` `format` on the same spec level; resolves to `null` when effective `format !== "currency"` or on `descriptor` columns.
  - `decimals`: `0..=14`; cannot pair with `"integer"`, `"date"`, or `"text"` on the same spec level; resolves to `null` when effective `format` is `"integer"`, `"date"`, or `"text"` (when effective `format` is `null`, explicit `decimals` still governs in-memory formatting).
  - Unknown column key in `row.cells` throws `SpecificationError`.
- Create vs. merge in `setCell`/`setCells`:
  - Existing 0D entry in `row.cells`: merges defined metadata while preserving existing `value`/`expr` unless a new payload is supplied.
  - New 0D override (including cells currently filled by `row.expr`/`col.expr`): must pass `value` (or scalar/`null`) or `expr`, else `Error`.
- `AxisSelector`: Accepts a single `string` ID, non-empty `Iterable<string>`, or inclusive `AxisSlice` (`{ start, stop }`) across declared data rows/columns (skipping gutters and section headers). Unknown IDs, inverted slices, or empty iterables throw `Error`. `setCells` validates all coordinates atomically before mutating.

```ts
table.setCells("amort", ["standalone", "adj"], null)
table.setCells(
  { start: "r1", stop: "r2" },
  { start: "c1", stop: "c2" },
  { expr: 'Table[@row, "base"] * Table["mult", @col]' }
)
table.setCell("r1", "c1", { highlight: true, note: "Patched existing 0D cell" })
```

## 5. 3-Tier Specificity Lattice

Resolution order per `(row, col)`: `Tier 1 (0D Cell) > Tier 2 (1D Row/Col) > Tier 3 (Default)`.

| Attribute   | Tier 1 (0D Cell)                                      | Tier 2 (`measure` Row vs. Col)                         | Descriptor Col                   | Tier 3 (Default)                                             |
| :---------- | :---------------------------------------------------- | :----------------------------------------------------- | :------------------------------- | :----------------------------------------------------------- |
| Payload     | `cell.value` / `cell.expr`                            | `row.expr` XOR `col.expr` (both -> error)              | Tier 1 required (else error)     | None (unassigned -> error)                                   |
| `format`    | `cell.format`                                         | `row.format` / `col.format` (diff -> error)            | `cell` else `col` (row isolated) | `null`                                                       |
| `decimals`  | `cell.decimals`                                       | `row.decimals` / `col.decimals` (diff -> error)        | `cell` else `col` (row isolated) | Theme default in formatting/emit (`null` in `cell.decimals`) |
| `currency`  | `cell.currency`                                       | `row.currency` / `col.currency` (diff -> error)        | `null` (ignored)                 | `table.currency` else `"usd"` (only on `"currency"`)         |
| `shade`     | `cell.shade` (`"none"` clears)                        | `row.shade` / `col.shade` (diff non-`"none"` -> error) | Same as `measure`                | `null`                                                       |
| `style`     | `"data"` resets both; others override horizontal axis | 2D pair `(h=row.style, v=col.style)`                   | Same as `measure`                | `("data", "data")`                                           |
| `highlight` | `cell.highlight` (`false` clears)                     | `Boolean(row.highlight \|\| col.highlight)`            | Same as `measure`                | `false`                                                      |
| `indent`    | `cell.indent`                                         | N/A                                                    | `cell.indent` else `0`           | `0` (`> 0` on `measure` -> error)                            |
| `note`      | `cell.note`                                           | N/A                                                    | `cell.note` else `null`          | `null`                                                       |

All lattice conflicts and unassigned cells throw `SpecificationError`.
