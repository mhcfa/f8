---
title: In-Memory Topological Evaluator & Evaluated Model Matrix
date: 2026-09-18
status: completed
target-living-spec: specs/living-specs/compiler/evaluator.md
---

## Proposal

### What

Implement the Pass 4B **In-Memory Topological Evaluator** (`finx::compiler::eval::Evaluator`), the evaluated runtime cell matrix (`finx::compiler::eval::EvaluatedModel`), symbol-table integrated model inspection APIs (`get_value`, `get_row_values` preserving declared column order, and `find_errors`), and end-to-end multi-table model integration tests.

### Why

1. **Headless model calculation**: Executes the full cell grid in verified DAG topological order, guaranteeing that predecessor cells are calculated before dependents.
2. **Declared column order query APIs**: `EvaluatedModel` links the calculated cell matrix with the `SymbolTable`, providing client and CLI query APIs that return columns in model declaration order, rather than DAG evaluation order.
3. **Precalculated workbook caching**: Packages a self-contained evaluated model artifact consumed by Pass 5 (Layout) and Pass 6 (Emission) for precomputed `.xlsx` XML cell value injection.
4. **Compile-time domain error diagnostics**: Provides `find_errors` to identify all cells containing `#DIV/0!`, `#VALUE!`, `#NUM!`, `#REF!` before workbook emission.

### Scope

- **In-scope**:
  - `src/compiler/eval/model.rs`:
    - `EvaluatedModel` struct storing `values: IndexMap<CellId, CellValue>` and `symbol_table: SymbolTable`.
    - Query APIs:
      - `get_value(&self, table: &TableId, row: &RowId, col: &ColId) -> Option<&CellValue>`
      - `get_cell(&self, id: &CellId) -> Option<&CellValue>`
      - `get_row_values(&self, table: &TableId, row: &RowId) -> Result<IndexMap<ColId, CellValue>, EvalError>` (preserving declared column order)
      - `find_errors(&self) -> Vec<(&CellId, &ExcelError)>`
    - Iteration accessors: `iter(&self)`, `len(&self)`, `is_empty(&self)`, `symbol_table(&self) -> &SymbolTable`.
  - `src/compiler/eval/error.rs`:
    - Extend `EvalError` with model query error variants: `TableNotFound(TableId)`, `RowNotFound { table: TableId, row: RowId }`.
  - `src/compiler/eval/mod.rs`:
    - Implement `Evaluator::evaluate(grid: &CellGrid, graph: &DependencyGraph, symbol_table: &SymbolTable, registry: &FunctionRegistry) -> Result<EvaluatedModel, EvalError>`:
      - Iterates `graph.topological_order()`.
      - For `CellPayload::Literal(lit)`: converts directly via `CellValue::from(lit)`.
      - For `CellPayload::Formula(expr)`: evaluates via `eval_expr` against accumulated values.
      - Returns `EvaluatedModel::new(values, symbol_table.clone())`.
    - Public re-exports of `EvaluatedModel` and `Evaluator`.
  - Comprehensive end-to-end integration tests in `tests/evaluator_test.rs`.
  - Promotion of living spec to `specs/living-specs/compiler/evaluator.md` and archiving draft specs.
- **Out-of-scope**:
  - Layout planning and freeze pane assignment (Pass 5).
  - Excel `.xlsx` emission (Pass 6).

### What Changes

- `src/compiler/eval/model.rs`: Implement `EvaluatedModel` and query methods with `SymbolTable` integration.
- `src/compiler/eval/error.rs`: Add `TableNotFound` and `RowNotFound` variants to `EvalError`.
- `src/compiler/eval/mod.rs`: Implement `Evaluator::evaluate` and export `EvaluatedModel` and `Evaluator`.
- `src/compiler/mod.rs`: Re-export `EvaluatedModel`, `Evaluator`, and `EvalError`.
- `tests/evaluator_test.rs`: End-to-end integration tests for Pass 4.

### Capabilities

- **New capabilities**:
  - Topologically evaluating complete multi-table financial models directly in memory.
  - Querying evaluated cell coordinates, entire row slices in declared column order, and isolating all Excel domain errors via `EvaluatedModel`.
  - Inspecting model values via clean, fail-safe APIs returning structured errors for missing tables or rows.
- **Modified capabilities**: None (depends on Pass 4A expression evaluator).

---

## Design

### Goals

1. **Deterministic execution**: Topologically ordered execution produces bit-for-bit identical results across runs.
2. **Self-contained compilation artifact**: `EvaluatedModel` encapsulates both the evaluated values and the `SymbolTable` to supply all structural and numeric data to downstream compiler passes.
3. **Zero production panics**: Zero unwrap or panic in non-test code; propagate all failures cleanly.

### Invariants

1. **DAG evaluation guarantee**: Every referenced predecessor cell is guaranteed to be evaluated before dependent cells.
2. **Column declaration order**: `get_row_values` always returns columns in the exact order declared in the model specification, regardless of DAG calculation order.
3. **Domain error persistence**: Cells evaluating to runtime errors (`#DIV/0!`, `#VALUE!`, `#NUM!`) are successfully stored in `EvaluatedModel` without halting compilation.

### Decisions

