---
title: Function Architecture Modularization & Math Aggregations
date: 2026-09-18
status: completed
target-living-spec: specs/living-specs/functions/registry.md
---

## Proposal

### What

Modularize the function subsystem into category-based submodules (`math/`, `logical/`) with decentralized registration, extract shared range aggregation and numeric collection utilities in `src/functions/common.rs`, and implement the remaining mathematical and aggregation functions specified in `docs/functions/catalog.md` (`AVERAGE`, `COUNT`, `COUNTA`, `MAX`, `MIN`, `ROUND`, `ROUNDUP`, `ROUNDDOWN`, and `SUMPRODUCT`).

### Why

1. **Scalability and maintainability**: Centralizing all functions in monolithic files (`math.rs`, `logical.rs`) causes code bloat, high merge contention, and poor separation of concerns as the catalog expands to 30+ functions.
2. **Single source of truth for range traversal**: Excel functions follow well-defined traversal archetypes (arithmetic aggregation with error bubbling in `SUM`/`AVERAGE`/`MIN`/`MAX`, numeric tallying without error bubbling in `COUNT`, and non-empty counting in `COUNTA`). Consolidating these traversals into dedicated, reusable primitives in `src/functions/common.rs` eliminates code duplication while preserving clean separation of concerns.
3. **Completing mathematical modeling capabilities**: Real-world financial models require statistical summary metrics (`AVERAGE`, `MIN`, `MAX`), cell counting (`COUNT`, `COUNTA`), exact debt/share rounding (`ROUND`, `ROUNDUP`, `ROUNDDOWN`), and weighted multi-range calculations (`SUMPRODUCT`).

### Scope

- **In-scope**:
  - Restructure `src/functions/` into category subdirectories:
    - `src/functions/common.rs`: Shared range visitation, `NumericCollector`, `count_numbers`, `count_non_empty`, and rounding helpers.
    - `src/functions/math/mod.rs`: Category exports and `register(r: &mut FunctionRegistry)`.
    - `src/functions/math/basic.rs`: `AbsFunction`.
    - `src/functions/math/aggregation.rs`: `SumFunction`, `AverageFunction`, `CountFunction`, `CountaFunction`, `MaxFunction`, `MinFunction`.
    - `src/functions/math/rounding.rs`: `RoundFunction`, `RoundupFunction`, `RounddownFunction`.
    - `src/functions/math/product.rs`: `SumproductFunction`.
    - `src/functions/logical/mod.rs`: Category exports and `register(r: &mut FunctionRegistry)`.
    - `src/functions/logical/branching.rs`: `IfFunction`.
  - Update `FunctionRegistry::standard()` to register all 11 math functions alongside `IF` via category `register` functions (12 standard functions total).
  - Remove top-level function struct re-exports from `finx::functions`; export function structs exclusively through category modules (`finx::functions::math`, `finx::functions::logical`).
  - Unit and integration tests in `tests/function_math_test.rs` and `tests/function_registry_test.rs`.
- **Out-of-scope**:
  - Phase 2 logical functions (`AND`, `OR`, `NOT`, `IFS`, `IFERROR`, `IFNA`).
  - Lookup and financial functions (`XLOOKUP`, `INDEX`, `NPV`, `XIRR`, `PMT`).
  - Dynamic spilling or multi-cell formula output (Finx strictly enforces scalar cell evaluation).

### What Changes

