---
title: Function Registry Architecture & Core Archetypes Skeleton
date: 2026-09-18
status: draft
target-living-spec: specs/living-specs/functions/registry.md
---

## Proposal

### What

Implement the foundational runtime value system (`finx::value`), the extensible Excel function registry architecture (`finx::functions`), and three canonical archetypal functions (`ABS`, `SUM`, `IF`) covering fixed-scalar, variadic-range-aggregator, and lazy-branching execution contracts.

### Why

1. **Unified function contract across compiler passes**: The function registry acts as the single source of truth across the compiler lifecycle:
   - Pass 1: Validating function identifiers, arity bounds, and variadic rules during AST parsing.
   - Pass 4: Dispatching scalar values, range slices, and lazy expressions to registered Rust implementations during topological evaluation.
   - Pass 6: Lowering semantic function names into standard uppercase Excel formula strings.
2. **Lean skeleton before catalog expansion**: Testing parameter contracts, variadic validation, range aggregation filtering, and lazy evaluation on three canonical archetypes proves the architecture and dispatch mechanics without the noise of implementing 20+ specialized financial or date algorithms upfront.
3. **Strict type safety and Excel parity**: Introduces minimal, type-safe primitives ([`CellValue`](src/value/mod.rs) and [`ExcelError`](src/value/mod.rs)) conforming to Excel calculation semantics while preserving zero-panic, pure-function guarantees.

### Scope

- **In-scope**:
  - `src/value/mod.rs` & `src/value/error.rs`:
    - `CellValue`: `Number(f64)`, `Text(String)`, `Boolean(bool)`, `Empty`, `Error(ExcelError)`.
    - `ExcelError`: `Div0`, `Value`, `Ref`, `Num`, `Na`, `Name`.
    - `Display` implementation formatting values and error codes to exact Excel conventions (`#DIV/0!`, `#VALUE!`, `TRUE`, `FALSE`, etc.).
    - `From<ValueLiteral> for CellValue`.
    - Minimal helper methods: `as_number`, `as_bool`, `as_text`, `is_empty`, `is_error`.
  - `src/functions/mod.rs`, `src/functions/registry.rs`, `src/functions/traits.rs`, `src/functions/error.rs`:
    - `FunctionCategory` enum (`Math`, `Logical`, `Financial`, `Lookup`, `DateTime`, `Text`, `Information`).
    - `ParamKind` enum (`Scalar`, `RangeOrArray`, `Lazy`).
    - `VariadicRule` enum (`None`, `Single`, `Pairs`).
    - `ParameterDef` metadata struct (`name`, `description`, `kind`, `optional`, `default_value`).
    - `FunctionArg<'a>` enum (`Scalar(CellValue)`, `Array { rows, cols, values, is_range_ref }`, `Lazy(&'a Expr)`, `Omitted`).
    - `EvalContext` trait abstracting expression evaluation for lazy branches (`eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError>`).
    - `ExcelFunction` trait (`name`, `category`, `parameters`, `variadic_rule`, `min_args`, `max_args`, `evaluate`).
    - `FunctionRegistry`: dynamic registration, lookup by uppercase name, deterministic `IndexMap` storage, signature validation (`validate_call`), and argument binding (`bind_arguments`).
  - Core function archetypes (`src/functions/math.rs` & `src/functions/logical.rs`):
    - `ABS`: Scalar unary function (`ParamKind::Scalar`, arity 1).
    - `SUM`: Variadic aggregator (`ParamKind::RangeOrArray`, `VariadicRule::Single`, arity $\ge 1$). Ignores text and booleans in range references; coerces booleans and errors on text in scalar positions.
    - `IF`: Conditional branch (`ParamKind::Lazy`, arity 2..=3). Evaluates condition eagerly; lazily evaluates only the chosen branch; defaults omitted false branch (`_` or missing 3rd arg) to `CellValue::Boolean(false)`.
  - Comprehensive unit and integration test suite in `tests/function_registry_test.rs`.
- **Prerequisites / Dependencies**:
  - Formula boolean literals (`TRUE`, `FALSE`) and omitted argument placeholders (`Expr::Omitted`) implemented via [`specs/drafts/2026-09-18-formula-boolean-and-omitted-arguments.md`](specs/drafts/2026-09-18-formula-boolean-and-omitted-arguments.md).
