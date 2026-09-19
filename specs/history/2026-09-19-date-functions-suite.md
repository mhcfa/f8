---
title: Date & Time Functions Suite (DATE, EDATE, EOMONTH, YEARFRAC, YEAR, MONTH, DAY, DAYS)
date: 2026-09-19
status: completed
target-living-spec: specs/living-specs/functions/registry.md
---

## Proposal

### What

Implement the standard Excel Date & Time function suite:
- `DATE`: Constructs an Excel serial date number from `(year, month, day)` integer components, supporting month/day rollover and 1900 date system conventions.
- `EDATE`: Calculates the serial date offset by a positive or negative count of calendar months from a start date, clamping to month-end boundaries.
- `EOMONTH`: Calculates the serial date of the last day of the calendar month offset by $n$ months from a start date.
- `YEARFRAC`: Computes the contractual year fraction between two dates across standard financial day-count conventions (Bases 0–4: US 30/360, Actual/Actual, Actual/360, Actual/365, European 30/360).
- `YEAR`: Extracts the calendar year integer (1900–9999) from an Excel serial date number.
- `MONTH`: Extracts the calendar month integer (1–12) from an Excel serial date number.
- `DAY`: Extracts the day of the month integer (1–31) from an Excel serial date number.
- `DAYS`: Computes the elapsed integer days between two dates (`end_date - start_date`).

In addition, implement a standalone pure Rust calendar and serial date conversion engine (`src/functions/date/serial.rs`) with zero external dependencies, bit-for-bit Excel 1900 leap year bug parity (serial `60.0` = `1900-02-29`), and zero runtime volatility.

### Why

1. **Financial timeline backbone**: Timeline indexing, debt amortization schedules, interest accrual, stub periods, and discounted cash flows (`XNPV`/`XIRR`) depend on exact calendar date arithmetic.
2. **Contractual day-count compliance**: Accurate bond yields, loan interest accruals, and discount factors require standard contractual year fractions (`YEARFRAC`) across US NASD 30/360, European 30E/360, and Actual/Actual conventions.
3. **1900 leap year bug parity**: Excel intentionally replicates Lotus 1-2-3's bug treating 1900 as a leap year. Without replicating serial day `60.0 = 1900-02-29` and the $+1$ day offset for dates on or after March 1, 1900, Finx serial dates would desynchronize from Excel by exactly one day.
4. **Deterministic and non-volatile**: Date functions must be pure functions of their coordinate arguments. Environmental volatile functions (`TODAY`, `NOW`) are strictly excluded to ensure headless CI/CD and compiled workbooks evaluate deterministically without runtime drift.
5. **Zero bloat**: Implementing the Gregorian calendar math and Excel 1900 epoch natively in Rust avoids heavy third-party dependencies (like `chrono`), keeping build times minimal and memory footprint lightweight.

### Scope

- **In-scope**:
  - `src/functions/date/mod.rs`: Module declaration and registration for `FunctionCategory::DateTime`.
  - `src/functions/date/serial.rs`: Pure Rust calendar engine, Gregorian math, 1900 leap year bug handling, serial $\leftrightarrow$ `(year, month, day)` conversions, and month clamping.
  - `src/functions/date/constructors.rs`: `DateFunction`, `EdateFunction`, and `EomonthFunction`.
  - `src/functions/date/parts.rs`: `YearFunction`, `MonthFunction`, `DayFunction`, and `DaysFunction`.
  - `src/functions/date/yearfrac.rs`: `YearfracFunction` supporting Bases 0, 1, 2, 3, and 4.
  - Integration into `src/functions/mod.rs` and `FunctionRegistry::standard()`.
  - Comprehensive unit and integration test suite in `tests/function_date_test.rs`.
  - Updating standard registry function counts across tests (from 35 to 43 functions).
- **Out-of-scope**:
  - Volatile environmental date/time functions (`TODAY`, `NOW`, `TIME`, `SECOND`, `MINUTE`, `HOUR`).
  - Implicit date string parsing (date arguments must be numbers or constructed via `DATE(...)`).
  - 1904 date system (Mac Excel legacy option).
  - Business day calculators (`WORKDAY`, `NETWORKDAYS`).

### What Changes

