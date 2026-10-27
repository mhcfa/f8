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
2. Compile: Call `compiled = model.compile()` (or `f8._core.compile(manifest, tables)`) to validate schema, expand grid, check cycles, evaluate cells, plan layout, and lower formulas.
3. Inspect: Query values, formulas, and dependency trees via `compiled.table(id)` (`TableAuditor`), `compiled.value(...)`, `compiled.num(...)`, `compiled.formatted(...)`, `compiled.inspect_cell(...)`, `compiled.precedents_of(...)`, and `compiled.dependents_of(...)`.
4. Diagnose: Catch typed `F8Error` subclasses (`CycleDetectedError`, `ReferenceResolutionError`, `FormulaParseError`, `SpecificationError`, `InspectionError`, `EmissionError`) and inspect structured attributes.
5. Emit: Write `.xlsx` to disk via `compiled.save(path, options=...)` or in-memory bytes via `compiled.emit_workbook(options=...)`.

CLI commands (`f8` or `python -m f8`):
- `f8 guide [syntax|functions|audit|all|--list]`: Stream reference documentation to stdout.
- `f8 agent init [--dest DIR] [-f|--force]`: Scaffold `.agents/skills/f8-modeling` into workspace.
- `f8 theme init [-f|--force]`: Write default `corporate_navy` theme specification to `theme.json`.

## Core Invariants

- Explicit 2D coordinates: All references require bare table identifier and double-quoted keys (`Table["row", @col]`, `Table["row", col.prev]`, `Table["r1".."r2", @col]`). Unquoted keys, quoted table names, bare `@`, or dot access (`Table.row`) raise `FormulaParseError`.
- Uppercase identifiers: Built-in functions (`SUM`, `IF`, `XLOOKUP`) and booleans (`TRUE`, `FALSE`) must be uppercase.
- Argument omission: Use `_` for omitted optional function parameters (`PMT(rate, nper, pv, _, type)`).
- Cell-level indentation: `indent` is restricted to `role="descriptor"` cells (`Cell("Label", indent=1)`) and section headers; setting `indent > 0` on a `measure` cell raises `SpecificationError`.

## Reference Index

- `references/syntax-and-grid.md`: Container constructors (`Model`, `Sheet`, `Table`, `Cell`), lookup/mutation APIs (`get_sheet`, `get_table`, `set_cell`, `set_cells`), 2D coordinate grammar, 3-tier resolution rules, layout placement, formatting presets, and themes.
- `references/functions.md`: 44 built-in Excel functions, signatures, parameter defaults, `*IFS` criteria grammar, lookup modes, `YEARFRAC` day-count conventions, and domain error rules.
- `references/auditing-and-repair.md`: `CompiledModel` and `TableAuditor` APIs, `MarkdownTable` and dictionary/DataFrame exports, `TypedDict` inspection schemas, and `F8Error` exception attributes.
