# Auditing and Self-Healing Reference

Model inspection via `CompiledModel` and `TableAuditor`, tabular export, calculation graph traversal, and structured exception diagnostics.

---

## 1. Model Inspection with CompiledModel

Compiling produces an in-memory `CompiledModel` providing summary statistics, discrete cell lookups, dependency trees, and table auditors:

```python
compiled = model.compile()
```

### Inspection Methods and Properties

| Method                                                                | Return type       | Description                                                                               |
| :-------------------------------------------------------------------- | :---------------- | :---------------------------------------------------------------------------------------- |
| `compiled.summary()`                                                  | `ModelSummary`    | Total counts for sheets, tables, cells, drivers, formulas, and per-sheet breakdowns       |
| `compiled.table(table_id)`                                            | `TableAuditor`    | Interactive 2D table auditor wrapper for tabular auditing and export                      |
| `compiled.value(target, row=None, col=None)`                          | `CellScalarValue` | Fast-path unboxed scalar value (`float`, `str`, `bool`, `None`, or Excel error string)    |
| `compiled.num(target, row=None, col=None)`                            | `float`           | Fast-path evaluated numeric `float` value (raises `TypeError` if non-numeric/empty/error) |
| `compiled.formatted(target, row=None, col=None, *, unit="")`          | `str`             | Fast-path formatted display string with optional unit suffix spliced inside `)`           |
| `compiled.inspect_cell(target, row=None, col=None)`                   | `CellInspection`  | Full cell metadata, evaluated value, physical address, and direct neighbors               |
| `compiled.inspect_table(table_id)`                                    | `TableInspection` | Raw 2D table inspection dictionary                                                        |
| `compiled.inspect_sheet(sheet_id)`                                    | `SheetInspection` | Worksheet physical bounding box, column widths, and cell layout                           |
| `compiled.precedents_of(target, row=None, col=None, recursive=False)` | `DependencyTree`  | Inbound dependency calculation tree (direct or recursive)                                 |
| `compiled.dependents_of(target, row=None, col=None, recursive=False)` | `DependencyTree`  | Outbound blast-radius calculation tree (direct or recursive)                              |
| `compiled.save(path, options=None)`                                   | `None`            | Serializes model to `.xlsx` OpenXML workbook on disk                                      |
| `compiled.emit_workbook(options=None)`                                | `bytes`           | Emits raw in-memory OpenXML workbook binary buffer                                        |

### Coordinate Addressing in Inspection APIs

Accepts either a single canonical coordinate string or discrete positional arguments:

```python
# 1-argument canonical coordinate syntax
rev = compiled.num('Operations["revenue", "2024E"]')
rev_str = compiled.formatted('Operations["revenue", "2024E"]', unit="M")
cell = compiled.inspect_cell('Operations["revenue", "2024E"]')
tree = compiled.precedents_of('Operations["ebitda", "2024E"]', recursive=True)

# 3-argument positional syntax
rev = compiled.num("Operations", "revenue", "2024E")
rev_str = compiled.formatted("Operations", "revenue", "2024E", unit="M")
cell = compiled.inspect_cell("Operations", "revenue", "2024E")
tree = compiled.precedents_of("Operations", "ebitda", "2024E", recursive=True)
```

### Inspection Data Contracts

Contracts defined in `f8.types`:

- **`ModelSummary`**: Top-level model metrics:
  - `model_name`: Declared model identifier.
  - `theme_name`: Applied visual theme.
  - `sheets`: List of `SheetSummary` entries (`name`, `tables`, `total_cells`).
  - Total counters: `total_sheets`, `total_tables`, `total_cells`, `total_drivers`, `total_formulas`.
  - Subordinate `TableSummary` entries in `sheets[*].tables`: `id`, `title`, `row_count`, `col_count`, `cell_count`, `driver_count`, `formula_count`.
