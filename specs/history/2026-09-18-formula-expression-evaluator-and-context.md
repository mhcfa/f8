---
title: Formula Expression Evaluator & Evaluation Context
date: 2026-09-18
status: completed
target-living-spec: specs/living-specs/compiler/evaluator.md
---

## Proposal

### What

Implement the **Formula Expression Evaluator** (`finx::compiler::eval::expr`), the evaluation context bridge (`finx::compiler::eval::context::CellEvalContext`), the compiler-level evaluation error taxonomy (`finx::compiler::eval::error::EvalError`), and upgrade coordinate slice resolution to preserve 2D shape metadata (`ResolvedSlice` in `finx::compiler::graph::resolve`).

### Why

1. **Excel calculation parity**: Formula expressions in financial models require rigorous, deterministic evaluation conforming to Excel calculation parity rules (IEEE 754 arithmetic, signed zero `-0.0` normalization, power edge cases, divide-by-zero, non-numeric `#VALUE!`, and type-aware comparisons).
2. **Autonomous verification**: Formula evaluation logic (operators, coercions, short-circuit lazy branches, range extractions, and function calls) can be exhaustively verified in isolation using static value maps without depending on the full DAG topological loop.
3. **Context abstraction**: Extensible functions like `IF` and `SUM` require a concrete implementation of the [`EvalContext`](../../src/functions/traits.rs) trait (`eval_expr` and `eval_range`) to evaluate nested expressions and fetch 2D range slices.
4. **2D shape preservation**: Functions operating on ranges and matrices require explicit grid dimensions `(rows, cols)` preserved by coordinate resolution, rather than losing shape data in flattened vectors.

### Scope

- **In-scope**:
  - `src/compiler/graph/resolve.rs`:
    - Define `ResolvedSlice` struct (`pub rows: usize, pub cols: usize, pub cells: Vec<CellId>`) with `into_cells`, `cells`, `Deref<Target = [CellId]>`, and `IntoIterator<Item = CellId>`.
    - Upgrade `resolve_table_ref` to return `Result<ResolvedSlice, ResolveError>`, supplying 2D shape metadata directly.
    - Re-export `ResolvedSlice` in `src/compiler/graph/mod.rs`.
    - Ensure `resolve_cell_dependencies` and existing reference resolution tests seamlessly consume `ResolvedSlice`.
  - `src/compiler/eval/error.rs`:
    - Define structured `EvalError` enum with `thiserror` for evaluation failure modes (`Resolve`, `Function`, `UnexpectedOmitted`, `UnevaluatedDependency`, `CellNotFound`).
  - `src/compiler/eval/context.rs`:
    - Implement `CellEvalContext<'a>` borrowing `origin: &'a CellId`, `memoized_values: &'a IndexMap<CellId, CellValue>`, `symbol_table: &'a SymbolTable`, and `registry: &'a FunctionRegistry`.
    - Implement `EvalContext` on `CellEvalContext`:
      - `eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError>` delegating to `expr::eval_expr`.
      - `eval_range(&self, expr: &Expr) -> Result<(usize, usize, Vec<CellValue>), FunctionError>` resolving references via `resolve_table_ref`, returning `(rows, cols, values)`.
  - `src/compiler/eval/expr.rs`:
    - Expression evaluation of AST [`Expr`](../../src/expr/ast.rs) nodes:
      - Constant literals (`Literal::Number`, `Literal::String`, `Literal::Boolean`).
      - Unary operations (`+`, `-`) with Excel sign, numeric coercion rules, and `-0.0` normalization.
      - Binary arithmetic (`+`, `-`, `*`, `/`, `^`) with IEEE 754 float precision, empty-cell zero coercion, non-numeric `#VALUE!`, divide-by-zero `#DIV/0!`, `0.0 ^ negative` `#DIV/0!`, non-real exponentiation / overflow / `NaN` `#NUM!`, and `-0.0` normalization.
      - Binary comparisons (`=`, `<>`, `<`, `<=`, `>`, `>=`) returning `CellValue::Boolean` with Excel type ranking:
        - Same type: standard value comparison (strings compare case-insensitively using ASCII case folding, numbers compare by float equality).
        - Numeric vs Empty: `Empty` coerces to `0.0`.
        - Text vs Empty: `Empty` coerces to `""`.
        - Boolean vs Empty: `Empty == false` evaluates to `false`, `Empty == true` evaluates to `false` (matching Excel).
        - Mixed types: Number < Text < Boolean (matching Excel order).
        - Error bubbling: Any comparison involving `CellValue::Error` propagates that error immediately.
      - Reference resolution:
        - Exactly 1 cell: look up in `memoized_values`. If missing, return `EvalError::UnevaluatedDependency`.
        - >1 cell in scalar position: return `Ok(CellValue::Error(ExcelError::Value))`.
        - 0 cells: return `Ok(CellValue::Error(ExcelError::Ref))`.
      - Standalone `Expr::Omitted`: return `Err(EvalError::UnexpectedOmitted(origin.clone()))`.
      - Function calls: dispatching through [`FunctionRegistry::bind_arguments`](../../src/functions/registry.rs) and executing [`ExcelFunction::evaluate`](../../src/functions/traits.rs).
  - `src/compiler/eval/mod.rs`:
    - Declare submodules `context`, `error`, `expr`.
    - Re-export `EvalError`, `CellEvalContext`, and `eval_expr`.
  - Comprehensive unit test suite in `tests/expression_eval_test.rs`.
