use crate::functions::common::{
    round_away_from_zero, round_half_away_from_zero, round_towards_zero,
};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError};

/// Positional parameter definitions for rounding functions (`ROUND`, `ROUNDUP`, `ROUNDDOWN`).
static ROUND_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "number",
        "The real number that you want to round.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "num_digits",
        "The number of digits to which you want to round the number.",
        ParamKind::Scalar,
    ),
];

/// Helper macro to implement rounding functions.
macro_rules! impl_rounding_function {
    ($struct_name:ident, $func_name:literal, $round_op:path, $doc:expr) => {
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
                if args.len() > 2 {
                    return Err(FunctionError::TooManyArguments {
                        function: self.name(),
                        expected: 2,
                        found: args.len(),
                    });
                }

                let val = args[0].eval_scalar(self.name(), ctx)?;
                let num = match val.coerce_to_number() {
                    Ok(n) => n,
                    Err(err) => return Ok(CellValue::Error(err)),
                };

                let digits_val = args[1].eval_scalar(self.name(), ctx)?;
                let digits_num = match digits_val.coerce_to_number() {
                    Ok(n) => n,
                    Err(err) => return Ok(CellValue::Error(err)),
                };

                if !digits_num.is_finite() {
                    return Ok(CellValue::Error(ExcelError::Value));
                }

                let digits = digits_num.trunc() as i32;
                let result = $round_op(num, digits);
                Ok(CellValue::Number(result))
            }
        }
    };
}

impl_rounding_function!(
    RoundFunction,
    "ROUND",
    round_half_away_from_zero,
    "`ROUND(number, num_digits)`: Rounds a number to a specified number of digits (half away from zero)."
);

impl_rounding_function!(
    RoundupFunction,
    "ROUNDUP",
    round_away_from_zero,
    "`ROUNDUP(number, num_digits)`: Rounds a number upward, away from zero."
);

impl_rounding_function!(
    RounddownFunction,
    "ROUNDDOWN",
    round_towards_zero,
    "`ROUNDDOWN(number, num_digits)`: Rounds a number downward, toward zero."
);
