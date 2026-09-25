---
title: Eliminate Path Matching and Use Logical Table IDs in SheetSpec
date: 2026-09-25
status: draft
target-living-spec:
  - specs/living-specs/model/spec-ast.md
  - specs/living-specs/compiler/cell-ir.md
  - specs/living-specs/interfaces/cli.md
---

## Proposal

### What

Refactor `SheetSpec.tables` from `Vec<PathBuf>` to `Vec<TableId>` across `f8-core`, eliminating the heuristic `matches_table_id` function and all filesystem path assumptions from the core compiler. Update `f8-cli` project loading to discover table specification files on disk, index them by `TableSpec.id`, and populate `ProjectManifest` using strongly typed `TableId`s.

### Why

- **Root-cause correctness**: `f8-core` is an in-memory compiler with zero OS I/O. A spreadsheet sheet logically contains calculation tables (`TableId`), not filesystem file paths (`PathBuf`).
- **Eliminate string heuristics**: `matches_table_id` performed fuzzy matching (stripping `.json`, stripping dashes/underscores, case insensitivity) between file paths and table IDs, violating the core principle of strong typing over string heuristics.
- **SDK & in-memory readiness**: Python SDK models (`sheet.add_table(id="OPERATIONS")`) construct tables purely in-memory with no backing disk files. Requiring fake `PathBuf` declarations (e.g. `"tables/OPERATIONS.json"`) in `SheetSpec` was an abstraction leak.

### Scope

- **In-scope**:
  - Update `SheetSpec.tables` to `Vec<TableId>` in `crates/f8-core/src/model/manifest.rs`.
  - Remove `matches_table_id` from `crates/f8-core/src/compiler/ir/expand.rs`.
  - Replace path matching with exact `TableId` lookups in compiler passes (`ir::expand_workbook`, `layout::LayoutPlanner`, `WorkbookEmitter::emit_to_buffer`, `CompiledModel`).
  - Update `ProjectLoader` in `crates/f8-cli` to discover table files in `tables/`, index them by `TableSpec.id`, and assemble `ProjectManifest`.
  - Update CLI project templates (`blank`, `three-statement`, `lbo`, `trading-comps`) to declare table IDs in `f8.json`.
  - Update test fixtures across `crates/f8-core/tests/` and `crates/f8-cli/tests/`.
- **Out-of-scope**:
  - Python SDK implementation (handled in subsequent SDK specs).
  - Changes to table calculation grammar, AST expressions, or evaluation engine.

### What Changes

- `crates/f8-core/src/model/manifest.rs`: `SheetSpec.tables` changed from `Vec<PathBuf>` to `Vec<TableId>`.
- `crates/f8-core/src/compiler/ir/expand.rs`: Remove `matches_table_id`; update `expand_workbook` to resolve tables by exact `t.id == *table_id`.
- `crates/f8-core/src/compiler/layout/mod.rs`: Update placement checks to test `sheet.tables.contains(&table.id)`.
- `crates/f8-core/src/compiler/emission/workbook.rs`: Update sheet table resolution to match by `t.id == *table_id`.
- `crates/f8-core/src/compiler/compiled/mod.rs`: Update `summary` and sheet inspections to use `TableId`.
- `crates/f8-cli/src/project/loader.rs`: Discover and parse `tables/*.json` from disk, index by `TableSpec.id`, and validate that all table IDs declared in `sheet.tables` exist.
- `crates/f8-cli/templates/**/f8.json`: Update `tables` arrays from relative paths (e.g. `"tables/assumptions.json"`) to logical IDs (e.g. `"Assumptions"`).

### Capabilities

- **New capabilities**: Direct in-memory assembly of `ProjectManifest` with zero filesystem path artifacts.
- **Modified capabilities**: Manifest files (`f8.json`) declare table IDs instead of file paths in `sheets[].tables`.

---

## Design

### Goals

- Strictly one way to associate tables with sheets: via `TableId`.
- Zero filesystem path references or fuzzy matching in `f8-core`.
- Backward-compatible or clean discovery in CLI loader for existing on-disk table layouts.

### Invariants

- **Table ID uniqueness**: Table IDs within a project remain globally unique.
- **Deterministic ordering**: Tables on a sheet are ordered strictly by their declaration order in `SheetSpec.tables`.
- **Zero OS I/O in core**: `f8-core` has zero filesystem dependencies and remains `wasm32-unknown-unknown` compatible.

### Decisions

