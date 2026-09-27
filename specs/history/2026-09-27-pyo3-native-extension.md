---
title: Native PyO3 extension (crates/f8-python)
date: 2026-09-27
status: completed
target-living-spec: specs/living-specs/interfaces/python-sdk.md
---

## Proposal

### What

Implement Phase 1 of the Python SDK roadmap defined in [`docs/interfaces/sdk.md`](../../docs/interfaces/sdk.md):
1. **Cargo workspace integration**: Create `crates/f8-python` as a `cdylib` crate wrapping `f8-core` via PyO3 (targeting stable ABI `abi3-py312`) and `pythonize`. Configure `[lib] name = "_core"` with `test = false` to prevent workspace `cargo test` linker failures.
2. **Python packaging and environment**: Set up root `pyproject.toml` targeting Python `>=3.12` configured with `maturin` and `uv` developer workflows, plus `python/f8` package root with `__init__.py`, `py.typed`, and `_core.pyi` type stubs.
3. **Thin C-ABI marshaling bridge**: Deserialize Python dictionary/JSON representations into Serde structures (`ProjectManifest`, `Vec<TableSpec>`, `Theme`) via `pythonize` in microseconds without filesystem I/O. Support built-in theme resolution by name or automatic fallback to the manifest theme.
4. **Compiled model inspection interface**: Expose native `PyCompiledModel` bridging in-memory methods (`emit_workbook`, `summary`, `inspect_cell`, `inspect_table`, `inspect_sheet`, `precedents_of`, `dependents_of`) supporting flexible coordinate inputs (3 positional tokens or 1 canonical IR string).
5. **Non-blocking CPU execution**: Release the Python GIL (`py.allow_threads`) during CPU-bound compilation and OpenXML workbook emission.
6. **Structured Python exceptions**: Map `CompilerError` and `InspectionError` variants into typed Python exception classes with structured attributes (`cycle`, `coordinate`, `target_table`, `row`, `col`, `token_offset`, `reason`, `message`), ensuring `message` propagates to base `PyException.__str__`.

### Why

The f8 architecture establishes Python as the primary "Code as Model" authoring layer for quantitative modelers and autonomous AI agents. Layer 1 (`crates/f8-python`) provides the native, in-process execution bridge to `f8-core`, eliminating CLI subprocess overhead while preserving the core engine's zero-I/O in-memory purity. Structured exception attributes ensure calling AI agents can diagnose and self-correct errors programmatically without regex string parsing. Targeting Python `>=3.12` with the `abi3-py312` stable ABI guarantees forward compatibility across Python 3.12, 3.13, and 3.14+ while unlocking modern type syntax (PEP 695).

### Scope

- **In-scope**:
  - `crates/f8-python` crate creation and Cargo workspace registration.
  - Root `pyproject.toml` targeting Python `>=3.12` with Maturin build backend and `uv` dev environment setup.
  - Pinning `.python-version` to `3.12` for deterministic hermetic interpreter resolution via `uv`.
  - Python package scaffolding in `python/f8/__init__.py`, `python/f8/py.typed`, and `python/f8/_core.pyi` re-exporting native primitives from `f8._core`.
  - Thin C-ABI PyO3 bridge module (`f8._core`).
  - Serializing/deserializing `f8-core` data structures via `pythonize`.
  - Flexible coordinate argument resolution (3 positional tokens or 1 canonical IR string) via `f8_core::CompiledModel::parse_coordinate`.
  - Native `PyCompiledModel` class wrapping `CompiledModel`.
  - Comprehensive custom Python exception hierarchy for compiler diagnostics, validation, inspection, and emission failures.
  - Releasing GIL during compilation and emission.
  - Verification via `uv run maturin develop`, `pytest`, `ruff`, `pyright`, and `cargo test`.

- **Out-of-scope**:
  - High-level fluent Python DSL (`f8.Model`, `Sheet`, `Table` builders) — deferred to Phase 2.
  - Project export (`model.export_project`) and disk project compilation (`f8.compile_project`) — deferred to Phase 2.
  - Third-party dataframe integrations (Pandas, Polars) — omitted by design; translation left to user scripts and standard dictionary protocols.

### What Changes

