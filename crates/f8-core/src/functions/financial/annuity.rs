use crate::functions::common::{optional_scalar_integer, optional_scalar_number, scalar_number};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType, normalize_zero};

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

/// Calculates the interest payment for a given period for an investment based on periodic, constant payments and a constant interest rate.
pub(crate) fn calculate_ipmt(
    rate: f64,
    per: f64,
    nper: f64,
    pv: f64,
    fv: f64,
    type_val: f64,
) -> Result<f64, ExcelError> {
    if !rate.is_finite()
        || !per.is_finite()
        || !nper.is_finite()
        || !pv.is_finite()
        || !fv.is_finite()
        || !type_val.is_finite()
    {
        return Err(ExcelError::Num);
    }
    if rate <= -1.0 || per < 1.0 || per > nper {
        return Err(ExcelError::Num);
    }
    let pmt = calculate_pmt(rate, nper, pv, fv, type_val)?;
    if rate == 0.0 {
        return Ok(0.0);
    }
    let t = if type_val != 0.0 { 1.0 } else { 0.0 };
    if t == 1.0 && per == 1.0 {
        return Ok(0.0);
    }
    let ipmt = if t == 0.0 {
        let term = (1.0 + rate).powf(per - 1.0);
        if !term.is_finite() {
            return Err(ExcelError::Num);
        }
        -(pv * term * rate + pmt * (term - 1.0))
    } else {
        let term = (1.0 + rate).powf(per - 2.0);
        if !term.is_finite() {
            return Err(ExcelError::Num);
        }
        -((pv + pmt) * term * rate + pmt * (term - 1.0))
    };
    if !ipmt.is_finite() {
        return Err(ExcelError::Num);
    }
    Ok(normalize_zero(ipmt))
}

