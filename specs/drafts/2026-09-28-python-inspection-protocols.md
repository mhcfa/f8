---
title: Python Table Inspection Protocols & Tabular Data Export
date: 2026-09-28
status: draft
target-living-spec: specs/living-specs/interfaces/python-sdk.md
---

## Proposal

### What

Implement high-level table auditing, coordinate inspection, and zero-dependency tabular data export protocols in the Python SDK (`f8`). Specifically:
1. Provide a lightweight, pure Python `TableAuditor` wrapper returned by `compiled.table(table_id)`.
2. Implement `.to_dict(orient="records" | "dict" | "index" | "split")` with unboxed Python scalar types (`float`, `int`, `str`, `bool`, `None`) for universal, zero-dependency ingestion into modern data engines (`pandas`, `polars`, `duckdb`) and standard Python loops.
3. Implement direct cell attribute lookups on `TableAuditor`: `.value(row, col)`, `.formula(row, col)`, `.expr(row, col)`, `.address(row, col)`, and `.cell(row, col)`.
4. Implement coordinate indexing ergonomics via `table[row, col]` mapping directly to `table.value(row, col)`.
5. Implement zero-dependency GitHub-Flavored Markdown table rendering via `.to_markdown(view="values" | "formulas" | "expr" | "addresses")` with aligned columns and section divider rows for instant LLM agent inspection and chat context logging.

### Why

While `CompiledModel` exposes low-level inspection methods (`inspect_table`, `inspect_cell`), the raw outputs present significant friction for Python users and AI agents:
1. **Serialization overhead & tagging**: `inspect_table` returns flat coordinate maps with Serde-tagged enum values (e.g. `{"Number": 115.0}`), requiring verbose unpacking to obtain raw Python numbers or strings.
2. **Missing tabular export**: Analysts and automated pipelines frequently need to pass evaluated model outputs into dataframe libraries or database clients without `f8` requiring heavy third-party dataframe dependencies.
3. **Lack of visual 2D inspection in Python**: In the CLI, `f8 inspect table` produces clean, aligned Markdown tables across values and formulas. In Python, agents currently have to write 30 lines of boilerplate to reshape flat coordinate dictionaries into readable 2D grids.

Providing zero-dependency table auditing and dictionary export completes Phase 3 of the Python SDK roadmap defined in [`docs/interfaces/sdk.md`](../../docs/interfaces/sdk.md).

### Scope

- **In-Scope**:
  - `python/f8/audit.py`: New module defining `TableAuditor`, canonical Excel error mapping, and unboxed scalar extraction helpers.
  - `CompiledModel.table(table_id: str) -> TableAuditor`: High-level table accessor method bound to `CompiledModel`.
  - `TableAuditor.to_dict(orient="records" | "dict" | "index" | "split" = "records")`:
    - `records`: `list[dict[str, CellScalarValue]]`
    - `dict`: `dict[str, list[CellScalarValue]]`
    - `index`: `dict[str, dict[str, CellScalarValue]]`
    - `split`: `SplitDict` containing `columns`, `index`, and `data`.
  - Strict `@overload` type signatures for `.to_dict(orient=...)` ensuring 100% strict `pyright` type inference without caller `isinstance` casts.
  - Unboxing of `CellValue`: convert Serde-tagged variants (`Number`, `Text`, `Boolean`, `Empty`, `Error`) into native Python types (`float`, `str`, `bool`, `None`), handling both Serde unit-variant strings (`"Empty"`) and tagged dictionaries (`{"Empty": ...}`), with canonical Excel error string mapping (`#DIV/0!`, `#VALUE!`, `#REF!`, `#NUM!`, `#N/A`, `#NAME?`).
  - Table metadata and dimensions: `table.id`, `table.title`, `table.subtitle`, `table.sheet`, `table.columns`, `table.rows`, `table.shape`, and `len(table)`.
  - Cell lookups: `table.value(row, col)`, `table[row, col]`, `table.formula(row, col)`, `table.expr(row, col)`, `table.address(row, col)`, and `table.cell(row, col)` backed by pre-indexed $O(1)$ coordinate tuples.
  - Visual 2D rendering: `table.to_markdown(view="values" | "formulas" | "expr" | "addresses")` with aligned column widths, stacked multi-tier headers, descriptor row indentation, and section divider rows.
  - Re-export `TableAuditor` in `python/f8/__init__.py`.
  - Update type stubs in `python/f8/_core.pyi` and type contracts in `python/f8/types.py`.
  - Comprehensive unit test suite in `python/tests/test_audit.py`.

