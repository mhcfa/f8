---
title: Semantic Formatting AST & IR Resolution
date: 2026-09-19
status: completed
target-living-spec:
  - specs/living-specs/model/spec-ast.md
  - specs/living-specs/compiler/cell-ir.md
---

## Proposal

### What

Implement the foundational Spec AST representations and Cell IR 3-Tier Specificity Lattice resolution algorithms for Finx's semantic formatting system:
1. Define the `SemanticStyle` enum (`data`, `subtotal`, `total`) in the `model` subsystem.
2. Extend `ColumnSpec`, `RowSpec`, and `DetailedCellInput` with `style`, `highlight`, and `decimals` properties.
3. Enforce AST validation preventing precision (`decimals`) on non-numeric format presets (`date`, `text`).
4. Extend `CellNode` with resolved `decimals: Option<u32>`, `horizontal: SemanticStyle`, `vertical: SemanticStyle`, and `is_highlight: bool`.
5. Implement orthogonal 2D border composition, precision resolution, vector conflict detection (`DecimalsConflict`), and highlight inheritance in `compiler::ir::expand`.

### Why

Finx's design requires complete decoupling between calculation logic and presentation across four orthogonal axes (`style`, `highlight`, `format`, `decimals`). Currently, `ColumnSpec`, `RowSpec`, `DetailedCellInput`, and `CellNode` lack fields for style, highlight, and display precision, and `expand_table` only resolves `FormatPreset`.

Before the spatial layout engine (Pass 5) or OpenXML binary emitter (Pass 6) can be built, the AST must accept these declarative visual properties, and the IR expansion engine must resolve them deterministically according to the 3-Tier Specificity Lattice.

### Scope

- **In-Scope**:
  - `SemanticStyle` enum and serialization in `finx::model::format`.
  - Adding `style`, `highlight`, and `decimals` to `ColumnSpec`, `RowSpec`, and `DetailedCellInput` in `finx::model::table`.
  - Schema validation rejecting `decimals` on `date` and `text` presets (`ModelError::InvalidPrecisionForPreset`).
  - Adding `decimals`, `horizontal`, `vertical`, and `is_highlight` to `CellNode` in `finx::compiler::ir::cell`.
  - Resolution algorithms in `finx::compiler::ir::expand` (2D symmetrical border composition, cell reset escape hatch `style: "data"`, measure decimals conflict detection `IrError::DecimalsConflict`, point highlight override).
  - Unit and integration tests covering resolution, inheritance, and error rejection.

- **Out-of-Scope**:
  - Theme JSON configuration schema, loading, and built-in theme presets (Phase 2).
  - Dynamic format mask template synthesis (`{dec}` expansion) (Phase 2).
  - Pass 5 spatial layout planning, margins, gutters, column width arbitration, and row heights (Phase 3).
  - Pass 6 OpenXML emission via `rust_xlsxwriter` (Phase 4).

### What Changes

- **Affected modules**:
  - `src/model/format.rs`: Add `SemanticStyle` enum (`Data`, `Subtotal`, `Total`).
  - `src/model/table.rs`: Add `style`, `highlight`, `decimals` fields to `ColumnSpec`, `RowSpec`, `DetailedCellInput`.
  - `src/model/error.rs`: Add `ModelError::InvalidPrecisionForPreset`.
  - `src/compiler/ir/cell.rs`: Add `decimals`, `horizontal`, `vertical`, `is_highlight` to `CellNode`.
  - `src/compiler/ir/error.rs`: Add `IrError::DecimalsConflict`.
  - `src/compiler/ir/expand.rs`: Update `expand_table` to resolve format, decimals, styles, and highlights.
  - `tests/`: Add unit tests for AST validation and IR resolution scenarios.

### Capabilities

- **New capabilities**:
  - Model authors can declare `style` (`data`, `subtotal`, `total`), `highlight` (`true`/`false`), and `decimals` (`u32`) symmetrically on rows, columns, and individual cell overrides.
  - Cell overrides can cleanly suppress inherited summary borders on total rows by setting `style: "data"`.
  - Total rows intersecting total columns automatically resolve both `horizontal: Total` and `vertical: Total`, forming 4-sided accounting boxes.
  - Conflicting measure precision across intersecting rows and columns without a cell override triggers explicit diagnostic error `IrError::DecimalsConflict`.
