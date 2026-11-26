# Auditing and Diagnostics Reference

Model inspection via `CompiledModel` and `TableAuditor`, TypeScript inspection interfaces, tabular export (`toMarkdown`, `toDict`), and structured `F8Error` exception properties.

---

## 1. `CompiledModel` API

```ts
const compiled = model.compile()
```

### Methods

| Method                                                                       | Return type       | Behavior                                                                                                          |
| :--------------------------------------------------------------------------- | :---------------- | :---------------------------------------------------------------------------------------------------------------- |
| `compiled.summary()`                                                         | `ModelSummary`    | Counts for sheets, tables, cells, literal drivers, formulas, and per-sheet/per-table breakdowns                   |
| `compiled.errors()`                                                          | `CellError[]`     | Deterministic workbook-wide list of runtime Excel error cells in canonical `sheet -> band -> lane -> table` order |
| `compiled.toMarkdown(options?)` / `compiled.toMarkdown(detail, options?)`    | `string`          | Compact whole-workbook Markdown (`"values"` GFM tables, `"index"` coordinate map, or `"layout"` 2D sheet map)     |
| `compiled.table(table)`                                                      | `TableAuditor`    | High-level 2D table wrapper for coordinate lookup, row extraction, and Markdown/object export                     |
| `compiled.value(coordinate)` / `compiled.value(table, row, col)`             | `CellScalarValue` | Unboxed scalar (`number`, `string`, `boolean`, `null`, or Excel error string `"#DIV/0!"`, `"#VALUE!"`, etc.)      |
| `compiled.num(coordinate)` / `compiled.num(table, row, col)`                 | `number`          | Evaluated numeric `number` (throws `TypeError` if cell is text, `boolean`, `null`, or error)                      |
| `compiled.formatted(coordinate, opts?)` / `compiled.formatted(t, r, c, o?)`  | `string`          | Formatted display string with optional `opts?.unit` spliced inside trailing `)` (e.g. `"($115.0M)"`)              |
| `compiled.inspectCell(coordinate)` / `compiled.inspectCell(table, row, col)` | `CellInspection`  | Full cell record including evaluated/formatted values, formulas, formatting, and direct neighbors                 |
| `compiled.inspectTable(table)`                                               | `TableInspection` | Raw 2D table inspection object                                                                                    |
| `compiled.inspectSheet(sheet)`                                               | `SheetInspection` | Physical worksheet bounding box, column character widths, and row/cell layout records                             |
| `compiled.precedentsOf(coord, opts?)` / `compiled.precedentsOf(t, r, c, o?)` | `DependencyTree`  | Upstream precedent tree (1-hop when `opts?.recursive` is `false`/omitted, full transitive DAG when `true`)        |
| `compiled.dependentsOf(coord, opts?)` / `compiled.dependentsOf(t, r, c, o?)` | `DependencyTree`  | Downstream dependent tree (1-hop when `opts?.recursive` is `false`/omitted, full transitive DAG when `true`)      |
| `compiled.save(path, options?)`                                              | `void`            | Emits `.xlsx` workbook to `path` via `node:fs` (creates parent directories automatically)                         |
| `compiled.emitWorkbook(options?)`                                            | `Uint8Array`      | Emits `.xlsx` workbook as an in-memory `Uint8Array` byte buffer                                                   |
| `compiled.dispose()` / `compiled[Symbol.dispose]()`                          | `void`            | Deterministically frees WASM linear memory (idempotent; subsequent method calls throw `F8Error`)                  |

### Coordinate Addressing in `CompiledModel`

Cell methods (`value`, `num`, `formatted`, `inspectCell`, `precedentsOf`, `dependentsOf`) accept either a 1-argument coordinate string (canonical `'Table["row", "col"]'` with `\"` and `\\` escaping, shorthand `"Table['row', 'col']"` without `,`, `[`, `]`, `'`, `"`, or unquoted `'Table[row, col]'` additionally without `@`, `.`, `(`, `)`) or 3 positional arguments (`table, row, col`, which support arbitrary non-empty characters in `row` and `col`). Missing entities or malformed coordinates throw `InspectionError`:

