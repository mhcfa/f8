---
title: Spatial Layout Planning & 2D Grid Coordinate Assignment
last_updated: 2026-09-21
domain: compiler
status: active
---

## Purpose

The `compiler::layout` subsystem provides Pass 5 of the Finx compilation pipeline, mapping high-level tables, hierarchical row groups, column groups, title/subtitle banners, and gutters onto physical 2D worksheet grid coordinates `(sheet, col, row, a1)`. It arbitrates sheet-wide column widths and row heights using active `Theme` metrics and produces the deterministic `LayoutMap` contract required for Pass 6a (Formula Lowering) and Pass 6b (Workbook Emission).

## Architecture & Boundaries

- **Modules & components**:
  - `finx::compiler::layout::a1`: Bijection between 0-indexed column integers and Excel column strings (`col_to_excel_col`), A1 coordinate formatting (`coordinate_to_a1`), and physical grid boundary constants (`MAX_EXCEL_COLUMNS = 16_384`, `MAX_EXCEL_ROWS = 1_048_576`, `MAX_OUTLINE_LEVEL = 7`).
  - `finx::compiler::layout::error`: Structured domain errors (`LayoutError`).
  - `finx::compiler::layout::mod`: Data structures (`LayoutMap`, `PhysicalCoordinate`, `StyleRole`, `TablePlacement`, `SuperHeaderPlacement`, `SectionHeaderRow`, `SheetLayout`) and planning engine (`LayoutPlanner`).
  - `finx::compiler::mod`: Compiler orchestration entry point (`Compiler::plan_layout`).
- **Data flow & interfaces**:
  1. Input: `ProjectManifest`, `&[TableSpec]`, `CellGrid`, `Theme`, and optional `&EvaluatedModel`.
  2. Geometry planning: Computes top/left gutters (Row 0, Col 0), primary table origin at `B2` (`col: 1, row: 1`), inter-table gutters (2 blank rows), title/subtitle blocks, title buffer gutters, super-headers, and column headers.
  3. Row hierarchy: Interleaves visual section headers before each group's first descendant row, tracks outline levels (`1..=7`), inserts section gutter rows above top-level section headers, and records descriptor indentation.
  4. Cell mapping: Assigns physical `(col, row)` coordinates, canonical Excel `a1` addresses, indentation level (`indent`), and `StyleRole` presentation flags to every `CellId`.
  5. Sizing arbitration: Populates sheet row heights from theme metrics and arbitrates sheet-wide column widths across stacked tables sharing columns, accounting for descriptor indentation padding and enforcing `theme.metrics.min_col_width` (12.0) on measure columns.
  6. Output: Deterministic `LayoutMap` consumed by Pass 6a (Formula Lowering) and Pass 6b (Workbook Emission).
- **Dependencies**:
  - `indexmap`: Guarantees deterministic iteration ordering for coordinate mappings.
  - `thiserror`: Diagnostic domain error hierarchy.
  - `serde`: Serialization of layout maps for headless inspection.

## Requirements & Invariants

### 1. Worksheet Margins & Primary Origin

On any worksheet with default margins, Row 0 is reserved as a top gutter (`theme.metrics.gutter_row_height`), and Column 0 / A is reserved as a left gutter (`theme.metrics.gutter_col_width`). The primary table's top-left calculation cell (or header) starts at physical cell `B2` (`col: 1, row: 1`).

#### Scenarios

- **Default margins applied**:
  - **When** planning layout for any worksheet
  - **Then** row 0 height equals `gutter_row_height` and column 0 width equals `gutter_col_width`.
- **Primary table placement**:
  - **When** the first table is placed on a worksheet
  - **Then** its bounding box starts at `col: 1, row: 1`.

### 2. Multi-Table Vertical Stacking & Inter-Table Spacing

When multiple tables are assigned to the same worksheet, they stack vertically in declared order, separated by exactly 2 blank gutter rows sized to `theme.metrics.gutter_row_height`.

#### Scenarios

- **Sequential stacking**:
  - **When** Table 1 ends at row $R$
  - **Then** rows $R+1$ and $R+2$ are allocated as blank gutter rows, and Table 2 starts at row $R+3$.

### 3. Table Header Structure

