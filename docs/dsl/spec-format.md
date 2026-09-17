# Specification Format

This document details Finx's multi-file JSON specification schema, orthogonal 2D matrix data model, axis roles, vector rules, and authoring patterns.

## 1. Project Architecture Overview

A Finx financial model is structured as a modular project directory:

```
my_model/
├── finx.json                  # Root manifest: metadata, theme, and sheet layout
└── tables/                    # Standalone table calculation specifications
    ├── assumptions.json
    ├── income_statement.json
    └── balance_sheet.json
```

### V1 Core Invariant: Strictly One Table Per File
To guarantee clean separation of concerns and deterministic round-trips:
- **Every table resides in its own JSON file**: Table definitions (`id`, `columns`, `rows`, formulas) must be stored in separate JSON files under `tables/` (or relative subdirectories).
- **No inlining in V1**: Sheets in `finx.json` reference table file paths. Inlining table objects directly into `finx.json` is not supported in V1.
- **Global table ID uniqueness**: Every table `id` must be globally unique across the entire workbook.

---

## 2. Project Manifest Schema (`finx.json`)

The root manifest defines workbook-level metadata and physical sheet presentation containers.

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
When a sheet lists multiple table paths, the [Spatial Layout Engine](../compiler/spatial-layout.md) stacks them vertically in the exact order listed, inserting standard blank-row gutters between tables:
- **Independent column dimensions**: Stacked tables are self-contained calculation blocks. They do not need to share identical column counts, column keys, or timeline spans (e.g. stacking a 2-column assumptions table above a 7-column forecast table is completely valid).
- **Physical anchoring**: Each table maps its declared columns $0 \dots C-1$ directly to worksheet column offsets, anchored by explicit descriptor columns without implicit Column A/B stubs.
- **Semantic decoupling**: Formula references resolve via semantic identifiers (`Table[row, col]`), remaining fully isolated from the physical row and column placement on the sheet.
- **Column width resolution**: Physical column widths resolve to the maximum required width across all stacked tables sharing that column.

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
      "label": "string (optional, defaults to id)",
      "format": "FormatPreset (optional)",
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
      "format": "FormatPreset (optional)",
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

### 3.1. Cell Input Syntax (`cells`)
Every key in the `cells` map represents strictly one discrete column identifier (`ColId`). Each value represents strictly one cell (0D point override) and must be one of:
- **Number literal**: `100000.0` or `42`
- **String literal**: `"Total Revenue"` or `"USD"`
- **Boolean literal**: `true` or `false`
- **Explicit empty**: `null`
- **Formula object**: `{ "expr": "IncomeStatement[\"revenue\", col.prev] * 1.08" }`
- **Detailed override object**: `{ "value": 85000, "note": "Audited actuals", "format": "currency" }`

Positional arrays and string prefix hacks (e.g. `=`) are completely eliminated in favor of typed objects.

---

## 4. Core Invariants

- **Orthogonal 2D matrix**: A table is an $R \times C$ grid of addressable cells $(row, col)$. Both row and column dimensions are symmetrical first-class calculation axes in the compiler intermediate representation (`Matrix2D`).
- **Explicit axis roles**: Both columns and rows declare their functional semantic role:
  - `role: "descriptor"`: Categorical labels, line item descriptions, units, tickers, or metadata. Serves as the anchor for hierarchy indentation and presentation stubs.
  - `role: "measure"`: Numerical data, financial time series, and calculation outputs (default).
- **Vector rule isolation**: Vector formulas (`row.expr`, `col.expr`) and vector formats broadcast strictly across opposing `measure` axes. They never bleed into or evaluate across `descriptor` axes.
- **Every column is explicit**: Every physical column rendered on a worksheet maps 1:1 to an explicit column declaration in `columns`. The compiler never injects implicit, unaddressed stub columns (e.g. magic Columns A and B).
- **Three-tier specificity lattice**: Cell properties resolve through a deterministic 3-tier hierarchy:
  $$\text{Cell Override (0D Point)} > \text{Vector Rule (1D Row or Column)} > \text{Explicit Null}$$
