---
title: Unified 2D Matrix Engine Refactor
date: 2026-09-17
status: completed
target-living-spec:
  - specs/living-specs/model/spec-ast.md
  - specs/living-specs/compiler/cell-ir.md
---

## Proposal

### What

Refactor the Finx core model (`finx::model`) and compiler intermediate representation (`finx::compiler::ir`) to natively implement the **Unified 2D Matrix Model**. Replace legacy schema constructs (`match` partition blocks, positional `value` vectors, `RowSpec.label`, and Column A/B stubs) with symmetrical `ColumnSpec` / `RowSpec` definitions, explicit `AxisRole` (`measure` vs `descriptor`), 0D cell override maps (`row.cells`), 1D vector rules (`row.expr` / `col.expr`), and the **3-Tier Specificity Resolution Lattice**.

### Why

1. **Architecture alignment**: The documentation suite (`docs/`) and formula expression parser (`finx::expr`) have already been updated to the unified 2D matrix architecture. The underlying model AST and grid expansion engine must align with the documentation and parser.
2. **Elimination of legacy baggage**: `match` partition blocks and positional `value` arrays created arbitrary asymmetry between rows and columns, required verbose boilerplate, and were prone to dimension mismatches.
3. **Greenfield rigor**: Finx enforces zero backward-compatibility shims, zero deprecated aliases, and a single source of truth. The engine must natively reflect 2D matrix mechanics.

### Scope

- **In-scope**:
  - `src/model/table.rs`: Introduce `ColumnSpec`, `AxisRole`, `CellInput`, and detailed cell overrides. Refactor `TableSpec` and `RowSpec`. Purge `MatchArmSpec`, `MatchSelector`, `RowSpec.label`, `RowSpec.match_arms`, and `RowSpec.value`.
  - `src/model/error.rs`: Update `ModelError` variants to reflect 2D matrix schema validation (e.g. invalid cell column reference, mutually exclusive cell payload, empty table/row/column). Purge obsolete match errors.
  - `src/compiler/ir/symbol.rs`: Update `TableMetadata` to store `Vec<ColumnSpec>`, track column roles and row roles, and index descriptor columns.
  - `src/compiler/ir/expand.rs`: Rewrite grid expansion (`expand_table`, `expand_grid`, `expand_workbook`) around the 3-Tier Specificity Lattice:
    $$\text{Cell Override (0D Point)} > (\text{Row Rule } \mathbf{XOR} \text{ Column Rule}) > \text{Explicit Null}$$
  - `src/compiler/ir/error.rs`: Add 2D matrix IR diagnostics: `AmbiguousExpressionConflict` and `UnassignedCell`.
  - Integration tests in `tests/spec_test.rs`, `tests/grid_expansion_test.rs`, and `tests/cell_ir_test.rs`: Reconcile all tests with the 2D matrix schema and invariants.
- **Out-of-scope**:
  - Dependency graph DAG construction (Pass 3, `finx::compiler::graph`).
  - Topological formula evaluation (Pass 4, `finx::compiler::eval`).
  - Physical sheet layout emission (Pass 5 & 6, `finx::compiler::layout` / `finx::compiler::emission`).

### What Changes

- `src/model/table.rs`:
  - `ColumnSpec`: Struct with `id: ColId`, `role: AxisRole`, `label: Option<String>`, `format: Option<FormatPreset>`, `expr: Option<String>`, `note: Option<String>`, `source: Option<serde_json::Value>`.
  - `AxisRole`: Enum `Measure` (default) and `Descriptor`.
  - `RowSpec`: Struct with `id: RowId`, `role: AxisRole`, `format: Option<FormatPreset>`, `expr: Option<String>`, `note: Option<String>`, `source: Option<serde_json::Value>`, `cells: IndexMap<ColId, CellInput>`.
  - `CellInput`: Enum/struct representing 0D cell input: scalar `ValueLiteral` (number, string, bool, null), formula `{ "expr": String }`, or detailed override `{ "value": ..., "expr": ..., "format": ..., "note": ..., "source": ... }`.
  - Delete `MatchArmSpec` and `MatchSelector`.
- `src/model/error.rs`:
  - Add `UnknownCellColumn`, remove `MutuallyExclusiveArmPayload`, `UncoveredColumns`, `OverlappingMatchArm`, `InvalidMatchSelector`.
- `src/compiler/ir/error.rs`:
  - Add `AmbiguousExpressionConflict { table: TableId, row: RowId, column: ColId }`.
  - Add `UnassignedCell { table: TableId, row: RowId, column: ColId }`.
  - Add `FormatConflict { table: TableId, row: RowId, column: ColId, row_format: FormatPreset, col_format: FormatPreset }`.
  - Update `FormulaParse` coordinate reporting with `Option<RowId>`.
