---
title: Workbook Assembly & OpenXML Binary Emission
date: 2026-09-20
status: completed
target-living-spec: specs/living-specs/compiler/workbook-emission.md
---

## Proposal

### What

Implement Finx's OpenXML Workbook Assembly and `.xlsx` Binary Emission Engine (Pass 6b) in `finx::compiler::emission::workbook`:
1. Define `EmissionOptions` controlling output generation parameters (values-only freezing, custom title and author metadata).
2. Implement `WorkbookEmitter` streaming sheets, tables, visual headers, and data cells into a `rust_xlsxwriter::Workbook`:
   - Worksheet initialization with declared ordering, tab colors, gridline visibility, and freeze panes.
   - Sheet-wide column widths and row heights application from `SheetLayout`.
   - Structural header streaming: table title banners, subtitle metadata, title buffer gutters, multi-cell super-headers (with single-cell guard), column key headers (descriptor left, measure right), and interleaved section headers.
   - Collapsible row group outline levels (`worksheet.group_rows`).
   - Cell streaming using deduplicated `FormatCache` styles, emitting literal values (numbers, strings, booleans, blanks) and live formulas.
   - Formula result pre-population via `Formula::new(expr).set_result(val)` using in-memory evaluated results for instant rendering in third-party viewers (Apple Numbers, Quick Look, Google Sheets, pandas).
   - Values-only freezing mode (`values_only: true`) emitting evaluated results directly as styled literals without formula definitions.
   - Deterministic package properties (`DocProperties`) ensuring reproducible binary compilation.
   - Direct serialization to in-memory buffers (`Vec<u8>`) and disk files.
3. Expose high-level compiler driver methods in `finx::compiler::Compiler`:
   - `Compiler::emit_workbook` compiling a model specification directly to an in-memory `.xlsx` byte buffer.
   - `Compiler::save_workbook` compiling and writing directly to a destination path on disk.
4. Provide an integration test suite in `tests/workbook_emission_test.rs` validating worksheet structure, styling, formula evaluation caches, values-only freezing, and end-to-end model compilation.

### Why

- **End-to-end pipeline completion**: Completes the core compiler pipeline from declarative JSON specifications (`finx.json` + `tables/*.json`) to auditable `.xlsx` binaries.
- **Instant preview fidelity**: Non-Excel spreadsheet viewers do not execute formula calculations on document load. Pre-populating formula calculation results in the cell cache ensures immediate, accurate visualization across macOS Quick Look, Apple Numbers, Google Drive preview, and data pipelines (pandas, openpyxl).
- **Institutional modeling aesthetics**: Guarantees that the generated workbook faithfully honors all spatial layout, typography, accounting underlines, column widths, and hardcode color-coding rules defined across the model.
- **Audit room and M&A compliance**: Supports values-only compilation to freeze calculations into immutable financial baselines.
- **Bit-for-bit reproducible compilation**: Ensures identical model inputs generate identical OpenXML packages without nondeterministic timestamp drift.

### Scope

- **In-scope**:
  - `EmissionOptions` in `src/compiler/emission/workbook.rs`.
  - Additional variants in `EmissionError`: `MissingSheet`, `MissingTable`, `MissingCell`.
  - `WorkbookEmitter` in `src/compiler/emission/workbook.rs` supporting buffer and file targets.
  - Applying worksheet settings: tab colors, gridlines, freeze panes, column widths, row heights.
  - Streaming structural headers (titles, subtitles, title gutters, super-headers, column headers, section headers).
  - Outlining rows for hierarchical row groups (`group_rows`).
  - Streaming literals, formulas, precalculated cached values, and values-only freezing.
  - Deterministic package properties configuration (`DocProperties`).
  - Compiler facade orchestration in `src/compiler/mod.rs` (`emit_workbook`, `save_workbook`).
  - Comprehensive integration test suite in `tests/workbook_emission_test.rs`.

