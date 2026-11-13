use crate::functions::common::{extract_scalar_integer, extract_scalar_number};
use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType};

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
        ParamType::AnnuityType,
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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let rate_res = extract_scalar_number(&args[0], self.name(), "rate", 1, ctx)?;
        let nper_res = extract_scalar_number(&args[1], self.name(), "nper", 2, ctx)?;
        let pv_res = extract_scalar_number(&args[2], self.name(), "pv", 3, ctx)?;
        let fv_res = match args.get(3) {
            None | Some(FunctionArg::Omitted) => Ok(0.0),
            Some(
                fv_arg @ (FunctionArg::Scalar(_)
                | FunctionArg::TypedErrorScalar { .. }
                | FunctionArg::Array { .. }
                | FunctionArg::Lazy(_)),
            ) => extract_scalar_number(fv_arg, self.name(), "fv", 4, ctx)?,
        };
        let type_res = match args.get(4) {
            None | Some(FunctionArg::Omitted) => Ok(0_i64),
            Some(
                type_arg @ (FunctionArg::Scalar(_)
                | FunctionArg::TypedErrorScalar { .. }
                | FunctionArg::Array { .. }
                | FunctionArg::Lazy(_)),
            ) => extract_scalar_integer(type_arg, self.name(), "type", 5, ctx)?,
        };

        let rate = match rate_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let nper = match nper_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let pv = match pv_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let fv = match fv_res {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };
        let type_val = match type_res {
            Ok(v) => v as f64,
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
        ParamType::AnnuityType,
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

fn extract_amortization_args(
    func_name: &'static str,
    args: &[FunctionArg<'_>],
    ctx: &dyn EvalContext,
) -> Result<Result<AmortizationArgs, ExcelError>, FunctionError> {
    let rate_res = extract_scalar_number(&args[0], func_name, "rate", 1, ctx)?;
    let per_res = extract_scalar_number(&args[1], func_name, "per", 2, ctx)?;
    let nper_res = extract_scalar_number(&args[2], func_name, "nper", 3, ctx)?;
    let pv_res = extract_scalar_number(&args[3], func_name, "pv", 4, ctx)?;
    let fv_res = match args.get(4) {
        None | Some(FunctionArg::Omitted) => Ok(0.0),
        Some(
            fv_arg @ (FunctionArg::Scalar(_)
            | FunctionArg::TypedErrorScalar { .. }
            | FunctionArg::Array { .. }
            | FunctionArg::Lazy(_)),
        ) => extract_scalar_number(fv_arg, func_name, "fv", 5, ctx)?,
    };
    let type_res = match args.get(5) {
        None | Some(FunctionArg::Omitted) => Ok(0_i64),
        Some(
            type_arg @ (FunctionArg::Scalar(_)
            | FunctionArg::TypedErrorScalar { .. }
            | FunctionArg::Array { .. }
            | FunctionArg::Lazy(_)),
        ) => extract_scalar_integer(type_arg, func_name, "type", 6, ctx)?,
    };

    let rate = match rate_res {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    let per = match per_res {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    let nper = match nper_res {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    let pv = match pv_res {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    let fv = match fv_res {
        Ok(v) => v,
        Err(e) => return Ok(Err(e)),
    };
    let type_val = match type_res {
        Ok(v) => v as f64,
        Err(e) => return Ok(Err(e)),
    };

    Ok(Ok(AmortizationArgs {
        rate,
        per,
        nper,
        pv,
        fv,
        type_val,
    }))
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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let parsed = match extract_amortization_args(self.name(), args, ctx)? {
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
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let parsed = match extract_amortization_args(self.name(), args, ctx)? {
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
