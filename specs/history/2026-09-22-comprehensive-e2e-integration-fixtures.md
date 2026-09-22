---
title: Comprehensive End-to-End Integration Fixtures Upgrade
date: 2026-09-22
status: completed
target-living-spec: specs/living-specs/compiler/workbook-emission.md
---

## Proposal

### What

Upgrade and differentiate the three flagship end-to-end integration test fixtures in `tests/fixtures/` (`styling_showcase.rs`, `lbo.rs`, and `three_statement.rs`) to systematically exercise all compiler capabilities introduced in the explicit grid architectural shift:
1. **`styling_showcase.rs` (visual gallery)**: Add explicit row and column gutters, 3-tier asymmetric headers with `CenterAcross` super-headers, 2D border intersection total boxes, deep typographic indentations (tiers 0–3), and negative accounting format displays.
2. **`lbo.rs` (complex irregular layout and math)**: Add a canonical 7-column Sources & Uses table with intra-table column gutter, multi-tier debt headers, fixed amortization math (`PMT`/`PPMT`/`IPMT`), scenario selection (`SWITCH`/`CHOOSE`), and multi-table sheet column width arbitration.
3. **`three_statement.rs` (corporate statement dynamics)**: Maintain a contiguous timeline projection while adding intra-statement row gutters and standard hierarchical line-item indentations on Income Statement, Cash Flow, and Balance Sheet.

### Why

- **Close gaps from recent architectural shifts**: Recent commits introduced tagged rows/columns (`RowSpec::Gutter`, `ColumnSpec::Gutter`), cell-level indentation (`indent: Option<u8>`), multi-tier column headers, and relative axis navigation. While unit tests verify these in isolation, none of the three physical export fixtures previously exercised row/column gutters, 3-tier headers, or statement line-item indentations.
- **Clear domain separation**: Rather than overlapping or creating redundant tests, each of the three e2e export fixtures specializes in a distinct compiler dimension:
  - `styling_showcase`: Comprehensive visual gallery for all styling, outlines, and formats.
  - `lbo`: Complex mathematical expressions and irregular multi-table sheet layouts.
  - `three_statement`: Clean corporate financial statement hierarchy and time-series projections.
- **Publication-grade exported workbooks**: Spreadsheets exported to `target/test-output/` reflect genuine institutional private equity and Wall Street modeling standards.

### Scope

- **In-scope**:
  - `tests/fixtures/mod.rs`: Add builder helpers for explicit gutters, multi-tier headers, and indentation.
  - `tests/fixtures/styling_showcase.rs`: Add dedicated gutters table, asymmetric/3-tier headers, 2D total boxes, and indentation tiers.
  - `tests/fixtures/lbo.rs`: Add 7-column Sources & Uses table, 3-tier debt headers, amortizing loan schedules (`PMT`/`PPMT`), and scenario switching.
  - `tests/fixtures/three_statement.rs`: Add intra-statement row gutters and hierarchical line-item indentations.
  - `tests/export_integration_test.rs`: Update and expand XML structure assertions (gutters, 3-tier headers, cross-sheet references).
- **Out-of-scope**:
  - Modifications to core compiler passes (Pass 1 through Pass 6 are fully implemented and verified).
  - Creating a fourth export fixture (the three existing fixtures provide complete coverage when upgraded).

### What Changes

- **Modified files**:
  - `tests/fixtures/mod.rs`
  - `tests/fixtures/styling_showcase.rs`
  - `tests/fixtures/lbo.rs`
  - `tests/fixtures/three_statement.rs`
  - `tests/export_integration_test.rs`
  - `tests/three_statement_model_test.rs` (align with updated statement structure)
  - `tests/lbo_model_test.rs` (align with updated LBO structure)

### Capabilities

- **New capabilities**:
  - End-to-end verification of `RowSpec::Gutter` and `ColumnSpec::Gutter` compilation and OpenXML emission.
  - End-to-end verification of 3-tier column headers and asymmetric header depth alignment.
  - End-to-end verification of 7-column irregular table structures with intra-table dividing gutters.
  - End-to-end verification of descriptor cell indentation width expansion on real financial models.
- **Modified capabilities**: None; existing formula evaluation semantics and public compiler APIs remain unchanged.

---

## Design

### Goals

1. **Domain specialization**: Each of the three fixtures has a distinct, non-overlapping architectural focus.
2. **Deterministic layout & zero panics**: All models compile cleanly through all six compiler passes with zero errors and no panics.
3. **Exact calculation integrity**: All evaluated financial figures (net income, cash flow, debt sweep, IRR, MOIC, balance checks) remain mathematically sound and verified.
4. **Institutional aesthetic quality**: Exported `.xlsx` files feature proper visual hierarchy, section spacing, column gutters, and accounting underlines.

### Invariants

- All measure columns begin after descriptor columns and gutters.
- `indent > 0` is strictly restricted to descriptor cells.
- Column gutters allocate width `theme.metrics.gutter_col_width` (3.0 chars) and contain no cell data, formulas, or borders.
- Row gutters allocate height `theme.metrics.gutter_row_height` (9.0 pt) and contain no cell data, formulas, or borders.
- Multi-tier headers allocate `theme.metrics.header_row_height` (24.0 pt) per tier.

### Decisions

