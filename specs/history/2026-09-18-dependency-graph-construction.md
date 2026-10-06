---
title: Dependency Graph Construction & Cycle Detection
date: 2026-09-18
status: completed
target-living-spec: specs/living-specs/compiler/dependency-graph.md
---

## Proposal

### What

Implement **Dependency Graph Construction, Cycle Detection, and Topological Sorting** (`finx::compiler::graph::dependency`), completing Pass 3 of the compiler pipeline. This pass ingests the resolved dependency mappings (`IndexMap<CellId, IndexSet<CellId>>`) from Pass 3A and the expanded `CellGrid`, constructs a directed cell dependency graph using `petgraph`, enforces FAST Standard Section 2.01 (zero circularity) via Tarjan's Strongly Connected Components (SCC) algorithm, extracts actionable semantic cycle traces upon circularity, and computes deterministic topological evaluation order.

### Why

1. **Complete Pass 3**: Reference resolution (Pass 3A) converts dynamic coordinate selectors to discrete cell-to-cell dependency sets. Graph construction (Pass 3B) structures these relationships into a verified DAG required for topological evaluation (Pass 4) and spatial layout planning (Pass 5).
2. **Strict FAST Standard compliance**: Circular references produce erratic recalculation loops and `#DIV/0!` locking in spreadsheets. Financial models in Finx must be verified DAGs with iterative calculation explicitly prohibited (`calcPr.iterate = false`).
3. **Actionable diagnostic firewall**: When circular logic is introduced, financial modelers and AI agents need an exact, step-by-step semantic cycle trace (`CellA -> CellB -> CellC -> CellA`) rather than a generic boolean error or stack overflow.
4. **Deterministic evaluation order**: In-memory evaluation and sheet lowering require a stable, reproducible execution sequence respecting dependency precedence and declaration order.

### Scope

- **In-scope**:
  - Add `petgraph = "0.8"` dependency to `Cargo.toml`.
  - Introduce `GraphError` in `src/compiler/graph/error.rs`, incorporating `ResolveError` and `CircularDependency { cycle: Vec<CellId>, trace: String }`.
  - Define `CycleHandler` trait and `StrictDagHandler` implementation in `src/compiler/graph/cycle.rs` (or `dependency.rs`).
  - Implement `DependencyGraph` in `src/compiler/graph/dependency.rs`:
    - Directed graph representation (`petgraph::graph::DiGraph<CellId, ()>`).
    - Bidirectional coordinate indexing (`IndexMap<CellId, NodeIndex>`).
    - Directed edge convention: $A \to B$ ($A$ is an input to $B$; $B$ depends on $A$).
    - Tarjan SCC analysis with self-loop detection.
    - Cycle path reconstruction via DFS to generate ordered semantic cycle traces.
    - Deterministic topological sorting.
    - Public query APIs: `dependencies_of(&CellId)` and `dependents_of(&CellId)`.
    - High-level builder: `DependencyGraph::from_grid(&CellGrid, &SymbolTable)`.
  - Re-export `DependencyGraph`, `GraphError`, `CycleHandler`, and `StrictDagHandler` in `src/compiler/graph/mod.rs`.
  - Integration tests in `tests/dependency_graph_test.rs` covering acyclic DAGs, direct/indirect cycles, self-loops, and end-to-end pipeline execution from Spec AST $\to$ `CellGrid` $\to$ `DependencyGraph`.
- **Out-of-scope**:
  - In-memory formula calculation and function execution (Pass 4: `finx::compiler::evaluator`).
  - Physical coordinate lowering ($A$1) and Excel workbook emission (Pass 5 & 6).

### What Changes

- `Cargo.toml`: Add `petgraph = "0.8"`.
- `src/compiler/graph/mod.rs`: Expose `dependency`, `cycle`, and re-export `DependencyGraph`, `GraphError`, `StrictDagHandler`.
- `src/compiler/graph/error.rs`: Introduce `GraphError` enum with structured cycle trace formatting.
- `src/compiler/graph/cycle.rs`: Declare `CycleHandler` trait and `StrictDagHandler`.
- `src/compiler/graph/dependency.rs`: Implement `DependencyGraph`, edge insertion, cycle path reconstruction, and topological sorting.
- `tests/dependency_graph_test.rs`: Exhaustive test suite for DAG validation, cycle detection, and pipeline integration.

