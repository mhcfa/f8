# Layout and Styling Reference

Table visual structure, 2D `Band`/`Lane` sheet placement, `auto` vs. `grid` layout contracts, `PrintSetup`/`HeaderFooter` page setup, formatting presets, semantic styles, shades, currencies, and themes.

## 1. Table Structure, Banners, and Spacing

```python
table.add_section_header(title: str, indent: int | None = None, note: str | None = None) -> Table
table.add_row_gutter() -> Table
table.add_column_gutter() -> Table
```

- Section headers & gutters:
  - `add_section_header`: Consumes 1 physical row at the table's start column with section-header styling, optional `indent` and `note`, and zero calculation cells.
  - `add_row_gutter` / `add_column_gutter`: Consumes 1 blank physical row or narrow column (`gutter_col_width=2.0` default) with zero calculation cells. Row formatting continues across column gutters; row gutters stay blank. Skipped by vector formulas, `prev`/`next` offsets, and `set_cells` `slice(...)` selectors; formula `start..end` slices cannot cross gutters or section headers (`ReferenceResolutionError`).
- Title banners, subtitles & collapsing:
  - `title`: Non-empty (non-whitespace) `title` emits a banner spanning from the table's start column to the sheet's rightmost column if in the last lane of any band (including single-lane bands), or to that lane's rightmost column in non-last lanes. Subtitles, headers, section headers, body borders, and widths stay bounded to the table's own columns.
  - `subtitle`: Single `str` or `Sequence[str]` (normalized to `table.subtitles: list[str]`), emitting 1 row per entry beneath `title` (empty/whitespace entries raise `SpecificationError`; pass `None` or `[]` to omit). Presence of `title` or `subtitle` inserts a 1-row blank buffer before headers/body; blank/whitespace `title` with no `subtitle` omits both banner and buffer.
  - `collapsible=True`: Groups rows below the table's first physical row in an Excel outline with summary symbols above.

## 2. 2D Worksheet Placement (`Band`, `Lane`, `set_bands`)

```python
type TableRef = str | Table
type LaneItem = Lane | TableRef
type BandItem = Band | TableRef

lane = f8.Lane(tables: Sequence[TableRef], *, table_gap: int | None = None)
band = f8.Band(
    lanes: Sequence[LaneItem],
    *,
    gap: int | None = None,
    lane_gap: int | None = None,
    page_break_before: bool = False,
)
sheet.set_bands(bands: Sequence[BandItem]) -> Sheet
```

- Canvas & defaults: Row 1 and Column A are reserved blank margins; Band `0` starts at `B2`. Without `set_bands`, each table gets its own 1-lane, 1-table vertical band in `add_table` order. Default spacing: `gap=2` rows between bands, `lane_gap=1` column between lanes, `table_gap=2` rows between tables in a lane.
- Bare `TableRef` shorthand: Pass a `Table` or `str` ID directly in `set_bands([...])` for a 1-table band or inside `Band([...])` for a 1-table lane (`Band([left, right])`, `Band([Lane([tl, bl]), tr])`, `Band([card], gap=1)`). Multi-table vertical lanes require explicit `Lane([...])` (nested sequences are rejected).
- Non-overlap & ordering: Tables cannot overlap; each lane's width is the max end column across its tables, shifting subsequent lanes right automatically. Compiled outputs (`summary()`, `errors()`, `inspect_sheet()`, `to_markdown()`) follow canonical `bands -> lanes -> tables` order.
- Validation: Enforces exact partition of all tables on the sheet (re-validated on `__f8_spec__` if `add_table` follows `set_bands`). Rejects empty `Band`/`Lane`, bare `str`/`Table` passed where a sequence is expected, negative/`bool`/non-int gaps, unused gaps (`gap` on Band `0`, `lane_gap` on 1-lane band, `table_gap` on 1-table lane), foreign/duplicate/missing tables, and `page_break_before=True` on Band `0`. `set_bands([])` is valid on an empty sheet; calling `set_bands` again replaces the layout.

```python
sheet.set_bands(
    [
        f8.Band([f8.Lane([tl, bl]), tr], lane_gap=1),
        timeline,  # bare TableRef shorthand for a 1-table band
    ]
)
```

## 3. Sheet Layout Modes (`auto` vs. `grid`)

Selected on `model.add_sheet(..., layout="auto" | "grid", col_width=...)`:

- `layout="auto"` (default):
  - Sizes each physical column independently to the widest header or data value across all tables spanning it (clamped between theme `min_col_width` and `max_col_width`, defaults `12.0..75.0`; `group` banners excluded from single-column width). Column A and gutter/gap-only columns use `gutter_col_width` (`2.0`).
  - Text cells wider than `max_col_width` or containing `\n` enable text wrap; Excel auto-fits row heights.
- `layout="grid"` (optional `col_width` in `1.0..=255.0`, defaults to `theme.metrics.min_col_width`):
  - Assigns one uniform width `W` to every physical column covered by any data column, widening `W` above base `col_width` to fit the widest measure or non-text descriptor cell. Ignores `max_col_width`; if base `col_width` or computed `W` exceeds `255.0`, compilation fails with `SpecificationError` (never clamped). Column A and gutter/gap-only columns keep `gutter_col_width`.
  - Descriptor overflow spans: `descriptor` columns auto-span contiguous physical columns (`>= 1`) with zero merged cells. Anchor cells remain in the first column (preserving vertical `XLOOKUP`/`XMATCH`/`INDEX` slices and contiguous `measure` ranges), while blank runway cells carry continuous fills and horizontal summary borders (`subtotal`/`total`) and place right borders on the final column.
  - Leading descriptor alignment: Lanes finalize left-to-right; tables sharing `(lane_idx, start_col)` with at least one leading `descriptor` before their first `measure` column align their leading descriptor block (excluding trailing `add_column_gutter()` spacers). Descriptor-only tables skip alignment; span growth shifts subsequent lanes right automatically.
  - Hard `grid` constraints (`SpecificationError`): Text wrapping is disabled, `\n`/`\r` line breaks are forbidden in data cells, column `headers`, and `group` banners, and every non-empty `measure` column header must fit within static base `col_width`.