- **Out-of-scope**:
  - Model-wide topological execution loop (`Evaluator::evaluate`) and `EvaluatedModel` query APIs (Pass 4B).
  - Layout planning and freeze pane assignment (Pass 5).
  - Excel `.xlsx` emission (Pass 6).

### What Changes

- `src/compiler/graph/resolve.rs`: Declare `ResolvedSlice` and update `resolve_table_ref` return type.
- `src/compiler/graph/mod.rs`: Re-export `ResolvedSlice`.
- `src/compiler/eval/mod.rs`: Replace stub with submodule declarations and public exports.
- `src/compiler/eval/error.rs`: Declare `EvalError` using `thiserror`.
- `src/compiler/eval/context.rs`: Implement `CellEvalContext` satisfying `EvalContext`.
- `src/compiler/eval/expr.rs`: Implement AST expression evaluation and Excel parity operator arithmetic.
- `tests/expression_eval_test.rs`: Unit test suite verifying expression evaluation and Excel parity.

### Capabilities

- **New capabilities**:
  - Full Excel arithmetic parity: coercion of empty cells to zero, boolean coercion in math, strict rejection of non-numeric text with `#VALUE!`, and divide-by-zero detection (`#DIV/0!`).
  - Excel comparison operator parity with mixed-type ranking (`Number < Text < Boolean`) and case-insensitive string comparisons.
  - IEEE 754 edge case normalization: negative zero `-0.0` $\rightarrow$ `0.0`, `0 ^ negative` $\rightarrow$ `#DIV/0!`, `(-x) ^ frac` / `Inf` / `NaN` $\rightarrow$ `#NUM!`.
  - Scalar vs. range reference isolation: resolving 1-cell references to scalar values, rejecting multi-cell ranges in scalar arithmetic with `#VALUE!`, and resolving 2D slices for range parameters.
  - Short-circuit lazy evaluation via `EvalContext` bridge, ensuring dormant branches in `IF` are never calculated.
  - Function dispatching through `FunctionRegistry` with signature validation, required argument checking, and default fallback substitution.
- **Modified capabilities**:
  - `resolve_table_ref` now returns `ResolvedSlice`, preserving 2D `(rows, cols)` dimensions alongside resolved `CellId` sequence.

---

## Design

### Goals

1. **Strict Excel parity**: Adhere to calculation rules specified in [`docs/compiler/parity.md`](../../docs/compiler/parity.md).
2. **Clean error boundaries**: Clearly separate internal compiler failures (`EvalError`) from runtime formula outcomes (`CellValue::Error(ExcelError)`).
3. **Autonomous verifiability**: Enable unit testing of all operators, functions, and coercions using static cell value maps without requiring a full model graph.
4. **Zero production panics**: Zero unwrap or panic in non-test code; propagate all failures cleanly.

### Invariants

1. **Scalar evaluation guarantee**: Every expression in a scalar context evaluates to exactly one scalar `CellValue` (or aborts on compiler error).
2. **Domain error bubbling**: When arithmetic operators encounter a `CellValue::Error(...)`, the error immediately bubbles up to the evaluating expression without panicking.
3. **Lazy branch isolation**: In lazy functions like `IF`, expressions in dormant branches are never evaluated.
4. **Range aggregation filtering**: Passing a 2D range reference to an aggregator like `SUM` ignores text, booleans, and empty cells, while explicit scalar text arguments trigger `#VALUE!`.
5. **No standalone omitted arguments**: The `_` placeholder is strictly valid only as an argument in function invocations; evaluating `_` directly within arithmetic or logical operators halts compilation.

