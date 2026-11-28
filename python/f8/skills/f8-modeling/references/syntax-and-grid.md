# Syntax and Grid Reference

2D coordinate grammar, core containers (`Model`, `Sheet`, `Table`, `compile`), column/row builders, 0D overrides, and the 3-tier specificity lattice.

## 1. 2D Coordinate Addressing Grammar

Every formula cell reference uses explicit 2D bracket syntax:

`Table[RowSelector, ColSelector]` or `@table[RowSelector, ColSelector]`

### Coordinate Selectors

| Selector            | Syntax                                      | Semantics                          | Example                                      |
| :------------------ | :------------------------------------------ | :--------------------------------- | :------------------------------------------- |
| Active table        | `@table["row", @col]`                       | Current table (intra-table)        | `@table["rev", @col] - @table["cogs", @col]` |
| Active column       | `Table["row", @col]`                        | Current evaluation column          | `IS["rev", @col] * 0.4`                      |
| Active row          | `Table[@row, "col"]`                        | Current evaluation row             | `Assump[@row, "rate"] * 100`                 |
| Absolute point      | `Table["row", "col"]`                       | Fixed cell (`$Col$Row`)            | `Assump["tax_rate", "val"]`                  |
| Column lag/lead (1) | `Table["row", col.prev]` / `col.next`       | Prior/next `measure` column        | `BS["cash", col.prev] + 10`                  |
| Column lag/lead (k) | `Table["row", col.prev(k)]` / `col.next(k)` | `k` `measure` cols (`1..=2^31-1`)  | `Rev["sales", col.prev(2)]`                  |
| Row lag/lead (1)    | `Table[row.prev, "col"]` / `row.next`       | Prior/next data row                | `Sched[row.prev, "col"] * 1.05`              |
| Row lag/lead (k)    | `Table[row.prev(k), "col"]` / `row.next(k)` | `k` data rows (`1..=2^31-1`)       | `Sched[row.next(2), "col"]`                  |
| Column slice        | `Table["row", "c1".."c2"]`                  | Inclusive horizontal range         | `SUM(Rev["sales", "2023A".."2025E"])`        |
| Dynamic col slice   | `Table["row", "c1"..@col]`                  | Any two column endpoints           | `SUM(Debt["amort", "2024E"..col.prev])`      |
| Row slice (active)  | `Table["r1".."r2", @col]`                   | Inclusive vertical range at `@col` | `SUM(Opex["rd".."ga", @col])`                |
| Dynamic row slice   | `Table["r1"..@row, @col]`                   | Any two row endpoints              | `SUM(Cohort["c1"..row.prev, @col])`          |
| Row slice (fixed)   | `Table["r1".."r2", "col"]`                  | Vertical range at fixed column     | `SUM(Comps["p1".."p5", "ev"])`               |
| 2D block slice      | `Table["r1".."r2", "c1".."c2"]`             | Inclusive 2D rectangular range     | `SUM(M["r1".."r3", "2023A".."2025E"])`       |

### Lexical, Operator, Slice, and Lowering Rules

- Table IDs & keys: Bare ASCII `[A-Za-z0-9_]+` with at least one letter or `_` (excluding `TRUE`, `FALSE`, `_`, and scientific tokens like `1e5`) or `@table` before `[`. Row/column keys inside `[...]` must be double-quoted (`\"` and `\\` escapes). Invalid table IDs raise `SpecificationError`; quoted table names, pure numeric table IDs, unquoted keys, or dot access (`Table.row`) raise `FormulaParseError`.
- Literals & tokens: Integers, decimals, and scientific notation (`[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?`) within finite `f64` bounds (overflow or malformed tokens raise `FormulaParseError`). Function names (`[A-Z][A-Z0-9_]*`) and booleans (`TRUE`, `FALSE`) must be uppercase; active tokens are `@col`, `@row`, `@table`. Lowercase functions/booleans, dotted function names, `null`, bare `@`, or standalone identifiers raise `FormulaParseError`.
- Omitted arguments (`_`): Syntactically confined to direct function call arguments (`PMT(r, n, pv, _, 1)`); `_` outside call arguments raises `FormulaParseError`. Only intermediate optional arguments are valid (see `references/functions.md`).
- Operators & precedence (high to low):
  1. `^`: Right-associative; binds tighter than unary `-` so `-2 ^ 2` evaluates to `-4` and lowers to `=-(2^2)`.
  2. Unary `-`: Right-associative (unary `+` raises `FormulaParseError`).
  3. `*`, `/`: Left-associative.
  4. `+`, `-`: Left-associative.
  5. `=`, `<>`, `<`, `<=`, `>`, `>=`: Non-associative unless parenthesized (`(a = b) = c`); unparenthesized chained comparisons, `==`, `!=`, and `&` raise `FormulaParseError`. Type/empty rules live in `references/functions.md`.
