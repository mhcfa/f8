---
title: Formula Expression AST & Pratt Parser
last_updated: 2026-09-18
domain: expr
status: active
---

## Purpose

The `expr` subsystem provides the Abstract Syntax Tree (AST), structured diagnostic errors, canonical string emission, and top-down operator precedence (Pratt) parser for f8 formula expressions. It translates declarative formula strings into strongly typed trees for consumption by downstream compiler passes (dependency graph analysis, spatial lowering, and calculation evaluation).

## Architecture & Boundaries

- **Modules & components**:
  - `f8_core::expr::ast`: Strongly typed AST definitions (`Expr`, `BinaryOp`, `UnaryOp`, `Literal`, `TableRef`, `RowSelector`, `ColSelector`, `RelativeOffset`) and `Display` implementations.
  - `f8_core::expr::error`: Diagnostic domain errors (`ParseError`).
  - `f8_core::expr::parser`: Two-stage parsing pipeline:
    - Lexer: `tokenize` parses `&str` into `Vec<SpannedToken>` using the borrowed `Token` enum with zero external dependencies.
    - Pratt parser: `TokenStream` cursor and operator precedence parsing loop over strongly typed tokens.
  - `f8_core::expr::Expr::parse`: Public parsing facade.
- **Data flow & interfaces**:
  1. Formula string slice (`&str`) $\rightarrow$ `tokenize` $\rightarrow$ `Vec<SpannedToken>` $\rightarrow$ `TokenStream` $\rightarrow$ `Expr::parse` $\rightarrow$ `Result<Expr, ParseError>`.
  2. `Expr::to_string()` / `Display` $\rightarrow$ Canonical formula string adhering to operator precedence and associativity.
  3. Parsed `Expr` is consumed by `compiler::graph` for dependency extraction and `compiler::lowering` for cell coordinate resolution.
- **Dependencies**:
  - `thiserror`: Structured domain and diagnostic error hierarchy.
  - `serde`: Serialization and deserialization of AST representations.

## Requirements & Invariants

### 1. Algebraic Precedence & Associativity

Formulas must strictly observe algebraic precedence across five tiers:
1. Exponentiation (`^`, tier 5, right-associative).
2. Unary positive and negation (`+`, `-`, tier 4, prefix right-associative).
3. Multiplicative (`*`, `/`, tier 3, left-associative).
4. Additive (`+`, `-`, tier 2, left-associative).
5. Relational comparisons (`=`, `<>`, `<`, `<=`, `>`, `>=`, tier 1, left-associative).

#### Scenarios

- **Precedence Hierarchy**:
  - **When** parsing `"1 + 2 * 3 ^ 2"`
  - **Then** evaluates to `1 + (2 * (3 ^ 2))`.
- **Unary Over Exponentiation**:
  - **When** parsing `"-2 ^ 2"`
  - **Then** evaluates to `-(2 ^ 2)`.
- **Right Associativity**:
  - **When** parsing `"2 ^ 3 ^ 4"`
  - **Then** evaluates to `2 ^ (3 ^ 4)`.
- **Parenthetical Override**:
  - **When** parsing `"(1 + 2) * 3"`
  - **Then** parentheses explicitly override default operator binding.

### 2. Mandatory Two-Coordinate Addressing & Orthogonal 2D Indexing

Every table reference must specify both a row selector and a column selector separated by a comma: `Table[RowSelector, ColSelector]`. Legacy dot-access (`Table.row[...]`) and bare identifiers are strictly rejected. Active evaluation coordinates must explicitly name the axis (`@col` or `@row`).

#### Scenarios

- **Valid Orthogonal 2D Access**:
  - **When** referencing `Table["row", @col]`, `Table[@row, "col"]`, or `Table["row", "col"]`
  - **Then** parses into `TableRef` with `{ table, row, col }`.
- **Legacy Dot-Access Rejection**:
  - **When** encountering `IncomeStatement.revenue[@col]` or `IncomeStatement.revenue`
  - **Then** returns `ParseError::UnexpectedToken` pointing to `.` after table identifier.
