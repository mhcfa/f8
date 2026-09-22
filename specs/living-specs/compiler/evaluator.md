---
title: In-Memory Topological Evaluator & Formula Execution
last_updated: 2026-09-18
domain: compiler
status: active
---

## Purpose

The `compiler::eval` subsystem provides formula expression evaluation with Excel calculation parity, 2D range extraction, in-memory topological model calculation, and the precalculated model matrix (`EvaluatedModel`). It executes compiled cell calculation nodes across the cell dependency DAG, isolating runtime domain errors without crashing compilation and supplying self-contained evaluated model data to downstream layout and emission passes.

## Architecture & Boundaries

- **Modules & components**:
  - `f8_core::compiler::eval::expr`: Formula AST expression evaluator (`eval_expr`), IEEE 754 arithmetic normalization, Excel type-aware comparison ranking, and domain error bubbling.
  - `f8_core::compiler::eval::context`: `CellEvalContext` bridging cell coordinates, memoized values, symbol table, and function registry to implement `EvalContext` for functions (`eval_expr`, `eval_range`).
  - `f8_core::compiler::eval::model`: `EvaluatedModel` self-contained matrix combining computed `CellValue` entries with `SymbolTable`, providing symbol-integrated query APIs (`get_value`, `get_cell`, `get_row_values`, `find_errors`).
  - `f8_core::compiler::eval::error`: Structured compiler and evaluation diagnostic hierarchy (`EvalError`).
  - `f8_core::compiler::eval::Evaluator`: Topological model execution engine iterating [`DependencyGraph::topological_order`](./dependency-graph.md).
  - `f8_core::compiler::graph::resolve::ResolvedSlice`: 2D slice metadata `(rows, cols, cells)` preserved during range reference resolution.
- **Data flow & interfaces**:
  1. `CellGrid` + `DependencyGraph` + `SymbolTable` + `FunctionRegistry` $\rightarrow$ `Evaluator::evaluate(&grid, &graph, &symbol_table, &registry)`.
  2. For each cell in `graph.topological_order()`:
     - Constant literals convert directly to `CellValue`.
     - Formula expressions evaluate via `eval_expr` using accumulated values in preceding cells.
     - Extensible functions (`SUM`, `IF`, `ABS`) dispatch through [`FunctionRegistry`](../functions/registry.md) using `CellEvalContext`.
  3. Returns `EvaluatedModel` holding all calculated values and model metadata, consumed by Pass 5 (Layout) and Pass 6 (Emission).
- **Dependencies**:
  - `indexmap`: Guarantees deterministic iteration ordering for reproducible compilation.
  - `thiserror`: Structured diagnostic error typing.
  - `f8_core::value`: `CellValue` and `ExcelError` runtime representations.
  - `f8_core::expr::ast`: AST expressions (`Expr`, `Literal`).

## Requirements & Invariants

### 1. Excel Arithmetic & Float Parity

Formula expression operators (`+`, `-`, `*`, `/`, `^`) adhere strictly to Excel calculation semantics:
- Unary negation `-` coerces operands to numbers and normalizes signed zero `-0.0` to `0.0`.
- Empty cells coerce to `0.0` in arithmetic operations.
- Boolean values coerce to `1.0` (`TRUE`) and `0.0` (`FALSE`) in arithmetic.
- Non-numeric text operands fail with `CellValue::Error(ExcelError::Value)`.
- Division by zero (`x / 0.0` or `x / Empty`) evaluates to `CellValue::Error(ExcelError::Div0)`.
- Power anomalies: `0.0 ^ negative` produces `#DIV/0!`; fractional power of negative numbers or `NaN`/infinity produces `#NUM!`.
- Negative floating point zero (`-0.0`) is always normalized to positive `0.0`.

#### Scenarios

- **Zero and negative float normalization**:
  - **When** evaluating `-0.0`, `0.0 * -1.0`, or `- (1.0 - 1.0)`
  - **Then** evaluates to `CellValue::Number(0.0)`.
- **Division by zero and blank cell**:
  - **When** evaluating `10 / 0` or `10 / Empty`
  - **Then** evaluates to `CellValue::Error(ExcelError::Div0)`.
- **Arithmetic coercion of text**:
  - **When** evaluating `10 + "hello"`
  - **Then** evaluates to `CellValue::Error(ExcelError::Value)`.

### 2. Excel Comparison & Type Ranking

Comparison operators (`=`, `<>`, `<`, `<=`, `>`, `>=`) evaluate to `CellValue::Boolean` conforming to Excel comparison type hierarchy:
- Mixed-type ranking order: `Number < Text < Boolean`.
- String comparisons are case-insensitive via ASCII case-folding (`"Apple" = "apple"` is `TRUE`).
- Empty cell comparisons:
  - Against numbers: `Empty` coerces to `0.0`.
  - Against text: `Empty` coerces to `""`.
  - Against booleans: `Empty == FALSE` evaluates to `FALSE` (matching Excel rules).
- Any comparison involving `CellValue::Error` bubbles that error immediately.

#### Scenarios

- **Mixed-type comparisons**:
  - **When** evaluating `100 < "2"`, `"text" < TRUE`, and `999 < TRUE`
  - **Then** all evaluate to `CellValue::Boolean(true)`.
- **Case-insensitive strings**:
  - **When** evaluating `"Gross Profit" = "gross profit"`
  - **Then** evaluates to `CellValue::Boolean(true)`.

### 3. Context Bridge & Lazy Branch Isolation

