use crate::functions::common::{require_args, scalar_number};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    VariadicRule,
};
use crate::value::{
    CellValue, DecimalRoundingMode, ValueType, f64_to_strict_integer, round_decimal_f64,
};

/// Positional parameter definitions for rounding functions (`ROUND`, `ROUNDUP`, `ROUNDDOWN`).
static ROUND_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "number",
        "The real number that you want to round.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::required(
        "num_digits",
        "The number of digits to which you want to round the number.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
];

/// Helper macro to implement rounding functions.
macro_rules! impl_rounding_function {
    ($struct_name:ident, $func_name:literal, $round_mode:expr, $doc:expr) => {
        #[doc = $doc]
        #[derive(Debug, Default, Clone, Copy)]
        pub struct $struct_name;

        impl ExcelFunction for $struct_name {
            fn name(&self) -> &'static str {
                $func_name
            }

            fn category(&self) -> FunctionCategory {
                FunctionCategory::Math
            }

            fn parameters(&self) -> &'static [ParameterDef] {
                ROUND_PARAMS
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
                let [num_arg, digits_arg] = require_args(self, args)?;
                let eval = || {
                    let num = scalar_number(num_arg)?;
                    let digits_f64 = scalar_number(digits_arg)?;
                    let digits_i64 = f64_to_strict_integer(digits_f64)?;
                    Ok(round_decimal_f64(num, digits_i64, $round_mode))
                };
                match eval() {
                    Ok(result) => Ok(CellValue::number(result)),
                    Err(err) => Ok(CellValue::Error(err)),
                }
            }
        }
    };
}

impl_rounding_function!(
    RoundFunction,
    "ROUND",
    DecimalRoundingMode::HalfAway,
    "`ROUND(number, num_digits)`: Rounds a number to a specified number of digits (half away from zero)."
);

impl_rounding_function!(
    RoundupFunction,
    "ROUNDUP",
    DecimalRoundingMode::Away,
    "`ROUNDUP(number, num_digits)`: Rounds a number upward, away from zero."
);

impl_rounding_function!(
    RounddownFunction,
    "ROUNDDOWN",
    DecimalRoundingMode::Truncate,
    "`ROUNDDOWN(number, num_digits)`: Rounds a number downward, toward zero."
);
