---
title: Reference Resolution Engine
date: 2026-09-18
status: completed
target-living-spec: specs/living-specs/compiler/dependency-graph.md
---

## Proposal

### What

Implement the **Reference Resolution Engine** (`finx::compiler::graph::resolve`), the first sub-pass of Pass 3 (Dependency Analysis). The engine traverses formula expression ASTs ([`Expr`](src/expr/ast.rs)) across a compiled [`CellGrid`](src/compiler/ir/cell.rs), lowers dynamic coordinate selectors (`@col`, `@row`, `col.prev(k)`, `col.next(k)`, coordinate slices, and group milestones) against the [`SymbolTable`](src/compiler/ir/symbol.rs), validates coordinate boundaries, and produces a deterministic dependency mapping (`IndexMap<CellId, IndexSet<CellId>>`).

### Why

1. **Prerequisite for dependency analysis**: Petgraph DAG construction and Tarjan's Strongly Connected Components (SCC) cycle detection require explicit, resolved cell-to-cell dependency edges.
2. **Separation of concerns**: Resolving multi-dimensional financial coordinate math (active coordinates, temporal timeline offsets, range slices, and group expansions) is domain logic distinct from graph theory algorithms. Decoupling reference resolution into an autonomous pass isolates reasoning, improves testability, and reduces cognitive load.
3. **Fail-fast semantic validation**: Invalid coordinates (non-existent tables, missing rows/columns, temporal offsets reaching past schedule boundaries, inverted slices) must be caught and reported with actionable diagnostics before graph construction begins.

### Scope

- **In-scope**:
  - `src/compiler/graph/error.rs`: Structured `ResolveError` enum defining domain coordinate failure modes with actionable diagnostics (origin `CellId`, offending token, expected boundary).
  - `src/compiler/graph/resolve.rs`:
    - AST traversal extracting all [`TableRef`](src/expr/ast.rs) references from an [`Expr`](src/expr/ast.rs).
    - Resolution of [`ColSelector`](src/expr/ast.rs): active column (`@` / `@col`), discrete key, closed interval range (`col_a..col_b`), temporal offset (`col.prev(k)`, `col.next(k)`), and group milestones (`cols.<group>`, `.first`, `.last`).
    - Resolution of [`RowSelector`](src/expr/ast.rs): discrete key, closed interval range (`row_a..row_b`), and group milestones (`rows.<group>`, `.first`, `.last`).
    - Cartesian product expansion for 2D block references ($R \times C$).
    - Top-level orchestration function `resolve_cell_dependencies(&CellGrid, &SymbolTable) -> Result<IndexMap<CellId, IndexSet<CellId>>, ResolveError>`.
  - `tests/reference_resolution_test.rs`: Exhaustive integration test suite validating every selector variant, bounds checking, and error scenario.
- **Out-of-scope**:
  - Graph data structures, Tarjan SCC cycle detection, and topological sorting (Pass 3B).
  - In-memory formula calculation and function execution (Pass 4).
  - Physical coordinate lowering ($A$1) and workbook emission (Pass 5 & 6).

### What Changes

- `src/compiler/graph/mod.rs`: Declare and re-export `error` and `resolve` submodules.
- `src/compiler/graph/error.rs`: Introduce `ResolveError` using `thiserror` with compact boxed origin and constructor helpers.
- `src/compiler/graph/resolve.rs`: Implement the reference resolution pass and AST reference extractor.
- `src/compiler/mod.rs`: Expose graph module.
- `tests/reference_resolution_test.rs`: Comprehensive test suite for all reference addressing shapes and error diagnostics.

### Capabilities

- **New capabilities**:
  - Traversal of formula ASTs to collect incoming cell dependencies.
  - Active coordinate binding (`@col` / `@`) to the evaluating cell's column, validating identical `ColId` existence in cross-table targets.
  - Relative timeline offset arithmetic (`col.prev(k)`, `col.next(k)`), finding predecessor/successor coordinates along the origin table's column axis.
  - Inclusive coordinate range expansion (`start..end`) into ordered sequences of row or column identifiers.
  - Row and column group expansion, resolving group identifiers and `.first` / `.last` milestone anchors against `SymbolTable`.
  - Deterministic dependency collection returning `IndexMap<CellId, IndexSet<CellId>>`.
  - Comprehensive coordinate bounds diagnostics halting compilation on out-of-bounds temporal offsets, unknown entities, and inverted slices.
- **Modified capabilities**: None (greenfield sub-pass).

---

## Design

### Goals

1. **Deterministic resolution**: Dependencies must be collected in exact declaration order without duplicate edges.
2. **Strict coordinate bounds checking**: Relative offsets and ranges must be strictly validated against registered axis lengths and positions.
3. **Actionable diagnostic reporting**: Every error must pinpoint the origin cell where the formula was declared and the specific unresolved target.
4. **Clean decoupling**: Zero dependencies on `petgraph` or downstream evaluation state; pure functional input $\rightarrow$ output.

