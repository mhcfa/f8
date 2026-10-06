---
title: Documentation Suite Audit & Synchronization
date: 2026-09-28
status: completed
target-living-spec: none
---

## Proposal

### What

Perform a comprehensive audit, synchronization, and structural reorganization of the `docs/` directory to achieve 100% alignment with the `f8` codebase across Rust compiler crates (`f8-core`, `f8-cli`, `f8-python`) and the Python SDK (`f8`). Decompose outdated descriptions, reconcile missing subcommands and flags, fix broken cross-references, and eliminate obsolete heuristics and legacy artifacts.

### Why

Over recent development cycles, several major architectural milestones were delivered:
1. **Logical Table IDs in SheetSpec**: Replaced filesystem file paths with programmatic `TableId`s and eliminated filename-stem heuristics (`matches_table_id`).
2. **Python SDK & Native PyO3 Extension**: Added `crates/f8-python` and the pure Python `f8` modeling package (`Model`, `Sheet`, `Table`, `Cell`, `TableAuditor`, `compile_project`, `export_project`).
3. **CLI Auditing Polish & Inspection Commands**: Added `f8 inspect sheet`, expanded `--view` modes (`expr`, `addresses`), and enriched cell cards and dependency trees with evaluated value annotations.
4. **Cell-Level Presentation & Formatting**: Refined 0D cell indentation, cell-level notes, and formatted values.

However, several documents in `docs/` still contain outdated assumptions (e.g. `f8-python` described as "Planned", obsolete table file path matching in authoring guides, missing CLI inspection subcommands, broken anchor links, and diverging trait signatures). Auditing and syncing `docs/` ensures both human modelers and autonomous AI agents have an authoritative, accurate, and modular reference.

### Scope

- **In-scope**:
  - Audit and synchronize all 19 documents under `docs/`.
  - Update `docs/architecture.md` and `docs/codebase-structure.md` to reflect current crate topology (including `f8-python` and `python/f8`), delivered milestones, and clean I/O boundaries.
  - Update `docs/interfaces/cli.md` to document `f8 inspect sheet`, full `--view` options (`values`, `formulas`, `expr`, `addresses`, `both`), blank template unpacking, and latest output card formats.
  - Update `docs/interfaces/sdk.md` from a forward-looking proposal/roadmap into an authoritative living subsystem specification reflecting current Python SDK implementation (`TableAuditor`, `Cell`, `export_project`, `compile_project`, `to_markdown`, `to_dict`).
  - Purge obsolete file path matching and filename-stem rules from `docs/authoring-guide.md` and `docs/dsl/spec-format.md`.
  - Fix broken markdown links and anchors across `docs/` (including `docs/dsl/formatting.md`).
  - Align code signatures in compiler docs (e.g. `CycleHandler` in `docs/compiler/dependency-graph.md`).
  - Purge obsolete `docs/feedback/` directory as all proposed capabilities (CLI inspection modes, Python SDK) are fully delivered.
  - Execute automated link and anchor verification scripts across `docs/`.
- **Out-of-scope**:
  - Modifying Rust code in `crates/` or Python code in `python/`.
  - Modifying historical specifications in `specs/history/`.

### What Changes

- `docs/codebase-structure.md`: Add `crates/f8-python` and `python/` to directory layout and dependency diagrams; update Layer 5 to mark `f8-python` as delivered; update delivery roadmap.
- `docs/architecture.md`: Position Python SDK as primary authoring layer alongside CLI; align adapter I/O boundaries.
- `docs/interfaces/cli.md`: Document `f8 inspect sheet`; update `f8 inspect table` view modes (`expr`, `addresses`); update cell card, precedent tree, and dependent tree output snippets.
- `docs/interfaces/sdk.md`: Convert future roadmap sections into active API reference; document `TableAuditor` Markdown and dictionary export protocols.
- `docs/authoring-guide.md`: Remove obsolete filename-stem matching and relative path table references; reflect logical table IDs in manifest schemas.
- `docs/dsl/spec-format.md`: Harmonize Section 1 and Section 2 invariants around logical `TableId`s; fix heading anchors.
- `docs/dsl/formatting.md`: Correct anchor links pointing to `spec-format.md`.
- `docs/compiler/dependency-graph.md`: Align `CycleHandler` trait definition with `crates/f8-core/src/compiler/graph/cycle.rs`.
- `docs/compiler/lowering-and-emission.md`: Document `--omit-cache` and workbook package metadata options (`title`, `author`).
- `docs/feedback/`: Purge obsolete feedback document as all proposed capabilities (sheet inspection, address view, Python SDK) are fully implemented.

### Capabilities

- **New capabilities**: Complete, verified, and synchronized documentation suite across CLI, Python SDK, DSL, and compiler internals.
- **Modified capabilities**: Zero stale or misleading architecture guidance in `docs/`.

---

## Design

### Goals

1. **100% Codebase Parity**: Every documented command, option, trait, class, schema, and API must exactly match current implementation.
2. **Strict Single Source of Truth**: Eliminate conflicting rules between `docs/dsl/spec-format.md`, `docs/authoring-guide.md`, and code.
3. **100% Link and Anchor Integrity**: Zero broken relative links or missing heading slugs across all markdown documents.
4. **Concise and Objective Tone**: Adhere strictly to the concise, sentence-case, objective documentation guidelines defined in `AGENTS.md`.

### Invariants

1. **Orthogonal 2D Matrix Paradigm**: All explanations and examples must preserve `Table[row, col]`, `@col`, `@row`, `col.prev`, and `row.prev`.
2. **I/O Purity Boundary**: `f8-core` is purely in-memory; filesystem I/O belongs exclusively to outer adapters (`f8-cli`, `f8` Python package loader/exporter).
3. **Logical Table IDs**: Worksheets reference tables strictly via programmatic `TableId` (`tables: ["IncomeStatement"]`), never filesystem paths.

