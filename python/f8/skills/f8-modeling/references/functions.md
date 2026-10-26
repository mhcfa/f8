# Function Reference

Signatures, parameter defaults, argument omission (`_`), criteria matching rules, and domain error modes for the 44 built-in Excel functions in `f8`.

---

## 1. Call Rules, Argument Omission, and Error Semantics

- Calculation parity: IEEE 754 `f64` arithmetic (`-0.0` normalized to `0.0`), Excel scalar type coercion, criteria wildcards, and domain error propagation. Excludes dynamic array spilling (`#SPILL!`), implicit intersection, and volatile functions (`TODAY()`, `RAND()`).
- Compile-time validation (raises `F8Error` during `model.compile()` Pass 4 when the call is evaluated; skipped inside untaken lazy branches):
  - Unknown function name (`UnknownFunction`).
  - Argument count below minimum or above maximum (`TooFewArguments`, `TooManyArguments`).
  - Odd trailing argument count in pair-variadic functions (`IFS`, `SUMIFS`, `COUNTIFS`, `AVERAGEIFS`, `MINIFS`, `MAXIFS` -> `InvalidVariadicPairs`).
  - Omitting a required parameter with `_` (`CannotOmitRequiredArgument`).
- Argument omission (`_`):
  - Optional positional parameters can be omitted via `_` (`PMT(rate, nper, pv, _, 1)` or `XLOOKUP(k, look, ret, _, -1)`). Omitting a required parameter with `_` fails at compile time (`CannotOmitRequiredArgument`).
  - Omitted parameters bind to their declared default value (or remain omitted for lazy fallback parameters like `XLOOKUP`'s `if_not_found`).
  - In lowered Excel formulas, `_` emits an empty positional argument (`=PMT(C5,C6,C7,,1)`).
- Runtime Excel error values (returned as cell values and propagated through dependent formulas):
  - `#DIV/0!`: `x / 0`, `0 ^ -k`, `AVERAGE` on range with no numbers, `AVERAGEIFS` with zero matches, `r = -1.0` in `NPV`, or `nper = 0` in `PMT`/`PPMT`.
  - `#VALUE!`: Multi-cell range slice evaluated in scalar context (`1 + Table["r1".."r2", @col]`), uncoercible text scalar in numeric operation, range dimension mismatch in `SUMPRODUCT`/`*IFS`/`XLOOKUP`, 2D matrix passed to `XLOOKUP`/`XMATCH`, omitting `col_num` on 2D matrix in `INDEX`, combining wildcard `match_mode = 2` with binary `search_mode = 2` or `-2`, `index_num < 1` or `> N` in `CHOOSE`, or zero logical inputs in `AND`/`OR`.
  - `#REF!`: Out-of-bounds `row_num` or `col_num` (`< 1` or exceeding slice dimensions) in `INDEX`.
  - `#NUM!`: Negative base with fractional exponent (`(-2) ^ 0.5`), `NaN`/`Inf` overflow, `MEDIAN` with zero numeric inputs, `IRR`/`XIRR` with `< 2` cash flows, missing sign alternation, or non-convergence, `XNPV` with `rate <= -1.0`, unequal array lengths or dates preceding the initial date (`d_i < d_1`) in `XNPV`/`XIRR`, `per < 1` or `per > nper` in `IPMT`/`PPMT` (or `nper <= 0` in `IPMT`), `basis` outside `0..=4` in `YEARFRAC`, or date serial outside `0.0..=2958465.0`.
  - `#N/A`: Unmatched lookup in `XLOOKUP` (without `if_not_found`) or `XMATCH`, no true condition in `IFS`, or unmatched value in `SWITCH` (without `default`).

---

## 2. Math and Aggregation Functions (17)

| Function     | Signature                                                                                  | Semantics                                                                                                                                                     |
| :----------- | :----------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `ABS`        | `ABS(number)`                                                                              | Absolute value. Text scalars return `#VALUE!`; booleans coerce to `1.0`/`0.0`; empty cells coerce to `0.0`.                                                   |
| `SUM`        | `SUM(number1, [number2], ...)`                                                             | Sums numbers across scalars and range slices. Text, booleans, and empty cells in cell/range references are ignored; direct non-reference booleans coerce (`1.0`/`0.0`); direct uncoercible text returns `#VALUE!`. |
| `AVERAGE`    | `AVERAGE(number1, [number2], ...)`                                                         | Arithmetic mean. Text, booleans, and empty cells in cell/range references are ignored; returns `#DIV/0!` if count of numeric values is 0.                    |
| `COUNT`      | `COUNT(value1, [value2], ...)`                                                             | Counts numeric values (ignores errors). In cell/range references, only numbers are counted; in direct non-reference scalar arguments, numbers and booleans count. |
| `COUNTA`     | `COUNTA(value1, [value2], ...)`                                                            | Counts non-empty cells (numbers, text including `""`, booleans, errors). Ignores only `Empty` (`None`) cells.                                                 |
| `MAX`        | `MAX(number1, [number2], ...)`                                                             | Maximum numeric value (`0.0` if no numeric values are present). Ignores non-numbers in cell/range references; direct uncoercible text returns `#VALUE!`.     |
| `MIN`        | `MIN(number1, [number2], ...)`                                                             | Minimum numeric value (`0.0` if no numeric values are present). Ignores non-numbers in cell/range references; direct uncoercible text returns `#VALUE!`.     |
| `MEDIAN`     | `MEDIAN(number1, [number2], ...)`                                                          | Median of numeric inputs (averages middle two on even count). Ignores non-numbers in cell/range references; returns `#NUM!` if no numeric values are present. |
| `ROUND`      | `ROUND(number, num_digits)`                                                                | Commercial rounding (half-away-from-zero) to `trunc(num_digits)` decimal places. Negative `num_digits` rounds left of the decimal point.                      |
| `ROUNDUP`    | `ROUNDUP(number, num_digits)`                                                              | Rounds away from zero to `trunc(num_digits)` decimal places.                                                                                                  |
| `ROUNDDOWN`  | `ROUNDDOWN(number, num_digits)`                                                            | Truncates toward zero to `trunc(num_digits)` decimal places.                                                                                                  |
| `SUMPRODUCT` | `SUMPRODUCT(array1, [array2], ...)`                                                        | Element-wise product and sum across equal-dimension 1D/2D ranges (no scalar broadcasting across multi-cell ranges; mismatched dimensions return `#VALUE!`). Non-numeric cells in ranges are treated as `0.0`. |
| `SUMIFS`     | `SUMIFS(sum_range, criteria_range1, criteria1, [criteria_range2, criteria2], ...)`         | Sums numeric cells in `sum_range` matching all `(criteria_range, criteria)` pairs. Returns `0.0` if no cells match; mismatched range dimensions return `#VALUE!`. |
| `COUNTIFS`   | `COUNTIFS(criteria_range1, criteria1, [criteria_range2, criteria2], ...)`                  | Counts cells matching all `(criteria_range, criteria)` pairs. Returns `0.0` if no cells match; mismatched range dimensions return `#VALUE!`.                  |
| `AVERAGEIFS` | `AVERAGEIFS(average_range, criteria_range1, criteria1, [criteria_range2, criteria2], ...)` | Mean of numeric cells in `average_range` matching all criteria pairs. Returns `#DIV/0!` if no numeric cells match; mismatched dimensions return `#VALUE!`.    |
| `MINIFS`     | `MINIFS(min_range, criteria_range1, criteria1, [criteria_range2, criteria2], ...)`         | Minimum numeric value in `min_range` matching all criteria pairs (`0.0` if no numeric cells match; `#VALUE!` on dimension mismatch).                          |
| `MAXIFS`     | `MAXIFS(max_range, criteria_range1, criteria1, [criteria_range2, criteria2], ...)`         | Maximum numeric value in `max_range` matching all criteria pairs (`0.0` if no numeric cells match; `#VALUE!` on dimension mismatch).                          |

### Conditional Aggregation Criteria (`*IFS`)

Each `criteria` argument accepts a number, boolean, string literal, or single-cell coordinate reference (note: `&` string concatenation is not supported in the formula grammar):

- Relational operator strings: Prefix with `">="`, `"<="`, `"<>"`, `">"`, `"<"`, or `"="` followed by a numeric, boolean, error, or text target (`">0"`, `">=100"`, `"<>0"`, `"> 10"`). Relational `<, <=, >, >=` with a numeric target match only numeric cells (text cells evaluate to false), whereas numeric `=` and `<>` also match text cells in `criteria_range` that parse to the target number, and boolean `=` and `<>` (`TRUE`, `"TRUE"`, `"=FALSE"`) also match text cells `"true"`/`"false"` (case-insensitive).
- Exact matching: Direct number (`10` or `"10"`), boolean (`TRUE` or `"TRUE"`), error (`"#DIV/0!"`), or ASCII case-insensitive text (`"Active"`, `"=Approved"`). Plain strings with leading or trailing spaces (`" 10"`, `"  DeptA"`) preserve whitespace and match strictly as text.
- Blank and non-blank sentinels: `""` or `"="` matches empty cells (`None` or `""`); `"<>"` matches non-blank cells (`"<> "` matches any cell except a single space `" "`). A direct reference to an empty cell (`None`) used as a `criteria` argument evaluates to numeric `0` (`=0`).
- Wildcards (`=` and `<>` text criteria): `?` matches any single character, `*` matches zero or more characters (`"Tech*"`, `"?est"`). Escape literal characters via `~?`, `~*`, `~~`.
- Conditional error bubbling: Errors in `criteria_range` are ignored unless matched by an error criterion; errors in target aggregation ranges (`sum_range`, `average_range`, `min_range`, `max_range`) propagate only for cells that satisfy all criteria.

---

## 3. Logical and Branching Functions (9)

| Function  | Signature                                                                  | Semantics                                                                                                                                                    |
| :-------- | :------------------------------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `IF`      | `IF(logical_test, value_if_true, [value_if_false])`                        | Short-circuiting branch: evaluates only the taken branch (referencing an `Empty` cell in a taken branch preserves `Empty`). `value_if_true` is required; `value_if_false` defaults to `FALSE` (whether omitted or passed `_`). Numbers coerce (`0.0` -> false, non-zero -> true). |
| `IFS`     | `IFS(logical_test1, value_if_true1, [logical_test2, value_if_true2], ...)` | Evaluates `(test, value)` pairs left-to-right with short-circuiting (referencing an `Empty` cell preserves `Empty`). Requires even argument count (`>= 2`). Returns `#N/A` if no condition is true. |
| `IFERROR` | `IFERROR(value, value_if_error)`                                           | Short-circuiting error trap: evaluates and returns `value_if_error` only if `value` evaluates to any Excel error (referencing an `Empty` cell preserves `Empty`). |
| `IFNA`    | `IFNA(value, value_if_na)`                                                 | Short-circuiting `#N/A` trap: evaluates `value_if_na` only if `value` is `#N/A` (referencing an `Empty` cell preserves `Empty`); other errors propagate without evaluating `value_if_na`. |
| `AND`     | `AND(logical1, [logical2], ...)`                                           | Logical conjunction across scalars and ranges. Cell/range references ignore text and empty cells; direct scalar arguments coerce `"TRUE"`/`"FALSE"` (case-insensitive) to `bool`, `Empty` to `FALSE`, and return `#VALUE!` on other strings. Returns `#VALUE!` if no logical/numeric values are present. |
| `OR`      | `OR(logical1, [logical2], ...)`                                            | Logical disjunction across scalars and ranges. Cell/range references ignore text and empty cells; direct scalar arguments coerce `"TRUE"`/`"FALSE"` (case-insensitive) to `bool`, `Empty` to `FALSE`, and return `#VALUE!` on other strings. Returns `#VALUE!` if no logical/numeric values are present. |
| `NOT`     | `NOT(logical)`                                                             | Logical negation (`NOT(TRUE)` -> `FALSE`, `NOT(0)` -> `TRUE`, `Empty` coerces to `FALSE` so `NOT` returns `TRUE`). Uncoercible text scalars return `#VALUE!`. |
| `CHOOSE`  | `CHOOSE(index_num, value1, [value2], ...)`                                 | Evaluates and returns choice at 1-based `trunc(index_num)` with short-circuiting (referencing an `Empty` cell preserves `Empty`). Returns `#VALUE!` if `index_num < 1` or `> N`. |
| `SWITCH`  | `SWITCH(expression, value1, result1, [value2, result2], ..., [default])`   | Matches `expression` against `value_i` using case-insensitive Excel equality, short-circuiting the matched `result_i` (referencing an `Empty` cell preserves `Empty`). Odd trailing argument acts as `default`; returns `#N/A` if unmatched without `default`. |

---

## 4. Financial and Discounting Functions (7)

| Function | Signature                                 | Semantics                                                                                                                                                                                                                           |
| :------- | :---------------------------------------- | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `NPV`    | `NPV(rate, value1, [value2], ...)`        | Periodic net present value starting at `t = 1`: `sum(CF_t / (1 + rate)^t)`. Range text, booleans, and empty cells are ignored; direct scalar arguments coerce booleans (`1.0`/`0.0`) and empty cells (`0.0`), and return `#VALUE!` on uncoercible text. Returns `#DIV/0!` if `rate = -1.0` (or denominator underflows to `0.0`); `rate < -1.0` is allowed (integer exponents); float overflow returns `#NUM!`. |
| `XNPV`   | `XNPV(rate, values, dates)`               | Exact-date net present value using Actual/365: `sum(CF_i / (1 + rate)^((d_i - d_1) / 365))`. Coerces all cells in `values` and `dates` (`Empty` -> `0.0`, booleans -> `1.0`/`0.0`, text -> `#VALUE!`). Requires `rate > -1.0`, equal array lengths (`>= 1`), and `trunc(d_i) >= trunc(d_1)` (violations return `#NUM!`). |
| `IRR`    | `IRR(values, [guess])`                    | Periodic internal rate of return via Newton-Raphson (max 100 iterations, tolerance `1e-9`, default `guess = 0.10`, clamped `> -1.0`). Range cells ignore empty/text/boolean cells; direct scalar arguments coerce booleans/empty and return `#VALUE!` on text. Requires `>= 2` cash flows with at least one positive and one negative value, else `#NUM!`. |
| `XIRR`   | `XIRR(values, dates, [guess])`            | Exact-date internal rate of return using Actual/365 (max 100 iterations, tolerance `1e-9`, default `guess = 0.10`, clamped `> -1.0`). Coerces all cells in `values` and `dates` (`Empty` -> `0.0`, booleans -> `1.0`/`0.0`, text -> `#VALUE!`). Requires equal array lengths (`>= 2`), `trunc(d_i) >= trunc(d_1)`, and sign alternation, else `#NUM!`. |
| `PMT`    | `PMT(rate, nper, pv, [fv], [type])`       | Constant annuity payment (`fv` default `0.0`; `type` default `0` for end-of-period, non-zero for beginning-of-period). Supports `rate = 0.0`; returns `#DIV/0!` if `nper = 0.0`.                                                    |
| `IPMT`   | `IPMT(rate, per, nper, pv, [fv], [type])` | Interest component of annuity payment in period `per` (`fv` default `0.0`, `type` default `0`). Returns `0.0` when `rate = 0.0`; returns `#NUM!` if `per < 1`, `per > nper`, or `nper <= 0`.                                         |
| `PPMT`   | `PPMT(rate, per, nper, pv, [fv], [type])` | Principal component of annuity payment in period `per` (`PMT - IPMT`; `fv` default `0.0`, `type` default `0`). Returns `#DIV/0!` if `nper = 0.0`; returns `#NUM!` if `per < 1` or `per > nper`.                                     |

---

## 5. Lookup and Reference Functions (3)

| Function  | Signature                                                                                        | Semantics                                                                                                                                                                                                                                                            |
| :-------- | :----------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `XLOOKUP` | `XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode])` | Searches 1D `lookup_array` for `lookup_value` and returns corresponding element from equal-length 1D `return_array` (orientation-independent; preserves `Empty` if matched cell is empty). Default `match_mode = 0`, `search_mode = 1`. Unmatched lookup returns `if_not_found` (lazily evaluated) or `#N/A` if omitted. 2D arrays or length mismatches return `#VALUE!`. |
| `XMATCH`  | `XMATCH(lookup_value, lookup_array, [match_mode], [search_mode])`                                | Returns 1-based index of `lookup_value` in 1D `lookup_array` (default `match_mode = 0`, `search_mode = 1`). Returns `#N/A` if unmatched; 2D arrays return `#VALUE!`.                                                                                                 |
| `INDEX`   | `INDEX(array, row_num, [col_num])`                                                               | Returns element at 1-based `trunc(row_num)` and `trunc(col_num)` within a 1D or 2D range (preserves `Empty` if target cell is empty). On 1D vectors, omitting `col_num` (or passing `1`) indexes along the vector axis; on 2D matrices, omitting `col_num` returns `#VALUE!`. Out-of-bounds indices return `#REF!`. |

### `match_mode` and `search_mode` (`XLOOKUP`, `XMATCH`)

- `match_mode`: `0` (default) = exact match (ASCII case-insensitive for text; strict type isolation so numeric `10` never matches text `"10"`, and `Empty` matches only `Empty`); `-1` = exact or next smaller; `1` = exact or next larger; `2` = wildcard match (`*`, `?`, `~`).
- `search_mode`: `1` (default) = first-to-last; `-1` = last-to-first; `2` = binary search ascending; `-2` = binary search descending. Combining `match_mode = 2` with `search_mode = 2` or `-2` returns `#VALUE!`.

---

## 6. Date and Time Functions (8)

| Function   | Signature                                 | Semantics                                                                                                                                                                                                                    |
| :--------- | :---------------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DATE`     | `DATE(year, month, day)`                  | Constructs Excel serial date (`0.0 <= serial <= 2958465.0`, corresponding to `1900-01-00`..`9999-12-31`). Years `0 <= y <= 1899` map to `y + 1900`. Normalizes month/day rollovers and preserves Excel's 1900 leap-year bug (`1900-02-29` -> `60.0`). Out-of-range dates return `#NUM!`. |
| `EDATE`    | `EDATE(start_date, months)`               | Shifts `start_date` by `trunc(months)` calendar months, clamping end-of-month overflow to the final valid day of the target month.                                                                                           |
| `EOMONTH`  | `EOMONTH(start_date, months)`             | Returns serial date for the last day of the month `trunc(months)` before or after `start_date`.                                                                                                                              |
| `YEARFRAC` | `YEARFRAC(start_date, end_date, [basis])` | Year fraction between `start_date` and `end_date` (order-independent; truncates fractional time). Default `basis = 0`. `basis` outside `0..=4` returns `#NUM!`.                                                              |
| `YEAR`     | `YEAR(serial_number)`                     | Extracts 4-digit year (`1900..=9999`) from serial date.                                                                                                                                                                      |
| `MONTH`    | `MONTH(serial_number)`                    | Extracts month (`1..=12`) from serial date.                                                                                                                                                                                  |
| `DAY`      | `DAY(serial_number)`                      | Extracts day of month (`1..=31`) from serial date.                                                                                                                                                                           |
| `DAYS`     | `DAYS(end_date, start_date)`              | Signed day difference `trunc(end_date) - trunc(start_date)`. Note parameter order: `end_date` first, `start_date` second.                                                                                                    |

### `YEARFRAC` Day-Count Basis (`0..=4`)

| `basis`          | Convention            | Denominator / Day-Count Rule                                                                 |
| :--------------- | :-------------------- | :------------------------------------------------------------------------------------------- |
| `0` (or omitted) | US (NASD) 30/360      | 30-day months / 360-day year with US end-of-February and 31st-of-month adjustment rules      |
| `1`              | Actual/Actual (AFB)   | Exact elapsed days divided by actual days in year (`365` or `366` based on leap-year rules)  |
| `2`              | Actual/360            | Exact elapsed days divided by `360.0`                                                        |
| `3`              | Actual/365            | Exact elapsed days divided by `365.0`                                                        |
| `4`              | European 30/360       | 30-day months / 360-day year where any 31st day of month is clamped to `30`                  |
