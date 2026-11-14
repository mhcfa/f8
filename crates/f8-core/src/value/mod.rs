//! Runtime value representations and Excel parity calculation semantics.
//!
//! Defines `CellValue`, `ExcelError`, and calculation rules.

use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::expr::ast::Literal;
use crate::model::table::ValueLiteral;

pub mod date;
pub mod error;
pub mod format;

pub use date::{
    DateSerial, DateTuple, EXCEL_1900_LEAP_DAY_SERIAL, MAX_EXCEL_SERIAL, MIN_INPUT_DATE_SERIAL,
    UNIX_EPOCH_SERIAL, is_valid_input_date_serial, validate_date_serial,
};
pub use error::{
    ExcelError, ExpectedType, ParseExcelError, StrictIntError, TextCriterionError, TypeError,
    ValueType,
};
pub use format::{format_accounting_number, format_cell_value};

/// Maximum exact integer magnitude representable without precision loss in IEEE-754 `f64` (`2^53 - 1`).
pub(crate) const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// Normalizes IEEE-754 negative zero (`-0.0`) to canonical positive zero (`0.0`).
#[inline]
#[must_use]
pub fn normalize_zero(val: f64) -> f64 {
    if val == 0.0 { 0.0 } else { val }
}

/// Parses a locale-independent finite Excel numeric string.
///
/// Trims leading and trailing ASCII spaces (`' '`), rejects non-finite float tokens
/// (`"NaN"`, `"inf"`, `"Infinity"`, and exponent overflows), and normalizes `-0.0` to `0.0`.
#[must_use]
pub(crate) fn parse_excel_number(s: &str) -> Option<f64> {
    let trimmed = s.trim_matches(' ');
    let num = trimmed.parse::<f64>().ok()?;
    if !num.is_finite() {
        return None;
    }
    Some(normalize_zero(num))
}

/// Base-10 15-significant-digit representation of a finite non-zero `f64`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Decimal15 {
    /// Sign of the original finite non-zero value.
    pub is_neg: bool,
    /// Unrounded base-10 exponent `floor(log10(abs(val)))`.
    pub raw_exp10: i32,
    /// 15-digit decimal significand in `100_000_000_000_000..=999_999_999_999_999`,
    /// rounded half-away-from-zero at the 16th significant digit (capped at Excel's
    /// `1.79769313486231e308` ceiling when `exp10 == 308`).
    pub sig15: u64,
    /// Post-rounding base-10 exponent (`raw_exp10` or `raw_exp10 + 1` on 15-digit carry),
    /// representing exact decimal magnitude `sig15 * 10^(exp10 - 14)`.
    pub exp10: i32,
}

impl Decimal15 {
    /// Extracts the 15-significant-digit base-10 representation of a finite non-zero `f64`.
    #[must_use]
    pub(crate) fn from_f64(val: f64) -> Option<Self> {
        if !val.is_finite() || val == 0.0 {
            return None;
        }
        let is_neg = val.is_sign_negative();
        let sci = format!("{:.35e}", val.abs());
        let (coeff, exp_str) = sci.split_once('e')?;
        let raw_exp10 = exp_str.parse::<i32>().ok()?;
        let mut exp10 = raw_exp10;

        let mut sig15: u64 = 0;
        let mut d16: u64 = 0;
        let mut digit_idx = 0usize;
        for b in coeff.bytes() {
            if b == b'.' {
                continue;
            }
            if !b.is_ascii_digit() {
                return None;
            }
            let d = u64::from(b - b'0');
            if digit_idx < 15 {
                sig15 = sig15 * 10 + d;
            } else if digit_idx == 15 {
                d16 = d;
                break;
            }
            digit_idx += 1;
        }

        if d16 >= 5 {
            sig15 += 1;
            if sig15 == 1_000_000_000_000_000 {
                sig15 = 100_000_000_000_000;
                exp10 += 1;
            }
        }
        if exp10 == 308 && sig15 > 179_769_313_486_231 {
            sig15 = 179_769_313_486_231;
        }

        Some(Self {
            is_neg,
            raw_exp10,
            sig15,
            exp10,
        })
    }

    /// Rounds `self * 10^scale_exp10` to `digits` decimal places under `mode`,
    /// returning `None` when the rounded magnitude is zero.
    #[must_use]
    pub(crate) fn round_scaled(
        self,
        scale_exp10: i32,
        digits: i64,
        mode: DecimalRoundingMode,
    ) -> Option<RoundedDecimal> {
        let scaled_exp10 = i128::from(self.exp10) + i128::from(scale_exp10);
        let k = scaled_exp10 + i128::from(digits) + 1;
        if k <= 0 {
            return match mode {
                DecimalRoundingMode::Truncate => None,
                DecimalRoundingMode::HalfAway => {
                    if k == 0 && self.sig15 >= 500_000_000_000_000 {
                        Some(RoundedDecimal {
                            coeff: 1,
                            num_digits: 1,
                            top_exp10: -i128::from(digits),
                        })
                    } else {
                        None
                    }
                }
                DecimalRoundingMode::Away => Some(RoundedDecimal {
                    coeff: 1,
                    num_digits: 1,
                    top_exp10: -i128::from(digits),
                }),
            };
        }
        if k <= 15 {
            let eff_k = k as u32;
            let drop = (15 - k) as u32;
            let scale = 10_u64.pow(drop);
            let mut q = self.sig15 / scale;
            let rem = self.sig15 % scale;
            let bump = match mode {
                DecimalRoundingMode::HalfAway => drop > 0 && rem >= scale / 2,
                DecimalRoundingMode::Away => rem > 0,
                DecimalRoundingMode::Truncate => false,
            };
            if bump {
                q += 1;
                if q == 10_u64.pow(eff_k) {
                    return Some(RoundedDecimal {
                        coeff: 1,
                        num_digits: 1,
                        top_exp10: scaled_exp10 + 1,
                    });
                }
            }
            return Some(RoundedDecimal {
                coeff: q,
                num_digits: eff_k,
                top_exp10: scaled_exp10,
            });
        }
        Some(RoundedDecimal {
            coeff: self.sig15,
            num_digits: 15,
            top_exp10: scaled_exp10,
        })
    }

