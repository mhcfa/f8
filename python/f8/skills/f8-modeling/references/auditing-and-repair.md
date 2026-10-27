# Auditing and Diagnostics Reference

Model inspection via `CompiledModel` and `TableAuditor`, `TypedDict` schemas, tabular export (`to_markdown`, `to_dict`), and structured `F8Error` exception attributes.

---

## 1. `CompiledModel` API

```python
compiled = model.compile()
```

### Methods

| Method                                                                | Return type       | Behavior                                                                                               |
| :-------------------------------------------------------------------- | :---------------- | :----------------------------------------------------------------------------------------------------- |
| `compiled.summary()`                                                  | `ModelSummary`    | Counts for sheets, tables, cells, literal drivers, formulas, and per-sheet/per-table breakdowns        |
| `compiled.table(table)`                                               | `TableAuditor`    | High-level 2D table wrapper for coordinate lookup, row extraction, and Markdown/dict export            |
| `compiled.value(target, row=None, col=None)`                          | `CellScalarValue` | Unboxed scalar (`float`, `str`, `bool`, `None`, or Excel error string `"#DIV/0!"`, `"#VALUE!"`, etc.)  |
| `compiled.num(target, row=None, col=None)`                            | `float`           | Evaluated numeric `float` (raises `TypeError` if cell is text, `bool`, `None`, or error)               |
| `compiled.formatted(target, row=None, col=None, *, unit="")`          | `str`             | Formatted display string with optional `unit` spliced inside trailing `)` (e.g. `"($115.0M)"`)         |
| `compiled.inspect_cell(target, row=None, col=None)`                   | `CellInspection`  | Full cell record including evaluated/formatted values, formulas, formatting, and direct neighbors      |
| `compiled.inspect_table(table)`                                       | `TableInspection` | Raw 2D table inspection dictionary                                                                     |
| `compiled.inspect_sheet(sheet)`                                       | `SheetInspection` | Physical worksheet bounding box, column character widths, and row/cell layout records                  |
| `compiled.precedents_of(target, row=None, col=None, recursive=False)` | `DependencyTree`  | Upstream precedent tree (1-hop when `recursive=False`, full transitive DAG when `recursive=True`)      |
| `compiled.dependents_of(target, row=None, col=None, recursive=False)` | `DependencyTree`  | Downstream dependent tree (1-hop when `recursive=False`, full transitive DAG when `recursive=True`)    |
| `compiled.save(path, options=None)`                                   | `None`            | Emits `.xlsx` workbook to `path` (creates parent directories automatically)                            |
| `compiled.emit_workbook(options=None)`                                | `bytes`           | Emits `.xlsx` workbook as an in-memory `bytes` buffer                                                  |

### Coordinate Addressing in `CompiledModel`

Cell methods (`value`, `num`, `formatted`, `inspect_cell`, `precedents_of`, `dependents_of`) accept either a 1-argument coordinate string (`'Table["row", "col"]'`, `"Table['row', 'col']"`, or unquoted `'Table[row, col]'`, provided `row` and `col` IDs do not contain `,`, `[`, or `]`) or 3 positional arguments (`table, row, col`, which support arbitrary characters in `row` and `col`). Missing entities or malformed coordinates raise `InspectionError`:

```python
v1 = compiled.num('Operations["revenue", "2024E"]')
v2 = compiled.num("Operations", "revenue", "2024E")
```

### Workbook Emission Options (`EmissionOptions`)

Optional dictionary passed to `compiled.save(path, options=...)` or `compiled.emit_workbook(options=...)`:

- `values_only: bool` (default `False`): Writes evaluated literal values instead of Excel formulas.
- `omit_formula_cache: bool` (default `False`): Emits formulas without cached `<v>` result tags.
- `title: str`: Sets OpenXML document title property.
- `author: str`: Sets OpenXML document author property (defaults to `"f8 Compiler"`).

---

## 2. `TableAuditor` API

```python
auditor = compiled.table("Operations")
```

### Properties and Methods

