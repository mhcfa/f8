# Auditing and Diagnostics Reference

Model inspection via `CompiledModel` and `TableAuditor`, Markdown/dict exports, `f8.types` pointers and quirks, and the structured `F8Error` hierarchy.

## 1. `CompiledModel` API

```python
compiled = model.compile()
```

- `summary() -> ModelSummary`: Model-, sheet-, and table-level cell, driver, and formula counts.
- `errors() -> list[CellError]`: Deterministic runtime Excel error list in `sheet -> band -> lane -> table` order.
- `to_markdown(detail: ModelMarkdownDetail = "values", *, sheets: Sequence[str] | None = None, tables: Sequence[str] | None = None, show_ids: bool = True) -> str`: Whole-workbook GFM overview (see below).
- `table(table: str) -> TableAuditor`: High-level 2D table wrapper for lookups and exports.
- `value(target: str, row: str | None = None, col: str | None = None) -> CellScalarValue`: Unboxed scalar (`float | str | bool | None`, or Excel error string `"#DIV/0!"`, etc.).
- `num(target: str, row: str | None = None, col: str | None = None) -> float`: Numeric `float` (`TypeError` if text, `bool`, `None`, or error).
- `formatted(target: str, row: str | None = None, col: str | None = None, *, unit: str = "") -> str`: Formatted display string with optional `unit` spliced inside trailing `)` (e.g. `"($115.0M)"`).
- `inspect_cell(target: str, row: str | None = None, col: str | None = None) -> CellInspection`: Full cell record (values, formulas, formatting, direct precedents/dependents).
- `inspect_table(table: str) -> TableInspection`: Raw 2D table inspection dict.
- `inspect_sheet(sheet: str) -> SheetInspection`: Physical worksheet grid, bounding box, and column widths.
- `precedents_of(target: str, row: str | None = None, col: str | None = None, recursive: bool = False) -> DependencyTree`: Upstream precedent tree (1-hop or full transitive DAG).
- `dependents_of(target: str, row: str | None = None, col: str | None = None, recursive: bool = False) -> DependencyTree`: Downstream dependent tree (1-hop or full transitive DAG).
- `save(path: str | Path, options: EmissionOptions | None = None) -> None`: Writes `.xlsx` to `path` (creates parent dirs).
- `emit_workbook(options: EmissionOptions | None = None) -> bytes`: Emits `.xlsx` as in-memory `bytes`.

### Coordinate Addressing

Cell methods (`value`, `num`, `formatted`, `inspect_cell`, `precedents_of`, `dependents_of`) accept either a 1-arg string (`'Table["row", "col"]'` with `\"`/`\\` escapes, shorthand `"Table['row', 'col']"`, or unquoted `'Table[row, col]'` without reserved delimiters) or 3 positional args `(table, row, col)` supporting arbitrary non-empty IDs. Missing entities or malformed strings raise `InspectionError`.

### Whole-Workbook Markdown (`compiled.to_markdown`)

- `detail` modes (`ModelMarkdownDetail`; invalid value raises `ValueError`):
  - `"values"` (default): Compact unpadded GFM tables with formatted values, bold summary rows, and `"—"` for empty measure cells.
  - `"index"`: Coordinate catalog per table (`Columns:` with roles/headers; `Rows:` with section headers, IDs, descriptor labels, and `(subtotal)`/`(total)` tags), zero measure values (`show_ids` ignored).
  - `"layout"`: 2D worksheet geometry map rendered from authored `Band` -> `Lane` -> `Table` placement (`show_ids` ignored). Non-obvious diagnostic flags: `!` on `fit_width` page fills `>0.95pg`, `[low scale <60%]` on heavily scaled prints, `sub:{n}({chars}ch>w)` subtitle width overflow, `title->{col} (+{n} empty)` title banner overhang, `req:{w}` demand on multi-col `grid` descriptors, `wrap:{n}` wrapped-cell counts, and `(context)` on unselected sibling tables under partial `tables` filtering.
- Filtering & annotations: `sheets`/`tables` form a strict union allowlist (`InspectionError` on unknown; empty allowlist emits header only; empty sheet emits `_(no tables)_`). `show_ids=True` (`"values"` only) appends `[id]` when display text differs case-insensitively and prepends a `Row` column on descriptor-less tables. `## Runtime Errors` scopes to selected tables (omitted when 0).

### `EmissionOptions`

Optional dict passed to `save` / `emit_workbook`: `values_only: bool = False` (literals instead of formulas), `omit_formula_cache: bool = False` (omit `<v>` tags), `title: str`, `author: str` (default `"f8 Compiler"`).

## 2. `TableAuditor` API and Exports

```python
auditor = compiled.table("Operations")
```

- Metadata properties: `id: str`, `title: str`, `subtitles: list[str]`, `sheet: str`, `rows: list[str]` (data rows only), `columns: list[str]` (non-gutter), `measure_columns: list[str]`, `shape: tuple[int, int]`, `len(auditor) -> int`.
- Lookups (`KeyError` if absent):
  - `auditor[row, col] -> CellScalarValue`: Alias for `value(row, col)` (non-2-tuple or non-`str` raises `TypeError`).
  - `value(row, col) -> CellScalarValue` / `num(row, col) -> float` (`TypeError` if non-numeric).
  - `formatted(row, col, *, unit: str = "") -> str`: Formatted display string with optional `unit`.
  - `row_values(row, cols: Sequence[str] | None = None) -> list[CellScalarValue]` / `row_nums(row, cols: Sequence[str] | None = None) -> list[float]`: Defaults to `measure_columns`.
  - `formula(row, col) -> str | None` (lowered Excel) / `expr(row, col) -> str | None` (`f8` source) / `address(row, col) -> str` (sheet-local A1) / `cell(row, col) -> TableCellInfo`.

