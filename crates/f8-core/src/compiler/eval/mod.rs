//! Pass 4: In-memory topological evaluator and evaluated matrix.
//!
//! Executes formula expressions with Excel parity and topological execution
//! over the cell dependency DAG.

mod context;
pub mod error;
pub mod expr;
pub mod model;

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
    /// Statically validates all formula function calls across the cell grid in declaration order.
    ///
    /// # Errors
    /// Returns [`EvalError::Function`] on the first cell containing an unknown function,
    /// arity violation, invalid variadic pair count, or invalid omitted argument (`_`).
    pub fn validate_grid(grid: &CellGrid, registry: &FunctionRegistry) -> Result<(), EvalError> {
        for (cell_id, cell_node) in grid.iter() {
            match &cell_node.payload {
                CellPayload::Literal(_) => {}
                CellPayload::Formula(expr) => {
                    registry
                        .validate_expr(expr)
                        .map_err(|source| EvalError::function(cell_id.clone(), source))?;
                }
            }
        }
        Ok(())
    }

    /// Evaluates all cells in the cell grid according to the dependency graph's topological calculation order.
    ///
    /// Literals are converted directly to [`CellValue`], while formulas are evaluated via [`eval_expr`]
    /// using `CellEvalContext` against accumulated predecessor values.
    ///
    /// # Errors
    /// Returns [`EvalError`] if static function call validation fails, a referenced dependency is missing
    /// from the grid, reference resolution fails, or a formula expression encounters a compiler-level evaluation error.
    pub fn evaluate(
        grid: &CellGrid,
        graph: &DependencyGraph,
        symbol_table: &SymbolTable,
        registry: &FunctionRegistry,
    ) -> Result<EvaluatedModel, EvalError> {
        Self::validate_grid(grid, registry)?;

        let mut values = IndexMap::with_capacity(graph.node_count());

        for cell_id in graph.topological_order() {
            let cell_node = grid
                .get(cell_id)
                .ok_or_else(|| EvalError::cell_not_found(cell_id.clone()))?;

            let value = match &cell_node.payload {
                CellPayload::Literal(lit) => CellValue::from(lit),
                CellPayload::Formula(expr) => {
                    expr::eval_expr_validated(expr, cell_id, &values, symbol_table, registry)?
                }
            };

            values.insert(cell_id.clone(), value);
        }

        Ok(EvaluatedModel::new(values, symbol_table.clone()))
    }
}