```ts
const v1 = compiled.num('Operations["revenue", "2024E"]')
const v2 = compiled.num("Operations", "revenue", "2024E")
```

### Whole-Workbook Markdown (`compiled.toMarkdown`)

```ts
export interface ModelMarkdownOptions {
  readonly detail?: ModelMarkdownDetail; // "values" | "index" | "layout"
  readonly sheets?: readonly string[];
  readonly tables?: readonly string[];
  readonly showIds?: boolean;
}

const md: string = compiled.toMarkdown(options?: ModelMarkdownOptions);
const indexMd: string = compiled.toMarkdown(
  detail: ModelMarkdownDetail,
  options?: Omit<ModelMarkdownOptions, "detail">,
);
```

- `detail` modes (`ModelMarkdownDetail`; invalid string throws plain `Error`):
  - `"values"` (default): Emits compact unpadded GFM tables (`| a | b |`, `| :--- | ---: |`) with pre-formatted values, bolded `bold`/`subtotal`/`total` rows, row-max descriptor indentation, and `"—"` for empty measure cells.
  - `"index"`: Emits a compact coordinate catalog per table (`Columns:` with IDs, display headers, and `descriptor` roles; `Rows:` with section headers, nested row IDs, descriptor labels joined by `" | "`, and `(subtotal)`/`(total)` tags) with zero evaluated measure values (`showIds` is ignored).
  - `"layout"`: Emits a 2D worksheet geometry map per sheet rendered from authored `Band` -> `Lane` -> `Table` placement (`showIds` is ignored):
    - Sheet header: `Layout:` mode (`auto` or `grid (W=...)`, adding `, base=...` only when `W` was widened above `base_w`), bounding box & total visual width, optional `Freeze` / `Gridlines: on` / `Tab`, run-length-encoded `Columns:` widths, and (when `print` is configured on a non-empty sheet) a `Print:` boundary summary line (`paper orientation | Fit: ... | Area: B2:... | Est. scale: ~..%`, reporting `Pages: P1 {rows}r (~{pg}pg[!, wrap:{n}])` with `!` above `0.95pg` on `fit_width`, `(h:~..%, v:~..%)` plus optional `| wrap:{n}` on `fit_page`, and trailing `[low scale <60%]` below `60%`).
    - Band / lane tree: 1-indexed `Band` entries with inter-band `gap:Nr`, `right_empty` columns, non-default `table_gap:Nr` (on multi-table single-lane bands), and `, page_break` on bands with `pageBreakBefore: true`; multi-lane bands nest `Lane` entries with inter-lane `gap:Nc`, `empty_below` rows, and non-default `table_gap:Nr`.
    - Per-table line: bounds & width, optional `untitled` or title overhang `title->{col} (+{n} empty)`, `sub:{n}` (appending `({max_chars}ch>w)` when `round(max_chars * char_width_multiplier, 2)` exceeds `w`), `hdr:{range}(grouped)`/`hdr:none`, `body:{range}`, `collapsible`, physical `cols:` role runs showing `req:{w}` required width on multi-column `grid` descriptors and `w:{gutter_w}` on widened gutters, and optional `wrap:{n}` counts.
- Filtering and annotations:
  - `sheets` / `tables`: Strict union allowlist (unknown sheet or table throws `InspectionError`); preserves canonical `sheet -> band -> lane -> table` order without duplicates. Explicit empty allowlists (`sheets: []` with `tables` omitted, or vice versa, or both empty) emit only the model header; empty selected sheets emit `_(no tables)_`. In `"layout"` mode, partial `tables` filtering on a sheet not matched by `sheets` renders that sheet's full 2D geometry and tags unselected sibling tables with `(context)` while keeping `## Runtime Errors` scoped to selected tables.
  - `showIds: true` (default): Appends `[colId]` / `[rowId]` when display text differs case-insensitively from IDs, prepends a left-aligned `Row` column on descriptor-less tables, and annotates `[rowId]` only on the first descriptor column of multi-descriptor tables (`"values"` mode only; ignored in `"index"` and `"layout"` modes).
  - Header & errors: Line 2 reports full-model counts including total `Errors: N`; `## Runtime Errors` lists errors belonging to selected tables (omitted when selected scope has 0 errors).

