# Model Context Protocol (MCP) Interface

This document defines the architecture, interaction models, and tool catalog of f8's Model Context Protocol (MCP) server (`f8-mcp`).

---

## 1. Overview & Agent-First Role

The Model Context Protocol (MCP) server establishes the **Adapter / Tooling Layer** between autonomous AI agents and the f8 compiler core. 

f8 is architected with **AI agents as first-class citizens**:
- Agents interact with f8 by invoking typed MCP tools with **structured JSON payloads**, rather than writing Rust code.
- Scaffolding tools instantiate curated canonical specification files (`f8.json` and `tables/*.json`) directly on disk.
- Agents verify model correctness instantly via headless in-memory evaluation and deterministic compiler diagnostics, creating an autonomous self-correction loop without requiring external spreadsheet runtimes.

```mermaid
flowchart TD
    subgraph Agent ["AI Agent (LLM)"]
        A["User Intent / Prompt"] --> B["Tool Call Selection"]
        H["Self-Correction / Verification"] <-- B
    end

    subgraph Server ["f8 MCP Server (f8-mcp)"]
        B -->|"Tool JSON Arguments"| C["MCP Tool Handler"]
        C -->|"Scaffold Request"| T["Template Ingest\n(templates/)"]
        C -->|"Canonical Spec Ingest"| D["TableSpec Validation\n(f8-core::model)"]
    end

    subgraph Workspace ["Project Workspace on Disk"]
        T --> F["Canonical Specifications\n(f8.json + tables/*.json)"]
        D --> F
    end

    subgraph Core ["f8 Core Compiler (f8-core)"]
        F --> G["6-Pass Compilation\n(DAG -> In-Memory Eval -> Layout -> Lowering)"]
        G -->|"Evaluated Cells & Diagnostics"| C
    end

    C -->|"Structured Tool Output\n(Values, ASTs, Cycle Traces)"| H
```

---

## 2. Declarative Authoring & "Scaffold-and-Fill"

f8 strictly avoids imperative micro-mutation tools (`insert_row`, `update_cell`, `extend_timeline`). Instead, agents interact through coarse-grained, declarative operations:

### 2.1. Institutional Template Scaffolding (`f8_scaffold_template`)

When building standard financial models, the agent invokes `f8_scaffold_template` to generate a complete institutional skeleton:

```json
// Tool Call
{
  "template": "three-statement",
  "sheet": "Financials"
}
```

The scaffolding engine drops battle-tested canonical JSON files directly into the project (`tables/income_statement.json`, `tables/balance_sheet.json`, `tables/cash_flow.json`, `tables/assumptions.json`):
- **Pre-wired formulas**: Net Income links to Retained Earnings and Operating Cash Flow; Cash Flow ending cash balances the Balance Sheet; subtotal slices (`SUM(cogs..sga)`) are established.
- **Institutional styling**: Accounting double-underlines, currency formatting, percentage precision, and section groupings are pre-applied.
- **Unpopulated driver inputs**: Historical actuals and projection assumptions are omitted or left without values.

### 2.2. Canonical Table I/O (`f8_save_table`, `f8_get_table`)

