---
title: JSON Schema Generation and IDE Real-Time Validation
date: 2026-09-23
status: draft
target-living-spec:
  - specs/living-specs/model/spec-ast.md
  - specs/living-specs/interfaces/cli.md
---

## Proposal

### What

Integrate JSON Schema generation into `f8-core` using `schemars` and introduce an `f8 schema` CLI command. Update project scaffolding in `f8 init` to output schema files and reference them via `$schema` in generated manifests and tables, providing real-time IDE validation, autocompletion, hover documentation, and syntax checking for financial analysts and AI agents.

### Why

Financial analysts and autonomous AI coding agents author declarative JSON specifications directly in text editors (VS Code, Cursor, Windsurf) and automated workflows. Without JSON schemas:
- Users and agents must guess field names and enum values.
- Typographical errors (e.g. `"descriptors"` instead of `"descriptor"`, or `"archetypes"` instead of `"tables"`) are discovered only after running compiler commands.
- IDEs cannot provide real-time red-squiggly linter feedback, autocomplete popups, or hover documentation.

Generating standards-compliant JSON Schemas directly from Rust structs establishes a single source of truth between the compiler's deserializer and editor tooling, eliminating hallucinated keys and syntax mistakes at authoring time.

### Scope

- **In-scope**:
  - Add `schemars` dependency with `derive` and `indexmap2` features to `f8-core`.
  - Derive `schemars::JsonSchema` on all public AST models in `f8-core`:
    - Identifiers: `TableId`, `RowId`, `ColId`, `SheetId` (via `define_id!`).
    - Format & Style: `FormatPreset`, `SemanticStyle`, `CurrencyCode`.
    - Manifest: `ProjectManifest`, `SheetSpec`, `FreezePanes`.
    - Table: `TableSpec`, `ColumnSpec`, `DataColumnSpec`, `AxisRole`, `RowSpec`, `DataRowSpec`, `SectionHeaderRowSpec`, `CellInput`, `DetailedCellInput`, `ValueLiteral`.
  - Provide public schema generators in `f8-core`:
    - `f8_core::schema::manifest_schema() -> schemars::schema::RootSchema`
    - `f8_core::schema::table_schema() -> schemars::schema::RootSchema`
  - Introduce `f8 schema` subcommand in `crates/f8-cli`:
    - `f8 schema [TARGET]` where `TARGET` can be `manifest`, `table`, or `all` (default: `all`).
    - `-o, --output <PATH>`: Write schema output to a specific file or stdout (`-`).
    - `-d, --dir <DIR>`: Write `f8.schema.json` and `f8.table.schema.json` to the target directory.
    - Global `--json` output support for machine-readable JSON status.
  - Update `f8 init` scaffolding in `crates/f8-cli`:
    - Write `f8.schema.json` and `f8.table.schema.json` into initialized project directories.
    - Add `"$schema": "./f8.schema.json"` to generated `f8.json`.
    - Add `"$schema": "../f8.table.schema.json"` to generated `tables/*.json`.
  - Comprehensive unit and integration test suite covering schema generation, schema validity, and CLI execution.

- **Out-of-scope**:
  - Formula expression grammar validation inside JSON Schema (formulas remain strings validated during compilation).
  - Custom theme file schemas (deferred to future theme authoring milestones).

### What Changes

- `Cargo.toml`: Add `schemars` to workspace dependencies.
- `crates/f8-core/Cargo.toml`: Add `schemars.workspace = true`.
- `crates/f8-core/src/`:
  - `model/id.rs`: Add `JsonSchema` to `define_id!` macro.
  - `model/format.rs`: Derive `JsonSchema` on `FormatPreset`, `SemanticStyle`, `CurrencyCode`.
  - `model/manifest.rs`: Derive `JsonSchema` on `ProjectManifest`, `SheetSpec`, `FreezePanes`.
  - `model/table.rs`: Derive/implement `JsonSchema` on `AxisRole`, `ColumnSpec`, `DataColumnSpec`, `RowSpec`, `DataRowSpec`, `SectionHeaderRowSpec`, `DetailedCellInput`, `ValueLiteral`, `CellInput`, `TableSpec`.
  - `schema/mod.rs` (new): Public schema generation facade.
  - `lib.rs`: Expose `schema` module.
- `crates/f8-cli/src/`:
  - `cli.rs`: Add `SchemaArgs` and `SchemaTarget` enum to clap command hierarchy.
  - `commands/schema.rs` (new): Command runner for `f8 schema`.
  - `commands/mod.rs`: Export `schema` command.
  - `main.rs`: Dispatch `Commands::Schema`.
  - `project/writer.rs`: Emit schema files and link `$schema` during `f8 init`.
  - `archetypes/mod.rs`: Include `$schema` fields in embedded archetypes.