## 4. Print Setup and Page Breaks (`PrintSetup`, `HeaderFooter`)

```python
hf = f8.HeaderFooter(left: str | None = None, center: str | None = None, right: str | None = None)
ps = f8.PrintSetup(
    paper_size: PaperSize | None = None,               # "a4" | "letter"
    orientation: PageOrientation | None = None,        # "landscape" (default) | "portrait"
    fit: PrintFitMode | None = None,                   # "fit_width" (default) | "fit_page"
    center_horizontally: bool | None = None,           # True (default)
    header: HeaderFooter | HeaderFooterDict | Literal["none"] | None = None,
    footer: HeaderFooter | HeaderFooterDict | Literal["none"] | None = None,
)
```

- Opt-in page setup: Omit `print` to leave Excel page setup untouched. When enabled, sets `normal` view, hides print gridlines, applies fixed print margins, and auto-trims `Print_Area` to exclude the blank Col A / Row 1 margin (`B2:<last_col><last_row>`; omitted on empty sheets). Preserved identically under `values_only=True`.
- Required & default fields: `paper_size` (`"a4" | "letter"`) is required on the resolved sheet config (no regional default). Defaults: `orientation="landscape"`, `fit="fit_width"` (`1 x auto`; `"fit_page"` is `1 x 1`), `center_horizontally=True`.
- Headers & footers: `HeaderFooter` / `HeaderFooterDict` requires at least one of `left`, `center`, `right`. Placeholders: `{page}`, `{pages}`, `{sheet}`, `{model}`, `{date}`, plus `{{`/`}}` for literal braces (`&` is auto-escaped).
  - Excel Online caveat: `{date}` emits Excel `&D`; Desktop Excel evaluates it, whereas Excel Online PDF export omits `&D` — inline a static date string if targeting Excel Online.
  - Validation (`SpecificationError`): Rejects unknown/unmatched `{...}`, control chars (including inside `{model}`), post-expansion empty/whitespace zones, raw `&[...]` Excel macros, invalid `theme.typography.font_family`, and oversized strings.
- Workbook-to-sheet cascade: `Model(..., print=...)` sets defaults (requires `paper_size`; rejects `"none"` on `header`/`footer`). On `add_sheet(..., print=...)` / `Sheet(..., print=...)`, `None` inherits `model.print`, `"none"` disables printing on that sheet, and a `PrintSetup` or `SheetPrintOverrideDict` shallow-merges fields (overriding `header`/`footer` replaces that section wholesale; `header="none"` / `footer="none"` clears an inherited section; unknown dict keys raise `ValueError`).
- Page breaks: `Band(..., page_break_before=True)` inserts a manual horizontal page break before Band `b > 0`. Valid only when sheet `print` is enabled with `fit="fit_width"` (rejected on Band `0`, without `print`, or with `fit="fit_page"`). On vertically stacked sheets, switch to `set_bands` with bare `TableRef` + `Band([tbl], page_break_before=True)`:

```python
sheet.set_bands(
    [
        income_stmt,  # Band 0
        f8.Band([balance_sheet], page_break_before=True),
        f8.Band([cash_flow], page_break_before=True),
    ]
)
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
| `"integer"`      | `None`             | No                  | No                  | `"2024"` / `"(2024)"` / `"0"`                      | Excel: neg `(-2024)`, zero `-`                                |
| `"date"`         | `None`             | No                  | No                  | `45657` -> `"2024-12-31"`                          | Expects Excel serial (Excel: `yyyy-mm-dd`)                    |
| `"text"`         | `None`             | No                  | No                  | `"Sample"`                                         | Excel: text mask                                              |
| `None` (default) | `None`             | In-memory only      | No                  | `"1,250"` / `"(12.5)"` / `"0"`                     | In-memory: 1 dec (trims `.0`) or `decimals`; Excel: `General` |

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

3-letter ISO codes (builders normalize to lowercase; raw `f8.compile(...)` requires lowercase):
`USD`, `EUR`, `GBP`, `JPY`, `CHF`, `CAD`, `AUD`, `CNY`, `HKD`, `SGD`, `SEK`, `NOK`, `NZD`, `INR`, `BRL`.

### Theme Resolution (`ThemeInput = Mapping[str, object] | Path | str`)

- `str` (default `"corporate_navy"`): Built-in preset name (does not read disk; unknown preset raises `SpecificationError`).
- `Path` (e.g. `Path("theme.json")`): Reads JSON theme file into `model.theme`. Scaffold or inspect the default schema via `f8 theme init` or `f8.default_theme()`.
- `Mapping[str, object]`: Inline theme dict validated for hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA`, plus `"none"`/`"transparent"` on fills), positive finite typography sizes and layout metrics (`> 0`, except `col_padding >= 0`, and `min_col_width <= max_col_width`), `{dec}`/`{sym}` templates, and default `decimals <= 14`.
