# Function Reference

Authoritative call, typing, empty-cell, and `_` omission rules, runtime Excel errors, all 37 built-in functions (`f8`-specific constraints and Excel deltas), composition patterns for omitted functions, lookup modes, and `YEARFRAC` bases.

## 1. Call, Typing, Empty, Omission (`_`), and Error Rules

- Parity & exclusions: IEEE 754 `f64` (`-0.0 -> 0.0`, non-finite overflow -> `#NUM!`). Excludes volatile functions (`NOW`, `TODAY`, `RAND`, `OFFSET`, `INDIRECT`), dynamic-array spilling, implicit intersection, and wildcards.
- Compile-time validation (`F8Error` at `compile()`, checking static type/shape/role across both active and untaken lazy branches in `IF`, `IFERROR`, `CHOOSE`, `XLOOKUP`; empty checks run on evaluated paths):
  - Strict typing: Arithmetic and numeric params require `Number`; logical conditions (`IF`, `NOT`, `AND`, `OR`) require `Boolean`; relational ops (`=`, `<>`, `<`, `<=`, `>`, `>=`) require matching non-empty types (`Number`, ASCII case-insensitive `Text`, or `Boolean`). No implicit coercions; root formulas cannot evaluate to empty.
  - Role & empty policy: Numeric/financial aggregations (`SUM`, `AVERAGE`, `MIN`, `MAX`, `MEDIAN`, `COUNT`, `SUMPRODUCT`, `NPV`, `IRR`, `XNPV`, `XIRR`) reject `descriptor` columns, `Text`, and `Boolean`. Scalar args and single-cell (non-slice) refs reject empty across all functions; `NPV`/`IRR`/`XNPV`/`XIRR` also reject empty cells inside `start..end` slices (use explicit `0.0`), while math/logical aggregations (`SUM`, `AVERAGE`, `MIN`, `MAX`, `MEDIAN`, `COUNT`, `SUMPRODUCT`, `AND`, `OR`) skip sparse empty cells inside syntactic `start..end` slices.
  - Monomorphic branches: Result arms of `IF`, `CHOOSE`, `IFERROR`, and `XLOOKUP` (`return_array` + `if_not_found`) must unify to one scalar type (evaluated arms cannot be empty). `NA()` acts as a polymorphic bottom (`#N/A`) unifying with any arm type; runtime-error formulas preserve their static type downstream.
  - Range-only slots & geometry: No syntactic `start..end` slices in scalar contexts. Range-only slots (`COUNT`, `SUMPRODUCT`, `XLOOKUP`/`XMATCH`/`INDEX` arrays, `IRR`/`XNPV`/`XIRR` series) require `Table[...]` refs. `SUMPRODUCT`/`XLOOKUP` require matching `(rows, cols)`; `XNPV`/`XIRR` require equal-length 1D ranges; `XLOOKUP`/`XMATCH`/`XNPV`/`XIRR` require 1D ranges; 2D `INDEX` requires `col_num`. Lookup/index arrays must be type-homogeneous (`lookup_value` must statically match `lookup_array` type) and reject empty keys, single-cell (non-slice) empty refs, and selected empty return elements.
- Argument omission (`_`): Skips intermediate optional parameters when followed by an explicit argument (`PMT(r, n, pv, _, 1)`, `XLOOKUP(k, look, ret, _, -1)`, `XLOOKUP(k, look, ret, _, _, -1)`), binding to the parameter default (or omitted lazy `if_not_found` -> `#N/A`) and lowering to `,,`. Omitting required or trailing optionals with `_` throws `F8Error`; drop trailing optionals directly.
- Runtime Excel errors (stored as cell values; propagate unless handled by `IFERROR` or inspected by `ISERROR`):
  - `#DIV/0!`: `x / 0`, `0 ^ -k`, `MOD(x, 0)`, all-empty `AVERAGE` slice, `NPV` `rate = -1.0`.
  - `#VALUE!`: `CHOOSE` `index_num` outside `1..=N`, all-empty `AND`/`OR` slice, unsupported `XLOOKUP`/`XMATCH` `match_mode`/`search_mode` (including wildcard `2`), empty/unsorted binary-search `lookup_array` (error cells in binary-search arrays bubble their error).
  - `#REF!`: Out-of-bounds `INDEX` `row_num` or `col_num`.
  - `#NUM!`: Overflow/`NaN`/`Inf`, `0 ^ 0`, negative base with fractional exponent, `LN(<= 0)`, `EXP` overflow (`> ~709.78`), `MOD` with `|number / divisor| > 2^53 - 1`, non-integer at discrete integer params (`ROUND*` `num_digits`, `CHOOSE`/`INDEX` indices, lookup modes, `DATE`/`EDATE`/`EOMONTH`/`YEARFRAC`/`PMT` integer params), invalid `DATE` (outside `1900..=9999`, 2-digit years, rollovers), date serials outside `1..=2958465` (including `0`), invalid `PMT` `type`/`rate <= -1`/`nper = 0`, `YEARFRAC` `basis` outside `0..=4`, all-empty `MEDIAN`, `IRR`/`XIRR`/`XNPV` domain/convergence errors, `EDATE`/`EOMONTH` stepping outside `1900..=9999`.
  - `#N/A`: `NA()`, unmatched `XLOOKUP` (without `if_not_found`), or unmatched `XMATCH`.