### Table Markdown (`auditor.to_markdown`)

```python
md: MarkdownTable = auditor.to_markdown(
    view: TableViewMode = "values",
    *,
    rows: Sequence[str] | None = None,
    cols: Sequence[str] | None = None,
    include_section_headers: bool | None = None,
    show_ids: bool = False,
    compact: bool = False,
)
```

- `view` (`TableViewMode`; invalid raises `ValueError`; `descriptor` columns always show indented labels falling back to `row_id`): `"values"` (`"—"` for empty), `"formulas"` (lowered Excel or literal), `"expr"` (`f8` source or literal), `"addresses"` (A1).
- Subsetting & formatting: `cols` auto-prepends unlisted `descriptor` columns in declaration order (`KeyError` on unknown). `include_section_headers` defaults to `True` when `rows=None` and `False` when `rows` is passed (`True` with `rows` emits matching rows in declaration order with non-empty section headers). `show_ids=True` appends `[id]` when differing; `compact=True` disables column padding. Summary rows (`bold`/`subtotal`/`total`) are bolded; `TableAuditor` and `MarkdownTable` (`str` subclass) implement `_repr_markdown_()`.

### Dictionary Export (`auditor.to_dict`)

`auditor.to_dict(orient: TableOrient = "records")` (data rows x data cols, excluding section headers/gutters):

| `orient`              | Return type                             | Structure                                         |
| :-------------------- | :-------------------------------------- | :------------------------------------------------ |
| `"records"` (default) | `list[dict[str, CellScalarValue]]`      | `[{col_id: scalar}, ...]` (`pd.DataFrame` ready)  |
| `"dict"`              | `dict[str, list[CellScalarValue]]`      | `{col_id: [row_values]}`                          |
| `"index"`             | `dict[str, dict[str, CellScalarValue]]` | `{row_id: {col_id: scalar}}`                      |
| `"split"`             | `SplitDict`                             | `{"columns": [...], "index": [...], "data": ...}` |

## 3. Inspection Schemas (`f8.types`)

All inspection `TypedDict`s and unions (`ModelSummary`, `SheetSummary`, `TableSummary`, `CellError`, `CellInspection`, `CellNeighborInspection`, `DependencyTree`, `DependencyNode`, `TableInspection`, `ColumnInspection`, `RowInspection`, `TableCellInfo`, `SheetInspection`, `SheetRowInspection`, `SheetCellInspection`, `SplitDict`, `CellCoordinate`, `ExcelErrorCode`, `SheetCellKind`) are exported from `f8` and `f8.types`. Non-obvious quirks:

- `CellValueDict` tagged union: Raw inspection dicts (`inspect_cell`, `inspect_table`, `inspect_sheet`, `precedents_of`, `dependents_of`) return Rust-tagged payloads (`{"Number": 10.0}`, `{"Text": "s"}`, `{"Boolean": True}`, `"Empty"`, `{"Error": "Div0" | "Value" | "Ref" | "Num" | "Na" | "Name"}`). Call `f8.unbox_cell_value(val)` to convert to `CellScalarValue` (`compiled.value(...)` and `auditor.value(...)` unbox automatically).
- `TableSummary` counting: `row_count` includes section headers and row gutters, and `col_count` includes column gutters, whereas `cell_count` counts only calculable cells (`data_rows * data_cols`).
- `SheetCellKind` `"overflow"` vs. `"empty"`: In `inspect_sheet`, `"overflow"` marks spanned `grid`-mode descriptor runway cells (`is_descriptor=True`, `value="Empty"`, owning `table_id`/`row_id`/`col_id`), while `"empty"` marks unowned sheet canvas.
- Keys & addresses: `TableInspection.cells` is keyed by `'Table["row", "col"]'`; `TableCellInfo.physical_a1` omits the sheet prefix (`"C14"`) whereas `CellInspection.physical_address` includes it (`"'Sheet'!C14"`). `DependencyNode.is_pruned` flags already-expanded subtrees when `recursive=True`.

## 4. Structured Exception Hierarchy (`F8Error`)

All compiler, evaluation, inspection, and emission errors inherit from `F8Error(Exception)` with typed attributes:

- `F8Error` (`message: str`): Base class; raised directly on formula validation, evaluation, and lowering errors (unknown function, arity/`_` omission, kind/shape/role/type/branch-unification mismatch, disallowed empty).
- `CycleDetectedError` (`cycle: list[str]`, `message: str`): Circular DAG dependency; `cycle` holds ordered coordinate loop (`['T["a", "c1"]', ...]`).
- `ReferenceResolutionError` (`coordinate: str`, `target_table: str`, `row: str | None`, `col: str | None`, `message: str`): Unresolved table/row/col key, active axis mismatch, inverted/discontinuous slice, or out-of-bounds `prev`/`next` offset.
- `FormulaParseError` (`table: str`, `row: str | None`, `col: str | None`, `token_offset: int | None`, `message: str`): Lexical or syntax error (`token_offset` is byte offset, or `None` on EOF).
- `SpecificationError` (`table: str | None`, `reason: str`, `message: str`): Schema, 3-tier lattice, formatting, theme, or spatial layout violation (`table=None` for manifest/theme/deserialization errors).
- `InspectionError` (`entity: str`, `message: str`): Unknown sheet/table/cell or malformed coordinate in `CompiledModel` inspection.
- `EmissionError` (`message: str`): OpenXML serialization or file I/O failure in `save` / `emit_workbook`.
