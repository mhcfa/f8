---
title: Currency Code AST & IR Resolution
date: 2026-09-20
status: completed
target-living-spec:
  - specs/living-specs/model/spec-ast.md
  - specs/living-specs/compiler/cell-ir.md
---

## Proposal

### What

Implement the AST schema representations, validation rules, and IR 3-Tier Specificity Lattice resolution algorithms for canonical currency codes in Finx:
1. Define the strongly typed `CurrencyCode` enum in `finx::model::format` supporting canonical 3-letter lowercase ISO 4217 codes (`usd`, `eur`, `gbp`, `jpy`, `chf`, `cad`, `aud`, `cny`, `hkd`, `sgd`, `sek`, `nok`, `nzd`, `inr`, `brl`) with `CurrencyCode::Usd` default, symbol mappings, and safe Excel formatting.
2. Extend `TableSpec`, `ColumnSpec`, `RowSpec`, and `DetailedCellInput` with optional `currency: Option<CurrencyCode>`.
3. Extend `TableSpec` with optional `subtitle: Option<String>` and non-empty validation (`ModelError::EmptyTableSubtitle`).
4. Implement AST validation in `TableSpec::validate` rejecting currency declarations on non-currency format presets (`ModelError::InvalidCurrencyForPreset`) and precision exceeding 14 (`ModelError::PrecisionOutOfBounds`).
5. Extend `CellNode` in `finx::compiler::ir::cell` with resolved `currency: Option<CurrencyCode>`.
6. Implement the 3-Tier Currency Specificity Lattice in `finx::compiler::ir::expand` (`Cell Override` $\succ$ `Vector Rule` $\succ$ `Table Default (defaults to USD)`), detecting conflicting vector currencies (`IrError::CurrencyConflict`).
7. Provide builder methods and comprehensive unit tests covering validation, inheritance, conflict detection, descriptor isolation, and non-currency preset isolation.

### Why

Financial models frequently operate across international currencies or feature multi-currency tables (e.g. FX conversions, foreign subsidiary consolidations):
- **Single canonical authoring**: A finite 3-letter lowercase enum eliminates authoring ambiguity (`"currency": "usd"` vs `"$"` / `"US$"` / `"$$"`).
- **Serde-level validation**: Serde automatically rejects unrecognized currency strings with clear, structured diagnostics at deserialization time without needing ad-hoc string checks.
- **Zero heap allocations (Rust 2024)**: `CurrencyCode` implements `Copy`, occupying 1 byte on the stack. Storing `Option<CurrencyCode>` on `CellNode` eliminates thousands of heap allocations and string clones during grid expansion.
- **Deterministic Excel symbol formatting**: Knowing the finite currency catalog enables deterministic mapping to standard Excel format masks and safe quoting of multi-letter symbols (`"CHF"`, `"kr"`, `"C$"`) to avoid collisions with Excel format syntax tokens.
- **Modular self-contained tables (3-tier lattice)**: Calculation tables in `tables/*.json` remain completely autonomous and testable in isolation without depending on cross-file manifest settings.

### Scope

- **In-Scope**:
  - `CurrencyCode` enum in `src/model/format.rs` with `Copy`, `Default`, `Display`, Serde lowercase rename, `symbol()`, and `excel_symbol()`.
  - `currency: Option<CurrencyCode>` on `TableSpec`, `ColumnSpec`, `RowSpec`, and `DetailedCellInput`.
  - `subtitle: Option<String>` on `TableSpec` with validation rejecting empty/whitespace strings (`ModelError::EmptyTableSubtitle`).
  - AST validation rejecting currency declarations on non-currency presets (`ModelError::InvalidCurrencyForPreset`).
  - `currency: Option<CurrencyCode>` on `CellNode`.
  - 3-tier specificity lattice resolution in `compiler::ir::expand` with `IrError::CurrencyConflict`.
  - Descriptor axis boundary enforcement (descriptor cells resolve `currency: None` and do not trigger conflicts).
  - Unit and integration tests covering all tiers, conflicts, descriptor boundaries, and preset applicability.

- **Out-of-Scope**:
  - Dynamic format mask expansion (`{sym}` and `{dec}` string substitution in `finx::model::theme`, covered in theme spec).
  - Pass 5 spatial layout and column width arbitration.
  - Pass 6 OpenXML binary emission via `rust_xlsxwriter`.

### What Changes

