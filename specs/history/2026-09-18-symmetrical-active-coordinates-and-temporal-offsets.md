---
title: Symmetrical Active Coordinates & Temporal Offsets
date: 2026-09-18
status: completed
target-living-spec:
  - specs/living-specs/expr/formula-parser.md
  - specs/living-specs/compiler/dependency-graph.md
---

## Proposal

### What

Retire bare `@` across the entire codebase and implement symmetrical 2D active coordinate tokens (`@col`, `@row`) and vertical temporal offsets (`row.prev`, `row.next`) in [`finx::expr`](src/expr/mod.rs) and [`finx::compiler::graph`](src/compiler/graph/mod.rs). Align the expression AST, lexer, Pratt parser, reference resolution engine, and all test suites with the unified 2D matrix model documented in [`docs/dsl/addressing.md`](docs/dsl/addressing.md) and [`docs/dsl/grammar.md`](docs/dsl/grammar.md).

### Why

1. **Single source of truth & zero legacy residue**: Commit `f1f926c` updated the documentation suite to establish `@col` and `@row` as canonical active coordinates and retired bare `@`. However, the implementation was not updated, leaving code and docs out of sync.
2. **Symmetrical 2D matrix architecture**: Finx treats rows and columns as symmetrical first-class calculation axes. Vertical schedules (e.g. Cap Tables, Debt Amortization) require column formulas to bind to the active row (`Table[@row, "shares"]`) and track preceding rows (`Table[row.prev, "balance"]`), matching the capabilities horizontal schedules possess with `@col` and `col.prev`.
3. **Greenfield minimalism**: Per `AGENTS.md`, Finx avoids transitional shims, deprecated aliases, or dual-syntax ambiguity. Bare `@` must be rejected at parse time with an actionable diagnostic.

### Scope

- **In-scope**:
  - `src/expr/ast.rs`:
    - Add `RowSelector::Active` (serializes and displays as `@row`).
    - Update `ColSelector::Active` `Display` implementation to format as `@col`.
    - Add `SpecialRow::Prev(u32)` and `SpecialRow::Next(u32)` (displays as `row.prev`, `row.prev(k)`, `row.next`, `row.next(k)`).
  - `src/expr/parser.rs`:
    - Lexer: Add `Token::AtCol` (`@col`) and `Token::AtRow` (`@row`). Reject bare `@` with an explicit diagnostic directing callers to `@col` or `@row`.
    - Parser: Support `RowSelector::Active`, `SpecialRow::Prev`, and `SpecialRow::Next` in `parse_row_spec`. Support `ColSelector::Active` via `Token::AtCol` in `parse_col_spec`.
  - `src/compiler/graph/error.rs`:
    - Add `ResolveError::ActiveRowMismatch { origin: Box<CellId>, target_table: TableId, row: RowId }`.
    - Fix temporal offset cross-table lookup error to use `UnknownColumn` / `UnknownRow` instead of `ActiveColumnMismatch`.
  - `src/compiler/graph/resolve.rs`:
    - Implement resolution of `RowSelector::Active`, `SpecialRow::Prev`, and `SpecialRow::Next`.
    - Reconcile `resolve_table_ref` to validate `origin.row` against `origin_table` when resolving `@row` and `row.prev` / `row.next`.
  - Repository-wide test suites and specs:
    - Update all formula strings from bare `@` to `@col` across `tests/`, `src/`, and `specs/`.
    - Add integration tests for `@row`, `row.prev`, `row.next`, and bare `@` rejection.
    - Update living specs [`specs/living-specs/expr/formula-parser.md`](specs/living-specs/expr/formula-parser.md) and [`specs/living-specs/compiler/dependency-graph.md`](specs/living-specs/compiler/dependency-graph.md).
- **Out-of-scope**:
  - Downstream Pass 3B (petgraph DAG cycle detection), Pass 4 (evaluator), Pass 5 (lowering).

### What Changes

- `src/expr/ast.rs`: Extend `RowSelector` and `SpecialRow` enums; update `Display` implementations for `@col` and `@row`.
- `src/expr/parser.rs`: Lex `@col` and `@row`; parse active row and vertical temporal offsets; reject bare `@`.
- `src/expr/error.rs`: Add parse error variant for bare `@` diagnostic.
- `src/compiler/graph/error.rs`: Add `ActiveRowMismatch`.
- `src/compiler/graph/resolve.rs`: Implement row coordinate resolution for `@row`, `row.prev`, and `row.next`.
- `tests/formula_parser_test.rs`: Update formulas to `@col`; add tests for `@row`, `row.prev`, `row.next`, and bare `@` error.
- `tests/reference_resolution_test.rs`: Update formulas to `@col`; add tests for `@row` and `row.prev` resolution.
- `tests/grid_expansion_test.rs`, `tests/cell_ir_test.rs`, `tests/spec_test.rs`: Replace bare `@` with `@col`.

