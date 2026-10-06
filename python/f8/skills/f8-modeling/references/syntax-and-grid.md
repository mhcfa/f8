# Syntax and Grid Reference

Specification of the `f8` 2D coordinate grammar, vector formula semantics, column roles, layout elements, and visual grid formatting.

---

## 1. 2D Coordinate Addressing Grammar

All cell references in `f8` require explicit 2D coordinates in bracket syntax:

`Table[RowSelector, ColSelector]`

### Grammar & Coordinate Types

| Coordinate type | Syntax | Semantics | Example |
| :--- | :--- | :--- | :--- |
| **Active column** | `Table["row", @col]` | Evaluates at the current measure column | `IS["revenue", @col] * 0.40` |
| **Active row** | `Table[@row, "col"]` | Evaluates at the current row | `Assumptions[@row, "rate"] * 100` |
| **Absolute point** | `Table["row", "col"]` | Fixed coordinate across all evaluations | `Assumptions["tax_rate", "val"]` |
| **Column lag (1)** | `Table["row", col.prev]` | Prior measure column | `BS["cash", col.prev] + 10.0` |
| **Column lag ($k$)** | `Table["row", col.prev(k)]` | $k$ measure columns prior | `Rev["sales", col.prev(2)]` |
| **Column lead (1)** | `Table["row", col.next]` | Next measure column | `Proj["milestone", col.next]` |
| **Column lead ($k$)** | `Table["row", col.next(k)]` | $k$ measure columns ahead | `Proj["milestone", col.next(2)]` |
| **Row lag (1)** | `Table[row.prev, "col"]` | Prior data row | `Table[row.prev, "col"] * 1.05` |
| **Row lag ($k$)** | `Table[row.prev(k), "col"]` | $k$ data rows prior | `Table[row.prev(2), "col"]` |
| **Row lead (1)** | `Table[row.next, "col"]` | Next data row | `Table[row.next, "col"]` |
| **Row lead ($k$)** | `Table[row.next(k), "col"]` | $k$ data rows ahead | `Table[row.next(2), "col"]` |
| **Column slice** | `Table["row", "c1".."c2"]` | Continuous horizontal range across columns | `SUM(Rev["sales", "2023A".."2025E"])` |
| **Row slice (active col)** | `Table["r1".."r2", @col]` | Continuous vertical range across rows at current column | `SUM(Opex["r_and_d".."g_and_a", @col])` |
| **Row slice (fixed col)** | `Table["r1".."r2", "col"]` | Continuous vertical range across rows at fixed column | `SUM(Comps["apple".."google", "market_cap"])` |
| **2D block slice** | `Table["r1".."r2", "c1".."c2"]` | Continuous 2D rectangular matrix range | `SUM(Matrix["r1".."r3", "2023A".."2025E"])` |

### Syntax Invariants

- **Table identifiers**: Table identifiers must be bare identifiers before the bracket (e.g. `Table[...]`, `IncomeStatement[...]`). Quoting the table name (e.g. `"Table"[...]`) raises `FormulaParseError`.
- **Quoted row and column keys**: Row and column keys inside brackets must be double-quoted strings (e.g. `["revenue", @col]`, `["tax_rate", "val"]`). Bare identifiers inside brackets raise `FormulaParseError` (`ParseError::UnquotedCoordinateKey`).
- **Explicit axis specifiers**: Active coordinates require `@col` or `@row`. Bare `@` is retired and raises `FormulaParseError` (`ParseError::RetiredBareAt`).
- **No dot notation**: Dot access such as `Table.revenue` or `Table.revenue[@col]` is prohibited and raises `FormulaParseError`.
- **Uppercase functions and booleans**: Built-in functions (`SUM`, `AVERAGE`, `IF`, `MIN`, `MAX`, `XLOOKUP`, `NPV`, etc.) and boolean literals (`TRUE`, `FALSE`) must be uppercase. Lowercase function names or literals raise `FormulaParseError`.
- **Complete purge of `null`**: The `null` literal is prohibited in formula expressions and raises `FormulaParseError`.
- **Operators and algebraic precedence**:
  1. Exponentiation (`^`, tier 5, right-associative)
  2. Unary prefix (`+`, `-`, tier 4, right-associative)
  3. Multiplicative (`*`, `/`, tier 3, left-associative)
  4. Additive (`+`, `-`, tier 2, left-associative)
  5. Relational comparisons (`=`, `<>`, `<`, `<=`, `>`, `>=`, tier 1, left-associative). Equality is `=` and inequality is `<>`.
