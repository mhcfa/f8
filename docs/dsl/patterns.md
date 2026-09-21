# Modeling Patterns & Archetypes

This document catalogs canonical financial modeling patterns in Finx, demonstrating how the unified 2D matrix model accommodates horizontal statements, vertical schedules, cross-footing matrices, and roll-forward corkscrews.

## 1. Overview

Because Finx treats both rows and columns as symmetrical first-class calculation axes, financial models do not require special table types. The direction of vector formulas (`expr`), axis roles (`measure` vs `descriptor`), and active coordinate tokens (`@col`, `@row`) naturally define the schedule shape:

- **Horizontal schedules**: Row-driven calculations across time columns using `@col` and `col.prev`.
- **Vertical schedules**: Column-driven calculations across line-item rows using `@row` and `row.prev`.
- **Cross-footing matrices**: Two-dimensional calculations with explicit cell overrides at intersections.
- **Transposed schedules**: Columns represent entities or scenarios; rows represent financial metrics.

---

## 2. Horizontal Statement (Income Statement)

Formulas and formatting are primarily row-driven. Descriptor columns (`line_item`, `unit`) isolate labels and metadata from calculations:

```json
{
  "id": "IncomeStatement",
  "title": "Income Statement",
  "columns": [
    { "kind": "data", "id": "line_item", "role": "primary_descriptor", "headers": ["Line Item"] },
    { "kind": "data", "id": "unit", "role": "descriptor", "headers": ["Unit"] },
    { "kind": "data", "id": "2021A", "headers": ["Historical Actuals", "2021A"] },
    { "kind": "data", "id": "2022A", "headers": ["Historical Actuals", "2022A"] },
    { "kind": "data", "id": "2023A", "headers": ["Historical Actuals", "2023A"] },
    { "kind": "data", "id": "2024E", "headers": ["Forecast Projections", "2024E"] },
    { "kind": "data", "id": "2025E", "headers": ["Forecast Projections", "2025E"] }
  ],
  "rows": [
    {
      "kind": "data",
      "id": "revenue",
      "format": "currency",
      "expr": "IncomeStatement[\"revenue\", col.prev] * 1.08",
      "cells": {
        "line_item": "Total Revenue",
        "unit": "USD",
        "2021A": 85000,
        "2022A": 92000,
        "2023A": 100000
      }
    },
    {
      "kind": "data",
      "id": "cogs",
      "format": "currency",
      "expr": "IncomeStatement[\"revenue\", @col] * 0.45",
      "cells": {
        "line_item": "Cost of Goods Sold",
        "unit": "USD"
      }
    },
    {
      "kind": "data",
      "id": "gross_profit",
      "style": "subtotal",
      "format": "currency",
      "expr": "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
      "cells": {
        "line_item": "Gross Profit",
        "unit": "USD"
      }
    },
    {
      "kind": "data",
      "id": "gross_margin",
      "format": "percentage",
      "expr": "IncomeStatement[\"gross_profit\", @col] / IncomeStatement[\"revenue\", @col]",
      "cells": {
        "line_item": "Gross Margin",
        "unit": "%"
      }
    }
  ]
}
```

### Key behaviors
- **Historical overrides vs forecast rules**: In `revenue`, `expr` defines forecast growth while historical years are supplied as explicit cell overrides (`"2021A": 85000`).
- **Active column tracking**: In `cogs` and `gross_profit`, the formula references the evaluating column using `@col`.
- **Descriptor isolation**: Because `line_item` and `unit` are declared as descriptor roles (`primary_descriptor` and `descriptor`), row vector formulas and currency formats never broadcast to them.

---

## 3. Vertical Schedule (Capitalization Table)

Formulas and formatting are column-driven. Rows supply data inputs, with point overrides taking precedence when needed:

```json
{
  "id": "CapTable",
  "title": "Capitalization Table",
  "columns": [
    { "kind": "data", "id": "investor", "role": "primary_descriptor", "headers": ["Investor Name"] },
    { "kind": "data", "id": "share_class", "role": "descriptor", "headers": ["Share Class"] },
    { "kind": "data", "id": "shares", "headers": ["Shares Held"], "format": "number" },
    { "kind": "data", "id": "share_price", "headers": ["Issue Price"], "format": "currency", "decimals": 2 },
    {
      "kind": "data",
      "id": "invested_capital",
      "headers": ["Invested Capital"],
      "format": "currency",
      "expr": "CapTable[@row, \"shares\"] * CapTable[@row, \"share_price\"]"
    },
    {
      "kind": "data",
      "id": "ownership_pct",
      "headers": ["Ownership %"],
      "format": "percentage",
      "expr": "CapTable[@row, \"shares\"] / CapTable[\"total\", \"shares\"]"
    }
  ],
  "rows": [
    {
      "kind": "data",
      "id": "founders",
      "cells": {
        "investor": "Founding Team",
        "share_class": "Common",
        "shares": 5000000,
        "share_price": 0.001,
        "invested_capital": 5000.0
      }
    },
    {
      "kind": "data",
      "id": "series_seed",
      "cells": {
        "investor": "First Step Capital",
        "share_class": "Seed Preferred",
        "shares": 1200000,
        "share_price": 1.25
      }
    },
    {
      "kind": "data",
      "id": "series_a",
      "cells": {
        "investor": "Acme Ventures",
        "share_class": "Series A Preferred",
        "shares": 2500000,
        "share_price": 2.00
      }
    },
    {
      "kind": "data",
      "id": "total",
      "style": "total",
      "cells": {
        "investor": "Total",
        "shares": { "expr": "SUM(CapTable[\"founders\"..\"series_a\", \"shares\"])" },
        "share_price": null,
        "invested_capital": { "expr": "SUM(CapTable[\"founders\"..\"series_a\", \"invested_capital\"])" },
        "ownership_pct": 1.0
      }
    }
  ]
}
```

