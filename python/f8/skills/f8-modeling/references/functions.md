# Function Reference

Signatures, parameter defaults, compile-time type/shape rules, argument omission (`_`), criteria matching rules, and domain error modes for the 45 built-in Excel functions in `f8`.

---

## 1. Call Rules, Compile-Time Validation, Argument Omission, and Error Semantics

- Calculation parity: IEEE 754 `f64` arithmetic (`-0.0` normalized to `0.0`, non-finite overflow mapped to `#NUM!`), strict compile-time type/shape verification, criteria wildcards, and Excel domain error propagation. Excludes dynamic array spilling (`#SPILL!`), implicit intersection, and volatile functions (`TODAY()`, `RAND()`).
- Two-stage compile-time validation (raises `F8Error` during `model.compile()` Pass 4 before `.xlsx` emission, covering both active and untaken lazy branches in `IF`, `IFS`, `IFERROR`, `IFNA`, `CHOOSE`, and `XLOOKUP`):
  - Signature & omission rules: Unknown function name (`UnknownFunction`), arity bounds (`TooFewArguments`, `TooManyArguments`), pair-variadic counts (`IFS`, `SUMIFS`, `COUNTIFS`, `AVERAGEIFS`, `MINIFS`, `MAXIFS` -> `InvalidVariadicPairs`), and `_` omission (`CannotOmitRequiredArgument` checked before `CannotOmitTrailingArgument`).
  - Scalar, logical, relational, and root `Empty` rules: Unary `-`, binary arithmetic, and numeric scalar parameters require `Number` (rejecting `Text`, `Boolean`, and `Empty`); logical conditions (`IF`, `IFS`, `NOT`, and non-slice `AND`/`OR` arguments including `1x1` single-cell references) require `Boolean`; relational operators (`=`, `<>`, `<`, `<=`, `>`, `>=`) require non-`Empty` operands of identical type; formula cells evaluating to `Empty` at the cell root fail with `EmptyFormulaResult` (guard nullable cells via `ISBLANK(ref)`).
  - Parameter-boundary integer rules: Discrete integer parameters (`ROUND`/`ROUNDUP`/`ROUNDDOWN` `num_digits`, `CHOOSE` `index_num`, `INDEX` `row_num`/`col_num`, `DATE` `year`/`month`/`day`, `EDATE`/`EOMONTH` `months`, `YEARFRAC` `basis`, and `PMT`/`IPMT`/`PPMT` `type`) enforce `Integer` strictly at the declaring parameter boundary across direct signed literals, dormant cell references, and evaluated argument values (`NonIntegerArgument` on fractional floats or magnitudes exceeding `9007199254740991`).
  - Monomorphic branches & error `ValueType` provenance: Result arms in `IF` (all 3 arguments required), `IFS`, `CHOOSE`, `IFERROR`, `IFNA` (non-`Empty` arms required), and `XLOOKUP` (`return_array` + `if_not_found`) must unify to a single scalar type (`IncompatibleBranchTypes`). `NA()` acts as polymorphic bottom (`#N/A`) for dynamic non-meaningful ratios, while errored expressions, single-cell references, and range slices preserve their inferred `ValueType` provenance.
  - Range-only slots, slice geometry, and lookup invariants: Syntactic `start..end` slices in scalar contexts fail with `SliceInScalarContext`; range-only slots (`COUNT`, `COUNTA`, `SUMPRODUCT`, `*IFS` range slots, `XLOOKUP`/`XMATCH`/`INDEX` arrays, `IRR`/`XNPV`/`XIRR` series) require `Table[...]` references (`InvalidArgumentKind`); mismatched range dimensions/lengths (`SUMPRODUCT`, `*IFS`, `XLOOKUP`, `XNPV`, `XIRR`), 2D matrices in 1D slots (`XLOOKUP`, `XMATCH`, `XNPV`, `XIRR`), omitted `col_num` on 2D `INDEX`, constant out-of-bounds `CHOOSE`/`INDEX` indices, non-literal or wildcard (`2`) `match_mode`/`search_mode`, `Empty` `lookup_value`, heterogeneous `lookup_array`/`return_array`/`INDEX` slices, and unsorted or `Empty`/`Error`-containing binary-search arrays fail compilation with `F8Error`.
