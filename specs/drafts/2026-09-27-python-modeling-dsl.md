---
title: Python Modeling DSL & In-Memory Compilation
date: 2026-09-27
status: draft
target-living-spec: specs/living-specs/interfaces/python-sdk.md
---

## Proposal

### What

Implement the pure Python authoring DSL (`f8.Model`, `Sheet`, `Table`, `Cell`) and direct in-memory compilation pipeline. This introduces a fluent, strictly typed authoring interface in Python that translates model definitions into Serde-compatible inputs for `f8._core.compile()`, emits production `.xlsx` workbooks directly to disk, and provides optional multi-file JSON export (`f8.json` + `tables/*.json`).

### Why

The native PyO3 extension (`crates/f8-python` / `f8._core`) provides low-level compilation primitives, but requires manually constructing nested dictionary specifications matching `ProjectManifest` and `TableSpec`. 

Financial analysts and AI agents need an ergonomic, type-safe Python API that allows defining financial models as software—using variables for assumptions, loops for projection horizons, canonical formula expressions, and point cell overrides—with zero boilerplate and instant IDE autocomplete under strict type checking (`pyright`).

The Python DSL maintains full feature parity with the underlying `f8-core` engine (including column and row spacing gutters, multi-tier headers, cell-level indentation, and notes) while strictly avoiding implicit heuristics or temporal declaration order dependencies.

### Scope

- **In-Scope**:
  - Pure Python builder classes: `Model`, `Sheet`, `Table`, and `Cell` with strict PEP 561 / `pyright` type hints.
  - Symmetrical table authoring methods: `add_column`, `add_column_gutter`, `add_row`, `add_row_gutter` (with `add_gutter` alias), `add_section_header`, and `set_cell`.
  - Global and sheet-level table access: `sheet.get_table(id)` and `model.get_table(id)` for globally unique table lookups.
  - Strict 0D cell authoring (`cells={col_id: value | Cell}`) with cell-level indentation and audit notes.
  - Full parity with Rust engine AST: column gutters (`ColumnSpec::Gutter`), row gutters (`RowSpec::Gutter`), multi-tier headers, and freeze panes (`(row, col)`).
  - Direct in-memory compilation: `model.compile() -> CompiledModel`.
  - Ergonomic workbook persistence: `CompiledModel.save(path, options=None)` accepting both `str` and `os.PathLike` / `pathlib.Path` with automatic parent directory creation.
  - Optional multi-file JSON export: `model.export_project(dir_path)` writing canonical `f8.json` and `tables/<id>.json`.
  - Comprehensive unit test suite covering builder construction, validation, compilation, `.xlsx` saving, and JSON export.

- **Out-of-Scope**:
  - Ingestion / compilation of existing JSON projects in Python (`f8.compile_project(dir)`): Python is the authoring source of truth; disk-based JSON compilation is natively handled by `f8-cli`.
  - Third-party dataframe conversions (e.g. `pandas.DataFrame` wrappers): Tabular ingestion is accomplished via standard Python iterables and dictionaries; dictionary export protocols belong to Phase 3.
  - Web or TypeScript bindings.

### What Changes

- **`python/f8/builder.py`**: New module implementing `Cell`, `Model`, `Sheet`, and `Table` builder classes with global table lookup and sentinel-based cell validation.
- **`python/f8/types.py`**: Add typed literal definitions for `AxisRole`, `FormatPreset`, `SemanticStyle`, `CurrencyCode`, and `CellInputValue` union.
- **`python/f8/__init__.py`**: Re-export `Cell`, `Model`, `Sheet`, `Table`, and new type definitions.
- **`crates/f8-python/src/model.rs`**: Add `save` method to `PyCompiledModel` accepting `str` or `Path` for writing workbook bytes to disk.
- **`python/f8/_core.pyi`**: Add `save(path: str | Path, options: EmissionOptions | None = None) -> None` signature to `CompiledModel` type stub.
- **`python/tests/test_dsl.py`**: Comprehensive test suite verifying the fluent modeling DSL.

### Capabilities

- **New capabilities**:
  - Author complete financial models programmatically in Python using `Model`, `Sheet`, `Table`, and `Cell`.
  - Retrieve tables directly by identifier via `model.get_table("table_id")` or `sheet.get_table("table_id")`.
  - Declare column gutters (`add_column_gutter()`) and row gutters (`add_row_gutter()`) for visual grid spacing.
  - Configure worksheet freeze panes using explicit 0-indexed `(row, col)` split coordinates.
  - Compile models directly in memory with a single call (`model.compile()`).
  - Save production `.xlsx` workbooks directly to disk with `compiled.save("output.xlsx")` or `compiled.save(Path("output.xlsx"))`.
  - Export in-memory models to canonical multi-file JSON directories via `model.export_project("./project_dir")`.
  - Full IDE autocomplete and strict static type checking for column roles, formatting presets, semantic styles, currency codes, and cell inputs.

