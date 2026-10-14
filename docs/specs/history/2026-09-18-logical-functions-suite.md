---
title: Logical Functions Suite
date: 2026-09-18
status: completed
target-living-spec: specs/living-specs/functions/registry.md
---

## Proposal

### What

Implement the remaining eight standard Excel logical functions specified in `docs/functions/catalog.md`:
- Boolean operators: `AND`, `OR`, `NOT`
- Error handling guards: `IFERROR`, `IFNA`
- Multi-branch conditional and selection: `IFS`, `CHOOSE`, `SWITCH`

In addition, centralize Excel equality and comparison logic (`excel_cmp`, `excel_eq`) on `CellValue`, introduce `LogicalCollector` in `src/functions/common.rs` for range and scalar truth value collection, and generalize `bind_arguments` to support cyclical parameter matching for `VariadicRule::Pairs`.

### Why

1. **Complete logical modeling capabilities**: Financial and operating models rely heavily on scenario selection (`CHOOSE`, `SWITCH`), multi-tiered boundary logic (`IFS`), error fallbacks (`IFERROR`, `IFNA`), and compound criteria (`AND`, `OR`, `NOT`).
2. **Short-circuiting execution guarantees**: Conditional branches must not evaluate dormant expressions that could cause division by zero, reference errors, or expensive cycles.
3. **Single source of truth for value comparisons**: Case-insensitive string equality and Excel type-ranking rules must be centralized on `CellValue` rather than duplicated across operator evaluation and function implementations like `SWITCH`.
4. **Variadic pairs binding contract**: Functions with paired arguments (e.g. `IFS`, and future `SUMIFS`/`COUNTIFS`) require `bind_arguments` to correctly alternate between parameter definitions for all trailing argument pairs.

### Scope

- **In-scope**:
  - `src/value/mod.rs`: Add `excel_cmp` and `excel_eq` to `CellValue` for ASCII case-insensitive text and type-ranked comparisons; refactor `src/compiler/eval/expr.rs` to reuse them.
  - `src/functions/common.rs`: Implement `LogicalCollector` for traversing scalars and ranges conforming to Excel truth coercion, text/blank ignoring in ranges, and error bubbling.
  - `src/functions/registry.rs`: Update `bind_arguments` to cycle through the repeating parameter pair when `VariadicRule::Pairs` is active.
  - `src/functions/logical/boolean.rs`: Implement `AndFunction` (`AND`), `OrFunction` (`OR`), and `NotFunction` (`NOT`).
  - `src/functions/logical/error_handling.rs`: Implement `IferrorFunction` (`IFERROR`) and `IfnaFunction` (`IFNA`) with lazy short-circuiting fallback.
  - `src/functions/logical/branching.rs`: Keep `IfFunction` (`IF`); implement `IfsFunction` (`IFS`), `ChooseFunction` (`CHOOSE`), and `SwitchFunction` (`SWITCH`) with lazy branch evaluation.
  - `src/functions/logical/mod.rs`: Organize submodules (`boolean`, `branching`, `error_handling`), export function structs, and register all 9 logical functions in `register(r: &mut FunctionRegistry)`.
  - Comprehensive unit and integration tests in `tests/function_logical_test.rs`.
  - Update standard registry count assertion in `tests/function_registry_test.rs` and `tests/function_math_test.rs` (from 12 to 20 functions).
- **Out-of-scope**:
  - Financial functions (`NPV`, `XNPV`, `IRR`, `XIRR`, `PMT`, `IPMT`, `PPMT`).
  - Lookup functions (`XLOOKUP`, `INDEX`).
  - Date & time functions (`YEARFRAC`, `DATE`, etc.).
  - Array formula spilling (Finx computes strictly scalar cell values).

### What Changes