- `src/functions/common.rs`: Add `NumericCollector`, `count_numbers`, `count_non_empty`, and rounding helpers.
- `src/functions/math/mod.rs`: Submodule declarations, re-exports, and `pub fn register(r: &mut FunctionRegistry)`.
- `src/functions/math/basic.rs`: Relocate `AbsFunction`.
- `src/functions/math/aggregation.rs`: Relocate `SumFunction`; implement `AverageFunction`, `CountFunction`, `CountaFunction`, `MaxFunction`, `MinFunction`.
- `src/functions/math/rounding.rs`: Implement `RoundFunction`, `RoundupFunction`, `RounddownFunction`.
- `src/functions/math/product.rs`: Implement `SumproductFunction`.
- `src/functions/logical/mod.rs`: Submodule declarations, re-exports, and `pub fn register(r: &mut FunctionRegistry)`.
- `src/functions/logical/branching.rs`: Relocate `IfFunction`.
- `src/functions/registry.rs`: Update `FunctionRegistry::standard` to call `math::register` and `logical::register`.
- `src/functions/mod.rs`: Export `common`, `math`, `logical`, and root traits/registry contracts without concrete function structs.
- `tests/function_math_test.rs`: Comprehensive test suite for all new mathematical and aggregation functions.
- `tests/function_registry_test.rs`: Update standard registry cardinality assertion to 12.

### Capabilities

- **New capabilities**:
  - `AVERAGE`: Computes arithmetic mean across numbers and ranges, ignoring blanks and text/booleans in ranges; returns `#DIV/0!` if no numbers are present.
  - `COUNT`: Computes count of numeric cells across ranges and arguments; ignores text, booleans, empty cells, and errors in ranges; counts numbers and direct booleans as 1; ignores text, empty cells, and errors in direct scalar arguments (`COUNT("hello")` returns `0.0`); never bubbles errors.
  - `COUNTA`: Computes count of non-empty cells across ranges and arguments (numbers, text, booleans, errors); ignores only `CellValue::Empty`.
  - `MAX` / `MIN`: Computes maximum / minimum numeric value across ranges and arguments; returns `0.0` if no numbers are present.
  - `ROUND` / `ROUNDUP` / `ROUNDDOWN`: Performs symmetric decimal rounding (half away from zero), rounding away from zero (up), and rounding towards zero (down/truncate) with positive, zero, or negative `num_digits`.
  - `SUMPRODUCT`: Multiplies corresponding elements across comma-separated range slices of equal dimensions and sums products; returns `#VALUE!` on mismatched dimensions.
- **Modified capabilities**:
  - `FunctionRegistry::standard()` automatically includes all 11 mathematical functions alongside `IF` (12 functions total).

---

## Design

### Goals

- Clean, scalable module layout organized strictly by `FunctionCategory`.
- Decentralized registration pattern where adding a new function requires modifying only its category module.
- Strict Excel calculation parity for range filtering, empty cell handling, and division-by-zero behavior.
- Zero production panics and complete preservation of existing test suite.

### Invariants

- **Range filtering parity**:
  - In arithmetic aggregators (`SUM`, `AVERAGE`, `MIN`, `MAX`), text, booleans, and empty cells in ranges are silently ignored; cell errors bubble up immediately.
  - In `COUNT`, only numeric cells are counted; text, booleans, empty cells, and cell errors in ranges are silently ignored (+0) without bubbling.
  - In `COUNTA`, all cells containing data (numbers, text, booleans, and errors) are counted (+1); only `CellValue::Empty` is ignored (+0).
- **Scalar argument parity**:
  - In arithmetic aggregators (`SUM`, `AVERAGE`, `MIN`, `MAX`), direct scalars coerce: booleans convert to `1.0`/`0.0`, empty cells coerce to `0.0`, text triggers `#VALUE!`, and errors bubble immediately.
  - In `COUNT`, direct scalars: numbers and booleans count as `1`; text, empty cells, and errors are ignored (+0, returning `0.0` without error).
  - In `COUNTA`, direct scalars: numbers, text, booleans, and errors count as `1`; empty cells count as `0`.
- **Zero dynamic spill**: Every function evaluates strictly to a scalar `CellValue`.
- **Exact dimensional alignment**: `SUMPRODUCT` requires all argument ranges to share identical `(rows, cols)` dimensions; mismatched dimensions return `CellValue::Error(ExcelError::Value)`.
- **SUMPRODUCT non-numeric parity**: Non-numeric entries (text, booleans, empty cells) within `SUMPRODUCT` range arrays are treated as `0.0`; cell errors bubble up immediately.
- **Rounding parameter coercion**: In `ROUND`, `ROUNDUP`, and `ROUNDDOWN`, `num_digits` is truncated to an integer (`num_digits.trunc() as i32`), clamped against float overflow limits (`[-308, 308]`), and normalizes `-0.0` to `0.0`.

