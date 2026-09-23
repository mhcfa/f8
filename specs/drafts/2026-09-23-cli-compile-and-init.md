---
title: CLI OpenXML Compilation and Project Scaffolding (f8 compile & f8 init)
date: 2026-09-23
status: draft
target-living-spec: specs/living-specs/interfaces/cli.md
---

## Proposal

### What

Implement the `f8 compile` and `f8 init` subcommands in `crates/f8-cli`. This milestone equips the CLI with end-to-end authoring and compilation capabilities:
1. `f8 compile`: Compiles project specifications into OpenXML `.xlsx` workbooks on disk or streams them to stdout, with support for theme overrides, static values-only freezing, cache omission, and operating system spreadsheet opening.
2. `f8 init`: Scaffolds new financial models from scratch or unpacks canonical, pre-wired institutional template archetypes (`three-statement`, `lbo`, `trading-comps`).

### Why

Once model verification (`f8 check`) is in place, users and AI agents need to produce production `.xlsx` spreadsheets and initialize new models from institutional best practices without manually writing JSON structures from scratch.

### Scope

- **In-scope**:
  - `f8 compile` subcommand:
    - Default output path resolution (`./<model_name>.xlsx`).
    - Explicit destination via `-o, --out <path>`.
    - Stdout streaming via `-o -` with interactive TTY protection (prevents garbling terminal displays unless redirected/piped).
    - `--theme <name>` override overriding the theme configured in `f8.json`.
    - `--values-only` compiling live formulas into frozen calculation values for client distribution.
    - `--omit-cache` disabling precalculated `<v>` calculation tags in OpenXML cells.
    - `--open` launching the generated `.xlsx` file using the OS default application (`xdg-open` on Linux, `open` on macOS, `start` on Windows) with graceful non-fatal warning on headless failure.
  - `f8 init` subcommand:
    - Initializing an empty project structure (`f8.json` + `tables/` directory) with a blank table.
    - Unpacking institutional template archetypes via `-t, --template <name>`:
      - `three-statement`: Integrated Income Statement, Balance Sheet, and Cash Flow.
      - `lbo`: Transaction assumptions, sources & uses, debt amortization schedules.
      - `trading-comps`: Peer valuation matrix with multiples and benchmark aggregations.
    - Specifying `--theme <name>` during initialization (default: `corporate_navy`).
  - Bundling canonical JSON projects in `crates/f8-cli/templates/<archetype>/` using `include_str!`.
  - Integration tests verifying all bundled templates compile cleanly and produce valid `.xlsx` archives.

- **Out-of-scope**:
  - Auditing and terminal inspection subcommands (`f8 inspect` - Milestone 3).
  - Imperative cell mutation commands (declarative JSON editing remains strictly preserved).
  - Cap-table template archetype authoring (deferred to a subsequent milestone).

### What Changes

- `crates/f8-cli/templates/`:
  - `three-statement/`: `f8.json` and `tables/*.json`.
  - `lbo/`: `f8.json` and `tables/*.json`.
  - `trading-comps/`: `f8.json` and `tables/*.json`.
- `crates/f8-cli/src/`:
  - `archetypes/mod.rs`: Embedded template catalog and extractor.
  - `project/writer.rs`: Filesystem directory scaffolding and JSON file writer.
  - `commands/compile.rs`: Full compiler pass execution, OpenXML buffer emission, and disk/stdout writing.
  - `commands/init.rs`: Template unpacking and blank project initialization.
  - `cli.rs`: Add `CompileArgs` and `InitArgs` to Clap command tree.
- `crates/f8-cli/tests/`:
  - `compile_test.rs`: Test `.xlsx` file generation, options, and stdout streaming.
  - `init_test.rs`: Test template scaffolding and verify all bundled archetypes pass `Compiler::compile`.

### Capabilities

- **New capabilities**:
  - Single-command `.xlsx` compilation via `f8 compile`.
  - Stdout streaming (`f8 compile -o - | ...`) for pipeline composability.
  - Operating system auto-open (`f8 compile --open`).
  - Instant project scaffolding (`f8 init my_model --template three-statement`).
  - Guaranteed compile-time validity for all bundled institutional templates.

- **Modified capabilities**: None.

---

## Design

### Goals

1. **Zero-redundancy emission**: Leverage `CompiledModel::emit_workbook` to emit OpenXML bytes directly from compilation artifacts.
2. **Terminal safety**: Prevent streaming binary zip bytes to interactive terminals unless redirected.
3. **Institutional templates**: Provide high-quality financial modeling archetypes out-of-the-box.
4. **Compile-time validation**: Ensure every bundled template in `templates/` is validated by unit tests in CI.

### Invariants

1. **Bit-for-bit reproducibility**: Given identical input files and options, `f8 compile` produces identical `.xlsx` archives.
2. **Declarative primacy**: `f8 init` unpacks clean, standard JSON files that users and agents edit directly.
3. **Graceful launcher fallback**: `--open` failure in headless or CI environments emits a non-fatal warning without failing the compilation (exit code remains `0`).

### Decisions

- **Decision 1: Embed templates via `include_str!`**
  - *Rationale*: Storing templates as separate JSON files in `templates/` allows them to be edited, formatted, and tested with standard tools, while `include_str!` bakes them into the CLI binary with zero runtime asset dependencies.
  - *Alternatives considered*: Runtime network downloads or procedural Rust generation (rejected: requires network or obscures template JSON readability).

