---
title: CLI inspection ergonomics and formatting polish
date: 2026-09-23
status: draft
target-living-spec:
  - specs/living-specs/interfaces/cli.md
  - specs/living-specs/compiler/compiled-model.md
  - specs/living-specs/model/spec-ast.md
---

## Proposal

### What

Implement Phase 3 of the CLI inspection ergonomics roadmap identified in [`docs/interfaces/cli-audit.md`](../../docs/interfaces/cli-audit.md):
1. **Dual coordinate input**: Allow `f8 inspect cell`, `precedents`, and `dependents` to accept coordinates either as three positional arguments (`<table> <row> <col>`) or as a single canonical IR string (`'Table["row", "col"]'`).
2. **Integer format preset and calendar year typography**: Introduce `FormatPreset::Integer` (`"integer"`) for unformatted whole numbers and apply year heuristics so calendar years (e.g. `2029`) never render with thousands commas (`2,029`).
3. **Multi-tier column header rendering**: Render stacked column category headers in terminal box-drawing tables and markdown views when columns declare multi-level header tiers.

### Why

- **Friction in coordinate querying**: AI agents copy cell references directly from compilation error messages and dependency trees formatted as canonical IR strings (e.g. `Assumptions["rev_growth", "val"]`). Restricting CLI commands to 3 space-separated arguments requires agents to manually parse the string into separate tokens, leading to syntax errors and failed agent tool calls.
- **Year formatting defects**: Financial models frequently project calendar years as calculated rows (e.g. `YEAR(...)` in `Timeline`). Because `format: "number"` forces thousands grouping, years display as `2,024`–`2,029`, violating accounting presentation conventions.
- **Omission of header categories**: Institutional tables use multi-tier column headers (e.g. grouping `y1`–`y5` under `"5-Year Investment Horizon"` and `"Project Titan LBO"`). Currently, the CLI renders only the lowest tier, stripping critical category context from terminal output.

### Scope

- **In-scope**:
  - Add `FormatPreset::Integer` to `f8-core::model::format` with `"integer"` serde tag.
  - Update `f8-core::model::theme` to include default integer format mask (`"0;(-0);\"-\"_)"`).
  - Update `format_accounting_number` in `f8-core::value::format` to omit thousands separators for `FormatPreset::Integer` and for integers in the calendar year range `[1900..=2100]` with `0` decimals.
  - Add coordinate parsing helper `parse_coordinate_args(target: &str, row: Option<&str>, col: Option<&str>) -> Result<(TableId, RowId, ColId), ...>` to resolve 3 arguments or 1 canonical IR string (`Table["row", "col"]`).
  - Update `InspectCellArgs`, `InspectPrecedentsArgs`, and `InspectDependentsArgs` in `f8-cli::cli` to accept flexible coordinates (`1..=3` positional tokens).
  - Update `f8-cli::render::table` to render multi-tier column headers for Unicode and Markdown tables.
  - Update `Timeline` table in `crates/f8-cli/templates/lbo` to use `"format": "integer"` on the `year` row.
  - Comprehensive unit and integration test suite across `f8-core` and `f8-cli`.

- **Out-of-scope**:
  - Physical Excel addresses (`Sheet!A1` or `A1`): omitted to keep coordinate resolution focused purely on symbolic model identifiers.
  - Table schema outline mode (`--schema`): deferred to the dedicated JSON schema generation and tooling specification ([`specs/drafts/2026-09-23-json-schema-generation-and-tooling.md`](2026-09-23-json-schema-generation-and-tooling.md)).
  - Interactive cell editing via CLI.
  - Terminal interactive scrolling / TUI pagination.

### What Changes

- `crates/f8-core/src/`:
  - `model/format.rs`: Add `FormatPreset::Integer` (`"integer"`).
  - `model/theme.rs`: Add `integer` number format to `NumberFormatsConfig`.
  - `value/format.rs`: Omit thousands commas for `FormatPreset::Integer` and year range `[1900..=2100]` with `0` decimals.
  - `compiler/compiled/mod.rs` & `compiler/compiled/model.rs`: Add helper to resolve coordinate arguments `(target, row, col)` into `(TableId, RowId, ColId)`.
- `crates/f8-cli/src/`:
  - `cli.rs`:
    - Update `InspectCellArgs`, `InspectPrecedentsArgs`, and `InspectDependentsArgs` to use `target: String`, `row: Option<String>`, `col: Option<String>`.
  - `commands/inspect.rs`:
    - Resolve coordinates through coordinate parser helper before dispatching.
  - `render/table.rs`:
    - Support multi-tier column header rendering across Unicode and Markdown views.
