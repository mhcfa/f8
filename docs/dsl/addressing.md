# Addressing Syntax

This document defines the reference addressing syntax used in Finx formulas and specification models.

## 1. Core Invariants

- **Mandatory table qualification**: Every cell reference must specify its target table identifier (`Table[row, col]`). Bare, unqualified references (e.g. `["revenue", @]`) are strictly prohibited.
- **Universal 2D coordinate matrix**: All references use two-dimensional bracket indexing: `Table[RowSelector, ColSelector]`. Dot access (e.g. `Table.row[...]`) is strictly prohibited, ensuring row and column dimensions are fully symmetrical and orthogonal across both horizontal and vertical schedules.
- **Strictly quoted coordinate keys**: Every row and column key inside bracket selectors must be an explicit double-quoted string (e.g. `["revenue"]`, `["2024E"]`, `["r_and_d".."g_and_a"]`). Bare identifiers are never permitted as coordinate keys, guaranteeing immunity from keyword collisions, numeric prefixes (`2024E`), and date formats.
- **Unquoted keywords and selectors**: Built-in language constructs (`@`, `col.prev`, `col.next`, `cols.<group>`, `rows.<group>`) are unquoted tokens within bracket selectors.
- **Active column symbol (`@`)**: The symbol `@` designates the evaluating cell's active column identifier (`ColId`) at index $t$. In cross-table references (`TableB["row", @]`), the target table must declare a column matching this identical `ColId`; positional index matching without matching column IDs is strictly prohibited.
- **Strict relative temporal offsets**: `col.prev(k)` and `col.next(k)` compute relative column offsets ($t - k$ and $t + k$) along the origin cell's table timeline, binding to the corresponding `ColId` in the target table. There is zero implicit string matching or calendar heuristics against column IDs.
- **Syntactic sugar for group milestones**: `cols.<group>.first`, `cols.<group>.last`, `rows.<group>.first`, and `rows.<group>.last` are compile-time syntactic sugar that resolve against the target table's registered groups into concrete, static coordinates.
- **Inclusive range intervals**: All coordinate ranges (`["col_a".."col_b"]` and `["row_a".."row_b"]`) are inclusive closed intervals ($[a, b]$) and must follow strict declaration order ($start\_idx \le end\_idx$). Inverted ranges are rejected at compile time.
- **No implicit broadcasting**: Aggregations across rows or columns must provide explicit range bounds (e.g. `["2024E".."2028E"]`) or named group bounds (e.g. `[cols.forecast]`).
- **Zero alternative syntax**: There is exactly one canonical way to reference any cell, range, or block. Syntactic redundancies are eliminated.

---

## 2. Addressing Modes

```mermaid
flowchart TD
    A["Reference"] --> B{"Syntax Shape"}
    B -->|"Table['row', @]"| C["Single Cell (Active Column)<br>e.g. IncomeStatement['revenue', @]"]
    B -->|"Table['row', 'col']"| D["Single Cell (Specific Column)<br>e.g. Assumptions['tax_rate', 'value']"]
    B -->|"Table['row', col.prev]"| E["Single Cell (Temporal Offset)<br>e.g. BalanceSheet['cash', col.prev]"]
    B -->|"Table['row', cols.group.last]"| J["Single Cell (Group Milestone Anchor)<br>e.g. DebtSchedule['balance', cols.historical.last]"]
    B -->|"Table['row', 'col_a'..'col_b']"| F["Row Range (Horizontal Slice)<br>e.g. IncomeStatement['revenue', '2024E'..'2028E']"]
    B -->|"Table['row', cols.group]"| K["Row Range (Horizontal Group Slice)<br>e.g. IncomeStatement['revenue', cols.forecast]"]
    B -->|"Table['row_a'..'row_b', @]"| G["Column Range (Vertical Slice, Active)<br>e.g. IncomeStatement['r_and_d'..'g_and_a', @]"]
    B -->|"Table[rows.group, @]"| L["Column Range (Vertical Group Slice, Active)<br>e.g. BalanceSheet[rows.current_assets, @]"]
    B -->|"Table['row_a'..'row_b', 'col']"| H["Column Range (Vertical Slice, Specific)<br>e.g. IncomeStatement['r_and_d'..'g_and_a', '2023A']"]
    B -->|"Table['row_a'..'row_b', 'col_a'..'col_b']"| I["2D Block Range (Explicit Keys)<br>e.g. IncomeStatement['r_and_d'..'g_and_a', '2024E'..'2028E']"]
    B -->|"Table[rows.group, cols.group]"| M["2D Block Range (Group Slices)<br>e.g. BalanceSheet[rows.current_assets, cols.forecast]"]
```

---

## 3. Syntax Specifications & Lowering Rules

### 3.1. Active Period Cell (`[@]`)

References a row at the active evaluation column index $t$.

```text
IncomeStatement["revenue", @] - IncomeStatement["cogs", @]
```

- **Resolution**: Binds to the column identifier (`ColId`) of the cell currently being evaluated at index $t$.
- **Cross-table validation**: When referencing another table (`TableB["metric", @]`), the target table must declare a column with that identical `ColId`. If the target table lacks this column (e.g. `Assumptions["tax_rate", @]` where `Assumptions` only declares `["value"]`), compilation fails with `UnknownColumn`.
- **Lowering**: Emits relative column coordinates (e.g. `=C10 - C11` when evaluating in column C, `=D10 - D11` in column D). When referencing another table sharing the timeline, emits relative column coordinates targeting the referenced table (e.g. `=BalanceSheet!C10`).

### 3.2. Specific Column Cell

References a concrete, static column coordinate.

