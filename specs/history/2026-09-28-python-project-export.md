---
title: Python Project Serialization & Disk Interoperability
date: 2026-09-28
status: completed
target-living-spec: specs/living-specs/interfaces/python-sdk.md
---

## Proposal

### What

Implement multi-file JSON project export (`model.export_project(dir_path)`) and direct project directory compilation (`f8.compile_project(dir_path)`) in the Python SDK. This establishes seamless two-way interoperability between Python "Code as Model" definitions, the canonical multi-file JSON intermediate representation (`f8.json` + `tables/*.json`), and headless `f8-cli` verification workflows without invoking subprocesses.

### Why

While Python scripts (`model.py`) serve as the primary authoring source of truth for quantitative analysts and AI agents, the multi-file JSON specification (`f8.json` + `tables/*.json`) remains the language-agnostic Intermediate Representation (IR) across the f8 ecosystem. It enables:
1. Deterministic Git diffs and branch reviews of declarative specifications.
2. Headless CI validation and verification using static binaries (`f8-cli check` / `f8-cli verify`) without requiring a Python environment.
3. Cross-language model exchange with future web and WASM frontends.

Providing pure Python project export and direct project loading completes the artifact lifecycle defined in [`docs/interfaces/sdk.md`](../../docs/interfaces/sdk.md) while preserving the zero-filesystem-I/O boundary of `f8-core`.

### Scope

- **In-Scope**:
  - `Model.export_project(dir_path, overwrite=False, indent=2)` exporting in-memory model instances to canonical `f8.json` and `tables/<table_id>.json`.
  - Directory management: automatic directory and `tables/` subdirectory creation with path normalization (`str` and `pathlib.Path`).
  - Overwrite safety: raise `FileExistsError` if the destination directory contains existing model files and `overwrite=False`.
  - `f8.compile_project(dir_path) -> CompiledModel`: load an existing multi-file JSON project from disk in pure Python and compile it directly via `f8.compile()`.
  - Deterministic serialization: maintain exact insertion order of sheets, tables, columns, and rows without dictionary key reordering.
  - Round-trip fidelity: models compiled directly in-memory evaluate to identical cell values, formulas, and layouts as models exported to JSON and recompiled via `compile_project`.
  - Comprehensive unit test suite in `python/tests/test_project_io.py`.

- **Out-of-Scope**:
  - Compressed `.f8` archive packaging or single-file binary bundles.
  - Git repository initialization or automatic commit generation.
  - Subprocess wrappers around `f8-cli`.

### What Changes

- **`python/f8/builder.py`**: Add `export_project` method to `Model` using `to_spec()` and pure Python filesystem I/O.
- **`python/f8/loader.py`**: New module implementing `compile_project(dir_path: str | Path) -> CompiledModel` that reads and validates `f8.json` and `tables/*.json` from disk and calls `f8.compile()`.
- **`python/f8/__init__.py`**: Re-export `compile_project`.
- **`python/tests/test_project_io.py`**: Test suite covering export, overwrite safety, loader parsing, and end-to-end round-trip fidelity against `f8-cli` fixture projects.

### Capabilities

- **New capabilities**:
  - Export Python models to canonical multi-file JSON project directories using `model.export_project("./models/lbo")`.
  - Compile existing on-disk JSON projects directly in Python using `compiled = f8.compile_project("./models/lbo")` without CLI subprocesses.
  - Round-trip fidelity guarantee: `model.compile().summary() == f8.compile_project(dir).summary()`.

- **Modified capabilities**: None.

---

## Design

### Goals

1. **Zero Core I/O Leakage**: All filesystem operations (`open`, `mkdir`, `read_text`, `write_text`) are executed entirely in pure Python using `pathlib.Path`. `f8-core` remains 100% in-memory and zero-I/O.
2. **Deterministic Layout**: Manifest and table files serialize deterministically, producing stable Git diffs.
3. **Safety by Default**: Refuse to overwrite existing files unless explicitly instructed via `overwrite=True`.
4. **Schema Parity**: Serialized JSON outputs match the exact schemas expected by `f8-cli` and `f8_core::ProjectManifest` / `TableSpec`.

### Invariants

- **Canonical Structure**: Every exported project directory MUST contain:
  - `<dir_path>/f8.json`
  - `<dir_path>/tables/`
  - `<dir_path>/tables/<table_id>.json` for each table declared in the model.
- **Schema Conformity**: Exported JSON files MUST omit `null` and default fields where `serde(skip_serializing_if)` is declared in Rust models to prevent bloat.
- **Fail-Fast Safety**: If `dir_path / "f8.json"` already exists and `overwrite=False`, raise `FileExistsError` before modifying any files on disk.

### Decisions

- **Decision 1: Pure Python Implementation (`pathlib` + `json`)**:
  - _Rationale_: Reading and writing JSON files is standard, high-performance in Python 3.12+. Keeping disk I/O in Python avoids adding OS filesystem dependencies or error marshaling to `crates/f8-python`.
  - _Alternatives considered_: Implementing project export in Rust `f8-cli`. Rejected because Python scripts are the authoring source of truth and must be able to export without relying on CLI binary installation.

