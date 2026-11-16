use crate::functions::common::{require_args, scalar_number};
use crate::functions::error::FunctionError;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    ReferenceKind, ShapePolicy, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType, validate_date_serial};

/// Collects an ordered sequence of strict numeric cash flows across one or more validated arguments.
fn collect_cash_flows(args: &[FunctionArg<'_>]) -> Result<Vec<f64>, ExcelError> {
    let mut cfs = Vec::new();
    for arg in args {
        match arg {
            FunctionArg::Scalar(val) => cfs.push(val.strict_number()?),
            FunctionArg::Range {
                values,
                ref_kind: ReferenceKind::SingleCell | ReferenceKind::Slice,
                ..
            } => {
                for v in values {
                    cfs.push(v.strict_number()?);
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
        result.push(v.strict_number()?);
    }
    Ok(result)
}

/// Validates extracted numeric date entries as calendar date serials (`1.0..=2_958_465.0`).
pub(crate) fn validate_slice_date_serials(raw_dates: &[f64]) -> Result<Vec<f64>, ExcelError> {
    let mut result = Vec::with_capacity(raw_dates.len());
    for &d in raw_dates {
        result.push(validate_date_serial(d)?);
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
        let [rate_arg, cf_args @ ..] = args else {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        };
        if cf_args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        }
        let rate = match scalar_number(rate_arg) {
            Ok(r) => r,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let cfs = match collect_cash_flows(cf_args) {
            Ok(c) => c,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if cfs.is_empty() {
            return Ok(CellValue::number(0.0));
        }

        let one_plus_r = 1.0 + rate;
        if rate == -1.0 || one_plus_r == 0.0 {
            return Ok(CellValue::Error(ExcelError::Div0));
        }

        let mut npv = 0.0;
        for (idx, &cf) in cfs.iter().enumerate() {
            if cf == 0.0 {
                continue;
            }
            let period = (idx + 1) as i32;
            npv += cf / one_plus_r.powi(period);
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
        ParamType::Number,
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
        let [rate_arg, values_arg, dates_arg] = require_args(self, args)?;
        let (_, _, raw_values) = values_arg.as_1d_range(self.name(), "values")?;
        let (_, _, raw_dates) = dates_arg.as_1d_range(self.name(), "dates")?;
        if raw_values.len() != raw_dates.len() {
            return Err(FunctionError::LengthMismatch {
                function: self.name(),
                expected: raw_values.len(),
                found: raw_dates.len(),
            });
        }

        let eval = || -> Result<f64, ExcelError> {
            let rate = scalar_number(rate_arg)?;
            let values = coerce_slice_numbers(raw_values)?;
            let raw_date_nums = coerce_slice_numbers(raw_dates)?;
            let dates = validate_slice_date_serials(&raw_date_nums)?;

            // Excel quirk: XNPV rejects all non-positive rates (including 0.0 and -1.0 < rate < 0.0) with #NUM!.
            if rate <= 0.0 {
                return Err(ExcelError::Num);
            }

            let Some(&start_date) = dates.first() else {
                return Err(ExcelError::Num);
            };
            let mut xnpv = 0.0;

            for (v, d_trunc) in values.iter().zip(dates.iter().copied()) {
                if d_trunc < start_date {
                    return Err(ExcelError::Num);
                }
                let year_fraction = (d_trunc - start_date) / 365.0;
                let denom = (1.0 + rate).powf(year_fraction);
                xnpv += v / denom;
            }
            Ok(xnpv)
        };

        match eval() {
            Ok(xnpv) => Ok(CellValue::number(xnpv)),
            Err(e) => Ok(CellValue::Error(e)),
        }
    }
}