Each table allocates header rows in strict sequence:
1. Title row (optional): 1 row displaying `table.title` with height `theme.metrics.title_row_height` (24.0 pt).
2. Subtitle row (optional): 1 row displaying `table.subtitle` with height `theme.metrics.data_row_height` (15.0 pt).
3. Title buffer gutter (optional): 1 blank row separating title/subtitle from column headers with height `theme.metrics.title_gutter_row_height` (8.0 pt). Allocated whenever title or subtitle is present.
4. Super-header rows (optional): 1 row per `column_groups` tier with height `theme.metrics.header_row_height` (18.0 pt).
5. Column header row: 1 row displaying column labels with height `theme.metrics.header_row_height` (18.0 pt).

#### Scenarios

- **Full header stack**:
  - **When** a table declares title, subtitle, column groups, and columns
  - **Then** title, subtitle, title gutter, super-header, and column header rows are placed in sequence before data rows.

### 4. Hierarchical Row Groups, Section Headers, Gutters & Indentation

Visual section header rows are interleaved in preorder before the first descendant row of each `RowGroup`. Section headers consume 1 physical row, assign `group_level: 1..=7`, and allocate no calculation `CellId` nodes.

To visually distinguish sections cleanly without heuristics:
1. **Section Gutters**: When a top-level section header (`group_level == 1`) occurs after preceding table data (`current_row > data_start_row`), a blank section gutter row sized to `theme.metrics.gutter_row_height` (9.0 pt) is inserted immediately above the section header. This ensures preceding line items and subtotals stay grouped together while clearly partitioning the next section.
2. **Descriptor Indentation**: Each data row within a row group hierarchy computes its nesting depth (`0` for root/subtotal, `1` for top-level group items, `2` for sub-group items, etc.). The depth is recorded in `PhysicalCoordinate.indent` on descriptor cells. Descriptor cell formats apply this indentation (`format.set_indent(indent)`), and sub-headers apply `group_level - 1` indent. Measure columns remain strictly unindented.
3. **Width Arbitration with Indent**: Column width arbitration for descriptor columns incorporates indentation padding (`row_depth * 2` characters) into character length calculations.

#### Scenarios

- **Section Header Placement**:
  - **When** a table has row group `"opex"` containing row `"salaries"`
  - **Then** a visual section header row for `"opex"` is placed immediately preceding row `"salaries"`.
- **Section Gutter Insertion**:
  - **When** a top-level row group begins after the first data row of a table
  - **Then** a 9pt gutter row is inserted above its section header.
- **Descriptor Indentation**:
  - **When** rows belong to nested row groups
  - **Then** descriptor cells carry `indent: 1` or `indent: 2` according to their hierarchy depth, while measure cells carry `indent: 0`.
- **Outline Depth Limit**:
  - **When** row group nesting exceeds depth 7
  - **Then** layout planning returns `LayoutError::OutlineDepthExceeded`.

### 5. Sheet-Wide Column Width Arbitration

Because column widths apply across the entire worksheet in Excel, the physical width for column $c$ is computed as the maximum required width across all stacked tables covering column $c$:
$$\text{ColWidth}(c) = \max_{T \in \text{StackedTables}} \text{RequiredWidth}(T, c)$$
where:
$$\text{RequiredWidth}(T, c) = \max(\text{max\_chars} \times \text{char\_width\_multiplier} + \text{col\_padding}, \text{min\_col\_width})$$

#### Scenarios

- **Width arbitration**:
  - **When** Table 1 requires width 45.0 in Column B and Table 2 requires width 20.0 in Column B
  - **Then** Column B width resolves to 45.0 for the entire sheet.
- **Minimum column width enforcement**:
  - **When** measure column content is short (e.g. 3 chars)
  - **Then** width resolves to at least `theme.metrics.min_col_width` (12.0 chars) to prevent `###` numeric overflow.

### 6. Physical Coordinate Mapping & StyleRole Attachment

Every `CellId` in the expanded grid maps 1:1 to a `PhysicalCoordinate` containing `(sheet, col, row, a1)`, `indent`, and `StyleRole` (`horizontal`, `vertical`, `is_highlight`) derived from the IR `CellNode`.

#### Scenarios

- **Complete cell coordinate resolution**:
  - **When** querying any `CellId` from `layout_map.cells`
  - **Then** returns its exact physical worksheet, 0-indexed column, 0-indexed row, Excel A1 string, indentation level, and semantic style role.

