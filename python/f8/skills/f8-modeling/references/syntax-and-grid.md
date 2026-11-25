# Syntax and Grid Reference

2D coordinate grammar, container constructors, vector expansion, 3-tier attribute resolution, layout placement, and formatting presets.

---

## 1. 2D Coordinate Addressing Grammar

All formula cell references require explicit 2D bracket syntax:

`Table[RowSelector, ColSelector]` or `@table[RowSelector, ColSelector]`

### Coordinate Selectors

| Selector type              | Syntax                          | Resolution semantics                                                                          | Example                                          |
| :------------------------- | :------------------------------ | :-------------------------------------------------------------------------------------------- | :----------------------------------------------- |
| **Active table**           | `@table["row", @col]`           | Current evaluation table (avoids repeating table name intra-table)                            | `@table["revenue", @col] - @table["cogs", @col]` |
| **Active column**          | `Table["row", @col]`            | Current evaluation column                                                                     | `IS["revenue", @col] * 0.40`                     |
| **Active row**             | `Table[@row, "col"]`            | Current evaluation row                                                                        | `Assumptions[@row, "rate"] * 100`                |
| **Absolute point**         | `Table["row", "col"]`           | Fixed coordinate (lowers to `$Col$Row`)                                                       | `Assumptions["tax_rate", "val"]`                 |
| **Column lag / lead (1)**  | `Table["row", col.prev]`        | Prior (`prev`) or next (`next`) `measure` column                                              | `BS["cash", col.prev] + 10.0`                    |
| **Column lag / lead (k)**  | `Table["row", col.prev(k)]`     | `k` `measure` columns prior (`prev(k)`) or ahead (`next(k)`) (integer `1 <= k <= 2147483647`) | `Rev["sales", col.prev(2)]`                      |
| **Row lag / lead (1)**     | `Table[row.prev, "col"]`        | Prior (`prev`) or next (`next`) data row                                                      | `Schedule[row.prev, "col"] * 1.05`               |
| **Row lag / lead (k)**     | `Table[row.prev(k), "col"]`     | `k` data rows prior (`prev(k)`) or ahead (`next(k)`) (integer `1 <= k <= 2147483647`)         | `Schedule[row.next(2), "col"]`                   |
| **Column slice**           | `Table["row", "c1".."c2"]`      | Inclusive horizontal range across columns                                                     | `SUM(Rev["sales", "2023A".."2025E"])`            |
| **Dynamic column slice**   | `Table["row", "c1"..@col]`      | Inclusive horizontal range between any two column endpoints                                   | `SUM(Debt["amort", "2024E"..col.prev])`          |
| **Row slice (active col)** | `Table["r1".."r2", @col]`       | Inclusive vertical range across rows at active column                                         | `SUM(Opex["r_and_d".."g_and_a", @col])`          |
| **Dynamic row slice**      | `Table["r1"..@row, @col]`       | Inclusive vertical range between any two row endpoints                                        | `SUM(Cohort["c1"..row.prev, @col])`              |
| **Row slice (fixed col)**  | `Table["r1".."r2", "col"]`      | Inclusive vertical range across rows at fixed column                                          | `SUM(Comps["peer1".."peer5", "ev"])`             |
| **2D block slice**         | `Table["r1".."r2", "c1".."c2"]` | Inclusive 2D rectangular matrix range                                                         | `SUM(Matrix["r1".."r3", "2023A".."2025E"])`      |

### Lexical and Grammatical Rules

