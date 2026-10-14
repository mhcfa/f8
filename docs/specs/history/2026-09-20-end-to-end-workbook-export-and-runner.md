---
title: End-to-End Workbook Export Integration Test & Dedicated Runner
date: 2026-09-20
status: completed
target-living-spec: specs/living-specs/compiler/workbook-emission.md
---

## Proposal

### What

Introduce a dedicated end-to-end integration test suite and a standalone executable runner that compile realistic, institutional financial models through all six passes of the Finx compiler and export physical, fully compliant `.xlsx` workbooks:

1. **Shared Model Fixtures (`tests/fixtures/`)**: Centralize the **Three-Statement Financial Model** and **Institutional LBO Model** definitions into `tests/fixtures/` (`mod.rs`, `three_statement.rs`, and `lbo.rs`), serving as a single source of truth across all integration tests and runners.
2. **Refactor Existing Benchmarks**: Update `tests/three_statement_model_test.rs` and `tests/lbo_model_test.rs` to import from `tests/fixtures/`, eliminating code duplication.
3. **Dedicated Integration Test (`tests/export_integration_test.rs`)**:
   - `test_models_emit_in_memory`: Always runs during standard `cargo test`. Compiles both the Three-Statement and LBO models in RAM via `Compiler::emit_workbook`, verifying OpenXML ZIP package structure, sheet ordering, lowered A1 formula syntax (`<f>`), and pre-populated evaluation result caches (`<v>`) without writing any files to disk.
   - `test_export_xlsx_to_disk`: Annotated with `#[ignore = "Generates physical .xlsx files on disk. Run with: cargo test --test export_integration_test -- --ignored"]`. Uses `Compiler::save_workbook` to write actual `.xlsx` files to a target directory and verifies physical file size and re-readability.
4. **Dedicated Runner (`examples/export_models.rs`)**:
   - Standalone Cargo example executable (`cargo run --example export_models [output_dir]`).
   - Compiles both models with real themes (`corporate_navy` and `classic_green`), exports them to the destination directory (defaulting to `./output/`), and prints formatted paths for instant manual inspection in Excel or Apple Numbers.

### Why

- **End-to-End Verification**: While Pass 6b emission unit tests (`tests/workbook_emission_test.rs`) verify isolated XML serialization mechanics, we need end-to-end proof that complete, interconnected multi-sheet financial models compile cleanly through spatial layout, formula lowering, and OpenXML emission.
- **Zero Pollution on Standard Test Runs**: Running file I/O tests on every `cargo test` execution litters the repository with binary artifacts and slows down the inner development loop. Using `#[ignore]` and a separate Cargo example ensures default test runs remain blazing fast, clean, and in-memory.
- **Convenient Visual Audit**: Developers and financial analysts need a trivial, one-command mechanism to produce real `.xlsx` files they can double-click and inspect in Microsoft Excel or Apple Numbers.
- **Single Source of Truth**: Multiple test files must not independently construct copy-pasted versions of the Three-Statement or LBO models.

### Scope

- **In-scope**:
  - `tests/fixtures/mod.rs`: Shared test fixture declarations and helper constructors.
  - `tests/fixtures/three_statement.rs`: Consolidated Three-Statement model generator.
  - `tests/fixtures/lbo.rs`: Consolidated Institutional LBO model generator.
  - Refactoring `tests/three_statement_model_test.rs` and `tests/lbo_model_test.rs` to consume shared fixtures.
  - `tests/export_integration_test.rs`: Dedicated integration test (in-memory + `#[ignore]` disk export).
  - `examples/export_models.rs`: Dedicated CLI example runner.
  - Updating living spec `specs/living-specs/compiler/workbook-emission.md`.
- **Out-of-scope**:
  - Production CLI binary (`src/main.rs` / `finx compile`), which belongs to the CLI interface milestone.
  - Modifying compiler core production logic (compiler passes 1–6 are already functional).

### What Changes

- **New files**:
  - `tests/fixtures/mod.rs`
  - `tests/fixtures/three_statement.rs`
  - `tests/fixtures/lbo.rs`
  - `tests/export_integration_test.rs`
  - `examples/export_models.rs`
- **Modified files**:
  - `tests/three_statement_model_test.rs`: Refactored to import from `fixtures`.
  - `tests/lbo_model_test.rs`: Refactored to import from `fixtures`.

### Capabilities

- **New capabilities**:
  - Developers can export complete, inspectable Excel workbooks with `cargo run --example export_models`.
  - Developers can explicitly run full disk export integration tests with `cargo test --test export_integration_test -- --ignored`.
  - Continuous integration automatically tests end-to-end workbook emission in RAM on every push without filesystem side effects.
- **Modified capabilities**: None (all existing test assertions remain intact).

---

## Design

### Goals

