---
title: Formula Lowering Engine & AST Reference Resolution
date: 2026-09-20
status: completed
target-living-spec: specs/living-specs/compiler/formula-lowering.md
---

## Proposal

### What

Implement Finx's Pass 6a Formula Lowering Engine in `finx::compiler::lowering`:
1. Define output contract types in `src/compiler/lowering/`: `LoweredFormulaMap`, `FormulaLowerer`, and structured `LoweringError` diagnostics.
2. Implement semantic AST reference lowering:
   - Resolve semantic `TableRef` nodes to physical 2D worksheet coordinates `(sheet, col, row, a1)` using `SymbolTable` and `LayoutMap`.
   - Enforce canonical Excel coordinate referencing rules (`$`):
     - Dynamic coordinates (`@col`, `@row`, `col.prev`, `col.next`, `row.prev`, `row.next`): Lower to relative references (e.g. `C10`, `C5`).
     - Static coordinates (specific row/col keys, milestone anchors): Lower to absolute coordinates (e.g. `=$C$10`).
     - Vertical slices with active columns (`BalanceSheet[rows.current_assets, @col]`): Lower to column-relative, row-pinned ranges (e.g. `C$14:C$18`).
     - Horizontal slices with active rows (`CapTable[@row, "shares".."capital"]`): Lower to row-relative, column-pinned ranges (e.g. `C5:E5`).
     - Cross-sheet references: Prepend single-quoted sheet qualification (e.g. `='Summary'!$B$4`, `='Assumptions'!$C$14:$G$18`).
3. Implement formula AST serialization:
   - Prefix emitted formula expressions with `=`.
   - Serialize literals: numbers, booleans (`TRUE`, `FALSE`), and quoted strings with doubled-quote escaping (`""`).
   - Emit canonical uppercase Excel function names (`SUM`, `AVERAGE`, `IF`, `XLOOKUP`, `INDEX`, `NPV`, etc.).
   - Preserve empty argument slots for omitted arguments (`Expr::Omitted` $\rightarrow$ `,,`).
   - Serialize unary and binary operators with parentheses governed by operator precedence and associativity rules.
4. Expose high-level compiler pipeline integration as `Compiler::lower_formulas(...)`.
5. Provide a comprehensive unit and integration test suite verifying coordinate lowering, range bounds, sheet prefixes, operator precedence, and end-to-end model compilation.

### Why

- **Prerequisite for Pass 6b (Workbook Emission)**: `rust_xlsxwriter` requires native Excel formula strings (e.g. `=C10-C11`, `=SUM(C$14:C$18)`) rather than high-level semantic AST representations (`IncomeStatement["revenue", @col]`).
- **Separation of concerns**: Lowering formula ASTs into concrete strings is pure string synthesis and coordinate mapping. Decoupling it from OpenXML binary packaging (`rust_xlsxwriter`) keeps compiler passes modular, independently testable, and deterministic.
- **Institutional modeling aesthetics**: Formula cells inspected in Excel must adhere to standard financial modeling conventions (e.g. relative references across statement forecast periods, absolute references to assumptions, and pinned ranges for subtotals).

### Scope

- **In-Scope**:
  - Lowering contract data structures (`LoweredFormulaMap`, `FormulaLowerer`) in `src/compiler/lowering/mod.rs`.
  - Structured diagnostics (`LoweringError`) in `src/compiler/lowering/error.rs`.
  - Coordinate formatter module handling relative/absolute `$` pinning and single-quoted sheet prefixes.
  - AST-to-Excel formula serializer handling literals, binary/unary operators with precedence parentheses, functions, omitted arguments, and references.
  - Integration with `Compiler::lower_formulas(...)` in `src/compiler/mod.rs`.
  - Comprehensive integration test suite in `tests/formula_lowering_test.rs`.

- **Out-of-Scope**:
  - Pass 6b OpenXML binary packaging with `rust_xlsxwriter` (Phase 4b).
  - Headless JSON / CSV / Parquet data emission (Phase 4b).
  - Runtime evaluation of formula strings (already performed in Pass 4 by in-memory evaluator).

