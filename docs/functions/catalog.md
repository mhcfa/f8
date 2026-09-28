# Standard Function Catalog (V1)

This document catalogs the standard built-in Excel functions supported in f8 V1, detailing signatures, constraints, and calculation behaviors.

## 1. Mathematical & Aggregation Functions

| Function | Min args | Max args | Variadic rule | Description |
| :--- | :--- | :--- | :--- | :--- |
| `SUM` | 1 | Unlimited | `Single` | Arithmetic sum across scalars and ranges; ignores text in ranges |
| `AVERAGE` | 1 | Unlimited | `Single` | Arithmetic mean, ignoring blank cells and text in ranges |
| `COUNT` | 1 | Unlimited | `Single` | Count of cells containing numeric values |
| `COUNTA` | 1 | Unlimited | `Single` | Count of non-empty cells (numbers, text, booleans, errors) |
| `MAX` | 1 | Unlimited | `Single` | Maximum numeric value |
| `MIN` | 1 | Unlimited | `Single` | Minimum numeric value |
| `MEDIAN` | 1 | Unlimited | `Single` | Statistical median (middle value) across scalars and ranges; ignores text in ranges |
| `ROUND` | 2 | 2 | `None` | Rounds number to specified decimal places |
| `ROUNDUP` | 2 | 2 | `None` | Rounds number upward, away from zero |
| `ROUNDDOWN` | 2 | 2 | `None` | Rounds number downward, toward zero |
| `ABS` | 1 | 1 | `None` | Absolute value of a number |
| `SUMPRODUCT` | 1 | Unlimited | `Single` | Multiplies corresponding elements in comma-separated range slices of identical dimensions and returns their sum (e.g. weighted averages); intermediate array arithmetic is prohibited |

---

## 2. Conditional Aggregation Functions (*IFS)

f8 standardizes exclusively on modern plural `*IFS` signatures with paired trailing criteria arguments, eliminating the inverted argument order of legacy singular `SUMIF`/`COUNTIF`.

| Function | Min args | Max args | Variadic rule | Description |
| :--- | :--- | :--- | :--- | :--- |
| `SUMIFS` | 3 | Unlimited | `Pairs` | Sums cells in `sum_range` satisfying all paired criteria (`sum_range`, `criteria_range1`, `criteria1`, ...) |
| `COUNTIFS` | 2 | Unlimited | `Pairs` | Counts cells satisfying all paired criteria (`criteria_range1`, `criteria1`, ...) |
| `AVERAGEIFS` | 3 | Unlimited | `Pairs` | Computes arithmetic mean of cells in `average_range` satisfying all paired criteria |
| `MINIFS` | 3 | Unlimited | `Pairs` | Returns minimum value among cells in `min_range` satisfying all paired criteria |
| `MAXIFS` | 3 | Unlimited | `Pairs` | Returns maximum value among cells in `max_range` satisfying all paired criteria |

### 2.1. 1:1 Criteria Parity & Syntax Rules

- **Exact equality**: Scalar values (`"US"`, `42`, `TRUE`). String equality is case-insensitive.
- **Comparison prefixes**: String expressions with relational operators (`">10"`, `"<=5"`, `"<>"`, `"="`), or cell references evaluating to criteria strings.
- **Wildcards**: Text criteria support `?` (any single character), `*` (zero or more characters), and `~` (escape character for `~?`, `~*`, `~~`).
- **Dimension matching**: Every `criteria_range_k` must have identical row and column counts to the primary range; mismatched dimensions return `#VALUE!`.
- **Excel emission parity**: f8 compiles and evaluates standard `*IFS` expressions directly and emits identical `=SUMIFS(...)` formulas into `.xlsx` binaries without synthetic AST transformations.

---

## 3. Logical Functions