Functions implementing `ExcelFunction` receive `EvalContext` implemented by `CellEvalContext`:
- `eval_expr` evaluates sub-expressions on demand against accumulated values.
- `eval_range` extracts 2D rectangular slices preserving `(rows, cols)` shape metadata.
- Conditional branches in lazy functions (e.g. `IF`) evaluate only the active branch; dormant branches are never executed.
- Standalone omitted placeholder `_` in expression arithmetic is rejected with `EvalError::UnexpectedOmitted`.

#### Scenarios

- **Short-circuit lazy execution**:
  - **When** evaluating `IF(TRUE, 100, 1 / 0)`
  - **Then** evaluates to `100.0` and the division-by-zero false branch is never executed.
- **Range shape extraction**:
  - **When** evaluating `SUM(Table["r1".."r3", @col])`
  - **Then** `eval_range` yields `(3, 1, values)` and `SUM` filters out empty and text cells.

### 4. Deterministic Topological Evaluation

`Evaluator::evaluate` iterates cells strictly according to `DependencyGraph::topological_order()`.
- Guaranteed prerequisite calculation: all predecessor cells referenced in a formula are guaranteed to be evaluated before the dependent cell executes.
- Domain error persistence: cells calculating to Excel runtime errors (`#DIV/0!`, `#VALUE!`, `#NUM!`) are stored in `EvaluatedModel` without halting compilation, allowing downstream dependencies to bubble or handle them.
- Compiler error boundaries: internal resolution failures, missing cells, or unexpected syntax anomalies halt evaluation and return structured `EvalError`.

#### Scenarios

- **Multi-table cross-reference propagation**:
  - **When** evaluating a multi-table model with inter-table references and relative column steps (`col.prev(1)`)
  - **Then** calculations execute in exact topological DAG order, propagating values across periods and tables.
- **Missing cell in grid guard**:
  - **When** a topological dependency node is absent from `CellGrid`
  - **Then** returns `Err(EvalError::CellNotFound)`.

### 5. Evaluated Model Query APIs & Column Declaration Order

`EvaluatedModel` links the computed value map with the `SymbolTable`:
- `get_value(&self, table, row, col)`: direct coordinate lookup.
- `get_cell(&self, id)`: 3-tuple `CellId` lookup.
- `get_row_values(&self, table, row)`: returns `IndexMap<ColId, CellValue>` strictly in table declared column order (`table_meta.columns`), regardless of DAG calculation order.
- Validates table and row existence, returning `EvalError::TableNotFound` or `EvalError::RowNotFound`.
- `find_errors(&self)`: inspects all cells and isolates every occurrence of `ExcelError`.

#### Scenarios

- **Declared column ordering in row query**:
  - **When** calling `get_row_values` on a table with declared columns `["2021A", "2022A", "2023A"]`
  - **Then** keys in the returned `IndexMap` match that exact sequence.
- **Domain error isolation**:
  - **When** calling `find_errors` on a model containing `#DIV/0!` and `#VALUE!` cells
  - **Then** returns a vector of all failing `(&CellId, &ExcelError)` references.

### 6. Unified Compiler Driver Facade (`Compiler`)

The top-level `f8_core::Compiler` orchestrates Passes 1 through 4 into a single coherent call:
- `Compiler::evaluate(manifest, tables)`: Validates manifest and table specifications, expands the cell grid, constructs the dependency DAG, verifies zero cycles, and evaluates all formulas topologically using `FunctionRegistry::standard()`.
- `Compiler::evaluate_with_registry(manifest, tables, registry)`: Permits custom function registry injection.
- Returns unified `CompilerError` wrapping `ModelError` (Pass 1), `IrError` (Pass 2), `GraphError` (Pass 3), and `EvalError` (Pass 4).

#### Scenarios

- **Single-call workbook compilation**:
  - **When** calling `Compiler::evaluate(&manifest, &tables)` on valid specifications
  - **Then** returns an `EvaluatedModel` with all calculated cell values populated in topological DAG order.
- **Pass error bubbling**:
  - **When** calling `Compiler::evaluate` on a specification with a cycle or duplicate sheet name
  - **Then** returns the appropriate `CompilerError` variant (`Model`, `Ir`, `Graph`, or `Eval`).

### 7. Anchoring Model Benchmarks

The evaluator is validated against three realistic end-to-end financial model benchmarks:
1. **Institutional LBO Model** (`tests/lbo_model_test.rs`): Canonical anchoring benchmark across 2 worksheets, 7 tables, and 6 timeline columns (`y0` close to `y5` exit). Exercises dynamic scenario selection (`XLOOKUP`), calendar timeline generation (`DATE`, `EDATE`, `YEAR`, `DAYS`, `YEARFRAC`), operating forecast with tax guard (`IF`), multi-tranche debt waterfall with mandatory amortization and cash sweep prepayments (`MIN`, `MAX`), and non-periodic sponsor returns (`XIRR`, `XNPV`).
2. **3-Statement Financial Model** (`tests/three_statement_model_test.rs`): Multi-sheet financial model (Income Statement, Balance Sheet, Cash Flow, Assumptions) with temporal predecessor offsets (`col.prev`) and accounting identities.
3. **Orthogonal Column-Based Cap Table** (`tests/cap_table_test.rs`): Financing rounds/shareholders as entity rows and calculation metrics as columns, exercising column vector formulas (`col.expr`) and active row selectors (`@row`).

#### Scenarios

- **LBO Base Case and sensitivity evaluation**:
  - **When** evaluating the institutional LBO model under Base (`active_case = "base"`), Bull, and Bear cases
  - **Then** evaluates with zero errors, confirms 2028 leap-year day count (`DAYS = 366`), executes senior debt cash sweeps, and computes exact `XIRR` and `XNPV` matching Excel.

