# Semantic Formatting & Themes

This document specifies Finx's semantic formatting system, preset catalog, structural axis styles, property resolution lattice, and pluggable workbook themes.

## 1. Core Philosophy

Finx enforces complete decoupling between calculation logic and visual styling:
- **Semantic intent over raw masks**: Model authors specify *what* data represents (`currency`, `number`, `percentage`, `multiple`), never raw Excel format strings (e.g. `_($* #,##0_);_($* (#,##0);_($* "-"_);_(@_)`).
- **Orthogonal visual axes**: Presentation decomposes cleanly into four independent, composable axes:
  1. **Structural style (`style`)**: Borders, summary rules, and bold typography (`data`, `subtotal`, `total`).
  2. **Visual emphasis (`highlight`)**: Background fill tint and text accenting (`true` / `false`).
  3. **Semantic format family (`format`)**: Unit/data kind (`currency`, `number`, `percentage`, `multiple`, `basis_points`, `date`, `text`).
  4. **Display precision (`decimals`)**: Explicit count of fractional decimal places (e.g. `0`, `1`, `2`, `4`), independent of format kind.
- **Symmetrical 2D matrix engine**: Formatting and styling rules apply symmetrically across columns, rows, and individual cell overrides.
- **Deterministic OpenXML emission**: Themes translate semantic presets and dynamic precision into institutional-grade formatting rules and accounting styles upon `.xlsx` emission.
- **Zero layout drift**: Format rules never alter underlying floating-point precision or calculation semantics.

---

## 2. Standard Format Preset Catalog (`FormatPreset`)

The `format` property on columns, rows, or cell overrides accepts strictly enumerated semantic presets:

| Format Preset | Semantic Purpose | Default Decimals | Typical Rendered Appearance | Institutional Accounting Standard |
| :--- | :--- | :---: | :--- | :--- |
| `currency` | Monetary amounts | `0` | `$1,250,000` / `(125,000)` / `-` | Accounting currency with parentheses for negatives and dash for zero |
| `number` | Discrete counts, shares, standard numbers | `0` | `1,000` / `(500)` / `-` | Comma-separated accounting number with negative parentheses |
| `percentage` | Rates, margins, and return metrics | `1` | `12.5%` | Standard percentage formatting |
| `multiple` | Valuation and leverage multiples | `1` | `12.4x` | Ratio with trailing lowercase `x` |
| `basis_points` | Spread and yield differences | `0` | `125 bps` | Integer with trailing `bps` suffix |
| `date` | Calendar dates and period markers | N/A | `2024-12-31` or `Dec-24` | Institutional ISO date format |
| `text` | Unformatted textual strings | N/A | `Line Item Name` | Left-aligned string formatting |

---

## 3. Structural Semantic Styles (`SemanticStyle`)