- `crates/f8-cli/tests/`:
  - `schema_test.rs` (new): CLI integration tests for `f8 schema`.
  - `init_test.rs`: Verify schema files and `$schema` links are created on `f8 init`.

### Capabilities

- **New capabilities**:
  - Export machine-readable JSON schemas for `f8.json` and `tables/*.json` via `f8 schema`.
  - Real-time schema validation, autocomplete, and hover documentation in VS Code, Cursor, and Windsurf.
  - Autonomous AI agents can query schemas to guarantee correct property keys and types before saving files.
- **Modified capabilities**:
  - Newly initialized models created by `f8 init` contain `$schema` headers and bundled schema files.

---

## Design

### Goals

1. **Single Source of Truth**: Schema definitions derive strictly from Rust AST data structures.
2. **Zero Invalidation Drift**: Any structural change to Rust model structs automatically updates generated schemas.
3. **Seamless Scaffolding**: Projects initialized with `f8 init` work with editor autocomplete immediately without manual setup.
4. **Clean Stream Discipline**: Piped schema outputs write pure JSON to `stdout` without decorative logs.
5. **Zero Production Panics**: Strictly propagate errors; zero `.unwrap()` or `.expect()`.

### Invariants

1. **Exact Serde Symmetry**: Schema properties, optionality, enums, renames, and tags must match serde serialization/deserialization behavior 1:1.
2. **Deterministic Output**: Schemas must generate deterministically with sorted or consistent property orders.
3. **Exit Code Stability**: `f8 schema` follows standard 3-state exit codes (`0` for success, `1` for I/O / CLI errors).

### Decisions

- **Decision 1: Dual schemas (`f8.schema.json` and `f8.table.schema.json`) instead of a single merged schema**
  - *Rationale*: In the f8 multi-file architecture, the manifest (`f8.json`) and table files (`tables/*.json`) have distinct root schemas (`ProjectManifest` vs `TableSpec`). Separate schema files allow VS Code's JSON Language Server to bind `$schema` directly to the correct root type without requiring complex JSON pointers or schema switching.
  - *Alternatives considered*: Single schema with definitions (rejected: requires complex URI fragments like `f8.schema.json#/definitions/TableSpec` which complicates relative path resolution).

- **Decision 2: Manual `JsonSchema` implementation for `ValueLiteral`**
  - *Rationale*: `ValueLiteral` implements custom Serde visitor deserialization to accept numbers, strings, booleans, and nulls. Implementing `JsonSchema` explicitly using `schemars` primitive types ensures the generated schema accurately reflects `oneOf: [number, string, boolean, null]` matching the visitor.
  - *Alternatives considered*: Untagged enum derive (rejected: custom visitor requires explicit schema control to avoid serde attribute discrepancies).

- **Decision 3: `f8 schema` command defaults to exporting both schemas**
  - *Rationale*: Running `f8 schema` or `f8 schema --dir .` in a project root should provide everything needed for IDE support in a single command. Targeted exports (`f8 schema manifest`, `f8 schema table`) allow piping individual schemas to stdout or custom filenames.
  - *Alternatives considered*: Requiring a target argument always (rejected: added friction for common workflow).

### Data Contracts

#### CLI Interface

```text
f8 schema [TARGET] [OPTIONS]

Arguments:
  [TARGET]   Target schema to export: manifest, table, or all [default: all]

Options:
  -o, --output <PATH>  Output file path, or '-' for stdout
  -d, --dir <DIR>      Output directory for schema files [default: .]
      --json           Machine-readable JSON command status
  -h, --help           Print help
```

#### Core Schema Facade (`f8_core::schema`)

```rust
pub fn manifest_schema() -> schemars::schema::RootSchema;
pub fn table_schema() -> schemars::schema::RootSchema;
pub fn manifest_schema_json() -> Result<String, serde_json::Error>;
pub fn table_schema_json() -> Result<String, serde_json::Error>;
```

#### Manifest Schema Reference (`f8.json`)

```json
{
  "$schema": "./f8.schema.json",
  "model_name": "LBO Model",
  "theme": "corporate_navy",
  "sheets": []
}
```

#### Table Schema Reference (`tables/*.json`)

```json
{
  "$schema": "../f8.table.schema.json",
  "id": "income_statement",
  "title": "Income Statement",
  "columns": [],
  "rows": []
}
```

### Risks & Trade-offs