### Capabilities

- **New capabilities**:
  - Constructing cell-level directed dependency graphs from compiled cell grids.
  - Detecting circular dependencies (direct $A \leftrightarrow B$, multi-step $A \to B \to C \to A$, and self-referential $A \to A$) using Tarjan's SCC algorithm.
  - Formatting human-readable, numbered semantic cycle traces indicating the closed loop.
  - Emitting deterministic topological evaluation orders for downstream formula evaluation.
  - Querying incoming predecessors (dependencies) and outgoing successors (dependents) for any cell in the workbook.
  - One-stop compiler pass execution via `DependencyGraph::from_grid(&grid, &symbol_table)`.
- **Modified capabilities**: None (greenfield sub-pass).

---

## Design

### Goals

1. **Deterministic order**: Identical models must produce identical topological sequences across runs and platforms.
2. **Zero false negatives on circularity**: Every cycle (including self-loops) must be caught before evaluation begins.
3. **Actionable diagnostics**: Cycle errors must output an exact trace showing the cell chain from origin back to itself.
4. **Clean boundary**: Decouple cycle detection and graph querying from formula computation and AST evaluation.

### Invariants

1. **Pure DAG compliance**: In V1, compilation unconditionally halts with `GraphError::CircularDependency` if any cycle is detected.
2. **Edge direction**: A directed edge $A \to B$ strictly denotes "$A$ is evaluated before $B$" ($B$ depends on $A$).
3. **Completeness**: Every cell present in the `CellGrid` (whether literal, formula, or empty) exists as a node in the graph.
4. **Node uniqueness**: Every `CellId` corresponds to exactly one node index in the graph.

### Decisions

- **Decision 1: Edge convention from dependency to dependent ($A \to B$)**:
  - *Choice*: Insert directed edge $A \to B$ when cell $B$ references cell $A$.
  - *Rationale*: Directly corresponds to data flow and evaluation precedence. A standard topological sort over the graph yields the exact sequence in which cells must be computed.
  - *Alternatives considered*: Edge $B \to A$ (from dependent to dependency). Rejected because it would require reversing the topological sort output or transposing the graph, adding mental overhead and departing from `docs/compiler/dependency-graph.md`.
- **Decision 2: Graph representation using `DiGraph<CellId, ()>` and `IndexMap<CellId, NodeIndex>`**:
  - *Choice*: Use `petgraph::graph::DiGraph` indexed by a bidirectional `IndexMap<CellId, NodeIndex>`.
  - *Rationale*: `CellId` contains dynamic string keys (`TableId`, `RowId`, `ColId`) and does not implement `Copy` (required by `DiGraphMap`). `IndexMap` guarantees $O(1)$ coordinate lookup and preserves deterministic insertion order from the grid.
  - *Alternatives considered*: `petgraph::graphmap::DiGraphMap` with custom integer IDs. Rejected as premature optimization that adds an unnecessary ID translation layer.
- **Decision 3: Tarjan SCC with self-loop check and DFS cycle path reconstruction**:
  - *Choice*: Run `petgraph::algo::tarjan_scc`. Identify cyclic components where $\text{len} > 1$ or ($\text{len} == 1$ and node has an edge to itself). Reconstruct the exact cycle path using depth-first search within the component.
  - *Rationale*: Tarjan's SCC isolates all nodes in a strongly connected component in $O(V + E)$ time. Crucially, `petgraph`'s `tarjan_scc` outputs components in **postorder (reverse topological order)**, and node IDs *within* each SCC vector are in **arbitrary stack order**, not in cycle traversal sequence. Reconstructing the ordered closed loop via DFS within the SCC is therefore strictly required to produce the exact numbered semantic trace specified in `docs/compiler/dependency-graph.md`.
  - *Alternatives considered*: Dumping raw SCC vector nodes directly. Rejected because node ordering within a Tarjan SCC component does not reflect directed cycle order and would produce jumbled, misleading diagnostics. Simple `petgraph::algo::is_cyclic_directed` was also rejected because it only returns a boolean without identifying the participating nodes or the cyclic path.
