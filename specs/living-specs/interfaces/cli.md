---
title: Command-Line Interface (f8 CLI)
last_updated: 2026-09-23
domain: interfaces
status: active
---

## Purpose

The `f8` command-line executable (`crates/f8-cli`) provides a lightweight, headless verification tool, compiler runner, project scaffolding utility, and terminal auditing interface for financial analysts and autonomous AI coding agents. It serves as an outer adapter consuming `f8-core`, enforcing zero production panics, clean stream discipline (`stdout` vs `stderr`), and deterministic 3-state exit codes.

## Architecture & Boundaries

- **Modules & components**:
  - `f8_cli::cli`: Clap command hierarchy and argument parsing with global `-p, --project` and `--json` flags.
  - `f8_cli::project::loader`: Manifest discovery via upward directory traversal, multi-file project assembly (`SheetSpec.tables`), and theme resolution.
  - `f8_cli::project::writer`: Filesystem directory scaffolding and JSON file writer for project initialization.
  - `f8_cli::archetypes`: Embedded template catalog bundling canonical JSON models (`three-statement`, `lbo`, `trading-comps`, `blank`) via `include_str!`.
  - `f8_cli::commands::check`: Headless model verification executing Passes 1–6a via `f8_core::Compiler::compile`.
  - `f8_cli::commands::compile`: End-to-end OpenXML binary workbook compilation via `CompiledModel::emit_workbook`.
  - `f8_cli::commands::init`: Project initialization from blank starter or embedded archetype templates.
  - `f8_cli::commands::inspect`: Interactive auditing dispatcher for model summaries, tables, cells, and DAG trees.
  - `f8_cli::render`: Pure formatters for overview cards, accounting tables, markdown grids, cell cards, and DAG trees.
  - `f8_cli::error`: Structured `CliError` classifying I/O/syntax failures vs compiler diagnostics and resolving exit codes.
- **Data flow & interfaces**:
  - Input: Discovers `f8.json` on disk by walking parent directories up to filesystem root or via explicit `-p, --project <path>`.
  - Loading: Reads manifest, resolves relative table file paths, deserializes `TableSpec`s, and loads built-in or custom JSON themes.
  - Delegation: Passes in-memory models directly to `Compiler::compile` and `emit_workbook`. CLI contains zero domain layout, graph, or formula evaluation logic.
  - Output: Success summaries and valid JSON to `stdout`; error messages, compiler diagnostics, and error JSON to `stderr`.

## Requirements & Invariants

### 1. Deterministic 3-State Exit Codes

The CLI strictly classifies execution outcomes into three stable exit codes:

| Exit Code | Classification | Cause |
| :--- | :--- | :--- |
| `0` | Success | Operation completed cleanly (checked, compiled, or initialized). |
| `1` | Operational / CLI / I/O Error | CLI syntax error, missing `f8.json`, unreadable file, JSON deserialization failure, interactive TTY binary protection. |
| `2` | Compiler Diagnostic | Passes 1–6a compiler failure (schema validation, cycle, unassigned driver, layout error, lowering failure). |

#### Scenarios

- **Verification success**:
  - **When** running `f8 check` on a valid project
  - **Then** prints human or JSON summary to `stdout` and exits with code `0`.
- **Operational error**:
  - **When** running `f8 check` in a directory without `f8.json` or with corrupt JSON
  - **Then** prints error message to `stderr` and exits with code `1`.
- **Compiler diagnostic**:
  - **When** running `f8 check` or `f8 compile` on a project containing a circular reference or layout overflow
  - **Then** outputs compiler diagnostic to `stderr` and exits with code `2`.

### 2. Upward Directory Traversal & Global Ergonomics

The CLI automatically traverses parent directories starting from the current working directory until it locates `f8.json`.

#### Scenarios

- **Upward discovery**:
  - **Given** an `f8.json` in a project root and a nested working directory
  - **When** executing `f8 check` or `f8 compile` without `-p`
  - **Then** discovers root `f8.json`, loads relative tables, and executes.
- **Global `-p, --project` flag**:
  - **When** specifying `-p <path>` before or after subcommands (`f8 -p ./models compile` or `f8 compile -p ./models`)
  - **Then** targets the explicit directory or manifest file directly.

