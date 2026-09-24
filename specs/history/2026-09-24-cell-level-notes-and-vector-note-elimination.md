---
title: Cell-Level Notes & Vector Note Elimination
date: 2026-09-24
status: completed
target-living-spec: specs/living-specs/compiler/cell-ir.md
---

## Proposal

### What

Eliminate vector-level audit notes from `DataRowSpec` and `DataColumnSpec`, establishing `DetailedCellInput` (0D point override) as the single source of truth for cell notes.

### Why

- **Eliminate unintended note broadcasting**: In the 3-Tier Specificity Lattice, row-level and column-level notes currently cascade down into every intersecting `CellNode`. During workbook emission, this causes identical Excel note comments to be stamped across every data cell in an entire row or column.
- **Asymmetric 0D presentation**: In financial modeling and spreadsheet design, audit notes and footnotes are inherently 0D point annotations attached to a specific cell (e.g. an audit sign-off on a line-item label or a footnote explaining an input assumption). They are not 1D vector properties (like numeric format, semantic style, or currency) that broadcast across calculation vectors.
- **Avoid artificial routing heuristics**: Allowing notes on row definitions tempts ad-hoc routing heuristics (e.g. "route the row note to the primary descriptor column"). As established during the elimination of `PrimaryDescriptor`, introducing magic routing rules violates the compiler core's principle of dead-simple, explicit, and predictable data structures.
- **Single source of truth**: Restricting cell notes to `DetailedCellInput` ensures strictly one way to represent cell-level notes without ambiguity or precedence conflicts.

### Scope

- **In-scope**:
  - Remove `note: Option<String>` from `DataRowSpec` in `crates/f8-core/src/model/table.rs`.
  - Remove `note: Option<String>` from `DataColumnSpec` in `crates/f8-core/src/model/table.rs`.
  - Remove builder methods `with_note` from `DataRowSpec`, `DataColumnSpec`, `ColumnSpec`, and `RowSpec`.
  - Retain `note: Option<String>` on `SectionHeaderRowSpec` (as a 0D structural banner row with no child cells).
  - Retain `note: Option<String>` on `DetailedCellInput` and `CellInput::note(&self) -> Option<&str>`.
  - Retain `note: Option<String>` on `CellNode` and `CellIR`.
  - Simplify `expand_table` in `crates/f8-core/src/compiler/ir/expand.rs`: resolve `CellNode.note` strictly from `cell_input.note()`.
  - Update unit and integration tests across `f8-core`.
  - Update living spec `specs/living-specs/compiler/cell-ir.md` and compiler docs `docs/compiler/ir.md`.
- **Out-of-scope**:
  - Changes to OpenXML note emission in `crates/f8-core/src/compiler/emission/workbook.rs` (`worksheet.insert_note` already operates directly on `CellNode.note`).
  - Changes to `source` provenance metadata cascading.

### What Changes

- `crates/f8-core/src/model/table.rs`:
  - `DataRowSpec`: Remove field `pub note: Option<String>` and method `with_note`.
  - `DataColumnSpec`: Remove field `pub note: Option<String>` and method `with_note`.
  - `ColumnSpec`: Remove method `with_note`.
  - `RowSpec`: Remove method `with_note` (callers constructing section headers use `SectionHeaderRowSpec::with_note`).
- `crates/f8-core/src/compiler/ir/expand.rs`:
  - `expand_table`: Remove row and column note fallback (`row_data.note` and `col_data.note`). `effective_note` evaluates strictly from `cell_input.note()`.

### Capabilities

- **New capabilities**: Clean 0D note authoring without unexpected side effects across entire rows or columns.
- **Modified capabilities**: Row-level and column-level `note` attributes are removed from specification schemas and rejected during compilation.

---

## Design

### Goals

1. Eliminate duplicate note emission across data rows and measure columns.
2. Establish `DetailedCellInput.note` as the unique source of truth for cell notes.
3. Keep the 3-Tier Specificity Lattice simple, predictable, and free of special-case descriptor heuristics.

### Invariants

- **0D annotation isolation**: Cell notes never broadcast across axes. A note belongs strictly to the cell coordinate $(r, c)$ where it is declared.
- **Structural section header compatibility**: Section header rows retain their explicit note at the table start column.

### Decisions

