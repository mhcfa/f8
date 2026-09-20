---
title: Spatial Layout Planning & 2D Grid Coordinate Assignment
date: 2026-09-20
status: completed
target-living-spec: specs/living-specs/compiler/spatial-layout.md
---

## Proposal

### What

Implement Finx's Pass 5 Spatial Layout Engine in `finx::compiler::layout`:
1. Define the deterministic output contract types in `src/compiler/layout/`: `LayoutMap`, `PhysicalCoordinate`, `StyleRole`, `TablePlacement`, `SuperHeaderPlacement`, `SheetLayout`, and `SectionHeaderRow`.
2. Implement sheet-level spatial geometry planning:
   - Margins and gutters: Row 0 top gutter (`gutter_row_height`), Column 0 / A left gutter (`gutter_col_width`), and primary table placement origin at cell `B2` (`(row: 1, col: 1)`).
   - Multi-table vertical stacking on worksheets separated by inter-table gutters (default 2 blank rows sized to `gutter_row_height`).
   - Table header stacks: optional title banner, optional subtitle, title buffer gutter (`title_gutter_row_height`), super-header rows from `column_groups`, and column key header row.
   - Row hierarchy: interleaving visual section header rows from `row_groups`, computing Excel outline levels (`1..7`) for native collapsible grouping (`[+]`/`[-]`), and tracking row descriptor indentation depth.
   - Assigning physical `(col, row)` coordinates and standard Excel A1 strings (e.g. `col: 1, row: 1` $\rightarrow$ `"B2"`, `col: 26, row: 0` $\rightarrow$ `"AA1"`) for all calculation `CellId` nodes.
   - Assigning `StyleRole` tags (`horizontal`, `vertical`, `is_highlight`) to each physical cell coordinate from the IR `CellGrid`.
3. Implement typography-aware row height mapping and sheet-wide column width arbitration:
   - Constructing the row heights map for every sheet row based on theme metrics (`title_row_height`, `header_row_height`, `section_row_height`, `data_row_height`, `gutter_row_height`, `title_gutter_row_height`).
   - Computing sheet-wide column widths across stacked tables sharing columns, measuring header lengths, section labels, and formatted cell contents/evaluated values, scaled by `theme.metrics.char_width_multiplier` and `col_padding`, enforcing `min_col_width` (12.0) on measure columns.
   - Passing through worksheet properties (`freeze_panes`, `tab_color`, `show_gridlines`) from `SheetSpec`.
4. Integrate Pass 5 into the compiler pipeline as `LayoutPlanner::plan(...)` and expose high-level query APIs.
5. Provide a comprehensive unit and integration test suite verifying spatial coordinate assignment, header interleaving, outline levels, column width arbitration, and A1 notation.

### Why

Presentation layout must be completely decoupled from calculation logic and table definitions:
- **Prerequisite for Pass 6a (Formula Lowering)**: Formulas referencing relative coordinates (`@col`, `@row`, `col.prev`, `row.prev`) or ranges cannot be translated into native Excel formulas (e.g. `=C10-C11`, `=SUM($C$14:$G$14)`) without a concrete physical coordinate lookup map (`CellId` $\rightarrow$ `(Sheet, Col, Row)`).
- **Prerequisite for Pass 6b (Workbook Emission)**: Serializing `.xlsx` workbooks requires physical sheet dimensions, column widths, row heights, visual section headers, and cell formatting roles.
- **Institutional spatial elegance**: Rather than dumping tables flush against `A1`, financial models require breathing room (Column A gutter, Row 0 gutter, title buffer gutters, and inter-table spacing) to produce executive-grade presentation artifacts.
- **Multi-table stacking & sheet arbitration**: Multiple financial statements (e.g. Income Statement above Balance Sheet) sharing the same sheet must align seamlessly without column width clipping or numeric overflow (`###`).

### Scope

