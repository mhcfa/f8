# Formula Lowering & Binary Emission

This document specifies Finx's formula lowering engine and workbook emission pipeline (Pass 6), transforming semantic AST expressions into Excel formulas and serializing `.xlsx` binaries or headless data.

## 1. Formula Lowering Engine

Formula lowering resolves high-level semantic coordinates into concrete Excel references with appropriate relative and absolute coordinate referencing (`$`):

```mermaid
flowchart LR
    A["Formula AST Node"] --> B["Resolve Target CellId"]
    B --> C["Lookup Physical (Sheet, Col, Row)<br>from Spatial Layout Map"]
    C --> D["Apply Referencing Rules ($)"]
    D --> E["Emit Excel Formula String<br>(e.g. =C10-C11)"]
```

### 1.1. Coordinate Lowering Rules

| Reference Type | Spec Syntax | Sheet Context | Lowered Excel Formula | Referencing Semantics |
| :--- | :--- | :--- | :--- | :--- |
| **Active Column (`@col`)** | `IncomeStatement["revenue", @col]` | Same sheet | `=C10` | Relative column (tracks evaluation column) |
| **Active Row (`@row`)** | `CapTable[@row, "shares"]` | Same sheet | `=C5` | Relative row (tracks evaluation row) |
| **Column Offset** | `BalanceSheet["cash", col.prev]` | Same sheet | `=C10` (at D10) | Relative column offset ($t \pm k$) |
| **Row Offset** | `DebtSchedule[row.prev, "end_balance"]` | Same sheet | `=E9` (at row 10) | Relative row offset ($r \pm k$) |
| **Specific Cell** | `Assumptions["tax_rate", "value"]` | Different sheet (`Summary`) | `='Summary'!$B$4` | Absolute coordinate (fixed key) |
| **Specific Cell** | `DebtSchedule["balance", "2023A"]` | Same sheet | `=$C$10` | Absolute coordinate (fixed key) |
| **Horizontal Range** | `IncomeStatement["revenue", "2024E".."2028E"]` | Same sheet | `=SUM($C$10:$G$10)` | Absolute column bounds |
| **Horizontal Range (Active)** | `CapTable[@row, "shares".."invested_capital"]` | Same sheet | `=SUM(C5:E5)` | Relative row, absolute column bounds |
| **Vertical Range (Active)** | `IncomeStatement["r_and_d".."g_and_a", @col]` | Same sheet | `=SUM(C$14:C$18)` | Relative column, absolute row bounds |
| **Vertical Range (Specific)** | `IncomeStatement["r_and_d".."g_and_a", "2024E"]` | Same sheet | `=SUM($C$14:$C$18)` | Absolute column, absolute row bounds |
| **2D Block Range** | `IncomeStatement["r_and_d".."g_and_a", "2024E".."2028E"]` | Same sheet | `=SUM($C$14:$G$18)` | Absolute 2D matrix bounds |

### 1.2. AST Serialization
- Function names map to canonical uppercase names (`SUM`, `IF`, `XLOOKUP`).
- Operators map to native Excel symbols (`+`, `-`, `*`, `/`, `^`, `=`, `<>`).
- Emitted formula strings are prefixed with `=`.

---

## 2. Excel Binary Emission (`rust_xlsxwriter`)

The binary emitter streams the spatial layout, styling, and lowered formulas into a valid `.xlsx` package:

1. **Calculation configuration**: Sets calculation properties in workbook XML:
   - `calcPr.iterate = false` (enforcing zero circularity).
   - `calcPr.fullCalcOnLoad = true` (requesting full calculation on first open).
2. **Format cache & theme translation**: Deduplicates styles, fonts, borders, fills, and number format strings by translating each coordinate's `StyleRole` and resolved `(format, decimals)` tuple through the active `Theme`, dynamically synthesizing concrete Excel number format masks via `{dec}` template expansion.
3. **Deterministic metadata & reproducible compilation**:
   - Workbook package properties (`docProps/core.xml`) are populated deterministically.
   - Dynamic wall-clock timestamps are omitted to ensure bit-for-bit reproducible binary outputs across identical Git commits (respecting `SOURCE_DATE_EPOCH`).
4. **Header & structure streaming**:
   - **Table title banners**: Emitted at `start_col` and styled across `start_col..=end_col` with title fill and borders.
   - **Super-headers (Center Across Selection)**: Emits contiguous identical multi-tier header runs using `FormatAlign::CenterAcross` across adjacent columns without `<mergeCell>` tags (conforming to FAST financial modeling standards).
   - **Section headers & gutters**: Emitted at explicit physical coordinates without calculation node overhead.
   - **Table collapsing (opt-in)**: When `table.collapsible: true`, groups body rows below the title banner (`worksheet.group_rows`) with `group_symbols_above(true)`.
5. **Cell streaming**:
   - **Descriptor indentation**: Applies `format.set_indent(indent)` to descriptor cells that specify an indentation level.
   - **Literal cells**: Emitted via `write_number_with_format`, `write_string_with_format`, or `write_boolean_with_format`.
   - **Formula cells**: Emitted via `write_formula_with_format`. In standard builds, each formula cell is pre-populated with its in-memory evaluated result (`set_result`). This ensures non-Excel viewers (Apple Numbers, macOS Quick Look, Google Drive preview, pandas, LibreOffice) display valid numbers immediately without an Excel recalculation pass.
   - **Values-only freezing**: When compiled with `--values-only`, formula cells are emitted as pure literals, producing frozen workbooks suitable for M&A data rooms and audit baselines.
6. **Finalization**: Compresses and saves the OpenXML package.

---

## 3. Headless Data Serialization

Because the in-memory evaluator maintains the complete `CellValue` matrix across all tables, Finx supports direct headless exports without Excel binary overhead:

- **JSON / CSV / Parquet**: Exports evaluated tables directly to structured files for AI agent consumption, CI/CD verification, and database pipelines.

See [Spatial Layout Planning](spatial-layout.md) for grid assignment, [Semantic Formatting & Themes](../dsl/formatting.md) for theme configurations, [In-Memory Evaluator](./evaluator.md) for precalculated cache values, and [Addressing Syntax](../dsl/addressing.md) for input reference formats.
