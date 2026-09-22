---
title: Decouple Core Filesystem I/O
date: 2026-09-22
status: complete
target-living-spec:
  - specs/living-specs/compiler/workbook-emission.md
  - specs/living-specs/model/theme.md
---

## Proposal

### What

Refactor `crates/f8-core` to eliminate all direct filesystem I/O (`std::fs`), establishing `f8-core` as a pure, 100% in-memory calculation and OpenXML emission compiler:
1. Remove `Compiler::save_workbook` and `WorkbookEmitter::emit_to_file`, retaining `Compiler::emit_workbook` and `WorkbookEmitter::emit_to_buffer` as the sole emission entrypoints returning `Vec<u8>`.
2. Refactor theme resolution: replace path-based filesystem loading in `Theme::resolve` with `Theme::resolve_builtin` (catalog lookup) and `Theme::from_json_str` (string deserialization and validation).
3. Update `ProjectManifest::resolve_theme` to resolve built-in themes in-memory without filesystem path parameters.
4. Streamline `ModelError`: remove `ModelError::ThemeIoError` and eliminate `PathBuf` references from `ThemeNotFound` and `ThemeParseError`.
5. Update core integration tests and examples to handle disk persistence directly via caller-level `std::fs::write`.

### Why

As defined in [Architecture Overview](../../docs/architecture.md) and [Codebase Structure](../../docs/codebase-structure.md), f8 adheres strictly to a "Functional Core, Imperative Shell" architecture:
- **WebAssembly target readiness**: `std::fs` does not exist on `wasm32-unknown-unknown`. Removing filesystem calls from `f8-core` guarantees that the compiler core can be compiled and executed in browser workers and WASM runtimes without runtime traps or OS shims.
- **Local reasoning and single responsibility**: A calculation compiler's single responsibility is transforming domain ASTs into evaluated matrices and byte streams. Operating system concerns (file path canonicalization, disk permissions, directory traversal, file writes) belong exclusively in outer interface adapters (`f8-cli`, `f8-mcp`).
- **Deterministic and leak-free testing**: Pure in-memory core tests eliminate temp file creation, leftover test artifacts, and filesystem race conditions during parallel test execution (`cargo test`).

### Scope

- **In-scope**:
  - Remove `WorkbookEmitter::emit_to_file` from `crates/f8-core/src/compiler/emission/workbook.rs`.
  - Remove `Compiler::save_workbook` from `crates/f8-core/src/compiler/mod.rs`.
  - Remove `std::fs::read_to_string` from `crates/f8-core/src/model/theme.rs`.
  - Implement `Theme::from_json_str(json: &str) -> Result<Self, ModelError>` and `Theme::resolve_builtin(name: &str) -> Result<Self, ModelError>`.
  - Update `ProjectManifest::resolve_theme(&self) -> Result<Theme, ModelError>`.
  - Update `ModelError` variants (`ThemeNotFound { name: String }`, `ThemeParseError { source: serde_json::Error }`, remove `ThemeIoError`).
  - Update all integration tests in `crates/f8-core/tests/` and example runners in `crates/f8-core/examples/`.
  - Verify zero `std::fs` calls remain in `crates/f8-core/src/`.
  - Verify clean compilation against both native target and `wasm32-unknown-unknown`.

- **Out-of-scope**:
  - Implementing CLI subcommands (`f8 compile`, `f8 scaffold`, `f8 init`) in `f8-cli` (scheduled for dedicated CLI milestone).
  - Bundling embedded templates into `f8-cli` (scheduled for dedicated CLI milestone).
  - `wasm-bindgen` JavaScript bindings or web packaging (Phase 3).

### What Changes

- `crates/f8-core/src/model/error.rs`:
  - Remove `ThemeIoError { path: PathBuf, source: std::io::Error }`.
  - Simplify `ThemeNotFound { name: String }` (remove `path`).
  - Simplify `ThemeParseError { source: serde_json::Error }` (remove `path`).