- **`CellInspection`**: Comprehensive single-cell audit record:
  - `coordinate`: `CellCoordinate` dictionary (`table`, `row`, `col`).
  - `sheet`: Parent worksheet identifier.
  - `physical_address`: Canonical worksheet address (e.g. `"Financials!C14"`).
  - `evaluated_value`: Evaluated runtime value dictionary (`CellValueDict`).
  - `formatted_value`: Formatted display string with accounting typography (e.g. `"$115.0"`).
  - `format`, `decimals`, `currency`: Typography preset and precision metadata.
  - `is_highlight`: Visual fill emphasis flag.
  - `indent`: Descriptor indentation depth integer.
  - `is_formula`: `True` if calculated by formula, `False` if constant literal.
  - `formula_expr`: High-level `f8` vector expression (or `None` if driver).
  - `excel_formula`: Lowered physical Excel formula string (e.g. `"=C5*1.15"`).
  - `note`: Author note string or `None`.
  - `precedents`: Immediate incoming `CellNeighborInspection` records (`cell_id`, `sheet_address`, `value`, `formatted_value`).
  - `dependents`: Immediate outgoing `CellNeighborInspection` records (`cell_id`, `sheet_address`, `value`, `formatted_value`).
- **`DependencyTree`**: Calculation dependency tree:
  - `root`: `CellCoordinate` dictionary (`table`, `row`, `col`).
  - `direct_count`: Immediate 1-hop dependencies.
  - `transitive_count`: Total unique cells reached across all calculation tiers (`direct_count` when `recursive=False`).
  - `nodes`: List of `DependencyNode` entries (`cell_id`, `sheet_address`, `value`, `formatted_value`, `excel_formula`, `is_pruned`, `children`). When `recursive=True`, `is_pruned` breaks circular paths and eliminates duplicate subtrees.
- **`TableInspection`**: 2D table grid metadata:
  - `table_id`, `title`, `subtitle`: Table identifiers and display headers.
  - `sheet`: Parent worksheet identifier.
  - `columns`: List of `ColumnInspection` records (`id`, `headers`, `is_gutter`, `is_descriptor`, `style`).
  - `rows`: List of `RowInspection` records (`id`, `section_title`, `is_section_header`, `is_gutter`, `indent`, `style`).
  - `cells`: Dictionary keyed by canonical coordinate string (`'Table["row", "col"]'`) mapping to `TableCellInfo`.
- **`SheetInspection`**: Physical worksheet grid layout:
  - `sheet_id`, `title`: Worksheet identifiers.
  - `tables`: List of placed table identifiers.
  - `min_col`, `max_col`, `min_row`, `max_row`: 0-indexed physical grid bounding box.
  - `col_widths`: Dictionary mapping 0-indexed column index to width in points.
  - `rows`: List of `SheetRowInspection` records (`physical_row`, `excel_row`, `cells` as `list[SheetCellInspection]`).

### Workbook Emission Options

`compiled.save()` and `compiled.emit_workbook()` accept optional `EmissionOptions`:

- `values_only: bool`: Emits calculated static values without underlying formulas.
- `omit_formula_cache: bool`: Omits formula cached values, forcing Excel recalculation upon opening.
- `title: str`, `author: str`: Document metadata properties embedded in OpenXML package.

`compiled.save(path)` automatically creates non-existent parent directories.

---

## 2. Interactive Auditing with TableAuditor

`TableAuditor` provides 2D coordinate lookups, dimension metadata, 1D row series extraction, and formatted cell access for a specific table:

```python
auditor = compiled.table("Operations")
```

### Inspection Members and Properties