### Capabilities

- **New capabilities**:
  - Vertical schedule formulas can bind to active rows via `Table[@row, "col"]`.
  - Formulas can reference relative predecessor and successor rows via `Table[row.prev, "col"]` and `Table[row.next(k), "col"]`.
  - Reference resolution engine resolves `@row` and `row.prev`/`row.next` coordinates with axis boundary and cross-table validation.
- **Modified capabilities**:
  - Active column token in formulas is strictly `@col`.
  - Bare `@` is rejected at parse time with an actionable diagnostic.

---

## Design

### Goals

1. **Syntactic symmetry**: Row and column selector capabilities must be fully orthogonal and symmetrical across 2D matrix formulas.
2. **Clean break from bare `@`**: Complete repository-wide purge with zero backward-compatibility alias or shim.
3. **Actionable diagnostics**: Clear parse error when bare `@` is encountered, guiding users to `@col` or `@row`.
4. **Deterministic bounds checking**: Vertical temporal offsets validate against row axis length in strict declaration order.

### Invariants

1. **Mandatory keyword specificity**: Active coordinates must specify the axis: `@col` for column, `@row` for row.
2. **Cross-table row identity**: `TableB[@row, "col"]` requires `TableB` to declare a row with identical `RowId`.
3. **Row axis bounds enforcement**: `row.prev(k)` and `row.next(k)` cannot escape row axis boundaries ($idx - k < 0$ or $idx + k \ge len$).
4. **Zero ambiguity**: Bare `@` without `col` or `row` is invalid syntax.

### Decisions

- **Decision 1: Tokenize `@col` and `@row` as atomic tokens in the lexer**:
  - *Rationale*: Prevents grammar ambiguity, simplifies Pratt parser cursor management, and allows the lexer to immediately reject bare `@` followed by non-axis characters or whitespace.
- **Decision 2: Dedicated `ActiveRowMismatch` error variant**:
  - *Rationale*: Symmetrical to `ActiveColumnMismatch`, pinpoints the origin cell coordinate and the missing `RowId` in the cross-table target.
- **Decision 3: Use `UnknownColumn` / `UnknownRow` for missing target periods in temporal offsets**:
  - *Rationale*: Avoids misleading diagnostics claiming that a calculated predecessor/successor coordinate was an "active" column/row.

### Data Contracts