- **Decision: Differentiate the 3 existing fixtures instead of adding a 4th test**:
  - *Rationale*: Avoids test sprawl and aligns with `export_integration_test.rs` and `examples/export_models.rs`. The 3 models cleanly cover visual styling, irregular math/layout, and statement flow.
  - *Alternatives considered*: Creating a dedicated "gutters & layout" fixture; rejected because gutters and irregular layouts naturally belong in the LBO summary dashboard and styling showcase.
- **Decision: Represent Sources & Uses as a single 7-column table with an intra-table column gutter**:
  - *Rationale*: A single table `[Sources (3 cols), Gutter (1 col), Uses (3 cols)]` tests intra-table column gutters, side-by-side total rows, and sheet column width arbitration against stacked 2-column and 5-column tables.
  - *Alternatives considered*: Two separate side-by-side tables; rejected because Finx Pass 5 spatial layout currently stacks tables vertically per sheet.
- **Decision: Maintain contiguous timeline with intra-statement row gutters in Three-Statement Model**:
  - *Rationale*: Keeps financial statement time-series columns contiguous while testing intra-statement row gutters and hierarchical cell indentations across Income Statement, Cash Flow, and Balance Sheet.
  - *Alternatives considered*: Adding an actuals-versus-forecast column gutter; evaluated and omitted to keep timeline columns contiguous while relying on the LBO and Styling Showcase fixtures for column gutter coverage.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Styling Showcase Visual Completeness**
  - **Given** the upgraded `styling_showcase` model
  - **When** compiled and emitted to OpenXML
  - **Then** `xl/worksheets/sheet1.xml` contains 3-tier headers, explicit gutter row/column allocations, 2D intersection total boxes, and typographic indentations up to level 3.

- [x] **Scenario 2: LBO Irregular Layout and Advanced Math**
  - **Given** the upgraded `lbo` model with 7-column Sources & Uses, 3-tier debt headers, and amortizing loan math
  - **When** evaluated and emitted through the compiler
  - **Then** all financial functions (`PMT`, `XLOOKUP`, `XIRR`, `XNPV`, `EDATE`) evaluate with exact precision, and sheet column arbitration cleanly handles tables with 2, 5, and 7 columns.

- [x] **Scenario 3: Three-Statement Line-Item Indentation & Statement Hierarchy**
  - **Given** the upgraded `three_statement` model with indented operating expenses, assets, and liabilities, plus intra-statement row gutters
  - **When** compiled through spatial layout and emission
  - **Then** descriptor column auto-width dynamically accommodates indentations, statement balance check equals 0, and row gutters cleanly separate statement major sections.

- [x] **Scenario 4: Verification Suite Clean Run**
  - **Given** all updated fixtures and integration tests
  - **When** running `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check`
  - **Then** all tests pass with zero warnings, zero clippy lints, and valid formatting.

---

## Tasks

### Phase 1: Builder Helpers & Styling Showcase Upgrade
- [x] Extend `tests/fixtures/mod.rs` with helper methods for gutter columns, 3-tier headers, and indentation tiers.
- [x] Update `tests/fixtures/styling_showcase.rs` to include:
  - A dedicated gutters section exercising `RowSpec::Gutter` and `ColumnSpec::Gutter`.
  - 3-tier asymmetric headers with `CenterAcross` super-headers and single-cell super-headers.
  - 2D summary border intersection (Total row x Total column forming a 4-sided total box).
  - Indentation levels 0 through 3 with auto-width expansion verification.
- [x] Verify Phase 1 with `cargo test --test export_integration_test`.

### Phase 2: LBO Irregular Layout & Advanced Math Upgrade
- [x] Update `tests/fixtures/lbo.rs` on sheet `Summary` to include:
  - Canonical 7-column Sources & Uses table with intra-table `ColumnSpec::Gutter`.
  - Scenario selection using `SWITCH` or `CHOOSE` alongside `XLOOKUP`.
- [x] Update `tests/fixtures/lbo.rs` on sheet `Model` to include:
  - 3-tier column headers on the debt schedule.
  - Amortizing loan tranche using `PMT` / `PPMT` / `IPMT` alongside cash sweep mechanics.
  - `RowSpec::Gutter` separating debt tranches and returns schedules.
- [x] Update `tests/lbo_model_test.rs` assertions to validate updated calculations.
- [x] Verify Phase 2 with `cargo test --test lbo_model_test` and `cargo test --test export_integration_test`.

### Phase 3: Three-Statement Statement Hierarchy & Section Spacing Upgrade
- [x] Update `tests/fixtures/three_statement.rs` to include:
  - Typographic cell indentations on sub-items across Income Statement, Cash Flow, and Balance Sheet.
  - `RowSpec::Gutter` between statement major sections.
- [x] Update `tests/three_statement_model_test.rs` assertions to validate updated structure.
- [x] Verify Phase 3 with `cargo test --test three_statement_model_test` and `cargo test --test export_integration_test`.

### Phase 4: End-to-End Verification & Documentation Reconcile
- [x] Expand XML inspection assertions in `tests/export_integration_test.rs` to verify gutter presence, 3-tier header emission, and column width arbitration.
- [x] Execute disk export test: `cargo test --test export_integration_test -- --ignored` to produce physical `.xlsx` files in `target/test-output/`.
- [x] Run full toolchain verification: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- [x] Reconcile and archive draft spec to `specs/history/`.
