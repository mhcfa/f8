---
title: Excel Function Registry & Core Archetypes
last_updated: 2026-09-18
domain: functions
status: active
---

## Purpose

The `functions` and `value` subsystems provide the runtime calculation value model (`CellValue`), domain error hierarchy (`ExcelError`), extensible Excel function registry architecture (`ExcelFunction`, `FunctionRegistry`), and canonical core function archetypes (`ABS`, `SUM`, `IF`, etc.). Together, they form the single source of truth for function signature validation, argument binding, scalar coercion, range aggregation filtering, Excel value comparison/equality, and lazy branch evaluation across compiler passes.

## Architecture & Boundaries

- **Modules & components**:
  - `finx::value`: Strongly typed runtime cell representations:
    - `CellValue`: `Number(f64)`, `Text(String)`, `Boolean(bool)`, `Empty`, `Error(ExcelError)`.
    - `CellValue::excel_cmp` and `CellValue::excel_eq`: Single source of truth for case-insensitive ASCII string comparisons and Excel type-ranked ordering.
    - `ExcelError`: Domain calculation error variants (`#DIV/0!`, `#VALUE!`, `#REF!`, `#NUM!`, `#N/A`, `#NAME?`).
  - `finx::functions::traits`: Function execution contracts:
    - `ExcelFunction`: Core trait defining function metadata, parameter definitions, arity rules, and evaluation mechanics.
    - `FunctionCategory`: Functional taxonomy (`Math`, `Logical`, `Financial`, `Lookup`, `DateTime`, `Text`, `Information`).
    - `ParamKind`: Positional evaluation contracts (`Scalar`, `RangeOrArray`, `Lazy`).
    - `VariadicRule`: Tail repetition rules (`None`, `Single`, `Pairs`).
    - `ParameterDef`: Positional metadata with optionality and default values.
    - `FunctionArg`: Bound runtime argument (`Scalar`, `Array`, `Lazy`, `Omitted`).
    - `EvalContext`: Abstraction decoupling function evaluation from internal compiler topological state.
  - `finx::functions::registry`: Deterministic `IndexMap`-backed registry with uppercase lookup, static call validation, and argument binding (`bind_arguments`), including cyclical pair resolution for `VariadicRule::Pairs`.
  - `finx::functions::common`: Shared argument traversal and branch evaluation helpers (`NumericCollector`, `LogicalCollector`, `eval_branch_arg`, `count_numbers`, `count_non_empty`) and IEEE-754 decimal rounding routines.
  - `finx::functions::error`: Structured `FunctionError` diagnostic variants with 1-indexed argument positions.
  - `finx::functions::math`: Built-in mathematical and aggregation functions (`ABS`, `SUM`, `AVERAGE`, `COUNT`, `COUNTA`, `MAX`, `MIN`, `ROUND`, `ROUNDUP`, `ROUNDDOWN`, `SUMPRODUCT`).
  - `finx::functions::logical`: Built-in logical functions (`IF`, `IFS`, `IFERROR`, `IFNA`, `AND`, `OR`, `NOT`, `CHOOSE`, `SWITCH`).
  - `finx::functions::financial`: Built-in financial and discounting functions (`NPV`, `XNPV`, `IRR`, `XIRR`, `PMT`, `IPMT`, `PPMT`).
- **Data flow & interfaces**:
  1. AST parsing (Pass 1) $\rightarrow$ `FunctionRegistry::validate_call` validates identifier existence, arity bounds, variadic pair counts, and non-omittable required arguments.
  2. Evaluator execution (Pass 4) $\rightarrow$ `FunctionRegistry::bind_arguments` pre-evaluates scalar coordinates and multi-dimensional range slices while wrapping lazy branches $\rightarrow$ `ExcelFunction::evaluate`.
  3. Formula lowering (Pass 6) $\rightarrow$ `FunctionRegistry::lookup` queries canonical uppercase names for formula string emission.
- **Dependencies**:
  - `thiserror`: Structured domain and compiler diagnostic errors.
  - `indexmap`: Deterministic function registration and iteration ordering.
  - `serde`: Serialization of values and categories.

## Requirements & Invariants

### 1. Strongly Typed Runtime Values & Parity Display

