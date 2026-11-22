---
name: f8-modeling
description: Author, audit, and compile financial models using the f8 Python SDK with 2D coordinates, vector expressions, TableAuditor, and live OpenXML Excel emission.
---

# f8 Financial Modeling Guide

Financial modeling compiler, evaluator, and Python DSL bridging programmatic models and spreadsheets. Compiles 2D tabular specifications into a validated DAG, evaluates in memory, and emits OpenXML (`.xlsx`) workbooks with live formulas.

## Mental Model

- Hierarchy: `Model` -> `Sheet` -> `Table` -> `Column` / `Row` -> `Cell`.
- Vector expansion: Horizontal `row.expr` broadcasts across `measure` columns; vertical `col.expr` on a `measure` column broadcasts down data rows.
- 3-tier specificity lattice:
  1. 0D cell override (`cells={...}`, `set_cell`, `set_cells`) takes highest precedence.
  2. 1D vector rules (`row` XOR `col`): Intersecting row and column `expr` (or differing row and column `format`, `decimals`, `currency`, or non-`"none"` `shade`) without a 0D cell override raise `SpecificationError`.
  3. Descriptor isolation & unassigned check: Vector formulas and row-level numeric formats never broadcast into `role="descriptor"` columns. Every data cell (`descriptor` or `measure`) must resolve to a value or formula, else `SpecificationError` (`unassigned cell`).
- In-memory evaluation: Instant DAG cycle detection, topological calculation, and tabular inspection prior to disk serialization.
- Live Excel emission: Lowers 2D coordinates to native A1/range formulas and emits styled OpenXML (`.xlsx`).

## Workflow & CLI

1. Define: Construct `Model(name=..., theme=...)` -> `model.add_sheet(...)` -> `sheet.add_table(...)` -> `table.add_column(...)` / `table.add_row(...)`.
2. Compile: Call `compiled = model.compile()` (or `f8.compile(manifest, tables)`) to validate schema, expand grid, check cycles, evaluate cells, plan layout, and lower formulas.
3. Inspect: Query values, formulas, and dependency trees via `compiled.table(id)` (`TableAuditor`), `compiled.value(...)`, `compiled.num(...)`, `compiled.formatted(...)`, `compiled.inspect_cell(...)`, `compiled.precedents_of(...)`, and `compiled.dependents_of(...)`.
4. Diagnose: Catch typed `F8Error` subclasses (`CycleDetectedError`, `ReferenceResolutionError`, `FormulaParseError`, `SpecificationError`, `InspectionError`, `EmissionError`) and inspect structured attributes.
5. Emit: Write `.xlsx` to disk via `compiled.save(path, options=...)` or in-memory bytes via `compiled.emit_workbook(options=...)`.

CLI commands (`f8` or `python -m f8`):

- `f8 guide [syntax|functions|audit|all|--list]`: Stream reference documentation to stdout.
- `f8 agent init [--dest DIR] [-f|--force]`: Scaffold `.agents/skills/f8-modeling` into workspace.
- `f8 theme init [-f|--force]`: Write default `corporate_navy` theme specification to `theme.json`.

## Core Invariants

- Explicit 2D coordinates: All references require bare table identifier or `@table` and double-quoted keys (`Table["row", @col]`, `@table["row", @col]`, `Table["row", col.prev]`, `Table["r1".."r2", @col]`), positive integer `prev(k)`/`next(k)` offsets (`k >= 1`), and contiguous `start..end` slices (ranges cannot cross `Gutter`s, `SectionHeader`s, or horizontal `descriptor` columns). Unquoted keys, quoted table names, bare `@`, or dot access (`Table.row`) raise `FormulaParseError`.
- Uppercase identifiers: Built-in functions (`SUM`, `IF`, `XLOOKUP`, `ISERROR`) and booleans (`TRUE`, `FALSE`) must be uppercase; dotted function names (`ERROR.TYPE`) raise `FormulaParseError`.
- Argument omission: Use `_` only for intermediate optional function parameters followed by a later explicit argument (`PMT(rate, nper, pv, _, type)`); omit trailing optional parameters directly (`PMT(rate, nper, pv)`, not `PMT(rate, nper, pv, _)`).
- Cell-level indentation: `indent` applies only to `role="descriptor"` cells (`Cell("Label", indent=1)`) and section headers; setting `indent > 0` on a `measure` cell raises `SpecificationError`.

## Strict Formula Rules (Excel Divergences)

`f8` rejects permissive Excel coercions and structural footguns at compile time (`F8Error`), then lowers valid ASTs into native Excel syntax during workbook emission:

