use crate::functions::common::scalar_number;
use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ValueType};

/// Positional parameter definitions for `ABS`.
static ABS_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number",
    "The real number for which you want the absolute value.",
    ParamKind::Scalar,
    ParamType::Number,
)];

/// `ABS(number)`: Returns the absolute value of a number.
#[derive(Debug, Default, Clone, Copy)]
pub struct AbsFunction;

impl ExcelFunction for AbsFunction {
    fn name(&self) -> &'static str {
        "ABS"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        ABS_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        match scalar_number(&args[0]) {
            Ok(n) => Ok(CellValue::number(n.abs())),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
