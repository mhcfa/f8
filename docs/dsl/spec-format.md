# Specification Format

This document defines Finx's multi-file declarative JSON specification schemas, table structure, axis roles, cell input syntax, and validation invariants.

## 1. Project Directory Architecture

A Finx financial model is structured as a modular project directory separating workbook containers from standalone calculation tables:

```text
my_model/
├── finx.json                  # Root manifest: metadata, theme, and sheet layout
└── tables/                    # Standalone table calculation specifications
    ├── assumptions.json
    ├── income_statement.json
    └── balance_sheet.json
```

### V1 Core Invariants
- **Strictly one table per file**: Table definitions (`id`, `columns`, `rows`, formulas) must reside in separate JSON files under `tables/` (or relative subdirectories).
- **No inlining in V1**: Sheets in `finx.json` reference table file paths. Inlining table objects directly into `finx.json` is prohibited.
- **Global table ID uniqueness**: Every table `id` must be globally unique across the entire workbook.

---

## 2. Project Manifest Schema (`finx.json`)

The root manifest defines workbook-level metadata, pluggable theme selection, and physical sheet presentation containers:

```json
{
  "model_name": "string",
  "theme": "string",
  "sheets": [
    {
      "name": "string",
      "tab_color": "string (optional)",
      "freeze_panes": {
        "row": "integer",
        "col": "integer"
      },
      "show_gridlines": "boolean (optional, default: true)",
      "tables": [
        "path/to/table_a.json",
        "path/to/table_b.json"
      ]
    }
  ]
}
```

### Vertical Table Stacking & Modularity
When a sheet lists multiple table paths, the [Spatial Layout Engine](../compiler/spatial-layout.md) stacks them vertically in the listed order, inserting blank-row gutters:
- **Independent column dimensions**: Stacked tables are self-contained calculation blocks. They do not need to share identical column counts, column keys, or timeline spans.
- **Physical anchoring**: Each table maps its declared columns $0 \dots C-1$ directly to worksheet column offsets, anchored by explicit descriptor columns without implicit Column A/B stubs.
- **Semantic decoupling**: Formula references resolve via semantic identifiers (`Table[row, col]`), remaining fully isolated from the physical row and column placement on the sheet.

---

## 3. Table Specification Schema (`tables/<id>.json`)

Each table file defines an orthogonal 2D matrix with symmetrical row and column axes:

```json
{
  "id": "string",
  "title": "string",
  "columns": [
    {
      "id": "string",
      "role": "measure | descriptor (optional, default: measure)",
      "style": "SemanticStyle (optional: data | subtotal | total, default: data)",
      "highlight": "boolean (optional, default: false)",
      "label": "string (optional, defaults to id)",
      "format": "FormatPreset (optional)",
      "decimals": "integer (optional, fractional display precision)",
      "expr": "string (optional, column-wide vector formula)",
      "note": "string (optional)",
      "source": "string or object (optional)"
    }
  ],
  "column_groups": [
    {
      "id": "string",
      "label": "string",
      "columns": ["string"]
    }
  ],
  "row_groups": [
    {
      "id": "string",
      "label": "string",
      "children": ["string | RowGroup (row ID or nested RowGroup)"]
    }
  ],
  "rows": [
    {
      "id": "string",
      "role": "measure | descriptor (optional, default: measure)",
      "style": "SemanticStyle (optional: data | subtotal | total, default: data)",
      "highlight": "boolean (optional, default: false)",
      "format": "FormatPreset (optional)",
      "decimals": "integer (optional, fractional display precision)",
      "expr": "string (optional, row-wide vector formula)",
      "note": "string (optional)",
      "source": "string or object (optional)",
      "cells": {
        "<col_id>": "ValueLiteral | { \"expr\": \"string\" } | DetailedCellInput"
      }
    }
  ]
}
```

### 3.1. Axis Roles (`role`)
Both columns and rows explicitly declare their calculation role:
- **`measure` (default)**: Numerical quantities, time periods, and calculation outputs. Vector formulas broadcast across opposing measure axes.
- **`descriptor`**: Line-item names, categorical labels, units, tickers, and presentation stubs. Vector formulas and measure formats never broadcast into or across descriptor axes.

### 3.2. Semantic Styles (`style`), Highlights (`highlight`), & Precision (`decimals`)
Columns and rows can declare structural presentation styles (`data`, `subtotal`, `total`), visual emphasis (`highlight: true`), and numeric display precision (`decimals: integer`), completely decoupled from number format masks. See [Semantic Formatting & Themes](formatting.md#3-structural-semantic-styles-semanticstyle).

### 3.3. Cell Input Syntax (`cells`)
Every key under `row.cells` is strictly a discrete column identifier (`ColId`). Each entry represents strictly one 0D coordinate point:
- **Literal value**: Number (`100000.0`, `42`), string (`"Total Revenue"`), boolean (`true`, `false`), or explicit empty (`null`).
- **Point formula override**: `{ "expr": "IncomeStatement[\"revenue\", col.prev] * 1.08" }`.
- **Detailed override**: `{ "value": 85000, "note": "Audited actuals", "format": "currency", "decimals": 2, "style": "total", "highlight": true }`. Cell overrides can declare any format preset, precision override, structural style (`data`, `subtotal`, `total`), or highlight flag. An explicit `style: "data"` suppresses inherited row/column total borders.

---

## 4. Core Validation Invariants

1. **Every column is explicit**: Every physical column rendered on a worksheet maps 1:1 to an explicit column declaration in `columns`. Implicit, unaddressed stub columns are never created.
2. **Single source of truth**:
   - `id`: Immutable programmatic identifier used in formulas and ASTs.
   - `label` / descriptor cell: Display text rendered in headers and row stubs.
   - Physical coordinates: Computed automatically by the layout engine.
3. **Three-tier specificity lattice**:
   $$\text{Cell Override (0D Point)} > \text{Vector Rule (1D Row or Column)} > \text{Explicit Null}$$
4. **Zero silent precedence**: If both a row rule (`row.expr`) and a column rule (`col.expr`) target the same measure cell without an explicit cell override, the compiler halts with `AmbiguousExpressionConflict`.
5. **Zero silent blanks**: Every measure cell must resolve to a value, formula, or an explicit `null`. Unassigned measure cells halt compilation with `UnassignedCellError`. Silent fallback to Excel zeros is strictly prohibited.
6. **Formula isolation**: Vector formulas (`row.expr`, `col.expr`) broadcast strictly across opposing `measure` axes and never evaluate across `descriptor` axes.

---

## 5. Related Documentation

- [Column & Row Groups](groups.md): Structured super-headers and hierarchical row sections.
- [Semantic Formatting & Themes](formatting.md): Format preset catalog, conflict resolution, and workbook themes.
- [Modeling Patterns & Archetypes](patterns.md): Canonical 2D patterns (Income Statement, Cap Table, Cross-Footing, Comps, Debt Schedule).
- [End-to-End Project Example](project-example.md): Complete working 3-file reference model.
- [Addressing Syntax](addressing.md): Formula reference coordinates (`Table[row, col]`, `@col`, `@row`, `col.prev`, `row.prev`).
- [Formula Grammar](grammar.md): Formal EBNF expression grammar and operator precedence.
- [Intermediate Representation (IR)](../compiler/ir.md): 2D Specificity Resolution Lattice flowchart and expansion pass.
