# End-to-End Model Project Example

This document provides a complete, working reference implementation of a multi-file Finx model project, illustrating how the root manifest and table files connect.

## 1. Project Directory Layout

```text
my_lbo_model/
├── finx.json                  # Root manifest: metadata, theme, and sheet layout
└── tables/                    # Standalone table calculation specifications
    ├── assumptions.json
    └── income_statement.json
```

---

## 2. Root Manifest (`finx.json`)

Defines workbook metadata, theme styling, and the placement of tables across worksheet tabs:

```json
{
  "model_name": "LBO_Simple",
  "theme": "corporate_navy",
  "sheets": [
    {
      "name": "Summary",
      "tab_color": "navy",
      "tables": [
        "tables/assumptions.json"
      ]
    },
    {
      "name": "Model",
      "tab_color": "forest_green",
      "freeze_panes": { "row": 2, "col": 1 },
      "tables": [
        "tables/income_statement.json"
      ]
    }
  ]
}
```

---

## 3. Assumptions Table (`tables/assumptions.json`)

A 2-column parameter lookup table demonstrating descriptor line items and measure values:

```json
{
  "id": "Assumptions",
  "title": "Model Assumptions",
  "columns": [
    { "id": "parameter", "role": "descriptor", "label": "Parameter" },
    { "id": "value", "role": "measure", "label": "Value" }
  ],
  "rows": [
    {
      "id": "tax_rate",
      "format": "percentage",
      "note": "Federal + blended state rate",
      "cells": {
        "parameter": "Corporate Tax Rate",
        "value": 0.25
      }
    },
    {
      "id": "exit_multiple",
      "format": "multiple",
      "cells": {
        "parameter": "Exit EV / EBITDA Multiple",
        "value": 10.0
      }
    }
  ]
}
```

---

## 4. Income Statement Table (`tables/income_statement.json`)

A multi-period forecast statement demonstrating column groups, forward growth vector rules, cross-table assumptions references, and subtotal formats:

```json
{
  "id": "IncomeStatement",
  "title": "Income Statement",
  "columns": [
    { "id": "line_item", "role": "descriptor", "label": "Line Item" },
    { "id": "2023A", "label": "2023A" },
    { "id": "2024E", "label": "2024E" },
    { "id": "2025E", "label": "2025E" },
    { "id": "2026E", "label": "2026E" },
    { "id": "2027E", "label": "2027E" }
  ],
  "column_groups": [
    {
      "id": "historical",
      "label": "Historical Actuals",
      "columns": ["2023A"]
    },
    {
      "id": "forecast",
      "label": "Forecast Projections",
      "columns": ["2024E", "2025E", "2026E", "2027E"]
    }
  ],
  "rows": [
    {
      "id": "revenue",
      "format": "currency",
      "note": "Actuals from SEC 10-K; forecast assumes 8% constant growth",
      "expr": "IncomeStatement[\"revenue\", col.prev] * 1.08",
      "cells": {
        "line_item": "Total Revenue",
        "2023A": 100000.0
      }
    },
    {
      "id": "cogs",
      "format": "currency",
      "expr": "IncomeStatement[\"revenue\", @col] * 0.45",
      "cells": {
        "line_item": "Cost of Goods Sold"
      }
    },
    {
      "id": "gross_profit",
      "style": "subtotal",
      "format": "currency",
      "expr": "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
      "cells": {
        "line_item": "Gross Profit"
      }
    },
    {
      "id": "tax_expense",
      "format": "currency",
      "expr": "IncomeStatement[\"gross_profit\", @col] * Assumptions[\"tax_rate\", \"value\"]",
      "cells": {
        "line_item": "Income Tax Provision"
      }
    }
  ]
}
```

---

See [Specification Format](spec-format.md) for schema details, [Column & Row Groups](groups.md) for super-headers, and [Modeling Patterns](patterns.md) for other schedule archetypes.
