---
title: Inspection Ergonomics, Unified Currency Typography, and Report Markdown Rendering
date: 2026-10-14
status: completed
---

## Proposal

### Problem
Real-world financial reporting consumers (such as Quarto deliverables and multi-scenario valuation scripts) expose five concrete gaps in `f8`'s current inspection and auditing surface:
1. **Inconsistent currency spacing**: `format_accounting_number` in `crates/f8-core/src/value/format.rs` inserts a space after the currency symbol (`"$ 1,234.56"`, `"($ 42.75)"`), diverging from the OpenXML Excel format mask (`"{sym}#,##0{dec}_);({sym}#,##0{dec});\"-\"_)"`) in `Theme::default()` and forcing downstream report authors to strip `"$ "` on every cell and table read.
2. **Missing semantic row/column style in `TableInspection`**: `RowInspection` and `ColumnInspection` in `crates/f8-core/src/compiler/compiled/model.rs` omit `style: Option<SemanticStyle>` (`"data" | "subtotal" | "total"`), causing downstream Markdown renderers that check `row.get("style") in ("subtotal", "total")` to silently fail to bold summary rows.
3. **High-friction and high-overhead scalar lookups**: Extracting a single numeric or formatted value from `CompiledModel` requires calling `inspect_cell(t, r, c)` (which traverses precedent/dependent edges in `DependencyGraph`, formats all neighbor cells, and serializes a 16-field dict via `pythonize`), importing `unbox_cell_value`, and narrowing `CellScalarValue` with `isinstance(..., (int, float))` to satisfy strict `pyright`. `TableAuditor` similarly lacks `.num(r, c)` and `.formatted(r, c, unit="")`.
4. **No 1D vector extraction on `TableAuditor`**: Plotting timelines and computing multi-period metrics require repeated list comprehensions (`[num(t, r, p) for p in periods]`) because `TableAuditor` only provides 0D `table[r, c]` and full-table `to_dict()`.
5. **`TableAuditor.to_markdown()` is incompatible with deliverable publishing**:
   - Unconditionally appends `[{col_id}]` and `[{row_id}]` debug suffixes to headers and row labels.
   - Emits multiple `| ... |` rows above the `| :--- |` alignment separator when `max_tiers > 1`, which is invalid GitHub-Flavored Markdown (GFM) and fails to parse in Pandoc and Quarto `pipe_tables`.
   - Lacks `rows` and `cols` subsetting or `include_section_headers` control.
   - Does not bold `subtotal`/`total` rows, does not render `"—"` for empty measure cells in `view="values"`, and does not implement IPython's `_repr_markdown_()` protocol.

### Solution
1. **Unify currency typography**: Remove the space after `{sym}` in `format_accounting_number` (`"$1,234.56"`, `"($1,234.56)"`, `"€115"`), matching Excel number format masks across all inspection surfaces.
2. **Expose `style` on `RowInspection` and `ColumnInspection`**: Populate `pub style: Option<SemanticStyle>` on `RowInspection` and `ColumnInspection` in `f8-core` and synchronize `python/f8/types.py`.
3. **Add fast-path scalar and formatted accessors on `CompiledModel` and `TableAuditor`**:
   - Add `CompiledModel::cell_value` and `CompiledModel::cell_formatted` in `f8-core` ($O(1)$ lookup without graph traversal).
   - Expose `compiled.value(target, row=None, col=None) -> CellScalarValue`, `compiled.num(target, row=None, col=None) -> float`, and `compiled.formatted(target, row=None, col=None, *, unit="") -> str` (plus native `compiled.table(table)`) on `PyCompiledModel` in `crates/f8-python/src/model.rs`, eliminating the `# type: ignore[method-assign]` patch in `python/f8/__init__.py`.
   - Add `auditor.num(row, col) -> float` and `auditor.formatted(row, col, *, unit="") -> str` on `TableAuditor`, splicing `unit` inside closing `")"` for negative accounting numbers (`"($42.8M)"`).
4. **Add 1D vector accessors on `TableAuditor`**: Expose `auditor.measure_columns -> list[str]`, `auditor.row_values(row, cols=None) -> list[CellScalarValue]`, and `auditor.row_nums(row, cols=None) -> list[float]`.
5. **Upgrade `TableAuditor.to_markdown()` for report publishing**:
   - Introduce `MarkdownTable(str)` implementing `_repr_markdown_(self) -> str`, returned by `TableAuditor.to_markdown()` (and hook `TableAuditor._repr_markdown_()`).
   - Flatten multi-tier column headers into a single valid GFM header row joined with `": "`.
   - Add `rows`, `cols` (automatically retaining descriptor columns), `include_section_headers`, and `show_ids: bool = False` parameters.
   - Bold `subtotal` and `total` rows and render `"—"` for empty measure cells when `view == "values"`.