- `src/value/mod.rs`: Introduce `CellValue::excel_cmp` and `CellValue::excel_eq`.
- `src/compiler/eval/expr.rs`: Delegate comparison operators to `CellValue::excel_cmp`.
- `src/functions/common.rs`: Introduce `LogicalCollector` tracking truth states and errors across scalars and ranges.
- `src/functions/registry.rs`: Fix parameter resolution in `bind_arguments` for `VariadicRule::Pairs`.
- `src/functions/logical/mod.rs`: Submodule declarations and updated `register` function.
- `src/functions/logical/boolean.rs`: Implement `AND`, `OR`, `NOT`.
- `src/functions/logical/error_handling.rs`: Implement `IFERROR`, `IFNA`.
- `src/functions/logical/branching.rs`: Implement `IFS`, `CHOOSE`, `SWITCH`.
- `tests/function_logical_test.rs`: New integration test suite covering all 8 new logical functions.
- `tests/function_registry_test.rs` & `tests/function_math_test.rs`: Adjust expected registry length to 20.

### Capabilities

- **New capabilities**:
  - `AND`: Evaluates logical conjunction across scalars and ranges; ignores range text and blank cells; bubbles errors; returns `#VALUE!` if no logical values exist.
  - `OR`: Evaluates logical disjunction across scalars and ranges; ignores range text and blank cells; bubbles errors; returns `#VALUE!` if no logical values exist.
  - `NOT`: Inverts logical scalar value; coerces numbers and booleans; text returns `#VALUE!`; errors bubble.
  - `IFERROR`: Evaluates value lazily; returns evaluated value if not an error; otherwise evaluates and returns fallback.
  - `IFNA`: Evaluates value lazily; returns fallback only if value evaluates to `#N/A`; otherwise returns value or bubbles other errors without evaluating fallback.
  - `IFS`: Evaluates condition-value pairs sequentially; short-circuits on first true condition; returns `#N/A` if no condition is met; validates pair count.
  - `CHOOSE`: Evaluates 1-based index; selects and evaluates only the target choice expression; returns `#VALUE!` if index is out of bounds or non-numeric.
  - `SWITCH`: Evaluates target expression; compares sequentially against case values; evaluates and returns matching result; falls back to optional default or returns `#N/A`.
- **Modified capabilities**:
  - `FunctionRegistry::standard()` now contains 20 built-in functions in deterministic registration order.

## Design

### Goals

1. **Strict Excel parity**: Match official Excel calculation rules for truth coercion, range filtering, error bubbling, and short-circuiting.
2. **Zero dormant evaluations**: Ensure dormant branches in `IFERROR`, `IFNA`, `IFS`, `CHOOSE`, and `SWITCH` are never evaluated.
3. **No code duplication**: Shared logic for comparisons lives in `CellValue`, while shared logic for logical range aggregation lives in `LogicalCollector`.
4. **Rust 2024 compliance**: Clean error propagation with `thiserror`, zero unwraps in production code, immutable bindings by default.

### Invariants

- **Lazy evaluation safety**: Dormant branches must never be evaluated. If an unselected branch would cause `#DIV/0!`, the function evaluation must succeed without error.
- **Immediate error propagation in eager functions**: In `AND`, `OR`, and `NOT`, any encountered error must immediately propagate.
- **Empty range guard**: Calling `AND` or `OR` over ranges with zero logical values (e.g. all empty or text) returns `ExcelError::Value`.

### Decisions

- **Decision 1: Lazy vs. Eager parameters**:
  - `AND`, `OR`: Parameters are `ParamKind::RangeOrArray` (eagerly evaluated scalars or ranges).
  - `NOT`: Parameter is `ParamKind::Scalar` (eagerly evaluated scalar).
  - `IFERROR`, `IFNA`: Both parameters are `ParamKind::Lazy` so unneeded evaluation is prevented.
  - `IFS`: Both condition and result parameters are `ParamKind::Lazy` with `VariadicRule::Pairs` so conditions and results after the first true match are never evaluated.
  - `CHOOSE`: `index_num` is `ParamKind::Scalar`, choice parameters are `ParamKind::Lazy` with `VariadicRule::Single`.
  - `SWITCH`: `expression` is `ParamKind::Scalar`, all trailing arguments (cases, results, default) are `ParamKind::Lazy` with `VariadicRule::Single`.
