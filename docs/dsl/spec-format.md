# Specification Format

This document details Finx's multi-file JSON specification schema, table modularity, row partitioning rules, and authoring patterns.

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
- **Independent column dimensions**: Stacked tables are self-contained calculation blocks. They do not need to share identical column counts, column keys, or timeline spans (e.g. stacking a 1-column assumptions table above a 5-column forecast table is completely valid).
- **Physical anchoring**: Each table anchors its row stub labels and data columns at standard sheet offsets (Columns A/B for stubs, Column C onward for data columns).
- **Semantic decoupling**: Formula references resolve via semantic identifiers (`Table[row, col]`), remaining fully isolated from the physical row and column placement on the sheet.
- **Column width resolution**: Physical column widths resolve to the maximum required width across all stacked tables sharing that column.

---

## 3. Table Specification Schema (`tables/<id>.json`)

Each table file defines the calculation dimensions, optional column groups, and row definitions for a single table:

```json
{
  "id": "string",
  "title": "string",
  "columns": ["string"],
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
      "label": "string",
      "format": "FormatPreset (optional)",
      "note": "string (optional)",
      "source": "string or object (optional)",
      "value": ["number | string | null (exactly one of value/expr/match)"],
      "expr": "string (exactly one of value/expr/match)",
      "match": {
        "<column_id | col_a..col_b | cols.group_id>": {
          "value": ["number | string | null (exactly one of value/expr)"],
          "expr": "string (exactly one of value/expr)",
          "note": "string (optional)",
          "source": "string or object (optional)"
        }
      }
    }
  ]
}
```

---

## 4. Core Invariants

- **Canonical core & agent-first IR**: Specifications strictly enforce unambiguous forms with zero implicit type coercion, broadcasting, or hidden defaults. The schema is designed as an intermediate representation (IR) optimized for deterministic synthesis by AI agents and rigorous compiler verification.
- **Universal column semantics**: Column identifiers (`ColId`) in `columns` are user-defined strings. Columns are not restricted to chronological timelines; they can represent units (`"unit"`), categorical metadata (`"segment"`, `"class"`), scenarios (`"Bull"`, `"Base"`), or multi-directional metrics (`"shares"`, `"ownership_pct"`). Every column declared in `columns` is an addressable coordinate in the table grid (`Table[row, col]`).
- **Mutually exclusive row payload**: A row definition must specify exactly one calculation payload: `value`, `expr`, or `match`. Specifying multiple payloads or omitting all three produces a compile-time schema error. Similarly, within each `match` arm, exactly one of `value` or `expr` must be specified.
- **Explicit input arrays**: All literal inputs are provided as explicit arrays: `"value": [...]`.
- **Strict 1-to-1 dimension match**: The length of `value` must equal the number of targeted columns (`len(value) == len(target_columns)`). Single-cell inputs require single-element arrays: `"value": [0.25]`.
- **Explicit empty cells**: Empty cells must be explicitly represented with `null`, e.g. `[null, 92000, 100000]`.
- **Disjoint & exhaustive partitioning (`match`)**: The `match` block is the sole mechanism for varying behavior across columns. Every column in the table's `columns` array must be covered by **exactly one** match arm. Overlapping selectors and uncovered columns produce compile-time errors. Catch-all wildcards or `default` arms are strictly prohibited in favor of explicit coordinates or groups.
- **Match selector grammar**: Each key in a `match` object must be one of:
  - Single column ID: e.g. `"2024E"`.
  - Bounded contiguous range: e.g. `"2021A..2023A"` (start column must precede end column in `columns`).
  - Column group alias: e.g. `"cols.forecast"`.
- **Payload cardinality**: In an arm targeting $k$ columns:
  - `value`: Array of literals of exact length $k$ (`len(value) == k`).
  - `expr`: Scalar formula string evaluated dynamically across each column in the matched partition.
