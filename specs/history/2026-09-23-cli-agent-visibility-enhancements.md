---
title: CLI agent visibility and inspection enhancements
date: 2026-09-23
status: reconciled
target-living-spec:
  - specs/living-specs/compiler/compiled-model.md
  - specs/living-specs/interfaces/cli.md
---

## Proposal

### What

Enhance CLI model inspection interfaces to be agent-first by exposing programmatic coordinates and evaluated runtime values directly in primary audit views:
1. **Dual ID and title rendering in table views**: Update `f8 inspect table` to display both presentation titles and programmatic identifiers (`[id]`) simultaneously (e.g. `Cost of Goods Sold [cogs]`, `2024E [y2024e]`), deduplicating if the label and ID are identical.
2. **Dual table title and ID rendering in model summary**: Update `f8 inspect` overview summary to display human-readable table titles alongside table IDs (e.g. `- Income Statement [IncomeStatement] [12 rows x 5 cols] (40 cells)`).
3. **Evaluated values in calculation trees**: Annotate formula and driver nodes in `f8 inspect precedents` and `f8 inspect dependents` with their evaluated, formatted runtime values (e.g. `├── Operations["revenue", "y5"] = $ 182.4`).
4. **Evaluated values in cell audit cards**: Display evaluated runtime values for direct precedents and dependents in `f8 inspect cell`.

### Why

- **Eliminate ID discovery round-trips**: Autonomous AI agents query `f8 inspect table` to understand financial models before modifying specifications or querying cells. Currently, tables only display presentation labels (e.g. `Cost of Goods Sold` and `2024E`), leaving the agent to guess the underlying IDs (`cogs` vs `cost_of_goods_sold`, `y2024e` vs `2024e`). When agents guess incorrectly, subsequent `f8 inspect cell` calls fail.
- **Instant formula verification**: Dependency trees and cell inspection cards display graph topology (`├── A -> B`) without evaluated numeric values. Auditors and AI agents cannot verify whether calculations produced correct numbers without issuing separate queries for every single upstream cell.
- **Semantic table orientation in model summary**: The high-level model overview displays raw table IDs (`IncomeStatement`) without human titles (`Income Statement`), hindering fast cognitive mapping on large multi-table models.

### Scope

- **In-scope**:
  - Relocate `format_cell_value` and `format_accounting_number` into `f8-core::value::format` as the central single source of truth for value typography.
  - Extend `DependencyNode` in `f8-core` with `pub formatted_value: Option<String>`.
  - Extend `CellInspection` in `f8-core` with `pub formatted_value: Option<String>` for the primary cell.
  - Define `CellNeighborInspection` in `f8-core` and update `CellInspection::precedents` and `CellInspection::dependents`.
  - Update `f8 inspect table` in `f8-cli` to render inline `Title [id]` and `Header [id]` across Unicode and Markdown outputs, deduplicating when label and ID match.
  - Update `f8 inspect` summary in `f8-cli` (`render_summary.rs`) to display `Title [id]`.
  - Update `render_node_list` in `f8-cli` to append `= <formatted_value>` to calculation tree nodes when non-empty.
  - Update `render_cell` in `f8-cli` to append `= <formatted_value>` to precedent and dependent list items when non-empty.
  - Unit and integration tests covering dual table rendering, summary table titles, and tree/card value annotations.

- **Out-of-scope**:
  - Modifier flags like `--ids-only` / `--labels-only`: omitted to avoid premature complexity. Machine-readable consumption is already fully served by `--format json`, while terminal inspection is intended for human and agent auditing.
  - Phase 3 ergonomics: flexible coordinate string parser (`Model!D20` / canonical string syntax), integer year formatting heuristics, multi-tier header stacking, table schema subcommand ([`docs/interfaces/cli-audit.md`](../../docs/interfaces/cli-audit.md) §4 Phase 3).

### What Changes

- `crates/f8-core/src/value/mod.rs` & `crates/f8-core/src/value/format.rs`:
  - Define `format_cell_value` and `format_accounting_number` in `f8_core::value::format`.
  - Re-export `format_cell_value` and `format_accounting_number` from `f8_core::value`.
- `crates/f8-core/src/compiler/compiled/model.rs`:
  - Add `#[serde(default)] pub formatted_value: Option<String>` to `CellInspection`.
  - Add `#[serde(default)] pub formatted_value: Option<String>` to `DependencyNode`.
  - Add `pub struct CellNeighborInspection` containing `cell_id: CellId`, `sheet_address: String`, `value: Option<CellValue>`, and `formatted_value: Option<String>`.
  - Update `CellInspection::precedents` and `CellInspection::dependents` to use `Vec<CellNeighborInspection>`.