- **Risk**: Adding `schemars` increases compile times and dependency footprint.
  - *Mitigation*: `schemars` is lean and relies heavily on macro expansion without heavy runtime overhead. Keeping it in workspace dependencies ensures unified version resolution.
- **Risk**: Future model changes could forget doc-comments or serde attributes.
  - *Mitigation*: Schema integration tests will validate that schema generation succeeds, produces non-empty descriptions, and parses successfully as JSON schema.

---

### Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Generate Manifest Schema to Stdout**
  - **Given** `f8` CLI binary
  - **When** executing `f8 schema manifest -o -`
  - **Then** emits valid JSON Schema to `stdout` containing `ProjectManifest` properties (`model_name`, `theme`, `sheets`) and exits with code `0`.

- [ ] **Scenario 2: Generate Table Schema to Stdout**
  - **Given** `f8` CLI binary
  - **When** executing `f8 schema table -o -`
  - **Then** emits valid JSON Schema to `stdout` containing `TableSpec` properties (`id`, `columns`, `rows`, `AxisRole`, `ColumnSpec`, `RowSpec`, `CellInput`) and exits with code `0`.

- [ ] **Scenario 3: Export Both Schemas to Directory**
  - **Given** an empty directory
  - **When** executing `f8 schema all --dir <dir>`
  - **Then** writes `f8.schema.json` and `f8.table.schema.json` to `<dir>` and exits with code `0`.

- [ ] **Scenario 4: Machine-Readable JSON Output on Export**
  - **Given** an output directory
  - **When** executing `f8 schema all --dir <dir> --json`
  - **Then** outputs `{"status": "ok", "schemas": [...]}` to `stdout` and exits with code `0`.

- [ ] **Scenario 5: Project Scaffolding Bundles Schemas and References**
  - **Given** an empty directory
  - **When** executing `f8 init my_model --template blank`
  - **Then** creates `f8.schema.json` and `f8.table.schema.json`, `f8.json` contains `"$schema": "./f8.schema.json"`, and table JSON files contain `"$schema": "../f8.table.schema.json"`.

- [ ] **Scenario 6: Schema Rejects Invalid Property and Enum in Validation Test**
  - **Given** the generated table schema
  - **When** validating JSON containing invalid `role: "descriptors"` or negative `indent: -1`
  - **Then** the schema validator flags schema validation errors.

---

## Tasks

### Phase 1: Core Dependencies & AST Annotations
- [ ] Add `schemars = { version = "1.2", features = ["derive", "indexmap2"] }` to workspace and `f8-core`.
- [ ] Annotate `define_id!` macro with `#[derive(schemars::JsonSchema)]`.
- [ ] Annotate `FormatPreset`, `SemanticStyle`, `CurrencyCode` in `model/format.rs` with `#[derive(schemars::JsonSchema)]`.
- [ ] Annotate `ProjectManifest`, `SheetSpec`, `FreezePanes` in `model/manifest.rs` with `#[derive(schemars::JsonSchema)]`.
- [ ] Annotate `AxisRole`, `ColumnSpec`, `DataColumnSpec`, `RowSpec`, `DataRowSpec`, `SectionHeaderRowSpec`, `DetailedCellInput`, `CellInput`, `TableSpec` in `model/table.rs`.
- [ ] Implement `schemars::JsonSchema` for `ValueLiteral`.
- [ ] Create `crates/f8-core/src/schema/mod.rs` with schema generator helpers.
- [ ] Run `cargo check -p f8-core` and unit tests.

### Phase 2: CLI Subcommand (`f8 schema`)
- [ ] Add `Schema` subcommand and arguments to `crates/f8-cli/src/cli.rs`.
- [ ] Implement `crates/f8-cli/src/commands/schema.rs`.
- [ ] Connect `schema` subcommand dispatch in `crates/f8-cli/src/main.rs`.
- [ ] Add CLI integration tests in `crates/f8-cli/tests/schema_test.rs`.
- [ ] Verify exit codes and stream discipline.

### Phase 3: Project Scaffolding & Archetypes Integration
- [ ] Update `f8_cli::project::writer` to emit `f8.schema.json` and `f8.table.schema.json`.
- [ ] Update archetypes (`blank`, `three-statement`, `lbo`, `trading-comps`) to include `$schema` properties.
- [ ] Update `tests/init_test.rs` to assert schema generation and references.

### Phase 4: Verification Suite & Documentation
- [ ] Run verification suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- [ ] Update living specs `specs/living-specs/model/spec-ast.md` and `specs/living-specs/interfaces/cli.md`.
- [ ] Reconcile and archive draft spec to `specs/history/`.
