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
    is_valid_input_date_serial, validate_date_serial,
};
pub use error::{ExcelError, TypeError, ValueType};
pub use format::{format_accounting_number, format_cell_value};

/// Maximum exact integer magnitude representable without precision loss in IEEE-754 `f64` (`2^53 - 1`).
pub(crate) const MAX_SAFE_INTEGER: f64 = 9_007_199_254_740_991.0;

/// Converts a finite `f64` into an exact safe integer in `-9_007_199_254_740_991..=9_007_199_254_740_991`.
pub(crate) fn f64_to_strict_integer(n: f64) -> Result<i64, ExcelError> {
    if !n.is_finite() {
        return Err(ExcelError::Num);
    }
    let norm = normalize_zero(n);
    if norm.fract() != 0.0 || !(-MAX_SAFE_INTEGER..=MAX_SAFE_INTEGER).contains(&norm) {
        return Err(ExcelError::Num);
    }
    Ok(norm as i64)
}

/// Normalizes IEEE-754 negative zero (`-0.0`) to canonical positive zero (`0.0`).
#[inline]
#[must_use]
pub fn normalize_zero(val: f64) -> f64 {
    if val == 0.0 { 0.0 } else { val }
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

    /// Returns the [`ValueType`] of this cell, or `None` if this cell is `Empty` or `Error(_)`.
    #[must_use]
    pub const fn value_type(&self) -> Option<ValueType> {
        match self {
            Self::Number(_) => Some(ValueType::Number),
            Self::Text(_) => Some(ValueType::Text),
            Self::Boolean(_) => Some(ValueType::Boolean),
            Self::Empty | Self::Error(_) => None,
        }
    }

    /// Extracts a strict finite numeric scalar without coercing `Text`, `Boolean`, or `Empty`.
    ///
    /// - `Number(n)` -> `Ok(normalize_zero(n))` if finite, else `Err(ExcelError::Num)`
    /// - `Error(e)` -> `Err(e)`
    /// - `Text` / `Boolean` / `Empty` -> `Err(ExcelError::Value)`
    pub fn strict_number(&self) -> Result<f64, ExcelError> {
        match self {
            Self::Number(n) => {
                if n.is_finite() {
                    Ok(normalize_zero(*n))
                } else {
                    Err(ExcelError::Num)
                }
            }
            Self::Error(e) => Err(*e),
            Self::Text(_) | Self::Boolean(_) | Self::Empty => Err(ExcelError::Value),
        }
    }

    /// Extracts a strict exact integer in `-9_007_199_254_740_991..=9_007_199_254_740_991`.
    ///
    /// - `Number(n)` -> `Err(ExcelError::Num)` if non-finite, `fract() != 0.0`, or outside safe integer bounds, else `Ok(n as i64)`
    /// - `Error(e)` -> `Err(e)`
    /// - `Text` / `Boolean` / `Empty` -> `Err(ExcelError::Value)`
    pub fn strict_integer(&self) -> Result<i64, ExcelError> {
        match self {
            Self::Number(n) => f64_to_strict_integer(*n),
            Self::Error(e) => Err(*e),
            Self::Text(_) | Self::Boolean(_) | Self::Empty => Err(ExcelError::Value),
        }
    }

    /// Extracts a strict boolean scalar without coercing `Number`, `Text`, or `Empty`.
    ///
    /// - `Boolean(b)` -> `Ok(b)`
    /// - `Error(e)` -> `Err(e)`
    /// - `Number` / `Text` / `Empty` -> `Err(ExcelError::Value)`
    pub fn strict_bool(&self) -> Result<bool, ExcelError> {
        match self {
            Self::Boolean(b) => Ok(*b),
            Self::Error(e) => Err(*e),
            Self::Number(_) | Self::Text(_) | Self::Empty => Err(ExcelError::Value),
        }
    }

    /// Compares two cell values under strict homogeneous non-`Empty` relational rules.
    ///
    /// - Any `Empty` operand returns `Err(ExcelError::Value)`.
    /// - `Error(e)` paired with any non-`Empty` operand bubbles `Err(e)` (left error first).
    /// - Homogeneous `Number`, `Text` (ASCII case-insensitive), and `Boolean` pairs return `Ok(Ordering)`.
    /// - Mismatched concrete types return `Err(ExcelError::Value)`.
    pub fn compare_relational(&self, other: &Self) -> Result<Ordering, ExcelError> {
        match (self, other) {
            (Self::Empty, _) | (_, Self::Empty) => Err(ExcelError::Value),
            (
                Self::Error(e),
                Self::Number(_) | Self::Text(_) | Self::Boolean(_) | Self::Error(_),
            ) => Err(*e),
            (Self::Number(_) | Self::Text(_) | Self::Boolean(_), Self::Error(e)) => Err(*e),
            (Self::Number(a), Self::Number(b)) => {
                if !a.is_finite() || !b.is_finite() {
                    Err(ExcelError::Num)
                } else {
                    Ok(normalize_zero(*a).total_cmp(&normalize_zero(*b)))
                }
            }
            (Self::Text(a), Self::Text(b)) => {
                Ok(compare_ascii_case_insensitive(a.as_str(), b.as_str()))
            }
            (Self::Boolean(a), Self::Boolean(b)) => Ok(a.cmp(b)),
            (Self::Number(_), Self::Text(_) | Self::Boolean(_))
            | (Self::Text(_), Self::Number(_) | Self::Boolean(_))
            | (Self::Boolean(_), Self::Number(_) | Self::Text(_)) => Err(ExcelError::Value),
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
        assert_eq!(CellValue::Number(12.5).strict_number(), Ok(12.5));
        assert_eq!(
            CellValue::Number(f64::NAN).strict_number(),
            Err(ExcelError::Num)
        );
        assert_eq!(
            CellValue::Error(ExcelError::Div0).strict_number(),
            Err(ExcelError::Div0)
        );
        assert_eq!(
            CellValue::Boolean(true).strict_number(),
            Err(ExcelError::Value)
        );
        assert_eq!(
            CellValue::Text("5".into()).strict_number(),
            Err(ExcelError::Value)
        );
        assert_eq!(CellValue::Empty.strict_number(), Err(ExcelError::Value));

        assert_eq!(CellValue::Number(42.0).strict_integer(), Ok(42));
        assert_eq!(
            CellValue::Number(MAX_SAFE_INTEGER).strict_integer(),
            Ok(9_007_199_254_740_991)
        );
        assert_eq!(
            CellValue::Number(-MAX_SAFE_INTEGER).strict_integer(),
            Ok(-9_007_199_254_740_991)
        );
        assert_eq!(
            CellValue::Number(2.5).strict_integer(),
            Err(ExcelError::Num)
        );
        assert_eq!(
            CellValue::Number(1e20).strict_integer(),
            Err(ExcelError::Num)
        );
        assert_eq!(
            CellValue::Boolean(false).strict_integer(),
            Err(ExcelError::Value)
        );

        assert_eq!(CellValue::Boolean(true).strict_bool(), Ok(true));
        assert_eq!(
            CellValue::Error(ExcelError::Na).strict_bool(),
            Err(ExcelError::Na)
        );
        assert_eq!(CellValue::Number(1.0).strict_bool(), Err(ExcelError::Value));
        assert_eq!(
            CellValue::Text("TRUE".into()).strict_bool(),
            Err(ExcelError::Value)
        );
        assert_eq!(CellValue::Empty.strict_bool(), Err(ExcelError::Value));

        assert_eq!(
            CellValue::Number(10.0).compare_relational(&CellValue::Number(20.0)),
            Ok(Ordering::Less)
        );
        assert_eq!(
            CellValue::Text("ABC".into()).compare_relational(&CellValue::Text("abc".into())),
            Ok(Ordering::Equal)
        );
        assert_eq!(
            CellValue::Number(1.0).compare_relational(&CellValue::Text("1".into())),
            Err(ExcelError::Value)
        );
        assert_eq!(
            CellValue::Error(ExcelError::Div0).compare_relational(&CellValue::Empty),
            Err(ExcelError::Value)
        );
        assert_eq!(
            CellValue::Error(ExcelError::Div0).compare_relational(&CellValue::Number(1.0)),
            Err(ExcelError::Div0)
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
    fn test_finite_constructors() {
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
}