- `Cargo.toml`: Add `crates/f8-python` to workspace members; add `pyo3 = { version = "0.24", default-features = false, features = ["macros"] }` and `pythonize = "0.24"` to workspace dependencies.
- `crates/f8-python/Cargo.toml`: New crate definition with workspace-inherited package metadata (`version`, `edition`, `license`, `authors`), `[lib] name = "_core"`, `crate-type = ["cdylib"]`, `test = false` (prevents workspace `cargo test` linker failure), `pyo3` (with `extension-module` and `abi3-py312` features), `pythonize`, `serde`, `serde_json`, `indexmap`, `f8-core`, and `[lints] workspace = true` for zero-panic enforcement.
- `crates/f8-python/src/lib.rs`: PyO3 module initialization (`_core`), entrypoint functions, and module exports.
- `crates/f8-python/src/error.rs`: Structured Python exception classes and error mapping from `f8_core::CompilerError` and `InspectionError` using pattern matching.
- `crates/f8-python/src/model.rs`: `PyCompiledModel` class wrapping `f8_core::CompiledModel` with inspection queries (serialized via `pythonize`) and OpenXML emission.
- `pyproject.toml`: New root project configuration for Maturin (`manifest-path = "crates/f8-python/Cargo.toml"`, `module-name = "f8._core"`), `requires-python = ">=3.12"`, `ruff` (`target-version = "py312"`), `pyright` (`pythonVersion = "3.12"`, `extraPaths = ["python"]`), and `uv` dev workflows.
- `README.md`: New root project overview and quickstart document required by `pyproject.toml`.
- `.python-version`: Pinned CPython `3.12` for deterministic `uv` hermetic environment resolution.
- `python/f8/__init__.py`: Package entrypoint re-exporting core compilation, inspection, and exception classes with explicit `__all__`.
- `python/f8/_core.pyi`: PEP 561 type stub definitions for native functions, classes, and exceptions, providing 100% type coverage for `pyright`.
- `python/f8/py.typed`: PEP 561 marker indicating typing support.
- `.gitignore`: Add Python virtual environment (`.venv`), Python bytecode (`__pycache__/`, `*.pyc`), Maturin build outputs (`*.so`, `*.pyd`, `dist/`), `.ruff_cache`, and pytest cache artifacts.
- `python/tests/test_core_binding.py`: Python unit tests exercising compilation, cell inspection, workbook emission, and exception propagation.

### Capabilities

- **New capabilities**:
  - Python programs can compile f8 models in-memory by calling `f8._core.compile(manifest, tables, theme)` or `f8.compile(manifest, tables, theme)`.
  - Python programs can query evaluated cell values, formulas, coordinates, and dependency trees from `CompiledModel`.
  - Inspection methods accept either 3 positional arguments (`table, row, col`) or a single canonical IR coordinate string (`TABLE["row", "col"]`).
  - Python programs can emit `.xlsx` byte buffers directly from `CompiledModel.emit_workbook(options)`.
  - Long-running compilation and workbook emission release the GIL, enabling concurrent execution across Python worker threads.
  - Compiler errors raise structured Python exceptions with inspectable diagnostic fields (e.g. `e.cycle` for cyclic dependencies, `e.token_offset` for formula parse errors, `e.coordinate` for missing references).

- **Modified capabilities**:
  - None (sibling adapter to `crates/f8-cli`, leaves `f8-core` untouched).

---

## Design

### Goals

1. **Thin C-ABI facade**: Avoid duplicating every AST node as a `#[pyclass]`. Instead, use `pythonize` to convert Python dictionaries directly into existing `f8-core` Serde types.
2. **Zero filesystem I/O**: Preserve `f8-core`'s pure in-memory contract; no temporary files or disk reads.
3. **Structured diagnostic exceptions**: Provide programmatic access to error context (cycle paths, token offsets, target coordinates) so calling agents can self-correct without regex parsing error strings.
4. **Flexible coordinate ergonomics**: Support both 3-token (`table, row, col`) and 1-token canonical IR strings (`TABLE["row", "col"]`) across all inspection APIs.
5. **Non-blocking concurrency**: Release the Python GIL during heavy compilation and emission steps.
6. **Seamless local developer loop**: Enable instant compilation and testing via `uv run maturin develop && uv run pytest`.

