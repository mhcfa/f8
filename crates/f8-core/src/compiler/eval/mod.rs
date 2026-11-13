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

use self::context::CellEvalContext;
use crate::compiler::graph::{DependencyGraph, resolve_table_ref};
use crate::compiler::ir::cell::{CellGrid, CellPayload};
use crate::compiler::ir::symbol::SymbolTable;
use crate::functions::registry::{FunctionRegistry, StageAError};
use crate::functions::traits::EvalContext;
use crate::value::{CellValue, TypeError, ValueType};

/// In-memory topological evaluator executing cells across compiled model DAGs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Evaluator;

impl Evaluator {
    /// Statically validates all formula cells across the cell grid in declaration order (Stage A).
    ///
    /// # Errors
    /// Returns [`EvalError`] on the first cell containing an unknown function, signature violation,
    /// static type or integer violation, scalar slice violation, or slice rank/dimension/index error.
    pub fn validate_grid(
        grid: &CellGrid,
        symbol_table: &SymbolTable,
        registry: &FunctionRegistry,
    ) -> Result<(), EvalError> {
        for (cell_id, cell_node) in grid.iter() {
            match &cell_node.payload {
                CellPayload::Literal(_) => {}
                CellPayload::Formula(expr) => {
                    registry
                        .validate_expr_with_shapes(expr, |table_ref| {
                            resolve_table_ref(cell_id, table_ref, symbol_table)
                                .map(|slice| Some((slice.rows, slice.cols, slice.axis_role)))
                        })
                        .map_err(|err| match err {
                            StageAError::Function(source) => {
                                EvalError::function(cell_id.clone(), source)
                            }
                            StageAError::Resolve(source) => EvalError::Resolve(source),
                        })?;
                }
            }
        }
        Ok(())
    }

    /// Evaluates all cells in the grid in topological dependency order.
    ///
    /// - `CellPayload::Literal`: converts directly via `CellValue::from` (`ValueLiteral::Null` remains `CellValue::Empty`).
    /// - `CellPayload::Formula`: evaluates via `eval_expr_validated` (preserving mid-expression `CellValue::Empty`)
    ///   and rejects a top-level `CellValue::Empty` result with `TypeError::EmptyFormulaResult`.
    ///
    /// # Errors
    /// Returns `EvalError` if static validation fails, a referenced dependency is missing from the grid,
    /// reference resolution fails, or a formula expression encounters a type or compiler-level evaluation error.
    pub fn evaluate(
        grid: &CellGrid,
        graph: &DependencyGraph,
        symbol_table: &SymbolTable,
        registry: &FunctionRegistry,
    ) -> Result<EvaluatedModel, EvalError> {
        Self::validate_grid(grid, symbol_table, registry)?;

        let mut values = IndexMap::with_capacity(graph.node_count());
        let mut error_types: IndexMap<_, ValueType> = IndexMap::new();

        for cell_id in graph.topological_order() {
            let cell_node = grid
                .get(cell_id)
                .ok_or_else(|| EvalError::cell_not_found(cell_id.clone()))?;

            let value = match &cell_node.payload {
                CellPayload::Literal(lit) => CellValue::from(lit),
                CellPayload::Formula(expr) => {
                    match expr::eval_expr_validated(
                        expr,
                        cell_id,
                        &values,
                        &error_types,
                        symbol_table,
                        registry,
                    )? {
                        CellValue::Empty => {
                            return Err(EvalError::Type {
                                cell: Box::new(cell_id.clone()),
                                source: TypeError::EmptyFormulaResult,
                            });
                        }
                        val @ CellValue::Error(_) => {
                            let ctx = CellEvalContext::new(
                                cell_id,
                                &values,
                                &error_types,
                                symbol_table,
                                registry,
                            );
                            if let Some(vt) = ctx
                                .validate_dormant_expr(expr, None)
                                .map_err(|source| EvalError::function(cell_id.clone(), source))?
                                && vt != ValueType::Empty
                            {
                                error_types.insert(cell_id.clone(), vt);
                            }
                            val
                        }
                        other @ (CellValue::Number(_)
                        | CellValue::Text(_)
                        | CellValue::Boolean(_)) => other,
                    }
                }
            };

            values.insert(cell_id.clone(), value);
        }

        Ok(EvaluatedModel::new(values, symbol_table.clone()))
    }
}
