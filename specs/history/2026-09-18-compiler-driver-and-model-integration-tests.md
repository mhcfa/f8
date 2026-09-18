---
title: Top-Level Compiler Driver & End-to-End Model Integration Tests
date: 2026-09-18
status: completed
target-living-spec: specs/living-specs/compiler/evaluator.md
---

## Proposal

### What

Introduce a unified compiler driver facade ([`Compiler::evaluate`](../../src/compiler/mod.rs) and `Compiler::evaluate_with_registry`) and a unified error hierarchy ([`CompilerError`](../../src/compiler/error.rs)) that seamlessly orchestrates Passes 1 through 4 (Spec Validation $\rightarrow$ Grid Expansion $\rightarrow$ Dependency Graph / Cycle Detection $\rightarrow$ Topological In-Memory Evaluation). 

Validate the closed authoring-to-in-memory calculation pipeline by authoring two rich, realistic end-to-end integration test suites:
1. **Simplified 3-Statement Financial Model** (`tests/three_statement_model_test.rs`): Income Statement, Cash Flow Statement, and Balance Sheet across historical (`2023A`) and forecast (`2024E`..`2026E`) periods, utilizing multi-table cross-references, temporal predecessor lags (`col.prev`), and verifying that the Balance Sheet balances (`Assets == Liabilities + Equity`) for every period.
2. **Orthogonal Column-Based Cap Table Model** (`tests/cap_table_test.rs`): Financing rounds/shareholders as entity rows, calculation metrics as columns, exercising column vector formulas (`col.expr`), active row selectors (`@row`), vertical range aggregations via `SUM`, and ownership percentage calculations.

### Why

Currently, running the compilation pipeline requires manual plumbing across 5 distinct subsystem calls:
```rust
manifest.validate()?;
for table in tables { table.validate()?; }
let (symbol_table, grid) = expand_workbook(manifest, tables)?;
let graph = DependencyGraph::from_grid(&grid, &symbol_table)?;
let model = Evaluator::evaluate(&grid, &graph, &symbol_table, registry)?;
```
A single, ergonomic compiler facade eliminates caller boilerplate and establishes a clean public interface.

Furthermore, while Passes 1–4 have passing unit tests for individual features, the engine has not yet been stress-tested on complete, interconnected financial models. Building realistic 3-statement and cap table models will verify whether the semantic authoring abstractions, reference resolution engine, temporal offsets, and DAG evaluator behave correctly together on real financial modeling paradigms before tackling OpenXML lowering and emission.

### Scope

- **In-scope**:
  - `src/compiler/error.rs`: Structured [`CompilerError`](../../src/compiler/error.rs) wrapping `ModelError`, `IrError`, `GraphError`, and `EvalError`.
  - `src/compiler/mod.rs`: Top-level [`Compiler`](../../src/compiler/mod.rs) facade with `evaluate` and `evaluate_with_registry`.
  - Re-exports in `src/lib.rs`.
  - `tests/three_statement_model_test.rs`: 3-statement financial model integration test.
  - `tests/cap_table_test.rs`: Column-based cap table integration test.
- **Out-of-scope**:
  - Passes 5 and 6 (Spatial Layout Planning, Formula Lowering, and `.xlsx` Emission).
  - Disk-based file discovery and ingestion (`finx.json` on disk).
  - Additional functions beyond the existing standard catalog (`ABS`, `SUM`, `IF`).

### What Changes

- **New files**:
  - `src/compiler/error.rs`: Defines [`CompilerError`](../../src/compiler/error.rs).
  - `tests/three_statement_model_test.rs`: Multi-table 3-statement financial model tests.
  - `tests/cap_table_test.rs`: Dual-axis column vector formula cap table tests.
- **Modified files**:
  - `src/compiler/mod.rs`: Re-export `Compiler`, `CompilerError`, and implement compilation orchestration.
  - `src/lib.rs`: Re-export top-level compiler symbols (`Compiler`, `CompilerError`).

### Capabilities

- **New capabilities**:
  - Callers can compile and evaluate an entire model from specs to `EvaluatedModel` with a single function call: `Compiler::evaluate(&manifest, &tables)`.
  - Integration tests prove that multi-table models with temporal lags, cross-table links, and column vector rules compile and evaluate deterministically.
- **Modified capabilities**: None (existing fine-grained pass APIs remain available for inspection).

---

## Design

### Goals

1. **Zero boilerplate**: Single-call compilation from `ProjectManifest` + `&[TableSpec]` to `EvaluatedModel`.
2. **Unified error propagation**: Clean domain and compiler error bubbling via `CompilerError` without unhandled panics.
3. **End-to-end accounting validation**: Prove that standard 3-statement financial accounting flows (Income Statement $\rightarrow$ Cash Flow $\rightarrow$ Balance Sheet) calculate cleanly in topological order.
4. **2D matrix flexibility**: Prove that column-oriented vector calculations with `@row` selectors and `SUM` ranges calculate cleanly.

### Invariants