### Workbook Emission Options (`EmissionOptions`)

Optional object passed to `compiled.save(path, options)` or `compiled.emitWorkbook(options)`:

- `valuesOnly?: boolean` (default `false`): Writes evaluated literal values instead of Excel formulas.
- `omitFormulaCache?: boolean` (default `false`): Emits formulas without cached `<v>` result tags.
- `title?: string`: Sets OpenXML document title property.
- `author?: string`: Sets OpenXML document author property (defaults to `"f8 Compiler"`).

---

## 2. `TableAuditor` API

```ts
const auditor = compiled.table("Operations")
```

### Properties and Methods

| Member                               | Return type         | Behavior                                                                                         |
| :----------------------------------- | :------------------ | :----------------------------------------------------------------------------------------------- |
| `auditor.id`                         | `string`            | Table programmatic identifier                                                                    |
| `auditor.title`                      | `string`            | Table display title                                                                              |
| `auditor.subtitles`                  | `string[]`          | Declared table subtitle lines (`[]` when omitted)                                                |
| `auditor.sheet`                      | `string`            | Parent worksheet name                                                                            |
| `auditor.rows`                       | `string[]`          | Non-gutter, non-section-header data row IDs in declaration order                                 |
| `auditor.columns`                    | `string[]`          | Non-gutter data column IDs (`descriptor` and `measure`) in declaration order                     |
| `auditor.measureColumns`             | `string[]`          | Non-gutter `measure` column IDs in declaration order                                             |
| `auditor.shape`                      | `[number, number]`  | `[auditor.rows.length, auditor.columns.length]`                                                  |
| `auditor.length`                     | `number`            | `auditor.rows.length`                                                                            |
| `auditor.value(row, col)`            | `CellScalarValue`   | Unboxed scalar (`number`, `string`, `boolean`, `null`, or Excel error string; absent -> `Error`) |
| `auditor.num(row, col)`              | `number`            | Evaluated numeric `number` (absent -> `Error`; non-numeric/bool/empty/error -> `TypeError`)      |
| `auditor.formatted(row, col, opts?)` | `string`            | Formatted display string with optional `opts?.unit` spliced inside trailing `)`                  |
| `auditor.rowValues(row, cols?)`      | `CellScalarValue[]` | 1D array of `value(row, c)` across `measureColumns` (or explicit `cols`)                         |
| `auditor.rowNums(row, cols?)`        | `number[]`          | 1D array of `num(row, c)` across `measureColumns` (or explicit `cols`)                           |
| `auditor.formula(row, col)`          | `string \| null`    | Lowered Excel formula string (e.g. `"=C5*1.1"`), or `null` if literal driver                     |
| `auditor.expr(row, col)`             | `string \| null`    | Source `f8` expression string, or `null` if literal driver                                       |
| `auditor.address(row, col)`          | `string`            | Worksheet-local A1 cell address (e.g. `"C6"`)                                                    |
| `auditor.cell(row, col)`             | `TableCellInfo`     | Raw cell inspection object (throws `Error` if absent)                                            |

### Unboxing Raw `CellValueDict` Payloads

`compiled.value(...)` and `auditor.value(...)` return unboxed JavaScript scalars directly. When reading raw objects from `inspectCell`, `inspectTable`, `inspectSheet`, `precedentsOf`, or `dependentsOf`, use `unboxCellValue(val)` to convert tagged `CellValueDict` payloads (`{ Number: 10.0 }`, `{ Text: "s" }`, `{ Boolean: true }`, `"Empty"`, `{ Error: "Div0" | "Value" | "Ref" | "Num" | "Na" | "Name" }`) into `number | string | boolean | null`.

---

## 3. Tabular Export (`toMarkdown`, `toDict`)

### GitHub-Flavored Markdown (`auditor.toMarkdown`)

