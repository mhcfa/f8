use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::CellValue;

/// Positional parameter definitions for `IF`.
static IF_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "logical_test",
        "Any value or expression that can be evaluated to TRUE or FALSE.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "value_if_true",
        "The value to return if logical_test evaluates to TRUE.",
        ParamKind::Lazy,
    ),
    ParameterDef {
        name: "value_if_false",
        description: "The value to return if logical_test evaluates to FALSE.",
        kind: ParamKind::Lazy,
        optional: true,
        default_value: Some(CellValue::Boolean(false)),
    },
];

/// `IF(logical_test, value_if_true, [value_if_false])`: Specifies a logical test to perform.
#[derive(Debug, Default, Clone, Copy)]
pub struct IfFunction;

impl ExcelFunction for IfFunction {
    fn name(&self) -> &'static str {
        "IF"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Logical
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        IF_PARAMS
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.len() < 2 {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        }
        if args.len() > 3 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 3,
                found: args.len(),
            });
        }

        let cond_val = args[0].eval_scalar(self.name(), ctx)?;
        if let CellValue::Error(err) = cond_val {
            return Ok(CellValue::Error(err));
        }

        let cond = match cond_val.coerce_to_bool() {
            Ok(b) => b,
            Err(err) => return Ok(CellValue::Error(err)),
        };

        if cond {
            match &args[1] {
                FunctionArg::Omitted => Ok(CellValue::Empty),
                FunctionArg::Scalar(val) => Ok(val.clone()),
                FunctionArg::Lazy(expr) => ctx.eval_expr(expr),
                FunctionArg::Array { .. } => Err(FunctionError::InvalidArgumentKind {
                    function: self.name(),
                    expected: "scalar or expression",
                    found: "array",
                }),
            }
        } else if args.len() < 3 {
            Ok(CellValue::Boolean(false))
        } else {
            match &args[2] {
                FunctionArg::Omitted => Ok(CellValue::Boolean(false)),
                FunctionArg::Scalar(val) => Ok(val.clone()),
                FunctionArg::Lazy(expr) => ctx.eval_expr(expr),
                FunctionArg::Array { .. } => Err(FunctionError::InvalidArgumentKind {
                    function: self.name(),
                    expected: "scalar or expression",
                    found: "array",
                }),
            }
        }
    }
}
