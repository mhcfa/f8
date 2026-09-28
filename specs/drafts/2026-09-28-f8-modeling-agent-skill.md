---
title: Autonomous Agent Modeling Skill
date: 2026-09-28
status: draft
target-living-spec: specs/living-specs/interfaces/python-sdk.md
---

## Proposal

### What

Package an autonomous agent modeling skill directly within the Python SDK package tree at `python/f8/skills/f8-modeling/` using pure Markdown (`.md`) files. The skill provides progressive disclosure across four structured documents:

1. `SKILL.md`: Root discovery document containing standard YAML frontmatter (`name: f8-modeling`), concise mental model, quickstart, golden rules, and topic routing index.
2. `references/syntax-and-grid.md`: Generic, objective syntax reference guide covering 2D cell coordinates, vector expressions, column roles (`descriptor`, `measure`), section headers, symmetric gutters (`add_row_gutter()`, `add_column_gutter()`), and cell-level indentation (`Cell(indent=N)`).
3. `references/auditing-and-repair.md`: Objective reference guide for `TableAuditor` tabular inspection, value unboxing, Markdown rendering, and structured `F8Error` compiler diagnostics (`CycleDetectedError`, `ReferenceResolutionError`, `FormulaParseError`).
4. `references/patterns.md`: Generic DSL wiring patterns (roll-forward corkscrews, cross-table links, multi-tier subtotals and totals, multi-tier headers) presented as concise structural templates rather than narrative story-driven examples.

### Why

- **Agent context and progressive disclosure**: Autonomous coding assistants (Antigravity, Claude Code, Cursor, Windsurf) need authoritative guidance on declarative 2D grid modeling, vector formulas, cell-level indentation, auditing, and self-healing on structured exceptions. Packaging this guidance as a modular skill with `references/` enables progressive disclosure: agents load a concise `SKILL.md` initially and fetch detailed syntax or repair patterns only when needed, minimizing token bloat.
- **Generic, objective syntax standard**: Authoring tools and agents need formal, standardized syntax definitions, parameter schemas, and semantic rules rather than narrative, domain-specific examples or test-like assertion scripts. Generic syntax definitions allow agents to apply the DSL flexibly across any financial model.
- **Pure Markdown safety and isolation**: Packaging purely static `.md` documentation eliminates security risks (no executable script payloads), prevents user CI and test runner interference (`pytest`, `ruff`, or `pyright` traversing external `.py` files in `.agents/`), and prevents bytecode (`.pyc`) cache pollution.
- **Separation of concerns**: Separates domain modeling instruction authoring from CLI module runner plumbing (`python -m f8`).

### Scope

- **In-scope**:
  - Authoring canonical skill documents (`SKILL.md`, `references/syntax-and-grid.md`, `references/auditing-and-repair.md`, `references/patterns.md`) in `python/f8/skills/f8-modeling/`.
  - Standard YAML frontmatter in `SKILL.md` with `name: f8-modeling` and descriptive intent routing metadata.
  - Standardized, generic syntax specifications, parameter contracts, and concise structural templates.
- **Out-of-scope**:
  - Writing test suites (`python/tests/test_skill_docs.py`) or enforcing arbitrary line count limits.
  - Implementing CLI module runner (`cli.py`, `__main__.py` - covered in `2026-09-28-python-sdk-module-runner.md`).
  - Distributing standalone `.py` or `.sh` script files inside the skill directory.
  - Encoding opinionated accounting doctrines, specific valuation models, or firm-specific line-item taxonomies (reserved for user- or team-level rules).
  - Modifying `crates/f8-cli` or altering Rust CLI commands.

### What Changes

- **New files**:
  - `python/f8/skills/f8-modeling/SKILL.md`: Concise executive summary, YAML frontmatter, mental model, quickstart, golden rules, and topic index.
  - `python/f8/skills/f8-modeling/references/syntax-and-grid.md`: Generic 2D coordinate grammar, vector expression rules, column roles, section headers, gutters, and cell-level indentation.
  - `python/f8/skills/f8-modeling/references/auditing-and-repair.md`: `TableAuditor` API contract, cell inspection, unboxing, and structured exception self-healing patterns.
  - `python/f8/skills/f8-modeling/references/patterns.md`: Generic mechanical DSL wiring templates (roll-forward corkscrews, inter-table links, subtotal/total tiers, multi-tier headers).
- **Modified files**: None.

### Capabilities

- **New capabilities**:
  - Progressive-disclosure skill package ready for distribution inside the `f8` Python package tree.
  - Standardized agent discovery via YAML frontmatter (`name: f8-modeling`).
  - Generic, objective syntax and pattern references for agent consumption.
- **Modified capabilities**: None.

## Design

### Goals

- **Progressive disclosure**: Keep `SKILL.md` focused on orientation and routing to prevent context bloat; detailed mechanics live in modular `references/` documents.
- **Generic and objective specification**: Document syntax, grammar, and parameter signatures generically and objectively without narrative boilerplate or assertions.
- **Pure static documentation**: Zero `.py` or executable scripts inside the skill tree; all guidance is pure Markdown.
- **Engine mechanics over financial doctrine**: Provide authoritative guidance on `f8` compiler invariants and DSL idioms without imposing opinionated accounting choices.

