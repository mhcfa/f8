use crate::functions::common::normalize_zero;
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError};

/// Extracts a scalar numeric value from an argument, coercing booleans and empty cells,
/// bubbling errors, and rejecting text with `#VALUE!`.
pub(crate) fn extract_scalar_number(arg: &FunctionArg<'_>) -> Result<f64, ExcelError> {
    match arg {
        FunctionArg::Scalar(val) => val.coerce_to_number(),
        FunctionArg::Array { values, .. } if values.len() == 1 => values[0].coerce_to_number(),
        _ => Err(ExcelError::Value),
    }
}

/// Collects an ordered sequence of numeric cash flows across one or more arguments.
///
/// - In range references (`is_range_ref: true`), text, booleans, and empty cells are ignored.
/// - In direct scalars and array literals, booleans and empty cells convert, while text produces `#VALUE!`.
/// - Errors bubble up immediately.
pub(crate) fn collect_cash_flows(args: &[FunctionArg<'_>]) -> Result<Vec<f64>, ExcelError> {
    let mut cfs = Vec::new();
    for arg in args {
        match arg {
            FunctionArg::Scalar(val) => {
                cfs.push(val.coerce_to_number()?);
            }
            FunctionArg::Array {
                values,
                is_range_ref,
                ..
            } => {
                if *is_range_ref {
                    for v in values {
                        match v {
                            CellValue::Number(n) => cfs.push(*n),
                            CellValue::Error(e) => return Err(*e),
                            CellValue::Text(_) | CellValue::Boolean(_) | CellValue::Empty => {}
                        }
                    }
                } else {
                    for v in values {
                        cfs.push(v.coerce_to_number()?);
                    }
                }
            }
            FunctionArg::Lazy(_) | FunctionArg::Omitted => {}
        }
    }
    Ok(cfs)
}

/// Extracts a sequence of numbers from a single array argument where every entry must be numeric.
/// Used for parallel vectors in non-periodic functions (`XNPV` and `XIRR`).
pub(crate) fn extract_vector_numbers(arg: &FunctionArg<'_>) -> Result<Vec<f64>, ExcelError> {
    match arg {
        FunctionArg::Scalar(val) => Ok(vec![val.coerce_to_number()?]),
        FunctionArg::Array { values, .. } => {
            let mut result = Vec::with_capacity(values.len());
            for v in values {
                result.push(v.coerce_to_number()?);
            }
            Ok(result)
        }
        FunctionArg::Lazy(_) | FunctionArg::Omitted => Err(ExcelError::Value),
    }
}

/// Positional parameter definitions for `NPV`.
static NPV_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "rate",
        "Discount rate over the length of the period.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "value1",
        "First value, cell reference, or range of cash flows.",
        ParamKind::RangeOrArray,
    ),
];

/// `NPV(rate, value1, [value2], ...)`: Calculates net present value of periodic cash flows starting at t=1.
#[derive(Debug, Default, Clone, Copy)]
pub struct NpvFunction;

impl ExcelFunction for NpvFunction {
    fn name(&self) -> &'static str {
        "NPV"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Financial
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        NPV_PARAMS
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.len() < 2 {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        }

        let rate = match extract_scalar_number(&args[0]) {
            Ok(r) => r,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let cfs = match collect_cash_flows(&args[1..]) {
            Ok(c) => c,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if cfs.is_empty() {
            return Ok(CellValue::Number(0.0));
        }

        if rate == -1.0 {
            return Ok(CellValue::Error(ExcelError::Div0));
        }

        let mut npv = 0.0;
        for (idx, &cf) in cfs.iter().enumerate() {
            let period = (idx + 1) as i32;
            let denom = (1.0 + rate).powi(period);
            if denom == 0.0 {
                return Ok(CellValue::Error(ExcelError::Div0));
            }
            npv += cf / denom;
        }

        if npv.is_nan() || npv.is_infinite() {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        Ok(CellValue::Number(normalize_zero(npv)))
    }
}

/// Positional parameter definitions for `XNPV`.
static XNPV_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "rate",
        "Discount rate to apply to the cash flows.",
        ParamKind::Scalar,
    ),
    ParameterDef::required(
        "values",
        "Series of cash flows corresponding to schedule of payments in dates.",
        ParamKind::RangeOrArray,
    ),
    ParameterDef::required(
        "dates",
        "Schedule of payment dates corresponding to the cash flow payments.",
        ParamKind::RangeOrArray,
    ),
];

/// `XNPV(rate, values, dates)`: Calculates net present value of non-periodic cash flows using Actual/365 year fractions.
#[derive(Debug, Default, Clone, Copy)]
pub struct XnpvFunction;

impl ExcelFunction for XnpvFunction {
    fn name(&self) -> &'static str {
        "XNPV"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Financial
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        XNPV_PARAMS
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
        if args.len() > 3 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 3,
                found: args.len(),
            });
        }

        let rate = match extract_scalar_number(&args[0]) {
            Ok(r) => r,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if rate <= -1.0 {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        let values = match extract_vector_numbers(&args[1]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let dates = match extract_vector_numbers(&args[2]) {
            Ok(d) => d,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if values.len() != dates.len() || values.is_empty() {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        let start_date = dates[0].trunc();
        let mut xnpv = 0.0;

        for (v, d) in values.iter().zip(dates.iter()) {
            let d_trunc = d.trunc();
            if d_trunc < start_date {
                return Ok(CellValue::Error(ExcelError::Num));
            }
            let year_fraction = (d_trunc - start_date) / 365.0;
            let denom = (1.0 + rate).powf(year_fraction);
            if denom == 0.0 {
                return Ok(CellValue::Error(ExcelError::Div0));
            }
            xnpv += v / denom;
        }

        if xnpv.is_nan() || xnpv.is_infinite() {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        Ok(CellValue::Number(normalize_zero(xnpv)))
    }
}
