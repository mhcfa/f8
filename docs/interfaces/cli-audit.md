# CLI Inspection Audit & Agent Ergonomics Evaluation

This document records the empirical audit, usability evaluation, and defect analysis of the `f8` command-line interface (`f8-cli`) when executed against the built-in institutional templates ([`three-statement`](../../crates/f8-cli/templates/three-statement), [`lbo`](../../crates/f8-cli/templates/lbo), [`trading-comps`](../../crates/f8-cli/templates/trading-comps), and [`blank`](../../crates/f8-cli/templates/blank)).

The audit assesses the interface from the perspective of an autonomous financial modeling AI agent, analyzing context token efficiency, information density, cognitive friction, and runtime reliability.

---

## 1. Executive Summary

The CLI implementation provides high compilation speed (< 30 ms for 286-cell LBO models), rigorous DAG validation, clean visual table typography, and expressive dual-formula inspection (IR AST expressions alongside lowered Excel formulas).

However, empirical testing against full models revealed **two critical runtime defects** and **six major interface friction points** that hinder autonomous agent workflows and token economics.

### Priority Defect & Enhancement Matrix

| Priority | Issue / Defect | Component | Symptoms / Evidence | Proposed Fix |
| :--- | :--- | :--- | :--- | :--- |
| **P0 (Bug)** | **Combinatorial DAG path explosion** | `f8-core` (`compiled/mod.rs`) | `f8 inspect precedents ReturnsSummary irr val --recursive` emitted **439,844 lines** for 150 unique cells, causing memory bloat and CLI stall. | Implement DAG-aware memoization/pruning in `build_precedent_tree_node` and `build_dependent_tree_node`. |
| **P0 (Bug)** | **SIGPIPE / broken pipe panic** | `f8-cli` (`main.rs` / stdout) | Piping output to Unix filters (`f8 inspect ... \| head -n 25`) panics with `Broken pipe (os error 32)`. | Reset SIGPIPE handler or handle `io::ErrorKind::BrokenPipe` gracefully. |
| **P1 (Agent DX)** | **Programmatic ID visibility blindspot** | `f8-cli` (`render/table.rs`) | Tables display user labels (`Cost of Goods Sold`, `2024E`), while `inspect cell` requires programmatic IDs (`cogs`, `y2024e`). Querying display labels errors. | Add `--show-ids` (or `--ids`) flag or render IDs in descriptors/headers. |
| **P1 (Agent DX)** | **Omission of evaluated values in dependency trees** | `f8-cli` (`render/tree.rs`, `render/cell.rs`) | Tree and cell cards list only cell coordinates (`Assumptions["rev_growth", "val"]`) without evaluated numbers, preventing single-glance formula verification. | Print evaluated values alongside coordinates in tree and card renders. |
| **P2 (Ergonomics)** | **Rigid coordinate arguments** | `f8-cli` (`commands/inspect.rs`, `cli.rs`) | Accepts only `<table> <row> <col>`; cannot parse canonical strings `Table["row", "col"]` or Excel addresses `Model!D20`. | Add coordinate parser supporting 3 args, canonical IR string, or sheet A1 address. |
| **P2 (Polish)** | **Thousands comma on fiscal years / IDs** | `f8-cli` (`render/table.rs`) | Fiscal years render as `2,029` due to generic number comma formatting. | Support unformatted integer/year display heuristics or presets. |
| **P2 (Polish)** | **Flattened multi-tier column headers** | `f8-cli` (`render/table.rs`) | Category header tiers ("Market Data", "Balance Sheet") are dropped, showing only lowest tiers. | Render parent header groupings in terminal and markdown tables. |
| **P2 (Agent DX)** | **Table schema / outline mode** | `f8-cli` (`cli.rs`, `commands/inspect.rs`) | Wide/deep tables dump thousands of tokens into agent context; no command exists to list column/row definitions without cell grids. | Add `f8 inspect table <Table> --schema` flag. |

---

## 2. Empirical Test Runs Across Templates

### 2.1. Model Summary (`f8 inspect -p <project>`)

#### Command
```bash
f8 inspect -p crates/f8-cli/templates/three-statement
```

