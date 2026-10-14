---
title: Conditional Aggregation Functions Suite (*IFS)
date: 2026-09-19
status: completed
target-living-spec: specs/living-specs/functions/registry.md
---

## Proposal

### What

Implement the five standard Excel conditional aggregation functions specified in `docs/functions/catalog.md`:
- `SUMIFS`: Conditional arithmetic sum across multi-criteria slices
- `COUNTIFS`: Multi-criteria cell count
- `AVERAGEIFS`: Conditional arithmetic mean across multi-criteria slices
- `MINIFS`: Conditional minimum numeric value across multi-criteria slices
- `MAXIFS`: Conditional maximum numeric value across multi-criteria slices

In addition, implement a dedicated criteria compilation and evaluation engine (`src/functions/math/criteria.rs`) supporting comparison prefixes (`=`, `<>`, `<`, `<=`, `>`, `>=`), numeric/boolean coercions, wildcard matching (`*`, `?`, `~`), and blank/non-blank sentinels (`""`, `"="`, `"<>"`).

### Why

1. **Essential financial modeling primitive**: Conditional range aggregation is foundational for financial models (e.g. departmental cost rollups, revenue by business unit or geography, cohort retention, and filtered transaction sums).
2. **Standardization on modern plural signatures**: Eliminates legacy inverted signatures (`SUMIF`, `COUNTIF`, `AVERAGEIF`) in favor of consistent target-first signatures with paired trailing criteria.
3. **Dedicated criteria compilation**: Interpreting criteria expressions directly inside inner loops would incur redundant string parsing over large grids. Compiling a `CellValue` criterion once into a strongly typed `Criterion` prior to cell traversal guarantees $O(N \cdot K)$ performance with minimal allocation.
4. **Dimension conformity enforcement**: Excel requires all criteria ranges to exactly match the dimensions of the primary aggregation range. Enforcing this prevents silent spatial mismatches.

### Scope

- **In-scope**:
  - `src/functions/math/criteria.rs`: Criteria compiler, comparison operators (`ComparisonOp`), target matcher (`CriteriaTarget`), wildcard pattern engine (`WildcardPattern`), and single-pass cell matching (`Criterion::matches`).
  - `src/functions/math/conditional.rs`: Implement `SumifsFunction` (`SUMIFS`), `CountifsFunction` (`COUNTIFS`), `AverageifsFunction` (`AVERAGEIFS`), `MinifsFunction` (`MINIFS`), and `MaxifsFunction` (`MAXIFS`).
  - `src/functions/math/mod.rs`: Export conditional functions, criteria types, and register all five functions in `register(r: &mut FunctionRegistry)`.
  - Comprehensive unit and integration test suite in `tests/function_conditional_test.rs`.
  - Update function count assertions in `tests/function_registry_test.rs` and `tests/function_math_test.rs` (from 27 to 32 standard functions).
- **Out-of-scope**:
  - Legacy singular functions (`SUMIF`, `COUNTIF`, `AVERAGEIF`).
  - Dynamic array formulas or vector spilling.
  - Date & time functions (`YEARFRAC`, `DATE`, `EDATE`, etc.).
  - Lookup functions (`XLOOKUP`, `INDEX`).

### What Changes

- `src/functions/math/criteria.rs`: New module containing criteria parser and pattern evaluation engine.
- `src/functions/math/conditional.rs`: New module implementing the five `*IFS` functions.
- `src/functions/math/mod.rs`: Register new functions and expose module public items.
- `tests/function_conditional_test.rs`: New integration tests covering exact matching, operators, wildcards, blanks, error bubbling, and multi-criteria intersections.
- `tests/function_registry_test.rs` & `tests/function_math_test.rs`: Adjust expected registry length to 32.

### Capabilities

