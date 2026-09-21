---
title: Multi-Level Column Headers, Column Gutters & Elimination of Column Groups
date: 2026-09-21
status: draft
target-living-spec:
  - specs/living-specs/model/spec-ast.md
  - specs/living-specs/expr/formula-parser.md
  - specs/living-specs/compiler/spatial-layout.md
  - specs/living-specs/compiler/workbook-emission.md
---

## Proposal

### What

1. **Eliminate `ColumnGroup` hierarchy**: Remove `ColumnGroup`, `ColGroupId`, and associated contiguity and overlapping validators from the AST and model.
2. **Explicit typed `ColumnSpec` enum**: Replace the flat `ColumnSpec` struct with a type-safe tagged enum:
   ```rust
   pub enum ColumnSpec {
       Data(DataColumnSpec),
       Gutter,
   }
   ```
   - Only `DataColumnSpec` possesses a `ColId` and participates in `CellGrid` calculations.
   - `ColumnSpec::Gutter` is a unit variant with zero fields and no `ColId`. It emits zero `CellNode`s into `CellGrid`.
3. **Explicit multi-level column headers**:
   - `DataColumnSpec.headers: Vec<String>` supports arbitrary explicit descriptor header tiers (e.g. `["Historical Actuals", "2023A"]`). Empty headers (`headers.is_empty()`) default to `[col.id.as_str()]` at tier $N-1$ to preserve default display labeling.
   - Add builder methods on `DataColumnSpec` and `ColumnSpec` (`.header(&str)`, `.headers(iter)`, `.display_label()`).
4. **Descriptor axis duality**:
   - Maintain `pub role: AxisRole` (`Measure` vs `Descriptor`) strictly on `DataColumnSpec` (purged from rows).
   - Rows describe themselves horizontally via descriptor columns on the left (`AxisRole::Descriptor`).
   - Columns describe themselves vertically via descriptor headers at the top (`headers: Vec<String>`).