### Decisions

- **Decision 1: Separation of `EvalError` and `ExcelError`**:
  - *Choice*: Internal compiler failures (e.g. missing cell in grid, unknown table in symbol table, unresolvable AST) return `Err(EvalError)` and abort compilation. Runtime formula errors (`#DIV/0!`, `#VALUE!`, `#NUM!`, `#REF!`, `#N/A`, `#NAME?`) return `Ok(CellValue::Error(ExcelError))` and are stored in the evaluated matrix.
  - *Rationale*: A spreadsheet model containing `=1/0` is a syntactically valid model that produces `#DIV/0!`. Downstream formulas must be able to inspect or propagate this error.
  - *Alternatives considered*: Aborting compilation on all Excel errors (rejected: breaks valid modeling patterns like `IFERROR` or error testing).

- **Decision 2: Evaluation context abstraction**:
  - *Choice*: Implement `CellEvalContext<'a>` borrowing `origin: &'a CellId`, `memoized_values: &'a IndexMap<CellId, CellValue>`, `symbol_table: &'a SymbolTable`, and `registry: &'a FunctionRegistry`. Implement the `EvalContext` trait on `CellEvalContext`.
  - *Rationale*: Directly implements the decoupling contract established in Pass 4A ([`src/functions/traits.rs`](../../src/functions/traits.rs)), allowing functions like `IF` to lazily evaluate AST expressions via `ctx.eval_expr(expr)` and `SUM` to fetch range grids via `ctx.eval_range(expr)`.
  - *Alternatives considered*: Passing mutable evaluator state into functions (rejected: violates thread safety and pure function contracts).

- **Decision 3: Comparison operator parity**:
  - *Choice*: Binary comparisons (`=`, `<>`, `<`, `<=`, `>`, `>=`) evaluate with Excel type ranking:
    - Same type: standard value comparison (strings compare case-insensitively using ASCII case folding, numbers compare by float equality).
    - Numeric vs Empty: `Empty` coerces to `0.0`.
    - Text vs Empty: `Empty` coerces to `""`.
    - Boolean vs Empty: `Empty == false` evaluates to `false`, `Empty == true` evaluates to `false` (matching Excel).
    - Mixed types: Number < Text < Boolean (matching Excel order).
    - Errors: Any comparison with an `ExcelError` propagates that error as `CellValue::Error`.
  - *Rationale*: Matches Excel comparison behavior and prevents unexpected ordering bugs in financial conditional checks.

- **Decision 4: Scalar reference isolation & multi-cell handling**:
  - *Choice*: When `eval_expr` resolves an `Expr::Reference(table_ref)`:
    - If it resolves to exactly 1 cell: fetch and return the evaluated scalar `CellValue`.
    - If it resolves to >1 cells: return `CellValue::Error(ExcelError::Value)`.
    - If it resolves to 0 cells: return `CellValue::Error(ExcelError::Ref)`.
    Multi-cell slices are evaluated strictly through `ctx.eval_range(expr)` within argument positions declared as `ParamKind::RangeOrArray`.
  - *Rationale*: Finx strictly adheres to FAST Standard Section 2.01 (zero dynamic arrays or cell spilling). Evaluating an array in a scalar arithmetic slot is a domain type error (`#VALUE!`).
  - *Alternatives considered*: Implicitly taking the first cell (rejected: silent miscalculation).

- **Decision 5: IEEE 754 float parity and non-finite normalization**:
  - *Choice*:
    - Signed zero `-0.0` normalizes to `0.0`.
    - `0.0 ^ negative` maps to `CellValue::Error(ExcelError::Div0)`.
    - Negative base with fractional exponent (e.g. `(-2.0)^0.5`) maps to `CellValue::Error(ExcelError::Num)`.
    - Float overflow to infinity or `NaN` maps to `CellValue::Error(ExcelError::Num)`.
    - Division by zero (`x / 0.0`) maps to `CellValue::Error(ExcelError::Div0)`.
  - *Rationale*: Conforms to Microsoft Excel calculation specifications and IEEE 754 handling documented in `docs/compiler/parity.md`.

- **Decision 6: `ResolvedSlice` compatibility**:
  - *Choice*: `ResolvedSlice` implements `Deref<Target = [CellId]>` and `IntoIterator<Item = CellId>`.
  - *Rationale*: Guarantees that callers like `resolve_cell_dependencies` and existing tests can continue indexing, slicing, and extending without churn.

