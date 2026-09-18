use crate::functions::traits::FunctionArg;
use crate::value::{CellValue, ExcelError};

/// Normalizes signed negative zero (`-0.0`) to positive zero (`0.0`).
#[inline]
pub fn normalize_zero(val: f64) -> f64 {
    if val == 0.0 { 0.0 } else { val }
}

/// Aggregated numeric statistics gathered across arguments for arithmetic functions.
#[derive(Debug, Clone, PartialEq)]
pub struct NumericCollector {
    /// Sum of all numbers and numeric scalars.
    pub sum: f64,
    /// Count of numbers collected.
    pub count: usize,
    /// Minimum numeric value encountered, or `None` if no numbers were collected.
    pub min: Option<f64>,
    /// Maximum numeric value encountered, or `None` if no numbers were collected.
    pub max: Option<f64>,
}

impl NumericCollector {
    /// Constructs a new, empty `NumericCollector`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            sum: 0.0,
            count: 0,
            min: None,
            max: None,
        }
    }

    /// Records a single numeric observation, updating sum, count, min, and max.
    pub fn record(&mut self, val: f64) {
        self.sum += val;
        self.count += 1;
        self.min = Some(match self.min {
            Some(curr) => curr.min(val),
            None => val,
        });
        self.max = Some(match self.max {
            Some(curr) => curr.max(val),
            None => val,
        });
    }

    /// Collects numbers across arguments for arithmetic aggregators (`SUM`, `AVERAGE`, `MIN`, `MAX`).
    ///
    /// # Excel Parity Rules
    /// - In range references (`is_range_ref: true`): text, booleans, and empty cells are ignored; cell errors bubble up immediately.
    /// - In direct scalars: booleans convert to `1.0`/`0.0`, empty cells contribute `0.0`, text returns `ExcelError::Value`, and errors bubble immediately.
    /// - In explicit array literals (`is_range_ref: false`): booleans and empty cells convert; text produces `ExcelError::Value`.
    pub fn collect(args: &[FunctionArg<'_>]) -> Result<Self, ExcelError> {
        let mut collector = Self::new();

        for arg in args {
            match arg {
                FunctionArg::Scalar(val) => match val {
                    CellValue::Number(n) => collector.record(*n),
                    CellValue::Boolean(true) => collector.record(1.0),
                    CellValue::Boolean(false) => collector.record(0.0),
                    CellValue::Empty => collector.record(0.0),
                    CellValue::Error(e) => return Err(*e),
                    CellValue::Text(_) => return Err(ExcelError::Value),
                },
                FunctionArg::Array {
                    values,
                    is_range_ref,
                    ..
                } => {
                    if *is_range_ref {
                        for v in values {
                            match v {
                                CellValue::Number(n) => collector.record(*n),
                                CellValue::Error(e) => return Err(*e),
                                CellValue::Text(_) | CellValue::Boolean(_) | CellValue::Empty => {
                                    // Range references silently ignore text, booleans, and empty cells
                                }
                            }
                        }
                    } else {
                        for v in values {
                            match v {
                                CellValue::Number(n) => collector.record(*n),
                                CellValue::Boolean(true) => collector.record(1.0),
                                CellValue::Boolean(false) => collector.record(0.0),
                                CellValue::Empty => collector.record(0.0),
                                CellValue::Error(e) => return Err(*e),
                                CellValue::Text(_) => return Err(ExcelError::Value),
                            }
                        }
                    }
                }
                FunctionArg::Lazy(_) | FunctionArg::Omitted => {
                    // Lazy arguments in arithmetic positions or omitted trailing arguments contribute nothing
                }
            }
        }

        collector.sum = normalize_zero(collector.sum);
        Ok(collector)
    }

    /// Returns the minimum value, defaulting to `0.0` if no numbers were collected.
    #[must_use]
    pub fn min_or_zero(&self) -> f64 {
        normalize_zero(self.min.unwrap_or(0.0))
    }

    /// Returns the maximum value, defaulting to `0.0` if no numbers were collected.
    #[must_use]
    pub fn max_or_zero(&self) -> f64 {
        normalize_zero(self.max.unwrap_or(0.0))
    }
}

