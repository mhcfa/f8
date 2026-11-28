# Auditing and Diagnostics Reference

Model inspection via `CompiledModel` and `TableAuditor`, Markdown/object exports, `f8-engine` types and quirks, and the structured `F8Error` hierarchy.

## 1. `CompiledModel` API

```ts
const compiled = model.compile()
```

- `summary(): ModelSummary`: Model-, sheet-, and table-level cell, driver, and formula counts.
- `errors(): CellError[]`: Deterministic runtime Excel error list in `sheet -> band -> lane -> table` order.
- `toMarkdown(options?: ModelMarkdownOptions): string` / `toMarkdown(detail: ModelMarkdownDetail, options?: Omit<ModelMarkdownOptions, "detail">): string`: Whole-workbook GFM overview (see below).
- `table(table: string): TableAuditor`: High-level 2D table wrapper for lookups and exports.
- `value(coord) / value(table, row, col): CellScalarValue`: Unboxed scalar (`number | string | boolean | null`, or Excel error string `"#DIV/0!"`, etc.).
- `num(coord) / num(table, row, col): number`: Numeric `number` (`TypeError` if text, `boolean`, `null`, or error).
- `formatted(coord, opts?) / formatted(table, row, col, opts?): string`: Formatted display string with optional `opts?.unit` spliced inside trailing `)` (e.g. `"($115.0M)"`).
- `inspectCell(coord) / inspectCell(table, row, col): CellInspection`: Full cell record (values, formulas, formatting, direct precedents/dependents).
- `inspectTable(table: string): TableInspection`: Raw 2D table inspection object.
- `inspectSheet(sheet: string): SheetInspection`: Physical worksheet grid, bounding box, and column widths.
- `precedentsOf(coord, opts?) / precedentsOf(table, row, col, opts?): DependencyTree`: Upstream precedent tree (1-hop or transitive when `opts?.recursive` is `true`).
- `dependentsOf(coord, opts?) / dependentsOf(table, row, col, opts?): DependencyTree`: Downstream dependent tree (1-hop or transitive when `opts?.recursive` is `true`).
- `save(path: string, options?: EmissionOptions): void`: Writes `.xlsx` via `node:fs` (creates parent dirs).
- `emitWorkbook(options?: EmissionOptions): Uint8Array`: Emits `.xlsx` as in-memory `Uint8Array`.
- `dispose(): void` / `[Symbol.dispose](): void`: Idempotently frees WASM linear memory (subsequent calls throw `F8Error`).

### Coordinate Addressing

Cell methods (`value`, `num`, `formatted`, `inspectCell`, `precedentsOf`, `dependentsOf`) accept either a 1-arg string (`'Table["row", "col"]'` with `\"`/`\\` escapes, shorthand `"Table['row', 'col']"`, or unquoted `'Table[row, col]'` without reserved delimiters) or 3 positional args `(table, row, col)` supporting arbitrary non-empty IDs. Missing entities or malformed strings throw `InspectionError`.

### Whole-Workbook Markdown (`compiled.toMarkdown`)

`ModelMarkdownOptions`: `{ detail?: "values" | "index" | "layout", sheets?: readonly string[], tables?: readonly string[], showIds?: boolean }`.

- `detail` modes (`ModelMarkdownDetail`; invalid value throws `Error`):
  - `"values"` (default): Compact unpadded GFM tables with formatted values, bold summary rows, and `"—"` for empty measure cells.
  - `"index"`: Coordinate catalog per table (`Columns:` with roles/headers; `Rows:` with section headers, IDs, descriptor labels, and `(subtotal)`/`(total)` tags), zero measure values (`showIds` ignored).
  - `"layout"`: 2D worksheet geometry map rendered from authored `BandInput` -> `LaneInput` -> `Table` placement (`showIds` ignored). Non-obvious diagnostic flags: `!` on `fit_width` page fills `>0.95pg`, `[low scale <60%]` on heavily scaled prints, `sub:{n}({chars}ch>w)` subtitle width overflow, `title->{col} (+{n} empty)` title banner overhang, `req:{w}` demand on multi-col `grid` descriptors, `wrap:{n}` wrapped-cell counts, and `(context)` on unselected sibling tables under partial `tables` filtering.
- Filtering & annotations: `sheets`/`tables` form a strict union allowlist (`InspectionError` on unknown; empty allowlist emits header only; empty sheet emits `_(no tables)_`). `showIds: true` (default, `"values"` only) appends `[id]` when display text differs case-insensitively and prepends a `Row` column on descriptor-less tables. `## Runtime Errors` scopes to selected tables (omitted when 0).

### `EmissionOptions`

Optional object passed to `save` / `emitWorkbook`: `valuesOnly?: boolean` (default `false`, literals instead of formulas), `omitFormulaCache?: boolean` (default `false`, omit `<v>` tags), `title?: string`, `author?: string` (default `"f8 Compiler"`).

## 2. `TableAuditor` API and Exports

```ts
const auditor = compiled.table("Operations")
```

- Metadata properties: `id: string`, `title: string`, `subtitles: string[]`, `sheet: string`, `rows: string[]` (data rows only), `columns: string[]` (non-gutter), `measureColumns: string[]`, `shape: [number, number]`, `length: number`.
- Lookups (throws `Error` if absent):
  - `value(row, col): CellScalarValue` / `num(row, col): number` (`TypeError` if non-numeric).
  - `formatted(row, col, opts?: FormattedOptions): string`: Formatted display string with optional `opts?.unit`.
  - `rowValues(row, cols?: readonly string[]): CellScalarValue[]` / `rowNums(row, cols?: readonly string[]): number[]`: Defaults to `measureColumns`.
  - `formula(row, col): string | null` (lowered Excel) / `expr(row, col): string | null` (`f8` source) / `address(row, col): string` (sheet-local A1) / `cell(row, col): TableCellInfo`.

