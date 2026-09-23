---
title: Command-Line Interface (f8 CLI)
last_updated: 2026-09-23
domain: interfaces
status: active
---

## Purpose

The `f8` command-line executable (`crates/f8-cli`) provides a lightweight, headless verification tool, compiler runner, and terminal auditing interface for financial analysts and autonomous AI coding agents. It serves as an outer adapter consuming `f8-core`, enforcing zero production panics, clean stream discipline (`stdout` vs `stderr`), and deterministic 3-state exit codes.

## Architecture & Boundaries

- **Modules & components**:
  - `f8_cli::cli`: Clap command hierarchy and argument parsing with global `-p, --project` and `--json` flags.
  - `f8_cli::project::loader`: Manifest discovery via upward directory traversal, multi-file project assembly (`SheetSpec.tables`), and theme resolution.
  - `f8_cli::commands::check`: Headless model verification executing Passes 1–6a via `f8_core::Compiler::compile`.
  - `f8_cli::error`: Structured `CliError` classifying I/O/syntax failures vs compiler diagnostics and resolving exit codes.
- **Data flow & interfaces**:
  - Input: Discovers `f8.json` on disk by walking parent directories up to filesystem root or via explicit `-p, --project <path>`.
  - Loading: Reads manifest, resolves relative table file paths, deserializes `TableSpec`s, and loads built-in or custom JSON themes.
  - Delegation: Passes in-memory models directly to `Compiler::compile`. CLI contains zero domain layout, graph, or formula evaluation logic.
  - Output: Success summaries and valid JSON to `stdout`; error messages, compiler diagnostics, and error JSON to `stderr`.

## Requirements & Invariants

### 1. Deterministic 3-State Exit Codes

The CLI strictly classifies execution outcomes into three stable exit codes:

| Exit Code | Classification | Cause |
| :--- | :--- | :--- |
| `0` | Success | Operation completed and model verified cleanly. |
| `1` | Operational / CLI / I/O Error | CLI syntax error, missing `f8.json`, unreadable file, JSON deserialization failure. |
| `2` | Compiler Diagnostic | Passes 1–6a compiler failure (schema validation, cycle, unassigned driver, layout error, lowering failure). |

#### Scenarios

- **Verification success**:
  - **When** running `f8 check` on a valid project
  - **Then** prints human or JSON summary to `stdout` and exits with code `0`.
- **Operational error**:
  - **When** running `f8 check` in a directory without `f8.json` or with corrupt JSON
  - **Then** prints error message to `stderr` and exits with code `1`.
- **Compiler diagnostic**:
  - **When** running `f8 check` on a project containing a circular reference or layout overflow
  - **Then** outputs compiler diagnostic to `stderr` and exits with code `2`.

### 2. Upward Directory Traversal & Global Ergonomics

The CLI automatically traverses parent directories starting from the current working directory until it locates `f8.json`.

#### Scenarios

- **Upward discovery**:
  - **Given** an `f8.json` in a project root and a nested working directory
  - **When** executing `f8 check` without `-p`
  - **Then** discovers root `f8.json`, loads relative tables, and executes verification.
- **Global `-p, --project` flag**:
  - **When** specifying `-p <path>` before or after subcommands (`f8 -p ./models check` or `f8 check -p ./models`)
  - **Then** targets the explicit directory or manifest file directly.

### 3. Stream Discipline & Structured `--json` Output

Outputs are strictly segregated: operational output and summaries go to `stdout`; diagnostic errors go to `stderr`.

#### Scenarios

- **Human verification output**:
  - **When** running `f8 check`
  - **Then** prints step checkmarks, sheet/table/cell counts, and status to `stdout`.
- **Machine-readable JSON output**:
  - **When** running `f8 check --json`
  - **Then** prints formatted JSON object with `"status": "ok"` and summary metrics to `stdout` on success, or `"status": "error"` and `"exit_code": <code>` to `stderr` on failure.
