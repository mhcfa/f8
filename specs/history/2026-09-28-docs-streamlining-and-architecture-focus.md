---
title: Documentation Streamlining and Architecture Focus
date: 2026-09-28
status: completed
target-living-spec: none
---

## Proposal

### What

Streamline and simplify the `docs/` directory to focus strictly on high-level project philosophy, architecture, 2D matrix DSL specification, and CLI authoring. Purge redundant compiler pass docs (`docs/compiler/`), interface manuals (`docs/interfaces/`), and internal function registry design (`docs/functions/registry.md`) which duplicate `specs/living-specs/`, CLI `--help`, and the bundled Python modeling skill in `python/f8/skills/f8-modeling`. Rename `docs/authoring-guide.md` to `docs/cli-authoring-guide.md` and consolidate `docs/codebase-structure.md` into `docs/architecture.md`.

### Why

Having compiler pass documents in `docs/compiler/` and interface guides in `docs/interfaces/` creates a dual source of truth alongside `specs/living-specs/` and `python/f8/skills/`. This introduces a heavy maintenance tax, causing docs to drift out of sync whenever compiler internals, CLI flags, or Python SDK methods change. Purging these redundant internal documents and establishing a clean 3-tier documentation boundary reduces maintenance burden by ~80% and provides unambiguous single sources of truth.

### Scope

- **In-scope**:
  - Merge `docs/codebase-structure.md` into `docs/architecture.md` and delete `docs/codebase-structure.md`.
  - Rename `docs/authoring-guide.md` to `docs/cli-authoring-guide.md` and clarify its scope (CLI and raw JSON authoring).
  - Delete `docs/compiler/` (`dependency-graph.md`, `evaluator.md`, `ir.md`, `lowering-and-emission.md`, `parity.md`, `spatial-layout.md`).
  - Delete `docs/interfaces/` (`cli.md`, `sdk.md`).
  - Delete `docs/functions/registry.md` (keep `docs/functions/catalog.md`).
  - Update cross-references in `docs/dsl/`, `docs/cli-authoring-guide.md`, `docs/functions/catalog.md`, `README.md`, and `AGENTS.md`.
  - Verify link integrity across all markdown files.
- **Out-of-scope**:
  - Modifying Rust compiler crates or Python SDK code.
  - Modifying archived historical specs in `specs/history/`.

### What Changes

- `docs/architecture.md`: Enriched with crate topology, dependency rules, distribution roadmap, and clean documentation map.
- `docs/cli-authoring-guide.md`: Renamed from `docs/authoring-guide.md` with updated links and pointers to `python/f8/skills/f8-modeling`.
- `docs/compiler/`: Deleted.
- `docs/interfaces/`: Deleted.
- `docs/functions/registry.md`: Deleted.
- `docs/codebase-structure.md`: Deleted (merged into `architecture.md`).
- `docs/dsl/formatting.md`, `docs/dsl/spec-format.md`, `docs/functions/catalog.md`: Updated cross-references.
- `README.md`: Updated documentation links.
- `AGENTS.md`: Updated documentation layout guidelines.

### Capabilities

- **New capabilities**: Streamlined, evergreen documentation architecture with single source of truth for compiler passes (`specs/living-specs/`), Python modeling (`python/f8/skills/`), and project architecture (`docs/`).
- **Modified capabilities**: Zero duplicate docs between `docs/` and `specs/`.

---

## Design

### Goals

1. **Strict Single Source of Truth**: Eliminate dual documentation of compiler passes and interfaces.
2. **Evergreen `docs/`**: `docs/` focuses on core philosophy, architecture, and the DSL reference, avoiding low-level implementation details that churn.
3. **Clear Boundary**:
   - `docs/`: Product philosophy, architecture, DSL reference, CLI authoring guide.
   - `python/f8/skills/`: Python financial modeling agent skill and authoring references (bundled in package wheel).
   - `specs/`: Engineering RFCs, living subsystem specifications, and archive.

### Invariants

1. Preserve all DSL reference docs in `docs/dsl/` (`addressing.md`, `formatting.md`, `grammar.md`, `patterns.md`, `spec-format.md`) and standard Excel function catalog (`catalog.md`).
2. Maintain 100% link integrity across all active documentation.

---

## Tasks

- [x] 1. Merge `docs/codebase-structure.md` into `docs/architecture.md`.
- [x] 2. Rename `docs/authoring-guide.md` to `docs/cli-authoring-guide.md` and update its cross-references.
- [x] 3. Purge redundant files: `docs/compiler/`, `docs/interfaces/`, `docs/functions/registry.md`, `docs/codebase-structure.md`.
- [x] 4. Update cross-references in `docs/dsl/` and `docs/functions/catalog.md`.
- [x] 5. Update `README.md` and `AGENTS.md`.
- [x] 6. Run link verification and ensure all cargo/uv checks pass.
- [x] 7. Archive draft spec to `specs/history/`.