- **New capabilities**:
  - `SUMIFS`: Sums numbers in target range where all criteria match; ignores text/empty in sum range; bubbles cell errors in ranges; returns `0.0` if no match.
  - `COUNTIFS`: Counts positions where all criteria match across all criteria ranges; returns numeric count.
  - `AVERAGEIFS`: Computes arithmetic mean of numbers in target range where all criteria match; ignores text/empty in average range; returns `#DIV/0!` if no cells match or no numeric values match.
  - `MINIFS`: Computes minimum numeric value in target range where all criteria match; ignores text/empty in min range; returns `0.0` if no cells match.
  - `MAXIFS`: Computes maximum numeric value in target range where all criteria match; ignores text/empty in max range; returns `0.0` if no cells match.
  - Criteria engine: Supports `<`, `<=`, `>`, `>=`, `=`, `<>`, wildcards `*` and `?` with `~` escape, empty cell matches (`""`, `"="`), and non-blank matches (`"<>"`).
- **Modified capabilities**:
  - Standard registry now contains 32 built-in functions.

---

## Design

### Goals

1. **Excel parity**: Full compliance with Excel criteria rules specified in `docs/compiler/parity.md` Section 5.
2. **Compile-once efficiency**: Parse criteria string once per call, avoiding string manipulation in the cell traversal loop.
3. **Strict dimensional conformity**: Enforce that all criteria ranges share identical `(rows, cols)` dimensions with the primary range, returning `#VALUE!` on violation.
4. **Rust 2024 compliance**: Clean error propagation, no unwrap in production code, immutable bindings.

### Invariants

- **Dimensional conformity**: For any `*IFS` invocation, every `criteria_range_k` must have identical row count and column count to the primary range (or `criteria_range1` in `COUNTIFS`). Mismatches immediately return `ExcelError::Value`.
- **Criteria range reference requirement**: Every criteria range argument must be a range reference (`FunctionArg::Array`), not a scalar value. Passing a scalar as a criteria range returns `ExcelError::Value`.
- **Target range reference requirement**: The primary target range (`sum_range`, `average_range`, `min_range`, `max_range`) must be a range reference (`FunctionArg::Array`).
- **Error propagation in target range**: Calculation domain errors present in the target range for matching cells bubble up immediately.
- **Criteria error matching**: If a criterion is an explicit error (e.g. `#DIV/0!`), it matches cells containing that error. Unmatched errors in criteria ranges do not match and do not abort evaluation unless evaluated as target cells.
- **Empty result defaults**: If no cells satisfy criteria:
  - `SUMIFS`, `MINIFS`, `MAXIFS` evaluate to `0.0`.
  - `COUNTIFS` evaluates to `0.0`.
  - `AVERAGEIFS` evaluates to `ExcelError::Div0`.

### Decisions

- **Decision 1: Modular criteria engine in `src/functions/math/criteria.rs`**:
  - **Choice**: Implement `Criterion` with dedicated `ComparisonOp`, `CriteriaTarget`, and `WildcardPattern`.
  - **Rationale**: Isolates string parsing and pattern matching from aggregation arithmetic; enables future reuse by `XLOOKUP` match mode 2.
  - **Alternatives considered**: Inline string parsing within `SUMIFS` loop. Rejected due to poor performance and code duplication across 5 functions.
- **Decision 2: Categorization under `FunctionCategory::Math`**:
  - **Choice**: Assign `*IFS` functions to `FunctionCategory::Math`.
  - **Rationale**: Matches Excel's standard classification of `SUMIFS`, `COUNTIFS`, `AVERAGEIFS` (Statistical/Math) and existing `FunctionCategory` taxonomy.
- **Decision 3: Dimension checking before criteria parsing**:
  - **Choice**: Validate shapes of all `RangeOrArray` arguments before compiling criteria.
  - **Rationale**: Fails fast on `#VALUE!` before performing any matching or aggregation work.

### Data Contracts

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    Eq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
}

