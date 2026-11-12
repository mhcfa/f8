use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, normalize_zero};

/// Positional parameter definitions for `ABS`.
static ABS_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number",
    "The real number for which you want the absolute value.",
    ParamKind::Scalar,
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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let val = args[0].eval_scalar(self.name(), ctx)?;
        match val.coerce_to_number() {
            Ok(n) => Ok(CellValue::Number(normalize_zero(n.abs()))),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}