5. **Multi-tier spatial header layout & bottom alignment**:
   - Compute `max_header_levels` across all `Data` columns ($N = \max(1, \max_{c \in \text{Data}}(\text{headers.len()}))$.
   - Allocate $N$ physical header rows above the table body region.
   - For `Data` column $c$ with $K = c.\text{headers.len()}$:
     - If $K = 0$, falls back to `[col.id]` placed at tier $N-1$.
     - If $K > 0$, header item $k$ ($0 \le k < K$) is placed at tier index $\text{tier}(k) = (N - K) + k$.
     - Any tier $t < N - K$ is an unfilled upper tier (emitted blank with no header fill or borders).
   - For `Gutter` columns: all $N$ header rows remain completely blank with no fill or borders.
6. **1:1 Spatial column mapping & explicit column gutters**:
   - `LayoutPlanner` maps `table.columns` strictly 1:1 to physical grid columns ($col_{\text{phys}} = col_{\text{start}} + \text{index}$).
   - `ColumnSpec::Gutter` columns receive `theme.metrics.gutter_col_width` (3.0 chars) uniformly, with blank content across all header tiers and body rows.
   - `DataColumnSpec` columns receive their configured width (`min_col_width`, descriptor width, or metric width).
   - Phantom column synthesis is completely eliminated.
7. **Visual `CenterAcross` multi-tier header emission**:
   - In the core data model, `DataColumnSpec.headers` explicitly defines the full path of headers for each column (e.g. `["Forecast", "2024E"]` and `["Forecast", "2025E"]`).
   - During the workbook emission phase (pure presentation), super-header tiers ($t < N-1$) visually center across contiguous data columns sharing identical labels using Excel's FAST-compliant `CenterAcross` alignment (`FormatAlignHorizontal::CenterAcross`), writing the string at the first column of the run and emitting blank cells with matching header formatting across the remainder of the span.
   - Contiguous super-header runs naturally break at `ColumnSpec::Gutter` boundaries.
   - Bottom-tier metric headers ($t = N-1$) are always emitted directly cell-by-cell.
8. **Unified axis offset navigation & descriptor boundary protection**:
   - Eliminate `SpecialCol` and `SpecialRow`, unifying relative axis navigation into a single `RelativeOffset` enum (`Prev(u32)`, `Next(u32)`) in `RowSelector::Offset` and `ColSelector::Offset`. Group selectors (`cols.<group>`, `rows.<group>`) are removed.
   - Temporal column offsets (`col.prev`, `col.next`) navigate strictly across calculable measure columns (`AxisRole::Measure`), bypassing `ColumnSpec::Gutter` and preventing relative navigation from crossing the descriptor boundary into non-measure label columns (e.g. `col.prev(1)` on the first measure column returns `ResolveError::TemporalOffsetOutOfBounds`).
9. **Minimal table placement bounding box**: Replace `col_header_row` and `super_header_rows` in `TablePlacement` with `pub header_row_range: (u32, u32)` and rename `data_row_range` to `pub body_row_range: (u32, u32)`, both representing `(start_row, end_row)` **inclusive**.

### Why

- **Descriptor coordinate symmetry**: Rows describe themselves horizontally via descriptor columns on the left (`row.cells[desc_col]`); columns describe themselves vertically via descriptor rows (header tiers) at the top (`col.headers`).
- **Data purity and presentation clarity**: The core AST and IR remain 100% self-describing (`headers: Vec<String>`) without artificial group entities. In presentation emission, contiguous identical super-headers use `CenterAcross` to visually span columns without merged cells, adhering to institutional FAST modeling standards.
- **Symmetric spacing with zero dummy IDs**: `ColumnSpec::Gutter` introduces explicit vertical spacer columns (3.0 chars) between periods (actuals vs forecast, quarters vs full year) without requiring dummy IDs or polluting `CellGrid` with sparse empty nodes.
- **Local column specification**: Each column explicitly and locally declares its full hierarchy of headers without needing separate sidecar group declarations.
- **Single source of truth**: Removes redundant `ColumnGroup` validation logic and synchronizes column definitions into a single list in `TableSpec.columns`.
- **Unified axis navigation**: Relative navigation (`row.prev`, `col.prev`) is structurally identical across both axes. Unifying into `RelativeOffset` removes duplicate enums and simplifies expression resolution.
- **Protected temporal navigation**: Confines `col.prev` and `col.next` strictly to measure columns, bypassing gutters and preventing calculation formulas from accidentally referencing non-measure descriptor columns.

### Scope

- **In-scope**:
  - Remove `ColumnGroup` from `src/model/table.rs` and `ColGroupId` from `src/model/id.rs`.
  - Define `ColumnSpec` enum (`Data(DataColumnSpec)`, `Gutter`) and `DataColumnSpec` struct in `src/model/table.rs`.
  - Retain `pub role: AxisRole` solely on `DataColumnSpec` to delineate calculation columns from descriptor columns.
  - Remove column group validation methods and errors from `src/model/table.rs` and `src/model/error.rs`.
  - Replace `SpecialCol` and `SpecialRow` with `RelativeOffset` in `src/expr/ast.rs` and update `src/expr/parser.rs`.
  - Update `expand_table` in `src/compiler/ir/expand.rs` to skip `ColumnSpec::Gutter`.
  - Remove `column_groups` from `SymbolTable` and group resolution in `src/compiler/graph/resolve.rs`.
  - Enforce measure-column boundary in `src/compiler/graph/resolve.rs` for `col.prev` and `col.next`, bypassing `ColumnSpec::Gutter`.
  - Replace `SuperHeaderPlacement` in `src/compiler/layout/mod.rs` with multi-tier header planning; update `TablePlacement` to use inclusive `header_row_range: (u32, u32)` and `body_row_range: (u32, u32)`.
  - Assign `theme.metrics.gutter_col_width` (3.0 chars) to `ColumnSpec::Gutter` in `LayoutPlanner`.
  - Update `WorkbookEmitter` to render super-header tiers with `CenterAcross` spans, bottom-tier headers cell-by-cell, leave unfilled higher tiers unstyled, and emit gutter columns as blank unstyled cells across all rows.
  - Update all test suites and model fixtures.

- **Out-of-scope**:
  - DSL / YAML desugaring macros (to be addressed in a future frontend tooling spec).

### What Changes

- **Affected modules**:
  - `src/model/id.rs`: Remove `ColGroupId`.
  - `src/model/table.rs`: Replace `ColumnSpec` struct with `ColumnSpec` enum (`Data(DataColumnSpec)`, `Gutter`) and `DataColumnSpec` struct. Remove `ColumnGroup`, `column_groups` from `TableSpec`.
  - `src/model/error.rs`: Remove `NonContiguousColumnGroup`, `OverlappingColumnGroup`, `DuplicateColumnGroupId`.
  - `src/expr/ast.rs` & `src/expr/parser.rs`: Replace `SpecialCol` and `SpecialRow` with unified `RelativeOffset`.
  - `src/compiler/ir/expand.rs`: Match on `ColumnSpec` variants in `expand_table`; skip `ColumnSpec::Gutter`.
  - `src/compiler/ir/symbol.rs`: Remove `column_groups` from `SymbolTable`; populate `SymbolTable.columns` strictly with `DataColumnSpec` IDs.
  - `src/compiler/graph/error.rs`: Remove `UnknownColumnGroup`.
  - `src/compiler/graph/resolve.rs`: Resolve relative offsets via `RelativeOffset` over measure columns strictly; error on descriptor boundary crossing; remove column group branches.
  - `src/compiler/layout/mod.rs`: Replace `SuperHeaderPlacement` with multi-tier header planning; assign `theme.metrics.gutter_col_width` to gutter columns; replace `col_header_row` and `data_row_range` with inclusive `header_row_range` and `body_row_range` in `TablePlacement`.
  - `src/compiler/emission/workbook.rs`: Render super-headers with `CenterAcross` across contiguous runs of identical labels without `<mergeCells>`; render bottom-tier metric headers cell-by-cell; stream gutter columns as blank cells.
  - `tests/`: Update fixtures and test suites.

### Capabilities

- **New capabilities**:
  - Multi-tier column headers of arbitrary depth ($N$ levels).
  - Explicit header paths with self-describing column definitions in core IR.
  - Explicit column gutters (`ColumnSpec::Gutter`) with `theme.metrics.gutter_col_width` (3.0 chars) for period separation.
  - Visual `CenterAcross` super-header presentation adhering to FAST financial modeling standards.
  - Protected temporal column navigation guarding descriptor boundaries.
- **Modified capabilities**:
  - Column headers are declared via `headers: Vec<String>` on `DataColumnSpec`.
  - Formula ranges across columns use native slice ranges (`"2023".."2025"`).
  - Relative formula offsets use unified `RelativeOffset` (`row.prev(k)`, `col.prev(k)`).

---

## Design

### Goals

1. **Orthogonal column schema**: Keep `table.columns` as the sole source of column geometric and header truth.
2. **Deterministic layout offsets**: Table header height is deterministic ($N$ header rows where $N = \max(1, \max_{c \in \text{Data}}(\text{headers.len()}))$.
3. **Data purity**: Every header cell explicitly contains its display label; zero phantom blank cells or merge traps.
4. **Canonical axis navigation**: Unified representation for relative offsets on both axes.

### Invariants

- **Bottom alignment indexing**: For a table with $N = \max(1, \max_{c \in \text{Data}}(\text{col.headers.len()}))$, `Data` column $c$ with $K = c.\text{headers.len()}$ maps header $k$ ($0 \le k < K$) to physical tier $(N - K) + k$. Any tier $t < N - K$ is an unfilled upper tier.
- **Empty headers fallback**: A `Data` column with `headers.is_empty()` defaults to a single bottom tier ($K = 1$) displaying `col.id.as_str()` at tier $N-1$, preserving default header naming.
- **Column gutter dimensions and blank emission**: A `ColumnSpec::Gutter` is planned and emitted with `theme.metrics.gutter_col_width` (3.0 chars), emits zero `CellNode`s into `CellGrid`, and produces blank unstyled cells across all header tiers and body rows without borders or fills.
- **Column gutter formula exclusion**: `ColumnSpec::Gutter` has no `ColId`. Relative offsets (`col.prev`, `col.next`) bypass column gutters completely. Slice ranges (`"start".."end"`) require valid `ColId`s, while the lowered Excel range physically encloses any intervening column gutters.
- **Visual CenterAcross super-header emission**: For super-header tiers ($t < N - 1$), contiguous columns sharing identical labels are emitted with `CenterAcross` alignment (label written at first column of the run, followed by blank cells with matching header formatting). Bottom-tier metric headers ($t = N - 1$) are emitted cell-by-cell. Contiguous runs break at `ColumnSpec::Gutter` columns.
- **Measure-axis temporal boundary**: `ColSelector::Offset(RelativeOffset::Prev(k))` and `Next(k)` navigate strictly across the ordered sequence of measure columns (`AxisRole::Measure`), matching how `row.prev` navigates only `DataRowSpec` IDs. Crossing the descriptor boundary or indexing before the first measure column produces `ResolveError::TemporalOffsetOutOfBounds`.
- **Inclusive header and body row ranges**: `TablePlacement.header_row_range` is `(start_row, end_row)` inclusive for header tiers, and `TablePlacement.body_row_range` is `(start_row, end_row)` inclusive for all table body rows (`Data`, `SectionHeader`, `Gutter`).
- **Unstyled blank tiers**: Unfilled upper tiers above descriptor or short columns are emitted blank with no header fill or borders.

### Decisions

- **Visual `CenterAcross` Emission for Super-Headers (Zero Merged Cells, Explicit Core IR)**:
  - *Choice*: In the core IR, each column explicitly declares its full header vector (`headers: Vec<String>`). During OpenXML workbook emission, identical contiguous super-header labels are rendered via `CenterAcross` (horizontal alignment across selection).
  - *Rationale*: Preserves 100% explicit local declaration in the core IR without group entities, while producing clean, professional financial models in Excel that adhere to FAST standards (no `<mergeCells>`) and avoid visually repetitive header blocks.
  - *Alternatives considered*: Literal string repetition in every Excel cell (rejected for presentation because adjacent identical labels clutter visual review in Excel); hard merged cells `<mergeCells>` (rejected because it breaks Excel keyboard navigation and formula drag-copy).

- **Descriptor Boundary Protection for Column Offsets**:
  - *Choice*: `col.prev` and `col.next` navigate the sequence of measure columns (`AxisRole::Measure`), ignoring descriptor columns.
  - *Rationale*: In financial modeling, relative column navigation (`col.prev`) is temporal (prior month/quarter/year). Descriptor columns contain string metadata (e.g. account name, code), not numerical calculation vectors. Attempting to navigate into a descriptor column is an out-of-bounds domain error, not a valid cell reference.
  - *Alternatives considered*: Navigating all columns including descriptors. Rejected because it produces invalid dependencies and runtime type/evaluation failures when a formula attempts to calculate against text labels.

- **Tagged Enum for `ColumnSpec` & Explicit Column Gutters**:
  - *Choice*: `pub enum ColumnSpec { Data(DataColumnSpec), Gutter }`.
  - *Rationale*: Symmetrically mirrors `RowSpec::Gutter`. Provides clean, explicit horizontal spacing between period blocks (e.g. between historical actuals and forecast periods, or quarterly schedules and full-year totals) without dummy IDs (e.g. `col_gutter_1`) or sparse missing cells in `CellGrid`. Emits zero nodes into the computation graph.
  - *Alternatives considered*: Flat struct with `AxisRole::Gutter` (rejected because it forces dummy ColIds on spacing columns and pollutes the identifier namespace); hardcoding layout heuristics (rejected because spacing must be explicitly author-controlled).

- **Inclusive Bounds for `TablePlacement`**:
  - *Choice*: `header_row_range: (u32, u32)` and `body_row_range: (u32, u32)` are inclusive `(start_row, end_row)`.
  - *Rationale*: Uniformity and consistency across header tiers and table body rows.

- **Local `headers: Vec<String>` Representation**:
  - *Choice*: Add `pub headers: Vec<String>` directly to `DataColumnSpec`.
  - *Rationale*: Eliminates `ColumnGroup` entity and separate validators; synchronizes column metadata into a single array.

- **Unified `RelativeOffset` for Axes**:
  - *Choice*: Replace `SpecialRow` and `SpecialCol` with `RelativeOffset::Prev(u32)` and `Next(u32)`.
  - *Rationale*: Relative axis navigation is identical across horizontal and vertical dimensions.

### Data Contracts

#### Updated `ColumnSpec` and `DataColumnSpec`
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColumnSpec {
    Data(DataColumnSpec),
    Gutter,
}

impl ColumnSpec {
    pub fn id(&self) -> Option<&ColId> {
        match self {
            Self::Data(d) => Some(&d.id),
            Self::Gutter => None,
        }
    }

    pub fn role(&self) -> Option<AxisRole> {
        match self {
            Self::Data(d) => Some(d.role),
            Self::Gutter => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataColumnSpec {
    pub id: ColId,
    #[serde(default)]
    pub role: AxisRole,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<SemanticStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<FormatPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<CurrencyCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
}

impl DataColumnSpec {
    pub fn display_label(&self) -> &str {
        self.headers.last().map(|s| s.as_str()).unwrap_or(self.id.as_str())
    }
}
```

#### Unified `RelativeOffset` and Selectors
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelativeOffset {
    Prev(u32),
    Next(u32),
}

pub enum RowSelector {
    Active,
    Key(RowId),
    Range(RowId, RowId),
    Offset(RelativeOffset),
}

pub enum ColSelector {
    Active,
    Key(ColId),
    Range(ColId, ColId),
    Offset(RelativeOffset),
}
```

#### Updated `TablePlacement`
```rust
pub struct TablePlacement {
    pub sheet: SheetId,
    pub start_col: u32,
    pub start_row: u32,
    pub end_col: u32,
    pub end_row: u32,
    pub title_row: Option<u32>,
    pub subtitle_row: Option<u32>,
    pub title_gutter_row: Option<u32>,
    /// Inclusive physical row range of header tiers: `(start_row, end_row)`.
    pub header_row_range: (u32, u32),
    /// Inclusive physical row range of table body rows (data, section headers, gutters): `(start_row, end_row)`.
    pub body_row_range: (u32, u32),
}
```

### Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Multi-tier column header layout & visual CenterAcross emission**
  - **Given** columns with `headers: ["Historical", "2023"]` and `headers: ["Historical", "2024"]`
  - **When** spatial layout and emission execute
  - **Then** exactly 2 header rows are allocated; at tier 0, physical column 0 receives `"Historical"` with `CenterAcross` format and column 1 receives a blank cell with matching header format, centering the label across the selection without merged cells (`<mergeCells>`); at tier 1, column 0 receives `"2023"` and column 1 receives `"2024"` directly.

- [ ] **Scenario 2: Unequal header tier depths (bottom alignment)**
  - **Given** descriptor column with `headers: ["Line Item"]` (1 tier) and measure columns with `headers: ["Actuals", "2023"]` (2 tiers)
  - **When** spatial layout plans the table
  - **Then** 2 header rows are allocated, and `"Line Item"` is placed at tier 1 (immediately above the data rows) with tier 0 left blank and unstyled.

- [ ] **Scenario 3: Unified relative offset & descriptor boundary protection**
  - **Given** a table with descriptor column `line_item` and measure columns `2023` and `2024`
  - **When** an expression on `2024` uses `col.prev`
  - **Then** it parses into `RelativeOffset::Prev(1)` and resolves to `2023`.
  - **When** an expression on `2023` uses `col.prev`
  - **Then** dependency resolution rejects it with `ResolveError::TemporalOffsetOutOfBounds` (preventing reference to descriptor column `line_item`).

- [ ] **Scenario 4: Elimination of group selectors in expressions**
  - **Given** an expression using slice range `IncomeStatement[@row, "2023".."2025"]`
  - **When** formula parsing, dependency resolution, and lowering execute
  - **Then** the range resolves cleanly without reference to any `ColGroupId`.

- [ ] **Scenario 5: Default column ID header fallback**
  - **Given** a `Data` column with `headers: []` (e.g. constructed via `ColumnSpec::new("2023")`) in a table with $N=2$ header tiers
  - **When** spatial layout and workbook emission execute
  - **Then** the column displays `"2023"` at tier 1 (immediately above data rows) with tier 0 left blank and unstyled.

- [ ] **Scenario 6: Explicit column gutter & formula bypass**
  - **Given** columns `Data("2023")`, `Gutter`, and `Data("2024")` with expression `col.prev` on `2024`
  - **When** grid expansion, spatial layout, dependency resolution, and emission execute
  - **Then** zero calculation cells are emitted for the gutter column into `CellGrid`; the physical gutter column is sized to `theme.metrics.gutter_col_width` (3.0 chars); header cells and data cells on that column remain completely blank without borders; `col.prev` on `2024` resolves directly to `2023` (bypassing the gutter column).

---

## Tasks

### Phase 1: Model & AST updates
- [ ] Define `ColumnSpec` enum (`Data(DataColumnSpec)`, `Gutter`) and `DataColumnSpec` struct in `src/model/table.rs`.
- [ ] Ensure `AxisRole` is preserved on `DataColumnSpec`.
- [ ] Remove `ColumnGroup` and `column_groups` from `TableSpec` in `src/model/table.rs`.
- [ ] Remove `ColGroupId` from `src/model/id.rs`.
- [ ] Remove column group validation methods and errors from `src/model/table.rs` and `src/model/error.rs`.
- [ ] Unify `SpecialRow` and `SpecialCol` into `RelativeOffset` in `src/expr/ast.rs`.
- [ ] Update `RowSelector` and `ColSelector` to use `Offset(RelativeOffset)`.

### Phase 2: Expression parser & symbol resolution
- [ ] Update formula parser in `src/expr/parser.rs` to parse `row.prev/next` and `col.prev/next` into `RelativeOffset`.
- [ ] Update `expand_table` in `src/compiler/ir/expand.rs` to skip `ColumnSpec::Gutter`.
- [ ] Populate `SymbolTable.columns` strictly with `DataColumnSpec` IDs.
- [ ] Remove `column_groups` symbol table and lookup methods from `src/compiler/ir/symbol.rs`.
- [ ] Remove `UnknownColumnGroup` from `src/compiler/graph/error.rs`.
- [ ] Update `src/compiler/graph/resolve.rs` to handle `RelativeOffset` for rows and columns, enforcing measure-column boundaries for column offsets, and remove group resolution branches.

### Phase 3: Spatial layout planner refactoring
- [ ] Update `LayoutPlanner` to calculate `max_header_levels` ($N \ge 1$) from `Data` columns.
- [ ] Allocate $N$ header rows for multi-tier column headers using the bottom-alignment indexing formula $\text{tier}(k) = (N - K) + k$.
- [ ] Assign `theme.metrics.gutter_col_width` (3.0 chars) to `ColumnSpec::Gutter` in `SheetLayout.col_widths`.
- [ ] Remove `SuperHeaderPlacement` and replace `col_header_row` and `data_row_range` with inclusive `header_row_range: (u32, u32)` and `body_row_range: (u32, u32)` in `TablePlacement`.

### Phase 4: Workbook emission & formatting
- [ ] Update `WorkbookEmitter` to render super-header tiers with `CenterAcross` across contiguous runs of identical labels, render bottom-tier metric headers cell-by-cell, leave upper unfilled tiers unstyled, and stream gutter columns as blank cells without borders.

### Phase 5: Test fixtures & verification
- [ ] Update model fixtures (`tests/fixtures/three_statement.rs`, `lbo.rs`, `styling_showcase.rs`).
- [ ] Update test suites across `tests/`.
- [ ] Verify full test suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- [ ] Reconcile living specs and archive this spec to `specs/history/`.