- `src/functions/date/`: New module with `mod.rs`, `serial.rs`, `constructors.rs`, `parts.rs`, and `yearfrac.rs`.
- `src/functions/mod.rs`: Declare `pub mod date;`.
- `src/functions/registry.rs`: Register the 8 date functions in `FunctionRegistry::standard()`.
- `tests/function_date_test.rs`: New comprehensive test suite covering all 8 functions, 1900 leap day edge cases, rollover semantics, year fraction bases, and formula parser integration.
- `tests/function_registry_test.rs` & `tests/function_math_test.rs`: Update total function count assertion from 35 to 43.

### Capabilities

- **New capabilities**:
  - Construct serial dates via `DATE(year, month, day)` with full rollover support (e.g. `DATE(2024, 13, 1)` $\rightarrow$ `2025-01-01`).
  - Calculate monthly payment and milestone dates with `EDATE` and `EOMONTH`.
  - Extract date components via `YEAR`, `MONTH`, and `DAY`.
  - Calculate day counts via `DAYS(end_date, start_date)`.
  - Calculate financial year fractions via `YEARFRAC(start_date, end_date, [basis])`.
- **Modified capabilities**:
  - Standard function registry expands from 35 to 43 functions.

---

## Design

### Goals

1. **Bit-for-bit Excel parity**: Match Excel 365 / 2024 calculation semantics, including the 1900 leap day bug and integer rollover rules.
2. **Deterministic purity**: Zero volatile functions; all calculations are pure functions of inputs.
3. **Zero external dependencies**: Calendar conversions implemented in pure Rust without `chrono`.
4. **Contractual day-count fidelity**: Accurate implementation of all 5 `YEARFRAC` basis conventions.
5. **Zero production panics**: Strict error bubbling using `thiserror` and `ExcelError`.

### Invariants

- **Serial date bounds**: Valid serial dates are in the range `0.0 <= serial <= 2958465.0` (`9999-12-31`). Values outside this range return `ExcelError::Num`.
- **1900 Leap Day Bug**:
  - Serial `60.0` represents `1900-02-29`.
  - Serials `1.0..=59.0` represent `1900-01-01` through `1900-02-28`.
  - Serials `>= 61.0` represent `1900-03-01` through `9999-12-31` with the historical $+1$ day offset.
  - Serial `0.0` represents `1900-01-00` (`YEAR=1900, MONTH=1, DAY=0`).
- **Date truncation**: Component extractors (`YEAR`, `MONTH`, `DAY`), step functions (`EDATE`, `EOMONTH`), and difference calculators (`DAYS`, `YEARFRAC`) truncate or floor fractional serials to integer days before processing.
- **Basis validation in YEARFRAC**: The `basis` argument must evaluate to an integer in `0..=4` (fractional values truncated). Any other value returns `ExcelError::Num`.
- **Order independence in YEARFRAC**: If `start_date > end_date`, `YEARFRAC` computes the positive fraction between the dates.
- **DAYS argument order**: First argument is `end_date`, second is `start_date`. `DAYS(end, start)` returns `end.trunc() - start.trunc()`.

### Decisions

- **Decision 1: Native pure Rust calendar math without chrono**:
  - **Choice**: Implement `DateSerial` conversions in `src/functions/date/serial.rs` using standard civil calendar algorithms.
  - **Rationale**: `chrono` does not natively support Excel's 1900 leap year bug (it rejects `1900-02-29` as invalid), nor does it model Excel serial day counts. A bespoke module is $\sim 100$ lines of code, adds zero compile-time overhead, and gives 100% control over Excel parity.
  - **Alternatives considered**: Pulling in `chrono`. Rejected due to incompatibility with the 1900 leap day bug and unnecessary binary bloat.
- **Decision 2: Comprehensive YEARFRAC basis coverage**:
  - **Choice**: Implement all 5 conventions (`0` = US 30/360 NASD, `1` = Actual/Actual, `2` = Actual/360, `3` = Actual/365, `4` = European 30E/360).
  - **Rationale**: Financial models use different conventions depending on the instrument (e.g. US bonds use basis 0, SOFR loans use basis 2, UK gilts use basis 3, Eurobonds use basis 4).
  - **Alternatives considered**: Supporting only Basis 0 and 3. Rejected because professional financial modeling requires all 5 standard conventions.
- **Decision 3: Rollover normalization in DATE**:
  - **Choice**: Allow `month` and `day` to take any integer value, propagating rollover into adjacent months and years.
  - **Rationale**: Excel formulas frequently use expressions like `DATE(year, month + 1, 0)` to calculate the last day of a month, or `DATE(year, 1, day_of_year)` to convert ordinal days.
  - **Alternatives considered**: Rejecting out-of-range months ($< 1$ or $> 12$) with `#VALUE!`. Rejected because it breaks standard Excel formula patterns.