- **Modified capabilities**:
  - `CompiledModel` gains `save(path, options=None)` to persist `.xlsx` binaries directly to a filesystem path (`str` or `Path`) with automatic parent directory creation outside the Python GIL.

---

## Design

### Goals

1. **Idiomatic & Fluent**: Provide an intuitive API where financial statements can be authored with concise, readable code.
2. **Strictly Typed**: Zero bare strings for options; enforce `typing.Literal` contracts for all formats, styles, currencies, and roles under `pyright` strict mode.
3. **Single Source of Truth & Core Parity**: Python models serialize directly into canonical `f8-core` Serde contracts with complete coverage of all core capabilities.
4. **Zero Magic & Strict 0D Indentation**: Explicit cell inputs; zero temporal coupling between column and row declarations; reject implicit heuristics or phantom row-level label mappings.

### Invariants

- **Determinism**: Tables, columns, and rows maintain their exact insertion order in generated manifests and table specifications.
- **Purity**: Model authoring and in-memory compilation perform zero filesystem I/O. File I/O occurs only when explicitly invoking `compiled.save()` or `model.export_project()`.
- **Validation**: Schema errors, invalid references, and cyclic dependencies bubble up as structured `f8` domain exceptions (`CycleDetectedError`, `ReferenceResolutionError`, etc.).
- **Orthogonality**: Column definitions and row definitions are independent; rows can be declared before or after columns without runtime errors or state coupling.

### Decisions

- **Decision 1: Add `save()` to `PyCompiledModel`**:
  - *Rationale*: Allows users to write `compiled.save("output.xlsx")` directly without needing manual `open(..., "wb")` boilerplate. The native implementation normalizes paths, ensures parent directories exist, and writes bytes outside the Python GIL.
  - *Alternatives considered*: Implementing `save()` purely in Python using `Path(path).write_bytes(...)`. Native implementation matches `f8-cli` and keeps type stubs synchronized.

- **Decision 2: Strict 0D Cell Authoring & Elimination of Row-Level Label/Indent Magic**:
  - *Rationale*: Finx decoupled indentation from axis roles in [`cell-level-indentation-and-primary-descriptor-elimination.md`](../history/2026-09-21-cell-level-indentation-and-primary-descriptor-elimination.md). Indentation in Excel is an asymmetric 0D cell presentation attribute, not a 1D vector row attribute. Placing `label` and `indent` on `add_row` introduces several critical problems:
    1. Temporal order dependency: `add_row` would fail if called before defining a descriptor column.
    2. Ambiguity: Tables with multiple descriptor columns (e.g. `code` and `line_item`) have no clean way to know which column receives `label`.
    3. Conflicting state: Ambiguity arises when both `label` and `cells={"line_item": ...}` are provided.
    4. Inaccurate mental model: Falsely implies rows possess an indentation level.
  - *Choice*: Keep `add_row` strictly aligned with `DataRowSpec`. Row values are placed explicitly in `cells`. Indentation is declared on the cell itself via `Cell(value, indent=1)` or a dictionary override `{"value": value, "indent": 1}`.
  - *Section headers*: `add_section_header(title, indent=None, note=None)` retains `indent` because section headers span the entire table width without columns and map directly to `SectionHeaderRowSpec.indent`.

- **Decision 3: Symmetrical Column and Row Gutters**:
  - *Rationale*: The core engine provides both horizontal column spacing (`ColumnSpec::Gutter`) and vertical row spacing (`RowSpec::Gutter`).
  - *Choice*: Expose explicit `add_column_gutter()` on the column axis and `add_row_gutter()` on the row axis, with `add_gutter()` retained as a convenient row-gutter alias.

- **Decision 4: Omit Untyped Provenance Metadata (`source`)**:
  - *Rationale*: While `f8-core` historically retains an untyped `source: Option<serde_json::Value>`, in practice financial modeling citations belong in visible, first-class presentation locations:
    1. Table/Sheet level: `Table.subtitle` (e.g. `"Source: SEC 10-K, FactSet"`), which renders prominently in the table title banner.
    2. Section level: `Table.add_section_header("Operating Assumptions", note="Source: Guidance")`.
    3. Cell level: `Cell(..., note="Source: 10-K p. 42")` (which emits directly into native Excel hover comments and CLI inspection cards).
  - *Choice*: Untyped `source` dictionaries are not emitted to `.xlsx`, are not surfaced in `CompiledModel` or CLI inspection, and add unnecessary clutter to method signatures. The Python DSL omits `source`, adhering to pragmatic simplicity and establishing `note` and `subtitle` as the single sources of truth for citations.

