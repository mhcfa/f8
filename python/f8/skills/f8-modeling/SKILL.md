---
name: f8-modeling
description: >-
  Author, compile, audit, and export financial models with the f8 Python SDK.
  Use when building, inspecting, or debugging f8 models in Python.
---

# f8 Financial Modeling Guide

Deterministic financial model compiler, in-memory evaluator, and Python DSL. Compiles 2D tabular specs into a validated DAG, evaluates formulas in memory, and optionally lowers them to live `.xlsx` workbooks.

## Mental Model

- Hierarchy: `Model` -> `Sheet` -> `Table` -> `Column` / `Row` -> `Cell`, with optional `Band` / `Lane` 2D sheet placement.
- Vector expansion: `row.expr` broadcasts across `measure` columns; `col.expr` broadcasts down data rows. `role="descriptor"` columns hold labels and stay isolated from vector formulas and row formats.
- 3-tier specificity: 0D cell override (`cells`, `set_cell`, `set_cells`) > 1D vector rule (`row`/`col`; `expr` is strict XOR) > defaults. Conflicting `row`/`col` rules or unassigned cells raise `SpecificationError` (see `references/syntax-and-grid.md`).

## Workflow & CLI

1. Define: Build `Model` -> `add_sheet` -> `add_table` -> `add_column` / `add_row`; optionally set 2D bands/page breaks via `sheet.set_bands(...)`.
2. Compile: Call `compiled = model.compile()` (or `f8.compile(manifest, tables)`).
3. Inspect: Check runtime errors with `compiled.errors()`, overview via `compiled.to_markdown("values" | "index" | "layout")`, and drill in via `compiled.table(id)` (`TableAuditor`), `compiled.num(...)`, `compiled.inspect_cell(...)`, or `compiled.precedents_of(...)` / `dependents_of(...)`.
4. Diagnose: Catch `F8Error` subclasses (`CycleDetectedError`, `ReferenceResolutionError`, `FormulaParseError`, `SpecificationError`, `InspectionError`, `EmissionError`).
5. Emit (optional): Write `.xlsx` via `compiled.save(path, options=...)` or bytes via `compiled.emit_workbook(options=...)`.

CLI (`f8` or `python -m f8`):

- `f8 guide [syntax|layout|functions|audit|all|--all|--list]`: Stream reference docs to stdout.
- `f8 agent init [--dest DIR] [-f|--force]`: Scaffold `.agents/skills/f8-modeling` into workspace.
- `f8 theme init [-f|--force]`: Write default `corporate_navy` theme to `theme.json`.

## Core Contracts & Excel Divergences

- 2D coordinates: Use bare table ID or `@table` with double-quoted keys (`Table["row", @col]`, `@table["row", col.prev]`, `Table["r1".."r2", @col]`). No dot access, unquoted keys, or slices across gutters, section headers, or horizontal `descriptor` columns (`references/syntax-and-grid.md`).
- Strict grammar: Uppercase functions (`SUM`, `XLOOKUP`) and booleans (`TRUE`, `FALSE`); no unary `+`, `&`, `==`, `!=`, dotted function names, or unparenthesized chained comparisons (`a = b = c`). Use `_` only for intermediate omitted optionals (`PMT(r, n, pv, _, 1)`), never trailing.
- Strict DAG, typing & empties: Zero circular references (use closed-form algebra). No implicit coercions between `Number`, `Text`, `Boolean`, and `None`. Formulas reject empty cells except sparse `start..end` slices in math/logical aggregations and unselected lookup slice elements (`NPV`/`IRR`/`XNPV`/`XIRR` require explicit `0.0`); numeric/financial aggregations reject `descriptor` columns (`references/functions.md`).
- Monomorphic branches & omitted functions: 3-arg `IF(cond, t, f)` with `Boolean` condition; branch arms (`IF`, `CHOOSE`, `IFERROR`, `XLOOKUP`) must share one scalar type (`NA()` unifies with any type). Volatile/spill functions and `ISBLANK`, `COUNTA`, `IFS`, `SWITCH`, `IFNA`, `ISNA`, `DAYS`, `*IFS` (`SUMIFS`..`MAXIFS`), `IPMT`, `PPMT` are omitted (`references/functions.md` has composition patterns).
- Dates & lookups: `DATE(yyyy, m, d)` requires 4-digit years (`1900..=9999`) with no rollover (use `EDATE`/`EOMONTH`); `XLOOKUP`/`XMATCH` omit wildcard `match_mode = 2`.
- Verbatim lowering: Formulas lower verbatim without re-rolling; grid range functions (`SUM`, `SUMPRODUCT`, `NPV`) emit Excel ranges (`SUM(C5:G5)`), whereas expanded scalar sums lower term-by-term.

## Reference Index

Read on demand — do not load all files up front:

- `references/syntax-and-grid.md`: 2D coordinate grammar, `Model`/`Sheet`/`Table`/`Cell` builders, `add_column`/`add_row`/`set_cell`/`set_cells`, and 3-tier specificity lattice.
- `references/layout-and-styling.md`: Section headers/gutters, `Band`/`Lane`/`set_bands`, `auto`/`grid` modes, `PrintSetup`/`HeaderFooter`, format presets, styles, shades, currencies, and themes.
- `references/functions.md`: 37 built-in functions, type/empty/`_` omission rules, omitted-function composition patterns, lookup modes, `YEARFRAC` bases, and runtime errors.
- `references/auditing-and-repair.md`: `CompiledModel` and `TableAuditor` APIs, Markdown/dict exports, `f8.types` pointers, and `F8Error` hierarchy.
