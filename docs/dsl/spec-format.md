# Specification Format & 2D Grid

This document defines f8's declarative multi-file JSON specification schemas, 2D grid structure, axis roles, cell input syntax, and validation invariants.

---

## 1. Project Directory Architecture

A f8 model project is structured as a directory separating workbook containers from standalone calculation tables:

```text
my_model/
├── f8.json                  # Root manifest: metadata, theme, and sheet layout
└── tables/                    # Standalone table calculation specifications
    ├── assumptions.json
    ├── income_statement.json
    └── balance_sheet.json
```

### Invariants
- **Strictly one table per file**: Table definitions (`id`, `columns`, `rows`, formulas) reside in separate JSON files under `tables/`.
- **Logical table references**: Sheets in `f8.json` reference tables purely by logical identifier (`TableId`, e.g. `"IncomeStatement"`), never filesystem paths.
- **Global table ID uniqueness**: Every table `id` must be globally unique across the workbook.
- **Decoupled file discovery**: The project loader discovers all `tables/*.json` files and indexes them by internal `id`. Filenames are decoupled from table IDs.

---

## 2. Project Manifest Schema (`f8.json`)

The root manifest defines workbook-level metadata, pluggable theme selection, and sheet containers:

```json
{
  "model_name": "LBO_Simple",
  "theme": "corporate_navy",
  "sheets": [
    {
      "name": "Summary",
      "tab_color": "#1B365D",
      "show_gridlines": true,
      "tables": [
        "Assumptions"
      ]
    },
    {
      "name": "Financials",
      "tab_color": "#2C4D75",
      "freeze_panes": {
        "row": 6,
        "col": 2
      },
      "show_gridlines": true,
      "tables": [
        "RevenueSchedule",
        "IncomeStatement"
      ]
    }
  ]
}
```

### Manifest Properties
- **`model_name`** (`string`, required): Model title used in metadata and workbook document properties.
- **`theme`** (`string`, optional, default `"corporate_navy"`): Built-in theme name or relative path to a custom theme JSON file.
- **`sheets`** (`array`, required): Ordered list of worksheet definitions:
  - `name` (`string`, required): Worksheet tab label. Must be unique across the workbook.
  - `tab_color` (`string`, optional): Hex color code starting with `#` (`#RGB`, `#RRGGBB`, or `#RRGGBBAA`).
  - `freeze_panes` (`object`, optional): `{ "row": u32, "col": u32 }` physical worksheet row and column split lines.
  - `show_gridlines` (`boolean`, optional, default `true`): Toggles gridline rendering.
  - `tables` (`array of strings`, required): Logical table IDs (`TableId`, e.g. `"Assumptions"`, `"IncomeStatement"`) assigned to this worksheet. Stacked vertically in declaration order, separated by blank row gutters.

---

## 3. Table Specification Schema (`tables/*.json`)

Each table file defines an orthogonal 2D calculation matrix with explicit columns and rows:

```json
{
  "id": "IncomeStatement",
  "title": "Income Statement",
  "subtitle": "($ in Millions)",
  "currency": "usd",
  "collapsible": false,
  "columns": [
    {
      "kind": "data",
      "id": "line_item",
      "role": "descriptor",
      "headers": ["Line Item"]
    },
    {
      "kind": "data",
      "id": "2024E",
      "role": "measure",
      "headers": ["Forecast Projections", "2024E"],
      "style": "data",
      "format": "currency",
      "decimals": 1,
      "currency": "usd",
      "expr": "...",
      "highlight": false
    },
    {
      "kind": "gutter"
    }
  ],
  "rows": [
    {
      "kind": "section_header",
      "title": "Operating Expenses",
      "indent": 0
    },
    {
      "kind": "gutter"
    },
    {
      "kind": "data",
      "id": "r_and_d",
      "style": "data",
      "format": "currency",
      "decimals": 1,
      "expr": "...",
      "cells": {
        "line_item": { "value": "Research & Development", "indent": 1 },
        "2024E": 120.0
      }
    }
  ]
}
```

---

## 4. Column Elements & Horizontal Grid Structure

Columns are declared in `table.columns` as either calculable data columns or spacing gutters.

### 4.1. Data Columns (`kind: "data"`)
- **`id`** (`string`, required): Column identifier (`ColId`), referenced in formula coordinates (e.g. `"2024E"`).
- **`role`** (`"measure"` | `"descriptor"`, optional, default `"measure"`):
  - `measure`: Numerical quantities, time periods, and calculation outputs. Row vector formulas (`row.expr`) broadcast across all measure columns.
  - `descriptor`: Row labels, line item descriptions, units, or metadata stubs. Descriptors are strictly isolated from vector broadcasting and temporal offsets (`col.prev` / `col.next`).
- **`headers`** (`array of strings`, optional): Multi-tier column header labels.
  - *Center across selection*: Adjacent columns sharing identical upper-tier labels center automatically across the span without cell merging (FAST financial modeling standard).
  - *Bottom alignment*: When columns declare differing header array lengths, headers are bottom-aligned so the bottom tier always holds the primary metric label while upper tiers remain blank.
  - *Zero headers*: Omitting `headers` allocates zero physical header rows.