### Data Contracts

```rust
/// Represents a decomposed calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTuple {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

/// Core calendar and serial conversion utilities.
pub struct DateSerial;

impl DateSerial {
    /// Converts a year, month, and day into an Excel serial day number.
    pub fn from_ymd(year: i32, month: i32, day: i32) -> Result<f64, ExcelError>;

    /// Converts an Excel serial date number into a decomposed calendar date.
    pub fn to_ymd(serial: f64) -> Result<DateTuple, ExcelError>;

    /// Returns the number of days in a given month of a given year (1-based month).
    pub fn days_in_month(year: i32, month: u32) -> u32;

    /// Checks whether a year is a leap year under the Gregorian calendar.
    pub fn is_leap_year(year: i32) -> bool;
}

/// Day-count conventions supported by YEARFRAC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DayCountBasis {
    /// US (NASD) 30/360 (basis: 0).
    Us30_360,
    /// Actual/Actual (basis: 1).
    ActualActual,
    /// Actual/360 (basis: 2).
    Actual360,
    /// Actual/365 (basis: 3).
    Actual365,
    /// European 30/360 (basis: 4).
    European30_360,
}
```

### Risks & Trade-offs

- **Actual/Actual edge cases in YEARFRAC**: Excel's Actual/Actual implementation has specific conventions when periods span leap and non-leap years. Mitigation: Accurately reproduce Excel's multi-year average annual day count and within-year leap day containment logic with rigorous test cases.
- **February 29 handling in EDATE**: Stepping 1 year from `2024-02-29` lands on `2025-02-28`. Mitigation: End-of-month clamping algorithm verifies target month length and clamps appropriately.

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: DATE constructor and 1900 leap year bug**
  - **Given** year `1900`, month `2`, day `29`
  - **When** evaluating `DATE(1900, 2, 29)`
  - **Then** evaluates to `CellValue::Number(60.0)`.
  - **When** evaluating `DATE(1900, 1, 1)`
  - **Then** evaluates to `CellValue::Number(1.0)`.
  - **When** evaluating `DATE(1970, 1, 1)` (Unix epoch)
  - **Then** evaluates to `CellValue::Number(25569.0)`.

- [x] **Scenario 2: DATE rollover with out-of-range month and day**
  - **Given** year `2024`, month `13`, day `1`
  - **When** evaluating `DATE(2024, 13, 1)`
  - **Then** evaluates to serial date for `2025-01-01` (`45658.0`).
  - **When** evaluating `DATE(2024, 1, 0)`
  - **Then** evaluates to serial date for `2023-12-31` (`45291.0`).
  - **When** evaluating `DATE(2024, 0, 1)`
  - **Then** evaluates to serial date for `2023-12-01` (`45261.0`).

- [x] **Scenario 3: 2-digit year expansion in DATE**
  - **Given** year `100`, month `1`, day `1`
  - **When** evaluating `DATE(100, 1, 1)`
  - **Then** evaluates to `2000-01-01` (`36526.0`) because years $0 \le y \le 1899$ add 1900.
  - **When** evaluating `DATE(-1, 1, 1)` or `DATE(10000, 1, 1)`
  - **Then** returns `ExcelError::Num`.

- [x] **Scenario 4: Date component extraction (YEAR, MONTH, DAY)**
  - **Given** serial date `25569.0` (`1970-01-01`)
  - **When** evaluating `YEAR(25569.0)`, `MONTH(25569.0)`, `DAY(25569.0)`
  - **Then** returns `1970.0`, `1.0`, and `1.0`.
  - **Given** serial date `60.0`
  - **When** evaluating `YEAR(60.0)`, `MONTH(60.0)`, `DAY(60.0)`
  - **Then** returns `1900.0`, `2.0`, and `29.0`.
  - **Given** serial date with fractional time `25569.75`
  - **Then** returns identical integer components as `25569.0`.

- [x] **Scenario 5: DAYS difference calculation**
  - **Given** `end_date = 25579.0` and `start_date = 25569.0`
  - **When** evaluating `DAYS(end_date, start_date)`
  - **Then** evaluates to `CellValue::Number(10.0)`.
  - **When** evaluating `DAYS(start_date, end_date)`
  - **Then** evaluates to `CellValue::Number(-10.0)`.
  - **When** evaluating with dates outside `0.0..=2958465.0`
  - **Then** returns `ExcelError::Num`.