- **Affected modules**:
  - `src/model/format.rs`: Define `CurrencyCode` enum with display and Excel mask helpers.
  - `src/model/table.rs`: Add `currency` field, `subtitle` field, builder methods, and validation to `TableSpec`, `ColumnSpec`, `RowSpec`, and `DetailedCellInput`.
  - `src/model/error.rs`: Add `ModelError::InvalidCurrencyForPreset` and `ModelError::EmptyTableSubtitle`.
  - `src/compiler/ir/cell.rs`: Add `currency: Option<CurrencyCode>` field and builder method to `CellNode`.
  - `src/compiler/ir/error.rs`: Add `IrError::CurrencyConflict`.
  - `src/compiler/ir/expand.rs`: Implement `resolve_currency` and wire into `expand_table` and `expand_workbook`.
  - `tests/`: Add unit tests for AST validation and IR resolution scenarios.

### Capabilities

- **New capabilities**:
  - Model authors can declare currency codes per table (`"currency": "eur"`), per column/row vector (`"currency": "gbp"`), or on individual cell overrides (`"currency": "jpy"`).
  - Model authors can declare optional table subtitles (`"subtitle": "($ in millions)"`).
  - Currency format cells automatically inherit and resolve the correct `CurrencyCode` according to the 3-tier specificity lattice, defaulting to `CurrencyCode::Usd` if omitted on the table.
  - Conflicting measure vector currencies (e.g. row USD intersecting column EUR) without a cell override cleanly fail compilation with `IrError::CurrencyConflict`.
  - Cells with non-currency presets (`number`, `percentage`, `multiple`, `basis_points`, `date`, `text`) cleanly resolve `currency: None`.

- **Modified capabilities**:
  - `CellNode` carries the resolved `currency: Option<CurrencyCode>` ready for OpenXML emission.

---

## Design

### Goals

- **Deterministic resolution**: Every currency cell deterministically resolves its `CurrencyCode` or cleanly signals conflict.
- **Architectural symmetry**: Follow the exact 3-tier lattice and error paradigms established for `decimals`, `format`, and `style`.
- **Zero layout drift**: Currency codes exist purely as presentation metadata on `CellNode` and never alter formula evaluation or floating-point values.
- **Fail-fast validation**: Invalid configurations (currency declared on percentage/date presets) are rejected during AST validation before compiler IR expansion.

### Invariants

1. **3-tier specificity lattice order**:
   $$\text{Cell Override} \succ \text{Vector Rule (Row or Column)} \succ \text{Table Default (defaults to USD)}$$
2. **Preset isolation**: Only cells resolving `format == Some(FormatPreset::Currency)` receive a resolved currency. All other presets resolve `currency: None`.
3. **Descriptor boundary**: Descriptor coordinates (`role: "descriptor"`) isolate measure vectors, resolve `currency: None`, and never broadcast measure currency rules or trigger conflicts.
4. **Vector conflict detection**: When a measure row and measure column declare differing currency codes, compilation halts with `IrError::CurrencyConflict` unless an explicit cell override is present.
5. **Canonical enum representation**: Currencies deserialize from 3-letter lowercase ISO strings (`"usd"`, `"eur"`, `"gbp"`, `"jpy"`, `"chf"`, `"cad"`, `"aud"`, `"cny"`, `"hkd"`, `"sgd"`, `"sek"`, `"nok"`, `"nzd"`, `"inr"`, `"brl"`). Any other string is rejected by Serde.

### Decisions

- **Decision: Strongly-typed `CurrencyCode` enum instead of free-form `String`**
  - *Rationale*: Eliminates authoring variations (`"$"` vs `"US$"` vs `"USD"`), allows Serde to perform validation automatically, avoids heap allocations (`Copy` 1-byte enum), and enables deterministic Excel format token escaping.
  - *Alternatives considered*: Free-form string. Rejected due to ambiguity, allocation overhead, and vulnerability to Excel syntax collisions.

- **Decision: 3-Tier Specificity Lattice without Manifest Currency**
  - *Rationale*: Financial tables in `tables/*.json` must remain self-contained, modular, and testable in isolation. Symmetrically aligns currency with the 3-tier scoping of `decimals`, `style`, and `format`.
  - *Alternatives considered*: 5-tier lattice with manifest currency. Rejected because it couples table expansion to project manifests and creates an outlier 5-tier concept for only one property.

- **Decision: Reject `currency` on non-currency presets during AST validation**
  - *Rationale*: Declaring `currency: "usd"` on a column or row configured with `format: "percentage"` or `format: "date"` is a semantic contradiction. Raising `ModelError::InvalidCurrencyForPreset` prevents author confusion.

### Data Contracts

#### 1. Model Subsystem