    fn parse_sci_f64(self, coeff: u64, exp10: i32) -> f64 {
        if coeff == 0 {
            return 0.0;
        }
        let sign = if self.is_neg { "-" } else { "" };
        let formatted = format!("{sign}{coeff}e{exp10}");
        match formatted.parse::<f64>() {
            Ok(parsed) => normalize_zero(parsed),
            Err(_) => {
                if self.is_neg {
                    f64::NEG_INFINITY
                } else {
                    f64::INFINITY
                }
            }
        }
    }
}

/// Non-zero base-10 value rounded at a target decimal place (`coeff * 10^(top_exp10 - (num_digits - 1))`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RoundedDecimal {
    /// Unsigned decimal significand (`1..=999_999_999_999_999`, `num_digits` digits, no leading zeros).
    pub coeff: u64,
    /// Exact decimal digit count of `coeff` (`1..=15`).
    pub num_digits: u32,
    /// Base-10 exponent of the leading digit of `coeff`.
    pub top_exp10: i128,
}

/// Directional decimal rounding mode for Excel `ROUND`, `ROUNDUP`, and `ROUNDDOWN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecimalRoundingMode {
    /// Symmetric half away from zero (`ROUND`).
    HalfAway,
    /// Strictly away from zero (`ROUNDUP`).
    Away,
    /// Strictly toward zero / truncation (`ROUNDDOWN`).
    Truncate,
}