### Decisions

- **Category subdirectories with domain groupings**:
  - *Choice*: Group functions into category subdirectories (`src/functions/math/`, `src/functions/logical/`) with domain-specific files (`aggregation.rs`, `rounding.rs`, `basic.rs`, `branching.rs`).
  - *Rationale*: Keeps individual file lengths under 250 lines, clusters functions that share internal algorithms, and mirrors `FunctionCategory`.
  - *Alternatives considered*: Single monolithic `math.rs` (rejected: unmaintainable at 10+ functions); strict 1-file-per-function (rejected: creates unnecessary directory clutter and boilerplate for tightly related functions like `ROUND`, `ROUNDUP`, `ROUNDDOWN`).
- **Dedicated, focused traversal primitives in `common.rs`**:
  - *Choice*: Implement 3 focused traversal helpers:
    1. `NumericCollector`: for arithmetic aggregations (`SUM`, `AVERAGE`, `MIN`, `MAX`) which require numeric coercion and immediate error bubbling.
    2. `count_numbers`: pure function for `COUNT` which counts numbers (and direct booleans) while ignoring text, empty cells, and errors without bubbling.
    3. `count_non_empty`: pure function for `COUNTA` which counts any cell containing data (numbers, text, booleans, errors) and ignores only empty cells.
  - *Rationale*: Maintains high cohesion and local reasoning. Prevents shoehorning contradictory error-handling and text-filtering semantics into a single complex struct with boolean flags.
  - *Alternatives considered*: Single monolithic collector with configuration flags (rejected: couples orthogonal semantics, increases cognitive overhead, and risks regressions); ad hoc loops in each function (rejected: violates single source of truth).
- **Decentralized `register(&mut FunctionRegistry)`**:
  - *Choice*: Each category exposes a `register` function to populate the registry.
  - *Rationale*: Keeps `registry.rs` decoupled from specific function struct imports.
  - *Alternatives considered*: Global constructor in `registry.rs` importing all 30 structs directly (rejected: high churn and merge conflicts in `registry.rs`).
- **Clean category-level exports without root shims**:
  - *Choice*: Re-export concrete function structs (`AbsFunction`, `SumFunction`, etc.) only at their respective category modules (`finx::functions::math::*`, `finx::functions::logical::*`). Keep `finx::functions` restricted to subsystem traits, registry, error models, and category modules.
  - *Rationale*: Adheres to greenfield minimalism by rejecting legacy backward compatibility shims. Prevents root namespace bloat as catalog expands to 30+ functions.
  - *Alternatives considered*: Preserving `finx::functions::{AbsFunction, SumFunction, IfFunction}` re-exports (rejected: arbitrary historical artifact with zero internal consumers); re-exporting all 30+ structs at root (rejected: namespace pollution).

### Data Contracts

#### Shared Helpers in `src/functions/common.rs`

```rust
pub struct NumericCollector {
    pub sum: f64,
    pub count: usize,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

impl NumericCollector {
    /// Collects numbers across arguments for arithmetic aggregators, bubbling any ExcelError encountered.
    pub fn collect(args: &[FunctionArg<'_>]) -> Result<Self, ExcelError>;

    /// Returns the minimum value, defaulting to 0.0 if no numbers were collected.
    pub fn min_or_zero(&self) -> f64;

    /// Returns the maximum value, defaulting to 0.0 if no numbers were collected.
    pub fn max_or_zero(&self) -> f64;
}

/// Counts numeric cells across arguments according to Excel COUNT semantics:
/// - In ranges: only numbers are counted; text, booleans, empty cells, and errors are ignored.
/// - In direct scalars: numbers and booleans count as 1; text, empty cells, and errors count as 0.
pub fn count_numbers(args: &[FunctionArg<'_>]) -> usize;

/// Counts non-empty cells across arguments according to Excel COUNTA semantics:
/// - In ranges: numbers, text, booleans, and errors count as 1; only empty cells are ignored.
/// - In direct scalars: numbers, text, booleans, and errors count as 1; empty cells count as 0.
pub fn count_non_empty(args: &[FunctionArg<'_>]) -> usize;

/// Performs symmetric rounding half away from zero.
pub fn round_half_away_from_zero(val: f64, digits: i32) -> f64;

/// Rounds away from zero (ceil for positive, floor for negative).
pub fn round_away_from_zero(val: f64, digits: i32) -> f64;

/// Rounds towards zero (truncate).
pub fn round_towards_zero(val: f64, digits: i32) -> f64;
```