- **Decision 1: Self-contained `EvaluatedModel` with `SymbolTable`**:
  - *Choice*: `EvaluatedModel` stores a cloned `SymbolTable` alongside the evaluated `IndexMap<CellId, CellValue>`.
  - *Rationale*: `get_row_values` must return columns in declared table column order (`table_meta.columns`), not in topological calculation order. Retaining `SymbolTable` also allows `EvaluatedModel` to validate table/row existence (`EvalError::TableNotFound`, `EvalError::RowNotFound`) and pass all structural model metadata self-contained into Pass 5 (Layout) and Pass 6 (Emission).
  - *Alternatives considered*: Passing `&SymbolTable` into `get_row_values` on every call (rejected: clutters API and fragments model state).

- **Decision 2: Sequential whole-model topological evaluation**:
  - *Choice*: Walk `DependencyGraph::topological_order()` sequentially, accumulating evaluated cells into an `IndexMap<CellId, CellValue>`.
  - *Rationale*: The topological order guarantees that each cell evaluates exactly once with zero redundant recomputation. Compact enum representation (`CellValue`) keeps memory usage well under 100 MB even for large 100,000-cell models.

### Data Contracts

#### `EvaluatedModel` (`src/compiler/eval/model.rs`)

```rust
pub struct EvaluatedModel {
    values: IndexMap<CellId, CellValue>,
    symbol_table: SymbolTable,
}

impl EvaluatedModel {
    pub fn new(values: IndexMap<CellId, CellValue>, symbol_table: SymbolTable) -> Self;
    pub fn get_value(&self, table: &TableId, row: &RowId, col: &ColId) -> Option<&CellValue>;
    pub fn get_cell(&self, id: &CellId) -> Option<&CellValue>;
    pub fn get_row_values(&self, table: &TableId, row: &RowId) -> Result<IndexMap<ColId, CellValue>, EvalError>;
    pub fn find_errors(&self) -> Vec<(&CellId, &ExcelError)>;
    pub fn symbol_table(&self) -> &SymbolTable;
    pub fn iter(&self) -> impl Iterator<Item = (&CellId, &CellValue)>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

#### `Evaluator` (`src/compiler/eval/mod.rs`)

```rust
pub struct Evaluator;

impl Evaluator {
    pub fn evaluate(
        grid: &CellGrid,
        graph: &DependencyGraph,
        symbol_table: &SymbolTable,
        registry: &FunctionRegistry,
    ) -> Result<EvaluatedModel, EvalError>;
}
```

### Risks & Trade-offs

- **Memory overhead of retaining `SymbolTable`**:
  - *Risk*: Retaining a cloned `SymbolTable` consumes additional memory.
  - *Mitigation*: `SymbolTable` contains lightweight metadata (vectors of identifiers and column/row specifications), negligible compared to full cell value grids. Packaging it in `EvaluatedModel` provides a self-contained compilation artifact consumed directly by downstream Layout and Emission passes without global state.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Topological Evaluation of Pure Literal Grid**
  - **Given** a `CellGrid` containing only literal values (numbers, strings, booleans, and nulls)
  - **When** evaluated via `Evaluator::evaluate`
  - **Then** produces an `EvaluatedModel` with matching `CellValue` entries and zero errors.

- [x] **Scenario 2: Multi-Table Topological Evaluation with Cross-References**
  - **Given** a model with tables `Assumptions` and `IncomeStatement`, where `IncomeStatement` formulas reference `Assumptions` and previous columns
  - **When** evaluated via `Evaluator::evaluate`
  - **Then** calculates all cells in exact topological order, correctly propagating values across tables.

- [x] **Scenario 3: Model Query APIs and Column Ordering**
  - **Given** an `EvaluatedModel` computed from a multi-table model
  - **When** querying `get_value`, `get_row_values`, and `find_errors`
  - **Then** `get_row_values` returns values in exact declared column order, returns `Err(EvalError::TableNotFound)` or `Err(EvalError::RowNotFound)` for invalid lookups, and `find_errors` accurately isolates all cells containing `ExcelError`.

---

## Tasks

### Phase 1: Evaluated Model Data Structure & Query APIs

- [x] Implement `EvaluatedModel` in `src/compiler/eval/model.rs` storing `values` and `symbol_table`.
- [x] Implement query methods: `get_value`, `get_cell`, `get_row_values` (preserving declared column order), `find_errors`, and iteration accessors.
- [x] Add `TableNotFound` and `RowNotFound` variants to `EvalError` in `src/compiler/eval/error.rs`.

### Phase 2: Topological Execution Loop

- [x] Implement `Evaluator::evaluate` in `src/compiler/eval/mod.rs` iterating `DependencyGraph::topological_order()`.
- [x] Re-export `Evaluator`, `EvaluatedModel`, and `EvalError` in `src/compiler/eval/mod.rs` and `src/compiler/mod.rs`.

### Phase 3: Integration Verification Suite

- [x] Create `tests/evaluator_test.rs` covering:
  - Pure literal grid evaluation.
  - Multi-table cross-referencing models and active column lowering.
  - Error propagation across formula dependencies.
  - Query APIs (`get_row_values` preserving column order, `TableNotFound`, `RowNotFound`, `find_errors`).
- [x] Run verification suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- [x] Reconcile and promote living spec to `specs/living-specs/compiler/evaluator.md`, archive draft specs to `specs/history/`.
