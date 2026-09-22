---
title: Excel Parity Verification Suite & Emission Mode
date: 2026-09-22
status: completed
target-living-spec: specs/living-specs/compiler/workbook-emission.md
---

## Proposal

### What

Design and build an end-to-end calculation parity verification model covering all 43 registered standard Excel functions across 5 categories (`Math`, `Logical`, `Financial`, `Lookup`, `Date`). Introduce an explicit `omit_formula_cache` flag in `EmissionOptions` to support emitting formulas without cached `<v>` result tags, forcing Microsoft Excel to compute formulas from scratch upon opening. Document the roadmap for subsequent automated golden reference assertions (Option 2).

### Why

- **Validate Excel calculation parity**: Finx guarantees calculation parity with Microsoft Excel (documented in `docs/compiler/parity.md`). However, current integration tests only assert Finx's internal AST evaluation against hardcoded constants, while emitted OpenXML workbooks embed Finx's pre-calculated results into `<v>` tags.
- **Side-by-side visual auditability**: Modelers and engineers need a single comprehensive reference spreadsheet where every supported function is evaluated alongside a static baseline and an automated Excel parity check formula (`=B=C`).
- **Uncached formula emission**: Opening workbooks in Microsoft Excel for the Web or desktop where `<v>` is absent forces Microsoft's calculation engine (ECS) to evaluate formulas independently.

### Scope

- **In-scope (Phase 1: Verification Suite & Uncached Emission)**:
  - Add `omit_formula_cache: bool` to `EmissionOptions` in `src/compiler/emission/workbook.rs`.
  - Update `WorkbookEmitter` to suppress calling `formula.set_result(...)` when `omit_formula_cache` is `true`.
  - Create a canonical parity test fixture in `tests/fixtures/parity_suite.rs` with test cases covering all 43 functions across all 5 functional domains.
  - Add integration tests in `tests/export_integration_test.rs` validating that uncached workbooks emit valid OpenXML `<f>` tags without `<v>` tags.
  - Provide an export runner or command to produce `target/test-output/excel_parity_verification.xlsx` for Web Excel inspection.
- **Out-of-scope / Deferred (Phase 2: Automated Golden Reference Assertion)**:
  - Downloading and committing the Web Excel recalculated workbook (`tests/fixtures/excel_parity_ground_truth.xlsx`).
  - Automated comparison test asserting `finx_evaluated == excel_recalculated_v`.

### What Changes

- **`src/compiler/emission/workbook.rs`**:
  - Add `pub omit_formula_cache: bool` to `EmissionOptions`.
  - Conditionally skip `formula.set_result(...)` during formula emission when `omit_formula_cache` is `true`.
- **`tests/fixtures/parity_suite.rs`**:
  - Construct a structured multi-table model containing test tables for `Math`, `Logical`, `Financial`, `Lookup`, and `Date`.
  - Each table includes columns: `description`, `formula_result`, `finx_baseline`, `parity_check`.
- **`tests/export_integration_test.rs`**:
  - Add test validating uncached formula emission.
  - Add physical disk export target in `test_export_xlsx_to_disk`.

### Capabilities

- **New capabilities**:
  - Ability to emit Excel workbooks with empty formula cache tags (`omit_formula_cache: true`).
  - Comprehensive 43-function parity verification fixture ready for visual audit in Web Excel.
- **Modified capabilities**:
  - `EmissionOptions` gains a non-breaking optional flag defaulting to `false`.

---

## Design

### Goals

1. **Complete functional coverage**: Exercise all 43 registered functions across Math, Logical, Financial, Lookup, and Date categories.
2. **Three-way column layout**:
   - `formula_col`: Emits the lowered Excel formula (e.g. `=SUM(10, 20)`).
   - `finx_col`: Emits Finx's statically evaluated literal value (e.g. `30.0`).
   - `check_col`: Emits an Excel comparison formula (e.g. `=IF(ISNUMBER(formula_col), ABS(formula_col - finx_col) < 1e-6, formula_col = finx_col)`).
3. **Deterministic compilation**: Cleanly plan layout, resolve symbols, evaluate topological DAG, and emit OpenXML with zero warnings or panics.
4. **Clean separation of phase 1 and phase 2**: Support immediate visual verification in Web Excel without requiring binary fixture commits in phase 1.

### Invariants

- Default behavior of `EmissionOptions` must remain backwards-compatible (`omit_formula_cache: false`), preserving cached values in production exports.
- All cell coordinates in parity check formulas must resolve dynamically through spatial layout planning.
- Parity check formulas must handle numeric floating-point tolerances ($\epsilon \le 10^{-6}$) and string/boolean exact equality.

### Decisions

- **Decision: Add `omit_formula_cache` to `EmissionOptions`**:
  - *Rationale*: Allows explicit caller control over whether cached values are written into `<v>` tags without altering default production behavior.
  - *Alternatives considered*: Passing `evaluated: None` to `emit_workbook`; rejected because `Compiler::save_workbook` automatically triggers evaluation when `None` is provided, and `finx_col` requires evaluated values to write static literals.