- `src/compiler/ir/symbol.rs`:
  - `TableMetadata`: Store `columns: Vec<ColumnSpec>` instead of `Vec<ColId>`. Add helper lookups for roles and column specs.
- `src/compiler/ir/expand.rs`:
  - Implement 3-tier lattice resolution for each `(row, col)` coordinate pair.
  - Emit descriptor cells with literal labels.
  - Isolate vector formulas: never evaluate `row.expr` or `col.expr` on descriptor axes.
  - Resolve formats via lattice: Cell override > (Row/Col format with FormatConflict on opposing measures).
- `tests/`:
  - Update `spec_test.rs` to validate `TableSpec` with `ColumnSpec`, `AxisRole`, `CellInput`.
  - Update `grid_expansion_test.rs` to verify 2D matrix resolution, cell overrides, vector formulas, conflict detection, and descriptor nodes.

### Capabilities

- **New capabilities**:
  - Declarative authoring using symmetrical rows and columns with explicit `role` (`measure` or `descriptor`).
  - Column vector formulas (`column.expr`) broadcast downward across opposing measure rows.
  - Row vector formulas (`row.expr`) broadcast across opposing measure columns.
  - Cell-level 0D point overrides via `row.cells` for numbers, strings, booleans, nulls, and point formulas.
  - Strict compiler diagnostic on vector rule conflicts (`AmbiguousExpressionConflict`) when row and column formulas intersect without a cell override.
  - Strict compiler diagnostic on unassigned measure cells (`UnassignedCellError`), prohibiting silent fallback to zero.
- **Modified capabilities**:
  - `expand_table` iterates over all $(r, c) \in \text{rows} \times \text{columns}$ applying the 3-tier specificity lattice.

---

## Design

### Goals

1. **Orthogonal symmetry**: Treat row and column dimensions symmetrically with first-class `role` declarations and vector rule execution.
2. **Deterministic conflict resolution**: Implement the 3-tier specificity lattice without ambiguity or silent precedence rules.
3. **Zero silent blanks or fallbacks**: Ensure every measure cell in a table has an explicit calculation contract (value, formula, or explicit null).
4. **Clean Rust 2024 implementation**: Zero `.unwrap()` / `.expect()` in production code, strict `thiserror` error enums, and `IndexMap` deterministic ordering.

### Invariants

1. **Every physical column is explicit**: Tables contain no implicit or hardcoded layout stubs.
2. **Strict specificity lattice**:
   $$\text{Cell Override (0D Point)} > (\text{Row Rule } \mathbf{XOR} \text{ Column Rule}) > \text{Explicit Null}$$
3. **Zero silent vector conflicts**: If both `row.expr` and `col.expr` target a measure cell and neither provides an override, emit `IrError::AmbiguousExpressionConflict`.
4. **Zero silent unassigned cells**: If a measure cell has no cell override, no row vector formula, and no column vector formula, emit `IrError::UnassignedCell`.
5. **Formula isolation**: Vector formulas (`row.expr`, `col.expr`) only broadcast across opposing `measure` axes; they never evaluate across `descriptor` axes.
6. **Format conflict resolution**: Cell override format takes precedence. If measure row $r$ and measure column $c$ specify conflicting format presets without a cell override, emit `IrError::FormatConflict`. Descriptor coordinates isolate formats.

### Decisions

- **Decision 1: Untagged Serde Deserialization for `CellInput`**:
  - *Choice*: `CellInput` deserializes seamlessly from raw JSON literals (`42`, `100.5`, `"Text"`, `true`, `null`), shorthand formula objects (`{ "expr": "..." }`), or full detailed objects (`{ "value": ..., "format": ... }`).
  - *Rationale*: Allows users to write concise specs like `"cells": { "2024A": 100.0, "2025E": { "expr": "..." } }` without verbose nesting.
- **Decision 2: Descriptor Cell IR Node Generation**:
  - *Choice*: In `expand_table`, descriptor cells (e.g. row labels or descriptor column entries) are emitted as full `CellNode` instances with `CellPayload::Literal(ValueLiteral::String(...))` (or provided value).
  - *Rationale*: Downstream layout and dependency graph engines can uniformly traverse all worksheet cells as first-class `CellId` nodes.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Deserializing and validating valid 2D TableSpec**
  - **Given** a 2D JSON table specification defining descriptor and measure columns, row vector formulas, and cell overrides
  - **When** `TableSpec` is deserialized and `validate()` is called
  - **Then** validation succeeds with zero errors.

- [x] **Scenario 2: Rejecting invalid cell column in row.cells**
  - **Given** a table with declared columns `["2024A", "2025E"]`
  - **When** a row defines a cell override for `"UNKNOWN_COL"`
  - **Then** `validate()` returns `ModelError::UnknownCellColumn`.

