# Model Authoring Guide

This guide provides an end-to-end, comprehensive reference for authoring raw Finx financial models. It details how to construct multi-file JSON specifications, structure orthogonal 2D calculation tables, write semantic formulas, and apply institutional formatting.

---

## 1. Core Modeling Paradigms

Traditional spreadsheets conflate calculation logic, time-series vectors, and presentation layout. Finx decouples these concerns into an explicit, deterministic compilation pipeline:

- **Declarative source of truth**: Models are written as structured JSON specifications (`finx.json` + `tables/*.json`). Spreadsheets (`.xlsx`) are unedited, compiled artifacts.
- **Orthogonal 2D matrix**: Tables are general $R \times C$ matrices where rows and columns are symmetrical calculation axes. There are no specialized table types; table behavior is determined by axis roles and vector formula directions.
- **Separation of concerns**:
  - *Identity*: Immutable programmatic identifiers (`TableId`, `RowId`, `ColId`) used in formulas and ASTs.
  - *Presentation*: Display labels (`headers`, descriptor cells), typographic indentation, formatting presets, and structural styles.
  - *Physical grid*: Sheet placement, column offsets, row heights, and freeze panes computed deterministically by the layout engine.
- **3-tier specificity lattice**:
  $$\text{Cell Override (0D Point)} > \text{Vector Rule (1D Row or Column)} > \text{Explicit Null}$$
- **Zero silent magic**: Every coordinate is explicit. Ambiguous formulas, missing cells, or contradictory formats halt compilation with actionable diagnostic errors.

---

## 2. Project Directory Architecture

A Finx model project is a directory separating workbook containers from standalone calculation tables:

```text
my_model/
├── finx.json                  # Root manifest: metadata, theme, and sheet structure
└── tables/                    # Standalone table calculation specifications
    ├── assumptions.json
    ├── revenue_schedule.json
    └── income_statement.json
```

### Invariants
- **One table per file**: Table specifications reside in separate JSON files under `tables/` (or relative subdirectories).
- **No inlining**: Sheets in `finx.json` reference table relative file paths.
- **Global table ID uniqueness**: Every table `id` must be unique across the entire project.
- **Filename-to-ID correspondence**: The filename stem of each table file must match its declared table `id` (case-insensitively, ignoring hyphens and underscores, e.g. `revenue_schedule.json` matches `RevenueSchedule`).
- **Complete and singular sheet assignment**: Every table specification must be assigned to exactly one sheet in `finx.json`. Unassigned tables or duplicating a table across multiple sheets halts compilation.

---

## 3. Root Manifest (`finx.json`)

The manifest defines workbook-level metadata, pluggable theme selection, and sheet tab configurations:

```json
{
  "model_name": "LBO_Simple",
  "theme": "corporate_navy",
  "sheets": [
    {
      "name": "Assumptions",
      "tab_color": "#1B365D",
      "show_gridlines": true,
      "tables": [
        "tables/assumptions.json"
      ]
    },
    {
      "name": "Financials",
      "tab_color": "#2C4D75",
      "freeze_panes": {
        "row": 6,
        "col": 2
      },
      "show_gridlines": true,
      "tables": [
        "tables/revenue_schedule.json",
        "tables/income_statement.json"
      ]
    }
  ]
}
```

### Manifest Properties
- **`model_name`** (`string`, required): Model title used in metadata and binary workbook doc properties.
- **`theme`** (`string`, optional, default `"corporate_navy"`): Built-in theme name (`"corporate_navy"`) or relative path to a custom theme JSON file (e.g. `"./themes/custom.json"`). Only `"corporate_navy"` is statically embedded.
- **`sheets`** (`array`, required): Ordered list of worksheet tab definitions.
  - `name` (`string`, required): Worksheet tab label. Must be unique across the workbook.
  - `tab_color` (`string`, optional): Hex color code (`"#RGB"`, `"#RRGGBB"`, or `"#RRGGBBAA"`). Must start with `#`.
  - `freeze_panes` (`object`, optional): `{ "row": u32, "col": u32 }` setting the 0-indexed physical worksheet row and column split lines. Because tables begin after sheet margins (Row 0, Col 0), title blocks, and column headers, calculate row and column splits using physical worksheet coordinates.
  - `show_gridlines` (`boolean`, optional, default `true`): Toggles gridline rendering.
  - `tables` (`array of strings`, required): Relative file paths to table specifications. When a sheet contains multiple tables, the spatial layout engine stacks them vertically, separated by blank row gutters.

---

## 4. Table Specification Structure (`tables/<id>.json`)

Each table JSON file defines a standalone 2D calculation matrix:

```json
{
  "id": "IncomeStatement",
  "title": "Income Statement",
  "subtitle": "($ in Millions)",
  "currency": "usd",
  "collapsible": false,
  "columns": [ ... ],
  "rows": [ ... ]
}
```