- [x] **Scenario 6: EDATE month shifting and clamping**
  - **Given** start date `DATE(2024, 1, 31)`
  - **When** evaluating `EDATE(start_date, 1)`
  - **Then** clamps to `2024-02-29` (leap year).
  - **When** evaluating `EDATE(DATE(2023, 1, 31), 1)`
  - **Then** clamps to `2023-02-28` (non-leap year).
  - **When** evaluating `EDATE(DATE(2024, 3, 31), -1)`
  - **Then** evaluates to `2024-02-29`.

- [x] **Scenario 7: EOMONTH month-end calculation**
  - **Given** start date `DATE(2024, 1, 15)`
  - **When** evaluating `EOMONTH(start_date, 0)`
  - **Then** evaluates to `2024-01-31`.
  - **When** evaluating `EOMONTH(start_date, 1)`
  - **Then** evaluates to `2024-02-29`.
  - **When** evaluating `EOMONTH(start_date, -1)`
  - **Then** evaluates to `2023-12-31`.

- [x] **Scenario 8: YEARFRAC across all 5 basis conventions**
  - **Given** `start_date = DATE(2024, 1, 1)` and `end_date = DATE(2024, 7, 1)`
  - **When** evaluating `YEARFRAC(start_date, end_date, 0)` (US 30/360)
  - **Then** evaluates to $180 / 360 = 0.50$.
  - **When** evaluating `YEARFRAC(start_date, end_date, 1)` (Actual/Actual)
  - **Then** evaluates to $182 / 366 \approx 0.4972677595628415$.
  - **When** evaluating `YEARFRAC(start_date, end_date, 2)` (Actual/360)
  - **Then** evaluates to $182 / 360 \approx 0.5055555555555555$.
  - **When** evaluating `YEARFRAC(start_date, end_date, 3)` (Actual/365)
  - **Then** evaluates to $182 / 365 \approx 0.4986301369863014$.
  - **When** evaluating `YEARFRAC(start_date, end_date, 4)` (European 30/360)
  - **Then** evaluates to $180 / 360 = 0.50$.
  - **When** evaluating with invalid basis `5` or `-1`
  - **Then** returns `ExcelError::Num`.
  - **When** evaluating with `start_date > end_date`
  - **Then** evaluates to the identical positive fraction.

---

## Tasks

- [x] **Task 1: Implement core calendar & 1900 date system engine (`src/functions/date/serial.rs`)**
  - [x] Implement `DateTuple` and `DateSerial` conversions with 1900 leap day bug handling.
  - [x] Implement month/day rollover normalization.
  - [x] Implement month-end day count calculation and leap year checks.
- [x] **Task 2: Implement date constructors (`src/functions/date/constructors.rs`)**
  - [x] Implement `DateFunction` (`DATE`).
  - [x] Implement `EdateFunction` (`EDATE`) with end-of-month clamping.
  - [x] Implement `EomonthFunction` (`EOMONTH`).
- [x] **Task 3: Implement component extractors and difference (`src/functions/date/parts.rs`)**
  - [x] Implement `YearFunction` (`YEAR`).
  - [x] Implement `MonthFunction` (`MONTH`).
  - [x] Implement `DayFunction` (`DAY`).
  - [x] Implement `DaysFunction` (`DAYS`) with `(end_date, start_date)` ordering.
- [x] **Task 4: Implement contractual year fractions (`src/functions/date/yearfrac.rs`)**
  - [x] Implement `DayCountBasis` enum with validation.
  - [x] Implement US 30/360 NASD (Basis 0).
  - [x] Implement Actual/Actual (Basis 1).
  - [x] Implement Actual/360 (Basis 2).
  - [x] Implement Actual/365 (Basis 3).
  - [x] Implement European 30E/360 (Basis 4).
  - [x] Implement order-independent `YearfracFunction`.
- [x] **Task 5: Registry wiring & test verification**
  - [x] Wire `date` module into `src/functions/mod.rs` and `FunctionRegistry::standard()`.
  - [x] Create comprehensive integration test suite `tests/function_date_test.rs`.
  - [x] Update function count assertions across test suite (from 35 to 43).
  - [x] Verify using standard Cargo suite (`check`, `test`, `clippy`, `fmt`).
- [x] **Task 6: Reconcile living specs & archive draft**
  - [x] Update `specs/living-specs/functions/registry.md` and `docs/functions/catalog.md`.
  - [x] Move draft spec to `specs/history/`.