### Invariants

1. **Wheel isolation**: The skill directory contains only `.md` files; zero `.py`, `.sh`, or binary assets.
2. **Valid YAML frontmatter**: `SKILL.md` must begin with valid YAML frontmatter containing `name: f8-modeling` and a non-empty `description`.
3. **Deterministic references**: All internal cross-references within `SKILL.md` link to relative paths under `references/`.
4. **Generic syntax precision**: All syntax definitions strictly adhere to the parser/compiler grammar and SDK type signatures.

### Decisions

- **Single skill with progressive disclosure over multiple skills**:
  - _Rationale_: Coding assistants use semantic description matching. Splitting into multiple skills (`f8-modeling`, `f8-auditing`) causes routing confusion and incomplete context. Progressive disclosure via `references/` allows the agent to load `SKILL.md` initially and read detailed references on demand via `view_file`.
  - _Alternatives considered_: Multiple separate skills in `.agents/skills/`. Rejected to avoid router confusion and workspace directory clutter.
- **Generic syntax specifications over narrative example scripts**:
  - _Rationale_: Narrative examples with full `Model`/`Sheet` boilerplate and `assert` statements clutter documentation and obscure syntax rules. Agents require clear, objective definitions of syntax grammar, parameter schemas, and concise structural templates that can be applied to any domain problem.
  - _Alternatives considered_: Long, narrative end-to-end Python test scripts. Rejected for being verbose, repetitive, and inflexible.
- **Pure Markdown files over executable `.py` scripts**:
  - _Rationale_: Scaffolding `.py` files into a user's workspace exposes them to the user's test runners (`pytest`), linters (`ruff`, `flake8`), and type checkers (`pyright`, `mypy`), which can cause unwanted CI failures. Additionally, shipping executable scripts in package assets raises supply-chain security scrutiny. Markdown provides complete documentation while keeping the footprint 100% inert and safe.
  - _Alternatives considered_: Shipping standalone `.py` script files in `examples/`. Rejected to preserve workspace hygiene, prevent CI breakage, and avoid security scan alarms.

### Risks & Trade-offs

- **Token overhead of references**: Reference files could become too lengthy if verbose.
  - _Mitigation_: Keep references laser-focused on generic grammar and DSL mechanics, using compact tables and minimal structural templates.

### Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Validate YAML frontmatter in SKILL.md**
  - **Given** `python/f8/skills/f8-modeling/SKILL.md`.
  - **When** parsed for YAML frontmatter.
  - **Then** contains `name: f8-modeling` and a non-empty `description` string suitable for agent routing.

- [x] **Scenario 2: Validate absence of non-markdown files**
  - **Given** `python/f8/skills/f8-modeling/` directory tree.
  - **When** traversed for file extensions.
  - **Then** every file matches `*.md` with zero `.py`, `.sh`, or binary files.

- [x] **Scenario 3: Generic and objective syntax reference**
  - **Given** `python/f8/skills/f8-modeling/references/syntax-and-grid.md`.
  - **When** reviewed for coverage and style.
  - **Then** documents 2D coordinates, vector expressions, column roles, section headers, gutters, and cell-level indentation with generic syntax tables and concise structural snippets without test assertions.

- [x] **Scenario 4: Objective auditing and repair reference**
  - **Given** `python/f8/skills/f8-modeling/references/auditing-and-repair.md`.
  - **When** reviewed for coverage and style.
  - **Then** documents `TableAuditor`, value unboxing, Markdown tables, and structured exception handling (`CycleDetectedError`, `ReferenceResolutionError`, `FormulaParseError`) with concise, generic recovery patterns.

- [x] **Scenario 5: Generic modeling patterns reference**
  - **Given** `python/f8/skills/f8-modeling/references/patterns.md`.
  - **When** reviewed for coverage and style.
  - **Then** documents roll-forward corkscrews, cross-table links, multi-tier subtotals and totals, and multi-tier headers using clean, generic structural templates.

- [x] **Scenario 6: Progressive disclosure hierarchy**
  - **Given** `python/f8/skills/f8-modeling/SKILL.md`.
  - **When** reviewed for structure.
  - **Then** provides a concise orientation, core mental model, quickstart, golden rules, and clean relative links to `references/`.

## Tasks

### Phase 1: Author Modular Skill Package

- [x] Update `python/f8/skills/f8-modeling/SKILL.md` to provide a concise orientation, core mental model, quickstart, golden rules, and topic index.
- [x] Refactor `python/f8/skills/f8-modeling/references/syntax-and-grid.md` to use generic syntax tables, concise structural snippets, and remove repetitive boilerplate and assertions.
- [x] Refactor `python/f8/skills/f8-modeling/references/auditing-and-repair.md` to provide an objective API reference for `TableAuditor` and structured exception recovery.
- [x] Refactor `python/f8/skills/f8-modeling/references/patterns.md` to present generic structural modeling templates rather than verbose narrative test scripts.

### Phase 2: Verification

- [x] Verify that all files in `python/f8/skills/f8-modeling/` are pure `.md` files.
- [x] Verify that `SKILL.md` has valid YAML frontmatter and clean relative links to `references/`.