- `crates/f8-core/src/compiler/compiled/mod.rs`:
  - Update `get_node_display_metadata` to format and return `(sheet_address, value, formatted_value, excel_formula)`.
  - Update `inspect_cell` to format the target cell's `formatted_value` and populate `CellNeighborInspection` for precedents and dependents.
  - Update `precedents_of` and `dependents_of` (both non-recursive and recursive paths) to populate `formatted_value` on `DependencyNode`.
- `crates/f8-cli/src/render/table.rs`:
  - Remove local formatting implementation and import `format_cell_value` from `f8_core::value`.
  - Format headers as `<Header> [<id>]` (or `<Header>` if `header.eq_ignore_ascii_case(id)`).
  - Format descriptor cells as `<Indented Title> [<id>]` (or `<Indented Title>` if `title.eq_ignore_ascii_case(id)`).
- `crates/f8-cli/src/render/summary.rs`:
  - Format tables as `- <Title> [<id>] [R rows x C cols] (N cells)` (or `- <id>` if title equals id or is empty).
- `crates/f8-cli/src/render/tree.rs`:
  - Render `= <formatted_value>` between cell coordinate and `(already visited)` marker when value is non-empty.
- `crates/f8-cli/src/render/cell.rs`:
  - Render `= <formatted_value>` next to precedent and dependent items when value is non-empty.

### Capabilities

- **New capabilities**:
  - `f8 inspect table <Table>` displays both programmatic IDs and presentation titles out of the box in terminal box-drawing and Markdown outputs.
  - `f8 inspect` summary displays human-readable table titles alongside IDs.
  - `f8 inspect precedents <cell>` and `f8 inspect dependents <cell>` display evaluated numeric values on every calculation node.
  - `f8 inspect cell <cell>` displays evaluated values of all immediate inputs and outputs, and includes pre-formatted value strings in `--json` output.
- **Modified capabilities**:
  - `DependencyNode` and `CellInspection` JSON payloads include `formatted_value` metadata.
  - `CellInspection::precedents` and `CellInspection::dependents` serialize as detailed neighbor records.

---

## Design

### Goals

- **Zero-guesswork agent navigation**: Agents discover coordinates and labels simultaneously on their first inspection command without needing discovery flags.
- **Immediate numeric verification**: Formula trees and cell cards provide immediate visual proof of calculation correctness.
- **Pragmatic simplicity**: Reject superfluous flags (`--ids-only`, `--labels-only`). Terminal inspection always displays complete audit context, while `--format json` serves raw programmatic needs.

### Invariants

- **Single source of truth for value formatting**: All cell value formatting across `f8-core` and `f8-cli` delegates to `f8_core::value::format_cell_value`.
- **Deduplication invariant**: When display title/header and programmatic ID are identical (case-insensitively, e.g. `col.headers = ["val"]`, `col.id = "val"`), omit the bracketed ID to avoid redundant `val [val]`.
- **Safe tree value formatting**: When a cell's evaluated value is empty or formatting yields an empty string, omit the `= <value>` suffix to avoid dangling `=` characters.
- **Aesthetic alignment**: Box-drawing and Markdown table columns dynamically re-calculate column widths to maintain exact typographic alignment when bracketed IDs are added.
- **Zero production panics**: Safe option handling across all formatting paths.

### Decisions

- **Decision 1: Inline `Title [id]` vs dedicated `[ID]` column**:
  - **Choice**: Inline format `Title [id]` for row descriptors and `Header [id]` for column headers.
  - **Rationale**: A dedicated ID column fragments visual hierarchy on tables with full-width section headers (e.g. `Operating Expenses`), adds blank gutters, and expands horizontal terminal width significantly. Inline brackets maintain existing indentation tiers (`  Cost of Goods Sold [cogs]`).
  - **Alternatives considered**:
    - *Dedicated column*: Rejected due to awkward handling of section headers and horizontal overflow.

- **Decision 2: Universal dual display vs modifier flags**:
  - **Choice**: Always render dual `Title [id]` in terminal inspection; do not add `--ids-only` or `--labels-only` flags.
  - **Rationale**: Pragmatic simplicity (`AGENTS.md`). Terminal inspection exists for human and agent auditing where seeing both semantic meaning and coordinates is universally advantageous. Pure programmatic tools requiring raw IDs or clean data already consume `--format json`.
  - **Alternatives considered**:
    - *Opt-in `--show-ids`*: Rejected because agents fail on default output before discovering the flag.
    - *Opt-out `--ids-only` / `--labels-only`*: Rejected as premature complexity without demonstrated user demand.