- `crates/f8-core/src/model/theme.rs`:
  - Remove `std::fs` import and disk file reading.
  - Add `Theme::resolve_builtin(name: &str) -> Result<Self, ModelError>`.
  - Add `Theme::from_json_str(s: &str) -> Result<Self, ModelError>`.
  - Deprecate/remove `Theme::resolve(name_or_path, base_dir)`.
- `crates/f8-core/src/model/manifest.rs`:
  - Update `ProjectManifest::resolve_theme(&self) -> Result<Theme, ModelError>` (drop `base_dir: Option<&Path>`).
- `crates/f8-core/src/compiler/emission/workbook.rs`:
  - Remove `WorkbookEmitter::emit_to_file`.
  - Remove `use std::path::Path`.
- `crates/f8-core/src/compiler/mod.rs`:
  - Remove `Compiler::save_workbook`.
  - Remove `use std::path::Path`.
- `crates/f8-core/tests/`:
  - Update `tests/workbook_emission_test.rs`, `tests/theme_test.rs`, and `tests/export_integration_test.rs`.
- `crates/f8-core/examples/`:
  - Update `examples/export_models.rs` to write emitted buffers via `std::fs::write`.

### Capabilities

- **New capabilities**:
  - `Theme::from_json_str` provides direct, in-memory custom theme validation from string content.
  - `f8-core` guarantees 100% zero filesystem I/O across all internal modules.
- **Modified capabilities**:
  - `Compiler::emit_workbook` is the single source of truth for workbook emission. Callers wanting to write to disk do `std::fs::write(path, Compiler::emit_workbook(...)?);`.
  - `ProjectManifest::resolve_theme()` operates entirely in memory without requiring a base directory.

---

## Design

### Goals

1. **Pure in-memory core**: Zero occurrences of `std::fs` in `crates/f8-core/src/`.
2. **Zero functional regression**: All existing calculation, formatting, and OpenXML emission tests pass identically.
3. **WASM readiness**: `cargo check -p f8-core --target wasm32-unknown-unknown` succeeds cleanly.
4. **Ergonomic caller handoff**: Callers persist workbooks using idiomatic standard library functions (`std::fs::write`).

### Invariants

1. **No production panics**: Zero `.unwrap()` or `.expect()` in `crates/f8-core/src/`.
2. **Deterministic emission**: Emitted `.xlsx` byte buffers are bit-for-bit identical to prior outputs.
3. **Strict error categorization**: `ModelError` represents domain and validation errors only, with no leaked filesystem I/O errors.

### Decisions

- **Choice 1: Pure in-memory emission via `Compiler::emit_workbook`**:
  - *Rationale*: A compiler library should produce an in-memory buffer (`Vec<u8>`). Outer binaries or callers choose whether to stream it to stdout, write it to disk, or send it over HTTP/WASM. Having a redundant `save_workbook` inside the core engine ties `f8-core` needlessly to filesystem traits.
  - *Alternatives considered*: Keeping `save_workbook` behind a `#[cfg(feature = "std-io")]` feature flag. Rejected because `f8-core` has no reason to own file writing at all when outer adapters (`f8-cli`, `f8-mcp`) already exist for that express purpose.
- **Choice 2: Split theme resolution into `resolve_builtin` and `from_json_str`**:
  - *Rationale*: Manifests using built-in themes (`corporate_navy`) resolve instantly with zero allocations or file checks. Manifests or users providing custom theme files are loaded by outer adapters (`f8-cli`) via `std::fs::read_to_string` and parsed using `Theme::from_json_str`.
  - *Alternatives considered*: Passing a custom reader trait `io::Read`. Rejected as unnecessary complexity; themes are small JSON documents where passing `&str` is idiomatic and clean.
- **Choice 3: Remove `ThemeIoError` from `ModelError`**:
  - *Rationale*: If `f8-core` cannot perform I/O, `ThemeIoError` is dead code. Outer CLI adapters will wrap their own filesystem errors using `thiserror` or `anyhow`.

### Risks & Trade-offs

