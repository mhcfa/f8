# Syntax and Grid Reference

2D coordinate grammar, container constructors, vector expansion, 3-tier attribute resolution, layout placement, and formatting presets.

---

## 1. 2D Coordinate Addressing Grammar

All formula cell references require explicit 2D bracket syntax:

`Table[RowSelector, ColSelector]`

### Coordinate Selectors

| Selector type              | Syntax                          | Resolution semantics                                                    | Example                                       |
| :------------------------- | :------------------------------ | :---------------------------------------------------------------------- | :-------------------------------------------- |
| **Active column**          | `Table["row", @col]`            | Current evaluation column                                               | `IS["revenue", @col] * 0.40`                  |
| **Active row**             | `Table[@row, "col"]`            | Current evaluation row                                                  | `Assumptions[@row, "rate"] * 100`             |
| **Absolute point**         | `Table["row", "col"]`           | Fixed coordinate (lowers to `$Col$Row`)                                 | `Assumptions["tax_rate", "val"]`              |
| **Column lag / lead (1)**  | `Table["row", col.prev]`        | Prior (`prev`) or next (`next`) `measure` column                        | `BS["cash", col.prev] + 10.0`                 |
| **Column lag / lead (k)**  | `Table["row", col.prev(k)]`     | `k` `measure` columns prior (`prev(k)`) or ahead (`next(k)`) (`k >= 0`) | `Rev["sales", col.prev(2)]`                   |
| **Row lag / lead (1)**     | `Table[row.prev, "col"]`        | Prior (`prev`) or next (`next`) data row                                | `Schedule[row.prev, "col"] * 1.05`            |
| **Row lag / lead (k)**     | `Table[row.prev(k), "col"]`     | `k` data rows prior (`prev(k)`) or ahead (`next(k)`) (`k >= 0`)         | `Schedule[row.next(2), "col"]`                |
| **Column slice**           | `Table["row", "c1".."c2"]`      | Inclusive horizontal range across columns                               | `SUM(Rev["sales", "2023A".."2025E"])`         |
| **Dynamic column slice**   | `Table["row", "c1"..@col]`      | Inclusive horizontal range between any two column endpoints             | `SUM(Debt["amort", "2024E"..col.prev])`       |
| **Row slice (active col)** | `Table["r1".."r2", @col]`       | Inclusive vertical range across rows at active column                   | `SUM(Opex["r_and_d".."g_and_a", @col])`       |
| **Dynamic row slice**      | `Table["r1"..@row, @col]`       | Inclusive vertical range between any two row endpoints                  | `SUM(Cohort["c1"..row.prev, @col])`           |
| **Row slice (fixed col)**  | `Table["r1".."r2", "col"]`      | Inclusive vertical range across rows at fixed column                    | `SUM(Comps["peer1".."peer5", "ev"])`          |
| **2D block slice**         | `Table["r1".."r2", "c1".."c2"]` | Inclusive 2D rectangular matrix range                                   | `SUM(Matrix["r1".."r3", "2023A".."2025E"])`   |

### Lexical and Grammatical Rules

