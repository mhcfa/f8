//! Runtime argument binding (`bind_arguments`) and active `EmptyPolicy` enforcement.

use crate::expr::ast::{CallArg, Expr};
use crate::functions::error::FunctionError;
use crate::functions::registry::FunctionRegistry;
use crate::functions::traits::{
    EmptyPolicy, EvalContext, ExcelFunction, FunctionArg, ParamKind, ParameterDef, RangeGrid,
    ReferenceKind, VariadicRule,
};
use crate::value::{CellValue, TypeError};

impl FunctionRegistry {
    /// Binds parsed formula arguments 1-to-1 into runtime [`FunctionArg`] values, substituting defaults
    /// for intermediate omitted optional parameters and enforcing [`EmptyPolicy`] on bound arguments.
    pub fn bind_arguments<'a>(
        &self,
        func: &dyn ExcelFunction,
        expr_args: &'a [CallArg],
        ctx: &dyn EvalContext,
    ) -> Result<Vec<FunctionArg<'a>>, FunctionError> {
        let mut bound = Vec::with_capacity(expr_args.len());

        for (i, call_arg) in expr_args.iter().enumerate() {
            let param = resolve_parameter(func, i)?;
            let arg_expr = match call_arg {
                CallArg::Expr(expr) => expr,
                CallArg::Omitted => {
                    bound.push(bind_omitted_arg(param));
                    continue;
                }
            };

            match (param.kind, arg_expr) {
                (ParamKind::Lazy, _) => {
                    bound.push(FunctionArg::Lazy(arg_expr));
                }
                (ParamKind::Range | ParamKind::ScalarOrRange, Expr::Reference(table_ref)) => {
                    let RangeGrid { rows, cols, values } = ctx.eval_range(table_ref)?;
                    let ref_kind = if table_ref.is_slice() {
                        ReferenceKind::Slice
                    } else {
                        ReferenceKind::SingleCell
                    };
                    if matches!(
                        (param.empty_policy, ref_kind),
                        (
                            EmptyPolicy::Forbid,
                            ReferenceKind::SingleCell | ReferenceKind::Slice
                        ) | (EmptyPolicy::AllowInSlice, ReferenceKind::SingleCell)
                    ) && values.iter().any(CellValue::is_empty)
                    {
                        return Err(FunctionError::Type(TypeError::EmptyArgument {
                            function: func.name(),
                            param: param.name,
                            position: i + 1,
                        }));
                    }
                    bound.push(FunctionArg::range(rows, cols, values, ref_kind));
                }
                (
                    ParamKind::Range,
                    Expr::Literal(_)
                    | Expr::Unary { .. }
                    | Expr::Binary { .. }
                    | Expr::FunctionCall { .. },
                ) => {
                    return Err(FunctionError::InvalidArgumentKind {
                        function: func.name(),
                        expected: "range",
                        found: "scalar",
                    });
                }
                (
                    ParamKind::ScalarOrRange,
                    Expr::Literal(_)
                    | Expr::Unary { .. }
                    | Expr::Binary { .. }
                    | Expr::FunctionCall { .. },
                )
                | (ParamKind::Scalar, _) => {
                    let val = ctx.eval_expr(arg_expr)?;
                    if param.empty_policy != EmptyPolicy::Allow && val.is_empty() {
                        return Err(FunctionError::Type(TypeError::EmptyArgument {
                            function: func.name(),
                            param: param.name,
                            position: i + 1,
                        }));
                    }
                    bound.push(FunctionArg::Scalar(val));
                }
            }
        }

        Ok(bound)
    }
}

fn bind_omitted_arg<'a>(param: &ParameterDef) -> FunctionArg<'a> {
    if let Some(default_val) = param.requirement.default_value() {
        FunctionArg::Scalar(default_val.clone())
    } else {
        FunctionArg::Omitted
    }
}

/// Resolves the expected parameter definition for a given 0-indexed argument position,
/// applying cyclical pairs alternation when `VariadicRule::Pairs` is enabled.
pub(crate) fn resolve_parameter(
    func: &dyn ExcelFunction,
    index: usize,
) -> Result<&ParameterDef, FunctionError> {
    if index < func.parameters().len() {
        Ok(&func.parameters()[index])
    } else if func.variadic_rule() == VariadicRule::Pairs {
        let fixed_count = func.parameters().len();
        if fixed_count < 2 {
            return Err(FunctionError::EvaluationFailure {
                function: func.name(),
                message: "variadic pairs rule requires at least two parameter definitions".into(),
            });
        }
        let offset = (index - fixed_count) % 2;
        Ok(&func.parameters()[fixed_count - 2 + offset])
    } else {
        func.parameters()
            .last()
            .ok_or_else(|| FunctionError::EvaluationFailure {
                function: func.name(),
                message: "variadic argument without parameter definition".into(),
            })
    }
}