- **Out-of-scope**:
  - Headless CSV / Parquet / JSON exports (deferred to Headless Data Serialization spec).
  - CLI binary commands (`finx compile`) and CLI argument parsing (deferred to CLI Interface spec).
  - MCP agent tools (deferred to MCP Server spec).

### What Changes

- **Affected modules**:
  - `src/compiler/emission/error.rs`: Add `MissingSheet`, `MissingTable`, and `MissingCell` error variants to `EmissionError`.
  - `src/compiler/emission/workbook.rs`: New module implementing `EmissionOptions` and `WorkbookEmitter`.
  - `src/compiler/emission/mod.rs`: Re-export `EmissionOptions` and `WorkbookEmitter`.
  - `src/compiler/mod.rs`: Re-export emission types and add `emit_workbook` / `save_workbook` facade methods.
  - `tests/workbook_emission_test.rs`: New integration test suite.

### Capabilities

- **New capabilities**:
  - Compile any valid Finx financial model directly into a fully formatted `.xlsx` workbook binary.
  - Emit live formulas pre-populated with in-memory evaluated results for instant cross-tool previewing.
  - Freeze models into values-only workbooks with preserved formatting.
  - Render hierarchical outlines, merged super-headers, and arbitrated column widths in Excel.
- **Modified capabilities**:
  - `Compiler` facade gains end-to-end workbook compilation entrypoints.

---

## Design

### Goals

1. **Complete fidelity**: The emitted workbook matches the exact coordinates, column widths, row heights, typography, fills, and borders computed in Passes 5, 6a, and 6b.
2. **Universal readability**: Formula cells carry precalculated evaluation results so third-party viewers display correct values without requiring an Excel recalculation cycle.
3. **Reproducible output**: Package metadata is populated deterministically to prevent timestamp drift across builds.
4. **Zero production panics**: All file I/O, OpenXML formatting, and coordinate lookups propagate structured errors via `EmissionError`.

### Invariants

- **Order preservation**: Worksheets and tables within worksheets are emitted in the exact order declared in `manifest.sheets`.
- **Single-cell super-header safety**: When a column group spans exactly one column (`start_col == end_col`), write as a single cell rather than calling `merge_range` to avoid `XlsxError::MergeRangeSingleCell`.
- **Precedence in values-only mode**: In values-only mode, formula cells are written as styled literal values (keeping formula black text, not input blue text).
- **Descriptor styling preservation**: Descriptor columns always use left alignment and neutral text color, even if literal string payloads are emitted.

### Decisions

- **Formula result cache population**:
  - *Choice*: When `evaluated` is provided, retrieve `evaluated.get_cell(&cell_id)` and pass stringified result into `Formula::new(formula).set_result(result_str)`.
  - *Rationale*: Pre-populating the formula cache allows macOS Quick Look, Apple Numbers, Google Drive preview, and pandas to display evaluated numbers immediately on open. Excel recalculates dynamically upon edit.
  - *Alternatives considered*: Omitting cached formula results. Rejected because third-party spreadsheet viewers display empty cells, `#VALUE!`, or zeros without an initial Excel recalculation.

- **Values-only freezing mechanism & evaluation fallback**:
  - *Choice*: When `options.values_only` is true, write the evaluated `CellValue` (or fallback literal payload) using `write_number_with_format`, `write_string_with_format`, or `write_boolean_with_format`. Maintain `is_input: false` during format resolution to retain standard formula text coloring. When `options.values_only` is true and the caller passes `evaluated: None`, `Compiler::emit_workbook` automatically executes in-memory evaluation (`Compiler::evaluate`) to ensure concrete frozen values are emitted without missing cells.
  - *Rationale*: M&A audit rooms, external investor distributions, and baseline models require static snapshots without exposing live formulas or internal calculation logic. Automatic evaluation fallback guarantees seamless ergonomics and prevents silent empty cells.

