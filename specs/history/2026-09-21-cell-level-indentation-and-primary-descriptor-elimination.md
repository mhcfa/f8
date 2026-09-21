---
title: Cell-Level Indentation & Primary Descriptor Elimination
date: 2026-09-21
status: completed
target-living-spec:
  - specs/living-specs/model/spec-ast.md
  - specs/living-specs/compiler/cell-ir.md
  - specs/living-specs/compiler/spatial-layout.md
---

## Proposal

### What

Decouple typographic cell indentation from `AxisRole` by eliminating `AxisRole::PrimaryDescriptor` and promoting indentation to a first-class 0D cell attribute on `DetailedCellInput` and `CellNode`.

### Why

- **Eliminate role pollution**: `AxisRole` was designed to delineate calculation vectors (`Measure`) from label metadata (`Descriptor`), governing where vector formulas broadcast and where temporal offsets (`col.prev` / `col.next`) navigate. Introducing `PrimaryDescriptor` conflated fundamental calculation semantics with visual cell presentation.
- **Isolate asymmetric 0D presentation**: Unlike 1D vector formatting properties (`style`, `format`, `highlight`, `decimals`, `currency`) that broadcast across columns or rows, indentation is inherently asymmetric across axes (numbers and secondary descriptor metadata like codes or audit notes should not be indented indiscriminately). Attaching `indent` to `DataRowSpec` broke this model and forced the core to invent an ad-hoc destination column (`PrimaryDescriptor`). Promoting indentation to a pure 0D cell attribute eliminates the need for any routing heuristics.
- **Eliminate architectural backdoors in layout**: Currently, `LayoutPlanner` bypasses `CellGrid` and cross-inspects `data_row.indent` and `data_col.role.is_primary_descriptor()`. With cell-level indentation, `CellNode.indent` is the pure single source of truth expanded by `expand_table`, and `LayoutPlanner` reads it directly from `CellGrid`.
- **Eliminate artificial validation complexity**: Eliminates `ModelError::DuplicatePrimaryDescriptor` and `ModelError::IndentWithoutPrimaryDescriptor`. It also resolves a subtle bug where section headers with indentation previously required a `PrimaryDescriptor` column to pass validation, despite section headers having zero column associations.

### Scope

- **In-scope**:
  - Remove `AxisRole::PrimaryDescriptor` from `AxisRole`, restoring a clean binary enum (`Measure` and `Descriptor`).
  - Add `indent: Option<u8>` to `DetailedCellInput` in `src/model/table.rs`.
  - Add `pub fn indent(&self) -> Option<u8>` getter to `CellInput` in `src/model/table.rs`.
  - Add `impl From<DetailedCellInput> for CellInput` in `src/model/table.rs` for seamless construction ergonomics.
  - Add `indent: u8` with `#[serde(default)]` to `CellNode` in `src/compiler/ir/cell.rs`.
  - Update `expand_table` in `src/compiler/ir/expand.rs` to propagate cell indentation into `CellNode`.
  - Update `LayoutPlanner` in `src/compiler/layout/mod.rs` to read `node.indent` directly from `CellGrid`.
  - Remove `DataRowSpec.indent`, `RowSpec.indent()`, `RowSpec::with_indent()`, `AxisRole::is_primary_descriptor()`, and `ColumnSpec::primary_descriptor()`.
  - Retain `SectionHeaderRowSpec.indent` and `SectionHeaderRowSpec::with_indent()`.
  - Add `ModelError::InvalidIndentOnMeasureCell` and enforce in `TableSpec::validate_cells()` so `indent > 0` cannot be set on cells intersecting a `Measure` column (pre-indexing column roles for $O(1)$ verification).
  - Remove `DuplicatePrimaryDescriptor` and `IndentWithoutPrimaryDescriptor` from `ModelError`.
  - Update test fixtures: remove `build_primary_descriptor_col` in `tests/fixtures/mod.rs` (migrating callers to `build_descriptor_col`), and update `RowBuilder` to track the descriptor column and attach `indent` to the line item's `DetailedCellInput`.
  - Update compiler documentation (`docs/compiler/`) and DSL specifications (`docs/dsl/`).
