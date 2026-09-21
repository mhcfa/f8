# Grid Architecture

Finx uses an explicit 2D grid model where declared rows and columns map 1:1 to physical spreadsheet coordinates. Tables define structural elements directly rather than through synthetic grouping abstractions.

## 1. Column Structure

Columns are declared in `table.columns` as either calculable data columns or spacing gutters.

### 1.1. Data Columns & Multi-Tier Headers

Data columns define calculation vectors or descriptor labels:

- **Axis roles**: `measure` (default calculation vector) or `descriptor` (metadata label).
- **Multi-tier headers**: Declared via `headers: ["Tier 1", "Tier 2", ...]`. Contiguous columns sharing identical upper-tier labels visually center across the span without cell merging.
- **Zero-header tables**: When all columns omit `headers`, zero physical header rows are allocated.

```json
{
  "columns": [
    {
      "kind": "data",
      "id": "line_item",
      "role": "descriptor",
      "headers": ["Line Item"]
    },
    {
      "kind": "data",
      "id": "2023a",
      "headers": ["Historical Actuals", "2023A"]
    },
    {
      "kind": "data",
      "id": "2024e",
      "headers": ["Forecast Projections", "2024E"]
    }
  ]
}
```

### 1.2. Column Gutters

Spacing columns separate analytical periods or categories without dummy identifiers:

```json
{ "kind": "gutter" }
```

Gutter columns allocate blank spacing width (default 3.0 characters) and produce zero calculation cells.

---

## 2. Row Structure

Rows are declared in `table.rows` in their exact vertical display order.

### 2.1. Row Kinds

- **Data rows (`kind: "data"`)**: Contain line-item calculations, vector formulas, or cell overrides.
- **Section headers (`kind: "section_header"`)**: Visual category dividers with a title banner and optional note.
- **Gutters (`kind: "gutter"`)**: Vertical blank spacing rows (default 9.0 pt height) between logical sections.

```json
{
  "rows": [
    {
      "kind": "section_header",
      "title": "Operating Expenses"
    },
    {
      "kind": "data",
      "id": "r_and_d",
      "cells": {
        "line_item": { "value": "Research & Development", "indent": 1 }
      },
      "expr": "..."
    },
    {
      "kind": "gutter"
    }
  ]
}
```

### 2.2. Typographic Indentation

Descriptor cells and section headers accept an optional `indent` level (e.g. `indent: 1`, `indent: 2`). Indentation applies directly to individual descriptor cells as a typographic cell indent. It has zero effect on row height, calculation graphs, or measure columns.

---

## 3. Table Collapsing

Tables can opt into outline folding via `"collapsible": true`. When enabled, all body rows collapse into the table title banner using spreadsheet outline groups.

---

See [Specification Format](spec-format.md) for full JSON schemas and [Addressing Syntax](addressing.md) for formula references.