### Data Contracts

#### `ResolvedSlice` & `resolve_table_ref` (`src/compiler/graph/resolve.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSlice {
    pub rows: usize,
    pub cols: usize,
    pub cells: Vec<CellId>,
}

impl ResolvedSlice {
    pub fn new(rows: usize, cols: usize, cells: Vec<CellId>) -> Self;
    pub fn is_empty(&self) -> bool;
    pub fn len(&self) -> usize;
    pub fn cells(&self) -> &[CellId];
    pub fn into_cells(self) -> Vec<CellId>;
}

impl std::ops::Deref for ResolvedSlice {
    type Target = [CellId];
    fn deref(&self) -> &Self::Target;
}

impl IntoIterator for ResolvedSlice {
    type Item = CellId;
    type IntoIter = std::vec::IntoIter<CellId>;
    fn into_iter(self) -> Self::IntoIter;
}

pub fn resolve_table_ref(
    origin: &CellId,
    table_ref: &TableRef,
    symbol_table: &SymbolTable,
) -> Result<ResolvedSlice, ResolveError>;
```

#### `EvalError` (`src/compiler/eval/error.rs`)

```rust
#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("Cell '{0}' not found in cell grid")]
    CellNotFound(CellId),
    #[error("Referenced cell '{0}' has not been evaluated (dependency ordering anomaly)")]
    UnevaluatedDependency(CellId),
    #[error("Table reference resolution error: {0}")]
    Resolve(#[from] ResolveError),
    #[error("Function error in cell '{cell}': {source}")]
    Function {
        cell: CellId,
        #[source]
        source: FunctionError,
    },
    #[error("Unexpected omitted argument placeholder '_' in cell '{0}'")]
    UnexpectedOmitted(CellId),
}
```

#### `CellEvalContext` (`src/compiler/eval/context.rs`)

```rust
pub struct CellEvalContext<'a> {
    pub origin: &'a CellId,
    pub memoized_values: &'a IndexMap<CellId, CellValue>,
    pub symbol_table: &'a SymbolTable,
    pub registry: &'a FunctionRegistry,
}

impl<'a> EvalContext for CellEvalContext<'a> {
    fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError>;
    fn eval_range(&self, expr: &Expr) -> Result<(usize, usize, Vec<CellValue>), FunctionError>;
}
```

#### `eval_expr` (`src/compiler/eval/expr.rs`)

```rust
pub fn eval_expr(
    expr: &Expr,
    origin: &CellId,
    memoized_values: &IndexMap<CellId, CellValue>,
    symbol_table: &SymbolTable,
    registry: &FunctionRegistry,
) -> Result<CellValue, EvalError>;
```

### Risks & Trade-offs

- **Direct AST recursion depth**:
  - *Risk*: Deeply nested formula expressions could potentially cause stack overflow.
  - *Mitigation*: Practical financial formulas rarely exceed 10–20 levels of nesting, well within standard thread stack limits.
- **Dynamic trait dispatch in `EvalContext`**:
  - *Risk*: Passing `&dyn EvalContext` introduces vtable indirect calls during function evaluation.
  - *Mitigation*: Calls occur only at function call boundaries (`IF`, `SUM`), whose cost is dominated by float math and reference resolution.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Formula Arithmetic and Excel Parity Coercion**
  - **Given** formula expressions evaluating arithmetic: `10 + 20 * 2`, `Empty + 5`, `TRUE * 10`, `FALSE + 2`
  - **When** evaluated via `eval_expr`
  - **Then** evaluates to `50.0`, `5.0`, `10.0`, and `2.0`.

- [x] **Scenario 2: Divide-by-Zero, Power Anomalies, and Float Normalization**
  - **Given** formula expressions evaluating `10 / 0`, `10 / Empty`, `0 ^ -1`, `(-2) ^ 0.5`, or `-0.0`
  - **When** evaluated
  - **Then** evaluates `10 / 0` and `0 ^ -1` to `CellValue::Error(ExcelError::Div0)`, `(-2) ^ 0.5` to `CellValue::Error(ExcelError::Num)`, normalizes `-0.0` to `0.0`, and downstream cells bubble errors.

- [x] **Scenario 3: Comparison Operators and Excel Type Ranking**
  - **Given** comparisons across mixed types: `10 < "2"`, `"Apple" = "apple"`, `100 < TRUE`, `Empty = 0`, `Empty = ""`, `Empty = FALSE`
  - **When** evaluated
  - **Then** returns `TRUE`, `TRUE`, `TRUE`, `TRUE`, `TRUE`, and `FALSE`.

- [x] **Scenario 4: Short-Circuit Lazy Evaluation (`IF`)**
  - **Given** a formula `IF(TRUE, 100, 1 / 0)`
  - **When** evaluated
  - **Then** returns `100.0` and the false branch `1 / 0` is never executed, producing no `#DIV/0!`.

