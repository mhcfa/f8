---
title: Grid Expansion Engine
date: 2026-09-16
status: completed
target-living-spec: specs/living-specs/compiler/cell-ir.md
---

## Proposal

### What

Implement the Grid Expansion Engine (`compiler::ir::expand`) that transforms high-level table specifications (`TableSpec`) into discrete, cell-level intermediate representation nodes (`CellNode`) within a deterministic `CellGrid`.

### Why

High-level declarative tables define rows compactly using scalar formulas, literal vectors, or partitioned match arms (`match`). Before building the dependency graph (Pass 3) or calculating values (Pass 4), the compiler must expand these compact row definitions across all declared table columns. This creates fine-grained `(TableId, RowId, ColId)` calculation nodes, parses formula expressions into `Expr` AST trees, and preserves styling and audit metadata at cell granularity.

### Scope

- **In-scope**:
  - Unrolling table rows across declared columns into discrete `CellNode` instances.
  - Expanding literal rows (`row.value`) into `CellPayload::Literal`.
  - Expanding uniform formula rows (`row.expr`) by parsing formula strings into `Expr` ASTs once and assigning `CellPayload::Formula` across all columns.
  - Expanding partitioned match arms (`row.match_arms`) by resolving column targets via `MatchSelector` and applying arm-level literal values or parsed formula expressions.
  - Cascading and overriding cell attributes: format presets (`FormatPreset`), developer notes (`note`), and source provenance (`source`).
  - Diagnostic error reporting wrapping formula syntax errors (`ParseError`) and model validation errors (`ModelError`) into `IrError`.
  - Standalone table expansion (`expand_table`), multi-table grid expansion (`expand_grid`), and workbook orchestration (`expand_workbook`).
- **Out-of-scope**:
  - Dependency graph edge construction and Tarjan SCC cycle detection (Pass 3, `compiler::graph`).
  - Dynamic alias evaluation (`col.prev`, `cols.forecast`) into concrete dependency edges (deferred to Pass 3).
  - In-memory formula calculation and topological evaluation (Pass 4, `compiler::eval`).
  - Lowering to concrete physical coordinates (`A1` / `$C$10`) (Pass 6, `compiler::lowering`).

### What Changes

- `src/compiler/ir/error.rs`: Add `FormulaParse` and `Model` variants to `IrError`.
- `src/compiler/ir/expand.rs`: Implement row expansion algorithms (`expand_table`, `expand_grid`, `expand_workbook`).
- `src/compiler/ir/mod.rs`: Re-export `expand_grid`, `expand_table`, and `expand_workbook`.
- `tests/grid_expansion_test.rs`: Integration test suite covering literal rows, formula rows, match arm partitions, attribute inheritance, and error conditions.

### Capabilities

- **New capabilities**:
  - Expand any valid `TableSpec` into an ordered `Vec<CellNode>`.
  - Populate a deterministic `CellGrid` containing every discrete cell for a set of tables.
  - Surface actionable compilation errors when formula expressions in rows or match arms contain syntax errors, specifying the exact table, row, and column context.
  - Orchestrate end-to-end manifest and table ingestion into synchronized `SymbolTable` and `CellGrid`.
- **Modified capabilities**:
  - `IrError` gains conversion from `ModelError` and wraps `ParseError`.

## Design

### Goals

1. **Deterministic expansion**: Emit cells in strict declaration order: table insertion order, then row order, then column order.
2. **Zero duplicate parsing**: Parse uniform row formulas once per row rather than re-parsing for every column.
3. **Pristine metadata propagation**: Correctly cascade row-level formatting presets and allow match arms to override or augment notes and provenance metadata.
4. **Clean boundary before Pass 3**: Hand off a complete, verified `CellGrid` to the dependency graph pass without prematurely resolving cross-cell dependency edges.

### Invariants

1. **Full grid coverage**: For any table with $R$ rows and $C$ columns, expansion produces exactly $R \times C$ cells.
2. **Deterministic iteration order**: Cells inserted into `CellGrid` preserve table $\rightarrow$ row $\rightarrow$ column order.
3. **Payload mutual exclusivity**: Every generated cell node possesses exactly one payload (`Literal` or `Formula`).
4. **Precise diagnostic context**: Any formula syntax error during expansion reports the containing `TableId`, `RowId`, and optional `ColId`.

### Decisions

- **Choice**: Parse formula expressions during grid expansion rather than storing unparsed strings in `CellNode`.
  - **Rationale**: Validates formula syntax early during IR construction, ensuring that downstream passes (`compiler::graph`, `compiler::eval`, `compiler::lowering`) only ever deal with validated `Expr` AST trees.
  - **Alternatives considered**: Storing unparsed strings in `CellNode` and parsing during graph construction (delays syntax error discovery and duplicates parsing effort across passes).
- **Choice**: Cache parsed `Expr` for uniform formula rows and clone AST per column.
  - **Rationale**: An expression for a 50-column forecast table is parsed once into `Expr`, then cloned 50 times. This reduces parsing overhead from $O(C)$ to $O(1)$ per row.
  - **Alternatives considered**: Parsing per column (needless CPU waste) or storing an `Arc<Expr>` in `CellPayload` (adds pointer indirection when `Expr` is already clone-efficient).
- **Choice**: Defer resolving dynamic column offsets (`col.prev`) to Pass 3 (Dependency Graph).
  - **Rationale**: `CellNode` retains the original `Expr` with `ColSelector::Active`, `ColSelector::Special(SpecialCol::Prev(1))`, etc. Lowering needs to know if a coordinate was originally relative (`[@]`) or absolute (`["2024E"]`) when generating Excel formulas (`C10` vs `$C$10`). Resolving dependencies belongs to the graph pass.
  - **Alternatives considered**: Rewriting `Expr` during expansion to replace `@` with concrete column names (destroys relative referencing semantics needed by Pass 6).

