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

use crate::compiler::graph::{DependencyGraph, resolve_table_ref};
use crate::compiler::ir::cell::{CellGrid, CellPayload};
use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::SymbolTable;
use crate::functions::registry::{FunctionRegistry, StageAError, StaticRef};
use crate::value::{CellValue, TypeError, ValueType};

/// In-memory topological evaluator executing cells across compiled model DAGs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Evaluator;

impl Evaluator {
    /// Statically validates all formula cells across the cell grid in topological dependency order,
    /// threading synthesized `Option<ValueType>` across cells.
    ///
    /// # Errors
    /// Returns [`EvalError`] on the first cell containing an unknown function, signature violation,
    /// static type or integer violation, scalar slice violation, or slice rank/dimension/index error.
    pub fn validate_grid(
        grid: &CellGrid,
        graph: &DependencyGraph,
        symbol_table: &SymbolTable,
        registry: &FunctionRegistry,
    ) -> Result<(), EvalError> {
        let mut cell_types: IndexMap<CellId, Option<ValueType>> =
            IndexMap::with_capacity(graph.node_count());

        for cell_id in graph.topological_order() {
            let cell_node = grid
                .get(cell_id)
                .ok_or_else(|| EvalError::cell_not_found(cell_id.clone()))?;

            let synth_vt = match &cell_node.payload {
                CellPayload::Literal(lit) => CellValue::from(lit)
                    .value_type()
                    .filter(|vt| *vt != ValueType::Empty),
                CellPayload::Formula(expr) => registry
                    .validate_and_synth_expr(expr, |table_ref| {
                        let slice = resolve_table_ref(cell_id, table_ref, symbol_table)?;
                        let elem_types = slice
                            .cells
                            .iter()
                            .map(|cid| cell_types.get(cid).copied().flatten())
                            .collect();
                        Ok(Some(StaticRef {
                            rows: slice.rows,
                            cols: slice.cols,
                            axis_role: slice.axis_role,
                            elem_types,
                        }))
                    })
                    .map_err(|err| match err {
                        StageAError::Function(source) => {
                            EvalError::function(cell_id.clone(), source)
                        }
                        StageAError::Resolve(source) => EvalError::Resolve(source),
                    })?,
            };

            cell_types.insert(cell_id.clone(), synth_vt);
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
        Self::validate_grid(grid, graph, symbol_table, registry)?;

        let mut values = IndexMap::with_capacity(graph.node_count());

        for cell_id in graph.topological_order() {
            let cell_node = grid
                .get(cell_id)
                .ok_or_else(|| EvalError::cell_not_found(cell_id.clone()))?;

            let value = match &cell_node.payload {
                CellPayload::Literal(lit) => CellValue::from(lit),
                CellPayload::Formula(expr) => {
                    match expr::eval_expr_validated(expr, cell_id, &values, symbol_table, registry)?
                    {
                        CellValue::Empty => {
                            return Err(EvalError::Type {
                                cell: Box::new(cell_id.clone()),
                                source: TypeError::EmptyFormulaResult,
                            });
                        }
                        other @ (CellValue::Number(_)
                        | CellValue::Text(_)
                        | CellValue::Boolean(_)
                        | CellValue::Error(_)) => other,
                    }
                }
            };

            values.insert(cell_id.clone(), value);
        }

        Ok(EvaluatedModel::new(values, symbol_table.clone()))
    }
}