| Function | Min args | Max args | Variadic rule | Description |
| :--- | :--- | :--- | :--- | :--- |
| `IF` | 2 | 3 | `None` | Conditional branch with lazy short-circuit evaluation |
| `IFS` | 2 | Unlimited | `Pairs` | Multi-condition evaluation with paired test-value branches |
| `IFERROR` | 2 | 2 | `None` | Evaluates value; lazily returns fallback if error occurs |
| `IFNA` | 2 | 2 | `None` | Evaluates value; lazily returns fallback if `#N/A` occurs |
| `AND` | 1 | Unlimited | `Single` | Logical conjunction across arguments (eagerly evaluated) |
| `OR` | 1 | Unlimited | `Single` | Logical disjunction across arguments (eagerly evaluated) |
| `NOT` | 1 | 1 | `None` | Logical negation |
| `CHOOSE` | 2 | Unlimited | `Single` | Selects value from list based on 1-based index (lazy evaluation) |
| `SWITCH` | 3 | Unlimited | `Single` | Evaluates expression against value-result cases with optional default |

---

## 4. Financial Functions

| Function | Min args | Max args | Variadic rule | Description |
| :--- | :--- | :--- | :--- | :--- |
| `NPV` | 2 | Unlimited | `Single` | Net present value for periodic cash flows starting at $t=1$ |
| `XNPV` | 3 | 3 | `None` | Net present value for non-periodic cash flows with specific dates |
| `IRR` | 1 | 2 | `None` | Internal rate of return for periodic cash flows (`values`, `[guess]`) |
| `XIRR` | 2 | 3 | `None` | Internal rate of return for non-periodic cash flows (`values`, `dates`, `[guess]`) |
| `PMT` | 3 | 5 | `None` | Constant periodic annuity payment (`rate`, `nper`, `pv`, `[fv]`, `[type]`) |
| `IPMT` | 4 | 6 | `None` | Interest portion of annuity payment for a given period |
| `PPMT` | 4 | 6 | `None` | Principal portion of annuity payment for a given period |

### 4.1. Periodic vs. Non-Periodic Discounting (`NPV` vs. `XNPV`)

- **`NPV(rate, value1, [value2], ...)`**:
  - Treats cash flows as occurring at periodic intervals starting at $t=1$:
    $$\text{NPV}(r, v_1, v_2, \dots) = \sum_{i=1}^n \frac{v_i}{(1 + r)^i}$$
  - Period 0 initial outlays must be added outside the formula: `=initial_outlay + NPV(rate, cash_flows)`.
- **`XNPV(rate, values, dates)`**:
  - Uses exact Actual/365 year fractions:
    $$\text{XNPV}(r, \mathbf{v}, \mathbf{d}) = \sum_{i=1}^n \frac{v_i}{(1 + r)^{(d_i - d_1) / 365.0}}$$
  - Constraints: `values` and `dates` must share equal length. Dates must satisfy $d_i \ge d_1$. Rate $r \le -1.0$ returns `ExcelError::Num`.
- **`XIRR(values, dates, [guess])`**:
  - Solves $\text{XNPV}(r, \mathbf{v}, \mathbf{d}) = 0$ via Newton-Raphson iteration (default guess: `0.10`).
  - Cash flows must contain at least one positive and one negative value; otherwise returns `ExcelError::Num`.

### 4.2. Annuity Calculations (`PMT`, `IPMT`, `PPMT`)

- **Sign convention**: Cash outflows are negative; inflows are positive. For a loan borrowing $+pv$, `PMT` evaluates to a negative periodic debt service amount.
- **Payment timing (`type`)**: `0` (or omitted) indicates payment at period end; `1` indicates payment at period start.
- **Defaults**: `fv = 0.0`, `type = 0`.

---

## 5. Lookup & Reference Functions

| Function | Min args | Max args | Variadic rule | Description |
| :--- | :--- | :--- | :--- | :--- |
| `XLOOKUP` | 3 | 6 | `None` | Modern lookup returning scalar value from matching return range |
| `XMATCH` | 2 | 4 | `None` | Returns relative position of an item in a 1D array or range |
| `INDEX` | 2 | 3 | `None` | Returns scalar value at row/col offset within range |

### 5.1. Modern Lookup (`XLOOKUP`)

