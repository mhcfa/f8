---
title: Institutional Financial Model Fixtures Upgrade
date: 2026-09-20
status: completed
target-living-spec: specs/living-specs/compiler/workbook-emission.md
---

## Proposal

### What

Upgrade the shared benchmark test fixtures in `tests/fixtures/` (`mod.rs`, `three_statement.rs`, and `lbo.rs`) to full institutional production quality:
1. **Descriptor columns & line item labels**: Add explicit descriptor columns (`ColumnSpec::descriptor("line_item", "Line Item")` / `ColumnSpec::descriptor("parameter", "Parameter")`) and populate descriptive, human-readable labels for all rows across every table.
2. **Semantic formatting**: Attach appropriate `FormatPreset` (Currency, Percentage, Multiple, Date, Integer), explicit `decimals`, and `SemanticStyle` (`Subtotal`, `Total`) to line items and summary metrics.
3. **Structured hierarchies**: Attach column groups (super-headers) for Historical vs Forecast timelines and collapsible row groups (section headers) for statement categories (e.g. Operating Expenses, Assets, Liabilities & Equity, Senior vs Sub Debt).
4. **End-to-end alignment**: Guarantee that in-memory evaluation tests (`tests/three_statement_model_test.rs`, `tests/lbo_model_test.rs`), emission integration tests (`tests/export_integration_test.rs`), and the physical `.xlsx` exporter (`examples/export_models.rs`) all pass with 100% mathematical precision and generate publication-grade Excel workbooks with visible line items.

### Why

- **Visible Line Items in Spreadsheets**: Currently, the shared fixtures contain only measure timeline columns (`y0..y5`, `y2023a..y2026e`, `val`). Because Finx strictly requires explicit column declarations without implicit stubs, the exported spreadsheets lack a descriptor column, placing measure columns flush at Column B with no line item labels.
- **Single Source of Truth**: The fixtures in `tests/fixtures/` serve both in-memory evaluation tests and physical `.xlsx` workbook generation. Upgrading them ensures that all downstream consumers (unit tests, CI integration tests, manual visual audits in Excel) exercise realistic, institutional-grade models.
- **Demonstration of Full Finx Power**: Realistic financial models require currency formatting, percentage growth rates, multi-tier super headers, and collapsible section outlines to showcase the compiler's layout and styling capabilities.

### Scope

- **In-Scope**:
  - `tests/fixtures/mod.rs`: Extend builder helpers to support row descriptor labels, formatting presets, and semantic styles.
  - `tests/fixtures/three_statement.rs`: Upgrade Assumptions, Income Statement, Cash Flow, and Balance Sheet with descriptor columns, labels, semantic formatting, super-headers, and section headers.
  - `tests/fixtures/lbo.rs`: Upgrade Scenarios, Deal Assumptions, Returns Summary, Timeline, Operations, Debt Schedule, and Cash Flows with descriptor columns, labels, semantic formatting, super-headers, and section headers.
  - Verification across `tests/three_statement_model_test.rs`, `tests/lbo_model_test.rs`, `tests/export_integration_test.rs`, and `examples/export_models.rs`.
- **Out-of-Scope**:
  - Modifying compiler core pipelines (Pass 1 through Pass 6 are already feature-complete).

### What Changes

- **Modified files**:
  - `tests/fixtures/mod.rs`: Add builder helpers supporting labels and styling.
  - `tests/fixtures/three_statement.rs`: Full descriptor, format, and section upgrade.
  - `tests/fixtures/lbo.rs`: Full descriptor, format, and section upgrade.
  - `specs/history/2026-09-20-institutional-model-fixtures-upgrade.md`: Record completed architectural evolution.

### Capabilities

- **New capabilities**:
  - Exported workbooks display complete, professionally formatted line item labels in Column B, with timeline columns starting in Column C.
  - Numbers render with appropriate currency symbols, percentage signs, and decimal precisions rather than generic raw numbers.
  - Major financial statements feature collapsible row groups and super-header timeline banners.
- **Modified capabilities**: None (all existing calculation assertions remain identical).

---

## Design

### Goals

1. **Exact calculation preservation**: All evaluated values, formula dependencies, and balance checks must evaluate to the exact same numerical values as before.
2. **Deterministic spatial layout**: Column B becomes the descriptor column for line item labels, Column C onwards hosts measure timelines.
3. **No magic shims**: Fully adhere to the Finx 2D matrix model by explicitly declaring `ColumnSpec::descriptor` and populating string inputs in `row.cells`.

### Decisions

- **Decision: Standardize descriptor column ID as `"line_item"` (or `"parameter"` for assumptions)**:
  - *Rationale*: Consistency across financial statements. Vector formulas only reference measure columns (`val`, `y0..y5`, etc.), so existing formula ASTs require zero changes.
- **Decision: Keep all existing measure column IDs and row IDs unchanged**:
  - *Rationale*: Preserves backward compatibility with all existing test assertions.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Line Item Labels Render in Column B**
  - **Given** the upgraded Three-Statement and LBO fixtures
  - **When** spatial layout and workbook emission are executed
  - **Then** Column B displays descriptor labels, and timeline columns begin at Column C.

- [x] **Scenario 2: Mathematical Precision Preserved**
  - **Given** the upgraded Three-Statement and LBO fixtures
  - **When** running `cargo test --test three_statement_model_test` and `cargo test --test lbo_model_test`
  - **Then** all evaluation assertions (compounding revenue, net income, cash flow, debt sweep, balance check = 0) pass.

- [x] **Scenario 3: End-to-End Workbook Emission & Export**
  - **Given** the upgraded models
  - **When** executing `cargo test --test export_integration_test` and `cargo run --example export_models`
  - **Then** valid `.xlsx` workbooks are generated with live formulas, cached results, and descriptor labels.

## Tasks

1. [x] Update `tests/fixtures/mod.rs` with flexible row builder functions supporting line item labels, format presets, and semantic styles.
2. [x] Upgrade `tests/fixtures/three_statement.rs` with descriptor columns, line item labels, currency/percentage formats, and row/column groups.
3. [x] Upgrade `tests/fixtures/lbo.rs` with descriptor columns, line item labels, currency/percentage formats, and row/column groups.
4. [x] Run `cargo test` and verify that all in-memory and evaluation tests pass without error.
5. [x] Run `cargo run --example export_models` to re-generate `./output/*.xlsx` and inspect the resulting spreadsheet structure.
6. [x] Verify with `cargo clippy` and `cargo fmt`.
7. [x] Reconcile and archive draft spec to `specs/history/`.
