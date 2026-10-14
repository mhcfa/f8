---
title: Formula Tokenizer and Parser Refactoring
date: 2026-09-16
status: completed
target-living-spec: specs/living-specs/expr/formula-parser.md
---

## Proposal

### What

Refactor `src/expr/parser.rs` from single-pass string-level scanning into a two-stage pipeline:
1. Lexer: Tokenizes the formula string slice into a stream of strongly typed [`Token`] items with byte offset spans.
2. Pratt Parser: Evaluates operator precedence, function calls, and addressing coordinates by pattern matching over the token stream.

### Why

The initial implementation performed manual string slicing (e.g. `*input = &input[8..]`), ad-hoc boundary checks (`has_ident_boundary`), and repetitive whitespace skipping (`skip_ws`). Introducing a dedicated `Token` enum:
- Eliminates manual byte-slice stepping and potential UTF-8 boundary hazards.
- Decouples lexical analysis from syntactic expression parsing.
- Replaces string comparisons with single-cycle enum discriminant matching in the Pratt parser.
- Retains 100% backward compatibility with existing AST definitions and diagnostic errors.

### Scope

- **In-scope**:
  - `Token` enum representing literals, identifiers, delimiters, and operators.
  - `SpannedToken` struct tracking token byte offsets and spans.
  - `tokenize` lexer function transforming `&str` into `Vec<SpannedToken>` with full error diagnostics.
  - Refactored `TokenStream` cursor and Pratt precedence parser.
  - Zero changes to public APIs (`Expr::parse`), AST nodes (`src/expr/ast.rs`), or diagnostic errors (`src/expr/error.rs`).
- **Out-of-scope**:
  - Modifying grammar rules in `docs/dsl/grammar.md`.
  - Semantic evaluation or spatial coordinate lowering.

### What Changes

- `src/expr/parser.rs`: Complete rewrite to define `Token`, `SpannedToken`, `TokenStream`, lexer combinators, and token-matching Pratt parser.

### Capabilities

- **New capabilities**: Clean lexical error reporting, compiler-checked token pattern matching.
- **Modified capabilities**: Internal parsing implementation only; all public interfaces remain identical.

---

## Design

### Data Contracts

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    String(String),
    Null,
    Ident(String),
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Dot,
    DotDot,
    At,
    Plus,
    Minus,
    Star,
    Slash,
    Caret,
    Eq,
    NotEq,
    Lt,
    Lte,
    Gt,
    Gte,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub offset: usize,
    pub len: usize,
}
```

### Invariants

- **Zero production panics**: Return `Result<Expr, ParseError>` with no unwrap/expect in production code.
- **Exact error parity**: Maintain exact diagnostic offset and error type parity with the existing test suite.
- **Precedence & Associativity**: Preserve exact 5-tier operator precedence and associativity rules.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Full Verification Suite Pass**
  - **Given** the refactored token-based parser
  - **When** running `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`
  - **Then** all 57 tests pass with zero warnings and no formatting issues.

- [x] **Scenario 2: Coordinate Key Validation Parity**
  - **Given** unquoted coordinate keys like `"Table.row[2024E]"` or `"Opex[r_and_d, @]"`
  - **When** parsing with the tokenized parser
  - **Then** returns `ParseError::UnquotedCoordinateKey` with exact token string and byte offset.

- [x] **Scenario 3: Roundtrip Display Parity**
  - **Given** formulas with complex nested operators and addressing
  - **When** parsed, displayed to string, and re-parsed
  - **Then** the resulting AST is structurally identical.

---

## Tasks

### Phase 1: Implementation

- [x] Define `Token`, `SpannedToken`, and `TokenStream` in `src/expr/parser.rs`.
- [x] Implement `tokenize(input: &str) -> Result<Vec<SpannedToken>, ParseError>`.
- [x] Implement Pratt parser functions operating on `TokenStream`.
- [x] Implement function calls, dot access, and bracket coordinate selectors on `TokenStream`.

### Phase 2: Verification & SDD Reconciliation

- [x] Run unit and integration tests.
- [x] Run clippy and format checks.
- [x] Update living spec `specs/living-specs/expr/formula-parser.md`.
- [x] Archive draft spec to `specs/history/`.
