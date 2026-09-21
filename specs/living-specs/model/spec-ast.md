---
title: Specification AST & Schema Ingestion
last_updated: 2026-09-21
domain: model
status: active
---

## Purpose

The `model` subsystem provides the foundational in-memory data structures, Serde models, and semantic validation rules for Finx financial models. It ingests multi-file declarative project specifications (`finx.json` manifests and `tables/*.json` calculation specifications), enforces orthogonal 2D matrix invariants and axis role definitions, and produces structured error diagnostics.

## Architecture & Boundaries

- **Modules & components**:
  - `finx::model::id`: Strongly typed identifier wrappers (`TableId`, `RowId`, `ColId`, `SheetId`).
  - `finx::model::format`: Semantic formatting preset catalog (`FormatPreset`), structural style enum (`SemanticStyle`), and canonical ISO currency codes (`CurrencyCode`).
  - `finx::model::manifest`: Project manifest structures (`ProjectManifest`, `SheetSpec`, `FreezePanes`).
  - `finx::model::table`: 2D table definitions (`TableSpec`), tagged column specifications (`ColumnSpec::Data`, `ColumnSpec::Gutter`), tagged row specifications (`RowSpec::Data`, `RowSpec::SectionHeader`, `RowSpec::Gutter`), axis roles (`AxisRole`), 0D cell inputs (`CellInput`, `DetailedCellInput`), and literal values (`ValueLiteral`). Visual properties (`style`, `highlight`, `decimals`, `currency`) are defined symmetrically on `DataColumnSpec`, `DataRowSpec`, and `DetailedCellInput`. Tables accept optional `subtitle` metadata, default table `currency`, and an opt-in `collapsible` boolean (default: `false`).
  - `finx::model::error`: Diagnostic error types (`ModelError`).

- **Data flow & interfaces**:
  1. JSON string / file content $\rightarrow$ `serde_json::from_str` $\rightarrow$ Unvalidated `TableSpec` / `ProjectManifest`.
  2. `TableSpec::validate` / `ProjectManifest::validate` $\rightarrow$ Invariant verification (unique IDs, primary descriptor validity, cell payload validity, numeric precision applicability) $\rightarrow$ `Ok(())` or `Err(ModelError)`.
  3. Validated models are ready for 2D grid expansion and dependency analysis.
- **Dependencies**:
  - `serde`, `serde_json`: JSON serialization and deserialization with internally/externally tagged specs and untagged cell inputs.
  - `indexmap`: Preserves deterministic insertion and file ordering for keys, rows, and columns.
  - `thiserror`: Structured domain and compiler diagnostic errors.

## Requirements & Invariants

### 1. Orthogonal 2D Matrix Schema

Every table defines an orthogonal 2D matrix with explicit `columns` (`ColumnSpec`) and `rows` (`RowSpec`). Each data column explicitly declares an `AxisRole`: `measure` (default), `descriptor`, or `primary_descriptor`. Each data row defines an optional `indent` level. Multi-tier column headers are explicitly declared as `headers: Vec<String>` on data columns.

#### Scenarios

- **Valid 2D table**:
  - **When** a table defines columns with IDs and roles, rows with vector formulas, and 0D cell overrides in `row.cells`
  - **Then** validation succeeds.
- **Non-empty axes**:
  - **When** a table specifies empty `columns` or empty `rows`
  - **Then** validation returns `ModelError::EmptyColumns` or `ModelError::EmptyRows`.
- **Unique identifiers**:
  - **When** duplicate data column IDs or data row IDs are declared
  - **Then** validation returns `ModelError::DuplicateColumnId` or `ModelError::DuplicateRowId`.

### 2. Cell Input Invariants

Every key in `data_row.cells` must refer to a declared data column ID in `table.columns`. A detailed cell override cannot specify both `value` and `expr`, nor can it specify neither.

#### Scenarios

- **Valid cell override**:
  - **When** `row.cells` specifies literal values, formula objects (`{ "expr": "..." }`), or detailed inputs for declared columns
  - **Then** validation succeeds.