- **Omitted function arguments**: Optional parameters are explicitly omitted using `_` (e.g. `PMT(rate, nper, pv, _, type)` or `IF(test, true_val, _)`). The `_` token is strictly confined to function call argument lists; using `_` in arithmetic or coordinate selectors raises `FormulaParseError`.
- **Slice ordering and declaration bounds**: Range slices (`"k1".."k2"`) must follow declared table axis order (non-inverted; start index $\le$ end index). Inverted ranges raise `ReferenceResolutionError`.
- **Temporal offset bounds**: `col.prev` and `col.next` navigate across measure columns only, automatically skipping descriptor columns. Evaluated offsets that fall outside the table's measure boundaries raise `ReferenceResolutionError`.

---

## 2. Column Specifications & Roles

Columns define the horizontal axes, layout geometry, and calculation boundaries of a table.

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

### Headers & Formatting Defaults

- **Single vs. multi-tier headers**: Provide `header="Title"` for single-tier headers or `headers=["FY 2024", "Q1"]` for stacked grouping (mutually exclusive; specifying both raises `ValueError`).
- **Multi-tier bottom alignment**: When columns have unequal header depths ($H = \max(\text{headers.len()})$), columns with fewer headers are padded with blank super-headers at the top so metric labels always align to the bottom tier.
- **Column-level defaults & conflicts**: Column-level `format`, `decimals`, `currency`, and `style` apply to cells in this column when the row does not specify them. If a measure row and measure column specify differing values without a cell override, the compiler raises a conflict error (`FormatConflict`, `DecimalsConflict`, `CurrencyConflict`).
- **Fluent chaining**: Returns `Table` to support chained builder calls.

```python
table.add_column("line_item", role="descriptor", header="Line Item")
table.add_column("2024Q1", role="measure", headers=["FY 2024", "Q1"], format="currency", decimals=1)
table.add_column("2024Q2", role="measure", headers=["FY 2024", "Q2"], format="currency", decimals=1)
```

### Column Vector Formulas (Vertical Expansion)

When `expr` is provided on `table.add_column(...)`, it defines a vertical vector formula evaluated down across all data rows from top to bottom.
- **Active row addressing**: Column expressions reference the active row via `@row` and relative row offsets via `row.prev` / `row.next`.
- **Cross-sectional modeling**: Ideal for Cap Tables, Trading Comps, and Portfolio Schedules where rows represent entities and columns represent computed metrics.

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

## 3. Row Specifications & Vector Formulas

Rows define the vertical axis and primary calculation rules of a table.

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

When `expr` is provided on `table.add_row(...)`, it defines a horizontal vector formula evaluated across all `measure` columns from left to right.
- **Active column addressing**: Row expressions reference the active column via `@col` and temporal column lags via `col.prev` / `col.next`.
- **Descriptor isolation**: Row expressions never broadcast across `role="descriptor"` columns.
- **Financial timeline modeling**: Ideal for P&L, Balance Sheet, and Cash Flow statements.
- **Base period literal anchor**: Because `col.prev` at the first measure column has no predecessor, provide its base value explicitly in `cells`.

```python
# Base period literal override with vector projection across subsequent periods
table.add_row(
    "revenue",
    cells={"line_item": "Revenue", "2023A": 1000.0},
    expr='Operations["revenue", col.prev] * 1.10',
    format="currency",
    decimals=1,
)
```

---

## 4. The 3-Tier Specificity Lattice & Ambiguity Rules