```text
IncomeStatement["ebit", @] * Assumptions["tax_rate", "value"]
DebtSchedule["balance", "2023A"]
Scenarios["growth_rate", "Bull"]
```

- **Lowering**: Emits absolute coordinates with `$` (e.g. `='Summary'!$B$4` or `=$C$10`).

### 3.3. Temporal Offset Cell (`col.prev`, `col.next`)

References a relative column offset from the active evaluation column $t$.

```text
BalanceSheet["cash", col.prev] + CashFlow["net_change", @]
CashFlow["ar_change", @] = BalanceSheet["accounts_receivable", @] - BalanceSheet["accounts_receivable", col.prev]
```

- **Supported forms**:
  - `col.prev`: Column $t - 1$.
  - `col.prev(k)`: Column $t - k$.
  - `col.next`: Column $t + 1$.
  - `col.next(k)`: Column $t + k$.
- **Timeline-relative resolution**: Evaluates strictly against column index $t \pm k$ within the origin table's declared columns:
  $$\text{TargetColId} = \text{OriginTable.columns}[t \pm k]$$
- **Cross-table support**: Allowed across tables (e.g. in Cash Flow Statement referencing prior-period Balance Sheet). The target table must contain $\text{TargetColId}$; otherwise compilation fails with `UnknownColumn`.
- **Bounds enforcement**:
  - If $t - k < 0$, compilation fails with `TemporalUnderflow`.
  - If $t + k \ge \text{OriginTable.columns.len()}$, compilation fails with `TemporalOverflow`.
- **Requirement**: The initial period ($t = 0$) using `col.prev` requires an explicit base anchor via `match` (e.g. `"2023A": { "value": [100.0] }` or referencing `cols.historical.last`).
- **Lowering**: Emits relative column coordinates in Excel (e.g. `=C10` when evaluating at column D).

### 3.4. Semantic Group Milestones (`cols.<group>.first`, `cols.<group>.last`, `rows.<group>.first`, `rows.<group>.last`)

References a named boundary within a declared column or row group.

```text
DebtSchedule["balance", cols.historical.last]
DebtSchedule["balance", cols.forecast.first]
Comps["market_cap", cols.market_data.first]
BalanceSheet[rows.current_assets.first, @]
BalanceSheet[rows.current_assets.last, "2024E"]
```

- **Syntactic sugar**: Resolves at compile time into the concrete coordinate key of the group's boundary (e.g. `"2023A"` or `"cash"`), lowering to an absolute coordinate (e.g. `=$C$10` or `='Summary'!$B$4`).
- **Target table scoping**: The group token is strictly resolved against the declared `column_groups` or `row_groups` of the **target table**. If the target table does not define the group, compilation fails with `UnknownColGroup` / `UnknownRowGroup`.
- **Safety guarantee**: Validated at schema ingestion to ensure groups are non-empty (`len >= 1`) and contiguous, ensuring `.first` and `.last` are always deterministic and statically safe.

### 3.5. Horizontal Range (Row Slice)

Aggregates multiple columns along a single row.

```text
SUM(IncomeStatement["revenue", "2024E".."2028E"])
SUM(IncomeStatement["revenue", cols.forecast])
```

- **Inclusive intervals**: Both boundary keys are included ($[start, end]$).
- **Declaration order**: Requires $start\_idx \le end\_idx$ in declaration order; inverted ranges fail with `InvalidColRange`.
- **Lowering**: Static coordinate keys and group boundaries lower to absolute cell ranges (e.g. `=SUM($C$10:$G$10)`).

### 3.6. Vertical Range (Column Slice)

Aggregates multiple rows within a single column using explicit row key bounds or semantic row groups.

```text
SUM(IncomeStatement["r_and_d".."g_and_a", @])
SUM(BalanceSheet[rows.current_assets, @])
SUM(BalanceSheet[rows.current_assets, "2024E"])
```

- **Inclusive intervals**: Both boundary rows are included ($[start, end]$). Inverted row ranges fail with `InvalidRowRange`.
- **Lowering**: Resolves `rows.<group>` to the contiguous vertical row slice of all flattened descendant rows of the targeted group, emitting vertical ranges with relative column for active column (`=SUM(C$14:C$18)`) or absolute column for specific column keys (`=SUM($B$14:$B$18)`).

### 3.7. 2D Block Range

Aggregates a two-dimensional matrix across multiple rows and columns.

```text
SUM(IncomeStatement["r_and_d".."g_and_a", "2024E".."2028E"])
SUM(BalanceSheet[rows.current_assets, cols.forecast])
```

- **Inclusive intervals**: Rectangular closed interval across both dimensions.
- **Lowering**: Emits 2D rectangular ranges (e.g. `=SUM($C$14:$G$18)`).

---

## 4. Cross-Sheet & Coordinate Anchoring Semantics

All table references are resolved through the workbook symbol table:

- **Intra-sheet**: When referencing a table located on the same worksheet, coordinates omit sheet prefixes (`=C10`).
- **Inter-sheet**: When referencing a table on a different worksheet, the compiler automatically adds quoted sheet qualification (`='Summary'!$B$4`).
- **Dynamic vs. absolute coordinate anchoring**:
  - **Dynamic coordinates (`@`, `col.prev`, `col.next`)**: Lower to relative column coordinates so that formulas track timeline periods across columns.
  - **Static coordinates (explicit `["col"]`, `["start".."end"]`, `cols.<group>`, milestones)**: Lower to absolute coordinates (`$`) so they remain fixed when evaluated across columns.

See [Formula Grammar](grammar.md) for the complete EBNF specification and [Formula Lowering & Emission](../compiler/lowering-and-emission.md) for coordinate resolution details.
