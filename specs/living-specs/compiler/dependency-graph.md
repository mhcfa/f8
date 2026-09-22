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
  - `f8_core::compiler::graph::resolve`: Formula AST traversal, coordinate selector resolution (`ColSelector`, `RowSelector`), range interval expansion, and dependency extraction.
  - `f8_core::compiler::graph::dependency`: Directed cell dependency graph (`DependencyGraph`), edge construction, Tarjan SCC cycle detection, DFS cycle path extraction, and deterministic topological sorting.
  - `f8_core::compiler::graph::cycle`: Pluggable `CycleHandler` trait and `StrictDagHandler` implementation.
  - `f8_core::compiler::graph::error`: Structured `ResolveError` and `GraphError` domain diagnostics.
- **Data flow & interfaces**:
  - Ingests: [`CellGrid`](../../../src/compiler/ir/cell.rs) and [`SymbolTable`](../../../src/compiler/ir/symbol.rs).
  - Emits: Verified `DependencyGraph` with deterministic topological evaluation order `&[CellId]` consumed by downstream in-memory evaluation (Pass 4) and layout planning (Pass 5).
- **Dependencies**: `petgraph::graph::DiGraph`, `petgraph::algo::tarjan_scc`, native Rust data structures (`indexmap::IndexMap`, `indexmap::IndexSet`), [`f8_core::expr::ast`](../../../src/expr/ast.rs), and [`f8_core::compiler::ir`](../../../src/compiler/ir/mod.rs).

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

Relative temporal predecessor and successor offsets (`col.prev(k)`, `col.next(k)`, `row.prev(k)`, `row.next(k)`) calculate relative position steps along the origin table's measure columns or data rows (strictly protecting descriptor column boundaries and skipping gutters), resolving to the corresponding coordinate identifier.

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

### 4. Range Interval Slices

Closed intervals (`"2024E".."2026E"`) expand into ordered sequences of coordinates based on axis declaration order.

#### Scenarios

- **Horizontal Column Slice**:
  - **When** evaluating a reference `T["rev", "2024E".."2026E"]`
  - **Then** resolves to `[T["rev", "2024E"], T["rev", "2025E"], T["rev", "2026E"]]`.
- **Inverted Range Rejected**:
  - **When** a slice specifies $start\_idx > end\_idx$ (e.g. `"2026E".."2024E"`)
  - **Then** compilation halts with `ResolveError::InvertedRange`.
- **2D Block Range**:
  - **When** a reference spans both a row range and a column range ($R \times C$)
  - **Then** resolves to all Cartesian product `CellId` nodes in row-major declaration order.

### 5. Zero Circularity & Tarjan SCC Firewall

Directed edge convention represents precedence: an edge $A \to B$ signifies that cell $B$ references and depends on cell $A$ ($A$ is an input to $B$). f8 enforces Section 2.01 of the FAST Standard by detecting and rejecting all circular dependency cycles.

#### Scenarios

- **Direct Two-Cell Cycle Rejected**:
  - **When** cell $A$ references $B$, and cell $B$ references $A$ ($A \leftrightarrow B$)
  - **Then** compilation halts with `GraphError::CircularDependency` and an actionable numbered semantic trace.
- **Single-Cell Self-Reference Rejected**:
  - **When** cell $A$ references itself in its own formula ($A \to A$)
  - **Then** compilation halts with `GraphError::CircularDependency` showing the self-loop.
- **Multi-Step Indirect Cycle Path**:
  - **When** a cyclic loop spans multiple tables ($A \to B \to C \to A$)
  - **Then** Tarjan SCC isolates the component, DFS reconstructs the closed directed loop, and the compiler outputs the exact sequence.

### 6. Deterministic Topological Evaluation Order

Cells are sorted into a single linear evaluation sequence via Kahn's algorithm with declaration-order tie-breaking.

#### Scenarios

- **Precedence Preservation**:
  - **When** an acyclic model contains dependency chains
  - **Then** every prerequisite cell $A$ strictly precedes its dependent cell $B$ in `topological_order`.
- **Declaration Stability**:
  - **When** multiple cells have zero unsatisfied dependencies (e.g. independent literal assumptions across periods)
  - **Then** they evaluate deterministically in their original table and axis declaration order.