#### AST Extensions (`finx::expr::ast`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RowSelector {
    /// Active evaluation row: `[@row]`.
    Active,
    /// Explicit single row key: `["revenue"]`.
    Key(RowId),
    /// Explicit row range slice: `["r_and_d".."g_and_a"]`.
    Range(RowId, RowId),
    /// Semantic row group milestone or hierarchy: `rows.current_assets`.
    Special(SpecialRow),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpecialRow {
    /// Temporal predecessor offset: `row.prev` (offset 1) or `row.prev(k)`.
    Prev(u32),
    /// Temporal successor offset: `row.next` (offset 1) or `row.next(k)`.
    Next(u32),
    /// All descendant rows in a group: `rows.<group>`.
    Group(RowGroupId),
    /// First row in a group: `rows.<group>.first`.
    GroupFirst(RowGroupId),
    /// Last row in a group: `rows.<group>.last`.
    GroupLast(RowGroupId),
}
```

#### Lexer Tokens (`finx::expr::parser`)

```rust
pub enum Token<'a> {
    // ...
    /// Active evaluation column symbol `@col`.
    AtCol,
    /// Active evaluation row symbol `@row`.
    AtRow,
    // (Token::At is purged)
}
```

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Parsing active column `@col`**
  - **Given** formula string `"Table[\"rev\", @col]"`
  - **When** `Expr::parse` is executed
  - **Then** parses into `TableRef` with `col: ColSelector::Active` and roundtrips to `"Table[\"rev\", @col]"`.

- [x] **Scenario 2: Parsing active row `@row`**
  - **Given** formula string `"Table[@row, \"shares\"]"`
  - **When** `Expr::parse` is executed
  - **Then** parses into `TableRef` with `row: RowSelector::Active` and roundtrips to `"Table[@row, \"shares\"]"`.

- [x] **Scenario 3: Rejecting bare `@` with actionable diagnostic**
  - **Given** formula string `"Table[\"rev\", @]"`
  - **When** `Expr::parse` is executed
  - **Then** fails with parse error explicitly stating that bare `'@'` is retired in favor of `'@col'` or `'@row'`.

- [x] **Scenario 4: Parsing vertical temporal offsets `row.prev` and `row.next`**
  - **Given** formula strings `"Table[row.prev, \"cash\"]"` and `"Table[row.next(2), \"cash\"]"`
  - **When** `Expr::parse` is executed
  - **Then** parses into `SpecialRow::Prev(1)` and `SpecialRow::Next(2)` respectively.

- [x] **Scenario 5: Resolving active row `@row` in reference resolution**
  - **Given** cell `CapTable["founder_a", "value"]` with formula `CapTable[@row, "shares"] * CapTable[@row, "share_price"]`
  - **When** `resolve_cell_dependencies` is executed
  - **Then** resolves dependencies to `CapTable["founder_a", "shares"]` and `CapTable["founder_a", "share_price"]`.

- [x] **Scenario 6: Cross-table active row mismatch rejected**
  - **Given** cell `T1["rev", "2024E"]` referencing `T2[@row, "metric"]` where `T2` has no row `"rev"`
  - **When** reference resolution is executed
  - **Then** fails with `ResolveError::ActiveRowMismatch`.

- [x] **Scenario 7: Resolving vertical temporal predecessor offset `row.prev`**
  - **Given** cell `DebtSchedule["tranche_b", "balance"]` (row index 1) referencing `DebtSchedule[row.prev, "balance"]`
  - **When** reference resolution is executed
  - **Then** resolves to `[DebtSchedule["tranche_a", "balance"]]`.

- [x] **Scenario 8: Vertical temporal offset out of bounds rejected**
  - **Given** cell `DebtSchedule["tranche_a", "balance"]` (row index 0) referencing `DebtSchedule[row.prev, "balance"]`
  - **When** reference resolution is executed
  - **Then** fails with `ResolveError::TemporalOffsetOutOfBounds { offset: -1 }`.

- [x] **Scenario 9: Full test suite migrated from bare `@` to `@col`**
  - **Given** all test files across `tests/`
  - **When** test suite is inspected
  - **Then** zero occurrences of bare `@` remain and all tests use `@col` or `@row`.

- [x] **Scenario 10: Full verification pass**
  - **Given** the modified codebase
  - **When** running verification commands
  - **Then** `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check` all pass with zero warnings.

---

## Tasks

### Phase 1: AST and Lexer / Parser Refactoring
- [x] Add `RowSelector::Active`, `SpecialRow::Prev(u32)`, and `SpecialRow::Next(u32)` to `src/expr/ast.rs`.
- [x] Update `Display` implementation for `ColSelector::Active` to emit `"@col"`.
- [x] Update lexer in `src/expr/parser.rs` to tokenize `@col` and `@row`, and reject bare `@`.
- [x] Implement parser support for `@row`, `row.prev`, and `row.next` in `parse_row_spec`.
- [x] Update parser support for `@col` in `parse_col_spec`.
- [x] Update unit tests in `src/expr/parser.rs` and `tests/formula_parser_test.rs`.

### Phase 2: Reference Resolution Engine Expansion
- [x] Add `ResolveError::ActiveRowMismatch` to `src/compiler/graph/error.rs`.
- [x] Implement `RowSelector::Active`, `SpecialRow::Prev`, and `SpecialRow::Next` in `resolve_row_selector`.
- [x] Reconcile temporal offset cross-table lookup error to emit `UnknownColumn` / `UnknownRow`.
- [x] Update and expand `tests/reference_resolution_test.rs` to validate `@col`, `@row`, `row.prev`, and `row.next`.

### Phase 3: Repository-Wide Formula Migration
- [x] Update all formula occurrences in `tests/grid_expansion_test.rs`, `tests/cell_ir_test.rs`, and `tests/spec_test.rs`.
- [x] Reconcile `specs/living-specs/expr/formula-parser.md` and `specs/living-specs/compiler/dependency-graph.md`.
- [x] Run full project verification:
  - `cargo check`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`