### Invariants

1. **Zero production panics**: No `.unwrap()` or `.expect()` in `crates/f8-python`. All Rust errors map cleanly to structured `pyo3::PyErr`.
2. **Deterministic data structures**: Maintain `IndexMap` preservation during Serde conversion.
3. **Rust 2024 edition conformance**: All new Rust code adheres to Rust 2024 edition guidelines and clippy `-D warnings`.

### Decisions

- **Decision 1: Use `pythonize` over individual PyO3 class wrappers for AST inputs**
  - *Rationale*: `f8-core` already defines full Serde implementations for `ProjectManifest`, `TableSpec`, and `Theme`. Using `pythonize` serializes lightweight Python dicts directly into Serde structs in microseconds. This prevents massive boilerplate and prevents drift between the Rust AST and Python bindings.
  - *Alternatives considered*: Hand-written `#[pyclass]` for every AST struct (rejected: over 2,000 lines of glue code and high maintenance burden).

- **Decision 2: Expose `PyCompiledModel` as a PyO3 class wrapping `f8_core::CompiledModel`**
  - *Rationale*: Once compiled, inspection operations (`inspect_cell`, `inspect_table`, `precedents_of`) and emission (`emit_workbook`) operate against the immutable in-memory compiled model. Wrapping `CompiledModel` in a `#[pyclass]` provides fast, zero-copy method calls from Python.
  - *Alternatives considered*: Serializing the entire `CompiledModel` to a Python dict at compile time (rejected: wasteful memory allocation for large models when the caller only wants to inspect a few cells or emit the workbook).

- **Decision 3: Structured Python exceptions subclassing a base `F8Error`**
  - *Rationale*: Distinct exception classes (`CycleDetectedError`, `ReferenceResolutionError`, `SpecificationError`, `FormulaParseError`, `InspectionError`, `EmissionError`) allow clean Python `try/except` blocks and programmatic access to error metadata.
  - *Alternatives considered*: Generic `RuntimeError` or `ValueError` with string message (rejected: forces agents to parse strings, violating the strong typing principle).

- **Decision 4: Flexible coordinate parsing via `CompiledModel::parse_coordinate`**
  - *Rationale*: Python callers and LLM agents author coordinates in both formats: discrete arguments (`table, row, col`) and canonical IR strings (`OPERATIONS["gross_profit", "2024E"]`). Invoking `CompiledModel::parse_coordinate` directly on `self.inner` (which internally delegates to `f8_core::parse_coordinate_tokens`) unifies behavior with `f8-cli` and provides instant ergonomics with zero duplicate parsing logic.
  - *Alternatives considered*: Enforcing only 3 positional arguments (rejected: clumsy when coordinates are already represented as unified strings).

- **Decision 5: Hybrid package layout (`python/f8` + `f8._core`)**
  - *Rationale*: Packaging the native extension as `f8._core` inside a `python/f8` source tree allows `python/f8/__init__.py` to re-export native primitives immediately in Phase 1, while providing the clean foundation for the pure Python DSL (`f8.Model`, `Sheet`, `Table`) in Phase 2.
  - *Alternatives considered*: Monolithic native extension module named `f8` (rejected: prevents writing high-level Python code and type stubs in Phase 2 without rewriting package configuration).

- **Decision 6: Explicit GIL release during compilation and emission**
  - *Rationale*: `Compiler::compile` and `WorkbookEmitter::emit_to_buffer` are pure CPU-bound Rust operations with zero Python runtime interaction. Calling `py.allow_threads` releases the GIL, preventing f8 from blocking concurrent Python threads or background workers.
  - *Alternatives considered*: Keeping the GIL held (rejected: introduces latency spikes in concurrent Python applications).

