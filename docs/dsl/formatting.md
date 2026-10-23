# Semantic Formatting & Themes

This document specifies f8's semantic formatting system, preset catalog, structural axis styles, property resolution lattice, and pluggable workbook themes.

## 1. Core Philosophy

f8 enforces complete decoupling between calculation logic and visual styling:
- Semantic intent over raw masks: Model authors specify *what* data represents (`currency`, `number`, `percentage`, `multiple`), never raw Excel format strings (e.g. `$#,##0_);($#,##0);"-"_)`).
- Orthogonal visual axes: Presentation decomposes cleanly into five independent, composable axes:
  1. Structural style (`style`): Borders, summary rules, and bold typography (`data`, `subtotal`, `total`).
  2. Visual emphasis (`highlight`): Callout background fill tint and text accenting (`true` / `false`).
  3. Background zone shade (`shade`): Theme-backed subtle background fill presets (`none`, `muted`, `accent`, `alt`) for historical/stub/forecast columns or row bands without altering font color.
  4. Semantic format family (`format`): Unit/data kind (`currency`, `number`, `integer`, `percentage`, `multiple`, `basis_points`, `date`, `text`).
  5. Display precision (`decimals`): Explicit count of fractional decimal places (e.g. `0`, `1`, `2`, `4`), independent of format kind.
- Symmetrical 2D matrix engine: Formatting and styling rules apply symmetrically across columns, rows, and individual cell overrides.
- Deterministic OpenXML emission: Themes translate semantic presets and dynamic precision into institutional-grade formatting rules and accounting styles upon `.xlsx` emission.
- Zero layout drift: Format rules never alter underlying floating-point precision or calculation semantics.

---

## 2. Standard Format Preset Catalog (`FormatPreset`)

The `format` property on columns, rows, or cell overrides accepts strictly enumerated semantic presets:

| Format Preset | Semantic Purpose | Default Decimals | Typical Rendered Appearance | Institutional Accounting Standard |
| :--- | :--- | :---: | :--- | :--- |
| `currency` | Monetary amounts | `0` | `$1,250,000` / `(125,000)` / `-` | Accounting currency with parentheses for negatives and dash for zero |
| `number` | Discrete counts, shares, standard numbers | `0` | `1,000` / `(500)` / `-` | Comma-separated accounting number with negative parentheses |
| `integer` | Undelimited whole integers (years, IDs) | N/A | `2024` / `(42)` / `-` | Whole integer without thousand separators |
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
    Subtotal,  // Intermediate summary (bold + single thin border)
    Total,     // Final bottom-line summary (bold + double underline)
}
```

### 3.1. Symmetrical Rendering

| Style | Row Application | Column Application | Cell Override Application |
| :--- | :--- | :--- | :--- |
| `data` | Standard row typography and borders | Standard column typography and borders | Resets/suppresses summary borders (e.g. memo or descriptor cells on total rows) |
| `subtotal` | Bold text + top thin border | Bold text + left thin border (separating timeline or segments) | Bold text + top thin border |
| `total` | Bold text + top thin border + bottom double underline | Bold text + left thin border + right double border | Bold text + top thin border + bottom double underline |

### 3.2. Visual Emphasis (`highlight`)

Visual emphasis (soft accent background fill tint and text color) is completely orthogonal to structural borders and declared via `highlight: true`:
- Row: `"highlight": true` tints the entire row across measure columns.
- Column: `"highlight": true` tints the entire column down measure rows.
- Cell: `"highlight": true` tints the individual cell coordinate.

### 3.3. Background Zone Shade (`Shade`)

Background zone shading (`shade`) applies subtle theme-managed fills to distinguish historical vs. stub vs. forecast columns or horizontal row bands without modifying cell text colors:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Shade {
    #[default]
    None,    // Unshaded default; point-level cell escape hatch
    Muted,   // Subtle neutral slate fill (e.g. historical / actual columns)
    Accent,  // Soft cool tint (e.g. stub / LTM / bridge columns)
    Alt,     // Secondary contrast band (e.g. KPI / memo rows)
}
```

- Body-cell scope: Column shading applies strictly to table body cells (column headers and super-headers remain styled via `col_header_fill` and `super_header_fill`).
- Continuous row bands: Row shading applies across both `descriptor` and `measure` columns so horizontal bands span the full table width.
- Point-level reset: Declaring `"shade": "none"` on a cell override clears any inherited row or column shade.

---

## 4. Display Precision (`decimals`)