- Table identifiers: Bare ASCII identifiers (`[A-Za-z0-9_]+` containing at least one ASCII letter or `_`, excluding reserved tokens `TRUE`, `FALSE`, `_`, and scientific-notation-shaped tokens like `1e5`) or `@table` before `[`. Invalid table IDs raise `SpecificationError`; quoted table names (`"Table"[...]`), pure numbers (`123[...]`), or dot access (`Table.row`) in formulas raise `FormulaParseError`.
- Coordinate keys: Explicit row and column IDs inside `[...]` must be double-quoted strings (`"revenue"`, `"2024E"`). Unquoted keys (`Table[revenue, @col]`, `Table["revenue", 2024E]`) raise `FormulaParseError`.
- Numeric and string literals: Numeric literals support integers, decimals, and scientific notation (`[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?`, e.g., `42`, `0.25`, `1e-9`, `1.5E+6`) within finite IEEE-754 `f64` bounds (overflow such as `1e500` or malformed tokens such as `1.5e-` raise `FormulaParseError`; finite magnitudes outside `[1e-6, 1e15)` lower to compact uppercase `E` notation in Excel). String literals and quoted coordinate keys use double quotes with `\"` and `\\` escapes.
- Active axis tokens: Must be `@col`, `@row`, or `@table` (before `[`). Bare `@` raises `FormulaParseError`.
- Uppercase identifiers: Function names (`[A-Z][A-Z0-9_]*`, e.g., `SUM`, `XLOOKUP`) and boolean literals (`TRUE`, `FALSE`) must be uppercase (table identifiers before `[` may be mixed-case). Lowercase functions/booleans, dotted function names (`ERROR.TYPE`), `null`, or standalone identifiers raise `FormulaParseError`.
- Omitted arguments (`_`): Syntactically confined to direct function call arguments (`PMT(rate, nper, pv, _, type)`); using `_` in arithmetic or coordinate selectors raises `FormulaParseError`. Valid only on intermediate optional parameters followed by at least one explicit argument (omitting a required or trailing parameter with `_` raises `F8Error`).
- Operators and precedence (highest to lowest):
  1. Exponentiation: `^` (right-associative, binds tighter than unary `-` so `-2 ^ 2` evaluates to `-4` and lowers to `=-(2^2)`; numeric operands required; `0 ^ 0` and negative base with non-integer exponent return `#NUM!`, `0 ^ -n` returns `#DIV/0!`).
  2. Unary negation: `-` (right-associative; numeric operand required; unary `+` raises `FormulaParseError`).
  3. Multiplicative: `*`, `/` (left-associative; numeric operands required).
  4. Additive: `+`, `-` (left-associative; numeric operands required).
  5. Relational comparisons: `=`, `<>`, `<`, `<=`, `>`, `>=` (non-associative unless parenthesized as `(a = b) = c`; unparenthesized chained comparisons raise `FormulaParseError`). Operands must be non-empty values of identical type (`Number`, `Text` via ASCII case-insensitive comparison, or `Boolean`; cross-type or empty operands raise `F8Error`, and runtime Excel errors bubble). `==`, `!=`, and `&` raise `FormulaParseError`.
- Slice endpoints, ordering, and contiguity: Range endpoints (`start..end`) accept quoted keys, `@col`/`@row`, or `prev`/`next` offsets on either side. Resolved indices must follow declared table order (`start <= end`; inverted ranges raise `ReferenceResolutionError`). Row range slices must span contiguous data rows with no intervening row gutters or section headers, and horizontal column range slices must span contiguous `measure` columns with no `descriptor` or gutter columns (violations raise `ReferenceResolutionError`). 1D vertical slices on a single `descriptor` column (`Table["r1".."r2", "line_item"]`) are valid in `XLOOKUP`, `XMATCH`, and `INDEX`, while numeric and financial aggregations reject `descriptor` columns (`F8Error`).
- Offset boundaries: `col.prev` / `col.next` anchor the origin column ID in the target table and step across `role="measure"` columns (skipping `descriptor` and gutter columns); `row.prev` / `row.next` anchor the origin row ID in the target table and step across data rows (skipping section headers and gutter rows). Missing origin IDs, `descriptor`-anchored column offsets, or steps past axis bounds raise `ReferenceResolutionError`.
- Excel A1 lowering and `$` pinning:
  - Single cell: Static `Table["r", "c"]` lowers to `$B$5`; any dynamic axis (`@col`, `@row`, `prev`, `next`) lowers to relative `B5`. Cross-sheet references in lowered formulas and sheet-qualified inspection addresses single-quote the sheet name (`'Sheet1'!$B$5`, `'Sheet1'!C14`, escaping `'` as `''`).
  - Range slice: Static key endpoints pin their respective axis on each endpoint (`Table["r", "c1"..@col]` -> `$C$5:E$5`; `Table[@row, "c1"..@col]` -> `$C5:E5`), except 1D horizontal static-column ranges on a dynamic row (`Table[@row, "c1".."c2"]` -> `C5:E5`).

