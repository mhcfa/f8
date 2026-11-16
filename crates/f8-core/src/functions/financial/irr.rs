use crate::functions::common::{optional_scalar_number, require_args};
use crate::functions::error::FunctionError;
use crate::functions::financial::discounting::{coerce_slice_numbers, validate_slice_date_serials};
use crate::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
    ShapePolicy, ValidationPolicy, VariadicRule,
};
use crate::value::{CellValue, ExcelError, ValueType, normalize_zero};

const MAX_NEWTON_ITERS: usize = 64;
const MAX_BRENT_ITERS: usize = 160;
const STEP_TOLERANCE: f64 = 1e-11;
const RESIDUAL_TOLERANCE: f64 = 1e-11;
const MAX_UNBRACKETED_ULP_TOLERANCE: f64 = 1e-3;
const DERIVATIVE_TOLERANCE: f64 = 1e-14;
const BRENT_LOG_TOLERANCE: f64 = 1e-14;
const INITIAL_LOG_STEP: f64 = 1e-4;
const LOG_STEP_GROWTH: f64 = 1.25;
const MAX_LOG_STEP: f64 = 32.0;
const LOG_MIN: f64 = -36.73;
const LOG_MAX: f64 = 709.78;

/// Positional parameter definitions for `IRR`.
static IRR_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "values",
        "Range of cells containing numbers for which you want to calculate IRR.",
        ParamKind::Range,
        ParamType::Number,
    )
    .measure_only(),
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
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let [values_arg] = require_args(self, args)?;
        let raw_values = values_arg.as_range_slice(self.name())?;
        let eval = || {
            let cfs = coerce_slice_numbers(raw_values)?;
            let guess = optional_scalar_number(args, 1, 0.10)?;
            solve_periodic_irr(&cfs, guess)
        };
        match eval() {
            Ok(rate) => Ok(CellValue::number(rate)),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// Positional parameter definitions for `XIRR`.
static XIRR_PARAMS: &[ParameterDef] = &[
    ParameterDef::required(
        "values",
        "1D range of cash flows corresponding to schedule of payments in dates.",
        ParamKind::Range,
        ParamType::Number,
    )
    .measure_only(),
    ParameterDef::required(
        "dates",
        "1D range of payment dates corresponding to cash flow payments.",
        ParamKind::Range,
        ParamType::Number,
    )
    .measure_only(),
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
        _ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let [values_arg, dates_arg] = require_args(self, args)?;
        let (_, _, raw_values) = values_arg.as_1d_range(self.name(), "values")?;
        let (_, _, raw_dates) = dates_arg.as_1d_range(self.name(), "dates")?;
        if raw_values.len() != raw_dates.len() {
            return Err(FunctionError::LengthMismatch {
                function: self.name(),
                expected: raw_values.len(),
                found: raw_dates.len(),
            });
        }

        let eval = || {
            let values = coerce_slice_numbers(raw_values)?;
            let raw_date_nums = coerce_slice_numbers(raw_dates)?;
            let guess = optional_scalar_number(args, 2, 0.10)?;
            let dates = validate_slice_date_serials(&raw_date_nums)?;
            solve_scheduled_xirr(&values, &dates, guess)
        };

        match eval() {
            Ok(rate) => Ok(CellValue::number(rate)),
            Err(err) => Ok(CellValue::Error(err)),
        }
    }
}

/// Gauge-normalized objective evaluation at a candidate discount rate `rate > -1.0`.
#[derive(Debug, Clone, Copy)]
struct ObjectiveEval {
    /// Objective value preserving exact sign of NPV across `(-1.0, +inf)`.
    f_val: f64,
    /// Exact derivative of `f_val` with respect to `rate` when finite.
    f_deriv: Option<f64>,
    /// Condition sum `sum(|cf_i| * weight_i) > 0.0` under the same gauge factor as `f_val`.
    mag_sum: f64,
}

impl ObjectiveEval {
    fn normalized_residual(self) -> f64 {
        self.f_val.abs() / self.mag_sum
    }
}

/// Scales cash flows by an exact power of two so `max(|cf|)` lands in `[0.5, 1.0]`.
///
/// Power-of-two scaling preserves IEEE-754 mantissa bits verbatim while preventing overflow
/// and underflow for extreme cash-flow magnitudes (`1e-300` to `1e308`).
fn normalize_cash_flow_scale(values: &mut [f64]) {
    let max_abs = values.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()));
    if max_abs == 0.0 || !max_abs.is_finite() {
        return;
    }
    for _ in 0..2 {
        let cur_max = values.iter().fold(0.0_f64, |acc, v| acc.max(v.abs()));
        if (0.5..=1.0).contains(&cur_max) {
            break;
        }
        let exp = cur_max.log2().floor() as i32 + 1;
        let shift = (-exp).clamp(-1022, 1023);
        let factor = 2.0_f64.powi(shift);
        for v in values.iter_mut() {
            *v *= factor;
        }
    }
}

/// Validates domain invariants and solves periodic `IRR(values, guess)`.
pub(crate) fn solve_periodic_irr(cfs: &[f64], guess: f64) -> Result<f64, ExcelError> {
    if !guess.is_finite() || guess <= -1.0 || cfs.len() < 2 {
        return Err(ExcelError::Num);
    }

    let mut has_pos = false;
    let mut has_neg = false;
    let mut first_nonzero = None;
    let mut last_nonzero = 0;

    for (idx, &v) in cfs.iter().enumerate() {
        if !v.is_finite() {
            return Err(ExcelError::Num);
        }
        if v > 0.0 {
            has_pos = true;
        } else if v < 0.0 {
            has_neg = true;
        }
        if v != 0.0 {
            if first_nonzero.is_none() {
                first_nonzero = Some(idx);
            }
            last_nonzero = idx;
        }
    }

    if !has_pos || !has_neg {
        return Err(ExcelError::Num);
    }

    let Some(start_idx) = first_nonzero else {
        return Err(ExcelError::Num);
    };
    if start_idx >= last_nonzero {
        return Err(ExcelError::Num);
    }

    let mut trimmed = cfs[start_idx..=last_nonzero].to_vec();
    normalize_cash_flow_scale(&mut trimmed);
    let max_exponent = (trimmed.len() - 1) as f64;

    solve_irr_root(guess, max_exponent, |rate| {
        eval_periodic_npv(&trimmed, rate)
    })
}