- **Decision 4: Stable topological order tie-broken by declaration index**:
  - *Choice*: Topologically sort nodes using `petgraph::algo::toposort` (or Kahn's algorithm) with declaration-order tie-breaking.
  - *Rationale*: Because `petgraph::algo::tarjan_scc` returns components in reverse topological order (sinks first), using `petgraph::algo::toposort` or reversing the condensation order yields the true dependency execution sequence ($A$ before $B$). Independent cells (e.g., literal assumption columns) must evaluate in consistent, predictable chronological and declaration order.
- **Decision 5: Pluggable `CycleHandler` trait**:
  - *Choice*: Introduce `CycleHandler` trait with `StrictDagHandler` as the default.
  - *Rationale*: Preserves future extensibility for iterative solvers without breaking the V1 FAST Standard invariant.

### Data Contracts

#### Error Definitions (`src/compiler/graph/error.rs`)

```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GraphError {
    #[error(transparent)]
    Resolve(#[from] ResolveError),

    #[error("Circular dependency detected in model.\nCycle trace:\n{trace}")]
    CircularDependency {
        cycle: Vec<CellId>,
        trace: String,
    },
}
```

#### Cycle Handler Trait (`src/compiler/graph/cycle.rs`)

```rust
pub trait CycleHandler {
    fn handle_cycles(&self, cycle: &[CellId]) -> Result<(), GraphError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StrictDagHandler;

impl CycleHandler for StrictDagHandler {
    fn handle_cycles(&self, cycle: &[CellId]) -> Result<(), GraphError> {
        let trace = format_cycle_trace(cycle);
        Err(GraphError::CircularDependency {
            cycle: cycle.to_vec(),
            trace,
        })
    }
}
```

#### Dependency Graph API (`src/compiler/graph/dependency.rs`)

```rust
pub struct DependencyGraph {
    graph: petgraph::graph::DiGraph<CellId, ()>,
    node_indices: IndexMap<CellId, NodeIndex>,
    topological_order: Vec<CellId>,
}

impl DependencyGraph {
    /// Builds and validates a dependency graph directly from a compiled CellGrid and SymbolTable.
    pub fn from_grid(grid: &CellGrid, symbol_table: &SymbolTable) -> Result<Self, GraphError>;

    /// Builds a dependency graph from a CellGrid and pre-resolved dependency mappings.
    pub fn from_dependencies(
        grid: &CellGrid,
        dependencies: &IndexMap<CellId, IndexSet<CellId>>,
    ) -> Result<Self, GraphError>;

    /// Builds a dependency graph using a custom CycleHandler.
    pub fn from_dependencies_with_handler<H: CycleHandler>(
        grid: &CellGrid,
        dependencies: &IndexMap<CellId, IndexSet<CellId>>,
        handler: &H,
    ) -> Result<Self, GraphError>;

    /// Returns the verified topological evaluation order.
    pub fn topological_order(&self) -> &[CellId];

    /// Returns incoming dependencies that must be computed before the given cell.
    pub fn dependencies_of(&self, cell: &CellId) -> Vec<&CellId>;

    /// Returns outgoing dependents that depend on the calculation of the given cell.
    pub fn dependents_of(&self, cell: &CellId) -> Vec<&CellId>;

    /// Returns the total number of cell nodes in the graph.
    pub fn node_count(&self) -> usize;

    /// Returns the total number of dependency edges in the graph.
    pub fn edge_count(&self) -> usize;
}
```

### Risks & Trade-offs

| Risk | Mitigation |
| :--- | :--------- |
| Non-deterministic topological sort order | Seed node insertion and sorting strictly from `CellGrid` declaration order. |
| Large models with deep recursion in cycle DFS | Tarjan's SCC restricts DFS search scope strictly to the small sub-graph of participating cyclic nodes. |
| Self-referential cells ($A \to A$) missed by basic SCC size checks | Explicitly check for self-loops (`graph.contains_edge(node, node)`) when SCC size is 1. |

---

## Acceptance Criteria (Given / When / Then)

- [x] **Scenario 1: Acyclic model DAG construction & topological sort**
  - **Given** an acyclic cell grid where cell $C$ depends on $B$, and $B$ depends on $A$
  - **When** `DependencyGraph::from_grid` is called
  - **Then** graph construction succeeds, and `topological_order` returns `[A, B, C]`.

- [x] **Scenario 2: Direct 2-cell circular dependency rejected with trace**
  - **Given** cell $A$ with formula referencing $B$, and cell $B$ with formula referencing $A$
  - **When** `DependencyGraph::from_grid` is called
  - **Then** compilation fails with `GraphError::CircularDependency` whose trace contains $A \to B \to A$.

- [x] **Scenario 3: Single-cell self-reference cycle rejected**
  - **Given** cell $A$ with formula referencing $A$ ($A \to A$)
  - **When** `DependencyGraph::from_grid` is called
  - **Then** compilation fails with `GraphError::CircularDependency` whose trace contains $A \to A$.

- [x] **Scenario 4: Multi-step indirect cycle detected with formatted trace**
  - **Given** a 3-table circular loop ($A \to B \to C \to A$)
  - **When** `DependencyGraph::from_grid` is called
  - **Then** compilation fails with `GraphError::CircularDependency` whose formatted trace displays:
    ```text
    1. TableA["r", "c"]
    2. TableB["r", "c"]
    3. TableC["r", "c"]
    4. TableA["r", "c"]
    ```

- [x] **Scenario 5: Querying dependencies and dependents**
  - **Given** a cell $B$ referencing inputs $A_1$ and $A_2$, and referenced by downstream cell $C$
  - **When** querying `dependencies_of(B)` and `dependents_of(B)`
  - **Then** `dependencies_of(B)` returns `[&A1, &A2]` and `dependents_of(B)` returns `[&C]`.

- [x] **Scenario 6: Disconnected / isolated literal cells preserved**
  - **Given** a grid with 5 independent literal cells and 2 formula cells
  - **When** `DependencyGraph::from_grid` is called
  - **Then** `node_count()` equals 7, and all 7 cells appear in `topological_order`.

- [x] **Scenario 7: Full pipeline integration (Spec AST $\to$ Grid Expansion $\to$ Dependency Graph)**
  - **Given** a complete multi-table `WorkbookSpec` with assumptions and calculation schedules
  - **When** passing the model through `expand_workbook` and `DependencyGraph::from_grid`
  - **Then** the entire pipeline executes successfully with zero errors and valid topological order.

---

## Tasks

### Phase 1: Dependencies & Error Diagnostics
- [x] Add `petgraph = "0.8"` to `Cargo.toml`.
- [x] Define `GraphError` in `src/compiler/graph/error.rs` wrapping `ResolveError` and introducing `CircularDependency`.
- [x] Implement `format_cycle_trace(cycle: &[CellId]) -> String` helper.

### Phase 2: Cycle Handler & Graph Construction
- [x] Define `CycleHandler` trait and `StrictDagHandler` in `src/compiler/graph/cycle.rs`.
- [x] Implement `DependencyGraph` struct and builder methods in `src/compiler/graph/dependency.rs`:
  - Node population from `CellGrid`.
  - Directed edge insertion from `IndexMap<CellId, IndexSet<CellId>>`.
  - Tarjan SCC execution and self-loop verification.
  - DFS cycle path reconstruction for cyclic SCCs.
  - Deterministic topological sorting.
  - Query methods: `dependencies_of`, `dependents_of`, `topological_order`, `node_count`, `edge_count`.
- [x] Update `src/compiler/graph/mod.rs` to expose `cycle`, `dependency`, and re-export public types.

### Phase 3: Tests & Verification Suite
- [x] Implement unit and integration tests in `tests/dependency_graph_test.rs` covering all 7 acceptance criteria scenarios.
- [x] Run full verification suite:
  - `cargo check`
  - `cargo test`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo fmt --check`

### Phase 4: Reconcile Living Spec & Archive Draft
- [x] Reconcile `specs/living-specs/compiler/dependency-graph.md` with graph construction contracts, cycle handling, and topological sort guarantees.
- [x] Move `specs/drafts/2026-09-18-dependency-graph-construction.md` to `specs/history/`.