- **Unknown cell column**:
  - **When** a row defines a cell override for a column not declared in `table.columns`
  - **Then** validation returns `ModelError::UnknownCellColumn`.
- **Mutually exclusive cell payload**:
  - **When** a detailed cell override defines both `value` and `expr`
  - **Then** validation returns `ModelError::MutuallyExclusiveCellPayload`.
- **Empty cell payload**:
  - **When** a detailed cell override defines neither `value` nor `expr`
  - **Then** validation returns `ModelError::EmptyCellPayload`.

### 3. Explicit Grid Elements & Primary Descriptor Invariants

Tables represent layout directly through tagged `RowSpec` (`Data`, `SectionHeader`, `Gutter`) and `ColumnSpec` (`Data`, `Gutter`) enums. Row indentation (`indent > 0`) requires exactly one primary descriptor column to be designated.

#### Scenarios

- **Duplicate primary descriptor rejected**:
  - **When** a table declares more than one column with `role: "primary_descriptor"`
  - **Then** validation returns `ModelError::DuplicatePrimaryDescriptor`.
- **Indent without primary descriptor rejected**:
  - **When** any row specifies `indent > 0` but the table has no column with `role: "primary_descriptor"`
  - **Then** validation returns `ModelError::IndentWithoutPrimaryDescriptor`.
- **Explicit gutters and section headers**:
  - **When** rows contain `SectionHeader` or `Gutter` and columns contain `Gutter`
  - **Then** validation succeeds and they participate in physical layout while being excluded from calculation grids.

### 4. Numeric Precision Applicability

The `decimals` attribute specifies fractional display precision and is strictly applicable only to numeric format presets (`currency`, `number`, `percentage`, `multiple`, `basis_points`). Non-numeric presets (`date`, `text`) reject explicit precision declarations. In addition, explicit precision declarations must not exceed the maximum display bound of 14 decimal places.

#### Scenarios

- **Valid Numeric Precision**:
  - **When** a column, row, or detailed cell override specifies `format: "currency"` and `decimals: 2`
  - **Then** validation succeeds.
- **Precision on Non-Numeric Preset Rejected**:
  - **When** a column, row, or detailed cell override specifies `format: "date"` or `format: "text"` with `decimals: Some(n)`
  - **Then** validation returns `ModelError::InvalidPrecisionForPreset`.
- **Precision Exceeding Bound Rejected**:
  - **When** a column, row, or detailed cell override specifies `decimals > 14`
  - **Then** validation returns `ModelError::PrecisionOutOfBounds`.

### 5. Table Subtitle and Currency Applicability

Tables accept optional `subtitle` metadata and an optional default table `currency`. Explicit `currency` declarations may also be applied to columns, rows, and cell overrides, but are strictly permitted only when the effective format preset is `currency`.

#### Scenarios

- **Empty or Whitespace Table Subtitle Rejected**:
  - **When** `table.subtitle` is present but empty or contains only whitespace characters
  - **Then** validation returns `ModelError::EmptyTableSubtitle`.
- **Currency on Non-Currency Preset Rejected**:
  - **When** a column, row, or cell override specifies `currency` with any format preset other than `currency` (e.g. `number`, `percentage`, `date`, `text`)
  - **Then** validation returns `ModelError::InvalidCurrencyForPreset`.

### 6. Opt-In Table-Level Collapsing

Tables accept an optional `collapsible: bool` property (default: `false`, omitted from JSON serialization when false). When enabled, the table's data and header rows are grouped beneath its title banner in Excel workbooks.

#### Scenarios

- **Default Collapsing State**:
  - **When** a table is instantiated without `collapsible`
  - **Then** `table.collapsible` defaults to `false`.
- **Opt-In Collapsing**:
  - **When** a table is created with `table.collapsible = true` or deserialized from `{ "collapsible": true }`
  - **Then** `table.collapsible` is `true`.



