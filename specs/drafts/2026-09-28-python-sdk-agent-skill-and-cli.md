---
title: Python SDK Agent Skill and Module Runner
date: 2026-09-28
status: draft
target-living-spec: specs/living-specs/interfaces/python-sdk.md
---

## Proposal

### What

Package a modular, progressive-disclosure autonomous agent skill inside the Python SDK (`python/f8/skills/f8-modeling/`) using pure Markdown (`.md`) files, and provide a lightweight, zero-dependency Python module runner (`python/f8/__main__.py` and `python/f8/cli.py`) invoked via `python -m f8` or `uv run -m f8` with two primary commands:

1. `python -m f8 guide [topic]`: Streams the financial modeling guide and syntax reference directly to stdout. Supports optional topic routing (`syntax`, `audit`, `patterns`, `all`) and listing flags (`--list`, `--all`).
2. `python -m f8 agent init`: Non-destructively scaffolds the full `.agents/skills/f8-modeling/` directory tree (including `references/`) into the user's workspace.

The Python SDK treats Python code (`f8.Model`, `Sheet`, `Table`) as the primary authoring source of truth. No console script is registered in `pyproject.toml`, preserving zero binary namespace collisions with the native Rust compiler CLI binary (`crates/f8-cli`). The runner never modifies or overwrites existing user configuration files (`AGENTS.md`, `.cursorrules`).

### Why

- **Agent context and progressive disclosure**: Autonomous coding assistants (Antigravity, Claude Code, Cursor, Windsurf) need authoritative guidance on declarative 2D grid modeling, vector formulas, cell-level indentation, auditing, and self-healing on structured exceptions. Packaging this guidance as a modular skill with `references/` enables progressive disclosure: agents load a concise `SKILL.md` initially and fetch detailed syntax or repair patterns only when needed, minimizing token bloat.
- **Python as authoring source of truth**: Unlike headless CI pipelines that consume on-disk JSON specifications via `crates/f8-cli`, Python developers and agents author models natively in Python scripts (`model.py`), verify them in-memory via `model.compile()`, test them via `pytest`, and inspect them via `TableAuditor.to_markdown()`. The Python SDK does not require an on-disk JSON compiler CLI.
- **Zero binary collision with Rust CLI**: Registering a global `f8` console script in `pyproject.toml` would shadow the native Rust `crates/f8-cli` binary in virtual environments while supporting none of the compiler commands (`check`, `compile`, `init`, `inspect`). Using standard Python module execution (`python -m f8` / `uv run -m f8`) ensures clean separation of concerns without namespace clashes.
- **Pure Markdown safety and isolation**: Packaging purely static `.md` documentation eliminates security red flags (no executable script payloads), prevents user CI and test runner interference (`pytest`, `ruff`, or `pyright` traversing external `.py` files in `.agents/`), and prevents bytecode (`.pyc`) cache pollution.
- **Zero post-install file pollution**: Package managers (`pip`, `uv`) unpack static wheels into `site-packages` and forbid arbitrary disk writes. Workspace files must be explicitly initialized.
- **Workspace safety**: Modifying or appending to existing `AGENTS.md` files risks merge conflicts, corrupted settings, or duplicate instructions. Scaffolding a dedicated, self-contained skill directory (`.agents/skills/f8-modeling/`) isolates `f8` knowledge cleanly.
- **Targeted CLI inspection**: Providing topic-based subcommands for `python -m f8 guide` (`syntax`, `audit`, `patterns`) prevents dumping a massive wall of text into terminal outputs or agent command buffers.
- **API purity**: Keeping prompt/instruction retrieval out of the root `f8` Python namespace preserves clean separation of concerns, keeping the modeling SDK lean and focused.

### Scope

