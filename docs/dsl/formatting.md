# Semantic Formatting & Themes

This document specifies Finx's semantic formatting system, preset catalog, vector format resolution rules, and pluggable workbook themes.

## 1. Core Philosophy

Finx enforces complete decoupling between calculation logic and visual styling:
- **Semantic intent over raw masks**: Model authors specify *what* a number represents (`currency`, `percentage`, `multiple`), never raw Excel format strings (e.g. `_($* #,##0_);_($* (#,##0);_($* "-"_);_(@_)`).
- **Deterministic emission**: The compiler and pluggable themes translate semantic presets into institutional-grade formatting rules and accounting styles upon `.xlsx` emission.
- **Zero layout drift**: Format rules never alter underlying floating-point precision or calculation semantics.

---

## 2. Standard Format Preset Catalog

The `format` property on columns, rows, or cell overrides accepts strictly enumerated semantic presets:

| Format Preset | Semantic Purpose | Typical Rendered Appearance | Accounting Style |
| :--- | :--- | :--- | :--- |
| `currency` | Standard monetary amounts | `$1,250,000` / `(125,000)` / `-` | Accounting currency format with parentheses for negatives |
| `currency_total` | Bottom-line and summary totals | `$1,250,000` | Bold text, top thin border, bottom double underline |
| `percentage` | Rates, margins, and return metrics | `12.5%` or `12.50%` | Standard percentage formatting |
| `multiple` | Valuation and leverage multiples | `12.4x` | Decimal with trailing lowercase `x` |
| `integer` | Discrete counts, volumes, shares | `1,000` / `(500)` | Comma-separated integer with negative parentheses |
| `decimal` | General floating point values | `12.34` | Fixed 2-decimal precision |
| `basis_points` | Spread and yield differences | `125 bps` | Integer with trailing `bps` suffix |
| `date` | Calendar dates and period markers | `2024-12-31` or `Dec-24` | Institutional date format |
| `text` | Unformatted textual strings | `Line Item Name` | Left-aligned string formatting |

---

## 3. Format Resolution & Conflict Invariants

Every cell coordinate $(r, c)$ resolves its effective format preset through the deterministic property lattice:

1. **Cell format override (Tier 1)**: An explicit point format declared on `CellInput` always wins.
2. **Descriptor boundary**: Descriptor coordinates (`role: "descriptor"`) isolate text formatting. Measure formats (`currency`, `percentage`, etc.) never broadcast to descriptor columns or rows.
3. **Measure vector conflict**: If row $r$ and column $c$ are both measures and specify differing format presets (e.g. `row.format: "currency_total"` and `col.format: "percentage"`), compilation halts with `FormatConflictError` unless resolved by an explicit cell format override.
4. **Single vector format**: If only the row or only the column specifies a format preset, that preset applies across the measure cells.
5. **Default fallback**: Unspecified numbers default to general numeric presentation; unspecified text defaults to standard string alignment.

---

## 4. Pluggable Themes

The root workbook manifest (`finx.json`) declares the active `theme`:

```json
{
  "model_name": "Acme Corp Financial Model",
  "theme": "classic_navy",
  "sheets": []
}
```

### 4.1. Theme provider responsibilities
The theme defines the visual identity of the emitted workbook:
- **Palette**: Header fill colors, accent colors, super-header shading, and gutter styling.
- **Typography**: Header font family, data font family, font sizes, and weight hierarchy.
- **Borders**: Gridline treatment, column group dividing borders, and subtotal/total underline styles.
- **Number format masks**: Precise regional accounting currency masks and symbol placements.

---

See [Specification Format](spec-format.md) for schema usage and [Intermediate Representation (IR)](../compiler/ir.md) for compiler resolution algorithms.