Numeric precision is an orthogonal property decoupled from the format preset:
- Separation of unit and precision: `currency` represents monetary formatting, while `decimals` controls whether it renders as whole dollars (`$1,250,000` with `decimals: 0`), EPS/cents (`$1.25` with `decimals: 2`), or cap table share prices (`$0.0125` with `decimals: 4`).
- Theme defaults: When `decimals` is omitted, the compiler defaults to the active theme's standard precision for that preset (`currency` -> 0, `number` -> 0, `percentage` -> 1, `multiple` -> 1, `basis_points` -> 0).
- Numeric applicability: `decimals` applies strictly to decimal-supporting numeric presets (`currency`, `number`, `percentage`, `multiple`, `basis_points`). Non-decimal presets (`integer`, `date`, `text`) ignore `decimals` during resolution and trigger a validation error (`InvalidPrecisionForPreset`) if explicitly configured.

### 4.1. Symmetrical Usage in Specifications
The `style`, `highlight`, `format`, and `decimals` properties are symmetrically available on `ColumnSpec`, `RowSpec`, and point-level `DetailedCellInput` (see `docs/dsl/spec-format.md`):

```json
{
  "columns": [
    { "kind": "data", "id": "q1" },
    { "kind": "data", "id": "q2" },
    { "kind": "data", "id": "q3" },
    { "kind": "data", "id": "q4" },
    { "kind": "data", "id": "fy", "style": "total" }
  ],
  "rows": [
    { "kind": "data", "id": "revenue", "format": "currency" },
    { "kind": "data", "id": "cogs", "format": "currency" },
    { "kind": "data", "id": "gross_profit", "style": "subtotal", "format": "currency" },
    { "kind": "data", "id": "gross_margin", "format": "percentage", "decimals": 1 },
    { "kind": "data", "id": "diluted_eps", "format": "currency", "decimals": 2 },
    { "kind": "data", "id": "shares_out", "format": "number", "decimals": 0 },
    { "kind": "data", "id": "net_income", "style": "total", "format": "currency" },
    { "kind": "data", "id": "irr", "highlight": true, "format": "percentage", "decimals": 2 },
    { "kind": "data", "id": "ev_ebitda", "format": "multiple", "decimals": 1 }
  ]
}
```

---

## 5. Property Resolution Lattice & Precedence

Every coordinate `(r, c)` resolves its effective format preset, precision, structural style, and highlight state through the deterministic 3-tier specificity lattice:

`Point Override (Cell) > Vector Rule (Row or Column) > Default Fallback`

### 5.1. Number format resolution
1. Cell format override (Tier 1): An explicit `format` declared on `CellInput` always wins.
2. Descriptor boundary: Descriptor columns (`role: "descriptor"`) isolate text formatting. Measure formats (`currency`, `percentage`, etc.) never broadcast to descriptor columns.
3. Measure vector conflict: If measure column `c` and row `r` specify differing format presets (e.g. `row.format: "currency"` and `col.format: "percentage"`), compilation halts with `FormatConflictError` unless resolved by an explicit cell format override.
4. Single vector format: If only the row or only the column specifies a format preset, that preset applies across the measure cells.
5. Default fallback: Unspecified numbers default to general numeric presentation; unspecified text defaults to standard string alignment.

### 5.2. Precision (`decimals`) resolution
1. Numeric applicability: `decimals` resolution applies strictly to decimal-supporting numeric presets (`currency`, `number`, `percentage`, `multiple`, `basis_points`). Non-decimal presets (`integer`, `date`, `text`) have no fractional precision concept and always resolve `decimals: None`. Supported fractional precision is bounded to `0 <= N <= 14` (exceeding 14 raises `PrecisionOutOfBounds`).
2. Cell decimals override (Tier 1): An explicit `decimals` declared on `CellInput` always wins.
3. Measure vector conflict: If measure column `c` and row `r` specify differing `decimals` (e.g. `row.decimals: 2` and `col.decimals: 0`), compilation halts with `DecimalsConflictError` unless resolved by an explicit cell override.
4. Single vector decimals: If only the row or only the column specifies `decimals`, that precision applies.
5. Theme default fallback: Unspecified decimals fall back to the active theme's configured default for the resolved `FormatPreset` (e.g. 0 for `currency` and `number`, 1 for `percentage` and `multiple`, 0 for `basis_points`).