When creating custom tables or modifying existing ones, agents work with canonical [`TableSpec`](../dsl/spec-format.md#3-table-specification-schema-tablesidjson) payloads:

- **Filesystem-enabled agents** (Antigravity, Claude Code, Cursor): Edit `tables/*.json` directly using standard text/JSON tools.
- **Chat-only MCP agents** (Claude Desktop): Retrieve the table via `f8_get_table`, edit the JSON in context, and persist via `f8_save_table`.

```json
// Example: f8_save_table tool invocation
{
  "sheet": "Financials",
  "table": {
    "id": "IncomeStatement",
    "title": "Income Statement",
    "subtitle": "($ in Millions)",
    "currency": "USD",
    "columns": [
      { "kind": "data", "id": "line_item", "role": "descriptor", "headers": ["Line Item"] },
      { "kind": "data", "id": "2023A", "headers": ["Historical Actuals", "2023A"] },
      { "kind": "data", "id": "2024E", "headers": ["Forecast Projections", "2024E"] }
    ],
    "rows": [
      { "kind": "section_header", "title": "Revenue" },
      {
        "kind": "data",
        "id": "revenue",
        "format": "currency",
        "decimals": 1,
        "cells": { "line_item": "Net Revenue", "2023A": 100.0 },
        "expr": "IncomeStatement[\"revenue\", col.prev] * (1.0 + Assumptions[\"rev_growth\", @col])"
      }
    ]
  }
}
```

---

## 3. Headless Verification & Self-Correction Loop

Traditional spreadsheet development requires human visual verification in Excel. f8 provides agents with programmatic compiler introspection:

### 3.1. Fast Compilation & Missing Input Checklist (`f8_compile`)

During the "Scaffold-and-Fill" loop, calling `f8_compile` with unpopulated inputs scans all tables across the workbook, aggregates every unassigned cell, and returns a structured, complete checklist of missing values in a single turn (rather than halting on the first unassigned coordinate):

```json
// Structured Error Response from f8_compile
{
  "status": "error",
  "error_type": "MissingRequiredInput",
  "missing_inputs": [
    { "table": "Assumptions", "row": "rev_growth", "col": "2024E" },
    { "table": "Assumptions", "row": "rev_growth", "col": "2025E" },
    { "table": "IncomeStatement", "row": "revenue", "col": "2023A" }
  ]
}
```

The agent consumes this diagnostic output as an automated task list, populates all missing values across the JSON specification files, and re-compiles in a single follow-up step.

### 3.2. Immediate Value & Formula Inspection (`f8_inspect_cell`, `f8_inspect_table`)

Powered by `f8-core`'s unified `CompiledModel` facade, agents query calculated values, underlying formula expressions, and dependency linkages directly without triggering redundant compiler passes:

```json
// Tool Call: f8_inspect_cell
{ "table_id": "IncomeStatement", "row_id": "total_opex", "col_id": "2028E" }

// Structured Response
{
  "evaluated_value": 74.2,
  "formula_expr": "SUM(IncomeStatement[\"cogs\"..\"sga\", @col])",
  "lowered_excel_formula": "=SUM(C14:C16)",
  "format": "currency",
  "decimals": 1,
  "precedents": [
    "IncomeStatement[\"cogs\", \"2028E\"]",
    "IncomeStatement[\"rd\", \"2028E\"]",
    "IncomeStatement[\"sga\", \"2028E\"]"
  ],
  "dependents": [
    "IncomeStatement[\"ebit\", \"2028E\"]"
  ]
}
```

Agents can also query `f8_inspect_table` to receive the entire evaluated 2D matrix formatted as Markdown or JSON for spatial verification across rows and columns:

```json
// Tool Call: f8_inspect_table (Formula Auditing Mode)
{
  "table_id": "IncomeStatement",
  "view": "formulas",
  "format": "markdown"
}
```

The response returns an aligned Markdown table displaying raw inputs for driver cells and semantic formulas for calculation cells, enabling the agent's spatial attention to verify horizontal vector consistency and temporal shifts in a single turn.

### 3.3. Blast Radius & Dependent Tracing (`f8_inspect_dependents`)

Before modifying driver assumptions or renaming line items, agents perform impact analysis by querying outgoing DAG edges from `f8-core`:

```json
// Tool Call: f8_inspect_dependents
{
  "table_id": "Assumptions",
  "row_id": "rev_growth",
  "col_id": "2024E",
  "recursive": true
}

// Structured Response
{
  "coordinate": "Assumptions[\"rev_growth\", \"2024E\"]",
  "direct_dependents": [
    "IncomeStatement[\"revenue\", \"2024E\"]"
  ],
  "transitive_dependents": [
    "IncomeStatement[\"revenue\", \"2024E\"]",
    "IncomeStatement[\"gross_profit\", \"2024E\"]",
    "IncomeStatement[\"ebit\", \"2024E\"]",
    "IncomeStatement[\"net_income\", \"2024E\"]",
    "BalanceSheet[\"retained_earnings\", \"2024E\"]",
    "CashFlow[\"net_income\", \"2024E\"]"
  ]
}
```

Autonomous agent use cases:
- **Refactoring & renaming safety**: Identifies every dependent formula before modifying or deleting an identifier.
- **Dynamic statement linkage verification**: Proves that three-statement models are dynamically integrated (e.g. confirming Net Income flows into Retained Earnings and Operating Cash Flow).
- **Orphan calculation detection**: Detects calculated rows that have zero downstream consumers.

### 3.4. Deterministic Cycle Traces (`f8_explain_cycle`)

When circular references occur, Tarjan's Strongly Connected Components (SCC) pass emits exact semantic cycle paths rather than generic `#REF!` errors:

```text
CyclicDependencyDetected:
IncomeStatement["tax", "2024E"] ──► IncomeStatement["ebit", "2024E"] ──► IncomeStatement["tax", "2024E"]
```

The agent consumes this trace, identifies the erroneous reference, updates the formula in the JSON, and re-verifies in a single autonomous turn.

---

## 4. MCP Tool Catalog

### 4.1. Project & Sheet Management
- `f8_init_project`: Initializes a standard model repository with root `f8.json` and directory layout.
- `f8_add_sheet`: Appends or configures a physical worksheet tab (freeze panes, tab colors, gridlines).

### 4.2. Scaffolding & Canonical Table I/O
- `f8_scaffold_template`: Scaffolds full institutional model archetypes (`three-statement`, `lbo`, `trading-comps`, `cap-table`) into canonical JSON on disk.
- `f8_save_table`: Ingests, validates, and persists a canonical `TableSpec` JSON payload into a sheet.
- `f8_get_table`: Retrieves the canonical `TableSpec` JSON for a specified table.

### 4.3. Compilation, Inspection & Diagnostics
- `f8_compile`: Executes full 6-pass compilation, topological DAG evaluation, and optional `.xlsx` binary emission.
- `f8_inspect_cell`: Evaluates and returns the calculated cell value, formula AST, upstream precedents, and downstream dependents.
- `f8_inspect_dependents`: Traces downstream dependent calculations (blast radius) for a coordinate, supporting direct and recursive queries.
- `f8_inspect_table`: Returns the evaluated headless 2D matrix (as JSON or Markdown table) across values, formulas, or dual auditing views.
- `f8_explain_cycle`: Returns the formatted semantic cycle path when Tarjan SCC detects circular dependency graphs.

---

See [Specification Format](../dsl/spec-format.md) for canonical JSON schema definitions, [CLI Interface](cli.md) for command-line workflows, and [Architecture Overview](../architecture.md) for compiler passes.