- **Risk: Breaking existing integration test call sites**:
  - *Mitigation*: Update all integration test calls from `Compiler::save_workbook` to `Compiler::emit_workbook` followed by `std::fs::write`.
- **Risk: Custom theme loading in CLI**:
  - *Mitigation*: The CLI binary will read the file and call `Theme::from_json_str`.

---

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Pure in-memory workbook emission**
  - **Given** a valid `ProjectManifest`, `TableSpec` slice, and `Theme`
  - **When** `Compiler::emit_workbook` is called
  - **Then** a valid `.xlsx` byte buffer (`Vec<u8>`) is returned without performing any filesystem access.

- [x] **Scenario 2: Built-in theme resolution without filesystem access**
  - **Given** a `ProjectManifest` declaring `theme: "corporate_navy"` or an empty string
  - **When** `manifest.resolve_theme()` is called
  - **Then** the built-in `corporate_navy` theme is returned successfully without accessing disk.

- [x] **Scenario 3: Custom theme parsing from JSON string**
  - **Given** a valid JSON string representing a custom theme
  - **When** `Theme::from_json_str(json)` is called
  - **Then** the custom theme is deserialized, validated, and returned.

- [x] **Scenario 4: Rejection of unknown theme name**
  - **Given** an unrecognized theme name (e.g. `"neon_cyberpunk"`)
  - **When** `Theme::resolve_builtin("neon_cyberpunk")` is called
  - **Then** it returns `ModelError::ThemeNotFound { name: "neon_cyberpunk".to_string() }`.

- [x] **Scenario 5: Zero `std::fs` in `crates/f8-core/src`**
  - **Given** `crates/f8-core/src/`
  - **When** grepping for `std::fs`
  - **Then** zero occurrences are found.

- [x] **Scenario 6: WebAssembly target compilation**
  - **Given** `crates/f8-core`
  - **When** running `cargo check -p f8-core --target wasm32-unknown-unknown`
  - **Then** compilation finishes successfully with zero warnings and zero errors.

---

## Tasks

1. **Phase 1: Model & Theme refactoring**:
   - [x] Remove `ThemeIoError` and update `ThemeNotFound` / `ThemeParseError` in `crates/f8-core/src/model/error.rs`.
   - [x] Replace `Theme::resolve` with `Theme::resolve_builtin` and `Theme::from_json_str` in `crates/f8-core/src/model/theme.rs`.
   - [x] Update `ProjectManifest::resolve_theme(&self)` in `crates/f8-core/src/model/manifest.rs` to drop `base_dir`.
2. **Phase 2: Compiler emission refactoring**:
   - [x] Remove `WorkbookEmitter::emit_to_file` from `crates/f8-core/src/compiler/emission/workbook.rs`.
   - [x] Remove `Compiler::save_workbook` from `crates/f8-core/src/compiler/mod.rs`.
3. **Phase 3: Integration test & example runner updates**:
   - [x] Update `crates/f8-core/tests/theme_test.rs` to test `resolve_builtin` and `from_json_str`.
   - [x] Update `crates/f8-core/tests/workbook_emission_test.rs` to test saving via `Compiler::emit_workbook` + `std::fs::write`.
   - [x] Update `crates/f8-core/tests/export_integration_test.rs` to write buffers directly in disk export scenarios.
   - [x] Update `crates/f8-core/examples/export_models.rs` to write buffers via `std::fs::write`.
4. **Phase 4: Living specs reconciliation & verification**:
   - [x] Update `specs/living-specs/model/theme.md` to reflect `Theme::resolve_builtin` and `Theme::from_json_str`.
   - [x] Update `specs/living-specs/compiler/workbook-emission.md` removing `save_workbook` and `emit_to_file`.
   - [x] Run `cargo check --workspace --all-targets`.
   - [x] Run `cargo test --workspace`.
   - [x] Run `cargo clippy --workspace --all-targets -- -D warnings`.
   - [x] Run `cargo fmt --check`.
   - [x] Run `cargo check -p f8-core --target wasm32-unknown-unknown`.