### Top-Level Table Fields
- **`id`** (`string`, required): Immutable programmatic table identifier used in formula coordinates (e.g. `IncomeStatement`).
- **`title`** (`string`, required): Display header rendered across the top of the table.
- **`subtitle`** (`string`, optional): Display sub-banner (e.g. `"($ in Millions)"`, `"Preliminary Draft"`). Must not be empty or whitespace-only if declared (`EmptyTableSubtitle`).
- **`currency`** (`CurrencyCode`, optional): Default ISO currency code for all currency cells in the table (e.g. `"usd"`, `"eur"`, `"gbp"`).
- **`collapsible`** (`boolean`, optional, default `false`): When `true`, the layout engine wraps all table body rows in an Excel outline group, allowing the user to collapse the table into its title banner.
- **`columns`** (`array`, required): Ordered column definitions (data columns and gutters). Must declare at least one column (`EmptyColumns`).
- **`rows`** (`array`, required): Ordered row definitions (data rows, section headers, and gutters). Must declare at least one row (`EmptyRows`).

---

## 5. Columns & Horizontal Layout

Columns define the horizontal axes of the table. Columns can be calculable data columns or blank spacing gutters.

### 5.1. Data Columns (`kind: "data"`)

```json
{
  "kind": "data",
  "id": "2024E",
  "role": "measure",
  "headers": ["Forecast Projections", "2024E"],
  "style": "data",
  "format": "currency",
  "decimals": 1,
  "currency": "usd",
  "expr": "...",
  "note": "Management base case",
  "highlight": false
}
```

- **`id`** (`string`, required): Column identifier (`ColId`), referenced in formula coordinates (e.g. `"2024E"`). Must be unique within the table.
- **`role`** (`"measure"` | `"descriptor"`, optional, default `"measure"`):
  - **`measure`**: Calculable values, metrics, and timeline periods. Row vector formulas (`row.expr`) broadcast across all measure columns.
  - **`descriptor`**: Row labels, line item descriptions, units, or metadata stubs. Descriptors are strictly isolated from vector formula broadcasting and temporal offsets (`col.prev` / `col.next`).
- **`headers`** (`array of strings`, optional): Multi-tier column header labels.
  - Single-tier: `["Value"]`.
  - Multi-tier: `["Forecast Projections", "2024E"]`. Identical adjacent parent headers automatically display across columns in the emitted worksheet using institutional `Center Across Selection` (FAST standard: zero merged cells), ensuring formula ranges and coordinate navigation remain uncompromised.
  - Alignment across depths: When columns within a table declare differing header array lengths, headers are bottom-aligned so the bottom tier always holds the primary metric label while upper tiers remain blank.
- **`format`** (`FormatPreset`, optional): Default formatting preset applied to this column's measure cells (`currency`, `number`, `percentage`, `multiple`, `basis_points`, `date`, `text`).
- **`decimals`** (`integer`, optional): Display precision ($0 \le N \le 14$). Valid only on numeric format presets.
- **`currency`** (`CurrencyCode`, optional): ISO currency code override (`usd`, `eur`, `gbp`, etc.). Valid only when `format` resolves to `currency`.
- **`style`** (`SemanticStyle`, optional): Default structural presentation style (`data`, `subtotal`, `total`).
- **`highlight`** (`boolean`, optional, default `false`): Applies visual theme callout shading.
- **`expr`** (`string`, optional): Column-wide vector formula that broadcasts down across all data rows (used in vertical schedules).
- **`note`** (`string`, optional): Audit or documentation note attached to the column header.
- **`source`** (`string` | `object`, optional): Data provenance metadata.

### 5.2. Column Gutters (`kind: "gutter"`)

```json
{
  "kind": "gutter"
}
```

A blank spacing column used to visually partition calculation blocks (e.g. separating historical periods from forecast periods). Column gutters have no ID, cannot contain cells, and are skipped by formula navigation.

---

## 6. Rows & Vertical Layout

Rows define the vertical calculation items, section banners, and spacing divisions of the table.

### 6.1. Data Rows (`kind: "data"`)

Calculable rows containing values, vector formulas, or individual cell overrides:

```json
{
  "kind": "data",
  "id": "revenue",
  "style": "data",
  "format": "currency",
  "decimals": 1,
  "expr": "IncomeStatement[\"revenue\", col.prev] * (1.0 + Assumptions[\"rev_growth\", \"val\"])",
  "note": "Audited actuals growing at 15%",
  "cells": {
    "line_item": "Total Net Revenue",
    "2023A": 100.0
  }
}
```

