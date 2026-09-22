use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

use indexmap::{IndexMap, IndexSet};
use petgraph::graph::{DiGraph, NodeIndex};

use crate::compiler::ir::cell::CellGrid;
use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::SymbolTable;

use super::cycle::{CycleHandler, StrictDagHandler};
use super::error::GraphError;
use super::resolve::resolve_cell_dependencies;

/// Directed cell dependency graph enforcing FAST Standard acyclicity.
///
/// Nodes represent discrete cell coordinates across tables, and directed edges
/// `A -> B` signify that cell `B` references and depends on cell `A` (i.e. `A` must
/// be evaluated before `B`).
#[derive(Debug, Clone)]
pub struct DependencyGraph {
    graph: DiGraph<CellId, ()>,
    node_indices: IndexMap<CellId, NodeIndex>,
    topological_order: Vec<CellId>,
}

impl DependencyGraph {
    /// Builds and validates a dependency graph directly from a compiled `CellGrid` and `SymbolTable`.
    ///
    /// Automatically performs formula reference resolution (Pass 3A), constructs the
    /// directed graph, runs Tarjan's SCC cycle detection, and computes the topological
    /// calculation order.
    pub fn from_grid(grid: &CellGrid, symbol_table: &SymbolTable) -> Result<Self, GraphError> {
        let dependencies = resolve_cell_dependencies(grid, symbol_table)?;
        Self::from_dependencies(grid, &dependencies)
    }

    /// Builds and validates a dependency graph from a `CellGrid` and pre-resolved dependency mappings.
    ///
    /// Uses the default `StrictDagHandler` to enforce zero circularity.
    pub fn from_dependencies(
        grid: &CellGrid,
        dependencies: &IndexMap<CellId, IndexSet<CellId>>,
    ) -> Result<Self, GraphError> {
        let handler = StrictDagHandler;
        Self::from_dependencies_with_handler(grid, dependencies, &handler)
    }

    /// Builds a dependency graph from a `CellGrid` and pre-resolved dependency mappings using a custom `CycleHandler`.
    pub fn from_dependencies_with_handler<H: CycleHandler>(
        grid: &CellGrid,
        dependencies: &IndexMap<CellId, IndexSet<CellId>>,
        handler: &H,
    ) -> Result<Self, GraphError> {
        let mut graph = DiGraph::<CellId, ()>::new();
        let mut node_indices = IndexMap::with_capacity(grid.len());

        // 1. Populate nodes from CellGrid in declaration order
        for cell in grid.values() {
            let idx = graph.add_node(cell.id.clone());
            node_indices.insert(cell.id.clone(), idx);
        }

        // 2. Insert directed edges (A -> B: A is dependency of B, B depends on A)
        for (dependent, deps) in dependencies {
            let Some(&target_idx) = node_indices.get(dependent) else {
                continue;
            };

            for dep in deps {
                if let Some(&source_idx) = node_indices.get(dep)
                    && !graph.contains_edge(source_idx, target_idx)
                {
                    graph.add_edge(source_idx, target_idx, ());
                }
            }
        }

        // 3. Cycle detection using Tarjan's SCC algorithm
        let sccs = petgraph::algo::tarjan_scc(&graph);
        for scc in &sccs {
            let is_cycle = scc.len() > 1 || (scc.len() == 1 && graph.contains_edge(scc[0], scc[0]));
            if is_cycle && let Some(cycle_path) = find_cycle_in_scc(&graph, scc) {
                handler.handle_cycles(&cycle_path)?;
            }
        }

        // 4. Deterministic topological sorting via Kahn's algorithm with declaration-order tie-breaking
        let topological_order = compute_stable_toposort(&graph)?;

        Ok(Self {
            graph,
            node_indices,
            topological_order,
        })
    }

    /// Returns the verified topological evaluation order of all cells in the model.
    pub fn topological_order(&self) -> &[CellId] {
        &self.topological_order
    }

    /// Returns the incoming dependencies of the given cell (cells that must be computed before it).
    pub fn dependencies_of(&self, cell: &CellId) -> Vec<&CellId> {
        let Some(&idx) = self.node_indices.get(cell) else {
            return Vec::new();
        };

        self.graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .map(|n| &self.graph[n])
            .collect()
    }

    /// Returns the outgoing dependents of the given cell (cells that depend on its value).
    pub fn dependents_of(&self, cell: &CellId) -> Vec<&CellId> {
        let Some(&idx) = self.node_indices.get(cell) else {
            return Vec::new();
        };

        self.graph
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .map(|n| &self.graph[n])
            .collect()
    }