- **Super-header multi-column merging**:
  - *Choice*: Guard against single-column groups. If `start_col < end_col`, call `worksheet.merge_range(row, start_col, row, end_col, label, &format)`. If `start_col == end_col`, call `worksheet.write_string_with_format(row, start_col, label, &format)`.
  - *Rationale*: `rust_xlsxwriter` returns `Err(XlsxError::MergeRangeSingleCell)` if a 1x1 range is passed to `merge_range`.

- **Collapsible row group outlining**:
  - *Choice*: For each row group in a table, determine physical start and end rows (spanning from its interleaved section header down through its last child data row) and invoke `worksheet.group_rows(start_row, end_row)` for nested groups, allowing Excel to natively collapse and expand sub-categories.
  - *Rationale*: Provides institutional model navigation matching Wall Street standards.

- **Deterministic package metadata**:
  - *Choice*: Configure `DocProperties` with a fixed creator string (`"Finx Compiler"`) and set custom or blank timestamps unless explicitly configured.
  - *Rationale*: OpenXML packages are zip archives. Avoiding dynamic wall-clock timestamps produces deterministic byte streams across identical model inputs.

### Data Contracts

#### Public Types in `src/compiler/emission/workbook.rs`

```rust
/// Configuration options for OpenXML workbook emission.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EmissionOptions {
    /// When true, emits evaluated values instead of formulas (frozen audit workbook).
    pub values_only: bool,
    /// Document title metadata in docProps/core.xml.
    pub title: Option<String>,
    /// Document author metadata in docProps/core.xml.
    pub author: Option<String>,
}

/// OpenXML workbook assembly engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkbookEmitter;

impl WorkbookEmitter {
    /// Compiles a model into an in-memory .xlsx OpenXML byte buffer.
    pub fn emit_to_buffer(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        grid: &CellGrid,
        layout_map: &LayoutMap,
        lowered_formulas: &LoweredFormulaMap,
        theme: &Theme,
        evaluated: Option<&EvaluatedModel>,
        options: &EmissionOptions,
    ) -> Result<Vec<u8>, EmissionError>;

    /// Compiles a model and writes the .xlsx workbook to a destination path on disk.
    pub fn emit_to_file<P: AsRef<Path>>(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        grid: &CellGrid,
        layout_map: &LayoutMap,
        lowered_formulas: &LoweredFormulaMap,
        theme: &Theme,
        evaluated: Option<&EvaluatedModel>,
        options: &EmissionOptions,
        path: P,
    ) -> Result<(), EmissionError>;
}
```

#### Compiler Facade Additions in `src/compiler/mod.rs`

```rust
impl Compiler {
    /// Compiles a model specification and theme into an in-memory .xlsx workbook byte buffer.
    pub fn emit_workbook(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        theme: &Theme,
        evaluated: Option<&EvaluatedModel>,
        options: &EmissionOptions,
    ) -> Result<Vec<u8>, CompilerError>;

    /// Compiles a model specification and theme and saves the .xlsx workbook to a file path.
    pub fn save_workbook<P: AsRef<Path>>(
        manifest: &ProjectManifest,
        tables: &[TableSpec],
        theme: &Theme,
        evaluated: Option<&EvaluatedModel>,
        options: &EmissionOptions,
        path: P,
    ) -> Result<(), CompilerError>;
}
```

### Risks & Trade-offs

- **Format count scaling**: Deduplicating via `FormatCache` keeps `<cellXfs>` well below Excel's 64,000 style limit even on massive financial models.
- **String conversion of formula results**: `rust_xlsxwriter` accepts string representations of results (e.g. `"1250000.50"` or `"TRUE"`). Exact string representation must match Excel formatting standards.

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: End-to-End Workbook Binary Generation**
  - **Given** a valid project manifest and table specifications
  - **When** executing `Compiler::emit_workbook` with default options
  - **Then** produces a non-empty `.xlsx` byte buffer that parses as a valid zip/OpenXML archive with all declared sheets.

