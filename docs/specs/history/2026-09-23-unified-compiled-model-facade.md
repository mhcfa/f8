---
title: Unified CompiledModel Inspection Facade
date: 2026-09-23
status: complete
target-living-spec: specs/living-specs/compiler/compiled-model.md
---

## Proposal

### What

Introduce the unified compiler inspection facade (`CompiledModel`) in `crates/f8-core`. Consolidate all compilation pass artifacts into a single in-memory structure produced by a single linear compilation pass (`Compiler::compile`), and expose pure in-memory inspection and emission methods:
- **Linear compilation orchestration**: `Compiler::compile` executes Passes 1–6a exactly once, returning a `CompiledModel` holding the validated model, theme, symbol table, cell grid, dependency graph, evaluated values, spatial layout map, and lowered Excel formulas.
- **Fast zero-redundancy emission**: `CompiledModel::emit_workbook` emits native `.xlsx` byte buffers directly from already-computed artifacts without re-evaluating or re-expanding the model.
- **Typed inspection APIs**: Pure in-memory queries for model overview summaries (`summary`), cell-level audits (`inspect_cell`), 2D table matrices (`inspect_table`), root-cause calculation trees (`precedents_of`), and downstream blast-radius impact analysis (`dependents_of`).
- **Fail-fast inspection diagnostics**: Structured, coordinate-rich `InspectionError` propagating missing table, cell, or sheet lookups cleanly without panics.

### Why

Currently, high-level methods on `Compiler` (`crates/f8-core/src/compiler/mod.rs`) (`evaluate`, `plan_layout`, `lower_formulas`, `emit_workbook`) each independently re-validate manifests and re-expand intermediate representations:
- **Redundant re-computation**: `Compiler::emit_workbook` runs specification validation and IR expansion twice (once in `evaluate` and once in `prepare_emission_context`), wasting CPU cycles.
- **Missing shared state for outer interfaces**: Upcoming interface crates (`f8-cli` and `f8-mcp`) need to answer cell, table, precedent, and dependent inspection queries without re-running passes or maintaining fragmented compiler state.
- **Single source of truth**: Retaining all pass artifacts in a cohesive `CompiledModel` establishes a clean public boundary between the functional compiler core and imperative presentation adapters, ensuring outer tools never assemble or manipulate compiler internals directly.

### Scope

- **In-scope**:
  - `CompiledModel` struct in `crates/f8-core/src/compiler/compiled/` holding all compilation pass artifacts.
  - `Compiler::compile` and `Compiler::compile_with_registry` orchestrating Passes 1–6a.
  - `CompiledModel::emit_workbook` delegating directly to `WorkbookEmitter` with zero redundant passes.
  - Inspection query APIs on `CompiledModel`: `summary()`, `inspect_cell(...)`, `inspect_table(...)`, `precedents_of(...)`, `dependents_of(...)`.
  - Dedicated inspection data contracts: `ModelSummary`, `SheetSummary`, `TableSummary`, `CellInspection`, `TableInspection`, `ColumnInspection`, `RowInspection`, `TableCellInfo`, `DependencyTree`, `DependencyNode`.
  - `InspectionError` enum integrated into `CompilerError`.
  - Comprehensive unit and integration test suite in `crates/f8-core/tests/compiled_model_test.rs`.
  - Backward-compatible retention of existing `Compiler::evaluate`, `Compiler::plan_layout`, `Compiler::lower_formulas`, and `Compiler::emit_workbook`.

- **Out-of-scope**:
  - Terminal CLI commands, argument parsing, and Unicode table rendering in `crates/f8-cli` (scheduled for Phase 2 CLI milestone).
  - JSON-RPC stdio protocol and MCP tool definitions in `crates/f8-mcp` (scheduled for Phase 3 MCP milestone).
  - WebAssembly bindings and JS wrappers in `crates/f8-wasm` (scheduled for Phase 4 WASM milestone).
  - Filesystem disk loading or file writes (already strictly decoupled).

### What Changes

- `crates/f8-core/src/compiler/compiled/`:
  - `mod.rs`: `CompiledModel` facade struct, constructors, and query implementations.
  - `model.rs`: Public inspection data contracts (`ModelSummary`, `CellInspection`, `TableInspection`, `DependencyTree`, etc.).
  - `error.rs`: `InspectionError` domain enum.
- `crates/f8-core/src/compiler/error.rs`:
  - Add `CompilerError::Inspection(#[from] InspectionError)`.
