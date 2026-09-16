---
title: Formula Expression AST & Pratt Parser
date: 2026-09-16
status: completed
target-living-spec: specs/living-specs/expr/formula-parser.md
---

## Proposal

### What

Implement the formula expression Abstract Syntax Tree (AST), structured error diagnostics, and a top-down operator precedence (Pratt) parser using `winnow` in [`src/expr/`](../../src/expr/mod.rs). This implements the formal grammar defined in [`docs/dsl/grammar.md`](../../docs/dsl/grammar.md) and reference addressing rules defined in [`docs/dsl/addressing.md`](../../docs/dsl/addressing.md).

### Why

The specification AST in [`src/model/`](../../src/model/mod.rs) captures formula calculations as raw strings (e.g. within `RowSpec::expr` and `MatchArmSpec::expr`). To perform dependency graph construction, topological sorting, cycle detection, coordinate lowering, and formula evaluation, the compiler requires a strongly typed, validated expression tree.

### Scope

- **In-scope**:
  - AST data structures: `Expr`, `BinaryOp`, `UnaryOp`, `Literal`, `TableRef`, `RefAccess`, `RowSelector`, `ColSelector`, `SpecialRow`, `SpecialCol`.
  - Structured domain error types: `ParseError` using `thiserror`.
  - Tokenization and combinator parsing using `winnow`.
  - Pratt expression parsing handling 5 precedence levels and associativity:
    - Exponentiation (`^`, right-associative).
    - Unary operators (`+`, `-`, prefix right-associative).
    - Multiplicative (`*`, `/`, left-associative).
    - Additive (`+`, `-`, left-associative).
    - Relational comparisons (`=`, `<>`, `<`, `<=`, `>`, `>=`, left-associative).
  - Built-in uppercase function call parsing (`SUM(...)`, `AVERAGE(...)`, `IF(...)`).
  - Reference syntax parsing:
    - Dot access: `Table.row[ColSpec]`.
    - Bracket access: `Table[RowSpec, ColSpec]`.
    - Temporal column offsets: `col.prev`, `col.prev(k)`, `col.next`, `col.next(k)`.
    - Named group boundaries: `rows.group`, `rows.group.first`, `rows.group.last`, `cols.group`, `cols.group.first`, `cols.group.last`.
    - Active column symbol: `@`.
    - Quoted range selectors: `"start".."end"`.
  - `Display` implementation for canonical formula re-emission and debugging.
  - Unit and integration test suite covering grammar edge cases and addressing variants.
- **Out-of-scope**:
  - Semantic symbol resolution (verifying whether tables, rows, or column groups exist in the project).
  - Formula coordinate lowering to Excel A1 notation (handled in `compiler::lowering`).
  - Formula evaluation and runtime math (handled in `compiler::eval`).

### What Changes

- **Dependencies**: Add `winnow = "1.0"` (or compatible) to `Cargo.toml`.
- **Modules in `src/expr/`**:
  - [`src/expr/mod.rs`](../../src/expr/mod.rs): Re-export AST, parser facade (`Expr::parse`), and error types.
  - `src/expr/ast.rs`: AST node definitions and `Display` formatting.
  - `src/expr/error.rs`: Structured `ParseError` enum with diagnostic spans/offsets.
  - `src/expr/parser.rs`: `winnow` combinators and Pratt precedence parser.

### Capabilities

- **New capabilities**:
  - Parse any valid Finx formula string into an `Expr` AST with zero production panics.
  - Reject malformed formulas (unclosed parentheses, invalid operators, bare coordinate keys) with actionable error diagnostics.
  - Distinguish between cell references, horizontal slices, vertical slices, and 2D block ranges at the syntax level.
- **Modified capabilities**: None (initial implementation of the `expr` subsystem).

---

## Design

### Goals

1. **Strict grammar compliance**: Adhere strictly to the EBNF grammar in [`docs/dsl/grammar.md`](../../docs/dsl/grammar.md).
2. **Algebraic correctness**: Honor standard precedence and associativity rules (e.g. `1 + 2 * 3 ^ 2` $\rightarrow$ `1 + (2 * (3 ^ 2))`, `-2 ^ 2` $\rightarrow$ `-(2 ^ 2)`).
3. **Zero production panics**: Return `Result<Expr, ParseError>` with no `.unwrap()` or `.expect()` calls.
4. **Clean boundary**: Pure syntactic parsing; no coupling to `TableSpec` or workbook symbol tables.

### Invariants