- **Zero silent precedence**: If both a row rule and a column rule target the same measure cell without an explicit cell override, the compiler halts with a hard `AmbiguousExpressionConflict` error.
- **Zero silent blanks**: Every measure cell must resolve to a value, formula, or an explicit `null`. Unassigned measure cells halt compilation with `UnassignedCellError`. Silent fallback to Excel zeros is strictly prohibited.
- **Strictly typed 1-to-1 cell inputs**: The `cells` map pairs a single column identifier (`ColId`) directly to a single cell payload (`CellInput`). Every entry represents strictly one cell.
- **Symmetrical coordinate referencing**: Formula references use symmetrical active tokens: `@row` for the active row and `@col` for the active column (`Table[@row, "shares"]`, `Table["revenue", @col]`), along with symmetrical temporal offsets (`col.prev` / `col.next` along columns and `row.prev` / `row.next` along rows).
- **Semantic format tokens**: The `format` field accepts strictly enumerated semantic format tokens (`currency`, `currency_total`, `percentage`, `multiple`, `integer`, `decimal`, `basis_points`, `date`, `text`). Raw Excel format masks or arbitrary pattern strings are prohibited. Presentation typography, number format strings, and accounting borders are resolved through the workbook's pluggable `theme`.
- **First-class notes & provenance**: Use the `note` or `source` fields on cells, rows, or columns. These resolve via `Cell > Row > Col` precedence and compile into native Excel cell comments.

---

## 5. Modeling Patterns & Archetypes

### 5.1. Horizontal Financial Statement (Income Statement)

Formulas and formatting are primarily row-driven. Descriptor columns (`line_item`, `unit`) isolate labels and metadata from calculations:

