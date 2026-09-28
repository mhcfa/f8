---
name: f8-modeling
description: Author, audit, and compile financial models using the f8 Python SDK with 2D coordinates, vector expressions, TableAuditor, and live OpenXML Excel emission.
---

# f8 Financial Modeling Guide

The `f8` Python SDK provides a strongly typed, in-memory DSL for authoring institutional-grade financial models compiled into live OpenXML (`.xlsx`) workbooks.

## Mental Model

- **Hierarchy**: `Model` $\rightarrow$ `Sheet` $\rightarrow$ `Table` $\rightarrow$ `Column` & `Row` $\rightarrow$ `Cell`.
- **2D Coordinates**: Every cell reference specifies both row and column: `Table[RowSelector, ColSelector]`.
- **Vector Formulas**: Rows declare vector formulas lowered across measure columns automatically.
- **In-Memory Verification**: Build in Python, compile in-memory via `model.compile()`, audit with `TableAuditor`, and save to `.xlsx` with live Excel formulas intact.

## Quickstart

```python
from f8 import Model, Cell

model = Model(name="CompanyForecast", theme="corporate_navy")
sheet = model.add_sheet("Forecast")
table = sheet.add_table("Operations", title="Operating Projections", currency="USD")

table.add_column("line_item", role="descriptor", header="Line Item")
table.add_column("2023A", role="measure", header="2023A", format="currency", decimals=1)
table.add_column("2024E", role="measure", header="2024E", format="currency", decimals=1)

table.add_row(
    "revenue",
    cells={"line_item": "Revenue", "2023A": 100.0},
    expr='Operations["revenue", col.prev] * 1.10',
    format="currency",
    decimals=1,
)
table.add_row(
    "cogs",
    cells={"line_item": Cell("Cost of Goods Sold", indent=1), "2023A": 40.0},
    expr='Operations["revenue", @col] * 0.40',
    format="currency",
    decimals=1,
)
table.add_row(
    "gross_profit",
    cells={"line_item": "Gross Profit"},
    expr='Operations["revenue", @col] - Operations["cogs", @col]',
    style="subtotal",
    format="currency",
    decimals=1,
)

compiled = model.compile()
auditor = compiled.table("Operations")
print(auditor.to_markdown(view="values"))
```

## Golden Rules

1. **Always 2D coordinates**: Always use `Table["row", @col]` for intra-period references and `Table["row", col.prev]` for period-over-period lags. Bare identifiers or dot-syntax (`Table.revenue`) are syntax errors.
2. **Explicit column roles**: Use `role="descriptor"` for text labels/metadata and `role="measure"` for calculable numeric timeline periods.
3. **Cell-level indentation**: Place indentation on descriptor cells (`Cell("Label", indent=1)`), not on rows.
4. **Clean visual hierarchy**: Separate logical blocks with `table.add_section_header("Title")` and visual spacing with `table.add_row_gutter()`.
5. **Self-healing structured exceptions**: Catch `F8Error` subclasses (`CycleDetectedError`, `ReferenceResolutionError`, `FormulaParseError`) to diagnose graph cycles, missing coordinates, or syntax errors programmatically.

## Topic Index

For in-depth guides and copy-pasteable patterns, see the references:

- [references/syntax-and-grid.md](references/syntax-and-grid.md): 2D coordinates, vector expressions, column roles, section headers, gutters, and indentation.
- [references/auditing-and-repair.md](references/auditing-and-repair.md): `TableAuditor`, cell inspection, unboxing, and structured exception self-healing.
- [references/patterns.md](references/patterns.md): Roll-forward corkscrews, cross-table links, multi-tier subtotals and totals, and multi-tier headers.
