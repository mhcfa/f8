//! Pass 3: Dependency graph construction and Tarjan SCC cycle detection.
//!
//! Enforces FAST Standard DAG compliance and provides cycle traces as documented
//! in `docs/compiler/dependency-graph.md`.

pub mod error;
pub mod resolve;

pub use error::ResolveError;
pub use resolve::{collect_references, resolve_cell_dependencies, resolve_table_ref};