#### Output
```text
Model:     ThreeStatementFinancialModel
Theme:     corporate_navy
Sheets (1):
  • Model (4 tables, 139 cells)
      - Assumptions [7 rows x 2 cols] (14 cells)
      - IncomeStatement [12 rows x 5 cols] (40 cells)
      - CashFlow [15 rows x 5 cols] (40 cells)
      - BalanceSheet [15 rows x 5 cols] (45 cells)
Total:     1 sheets, 4 tables, 139 cells (45 driver inputs, 94 formulas)
Status:    DAG acyclic (0 cycles), evaluated in 2.7ms
```

#### Evaluation
- **Strengths**: Extremely concise, fast, and informative. Establishes the sheet/table breakdown and acyclicity status immediately.
- **Weaknesses**: Omits human-readable table titles (e.g. `"Income Statement"`), which are present in the JSON schema (`"title"`). Displaying table titles alongside IDs accelerates semantic comprehension.

---

### 2.2. Table Grid (`f8 inspect table`)

#### Command
```bash
f8 inspect table IncomeStatement -p crates/f8-cli/templates/three-statement
```

#### Output
```text
┌────────────────────────────┬─────────┬─────────┬─────────┬─────────┐
│ Line Item                  │   2023A │   2024E │   2025E │   2026E │
├────────────────────────────┼─────────┼─────────┼─────────┼─────────┤
│ Revenue                    │ $ 100.0 │ $ 115.0 │ $ 132.2 │ $ 152.1 │
│   Cost of Goods Sold       │  $ 40.0 │  $ 46.0 │  $ 52.9 │  $ 60.8 │
│ Gross Profit               │  $ 60.0 │  $ 69.0 │  $ 79.3 │  $ 91.3 │
│ Operating Expenses                                                 │
├────────────────────────────┼─────────┼─────────┼─────────┼─────────┤
│   Salaries & SG&A Overhead │  $ 20.0 │  $ 20.0 │  $ 20.0 │  $ 20.0 │
│   Depreciation Expense     │  $ 10.0 │  $ 10.0 │  $ 10.0 │  $ 10.0 │
│ Operating Income (EBIT)    │  $ 30.0 │  $ 39.0 │  $ 49.3 │  $ 61.3 │
│   Income Tax Provision     │   $ 6.0 │   $ 7.8 │   $ 9.9 │  $ 12.3 │
│ Net Income                 │  $ 24.0 │  $ 31.2 │  $ 39.5 │  $ 49.0 │
└────────────────────────────┴─────────┴─────────┴─────────┴─────────┘
```

#### Evaluation
- **Strengths**: High-quality accounting presentation. Indentation levels, section headers, currencies, and parentheses for negatives match institutional standards. Markdown output (`--format markdown`) is compact and agent-friendly.
- **Weaknesses**:
  - **Programmatic ID disconnect**: The headers display `2024E`, but the column ID is `y2024e`. The row displays `Cost of Goods Sold`, but the row ID is `cogs`.
  - **Dead end for downstream tool calls**: An agent cannot take the displayed labels and query `f8 inspect cell`. Attempting `f8 inspect cell IncomeStatement "Cost of Goods Sold" "2024E"` results in `Error: Cell 'IncomeStatement["Cost of Goods Sold", "2024E"]' not found`.
  - In `trading-comps`, the summary statistics row displays `Low`, but its row ID is `min`. Querying `f8 inspect cell Comps low ev` fails.

---

### 2.3. Cell Audit Card (`f8 inspect cell`)

#### Command
```bash
f8 inspect cell IncomeStatement revenue y2024e -p crates/f8-cli/templates/three-statement
```

#### Output
```text
Coordinate:     IncomeStatement["revenue", "y2024e"]
Sheet Address:  Model!D20
Evaluated:      $ 115.0 (Currency, 1 decimal, USD)
Formula Expr:   IncomeStatement["revenue", col.prev] * (1 + Assumptions["rev_growth", "val"])
Excel Formula:  =C20*(1+$C$7)

Precedents (2):
  ├── Assumptions["rev_growth", "val"]
  └── IncomeStatement["revenue", "y2023a"]

Dependents (6):
  ├── BalanceSheet["working_capital", "y2024e"]
  ├── CashFlow["change_in_wc", "y2025e"]
  ├── CashFlow["change_in_wc", "y2024e"]
  ├── IncomeStatement["gross_profit", "y2024e"]
  ├── IncomeStatement["cogs", "y2024e"]
  └── IncomeStatement["revenue", "y2025e"]
```