impl Default for NumericCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Counts numeric cells across arguments according to Excel `COUNT` semantics:
///
/// - In ranges: only numeric cells are counted; text, booleans, empty cells, and cell errors are ignored (+0).
/// - In direct scalars: numbers and booleans count as 1; text, empty cells, and errors count as 0.
#[must_use]
pub fn count_numbers(args: &[FunctionArg<'_>]) -> usize {
    let mut count = 0;

    for arg in args {
        match arg {
            FunctionArg::Scalar(val) => match val {
                CellValue::Number(_) | CellValue::Boolean(_) => count += 1,
                CellValue::Text(_) | CellValue::Empty | CellValue::Error(_) => {}
            },
            FunctionArg::Array { values, .. } => {
                for v in values {
                    if matches!(v, CellValue::Number(_)) {
                        count += 1;
                    }
                }
            }
            FunctionArg::Lazy(_) | FunctionArg::Omitted => {}
        }
    }

    count
}

/// Counts non-empty cells across arguments according to Excel `COUNTA` semantics:
///
/// - In ranges: cells containing numbers, text, booleans, or errors count as 1; only `CellValue::Empty` is ignored.
/// - In direct scalars: numbers, text, booleans, and errors count as 1; `CellValue::Empty` and omitted arguments count as 0.
#[must_use]
pub fn count_non_empty(args: &[FunctionArg<'_>]) -> usize {
    let mut count = 0;

    for arg in args {
        match arg {
            FunctionArg::Scalar(val) => {
                if !matches!(val, CellValue::Empty) {
                    count += 1;
                }
            }
            FunctionArg::Array { values, .. } => {
                for v in values {
                    if !matches!(v, CellValue::Empty) {
                        count += 1;
                    }
                }
            }
            FunctionArg::Lazy(_) | FunctionArg::Omitted => {}
        }
    }

    count
}

/// Clamps decimal places to prevent IEEE-754 exponent overflow / underflow.
const MAX_DECIMAL_DIGITS: i32 = 308;

/// Performs symmetric rounding half away from zero (standard Excel `ROUND`).
#[must_use]
pub fn round_half_away_from_zero(val: f64, digits: i32) -> f64 {
    if !val.is_finite() || val == 0.0 {
        return normalize_zero(val);
    }
    let clamped_digits = digits.clamp(-MAX_DECIMAL_DIGITS, MAX_DECIMAL_DIGITS);
    if clamped_digits >= 0 {
        let factor = 10_f64.powi(clamped_digits);
        normalize_zero((val * factor).round() / factor)
    } else {
        let factor = 10_f64.powi(-clamped_digits);
        normalize_zero((val / factor).round() * factor)
    }
}

/// Rounds away from zero (standard Excel `ROUNDUP`: ceil for positive, floor for negative).
#[must_use]
pub fn round_away_from_zero(val: f64, digits: i32) -> f64 {
    if !val.is_finite() || val == 0.0 {
        return normalize_zero(val);
    }
    let clamped_digits = digits.clamp(-MAX_DECIMAL_DIGITS, MAX_DECIMAL_DIGITS);
    let sign = if val < 0.0 { -1.0 } else { 1.0 };
    if clamped_digits >= 0 {
        let factor = 10_f64.powi(clamped_digits);
        let scaled = (val * factor).abs().ceil() * sign;
        normalize_zero(scaled / factor)
    } else {
        let factor = 10_f64.powi(-clamped_digits);
        let scaled = (val / factor).abs().ceil() * sign;
        normalize_zero(scaled * factor)
    }
}

/// Rounds towards zero (standard Excel `ROUNDDOWN`: truncate toward zero).
#[must_use]
pub fn round_towards_zero(val: f64, digits: i32) -> f64 {
    if !val.is_finite() || val == 0.0 {
        return normalize_zero(val);
    }
    let clamped_digits = digits.clamp(-MAX_DECIMAL_DIGITS, MAX_DECIMAL_DIGITS);
    let sign = if val < 0.0 { -1.0 } else { 1.0 };
    if clamped_digits >= 0 {
        let factor = 10_f64.powi(clamped_digits);
        let scaled = (val * factor).abs().floor() * sign;
        normalize_zero(scaled / factor)
    } else {
        let factor = 10_f64.powi(-clamped_digits);
        let scaled = (val / factor).abs().floor() * sign;
        normalize_zero(scaled * factor)
    }
}