Structural presentation (bold typography, accounting underline rules) is declared via `style` on columns, rows, or cell overrides:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SemanticStyle {
    #[default]
    Data,      // Standard data cells (normal font, default borders)
    Subtotal,  // Intermediate summary (single thin border)
    Total,     // Final bottom-line summary (bold + double underline)
}
```

### 3.1. Symmetrical Rendering

| Style | Row Application | Column Application | Cell Override Application |
| :--- | :--- | :--- | :--- |
| `data` | Standard row typography and borders | Standard column typography and borders | Resets/suppresses summary borders (e.g. memo or descriptor cells on total rows) |
| `subtotal` | Top thin border | Left thin border (separating timeline or segments) | Top thin border |
| `total` | Bold text + top thin border + bottom double underline | Bold text + left thin border + right double border | Bold text + top thin border + bottom double underline |

### 3.2. Visual Emphasis (`highlight`)

Visual emphasis (soft accent background fill tint and text color) is completely orthogonal to structural borders and declared via `highlight: true`:
- **Row**: `"highlight": true` tints the entire row across measure columns.
- **Column**: `"highlight": true` tints the entire column down measure rows.
- **Cell**: `"highlight": true` tints the individual cell coordinate.

---

## 4. Display Precision (`decimals`)

Numeric precision is an orthogonal property decoupled from the format preset:
- **Separation of unit and precision**: `currency` represents monetary formatting, while `decimals` controls whether it renders as whole dollars (`$1,250,000` with `decimals: 0`), EPS/cents (`$1.25` with `decimals: 2`), or cap table share prices (`$0.0125` with `decimals: 4`).
- **Unified numeric formatting**: Replaces rigid legacy presets (`integer`, `decimal`, `currency_decimal`) with pure `format: "number"` or `format: "currency"` paired with optional `decimals`.
- **Theme defaults**: When `decimals` is omitted, the compiler defaults to the active theme's standard precision for that preset (`currency` -> 0, `number` -> 0, `percentage` -> 1, `multiple` -> 1, `basis_points` -> 0).

### 4.1. Symmetrical Usage in Specifications
The `style`, `highlight`, `format`, and `decimals` properties are symmetrically available on [`ColumnSpec`](spec-format.md#3-table-specification-schema), [`RowSpec`](spec-format.md#3-table-specification-schema), and point-level [`DetailedCellInput`](spec-format.md#33-cell-input-syntax-cells):

```json
{
  "columns": [
    { "id": "q1" },
    { "id": "q2" },
    { "id": "q3" },
    { "id": "q4" },
    { "id": "fy", "style": "total" }
  ],
  "rows": [
    { "id": "revenue", "format": "currency" },
    { "id": "cogs", "format": "currency" },
    { "id": "gross_profit", "style": "subtotal", "format": "currency" },
    { "id": "gross_margin", "format": "percentage", "decimals": 1 },
    { "id": "diluted_eps", "format": "currency", "decimals": 2 },
    { "id": "shares_out", "format": "number", "decimals": 0 },
    { "id": "net_income", "style": "total", "format": "currency" },
    { "id": "irr", "highlight": true, "format": "percentage", "decimals": 2 },
    { "id": "ev_ebitda", "format": "multiple", "decimals": 1 }
  ]
}
```

---

## 5. Property Resolution Lattice & Precedence

Every coordinate $(r, c)$ resolves its effective format preset, precision, structural style, and highlight state through the deterministic 3-tier specificity lattice:

$$\text{Point Override (Cell)} \succ \text{Vector Rule (Row or Column)} \succ \text{Default Fallback}$$

### 5.1. Number format resolution
1. **Cell format override (Tier 1)**: An explicit `format` declared on `CellInput` always wins.
2. **Descriptor boundary**: Descriptor coordinates (`role: "descriptor"`) isolate text formatting. Measure formats (`currency`, `percentage`, etc.) never broadcast to descriptor columns or rows.
3. **Measure vector conflict**: If row $r$ and column $c$ are both measures and specify differing format presets (e.g. `row.format: "currency"` and `col.format: "percentage"`), compilation halts with `FormatConflictError` unless resolved by an explicit cell format override.
4. **Single vector format**: If only the row or only the column specifies a format preset, that preset applies across the measure cells.
5. **Default fallback**: Unspecified numbers default to general numeric presentation; unspecified text defaults to standard string alignment.

### 5.2. Precision (`decimals`) resolution
1. **Cell decimals override (Tier 1)**: An explicit `decimals` declared on `CellInput` always wins.
2. **Measure vector conflict**: If row $r$ and column $c$ are both measures and specify differing `decimals` (e.g. `row.decimals: 2` and `col.decimals: 0`), compilation halts with `DecimalsConflictError` unless resolved by an explicit cell override.
3. **Single vector decimals**: If only the row or only the column specifies `decimals`, that precision applies.
4. **Theme default fallback**: Unspecified decimals fall back to the active theme's configured default for the resolved `FormatPreset` (e.g. 0 for `currency` and `number`, 1 for `percentage` and `multiple`, 0 for `basis_points`).

### 5.3. Style resolution & composition
1. **Structural style precedence**: `cell.style.or(row.style).or(col.style).unwrap_or(SemanticStyle::Data)`.
   - An explicit cell override with `style: "data"` cleanly suppresses inherited row or column summary borders.
2. **Symmetrical border composition**:
   - When a row is `total` and a column is `total` (e.g. *Net Income* row $\times$ *FY Total* column), both rules apply:
     - Horizontal: top thin border + bottom double underline.
     - Vertical: left thin border + right double border.
     - Font: bold.
     The cell naturally forms a completely framed accounting total block without conflict.
3. **Fill and border orthogonality**:
   - `highlight` controls background fill tint and accent text color.
   - `subtotal` and `total` control borders and font weight.
   - Because `style` and `highlight` are distinct orthogonal properties, they compose cleanly: a cell can have `style: "total"` and `highlight: true` (e.g. a total row where a punchline cell is highlighted), rendering both the accent background fill and the accounting double underline without schema collision.
4. **Highlight resolution**: `cell.highlight.unwrap_or(row.highlight || col.highlight)`.
   - Point override `highlight: false` explicitly un-highlights a cell even if its row or column is highlighted.

---

## 6. Pluggable Themes (IDE-Style Config)

Finx supports pluggable, external workbook themes inspired by modern IDE and editor color themes (e.g. VS Code, TextMate). The visual identity of an emitted workbook is completely isolated into standalone theme configurations.

### 6.1. Manifest theme declaration

The root workbook manifest (`finx.json`) declares the active `theme` either by referencing a built-in preset or a relative file path to a custom theme JSON:

```json
{
  "model_name": "Project Titan LBO",
  "theme": "corporate_navy",
  "sheets": []
}
```

Or pointing to an external custom theme file:

```json
{
  "model_name": "Project Titan LBO",
  "theme": "./themes/firm_brand.json",
  "sheets": []
}
```

### 6.2. Built-in theme catalog

Finx packages institutional built-in themes out of the box:
- **`corporate_navy`**: Institutional private equity and investment banking style with deep navy headers, subtle blue section fills, and crisp thin borders.
- **`classic_green`**: Traditional financial modeling style with hunter green headers and muted mint accents.
- **`modern_slate`**: Tech and venture capital style with slate/charcoal headers and cool gray section dividers.
- **`minimal_monochrome`**: High-contrast black and white styling with zero colored fills, optimized for black-and-white printing and audit packages.

### 6.3. Theme JSON configuration schema

Custom themes conform to a structured schema defining typography, palette, borders, row height metrics, and number format masks:

```json
{
  "$schema": "https://finx.dev/schemas/theme.json",
  "name": "corporate_navy",
  "typography": {
    "font_family": "Aptos",
    "title_size": 14.0,
    "super_header_size": 10.0,
    "col_header_size": 10.0,
    "section_header_size": 10.0,
    "data_size": 10.0
  },
  "metrics": {
    "title_row_height": 24.0,
    "header_row_height": 18.0,
    "section_row_height": 18.0,
    "data_row_height": 15.0,
    "char_width_multiplier": 1.15,
    "min_col_width": 12.0
  },
  "palette": {
    "table_title_fill": "#1B365D",
    "table_title_text": "#FFFFFF",
    "super_header_fill": "#2C4D75",
    "super_header_text": "#FFFFFF",
    "col_header_fill": "#F0F4F8",
    "col_header_text": "#1B365D",
    "section_header_fill": "#F7F9FC",
    "section_header_text": "#1B365D",
    "zebra_fill": "#FAFBFC",
    "highlight_fill": "#FEF3C7",
    "highlight_text": "#92400E",
    "gridline_color": "#E5E7EB",
    "border_color": "#D1D5DB"
  },
  "borders": {
    "header_border_bottom": "thin",
    "subtotal_border_top": "thin",
    "total_border_top": "thin",
    "total_border_bottom": "double",
    "col_subtotal_border_left": "thin",
    "col_total_border_left": "thin",
    "col_total_border_right": "double"
  },
  "number_formats": {
    "currency": "_($* #,##0{dec}_);_($* (#,##0{dec});_($* \"-\"_);_(@_)",
    "number": "#,##0{dec};(#,##0{dec});\"-\"",
    "percentage": "0{dec}%",
    "multiple": "0{dec}\"x\"",
    "basis_points": "#,##0{dec}\" bps\"",
    "date": "yyyy-mm-dd",
    "text": "@",
    "defaults": {
      "currency_decimals": 0,
      "number_decimals": 0,
      "percentage_decimals": 1,
      "multiple_decimals": 1,
      "basis_points_decimals": 0
    }
  }
}
```

### 6.4. Dynamic decimal mask generation (`{dec}`)

Themes declare format pattern templates utilizing the `{dec}` placeholder. During OpenXML compilation, the emitter dynamically synthesizes the concrete format string:
- **`decimals == 0`**: `{dec}` expands to empty string `""` (e.g. `_($* #,##0_);_($* (#,##0);_($* "-"_);_(@_)`).
- **`decimals > 0`**: `{dec}` expands to `.` followed by $N$ zeros (e.g. for `decimals: 2`, `{dec}` $\rightarrow$ `.00`, yielding `_($* #,##0.00_);_($* (#,##0.00);_($* "-"_);_(@_)`).

This completely eliminates combinatorial mask explosion while giving model authors arbitrary precision control across any numerical preset.

### 6.5. Two-phase theme utilization in the compiler

The resolved theme is utilized across two compiler passes:
1. **Pass 5 (Spatial Layout Planning)**: Consumes theme `typography` and `metrics` to compute physical row heights and font-scaled sheet-wide column widths. Assigns semantic `StyleRole` tags to cells.
2. **Pass 6b (Excel Binary Emission)**: Consumes theme `palette`, `borders`, and `number_formats` with dynamic `{dec}` expansion to instantiate concrete `rust_xlsxwriter::Format` styles and write cells.

This guarantees that changing themes re-skins the emitted workbook completely and resizes rows and columns without touching any table logic, formulas, or ASTs.

---

See [Specification Format](spec-format.md) for schema usage and [Spatial Layout Planning](../compiler/spatial-layout.md) for coordinate and metric assignment.
