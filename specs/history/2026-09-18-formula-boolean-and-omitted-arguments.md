---
title: Formula Boolean Literals & Omitted Argument Placeholder (_)
date: 2026-09-18
status: completed
target-living-spec: specs/living-specs/expr/formula-parser.md
---

## Proposal

### What

Introduce first-class boolean literals (`TRUE`, `FALSE`) and an explicit omitted argument placeholder (`_`) to the Finx formula expression syntax (`finx::expr`). Completely purge obsolete `Literal::Null`, `Expr::null()`, and `Token::Null` from the formula expression grammar and parser, while preserving JSON cell input null handling (`finx::model::ValueLiteral::Null`).

### Why

1. **Excel parity**: Standard Excel formulas use uppercase `TRUE` and `FALSE` as first-class boolean literals.
2. **Syntactic and semantic isolation**: In spreadsheet modeling, omitted positional arguments (e.g. `PMT(r, n, pv, _, 1)`) are fundamentally distinct from empty cell values (`CellValue::Empty` originating from JSON grid inputs). Allowing `null` in formula expressions permitted nonsensical expressions like `1 + null` or `=null`. Replacing formula argument omission with `_` and confining `_` strictly to function call argument lists (`Argument = Expression | OmittedArg`) makes arithmetic with omitted arguments (`1 + _`) an immediate syntax error.
3. **Greenfield minimalism**: Purging legacy `null` from `finx::expr` eliminates dead AST variants, unused constructors, and backward-compatibility shims across the codebase.

### Scope

- **In-scope**:
  - `src/expr/ast.rs`:
    - Add `Literal::Boolean(bool)` to `Literal` and format as uppercase `"TRUE"` / `"FALSE"` in `Display`.
    - Add `Expr::Omitted` to `Expr` and format as `"_"` in `Display`.
    - Purge `Literal::Null` and `Expr::null()`.
  - `src/expr/parser.rs`:
    - Add `Token::Boolean(bool)` and `Token::Underscore` to `Token`.
    - Purge `Token::Null`.
    - Update tokenizer:
      - Emit `Token::Boolean(true)` for `"TRUE"`.
      - Emit `Token::Boolean(false)` for `"FALSE"`.
      - Emit `Token::Underscore` for standalone `_`.
      - Preserve multi-character identifiers containing underscores (e.g. `_rev`, `tax_rate`).
      - Reject bare `_` table/group identifiers.
    - Update Pratt parser:
      - In `parse_primary`, parse `Token::Boolean(b)` into `Expr::Literal(Literal::Boolean(b))`.
      - In `parse_argument_list`, parse `Token::Underscore` into `Expr::Omitted`.
      - Enforce grammatical confinement: encountering `Token::Underscore` in `parse_expression` or `parse_primary` returns `ParseError::UnexpectedToken`.
  - `src/compiler/graph/resolve.rs`:
    - Add `Expr::Omitted => {}` to `collect_refs_recursive`.
  - Tests:
    - Update existing parser test in `tests/formula_parser_test.rs` from `null` to `_`.
    - Add unit tests in `src/expr/parser.rs` verifying boolean parsing, argument omission, identifier preservation, and rejection of `_` and `null` in invalid positions.
- **Out-of-scope**:
  - Function signature validation and argument binding (covered in [`specs/drafts/2026-09-18-function-registry-skeleton.md`](specs/drafts/2026-09-18-function-registry-skeleton.md)).
  - Runtime calculation values `CellValue` and `ExcelError` (covered in [`specs/drafts/2026-09-18-function-registry-skeleton.md`](specs/drafts/2026-09-18-function-registry-skeleton.md)).
  - Topological evaluation (Phase 4B: `compiler::eval`).

### What Changes

- `src/expr/ast.rs`: Add `Literal::Boolean`, add `Expr::Omitted`, remove `Literal::Null`, remove `Expr::null()`, update `Display`.
- `src/expr/parser.rs`: Add `Token::Boolean(bool)`, add `Token::Underscore`, remove `Token::Null`, update tokenizer and parsing routines.
- `src/compiler/graph/resolve.rs`: Add `Expr::Omitted => {}` to `collect_refs_recursive`.
- `tests/formula_parser_test.rs`: Update `IF` test with `_`.