- **Decision 5: Header Specification Mutual Exclusivity**:
  - *Rationale*: A column can have either a single header or a multi-tier hierarchy (`headers: Vec<String>`).
  - *Choice*: `add_column` accepts `header: str | None = None` and `headers: list[str] | None = None`. If both are provided, raise `ValueError`. If only `header` is given, it normalizes to `[header]`. If neither is given, it defaults to `[]`.

- **Decision 6: Omit `compile_project`**:
  - *Rationale*: YAGNI. Python users author models in Python code. Loading multi-file JSON projects is already natively handled by `f8-cli`.

- **Decision 7: Global Table Lookup on `Model`**:
  - *Rationale*: Table identifiers in `f8` are globally unique across the entire workbook. While tables belong to a sheet (`sheet.tables`), requiring callers to navigate through sheets to look up a table (`model.get_sheet(s).get_table(t)`) introduces unnecessary cognitive overhead when writing cross-table formulas or inspecting model state. `model.get_table(id)` searches all sheets and returns the matching table directly, while `sheet.get_table(id)` remains available for sheet-scoped access.

- **Decision 8: Strict Point Cell Override Semantics (`set_cell` & `Cell`)**:
  - *Rationale*: In `f8-core`, every cell entry in `row.cells` MUST specify either `value` or `expr` (`EmptyCellPayload` error otherwise). When calling `table.set_cell(row, col, ...)`:
    1. If the cell already exists in `row.cells`, omitting `value` and `expr` updates other presentation attributes (style, note, format, indent) while preserving the cell's existing payload.
    2. If creating a new cell override on a row that previously only had a vector formula, the caller MUST provide either `value` or `expr` (raising `ValueError` if neither or both are provided).
    3. If `row` does not exist in the table, raise `KeyError(f"Row '{row}' does not exist in table '{self.id}'")`.
    4. `Cell` uses an internal sentinel `_UNSET = object()` for `value` to distinguish `value=None` (explicit null) from omitted `value`, validating in `__post_init__` that either `value` or `expr` is provided.

- **Decision 9: Explicit Freeze Panes Coordinate Convention**:
  - *Rationale*: Spreadsheet coordinate systems often cause confusion between `(row, col)` vs `(col, row)` or `(x, y)`.
  - *Choice*: `freeze_panes: tuple[int, int]` is strictly documented and validated as `(row, col)` (0-indexed split coordinate: number of top frozen rows, number of left frozen columns), directly matching `rust_xlsxwriter::Worksheet::set_freeze_panes`.

### Data Contracts

#### Builder Classes & Helpers