- **Modified capabilities**:
  - `CellNode` produces fully resolved visual attributes ready for downstream spatial layout and emission.

---

## Design

### Goals

- Symmetrical 2D matrix model: `style`, `highlight`, `format`, and `decimals` behave identically on rows, columns, and cell overrides.
- Orthogonal geometric border axes: rows govern horizontal edges; columns govern vertical edges.
- Zero layout drift: visual attributes do not affect formula evaluation or underlying floating-point values.
- Clean greenfield architecture: no legacy shims, zero unwraps in non-test code, structured error handling with `thiserror`.

### Invariants

1. **Orthogonal border axes**: A row style never overwrites a column style; they resolve independently into `(horizontal, vertical)` pairs.
2. **Descriptor boundary**: Descriptor coordinates (`role: "descriptor"`) never inherit measure formats, but do participate in structural styles and highlights.
3. **Point override precedence**: An explicit property on `CellInput` always supersedes vector properties from rows or columns.
4. **Data style reset**: `cell.style == Some(SemanticStyle::Data)` unconditionally suppresses both horizontal and vertical summary rules, resolving `(Data, Data)`.
5. **Numeric precision applicability**: Non-numeric presets (`date`, `text`) never carry `decimals`; attempting to configure `decimals` on them triggers `ModelError::InvalidPrecisionForPreset`.

### Decisions

- **Decision: Separate `horizontal` and `vertical` styles on `CellNode`**
  - *Rationale*: A single scalar `style` forces an arbitrary winner between row totals and column totals. An orthogonal pair `(horizontal, vertical)` models the physical Excel grid where horizontal borders (top/bottom) and vertical borders (left/right) naturally coexist.
  - *Alternatives considered*: Single scalar `style` with row-wins or col-wins priority. Rejected because it breaks framed total boxes at matrix intersections and prevents clean accounting total columns.

- **Decision: Cell-level `style: "data"` reset**
  - *Rationale*: In institutional modeling, total rows often include row label/descriptor columns (e.g. "Total Operating Expenses") where top/bottom double underlines look visually broken. `style: "data"` provides an explicit, local escape hatch without complex heuristic guessing.
  - *Alternatives considered*: Automatically suppressing borders on descriptor columns. Rejected because users sometimes want borders to span the full row, and explicit data flow is preferred over magic heuristics.

- **Decision: Decimal precision on non-numeric presets as validation error**
  - *Rationale*: Permitting `decimals: 2` on a `date` or `text` preset represents a semantic contradiction in the model specification. Catching this during `TableSpec::validate` provides immediate, actionable feedback to model authors.
  - *Alternatives considered*: Silently ignoring `decimals` on dates and text. Rejected because silent truncation masks author errors.

### Data Contracts

#### 1. Model Subsystem

```rust
// src/model/format.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SemanticStyle {
    #[default]
    Data,
    Subtotal,
    Total,
}

// src/model/table.rs
pub struct ColumnSpec {
    // Existing fields...
    pub style: Option<SemanticStyle>,
    pub highlight: Option<bool>,
    pub decimals: Option<u32>,
}

pub struct RowSpec {
    // Existing fields...
    pub style: Option<SemanticStyle>,
    pub highlight: Option<bool>,
    pub decimals: Option<u32>,
}

pub struct DetailedCellInput {
    // Existing fields...
    pub style: Option<SemanticStyle>,
    pub highlight: Option<bool>,
    pub decimals: Option<u32>,
}
```

#### 2. Compiler IR Subsystem