### 5.3. Currency resolution & 3-tier specificity lattice
Monetary denominations use canonical 3-letter lowercase ISO codes (`CurrencyCode`, e.g. `"usd"`, `"eur"`, `"gbp"`, `"jpy"`, `"chf"`) and resolve via a deterministic 3-tier specificity lattice:
`Point Override (Cell) > Vector Rule (Row or Column) > Table Default (defaults to USD)`

1. Cell override (Tier 1): An explicit `currency` code (e.g. `"currency": "eur"`) on `DetailedCellInput` always takes precedence for that coordinate.
2. Vector rule (Tier 2): Declaring `currency` on a `ColumnSpec` or `RowSpec` applies across measure cells (e.g. an FX conversion table with an `eur_amount` column). If a measure row and measure column declare differing currencies without a cell override, compilation halts with a currency conflict error.
3. Table default (Tier 3): Declaring `currency` on `TableSpec` sets the default currency for all currency cells within that table (e.g. a European subsidiary cash flow statement). If omitted, the table defaults to `"usd"`.
4. Preset isolation: Only cells resolving `format == Some(FormatPreset::Currency)` receive a resolved currency. All other presets (`number`, `integer`, `percentage`, `multiple`, `basis_points`, `date`, `text`) resolve `currency: None`.
5. Descriptor boundary: Descriptor columns isolate calculation axes, resolve `currency: None`, and never trigger vector currency conflicts.

### 5.4. Style resolution & symmetrical 2D border composition

In a 2D matrix, rows and columns do not compete for a single border style. Instead, they govern orthogonal geometric axes:
- Rows: govern the horizontal axis (`top` and `bottom` edges).
- Columns: govern the vertical axis (`left` and `right` edges).
- Cell overrides: act as the point-level escape hatch to reset or enforce styles.

#### 5.4.1. Resolution algorithm
For coordinate `(r, c)`, the effective `(horizontal, vertical)` structural style pair resolves as follows:

1. Explicit cell reset: If `cell.style == Some(SemanticStyle::Data)`, both horizontal and vertical summary rules are suppressed:
   `(horizontal, vertical) = (Data, Data)`
   This provides the clean escape hatch for descriptor columns or memo cells on total rows (e.g. preventing double underlines under line item names).
2. Explicit cell summary rule: If `cell.style == Some(s)` where `s` is `Subtotal` or `Total`:
   `horizontal = s, vertical = col.style.unwrap_or(Data)`
   The cell applies the explicit horizontal summary rule while preserving column boundaries.
3. Vector rules (default): When `cell.style` is `None`:
   `horizontal = row.style.unwrap_or(Data), vertical = col.style.unwrap_or(Data)`

#### 5.4.2. Symmetrical border and summary composition & emission
During OpenXML emission (Pass 6b), the emitter maps `(horizontal, vertical)` through the active `Theme`:

| Coordinate Scenario | Resolved `horizontal` | Resolved `vertical` | Applied Excel Borders | Fill & Typography | Typical Rendered Appearance |
| :--- | :---: | :---: | :--- | :--- | :--- |
| **Standard Data Cell** | `Data` | `Data` | None | `data_fill`, `data_text` (or `input_text` / `formula_text`), Normal weight | Standard data values |
| **Row Subtotal** *(e.g. Gross Profit)* | `Subtotal` | `Data` | **Top**: `subtotal_border_top` | `subtotal_fill`, `subtotal_text`, **Bold** | Intermediate section subtotal |
| **Row Total** *(e.g. Net Income)* | `Total` | `Data` | **Top**: `total_border_top`<br>**Bottom**: `total_border_bottom` (`total_border_color`) | `total_fill`, `total_text`, **Bold** | Institutional accounting total line |
| **Column Subtotal** *(e.g. Segment divider)* | `Data` | `Subtotal` | **Left**: `col_subtotal_border_left` | `subtotal_fill`, `subtotal_text`, **Bold** | Vertical section dividing line |
| **Column Total** *(e.g. FY Total column)* | `Data` | `Total` | **Left**: `col_total_border_left`<br>**Right**: `col_total_border_right` (`total_border_color`) | `total_fill`, `total_text`, **Bold** | Vertical total summary column |
| **Intersection (Grand Total / Punchline)** | `Total` | `Total` | **Top/Left**: Thin, **Bottom/Right**: Double (`total_border_color`) | `total_fill`, `total_text`, **Bold** | **Framed accounting total box** |
| **Descriptor / Memo on Total Row** *(with `style: "data"`)* | `Data` | `Data` | None | `data_fill`, `data_text`, Normal weight | Clean label without unwanted underlines |