#### New Math Function Signatures

| Function | Min args | Max args | Variadic rule | Param definitions |
| :--- | :--- | :--- | :--- | :--- |
| `AVERAGE` | 1 | None | `Single` | `number1` (`RangeOrArray`) |
| `COUNT` | 1 | None | `Single` | `value1` (`RangeOrArray`) |
| `COUNTA` | 1 | None | `Single` | `value1` (`RangeOrArray`) |
| `MAX` | 1 | None | `Single` | `number1` (`RangeOrArray`) |
| `MIN` | 1 | None | `Single` | `number1` (`RangeOrArray`) |
| `ROUND` | 2 | 2 | `None` | `number` (`Scalar`), `num_digits` (`Scalar`) |
| `ROUNDUP` | 2 | 2 | `None` | `number` (`Scalar`), `num_digits` (`Scalar`) |
| `ROUNDDOWN` | 2 | 2 | `None` | `number` (`Scalar`), `num_digits` (`Scalar`) |
| `SUMPRODUCT` | 1 | None | `Single` | `array1` (`RangeOrArray`) |

### Risks & Trade-offs

- **Floating-point rounding precision**: Multiplying by $10^d$ and dividing can introduce tiny IEEE-754 epsilon artifacts in edge cases.
  - *Mitigation*: Normalize signed zero (`-0.0` to `0.0`), clamp `digits` to `[-308, 308]`, and use standard scaling factors with `f64::powi`.
- **Module path breaking changes**: Direct imports of concrete function structs from `finx::functions::*` are removed.
  - *Mitigation*: Callers needing specific structs import via category modules (`finx::functions::math::*`, `finx::functions::logical::*`). The compiler and test suite already interact exclusively through `FunctionRegistry::standard()` and dynamic dispatch.

---

## Acceptance Criteria

- [x] **Scenario 1: Category folder modularization and registration**
  - **Given** the restructured `src/functions/` directory
  - **When** calling `FunctionRegistry::standard()`
  - **Then** all 12 standard functions (`ABS`, `SUM`, `AVERAGE`, `COUNT`, `COUNTA`, `MAX`, `MIN`, `ROUND`, `ROUNDUP`, `ROUNDDOWN`, `SUMPRODUCT`, `IF`) are registered and discoverable.

- [x] **Scenario 2: AVERAGE calculation and empty divisor**
  - **Given** a range containing `[10.0, 20.0, "ignored", true, 30.0]`
  - **When** evaluating `AVERAGE(range)`
  - **Then** evaluates to `20.0`.
  - **When** evaluating `AVERAGE(empty_range)` with no numbers
  - **Then** returns `ExcelError::Div0`.

- [x] **Scenario 3: COUNT vs. COUNTA semantics**
  - **Given** a range containing `[10.0, "text", true, Empty, ExcelError::Div0]`
  - **When** evaluating `COUNT(range)`
  - **Then** evaluates to `1.0` (only numbers counted; text, booleans, empty, and errors in ranges ignored).
  - **When** evaluating `COUNTA(range)`
  - **Then** evaluates to `4.0` (all non-empty cells counted: number, text, boolean, and error).
  - **When** evaluating `COUNT(10.0, "hello", true, ExcelError::Div0)` directly as scalars
  - **Then** evaluates to `2.0` (10.0 and true count as 1; "hello" and error ignored).
  - **When** evaluating `COUNT("hello")`
  - **Then** evaluates to `0.0`.
  - **When** evaluating `COUNTA(10.0, "hello", true, ExcelError::Div0)` directly as scalars
  - **Then** evaluates to `4.0`.