- **Out-of-scope**:
  - Changes to OpenXML workbook format synthesis (`format_cache.get_cell_format` and `Format::set_indent` remain unchanged).
  - Changes to section header emission geometry.

### What Changes

- `src/model/table.rs`:
  - `AxisRole`: Remove `PrimaryDescriptor` variant.
  - `DetailedCellInput`: Add `pub indent: Option<u8>`.
  - `CellInput`: Add getter method `pub fn indent(&self) -> Option<u8>`.
  - `CellInput`: Add `impl From<DetailedCellInput> for CellInput`.
  - `DataRowSpec`: Remove `pub indent: Option<u8>` and `with_indent()`.
  - `RowSpec`: Remove `indent(&self)` and `with_indent()`, preventing silent no-ops on data rows.
  - `SectionHeaderRowSpec`: Retain `pub indent: Option<u8>` and `pub fn with_indent(mut self, indent: Option<u8>) -> Self`.
  - `ColumnSpec`: Remove `primary_descriptor()` constructor.
  - `TableSpec::validate_columns`: Remove duplicate primary descriptor check.
  - `TableSpec::validate_rows`: Remove indent-without-primary-descriptor check.
  - `TableSpec::validate_cells`: Validate that `indent > 0` is strictly permitted only on descriptor columns, pre-indexing column roles for $O(1)$ checks.
- `src/model/error.rs`:
  - Remove `DuplicatePrimaryDescriptor` and `IndentWithoutPrimaryDescriptor`.
  - Add `InvalidIndentOnMeasureCell`.
- `src/compiler/ir/cell.rs`:
  - `CellNode`: Add `pub indent: u8`, `with_indent(u8)`.
- `src/compiler/ir/expand.rs`:
  - `expand_table`: Pass `cell_input.indent().unwrap_or(0)` into `CellNode.with_indent(...)`.
- `src/compiler/layout/mod.rs`:
  - Read `node.indent` directly from `grid.get(&cell_id)`.
- `tests/fixtures/`:
  - `tests/fixtures/mod.rs`: Remove `build_primary_descriptor_col`, and update `RowBuilder` so `.indent(u8)` updates the line item's `DetailedCellInput`.
  - `tests/fixtures/lbo.rs`, `tests/fixtures/styling_showcase.rs`, `tests/fixtures/three_statement.rs`: Migrate `build_primary_descriptor_col` to `build_descriptor_col`.
- `docs/compiler/` & `docs/dsl/`:
  - Update compiler docs (`ir.md`, `lowering-and-emission.md`) and DSL docs (`grid.md`, `spec-format.md`, `patterns.md`, `formatting.md`, `project-example.md`).

### Capabilities

- **New capabilities**:
  - Any descriptor cell (e.g. secondary descriptor columns or audit notes) can declare typographic indentation explicitly.
  - Tables with zero descriptor columns or multiple descriptor columns have zero ambiguity or artificial cardinality constraints.
- **Modified capabilities**:
  - `AxisRole` accepts only `measure` and `descriptor`.
  - Row-level `indent` on data rows is replaced with cell-level `indent` in `row.cells`.

---

## Design

### Goals

1. **Dead simple core**: `AxisRole` is strictly binary (`Measure` vs `Descriptor`).
2. **Explicit 0D cell data flow**: Cell indentation follows the clean data path (`DetailedCellInput` $\rightarrow$ `CellNode` $\rightarrow$ `PhysicalCoordinate` $\rightarrow$ OpenXML `set_indent`).
3. **Zero layout backdoors**: `LayoutPlanner` never cross-examines row and column specs to infer cell indentation.
4. **Compile-time consistency**: Rejects indentation on measure cells at validation time, preventing silent formatting discrepancies between specification and OpenXML emission.

