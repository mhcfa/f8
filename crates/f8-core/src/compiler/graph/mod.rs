//! Pass 3: Dependency graph construction and Tarjan SCC cycle detection.
//!
//! Enforces FAST Standard DAG compliance and provides cycle traces as documented
//! in `docs/compiler/dependency-graph.md`.

pub mod cycle;
pub mod dependency;
pub mod error;
pub mod resolve;

pub use cycle::{CycleHandler, StrictDagHandler};
pub use dependency::DependencyGraph;
pub use error::{GraphError, ResolveError, format_cycle_trace};
pub use resolve::{
    ResolvedSlice, collect_references, resolve_cell_dependencies, resolve_table_ref,
};