- `crates/f8-core/src/compiler/mod.rs`:
  - Expose `compiled` module.
  - Add `Compiler::compile` and `Compiler::compile_with_registry`.
  - Refactor `Compiler::emit_workbook` to delegate to `CompiledModel`.
  - Re-export `CompiledModel` and primary inspection contracts.
- `crates/f8-core/src/lib.rs`:
  - Re-export `CompiledModel`, `CellInspection`, `TableInspection`, `DependencyTree`, and `ModelSummary`.
- `crates/f8-core/tests/compiled_model_test.rs`:
  - End-to-end unit and integration test suite covering compilation, emission, inspection queries, and error handling.

### Capabilities

- **New capabilities**:
  - `Compiler::compile` builds and returns a complete `CompiledModel` in a single pass.
  - `CompiledModel::emit_workbook` emits `.xlsx` byte buffers with zero redundant pass executions.
  - `CompiledModel::summary` provides hierarchical model, sheet, and table counts and driver/formula ratios.
  - `CompiledModel::inspect_cell` resolves coordinates to physical sheet addresses, evaluated values, formatting, formula ASTs, lowered Excel formulas, and incoming/outgoing edges.
  - `CompiledModel::inspect_table` outputs full 2D table structures with headers, line items, and cell details for terminal and markdown rendering.
  - `CompiledModel::precedents_of` and `CompiledModel::dependents_of` construct hierarchical calculation trees for upstream root-cause and downstream blast-radius analysis.
- **Modified capabilities**:
  - `Compiler::emit_workbook` internally compiles and emits in one pass while preserving its identical public function signature.

---

## Design

### Goals

1. **Single-pass compilation**: Transform input manifests and table specs into all pass artifacts in a single sequential execution of Passes 1–6a.
2. **Zero redundant emission**: Generate `.xlsx` workbooks directly from already-computed artifacts without re-evaluating or re-expanding.
3. **Pure in-memory inspection**: Expose deterministic, strongly typed query methods operating purely in RAM with $O(1)$ lookups and $O(V+E)$ tree traversals.
4. **Clean separation of concerns**: Keep presentation formatting (ASCII/Unicode tables, tree branches) in outer adapters (`f8-cli`), while providing structured data from `f8-core`.
5. **Zero production panics**: Validate all queries and propagate coordinate-rich `InspectionError` variants on missing entities.

### Invariants

1. **Deterministic order**: All collection fields in inspection contracts preserve declared column and row ordering via `IndexMap` / `Vec`.
2. **Acyclic dependency trees**: Precedent and blast-radius traversals guarantee finite tree generation based on Pass 3 DAG acyclicity proofs.
3. **Single source of truth**: Inspection query results reflect the exact evaluated values, layout coordinates, and lowered formulas used for workbook emission.

### Decisions

- **Choice 1: Eager consolidated artifact over lazy on-demand pass execution**:
  - *Rationale*: Financial models in f8 evaluate in under 2 milliseconds. Eagerly executing Passes 1–6a during `Compiler::compile` guarantees that any specification, cycle, layout, or lowering error fails fast immediately, rather than failing during an inspection query.
  - *Alternatives considered*: Lazy evaluation of layout or lowering only when `inspect_cell` is called. Rejected because it adds stateful caching and interior mutability to `CompiledModel`, violating pure data flow.
- **Choice 2: Dedicated public inspection contracts over exposing internal IR nodes**:
  - *Rationale*: Returning internal structures (such as `CellNode`, `petgraph::NodeIndex`, or raw `CellPayload` enums) would couple outer tools to compiler internals and complicate serialization. Dedicated inspection contracts (`CellInspection`, `TableInspection`, `DependencyTree`) present an ergonomic, stable API tailored for human CLI tables and agent JSON serialization.
  - *Alternatives considered*: Exposing `CellNode` directly. Rejected because `CellNode` lacks physical layout addresses, evaluated values, and lowered Excel formulas (which reside across different passes).
- **Choice 3: Hierarchical dependency trees with unique transitive counts**:
  - *Rationale*: Both terminal output (`f8 inspect precedents --recursive`) and AI agent root-cause analysis benefit from seeing the hierarchical calculation path rather than just a flat set of cell IDs.
  - *Alternatives considered*: Returning a flat `Vec<CellId>`. Rejected because flat lists lose the causal tree structure of financial formulas.

### Data Contracts

