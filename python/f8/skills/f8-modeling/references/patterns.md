# Financial Modeling Patterns

Canonical structural wiring templates for recurring financial and analytical modeling patterns in `f8`.

---

## 1. Roll-Forward Corkscrews

A roll-forward corkscrew models balance sheet schedules (debt, cash, fixed assets, inventory) where period balances propagate forward:

$$\text{Ending Balance}_t = \text{Beginning Balance}_t + \text{Additions}_t - \text{Deductions}_t$$
$$\text{Beginning Balance}_t = \text{Ending Balance}_{t-1}$$

### Structural Template

```python
# 1. Beginning balance: Lagged ending balance from previous period
# The initial base period requires a literal input in `cells`
table.add_row(
    "beginning",
    cells={"line_item": "Beginning Balance", "base_period": initial_balance},
    expr='Table["ending", col.prev]',
)

# 2. Additions / Inflows
table.add_row(
    "additions",
    cells={"line_item": "New Additions"},
    expr=...,
)

# 3. Deductions / Amortization
table.add_row(
    "deductions",
    cells={"line_item": "Reductions"},
    expr=...,
)

# 4. Ending balance: Beginning + Additions - Deductions
table.add_row(
    "ending",
    cells={"line_item": "Ending Balance"},
    expr='Table["beginning", @col] + Table["additions", @col] - Table["deductions", @col]',
    style="total",
)
```

> **Key Rule**: The opening balance of the first measure column has no predecessor. Provide its value explicitly in the `cells` dictionary for that base column.

---

## 2. Cross-Table Links

Isolate exogenous drivers, valuation multiples, and macro inputs in dedicated tables and reference them across statements.

### Structural Template

```python
# Drivers Table (scalar inputs)
drivers.add_row("rev_growth", cells={"driver": "Revenue Growth Rate", "val": 0.15})
drivers.add_row("tax_rate", cells={"driver": "Tax Rate", "val": 0.21})

# Financial Statement Table (referencing Drivers)
pnl.add_row(
    "revenue",
    cells={"line_item": "Revenue", "2023A": 1000.0},
    expr='IncomeStatement["revenue", col.prev] * (1.0 + Drivers["rev_growth", "val"])',
)

pnl.add_row(
    "tax_expense",
    cells={"line_item": "Income Tax Expense"},
    expr='IncomeStatement["ebt", @col] * Drivers["tax_rate", "val"]',
)
```

> **Key Rule**: Use `Drivers["driver_row", "val"]` when referencing a fixed scalar driver coordinate. Use `Drivers["driver_row", @col]` if the driver table shares the same timeline columns as the consumer table.

---

## 3. Multi-Tier Subtotals & Grand Totals

Implement standard accounting border conventions (`style="subtotal"` and `style="total"`) with section spacing.

### Structural Template

```python
# Primary line items
table.add_row("sales_hardware", cells={"line_item": "Hardware Sales"}, expr=...)
table.add_row("sales_services", cells={"line_item": "Services Revenue"}, expr=...)

# Intermediate Subtotal (thin top border)
table.add_row(
    "total_revenue",
    cells={"line_item": "Total Revenue"},
    expr='Table["sales_hardware", @col] + Table["sales_services", @col]',
    style="subtotal",
)

table.add_row_gutter()

# Next operational block
table.add_row("cogs", cells={"line_item": "Cost of Goods Sold"}, expr=...)

# Grand Total (thin top border and double bottom underline)
table.add_row(
    "gross_profit",
    cells={"line_item": "Gross Profit"},
    expr='Table["total_revenue", @col] - Table["cogs", @col]',
    style="total",
)
```

---

## 4. Multi-Tier Column Headers

Group timeline periods (quarters under years) or scenarios (cases under metrics) using stacked column headers.

### Structural Template

```python
# Descriptor column
table.add_column("metric", role="descriptor", headers=["Period", "Line Item"])

# Grouped measure columns
table.add_column("2024Q1", role="measure", headers=["FY 2024", "Q1"])
table.add_column("2024Q2", role="measure", headers=["FY 2024", "Q2"])
table.add_column("2025Q1", role="measure", headers=["FY 2025", "Q1"])
table.add_column("2025Q2", role="measure", headers=["FY 2025", "Q2"])
```
