use crate::functions::error::FunctionError;
use crate::functions::traits::{EvalContext, FunctionArg};
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

    /// Iterates over numeric observations across arguments following Excel parity rules.
    fn for_each_numeric<F>(args: &[FunctionArg<'_>], mut f: F) -> Result<(), ExcelError>
    where
        F: FnMut(f64),
    {
        for arg in args {
            match arg {
                FunctionArg::Scalar(val) => match val {
                    CellValue::Number(n) => f(*n),
                    CellValue::Boolean(true) => f(1.0),
                    CellValue::Boolean(false) => f(0.0),
                    CellValue::Empty => f(0.0),
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
                                CellValue::Number(n) => f(*n),
                                CellValue::Error(e) => return Err(*e),
                                CellValue::Text(_) | CellValue::Boolean(_) | CellValue::Empty => {
                                    // Range references silently ignore text, booleans, and empty cells
                                }
                            }
                        }
                    } else {
                        for v in values {
                            match v {
                                CellValue::Number(n) => f(*n),
                                CellValue::Boolean(true) => f(1.0),
                                CellValue::Boolean(false) => f(0.0),
                                CellValue::Empty => f(0.0),
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
        Ok(())
    }

    /// Collects numbers across arguments for arithmetic aggregators (`SUM`, `AVERAGE`, `MIN`, `MAX`).
    ///
    /// # Excel Parity Rules
    /// - In range references (`is_range_ref: true`): text, booleans, and empty cells are ignored; cell errors bubble up immediately.
    /// - In direct scalars: booleans convert to `1.0`/`0.0`, empty cells contribute `0.0`, text returns `ExcelError::Value`, and errors bubble immediately.
    /// - In explicit array literals (`is_range_ref: false`): booleans and empty cells convert; text produces `ExcelError::Value`.
    pub fn collect(args: &[FunctionArg<'_>]) -> Result<Self, ExcelError> {
        let mut collector = Self::new();
        Self::for_each_numeric(args, |n| collector.record(n))?;
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

    /// Collects all numeric observations across arguments into a vector following Excel parity rules.
    pub fn collect_values(args: &[FunctionArg<'_>]) -> Result<Vec<f64>, ExcelError> {
        let mut values = Vec::new();
        Self::for_each_numeric(args, |n| values.push(n))?;
        Ok(values)
    }
}

impl Default for NumericCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Evaluates an individual branch argument, returning its scalar value or evaluating its lazy expression.
pub fn eval_branch_arg(
    arg: &FunctionArg<'_>,
    function: &'static str,
    ctx: &dyn EvalContext,
) -> Result<CellValue, FunctionError> {
    match arg {
        FunctionArg::Omitted => Ok(CellValue::Empty),
        FunctionArg::Scalar(val) => Ok(val.clone()),
        FunctionArg::Lazy(expr) => ctx.eval_expr(expr),
        FunctionArg::Array { .. } => Err(FunctionError::InvalidArgumentKind {
            function,
            expected: "scalar or expression",
            found: "array",
        }),
    }
}

/// Aggregated logical truth statistics gathered across arguments for boolean functions (`AND`, `OR`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogicalCollector {
    /// Number of valid logical values collected.
    pub count: usize,
    /// Whether all collected values are true (logical conjunction).
    pub all_true: bool,
    /// Whether any collected value is true (logical disjunction).
    pub any_true: bool,
}

impl LogicalCollector {
    /// Constructs a new, empty `LogicalCollector`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            count: 0,
            all_true: true,
            any_true: false,
        }
    }

    /// Records a single logical truth observation.
    pub fn record(&mut self, b: bool) {
        self.count += 1;
        self.all_true = self.all_true && b;
        self.any_true = self.any_true || b;
    }

    /// Collects logical truth values across arguments for `AND` and `OR`.
    ///
    /// # Excel Parity Rules
    /// - In range references (`is_range_ref: true`): text, empty cells are ignored; cell errors bubble immediately.
    /// - In direct scalars: numbers (0 = false, non-zero = true), booleans, text ("TRUE"/"FALSE" case-insensitive, else `#VALUE!`), empty cells evaluate to false; errors bubble immediately.
    /// - In array literals (`is_range_ref: false`): text parses if "TRUE"/"FALSE", else `#VALUE!`.
    /// - If no logical values are found across all arguments and ranges, returns `ExcelError::Value`.
    pub fn collect(args: &[FunctionArg<'_>]) -> Result<Self, ExcelError> {
        let mut collector = Self::new();

        for arg in args {
            match arg {
                FunctionArg::Scalar(val) => {
                    let b = val.coerce_to_bool()?;
                    collector.record(b);
                }
                FunctionArg::Array {
                    values,
                    is_range_ref,
                    ..
                } => {
                    if *is_range_ref {
                        for v in values {
                            match v {
                                CellValue::Boolean(b) => collector.record(*b),
                                CellValue::Number(n) => collector.record(*n != 0.0),
                                CellValue::Error(e) => return Err(*e),
                                CellValue::Text(_) | CellValue::Empty => {
                                    // Range references silently ignore text and empty cells
                                }
                            }
                        }
                    } else {
                        for v in values {
                            let b = v.coerce_to_bool()?;
                            collector.record(b);
                        }
                    }
                }
                FunctionArg::Lazy(_) | FunctionArg::Omitted => {}
            }
        }

        if collector.count == 0 {
            return Err(ExcelError::Value);
        }

        Ok(collector)
    }
}

impl Default for LogicalCollector {
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

/// Token in a compiled Excel wildcard pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WildcardToken {
    /// Exact character to match (ASCII-lowercased for case-insensitivity).
    Char(char),
    /// Wildcard `?` matching exactly one character.
    AnySingle,
    /// Wildcard `*` matching zero or more characters.
    AnyZeroOrMore,
}

/// Parses a pattern string into wildcard tokens, handling the `~` escape sequence.
#[must_use]
pub fn parse_wildcard_tokens(pattern: &str) -> (Vec<WildcardToken>, bool) {
    let mut tokens = Vec::new();
    let mut chars = pattern.chars().peekable();
    let mut has_wildcard = false;

    while let Some(c) = chars.next() {
        match c {
            '~' => {
                if let Some(&next_c) = chars.peek() {
                    if next_c == '?' || next_c == '*' || next_c == '~' {
                        chars.next();
                        tokens.push(WildcardToken::Char(next_c.to_ascii_lowercase()));
                    } else {
                        tokens.push(WildcardToken::Char('~'));
                    }
                } else {
                    tokens.push(WildcardToken::Char('~'));
                }
            }
            '?' => {
                has_wildcard = true;
                tokens.push(WildcardToken::AnySingle);
            }
            '*' => {
                has_wildcard = true;
                // Collapse consecutive asterisks
                if !matches!(tokens.last(), Some(WildcardToken::AnyZeroOrMore)) {
                    tokens.push(WildcardToken::AnyZeroOrMore);
                }
            }
            other => {
                tokens.push(WildcardToken::Char(other.to_ascii_lowercase()));
            }
        }
    }

    (tokens, has_wildcard)
}

/// Evaluates whether a text string matches a slice of wildcard tokens using linear-time backtracking.
#[must_use]
pub fn matches_wildcard(tokens: &[WildcardToken], text: &str) -> bool {
    let s: Vec<char> = text.chars().map(|c| c.to_ascii_lowercase()).collect();
    let mut s_idx = 0;
    let mut p_idx = 0;
    let mut star_idx: Option<usize> = None;
    let mut s_match_idx = 0;

    while s_idx < s.len() {
        if p_idx < tokens.len()
            && match tokens[p_idx] {
                WildcardToken::Char(c) => c == s[s_idx],
                WildcardToken::AnySingle => true,
                WildcardToken::AnyZeroOrMore => false,
            }
        {
            s_idx += 1;
            p_idx += 1;
        } else if p_idx < tokens.len() && tokens[p_idx] == WildcardToken::AnyZeroOrMore {
            star_idx = Some(p_idx);
            s_match_idx = s_idx;
            p_idx += 1;
        } else if let Some(star) = star_idx {
            p_idx = star + 1;
            s_match_idx += 1;
            s_idx = s_match_idx;
        } else {
            return false;
        }
    }

    while p_idx < tokens.len() && tokens[p_idx] == WildcardToken::AnyZeroOrMore {
        p_idx += 1;
    }

    p_idx == tokens.len()
}

/// Extracts a scalar numeric value from an argument, coercing booleans and empty cells,
/// bubbling errors, and rejecting text with `#VALUE!`.
pub fn extract_scalar_number(arg: &FunctionArg<'_>) -> Result<f64, ExcelError> {
    match arg {
        FunctionArg::Scalar(val) => val.coerce_to_number(),
        FunctionArg::Array { values, .. } if values.len() == 1 => values[0].coerce_to_number(),
        _ => Err(ExcelError::Value),
    }
}
