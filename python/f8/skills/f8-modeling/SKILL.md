---
name: f8-modeling
description: Author, audit, and compile financial models using the f8 Python SDK with 2D coordinates, vector expressions, TableAuditor, and live OpenXML Excel emission.
---

# f8 Financial Modeling Guide

`f8` is a financial modeling compiler and Python DSL that bridges programmatic modeling and spreadsheet software. It compiles in-memory tabular structures and vector formulas into a validated dependency DAG, enables immediate programmatic auditing, and emits native OpenXML (`.xlsx`) workbooks with live, interactive Excel formulas.

## Mental Model

- **Structural hierarchy**: `Model` $\rightarrow$ `Sheet` $\rightarrow$ `Table` $\rightarrow$ `Column` / `Row` $\rightarrow$ `Cell`.
- **Vector-to-grid lowering**: Tables evaluate formulas along 1D vectors: horizontal row expressions (`row.expr`) broadcast across measure columns, while vertical column expressions (`col.expr`) broadcast down data rows.
- **3-tier specificity lattice**: Cell resolution follows strict precedence:
  1. *0D cell override*: Explicit cell values or `Cell(expr=...)` take highest priority.
  2. *1D vector rules*: `row.expr` XOR `col.expr` (intersecting expressions without a 0D override raise `AmbiguousExpressionConflict`).
  3. *Unassigned enforcement*: Every measure cell must resolve to an explicit value or formula; unassigned coordinates raise `SpecificationError`.
- **In-memory evaluation**: Compiling executes the model in memory, enabling instant value calculation, tabular inspection, and cycle detection before saving to disk.
- **Live Excel emission**: Emitted `.xlsx` workbooks retain native Excel formulas, formatting styles, and column layouts.

## Modeling Lifecycle

1. **Define**: Construct the model hierarchy with explicit column roles (`descriptor` vs `measure`), section headers, row gutters, and vector formulas.
2. **Compile**: Call `compiled = model.compile()` to parse formulas, lower vector coordinates, and resolve the dependency DAG.
3. **Audit**: Inspect tabular slices and cell calculations via `compiled.table(...)` and `compiled.inspect_cell(...)` before generating outputs.
4. **Self-heal**: Catch structured `F8Error` exceptions (`CycleDetectedError`, `ReferenceResolutionError`, `FormulaParseError`, etc.) to diagnose and resolve errors programmatically.
5. **Emit**: Save the final workbook via `compiled.save("model.xlsx")` or export in-memory bytes using `compiled.emit_workbook()`.

## Core Invariants

- **Always 2D coordinates**: All cell references require explicit 2D bracket syntax (`Table["row", @col]`, `Table["row", col.prev]`, `Table["r1".."r2", @col]`). Quoted row and column keys are required; bare identifiers inside brackets or dot syntax (`Table.row`) raise parse errors.
- **Explicit column roles**: Assign `role="descriptor"` for text labels/metadata and `role="measure"` for calculated timeline periods.
- **3-tier expression precedence**: 0D cell overrides (`Cell(value=...)` or `Cell(expr=...)`) override 1D vector expressions. Never define both a `row.expr` and `col.expr` for the same cell without a 0D override.
- **Complete grid assignment**: Every measure cell must have an explicit value, vector formula, or cell override. Unassigned cells raise `SpecificationError`.
- **Cell-level formatting and indentation**: Apply indentation and cell-specific styles directly to cells (`Cell("Label", indent=1)`), not to rows.
- **Uppercase functions and booleans**: Built-in Excel functions (`SUM`, `AVERAGE`, `IF`, `XLOOKUP`) and boolean literals (`TRUE`, `FALSE`) must be uppercase.
- **Explicit argument omission**: Use the underscore placeholder `_` to omit optional positional function arguments (e.g. `PMT(rate, nper, pv, _, type)`).
- **Structured error handling**: Rely on typed attributes on `F8Error` subclasses rather than string matching to resolve compiler diagnostics.

## Reference Index

Consult the detailed references when authoring specific model components:

- [references/syntax-and-grid.md](references/syntax-and-grid.md): 2D coordinate grammar, active axes (`@col`, `@row`), temporal offsets (`col.prev`, `col.next`), slice syntax, column roles, 3-tier specificity lattice, section headers, row gutters, and styling.
- [references/functions.md](references/functions.md): 44 built-in Excel functions (math, logical, financial, lookup, date), signatures, default values, criteria syntax, and argument omission rules.
- [references/auditing-and-repair.md](references/auditing-and-repair.md): `CompiledModel` methods, `TableAuditor` inspection/markdown/DataFrame export, precedent/dependent traversal, and structured exception diagnostics.
