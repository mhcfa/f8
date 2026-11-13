//! Runtime value representations and Excel parity calculation semantics.
//!
//! Defines `CellValue`, `ExcelError`, and calculation/coercion rules.

pub mod date;
pub mod error;
pub mod format;

pub use date::{
    DateSerial, DateTuple, EXCEL_1900_LEAP_DAY_SERIAL, MAX_EXCEL_SERIAL, MIN_INPUT_DATE_SERIAL,
    UNIX_EPOCH_SERIAL, is_valid_input_date_serial, validate_date_serial,
};
pub use error::{ExcelError, ExpectedType, ParseExcelError, StrictIntError, TypeError, ValueType};
pub use format::{format_accounting_number, format_cell_value};

use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::expr::ast::Literal;
use crate::model::table::ValueLiteral;

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
}
