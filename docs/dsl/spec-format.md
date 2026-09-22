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

Each table file defines an orthogonal 2D calculation matrix with explicit columns and rows:

```json
{
  "id": "string",
  "title": "string",
  "subtitle": "string (optional)",
  "currency": "CurrencyCode (optional default, e.g. USD)",
  "collapsible": "boolean (optional, default: false)",
  "columns": [
    {
      "kind": "data",
      "id": "string",
      "role": "measure | descriptor (optional, default: measure)",
      "headers": ["string (multi-tier header labels, optional)"],
      "style": "SemanticStyle (optional: data | subtotal | total, default: data)",
      "highlight": "boolean (optional, default: false)",
      "format": "FormatPreset (optional)",
      "decimals": "integer (optional, fractional display precision)",
      "currency": "CurrencyCode (optional)",
      "expr": "string (optional, column-wide vector formula)",
      "note": "string (optional)",
      "source": "string or object (optional)"
    },
    {
      "kind": "gutter"
    }
  ],
  "rows": [
    {
      "kind": "section_header",
      "title": "string",
      "indent": "integer (optional, typographic indent level, default: 0)",
      "note": "string (optional)"
    },
    {
      "kind": "gutter"
    },
    {
      "kind": "data",
      "id": "string",
      "style": "SemanticStyle (optional: data | subtotal | total, default: data)",
      "highlight": "boolean (optional, default: false)",
      "format": "FormatPreset (optional)",
      "decimals": "integer (optional, fractional display precision)",
      "currency": "CurrencyCode (optional)",
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
Data columns explicitly declare their calculation role:
- **`measure` (default)**: Numerical quantities, time periods, and calculation outputs. Vector formulas broadcast across opposing measure axes.
- **`descriptor`**: Categorical labels, metadata, units, and stub information. Excluded from vector broadcasting and temporal navigation.

### 3.2. Semantic Styles (`style`), Highlights (`highlight`), & Precision (`decimals`)
Columns and rows declare structural presentation styles (`data`, `subtotal`, `total`), visual emphasis (`highlight: true`), and numeric display precision (`decimals: integer`), completely decoupled from number format masks. See [Semantic Formatting & Themes](formatting.md#3-structural-semantic-styles-semanticstyle).

### 3.3. Cell Input Syntax (`cells`)
Every key under `data_row.cells` refers strictly to a declared data column identifier (`ColId`). Each entry represents one 0D coordinate point:
- **Literal value**: Number (`100000.0`, `42`), string (`"Total Revenue"`), boolean (`true`, `false`), or explicit empty (`null`).
- **Point formula override**: `{ "expr": "IncomeStatement[\"revenue\", col.prev] * 1.08" }`.
- **Detailed override**: `{ "value": 85000, "note": "Audited actuals", "format": "currency", "decimals": 2, "style": "total", "highlight": true, "indent": 1 }`. An explicit `style: "data"` suppresses inherited row/column total borders. Note that `indent` is valid only on descriptor columns; setting `indent > 0` on a measure column produces a validation error.

---

## 4. Core Validation Invariants

1. **Every coordinate is explicit**: Physical cells map 1:1 to explicit table declarations. The compiler never synthesizes phantom rows or unaddressed stub columns.
2. **Single source of truth**:
   - `id`: Immutable programmatic identifier used in formulas and ASTs.
   - `headers` / descriptor cell: Display text rendered in headers and row stubs.
   - Physical coordinates: Computed deterministically by the layout engine.
3. **Three-tier specificity lattice**:
   $$\text{Cell Override (0D Point)} > \text{Vector Rule (1D Row or Column)} > \text{Explicit Null}$$
4. **Zero silent precedence**: If both a row rule (`row.expr`) and a column rule (`col.expr`) target the same measure cell without an explicit cell override, compilation halts with `AmbiguousExpressionConflict`.
5. **Zero silent blanks**: Every measure cell must resolve to a value, formula, or explicit `null`. Unassigned measure cells halt compilation with `UnassignedCellError`.
6. **Formula isolation**: Vector formulas (`row.expr`, `col.expr`) broadcast strictly across opposing data rows and measure columns, and never evaluate across descriptor columns.

---

## 5. Related Documentation

- [Model Authoring Guide](authoring-guide.md): Comprehensive end-to-end guide to authoring raw Finx models.
- [Grid Architecture](grid.md): Multi-tier column headers, section headers, gutters, and table collapsing.
- [Semantic Formatting & Themes](formatting.md): Format preset catalog, conflict resolution, and workbook themes.
- [Modeling Patterns & Archetypes](patterns.md): Canonical 2D patterns (Income Statement, Cap Table, Cross-Footing, Comps, Debt Schedule).
- [End-to-End Project Example](project-example.md): Complete working reference model.
- [Addressing Syntax](addressing.md): Formula reference coordinates (`Table[row, col]`, `@col`, `@row`, `col.prev`, `row.prev`).
- [Formula Grammar](grammar.md): Formal EBNF expression grammar and operator precedence.
- [Intermediate Representation (IR)](../compiler/ir.md): 2D Specificity Resolution Lattice flowchart and expansion pass.
