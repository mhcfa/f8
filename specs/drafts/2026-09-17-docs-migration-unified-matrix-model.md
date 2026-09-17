---
title: Unified 2D Matrix Model Documentation Migration & Legacy Purge
date: 2026-09-17
status: draft
target-living-spec: none
---

## Proposal

### What

Migrate the entire documentation suite under `docs/` to reflect the **Unified 2D Matrix Model** specified in [`docs/unified-matrix-model.md`](../../docs/unified-matrix-model.md). Completely purge all obsolete conceptual baggage, legacy schemas (`match` blocks, `value` arrays, `RowSpec.label`), hardcoded presentation assumptions ("Columns A & B" stubs), and outdated reference tokens (`@` to `@col`, adding `@row`).

### Why

1. **Single source of truth**: Outdated or contradictory documentation misleads AI agents and human developers, causing them to hallucinate deprecated patterns (such as generating nested `match` blocks or assuming phantom layout stubs).
2. **Greenfield rigor**: Per `AGENTS.md`, Finx maintains clean separation of concerns with zero backward-compatibility shims, deprecated aliases, or legacy baggage. The documentation must present the Unified 2D Matrix Model as the native, standard paradigm.
3. **Execution clarity**: Upfront documentation reconciliation provides an unambiguous specification contract for the upcoming code implementation passes (parser updates, model AST refactoring, grid expansion rewrite).

### Scope

- **In-scope**:
  - [`docs/dsl/spec-format.md`](../../docs/dsl/spec-format.md): Replace the entire spec schema with `ColumnSpec`, `AxisRole`, `RowSpec.cells` (1-to-1 typed inputs), vector rules (`row.expr`, `col.expr`), explicit `null`, and format resolution. Purge all references to `match`, `value` arrays, and `RowSpec.label`.
  - [`docs/dsl/addressing.md`](../../docs/dsl/addressing.md): Update active evaluation tokens from `@` to `@col`, add active row token `@row`, add vertical temporal offsets (`row.prev`, `row.next`), and update all examples.
  - [`docs/dsl/grammar.md`](../../docs/dsl/grammar.md): Update EBNF grammar to formally include `@col`, `@row`, `row.prev`, and `row.next`.
  - [`docs/compiler/spatial-layout.md`](../../docs/compiler/spatial-layout.md): Purge the hardcoded "Columns A & B (Row Stubs)" rule; document dynamic column layout mapping table columns $0 \dots C-1$ directly, anchored by `role: "descriptor"` columns.
  - [`docs/compiler/ir.md`](../../docs/compiler/ir.md): Document cell IR unrolling using the 3-tier specificity lattice; confirm descriptor cells are registered calculation/literal nodes in `CellGrid`.
  - [`docs/compiler/lowering-and-emission.md`](../../docs/compiler/lowering-and-emission.md): Update coordinate lowering rules to use `@col` instead of bare `@`, add lowering rules for `@row`, `row.prev`, and `row.next`.
  - [`docs/compiler/parity.md`](../../docs/compiler/parity.md): Update formula examples from bare `@` to canonical `@col`.
  - [`docs/architecture.md`](../../docs/architecture.md): Reconcile the 3-tier separation of concerns, 2D matrix paradigm, and pipeline descriptions to reflect explicit axis roles and vector rules.
  - [`docs/interfaces/mcp.md`](../../docs/interfaces/mcp.md): Purge references to `match` boilerplate absorption; simplify tool descriptions to reflect concise 2D authoring.
- **Out-of-scope**:
  - Modifying Rust source files in `src/` (to be executed in subsequent code implementation draft specs).
  - Editing completed archive logs in `specs/history/`.

### What Changes

- `docs/dsl/spec-format.md`: Complete rewrite to document `TableSpec`, `ColumnSpec`, `RowSpec`, `AxisRole`, `CellInput`, vector rules, and 2D conflict resolution.
- `docs/dsl/addressing.md`: Reconcile all addressing modes to use `@col` and `@row`.
- `docs/dsl/grammar.md`: Update EBNF production rules for `RowSpec`, `ColSpec`, `SpecialRow`, and `SpecialCol`.
- `docs/compiler/spatial-layout.md`: Rewrite grid structure section to reflect dynamic, descriptor-anchored column layout.
- `docs/compiler/ir.md`: Update grid expansion pass and cell node descriptions.
- `docs/compiler/lowering-and-emission.md`: Lowering rules table updated for `@col`, `@row`, `row.prev`, and `row.next`.
- `docs/compiler/parity.md`: Formula parity examples updated to use canonical `@col`.
- `docs/architecture.md`: Update architecture diagrams and table abstractions.
- `docs/interfaces/mcp.md`: Update MCP authoring tool signatures.