### Invariants

- Every cell in `CellGrid` has a deterministic `indent: u8` (default: 0).
- Only cells residing on descriptor columns (`col.role.is_descriptor()`) may specify `indent > 0`; measure cells with `indent > 0` are rejected during model validation (`ModelError::InvalidIndentOnMeasureCell`).
- OpenXML emission applies `fmt.set_indent(node.indent)` only when `node.indent > 0` on descriptor columns.
- Section header rows continue to emit `sh.indent` at the table's left margin (`placement.start_col`).

### Decisions

- **Cell-Level `DetailedCellInput.indent` over 1D `DataRowSpec.indent`**:
  - *Choice*: Move `indent` from `DataRowSpec` to `DetailedCellInput`.
  - *Rationale*: Indentation in Excel is a cell-level horizontal alignment attribute. By placing it directly on the cell, we avoid destination-routing heuristics or ad-hoc column roles.
  - *Alternatives considered*: Keeping `DataRowSpec.indent` and defaulting to the first descriptor column (rejected because it introduces implicit positional heuristics into the core).
- **Remove `AxisRole::PrimaryDescriptor`**:
  - *Choice*: Eliminate `PrimaryDescriptor` entirely.
  - *Rationale*: Without row-level indentation routing, there is no semantic difference between a primary descriptor and any other descriptor column.
  - *Alternatives considered*: Keeping `PrimaryDescriptor` as a deprecated synonym (rejected: zero legacy shims principle).
- **Strict Validation for Measure Cell Indentation**:
  - *Choice*: Reject `indent > 0` on measure cells with `ModelError::InvalidIndentOnMeasureCell`.
  - *Rationale*: OpenXML emission right-aligns measure cells and ignores indent. In accordance with Finx compiler principles, invalid visual configurations must be caught at compile time rather than silently ignored. Pre-indexing column roles ensures $O(1)$ validation checks.
- **Preserve `SectionHeaderRowSpec.indent` & Remove `RowSpec::with_indent`**:
  - *Choice*: Keep `pub indent: Option<u8>` and `pub fn with_indent` strictly on `SectionHeaderRowSpec`; remove `with_indent` and `indent()` from `RowSpec` and `DataRowSpec`.
  - *Rationale*: A section header is a 1D full-width row with exactly one text element (`title`), not a 2D matrix row with multiple columns. Its indentation is unambiguous. Removing `with_indent` from `RowSpec` eliminates deceptive no-ops on data rows. `SectionHeaderRowSpec` implements `Into<RowSpec>`, so section headers remain cleanly constructible (`SectionHeaderRowSpec::new("...").with_indent(Some(1))`).
  - *Alternatives considered*: Keeping `RowSpec::with_indent` (rejected: silently ignores data rows, creating a bug-prone no-op trap).
- **Explicit Cell Inputs over Ad-Hoc `CellInput::with_indent`**:
  - *Choice*: Provide only the `indent(&self) -> Option<u8>` getter on `CellInput`; do not add variant-promoting `with_indent` to `CellInput`.
  - *Rationale*: `CellInput` has no other `with_*` mutators (e.g. `with_style`, `with_format`). Detailed cells are explicitly instantiated via `DetailedCellInput`. Avoiding ad-hoc promotion methods preserves compiler core purity and predictability. Higher-level test fixtures (`RowBuilder`) handle test boilerplate.
  - *Alternatives considered*: Adding `CellInput::with_indent` (rejected: ad-hoc variant promotion that violates single-way predictability in the core AST).

### Data Contracts