1. **Deterministic, zero-side-effect test suite**: `cargo test` runs 100% in memory and writes zero files to disk.
2. **Single source of truth for benchmark models**: Model definitions are authored once in `tests/fixtures/` and reused across evaluation tests, emission tests, and export runners.
3. **Comprehensive OpenXML validation**: Tests inspect the emitted ZIP archive to confirm sheet definitions, `<f>` formula tags, `<v>` cached values, and format styles.
4. **Convenient developer workflow**: One command to generate inspectable files.

### Invariants

- Model specs and generated OpenXML workbooks must strictly adhere to Rust 2024 idioms.
- No production unwrap or expect calls.
- Shared fixtures must remain private to test targets (`tests/` and `examples/`).

### Decisions

- **Decision: Use Cargo `examples/` for file-writing runner**:
  - *Rationale*: Examples are never executed by `cargo test`, avoiding filesystem pollution while ensuring code is continuously compiled and type-checked.
  - *Alternatives considered*: Writing a bash script (less portable, not typed) or adding a flag to `src/main.rs` (preempts the upcoming CLI specification).
- **Decision: Use `#[ignore]` for disk export integration test**:
  - *Rationale*: Idiomatic Rust/Cargo convention for tests that touch disk or external resources.
  - *Alternatives considered*: Checking environment variables only (less transparent in `cargo test` summaries).

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: In-Memory End-to-End Workbook Emission**
  - **Given** the Three-Statement Model and Institutional LBO Model from `tests/fixtures/`
  - **When** calling `Compiler::emit_workbook` with `corporate_navy` and `classic_green` themes
  - **Then** emission produces valid OpenXML byte buffers containing `[Content_Types].xml`, `xl/workbook.xml`, worksheets with lowered formulas (`<f>`), and cached values (`<v>`).

- [x] **Scenario 2: Default `cargo test` Excludes Disk Exports**
  - **Given** the test suite is executed via `cargo test`
  - **When** the test runner reaches `tests/export_integration_test.rs`
  - **Then** `test_export_xlsx_to_disk` is reported as `ignored` and zero files are written to the working directory.

- [x] **Scenario 3: Explicit Ignored Test Executes Disk Export**
  - **Given** the command `cargo test --test export_integration_test -- --ignored`
  - **When** executed
  - **Then** `.xlsx` files are written to `target/test-output/`, verified to be valid ZIP packages, and cleaned up.

- [x] **Scenario 4: Standalone Example Runner Exports Inspectable Workbooks**
  - **Given** the command `cargo run --example export_models`
  - **When** executed
  - **Then** `three_statement_model.xlsx` and `institutional_lbo_model.xlsx` are saved to the output directory and clickable terminal links are displayed.

- [x] **Scenario 5: Existing Benchmark Test Parity**
  - **Given** `tests/three_statement_model_test.rs` and `tests/lbo_model_test.rs` refactored to use `tests/fixtures/`
  - **When** running `cargo test --test three_statement_model_test --test lbo_model_test`
  - **Then** all existing mathematical and sensitivity assertions continue to pass without regression.

---

## Tasks

### Phase 1: Shared Test Fixtures
- [x] Create `tests/fixtures/mod.rs` declaring helper constructors (`build_measure_col`, `build_point_row`, `build_vector_row`).
- [x] Create `tests/fixtures/three_statement.rs` implementing `build_three_statement_model()`.
- [x] Create `tests/fixtures/lbo.rs` implementing `build_lbo_model(active_case)`.
- [x] Refactor `tests/three_statement_model_test.rs` to import from `fixtures`.
- [x] Refactor `tests/lbo_model_test.rs` to import from `fixtures`.
- [x] Verify with `cargo test --test three_statement_model_test --test lbo_model_test`.

### Phase 2: Dedicated Integration Test
- [x] Author `tests/export_integration_test.rs`.
- [x] Implement `test_models_emit_in_memory` validating OpenXML ZIP entries, formula tags, and cached values.
- [x] Implement `test_export_xlsx_to_disk` with `#[ignore]` attribute.
- [x] Verify standard `cargo test` reports it as ignored.
- [x] Verify `cargo test --test export_integration_test -- --ignored` executes cleanly.

### Phase 3: Dedicated Example Runner
- [x] Author `examples/export_models.rs`.
- [x] Implement output directory parsing (defaulting to `./output`).
- [x] Emit both models with themes and formatted terminal feedback.
- [x] Run `cargo run --example export_models` and verify files are generated.

### Phase 4: Verification & Reconciliation
- [x] Run full test suite: `cargo test`.
- [x] Run linter: `cargo clippy --all-targets -- -D warnings`.
- [x] Run formatter: `cargo fmt --check`.
- [x] Update living spec `specs/living-specs/compiler/workbook-emission.md`.
- [x] Archive draft spec to `specs/history/`.
