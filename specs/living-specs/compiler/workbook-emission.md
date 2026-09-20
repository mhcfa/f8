---
title: Workbook Assembly & OpenXML Binary Emission
last_updated: 2026-09-20
domain: compiler
status: active
---

## Purpose

The `compiler::emission::workbook` subsystem implements Finx's OpenXML Workbook Assembly and `.xlsx` Binary Emission Engine (Pass 6b). It transforms spatial layouts, theme format caches, lowered Excel formula expressions, and in-memory evaluated results into fully compliant, auditable, and aesthetically polished `.xlsx` OpenXML packages. It guarantees universal preview fidelity (Apple Numbers, Quick Look, Google Sheets, pandas), reproducible compilation, and values-only audit freezing.

## Architecture & Boundaries

- **Modules & components**:
  - `finx::compiler::emission::workbook`: Core structures (`EmissionOptions`, `WorkbookEmitter`), worksheet streamer, structural header assembler, row outline aggregator, and cell writer.
  - `finx::compiler::emission::error`: Diagnostic domain errors (`EmissionError::MissingSheet`, `MissingTable`, `MissingCell`, `Xlsx`, `InvalidColor`, `Model`).
  - `finx::compiler::emission::mod`: Module declarations and public API re-exports.
  - `finx::compiler::mod`: High-level compiler driver facade (`Compiler::emit_workbook`, `Compiler::save_workbook`).
- **Data flow & interfaces**:
  1. Input: `ProjectManifest`, `TableSpec` slice, `CellGrid` (Pass 2 IR), `LayoutMap` (Pass 5), `LoweredFormulaMap` (Pass 6a), `Theme`, optional `EvaluatedModel` (Pass 4), and `EmissionOptions`.
  2. Package setup: Initializes `rust_xlsxwriter::Workbook` with deterministic `DocProperties` (fixed author or custom, fixed timestamp for bit-for-bit reproducibility).
  3. Sheet configuration: Sets sheet names, tab accent colors, gridline visibility, freeze panes, arbitrated column widths, and row heights from `SheetLayout`.
  4. Header streaming: Writes table title banners, subtitles, gutters, super-headers (with 1-column single-cell guard), column key headers (descriptor left, measure right), and interleaved section headers.
  5. Row outline grouping: Recursively determines hierarchical row ranges and applies native Excel outline levels (`worksheet.group_rows`).
  6. Data cell streaming: Queries deduplicated formats from `FormatCache`. In standard mode, writes live formulas pre-populated with evaluation results (`set_result`) and literal values. In values-only mode, writes concrete evaluated numbers/text directly without formula definitions, preserving formula typography.
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

Table title banners, subtitles, super-headers, and column key headers are placed at exact physical row offsets determined in Pass 5.
- **Table title banner**: Written to `start_col` and filled across `(start_col + 1)..=end_col` with `title_format` blanks to create a seamless banner matching the exact table width.
- **Super-headers (Center Across Selection)**: To conform with institutional financial modeling standards (FAST standard: zero merged cells), column groups are emitted using `FormatAlign::CenterAcross` across `start_col..=end_col` without any `<mergeCell>` tags.
- **Row heights**: Left automatic so Excel dynamically computes natural line heights based on font size metrics, maintaining unified row heights across data and gutters.

#### Scenarios

- **Full-width title banner**:
  - **When** table title is emitted
  - **Then** all columns across `start_col..=end_col` receive the title background fill and bottom border.
- **Multi-column super-header without merge cells**:
  - **When** column group covers multiple columns
  - **Then** emitted with `FormatAlign::CenterAcross` across the range without any `<mergeCell>` elements.
- **Single-column super-header**:
  - **When** column group spans exactly one column
  - **Then** emitted as a regular formatted cell with CenterAcross alignment.

### 3. Formula Evaluation Result Caching

Formula cells are written with their in-memory calculation result pre-populated via `Formula::new(formula).set_result(result_str)`.

#### Scenarios

- **Cross-tool formula preview**:
  - **Given** an evaluated model with formula cells
  - **When** emitting the workbook
  - **Then** formula cells contain both `<f>` (formula expression) and `<v>` (precalculated value).

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

### 5. Collapsible Row Group Outlining

Hierarchical row groups are outlined using native Excel row grouping (`worksheet.group_rows(start_row, end_row)`), spanning from the section header down through the last child data row.

#### Scenarios

- **Hierarchical outline levels**:
  - **Given** a table with nested row groups
  - **When** assembling the worksheet
  - **Then** rows carry `outlineLevel="1"`, `outlineLevel="2"`, etc., in worksheet XML.

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

Complete financial models are verified end-to-end across all six compiler passes without polluting CI environments with physical disk artifacts:
- In-memory verification: `tests/export_integration_test.rs` tests `Compiler::emit_workbook` against the Three-Statement, Institutional LBO, and Styling Showcase models in RAM, asserting OpenXML ZIP entries, lowered formulas (`<f>`), and cached values (`<v>`).
- On-demand disk export: `test_export_xlsx_to_disk` is marked with `#[ignore]` and executes only when explicitly requested (`cargo test --test export_integration_test -- --ignored`).
- Standalone runners: `examples/export_models.rs` compiles and exports all models on demand (`cargo run --example export_models`), and `examples/export_styling_showcase.rs` exports the dedicated styling and formatting visual showcase (`cargo run --example export_styling_showcase`).

## Public Interfaces

| Type / Method | Purpose | Signature |
| :--- | :--- | :--- |
| `EmissionOptions` | Emission configuration | `pub struct EmissionOptions { pub values_only: bool, pub title: Option<String>, pub author: Option<String> }` |
| `WorkbookEmitter::emit_to_buffer` | Assemble workbook to byte buffer | `emit_to_buffer(...) -> Result<Vec<u8>, EmissionError>` |
| `WorkbookEmitter::emit_to_file` | Assemble and write to file | `emit_to_file<P: AsRef<Path>>(...) -> Result<(), EmissionError>` |
| `Compiler::emit_workbook` | Facade compiling model to buffer | `emit_workbook(...) -> Result<Vec<u8>, CompilerError>` |
| `Compiler::save_workbook` | Facade compiling and saving to disk | `save_workbook<P: AsRef<Path>>(...) -> Result<(), CompilerError>` |