- **Decision 7: Cargo workspace integration with Maturin `manifest-path` and stable ABI**
  - *Rationale*: Because the repository is a Cargo workspace, Maturin configures `manifest-path = "crates/f8-python/Cargo.toml"` with `module-name = "f8._core"`, and `crates/f8-python/Cargo.toml` declares `[lib] name = "_core"`. To prevent Cargo from generating a broken test binary that fails linking against `libpython` during root `cargo test`, `[lib] test = false` is explicitly set. Enabling `pyo3 = { features = ["extension-module", "abi3-py312"] }` compiles against the Python 3.12+ Limited API (PEP 384), producing a single wheel compatible across Python 3.12, 3.13, and 3.14+. The crate inherits `[lints] workspace = true` to strictly enforce zero unwrap/expect in production code.
  - *Alternatives considered*: Single root crate or separate standalone repository (rejected: fragments versioning, CI workflows, and monorepo cohesion); non-abi3 builds (rejected: requires building and distributing separate wheels for every micro/minor Python version).

- **Decision 8: Marshaling inspection queries via `pythonize`**
  - *Rationale*: Inspection results (`CellInspection`, `TableInspection`, `SheetInspection`, `ModelSummary`, `DependencyTree`) already implement `serde::Serialize` in `f8-core`. Returning Python dictionaries directly via `pythonize::pythonize` avoids defining dozens of read-only `#[pyclass]` wrappers while preserving fast in-memory query performance.
  - *Alternatives considered*: Hand-rolled PyO3 class hierarchy for inspection objects (rejected: high boilerplate and synchronization overhead).

- **Decision 9: Tooling parity with Cargo via `uv`, `ruff`, and `pyright` targeting Python `>=3.12`**
  - *Rationale*: Python 3.10 reached end of life (EOL in 2026), whereas Python 3.12 is the modern baseline supporting PEP 695 type parameter syntax and Faster CPython optimizations. Python development is orchestrated entirely via `uv` with `.python-version` pinned to `3.12`. Code formatting and linting are unified under `ruff` (`target-version = "py312"`), and static typing is verified with `pyright` (`pythonVersion = "3.12"`, `extraPaths = ["python"]`) backed by `python/f8/_core.pyi` type stubs and `python/f8/py.typed`.
  - *Alternatives considered*: Supporting Python 3.10 (rejected: EOL, lacks modern type syntax, and complicates ABI targeting); legacy tools like `flake8`/`black`/`isort` (rejected: slow, fragmented configuration) or `mypy` (rejected: significantly slower than `pyright`).

### Tooling Configuration Specifications

#### `pyproject.toml`
```toml
[build-system]
requires = ["maturin>=1.8,<2.0"]
build-backend = "maturin"

[project]
name = "f8"
version = "0.1.0"
description = "Financial modeling engine with compiler verification and live Excel formulas"
readme = "README.md"
requires-python = ">=3.12"
dependencies = []

[dependency-groups]
dev = [
    "maturin>=1.8",
    "pytest>=8.0",
    "ruff>=0.9",
    "pyright>=1.1",
]

[tool.maturin]
manifest-path = "crates/f8-python/Cargo.toml"
python-source = "python"
module-name = "f8._core"

[tool.uv]
cache-keys = [
    { file = "pyproject.toml" },
    { file = "Cargo.toml" },
    { file = "Cargo.lock" },
    { file = "crates/f8-python/Cargo.toml" },
    { file = "crates/f8-python/src/**/*.rs" },
    { file = "crates/f8-core/src/**/*.rs" },
]

[tool.ruff]
line-length = 100
target-version = "py312"

[tool.ruff.lint]
select = ["E", "F", "I", "UP", "B", "SIM"]

[tool.pyright]
include = ["python", "tests"]
extraPaths = ["python"]
pythonVersion = "3.12"
typeCheckingMode = "standard"

[tool.pytest.ini_options]
testpaths = ["tests"]
python_files = ["test_*.py"]
```

#### `crates/f8-python/Cargo.toml`
```toml
[package]
name = "f8-python"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[lib]
name = "_core"
crate-type = ["cdylib"]
test = false

[dependencies]
f8-core = { workspace = true }
indexmap = { workspace = true }
pyo3 = { workspace = true, features = ["abi3-py312", "extension-module"] }
pythonize = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }

[lints]
workspace = true
```

### Data Contracts

#### Python Module Interface (`f8._core` and `f8`)