```json
{
  "id": "IncomeStatement",
  "title": "Income Statement",
  "columns": [
    { "id": "line_item", "role": "descriptor", "label": "Line Item" },
    { "id": "unit", "role": "descriptor", "label": "Unit" },
    { "id": "2021A", "label": "2021A" },
    { "id": "2022A", "label": "2022A" },
    { "id": "2023A", "label": "2023A" },
    { "id": "2024E", "label": "2024E" },
    { "id": "2025E", "label": "2025E" }
  ],
  "column_groups": [
    { "id": "historical", "label": "Historical Actuals", "columns": ["2021A", "2022A", "2023A"] },
    { "id": "forecast", "label": "Forecast Projections", "columns": ["2024E", "2025E"] }
  ],
  "rows": [
    {
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
      "id": "cogs",
      "format": "currency",
      "expr": "IncomeStatement[\"revenue\", @col] * 0.45",
      "cells": {
        "line_item": "Cost of Goods Sold",
        "unit": "USD"
      }
    },
    {
      "id": "gross_profit",
      "format": "currency_total",
      "expr": "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
      "cells": {
        "line_item": "Gross Profit",
        "unit": "USD"
      }
    },
    {
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

### 5.2. Vertical Schedule (Capitalization Table)

Formulas and formatting are column-driven. Rows supply data inputs, with point overrides taking precedence when needed:

```json
{
  "id": "CapTable",
  "title": "Capitalization Table",
  "columns": [
    { "id": "investor", "role": "descriptor", "label": "Investor Name" },
    { "id": "share_class", "role": "descriptor", "label": "Share Class" },
    { "id": "shares", "label": "Shares Held", "format": "integer" },
    { "id": "share_price", "label": "Issue Price", "format": "currency" },
    {
      "id": "invested_capital",
      "label": "Invested Capital",
      "format": "currency",
      "expr": "CapTable[@row, \"shares\"] * CapTable[@row, \"share_price\"]"
    },
    {
      "id": "ownership_pct",
      "label": "Ownership %",
      "format": "percentage",
      "expr": "CapTable[@row, \"shares\"] / CapTable[\"total\", \"shares\"]"
    }
  ],
  "rows": [
    {
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
      "id": "series_seed",
      "cells": {
        "investor": "First Step Capital",
        "share_class": "Seed Preferred",
        "shares": 1200000,
        "share_price": 1.25
      }
    },
    {
      "id": "series_a",
      "cells": {
        "investor": "Acme Ventures",
        "share_class": "Series A Preferred",
        "shares": 2500000,
        "share_price": 2.00
      }
    },
    {
      "id": "total",
      "format": "currency_total",
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

### 5.3. Cross-Footing Conflict & Resolution

When a summary row intersects a summary column, both vector rules collide. An explicit cell override resolves the collision:

```json
{
  "id": "Matrix",
  "columns": [
    { "id": "q1" },
    { "id": "q2" },
    { "id": "total", "expr": "SUM(Matrix[@row, \"q1\"..\"q2\"])" }
  ],
  "rows": [
    { "id": "a", "cells": { "q1": 10, "q2": 20 } },
    { "id": "b", "cells": { "q1": 30, "q2": 40 } },
    {
      "id": "sum_ab",
      "expr": "Matrix[\"a\", @col] + Matrix[\"b\", @col]",
      "cells": {
        "total": { "expr": "Matrix[\"a\", \"total\"] + Matrix[\"b\", \"total\"]" }
      }
    }
  ]
}
```

### 5.4. Transposed Entity Schedule (Public Comps)

In a transposed peer comparison table, entities are columns and metrics are rows:

```json
{
  "id": "PeerComps",
  "title": "Public Comps",
  "columns": [
    { "id": "metric", "role": "descriptor", "label": "Metric" },
    { "id": "AAPL", "label": "Apple Inc." },
    { "id": "MSFT", "label": "Microsoft Corp." },
    { "id": "GOOG", "label": "Alphabet Inc." }
  ],
  "rows": [
    {
      "id": "ticker",
      "role": "descriptor",
      "cells": {
        "metric": "Ticker Symbol",
        "AAPL": "AAPL",
        "MSFT": "MSFT",
        "GOOG": "GOOG"
      }
    },
    {
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

### 5.5. Vertical Schedule (Debt Amortization with `row.prev`)

For vertical amortization schedules where rows represent time periods and columns represent metrics:

```json
{
  "id": "DebtSchedule",
  "title": "Senior Debt Amortization",
  "columns": [
    { "id": "period", "role": "descriptor", "label": "Period" },
    {
      "id": "beg_balance",
      "label": "Beginning Balance",
      "format": "currency",
      "expr": "DebtSchedule[row.prev, \"end_balance\"]"
    },
    {
      "id": "interest",
      "label": "Interest Expense",
      "format": "currency",
      "expr": "DebtSchedule[@row, \"beg_balance\"] * (Assumptions[\"interest_rate\", \"value\"] / 12)"
    },
    { "id": "repayment", "label": "Principal Repayment", "format": "currency" },
    {
      "id": "end_balance",
      "label": "Ending Balance",
      "format": "currency",
      "expr": "DebtSchedule[@row, \"beg_balance\"] - DebtSchedule[@row, \"repayment\"]"
    }
  ],
  "rows": [
    {
      "id": "period_1",
      "cells": {
        "period": "Month 1",
        "beg_balance": { "expr": "Assumptions[\"initial_debt\", \"value\"]" },
        "repayment": 50000.0
      }
    },
    {
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

## 6. Column Groups & Structured Super-Headers

Columns can be grouped into semantic phases or analytical categories using the `column_groups` block:

### 6.1. Financial Statement Timeline Example

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

### 6.2. Public Comparables (Comps) Example

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

### 6.3. Invariants & Compiler Behaviors
- **Contiguity invariant**: Columns within each group must form a contiguous sub-slice of the table's declared `columns` array.
- **Disjoint partitions**: Each column can belong to at most one group. Groups cannot overlap.
- **Super-header rendering**: In physical Excel layout, the layout engine generates a top-tier super-header row with merged cells displaying group `label` values.
- **Alias resolution**: Group names resolve dynamically in expressions:
  - Boundaries: `cols.historical.last`, `cols.forecast.first`.
  - Full range slicing: `SUM(IncomeStatement["revenue", cols.forecast])` or `SUM(Comps["market_cap", cols.market_data])`.
- **Automated styling**: The layout engine detects boundaries between adjacent column groups and applies standard vertical dividing borders.

---

## 7. Row Groups & Hierarchical Sections

Rows can be organized into structural sections and calculation aggregates using the recursive `row_groups` block.

### 7.1. Single-Level Row Group Example (OpEx)

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
      "format": "currency_total",
      "expr": "SUM(IncomeStatement[rows.opex, @col])",
      "cells": {
        "line_item": "Total Operating Expenses"
      }
    }
  ]
}
```

### 7.2. Multi-Level Nested Hierarchy Example (Balance Sheet)

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
      "format": "currency_total",
      "expr": "SUM(BalanceSheet[rows.current_assets, @col])",
      "cells": { "line_item": "Total Current Assets" }
    },
    { "id": "ppe", "expr": "..." },
    { "id": "goodwill", "expr": "..." },
    { "id": "intangibles", "expr": "..." },
    {
      "id": "total_non_current_assets",
      "format": "currency_total",
      "expr": "SUM(BalanceSheet[rows.non_current_assets, @col])",
      "cells": { "line_item": "Total Non-Current Assets" }
    },
    {
      "id": "total_assets",
      "format": "currency_total",
      "expr": "BalanceSheet[\"total_current_assets\", @col] + BalanceSheet[\"total_non_current_assets\", @col]",
      "cells": { "line_item": "Total Assets" }
    }
  ]
}
```

### 7.3. Invariants & Compiler Behaviors
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

## 8. Semantic Formatting & Pluggable Themes

Finx enforces complete decoupling between calculation logic and visual styling. Coordinate definitions declare semantic format intent rather than physical formatting instructions or raw Excel formatting masks.

### 8.1. Standard Format Preset Catalog

The `format` property accepts strictly enumerated semantic presets:

| Format Preset | Semantic Purpose | Typical Rendered Appearance |
| :--- | :--- | :--- |
| `currency` | Standard monetary amounts | `$1,250,000` / `(125,000)` / `-` |
| `currency_total` | Bottom-line/summary totals | Bold, top thin border, bottom double underline |
| `percentage` | Rates and return metrics | `12.5%` or `12.50%` |
| `multiple` | Valuation and leverage multiples | `12.4x` |
| `integer` | Discrete counts, volumes, shares | `1,000` / `(500)` |
| `decimal` | General floating point values | `12.34` |
| `basis_points` | Spread and yield differences | `125 bps` |
| `date` | Calendar dates and period markers | `2024-12-31` or `Dec-24` |
| `text` | Unformatted textual strings | Left-aligned string |

### 8.2. Format Resolution & Conflict Invariants
1. **Point override**: An explicit format declared on `CellInput` always wins.
2. **Descriptor boundary**: Descriptor coordinates isolate text formatting. Measure formats never broadcast to descriptor columns or rows.
3. **Measure vector conflict**: If row $r$ and column $c$ are both measures and specify conflicting formats, compilation halts with `FormatConflictError` unless resolved by a point override.
4. **Single vector format**: If only the row or column specifies a format, that format applies.

### 8.3. Pluggable Themes

The root workbook manifest (`finx.json`) declares the active `theme` (e.g. `"theme": "corporate_navy"`). The theme acts as a pluggable styling provider responsible for number format masks, typography, accounting borders, and color palettes.

---

## 9. End-to-End Model Project Example

### `finx.json`

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

### `tables/assumptions.json`

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

### `tables/income_statement.json`

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
      "format": "currency_total",
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

See [Addressing Syntax](addressing.md) and [Formula Grammar](grammar.md) for expression details.