- **Bare Reference Rejection**:
  - **When** encountering bare `IncomeStatement` without brackets
  - **Then** returns `ParseError::UnexpectedToken`.
- **Retired Bare '@' Rejection**:
  - **When** encountering bare `@` (e.g. `Table["row", @]`)
  - **Then** returns `ParseError::RetiredBareAt` directing caller to `@col` or `@row`.

### 3. Strictly Quoted Coordinate Keys & Relative Offsets

All coordinate row and column keys inside brackets must be double-quoted strings (e.g. `["revenue", @col]`, `["tax_rate", "value"]`, `["revenue", "2024E".."2028E"]`). Bare identifiers are prohibited unless they match recognized temporal navigation syntax (`col.prev`, `col.next`, `row.prev`, `row.next` with optional `(N)` step count).

#### Scenarios

- **Quoted Key Coordinate**:
  - **When** referencing `IncomeStatement["revenue", "2024E"]`
  - **Then** parses into `RowSelector::Key("revenue")` and `ColSelector::Key("2024E")`.
- **Relative Temporal Offset**:
  - **When** referencing `IncomeStatement[@row, col.prev]` or `IncomeStatement[@row, col.prev(2)]`
  - **Then** parses into `ColSelector::Offset(RelativeOffset::Prev(1))` or `ColSelector::Offset(RelativeOffset::Prev(2))`.
- **Unquoted Key Rejection**:
  - **When** encountering `IncomeStatement["revenue", 2024E]` or `Opex[r_and_d, @col]`
  - **Then** returns `ParseError::UnquotedCoordinateKey`.

### 4. Uppercase Function Names

Built-in Excel functions must be uppercase identifiers matching `[A-Z][A-Z0-9_]*`.

#### Scenarios

- **Valid Function Call**:
  - **When** invoking `SUM(...)`, `IF(...)`, or `AVERAGE(...)`
  - **Then** parses into `Expr::FunctionCall`.
- **Lowercase Function Rejection**:
  - **When** invoking `sum(1, 2)`
  - **Then** returns `ParseError::InvalidFunctionName`.

### 5. First-Class Booleans & Omitted Argument Confinement

Formulas support native uppercase Excel boolean literals (`TRUE`, `FALSE`) and explicit positional argument omission in function calls via the underscore placeholder (`_`).

- **Literal Booleans**: `TRUE` and `FALSE` parse as `Literal::Boolean` and round-trip via `Display`. Lowercase `true` / `false` are rejected as bare identifiers.
- **Omitted Argument Placeholder (`_`)**: Optional positional parameters can be omitted explicitly via `_` (e.g. `PMT(r, n, pv, _, 1)`). Omitted arguments parse as `Expr::Omitted` and contribute zero dependency references during graph extraction.
- **Grammatical Confinement**: The `_` token is strictly confined to function call argument lists (`Argument = Expression | OmittedArg`). Using `_` in arithmetic expressions (`1 + _`, `-_`, `_ * 2`), table coordinate selectors (`Table[_, @col]`), or table identifiers fails at parse time with `ParseError::UnexpectedToken`.
- **Complete Purge of `null`**: The `null` keyword is prohibited in formula expressions. Encountering `null` produces `ParseError::UnexpectedToken`.

#### Scenarios

- **Boolean Literal Roundtrip**:
  - **When** parsing `"TRUE"` and `"FALSE"`
  - **Then** parses into `Expr::Literal(Literal::Boolean(true))` and `Expr::Literal(Literal::Boolean(false))`.
- **Explicit Argument Omission**:
  - **When** parsing `"PMT(0.05, 10, 1000, _, 1)"`
  - **Then** argument 3 parses as `Expr::Omitted` and formats back as `"_"`.
- **Confinement Rejection**:
  - **When** parsing `"1 + _"`, `"-_"`, or `"Table[_, @col]"`
  - **Then** parsing fails with `ParseError::UnexpectedToken`.
- **Formula Null Rejection**:
  - **When** parsing `"null"` or `"IF(A1, B1, null)"`
  - **Then** parsing fails with `ParseError::UnexpectedToken`.