- **Out-of-scope**:
  - In-memory topological evaluator walking `DependencyGraph` (Phase 4B: `finx::compiler::eval`).
  - Remaining standard catalog functions (Financial: `XIRR`, `NPV`, `PMT`; Date: `YEARFRAC`, `DATE`; Lookup: `XLOOKUP`, `INDEX`).
  - Physical coordinate lowering ($A$1) and Excel `.xlsx` emission (Pass 5 & 6).

### What Changes

- `src/value/mod.rs`: Declare and export `CellValue` and `ExcelError`.
- `src/value/error.rs`: Declare `ExcelError` with standard Excel string displays (`#DIV/0!`, etc.).
- `src/functions/mod.rs`: Re-export traits, registry, types, and standard library instances.
- `src/functions/traits.rs`: Declare `ExcelFunction`, `EvalContext`, `ParameterDef`, `ParamKind`, `VariadicRule`, `FunctionArg`, and `FunctionCategory`.
- `src/functions/registry.rs`: Implement `FunctionRegistry` with thread-safe registration (`IndexMap`), signature validation, and argument binding (`bind_arguments`).
- `src/functions/error.rs`: Declare structured `FunctionError` enum for validation, omission, and evaluation errors.
- `src/functions/math.rs`: Implement `AbsFunction` and `SumFunction`.
- `src/functions/logical.rs`: Implement `IfFunction`.
- `tests/function_registry_test.rs`: Integration tests for registry validation, evaluation semantics, range filtering, lazy evaluation, and argument binding.

### Capabilities

- **New capabilities**:
  - Representing typed runtime calculation outputs (`CellValue`) and Excel domain errors (`ExcelError`).
  - Converting AST `ValueLiteral` values into initial `CellValue` instances.
  - Registering and querying built-in and custom Excel functions in a thread-safe, deterministically ordered `FunctionRegistry`.
  - Validating function calls statically against minimum arity, maximum arity, variadic rules, and non-omittable required arguments with 1-indexed diagnostic positions.
  - Binding AST arguments into pre-evaluated scalars, multi-dimensional range arrays, lazy unevaluated expressions, or defaulted omitted parameters.
  - Evaluating `ABS` across numeric scalars, booleans, and empty cells, while rejecting omitted arguments (`_`).
  - Evaluating `SUM` across mixed scalar and range inputs, correctly ignoring text and booleans in range references.
  - Evaluating `IF` conditionally with short-circuiting so dormant branches are never executed.
- **Modified capabilities**: None (greenfield modules).

---

## Design

### Goals