## 2. Math and Aggregation (14)

All parameters require `Number`; aggregations follow Section 1 `descriptor` and sparse-empty rules.

- `ABS(number)`: Scalar `Number`.
- `LN(number)`: `number <= 0` -> `#NUM!`.
- `EXP(number)`: Overflow (`> ~709.78`) -> `#NUM!`.
- `MOD(number, divisor)`: Floored remainder (divisor-sign rule); `divisor = 0` -> `#DIV/0!`; `|number / divisor| > 2^53 - 1` -> `#NUM!`.
- `SUM(number1, [number2], ...)`: Sparse empties in slices -> `0.0`.
- `AVERAGE(number1, [number2], ...)`: Skips sparse empties; all-empty slice -> `#DIV/0!`.
- `COUNT(value1, [value2], ...)`: Range-only `Table[...]` `measure` refs; ignores sparse empties and runtime error cells.
- `MAX(number1, [number2], ...)`: Skips sparse empties; all-empty slice -> `0.0`.
- `MIN(number1, [number2], ...)`: Skips sparse empties; all-empty slice -> `0.0`.
- `MEDIAN(number1, [number2], ...)`: Skips sparse empties; all-empty slice -> `#NUM!`.
- `ROUND(number, num_digits)`: 15-sig-digit base-10 half-away-from-zero; strict integer `num_digits` (non-integer -> `#NUM!`).
- `ROUNDUP(number, num_digits)`: 15-sig-digit away from zero; strict integer `num_digits` (non-integer -> `#NUM!`).
- `ROUNDDOWN(number, num_digits)`: 15-sig-digit toward zero; strict integer `num_digits` (non-integer -> `#NUM!`).
- `SUMPRODUCT(array1, [array2], ...)`: Equal-dimension `(rows, cols)` `Table[...]` `measure` ranges; sparse empties -> `0.0`.

### Composition Patterns for Omitted Conditional Aggregations

`COUNTA`, `SUMIFS`, `COUNTIFS`, `AVERAGEIFS`, `MINIFS`, and `MAXIFS` are omitted (`F8Error`). Patterns:

- Conditional sum: Helper `IF(cond, value, 0)` + `SUM(...)`, or `0`/`1` flag `IF(cond, 1, 0)` + `SUMPRODUCT(values, flags)`.
- Conditional count: `0`/`1` flag `IF(cond, 1, 0)` (combine via `AND`/`OR`) + `SUM(flags)`.
- Conditional average: `SUMPRODUCT(values, flags) / SUM(flags)` for finite values, or `SUM(zeroed_helper) / SUM(flags)` when excluded cells may be `NA()` (`SUMPRODUCT` propagates `#N/A` even against `0`; `SUM(flags) = 0` -> `#DIV/0!`).
- Static subset `MIN`/`MAX`/`AVERAGE`/`MEDIAN`: Use 0D `null` overrides on excluded cells in a helper `start..end` slice (skipped automatically).

## 3. Logical and Branching (6)

- `IF(logical_test, value_if_true, value_if_false)`: 3-arg required, short-circuiting, `Boolean` condition, monomorphic arms (`NA()` allowed).
- `IFERROR(value, value_if_error)`: Short-circuiting; traps runtime Excel errors only (`F8Error` propagates); monomorphic arms.
- `AND(logical1, [logical2], ...)`: `Boolean` only; skips sparse empties in slices; zero inputs -> `#VALUE!`.
- `OR(logical1, [logical2], ...)`: `Boolean` only; skips sparse empties in slices; zero inputs -> `#VALUE!`.
- `NOT(logical)`: Scalar `Boolean`.
- `CHOOSE(index_num, value1, [value2], ...)`: Short-circuiting 1-based; strict integer `index_num` (non-int -> `#NUM!`, outside `1..=N` -> `#VALUE!`); monomorphic arms.

## 4. Financial and Discounting (5)

Cash-flow and date slices reject empty cells (`F8Error`; use explicit `0.0`) and `descriptor` columns.

- `NPV(rate, value1, [value2], ...)`: Periodic starting at `t = 1`; `rate = -1.0` -> `#DIV/0!` (`rate < -1.0` allowed).
- `XNPV(rate, values, dates)`: Actual/365 over equal-length 1D `measure` ranges; requires `rate > 0.0`, serials `1..=2958465`, and `trunc(d_i) >= trunc(d_1)` (else `#NUM!`).
- `IRR(values, [guess])`: `measure` range, default `guess = 0.10`; `#NUM!` on `guess <= -1.0`, `< 2` cash flows, no sign alternation, or no root `> -1.0`.
- `XIRR(values, dates, [guess])`: Actual/365 over equal-length 1D `measure` ranges, default `guess = 0.10`; `#NUM!` on `guess <= -1.0`, `< 2` cash flows, invalid dates, `trunc(d_i) < trunc(d_1)`, no net sign alternation across distinct dates, or no root `> -1.0`.
- `PMT(rate, nper, pv, [fv], [type])`: Defaults `fv = 0.0`, strict integer `type = 0` (`0` end / `1` beginning); supports `rate = 0.0`; invalid `type`, `rate <= -1.0`, or `nper = 0` -> `#NUM!`.

