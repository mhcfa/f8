---
title: CLI Core, Project Loader, and Verification (f8 check)
date: 2026-09-23
status: complete
target-living-spec: specs/living-specs/interfaces/cli.md
---

## Proposal

### What

Implement the foundational `f8` command-line executable in `crates/f8-cli`, focusing on project discovery, multi-file project loading, standardized exit codes, and the fast headless verification command (`f8 check`).

### Why

Financial analysts and autonomous AI coding agents need a lightweight, headless verification tool to validate financial models directly in their local workflow and CI pipelines. Building this core foundation first establishes the binary infrastructure, argument parsing, directory discovery, and error classification before layering on compilation, scaffolding, and terminal rendering.

### Scope

- **In-scope**:
  - Workspace configuration and dependencies for `crates/f8-cli` (`clap`, `serde`, `serde_json`, `thiserror`).
  - Project manifest discovery via upward directory traversal starting from the current working directory to locate `f8.json`.
  - Explicit project targeting via global `-p, --project <path>` supported before or after subcommands.
  - Multi-file project loading: reading `f8.json` and all declared `tables/*.json` relative to the project directory, as well as resolving custom themes.
  - Fail-fast error hierarchy and deterministic exit code mapping:
    - `0`: Success (verification passed).
    - `1`: Operational, I/O, or CLI syntax error (e.g., `f8.json` not found, file unreadable, invalid CLI argument).
    - `2`: Compiler diagnostic (Passes 1–6a error: syntax, cyclic dependency, unresolved reference, layout overflow, lowering failure).
  - Headless verification subcommand `f8 check`: Executes Passes 1–6a via `f8_core::Compiler::compile`.
  - Dual human-readable terminal output and structured machine-readable `--json` output.
  - Standard stream separation: results to `stdout`, errors and diagnostics to `stderr`.
  - Comprehensive CLI integration tests covering upward traversal, manifest errors, compiler diagnostic exit codes, and successful verification.

- **Out-of-scope**:
  - OpenXML binary workbook compilation (`f8 compile` - Milestone 2).
  - Project scaffolding and archetype templates (`f8 init` - Milestone 2).
  - Auditing and terminal inspection subcommands (`f8 inspect` - Milestone 3).
  - Imperative model mutation via CLI flags.

### What Changes

- `crates/f8-cli/Cargo.toml`:
  - Add workspace dependencies (`clap` with derive, `serde`, `serde_json`, `thiserror`).
  - Add dev-dependencies for CLI integration tests (`assert_cmd`, `predicates`, `tempfile`).
- `crates/f8-cli/src/`:
  - `main.rs`: Entrypoint, clap dispatch, top-level error mapping to exit codes 0, 1, 2.
  - `cli.rs`: Clap command definitions, argument structs, global flags (`-p`, `--json`).
  - `error.rs`: `CliError` enum with exit code resolution.
  - `project/mod.rs` & `project/loader.rs`: Upward directory traversal, project manifest loader, multi-table reader.
  - `commands/mod.rs` & `commands/check.rs`: `f8 check` execution logic and human/JSON formatting.
- `crates/f8-cli/tests/`:
  - Integration tests verifying CLI binary execution, upward discovery, exit codes, and output formats.

### Capabilities

- **New capabilities**:
  - Standalone `f8` executable available in Cargo workspace.
  - Upward discovery of `f8.json` allowing commands to run from nested subdirectories.
  - Sub-millisecond headless model verification via `f8 check`.
  - Deterministic exit codes (0, 1, 2) enabling reliable scripting in CI and agent workflows.
  - `--json` machine-readable output for programmatic agent consumption.

- **Modified capabilities**: None (greenfield CLI binary).

---

## Design

### Goals

1. **Pure compiler delegation**: Zero business logic, layout calculations, or AST validation inside the CLI; all validation delegates directly to `f8_core::Compiler::compile`.
2. **Deterministic exit codes**: Strict classification of operational errors (exit code 1) vs compiler diagnostics (exit code 2).
3. **Ergonomic discovery**: Run `f8 check` from any subdirectory without requiring `-p` when working within an f8 project tree.
4. **Zero production panics**: Enforce `deny(clippy::unwrap_used, clippy::expect_used)` throughout `crates/f8-cli`.
5. **Stream discipline**: Human summary or JSON output on `stdout`; errors and compiler diagnostics on `stderr`.