- **`id`** (`string`, required): Row identifier (`RowId`), referenced in formula coordinates (e.g. `"revenue"`). Must be unique within the table.
- **`expr`** (`string`, optional): Row-wide vector formula broadcasting across all measure columns.
- **`style`** (`SemanticStyle`, optional, default `"data"`): Structural style (`data`, `subtotal`, `total`).
- **`format`** (`FormatPreset`, optional): Default format preset for measure cells in this row.
- **`decimals`** (`integer`, optional): Display precision ($0 \le N \le 14$).
- **`currency`** (`CurrencyCode`, optional): Currency code override for this row.
- **`highlight`** (`boolean`, optional, default `false`): Highlights the entire row.
- **`note`** (`string`, optional): Audit note attached to the row.
- **`source`** (`string` | `object`, optional): Data provenance metadata.
- **`cells`** (`object`, optional): Map of column IDs (`ColId`) to cell inputs.

### 6.2. Section Header Rows (`kind: "section_header"`)

Visual divider rows used to organize line items into logical financial sections:

```json
{
  "kind": "section_header",
  "title": "Operating Expenses",
  "indent": 0,
  "note": "Excludes depreciation and amortization"
}
```

- **`title`** (`string`, required): Header label displayed across the table width.
- **`indent`** (`integer`, optional, default `0`): Typographic indent level ($0 \le N \le 3$) for hierarchical section nesting.
- **`note`** (`string`, optional): Audit note attached to the section header.

Section headers contain no calculable cells and do not interrupt vector formulas across data rows.

### 6.3. Row Gutters (`kind: "gutter"`)

```json
{
  "kind": "gutter"
}
```

Inserts a blank spacing row between financial sections for clean visual presentation. Row gutters have no ID and contain no cells.

---

## 7. Cell Inputs & the Specificity Lattice

Each key in `data_row.cells` must refer to a declared data column `ColId`.

### 7.1. Input Forms

1. **Literal value**: Number, string, boolean, or explicit `null`:
   ```json
   "cells": {
     "line_item": "Revenue",
     "2023A": 100000.0,
     "active": true,
     "discontinued_col": null
   }
   ```
2. **Formula override**: Evaluated expression replacing vector rules for this coordinate:
   ```json
   "cells": {
     "2024E": { "expr": "IncomeStatement[\"revenue\", col.prev] * 1.10" }
   }
   ```
3. **Detailed cell override**: Explicit formatting, notes, or indentation overrides:
   ```json
   "cells": {
     "line_item": { "value": "Base Salary", "indent": 1 },
     "2023A": {
       "value": 50000.0,
       "style": "total",
       "format": "currency",
       "decimals": 2,
       "highlight": true,
       "note": "Audited by PwC"
     },
     "2024E": {
       "expr": "IncomeStatement[\"revenue\", col.prev] * 1.10",
       "highlight": true,
       "note": "Management upside case"
     }
   }
   ```
   *Payload requirement*: A detailed cell override must specify **exactly one** of `"value"` or `"expr"`. Specifying both halts compilation with `MutuallyExclusiveCellPayload`; specifying neither halts with `EmptyCellPayload`. Both literal values and formula overrides support detailed presentation attributes (`format`, `decimals`, `currency`, `style`, `highlight`, `indent`, `note`, `source`).

4. **Style reset escape hatch**: Setting `"style": "data"` on an individual cell override suppresses inherited row or column summary borders, allowing clean descriptor labels or memo annotations on subtotal and total rows without unwanted underlines.

### 7.2. Specificity Resolution Lattice

When evaluating what value, formula, or format applies to a cell coordinate `(Row, Col)`:

$$\text{Cell Override (0D)} > \text{Vector Rule (1D Row or Col)} > \text{Explicit Null}$$

1. **Point override**: If `cells[col_id]` exists, its value, formula, or `null` is authoritative.
2. **Vector formula**: If no cell override exists, the engine checks `row.expr` and `col.expr`.
3. **Zero ambiguity rule**: If both `row.expr` and `col.expr` exist for a cell coordinate and no point override is provided, compilation halts with `IrError::AmbiguousExpressionConflict`.
4. **Mandatory descriptor assignment**: Vector formulas broadcast strictly across measure columns and never evaluate across descriptor columns. Consequently, **every data row must explicitly define a cell entry for every descriptor column** in `row.cells` (with literal text or `null`), otherwise compilation halts with `IrError::UnassignedCell`.
5. **Zero silent blanks rule**: Every measure cell must resolve to a value, a vector formula, or an explicit `null`. Unassigned measure cells halt compilation with `IrError::UnassignedCell`.
6. **Vector attribute conflicts**: If a measure cell's row and column define conflicting formatting attributes without a point override, compilation halts with `IrError::FormatConflict`, `IrError::DecimalsConflict`, or `IrError::CurrencyConflict`.
7. **Indentation rule**: Setting `indent > 0` on a measure cell causes an `InvalidIndentOnMeasureCell` error. Typographic indentation ($0 \le N \le 3$) is permitted only on descriptor columns or section headers.

---

## 8. Formula Syntax & Addressing

