---
title: Pre-formatted Value Synthesis in Table Inspection
date: 2026-09-28
status: completed
target-living-spec: specs/living-specs/compiler/compiled-model.md
---

## Proposal

### What

Extend `TableCellInfo` in `f8-core` with `pub formatted_value: Option<String>` pre-rendered via `format_cell_value(&value, format, decimals, currency)` during `CompiledModel::inspect_table`. Propagate this contract to outer interfaces:
1. Expose `formatted_value: str | None` on `TableCellInfo` in the Python SDK (`python/f8/types.py`).
2. Consume `cell.formatted_value` directly in the CLI table renderer (`crates/f8-cli/src/render/table.rs`), eliminating redundant on-the-fly formatting.
3. Provide instant access to canonical accounting-formatted strings for the pure Python `TableAuditor.to_markdown(view="values")` renderer defined in `specs/history/2026-09-28-python-inspection-protocols.md`.

### Why

In the current compiler inspection data model:
- `CellInspection` (`inspect_cell`), `SheetCellInspection` (`inspect_sheet`), and `CellNeighborInspection` (dependency trees) all pre-render `formatted_value: Option<String>` using `f8_core::value::format_cell_value`.
- `TableCellInfo` in `TableInspection` is currently the sole inspection structure lacking `formatted_value`, even though it stores `format`, `decimals`, and `currency`.
- As a consequence, `crates/f8-cli/src/render/table.rs` is forced to invoke `format_cell_value` on the fly for every cell during table rendering, and Python consumers would either have to duplicate the Rust formatting engine or display unformatted raw numbers.

Extending `TableCellInfo` with `formatted_value` establishes `format_cell_value` as the centralized single source of truth (SSOT) across all compiler inspection entrypoints.

### Scope

- **In-Scope**:
  - `crates/f8-core/src/compiler/compiled/model.rs`: Add `pub formatted_value: Option<String>` to `TableCellInfo`.
  - `crates/f8-core/src/compiler/compiled/mod.rs`: Populate `formatted_value` in `CompiledModel::inspect_table`.
  - `crates/f8-cli/src/render/table.rs`: Update `get_cell_text` to read `cell.formatted_value` directly.
  - `python/f8/types.py`: Add `formatted_value: str | None` to `TableCellInfo` TypedDict.
  - Core Rust unit tests in `crates/f8-core/tests/` and CLI rendering tests in `crates/f8-cli/tests/`.
  - Living spec reconciliation in `specs/living-specs/compiler/compiled-model.md`.

- **Out-of-Scope**:
  - Modifying the underlying formatting algorithms in `f8_core::value::format_cell_value`.
  - Modifying other inspection structs (`CellInspection`, `SheetCellInspection`).

### What Changes

- **`crates/f8-core/src/compiler/compiled/model.rs`**:
  - Add `pub formatted_value: Option<String>` to `TableCellInfo`.
- **`crates/f8-core/src/compiler/compiled/mod.rs`**:
  - In `inspect_table`, invoke `format_cell_value(&value, cell_node.format, cell_node.decimals, cell_node.currency)` and assign to `TableCellInfo.formatted_value`.
- **`crates/f8-cli/src/render/table.rs`**:
  - Replace on-the-fly `format_cell_value` calls in `get_cell_text` with direct reads from `cell.formatted_value`.
- **`python/f8/types.py`**:
  - Add `formatted_value: str | None` to `TableCellInfo`.

### Capabilities

- **New capabilities**:
  - Consumers of `compiled.inspect_table(table_id)` in Python and Rust immediately receive canonical accounting strings (e.g. `"$ 1,200.0"`, `"(50.0)"`, `"15.0%"`) without external formatting utilities.
  - `TableAuditor.to_markdown(view="values")` in Python can render fully formatted accounting tables matching CLI presentation out of the box.

- **Modified capabilities**:
  - `TableCellInfo` serialization contains `"formatted_value"`.
  - `crates/f8-cli` table rendering delegates cell formatting entirely to the compiler inspection artifact.

---

## Design

### Goals

1. **Single Source of Truth**: All inspection artifacts (`inspect_cell`, `inspect_sheet`, `inspect_table`, `precedents_of`, `dependents_of`) provide identical, pre-computed accounting typography via `format_cell_value`.
2. **Zero Downstream Formatting Duplication**: Outer adapters (CLI, Python SDK, web frontends) consume pre-rendered text without re-implementing accounting number rules.
3. **Microsecond In-Memory Synthesis**: Pre-formatting occurs in Rust during the inspection pass in microseconds, utilizing existing layout and format node metadata already loaded in cache.

### Invariants

- **Formatting Equivalence**: The `formatted_value` of a cell in `inspect_table` is strictly identical to the `formatted_value` returned for that same cell by `inspect_cell` and `inspect_sheet`.
- **Empty Cell Behavior**: Empty cells (`CellValue::Empty`) format as an empty string (`Some(String::new())` or empty string equivalent) matching `format_cell_value` behavior.
- **Error Formatting**: Excel error values format as their canonical string representations (`"#DIV/0!"`, `"#VALUE!"`, etc.).
- **Read-Only Purity**: Pre-formatting does not mutate the cell node, evaluated value, or compiler layout map.

### Decisions