- **Out-of-Scope**:
  - Third-party library dependencies (no hard dependency on `pandas`, `polars`, `pyarrow`, or `tabulate`).
  - Terminal interactive TUI widgets or curses-based inspectors.
  - Modifying in-memory values (inspections are strictly read-only).

### What Changes

- **`python/f8/audit.py`**: New module implementing `TableAuditor`, canonical error mapping, and unboxing utility functions.
- **`python/f8/types.py`**: Add typed definitions for `CellScalarValue`, `TableOrient`, `TableViewMode`, and `SplitDict`.
- **`python/f8/_core.pyi`**: Add `table(self, table: str) -> TableAuditor` method signature to `CompiledModel`.
- **`python/f8/__init__.py`**: Bind `CompiledModel.table` and re-export `TableAuditor`.
- **`python/tests/test_audit.py`**: Test suite covering all orientations, unboxing, coordinate lookups, markdown rendering, and error handling.

### Capabilities

- **New capabilities**:
  - Direct dataframe ingestion via `pd.DataFrame(compiled.table("IncomeStatement").to_dict())` or `pl.DataFrame(...)`.
  - Direct row-keyed dictionary lookups via `compiled.table("IncomeStatement").to_dict("index")["revenue"]["2024E"]`.
  - Instant scalar querying via `table.value("revenue", "2024E")` and `table["revenue", "2024E"]`.
  - Dimension introspection via `table.shape` (`(n_rows, n_cols)`) and `len(table)`.
  - Formula auditing via `table.formula("revenue", "2024E")` returning lowered Excel formulas.
  - LLM-friendly 2D text inspection via `print(table.to_markdown())` and `print(table.to_markdown(view="formulas"))` with column alignment, stacked headers, and section dividers.

- **Modified capabilities**:
  - `CompiledModel` gains the `.table(table_id)` convenience accessor.

---

## Design

### Goals

1. **Zero External Dependencies**: All dictionary transformations and Markdown generation execute in standard Python 3.12+ with zero third-party packages.
2. **Universal Interoperability**: Dictionary schemas strictly follow de facto standards (`records`, `dict`, `index`, `split`) accepted out-of-the-box by Pandas, Polars, DuckDB, and standard Python scripts.
3. **Strict Python Typings**: Cell values are cleanly unboxed into primitive `float`, `int`, `str`, `bool`, or `None` instead of Serde enum wrappers, with complete `@overload` definitions for static analyzers (`pyright` in strict mode).
4. **Agent-First Ergonomics**: Table grids can be rendered directly to aligned Markdown strings for logging, debugging, and AI agent reasoning.

### Invariants

- **Coordinate Case-Sensitivity**: Table, row, and column IDs are strictly case-sensitive, matching the single source of truth in `f8-core` (`TableId`, `RowId`, `ColId`).
- **Clear Exception Boundaries**:
  - Looking up an unknown table via `compiled.table(table_id)` delegates to `compiled.inspect_table(table_id)` and raises `InspectionError`.
  - Looking up an unknown row or column coordinate on a `TableAuditor` via `.value(row, col)`, `.formula(row, col)`, etc., raises `KeyError(f"Coordinate ('{row}', '{col}') not found in table '{self.id}'")`.
  - Invoking `table[key]` with an invalid key type (e.g. non-tuple or wrong element count) raises `TypeError("TableAuditor coordinate indexing requires a (row, col) tuple, e.g. table['revenue', '2024E']")`.