### What Changes

- **Affected modules**:
  - `src/compiler/lowering/mod.rs`: Implement lowering orchestrator and AST serializer.
  - `src/compiler/lowering/error.rs`: Implement domain diagnostics (`LoweringError`).
  - `src/compiler/error.rs`: Add `CompilerError::Lowering(#[from] LoweringError)`.
  - `src/compiler/mod.rs`: Expose `lowering` module and `Compiler::lower_formulas` facade.
  - `tests/formula_lowering_test.rs`: Add integration tests covering all reference modes, precedence, and multi-table models.

### Capabilities

- **New capabilities**:
  - Translate any valid Finx formula AST into an executable Excel formula string.
  - Resolve 1D horizontal ranges, 1D vertical ranges, and 2D blocks into canonical Excel range notation (`A1:B10`).
  - Automatic `$` pinning based on semantic selector types (`Active`, `Prev`, `Key`, `Range`, `Group`).
  - Automatic single-quote sheet qualifying for inter-sheet references (`'Sheet'!$A$1`).
- **Modified capabilities**:
  - `Compiler` facade gains `lower_formulas(...)` pass between `plan_layout(...)` and binary emission.

---

## Design

### Goals

1. **Exact Excel syntax**: Every lowered formula string must be immediately valid when opened in Excel without syntax repair dialogs.
2. **Deterministic output**: Formula lowering must be 100% pure and deterministic with zero platform or iteration order dependencies.
3. **Institutional financial conventions**: Clean formulas (`=C10-C11` across forecast columns, `='Assumptions'!$B$4` for inputs, `=SUM(C$14:C$18)` for column groups).

### Invariants

- **Mandatory `=` prefix**: All emitted formula strings start with `=`.
- **Inter-sheet qualification**: References targeting tables on different worksheets must include single-quoted sheet names (`'Sheet'!`). References on the same sheet must omit the sheet prefix.
- **Range ordering**: Range addresses always emit top-left cell first, followed by colon, followed by bottom-right cell (`TopLeft:BottomRight`).
- **Operator precedence**: Binary operations with lower precedence than their parent operation must be enclosed in parentheses. Left-associative operations on the right-hand side of non-commutative operators (`-`, `/`) and right-hand operations of exponentiation (`^`) must be parenthesized to guarantee Excel evaluates according to AST semantics.
- **Zero production panics**: All coordinate lookups and selector resolutions return `Result<_, LoweringError>`.

### Decisions

- **Coordinate Pinning Rules**:
  - *Choice*: Pin column with `$` if and only if `ColSelector` is static (`Key`, `Range`, `Group`, `GroupFirst`, `GroupLast`) AND row is NOT active. If row or column is active (`@col`, `@row`) or temporal offset (`col.prev`, `row.prev`), emit relative coordinates for that axis.
  - *Rationale*: Matches institutional modeling expectations: intra-statement period-by-period formulas are relative (`=C10-C11`), group totals pin row ranges (`=SUM(C$14:C$18)`), and external assumptions are absolute (`='Summary'!$B$4`).
  - *Alternatives considered*: Making all coordinates absolute (`=$C$10-$C$11`). Rejected because copying formulas across periods in Excel would fail to track time, and formula bars would look cluttered.

- **Range Notation for 1D and 2D Blocks**:
  - *Choice*: Map ranges by looking up top-left and bottom-right `CellId` coordinates from `LayoutMap` and emitting `StartA1:EndA1`.
  - *Rationale*: Excel functions (`SUM`, `AVERAGE`) take contiguous rectangular block ranges. Interleaved section headers in row groups contain text labels that Excel functions natively ignore during aggregation.
  - *Alternatives considered*: Emitting comma-separated individual cell lists (`SUM(C14, C15, C16)`). Rejected because it violates institutional modeling standards and hits Excel argument limits on large schedules.