### Data Contracts

#### Function Signatures

```rust
/// Expands a single table specification into discrete cell nodes.
pub fn expand_table(
    symbol_table: &SymbolTable,
    table: &TableSpec,
) -> Result<Vec<CellNode>, IrError>;

/// Expands a slice of table specifications into a populated CellGrid.
pub fn expand_grid(
    symbol_table: &SymbolTable,
    tables: &[TableSpec],
) -> Result<CellGrid, IrError>;

/// Orchestrates workbook symbol registration and grid expansion from a manifest and table specs.
pub fn expand_workbook(
    manifest: &ProjectManifest,
    tables: &[TableSpec],
) -> Result<(SymbolTable, CellGrid), IrError>;
```

#### Extended Error Variants

```rust
pub enum IrError {
    // Existing variants...
    #[error("formula parse error in table '{table}', row '{row}'{}: {source}", column.as_ref().map(|c| format!(", col '{c}'")).unwrap_or_default())]
    FormulaParse {
        table: TableId,
        row: RowId,
        column: Option<ColId>,
        source: ParseError,
    },

    #[error("model validation error: {0}")]
    Model(#[from] ModelError),
}
```

### Risks & Trade-offs

- **Memory usage of cloned `Expr` trees**: Cloning `Expr` across large column counts incurs heap allocations.
  - *Mitigation*: Expression ASTs in financial models are typically shallow (depth 2–6). Benchmarks show negligible memory impact for standard workbook sizes. If needed in the future, `Arc<Expr>` or arena allocation can be introduced without breaking public API boundaries.
- **Match arm note/source overrides**: Determining whether arm metadata replaces or concatenates row metadata.
  - *Decision*: Arm note overrides row note if present (`arm.note.or(row.note)`). Arm source overrides row source if present (`arm.source.or(row.source)`).

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Expand literal row across columns**
  - **Given** a table with columns `["2021A", "2022A", "2023A"]` and a row with literal values `[100.0, 110.0, 120.0]`
  - **When** expanding the table
  - **Then** produces 3 `CellNode`s with `CellPayload::Literal` containing the corresponding numbers.

- [x] **Scenario 2: Expand uniform formula row across columns**
  - **Given** a table with columns `["2024E", "2025E"]` and a row with expression `"IncomeStatement.revenue[@] * 0.1"`
  - **When** expanding the table
  - **Then** produces 2 `CellNode`s with `CellPayload::Formula` containing identical parsed `Expr` AST trees.

- [x] **Scenario 3: Expand partitioned row with match arms**
  - **Given** a table with columns `["2023A", "2024E", "2025E"]` where row has `match` arm `"2023A"` (value `[100.0]`) and arm `"2024E..2025E"` (expr `"col.prev * 1.05"`)
  - **When** expanding the table
  - **Then** cell for `"2023A"` is `CellPayload::Literal(100.0)` and cells for `"2024E"` and `"2025E"` are `CellPayload::Formula`.

- [x] **Scenario 4: Metadata and format preset propagation**
  - **Given** a row with format `Currency`, note `"Base revenue"`, and an arm for `"2025E"` with note `"Forecast update"`
  - **When** expanding the table
  - **Then** all cells retain format `Currency`, while `"2025E"` has note `"Forecast update"` and earlier columns have `"Base revenue"`.

- [x] **Scenario 5: Actionable formula parse error diagnostic**
  - **Given** a row with invalid formula expression `"IncomeStatement.revenue[@] + * 5"`
  - **When** expanding the table
  - **Then** expansion returns `Err(IrError::FormulaParse { table, row, .. })` detailing the parse failure.

- [x] **Scenario 6: End-to-end workbook orchestration**
  - **Given** a `ProjectManifest` with sheets `"Summary"` and `"Model"` and corresponding `TableSpec`s
  - **When** calling `expand_workbook(manifest, tables)`
  - **Then** returns a registered `SymbolTable` and a populated `CellGrid` containing all cells across both sheets with no errors.

- [x] **Scenario 7: Reject table not in symbol table**
  - **Given** a `SymbolTable` that does not contain table `"UnknownTable"`
  - **When** calling `expand_table(&symbol_table, &unknown_table)`
  - **Then** returns `Err(IrError::UnknownTable { .. })`.

## Tasks

- [x] **Phase 1: Error and interface definitions**
  - [x] Add `FormulaParse` and `Model` variants to `IrError` in `src/compiler/ir/error.rs`.
  - [x] Add module `src/compiler/ir/expand.rs` and re-export in `src/compiler/ir/mod.rs`.

- [x] **Phase 2: Row unrolling and grid expansion implementation**
  - [x] Implement `expand_table` handling literal rows, uniform formula rows, and match arms.
  - [x] Implement attribute cascading for format, note, and source.
  - [x] Implement `expand_grid` inserting cells into `CellGrid`.
  - [x] Implement `expand_workbook` orchestrating manifest validation, symbol registration, and grid expansion.

- [x] **Phase 3: Integration test suite**
  - [x] Create `tests/grid_expansion_test.rs` covering all acceptance criteria (Scenarios 1 through 7).
  - [x] Verify using `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`.

- [x] **Phase 4: Living spec update and archive**
  - [x] Update `specs/living-specs/compiler/cell-ir.md` with grid expansion rules and interfaces.
  - [x] Move draft spec to `specs/history/2026-09-16-grid-expansion-engine.md`.
