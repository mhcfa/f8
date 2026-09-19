//! Pass 4: In-memory topological evaluator and evaluated matrix.
//!
//! Executes formula expressions with Excel parity and topological execution
//! over the cell dependency DAG as documented in `docs/compiler/evaluator.md`
//! and `docs/compiler/parity.md`.

pub mod context;
pub mod error;
pub mod expr;
pub mod model;

pub use context::CellEvalContext;
pub use error::EvalError;
pub use expr::eval_expr;
pub use model::EvaluatedModel;

use indexmap::IndexMap;

use crate::compiler::graph::DependencyGraph;
use crate::compiler::ir::cell::{CellGrid, CellPayload};
use crate::compiler::ir::symbol::SymbolTable;
use crate::functions::registry::FunctionRegistry;
use crate::value::CellValue;

/// In-memory topological evaluator executing cells across compiled model DAGs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Evaluator;

impl Evaluator {
    /// Evaluates all cells in the cell grid according to the dependency graph's topological calculation order.
    ///
    /// Literals are converted directly to [`CellValue`], while formulas are evaluated via [`eval_expr`]
    /// using [`CellEvalContext`] against accumulated predecessor values.
    ///
    /// # Errors
    /// Returns [`EvalError`] if a referenced dependency is missing from the grid, reference
    /// resolution fails, or a formula expression encounters a compiler-level evaluation error.
    pub fn evaluate(
        grid: &CellGrid,
        graph: &DependencyGraph,
        symbol_table: &SymbolTable,
        registry: &FunctionRegistry,
    ) -> Result<EvaluatedModel, EvalError> {
        let mut values = IndexMap::with_capacity(graph.node_count());

        for cell_id in graph.topological_order() {
            let cell_node = grid
                .get(cell_id)
                .ok_or_else(|| EvalError::cell_not_found(cell_id.clone()))?;

            let value = match &cell_node.payload {
                CellPayload::Literal(lit) => CellValue::from(lit),
                CellPayload::Formula(expr) => {
                    eval_expr(expr, cell_id, &values, symbol_table, registry)?
                }
            };

            values.insert(cell_id.clone(), value);
        }

        Ok(EvaluatedModel::new(values, symbol_table.clone()))
    }
}