- Table identifiers: Bare ASCII identifiers (`[A-Za-z0-9_]+` containing at least one ASCII letter or `_`) before `[`. Quoted table names (`"Table"[...]`), pure numbers (`123[...]`), or dot access (`Table.row`) raise `FormulaParseError`.
- Coordinate keys: Explicit row and column IDs inside `[...]` must be double-quoted strings (`"revenue"`, `"2024E"`). Unquoted keys (`Table[revenue, @col]`) raise `FormulaParseError`.
- Active axis tokens: Must be `@col` or `@row`. Bare `@` raises `FormulaParseError`.
- Uppercase identifiers: Function names (`[A-Z][A-Z0-9_]*`, starting with an ASCII uppercase letter) and boolean literals (`TRUE`, `FALSE`) must be uppercase (table identifiers before `[` may be mixed-case). Lowercase functions/booleans, `null`, or standalone identifiers raise `FormulaParseError`.
- Omitted arguments (`_`): Valid only as a direct argument in a function call (`PMT(rate, nper, pv, _, type)`). Using `_` in arithmetic or coordinate selectors raises `FormulaParseError`.
- Operators and precedence (highest to lowest):
  1. Exponentiation: `^` (tier 5, right-associative)
  2. Unary prefix: `+`, `-` (tier 4, right-associative)
  3. Multiplicative: `*`, `/` (tier 3, left-associative)
  4. Additive: `+`, `-` (tier 2, left-associative)
  5. Relational comparisons: `=`, `<>`, `<`, `<=`, `>`, `>=` (tier 1, left-associative; cross-type rank order `Number < Text < Boolean`, case-insensitive ASCII string comparison, error bubbling, and `Empty` coerced to `0.0` vs. `Number`, `""` vs. `Text`, equal to `Empty`, and ordered less than `Boolean` so `Empty < FALSE < TRUE` and `Empty = FALSE` is `FALSE`). Note: `==`, `!=`, and `&` are not supported and raise `FormulaParseError`.
- Slice endpoints and ordering: Range endpoints (`start..end`) accept quoted keys, `@col`/`@row`, or `prev`/`next` offsets on either side. Resolved indices must follow declared table order (`start_idx <= end_idx`); inverted ranges raise `ReferenceResolutionError`. Note: column range slices (`"c1".."c2"`) span all data columns between the endpoints in declaration order (including any intermediate `descriptor` columns, skipping gutters), whereas `col.prev`/`col.next` step across `measure` columns only.
- Offset boundaries: `col.prev` / `col.next` step across `role="measure"` columns in the origin table (skipping `descriptor` and gutter columns) from `origin.col` to resolve a target column ID, then look up that ID in the target table; `row.prev` / `row.next` step across data rows in the origin table (skipping section headers and gutter rows) from `origin.row` to resolve a target row ID, then look up that ID in the target table. Stepping outside the origin table's axis (or evaluating `col.prev`/`col.next` from a `descriptor` column) raises `ReferenceResolutionError` (`TemporalOffsetOutOfBounds`); if the offset-resolved ID is absent in the target table, it raises `ReferenceResolutionError` (`UnknownColumn` / `UnknownRow`).
- Excel A1 lowering and `$` pinning:
  - Single cell: Static `Table["r", "c"]` lowers to `$B$5`; any dynamic axis (`@col`, `@row`, `prev`, `next`) lowers to relative `B5`. Cross-sheet references always single-quote the sheet name in lowered formulas (`'Sheet1'!$B$5`, escaping `'` as `''`), whereas inspection metadata (`physical_address`, `sheet_address`) uses unquoted `Sheet1!C14`.
  - Range slice: Static key endpoints pin their respective axis on each endpoint (`Table["r", "c1"..@col]` -> `$C$5:E$5`; `Table[@row, "c1"..@col]` -> `$C5:E5`), except 1D horizontal static-column ranges on a dynamic row (`Table[@row, "c1".."c2"]` -> `C5:E5`).

---

## 2. Container Constructors (`Model`, `Sheet`, `Table`)

```python
model = f8.Model(
    name: str = "Model",
    theme: ThemeInput = "corporate_navy",
)

sheet = model.add_sheet(
    name: str,
    tab_color: str | None = None,
    show_gridlines: bool = True,
    freeze_panes: tuple[int, int] | None = None,
) -> Sheet

table = sheet.add_table(
    id: str,
    title: str,
    subtitle: str | None = None,
    currency: CurrencyCode | None = None,
    collapsible: bool = False,
    *,
    right_of: str | Table | None = None,
    below: str | Table | None = None,
    gap: int | None = None,
) -> Table
```

### Container Methods and Properties

