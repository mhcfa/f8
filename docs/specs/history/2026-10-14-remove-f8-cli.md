---
title: Remove Redundant Rust CLI Crate and JSON Authoring Guide
date: 2026-10-14
status: completed
---

## Proposal

### Problem
Model authoring, compilation, and auditing are now performed through the Python SDK (`python/f8/` and `crates/f8-python/`), making the standalone Rust CLI crate (`crates/f8-cli/`) and manual JSON authoring workflow (`docs/cli-authoring-guide.md`) redundant. Maintaining `crates/f8-cli` introduces cross-language logic duplication (on-disk `f8.json` project loading duplicated in `python/f8/loader.py`; 2D Markdown table rendering duplicated in `python/f8/audit.py`), ongoing maintenance overhead whenever `f8-core` evolves, unnecessary workspace dependencies (`clap`, `assert_cmd`, `predicates`, `tempfile`, `libc`), and split documentation between JSON+CLI and Python DSL workflows. Furthermore, two Python tests (`python/tests/test_project_io.py` and `python/tests/test_audit.py`) still reference `crates/f8-cli/templates` with a `pytest.skip` fallback.

### Solution
1. Delete `crates/f8-cli/` entirely (source, integration tests, and embedded JSON archetype templates).
2. Delete `docs/cli-authoring-guide.md`.
3. Remove `"crates/f8-cli"` and CLI-only dependencies (`clap`, `assert_cmd`, `predicates`, `tempfile`, `libc`) from the workspace root `Cargo.toml` and `Cargo.lock`.
4. Make the Python test suite 100% self-contained without `crates/f8-cli/templates` or `pytest.skip` fallbacks:
   - Remove `test_compile_fixture_templates` from `python/tests/test_project_io.py`.
   - Refactor `test_scenario_9_fixture_template_auditing` in `python/tests/test_audit.py` into an in-memory multi-tier header and section-header auditing test (`test_scenario_9_multi_tier_header_and_section_auditing`) built via the Python `Model` DSL.
5. Synchronize `AGENTS.md`, `README.md`, `docs/architecture.md`, and `docs/dsl/spec-format.md` to reflect the 2-crate Rust workspace (`f8-core` and `f8-python`) and Python SDK authoring workflow.

### Scope
- **In-scope**:
  - Deleting `crates/f8-cli/` and `docs/cli-authoring-guide.md`.
  - Pruning `Cargo.toml` workspace members and unused `[workspace.dependencies]`.
  - Removing `crates/f8-cli/templates` references and `pytest.skip` guards from `python/tests/test_project_io.py` and `python/tests/test_audit.py`.
  - Updating `AGENTS.md`, `README.md`, `docs/architecture.md`, and `docs/dsl/spec-format.md`.
- **Out-of-scope**:
  - Modifying `crates/f8-core/` or `crates/f8-python/`.
  - Modifying the Python SDK module runner (`python/f8/cli.py` and `python/f8/__main__.py` for `python -m f8 guide` / `python -m f8 agent init`).
  - Modifying historical spec archives under `docs/specs/history/`.

### Impacted components
- `crates/f8-cli/`: Deleted crate directory.
- `docs/cli-authoring-guide.md`: Deleted documentation guide.
- `Cargo.toml` & `Cargo.lock`: Workspace member list and shared dependencies.
- `python/tests/test_project_io.py` & `python/tests/test_audit.py`: Removal of `crates/f8-cli/templates` coupling.
- `AGENTS.md`, `README.md`, `docs/architecture.md`, `docs/dsl/spec-format.md`: Workspace topology and documentation links.

### Capabilities
- **New capabilities**: None (simplification and consolidation).
- **Modified capabilities**: Retires the standalone `f8` Rust CLI binary (`init`, `check`, `compile`, `inspect`). On-disk JSON project compilation and inspection remain supported via `f8.compile_project()` and `CompiledModel` / `TableAuditor` in Python.

---

## Design