1. **Strict FAST DAG compliance**: No circular dependencies allowed. Cash/debt or retained earnings loops must be resolved linearly via historical lags (`col.prev`).
2. **Zero production panics**: No `.unwrap()` or `.expect()` outside tests; errors propagate via `?`.
3. **Excel numerical parity**: Mathematical calculations adhere to IEEE 754 parity and Excel evaluation semantics.

### Decisions

- **Choice: Static methods on `Compiler` struct**:
  - `Compiler::evaluate(&manifest, &tables)` defaults to `FunctionRegistry::standard()`.
  - `Compiler::evaluate_with_registry(&manifest, &tables, &registry)` permits custom function injection.
  - *Rationale*: Clean, idiomatic Rust API that prepares for future pass options (e.g. layout/emission options) while remaining simple and stateless.
- **Choice: Exhaustive `CompilerError` enum**:
  - Wraps `Model(ModelError)`, `Ir(IrError)`, `Graph(GraphError)`, and `Eval(EvalError)` using `thiserror`.
  - *Rationale*: Preserves rich diagnostic information from whichever pass encounters a failure.

### Data Contracts

#### Compiler Error Enum

```rust
#[derive(Debug, thiserror::Error)]
pub enum CompilerError {
    #[error("Specification validation error: {0}")]
    Model(#[from] crate::model::error::ModelError),

    #[error("Intermediate representation error: {0}")]
    Ir(#[from] crate::compiler::ir::error::IrError),

    #[error("Dependency graph error: {0}")]
    Graph(#[from] crate::compiler::graph::error::GraphError),

    #[error("Evaluation error: {0}")]
    Eval(#[from] crate::compiler::eval::error::EvalError),
}
```

#### Compiler Facade

```rust
pub struct Compiler;

impl Compiler {
    pub fn evaluate(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
    ) -> Result<EvaluatedModel, CompilerError>;

    pub fn evaluate_with_registry(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        registry: &FunctionRegistry,
    ) -> Result<EvaluatedModel, CompilerError>;
}
```

---

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Unified Compiler Facade Execution**
  - **Given** a valid `ProjectManifest` and list of `TableSpec`s
  - **When** calling `Compiler::evaluate(manifest, tables)`
  - **Then** successfully returns an `EvaluatedModel` with all cells populated in topological order.

- [x] **Scenario 2: Unified Error Propagation**
  - **Given** an invalid specification (e.g., duplicate sheet name, unknown reference, or circular dependency)
  - **When** calling `Compiler::evaluate(manifest, tables)`
  - **Then** returns the corresponding `CompilerError` variant (`Model`, `Ir`, `Graph`, or `Eval`).

- [x] **Scenario 3: Simplified 3-Statement Financial Model**
  - **Given** an `Assumptions` table, an `IncomeStatement` table, a `CashFlow` table, and a `BalanceSheet` table across `2023A`, `2024E`, `2025E`, `2026E`
  - **When** evaluating the model
  - **Then** historical cells use point inputs, forecast cells chain temporally via `col.prev`, Net Income links to Cash Flow and Retained Earnings, Ending Cash links to Balance Sheet Cash, and `Assets - (Liabilities + Equity) == 0.0` holds for every period.

- [x] **Scenario 4: Orthogonal Column-Based Cap Table**
  - **Given** a `CapTable` with shareholder rows (`founders`, `seed`, `series_a`, `option_pool`, `total`) and measure columns (`shares`, `pps`, `capital`, `ownership_pct`) where `capital` and `ownership_pct` use column vector formulas with `@row`
  - **When** evaluating the model
  - **Then** invested capital equals `shares * pps` for each shareholder row, total shares equals the sum of shareholder rows, and ownership percentages sum to 100% (`1.0`).

---

## Tasks

### Phase 1: Top-Level Compiler Facade & Error Type
- [x] Create `src/compiler/error.rs` with `CompilerError` using `thiserror`.
- [x] Implement `Compiler::evaluate` and `Compiler::evaluate_with_registry` in `src/compiler/mod.rs`.
- [x] Export `Compiler` and `CompilerError` in `src/lib.rs` and `src/compiler/mod.rs`.

### Phase 2: Simplified 3-Statement Financial Model Integration Test
- [x] Create `tests/three_statement_model_test.rs`.
- [x] Define `Assumptions`, `IncomeStatement`, `CashFlow`, and `BalanceSheet` tables.
- [x] Wire cross-table formulas and temporal lags (`col.prev`).
- [x] Assert exact calculated values and verify that Balance Sheet balances across all periods.

### Phase 3: Orthogonal Column-Based Cap Table Integration Test
- [x] Create `tests/cap_table_test.rs`.
- [x] Define shareholder rows and measure columns with column vector rules (`col.expr`).
- [x] Use active row selector (`@row`) and vertical `SUM` aggregation range.
- [x] Assert exact calculated share counts, capital amounts, and ownership percentages.

### Phase 4: Verification & Reconciliation
- [x] Run `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`.
- [x] Update living spec `specs/living-specs/compiler/evaluator.md` with the new `Compiler` facade.
- [x] Move completed draft spec to `specs/history/`.