### Invariants

1. **Mandatory table qualification**: Every reference explicitly identifies a target `TableId`.
2. **Axis bounds enforcement**: Temporal offsets cannot escape axis boundaries ($idx - k < 0$ or $idx + k \ge len$).
3. **Range monotonicity**: Range slices $[a, b]$ must satisfy $start\_idx \le end\_idx$ in declaration order; inverted ranges are strictly rejected.
4. **Cross-table coordinate identity**: Cross-table active column (`TableB["rev", @col]`) requires `TableB` to declare a column with identical `ColId`.
5. **Deterministic order**: Resolved range slices and group expansions produce cells in exact axis declaration order.

### Decisions

- **Decision 1: Return `IndexMap<CellId, IndexSet<CellId>>` rather than immediate graph construction**:
  - *Rationale*: Isolates coordinate math from graph representation. Enables independent unit testing, dependency serialization, and re-use by future incremental compiler caching passes without `petgraph` coupling.
- **Decision 2: Origin table axis anchors relative temporal offsets**:
  - *Rationale*: When evaluating `TableB["cash", col.prev]` inside `TableA`, the temporal step corresponds to `TableA`'s timeline index $t - 1$, looking up the corresponding `ColId` in `TableA`, then verifying that `TableB` possesses that matching period. This aligns with financial modeling semantics where models advance in lockstep across a shared timeline.
- **Decision 3: Dedicated `ResolveError` enum with `thiserror` and `origin: Box<CellId>`**:
  - *Rationale*: Clear, typed domain diagnostics pinpointing the exact offending cell coordinate, failed identifier, and reason for failure. Boxing the origin keeps the error variant size small (<128 bytes), preventing large-result-error warnings and keeping memory efficient.

### Data Contracts

#### `ResolveError` Definition

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ResolveError {
    #[error("Formula in cell '{origin}' references unknown table '{target_table}'")]
    UnknownTable {
        origin: Box<CellId>,
        target_table: TableId,
    },

    #[error("Formula in cell '{origin}' references unknown row '{row}' in table '{target_table}'")]
    UnknownRow {
        origin: Box<CellId>,
        target_table: TableId,
        row: RowId,
    },

    #[error("Formula in cell '{origin}' references unknown column '{column}' in table '{target_table}'")]
    UnknownColumn {
        origin: Box<CellId>,
        target_table: TableId,
        column: ColId,
    },

    #[error("Formula in cell '{origin}' references unknown row group '{group}' in table '{target_table}'")]
    UnknownRowGroup {
        origin: Box<CellId>,
        target_table: TableId,
        group: RowGroupId,
    },

    #[error("Formula in cell '{origin}' references unknown column group '{group}' in table '{target_table}'")]
    UnknownColumnGroup {
        origin: Box<CellId>,
        target_table: TableId,
        group: ColGroupId,
    },

    #[error("Formula in cell '{origin}' references active column '{column}', but target table '{target_table}' does not declare this column")]
    ActiveColumnMismatch {
        origin: Box<CellId>,
        target_table: TableId,
        column: ColId,
    },

    #[error("Formula in cell '{origin}' has temporal offset {offset:+} which exceeds axis boundaries")]
    TemporalOffsetOutOfBounds {
        origin: Box<CellId>,
        offset: i32,
    },

    #[error("Formula in cell '{origin}' specifies inverted range '{start}'..'{end}' in table '{target_table}'")]
    InvertedRange {
        origin: Box<CellId>,
        target_table: TableId,
        start: String,
        end: String,
    },

    #[error("Table '{table}' containing cell '{origin}' is not registered in the symbol table")]
    UnregisteredTable {
        origin: Box<CellId>,
        table: TableId,
    },
}
```

#### Resolution Functions

```rust
/// Traverses all formulas in the cell grid, resolving each cell's references into discrete target CellIds.
pub fn resolve_cell_dependencies(
    grid: &CellGrid,
    symbol_table: &SymbolTable,
) -> Result<IndexMap<CellId, IndexSet<CellId>>, ResolveError>;