- **In-scope**:
  - Authoring the canonical modeling skill directory at `python/f8/skills/f8-modeling/` containing pure Markdown files (`SKILL.md` with standard YAML frontmatter and `references/*.md`).
  - Embedding comprehensive, runnable Python DSL code snippets directly in fenced markdown blocks within `SKILL.md` and `references/patterns.md`.
  - Implementing `python/f8/cli.py` using Python's standard library `argparse` and `importlib.resources`.
  - Supporting topic routing in `python -m f8 guide [topic]` (`syntax`, `audit`, `patterns`, `all`, `--list`, `--all`).
  - Implementing recursive directory copy in `python -m f8 agent init` with `--dest` and `--force` options.
  - Implementing `python/f8/__main__.py` to support `python -m f8 <command>` and `uv run -m f8 <command>`.
  - Unit tests covering `python -m f8 guide` (all topics and flags) and `python -m f8 agent init` (recursive scaffolding, collision error, and `--force` overwrite).
- **Out-of-scope**:
  - Registering a global console script `f8` in `pyproject.toml` (reserved for `crates/f8-cli`).
  - Re-implementing JSON compiler CLI commands (`check`, `compile`, `init`, `inspect`) in Python (on-disk compiler workflows belong to `crates/f8-cli`).
  - Distributing standalone `.py` or `.sh` script files inside the skill directory.
  - Encoding opinionated accounting doctrines, specific valuation models, or firm-specific line-item taxonomies (these belong in user- or team-level skills/rules).
  - Modifying `crates/f8-cli` or altering Rust CLI commands.
  - Adding third-party CLI dependencies (`click`, `typer`).
  - Editing, appending to, or creating user `AGENTS.md` or `.cursorrules` files.
  - Exposing prompt/instruction retrieval helper functions on the root `f8` module.

### What Changes

- **New files**:
  - `python/f8/skills/f8-modeling/SKILL.md`: Concise executive summary, YAML frontmatter (`name: f8-modeling`), mental model, quickstart, golden rules, and topic index.
  - `python/f8/skills/f8-modeling/references/syntax-and-grid.md`: 2D coordinates, vector expressions, column roles, section headers, gutters, and 0D cell indentation.
  - `python/f8/skills/f8-modeling/references/auditing-and-repair.md`: `TableAuditor`, cell inspection, unboxing, and self-healing structured exception recovery.
  - `python/f8/skills/f8-modeling/references/patterns.md`: Generic mechanical DSL wiring patterns (roll-forward corkscrews, inter-table links, subtotal/total tiers, multi-tier headers).
  - `python/f8/cli.py`: Minimal CLI entrypoint implementing `guide` and `agent init`.
  - `python/f8/__main__.py`: Module entrypoint for `python -m f8` and `uv run -m f8`.
  - `python/tests/test_cli.py`: Verification test suite for module runner commands.
- **Modified files**: None. `pyproject.toml` remains unchanged, avoiding global binary name collisions.

### Capabilities

- **New capabilities**:
  - `uv run -m f8 guide` (or `python -m f8 guide`): Streams `SKILL.md` (overview and topic index) to stdout with zero disk writes.
  - `uv run -m f8 guide <topic>`: Streams specific topic guide (`syntax`, `audit`, `patterns`, `all`) to stdout.
  - `uv run -m f8 guide --list`: Lists available topics with summaries.
  - `uv run -m f8 agent init` (or `python -m f8 agent init`): Recursively scaffolds `.agents/skills/f8-modeling/` into the active workspace, failing fast if destination files exist unless `--force` is provided.
- **Modified capabilities**: None. Existing Python DSL, PyO3 bindings, and Rust compiler remain unchanged.

## Design

### Goals