```rust
// src/model/format.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum CurrencyCode {
    #[default]
    Usd,
    Eur,
    Gbp,
    Jpy,
    Chf,
    Cad,
    Aud,
    Cny,
    Hkd,
    Sgd,
    Sek,
    Nok,
    Nzd,
    Inr,
    Brl,
}

impl CurrencyCode {
    /// Returns the standard currency symbol.
    pub const fn symbol(&self) -> &'static str {
        match self {
            Self::Usd => "$",
            Self::Eur => "€",
            Self::Gbp => "£",
            Self::Jpy | Self::Cny => "¥",
            Self::Chf => "CHF",
            Self::Cad => "C$",
            Self::Aud => "A$",
            Self::Hkd => "HK$",
            Self::Sgd => "S$",
            Self::Sek | Self::Nok => "kr",
            Self::Nzd => "NZ$",
            Self::Inr => "₹",
            Self::Brl => "R$",
        }
    }

    /// Returns the Excel-safe mask symbol, quoting multi-letter and non-ASCII codes to prevent syntax collisions.
    pub const fn excel_symbol(&self) -> &'static str {
        match self {
            Self::Usd => "$",
            Self::Eur => "€",
            Self::Gbp => "£",
            Self::Jpy | Self::Cny => "¥",
            Self::Chf => "\"CHF\"",
            Self::Cad => "\"C$\"",
            Self::Aud => "\"A$\"",
            Self::Hkd => "\"HK$\"",
            Self::Sgd => "\"S$\"",
            Self::Sek | Self::Nok => "\"kr\"",
            Self::Nzd => "\"NZ$\"",
            Self::Inr => "\"₹\"",
            Self::Brl => "\"R$\"",
        }
    }
}

// src/model/table.rs
pub struct TableSpec {
    pub id: TableId,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<CurrencyCode>,
    pub columns: Vec<ColumnSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_groups: Vec<ColumnGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_groups: Vec<RowGroup>,
    pub rows: Vec<RowSpec>,
}

pub struct ColumnSpec {
    // Existing fields...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<CurrencyCode>,
}

pub struct RowSpec {
    // Existing fields...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<CurrencyCode>,
}

pub struct DetailedCellInput {
    // Existing fields...
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<CurrencyCode>,
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
    pub currency: Option<CurrencyCode>,
    pub horizontal: SemanticStyle,
    pub vertical: SemanticStyle,
    pub is_highlight: bool,
    pub note: Option<String>,
    pub source: Option<serde_json::Value>,
}

// src/compiler/ir/error.rs
pub enum IrError {
    // Existing variants...
    #[error(
        "Table '{table}': currency conflict at row '{row}' ('{row_currency}') and column '{column}' ('{col_currency}')"
    )]
    CurrencyConflict {
        table: TableId,
        row: RowId,
        column: ColId,
        row_currency: CurrencyCode,
        col_currency: CurrencyCode,
    },
}
```

#### 3. Diagnostic Errors (`ModelError`)

```rust
// src/model/error.rs
pub enum ModelError {
    // Existing variants...
    #[error(
        "Table '{table}': {context} specifies currency ('{currency}') for non-currency preset '{preset}'"
    )]
    InvalidCurrencyForPreset {
        table: TableId,
        context: String,
        preset: FormatPreset,
        currency: CurrencyCode,
    },

    #[error("Table '{table}': subtitle must not be empty or whitespace")]
    EmptyTableSubtitle {
        table: TableId,
    },

    #[error(
        "Table '{table}': {context} specifies decimals ({decimals}) exceeding maximum supported precision ({max})"
    )]
    PrecisionOutOfBounds {
        table: TableId,
        context: String,
        decimals: u32,
        max: u32,
    },
}
```

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Point override (Cell) wins over vector and table tiers**
  - **Given** table currency `eur`, column currency `gbp`, and row currency `jpy`
  - **When** cell specifies `currency: "chf"` and format resolves to `Currency`
  - **Then** `cell_node.currency` resolves to `Some(CurrencyCode::Chf)`.

- [x] **Scenario 2: Vector rule (Row or Column) wins over table default**
  - **Given** table currency `eur` and column currency `gbp`
  - **When** cell has no currency override and format resolves to `Currency`
  - **Then** `cell_node.currency` resolves to `Some(CurrencyCode::Gbp)`.

- [x] **Scenario 3: Measure vector currency conflict rejected**
  - **Given** a measure row with `currency: "eur"` and measure column with `currency: "usd"` without a cell override
  - **When** the table is expanded into `CellGrid`
  - **Then** compilation halts with `IrError::CurrencyConflict`.