- **Structural Grid Filtering in Tabular Export (`to_dict`)**:
  - Blank spacing gutter columns (`is_gutter: True`) and gutter rows are omitted from tabular data export (`to_dict`) and from `table.columns` / `table.rows`.
  - Section header rows (`is_section_header: True`) contain no row IDs or calculable cell coordinates in `f8-core` IR; they are excluded from tabular cell data export (`to_dict`) and from `table.rows`.
  - `table.columns` returns an ordered list of non-gutter column IDs (`list[str]`).
  - `table.rows` returns an ordered list of non-gutter, non-section data row IDs (`list[str]`).
- **Strict Rectangular Tabular Matrix Guarantee**:
  - In `to_dict()` across all orientations (`records`, `dict`, `index`, `split`), every data row in `table.rows` is guaranteed to contain entries for 100% of the columns in `table.columns`. Any unpopulated or empty cell coordinate defaults to `None`. Downstream consumers (`pandas.DataFrame`, `polars.DataFrame`, matrix algorithms) are guaranteed uniform rectangular dimensions.
- **Markdown Rendering Presentation (`to_markdown`)**:
  - Gutter rows and gutter columns are skipped.
  - Section header rows are rendered as full-width bold divider rows (`| **{title}** | | ... |`), matching CLI table presentation.
  - Descriptor columns preserve line-item label and row indentation (`"  " * indent`) across all view modes (`values`, `formulas`, `expr`, `addresses`), displaying `{label} [{id}]` when label text differs from row ID to match CLI parity.
  - Multi-tier column headers produce stacked header rows matching CLI markdown format, with bottom headers displaying `{header} [{id}]` if header text differs from column ID.
  - In `view="values"`, cells render `cell.get("formatted_value")` if present; if not yet available, fallback to clean unboxed scalar strings.
- **Read-Only Purity**: Auditing operations never mutate compiler state, cell values, or underlying layout.

### Decisions

- **Decision 1: Pure Python Implementation vs Rust PyO3 Methods**:
  - _Rationale_: `TableInspection` already transfers 100% of table cells, columns, rows, formulas, and addresses across the C-ABI in microseconds. Implementing `TableAuditor`, `.to_dict()`, and `.to_markdown()` in pure Python keeps Rust bindings simple, makes formatting logic easily extensible, and allows dynamic duck-typing without recompiling native binaries.
  - _Alternatives considered_: Implementing `.to_dict()` inside `crates/f8-python/src/model.rs`. Rejected because generating dynamic Python dictionaries and list-of-dicts inside Rust PyO3 is verbose, rigid, and tightly couples data structure shaping to Rust.

- **Decision 2: Comprehensive Dictionary Orientations (`records`, `dict`, `index`, `split`)**:
  - _Rationale_: Modern data engines consume distinct formats:
    - Pandas/DuckDB thrive on `records` (`list[dict[str, Any]]`).
    - Polars/Arrow prefer columnar `dict` (`dict[str, list[Any]]`).
    - Python modeling scripts thrive on `index` (`dict[str, dict[str, Any]]` for direct `data["row"]["col"]` access).
    - Matrix algorithms prefer `split` (`columns`, `index`, `data`).
  - _Choice_: Support `records` as the default, with `dict`, `index`, and `split` options.

- **Decision 3: Unboxed Scalar Typing (`CellScalarValue`)**:
  - _Rationale_: Serde serializes Rust's `CellValue` data-carrying variants as tagged dictionaries (`{"Number": 100.0}`, `{"Text": "Rev"}`, `{"Error": "Div0"}`), and unit variants as strings (`"Empty"`). Callers want primitive `100.0`, `"Rev"`, or `None`.
  - _Choice_: Unbox variants automatically, handling both unit-variant strings (`"Empty"`) and tagged dictionaries:
    - `Number` $\to$ `float` (consistent floating-point scalar type to preserve uniform DataFrame column dtypes).
    - `Text` $\to$ `str`.
    - `Boolean` $\to$ `bool`.
    - `Empty` $\to$ `None`.
    - `Error` $\to$ canonical `#<CODE>` string mapped via:
      - `"Div0"` $\to$ `"#DIV/0!"`
      - `"Value"` $\to$ `"#VALUE!"`
      - `"Ref"` $\to$ `"#REF!"`
      - `"Num"` $\to$ `"#NUM!"`
      - `"Na"` $\to$ `"#N/A"`
      - `"Name"` $\to$ `"#NAME?"`

