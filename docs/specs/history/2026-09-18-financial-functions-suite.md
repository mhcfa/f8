---
title: Financial Functions Suite
date: 2026-09-18
status: completed
target-living-spec: specs/living-specs/functions/registry.md
---

## Proposal

### What

Implement the seven standard Excel financial functions specified in `docs/functions/catalog.md`:
- Periodic and non-periodic discounting: `NPV`, `XNPV`
- Periodic and non-periodic internal rate of return: `IRR`, `XIRR`
- Fixed-rate annuity and debt amortization: `PMT`, `IPMT`, `PPMT`

Organize these under a modular `src/functions/financial/` domain package, implement robust numeric solvers with Excel parity convergence safeguards, register all seven functions into `FunctionRegistry::standard()`, and verify end-to-end with unit and integration tests.

### Why

1. **Core financial modeling capabilities**: Corporate finance, project finance, debt schedules, DCF models, and LBO/private equity returns rely heavily on discounted cash flow metrics (`NPV`, `XNPV`, `IRR`, `XIRR`) and debt service schedules (`PMT`, `IPMT`, `PPMT`).
2. **Strict Excel parity**: Financial models require exact alignment with Excel's convention for periodic vs. date-based discounting (Actual/365), cash flow sign rules (outflows negative, inflows positive), payment timing (`type = 0` end vs. `type = 1` start), and Newton-Raphson root finding.
3. **Robustness & zero production panics**: Numerical algorithms (Newton-Raphson for `IRR`/`XIRR`, power series for annuity equations) must handle boundary conditions (rates $\le -1.0$, non-convergent cash flows, zero interest rates, non-existent sign changes) gracefully by returning structured domain errors (`#NUM!`, `#DIV/0!`, `#VALUE!`).

### Scope

- **In-scope**:
  - `src/functions/financial/mod.rs`: Module exports and `register(r: &mut FunctionRegistry)` for the financial suite.
  - `src/functions/financial/discounting.rs`: Implement `NPV` and `XNPV`.
  - `src/functions/financial/irr.rs`: Implement `IRR` and `XIRR` using Newton-Raphson root finding with step guards and fallback tolerances.
  - `src/functions/financial/annuity.rs`: Implement `PMT`, `IPMT`, and `PPMT` with full period-timing (`type = 0` and `type = 1`) support and closed-form amortization balance tracking.
  - `src/functions/mod.rs` & `src/functions/registry.rs`: Expose `finx::functions::financial` and wire `register` into `FunctionRegistry::standard()`.
  - Update standard registry count assertions from 20 to 27 across tests (`tests/function_registry_test.rs`, `tests/function_math_test.rs`).
  - Comprehensive integration test suite in `tests/function_financial_test.rs`.
- **Out-of-scope**:
  - Lookup functions (`XLOOKUP`, `INDEX`).
  - Date & time functions (`YEARFRAC`, `DATE`, `EOMONTH`, etc.).
  - Volatile functions (`TODAY`, `NOW`, `RAND`).

### What Changes

- `src/functions/financial/discounting.rs`: New file with `NpvFunction` and `XnpvFunction`.
- `src/functions/financial/irr.rs`: New file with `IrrFunction` and `XirrFunction` and shared numerical solver helpers.
- `src/functions/financial/annuity.rs`: New file with `PmtFunction`, `IpmtFunction`, and `PpmtFunction`.
- `src/functions/financial/mod.rs`: New file exporting structs and implementing `register`.
- `src/functions/mod.rs`: Declare `pub mod financial;`.
- `src/functions/registry.rs`: Call `crate::functions::financial::register(&mut registry)` in `FunctionRegistry::standard()`.
- `tests/function_financial_test.rs`: New comprehensive integration tests covering all seven financial functions.
- `tests/function_registry_test.rs` & `tests/function_math_test.rs`: Update standard registry count assertion to 27.

### Capabilities