```python
def compile(
    manifest: dict,
    tables: list[dict],
    theme: dict | str | None = None,
) -> CompiledModel: ...


class CompiledModel:
    def emit_workbook(self, options: dict | None = None) -> bytes: ...
    def summary(self) -> dict: ...
    def inspect_cell(self, target: str, row: str | None = None, col: str | None = None) -> dict: ...
    def inspect_table(self, table: str) -> dict: ...
    def inspect_sheet(self, sheet: str) -> dict: ...
    def precedents_of(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
        recursive: bool = False,
    ) -> dict: ...
    def dependents_of(
        self,
        target: str,
        row: str | None = None,
        col: str | None = None,
        recursive: bool = False,
    ) -> dict: ...


class F8Error(Exception):
    message: str


class CycleDetectedError(F8Error):
    cycle: list[str]


class ReferenceResolutionError(F8Error):
    coordinate: str
    target_table: str | None
    row: str | None
    col: str | None


class SpecificationError(F8Error):
    table: str | None
    reason: str


class FormulaParseError(F8Error):
    table: str
    row: str | None
    col: str | None
    token_offset: int | None


class InspectionError(F8Error):
    entity: str


class EmissionError(F8Error):
    pass
```

#### Theme Resolution Order in `compile`

When resolving `theme` in `compile(manifest, tables, theme=None)`:
1. **`theme is None`**: Resolve theme name from `manifest.get("theme")` via `ProjectManifest::resolve_theme(&manifest)` (falls back to built-in `corporate_navy`).
2. **`isinstance(theme, str)`**: Resolve built-in theme by name via `Theme::resolve_builtin(name)`.
3. **`isinstance(theme, dict)`**: Deserialize explicit theme specification via `pythonize::depythonize::<Theme>`.

#### CompilerError Mapping Matrix

All fields are extracted via pattern matching in `crates/f8-python/src/error.rs` against `f8_core` error variants:

| Rust Variant | Python Exception | Populated Attributes |
| :--- | :--- | :--- |
| `CompilerError::Model(ModelError)` | `SpecificationError` | `table = pattern_matched_table`, `reason = err.to_string()` |
| `CompilerError::Ir(IrError::FormulaParse)` | `FormulaParseError` | `table`, `row`, `col`, `token_offset` (`Some(offset)` or `None` on EOF), `message` |
| `CompilerError::Ir(other)` | `SpecificationError` | `table = pattern_matched_table`, `reason = err.to_string()` |
| `CompilerError::Graph(GraphError::CircularDependency)` | `CycleDetectedError` | `cycle = cycle.iter().map(ToString::to_string).collect()`, `message` |
| `CompilerError::Graph(GraphError::Resolve(ResolveError))` | `ReferenceResolutionError` | `coordinate = origin.to_string()`, `target_table`, `row`, `col` (mapped from `column`), `message` |
| `CompilerError::Eval(EvalError::Resolve(ResolveError))` | `ReferenceResolutionError` | `coordinate = origin.to_string()`, `target_table`, `row`, `col` (mapped from `column`), `message` |
| `CompilerError::Eval(other)` | `F8Error` | `message = err.to_string()` |
| `CompilerError::Layout(LayoutError)` | `SpecificationError` | `table = None`, `reason = err.to_string()` |
| `CompilerError::Lowering(LoweringError)` | `F8Error` | `message = err.to_string()` |
| `CompilerError::Emission(EmissionError)` | `EmissionError` | `message = err.to_string()` |
| `CompilerError::Inspection(InspectionError)` | `InspectionError` | `entity = pattern_matched_entity`, `message = err.to_string()` |
| `pythonize::PythonizeError` | `SpecificationError` | `table = None`, `reason = err.to_string()` |

*Note: When instantiating custom PyO3 exception structs, pass the formatted diagnostic message to the base `PyException` constructor (e.g. `PyClassInitializer::from(PyException::new_err(message)).add_subclass(...)`) so standard Python formatting (`str(err)` and tracebacks) displays the full diagnostic explanation. In `crates/f8-python/src/error.rs`, name the PyO3 exception struct `PyInspectionError` with `#[pyclass(name = "InspectionError", extends = F8Error)]` to cleanly avoid naming collisions with `f8_core::InspectionError`.*

#### Inspection Query Marshaling