- **Decision 4: Single Source of Truth for Scalar Type Aliases**:
  - _Rationale_: `python/f8/types.py` already defines `CellValueLiteral = float | int | str | bool | None`.
  - _Choice_: Alias `type CellScalarValue = CellValueLiteral` in `python/f8/types.py` to preserve single-source-of-truth integrity while establishing domain vocabulary for auditing outputs.

- **Decision 5: Strict `@overload` Signatures for `.to_dict()`**:
  - _Rationale_: Union return types force callers using strict static analyzers (`pyright`) to add unnecessary narrowing boilerplate.
  - _Choice_: Explicitly overload `.to_dict()` for each `TableOrient` literal variant.

- **Decision 6: Pre-indexed Coordinate Map for $O(1)$ Cell Lookups**:
  - _Rationale_: `TableInspection.cells` is keyed by canonical string coordinate (`TABLE["row", "col"]`). Formatting string coordinates dynamically on every `.value(row, col)`, `.formula(row, col)`, and `table[row, col]` lookup incurs continuous string allocation overhead.
  - _Choice_: In `TableAuditor.__init__`, pre-index cells into an internal `dict[tuple[str, str], TableCellInfo]` mapping. Yields instantaneous $O(1)$ lookups with zero per-lookup string formatting allocations.

- **Decision 7: Decoupling Accounting Formatting via Companion Spec**:
  - _Rationale_: To achieve 100% parity with CLI accounting typography (`$ 1,200.0`, `15.0%`, `(50.0)`), `TableCellInfo` in `f8-core` will be extended with `formatted_value: Option<String>` (populated by `format_cell_value`) in a focused companion spec (`specs/drafts/2026-09-28-table-cell-formatted-value.md`).
  - _Choice_: In `TableAuditor.to_markdown(view="values")`, read `cell.get("formatted_value")` if present; if not yet populated, gracefully fall back to unboxed scalar string representation (`str(val)`). This decouples the pure Python SDK auditing increment while making it immediately forward-compatible with companion compiler updates.

### Data Contracts

#### Type Signatures (`python/f8/types.py`)

```python
type CellScalarValue = CellValueLiteral  # float | int | str | bool | None
type TableOrient = Literal["records", "dict", "index", "split"]
type TableViewMode = Literal["values", "formulas", "expr", "addresses"]

class SplitDict(TypedDict):
    columns: list[str]
    index: list[str]
    data: list[list[CellScalarValue]]
```

#### TableAuditor Class Interface (`python/f8/audit.py`)

```python
class TableAuditor:
    def __init__(self, raw: TableInspection) -> None: ...
    
    @property
    def id(self) -> str: ...
    
    @property
    def title(self) -> str: ...
    
    @property
    def subtitle(self) -> str | None: ...
    
    @property
    def sheet(self) -> str: ...
    
    @property
    def columns(self) -> list[str]: ...
    
    @property
    def rows(self) -> list[str]: ...
    
    @property
    def shape(self) -> tuple[int, int]: ...
    
    def __len__(self) -> int: ...
    
    def cell(self, row: str, col: str) -> TableCellInfo: ...
    def value(self, row: str, col: str) -> CellScalarValue: ...
    def formula(self, row: str, col: str) -> str | None: ...
    def expr(self, row: str, col: str) -> str | None: ...
    def address(self, row: str, col: str) -> str: ...
    
    def __getitem__(self, key: tuple[str, str]) -> CellScalarValue: ...
    
    @overload
    def to_dict(self, orient: Literal["records"] = "records") -> list[dict[str, CellScalarValue]]: ...
    @overload
    def to_dict(self, orient: Literal["dict"]) -> dict[str, list[CellScalarValue]]: ...
    @overload
    def to_dict(self, orient: Literal["index"]) -> dict[str, dict[str, CellScalarValue]]: ...
    @overload
    def to_dict(self, orient: Literal["split"]) -> SplitDict: ...
    def to_dict(
        self,
        orient: TableOrient = "records",
    ) -> (
        list[dict[str, CellScalarValue]]
        | dict[str, list[CellScalarValue]]
        | dict[str, dict[str, CellScalarValue]]
        | SplitDict
    ): ...
    
    def to_markdown(
        self,
        view: TableViewMode = "values",
    ) -> str: ...
```