/// Rounds `val` to `digits` decimal places in base 10 using Excel's 15-significant-digit decimal model.
///
/// - Preserves `val` unchanged when `digits >= 15 - floor(log10(abs(val)))` (exceeding 15 significant digits).
/// - Returns `+/-f64::INFINITY` when rounding overflows IEEE-754 `f64::MAX` (for `CellValue::number` -> `#NUM!`).
/// - Normalizes `-0.0` to `0.0`.
#[must_use]
pub(crate) fn round_decimal_f64(val: f64, digits: i64, mode: DecimalRoundingMode) -> f64 {
    if !val.is_finite() || val == 0.0 {
        return normalize_zero(val);
    }
    let Some(dec) = Decimal15::from_f64(val) else {
        return normalize_zero(val);
    };
    if i128::from(digits) >= 15 - i128::from(dec.raw_exp10) {
        return normalize_zero(val);
    }
    let Some(rounded) = dec.round_scaled(0, digits, mode) else {
        return 0.0;
    };
    if rounded.top_exp10 > 308 {
        return if dec.is_neg {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    let place_exp10 = rounded.top_exp10 - i128::from(rounded.num_digits - 1);
    let Ok(exp10) = i32::try_from(place_exp10) else {
        return 0.0;
    };
    dec.parse_sci_f64(rounded.coeff, exp10)
}

/// Normalizes a finite `f64` comparator threshold to Excel's 15-significant-digit decimal value
/// (half-away-from-zero, `-0.0` normalized to `0.0`).
#[must_use]
pub fn normalize_criteria_number(val: f64) -> f64 {
    let norm = normalize_zero(val);
    if !norm.is_finite() || norm == 0.0 {
        return norm;
    }
    let formatted = format_criteria_number(norm);
    match formatted.parse::<f64>() {
        Ok(parsed) => normalize_zero(parsed),
        Err(_) => f64::NAN,
    }
}

/// Formats a finite `f64` comparator literal using Excel's 15-significant-digit decimal representation,
/// emitting uppercase `E` scientific notation on extreme exponents to stay compact (`<= 23` chars).
#[must_use]
pub fn format_criteria_number(val: f64) -> String {
    let norm = normalize_zero(val);
    if !norm.is_finite() {
        return norm.to_string();
    }
    if norm == 0.0 {
        return "0".to_string();
    }
    let Some(dec) = Decimal15::from_f64(norm) else {
        return "0".to_string();
    };
    let raw_digits = dec.sig15.to_string();
    let digits = raw_digits.trim_end_matches('0');
    let exp10 = dec.exp10;
    let sign = if dec.is_neg { "-" } else { "" };
    if (0..15).contains(&exp10) {
        let int_len = (exp10 + 1) as usize;
        if let (Some(int_part), Some(frac_part)) = (digits.get(..int_len), digits.get(int_len..))
            && !frac_part.is_empty()
        {
            format!("{sign}{int_part}.{frac_part}")
        } else {
            let zeros = "0".repeat(int_len.saturating_sub(digits.len()));
            format!("{sign}{digits}{zeros}")
        }
    } else if (-6..0).contains(&exp10) {
        let leading_zeros = "0".repeat((-exp10 - 1) as usize);
        format!("{sign}0.{leading_zeros}{digits}")
    } else if let (Some(first), Some(rest)) = (digits.get(..1), digits.get(1..))
        && !rest.is_empty()
    {
        format!("{sign}{first}.{rest}E{exp10}")
    } else {
        format!("{sign}{digits}E{exp10}")
    }
}

const EXCEL_ERROR_LITERALS: &[&str] = &[
    "#NULL!",
    "#DIV/0!",
    "#VALUE!",
    "#REF!",
    "#NAME?",
    "#NUM!",
    "#N/A",
    "#GETTING_DATA",
    "#SPILL!",
    "#CALC!",
    "#FIELD!",
    "#BLOCKED!",
    "#UNKNOWN!",
    "#CONNECT!",
    "#PYTHON!",
];

fn is_excel_error_literal(trimmed: &str) -> bool {
    EXCEL_ERROR_LITERALS
        .iter()
        .any(|lit| trimmed.eq_ignore_ascii_case(lit))
}

fn strip_digit_group_commas(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    for (idx, ch) in s.char_indices() {
        if ch == ','
            && idx
                .checked_sub(1)
                .and_then(|prev| bytes.get(prev))
                .is_some_and(u8::is_ascii_digit)
            && bytes.get(idx + 1).is_some_and(u8::is_ascii_digit)
        {
            continue;
        }
        out.push(ch);
    }
    out
}

fn is_coercible_numeric_string(trimmed: &str) -> bool {
    if trimmed.is_empty() {
        return false;
    }
    if parse_excel_number(trimmed).is_some() {
        return true;
    }

    // Formatted numeric tokens: outer accounting (...), +/-, %, $/€/£/¥, and ',' between digits.
    let mut cur = trimmed;
    let mut used_parens = false;
    let mut used_sign = false;
    let mut used_curr = false;
    let mut used_pct = false;
    loop {
        cur = cur.trim();
        if !used_parens && let Some(rest) = cur.strip_prefix('(').and_then(|r| r.strip_suffix(')'))
        {
            cur = rest;
            used_parens = true;
            continue;
        }
        if !used_sign {
            if let Some(rest) = cur.strip_prefix(['+', '-']) {
                cur = rest;
                used_sign = true;
                continue;
            }
            if let Some(rest) = cur.strip_suffix(['+', '-']) {
                cur = rest;
                used_sign = true;
                continue;
            }
        }
        if !used_curr {
            if let Some(rest) = cur.strip_prefix(['$', '€', '£', '¥']) {
                cur = rest;
                used_curr = true;
                continue;
            }
            if let Some(rest) = cur.strip_suffix(['$', '€', '£', '¥']) {
                cur = rest;
                used_curr = true;
                continue;
            }
        }
        if !used_pct {
            if let Some(rest) = cur.strip_prefix('%') {
                cur = rest;
                used_pct = true;
                continue;
            }
            if let Some(rest) = cur.strip_suffix('%') {
                cur = rest;
                used_pct = true;
                continue;
            }
        }
        break;
    }
    let degrouped = strip_digit_group_commas(cur.trim());
    if !degrouped.is_empty() && parse_excel_number(&degrouped).is_some() {
        return true;
    }

    // Mixed ASCII-digit fractions: [-]INT NUM/DEN (e.g. "1 1/2")
    let unsigned = trimmed.strip_prefix(['+', '-']).unwrap_or(trimmed);
    let mut parts = unsigned.split_ascii_whitespace();
    if let (Some(int_part), Some(frac_part), None) = (parts.next(), parts.next(), parts.next())
        && !int_part.is_empty()
        && int_part.bytes().all(|b| b.is_ascii_digit())
        && let Some((num, den)) = frac_part.split_once('/')
        && !num.is_empty()
        && !den.is_empty()
        && num.bytes().all(|b| b.is_ascii_digit())
        && den.bytes().all(|b| b.is_ascii_digit())
    {
        return true;
    }

    false
}

fn is_english_month_token(tok: &str) -> bool {
    const MONTHS: &[&str] = &[
        "jan",
        "january",
        "feb",
        "february",
        "mar",
        "march",
        "apr",
        "april",
        "may",
        "jun",
        "june",
        "jul",
        "july",
        "aug",
        "august",
        "sep",
        "sept",
        "september",
        "oct",
        "october",
        "nov",
        "november",
        "dec",
        "december",
    ];
    MONTHS.iter().any(|m| tok.eq_ignore_ascii_case(m))
}

fn is_digits_or_decimal_seconds(s: &str) -> bool {
    if let Some((sec, frac)) = s.split_once('.') {
        !sec.is_empty()
            && !frac.is_empty()
            && sec.bytes().all(|b| b.is_ascii_digit())
            && frac.bytes().all(|b| b.is_ascii_digit())
    } else {
        !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
    }
}

fn strip_ascii_am_pm(s: &str) -> Option<&str> {
    if let [.., b1, b2] = s.as_bytes()
        && matches!(
            [b1.to_ascii_lowercase(), b2.to_ascii_lowercase()],
            [b'a', b'm'] | [b'p', b'm']
        )
    {
        s.get(..s.len() - 2)
    } else {
        None
    }
}

fn is_clock_time_token(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() {
        return false;
    }
    if t.contains(':') {
        let without_meridiem = if let Some(rest) = strip_ascii_am_pm(t) {
            rest.trim_end()
        } else if let Some(rest) = t.strip_suffix(['a', 'A', 'p', 'P']) {
            rest.trim_end()
        } else {
            t
        };
        let segs: Vec<&str> = without_meridiem.split(':').collect();
        match segs.as_slice() {
            [h, m] => {
                !h.is_empty()
                    && h.bytes().all(|b| b.is_ascii_digit())
                    && is_digits_or_decimal_seconds(m)
            }
            [h, m, sec] => {
                !h.is_empty()
                    && h.bytes().all(|b| b.is_ascii_digit())
                    && !m.is_empty()
                    && m.bytes().all(|b| b.is_ascii_digit())
                    && is_digits_or_decimal_seconds(sec)
            }
            [] | [_] | [_, _, _, ..] => false,
        }
    } else if let Some(rest) = strip_ascii_am_pm(t) {
        let digits = rest.trim_end();
        !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
    } else {
        false
    }
}

fn is_uniform_digit_date_segments(date_part: &str) -> bool {
    let has_slash = date_part.contains('/');
    let has_hyphen = date_part.contains('-');
    let sep = match (has_slash, has_hyphen) {
        (true, false) => '/',
        (false, true) => '-',
        (true, true) | (false, false) => return false,
    };
    let segs: Vec<&str> = date_part.split(sep).collect();
    matches!(segs.len(), 2 | 3)
        && segs
            .iter()
            .all(|seg| !seg.is_empty() && seg.bytes().all(|b| b.is_ascii_digit()))
}

fn is_english_month_date_segments(s: &str) -> bool {
    let mut month_count = 0usize;
    let mut digit_count = 0usize;
    let mut other_count = 0usize;
    for tok in s
        .split(|c: char| matches!(c, '/' | '-' | ',') || c.is_ascii_whitespace())
        .filter(|tok| !tok.is_empty())
    {
        if is_english_month_token(tok) {
            month_count += 1;
        } else if tok.bytes().all(|b| b.is_ascii_digit()) {
            digit_count += 1;
        } else {
            other_count += 1;
        }
    }
    other_count == 0 && month_count == 1 && digit_count >= 1
}

fn is_coercible_date_or_time_string(trimmed: &str) -> bool {
    if trimmed.is_empty() {
        return false;
    }

    // 1. 2 or 3 non-empty ASCII-digit segments separated uniformly by '/' or '-'
    //    with optional ' ' / 'T' clock-time suffix.
    if is_uniform_digit_date_segments(trimmed) {
        return true;
    }
    if let Some((date_part, rest)) = trimmed.split_once([' ', 'T', 't']) {
        let raw_time = rest.trim();
        let time_part = raw_time.strip_suffix(['Z', 'z']).unwrap_or(raw_time);
        if is_uniform_digit_date_segments(date_part) && is_clock_time_token(time_part) {
            return true;
        }
    }

    // 2. Clock time tokens: H:MM[:SS[.fff]] [AM|PM|A|P] or DIGITS[ ]AM|PM
    if is_clock_time_token(trimmed) {
        return true;
    }

    // 3. English month-name/abbreviation paired solely with >= 1 '/', '-', ',', or space-separated ASCII-digit tokens
    //    (with optional trailing clock-time suffix).
    if is_english_month_date_segments(trimmed) {
        return true;
    }
    for (idx, ch) in trimmed.char_indices() {
        if ch.is_ascii_whitespace()
            && let (Some(date_part), Some(rest)) = (trimmed.get(..idx), trimmed.get(idx + 1..))
            && is_english_month_date_segments(date_part.trim())
            && is_clock_time_token(rest.trim())
        {
            return true;
        }
    }
    false
}

/// Validates a `Text` criterion string for conditional aggregation (`*IFS`) functions.
///
/// `prefix_utf16_len` is `0` for exact criteria and `2` (`"<>".len()`) for `NotEq` criteria.
pub fn validate_text_criterion(s: &str, prefix_utf16_len: usize) -> Result<(), TextCriterionError> {
    if s.is_empty() {
        return Err(TextCriterionError::EmptyString);
    }
    if s.contains(['*', '?', '~']) {
        return Err(TextCriterionError::ContainsWildcardOrEscape);
    }
    if s.starts_with(['=', '<', '>']) {
        return Err(TextCriterionError::LeadingOperatorPrefix);
    }
    if prefix_utf16_len.saturating_add(s.encode_utf16().count()) > 255 {
        return Err(TextCriterionError::TooLong);
    }
    let trimmed = s.trim();
    if trimmed.eq_ignore_ascii_case("true") || trimmed.eq_ignore_ascii_case("false") {
        return Err(TextCriterionError::CoercibleBooleanString);
    }
    if is_excel_error_literal(trimmed) {
        return Err(TextCriterionError::CoercibleErrorString);
    }
    if is_coercible_numeric_string(trimmed) {
        return Err(TextCriterionError::CoercibleNumericString);
    }
    if is_coercible_date_or_time_string(trimmed) {
        return Err(TextCriterionError::CoercibleDateOrTimeString);
    }
    Ok(())
}

/// Runtime representation of a spreadsheet cell calculation result or literal value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CellValue {
    /// 64-bit floating-point numeric value.
    Number(f64),
    /// Text string value.
    Text(String),
    /// Boolean truth value (`TRUE` or `FALSE`).
    Boolean(bool),
    /// Unpopulated or blank cell.
    Empty,
    /// Excel domain error code.
    Error(ExcelError),
}

impl CellValue {
    /// Constructs a numeric `CellValue`, normalizing `-0.0` to `0.0` and mapping
    /// non-finite floats (`NaN`, `inf`, `-inf`) to `CellValue::Error(ExcelError::Num)`.
    #[inline]
    #[must_use]
    pub fn number(n: f64) -> Self {
        if n.is_finite() {
            Self::Number(normalize_zero(n))
        } else {
            Self::Error(ExcelError::Num)
        }
    }

    /// Returns the contained numeric value if this cell is `Number`.
    #[must_use]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            Self::Text(_) | Self::Boolean(_) | Self::Empty | Self::Error(_) => None,
        }
    }

    /// Returns the contained boolean value if this cell is `Boolean`.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(b) => Some(*b),
            Self::Number(_) | Self::Text(_) | Self::Empty | Self::Error(_) => None,
        }
    }

    /// Returns the contained text slice if this cell is `Text`.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s.as_str()),
            Self::Number(_) | Self::Boolean(_) | Self::Empty | Self::Error(_) => None,
        }
    }

    /// Returns `true` if this cell is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Returns `true` if this cell contains an Excel error.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Returns the contained `ExcelError` if this cell is `Error`.
    #[must_use]
    pub fn as_error(&self) -> Option<ExcelError> {
        match self {
            Self::Error(e) => Some(*e),
            Self::Number(_) | Self::Text(_) | Self::Boolean(_) | Self::Empty => None,
        }
    }

    /// Returns the [`ValueType`] of this cell, or `None` if this cell is `CellValue::Error(_)`.
    #[must_use]
    pub const fn value_type(&self) -> Option<ValueType> {
        match self {
            Self::Number(_) => Some(ValueType::Number),
            Self::Text(_) => Some(ValueType::Text),
            Self::Boolean(_) => Some(ValueType::Boolean),
            Self::Empty => Some(ValueType::Empty),
            Self::Error(_) => None,
        }
    }

    /// Extracts a strict finite numeric scalar without coercing `Text`, `Boolean`, or `Empty`.
    ///
    /// - `Number(n)` -> `Ok(Ok(normalize_zero(n)))` if finite, else `Ok(Err(ExcelError::Num))`
    /// - `Error(e)` -> `Ok(Err(e))`
    /// - `Text` / `Boolean` / `Empty` -> `Err(ValueType)`
    pub fn strict_number(&self) -> Result<Result<f64, ExcelError>, ValueType> {
        match self {
            Self::Number(n) => {
                if n.is_finite() {
                    Ok(Ok(normalize_zero(*n)))
                } else {
                    Ok(Err(ExcelError::Num))
                }
            }
            Self::Error(e) => Ok(Err(*e)),
            Self::Text(_) => Err(ValueType::Text),
            Self::Boolean(_) => Err(ValueType::Boolean),
            Self::Empty => Err(ValueType::Empty),
        }
    }

    /// Extracts a strict exact integer in `-9_007_199_254_740_991..=9_007_199_254_740_991`.
    ///
    /// - `Number(n)` -> `Ok(Err(ExcelError::Num))` if non-finite, `Err(StrictIntError::NonInteger)`
    ///   if `fract() != 0.0` or outside safe integer bounds, else `Ok(Ok(n as i64))`
    /// - `Error(e)` -> `Ok(Err(e))`
    /// - `Text` / `Boolean` / `Empty` -> `Err(StrictIntError::WrongType(ValueType))`
    pub fn strict_integer(&self) -> Result<Result<i64, ExcelError>, StrictIntError> {
        match self {
            Self::Number(n) => {
                if !n.is_finite() {
                    return Ok(Err(ExcelError::Num));
                }
                let norm = normalize_zero(*n);
                if norm.fract() != 0.0 || !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&norm) {
                    return Err(StrictIntError::NonInteger(norm.to_string()));
                }
                Ok(Ok(norm as i64))
            }
            Self::Error(e) => Ok(Err(*e)),
            Self::Text(_) => Err(StrictIntError::WrongType(ValueType::Text)),
            Self::Boolean(_) => Err(StrictIntError::WrongType(ValueType::Boolean)),
            Self::Empty => Err(StrictIntError::WrongType(ValueType::Empty)),
        }
    }

    /// Extracts a strict boolean scalar without coercing `Number`, `Text`, or `Empty`.
    ///
    /// - `Boolean(b)` -> `Ok(Ok(b))`
    /// - `Error(e)` -> `Ok(Err(e))`
    /// - `Number` / `Text` / `Empty` -> `Err(ValueType)`
    pub fn strict_bool(&self) -> Result<Result<bool, ExcelError>, ValueType> {
        match self {
            Self::Boolean(b) => Ok(Ok(*b)),
            Self::Error(e) => Ok(Err(*e)),
            Self::Number(_) => Err(ValueType::Number),
            Self::Text(_) => Err(ValueType::Text),
            Self::Empty => Err(ValueType::Empty),
        }
    }

    /// Compares two cell values under strict homogeneous non-`Empty` relational rules.
    ///
    /// - Any `Empty` operand or mismatched `(Number, Text | Boolean)` / `(Text, Boolean)` types
    ///   return `Err((left_type, right_type))` (checked before bubbling `Error(_)`).
    /// - `Error(e)` paired with any non-`Empty` operand bubbles `Ok(Err(e))` (left error first).
    /// - Homogeneous `Number`, `Text` (ASCII case-insensitive), and `Boolean` pairs return `Ok(Ok(Ordering))`.
    pub fn compare_relational(
        &self,
        other: &Self,
    ) -> Result<Result<Ordering, ExcelError>, (ValueType, ValueType)> {
        match (self, other) {
            (Self::Empty, Self::Empty)
            | (Self::Empty, Self::Error(_))
            | (Self::Error(_), Self::Empty) => Err((ValueType::Empty, ValueType::Empty)),
            (Self::Empty, Self::Number(_)) => Err((ValueType::Empty, ValueType::Number)),
            (Self::Empty, Self::Text(_)) => Err((ValueType::Empty, ValueType::Text)),
            (Self::Empty, Self::Boolean(_)) => Err((ValueType::Empty, ValueType::Boolean)),
            (Self::Number(_), Self::Empty) => Err((ValueType::Number, ValueType::Empty)),
            (Self::Text(_), Self::Empty) => Err((ValueType::Text, ValueType::Empty)),
            (Self::Boolean(_), Self::Empty) => Err((ValueType::Boolean, ValueType::Empty)),

            (Self::Number(_), Self::Text(_)) => Err((ValueType::Number, ValueType::Text)),
            (Self::Text(_), Self::Number(_)) => Err((ValueType::Text, ValueType::Number)),
            (Self::Number(_), Self::Boolean(_)) => Err((ValueType::Number, ValueType::Boolean)),
            (Self::Boolean(_), Self::Number(_)) => Err((ValueType::Boolean, ValueType::Number)),
            (Self::Text(_), Self::Boolean(_)) => Err((ValueType::Text, ValueType::Boolean)),
            (Self::Boolean(_), Self::Text(_)) => Err((ValueType::Boolean, ValueType::Text)),

            (
                Self::Error(e),
                Self::Number(_) | Self::Text(_) | Self::Boolean(_) | Self::Error(_),
            ) => Ok(Err(*e)),
            (Self::Number(_) | Self::Text(_) | Self::Boolean(_), Self::Error(e)) => Ok(Err(*e)),

            (Self::Number(a), Self::Number(b)) => {
                if !a.is_finite() || !b.is_finite() {
                    Ok(Err(ExcelError::Num))
                } else {
                    Ok(Ok(normalize_zero(*a).total_cmp(&normalize_zero(*b))))
                }
            }
            (Self::Text(a), Self::Text(b)) => {
                Ok(Ok(compare_ascii_case_insensitive(a.as_str(), b.as_str())))
            }
            (Self::Boolean(a), Self::Boolean(b)) => Ok(Ok(a.cmp(b))),
        }
    }
}