- **Signature**: `XLOOKUP(lookup_value, lookup_array, return_array, [if_not_found], [match_mode], [search_mode])`.
- **Defaults**:
  - `if_not_found`: Defaults to returning `#N/A` if no match is found (short-circuiting lazy evaluation).
  - `match_mode`: Defaults to `0` (exact match). `-1` = exact or next smaller, `1` = exact or next larger, `2` = wildcard (`*`, `?`, `~`).
  - `search_mode`: Defaults to `1` (search first-to-last). `-1` = search last-to-first, `2` = binary search ascending, `-2` = binary search descending.
- **Return value**: Evaluates strictly to the matching scalar `CellValue` from `return_array`.

### 5.2. Relative Position Matching (`XMATCH`)

- **Signature**: `XMATCH(lookup_value, lookup_array, [match_mode], [search_mode])`.
- **Defaults**:
  - `match_mode`: Defaults to `0` (exact match). `-1` = exact or next smaller, `1` = exact or next larger, `2` = wildcard (`*`, `?`, `~`).
  - `search_mode`: Defaults to `1` (search first-to-last). `-1` = search last-to-first, `2` = binary search ascending, `-2` = binary search descending.
- **Return value**: Evaluates to the 1-based relative position integer (`f64`) of the matched item, or `#N/A` if not found.

### 5.3. Positional Indexing (`INDEX`)

- **`INDEX(array, row_num, [col_num])`**:
  - Retrieves the scalar `CellValue` located at the 1-based `row_num` and `col_num` offsets within `array`.
  - On 1D row ranges, `col_num` represents the column offset; on 1D column ranges, `row_num` represents the row offset.
  - On 2D matrices, both `row_num` and `col_num` are evaluated; out-of-bounds coordinates return `#REF!`.

---

## 6. Date & Time Functions

f8 models dates as dedicated calculation rows (`format: "date"`), keeping column dimensions structural and coordinates static. Date math operates on numeric serial dates or explicit `DATE(year, month, day)` constructors, avoiding implicit string parsing.

| Function | Min args | Max args | Variadic rule | Description |
| :--- | :--- | :--- | :--- | :--- |
| `YEARFRAC` | 2 | 3 | `None` | Year fraction between two dates using specified day count convention |
| `DATE` | 3 | 3 | `None` | Constructs Excel serial date from integer `year`, `month`, `day` |
| `EDATE` | 2 | 2 | `None` | Serial date that is indicated number of months before/after start date |
| `EOMONTH` | 2 | 2 | `None` | Serial date of the last day of the month before/after offset months |
| `YEAR` | 1 | 1 | `None` | Calendar year (1900–9999) corresponding to serial date |
| `MONTH` | 1 | 1 | `None` | Calendar month (1–12) corresponding to serial date |
| `DAY` | 1 | 1 | `None` | Day of the month (1–31) corresponding to serial date |
| `DAYS` | 2 | 2 | `None` | Number of days between two dates (`end_date`, `start_date`) |

### 6.1. Serial Date Representation & The 1900 Leap Day Bug

Excel represents dates as floating-point day counts where serial day `1.0` is `1900-01-01`.

To guarantee bit-for-bit Excel calculation parity, f8 strictly reproduces the historical Lotus 1-2-3 / Excel leap year bug:
- **Serial day 60.0**: Treats `1900-02-29` as serial day `60.0`, even though 1900 was not a leap year in the Gregorian calendar.
- **Offset on modern dates**: All serial dates on or after March 1, 1900 carry a $+1$ offset relative to true astronomical day counts, causing the Unix epoch (`1970-01-01`) to map to serial day `25569.0` rather than `25568.0`.
- See [In-Memory Evaluator & Parity](../../specs/living-specs/compiler/evaluator.md) for details on the 1900 leap year bug.

Day count conventions and basis values (`0` through `4`) for `YEARFRAC` are specified in [In-Memory Evaluator & Parity](../../specs/living-specs/compiler/evaluator.md).

See [Function Registry Architecture](../../specs/living-specs/functions/registry.md) for traits and dispatch, [In-Memory Evaluator & Parity](../../specs/living-specs/compiler/evaluator.md) for calculation rules, and [Formula Grammar](../dsl/grammar.md) for formula expressions.