| Member                                    | Return type             | Description                                                                     |
| :---------------------------------------- | :---------------------- | :------------------------------------------------------------------------------ |
| `auditor.id`                              | `str`                   | Unique table identifier                                                         |
| `auditor.title`                           | `str`                   | Table display title                                                             |
| `auditor.subtitle`                        | `str \| None`           | Optional table subtitle or units label                                          |
| `auditor.sheet`                           | `str`                   | Parent worksheet identifier                                                     |
| `auditor.shape`                           | `tuple[int, int]`       | Dimensions `(row_count, col_count)` of non-gutter, non-section data grid        |
| `auditor.rows`                            | `list[str]`             | Non-gutter, non-section data row IDs in layout order                            |
| `auditor.columns`                         | `list[str]`             | Non-gutter column IDs in layout order                                           |
| `auditor.measure_columns`                 | `list[str]`             | Non-gutter, non-descriptor measure column IDs in layout order                   |
| `len(auditor)`                            | `int`                   | Number of calculable data rows (`len(auditor.rows)`)                            |
| `auditor[row, col]`                       | `CellScalarValue`       | 2D coordinate bracket indexing returning unboxed scalar value                   |
| `auditor.value(row, col)`                 | `CellScalarValue`       | Evaluated scalar value (`float`, `int`, `str`, `bool`, or `None`)               |
| `auditor.num(row, col)`                   | `float`                 | Evaluated numeric `float` value (raises `TypeError` if non-numeric/empty/error) |
| `auditor.formatted(row, col, *, unit="")` | `str`                   | Pre-formatted display string with optional unit suffix spliced inside `)`       |
| `auditor.row_values(row, cols=None)`      | `list[CellScalarValue]` | 1D list of scalar values across `measure_columns` (or specified `cols`)         |
| `auditor.row_nums(row, cols=None)`        | `list[float]`           | 1D list of `float` values across `measure_columns` (or specified `cols`)        |
| `auditor.formula(row, col)`               | `str \| None`           | Lowered physical Excel formula (e.g. `"=C5*0.2"`), or `None` if driver          |
| `auditor.expr(row, col)`                  | `str \| None`           | High-level `f8` vector expression, or `None` if driver                          |
| `auditor.address(row, col)`               | `str`                   | Physical Excel cell address (e.g. `"C6"`)                                       |
| `auditor.cell(row, col)`                  | `TableCellInfo`         | Raw cell metadata dictionary (raises `KeyError` if absent)                      |

### TableCellInfo Schema

Returned by `auditor.cell(row, col)` and `compiled.inspect_table(table_id)["cells"]`:

- `value`: Evaluated serde value payload (`CellValueDict`).
- `formatted_value`: Formatted display string with accounting typography (e.g. `"$115.0"`), or `None`.
- `is_formula`: `True` if formula-calculated, `False` if literal driver.
- `formula_expr`: High-level vector DSL expression string, or `None`.
- `excel_formula`: Lowered physical Excel formula string (e.g. `"=C5*1.15"`), or `None`.
- `physical_a1`: Physical worksheet cell address without sheet name (e.g. `"D7"`).
- `format`, `decimals`, `currency`: Typography preset and precision metadata.

> [!NOTE]
> `CellInspection` from `compiled.inspect_cell` uses key `evaluated_value` and worksheet-qualified `physical_address` (`"Sheet!D7"`), whereas `TableCellInfo` from `auditor.cell` uses key `value` and table-local `physical_a1` (`"D7"`).

### Coordinate Lookup Error Semantics

- **Absent coordinate (`KeyError`)**: Calling `auditor.cell(row, col)`, `auditor.value(row, col)`, `auditor.num(row, col)`, `auditor.formatted(row, col)`, `auditor.row_values(row, cols)`, `auditor.row_nums(row, cols)`, `auditor.to_markdown(rows=..., cols=...)`, or `auditor[row, col]` with non-existent row or column IDs raises `KeyError`.
- **Non-numeric value (`TypeError`)**: Calling `compiled.num(...)`, `auditor.num(row, col)`, or `auditor.row_nums(row, cols)` on a cell that evaluated to text, boolean, `None` (empty), or an Excel error raises `TypeError`.
- **Invalid index key (`TypeError`)**: Bracket indexing `auditor[key]` expects a 2-tuple `(row, col)`. Passing non-tuple or non-string elements raises `TypeError`.
- **Unassigned cell (`None`)**: A calculable coordinate declared within table bounds but unassigned evaluates to `None`.

### Cell Value Unboxing and Error Codes