/// Coalesces sorted `(day, cash_flow)` entries on identical integer days, returning `None` if
/// any same-day sum overflows `f64`.
fn try_coalesce_dated_cfs(dated_cfs: &[(f64, f64)]) -> Option<Vec<(f64, f64)>> {
    let mut coalesced: Vec<(f64, f64)> = Vec::with_capacity(dated_cfs.len());
    for &(day, cf) in dated_cfs {
        if let Some(last) = coalesced.last_mut()
            && last.0 == day
        {
            let sum = last.1 + cf;
            if !sum.is_finite() {
                return None;
            }
            last.1 = sum;
        } else {
            coalesced.push((day, cf));
        }
    }
    coalesced.retain(|&(_, cf)| cf != 0.0);
    Some(coalesced)
}

/// Validates domain invariants and solves scheduled `XIRR(values, dates, guess)`.
pub(crate) fn solve_scheduled_xirr(
    values: &[f64],
    dates: &[f64],
    guess: f64,
) -> Result<f64, ExcelError> {
    if !guess.is_finite() || guess <= -1.0 || values.len() < 2 || values.len() != dates.len() {
        return Err(ExcelError::Num);
    }

    let Some(&first_date) = dates.first() else {
        return Err(ExcelError::Num);
    };
    if !first_date.is_finite() {
        return Err(ExcelError::Num);
    }
    let start_date = first_date.trunc();

    let mut raw_has_pos = false;
    let mut raw_has_neg = false;
    let mut raw_dated: Vec<(f64, f64)> = Vec::with_capacity(values.len());

    for (&v, &d) in values.iter().zip(dates.iter()) {
        if !v.is_finite() || !d.is_finite() {
            return Err(ExcelError::Num);
        }
        let day = d.trunc();
        if day < start_date {
            return Err(ExcelError::Num);
        }
        if v > 0.0 {
            raw_has_pos = true;
            raw_dated.push((day, v));
        } else if v < 0.0 {
            raw_has_neg = true;
            raw_dated.push((day, v));
        }
    }

    if !raw_has_pos || !raw_has_neg {
        return Err(ExcelError::Num);
    }

    raw_dated.sort_by(|a, b| a.0.total_cmp(&b.0));

    let coalesced = match try_coalesce_dated_cfs(&raw_dated) {
        Some(c) => c,
        None => {
            let mut scaled_cfs: Vec<f64> = raw_dated.iter().map(|&(_, cf)| cf).collect();
            normalize_cash_flow_scale(&mut scaled_cfs);
            let scaled_dated: Vec<(f64, f64)> = raw_dated
                .iter()
                .zip(scaled_cfs)
                .filter_map(|(&(day, _), cf)| (cf != 0.0).then_some((day, cf)))
                .collect();
            try_coalesce_dated_cfs(&scaled_dated).ok_or(ExcelError::Num)?
        }
    };

    if coalesced.len() < 2 {
        return Err(ExcelError::Num);
    }

    let mut net_has_pos = false;
    let mut net_has_neg = false;
    let mut net_cfs = Vec::with_capacity(coalesced.len());
    for &(_, cf) in &coalesced {
        if cf > 0.0 {
            net_has_pos = true;
        } else if cf < 0.0 {
            net_has_neg = true;
        }
        net_cfs.push(cf);
    }
    if !net_has_pos || !net_has_neg {
        return Err(ExcelError::Num);
    }

    normalize_cash_flow_scale(&mut net_cfs);

    let Some(&(base_day, _)) = coalesced.first() else {
        return Err(ExcelError::Num);
    };
    let mut schedule = Vec::with_capacity(coalesced.len());
    for ((day, _), cf) in coalesced.into_iter().zip(net_cfs) {
        let yf = (day - base_day) / 365.0;
        if !yf.is_finite() {
            return Err(ExcelError::Num);
        }
        schedule.push((yf, cf));
    }

    let Some(&(last_yf, _)) = schedule.last() else {
        return Err(ExcelError::Num);
    };
    let max_exponent = last_yf.max(1.0);
    solve_irr_root(guess, max_exponent, |rate| {
        eval_scheduled_xnpv(&schedule, rate)
    })
}