### Decisions

- **Decision 1: Phased Audit Execution**: Group the audit into 5 sequential phases (Architecture $\to$ Interfaces $\to$ DSL $\to$ Compiler $\to$ Verification).
  - *Rationale*: Allows incremental, independently verifiable updates without overwhelming context or introducing cross-document regressions.
- **Decision 2: Purge obsolete `docs/feedback/` directory**: Delete `agent-experience-and-enhancements.md` now that its proposals (CLI inspection modes, Python SDK) are fully implemented and documented in living specs.
  - *Rationale*: Eliminates stale proposal documents from `docs/`, keeping `docs/` focused exclusively on canonical architecture, DSL specifications, and active interfaces.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Codebase structure reflects workspace crates and Python package**
  - **Given** `docs/codebase-structure.md`
  - **When** checking directory layout, architectural layers, and dependency diagrams
  - **Then** `crates/f8-python` and `python/f8` are documented as active workspace components and `f8-python` is marked as delivered in Layer 5.

- [x] **Scenario 2: CLI documentation reflects all subcommands and view options**
  - **Given** `docs/interfaces/cli.md`
  - **When** checking command hierarchy and inspect options
  - **Then** `f8 inspect sheet` is fully documented, `--view` lists all 5 modes (`values`, `formulas`, `expr`, `addresses`, `both`), and output snippets match current renderer formatting.

- [x] **Scenario 3: Python SDK documentation reflects current implementation**
  - **Given** `docs/interfaces/sdk.md`
  - **When** checking SDK design and capabilities
  - **Then** `TableAuditor`, `Cell` dataclass, `compile_project`, `export_project`, coordinate indexing (`table[row, col]`), and Markdown rendering are documented as delivered capabilities.

- [x] **Scenario 4: Authoring guide and spec format use logical table IDs**
  - **Given** `docs/authoring-guide.md` and `docs/dsl/spec-format.md`
  - **When** inspecting project architecture, manifest schemas, and invariants
  - **Then** `tables` arrays contain logical `TableId`s, and obsolete filename-to-ID matching and relative file path rules are removed.

- [x] **Scenario 5: Trait signatures and emission options in compiler docs match code**
  - **Given** `docs/compiler/dependency-graph.md` and `docs/compiler/lowering-and-emission.md`
  - **When** comparing code snippets and emission options with `f8-core`
  - **Then** `CycleHandler::handle_cycles` matches `cycle.rs`, and `--omit-cache` is documented.

- [x] **Scenario 6: Zero broken links or anchors across docs/**
  - **Given** all markdown files in `docs/`
  - **When** running automated link and anchor verification scripts
  - **Then** all relative file links and heading anchors resolve with zero errors.

---

## Tasks

### Phase 1: Architecture & Codebase Structure Alignment
- [x] Update `docs/codebase-structure.md` directory layout to include `crates/f8-python`, `python/f8`, and `python/tests`.
- [x] Update Layer 5 in `docs/codebase-structure.md` to classify `f8-python` as delivered rather than planned.
- [x] Update dependency diagrams and roadmap in `docs/codebase-structure.md`.
- [x] Update `docs/architecture.md` to position Python SDK as primary authoring tier alongside CLI and clarify adapter I/O boundaries.

### Phase 2: Interface Documentation Synchronization (CLI & SDK)
- [x] Update `docs/interfaces/cli.md` command hierarchy to include `f8 inspect sheet`.
- [x] Add dedicated documentation section for `f8 inspect sheet` (arguments, options, view modes, examples).
- [x] Update `f8 inspect table` `--view` modes to include `expr` and `addresses`.
- [x] Update `f8 inspect cell`, `precedents`, and `dependents` sample outputs to match current renderer format (notes, types, evaluated values in trees).
- [x] Document `templates/blank/` template in `f8 init`.
- [x] Update `docs/interfaces/sdk.md` to document `TableAuditor` methods (`to_markdown`, `to_dict`, `to_string`, `cell`, `value`, `formula`, `expr`, `address`, `__getitem__`).
- [x] Update `docs/interfaces/sdk.md` Section 6 from a future roadmap to active delivered capabilities.

### Phase 3: DSL, Spec Formats & Authoring Guide Alignment
- [x] Remove obsolete filename-stem matching and file-path reference rules from `docs/authoring-guide.md`.
- [x] Remove obsolete filename-to-ID correspondence and file-path references from `docs/dsl/spec-format.md` Section 1.
- [x] Fix broken anchor links in `docs/dsl/formatting.md` targeting `spec-format.md`.
- [x] Review `docs/dsl/addressing.md`, `grammar.md`, and `patterns.md` for consistency.

### Phase 4: Compiler Passes, Function Catalog & Feedback Reconciliation
- [x] Update `CycleHandler` trait signature in `docs/compiler/dependency-graph.md` to match `crates/f8-core/src/compiler/graph/cycle.rs`.
- [x] Update `docs/compiler/lowering-and-emission.md` to document `--omit-cache` and `EmissionOptions`.
- [x] Purge obsolete `docs/feedback/agent-experience-and-enhancements.md` after verifying all proposals are implemented.

### Phase 5: Verification & Link Integrity
- [x] Run automated relative link validator script across `docs/` and verify 0 broken links.
- [x] Run automated markdown anchor validator script across `docs/` and verify 0 broken anchors.
- [x] Verify `AGENTS.md` documentation style guidelines (sentence case, clean formatting, concise language).