### 4.2. Column Gutters (`kind: "gutter"`)
Spacing columns visually separate analytical periods or categories without dummy identifiers:
- Allocate blank spacing width (default 3.0 characters).
- Produce zero calculation cells and are skipped by formula navigation.

---

## 5. Row Elements & Vertical Grid Structure

Rows are declared in `table.rows` in their exact vertical display order.

### 5.1. Data Rows (`kind: "data"`)
Calculable rows containing values, vector formulas, or individual cell overrides:
- **`id`** (`string`, required): Row identifier (`RowId`), referenced in formula coordinates (e.g. `"revenue"`).
- **`expr`** (`string`, optional): Row-wide vector formula broadcasting across all measure columns.
- **`style`**, **`format`**, **`decimals`**, **`currency`**, **`highlight`**: Row-level default presentation attributes.
- **`cells`** (`object`, optional): Map of column IDs (`ColId`) to cell inputs.

### 5.2. Section Header Rows (`kind: "section_header"`)
Visual divider banners organizing line items into logical financial sections:
- **`title`** (`string`, required): Header label displayed across the table width.
- **`indent`** (`integer`, optional, default `0`): Typographic indent level ($0 \le N \le 3$).
- Section headers contain zero calculable cells and do not interrupt vector formulas across data rows.

### 5.3. Row Gutters (`kind: "gutter"`)
Vertical blank spacing rows (default 9.0 pt height) between logical sections. Gutters have no ID and contain no cells.

### 5.4. Typographic Indentation
Descriptor cells and section headers accept an optional `indent` level ($0 \le N \le 3$). Indentation applies directly to individual descriptor cells as a typographic cell indent. It has zero effect on row height, calculation graphs, or measure columns. Setting `indent` on a measure cell produces a validation error (`InvalidIndentOnMeasureCell`).

### 5.5. Table Collapsing (`collapsible`)
Tables can opt into outline folding via `"collapsible": true`. When enabled, all body rows collapse into the table title banner using spreadsheet outline groups.

---

## 6. Cell Input Syntax (`cells`)

Every key under `data_row.cells` refers strictly to a declared data column identifier (`ColId`). Each entry represents one 0D coordinate point:

1. **Literal value**: Number (`100000.0`, `42`), string (`"Total Revenue"`), boolean (`true`, `false`), or explicit empty (`null`).
2. **Point formula override**: `{ "expr": "IncomeStatement[\"revenue\", col.prev] * 1.08" }`.
3. **Detailed override**:
   ```json
   {
     "value": 85000,
     "note": "Audited actuals",
     "format": "currency",
     "decimals": 2,
     "style": "total",
     "highlight": true,
     "indent": 1
   }
   ```
   Setting `style: "data"` on an individual cell override acts as an explicit point escape hatch, suppressing inherited horizontal or vertical summary borders.

---

## 7. Core Validation Invariants

1. **1:1 physical mapping**: Declared rows and columns map 1:1 to physical spreadsheet coordinates. The compiler never synthesizes phantom rows or unaddressed stub columns.
2. **Single source of truth**:
   - `id`: Immutable programmatic identifier used in formulas and ASTs.
   - `headers` / descriptor cell: Display text rendered in headers and row stubs.
   - Physical coordinates: Computed deterministically by the spatial layout engine.
3. **Three-tier specificity lattice**:
   $$\text{Cell override (0D point)} > \text{Vector rule (1D row or column)} > \text{Explicit null}$$
4. **Zero silent ambiguity**: If both a row rule (`row.expr`) and a column rule (`col.expr`) target the same measure cell without an explicit cell override, compilation halts with `AmbiguousExpressionConflict`.
5. **Zero silent blanks**: Every measure cell must resolve to a value, formula override, vector formula, or explicit `null`. Unassigned measure cells halt compilation with `UnassignedCellError`.
6. **Descriptor boundary isolation**: Vector formulas (`row.expr`, `col.expr`) broadcast strictly across opposing data rows and measure columns, and never evaluate across descriptor columns.

---

## 8. Related Documentation

- [CLI & JSON Authoring Guide](../cli-authoring-guide.md): Practical end-to-end walkthrough for creating financial models.
- [Addressing Syntax](./addressing.md): Formula reference coordinates (`Table[row, col]`, `@col`, `@row`, `col.prev`, `row.prev`).
- [Formula Grammar](./grammar.md): Formal EBNF expression grammar and operator precedence.
- [Semantic Formatting & Themes](./formatting.md): Format preset catalog, conflict resolution, and workbook themes.
- [Modeling Patterns & Archetypes](./patterns.md): Canonical 2D patterns (Income Statement, Cap Table, Cross-Footing, Comps, Debt Schedule).
- [Architecture Overview](../architecture.md): Compilation pipeline, spatial layout planning, and column width arbitration.