### Capabilities

- **New capabilities**:
  - Parsing uppercase `TRUE` and `FALSE` as boolean formula literals.
  - Parsing `_` as an explicit omitted argument placeholder within function argument lists.
  - Round-tripping formula expressions containing booleans and omitted arguments via `Display`.
- **Modified capabilities**:
  - Formulas containing `null` are rejected with `ParseError::UnexpectedToken`.
  - Formulas attempting arithmetic or operations on `_` (e.g. `1 + _`, `-_`, `_ * 2`) are rejected with `ParseError::UnexpectedToken`.

---

## Design

### Goals

1. **Grammatical confinement**: `_` is valid strictly within function call argument lists.
2. **Excel parity**: Native boolean literals `TRUE` and `FALSE`.
3. **Greenfield minimalism**: Zero legacy shims or stubs for `null` in `finx::expr`.

### Invariants

1. **Confinement invariant**: `Expr::Omitted` cannot be constructed outside `parse_argument_list`.
2. **Case sensitivity**: Booleans must be uppercase `TRUE` and `FALSE`. Lowercase `true` / `false` are rejected as bare identifiers.
3. **Identifier preservation**: Identifiers containing underscores (e.g. `_rev`, `gross_margin`) continue to tokenize as `Token::Ident`; only standalone `_` tokenizes as `Token::Underscore`.
4. **Reference extraction purity**: `Expr::Omitted` contributes zero cell or table references in dependency graph analysis.

### Decisions

- **Decision 1: `Expr::Omitted` as an AST variant of `Expr`**:
  - *Choice*: Represent omitted arguments directly as `Expr::Omitted`.
  - *Rationale*: Allows `FunctionCall.args: Vec<Expr>` to remain a uniform collection without adding intermediate wrapper enums like `Argument { Expr, Omitted }`. All existing AST passes match directly on `Expr`.
  - *Alternatives considered*: Introducing `pub enum Argument { Expr(Expr), Omitted }`. Rejected as unnecessary wrapper complexity.
- **Decision 2: Tokenizer disambiguation of `_` vs. identifiers**:
  - *Choice*: In the identifier lexing loop, if the scanned token string is exactly `"_"`, emit `Token::Underscore`. If it contains additional characters (e.g. `_id` or `id_`), emit `Token::Ident`.
  - *Rationale*: Clean, zero-allocation disambiguation preserving existing identifier rules while isolating the keyword placeholder.
- **Decision 3: Complete purge of `null` from `finx::expr`**:
  - *Choice*: Remove `Literal::Null`, `Expr::null()`, and `Token::Null` entirely.
  - *Rationale*: Excel formulas have no `null` literal. `null` in Finx exists strictly in JSON grid manifests (`ValueLiteral::Null`), lowering to `CellValue::Empty`.

### Data Contracts

