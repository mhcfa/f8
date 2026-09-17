---
title: Specification AST & Schema Ingestion
last_updated: 2026-09-16
domain: model
status: active
---

## Purpose

The `model` subsystem provides the foundational in-memory data structures, Serde models, and semantic validation rules for Finx financial models. It ingests multi-file declarative project specifications (`finx.json` manifests and `tables/*.json` calculation specifications), enforces dimensional and partitioning invariants, and produces structured error diagnostics.

## Architecture & Boundaries

- **Modules & components**:
  - `finx::model::id`: Strongly typed identifier wrappers (`TableId`, `RowId`, `ColId`, `ColGroupId`, `RowGroupId`).
  - `finx::model::format`: Semantic formatting preset catalog (`FormatPreset`).
  - `finx::model::manifest`: Project manifest structures (`ProjectManifest`, `SheetSpec`, `FreezePanes`).
  - `finx::model::table`: Table definitions (`TableSpec`), row specifications (`RowSpec`), match selectors (`MatchSelector`), column and row groups (`ColumnGroup`, `RowGroup`), and literal values (`ValueLiteral`).
  - `finx::model::error`: Diagnostic error types (`ModelError`).
- **Data flow & interfaces**:
  1. JSON string / file content $\rightarrow$ `serde_json::from_str` $\rightarrow$ Unvalidated `TableSpec` / `ProjectManifest`.
  2. `TableSpec::validate` / `ProjectManifest::validate` $\rightarrow$ Invariant verification $\rightarrow$ `Ok(())` or `Err(ModelError)`.
  3. Validated models are ready for grid expansion and dependency analysis.
- **Dependencies**:
  - `serde`, `serde_json`: JSON serialization and deserialization.
  - `indexmap`: Preserves deterministic insertion and file ordering for keys and columns.
  - `thiserror`: Structured domain and compiler diagnostic errors.

## Requirements & Invariants

### 1. Payload Mutual Exclusivity

Every row must specify exactly one calculation payload: `value`, `expr`, or `match`. Similarly, every arm in a `match` block must specify exactly one of `value` or `expr`.

#### Scenarios

- **Valid Single Payload**:
  - **When** a row defines only `"expr": "Table[\"x\", @]"` or only `"value": [1.0, 2.0]`
  - **Then** validation succeeds.
- **Multiple Payloads Rejected**:
  - **When** a row specifies both `"value": [1.0]` and `"expr": "Table[\"x\", @]"`
  - **Then** validation returns `ModelError::MutuallyExclusivePayload`.
- **Zero Payloads Rejected**:
  - **When** a row specifies neither `value`, `expr`, nor `match`
  - **Then** validation returns `ModelError::MutuallyExclusivePayload`.

### 2. Dimension Consistency

Literal arrays in `value` fields must have exact length matching the targeted column slice.

#### Scenarios

- **Matching Dimensions**:
  - **When** a table with 3 columns defines a row with `"value": [10.0, 20.0, 30.0]`
  - **Then** validation succeeds.
- **Mismatched Dimensions**:
  - **When** a table with 3 columns defines a row with `"value": [10.0, 20.0]`
  - **Then** validation returns `ModelError::DimensionMismatch`.

### 3. Exhaustive & Disjoint Match Partitioning

When a row uses a `match` block, every column in `table.columns` must be covered by exactly one arm.

#### Scenarios

- **Exhaustive Partitioning**:
  - **When** arms cover all declared table columns without overlap
  - **Then** validation succeeds.
- **Uncovered Columns**:
  - **When** one or more table columns are not matched by any arm
  - **Then** validation returns `ModelError::UncoveredColumns` listing missing columns.
- **Overlapping Arms**:
  - **When** two match arms select the same column
  - **Then** validation returns `ModelError::OverlappingMatchArm`.

### 4. Contiguous & Disjoint Hierarchies

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
