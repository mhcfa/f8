---
title: Workbook Assembly & OpenXML Binary Emission
last_updated: 2026-09-22
domain: compiler
status: active
---

## Purpose

The `compiler::emission::workbook` subsystem implements Finx's OpenXML Workbook Assembly and `.xlsx` Binary Emission Engine (Pass 6b). It transforms spatial layouts, theme format caches, lowered Excel formula expressions, and in-memory evaluated results into fully compliant, auditable, and aesthetically polished `.xlsx` OpenXML packages. It guarantees universal preview fidelity (Apple Numbers, Quick Look, Google Sheets, pandas), reproducible compilation, and values-only audit freezing.

## Architecture & Boundaries

- **Modules & components**:
  - `finx::compiler::emission::workbook`: Core structures (`EmissionOptions`, `WorkbookEmitter`), worksheet streamer, structural header assembler, table-level collapsing manager, and cell writer.
  - `finx::compiler::emission::error`: Diagnostic domain errors (`EmissionError::MissingSheet`, `MissingTable`, `MissingCell`, `Xlsx`, `InvalidColor`, `Model`).
  - `finx::compiler::emission::mod`: Module declarations and public API re-exports.
  - `finx::compiler::mod`: High-level compiler driver facade (`Compiler::emit_workbook`, `Compiler::save_workbook`).
- **Data flow & interfaces**:
  1. Input: `ProjectManifest`, `TableSpec` slice, `CellGrid` (Pass 2 IR), `LayoutMap` (Pass 5), `LoweredFormulaMap` (Pass 6a), `Theme`, optional `EvaluatedModel` (Pass 4), and `EmissionOptions`.
  2. Package setup: Initializes `rust_xlsxwriter::Workbook` with deterministic `DocProperties` (fixed author or custom, fixed timestamp for bit-for-bit reproducibility).
  3. Sheet configuration: Sets sheet names, tab accent colors, gridline visibility, freeze panes, arbitrated column widths, and row heights from `SheetLayout`.
  4. Header streaming: Writes table title banners, subtitles, gutters, multi-tier super-headers (with contiguous run `CenterAcross` emission), column key headers (descriptor left, measure right), and continuous section header rows.
  5. Table-level collapsing (opt-in): If `table.collapsible` is true, configures symbols above (`group_symbols_above(true)`) and collapses the table body under the title banner (`worksheet.group_rows(start_row + 1, end_row)`). Outline buttons are omitted to eliminate UI clutter, relying instead on descriptor cell indentation and explicit gutters.
  6. Data cell streaming: Queries deduplicated formats from `FormatCache` with descriptor indentation applied directly. In standard mode, writes live formulas pre-populated with evaluation results (`set_result`) and literal values. In values-only mode, writes concrete evaluated numbers/text directly without formula definitions, preserving formula typography.
  7. Finalization: Serializes to in-memory byte buffer (`Vec<u8>`) or writes directly to disk.
- **Dependencies**:
  - `rust_xlsxwriter`: High-performance OpenXML writer.
  - `indexmap`: Preserves deterministic order for worksheets, tables, rows, and columns.
  - `thiserror`: Structured diagnostic error propagation.

## Requirements & Invariants

### 1. Order Preservation & Worksheet Setup

Worksheets and tables are emitted in the exact order declared in `manifest.sheets`. Tab colors, gridline visibility, freeze panes, column widths, and row heights are applied directly from `SheetLayout`.

#### Scenarios

- **Multi-sheet order**:
  - **Given** a manifest declaring multiple sheets and tables
  - **When** assembling the workbook
  - **Then** sheets appear in declared order in `xl/workbook.xml`.
- **Sheet metrics & properties**:
  - **Given** a worksheet layout with tab color, disabled gridlines, and freeze panes
  - **When** assembling the worksheet
  - **Then** `tabColor`, `showGridLines="0"`, and `pane` elements are faithfully emitted in worksheet XML.

### 2. Structural Headers & Super-Header Safety

Table title banners, subtitles, multi-tier super-headers, and column key headers are placed at exact physical row offsets determined in Pass 5.
- **Table title banner**: Written to `start_col` and filled across `(start_col + 1)..=end_col` with `title_format` blanks to create a seamless banner matching the exact table width.
- **Super-headers (Center Across Selection)**: To conform with institutional financial modeling standards (FAST standard: zero merged cells), contiguous identical super-header runs across adjacent columns are emitted using `FormatAlign::CenterAcross` across `start_col..=end_col` without any `<mergeCell>` tags.
- **Row heights**: Left automatic so Excel dynamically computes natural line heights based on font size metrics, maintaining unified row heights across data and gutters.

#### Scenarios

- **Full-width title banner**:
  - **When** table title is emitted
  - **Then** all columns across `start_col..=end_col` receive the title background fill and bottom border.
- **Multi-column super-header without merge cells**:
  - **When** a super-header label spans multiple adjacent columns
  - **Then** emitted with `FormatAlign::CenterAcross` across the range without any `<mergeCell>` elements.
- **Single-column super-header**:
  - **When** a super-header label spans exactly one column
  - **Then** emitted as a regular formatted cell with CenterAcross alignment.

### 3. Formula Evaluation Result Caching

Formula cells are written with their in-memory calculation result pre-populated via `Formula::new(formula).set_result(result_str)`. When `options.omit_formula_cache` is true, result tags (`<v>`) are omitted for formula cells, forcing external calculation engines (such as Microsoft Excel or Web Excel) to evaluate formulas independently upon opening.

