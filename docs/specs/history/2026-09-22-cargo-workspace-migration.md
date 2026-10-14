---
title: Multi-Crate Cargo Workspace Migration
date: 2026-09-22
status: complete
target-living-spec: none
---

## Proposal

### What

Migrate the f8 codebase from a single-package Cargo structure into a modular Cargo workspace (monorepo) consisting of:
- `crates/f8-core`: The pure compiler engine, intermediate representations, calculation evaluator, spatial layout planner, and OpenXML emitter.
- `crates/f8-cli`: The command-line interface binary (`f8`) powered by `clap`.
- `crates/f8-mcp`: The Model Context Protocol server binary (`f8-mcp`) for AI agent tool integration.

This migration is purely structural: it moves existing modules and tests into `crates/f8-core` without altering compiler logic, I/O handling, or domain behavior.

### Why

As specified in `docs/codebase-structure.md` and `docs/architecture.md`, f8 enforces a strict boundary between its mathematical calculation engine and outer interface adapters. 

Restructuring into a Cargo workspace now:
1. Enforces acyclic crate boundaries at compile time (`f8-core` cannot depend on outer interfaces).
2. Verifies that all 200+ unit and integration tests continue to pass in an isolated library crate before any CLI or MCP code is introduced.
3. Leaves file I/O and `std::fs` refactoring for a subsequent dedicated step, eliminating mixed-concern regressions.
4. Reserves the `f8` executable name for `f8-cli`, matching analyst CLI workflows.

### Scope

- **In-scope**:
  - Reconfigure root `Cargo.toml` as a Cargo workspace with shared `[workspace.package]`, `[workspace.dependencies]`, and `[workspace.lints]`.
  - Move current `src/` (excluding root `main.rs`) and `tests/` into `crates/f8-core/`.
  - Create `crates/f8-core/Cargo.toml` with crate name `f8-core` (`f8_core`).
  - Update `use finx::` import paths across all 19 integration test files to `use f8_core::`.
  - Lock in canonical root manifest convention as `f8.json` (replacing legacy `finx.json`).
  - Align internal codebase references (`f8 Compiler` author string, test fixture labels, manifest doc comments, and example runners).
  - Update documentation suite (`docs/interfaces/cli.md`, `docs/interfaces/mcp.md`, `docs/dsl/`, `docs/architecture.md`, `docs/codebase-structure.md`) and living specs (`specs/living-specs/`) from `finx` / `finx.json` to `f8` / `f8.json`.
  - Create `crates/f8-cli/` stub binary crate configured to produce the `f8` binary and depending on `f8-core`.
  - Create `crates/f8-mcp/` stub binary crate configured to produce the `f8-mcp` binary and depending on `f8-core`.
  - Verify complete workspace with `cargo check`, `cargo test`, `cargo clippy`, and `cargo fmt`.

- **Out-of-scope**:
  - No refactoring of compiler passes, ASTs, or evaluation algorithms.
  - No removal or refactoring of filesystem I/O (`std::fs`) in `f8-core` (scheduled for a subsequent phase).
  - No loose `templates/` directory at repository root (factory templates will be embedded in binary assets).
  - No implementation of CLI subcommands or MCP protocol handlers (stub entrypoints only).

### What Changes

- `Cargo.toml`: Converted from package manifest to workspace root manifest.
- `crates/f8-core/Cargo.toml`: New library crate manifest inheriting workspace dependencies.
- `crates/f8-core/src/`: Moved from repository root `src/` (minus root `main.rs`).
- `crates/f8-core/tests/`: Moved from repository root `tests/`.
- `crates/f8-cli/Cargo.toml`: New CLI binary crate manifest (`bin.name = "f8"`).
- `crates/f8-cli/src/main.rs`: Minimal CLI entrypoint.
- `crates/f8-mcp/Cargo.toml`: New MCP binary crate manifest (`bin.name = "f8-mcp"`).
- `crates/f8-mcp/src/main.rs`: Minimal MCP entrypoint.
- `docs/` & `specs/living-specs/`: Branding and manifest references aligned to `f8` / `f8.json`.