Formulas in Finx are written as plain strings without a leading `=` sign.

### 8.1. Coordinate References

| Syntax | Description | Example |
| :--- | :--- | :--- |
| `Table[row, col]` | Absolute 2D cell coordinate | `IncomeStatement["revenue", "2024E"]` |
| `Table[row, @col]` | Current active column (same table or cross-table) | `Assumptions["inflation_rate", @col]` |
| `Table[@row, col]` | Current active row (same table or cross-table) | `Summary[@row, "val"]` |
| `Table[row, col.prev]` | Previous adjacent measure column | `IncomeStatement["revenue", col.prev]` |
| `Table[row, col.next]` | Next adjacent measure column | `IncomeStatement["revenue", col.next]` |
| `Table[row, col.prev(k)]` | Predecessor column $k$ steps prior | `IncomeStatement["revenue", col.prev(2)]` |
| `Table[row, col.next(k)]` | Successor column $k$ steps forward | `IncomeStatement["revenue", col.next(2)]` |
| `Table[row.prev, col]` | Previous adjacent data row | `Waterfall[row.prev, "val"]` |
| `Table[row.next, col]` | Next adjacent data row | `Waterfall[row.next, "val"]` |
| `Table[row.prev(k), col]` | Predecessor row $k$ steps prior | `DebtSchedule[row.prev(2), "balance"]` |
| `Table[row.next(k), col]` | Successor row $k$ steps forward | `DebtSchedule[row.next(2), "balance"]` |
| `Table[row, col1..col2]` | 1D horizontal range slice | `SUM(IncomeStatement["revenue", "2023A".."2027E"])` |
| `Table[row1..row2, col]` | 1D vertical range slice | `SUM(BalanceSheet["cash".."inventory", @col])` |
| `Table[row1..row2, col1..col2]` | 2D matrix block range slice | `SUMPRODUCT(Ops["units".."price", "2024E".."2028E"])` |

### 8.2. Relative Tokens (`@col`, `@row`, `col.prev`, `row.prev`)

- **`@col`**: Refers to the measure column currently being calculated in a row vector formula:
  ```text
  IncomeStatement["revenue", @col] * Assumptions["cogs_pct", "val"]
  ```
- **`col.prev` and `col.prev(k)`**: Refers to the previous measure column (or predecessor $k$ periods prior). Perfect for roll-forward schedules and period-over-period compounding:
  ```text
  IncomeStatement["revenue", col.prev] * 1.05
  ```
  *(Note: The first measure column in a roll-forward row has no `col.prev`. An explicit cell override must be supplied for the initial column, or reference resolution halts with `TemporalOffsetOutOfBounds`).*
- **`@row`**: Refers to the active data row in a column vector formula.
- **`row.prev` and `row.prev(k)`**: Refers to the previous data row (e.g. cumulative balance waterfalls).
- **Gutter isolation**: Blank spacing gutters (`kind: "gutter"`) and descriptor columns are transparently skipped by `col.prev` and `col.next`. Similarly, `row.prev` and `row.next` step strictly across calculable data rows (`kind: "data"`), transparently skipping section headers (`kind: "section_header"`) and row gutters (`kind: "gutter"`).
- **Boundary enforcement**: Stepping past timeline boundaries (`col.prev` before the first measure column, `col.next` beyond the last measure column, `row.prev` above row 0, `row.next` past the final data row) halts compilation with `TemporalOffsetOutOfBounds`.
- **Retired bare `@`**: Bare `@` is retired. Always specify the active axis explicitly using `@col` or `@row`. Bare `@` halts compilation with `ParseError::RetiredBareAt`.
- **Coordinate quoting requirement**: Table coordinate keys in formulas **must** be double-quoted strings (e.g. `IncomeStatement["revenue", "2024E"]`). Bare unquoted identifiers (e.g. `IncomeStatement[revenue, 2024E]`) or numbers halt compilation with `ParseError::UnquotedCoordinateKey`. Single quotes (`'`) are not recognized string delimiters and halt tokenization with `ParseError::UnexpectedToken`.

### 8.3. Supported Excel Functions

Finx includes a built-in topological formula evaluator with exact Excel parity across standard financial and numerical operations:

- **Math & aggregation**: `SUM`, `AVERAGE`, `COUNT`, `COUNTA`, `MIN`, `MAX`, `MEDIAN`, `ABS`, `ROUND`, `ROUNDUP`, `ROUNDDOWN`, `SUMPRODUCT`.
- **Logical**: `IF`, `IFS`, `IFERROR`, `IFNA`, `AND`, `OR`, `NOT`, `CHOOSE`, `SWITCH`.
- **Financial**: `NPV`, `XNPV`, `IRR`, `XIRR`, `PMT`, `IPMT`, `PPMT`.
- **Lookup & reference**: `XLOOKUP`, `XMATCH`, `INDEX`.
- **Date & calendar**: `DATE`, `EDATE`, `EOMONTH`, `YEARFRAC`, `YEAR`, `MONTH`, `DAY`, `DAYS`.
- **Conditional aggregations**: `SUMIFS`, `COUNTIFS`, `AVERAGEIFS`, `MINIFS`, `MAXIFS`. Finx standardizes exclusively on modern plural `*IFS` signatures with paired trailing criteria arguments, eliminating the legacy singular `SUMIF`/`COUNTIF` signatures.