    /// Returns the total number of cell nodes registered in the graph.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Returns the total number of dependency edges in the graph.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Returns a reference to the underlying `petgraph::graph::DiGraph`.
    pub fn inner(&self) -> &DiGraph<CellId, ()> {
        &self.graph
    }
}

/// Reconstructs an ordered simple cycle loop from nodes in an SCC.
fn find_cycle_in_scc(graph: &DiGraph<CellId, ()>, scc: &[NodeIndex]) -> Option<Vec<CellId>> {
    if scc.is_empty() {
        return None;
    }

    // Handle single-node self-loop
    if scc.len() == 1 {
        let node = scc[0];
        if graph.contains_edge(node, node) {
            let cell = graph[node].clone();
            return Some(vec![cell.clone(), cell]);
        }
        return None;
    }

    let scc_set: HashSet<NodeIndex> = scc.iter().copied().collect();

    // Sort SCC nodes deterministically by CellId for predictable cycle starting point
    let mut sorted_scc = scc.to_vec();
    sorted_scc.sort_by(|&a, &b| graph[a].cmp(&graph[b]));

    for &start in &sorted_scc {
        let mut visited = HashSet::new();
        let mut on_stack = HashSet::new();
        let mut path = Vec::new();

        if let Some(cycle) = dfs_cycle(
            graph,
            start,
            &scc_set,
            &mut visited,
            &mut on_stack,
            &mut path,
        ) {
            return Some(cycle.iter().map(|&n| graph[n].clone()).collect());
        }
    }

    None
}

/// Recursive DFS helper for cycle path extraction within an SCC.
fn dfs_cycle(
    graph: &DiGraph<CellId, ()>,
    u: NodeIndex,
    scc_set: &HashSet<NodeIndex>,
    visited: &mut HashSet<NodeIndex>,
    on_stack: &mut HashSet<NodeIndex>,
    path: &mut Vec<NodeIndex>,
) -> Option<Vec<NodeIndex>> {
    visited.insert(u);
    on_stack.insert(u);
    path.push(u);

    // Collect and sort outgoing neighbors within the SCC for determinism
    let mut neighbors: Vec<NodeIndex> = graph
        .neighbors_directed(u, petgraph::Direction::Outgoing)
        .filter(|v| scc_set.contains(v))
        .collect();
    neighbors.sort_by(|&a, &b| graph[a].cmp(&graph[b]));

    for v in neighbors {
        if on_stack.contains(&v) {
            if let Some(pos) = path.iter().position(|&x| x == v) {
                let mut cycle = path[pos..].to_vec();
                cycle.push(v);
                return Some(cycle);
            }
        } else if !visited.contains(&v)
            && let Some(cycle) = dfs_cycle(graph, v, scc_set, visited, on_stack, path)
        {
            return Some(cycle);
        }
    }

    path.pop();
    on_stack.remove(&u);
    None
}

/// Computes a stable topological sort using Kahn's algorithm with declaration-order tie-breaking.
fn compute_stable_toposort(graph: &DiGraph<CellId, ()>) -> Result<Vec<CellId>, GraphError> {
    let node_count = graph.node_count();
    let mut in_degrees = vec![0usize; node_count];

    for idx in graph.node_indices() {
        in_degrees[idx.index()] = graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .count();
    }

    // Min-heap ordered by declaration NodeIndex ensures deterministic, stable evaluation order
    let mut ready = BinaryHeap::new();
    for idx in graph.node_indices() {
        if in_degrees[idx.index()] == 0 {
            ready.push(Reverse(idx.index()));
        }
    }

    let mut order = Vec::with_capacity(node_count);

    while let Some(Reverse(idx_raw)) = ready.pop() {
        let idx = NodeIndex::new(idx_raw);
        order.push(graph[idx].clone());

        // Sort neighbors deterministically by declaration NodeIndex
        let mut neighbors: Vec<NodeIndex> = graph
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .collect();
        neighbors.sort_by_key(|n| n.index());

        for neighbor in neighbors {
            let n_raw = neighbor.index();
            in_degrees[n_raw] -= 1;
            if in_degrees[n_raw] == 0 {
                ready.push(Reverse(n_raw));
            }
        }
    }

    if order.len() != node_count {
        // Fallback: If for any reason Kahn's algorithm did not visit all nodes (due to cycle that escaped SCC check)
        return Err(GraphError::circular_dependency(Vec::new()));
    }

    Ok(order)
}
