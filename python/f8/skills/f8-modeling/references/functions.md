# Function Reference

Signatures, parameter defaults, compile-time type/shape rules, argument omission (`_`), composition patterns for omitted functions, and domain error modes for the 37 built-in Excel functions in `f8`.

---

## 1. Call Rules, Compile-Time Validation, Argument Omission, and Error Semantics

- Calculation parity: IEEE 754 `f64` arithmetic (`-0.0` normalized to `0.0`, non-finite overflow mapped to `#NUM!`), compile-time type/shape verification, and Excel domain error propagation. Excludes dynamic array spilling (`#SPILL!`), implicit intersection, wildcards, and volatile functions (`TODAY()`, `RAND()`).
- Compile-time validation (raises `F8Error` during `model.compile()` before `.xlsx` emission; static type, shape, and role checks cover both taken and untaken lazy branches in `IF`, `IFERROR`, `CHOOSE`, and `XLOOKUP`, while empty-cell checks run on evaluated paths):
  - Signatures & omission: Unknown function names, arity violations, and invalid `_` placeholders raise `F8Error` (required-argument omission is rejected before trailing-argument omission).
  - Scalar, logical, relational, and empty rules: Unary `-`, binary arithmetic, and numeric parameters require `Number` (rejecting `Text`, `Boolean`, and empty). Logical conditions (`IF`, `NOT`, and `AND`/`OR`) require `Boolean`. Relational operators (`=`, `<>`, `<`, `<=`, `>`, `>=`) require non-empty operands of identical type. Formula cells cannot evaluate to an empty cell at the root.
  - Aggregation role & empty policies: `SUM`, `AVERAGE`, `MIN`, `MAX`, `MEDIAN`, `COUNT`, `SUMPRODUCT`, `NPV`, `IRR`, `XNPV`, and `XIRR` reject `descriptor` columns, `Text`, `Boolean`, and empty scalars or `1x1` cell references. `NPV`, `IRR`, `XNPV`, and `XIRR` also reject empty cells inside `start..end` slices (zero cash flows require explicit `0.0`), while math/logical aggregations (`SUM`, `AVERAGE`, `MIN`, `MAX`, `MEDIAN`, `COUNT`, `SUMPRODUCT`, `AND`, `OR`) skip sparse empty cells inside syntactic `start..end` slices (`0.0` in `SUM`/`SUMPRODUCT`).
  - Monomorphic branches: Result arms in 3-arg `IF`, `CHOOSE`, `IFERROR`, and `XLOOKUP` (`return_array` + `if_not_found`) must unify to a single scalar type and evaluate to non-empty values when selected. `NA()` acts as a polymorphic bottom (`#N/A`) unifying with any arm type; formulas producing runtime Excel errors preserve their static type downstream.
  - Range-only slots, slice geometry, and lookups: Syntactic `start..end` slices are forbidden in scalar contexts. Range-only slots (`COUNT`, `SUMPRODUCT`, `XLOOKUP`/`XMATCH`/`INDEX` arrays, `IRR`/`XNPV`/`XIRR` series) require `Table[...]` references. `SUMPRODUCT` and `XLOOKUP` require matching `(rows, cols)` dimensions; `XNPV` and `XIRR` require equal-length 1D ranges; `XLOOKUP`, `XMATCH`, `XNPV`, and `XIRR` require 1D ranges; 2D `INDEX` requires `col_num`; `lookup_array`/`return_array`/`INDEX` slices must be type-homogeneous; and `XLOOKUP`/`XMATCH`/`INDEX` reject empty lookup keys, `1x1` empty arrays, and selected empty return elements.
- Argument omission (`_`):
  - Intermediate optional parameters may be skipped via `_` when followed by at least one explicit argument (`PMT(rate, nper, pv, _, 1)`, `XLOOKUP(k, look, ret, _, -1)`, `XLOOKUP(k, look, ret, _, _, -1)`). Omitting a required parameter (`ABS(_)`, `IF(cond, t, _)`) or trailing optional parameter (`PMT(r, n, pv, _)`, `YEARFRAC(s, e, _)`) with `_` raises `F8Error`; omit trailing optionals directly (`PMT(r, n, pv)`, `YEARFRAC(s, e)`).
  - Intermediate `_` binds to the parameter's declared default (or omitted lazy `if_not_found` in `XLOOKUP`, returning `#N/A` on miss) and lowers to an empty Excel slot (`=PMT(C5,C6,C7,,1)`).