#### 8.3.1. Conditional Criteria Syntax (`*IFS`)

> [!NOTE]
> String-encoded criteria (e.g. `">10"`, `"<="`, `"*US*"`) are **strictly exclusive to the `*IFS` conditional aggregation family** (`SUMIFS`, `COUNTIFS`, `AVERAGEIFS`, `MINIFS`, `MAXIFS`) to maintain 100% Excel parity. Standard formula expressions and logical functions (like `IF` or `AND`) do not use string criteria; they use standard binary comparison operators directly (e.g. `IF(IncomeStatement["revenue", @col] > 100.0, 1, 0)`).

Criteria parameters in `*IFS` functions support standard Excel criteria syntax:
- **Exact scalar matches**: `"US"`, `42`, `TRUE` (case-insensitive for text).
- **Relational comparisons**: `">10"`, `"<=5"`, `"<>"`, `"="`. Criteria can be literal strings or cell coordinates resolving to criteria strings (e.g. `Assumptions["growth_floor", "val"]`). (Note: string concatenation via `&` is not supported in Finx formula expressions).
- **Wildcards**: `?` (matches any single character), `*` (matches zero or more characters), and `~` (escape prefix for literal `~?`, `~*`, `~~`).

### 8.4. Operators & Precedence

Formulas are parsed using standard Pratt operator precedence:

| Precedence | Operator | Description | Associativity | Example |
| :---: | :--- | :--- | :---: | :--- |
| **5** | `^` | Exponentiation | Right | `(1.0 + Assumptions["discount_rate", "val"]) ^ 5` |
| **4** | `+`, `-` | Unary positive and negation | Right | `-DebtSchedule["principal_repayment", @col]` |
| **3** | `*`, `/` | Multiplication and division | Left | `Units[@row, @col] * Price[@row, "val"]` |
| **2** | `+`, `-` | Addition and subtraction | Left | `Revenue[@row, @col] - COGS[@row, @col]` |
| **1** | `=`, `<>`, `<`, `<=`, `>`, `>=` | Relational comparisons | Left | `IF(EBITDA[@row, @col] >= 0, 1, 0)` |

Parentheses `( ... )` override default operator precedence. Note that `<>` denotes inequality (not equal).

### 8.5. Formula Syntax Rules & Conventions

- **Mandatory table identifier**: Every coordinate reference must include its target table identifier (e.g. `IncomeStatement["revenue", @col]`), even for references targeting the same table. Bare coordinates without a table name (e.g. `["revenue", @col]`) halt compilation with `ParseError::UnexpectedToken`.
- **Uppercase function names**: Function names must be strictly uppercase (`SUM`, `IF`, `XLOOKUP`). Lowercase names (e.g. `sum(...)`) halt compilation with `ParseError::InvalidFunctionName`.
- **Uppercase boolean literals**: Formulas require uppercase `TRUE` and `FALSE`. Lowercase `true` and `false` are rejected as bare identifiers in formulas (contrast with JSON literal cells where standard lowercase JSON `true`/`false` are valid).
- **Omitted argument placeholder (`_`)**: When omitting an optional positional argument in function calls (e.g. `PMT` or `XLOOKUP`), use an explicit underscore `_` (e.g. `XLOOKUP(val, lookup_range, return_range, _, 0, 1)` or `PMT(r, n, pv, _, 1)`). Empty commas (`, ,`) halt compilation with `ParseError::UnexpectedToken`.
- **No `null` in formulas**: While `null` is a valid literal in JSON data row `cells` (e.g. `"cells": { "discontinued": null }`), formula expressions strictly reject `null`.
- **Homologous cross-table `@col` / `@row` matching**: Using `Table[row, @col]` or `Table[@row, col]` against another table requires that the target table declares a matching column or row identifier. Referencing a parameter table with a single value column must use explicit coordinates (e.g. `Assumptions["growth", "val"]`), not `@col` (which halts with `ResolveError::ActiveColumnMismatch` or `ResolveError::ActiveRowMismatch`).
- **Numeric literals in formulas**: Numbers in formulas support integer and standard decimal notation (`100`, `0.15`, `-5.2`). Scientific or exponential notation (e.g. `1e6`) is not supported by the formula tokenizer.
- **Double-dot range slice (`..`)**: Range slices exclusively use `..` (`Table["r", "c1".."c2"]`). Excel colons (`:`) halt compilation with `ParseError::UnexpectedToken`.
- **Forward range slice order**: Range slices (`col1..col2` and `row1..row2`) must follow declaration order. Inverted ranges (e.g. `"2027E".."2023A"`) halt compilation with `ResolveError::InvertedRange`.