```rust
// src/compiler/ir/cell.rs
pub struct CellNode {
    pub id: CellId,
    pub sheet: SheetId,
    pub payload: CellPayload,
    pub format: Option<FormatPreset>,
    pub decimals: Option<u32>,
    pub horizontal: SemanticStyle,
    pub vertical: SemanticStyle,
    pub is_highlight: bool,
    pub note: Option<String>,
    pub source: Option<serde_json::Value>,
}
```

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Symmetrical 2D border composition at matrix intersection**
  - **Given** a table where row `net_income` has `style: "total"` and column `fy` has `style: "total"`
  - **When** the table is expanded into `CellGrid`
  - **Then** the cell at `("net_income", "fy")` has `horizontal == SemanticStyle::Total` and `vertical == SemanticStyle::Total`.

- [x] **Scenario 2: Cell reset suppresses inherited summary borders**
  - **Given** a table where row `gross_profit` has `style: "subtotal"`
  - **When** cell `("gross_profit", "line_item")` specifies `style: "data"`
  - **Then** the cell resolves `horizontal == SemanticStyle::Data` and `vertical == SemanticStyle::Data`.

- [x] **Scenario 3: Cell precision override wins over vector decimals**
  - **Given** a row `eps` with `decimals: 2`
  - **When** cell `("eps", "q1")` specifies `decimals: 4`
  - **Then** the cell resolves `decimals == Some(4)`.

- [x] **Scenario 4: Measure vector precision conflict rejected**
  - **Given** measure row $r$ with `decimals: 2` and measure column $c$ with `decimals: 0` without a cell override
  - **When** the table is expanded into `CellGrid`
  - **Then** compilation halts with `IrError::DecimalsConflict`.

- [x] **Scenario 5: Non-numeric preset with decimals rejected at AST validation**
  - **Given** a `ColumnSpec`, `RowSpec`, or `DetailedCellInput` with `format: "date"` and `decimals: 2`
  - **When** `table.validate()` is executed
  - **Then** validation fails with `ModelError::InvalidPrecisionForPreset`.

- [x] **Scenario 6: Point highlight override suppresses vector highlight**
  - **Given** a row with `highlight: true`
  - **When** a cell override specifies `highlight: false`
  - **Then** the resolved cell has `is_highlight == false`.

- [x] **Scenario 7: Highlight inheritance from row or column**
  - **Given** a column with `highlight: true` and a normal row
  - **When** the table is expanded into `CellGrid`
  - **Then** all cells in that column resolve `is_highlight == true`.

---

## Tasks

### Phase 1: Model Subsystem Enhancements
- [x] Define `SemanticStyle` enum with `serde(rename_all = "snake_case")` and `Default` in `src/model/format.rs`.
- [x] Add `style`, `highlight`, and `decimals` to `ColumnSpec`, `RowSpec`, and `DetailedCellInput` in `src/model/table.rs`.
- [x] Add `InvalidPrecisionForPreset` error variant to `ModelError` in `src/model/error.rs`.
- [x] Implement AST validation in `TableSpec::validate` checking that `format: "date"` or `format: "text"` does not have `decimals.is_some()`.
- [x] Add unit tests in `tests/spec_test.rs` verifying serialization, deserialization, and validation.

### Phase 2: Cell IR & Specificity Lattice Resolution
- [x] Add `decimals`, `horizontal`, `vertical`, and `is_highlight` fields and builder methods to `CellNode` in `src/compiler/ir/cell.rs`.
- [x] Add `DecimalsConflict` error variant to `IrError` in `src/compiler/ir/error.rs`.
- [x] Implement decimals resolution logic with conflict detection in `src/compiler/ir/expand.rs`.
- [x] Implement 2D symmetrical style resolution logic with `SemanticStyle::Data` reset in `src/compiler/ir/expand.rs`.
- [x] Implement highlight resolution logic with point-level `Option<bool>` precedence in `src/compiler/ir/expand.rs`.
- [x] Add comprehensive unit tests in `src/compiler/ir/expand.rs` and `tests/grid_expansion_test.rs` covering all acceptance scenarios.

### Phase 3: Verification Suite
- [x] Run `cargo check`.
- [x] Run `cargo test`.
- [x] Run `cargo clippy --all-targets -- -D warnings`.
- [x] Run `cargo fmt --check`.
