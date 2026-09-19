---
title: Cell Intermediate Representation & Symbol Table
last_updated: 2026-09-19
domain: compiler
status: active
---

## Purpose

The `compiler::ir` subsystem provides discrete cell-level Intermediate Representation (IR) data structures, the canonical cell identity model, deterministic cell collection containers, and the workbook symbol table. It unrolls high-level 2D table specifications through the 3-Tier Specificity Lattice into fine-grained calculation nodes, preventing false-positive circular dependencies in inter-period roll-forwards and providing the node matrix for dependency analysis, topological evaluation, and coordinate lowering.

## Architecture & Boundaries

- **Modules & components**:
  - `finx::compiler::ir::id`: Canonical `CellId` representing the 3-tuple `(TableId, RowId, ColId)`.
  - `finx::compiler::ir::cell`: `CellPayload` (`Literal` vs `Formula`), `CellNode` (sheet binding, format preset, fractional decimals, orthogonal 2D structural styles `(horizontal, vertical)`, visual highlight emphasis, payload, audit metadata), and `CellGrid` (`IndexMap`-backed deterministic container with indexed lookups).
  - `finx::compiler::ir::symbol`: `SymbolTable` and `TableMetadata` managing workbook-wide table ownership, sheet placement, column/row coordinate indexing, axis roles, and group catalogs.
  - `finx::compiler::ir::expand`: Grid expansion engine implementing the 3-Tier Specificity Lattice (`expand_table`, `expand_grid`, `expand_workbook`).
  - `finx::compiler::ir::error`: Structured domain errors (`IrError`).
  - `finx::model::id`: Strongly typed identifier wrappers including `SheetId`.
- **Data flow & interfaces**:
  1. `TableSpec` $\rightarrow$ `SymbolTable::register_table(sheet_id, &table)` $\rightarrow$ Validates global table uniqueness, indexes column/row positions, and stores axis roles.
  2. `TableSpec` + `SymbolTable` $\rightarrow$ `expand_table(&symbol_table, &table)` $\rightarrow$ Evaluates the 3-Tier Specificity Lattice for each $(r, c)$ coordinate pair into `Vec<CellNode>`.
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

Each table in a workbook must belong to exactly one sheet. The `SymbolTable` enforces global table uniqueness, maintains 0-based column and row indices, and exposes axis roles.

#### Scenarios

- **Valid registration**:
  - **When** registering a valid `TableSpec` under a `SheetId`
  - **Then** `get_sheet(&table_id)` resolves the sheet, and `col_index` / `row_index` / `col_role` / `row_role` return metadata.
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

### 4. 3-Tier Specificity Lattice Grid Expansion

`expand_table` iterates over all declared rows and columns applying the 3-Tier Specificity Lattice:
$$\text{Cell Override (0D Point)} > (\text{Row Rule } \mathbf{XOR} \text{ Column Rule}) > \text{Explicit Null}$$

#### Scenarios

- **0D Point Cell Override**:
  - **When** a cell coordinate $(r, c)$ has an entry in `row.cells`
  - **Then** that payload (literal or point formula) and optional format/note override takes precedence.
- **Descriptor Axis Isolation**:
  - **When** row $r$ or column $c$ has `role: "descriptor"`
  - **Then** vector rules never broadcast across it; unassigned descriptor cells halt with `IrError::UnassignedCell`.
- **Ambiguous Expression Conflict**:
  - **When** row $r$ defines `row.expr` and column $c$ defines `col.expr` without a cell override at their intersection
  - **Then** compilation halts with `IrError::AmbiguousExpressionConflict`.
- **Unassigned Measure Cell**:
  - **When** a measure-on-measure coordinate has no cell override, no row expression, and no column expression
  - **Then** compilation halts with `IrError::UnassignedCell`.
- **Format Conflict**:
  - **When** measure row $r$ and measure column $c$ specify conflicting format presets without a cell format override
  - **Then** compilation halts with `IrError::FormatConflict`.
- **Symmetrical 2D Border Composition**:
  - **When** row $r$ specifies `style: "total"` and column $c$ specifies `style: "total"`
  - **Then** the cell at $(r, c)$ resolves `horizontal == SemanticStyle::Total` and `vertical == SemanticStyle::Total`, forming a complete accounting box (`is_bold == true`).
- **Cell Style Reset**:
  - **When** cell $(r, c)$ specifies `style: "data"`
  - **Then** summary rules are unconditionally suppressed, resolving `(Data, Data)`.
- **Decimal Precision Override & Vector Conflict**:
  - **When** cell $(r, c)$ specifies `decimals: Some(d)`
  - **Then** point precision takes precedence over row and column decimals.
  - **When** measure row $r$ and measure column $c$ specify unequal decimals without a point override
  - **Then** compilation halts with `IrError::DecimalsConflict`.
- **Highlight Resolution & Inheritance**:
  - **When** a row or column specifies `highlight: true`
  - **Then** intersecting cells inherit `is_highlight == true`, unless explicitly overridden by `highlight: false` on the point override.