- **AST Serializer Architecture**:
  - *Choice*: Direct recursive AST-to-string lowering using standard buffer formatting with precedence tracking.
  - *Rationale*: Direct, idiomatic Rust with zero external parser/printer dependencies.

### Data Contracts

#### Public Types in `src/compiler/lowering/`

| Struct / Enum | Role | Key Fields / Variants |
| :--- | :--- | :--- |
| `LoweredFormulaMap` | Deterministic map of lowered formulas | `pub formulas: IndexMap<CellId, String>` |
| `FormulaLowerer` | Stateless lowering engine | `lower_cell(...)`, `lower_grid(...)` |
| `LoweringError` | Structured diagnostic enum | `Resolve(ResolveError)`, `MissingCellCoordinate(CellId)`, `EmptyRange(TableRef)`, `SyntaxError(String)` |

### Risks & Trade-offs

- **Section header row spans in vertical aggregations**: Visual section headers interleaved by Pass 5 fall within the row range index span ($R_{start}..R_{end}$). In native Excel, aggregate functions (`SUM`, `AVERAGE`, `MIN`, `MAX`) automatically skip non-numeric string labels. Risk is mitigated because standard financial modeling practices expect this native Excel behavior.
- **Inter-sheet single quote escaping**: Sheet names containing apostrophes or special characters must be properly escaped in Excel formula syntax (e.g. `'O''Reilly'`). The coordinate formatter must safely double internal single quotes.

### Acceptance Criteria

- [x] **Scenario 1: Intra-Sheet Active Column Formula**
  - **Given** an Income Statement table where row `"gross_profit"` defines formula `IncomeStatement["revenue", @col] - IncomeStatement["cogs", @col]`
  - **When** lowering formulas at column `"2024E"` (physical column C, row 12, with revenue at row 10 and cogs at row 11)
  - **Then** the emitted formula string is `"=C10-C11"`.

- [x] **Scenario 2: Intra-Sheet Active Row Formula**
  - **Given** a Cap Table where column `"invested"` defines formula `CapTable[@row, "shares"] * CapTable[@row, "share_price"]`
  - **When** lowering formulas at row `"series_a"` (physical row 5, with shares in column C and price in column D)
  - **Then** the emitted formula string is `"=C5*D5"`.

- [x] **Scenario 3: Relative Temporal Predecessor Offset**
  - **Given** a Balance Sheet where row `"cash"` defines formula `BalanceSheet["cash", col.prev] + CashFlow["net_change", @col]`
  - **When** lowering formulas at column `"2025E"` (physical column D, with row cash at row 10 and net change at row 25)
  - **Then** the emitted formula string is `"=C10+D25"`.

- [x] **Scenario 4: Cross-Sheet Absolute Assumption Reference**
  - **Given** an Income Statement on sheet `"Model"` referencing `Assumptions["tax_rate", "value"]` on sheet `"Assumptions"` (physical cell `B4`)
  - **When** lowering formula `IncomeStatement["ebit", @col] * Assumptions["tax_rate", "value"]` at column C, row 15
  - **Then** the emitted formula string is `"=C14*'Assumptions'!$B$4"`.

- [x] **Scenario 5: Intra-Sheet Specific Key Absolute Reference**
  - **Given** a Debt Schedule formula referencing `DebtSchedule["beginning_balance", "2023A"]` (physical cell `C10`)
  - **When** lowering the formula at column `"2024E"`
  - **Then** the emitted coordinate is `"$C$10"`.

- [x] **Scenario 6: Group Milestone Anchor Reference**
  - **Given** a Debt Schedule formula referencing `DebtSchedule["balance", cols.historical.last]` where historical last resolves to `"2023A"` (`C10`)
  - **When** lowering the formula at column `"2024E"`
  - **Then** the emitted coordinate is `"$C$10"`.

- [x] **Scenario 7: Horizontal Range Row Slice**
  - **Given** a formula `SUM(IncomeStatement["revenue", cols.forecast])` spanning columns C through G at row 10
  - **When** lowering the formula
  - **Then** the emitted formula string is `"=SUM($C$10:$G$10)"`.