- Runtime Excel error values (stored as cell values and propagated through dependent formulas unless handled by `IFERROR` or inspected by `ISERROR`):
  - `#DIV/0!`: `x / 0`, `0 ^ -k`, `MOD(number, 0)`, `AVERAGE` on an all-empty slice, or `rate = -1.0` in `NPV`.
  - `#VALUE!`: Out-of-bounds `CHOOSE` `index_num` (`< 1` or `> N`), zero `Boolean` values across sparse `AND`/`OR` slices, unsupported `match_mode` (outside `{0, -1, 1}`, including wildcard `2`) or `search_mode` (outside `{1, -1, 2, -2}`) in `XLOOKUP`/`XMATCH`, or empty/unsorted `lookup_array` under binary search (`search_mode = 2` or `-2`; error cells inside binary-search arrays bubble their error).
  - `#REF!`: Out-of-bounds `INDEX` `row_num` or `col_num` (`< 1` or exceeding slice dimensions).
  - `#NUM!`: Negative base with fractional exponent (`(-2) ^ 0.5`), `0 ^ 0`, `LN(number <= 0)`, `EXP` overflow (`> ~709.78`), `MOD` with `|number / divisor| > 2^53 - 1` or non-finite quotient, `NaN`/`Inf` overflow, non-integer numeric arguments at discrete integer parameters (`ROUND`/`ROUNDUP`/`ROUNDDOWN` `num_digits`, `CHOOSE` `index_num`, `INDEX` `row_num`/`col_num`, `XLOOKUP`/`XMATCH` `match_mode`/`search_mode`, `DATE` `year`/`month`/`day`, `EDATE`/`EOMONTH` `months`, `YEARFRAC` `basis`, `PMT` `type`), invalid `DATE(year, month, day)` outside `1900..=9999` / `1..=12` / `1..=days_in_month` (`1900-02-29` -> `60.0` preserved; 2-digit years and rollovers rejected), input date serials outside `1.0..=2958465.0` (including `0`) across date functions and `XNPV`/`XIRR` `dates`, `PMT` `type` outside `{0, 1}`, `YEARFRAC` `basis` outside `0..=4`, `MEDIAN` on an all-empty slice, `IRR`/`XIRR` with `guess <= -1.0`, `< 2` cash flows, no sign alternation, or non-convergence, `XNPV` with `rate <= 0.0`, dates preceding the initial date (`trunc(d_i) < trunc(d_1)`) in `XNPV`/`XIRR`, `rate <= -1.0` or `nper = 0` in `PMT`, or `EDATE`/`EOMONTH` stepping outside `1900..=9999`.
  - `#N/A`: `NA()` or unmatched lookup in `XLOOKUP` (without `if_not_found`) or `XMATCH`.

---

## 2. Math and Aggregation Functions (14)

All numeric/aggregation parameters require `Number` (see Section 1 for `descriptor` and empty-cell policies).

| Function     | Signature                           | Semantics                                                                                                                                                                          |
| :----------- | :---------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ABS`        | `ABS(number)`                       | Absolute value of scalar `number`.                                                                                                                                                 |
| `LN`         | `LN(number)`                        | Natural logarithm of positive scalar `number` (`number <= 0` returns `#NUM!`).                                                                                                     |
| `EXP`        | `EXP(number)`                       | Exponential `e ^ number` (overflow above ~`709.78` returns `#NUM!`; large negative inputs underflow to `0.0`).                                                                     |
| `MOD`        | `MOD(number, divisor)`              | Floored remainder `number - divisor * floor(number / divisor)` matching Excel's divisor-sign rule (`divisor = 0` -> `#DIV/0!`; `abs(q) > 2^53 - 1` or non-finite `q` -> `#NUM!`).  |
| `SUM`        | `SUM(number1, [number2], ...)`      | Sums numeric scalars and `measure` references/slices (sparse empty cells in slices contribute `0.0`; overflow -> `#NUM!`).                                                         |
| `AVERAGE`    | `AVERAGE(number1, [number2], ...)`  | Arithmetic mean across numeric scalars and `measure` references/slices (skips sparse empty cells in slices; all-empty slice -> `#DIV/0!`).                                         |
| `COUNT`      | `COUNT(value1, [value2], ...)`      | Counts numeric cells across `Table[...]` `measure` references (ignores sparse empty cells in slices and runtime error cells).                                                      |
| `MAX`        | `MAX(number1, [number2], ...)`      | Maximum across numeric scalars and `measure` references/slices (skips sparse empty cells in slices; all-empty slice -> `0.0`).                                                     |
| `MIN`        | `MIN(number1, [number2], ...)`      | Minimum across numeric scalars and `measure` references/slices (skips sparse empty cells in slices; all-empty slice -> `0.0`).                                                     |
| `MEDIAN`     | `MEDIAN(number1, [number2], ...)`   | Median across numeric scalars and `measure` references/slices (skips sparse empty cells in slices; averages middle two on even count; all-empty slice -> `#NUM!`).                 |
| `ROUND`      | `ROUND(number, num_digits)`         | Base-10 15-significant-digit commercial rounding (half-away-from-zero) to integer `num_digits` (negative rounds left of decimal; non-integer `num_digits` or overflow -> `#NUM!`). |
| `ROUNDUP`    | `ROUNDUP(number, num_digits)`       | Base-10 15-significant-digit rounding away from zero to integer `num_digits` (non-integer `num_digits` or overflow -> `#NUM!`).                                                    |
| `ROUNDDOWN`  | `ROUNDDOWN(number, num_digits)`     | Base-10 15-significant-digit truncation toward zero to integer `num_digits` (non-integer `num_digits` -> `#NUM!`).                                                                 |
| `SUMPRODUCT` | `SUMPRODUCT(array1, [array2], ...)` | Element-wise product and sum across equal-dimension `(rows, cols)` 1D/2D `Table[...]` `measure` ranges (sparse empty cells in slices contribute `0.0`; overflow -> `#NUM!`).       |

