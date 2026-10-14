---
title: Row Group Indentation, Section Gutters, & Opt-In Table Collapsing
date: 2026-09-21
status: completed
target-living-spec:
  - specs/living-specs/model/spec-ast.md
  - specs/living-specs/compiler/spatial-layout.md
  - specs/living-specs/compiler/workbook-emission.md
---

## Proposal

### What

1. **Descriptor column indentation**: Automatically compute and apply OpenXML cell indentation (`rust_xlsxwriter::Format::set_indent(u8)`) to primary descriptor columns and section headers based on row group hierarchy nesting depth. Numeric measure columns remain unindented and right-aligned.
2. **Top-level section gutters**: Insert an intentional blank gutter row (`theme.metrics.gutter_row_height`, 9.0 pt) immediately above each top-level section header (`depth == 1`) whenever it is preceded by prior data or section rows (`current_row > data_start_row`). First-position groups and nested sub-groups (`depth > 1`) do not receive a preceding gutter.
3. **Opt-in table-level collapsing**: Add `collapsible: bool` (defaulting to `false`) to `TableSpec`. Eliminate automatic row-group outline grouping (`worksheet.group_rows`) in favor of clean typographic hierarchy, while allowing entire tables with `collapsible: true` to collapse under their title banner using `worksheet.group_symbols_above(true)`.

### Why

- **Indentation bug/gap**: Living specs and DSL documentation specify that descriptor line items indent according to row group tree depth, and layout column arbitration already pads column widths anticipating indentation. However, emission never plumbed `Format::set_indent`, causing all descriptor cells in output Excel files to align at indent 0.
- **Row group visual breathing room**: Accounting tables frequently position un-grouped subtotal rows (e.g. `Total Current Assets` or `Gross Profit`) after a row group's children. Attempting to place a gutter after a row group's children would erroneously separate line items from their subtotal. Inserting a gutter immediately *above* subsequent top-level section headers reliably partitions sections without breaking accounting formatting or requiring heuristics about trailing rows.
- **Institutional modeling standards & clean margins**: Collapsible outline brackets (`[+]`/`[-]`) on every small row group create visual clutter on the left worksheet margin and, when collapsed, hide the section header itself. Modern financial modeling standards rely on bold headings and indented line items for internal sections, reserving outline collapse for entire secondary schedules and tables.

### Scope

- **In-scope**:
  - Add `#[serde(default)] pub collapsible: bool` to `TableSpec` and `TableSpec::with_collapsible`.
  - Track row group nesting depth (`0` for ungrouped, `1` for top-level group items, `2` for sub-group items, etc.) in `LayoutPlanner`.
  - Pass `indent: u8` in `CellFormatKey` for descriptor columns and invoke `format.set_indent(indent)` in `FormatCache`.
  - Synthesize section header formats with appropriate indentation for nested sub-headers.
  - Insert section gutter rows above top-level section headers (`depth == 1`) when `current_row > data_start_row`.
  - Replace row-group `group_rows` calls with opt-in table-level `group_rows` from `table_placement.start_row + 1` to `table_placement.end_row` with `worksheet.group_symbols_above(true)`.
  - Update and expand test suites across layout, emission, and fixtures.

- **Out-of-scope**:
  - Column group outline collapsing (column groups retain super-header banner presentation).
  - Custom user-specified per-row indent overrides outside row group depth.

### What Changes

- **Affected modules**:
  - `src/model/table.rs`: Add `collapsible: bool` to `TableSpec`.
  - `src/compiler/layout/mod.rs`: Compute row nesting depth in `RowGroupAnalysis`, record descriptor cell indentation, and insert top-level section gutters.
  - `src/compiler/emission/format.rs`: Add `indent: u8` to `CellFormatKey`, apply `set_indent` in `FormatCache::get_cell_format`, and support indent in section headers.
  - `src/compiler/emission/workbook.rs`: Remove row-group level outlining; implement table-level outlining when `table.collapsible == true` with `group_symbols_above(true)`.
  - `tests/spatial_layout_test.rs`: Validate section gutter row insertion and physical coordinate indexing.
  - `tests/workbook_emission_test.rs`: Validate descriptor indentation in OpenXML XML and table-level outline collapsing.
  - `tests/fixtures/`: Update model fixtures if required.

### Capabilities

- **New capabilities**:
  - Tables can specify `"collapsible": true` to permit folding an entire table down to its title banner.
  - Hierarchical financial statements cleanly display indented line items and sub-items in the descriptor column.
  - Top-level sections have distinct visual separation via 9pt gutter rows without manual blank row specifications.
- **Modified capabilities**:
  - Default tables produce clean left margins without row-group outline buttons.

---

## Design

### Goals

1. **Typographic hierarchy**: Line items within groups are indented according to nesting depth, matching FAST standard modeling practices.
2. **Reliable section spacing**: Top-level section headers are separated by gutters without splitting line items from trailing subtotals.
3. **Simplicity and zero regression**: Table collapsing is strictly opt-in (`collapsible: false` by default).

### Invariants

- **Measure immunity**: Numeric/measure columns must never receive indentation; numbers remain right-aligned.
- **Gutter suppression at table start**: If a table's first data row is a section header, no gutter row is inserted above it.
- **Sub-group contiguity**: Nested sub-groups (`depth > 1`) never receive a preceding gutter row.
- **Deterministic layout**: Layout planning and coordinate assignment remain strictly deterministic.

### Decisions

