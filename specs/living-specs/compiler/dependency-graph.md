---
title: Dependency Analysis & Reference Resolution
last_updated: 2026-09-18
domain: compiler
status: active
---

## Purpose

The dependency analysis subsystem provides semantic reference resolution, coordinate bounds validation, and DAG construction across compiled model tables. It transforms formula expression ASTs into discrete cell-to-cell dependency relationships and enforces FAST Standard circularity rules.

## Architecture & Boundaries

- **Modules & components**:
  - `finx::compiler::graph::resolve`: Formula AST traversal, coordinate selector resolution (`ColSelector`, `RowSelector`), range interval expansion, and dependency extraction.
  - `finx::compiler::graph::error`: Structured `ResolveError` domain diagnostics.
- **Data flow & interfaces**:
  - Ingests: [`CellGrid`](src/compiler/ir/cell.rs) and [`SymbolTable`](src/compiler/ir/symbol.rs).
  - Emits: Deterministic dependency mapping `IndexMap<CellId, IndexSet<CellId>>` consumed by downstream graph construction and DAG topological sorting.
- **Dependencies**: Native Rust data structures (`indexmap::IndexMap`, `indexmap::IndexSet`), [`finx::expr::ast`](src/expr/ast.rs), and [`finx::compiler::ir`](src/compiler/ir/mod.rs).

## Requirements & Invariants

### 1. Mandatory Table Qualification

Every cell reference within a formula expression must explicitly name its target `TableId` (`Table[row, col]`). Unqualified references are rejected at parse time.

#### Scenarios

- **Discrete Single Cell Reference**:
  - **When** evaluating cell `T1["rev", "2024E"]` referencing `T2["tax", "rate"]`
  - **Then** resolves to exactly `[CellId(T2, "tax", "rate")]`.
- **Unknown Table Rejected**:
  - **When** a formula references a table not present in `SymbolTable`
  - **Then** compilation halts with `ResolveError::UnknownTable` identifying the origin `CellId`.

### 2. Active Coordinate Lowering & Cross-Table Identity

The active coordinate tokens (`@col` and `@row`) resolve to the evaluating cell's column identifier (`ColId`) or row identifier (`RowId`). When used in cross-table references (`TargetTable["row", @col]` or `TargetTable[@row, "col"]`), the target table must declare a coordinate with the exact identical identifier.

#### Scenarios

- **Same-Table Active Reference**:
  - **When** evaluating `T1["ebit", "2024E"]` with formula `T1["rev", @col] - T1["cogs", @col]`
  - **Then** resolves dependencies to `T1["rev", "2024E"]` and `T1["cogs", "2024E"]`.
- **Same-Table Active Row Reference**:
  - **When** evaluating `T1["product_a", "revenue"]` with formula `T1[@row, "price"] * T1[@row, "volume"]`
  - **Then** resolves dependencies to `T1["product_a", "price"]` and `T1["product_a", "volume"]`.
- **Cross-Table Active Column Mismatch**:
  - **When** a formula in `T1` at active column `"2024E"` references `T2["metric", @col]` where `T2` has no column `"2024E"`
  - **Then** compilation halts with `ResolveError::ActiveColumnMismatch`.
- **Cross-Table Active Row Mismatch**:
  - **When** a formula in `T1` at active row `"revenue"` references `T2[@row, "metric"]` where `T2` has no row `"revenue"`
  - **Then** compilation halts with `ResolveError::ActiveRowMismatch`.

### 3. Relative Temporal Offsets & Axis Bounds

Relative temporal predecessor and successor offsets (`col.prev(k)`, `col.next(k)`, `row.prev(k)`, `row.next(k)`) calculate relative position steps ($idx \pm k$) along the origin table's column or row axis, resolving to the corresponding coordinate identifier.

#### Scenarios

- **Column Predecessor Offset**:
  - **When** evaluating cell `BS["cash", "2025E"]` (column index 1) referencing `BS["cash", col.prev]`
  - **Then** resolves to `[BS["cash", "2024E"]]`.
- **Row Predecessor Offset**:
  - **When** evaluating cell `DS["tranche_b", "balance"]` (row index 1) referencing `DS[row.prev, "balance"]`
  - **Then** resolves to `[DS["tranche_a", "balance"]]`.
- **Temporal Offset Out of Bounds**:
  - **When** evaluating cell `BS["cash", "2024E"]` (column index 0) referencing `BS["cash", col.prev]`
  - **Then** compilation halts with `ResolveError::TemporalOffsetOutOfBounds { offset: -1 }`.

### 4. Range Interval Slices & Group Expansions

Closed intervals (`"2024E".."2026E"`) and group selectors (`cols.<group>`, `rows.<group>`) expand into ordered sequences of coordinates based on axis declaration order.

#### Scenarios

- **Horizontal Column Slice**:
  - **When** evaluating a reference `T["rev", "2024E".."2026E"]`
  - **Then** resolves to `[T["rev", "2024E"], T["rev", "2025E"], T["rev", "2026E"]]`.
- **Inverted Range Rejected**:
  - **When** a slice specifies $start\_idx > end\_idx$ (e.g. `"2026E".."2024E"`)
  - **Then** compilation halts with `ResolveError::InvertedRange`.
- **2D Block Range**:
  - **When** a reference spans both a row group/range and a column group/range ($R \times C$)
  - **Then** resolves to all Cartesian product `CellId` nodes in row-major declaration order.