### Scope
- **In-scope**:
  - `crates/f8-core/src/value/format.rs`: Remove space after `{sym}` in `format_accounting_number`.
  - `crates/f8-core/src/compiler/compiled/model.rs` & `mod.rs`: Add `style: Option<SemanticStyle>` to `ColumnInspection` and `RowInspection`; add `CompiledModel::cell_value` and `CompiledModel::cell_formatted`.
  - `crates/f8-python/src/model.rs`: Add `value`, `num`, `formatted`, and `table` methods to `PyCompiledModel`.
  - `python/f8/audit.py`: Add `MarkdownTable`, `format_with_unit`, `TableAuditor.measure_columns`, `num`, `formatted`, `row_values`, `row_nums`, `_repr_markdown_`, and enhanced `to_markdown`.
  - `python/f8/__init__.py`, `python/f8/_core.pyi`, `python/f8/types.py`: Synchronize exports, stubs, and `TypedDict` contracts; remove `# type: ignore[method-assign]`.
  - `python/f8/skills/f8-modeling/references/auditing-and-repair.md` & `docs/architecture.md`: Synchronize documentation and skill reference.
  - Rust and Python test suites (`crates/f8-core/tests/`, `python/tests/`).
- **Out-of-scope**:
  - Modifying Pass 5 Excel column width estimation heuristics (`crates/f8-core/src/compiler/layout/mod.rs`).
  - Adding third-party dataframe or plotting library dependencies.

### Impacted components
- `crates/f8-core/src/value/format.rs`
- `crates/f8-core/src/compiler/compiled/model.rs`
- `crates/f8-core/src/compiler/compiled/mod.rs`
- `crates/f8-core/tests/value_format_test.rs` & `crates/f8-core/tests/compiled_model_test.rs`
- `crates/f8-python/src/model.rs`
- `python/f8/audit.py`
- `python/f8/types.py`
- `python/f8/_core.pyi`
- `python/f8/__init__.py`
- `python/f8/skills/f8-modeling/references/auditing-and-repair.md`
- `docs/architecture.md`
- `python/tests/test_audit.py`, `python/tests/test_core_binding.py`, `python/tests/test_e2e.py`

### Capabilities
- **New capabilities**:
  - Direct $O(1)$ scalar, float, and unit-suffixed formatted string queries on `CompiledModel` (`compiled.value`, `compiled.num`, `compiled.formatted`).
  - Direct float, unit-suffixed formatted string, and 1D row vector extraction on `TableAuditor` (`auditor.num`, `auditor.formatted`, `auditor.measure_columns`, `auditor.row_values`, `auditor.row_nums`).
  - Publication-ready GFM table rendering via `auditor.to_markdown(view="values", *, rows=None, cols=None, include_section_headers=None, show_ids=False)` returning `MarkdownTable(str)` with IPython/Quarto `_repr_markdown_()` support.
  - Row and column semantic style inspection (`style: "data" | "subtotal" | "total" | None`) in `TableInspection`.
- **Modified capabilities**:
  - `format_accounting_number` formats currency values without a space after the currency symbol (`"$115.0"`, `"($42.75)"`, `"€115"`).
  - `TableAuditor.to_markdown()` emits a single flattened GFM header row for multi-tier headers, defaults `show_ids` to `False`, bolds `subtotal`/`total` rows, and displays `"—"` for empty measure cells in `view="values"`.

---

## Design

### Goals & non-goals
- **Goals**:
  - Unify currency formatting across in-memory inspection (`formatted_value`) and Excel OpenXML emission (`$1,234.56`).
  - Provide zero-boilerplate, strictly typed (`float` and `str`) scalar and 1D vector accessors for financial reporting and charting.
  - Make `TableAuditor.to_markdown()` produce valid, publication-grade GitHub-Flavored Markdown out of the box for Quarto/Jupyter/Pandoc while retaining `show_ids=True` and `view="formulas" | "expr" | "addresses"` for agent debugging.
  - Eliminate the `# type: ignore[method-assign]` monkey-patch in `python/f8/__init__.py`.
- **Non-goals**:
  - Introducing domain-specific financial formulas (e.g., WACC/DCF bridges) or plotting routines into `f8`.
  - Mutating compiled model values post-compilation.