#### Evaluation
- **Strengths**: Best-in-class audit card. Gives logical coordinate, sheet A1 address, evaluated value with formatting, high-level declarative expression, lowered Excel formula, and immediate fan-in/fan-out.
- **Weaknesses**:
  - Does not show current evaluated values of precedents (`Assumptions["rev_growth", "val"]` = `15.0%`, `IncomeStatement["revenue", "y2023a"]` = `$ 100.0`). The agent must make additional tool calls to verify arithmetic.
  - Requires 3 positional arguments. Does not accept `Model!D20` or canonical string `IncomeStatement["revenue", "y2024e"]`.

---

### 2.4. Dependency Tree & Blast Radius (`f8 inspect precedents / dependents`)

#### Command
```bash
f8 inspect precedents ReturnsSummary irr val -p crates/f8-cli/templates/lbo --recursive
```

#### Result
- Emitted **439,844 lines of output**.
- Execution stalled while serializing hundreds of thousands of redundant nodes.
- Total unique cells in the entire upstream calculation DAG: **150 cells**.

---

## 3. In-Depth Root Cause Analysis

### 3.1. Combinatorial DAG Path Explosion

#### Mechanism
In `crates/f8-core/src/compiler/compiled/mod.rs`:
```rust
fn build_precedent_tree_node(
    &self,
    cell_id: &CellId,
    visited_ancestors: &mut HashSet<CellId>,
    all_visited: &mut HashSet<CellId>,
) -> DependencyNode {
    all_visited.insert(cell_id.clone());
    let (sheet_address, value, excel_formula) = self.get_node_display_metadata(cell_id);

    let mut children = Vec::new();
    if !visited_ancestors.contains(cell_id) {
        visited_ancestors.insert(cell_id.clone());
        let deps = self.graph.dependencies_of(cell_id);
        for dep in deps {
            children.push(self.build_precedent_tree_node(dep, visited_ancestors, all_visited));
        }
        visited_ancestors.remove(cell_id); // <-- Root cause: unmarks node on backtrack
    }

    DependencyNode { ... }
}
```

#### Analysis
- Financial calculation graphs are directed acyclic graphs (DAGs) with extensive reconvergent fan-out (e.g. debt balances and interest drive multiple lines across multiple forecast periods).
- Removing `cell_id` from `visited_ancestors` on backtrack causes the traversal to enumerate **every simple path** from the query root to all upstream nodes rather than traversing the DAG once.
- The number of paths in a multi-period financial model with debt sweeps and working capital grows exponentially ($O(2^d)$ to $O(k^d)$ where $d$ is depth).
- For `ReturnsSummary["irr", "val"]`, 150 unique cells expand into 439,844 tree nodes.

#### Solution
1. **DAG Pruning**: When a node has already been visited and expanded elsewhere in the tree, do not re-expand its subtree. Render it as a leaf or mark it: `└── CellId (already expanded)`.
2. **Flat Topological Mode**: Provide an optional `--format flat` or `--view list` flag returning a deduplicated, topologically ordered sequence of calculations with level depths.

---

### 3.2. UNIX SIGPIPE Panic

#### Mechanism
When piping CLI output to pagers or stream truncators:
```bash
f8 inspect dependents Assumptions rev_growth val -p crates/f8-cli/templates/three-statement --recursive --json | head -n 20
```
The receiving process (`head`) closes the pipe after 20 lines. The Rust runtime's `println!` encounters `std::io::ErrorKind::BrokenPipe` and panics:
```text
thread 'main' (3585882) panicked at library/std/src/io/stdio.rs:1166:9:
failed printing to stdout: Broken pipe (os error 32)
```

#### Solution
Reset the SIGPIPE handler at CLI startup (standard for Unix CLI utilities) or catch broken pipe errors in the top-level command runner and terminate with exit code 0.