- **New capabilities**:
  - `NPV`: Calculates net present value of periodic cash flows starting at $t=1$; ignores text/empty in ranges; coerces scalar arguments; handles $r = -1.0$ as `#DIV/0!`.
  - `XNPV`: Calculates net present value of non-periodic cash flows based on Actual/365 year fractions from $d_1$; validates equal array lengths, monotonic dates ($d_i \ge d_1$), and rate $r > -1.0$ (returning `#NUM!` otherwise).
  - `IRR`: Solves for periodic internal rate of return via Newton-Raphson starting at $t=0$; enforces at least one positive and one negative cash flow (returning `#NUM!` otherwise); accepts optional initial guess (default `0.10`).
  - `XIRR`: Solves for non-periodic internal rate of return using Actual/365 day counts; enforces equal lengths, $d_i \ge d_1$, and sign diversity (returning `#NUM!` otherwise); accepts optional initial guess (default `0.10`).
  - `PMT`: Evaluates constant periodic debt service / annuity payment given `rate`, `nper`, `pv`, optional `fv` (default `0.0`), and optional `type` (`0` for period end, `1` for period start); handles `rate = 0.0` linearly.
  - `IPMT`: Evaluates interest portion of periodic annuity payment for target period `per` ($1 \le per \le nper$); validates period bounds (returning `#NUM!` if out of bounds); handles $r = 0.0$ as `0.0` interest; correctly calculates period-end and period-start interest accruals.
  - `PPMT`: Evaluates principal portion of periodic annuity payment such that $\text{PMT} = \text{IPMT} + \text{PPMT}$.
- **Modified capabilities**:
  - `FunctionRegistry::standard()` contains 27 built-in functions in deterministic registration order.

## Design

### Goals

