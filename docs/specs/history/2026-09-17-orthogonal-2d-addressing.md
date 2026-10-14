---
title: Orthogonal 2D Table Addressing & Dot-Access Purge
date: 2026-09-17
status: completed
target-living-spec: specs/living-specs/expr/formula-parser.md
---

## Proposal

### What

Refactor the formula Abstract Syntax Tree (`TableRef`, `RefAccess`, `RowSelector`, `ColSelector`) and Pratt parser (`parse_reference`, `parse_bracket_access`) to enforce pure, orthogonal two-dimensional table indexing (`Table[RowSelector, ColSelector]`) across all formula references, completely purging legacy dot-access (`Table.row[...]`) and the redundant `RefAccess` enum.

### Why

1. **Multi-directional modeling**: Financial models and spreadsheets are not limited to horizontal timelines. They include vertical schedules (amortization, debt roll-forwards), non-temporal matrices (cap tables, valuation sensitivities), and general 2D tables. Hardcoding dot-access as `Table.row[col_timeline]` artificially biased the grammar toward horizontal schedules.
2. **Elimination of ambiguity and syntactic sugar**: The previous design permitted multiple ways to express single-cell coordinates (`Table.row[...]` vs `Table["row", ...]`) and asymmetrical slice forms (`Table.row[col_slice]` vs `Table[row_slice, col]`).
3. **AST and architectural simplification**: Purging `RefAccess` replaces a branching access enum with a direct, strongly typed 3-tuple `{ table, row, col }`, simplifying downstream reference resolution (Pass 3.1), dependency graph construction (Pass 3.2), and formula lowering (Pass 6).

### Scope

- **In-scope**:
  - Remove `RefAccess` enum from `src/expr/ast.rs`.
  - Refactor `TableRef` in `src/expr/ast.rs` to `{ pub table: TableId, pub row: RowSelector, pub col: ColSelector }`.
  - Provide a single constructor `TableRef::new(table, row, col)` and update `Display` implementation to `{table}[{row}, {col}]`.
  - Update `CellId` display to `{table}["{row}", "{col}"]` for end-to-end consistency.
  - Purge `parse_dot_access` from `src/expr/parser.rs`.
  - Update `parse_reference` to strictly expect `Identifier` followed by `[` (rejecting `.` after table identifier as `ParseError::UnexpectedToken`).
  - Update all unit and integration tests across `expr`, `compiler::ir::expand`, and test targets to use `Table[row, col]`.
  - Add explicit negative tests verifying that `Table.row[...]` is rejected at parse time.
- **Out-of-scope**:
  - Reference resolution engine implementation (`compiler::graph::resolve`, covered in downstream spec).
  - Adding `row.prev` or `row.@` for vertical timelines (can be introduced in a future enhancement without grammar breaking changes).

### What Changes

- `src/expr/ast.rs`: Remove `RefAccess`. Update `TableRef` struct and `Expr::Reference` serialization.
- `src/expr/parser.rs`: Purge `parse_dot_access`, update `parse_reference`, update parser unit tests.
- `src/compiler/ir/id.rs`: Update `CellId` `Display` implementation to `Table["row", "col"]`.
- `src/compiler/ir/expand.rs`: Update unit test formula strings.
- `tests/formula_parser_test.rs`: Update integration tests for orthogonal 2D references.
- `tests/grid_expansion_test.rs`: Update formula strings in grid expansion integration tests.
- `tests/cell_ir_test.rs`: Update tests for `CellId` display and formulas.
- `tests/spec_test.rs`: Update formula strings in model tests.

### Capabilities

- **New capabilities**:
  - Uniform 2D coordinate indexing `Table[row, col]` across all reference shapes (cells, horizontal slices, vertical slices, 2D blocks).
  - Explicit rejection of ambiguous dot-access references at compile time.
- **Modified capabilities**:
  - `TableRef` AST node directly holds `RowSelector` and `ColSelector` without intermediate `RefAccess` wrapper.
  - Formula string formatting strictly produces `Table["row", "col"]` and `Table["row", @]`.
  - `CellId` displays matching canonical 2D addressing: `Table["row", "col"]`.

---

## Design

### Goals

1. **Pure symmetry**: Every table reference in Finx adheres to `Table[RowSelector, ColSelector]`.
2. **Zero ambiguity**: Exactly one way to write any reference.
3. **Greenfield minimalism**: Zero backward-compatibility shims, deprecated aliases, or legacy parsers for dot-access.

### Invariants

1. **Mandatory two coordinates**: All bracket accesses require both row and column selectors separated by a comma (`Table[row, col]`).
2. **Strictly quoted coordinate keys**: Discrete row and column coordinate keys must be double-quoted strings (`"revenue"`, `"2024E"`).
3. **Unquoted keywords**: Built-in tokens (`@`, `col.prev`, `col.next`, `cols.<group>`, `rows.<group>`) remain unquoted.

### Data Contracts

#### Updated `TableRef` Definition