- [x] **Scenario 5: Multi-Cell Reference in Scalar Position Rejection**
  - **Given** a cell formula evaluating a multi-cell range in scalar arithmetic: `Table["r1".."r3", @col] + 10`
  - **When** evaluated
  - **Then** evaluates to `CellValue::Error(ExcelError::Value)` without engine panic.

- [x] **Scenario 6: 2D Range Resolution and Aggregation (`SUM`)**
  - **Given** a formula evaluating `SUM(Table["r1".."r3", @col])` where cells contain a mix of numbers, empty cells, and text
  - **When** evaluated
  - **Then** `eval_range` extracts the 2D slice with accurate `(rows, cols)` and `SUM` aggregates numeric values while ignoring text and empty cells.

- [x] **Scenario 7: Standalone Omitted Argument Rejection**
  - **Given** a cell formula containing standalone `_` in arithmetic (e.g. `1 + _`)
  - **When** evaluated
  - **Then** returns `Err(EvalError::UnexpectedOmitted)`.

---

## Tasks

### Phase 1: Range Resolution & Error Taxonomy

- [x] Implement `ResolvedSlice` in `src/compiler/graph/resolve.rs` (`rows, cols, cells`) with `into_cells`, `cells`, `Deref`, and `IntoIterator`.
- [x] Upgrade `resolve_table_ref` to return `Result<ResolvedSlice, ResolveError>`.
- [x] Verify `resolve_cell_dependencies` and existing tests in `tests/reference_resolution_test.rs` pass with `ResolvedSlice`. Re-export in `src/compiler/graph/mod.rs`.
- [x] Define `EvalError` in `src/compiler/eval/error.rs` with structured variants including `UnexpectedOmitted` and `thiserror` annotations.

### Phase 2: Expression Evaluator & Excel Parity Engine

- [x] Implement `src/compiler/eval/expr.rs` handling:
  - Literal evaluation (`Literal` $\rightarrow$ `CellValue`).
  - Unary operators (`+`, `-`) with Excel coercion and `-0.0` normalization.
  - Binary arithmetic operators (`+`, `-`, `*`, `/`, `^`) with domain error generation (`#DIV/0!`, `#VALUE!`, `#NUM!`) and `-0.0` normalization.
  - Binary comparison operators (`=`, `<>`, `<`, `<=`, `>`, `>=`) with Excel type-aware ordering and case-insensitive string matching.
  - Reference resolution: scalar lookup for single cell, `ExcelError::Value` for multi-cell in scalar context, `ExcelError::Ref` for empty range.
  - Standalone `Expr::Omitted` error propagation.
  - Function call dispatching via `FunctionRegistry`.

### Phase 3: Evaluation Context Bridge

- [x] Implement `CellEvalContext` in `src/compiler/eval/context.rs` satisfying `EvalContext`:
  - `eval_expr` delegating to `expr::eval_expr`.
  - `eval_range` using `resolve_table_ref` to construct 2D array shapes `(rows, cols, values)`.
- [x] Re-export `EvalError`, `CellEvalContext`, and `eval_expr` in `src/compiler/eval/mod.rs` and `src/compiler/mod.rs`.

### Phase 4: Verification Suite

- [x] Create `tests/expression_eval_test.rs` covering:
  - Arithmetic operations, precedence, and IEEE 754 float parity (`-0.0`, `0 ^ -1`, `NaN`).
  - Coercion semantics (empty cell to zero, booleans to numbers, invalid text to `#VALUE!`).
  - Comparison operator type ranking (`Number < Text < Boolean`, case-insensitive strings).
  - Multi-cell scalar rejection (`#VALUE!`) and empty range (`#REF!`).
  - Lazy branch execution with `IF`.
  - Range aggregation with `SUM` ignoring text and blank cells.
  - Standalone `_` rejection.
- [x] Run verification suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- [x] Reconcile draft spec and prepare for Pass 4B.