---

## Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Table Accessor on CompiledModel**
  - **Given** a compiled model with table `"IncomeStatement"`.
  - **When** calling `compiled.table("IncomeStatement")`.
  - **Then** returns a `TableAuditor` instance populated with table columns, rows, and cell records.
  - **When** calling `compiled.table("NonExistent")`.
  - **Then** raises `InspectionError` indicating the table does not exist.

- [ ] **Scenario 2: Unboxed Scalar Cell Lookups**
  - **Given** a `TableAuditor` containing literal inputs, evaluated formulas, empty cells, and division by zero.
  - **When** calling `table.value("revenue", "2024E")` or `table["revenue", "2024E"]`.
  - **Then** returns unboxed primitive `float` (e.g. `115.0`) rather than a tagged dictionary.
  - **When** calling `table.formula("revenue", "2024E")`.
  - **Then** returns the lowered Excel formula string (e.g. `"=C20*(1+$C$7)"`).
  - **When** querying a literal driver cell with `table.formula("revenue", "2023A")`.
  - **Then** returns `None`.
  - **When** querying an empty cell with Serde unit string `"Empty"`.
  - **Then** returns `None`.
  - **When** querying an evaluated `#DIV/0!` cell.
  - **Then** returns `"#DIV/0!"`.
  - **When** calling `table.value("unknown_row", "2024E")`.
  - **Then** raises `KeyError` indicating the coordinate was not found.
  - **When** invoking coordinate indexing with a non-tuple key (e.g. `table["revenue"]`).
  - **Then** raises `TypeError` indicating a 2-tuple `(row, col)` is required.

- [ ] **Scenario 3: Tabular Export with Records Orientation & Rectangular Guarantee**
  - **Given** a `TableAuditor` with descriptor column `"line_item"`, measure columns `"2023A"`, `"2024E"`, and an unassigned cell.
  - **When** calling `table.to_dict(orient="records")`.
  - **Then** returns a list of dictionaries where each entry corresponds to a calculable row with column keys mapped to unboxed scalar values.
  - **And** blank gutter columns and visual section header rows are excluded.
  - **And** every row dictionary contains 100% of `table.columns` keys, with unassigned cells mapped to `None`.

- [ ] **Scenario 4: Tabular Export with Columnar Dict Orientation**
  - **Given** a `TableAuditor` with 3 data rows and 2 measure columns.
  - **When** calling `table.to_dict(orient="dict")`.
  - **Then** returns a dictionary mapping each non-gutter column ID to an ordered list of cell scalar values matching row order and length.

- [ ] **Scenario 5: Tabular Export with Index Orientation**
  - **Given** a `TableAuditor` with rows `"revenue"`, `"cogs"` and columns `"line_item"`, `"2024E"`.
  - **When** calling `table.to_dict(orient="index")`.
  - **Then** returns a nested dictionary mapping each row ID to a dictionary of column IDs and unboxed scalar values.

- [ ] **Scenario 6: Tabular Export with Split Orientation**
  - **Given** a `TableAuditor`.
  - **When** calling `table.to_dict(orient="split")`.
  - **Then** returns a `SplitDict` with `"columns"`, `"index"` (data row IDs), and 2D `"data"` matrix with uniform rectangular dimensions `len(index) x len(columns)`.

