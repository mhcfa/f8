---
title: Unified Inspection Facade (CompiledModel)
last_updated: 2026-09-23
domain: compiler
status: active
---

## Purpose

The unified inspection facade (`CompiledModel`) consolidates all compiler pass artifacts into a single in-memory structure produced by a single linear compilation pass (`Compiler::compile`). It serves as the authoritative single source of truth for both OpenXML workbook emission and in-memory model inspection across outer interfaces (`f8-cli`, `f8-mcp`, and WebAssembly runtimes).

## Architecture & Boundaries

- **Modules & components**:
  - `f8_core::compiler::compiled`: Declares `CompiledModel`, inspection query implementations, and domain error `InspectionError`.
  - `f8_core::compiler::compiled::model`: Declares public inspection data contracts (`ModelSummary`, `SheetSummary`, `TableSummary`, `CellInspection`, `CellNeighborInspection`, `TableInspection`, `ColumnInspection`, `RowInspection`, `TableCellInfo`, `DependencyTree`, `DependencyNode`).
  - `f8_core::compiler::mod`: Exposes `Compiler::compile` and `Compiler::compile_with_registry` as the high-level compiler drivers.
- **Data flow & interfaces**:
  - Input: `ProjectManifest`, `&[TableSpec]`, and `Theme`.
  - Execution: Executes Passes 1–6a (`validate` $\rightarrow$ `ir::expand` $\rightarrow$ `graph` $\rightarrow$ `eval` $\rightarrow$ `layout` $\rightarrow$ `lowering`) sequentially in a single pass.
  - Emission: `CompiledModel::emit_workbook` delegates directly to `WorkbookEmitter::emit_to_buffer` without re-running passes.
  - Inspection: Exposes pure in-memory methods (`summary`, `inspect_cell`, `inspect_table`, `precedents_of`, `dependents_of`).
- **Dependencies**: Operates purely in-memory with zero filesystem access; fully compatible with both native OS targets and `wasm32-unknown-unknown`.

## Requirements & Invariants

### 1. Single-Pass Linear Compilation

`Compiler::compile` runs all six passes in linear order, building the unified `CompiledModel` without redundant specification validation, IR expansion, or graph construction.

#### Scenarios

- **Single-pass compilation**:
  - **When** calling `Compiler::compile(&manifest, &tables, &theme)`
  - **Then** executes Passes 1–6a once and returns `CompiledModel` holding all pass artifacts.
- **Fail-fast compilation error**:
  - **When** any pass encounters an error (schema validation, cycle, unassigned driver cell, layout boundary, or lowering failure)
  - **Then** compilation halts immediately and returns the appropriate `CompilerError` variant.

### 2. Zero-Redundancy Workbook Emission

`CompiledModel::emit_workbook` emits OpenXML `.xlsx` byte buffers directly from already-computed artifacts (`grid`, `layout_map`, `lowered_map`, `evaluated`, `theme`).

#### Scenarios

- **Buffer emission from compiled model**:
  - **When** calling `compiled.emit_workbook(&options)`
  - **Then** streams binary OpenXML byte buffer (`Vec<u8>`) with zero pass re-execution.

### 3. High-Level Model Overview

`CompiledModel::summary` calculates hierarchical sheet, table, cell, driver, and formula counts.

#### Scenarios

- **Model summary query**:
  - **When** calling `compiled.summary()`
  - **Then** returns `ModelSummary` where `total_cells == total_drivers + total_formulas` and sheet/table breakdowns match declared metadata.

### 4. Cell-Level Inspection & Auditing

`CompiledModel::inspect_cell` resolves coordinates to physical sheet addresses (`Financials!C18`), evaluated values, pre-formatted value strings (`"$ 115.0"`), format presets, precision, currency, formula ASTs, lowered Excel formulas, and direct incoming/outgoing edges enriched with neighbor display metadata (`CellNeighborInspection`).

#### Scenarios

- **Formula cell inspection**:
  - **When** inspecting a formula cell
  - **Then** returns `CellInspection` with `is_formula = true`, source DSL expression string, lowered `=A1` formula, `formatted_value`, and populated `precedents` containing coordinates and formatted values.
- **Literal cell inspection**:
  - **When** inspecting a literal input cell
  - **Then** returns `CellInspection` with `is_formula = false`, `formula_expr = None`, `formatted_value`, and empty `precedents`.
- **Missing cell coordinate**:
  - **When** querying a coordinate not present in the model grid
  - **Then** returns `InspectionError::CellNotFound`.