#[derive(Debug, Clone)]
pub enum CriteriaTarget {
    Number(f64),
    Boolean(bool),
    Text(String),
    Wildcard(WildcardPattern),
    Empty,
    NonEmpty,
    Error(ExcelError),
}

#[derive(Debug, Clone)]
pub struct Criterion {
    pub op: ComparisonOp,
    pub target: CriteriaTarget,
}

#[derive(Debug, Clone)]
pub struct WildcardPattern {
    // Compiled pattern segments for *, ?, and literal ASCII characters
    pub segments: Vec<PatternSegment>,
}
```

### Risks & Trade-offs

- **Wildcard matching complexity**: Unescaped `*` can match arbitrary substrings, risking exponential backtracking on malformed input.
  - *Mitigation*: Implement standard linear / greedy wildcard matcher without external regex dependency.
- **String vs. numeric criteria ambiguity**: In Excel, criteria `">=10"` treats `10` as a number. Criteria `">=foo"` treats `"foo"` as a text comparison.
  - *Mitigation*: Parse operand after operator: if `f64::from_str` succeeds, target is numeric; otherwise target is text.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: `SUMIFS` multi-criteria evaluation with numeric and text criteria**
  - **Given** a sum range `[10, 20, 30, 40]`, criteria range 1 `["A", "B", "A", "B"]` with criterion `"A"`, and criteria range 2 `[1, 2, 1, 2]` with criterion `">0"`
  - **When** evaluated with `SUMIFS`
  - **Then** sums only matching entries `10 + 30 = 40.0`.
- [x] **Scenario 2: `COUNTIFS` multi-criteria counting**
  - **Given** criteria range 1 `[10, 20, 30, 40]` with criterion `">=20"`, and criteria range 2 `["East", "West", "East", "West"]` with criterion `"*st"`
  - **When** evaluated with `COUNTIFS`
  - **Then** counts matching positions 2, 3, 4 $\rightarrow$ `3.0`.
- [x] **Scenario 3: `AVERAGEIFS` empty match `#DIV/0!` fallback**
  - **Given** average range `[10, 20, 30]` and criteria range `["X", "Y", "Z"]` with criterion `"W"`
  - **When** evaluated with `AVERAGEIFS`
  - **Then** evaluates to `ExcelError::Div0`.
- [x] **Scenario 4: `MINIFS` and `MAXIFS` zero fallback on no match**
  - **Given** ranges with no matching criteria
  - **When** evaluated with `MINIFS` and `MAXIFS`
  - **Then** both evaluate to `0.0`.
- [x] **Scenario 5: Wildcard matching with escape sequences**
  - **Given** criteria range `["abc", "a*c", "axc", "a?c"]`
  - **When** matching criterion `"a~*c"`
  - **Then** matches only literal `"a*c"`.
- [x] **Scenario 6: Blank and non-blank criteria sentinels**
  - **Given** range containing `[Empty, "Text", 10.0, ""]`
  - **When** evaluated with criterion `""` (or `"="`) and `"<>"`
  - **Then** `""` matches `Empty` and `""`, while `"<>"` matches `"Text"` and `10.0`.
- [x] **Scenario 7: Dimension conformity validation**
  - **Given** target range of shape `(1, 4)` and criteria range of shape `(1, 3)`
  - **When** evaluated with any `*IFS` function
  - **Then** immediately returns `ExcelError::Value`.

---

## Tasks

- [x] 1. Implement criteria compiler and wildcard matcher in `src/functions/math/criteria.rs`.
- [x] 2. Implement `SUMIFS`, `COUNTIFS`, `AVERAGEIFS`, `MINIFS`, `MAXIFS` in `src/functions/math/conditional.rs`.
- [x] 3. Export submodules and register all five functions in `src/functions/math/mod.rs`.
- [x] 4. Add comprehensive integration test suite in `tests/function_conditional_test.rs`.
- [x] 5. Update standard function count assertions across test suites to 32.
- [x] 6. Run verification suite (`cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`).
