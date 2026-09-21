---
title: Explicit Rows & Elimination of Row Groups
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

1. **Eliminate `RowGroup` hierarchy**: Remove `RowGroup`, `RowGroupChild`, `RowGroupId`, and associated contiguity, overlapping, and cycle validators from the AST and model.
2. **Explicit typed `RowSpec` enum**: Replace the flat `RowSpec` struct with a type-safe tagged enum:
   ```rust
   pub enum RowSpec {
       Data(DataRowSpec),
       SectionHeader(SectionHeaderRowSpec),
       Gutter,
   }
   ```
3. **Purge `AxisRole` from rows**: Remove `role: AxisRole` from rows completely. `AxisRole` belongs strictly on `ColumnSpec` to demarcate calculation vectors from descriptor labels.
4. **Optional visual `indent: Option<u8>`**: Support `pub indent: Option<u8>` strictly as an optional cell-formatting attribute on both `DataRowSpec` and `SectionHeaderRowSpec`. It applies `format.set_indent(...)` directly to the table's `AxisRole::PrimaryDescriptor` cell. Secondary descriptor columns (`AxisRole::Descriptor`) and measure columns (`AxisRole::Measure`) receive `indent: 0`. It does not alter row heights, does not simulate pseudo-group levels or outline folding, and has zero effect on the calculation graph. In `TableSpec::validate()`, declaring `indent: Some(k)` where $k > 0$ on any row requires the table to declare a `PrimaryDescriptor` column (`ModelError::IndentWithoutPrimaryDescriptor`). Rows specifying `indent: None` or `indent: Some(0)` are valid on any table (including pure measure matrices without descriptor columns).
5. **Pure accounting `SemanticStyle`**: Keep `SemanticStyle` strictly for accounting typography and border treatments (`Data`, `Subtotal`, `Total`). Calculation IR expansion never inspects visual styles.
6. **Zero dummy IDs for gutters & section headers**: `RowSpec::Gutter` is a unit variant with zero fields. `SectionHeaderRowSpec` carries only visual title, optional indent, and note—it has no `RowId`. Only `DataRowSpec` possesses a `RowId`, ensuring `RowId` is 1:1 with calculable line items.
7. **Calculation-only Pass 2 grid expansion**: In `expand_table`:
   - Structural rows (`Gutter` and `SectionHeader`) emit zero `CellNode`s into `CellGrid`. `CellGrid` remains strictly the IR for calculable data cells.
   - Structural rows never create partial-row missing-cell traps in `CellGrid`, `SymbolTable`, dependency analysis, or evaluation.
   - `Data` rows execute standard 3-tier specificity expansion across all columns.