/// Calculates the payment on the principal for a given investment based on periodic, constant payments and a constant interest rate.
pub(crate) fn calculate_ppmt(
    rate: f64,
    per: f64,
    nper: f64,
    pv: f64,
    fv: f64,
    type_val: f64,
) -> Result<f64, ExcelError> {
    if !rate.is_finite()
        || !per.is_finite()
        || !nper.is_finite()
        || !pv.is_finite()
        || !fv.is_finite()
        || !type_val.is_finite()
    {
        return Err(ExcelError::Num);
    }
    let ipmt = calculate_ipmt(rate, per, nper, pv, fv, type_val)?;
    let pmt = calculate_pmt(rate, nper, pv, fv, type_val)?;
    let ppmt = pmt - ipmt;
    if !ppmt.is_finite() {
        return Err(ExcelError::Num);
    }
    Ok(normalize_zero(ppmt))
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
        let rate = match scalar_number(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let nper = match scalar_number(&args[1]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let pv = match scalar_number(&args[2]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let fv = match optional_scalar_number(args, 3, 0.0) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let type_val = match optional_scalar_integer(args, 4, 0) {
            Ok(v) if (0..=1).contains(&v) => v as f64,
            Ok(_) => return Ok(CellValue::Error(ExcelError::Num)),
            Err(e) => return Ok(CellValue::Error(e)),
        };

        match calculate_pmt(rate, nper, pv, fv, type_val) {
            Ok(val) => Ok(CellValue::number(val)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

/// Positional parameter definitions for `IPMT` and `PPMT`.
static AMORTIZATION_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "rate",
        "The interest rate per period for the loan.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::required(
        "per",
        "The period for which you want to find the payment, from 1 to nper.",
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

struct AmortizationArgs {
    rate: f64,
    per: f64,
    nper: f64,
    pv: f64,
    fv: f64,
    type_val: f64,
}

fn extract_amortization_args(args: &[FunctionArg<'_>]) -> Result<AmortizationArgs, ExcelError> {
    let rate = scalar_number(&args[0])?;
    let per = scalar_number(&args[1])?;
    let nper = scalar_number(&args[2])?;
    let pv = scalar_number(&args[3])?;
    let fv = optional_scalar_number(args, 4, 0.0)?;
    let type_int = optional_scalar_integer(args, 5, 0)?;
    if !(0..=1).contains(&type_int) {
        return Err(ExcelError::Num);
    }
    let type_val = type_int as f64;

    Ok(AmortizationArgs {
        rate,
        per,
        nper,
        pv,
        fv,
        type_val,
    })
}

/// `IPMT(rate, per, nper, pv, [fv], [type])`: Calculates interest portion of annuity payment for a given period.
#[derive(Debug, Default, Clone, Copy)]
pub struct IpmtFunction;

impl ExcelFunction for IpmtFunction {
    fn name(&self) -> &'static str {
        "IPMT"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Financial
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        AMORTIZATION_PARAMS
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
        let parsed = match extract_amortization_args(args) {
            Ok(parsed) => parsed,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        match calculate_ipmt(
            parsed.rate,
            parsed.per,
            parsed.nper,
            parsed.pv,
            parsed.fv,
            parsed.type_val,
        ) {
            Ok(val) => Ok(CellValue::number(val)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

/// `PPMT(rate, per, nper, pv, [fv], [type])`: Calculates principal portion of annuity payment for a given period.
#[derive(Debug, Default, Clone, Copy)]
pub struct PpmtFunction;

impl ExcelFunction for PpmtFunction {
    fn name(&self) -> &'static str {
        "PPMT"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Financial
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        AMORTIZATION_PARAMS
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
        let parsed = match extract_amortization_args(args) {
            Ok(parsed) => parsed,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        match calculate_ppmt(
            parsed.rate,
            parsed.per,
            parsed.nper,
            parsed.pv,
            parsed.fv,
            parsed.type_val,
        ) {
            Ok(val) => Ok(CellValue::number(val)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{calculate_ipmt, calculate_pmt, calculate_ppmt};
    use crate::value::ExcelError;

    #[test]
    fn helpers_reject_non_finite_inputs_and_overflow() -> Result<(), ExcelError> {
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

            assert_eq!(
                calculate_ipmt(bad, 1.0, 10.0, 1000.0, 0.0, 0.0),
                Err(ExcelError::Num)
            );
            assert_eq!(
                calculate_ipmt(0.0, bad, 10.0, 1000.0, 0.0, 0.0),
                Err(ExcelError::Num)
            );
            assert_eq!(
                calculate_ipmt(0.0, 1.0, bad, 1000.0, 0.0, 0.0),
                Err(ExcelError::Num)
            );
            assert_eq!(
                calculate_ipmt(0.0, 1.0, 10.0, bad, 0.0, 0.0),
                Err(ExcelError::Num)
            );
            assert_eq!(
                calculate_ipmt(0.0, 1.0, 10.0, 1000.0, bad, 0.0),
                Err(ExcelError::Num)
            );
            assert_eq!(
                calculate_ipmt(0.0, 1.0, 10.0, 1000.0, 0.0, bad),
                Err(ExcelError::Num)
            );

            assert_eq!(
                calculate_ppmt(0.05, bad, 10.0, 1000.0, 0.0, 0.0),
                Err(ExcelError::Num)
            );
            assert_eq!(
                calculate_ppmt(0.0, 1.0, 10.0, bad, 0.0, 0.0),
                Err(ExcelError::Num)
            );
        }

        // Zero-rate pv + fv overflow fails across all three helpers
        assert_eq!(
            calculate_pmt(0.0, 1.0, 1e308, 1e308, 0.0),
            Err(ExcelError::Num)
        );
        assert_eq!(
            calculate_ipmt(0.0, 1.0, 1.0, 1e308, 1e308, 0.0),
            Err(ExcelError::Num)
        );
        assert_eq!(
            calculate_ppmt(0.0, 1.0, 1.0, 1e308, 1e308, 0.0),
            Err(ExcelError::Num)
        );

        // Zero principal/future value normalizes -0.0 to +0.0
        let zero_pmt = calculate_pmt(0.0, 10.0, 0.0, 0.0, 0.0)?;
        assert!(zero_pmt.is_sign_positive() && zero_pmt == 0.0);
        Ok(())
    }
}
