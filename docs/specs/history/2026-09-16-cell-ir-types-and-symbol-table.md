---
title: Cell IR Data Structures & Symbol Table
date: 2026-09-16
status: completed
target-living-spec: specs/living-specs/compiler/cell-ir.md
---

## Proposal

### What

Implement the foundational cell-level Intermediate Representation (IR) data structures, cell identity model, cell collection container, and workbook symbol table in `finx::compiler::ir`.

### Why

The Finx compilation pipeline requires discrete, cell-level granularity (`CellId = (TableId, RowId, ColId)`) to:
1. Prevent false-positive cycle detection in inter-period roll-forwards (such as corkscrew balance calculations).
2. Maintain clean separation between high-level declarative table specifications and cell-level calculation nodes.
3. Provide the node matrix required by downstream passes: dependency graph construction (`compiler::graph`), topological evaluation (`compiler::eval`), and spatial lowering (`compiler::lowering`).

By establishing these typed structures and lookup indices in Phase 2.1, Phase 2.2 (Grid Expansion Engine) can focus purely on the unrolling transformation without conflating data representation with algorithm logic.

### Scope

- **In-scope**:
  - Add `SheetId` strongly typed identifier in `model::id`.
  - Canonical `CellId` structure with `Display` format (`Table.row["col"]`), hashing, and deterministic ordering.
  - `CellPayload` enum representing either `Literal(ValueLiteral)` or `Formula(Expr)`.
  - `CellNode` structure containing cell coordinate, parent sheet ID, format preset, payload, and optional metadata.
  - `CellGrid` collection backed by `IndexMap<CellId, CellNode>` with indexed lookups by table, row, and column.
  - `SymbolTable` registering table-to-sheet ownership, column ordering, and row catalogs.
  - Structured domain error type `IrError` using `thiserror`.
- **Out-of-scope**:
  - Grid expansion unrolling algorithm and match arm partitioning (deferred to Phase 2.2).
  - Dependency graph construction and Tarjan SCC cycle detection (Pass 3, `compiler::graph`).
  - Coordinate lowering to concrete Excel cell coordinates `$A$1` (Pass 6, `compiler::lowering`).

### What Changes

- `src/model/id.rs`: Define `SheetId` via `define_id!` macro.
- `src/model/mod.rs`: Re-export `SheetId`.
- `src/compiler/ir/mod.rs`: Re-export IR public types (`CellId`, `CellPayload`, `CellNode`, `CellGrid`, `SymbolTable`, `IrError`).
- `src/compiler/ir/cell.rs`: Implement `CellId`, `CellPayload`, `CellNode`, and `CellGrid`.
- `src/compiler/ir/symbol.rs`: Implement `SymbolTable` and table metadata registry.
- `src/compiler/ir/error.rs`: Implement `IrError`.
- `tests/cell_ir_test.rs`: Comprehensive test suite verifying data structures, ordering, formatting, and symbol lookup.

### Capabilities

- **New capabilities**:
  - Unambiguously identify any cell across the workbook using `CellId(TableId, RowId, ColId)`.
  - Query cells by table, row, or column from `CellGrid` with deterministic insertion ordering.
  - Map tables to sheets and validate table name uniqueness across sheets using `SymbolTable`.
  - Display cell references matching user-facing DSL syntax (`IncomeStatement.revenue["2024E"]`).
- **Modified capabilities**: None (new subsystem under `compiler::ir`).

## Design

### Goals

1. **Zero production panics**: Pure error propagation with `IrError` and `Result`.
2. **Deterministic ordering**: Use `IndexMap` / `IndexSet` to guarantee reproducible cell iteration and serialization.
3. **Explicit data flow**: Provide pure, inspectable query methods on `CellGrid` and `SymbolTable`.
4. **Clean integration**: Seamlessly integrate `ValueLiteral` from `model::table` and `Expr` from `expr::ast`.

### Invariants

1. **Workbook table uniqueness**: A `TableId` can belong to exactly one sheet across the entire workbook. Registering the same `TableId` multiple times is rejected.
2. **Cell uniqueness**: A `CellId` cannot be inserted into `CellGrid` more than once.
3. **Canonical display parity**: `CellId::to_string()` must strictly format as `{table}.{row}["{col}"]`.

### Decisions

- **Choice**: Store cells in an `IndexMap<CellId, CellNode>` within `CellGrid`.
  - **Rationale**: Preserves deterministic insertion order (critical for reproducible `.xlsx` emission and evaluation traces) while providing $O(1)$ key lookup.
  - **Alternatives considered**: Standard `HashMap` (non-deterministic ordering) or `BTreeMap` (requires `Ord` overhead and loses declaration order).
- **Choice**: Use strongly typed `SheetId` rather than bare `String`.
  - **Rationale**: Type safety parity with `TableId`, `RowId`, and `ColId`, preventing accidental argument transposition.
  - **Alternatives considered**: Bare `String` (error-prone when passing multiple string-like identifiers).
