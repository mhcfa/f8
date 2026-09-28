# Syntax and Grid Reference

Specification of the `f8` 2D coordinate grammar, vector formula semantics, column roles, layout elements, and visual grid formatting.

---

## 1. 2D Coordinate Addressing Grammar

All cell references in `f8` require explicit 2D coordinates in bracket syntax:

$$\text{Table}[\text{RowSelector}, \text{ColSelector}]$$

### Grammar & Coordinate Types

| Coordinate Type | Syntax | Semantics | Example |
| :--- | :--- | :--- | :--- |
| **Active column** | `Table["row", @col]` | Evaluates at the current measure column | `IS["revenue", @col] * 0.40` |
| **Active row** | `Table[@row, "col"]` | Evaluates at the current row | `Assumptions[@row, "rate"] * 100` |
| **Absolute point** | `Table["row", "col"]` | Fixed coordinate across all evaluations | `Assumptions["tax_rate", "val"]` |
| **Temporal lag (1)** | `Table["row", col.prev]` | Prior measure column | `BS["cash", col.prev] + 10.0` |
| **Temporal lag ($k$)** | `Table["row", col.prev(k)]` | $k$ measure columns prior | `Rev["sales", col.prev(2)]` |
| **Temporal lead** | `Table["row", col.next]` | Next measure column | `Proj["milestone", col.next]` |
| **Column slice** | `Table["row", "c1".."c2"]` | Continuous horizontal range | `SUM(Rev["sales", "2023A".."2025E"])` |

### Syntax Invariants

- **Quoted string identifiers**: All table, row, and column identifiers in formulas must be double-quoted strings (e.g. `["revenue", @col]`). Bare identifiers raise `FormulaParseError`.
- **Explicit axis specifiers**: Active coordinates require `@col` or `@row`. Bare `@` is rejected.
- **No dot notation**: Dot access such as `Table.revenue` or `Table.revenue[@col]` is prohibited.
- **Uppercase functions and booleans**: Built-in functions and literals must be uppercase (`SUM`, `AVERAGE`, `IF`, `MIN`, `MAX`, `TRUE`, `FALSE`). Lowercase identifiers raise `FormulaParseError`.
- **Omitted function arguments**: Optional parameters are explicitly omitted using `_` (e.g. `PMT(rate, nper, pv, _, type)`).

---

## 2. Column Specifications & Roles

Columns define the horizontal axes and calculation boundaries of a table.

### Method Signature

```python
table.add_column(
    id: str,
    *,
    role: "measure" | "descriptor" = "measure",
    header: str | None = None,
    headers: list[str] | None = None,
    format: str | None = None,
    decimals: int | None = None,
    total: str | None = None,
    width: float | None = None,
) -> ColumnSpec
```

### Column Roles

- `role="descriptor"`: Metadata, line-item titles, unit descriptions, notes. Excluded from vector formula expansion.
- `role="measure"` (default): Quantitative timeline periods, scenario buckets, numeric data. Vector formulas expand across measure columns from left to right.

### Multi-Tier Headers

Supply `headers` with a list of strings for stacked grouping:

```python
table.add_column("2024Q1", role="measure", headers=["FY 2024", "Q1"])
table.add_column("2024Q2", role="measure", headers=["FY 2024", "Q2"])
```

---

## 3. Row Specifications & Vector Formulas

Rows define the vertical axis and primary calculation rules of a table.

### Method Signature

```python
table.add_row(
    id: str,
    *,
    cells: dict[str, Any] | None = None,
    expr: str | None = None,
    style: "data" | "subtotal" | "total" = "data",
    format: str | None = None,
    decimals: int | None = None,
    notes: dict[str, str] | None = None,
) -> RowSpec
```

### Formula Evaluation & Overrides

1. **Vector expansion**: If `expr` is provided, it is evaluated across all `measure` columns from left to right.
2. **Point overrides**: Explicit values in `cells` supersede `expr` for that column (common for base/historical periods).
3. **Manual updates**: `table.set_cell(row_id, col_id, *, value=..., expr=..., note=...)` overrides individual coordinates dynamically.

```python
# Base period literal override with vector projection across subsequent periods
table.add_row(
    "revenue",
    cells={"item": "Revenue", "2023A": 1000.0},
    expr='Operations["revenue", col.prev] * 1.10',
)
```

---

## 4. Visual Layout Elements

Visual formatting elements establish publication hierarchy and spacing without affecting calculation dependencies.

### Section Headers

Spans across all table columns as a full-width divider:

```python
table.add_section_header(title: str, *, indent: int = 0)
```

### Gutters

Inserts empty rows or columns for visual spacing:

```python
table.add_row_gutter(*, height: float | None = None)
table.add_column_gutter(*, width: float | None = None)
```

### Layout Sequencing Pattern

```python
table.add_section_header("Revenue & Direct Costs")
table.add_row("rev", cells={"item": "Gross Revenue"}, expr=...)
table.add_row("cogs", cells={"item": "Cost of Goods Sold"}, expr=...)

table.add_row_gutter()

table.add_section_header("Operating Expenses", indent=1)
table.add_row("sga", cells={"item": "SG&A Expense"}, expr=...)
```

---

## 5. Cell-Level Attributes & Indentation

Hierarchical row labels require visual indentation. Indentation is attached directly to the `Cell` object on descriptor columns, never on the row itself.

### Cell Signature

```python
Cell(
    value: Any,
    *,
    indent: int = 0,
    format: str | None = None,
    decimals: int | None = None,
    style: str | None = None,
    note: str | None = None,
)
```

### Indentation Hierarchy Pattern

```python
# Level 0 (no indent)
table.add_row("gross_profit", cells={"item": "Gross Profit"})

# Level 1 (child item)
table.add_row("software_rev", cells={"item": Cell("Software Licenses", indent=1)})

# Level 2 (sub-child item)
table.add_row("cloud_rev", cells={"item": Cell("Cloud Subscriptions", indent=2)})
```

---

## 6. Formatting Presets & Semantic Styles

### Format Presets

Formatting applies hierarchically: cell override $\rightarrow$ row default $\rightarrow$ column default $\rightarrow$ table default.

| Preset | Description | Example Output |
| :--- | :--- | :--- |
| `"currency"` | Formatted currency with table currency symbol | `$1,250.00` |
| `"percentage"` | Scaled percentage | `15.0%` |
| `"number"` | Standard delimited numeric value | `1,250.00` |
| `"integer"` | Delimited integer without decimals | `1,250` |
| `"multiple"` | Valuation multiple | `12.5x` |
| `"basis_points"` | Basis points | `150 bps` |
| `"date"` | ISO date string | `2024-12-31` |
| `"text"` | Plain text | `Sample` |

### Semantic Styles

Row styles control accounting borders and typography:

- `style="data"` (default): Standard data row.
- `style="subtotal"`: Top thin line separator for subtotals.
- `style="total"`: Top thin line and bottom double-underline for grand totals.