---

## 2. Container Constructors (`Model`, `Sheet`, `Band`, `Lane`, `Table`)

```python
model = f8.Model(
    name: str = "Model",
    theme: ThemeInput = "corporate_navy",
)

sheet = model.add_sheet(
    name: str,
    tab_color: str | None = None,
    show_gridlines: bool = False,
    freeze_panes: tuple[int, int] | None = None,
    *,
    layout: SheetLayoutMode = "auto",
    col_width: float | None = None,
) -> Sheet

table = sheet.add_table(
    id: str,
    title: str,
    subtitle: str | Sequence[str] | None = None,
    currency: CurrencyCode | None = None,
    collapsible: bool = False,
) -> Table

type TableRef = str | Table
type LaneItem = Lane | TableRef
type BandItem = Band | TableRef

lane = f8.Lane(tables: Sequence[TableRef], *, table_gap: int | None = None)
band = f8.Band(lanes: Sequence[LaneItem], *, gap: int | None = None, lane_gap: int | None = None)
sheet.set_bands(bands: Sequence[BandItem]) -> Sheet
```

### Container Methods and Properties

| Container | Member                  | Return type                                       | Behavior                                                                                                                                                                                                                |
| :-------- | :---------------------- | :------------------------------------------------ | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Model`   | `model.add_sheet(...)`  | `Sheet`                                           | Appends worksheet; duplicate `name`, negative `freeze_panes`, invalid `layout`, `col_width` with `"auto"`, or `col_width` outside `1.0..=255.0` raises `ValueError` (`bool`/non-numeric `col_width` raises `TypeError`) |
| `Model`   | `model.get_sheet(name)` | `Sheet`                                           | Looks up worksheet by `name` (raises `KeyError` if absent)                                                                                                                                                              |
| `Model`   | `model.get_table(id)`   | `Table`                                           | Looks up table by `id` across all worksheets (raises `KeyError` if absent)                                                                                                                                              |
| `Model`   | `model.sheets`          | `list[Sheet]`                                     | Ordered list of worksheets                                                                                                                                                                                              |
| `Model`   | `model.tables`          | `list[Table]`                                     | Ordered list of all tables across all worksheets                                                                                                                                                                        |
| `Model`   | `model.compile()`       | `CompiledModel`                                   | Validates, expands, evaluates, lays out, and lowers model                                                                                                                                                               |
| `Model`   | `model.__f8_spec__()`   | `tuple[ProjectManifestDict, list[TableSpecDict]]` | Serializes model into raw manifest and table spec dictionaries                                                                                                                                                          |
| `Sheet`   | `sheet.add_table(...)`  | `Table`                                           | Appends table; duplicate `id` across model raises `ValueError`                                                                                                                                                          |
| `Sheet`   | `sheet.set_bands(...)`  | `Sheet`                                           | Replaces worksheet 2D band/lane layout (exact partition of tables on this sheet; accepts `Band` or bare `TableRef` single-table band shorthand; returns `self`)                                                         |
| `Sheet`   | `sheet.bands`           | `list[Band]`                                      | Normalized band layout (`Lane.tables: tuple[str, ...]`); defaults to one 1-table band per table in `add_table` order                                                                                                    |
| `Sheet`   | `sheet.get_table(id)`   | `Table`                                           | Looks up table on this worksheet by `id` (raises `KeyError` if absent)                                                                                                                                                  |
| `Sheet`   | `sheet.tables`          | `list[Table]`                                     | Tables on this worksheet in `add_table` declaration order (compiled outputs follow canonical `bands -> lanes -> tables` order)                                                                                          |
| `Sheet`   | `sheet.__f8_spec__()`   | `SheetSpecDict`                                   | Serializes worksheet into raw specification dictionary (re-validates exact partition if `add_table` followed `set_bands`)                                                                                               |
| `Table`   | `table.__f8_spec__()`   | `TableSpecDict`                                   | Serializes table into raw specification dictionary                                                                                                                                                                      |

- Low-level compilation: `f8.compile(manifest, tables) -> CompiledModel` compiles raw `ProjectManifestDict` and `Sequence[TableSpecDict]` payloads directly.
- Theme resolution (`ThemeInput = Mapping[str, object] | Path | str`):
  - `str` (default `"corporate_navy"`): Resolves to a compiled-in preset (does not read disk files; unknown preset raises `SpecificationError`).
  - `Path` (e.g. `Path("theme.json")`): Reads and parses a JSON theme file into `model.theme`. Use `f8 theme init` or `f8.default_theme()` to inspect or scaffold the default `corporate_navy` schema.
  - `Mapping[str, object]`: Inline custom theme dictionary. Compiler validates hex colors (`#RGB`, `#RRGGBB`, `#RRGGBBAA`, or `"none"`/`"transparent"` on fills), positive finite metrics (`col_padding >= 0.0`, `min_col_width <= max_col_width`), `{dec}`/`{sym}` format placeholders, and default decimals `<= 14`.
