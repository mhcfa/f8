use crate::functions::common::normalize_zero;
use crate::functions::error::FunctionError;
use crate::functions::financial::discounting::{
    collect_cash_flows, extract_scalar_number, extract_vector_numbers,
};
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
    VariadicRule,
};
use crate::value::{CellValue, ExcelError};

const MAX_ITERATIONS: usize = 100;
const CONVERGENCE_TOLERANCE: f64 = 1e-9;
const DERIVATIVE_TOLERANCE: f64 = 1e-14;

/// Positional parameter definitions for `IRR`.
static IRR_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "values",
        "Array or reference to cells containing numbers for which you want to calculate IRR.",
        ParamKind::RangeOrArray,
    ),
    ParameterDef {
        name: "guess",
        description: "Number you guess is close to the result of IRR.",
        kind: ParamKind::Scalar,
        optional: true,
        default_value: Some(CellValue::Number(0.10)),
    },
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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        if args.is_empty() {
            return Err(FunctionError::TooFewArguments {
                function: self.name(),
                expected: 1,
                found: 0,
            });
        }
        if args.len() > 2 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 2,
                found: args.len(),
            });
        }

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

        let guess = if args.len() >= 2 && !matches!(args[1], FunctionArg::Omitted) {
            match extract_scalar_number(&args[1]) {
                Ok(g) => g,
                Err(e) => return Ok(CellValue::Error(e)),
            }
        } else {
            0.10
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

            if f_val.abs() < CONVERGENCE_TOLERANCE {
                return Ok(CellValue::Number(normalize_zero(rate)));
            }

            if f_deriv.abs() < DERIVATIVE_TOLERANCE {
                rate += 0.01;
                continue;
            }

            let delta = f_val / f_deriv;
            if delta.abs() < CONVERGENCE_TOLERANCE {
                return Ok(CellValue::Number(normalize_zero(rate - delta)));
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
        "Series of cash flows corresponding to schedule of payments in dates.",
        ParamKind::RangeOrArray,
    ),
    ParameterDef::required(
        "dates",
        "Schedule of payment dates corresponding to cash flow payments.",
        ParamKind::RangeOrArray,
    ),
    ParameterDef {
        name: "guess",
        description: "Number you guess is close to the result of XIRR.",
        kind: ParamKind::Scalar,
        optional: true,
        default_value: Some(CellValue::Number(0.10)),
    },
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

    fn variadic_rule(&self) -> VariadicRule {
        VariadicRule::None
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
        if args.len() > 3 {
            return Err(FunctionError::TooManyArguments {
                function: self.name(),
                expected: 3,
                found: args.len(),
            });
        }

        let values = match extract_vector_numbers(&args[0]) {
            Ok(v) => v,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        let dates = match extract_vector_numbers(&args[1]) {
            Ok(d) => d,
            Err(e) => return Ok(CellValue::Error(e)),
        };

        if values.len() != dates.len() || values.len() < 2 {
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

        let guess = if args.len() >= 3 && !matches!(args[2], FunctionArg::Omitted) {
            match extract_scalar_number(&args[2]) {
                Ok(g) => g,
                Err(e) => return Ok(CellValue::Error(e)),
            }
        } else {
            0.10
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

            if f_val.abs() < CONVERGENCE_TOLERANCE {
                return Ok(CellValue::Number(normalize_zero(rate)));
            }

            if f_deriv.abs() < DERIVATIVE_TOLERANCE {
                rate += 0.01;
                continue;
            }

            let delta = f_val / f_deriv;
            if delta.abs() < CONVERGENCE_TOLERANCE {
                return Ok(CellValue::Number(normalize_zero(rate - delta)));
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
