use crate::functions::common::{optional_scalar_number, require_args, scalar_number};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType, f64_to_strict_integer, normalize_zero};

/// Calculates the periodic payment for an annuity based on constant payments and a constant interest rate.
pub(crate) fn calculate_pmt(
    rate: f64,
    nper: f64,
    pv: f64,
    fv: f64,
    type_val: f64,
) -> Result<f64, ExcelError> {
    if !rate.is_finite()
        || !nper.is_finite()
        || !pv.is_finite()
        || !fv.is_finite()
        || !type_val.is_finite()
    {
        return Err(ExcelError::Num);
    }
    if rate <= -1.0 || nper == 0.0 {
        return Err(ExcelError::Num);
    }
    if rate == 0.0 {
        let pmt = -(pv + fv) / nper;
        if !pmt.is_finite() {
            return Err(ExcelError::Num);
        }
        return Ok(normalize_zero(pmt));
    }
    let t = if type_val != 0.0 { 1.0 } else { 0.0 };
    let term = (1.0 + rate).powf(nper);
    if !term.is_finite() {
        return Err(ExcelError::Num);
    }
    let denom = (1.0 + rate * t) * (term - 1.0) / rate;
    if denom == 0.0 || !denom.is_finite() {
        return Err(ExcelError::Num);
    }
    let pmt = -(pv * term + fv) / denom;
    if !pmt.is_finite() {
        return Err(ExcelError::Num);
    }
    Ok(normalize_zero(pmt))
}

/// Positional parameter definitions for `PMT`.
static PMT_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "rate",
        "The interest rate per period for the loan.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::required(
        "nper",
        "The total number of payment periods for the loan.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::required(
        "pv",
        "The present value, or the total amount that a series of future payments is worth now.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::with_default(
        "fv",
        "The future value, or cash balance you want after the last payment is made.",
        ParamKind::Scalar,
        ParamType::Number,
        CellValue::Number(0.0),
    ),
    ParameterDef::with_default(
        "type",
        "The integer 0 or 1 indicating when payments are due (0 = end, 1 = beginning).",
        ParamKind::Scalar,
        ParamType::Number,
        CellValue::Number(0.0),
    ),
];

/// `PMT(rate, nper, pv, [fv], [type])`: Calculates the payment for a loan based on constant payments and a constant interest rate.
#[derive(Debug, Default, Clone, Copy)]
pub struct PmtFunction;

impl ExcelFunction for PmtFunction {
    fn name(&self) -> &'static str {
        "PMT"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Financial
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        PMT_PARAMS
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
        let [rate_arg, nper_arg, pv_arg] = require_args(self, args)?;
        let eval = || {
            let rate = scalar_number(rate_arg)?;
            let nper = scalar_number(nper_arg)?;
            let pv = scalar_number(pv_arg)?;
            let fv = optional_scalar_number(args, 3, 0.0)?;
            let type_f = optional_scalar_number(args, 4, 0.0)?;
            let type_i64 = f64_to_strict_integer(type_f)?;
            if !(0..=1).contains(&type_i64) {
                return Err(ExcelError::Num);
            }
            calculate_pmt(rate, nper, pv, fv, type_i64 as f64)
        };

        match eval() {
            Ok(val) => Ok(CellValue::number(val)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::calculate_pmt;
    use crate::value::ExcelError;

    #[test]
    fn calculate_pmt_rejects_non_finite_inputs_and_overflow() -> Result<(), ExcelError> {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                calculate_pmt(bad, 10.0, 1000.0, 0.0, 0.0),
                Err(ExcelError::Num)
            );
            assert_eq!(
                calculate_pmt(0.0, bad, 1000.0, 0.0, 0.0),
                Err(ExcelError::Num)
            );
            assert_eq!(
                calculate_pmt(0.0, 10.0, bad, 0.0, 0.0),
                Err(ExcelError::Num)
            );
            assert_eq!(
                calculate_pmt(0.0, 10.0, 1000.0, bad, 0.0),
                Err(ExcelError::Num)
            );
            assert_eq!(
                calculate_pmt(0.0, 10.0, 1000.0, 0.0, bad),
                Err(ExcelError::Num)
            );
        }

        // Zero-rate pv + fv overflow fails
        assert_eq!(
            calculate_pmt(0.0, 1.0, 1e308, 1e308, 0.0),
            Err(ExcelError::Num)
        );

        // Zero principal/future value normalizes -0.0 to +0.0
        let zero_pmt = calculate_pmt(0.0, 10.0, 0.0, 0.0, 0.0)?;
        assert!(zero_pmt.is_sign_positive() && zero_pmt == 0.0);
        Ok(())
    }
}