- **Decision 3: Move value formatting to `f8-core::value::format`**:
  - **Choice**: Relocate `format_cell_value` and `format_accounting_number` from `f8-cli` into `f8-core::value::format`.
  - **Rationale**: Enforces single source of truth. `CompiledModel` inspection queries run inside `f8-core` and must generate `formatted_value` without duplicating logic or taking an illegal circular dependency on `f8-cli`.

- **Decision 4: Format parity on `CellInspection`**:
  - **Choice**: Add `formatted_value: Option<String>` to `CellInspection` in addition to `DependencyNode` and `CellNeighborInspection`.
  - **Rationale**: Consistency across data contracts. An agent querying `f8 inspect cell --json` receives pre-formatted strings for the inspected cell and all its neighbor cells uniformly.

- **Decision 5: Dual table title display in model summary**:
  - **Choice**: Display `- Title [id] [R rows x C cols] (N cells)` in `render_summary.rs` (deduplicating if title matches ID).
  - **Rationale**: Completes the P1 visibility initiative by eliminating the table ID disconnect identified in `cli-audit.md` §2.1.

### Data Contracts

#### Extended `CellInspection`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellInspection {
    pub coordinate: CellId,
    pub sheet: SheetId,
    pub physical_address: String,
    pub evaluated_value: CellValue,
    /// Pre-formatted runtime value string (e.g. `"$ 115.0"`, `"15.0%"`).
    #[serde(default)]
    pub formatted_value: Option<String>,
    pub format: Option<FormatPreset>,
    pub decimals: Option<u32>,
    pub currency: Option<CurrencyCode>,
    pub is_highlight: bool,
    pub indent: u8,
    pub is_formula: bool,
    pub formula_expr: Option<String>,
    pub excel_formula: Option<String>,
    pub note: Option<String>,
    pub precedents: Vec<CellNeighborInspection>,
    pub dependents: Vec<CellNeighborInspection>,
}
```

#### Extended `DependencyNode`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyNode {
    pub cell_id: CellId,
    pub sheet_address: String,
    pub value: Option<CellValue>,
    /// Pre-formatted runtime value string (e.g. `"$ 182.4"`, `"15.0%"`).
    #[serde(default)]
    pub formatted_value: Option<String>,
    pub excel_formula: Option<String>,
    #[serde(default)]
    pub is_pruned: bool,
    pub children: Vec<DependencyNode>,
}
```

