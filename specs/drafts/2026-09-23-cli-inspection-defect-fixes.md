---
title: CLI inspection runtime defect fixes
date: 2026-09-23
status: draft
target-living-spec:
  - specs/living-specs/compiler/compiled-model.md
  - specs/living-specs/interfaces/cli.md
---

## Proposal

### What

Fix two critical runtime defects in model inspection identified in [`docs/interfaces/cli-audit.md`](../../docs/interfaces/cli-audit.md):
1. **Combinatorial DAG path explosion** in `f8-core` recursive precedent and dependent tree construction ([`crates/f8-core/src/compiler/compiled/mod.rs`](../../crates/f8-core/src/compiler/compiled/mod.rs)).
2. **Unhandled `SIGPIPE` panic** (`Broken pipe (os error 32)`) in `f8-cli` when piping stdout output into downstream Unix utilities such as `head`, `less`, or `grep` ([`crates/f8-cli/src/main.rs`](../../crates/f8-cli/src/main.rs)).

### Why

- **Exponential path bloat in DAG trees**: Financial models feature extensive reconvergent fan-out (e.g. shared dates, growth drivers, tax rates, and debt balances driving multi-period lines). The current tree traversal unmarks visited nodes upon backtracking, turning a DAG traversal into an exhaustive enumeration of all simple paths. In the `lbo` template, querying precedents for `ReturnsSummary["irr", "val"]` traverses 439,844 paths for just 150 unique cells, taking ~17 seconds, consuming megabytes of memory, and overwhelming AI agent context windows.
- **Pipeline breakage on stream truncation**: Piping CLI output into Unix utilities (`f8 inspect ... | head -n 25`) panics because the Rust runtime ignores `SIGPIPE` by default, triggering an `io::ErrorKind::BrokenPipe` error on subsequent `println!` calls. This violates zero-panic production standards and disrupts shell workflows.

### Scope

- **In-scope**:
  - Add global expansion tracking and DAG pruning to `CompiledModel::build_precedent_tree_node` and `CompiledModel::build_dependent_tree_node`.
  - Extend `DependencyNode` with `pub is_pruned: bool` (`#[serde(default)]`) to indicate pruned subtrees.
  - Update `f8-cli` tree rendering (`render/tree.rs`) to annotate pruned nodes with ` (already visited)` and terminate traversal recursion on pruned branches.
  - Reset `SIGPIPE` to default (`SIG_DFL`) on Unix targets in `f8-cli::main` and install a clean broken pipe handler for cross-platform robustness.
  - Unit tests in `f8-core` asserting linear, non-duplicative tree generation on diamond DAG structures.
  - Integration tests in `f8-cli` asserting `ReturnsSummary irr val --recursive` generates concise output (< 250 lines) in < 100ms, and pipes cleanly to Unix filters without panic.

- **Out-of-scope**:
  - Phase 2 agent DX enhancements: `--show-ids` table flag and evaluated cell values in dependency trees ([`docs/interfaces/cli-audit.md`](../../docs/interfaces/cli-audit.md) §4 Phase 2).
  - Phase 3 ergonomics and formatting polish: flexible coordinate parser, integer year heuristics, multi-tier headers, and table outline schema mode ([`docs/interfaces/cli-audit.md`](../../docs/interfaces/cli-audit.md) §4 Phase 3).

### What Changes

- `Cargo.toml`: Add `libc = "0.2"` to workspace dependencies under `[target.'cfg(unix)'.dependencies]`.
- `crates/f8-core/src/compiler/compiled/model.rs`: Add `#[serde(default)] pub is_pruned: bool` to `DependencyNode`.
- `crates/f8-core/src/compiler/compiled/mod.rs`: Update `build_precedent_tree_node` and `build_dependent_tree_node` to accept `expanded: &mut HashSet<CellId>`. Prune recursion on already expanded nodes and set `is_pruned = true`.
- `crates/f8-core/tests/`: Add unit tests for diamond and reconvergent DAG tree traversal pruning.
- `crates/f8-cli/Cargo.toml`: Add `libc.workspace = true` under `[target.'cfg(unix)'.dependencies]`.
- `crates/f8-cli/src/main.rs`: Reset `SIGPIPE` handler to `SIG_DFL` on Unix at process entry.
- `crates/f8-cli/src/lib.rs`: Set up broken pipe panic guard to ensure graceful zero-code process termination on early closed pipe.
- `crates/f8-cli/src/render/tree.rs`: Render ` (already visited)` on pruned nodes.
- `crates/f8-cli/tests/`: Add CLI regression tests for recursive tree line bounds and `head` piping.

### Capabilities

- **New capabilities**:
  - `f8 inspect precedents <cell> --recursive` and `f8 inspect dependents <cell> --recursive` execute in sub-10ms across complex financial models.
  - Piped commands (`f8 inspect ... | head -n 20`) exit cleanly with code 0 without Rust runtime panic messages.
  - Machine-readable JSON output contains `"is_pruned": true` on pruned dependency nodes.