### 5. 2D Table Matrix Inspection

`CompiledModel::inspect_table` outputs full 2D table structures with declared column headers, row descriptors, indentations, and cell records containing formatting presets, display precision, and currency metadata in declared order.

#### Scenarios

- **Table inspection**:
  - **When** inspecting a registered table
  - **Then** returns `TableInspection` with columns, rows, and cells enriched with formatting presets, decimal precision, and currency codes.
- **Missing table identifier**:
  - **When** querying a table not present in the model
  - **Then** returns `InspectionError::TableNotFound`.

### 6. Dependency Trees & Blast-Radius Traversals

`precedents_of` and `dependents_of` construct hierarchical trees for upstream root-cause analysis and downstream blast-radius analysis, populating `formatted_value` on each calculation node.

#### Scenarios

- **Direct precedents/dependents**:
  - **When** calling `precedents_of(cell, false)` or `dependents_of(cell, false)`
  - **Then** returns `DependencyTree` containing only direct child nodes with `formatted_value` and empty `children`.
- **Recursive calculation tree**:
  - **When** calling `precedents_of(cell, true)` or `dependents_of(cell, true)`
  - **Then** returns `DependencyTree` with nested children containing `formatted_value` and accurate `transitive_count` reflecting unique reached cells.
- **Reconvergent DAG path pruning**:
  - **When** traversing a calculation graph where a cell is reached through multiple paths
  - **Then** expands children on first encounter and marks subsequent encounters as pruned (`is_pruned = true`, `children = []`), retaining `formatted_value` while bounding tree expansion linearly and preserving exact `transitive_count`.

### 7. Physical Worksheet Grid Inspection

`CompiledModel::inspect_sheet` composites all tables placed on a worksheet into a unified physical 2D grid spanning the worksheet's active bounding box (`min_col..=max_col`, `min_row..=max_row`), rendering table titles, subtitles, buffer gutters, multi-tier column headers, section headers, and data cells.

#### Scenarios

- **Sheet grid inspection**:
  - **When** calling `compiled.inspect_sheet(sheet_id)`
  - **Then** returns `SheetInspection` containing all tables placed on the sheet, physical bounding coordinates, sheet-wide column widths, and physical row records (`SheetRowInspection`) with discrete cell classifications (`SheetCellKind`), values, formatted text, and lowered Excel formulas.
- **Missing worksheet identifier**:
  - **When** querying a sheet not present in the model manifest
  - **Then** returns `InspectionError::SheetNotFound`.

## Public Interfaces

| Method / Type | Purpose | Signature |
| :--- | :--- | :--- |
| `Compiler::compile` | Compiles model specification into `CompiledModel` | `compile(manifest: &ProjectManifest, tables: &[TableSpec], theme: &Theme) -> Result<CompiledModel, CompilerError>` |
| `CompiledModel::emit_workbook` | Zero-redundancy OpenXML workbook emission | `emit_workbook(&self, options: &EmissionOptions) -> Result<Vec<u8>, CompilerError>` |
| `CompiledModel::summary` | Model-, sheet-, and table-level summary | `summary(&self) -> ModelSummary` |
| `CompiledModel::inspect_cell` | Discrete cell-level audit query | `inspect_cell(&self, table: &TableId, row: &RowId, col: &ColId) -> Result<CellInspection, InspectionError>` |
| `CompiledModel::inspect_table` | 2D table grid query | `inspect_table(&self, table: &TableId) -> Result<TableInspection, InspectionError>` |
| `CompiledModel::inspect_sheet` | Physical 2D worksheet grid query across all placed tables | `inspect_sheet(&self, sheet: &SheetId) -> Result<SheetInspection, InspectionError>` |
| `CompiledModel::precedents_of` | Upstream root-cause calculation tree | `precedents_of(&self, cell: &CellId, recursive: bool) -> Result<DependencyTree, InspectionError>` |
| `CompiledModel::dependents_of` | Downstream blast-radius impact tree | `dependents_of(&self, cell: &CellId, recursive: bool) -> Result<DependencyTree, InspectionError>` |
| `CompiledModel::parse_coordinate` | Resolves 3 arguments or 1 canonical IR string into CellId | `parse_coordinate(&self, target: &str, row: Option<&str>, col: Option<&str>) -> Result<CellId, InspectionError>` |
| `parse_coordinate_tokens` | Free function parsing coordinate arguments into IDs | `parse_coordinate_tokens(target: &str, row: Option<&str>, col: Option<&str>) -> Result<(TableId, RowId, ColId), InspectionError>` |