### Invariants

1. **Declarative primacy**: The CLI only reads project files; it never mutates `f8.json` or `tables/*.json`.
2. **Linear pass execution**: `f8 check` executes Passes 1–6a once via `Compiler::compile`. If verification succeeds, workbook emission is guaranteed to succeed.
3. **Exit code stability**: Scripts and agents can strictly rely on exit codes `0`, `1`, and `2`.

### Decisions

- **Decision 1: Global `-p, --project` flag supported in Clap root**
  - *Rationale*: Users and AI agents often position arguments flexibly (e.g. `f8 -p ./models check` vs `f8 check -p ./models`). Marking `-p` as a global flag on the root command ensures both forms parse without syntax errors.
  - *Alternatives considered*: Subcommand-only flag (rejected: causes frequent user syntax friction).

- **Decision 2: Strict 3-state exit code contract**
  - *Rationale*: Differentiating between operational failures (exit code 1: missing file, invalid flag) and domain compiler diagnostics (exit code 2: circular dependency, unassigned driver) allows CI tools and agents to take distinct recovery actions.
  - *Alternatives considered*: Single non-zero exit code (rejected: obscures whether the failure was an environmental error or a model calculation defect).

- **Decision 3: Multi-file project assembly in loader**
  - *Rationale*: `f8-core` takes `ModelSpec` and custom `Theme` in memory. The CLI's `ProjectLoader` handles disk discovery, reads `f8.json`, resolves each relative path in `SheetSpec.tables`, deserializes them into `TableSpec`, and constructs the complete in-memory `ModelSpec`.
  - *Alternatives considered*: Having `f8-core` read from disk (rejected: `f8-core` is strictly decoupled from filesystem I/O for WASM compatibility).

### Data Contracts

#### CLI Structure

```text
f8 [-p, --project <path>] [--json] <subcommand>

Subcommands:
  check     Run headless compiler verification across Passes 1–6a
```

#### Exit Codes

| Exit Code | Classification | Cause |
| :--- | :--- | :--- |
| `0` | Success | Passes 1–6a verified cleanly. |
| `1` | Operational / CLI / I/O Error | CLI syntax error, `f8.json` not found, file unreadable, invalid JSON syntax. |
| `2` | Compiler Diagnostic | Schema validation failure, duplicate ID, unassigned driver, circular reference, layout overflow, lowering failure. |

#### `f8 check` Output Formats

**Human Output (`stdout`)**:
```text
✓ Validated 4 sheets, 6 tables, 142 cells
✓ DAG acyclicity verified (0 cycles)
✓ Evaluated 142 cells in 1.4ms
✓ Planned spatial layouts and verified formula lowering across 4 sheets
Status: OK
```

**JSON Output (`stdout` with `--json`)**:
```json
{
  "status": "ok",
  "summary": {
    "sheets": 4,
    "tables": 6,
    "cells": 142,
    "duration_ms": 1.4
  }
}
```

**Error Output (`stderr` with `--json`)**:
```json
{
  "status": "error",
  "exit_code": 2,
  "error": "Circular dependency detected: IncomeStatement[\"ebit\", \"2024E\"] -> IncomeStatement[\"tax\", \"2024E\"] -> IncomeStatement[\"ebit\", \"2024E\"]"
}
```

### Risks & Trade-offs

- **Deep directory trees**: Upward traversal could theoretically traverse up to root `/` if no `f8.json` exists.
  - *Mitigation*: Stop immediately at filesystem boundary / root directory and return a descriptive `CliError::ProjectNotFound`.

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Upward project discovery**
  - **Given** an `f8.json` in a root directory and a deeply nested current working directory
  - **When** running `f8 check` without `-p`
  - **Then** the CLI traverses parent directories, locates `f8.json`, loads all relative table files, and exits with code `0`.

