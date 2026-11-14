use crate::functions::common::{extract_scalar_integer, extract_scalar_number};
use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, DecimalRoundingMode, ValueType, round_decimal_f64};

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
        ParamType::Integer,
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
                ctx: &dyn EvalContext,
            ) -> Result<CellValue, FunctionError> {
                validate_bound_args(self, args)?;

                let num_res = extract_scalar_number(&args[0], self.name(), "number", 1, ctx)?;
                let digits_res =
                    extract_scalar_integer(&args[1], self.name(), "num_digits", 2, ctx)?;

                let num = match num_res {
                    Ok(n) => n,
                    Err(err) => return Ok(CellValue::Error(err)),
                };
                let digits_i64 = match digits_res {
                    Ok(d) => d,
                    Err(err) => return Ok(CellValue::Error(err)),
                };

                let result = round_decimal_f64(num, digits_i64, $round_mode);
                Ok(CellValue::number(result))
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