- **Zero dependencies**: Rely exclusively on Python standard library modules (`argparse`, `importlib.resources`, `pathlib`, `shutil`, `sys`).
- **Progressive disclosure**: Keep `SKILL.md` concise (< 120 lines) with valid YAML frontmatter to avoid context bloat; deeper instructions live in `references/`.
- **Pure static documentation**: Avoid shipping executable files in the skill directory; keep all references and code snippets in Markdown.
- **Engine mechanics over financial doctrine**: Provide authoritative guidance on `f8` compiler invariants and DSL idioms without imposing opinionated accounting choices.
- **Zero binary collision**: Run exclusively via Python module execution (`python -m f8` / `uv run -m f8`), leaving the `f8` executable name for `crates/f8-cli`.
- **Single source of truth**: `python/f8/skills/f8-modeling/` serves as the authoritative source of truth for both the packaged `.whl` distribution and the module stdout runner.
- **Idempotent and safe**: Never overwrite existing workspace files without explicit user consent (`--force`).
- **Standard Python exit codes**: Follow standard Python conventions (`0` for success, `1` for operational / runtime errors, `2` for CLI syntax / argument errors).

### Invariants

1. **Wheel isolation**: The Python wheel distribution contains only `f8` Python code, package documentation data, and `f8._core` binary extension.
2. **Workspace non-interference**: No files outside `.agents/skills/f8-modeling/` are created or modified during `agent init`.
3. **Fail-safe initialization**: `python -m f8 agent init` raises an error and aborts with exit code `1` if target files or directories exist and `--force` is omitted.
4. **Safe resource traversal**: Package assets are resolved via `importlib.resources.files("f8").joinpath("skills", "f8-modeling")` rather than passing hyphenated package module strings.
5. **Strict typing**: `python/f8/cli.py` enforces 100% type annotations with zero `pyright` strict-mode errors.
6. **Standard Python exit codes**: Command-line argument syntax and invalid choice errors return exit code `2` (handled automatically by standard `argparse`), operational errors (e.g. target collisions without `--force`) return exit code `1`, and successful executions return exit code `0`.

### Decisions

- **Python module runner (`python -m f8`) over global console script**:
  - _Rationale_: Module execution (`python -m f8` / `uv run -m f8`) provides 100% deterministic interpreter binding directly to whichever virtual environment executes the command, without relying on shell `$PATH` configuration or risking virtualenv shebang desync. It avoids generating platform-specific console script wrappers (`f8.exe` on Windows or `.venv/bin/f8` on Unix) and leaves `pyproject.toml` completely untouched. Autonomous agent harnesses execute via `uv run -m f8` deterministically.
  - _Alternatives considered_: Registering `[project.scripts] f8 = "f8.cli:main"`. Rejected because console script shims can desync from relocated virtual environments, fail when virtualenvs are not on `$PATH`, and introduce global executable namespace pollution for a lightweight onboarding helper.
- **Standard Python `argparse` exit codes (`0`, `1`, `2`)**:
  - _Rationale_: Autonomous agents parse structured `stderr` output (usage information and allowed option lists) to self-correct rather than branching on specific non-zero error codes. Adopting standard Python `argparse` defaults (`0` success, `1` operational/runtime errors, `2` CLI argument syntax errors) eliminates custom error-handling overrides and maintains clean Python idioms.
  - _Alternatives considered_: Forcing exit code `1` on CLI syntax errors via custom `ArgumentParser.error()` monkey-patching. Rejected because standard exit code `2` with built-in `argparse` formatting is idiomatic, zero-boilerplate, and universally supported by agent execution harnesses.
- **Pure Markdown files over executable `.py` scripts**:
  - _Rationale_: Scaffolding `.py` files into a user's workspace exposes them to the user's test runners (`pytest`), linters (`ruff`, `flake8`), and type checkers (`pyright`, `mypy`), which can cause unwanted CI failures. Additionally, shipping executable scripts in package assets raises supply-chain security scrutiny. Embedding fenced code snippets directly in Markdown provides complete documentation while keeping the footprint 100% inert and safe.
  - _Alternatives considered_: Shipping standalone `.py` script files in `examples/`. Rejected to preserve workspace hygiene, prevent CI breakage, and avoid security scan alarms.