Values unbox into standard Python scalars (`float`, `int`, `str`, `bool`, `None`). Prefer `compiled.value(...)`, `compiled.num(...)`, `auditor.value(...)`, or `auditor.num(...)` for direct scalar reads. When inspecting raw cell dictionaries, `unbox_cell_value` unboxes the underlying serde payload:

```python
from f8 import unbox_cell_value

# Unbox from inspect_cell ("evaluated_value" key)
raw_cell = compiled.inspect_cell("Operations", "revenue", "2024E")
val1 = unbox_cell_value(raw_cell["evaluated_value"])

# Unbox from inspect_table or auditor.cell ("value" key)
table_cell = auditor.cell("revenue", "2024E")
val2 = unbox_cell_value(table_cell["value"])
```

Calculation failures map to standard Excel error strings:

- `#DIV/0!`: Division by zero.
- `#VALUE!`: Type coercion failure or dimension mismatch.
- `#REF!`: Out-of-bounds coordinate reference.
- `#NUM!`: Numeric iteration failure or invalid domain argument.
- `#N/A`: Unmatched lookup without fallback.
- `#NAME?`: Unrecognized identifier.

---

## 3. Tabular Export and Visual Rendering

### Visual Markdown Table

Renders aligned GitHub-Flavored Markdown tables (`MarkdownTable`, a `str` subclass implementing `_repr_markdown_()`) with accounting typography:

```python
markdown_str = auditor.to_markdown(
    view="values",
    rows=None,
    cols=None,
    include_section_headers=None,
    show_ids=False,
)
```

| View mode            | Content rendered in data cells                                                                   |
| :------------------- | :----------------------------------------------------------------------------------------------- |
| `"values"` (default) | Formatted values with accounting typography, currency symbols, and `"—"` for empty measure cells |
| `"formulas"`         | Lowered physical Excel formulas (e.g. `"=C5*1.15"`) or literal values                            |
| `"expr"`             | Raw declarative `f8` DSL expressions or literal values                                           |
| `"addresses"`        | Physical worksheet cell coordinates (e.g. `"D7"`)                                                |

- **Single-row GFM headers**: Multi-tier column headers are flattened into a single valid GFM header row joined with `": "` (e.g. `"Historical Actuals: 2023A"`).
- **Row and column subsetting**: Pass `rows=[...]` and/or `cols=[...]` to render a focused slice. Descriptor columns are automatically retained alongside `cols`. When `rows` is specified, section headers are omitted by default unless `include_section_headers=True`.
- **Summary row bolding**: Rows with `style="subtotal"` or `style="total"` automatically bold all non-empty cells (`**...**`).
- **Coordinate ID suffixes (`show_ids`)**: Defaults to `False` for clean report publishing. Set `show_ids=True` during debugging to append `[{col_id}]` and `[{row_id}]` when display text differs from coordinate IDs.
- **IPython / Quarto display**: Both `TableAuditor` and `MarkdownTable` implement `_repr_markdown_()` for direct rendering in Jupyter and Quarto without `Markdown(...)` wrappers. Unrecognized `view` raises `ValueError`.

### Tabular Dictionary Export and DataFrame Ingestion

Exports rectangular matrices mapping unassigned cells to `None`:

```python
data = auditor.to_dict(orient="records")
```

| Orientation           | Return type                             | Structure description                                                  |
| :-------------------- | :-------------------------------------- | :--------------------------------------------------------------------- |
| `"records"` (default) | `list[dict[str, CellScalarValue]]`      | List of row dictionaries keyed by column ID                            |
| `"dict"`              | `dict[str, list[CellScalarValue]]`      | Column dictionary keyed by column ID with row-ordered value lists      |
| `"index"`             | `dict[str, dict[str, CellScalarValue]]` | Row dictionary keyed by row ID containing column-value mappings        |
| `"split"`             | `SplitDict`                             | Dictionary containing `columns`, `index` row IDs, and 2D `data` matrix |

Dictionary exports strictly omit section headers and gutter rows, guaranteeing uniform rectangular output of shape `(len(auditor.rows), len(auditor.columns))`. Unrecognized `orient` raises `ValueError`.