- **Use `TableId` directly in `SheetSpec.tables`**:
  - *Rationale*: A sheet contains tables. Modeling this as `Vec<TableId>` aligns with compiler core simplicity and explicit domain typing.
  - *Alternatives considered*: Keeping `Vec<PathBuf>` with dummy paths in Python (rejected: abstraction leak, fragile heuristics).
- **CLI discovery via `tables/` directory scan and index**:
  - *Rationale*: The CLI discovers all `*.json` files in the project's `tables/` directory, parses each `TableSpec`, and indexes them by `id`. The `f8.json` sheet references tables by ID. This decouples file naming on disk from model identifiers.
  - *Alternatives considered*: Requiring `tables/<id>.json` strict filename parity (rejected: breaks if filenames differ in casing or style from table IDs).

### Data Contracts

#### In-Memory `SheetSpec` (`crates/f8-core/src/model/manifest.rs`)
```rust
pub struct SheetSpec {
    pub name: String,
    pub tab_color: Option<String>,
    pub freeze_panes: Option<FreezePanes>,
    pub show_gridlines: bool,
    pub tables: Vec<TableId>,
}
```

#### Canonical `f8.json` Representation
```json
{
  "model_name": "LBO_Model",
  "theme": "corporate_navy",
  "sheets": [
    {
      "name": "Operations",
      "tab_color": "#1F4E79",
      "show_gridlines": true,
      "tables": ["OPERATIONS", "DEBT_SCHEDULE"]
    }
  ]
}
```

### Risks & Trade-offs

- **Risk**: Existing templates and test fixtures use `"tables/operations.json"` in `f8.json`.
  - **Mitigation**: Update all 4 embedded CLI templates and CLI tests in the same change. In `ProjectLoader`, accept either table IDs or paths during migration by stripping filename stems if a path is encountered in `f8.json`.

---

## Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: In-memory workbook expansion by TableId**
  - **Given** a `ProjectManifest` with a sheet specifying `tables: vec![TableId::from("OPERATIONS")]`
  - **When** `expand_workbook` is called with a `TableSpec` having `id == "OPERATIONS"`
  - **Then** the table is successfully registered to the sheet with zero path comparisons.

- [ ] **Scenario 2: Missing table on sheet diagnostic**
  - **Given** a `ProjectManifest` specifying `tables: vec![TableId::from("NON_EXISTENT")]`
  - **When** `expand_workbook` is executed
  - **Then** `IrError::UnknownTable` is returned identifying `NON_EXISTENT`.

- [ ] **Scenario 3: CLI project discovery and compilation**
  - **Given** an on-disk project with `f8.json` referencing table IDs in `tables/`
  - **When** `f8 check` or `f8 compile` is executed
  - **Then** the loader discovers table files, matches IDs, and compilation succeeds with exit code `0`.

- [ ] **Scenario 4: Spatial layout and workbook emission parity**
  - **Given** compiled models with multiple stacked tables on a sheet
  - **When** spatial layout planning and `.xlsx` emission execute
  - **Then** tables are positioned and emitted in exact `SheetSpec.tables` sequence.

---

## Tasks

### Phase 1: Core AST & Compiler Passes
- [ ] Update `SheetSpec.tables` to `Vec<TableId>` in `crates/f8-core/src/model/manifest.rs`.
- [ ] Remove `matches_table_id` and update `ir::expand_workbook` in `crates/f8-core/src/compiler/ir/expand.rs`.
- [ ] Update table resolution in `crates/f8-core/src/compiler/layout/mod.rs`.
- [ ] Update table resolution in `crates/f8-core/src/compiler/emission/workbook.rs`.
- [ ] Update table resolution in `crates/f8-core/src/compiler/compiled/mod.rs`.
- [ ] Update all unit and integration test fixtures in `crates/f8-core/tests/`.

### Phase 2: CLI Loader & Templates
- [ ] Update `ProjectLoader` in `crates/f8-cli/src/project/loader.rs` to scan `tables/` and resolve `TableId`s.
- [ ] Update template manifests in `crates/f8-cli/templates/**/f8.json`.
- [ ] Update CLI integration tests in `crates/f8-cli/tests/`.

### Phase 3: Verification & Reconciliation
- [ ] Verify full test suite: `cargo test`.
- [ ] Verify zero warnings: `cargo clippy --all-targets -- -D warnings`.
- [ ] Verify formatting: `cargo fmt --check`.
- [ ] Reconcile living specs (`specs/living-specs/model/spec-ast.md`, `specs/living-specs/compiler/cell-ir.md`, `specs/living-specs/interfaces/cli.md`).
- [ ] Archive draft spec to `specs/history/`.