The `f8` compiler evaluates every cell coordinate $(r, c)$ using a deterministic 3-tier specificity lattice:

1. **Tier 1 (0D point cell override)**:
   Explicit cell inputs in `cells` (literal values, `Cell(value=...)`, or `Cell(expr=...)`) and dynamic updates via `table.set_cell(...)` take highest precedence, overriding any row or column vector formula, format, decimals, or currency.
2. **Tier 2 (1D vector rules: row XOR col)**:
   - **Vector formulas**: `row.expr` XOR `col.expr`. If both a row defines `row.expr` AND a column defines `col.expr` for the same intersecting coordinate without a 0D cell override, compilation halts with `SpecificationError` (`AmbiguousExpressionConflict`).
   - **Vector format conflict**: If measure row format != measure column format without a 0D cell override, compilation halts with `SpecificationError` (`FormatConflict`).
   - **Vector decimals conflict**: If measure row decimals != measure column decimals without a 0D cell override, compilation halts with `SpecificationError` (`DecimalsConflict`).
   - **Vector currency conflict**: If measure row currency != measure column currency without a 0D cell override, compilation halts with `SpecificationError` (`CurrencyConflict`).
   - **Symmetrical 2D border composition**: Rows control horizontal borders (`style="subtotal"` top thin line, `style="total"` top thin line and bottom double-underline); columns control vertical borders. When row and column both specify `style="total"`, they compose into a full 2D accounting total box (`is_bold=True`). Setting `style="data"` on a 0D cell override unconditionally resets both axes to standard data.
   - **Highlight propagation**: Cells inherit highlight if either row or column specifies `highlight=True`, unless explicitly overridden by `highlight=False` on the 0D cell override.
3. **Tier 3 (Descriptor boundaries & unassigned cells)**:
   - **Descriptor axis isolation**: Vector formulas never broadcast across descriptor columns (`role="descriptor"`). Descriptor coordinates isolate formatting and currency conflicts.
   - **Unassigned cell enforcement**: Any measure cell lacking a 0D override, row expression, and column expression raises `SpecificationError` (`UnassignedCell`). Every measure cell must have an explicit value or formula. Any descriptor cell without an explicit value also raises `SpecificationError` (`UnassignedCell`).

---

## 5. Cell-Level Specifications & Overrides

### Cell Signature & Invariants

A `Cell` instance represents a discrete 0D cell override, specifying either a constant scalar value or an explicit cell-level formula expression:

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

- **Mutually exclusive payload**: Must specify either `value` or `expr`, never both (raises `ValueError` in Python and `SpecificationError` in compiler).
- **Allowed values**: `CellValueLiteral = float | int | str | bool | None`.
- **Indentation constraint**: `indent: int` is strictly restricted to descriptor cells (`role="descriptor"`). Setting `indent > 0` on a measure cell raises `SpecificationError` (`InvalidIndentOnMeasureCell`).
- **Currency constraint**: `currency` is permitted only when effective format is `"currency"`. Specifying currency on non-currency presets raises `SpecificationError` (`InvalidCurrencyForPreset`).
- **Precision constraint**: `decimals` is permitted only on numeric presets (`currency`, `number`, `percentage`, `multiple`, `basis_points`, `integer`). Setting decimals on non-numeric presets (`date`, `text`) raises `SpecificationError` (`InvalidPrecisionForPreset`). Decimals cannot exceed 14 (`PrecisionOutOfBounds`).
- **Highlight**: Boolean flag (`highlight=True`) applying prominent visual fill styling to callout drivers or summary outputs.
- **0D note isolation**: Free-form audit comment or footnote attached to a discrete cell coordinate. Notes never broadcast across rows or columns; vector-generated cells have `note=None`.

### Input Types for `cells`

The `cells` mapping on `table.add_row` accepts `CellInputValue`:

$$\text{CellInputValue} = \text{CellValueLiteral} \mid \text{Cell} \mid \text{DetailedCellDict}$$

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