- **Mandatory two-coordinate references**: Every reference specifies both row and column selectors. Bare row references (e.g. `IncomeStatement.revenue`) are rejected.
- **Strictly quoted coordinate keys**: All row and column keys inside bracket selectors must be double-quoted strings (e.g. `["value"]`, `["2024E"]`, `["r_and_d".."g_and_a"]`). Bare identifiers inside coordinate brackets are syntax errors.
- **Uppercase function names**: Function identifiers must match `[A-Z][A-Z0-9_]*`. Lowercase function calls (e.g. `sum(...)`) are rejected.

### Decisions

- **Use `winnow` for parser combinators**:
  - *Rationale*: `winnow` is fast, zero-copy capable, actively maintained, and idiomatic in modern Rust. It allows clear separation of token combinators and precedence loops.
  - *Alternatives considered*: Hand-written recursive descent (more boilerplate and manual cursor handling); `pest` / `nom` (`pest` requires separate grammar files and runtime overhead; `nom` is the older predecessor to `winnow`).
- **Pratt parsing loop for binary and unary expressions**:
  - *Rationale*: Pratt parsing elegantly handles operator precedence and associativity without deeply nested grammar recursion or grammar ambiguity.
  - *Alternatives considered*: Classical shunting-yard (requires explicit operator/operand stacks); pure recursive descent with 5 helper functions (one per precedence level, less extensible).

### Data Contracts

#### AST Types (`src/expr/ast.rs`)

| Type | Kind | Variants / Fields | Description |
| :--- | :--- | :--- | :--- |
| `Expr` | `enum` | `Literal(Literal)`, `Reference(TableRef)`, `Unary { op, expr }`, `Binary { op, left, right }`, `FunctionCall { name, args }` | Root expression AST node |
| `Literal` | `enum` | `Number(f64)`, `String(String)`, `Null` | Atomic constant literals |
| `BinaryOp` | `enum` | `Add`, `Sub`, `Mul`, `Div`, `Pow`, `Eq`, `NotEq`, `Lt`, `Lte`, `Gt`, `Gte` | Binary operators |
| `UnaryOp` | `enum` | `Pos`, `Neg` | Unary prefix operators |
| `TableRef` | `struct` | `table: TableId`, `access: RefAccess` | Reference to a table coordinate or slice |
| `RefAccess` | `enum` | `Dot { row: RowId, col: ColSelector }`, `Bracket { row: RowSelector, col: ColSelector }` | Addressing shape (`Table.row[...]` vs `Table[...]`) |
| `RowSelector` | `enum` | `Key(RowId)`, `Range(RowId, RowId)`, `Special(SpecialRow)` | Row coordinate selector |
| `ColSelector` | `enum` | `Active`, `Key(ColId)`, `Range(ColId, ColId)`, `Special(SpecialCol)` | Column coordinate selector |
| `SpecialRow` | `enum` | `Group(RowGroupId)`, `GroupFirst(RowGroupId)`, `GroupLast(RowGroupId)` | Semantic row milestone or group |
| `SpecialCol` | `enum` | `Prev(u32)`, `Next(u32)`, `Group(ColGroupId)`, `GroupFirst(ColGroupId)`, `GroupLast(ColGroupId)` | Temporal offset or column group |