- [x] **Scenario 4: Harmonious measure vector currencies accepted**
  - **Given** a measure row with `currency: "eur"` and measure column with `currency: "eur"`
  - **When** the table is expanded into `CellGrid`
  - **Then** `cell_node.currency` resolves to `Some(CurrencyCode::Eur)`.

- [x] **Scenario 5: Table default applies when vectors omit currency**
  - **Given** table currency `eur` and rows/columns with no currency
  - **When** format resolves to `Currency`
  - **Then** `cell_node.currency` resolves to `Some(CurrencyCode::Eur)`.

- [x] **Scenario 6: Fallback to USD when omitted across all tiers**
  - **Given** table with `currency: None` and no vector or cell currency
  - **When** format resolves to `Currency`
  - **Then** `cell_node.currency` resolves to `Some(CurrencyCode::Usd)`.

- [x] **Scenario 7: Non-currency format presets resolve `currency: None`**
  - **Given** table currency `eur`
  - **When** a cell's format resolves to `Number`, `Percentage`, `Date`, or `Text`
  - **Then** `cell_node.currency` resolves to `None`.

- [x] **Scenario 8: Rejection of currency on non-currency preset during AST validation**
  - **Given** a `ColumnSpec`, `RowSpec`, or `DetailedCellInput` with `format: "percentage"` and `currency: "usd"`
  - **When** `table.validate()` is executed
  - **Then** validation fails with `ModelError::InvalidCurrencyForPreset`.

- [x] **Scenario 9: Rejection of invalid currency code by Serde**
  - **Given** JSON containing `"currency": "invalid_curr"`
  - **When** deserialized into `TableSpec`, `ColumnSpec`, `RowSpec`, or `DetailedCellInput`
  - **Then** deserialization fails with a Serde error listing valid variants.

- [x] **Scenario 10: Descriptor axis isolation suppresses currency and prevents conflicts**
  - **Given** a descriptor column (e.g. `item`) and a measure row with `currency: "eur"`
  - **When** the table is expanded
  - **Then** the descriptor cell resolves `currency: None` and no vector conflict is raised.

- [x] **Scenario 11: Table subtitle deserialization and validation**
  - **Given** a table with `"subtitle": "   "`
  - **When** `table.validate()` is executed
  - **Then** validation fails with `ModelError::EmptyTableSubtitle`.

- [x] **Scenario 12: Rejection of precision exceeding maximum bounds during AST validation**
  - **Given** a `ColumnSpec`, `RowSpec`, or `DetailedCellInput` with `decimals: 15`
  - **When** `table.validate()` is executed
  - **Then** validation fails with `ModelError::PrecisionOutOfBounds` identifying the table and vector/cell context.

---

## Tasks

### Phase 1: Model Subsystem AST & Validation
- [x] Define `CurrencyCode` enum with display and Excel mask helpers in `src/model/format.rs`.
- [x] Add `currency: Option<CurrencyCode>` and builder methods to `TableSpec`, `ColumnSpec`, `RowSpec`, and `DetailedCellInput` in `src/model/table.rs`.
- [x] Add `subtitle: Option<String>` and `with_subtitle` builder method to `TableSpec` in `src/model/table.rs`.
- [x] Add `InvalidCurrencyForPreset`, `EmptyTableSubtitle`, and `PrecisionOutOfBounds` to `src/model/error.rs`.
- [x] Update `TableSpec::validate` to reject empty subtitle, currency on non-currency presets, and precision exceeding 14.

### Phase 2: Compiler IR Subsystem
- [x] Add `currency: Option<CurrencyCode>` and `with_currency` builder method to `CellNode` in `src/compiler/ir/cell.rs`.
- [x] Add `CurrencyConflict` variant to `IrError` in `src/compiler/ir/error.rs`.

### Phase 3: Resolution Algorithm in IR Expander
- [x] Implement `resolve_currency` in `src/compiler/ir/expand.rs` adhering to the 3-tier specificity lattice.
- [x] Enforce preset isolation (non-currency presets resolve `None`).
- [x] Enforce descriptor boundary isolation.
- [x] Wire `resolve_currency` into `expand_table` and `expand_workbook`.

### Phase 4: Verification & Testing
- [x] Add unit tests in `tests/` covering all 12 acceptance scenarios.
- [x] Verify clean compilation: `cargo check`.
- [x] Run full test suite: `cargo test`.
- [x] Run linter: `cargo clippy --all-targets -- -D warnings`.
- [x] Run formatter check: `cargo fmt --check`.