/// Compares two strings case-insensitively using ASCII case folding.
pub(crate) fn compare_ascii_case_insensitive(a: &str, b: &str) -> std::cmp::Ordering {
    a.chars()
        .map(|c| c.to_ascii_lowercase())
        .cmp(b.chars().map(|c| c.to_ascii_lowercase()))
}

impl fmt::Display for CellValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => {
                if *n == 0.0 && n.is_sign_negative() {
                    write!(f, "0")
                } else {
                    write!(f, "{n}")
                }
            }
            Self::Text(s) => write!(f, "{s}"),
            Self::Boolean(true) => write!(f, "TRUE"),
            Self::Boolean(false) => write!(f, "FALSE"),
            Self::Empty => Ok(()),
            Self::Error(e) => write!(f, "{e}"),
        }
    }
}

impl From<ValueLiteral> for CellValue {
    fn from(lit: ValueLiteral) -> Self {
        match lit {
            ValueLiteral::Number(n) => Self::number(n),
            ValueLiteral::String(s) => Self::Text(s),
            ValueLiteral::Boolean(b) => Self::Boolean(b),
            ValueLiteral::Null => Self::Empty,
        }
    }
}

impl From<&ValueLiteral> for CellValue {
    fn from(lit: &ValueLiteral) -> Self {
        match lit {
            ValueLiteral::Number(n) => Self::number(*n),
            ValueLiteral::String(s) => Self::Text(s.clone()),
            ValueLiteral::Boolean(b) => Self::Boolean(*b),
            ValueLiteral::Null => Self::Empty,
        }
    }
}

