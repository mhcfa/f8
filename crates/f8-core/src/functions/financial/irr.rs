use crate::functions::common::extract_scalar_number;
use crate::functions::error::FunctionError;
use crate::functions::financial::discounting::{
    coerce_slice_numbers, collect_cash_flows, extract_1d_range_values,
};
use crate::functions::registry::validate_bound_args;
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    ShapePolicy, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType};

const MAX_ITERATIONS: usize = 100;
const CONVERGENCE_TOLERANCE: f64 = 1e-9;
const DERIVATIVE_TOLERANCE: f64 = 1e-14;

/// Positional parameter definitions for `IRR`.
static IRR_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "values",
        "Range of cells containing numbers for which you want to calculate IRR.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::with_default(
        "guess",
        "Number you guess is close to the result of IRR.",
        ParamKind::Scalar,
        ParamType::Number,
        CellValue::Number(0.10),
    ),
];

/// `IRR(values, [guess])`: Calculates the internal rate of return for a series of periodic cash flows.
#[derive(Debug, Default, Clone, Copy)]
pub struct IrrFunction;

impl ExcelFunction for IrrFunction {
    fn name(&self) -> &'static str {
        "IRR"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Financial
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        IRR_PARAMS
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

        let guess_res = match args.get(1) {
            None | Some(FunctionArg::Omitted) => Ok(0.10),
            Some(
                guess_arg @ (FunctionArg::Scalar(_)
                | FunctionArg::TypedErrorScalar { .. }
                | FunctionArg::Array { .. }
                | FunctionArg::Lazy(_)),
            ) => extract_scalar_number(guess_arg, self.name(), "guess", 2, ctx)?,
        };

        let cfs = match collect_cash_flows(&args[0..1]) {
            Ok(c) => c,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if cfs.len() < 2 {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        let mut has_pos = false;
        let mut has_neg = false;
        for &v in &cfs {
            if v > 0.0 {
                has_pos = true;
            } else if v < 0.0 {
                has_neg = true;
            }
        }
        if !has_pos || !has_neg {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        let guess = match guess_res {
            Ok(g) => g,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let mut rate = guess;

        for _ in 0..MAX_ITERATIONS {
            if rate <= -0.999999 {
                rate = -0.999999;
            }

            let mut f_val = 0.0;
            let mut f_deriv = 0.0;

            for (idx, &cf) in cfs.iter().enumerate() {
                let t = idx as i32;
                let denom = (1.0 + rate).powi(t);
                if denom == 0.0 {
                    return Ok(CellValue::Error(ExcelError::Div0));
                }
                f_val += cf / denom;
                if idx > 0 {
                    let deriv_denom = (1.0 + rate).powi(t + 1);
                    if deriv_denom != 0.0 {
                        f_deriv -= (t as f64) * cf / deriv_denom;
                    }
                }
            }

            if f_val == 0.0 {
                return Ok(CellValue::number(rate));
            }

            if f_deriv.abs() < DERIVATIVE_TOLERANCE {
                rate += 0.01;
                continue;
            }

            let delta = f_val / f_deriv;
            if delta.abs() < CONVERGENCE_TOLERANCE {
                return Ok(CellValue::number(rate - delta));
            }

            let next_rate = rate - delta;
            if next_rate <= -1.0 {
                rate = (rate - 1.0) / 2.0;
            } else {
                rate = next_rate;
            }
        }

        Ok(CellValue::Error(ExcelError::Num))
    }
}

/// Positional parameter definitions for `XIRR`.
static XIRR_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "values",
        "1D range of cash flows corresponding to schedule of payments in dates.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::required(
        "dates",
        "1D range of payment dates corresponding to cash flow payments.",
        ParamKind::Range,
        ParamType::Any,
    ),
    ParameterDef::with_default(
        "guess",
        "Number you guess is close to the result of XIRR.",
        ParamKind::Scalar,
        ParamType::Number,
        CellValue::Number(0.10),
    ),
];

/// `XIRR(values, dates, [guess])`: Calculates internal rate of return for a schedule of cash flows that is not necessarily periodic.
#[derive(Debug, Default, Clone, Copy)]
pub struct XirrFunction;

impl ExcelFunction for XirrFunction {
    fn name(&self) -> &'static str {
        "XIRR"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Financial
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        XIRR_PARAMS
    }

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
    }

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn validation_policy(&self) -> ValidationPolicy {
        ValidationPolicy::ShapeCoupled(ShapePolicy::OneDimEqualLength {
            first_idx: 0,
            first_param: "values",
            second_idx: 1,
            second_param: "dates",
        })
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        validate_bound_args(self, args)?;

        let raw_values = extract_1d_range_values(&args[0], self.name(), "values")?;
        let raw_dates = extract_1d_range_values(&args[1], self.name(), "dates")?;

        if raw_values.len() != raw_dates.len() {
            return Err(FunctionError::LengthMismatch {
                function: self.name(),
                expected: raw_values.len(),
                found: raw_dates.len(),
            });
        }

        let guess_res = match args.get(2) {
            None | Some(FunctionArg::Omitted) => Ok(0.10),
            Some(
                guess_arg @ (FunctionArg::Scalar(_)
                | FunctionArg::TypedErrorScalar { .. }
                | FunctionArg::Array { .. }
                | FunctionArg::Lazy(_)),
            ) => extract_scalar_number(guess_arg, self.name(), "guess", 3, ctx)?,
        };

        let values = match coerce_slice_numbers(raw_values) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let dates = match coerce_slice_numbers(raw_dates) {
            Ok(d) => d,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if values.len() < 2 {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        let start_date = dates[0].trunc();
        let mut has_pos = false;
        let mut has_neg = false;

        for (v, d) in values.iter().zip(dates.iter()) {
            if d.trunc() < start_date {
                return Ok(CellValue::Error(ExcelError::Num));
            }
            if *v > 0.0 {
                has_pos = true;
            } else if *v < 0.0 {
                has_neg = true;
            }
        }

        if !has_pos || !has_neg {
            return Ok(CellValue::Error(ExcelError::Num));
        }

        let guess = match guess_res {
            Ok(g) => g,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let year_fractions: Vec<f64> = dates
            .iter()
            .map(|d| (d.trunc() - start_date) / 365.0)
            .collect();
        let mut rate = guess;

        for _ in 0..MAX_ITERATIONS {
            if rate <= -0.999999 {
                rate = -0.999999;
            }

            let mut f_val = 0.0;
            let mut f_deriv = 0.0;

            for (&cf, &yf) in values.iter().zip(year_fractions.iter()) {
                let denom = (1.0 + rate).powf(yf);
                if denom == 0.0 {
                    return Ok(CellValue::Error(ExcelError::Div0));
                }
                f_val += cf / denom;
                if yf > 0.0 {
                    let deriv_denom = (1.0 + rate).powf(yf + 1.0);
                    if deriv_denom != 0.0 {
                        f_deriv -= yf * cf / deriv_denom;
                    }
                }
            }

            if f_val == 0.0 {
                return Ok(CellValue::number(rate));
            }

            if f_deriv.abs() < DERIVATIVE_TOLERANCE {
                rate += 0.01;
                continue;
            }

            let delta = f_val / f_deriv;
            if delta.abs() < CONVERGENCE_TOLERANCE {
                return Ok(CellValue::number(rate - delta));
            }

            let next_rate = rate - delta;
            if next_rate <= -1.0 {
                rate = (rate - 1.0) / 2.0;
            } else {
                rate = next_rate;
            }
        }

        Ok(CellValue::Error(ExcelError::Num))
    }
}