1. **Clean boundary**: `functions` and `value` remain independent of `compiler`, depending only on `expr` and `model`.
2. **Deterministic execution**: Arithmetic and range filtering adhere to Excel parity rules ([`docs/compiler/parity.md`](file:///home/mhcfa/Work/finx/docs/compiler/parity.md)). Function iteration order is strictly deterministic.
3. **Zero legacy shims or stubs**: Every trait and data contract created in this skeleton is permanent and directly consumed by the Phase 4B evaluator.
4. **Actionable diagnostics**: Arity and signature mismatches produce clear, structured `FunctionError` variants with 1-indexed argument positions.

### Invariants

1. **Scalar evaluation guarantee**: Every function call evaluates strictly to a single scalar `CellValue` (or propagates an engine error).
2. **Domain vs. engine error isolation**: Mathematical and domain errors (`#DIV/0!`, `#VALUE!`, `#NUM!`) are returned as `Ok(CellValue::Error(...))`, allowing downstream formulas to propagate them. Internal compiler execution failures return `Err(FunctionError::...)`.
3. **Range reference aggregation filtering**: When range references are passed to `SUM`, text and boolean cells are ignored, whereas explicit scalar text arguments trigger `#VALUE!`.
4. **Lazy branch isolation**: In `IF(test, true_expr, false_expr)`, only the branch corresponding to the resolved condition is evaluated.
5. **Deterministic order**: Functions in `FunctionRegistry` iterate deterministically matching insertion order using `IndexMap`.

### Decisions

- **Decision 1: `EvalContext` as a trait in `finx::functions`**:
  - *Choice*: Define `pub trait EvalContext { fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError>; }` within `finx::functions::traits`.
  - *Rationale*: Allows `IF` (and future lazy functions like `IFS`, `CHOOSE`, `SWITCH`) to evaluate lazy branches without coupling `finx::functions` to the internal state of `finx::compiler::eval`.
  - *Alternatives considered*: Passing a concrete compiler struct. Rejected because it creates a circular crate/module dependency between `compiler` and `functions`.
- **Decision 2: Three distinct archetypes (`ABS`, `SUM`, `IF`)**:
  - *Choice*: Implement exactly three functions representing the three fundamental argument consumption models: scalar unary, variadic range aggregator, and lazy conditional branch.
  - *Rationale*: Tests all execution pathways of `FunctionArg` and `ParamKind` with minimal code surface.
  - *Alternatives considered*: Implementing only `SUM`. Rejected because it wouldn't exercise `ParamKind::Lazy` or fixed arity validation. Implementing all 25 catalog functions. Rejected as overly complex for a single spec.
- **Decision 3: `FunctionRegistry` built-in factory `standard()` with `IndexMap` storage**:
  - *Choice*: Provide `FunctionRegistry::standard()` pre-loaded with the registered archetypes, backed by `IndexMap<String, Arc<dyn ExcelFunction>>`.
  - *Rationale*: Offers a turnkey, thread-safe registry for compiler passes and testing with guaranteed deterministic iteration order conforming to `AGENTS.md`.
- **Decision 4: Diagnostic error context on argument resolution**:
  - *Choice*: Pass `function: &'static str` into `FunctionArg::eval_scalar` and `FunctionArg::eval_array`, and format `CannotOmitRequiredArgument.position` as 1-indexed.
  - *Rationale*: Enables argument binding and resolution helpers to construct fully qualified, user-friendly `FunctionError::InvalidArgumentKind` or `FunctionError::CannotOmitRequiredArgument` diagnostics containing the enclosing function's name and human-readable 1-indexed argument positions.
- **Decision 5: Unified handling of missing vs. explicit `_` omitted arguments in `bind_arguments`**:
  - *Choice*: When binding arguments, both missing trailing optional parameters (e.g. `IF` called with 2 arguments) and explicit `Expr::Omitted` placeholders (`_`) are bound to the parameter's `default_value` if present, or to `FunctionArg::Omitted`.
  - *Rationale*: Guarantees identical execution semantics whether a caller writes `IF(test, true_val)` or `IF(test, true_val, _)`.

### Data Contracts

#### Value Enums (`src/value/mod.rs`, `src/value/error.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExcelError {
    Div0,   // #DIV/0!
    Value,  // #VALUE!
    Ref,    // #REF!
    Num,    // #NUM!
    Na,     // #N/A
    Name,   // #NAME?
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CellValue {
    Number(f64),
    Text(String),
    Boolean(bool),
    Empty,
    Error(ExcelError),
}
```

#### Error Enum (`src/functions/error.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FunctionError {
    #[error("unknown function '{0}'")]
    UnknownFunction(String),
    #[error("too few arguments for '{function}': expected at least {expected}, found {found}")]
    TooFewArguments {
        function: &'static str,
        expected: usize,
        found: usize,
    },
    #[error("too many arguments for '{function}': expected at most {expected}, found {found}")]
    TooManyArguments {
        function: &'static str,
        expected: usize,
        found: usize,
    },
    #[error("argument '{param}' at position {position} of '{function}' cannot be omitted")]
    CannotOmitRequiredArgument {
        function: &'static str,
        param: &'static str,
        /// 1-indexed argument position for user-facing diagnostics.
        position: usize,
    },
    #[error("invalid argument count for variadic pairs in '{function}'")]
    InvalidVariadicPairs {
        function: &'static str,
        count: usize,
    },
    #[error("invalid argument kind for '{function}': expected {expected}, found {found}")]
    InvalidArgumentKind {
        function: &'static str,
        expected: &'static str,
        found: &'static str,
    },
    #[error("internal evaluation error in '{function}': {message}")]
    EvaluationFailure {
        function: &'static str,
        message: String,
    },
}
```

#### Registry Types & Contracts (`src/functions/traits.rs`, `src/functions/registry.rs`)

```rust
pub enum ParamKind {
    Scalar,
    RangeOrArray,
    Lazy,
}

pub enum VariadicRule {
    None,
    Single,
    Pairs,
}

pub struct ParameterDef {
    pub name: &'static str,
    pub description: &'static str,
    pub kind: ParamKind,
    pub optional: bool,
    pub default_value: Option<CellValue>,
}

pub enum FunctionArg<'a> {
    Scalar(CellValue),
    Array {
        rows: usize,
        cols: usize,
        values: Vec<CellValue>,
        is_range_ref: bool,
    },
    Lazy(&'a Expr),
    Omitted,
}

impl<'a> FunctionArg<'a> {
    pub fn eval_scalar(&self, function: &'static str, ctx: &dyn EvalContext) -> Result<CellValue, FunctionError>;
    pub fn eval_array(&self, function: &'static str, ctx: &dyn EvalContext) -> Result<(usize, usize, Vec<CellValue>), FunctionError>;
}

pub trait EvalContext {
    fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError>;
}

pub trait ExcelFunction: Send + Sync {
    fn name(&self) -> &'static str;
    fn category(&self) -> FunctionCategory;
    fn parameters(&self) -> &'static [ParameterDef];
    fn variadic_rule(&self) -> VariadicRule { VariadicRule::None }
    fn min_args(&self) -> usize {
        self.parameters().iter().take_while(|p| !p.optional).count()
    }
    fn max_args(&self) -> Option<usize> {
        match self.variadic_rule() {
            VariadicRule::None => Some(self.parameters().len()),
            VariadicRule::Single | VariadicRule::Pairs => None,
        }
    }
    fn evaluate(&self, args: &[FunctionArg], ctx: &dyn EvalContext) -> Result<CellValue, FunctionError>;
}

pub struct FunctionRegistry {
    functions: IndexMap<String, Arc<dyn ExcelFunction>>,
}

impl FunctionRegistry {
    pub fn new() -> Self;
    pub fn standard() -> Self;
    pub fn register(&mut self, func: Arc<dyn ExcelFunction>);
    pub fn lookup(&self, name: &str) -> Option<&dyn ExcelFunction>;
    pub fn validate_call(&self, name: &str, args: &[Expr]) -> Result<&dyn ExcelFunction, FunctionError>;
    pub fn bind_arguments<'a>(
        &self,
        func: &dyn ExcelFunction,
        expr_args: &'a [Expr],
        ctx: &dyn EvalContext,
    ) -> Result<Vec<FunctionArg<'a>>, FunctionError>;
}
```

#### Archetype Signatures

| Function | Category | Parameters | Variadic rule | Semantics |
| :--- | :--- | :--- | :--- | :--- |
| `ABS` | `Math` | `number` (`Scalar`, required) | `None` | Unary scalar magnitude. Passing `_` returns `CannotOmitRequiredArgument`. |
| `SUM` | `Math` | `value` (`RangeOrArray`, required) | `Single` | Variadic aggregation. Ignores text and booleans in range references. |
| `IF` | `Logical` | 1. `logical_test` (`Scalar`, required)<br>2. `value_if_true` (`Lazy`, required)<br>3. `value_if_false` (`Lazy`, optional, default: `CellValue::Boolean(false)`) | `None` | Short-circuit lazy branching. If condition is false and branch 3 is omitted (`_` or missing), evaluates to `FALSE`. |

### Risks & Trade-offs

| Risk | Mitigation |
| :--- | :--------- |
| Floating point precision issues in `SUM` | Use 64-bit IEEE 754 floats (`f64`), normalize `-0.0` to `0.0`, and match Excel IEEE-754 semantics. |
| Lazy evaluation infinite loops | Graph cycle detection in Pass 3 guarantees the formula dependency graph is acyclic before evaluation occurs. |
| Signature validation out of sync with lowering | Single registry source of truth ensures both parser validation and lowering queries use identical metadata. |
| Semantic confusion between empty cells and omitted parameters | Enforce `_` exclusively as an omitted argument placeholder in formula calls; `CellValue::Empty` is restricted to unpopulated cell coordinates. |

---

## Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: `CellValue` and `ExcelError` representation and display**
  - **Given** various `CellValue` and `ExcelError` variants
  - **When** formatted via `Display`
  - **Then** `ExcelError::Div0` prints `"#DIV/0!"`, `ExcelError::Value` prints `"#VALUE!"`, booleans print `"TRUE"` / `"FALSE"`, and `From<ValueLiteral>` converts correctly.

- [ ] **Scenario 2: Registry signature, arity, and omission validation**
  - **Given** a `FunctionRegistry` with `ABS` (fixed arity 1) and `SUM` (variadic single, min 1)
  - **When** validating calls with valid argument counts, invalid counts, or passing `_` to non-optional parameters
  - **Then** valid counts succeed; `ABS` with 0 or 2 args fails with `TooFewArguments` or `TooManyArguments`; passing `_` to `ABS(_)` fails with `CannotOmitRequiredArgument` with 1-indexed position; unknown functions fail with `UnknownFunction`.

- [ ] **Scenario 3: `ABS` scalar evaluation**
  - **Given** an `ABS` function instance
  - **When** evaluated with positive numbers, negative numbers, empty cells, booleans, and errors
  - **Then** `-42.5` evaluates to `42.5`, `Empty` evaluates to `0.0`, `TRUE` evaluates to `1.0`, errors propagate, and text evaluates to `ExcelError::Value`.

- [ ] **Scenario 4: `SUM` aggregation and range filtering**
  - **Given** a `SUM` function instance
  - **When** evaluated with a mix of scalar numbers, empty cells, and a range slice containing numbers, text, and booleans
  - **Then** range text and booleans are ignored, numbers are summed, empty cells contribute `0.0`, and cell errors in ranges immediately bubble up.

- [ ] **Scenario 5: `IF` short-circuiting lazy branch execution and missing/omitted false branch**
  - **Given** an `IF` function instance and an `EvalContext` tracking branch execution
  - **When** evaluating `IF(TRUE, val_true, val_false)`, `IF(FALSE, val_true, val_false)`, `IF(FALSE, val_true, _)`, and `IF(FALSE, val_true)`
  - **Then** only the selected branch expression is dispatched to `eval_expr`, the dormant branch is never executed, and omitted false branch evaluates to `CellValue::Boolean(false)`.

---

## Tasks

### Phase 1: Runtime Values (`finx::value`)
- [ ] Implement `ExcelError` in `src/value/error.rs` with `Display` and serde serialization.
- [ ] Implement `CellValue` in `src/value/mod.rs` with `Display`, accessors, and `From<ValueLiteral>`.
- [ ] Write unit tests in `src/value/mod.rs` verifying formatting, conversions, and error representations.

### Phase 2: Function Registry Architecture (`finx::functions`)
- [ ] Define parameter and argument types (`ParamKind`, `VariadicRule`, `ParameterDef`, `FunctionCategory`, `FunctionArg`) in `src/functions/traits.rs`.
- [ ] Implement `eval_scalar` and `eval_array` on `FunctionArg` taking `function: &'static str` for contextual error diagnostics.
- [ ] Define `EvalContext` and `ExcelFunction` traits in `src/functions/traits.rs`.
- [ ] Define `FunctionError` in `src/functions/error.rs` with 1-indexed `position` in `CannotOmitRequiredArgument`.
- [ ] Implement `FunctionRegistry` in `src/functions/registry.rs` with `IndexMap` storage, registration, lookup, arity validation, and `bind_arguments`.
- [ ] Re-export all core types in `src/functions/mod.rs`.

### Phase 3: Core Archetype Implementations
- [ ] Implement `ABS` in `src/functions/math.rs`.
- [ ] Implement `SUM` in `src/functions/math.rs` with scalar coercion and range reference filtering.
- [ ] Implement `IF` in `src/functions/logical.rs` with lazy short-circuiting and default value handling.
- [ ] Wire archetypes into `FunctionRegistry::standard()`.

### Phase 4: Verification Suite
- [ ] Create `tests/function_registry_test.rs` covering all acceptance criteria scenarios.
- [ ] Run `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`.
