---
title: Autonomous Agent Modeling Skill
date: 2026-09-28
status: draft
target-living-spec: specs/living-specs/interfaces/python-sdk.md
---

## Proposal

### What

Package an autonomous agent modeling skill directly within the Python SDK package tree at `python/f8/skills/f8-modeling/` using pure Markdown (`.md`) files. The skill provides progressive disclosure across four structured documents:

1. `SKILL.md`: Root discovery document containing standard YAML frontmatter (`name: f8-modeling`), concise mental model, quickstart, golden rules, and topic routing index (< 120 lines).
2. `references/syntax-and-grid.md`: Reference guide for 2D cell coordinates, vector expressions, column roles (`label`, `meta`, `measure`), section headers, symmetric gutters (`add_row_gutter()`, `add_column_gutter()`), and cell-level indentation (`Cell(indent=N)`).
3. `references/auditing-and-repair.md`: Reference guide for `TableAuditor` tabular inspection, value unboxing, Markdown rendering, and self-healing recovery patterns for structured `F8Error` compiler diagnostics (`CycleDetectedError`, `ParseError`, `CompileError`).
4. `references/patterns.md`: Canonical financial modeling DSL wiring patterns (roll-forward corkscrews with `PREV`, cross-table links, multi-tier subtotals and totals, multi-tier headers).

### Why

- **Agent context and progressive disclosure**: Autonomous coding assistants (Antigravity, Claude Code, Cursor, Windsurf) need authoritative guidance on declarative 2D grid modeling, vector formulas, cell-level indentation, auditing, and self-healing on structured exceptions. Packaging this guidance as a modular skill with `references/` enables progressive disclosure: agents load a concise `SKILL.md` initially and fetch detailed syntax or repair patterns only when needed, minimizing token bloat.
- **Pure Markdown safety and isolation**: Packaging purely static `.md` documentation eliminates security risks (no executable script payloads), prevents user CI and test runner interference (`pytest`, `ruff`, or `pyright` traversing external `.py` files in `.agents/`), and prevents bytecode (`.pyc`) cache pollution.
- **Executable correctness**: Embedded Python DSL code snippets must serve as tested, copy-pasteable references that compile and execute with 100% fidelity against the `f8` compiler engine.
- **Separation of concerns**: Separates domain modeling instruction authoring from CLI module runner plumbing (`python -m f8`).

### Scope

- **In-scope**:
  - Authoring canonical skill documents (`SKILL.md`, `references/syntax-and-grid.md`, `references/auditing-and-repair.md`, `references/patterns.md`) in `python/f8/skills/f8-modeling/`.
  - Standard YAML frontmatter in `SKILL.md` with `name: f8-modeling` and descriptive intent routing metadata.
  - Runnable, copy-pasteable Python DSL snippets embedded in fenced markdown code blocks.
  - Snippet and documentation verification tests (`python/tests/test_skill_docs.py`) validating frontmatter schema, file structure, and executing all embedded DSL snippets against `f8`.
- **Out-of-scope**:
  - Implementing CLI module runner (`cli.py`, `__main__.py` - covered in `2026-09-28-python-sdk-module-runner.md`).
  - Distributing standalone `.py` or `.sh` script files inside the skill directory.
  - Encoding opinionated accounting doctrines, specific valuation models, or firm-specific line-item taxonomies (reserved for user- or team-level rules).
  - Modifying `crates/f8-cli` or altering Rust CLI commands.

### What Changes

- **New files**:
  - `python/f8/skills/f8-modeling/SKILL.md`: Concise executive summary, YAML frontmatter, mental model, quickstart, golden rules, and topic index.
  - `python/f8/skills/f8-modeling/references/syntax-and-grid.md`: 2D coordinates, vector expressions, column roles, section headers, gutters, and 0D cell indentation.
  - `python/f8/skills/f8-modeling/references/auditing-and-repair.md`: `TableAuditor`, cell inspection, unboxing, and self-healing structured exception recovery.
  - `python/f8/skills/f8-modeling/references/patterns.md`: Generic mechanical DSL wiring patterns (roll-forward corkscrews, inter-table links, subtotal/total tiers, multi-tier headers).
  - `python/tests/test_skill_docs.py`: Verification tests for YAML frontmatter and automated execution of embedded Python code snippets.
- **Modified files**: None.

### Capabilities

- **New capabilities**:
  - Progressive-disclosure skill package ready for distribution inside the `f8` Python package tree.
  - Standardized agent discovery via YAML frontmatter (`name: f8-modeling`).
  - Automated test verification of all documentation code snippets.
- **Modified capabilities**: None.

## Design

### Goals

- **Progressive disclosure**: Keep `SKILL.md` concise (< 120 lines) to prevent context bloat; detailed mechanics live in modular `references/` documents.
- **Pure static documentation**: Zero `.py` or executable scripts inside the skill tree; all guidance is pure Markdown.
- **Engine mechanics over financial doctrine**: Provide authoritative guidance on `f8` compiler invariants and DSL idioms without imposing opinionated accounting choices.
- **Executable documentation guarantee**: 100% of Python DSL snippets embedded in documentation are executed and validated against the `f8` compiler during CI.

### Invariants

1. **Wheel isolation**: The skill directory contains only `.md` files; zero `.py`, `.sh`, or binary assets.
2. **Valid YAML frontmatter**: `SKILL.md` must begin with valid YAML frontmatter containing `name: f8-modeling` and a non-empty `description`.
3. **Executable code blocks**: Every fenced Python code block intended as an example must compile cleanly without unhandled exceptions.
4. **Deterministic references**: All internal cross-references within `SKILL.md` link to relative paths under `references/`.

