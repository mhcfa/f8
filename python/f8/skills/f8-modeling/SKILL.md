---
name: f8-modeling
description: Author, audit, and compile financial models using the f8 Python SDK with 2D coordinates, vector expressions, band/lane layouts, TableAuditor, and OpenXML Excel emission. Use when building, inspecting, or debugging models with f8 in Python.
---

# f8 Financial Modeling Guide

Deterministic financial modeling compiler, in-memory evaluator, and Python DSL. Compiles 2D tabular specifications into a validated DAG, evaluates formulas in memory, and optionally emits OpenXML (`.xlsx`) workbooks with live Excel formulas.

## Mental Model

- Hierarchy: `Model` -> `Sheet` -> `Table` -> `Column` / `Row` -> `Cell`, with optional `Band` / `Lane` 2D sheet arrangement.
- Vector expansion: Horizontal `row.expr` broadcasts across `measure` columns; vertical `col.expr` on a `measure` column broadcasts down data rows.
- 3-tier specificity lattice: 0D cell override (`cells`, `set_cell`, `set_cells`) > 1D vector rule (`row` XOR `col`) > defaults. Intersecting `row` and `col` `expr` (or differing `format`, `decimals`, `currency`, or non-`"none"` `shade`) without a 0D override raises `SpecificationError`. `role="descriptor"` columns isolate vector formulas and row-level numeric formats; every data cell must resolve to a value or formula, else `SpecificationError` (see `references/syntax-and-grid.md`).
- In-memory evaluation & optional emission: Compiles, validates types/shapes/DAG acyclicity, and evaluates cells in memory before optional lowering to native Excel A1/range formulas and `.xlsx` emission.

## Workflow & CLI

1. Define: Build `Model(name=..., theme=...)` -> `model.add_sheet(...)` -> `sheet.add_table(...)` -> `table.add_column(...)` / `table.add_row(...)`, and optionally arrange multi-lane bands or custom gaps via `sheet.set_bands(...)`.
2. Compile: Call `compiled = model.compile()` (or `f8.compile(manifest, tables)`).
3. Inspect: Audit runtime errors via `compiled.errors()`, inspect workbook views via `compiled.to_markdown("values" | "index" | "layout")`, and query tables, cells, or dependency trees via `compiled.table(id)` (`TableAuditor`), `compiled.value(...)`, `compiled.num(...)`, `compiled.formatted(...)`, `compiled.inspect_cell(...)`, `compiled.precedents_of(...)`, and `compiled.dependents_of(...)` (see `references/auditing-and-repair.md`).
4. Diagnose: Catch `F8Error` subclasses (`CycleDetectedError`, `ReferenceResolutionError`, `FormulaParseError`, `SpecificationError`, `InspectionError`, `EmissionError`) and inspect their typed attributes.
5. Emit (optional): Write `.xlsx` to disk via `compiled.save(path, options=...)` or in-memory bytes via `compiled.emit_workbook(options=...)`.

CLI commands (`f8` or `python -m f8`):

- `f8 guide [syntax|functions|audit|all|--all|--list]`: Stream reference documentation to stdout.
- `f8 agent init [--dest DIR] [-f|--force]`: Scaffold `.agents/skills/f8-modeling` into workspace.
- `f8 theme init [-f|--force]`: Write default `corporate_navy` theme JSON to `theme.json`.

## Core Contracts & Excel Divergences

`f8` enforces an explicit formula grammar and strict static/runtime typing at compile time (`F8Error`), then lowers valid formulas into native Excel syntax:

- Explicit 2D coordinates: References require a bare table ID or `@table` and double-quoted keys (`Table["row", @col]`, `@table["row", @col]`, `Table["row", col.prev]`, `Table["r1".."r2", @col]`), positive integer `prev(k)`/`next(k)` offsets (`k >= 1`), and contiguous `start..end` slices (ranges cannot cross gutters, section headers, or horizontal `descriptor` columns). Unquoted keys, quoted table names, bare `@`, or dot access (`Table.row`) raise `FormulaParseError`.
- Identifiers, operators & argument omission: Built-in functions (`SUM`, `IF`, `XLOOKUP`, `ISERROR`) and booleans (`TRUE`, `FALSE`) must be uppercase; dotted function names, unary `+`, `&`, `==`, `!=`, and unparenthesized chained comparisons (`a = b = c`; parenthesize `(a = b) = c`) raise `FormulaParseError`. Use `_` only for intermediate optional arguments followed by a later explicit argument (`PMT(r, n, pv, _, 1)`); omit trailing optionals directly.
- Strict typing & empty cells: No implicit coercions between `Number`, `Text`, `Boolean`, and empty (`None`). Relational comparisons require matching non-empty operand types. Arithmetic, scalar arguments, `1x1` single-cell references, evaluated branch arms, selected lookup/index elements, and root formula results reject empty (`F8Error`). Only syntactic `start..end` slices in math/logical aggregations and unselected elements in `XLOOKUP`/`XMATCH`/`INDEX` tolerate sparse empty cells; `NPV`/`IRR`/`XNPV`/`XIRR` reject empty cells in cash-flow/date slices (zero cash flows require explicit `0.0`), and numeric/financial aggregations reject `descriptor` columns.
- Monomorphic branches & omitted functions: `IF(cond, t, f)` requires 3 arguments and a `Boolean` condition. Result arms across `IF`, `CHOOSE`, `IFERROR`, and `XLOOKUP` must unify to one scalar type (mixing types such as `Number` and `Text` fails compilation; `NA()` unifies with any scalar type). Omitted Excel functions (`ISBLANK`, `COUNTA`, `IFS`, `SWITCH`, `IFNA`, `ISNA`, `DAYS`, `SUMIFS`, `COUNTIFS`, `AVERAGEIFS`, `MINIFS`, `MAXIFS`, `IPMT`, `PPMT`, volatile/spill functions) fail compilation; see `references/functions.md` for composition patterns.
- Dates & lookups: `DATE(yyyy, m, d)` requires 4-digit years (`1900..=9999`) and valid calendar days without rollover (`EDATE`/`EOMONTH` for month stepping); input date serials require `1..=2958465` (else `#NUM!`). Wildcard `match_mode = 2` is unsupported (`#VALUE!`).
- Expression lowering behavior: Formula strings lower verbatim into Excel cells without algebraic factoring or loop re-rolling; host-assembled expressions (string interpolation or `" + ".join(...)`) lower as inlined single-cell formulas, while range functions over grid slices (`SUM`, `SUMPRODUCT`, `NPV`, `XNPV`) lower to Excel range formulas (e.g. `SUM(C5:G5)`).
- Cell indentation: `indent` applies only to `role="descriptor"` cells (`Cell("Label", indent=1)`) and section headers; `indent > 0` on a `measure` cell raises `SpecificationError`.

## Reference Index

- `references/syntax-and-grid.md`: Container constructors (`Model`, `Sheet`, `Band`, `Lane`, `Table`, `Cell`), lookup/mutation APIs (`get_sheet`, `get_table`, `set_bands`, `set_cell`, `set_cells`), 2D coordinate grammar, 3-tier resolution lattice, band/lane and `auto`/`grid` layouts, formatting presets, styles, shades, and themes.
- `references/functions.md`: 37 built-in Excel functions, compile-time type/shape rules, argument omission (`_`), composition patterns for omitted functions, lookup modes, `YEARFRAC` day-count conventions, and domain error rules.
- `references/auditing-and-repair.md`: `CompiledModel` and `TableAuditor` APIs, whole-workbook and table Markdown/dict exports, `f8.types` inspection schemas, and `F8Error` exception attributes.