```rust
// src/model/table.rs
pub enum AxisRole {
    #[default]
    Measure,
    Descriptor,
}

pub struct DetailedCellInput {
    pub value: Option<ValueLiteral>,
    pub expr: Option<String>,
    pub style: Option<SemanticStyle>,
    pub highlight: Option<bool>,
    pub format: Option<FormatPreset>,
    pub decimals: Option<u32>,
    pub currency: Option<CurrencyCode>,
    pub indent: Option<u8>, // New 0D cell attribute
    pub note: Option<String>,
    pub source: Option<serde_json::Value>,
}

impl CellInput {
    pub fn indent(&self) -> Option<u8> {
        match self {
            Self::Literal(_) => None,
            Self::Detailed(d) => d.indent,
        }
    }
}

impl From<DetailedCellInput> for CellInput {
    fn from(d: DetailedCellInput) -> Self {
        Self::Detailed(d)
    }
}

pub struct SectionHeaderRowSpec {
    pub title: String,
    pub indent: Option<u8>,
    pub note: Option<String>,
}

impl SectionHeaderRowSpec {
    pub fn with_indent(mut self, indent: Option<u8>) -> Self {
        self.indent = indent;
        self
    }
}

// src/model/error.rs
pub enum ModelError {
    // ...
    #[error("Table '{table}': cell at row '{row}', column '{column}' specifies indent ({indent}), but column is a measure axis")]
    InvalidIndentOnMeasureCell {
        table: TableId,
        row: RowId,
        column: ColId,
        indent: u8,
    },
}

// src/compiler/ir/cell.rs
pub struct CellNode {
    pub id: CellId,
    pub sheet: SheetId,
    pub format: Option<FormatPreset>,
    pub decimals: Option<u32>,
    pub currency: Option<CurrencyCode>,
    pub horizontal: SemanticStyle,
    pub vertical: SemanticStyle,
    pub is_highlight: bool,
    #[serde(default)]
    pub indent: u8, // New IR node attribute
    pub payload: CellPayload,
    pub note: Option<String>,
    pub source: Option<serde_json::Value>,
}
```

### Risks & Trade-offs

- **JSON verbosity for manual specs**: Declaring `indent` on a cell requires `{ "value": "Label", "indent": 1 }` rather than row-level `"indent": 1`.
  - *Mitigation*: Handled cleanly by fluent test builders (`RowBuilder`), adapters, and DSL pre-processors. The core data model prioritizes predictability and explicitness over premature ergonomics.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Cell-level indentation in IR expansion**
  - **Given** a table with descriptor column `line_item` and a row specifying cell `"line_item": { "value": "Salaries", "indent": 2 }`
  - **When** `expand_table` is executed
  - **Then** the resulting `CellNode` has `payload: Literal("Salaries")` and `indent: 2`.

- [x] **Scenario 2: Unindented cells default to 0**
  - **Given** a cell declared as a simple string literal `"Revenue"` or a measure formula
  - **When** `expand_table` is executed
  - **Then** the resulting `CellNode` has `indent: 0`.

- [x] **Scenario 3: Layout coordinate inherits CellNode indent**
  - **Given** a `CellGrid` containing a node with `indent: 2`
  - **When** `LayoutPlanner::plan` is executed
  - **Then** `PhysicalCoordinate.indent` is 2 for that cell, and 0 for adjacent measure cells.

- [x] **Scenario 4: Rejection of removed PrimaryDescriptor in deserialization**
  - **Given** deserialization or construction of `AxisRole`
  - **Then** only `measure` and `descriptor` are accepted; `primary_descriptor` is rejected as an unknown variant.

- [x] **Scenario 5: Rejection of indent on measure cells**
  - **Given** a table with measure column `amount` and a row specifying cell `"amount": { "value": 100, "indent": 1 }`
  - **When** `table.validate()` is executed
  - **Then** validation fails with `ModelError::InvalidIndentOnMeasureCell`.

- [x] **Scenario 6: Section header indentation preserved**
  - **Given** a `SectionHeaderRowSpec` with `indent: Some(1)`
  - **When** workbook emission executes
  - **Then** the section header format receives `set_indent(1)`.