8. **1:1 Spatial layout mapping & continuous section bands**: `LayoutPlanner` maps `table.rows` strictly 1:1 to physical grid rows:
   - `SectionHeader` rows receive `theme.metrics.section_row_height` (18.0 pt) uniformly. The section header title is emitted at physical column `start_col` (the table's physical left margin) with left-alignment and optional indent format. If `theme.palette.section_header_fill` is present (not none/transparent), columns `start_col + 1 ..= end_col` are emitted as blank formatted cells using `section_header_format`, providing a continuous colored horizontal section band across the table while text naturally overflows without merged cells (`<mergeCells>`). If fill is transparent/none, subsequent columns on that row remain unwritten/empty.
   - `Gutter` rows receive `theme.metrics.gutter_row_height` (9.0 pt).
   - `Data` rows receive `theme.metrics.data_row_height` (15.0 pt).
   - Phantom row synthesis is completely eliminated ($row_{\text{phys}} = row_{\text{body\_start}} + \text{index}$).
9. **Eliminate formula group row selectors**: Remove `SpecialRow::Group*` (`Group`, `GroupFirst`, `GroupLast`) from the formula parser and symbol table resolution, relying on native slice ranges (`"opex".."depreciation"`).
10. **Data-row temporal navigation**: Relative offsets (`row.prev`, `row.next`) navigate strictly across the sequence of calculable data rows (`SymbolTable.rows`), bypassing structural rows (`Gutter`, `SectionHeader`). Bounding boxes in Excel formula lowering bridge any intervening physical rows seamlessly.

### Why

- **Eliminate phantom row synthesis**: Layout previously synthesized phantom section header and gutter rows dynamically, desynchronizing `TableSpec.rows` from physical grid rows and causing spatial coordinate drift.
- **Type-level impossibility of invalid states**: Neither `Gutter` nor `SectionHeader` can hold formulas or pollute `CellGrid` with unassigned measure cells. `SemanticStyle` stays isolated from calculation expansion logic.
- **RowId identity is strictly calculable**: Because only `DataRowSpec` has a `RowId`, every `RowId` in `TableMetadata.rows` is guaranteed to have valid cells across all columns. Range slices (e.g. `"rev".."cogs"`) never encounter missing cells during dependency analysis or evaluation.
- **Pure typographic indentation**: `indent: Option<u8>` is decoupled from geometry and row height, avoiding pseudo-hierarchy in the core IR while preserving clean string values for lookups.
- **Natural text overflow**: Emitting section header titles at `start_col` with empty subsequent cells allows text to flow naturally without cell merging, adhering to institutional financial modeling standards (FAST) and keeping compiler geometry minimal.
- **Single source of truth for vertical sequence**: Eliminates over 120 lines of recursive group validation code (checking for non-contiguous rows, overlapping ranges, and cycle detection) because row order and membership are declared strictly once in `TableSpec.rows`.
- **Clean streaming emission**: Structural rows (`SectionHeader`, `Gutter`) are styled and emitted directly during row streaming, eliminating sidecar section header buffers.

### Scope

- **In-scope**:
  - Remove `RowGroup`, `RowGroupChild` from `src/model/table.rs`.
  - Remove `RowGroupId` from `src/model/id.rs`.
  - Remove row group validation methods and errors from `src/model/table.rs` and `src/model/error.rs`.
  - Define `RowSpec` enum (`Data(DataRowSpec)`, `SectionHeader(SectionHeaderRowSpec)`, `Gutter`) in `src/model/table.rs`. `SectionHeaderRowSpec` carries title, optional indent, and note (no `RowId`).
  - Remove `AxisRole` from rows in `src/model/table.rs`.
  - Update `expand_table` in `src/compiler/ir/expand.rs` to skip `Gutter` and `SectionHeader`, expanding only `Data` rows into `CellGrid`.
  - Remove `SpecialRow::Group*` from `src/expr/ast.rs` and `src/expr/parser.rs`.
  - Remove `row_groups` and `row_roles` from `SymbolTable` in `src/compiler/ir/symbol.rs`; register exclusively `DataRowSpec` IDs.
  - Remove `UnknownRowGroup` from `src/compiler/graph/error.rs` and resolve logic from `src/compiler/graph/resolve.rs`.
  - Update `LayoutPlanner` to place `table.rows` 1:1 on the physical grid; remove `RowGroupAnalysis`, phantom section headers, and phantom gutter insertion.
  - Update `FormatCache` to format `SectionHeader` and `Gutter` rows directly without overloading `SemanticStyle`.
  - Update `WorkbookEmitter` to set explicit row heights for `SectionHeader` and `Gutter` rows, and stream structural rows directly in the row emission loop with natural text overflow.
  - Migrate all test suites and model fixtures (`three_statement.rs`, `lbo.rs`, `styling_showcase.rs`).

- **Out-of-scope**:
  - Multi-tier column headers and `ColumnGroup` elimination (addressed in Part 2).
  - Surface syntax adapters or DSL YAML desugaring macros.

### What Changes

- **Affected modules**:
  - `src/model/id.rs`: Remove `RowGroupId`.
  - `src/model/table.rs`: Replace `RowSpec` struct with `RowSpec` enum (`Data`, `SectionHeader`, `Gutter`). Remove `RowGroup`, `RowGroupChild`, `row_groups` from `TableSpec`. Remove row group validators.
  - `src/model/error.rs`: Remove `NonContiguousRowGroup`, `OverlappingRowGroup`, `CyclicRowGroup`, `DuplicateRowGroupId`.
  - `src/expr/ast.rs` & `src/expr/parser.rs`: Remove `SpecialRow::Group*`.
  - `src/compiler/ir/expand.rs`: Match on `RowSpec` variants in `expand_table`; skip `Gutter` and `SectionHeader`.
  - `src/compiler/ir/symbol.rs`: Remove `row_groups` and `row_roles` from `SymbolTable`; populate `SymbolTable.rows` strictly with `DataRowSpec` IDs.
  - `src/compiler/graph/error.rs`: Remove `UnknownRowGroup`.
  - `src/compiler/graph/resolve.rs`: Remove row group resolution branches.
  - `src/compiler/layout/mod.rs`: Remove `RowGroupAnalysis` and phantom row synthesis; map rows 1:1; assign `indent` directly from `row.indent()`.
  - `src/compiler/emission/format.rs`: Keep `SemanticStyle` pure (`Data`, `Subtotal`, `Total`).
  - `src/compiler/emission/workbook.rs`: Set explicit row heights for `SectionHeader` and `Gutter`; stream structural rows directly in the row emission loop; remove `SectionHeaderRow` rendering loop.
  - `tests/`: Update fixtures, layout tests, spec tests, and emission tests.

### Capabilities

- **New capabilities**:
  - Type-safe, zero-overhead explicit row modeling (`Data`, `SectionHeader`, `Gutter`).
  - Typographic cell indentation via optional `indent: Option<u8>` on `DataRowSpec` and `SectionHeaderRowSpec`.
  - Blank spacing gutters without dummy IDs or calculation nodes.
- **Modified capabilities**:
  - Physical row coordinates in `LayoutMap` match `table.rows` 1:1.
  - Formula ranges across row items use native slice ranges (`row_a..row_b`).

---

## Design

### Goals

1. **1:1 Row mapping**: Every item in `table.rows` maps to exactly one physical row on the worksheet.
2. **Zero row magic**: No synthesized phantom rows or invisible gutter injections during layout.
3. **Type-level safety**: Gutters and section headers have no RowIds or calculation cells; data rows have calculations.
4. **Pure calculation IR**: Structural rows produce zero `CellNode`s in `CellGrid`, keeping the computation DAG strictly calculable.

### Invariants

- **Row contiguity**: `table.rows` specifies the complete vertical sequence of the table body region.
- **Primary descriptor indentation only**: `indent` applies exclusively to the table's `AxisRole::PrimaryDescriptor` column; secondary descriptor columns (`AxisRole::Descriptor`) and measure columns (`AxisRole::Measure`) receive `indent: 0`. If any row declares `indent: Some(k)` where $k > 0$ but the table does not declare a `PrimaryDescriptor` column, validation returns `ModelError::IndentWithoutPrimaryDescriptor`. Rows specifying `indent: None` or `indent: Some(0)` are valid on any table (including pure numerical matrices without descriptor columns).
- **Structural calculation exclusion**: Neither `RowSpec::Gutter` nor `RowSpec::SectionHeader` emits `CellNode`s into `CellGrid`. Only `RowSpec::Data` produces calculation cells.
- **RowId calculation bijection**: Every `RowId` in `TableMetadata.rows` belongs strictly to a `DataRowSpec` and possesses calculation cells for all columns.
- **Gutter row height**: A `RowSpec::Gutter` is planned and emitted with `theme.metrics.gutter_row_height` (9.0 pt) and blank content across all columns.
- **Section header row height & continuous banner emission**: A `RowSpec::SectionHeader` is planned and emitted with uniform `theme.metrics.section_row_height` (18.0 pt). Its `title` is written at physical column `start_col` (the table's physical left margin) with left-aligned format (and optional `indent`). If `theme.palette.section_header_fill` is set (not none/transparent), columns `start_col + 1 ..= end_col` are emitted as blank formatted cells using `section_header_format`, providing a continuous colored horizontal section band across the table while text naturally overflows without cell merging (`<mergeCells>`). If fill is transparent/none, subsequent columns remain unwritten/empty.
- **Data-row temporal navigation**: Relative offsets (`row.prev`, `row.next`) navigate strictly across the ordered sequence of `DataRowSpec` IDs in `SymbolTable.rows`, bypassing intervening `Gutter` and `SectionHeader` rows. Formula lowering to Excel emits an A1 cell coordinate corresponding to the target data row's physical row.
- **Slice range endpoints**: Slice ranges (`"start".."end"`) require valid `RowId`s and can only be anchored to `DataRowSpec` items. Intermediate structural rows within the range are ignored in Finx evaluation, while the lowered Excel range encloses the physical span.

### Decisions

- **Tagged Enum for `RowSpec`**:
  - *Choice*: `pub enum RowSpec { Data(DataRowSpec), SectionHeader(SectionHeaderRowSpec), Gutter }`.
  - *Rationale*: Eliminates dummy IDs for gutters, isolates section header fields, and decouples calculation expansion from visual `SemanticStyle`.
  - *Alternatives considered*: Flat struct with `style: Option<SemanticStyle>`. Rejected because it forced gutters to have dummy IDs and leaked formatting styles into the calculation engine.

- **Purely Structural Section Headers (Zero RowId, Zero CellGrid Nodes)**:
  - *Choice*: `SectionHeaderRowSpec` contains only `title: String`, `indent: Option<u8>`, and `note: Option<String>` without a `RowId`. It emits zero `CellNode`s into `CellGrid`.
  - *Rationale*: Assigning a `RowId` to a section header causes formula slice ranges (e.g. `"rev".."cogs"`) to include the header ID, leading to missing cell coordinates in measure columns and runtime evaluation failures in `eval_range`. Keeping `RowId` strictly on `DataRowSpec` ensures that every `RowId` is 100% calculable across all columns. Structural rows are emitted directly to OpenXML during worksheet row streaming.
  - *Alternatives considered*: Giving `SectionHeader` a `RowId` and emitting partial/empty descriptor cells into `CellGrid`. Rejected because it leaks visual-only rows into the calculation DAG, complicates dependency analysis, and creates non-existent cell references.

- **Explicit `PrimaryDescriptor` Indentation (Zero Pseudo-Hierarchy)**:
  - *Choice*: Add `pub indent: Option<u8>` to both `DataRowSpec` and `SectionHeaderRowSpec` as an optional formatting attribute, applied strictly to the column with `AxisRole::PrimaryDescriptor`.
  - *Rationale*: Cell indentation in financial modeling is a visual presentation choice. Decoupling it from row height prevents `indent` from simulating a group hierarchy. Binding it explicitly to `PrimaryDescriptor` ensures that secondary descriptors (e.g. `Code`, `Units`, `Notes`) never receive unwanted indentation without relying on positional heuristics.
  - *Alternatives considered*: Leading whitespace in label strings (brittle for lookups); mandatory `indent: u8` driving row height (reintroduces group-like hierarchy into layout); positional stub-column heuristic (implicit, rejected for core purity).

- **Continuous Section Header Banner Emission (Zero Merged Cells)**:
  - *Choice*: Write section header title to `(row, start_col)` and format `start_col + 1 ..= end_col` with `section_header_format` when fill is present.
  - *Rationale*: The FAST financial modeling standard prohibits merged cells (`<mergeCells>`). Excel naturally spills left-aligned text across adjacent empty cells (even when formatted with matching fill), preserving keyboard navigation, copy-paste, and a clean continuous visual band across the table.

- **Data-Row Navigation for Temporal Offsets**:
  - *Choice*: `row.prev(k)` and `row.next(k)` navigate the sequence of `DataRowSpec` IDs in `SymbolTable.rows`, not physical rows.
  - *Rationale*: Structural rows have no `RowId` and hold no calculation values. In financial modeling, referencing a prior line item (e.g. beginning balance or previous line cost) must resolve to a valid calculable cell rather than a structural gap. Excel formula lowering bridges the physical row gap by computing the target data row's exact physical A1 coordinate.
  - *Alternatives considered*: Navigating physical rows. Rejected because hitting a gutter or section header would produce unresolvable missing-cell errors.

- **Purge `AxisRole` from Rows**:
  - *Choice*: Remove `role: AxisRole` from `DataRowSpec` and rows entirely.
  - *Rationale*: In tabular financial models, rows are line items/entities, while columns define dimensions/measures. Row roles were unused and redundant.

### Data Contracts

#### Updated `RowSpec` and Components
```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RowSpec {
    Data(DataRowSpec),
    SectionHeader(SectionHeaderRowSpec),
    Gutter,
}

impl RowSpec {
    pub fn id(&self) -> Option<&RowId> {
        match self {
            Self::Data(d) => Some(&d.id),
            Self::SectionHeader(_) | Self::Gutter => None,
        }
    }

    pub fn indent(&self) -> Option<u8> {
        match self {
            Self::Data(d) => d.indent,
            Self::SectionHeader(h) => h.indent,
            Self::Gutter => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataRowSpec {
    pub id: RowId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indent: Option<u8>,
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
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub cells: IndexMap<ColId, CellInput>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionHeaderRowSpec {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}
```

#### Pure Accounting `SemanticStyle`
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SemanticStyle {
    #[default]
    Data,
    Subtotal,
    Total,
}
```

### Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Explicit row indentation on PrimaryDescriptor**
  - **Given** a `TableSpec` with a `PrimaryDescriptor` column and a `DataRowSpec` with `indent: Some(2)`
  - **When** spatial layout and workbook emission execute
  - **Then** the primary descriptor cell receives `format.set_indent(2)`, while secondary descriptor columns and measure columns receive `indent: 0`.

- [ ] **Scenario 2: Section headers with continuous band fill and natural text overflow**
  - **Given** a `SectionHeaderRowSpec` with `title: "Operating Expenses"` and `indent: Some(1)`
  - **When** layout and emission execute
  - **Then** zero calculation cells are emitted into `CellGrid`; the header is planned with uniform 18.0 pt row height and bold styling; the title is written at physical column `start_col` with `format.set_indent(1)`, and if `theme.palette.section_header_fill` is present, columns `start_col + 1 ..= end_col` are emitted as blank formatted cells using `section_header_format`, providing a continuous colored horizontal section band across the table while text naturally overflows without merged cells.

- [ ] **Scenario 3: Explicit gutter row**
  - **Given** a `RowSpec::Gutter`
  - **When** grid expansion and layout plan the table
  - **Then** zero calculation cells are emitted into `CellGrid`, and the physical row is sized to `theme.metrics.gutter_row_height` (9.0 pt) and emitted blank with zero dummy ID required.

- [ ] **Scenario 4: Strict 1:1 physical coordinate mapping**
  - **Given** a `TableSpec` with $N$ explicit rows
  - **When** spatial layout calculates table bounds
  - **Then** the table body region spans exactly $N$ rows with zero phantom rows synthesized.

- [ ] **Scenario 5: Slice ranges spanning structural rows**
  - **Given** an expression using slice range `IncomeStatement["subscription".."support", @col]` where `Gutter` or `SectionHeader` intervenes between the rows
  - **When** formula parsing, dependency resolution, evaluation, and lowering execute
  - **Then** the range resolves strictly over `DataRowSpec` IDs without missing-cell errors, evaluates correctly in Finx, and lowers to an Excel range encompassing the physical span.

- [ ] **Scenario 6: Temporal offset skipping structural rows**
  - **Given** consecutive data rows separated by `Gutter` and `SectionHeader` (e.g. `cogs` at physical row 10, `Gutter` at row 11, `SectionHeader` at row 12, `salaries` at physical row 13 with formula `row.prev`)
  - **When** formula parsing, dependency resolution, evaluation, and lowering execute
  - **Then** `row.prev` on `salaries` resolves directly to `cogs`, evaluates correctly in Finx, and lowers to cell coordinate `C10` in Excel (bridging the 3-row physical gap).

---

## Tasks

### Phase 1: Model & AST updates
- [ ] Define `RowSpec` enum (`Data(DataRowSpec)`, `SectionHeader(SectionHeaderRowSpec)`, `Gutter`) in `src/model/table.rs`. `SectionHeaderRowSpec` has no `RowId`.
- [ ] Support `indent: Option<u8>` on `DataRowSpec` and `SectionHeaderRowSpec`.
- [ ] Validate that if any row has `indent.is_some_and(|k| k > 0)`, the table declares a `PrimaryDescriptor` column (`ModelError::IndentWithoutPrimaryDescriptor`).
- [ ] Remove `RowGroup`, `RowGroupChild`, and `row_groups` from `TableSpec` in `src/model/table.rs`.
- [ ] Remove `RowGroupId` from `src/model/id.rs`.
- [ ] Remove `role` from rows; keep `AxisRole` strictly on `ColumnSpec`.
- [ ] Remove row group validation methods and errors from `src/model/table.rs` and `src/model/error.rs`.
- [ ] Note: Core model migration (`AxisRole::PrimaryDescriptor`, `RowSpec`, `ColumnSpec`, `RelativeOffset`) is executed in lockstep with the multi-tier column headers spec.

### Phase 2: Expression parser, IR expansion & symbol resolution
- [ ] Remove `SpecialRow::Group*` variants from `src/expr/ast.rs` and `src/expr/parser.rs`.
- [ ] Update `expand_table` in `src/compiler/ir/expand.rs`:
  - Match on `RowSpec::Gutter` | `RowSpec::SectionHeader(_)` $\rightarrow$ skip cell emission.
  - Match on `RowSpec::Data(data)` $\rightarrow$ standard 3-tier expansion.
- [ ] Populate `SymbolTable.rows` and `TableMetadata.rows` strictly with `DataRowSpec` IDs.
- [ ] Remove `row_groups` and `row_roles` from `SymbolTable` and lookup methods in `src/compiler/ir/symbol.rs`.
- [ ] Remove `UnknownRowGroup` from `src/compiler/graph/error.rs`.
- [ ] Remove row group resolution branches from `src/compiler/graph/resolve.rs`.

### Phase 3: Spatial layout planner refactoring
- [ ] Remove `RowGroupAnalysis`, `analyze_row_groups`, phantom section header insertion, and phantom gutter insertion from `src/compiler/layout/mod.rs`.
- [ ] Map `table.rows` strictly 1:1 to physical row coordinates.
- [ ] Set physical row height in `SheetLayout.row_heights`:
  - `RowSpec::Gutter`: `theme.metrics.gutter_row_height` (9.0 pt).
  - `RowSpec::SectionHeader`: uniform `theme.metrics.section_row_height` (18.0 pt).
  - `RowSpec::Data`: `theme.metrics.data_row_height` (15.0 pt).
- [ ] Pass `row.indent().unwrap_or(0)` strictly to cells of `AxisRole::PrimaryDescriptor` in `PhysicalCoordinate.indent`.

### Phase 4: Emission & format cache
- [ ] Update `FormatCache` with helper to format `SectionHeader` with optional `indent`.
- [ ] Remove `SectionHeaderRow` from `SheetLayout` and remove the interleaved section header loop from `src/compiler/emission/workbook.rs`.
- [ ] In `src/compiler/emission/workbook.rs`, stream structural rows (`SectionHeader` and `Gutter`) directly during row emission, setting explicit row heights, writing header text/note at `table.start_col` with left-aligned indentation format, and if `theme.palette.section_header_fill` is present, writing blank formatted cells across `start_col + 1 ..= end_col` for a continuous section band without merged cells.

### Phase 5: Test fixtures & verification
- [ ] Update model fixtures (`tests/fixtures/three_statement.rs`, `lbo.rs`, `styling_showcase.rs`) to use the new `RowSpec` constructors.
- [ ] Update test suites across `tests/` (`spatial_layout_test.rs`, `spec_test.rs`, `workbook_emission_test.rs`, `format_cache_test.rs`, `grid_expansion_test.rs`, etc.).
- [ ] Verify full test suite: `cargo check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- [ ] Reconcile living specs and archive this spec to `specs/history/`.