```rust
/// Unified compiled model consolidating all pass artifacts.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledModel {
    pub manifest: ProjectManifest,
    pub tables: IndexMap<TableId, TableSpec>,
    pub theme: Theme,
    pub symbol_table: SymbolTable,
    pub grid: CellGrid,
    pub graph: DependencyGraph,
    pub evaluated: EvaluatedModel,
    pub layout_map: LayoutMap,
    pub lowered_map: LoweredFormulaMap,
}

/// High-level model summary and hierarchical statistics.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSummary {
    pub model_name: String,
    pub theme_name: String,
    pub sheets: Vec<SheetSummary>,
    pub total_sheets: usize,
    pub total_tables: usize,
    pub total_cells: usize,
    pub total_drivers: usize,
    pub total_formulas: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetSummary {
    pub name: SheetId,
    pub tables: Vec<TableSummary>,
    pub total_cells: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableSummary {
    pub id: TableId,
    pub title: String,
    pub row_count: usize,
    pub col_count: usize,
    pub cell_count: usize,
    pub driver_count: usize,
    pub formula_count: usize,
}

/// Comprehensive inspection record for an individual cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellInspection {
    pub coordinate: CellId,
    pub sheet: SheetId,
    pub physical_address: String,
    pub evaluated_value: CellValue,
    pub format: Option<FormatPreset>,
    pub decimals: Option<u32>,
    pub currency: Option<CurrencyCode>,
    pub is_highlight: bool,
    pub indent: u8,
    pub is_formula: bool,
    pub formula_expr: Option<String>,
    pub excel_formula: Option<String>,
    pub note: Option<String>,
    pub precedents: Vec<CellId>,
    pub dependents: Vec<CellId>,
}

/// 2D tabular inspection record for a table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableInspection {
    pub table_id: TableId,
    pub title: String,
    pub subtitle: Option<String>,
    pub sheet: SheetId,
    pub columns: Vec<ColumnInspection>,
    pub rows: Vec<RowInspection>,
    pub cells: IndexMap<CellId, TableCellInfo>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnInspection {
    pub id: ColId,
    pub headers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RowInspection {
    pub id: RowId,
    pub line_item: String,
    pub indent: u8,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableCellInfo {
    pub value: CellValue,
    pub is_formula: bool,
    pub formula_expr: Option<String>,
    pub excel_formula: Option<String>,
    pub physical_a1: String,
}

/// Hierarchical dependency or blast-radius tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyTree {
    pub root: CellId,
    pub direct_count: usize,
    pub transitive_count: usize,
    pub nodes: Vec<DependencyNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyNode {
    pub cell_id: CellId,
    pub sheet_address: String,
    pub value: Option<CellValue>,
    pub excel_formula: Option<String>,
    pub children: Vec<DependencyNode>,
}

/// Domain errors during inspection queries.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum InspectionError {
    #[error("Table '{0}' not found in compiled model")]
    TableNotFound(TableId),

    #[error("Cell '{0}' not found in compiled model")]
    CellNotFound(CellId),

    #[error("Sheet '{0}' not found in compiled model")]
    SheetNotFound(SheetId),
}
```

### Risks & Trade-offs

- **Memory overhead of retaining full compilation artifacts**: Retaining `SymbolTable`, `CellGrid`, `DependencyGraph`, `EvaluatedModel`, `LayoutMap`, and `LoweredFormulaMap` in memory.
  - *Mitigation*: Even multi-statement models contain only hundreds to low thousands of cells, consuming negligible RAM (<5 MB). The speedup of instant inspection without re-running passes far outweighs the memory footprint.
- **Tree expansion for heavily referenced driver cells**: High-fanout drivers (such as `rev_growth`) could generate large recursive dependent trees.
  - *Mitigation*: Graph traversals use standard visited sets to compute accurate `transitive_count`, and cycle prevention from Pass 3 guarantees strictly finite traversals.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Single-pass compilation through Passes 1–6a**
  - **Given** a valid `ProjectManifest`, `Vec<TableSpec>`, and `Theme`
  - **When** calling `Compiler::compile(&manifest, &tables, &theme)`
  - **Then** returns a `CompiledModel` containing all pass artifacts with zero errors.

- [x] **Scenario 2: Zero-redundancy workbook emission from `CompiledModel`**
  - **Given** a valid `CompiledModel`
  - **When** calling `compiled.emit_workbook(&EmissionOptions::default())`
  - **Then** returns a valid OpenXML `.xlsx` byte buffer identical to `Compiler::emit_workbook`.

- [x] **Scenario 3: Model overview summary query**
  - **Given** a compiled model with 3 sheets and 4 tables
  - **When** calling `compiled.summary()`
  - **Then** returns a `ModelSummary` matching exact counts of sheets, tables, total cells, driver cells, and formula cells.

