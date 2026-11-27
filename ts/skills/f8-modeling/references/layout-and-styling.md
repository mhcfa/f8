# Layout and Styling Reference

Table visual structure, 2D `BandInput`/`LaneInput` sheet placement, `auto` vs. `grid` layout contracts, `SheetPrintSpecInput`/`HeaderFooterInput` page setup, formatting presets, semantic styles, shades, currencies, and themes.

## 1. Table Structure, Banners, and Spacing

```ts
table.addSectionHeader(title: string, options?: { indent?: number; note?: string }): this
table.addRowGutter(): this
table.addColumnGutter(): this
```

- Section headers & gutters:
  - `addSectionHeader`: Consumes 1 physical row at the table's start column with section-header styling, optional `indent` and `note`, and zero calculation cells.
  - `addRowGutter` / `addColumnGutter`: Consumes 1 blank physical row or narrow column (`gutter_col_width = 2.0` default) with zero calculation cells. Row formatting continues across column gutters; row gutters stay blank. Skipped by vector formulas, `prev`/`next` offsets, and `setCells` `AxisSlice` selectors; formula `start..end` slices cannot cross gutters or section headers (`ReferenceResolutionError`).
- Title banners, subtitles & collapsing:
  - `title`: Non-empty (non-whitespace) `title` emits a banner spanning from the table's start column to the sheet's rightmost column if in the last lane of any band (including single-lane bands), or to that lane's rightmost column in non-last lanes. Subtitles, headers, section headers, body borders, and widths stay bounded to the table's own columns.
  - `subtitle`: Single `string` or `readonly string[]` (normalized to `table.subtitles: readonly string[]`), emitting 1 row per entry beneath `title` (empty/whitespace entries throw `SpecificationError`; omit or pass `[]` when unused). Presence of `title` or `subtitle` inserts a 1-row blank buffer before headers/body; blank/whitespace `title` with no `subtitle` omits both banner and buffer.
  - `collapsible: true`: Groups rows below the table's first physical row in an Excel outline with summary symbols above.

## 2. 2D Worksheet Placement (`BandInput`, `LaneInput`, `setBands`)

```ts
export type TableRef = string | Table
export interface LaneInput { readonly tables: readonly TableRef[]; readonly tableGap?: number }
export type LaneInputItem = LaneInput | TableRef
export interface BandInput {
  readonly lanes: readonly LaneInputItem[]
  readonly gap?: number
  readonly laneGap?: number
  readonly pageBreakBefore?: boolean
}
export type BandInputItem = BandInput | TableRef

sheet.setBands(bands: readonly BandInputItem[]): this
```

- Canvas & defaults: Row 1 and Column A are reserved blank margins; Band `0` starts at `B2`. Without `setBands`, each table gets its own 1-lane, 1-table vertical band in `addTable` order. Default spacing: `gap = 2` rows between bands, `laneGap = 1` column between lanes, `tableGap = 2` rows between tables in a lane.
- Bare `TableRef` shorthand: Pass a `Table` or `string` ID directly in `setBands([...])` for a 1-table band or inside `lanes: [...]` for a 1-table lane (`{ lanes: [left, right] }`, `{ lanes: [{ tables: [tl, bl] }, tr] }`, `{ lanes: [card], gap: 1 }`). Multi-table vertical lanes require explicit `{ tables: [...] }` objects (nested `TableRef[]` arrays are rejected).
- Non-overlap & ordering: Tables cannot overlap; each lane's width is the max end column across its tables, shifting subsequent lanes right automatically. Compiled outputs (`summary()`, `errors()`, `inspectSheet()`, `toMarkdown()`) follow canonical `bands -> lanes -> tables` order.
- Validation: Enforces exact partition of all tables on the sheet (re-validated on `toSpec()` if `addTable` follows `setBands`). Throws `Error` on empty `lanes`/`tables`, nested arrays or invalid items, negative/non-integer gaps, unused gaps (`gap` on Band `0`, `laneGap` on 1-lane band, `tableGap` on 1-table lane), foreign/duplicate/missing tables, and `pageBreakBefore: true` on Band `0`. `setBands([])` is valid on an empty sheet; calling `setBands` again replaces the layout.

```ts
sheet.setBands([
  { lanes: [{ tables: [tl, bl] }, tr], laneGap: 1 },
  timeline, // bare TableRef shorthand for a 1-table band
])
```

## 3. Sheet Layout Modes (`auto` vs. `grid`)

Selected on `model.addSheet(name, { layout: "auto" | "grid", colWidth })`:

- `layout: "auto"` (default):
  - Sizes each physical column independently to the widest header or data value across all tables spanning it (clamped between theme `min_col_width` and `max_col_width`, defaults `12.0..75.0`; `group` banners excluded from single-column width). Column A and gutter/gap-only columns use `gutter_col_width` (`2.0`).
  - Text cells wider than `max_col_width` or containing `\n` enable text wrap; Excel auto-fits row heights.
- `layout: "grid"` (optional `colWidth` in `1..=255`, defaults to `theme.metrics.min_col_width`):
  - Assigns one uniform width `W` to every physical column covered by any data column, widening `W` above base `colWidth` to fit the widest measure or non-text descriptor cell. Ignores `max_col_width`; if base `colWidth` or computed `W` exceeds `255`, compilation fails with `SpecificationError` (never clamped). Column A and gutter/gap-only columns keep `gutter_col_width`.
  - Descriptor overflow spans: `descriptor` columns auto-span contiguous physical columns (`>= 1`) with zero merged cells. Anchor cells remain in the first column (preserving vertical `XLOOKUP`/`XMATCH`/`INDEX` slices and contiguous `measure` ranges), while blank runway cells carry continuous fills and horizontal summary borders (`subtotal`/`total`) and place right borders on the final column.
  - Leading descriptor alignment: Lanes finalize left-to-right; tables sharing `(laneIndex, startCol)` with at least one leading `descriptor` before their first `measure` column align their leading descriptor block (excluding trailing `addColumnGutter()` spacers). Descriptor-only tables skip alignment; span growth shifts subsequent lanes right automatically.
  - Hard `grid` constraints (`SpecificationError`): Text wrapping is disabled, `\n`/`\r` line breaks are forbidden in data cells, column `headers`, and `group` banners, and every non-empty `measure` column header must fit within static base `colWidth`.

## 4. Print Setup and Page Breaks (`SheetPrintSpecInput`, `HeaderFooterInput`)

```ts
export interface HeaderFooterInput {
  readonly left?: string
  readonly center?: string
  readonly right?: string
}
export interface SheetPrintSpecInput {
  readonly paperSize: PaperSize // "a4" | "letter"
  readonly orientation?: PageOrientation // "landscape" (default) | "portrait"
  readonly fit?: PrintFitMode // "fit_width" (default) | "fit_page"
  readonly centerHorizontally?: boolean // default true
  readonly header?: HeaderFooterInput
  readonly footer?: HeaderFooterInput
}
export interface SheetPrintOverrideInput {
  readonly paperSize?: PaperSize
  readonly orientation?: PageOrientation
  readonly fit?: PrintFitMode
  readonly centerHorizontally?: boolean
  readonly header?: HeaderFooterInput | "none"
  readonly footer?: HeaderFooterInput | "none"
}
```

- Opt-in page setup: Omit `print` to leave Excel page setup untouched. When enabled, sets `normal` view, hides print gridlines, applies fixed print margins, and auto-trims `Print_Area` to exclude the blank Col A / Row 1 margin (`B2:<last_col><last_row>`; omitted on empty sheets). Preserved identically under `valuesOnly: true`.
- Required & default fields: `paperSize` (`"a4" | "letter"`) is required on the resolved sheet config (no regional default). Defaults: `orientation: "landscape"`, `fit: "fit_width"` (`1 x auto`; `"fit_page"` is `1 x 1`), `centerHorizontally: true`.
- Headers & footers: `HeaderFooterInput` requires at least one of `left`, `center`, `right`. Placeholders: `{page}`, `{pages}`, `{sheet}`, `{model}`, `{date}`, plus `{{`/`}}` for literal braces (`&` is auto-escaped).
  - Excel Online caveat: `{date}` emits Excel `&D`; Desktop Excel evaluates it, whereas Excel Online PDF export omits `&D` — inline a static date string if targeting Excel Online.
  - Validation (`SpecificationError`): Rejects unknown/unmatched `{...}`, control chars (including inside `{model}`), post-expansion empty/whitespace zones, raw `&[...]` Excel macros, invalid `theme.typography.font_family`, and oversized strings.
- Workbook-to-sheet cascade: `new Model({ print })` sets defaults (requires `paperSize`; rejects `"none"` on `header`/`footer`). On `model.addSheet(name, { print })` / `new Sheet(name, { print })`, omitted/`undefined` inherits `model.print`, `"none"` disables printing on that sheet, and a `SheetPrintOverrideInput` shallow-merges fields (overriding `header`/`footer` replaces that section wholesale; `header: "none"` / `footer: "none"` clears an inherited section; unknown keys throw `Error` in builders and `SpecificationError` in `compile()`).
- Page breaks: `{ lanes: [...], pageBreakBefore: true }` inserts a manual horizontal page break before Band `b > 0`. Valid only when sheet `print` is enabled with `fit: "fit_width"` (rejected on Band `0`, without `print`, or with `fit: "fit_page"`). On vertically stacked sheets, switch to `setBands` with bare `TableRef` + `{ lanes: [tbl], pageBreakBefore: true }`:

```ts
sheet.setBands([
  incomeStmt, // Band 0
  { lanes: [balanceSheet], pageBreakBefore: true },
  { lanes: [cashFlow], pageBreakBefore: true },
])
```

## 5. Formats, Styles, Shades, Currencies, and Themes

### Format Presets (`FormatPreset`)

| Preset           | Default `decimals` | Supports `decimals` | Supports `currency` | In-memory sample (`+` / `-` / `0`)                 | Notes                                                         |
| :--------------- | :----------------- | :------------------ | :------------------ | :------------------------------------------------- | :------------------------------------------------------------ |
| `"currency"`     | `0`                | `0..=14`            | Yes (default `USD`) | `"$1,250"` / `"($1,250)"` / `"$0"`                 | Excel: zero displays `-`                                      |
| `"number"`       | `0`                | `0..=14`            | No                  | `"1,250"` / `"(1,250)"` / `"0"`                    | Excel: zero displays `-`                                      |
| `"percentage"`   | `1`                | `0..=14`            | No                  | `"15.0%"` / `"(15.0%)"` / `"0.0%"`                 | Excel: zero displays `-`                                      |
| `"multiple"`     | `1`                | `0..=14`            | No                  | `"12.5x"` / `"(12.5x)"` / `"0.0x"`                 | Excel: zero displays `-`                                      |
| `"basis_points"` | `0`                | `0..=14`            | No                  | `0.015` -> `"150 bps"` / `"(150 bps)"` / `"0 bps"` | In-memory scales `10,000x`; Excel: unscaled, zero `-`         |
| `"integer"`      | `null`             | No                  | No                  | `"2024"` / `"(2024)"` / `"0"`                      | Excel: neg `(-2024)`, zero `-`                                |
| `"date"`         | `null`             | No                  | No                  | `45657` -> `"2024-12-31"`                          | Expects Excel serial (Excel: `yyyy-mm-dd`)                    |
| `"text"`         | `null`             | No                  | No                  | `"Sample"`                                         | Excel: text mask                                              |
| `null` (default) | `null`             | In-memory only      | No                  | `"1,250"` / `"(12.5)"` / `"0"`                     | In-memory: 1 dec (trims `.0`) or `decimals`; Excel: `General` |

### Semantic Styles (`SemanticStyle`)

- `"data"` (default): Regular font, `data_fill`, no summary borders.
- `"bold"`: Bold font only; no borders, standard fill/text precedence (preserves `input_text` on `measure` literals).
- `"subtotal"`: Bold font, `subtotal_fill`/`subtotal_text`, top border on rows, dashed left border on columns.
- `"total"`: Bold font, `total_fill`/`total_text`, thin top + double bottom border on rows, thin left border on columns.

### Background Shades (`Shade`) and Precedence

- Values: `"none"` (clears inherited row/col shade on a 0D override), `"muted"` (`shade_muted_fill`), `"alt"` (`shade_alt_fill`).
- Fill precedence: `highlight_fill` > `total_fill` (or shade) > `subtotal_fill` (or shade) > shade > `data_fill`.
- Font color precedence: `input_text` (`measure` literal inputs) > `highlight_text` > `total_text` > `subtotal_text` > `data_text` (`descriptor` cells) > `formula_text`.

### Currency Codes (`CurrencyCode`)

3-letter ISO codes (normalized to lowercase by builders and `compile(...)`):
`USD`, `EUR`, `GBP`, `JPY`, `CHF`, `CAD`, `AUD`, `CNY`, `HKD`, `SGD`, `SEK`, `NOK`, `NZD`, `INR`, `BRL`.

### Theme Resolution (`ThemeInput = string | Readonly<Record<string, unknown>> | URL`)

- `string` (default `"corporate_navy"`): Built-in preset name (does not read disk; unknown preset throws `SpecificationError`).
- `URL` (e.g. `pathToFileURL("theme.json")` or `new URL("./theme.json", import.meta.url)`): Reads JSON theme file via `node:fs` into `model.theme`. Scaffold or inspect the default schema via `f8 theme init` or `defaultTheme()`.
- `Readonly<Record<string, unknown>>`: Inline theme object validated for hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA`, plus `"none"`/`"transparent"` on fills), positive finite typography sizes and layout metrics (`> 0`, except `col_padding >= 0`, and `min_col_width <= max_col_width`), `{dec}`/`{sym}` templates, and default `decimals <= 14`.