### Capabilities

- **New capabilities**:
  - Documentation fully details bidirectional vector rules (`column.expr` and `row.expr`), explicit axis roles (`measure` vs `descriptor`), 1-to-1 typed cell inputs, and `@row`/`@col` addressing.
- **Modified capabilities**:
  - All examples across `docs/` reflect canonical 2D addressing: `Table[row, col]` with quoted keys and symmetrical `@col`/`@row` tokens.
  - Eliminates all obsolete documentation references to `match` blocks, `value` arrays, and phantom A/B stubs.

---

## Design

### Goals

1. **Zero legacy residue**: Ensure zero occurrences of purged constructs (`match` arms, `RowSpec.label`, Column A/B stubs) remain in `docs/`.
2. **Universal consistency**: Every documentation file aligns 100% with [`docs/unified-matrix-model.md`](../../docs/unified-matrix-model.md).
3. **Clean relative links**: Maintain valid cross-references across all markdown files.

### Invariants

1. **No backward-compatibility sections**: Do not include "Migration from old syntax" or "Deprecated" sections in living documentation. Present the unified matrix model as the sole architecture.
2. **Mandatory 2D addressing**: All formula examples must use `Table[RowSelector, ColSelector]` with strictly quoted coordinate keys.
3. **Deterministic resolution**: All docs must consistently explain the 3-tier specificity hierarchy:
   $$\text{Cell Override (0D Point)} > (\text{Row Rule } \mathbf{XOR} \text{ Column Rule}) > \text{Explicit Null}$$

---

## Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Spec format documentation reflects unified 2D matrix**
  - **Given** [`docs/dsl/spec-format.md`](../../docs/dsl/spec-format.md)
  - **When** inspected for table schema and authoring patterns
  - **Then** contains `ColumnSpec`, `RowSpec`, `AxisRole`, `CellInput`, vector rules, and zero references to `match` or `RowSpec.label`.

- [ ] **Scenario 2: Addressing documentation reflects symmetrical `@col` and `@row`**
  - **Given** [`docs/dsl/addressing.md`](../../docs/dsl/addressing.md)
  - **When** inspected for coordinate syntax
  - **Then** uses `@col` for active columns, `@row` for active rows, and includes `row.prev`/`row.next` without bare `@`.

- [ ] **Scenario 3: Formula grammar reflects updated tokens**
  - **Given** [`docs/dsl/grammar.md`](../../docs/dsl/grammar.md)
  - **When** inspected for EBNF definitions
  - **Then** `RowSpec` includes `"@row"` and `SpecialRow` includes `row.prev`/`row.next`, while `ColSpec` uses `"@col"`.

- [ ] **Scenario 4: Spatial layout reflects dynamic column mapping**
  - **Given** [`docs/compiler/spatial-layout.md`](../../docs/compiler/spatial-layout.md)
  - **When** inspected for column layout rules
  - **Then** physical columns map 1:1 to table columns without hardcoded Columns A/B stubs, using `role: "descriptor"` as the outline anchor.

- [ ] **Scenario 5: Architecture and IR documentation reflect first-class descriptor nodes**
  - **Given** [`docs/architecture.md`](../../docs/architecture.md) and [`docs/compiler/ir.md`](../../docs/compiler/ir.md)
  - **When** inspected for IR nodes and expansion passes
  - **Then** all cells (both descriptor labels and measure values) are documented as first-class `CellId` nodes in `CellGrid`.

- [ ] **Scenario 6: MCP documentation purges match boilerplate references**
  - **Given** [`docs/interfaces/mcp.md`](../../docs/interfaces/mcp.md)
  - **When** inspected for tool roles
  - **Then** all references to expanding or absorbing `match` blocks are removed.