- `crates/f8-cli/templates/lbo/tables/timeline.json`:
  - Update `year` row to `"format": "integer"`.

### Capabilities

- **New capabilities**:
  - `f8 inspect cell 'IncomeStatement["cogs", "y2024e"]'` resolves canonical IR syntax.
  - `f8 inspect precedents 'ReturnsSummary["irr", "val"]'` resolves canonical IR syntax.
  - `f8 inspect dependents 'Assumptions["tax_rate", "val"]'` resolves canonical IR syntax.
  - `format: "integer"` allows model authors to declare unformatted integer rows.
- **Modified capabilities**:
  - `f8 inspect table` renders multi-tier column category headers when declared.
  - Calendar years format cleanly as `2029` instead of `2,029`.

---

## Design

### Goals

1. **Single source of truth**: Coordinate parsing logic is centralized and tested in `f8-core` so all outer tooling shares identical parsing rules.
2. **Backward compatibility**: Existing 3-argument invocation (`f8 inspect cell Table row col`) remains fully supported without breaking existing scripts.
3. **Zero shell quoting friction for positional args**: Users typing in shell can continue using space-separated arguments without quotes, while agents can paste quoted IR strings directly.
4. **Zero production panics**: Strictly propagate errors using `thiserror` and `Result`; zero `.unwrap()` or `.expect()`.
5. **Faithful visual hierarchy**: Multi-tier column headers preserve the complete hierarchical grouping declared in table specifications.

### Invariants

1. **Coordinate equivalence**: Querying a cell via 3 positional arguments or a canonical IR string yields the exact same inspection output.
2. **Format preset symmetry**: `FormatPreset::Integer` serializes to and deserializes from `"integer"`, matching all existing presets across AST and theme.
3. **Header alignment invariant**: In multi-tier header rendering, column headers are bottom-aligned; columns with fewer tiers render blank padding in upper tiers.

### Decisions

- **Decision 1: Dual coordinate support (3 positional tokens OR 1 canonical IR string)**
  - *Rationale*: Positional arguments (`f8 inspect cell IncomeStatement cogs y2024e`) avoid shell quoting friction in interactive terminal sessions. Canonical IR strings (`'IncomeStatement["cogs", "y2024e"]'`) allow autonomous agents and clipboard users to query coordinates directly from error messages and tree outputs without string manipulation.
  - *Alternatives considered*: Positional only (rejected: causes cognitive friction for agents copying coordinates). Canonical IR string only (rejected: requires shell quotes in bash/zsh due to `[` globbing).

- **Decision 2: Support both explicit `FormatPreset::Integer` and year heuristic**
  - *Rationale*: Explicit `format: "integer"` is the robust, declarative single source of truth for AST specifications. However, existing models or expressions like `YEAR(...)` often use `format: "number", decimals: 0`. Adding a fallback heuristic for whole numbers in `[1900..=2100]` ensures immediate clean rendering without breaking existing project files.
  - *Alternatives considered*: Heuristic only (rejected: violates explicit data structure principle; cannot format non-year integers like item counts without commas). Preset only (rejected: leaves existing templates with `2,029` until manually updated).

- **Decision 3: Multi-tier headers stacked per-column in terminal and markdown**
  - *Rationale*: While Excel can merge cells via `CenterAcross`, terminal widths vary and markdown tables (GFM) do not support `colspan`. Rendering each tier row with column-aligned cells provides consistent, readable output across both Unicode terminal box-drawing and Markdown formats.
  - *Alternatives considered*: Omitting upper tiers in markdown (rejected: causes loss of information between CLI output modes).

### Data Contracts

#### Coordinate argument parsing (`crates/f8-core/src/compiler/compiled/mod.rs`)

```rust
/// Parses coordinate arguments from either 3 positional tokens or 1 canonical IR string.
///
/// Supported formats:
/// - 3 tokens: `("IncomeStatement", Some("cogs"), Some("y2024e"))`
/// - Canonical IR string: `("IncomeStatement[\"cogs\", \"y2024e\"]", None, None)`
/// - Canonical IR string (unquoted IDs): `("IncomeStatement[cogs, y2024e]", None, None)`
pub fn parse_coordinate_tokens(
    target: &str,
    row: Option<&str>,
    col: Option<&str>,
) -> Result<(TableId, RowId, ColId), InspectionError>;
```

#### CLI coordinate arguments (`crates/f8-cli/src/cli.rs`)

```rust
#[derive(Debug, Args)]
pub struct InspectCellArgs {
    /// Target coordinate: table ID (when specifying 3 args) or canonical IR reference `Table["row", "col"]`
    pub target: String,

    /// Row identifier (when using 3 positional arguments: <table row col>)
    pub row: Option<String>,

    /// Column identifier (when using 3 positional arguments: <table row col>)
    pub col: Option<String>,
}
```