- **Decision 2: Explicit Overwrite Protection**:
  - _Rationale_: Prevents accidental destruction of existing modeling projects.
  - _Choice_: Default `overwrite=False`. If files exist, raise `FileExistsError("Project files already exist in ... Use overwrite=True to replace.")`.

- **Decision 3: Symmetrical Ingestion via `compile_project`**:
  - _Rationale_: Python users and automated evaluation scripts frequently need to run models defined on disk without installing or shelling out to `f8-cli`.
  - _Choice_: Provide `f8.compile_project(dir_path)` directly in the top-level package.

### Data Contracts

#### Interface Signatures

```python
from pathlib import Path
from f8._core import CompiledModel

# On Model class in python/f8/builder.py
class Model:
    ...
    def export_project(
        self,
        dir_path: str | Path,
        overwrite: bool = False,
        indent: int = 2,
    ) -> None: ...

# Top-level loader function in python/f8/loader.py and python/f8/__init__.py
def compile_project(
    dir_path: str | Path,
    theme: str | None = None,
) -> CompiledModel: ...
```

### Risks & Trade-offs

- **Risk**: Partial export write failure (e.g. disk full after writing 2 of 5 tables).
  - _Mitigation_: Validate entire model and synthesize specs via `to_spec()` in memory *before* initiating any filesystem writes. Write to temporary staging files or perform atomic directory updates if required.
- **Risk**: Inconsistent table file naming (e.g. casing or invalid characters).
  - _Mitigation_: Table files are named strictly `<dir_path>/tables/<table.id>.json`. Table identifiers are already validated by the builder to be valid alphanumeric/underscore identifiers.

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Canonical Multi-File JSON Export**
  - **Given** a valid `Model` instance with 2 sheets and 3 tables.
  - **When** calling `model.export_project(dir_path)`.
  - **Then** creates `<dir_path>/f8.json` containing the manifest and `<dir_path>/tables/<table_id>.json` for each table.
  - **And** all JSON files are formatted with 2-space indentation and valid UTF-8.

- [x] **Scenario 2: Overwrite Protection**
  - **Given** an existing project directory containing `f8.json`.
  - **When** calling `model.export_project(dir_path, overwrite=False)`.
  - **Then** raises `FileExistsError` without modifying any disk files.
  - **When** calling `model.export_project(dir_path, overwrite=True)`.
  - **Then** successfully updates the project files on disk.

- [x] **Scenario 3: Direct Project Compilation from Disk**
  - **Given** an on-disk project directory with `f8.json` and tables.
  - **When** calling `f8.compile_project(dir_path)`.
  - **Then** compiles the model into `CompiledModel` with zero CLI subprocess execution.

- [x] **Scenario 4: Round-Trip Evaluation Fidelity**
  - **Given** a complex model authored in Python containing formulas, cell overrides, gutters, and freeze panes.
  - **When** compiling directly in memory via `model.compile()` and compiling the exported project via `f8.compile_project(dir_path)`.
  - **Then** both `CompiledModel` instances produce identical summary metrics, evaluated cell values, and lowered formulas.

- [x] **Scenario 5: Missing Project Manifest Error Handling**
  - **Given** a directory without `f8.json`.
  - **When** calling `f8.compile_project(dir_path)`.
  - **Then** raises `FileNotFoundError` with a clear message indicating `f8.json` is missing.

---

## Tasks

### Phase 1: Pure Python Project Exporter

- [x] Implement `Model.export_project(dir_path, overwrite=False, indent=2)` in `python/f8/builder.py`.
- [x] Add directory validation, parent creation, and `FileExistsError` check.
- [x] Format and serialize `f8.json` and `tables/<id>.json` using `to_spec()`.

### Phase 2: Direct Project Loader & Compiler

- [x] Implement `compile_project(dir_path: str | Path, theme: str | None = None) -> CompiledModel` in `python/f8/loader.py`.
- [x] Load and parse `f8.json` and contained `tables/*.json` using standard `json.loads`.
- [x] Delegate to `f8._core.compile(manifest, tables, theme)`.
- [x] Re-export `compile_project` in `python/f8/__init__.py`.

### Phase 3: Verification & Round-Trip Test Suite

- [x] Add comprehensive tests in `python/tests/test_project_io.py`:
  - Test export creation and directory structure.
  - Test overwrite protection.
  - Test round-trip evaluation against fixture models.
  - Test missing manifest error reporting.
- [x] Verify complete project suite:
  - `cargo test && cargo clippy --all-targets -- -D warnings && cargo fmt --check`
  - `uv run ruff check . && uv run ruff format --check . && uv run pyright && uv run pytest`

### Phase 4: Living Spec Reconciliation

- [x] Update living spec `specs/living-specs/interfaces/python-sdk.md` with project export and loader signatures.
- [x] Archive completed draft spec to `specs/history/`.