- [x] **Scenario 8: Horizontal Range with Active Row**
  - **Given** a formula `SUM(CapTable[@row, "shares".."invested_capital"])` at physical row 5 spanning columns C to E
  - **When** lowering the formula
  - **Then** the emitted formula string is `"=SUM(C5:E5)"`.

- [x] **Scenario 9: Vertical Range Column Slice with Active Column**
  - **Given** a formula `SUM(BalanceSheet[rows.current_assets, @col])` at physical column C where `current_assets` spans physical rows 14 to 18
  - **When** lowering the formula
  - **Then** the emitted formula string is `"=SUM(C$14:C$18)"`.

- [x] **Scenario 10: Vertical Range Column Slice with Specific Column**
  - **Given** a formula `SUM(BalanceSheet[rows.current_assets, "2024E"])` where `"2024E"` is physical column C and `current_assets` spans physical rows 14 to 18
  - **When** lowering the formula
  - **Then** the emitted formula string is `"=SUM($C$14:$C$18)"`.

- [x] **Scenario 11: 2D Block Range**
  - **Given** a formula `SUM(BalanceSheet[rows.current_assets, cols.forecast])` where `current_assets` spans rows 14 to 18 and `forecast` spans columns C to G
  - **When** lowering the formula
  - **Then** the emitted formula string is `"=SUM($C$14:$G$18)"`.

- [x] **Scenario 12: Binary Operator Precedence and Parentheses**
  - **Given** a formula `(A + B) * C / (D - E)`
  - **When** lowering the formula
  - **Then** parentheses are preserved around `(A+B)` and `(D-E)`.

- [x] **Scenario 13: Function Calls with Omitted Positional Arguments**
  - **Given** a formula `XLOOKUP(lookup, array, return, _, 0, 1)` with argument 4 omitted
  - **When** lowering the formula
  - **Then** the emitted formula string preserves empty argument slots: `"=XLOOKUP(lookup,array,return,,0,1)"`.

- [x] **Scenario 14: String Literal Escaping**
  - **Given** a formula `IF(A1="US", "Domestic ""Main""", "Export")`
  - **When** lowering the formula
  - **Then** inner double quotes are escaped as `""`.

- [x] **Scenario 15: Full Compiler Facade Integration**
  - **Given** a valid `ProjectManifest`, `&[TableSpec]`, active `Theme`, and planned `LayoutMap`
  - **When** invoking `Compiler::lower_formulas(&manifest, tables, &layout_map)`
  - **Then** returns a `LoweredFormulaMap` containing lowered formula strings for every formula cell in the expanded grid.

---

## Tasks

### Phase 1: Diagnostics & Contracts
- [x] Create `src/compiler/lowering/error.rs` defining `LoweringError`.
- [x] Update `src/compiler/error.rs` to include `CompilerError::Lowering(#[from] LoweringError)`.
- [x] Define `LoweredFormulaMap` in `src/compiler/lowering/mod.rs`.

### Phase 2: Coordinate & Reference Lowering
- [x] Implement coordinate formatting helper supporting relative/absolute `$` and single-quoted sheet qualification.
- [x] Implement reference lowering mapping single cells, horizontal slices, vertical slices, and 2D blocks to canonical Excel addresses.

### Phase 3: AST Expression Serialization
- [x] Implement recursive AST serializer covering literals, strings with double-quote escaping, booleans, and omitted arguments.
- [x] Implement operator serializer with precedence-aware parenthesization.
- [x] Implement function call serializer with uppercase naming and argument separation.
- [x] Prepend `=` to all formula roots.

### Phase 4: Compiler Facade & Integration Tests
- [x] Implement `FormulaLowerer::lower_all` and expose `Compiler::lower_formulas`.
- [x] Create `tests/formula_lowering_test.rs` covering Scenarios 1–15.
- [x] Run full verification suite (`cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`).
