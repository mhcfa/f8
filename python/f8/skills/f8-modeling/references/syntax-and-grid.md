# Syntax and Grid Reference

2D coordinate grammar, vector lowering semantics, column roles, layout elements, and formatting presets.

---

## 1. 2D Coordinate Addressing Grammar

All cell references require explicit 2D coordinates in bracket syntax:

`Table[RowSelector, ColSelector]`

### Grammar and Coordinate Types

| Coordinate type | Syntax | Semantics | Example |
| :--- | :--- | :--- | :--- |
| **Active column** | `Table["row", @col]` | Current measure column | `IS["revenue", @col] * 0.40` |
| **Active row** | `Table[@row, "col"]` | Current data row | `Assumptions[@row, "rate"] * 100` |
| **Absolute point** | `Table["row", "col"]` | Fixed coordinate across all evaluations | `Assumptions["tax_rate", "val"]` |
| **Column lag (1)** | `Table["row", col.prev]` | Prior measure column | `BS["cash", col.prev] + 10.0` |
| **Column lag (k)** | `Table["row", col.prev(k)]` | `k` measure columns prior | `Rev["sales", col.prev(2)]` |
| **Column lead (1)** | `Table["row", col.next]` | Next measure column | `Proj["milestone", col.next]` |
| **Column lead (k)** | `Table["row", col.next(k)]` | `k` measure columns ahead | `Proj["milestone", col.next(2)]` |
| **Row lag (1)** | `Table[row.prev, "col"]` | Prior data row | `Table[row.prev, "col"] * 1.05` |
| **Row lag (k)** | `Table[row.prev(k), "col"]` | `k` data rows prior | `Table[row.prev(2), "col"]` |
| **Row lead (1)** | `Table[row.next, "col"]` | Next data row | `Table[row.next, "col"]` |
| **Row lead (k)** | `Table[row.next(k), "col"]` | `k` data rows ahead | `Table[row.next(2), "col"]` |
| **Column slice** | `Table["row", "c1".."c2"]` | Continuous horizontal range across columns | `SUM(Rev["sales", "2023A".."2025E"])` |
| **Row slice (active col)** | `Table["r1".."r2", @col]` | Continuous vertical range across rows at active column | `SUM(Opex["r_and_d".."g_and_a", @col])` |
| **Row slice (fixed col)** | `Table["r1".."r2", "col"]` | Continuous vertical range across rows at fixed column | `SUM(Comps["apple".."google", "market_cap"])` |
| **2D block slice** | `Table["r1".."r2", "c1".."c2"]` | Continuous 2D rectangular matrix range | `SUM(Matrix["r1".."r3", "2023A".."2025E"])` |

### Syntax Invariants

- Table identifiers: Bare identifiers before the bracket (e.g. `Table[...]`, `IncomeStatement[...]`). Quoted names (`"Table"[...]`) raise `FormulaParseError`.
- Quoted keys: Row and column keys inside brackets must be double-quoted strings (e.g. `["revenue", @col]`, `["tax_rate", "val"]`). Bare identifiers inside brackets raise `FormulaParseError` (`ParseError::UnquotedCoordinateKey`).
- Explicit axis specifiers: Active coordinates require `@col` or `@row`. Bare `@` raises `FormulaParseError` (`ParseError::RetiredBareAt`).
- No dot notation: Dot access (`Table.revenue`, `Table.revenue[@col]`) raises `FormulaParseError`.
- Uppercase identifiers: Built-in functions (`SUM`, `AVERAGE`, `IF`, `MIN`, `MAX`, `XLOOKUP`, `NPV`, etc.) and boolean literals (`TRUE`, `FALSE`) must be uppercase. Lowercase identifiers raise `FormulaParseError`.
- No null literal: The `null` literal is prohibited in formulas and raises `FormulaParseError`.
- Algebraic precedence:
  1. Exponentiation (`^`, tier 5, right-associative)
  2. Unary prefix (`+`, `-`, tier 4, right-associative)
  3. Multiplicative (`*`, `/`, tier 3, left-associative)
  4. Additive (`+`, `-`, tier 2, left-associative)
  5. Relational comparisons (`=`, `<>`, `<`, `<=`, `>`, `>=`, tier 1, left-associative; equality is `=`, inequality is `<>`).