- **Semantic format tokens**: The `format` field accepts strictly enumerated semantic format tokens (`currency`, `currency_total`, `percentage`, `multiple`, `integer`, `decimal`, `basis_points`, `date`, `text`). Raw Excel format masks or arbitrary pattern strings are prohibited. Presentation typography, number format strings, and accounting borders are resolved through the workbook's pluggable `theme`.
- **First-class notes & provenance**: Use the `note` or `source` fields to record model notes and data provider provenance. These survive serialization and compile into native Excel cell comments.

---

## 5. Row Authoring Patterns

### 5.1. Uniform Calculation Across All Columns

When a formula applies identically to all columns in a table:

```json
{
  "id": "gross_profit",
  "label": "Gross Profit",
  "format": "currency_total",
  "expr": "IncomeStatement[\"revenue\", @] - IncomeStatement[\"cogs\", @]"
}
```

### 5.2. Single Assumption (1-Column Table)

Assumptions and global parameters are modeled as single-column tables:

```json
{
  "id": "tax_rate",
  "label": "Corporate Tax Rate",
  "format": "percentage",
  "value": [0.25],
  "note": "Statutory corporate federal + blended state tax rate"
}
```

### 5.3. Partitioned Row (Historical Actuals vs Forecast)

Separates hardcoded historical values from formula-driven forecast projections:

```json
{
  "id": "revenue",
  "label": "Total Revenue",
  "format": "currency",
  "match": {
    "2021A..2023A": { "value": [85000, 92000, 100000] },
    "2024E..2028E": { "expr": "IncomeStatement[\"revenue\", col.prev] * 1.08" }
  }
}
```

### 5.4. Corkscrew Roll-Forward with Base Anchor

Models balance sheet or debt roll-forwards with an explicit initial period and explicit roll-forward range:

```json
{
  "id": "beginning_cash",
  "label": "Beginning Cash Balance",
  "format": "currency",
  "match": {
    "2024E": { "expr": "Assumptions[\"initial_cash\", \"value\"]" },
    "2025E..2028E": { "expr": "BalanceSheet[\"ending_cash\", col.prev]" }
  }
}
```

### 5.5. Mixing Units & Metadata with Timeseries Columns

When declaring metadata columns (such as `"unit"`) alongside a financial timeline:

```json
{
  "id": "revenue",
  "label": "Total Revenue",
  "format": "currency",
  "match": {
    "unit": { "expr": "Assumptions[\"reporting_currency\", \"value\"]" },
    "2023A": { "value": [100000.0] },
    "2024E..2028E": { "expr": "IncomeStatement[\"revenue\", col.prev] * 1.08" }
  }
}
```

- **Partition isolation**: The `match` block isolates the text/metadata column from numeric operations, avoiding `#VALUE!` errors in Excel.
- **Dynamic references**: The unit cell is a full calculation citizen (`IncomeStatement["revenue", "unit"]`) that can evaluate formulas referencing global parameters or be referenced by other tables.

### 5.6. Capitalization Table (Cap Table / General Tabular Matrix)

For non-temporal matrices where calculations vary by column rather than across time:

```json
{
  "id": "CapTable",
  "title": "Capitalization Table",
  "columns": ["investor", "share_class", "shares", "share_price", "invested_capital", "ownership_pct"],
  "rows": [
    {
      "id": "series_a",
      "label": "Series A Lead Investor",
      "match": {
        "investor": { "value": ["Acme Capital"] },
        "share_class": { "value": ["Preferred"] },
        "shares": { "value": [2500000] },
        "share_price": { "value": [2.00] },
        "invested_capital": { "expr": "CapTable[\"series_a\", \"shares\"] * CapTable[\"series_a\", \"share_price\"]" },
        "ownership_pct": { "expr": "CapTable[\"series_a\", \"shares\"] / CapTable[\"total\", \"shares\"]" }
      }
    }
  ]
}
```

### 5.7. Vertical Schedule (Time on Rows, Metrics on Columns)

For vertical amortization waterfalls or schedules where rows represent time periods and columns represent metrics:

```json
{
  "id": "DebtSchedule",
  "title": "Senior Debt Amortization",
  "columns": ["beg_balance", "interest", "repayment", "end_balance"],
  "rows": [
    {
      "id": "period_1",
      "label": "Month 1",
      "match": {
        "beg_balance": { "expr": "Assumptions[\"initial_debt\", \"value\"]" },
        "interest": { "expr": "DebtSchedule[\"period_1\", \"beg_balance\"] * (Assumptions[\"interest_rate\", \"value\"] / 12)" },
        "repayment": { "value": [50000.0] },
        "end_balance": { "expr": "DebtSchedule[\"period_1\", \"beg_balance\"] - DebtSchedule[\"period_1\", \"repayment\"]" }
      }
    },
    {
      "id": "period_2",
      "label": "Month 2",
      "match": {
        "beg_balance": { "expr": "DebtSchedule[\"period_1\", \"end_balance\"]" },
        "interest": { "expr": "DebtSchedule[\"period_2\", \"beg_balance\"] * (Assumptions[\"interest_rate\", \"value\"] / 12)" },
        "repayment": { "value": [50000.0] },
        "end_balance": { "expr": "DebtSchedule[\"period_2\", \"beg_balance\"] - DebtSchedule[\"period_2\", \"repayment\"]" }
      }
    }
  ]
}
```

---

## 6. Column Groups & Structured Super-Headers

Columns can be grouped into semantic phases or analytical categories using the `column_groups` block. This supports both chronological timelines (e.g. historical actuals vs forecast projections) and general tabular groupings (e.g. trading comps or scenario analysis).

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
- **Timeline calculations**: Dynamic timing metrics (such as period fractions, discount periods, and day counts) must be defined as dedicated calculation rows within `rows`, keeping headers strictly declarative and coordinates static.

---

## 7. Row Groups & Hierarchical Sections

Rows can be organized into structural sections and calculation aggregates using the recursive `row_groups` block. This models the nested hierarchies common in financial statements (such as Balance Sheets and OpEx schedules) while keeping the underlying `rows` array as the single source of truth for calculations.

### 7.1. Single-Level Row Group Example (OpEx)

For tables with flat sections, groups list targeted row IDs in `children`:

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
    { "id": "r_and_d", "label": "Research & Development", "expr": "..." },
    { "id": "sales_marketing", "label": "Sales & Marketing", "expr": "..." },
    { "id": "g_and_a", "label": "General & Administrative", "expr": "..." },
    {
      "id": "total_opex",
      "label": "Total Operating Expenses",
      "format": "currency_total",
      "expr": "SUM(IncomeStatement[rows.opex, @])"
    }
  ]
}
```

### 7.2. Multi-Level Nested Hierarchy Example (Balance Sheet)

For multi-tier statements, parent groups nest sub-groups recursively using `children`:

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
    { "id": "cash", "label": "Cash & Cash Equivalents", "match": { ... } },
    { "id": "accounts_receivable", "label": "Accounts Receivable", "expr": "..." },
    { "id": "inventory", "label": "Inventories", "expr": "..." },
    {
      "id": "total_current_assets",
      "label": "Total Current Assets",
      "format": "currency_total",
      "expr": "SUM(BalanceSheet[rows.current_assets, @])"
    },
    { "id": "ppe", "label": "Property, Plant & Equipment", "expr": "..." },
    { "id": "goodwill", "label": "Goodwill", "expr": "..." },
    { "id": "intangibles", "label": "Intangibles", "expr": "..." },
    {
      "id": "total_non_current_assets",
      "label": "Total Non-Current Assets",
      "format": "currency_total",
      "expr": "SUM(BalanceSheet[rows.non_current_assets, @])"
    },
    {
      "id": "total_assets",
      "label": "Total Assets",
      "format": "currency_total",
      "expr": "BalanceSheet[\"total_current_assets\", @] + BalanceSheet[\"total_non_current_assets\", @]"
    }
  ]
}
```

### 7.3. Mixed Hierarchy Example

Groups can also combine direct row IDs and nested sub-groups within the same `children` array:

```json
{
  "row_groups": [
    {
      "id": "opex",
      "label": "Operating Expenses",
      "children": [
        "depreciation",
        {
          "id": "marketing",
          "label": "Marketing Expenses",
          "children": ["digital_ads", "events"]
        },
        "general_admin"
      ]
    }
  ]
}
```

