---
title: CLI Model Inspection and Terminal Formatting (f8 inspect)
date: 2026-09-23
status: draft
target-living-spec:
  - specs/living-specs/interfaces/cli.md
  - specs/living-specs/compiler/compiled-model.md
---

## Proposal

### What

Implement the interactive terminal auditing and inspection command suite (`f8 inspect`) in `crates/f8-cli`. This milestone equips financial analysts and autonomous AI coding agents with terminal audit tools:
1. `f8 inspect`: High-level summary overview of workbook structure, cell counts, drivers, and formula ratios.
2. `f8 inspect table`: 2D table grid rendering in terminal Unicode table, GitHub Flavored Markdown, or JSON, featuring formatted accounting numbers, formula auditing, or dual stacked view.
3. `f8 inspect cell`: Cell audit card showing coordinate, physical sheet address, evaluated value, formula AST, lowered Excel formula, precedents, and dependents.
4. `f8 inspect precedents`: Upstream calculation tree with direct and recursive hierarchical rendering (`├──`, `└──`).
5. `f8 inspect dependents`: Downstream blast-radius tree with direct and recursive hierarchical rendering.

To support realistic accounting number formatting in 2D table matrices, this milestone also enriches `TableCellInfo` in `crates/f8-core` with formatting metadata.

### Why

Financial modeling requires continuous auditing of calculation chains, intermediate subtotal values, and downstream impacts. Providing direct terminal formatters and machine-readable JSON exports allows analysts and coding agents to audit models interactively without needing an external spreadsheet application.

### Scope

- **In-scope**:
  - `crates/f8-core`:
    - Enrich `TableCellInfo` with `format: Option<FormatPreset>`, `decimals: Option<u32>`, and `currency: Option<CurrencyCode>`.
    - Update `inspect_table` to populate cell format metadata from cell IR nodes.
    - Reconcile `specs/living-specs/compiler/compiled-model.md` upon completion.
  - `crates/f8-cli/src/render/`:
    - `summary.rs`: Overview card rendering sheets, tables, cell counts, driver inputs, formulas, and execution duration.
    - `table.rs`: 2D matrix renderer supporting:
      - `--view values`: Numbers formatted with accounting typography (`$ 100.0`, `(50.0)`, `15.0%`).
      - `--view formulas`: Live formula expressions for calculations, literals for inputs.
      - `--view both`: Stacked cell view showing both evaluated value and formula.
      - `--format table`: Aligned Unicode box-drawing table (`┌─┬─┐`, `│`).
      - `--format markdown`: GitHub Flavored Markdown table.
      - `--format json` / `--json`: 2D JSON array of cell records.
    - `cell.rs`: Detailed cell audit card.
    - `tree.rs`: Hierarchical tree renderer for direct and recursive DAG edges (`├──`, `└──`).
  - `crates/f8-cli/src/commands/inspect.rs`:
    - Subcommands: `inspect`, `inspect table`, `inspect cell`, `inspect precedents`, `inspect dependents`.
    - Structured `--json` serialization for all subcommands.
  - Integration tests verifying table formatting, markdown tables, cell cards, and recursive tree branches.

- **Out-of-scope**:
  - Interactive full-screen TUI (e.g. mouse scrolling, pane navigation).

### What Changes

- `crates/f8-core/src/compiler/compiled/`:
  - `model.rs`: Add `format`, `decimals`, and `currency` fields to `TableCellInfo`.
  - `mod.rs`: Populate fields in `inspect_table`.
- `crates/f8-cli/src/`:
  - `render/mod.rs`, `render/summary.rs`, `render/table.rs`, `render/cell.rs`, `render/tree.rs`.
  - `commands/inspect.rs`: Inspection subcommand handlers.
  - `cli.rs`: Add `InspectArgs` and nested subcommands to Clap command tree.
- `crates/f8-cli/tests/`:
  - `inspect_test.rs`: Test summary, table formatting, cell card, precedents, and dependents.

### Capabilities

- **New capabilities**:
  - Direct terminal auditing of 2D financial statements with accounting formatting.
  - Fast formula auditing parity (equivalent to Excel `Ctrl + ~`).
  - Visual DAG tree inspection for upstream root causes and downstream blast radius.
  - JSON inspection output across all commands for automated agent tooling.

- **Modified capabilities**:
  - `f8_core::compiler::compiled::TableCellInfo` now provides cell format presets, precision, and currency metadata.

---

## Design

### Goals

1. **Lightweight rendering**: Pure Unicode box-drawing formatters with zero heavy terminal dependencies.
2. **Accounting typography**: Clean alignment, proper negative parentheses, currency symbols, and percentage formatting.
3. **Pure compiler delegation**: Formatters consume structured data contracts directly from `CompiledModel`.
4. **Single source of truth**: Number formatting rules derive from `f8-core` format presets and themes.

### Invariants

1. **Deterministic layout**: Table column widths and alignments adapt deterministically to the longest cell content in each column.
2. **Finite tree recursion**: Precedent and blast-radius tree formatters rely on `f8-core` DAG acyclicity guarantees and cycle detection.

### Decisions

- **Decision 1: Pure Unicode box-drawing formatters over heavy TUI libraries**
  - *Rationale*: Rendering 2D tables and DAG trees using standard Unicode box-drawing characters (`┌─┬─┐`, `│`, `├──`, `└──`) keeps the CLI lightweight, fast, and pipe-friendly. Tables can be cleanly redirected to files or pagers (`cat`, `less`) without terminal initialization overhead.
  - *Alternatives considered*: Heavy TUI frameworks such as `ratatui` (rejected: introduces large dependency tree, breaks standard stdout redirection/piping, unnecessary for inspection commands).

