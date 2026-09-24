use crate::compiler::ir::id::CellId;

use super::error::GraphError;

/// Pluggable handler for circular dependencies detected during graph construction.
pub trait CycleHandler {
    /// Handles a detected circular dependency cycle path.
    fn handle_cycles(&self, cycle: &[CellId]) -> Result<(), GraphError>;
}

/// Default strict FAST Standard (Section 2.01) zero-circularity handler.
///
/// Always halts compilation and emits an actionable `GraphError::CircularDependency`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StrictDagHandler;

impl CycleHandler for StrictDagHandler {
    fn handle_cycles(&self, cycle: &[CellId]) -> Result<(), GraphError> {
        Err(GraphError::circular_dependency(cycle.to_vec()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strict_dag_handler_emits_cycle_error() {
        let handler = StrictDagHandler;
        let c1 = CellId::new("T", "r1", "c1");
        let c2 = CellId::new("T", "r2", "c2");
        let cycle = vec![c1.clone(), c2.clone(), c1.clone()];

        let result = handler.handle_cycles(&cycle);
        match result {
            Err(GraphError::CircularDependency {
                cycle: err_cycle,
                trace,
            }) => {
                assert_eq!(err_cycle, cycle);
                assert!(trace.contains("1. T[\"r1\", \"c1\"]"));
                assert!(trace.contains("2. T[\"r2\", \"c2\"]"));
                assert!(trace.contains("3. T[\"r1\", \"c1\"]"));
            }
            _ => panic!("expected CircularDependency error"),
        }
    }
}
