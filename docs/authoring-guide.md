# Model Authoring Guide

This guide provides an end-to-end walkthrough for authoring declarative financial models in f8. It details the project architecture, directory structure, step-by-step authoring workflow, and verification steps.

---

## 1. Core Modeling Philosophy

Traditional spreadsheets interleave business calculation logic, multi-dimensional time series, and visual presentation layout. f8 establishes a decoupled, compiler-driven paradigm:

- **Declarative source of truth**: Models are written as structured, human- and agent-readable JSON files (`f8.json` + `tables/*.json`). Spreadsheets (`.xlsx`) are unedited, compiled output artifacts.
- **Orthogonal 2D matrix**: Tables are general $R \times C$ matrices where rows and columns are symmetrical calculation axes. Axis roles and vector formulas define table behavior without requiring specialized table types.
- **Separation of concerns**:
  - *Identity*: Immutable programmatic identifiers (`TableId`, `RowId`, `ColId`) used in formulas and ASTs.
  - *Presentation*: Display labels (`headers`, descriptor cells), typographic indentation, formatting presets, and structural styles.
  - *Physical grid*: Sheet placement, column offsets, row heights, and freeze panes computed deterministically by the layout engine.
- **Three-tier specificity lattice**:
  $$\text{Cell override (0D point)} > \text{Vector rule (1D row or column)} > \text{Explicit null}$$
- **Deterministic verification**: Ambiguous formulas, missing coordinates, or cyclic references halt compilation with actionable diagnostic traces.

---

## 2. Project Directory Architecture

A f8 model is structured as a directory containing a root manifest and a `tables/` directory:

```text
my_model/
├── f8.json                  # Root manifest: metadata, theme, and sheet layout
└── tables/                    # Standalone table calculation specifications
    ├── assumptions.json
    └── income_statement.json
```

### Invariants
- **One table per file**: Table specifications reside in separate JSON files under `tables/` (or relative subdirectories).
- **No inlining**: Sheets in `f8.json` reference table relative file paths.
- **Global table ID uniqueness**: Every table `id` must be unique across the entire workbook.
- **Filename-to-ID correspondence**: The filename stem of each table file must match its declared table `id` (case-insensitively, ignoring hyphens and underscores, e.g. `income_statement.json` matches `IncomeStatement`).
- **Complete sheet assignment**: Every table specification must be assigned to exactly one sheet in `f8.json`.

---

## 3. Step-by-Step Walkthrough

The following walkthrough builds a verified two-sheet financial model with assumptions, forward growth vector formulas, and subtotal styling.

### Step 1: Initialize the Directory

```bash
mkdir -p my_model/tables
```

### Step 2: Write Root Manifest (`my_model/f8.json`)

The manifest defines global workbook properties, theme selection, and sheet tab organization:

```json
{
  "model_name": "Acme_Financial_Plan",
  "theme": "corporate_navy",
  "sheets": [
    {
      "name": "Summary",
      "tab_color": "#1B365D",
      "tables": [
        "tables/assumptions.json"
      ]
    },
    {
      "name": "Financials",
      "tab_color": "#2C4D75",
      "freeze_panes": {
        "row": 6,
        "col": 2
      },
      "show_gridlines": true,
      "tables": [
        "tables/income_statement.json"
      ]
    }
  ]
}
```

> [!NOTE]
> `tab_color` requires a valid hex color code (`#RGB`, `#RRGGBB`, or `#RRGGBBAA`). Named colors are prohibited.

### Step 3: Write Assumptions Table (`my_model/tables/assumptions.json`)

A 2-column parameter lookup table demonstrating descriptor line items and measure values:

```json
{
  "id": "Assumptions",
  "title": "Model Drivers & Assumptions",
  "columns": [
    { "kind": "data", "id": "param", "role": "descriptor", "headers": ["Parameter"] },
    { "kind": "data", "id": "val", "role": "measure", "headers": ["Value"] }
  ],
  "rows": [
    {
      "kind": "data",
      "id": "rev_growth",
      "format": "percentage",
      "decimals": 1,
      "cells": {
        "param": "Annual Revenue Growth",
        "val": 0.12
      }
    },
    {
      "kind": "data",
      "id": "cogs_rate",
      "format": "percentage",
      "decimals": 1,
      "cells": {
        "param": "COGS Margin (% Revenue)",
        "val": 0.40
      }
    },
    {
      "kind": "data",
      "id": "tax_rate",
      "format": "percentage",
      "decimals": 1,
      "cells": {
        "param": "Effective Tax Rate",
        "val": 0.25
      }
    }
  ]
}
```

### Step 4: Write Forecast Table (`my_model/tables/income_statement.json`)

A multi-period forecast statement demonstrating multi-tier headers, row-wide vector formulas, temporal offsets (`col.prev`), cross-table references, and structural styles:

```json
{
  "id": "IncomeStatement",
  "title": "Income Statement Forecast",
  "subtitle": "($ in Millions)",
  "currency": "usd",
  "columns": [
    { "kind": "data", "id": "line_item", "role": "descriptor", "headers": ["Line Item"] },
    { "kind": "data", "id": "2023A", "headers": ["Historical", "2023A"] },
    { "kind": "data", "id": "2024E", "headers": ["Projections", "2024E"] },
    { "kind": "data", "id": "2025E", "headers": ["Projections", "2025E"] },
    { "kind": "data", "id": "2026E", "headers": ["Projections", "2026E"] }
  ],
  "rows": [
    {
      "kind": "data",
      "id": "revenue",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"revenue\", col.prev] * (1.0 + Assumptions[\"rev_growth\", \"val\"])",
      "cells": {
        "line_item": "Total Revenue",
        "2023A": 100.0
      }
    },
    {
      "kind": "data",
      "id": "cogs",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"revenue\", @col] * Assumptions[\"cogs_rate\", \"val\"]",
      "cells": {
        "line_item": "Cost of Goods Sold",
        "2023A": 40.0
      }
    },
    {
      "kind": "data",
      "id": "gross_profit",
      "style": "subtotal",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
      "cells": {
        "line_item": "Gross Profit"
      }
    },
    {
      "kind": "gutter"
    },
    {
      "kind": "section_header",
      "title": "Operating Expenses"
    },
    {
      "kind": "data",
      "id": "sga",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"revenue\", @col] * 0.15",
      "cells": {
        "line_item": { "value": "Selling, General & Administrative", "indent": 1 },
        "2023A": 15.0
      }
    },
    {
      "kind": "data",
      "id": "ebit",
      "style": "subtotal",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"gross_profit\", @col] - IncomeStatement[\"sga\", @col]",
      "cells": {
        "line_item": "Operating Income (EBIT)"
      }
    },
    {
      "kind": "data",
      "id": "tax",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"ebit\", @col] * Assumptions[\"tax_rate\", \"val\"]",
      "cells": {
        "line_item": "Income Tax Expense"
      }
    },
    {
      "kind": "data",
      "id": "net_income",
      "style": "total",
      "format": "currency",
      "decimals": 1,
      "expr": "IncomeStatement[\"ebit\", @col] - IncomeStatement[\"tax\", @col]",
      "cells": {
        "line_item": "Net Income"
      }
    }
  ]
}
```

---

## 4. Verification & Inspection Workflow

With model files authored, the CLI provides immediate terminal feedback:

```bash
# 1. Fast headless verification (schema, graph acyclicity, evaluation)
f8 check

# 2. Inspect evaluated table values in terminal
f8 inspect table IncomeStatement

# 3. Inspect a specific cell, its lowered formula, and dependents
f8 inspect cell IncomeStatement net_income 2024E

# 4. Trace precedent dependencies
f8 inspect precedents IncomeStatement net_income 2024E --recursive

# 5. Compile to an Excel workbook (.xlsx)
f8 compile -o output/model.xlsx
```

---

## 5. Authoring Invariants Checklist

Before compiling, verify that your model conforms to these compiler rules:

- **Globally unique IDs**: Sheet names in `f8.json` and table IDs across the workbook must be globally unique.
- **Quoted coordinate keys**: Row and column keys in formula brackets must be double-quoted strings (e.g. `Table["revenue", @col]`). Unquoted keys trigger parse errors.
- **Mandatory table qualification**: Every formula reference must include its target table name (`Table[row, col]`). Unqualified references are rejected.
- **Active coordinate tokens**: Use explicit `@col` or `@row`. Bare `@` is retired and rejected.
- **Explicit descriptor values**: Every data row must supply a value in `cells` for every descriptor column. Vector formulas never broadcast across descriptor columns.
- **Roll-forward anchor**: The initial period in a `col.prev` row must supply an explicit literal override in `cells`.
- **No unassigned measure cells**: Every coordinate in a data row must resolve to a literal, formula, or explicit `null`.
- **Forward range slice order**: Coordinate ranges must declare endpoints in forward declaration order (`"2023A".."2027E"`).
- **Uppercase functions and booleans**: Excel function names (`SUM`, `IF`, `XLOOKUP`) and formula booleans (`TRUE`, `FALSE`) must be uppercase.
- **Omitted arguments**: Use an explicit underscore (`_`) for omitted optional arguments (e.g. `XLOOKUP(val, rng1, rng2, _, 0, 1)`).
- **Valid hex tab colors**: `tab_color` in `f8.json` must start with `#` (`#RGB`, `#RRGGBB`, `#RRGGBBAA`).
- **Acyclic formula graph**: Circular dependencies halt compilation with an exact semantic cycle trace.

---

## 6. Related Specifications & Reference Documents

- **DSL Specifications**:
  - [Specification Format & 2D Grid](./dsl/spec-format.md): Declarative JSON schemas, table structure, gutters, and multi-tier headers.
  - [Addressing Syntax](./dsl/addressing.md): Reference coordinates, active tokens, relative offsets, and range slicing.
  - [Formula Grammar](./dsl/grammar.md): Formal EBNF grammar, Pratt precedence, and operator rules.
  - [Semantic Formatting & Themes](./dsl/formatting.md): Format presets, display precision, structural styles, and themes.
  - [Modeling Patterns & Archetypes](./dsl/patterns.md): Canonical patterns (Horizontal Statements, Cap Tables, Corkscrews).
- **Compiler Internals**:
  - [Intermediate Representation (IR)](./compiler/ir.md): Cell-level IR and the 2D Specificity Resolution Lattice.
  - [Dependency Graph](./compiler/dependency-graph.md): Directed cell graph, cycle prevention, and topological sort.
  - [In-Memory Evaluator](./compiler/evaluator.md): Topological DAG evaluation loop and runtime values.
  - [Excel Parity](./compiler/parity.md): Precision, coercion mechanics, date arithmetic, and criteria rules.
  - [Spatial Layout Planning](./compiler/spatial-layout.md): Physical grid assignment and column width arbitration.
  - [Formula Lowering & Emission](./compiler/lowering-and-emission.md): Excel coordinate lowering and binary `.xlsx` emission.
- **Functions & Interfaces**:
  - [Standard Function Catalog](./functions/catalog.md): Supported Excel built-ins, signatures, and calculation behaviors.
  - [Function Registry Architecture](./functions/registry.md): `ExcelFunction` trait, parameter contracts, and dispatch.
  - [Command-Line Interface (CLI)](./interfaces/cli.md): CLI subcommands, workflows, and inspection commands.