Direct ingestion into pandas:

```python
import pandas as pd

df = pd.DataFrame(auditor.to_dict(orient="records"))
```

---

## 4. Structured Exception Hierarchy

The compiler raises structured exceptions inheriting from `F8Error` (subclass of Python's standard `Exception`). All carry typed attributes for diagnostic analysis:

| Exception                  | Typed attributes                                                                                       | Root cause                                                                                                                          |
| :------------------------- | :----------------------------------------------------------------------------------------------------- | :---------------------------------------------------------------------------------------------------------------------------------- |
| `CycleDetectedError`       | `cycle: list[str]`, `message: str`                                                                     | Dependency cycle in calculation graph                                                                                               |
| `ReferenceResolutionError` | `coordinate: str`, `target_table: str \| None`, `row: str \| None`, `col: str \| None`, `message: str` | Undeclared table, row, or column ID, active axis mismatch, inverted range slice, or temporal lag out of bounds                      |
| `FormulaParseError`        | `table: str`, `row: str \| None`, `col: str \| None`, `token_offset: int \| None`, `message: str`      | Formula syntax violation (unquoted key, lowercase function/boolean, bare `@`, invalid operator, or misplaced `_`)                   |
| `SpecificationError`       | `table: str \| None`, `reason: str`, `message: str`                                                    | Structural model constraint violation (unassigned cells, format/decimals/currency conflict, duplicate IDs, invalid indent/decimals) |
| `InspectionError`          | `entity: str`, `message: str`                                                                          | Entity lookup failure during cell, table, or sheet inspection                                                                       |
| `EmissionError`            | `message: str`                                                                                         | OpenXML workbook packaging or filesystem persistence failure                                                                        |

### Programmatic Exception Handling Pattern

```python
from f8 import (
    CycleDetectedError,
    EmissionError,
    F8Error,
    FormulaParseError,
    InspectionError,
    ReferenceResolutionError,
    SpecificationError,
)

try:
    compiled = model.compile()
    compiled.save("models/forecast.xlsx")
    auditor = compiled.table("Operations")
except CycleDetectedError as err:
    print(f"Calculation cycle detected: {' -> '.join(err.cycle)}")
except ReferenceResolutionError as err:
    print(
        f"Unresolved reference at {err.coordinate}: table={err.target_table}, row={err.row}, col={err.col}"
    )
except FormulaParseError as err:
    print(f"Syntax error in table {err.table} at offset {err.token_offset}: {err.message}")
except SpecificationError as err:
    print(f"Model constraint violation in table {err.table}: {err.reason}")
except (InspectionError, EmissionError) as err:
    print(f"Operational failure: {err}")
```

---

## 5. Diagnostic and Self-Healing Guidelines

Inspect typed attributes to determine failure level (model, table, vector axis, or 0D cell override) and apply structural remediation.

### Circular Dependencies (`CycleDetectedError`)

- **Diagnostic attributes**: `err.cycle` contains ordered path of coordinates forming the loop (e.g. `['T["a", "2024E"]', 'T["b", "2024E"]', 'T["a", "2024E"]']`).
- **Inspection rules**:
  - Check whether the cycle is a single-cell self-reference (e.g. `T["a", @col] + 1`).
  - Check whether mutual references across measure columns lack temporal boundaries.
- **Remediation**:
  - Convert intra-period active column references (`@col`) to temporal lags (`col.prev` or `col.prev(k)`).
  - Provide an explicit exogenous base-period value in `cells` on the first measure column when using `col.prev`.
  - Break circular chains by introducing an intermediate driver or state variable.

### Reference Failures (`ReferenceResolutionError`)

- **Diagnostic attributes**:
  - `err.coordinate`: Origin cell coordinate where resolution halted.
  - `err.target_table`: Referenced table identifier.
  - `err.row`: Referenced row identifier.
  - `err.col`: Referenced column identifier.
- **Inspection rules**:
  - If `err.target_table` does not match declared tables, verify table spelling or registration order.
  - If `err.row` or `err.col` is populated, check whether ID exists in target table.
  - If `err.target_table` is populated but `err.row` and `err.col` are `None`, check for inverted range slices (`"2025E".."2023A"`) or unregistered tables.
  - If `err.target_table`, `row`, and `col` are all `None`, check for out-of-bounds temporal navigation (`col.prev` on first measure column without base override, or `col.next` on final measure column).
- **Remediation**:
  - Correct coordinate keys to match declared table, row, and column IDs.
  - Ensure range slices follow declared axis order: `Table["row", "start".."end"]`.
  - Supply base-period override in `cells={"start_col": <value>}` to terminate `col.prev` lags.

### Formula Grammar Failures (`FormulaParseError`)

- **Diagnostic attributes**:
  - `err.table`: Table containing invalid formula.
  - `err.row`: Row identifier if failure occurred on row vector formula or 0D override.
  - `err.col`: Column identifier if failure occurred on column vector formula or 0D override.
  - `err.token_offset`: Character index where parsing failed (`None` on unexpected EOF).
- **Inspection rules**: Check `err.token_offset` in source expression to locate invalid token.
- **Remediation**:
  - Enclose coordinate keys in double quotes: `Table["row", @col]` instead of `Table[row, @col]`.
  - Ensure table identifier is bare: `Table[...]` instead of `"Table"[...]`.
  - Uppercase all function names (`SUM`, `AVERAGE`, `IF`, `NPV`) and booleans (`TRUE`, `FALSE`).
  - Specify explicit axis specifiers (`@col`, `@row`); never bare `@`.
  - Confine argument omission (`_`) strictly to function call parameter lists.
  - Replace relational equality `==` with `=` and inequality `!=` with `<>`.

### Model Constraint Violations (`SpecificationError`)

- **Diagnostic attributes**:
  - `err.table`: Associated table identifier (`None` if model or sheet constraint).
  - `err.reason`: Explanation of constraint failure.
- **Inspection rules**:
  - **Unassigned cell**: Measure cell lacks row expression, column expression, and 0D override; or descriptor cell lacks a label.
  - **Vector conflict**: Intersecting row and column both define expressions (`row.expr` XOR `col.expr`), or conflicting `format`/`decimals`/`currency` without 0D override.
  - **Invalid formatting options**: `indent` specified on measure cell, `currency` specified on non-currency preset, or `decimals` specified on non-numeric preset or exceeding 14.
  - **Duplicate identifiers**: Colliding table, sheet, row, or column IDs.
- **Remediation**:
  - Supply missing values in `cells` or provide vector expressions covering all measure columns.
  - Add 0D `Cell` override to disambiguate intersecting vector formulas or formatting conflicts.
  - Move indentation from rows/measure cells to descriptor cells: `cells={"line_item": Cell("Title", indent=1)}`.

### Inspection Failures (`InspectionError`)

- **Diagnostic attributes**: `err.entity` (identifier string of missing entity or unparseable coordinate).
- **Inspection rules**: Triggered during post-compile inspection when requesting unregistered table, non-existent worksheet, unpopulated cell coordinate, or passing malformed coordinate string.
- **Remediation**:
  - Check `compiled.summary()` to verify declared sheet and table identifiers.
  - Check `auditor.rows` and `auditor.columns` to verify valid keys before looking up cells.
  - Ensure coordinate strings match standard syntax: `'Table["row", "col"]'` or use 3 positional arguments: `compiled.inspect_cell("Table", "row", "col")`.

### Emission Failures (`EmissionError`)

- **Diagnostic attributes**: `err.message` (OS I/O error or OpenXML packaging failure details).
- **Inspection rules**: Triggered during `compiled.save(path)` or `compiled.emit_workbook()` when filesystem write permissions are denied, target file is locked by Excel, or path targets an existing directory.
- **Remediation**:
  - Ensure destination workbook file is closed and unlocked in Microsoft Excel or other viewers.
  - Verify filesystem write permissions for destination path.
