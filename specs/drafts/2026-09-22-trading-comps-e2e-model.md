---
title: Public Trading Comps Model End-to-End Test Fixture
date: 2026-09-22
status: draft
target-living-spec: specs/living-specs/compiler/workbook-emission.md
---

## Proposal

### What

Implement a flagship institutional **Public Trading Comparables ("Trading Comps")** model fixture and end-to-end test suite (`tests/fixtures/trading_comps.rs` and `tests/trading_comps_test.rs`), wire it into workbook emission tests (`tests/export_integration_test.rs`), and register it in the export runner (`examples/export_models.rs`) to export `output/trading_comps_model.xlsx`.

### Why

- **Closing the Vertical Table Architectural Gap**: All four existing export fixtures (`three_statement`, `lbo`, `styling_showcase`, and `parity_suite`) are predominantly horizontal time-series models where columns represent timeline periods and formulas broadcast horizontally via `@col`.
- **Vertical Calculation Direction**: In financial institutions, Trading Comps and Cap Tables are the definitive vertical models:
  - Rows represent discrete entities (peer companies, shareholder classes).
  - Columns represent distinct multi-tier financial measures (market data, balance sheet, operating metrics, valuation multiples).
  - Formula calculations broadcast vertically across entity rows using active row coordinates (`@row`).
  - Summary benchmark rows aggregate vertical ranges using statistical functions (`AVERAGE`, `MEDIAN`, `MIN`, `MAX`).
- **Comprehensive Compiler Stress Test**: This fixture exercises multiple compiler features in a single realistic model:
  1. Multiple descriptor columns (`Company`, `Ticker`) side-by-side.
  2. Multi-tier asymmetric column headers (`Market Data`, `Balance Sheet`, `Operating Metrics`, `Valuation Multiples`) with `CenterAcross` selection.
  3. Intra-table column gutters separating distinct thematic financial groups.
  4. Column-vector formula broadcasting with `@row` active coordinates.
  5. Vertical range summary rows testing `MEDIAN`, `AVERAGE`, `MIN`, and `MAX`.
  6. Heterogeneous column format presets (`Currency`, `Number`, `Percentage`, `Multiple`) within the same row.
  7. Cross-table valuation benchmarking in a stacked secondary table on the same worksheet.

### Scope

- **In-scope**:
  - `tests/fixtures/trading_comps.rs`: Builder function `build_trading_comps_model()` creating the complete 2-table model.
  - `tests/fixtures/mod.rs`: Re-export `trading_comps` module.
  - `tests/trading_comps_test.rs`: Dedicated evaluation test asserting exact mathematical values, multiples, summary statistics, and target valuation.
  - `tests/export_integration_test.rs`: Add `trading_comps` to in-memory workbook emission and disk export scenarios with XML structure assertions.
  - `examples/export_models.rs`: Add `trading_comps_model.xlsx` to the export runner.
- **Out-of-scope**:
  - Modifying compiler core passes (Pass 1 through Pass 6 already architecturally support all required primitives).
  - Horizontal multi-table placement (tables are stacked vertically on the sheet using Pass 5 table stacking).

### What Changes

- **New files**:
  - `tests/fixtures/trading_comps.rs`
  - `tests/trading_comps_test.rs`
- **Modified files**:
  - `tests/fixtures/mod.rs`
  - `tests/export_integration_test.rs`
  - `examples/export_models.rs`

### Capabilities

- **New capabilities**:
  - End-to-end verification of vertical table calculations (`@row` active coordinates) through spatial layout and OpenXML workbook emission.
  - Verification of `MEDIAN` and `AVERAGE` statistical aggregations over vertical range slices in physical Excel sheets.
  - End-to-end verification of `FormatPreset::Multiple` (`#,#0.0"x"`) rendering in exported workbooks.
  - Cross-table benchmark multiple references (`Comps["median", "ev_ebitda"]`) lowered into concrete pinned Excel cell coordinates.

---

## Design

### Goals

1. **Institutional Modeling Realism**: Follow Wall Street investment banking standards for Public Trading Comparables (clean visual hierarchy, multiple metrics, summary statistics, valuation benchmarking).
2. **Deterministic Layout & Zero Panics**: Clean compilation through all 6 passes with zero errors and no unwraps in production code.
3. **Exact Mathematical Integrity**: All formulas (`Market Cap`, `EV`, `EBITDA Margin`, `EV/Rev`, `EV/EBITDA`, `P/E`, `Mean`, `Median`, `Implied Valuation`) evaluate with exact floating-point precision.

### Model Architecture & Invariants

#### 1. Peer Trading Comps Table (`Comps`)
- **Columns (15 physical columns)**:
  - `company` (Descriptor): "Company"
  - `ticker` (Descriptor): "Ticker"
  - Gutter (`ColumnSpec::gutter()`)
  - Market Data (`["Market Data", ...]`, 3 cols):
    - `share_price` (Currency, 2 dec)
    - `shares_out` (Number, 1 dec, in millions)
    - `market_cap` (Currency, 1 dec, formula: `Comps[@row, "share_price"] * Comps[@row, "shares_out"]`)
  - Gutter (`ColumnSpec::gutter()`)
  - Balance Sheet (`["Balance Sheet", ...]`, 3 cols):
    - `cash` (Currency, 1 dec)
    - `debt` (Currency, 1 dec)
    - `ev` (Currency, 1 dec, formula: `Comps[@row, "market_cap"] + Comps[@row, "debt"] - Comps[@row, "cash"]`)
  - Gutter (`ColumnSpec::gutter()`)
  - Operating Metrics (`["Operating Metrics", ...]`, 3 cols):
    - `revenue` (Currency, 1 dec)
    - `ebitda` (Currency, 1 dec)
    - `ebitda_margin` (Percentage, 1 dec, formula: `Comps[@row, "ebitda"] / Comps[@row, "revenue"]`)
  - Gutter (`ColumnSpec::gutter()`)
  - Valuation Multiples (`["Valuation Multiples", ...]`, 3 cols):
    - `ev_rev` (Multiple, 1 dec, formula: `Comps[@row, "ev"] / Comps[@row, "revenue"]`)
    - `ev_ebitda` (Multiple, 1 dec, formula: `Comps[@row, "ev"] / Comps[@row, "ebitda"]`)
    - `pe` (Multiple, 1 dec, formula: `Comps[@row, "market_cap"] / (Comps[@row, "ebitda"] * 0.75)`)