impl From<Literal> for CellValue {
    fn from(lit: Literal) -> Self {
        match lit {
            Literal::Number(n) => Self::number(n),
            Literal::String(s) => Self::Text(s),
            Literal::Boolean(b) => Self::Boolean(b),
        }
    }
}

impl From<&Literal> for CellValue {
    fn from(lit: &Literal) -> Self {
        match lit {
            Literal::Number(n) => Self::number(*n),
            Literal::String(s) => Self::Text(s.clone()),
            Literal::Boolean(b) => Self::Boolean(*b),
        }
    }
}

impl From<f64> for CellValue {
    fn from(n: f64) -> Self {
        Self::number(n)
    }
}

impl From<bool> for CellValue {
    fn from(b: bool) -> Self {
        Self::Boolean(b)
    }
}

impl From<String> for CellValue {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for CellValue {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

impl From<ExcelError> for CellValue {
    fn from(e: ExcelError) -> Self {
        Self::Error(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_value_display() {
        assert_eq!(CellValue::Number(42.5).to_string(), "42.5");
        assert_eq!(CellValue::Number(-0.0).to_string(), "0");
        assert_eq!(CellValue::Text("Revenue".into()).to_string(), "Revenue");
        assert_eq!(CellValue::Boolean(true).to_string(), "TRUE");
        assert_eq!(CellValue::Boolean(false).to_string(), "FALSE");
        assert_eq!(CellValue::Empty.to_string(), "");
        assert_eq!(CellValue::Error(ExcelError::Div0).to_string(), "#DIV/0!");
        assert_eq!(CellValue::Error(ExcelError::Value).to_string(), "#VALUE!");
    }

    #[test]
    fn test_cell_value_from_conversions() {
        assert_eq!(
            CellValue::from(ValueLiteral::Number(10.0)),
            CellValue::Number(10.0)
        );
        assert_eq!(
            CellValue::from(ValueLiteral::String("abc".into())),
            CellValue::Text("abc".into())
        );
        assert_eq!(
            CellValue::from(ValueLiteral::Boolean(true)),
            CellValue::Boolean(true)
        );
        assert_eq!(CellValue::from(ValueLiteral::Null), CellValue::Empty);

        assert_eq!(
            CellValue::from(Literal::Number(25.0)),
            CellValue::Number(25.0)
        );
        assert_eq!(
            CellValue::from(Literal::Boolean(false)),
            CellValue::Boolean(false)
        );
    }

    #[test]
    fn test_cell_value_strict_extractors() {
        assert_eq!(CellValue::Number(12.5).strict_number(), Ok(Ok(12.5)));
        assert_eq!(
            CellValue::Number(f64::NAN).strict_number(),
            Ok(Err(ExcelError::Num))
        );
        assert_eq!(
            CellValue::Error(ExcelError::Div0).strict_number(),
            Ok(Err(ExcelError::Div0))
        );
        assert_eq!(
            CellValue::Boolean(true).strict_number(),
            Err(ValueType::Boolean)
        );
        assert_eq!(
            CellValue::Text("5".into()).strict_number(),
            Err(ValueType::Text)
        );
        assert_eq!(CellValue::Empty.strict_number(), Err(ValueType::Empty));

        assert_eq!(CellValue::Number(42.0).strict_integer(), Ok(Ok(42)));
        assert_eq!(
            CellValue::Number(MAX_SAFE_INTEGER).strict_integer(),
            Ok(Ok(9_007_199_254_740_991))
        );
        assert_eq!(
            CellValue::Number(-MAX_SAFE_INTEGER).strict_integer(),
            Ok(Ok(-9_007_199_254_740_991))
        );
        assert_eq!(
            CellValue::Number(2.5).strict_integer(),
            Err(StrictIntError::NonInteger("2.5".to_string()))
        );
        assert!(matches!(
            CellValue::Number(1e20).strict_integer(),
            Err(StrictIntError::NonInteger(_))
        ));
        assert_eq!(
            CellValue::Boolean(false).strict_integer(),
            Err(StrictIntError::WrongType(ValueType::Boolean))
        );

        assert_eq!(CellValue::Boolean(true).strict_bool(), Ok(Ok(true)));
        assert_eq!(
            CellValue::Error(ExcelError::Na).strict_bool(),
            Ok(Err(ExcelError::Na))
        );
        assert_eq!(CellValue::Number(1.0).strict_bool(), Err(ValueType::Number));
        assert_eq!(
            CellValue::Text("TRUE".into()).strict_bool(),
            Err(ValueType::Text)
        );
        assert_eq!(CellValue::Empty.strict_bool(), Err(ValueType::Empty));

        assert_eq!(
            CellValue::Number(10.0).compare_relational(&CellValue::Number(20.0)),
            Ok(Ok(Ordering::Less))
        );
        assert_eq!(
            CellValue::Text("ABC".into()).compare_relational(&CellValue::Text("abc".into())),
            Ok(Ok(Ordering::Equal))
        );
        assert_eq!(
            CellValue::Number(1.0).compare_relational(&CellValue::Text("1".into())),
            Err((ValueType::Number, ValueType::Text))
        );
        assert_eq!(
            CellValue::Error(ExcelError::Div0).compare_relational(&CellValue::Empty),
            Err((ValueType::Empty, ValueType::Empty))
        );
        assert_eq!(
            CellValue::Error(ExcelError::Div0).compare_relational(&CellValue::Number(1.0)),
            Ok(Err(ExcelError::Div0))
        );
    }

    #[test]
    fn test_accessors() {
        let num = CellValue::Number(10.0);
        assert_eq!(num.as_number(), Some(10.0));
        assert_eq!(num.as_bool(), None);
        assert!(!num.is_empty());
        assert!(!num.is_error());

        let err = CellValue::Error(ExcelError::Num);
        assert!(err.is_error());
        assert_eq!(err.as_error(), Some(ExcelError::Num));
    }

    #[test]
    fn test_parse_excel_number_and_finite_constructors() {
        assert_eq!(parse_excel_number("42.5"), Some(42.5));
        assert_eq!(parse_excel_number("  -12.25  "), Some(-12.25));
        assert_eq!(parse_excel_number("+3.5"), Some(3.5));
        assert_eq!(parse_excel_number(".5"), Some(0.5));
        assert_eq!(parse_excel_number("5."), Some(5.0));
        assert_eq!(parse_excel_number("1.5e-3"), Some(0.0015));
        assert_eq!(parse_excel_number("2E+4"), Some(20_000.0));

        // Negative zero and subnormal underflow normalize to +0.0
        for neg_zero_str in ["-0", "-0.0", " -0.00 ", "-1e-400"] {
            assert_eq!(
                parse_excel_number(neg_zero_str).map(f64::to_bits),
                Some(0.0_f64.to_bits()),
                "expected +0.0 for {neg_zero_str:?}"
            );
        }

        // Non-finite tokens, overflows, control whitespace, and locale formats are rejected
        for rejected in [
            "",
            "   ",
            "NaN",
            "nan",
            "NAN",
            "+NaN",
            "-NaN",
            "inf",
            "INF",
            "-inf",
            "+inf",
            "Infinity",
            "infinity",
            "-INFINITY",
            "1e500",
            "-1e500",
            "\t10",
            "10\n",
            "$10",
            "10%",
            "1,000",
            "1_000",
            "0x10",
        ] {
            assert_eq!(
                parse_excel_number(rejected),
                None,
                "expected None for {rejected:?}"
            );
        }

        // CellValue::number and From conversions guard against NaN/Inf and normalize -0.0
        if let CellValue::Number(n) = CellValue::from(-0.0) {
            assert_eq!(n.to_bits(), 0.0_f64.to_bits());
        } else {
            panic!("expected Number(0.0)");
        }
        if let CellValue::Number(n) = CellValue::from(ValueLiteral::Number(-0.0)) {
            assert_eq!(n.to_bits(), 0.0_f64.to_bits());
        } else {
            panic!("expected Number(0.0)");
        }
        if let CellValue::Number(n) = CellValue::from(Literal::Number(-0.0)) {
            assert_eq!(n.to_bits(), 0.0_f64.to_bits());
        } else {
            panic!("expected Number(0.0)");
        }

        for non_finite in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                CellValue::number(non_finite),
                CellValue::Error(ExcelError::Num)
            );
            assert_eq!(
                CellValue::from(non_finite),
                CellValue::Error(ExcelError::Num)
            );
            assert_eq!(
                CellValue::from(ValueLiteral::Number(non_finite)),
                CellValue::Error(ExcelError::Num)
            );
            assert_eq!(
                CellValue::from(&ValueLiteral::Number(non_finite)),
                CellValue::Error(ExcelError::Num)
            );
            assert_eq!(
                CellValue::from(Literal::Number(non_finite)),
                CellValue::Error(ExcelError::Num)
            );
            assert_eq!(
                CellValue::from(&Literal::Number(non_finite)),
                CellValue::Error(ExcelError::Num)
            );
        }
    }

