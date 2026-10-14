---
title: Median Statistical Function
date: 2026-09-22
status: completed
target-living-spec: specs/living-specs/functions/registry.md
---

## Proposal

### What

Implement the standard Excel `MEDIAN` function in `src/functions/math/aggregation.rs`, register it in `FunctionRegistry::standard()`, and update test suites and function counts from 43 to 44 functions.

### Why

- **Financial Modeling Prerequisite**: In Comparable Company Analysis ("Trading Comps") and valuation benchmarking, financial analysts rely on `MEDIAN` rather than `AVERAGE` to avoid distortion from high-multiple or low-margin outliers.
- **Excel Function Parity**: `MEDIAN` is one of the foundational statistical functions in spreadsheet modeling.

### Scope

- **In-scope**:
  - `src/functions/common.rs`: Add `collect_values` helper for extracting ordered numeric vectors from range references and scalar arguments.
  - `src/functions/math/aggregation.rs`: Implement `MedianFunction`.
  - `src/functions/math/mod.rs`: Export `MedianFunction` and register in `register()`.
  - `tests/function_math_test.rs`: Add unit tests for `MEDIAN` (odd count, even count, ranges with empty/text cells, direct scalars, error bubbling, empty set `#NUM!`).
  - Update function count assertions across tests from 43 to 44.
  - `specs/living-specs/functions/registry.md`: Reconcile registered function list and count.
- **Out-of-scope**:
  - Advanced statistical distributions (e.g. `PERCENTILE.INC`, `QUARTILE.INC`, `STDEV.S`), which will be handled in separate specs.

### What Changes

- **Modified files**:
  - `src/functions/common.rs`
  - `src/functions/math/aggregation.rs`
  - `src/functions/math/mod.rs`
  - `tests/function_math_test.rs`
  - `tests/function_registry_test.rs`
  - `tests/function_financial_test.rs`
  - `tests/function_date_test.rs`
  - `specs/living-specs/functions/registry.md`

### Capabilities

- **New capabilities**:
  - Formulas can invoke `MEDIAN(range)` or `MEDIAN(val1, val2, ...)` across odd and even collections of numbers.
  - When invoked on ranges with text or empty cells, non-numeric values are safely filtered out.
  - When no numeric observations are present, returns `#NUM!`.

---

## Design

### Goals

1. **Exact Excel parity**: Match Excel's `MEDIAN` behavior on ranges (filter non-numbers) vs scalar arguments (coerce booleans, error on text).
2. **Safe float sorting**: Use `f64::total_cmp` to avoid floating-point NaN panics.
3. **Zero production panics**: Cleanly bubble Excel errors (`#NUM!`, `#VALUE!`, cell errors).

### Invariants

- Empty range inputs return `ExcelError::Num` (unlike `AVERAGE` which returns `ExcelError::Div0`).
- Range references silently ignore text, booleans, and empty cells.
- Direct scalar text produces `ExcelError::Value`.
- Odd $N$ returns element at index $N / 2$.
- Even $N$ returns `(sorted[N / 2 - 1] + sorted[N / 2]) / 2.0`.
- Return value zero is normalized to `0.0`.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Odd and Even Number Sets**
  - **Given** odd set `[10.0, 20.0, 30.0]` and even set `[10.0, 20.0, 30.0, 40.0]`
  - **When** evaluating `MEDIAN`
  - **Then** odd set returns `20.0` and even set returns `25.0`.

- [x] **Scenario 2: Range Reference Filtering**
  - **Given** a range containing `[10.0, "text", true, 50.0, empty, 30.0]`
  - **When** evaluating `MEDIAN`
  - **Then** non-numeric cells are ignored and median of `[10.0, 30.0, 50.0]` returns `30.0`.

- [x] **Scenario 3: Empty Range Error**
  - **Given** a range containing zero numbers (only text and empty cells)
  - **When** evaluating `MEDIAN`
  - **Then** returns `ExcelError::Num`.

- [x] **Scenario 4: Scalar Coercion and Errors**
  - **Given** direct scalars `MEDIAN(10, TRUE, FALSE)`
  - **When** evaluating `MEDIAN`
  - **Then** evaluates median of `[0.0, 1.0, 10.0]` which is `1.0`. Direct string scalar produces `#VALUE!`.

---

## Tasks

- [x] Add `collect_values` helper in `src/functions/common.rs`.
- [x] Implement `MedianFunction` in `src/functions/math/aggregation.rs`.
- [x] Register `MedianFunction` in `src/functions/math/mod.rs`.
- [x] Add unit tests in `tests/function_math_test.rs`.
- [x] Update function count assertions (43 -> 44) across test files.
- [x] Update Excel calculation parity fixture (`tests/fixtures/parity_suite.rs`) and parity test assertions.
- [x] Verify test suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- [x] Reconcile `specs/living-specs/functions/registry.md`, `docs/functions/catalog.md`, and move draft spec to `specs/history/`.
