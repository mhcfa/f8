---
title: Excel Function Registry & Core Archetypes
last_updated: 2026-09-18
domain: functions
status: active
---

## Purpose

The `functions` and `value` subsystems provide the runtime calculation value model (`CellValue`), domain error hierarchy (`ExcelError`), extensible Excel function registry architecture (`ExcelFunction`, `FunctionRegistry`), and canonical core function archetypes (`ABS`, `SUM`, `IF`). Together, they form the single source of truth for function signature validation, argument binding, scalar coercion, range aggregation filtering, and lazy branch evaluation across compiler passes.

## Architecture & Boundaries

- **Modules & components**:
  - `finx::value`: Strongly typed runtime cell representations:
    - `CellValue`: `Number(f64)`, `Text(String)`, `Boolean(bool)`, `Empty`, `Error(ExcelError)`.
    - `ExcelError`: Domain calculation error variants (`#DIV/0!`, `#VALUE!`, `#REF!`, `#NUM!`, `#N/A`, `#NAME?`).
  - `finx::functions::traits`: Function execution contracts:
    - `ExcelFunction`: Core trait defining function metadata, parameter definitions, arity rules, and evaluation mechanics.
    - `FunctionCategory`: Functional taxonomy (`Math`, `Logical`, `Financial`, `Lookup`, `DateTime`, `Text`, `Information`).
    - `ParamKind`: Positional evaluation contracts (`Scalar`, `RangeOrArray`, `Lazy`).
    - `VariadicRule`: Tail repetition rules (`None`, `Single`, `Pairs`).
    - `ParameterDef`: Positional metadata with optionality and default values.
    - `FunctionArg`: Bound runtime argument (`Scalar`, `Array`, `Lazy`, `Omitted`).
    - `EvalContext`: Abstraction decoupling function evaluation from internal compiler topological state.
  - `finx::functions::registry`: Deterministic `IndexMap`-backed registry with uppercase lookup, static call validation, and argument binding (`bind_arguments`).
  - `finx::functions::error`: Structured `FunctionError` diagnostic variants with 1-indexed argument positions.
  - `finx::functions::math`: Built-in mathematical functions (`ABS`, `SUM`).
  - `finx::functions::logical`: Built-in logical branching functions (`IF`).
- **Data flow & interfaces**:
  1. AST parsing (Pass 1) $\rightarrow$ `FunctionRegistry::validate_call` validates identifier existence, arity bounds, and non-omittable required arguments.
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
  - **Then** yields registered functions strictly in registration order (`["ABS", "SUM", "IF"]`).
- **Arity validation**:
  - **When** validating `ABS` with 0 or 2 arguments
  - **Then** rejects with `FunctionError::TooFewArguments` or `FunctionError::TooManyArguments`.
- **Required argument omission rejection**:
  - **When** validating `ABS(_)` or `IF(_, 10, 20)`
  - **Then** rejects with `FunctionError::CannotOmitRequiredArgument` specifying the parameter name and 1-indexed position.

### 3. Argument Binding & Omission Normalization

Positional arguments are bound according to parameter metadata. Trailing missing arguments and explicit omitted placeholders (`_`) are unified.

#### Scenarios

- **Default value fallback**:
  - **When** binding `IF(test, val_true)` or `IF(test, val_true, _)`
  - **Then** the third argument binds to `FunctionArg::Scalar(CellValue::Boolean(false))`.
- **Range reference binding**:
  - **When** binding a table coordinate range (e.g. `Table["r1".."r3", @col]`) to a `ParamKind::RangeOrArray` parameter
  - **Then** binds as `FunctionArg::Array` with `is_range_ref: true`.

### 4. Core Archetype Execution Semantics

Canonical functions demonstrate distinct evaluation models.

#### Scenarios

- **Unary scalar (`ABS`)**:
  - **When** evaluated with `-42.5`, `Empty`, or `TRUE`
  - **Then** produces `42.5`, `0.0`, and `1.0`. Text inputs produce `#VALUE!`, and existing domain errors propagate.
- **Variadic range aggregator (`SUM`)**:
  - **When** evaluated with a range reference containing numbers, text, booleans, and empty cells
  - **Then** text, booleans, and empty cells are ignored, numbers are aggregated, and cell errors immediately bubble up. Explicit scalar text produces `#VALUE!`.
- **Short-circuiting conditional branch (`IF`)**:
  - **When** evaluating `IF(TRUE, val_true, val_false)` or `IF(FALSE, val_true, val_false)`
  - **Then** only the selected branch expression is evaluated, and the dormant branch is never executed.