Calculated values and table inputs are encapsulated in `CellValue` with standard Excel string displays and explicit coercion helpers.

#### Scenarios

- **Error and boolean formatting**:
  - **When** formatting `ExcelError::Div0` or `CellValue::Boolean(true)` via `Display`
  - **Then** prints `"#DIV/0!"` and `"TRUE"`.
- **Empty cell display**:
  - **When** formatting `CellValue::Empty` via `Display`
  - **Then** emits an empty string `""`.
- **AST literal conversion**:
  - **When** converting `ValueLiteral` or `Literal` into `CellValue`
  - **Then** numbers, strings, and booleans map to their corresponding variants, and `ValueLiteral::Null` maps to `CellValue::Empty`.

### 2. Registry Validation & Deterministic Ordering

Function dispatch is case-insensitive, thread-safe, and deterministic. Function calls are validated against arity bounds, variadic constraints, and argument omission rules.

#### Scenarios

- **Case-insensitive lookup**:
  - **When** looking up `"abs"`, `"Abs"`, or `"ABS"`
  - **Then** resolves to `AbsFunction`.
- **Deterministic iteration**:
  - **When** iterating through `FunctionRegistry::standard()`
  - **Then** yields registered functions strictly in registration order (`["ABS", "SUM", "AVERAGE", "COUNT", "COUNTA", "MAX", "MIN", "ROUND", "ROUNDUP", "ROUNDDOWN", "SUMPRODUCT", "IF", "IFS", "IFERROR", "IFNA", "AND", "OR", "NOT", "CHOOSE", "SWITCH", "NPV", "XNPV", "IRR", "XIRR", "PMT", "IPMT", "PPMT"]`).
- **Arity validation**:
  - **When** validating `ABS` with 0 or 2 arguments
  - **Then** rejects with `FunctionError::TooFewArguments` or `FunctionError::TooManyArguments`.
- **Required argument omission rejection**:
  - **When** validating `ABS(_)` or `IF(_, 10, 20)`
  - **Then** rejects with `FunctionError::CannotOmitRequiredArgument` specifying the parameter name and 1-indexed position.
- **Variadic pairs validation**:
  - **When** validating `IFS(1=1)` or `IFS(1=1, 2, 3)`
  - **Then** rejects with `FunctionError::InvalidVariadicPairs`.

### 3. Argument Binding & Omission Normalization

Positional arguments are bound according to parameter metadata. Trailing missing arguments and explicit omitted placeholders (`_`) are unified. Trailing variadic arguments alternate across repeating parameter definitions when `VariadicRule::Pairs` is enabled.

#### Scenarios

- **Default value fallback**:
  - **When** binding `IF(test, val_true)` or `IF(test, val_true, _)`
  - **Then** the third argument binds to `FunctionArg::Scalar(CellValue::Boolean(false))`.
- **Range reference binding**:
  - **When** binding a table coordinate range (e.g. `Table["r1".."r3", @col]`) to a `ParamKind::RangeOrArray` parameter
  - **Then** binds as `FunctionArg::Array` with `is_range_ref: true`.
- **Variadic pairs alternation**:
  - **When** binding arguments 3 and 4 in `IFS(c1, v1, c2, v2)`
  - **Then** argument 3 binds to parameter definition 1 (`logical_test`) and argument 4 binds to parameter definition 2 (`value_if_true`).

### 4. Function Execution Semantics

Standard functions demonstrate distinct evaluation models.

#### Scenarios

- **Unary scalar (`ABS`)**:
  - **When** evaluated with `-42.5`, `Empty`, or `TRUE`
  - **Then** produces `42.5`, `0.0`, and `1.0`. Text inputs produce `#VALUE!`, and existing domain errors propagate.
- **Variadic arithmetic aggregators (`SUM`, `AVERAGE`, `MIN`, `MAX`)**:
  - **When** evaluated with a range reference containing numbers, text, booleans, and empty cells
  - **Then** text, booleans, and empty cells are ignored, numbers are aggregated, and cell errors immediately bubble up. Explicit scalar text produces `#VALUE!`.
  - **When** evaluating `AVERAGE` over empty ranges, returns `#DIV/0!`.
  - **When** evaluating `MAX`/`MIN` over empty ranges, returns `0.0`.