#### Extended `CellNeighborInspection`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellNeighborInspection {
    pub cell_id: CellId,
    pub sheet_address: String,
    pub value: Option<CellValue>,
    pub formatted_value: Option<String>,
}
```

### Risks & Trade-offs

- **Table horizontal width**: Inline `Title [id]` and `Header [id]` widens columns.
  - *Mitigation*: Column widths already dynamically expand based on content width; table layout remains compact for typical accounting models.
- **Breaking change on `CellInspection`**: Updating `precedents` and `dependents` from `Vec<CellId>` to `Vec<CellNeighborInspection>`.
  - *Mitigation*: Greenfield codebase with no external consumers; all internal callers and unit tests updated in-place.

### Acceptance Criteria

- [x] **Scenario 1: Default table inspection renders both IDs and titles**
  - **Given** a compiled model with table `IncomeStatement`
  - **When** executing `f8 inspect table IncomeStatement`
  - **Then** columns render `2024E [y2024e]` and rows render `Cost of Goods Sold [cogs]` with indentation preserved.

- [x] **Scenario 2: Deduplication when title and ID match**
  - **Given** a table where column header is `Value` and col ID is `val` (different), but another column has header `val` and ID `val` (identical)
  - **When** executing `f8 inspect table <Table>`
  - **Then** the first column renders `Value [val]` and the second renders `val` without duplicate brackets.

- [x] **Scenario 3: Model overview summary renders table titles alongside IDs**
  - **Given** a compiled model with table `IncomeStatement` having title `"Income Statement"`
  - **When** executing `f8 inspect`
  - **Then** table line renders `- Income Statement [IncomeStatement] [12 rows x 5 cols] (40 cells)`.

- [x] **Scenario 4: Dependency tree displays evaluated values**
  - **Given** a compiled model
  - **When** executing `f8 inspect precedents <table_id> <row_id> <col_id>`
  - **Then** calculation nodes render coordinates followed by `= <formatted_value>`.

- [x] **Scenario 5: Pruned tree nodes retain evaluated value**
  - **Given** a reconvergent calculation graph
  - **When** traversing a node marked pruned
  - **Then** output renders `<CellId> = <formatted_value> (already visited)`.

- [x] **Scenario 6: Cell audit card displays precedent and dependent values**
  - **Given** a cell with precedents and dependents
  - **When** executing `f8 inspect cell <table_id> <row_id> <col_id>`
  - **Then** precedent and dependent entries render `├── <CellId> = <formatted_value>`.

- [x] **Scenario 7: JSON inspection payload parity**
  - **Given** a compiled model
  - **When** executing `f8 inspect cell <table_id> <row_id> <col_id> --json`
  - **Then** the JSON payload contains `"formatted_value"` for the cell itself, and inside each item of `"precedents"` and `"dependents"`.

---

## Tasks

### Phase 1: Core Value Formatting & Single Source of Truth (`f8-core`)

- [x] Create `crates/f8-core/src/value/format.rs` migrating `format_cell_value` and `format_accounting_number` from `f8-cli`.
- [x] Re-export formatting functions in `crates/f8-core/src/value/mod.rs` and `crates/f8-core/src/lib.rs`.
- [x] Add unit tests in `crates/f8-core/tests/` verifying value formatting for currency, percentages, multiples, basis points, negatives, and precision overrides.

### Phase 2: Core Model Contracts & Inspection Facade (`f8-core`)

- [x] Add `formatted_value: Option<String>` to `CellInspection` in [`crates/f8-core/src/compiler/compiled/model.rs`](../../crates/f8-core/src/compiler/compiled/model.rs).
- [x] Add `formatted_value: Option<String>` to `DependencyNode` in [`crates/f8-core/src/compiler/compiled/model.rs`](../../crates/f8-core/src/compiler/compiled/model.rs).
- [x] Define `CellNeighborInspection` in [`crates/f8-core/src/compiler/compiled/model.rs`](../../crates/f8-core/src/compiler/compiled/model.rs).
- [x] Update `CellInspection::precedents` and `CellInspection::dependents` to `Vec<CellNeighborInspection>`.
- [x] Update `get_node_display_metadata` in [`crates/f8-core/src/compiler/compiled/mod.rs`](../../crates/f8-core/src/compiler/compiled/mod.rs) to return `(sheet_address, value, formatted_value, excel_formula)`.
- [x] Update `inspect_cell` in [`crates/f8-core/src/compiler/compiled/mod.rs`](../../crates/f8-core/src/compiler/compiled/mod.rs) to populate `formatted_value` on the target cell, and build `CellNeighborInspection` lists for precedents and dependents.
- [x] Update `precedents_of` and `dependents_of` (both non-recursive and recursive traversal paths) to pass `formatted_value` to `DependencyNode`.
- [x] Update existing unit and integration tests in `crates/f8-core/tests/compiled_model_test.rs` to match the updated contract signatures.

### Phase 3: Dependency Tree & Cell Value Rendering (`f8-cli`)

- [x] Update `render_node_list` in [`crates/f8-cli/src/render/tree.rs`](../../crates/f8-cli/src/render/tree.rs) to format `= <formatted_value>` on tree nodes when non-empty.
- [x] Update `render_cell` in [`crates/f8-cli/src/render/cell.rs`](../../crates/f8-cli/src/render/cell.rs) to format `= <formatted_value>` for precedents and dependents when non-empty.
- [x] Verify that pruned nodes render `<CellId> = <formatted_value> (already visited)`.

### Phase 4: Table & Summary Dual Rendering (`f8-cli`)

- [x] Update [`crates/f8-cli/src/render/table.rs`](../../crates/f8-cli/src/render/table.rs) to import `format_cell_value` from `f8_core::value`.
- [x] Update `extract_render_columns` and `get_cell_text` in [`crates/f8-cli/src/render/table.rs`](../../crates/f8-cli/src/render/table.rs) to render `Header [id]` and `Title [id]` across Unicode and Markdown outputs, deduplicating when label and ID match case-insensitively.
- [x] Update `render_summary` in [`crates/f8-cli/src/render/summary.rs`](../../crates/f8-cli/src/render/summary.rs) to render `- Title [id] [R rows x C cols] (N cells)`, deduplicating when title and ID match.

### Phase 5: Verification Suite

- [x] Update existing CLI tests in `crates/f8-cli/tests/inspect_test.rs` to assert:
  - Table inspection displays `[id]` alongside titles and headers.
  - Summary overview displays `Title [id]`.
  - Precedent/dependent trees display `= <value>`.
  - Cell card displays precedent/dependent values and JSON payload contains `formatted_value`.
- [x] Run full verification suite:
  - `cargo check`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`