- **Decision 2: Stdout TTY detection**
  - *Rationale*: Writing binary ZIP data directly to an interactive terminal scrambles terminal state. Checking `std::io::stdout().is_terminal()` when `-o -` is requested prevents accidental binary dumps unless redirected.
  - *Alternatives considered*: Blindly writing to stdout (rejected: poor user experience when accidentally running `f8 compile -o -` without a pipe).

### Data Contracts

#### CLI Arguments

```text
f8 compile [-p <path>] [-o <path>] [--theme <name>] [--values-only] [--omit-cache] [--open]
f8 init [path] [-t, --template <name>] [--theme <name>]
```

| Subcommand | Flag | Description | Default |
| :--- | :--- | :--- | :--- |
| `compile` | `-o, --out <path>` | Output `.xlsx` file destination (`-` for stdout) | `./<model_name>.xlsx` |
| `compile` | `--theme <name>` | Override theme name | From `f8.json` |
| `compile` | `--values-only` | Freeze calculated formulas into static values | `false` |
| `compile` | `--omit-cache` | Omit precalculated `<v>` tags | `false` |
| `compile` | `--open` | Open in default spreadsheet application | `false` |
| `init` | `[path]` | Destination directory | Current directory (`.`) |
| `init` | `-t, --template <name>`| Template archetype (`three-statement`, `lbo`, `trading-comps`) | Blank |
| `init` | `--theme <name>` | Theme preset for new project | `corporate_navy` |

### Risks & Trade-offs

- **Headless OS launcher failure**: In CI environments, containers, or headless servers, running `f8 compile --open` will fail to spawn a GUI spreadsheet application.
  - *Mitigation*: Treat launcher spawn failure as a non-fatal warning printed to `stderr` and preserve exit code `0` since the `.xlsx` file was successfully written.
- **Accidental binary dump to interactive terminal**: Executing `f8 compile -o -` directly in an interactive shell could write raw ZIP bytes to stdout.
  - *Mitigation*: Reject `-o -` if `std::io::stdout().is_terminal()` returns true, exiting with code `1` and a guidance error.
- **Embedded template schema drift**: As compiler features evolve, embedded archetype JSON files could become outdated.
  - *Mitigation*: Unit tests in `init_test.rs` compile all embedded templates through `Compiler::compile` on every `cargo test` run.

---

## Acceptance Criteria (Given / When / Then)

- [ ] **Scenario 1: Default compilation to disk**
  - **Given** a valid f8 project
  - **When** running `f8 compile`
  - **Then** `./<model_name>.xlsx` is created on disk, contains valid OpenXML archive bytes, and exits with code `0`.

- [ ] **Scenario 2: Explicit output path and theme override**
  - **Given** a valid f8 project
  - **When** running `f8 compile -o dist/output.xlsx --theme corporate_navy`
  - **Then** `dist/output.xlsx` is written with corporate navy theme styling and exits with code `0`.

- [ ] **Scenario 3: Stdout streaming when piped**
  - **Given** a valid f8 project
  - **When** running `f8 compile -o -` redirected to a file or piped
  - **Then** binary `.xlsx` bytes are written to stdout with exit code `0`.

- [ ] **Scenario 4: Stdout streaming blocked on interactive TTY**
  - **Given** a terminal TTY
  - **When** running `f8 compile -o -` without redirection
  - **Then** command aborts with exit code `1` and emits a warning on `stderr` explaining stdout cannot be an interactive terminal.

- [ ] **Scenario 5: Blank project initialization**
  - **Given** a new directory path `new_model`
  - **When** running `f8 init new_model`
  - **Then** `new_model/f8.json` and a starter `new_model/tables/table.json` are created, and `f8 check -p new_model` exits with `0`.

- [ ] **Scenario 6: Template project initialization**
  - **Given** a new directory path `corp_lbo`
  - **When** running `f8 init corp_lbo --template lbo`
  - **Then** full LBO multi-table project files are written, and `f8 check -p corp_lbo` exits with `0`.

- [ ] **Scenario 7: Embedded template compilation test**
  - **Given** all embedded archetypes in `crates/f8-cli/templates/`
  - **When** running unit tests
  - **Then** every archetype is loaded and successfully compiled via `Compiler::compile`.

---

## Tasks

### Phase 1: Compilation Engine (`f8 compile`)
- [ ] Use standard library `std::io::IsTerminal` for stdout streaming checks.
- [ ] Implement `commands/compile.rs` calling `Compiler::compile` and `CompiledModel::emit_workbook`.
- [ ] Wire `--values-only`, `--omit-cache`, and `--theme` flags to `EmissionOptions`.
- [ ] Implement `--open` using OS launcher abstraction (`xdg-open` / `open` / `start`).
- [ ] Add `CompileArgs` to Clap CLI.

### Phase 2: Template Archetypes & Project Writer
- [ ] Construct canonical JSON model files for `three-statement`, `lbo`, and `trading-comps` in `crates/f8-cli/templates/`.
- [ ] Implement `archetypes/mod.rs` embedding templates with `include_str!`.
- [ ] Implement `project/writer.rs` writing files to target directories.
- [ ] Implement `commands/init.rs` handling blank and template project scaffolding.
- [ ] Add `InitArgs` to Clap CLI.

### Phase 3: Verification & Integration Tests
- [ ] Add unit test compiling every embedded template during `cargo test`.
- [ ] Add integration tests in `crates/f8-cli/tests/compile_test.rs` and `init_test.rs`.
- [ ] Run full workspace verification (`cargo check`, `cargo test`, `cargo clippy`, `cargo fmt`).