| Container | Member                        | Return type                                     | Behavior                                                                                      |
| :-------- | :---------------------------- | :---------------------------------------------- | :-------------------------------------------------------------------------------------------- |
| `Model`   | `model.add_sheet(...)`        | `Sheet`                                         | Appends worksheet; duplicate `name` or negative `freeze_panes` `(row, col)` raises `ValueError` |
| `Model`   | `model.get_sheet(name)`       | `Sheet`                                         | Looks up worksheet by `name` (raises `KeyError` if absent)                                    |
| `Model`   | `model.get_table(id)`         | `Table`                                         | Looks up table by `id` across all worksheets (raises `KeyError` if absent)                    |
| `Model`   | `model.sheets`                | `list[Sheet]`                                   | Ordered list of worksheets                                                                    |
| `Model`   | `model.tables`                | `list[Table]`                                   | Ordered list of all tables across all worksheets                                              |
| `Model`   | `model.compile()`             | `CompiledModel`                                 | Validates, expands, evaluates, lays out, and lowers model                                     |
| `Model`   | `model.__f8_spec__()`         | `tuple[ProjectManifestDict, list[TableSpecDict]]` | Serializes model into raw manifest and table spec dictionaries                                |
| `Sheet`   | `sheet.add_table(...)`        | `Table`                                         | Appends table; duplicate `id` across model or `gap < 0` raises `ValueError`                   |
| `Sheet`   | `sheet.get_table(id)`         | `Table`                                         | Looks up table on this worksheet by `id` (raises `KeyError` if absent)                        |
| `Sheet`   | `sheet.tables`                | `list[Table]`                                   | Ordered list of tables on this worksheet                                                      |
| `Sheet`   | `sheet.__f8_spec__()`         | `SheetSpecDict`                                 | Serializes worksheet into raw specification dictionary                                        |
| `Table`   | `table.__f8_spec__()`         | `TableSpecDict`                                 | Serializes table into raw specification dictionary                                            |

- Low-level compilation: `f8._core.compile(manifest, tables) -> CompiledModel` compiles raw `ProjectManifestDict` and `Sequence[TableSpecDict]` payloads directly.
- Theme resolution (`ThemeInput = Mapping[str, object] | Path | str`):
  - `str` (default `"corporate_navy"`): Resolves strictly to a compiled-in preset in `f8-core` (does not read disk files; unknown preset name raises `SpecificationError`).
  - `Path` (e.g. `Path("theme.json")`): Reads and parses a JSON theme file into `model.theme`. Use `f8 theme init` (or `f8._core.default_theme()`) to inspect or scaffold the default `corporate_navy` schema.
  - `Mapping[str, object]`: Inline custom theme dictionary. Compiler validates hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA`, or `"none"`/`"transparent"` on fills), positive finite metrics, `{dec}`/`{sym}` format placeholders, and default decimals `<= 14`.
- Worksheet options: `tab_color` accepts hex color strings (invalid hex raises `EmissionError` on workbook emission). `freeze_panes=(row, col)` sets 0-indexed split coordinates.

---

## 3. Column and Row Specifications

### `table.add_column`

```python
table.add_column(
    id: str,
    role: AxisRole = "measure",
    header: str | None = None,
    headers: list[str] | None = None,
    group: str | None = None,
    style: SemanticStyle | None = None,
    highlight: bool | None = None,
    shade: Shade | None = None,
    format: FormatPreset | None = None,
    decimals: int | None = None,
    currency: CurrencyCode | None = None,
    expr: str | None = None,
) -> Table
```

- Column roles (`AxisRole`):
  - `"measure"` (default): Calculable data column. Participates in horizontal `row.expr` broadcasting and `col.prev`/`col.next` navigation.
  - `"descriptor"`: Metadata/label column. Isolated from vector formula broadcasting (`row.expr` and `col.expr` never expand into descriptor columns) and isolated from row-level `format`, `decimals`, and `currency`.
- Headers and group banners:
  - Pass `header="Title"` (1 tier) or `headers=["Tier 1", "Tier 2"]` (stacked tiers within the column); passing both raises `ValueError`.
  - Pass `group="Banner"` for a Tier 0 horizontal banner spanning contiguous columns with the same `group` string (rendered with `CenterAcross` and `super_header_format`; excluded from single-column width calculation). Empty or whitespace-only `group` raises `ValueError`.
  - Per-column header depth is `max(len(headers), 1) + 1` when `group` is set, or `len(headers)` (`0` if omitted) when ungrouped. Table header depth is `max(col.header_depth())` across all columns (`0` header rows when ungrouped and no column sets `header`/`headers`). When header rows exist (`depth > 0`), columns with fewer lines are bottom-aligned; if a column specifies neither `header` nor `headers`, `.xlsx` emission writes a blank header cell while `inspect_sheet` and `TableAuditor.to_markdown()` fall back to `col.id`.
- Vertical vector formula (`expr`): Evaluates top-to-bottom across all data rows in `measure` columns (ignored on `descriptor` columns, which require explicit Tier 1 `cells` entries).

### `table.add_row`

```python
table.add_row(
    id: str,
    cells: Mapping[str, CellInputValue] | None = None,
    expr: str | None = None,
    style: SemanticStyle | None = None,
    highlight: bool | None = None,
    shade: Shade | None = None,
    format: FormatPreset | None = None,
    decimals: int | None = None,
    currency: CurrencyCode | None = None,
) -> Table
```

- Horizontal vector formula (`expr`): Evaluates left-to-right across all `measure` columns. When `expr` uses `col.prev` on the first `measure` column, supply an explicit 0D base value for that column in `cells`.
- Duplicate `id` in `add_column` or `add_row` raises `ValueError`. Tables with an empty `columns` or `rows` spec list raise `SpecificationError`.

---

## 4. 0D Cell Overrides and Dynamic Updates (`Cell`, `set_cell`, `set_cells`)

### `Cell` Dataclass

```python
@dataclass(frozen=True)
class Cell:
    value: CellValueLiteral | _Unset = _UNSET
    expr: str | None = None
    style: SemanticStyle | None = None
    highlight: bool | None = None
    shade: Shade | None = None
    format: FormatPreset | None = None
    decimals: int | None = None
    currency: CurrencyCode | None = None
    indent: int | None = None
    note: str | None = None