# 4. Overriding a column vector expression on a summary row
table.add_row(
    "total",
    cells={"market_cap": Cell(expr='SUM(Comps["apple".."google", "market_cap"])')},
    style="total",
)
```

### Dynamic Coordinate Updates: `table.set_cell`

Use `set_cell` to update or patch existing cell coordinates without redefining the row:

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
# Add note and highlight to an existing cell
table.set_cell("revenue", "2024E", highlight=True, note="Revised Q3 forecast")
```

---

## 6. Visual Layout Elements

Visual formatting elements establish publication hierarchy and spacing without affecting calculation dependencies.

### Section Headers

Spans across all table columns as a full-width divider without cell merging:

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

Inserts empty rows or columns for visual spacing. Both methods take zero arguments:

```python
table.add_row_gutter() -> Table
table.add_column_gutter() -> Table
```

- **Row gutter**: Consumes 1 physical row sized to `theme.metrics.gutter_row_height` (9.0 pt) with zero calculation cells.
- **Column gutter**: Consumes 1 physical column sized to `theme.metrics.gutter_col_width` with zero calculation cells.
- Vector formulas skip gutter columns and gutter rows.

### Worksheet Margins & Table Spacing

- **Default margins**: On worksheets with default margins, Row 0 is reserved as a top gutter and Col 0 (Column A) is reserved as a left gutter. The primary table starts at physical cell `B2` (`col: 1, row: 1`).
- **Multi-table vertical stacking**: When multiple tables are placed on the same worksheet, they stack vertically in declared order, separated automatically by 2 blank gutter rows.
- **Table banners & buffer gutters**: When `title` or `subtitle` is defined, the table emits title/subtitle rows followed by an 8.0 pt title buffer gutter before column headers.
- **Column width arbitration**: Sheet-wide column widths arbitrate across stacked tables, accounting for descriptor indentation padding (`indent * 2` characters) and enforcing `theme.metrics.min_col_width` (12.0 chars) on measure columns.

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

## 7. Formatting Presets & Semantic Styles

### Format Presets (`FormatPreset`)

Formatting applies through the 3-tier specificity lattice:
$$\text{Cell Override} > (\text{Row Rule } \mathbf{XOR} \text{ Column Rule}) > \text{Table Default} > \text{Model Theme}$$

| Preset | Description | Example output |
| :--- | :--- | :--- |
| `"currency"` | Formatted currency with table currency symbol | `$1,250.00` |
| `"percentage"` | Scaled percentage | `15.0%` |
| `"number"` | Standard delimited numeric value | `1,250.00` |
| `"integer"` | Undelimited integer without thousand separators (e.g. years, indices) | `2024` |
| `"multiple"` | Valuation multiple | `12.5x` |
| `"basis_points"` | Basis points | `150 bps` |
| `"date"` | ISO date string | `2024-12-31` |
| `"text"` | Plain text | `Sample` |

### Semantic Styles (`SemanticStyle`)

Row and column styles control accounting borders and typography:

- `style="data"` (default): Standard data row/column.
- `style="subtotal"`: Top thin line separator for subtotals.
- `style="total"`: Top thin line and bottom double-underline for grand totals (renders bold typography).
- **Orthogonal 2D composition**: Row styles format horizontal borders; column styles format vertical borders. When row and column both specify `"total"`, they form a complete 2D accounting total box.

### Currency Codes (`CurrencyCode`)

Tables, columns, rows, and cells accept ISO 3-letter currency codes:

`USD`, `EUR`, `GBP`, `JPY`, `CHF`, `CAD`, `AUD`, `CNY`, `HKD`, `SGD`, `SEK`, `NOK`, `NZD`, `INR`, `BRL`

- Input is case-insensitive (e.g. `"usd"` or `"USD"`).
- Table-level default falls back to `"USD"` if unspecified.
- Permitted only on cells, rows, or columns where effective format is `"currency"`. Specifying currency on non-currency presets raises `SpecificationError` (`InvalidCurrencyForPreset`).