1. **Exact Excel mathematical parity**: Conform strictly to the Excel formulas documented in `docs/functions/catalog.md` and standard financial textbooks.
2. **Numerical stability & bounded convergence**: Guard iterative root solvers against division by zero, stationary derivatives ($f'(r) \approx 0$), domain boundaries ($r \le -1.0$), and infinite loops by capping iterations at 100 with $10^{-9}$ tolerance.
3. **Amortization invariant**: Ensure $\text{PPMT}(r, k, n, pv, fv, t) + \text{IPMT}(r, k, n, pv, fv, t) \equiv \text{PMT}(r, n, pv, fv, t)$ across all valid periods, rates, and payment timings.
4. **Rust 2024 compliance**: Zero production `.unwrap()` or `.expect()`, clean error propagation via `?`, immutable bindings, and zero compiler or clippy warnings.

### Invariants

- **Cash flow sign diversity**: Both `IRR` and `XIRR` require at least one positive cash flow ($> 0.0$) and at least one negative cash flow ($< 0.0$). If all cash flows share the same sign or are zero, they immediately return `ExcelError::Num`.
- **Date ordering in non-periodic functions**: For `XNPV` and `XIRR`, every date $d_i$ must be on or after the initial date $d_1$ ($d_i \ge d_1$). If any date precedes $d_1$, return `ExcelError::Num`.
- **Equal dimensions**: For `XNPV` and `XIRR`, `values` and `dates` must contain the exact same number of elements ($N \ge 1$ for `XNPV`, $N \ge 2$ for `XIRR`). If lengths mismatch, return `ExcelError::Num`.
- **Amortization bounds**: For `IPMT` and `PPMT`, target period `per` must satisfy $1 \le per \le nper$. Out-of-bounds periods return `ExcelError::Num`. $nper \le 0$ returns `ExcelError::Num`.

### Decisions

- **Decision 1: Cash flow traversal in NPV vs IRR**:
  - `NPV`: Parameter `rate` is scalar (`ParamKind::Scalar`), while trailing arguments are `ParamKind::RangeOrArray` with `VariadicRule::Single`. Cash flows start discounting at period $t=1$. Range references ignore text, booleans, and empty cells. Direct scalars coerce booleans to 1.0/0.0, empty to 0.0, and error on text (`#VALUE!`).
  - `IRR`: Parameter `values` is `ParamKind::RangeOrArray`. Cash flows are ordered $t = 0, 1, \dots, n-1$. Range references ignore non-numeric entries.
- **Decision 2: Newton-Raphson root finding for IRR & XIRR**:
  - Initial estimate: user-supplied `guess` (default `0.10`).
  - Maximum iterations: 100.
  - Convergence tolerance: $|f(r)| < 1e-9$ or $|\Delta r| < 1e-9$.
  - Domain guard: Since $(1+r)$ must remain strictly positive for non-integer powers $(d_i - d_1)/365.0$, any step taking $r \le -0.999999$ is clamped or bisected towards a valid neighborhood.
  - If the solver does not converge or derivative $|f'(r)| < 1e-14$ cannot be navigated, return `ExcelError::Num`.
- **Decision 3: Closed-form annuity formulas for IPMT & PPMT**:
  - For $r = 0$: $\text{PMT} = -(pv + fv) / nper$, $\text{IPMT} = 0.0$, $\text{PPMT} = \text{PMT}$.
  - For $r \neq 0$:
    - When $type = 0$ (end of period):
      $$\text{IPMT}_k = - [ pv \cdot (1+r)^{k-1} \cdot r + pmt \cdot ((1+r)^{k-1} - 1) ]$$
    - When $type = 1$ (beginning of period):
      For $k = 1$: $\text{IPMT}_1 = 0.0$.
      For $k > 1$:
      $$\text{IPMT}_k = - [ (pv + pmt) \cdot (1+r)^{k-2} \cdot r + pmt \cdot ((1+r)^{k-2} - 1) ]$$
    - In all cases: $\text{PPMT}_k = pmt - \text{IPMT}_k$.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: `NPV` periodic discounting and scalar coercion**
  - **Given** discount rate and cash flows (scalars or range reference)
  - **When** evaluated with `NPV`
  - **Then** discounts cash flows starting at $t=1$; ignores text/blanks in ranges; bubbles errors; returns `#DIV/0!` if rate is `-1.0`.
- [x] **Scenario 2: `XNPV` non-periodic discounting with dates**
  - **Given** rate, cash flows array, and corresponding serial dates
  - **When** evaluated with `XNPV`
  - **Then** discounts using $(d_i - d_1)/365.0$; returns `#NUM!` if lengths differ, if $d_i < d_1$, or if rate $\le -1.0$.
- [x] **Scenario 3: `IRR` periodic internal rate of return**
  - **Given** series of cash flows containing both negative outlays and positive inflows
  - **When** evaluated with `IRR`
  - **Then** finds the periodic rate where NPV is zero; returns `#NUM!` if cash flows lack sign variation or fail to converge.
- [x] **Scenario 4: `XIRR` non-periodic internal rate of return**
  - **Given** non-periodic cash flows and dates with at least one negative and one positive entry
  - **When** evaluated with `XIRR`
  - **Then** finds the annualized rate satisfying $\text{XNPV} = 0$; returns `#NUM!` if all cash flows have the same sign or dates are invalid.
- [x] **Scenario 5: `PMT` annuity payments with type 0 and 1**
  - **Given** `rate`, `nper`, `pv`, and optional `fv` and `type`
  - **When** evaluated with `PMT`
  - **Then** evaluates correct periodic payment for both period-end (`type = 0`) and period-start (`type = 1`); handles zero interest rate linearly.
- [x] **Scenario 6: `IPMT` and `PPMT` amortization breakdown**
  - **Given** loan parameters and period `per`
  - **When** evaluated with `IPMT` and `PPMT`
  - **Then** $\text{IPMT} + \text{PPMT} \equiv \text{PMT}$; returns `#NUM!` if $per < 1$ or $per > nper$; produces zero interest for $k=1$ when $type=1$.

## Tasks

- [x] 1. Create `src/functions/financial/discounting.rs` implementing `NPV` and `XNPV`.
- [x] 2. Create `src/functions/financial/irr.rs` implementing `IRR` and `XIRR` with Newton-Raphson solver.
- [x] 3. Create `src/functions/financial/annuity.rs` implementing `PMT`, `IPMT`, and `PPMT`.
- [x] 4. Create `src/functions/financial/mod.rs` exporting structs and implementing `register`.
- [x] 5. Wire `src/functions/financial` into `src/functions/mod.rs` and `FunctionRegistry::standard()`.
- [x] 6. Update registry count assertions in existing test files.
- [x] 7. Create comprehensive test suite in `tests/function_financial_test.rs`.
- [x] 8. Verify using `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`.
- [x] 9. Reconcile living spec `specs/living-specs/functions/registry.md` and archive draft to `specs/history/`.