```

`CellInputValue = CellValueLiteral | Cell | DetailedCellDict` where `CellValueLiteral = float | int | str | bool | None`.

- Payload exclusivity: `Cell` (and `DetailedCellDict`) must specify either `value` or `expr`, never both (`ValueError` at construction, `SpecificationError` at compile time). Note: `value=None` is a valid explicit empty payload (`CellValue::Empty`).
- Constraint checks (violations raise `SpecificationError`):
  - `indent`: Non-negative integer (`0..=255`); allowed only on `role="descriptor"` cells (`indent > 0` on a `measure` cell is rejected).
  - `currency`: Rejects pairing `currency` with a non-`"currency"` `format` on the same spec level (`cell`, `row`, or `col`). During grid expansion, if the cell's resolved effective `format` is not `"currency"` (or the column is `role="descriptor"`), effective `currency` resolves to `None`.
  - `decimals`: Requires `0 <= decimals <= 14`. Pairing `decimals` with `"integer"`, `"date"`, or `"text"` on the same spec level (`cell`, `row`, or `col`) raises `SpecificationError`. During grid expansion, if the cell's resolved effective `format` is `"integer"`, `"date"`, or `"text"`, effective `decimals` resolves to `None` (when effective `format` is `None`, explicit `cell`/`row`/`col` `decimals` is retained for in-memory formatting).
  - Unknown column key in `row.cells` raises `SpecificationError`.

### `table.set_cell` and `table.set_cells`

```python
type AxisSelector = str | Iterable[str] | slice

table.set_cell(
    row: str,
    col: str,
    cell: CellInputValue | _Unset = _UNSET,
    value: CellValueLiteral | _Unset = _UNSET,
    expr: str | None = None,
    style: SemanticStyle | None = None,
    highlight: bool | None = None,
    shade: Shade | None = None,
    format: FormatPreset | None = None,
    decimals: int | None = None,
    currency: CurrencyCode | None = None,
    indent: int | None = None,
    note: str | None = None,
) -> Table