- [x] **Scenario 4: Discrete cell inspection**
  - **Given** a compiled model containing formula cell `IncomeStatement["ebit", "2024E"]`
  - **When** calling `compiled.inspect_cell("IncomeStatement", "ebit", "2024E")`
  - **Then** returns a `CellInspection` with physical sheet address (`Financials!C15`), evaluated number value, formula expression string, lowered Excel formula (`=C13-C14`), and direct precedents and dependents.

- [x] **Scenario 5: Missing cell inspection error**
  - **Given** a compiled model
  - **When** calling `compiled.inspect_cell("NonExistent", "row", "col")`
  - **Then** returns `Err(CompilerError::Inspection(InspectionError::CellNotFound(...)))`.

- [x] **Scenario 6: 2D table matrix inspection**
  - **Given** a compiled model with table `IncomeStatement`
  - **When** calling `compiled.inspect_table("IncomeStatement")`
  - **Then** returns `TableInspection` with columns, rows, and all cell values/formulas in declared order.

- [x] **Scenario 7: Direct and recursive precedent calculation tree queries**
  - **Given** a compiled model with multi-tier formula linkages (`net_income` -> `ebt` -> `ebit` -> `revenue`)
  - **When** calling `compiled.precedents_of(&cell_id, true)`
  - **Then** returns a `DependencyTree` with `direct_count = 2`, `transitive_count = 5`, and properly nested `DependencyNode` hierarchy.

- [x] **Scenario 8: Direct and recursive blast-radius dependent queries**
  - **Given** a driver cell `Assumptions["rev_growth", "2024E"]` referenced downstream by revenue and statements
  - **When** calling `compiled.dependents_of(&cell_id, true)`
  - **Then** returns a `DependencyTree` showing the transitive downstream calculation impact tree.

- [x] **Scenario 9: Full backward compatibility of existing compiler APIs**
  - **Given** existing test suites calling `Compiler::evaluate`, `Compiler::plan_layout`, and `Compiler::emit_workbook`
  - **When** running `cargo test --workspace`
  - **Then** all 100+ existing tests pass without regressions.

---

## Tasks

### Phase 1: Inspection Error Types & Data Contracts
- [x] Create `crates/f8-core/src/compiler/compiled/error.rs` defining `InspectionError`.
- [x] Integrate `InspectionError` into `CompilerError` in `crates/f8-core/src/compiler/error.rs`.
- [x] Create `crates/f8-core/src/compiler/compiled/model.rs` defining `ModelSummary`, `SheetSummary`, `TableSummary`, `CellInspection`, `TableInspection`, `ColumnInspection`, `RowInspection`, `TableCellInfo`, `DependencyTree`, and `DependencyNode`.

### Phase 2: `CompiledModel` Struct & Compilation Orchestration
- [x] Create `crates/f8-core/src/compiler/compiled/mod.rs` defining `CompiledModel`.
- [x] Implement `Compiler::compile` and `Compiler::compile_with_registry` in `crates/f8-core/src/compiler/mod.rs` orchestrating Passes 1–6a linearly.
- [x] Implement `CompiledModel::emit_workbook` delegating directly to `WorkbookEmitter::emit_to_buffer`.
- [x] Refactor `Compiler::emit_workbook` to delegate to `Compiler::compile`.

### Phase 3: Inspection Query APIs
- [x] Implement `CompiledModel::summary(&self) -> ModelSummary`.
- [x] Implement `CompiledModel::inspect_cell(&self, table, row, col) -> Result<CellInspection, InspectionError>`.
- [x] Implement `CompiledModel::inspect_table(&self, table) -> Result<TableInspection, InspectionError>`.

### Phase 4: Dependency Tree & Blast-Radius Traversals
- [x] Implement `CompiledModel::precedents_of(&self, cell, recursive) -> Result<DependencyTree, InspectionError>`.
- [x] Implement `CompiledModel::dependents_of(&self, cell, recursive) -> Result<DependencyTree, InspectionError>`.

### Phase 5: Re-exports, Testing & Verification
- [x] Re-export `CompiledModel` and inspection types in `crates/f8-core/src/compiler/mod.rs` and `crates/f8-core/src/lib.rs`.
- [x] Author comprehensive test suite in `crates/f8-core/tests/compiled_model_test.rs` covering all acceptance scenarios.
- [x] Run full cargo verification suite: `cargo check`, `cargo test --workspace`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`.
