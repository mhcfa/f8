---
title: Formula Lowering & Reference Coordinate Resolution
last_updated: 2026-09-20
domain: compiler
status: active
---

## Purpose

The `compiler::lowering` subsystem provides Pass 6a of the Finx compilation pipeline, transforming high-level semantic AST expressions (`Expr`) into concrete Excel formula strings (e.g. `"=C10-C11"`, `"=SUM(C$14:C$18)"`, `"='Summary'!$B$4"`). It arbitrates coordinate referencing rules (`$`), sheet qualification, uppercase Excel function names, and operator precedence parenthesization using the deterministic `LayoutMap` produced by Pass 5.

## Architecture & Boundaries

- **Modules & components**:
  - `finx::compiler::lowering::address`: Formats 0-indexed column/row coordinates into Excel A1 addresses with `$` pinning (`format_cell_address`), handles single-quoted worksheet qualifiers (`format_sheet_qualifier`), and constructs single-cell (`format_single_cell`) or range (`format_range`) address strings.
  - `finx::compiler::lowering::error`: Structured domain errors (`LoweringError`).
  - `finx::compiler::lowering::mod`: Public types (`LoweredFormulaMap`, `FormulaLowerer`) and AST-to-Excel serializer.
  - `finx::compiler::mod`: High-level compiler driver facade (`Compiler::lower_formulas`).
- **Data flow & interfaces**:
  1. Input: `CellGrid`, `SymbolTable`, and `LayoutMap`.
  2. Reference resolution: Resolves `TableRef` row and column selectors against `SymbolTable` to locate top-left and bottom-right `CellId` coordinates.
  3. Coordinate mapping: Queries `LayoutMap` to obtain physical worksheet grid coordinates `(sheet, col, row)`.
  4. Pinning arbitration: Determines relative vs. absolute `$` pinning based on whether selectors are dynamic (`@col`, `@row`, `prev`, `next`) or static (`Key`, `Range`, `Group`).
  5. Sheet qualification: Injects `'Sheet'!` prefix whenever target table is on a different worksheet from the evaluating cell.
  6. AST serialization: Serializes operators with precedence-governed parentheses, uppercase function names, double-quote escaped strings, and empty slots for omitted arguments.
  7. Output: Deterministic `LoweredFormulaMap` containing `=`-prefixed formula strings consumed by Pass 6b (Workbook Emission).
- **Dependencies**:
  - `indexmap`: Guarantees deterministic iteration ordering for lowered formula maps.
  - `thiserror`: Diagnostic domain error hierarchy.
  - `serde`: Serialization of lowered formula maps for inspection.

## Requirements & Invariants

### 1. Mandatory Formula Root Prefix

All lowered formula cells must produce formula strings starting with `=`. Sub-expressions inside function calls or binary operations must omit the leading `=`.

#### Scenarios

- **Formula root prefix**:
  - **When** lowering any formula expression
  - **Then** the resulting string begins with `=`.

### 2. Relative Coordinate Lowering for Dynamic Axes

References using dynamic column (`@col`, `col.prev`, `col.next`) or row (`@row`, `row.prev`, `row.next`) selectors emit relative coordinates without `$`.

#### Scenarios

- **Active column reference**:
  - **When** evaluating `Table["revenue", @col]` in column C
  - **Then** emits relative column `C` (e.g. `=C6`).
- **Active row reference**:
  - **When** evaluating `Table[@row, "shares"]` in row 18
  - **Then** emits relative row `18` (e.g. `=B18`).
- **Temporal predecessor offset**:
  - **When** evaluating `Table["cash", col.prev]` at column D
  - **Then** emits relative predecessor column `C` (e.g. `=C25`).

### 3. Absolute Coordinate Lowering for Static Coordinates

References where both row and column are static keys, group milestone anchors, or cross-sheet references emit absolute coordinates with `$` (e.g. `=$B$41`, `='Summary'!$B$4`).

#### Scenarios

- **Intra-sheet static key**:
  - **When** evaluating `Table["balance", "2023A"]`
  - **Then** emits absolute coordinates (e.g. `=$B$41`).
- **Cross-sheet assumption reference**:
  - **When** referencing an assumption on another worksheet
  - **Then** emits single-quoted sheet name and absolute coordinates (e.g. `='Assumptions'!$B$5`).

### 4. Slice Range Lowering

Range slices format as `TopLeft:BottomRight`:
- **Vertical range with active column** (`Table["cash".."inventory", @col]`): Relative column, pinned row bounds (e.g. `C$25:C$27`).
- **Horizontal range with active row** (`Table[@row, "2024E".."2026E"]`): Relative row bounds, relative columns (e.g. `B18:C18`).
- **Fixed ranges & 2D blocks**: Fully absolute bounds (e.g. `=$C$6:$F$6`, `=$C$9:$F$10`).

#### Scenarios

- **Column slice aggregation**:
  - **When** lowering `SUM(Table["cash".."inventory", @col])`
  - **Then** emits `=SUM(C$25:C$27)`.
- **2D block aggregation**:
  - **When** lowering `SUM(Table["r_and_d".."s_and_m", "2024E".."2027E"])`
  - **Then** emits `=SUM($C$9:$F$10)`.

### 5. Operator Precedence and Associativity

Expressions with lower precedence than their parent operator are wrapped in parentheses. Left-associative operations on the right-hand side of non-commutative operators (`-`, `/`) and right-hand operations of exponentiation (`^`) are wrapped in parentheses to counteract Excel's non-standard left-to-right power evaluation.

#### Scenarios

- **Compound arithmetic expression**:
  - **When** lowering `(A + B) * C / (D - E)`
  - **Then** emits `=(C6+C7)*2/(C11-10)` with exact single-parenthesis pairs.
- **Exponentiation associativity**:
  - **When** lowering `A ^ (B ^ C)` or `(A ^ B) ^ C`
  - **Then** emits `=C6^(C7^2)` or `=(C6^C7)^2` respectively.

### 6. Function Calls and Omitted Arguments

Function names serialize in canonical uppercase (`SUM`, `IF`, `XLOOKUP`). Positional arguments that are omitted (`Expr::Omitted`) preserve empty comma slots (e.g. `,,`).

#### Scenarios

- **Omitted argument in XLOOKUP**:
  - **When** lowering `XLOOKUP(val, lookup_range, return_range, _, 0, 1)`
  - **Then** emits `=XLOOKUP(val,lookup_range,return_range,,0,1)`.