#### Scenarios

- **Cross-tool formula preview**:
  - **Given** an evaluated model with formula cells
  - **When** emitting the workbook with default options (`omit_formula_cache: false`)
  - **Then** formula cells contain both `<f>` (formula expression) and `<v>` (precalculated value).
- **Uncached formula emission**:
  - **Given** an evaluated model with formula cells and `EmissionOptions { omit_formula_cache: true, .. }`
  - **When** emitting the workbook
  - **Then** formula cells contain `<f>` (formula expression) and omit precalculated `<v>` tags.

### 4. Values-Only Freezing Mode

When `options.values_only` is true:
- All formula cells are written as styled literal values without formula definitions (`<f>`).
- If caller provides `evaluated: None`, compiler driver automatically evaluates the model to guarantee concrete values are frozen without blank cells.
- Formula cells preserve `is_input: false` to retain formula black text rather than input blue text.

#### Scenarios

- **Audit data room freeze**:
  - **Given** `EmissionOptions { values_only: true, .. }`
  - **When** emitting the workbook
  - **Then** no `<f>` tags exist in sheet XML; all numbers and strings are emitted directly into `<v>` with standard presentation formatting.

### 5. Table-Level Collapsing & Row Presentation

To keep financial models clean, legible, and unencumbered by Excel outline UI clutter:
- **No row outline buttons**: Intermediate rows do not emit Excel outline buttons (`[+]`/`[-]`). Hierarchy is communicated visually via descriptor cell indentation (`format.set_indent(indent)`), explicit section header rows, and explicit gutter rows.
- **Opt-in table collapsing**: When `table.collapsible` is true, the emitter calls `worksheet.group_symbols_above(true)` and `worksheet.group_rows(placement.start_row + 1, placement.end_row)`. This creates a native Excel level 1 outline spanning the entire table body below the table title banner. When collapsed, the title banner remains visible while the headers, gutters, and data cells are hidden.

#### Scenarios

- **Default presentation (no outline buttons)**:
  - **Given** a table with indented rows and `collapsible: false`
  - **When** assembling the worksheet
  - **Then** sheet XML contains no row outline levels (`summaryBelow` and outline brackets are absent), and descriptor cells contain `indent="1"`, `indent="2"`.
- **Opt-in table-level collapsing**:
  - **Given** a table with `collapsible: true`
  - **When** assembling the worksheet
  - **Then** table rows from `start_row + 1` to `end_row` carry `outlineLevel="1"`, and `summaryBelow="0"` is set on `sheetPr/outlinePr`.


### 6. Deterministic Package Properties

OpenXML core document properties are configured deterministically with fixed author and constant creation date (`2026-01-01T00:00:00Z`), guaranteeing identical byte buffers across identical model inputs.

#### Scenarios

- **Reproducible compilation**:
  - **When** compiling the same model specification twice
  - **Then** produces bit-for-bit identical `.xlsx` byte buffers.

### 7. Zero Production Panics & Error Diagnostics

Missing coordinates or unassigned entities propagate structured diagnostics (`EmissionError::MissingSheet`, `MissingTable`, `MissingCell`) without unwraps or panics.

#### Scenarios

- **Missing cell coordinate**:
  - **When** layout map is missing a required cell ID
  - **Then** returns `EmissionError::MissingCell` cleanly.

### 8. End-to-End Integration Verification & Model Exports

Complete financial models and calculation verification suites are verified end-to-end across all six compiler passes without polluting CI environments with physical disk artifacts:
- In-memory verification: `tests/export_integration_test.rs` tests `Compiler::emit_workbook` against the Three-Statement, Institutional LBO, Styling Showcase, and 44-function Excel Parity Verification models in RAM, asserting OpenXML ZIP entries, lowered formulas (`<f>`), and cached values (`<v>`).
- On-demand disk export: `test_export_xlsx_to_disk` is marked with `#[ignore]` and executes only when explicitly requested (`cargo test --test export_integration_test -- --ignored`). It produces disk workbooks in `output/` including `output/excel_parity_verification.xlsx` (emitted uncached for visual verification in Microsoft Excel / Web Excel).
- Standalone runner: `examples/export_models.rs` compiles and exports all five models on demand (`cargo run --example export_models`) into `output/`.

## Public Interfaces

| Type / Method | Purpose | Signature |
| :--- | :--- | :--- |
| `EmissionOptions` | Emission configuration | `pub struct EmissionOptions { pub values_only: bool, pub omit_formula_cache: bool, pub title: Option<String>, pub author: Option<String> }` |
| `EmissionOptions::with_omit_formula_cache` | Builder for uncached formulas | `pub const fn with_omit_formula_cache(mut self, omit_formula_cache: bool) -> Self` |
| `WorkbookEmitter::emit_to_buffer` | Assemble workbook to byte buffer | `emit_to_buffer(...) -> Result<Vec<u8>, EmissionError>` |
| `WorkbookEmitter::emit_to_file` | Assemble and write to file | `emit_to_file<P: AsRef<Path>>(...) -> Result<(), EmissionError>` |
| `Compiler::emit_workbook` | Facade compiling model to buffer | `emit_workbook(...) -> Result<Vec<u8>, CompilerError>` |
| `Compiler::save_workbook` | Facade compiling and saving to disk | `save_workbook<P: AsRef<Path>>(...) -> Result<(), CompilerError>` |