- **In-Scope**:
  - Spatial layout contract data structures (`LayoutMap`, `PhysicalCoordinate`, `StyleRole`, `TablePlacement`, `SuperHeaderPlacement`, `SheetLayout`, `SectionHeaderRow`) in `src/compiler/layout/mod.rs`.
  - Coordinate calculation algorithms: top/left margins, table stacking, inter-table gutters, title blocks, super-headers, column headers, and data rows.
  - Section header row interleaving from hierarchical `row_groups` and Excel outline level computation (`1..7`).
  - Coordinate to A1 string converter supporting columns $0 \dots 16,383$ and rows $0 \dots 1,048,575$.
  - Row height map generation from active `Theme` metrics.
  - Sheet-wide column width arbitration across stacked tables using text length, theme multipliers, padding, and minimum bounds.
  - Passing through `freeze_panes`, `tab_color`, and `show_gridlines` into `SheetLayout`.
  - Comprehensive unit and integration tests in `tests/spatial_layout_test.rs`.

- **Out-of-Scope**:
  - Pass 6a AST formula lowering and string emission (Phase 4a).
  - Pass 6b OpenXML binary packaging with `rust_xlsxwriter` (Phase 4b).
  - Explicit non-stacking 2D coordinate placement (`PlacementStrategy::Explicit { row, col }`) beyond vertical stacking defaults.
  - Interactive CLI / MCP layout visualization commands.

### What Changes

- **Affected modules**:
  - `src/compiler/layout/mod.rs`: Implement the complete spatial layout engine, public types, and coordinate conversion functions.
  - `src/compiler/layout/error.rs`: Layout error diagnostics (`LayoutError`) covering Excel grid bounds and outline level limits.
  - `src/compiler/error.rs`: Add `CompilerError::Layout(#[from] LayoutError)` variant.
  - `src/compiler/mod.rs`: Expose `layout` module and provide `Compiler::layout` orchestration method.
  - `tests/spatial_layout_test.rs`: New comprehensive integration test suite.

### Capabilities

- **New capabilities**:
  - The compiler can execute Pass 5 to transform an expanded `CellGrid`, `SymbolTable`, `ProjectManifest`, `Theme`, and optional `EvaluatedModel` into a complete `LayoutMap`.
  - Every `CellId` in the workbook receives its deterministic `(col, row)` integer coordinates and Excel `a1` string representation.
  - Stacked tables receive isolated bounding boxes with automatic inter-table and title buffer gutters.
  - Visual section headers are automatically interleaved before the first descendant row of each group with proper outline levels for collapsible tree views.
  - Sheet columns receive arbitrated physical widths guaranteed to prevent numeric clipping (`###`) on formatted values.
  - Every physical cell coordinate carries a `StyleRole` with resolved horizontal/vertical border styles and highlight emphasis.

- **Modified capabilities**:
  - `src/compiler/mod.rs` can coordinate passes 1 through 5 end-to-end.

---

## Design

### Goals

1. **Deterministic 2D mapping**: Identical input specifications must yield bit-for-bit identical `LayoutMap` coordinates, row heights, and column widths across all runs.
2. **Zero runtime mutation of calculation ASTs**: Layout operates purely on coordinates and presentation roles; it never modifies cell formulas, expressions, or values.
3. **Pure, allocation-conscious algorithms (Rust 2024)**: Reuse borrowed slices, pre-size index maps, and eliminate unnecessary cloning.
4. **Institutional visual standards**: Produce professional financial model formatting with standard margins (Column A gutter, Row 0 gutter), title banners, and safe column widths by default.

### Invariants

1. **Origin invariant**: On any worksheet utilizing default margins, the first table's top-left calculation cell (or title banner) begins at physical cell `B2` (`col: 1, row: 1`). Row 0 and Column 0 are reserved as empty gutter buffers.
2. **Contiguity invariant**: Every table occupies a strictly contiguous rectangular bounding box $[start\_col, end\_col] \times [start\_row, end\_row]$.
3. **Descriptor column isolation**: In section header rows, the section label occupies the table's first column ($start\_col$). All other columns across the table width are empty.
4. **Outline level bounds**: Excel outline levels must satisfy $1 \le level \le 7$. If a row group hierarchy exceeds depth 7, compilation returns `LayoutError::OutlineDepthExceeded`.
5. **Excel physical boundary invariants**: Physical coordinates must strictly stay within Excel 2007+ grid limits ($col < 16,384$, $row < 1,048,576$).
6. **Sheet-wide column width dominance**: For any column $c$ shared by multiple stacked tables, the sheet-wide width is the maximum of all table requirements for column $c$.