table.set_cells(
    row: AxisSelector,
    cols: AxisSelector,
    cell: CellInputValue | _Unset = _UNSET,
    value: CellValueLiteral | _Unset = _UNSET,
    expr: str | None = None,
    style: SemanticStyle | None = None,
    highlight: bool | None = None,
    shade: Shade | None = None,
    format: FormatPreset | None = None,
    decimals: int | None = None,
    currency: CurrencyCode | None = None,
    indent: int | None = None,
    note: str | None = None,
) -> Table
```

- Create vs. merge semantics:
  - **Existing 0D cell**: If `(row, col)` already has an entry in `row.cells`, `set_cell` / `set_cells` merges any non-`None` keyword attributes (`style`, `highlight`, `shade`, `format`, `decimals`, `currency`, `indent`, `note`) while preserving its existing `value` or `expr` (unless a new `value`, `expr`, or `cell` payload is provided, which replaces the payload).
  - **New 0D override**: If `(row, col)` does not yet have a 0D entry in `row.cells` (including coordinates currently covered only by `row.expr` or `col.expr`), `set_cell` / `set_cells` must provide `value`, `expr`, or `cell`; omitting both `value` and `expr` raises `ValueError`.
- Axis selectors (`AxisSelector`):
  - Accepts a single ID (`str`), a non-empty `Iterable[str]`, or an inclusive `slice("start_id", "stop_id")` across declared data rows or data columns (skipping gutters and section headers).
  - Unknown row or column IDs raise `KeyError`. Open-ended slices (`slice(None, "c2")`), stepped slices, inverted slices (`start_idx > stop_idx`), or empty iterables raise `ValueError`.
  - `set_cells` pre-validates all target `(row, col)` coordinates atomically before mutating `row.cells`.

```python
# Create 0D overrides across a column list or 2D slice
table.set_cells("amort", ["standalone", "adj"], value=None)
table.set_cells(
    slice("r1", "r2"), slice("c1", "c2"), expr='Table[@row, "base"] * Table["mult", @col]'
)

