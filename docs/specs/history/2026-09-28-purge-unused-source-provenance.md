---
title: Purge Unused Source Provenance Metadata
date: 2026-09-28
status: completed
target-living-spec: specs/living-specs/compiler/cell-ir.md
---

## Proposal

### What

Purge the vestigial and unused `source: Option<serde_json::Value>` provenance field and its cascading resolution logic across the compiler AST models, IR cell nodes, and grid expansion engine.

### Why

- **Greenfield minimalism**: Zero tolerance for dead code, legacy stubs, or unused speculative fields (`AGENTS.md`).
- **Eliminate untyped structures**: `source` was modeled as an arbitrary untyped JSON tree (`Option<serde_json::Value>`), conflicting with the compiler's strict typing principles.
- **Eliminate hidden allocation overhead**: In `expand_table`, `effective_source` performs deep heap clones of `serde_json::Value` across every cell in a table, only for downstream passes (OpenXML workbook emission, calculation DAG evaluation, inspection facade) to discard it.
- **Precedence alignment**: In `specs/history/2026-09-24-cell-level-notes-and-vector-note-elimination.md`, vector-level notes on rows and columns were eliminated because 1D vector broadcasting is flawed for point annotations. `source` retains that same flawed vector cascading (`Cell > Row > Col`).
- **Future-proofing**: Any future data provenance or lineage tracking subsystem must be designed intentionally with strongly typed domain structures and living specs, rather than retaining an unconsumed, untyped placeholder.

### Scope

- **In-scope**:
  - Remove `source: Option<serde_json::Value>` and builder method `with_source` from `DataColumnSpec` and `ColumnSpec` in `crates/f8-core/src/model/table.rs`.
  - Remove `source: Option<serde_json::Value>` and builder method `with_source` from `DataRowSpec` and `RowSpec` in `crates/f8-core/src/model/table.rs`.
  - Remove `source: Option<serde_json::Value>` from `DetailedCellInput` in `crates/f8-core/src/model/table.rs`.
  - Remove getter method `CellInput::source(&self)` in `crates/f8-core/src/model/table.rs`.
  - Remove `source: Option<serde_json::Value>` and builder method `with_source` from `CellNode` in `crates/f8-core/src/compiler/ir/cell.rs`.
  - Remove cascading `cell_source` and `effective_source` resolution logic from `expand_table` in `crates/f8-core/src/compiler/ir/expand.rs`.
  - Update unit and integration tests in `crates/f8-core/tests/grid_expansion_test.rs` and `crates/f8-core/tests/spec_test.rs`.
  - Update documentation in `docs/compiler/ir.md`.
  - Reconcile living spec `specs/living-specs/compiler/cell-ir.md`.
- **Out-of-scope**:
  - Standard library and `thiserror` error chaining fields named `source` (`#[source] source: ...`).
  - Lexer input byte length `source_len: usize` in `crates/f8-core/src/expr/parser.rs`.
  - Dependency graph node index `source_idx` in `crates/f8-core/src/compiler/graph/dependency.rs`.
  - Financial statement column identifiers in test fixtures (e.g., `"source_item"`, `"source_val"`, `"source_pct"` in Sources & Uses tables).

### What Changes

- `crates/f8-core/src/model/table.rs`:
  - `DataColumnSpec`: Remove `pub source: Option<serde_json::Value>`, remove constructor initialization `source: None`, remove `pub fn with_source(...)`.
  - `ColumnSpec`: Remove `pub fn with_source(...)`.
  - `DataRowSpec`: Remove `pub source: Option<serde_json::Value>`, remove constructor initialization `source: None`, remove `pub fn with_source(...)`.
  - `RowSpec`: Remove `pub fn with_source(...)`.
  - `DetailedCellInput`: Remove `pub source: Option<serde_json::Value>`.
  - `CellInput`: Remove `pub fn source(&self) -> Option<&serde_json::Value>`.
- `crates/f8-core/src/compiler/ir/cell.rs`:
  - `CellNode`: Remove `pub source: Option<serde_json::Value>`, remove constructor initialization `source: None`, remove `pub fn with_source(...)`.
- `crates/f8-core/src/compiler/ir/expand.rs`:
  - `expand_table`: Remove `let cell_source = ...`, remove `let effective_source = ...`, and remove `.with_source(...)` calls on `CellNode`.
- `crates/f8-core/tests/grid_expansion_test.rs`:
  - Refactor `test_scenario_4_metadata_and_format_preset_propagation` to focus on format preset and cell note propagation without `source`.
  - Remove `source: None` literals in test fixtures.
- `crates/f8-core/tests/spec_test.rs`:
  - Remove `source: None` literals in test fixtures.
- `docs/compiler/ir.md`:
  - Remove "Provenance Source Precedence" and metadata notes.

### Capabilities

- **New capabilities**: Zero heap allocation overhead from cloning unused JSON provenance metadata during table expansion.
- **Modified capabilities**: AST and IR nodes no longer carry or deserialize the unused `source` attribute.

---

## Design

### Goals

1. Eliminate all dead `source` fields and builder methods across AST and IR models.
2. Eliminate dynamic `serde_json::Value` heap cloning during grid expansion.
3. Keep the compiler core minimalist, explicit, and free of untyped JSON structures.
4. Maintain 100% green verification passes across all unit, integration, and lint suites.

