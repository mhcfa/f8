---
name: f8-modeling
description: Author, audit, and compile financial models using the f8 Python SDK with 2D coordinates, vector expressions, TableAuditor, and live OpenXML Excel emission.
---

# f8 Financial Modeling Guide

Financial modeling compiler, evaluator, and Python DSL bridging programmatic models and spreadsheets. Compiles tabular vectors into a validated DAG, audits in memory, and emits OpenXML (`.xlsx`) with live formulas.

## Mental Model

- Hierarchy: `Model` -> `Sheet` -> `Table` -> `Column` / `Row` -> `Cell`.
- Vector expansion: Horizontal `row.expr` broadcasts across measure columns; vertical `col.expr` broadcasts down data rows.
- 3-tier resolution lattice:
  1. 0D cell override: `Cell(value=...)` or `Cell(expr=...)` takes highest precedence.
  2. 1D vector rules: `row.expr` XOR `col.expr`. Intersecting expressions without 0D override raise `AmbiguousExpressionConflict`.
  3. Unassigned enforcement: Every cell must resolve to a value or formula (descriptor cells require explicit label; measure cells require value, vector formula, or cell override); unassigned coordinates raise `SpecificationError`.
- Column roles: `role="descriptor"` for metadata/labels (excluded from formulas); `role="measure"` for calculated timeline/metric data.
- In-memory evaluation: Instant DAG cycle detection, value calculation, and tabular inspection prior to disk serialization.
- Live Excel emission: Emits native formulas, formatting styles, and column layouts to OpenXML (`.xlsx`).

## Modeling Lifecycle

1. Define: Build model hierarchy with explicit column roles, section headers, row gutters, and vector formulas.
2. Compile: Call `compiled = model.compile()` to parse formulas, lower coordinates, and build the DAG.
3. Audit: Inspect tabular slices and cell calculations via `compiled.table(...)` and `compiled.inspect_cell(...)`.
4. Self-heal: Catch typed `F8Error` exceptions (`CycleDetectedError`, `ReferenceResolutionError`, etc.) to diagnose and repair coordinates.
5. Emit: Save to disk via `compiled.save("model.xlsx")` or export in-memory bytes via `compiled.emit_workbook()`.

## Core Invariants

- Explicit 2D coordinates: All cell references require double-quoted 2D bracket syntax (`Table["row", @col]`, `Table["row", col.prev]`, `Table["r1".."r2", @col]`). Bare identifiers or dot syntax (`Table.row`) raise `FormulaParseError`.
- Axis specifiers: Active coordinates require `@col` or `@row`. Bare `@` is retired and raises `FormulaParseError`.
- Cell-level formatting: Apply indentation (`indent=1`) and cell styles to discrete cells (`Cell("Label", indent=1)`), never to rows.
- Uppercase identifiers: Built-in functions (`SUM`, `AVERAGE`, `IF`, `XLOOKUP`) and boolean literals (`TRUE`, `FALSE`) must be uppercase.
- Argument omission: Use `_` placeholder for omitted optional function parameters (e.g. `PMT(rate, nper, pv, _, type)`).
- Structured diagnostics: Catch typed attributes on `F8Error` subclasses; avoid string-matching error messages.

## Reference Index

- [references/syntax-and-grid.md](references/syntax-and-grid.md): 2D coordinate grammar, active axes (`@col`, `@row`), temporal offsets (`col.prev`, `col.next`), slice syntax, column roles, section headers, row gutters, and styling presets.
- [references/functions.md](references/functions.md): 44 built-in Excel functions, signatures, defaults, criteria syntax, day-count conventions, and argument omission rules.
- [references/auditing-and-repair.md](references/auditing-and-repair.md): `CompiledModel` and `TableAuditor` inspection, markdown/DataFrame export, precedent/dependent trees, and structured exception diagnostics.
