---
title: Documentation Suite Reorganization & Cleanup
date: 2026-09-17
status: completed
target-living-spec: none
---

## Proposal

### What

Reorganize and modularize the Finx documentation suite under `docs/`. Decompose monolithic documents, eliminate duplicated content, retire the RFC proposal artifact (`docs/unified-matrix-model.md`), and ensure every document focuses strictly on a single concern with clean boundaries.

### Why

1. **Single source of truth**: Having both `docs/unified-matrix-model.md` and `docs/dsl/spec-format.md` duplicate identical table schemas and modeling patterns violates the single source of truth principle and risks divergent documentation.
2. **Separation of concerns & readability**: `docs/dsl/spec-format.md` previously spanned 701 lines and mixed project manifests, table schemas, 5 financial archetypes, column/row grouping, semantic formatting, and an end-to-end model project. Splitting it into single-topic documents (~65–280 lines each) drastically improves local reasoning and navigation for humans and AI agents.
3. **Purity of living documentation**: Retiring `docs/unified-matrix-model.md` removes migration checklists, old-vs-new comparison tables, and implementation Rust structs from user/agent architectural guides.
4. **Authoritative compiler IR**: Relocating the 2D Specificity Resolution Lattice algorithm and Mermaid flowchart into `docs/compiler/ir.md` establishes `ir.md` as the definitive single source of truth for grid expansion and specificity evaluation.

### Scope

- **In-scope**:
  - Integrate the 2D Specificity Resolution Lattice (algorithm, flowchart, format conflict, notes) into `docs/compiler/ir.md`.
  - Decompose `docs/dsl/spec-format.md` into focused modules:
    - `docs/dsl/spec-format.md`: Core manifest and table schemas, axis roles, cell input syntax, and invariants.
    - `docs/dsl/groups.md`: Column super-headers and hierarchical row groups.
    - `docs/dsl/formatting.md`: Semantic format preset catalog, conflict rules, and themes.
    - `docs/dsl/patterns.md`: Canonical 2D modeling archetypes (Income Statement, Cap Table, Cross-Footing, Comps, Debt Amortization).
    - `docs/dsl/project-example.md`: Complete 3-file working reference model.
  - Retire `docs/unified-matrix-model.md`.
  - Reconcile links in `docs/architecture.md` and across the documentation suite.
- **Out-of-scope**:
  - Modifying Rust code in `src/`.
  - Modifying historical specs in `specs/history/`.

### What Changes

- `docs/compiler/ir.md`: Add detailed 2D Specificity Resolution Lattice flowchart, payload evaluation algorithm, format conflict rules, and note inheritance.
- `docs/dsl/spec-format.md`: Refactor to focus purely on project and table specification schemas and core invariants.
- `docs/dsl/groups.md`: New document specifying column and row groups.
- `docs/dsl/formatting.md`: New document specifying semantic format presets, conflict resolution, and pluggable themes.
- `docs/dsl/patterns.md`: New document specifying canonical 2D modeling patterns.
- `docs/dsl/project-example.md`: New document specifying the end-to-end 3-file reference model.
- `docs/unified-matrix-model.md`: Removed.
- `docs/architecture.md`: Updated Subsystem Documentation directory to index all new modular docs.

### Capabilities

- **New capabilities**: Clean modular navigation across DSL features; each document is self-contained and focused on a single topic.
- **Modified capabilities**: No loss of technical documentation; all algorithms, diagrams, and schemas are preserved in their respective single source of truth.

---

## Design

### Goals

1. **Strict single responsibility**: Every document under `docs/` focuses on one architectural topic, targeting 60–200 lines.
2. **Zero duplication**: Eliminate redundant schemas, archetype examples, and RFC comparison tables.
3. **100% link integrity**: All internal relative links resolve accurately.

### Invariants

1. **No backward-compatibility sections**: Maintain zero legacy comparison or migration sections in living docs.
2. **Symmetrical 2D model**: All examples and explanations preserve the unified 2D matrix model (`Table[row, col]`, `@col`, `@row`, `col.prev`, `row.prev`).

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: `docs/compiler/ir.md` contains the 2D resolution lattice**
  - **Given** `docs/compiler/ir.md`
  - **When** inspected for specificity resolution
  - **Then** contains the complete resolution flowchart, step-by-step payload evaluation, format conflict rules, and note inheritance.

- [x] **Scenario 2: `docs/dsl/spec-format.md` is concise and focused**
  - **Given** `docs/dsl/spec-format.md`
  - **When** inspected for scope and line count
  - **Then** contains manifest and table schemas, axis roles, cell input syntax, invariants, and is under 200 lines without inlined patterns or group walkthroughs.

- [x] **Scenario 3: Groups, formatting, patterns, and examples are modularized**
  - **Given** `docs/dsl/`
  - **When** inspecting directory contents
  - **Then** contains `groups.md`, `formatting.md`, `patterns.md`, and `project-example.md`, each with dedicated, focused content.

- [x] **Scenario 4: `docs/unified-matrix-model.md` is retired**
  - **Given** the repository file tree
  - **When** checking for `docs/unified-matrix-model.md`
  - **Then** the file does not exist.

- [x] **Scenario 5: Documentation links and cross-references are valid**
  - **Given** all markdown files in `docs/` and `specs/`
  - **When** validating relative file links
  - **Then** all internal markdown links resolve to existing files.

---

## Tasks

### Phase 1: Enrich Compiler IR Documentation
- [x] Update `docs/compiler/ir.md` with the 2D Specificity Resolution Lattice flowchart, payload algorithm, format resolution, and note precedence.

### Phase 2: Modularize DSL Documentation
- [x] Create `docs/dsl/groups.md` (Column groups and nested row groups).
- [x] Create `docs/dsl/formatting.md` (Format presets, conflict resolution, themes).
- [x] Create `docs/dsl/patterns.md` (5 modeling archetypes).
- [x] Create `docs/dsl/project-example.md` (3-file reference model).
- [x] Refactor `docs/dsl/spec-format.md` to focus strictly on schemas and invariants (~150-180 lines).

### Phase 3: Retire RFC Artifact & Reconcile Links
- [x] Delete `docs/unified-matrix-model.md`.
- [x] Update `docs/architecture.md` Subsystem Documentation index.
- [x] Update cross-references in other docs (`docs/interfaces/mcp.md`, etc.).

### Phase 4: Verification & Archive
- [x] Verify zero broken internal links across `docs/` and `specs/`.
- [x] Verify file lengths and formatting standards (`AGENTS.md`).
- [x] Archive draft spec to `specs/history/2026-09-17-docs-reorganization-and-cleanup.md`.