### Invariants

- **0D cell note preservation**: Cell notes (`DetailedCellInput.note`) remain intact as the single source of truth for cell-level text annotations.
- **Emission invariance**: OpenXML workbook emission byte output and inspection values remain identical.
- **Deterministic compilation**: Grid expansion ordering and node structure remain fully deterministic.

### Decisions

- **Choice**: Completely purge `source` rather than deprecating or retaining it as a pass-through stub.
  - **Rationale**: Greenfield codebase principle forbids backward-compatibility shims, aliases, or dead stubs. Callers and internal tests are updated in-place.
  - **Alternatives considered**: Keeping `Option<serde_json::Value>` on `CellNode` but removing it from rows and columns. Rejected because downstream passes do not consume cell provenance, leaving it as dead code.
- **Choice**: Defer future provenance modeling to a dedicated, strongly typed design.
  - **Rationale**: If audit lineage or data origin tracking is needed, it must be designed with typed schemas, living specs, and end-to-end emission/inspection contracts.

### Data Contracts

#### AST & IR Changes

| Type | Member | Status | Description |
| :--- | :--- | :--- | :--- |
| `DataColumnSpec` | `pub source: Option<serde_json::Value>` | **Removed** | Dead provenance metadata field |
| `DataColumnSpec` | `pub fn with_source(...)` | **Removed** | Removed builder method |
| `ColumnSpec` | `pub fn with_source(...)` | **Removed** | Removed builder method |
| `DataRowSpec` | `pub source: Option<serde_json::Value>` | **Removed** | Dead provenance metadata field |
| `DataRowSpec` | `pub fn with_source(...)` | **Removed** | Removed builder method |
| `RowSpec` | `pub fn with_source(...)` | **Removed** | Removed builder method |
| `DetailedCellInput` | `pub source: Option<serde_json::Value>` | **Removed** | Dead provenance metadata field |
| `CellInput` | `pub fn source(&self)` | **Removed** | Removed getter method |
| `CellNode` | `pub source: Option<serde_json::Value>` | **Removed** | Dead provenance metadata field |
| `CellNode` | `pub fn with_source(...)` | **Removed** | Removed builder method |

### Risks & Trade-offs

- **Risk**: External JSON specifications specifying `"source"` on detailed cell inputs will fail deserialization due to `#[serde(deny_unknown_fields)]` on `DetailedCellInput`.
  - **Mitigation**: No existing models or fixtures in the repository or SDK use `"source"`. Eliminating unknown fields immediately surfaces dead configurations.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Grid expansion without provenance cloning**
  - **Given** a table with data rows, measure columns, and detailed cell inputs
  - **When** the table is expanded via `expand_table`
  - **Then** all `CellNode` items are created with format presets, styles, and notes without allocating or propagating `source`.

- [x] **Scenario 2: DetailedCellInput deserialization schema enforcement**
  - **Given** a JSON payload specifying `"source": { "doc": "10-K" }` inside a detailed cell input
  - **When** deserialized into `DetailedCellInput`
  - **Then** deserialization is rejected by `deny_unknown_fields`.

- [x] **Scenario 3: CellNode constructor and builder cleanliness**
  - **Given** a new `CellNode` created via `CellNode::new`
  - **When** builder methods (`with_format`, `with_note`, etc.) are chained
  - **Then** no `with_source` method exists and `CellNode` has no `source` field.

---

## Tasks

### Phase 1: Model & AST Updates
- [x] Remove `source` and `with_source` from `DataColumnSpec` and `ColumnSpec` in `crates/f8-core/src/model/table.rs`.
- [x] Remove `source` and `with_source` from `DataRowSpec` and `RowSpec` in `crates/f8-core/src/model/table.rs`.
- [x] Remove `source` from `DetailedCellInput` in `crates/f8-core/src/model/table.rs`.
- [x] Remove `source(&self)` from `CellInput` in `crates/f8-core/src/model/table.rs`.

### Phase 2: Compiler IR & Grid Expansion Updates
- [x] Remove `source` and `with_source` from `CellNode` in `crates/f8-core/src/compiler/ir/cell.rs`.
- [x] Remove `cell_source`, `effective_source`, and `.with_source(...)` calls from `expand_table` in `crates/f8-core/src/compiler/ir/expand.rs`.

### Phase 3: Test Fixture & Documentation Updates
- [x] Refactor `test_scenario_4_metadata_and_format_preset_propagation` in `crates/f8-core/tests/grid_expansion_test.rs` to verify format preset and note propagation without `source`.
- [x] Remove `source: None` literals in `crates/f8-core/tests/grid_expansion_test.rs`.
- [x] Remove `source: None` literals in `crates/f8-core/tests/spec_test.rs`.
- [x] Add unit test verifying `DetailedCellInput` rejects unknown `"source"` field.
- [x] Update documentation in `docs/compiler/ir.md`.

### Phase 4: Verification Suite
- [x] Run `cargo check`.
- [x] Run `cargo test`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo fmt --check`.
- [x] Run `uv run pytest`.

### Phase 5: Living Spec Reconciliation & Archive
- [x] Reconcile `specs/living-specs/compiler/cell-ir.md`.
- [x] Move draft spec from `specs/drafts/` to `specs/history/`.