- Slices & contiguity: Range endpoints (`start..end`) accept quoted keys, `@col`/`@row`, or `prev`/`next` offsets and must satisfy `start <= end` in declared order (inverted ranges raise `ReferenceResolutionError`). Row slices must span contiguous data rows (no intervening gutters or section headers); horizontal column slices must span contiguous `measure` columns (no `descriptor` or gutter columns), else `ReferenceResolutionError`. Vertical 1D slices on a single `descriptor` column (`Table["r1".."r2", "item"]`) are valid in `XLOOKUP`/`XMATCH`/`INDEX` and `AND`/`OR`, while numeric/financial aggregations reject `descriptor` columns (`F8Error`).
- Offsets: `col.prev`/`col.next` anchor the origin column ID in the target table and step across `measure` columns (skipping `descriptor` and gutter columns); `row.prev`/`row.next` anchor the origin row ID and step across data rows (skipping section headers and row gutters). Missing origin IDs, `descriptor`-anchored column offsets, or steps past bounds raise `ReferenceResolutionError`.
- Excel A1 lowering & `$` pinning:
  - Single cell: Static `Table["r", "c"]` lowers to `$B$5`; any dynamic axis (`@col`, `@row`, `prev`, `next`) lowers to relative `B5`. Cross-sheet references single-quote the sheet name (`'Sheet1'!$B$5`, escaping `'` as `''`).
  - Range slice: Static key endpoints pin their axis per endpoint (`Table["r", "c1"..@col]` -> `$C$5:E$5`; `Table[@row, "c1"..@col]` -> `$C5:E5`), except 1D horizontal static-column ranges on a dynamic row (`Table[@row, "c1".."c2"]` -> `C5:E5`).

## 2. Core Containers (`Model`, `Sheet`, `Table`, `compile`)

```python
model = f8.Model(
    name: str = "Model",
    theme: ThemeInput = "corporate_navy",
    *,
    print: PrintSetup | SheetPrintDict | None = None,
)
sheet = model.add_sheet(
    name: str,
    tab_color: str | None = None,
    show_gridlines: bool = False,
    freeze_panes: tuple[int, int] | None = None,
    *,
    layout: SheetLayoutMode = "auto",
    col_width: float | None = None,
    print: PrintSetup | SheetPrintOverrideDict | Literal["none"] | None = None,
) -> Sheet
table = sheet.add_table(
    id: str,
    title: str,
    subtitle: str | Sequence[str] | None = None,
    currency: CurrencyCode | None = None,
    collapsible: bool = False,
) -> Table
compiled = f8.compile(manifest: ManifestInput, tables: Sequence[TableInput]) -> CompiledModel
```

- `Model`:
  - `add_sheet(...) -> Sheet`: Appends worksheet (duplicate `name`, negative `freeze_panes`, invalid `layout`, `col_width` with `"auto"`, or `col_width` outside `1.0..=255.0` raises `ValueError`; `bool`/non-numeric `col_width` raises `TypeError`).
  - `get_sheet(name) -> Sheet` / `get_table(id) -> Table`: Lookup across model (`KeyError` if absent).
  - `sheets: list[Sheet]` / `tables: list[Table]`: Ordered containers across model.
  - `compile() -> CompiledModel`: Runs all compiler passes and returns inspection/emission facade.
  - `__f8_spec__() -> tuple[ProjectManifestDict, list[TableSpecDict]]`: Raw spec serialization.