### 3. Stream Discipline & Structured `--json` Output

Outputs are strictly segregated: operational output and summaries go to `stdout`; diagnostic errors go to `stderr`.

#### Scenarios

- **Human verification output**:
  - **When** running `f8 check`
  - **Then** prints step checkmarks, sheet/table/cell counts, and status to `stdout`.
- **Machine-readable JSON output**:
  - **When** running `f8 check --json` or `f8 compile --json`
  - **Then** prints formatted JSON object with `"status": "ok"` and summary metrics to `stdout` on success, or `"status": "error"` and `"exit_code": <code>` to `stderr` on failure.

### 4. OpenXML Workbook Compilation (`f8 compile`)

Compiles project specifications into native `.xlsx` workbooks on disk or streams them to stdout.

#### Scenarios

- **Default output path**:
  - **When** running `f8 compile` without `-o`
  - **Then** writes `./<model_name>.xlsx` to current directory.
- **Explicit destination & theme override**:
  - **When** running `f8 compile -o dist/output.xlsx --theme corporate_navy`
  - **Then** creates parent directories as needed and writes workbook styled with specified theme.
- **Stdout streaming with TTY protection**:
  - **When** running `f8 compile -o -` redirected to a file or pipe
  - **Then** writes raw binary OpenXML zip bytes directly to `stdout`.
  - **When** running `f8 compile -o -` connected to an interactive terminal
  - **Then** rejects operation with exit code `1` and guidance error on `stderr` to prevent display corruption.
- **Values-only freezing & cache omission**:
  - **When** passing `--values-only` or `--omit-cache`
  - **Then** passes corresponding flags to `EmissionOptions`.

### 5. Project Scaffolding & Institutional Archetypes (`f8 init`)

Scaffolds new projects or unpacks pre-wired institutional templates.

#### Scenarios

- **Blank project**:
  - **When** running `f8 init [path]` without `--template`
  - **Then** creates `f8.json` and starter `tables/table.json`, guaranteed to pass `f8 check`.
- **Institutional archetype unpacking**:
  - **When** running `f8 init <path> --template <name>` with `three-statement`, `lbo`, or `trading-comps`
  - **Then** unpacks embedded multi-table model files, applies optional `--theme`, and self-verifies cleanly.
- **Collision prevention**:
  - **When** initializing into a directory that already contains `f8.json`
  - **Then** aborts with exit code `1` without overwriting existing files.

### 6. Interactive Auditing & Model Inspection (`f8 inspect`)

Provides terminal auditing tools for model structures, 2D statement tables, cell audit cards, and dependency DAG trees.

#### Scenarios

- **Model overview summary**:
  - **When** running `f8 inspect`
  - **Then** renders summary listing sheets, tables, cell counts, driver inputs, formulas, and evaluation duration to `stdout`.
- **Table matrix inspection with accounting formatting**:
  - **When** running `f8 inspect table <table_id> --view values`
  - **Then** renders aligned Unicode table with accounting typography (`$ 100.0`, `(50.0)`, `15.0%`).
- **Formula auditing parity & markdown export**:
  - **When** running `f8 inspect table <table_id> --view formulas --format markdown`
  - **Then** outputs a GitHub Flavored Markdown table displaying live formula expressions.
- **Dual stacked auditing view**:
  - **When** running `f8 inspect table <table_id> --view both`
  - **Then** displays stacked cells showing evaluated values and formula expressions simultaneously.
- **Cell audit card**:
  - **When** running `f8 inspect cell <table_id> <row_id> <col_id>`
  - **Then** displays coordinates, canonical physical sheet address, evaluated value, formatting metadata, DSL formula AST, lowered Excel formula, precedents, and dependents.
- **Precedent & blast-radius DAG trees**:
  - **When** running `f8 inspect precedents <table_id> <row_id> <col_id>` or `f8 inspect dependents <table_id> <row_id> <col_id>`
  - **Then** renders immediate direct edges or recursive hierarchical tree (`├──`, `└──`) when `--recursive` is supplied.
- **Structured machine-readable JSON**:
  - **When** passing `--json` to any inspect subcommand
  - **Then** prints structured JSON to `stdout` on success, or structured error JSON to `stderr` with exit code `2` if a coordinate/table entity does not exist.