```ts
export interface TableMarkdownOptions {
  readonly view?: TableViewMode; // "values" | "formulas" | "expr" | "addresses"
  readonly rows?: readonly string[];
  readonly cols?: readonly string[];
  readonly includeSectionHeaders?: boolean;
  readonly showIds?: boolean;
  readonly compact?: boolean;
}

const md: string = auditor.toMarkdown(options?: TableMarkdownOptions);
const mdView: string = auditor.toMarkdown(
  view: TableViewMode,
  options?: Omit<TableMarkdownOptions, "view">,
);
```

- `view` modes (`TableViewMode`; unrecognized value throws `Error`; `descriptor` columns display their indented text label across all modes, falling back to `rowId` when empty, while `measure` columns switch by `view`):
  - `"values"` (default): Pre-formatted strings (`"—"` for empty `measure` cells).
  - `"formulas"`: Lowered Excel formulas (`"=C5*1.15"`) for formula cells, formatted values for literals (`""` if empty).
  - `"expr"`: Source `f8` expressions for formula cells, formatted values for literals (`""` if empty).
  - `"addresses"`: Physical A1 addresses (`"D7"`).
- Subsetting and formatting:
  - `cols`: Any `descriptor` columns not explicitly listed in `cols` are prepended in declaration order ahead of the requested `cols` (which preserve caller order; unknown ID throws `Error`). `descriptor` columns are left-aligned (`:---`); `measure` columns are right-aligned (`---:`).
  - `rows`: Renders only requested `rows` (unknown ID throws `Error`). `includeSectionHeaders` defaults to `true` when `rows` is omitted and `false` when `rows` is specified; when `rows` is specified with `includeSectionHeaders: true`, matching rows emit in table declaration order with only section headers that have at least one matching child row.
  - `showIds: true`: Appends `[{colId}]` to column headers and `[{rowId}]` to descriptor labels when display text differs (case-insensitively) from the ID.
  - `compact: true`: Emits unpadded GFM rows (`| a | b |`) and minimal alignment dividers (`| :--- | ---: |`) without column-width padding (default `false` pads columns).
  - Rows with `style: "bold"`, `style: "subtotal"`, or `style: "total"` wrap non-empty cell text in `**...**`. Line breaks (`\r\n`, `\r`, `\n`) are normalized to spaces and `|` delimiters are escaped as `\|`.

### Object and Record Export (`auditor.toDict`)

```ts
const data = auditor.toDict(orient: TableOrient = "records");
```

| `orient`              | Return type                                       | Structure (shape `auditor.rows.length` x `auditor.columns.length`, omitting section headers/gutters) |
| :-------------------- | :------------------------------------------------ | :--------------------------------------------------------------------------------------------------- |
| `"records"` (default) | `Record<string, CellScalarValue>[]`               | Row array of `{ [colId]: scalarValue }` objects                                                      |
| `"dict"`              | `Record<string, CellScalarValue[]>`               | `{ [colId]: [rowOrderedValues] }`                                                                    |
| `"index"`             | `Record<string, Record<string, CellScalarValue>>` | `{ [rowId]: { [colId]: scalarValue } }`                                                              |
| `"split"`             | `SplitDict`                                       | `{ columns: string[], index: string[], data: CellScalarValue[][] }`                                  |

---

## 4. Inspection Interfaces (`f8-engine`)

All inspection interfaces and union types are re-exported from `"f8-engine"`:

- `ModelSummary`: `modelName: string`, `themeName: string`, `sheets: readonly SheetSummary[]`, `totalSheets: number`, `totalTables: number`, `totalCells: number`, `totalDrivers: number`, `totalFormulas: number`.
  - `SheetSummary`: `name: string`, `tables: readonly TableSummary[]`, `totalCells: number`.
  - `TableSummary`: `id: string`, `title: string`, `rowCount: number` (includes section headers and row gutters), `colCount: number` (includes column gutters), `cellCount: number` (`dataRows * dataCols`), `driverCount: number`, `formulaCount: number`.