```rust
/// Reference addressing target to a table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRef {
    /// Target table identifier.
    pub table: TableId,
    /// Row coordinate selector.
    pub row: RowSelector,
    /// Column coordinate selector.
    pub col: ColSelector,
}

impl TableRef {
    /// Constructs a table reference: `Table[row, col]`.
    pub fn new(table: impl Into<TableId>, row: RowSelector, col: ColSelector) -> Self {
        Self {
            table: table.into(),
            row,
            col,
        }
    }
}

impl fmt::Display for TableRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}[{}, {}]", self.table, self.row, self.col)
    }
}
```

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Parse single-cell active column reference**
  - **Given** formula string `"IncomeStatement[\"revenue\", @]"`
  - **When** parsing formula expression
  - **Then** produces `TableRef` with `row = RowSelector::Key("revenue")` and `col = ColSelector::Active`.

- [x] **Scenario 2: Parse single-cell specific column reference**
  - **Given** formula string `"Assumptions[\"tax_rate\", \"value\"]"`
  - **When** parsing formula expression
  - **Then** produces `TableRef` with `row = RowSelector::Key("tax_rate")` and `col = ColSelector::Key("value")`.

- [x] **Scenario 3: Parse temporal offset reference**
  - **Given** formula string `"BalanceSheet[\"cash\", col.prev]"`
  - **When** parsing formula expression
  - **Then** produces `TableRef` with `row = RowSelector::Key("cash")` and `col = ColSelector::Special(SpecialCol::Prev(1))`.

- [x] **Scenario 4: Parse horizontal row slice**
  - **Given** formula string `"SUM(IncomeStatement[\"revenue\", \"2024E\"..\"2028E\"])`
  - **When** parsing formula expression
  - **Then** produces `TableRef` with `row = RowSelector::Key("revenue")` and `col = ColSelector::Range("2024E", "2028E")`.

- [x] **Scenario 5: Parse vertical column slice**
  - **Given** formula string `"SUM(BalanceSheet[rows.current_assets, @])"`
  - **When** parsing formula expression
  - **Then** produces `TableRef` with `row = RowSelector::Special(SpecialRow::Group("current_assets"))` and `col = ColSelector::Active`.

- [x] **Scenario 6: Parse 2D block range**
  - **Given** formula string `"SUM(IncomeStatement[\"r_and_d\"..\"g_and_a\", \"2024E\"..\"2028E\"])`
  - **When** parsing formula expression
  - **Then** produces `TableRef` with `row = RowSelector::Range(...)` and `col = ColSelector::Range(...)`.

- [x] **Scenario 7: Reject legacy dot-access**
  - **Given** formula string `"IncomeStatement.revenue[@]"`
  - **When** parsing formula expression
  - **Then** returns `Err(ParseError::UnexpectedToken)` pointing to `.` after table name.

- [x] **Scenario 8: Reject unquoted coordinate keys**
  - **Given** formula string `"IncomeStatement[revenue, @]"`
  - **When** parsing formula expression
  - **Then** returns `Err(ParseError::UnquotedCoordinateKey)`.

- [x] **Scenario 9: Display serialization round-trip**
  - **Given** parsed `TableRef` for `"IncomeStatement[\"revenue\", @]"`
  - **When** converting to string via `to_string()`
  - **Then** yields exact string `"IncomeStatement[\"revenue\", @]"`.

---

## Tasks

- [x] **Phase 1: AST refactoring**
  - [x] Remove `RefAccess` from `src/expr/ast.rs`.
  - [x] Simplify `TableRef` struct to `{ table, row, col }`.
  - [x] Update `TableRef::new` constructor and `Display` implementation.
  - [x] Update `CellId` `Display` implementation in `src/compiler/ir/id.rs`.

- [x] **Phase 2: Parser refactoring**
  - [x] Purge `parse_dot_access` from `src/expr/parser.rs`.
  - [x] Update `parse_reference` to require `[` immediately following `Identifier`.
  - [x] Update error messaging when encountering unexpected tokens after table name.

- [x] **Phase 3: Test updates & regressions**
  - [x] Update all unit tests in `src/expr/parser.rs`.
  - [x] Update integration tests in `tests/formula_parser_test.rs`.
  - [x] Update formula strings in `tests/grid_expansion_test.rs` and `src/compiler/ir/expand.rs`.
  - [x] Update tests in `tests/cell_ir_test.rs` and `tests/spec_test.rs`.
  - [x] Add negative tests asserting `Table.row[...]` is rejected.

- [x] **Phase 4: Verification suite**
  - [x] Run `cargo check`.
  - [x] Run `cargo test`.
  - [x] Run `cargo clippy --all-targets -- -D warnings`.
  - [x] Run `cargo fmt --check`.

- [x] **Phase 5: Living spec reconciliation**
  - [x] Reconcile `specs/living-specs/expr/formula-parser.md`.
  - [x] Reconcile `specs/living-specs/compiler/cell-ir.md`.
  - [x] Archive draft spec to `specs/history/`.