- [x] **Scenario 7: Independent indentation across multiple descriptor columns**
  - **Given** a table with two descriptor columns (`category` and `item`)
  - **When** a row defines `category` with `indent: 1` and `item` with `indent: 2`
  - **Then** both cells in `CellGrid` and `PhysicalCoordinate` preserve their respective indentation levels independently.

---

## Tasks

### Phase 1: Core Model & Schema Migration
- [x] Add `indent: Option<u8>` to `DetailedCellInput` in `src/model/table.rs`.
- [x] Add `pub fn indent(&self) -> Option<u8>` getter to `CellInput` in `src/model/table.rs`.
- [x] Add `impl From<DetailedCellInput> for CellInput` in `src/model/table.rs`.
- [x] Remove `AxisRole::PrimaryDescriptor` and `AxisRole::is_primary_descriptor()` from `src/model/table.rs`.
- [x] Remove `DataRowSpec.indent` and `DataRowSpec::with_indent()` from `src/model/table.rs`.
- [x] Remove `RowSpec::indent()` and `RowSpec::with_indent()` from `src/model/table.rs`.
- [x] Retain `SectionHeaderRowSpec.indent` and `SectionHeaderRowSpec::with_indent()` in `src/model/table.rs`.
- [x] Remove `ColumnSpec::primary_descriptor` from `src/model/table.rs`.
- [x] Add `ModelError::InvalidIndentOnMeasureCell` to `src/model/error.rs`.
- [x] Remove `DuplicatePrimaryDescriptor` and `IndentWithoutPrimaryDescriptor` from `src/model/error.rs`.
- [x] Update `TableSpec::validate_columns` and `TableSpec::validate_rows` to remove primary descriptor checks.
- [x] Update `TableSpec::validate_cells` to reject `indent > 0` on cells intersecting measure columns (`ModelError::InvalidIndentOnMeasureCell`), pre-indexing column roles for $O(1)$ checks.

### Phase 2: Compiler IR & Layout Pipeline
- [x] Add `pub indent: u8` (with `#[serde(default)]`) and `with_indent(u8)` to `CellNode` in `src/compiler/ir/cell.rs`.
- [x] Update `expand_table` in `src/compiler/ir/expand.rs` to propagate `cell_input.indent().unwrap_or(0)` to `CellNode`.
- [x] Update `LayoutPlanner` in `src/compiler/layout/mod.rs` to read `node.indent` from `CellGrid` instead of inspecting row/column specs.

### Phase 3: Fixtures, Tests & Documentation
- [x] Remove `build_primary_descriptor_col` in `tests/fixtures/mod.rs`.
- [x] Migrate `build_primary_descriptor_col` calls to `build_descriptor_col` in `tests/fixtures/lbo.rs`, `tests/fixtures/styling_showcase.rs`, and `tests/fixtures/three_statement.rs`.
- [x] Update `RowBuilder` in `tests/fixtures/mod.rs` to track the descriptor column (`desc_col`), promote `Literal` cells to `DetailedCellInput`, and attach `.indent(u8)`.
- [x] Migrate `RowSpec::section_header(...).with_indent(...)` to `SectionHeaderRowSpec::new(...).with_indent(...).into()` in `tests/fixtures/styling_showcase.rs`, `tests/spatial_layout_test.rs`, `tests/spec_test.rs`, and `tests/workbook_emission_test.rs`.
- [x] Update tests in `tests/spec_test.rs`, `tests/spatial_layout_test.rs`, and `tests/workbook_emission_test.rs` to assert cell-level indentation.
- [x] Update compiler documentation: `docs/compiler/ir.md` and `docs/compiler/lowering-and-emission.md`.
- [x] Update DSL documentation: `docs/dsl/grid.md`, `docs/dsl/spec-format.md`, `docs/dsl/patterns.md`, `docs/dsl/formatting.md`, and `docs/dsl/project-example.md`.
- [x] Run full verification suite (`cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`).
