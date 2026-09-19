# Column & Row Groups

This document specifies the structural grouping mechanisms in Finx: column super-headers (`column_groups`) and hierarchical row sections (`row_groups`).

## 1. Overview

Grouping structures enrich tabular data with visual hierarchy, aggregation bounds, and outline metadata without complicating the underlying orthogonal 2D matrix calculation model:
- **Column groups (`column_groups`)**: Partition contiguous columns into timeline phases or analytical categories, rendering merged super-headers in Excel.
- **Row groups (`row_groups`)**: Organize rows into single-level or recursive multi-level hierarchies, driving section headers, collapsible outline levels, and indentation on descriptor columns.

---

## 2. Column Groups & Structured Super-Headers

Columns can be grouped into semantic phases or analytical categories using the table's `column_groups` block.

### 2.1. Financial Statement Timeline Example

```json
{
  "column_groups": [
    {
      "id": "historical",
      "label": "Historical Actuals",
      "columns": ["2021A", "2022A", "2023A"]
    },
    {
      "id": "forecast",
      "label": "Forecast Projections",
      "columns": ["2024E", "2025E", "2026E", "2027E", "2028E"]
    }
  ]
}
```

### 2.2. Analytical Categories (Public Comps) Example

```json
{
  "column_groups": [
    {
      "id": "market_data",
      "label": "Market & Capital Structure",
      "columns": ["share_price", "market_cap", "net_debt", "enterprise_value"]
    },
    {
      "id": "multiples",
      "label": "NTM Valuation Multiples",
      "columns": ["ev_sales_ntm", "ev_ebitda_ntm", "p_e_ntm"]
    }
  ]
}
```

### 2.3. Invariants & Compiler Behaviors

- **Contiguity invariant**: Columns within each group must form an exact contiguous sub-slice of the table's declared `columns` array.
- **Disjoint partitions**: Each column can belong to at most one group. Groups cannot overlap.
- **Super-header rendering**: In physical Excel layout, the layout engine generates a top-tier super-header row with merged cells displaying group `label` values.
- **Alias resolution**: Group names resolve dynamically in expressions:
  - Boundaries: `cols.historical.last`, `cols.forecast.first`.
  - Full range slicing: `SUM(IncomeStatement["revenue", cols.forecast])` or `SUM(Comps["market_cap", cols.market_data])`.
- **Automated styling**: The layout engine detects boundaries between adjacent column groups and applies standard vertical dividing borders.

---

## 3. Row Groups & Hierarchical Sections

Rows can be organized into structural sections and calculation aggregates using the recursive `row_groups` block.

### 3.1. Single-Level Row Group Example (OpEx)

```json
{
  "row_groups": [
    {
      "id": "opex",
      "label": "Operating Expenses",
      "children": ["r_and_d", "sales_marketing", "g_and_a"]
    }
  ],
  "rows": [
    { "id": "r_and_d", "expr": "..." },
    { "id": "sales_marketing", "expr": "..." },
    { "id": "g_and_a", "expr": "..." },
    {
      "id": "total_opex",
      "style": "subtotal",
      "format": "currency",
      "expr": "SUM(IncomeStatement[rows.opex, @col])",
      "cells": {
        "line_item": "Total Operating Expenses"
      }
    }
  ]
}
```

### 3.2. Multi-Level Nested Hierarchy Example (Balance Sheet Assets)

```json
{
  "row_groups": [
    {
      "id": "assets",
      "label": "Assets",
      "children": [
        {
          "id": "current_assets",
          "label": "Current Assets",
          "children": ["cash", "accounts_receivable", "inventory"]
        },
        {
          "id": "non_current_assets",
          "label": "Non-Current Assets",
          "children": ["ppe", "goodwill", "intangibles"]
        }
      ]
    }
  ],
  "rows": [
    { "id": "cash", "cells": { "line_item": "Cash & Cash Equivalents" } },
    { "id": "accounts_receivable", "expr": "..." },
    { "id": "inventory", "expr": "..." },
    {
      "id": "total_current_assets",
      "style": "subtotal",
      "format": "currency",
      "expr": "SUM(BalanceSheet[rows.current_assets, @col])",
      "cells": { "line_item": "Total Current Assets" }
    },
    { "id": "ppe", "expr": "..." },
    { "id": "goodwill", "expr": "..." },
    { "id": "intangibles", "expr": "..." },
    {
      "id": "total_non_current_assets",
      "style": "subtotal",
      "format": "currency",
      "expr": "SUM(BalanceSheet[rows.non_current_assets, @col])",
      "cells": { "line_item": "Total Non-Current Assets" }
    },
    {
      "id": "total_assets",
      "style": "total",
      "format": "currency",
      "expr": "BalanceSheet[\"total_current_assets\", @col] + BalanceSheet[\"total_non_current_assets\", @col]",
      "cells": { "line_item": "Total Assets" }
    }
  ]
}
```

### 3.3. Invariants & Compiler Behaviors

- **Unified member hierarchy**: Every group node contains `children`, which can hold row ID strings (referencing calculation line items in `rows`) and/or nested `RowGroup` objects.
- **Contiguity invariant**: The flattened descendant row IDs of any group must form an exact contiguous sub-slice of the table's `rows` array.
- **Disjoint row membership**: Every row ID can appear at most once across the entire `row_groups` tree.
- **Subtotal exclusion**: Subtotal rows (e.g. `total_current_assets`) must **not** be members of the group they aggregate. They sit immediately after the group in `rows`, avoiding circular self-references and ensuring summary lines remain visible during outline collapse.
- **Formula range lowering**: Group tokens (`rows.<group>`) resolve to the contiguous vertical slice of all flattened descendant row IDs of that group (e.g. `SUM(Table[rows.opex, @col])` or `SUM(BalanceSheet[rows.assets, @col])`).
- **Layout generation & descriptor anchoring**:
  - Emits section header rows in the physical Excel grid without calculation `CellId` allocations.
  - Automatically applies row stub indentation to the primary descriptor column based on nesting depth (depth 0: major header, depth 1: sub-header, depth 2: indented line items).
  - Emits native Excel collapsible outline levels (`[+]` / `[-]`) corresponding to tree depth.

---

See [Specification Format](spec-format.md) for table schemas and [Addressing Syntax](addressing.md) for group slice syntax in formulas.