- [x] **Scenario 3: Grid expansion with 3-tier specificity lattice**
  - **Given** a table with a row vector formula `row.expr = "Table[\"rev\", @col] * 0.1"` and a cell override for `"2024A"` with literal `500.0`
  - **When** `expand_table` is executed
  - **Then** the cell at `(row, "2024A")` has `CellPayload::Literal(500.0)` and the cell at `(row, "2025E")` has `CellPayload::Formula`.

- [x] **Scenario 4: Vector conflict diagnostic**
  - **Given** a measure column with `col.expr = "col_calc"` and a measure row with `row.expr = "row_calc"`, with no cell override at their intersection
  - **When** `expand_table` is executed
  - **Then** compilation halts with `IrError::AmbiguousExpressionConflict`.

- [x] **Scenario 5: Unassigned cell diagnostic**
  - **Given** a measure row and measure column with no `row.expr`, no `col.expr`, and no entry in `row.cells`
  - **When** `expand_table` is executed
  - **Then** compilation halts with `IrError::UnassignedCell`.

- [x] **Scenario 6: Format resolution and conflict diagnostics**
  - **Given** a measure column with `format: percentage` and a measure row with `format: currency`
  - **When** cells are expanded without a cell override
  - **Then** compilation halts with `IrError::FormatConflict`, while explicit cell-level format overrides resolve the conflict to the override format.

- [x] **Scenario 7: Descriptor cell handling**
  - **Given** a table with a descriptor column `"line_item"` and measure columns
  - **When** `expand_table` is executed
  - **Then** descriptor column cells contain string literals, and row formulas evaluate only across measure columns.

---

## Tasks

### Phase 1: Model Subsystem Refactor (`src/model`)
- [x] Implement `AxisRole` enum (`Measure`, `Descriptor`) in `src/model/table.rs`.
- [x] Implement `ColumnSpec` struct in `src/model/table.rs` with `id`, `role`, `label`, `format`, `expr`, `note`, `source`.
- [x] Implement `CellInput` enum / struct in `src/model/table.rs` supporting scalar literals, formula objects, and detailed overrides.
- [x] Refactor `RowSpec` in `src/model/table.rs`: replace `label`, `value`, `match_arms` with `role`, `expr`, `cells`.
- [x] Purge `MatchArmSpec` and `MatchSelector` from `src/model/table.rs`.
- [x] Update `ModelError` in `src/model/error.rs` to purge match errors and add 2D validation errors.
- [x] Update `TableSpec::validate` to enforce 2D matrix invariants:
  - Column uniqueness and non-empty columns.
  - Row uniqueness and non-empty rows.
  - Column and row group contiguity and disjointness.
  - Validation that every key in `row.cells` exists in `table.columns`.

### Phase 2: Compiler IR Grid Expansion Engine Rewrite (`src/compiler/ir`)
- [x] Update `TableMetadata` in `src/compiler/ir/symbol.rs` to store `Vec<ColumnSpec>` and provide role lookups.
- [x] Add `AmbiguousExpressionConflict` and `UnassignedCell` to `IrError` in `src/compiler/ir/error.rs`.
- [x] Rewrite `expand_table` in `src/compiler/ir/expand.rs`:
  - Pre-parse column expressions and row expressions.
  - Iterate over rows and columns in declaration order.
  - Apply 3-tier specificity lattice for measure cells:
    1. Cell override (`row.cells.get(col_id)`).
    2. Vector formula (`row.expr` XOR `col.expr`; conflict error if both present).
    3. Unassigned cell check (error if neither override nor vector rule exists).
  - Handle descriptor cells: materialize string/literal nodes, skip formula broadcasting.
  - Resolve cascading formats: Cell override > Row format > Column format.
- [x] Reconcile `expand_grid` and `expand_workbook`.

### Phase 3: Test Verification & Audit
- [x] Reconcile `tests/spec_test.rs` to test the new 2D matrix JSON schema and validation rules.
- [x] Reconcile `tests/grid_expansion_test.rs` and `tests/cell_ir_test.rs` to test 2D matrix grid expansion and error scenarios.
- [x] Verify unit tests in `src/model/` and `src/compiler/ir/`.
- [x] Run verification suite:
  - `cargo check`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`

### Phase 4: Reconcile Living Specs & Archive
- [x] Update `specs/living-specs/model/spec-ast.md` to document the unified 2D matrix schema.
- [x] Update `specs/living-specs/compiler/cell-ir.md` to document the 3-tier specificity resolution lattice.
- [x] Move draft spec to `specs/history/2026-09-17-unified-2d-matrix-engine.md`.
