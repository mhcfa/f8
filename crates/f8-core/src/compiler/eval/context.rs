use indexmap::IndexMap;

use super::error::EvalError;
use crate::compiler::graph::resolve_table_ref;
use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::SymbolTable;
use crate::expr::ast::{Expr, TableRef};
use crate::functions::error::FunctionError;
use crate::functions::registry::FunctionRegistry;
use crate::functions::traits::{EvalContext, RangeGrid};
use crate::value::CellValue;

/// Concrete evaluation context bridging cell coordinates and memoized values to function implementations
/// for already-validated formula ASTs.
#[derive(Clone, Copy)]
pub(crate) struct CellEvalContext<'a> {
    /// Origin cell identifier being evaluated.
    pub(crate) origin: &'a CellId,
    /// Pre-evaluated cell values keyed by cell coordinate.
    pub(crate) memoized_values: &'a IndexMap<CellId, CellValue>,
    /// Model metadata and symbol definitions.
    pub(crate) symbol_table: &'a SymbolTable,
    /// Registered Excel functions.
    pub(crate) registry: &'a FunctionRegistry,
}

impl<'a> CellEvalContext<'a> {
    /// Constructs a new `CellEvalContext`.
    #[must_use]
    pub(crate) const fn new(
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

    fn resolve_evaluated_table_ref(
        &self,
        table_ref: &TableRef,
    ) -> Result<RangeGrid, FunctionError> {
        let slice =
            resolve_table_ref(self.origin, table_ref, self.symbol_table).map_err(|err| {
                FunctionError::EvaluationFailure {
                    function: "eval_range",
                    message: err.to_string(),
                }
            })?;

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

        Ok(RangeGrid {
            rows: slice.rows,
            cols: slice.cols,
            values,
        })
    }
}

impl EvalContext for CellEvalContext<'_> {
    fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError> {
        super::expr::eval_expr_validated(
            expr,
            self.origin,
            self.memoized_values,
            self.symbol_table,
            self.registry,
        )
        .map_err(|err| match err {
            EvalError::Function { source, .. } => *source,
            EvalError::Type { source, .. } => FunctionError::Type(source),
            EvalError::SliceInScalarContext { rows, cols, .. } => {
                FunctionError::SliceInScalarContext { rows, cols }
            }
            other @ (EvalError::CellNotFound(_)
            | EvalError::UnevaluatedDependency(_)
            | EvalError::Resolve(_)
            | EvalError::TableNotFound(_)
            | EvalError::RowNotFound { .. }) => FunctionError::EvaluationFailure {
                function: "eval_expr",
                message: other.to_string(),
            },
        })
    }

    fn eval_range(&self, expr: &Expr) -> Result<RangeGrid, FunctionError> {
        match expr {
            Expr::Reference(table_ref) => self.resolve_evaluated_table_ref(table_ref),
            Expr::Literal(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::FunctionCall { .. } => {
                let val = self.eval_expr(expr)?;
                Ok(RangeGrid {
                    rows: 1,
                    cols: 1,
                    values: vec![val],
                })
            }
        }
    }
}