- **Decision 2: Centralizing Excel comparison on `CellValue`**:
  - Move ASCII case-insensitive and type-ranked comparisons into `CellValue::excel_cmp(&self, other: &Self) -> Ordering` and `CellValue::excel_eq(&self, other: &Self) -> bool`.
  - Used by formula binary operators (`=`, `<>`, `<`, `>`, `<=`, `>=`) and `SWITCH`.
- **Decision 3: `VariadicRule::Pairs` indexing in `bind_arguments`**:
  - When `i >= func.parameters().len()` and `rule == VariadicRule::Pairs`, the parameter definition is resolved by cycling between the last two parameter definitions: `(i - fixed_count) % 2`.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: `AND` and `OR` range aggregation and scalar coercion**
  - **Given** ranges containing booleans, numbers, empty cells, and text
  - **When** evaluated with `AND` and `OR`
  - **Then** empty cells and text are ignored, `0.0` is false, non-zero is true; if all are true `AND` returns `TRUE`; if any is true `OR` returns `TRUE`; if no logical values exist returns `#VALUE!`.
- [x] **Scenario 2: `NOT` logical inversion and error bubbling**
  - **Given** numeric, boolean, or error inputs
  - **When** evaluated with `NOT`
  - **Then** inverts boolean and numeric truth; non-convertible text returns `#VALUE!`; errors bubble immediately.
- [x] **Scenario 3: `IFERROR` short-circuiting**
  - **Given** an expression evaluating to a valid value or an error
  - **When** evaluated with `IFERROR(val, fallback)`
  - **Then** returns `val` without evaluating `fallback` when `val` is valid; returns `fallback` when `val` is any `ExcelError`.
- [x] **Scenario 4: `IFNA` specific `#N/A` catching**
  - **Given** expressions evaluating to `#N/A`, `#DIV/0!`, or valid values
  - **When** evaluated with `IFNA(val, fallback)`
  - **Then** returns `fallback` only on `#N/A`; returns `#DIV/0!` without evaluating `fallback` on other errors; returns valid `val` directly.
- [x] **Scenario 5: `IFS` multi-condition short-circuiting and `#N/A` fallback**
  - **Given** condition-value pairs where an early condition is true and a later condition contains a division by zero
  - **When** evaluated with `IFS`
  - **Then** returns the early true value without evaluating subsequent conditions or values; if all conditions are false returns `#N/A`; if pair arity is odd rejects validation.
- [x] **Scenario 6: `CHOOSE` index selection and short-circuiting**
  - **Given** a 1-based index and multiple candidate expressions
  - **When** evaluated with `CHOOSE`
  - **Then** evaluates and returns only the indexed candidate; returns `#VALUE!` if index is `< 1` or exceeds the candidate count.
- [x] **Scenario 7: `SWITCH` sequential case matching and default fallback**
  - **Given** a target expression, case-result pairs, and an optional default
  - **When** evaluated with `SWITCH`
  - **Then** matches case values case-insensitively for strings; returns matching result; falls back to default or `#N/A`; never evaluates dormant branches.

## Tasks

- [x] 1. Centralize Excel comparison on `CellValue` and refactor `src/compiler/eval/expr.rs`.
- [x] 2. Implement `LogicalCollector` in `src/functions/common.rs`.
- [x] 3. Update `bind_arguments` in `src/functions/registry.rs` to cycle parameter pairs for `VariadicRule::Pairs`.
- [x] 4. Implement `AND`, `OR`, and `NOT` in `src/functions/logical/boolean.rs`.
- [x] 5. Implement `IFERROR` and `IFNA` in `src/functions/logical/error_handling.rs`.
- [x] 6. Implement `IFS`, `CHOOSE`, and `SWITCH` in `src/functions/logical/branching.rs`.
- [x] 7. Update `src/functions/logical/mod.rs` with submodule exports and registration.
- [x] 8. Update registry length assertions in existing tests and create comprehensive test suite `tests/function_logical_test.rs`.
- [x] 9. Verify with `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`.