- **DSL engine mechanics over prescriptive financial methodology**:
  - _Rationale_: `f8` is a compiler and modeling engine (comparable to SQL or pandas). Different financial disciplines (M&A investment banking, private equity LBO, real estate, FP&A) follow divergent accounting and valuation doctrines. The packaged skill focuses strictly on `f8` DSL idioms, 2D coordinates, vector formulas, layout, auditing, and self-healing. Opinionated valuation or accounting conventions belong in user- or team-level custom skills and rules.
  - _Alternatives considered_: Including detailed financial valuation templates (e.g. specific 3-statement LBO frameworks). Rejected to avoid prescribing domain opinions into a general-purpose compiler skill.
- **Single skill with progressive disclosure over multiple skills**:
  - _Rationale_: Coding assistants use semantic description matching. Splitting into multiple skills (`f8-modeling`, `f8-auditing`) causes routing confusion and incomplete context. Progressive disclosure via `references/` allows the agent to load `SKILL.md` initially and read detailed references on demand via `view_file`.
  - _Alternatives considered_: Multiple separate skills in `.agents/skills/`. Rejected to avoid router confusion and workspace directory clutter.
- **Standard YAML frontmatter for `SKILL.md`**:
  - _Rationale_: Agent systems (Antigravity, Claude Code, Cursor) rely on top-level YAML frontmatter containing `name` and `description` to discover, index, and activate skills based on user intent.
  - _Alternatives considered_: Plain markdown heading. Rejected because agent discovery engines require structured frontmatter.
- **Standard library `argparse` over `click`/`typer`**:
  - _Rationale_: Eliminates external dependencies for the Python package, keeping installation lightweight and instant.
  - _Alternatives considered_: `click` or `typer`. Rejected to preserve zero-dependency runtime for the pure Python layer.
- **Decoupled from `crates/f8-cli`**:
  - _Rationale_: `f8-cli` is a separate native binary distribution for non-Python environments. Coupling Python wheels to the Rust CLI binary would complicate cross-compilation and packaging.
  - _Alternatives considered_: Invoking `f8-cli` via subprocess. Rejected because `f8-cli` is not bundled into wheels.
- **Omitting `f8.get_instructions()` in library API**:
  - _Rationale_: Keeps `f8` focused strictly on financial modeling constructs (`Model`, `Sheet`, `Table`, `compile`, `TableAuditor`).
  - _Alternatives considered_: Exporting `get_instructions()` on `f8`. Rejected to maintain separation of concerns.

### Command Specifications

#### Empty Invocation

- **Invocation**: `python -m f8` or `uv run -m f8`
- **Behavior**: Prints top-level usage/help summary to `sys.stdout` and exits with code `0`.

#### `guide`

- **Invocation**: `python -m f8 guide [topic]` or `uv run -m f8 guide [topic]`
- **Arguments**:
  - `topic` (positional, optional): Specific guide topic to display. Allowed values: `syntax`, `audit`, `patterns`, `all`. Defaults to `None` (displays `SKILL.md` overview).
  - `--all`: Alias to output complete concatenated documentation.
  - `--list`: Lists available topics with summaries and exits.
- **Behavior**:
  - `guide` (no topic): Reads `SKILL.md` via `importlib.resources` and writes UTF-8 text to `sys.stdout`.
  - `guide syntax`: Reads `references/syntax-and-grid.md` and writes to `sys.stdout`.
  - `guide audit`: Reads `references/auditing-and-repair.md` and writes to `sys.stdout`.
  - `guide patterns`: Reads `references/patterns.md` and writes to `sys.stdout`.
  - `guide all` or `guide --all`: Concatenates `SKILL.md` and reference documents in deterministic sequence:
    1. `SKILL.md`
    2. `references/syntax-and-grid.md`
    3. `references/auditing-and-repair.md`
    4. `references/patterns.md`
    Each section is joined with `\n\n---\n\n` and written to `sys.stdout`.
  - `guide --list`: Formats available topics into a clean, aligned list on `sys.stdout`:
    ```text
    Available topics:
      syntax    2D coordinates, vector expressions, column roles, gutters, indentation
      audit     TableAuditor, inspection, unboxing, structured exception recovery
      patterns  Corkscrews, inter-table links, subtotal/total tiers, multi-tier headers
      all       Complete concatenated guide
    ```
  - Invalid topic: Standard `argparse` validation writes error and available choices to `stderr` and exits with code `2`.