- **Numeric count (`COUNT`)**:
  - **When** evaluated with ranges or scalars
  - **Then** counts numbers and direct booleans; text, empty cells, and range errors are ignored (+0) without error propagation (`COUNT("hello")` returns `0.0`).
- **Non-empty count (`COUNTA`)**:
  - **When** evaluated with ranges or scalars
  - **Then** counts all non-empty cells (numbers, text, booleans, errors), ignoring only `CellValue::Empty`.
- **Decimal rounding (`ROUND`, `ROUNDUP`, `ROUNDDOWN`)**:
  - **When** evaluated with `number` and `num_digits`
  - **Then** `ROUND` rounds half away from zero, `ROUNDUP` rounds away from zero, and `ROUNDDOWN` truncates toward zero. Negative `num_digits` rounds left of the decimal point.
- **Vector product sum (`SUMPRODUCT`)**:
  - **When** evaluated with matching array/range dimensions
  - **Then** multiplies corresponding elements across ranges and sums products; non-numeric entries count as `0.0`, cell errors bubble immediately, and mismatched dimensions return `#VALUE!`.
- **Logical conjunction & disjunction (`AND`, `OR`)**:
  - **When** evaluated across scalars and ranges
  - **Then** numbers (`0.0` = false, non-zero = true), booleans, and valid text (`"TRUE"`/`"FALSE"`) are evaluated; range text and empty cells are ignored; cell errors bubble immediately; returns `#VALUE!` if no logical values are present.
- **Logical inversion (`NOT`)**:
  - **When** evaluated with boolean, numeric, or boolean string scalar (`"TRUE"`/`"FALSE"`)
  - **Then** inverts truth value; errors bubble immediately; non-convertible text returns `#VALUE!`.
- **Error handling fallbacks (`IFERROR`, `IFNA`)**:
  - **When** evaluated with primary expression and fallback
  - **Then** `IFERROR` returns primary value without evaluating fallback unless primary evaluates to an error; `IFNA` returns fallback only on `#N/A`, propagating other errors without evaluating fallback.
- **Short-circuiting conditional branch (`IF`, `IFS`)**:
  - **When** evaluating conditions sequentially
  - **Then** only the selected branch expression is evaluated, and dormant branches are never executed; if no condition evaluates to true in `IFS`, returns `#N/A`.
- **Index selection (`CHOOSE`)**:
  - **When** evaluated with a 1-based index
  - **Then** selects and evaluates strictly the matching choice expression; out-of-bounds indices return `#VALUE!`.
- **Case dispatch (`SWITCH`)**:
  - **When** evaluated with a target expression and case-result pairs
  - **Then** compares cases using Excel case-insensitive equality; evaluates and returns matching result; falls back to optional trailing default or `#N/A`.
- **Periodic & non-periodic discounting (`NPV`, `XNPV`)**:
  - **When** evaluated with periodic cash flows (`NPV`)
  - **Then** discounts sequentially starting at $t=1$, ignores text and blank cells in ranges, and evaluates $r = -1.0$ as `#DIV/0!`.
  - **When** evaluated with non-periodic cash flows and dates (`XNPV`)
  - **Then** applies Actual/365 year fractions from initial date $d_1$; validates equal array lengths, monotonic dates ($d_i \ge d_1$), and discount rate $r > -1.0$ (returning `#NUM!` otherwise).
- **Internal rate of return (`IRR`, `XIRR`)**:
  - **When** evaluated with periodic (`IRR`) or date-scheduled (`XIRR`) cash flows
  - **Then** solves for discount rate where net present value equals zero via Newton-Raphson iteration; enforces at least one positive and one negative cash flow, returning `#NUM!` otherwise.
- **Periodic debt service & amortization (`PMT`, `IPMT`, `PPMT`)**:
  - **When** evaluated with loan parameters (`rate`, `nper`, `pv`, `[fv]`, `[type]`)
  - **Then** computes constant periodic payment (`PMT`), interest component (`IPMT`), and principal component (`PPMT`) across periods $1 \le per \le nper$; strictly satisfies $\text{PMT} = \text{IPMT} + \text{PPMT}$; supports period-end (`type = 0`) and period-start (`type = 1`) timing.

