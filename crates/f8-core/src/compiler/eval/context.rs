use indexmap::IndexMap;

use super::error::EvalError;
use crate::compiler::graph::resolve_table_ref;
use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::SymbolTable;
use crate::expr::ast::{Expr, TableRef};
use crate::functions::error::FunctionError;
use crate::functions::registry::{FunctionRegistry, ReferenceResolver, ResolvedRef, StageAError};
use crate::functions::traits::{EvalContext, TypeExpectation, TypedRangeGrid};
use crate::value::{CellValue, ValueType};

/// Concrete evaluation context bridging cell coordinates and memoized values to function implementations
/// for already-validated formula ASTs.
#[derive(Clone, Copy)]
pub(crate) struct CellEvalContext<'a> {
    /// Origin cell identifier being evaluated.
    pub(crate) origin: &'a CellId,
    /// Pre-evaluated cell values keyed by cell coordinate.
    pub(crate) memoized_values: &'a IndexMap<CellId, CellValue>,
    /// Concrete non-`Empty` inferred value types for formula cells that evaluated to `CellValue::Error(_)`.
    pub(crate) memoized_error_types: &'a IndexMap<CellId, ValueType>,
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
        memoized_error_types: &'a IndexMap<CellId, ValueType>,
        symbol_table: &'a SymbolTable,
        registry: &'a FunctionRegistry,
    ) -> Self {
        Self {
            origin,
            memoized_values,
            memoized_error_types,
            symbol_table,
            registry,
        }
    }
}

impl ReferenceResolver for CellEvalContext<'_> {
    type Error = FunctionError;

    fn resolve_ref(&mut self, table_ref: &TableRef) -> Result<Option<ResolvedRef>, Self::Error> {
        let slice =
            resolve_table_ref(self.origin, table_ref, self.symbol_table).map_err(|err| {
                FunctionError::EvaluationFailure {
                    function: "validate_dormant_expr",
                    message: err.to_string(),
                }
            })?;

        let mut values = Vec::with_capacity(slice.cells.len());
        let mut error_types = Vec::with_capacity(slice.cells.len());
        for cell_id in &slice.cells {
            if let Some(val) = self.memoized_values.get(cell_id) {
                values.push(val.clone());
                error_types.push(self.memoized_error_types.get(cell_id).copied());
            }
        }

        Ok(Some(ResolvedRef {
            rows: slice.rows,
            cols: slice.cols,
            values: Some(values),
            error_types: Some(error_types),
        }))
    }
}

impl CellEvalContext<'_> {
    fn resolve_evaluated_table_ref(
        &self,
        table_ref: &TableRef,
    ) -> Result<TypedRangeGrid, FunctionError> {
        let slice =
            resolve_table_ref(self.origin, table_ref, self.symbol_table).map_err(|err| {
                FunctionError::EvaluationFailure {
                    function: "eval_range",
                    message: err.to_string(),
                }
            })?;

        let mut values = Vec::with_capacity(slice.cells.len());
        let mut error_types = Vec::with_capacity(slice.cells.len());
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
            let err_vt = self.memoized_error_types.get(cell_id).copied();
            values.push(val);
            error_types.push(err_vt);
        }

        Ok((slice.rows, slice.cols, values, error_types))
    }
}

impl EvalContext for CellEvalContext<'_> {
    fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError> {
        super::expr::eval_expr_validated(
            expr,
            self.origin,
            self.memoized_values,
            self.memoized_error_types,
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

    fn eval_range(&self, expr: &Expr) -> Result<(usize, usize, Vec<CellValue>), FunctionError> {
        match expr {
            Expr::Reference(table_ref) => {
                let (rows, cols, values, _) = self.resolve_evaluated_table_ref(table_ref)?;
                Ok((rows, cols, values))
            }
            Expr::Literal(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::FunctionCall { .. } => {
                let val = self.eval_expr(expr)?;
                Ok((1, 1, vec![val]))
            }
        }
    }

    fn eval_typed_range(&self, expr: &Expr) -> Result<TypedRangeGrid, FunctionError> {
        match expr {
            Expr::Reference(table_ref) => self.resolve_evaluated_table_ref(table_ref),
            Expr::Literal(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::FunctionCall { .. } => {
                let val = self.eval_expr(expr)?;
                let err_vt = if matches!(val, CellValue::Error(_)) {
                    self.validate_dormant_expr(expr, None)?
                        .filter(|vt| *vt != ValueType::Empty)
                } else {
                    None
                };
                Ok((1, 1, vec![val], vec![err_vt]))
            }
        }
    }

    fn validate_dormant_expr(
        &self,
        expr: &Expr,
        expected: Option<TypeExpectation>,
    ) -> Result<Option<ValueType>, FunctionError> {
        let mut resolver = *self;
        match self
            .registry
            .validate_ast_with_resolver(expr, expected, &mut resolver)
        {
            Ok(vt) => Ok(vt),
            Err(StageAError::Function(err) | StageAError::Resolve(err)) => Err(err),
        }
    }
}