/// Evaluates periodic NPV using overflow-free backward Horner (`rate >= 0.0`) or
/// `(1 + rate)^n`-rescaled forward Horner (`-1.0 < rate < 0.0`).
fn eval_periodic_npv(trimmed: &[f64], rate: f64) -> Option<ObjectiveEval> {
    if !rate.is_finite() || rate <= -1.0 {
        return None;
    }
    let one_plus_r = 1.0 + rate;
    if !one_plus_r.is_finite() || one_plus_r <= 0.0 {
        return None;
    }

    if rate >= 0.0 {
        let u = 1.0 / one_plus_r;
        let mut p = 0.0_f64;
        let mut dp_du = 0.0_f64;
        let mut mag = 0.0_f64;

        for &cf in trimmed.iter().rev() {
            dp_du = dp_du * u + p;
            p = p * u + cf;
            mag = mag * u + cf.abs();
        }

        if !p.is_finite() || !mag.is_finite() || mag <= 0.0 {
            return None;
        }
        let deriv_raw = (-dp_du * u) * u;
        Some(ObjectiveEval {
            f_val: p,
            f_deriv: deriv_raw.is_finite().then_some(deriv_raw),
            mag_sum: mag,
        })
    } else {
        let v = one_plus_r;
        let mut p = 0.0_f64;
        let mut dp_dv = 0.0_f64;
        let mut mag = 0.0_f64;

        for &cf in trimmed {
            dp_dv = dp_dv * v + p;
            p = p * v + cf;
            mag = mag * v + cf.abs();
        }

        if !p.is_finite() || !mag.is_finite() || mag <= 0.0 {
            return None;
        }
        Some(ObjectiveEval {
            f_val: p,
            f_deriv: dp_dv.is_finite().then_some(dp_dv),
            mag_sum: mag,
        })
    }
}

/// Evaluates scheduled XNPV with non-positive exponents across `(-1.0, +inf)`.
fn eval_scheduled_xnpv(schedule: &[(f64, f64)], rate: f64) -> Option<ObjectiveEval> {
    if !rate.is_finite() || rate <= -1.0 {
        return None;
    }
    let one_plus_r = 1.0 + rate;
    if !one_plus_r.is_finite() || one_plus_r <= 0.0 {
        return None;
    }
    let ln_1p_r = rate.ln_1p();
    if !ln_1p_r.is_finite() {
        return None;
    }

    if ln_1p_r >= 0.0 {
        let &[(_, first_cf), ref rest @ ..] = schedule else {
            return None;
        };
        let mut f_val = first_cf;
        let mut mag_sum = first_cf.abs();
        let mut deriv_sum = 0.0_f64;

        for &(yf, cf) in rest {
            let disc = (-yf * ln_1p_r).exp();
            if !disc.is_finite() {
                return None;
            }
            let term = cf * disc;
            f_val += term;
            mag_sum += term.abs();
            deriv_sum -= yf * term;
            if !f_val.is_finite() || !mag_sum.is_finite() {
                return None;
            }
        }

        if mag_sum <= 0.0 {
            return None;
        }
        let deriv_raw = deriv_sum / one_plus_r;
        Some(ObjectiveEval {
            f_val,
            f_deriv: deriv_raw.is_finite().then_some(deriv_raw),
            mag_sum,
        })
    } else {
        let &(yf_max, _) = schedule.last()?;
        let mut f_val = 0.0_f64;
        let mut mag_sum = 0.0_f64;
        let mut deriv_sum = 0.0_f64;

        for &(yf, cf) in schedule {
            let rem = yf_max - yf;
            let disc = (rem * ln_1p_r).exp();
            if !disc.is_finite() {
                return None;
            }
            let term = cf * disc;
            f_val += term;
            mag_sum += term.abs();
            if rem > 0.0 {
                deriv_sum += rem * term;
            }
            if !f_val.is_finite() || !mag_sum.is_finite() {
                return None;
            }
        }

        if mag_sum <= 0.0 {
            return None;
        }
        let deriv_raw = deriv_sum / one_plus_r;
        Some(ObjectiveEval {
            f_val,
            f_deriv: deriv_raw.is_finite().then_some(deriv_raw),
            mag_sum,
        })
    }
}

fn rate_from_log(x: f64) -> Option<f64> {
    if !x.is_finite() {
        return None;
    }
    let r = x.exp_m1();
    (r.is_finite() && r > -1.0).then_some(r)
}

fn effective_residual_tolerance(rate: f64, max_exponent: f64) -> f64 {
    let log_mag = rate.ln_1p().abs().max(1.0);
    RESIDUAL_TOLERANCE.max(max_exponent * 16.0 * f64::EPSILON * log_mag)
}