- [x] **Scenario 2: Explicit project flag**
  - **Given** an f8 project at `./models/lbo`
  - **When** running `f8 -p ./models/lbo check` or `f8 check -p ./models/lbo`
  - **Then** the CLI loads the specified project and exits with code `0`.

- [x] **Scenario 3: Missing manifest operational error**
  - **Given** a directory tree containing no `f8.json`
  - **When** running `f8 check`
  - **Then** the CLI exits with code `1` and emits an error message to `stderr`.

- [x] **Scenario 4: Invalid JSON file error**
  - **Given** a project where a table JSON file contains a syntax error
  - **When** running `f8 check`
  - **Then** the CLI exits with code `1` and identifies the unreadable file on `stderr`.

- [x] **Scenario 5: Fail-fast compiler diagnostic**
  - **Given** an f8 project containing a circular reference or unresolved coordinate
  - **When** running `f8 check`
  - **Then** compilation halts, exits with code `2`, and outputs the exact compiler error to `stderr`.

- [x] **Scenario 6: Machine-readable JSON output on success**
  - **Given** a valid f8 project
  - **When** running `f8 check --json`
  - **Then** valid JSON with `"status": "ok"` and summary metrics is emitted to `stdout` with exit code `0`.

- [x] **Scenario 7: Machine-readable JSON output on compiler failure**
  - **Given** an f8 project with a circular reference
  - **When** running `f8 check --json`
  - **Then** valid JSON with `"status": "error"` and `"exit_code": 2` is emitted to `stderr` with exit code `2`.

- [x] **Scenario 8: Zero production panics**
  - **Given** `crates/f8-cli`
  - **When** running `cargo clippy -p f8-cli --all-targets -- -D warnings`
  - **Then** zero warnings and zero unhandled panics (`unwrap`/`expect`) are found.

---

## Tasks

### Phase 1: Dependencies & Error Hierarchy
- [x] Add `clap` (derive), `serde`, `serde_json`, and `thiserror` to `crates/f8-cli/Cargo.toml`.
- [x] Add `assert_cmd`, `predicates`, and `tempfile` to `crates/f8-cli/Cargo.toml` under `[dev-dependencies]`.
- [x] Implement `CliError` in `crates/f8-cli/src/error.rs` with variants for `ProjectNotFound`, `Io`, `Json`, and `Compiler(CompilerError)`.
- [x] Implement `exit_code()` on `CliError` mapping I/O/operational errors to `1` and compiler errors to `2`.

### Phase 2: Upward Traversal & Multi-File Project Loader
- [x] Implement upward directory search for `f8.json` in `crates/f8-cli/src/project/loader.rs`.
- [x] Implement multi-file reader resolving relative table paths declared in `SheetSpec.tables`.
- [x] Support custom theme loading if a custom path is declared in `f8.json`.
- [x] Add unit tests for project loader verifying upward traversal and multi-file assembly.

### Phase 3: Clap CLI Structure & Dispatcher
- [x] Define Clap command hierarchy in `crates/f8-cli/src/cli.rs` with global `-p, --project` and `--json` flags.
- [x] Define `CheckArgs` for the `check` subcommand.
- [x] Implement `main.rs` dispatching to command handlers, catching errors, formatting to `stderr`, and returning the appropriate process exit code.

### Phase 4: `f8 check` Runner & Output Formatting
- [x] Implement `commands/check.rs` executing Passes 1–6a via `Compiler::compile`.
- [x] Implement human-readable check summary output.
- [x] Implement JSON check summary output for `--json`.
- [x] Implement JSON error output on `stderr` when `--json` is enabled.

### Phase 5: Verification Suite
- [x] Add integration test suite in `crates/f8-cli/tests/check_test.rs` covering all acceptance criteria.
- [x] Verify `cargo check`.
- [x] Verify `cargo test --workspace`.
- [x] Verify `cargo clippy --workspace --all-targets -- -D warnings`.
- [x] Verify `cargo fmt --check`.