#### Diagnostic Error (`src/expr/error.rs`)

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("Unexpected end of expression, expected {expected}")]
    UnexpectedEof { expected: &'static str },

    #[error("Unexpected token at offset {offset}: expected {expected}, found '{found}'")]
    UnexpectedToken {
        expected: &'static str,
        found: String,
        offset: usize,
    },

    #[error("Invalid number literal at offset {offset}: '{raw}'")]
    InvalidNumber { raw: String, offset: usize },

    #[error("Invalid function name '{name}' at offset {offset}: must be uppercase [A-Z][A-Z0-9_]*")]
    InvalidFunctionName { name: String, offset: usize },

    #[error("Coordinate key must be quoted at offset {offset}")]
    UnquotedCoordinateKey { offset: usize },
}
```

### Risks & Trade-offs

- **Operator ambiguity (`+` and `-`)**: `+` and `-` can be either unary prefix or binary infix.
  - *Mitigation*: Pratt parser checks token context: at the beginning of an expression or immediately following an operator/left parenthesis, `+` and `-` are parsed as prefix unary operators.
- **Float representation**: `Literal::Number` stores `f64`.
  - *Mitigation*: Complies with standard Excel double-precision floating point semantics.
- **Whitespace handling**: Formulas may contain arbitrary whitespace around operators, commas, and brackets.
  - *Mitigation*: Built-in whitespace combinator strips leading/trailing trivia around tokens.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Standard Arithmetic Precedence & Associativity**
  - **Given** formula `"1 + 2 * 3 ^ 2"`
  - **When** `Expr::parse` is invoked
  - **Then** expression parses as `Add(1, Mul(2, Pow(3, 2)))`.

- [x] **Scenario 2: Unary Operator Precedence**
  - **Given** formula `"-2 ^ 2"`
  - **When** `Expr::parse` is invoked
  - **Then** expression parses as `Neg(Pow(2, 2))`.

- [x] **Scenario 3: Relational Comparisons**
  - **Given** formula `"a.b[@] >= 100"`
  - **When** `Expr::parse` is invoked
  - **Then** expression parses as `Binary { op: BinaryOp::Gte, left: TableRef, right: Literal(100.0) }`.

- [x] **Scenario 4: Function Calls & Argument Lists**
  - **Given** formula `"IF(Assumptions.flag[\"active\"] = 1, 100, 0)"`
  - **When** `Expr::parse` is invoked
  - **Then** expression parses as `FunctionCall` with name `"IF"` and 3 argument expressions.

- [x] **Scenario 5: Dot Access References**
  - **Given** formulas:
    - `"IncomeStatement.revenue[@]"`
    - `"Assumptions.tax_rate[\"value\"]"`
    - `"BalanceSheet.cash[col.prev]"`
    - `"BalanceSheet.cash[col.prev(2)]"`
    - `"DebtSchedule.balance[cols.historical.last]"`
  - **When** `Expr::parse` is invoked
  - **Then** all parse into `TableRef` with matching `Dot` access and `ColSelector` variants.

- [x] **Scenario 6: Bracket Access Slices & 2D Ranges**
  - **Given** formulas:
    - `"Opex[\"r_and_d\"..\"g_and_a\", @]"`
    - `"BalanceSheet[rows.current_assets, @]"`
    - `"Opex[\"r_and_d\"..\"g_and_a\", \"2024E\"..\"2028E\"]"`
    - `"BalanceSheet[rows.current_assets, cols.forecast]"`
  - **When** `Expr::parse` is invoked
  - **Then** all parse into `TableRef` with matching `Bracket` access and corresponding row/column selectors.

- [x] **Scenario 7: Reject Bare Identifiers in Coordinates**
  - **Given** formula `"IncomeStatement.revenue[2024E]"` or `"Opex[r_and_d, @]"`
  - **When** `Expr::parse` is invoked
  - **Then** parsing fails with `ParseError::UnquotedCoordinateKey` or `ParseError::UnexpectedToken`.

- [x] **Scenario 8: Reject Lowercase Function Names**
  - **Given** formula `"sum(1, 2)"`
  - **When** `Expr::parse` is invoked
  - **Then** parsing fails with `ParseError::InvalidFunctionName` or `ParseError::UnexpectedToken`.

- [x] **Scenario 9: Display / Formatting Roundtrip**
  - **Given** a parsed expression `expr`
  - **When** `expr.to_string()` is formatted
  - **Then** the resulting string re-parses into an identical AST structure.

---

## Tasks

### Phase 1: Dependencies & AST Definition

- [x] Add `winnow` to `Cargo.toml`.
- [x] Create `src/expr/ast.rs` with `Expr`, `BinaryOp`, `UnaryOp`, `Literal`, `TableRef`, `RefAccess`, `RowSelector`, `ColSelector`, `SpecialRow`, `SpecialCol`.
- [x] Implement `Display` for all AST nodes for canonical string emission.
- [x] Create `src/expr/error.rs` with `ParseError`.

### Phase 2: Lexer & Pratt Parser Implementation

- [x] Implement token combinators in `src/expr/parser.rs` (literals, identifiers, coordinate selectors, whitespace trimming).
- [x] Implement Pratt precedence parser loop for prefix unary and infix binary expressions.
- [x] Implement function call and argument list combinators.
- [x] Implement dot access and bracket access reference combinators.
- [x] Implement `Expr::parse(input: &str) -> Result<Expr, ParseError>` facade in [`src/expr/mod.rs`](../../src/expr/mod.rs).

### Phase 3: Testing & Verification

- [x] Add unit tests in `src/expr/` covering arithmetic precedence, associativity, parentheses grouping, function calls, and error cases.
- [x] Add tests for all addressing modes from [`docs/dsl/addressing.md`](../../docs/dsl/addressing.md).
- [x] Run full verification suite:
  - `cargo check`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`