---

## 9. Semantic Formatting, Styles & Themes

Model authors specify semantic intent rather than raw Excel format masks.

### 9.1. Format Presets (`format`)

| Preset | Purpose | Emitted Format Sample |
| :--- | :--- | :--- |
| `"currency"` | Monies with currency symbol and thousands separators | `$1,250,000` |
| `"number"` | Standard numerical quantities with thousands separators | `1,250,000` |
| `"percentage"` | Fractional numbers scaled by 100 with `%` | `15.0%` |
| `"multiple"` | Valuation or operational ratios with trailing `x` | `10.5x` |
| `"basis_points"` | Yield or spread rates scaled by 10,000 with `bps` | `250 bps` |
| `"date"` | Calendar dates | `YYYY-MM-DD` |
| `"text"` | Strings and non-numeric labels | Left-aligned text |

### 9.2. Structural Styles (`style`)

- **`"data"`** (default): Normal weight text, transparent fill, standard workbook gridlines preserved.
- **`"subtotal"`**: Single top border line, subtle bolding (used for Gross Profit, EBITDA, Total Opex).
- **`"total"`**: Single top border line, double bottom accounting underline, bold text (used for Net Income, Total Assets).

### 9.3. Display Precision (`decimals`) & Currency Codes (`currency`)

- **`decimals`** ($0 \le N \le 14$): Specifies display decimal precision. Supported strictly on numeric presets (`currency`, `number`, `percentage`, `multiple`, `basis_points`). Declaring `decimals` on non-numeric presets (`date`, `text`) halts compilation with `InvalidPrecisionForPreset`. Precision exceeding 14 halts with `PrecisionOutOfBounds`.
- **`currency`**: Canonical 3-letter lowercase ISO codes:
  `usd` ($), `eur` (€), `gbp` (£), `jpy` (¥), `chf` (CHF), `cad` (C$), `aud` (A$), `cny` (¥), `hkd` (HK$), `sgd` (S$), `sek` (kr), `nok` (kr), `nzd` (NZ$), `inr` (₹), `brl` (R$).
  Valid strictly when the effective format resolves to `currency`. Declaring `currency` on non-currency presets halts compilation with `InvalidCurrencyForPreset`.

### 9.4. Specificity Lattice & Symmetrical 2D Resolution

Finx enforces a deterministic 3-tier specificity lattice without silent magic or arbitrary axis favoritism:

$$\text{Cell Override (0D)} \succ \text{Vector Rule (1D Row or Column)} \succ \text{Table Default} \succ \text{Theme Default}$$

1. **Number format, precision & currency resolution**:
   - **Tier 1 (point override)**: Explicit attributes declared on `DetailedCellInput` always take precedence for that coordinate.
   - **Tier 2 (symmetrical vector rules)**: If only the row or only the column specifies an attribute (or both declare identical attributes), that attribute applies across the measure cells. If a measure row and measure column specify differing format presets, precisions, or currencies without a point override, compilation halts with an explicit conflict error (`FormatConflict`, `DecimalsConflict`, `CurrencyConflict`).
   - **Tier 3 (table default)**: Unspecified currency falls back to `table.currency` (defaults to `"usd"`).
   - **Tier 4 (theme default)**: Unspecified decimals fall back to the active theme's configured default for that preset (`currency` $\rightarrow$ 0, `number` $\rightarrow$ 0, `percentage` $\rightarrow$ 1, `multiple` $\rightarrow$ 1, `basis_points` $\rightarrow$ 0).
2. **Orthogonal structural style composition**:
   - Rows govern the horizontal axis (top and bottom borders: `subtotal_border_top`, `total_border_bottom`).
   - Columns govern the vertical axis (left and right borders: `col_subtotal_border_left`, `col_total_border_right`).
   - At matrix intersections, horizontal and vertical rules compose symmetrically into framed accounting totals (e.g. bold text with thin top border and double bottom border).
   - **Style reset**: Setting `style: "data"` on an individual cell override acts as an explicit point escape hatch, suppressing inherited horizontal and vertical summary borders.
3. **Automatic font color-coding**: Literal input values are automatically emitted in blue font (`palette.input_text`), while formula-driven calculation cells are emitted in black font (`palette.formula_text`), adhering to institutional Wall Street modeling standards without manual intervention.

---

## 10. Step-by-Step Walkthrough: Building a Model

Here is how to author a complete, realistic financial model from scratch.

### Step 1: Create the Directory Layout

```bash
mkdir -p my_model/tables
```

### Step 2: Write `my_model/finx.json`

