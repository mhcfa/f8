---
title: Purge Winnow, Compatibility Shims, and Dead Code in Formula Parser
date: 2026-09-16
status: completed
target-living-spec: specs/living-specs/expr/formula-parser.md
---

## Proposal

### What

Purge the vestigial `winnow` dependency, dead code paths, unnecessary heap cloning, and compatibility shims from the formula expression parser subsystem (`finx::expr`):
1. Remove `winnow` dependency from `Cargo.toml` and replace `multispace0` with standard Rust `trim_start()`.
2. Optimize lexical tokenization with zero-allocation borrowed slices (`Token<'a>::Ident(&'a str)`).
3. Replace the cloning `Iterator` on `TokenStream` with an allocation-free cursor (`advance`, `consume`, `peek`).
4. Remove dead code: redundant upfront `trim().is_empty()` checks, unused `Expr::string`, dead test match arms, and unused token cloning in `expect`.
5. Scope internal parser symbols to `pub(crate)` and remove unintended public re-exports in `finx::expr`.

### Why

Finx is a greenfield project governed by the development principles in `AGENTS.md`:
- **Pragmatic simplicity**: Favor direct, idiomatic Rust code; avoid premature abstractions and unnecessary external dependencies.
- **Allocations and efficiency**: Prefer borrowed references (`&str`, `&[T]`) over unnecessary cloning in compiler passes.
- **Zero compatibility shims**: Eliminate vestigial dependencies and obsolete test branches left over from the initial string-scanning prototype.

### Scope

- **In-scope**:
  - Remove `winnow` from `Cargo.toml` and `src/expr/parser.rs`.
  - Refactor `Token<'a>` to borrow `&'a str` for identifiers and coordinate words.
  - Refactor `TokenStream` cursor to navigate without cloning.
  - Remove redundant empty string check in `parse_expr_str`.
  - Remove dead test match arm in `test_keyword_boundary_validation`.
  - Remove unused helper `Expr::string` in `src/expr/ast.rs`.
  - Restrict internal parser types and functions to `pub(crate)`.
  - Update `docs/architecture.md`, `docs/dsl/grammar.md`, and `specs/living-specs/expr/formula-parser.md`.
- **Out-of-scope**:
  - Modifying the formal grammar rules or AST node variants.
  - Changing operator precedence or associativity rules.

### What Changes

- `Cargo.toml`: Remove `winnow = "1.0.4"`.
- `src/expr/mod.rs`: Remove `pub use parser::{SpannedToken, Token, tokenize};`.
- `src/expr/ast.rs`: Remove unused `Expr::string` constructor.
- `src/expr/parser.rs`:
  - Hand-roll zero-dependency whitespace skipping with `trim_start()`.
  - Refactor `Token<'a>` and `SpannedToken<'a>` to borrow string slices.
  - Replace `Iterator` implementation on `TokenStream` with non-cloning cursor methods (`advance`, `consume`, `peek`, `expect`).
  - Drop `orig: &'a str` in favor of `source_len: usize`.
  - Remove redundant `trim().is_empty()` check in `parse_expr_str`.
  - Clean up dead match arm in `test_keyword_boundary_validation`.
- `docs/architecture.md`, `docs/dsl/grammar.md`, `specs/living-specs/expr/formula-parser.md`: Remove references to `winnow`.

### Capabilities

- **New capabilities**: Zero external parsing dependencies, zero-allocation token scanning for identifiers and operators.
- **Modified capabilities**: Purely internal parser optimization and hygiene; `Expr::parse` behavior and error diagnostics remain identical.

---

## Design

### Goals

1. Completely eliminate `winnow` from the dependency graph.
2. Eliminate all heap allocations during token scanning except when string unescaping requires owned storage.
3. Eliminate all token cloning during Pratt parsing cursor traversal.
4. Maintain 100% test parity across all unit and integration tests.

### Invariants

- **Exact diagnostic parity**: Maintain character-exact error offsets and error variants for all syntax errors.
- **Zero production panics**: No `unwrap()` or `expect()` in production code.
- **Strict AST compatibility**: AST definitions, operator precedence, and canonical string round-tripping remain unaffected.

### Decisions

- **Borrowing identifiers in `Token<'a>`**: Identifiers never need unescaping, so they can directly reference slices of the input `&'a str`.
- **Cursor without `Iterator`**: A cursor with `advance()`, `peek()`, and `consume()` eliminates the need for `Iterator::next` to clone items.
- **Drop `winnow`**: Standard Rust string slicing and char iterators are cleaner, faster, and require zero dependencies for this grammar.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Zero External Parser Dependencies**
  - **Given** `Cargo.toml` without `winnow`
  - **When** building with `cargo check` and `cargo test`
  - **Then** the crate compiles and passes with zero `winnow` references.

- [x] **Scenario 2: Zero-Clone Token Stream**
  - **Given** a formula expression
  - **When** tokenizing and Pratt parsing
  - **Then** tokens are borrowed from the input string and traversed without cloning.

- [x] **Scenario 3: Verification Suite Conformance**
  - **Given** the clean zero-dependency parser
  - **When** running `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`
  - **Then** all 57+ tests pass with zero warnings and clean formatting.

---

## Tasks

### Phase 1: Dependency & Model Cleanup
- [x] Remove `winnow` from `Cargo.toml`.
- [x] Remove unused `Expr::string` in `src/expr/ast.rs`.
- [x] Remove public re-exports of `SpannedToken`, `Token`, `tokenize` from `src/expr/mod.rs`.

### Phase 2: Lexer & Cursor Refactoring
- [x] Refactor `Token<'a>` to borrow `&'a str` for `Ident`.
- [x] Replace `multispace0` with `cursor = cursor.trim_start()`.
- [x] Refactor `TokenStream` to use `tokens: Vec<SpannedToken<'a>>` and `source_len: usize`.
- [x] Replace `impl Iterator` with `advance()`, `consume()`, and zero-clone `expect()`.
- [x] Remove redundant `trim().is_empty()` check in `parse_expr_str`.
- [x] Clean up dead arm in `test_keyword_boundary_validation`.

### Phase 3: Verification & Documentation Reconciliation
- [x] Run `cargo check`, `cargo test`, `cargo clippy`, and `cargo fmt`.
- [x] Reconcile `specs/living-specs/expr/formula-parser.md`, `docs/architecture.md`, and `docs/dsl/grammar.md`.
- [x] Archive draft spec to `specs/history/`.