- **Decision 1: Pre-compute in `inspect_table` vs Lazy Getter**:
  - _Rationale_: `inspect_table` already iterates through all declared table rows and columns to construct `TableCellInfo`. Populating `formatted_value` alongside `value`, `format`, `decimals`, and `currency` requires zero extra lookups and serializes across the PyO3 C-ABI in a single pass.
  - _Alternatives considered_: Exposing a separate formatting function in PyO3 or computing on demand in Python. Rejected because it fragments the single source of truth and requires Python callers to manage formatting rules.

- **Decision 2: Option<String> Field Signature**:
  - _Rationale_: Matches `CellNeighborInspection.formatted_value: Option<String>` and `SheetCellInspection.formatted_value: Option<String>`, preserving uniform schema conventions across the entire compiler inspection API.

### Data Contracts

#### Rust Contract (`crates/f8-core/src/compiler/compiled/model.rs`)

```rust
pub struct TableCellInfo {
    /// Evaluated cell value.
    pub value: CellValue,
    /// Pre-formatted runtime value string according to cell format preset, currency, and decimals.
    pub formatted_value: Option<String>,
    /// `true` if calculated by formula.
    pub is_formula: bool,
    /// High-level formula expression string if applicable.
    pub formula_expr: Option<String>,
    /// Lowered Excel formula string if applicable.
    pub excel_formula: Option<String>,
    /// Physical cell address on sheet (e.g. `"C14"`).
    pub physical_a1: String,
    /// Formatting preset if assigned.
    pub format: Option<FormatPreset>,
    /// Display precision override if specified.
    pub decimals: Option<u32>,
    /// Currency code if assigned.
    pub currency: Option<CurrencyCode>,
}
```

#### Python Type Contract (`python/f8/types.py`)

```python
class TableCellInfo(TypedDict):
    """Cell audit metadata within a 2D table inspection view."""

    value: CellValueDict
    formatted_value: str | None
    is_formula: bool
    formula_expr: str | None
    excel_formula: str | None
    physical_a1: str
    format: str | None
    decimals: int | None
    currency: str | None
```

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Currency & Percentage Formatted Value Synthesis**
  - **Given** a compiled model with a table containing a currency cell (`value = 1200.0`, `format = Currency`) and a percentage cell (`value = 0.15`, `format = Percentage`).
  - **When** calling `compiled.inspect_table(table_id)`.
  - **Then** the currency cell's `formatted_value` equals `Some("$ 1,200.0")` (or configured currency format).
  - **And** the percentage cell's `formatted_value` equals `Some("15.0%")`.

- [x] **Scenario 2: Negative Number Accounting Format in Table Inspection**
  - **Given** a cell with negative numeric value `-50.0` and currency format.
  - **When** calling `compiled.inspect_table(table_id)`.
  - **Then** `formatted_value` displays accounting parentheses: `Some("($ 50.0)")`.

- [x] **Scenario 3: CLI Table Rendering Parity**
  - **Given** a table with formatted currency, integer, and percentage cells.
  - **When** executing `f8 inspect table <id>` in the CLI.
  - **Then** rendered table output is identical before and after switching `crates/f8-cli/src/render/table.rs` to consume `cell.formatted_value`.

- [x] **Scenario 4: Python SDK Marshaling & Pyright Compliance**
  - **Given** a compiled model inspected via Python `compiled.inspect_table(table_id)`.
  - **When** accessing `cell["formatted_value"]`.
  - **Then** returns a string matching the pre-formatted accounting value.
  - **And** static analysis via `uv run pyright` passes with zero type errors.

---

## Tasks

### Phase 1: Core Compiler Inspection Enhancement

- [x] Add `pub formatted_value: Option<String>` to `TableCellInfo` in `crates/f8-core/src/compiler/compiled/model.rs`.
- [x] Populate `formatted_value` using `format_cell_value` in `CompiledModel::inspect_table` in `crates/f8-core/src/compiler/compiled/mod.rs`.
- [x] Update unit and integration tests in `crates/f8-core` verifying `TableCellInfo.formatted_value`.

### Phase 2: CLI Rendering Simplification

- [x] Update `get_cell_text` in `crates/f8-cli/src/render/table.rs` to read `cell.formatted_value.as_deref().unwrap_or_default()`.
- [x] Remove redundant `format_cell_value` call from `crates/f8-cli/src/render/table.rs`.
- [x] Verify CLI snapshot and rendering tests pass with zero regressions.

### Phase 3: Python SDK Contract Synchronization

- [x] Update `TableCellInfo` TypedDict in `python/f8/types.py` to include `formatted_value: str | None`.
- [x] Verify native extension marshaling with `uv run maturin develop`.
- [x] Add Python test in `python/tests/` asserting `inspect_table` returns `formatted_value`.
- [x] Verify Python verification suite passes (`uv run ruff check .`, `uv run pyright`, `uv run pytest`).

### Phase 4: Verification & Living Spec Reconciliation

- [x] Run full workspace verification:
  - `cargo check && cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
  - `uv run ruff check . && uv run ruff format --check . && uv run pyright && uv run pytest`
- [x] Update `specs/living-specs/compiler/compiled-model.md` Section 5 to document `formatted_value` on `TableCellInfo`.
- [x] Archive draft spec to `specs/history/` upon completion.