### Capabilities

- **New capabilities**:
  - Workspace-level orchestration (`cargo test --workspace`, `cargo clippy --workspace`).
  - Independent compilation and packaging of `f8-core`, `f8-cli`, and `f8-mcp`.
- **Modified capabilities**:
  - Integration tests import `f8_core` instead of `finx`.
  - Canonical manifest recognized as `f8.json`.

---

## Design

### Goals

1. **Zero behavioral regression**: 100% of existing tests pass with identical outputs.
2. **Preserve git history**: Use `git mv` for file relocation where possible.
3. **Clean workspace lints**: Deny production panics (`unwrap_used`, `expect_used`) across all workspace crates.
4. **Pure layout migration**: Defer all functional and I/O refactoring to subsequent increments.
5. **Unified brand consistency**: Ensure all code, doc comments, test fixtures, documentation, and specs refer strictly to `f8` and `f8.json`.

### Invariants

1. **Acyclic hierarchy**: `f8-core` must never depend on `f8-cli` or `f8-mcp`.
2. **Rust 2024 edition**: All workspace crates must use the 2024 edition.
3. **Deterministic testing**: Test execution order and output parity remain completely unchanged.
4. **Single manifest convention**: All references standardize on `f8.json` across documentation, specs, and code.

### Decisions

- **Choice 1: Pure relocation without code refactoring**:
  - *Rationale*: Isolating structural workspace changes from behavioral refactoring prevents conflating packaging issues with compiler regressions.
  - *Alternatives considered*: Refactoring `Theme::resolve` and `Compiler::save_workbook` away from `std::fs` concurrently. Rejected to keep the migration diff small, verifiable, and safe.
- **Choice 2: Name core library crate `f8-core` (`f8_core`)**:
  - *Rationale*: Aligns with documented architecture and reserves the `f8` binary target name for `f8-cli`. Starts with an ASCII letter, making `f8_core` a valid Rust identifier.
  - *Alternatives considered*: Keeping the library as `f8`. Rejected because Cargo binary and library collisions in multi-crate workspaces create naming ambiguity.
- **Choice 3: Embed factory templates rather than maintaining external directory**:
  - *Rationale*: Eliminates path-discovery failures when `f8-cli` is installed as a standalone binary via `cargo install`.
- **Choice 4: Lock canonical manifest filename to `f8.json`**:
  - *Rationale*: Avoids namespace collisions with web PWA/extension `manifest.json`, allows deterministic CLI parent-directory traversal to discover the project root, and enables dedicated IDE file icon and JSON schema bindings.
  - *Alternatives considered*: `manifest.json`. Rejected due to ambiguity in multi-project repositories and collisions with web platform manifests.

### Risks & Trade-offs

- **Risk: Disrupted git blame / history**:
  - *Mitigation*: Relocate files using `git mv` rather than deleting and recreating.
- **Risk: Missed test import updates**:
  - *Mitigation*: Run `cargo test --workspace` and verify all 19 test integration files succeed.
- **Risk: Inconsistent documentation references**:
  - *Mitigation*: Systematically update `docs/` and `specs/living-specs/` during Phase 3 verification.

---

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Full workspace compilation and test suite execution**
  - **Given** the workspace root and member crates `f8-core`, `f8-cli`, and `f8-mcp`
  - **When** `cargo test --workspace` is executed
  - **Then** all 200+ unit and integration tests in `f8-core` compile and pass cleanly with zero failures.

- [x] **Scenario 2: Strict clippy and panic check adherence**
  - **Given** workspace-level clippy lints (`unwrap_used = "deny"`, `expect_used = "deny"`)
  - **When** `cargo clippy --workspace --all-targets -- -D warnings` is executed
  - **Then** the check passes with zero warnings across all crates.

- [x] **Scenario 3: Binary entrypoint builds**
  - **Given** `f8-cli` and `f8-mcp` crates
  - **When** `cargo build --workspace --bins` is executed
  - **Then** binary artifacts `target/debug/f8` and `target/debug/f8-mcp` are generated successfully.