| Member                                    | Return type             | Behavior                                                                                      |
| :---------------------------------------- | :---------------------- | :-------------------------------------------------------------------------------------------- |
| `auditor.id`                              | `str`                   | Table programmatic identifier                                                                 |
| `auditor.title`                           | `str`                   | Table display title                                                                           |
| `auditor.subtitle`                        | `str \| None`           | Optional table subtitle                                                                       |
| `auditor.sheet`                           | `str`                   | Parent worksheet name                                                                         |
| `auditor.rows`                            | `list[str]`             | Non-gutter, non-section-header data row IDs in declaration order                              |
| `auditor.columns`                         | `list[str]`             | Non-gutter data column IDs (`descriptor` and `measure`) in declaration order                  |
| `auditor.measure_columns`                 | `list[str]`             | Non-gutter `measure` column IDs in declaration order                                          |
| `auditor.shape`                           | `tuple[int, int]`       | `(len(auditor.rows), len(auditor.columns))`                                                   |
| `len(auditor)`                            | `int`                   | `len(auditor.rows)`                                                                           |
| `auditor[row, col]`                       | `CellScalarValue`       | Alias for `auditor.value(row, col)` (non-2-tuple or non-`str` key raises `TypeError`)         |
| `auditor.value(row, col)`                 | `CellScalarValue`       | Unboxed scalar (`float`, `str`, `bool`, `None`, or Excel error string; absent -> `KeyError`)  |
| `auditor.num(row, col)`                   | `float`                 | Evaluated numeric `float` (absent -> `KeyError`; non-numeric/bool/empty/error -> `TypeError`) |
| `auditor.formatted(row, col, *, unit="")` | `str`                   | Formatted display string with optional `unit` spliced inside trailing `)`                     |
| `auditor.row_values(row, cols=None)`      | `list[CellScalarValue]` | 1D list of `value(row, c)` across `measure_columns` (or explicit `cols`)                      |
| `auditor.row_nums(row, cols=None)`        | `list[float]`           | 1D list of `num(row, c)` across `measure_columns` (or explicit `cols`)                        |
| `auditor.formula(row, col)`               | `str \| None`           | Lowered Excel formula string (e.g. `"=C5*1.1"`), or `None` if literal driver                  |
| `auditor.expr(row, col)`                  | `str \| None`           | Source `f8` expression string, or `None` if literal driver                                    |
| `auditor.address(row, col)`               | `str`                   | Worksheet-local A1 cell address (e.g. `"C6"`)                                                 |
| `auditor.cell(row, col)`                  | `TableCellInfo`         | Raw cell inspection dictionary (raises `KeyError` if absent)                                  |

### Unboxing Raw `CellValueDict` Payloads

`compiled.value(...)` and `auditor.value(...)` return unboxed Python scalars directly. When reading raw dictionaries from `inspect_cell`, `inspect_table`, `inspect_sheet`, or `precedents_of`, use `f8.unbox_cell_value(val)` to convert serde-tagged `CellValueDict` payloads (`{"Number": 10.0}`, `{"Text": "s"}`, `{"Boolean": True}`, `"Empty"`, `{"Error": "Div0" | "Value" | "Ref" | "Num" | "Na" | "Name"}`) into `float | str | bool | None`.

---

## 3. Tabular Export (`to_markdown`, `to_dict`)

### GitHub-Flavored Markdown (`auditor.to_markdown`)

```python
md: MarkdownTable = auditor.to_markdown(
    view: TableViewMode = "values",
    *,
    rows: Sequence[str] | None = None,
    cols: Sequence[str] | None = None,
    include_section_headers: bool | None = None,
    show_ids: bool = False,
)
```

- `view` modes (`TableViewMode`; unrecognized value raises `ValueError`; `descriptor` columns always display their indented text label across all modes, falling back to `row_id` when empty, while `measure` columns switch by `view`):
  - `"values"` (default): Pre-formatted strings (`"—"` for empty `measure` cells).
  - `"formulas"`: Lowered Excel formulas (`"=C5*1.15"`) for formula cells, formatted values for literals (`""` if empty).
  - `"expr"`: Source `f8` expressions for formula cells, formatted values for literals (`""` if empty).
  - `"addresses"`: Physical A1 addresses (`"D7"`).