- **Choice**: Eliminate `note` from `DataRowSpec` and `DataColumnSpec` rather than routing row notes to the first descriptor column.
  - **Rationale**: Keeps compiler core explicit and predictable. Avoids heuristic edge cases when tables contain zero or multiple descriptor columns.
  - **Alternatives considered**: Routing row note to the first descriptor column. Rejected because it introduces implicit behavior and magic fallbacks.

### Data Contracts

#### AST Changes (`f8_core::model::table`)

| Type | Field / Method | Status | Notes |
| :--- | :--- | :--- | :--- |
| `DataRowSpec` | `pub note: Option<String>` | **Removed** | Avoids vector-level note declarations |
| `DataRowSpec` | `pub fn with_note(...)` | **Removed** | Removed builder |
| `DataColumnSpec` | `pub note: Option<String>` | **Removed** | Avoids vector-level note declarations |
| `DataColumnSpec` | `pub fn with_note(...)` | **Removed** | Removed builder |
| `ColumnSpec` | `pub fn with_note(...)` | **Removed** | Removed builder |
| `RowSpec` | `pub fn with_note(...)` | **Removed** | Use `SectionHeaderRowSpec::with_note` |
| `SectionHeaderRowSpec` | `pub note: Option<String>` | **Retained** | Emitted at `(current_phys_row, start_col)` |
| `DetailedCellInput` | `pub note: Option<String>` | **Retained** | Single source of truth for cell notes |

### Risks & Trade-offs

- **Breaking change for existing specs with row notes**: Specifications that authored `"note"` at the row level will need notes placed directly on specific cells (e.g. the line item label).
- **Mitigation**: Update all internal tests, examples, and documentation to model cell notes via `DetailedCellInput`.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Explicit cell note emission**
  - **Given** a table with a data row having a `DetailedCellInput` with `note: Some("Audited value")` on column `"2023A"`
  - **When** the table is expanded and emitted to workbook
  - **Then** `CellNode` for `("revenue", "2023A")` contains `note: Some("Audited value")`, and other cells in the row have `note: None`.

- [x] **Scenario 2: Zero note broadcasting across vector rows**
  - **Given** a data row with a vector formula across measure columns
  - **When** the table is expanded
  - **Then** all generated formula `CellNode`s have `note: None`.

- [x] **Scenario 3: Section header note preservation**
  - **Given** a `SectionHeaderRowSpec` with `note: Some("Operating section")`
  - **When** the workbook is emitted
  - **Then** an Excel note is inserted at `(current_phys_row, start_col)` and no other cells.

- [x] **Scenario 4: Serialization schema conformance**
  - **Given** JSON with row-level `"note"` on a data row
  - **When** deserialized with `deny_unknown_fields` (or schema validated)
  - **Then** row-level `"note"` is unrecognized or ignored, and cell-level `"note"` deserializes cleanly.

---

## Tasks

### Phase 1: Model & AST Updates
- [x] Remove `note` and `with_note` from `DataRowSpec` in `crates/f8-core/src/model/table.rs`.
- [x] Remove `note` and `with_note` from `DataColumnSpec` in `crates/f8-core/src/model/table.rs`.
- [x] Remove `with_note` from `ColumnSpec` and `RowSpec` in `crates/f8-core/src/model/table.rs`.

### Phase 2: Compiler IR Grid Expansion Updates
- [x] Simplify note resolution in `crates/f8-core/src/compiler/ir/expand.rs` to derive strictly from `cell_input.note()`.
- [x] Ensure vector-generated cells default to `note: None`.

### Phase 3: Test Fixture & Documentation Updates
- [x] Update `crates/f8-core/tests/grid_expansion_test.rs` to test 0D cell note isolation.
- [x] Update `crates/f8-core/tests/spec_test.rs` to author notes at cell level.
- [x] Update internal test in `crates/f8-core/src/compiler/ir/expand.rs`.
- [x] Update documentation in `docs/compiler/ir.md` and `docs/dsl/spec-format.md`.

### Phase 4: Verification Suite
- [x] Run `cargo check`.
- [x] Run `cargo test`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo fmt --check`.

### Phase 5: Living Spec Reconciliation & Archive
- [x] Update `specs/living-specs/compiler/cell-ir.md`.
- [x] Move draft spec from `specs/drafts/` to `specs/history/`.
