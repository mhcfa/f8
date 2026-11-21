use crate::functions::common::{require_args, scalar_number};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError, MAX_SAFE_INTEGER, ValueType};

/// Positional parameter definitions for `ABS`.
static ABS_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number",
    "The real number for which you want the absolute value.",
    ParamKind::Scalar,
    ParamType::Number,
)];

/// Positional parameter definitions for `LN`.
static LN_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number",
    "The positive real number for which you want the natural logarithm.",
    ParamKind::Scalar,
    ParamType::Number,
)];

/// Positional parameter definitions for `EXP`.
static EXP_PARAMS: &[ParameterDef] = &[ParameterDef::required(
    "number",
    "The exponent applied to the base e.",
    ParamKind::Scalar,
    ParamType::Number,
)];

/// Positional parameter definitions for `MOD`.
static MOD_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "number",
        "The number for which you want to find the remainder.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::required(
        "divisor",
        "The number by which you want to divide number.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
];

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
        let [num_arg] = require_args(self, args)?;
        match scalar_number(num_arg) {
            Ok(n) => Ok(CellValue::number(n.abs())),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// `LN(number)`: Returns the natural logarithm of a positive real number.
#[derive(Debug, Default, Clone, Copy)]
pub struct LnFunction;

impl ExcelFunction for LnFunction {
    fn name(&self) -> &'static str {
        "LN"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        LN_PARAMS
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
        let [num_arg] = require_args(self, args)?;
        let n = match scalar_number(num_arg) {
            Ok(n) => n,
            Err(err) => return Ok(CellValue::Error(err)),
        };
        if n <= 0.0 {
            return Ok(CellValue::Error(ExcelError::Num));
        }
        Ok(CellValue::number(n.ln()))
    }
}

/// `EXP(number)`: Returns `e` raised to the power of a number.
#[derive(Debug, Default, Clone, Copy)]
pub struct ExpFunction;

impl ExcelFunction for ExpFunction {
    fn name(&self) -> &'static str {
        "EXP"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        EXP_PARAMS
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
        let [num_arg] = require_args(self, args)?;
        match scalar_number(num_arg) {
            Ok(n) => Ok(CellValue::number(n.exp())),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// `MOD(number, divisor)`: Returns Excel's IEEE-754 floored remainder
/// `number - divisor * floor(number / divisor)` (matching the sign of `divisor` on exact/dyadic
/// values, with standard IEEE-754 rounding behavior on non-dyadic decimals).
#[derive(Debug, Default, Clone, Copy)]
pub struct ModFunction;

impl ExcelFunction for ModFunction {
    fn name(&self) -> &'static str {
        "MOD"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        MOD_PARAMS
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
        let [num_arg, div_arg] = require_args(self, args)?;
        let num = match scalar_number(num_arg) {
            Ok(n) => n,
            Err(err) => return Ok(CellValue::Error(err)),
        };
        let div = match scalar_number(div_arg) {
            Ok(d) => d,
            Err(err) => return Ok(CellValue::Error(err)),
        };
        if div == 0.0 {
            return Ok(CellValue::Error(ExcelError::Div0));
        }
        let quotient = num / div;
        if !quotient.is_finite() || quotient.abs() > MAX_SAFE_INTEGER {
            return Ok(CellValue::Error(ExcelError::Num));
        }
        let q_floor =
            if quotient == 0.0 && num != 0.0 && num.is_sign_negative() != div.is_sign_negative() {
                -1.0
            } else {
                quotient.floor()
            };
        let product = div * q_floor;
        let remainder = if product.is_finite() {
            num - product
        } else if q_floor < 0.0 {
            (num - div * (q_floor + 1.0)) + div
        } else {
            num - div * (q_floor - 1.0)
        };
        Ok(CellValue::number(remainder))
    }
}