- **Modified capabilities**:
  - `DependencyTree::nodes` returns a pruned DAG tree where each formula node is expanded exactly once.

---

## Design

### Goals

- Eliminate exponential path explosion: reduce `ReturnsSummary irr val --recursive` line count from 439,844 to < 250 lines and latency from ~17s to < 10ms.
- Eliminate broken pipe crashes: piping any `f8` command to standard stream filters produces zero panic output and exits cleanly.
- Maintain accurate metrics: preserve exact `direct_count` and `transitive_count` values reflecting unique calculations.

### Invariants

- **Acyclicity guarantee**: Model graphs are verified acyclic during Pass 3. Tree traversal must retain defensive recursion guards (`visited_ancestors`) to prevent infinite recursion on arbitrary graphs.
- **Transitive uniqueness**: `DependencyTree::transitive_count` must strictly equal the count of unique reachable cells in the calculation subgraph.
- **Deterministic exit codes**: Pipeline termination via broken pipe must exit cleanly with code `0` (or `141` if terminated by `SIGPIPE`), never code `101` (panic).
- **Single source of truth**: Pruning logic resides in `f8-core` so all outer adapters (`f8-cli`, `f8-mcp`, WebAssembly) benefit uniformly.

### Decisions

- **Decision 1: Explicit pruning flag on `DependencyNode` vs dropping duplicate nodes**:
  - **Choice**: Keep duplicate dependency nodes in the parent's children list as leaf nodes with `is_pruned = true` and `children = Vec::new()`. Annotate with ` (already visited)` in terminal renders.
  - **Rationale**: If duplicate nodes were completely omitted from child lists, the rendered tree would obscure direct dependency relationships for formulas that depend on already-expanded cells. Marking them as pruned leaves makes every direct dependency visible while preventing recursive subtree re-expansion.
  - **Alternatives considered**:
    - *Full path enumeration (status quo)*: Exponential complexity ($O(k^d)$), causing memory exhaustion and CLI stalls.
    - *Omitting duplicate nodes entirely*: Violates dependency transparency; users and agents cannot see all direct inputs of downstream formulas.

- **Decision 2: Reset `SIGPIPE` to `SIG_DFL` on Unix with broken pipe panic hook fallback**:
  - **Choice**: On Unix targets, reset `SIGPIPE` signal handler to `SIG_DFL` before entering the main loop. In addition, install a panic hook that intercepts `Broken pipe` panics and exits with code 0.
  - **Rationale**: Setting `SIG_DFL` is standard Unix CLI practice (used by `ripgrep`, `grep`, `cat`), allowing the OS kernel to terminate the process without runtime overhead or buffer flush errors. The panic hook guarantees zero panic outputs on non-Unix platforms or in edge cases where Rust stdio catches `EPIPE` before the signal is raised.
  - **Alternatives considered**:
    - *Wrapping every `println!` / `writeln!` in error checks*: Highly invasive across all commands, error-prone, and adds boilerplate to every print site.
    - *Doing nothing*: Breaks automated pipelines, scripts, and agent tool execution.

### Data Contracts

#### Modified `DependencyNode`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DependencyNode {
    /// Coordinate identifier of this cell.
    pub cell_id: CellId,
    /// Canonical Excel sheet address (e.g. `"Financials!C14"`).
    pub sheet_address: String,
    /// Evaluated runtime value.
    pub value: Option<CellValue>,
    /// Lowered Excel formula string if applicable.
    pub excel_formula: Option<String>,
    /// `true` if this calculation subtree was already expanded earlier in traversal.
    #[serde(default)]
    pub is_pruned: bool,
    /// Nested child calculation nodes.
    pub children: Vec<DependencyNode>,
}
```

#### Traversal Algorithm

```rust
fn build_precedent_tree_node(
    &self,
    cell_id: &CellId,
    visited_ancestors: &mut HashSet<CellId>,
    expanded: &mut HashSet<CellId>,
    all_visited: &mut HashSet<CellId>,
) -> DependencyNode {
    all_visited.insert(cell_id.clone());
    let (sheet_address, value, excel_formula) = self.get_node_display_metadata(cell_id);

    // If already expanded in another branch or cycle detected, prune
    if visited_ancestors.contains(cell_id) || expanded.contains(cell_id) {
        return DependencyNode {
            cell_id: cell_id.clone(),
            sheet_address,
            value,
            excel_formula,
            is_pruned: true,
            children: Vec::new(),
        };
    }

    visited_ancestors.insert(cell_id.clone());
    expanded.insert(cell_id.clone());

    let deps = self.graph.dependencies_of(cell_id);
    let mut children = Vec::with_capacity(deps.len());
    for dep in deps {
        children.push(self.build_precedent_tree_node(
            dep,
            visited_ancestors,
            expanded,
            all_visited,
        ));
    }

    visited_ancestors.remove(cell_id);

    DependencyNode {
        cell_id: cell_id.clone(),
        sheet_address,
        value,
        excel_formula,
        is_pruned: false,
        children,
    }
}
```

### Risks & Trade-offs

- **Serialization backward compatibility**: Adding `#[serde(default)] pub is_pruned: bool` ensures existing serialized JSON fixtures remain valid without schema breakage.
- **`libc` dependency**: Scoped exclusively to `[target.'cfg(unix)'.dependencies]`. It is already present in the workspace dependency tree via transitive dependencies.
- **Tree ordering**: Depth-first traversal expands the first branch that encounters a shared node in full, while subsequent branches display it with `(already visited)`. This is deterministic and matches standard DAG-to-tree rendering in compiler and package managers (e.g. `cargo tree`).