### Goals & non-goals
- **Goals**:
  - Eliminate cross-language duplication of project loading and Markdown table rendering between Rust and Python.
  - Reduce Cargo workspace surface to `crates/f8-core` and `crates/f8-python`.
  - Ensure zero skipped tests (`0 skipped`) in `uv run pytest` after removing `crates/f8-cli/templates`.
  - Ensure zero dangling links or stale `f8-cli` references in active documentation (`AGENTS.md`, `README.md`, `docs/`).
- **Non-goals**:
  - Adding new CLI subcommands to `python/f8/cli.py`.
  - Altering `f8-core` JSON deserialization schemas or `python/f8/loader.py`.

### Invariants
- **Pure calculation core**: `crates/f8-core` remains 100% in-memory and unchanged.
- **Single source of truth for outer I/O and auditing**: `python/f8/loader.py` and `python/f8/audit.py` serve as the canonical implementation for disk project loading and tabular Markdown rendering.
- **Zero silent test skips**: Python tests must execute unconditionally without `pytest.skip` directory guards.

### Technical decisions
- **Decision**: Delete `crates/f8-cli/templates` rather than relocating static JSON templates into `python/tests/fixtures/`.
  - **Rationale**: The Python test suite (`test_e2e.py`, `test_project_io.py`, `test_audit.py`) already constructs institutional models directly via `f8.Model` and verifies on-disk JSON round-tripping via `export_project()` and `compile_project()`. Replacing the static template read in `test_audit.py` with an explicit `Model` builder preserves 100% coverage of multi-tier column headers, section headers, and indented row labels without maintaining static JSON fixture blobs.
- **Decision**: Prune `clap`, `assert_cmd`, `predicates`, `tempfile`, and `libc` from root `Cargo.toml`.
  - **Rationale**: None of these crates are referenced by `crates/f8-core` or `crates/f8-python`. Removing them trims the dependency tree and `Cargo.lock`.

### Risks & mitigations
- **Risk**: Dangling markdown links to `docs/cli-authoring-guide.md` in reference docs.
- **Mitigation**: Explicitly update all references in `README.md`, `docs/architecture.md`, and `docs/dsl/spec-format.md`, and verify via ripgrep across active docs.

---

### Acceptance criteria (Given / When / Then)

- [x] **Scenario 1: Clean 2-crate Rust workspace build and test**
  - **Given** `crates/f8-cli` and its exclusively used dependencies are removed from `Cargo.toml`
  - **When** `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo fmt --check` are executed
  - **Then** all commands succeed with zero warnings or errors across `f8-core` and `f8-python`.

- [x] **Scenario 2: Self-contained Python test suite with zero skips**
  - **Given** `crates/f8-cli/templates` is deleted and `test_project_io.py` / `test_audit.py` are updated
  - **When** `uv run pytest` is executed
  - **Then** all tests pass with `0 skipped`, including multi-tier header and section-header Markdown rendering in `test_audit.py`.

- [x] **Scenario 3: Zero stale CLI references in active documentation**
  - **Given** `docs/cli-authoring-guide.md` is deleted and `AGENTS.md`, `README.md`, `docs/architecture.md`, and `docs/dsl/spec-format.md` are updated
  - **When** searching active repository files (excluding `docs/specs/history/` and `docs/specs/drafts/`) for `f8-cli` or `cli-authoring-guide`
  - **Then** zero matches are returned.

---

## Tasks

- [x] 1. Delete `crates/f8-cli/` directory and `docs/cli-authoring-guide.md`.
- [x] 2. Update `Cargo.toml` to remove `"crates/f8-cli"` from `[workspace].members` and remove `clap`, `assert_cmd`, `predicates`, `tempfile`, and `libc` from `[workspace.dependencies]`.
- [x] 3. Update `python/tests/test_project_io.py` to remove `test_compile_fixture_templates`, and update `python/tests/test_audit.py` to replace `test_scenario_9_fixture_template_auditing` with an in-memory `Model`-based multi-tier header and section-header audit test without `pytest.skip`.
- [x] 4. Update `AGENTS.md`, `README.md`, `docs/architecture.md`, and `docs/dsl/spec-format.md` to remove `f8-cli` and `cli-authoring-guide.md` references.
- [x] 5. Run full Rust and Python verification suites (`cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, `uv run ruff check .`, `uv run ruff format --check .`, `uv run pyright`, `uv run pytest`).