- [x] **Scenario 4: Dependency acyclicity**
  - **Given** `crates/f8-core/Cargo.toml`
  - **When** inspecting its dependency list
  - **Then** it contains zero references to `f8-cli` or `f8-mcp`.

- [x] **Scenario 5: Branding and manifest consistency**
  - **Given** source files, tests, documentation, and living specs
  - **When** searching for `finx` (excluding historical archived specs in `specs/history/`)
  - **Then** zero active occurrences of `finx` or `finx.json` remain.

---

## Tasks

1. **Phase 1: Workspace root configuration & crate layout**:
   - [x] Relocate `src/` (excluding `main.rs`) to `crates/f8-core/src/` via `git mv`.
   - [x] Relocate `tests/` to `crates/f8-core/tests/` via `git mv`.
   - [x] Create root `Cargo.toml` with `[workspace]`, `[workspace.dependencies]`, and `[workspace.lints]`.
   - [x] Create `crates/f8-core/Cargo.toml` inheriting workspace dependencies.
2. **Phase 2: Core test import alignment & codebase branding**:
   - [x] Update `use finx::` statements to `use f8_core::` across all 19 test files in `crates/f8-core/tests/`.
   - [x] Update default author in `crates/f8-core/src/compiler/emission/workbook.rs` to `"f8 Compiler"`.
   - [x] Update test fixture references in `crates/f8-core/tests/fixtures/parity_suite.rs` (`finx_val` -> `f8_val`, `Finx Evaluated` -> `f8 Evaluated`).
   - [x] Update model name in `crates/f8-core/tests/fixtures/styling_showcase.rs` (`Finx_Styling_Showcase` -> `f8_Styling_Showcase`).
   - [x] Update author assertions in `crates/f8-core/tests/export_integration_test.rs` (`"Finx Institutional Engine"` etc. -> `"f8 ..."`).
   - [x] Update dummy test input in `crates/f8-core/tests/evaluator_test.rs` (`"FinX"` -> `"f8"`).
   - [x] Update examples runner in `examples/export_models.rs` (imports to `f8_core`, banner text, and author metadata).
   - [x] Update doc comments in `crates/f8-core/src/` (`model/manifest.rs`, `model/format.rs`, `model/error.rs`, `expr/ast.rs`, `compiler/emission/format.rs`) referencing `finx` / `finx.json` to `f8` / `f8.json`.
   - [x] Remove legacy root `src/main.rs`.
3. **Phase 3: Outer interface stubs**:
   - [x] Create `crates/f8-cli/Cargo.toml` (`bin.name = "f8"`) and `crates/f8-cli/src/main.rs`.
   - [x] Create `crates/f8-mcp/Cargo.toml` (`bin.name = "f8-mcp"`) and `crates/f8-mcp/src/main.rs`.
4. **Phase 4: Documentation & living specs alignment**:
   - [x] Update CLI interface docs in `docs/interfaces/cli.md` (`finx` CLI commands -> `f8`).
   - [x] Update MCP interface docs in `docs/interfaces/mcp.md` (`finx-mcp` binary and `finx_*` tools -> `f8-mcp` / `f8_*`).
   - [x] Update DSL specifications (`docs/dsl/spec-format.md`, `docs/dsl/authoring-guide.md`, `docs/dsl/project-example.md`, `docs/dsl/formatting.md`) replacing `finx.json` with `f8.json`.
   - [x] Update architecture and structure docs (`docs/architecture.md`, `docs/codebase-structure.md`, `docs/compiler/parity.md`).
   - [x] Update living specs in `specs/living-specs/` (`model/spec-ast.md`, `model/theme.md`, `compiler/workbook-emission.md`, etc.).
5. **Phase 5: Verification**:
   - [x] Run `cargo check --workspace --all-targets`.
   - [x] Run `cargo test --workspace`.
   - [x] Run `cargo clippy --workspace --all-targets -- -D warnings`.
   - [x] Run `cargo fmt --check`.