# Patch metadata on an existing 0D cell (already in row.cells)
table.set_cell("r1", "c1", highlight=True, note="Patched existing 0D cell")
```

---

## 5. 3-Tier Specificity Lattice

Every `(row, col)` data coordinate resolves its payload and formatting through a 3-tier specificity lattice (`Tier 1: 0D Cell > Tier 2: 1D Row/Col > Tier 3: Default`):

| Attribute    | Tier 1 (0D Cell) | Tier 2 (1D Row vs. Column on `measure` cols)                     | Descriptor Column (`role="descriptor"`)      | Tier 3 (Fallback Default)                                         |
| :----------- | :--------------- | :--------------------------------------------------------------- | :------------------------------------------- | :---------------------------------------------------------------- |
| **Payload**  | `cell.value` or `cell.expr` | `row.expr` XOR `col.expr` (both -> `SpecificationError`) | Requires Tier 1 entry (`cells`), else `SpecificationError` | None (unassigned `measure` cell -> `SpecificationError`) |
| **`format`** | `cell.format`    | `row.format` XOR `col.format` (differing -> `SpecificationError`) | `cell.format` else `col.format` (row isolated) | `None` (unformatted)                                              |
| **`decimals`** | `cell.decimals` | `row.decimals` XOR `col.decimals` (differing -> `SpecificationError`) | `cell.decimals` else `col.decimals` (row isolated) | `theme.number_formats.defaults` for active preset (`None` if non-decimal preset) |
| **`currency`** | `cell.currency` | `row.currency` XOR `col.currency` (differing -> `SpecificationError`) | `None` (descriptor columns ignore `currency`) | `table.currency` else `"USD"` (only when effective `format == "currency"`, else `None`) |
| **`shade`**  | `cell.shade` (`"none"` clears to `None`) | `row.shade` XOR `col.shade` (differing non-`"none"` -> `SpecificationError`) | Same as `measure` (row `shade` spans descriptors) | `None`                                                            |
| **`style`**  | `"data"` resets both axes to `"data"`; `"subtotal"`/`"total"` overrides horizontal axis | Orthogonal 2D composition: `(horizontal=row.style, vertical=col.style)` | Same as `measure` (row `style` spans descriptors) | `("data", "data")`                                                |
| **`highlight`** | `cell.highlight` (`False` clears inherited highlight) | `bool(row.highlight or col.highlight)` | Same as `measure`                            | `False`                                                           |
| **`indent`** | `cell.indent`    | N/A                                                              | `cell.indent` else `0`                       | `0` (`indent > 0` on `measure` cell -> `SpecificationError`)      |
| **`note`**   | `cell.note`      | N/A                                                              | `cell.note` else `None`                      | `None`                                                            |

---

## 6. Layout Elements and Worksheet Placement

### Section Headers and Spacing Gutters

```python
table.add_section_header(title: str, indent: int | None = None, note: str | None = None) -> Table
table.add_row_gutter() -> Table
table.add_column_gutter() -> Table
```

- `add_section_header`: Consumes 1 physical row, writes `title` at `table.start_col` with `section_header_format(indent)` and optional Excel note, and allocates zero calculation cells.
- `add_row_gutter` / `add_column_gutter`: Consumes 1 blank physical row or column (`gutter_col_width` = `3.0` chars by default) with zero calculation cells. Skipped by vector formulas, `prev`/`next` offsets, and `slice(...)` selectors.

### Table Placement (`stack`, `below`, `right_of`, `gap`)

- Worksheet margins: Physical Row 0 and Column 0 (Column A) are reserved as top and left margins. The first table on a sheet starts at `B2` (`col=1, row=1`).
- Placement modes on `sheet.add_table(...)`:
  - Default (`stack`): Places table at `start_col = 1` (Column B) below the lowest `end_row` of all preceding tables on the sheet, separated by `gap=2` gutter rows.
  - `below="TargetTable"` (or `Table` instance): Aligns `start_col = target.start_col` and places table at `target.end_row + 1 + gap` (default `gap=2`).
  - `right_of="TargetTable"` (or `Table` instance): Aligns `start_row = target.start_row` and places table at `target.end_col + 1 + gap` (default `gap=1` gutter column).
  - Placement validation: `right_of` and `below` are mutually exclusive (`ValueError`), and `gap < 0` raises `ValueError`. Target table must be declared earlier on the same worksheet, and table bounding boxes cannot overlap (violations raise `SpecificationError`).
- Table banners and collapsing:
  - Non-empty `title` emits a full-width title banner row (`start_col..=end_col`). `subtitle` emits a subtitle row beneath `title` (passing an empty or whitespace-only `subtitle=""` raises `SpecificationError`; use `subtitle=None` to omit). If either `title` or `subtitle` is present, a 1-row buffer gutter is inserted before column headers; passing `title=""` (with `subtitle=None`) omits both banner and buffer gutter so column headers start at `start_row`.
  - `collapsible=True`: Groups physical rows `start_row + 1..=end_row` in Excel outline with summary symbols above (`group_symbols_above(True)`).
- Column width arbitration: Each physical worksheet column takes the maximum width across all tables spanning that column, computed as `round(max(max_chars * char_width_multiplier + col_padding, min_col_width), 2)` (default `col_padding=0.0`, `min_col_width=12.0`, where `max_chars` spans per-column header lines—excluding spanning `group`—and data cells with `indent * 2` plus `+1` alignment-space character on formatted values with trailing alignment spaces; gutter-only columns use `gutter_col_width=3.0`).

---

## 7. Formatting Presets, Styles, Shades, and Currencies

### Format Presets (`FormatPreset`)

| Preset           | Default `decimals` | Supports `decimals` | Supports `currency` | Sample inputs (`pos`, `neg`, `zero`) | In-memory formatted output (`formatted()`, `to_markdown()`) |
| :--------------- | :----------------- | :------------------ | :------------------ | :----------------------------------- | :---------------------------------------------------------- |
| `"currency"`     | `0`                | Yes (`0..=14`)      | Yes (default `USD`) | `1250`, `-1250`, `0`                 | `"$1,250"` / `"($1,250)"` / `"$0"` (Excel zero mask: `$-`)  |
| `"number"`       | `0`                | Yes (`0..=14`)      | No                  | `1250`, `-1250`, `0`                 | `"1,250"` / `"(1,250)"` / `"0"` (Excel zero mask: `-`)      |
| `"percentage"`   | `1`                | Yes (`0..=14`)      | No                  | `0.15`, `-0.15`, `0`                 | `"15.0%"` / `"(15.0%)"` / `"0.0%"` (Excel zero mask: `-`)   |
| `"multiple"`     | `1`                | Yes (`0..=14`)      | No                  | `12.5`, `-12.5`, `0`                 | `"12.5x"` / `"(12.5x)"` / `"0.0x"` (Excel neg: `-12.5x`)    |
| `"basis_points"` | `0`                | Yes (`0..=14`)      | No                  | `0.015`, `-0.015`, `0`               | `"150 bps"` / `"(150 bps)"` / `"0 bps"` (scales `10,000x` in-memory; Excel `#,##0" bps"` does not scale) |
| `"integer"`      | `None`             | No                  | No                  | `2024`, `-2024`, `0`                 | `"2024"` / `"(2024)"` / `"0"` (Excel: `2024` / `(-2024)` / `-`) |
| `"date"`         | `None`             | No                  | No                  | `45657` (Excel serial)               | `"2024-12-31"`                                              |
| `"text"`         | `None`             | No                  | No                  | `"Sample"`                           | `"Sample"`                                                  |
| `None` (default) | `None`             | Yes (in-memory)     | No                  | `1250`, `-12.5`, `0`                 | `"1,250"` / `"(12.5)"` / `"0"` (uses explicit `decimals` if set, else `0` if integer and `1` if fractional; Excel `General`) |