- **Entity Rows (6 SaaS Peers)**:
  - `snow` (Snowflake): $150.00 / 330.0M shares / $3,500M cash / $0M debt / $3,200M rev / $350M EBITDA
  - `ddog` (Datadog): $120.00 / 325.0M shares / $2,800M cash / $0M debt / $2,600M rev / $600M EBITDA
  - `mdb` (MongoDB): $280.00 / 75.0M shares / $1,900M cash / $0M debt / $1,850M rev / $280M EBITDA
  - `net` (Cloudflare): $85.00 / 340.0M shares / $1,400M cash / $1,200M debt / $1,650M rev / $210M EBITDA
  - `crwd` (CrowdStrike): $310.00 / 245.0M shares / $3,700M cash / $750M debt / $3,450M rev / $900M EBITDA
  - `estc` (Elastic): $105.00 / 100.0M shares / $1,000M cash / $570M debt / $1,300M rev / $195M EBITDA
- **Separators & Summary Rows**:
  - Explicit row gutter (`RowSpec::Gutter`)
  - `mean` (`SemanticStyle::Summary`): `AVERAGE(Comps["snow".."estc", @col])` across measures
  - `median` (`SemanticStyle::Summary`): `MEDIAN(Comps["snow".."estc", @col])` across measures
  - `max` (`SemanticStyle::Data`): `MAX(Comps["snow".."estc", @col])` across measures
  - `min` (`SemanticStyle::Data`): `MIN(Comps["snow".."estc", @col])` across measures
  - *Note*: Summary rows populate formulas as explicit cell inputs across measure columns (and set descriptor label on `company`) to adhere to Tier 1 precedence and avoid `AmbiguousExpressionConflict` with column-vector formulas.

#### 2. Target Valuation Benchmarking Table (`TargetValuation`)
- Stacked below `Comps` table with inter-table margin.
- 2 columns: `metric` (Descriptor), `val` (Measure).
- Line items:
  - Inputs: Target Revenue ($250.0M), Target EBITDA ($45.0M), Target Debt ($20.0M), Target Cash ($15.0M).
  - Benchmarks:
    - `peer_median_ev_rev`: `Comps["median", "ev_rev"]`
    - `peer_median_ev_ebitda`: `Comps["median", "ev_ebitda"]`
  - Implied Valuations:
    - `implied_ev_rev`: `TargetValuation["target_rev", "val"] * TargetValuation["peer_median_ev_rev", "val"]`
    - `implied_ev_ebitda`: `TargetValuation["target_ebitda", "val"] * TargetValuation["peer_median_ev_ebitda", "val"]`
    - `implied_ev_blended`: `AVERAGE(TargetValuation["implied_ev_rev".."implied_ev_ebitda", "val"])`
    - `implied_equity_val`: `TargetValuation["implied_ev_blended", "val"] - TargetValuation["target_debt", "val"] + TargetValuation["target_cash", "val"]`

---

## Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Mathematical Accuracy of Vertical Formulas & Summary Rows**
  - **Given** the trading comps model specification
  - **When** evaluated through `Compiler::evaluate`
  - **Then** all column formulas calculate exact market cap, EV, margins, and multiples; `mean`, `median`, `max`, and `min` rows evaluate accurately; and target implied equity value resolves without errors.

- [ ] **Scenario 2: OpenXML Package Assembly & Emission**
  - **Given** the evaluated model
  - **When** emitted via `Compiler::emit_workbook`
  - **Then** produces a valid `.xlsx` buffer containing `<f>` tags for `@row` lowered formulas (e.g. `D6*E6`), 2-tier super-headers with `CenterAcross`, and `<f>` tags for vertical ranges (e.g. `MEDIAN(F$6:F$11)`).

- [ ] **Scenario 3: Physical Disk Export & Integration Runner**
  - **Given** `export_models` runner and `export_integration_test`
  - **When** executed
  - **Then** exports `output/trading_comps_model.xlsx` with clean sheet-wide column width arbitration across descriptors, gutters, and measures.

---

## Tasks

### Phase 1: Fixture Implementation
- [ ] Create `tests/fixtures/trading_comps.rs` with `build_trading_comps_model()`.
- [ ] Re-export `trading_comps` in `tests/fixtures/mod.rs`.
- [ ] Create dedicated evaluation test `tests/trading_comps_test.rs` validating mathematical precision.

### Phase 2: Integration Test & Export Runner
- [ ] Add `trading_comps` scenario to `tests/export_integration_test.rs` (in-memory buffer and disk export).
- [ ] Register `trading_comps_model.xlsx` in `examples/export_models.rs`.
- [ ] Run disk export to generate `output/trading_comps_model.xlsx`.

### Phase 3: Verification & Reconcile
- [ ] Run full toolchain check: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- [ ] Update living spec `specs/living-specs/compiler/workbook-emission.md` and archive draft spec to `specs/history/`.
