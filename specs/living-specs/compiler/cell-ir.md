---
title: Cell Intermediate Representation & Symbol Table
last_updated: 2026-09-16
domain: compiler
status: active
---

## Purpose

The `compiler::ir` subsystem provides discrete cell-level Intermediate Representation (IR) data structures, the canonical cell identity model, deterministic cell collection containers, and the workbook symbol table. It translates high-level table declarations into fine-grained calculation nodes, preventing false-positive circular dependencies in inter-period roll-forwards and providing the node matrix for dependency analysis, topological evaluation, and coordinate lowering.

## Architecture & Boundaries

- **Modules & components**:
  - `finx::compiler::ir::id`: Canonical `CellId` representing the 3-tuple `(TableId, RowId, ColId)`.
  - `finx::compiler::ir::cell`: `CellPayload` (`Literal` vs `Formula`), `CellNode` (sheet binding, format preset, payload, audit metadata), and `CellGrid` (`IndexMap`-backed deterministic container with indexed lookups).
  - `finx::compiler::ir::symbol`: `SymbolTable` and `TableMetadata` managing workbook-wide table ownership, sheet placement, column/row coordinate indexing, and group catalogs.
  - `finx::compiler::ir::expand`: Grid expansion algorithms (`expand_table`, `expand_grid`, `expand_workbook`) unrolling table specifications into discrete cell nodes.
  - `finx::compiler::ir::error`: Structured domain errors (`IrError`).
  - `finx::model::id`: Strongly typed identifier wrappers including `SheetId`.
- **Data flow & interfaces**:
  1. `TableSpec` $\rightarrow$ `SymbolTable::register_table(sheet_id, &table)` $\rightarrow$ Validates global table uniqueness and indexes column/row positions.
  2. `TableSpec` + `SymbolTable` $\rightarrow$ `expand_table(&symbol_table, &table)` $\rightarrow$ Unrolls literal, formula, or match arm rows into `Vec<CellNode>`.
  3. `expand_grid(&symbol_table, &tables)` $\rightarrow$ Populates `CellGrid` preserving declaration order (`table -> row -> col`).
  4. `ProjectManifest` + `&[TableSpec]` $\rightarrow$ `expand_workbook(&manifest, tables)` $\rightarrow$ Synchronizes symbol registration and grid expansion.
  5. `CellGrid` and `SymbolTable` are consumed by downstream compiler passes (`compiler::graph` for dependency DAG construction and `compiler::eval` for runtime evaluation).
- **Dependencies**:
  - `indexmap`: Guarantees deterministic iteration ordering for reproducible compilation.
  - `thiserror`: Structured diagnostic domain error hierarchy.
  - `serde`: Serialization and deserialization of IR nodes.

## Requirements & Invariants

### 1. Canonical Cell Addressing

Every discrete cell in the model is identified by `CellId(TableId, RowId, ColId)`. Its `Display` implementation strictly produces `{table}["{row}", "{col}"]` matching the DSL reference addressing syntax.

#### Scenarios

- **Valid cell display**:
  - **When** formatting `CellId::new("IncomeStatement", "revenue", "2024E")`
  - **Then** output is `IncomeStatement["revenue", "2024E"]`.
- **Deterministic ordering and hash**:
  - **When** inserting `CellId` keys into maps or sorting them
  - **Then** ordering and hashing evaluate strictly on `(table, row, col)`.

### 2. Cell Collection & Duplicate Rejection

`CellGrid` maintains cell nodes in deterministic insertion order and rejects duplicate insertions of the same `CellId`.

#### Scenarios

- **Single insertion and retrieval**:
  - **When** inserting a cell node into an empty `CellGrid`
  - **Then** `grid.get(&id)` returns the node and `grid.len()` increments.
- **Duplicate cell rejection**:
  - **When** attempting to insert a second cell with an existing `CellId`
  - **Then** returns `IrError::DuplicateCell`.
- **Filtered queries and keys**:
  - **When** invoking `cells_for_table`, `cells_for_row`, `cells_for_col`, or `keys`
  - **Then** yields matching cell nodes or cell IDs in original insertion order.

### 3. Workbook Table Ownership & Coordinate Indexing

Each table in a workbook must belong to exactly one sheet. The `SymbolTable` enforces global table uniqueness and maintains 0-based column and row indices.

#### Scenarios

- **Valid registration**:
  - **When** registering a valid `TableSpec` under a `SheetId`
  - **Then** `get_sheet(&table_id)` resolves the sheet, and `col_index` / `row_index` return 0-based coordinates.
- **Duplicate table rejection**:
  - **When** registering a table whose `TableId` already exists on any sheet
  - **Then** returns `IrError::DuplicateTable` detailing both sheet assignments.
- **Group slices resolution**:
  - **When** querying `cols_in_group` or `rows_in_group`
  - **Then** returns the ordered slice of column or row IDs belonging to that section.
- **Hierarchical row flattening**:
  - **When** querying `rows_in_group` for a parent group containing nested sub-groups
  - **Then** returns all flattened descendant row IDs in declaration order.
- **Empty table rejection**:
  - **When** registering a table with no columns
  - **Then** returns `IrError::EmptyTable`.
- **Sheet catalog iteration**:
  - **When** invoking `sheet_ids`
  - **Then** yields all registered sheet identifiers in registration order.

### 4. Grid Expansion & Row Unrolling

`expand_table`, `expand_grid`, and `expand_workbook` expand high-level row definitions across declared table columns into discrete `CellNode` calculation nodes within a deterministic `CellGrid`.

#### Scenarios

- **Literal row unrolling**:
  - **When** expanding a row with literal values across $N$ columns
  - **Then** emits $N$ cells with `CellPayload::Literal` containing corresponding values.
- **Uniform formula row expansion**:
  - **When** expanding a row with a formula string across $N$ columns
  - **Then** parses the formula expression once and assigns identical `CellPayload::Formula(Expr)` across all $N$ cells.
- **Partitioned match arm unrolling**:
  - **When** expanding a row partitioned by match arms (`single`, `range`, or `group`)
  - **Then** resolves target columns and emits cells in strict `table.columns` order with appropriate literal or formula payloads.
- **Metadata and format preset propagation**:
  - **When** expanding rows with formatting presets, notes, and provenance
  - **Then** all cells inherit row-level format presets, while arm notes and sources override row-level metadata if present.
- **Actionable formula parse errors**:
  - **When** expanding a row containing invalid formula syntax
  - **Then** returns `IrError::FormulaParse` identifying the table, row, and exact parser failure.
- **End-to-end workbook orchestration**:
  - **When** invoking `expand_workbook(manifest, tables)`
  - **Then** registers all tables on their declared sheets and expands all tables into a synchronized `CellGrid`.