### Invariants
- **Single source of truth for cell formatting**: All currency, percentage, multiple, basis points, integer, and number string formatting originates from `f8_core::value::format_cell_value`.
- **Consistent coordinate addressing**: `compiled.value`, `compiled.num`, and `compiled.formatted` accept both 1-argument canonical coordinates (`'Table["row", "col"]'`) and 3-argument positional coordinates (`"Table", "row", "col"`), identical to `compiled.inspect_cell`.
- **Strict exception semantics**:
  - Unknown coordinates on `CompiledModel` methods (`value`, `num`, `formatted`, `inspect_cell`) raise `InspectionError`.
  - Unknown coordinates on `TableAuditor` methods (`value`, `num`, `formatted`, `row_values`, `row_nums`, `to_markdown(rows=..., cols=...)`) raise `KeyError`.
  - Non-numeric cells (text, boolean, empty/`None`, or Excel error) queried via `compiled.num(...)`, `auditor.num(...)`, or `auditor.row_nums(...)` raise `TypeError`.
- **Backwards-compatible `str` return type**: `MarkdownTable` subclasses `builtins.str`, preserving `isinstance(md, str) is True` and all string methods.

### Technical decisions
- **Decision 1: Implement `value`, `num`, `formatted`, and `table` natively on `PyCompiledModel` in `crates/f8-python/src/model.rs` backed by `CompiledModel::cell_value` / `cell_formatted` in `f8-core`**.
  - **Rationale**: `CompiledModel::inspect_cell` constructs a full 16-field `CellInspection` struct including `DependencyGraph` precedent/dependent lookups and `pythonize` dictionary serialization. Adding `cell_value` and `cell_formatted` to `f8-core` allows `PyCompiledModel` to return primitive Python `float`/`str`/`bool`/`None` directly from `self.inner.evaluated` and `self.inner.grid` in $O(1)$ time with zero graph traversal or dict allocation, and moving `table` to `PyCompiledModel` removes the `# type: ignore[method-assign]` monkey-patch in `python/f8/__init__.py`.
  - **Alternatives considered**: Implementing `value`, `num`, and `formatted` as pure-Python wrappers calling `self.inspect_cell(...)`. Rejected due to unnecessary $O(\text{deg}(v))$ neighbor formatting and `pythonize` serde-dict marshaling on every scalar read.
- **Decision 2: Unify currency formatting by removing the space after `{sym}` in `format_accounting_number`**.
  - **Rationale**: `Theme::default()` defines the Excel currency mask as `"{sym}#,##0{dec}_);({sym}#,##0{dec});\"-\"_)"` (no space after `$`). Removing the space in `format_accounting_number` (`"$1,234.56"`, `"($1,234.56)"`) establishes 100% parity between Excel emission, `inspect_cell`, `inspect_table`, and prose/Markdown reports.
  - **Alternatives considered**: Adding a `compact_currency: bool` toggle to `formatted()`. Rejected because maintaining two competing currency formats violates the single-source-of-truth principle.
- **Decision 3: Flatten multi-tier column headers in `TableAuditor.to_markdown()` with `": "` into a single header row**.
  - **Rationale**: Standard GFM and Pandoc/Quarto `pipe_tables` grammar permits only a single header row before the `| :--- | ---: |` alignment delimiter. Joining non-empty header tiers with `": "` for multi-tier measure columns (e.g., `["Historical Actuals", "2023A"]` $\to$ `"Historical Actuals: 2023A"`) preserves full tier context while producing 100% valid GFM pipe tables.
  - **Alternatives considered**: Emitting raw HTML `<table>` tags for multi-tier headers. Rejected because GFM pipe tables remain readable in plain text, terminal logs, LLM prompts, and Quarto/Pandoc alike.

### Data contracts

#### Rust Contracts (`crates/f8-core/src/compiler/compiled/model.rs` & `mod.rs`)

```rust
pub struct ColumnInspection {
    pub id: Option<ColId>,
    pub headers: Vec<String>,
    pub is_gutter: bool,
    pub is_descriptor: bool,
    pub style: Option<SemanticStyle>,
}

pub struct RowInspection {
    pub id: Option<RowId>,
    pub section_title: Option<String>,
    pub is_section_header: bool,
    pub is_gutter: bool,
    pub indent: u8,
    pub style: Option<SemanticStyle>,
}

impl CompiledModel {
    pub fn cell_value(
        &self,
        table: &TableId,
        row: &RowId,
        col: &ColId,
    ) -> Result<CellValue, InspectionError>;

    pub fn cell_formatted(
        &self,
        table: &TableId,
        row: &RowId,
        col: &ColId,
    ) -> Result<String, InspectionError>;
}
```