### Decisions

- **Decision: 0-indexed integer coordinates internally, 1-indexed for A1 strings**
  - *Rationale*: Internal data structures use 0-indexed `col: u32` and `row: u32` to align with `rust_xlsxwriter`'s zero-indexed API (`worksheet.write(row, col, ...)`). The `a1` field provides the standard human-readable 1-indexed string (`col: 0, row: 0` $\rightarrow$ `"A1"`).
  - *Alternatives considered*: 1-indexed tuples internally. Rejected because it introduces error-prone `+1` / `-1` conversions when interfacing with `rust_xlsxwriter`.

- **Decision: Derive `StyleRole` directly from `CellNode`**
  - *Rationale*: Pass 2 (`ir::expand`) already resolved horizontal borders, vertical borders, and highlight flags through the 3-tier formatting lattice. Pass 5 bundles these into `StyleRole` for downstream emission without re-evaluating formatting rules.
  - *Alternatives considered*: Re-evaluating styles from `TableSpec`. Rejected to maintain single source of truth in the IR.

- **Decision: Section headers allocate no `CellId` nodes**
  - *Rationale*: Section headers are visual presentation artifacts, not calculation nodes. They are tracked in `SheetLayout.section_header_rows` and written during binary emission, leaving the calculation DAG completely unaffected.
  - *Alternatives considered*: Creating synthetic `CellId` nodes for section headers. Rejected because it would pollute the DAG with non-calculating nodes and break acyclic validation.

- **Decision: Optional evaluation values for column width arbitration**
  - *Rationale*: Passing `Option<&EvaluatedModel>` allows Pass 5 to inspect runtime calculated numbers (e.g. `$1,450,230.00`) when determining column widths, while still functioning with fallback text/header lengths when run headless or prior to evaluation.

### Data Contracts

#### Core Data Structures

```rust
pub struct LayoutMap {
    pub cells: IndexMap<CellId, PhysicalCoordinate>,
    pub tables: IndexMap<TableId, TablePlacement>,
    pub sheets: IndexMap<SheetId, SheetLayout>,
}

pub struct PhysicalCoordinate {
    pub sheet: SheetId,
    pub col: u32,
    pub row: u32,
    pub a1: String,
    pub style_role: StyleRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleRole {
    pub horizontal: SemanticStyle,
    pub vertical: SemanticStyle,
    pub is_highlight: bool,
}

pub struct TablePlacement {
    pub sheet: SheetId,
    pub start_col: u32,
    pub start_row: u32,
    pub end_col: u32,
    pub end_row: u32,
    pub title_row: Option<u32>,
    pub subtitle_row: Option<u32>,
    pub title_gutter_row: Option<u32>,
    pub super_header_rows: Vec<SuperHeaderPlacement>,
    pub col_header_row: u32,
    pub data_row_range: (u32, u32),
}

pub struct SuperHeaderPlacement {
    pub row: u32,
    pub group_id: String,
    pub label: String,
    pub start_col: u32,
    pub end_col: u32,
}

pub struct SectionHeaderRow {
    pub row: u32,
    pub table_id: TableId,
    pub group_id: String,
    pub label: String,
    pub group_level: u8,
    pub descriptor_col: u32,
}

pub struct SheetLayout {
    pub sheet: SheetId,
    pub col_widths: IndexMap<u32, f64>,
    pub row_heights: IndexMap<u32, f64>,
    pub section_header_rows: Vec<SectionHeaderRow>,
    pub freeze_panes: Option<FreezePanes>,
    pub show_gridlines: bool,
    pub tab_color: Option<String>,
}
```

