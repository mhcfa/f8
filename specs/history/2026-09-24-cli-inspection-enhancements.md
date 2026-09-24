---
title: CLI Inspection Enhancements & Sheet Grid Auditing
date: 2026-09-24
status: completed
target-living-spec:
  - specs/living-specs/interfaces/cli.md
  - specs/living-specs/compiler/compiled-model.md
---

## Proposal

### What

Enhance the `f8 inspect` family of commands to provide comprehensive whole-grid auditing for both logical tables and physical worksheets:
1. Extend `f8 inspect table <Table>` view modes to support concrete lowered Excel formulas (`=C7+C8`), high-level DSL expressions, and physical Excel coordinates (`C7`).
2. Add `f8 inspect sheet <Sheet>` to composite and render the unified 2D physical grid of worksheets containing multiple stacked or adjacent tables.
3. Explicitly surface programmatic IDs (`sheet_id`, `table_id`, `row_id`, `col_id`) alongside human-readable labels across terminal tables, Markdown grids, and structured JSON payloads.

### Why

During real-world financial due diligence modeling, autonomous agents and analysts need to audit formula lowering across dozens of dynamic rows simultaneously. Currently:
- `f8 inspect cell` operates strictly on a single 0D coordinate, requiring prohibitive round-trips for batch auditing.
- `f8 inspect table` renders only evaluated numerical values or high-level AST expressions, omitting concrete lowered Excel formulas and A1 grid coordinates.
- Worksheets with multiple stacked tables (such as reconciliation bridges and multi-entity schedules) cannot be viewed in unified physical layout space.

Exposing what the compiler already holds in memory resolves these gaps without requiring external `.xlsx` unzipping scripts.

### Scope

- **In-scope**:
  - `ViewMode` enum expansion: `values`, `formulas`, `expr`, `addresses`, `both`.
  - Upgrading `render_table` to output concrete lowered Excel formulas (`cell.excel_formula`) and physical coordinates (`cell.physical_a1`).
  - Core `SheetInspection`, `SheetRowInspection`, and `SheetCellInspection` data structures in `f8-core`.
  - `CompiledModel::inspect_sheet` implementation compositing titles, subtitles, column headers, section headers, gutters, and data cells across the sheet bounding box.
  - Adding `f8 inspect sheet <Sheet>` subcommand to `f8-cli` with `--view` and `--format` options.
  - Inclusion of programmatic identifiers (`table_id`, `row_id`, `col_id`) in visual headers and JSON records.
- **Out-of-scope**:
  - OpenXML package decompression or zip-level binary parsing inside Rust CLI.
  - Altering compiler Pass 1–6a core data structures or layout scheduling.
  - Python / TypeScript FFI bindings (addressed in subsequent SDK specs).

### What Changes

- `crates/f8-core/src/compiler/compiled/model.rs`: Add `SheetInspection`, `SheetRowInspection`, `SheetCellInspection`, and `SheetCellKind`.
- `crates/f8-core/src/compiler/compiled/mod.rs`: Implement `CompiledModel::inspect_sheet(&self, sheet_id: &SheetId) -> Result<SheetInspection, InspectionError>`.
- `crates/f8-cli/src/render/table.rs`: Expand `ViewMode` enum (`Values`, `Formulas`, `Expr`, `Addresses`, `Both`) and update `get_cell_text`.
- `crates/f8-cli/src/cli.rs`: Add `InspectSheetArgs` and register `InspectSubcommand::Sheet`.
- `crates/f8-cli/src/render/sheet.rs`: Implement multi-table sheet renderer supporting Markdown, Unicode box-drawing, and JSON.
- `crates/f8-cli/src/render/mod.rs`: Re-export `render_sheet`.
- `crates/f8-cli/src/commands/inspect.rs`: Wire `InspectSubcommand::Sheet` dispatcher.
- `crates/f8-core/tests/compiled_model_test.rs`: Add unit tests for sheet inspection.
- `crates/f8-cli/tests/inspect_test.rs`: Add integration tests for CLI sheet inspection and table view modes.

### Capabilities

