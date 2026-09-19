---
title: Specification AST & Schema Ingestion
last_updated: 2026-09-19
domain: model
status: active
---

## Purpose

The `model` subsystem provides the foundational in-memory data structures, Serde models, and semantic validation rules for Finx financial models. It ingests multi-file declarative project specifications (`finx.json` manifests and `tables/*.json` calculation specifications), enforces orthogonal 2D matrix invariants and axis role definitions, and produces structured error diagnostics.

## Architecture & Boundaries

- **Modules & components**:
  - `finx::model::id`: Strongly typed identifier wrappers (`TableId`, `RowId`, `ColId`, `ColGroupId`, `RowGroupId`).
  - `finx::model::format`: Semantic formatting preset catalog (`FormatPreset`) and structural style enum (`SemanticStyle`).
  - `finx::model::manifest`: Project manifest structures (`ProjectManifest`, `SheetSpec`, `FreezePanes`).
  - `finx::model::table`: 2D table definitions (`TableSpec`), column specifications (`ColumnSpec`), row specifications (`RowSpec`), axis roles (`AxisRole`), 0D cell inputs (`CellInput`, `DetailedCellInput`), column and row groups (`ColumnGroup`, `RowGroup`), and literal values (`ValueLiteral`). Visual properties (`style`, `highlight`, `decimals`) are defined symmetrically on `ColumnSpec`, `RowSpec`, and `DetailedCellInput`.
  - `finx::model::error`: Diagnostic error types (`ModelError`).
- **Data flow & interfaces**:
  1. JSON string / file content $\rightarrow$ `serde_json::from_str` $\rightarrow$ Unvalidated `TableSpec` / `ProjectManifest`.
  2. `TableSpec::validate` / `ProjectManifest::validate` $\rightarrow$ Invariant verification (unique IDs, contiguous groups, cell payload validity, numeric precision applicability) $\rightarrow$ `Ok(())` or `Err(ModelError)`.
  3. Validated models are ready for 2D grid expansion and dependency analysis.
- **Dependencies**:
  - `serde`, `serde_json`: JSON serialization and deserialization with untagged cell inputs.
  - `indexmap`: Preserves deterministic insertion and file ordering for keys, rows, and columns.
  - `thiserror`: Structured domain and compiler diagnostic errors.

## Requirements & Invariants

### 1. Orthogonal 2D Matrix Schema

Every table defines an orthogonal 2D matrix with explicit `columns` (`ColumnSpec`) and `rows` (`RowSpec`). Each axis explicitly declares an `AxisRole`: `measure` (default) or `descriptor`.

#### Scenarios

- **Valid 2D Table**:
  - **When** a table defines columns with IDs and roles, rows with vector formulas, and 0D cell overrides in `row.cells`
  - **Then** validation succeeds.
- **Non-Empty Axes**:
  - **When** a table specifies empty `columns` or empty `rows`
  - **Then** validation returns `ModelError::EmptyColumns` or `ModelError::EmptyRows`.
- **Unique Identifiers**:
  - **When** duplicate column IDs or row IDs are declared
  - **Then** validation returns `ModelError::DuplicateColumnId` or `ModelError::DuplicateRowId`.

### 2. Cell Input Invariants

Every key in `row.cells` must refer to a declared column ID in `table.columns`. A detailed cell override cannot specify both `value` and `expr`, nor can it specify neither.

#### Scenarios

- **Valid Cell Override**:
  - **When** `row.cells` specifies literal values, formula objects (`{ "expr": "..." }`), or detailed inputs for declared columns
  - **Then** validation succeeds.
- **Unknown Cell Column**:
  - **When** a row defines a cell override for a column not declared in `table.columns`
  - **Then** validation returns `ModelError::UnknownCellColumn`.
- **Mutually Exclusive Cell Payload**:
  - **When** a detailed cell override defines both `value` and `expr`
  - **Then** validation returns `ModelError::MutuallyExclusiveCellPayload`.
- **Empty Cell Payload**:
  - **When** a detailed cell override defines neither `value` nor `expr`
  - **Then** validation returns `ModelError::EmptyCellPayload`.

### 3. Contiguous & Disjoint Hierarchies

Column groups and row groups must form contiguous sub-slices of their respective dimension arrays, with no overlapping memberships.

#### Scenarios

- **Contiguous Groups**:
  - **When** group members appear sequentially in `columns` or `rows`
  - **Then** validation succeeds.
- **Non-Contiguous Groups**:
  - **When** group items are separated by intervening non-member items
  - **Then** validation returns `ModelError::NonContiguousColumnGroup` or `ModelError::NonContiguousRowGroup`.
- **Cyclic Group Hierarchy**:
  - **When** a nested row group references an ancestor group ID
  - **Then** validation returns `ModelError::CyclicRowGroup`.

### 4. Numeric Precision Applicability

The `decimals` attribute specifies fractional display precision and is strictly applicable only to numeric format presets (`currency`, `number`, `percentage`, `multiple`, `basis_points`). Non-numeric presets (`date`, `text`) reject explicit precision declarations.

#### Scenarios

- **Valid Numeric Precision**:
  - **When** a column, row, or detailed cell override specifies `format: "currency"` and `decimals: 2`
  - **Then** validation succeeds.
- **Precision on Non-Numeric Preset Rejected**:
  - **When** a column, row, or detailed cell override specifies `format: "date"` or `format: "text"` with `decimals: Some(n)`
  - **Then** validation returns `ModelError::InvalidPrecisionForPreset`.