    #[test]
    fn test_criteria_number_normalization_and_formatting() {
        assert_eq!(format_criteria_number(0.0), "0");
        assert_eq!(format_criteria_number(-0.0), "0");
        assert_eq!(format_criteria_number(15.0), "15");
        assert_eq!(format_criteria_number(-10.0), "-10");
        assert_eq!(format_criteria_number(100.0), "100");
        assert_eq!(format_criteria_number(1.0 / 3.0), "0.333333333333333");
        assert_eq!(format_criteria_number(2.0 / 3.0), "0.666666666666667");
        assert_eq!(
            format_criteria_number(0.33333333333333337),
            "0.333333333333333"
        );
        assert_eq!(format_criteria_number(1e-300), "1E-300");
        assert_eq!(
            format_criteria_number(100_000_000_000_000.5),
            "100000000000001"
        );

        assert_eq!(normalize_criteria_number(1.0 / 3.0), 0.333333333333333);
        assert_eq!(normalize_criteria_number(2.0 / 3.0), 0.666666666666667);
        assert_eq!(
            normalize_criteria_number(100_000_000_000_000.5),
            100_000_000_000_001.0
        );
        assert_eq!(format_criteria_number(f64::MAX), "1.79769313486231E308");
        assert_eq!(format_criteria_number(-f64::MAX), "-1.79769313486231E308");
        assert_eq!(
            normalize_criteria_number(f64::MAX),
            1.797_693_134_862_31e308
        );
        assert_eq!(
            normalize_criteria_number(-f64::MAX),
            -1.797_693_134_862_31e308
        );

        for x in [
            f64::MAX,
            -f64::MAX,
            1.797_693_134_862_315e308,
            -1.797_693_134_862_315e308,
            f64::MIN_POSITIVE,
            -f64::MIN_POSITIVE,
            f64::from_bits(1),
            -f64::from_bits(1),
            1.0 / 3.0,
            2.0 / 3.0,
            0.0,
            -0.0,
        ] {
            let formatted = format_criteria_number(x);
            let normalized = normalize_criteria_number(x);
            assert!(
                normalized.is_finite(),
                "expected finite normalized threshold for {x}"
            );
            assert_eq!(
                parse_excel_number(&formatted),
                Some(normalized),
                "Pass 4 vs Pass 6a 15-sig-digit parity mismatch for {x} ({formatted:?})"
            );
        }
    }