### Composition Patterns for Omitted Conditional Aggregations

`COUNTA`, `SUMIFS`, `COUNTIFS`, `AVERAGEIFS`, `MINIFS`, and `MAXIFS` are omitted (`F8Error`). Equivalent grid patterns:

- Conditional sum: Helper row/column `IF(cond, value, 0)` + `SUM(...)`, or `0`/`1` flag row/column `IF(cond, 1, 0)` + `SUMPRODUCT(values, flags)`.
- Conditional count: `0`/`1` flag row/column `IF(cond, 1, 0)` (combining conditions via `AND`/`OR`) + `SUM(flags)`.
- Conditional average: `SUMPRODUCT(values, flags) / SUM(flags)` for finite values, or `SUM(zeroed_helper) / SUM(flags)` when excluded cells evaluate to `NA()` (`SUMPRODUCT` propagates `#N/A` even against `0`; evaluates to `#DIV/0!` when `SUM(flags) = 0`).
- Static subset `MIN` / `MAX` / `AVERAGE` / `MEDIAN`: `MIN`, `MAX`, `AVERAGE`, and `MEDIAN` skip empty (`None`) cells inside `start..end` slices, allowing 0D `None` overrides on excluded cells in a helper slice.

---

## 3. Logical and Branching Functions (6)

| Function  | Signature                                         | Semantics                                                                                                                                                                                             |
| :-------- | :------------------------------------------------ | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `IF`      | `IF(logical_test, value_if_true, value_if_false)` | Short-circuiting 3-argument branch (`value_if_false` required). `logical_test` requires `Boolean`; both arms must unify to one non-empty scalar type (`NA()` allowed).                                |
| `IFERROR` | `IFERROR(value, value_if_error)`                  | Short-circuiting runtime error trap: returns `value_if_error` only when `value` evaluates to a runtime Excel error (compile-time `F8Error`s propagate; arms must unify to one non-empty scalar type). |
| `AND`     | `AND(logical1, [logical2], ...)`                  | Logical conjunction across `Boolean` scalars, single-cell refs, and slices (skips sparse empty cells in slices; zero `Boolean` inputs -> `#VALUE!`).                                                  |
| `OR`      | `OR(logical1, [logical2], ...)`                   | Logical disjunction across `Boolean` scalars, single-cell refs, and slices (skips sparse empty cells in slices; zero `Boolean` inputs -> `#VALUE!`).                                                  |
| `NOT`     | `NOT(logical)`                                    | Logical negation of scalar `Boolean` (`NOT(TRUE)` -> `FALSE`, `NOT(FALSE)` -> `TRUE`).                                                                                                                |
| `CHOOSE`  | `CHOOSE(index_num, value1, [value2], ...)`        | Short-circuiting 1-based selection by integer `index_num` (all arms must unify to one non-empty scalar type). Non-integer -> `#NUM!`; out of `1..=N` -> `#VALUE!`.                                    |

---

## 4. Financial and Discounting Functions (5)

All cash-flow and date slices reject empty cells (`F8Error`; zero cash flows require explicit `0.0`) and `descriptor` columns.