- Worksheet options: `name` enforces Excel worksheet rules (`1..=31` UTF-16 code units, no `: \ / ? * [ ]`, no leading/trailing `'`, not `"History"`, case-insensitive uniqueness; violations raise `SpecificationError`). `tab_color` accepts `#RGB`, `#RRGGBB`, or `#RRGGBBAA` hex strings (invalid hex raises `SpecificationError`). `freeze_panes=(row, col)` sets 0-indexed split coordinates within Excel grid bounds (`row < 1_048_576`, `col < 16_384`). `layout: SheetLayoutMode` (`"auto"` default | `"grid"`, re-exported on `f8`) selects per-column auto-fit vs. uniform grid layout; `col_width` (finite `1.0..=255.0`, valid only when `layout="grid"`, defaulting to `theme.metrics.min_col_width`) sets the base grid column width.

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
  - `"descriptor"`: Label/metadata column. Isolated from vector formula broadcasting (`row.expr` and `col.expr` never expand into descriptor columns) and row-level `format`, `decimals`, and `currency`; rejected in numeric and financial aggregations (`F8Error`; valid in `XLOOKUP`, `XMATCH`, and `INDEX`).
- Headers and group banners:
  - Pass `header="Title"` (1 tier) or `headers=["Tier 1", "Tier 2"]` (stacked tiers within the column); passing both raises `ValueError`.
  - Pass `group="Banner"` for a Tier 0 horizontal banner spanning contiguous columns with the same `group` string (rendered with `CenterAcross` using the theme's super-header styling; excluded from single-column width calculation). Empty or whitespace-only `group` raises `ValueError`.
  - Per-column header depth is `max(len(headers), 1) + 1` when `group` is set, or `len(headers)` (`0` if omitted) when ungrouped. Table header depth is the maximum per-column depth across all columns (`0` header rows when ungrouped and no column sets `header`/`headers`). When header rows exist (`depth > 0`), columns with fewer lines are bottom-aligned; if a column sets neither `header` nor `headers`, `.xlsx` emission writes a blank header cell while `inspect_sheet` and `TableAuditor.to_markdown()` fall back to `col.id`.
- Vertical vector formula (`expr`): Evaluates top-to-bottom across data rows in `measure` columns (ignored on `descriptor` columns, which require explicit Tier 1 `cells` entries).

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

- Horizontal vector formula (`expr`): Evaluates left-to-right across `measure` columns. When `expr` uses `col.prev` on the first `measure` column, supply an explicit 0D override for that column in `cells`.
- Duplicate `id` in `add_column` or `add_row` raises `ValueError`. Tables with empty `columns` or `rows` raise `SpecificationError`.

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

- Payload exclusivity: `Cell` (and `DetailedCellDict`) must specify either `value` or `expr`, never both (`ValueError` at construction, `SpecificationError` at compile time). `value=None` (or literal `None`) is a valid explicit empty cell; formula cells cannot evaluate to empty at the cell root, and scalar args, `1x1` cell refs, evaluated branch arms, and selected lookup/index elements reject empty (`F8Error`).
- Constraint checks (violations raise `SpecificationError`):
  - `indent`: Non-negative integer (`0..=255`); allowed only on `role="descriptor"` cells (`indent > 0` on a `measure` cell is rejected).
  - `currency`: Rejects pairing `currency` with a non-`"currency"` `format` on the same spec level (`cell`, `row`, or `col`). If a cell's resolved effective `format` is not `"currency"` (or the column is `role="descriptor"`), effective `currency` resolves to `None`.
  - `decimals`: Requires `0 <= decimals <= 14`. Pairing `decimals` with `"integer"`, `"date"`, or `"text"` on the same spec level (`cell`, `row`, or `col`) raises `SpecificationError`. If effective `format` is `"integer"`, `"date"`, or `"text"`, effective `decimals` resolves to `None` (when effective `format` is `None`, explicit `decimals` is retained for in-memory formatting).
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
  - **Existing 0D cell**: If `(row, col)` already has an entry in `row.cells`, `set_cell` / `set_cells` merges non-`None` metadata from `cell` (`Cell` / `DetailedCellDict`) and keyword attributes (`style`, `highlight`, `shade`, `format`, `decimals`, `currency`, `indent`, `note`) while preserving its existing `value` or `expr` (providing a new `value`, `expr`, or scalar/`Cell` payload replaces only the payload).
  - **New 0D override**: If `(row, col)` has no 0D entry in `row.cells` (including cells currently covered only by `row.expr` or `col.expr`), `set_cell` / `set_cells` must provide `value`, `expr`, or `cell`; omitting payload raises `ValueError`.
- Axis selectors (`AxisSelector`):
  - Accepts a single ID (`str`), a non-empty `Iterable[str]`, or an inclusive `slice("start_id", "stop_id")` across declared data rows or data columns (skipping gutters and section headers).
  - Unknown IDs raise `KeyError`. Open-ended slices (`slice(None, "c2")`), stepped slices, inverted slices, or empty iterables raise `ValueError`.
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

Every `(row, col)` data coordinate resolves payload and formatting through a 3-tier specificity lattice (`Tier 1: 0D Cell > Tier 2: 1D Row/Col > Tier 3: Default`):

| Attribute       | Tier 1 (0D Cell)                                                                                 | Tier 2 (1D Row vs. Column on `measure` cols)                                 | Descriptor Column (`role="descriptor"`)                    | Tier 3 (Fallback Default)                                                               |
| :-------------- | :----------------------------------------------------------------------------------------------- | :--------------------------------------------------------------------------- | :--------------------------------------------------------- | :-------------------------------------------------------------------------------------- |
| **Payload**     | `cell.value` or `cell.expr`                                                                      | `row.expr` XOR `col.expr` (both -> `SpecificationError`)                     | Requires Tier 1 entry (`cells`), else `SpecificationError` | None (unassigned `measure` cell -> `SpecificationError`)                                |
| **`format`**    | `cell.format`                                                                                    | `row.format` XOR `col.format` (differing -> `SpecificationError`)            | `cell.format` else `col.format` (row isolated)             | `None` (unformatted)                                                                    |
| **`decimals`**  | `cell.decimals`                                                                                  | `row.decimals` XOR `col.decimals` (differing -> `SpecificationError`)        | `cell.decimals` else `col.decimals` (row isolated)         | `theme.number_formats.defaults` at format/emission (`None` in inspection)               |
| **`currency`**  | `cell.currency`                                                                                  | `row.currency` XOR `col.currency` (differing -> `SpecificationError`)        | `None` (descriptor columns ignore `currency`)              | `table.currency` else `"usd"` (only when effective `format == "currency"`, else `None`) |
| **`shade`**     | `cell.shade` (`"none"` clears to `None`)                                                         | `row.shade` XOR `col.shade` (differing non-`"none"` -> `SpecificationError`) | Same as `measure` (row `shade` spans descriptors)          | `None`                                                                                  |
| **`style`**     | `"data"` resets both axes to `"data"`; `"bold"`/`"subtotal"`/`"total"` overrides horizontal axis | Orthogonal 2D composition: `(horizontal=row.style, vertical=col.style)`      | Same as `measure` (row `style` spans descriptors)          | `("data", "data")`                                                                      |
| **`highlight`** | `cell.highlight` (`False` clears inherited highlight)                                            | `bool(row.highlight or col.highlight)`                                       | Same as `measure`                                          | `False`                                                                                 |
| **`indent`**    | `cell.indent`                                                                                    | N/A                                                                          | `cell.indent` else `0`                                     | `0` (`indent > 0` on `measure` cell -> `SpecificationError`)                            |
| **`note`**      | `cell.note`                                                                                      | N/A                                                                          | `cell.note` else `None`                                    | `None`                                                                                  |

---

## 6. Layout Elements and Worksheet Placement

### Section Headers and Spacing Gutters

```python
table.add_section_header(title: str, indent: int | None = None, note: str | None = None) -> Table
table.add_row_gutter() -> Table
table.add_column_gutter() -> Table
```

- `add_section_header`: Consumes 1 physical row, writes `title` at the table's starting column with the theme's section-header style, optional indentation, and optional Excel note, and allocates zero calculation cells.
- `add_row_gutter` / `add_column_gutter`: Consumes 1 blank physical row or column (`gutter_col_width=2.0` chars by default) with zero calculation cells; row formatting continues across column gutters while row gutters remain blank. Skipped by vector formulas, `prev`/`next` offsets, and `Table.set_cells(...)` `slice(...)` selectors; formula `start..end` slices cannot span gutters or section headers (`ReferenceResolutionError`).

### Worksheet Layout (`Band`, `Lane`, `set_bands`)

- Worksheet margins: Physical Row 0 (Row 1) and Column 0 (Column A) are reserved as blank margins. Band `0` starts at `B2` (`col=1, row=1`).
- Default vertical stacking: Without `sheet.set_bands(...)`, each table is placed in its own 1-lane, 1-table band at Column `B` separated by `2` blank rows in `add_table` order.
- Explicit 2D band/lane layout via `sheet.set_bands(bands: Sequence[Band | TableRef]) -> Sheet`:
  - `Band(lanes: Sequence[Lane | TableRef], *, gap: int | None = None, lane_gap: int | None = None)`: Horizontal row of 1 or more vertical `Lane`s (or bare `TableRef` 1-table lane shorthands, normalized to `tuple[Lane, ...]`). Band `b > 0` starts `gap` (default `2`) blank rows below the previous band's lowest row; Lane `l > 0` starts `lane_gap` (default `1`) blank columns right of the previous lane's rightmost column.
  - `Lane(tables: Sequence[TableRef], *, table_gap: int | None = None)`: Vertical stack of 1 or more tables (`Table` or `str` ID, normalized to `tuple[str, ...]`). Table `0` starts at the band's top row; Table `t > 0` starts `table_gap` (default `2`) blank rows below the previous table in the lane.
  - Bare `TableRef` shorthand: Pass a `Table` or `str` directly inside `set_bands([...])` for a 1-lane, 1-table band, or directly inside `Band([...])` for a 1-table lane (`Band([left, right])`, `Band([Lane([tl, bl]), tr])`, or `Band([card], gap=1)`). Multi-table vertical lanes require explicit `Lane([...])` objects (nested sequences are rejected).
  - Non-overlap & ordering: Tables cannot overlap by construction (each lane's width is the maximum end column across its tables, shifting subsequent lanes right automatically). Compiled outputs (`summary()`, `errors()`, `inspect_sheet()`, `to_markdown()`) follow canonical `bands -> lanes -> tables` order.
  - Validation: Rejects empty `Band`/`Lane`, nested sequences or invalid lane items, bare `str`/`Table` passed as `tables`/`lanes`/`bands`, negative/`bool`/non-int gaps, unused gaps (`gap` on Band `0`, `lane_gap` on a 1-lane band, `table_gap` on a 1-table lane), foreign-sheet or duplicate tables, and missing tables (exact partition enforced on `set_bands` and re-validated on `__f8_spec__` if `add_table` is called later). `set_bands([])` is valid on an empty sheet; re-calling `set_bands` replaces the layout idempotently.

```python
sheet.set_bands(
    [
        f8.Band([f8.Lane([tl, bl]), tr], lane_gap=1),  # bare TableRef shorthand for 1-table lane
        timeline,  # bare TableRef shorthand for a 1-table band
    ]
)
```

### Table Banners and Collapsing

- Title banners: Non-empty `title` emits a title banner row spanning from the table's starting column to the sheet's rightmost column for tables in the last lane of any band (including single-lane bands), or to that lane's rightmost column for tables in non-last lanes. Subtitles, column headers, section headers, body borders, and column widths stay bounded to the table's own columns.
- Subtitles & buffer gutter: `subtitle` accepts a single string or a sequence of strings (normalized to `table.subtitles: list[str]`), emitting one row per entry beneath `title` (empty or whitespace-only entries raise `SpecificationError`; pass `None` or `[]` to omit). When either `title` or `subtitle` is present, a 1-row blank buffer gutter is inserted before column headers; passing `title=""` with no `subtitle` omits both banner and buffer gutter.
- Collapsing (`collapsible=True`): Groups rows below the table's first physical row in an Excel outline with summary symbols above.

### Column Width Arbitration (`layout="auto"`, default)

- Per-column width: Each physical column takes the maximum width across all tables spanning it: `round(clamp(max_chars * char_width_multiplier + col_padding, min_col_width, max_col_width), 2)` (defaults: `char_width_multiplier=1.0`, `col_padding=0.0`, `min_col_width=12.0`, `max_col_width=75.0`). Column A and gutter/gap-only columns use `gutter_col_width` (`2.0` default).
- Character demand (`max_chars`): Longest line across per-column headers (excluding spanning `group` banners) and `\n`-delimited data cell segments, adding `indent * 2` plus `+1` trailing alignment character on non-empty `measure` headers, non-parenthesized numbers under `date`/numeric presets or unformatted `measure` cells, and non-empty text cells under trailing-`_)` formats (leaving `descriptor` headers, unformatted `descriptor` cells, and `text`-formatted numbers unpadded).
- Text wrapping: Cells whose width exceeds `max_col_width` or that contain `\n` enable OpenXML `wrapText="1"` (all data cells are vertically centered); `.xlsx` row heights remain automatic.

### Uniform Grid Layout (`layout="grid"`)

- Uniform column width `W`: Every physical column covered by any data column span shares width `W = round(max(base_w, max_measure_cell_width, max_non_text_descriptor_cell_width), 2)`, where `base_w = round(col_width or theme.metrics.min_col_width, 2)` in `1.0..=255.0`. `grid` ignores `max_col_width`; if `base_w` or computed `W` exceeds `255.0`, compilation fails with `SpecificationError` (it is not clamped). Column A and gutter/gap-only columns keep `gutter_col_width` (`2.0` default).
- Descriptor overflow spans: `descriptor` columns automatically span `ceil(req_w / W)` (`>= 1`) contiguous physical columns (from non-empty descriptor headers/text with `+1` breathing space and `indent * 2`) with zero merged cells. Anchor cells stay at the span's first column (preserving vertical `XLOOKUP`/`XMATCH`/`INDEX` slices and contiguous `measure` ranges), while styled blank runway cells carry continuous fills/borders and place vertical right borders on the span's final column.
- Leading descriptor alignment: Lanes finalize left-to-right. Within each `(lane_index, start_col)` group, tables with at least one `descriptor` before their first `measure` column align their leading descriptor block (through the last leading `descriptor`, excluding trailing `add_column_gutter()` spacers after it) by extending their last leading `descriptor` span. Descriptor-only tables skip alignment; span growth in an earlier lane shifts later lanes right automatically.
- `grid` constraints (violations raise `SpecificationError`): Text wrapping is disabled, `\n`/`\r` line breaks are forbidden in data cells, column `headers`, and `group` banners, and every non-empty `measure` column header line must fit within static `base_w`.

---

## 7. Formatting Presets, Styles, Shades, and Currencies

### Format Presets (`FormatPreset`)

Default `corporate_navy` Excel masks reserve a uniform trailing alignment space (`_)`) on right-aligned `measure` elements: measure column headers use `theme.number_formats.text` (`@_)`), numeric presets append a 4th `;@_)` text section, `date` uses `yyyy-mm-dd_)`, and unformatted (`None`) measure cells use `theme.number_formats.general` (`General_);(General);0_);@_)`). Setting any template to `""` or `"General"` in a custom theme emits native Excel `numFmtId="0"`; left-aligned `descriptor` headers and unformatted `descriptor` cells use native `General`.

| Preset           | Default `decimals` | Supports `decimals` | Supports `currency` | Sample inputs (`pos`, `neg`, `zero`) | In-memory formatted output (`formatted()`, `to_markdown()`)                                                                                                                                             |
| :--------------- | :----------------- | :------------------ | :------------------ | :----------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `"currency"`     | `0`                | Yes (`0..=14`)      | Yes (default `USD`) | `1250`, `-1250`, `0`                 | `"$1,250"` / `"($1,250)"` / `"$0"` (Excel zero mask: `-`)                                                                                                                                               |
| `"number"`       | `0`                | Yes (`0..=14`)      | No                  | `1250`, `-1250`, `0`                 | `"1,250"` / `"(1,250)"` / `"0"` (Excel zero mask: `-`)                                                                                                                                                  |
| `"percentage"`   | `1`                | Yes (`0..=14`)      | No                  | `0.15`, `-0.15`, `0`                 | `"15.0%"` / `"(15.0%)"` / `"0.0%"` (Excel zero mask: `-`)                                                                                                                                               |
| `"multiple"`     | `1`                | Yes (`0..=14`)      | No                  | `12.5`, `-12.5`, `0`                 | `"12.5x"` / `"(12.5x)"` / `"0.0x"` (Excel: `12.5x` / `(12.5x)` / `-`)                                                                                                                                   |
| `"basis_points"` | `0`                | Yes (`0..=14`)      | No                  | `0.015`, `-0.015`, `0`               | `"150 bps"` / `"(150 bps)"` / `"0 bps"` (scales `10,000x` in-memory; Excel `#,##0" bps"` does not scale; Excel zero mask: `-`)                                                                          |
| `"integer"`      | `None`             | No                  | No                  | `2024`, `-2024`, `0`                 | `"2024"` / `"(2024)"` / `"0"` (Excel: `2024` / `(-2024)` / `-`)                                                                                                                                         |
| `"date"`         | `None`             | No                  | No                  | `45657` (Excel serial)               | `"2024-12-31"` (Excel: `yyyy-mm-dd_)`)                                                                                                                                                                  |
| `"text"`         | `None`             | No                  | No                  | `"Sample"`                           | `"Sample"` (Excel: `@_)`)                                                                                                                                                                               |
| `None` (default) | `None`             | Yes (in-memory)     | No                  | `1250`, `-12.5`, `0`                 | `"1,250"` / `"(12.5)"` / `"0"` (uses explicit `decimals` if set, else rounds to `1` decimal place and trims trailing `.0`; Excel measure: `theme.number_formats.general`, descriptor: native `General`) |

### Semantic Styles (`SemanticStyle`)

Typography, border, and fill roles for rows, columns, and 0D cell overrides (borders and fills come from the active theme; defaults shown for `corporate_navy`):

- `"data"` (default): Regular typography, `data_fill`, and no summary borders.
- `"bold"`: Bold typography only; no borders, using standard fill/text color precedence (`highlight` > `shade` > `data`, preserving `input_text` on `measure` literals).
- `"subtotal"`: Bold typography; `subtotal_fill`/`subtotal_text`, thin top border on rows (`subtotal_border_top`), dashed left border on columns (`col_subtotal_border_left`).
- `"total"`: Bold typography; `total_fill`/`total_text`, thin top and double bottom border on rows (`total_border_top`, `total_border_bottom`), thin left border on columns (`col_total_border_left`; `col_total_border_right` defaults to `"none"`).

### Background Shades (`Shade`)

Applies theme palette background fill to body cells without changing font color (fill precedence: `highlight_fill` if highlighted; else `total_fill` or shade if `total`; else `subtotal_fill` or shade if `subtotal`; else shade or `data_fill`; font color precedence: `input_text` on `measure` literal inputs > `highlight_text` > `total_text` > `subtotal_text` > `data_text` on `descriptor` cells > `formula_text`):

- `"none"`: No background shade; clears inherited row/column shade when set on a 0D cell override.
- `"muted"`: Maps to `theme.palette.shade_muted_fill`.
- `"alt"`: Maps to `theme.palette.shade_alt_fill`.

### Currency Codes (`CurrencyCode`)

3-letter ISO codes normalized to lowercase by builder APIs (raw `f8.compile(...)` requires lowercase; `CurrencyCode` types accept lowercase or uppercase):
`USD`, `EUR`, `GBP`, `JPY`, `CHF`, `CAD`, `AUD`, `CNY`, `HKD`, `SGD`, `SEK`, `NOK`, `NZD`, `INR`, `BRL`.