### Decisions

- **Single skill with progressive disclosure over multiple skills**:
  - _Rationale_: Coding assistants use semantic description matching. Splitting into multiple skills (`f8-modeling`, `f8-auditing`) causes routing confusion and incomplete context. Progressive disclosure via `references/` allows the agent to load `SKILL.md` initially and read detailed references on demand via `view_file`.
  - _Alternatives considered_: Multiple separate skills in `.agents/skills/`. Rejected to avoid router confusion and workspace directory clutter.
- **Pure Markdown files over executable `.py` scripts**:
  - _Rationale_: Scaffolding `.py` files into a user's workspace exposes them to the user's test runners (`pytest`), linters (`ruff`, `flake8`), and type checkers (`pyright`, `mypy`), which can cause unwanted CI failures. Additionally, shipping executable scripts in package assets raises supply-chain security scrutiny. Embedding fenced code snippets directly in Markdown provides complete documentation while keeping the footprint 100% inert and safe.
  - _Alternatives considered_: Shipping standalone `.py` script files in `examples/`. Rejected to preserve workspace hygiene, prevent CI breakage, and avoid security scan alarms.
- **Automated snippet execution in test suite**:
  - _Rationale_: Documentation drifts quickly when APIs evolve. Extracting and running code blocks in `python/tests/test_skill_docs.py` guarantees code examples in the skill are always functional and up to date.
  - _Alternatives considered_: Manual review only. Rejected because manual review fails to catch regressions during future SDK changes.

### Risks & Trade-offs

- **Documentation drift**: Embedded snippets may fall out of sync with future SDK API changes.
  - _Mitigation_: Automated test runner in `python/tests/test_skill_docs.py` parses and executes code snippets as part of `pytest`.
- **Token overhead of references**: Reference files could become too lengthy if verbose.
  - _Mitigation_: Keep references laser-focused on DSL mechanics, using compact tables and minimal working examples.

### Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Validate YAML frontmatter in SKILL.md**
  - **Given** `python/f8/skills/f8-modeling/SKILL.md`.
  - **When** parsed for YAML frontmatter.
  - **Then** contains `name: f8-modeling` and a non-empty `description` string suitable for agent routing.

- [ ] **Scenario 2: Validate concise length of SKILL.md**
  - **Given** `python/f8/skills/f8-modeling/SKILL.md`.
  - **When** total line count is measured.
  - **Then** file contains fewer than 120 lines.

- [ ] **Scenario 3: Validate absence of non-markdown files**
  - **Given** `python/f8/skills/f8-modeling/` directory tree.
  - **When** traversed for file extensions.
  - **Then** every file matches `*.md` with zero `.py`, `.sh`, or binary files.

- [ ] **Scenario 4: Verify syntax and grid reference documentation**
  - **Given** `python/f8/skills/f8-modeling/references/syntax-and-grid.md`.
  - **When** reviewed for coverage.
  - **Then** documents 2D coordinates, vector expressions, column roles, section headers, gutters, and 0D cell indentation with valid code examples.

- [ ] **Scenario 5: Verify auditing and repair reference documentation**
  - **Given** `python/f8/skills/f8-modeling/references/auditing-and-repair.md`.
  - **When** reviewed for coverage.
  - **Then** documents `TableAuditor`, value unboxing, Markdown tables, and structured exception handling (`CycleDetectedError`, `CompileError`, `ParseError`).

- [ ] **Scenario 6: Verify modeling patterns reference documentation**
  - **Given** `python/f8/skills/f8-modeling/references/patterns.md`.
  - **When** reviewed for coverage.
  - **Then** documents roll-forward corkscrews with `PREV`, cross-table links, multi-tier subtotals and totals, and multi-tier headers.

- [ ] **Scenario 7: Automated execution of documentation code snippets**
  - **Given** `python/tests/test_skill_docs.py`.
  - **When** executed via `uv run pytest python/tests/test_skill_docs.py`.
  - **Then** all extracted DSL snippets execute and compile successfully against `f8`.

- [ ] **Scenario 8: Linting and formatting conformance**
  - **Given** all new test and support files.
  - **When** running `uv run ruff check .`, `uv run ruff format --check .`, and `uv run pyright`.
  - **Then** all checks pass with zero errors and zero warnings.

## Tasks

### Phase 1: Author Modular Skill Package

- [ ] Create `python/f8/skills/f8-modeling/SKILL.md` documenting core mental model, quickstart, golden rules, topic index, and standard YAML frontmatter (`name: f8-modeling`).
- [ ] Create `python/f8/skills/f8-modeling/references/syntax-and-grid.md` covering 2D coordinates, vector expressions, column roles, section headers, gutters, and 0D cell indentation.
- [ ] Create `python/f8/skills/f8-modeling/references/auditing-and-repair.md` covering `TableAuditor`, inspection, unboxing, and structured exception self-healing.
- [ ] Create `python/f8/skills/f8-modeling/references/patterns.md` covering generic mechanical DSL wiring patterns (roll-forward corkscrews, inter-table links, subtotal/total tiers, multi-tier headers).

### Phase 2: Snippet Verification & Testing

- [ ] Create `python/tests/test_skill_docs.py` to:
  - Validate `SKILL.md` YAML frontmatter and line limit (< 120 lines).
  - Assert that all files in `python/f8/skills/f8-modeling/` end in `.md`.
  - Parse and execute all embedded Python DSL blocks to guarantee compiling examples.
- [ ] Verify test suite via `uv run pytest python/tests/test_skill_docs.py`.
- [ ] Run full Python verification suite (`uv run ruff check .`, `uv run ruff format --check .`, `uv run pyright`).