- **Exit code**: `0` on success, `2` on invalid topic argument.

#### `agent init`

- **Invocation**: `python -m f8 agent init [--dest <PATH>] [--force]` or `uv run -m f8 agent init`
- **Arguments**:
  - `--dest <PATH>`: Target root directory (defaults to current working directory `.`).
  - `-f`, `--force`: Overwrite existing skill directory and files if present.
- **Behavior**:
  - Resolves target path `<dest>/.agents/skills/f8-modeling`.
  - Self-copy guard: If destination path resolves to the packaged source asset directory (`dest.resolve() == src.resolve()`), prints error to `stderr` and exits with code `1` to prevent copying into source assets.
  - Checks if target directory exists and contains files:
    - If target directory contains files and `--force` is false: prints error to `stderr` and exits with code `1`.
    - If target directory does not exist or `--force` is true: creates parent directories recursively, recursively reads files from the packaged `Traversable` asset hierarchy and writes bytes to `<dest>/.agents/skills/f8-modeling/`, and prints confirmation to `stdout`.
- **Exit code**: `0` on success, `1` on collision without `--force` or self-copy attempt.

### Risks & Trade-offs

- **Workspace skill divergence**: Once an agent skill is scaffolded into a user's workspace, local edits may diverge from updated library releases.
  - _Mitigation_: `python -m f8 agent init --force` provides an explicit, non-destructive upgrade path to overwrite and refresh the skill with the latest package version on demand.
- **Resource traversal in packaged wheels**: Hyphenated directory names (`f8-modeling`) cannot be imported as module strings via `importlib.resources.files("f8.skills.f8-modeling")`. Additionally, non-extracted wheels provide `Traversable` objects rather than concrete filesystem paths.
  - _Mitigation_: Access resources via `importlib.resources.files("f8").joinpath("skills", "f8-modeling")` and traverse the `Traversable` hierarchy directly, reading and writing file bytes/text without assuming a flat unpacked filesystem.
- **Absence of global compiler CLI in Python wheel**: Users installing `f8` via `pip` or `uv` cannot run `f8 compile` or `f8 check` on disk JSON files without the native `crates/f8-cli` binary.
  - _Mitigation_: By design, Python SDK users author and compile models in Python code. Clear documentation in the README directs users to either run Python scripts (`python model.py`) or install `crates/f8-cli` if on-disk JSON pipelines are required.

### Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Output modeling guide overview to stdout**
  - **Given** the `f8` package is installed.
  - **When** running `python -m f8 guide` or `uv run -m f8 guide`.
  - **Then** prints `SKILL.md` to `stdout` with exit code `0` and writes no files to disk.

- [ ] **Scenario 2: Output specific topic guide to stdout**
  - **Given** the `f8` package is installed.
  - **When** running `python -m f8 guide syntax` (or `audit`, `patterns`).
  - **Then** prints the corresponding reference markdown file to `stdout` with exit code `0`.

- [ ] **Scenario 3: List available guide topics**
  - **Given** the `f8` package is installed.
  - **When** running `python -m f8 guide --list`.
  - **Then** prints available topics (`syntax`, `audit`, `patterns`, `all`) with summaries to `stdout` and exits with code `0`.

- [ ] **Scenario 4: Output complete concatenated guide**
  - **Given** the `f8` package is installed.
  - **When** running `python -m f8 guide --all` or `python -m f8 guide all`.
  - **Then** prints concatenated contents of `SKILL.md` and all reference files to `stdout` with exit code `0`.

- [ ] **Scenario 5: Reject invalid guide topic**
  - **Given** the `f8` package is installed.
  - **When** running `python -m f8 guide unknown_topic`.
  - **Then** prints standard usage and allowed choices to `stderr` and exits with code `2`.