- [ ] **Scenario 7: Lowering and parity documentation reflects unified 2D addressing**
  - **Given** [`docs/compiler/lowering-and-emission.md`](../../docs/compiler/lowering-and-emission.md) and [`docs/compiler/parity.md`](../../docs/compiler/parity.md)
  - **When** inspected for formula syntax and lowering rules
  - **Then** coordinate lowering table specifies `@col`, `@row`, `row.prev`, and `row.next` without bare `@`, and parity examples use canonical `@col`.

- [ ] **Scenario 8: Repository audit verifies zero stale keywords in `docs/`**
  - **Given** the entire `docs/` directory (excluding `unified-matrix-model.md`)
  - **When** running ripgrep for `"match_arms"`, `"RowSpec.label"`, `"match partition"`, `"Columns A & B"`, bare active column references (`", @]"`, `"[@]"`), legacy `"match":` blocks, and positional `"value": [` arrays
  - **Then** returns zero matches.

---

## Tasks

### Phase 1: DSL Documentation Migration
- [ ] Update [`docs/dsl/spec-format.md`](../../docs/dsl/spec-format.md):
  - [ ] Replace root JSON schema with `TableSpec`, `ColumnSpec`, `RowSpec`, `AxisRole`, `CellInput`.
  - [ ] Document vector rules (`column.expr`, `row.expr`) and 1-to-1 typed cell overrides.
  - [ ] Replace all authoring patterns (Income Statement, Cap Table, Debt Schedule, Comps) with clean 2D syntax.
  - [ ] Purge all sections, tables, and invariants referencing `match` blocks or `RowSpec.label`.
- [ ] Update [`docs/dsl/addressing.md`](../../docs/dsl/addressing.md):
  - [ ] Replace bare `@` with `@col`.
  - [ ] Add `@row` active row coordinate specification and examples.
  - [ ] Add `row.prev` and `row.next` temporal row offsets.
  - [ ] Update addressing flowchart and all example expressions.
- [ ] Update [`docs/dsl/grammar.md`](../../docs/dsl/grammar.md):
  - [ ] Update EBNF grammar rules for `RowSpec`, `ColSpec`, `SpecialRow`, and `SpecialCol`.
  - [ ] Update lexical rules and references.

### Phase 2: Compiler & Architecture Documentation Migration
- [ ] Update [`docs/compiler/spatial-layout.md`](../../docs/compiler/spatial-layout.md):
  - [ ] Purge "Columns A & B (Row Stubs)" section.
  - [ ] Document 1:1 physical sheet mapping from declared `columns`.
  - [ ] Document descriptor column role as the anchor for row group outlines and indentation.
- [ ] Update [`docs/compiler/ir.md`](../../docs/compiler/ir.md):
  - [ ] Document grid expansion using the 3-tier specificity lattice.
  - [ ] Confirm descriptor cells are full `CellNode` entries in `CellGrid`.
- [ ] Update [`docs/compiler/lowering-and-emission.md`](../../docs/compiler/lowering-and-emission.md):
  - [ ] Update coordinate lowering rules table from bare `@` to `@col`.
  - [ ] Add coordinate lowering rules for `@row`, `row.prev`, and `row.next`.
- [ ] Update [`docs/compiler/parity.md`](../../docs/compiler/parity.md):
  - [ ] Replace bare `@` in unrolled formula examples with `@col`.
- [ ] Update [`docs/architecture.md`](../../docs/architecture.md):
  - [ ] Update structural hierarchy and 3-tier separation of concerns table.
  - [ ] Reconcile pipeline pass descriptions with the 2D matrix model.

### Phase 3: Interface Documentation Migration
- [ ] Update [`docs/interfaces/mcp.md`](../../docs/interfaces/mcp.md):
  - [ ] Purge references to `match` boilerplate absorption.
  - [ ] Reconcile tool descriptions for `finx_create_statement`, `finx_create_table`, and `finx_add_row`.

### Phase 4: Verification & Audit
- [ ] Run ripgrep audit across `docs/` to guarantee zero stale terms (`"match_arms"`, `"RowSpec.label"`, `"match":`, `"value": [`, `", @]"`, `"[@]"`, `"Columns A & B"`).
- [ ] Verify all internal relative markdown links resolve correctly.