```python
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path

from f8._core import CompiledModel
from f8.types import (
    AxisRole,
    CellInputValue,
    CurrencyCode,
    EmissionOptions,
    FormatPreset,
    ManifestInput,
    SemanticStyle,
    TableInput,
    ThemeInput,
)

_UNSET: object = object()

@dataclass(frozen=True)
class Cell:
    """Detailed 0D cell override specification."""

    value: object = _UNSET
    expr: str | None = None
    style: SemanticStyle | None = None
    highlight: bool | None = None
    format: FormatPreset | None = None
    decimals: int | None = None
    currency: CurrencyCode | None = None
    indent: int | None = None
    note: str | None = None

    def __post_init__(self) -> None:
        has_value = self.value is not _UNSET
        has_expr = self.expr is not None
        if has_value and has_expr:
            raise ValueError("Cell cannot specify both 'value' and 'expr'")
        if not has_value and not has_expr:
            raise ValueError("Cell must specify either 'value' or 'expr'")

class Model:
    def __init__(self, name: str = "Model", theme: str = "corporate_navy") -> None: ...
    
    def add_sheet(
        self,
        name: str,
        tab_color: str | None = None,
        show_gridlines: bool = True,
        freeze_panes: tuple[int, int] | None = None,  # (row, col) 0-indexed split coordinate
    ) -> Sheet: ...
    
    def get_sheet(self, name: str) -> Sheet: ...

    def get_table(self, id: str) -> Table: ...

    @property
    def sheets(self) -> list[Sheet]: ...

    @property
    def tables(self) -> list[Table]: ...
    
    def compile(self, theme: ThemeInput | None = None) -> CompiledModel: ...
    
    def to_spec(self) -> tuple[ManifestInput, list[TableInput]]: ...
    
    def export_project(self, dir_path: str | Path) -> None: ...

class Sheet:
    def add_table(
        self,
        id: str,
        title: str,
        subtitle: str | None = None,
        currency: CurrencyCode | None = None,
        collapsible: bool = False,
    ) -> Table: ...
    
    def get_table(self, id: str) -> Table: ...

    @property
    def tables(self) -> list[Table]: ...

class Table:
    def add_column(
        self,
        id: str,
        role: AxisRole = "measure",
        header: str | None = None,
        headers: list[str] | None = None,
        style: SemanticStyle | None = None,
        highlight: bool | None = None,
        format: FormatPreset | None = None,
        decimals: int | None = None,
        currency: CurrencyCode | None = None,
        expr: str | None = None,
    ) -> Table: ...

    def add_column_gutter(self) -> Table: ...

    def add_row(
        self,
        id: str,
        cells: Mapping[str, CellInputValue] | None = None,
        expr: str | None = None,
        style: SemanticStyle | None = None,
        highlight: bool | None = None,
        format: FormatPreset | None = None,
        decimals: int | None = None,
        currency: CurrencyCode | None = None,
    ) -> Table: ...

    def add_row_gutter(self) -> Table: ...
    
    def add_gutter(self) -> Table: ...  # Alias for add_row_gutter

    def add_section_header(
        self,
        title: str,
        indent: int | None = None,
        note: str | None = None,
    ) -> Table: ...

    def set_cell(
        self,
        row: str,
        col: str,
        value: object = _UNSET,
        expr: str | None = None,
        style: SemanticStyle | None = None,
        highlight: bool | None = None,
        format: FormatPreset | None = None,
        decimals: int | None = None,
        currency: CurrencyCode | None = None,
        indent: int | None = None,
        note: str | None = None,
    ) -> Table: ...
```

### Risks & Trade-offs

- **Risk**: Mutual exclusivity of `header` and `headers` on `add_column`.
  - *Mitigation*: Raise `ValueError("Cannot specify both 'header' and 'headers'")` immediately if both arguments are provided.
- **Risk**: Cell override specifies both literal `value` and formula `expr`, or neither on a new cell override.
  - *Mitigation*: Validate in `Cell` and `set_cell`, raising `ValueError` early before serialization to provide immediate fail-fast developer feedback.
- **Risk**: Calling `set_cell` on an un-declared row.
  - *Mitigation*: Raise `KeyError(f"Row '{row}' does not exist in table '{self.id}'")` immediately.
- **Risk**: Duplicate sheet, table, column, or row identifiers.
  - *Mitigation*: Validate uniqueness immediately upon addition in Python builders to provide immediate fail-fast feedback before compilation.
- **Risk**: Ambiguity in `freeze_panes` coordinate conventions (row/col vs x/y).
  - *Mitigation*: Validate non-negative integer pairs and explicitly document `(row, col)` 0-indexed split ordering matching `rust_xlsxwriter`.

---

## Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: End-to-End Model Construction and Compilation**
  - **Given** a Python script constructing a `Model` with a sheet, columns, rows, and vector formulas.
  - **When** calling `model.compile()`.
  - **Then** compilation succeeds, returning a `CompiledModel` with correct cell counts and evaluated values.

- [ ] **Scenario 2: Workbook Saving to Disk with Parent Directory Creation**
  - **Given** a compiled model.
  - **When** calling `compiled.save("nested/dir/output.xlsx")`.
  - **Then** creates parent directories as needed and writes a valid `.xlsx` file starting with the `PK\x03\x04` zip magic header.

- [ ] **Scenario 3: Explicit 0D Cell Indentation via `Cell` and Dictionary Overrides**
  - **Given** a table with a descriptor column `line_item`.
  - **When** adding rows with `cells={"line_item": Cell("Cost of Goods Sold", indent=1)}` and `cells={"line_item": {"value": "Operating Expenses", "indent": 2}}`.
  - **Then** the serialized row specs contain `indent: 1` and `indent: 2` on the respective cell inputs, and compilation evaluates correctly.

- [ ] **Scenario 4: Column and Row Spacing Gutters**
  - **Given** a table where `add_column_gutter()` is placed between column groups and `add_row_gutter()` is placed between row groups.
  - **When** calling `model.to_spec()`.
  - **Then** the serialized table specification contains `{"kind": "gutter"}` entries in both `columns` and `rows`.