### Key behaviors
- **Active row tracking**: Column formulas use `@row` (`CapTable[@row, "shares"]`) to bind to the evaluating row.
- **Summary row overrides**: Row `total` supplies point formula overrides (`{ "expr": "SUM(...)" }`) that supersede the column vector formulas at the bottom line.

---

## 4. Cross-Footing Conflict & Resolution

When a summary row intersects a summary column, both vector rules collide. An explicit cell override resolves the collision deterministically:

```json
{
  "id": "Matrix",
  "columns": [
    { "kind": "data", "id": "q1" },
    { "kind": "data", "id": "q2" },
    { "kind": "data", "id": "total", "expr": "SUM(Matrix[@row, \"q1\"..\"q2\"])" }
  ],
  "rows": [
    { "kind": "data", "id": "a", "cells": { "q1": 10, "q2": 20 } },
    { "kind": "data", "id": "b", "cells": { "q1": 30, "q2": 40 } },
    {
      "kind": "data",
      "id": "sum_ab",
      "expr": "Matrix[\"a\", @col] + Matrix[\"b\", @col]",
      "cells": {
        "total": { "expr": "Matrix[\"a\", \"total\"] + Matrix[\"b\", \"total\"]" }
      }
    }
  ]
}
```

If cell `sum_ab.total` were omitted, the compiler would halt with `AmbiguousExpressionConflict` because both `row.expr` and `col.expr` target that measure cell. Supplying the point override resolves the collision.

---

## 5. Transposed Entity Schedule (Public Comps)

In a transposed peer comparison table, entities are columns and metrics are rows:

```json
{
  "id": "PeerComps",
  "title": "Public Comps",
  "columns": [
    { "kind": "data", "id": "metric", "role": "primary_descriptor", "headers": ["Metric"] },
    { "kind": "data", "id": "AAPL", "headers": ["Apple Inc."] },
    { "kind": "data", "id": "MSFT", "headers": ["Microsoft Corp."] },
    { "kind": "data", "id": "GOOG", "headers": ["Alphabet Inc."] }
  ],
  "rows": [
    {
      "kind": "data",
      "id": "ticker",
      "cells": {
        "metric": "Ticker Symbol",
        "AAPL": "AAPL",
        "MSFT": "MSFT",
        "GOOG": "GOOG"
      }
    },
    {
      "kind": "data",
      "id": "share_price",
      "format": "currency",
      "cells": {
        "metric": "Share Price",
        "AAPL": 180.50,
        "MSFT": 420.00,
        "GOOG": 175.25
      }
    }
  ]
}
```

---

## 6. Vertical Schedule with Temporal Lag (Debt Amortization)

For vertical amortization schedules where rows represent time periods and columns represent metrics, `row.prev` provides vertical roll-forward linking:

```json
{
  "id": "DebtSchedule",
  "title": "Senior Debt Amortization",
  "columns": [
    { "kind": "data", "id": "period", "role": "primary_descriptor", "headers": ["Period"] },
    {
      "kind": "data",
      "id": "beg_balance",
      "headers": ["Beginning Balance"],
      "format": "currency",
      "expr": "DebtSchedule[row.prev, \"end_balance\"]"
    },
    {
      "kind": "data",
      "id": "interest",
      "headers": ["Interest Expense"],
      "format": "currency",
      "expr": "DebtSchedule[@row, \"beg_balance\"] * (Assumptions[\"interest_rate\", \"value\"] / 12)"
    },
    { "kind": "data", "id": "repayment", "headers": ["Principal Repayment"], "format": "currency" },
    {
      "kind": "data",
      "id": "end_balance",
      "headers": ["Ending Balance"],
      "format": "currency",
      "expr": "DebtSchedule[@row, \"beg_balance\"] - DebtSchedule[@row, \"repayment\"]"
    }
  ],
  "rows": [
    {
      "kind": "data",
      "id": "period_1",
      "cells": {
        "period": "Month 1",
        "beg_balance": { "expr": "Assumptions[\"initial_debt\", \"value\"]" },
        "repayment": 50000.0
      }
    },
    {
      "kind": "data",
      "id": "period_2",
      "cells": {
        "period": "Month 2",
        "repayment": 50000.0
      }
    }
  ]
}
```

---

See [Specification Format](spec-format.md) for schema syntax and [Addressing Syntax](addressing.md) for coordinate lowering rules.