- `Sheet`:
  - `add_table(...) -> Table`: Appends table (duplicate `id` across model raises `ValueError`).
  - `set_bands(bands) -> Sheet` / `bands: list[Band]`: Explicit 2D band/lane placement (defaults to 1-table vertical bands in `add_table` order; see `references/layout-and-styling.md`).
  - `get_table(id) -> Table`: Lookup on sheet (`KeyError` if absent).
  - `tables: list[Table]`: Tables in `add_table` order (compiled outputs follow `bands -> lanes -> tables` order).
  - `__f8_spec__() -> SheetSpecDict`: Serializes sheet (re-validates band partition if `add_table` followed `set_bands`).
- `Table`:
  - `__f8_spec__() -> TableSpecDict`: Serializes table; `table.subtitles: list[str]` stores normalized subtitle lines.
- Sheet options: `name` follows Excel sheet rules (`1..=31` UTF-16 code units, no `: \ / ? * [ ]`, no leading/trailing `'`, not `"History"`, case-insensitive unique; else `SpecificationError`). `tab_color` accepts `#RGB`/`#RRGGBB`/`#RRGGBBAA`; `freeze_panes=(row, col)` uses 0-indexed coordinates. For `layout`, `col_width`, `print`, `theme`, banners, and bands see `references/layout-and-styling.md`.

## 3. Columns and Rows (`add_column`, `add_row`)

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

- Column roles (`AxisRole`):
  - `"measure"` (default): Calculable column participating in `row.expr` broadcasting and `col.prev`/`col.next` stepping.
  - `"descriptor"`: Label/metadata column isolated from `row.expr`, `col.expr`, and row-level `format`/`decimals`/`currency`; requires explicit Tier-1 `cells` entries; rejected by numeric/financial aggregations (`XLOOKUP`/`XMATCH`/`INDEX` allow 1D vertical descriptor slices).
- Headers & `group`: Pass `header="Title"` or stacked `headers=["T1", "T2"]` (not both, else `ValueError`). `group="Banner"` renders a Tier-0 `CenterAcross` banner across contiguous columns with the same non-empty `group` (excluded from single-column width sizing). Columns with fewer header lines bottom-align to the table's max header depth (`0` header rows when ungrouped and no column sets headers); missing column headers emit blank in `.xlsx` while `inspect_sheet` and `TableAuditor.to_markdown()` fall back to `col.id`.
- Vector formulas (`expr`): `col.expr` broadcasts down data rows in `measure` columns; `row.expr` broadcasts across `measure` columns. When `row.expr` uses `col.prev` on the first `measure` column, provide an explicit 0D override in `cells`.
- Validation: Duplicate `id` raises `ValueError`; empty `columns` or `rows` raises `SpecificationError`.

## 4. 0D Cell Overrides (`Cell`, `set_cell`, `set_cells`)

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

type CellValueLiteral = float | int | str | bool | None
type CellInputValue = CellValueLiteral | Cell | DetailedCellDict
type AxisSelector = str | Iterable[str] | slice

table.set_cell(
    row: str, col: str, cell: CellInputValue | _Unset = _UNSET,
    value: CellValueLiteral | _Unset = _UNSET, expr: str | None = None,
    style: SemanticStyle | None = None, highlight: bool | None = None,
    shade: Shade | None = None, format: FormatPreset | None = None,
    decimals: int | None = None, currency: CurrencyCode | None = None,
    indent: int | None = None, note: str | None = None,
) -> Table