- **Table-level collapse boundary**:
  - *Choice*: Group from `table_placement.start_row + 1` to `table_placement.end_row`.
  - *Rationale*: Keeps the Table Title banner visible when collapsed (`[+] Table Title`), allowing clean executive dashboard views.
  - *Alternatives considered*: Grouping from `col_header_row + 1` (leaves column headers floating without title context) or entire table including title (hides table title entirely when collapsed).

- **Gutter placement above section header**:
  - *Choice*: Insert gutter row at `current_row` immediately before the top-level section header if `current_row > data_start_row`.
  - *Rationale*: Eliminates fragile heuristics about whether preceding sections ended with 0, 1, or multiple subtotals or ratios.
  - *Alternatives considered*: Inserting gutter after group children. Rejected because it splits member line items from their subtotal row.

- **Descriptor indent mapping**:
  - *Choice*:
    - Ungrouped root rows: `indent = 0`.
    - Group children at nesting depth $D$: `indent = D` (e.g. depth 1 = indent 1, depth 2 = indent 2).
    - Section headers: top-level section header has `indent = 0` (bold section title); nested sub-group headers have `indent = D - 1`.
  - *Rationale*: Matches standard financial reporting where major section headers sit flush-left in bold, while line items indent underneath.

### Data Contracts

#### `TableSpec` Schema Extension

```rust
pub struct TableSpec {
    pub id: TableId,
    pub title: String,
    pub subtitle: Option<String>,
    #[serde(default)]
    pub collapsible: bool,
    // ... remaining existing fields
}
```

#### `CellFormatKey` Extension

```rust
pub struct CellFormatKey {
    pub role: StyleRole,
    pub preset: Option<FormatPreset>,
    pub decimals: Option<u32>,
    pub currency: Option<CurrencyCode>,
    pub is_input: bool,
    pub is_descriptor: bool,
    pub indent: u8,
}
```

### Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Descriptor cell indentation reflects group nesting depth**
  - **Given** a table with top-level group `"opex"` containing row `"salaries"` and nested sub-group `"marketing"` containing row `"ads"`
  - **When** the workbook is emitted
  - **Then** the descriptor cell for `"salaries"` has indent level 1, `"ads"` has indent level 2, and ungrouped root rows have indent level 0.

- [ ] **Scenario 2: Measure cells are not indented**
  - **Given** any row inside a nested row group
  - **When** the workbook is emitted
  - **Then** numeric and currency measure cells have indent level 0 and retain right-alignment.

- [ ] **Scenario 3: Section gutter row placed above subsequent top-level section headers**
  - **Given** a table with root rows followed by row group `"opex"`, followed by subtotal `"total_opex"`, followed by row group `"capex"`
  - **When** layout is planned
  - **Then** a 9pt gutter row is inserted immediately before `"opex"` and immediately before `"capex"`, but no gutter is inserted before child rows or subtotals.

- [ ] **Scenario 4: Section gutter suppressed at data start**
  - **Given** a table where the very first data item is row group `"revenue"`
  - **When** layout is planned
  - **Then** no gutter row is inserted between the column header row and the `"revenue"` section header.

- [ ] **Scenario 5: Default table emission omits outline brackets**
  - **Given** a table with row groups and `collapsible: false` (default)
  - **When** the workbook is emitted
  - **Then** the worksheet XML contains no `outlineLevel` attributes and no row grouping.

- [ ] **Scenario 6: Collapsible table folds under title banner**
  - **Given** a table with `collapsible: true` spanning rows $R_{start}..R_{end}$
  - **When** the workbook is emitted
  - **Then** `worksheet.group_rows(R_{start} + 1, R_{end})` is executed with `group_symbols_above(true)`, and worksheet XML contains `outlinePr` with `summaryBelow="0"`.

---

## Tasks

### Phase 1: Model & AST Updates
- [ ] Add `collapsible: bool` to `TableSpec` with `#[serde(default)]` and builder method `with_collapsible`.
- [ ] Verify serialization and deserialization of `TableSpec` with and without `collapsible`.

### Phase 2: Spatial Layout Planning (Indentation & Gutters)
- [ ] Update `analyze_row_groups` to map each `RowId` to its hierarchy depth (`1..=7`).
- [ ] In `LayoutPlanner::plan_layout`, insert top-level section gutter row (`theme.metrics.gutter_row_height`) above groups when `current_row > data_start_row` and `depth == 1`.
- [ ] Associate row descriptor indentation depth with layout physical coordinates or cell format keys.
- [ ] Update layout unit tests in `tests/spatial_layout_test.rs` to verify gutter rows and row coordinate offsets.

### Phase 3: Format Synthesis & Emission (Indentation & Outlines)
- [ ] Add `indent: u8` to `CellFormatKey` in `src/compiler/emission/format.rs`.
- [ ] In `FormatCache::get_cell_format`, apply `format.set_indent(key.indent)` when `key.indent > 0`.
- [ ] In `FormatCache`, synthesize section header formats with indent for sub-headers.
- [ ] In `src/compiler/emission/workbook.rs`, remove row-group `apply_row_group_outlines`.
- [ ] In `src/compiler/emission/workbook.rs`, implement table-level outline grouping when `table.collapsible == true` using `worksheet.group_symbols_above(true)`.
- [ ] Update `tests/workbook_emission_test.rs` and model fixtures to verify indentation, section gutters, and opt-in table outlines.

### Phase 4: Verification Suite & Spec Reconciliation
- [ ] Run `cargo check`.
- [ ] Run `cargo test`.
- [ ] Run `cargo clippy --all-targets -- -D warnings`.
- [ ] Run `cargo fmt --check`.
- [ ] Update living specs (`spatial-layout.md`, `workbook-emission.md`, `spec-ast.md`).
- [ ] Reconcile and archive draft spec to `specs/history/`.