#### Python SDK Contracts (`python/f8/_core.pyi`, `python/f8/audit.py`, `python/f8/types.py`)

```python
class ColumnInspection(TypedDict):
    id: str | None
    headers: list[str]
    is_gutter: bool
    is_descriptor: bool
    style: SemanticStyle | None

class RowInspection(TypedDict):
    id: str | None
    section_title: str | None
    is_section_header: bool
    is_gutter: bool
    indent: int
    style: SemanticStyle | None

class MarkdownTable(str):
    def _repr_markdown_(self) -> str: ...

class CompiledModel:
    def value(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
    ) -> CellScalarValue: ...
    def num(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
    ) -> float: ...
    def formatted(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
        *,
        unit: str = "",
    ) -> str: ...
    def table(self, table: str) -> TableAuditor: ...

class TableAuditor:
    @property
    def measure_columns(self) -> list[str]: ...
    def num(self, row: str, col: str) -> float: ...
    def formatted(self, row: str, col: str, *, unit: str = "") -> str: ...
    def row_values(
        self,
        row: str,
        cols: Sequence[str] | None = None,
    ) -> list[CellScalarValue]: ...
    def row_nums(
        self,
        row: str,
        cols: Sequence[str] | None = None,
    ) -> list[float]: ...
    def to_markdown(
        self,
        view: TableViewMode = "values",
        *,
        rows: Sequence[str] | None = None,
        cols: Sequence[str] | None = None,
        include_section_headers: bool | None = None,
        show_ids: bool = False,
    ) -> MarkdownTable: ...
    def _repr_markdown_(self) -> str: ...
```

### Risks & mitigations
- **Risk**: Existing tests in `crates/f8-core/tests/` and `python/tests/` assert the old `"$ "` space in `formatted_value`.
- **Mitigation**: Update all currency formatting assertions across Rust and Python test suites and verify with `cargo test` and `uv run pytest`.
- **Risk**: `unit` suffix concatenation on negative accounting numbers (`"($42.8)"` + `unit="M"`) could produce `"($42.8)M"` if naively appended.
- **Mitigation**: Centralize suffix formatting so any non-empty formatted string ending with `")"` splices `unit` before the closing parenthesis (`"($42.8M)"`), while empty strings `""` remain `""` even when `unit` is provided.

---

### Acceptance criteria (Given / When / Then)

- [x] **Scenario 1: Unified Currency Typography Without Internal Space**
  - **Given** a cell with `format="currency"`, `currency="USD"`, `decimals=2`, and values `1234.56` and `-42.75`
  - **When** formatted via `format_cell_value`, `inspect_cell`, `inspect_table`, `compiled.formatted`, or `auditor.formatted`
  - **Then** positive values format as `"$1,234.56"` and negative values format as `"($42.75)"` (and `"€115"`, `"£115"` for EUR/GBP with `decimals=0`).

- [x] **Scenario 2: Row and Column Semantic Style Inspection**
  - **Given** a compiled table containing data rows with `style="data"`, `style="subtotal"`, and `style="total"`, plus section headers and gutters
  - **When** `compiled.inspect_table(table_id)` is called
  - **Then** `RowInspection.style` equals `"data"`, `"subtotal"`, and `"total"` respectively for data rows, and `None` for section header and gutter rows; `ColumnInspection.style` equals `"data"` (or configured column style) for data columns and `None` for gutter columns.

- [x] **Scenario 3: Fast-Path `CompiledModel` Scalar, Float, and Formatted Lookups**
  - **Given** a compiled model with numeric, text, empty, and error cells
  - **When** calling `compiled.value`, `compiled.num`, and `compiled.formatted` using either 1-arg (`'Table["row", "col"]'`) or 3-arg (`"Table", "row", "col"`) coordinates
  - **Then** `compiled.value` returns the unboxed `CellScalarValue` (`float`, `str`, `bool`, `None`, or `#DIV/0!`), `compiled.num` returns `float` for numeric cells and raises `TypeError` for non-numeric/empty/error cells, `compiled.formatted(..., unit="M")` returns `"$115.0M"` for `115.0` and `"($50.0M)"` for `-50.0` (and `""` for empty cells), and unknown coordinates raise `InspectionError`.

