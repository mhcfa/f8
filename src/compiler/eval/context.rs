use indexmap::IndexMap;

use super::error::EvalError;
use crate::compiler::graph::resolve_table_ref;
use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::SymbolTable;
use crate::expr::ast::Expr;
use crate::functions::error::FunctionError;
use crate::functions::registry::FunctionRegistry;
use crate::functions::traits::EvalContext;
use crate::value::CellValue;

/// Concrete evaluation context bridging cell coordinates and memoized values to function implementations.
pub struct CellEvalContext<'a> {
    /// Origin cell identifier being evaluated.
    pub origin: &'a CellId,
    /// Pre-evaluated cell values keyed by cell coordinate.
    pub memoized_values: &'a IndexMap<CellId, CellValue>,
    /// Model metadata and symbol definitions.
    pub symbol_table: &'a SymbolTable,
    /// Registered Excel functions.
    pub registry: &'a FunctionRegistry,
}

impl<'a> CellEvalContext<'a> {
    /// Constructs a new `CellEvalContext`.
    #[must_use]
    pub fn new(
        origin: &'a CellId,
        memoized_values: &'a IndexMap<CellId, CellValue>,
        symbol_table: &'a SymbolTable,
        registry: &'a FunctionRegistry,
    ) -> Self {
        Self {
            origin,
            memoized_values,
            symbol_table,
            registry,
        }
    }
}

impl<'a> EvalContext for CellEvalContext<'a> {
    fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError> {
        super::expr::eval_expr(
            expr,
            self.origin,
            self.memoized_values,
            self.symbol_table,
            self.registry,
        )
        .map_err(|err| match err {
            EvalError::Function { source, .. } => *source,
            other => FunctionError::EvaluationFailure {
                function: "eval_expr",
                message: other.to_string(),
            },
        })
    }

    fn eval_range(&self, expr: &Expr) -> Result<(usize, usize, Vec<CellValue>), FunctionError> {
        match expr {
            Expr::Reference(table_ref) => {
                let slice = resolve_table_ref(self.origin, table_ref, self.symbol_table).map_err(
                    |err| FunctionError::EvaluationFailure {
                        function: "eval_range",
                        message: err.to_string(),
                    },
                )?;

                let mut values = Vec::with_capacity(slice.cells.len());
                for cell_id in &slice.cells {
                    let val = self.memoized_values.get(cell_id).cloned().ok_or_else(|| {
                        FunctionError::EvaluationFailure {
                            function: "eval_range",
                            message: format!(
                                "referenced cell '{cell_id}' has not been evaluated prior to origin '{}'",
                                self.origin
                            ),
                        }
                    })?;
                    values.push(val);
                }

                Ok((slice.rows, slice.cols, values))
            }
            _ => {
                let val = self.eval_expr(expr)?;
                Ok((1, 1, vec![val]))
            }
        }
    }
}