- **New capabilities**:
  - Inspect multi-table physical sheet grids via `f8 inspect sheet <name>`.
  - Batch audit lowered Excel formulas across entire tables via `f8 inspect table <id> --view formulas`.
  - Inspect physical cell coordinates across entire tables via `f8 inspect table <id> --view addresses`.
  - Retrieve full bidirectional coordinate mapping (`[sheet, a1] <-> [table, row, col]`) in machine-readable JSON.
- **Modified capabilities**:
  - `f8 inspect table --view formulas` renders concrete lowered Excel formulas (e.g. `=C7+C8`) instead of internal DSL strings.
  - `f8 inspect table --view expr` added to view original model DSL expressions.

---

## Design

### Goals

1. **Zero compiler overhead**: Leverage existing spatial layout contracts and lowered formula maps already produced by Passes 5 and 6a.
2. **Unified physical sheet inspection**: Seamlessly render sheets hosting multiple stacked or spaced tables.
3. **Explicit programmatic identity**: Display programmatic identifiers alongside display labels to enable direct copy-pasting for CLI commands and edits.
4. **Clean presentation parity**: Maintain identical `--format <markdown|table|json>` output capabilities across both table and sheet commands.

### Invariants

- **Single source of truth**: `SheetInspection` is derived directly from `CompiledModel` fields (`layout_map`, `tables`, `lowered_map`, `evaluated`, `grid`).
- **Zero production panics**: Return structured `InspectionError::SheetNotFound` when an invalid sheet is requested.
- **Deterministic ordering**: Rows and columns are iterated in ascending numerical order (`0..=max_row`, `0..=max_col`).

### Decisions

- **Decision: Concrete Excel formulas for `--view formulas`**:
  - *Rationale*: When inspecting formulas in spreadsheets, users expect the final lowered formula that Excel executes (`=C7+C8`), not internal compiler AST representations. DSL expressions remain available under `--view expr`.
  - *Alternatives considered*: Keeping `--view formulas` as DSL and adding `--view excel` was rejected to avoid confusing terminology and align with spreadsheet conventions.
- **Decision: Unified 2D bounding grid for `f8 inspect sheet`**:
  - *Rationale*: A sheet is fundamentally a single 2D grid of rows and columns. Composing table title banners, column headers, and data cells into a unified row-by-row structure mirrors Excel's layout.
  - *Alternatives considered*: Rendering each table on the sheet sequentially was rejected because it fails to show spatial row alignment, inter-table spacing, and shared column width arbitration.
- **Decision: Explicit ID annotations**:
  - *Rationale*: In human displays, appending `[id]` to headers and row labels provides immediate context without cluttering values. In JSON displays, dedicated fields (`table_id`, `row_id`, `col_id`) provide unambiguous keys.

### Data Contracts

#### Core Data Structures (`f8-core`)

```rust
/// Structural classification of a cell within a physical worksheet layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SheetCellKind {
    Empty,
    TableTitle,
    TableSubtitle,
    ColumnHeader,
    SectionHeader,
    Gutter,
    Data,
}

/// Inspection record for a discrete physical cell on a worksheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetCellInspection {
    pub physical_col: u32,
    pub physical_a1: String,
    pub table_id: Option<TableId>,
    pub row_id: Option<RowId>,
    pub col_id: Option<ColId>,
    pub kind: SheetCellKind,
    pub is_descriptor: bool,
    pub value: CellValue,
    pub formatted_value: Option<String>,
    pub formula_expr: Option<String>,
    pub excel_formula: Option<String>,
}

/// Physical row record containing all non-empty or bounded cells across sheet columns.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetRowInspection {
    pub physical_row: u32, // 0-indexed row
    pub excel_row: u32,    // 1-indexed row number
    pub cells: Vec<SheetCellInspection>,
}

/// Comprehensive physical grid inspection for an entire worksheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetInspection {
    pub sheet_id: SheetId,
    pub title: String,
    pub tables: Vec<TableId>,
    pub min_col: u32,
    pub max_col: u32,
    pub min_row: u32,
    pub max_row: u32,
    pub col_widths: IndexMap<u32, f64>,
    pub rows: Vec<SheetRowInspection>,
}
```