#### Diagnostic Errors (`LayoutError`)

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LayoutError {
    #[error("table '{table}' is not assigned to any sheet in the manifest")]
    UnassignedTable { table: TableId },

    #[error("table '{table}' has row group '{group}' exceeding maximum Excel outline depth of 7 (depth: {depth})")]
    OutlineDepthExceeded { table: TableId, group: String, depth: usize },

    #[error("layout placement exceeds Excel grid limit of {max} {dimension} (requested: {actual})")]
    GridLimitExceeded { dimension: &'static str, max: u32, actual: u32 },
}
```

### Coordinate Conversion Specification

The bijection between 0-indexed column integer $C$ and Excel column letters:
$$\text{col\_to\_excel\_col}(C) \implies \begin{cases} C = 0 \to \text{"A"} \\ C = 25 \to \text{"Z"} \\ C = 26 \to \text{"AA"} \\ C = 701 \to \text{"ZZ"} \\ C = 702 \to \text{"AAA"} \end{cases}$$
Cell A1 coordinate:
$$\text{coordinate\_to\_a1}(C, R) = \text{format}!("\{\}\{\}", \text{col\_to\_excel\_col}(C), R + 1)$$

### Risks & Trade-offs

- **Font metric variance across operating systems**: True rendered text width depends on OS font rasterizers (FreeType vs DirectWrite vs CoreText). Using `char_count * char_width_multiplier + col_padding` with an institutional safe minimum (`min_col_width = 12.0`) provides deterministic, cross-platform reproducibility without OS font dependency.
- **Stacked table width interference**: An extremely long line item in Table 1 widens that column for Table 2 stacked below it. This is standard Excel behavior; providing minimum widths on measure columns guarantees numeric cells are never truncated.

---

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Default sheet margins and primary table origin at B2**
  - **Given** a manifest with 1 sheet and a simple table without titles or headers
  - **When** spatial layout planning is executed
  - **Then** row 0 height is `theme.metrics.gutter_row_height` (9.0 pt)
  - **And** column 0 width is `theme.metrics.gutter_col_width` (3.0 chars)
  - **And** the table's first data cell is placed at cell `B2` (`col: 1, row: 1`).

- [x] **Scenario 2: Table header stack with title, subtitle, buffer gutter, and column headers**
  - **Given** a table with title `"Income Statement"`, subtitle `"($ in thousands)"`, and columns `["line_item", "2023A", "2024E"]`
  - **When** spatial layout planning is executed
  - **Then** row 1 is the title row (`title_row == Some(1)`), height 24.0 pt
  - **And** row 2 is the subtitle row (`subtitle_row == Some(2)`), height 15.0 pt
  - **And** row 3 is the title buffer gutter (`title_gutter_row == Some(3)`), height 8.0 pt
  - **And** row 4 is the column header row (`col_header_row == 4`), height 18.0 pt
  - **And** data rows begin at row 5.

- [x] **Scenario 3: Super-headers spanning grouped column ranges**
  - **Given** a table with columns `["line_item", "2021A", "2022A", "2023A", "2024E", "2025E"]`
  - **And** column groups `"historical"` (cols 1..3) and `"forecast"` (cols 4..5)
  - **When** spatial layout planning is executed
  - **Then** a super-header row is placed above the column header row
  - **And** `SuperHeaderPlacement` for `"historical"` spans physical columns 2..4
  - **And** `SuperHeaderPlacement` for `"forecast"` spans physical columns 5..6.

- [x] **Scenario 4: Interleaved section headers and Excel outline levels**
  - **Given** a table with row group `"opex"` (depth 1) containing row `"r_and_d"` and sub-group `"sales_marketing"` (depth 2) containing `"ad_spend"`
  - **When** spatial layout planning is executed
  - **Then** a visual section header for `"opex"` is placed before `"r_and_d"` with `group_level: 1`
  - **And** a visual section header for `"sales_marketing"` is placed before `"ad_spend"` with `group_level: 2`
  - **And** section headers do not allocate calculation `CellId` nodes
  - **And** section header row height is `theme.metrics.section_row_height` (18.0 pt).

- [x] **Scenario 5: Multi-table vertical stacking and inter-table gutters**
  - **Given** a sheet containing two tables `IncomeStatement` (10 rows) and `BalanceSheet`
  - **When** spatial layout planning is executed
  - **Then** `IncomeStatement` is placed starting at row 1
  - **And** exactly 2 blank gutter rows separate `IncomeStatement` and `BalanceSheet`
  - **And** each gutter row is assigned height `theme.metrics.gutter_row_height` (9.0 pt)
  - **And** `BalanceSheet` starts immediately after the inter-table gutter.

- [x] **Scenario 6: Sheet-wide column width arbitration across stacked tables**
  - **Given** Table 1 has descriptor label length 25 characters in column B
  - **And** Table 2 stacked below it has descriptor label length 15 characters in column B
  - **When** spatial layout planning is executed
  - **Then** physical Column B (`col: 1`) width is computed using the maximum 25 characters
  - **And** measure columns enforce `theme.metrics.min_col_width` (12.0 chars).

- [x] **Scenario 7: Full StyleRole propagation to physical coordinates**
  - **Given** a table with a total summary row and a highlight punchline cell
  - **When** spatial layout planning is executed
  - **Then** every `PhysicalCoordinate` in `layout_map.cells` has its `StyleRole` matching the IR `CellNode`
  - **And** the punchline cell has `style_role.is_highlight == true` and `style_role.horizontal == SemanticStyle::Total`.

- [x] **Scenario 8: Exact Excel A1 coordinate bijection and boundary checks**
  - **Given** column indices `0`, `1`, `25`, `26`, `27`, `701`, `702`
  - **When** converting to Excel coordinates
  - **Then** column strings are `"A"`, `"B"`, `"Z"`, `"AA"`, `"AB"`, `"ZZ"`, `"AAA"`
  - **And** row `0` converts to `"1"`, row `10` converts to `"11"`.

- [x] **Scenario 9: Rejection of outline levels exceeding maximum depth of 7**
  - **Given** a table with nested row groups with hierarchy depth of 8
  - **When** spatial layout planning is executed
  - **Then** compilation fails with `LayoutError::OutlineDepthExceeded`.

---

## Tasks

### Phase 1: Data Contracts & Error Types
- [x] Define `LayoutMap`, `PhysicalCoordinate`, `StyleRole`, `TablePlacement`, `SuperHeaderPlacement`, `SheetLayout`, and `SectionHeaderRow` in `src/compiler/layout/mod.rs`.
- [x] Define `LayoutError` enum in `src/compiler/layout/error.rs` and wire into `CompilerError` in `src/compiler/error.rs`.
- [x] Implement `col_to_excel_col(u32) -> String` and `coordinate_to_a1(u32, u32) -> String` coordinate formatting utilities with unit tests.

### Phase 2: Table Header & Section Interleaving Geometry
- [x] Implement table placement geometry: title rows, subtitle rows, title buffer gutters, super-headers, and column headers.
- [x] Implement recursive row group section header placement, outline level computation (`1..7`), and descriptor indentation.
- [x] Implement cell coordinate mapping (`CellId` $\rightarrow$ `PhysicalCoordinate`) with `StyleRole` attachment.

### Phase 3: Worksheet Stacking & Metric Arbitration
- [x] Implement sheet-level stacking logic: margin gutters (Row 0, Col 0), table origins at `B2`, and inter-table gutters (2 blank rows).
- [x] Implement row height mapping for every row on each sheet based on semantic row roles and `Theme` metrics.
- [x] Implement sheet-wide column width arbitration across stacked tables with `char_width_multiplier`, `col_padding`, and `min_col_width`.
- [x] Pass through worksheet metadata (`freeze_panes`, `tab_color`, `show_gridlines`) to `SheetLayout`.

### Phase 4: Verification & Compiler Integration
- [x] Wire `LayoutPlanner::plan` into `src/compiler/mod.rs`.
- [x] Create comprehensive integration tests in `tests/spatial_layout_test.rs` validating all acceptance scenarios.
- [x] Run verification suite (`cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`).