- Operators & comparisons: Use `=` and `<>` (no `==`, `!=`, `&` concatenation, or prefix `+`). Relational operators (`=`, `<>`, `<`, `<=`, `>`, `>=`) are non-associative (parenthesize `(a = b) = c`) and require matching non-`Empty` operand types (`Number`, `Text`, or `Boolean`).
- Scalars, blanks & cash flows: Arithmetic, scalar parameters, `1x1` single-cell references, evaluated branch arms (`IF`, `CHOOSE`, `IFERROR`, `XLOOKUP` `if_not_found`), selected lookup/index elements (`XLOOKUP` `return_array`, `INDEX` `array`), and root formula results reject `Text`, `Boolean` (where `Number` is expected), and `Empty` (`None`). Only syntactic `start..end` range slices in math/logical aggregations (`SUM`, `AVERAGE`, `MIN`, `MAX`, `MEDIAN`, `COUNT`, `SUMPRODUCT`, `AND`, `OR`) and unselected elements in `XLOOKUP`/`XMATCH`/`INDEX` range slices tolerate `Empty`; `ISBLANK` and `COUNTA` are omitted (`UnknownFunction`). Numeric aggregations (`SUM`, `AVERAGE`, `MIN`, `MAX`, `MEDIAN`, `COUNT`, `SUMPRODUCT`) reject `descriptor` columns; `NPV`, `IRR`, `XNPV`, and `XIRR` reject `Empty` across cash-flow and date slices (use `0.0`).
- Monomorphic branches & error traps: `IF(cond, t, f)` requires 3 arguments and `Boolean` conditions (`IF(rev <> 0, ...)`, not `IF(rev, ...)`). Result arms across `IF`, `CHOOSE`, `IFERROR`, and `XLOOKUP` must unify to one scalar type and evaluate to non-`Empty` values—use `NA()` for suppressed ratios (`IF(ebitda > 0, ev / ebitda, NA())`), not `"NM"` text; `IFS`, `SWITCH`, `IFNA`, and `ISNA` are omitted (`IF`, `CHOOSE`, `IFERROR`, `ISERROR`).
- Conditional aggregation & debt amortization: `SUMIFS`, `COUNTIFS`, `AVERAGEIFS`, `MINIFS`, `MAXIFS`, `IPMT`, and `PPMT` are omitted (`UnknownFunction`). Express conditional aggregations explicitly via `IF` helper rows/columns (`IF(cond, val, 0)` + `SUM`, `IF(cond, 1, 0)` flag + `SUM`/`SUMPRODUCT`, `SUMPRODUCT(vals, flags) / SUM(flags)` for conditional averages, or 0D `None` overrides in helper slices for `MIN`/`MAX` exclusions), and express period-by-period debt interest and principal via `PMT` and a 2D corkscrew schedule (`interest = opening_balance * rate`, `principal = scheduled_pmt - interest`).
- Lookups & calendar dates: Range slots require matching-shape `Table[...]` references and homogeneous element types; `XLOOKUP`/`XMATCH` reject `Empty` lookup keys and `1x1` single-cell `Empty` arrays (`F8Error`), while invalid mode codes (including wildcard mode `2`), `Empty` or unsorted binary-search (`2`/`-2`) keys (`#VALUE!`), `Error` cells in binary-search arrays (bubbled `ExcelError`), and out-of-bounds `CHOOSE`/`INDEX` indices return runtime Excel errors (`#VALUE!`, `#REF!`, `#NUM!`). `DATE(yyyy, m, d)` requires 4-digit years (`1900..=9999`) and valid calendar days without month/day rollover (use `EDATE` or `EOMONTH` for month stepping), input date serials require `1..=2958465` (violations return `#NUM!`), and `DAYS` is omitted (subtract date serials `end_date - start_date`).

## Formula Clarity & Decomposition

`f8` compiles source expressions into live Excel formulas audited by humans in spreadsheets. Keep each cell to one clear logical step:

- Decompose in the grid, not in code strings: Local variables or helper functions that assemble and interpolate `expr` subexpressions (`y_half = ...`, `pv_tv = ...`) only tidy Python—they inline and duplicate every piece into one unreadable Excel cell. Promote intermediate steps and repeated subexpressions into explicit helper rows, columns, or supporting tables instead.
- Prefer range primitives over unrolled loops: Avoid generating `a + b + c + ...` chains across periods or items via string joins (e.g. `" + ".join(...)`). Use range functions (`SUM`, `SUMPRODUCT`, `NPV`, `XNPV`) and `@row`/`@col` vector rules; when elements need per-period transforms (such as custom discount factors or conditional weights), compute them in a helper row, column, or matrix and aggregate with `SUM` or `SUMPRODUCT`.
- Factor sensitivities without Data Tables: In 1D/2D sensitivity grids, move terms that vary along only one axis (or neither axis) into 1D helper rows/columns or a helper table so output cells remain concise. For multi-step, path-dependent sensitivities, build explicit scenario schedules rather than inlining a model into one cell.
- Inspect lowered formulas during audit: Numeric checks pass even when formulas are bloated—spot-check complex and sensitivity tables with `to_markdown(view="formulas")`.

## Reference Index

- `references/syntax-and-grid.md`: Container constructors (`Model`, `Sheet`, `Table`, `Cell`), lookup/mutation APIs (`get_sheet`, `get_table`, `set_cell`, `set_cells`), 2D coordinate grammar, 3-tier resolution rules, layout placement, formatting presets, and themes.
- `references/functions.md`: 37 built-in Excel functions, compile-time type/shape validation, signatures, parameter defaults, conditional aggregation and debt amortization composition patterns, lookup modes, `YEARFRAC` day-count conventions, and domain error rules.
- `references/auditing-and-repair.md`: `CompiledModel` and `TableAuditor` APIs, `MarkdownTable` and dictionary/DataFrame exports, `TypedDict` inspection schemas, and `F8Error` exception attributes.