- [x] **Scenario 4: MAX and MIN with range filtering and empty fallback**
  - **Given** a range containing `[-15.0, 0.0, 42.5, "text"]`
  - **When** evaluating `MAX(range)` and `MIN(range)`
  - **Then** evaluates to `42.5` and `-15.0`.
  - **Given** a range containing all negative numbers `[-20.0, -10.0, -5.0]`
  - **When** evaluating `MAX(negative_range)` and `MIN(negative_range)`
  - **Then** evaluates to `-5.0` and `-20.0`.
  - **When** evaluating `MAX(empty_range)` and `MIN(empty_range)`
  - **Then** returns `0.0` and `0.0`.

- [x] **Scenario 5: ROUND, ROUNDUP, and ROUNDDOWN variants**
  - **Given** number `2.15` and `-2.15`
  - **When** evaluating `ROUND(2.15, 1)` and `ROUND(-2.15, 1)`
  - **Then** evaluates to `2.2` and `-2.2` (half away from zero).
  - **When** evaluating `ROUNDUP(2.141, 2)` and `ROUNDDOWN(2.149, 2)`
  - **Then** evaluates to `2.15` and `2.14`.
  - **When** evaluating `ROUND(125.0, -1)`
  - **Then** evaluates to `130.0`.

- [x] **Scenario 6: SUMPRODUCT dimension checking and calculation**
  - **Given** two ranges of dimensions $3 \times 1$: `[1.0, 2.0, 3.0]` and `[4.0, 5.0, 6.0]`
  - **When** evaluating `SUMPRODUCT(r1, r2)`
  - **Then** evaluates to `1*4 + 2*5 + 3*6 = 32.0`.
  - **Given** ranges with non-numeric text `[1.0, "text", 3.0]` and `[4.0, 5.0, 6.0]`
  - **When** evaluating `SUMPRODUCT(r1, r2)`
  - **Then** evaluates to `1*4 + 0*5 + 3*6 = 22.0` (text treated as 0.0).
  - **When** evaluating `SUMPRODUCT` with mismatched dimensions ($3 \times 1$ and $2 \times 1$)
  - **Then** returns `ExcelError::Value`.

---

## Tasks

1. [x] **Phase 1: Architecture modularization & shared utilities**
   - Create `src/functions/common.rs` with `NumericCollector`, `count_numbers`, `count_non_empty`, and rounding helpers.
   - Restructure `src/functions/logical/` (`mod.rs`, `branching.rs`).
   - Restructure `src/functions/math/` (`mod.rs`, `basic.rs`).
   - Update `src/functions/registry.rs` and `src/functions/mod.rs` with decentralized registration.
   - Verify existing tests pass (`cargo test`).

2. [x] **Phase 2: Math aggregation implementations**
   - Implement `AverageFunction`, `CountFunction`, `CountaFunction`, `MaxFunction`, `MinFunction` in `src/functions/math/aggregation.rs`.
   - Register new aggregation functions in `src/functions/math/mod.rs`.
   - Update `test_registry_metadata_and_iteration` in `tests/function_registry_test.rs` to reflect standard registry growth.
   - Add unit tests for aggregation functions.

3. [x] **Phase 3: Rounding and SUMPRODUCT implementations**
   - Implement `RoundFunction`, `RoundupFunction`, `RounddownFunction` in `src/functions/math/rounding.rs`.
   - Implement `SumproductFunction` in `src/functions/math/product.rs`.
   - Register rounding and product functions in `src/functions/math/mod.rs`.
   - Add unit tests for rounding and SUMPRODUCT.

4. [x] **Phase 4: Full verification & living spec reconciliation**
   - Add comprehensive tests in `tests/function_math_test.rs`.
   - Run verification suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
   - Reconcile `specs/living-specs/functions/registry.md` and archive draft spec to `specs/history/`.