- [ ] **Scenario 7: Visual Markdown Grid Rendering**
  - **Given** a `TableAuditor` with multi-tier headers, section headers, descriptor column with row indentation, and measure columns.
  - **When** calling `table.to_markdown(view="values")`.
  - **Then** returns an aligned GitHub-Flavored Markdown table with space-padded columns, stacked column header tiers, left-aligned descriptor headers, right-aligned measure headers, section header divider rows, and evaluated values (using `formatted_value` if available, or unboxed scalar string).
  - **And** descriptor column cells preserve line item labels and row indentation (`"  " * indent`) across all view modes.
  - **When** calling `table.to_markdown(view="formulas")`.
  - **Then** returns an aligned Markdown table displaying lowered Excel formulas for formula cells and literal values for inputs.

- [ ] **Scenario 8: Dimension and Metadata Introspection**
  - **Given** a `TableAuditor` initialized from a table with title `"Performance"`, subtitle `"($ in Millions)"`, 5 data rows, and 4 non-gutter columns (1 descriptor and 3 measure columns).
  - **When** inspecting `table.title`, `table.subtitle`, `table.shape`, and `len(table)`.
  - **Then** `table.title` returns `"Performance"`.
  - **And** `table.subtitle` returns `"($ in Millions)"`.
  - **And** `table.shape` returns `(5, 4)`.
  - **And** `len(table)` returns `5`.

---

## Tasks

### Phase 1: Type Contracts & Unboxing Engine

- [ ] Add `CellScalarValue`, `TableOrient`, `TableViewMode`, and `SplitDict` to `python/f8/types.py`.
- [ ] Implement `unbox_cell_value` in `python/f8/audit.py` handling all `CellValue` variants (including bare `"Empty"` string and tagged dicts) and canonical `_EXCEL_ERROR_MAP`.

### Phase 2: TableAuditor Implementation

- [ ] Implement `TableAuditor` class in `python/f8/audit.py`:
  - Internal indexing: pre-index cells into `dict[tuple[str, str], TableCellInfo]` in `__init__` for $O(1)$ lookups.
  - Properties & dimensions: `id`, `title`, `subtitle`, `sheet`, `columns`, `rows`, `shape`, and `__len__()`.
  - Cell lookups: `cell()`, `value()`, `formula()`, `expr()`, `address()`, and `__getitem__()` with tuple type validation.
  - Export: `to_dict()` supporting `records`, `dict`, `index`, and `split` with strict `@overload` signatures and guaranteed rectangularity.
  - Rendering: `to_markdown()` supporting `values`, `formulas`, `expr`, and `addresses` with column alignment, stacked multi-tier headers, descriptor row indentation, and section dividers.
- [ ] Bind `.table(table_id: str)` on `CompiledModel` in `python/f8/__init__.py`.
- [ ] Update `python/f8/_core.pyi` with `table()` signature and re-export `TableAuditor`.

### Phase 3: Comprehensive Test Suite & Verification

- [ ] Create `python/tests/test_audit.py`:
  - Test `compiled.table()` lookup and error handling on unknown table (`InspectionError`).
  - Test scalar unboxing across numbers, text, booleans, errors (`#N/A`, `#DIV/0!`), and empty cells (`"Empty"` string and dict).
  - Test `.to_dict()` across `records`, `dict`, `index`, and `split` including rectangular matrix guarantee.
  - Test `.value()`, `table[row, col]`, `.formula()`, `.expr()`, `.address()`, and `.cell()`.
  - Test `KeyError` handling on non-existent cell coordinates and `TypeError` on non-tuple indexing.
  - Test `table.subtitle`, `table.shape`, and `len(table)`.
  - Test `.to_markdown()` across all four view modes (`values`, `formulas`, `expr`, `addresses`) including multi-tier headers and descriptor indentation.
  - Test end-to-end compatibility with fixture templates (`three-statement`, `lbo`).
- [ ] Run full project verification:
  - `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
  - `uv run ruff check . && uv run ruff format --check . && uv run pyright && uv run pytest`

### Phase 4: Living Spec Reconciliation

- [ ] Update `specs/living-specs/interfaces/python-sdk.md` with table inspection contracts and `.to_dict()` schemas.
- [ ] Archive completed draft to `specs/history/`.