#### AST Definitions (`src/expr/ast.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Literal {
    Number(f64),
    String(String),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Literal(Literal),
    Reference(TableRef),
    Unary { op: UnaryOp, expr: Box<Expr> },
    Binary { op: BinaryOp, left: Box<Expr>, right: Box<Expr> },
    FunctionCall { name: String, args: Vec<Expr> },
    Omitted,
}
```

#### Token Definitions (`src/expr/parser.rs`)

```rust
pub(crate) enum Token<'a> {
    Number(f64),
    String(String),
    Boolean(bool),
    Ident(&'a str),
    Underscore,
    // ... delimiters and operators remain unchanged
}
```

### Risks & Trade-offs

| Risk | Mitigation |
| :--- | :--------- |
| Collision between `_` and table names | Disambiguate in tokenizer: bare `_` is `Token::Underscore`, which is rejected where table names are expected (`parse_bracket_access`). |
| Leaking `_` into arithmetic expressions | Enforce that `parse_primary` never handles `Token::Underscore`, ensuring it immediately returns `ParseError::UnexpectedToken`. |

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Boolean literals parsing and roundtrip**
  - **Given** formula expressions `"TRUE"` and `"FALSE"`
  - **When** parsed via `Expr::parse`
  - **Then** produce `Expr::Literal(Literal::Boolean(true))` and `Expr::Literal(Literal::Boolean(false))`, and format back to `"TRUE"` and `"FALSE"`.

- [x] **Scenario 2: Omitted argument in function call**
  - **Given** formula expression `"PMT(0.05, 10, 1000, _, 1)"`
  - **When** parsed via `Expr::parse`
  - **Then** produces `Expr::FunctionCall` where argument 3 is `Expr::Omitted`, and formats back to `"PMT(0.05, 10, 1000, _, 1)"`.

- [x] **Scenario 3: Grammatical confinement of `_`**
  - **Given** invalid formulas `"1 + _"`, `"-_"`, `"_ * 2"`, or `"Table[_, @col]"`
  - **When** parsed via `Expr::parse`
  - **Then** parsing fails with `ParseError::UnexpectedToken`.

- [x] **Scenario 4: Identifier preservation with underscores**
  - **Given** identifiers with leading or embedded underscores (`_rate`, `tax_bracket`, `rev_2024`)
  - **When** parsed in formulas like `Table["tax_bracket", @col]`
  - **Then** identifiers tokenize normally as `Token::Ident`.

- [x] **Scenario 5: Rejection of `null` keyword in formulas**
  - **Given** formula expressions containing `"null"` (e.g. `"IF(A1, B1, null)"` or `"null"`)
  - **When** parsed via `Expr::parse`
  - **Then** parsing fails with `ParseError::UnexpectedToken`.

- [x] **Scenario 6: Dependency reference collection on `Expr::Omitted`**
  - **Given** an expression containing `Expr::Omitted` (e.g. within a function call)
  - **When** references are extracted via `collect_refs_recursive`
  - **Then** zero references are extracted from the omitted argument branch.

---

## Tasks

### Phase 1: AST and Tokenizer Updates
- [x] Add `Literal::Boolean(bool)` to `Literal` in `src/expr/ast.rs`, implement `Display` emitting `"TRUE"` / `"FALSE"`, and add `Expr::boolean(val: bool)` helper if appropriate.
- [x] Add `Expr::Omitted` to `Expr` in `src/expr/ast.rs` and implement `Display` emitting `write!(f, "_")`.
- [x] Purge `Literal::Null` and `Expr::null()` from `src/expr/ast.rs`.
- [x] Add `Token::Boolean(bool)` and `Token::Underscore` to `Token` in `src/expr/parser.rs`, and remove `Token::Null`.
- [x] Update tokenizer in `src/expr/parser.rs`:
  - Tokenize `"TRUE"` as `Token::Boolean(true)`.
  - Tokenize `"FALSE"` as `Token::Boolean(false)`.
  - Tokenize standalone `"\_"` as `Token::Underscore`.
  - Preserve identifiers containing underscores (`_foo`, `tax_rate`).

### Phase 2: Parser and Grammatical Confinement
- [x] Update `parse_primary` in `src/expr/parser.rs` to parse `Token::Boolean` into `Expr::Literal(Literal::Boolean)`.
- [x] Update `parse_argument_list` in `src/expr/parser.rs` to detect `Token::Underscore` and insert `Expr::Omitted`.
- [x] Verify `parse_expression` and `parse_primary` treat `Token::Underscore` as an unexpected token.

### Phase 3: Downstream Compiler Pass and Test Reconciliations
- [x] Add `Expr::Omitted => {}` to `collect_refs_recursive` in `src/compiler/graph/resolve.rs`.
- [x] Update line 59 of `tests/formula_parser_test.rs` to use `_` instead of `null` and assert `Expr::Omitted`.
- [x] Add unit tests in `src/expr/parser.rs` for booleans, `_` in function calls, confinement rejections, and `null` rejection.

### Phase 4: Verification Suite
- [x] Run `cargo check`.
- [x] Run `cargo test`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo fmt --check`.