| Function | Signature                           | Semantics                                                                                                                                                                                                                                                                   |
| :------- | :---------------------------------- | :-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `NPV`    | `NPV(rate, value1, [value2], ...)`  | Periodic NPV starting at `t = 1`: `sum(CF_t / (1 + rate)^t)`. `rate = -1.0` -> `#DIV/0!`; `rate < -1.0` allowed (integer exponents); overflow -> `#NUM!`.                                                                                                                   |
| `XNPV`   | `XNPV(rate, values, dates)`         | Exact-date NPV (Actual/365) over equal-length 1D `measure` ranges: `sum(CF_i / (1 + rate)^((d_i - d_1) / 365))`. Requires `rate > 0.0` (matching Excel's `#NUM!` for non-positive rates), date serials in `1.0..=2958465.0`, and `trunc(d_i) >= trunc(d_1)` (else `#NUM!`). |
| `IRR`    | `IRR(values, [guess])`              | Periodic IRR via safeguarded Newton-Brent root-finding (`values` is a `measure` range; default `guess = 0.10`). Returns `#NUM!` if `guess <= -1.0`, `< 2` cash flows, no sign alternation, or no real root `> -1.0`.                                                        |
| `XIRR`   | `XIRR(values, dates, [guess])`      | Exact-date IRR (Actual/365) over equal-length 1D `measure` ranges (default `guess = 0.10`). Returns `#NUM!` if `guess <= -1.0`, `< 2` cash flows, invalid date serials, `trunc(d_i) < trunc(d_1)`, no net sign alternation across distinct dates, or no root `> -1.0`.      |
| `PMT`    | `PMT(rate, nper, pv, [fv], [type])` | Constant annuity payment (default `fv = 0.0`, `type = 0` end-of-period or `1` beginning-of-period). Supports `rate = 0.0`; non-integer or invalid `type`, `rate <= -1.0`, or `nper = 0.0` -> `#NUM!`.                                                                       |

### Composition Pattern for Omitted Debt Amortization Functions

`IPMT` and `PPMT` are omitted (`F8Error`). Period-by-period interest and principal can be expressed with `PMT` and a 2D balance schedule (e.g. constant payment via `PMT(rate, nper, -opening_principal)`, `opening_balance` from `col.prev` `closing_balance`, `interest = opening_balance * rate`, `principal = scheduled_pmt - interest` or `MIN(opening_balance, scheduled_pmt - interest)`, `closing_balance = opening_balance - principal`).

---

## 5. Lookup and Reference Functions (3)

| Function  | Signature                                                                                        | Semantics                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| :-------- | :----------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `XLOOKUP` | `XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode])` | Searches homogeneous 1D `lookup_array` (`Table[...]` required) for non-empty `lookup_value` of matching type and returns the corresponding element from a homogeneous 1D `return_array` of matching `(rows, cols)` dimensions (both vertical or both horizontal; monomorphic with lazy `if_not_found`). Unselected empty cells in `start..end` slices are skipped in linear search; `1x1` empty arrays or selected empty return/`if_not_found` values raise `F8Error`. Default `match_mode = 0`, `search_mode = 1`; unmatched returns `if_not_found` or `#N/A`.     |
| `XMATCH`  | `XMATCH(lookup_value, lookup_array, [match_mode], [search_mode])`                                | Returns 1-based position of non-empty `lookup_value` in homogeneous 1D `lookup_array` (default `match_mode = 0`, `search_mode = 1`; skips sparse empty cells in linear search; `1x1` empty arrays raise `F8Error`; unmatched -> `#N/A`).                                                                                                                                                                                                                                                                                                                            |
| `INDEX`   | `INDEX(array, row_num, [col_num])`                                                               | Returns element at 1-based integer `row_num` / `col_num` in a homogeneous 1D or 2D `Table[...]` range (`1x1` empty `array` or selected empty element raises `F8Error`). On 1D vectors, omitting `col_num` indexes along the vector axis (`row_num` indexes rows on `N x 1` and columns on `1 x N`); when `col_num` is passed, `(row_num, col_num)` indexes 2D coordinates directly (so `col_num = 1` works on `N x 1`, while `1 x N` requires `INDEX(array, 1, col_num)`). On 2D matrices, `col_num` is required. Non-integer -> `#NUM!`; out-of-bounds -> `#REF!`. |

### `match_mode` and `search_mode` (`XLOOKUP`, `XMATCH`)

- Mode values: Both require `Number` (non-integer -> `#NUM!`; unsupported integer mode -> `#VALUE!`).
- `match_mode`: `0` (default) = exact match (ASCII case-insensitive for text); `-1` = exact or next smaller; `1` = exact or next larger (wildcard `2` is unsupported -> `#VALUE!`).
- `search_mode`: `1` (default) = first-to-last; `-1` = last-to-first; `2` = binary search ascending (first exact match on duplicates); `-2` = binary search descending (last exact match on duplicates); on approximate misses (`-1`/`1`), binary modes return the duplicate adjacent to the insertion point. Binary modes (`2`, `-2`) require every `lookup_array` cell to be non-empty, non-error, and monotonically sorted (`#VALUE!` on empty or unsorted keys; error cells bubble their error).

---

## 6. Date and Time Functions (7)

`DAYS` is omitted (`F8Error`); elapsed days are computed by subtracting date serials (`end_date - start_date`).

| Function   | Signature                                 | Semantics                                                                                                                                                                                                                                                                |
| :--------- | :---------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DATE`     | `DATE(year, month, day)`                  | Constructs Excel serial date (`1.0..=2958465.0`, `1900-01-01`..`9999-12-31`) from integer `year` (`1900..=9999`), `month` (`1..=12`), and `day` (`1..=days_in_month`, preserving Excel's `1900-02-29` -> `60.0`; non-integers, 2-digit years, and rollovers -> `#NUM!`). |
| `EDATE`    | `EDATE(start_date, months)`               | Shifts `start_date` (`1.0..=2958465.0`) by integer `months`, clamping month-end overflow to the last valid Gregorian day (`1900-02` clamps to `1900-02-28` -> `59.0`; invalid inputs or stepping outside `1900..=9999` -> `#NUM!`).                                      |
| `EOMONTH`  | `EOMONTH(start_date, months)`             | Returns serial date for the last Gregorian day of the month integer `months` from `start_date` (`1900-02` -> `59.0`; invalid inputs or stepping outside `1900..=9999` -> `#NUM!`).                                                                                       |
| `YEARFRAC` | `YEARFRAC(start_date, end_date, [basis])` | Order-independent year fraction between `start_date` and `end_date` (`1.0..=2958465.0`, truncates fractional time). Default integer `basis = 0` in `0..=4` (invalid serial or `basis` -> `#NUM!`).                                                                       |
| `YEAR`     | `YEAR(serial_number)`                     | Extracts 4-digit year (`1900..=9999`) from `serial_number` (`1.0..=2958465.0`, else `#NUM!`).                                                                                                                                                                            |
| `MONTH`    | `MONTH(serial_number)`                    | Extracts month (`1..=12`) from `serial_number` (`1.0..=2958465.0`, else `#NUM!`).                                                                                                                                                                                        |
| `DAY`      | `DAY(serial_number)`                      | Extracts day of month (`1..=31`) from `serial_number` (`1.0..=2958465.0`, else `#NUM!`).                                                                                                                                                                                 |

### `YEARFRAC` Day-Count Basis (`0..=4`)

| `basis`          | Convention       | Denominator / Day-Count Rule                                                                                             |
| :--------------- | :--------------- | :----------------------------------------------------------------------------------------------------------------------- |
| `0` (or omitted) | US (NASD) 30/360 | 30-day months / 360-day year with US end-of-February and 31st-of-month adjustment rules                                  |
| `1`              | Actual/Actual    | Exact elapsed days divided by actual days in year (`365`/`366` within 1 year; average days/year across multi-year spans) |
| `2`              | Actual/360       | Exact elapsed days divided by `360.0`                                                                                    |
| `3`              | Actual/365       | Exact elapsed days divided by `365.0`                                                                                    |
| `4`              | European 30/360  | 30-day months / 360-day year where any 31st day of month is clamped to `30`                                              |

---

## 7. Information Functions (2)

| Function  | Signature        | Semantics                                                                                                                                                                                                                                                   |
| :-------- | :--------------- | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ISERROR` | `ISERROR(value)` | Returns `TRUE` if scalar `value` evaluates to any runtime Excel error (`#DIV/0!`, `#VALUE!`, `#REF!`, `#NAME?`, `#NUM!`, `#N/A`), and `FALSE` for `Number`, `Text`, and `Boolean` (empty raises `F8Error`; compile-time `F8Error`s still fail compilation). |
| `NA`      | `NA()`           | Returns `#N/A` with polymorphic bottom type, unifying with any typed branch arm (e.g. `IF(ebitda > 0, ev / ebitda, NA())`).                                                                                                                                 |