All `PyCompiledModel` query methods (`inspect_cell`, `inspect_table`, `inspect_sheet`, `summary`, `precedents_of`, `dependents_of`) serialize their respective Rust structs (`CellInspection`, `TableInspection`, `SheetInspection`, `ModelSummary`, `DependencyTree`) directly to Python dictionaries via `pythonize::pythonize(py, &result)`. Because `TableInspection` converts coordinate keys to canonical string representations (`TABLE["row", "col"]`), dictionary keys remain valid and hashable.

### Risks & Trade-offs

- **Risk**: Python dictionary input lacks compile-time type validation before entering Rust.
  - *Mitigation*: In Phase 2, the high-level Python DSL (`f8.Model`) enforces static types with `py.typed` and `Literal` definitions. In Phase 1, `pythonize` returns informative `SpecificationError` exceptions detailing missing or malformed fields.
- **Risk**: GIL contention during heavy calculation or emission across multi-threaded Python runtimes.
  - *Mitigation*: PyO3 releases the GIL (`py.allow_threads(...)`) during `Compiler::compile` and `CompiledModel::emit_workbook`.
- **Risk**: Python unhashable dictionary keys when serializing inspection maps.
  - *Mitigation*: `f8_core::TableInspection` already provides custom serialization (`serialize_cells_map`) converting coordinate keys to canonical IR strings (`TABLE["row", "col"]`), guaranteeing valid Python dictionary keys.

---

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: In-memory compilation and workbook emission**
  - **Given** valid manifest, table, and theme dictionaries representing a model
  - **When** calling `f8._core.compile(manifest, tables, theme)` from Python
  - **Then** a `CompiledModel` instance is returned, and `emit_workbook()` returns valid non-empty OpenXML bytes (`PK\x03\x04...`).

- [x] **Scenario 2: Emission options with values-only freezing**
  - **Given** a compiled model with calculated formula cells
  - **When** calling `compiled.emit_workbook({"values_only": True})`
  - **Then** returns valid OpenXML bytes emitting evaluated values without live formulas.

- [x] **Scenario 3: Cell inspection with flexible coordinates**
  - **Given** a compiled model with calculated formula cells
  - **When** calling `inspect_cell("OPERATIONS", "gross_profit", "2024E")` or `inspect_cell('OPERATIONS["gross_profit", "2024E"]')`
  - **Then** both invocations return identical dictionaries containing `physical_address`, `evaluated_value`, `excel_formula`, and populated `precedents`.

- [x] **Scenario 4: Dependency tree querying (direct and recursive)**
  - **Given** a compiled model with calculation chains
  - **When** calling `precedents_of("OPERATIONS", "ebitda", "2024E", recursive=True)`
  - **Then** returns a dictionary representing the recursive `DependencyTree` with `root`, `direct_count`, `transitive_count`, and pruned nodes.

- [x] **Scenario 5: Model and sheet summary inspection**
  - **Given** a compiled model with multiple tables across sheets
  - **When** calling `compiled.summary()` and `compiled.inspect_sheet("Operations")`
  - **Then** `summary()` returns cell/driver/formula totals, and `inspect_sheet()` returns physical 2D grid dimensions and cell records.

- [x] **Scenario 6: Cyclic dependency error propagation**
  - **Given** a table with a formula cycle (e.g. `A = B` and `B = A`)
  - **When** attempting to compile the model
  - **Then** `f8._core.CycleDetectedError` is raised containing the ordered `cycle` list of coordinate strings and diagnostic message.

- [x] **Scenario 7: Reference resolution error propagation**
  - **Given** a table with a formula referencing a non-existent table or column
  - **When** attempting to compile the model
  - **Then** `f8._core.ReferenceResolutionError` is raised with `coordinate`, `target_table`, `row`, or `col` diagnostic fields.

- [x] **Scenario 8: Formula syntax parse error propagation**
  - **Given** a table with a syntactically invalid formula (e.g. unclosed parenthesis or invalid operator)
  - **When** attempting to compile the model
  - **Then** `f8._core.FormulaParseError` is raised with `table`, `row`, `col`, `token_offset`, and syntax error `message`.