- Subsetting and formatting:
  - `cols`: Any `descriptor` columns not explicitly listed in `cols` are automatically prepended in declaration order ahead of the requested `cols` (which preserve the caller's list order; unknown ID raises `KeyError`). `descriptor` columns are left-aligned (`:---`); `measure` columns are right-aligned (`---:`).
  - `rows`: Renders only requested `rows` (unknown ID raises `KeyError`). `include_section_headers` defaults to `True` when `rows=None` and `False` when `rows` is specified.
  - `show_ids=True`: Appends `[{col_id}]` to column headers and `[{row_id}]` to descriptor labels when display text differs (case-insensitively) from the ID.
  - Rows with `style="subtotal"` or `style="total"` wrap non-empty cell text in `**...**`. Both `TableAuditor` and `MarkdownTable` (`str` subclass) implement `_repr_markdown_()`.

### Dictionary and DataFrame Export (`auditor.to_dict`)

```python
data = auditor.to_dict(orient: TableOrient = "records")
```

| `orient`              | Return type                             | Structure (shape `len(auditor.rows)` x `len(auditor.columns)`, omitting section headers/gutters) |
| :-------------------- | :-------------------------------------- | :----------------------------------------------------------------------------------------------- |
| `"records"` (default) | `list[dict[str, CellScalarValue]]`      | Row list of `{col_id: scalar_value}` dictionaries (`pd.DataFrame(auditor.to_dict("records"))`)   |
| `"dict"`              | `dict[str, list[CellScalarValue]]`      | `{col_id: [row_ordered_values]}`                                                                 |
| `"index"`             | `dict[str, dict[str, CellScalarValue]]` | `{row_id: {col_id: scalar_value}}`                                                               |
| `"split"`             | `SplitDict`                             | `{"columns": list[str], "index": list[str], "data": list[list[CellScalarValue]]}`                |

---

## 4. Inspection Schemas (`f8.types`)

- `ModelSummary`: `model_name: str`, `theme_name: str`, `sheets: list[SheetSummary]`, `total_sheets: int`, `total_tables: int`, `total_cells: int`, `total_drivers: int`, `total_formulas: int`.
  - `SheetSummary`: `name: str`, `tables: list[TableSummary]`, `total_cells: int`.
  - `TableSummary`: `id: str`, `title: str`, `row_count: int` (includes section headers and row gutters), `col_count: int` (includes column gutters), `cell_count: int` (`data_rows * data_cols`), `driver_count: int`, `formula_count: int`.
- `CellInspection`: `coordinate: CellCoordinate` (`{"table": str, "row": str, "col": str}`), `sheet: str`, `physical_address: str` (`"Sheet!C14"`), `evaluated_value: CellValueDict`, `formatted_value: str | None`, `format: FormatPreset | None`, `decimals: int | None`, `currency: CurrencyCode | None`, `is_highlight: bool`, `shade: Shade | None`, `indent: int`, `is_formula: bool`, `formula_expr: str | None`, `excel_formula: str | None`, `note: str | None`, `precedents: list[CellNeighborInspection]`, `dependents: list[CellNeighborInspection]`.
  - `CellNeighborInspection`: `cell_id: CellCoordinate`, `sheet_address: str`, `value: CellValueDict | None`, `formatted_value: str | None`.
- `DependencyTree`: `root: CellCoordinate`, `direct_count: int`, `transitive_count: int`, `nodes: list[DependencyNode]`.
  - `DependencyNode`: `cell_id: CellCoordinate`, `sheet_address: str`, `value: CellValueDict | None`, `formatted_value: str | None`, `excel_formula: str | None`, `is_pruned: bool` (marks already-expanded subtrees when `recursive=True`), `children: list[DependencyNode]`.
- `TableInspection`: `table_id: str`, `title: str`, `subtitle: str | None`, `sheet: str`, `columns: list[ColumnInspection]`, `rows: list[RowInspection]`, `cells: dict[str, TableCellInfo]` (keyed by `'Table["row", "col"]'`).
  - `ColumnInspection`: `id: str | None`, `group: str | None`, `headers: list[str]`, `is_gutter: bool`, `is_descriptor: bool`, `style: SemanticStyle | None` (defaults to `"data"` on data columns; `None` on gutters), `shade: Shade | None`.
  - `RowInspection`: `id: str | None`, `section_title: str | None`, `is_section_header: bool`, `is_gutter: bool`, `indent: int` (for data rows, max `cell.indent` across all cells in that row; for section headers, `SectionHeaderSpec.indent` or `0`; `to_markdown()` indents all descriptor columns on data rows by `'  ' * indent`), `style: SemanticStyle | None` (defaults to `"data"` on data rows; `None` on gutters/section headers), `shade: Shade | None`.
  - `TableCellInfo`: `value: CellValueDict`, `formatted_value: str | None`, `is_formula: bool`, `formula_expr: str | None`, `excel_formula: str | None`, `physical_a1: str` (`"C14"` without sheet prefix), `format: FormatPreset | None`, `decimals: int | None`, `currency: CurrencyCode | None`.
- `SheetInspection`: `sheet_id: str`, `title: str`, `tables: list[str]`, `min_col: int`, `max_col: int`, `min_row: int`, `max_row: int`, `col_widths: dict[int, float]` (0-indexed column -> character width), `rows: list[SheetRowInspection]`.
  - `SheetRowInspection`: `physical_row: int` (0-indexed), `excel_row: int` (1-indexed), `cells: list[SheetCellInspection]`.
  - `SheetCellInspection`: `physical_col: int`, `physical_a1: str`, `table_id: str | None`, `row_id: str | None`, `col_id: str | None`, `kind: SheetCellKind` (`"empty" | "table_title" | "table_subtitle" | "column_header" | "section_header" | "gutter" | "data"`), `is_descriptor: bool` (`True` only when `kind == "data"` and the column is `descriptor`), `value: CellValueDict`, `formatted_value: str | None`, `formula_expr: str | None`, `excel_formula: str | None`.

---

## 5. Structured Exception Hierarchy (`F8Error`)

All compiler, evaluation, inspection, and emission errors inherit from `F8Error(Exception)` and expose typed attributes:

| Exception                  | Attributes                                                                                             | Trigger conditions                                                                                                                                                            |
| :------------------------- | :----------------------------------------------------------------------------------------------------- | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `F8Error`                  | `message: str`                                                                                         | Base exception class; raised directly on function call validation failures (`UnknownFunction`, `TooFewArguments`, `TooManyArguments`, `InvalidVariadicPairs`, `CannotOmitRequiredArgument`, `InvalidArgumentKind` when a lazy branch returns a multi-cell range) |
| `CycleDetectedError`       | `cycle: list[str]`, `message: str`                                                                     | Circular dependency in DAG; `cycle` contains ordered coordinate strings forming the loop (`['T["a", "c1"]', 'T["b", "c1"]', 'T["a", "c1"]']`)                                  |
| `ReferenceResolutionError` | `coordinate: str`, `target_table: str \| None`, `row: str \| None`, `col: str \| None`, `message: str` | Unresolved table/row/col key, active axis mismatch, inverted slice, or out-of-bounds `prev`/`next` offset from origin `coordinate`                                            |
| `FormulaParseError`        | `table: str`, `row: str \| None`, `col: str \| None`, `token_offset: int \| None`, `message: str`      | Lexical or grammatical syntax error in formula (`token_offset` is byte offset in expression string, or `None` on unexpected EOF)                                              |
| `SpecificationError`       | `table: str \| None`, `reason: str`, `message: str`                                                    | Schema, 3-tier lattice, formatting, theme, or spatial layout violation (`table` is `None` for manifest/theme/deserialization errors)                                          |
| `InspectionError`          | `entity: str`, `message: str`                                                                          | Unknown sheet, table, or cell coordinate (or malformed coordinate string) passed to `CompiledModel` inspection methods                                                        |
| `EmissionError`            | `message: str`                                                                                         | OpenXML serialization error, invalid `tab_color` hex string, or file I/O failure during `save`/`emit_workbook`                                                                |

### `ReferenceResolutionError` Attribute Matrix

| Failure kind                    | `target_table` | `row`        | `col`        | Cause                                                                                     |
| :------------------------------ | :------------- | :----------- | :----------- | :---------------------------------------------------------------------------------------- |
| `UnknownTable` / `UnregisteredTable` | `str`     | `None`       | `None`       | Referenced table ID does not exist in model                                               |
| `InvertedRange`                 | `str`          | `None`       | `None`       | Slice `start..end` resolves to `start_idx > end_idx` in `target_table`                    |
| `UnknownRow`                    | `str`          | `str`        | `None`       | Explicit or offset-resolved row key does not exist in `target_table`                      |
| `ActiveRowMismatch`             | `str`          | `str`        | `None`       | `@row` evaluated at origin row ID that does not exist in `target_table`                   |
| `UnknownColumn`                 | `str`          | `None`       | `str`        | Explicit or offset-resolved column key does not exist in `target_table`                   |
| `ActiveColumnMismatch`          | `str`          | `None`       | `str`        | `@col` evaluated at origin column ID that does not exist in `target_table`                |
| `TemporalOffsetOutOfBounds`     | `None`         | `None`       | `None`       | `col.prev`/`col.next` or `row.prev`/`row.next` steps outside origin table axis bounds (or `col.prev`/`col.next` evaluated from a `descriptor` column) |