    #[test]
    fn test_validate_text_criterion() {
        assert_eq!(
            validate_text_criterion("", 0),
            Err(TextCriterionError::EmptyString)
        );
        for wc in ["A*", "a?b", "~*", "foo~bar"] {
            assert_eq!(
                validate_text_criterion(wc, 0),
                Err(TextCriterionError::ContainsWildcardOrEscape)
            );
        }
        for op_str in [">15", "=Alpha", "<>0", "<10"] {
            assert_eq!(
                validate_text_criterion(op_str, 0),
                Err(TextCriterionError::LeadingOperatorPrefix)
            );
        }
        assert_eq!(validate_text_criterion(&"a".repeat(255), 0), Ok(()));
        assert_eq!(
            validate_text_criterion(&"a".repeat(256), 0),
            Err(TextCriterionError::TooLong)
        );
        assert_eq!(validate_text_criterion(&"a".repeat(253), 2), Ok(()));
        assert_eq!(
            validate_text_criterion(&"a".repeat(254), 2),
            Err(TextCriterionError::TooLong)
        );

        // Surrogate-pair (4-byte UTF-8 / 2 UTF-16 code units) 255-UTF-16 boundary checks
        assert_eq!(
            validate_text_criterion(&format!("{}a", "😀".repeat(127)), 0),
            Ok(())
        );
        assert_eq!(
            validate_text_criterion(&"😀".repeat(128), 0),
            Err(TextCriterionError::TooLong)
        );
        assert_eq!(
            validate_text_criterion(&format!("{}a", "😀".repeat(126)), 2),
            Ok(())
        );
        assert_eq!(
            validate_text_criterion(&"😀".repeat(127), 2),
            Err(TextCriterionError::TooLong)
        );

        for b_str in ["TRUE", "false", " true "] {
            assert_eq!(
                validate_text_criterion(b_str, 0),
                Err(TextCriterionError::CoercibleBooleanString)
            );
        }
        for err_str in ["#DIV/0!", "#N/A", " #value! ", "#SPILL!", "#CALC!"] {
            assert_eq!(
                validate_text_criterion(err_str, 0),
                Err(TextCriterionError::CoercibleErrorString)
            );
        }
        for num_str in [
            "42",
            "042",
            " 42 ",
            "1e2",
            "10%",
            "1,000",
            "$50",
            "€100",
            "100€",
            "£50",
            "¥1,000",
            "($1,200.50)",
            "(100)",
            "1 1/2",
        ] {
            assert_eq!(
                validate_text_criterion(num_str, 0),
                Err(TextCriterionError::CoercibleNumericString),
                "expected CoercibleNumericString for {num_str:?}"
            );
        }
        for dt_str in [
            "2024-01-01",
            "2024-01-01 09:30",
            "2024-01-01T12:30:00Z",
            "01/15/2024",
            "2024-01",
            "1/2",
            "1-2",
            "Jan 2024",
            "Jan-24",
            "15-Jan-2024",
            "15-Jan-2024 09:30",
            "January 1, 2024",
            "Jan 1, 2024 12:00 PM",
            "12:30",
            "9:00 AM",
            "5 PM",
            "5pm",
        ] {
            assert_eq!(
                validate_text_criterion(dt_str, 0),
                Err(TextCriterionError::CoercibleDateOrTimeString),
                "expected CoercibleDateOrTimeString for {dt_str:?}"
            );
        }
        for valid in [
            "Alpha",
            "nan",
            "Infinity",
            "#1 Priority",
            "Q1-2024",
            "2024E",
            "FY24",
            "Tier-1",
            "May",
            "1A",
            "2P",
            "€",
            "España",
            "東京",
            "café!",
            "Note: café",
            "Note: caféa",
            "Note: caféam",
            "T:東京",
            "😀",
        ] {
            assert_eq!(
                validate_text_criterion(valid, 0),
                Ok(()),
                "expected Ok(0) for {valid:?}"
            );
            assert_eq!(
                validate_text_criterion(valid, 2),
                Ok(()),
                "expected Ok(2) for {valid:?}"
            );
        }
    }
}