- [ ] **Scenario 5: Point Cell Override via `set_cell`**
  - **Given** an existing row with a vector formula and a row with an existing cell literal.
  - **When** calling `table.set_cell(row="rev", col="2023A", value=150.0, note="Audited", indent=0)` and updating an existing cell with `table.set_cell(row="existing", col="c1", note="Updated note")`.
  - **Then** the cell in `2023A` is overridden with literal value `150.0` and audit note `"Audited"`, and `existing["c1"]` retains its previous value while updating the note.
  - **And** calling `set_cell` on a non-existent row raises `KeyError`, and creating a new cell without `value` or `expr` raises `ValueError`.

- [ ] **Scenario 6: Project Export to Canonical Multi-File JSON**
  - **Given** a valid Python `Model` instance.
  - **When** calling `model.export_project(dir_path)`.
  - **Then** generates `<dir_path>/f8.json` and `<dir_path>/tables/<table_id>.json` files formatted with indentation, adhering to core specification schemas.

- [ ] **Scenario 7: Header Specification Validation**
  - **Given** a table builder.
  - **When** calling `table.add_column("rev", header="Revenue", headers=["Financials", "Revenue"])`.
  - **Then** raises `ValueError` indicating `header` and `headers` are mutually exclusive.

- [ ] **Scenario 8: Duplicate Identifier Validation**
  - **Given** an existing sheet, table, column, or row.
  - **When** attempting to add an entity with a duplicate ID.
  - **Then** raises `ValueError` with entity details before reaching compilation.

- [ ] **Scenario 9: Global Table Lookup and Iteration**
  - **Given** a model with multiple sheets and tables.
  - **When** calling `model.get_table("income_statement")` and inspecting `model.tables`.
  - **Then** resolves the table directly across sheets, and `model.tables` lists all tables in order.

- [ ] **Scenario 10: Cell Payload Validation and Sentinel Nulls**
  - **Given** cell definitions using `Cell`.
  - **When** creating `Cell(value=None)` (explicit null) and `Cell(expr="PREV * 1.05")`.
  - **Then** both construct valid specs without errors.
  - **And** attempting `Cell()` with neither value nor expr, or specifying both, raises `ValueError`.

- [ ] **Scenario 11: Worksheet Freeze Panes Split Coordinates**
  - **Given** a sheet configured with `freeze_panes=(4, 2)`.
  - **When** calling `model.to_spec()`.
  - **Then** the manifest sheet spec contains `{"freeze_panes": {"row": 4, "col": 2}}`, preserving 0-indexed split coordinates.

---

## Tasks

### Phase 1: Native & Type Foundations
- [ ] Add `save(path, options=None)` method to `PyCompiledModel` in `crates/f8-python/src/model.rs` accepting `str` and `Path` with automatic parent directory creation.
- [ ] Update `python/f8/_core.pyi` type stub with `CompiledModel.save(path: str | Path, options: EmissionOptions | None = None) -> None`.
- [ ] Add `AxisRole`, `FormatPreset`, `SemanticStyle`, `CurrencyCode`, and `CellInputValue` type definitions in `python/f8/types.py`.

### Phase 2: Python Builder Implementation
- [ ] Implement `Cell` dataclass helper with `_UNSET` sentinel and payload validation in `python/f8/builder.py`.
- [ ] Implement `Table` builder with `add_column`, `add_column_gutter`, `add_row`, `add_row_gutter`, `add_gutter`, `add_section_header`, and `set_cell` in `python/f8/builder.py`.
- [ ] Implement `Sheet` container with `add_table`, `get_table`, and `tables` property in `python/f8/builder.py`.
- [ ] Implement `Model` container with `add_sheet`, `get_sheet`, `get_table`, `sheets`, `tables`, `to_spec`, `compile`, and `export_project` in `python/f8/builder.py`.
- [ ] Re-export `Cell`, `Model`, `Sheet`, `Table`, and `CellInputValue` in `python/f8/__init__.py`.

### Phase 3: Verification & Test Suite
- [ ] Add comprehensive test suite in `python/tests/test_dsl.py` verifying all acceptance scenarios.
- [ ] Verify complete project suite:
  - `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
  - `uv run ruff check . && uv run ruff format --check . && uv run pyright && uv run pytest`

### Phase 4: Reconciliation & Documentation
- [ ] Update living spec `specs/living-specs/interfaces/python-sdk.md` with DSL builder contracts and `.save()`.
- [ ] Archive completed draft spec to `specs/history/`.