### 7.4. Invariants & Compiler Behaviors
- **Unified member hierarchy**: Every group node contains `children`, which can hold row ID strings (referencing calculation line items in `rows`) and/or nested `RowGroup` objects.
- **Contiguity invariant**: The flattened descendant row IDs of any group must form an exact contiguous sub-slice of the table's `rows` array.
- **Disjoint row membership**: Every row ID can appear at most once across the entire `row_groups` tree.
- **Subtotal exclusion**: Subtotal rows (e.g. `total_current_assets`) must **not** be members of the group they aggregate. They sit immediately after the group in `rows`, avoiding circular self-references and ensuring summary lines remain visible during outline collapse.
- **Formula range lowering**: Group tokens (`rows.<group>`) resolve to the contiguous vertical slice of all flattened descendant row IDs of that group (e.g. `SUM(Table[rows.opex, @])` or `SUM(BalanceSheet[rows.assets, @])`). Both leaf and parent groups can be referenced in formulas as long as their descendant rows are contiguous.
- **Layout generation**:
  - Emits section header rows in the physical Excel grid without calculation `CellId` allocations.
  - Automatically calculates row stub indentation based on nesting depth (depth 0: major header, depth 1: sub-header, depth 2: indented line items).
  - Emits native Excel collapsible outline levels (`[+]` / `[-]`) corresponding to tree depth.

---

## 8. Semantic Formatting & Pluggable Themes

Finx enforces complete decoupling between calculation logic and visual styling. Row definitions declare semantic format intent rather than physical formatting instructions or raw Excel formatting masks.

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

### 8.2. Pluggable Themes

The root workbook manifest (`finx.json`) declares the active `theme` (e.g. `"theme": "corporate_navy"`). The theme acts as a pluggable styling provider responsible for:
- **Number format masks**: Mapping semantic tokens to locale-aware Excel masks (e.g. accounting parentheses vs negative minus signs, zero dashes, currency symbols).
- **Typography**: Font family, base point size, and weight hierarchy (e.g. bolding on section headers and `currency_total`).
- **Accounting borders**: Automated insertion of standard financial underlines (top thin, bottom double) on subtotal and total rows.
- **Palette and accents**: Tab colors, header fills, and column group dividing borders.

External or organization-specific styling plugins can provide custom themes without modifying any underlying calculation files under `tables/`.

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
  "columns": ["value"],
  "rows": [
    {
      "id": "tax_rate",
      "label": "Corporate Tax Rate",
      "format": "percentage",
      "value": [0.25],
      "note": "Federal + blended state rate"
    },
    {
      "id": "exit_multiple",
      "label": "Exit EV / EBITDA Multiple",
      "format": "multiple",
      "value": [10.0]
    }
  ]
}
```

### `tables/income_statement.json`

```json
{
  "id": "IncomeStatement",
  "title": "Income Statement",
  "columns": ["2023A", "2024E", "2025E", "2026E", "2027E"],
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
      "label": "Total Revenue",
      "format": "currency",
      "note": "Actuals from SEC 10-K; forecast assumes 8% constant growth",
      "match": {
        "2023A": { "value": [100000.0] },
        "cols.forecast": { "expr": "IncomeStatement[\"revenue\", col.prev] * 1.08" }
      }
    },
    {
      "id": "cogs",
      "label": "Cost of Goods Sold",
      "format": "currency",
      "expr": "IncomeStatement[\"revenue\", @] * 0.45"
    },
    {
      "id": "gross_profit",
      "label": "Gross Profit",
      "format": "currency_total",
      "expr": "IncomeStatement[\"revenue\", @] - IncomeStatement[\"cogs\", @]"
    },
    {
      "id": "tax_expense",
      "label": "Income Tax Provision",
      "format": "currency",
      "expr": "IncomeStatement[\"gross_profit\", @] * Assumptions[\"tax_rate\", \"value\"]"
    }
  ]
}
```

See [Addressing Syntax](addressing.md) and [Formula Grammar](grammar.md) for expression details.
