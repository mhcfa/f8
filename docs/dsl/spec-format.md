# Specification Format

This document details the YAML/JSON specification schema, row partitioning rules, and authoring patterns for Finx models.

## 1. Specification Schema Overview

A model specification consists of top-level metadata and a hierarchy of sheets, tables, and rows:

```yaml
model_name: string
theme: string

sheets:
  - name: string
    tab_color?: string
    freeze_panes?: { row: integer, col: integer }
    show_gridlines?: boolean
    tables:
      - id: string
        title: string
        columns: [string]
        timeline?:
          <group_name>: [string]
        rows:
          - id: string
            label: string
            format?: string
            # Value definition: either uniform or partitioned
            value?: [number | string | null]
            expr?: string
            match?:
              <column_or_range_selector>:
                value?: [number | string | null]
                expr?: string
```

---

## 2. Core Invariants

- **Canonical core**: Specifications strictly enforce unambiguous canonical forms with zero implicit type coercion or broadcasting.
- **Explicit input arrays**: All literal inputs are provided as explicit arrays: `value: [...]`.
- **Strict 1-to-1 dimension match**: The length of `value` must equal the number of targeted columns (`len(value) == len(target_columns)`). Single-cell inputs require single-element arrays: `value: [0.25]`.
- **Explicit empty cells**: Empty cells must be explicitly represented with `null`, e.g. `[null, 92000, 100000]`.
- **Unified partitioning (`match`)**: The `match` block is the sole mechanism for varying behavior across columns.
- **Exhaustiveness checking**: Every column in the table must be covered by a `match` selector or a `default` arm. Missing columns produce compile-time errors.

---

## 3. Row Authoring Patterns

### 3.1. Uniform Calculation Across All Columns

When a formula applies identically to all columns in a table:

```yaml
- id: "gross_profit"
  label: "Gross Profit"
  format: "currency_total"
  expr: "IncomeStatement.revenue[@] - IncomeStatement.cogs[@]"
```

### 3.2. Single Assumption (1-Column Table)

Assumptions and global parameters are modeled as single-column tables:

```yaml
- id: "tax_rate"
  label: "Corporate Tax Rate"
  format: "percentage"
  value: [0.25]
```

### 3.3. Partitioned Row (Historical Actuals vs Forecast)

Separates hardcoded historical values from formula-driven forecast projections:

```yaml
- id: "revenue"
  label: "Total Revenue"
  format: "currency"
  match:
    "2021A".."2023A": { value: [85000, 92000, 100000] }
    "2024E".."2028E": { expr: "IncomeStatement.revenue[col.prev] * 1.08" }
```

### 3.4. Corkscrew Roll-Forward with Base Anchor

Models balance sheet or debt roll-forwards with an explicit initial period:

```yaml
- id: "beginning_cash"
  label: "Beginning Cash Balance"
  format: "currency"
  match:
    "2024E":  { expr: "Assumptions.initial_cash[\"value\"]" }
    default:  { expr: "BalanceSheet.ending_cash[col.prev]" }
```

---

## 4. Column Groups & Structured Timelines

Columns can be grouped into semantic phases using the `timeline` block:

```yaml
timeline:
  historical: ["2021A", "2022A", "2023A"]
  forecast: ["2024E", "2025E", "2026E", "2027E", "2028E"]
```

### Compiler Behaviors
- **Alias resolution**: References such as `cols.historical.last` or `cols.forecast.first` resolve to concrete column keys during grid expansion.
- **Automated styling**: The layout engine detects the boundary between timeline groups and applies standard vertical dividing borders.

---

## 5. End-to-End Model Example

```yaml
model_name: "LBO_Simple"
theme: "corporate_navy"

sheets:
  - name: "Summary"
    tab_color: "navy"
    tables:
      - id: "Assumptions"
        title: "Model Assumptions"
        columns: ["value"]
        rows:
          - id: "tax_rate"
            label: "Corporate Tax Rate"
            format: "percentage"
            value: [0.25]
          - id: "exit_multiple"
            label: "Exit EV / EBITDA Multiple"
            format: "multiple"
            value: [10.0]

  - name: "Model"
    tab_color: "forest_green"
    tables:
      - id: "IncomeStatement"
        title: "Income Statement"
        columns: ["2023A", "2024E", "2025E", "2026E", "2027E"]
        timeline:
          historical: ["2023A"]
          forecast: ["2024E", "2025E", "2026E", "2027E"]
        rows:
          - id: "revenue"
            label: "Total Revenue"
            format: "currency"
            match:
              "2023A": { value: [100000.0] }
              cols.forecast: { expr: "IncomeStatement.revenue[col.prev] * 1.08" }

          - id: "cogs"
            label: "Cost of Goods Sold"
            format: "currency"
            expr: "IncomeStatement.revenue[@] * 0.45"

          - id: "gross_profit"
            label: "Gross Profit"
            format: "currency_total"
            expr: "IncomeStatement.revenue[@] - IncomeStatement.cogs[@]"

          - id: "tax_expense"
            label: "Income Tax Provision"
            format: "currency"
            expr: "IncomeStatement.gross_profit[@] * Assumptions.tax_rate[\"value\"]"
```

See [Addressing Syntax](addressing.md) and [Formula Grammar](grammar.md) for expression details.