- Omitted arguments: Positional parameters are omitted via `_` (e.g. `PMT(rate, nper, pv, _, type)`). Restricted to function argument lists; using `_` in arithmetic or coordinate selectors raises `FormulaParseError`.
- Slice ordering: Range slices (`"k1".."k2"`) must follow declared table axis order (`start_index <= end_index`). Inverted ranges raise `ReferenceResolutionError`.
- Temporal bounds: `col.prev` and `col.next` navigate measure columns only, automatically skipping descriptor columns. Offsets extending outside table bounds raise `ReferenceResolutionError`.

---

## 2. Column Specifications and Roles

Define horizontal axes, layout geometry, and calculation boundaries.

### Method Signature

```python
table.add_column(
    id: str,
    role: AxisRole = "measure",
    header: str | None = None,
    headers: list[str] | None = None,
    style: SemanticStyle | None = None,
    highlight: bool | None = None,
    format: FormatPreset | None = None,
    decimals: int | None = None,
    currency: CurrencyCode | None = None,
    expr: str | None = None,
) -> Table
```

### Column Roles

- `role="descriptor"`: Metadata, line-item titles, unit descriptions, and audit notes. Excluded from vector formula expansion.
- `role="measure"` (default): Quantitative timeline periods, scenario buckets, and numeric metrics. Vector formulas expand across measure columns from left to right.

### Headers and Formatting Defaults

- Single vs. multi-tier headers: Provide `header="Title"` for single-tier or `headers=["FY 2024", "Q1"]` for stacked tiers (mutually exclusive; specifying both raises `ValueError`).
- Multi-tier bottom alignment: When columns have unequal header depths, columns with fewer headers are padded with blank super-headers at the top so metric labels align to the bottom tier.
- Column defaults and conflicts: Column-level `format`, `decimals`, `currency`, and `style` apply when the row does not specify them. Differing row and column values without a 0D cell override raise conflict errors (`FormatConflict`, `DecimalsConflict`, `CurrencyConflict`).
- Fluent chaining: Returns `Table` to support method chaining.

```python
table.add_column("line_item", role="descriptor", header="Line Item")
table.add_column("2024Q1", role="measure", headers=["FY 2024", "Q1"], format="currency", decimals=1)
table.add_column("2024Q2", role="measure", headers=["FY 2024", "Q2"], format="currency", decimals=1)
```

### Column Vector Formulas (Vertical Expansion)

When `expr` is provided on `table.add_column(...)`, it evaluates down across all data rows from top to bottom.
- Active row addressing: References active row via `@row` and relative offsets via `row.prev` / `row.next`.
- Cross-sectional modeling: Standard for Cap Tables, Trading Comps, and Portfolio Schedules where rows represent entities and columns represent computed metrics.

```python
table.add_column(
    "market_cap",
    role="measure",
    header="Market Cap",
    expr='Comps[@row, "share_price"] * Comps[@row, "shares_out"]',
    format="currency",
    decimals=1,
)
```

---

## 3. Row Specifications and Vector Formulas

Define vertical axis and primary calculation rules.

### Method Signature

```python
table.add_row(
    id: str,
    cells: Mapping[str, CellInputValue] | None = None,
    expr: str | None = None,
    style: SemanticStyle | None = None,
    highlight: bool | None = None,
    format: FormatPreset | None = None,
    decimals: int | None = None,
    currency: CurrencyCode | None = None,
) -> Table
```

### Row Vector Formulas (Horizontal Expansion)

When `expr` is provided on `table.add_row(...)`, it evaluates across all `measure` columns from left to right.
- Active column addressing: References active column via `@col` and temporal lags via `col.prev` / `col.next`.
- Descriptor isolation: Never broadcasts across `role="descriptor"` columns.
- Timeline modeling: Standard for P&L, Balance Sheet, and Cash Flow statements.
- Base period anchor: Because `col.prev` at the first measure column has no predecessor, provide its base value explicitly in `cells`.

```python
table.add_row(
    "revenue",
    cells={"line_item": "Revenue", "2023A": 1000.0},
    expr='Operations["revenue", col.prev] * 1.10',
    format="currency",
    decimals=1,
)
```

---

## 4. 3-Tier Specificity Lattice and Ambiguity Rules

Cell coordinates resolve through a deterministic 3-tier specificity lattice:

1. Tier 1 (0D point cell override):
   Explicit cell inputs in `cells` (literal values, `Cell(value=...)`, `Cell(expr=...)`) and dynamic updates via `table.set_cell(...)` take highest precedence, overriding any row or column vector formula, format, decimals, or currency.
2. Tier 2 (1D vector rules: row XOR col):
   - Vector formulas: `row.expr` XOR `col.expr`. Intersecting row and column expressions without a 0D cell override raise `SpecificationError` (`AmbiguousExpressionConflict`).
   - Vector conflicts: Differing row and column `format`, `decimals`, or `currency` without a 0D cell override raise `SpecificationError` (`FormatConflict`, `DecimalsConflict`, `CurrencyConflict`).
   - Symmetrical 2D border composition: Rows control horizontal borders (`style="subtotal"` thin top; `style="total"` thin top and double-bottom); columns control vertical borders. Row `"total"` + Column `"total"` composes into a 2D accounting total box (`is_bold=True`). Setting `style="data"` on a 0D cell override resets both axes to standard data.
   - Highlight propagation: Cells inherit highlight if either row or column has `highlight=True`, unless explicitly overridden by `highlight=False` on a 0D cell override.
3. Tier 3 (Descriptor boundaries and unassigned cells):
   - Descriptor isolation: Vector formulas never broadcast across descriptor columns (`role="descriptor"`).
   - Unassigned cell enforcement: Every calculable measure cell requires an explicit value, vector formula, or cell override. Unassigned measure coordinates raise `SpecificationError` (`UnassignedCell`). Descriptor cells lacking an explicit label also raise `SpecificationError` (`UnassignedCell`).

---

## 5. Cell-Level Specifications and Overrides

### Cell Signature and Invariants

Represents a discrete 0D cell override specifying a constant scalar value or explicit cell formula:

```python
@dataclass(frozen=True)
class Cell:
    value: CellValueLiteral | _Unset = _UNSET
    expr: str | None = None
    style: SemanticStyle | None = None
    highlight: bool | None = None
    format: FormatPreset | None = None
    decimals: int | None = None
    currency: CurrencyCode | None = None
    indent: int | None = None
    note: str | None = None
```

- Payload exclusivity: Must specify either `value` or `expr`, never both (raises `ValueError` in Python, `SpecificationError` in compiler).
- Allowed values: `CellValueLiteral = float | int | str | bool | None`.
- Indentation constraint: `indent: int` restricted to descriptor cells (`role="descriptor"`). Setting `indent > 0` on a measure cell raises `SpecificationError` (`InvalidIndentOnMeasureCell`).
- Currency constraint: Permitted only when effective format is `"currency"`. Specifying on other presets raises `SpecificationError` (`InvalidCurrencyForPreset`).
- Precision constraint: Permitted only on numeric presets (`currency`, `number`, `percentage`, `multiple`, `basis_points`, `integer`). Specifying on `date` or `text` raises `SpecificationError` (`InvalidPrecisionForPreset`). Decimals cannot exceed 14 (`PrecisionOutOfBounds`).
- Highlight: Boolean flag (`highlight=True`) applying visual fill styling to callout drivers or summary outputs.
- 0D note isolation: Free-form audit comment attached to a discrete coordinate. Notes never broadcast across rows or columns; vector-generated cells have `note=None`.

### Input Types for `cells`

```python
CellInputValue = CellValueLiteral | Cell | DetailedCellDict
```

```python
# 1. Bare scalar literal
table.add_row("sales", cells={"line_item": "Sales", "2023A": 1000.0})

# 2. Rich Cell object with indentation and highlighting
table.add_row(
    "software_rev",
    cells={
        "line_item": Cell("Software Licenses", indent=1),
        "2023A": Cell(600.0, highlight=True, note="Audited base"),
    },
)

# 3. Cell formula expression override
table.add_row(
    "tax_override",
    cells={"2024E": Cell(expr='Assumptions["fixed_tax", "val"]')},
)

# 4. Overriding column vector expression on summary row
table.add_row(
    "total",
    cells={"market_cap": Cell(expr='SUM(Comps["apple".."google", "market_cap"])')},
    style="total",
)
```

### Dynamic Coordinate Updates: `table.set_cell`

Updates or patches existing cell coordinates without redefining the row:

```python
table.set_cell(
    row: str,
    col: str,
    cell: CellInputValue | _Unset = _UNSET,
    value: CellValueLiteral | _Unset = _UNSET,
    expr: str | None = None,
    style: SemanticStyle | None = None,
    highlight: bool | None = None,
    format: FormatPreset | None = None,
    decimals: int | None = None,
    currency: CurrencyCode | None = None,
    indent: int | None = None,
    note: str | None = None,
) -> Table
```

```python
table.set_cell("revenue", "2024E", highlight=True, note="Revised Q3 forecast")
```

---

## 6. Visual Layout Elements

Visual hierarchy and spacing elements without affecting calculation dependencies.

### Section Headers

Full-width table divider without cell merging:

```python
table.add_section_header(
    title: str,
    indent: int | None = None,
    note: str | None = None,
) -> Table
```

- Consumes 1 physical row, allocating zero calculation cells.
- Supports visual indentation (`indent: int`) and audit footnotes (`note: str`).

### Gutters

Insert empty rows or columns for visual spacing (zero arguments):

```python
table.add_row_gutter() -> Table
table.add_column_gutter() -> Table
```

- Row gutter: Consumes 1 physical row sized to `theme.metrics.gutter_row_height` (9.0 pt) with zero calculation cells.
- Column gutter: Consumes 1 physical column sized to `theme.metrics.gutter_col_width` with zero calculation cells.
- Vector formulas automatically skip gutter rows and columns.

### Worksheet Margins and Spacing

- Default margins: Row 0 reserved as top gutter; Col 0 (Column A) reserved as left gutter. Primary table starts at cell `B2` (`col: 1, row: 1`).
- Vertical stacking: Multiple tables on a single worksheet stack vertically in declared order, separated automatically by 2 blank gutter rows.
- Table banners: Tables with `title` or `subtitle` emit banner rows followed by an 8.0 pt buffer gutter before column headers.
- Width arbitration: Sheet-wide column widths arbitrate across stacked tables, accounting for indentation (`indent * 2` chars) and enforcing `theme.metrics.min_col_width` (12.0 chars) on measure columns.

### Layout Sequencing Pattern

```python
table.add_section_header("Revenue & Direct Costs")
table.add_row("rev", cells={"line_item": "Gross Revenue"}, expr=...)
table.add_row("cogs", cells={"line_item": Cell("Cost of Goods Sold", indent=1)}, expr=...)

table.add_row_gutter()

table.add_section_header("Operating Expenses", indent=1)
table.add_row("sga", cells={"line_item": "SG&A Expense"}, expr=...)
```

---

## 7. Formatting Presets and Semantic Styles

### Format Presets (`FormatPreset`)

Precedence: `Cell override > (Row rule XOR Column rule) > Table default > Model theme`.

| Preset | Description | Example output |
| :--- | :--- | :--- |
| `"currency"` | Formatted currency with table currency symbol | `$1,250.00` |
| `"percentage"` | Scaled percentage | `15.0%` |
| `"number"` | Standard delimited numeric value | `1,250.00` |
| `"integer"` | Undelimited integer without thousand separators | `2024` |
| `"multiple"` | Valuation multiple | `12.5x` |
| `"basis_points"` | Basis points | `150 bps` |
| `"date"` | ISO date string | `2024-12-31` |
| `"text"` | Plain text | `Sample` |

### Semantic Styles (`SemanticStyle`)

Row and column styles control accounting borders and typography:

- `style="data"` (default): Standard data row/column.
- `style="subtotal"`: Top thin line separator for subtotals.
- `style="total"`: Top thin line and bottom double-underline for grand totals (renders bold typography).
- Orthogonal 2D composition: Row styles control horizontal borders; column styles control vertical borders. Row `"total"` + Column `"total"` composes into a complete 2D accounting total box.

### Currency Codes (`CurrencyCode`)

Accepts ISO 3-letter currency codes (case-insensitive):

`USD`, `EUR`, `GBP`, `JPY`, `CHF`, `CAD`, `AUD`, `CNY`, `HKD`, `SGD`, `SEK`, `NOK`, `NZD`, `INR`, `BRL`

- Falls back to `"USD"` if unspecified on the table.
- Permitted only on cells, rows, or columns with effective format `"currency"`. Specifying currency on non-currency presets raises `SpecificationError` (`InvalidCurrencyForPreset`).