#### 5.4.3. Font weight composition
A cell renders with **bold typography** if either axis demands subtotal or total styling:
`is_bold = horizontal in (Subtotal, Total) or vertical in (Subtotal, Total)`

#### 5.4.4. Fill, text color, and border orthogonality
- Summary vs data shading: Standard data cells apply `theme.palette.data_fill` and `data_text`. Subtotals apply `subtotal_fill` and `subtotal_text`. Totals apply `total_fill` and `total_text`.
- Background zone shade (`shade`): Maps `Shade::Muted` -> `shade_muted_fill`, `Shade::Accent` -> `shade_accent_fill`, and `Shade::Alt` -> `shade_alt_fill`. Background fill precedence is:
  `is_highlight > total_fill.or(shade_fill) > subtotal_fill.or(shade_fill) > shade_fill.or(data_fill)`
  When `total_fill` or `subtotal_fill` is `"none"` (transparent), shaded columns continue seamlessly across subtotal and total rows; when a theme defines an opaque summary fill, the summary fill takes precedence over `shade_fill`. `shade` never alters cell font colors.
- Financial hardcode color coding: In standard data cells, literal cell inputs (`ValueLiteral`) apply `theme.palette.input_text` (institutional blue, e.g. `#0000FF`), while calculated formula cells apply `theme.palette.formula_text` (standard black, e.g. `#000000`).
- Visual emphasis (`highlight`): Declaring `highlight: true` overrides cell fills and text colors with `highlight_fill` and `highlight_text`.
Because `style`, `shade`, and `highlight` are distinct orthogonal properties, they compose cleanly: a table can apply `shade: "muted"` on historical columns, `style: "total"` on a summary row, and `highlight: true` on a punchline IRR cell without schema collision.

#### 5.4.5. Highlight resolution
`effective_highlight = cell.highlight.unwrap_or(row.highlight or col.highlight)`
A point override `highlight: false` explicitly un-highlights a cell even if its row or column has `highlight: true`.

#### 5.4.6. Shade resolution & symmetrical vector conflict
1. Cell override (Tier 1): An explicit `shade` declared on `CellInput` always wins. Declaring `shade: "none"` resolves to `None`, clearing any row or column shade.
2. Symmetrical vector rule (Tier 2): `row.shade` (spanning both descriptor and measure columns) and `col.shade` sit at the same vector tier. If a row and column specify differing active shades (`row_shade != col_shade`, where neither is `"none"`) without a cell override, compilation halts with `IrError::ShadeConflict` (`SpecificationError` in Python) to prevent unintentional crosshair/plaid table rendering. Identical shades (`row_shade == col_shade`) resolve without conflict.
3. Default fallback: Unshaded (`None`).

---

## 6. Pluggable Themes (IDE-Style Config)

f8 supports pluggable, external workbook themes inspired by modern IDE and editor color themes (e.g. VS Code, TextMate). The visual identity of an emitted workbook is completely isolated into standalone theme configurations.

### 6.1. Theme declaration

`Model(name="...", theme=...)` configures the active workbook theme either by referencing a built-in preset name (`str`) or by passing a custom theme dictionary (`Mapping[str, object]`). If omitted, it defaults to `"corporate_navy"`:

```json
{
  "model_name": "Project Titan LBO",
  "theme": "corporate_navy",
  "sheets": []
}
```

### 6.2. Built-in theme

f8 provides `corporate_navy` as its built-in institutional theme:
- `corporate_navy`: Institutional private equity and investment banking style with deep navy headers, underlined section titles on transparent backgrounds, crisp thin borders, and classic blue inputs.

Custom themes can be supplied in-memory via `Model(theme=custom_theme_dict)` in Python or `ThemeSpec::Custom` on `ProjectManifest` in Rust (see schema below).

### 6.3. Theme JSON configuration schema

Custom themes conform to a structured schema defining typography, palette, borders, row height metrics, and number format masks:

```json
{
  "$schema": "https://f8.dev/schemas/theme.json",
  "name": "corporate_navy",
  "typography": {
    "font_family": "Aptos",
    "title_size": 14.0,
    "subtitle_size": 9.0,
    "super_header_size": 10.0,
    "col_header_size": 10.0,
    "section_header_size": 10.0,
    "data_size": 10.0,
    "section_header_underline": true
  },
  "metrics": {
    "title_row_height": 24.0,
    "header_row_height": 18.0,
    "section_row_height": 18.0,
    "data_row_height": 15.0,
    "gutter_row_height": 9.0,
    "gutter_col_width": 3.0,
    "title_gutter_row_height": 8.0,
    "char_width_multiplier": 1.0,
    "col_padding": 1.0,
    "min_col_width": 12.0
  },
  "palette": {
    "table_title_fill": "#1B365D",
    "table_title_text": "#FFFFFF",
    "table_subtitle_text": "#4B5563",
    "super_header_fill": "#ced4da",
    "super_header_text": "#000000",
    "col_header_fill": "none",
    "col_header_text": "#000000",
    "section_header_fill": "none",
    "section_header_text": "#000000",
    "data_fill": "none",
    "data_text": "#000000",
    "subtotal_fill": "none",
    "subtotal_text": "#000000",
    "total_fill": "none",
    "total_text": "#000000",
    "input_text": "#0000FF",
    "formula_text": "#000000",
    "shade_muted_fill": "#F8FAFC",
    "shade_accent_fill": "#EFF6FF",
    "shade_alt_fill": "#F1F5F9",
    "highlight_fill": "#FEF3C7",
    "highlight_text": "#000000",
    "gridline_color": "#E5E7EB",
    "border_color": "#000000",
    "total_border_color": "#1B365D"
  },
  "borders": {
    "table_title_border_bottom": "none",
    "super_header_border_bottom": "none",
    "header_border_bottom": "thin",
    "descriptor_border_right": "none",
    "subtotal_border_top": "thin",
    "total_border_top": "thin",
    "total_border_bottom": "double",
    "col_subtotal_border_left": "thin",
    "col_total_border_left": "thin",
    "col_total_border_right": "double"
  },
  "number_formats": {
    "currency": "{sym}#,##0{dec}_);({sym}#,##0{dec});\"-\"_)",
    "number": "#,##0{dec}_);(#,##0{dec});\"-\"_)",
    "percentage": "0{dec}%_);(0{dec}%);\"-\"_)",
    "multiple": "0{dec}\"x\"",
    "basis_points": "#,##0{dec}\" bps\"",
    "date": "yyyy-mm-dd",
    "text": "@",
    "integer": "0;(-0);\"-\"_)",
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

### 6.4. Dynamic decimal and currency mask generation (`{dec}` and `{sym}`)

Themes declare format pattern templates utilizing `{dec}` and `{sym}` placeholders. During OpenXML compilation, the emitter dynamically synthesizes the concrete format string by globally replacing all occurrences:
- `decimals == 0`: `{dec}` expands to empty string `""` (e.g. `$#,##0_);($#,##0);"-"_)`, `#,##0_);(#,##0);"-"_)`).
- `decimals > 0`: `{dec}` expands to `.` followed by `N` zeros (e.g. for `decimals: 2`, `{dec}` -> `.00`, yielding `$#,##0.00_);($#,##0.00);"-"_)`). Maximum supported decimal precision is `N <= 14` (in table specs, precision exceeding 14 raises `PrecisionOutOfBounds`; in theme configurations or expansions, exceeding 14 raises `ThemePrecisionOutOfBounds`).
- `{sym}`: Expands to the display symbol of the resolved `CurrencyCode` (defaulting to `CurrencyCode::Usd` / `"$"` if omitted). Standard single-character symbols (`$`, `€`, `£`, `¥`) substitute directly; multi-character codes and non-ASCII glyphs (`CHF`, `kr`, `C$`, `A$`, `₹`) are safely wrapped in double quotes (e.g. `"\"CHF\""`, `"\"₹\""`) to prevent collision with Excel format syntax tokens.

This completely eliminates combinatorial mask explosion while giving model authors arbitrary precision control across any numerical preset and seamless international currency localization.

### 6.5. Two-phase theme utilization in the compiler

The resolved theme is utilized across two compiler passes:
1. Pass 5 (Spatial Layout Planning): Consumes theme `typography` and `metrics` to compute physical row heights and font-scaled sheet-wide column widths. Assigns semantic `StyleRole` tags to cells.
2. Pass 6b (Excel Binary Emission): Consumes theme `palette`, `borders`, and `number_formats` with dynamic `{dec}` expansion to instantiate concrete `rust_xlsxwriter::Format` styles and write cells.

This guarantees that changing themes re-skins the emitted workbook completely and resizes rows and columns without touching any table logic, formulas, or ASTs.

---

See `docs/dsl/spec-format.md` for schema usage and `docs/architecture.md` for spatial layout coordinate and metric assignment.