```json
{
  "model_name": "Acme_Financial_Plan",
  "theme": "corporate_navy",
  "sheets": [
    {
      "name": "Summary",
      "tab_color": "#1B365D",
      "tables": [
        "tables/assumptions.json"
      ]
    },
    {
      "name": "Operating Model",
      "tab_color": "#2C4D75",
      "freeze_panes": { "row": 6, "col": 2 },
      "tables": [
        "tables/income_statement.json"
      ]
    }
  ]
}
```

### Step 3: Write `my_model/tables/assumptions.json`

A two-column parameter lookup table:

```json
{
  "id": "Assumptions",
  "title": "Model Drivers & Assumptions",
  "columns": [
    { "kind": "data", "id": "param", "role": "descriptor", "headers": ["Parameter"] },
    { "kind": "data", "id": "val", "role": "measure", "headers": ["Value"] }
  ],
  "rows": [
    {
      "kind": "data",
      "id": "rev_growth",
      "format": "percentage",
      "decimals": 1,
      "cells": {
        "param": "Annual Revenue Growth",
        "val": 0.12
      }
    },
    {
      "kind": "data",
      "id": "cogs_rate",
      "format": "percentage",
      "decimals": 1,
      "cells": {
        "param": "COGS Margin (% Revenue)",
        "val": 0.40
      }
    },
    {
      "kind": "data",
      "id": "tax_rate",
      "format": "percentage",
      "decimals": 1,
      "cells": {
        "param": "Effective Tax Rate",
        "val": 0.25
      }
    }
  ]
}
```

### Step 4: Write `my_model/tables/income_statement.json`

A multi-period forecast table showing vector rules, initial period overrides, subtotal styling, and cross-table references:

```json
{
  "id": "IncomeStatement",
  "title": "Income Statement Forecast",
  "subtitle": "($ in Millions)",
  "currency": "usd",
  "columns": [
    { "kind": "data", "id": "line_item", "role": "descriptor", "headers": ["Line Item"] },
    { "kind": "data", "id": "2023A", "headers": ["Historical", "2023A"] },
    { "kind": "data", "id": "2024E", "headers": ["Projections", "2024E"] },
    { "kind": "data", "id": "2025E", "headers": ["Projections", "2025E"] },
    { "kind": "data", "id": "2026E", "headers": ["Projections", "2026E"] }
  ],
  "rows": [
    {
      "kind": "data",
      "id": "revenue",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"revenue\", col.prev] * (1.0 + Assumptions[\"rev_growth\", \"val\"])",
      "cells": {
        "line_item": "Total Revenue",
        "2023A": 100.0
      }
    },
    {
      "kind": "data",
      "id": "cogs",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"revenue\", @col] * Assumptions[\"cogs_rate\", \"val\"]",
      "cells": {
        "line_item": "Cost of Goods Sold",
        "2023A": 40.0
      }
    },
    {
      "kind": "data",
      "id": "gross_profit",
      "style": "subtotal",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
      "cells": {
        "line_item": "Gross Profit"
      }
    },
    {
      "kind": "gutter"
    },
    {
      "kind": "section_header",
      "title": "Operating Expenses"
    },
    {
      "kind": "data",
      "id": "sga",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"revenue\", @col] * 0.15",
      "cells": {
        "line_item": { "value": "Selling, General & Administrative", "indent": 1 },
        "2023A": 15.0
      }
    },
    {
      "kind": "data",
      "id": "ebit",
      "style": "subtotal",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"gross_profit\", @col] - IncomeStatement[\"sga\", @col]",
      "cells": {
        "line_item": "Operating Income (EBIT)"
      }
    },
    {
      "kind": "data",
      "id": "tax",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"ebit\", @col] * Assumptions[\"tax_rate\", \"val\"]",
      "cells": {
        "line_item": "Income Tax Expense"
      }
    },
    {
      "kind": "data",
      "id": "net_income",
      "style": "total",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"ebit\", @col] - IncomeStatement[\"tax\", @col]",
      "cells": {
        "line_item": "Net Income"
      }
    }
  ]
}
```

---

## 11. Authoring Invariants & Validation Checklist

Before compiling, verify that your model obeys these strict compiler rules:

- [ ] **Unique identifiers**: Sheet names in `finx.json` and table IDs across the project are globally unique. Column IDs and row IDs are unique within their respective table.
- [ ] **Non-empty tables**: Every table must declare at least one column (`EmptyColumns`) and at least one row (`EmptyRows`).
- [ ] **Table file stem and sheet assignment**: The filename stem of each table file must match its declared table `id` (case-insensitively, ignoring hyphens and underscores), and every table specification must be assigned to exactly one sheet in `finx.json`.
- [ ] **Mandatory descriptor assignment**: Every data row must explicitly assign a cell value in `cells` for every descriptor column declared in the table. Vector formulas do not broadcast to descriptor columns.
- [ ] **No silent unassigned cells**: Every measure column coordinate in a data row has a literal value, formula override, vector formula, or explicit `null`. Unassigned cells halt compilation with `IrError::UnassignedCell`.
- [ ] **Initial roll-forward override**: Any row utilizing `col.prev` must provide an explicit literal or formula override in the first measure column (since `col.prev` does not exist for the initial measure column).
- [ ] **Temporal offset bounds**: Ensure `col.prev(k)`, `col.next(k)`, `row.prev(k)`, and `row.next(k)` do not step past timeline or row boundaries. Stepping out-of-bounds raises `TemporalOffsetOutOfBounds`.
- [ ] **No ambiguous vector expressions**: Do not assign both `row.expr` and `col.expr` targeting the same cell coordinate unless an explicit cell override is defined at that point.
- [ ] **Vector formatting harmony**: Do not assign conflicting `format`, `decimals`, or `currency` on opposing row and column vectors without an explicit cell override (`FormatConflict`, `DecimalsConflict`, `CurrencyConflict`).
- [ ] **Mandatory table identifier**: Table identifiers must precede all formula coordinate brackets (e.g. `IncomeStatement["revenue", @col]`). Bare coordinates like `["revenue", @col]` without a table name trigger parse errors.
- [ ] **Coordinate quoting requirement**: Table coordinates in formulas must double-quote row and column identifiers (`Table["row", "col"]`). Bare unquoted keys trigger `ParseError::UnquotedCoordinateKey`; single quotes trigger `ParseError::UnexpectedToken`.
- [ ] **Retired bare `@`**: Active coordinates must be explicitly written as `@col` or `@row`. Bare `@` triggers `ParseError::RetiredBareAt`.
- [ ] **Active coordinate matching**: Ensure target tables in `Table[row, @col]` and `Table[@row, col]` declare matching column or row identifiers (`ActiveColumnMismatch`, `ActiveRowMismatch`).
- [ ] **Double-dot range delimiter**: Range slices in formulas must use `..` (`Table["r", "c1".."c2"]`). Never use colons `:`.
- [ ] **Forward range order**: Coordinate ranges must declare endpoints in forward declaration order (`"c1".."c2"` and `"r1".."r2"`). Inverted ranges trigger `InvertedRange`.
- [ ] **Uppercase function and boolean names**: All built-in Excel functions must be uppercase (`SUM`, `IF`, `XLOOKUP`), and boolean literals in formulas must be uppercase (`TRUE`, `FALSE`).
- [ ] **Omitted argument placeholder**: Use explicit underscore `_` to omit optional arguments in function calls (`XLOOKUP(val, rng1, rng2, _, 0, 1)`). Do not leave empty commas.
- [ ] **Indentation on descriptors only**: Only descriptor cells and section headers can have `indent > 0` ($0 \le N \le 3$). Setting `indent` on measure cells raises `InvalidIndentOnMeasureCell`.
- [ ] **Cell payload exclusivity**: A detailed cell override cannot contain both `"value"` and `"expr"` (`MutuallyExclusiveCellPayload`), and cannot omit both (`EmptyCellPayload`). It must contain exactly one or the other.
- [ ] **Format-currency compatibility**: Do not declare `"currency"` unless `format` is `"currency"` or omitted (which defaults to currency).
- [ ] **Format-precision compatibility**: Do not declare `"decimals"` on non-numeric formats (`"date"`, `"text"`). Precision cannot exceed 14 decimal places.
- [ ] **Non-empty table subtitles**: Omit `"subtitle"` if not needed. Declaring an empty string or whitespace-only subtitle triggers `EmptyTableSubtitle`.
- [ ] **Valid hex tab colors**: In `finx.json`, `tab_color` must be a valid hex color code starting with `#` (`#RGB`, `#RRGGBB`, `#RRGGBBAA`).
- [ ] **Acyclic formula graph**: Ensure formula references do not contain circular dependencies (e.g. Row A depends on Row B, which depends on Row A). The compiler automatically performs Tarjan's SCC analysis and halts if any cycle is detected.

---

## 12. Related Documentation

- [Specification Schema Reference](spec-format.md): Comprehensive JSON schema definitions and AST types.
- [Grid Architecture](grid.md): Multi-tier headers, section headers, gutters, and table collapsing.
- [Semantic Formatting & Themes](formatting.md): Deep-dive into format masks, theme color palettes, and borders.
- [Addressing Syntax](addressing.md): Complete formula coordinate syntax, slicing, and dynamic tokens.
- [Formula Grammar](grammar.md): Formal EBNF grammar and operator precedence.
- [Modeling Patterns & Archetypes](patterns.md): Canonical models (Cap Tables, Debt Corkscrews, Comps, Cross-Footing).
- [End-to-End Reference Model](project-example.md): Additional multi-file working reference model.
