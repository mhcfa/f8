use crate::functions::common::normalize_zero;
use crate::functions::error::FunctionError;
use crate::functions::financial::discounting::extract_scalar_number;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError};

/// Calculates the periodic payment for an annuity based on constant payments and a constant interest rate.
pub(crate) fn calculate_pmt(
    rate: f64,
    nper: f64,
    pv: f64,
    fv: f64,
    type_val: f64,
) -> Result<f64, ExcelError> {
    if nper == 0.0 {
        return Err(ExcelError::Div0);
    }
    let t = if type_val != 0.0 { 1.0 } else { 0.0 };
    if rate == 0.0 {
        return Ok(-(pv + fv) / nper);
    }
    let term = (1.0 + rate).powf(nper);
    if term.is_nan() || term.is_infinite() {
        return Err(ExcelError::Num);
    }
    let denom = (1.0 + rate * t) * (term - 1.0) / rate;
    if denom == 0.0 {
        return Err(ExcelError::Div0);
    }
    let pmt = -(pv * term + fv) / denom;
    if pmt.is_nan() || pmt.is_infinite() {
        return Err(ExcelError::Num);
    }
    Ok(pmt)
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
    let k = per.trunc();
    let n = nper.trunc();
    if k < 1.0 || k > n || n <= 0.0 {
        return Err(ExcelError::Num);
    }
    if rate == 0.0 {
        return Ok(0.0);
    }
    let t = if type_val != 0.0 { 1.0 } else { 0.0 };
    if t == 1.0 && k == 1.0 {
        return Ok(0.0);
    }
    let pmt = calculate_pmt(rate, nper, pv, fv, type_val)?;
    let ipmt = if t == 0.0 {
        let term = (1.0 + rate).powf(k - 1.0);
        -(pv * term * rate + pmt * (term - 1.0))
    } else {
        let term = (1.0 + rate).powf(k - 2.0);
        -((pv + pmt) * term * rate + pmt * (term - 1.0))
    };
    if ipmt.is_nan() || ipmt.is_infinite() {
        return Err(ExcelError::Num);
    }
    Ok(ipmt)
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
    let pmt = calculate_pmt(rate, nper, pv, fv, type_val)?;
    let ipmt = calculate_ipmt(rate, per, nper, pv, fv, type_val)?;
    Ok(pmt - ipmt)
}

/// Positional parameter definitions for `PMT`.
static PMT_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "rate",
        "The interest rate per period for the loan.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "nper",
        "The total number of payment periods for the loan.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "pv",
        "The present value, or the total amount that a series of future payments is worth now.",
        ParamKind::Scalar,
    ),
    ParameterDef {
        name: "fv",
        description: "The future value, or cash balance you want after the last payment is made.",
        kind: ParamKind::Scalar,
        optional: true,
        default_value: Some(CellValue::Number(0.0)),
    },
    ParameterDef {
        name: "type",
        description: "The number 0 or 1 indicating when payments are due (0 = end, 1 = beginning).",
        kind: ParamKind::Scalar,
        optional: true,
        default_value: Some(CellValue::Number(0.0)),
    },
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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.len() < 3 {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 3,
                found: args.len(),
            });
        }
        if args.len() > 5 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 5,
                found: args.len(),
            });
        }

        let rate = match extract_scalar_number(&args[0]) {
            Ok(r) => r,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let nper = match extract_scalar_number(&args[1]) {
            Ok(n) => n,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let pv = match extract_scalar_number(&args[2]) {
            Ok(p) => p,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let fv = if args.len() >= 4 && !matches!(args[3], FunctionArg::Omitted) {
            match extract_scalar_number(&args[3]) {
                Ok(f) => f,
                Err(e) => return Ok(CellValue::Error(e)),
            }
        } else {
            0.0
        };
        let type_val = if args.len() >= 5 && !matches!(args[4], FunctionArg::Omitted) {
            match extract_scalar_number(&args[4]) {
                Ok(t) => t,
                Err(e) => return Ok(CellValue::Error(e)),
            }
        } else {
            0.0
        };

        match calculate_pmt(rate, nper, pv, fv, type_val) {
            Ok(val) => Ok(CellValue::Number(normalize_zero(val))),
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
    ),
    ParameterDef::required(
        "per",
        "The period for which you want to find the payment, from 1 to nper.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "nper",
        "The total number of payment periods for the loan.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "pv",
        "The present value, or the total amount that a series of future payments is worth now.",
        ParamKind::Scalar,
    ),
    ParameterDef {
        name: "fv",
        description: "The future value, or cash balance you want after the last payment is made.",
        kind: ParamKind::Scalar,
        optional: true,
        default_value: Some(CellValue::Number(0.0)),
    },
    ParameterDef {
        name: "type",
        description: "The number 0 or 1 indicating when payments are due (0 = end, 1 = beginning).",
        kind: ParamKind::Scalar,
        optional: true,
        default_value: Some(CellValue::Number(0.0)),
    },
];

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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.len() < 4 {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 4,
                found: args.len(),
            });
        }
        if args.len() > 6 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 6,
                found: args.len(),
            });
        }

        let rate = match extract_scalar_number(&args[0]) {
            Ok(r) => r,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let per = match extract_scalar_number(&args[1]) {
            Ok(p) => p,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let nper = match extract_scalar_number(&args[2]) {
            Ok(n) => n,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let pv = match extract_scalar_number(&args[3]) {
            Ok(p) => p,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let fv = if args.len() >= 5 && !matches!(args[4], FunctionArg::Omitted) {
            match extract_scalar_number(&args[4]) {
                Ok(f) => f,
                Err(e) => return Ok(CellValue::Error(e)),
            }
        } else {
            0.0
        };
        let type_val = if args.len() >= 6 && !matches!(args[5], FunctionArg::Omitted) {
            match extract_scalar_number(&args[5]) {
                Ok(t) => t,
                Err(e) => return Ok(CellValue::Error(e)),
            }
        } else {
            0.0
        };

        match calculate_ipmt(rate, per, nper, pv, fv, type_val) {
            Ok(val) => Ok(CellValue::Number(normalize_zero(val))),
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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.len() < 4 {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 4,
                found: args.len(),
            });
        }
        if args.len() > 6 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 6,
                found: args.len(),
            });
        }

        let rate = match extract_scalar_number(&args[0]) {
            Ok(r) => r,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let per = match extract_scalar_number(&args[1]) {
            Ok(p) => p,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let nper = match extract_scalar_number(&args[2]) {
            Ok(n) => n,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let pv = match extract_scalar_number(&args[3]) {
            Ok(p) => p,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let fv = if args.len() >= 5 && !matches!(args[4], FunctionArg::Omitted) {
            match extract_scalar_number(&args[4]) {
                Ok(f) => f,
                Err(e) => return Ok(CellValue::Error(e)),
            }
        } else {
            0.0
        };
        let type_val = if args.len() >= 6 && !matches!(args[5], FunctionArg::Omitted) {
            match extract_scalar_number(&args[5]) {
                Ok(t) => t,
                Err(e) => return Ok(CellValue::Error(e)),
            }
        } else {
            0.0
        };

        match calculate_ppmt(rate, per, nper, pv, fv, type_val) {
            Ok(val) => Ok(CellValue::Number(normalize_zero(val))),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}