- **Choice**: Direct AST inclusion in `CellPayload::Formula(Expr)`.
  - **Rationale**: `Expr` is already fully validated and strongly typed, allowing downstream compiler passes to inspect AST nodes directly without re-parsing.
  - **Alternatives considered**: Storing raw formula strings (defers parse errors to later passes and violates early validation).

### Data Contracts

#### Cell Identity & Node Structure

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CellId {
    pub table: TableId,
    pub row: RowId,
    pub col: ColId,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CellPayload {
    Literal(ValueLiteral),
    Formula(Expr),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellNode {
    pub id: CellId,
    pub sheet: SheetId,
    pub format: Option<FormatPreset>,
    pub payload: CellPayload,
    pub note: Option<String>,
    pub source: Option<serde_json::Value>,
}
```

#### Symbol Table Structure

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableMetadata {
    pub table_id: TableId,
    pub sheet_id: SheetId,
    pub columns: Vec<ColId>,
    pub column_index: IndexMap<ColId, usize>,
    pub rows: IndexSet<RowId>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SymbolTable {
    tables: IndexMap<TableId, TableMetadata>,
    sheet_tables: IndexMap<SheetId, Vec<TableId>>,
}
```

### Risks & Trade-offs

- **Memory overhead of individual `CellNode`s**: In models with tens of thousands of cells, individual node allocations could increase memory footprint.
  - *Mitigation*: Cells are stored contiguously in `IndexMap`, and identifier strings share memory or are small. Benchmarks can assess arena allocation later if needed.
- **Index synchronization**: `SymbolTable` maintains reverse mappings (`sheet_tables`).
  - *Mitigation*: Encapsulate state mutations behind `register_table` methods to ensure atomic synchronization.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: CellId display and formatting**
  - **Given** a `CellId` with table `"IncomeStatement"`, row `"revenue"`, and column `"2024E"`
  - **When** calling `cell_id.to_string()` or formatting with `Display`
  - **Then** output is exactly `IncomeStatement.revenue["2024E"]`.

- [x] **Scenario 2: CellGrid insertion and lookup**
  - **Given** an empty `CellGrid`
  - **When** inserting a `CellNode` with `CellPayload::Literal(ValueLiteral::Number(100.0))`
  - **Then** `grid.get(&id)` returns the cell node, and `grid.len()` is 1.

- [x] **Scenario 3: CellGrid duplicate rejection**
  - **Given** a `CellGrid` containing a cell with `CellId("T", "R", "C")`
  - **When** attempting to insert another cell with the same `CellId`
  - **Then** `grid.insert(cell)` returns `Err(IrError::DuplicateCell { .. })`.

- [x] **Scenario 4: CellGrid filtered iteration**
  - **Given** a `CellGrid` containing multiple cells across tables `T1` and `T2`
  - **When** calling `grid.cells_for_table(&TableId::new("T1"))`
  - **Then** only cells belonging to table `T1` are yielded in insertion order.

- [x] **Scenario 5: SymbolTable table registration**
  - **Given** an empty `SymbolTable` and a `TableSpec` with columns `["2024E", "2025E"]`
  - **When** registering the table under sheet `"Model"`
  - **Then** `symbols.get_sheet(&table_id)` returns `Some(&SheetId::new("Model"))`, and column index lookups return `0` and `1`.

- [x] **Scenario 6: SymbolTable duplicate table rejection**
  - **Given** a `SymbolTable` with table `"IncomeStatement"` registered on sheet `"Model"`
  - **When** attempting to register `"IncomeStatement"` again on sheet `"Summary"`
  - **Then** returns `Err(IrError::DuplicateTable { .. })`.

## Tasks

- [x] **Phase 1: Identifier and error definitions**
  - [x] Add `SheetId` to `src/model/id.rs` and re-export in `src/model/mod.rs`.
  - [x] Create `src/compiler/ir/error.rs` defining `IrError`.
  - [x] Create `src/compiler/ir/id.rs` defining `CellId` with `Display`, `Hash`, `Ord`.

- [x] **Phase 2: Cell structures and CellGrid collection**
  - [x] Create `src/compiler/ir/cell.rs` defining `CellPayload`, `CellNode`, and `CellGrid`.
  - [x] Implement query methods on `CellGrid` (`get`, `contains`, `cells_for_table`, `cells_for_row`, `cells_for_col`, `keys`).
  - [x] Re-export types in `src/compiler/ir/mod.rs`.

- [x] **Phase 3: Symbol table registry**
  - [x] Create `src/compiler/ir/symbol.rs` defining `SymbolTable` and `TableMetadata`.
  - [x] Implement `register_table` validating uniqueness and indexing column/row positions.
  - [x] Implement query methods (`get_sheet`, `col_index`, `row_index`, `tables_on_sheet`, `sheet_ids`).

- [x] **Phase 4: Verification suite**
  - [x] Create unit tests in `src/compiler/ir/` for `CellId`, `CellGrid`, and `SymbolTable`.
  - [x] Create integration test `tests/cell_ir_test.rs` covering all acceptance scenarios.
  - [x] Verify using `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`.