---

### 3.3. Programmatic ID Visibility Gap

#### Problem Statement
An AI agent inspects a model via CLI to understand what to edit or verify. It sees:
```text
┌──────────────────────┬────────┐
│ Line Item            │  2024E │
├──────────────────────┼────────┤
│ Cost of Goods Sold   │ $ 46.0 │
└──────────────────────┴────────┘
```
The agent does not know that `Cost of Goods Sold` maps to row ID `cogs` and `2024E` maps to column ID `y2024e`.

#### Solution
1. **`--show-ids` flag on `f8 inspect table`**:
   - Headers: `2024E [y2024e]`
   - Row descriptors: `Cost of Goods Sold [cogs]`
   - Or display a dedicated `[Row ID]` column when `--show-ids` is active.
2. **Table Schema Subcommand / Option**:
   `f8 inspect table <Table> --schema` (or `f8 inspect table <Table> --info`):
   Outputs column schemas (ID, role, type, headers) and row schemas (ID, style, format, formula expression) without rendering cell grids.

---

### 3.4. Precedent & Dependent Value Elision

#### Problem Statement
In `f8 inspect cell`:
```text
Precedents (2):
  ├── Assumptions["rev_growth", "val"]
  └── IncomeStatement["revenue", "y2023a"]
```
In `f8 inspect precedents`:
```text
Transitive Precedent Tree (4 calculations):
  ├── Assumptions["rev_growth", "val"]
  └── IncomeStatement["revenue", "y2025e"]
      ├── Assumptions["rev_growth", "val"]
      └── IncomeStatement["revenue", "y2024e"]
          ├── Assumptions["rev_growth", "val"]
          └── IncomeStatement["revenue", "y2023a"]
```
Neither view prints evaluated runtime values, even though `DependencyNode` already stores `value: Option<CellValue>` and `excel_formula: Option<String>`.

#### Solution
Update `render_cell` and `render_tree` to format evaluated values:
```text
Precedents (2):
  ├── Assumptions["rev_growth", "val"] = 15.0%
  └── IncomeStatement["revenue", "y2023a"] = $ 100.0
```

---

## 4. Prioritized Implementation Roadmap

### Phase 1: Critical Bug Fixes (P0)
1. **Fix DAG expansion in `f8-core`**:
   - Modify `build_precedent_tree_node` and `build_dependent_tree_node` in [`crates/f8-core/src/compiler/compiled/mod.rs`](file:///home/mhcfa/Work/f8/crates/f8-core/src/compiler/compiled/mod.rs).
   - Track globally expanded nodes to prune duplicate subtrees while preserving cycle detection.
   - Verify `inspect precedents ReturnsSummary irr val --recursive` completes instantly with < 250 total lines.
2. **Handle SIGPIPE in `f8-cli`**:
   - Ensure clean exit on broken stdout pipes.

### Phase 2: Agent Usability & Visibility (P1)
1. **Implement `--show-ids` for `f8 inspect table`**:
   - Add `--show-ids` flag to `InspectTableArgs`.
   - Update `render_unicode_table` and `render_markdown_table` to append programmatic IDs.
2. **Display evaluated values in dependency trees**:
   - Update `render_node_list` in [`crates/f8-cli/src/render/tree.rs`](file:///home/mhcfa/Work/f8/crates/f8-cli/src/render/tree.rs).
   - Update `render_cell` in [`crates/f8-cli/src/render/cell.rs`](file:///home/mhcfa/Work/f8/crates/f8-cli/src/render/cell.rs) to include formatted values for precedents and dependents.

### Phase 3: Ergonomics & Formatting Polish (P2)
1. **Flexible coordinate parser**:
   - Allow `f8 inspect cell` to parse `<table> <row> <col>`, `'Table["row", "col"]'`, or physical sheet coordinates like `Model!D20` / `D20`.
2. **Numeric year / integer formatting**:
   - Avoid thousands commas on integer years (e.g. `2029` instead of `2,029`).
3. **Multi-tier column header rendering**:
   - Support rendering stacked header categories in table inspection.
4. **Table schema inspection**:
   - Add `f8 inspect table <Table> --schema` for token-efficient table structure summaries.