- [ ] **Scenario 6: Initialize agent skill directory tree in empty workspace**
  - **Given** an empty workspace directory with no existing `.agents/` folder.
  - **When** running `python -m f8 agent init` or `uv run -m f8 agent init`.
  - **Then** recursively creates `.agents/skills/f8-modeling/` with `SKILL.md` and `references/*.md` matching packaged contents, containing zero `.py` files, and exits with code `0`.

- [ ] **Scenario 7: Reject overwrite on collision without force flag**
  - **Given** a workspace where `.agents/skills/f8-modeling/SKILL.md` already exists.
  - **When** running `python -m f8 agent init` without `--force`.
  - **Then** prints an error message to `stderr`, does not overwrite any files, and exits with code `1`.

- [ ] **Scenario 8: Overwrite existing skill directory with force flag**
  - **Given** a workspace where `.agents/skills/f8-modeling/` already exists with existing or modified files.
  - **When** running `python -m f8 agent init --force`.
  - **Then** overwrites files with latest packaged contents, prints confirmation, and exits with code `0`.

- [ ] **Scenario 9: Validate YAML frontmatter in SKILL.md**
  - **Given** the generated or packaged `SKILL.md`.
  - **When** parsed for YAML frontmatter.
  - **Then** contains non-empty `name: f8-modeling` and a descriptive `description` string suitable for agent routing.

- [ ] **Scenario 10: Type safety and lint verification**
  - **Given** all new Python CLI modules.
  - **When** running `uv run ruff check .`, `uv run ruff format --check .`, and `uv run pyright`.
  - **Then** all checks pass with zero errors and zero warnings.

## Tasks

### Phase 1: Author Modular Skill Package

- [ ] Create `python/f8/skills/f8-modeling/SKILL.md` documenting core mental model, quickstart, golden rules, topic index, and standard YAML frontmatter (`name: f8-modeling`).
- [ ] Create `python/f8/skills/f8-modeling/references/syntax-and-grid.md` covering 2D coordinates, vector expressions, column roles, section headers, gutters, and 0D cell indentation.
- [ ] Create `python/f8/skills/f8-modeling/references/auditing-and-repair.md` covering `TableAuditor`, inspection, unboxing, and structured exception self-healing.
- [ ] Create `python/f8/skills/f8-modeling/references/patterns.md` covering generic mechanical DSL wiring patterns (roll-forward corkscrews, inter-table links, subtotal/total tiers, multi-tier headers).

### Phase 2: Implement Module Runner Entrypoint

- [ ] Implement `python/f8/cli.py` with:
  - Top-level help summary when invoked without arguments (exit code `0`).
  - `guide` subcommand supporting topic routing (`syntax`, `audit`, `patterns`), deterministic `--all` concatenation, `--list` formatting, and standard exit code `2` on invalid topic argument.
  - `agent init` subcommand supporting recursive `Traversable` file extraction (reading and writing bytes without relying on `shutil.copytree`), self-copy guard, parent directory creation, collision error (exit code `1`), and `--force` overwrite.
- [ ] Implement `python/f8/__main__.py` to dispatch to `f8.cli.main()`.

### Phase 3: Unit Testing & Verification

- [ ] Create `python/tests/test_cli.py` testing:
  - Empty invocation prints usage and exits with code `0`.
  - `guide` default stdout, topic streams (`syntax`, `audit`, `patterns`), `--list`, and deterministic `--all`.
  - `guide` invalid topic exits with code `2`.
  - `agent init` recursive scaffolding, collision failure with exit code `1`, and `--force` overwrite.
  - `agent init` self-copy detection prevents copying into source asset directory.
  - `SKILL.md` frontmatter validation.
- [ ] Run full verification suite (`uv run ruff check .`, `uv run ruff format --check .`, `uv run pyright`, `uv run pytest`).
- [ ] Update living spec `specs/living-specs/interfaces/python-sdk.md` with CLI runner and agent skill documentation upon reconciliation.