#### CLI View Mode Enum (`f8-cli`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    #[default]
    Values,
    Formulas,
    Expr,
    Addresses,
    Both,
}
```

### Risks & Trade-offs

- **Sparse vs Dense Sheet Grid**:
  - *Risk*: A sheet spanning row 2 to row 100 with blank gutters between tables could produce empty rows.
  - *Mitigation*: Blank buffer gutters are collapsed to single blank rows in rendered tables, and non-empty columns are bounded to the sheet's active bounding box `min_col..=max_col`.
- **Wide Tables in Terminal**:
  - *Risk*: Sheets with 10+ columns can wrap awkwardly in standard terminal widths.
  - *Mitigation*: Markdown (`--format markdown`, default) and JSON (`--format json`) wrap cleanly; `--format table` aligns box characters.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Inspect table with lowered Excel formulas**
  - **Given** a compiled model with a formula cell `C7 = A7 + B7`
  - **When** running `f8 inspect table <Table> --view formulas`
  - **Then** the cell renders `=A7+B7` with exit code `0`.

- [x] **Scenario 2: Inspect table with physical addresses**
  - **Given** a compiled table placed at physical origin `B4`
  - **When** running `f8 inspect table <Table> --view addresses`
  - **Then** data cells render their A1 addresses (e.g. `C7`, `D7`) with exit code `0`.

- [x] **Scenario 3: Inspect multi-table sheet grid**
  - **Given** a sheet containing two stacked tables `REV_RECON` and `REV_RECON_ENTITY`
  - **When** running `f8 inspect sheet "Revenue Bridge" --view formulas`
  - **Then** outputs a unified 2D grid displaying both tables, headers, section rows, and lowered formulas with exit code `0`.

- [x] **Scenario 4: Inspect sheet structured JSON**
  - **Given** a compiled model
  - **When** running `f8 inspect sheet <Sheet> --json`
  - **Then** outputs valid JSON containing `sheet_id`, `tables`, bounding coordinates, and cell records with `table_id`, `row_id`, `col_id`, `excel_formula`, and `physical_a1`.

- [x] **Scenario 5: Unknown sheet rejection**
  - **Given** a compiled model without a sheet named `NonExistent`
  - **When** running `f8 inspect sheet NonExistent`
  - **Then** outputs an error to `stderr` and exits with code `2`.

---

## Tasks

### Phase 1: Core Sheet Inspection (`f8-core`)
- [x] Add `SheetCellKind`, `SheetCellInspection`, `SheetRowInspection`, and `SheetInspection` in `crates/f8-core/src/compiler/compiled/model.rs`.
- [x] Add `InspectionError::SheetNotFound(SheetId)` to `crates/f8-core/src/compiler/compiled/mod.rs` (or error enum).
- [x] Implement `CompiledModel::inspect_sheet(&self, sheet_id: &SheetId)` in `crates/f8-core/src/compiler/compiled/mod.rs`.
- [x] Add unit tests for `inspect_sheet` in `crates/f8-core/tests/compiled_model_test.rs`.

### Phase 2: Table View Mode Enhancements (`f8-cli`)
- [x] Update `ViewMode` in `crates/f8-cli/src/render/table.rs` to include `Expr` and `Addresses`.
- [x] Update `get_cell_text` to display `cell.excel_formula` for `Formulas` and `Both`, `cell.formula_expr` for `Expr`, and `cell.physical_a1` for `Addresses`.
- [x] Update `crates/f8-cli/tests/inspect_test.rs` to verify new table view modes.

### Phase 3: Sheet CLI Command & Renderer (`f8-cli`)
- [x] Add `InspectSheetArgs` and register `InspectSubcommand::Sheet` in `crates/f8-cli/src/cli.rs`.
- [x] Implement `crates/f8-cli/src/render/sheet.rs` supporting Markdown, Unicode Table, and JSON formats.
- [x] Wire `InspectSubcommand::Sheet` in `crates/f8-cli/src/commands/inspect.rs`.
- [x] Add CLI integration tests for `f8 inspect sheet` in `crates/f8-cli/tests/inspect_test.rs`.

### Phase 4: Verification Suite
- [x] Run `cargo check` across all workspace targets.
- [x] Run `cargo test` across all unit and integration test suites.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo fmt --check`.