- **Decision 2: Enrich `TableCellInfo` in `f8-core` rather than ad-hoc CLI wrappers**
  - *Rationale*: Preserves the single source of truth (SSOT). All outer interfaces (`f8-cli`, `f8-mcp`, WebAssembly runtimes) require cell format presets, precision, and currency metadata for faithful financial statement rendering. Adding these fields to `TableCellInfo` avoids duplicating AST and IR resolution logic across consumers.
  - *Alternatives considered*: Querying cell IR or raw AST inside CLI inspection handlers (rejected: leaks compiler internal structures and duplicates format synthesis logic).

### Data Contracts

#### Enriched `TableCellInfo`

```rust
pub struct TableCellInfo {
    pub value: CellValue,
    pub is_formula: bool,
    pub formula_expr: Option<String>,
    pub excel_formula: Option<String>,
    pub physical_a1: String,
    pub format: Option<FormatPreset>,
    pub decimals: Option<u32>,
    pub currency: Option<CurrencyCode>,
}
```

#### Inspection Command Hierarchy

```text
f8 inspect [-p <path>] [--json]
f8 inspect table <table_id> [--view <values|formulas|both>] [--format <table|markdown|json>] [--json] [-p <path>]
f8 inspect cell <table_id> <row_id> <col_id> [-p <path>] [--json]
f8 inspect precedents <table_id> <row_id> <col_id> [--recursive] [-p <path>] [--json]
f8 inspect dependents <table_id> <row_id> <col_id> [--recursive] [-p <path>] [--json]
```

### Risks & Trade-offs

- **Wide tables in narrow terminals**: Complex financial statements with many time columns (e.g. 5 historical + 5 projection years) can exceed standard 80- or 120-column terminal displays.
  - *Mitigation*: Format columns deterministically based on maximum cell content width, provide `--format markdown` for flexible rendering, and offer `--json` for machine processing.
- **Deep DAG tree traversal**: Models with deeply nested calculation chains could produce verbose recursive tree displays.
  - *Mitigation*: DAG acyclicity is proven in Pass 3, guaranteeing tree traversals will terminate. Tree inspection displays immediate precedents/dependents by default; deep traversal requires explicit `--recursive`.

---

## Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Model summary inspection**
  - **Given** a compiled model
  - **When** running `f8 inspect`
  - **Then** human summary listing sheets, tables, cell counts, drivers, and formulas is output to `stdout`.
  - **When** running `f8 inspect --json`
  - **Then** `ModelSummary` JSON object is printed.

- [ ] **Scenario 2: Table matrix inspection with accounting formatting**
  - **Given** a compiled table with currency numbers
  - **When** running `f8 inspect table IncomeStatement --view values`
  - **Then** cells are formatted with accounting symbols (e.g. `$ 100.0`, `(50.0)`, `15.0%`) in a clean Unicode table.

- [ ] **Scenario 3: Table matrix formula view and markdown output**
  - **Given** a compiled table
  - **When** running `f8 inspect table IncomeStatement --view formulas --format markdown`
  - **Then** a GitHub Flavored Markdown table is output with formula expressions in calculation cells.

- [ ] **Scenario 4: Cell inspection card**
  - **Given** a compiled model
  - **When** running `f8 inspect cell IncomeStatement total_opex 2028E`
  - **Then** coordinate, physical sheet address, evaluated value, formula AST, lowered Excel formula, precedents, and dependents are printed.

- [ ] **Scenario 5: Precedent calculation tree**
  - **Given** a multi-tier calculation cell
  - **When** running `f8 inspect precedents IncomeStatement net_income 2024E --recursive`
  - **Then** hierarchical box-drawing tree of upstream calculation dependencies is printed.

- [ ] **Scenario 6: Blast-radius calculation tree**
  - **Given** an operating driver cell
  - **When** running `f8 inspect dependents Assumptions rev_growth 2024E --recursive`
  - **Then** hierarchical box-drawing tree of downstream affected cells is printed.

- [ ] **Scenario 7: Zero production panics**
  - **Given** `crates/f8-cli`
  - **When** running `cargo clippy -p f8-cli --all-targets -- -D warnings`
  - **Then** zero warnings and zero unhandled panics (`unwrap`/`expect`) are found.

---

## Tasks

### Phase 1: Core Facade Enrichment
- [ ] Add `format`, `decimals`, and `currency` fields to `TableCellInfo` in `crates/f8-core/src/compiler/compiled/model.rs`.
- [ ] Populate fields in `inspect_table` in `crates/f8-core/src/compiler/compiled/mod.rs`.
- [ ] Update unit tests in `crates/f8-core/tests/compiled_model_test.rs`.

### Phase 2: Terminal Formatters & Renderers
- [ ] Implement `render/summary.rs` for model overview card.
- [ ] Implement `render/table.rs` with accounting typography formatting, column width calculation, Unicode box-drawing, and markdown generator.
- [ ] Implement `render/cell.rs` for cell audit card.
- [ ] Implement `render/tree.rs` for recursive DAG branch rendering (`├──`, `└──`).

### Phase 3: Inspection Subcommands & Dispatcher
- [ ] Implement `commands/inspect.rs` connecting `CompiledModel` methods to renderers.
- [ ] Wire `InspectArgs` and nested subcommands into Clap in `cli.rs`.
- [ ] Implement `--json` serialization for all inspection subcommands.

### Phase 4: Verification & Living Spec Reconciliation
- [ ] Add integration tests in `crates/f8-cli/tests/inspect_test.rs`.
- [ ] Reconcile `specs/living-specs/compiler/compiled-model.md` and `specs/living-specs/interfaces/cli.md`.
- [ ] Verify `cargo check`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --check`.