### Table Markdown (`auditor.toMarkdown`)

```ts
export interface TableMarkdownOptions {
  readonly view?: TableViewMode // "values" (default) | "formulas" | "expr" | "addresses"
  readonly rows?: readonly string[]
  readonly cols?: readonly string[]
  readonly includeSectionHeaders?: boolean
  readonly showIds?: boolean // default false
  readonly compact?: boolean // default false
}
const md = auditor.toMarkdown(options?: TableMarkdownOptions)
const mdView = auditor.toMarkdown(view: TableViewMode, options?: Omit<TableMarkdownOptions, "view">)
```

- `view` (`TableViewMode`; invalid throws `Error`; `descriptor` columns always show indented labels falling back to `rowId`): `"values"` (`"—"` for empty), `"formulas"` (lowered Excel or literal), `"expr"` (`f8` source or literal), `"addresses"` (A1).
- Subsetting & formatting: `cols` auto-prepends unlisted `descriptor` columns in declaration order (`Error` on unknown). `includeSectionHeaders` defaults to `true` when `rows` is omitted and `false` when `rows` is passed (`true` with `rows` emits matching rows in declaration order with non-empty section headers). `showIds: true` appends `[id]` when differing; `compact: true` disables column padding. Summary rows (`bold`/`subtotal`/`total`) are bolded.

### Object Export (`auditor.toDict`)

`auditor.toDict(orient: TableOrient = "records")` (data rows x data cols, excluding section headers/gutters):

| `orient`              | Return type                                       | Structure                          |
| :-------------------- | :------------------------------------------------ | :--------------------------------- |
| `"records"` (default) | `Record<string, CellScalarValue>[]`               | `[{ [colId]: scalar }, ...]`       |
| `"dict"`              | `Record<string, CellScalarValue[]>`               | `{ [colId]: [rowValues] }`         |
| `"index"`             | `Record<string, Record<string, CellScalarValue>>` | `{ [rowId]: { [colId]: scalar } }` |
| `"split"`             | `SplitDict`                                       | `{ columns, index, data }`         |

## 3. Inspection Types (`f8-engine`)

All inspection interfaces and unions (`ModelSummary`, `SheetSummary`, `TableSummary`, `CellError`, `CellInspection`, `CellNeighborInspection`, `DependencyTree`, `DependencyNode`, `TableInspection`, `ColumnInspection`, `RowInspection`, `TableCellInfo`, `SheetInspection`, `SheetRowInspection`, `SheetCellInspection`, `SplitDict`, `CellCoordinate`, `ExcelErrorCode`, `SheetCellKind`) are exported from `"f8-engine"`. Non-obvious quirks:

- `CellValueDict` tagged union: Raw inspection objects (`inspectCell`, `inspectTable`, `inspectSheet`, `precedentsOf`, `dependentsOf`) return Rust-tagged payloads (`{ Number: 10.0 }`, `{ Text: "s" }`, `{ Boolean: true }`, `"Empty"`, `{ Error: "Div0" | "Value" | "Ref" | "Num" | "Na" | "Name" }`). Call `unboxCellValue(val)` to convert to `CellScalarValue` (`compiled.value(...)` and `auditor.value(...)` unbox automatically).
- `TableSummary` counting: `rowCount` includes section headers and row gutters, and `colCount` includes column gutters, whereas `cellCount` counts only calculable cells (`dataRows * dataCols`).
- `SheetCellKind` `"overflow"` vs. `"empty"`: In `inspectSheet`, `"overflow"` marks spanned `grid`-mode descriptor runway cells (`isDescriptor: true`, `value: "Empty"`, owning `tableId`/`rowId`/`colId`), while `"empty"` marks unowned sheet canvas.
- Keys & addresses: `TableInspection.cells` is keyed by `'Table["row", "col"]'`; `TableCellInfo.physicalA1` omits the sheet prefix (`"C14"`) whereas `CellInspection.physicalAddress` includes it (`"'Sheet'!C14"`). `DependencyNode.isPruned` flags already-expanded subtrees when `recursive: true`.

## 4. Structured Exception Hierarchy (`F8Error`)

All compiler, evaluation, inspection, and emission errors inherit from `F8Error extends Error` (exported from `"f8-engine"`) with typed properties:

- `F8Error` (`message: string`): Base class; thrown directly on formula validation, evaluation, and lowering errors (unknown function, arity/`_` omission, kind/shape/role/type/branch-unification mismatch, disallowed empty), WASM sync init failure, or calls on a disposed `CompiledModel`.
- `CycleDetectedError` (`cycle: readonly string[]`, `message: string`): Circular DAG dependency; `cycle` holds ordered coordinate loop (`['T["a", "c1"]', ...]`).
- `ReferenceResolutionError` (`coordinate: string`, `targetTable: string`, `row: string | null`, `col: string | null`, `message: string`): Unresolved table/row/col key, active axis mismatch, inverted/discontinuous slice, or out-of-bounds `prev`/`next` offset.
- `FormulaParseError` (`table: string`, `row: string | null`, `col: string | null`, `tokenOffset: number | null`, `message: string`): Lexical or syntax error (`tokenOffset` is byte offset, or `null` on EOF).
- `SpecificationError` (`table: string | null`, `reason: string`, `message: string`): Schema, 3-tier lattice, formatting, theme, or spatial layout violation (`table: null` for manifest/theme/deserialization errors).
- `InspectionError` (`entity: string`, `message: string`): Unknown sheet/table/cell or malformed coordinate in `CompiledModel` inspection.
- `EmissionError` (`message: string`): OpenXML serialization or file I/O failure in `save` / `emitWorkbook`.