- `CellError`: `coordinate: CellCoordinate`, `sheet: string`, `physicalAddress: string` (`"'Sheet'!C14"`), `error: ExcelErrorCode` (`"#DIV/0!" | "#VALUE!" | "#REF!" | "#NUM!" | "#N/A" | "#NAME?"`), `formulaExpr: string | null`, `excelFormula: string | null`.
- `ModelMarkdownDetail`: `"values" | "index" | "layout"`.
- `ModelMarkdownOptions`: `detail?: ModelMarkdownDetail`, `sheets?: readonly string[]`, `tables?: readonly string[]`, `showIds?: boolean`.
- `CellInspection`: `coordinate: CellCoordinate` (`{ table: string, row: string, col: string }`), `sheet: string`, `physicalAddress: string` (`"'Sheet'!C14"`), `evaluatedValue: CellValueDict`, `formattedValue: string | null`, `format: FormatPreset | null`, `decimals: number | null`, `currency: CurrencyCode | null`, `isHighlight: boolean`, `shade: Shade | null`, `indent: number`, `isFormula: boolean`, `formulaExpr: string | null`, `excelFormula: string | null`, `note: string | null`, `precedents: readonly CellNeighborInspection[]`, `dependents: readonly CellNeighborInspection[]`.
  - `CellNeighborInspection`: `cellId: CellCoordinate`, `sheetAddress: string`, `value: CellValueDict | null`, `formattedValue: string | null`.
- `DependencyTree`: `root: CellCoordinate`, `directCount: number`, `transitiveCount: number`, `nodes: readonly DependencyNode[]`.
  - `DependencyNode`: `cellId: CellCoordinate`, `sheetAddress: string`, `value: CellValueDict | null`, `formattedValue: string | null`, `excelFormula: string | null`, `isPruned: boolean` (marks already-expanded subtrees when `recursive: true`), `children: readonly DependencyNode[]`.
- `TableInspection`: `tableId: string`, `title: string`, `subtitles: readonly string[]`, `sheet: string`, `columns: readonly ColumnInspection[]`, `rows: readonly RowInspection[]`, `cells: Readonly<Record<string, TableCellInfo>>` (keyed by `'Table["row", "col"]'`).
  - `ColumnInspection`: `id: string | null`, `group: string | null`, `headers: readonly string[]`, `isGutter: boolean`, `isDescriptor: boolean`, `style: SemanticStyle | null` (defaults to `"data"` on data columns; `null` on gutters), `shade: Shade | null`.
  - `RowInspection`: `id: string | null`, `sectionTitle: string | null`, `isSectionHeader: boolean`, `isGutter: boolean`, `indent: number` (for data rows, max `cell.indent` across all cells in that row; for section headers, the `indent` passed to `addSectionHeader` or `0`; `toMarkdown()` indents all descriptor columns on data rows by `'  '.repeat(indent)`), `style: SemanticStyle | null` (defaults to `"data"` on data rows; `null` on gutters/section headers), `shade: Shade | null`.
  - `TableCellInfo`: `row: string`, `col: string`, `value: CellValueDict`, `formattedValue: string | null`, `isFormula: boolean`, `formulaExpr: string | null`, `excelFormula: string | null`, `physicalA1: string` (`"C14"` without sheet prefix), `format: FormatPreset | null`, `decimals: number | null`, `currency: CurrencyCode | null`.
- `SheetInspection`: `sheetId: string`, `title: string`, `tables: readonly string[]`, `minCol: number`, `maxCol: number`, `minRow: number`, `maxRow: number`, `colWidths: Readonly<Record<number, number>>` (0-indexed column -> character width), `rows: readonly SheetRowInspection[]`.
  - `SheetRowInspection`: `physicalRow: number` (0-indexed), `excelRow: number` (1-indexed), `cells: readonly SheetCellInspection[]`.
  - `SheetCellInspection`: `physicalCol: number`, `physicalA1: string`, `tableId: string | null`, `rowId: string | null`, `colId: string | null`, `kind: SheetCellKind` (`"empty" | "table_title" | "table_subtitle" | "column_header" | "section_header" | "gutter" | "data" | "overflow"`), `isDescriptor: boolean` (`true` when `kind` is `"data"` or `"overflow"` on a `descriptor` column; `"overflow"` marks spanned descriptor runway cells with `value: "Empty"` and owning `tableId`/`rowId`/`colId`, while `"empty"` marks unowned canvas), `value: CellValueDict`, `formattedValue: string | null`, `formulaExpr: string | null`, `excelFormula: string | null`.