### Acceptance Criteria

- [ ] **Scenario 1: Recursive precedent traversal on LBO model**
  - **Given** the compiled `lbo` archetype project
  - **When** executing `f8 inspect precedents ReturnsSummary irr val --recursive`
  - **Then** output completes in < 100ms, contains < 250 total lines, and reports `150 calculations`.

- [ ] **Scenario 2: Pruned DAG node notation in terminal render**
  - **Given** a cell whose dependency is reached via multiple paths
  - **When** rendering the tree in text mode
  - **Then** the first path renders full children, and subsequent paths render `├── <CellId> (already visited)` with no subtrees.

- [ ] **Scenario 3: Machine-readable JSON pruning metadata**
  - **Given** a recursive tree query with `--json`
  - **When** parsing the JSON output
  - **Then** duplicate nodes have `"is_pruned": true` and `"children": []`.

- [ ] **Scenario 4: Downstream blast-radius dependent traversal pruning**
  - **Given** an upstream driver cell (e.g. `Assumptions rev_growth val`) with reconvergent downstream dependents
  - **When** executing `f8 inspect dependents Assumptions rev_growth val --recursive`
  - **Then** downstream calculations are expanded once, reconvergent dependents are marked pruned, and output line count is bounded linearly by graph edges.

- [ ] **Scenario 5: Unix pipeline stream truncation (SIGPIPE)**
  - **Given** any `f8 inspect` or `f8 check` command generating multiline output
  - **When** piped into `head -n 5` on a Unix system
  - **Then** the command exits cleanly with exit code 0 or 141 without printing any panic trace or `Broken pipe` error to `stderr`.

- [ ] **Scenario 6: Cross-platform broken pipe panic handler**
  - **Given** an environment where `println!` encounters `io::ErrorKind::BrokenPipe`
  - **When** writing output to closed stdout
  - **Then** the panic hook catches the error and exits process cleanly with exit code 0.

---

## Tasks

### Phase 1: Core DAG Traversal & Pruning (`f8-core`)

- [ ] Add `#[serde(default)] pub is_pruned: bool` to `DependencyNode` in [`crates/f8-core/src/compiler/compiled/model.rs`](../../crates/f8-core/src/compiler/compiled/model.rs).
- [ ] Update `build_precedent_tree_node` in [`crates/f8-core/src/compiler/compiled/mod.rs`](../../crates/f8-core/src/compiler/compiled/mod.rs) to accept `expanded: &mut HashSet<CellId>` and prune duplicate subtrees.
- [ ] Update `build_dependent_tree_node` in [`crates/f8-core/src/compiler/compiled/mod.rs`](../../crates/f8-core/src/compiler/compiled/mod.rs) to accept `expanded: &mut HashSet<CellId>` and prune duplicate subtrees.
- [ ] Add unit tests in `crates/f8-core/tests/` asserting that diamond DAGs (A -> B, A -> C, B -> D, C -> D) prune duplicate expansion of D.

### Phase 2: CLI Tree Renderer Pruning Annotation (`f8-cli`)

- [ ] Update `render_node_list` in [`crates/f8-cli/src/render/tree.rs`](../../crates/f8-cli/src/render/tree.rs) to append ` (already visited)` when `node.is_pruned` is true.
- [ ] Verify that `render_node_list` does not render recursive children for pruned nodes.

### Phase 3: SIGPIPE and Broken Pipe Handling (`f8-cli`)

- [ ] Add `libc` dependency under `[target.'cfg(unix)'.dependencies]` in [`Cargo.toml`](../../Cargo.toml) and [`crates/f8-cli/Cargo.toml`](../../crates/f8-cli/Cargo.toml).
- [ ] Reset `SIGPIPE` handler to `SIG_DFL` in [`crates/f8-cli/src/main.rs`](../../crates/f8-cli/src/main.rs) on `#[cfg(unix)]`.
- [ ] Install broken pipe panic hook in [`crates/f8-cli/src/lib.rs`](../../crates/f8-cli/src/lib.rs) ensuring clean exit on broken pipe.

### Phase 4: Integration Verification Suite

- [ ] Add integration test in `crates/f8-cli/tests/` verifying `f8 inspect precedents ReturnsSummary irr val -p crates/f8-cli/templates/lbo --recursive` outputs < 250 lines in < 100ms.
- [ ] Add integration test in `crates/f8-cli/tests/` verifying piping output to `head` exits without broken pipe panic.
- [ ] Run verification suite:
  - `cargo check`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`