### Semantic Styles (`SemanticStyle`)

- `"data"` (default): Standard body typography and borders.
- `"subtotal"`: Bold typography; thin top border on rows (`subtotal_border_top`), thin left border on columns (`col_subtotal_border_left`).
- `"total"`: Bold typography; thin top and double bottom border on rows (`total_border_top`, `total_border_bottom`), thin left and double right border on columns (`col_total_border_left`, `col_total_border_right`).

### Background Shades (`Shade`)

Applies theme palette background fill to body cells without changing font color (fill resolution: `highlight_fill` if highlighted; else `total_fill.or(shade_fill)` if `total`; else `subtotal_fill.or(shade_fill)` if `subtotal`; else `shade_fill.or(data_fill)`; font color precedence: `input_text` on `measure` literal inputs `> highlight_text > total_text > subtotal_text > data_text` on `descriptor` cells `> formula_text`):

- `"none"`: No background shade; clears inherited row/column shade when set on a 0D cell override.
- `"muted"`: Maps to `theme.palette.shade_muted_fill` (`#F8FAFC` in `corporate_navy`).
- `"accent"`: Maps to `theme.palette.shade_accent_fill` (`#EFF6FF` in `corporate_navy`).
- `"alt"`: Maps to `theme.palette.shade_alt_fill` (`#F1F5F9` in `corporate_navy`).

### Currency Codes (`CurrencyCode`)

Case-insensitive 3-letter ISO codes (normalized to lowercase in spec payloads):
`USD`, `EUR`, `GBP`, `JPY`, `CHF`, `CAD`, `AUD`, `CNY`, `HKD`, `SGD`, `SEK`, `NOK`, `NZD`, `INR`, `BRL`.