- [x] **Scenario 9: Specification and deserialization error propagation**
  - **Given** an invalid manifest or table dictionary (e.g. missing required fields or duplicate row IDs)
  - **When** attempting to compile the model
  - **Then** `f8._core.SpecificationError` is raised containing the faulty `table` (if applicable) and descriptive `reason`.

- [x] **Scenario 10: Inspection entity not found error propagation**
  - **Given** a compiled model
  - **When** calling `inspect_cell("NONEXISTENT", "r", "c")` or `inspect_table("UNKNOWN")`
  - **Then** `f8._core.InspectionError` is raised detailing the missing entity.

---

## Tasks

### Phase 1: Environment & Workspace Setup
- [x] Add `.venv`, `__pycache__/`, `*.pyc`, `*.so`, `*.pyd`, `dist/`, `.pytest_cache`, `.ruff_cache` to `.gitignore`.
- [x] Create root `README.md` project overview and architecture links as required by `pyproject.toml`.
- [x] Create `.python-version` pinning `3.12` for deterministic `uv` hermetic environment resolution.
- [x] Create `pyproject.toml` configuring `maturin` build backend (`manifest-path = "crates/f8-python/Cargo.toml"`, `python-source = "python"`, `module-name = "f8._core"`), project metadata (`requires-python = ">=3.12"`), `ruff` (`target-version = "py312"`), `pyright` (`pythonVersion = "3.12"`, `extraPaths = ["python"]`), and `uv` dev dependencies (`maturin`, `pytest`, `ruff`, `pyright`).
- [x] Create `python/f8/__init__.py` re-exporting native primitives from `f8._core` (`compile`, `CompiledModel`, exception classes) with explicit `__all__`, plus `python/f8/py.typed` and `python/f8/_core.pyi` type stubs.
- [x] Create `crates/f8-python` directory and configure `crates/f8-python/Cargo.toml` with workspace-inherited package metadata (`version`, `edition`, `license`, `authors`), `[lib] name = "_core"`, `crate-type = ["cdylib"]`, `test = false` (prevents workspace `cargo test` linker failure), `pyo3 = { workspace = true, features = ["abi3-py312", "extension-module"] }`, `pythonize`, `serde`, `serde_json`, `indexmap`, `f8-core`, and `[lints] workspace = true`.
- [x] Add `crates/f8-python` to root `Cargo.toml` workspace members and register `pyo3 = { version = "0.24", default-features = false, features = ["macros"] }` and `pythonize` in `[workspace.dependencies]`.

### Phase 2: Native Bridge Implementation
- [x] Define Python exception hierarchy (`F8Error`, `CycleDetectedError`, `ReferenceResolutionError`, `SpecificationError`, `FormulaParseError`, `InspectionError` as `PyInspectionError`, `EmissionError`) with typed attributes in `crates/f8-python/src/error.rs`, propagating `message` to `PyException`.
- [x] Implement error mapping functions via pattern matching from `f8_core::CompilerError` (including `Eval` and `FormulaParse`) and `f8_core::InspectionError` into PyO3 exceptions.
- [x] Implement `PyCompiledModel` wrapper around `f8_core::CompiledModel` in `crates/f8-python/src/model.rs` supporting flexible coordinate arguments (`target`, `row`, `col`) via `CompiledModel::parse_coordinate`, inspection marshaling via `pythonize`, and releasing GIL during `emit_workbook`.
- [x] Implement `compile` entrypoint in `crates/f8-python/src/lib.rs` deserializing via `pythonize`, resolving themes (None/str/dict), and releasing GIL during compilation.
- [x] Register module entrypoint `_core` with all classes and functions.

### Phase 3: Verification & Python Tests
- [x] Build extension with `uv run maturin develop`.
- [x] Write comprehensive unit tests in `python/tests/test_core_binding.py` covering all 10 acceptance scenarios (compilation, values-only emission, flexible coordinate inspection, dependency trees, and structured exception attributes).
- [x] Run `cargo check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` (verifying workspace tests pass cleanly with `test = false` on cdylib), and `cargo fmt --check`.
- [x] Run `uv run ruff check .` and `uv run ruff format --check .`.
- [x] Run `uv run pyright`.
- [x] Run `uv run pytest` to verify all acceptance scenarios.