table.set_cells(
    row: AxisSelector, cols: AxisSelector, cell: CellInputValue | _Unset = _UNSET,
    value: CellValueLiteral | _Unset = _UNSET, expr: str | None = None,
    style: SemanticStyle | None = None, highlight: bool | None = None,
    shade: Shade | None = None, format: FormatPreset | None = None,
    decimals: int | None = None, currency: CurrencyCode | None = None,
    indent: int | None = None, note: str | None = None,
) -> Table
```

- Payload exclusivity: `Cell` and `DetailedCellDict` on new cells must set `value` XOR `expr` (`ValueError` at construction, `SpecificationError` at compile). `value=None` (or literal `None`) creates an explicit empty cell.
- Attribute constraints (`SpecificationError`):
  - `indent`: Integer `0..=255`; `> 0` valid only on `descriptor` cells (rejected on `measure`).
  - `currency`: Cannot pair with non-`"currency"` `format` on the same spec level; resolves to `None` when effective `format != "currency"` or on `descriptor` columns.
  - `decimals`: `0..=14`; cannot pair with `"integer"`, `"date"`, or `"text"` on the same spec level; resolves to `None` when effective `format` is `"integer"`, `"date"`, or `"text"` (when effective `format` is `None`, explicit `decimals` still governs in-memory formatting).
  - Unknown column key in `row.cells` raises `SpecificationError`.
- Create vs. merge in `set_cell`/`set_cells`:
  - Existing 0D entry in `row.cells`: merges non-`None` metadata while preserving existing `value`/`expr` unless a new payload is supplied.
  - New 0D override (including cells currently filled by `row.expr`/`col.expr`): must pass `value`, `expr`, or `cell` payload, else `ValueError`.
- `AxisSelector`: Accepts a single `str` ID, non-empty `Iterable[str]`, or inclusive `slice("start", "stop")` across declared data rows/columns (skipping gutters and section headers). Unknown IDs raise `KeyError`; open-ended/stepped/inverted slices or empty iterables raise `ValueError`. `set_cells` validates all coordinates atomically before mutating.

```python
table.set_cells("amort", ["standalone", "adj"], value=None)
table.set_cells(
    slice("r1", "r2"), slice("c1", "c2"), expr='Table[@row, "base"] * Table["mult", @col]'
)
table.set_cell("r1", "c1", highlight=True, note="Patched existing 0D cell")
```

## 5. 3-Tier Specificity Lattice

Resolution order per `(row, col)`: `Tier 1 (0D Cell) > Tier 2 (1D Row/Col) > Tier 3 (Default)`.

| Attribute   | Tier 1 (0D Cell)                                      | Tier 2 (`measure` Row vs. Col)                         | Descriptor Col                   | Tier 3 (Default)                                             |
| :---------- | :---------------------------------------------------- | :----------------------------------------------------- | :------------------------------- | :----------------------------------------------------------- |
| Payload     | `cell.value` / `cell.expr`                            | `row.expr` XOR `col.expr` (both -> error)              | Tier 1 required (else error)     | None (unassigned -> error)                                   |
| `format`    | `cell.format`                                         | `row.format` / `col.format` (diff -> error)            | `cell` else `col` (row isolated) | `None`                                                       |
| `decimals`  | `cell.decimals`                                       | `row.decimals` / `col.decimals` (diff -> error)        | `cell` else `col` (row isolated) | Theme default in formatting/emit (`None` in `cell.decimals`) |
| `currency`  | `cell.currency`                                       | `row.currency` / `col.currency` (diff -> error)        | `None` (ignored)                 | `table.currency` else `"usd"` (only on `"currency"`)         |
| `shade`     | `cell.shade` (`"none"` clears)                        | `row.shade` / `col.shade` (diff non-`"none"` -> error) | Same as `measure`                | `None`                                                       |
| `style`     | `"data"` resets both; others override horizontal axis | 2D pair `(h=row.style, v=col.style)`                   | Same as `measure`                | `("data", "data")`                                           |
| `highlight` | `cell.highlight` (`False` clears)                     | `bool(row.highlight or col.highlight)`                 | Same as `measure`                | `False`                                                      |
| `indent`    | `cell.indent`                                         | N/A                                                    | `cell.indent` else `0`           | `0` (`> 0` on `measure` -> error)                            |
| `note`      | `cell.note`                                           | N/A                                                    | `cell.note` else `None`          | `None`                                                       |

All lattice conflicts and unassigned cells raise `SpecificationError`.