/// Returns `Some(df)` when `ev.f_deriv` is finite and non-degenerate relative to the
/// dimensionless log-rate slope scale `ev.mag_sum / (1 + rate)`.
fn has_usable_derivative(rate: f64, ev: ObjectiveEval) -> Option<f64> {
    let df = ev.f_deriv?;
    let log_slope = df.abs() * (1.0 + rate);
    (df.is_finite() && log_slope >= DERIVATIVE_TOLERANCE * ev.mag_sum).then_some(df)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RootContext {
    Unbracketed,
    ConvergedBracket,
}

/// Checks whether `ev.normalized_residual()` lies within floating-point root tolerance,
/// accounting for `1 + rate` ULP cancellation when `rate` approaches `-1.0`.
fn residual_within_tolerance(
    rate: f64,
    ev: ObjectiveEval,
    max_exponent: f64,
    context: RootContext,
) -> bool {
    let base_tol = effective_residual_tolerance(rate, max_exponent);
    let rate_ulp = f64::EPSILON * rate.abs().max(1.0);
    let raw_ulp_tol = ev
        .f_deriv
        .map_or(0.0, |df| (df.abs() * 16.0 * rate_ulp) / ev.mag_sum);
    let ulp_tol = match context {
        RootContext::ConvergedBracket => raw_ulp_tol,
        RootContext::Unbracketed => raw_ulp_tol.min(MAX_UNBRACKETED_ULP_TOLERANCE),
    };
    ev.normalized_residual() <= base_tol.max(ulp_tol)
}

/// Applies at most one safeguarded rate-space Newton polish step when it stays within `bounds`
/// (and within `max_step` when specified) and strictly reduces `normalized_residual()`.
fn polish_root_candidate<F>(
    r_cand: f64,
    ev_cand: ObjectiveEval,
    bounds: Option<(f64, f64)>,
    max_step: Option<f64>,
    eval: &F,
) -> (f64, ObjectiveEval)
where
    F: Fn(f64) -> Option<ObjectiveEval>,
{
    if ev_cand.f_val == 0.0 {
        return (r_cand, ev_cand);
    }
    let Some(df) = has_usable_derivative(r_cand, ev_cand) else {
        return (r_cand, ev_cand);
    };
    let step = ev_cand.f_val / df;
    if !step.is_finite() {
        return (r_cand, ev_cand);
    }
    if let Some(limit) = max_step
        && step.abs() > limit
    {
        return (r_cand, ev_cand);
    }
    let r_polish = r_cand - step;
    if !r_polish.is_finite() || r_polish <= -1.0 {
        return (r_cand, ev_cand);
    }
    if let Some((lo, hi)) = bounds
        && (r_polish < lo || r_polish > hi)
    {
        return (r_cand, ev_cand);
    }
    if let Some(ev_polish) = eval(r_polish)
        && ev_polish.normalized_residual() < ev_cand.normalized_residual()
    {
        return (r_polish, ev_polish);
    }
    (r_cand, ev_cand)
}

fn has_opposite_sign(a: f64, b: f64) -> bool {
    (a > 0.0 && b < 0.0) || (a < 0.0 && b > 0.0)
}

fn update_tightest_bracket(
    bracket: &mut Option<(f64, f64, f64, f64)>,
    prev_r: f64,
    prev_f: f64,
    curr_r: f64,
    curr_f: f64,
) {
    if !has_opposite_sign(prev_f, curr_f) || prev_r == curr_r {
        return;
    }
    let (lo, f_lo, hi, f_hi) = if prev_r < curr_r {
        (prev_r, prev_f, curr_r, curr_f)
    } else {
        (curr_r, curr_f, prev_r, prev_f)
    };
    let should_replace = match *bracket {
        None => true,
        Some((old_lo, _, old_hi, _)) => (hi - lo) < (old_hi - old_lo),
    };
    if should_replace {
        *bracket = Some((lo, f_lo, hi, f_hi));
    }
}

/// Unified root finder for `IRR` and `XIRR`:
/// 1. Safeguarded Newton-Raphson from `guess` (preserving local multi-root `guess` affinity).
/// 2. Local outward bracket expansion in `x = ln(1 + rate)` around `ln(1 + guess)` plus Brent's method.
fn solve_irr_root<F>(guess: f64, max_exponent: f64, eval: F) -> Result<f64, ExcelError>
where
    F: Fn(f64) -> Option<ObjectiveEval>,
{
    let mut rate = guess;
    let mut bracket: Option<(f64, f64, f64, f64)> = None;
    let mut prev_sample: Option<(f64, f64)> = None;

    for _ in 0..MAX_NEWTON_ITERS {
        let Some(ev) = eval(rate) else {
            break;
        };

        if ev.f_val == 0.0 {
            return Ok(normalize_zero(rate));
        }

        if let Some((prev_r, prev_f)) = prev_sample {
            update_tightest_bracket(&mut bracket, prev_r, prev_f, rate, ev.f_val);
        }
        prev_sample = Some((rate, ev.f_val));

        let Some(df) = has_usable_derivative(rate, ev) else {
            break;
        };

        let delta = ev.f_val / df;
        if !delta.is_finite() {
            break;
        }

        let next_rate = rate - delta;
        if !next_rate.is_finite() || next_rate <= -1.0 {
            break;
        }

        if let Some((lo, f_lo, hi, f_hi)) = bracket
            && !(next_rate > lo && next_rate < hi)
        {
            return solve_brent_from_rates(lo, f_lo, hi, f_hi, max_exponent, &eval);
        }

        let step_limit = (STEP_TOLERANCE * (1.0 + next_rate)).max(4.0 * f64::EPSILON);
        if delta.abs() <= step_limit
            && let Some(ev_next) = eval(next_rate)
        {
            if ev_next.f_val == 0.0 {
                return Ok(normalize_zero(next_rate));
            }
            update_tightest_bracket(&mut bracket, rate, ev.f_val, next_rate, ev_next.f_val);

            let bounds = bracket.map(|(lo, _, hi, _)| (lo, hi));
            let (best_rate, ev_best) =
                polish_root_candidate(next_rate, ev_next, bounds, Some(step_limit), &eval);
            if residual_within_tolerance(best_rate, ev_best, max_exponent, RootContext::Unbracketed)
            {
                return Ok(normalize_zero(best_rate));
            }
        }

        rate = next_rate;
    }

    if let Some((lo, f_lo, hi, f_hi)) = bracket {
        return solve_brent_from_rates(lo, f_lo, hi, f_hi, max_exponent, &eval);
    }

    expand_and_solve_from_guess(guess, max_exponent, &eval)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrontierDirection {
    Left,
    Right,
}

enum FrontierStep {
    Advanced,
    ExactRoot(f64),
    Bracket {
        r_lo: f64,
        f_lo: f64,
        r_hi: f64,
        f_hi: f64,
    },
}

struct FrontierArm {
    x: f64,
    r: f64,
    f: f64,
    step: f64,
    bound: f64,
    dir: FrontierDirection,
    active: bool,
}

impl FrontierArm {
    fn new(x0: f64, r0: f64, f0: f64, bound: f64, dir: FrontierDirection) -> Self {
        let active = match dir {
            FrontierDirection::Left => x0 > bound,
            FrontierDirection::Right => x0 < bound,
        };
        Self {
            x: x0,
            r: r0,
            f: f0,
            step: INITIAL_LOG_STEP,
            bound,
            dir,
            active,
        }
    }

    fn distance_from(&self, x0: f64) -> f64 {
        (self.x - x0).abs()
    }

    fn step_once<F>(&mut self, eval: &F) -> FrontierStep
    where
        F: Fn(f64) -> Option<ObjectiveEval>,
    {
        if !self.active {
            return FrontierStep::Advanced;
        }
        let local_cap = 0.02 + 0.10 * self.x.abs();
        let dx = self.step.min(local_cap);
        self.step = (self.step * LOG_STEP_GROWTH).min(MAX_LOG_STEP);

        let (next_x, progressed, hit_bound) = match self.dir {
            FrontierDirection::Left => {
                let nx = (self.x - dx).max(self.bound);
                (nx, nx < self.x, nx <= self.bound)
            }
            FrontierDirection::Right => {
                let nx = (self.x + dx).min(self.bound);
                (nx, nx > self.x, nx >= self.bound)
            }
        };

        if !progressed {
            self.active = false;
            return FrontierStep::Advanced;
        }
        if hit_bound {
            self.active = false;
        }

        let Some(next_r) = rate_from_log(next_x) else {
            self.active = false;
            return FrontierStep::Advanced;
        };
        let Some(ev_next) = eval(next_r) else {
            self.active = false;
            return FrontierStep::Advanced;
        };
        if ev_next.f_val == 0.0 {
            self.active = false;
            return FrontierStep::ExactRoot(next_r);
        }

        if has_opposite_sign(self.f, ev_next.f_val) {
            self.active = false;
            let (r_lo, f_lo, r_hi, f_hi) = if next_r < self.r {
                (next_r, ev_next.f_val, self.r, self.f)
            } else {
                (self.r, self.f, next_r, ev_next.f_val)
            };
            return FrontierStep::Bracket {
                r_lo,
                f_lo,
                r_hi,
                f_hi,
            };
        }

        self.x = next_x;
        self.r = next_r;
        self.f = ev_next.f_val;
        FrontierStep::Advanced
    }
}

/// Expands symmetrically outward from `x0 = ln(1 + guess)` in log-space, advancing whichever
/// active frontier is closer to `x0` while decelerating smoothly near `x = 0` so narrow
/// multi-root intervals are never skipped even from extreme guesses.
fn expand_and_solve_from_guess<F>(
    guess: f64,
    max_exponent: f64,
    eval: &F,
) -> Result<f64, ExcelError>
where
    F: Fn(f64) -> Option<ObjectiveEval>,
{
    let x0 = guess.ln_1p();
    if !x0.is_finite() {
        return Err(ExcelError::Num);
    }

    let Some(ev0) = eval(guess) else {
        return Err(ExcelError::Num);
    };
    if ev0.f_val == 0.0 {
        return Ok(normalize_zero(guess));
    }

    let mut left = FrontierArm::new(
        x0,
        guess,
        ev0.f_val,
        LOG_MIN.min(x0),
        FrontierDirection::Left,
    );
    let mut right = FrontierArm::new(
        x0,
        guess,
        ev0.f_val,
        LOG_MAX.max(x0),
        FrontierDirection::Right,
    );

    while left.active || right.active {
        let step_left = if left.active && right.active {
            left.distance_from(x0) <= right.distance_from(x0)
        } else {
            left.active
        };

        let (primary, secondary) = if step_left {
            (&mut left, &mut right)
        } else {
            (&mut right, &mut left)
        };

        let found_root = match primary.step_once(eval) {
            FrontierStep::Advanced => continue,
            FrontierStep::ExactRoot(r) => r,
            FrontierStep::Bracket {
                r_lo,
                f_lo,
                r_hi,
                f_hi,
            } => solve_brent_from_rates(r_lo, f_lo, r_hi, f_hi, max_exponent, eval)?,
        };

        let found_dist = (found_root.ln_1p() - x0).abs();
        while secondary.active && secondary.distance_from(x0) < found_dist {
            match secondary.step_once(eval) {
                FrontierStep::Advanced => {}
                FrontierStep::ExactRoot(alt_r) => {
                    if (alt_r.ln_1p() - x0).abs() < found_dist {
                        return Ok(normalize_zero(alt_r));
                    }
                    break;
                }
                FrontierStep::Bracket {
                    r_lo,
                    f_lo,
                    r_hi,
                    f_hi,
                } => {
                    if let Ok(alt_r) =
                        solve_brent_from_rates(r_lo, f_lo, r_hi, f_hi, max_exponent, eval)
                        && (alt_r.ln_1p() - x0).abs() < found_dist
                    {
                        return Ok(normalize_zero(alt_r));
                    }
                    break;
                }
            }
        }

        return Ok(normalize_zero(found_root));
    }

    Err(ExcelError::Num)
}

/// Solves a sign-change bracket `[r_lo, r_hi]` using Brent's method in `x = ln(1 + r)` space
/// followed by a safeguarded rate-space Newton polish step.
fn solve_brent_from_rates<F>(
    r_lo: f64,
    f_lo_orig: f64,
    r_hi: f64,
    f_hi_orig: f64,
    max_exponent: f64,
    eval: &F,
) -> Result<f64, ExcelError>
where
    F: Fn(f64) -> Option<ObjectiveEval>,
{
    if f_lo_orig == 0.0 {
        return Ok(normalize_zero(r_lo));
    }
    if f_hi_orig == 0.0 {
        return Ok(normalize_zero(r_hi));
    }
    if !has_opposite_sign(f_lo_orig, f_hi_orig) {
        return Err(ExcelError::Num);
    }

    let x_lo = r_lo.ln_1p();
    let x_hi = r_hi.ln_1p();
    if !x_lo.is_finite() || !x_hi.is_finite() {
        return Err(ExcelError::Num);
    }

    let bound_lo = r_lo.min(r_hi);
    let bound_hi = r_lo.max(r_hi);

    // Handle 1-ULP round-trip boundary shifts between `r` and `r.ln_1p().exp_m1()`.
    let (mut a, mut fa) = match rate_from_log(x_lo).and_then(eval) {
        Some(ev) if ev.f_val == 0.0 => return Ok(normalize_zero(r_lo)),
        Some(ev) if has_opposite_sign(ev.f_val, f_lo_orig) => {
            return finalize_root_candidate(
                r_lo,
                bound_lo,
                bound_hi,
                max_exponent,
                RootContext::ConvergedBracket,
                eval,
            );
        }
        Some(ev) => (x_lo, ev.f_val),
        None => (x_lo, f_lo_orig),
    };

    let (mut b, mut fb) = match rate_from_log(x_hi).and_then(eval) {
        Some(ev) if ev.f_val == 0.0 => return Ok(normalize_zero(r_hi)),
        Some(ev) if has_opposite_sign(ev.f_val, f_hi_orig) => {
            return finalize_root_candidate(
                r_hi,
                bound_lo,
                bound_hi,
                max_exponent,
                RootContext::ConvergedBracket,
                eval,
            );
        }
        Some(ev) => (x_hi, ev.f_val),
        None => (x_hi, f_hi_orig),
    };

    if !has_opposite_sign(fa, fb) {
        return Err(ExcelError::Num);
    }

    let mut c = a;
    let mut fc = fa;
    let mut d = b - a;
    let mut e = d;
    let mut context = RootContext::Unbracketed;

    for _ in 0..MAX_BRENT_ITERS {
        if (fb > 0.0) == (fc > 0.0) {
            c = a;
            fc = fa;
            d = b - a;
            e = d;
        }

        if fc.abs() < fb.abs() {
            a = b;
            b = c;
            c = a;
            fa = fb;
            fb = fc;
            fc = fa;
        }

        let r_b = b.exp_m1();
        let dx_ulp = (f64::EPSILON * r_b.abs().max(0.5)) / (1.0 + r_b);
        let tol = 2.0 * f64::EPSILON * b.abs() + 0.5 * BRENT_LOG_TOLERANCE.max(dx_ulp);
        let m = 0.5 * (c - b);
        if m.abs() <= tol || fb == 0.0 {
            context = RootContext::ConvergedBracket;
            break;
        }

        if e.abs() >= tol && fa.abs() > fb.abs() {
            let s = fb / fa;
            let (mut p, mut q) = if (a - c).abs() <= f64::EPSILON {
                (2.0 * m * s, 1.0 - s)
            } else {
                let q_iqi = fa / fc;
                let r_iqi = fb / fc;
                (
                    s * (2.0 * m * q_iqi * (q_iqi - r_iqi) - (b - a) * (r_iqi - 1.0)),
                    (q_iqi - 1.0) * (r_iqi - 1.0) * (s - 1.0),
                )
            };

            if p > 0.0 {
                q = -q;
            } else {
                p = -p;
            }

            let bound_m = 3.0 * m * q - (tol * q).abs();
            let bound_e = (e * q).abs();
            if 2.0 * p < bound_m.min(bound_e) {
                e = d;
                d = p / q;
            } else {
                d = m;
                e = m;
            }
        } else {
            d = m;
            e = m;
        }

        a = b;
        fa = fb;
        if d.abs() > tol {
            b += d;
        } else if m > 0.0 {
            b += tol;
        } else {
            b -= tol;
        }

        let Some(r_b) = rate_from_log(b) else {
            return Err(ExcelError::Num);
        };
        let Some(ev_b) = eval(r_b) else {
            return Err(ExcelError::Num);
        };
        fb = ev_b.f_val;
    }

    let r_cand = rate_from_log(b).unwrap_or(if f_lo_orig.abs() <= f_hi_orig.abs() {
        r_lo
    } else {
        r_hi
    });
    finalize_root_candidate(r_cand, bound_lo, bound_hi, max_exponent, context, eval)
}

fn finalize_root_candidate<F>(
    r_cand: f64,
    bound_lo: f64,
    bound_hi: f64,
    max_exponent: f64,
    context: RootContext,
    eval: &F,
) -> Result<f64, ExcelError>
where
    F: Fn(f64) -> Option<ObjectiveEval>,
{
    let r_clamped = r_cand.clamp(bound_lo, bound_hi);
    let Some(ev_cand) = eval(r_clamped) else {
        return Err(ExcelError::Num);
    };
    let (best_r, ev_best) =
        polish_root_candidate(r_clamped, ev_cand, Some((bound_lo, bound_hi)), None, eval);

    if residual_within_tolerance(best_r, ev_best, max_exponent, context) {
        Ok(normalize_zero(best_r))
    } else {
        Err(ExcelError::Num)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periodic_irr_rejects_invalid_guesses_and_non_finite_inputs() {
        let cfs = [-100.0, 110.0];
        assert_eq!(solve_periodic_irr(&cfs, -1.0), Err(ExcelError::Num));
        assert_eq!(solve_periodic_irr(&cfs, -1.5), Err(ExcelError::Num));
        assert_eq!(solve_periodic_irr(&cfs, -100.0), Err(ExcelError::Num));
        assert_eq!(solve_periodic_irr(&cfs, f64::NAN), Err(ExcelError::Num));
        assert_eq!(
            solve_periodic_irr(&cfs, f64::INFINITY),
            Err(ExcelError::Num)
        );
        assert_eq!(
            solve_periodic_irr(&cfs, f64::NEG_INFINITY),
            Err(ExcelError::Num)
        );
        assert_eq!(
            solve_periodic_irr(&[f64::NAN, 110.0], 0.10),
            Err(ExcelError::Num)
        );
    }

    #[test]
    fn scheduled_xirr_rejects_invalid_guesses_and_non_finite_inputs() {
        let cfs = [-100.0, 110.0];
        let dates = [45292.0, 45657.0];
        assert_eq!(
            solve_scheduled_xirr(&cfs, &dates, -1.0),
            Err(ExcelError::Num)
        );
        assert_eq!(
            solve_scheduled_xirr(&cfs, &dates, -1.5),
            Err(ExcelError::Num)
        );
        assert_eq!(
            solve_scheduled_xirr(&cfs, &dates, f64::NAN),
            Err(ExcelError::Num)
        );
        assert_eq!(
            solve_scheduled_xirr(&cfs, &dates, f64::INFINITY),
            Err(ExcelError::Num)
        );
        assert_eq!(
            solve_scheduled_xirr(&[f64::INFINITY, 110.0], &dates, 0.10),
            Err(ExcelError::Num)
        );
        assert_eq!(
            solve_scheduled_xirr(&cfs, &[45292.0, f64::NAN], 0.10),
            Err(ExcelError::Num)
        );
    }

    #[test]
    fn periodic_irr_solves_30_year_and_100_year_monthly_schedules() -> Result<(), ExcelError> {
        // 30-year monthly mortgage: -100,000 followed by 360 monthly inflows of 600.
        let mut mortgage_30y = vec![-100_000.0];
        mortgage_30y.extend(std::iter::repeat_n(600.0, 360));
        let r_30y = solve_periodic_irr(&mortgage_30y, 0.10)?;
        assert!((r_30y - 0.005_005_825_006_762_407).abs() < 1e-13);

        // Extreme negative initial guess near -1 recovers cleanly without overflow.
        let r_30y_neg_guess = solve_periodic_irr(&mortgage_30y, -0.999_999)?;
        assert!((r_30y_neg_guess - r_30y).abs() < 1e-13);

        // Negative-return 30-year schedule: -100,000 followed by 360 inflows of 200.
        let mut neg_30y = vec![-100_000.0];
        neg_30y.extend(std::iter::repeat_n(200.0, 360));
        let r_neg_30y = solve_periodic_irr(&neg_30y, 0.10)?;
        assert!(r_neg_30y < 0.0 && r_neg_30y > -1.0);
        let check_npv: f64 = neg_30y
            .iter()
            .enumerate()
            .map(|(t, &cf)| cf * (1.0 + r_neg_30y).powi(-(t as i32)))
            .sum();
        assert!(check_npv.abs() < 1e-6);

        // 100-year (1200-month) schedule where (1 - 0.45)^-1200 overflows f64::MAX.
        let mut schedule_100y = vec![-100_000.0];
        schedule_100y.extend(std::iter::repeat_n(500.0, 1200));
        let r_100y = solve_periodic_irr(&schedule_100y, 0.10)?;
        let check_100y: f64 = schedule_100y
            .iter()
            .enumerate()
            .map(|(t, &cf)| cf * (1.0 + r_100y).powi(-(t as i32)))
            .sum();
        assert!(check_100y.abs() < 1e-6);
        Ok(())
    }

    #[test]
    fn leading_and_trailing_zeros_resist_overflow_false_roots() -> Result<(), ExcelError> {
        let cfs = [0.0, 0.0, -100.0, 110.0, 0.0];
        let r_default = solve_periodic_irr(&cfs, 0.10)?;
        assert!((r_default - 0.10).abs() < 1e-14);

        let r_huge_guess = solve_periodic_irr(&cfs, 1e160)?;
        assert!((r_huge_guess - 0.10).abs() < 1e-13);

        let dates = [45292.0, 45657.0, 46022.0, 46387.0, 46752.0];
        let xirr_huge_guess = solve_scheduled_xirr(&cfs, &dates, 1e160)?;
        assert!((xirr_huge_guess - 0.10).abs() < 1e-13);
        Ok(())
    }

    #[test]
    fn multi_root_cash_flows_preserve_guess_affinity() -> Result<(), ExcelError> {
        // -1600 + 10000/(1+r) - 10000/(1+r)^2 = 0 has exact roots r = 0.25 and r = 4.0.
        let cfs = [-1600.0, 10_000.0, -10_000.0];
        let low = solve_periodic_irr(&cfs, 0.10)?;
        assert!((low - 0.25).abs() < 1e-13);

        let high = solve_periodic_irr(&cfs, 3.0)?;
        assert!((high - 4.0).abs() < 1e-13);

        // Stationary point guess = 1.0 (where derivative == 0) picks closer root in log-space (0.25).
        let stationary = solve_periodic_irr(&cfs, 1.0)?;
        assert!((stationary - 0.25).abs() < 1e-13);

        // Huge guess = 1e160 finds the upper root (4.0) without skipping [0.25, 4.0].
        let from_inf = solve_periodic_irr(&cfs, 1e160)?;
        assert!((from_inf - 4.0).abs() < 1e-12);

        // Narrow multi-root series [-1000, 2300, -1320] with roots at 0.10 and 0.20.
        let narrow = [-1000.0, 2300.0, -1320.0];
        assert!((solve_periodic_irr(&narrow, 0.05)? - 0.10).abs() < 1e-12);
        assert!((solve_periodic_irr(&narrow, 0.25)? - 0.20).abs() < 1e-12);
        assert!((solve_periodic_irr(&narrow, 1e160)? - 0.20).abs() < 1e-12);

        // Multiplicity-2 tangent root [100, -200, 100] at r = 0.0 (f64 conditioning limit ~sqrt(eps) = 1.5e-8).
        let tangent = solve_periodic_irr(&[100.0, -200.0, 100.0], 0.10)?;
        assert!(tangent.abs() < 1e-7);
        Ok(())
    }

    #[test]
    fn rootless_and_same_day_degenerate_series_return_num_error() {
        assert_eq!(
            solve_periodic_irr(&[100.0, -10.0, 100.0], 0.10),
            Err(ExcelError::Num)
        );
        assert_eq!(
            solve_periodic_irr(&[-100.0, 50.0, -100.0], 0.10),
            Err(ExcelError::Num)
        );
        assert_eq!(
            solve_periodic_irr(&[-100.0, 50.0, -100.0], -0.9999),
            Err(ExcelError::Num)
        );
        assert_eq!(
            solve_periodic_irr(&[0.0, 100.0, -10.0, 100.0, 0.0], 1e160),
            Err(ExcelError::Num)
        );

        // Same-day cancelling cash flows or single-sign coalesced net cash flows have no root.
        assert_eq!(
            solve_scheduled_xirr(&[-100.0, 100.0], &[45292.0, 45292.0], 0.10),
            Err(ExcelError::Num)
        );
        assert_eq!(
            solve_scheduled_xirr(&[-100.0, 50.0, -40.0], &[45292.0, 45292.0, 45657.0], 0.10),
            Err(ExcelError::Num)
        );
    }

    #[test]
    fn zero_root_normalizes_negative_zero_and_scales_extreme_magnitudes() -> Result<(), ExcelError>
    {
        let r_exact_neg_zero_guess = solve_periodic_irr(&[-100.0, 50.0, 50.0], -0.0)?;
        assert_eq!(r_exact_neg_zero_guess.to_bits(), 0.0_f64.to_bits());

        let r_iterated = solve_periodic_irr(&[-100.0, 50.0, 50.0], -0.05)?;
        assert!(r_iterated.abs() < 1e-15);

        let tiny = solve_periodic_irr(&[-1e-250, 1.1e-250], 0.10)?;
        assert!((tiny - 0.10).abs() < 1e-14);

        let huge = solve_periodic_irr(&[-1e250, 1.1e250], 0.10)?;
        assert!((huge - 0.10).abs() < 1e-14);
        Ok(())
    }

    #[test]
    fn wipeout_and_extreme_scale_roots_converge_precisely() -> Result<(), ExcelError> {
        // Severe-loss / salvage cash flows near -1.0 where 1 + r suffers f64 cancellation.
        for (cfs, expected) in [
            ([-500_000.0, 1.0], -0.999_998),
            ([-1_000_000.0, 1.0], -0.999_999),
            ([-1_000.0, 1e-12], -1.0 + 1e-15),
        ] {
            let r_default = solve_periodic_irr(&cfs, 0.10)?;
            assert!((r_default - expected).abs() <= 2e-16);

            let r_exact_guess = solve_periodic_irr(&cfs, expected)?;
            assert!((r_exact_guess - expected).abs() <= 2e-16);

            let dates = [45292.0, 45657.0];
            let xirr_default = solve_scheduled_xirr(&cfs, &dates, 0.10)?;
            assert!((xirr_default - expected).abs() <= 2e-16);

            let xirr_exact = solve_scheduled_xirr(&cfs, &dates, expected)?;
            assert!((xirr_exact - expected).abs() <= 2e-16);
        }

        // Half-year (182-day) salvage schedule where (1e-6)^(365/182) - 1 = -0.999_999_999_999_073_1
        // exercises log-space ULP-aware Brent floor (dx_ulp) and (1 + next_rate)-scaled Newton step limit.
        let half_year_dates = [45292.0, 45474.0];
        let half_year_cfs = [-1_000_000.0, 1.0];
        let half_year_expected = -0.999_999_999_999_073_1_f64;
        for guess in [0.10, -0.999_999_999_99, -0.999_999_999_998_5] {
            let r_half = solve_scheduled_xirr(&half_year_cfs, &half_year_dates, guess)?;
            assert!(
                (r_half - half_year_expected).abs() <= 2e-16,
                "guess={guess}: expected {half_year_expected:.16}, got {r_half:.16}"
            );
        }

        // Roots closer to -1.0 than f64 spacing (1 + r* = 1e-20 << 2^-53) have no representable root > -1.0.
        assert_eq!(
            solve_periodic_irr(&[-1e10, 1e-10], 0.10),
            Err(ExcelError::Num)
        );
        assert_eq!(
            solve_periodic_irr(&[-1.0, 2e-8, -1e-15], -0.999_999_999_999_999_9),
            Err(ExcelError::Num)
        );

        // Extreme positive root r = 1e12 - 1 exercises scale-aware derivative check.
        let r_high = solve_periodic_irr(&[-1e-12, 1.0], 1e11)?;
        assert!((r_high - (1e12 - 1.0)) / 1e12 < 1e-14);

        // Same-day cancelling huge cash flows preserve full precision on remaining tiny entries.
        let xirr_cancel = solve_scheduled_xirr(
            &[1e307, -1e307, -1e-10, 1.21e-10],
            &[45292.0, 45292.0, 45292.0, 45657.0],
            0.10,
        )?;
        assert!((xirr_cancel - 0.21).abs() < 1e-14);

        // Same-day f64::MAX-scale overflow falls back cleanly to power-of-two pre-scaling.
        let xirr_overflow_day = solve_scheduled_xirr(
            &[-1e308, -1e308, 1.1e308, 1.1e308],
            &[45292.0, 45292.0, 45657.0, 45657.0],
            0.20,
        )?;
        assert!((xirr_overflow_day - 0.10).abs() < 1e-14);
        Ok(())
    }
}