---

## 5. Structured Exception Hierarchy (`F8Error`)

All compiler, evaluation, inspection, and emission errors inherit from `F8Error extends Error` (exported from `"f8-engine"`) and expose typed properties:

| Exception                  | Properties                                                                                                      | Trigger conditions                                                                                                                                                                                                                                                                                                                                                                                         |
| :------------------------- | :-------------------------------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `F8Error`                  | `message: string`                                                                                               | Base error class; thrown directly on formula validation, evaluation, and lowering failures (unknown function, arity or `_` omission violation, scalar/range kind or shape mismatch, `descriptor` reference in numeric/financial aggregations, static type or branch-unification mismatch, disallowed empty operand/argument/result), WASM sync init failure, or method calls on a disposed `CompiledModel` |
| `CycleDetectedError`       | `cycle: readonly string[]`, `message: string`                                                                   | Circular dependency in DAG; `cycle` contains ordered coordinate strings forming the loop (`['T["a", "c1"]', 'T["b", "c1"]', 'T["a", "c1"]']`)                                                                                                                                                                                                                                                              |
| `ReferenceResolutionError` | `coordinate: string`, `targetTable: string`, `row: string \| null`, `col: string \| null`, `message: string`    | Unresolved table/row/col key, active axis mismatch, inverted or discontinuous slice, or out-of-bounds `prev`/`next` offset from origin `coordinate`                                                                                                                                                                                                                                                        |
| `FormulaParseError`        | `table: string`, `row: string \| null`, `col: string \| null`, `tokenOffset: number \| null`, `message: string` | Lexical or grammatical syntax error in formula (`tokenOffset` is byte offset in expression string, or `null` on unexpected EOF)                                                                                                                                                                                                                                                                            |
| `SpecificationError`       | `table: string \| null`, `reason: string`, `message: string`                                                    | Schema, 3-tier lattice, formatting, theme, or spatial layout violation (`table` is `null` for manifest/theme/deserialization errors)                                                                                                                                                                                                                                                                       |
| `InspectionError`          | `entity: string`, `message: string`                                                                             | Unknown sheet, table, or cell coordinate (or malformed coordinate string) passed to `CompiledModel` inspection methods                                                                                                                                                                                                                                                                                     |
| `EmissionError`            | `message: string`                                                                                               | OpenXML serialization error or file I/O failure during `save`/`emitWorkbook`                                                                                                                                                                                                                                                                                                                               |

### `ReferenceResolutionError` Property Matrix

| Failure condition     | `targetTable` | `row`    | `col`            | Cause                                                                                                                                                              |
| :-------------------- | :------------ | :------- | :--------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Unknown table         | `string`      | `null`   | `null`           | Referenced table ID does not exist in model                                                                                                                        |
| Inverted slice        | `string`      | `null`   | `null`           | Slice `start..end` resolves to `start > end` in `targetTable`                                                                                                      |
| Discontinuous slice   | `string`      | `null`   | `string \| null` | Slice `start..end` spans a row/column gutter, section header (`col = null`), or `descriptor` column (`col = string`)                                               |
| Unknown row           | `string`      | `string` | `null`           | Explicit row key does not exist in `targetTable`                                                                                                                   |
| Missing active row    | `string`      | `string` | `null`           | `@row` or `row.prev`/`row.next` evaluated at an origin row ID absent from `targetTable`                                                                            |
| Unknown column        | `string`      | `null`   | `string`         | Explicit column key does not exist in `targetTable`                                                                                                                |
| Missing active column | `string`      | `null`   | `string`         | `@col` or `col.prev`/`col.next` evaluated at an origin column ID absent from `targetTable`                                                                         |
| Out-of-bounds offset  | `string`      | `null`   | `null`           | `col.prev`/`col.next` or `row.prev`/`row.next` steps past `targetTable` axis bounds (or `col.prev`/`col.next` evaluated at a `descriptor` column in `targetTable`) |
