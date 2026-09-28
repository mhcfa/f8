# Function Reference

Comprehensive specification of the 44 built-in Excel functions supported by the `f8` formula engine, covering signatures, parameter semantics, default values, argument omission, and error modes.

---

## 1. Syntax Invariants & Call Rules

- **Calculation parity vs. runtime behaviors**: Built-in functions maintain 100% calculation parity with Excel (identical IEEE 754 precision, type coercion rules, criteria wildcards, and domain error codes). Because `f8` compiles into a static dependency DAG with fixed 2D cell geometry, dynamic runtime behaviors such as dynamic array spilling (`#SPILL!`), implicit intersection, or volatile non-deterministic evaluation (`TODAY()`, `RAND()`) are excluded by design.
- **Uppercase identifiers**: Built-in function names must be uppercase (e.g. `SUM`, `AVERAGE`, `IF`, `XLOOKUP`). Lowercase identifiers (e.g. `sum(...)`) raise `FormulaParseError`.
- **Compile-time function validation**: Referencing an unknown or misspelled function halts compilation with `F8Error` (`unknown function '...'`), rather than evaluating to `#NAME?`.
- **Positional argument omission (`_`)**: Optional positional parameters can be explicitly omitted using the underscore placeholder `_` (e.g. `PMT(rate, nper, pv, _, 1)` or `IF(test, true_val, _)`). Omitted arguments resolve to their parameter default value (or remain omitted for lazy arguments such as `XLOOKUP`'s `if_not_found`).
- **Required parameter enforcement**: Required parameters cannot be omitted; invoking `ABS(_)` or `IF(_, 10, 20)` raises `F8Error` (`CannotOmitRequiredArgument`).
- **Grammatical confinement**: The `_` token is strictly confined to function call argument lists (`Argument = Expression | OmittedArg`). Using `_` in arithmetic expressions (`1 + _`), table coordinate selectors (`Table[_, @col]`), or table identifiers raises `FormulaParseError`.
- **Excel domain calculation errors**: Evaluated functions return standard Excel calculation errors when preconditions or domain constraints fail:
  - `#DIV/0!`: Division by zero (e.g. `AVERAGE` over an empty range, $r = -1.0$ in `NPV`, `nper = 0` in `PMT`, or `AVERAGEIFS` with zero matching cells).
  - `#VALUE!`: Type coercion failure (e.g. text in arithmetic functions), dimension mismatch in `SUMPRODUCT`/`*IFS`/`XLOOKUP`, supplying a 2D matrix to `XLOOKUP`/`XMATCH`, omitting `col_num` on a 2D matrix in `INDEX`, combining wildcard matching with binary search, or supplying zero logical values to `AND`/`OR`.
  - `#REF!`: Out-of-bounds row/column coordinate or empty coordinate range in `INDEX`.
  - `#NUM!`: Iteration convergence failure in `IRR`/`XIRR`, discount rate $r \le -1.0$ in `XNPV`, fewer than 2 cash flows or missing sign alternation in `IRR`/`XIRR`, target period $per < 1$ or $per > nper$ in `IPMT`/`PPMT`, empty numeric input in `MEDIAN`, invalid `basis` outside `0..=4` in `YEARFRAC`, or serial date outside `0.0..=2958465.0`.
  - `#N/A`: Unmatched lookup in `XLOOKUP`/`XMATCH` without fallback, unsatisfied conditions in `IFS`, or unmatched case in `SWITCH` without default.

---

## 2. Math & Aggregation Functions (17)

| Function | Signature | Semantics |
| :--- | :--- | :--- |
| `ABS` | `ABS(number)` | Absolute value. Text scalars return `#VALUE!`; booleans convert to `1.0`/`0.0`; empty cells evaluate to `0.0`. |
| `SUM` | `SUM(number1, [number2], ...)` | Sums numbers across scalars and range slices. Text, booleans, and empty cells in ranges are ignored; direct text scalars return `#VALUE!`. |
| `AVERAGE` | `AVERAGE(number1, [number2], ...)` | Arithmetic mean. Range text and booleans are ignored; empty ranges return `#DIV/0!`. |
| `COUNT` | `COUNT(value1, [value2], ...)` | Counts numeric cells. In ranges, only numbers are counted (booleans, text, blanks, and errors are ignored); in direct scalars, numbers and booleans count as 1. |
| `COUNTA` | `COUNTA(value1, [value2], ...)` | Counts non-empty cells (numbers, text, booleans, errors). Ignores only empty cells. |
| `MAX` | `MAX(number1, [number2], ...)` | Maximum numeric value. Ignores non-numbers in ranges; empty ranges return `0.0`. Direct text scalars return `#VALUE!`. |
| `MIN` | `MIN(number1, [number2], ...)` | Minimum numeric value. Ignores non-numbers in ranges; empty ranges return `0.0`. Direct text scalars return `#VALUE!`. |
| `MEDIAN` | `MEDIAN(number1, [number2], ...)` | Median value of numeric inputs. Ignores non-numeric cells in ranges; empty ranges or inputs with no numeric values return `#NUM!`. |
| `ROUND` | `ROUND(number, num_digits)` | Rounds to `num_digits` precision using half-away-from-zero (standard commercial rounding). Negative digits round left of the decimal point. |
| `ROUNDUP` | `ROUNDUP(number, num_digits)` | Rounds number away from zero to `num_digits` places. |
| `ROUNDDOWN` | `ROUNDDOWN(number, num_digits)` | Truncates number toward zero to `num_digits` places. |
| `SUMPRODUCT` | `SUMPRODUCT(array1, [array2], ...)` | Multiplies corresponding elements across matching range slices and sums products. Non-numeric entries count as `0.0`; mismatched dimensions return `#VALUE!`. |
| `SUMIFS` | `SUMIFS(sum_range, criteria_range1, criteria1, [criteria_range2, criteria2], ...)` | Sums cells in `sum_range` satisfying all criteria pairs. Range dimension mismatches return `#VALUE!`. Returns `0.0` if no cells match. |
| `COUNTIFS` | `COUNTIFS(criteria_range1, criteria1, [criteria_range2, criteria2], ...)` | Counts cells satisfying all criteria pairs. Range dimension mismatches return `#VALUE!`. Returns `0.0` if no cells match. |
| `AVERAGEIFS` | `AVERAGEIFS(average_range, criteria_range1, criteria1, [criteria_range2, criteria2], ...)` | Arithmetic mean of cells in `average_range` satisfying all criteria pairs. Returns `#DIV/0!` if no cells match; mismatched dimensions return `#VALUE!`. |
| `MINIFS` | `MINIFS(min_range, criteria_range1, criteria1, [criteria_range2, criteria2], ...)` | Minimum value in `min_range` matching all criteria pairs. Returns `0.0` if no cells match; mismatched dimensions return `#VALUE!`. |
| `MAXIFS` | `MAXIFS(max_range, criteria_range1, criteria1, [criteria_range2, criteria2], ...)` | Maximum value in `max_range` matching all criteria pairs. Returns `0.0` if no cells match; mismatched dimensions return `#VALUE!`. |

### Criteria Syntax (`*IFS`)

Conditional aggregation functions accept criterion strings, numbers, or boolean targets:
- **Comparison operators**: String expressions with relational operators: `">0"`, `">=100"`, `"<0"`, `"<>0"`.
- **Exact matching**: Direct numeric, text, or boolean match: `10`, `"Active"`, `"=Approved"`. Text comparisons are ASCII case-insensitive.
- **Blank and non-blank sentinels**: `""` or `"="` matches blank/empty cells; `"<>"` matches non-blank cells. Empty cell references evaluate to numeric `0`.
- **Wildcard matching**: Text matching supporting `?` (single character) and `*` (zero or more characters): `"Tech*"` or `"?024"`. Use `~*`, `~?`, or `~~` to escape literal wildcards or tildes.
- **Dynamic expressions**: Concatenated criteria: `">=" & Assumptions["hurdle", "val"]`.

---

## 3. Logical & Branching Functions (9)

| Function | Signature | Semantics |
| :--- | :--- | :--- |
| `IF` | `IF(logical_test, value_if_true, [value_if_false])` | Short-circuiting branch. Evaluates only the selected branch. `value_if_true` is required; `value_if_false` is optional and defaults to `FALSE`. |
| `IFS` | `IFS(logical_test1, value_if_true1, [logical_test2, value_if_true2], ...)` | Evaluates condition-value pairs in order with short-circuiting. Requires an even number of arguments ($\ge 2$). Returns `#N/A` if no condition evaluates to true. |
| `IFERROR` | `IFERROR(value, value_if_error)` | Short-circuiting fallback. Evaluates and returns `value_if_error` only if `value` evaluates to any error; otherwise returns `value`. |
| `IFNA` | `IFNA(value, value_if_na)` | Short-circuiting fallback. Returns `value_if_na` strictly if `value` evaluates to `#N/A`. Propagates other errors without evaluating fallback. |
| `AND` | `AND(logical1, [logical2], ...)` | Logical conjunction across scalars and ranges. `0` = false, non-zero = true. Ignores empty cells and text in ranges. Returns `#VALUE!` if no logical values are present. |
| `OR` | `OR(logical1, [logical2], ...)` | Logical disjunction across scalars and ranges. Ignores empty cells and text in ranges. Returns `#VALUE!` if no logical values are present. |
| `NOT` | `NOT(logical)` | Inverts truth value: `NOT(TRUE)` $\rightarrow$ `FALSE`, `NOT(0)` $\rightarrow$ `TRUE`. Non-boolean/numeric strings return `#VALUE!`. |
| `CHOOSE` | `CHOOSE(index_num, value1, [value2], ...)` | Evaluates choice at 1-based `index_num` with short-circuiting. Out-of-bounds index ($< 1$ or $> N$) or non-numeric index returns `#VALUE!`. |
| `SWITCH` | `SWITCH(expression, value1, result1, [value2, result2], ..., [default])` | Compares `expression` against values using case-insensitive Excel equality. Short-circuits matching result; returns trailing default or `#N/A` if unhandled. |

---

## 4. Financial & Discounting Functions (7)

| Function | Signature | Semantics |
| :--- | :--- | :--- |
| `NPV` | `NPV(rate, value1, [value2], ...)` | Net present value of periodic cash flows discounted starting at $t=1$: $\sum_{t=1}^n \frac{CF_t}{(1+r)^t}$. Range text and blanks are ignored. Returns `#DIV/0!` if $r = -1.0$. |
| `XNPV` | `XNPV(rate, values, dates)` | Net present value for non-periodic cash flows scheduled on specific dates: $\sum_{i=1}^n \frac{CF_i}{(1+r)^{(d_i - d_1)/365}}$. Requires $r > -1.0$ (rate $\le -1.0$ returns `#NUM!`), equal vector lengths ($\ge 1$), and monotonic dates ($d_i \ge d_1$). |
| `IRR` | `IRR(values, [guess])` | Internal rate of return for periodic cash flows using Newton-Raphson iteration. Default `guess` is `0.10`. Requires at least 2 cash flows and at least one positive and one negative cash flow, else `#NUM!`. |
| `XIRR` | `XIRR(values, dates, [guess])` | Internal rate of return for date-scheduled cash flows using Actual/365 year fractions. Default `guess` is `0.10`. Requires equal array lengths ($\ge 2$), monotonic dates ($d_i \ge d_1$), and at least one positive and one negative cash flow, else `#NUM!`. |
| `PMT` | `PMT(rate, nper, pv, [fv], [type])` | Constant periodic payment for an annuity based on periodic interest `rate`, total periods `nper`, and present value `pv`. Default `fv` is `0.0`, default `type` is `0` (end of period; non-zero for beginning). Supports `rate = 0.0` linear amortization; returns `#DIV/0!` if `nper = 0.0`. |
| `IPMT` | `IPMT(rate, per, nper, pv, [fv], [type])` | Interest portion of an annuity payment for period `per`. Returns `0.0` if `rate = 0.0`. Requires $1 \le \text{per} \le \text{nper}$, otherwise `#NUM!`. |
| `PPMT` | `PPMT(rate, per, nper, pv, [fv], [type])` | Principal repayment portion of an annuity payment for period `per`. Requires $1 \le \text{per} \le \text{nper}$, otherwise `#NUM!`. Strictly satisfies $\text{PMT} = \text{IPMT} + \text{PPMT}$. |

### Financial Modeling Patterns

```python
from f8 import Cell

# Monthly loan payment with omitted optional parameters
table.add_row(
    "debt_service",
    cells={"line_item": "Monthly Debt Service"},
    expr='PMT(Assumptions["rate", @col] / 12, Assumptions["tenor_months", @col], -DebtSchedule["principal", @col], _, 0)',
    format="currency",
    decimals=0,
)

# Project NPV using cost of capital driver
table.add_row(
    "npv",
    cells={
        "line_item": "Project NPV",
        "metric": Cell(expr='NPV(Assumptions["wacc", "val"], CashFlows["fcf", "2024E".."2028E"])'),
    },
    format="currency",
    decimals=1,
)
```

---

## 5. Lookup & Reference Functions (3)

| Function | Signature | Semantics |
| :--- | :--- | :--- |
| `XLOOKUP` | `XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode])` | Searches 1D `lookup_array` for `lookup_value` and returns corresponding item from 1D `return_array`. Default `match_mode` is `0` (exact); default `search_mode` is `1` (first-to-last). If omitted without `if_not_found`, returns `#N/A`. 2D ranges or length mismatches return `#VALUE!`. |
| `XMATCH` | `XMATCH(lookup_value, lookup_array, [match_mode], [search_mode])` | Returns 1-based relative position of `lookup_value` in 1D `lookup_array`. Returns `#N/A` if no match; 2D arrays return `#VALUE!`. |
| `INDEX` | `INDEX(array, row_num, [col_num])` | Returns the cell value at 1-based `row_num` and `col_num` within a 1D or 2D range. On 1D vectors, omitting `col_num` resolves along the vector's primary axis; on 2D matrices, omitting `col_num` returns `#VALUE!`. Out-of-bounds indices return `#REF!`. |

### Match & Search Modes

- **`match_mode`**:
  - `0` (default): Exact match.
  - `-1`: Exact match or next smaller item.
  - `1`: Exact match or next larger item.
  - `2`: Wildcard match (`*`, `?`, `~`).
- **`search_mode`**:
  - `1` (default): Search starting at first item (top/left to bottom/right).
  - `-1`: Reverse search starting at last item.
  - `2`: Binary search on ascending sorted array.
  - `-2`: Binary search on descending sorted array.
- **Lookup typing & dimension rules**:
  - **Strict type equality**: Lookups use type-aware equality (`lookup_eq`); numeric values never match numeric text strings (e.g. `10` does not match `"10"`), and `Empty` only matches `Empty`.
  - **Wildcard and binary search incompatibility**: Combining wildcard match mode (`match_mode = 2`) with binary search (`search_mode = 2` or `-2`) returns `#VALUE!`.
  - **1D vector constraint**: `lookup_array` and `return_array` must be 1D slices (single row or single column of matching length, orientation-independent). Passing a 2D matrix (both rows > 1 and cols > 1) returns `#VALUE!`.

```python
# Scenario lookup across horizontal parameter ranges
table.add_row(
    "growth_rate",
    cells={"line_item": "Revenue Growth Rate"},
    expr='XLOOKUP(Assumptions["active_scenario", "val"], Scenarios["id", "base".."bull"], Scenarios["growth", "base".."bull"])',
    format="percentage",
    decimals=1,
)
```

---

## 6. Date & Time Functions (8)

| Function | Signature | Semantics |
| :--- | :--- | :--- |
| `DATE` | `DATE(year, month, day)` | Constructs an Excel serial date number ($0.0 \le \text{serial} \le 2958465.0$). Years $0 \le y \le 1899$ map to $y + 1900$. Handles month/day rollover and preserves the Excel 1900 leap year bug (`1900-02-29` $\rightarrow 60.0$). Dates outside range return `#NUM!`. |
| `EDATE` | `EDATE(start_date, months)` | Shifts `start_date` by `months` calendar months, preserving day-of-month or pinning to the final valid day of the target month. |
| `EOMONTH` | `EOMONTH(start_date, months)` | Returns serial date for the final day of the month that is `months` before or after `start_date`. |
| `YEARFRAC` | `YEARFRAC(start_date, end_date, [basis])` | Fraction of a year between two dates based on contractual day-count convention. Default `basis` is `0` (US 30/360). Order-independent ($d_1$ and $d_2$ can be in either order). Basis outside `0..=4` returns `#NUM!`. |
| `YEAR` | `YEAR(serial_number)` | Extracts 4-digit calendar year (1900–9999) from serial date. Fractional day times are truncated. |
| `MONTH` | `MONTH(serial_number)` | Extracts calendar month (1–12) from serial date. |
| `DAY` | `DAY(serial_number)` | Extracts day of the month (1–31) from serial date. |
| `DAYS` | `DAYS(end_date, start_date)` | Number of calendar days between two dates (`end_date.trunc() - start_date.trunc()`). Note: `end_date` is the first parameter. |

### Day-Count Conventions (`YEARFRAC`)

| Basis | Day-count convention | Standard financial application |
| :--- | :--- | :--- |
| `0` (or omitted) | US (NASD) 30/360 | Corporate bonds, US agency debt |
| `1` | Actual/Actual | US Treasury bonds, sovereign debt |
| `2` | Actual/360 | Money market instruments, commercial paper, SOFR/LIBOR |
| `3` | Actual/365 | UK Gilts, commonwealth money markets |
| `4` | European 30/360 | Eurobonds, European corporate issues |
