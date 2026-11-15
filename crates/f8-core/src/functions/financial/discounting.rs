use crate::functions::common::scalar_number;
use crate::functions::error::FunctionError;
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    ReferenceKind, ShapePolicy, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType};

/// Collects an ordered sequence of strict numeric cash flows across one or more validated arguments.
///
/// Errors bubble up immediately; non-numeric or empty cells are rejected by `validate_bound_args`
/// (`EmptyPolicy::ForbidAll`).
pub(crate) fn collect_cash_flows(args: &[FunctionArg<'_>]) -> Result<Vec<f64>, ExcelError> {
    let mut cfs = Vec::new();
    for arg in args {
        match arg {
            FunctionArg::Scalar(val) => match val.strict_number() {
                Ok(Ok(n)) => cfs.push(n),
                Ok(Err(e)) => return Err(e),
                Err(_) => return Err(ExcelError::Value),
            },
            FunctionArg::Range {
                values,
                ref_kind: ReferenceKind::SingleCell | ReferenceKind::Slice,
                ..
            } => {
                for v in values {
                    match v.strict_number() {
                        Ok(Ok(n)) => cfs.push(n),
                        Ok(Err(e)) => return Err(e),
                        Err(_) => return Err(ExcelError::Value),
                    }
                }
            }
            FunctionArg::Lazy(_) | FunctionArg::Omitted => {}
        }
    }
    Ok(cfs)
}

/// Extracts strict numeric entries from a validated 1D slice for `XNPV` and `XIRR`.
pub(crate) fn coerce_slice_numbers(slice: &[CellValue]) -> Result<Vec<f64>, ExcelError> {
    let mut result = Vec::with_capacity(slice.len());
    for v in slice {
        match v.strict_number() {
            Ok(Ok(n)) => result.push(n),
            Ok(Err(e)) => return Err(e),
            Err(_) => return Err(ExcelError::Value),
        }
    }
    Ok(result)
}

/// Positional parameter definitions for `NPV`.
static NPV_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "rate",
        "Discount rate over the length of the period.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::required(
        "value1",
        "First value, cell reference, or range of cash flows.",
        ParamKind::ScalarOrRange,
        ParamType::Number,
    )
    .measure_only(),
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

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::Single
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let rate = match scalar_number(&args[0]) {
            Ok(r) => r,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let cfs = match collect_cash_flows(&args[1..]) {
            Ok(c) => c,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if cfs.is_empty() {
            return Ok(CellValue::number(0.0));
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

        Ok(CellValue::number(npv))
    }
}

/// Positional parameter definitions for `XNPV`.
static XNPV_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "rate",
        "Discount rate to apply to the cash flows.",
        ParamKind::Scalar,
        ParamType::Number,
    ),
    ParameterDef::required(
        "values",
        "1D range of cash flows corresponding to schedule of payments in dates.",
        ParamKind::Range,
        ParamType::Number,
    )
    .measure_only(),
    ParameterDef::required(
        "dates",
        "1D range of payment dates corresponding to the cash flow payments.",
        ParamKind::Range,
        ParamType::DateSerial,
    )
    .measure_only(),
];

/// `XNPV(rate, values, dates)`: Calculates net present value of non-periodic cash flows using Actual/365 year fractions.
///
/// Rejects `rate <= 0.0` with runtime `#NUM!` (`ExcelError::Num`) for exact recalculation parity with emitted
/// `=XNPV(...)` formulas: although `-1.0 < rate <= 0.0` is mathematically well-defined (`1.0 + rate > 0.0`),
/// Excel's legacy `XNPV` implementation rejects non-positive discount rates with `#NUM!`.
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

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::ShapeCoupled(ShapePolicy::OneDimEqualLength {
            first_idx: 1,
            first_param: "values",
            second_idx: 2,
            second_param: "dates",
        })
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let raw_values = args[1].as_range_slice(self.name())?;
        let raw_dates = args[2].as_range_slice(self.name())?;

        let rate = match scalar_number(&args[0]) {
            Ok(r) => r,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        // Excel quirk: XNPV rejects all non-positive rates (including 0.0 and -1.0 < rate < 0.0) with #NUM!.
        if rate <= 0.0 {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        let values = match coerce_slice_numbers(raw_values) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let dates = match coerce_slice_numbers(raw_dates) {
            Ok(d) => d,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if values.is_empty() {
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
            xnpv += v / denom;
        }

        Ok(CellValue::number(xnpv))
    }
}