- Argument omission (`_`):
  - Intermediate optional positional parameters can be skipped via `_` when followed by at least one explicit argument (`PMT(rate, nper, pv, _, 1)`, `XLOOKUP(k, look, ret, _, -1)`, `XLOOKUP(k, look, ret, _, _, -1)`). Omitting a required parameter (`ABS(_)`, `IF(cond, t, _)`) or trailing optional parameter (`PMT(r, n, pv, _)`, `YEARFRAC(s, e, _)`) with `_` fails at compile time (`CannotOmitRequiredArgument` / `CannotOmitTrailingArgument`); omit trailing optional arguments directly (`PMT(r, n, pv)`, `YEARFRAC(s, e)`).
  - Intermediate `_` slots bind to their declared default value (or `FunctionArg::Omitted` for `XLOOKUP`'s lazy `if_not_found`, returning `#N/A` on miss); absent trailing optional arguments are not synthetically padded.
  - In lowered Excel formulas, intermediate `_` emits an empty positional argument (`=PMT(C5,C6,C7,,1)`, `=XLOOKUP(..., ..., ..., , , -1)`).
- Runtime Excel error values (returned as cell values and propagated through dependent formulas):
  - `#DIV/0!`: `x / 0`, `0 ^ -k`, `AVERAGE` on range with no numbers, `AVERAGEIFS` with zero matches, `r = -1.0` in `NPV`, or `nper = 0` in `PMT`/`PPMT`.
  - `#VALUE!`: Uncoercible direct text scalar in `SUM`/`AVERAGE`/`MIN`/`MAX`/`MEDIAN`/`NPV`/`IRR` or text cell in `XNPV`/`XIRR`, dynamic `index_num < 1` or `> N` in `CHOOSE`, or zero `Boolean` values across sparse `AND`/`OR` range slices.
  - `#REF!`: Dynamic out-of-bounds `row_num` or `col_num` (`< 1` or exceeding slice dimensions) in `INDEX`.
  - `#NUM!`: Negative base with fractional exponent (`(-2) ^ 0.5`), `NaN`/`Inf` overflow, `MEDIAN` with zero numeric inputs, `IRR`/`XIRR` with `< 2` cash flows, missing sign alternation, or non-convergence, `XNPV` with `rate <= -1.0`, dates preceding the initial date (`d_i < d_1`) in `XNPV`/`XIRR`, `per < 1` or `per > nper` in `IPMT`/`PPMT` (or `nper <= 0` in `IPMT`), `basis` outside `0..=4` in `YEARFRAC`, or date serial outside `0.0..=2958465.0`.
  - `#N/A`: `NA()`, unmatched lookup in `XLOOKUP` (without `if_not_found`) or `XMATCH`, or no true condition in `IFS`.

---

## 2. Math and Aggregation Functions (17)

| Function     | Signature                                                                                  | Semantics                                                                                                                                                     |
| :----------- | :----------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `ABS`        | `ABS(number)`                                                                              | Absolute value of a `Number` scalar (`Text`, `Boolean`, and `Empty` raise `F8Error`).                                                                         |
| `SUM`        | `SUM(number1, [number2], ...)`                                                             | Sums numbers across scalars and range slices. Text, booleans, and empty cells in cell/range references are ignored; direct non-reference booleans coerce (`1.0`/`0.0`); direct uncoercible text returns `#VALUE!`. |
| `AVERAGE`    | `AVERAGE(number1, [number2], ...)`                                                         | Arithmetic mean. Text, booleans, and empty cells in cell/range references are ignored; returns `#DIV/0!` if count of numeric values is 0.                    |
| `COUNT`      | `COUNT(value1, [value2], ...)`                                                             | Counts numeric cells across cell/range references (`Table[...]` required; ignores errors, text, booleans, and empty cells).                                   |
| `COUNTA`     | `COUNTA(value1, [value2], ...)`                                                            | Counts non-empty cells across cell/range references (`Table[...]` required; counts numbers, text including `""`, booleans, and errors; ignores `Empty`/`None`). |
| `MAX`        | `MAX(number1, [number2], ...)`                                                             | Maximum numeric value (`0.0` if no numeric values are present). Ignores non-numbers in cell/range references; direct uncoercible text returns `#VALUE!`.     |
| `MIN`        | `MIN(number1, [number2], ...)`                                                             | Minimum numeric value (`0.0` if no numeric values are present). Ignores non-numbers in cell/range references; direct uncoercible text returns `#VALUE!`.     |
| `MEDIAN`     | `MEDIAN(number1, [number2], ...)`                                                          | Median of numeric inputs (averages middle two on even count). Ignores non-numbers in cell/range references; returns `#NUM!` if no numeric values are present. |
| `ROUND`      | `ROUND(number, num_digits)`                                                                | Commercial rounding (half-away-from-zero) of `Number` `number` to `Integer` `num_digits` decimal places (negative `num_digits` rounds left of decimal point; non-integer `num_digits` raises `F8Error`). |
| `ROUNDUP`    | `ROUNDUP(number, num_digits)`                                                              | Rounds `Number` `number` away from zero to `Integer` `num_digits` decimal places (`F8Error` on non-integer `num_digits`).                                     |
| `ROUNDDOWN`  | `ROUNDDOWN(number, num_digits)`                                                            | Truncates `Number` `number` toward zero to `Integer` `num_digits` decimal places (`F8Error` on non-integer `num_digits`).                                     |
| `SUMPRODUCT` | `SUMPRODUCT(array1, [array2], ...)`                                                        | Element-wise product and sum across equal-dimension 1D/2D `Table[...]` ranges (non-reference scalars or mismatched dimensions raise `F8Error`). Non-numeric cells in ranges are treated as `0.0`. |
| `SUMIFS`     | `SUMIFS(sum_range, criteria_range1, criteria1, [criteria_range2, criteria2], ...)`         | Sums numeric cells in `sum_range` matching all `(criteria_range, criteria)` pairs. Returns `0.0` if no cells match; non-reference ranges or mismatched range dimensions raise `F8Error`. |
| `COUNTIFS`   | `COUNTIFS(criteria_range1, criteria1, [criteria_range2, criteria2], ...)`                  | Counts cells matching all `(criteria_range, criteria)` pairs. Returns `0.0` if no cells match; non-reference ranges or mismatched range dimensions raise `F8Error`. |
| `AVERAGEIFS` | `AVERAGEIFS(average_range, criteria_range1, criteria1, [criteria_range2, criteria2], ...)` | Mean of numeric cells in `average_range` matching all criteria pairs. Returns `#DIV/0!` if no numeric cells match; non-reference ranges or mismatched dimensions raise `F8Error`. |
| `MINIFS`     | `MINIFS(min_range, criteria_range1, criteria1, [criteria_range2, criteria2], ...)`         | Minimum numeric value in `min_range` matching all criteria pairs (`0.0` if no numeric cells match; `F8Error` on non-reference ranges or dimension mismatch).  |
| `MAXIFS`     | `MAXIFS(max_range, criteria_range1, criteria1, [criteria_range2, criteria2], ...)`         | Maximum numeric value in `max_range` matching all criteria pairs (`0.0` if no numeric cells match; `F8Error` on non-reference ranges or dimension mismatch).  |

### Conditional Aggregation Criteria (`*IFS`)

Each `criteria` argument accepts a number, boolean, string literal, or single-cell coordinate reference (multi-cell slices raise `F8Error`; `&` string concatenation is not supported in the formula grammar):

- Relational operator strings: Prefix with `">="`, `"<="`, `"<>"`, `">"`, `"<"`, or `"="` followed by a numeric, boolean, error, or text target (`">0"`, `">=100"`, `"<>0"`, `"> 10"`). Relational `<, <=, >, >=` with a numeric target match only numeric cells (text cells evaluate to false), whereas numeric `=` and `<>` also match text cells in `criteria_range` that parse to the target number, and boolean `=` and `<>` (`TRUE`, `"TRUE"`, `"=FALSE"`) also match text cells `"true"`/`"false"` (case-insensitive).
- Exact matching: Direct number (`10` or `"10"`), boolean (`TRUE` or `"TRUE"`), error (`"#DIV/0!"`), or ASCII case-insensitive text (`"Active"`, `"=Approved"`). Plain strings with leading or trailing spaces (`" 10"`, `"  DeptA"`) preserve whitespace and match strictly as text.
- Blank and non-blank sentinels: `""` or `"="` matches empty cells (`None` or `""`); `"<>"` matches non-blank cells (`"<> "` matches any cell except a single space `" "`). A direct reference to an empty cell (`None`) used as a `criteria` argument evaluates to numeric `0` (`=0`).
- Wildcards (`=` and `<>` text criteria): `?` matches any single character, `*` matches zero or more characters (`"Tech*"`, `"?est"`). Escape literal characters via `~?`, `~*`, `~~`.
- Conditional error bubbling: Errors in `criteria_range` are ignored unless matched by an error criterion; errors in target aggregation ranges (`sum_range`, `average_range`, `min_range`, `max_range`) propagate only for cells that satisfy all criteria.

---

## 3. Logical and Branching Functions (8)

| Function  | Signature                                                                  | Semantics                                                                                                                                                    |
| :-------- | :------------------------------------------------------------------------- | :----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `IF`      | `IF(logical_test, value_if_true, value_if_false)`                          | Short-circuiting 3-argument branch (`TooFewArguments` if `value_if_false` is omitted). `logical_test` requires `Boolean`; `value_if_true` and `value_if_false` must unify to a single scalar type (`NA()` allowed for dynamic `#N/A` suppression; referencing an `Empty` cell preserves `Empty` mid-expression for `ISBLANK`, raising `F8Error` if unguarded at the cell root). |
| `IFS`     | `IFS(logical_test1, value_if_true1, [logical_test2, value_if_true2], ...)` | Evaluates `(test, value)` pairs left-to-right with short-circuiting. Requires even argument count (`>= 2`), `Boolean` tests, and monomorphic result arms (preserving `Empty` mid-expression for `ISBLANK`). Returns `#N/A` if no condition is true. |
| `IFERROR` | `IFERROR(value, value_if_error)`                                           | Short-circuiting error trap: returns `value_if_error` only if `value` evaluates to a runtime Excel error (compile-time `F8Error`s propagate without trapping; both arms must be non-`Empty` and unify to a single scalar type). |
| `IFNA`    | `IFNA(value, value_if_na)`                                                 | Short-circuiting `#N/A` trap: returns `value_if_na` only if `value` evaluates to `#N/A`, propagating other Excel errors without evaluating `value_if_na` (both arms must be non-`Empty` and unify to a single scalar type). |
| `AND`     | `AND(logical1, [logical2], ...)`                                           | Logical conjunction across `Boolean` scalars, single-cell references, and range slices. `Number` and `Text` anywhere, and `Empty` on scalars or `1x1` single-cell references, raise `F8Error`; sparse `Empty` cells inside syntactic `start..end` slices are skipped, returning `#VALUE!` if zero `Boolean` inputs remain. |
| `OR`      | `OR(logical1, [logical2], ...)`                                            | Logical disjunction across `Boolean` scalars, single-cell references, and range slices. `Number` and `Text` anywhere, and `Empty` on scalars or `1x1` single-cell references, raise `F8Error`; sparse `Empty` cells inside syntactic `start..end` slices are skipped, returning `#VALUE!` if zero `Boolean` inputs remain. |
| `NOT`     | `NOT(logical)`                                                             | Logical negation of a `Boolean` scalar (`NOT(TRUE)` -> `FALSE`, `NOT(FALSE)` -> `TRUE`; `Number`, `Text`, and `Empty` raise `F8Error`).                      |
| `CHOOSE`  | `CHOOSE(index_num, value1, [value2], ...)`                                 | Evaluates and returns choice at 1-based `Integer` `index_num` with short-circuiting (all choice arms must unify to a single scalar type; preserves `Empty` mid-expression for `ISBLANK`). Constant out-of-bounds `index_num` raises `F8Error`; dynamic out-of-bounds `index_num` returns `#VALUE!`. |

---

## 4. Financial and Discounting Functions (7)

| Function | Signature                                 | Semantics                                                                                                                                                                                                                           |
| :------- | :---------------------------------------- | :---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `NPV`    | `NPV(rate, value1, [value2], ...)`        | Periodic net present value starting at `t = 1` (`rate` requires `Number`): `sum(CF_t / (1 + rate)^t)`. Range text, booleans, and empty cells are ignored; direct scalar `value_i` arguments coerce booleans (`1.0`/`0.0`) and empty cells (`0.0`), and return `#VALUE!` on uncoercible text. Returns `#DIV/0!` if `rate = -1.0` (or denominator underflows to `0.0`); `rate < -1.0` is allowed (integer exponents); float overflow returns `#NUM!`. |
| `XNPV`   | `XNPV(rate, values, dates)`               | Exact-date net present value using Actual/365 (`rate` requires `Number`; `values` and `dates` require equal-length 1D `Table[...]` ranges, else `F8Error`): `sum(CF_i / (1 + rate)^((d_i - d_1) / 365))`. Coerces cells in `values` and `dates` (`Empty` -> `0.0`, booleans -> `1.0`/`0.0`, text -> `#VALUE!`). Requires `rate > -1.0` and `trunc(d_i) >= trunc(d_1)` (violations return `#NUM!`). |
| `IRR`    | `IRR(values, [guess])`                    | Periodic internal rate of return via Newton-Raphson (`values` requires `Table[...]` range; `guess` requires `Number`, default `0.10`, max 100 iterations, tolerance `1e-9`, clamped `> -1.0`). Range cells ignore empty/text/boolean cells. Requires `>= 2` cash flows with at least one positive and one negative value, else `#NUM!`. |
| `XIRR`   | `XIRR(values, dates, [guess])`            | Exact-date internal rate of return using Actual/365 (`values` and `dates` require equal-length 1D `Table[...]` ranges; `guess` requires `Number`, default `0.10`, max 100 iterations, tolerance `1e-9`, clamped `> -1.0`). Coerces cells in `values` and `dates` (`Empty` -> `0.0`, booleans -> `1.0`/`0.0`, text -> `#VALUE!`). Requires `>= 2` cash flows, `trunc(d_i) >= trunc(d_1)`, and sign alternation, else `#NUM!`. |
| `PMT`    | `PMT(rate, nper, pv, [fv], [type])`       | Constant annuity payment (`rate`, `nper`, `pv`, `fv` require `Number`, `fv` default `0.0`; `type` requires `Integer`, default `0` for end-of-period, non-zero for beginning-of-period). Supports `rate = 0.0`; returns `#DIV/0!` if `nper = 0.0`. |
| `IPMT`   | `IPMT(rate, per, nper, pv, [fv], [type])` | Interest component of annuity payment in period `per` (`rate`, `per`, `nper`, `pv`, `fv` require `Number`, `fv` default `0.0`; `type` requires `Integer`, default `0`). Returns `0.0` when `rate = 0.0`; returns `#NUM!` if `per < 1`, `per > nper`, or `nper <= 0`. |
| `PPMT`   | `PPMT(rate, per, nper, pv, [fv], [type])` | Principal component of annuity payment in period `per` (`PMT - IPMT`; `fv` default `0.0`, `Integer` `type` default `0`). Returns `#DIV/0!` if `nper = 0.0`; returns `#NUM!` if `per < 1` or `per > nper`.                           |

---

## 5. Lookup and Reference Functions (3)

| Function  | Signature                                                                                        | Semantics                                                                                                                                                                                                                                                            |
| :-------- | :----------------------------------------------------------------------------------------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `XLOOKUP` | `XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode])` | Searches homogeneous 1D `lookup_array` (`Table[...]` required) for non-`Empty` `lookup_value` of matching type and returns corresponding element from equal-length homogeneous 1D `return_array` (monomorphic with lazy `if_not_found`; preserves `Empty` mid-expression for `ISBLANK`). Default `match_mode = 0`, `search_mode = 1`. Unmatched lookup returns `if_not_found` or `#N/A` if omitted; 2D slices, length mismatches, `Empty` `lookup_value`, or heterogeneous arrays raise `F8Error`. |
| `XMATCH`  | `XMATCH(lookup_value, lookup_array, [match_mode], [search_mode])`                                | Returns 1-based index of non-`Empty` `lookup_value` in homogeneous 1D `lookup_array` (default `match_mode = 0`, `search_mode = 1`). Returns `#N/A` if unmatched; 2D slices, `Empty` `lookup_value`, or heterogeneous keys raise `F8Error`.                            |
| `INDEX`   | `INDEX(array, row_num, [col_num])`                                                               | Returns element at 1-based `Integer` `row_num` and `col_num` within a homogeneous 1D or 2D `Table[...]` range (preserves `Empty` mid-expression for `ISBLANK`). On 1D vectors, omitting `col_num` (or passing `1`) indexes along the vector axis; on 2D matrices, `col_num` is required (`MissingColumnIndexForMatrix`). Constant out-of-bounds indices raise `F8Error`; dynamic out-of-bounds indices return `#REF!`. |

### `match_mode` and `search_mode` (`XLOOKUP`, `XMATCH`)

- Literal-only modes: Both `match_mode` and `search_mode` must be integer literals when supplied (non-literal expressions, floats, or unsupported mode codes raise `F8Error`).
- `match_mode`: `0` (default) = exact match (ASCII case-insensitive for text); `-1` = exact or next smaller; `1` = exact or next larger (wildcard `match_mode = 2` is not supported).
- `search_mode`: `1` (default) = first-to-last; `-1` = last-to-first; `2` = binary search ascending; `-2` = binary search descending. Binary search modes (`2`, `-2`) require every `lookup_array` cell to be non-`Empty`, non-`Error`, homogeneous, and monotonically sorted (`F8Error` on violation).

---

## 6. Date and Time Functions (8)

| Function   | Signature                                 | Semantics                                                                                                                                                                                                                    |
| :--------- | :---------------------------------------- | :--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `DATE`     | `DATE(year, month, day)`                  | Constructs Excel serial date (`0.0 <= serial <= 2958465.0`, corresponding to `1900-01-00`..`9999-12-31`) from `Integer` `year`, `month`, and `day` (`F8Error` on non-integers). Years `0 <= y <= 1899` map to `y + 1900`. Normalizes month/day rollovers and preserves Excel's 1900 leap-year bug (`1900-02-29` -> `60.0`). Out-of-range dates return `#NUM!`. |
| `EDATE`    | `EDATE(start_date, months)`               | Shifts `Number` `start_date` by `Integer` `months` calendar months, clamping end-of-month overflow to the final valid day of the target month.                                                                                |
| `EOMONTH`  | `EOMONTH(start_date, months)`             | Returns serial date for the last day of the month `Integer` `months` before or after `Number` `start_date`.                                                                                                                   |
| `YEARFRAC` | `YEARFRAC(start_date, end_date, [basis])` | Year fraction between `Number` `start_date` and `end_date` (order-independent; truncates fractional time). Default `Integer` `basis = 0`. `basis` outside `0..=4` returns `#NUM!`.                                            |
| `YEAR`     | `YEAR(serial_number)`                     | Extracts 4-digit year (`1900..=9999`) from `Number` serial date.                                                                                                                                                              |
| `MONTH`    | `MONTH(serial_number)`                    | Extracts month (`1..=12`) from `Number` serial date.                                                                                                                                                                          |
| `DAY`      | `DAY(serial_number)`                      | Extracts day of month (`1..=31`) from `Number` serial date.                                                                                                                                                                   |
| `DAYS`     | `DAYS(end_date, start_date)`              | Signed day difference `trunc(end_date) - trunc(start_date)` between `Number` serial dates. Note parameter order: `end_date` first, `start_date` second.                                                                      |

### `YEARFRAC` Day-Count Basis (`0..=4`)

| `basis`          | Convention            | Denominator / Day-Count Rule                                                                 |
| :--------------- | :-------------------- | :------------------------------------------------------------------------------------------- |
| `0` (or omitted) | US (NASD) 30/360      | 30-day months / 360-day year with US end-of-February and 31st-of-month adjustment rules      |
| `1`              | Actual/Actual (AFB)   | Exact elapsed days divided by actual days in year (`365` or `366` based on leap-year rules)  |
| `2`              | Actual/360            | Exact elapsed days divided by `360.0`                                                        |
| `3`              | Actual/365            | Exact elapsed days divided by `365.0`                                                        |
| `4`              | European 30/360       | 30-day months / 360-day year where any 31st day of month is clamped to `30`                  |

---

## 7. Information Functions (2)

| Function  | Signature        | Semantics                                                                                                                                                                                                |
| :-------- | :--------------- | :------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ISBLANK` | `ISBLANK(value)` | Returns `TRUE` if `value` evaluates to `Empty` (`None` cell or reference-returning `IF`/`IFS`/`CHOOSE`/`INDEX`/`XLOOKUP` selecting an empty cell), and `FALSE` for all other values including `""` and errors. |
| `NA`      | `NA()`           | Returns `#N/A` with polymorphic bottom type, unifying with any typed branch arm (`IF(ebitda > 0, ev / ebitda, NA())`).                                                                                   |