- **Decision: Group functions into modular category tables**:
  - *Rationale*: Placing 43 functions into five dedicated category tables (`MathParity`, `LogicalParity`, `FinancialParity`, `LookupParity`, `DateParity`) prevents massive unwieldy tables and aligns with Finx's function module structure.
  - *Alternatives considered*: Single monolithic 43-row table; rejected because financial/date functions require dedicated input ranges and date seed cells.

### Data Contracts

#### `EmissionOptions` Addition

```rust
pub struct EmissionOptions {
    /// When true, emits evaluated values instead of formulas (frozen audit workbook).
    pub values_only: bool,
    /// When true, emits formulas without cached evaluation result tags (<v>).
    pub omit_formula_cache: bool,
    /// Document title metadata in docProps/core.xml.
    pub title: Option<String>,
    /// Document author metadata in docProps/core.xml.
    pub author: Option<String>,
}
```

#### Category Table Structure

Each category sheet or section contains:
1. **Inputs Table**: Dedicated literal cells used as inputs for functions (dates, cash flow vectors, lookup arrays).
2. **Test Matrix Table**:
   - `test_id`: Descriptor identifier (e.g. `"sum_basic"`).
   - `formula_col`: Calculated via function formula expression.
   - `finx_col`: Bound to the literal evaluated result of the same expression.
   - `check_col`: Formula comparing `formula_col` and `finx_col`.

---

## Roadmap: Option 2 (Automated Golden Reference Assertion)

This section documents the future implementation architecture for automated CI verification:

1. **One-time baseline generation**:
   - Export `excel_parity_verification.xlsx` with `omit_formula_cache: true`.
   - Open workbook in Microsoft Excel (or Web Excel in OneDrive).
   - Allow Excel's calculation engine to evaluate all formulas.
   - Download the recalculated workbook and commit to `tests/fixtures/excel_ground_truth.xlsx`.
2. **Automated extraction & assertion**:
   - In `tests/excel_parity_test.rs`:
     - Unpack `xl/worksheets/sheet*.xml` from `excel_ground_truth.xlsx`.
     - Extract Excel's recalculated `<v>` values from each formula cell coordinate.
     - Compile and evaluate the parity model in memory with Finx.
     - Assert that Finx's `CellValue` matches Excel's `<v>` value within numeric $\epsilon = 10^{-6}$.
3. **Zero runtime dependencies**:
   - Once the fixture is saved, all CI assertions run in milliseconds in offline pure Rust on Linux/macOS/Windows.

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Uncached Formula Emission**
  - **Given** an `EmissionOptions` configuration with `omit_formula_cache: true`
  - **When** emitting a model containing formulas to an OpenXML buffer
  - **Then** `xl/worksheets/sheet*.xml` contains `<f>` formula tags and does not contain `<v>` tags with precomputed evaluated values for formula cells.

- [x] **Scenario 2: Full Functional Coverage Fixture**
  - **Given** the 43-function parity fixture model
  - **When** compiled through `Compiler::evaluate`
  - **Then** all 43 functions evaluate with zero domain errors, and `parity_status` evaluates to `"PASS"` across all rows.

- [x] **Scenario 3: Physical Parity Export Generation**
  - **Given** the parity verification fixture
  - **When** running the disk export integration test (`cargo test --test export_integration_test -- --ignored`)
  - **Then** `target/test-output/excel_parity_verification.xlsx` is written to disk with uncached formula cells and hardcoded comparison baselines.

---

## Tasks

### Phase 1: Verification Suite & Uncached Emission

- [x] 1. Add `omit_formula_cache` field and builder method to `EmissionOptions` in `src/compiler/emission/workbook.rs`.
- [x] 2. Update `WorkbookEmitter::emit_cells` to bypass `formula.set_result(...)` when `omit_formula_cache` is enabled.
- [x] 3. Create `tests/fixtures/parity_suite.rs` with tables and test formulas covering all 43 functions across all 5 categories.
- [x] 4. Add unit test in `tests/export_integration_test.rs` asserting `<f>` presence and `<v>` absence when `omit_formula_cache: true`.
- [x] 5. Add physical export runner in `tests/export_integration_test.rs` to generate `target/test-output/excel_parity_verification.xlsx`.
- [x] 6. Run verification suite (`cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`).

### Phase 2: Automated Golden Reference Assertion (Deferred / Future)

- [ ] 1. Open exported `excel_parity_verification.xlsx` in Web Excel / Microsoft Excel to recalculate formulas.
- [ ] 2. Save recalculated workbook to `tests/fixtures/excel_ground_truth.xlsx`.
- [ ] 3. Implement test harness in `tests/excel_parity_test.rs` to extract Excel's `<v>` values and assert equality against Finx evaluation.