/// Resolves a single TableRef expression in the context of an evaluating cell coordinate.
pub fn resolve_table_ref(
    origin: &CellId,
    table_ref: &TableRef,
    symbol_table: &SymbolTable,
) -> Result<Vec<CellId>, ResolveError>;
```

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Resolving discrete coordinate reference**
  - **Given** an evaluating cell `T1["rev", "2024E"]` with formula `T2["tax", "rate"]`
  - **When** `resolve_table_ref` is called
  - **Then** it resolves exactly to `[CellId(T2, "tax", "rate")]`.

- [x] **Scenario 2: Resolving active column reference**
  - **Given** an evaluating cell `T1["ebit", "2024E"]` with formula `T1["rev", @] - T1["cogs", @]`
  - **When** `resolve_cell_dependencies` is called
  - **Then** dependencies for `T1["ebit", "2024E"]` contain `T1["rev", "2024E"]` and `T1["cogs", "2024E"]`.

- [x] **Scenario 3: Cross-table active column mismatch rejected**
  - **Given** an evaluating cell `T1["ebit", "2024E"]` referencing `T2["metric", @]` where `T2` only has columns `["A", "B"]`
  - **When** reference resolution is executed
  - **Then** it fails with `ResolveError::ActiveColumnMismatch`.

- [x] **Scenario 4: Resolving relative temporal predecessor offset**
  - **Given** an evaluating cell `BS["cash", "2025E"]` with column index 1 referencing `BS["cash", col.prev]`
  - **When** reference resolution is executed
  - **Then** it resolves to `[BS["cash", "2024E"]]`.

- [x] **Scenario 5: Temporal offset out of bounds rejected**
  - **Given** an evaluating cell `BS["cash", "2024E"]` with column index 0 referencing `BS["cash", col.prev]`
  - **When** reference resolution is executed
  - **Then** it fails with `ResolveError::TemporalOffsetOutOfBounds { offset: -1 }`.

- [x] **Scenario 6: Resolving closed horizontal column range**
  - **Given** an evaluating cell referencing `T["rev", "2024E".."2026E"]` across columns `["2024E", "2025E", "2026E", "2027E"]`
  - **When** reference resolution is executed
  - **Then** it resolves to `[T["rev", "2024E"], T["rev", "2025E"], T["rev", "2026E"]]` in declaration order.

- [x] **Scenario 7: Inverted range slice rejected**
  - **Given** an evaluating cell referencing `T["rev", "2026E".."2024E"]`
  - **When** reference resolution is executed
  - **Then** it fails with `ResolveError::InvertedRange`.

- [x] **Scenario 8: Resolving column group and milestone selectors**
  - **Given** a table with column group `forecast` containing `["2025E", "2026E"]`
  - **When** resolving `cols.forecast`, `cols.forecast.first`, and `cols.forecast.last`
  - **Then** they resolve to the full slice, `2025E`, and `2026E` respectively.

- [x] **Scenario 9: Resolving 2D block range**
  - **Given** a reference `T[rows.expenses, cols.forecast]` with 2 rows and 2 columns
  - **When** reference resolution is executed
  - **Then** it resolves to all 4 Cartesian product `CellId`s in row-major declaration order.

- [x] **Scenario 10: Rejecting unknown table, row, or column**
  - **Given** a formula referencing non-existent table, row, or column IDs
  - **When** reference resolution is executed
  - **Then** it returns the corresponding `UnknownTable`, `UnknownRow`, or `UnknownColumn` error with the origin `CellId`.

---

## Tasks

### Phase 1: Error Diagnostics and AST Traversal
- [x] Create `src/compiler/graph/error.rs` with `ResolveError` enum implementing `thiserror`.
- [x] Implement recursive AST helper `collect_references(expr: &Expr) -> Vec<&TableRef>` in `src/compiler/graph/resolve.rs`.
- [x] Update `src/compiler/graph/mod.rs` to expose `error` and `resolve` submodules.

### Phase 2: Coordinate Resolution Engine
- [x] Implement `resolve_col_selector(origin: &CellId, target_table: &TableMetadata, origin_table: &TableMetadata, col: &ColSelector) -> Result<Vec<ColId>, ResolveError>`.
- [x] Implement `resolve_row_selector(origin: &CellId, target_table: &TableMetadata, row: &RowSelector) -> Result<Vec<RowId>, ResolveError>`.
- [x] Implement `resolve_table_ref(origin: &CellId, table_ref: &TableRef, symbol_table: &SymbolTable) -> Result<Vec<CellId>, ResolveError>`.
- [x] Implement `resolve_cell_dependencies(grid: &CellGrid, symbol_table: &SymbolTable) -> Result<IndexMap<CellId, IndexSet<CellId>>, ResolveError>`.

### Phase 3: Integration Tests & Verification Suite
- [x] Create `tests/reference_resolution_test.rs` covering all 10 acceptance scenarios.
- [x] Run full verification suite:
  - `cargo check`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`

### Phase 4: Reconcile Living Spec & Archive Draft
- [x] Update `specs/living-specs/compiler/dependency-graph.md` to document the reference resolution rules and contracts.
- [x] Move draft spec to `specs/history/2026-09-18-reference-resolution-engine.md`.
