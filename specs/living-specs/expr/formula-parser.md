---
title: Formula Expression AST & Pratt Parser
last_updated: 2026-09-16
domain: expr
status: active
---

## Purpose

The `expr` subsystem provides the Abstract Syntax Tree (AST), structured diagnostic errors, canonical string emission, and top-down operator precedence (Pratt) parser for Finx formula expressions. It translates declarative formula strings into strongly typed trees for consumption by downstream compiler passes (dependency graph analysis, spatial lowering, and calculation evaluation).

## Architecture & Boundaries

- **Modules & components**:
  - `finx::expr::ast`: Strongly typed AST definitions (`Expr`, `BinaryOp`, `UnaryOp`, `Literal`, `TableRef`, `RefAccess`, `RowSelector`, `ColSelector`, `SpecialRow`, `SpecialCol`) and `Display` implementations.
  - `finx::expr::error`: Diagnostic domain errors (`ParseError`).
  - `finx::expr::parser`: Two-stage parsing pipeline:
    - Lexer: `tokenize` parses `&str` into `Vec<SpannedToken>` using the borrowed `Token` enum with zero external dependencies.
    - Pratt parser: `TokenStream` cursor and operator precedence parsing loop over strongly typed tokens.
  - `finx::expr::Expr::parse`: Public parsing facade.
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

### 2. Mandatory Two-Coordinate Addressing

Every reference must specify both a row selector and a column selector. Bare table or row identifiers without coordinate brackets are strictly rejected.

#### Scenarios

- **Valid Dot Access**:
  - **When** referencing `Table.row[@]` or `Table.row["col"]`
  - **Then** parses into `TableRef` with `RefAccess::Dot`.
- **Valid Bracket Access**:
  - **When** referencing `Table[row, col]`
  - **Then** parses into `TableRef` with `RefAccess::Bracket`.
- **Bare Reference Rejection**:
  - **When** encountering `IncomeStatement.revenue` without brackets
  - **Then** returns `ParseError::UnexpectedToken`.

### 3. Strictly Quoted Coordinate Keys

All coordinate row and column keys inside brackets must be double-quoted strings (e.g. `["value"]`, `["2024E".."2028E"]`). Bare identifiers are prohibited unless they match recognized semantic prefixes (`rows.`, `cols.`, `col.prev`, `col.next`).

#### Scenarios

- **Quoted Key Coordinate**:
  - **When** referencing `IncomeStatement.revenue["2024E"]`
  - **Then** parses into `ColSelector::Key("2024E")`.
- **Unquoted Key Rejection**:
  - **When** encountering `IncomeStatement.revenue[2024E]` or `Opex[r_and_d, @]`
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