- [x] **Scenario 2: Sheet Layout Metrics and Column Widths Application**
  - **Given** a sheet layout with computed column widths, row heights, freeze panes, tab color, and gridline visibility
  - **When** assembling the worksheet in `WorkbookEmitter`
  - **Then** each worksheet attribute is applied to the corresponding `rust_xlsxwriter::Worksheet`.

- [x] **Scenario 3: Structural Headers and Super-Headers Emission**
  - **Given** tables with title, subtitle, title buffer gutter, super-headers, and column key headers
  - **When** emitting the table header block
  - **Then** table title is written with title format, subtitle with subtitle format, multi-column super-headers are merged, and column headers are placed at physical row indices.

- [x] **Scenario 4: Single-Cell Super-Header Safety**
  - **Given** a column group spanning exactly one column (`start_col == end_col`)
  - **When** emitting the super-header
  - **Then** the header is written as a regular cell without calling `merge_range`, avoiding `MergeRangeSingleCell` errors.

- [x] **Scenario 5: Live Formulas with Evaluated Result Caching**
  - **Given** a model with formula cells and an in-memory `EvaluatedModel`
  - **When** emitting the workbook
  - **Then** formula cells are written via `write_formula_with_format` with their evaluated result pre-populated in the formula cache.

- [x] **Scenario 6: Values-Only Freezing Mode**
  - **Given** `EmissionOptions { values_only: true, .. }`
  - **When** emitting the workbook
  - **Then** formula cells are written as literal numbers/strings without formula definitions, preserving black text formatting.

- [x] **Scenario 7: Input Blue vs Formula Black Color Coding**
  - **Given** a model with measure input literals and formula cells
  - **When** streaming data cells
  - **Then** measure input cells use blue `input_text` format and formula cells use black `formula_text` format.

- [x] **Scenario 8: Collapsible Row Group Outlining**
  - **Given** a table with hierarchical row groups
  - **When** emitting the table
  - **Then** `group_rows` is invoked for row ranges matching the group hierarchy.

- [x] **Scenario 9: Save Workbook Directly to Disk File**
  - **Given** a valid model and destination file path
  - **When** executing `Compiler::save_workbook`
  - **Then** the `.xlsx` file is successfully created on disk.

- [x] **Scenario 10: Missing Coordinate or Entity Diagnostics**
  - **Given** an inconsistent layout map missing a required cell coordinate
  - **When** streaming workbook cells
  - **Then** returns `EmissionError::MissingCell` without panicking.

---

## Tasks

### Phase 1: Error Types & Options
- [x] Add `MissingSheet { sheet: SheetId }`, `MissingTable { table: TableId }`, and `MissingCell { cell: CellId }` variants to `EmissionError` in `src/compiler/emission/error.rs`.
- [x] Define `EmissionOptions` in `src/compiler/emission/workbook.rs`.
- [x] Update `src/compiler/emission/mod.rs` to declare and re-export `workbook` types.

### Phase 2: Workbook Assembly Engine
- [x] Implement worksheet setup: creation, tab colors, gridlines, freeze panes, column widths, row heights.
- [x] Implement structural header streaming: title, subtitle, title gutter, super-headers (with 1-cell check), column headers, section headers.
- [x] Implement row group outline grouping (`worksheet.group_rows`).
- [x] Implement data cell streaming: format cache lookup, literal cells, formula cells with cached results, values-only mode.
- [x] Implement `WorkbookEmitter::emit_to_buffer` and `WorkbookEmitter::emit_to_file`.

### Phase 3: Compiler Driver Integration
- [x] Add `Compiler::emit_workbook` and `Compiler::save_workbook` methods to `src/compiler/mod.rs`.
- [x] Export `EmissionOptions` and `WorkbookEmitter` from `src/compiler/mod.rs`.

### Phase 4: Integration Test Suite & Verification
- [x] Create `tests/workbook_emission_test.rs` covering all 10 acceptance scenarios.
- [x] Run verification suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- [x] Update living specification `specs/living-specs/compiler/workbook-emission.md` and archive draft to `specs/history/`.
