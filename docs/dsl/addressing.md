# Addressing Syntax

This document defines the reference addressing syntax used in Finx formulas and specification models.

## 1. Core Invariants

- **Two coordinates mandatory**: Every cell reference must specify both a row selector and a column selector. Bare row references (e.g. `IncomeStatement.revenue`) are strictly prohibited.
- **Active column symbol (`@`)**: The symbol `@` designates the active evaluation column $t$.
- **No implicit broadcasting**: Aggregations across rows or columns must provide explicit range bounds (e.g. `["2024E".."2028E"]`) or named group bounds (e.g. `[cols.forecast]`).
- **Strictly quoted coordinate keys**: Every row and column key inside bracket selectors must be an explicit double-quoted string (e.g. `["value"]`, `["2024E"]`, `["r_and_d".."g_and_a"]`). Bare identifiers are never permitted as coordinate keys inside brackets.

---

## 2. Addressing Modes

```mermaid
flowchart TD
    A["Reference"] --> B{"Syntax Shape"}
    B -->|"Table.row[@]"| C["Single Cell (Active Column)\ne.g. IncomeStatement.revenue[@]"]
    B -->|"Table.row[\"col\"]"| D["Single Cell (Specific Column)\ne.g. Assumptions.tax_rate[\"value\"]"]
    B -->|"Table.row[col.prev]"| E["Single Cell (Temporal Offset)\ne.g. BalanceSheet.cash[col.prev]"]
    B -->|"Table.row[\"col_a\"..\"col_b\"]"| F["Row Range (Horizontal Slice)\ne.g. revenue[\"2024E\"..\"2028E\"]"]
    B -->|"Table[\"row_a\"..\"row_b\", @]"| G["Column Range (Vertical Slice, Active)\ne.g. Opex[\"r_and_d\"..\"g_and_a\", @]"]
    B -->|"Table[\"row_a\"..\"row_b\", \"col\"]"| H["Column Range (Vertical Slice, Specific)\ne.g. Opex[\"r_and_d\"..\"g_and_a\", \"2023A\"]"]
    B -->|"Table[\"row_a\"..\"row_b\", \"col_a\"..\"col_b\"]"| I["2D Block Range\ne.g. Opex[\"r_and_d\"..\"g_and_a\", cols.forecast]"]
```

---

## 3. Syntax Specifications & Lowering Rules

### 3.1. Active Period Cell (`[@]`)

References a row at the active evaluation column $t$.

```text
IncomeStatement.revenue[@] - IncomeStatement.cogs[@]
```

- **Lowering**: Emits relative column coordinates within the same sheet (e.g. `=C10 - C11` for column C, `=D10 - D11` for column D).

### 3.2. Specific Column Cell

References a concrete, static column coordinate.

```text
IncomeStatement.ebit[@] * Assumptions.tax_rate["value"]
DebtSchedule.balance["2023A"]
Scenarios.growth_rate["Bull"]
```

- **Lowering**: Emits absolute column coordinates (e.g. `='Summary'!$B$4` or `$C$10`).

### 3.3. Temporal Offset Cell

References an offset relative to active column $t$.

```text
BalanceSheet.cash[col.prev] + CashFlow.net_change[@]
```

- **Supported forms**:
  - `col.prev`: Column $t-1$.
  - `col.prev(k)`: Column $t-k$.
  - `col.next(k)`: Column $t+k$.
- **Requirement**: The initial column using `col.prev` requires an explicit base anchor via `match`.

### 3.4. Semantic Column Anchor

References a named milestone or boundary within a column group.

```text
DebtSchedule.balance[cols.historical.last]
DebtSchedule.balance[cols.forecast.first]
Comps.market_cap[cols.market_data.first]
```

- **Lowering**: Resolves at compile time to the matching column key (e.g. `"2023A"`), lowering to an absolute coordinate.

### 3.5. Horizontal Range (Row Slice)

Aggregates multiple columns along a single row.

```text
SUM(IncomeStatement.revenue["2024E".."2028E"])
SUM(IncomeStatement.revenue[cols.forecast])
```

- **Lowering**: Emits contiguous horizontal cell ranges (e.g. `=SUM(C10:G10)`).

### 3.6. Vertical Range (Column Slice)

Aggregates multiple rows within a single column using explicit row key bounds or semantic row groups.

```text
SUM(Opex["r_and_d".."g_and_a", @])
SUM(BalanceSheet[rows.current_assets, @])
SUM(BalanceSheet[rows.current_assets, "2024E"])
```

- **Lowering**: Resolves `rows.<group>` to the contiguous vertical row slice of all flattened descendant rows of the targeted group, emitting vertical ranges for active column (`=SUM(C14:C18)`) or absolute column (`=SUM($B$14:$B$18)`).

### 3.7. 2D Block Range

Aggregates a two-dimensional matrix across multiple rows and columns.

```text
SUM(Opex["r_and_d".."g_and_a", "2024E".."2028E"])
SUM(BalanceSheet[rows.current_assets, cols.forecast])
```

- **Lowering**: Emits 2D rectangular ranges (e.g. `=SUM(C14:G18)`).

---

## 4. Cross-Sheet Lowering Semantics

All table references are resolved through the workbook symbol table:

- **Intra-sheet**: When referencing a table located on the same worksheet, coordinates omit sheet prefixes (`=C10`).
- **Inter-sheet**: When referencing a table on a different worksheet, the compiler automatically adds quoted sheet qualification (`='Summary'!$B$4`).

See [Formula Grammar](grammar.md) for the complete EBNF specification and [Formula Lowering & Emission](../compiler/lowering-and-emission.md) for coordinate resolution details.