### Risks & Trade-offs

- **Risk**: Multi-tier header rows increase the vertical height of terminal tables.
  - *Mitigation*: Only tables that actually define multiple tiers incur extra header rows; single-tier tables retain their exact single-row appearance.
- **Risk**: Incomplete coordinate arguments (e.g. providing `target` and `row` but omitting `col`).
  - *Mitigation*: The parser explicitly detects partial arguments and returns an error explaining the expected format: either 3 positional tokens (`<table row col>`) or 1 canonical IR string (`Table["row", "col"]`).

---

### Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Flexible coordinate via 3 positional arguments**
  - **Given** a compiled model
  - **When** executing `f8 inspect cell IncomeStatement cogs y2024e`
  - **Then** resolves to `IncomeStatement["cogs", "y2024e"]` and displays the cell audit card with exit code `0`.

- [ ] **Scenario 2: Flexible coordinate via canonical IR string**
  - **Given** a compiled model
  - **When** executing `f8 inspect cell 'IncomeStatement["cogs", "y2024e"]'` or `f8 inspect precedents 'ReturnsSummary["irr", "val"]'`
  - **Then** parses the IR string, resolves to `CellId`, and outputs inspection results with exit code `0`.

- [ ] **Scenario 3: Flexible coordinate with unquoted IDs in IR string**
  - **Given** a compiled model
  - **When** executing `f8 inspect cell 'IncomeStatement[cogs, y2024e]'`
  - **Then** parses the IR string, trims whitespace, resolves to `CellId`, and displays the cell audit card with exit code `0`.

- [ ] **Scenario 4: Invalid coordinate arguments rejection**
  - **Given** a compiled model
  - **When** executing `f8 inspect cell IncomeStatement cogs` (missing 3rd argument without brackets)
  - **Then** fails with exit code `1` and a message explaining coordinates must be 3 arguments (`<table row col>`) or 1 IR string (`Table["row", "col"]`).

- [ ] **Scenario 5: Calendar year formatting without thousands commas**
  - **Given** a cell evaluating to `2029.0` with `format: "integer"` or `format: "number"` with `decimals: 0`
  - **When** formatted via `format_cell_value` or rendered in `f8 inspect table`
  - **Then** renders as `2029` (not `2,029`).

- [ ] **Scenario 6: Multi-tier column header rendering**
  - **Given** the `Timeline` table in the `lbo` template with 3 header tiers
  - **When** executing `f8 inspect table Timeline`
  - **Then** renders 3 distinct header rows showing `"Project Titan LBO"` across columns, intermediate category headers, and bottom-tier column headers with inline IDs.

---

## Tasks

### Phase 1: Core AST & Coordinate Parser
- [ ] Add `FormatPreset::Integer` to `crates/f8-core/src/model/format.rs`.
- [ ] Update `NumberFormatsConfig` in `crates/f8-core/src/model/theme.rs` to support `integer`.
- [ ] Update `crates/f8-core/src/value/format.rs` to format `Integer` and year values `[1900..=2100]` without commas.
- [ ] Implement `parse_coordinate_tokens` in `crates/f8-core/src/compiler/compiled/mod.rs`.
- [ ] Add unit tests in `crates/f8-core` for coordinate parsing and year formatting.

### Phase 2: CLI Multi-tier Column Headers
- [ ] Update `extract_render_columns` and header row rendering in `crates/f8-cli/src/render/table.rs` for Unicode and Markdown.
- [ ] Verify single-tier tables retain unchanged formatting.
- [ ] Add regression test asserting multi-tier header rendering for `Timeline` in `lbo`.

### Phase 3: CLI Coordinate Integration & Template Updates
- [ ] Update `InspectCellArgs`, `InspectPrecedentsArgs`, and `InspectDependentsArgs` in `crates/f8-cli/src/cli.rs` for flexible arguments (`target`, `row`, `col`).
- [ ] Update `crates/f8-cli/src/commands/inspect.rs` to invoke `parse_coordinate_tokens`.
- [ ] Update `crates/f8-cli/templates/lbo/tables/timeline.json` to use `"format": "integer"` on `year` row.
- [ ] Add CLI integration tests in `crates/f8-cli/tests/inspect_test.rs` covering 3-arg and IR string invocations.

### Phase 4: Verification Suite & Documentation
- [ ] Run full verification suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- [ ] Update living specs `specs/living-specs/interfaces/cli.md`, `specs/living-specs/compiler/compiled-model.md`, and `specs/living-specs/model/spec-ast.md`.
- [ ] Archive draft spec to `specs/history/`.
