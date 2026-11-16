use crate::functions::common::require_args;
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    BranchPolicy, EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType,
    ParameterDef, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ValueType};

static IFERROR_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "value",
        "The value or expression to check for an error.",
        ParamKind::Scalar,
        ParamType::Any,
    ),
    ParameterDef::required(
        "value_if_error",
        "The value to return if the formula evaluates to an error.",
        ParamKind::Lazy,
        ParamType::Any,
    ),
];

/// `IFERROR(value, value_if_error)`: Returns value_if_error if value is an error; otherwise returns value.
#[derive(Debug, Default, Clone, Copy)]
pub struct IferrorFunction;

impl ExcelFunction for IferrorFunction {
    fn name(&self) -> &'static str {
        "IFERROR"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Logical
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        IFERROR_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        None
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::Branch(BranchPolicy::ErrorTrap)
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let [val_arg, fallback_arg] = require_args(self, args)?;

        let evaluated_value = val_arg.eval_scalar(self.name(), "value", 1, ctx)?;

        if matches!(evaluated_value, CellValue::Error(_)) {
            fallback_arg.eval_scalar(self.name(), "value_if_error", 2, ctx)
        } else {
            Ok(evaluated_value)
        }
    }
}