### Composition Pattern for Omitted Debt Amortization (`IPMT`, `PPMT`)

`IPMT` and `PPMT` are omitted (`F8Error`). Model per-period interest and principal with `PMT` on a 2D balance schedule: `scheduled_pmt = PMT(rate, nper, -opening_principal)`, `opening_balance = @table["closing_balance", col.prev]`, `interest = opening_balance * rate`, `principal = scheduled_pmt - interest` (or `MIN(opening_balance, scheduled_pmt - interest)`), `closing_balance = opening_balance - principal`.

## 5. Lookup and Reference (3)

- `XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode])`: Homogeneous 1D `Table[...]` arrays of matching `(rows, cols)` (both vertical or both horizontal); `lookup_value` must match `lookup_array` type; monomorphic with lazy `if_not_found`. Defaults `match_mode = 0`, `search_mode = 1`; linear search skips sparse empties and runtime errors; single-cell (non-slice) empty refs or selected empty return/`if_not_found` throw `F8Error`; miss -> `if_not_found` or `#N/A`.
- `XMATCH(lookup_value, lookup_array, [match_mode], [search_mode])`: 1-based position in homogeneous 1D `Table[...]` matching `lookup_value` type; defaults `match_mode = 0`, `search_mode = 1`; linear search skips sparse empties and runtime errors; single-cell (non-slice) empty refs throw `F8Error`; miss -> `#N/A`.
- `INDEX(array, row_num, [col_num])`: 1-based strict integer indices (`#NUM!` / `#REF!`) into homogeneous 1D or 2D `Table[...]` (single-cell (non-slice) empty `array` or selected empty throws `F8Error`). On 1D vectors, omitting `col_num` indexes along the vector axis (`N x 1` or `1 x N`); passing `col_num` indexes 2D coordinates directly (`1 x N` requires `INDEX(arr, 1, col_num)`). On 2D matrices, `col_num` is required.

### `match_mode` and `search_mode` (`XLOOKUP`, `XMATCH`)

Both require strict integer `Number` (non-integer -> `#NUM!`; unsupported code -> `#VALUE!`):

- `match_mode`: `0` (default) exact (ASCII case-insensitive for text), `-1` exact or next smaller, `1` exact or next larger (wildcard `2` unsupported -> `#VALUE!`).
- `search_mode`: `1` (default) first-to-last, `-1` last-to-first, `2` binary ascending (first exact match on duplicates), `-2` binary descending (last exact match on duplicates); approximate misses return the duplicate adjacent to the insertion point. Binary modes (`2`, `-2`) require every `lookup_array` cell to be non-empty, non-error, and monotonically sorted (`#VALUE!` on empty/unsorted; errors bubble).

## 6. Date and Time (7)

`DAYS` is omitted (`F8Error`); subtract date serials (`end_date - start_date`). All input serials must be in `1..=2958465` (`1900-01-01`..`9999-12-31`, rejecting `0`; else `#NUM!`).

- `DATE(year, month, day)`: Strict integer `year` (`1900..=9999`), `month` (`1..=12`), `day` (`1..=days_in_month`); preserves `1900-02-29` -> `60.0`; rejects 2-digit years and rollovers (`#NUM!`).
- `EDATE(start_date, months)`: Strict integer `months` shift clamping month-end (`1900-02` -> `59.0`; outside `1900..=9999` -> `#NUM!`).
- `EOMONTH(start_date, months)`: Last Gregorian day strict integer `months` away (`1900-02` -> `59.0`; outside `1900..=9999` -> `#NUM!`).
- `YEARFRAC(start_date, end_date, [basis])`: Order-independent year fraction (truncates fractional serial time); default strict integer `basis = 0` in `0..=4`.
- `YEAR(serial_number)`: Extracts 4-digit year (`1900..=9999`).
- `MONTH(serial_number)`: Extracts month (`1..=12`).
- `DAY(serial_number)`: Extracts day (`1..=31`).

`YEARFRAC` `basis`: `0` (default) US NASD 30/360, `1` Actual/Actual, `2` Actual/360, `3` Actual/365, `4` European 30/360.

## 7. Information (2)

- `ISERROR(value)`: `TRUE` for any runtime Excel error (`#DIV/0!`, `#VALUE!`, `#REF!`, `#NAME?`, `#NUM!`, `#N/A`), `FALSE` for `Number`/`Text`/`Boolean` (empty throws `F8Error`; compile-time `F8Error`s still fail).
- `NA()`: Returns `#N/A` with polymorphic bottom type unifying with any typed branch arm (e.g. `IF(ebitda > 0, ev / ebitda, NA())`).