- [x] **Scenario 4: `TableAuditor` Scalar & 1D Vector Accessors (`num`, `formatted`, `measure_columns`, `row_values`, `row_nums`)**
  - **Given** a `TableAuditor` with a descriptor column `"line_item"` and measure columns `["2023A", "2024E", "2025E"]`
  - **When** querying `auditor.measure_columns`, `auditor.num(row, col)`, `auditor.formatted(row, col, unit="M")`, `auditor.row_values(row)`, and `auditor.row_nums(row, cols=["2023A", "2024E"])`
  - **Then** `measure_columns` returns `["2023A", "2024E", "2025E"]`, `row_values("revenue")` returns `[100.0, 115.0, 132.25]`, `row_nums("revenue", cols=["2023A", "2024E"])` returns `[100.0, 115.0]`, `row_nums` on a row containing `None` or error raises `TypeError`, and unknown row/col IDs raise `KeyError`.

- [x] **Scenario 5: Publication-Ready `TableAuditor.to_markdown()` and `_repr_markdown_()`**
  - **Given** a `TableAuditor` with multi-tier headers (`["Historical Actuals", "2023A"]`), section headers, indented descriptor cells, a `style="total"` row, and an unassigned measure cell
  - **When** calling `md = auditor.to_markdown(view="values")`
  - **Then** `md` is an instance of `MarkdownTable` (and `str`) implementing `_repr_markdown_()`, emits a single valid GFM header row with flattened multi-tier headers (`"Historical Actuals: 2023A"`), omits `[row_id]` and `[col_id]` brackets by default (`show_ids=False`), bolds all non-empty cells in `subtotal`/`total` rows (`**...**`), and renders `"—"` for empty measure cells.
  - **When** calling `auditor.to_markdown(rows=["revenue", "net_income"], cols=["y2023a", "y2025e"])`
  - **Then** the descriptor column is automatically retained alongside `["y2023a", "y2025e"]`, only the requested rows are rendered, section headers are omitted by default when `rows` is specified (unless `include_section_headers=True`), and unknown row or column IDs in `rows`/`cols` raise `KeyError`.
  - **When** calling `auditor.to_markdown(show_ids=True)`
  - **Then** headers and descriptor labels append `[{col_id}]` and `[{row_id}]` when display text differs from coordinate ID.

---

## Tasks

- [x] 1. **Unify currency typography in `f8-core`**: Update `format_accounting_number` in `crates/f8-core/src/value/format.rs` to format currency without a space after `{sym}` (`"{sym}{formatted_num}"` and `"({sym}{formatted_num})"`), and update Rust unit/integration tests (`crates/f8-core/src/value/format.rs`, `crates/f8-core/tests/value_format_test.rs`, `crates/f8-core/tests/compiled_model_test.rs`).
- [x] 2. **Expose semantic style & fast cell lookups in `f8-core`**: Add `pub style: Option<SemanticStyle>` to `ColumnInspection` and `RowInspection` in `crates/f8-core/src/compiler/compiled/model.rs` and populate in `CompiledModel::inspect_table` (`crates/f8-core/src/compiler/compiled/mod.rs`); implement `CompiledModel::cell_value` and `CompiledModel::cell_formatted` with unit tests in `crates/f8-core/tests/compiled_model_test.rs`.
- [x] 3. **Expose `value`, `num`, `formatted`, and `table` on `PyCompiledModel` in `f8-python`**: Implement `value`, `num`, `formatted`, and `table` in `crates/f8-python/src/model.rs`; remove the monkey-patched `_compiled_model_table` and `# type: ignore[method-assign]` from `python/f8/__init__.py`.
- [x] 4. **Enhance `TableAuditor` and type contracts in `python/f8/`**: Update `python/f8/types.py`, `python/f8/_core.pyi`, `python/f8/__init__.py`, and `python/f8/audit.py` to add `MarkdownTable`, `measure_columns`, `num`, `formatted`, `row_values`, `row_nums`, `_repr_markdown_`, and upgraded `to_markdown` (`rows`, `cols`, `include_section_headers`, `show_ids`, single-row GFM header flattening, `subtotal`/`total` bolding, and `"—"` empty measure display).
- [x] 5. **Update Python test suite & documentation**: Update existing currency assertions and add comprehensive tests for all new scenarios in `python/tests/test_audit.py`, `python/tests/test_core_binding.py`, and `python/tests/test_e2e.py`; synchronize `python/f8/skills/f8-modeling/references/auditing-and-repair.md` and `docs/architecture.md`.
- [x] 6. **Run full verification gate**: Execute `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `uv run maturin develop`, `uv run ruff check .`, `uv run ruff format --check .`, `uv run pyright`, and `uv run pytest`.
