//! Runtime value representations and Excel parity calculation semantics.
//!
//! Defines `CellValue`, `ExcelError`, and calculation/coercion rules.

pub mod error;
pub mod format;

pub use error::{ExcelError, ParseExcelError};
pub use format::{format_accounting_number, format_cell_value};

use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::expr::ast::Literal;
use crate::model::table::ValueLiteral;

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

    /// Coerces this cell value to a number according to Excel scalar coercion semantics:
    /// - `Number(n)` -> `Ok(n)`
    /// - `Boolean(true)` -> `Ok(1.0)`, `Boolean(false)` -> `Ok(0.0)`
    /// - `Empty` -> `Ok(0.0)`
    /// - `Error(e)` -> `Err(e)` (bubbles up existing error)
    /// - `Text(_)` -> `Err(ExcelError::Value)`
    pub fn coerce_to_number(&self) -> Result<f64, ExcelError> {
        match self {
            Self::Number(n) => Ok(*n),
            Self::Boolean(true) => Ok(1.0),
            Self::Boolean(false) => Ok(0.0),
            Self::Empty => Ok(0.0),
            Self::Error(e) => Err(*e),
            Self::Text(_) => Err(ExcelError::Value),
        }
    }

    /// Coerces this cell value to a boolean according to Excel scalar coercion semantics:
    /// - `Boolean(b)` -> `Ok(b)`
    /// - `Number(n)` -> `Ok(n != 0.0)`
    /// - `Empty` -> `Ok(false)`
    /// - `Error(e)` -> `Err(e)` (bubbles up existing error)
    /// - `Text("true" / "false")` (case-insensitive) -> `Ok(true / false)`
    /// - `Text(_)` (other strings) -> `Err(ExcelError::Value)`
    pub fn coerce_to_bool(&self) -> Result<bool, ExcelError> {
        match self {
            Self::Boolean(b) => Ok(*b),
            Self::Number(n) => Ok(*n != 0.0),
            Self::Empty => Ok(false),
            Self::Error(e) => Err(*e),
            Self::Text(s) => {
                if s.eq_ignore_ascii_case("true") {
                    Ok(true)
                } else if s.eq_ignore_ascii_case("false") {
                    Ok(false)
                } else {
                    Err(ExcelError::Value)
                }
            }
        }
    }

    /// Compares two cell values conforming to Excel pairwise relational comparison rules:
    /// - Same types: standard comparison (strings compare case-insensitively using ASCII case folding).
    /// - Empty cell coercions: `Empty` coerces to the other operand's neutral identity (`0.0` vs `Number`, `""` vs `Text`, `false` vs `Boolean`).
    /// - Mixed non-empty types: `Number < Text < Boolean < Error`.
    /// - Pairwise comparator: not a transitive total order across heterogeneous sets containing `Empty`.
    #[must_use]
    pub fn excel_cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Empty, Self::Empty) => Ordering::Equal,

            // Empty vs Number coerces Empty to 0.0
            (Self::Empty, Self::Number(n)) => 0.0_f64.total_cmp(&normalize_zero(*n)),
            (Self::Number(n), Self::Empty) => normalize_zero(*n).total_cmp(&0.0_f64),

            // Empty vs Text coerces Empty to ""
            (Self::Empty, Self::Text(s)) => compare_ascii_case_insensitive("", s.as_str()),
            (Self::Text(s), Self::Empty) => compare_ascii_case_insensitive(s.as_str(), ""),

            // Empty vs Boolean coerces Empty to false
            (Self::Empty, Self::Boolean(b)) => false.cmp(b),
            (Self::Boolean(b), Self::Empty) => b.cmp(&false),

            // Same type comparisons
            (Self::Number(a), Self::Number(b)) => normalize_zero(*a).total_cmp(&normalize_zero(*b)),
            (Self::Text(a), Self::Text(b)) => {
                compare_ascii_case_insensitive(a.as_str(), b.as_str())
            }
            (Self::Boolean(a), Self::Boolean(b)) => a.cmp(b),
            (Self::Error(a), Self::Error(b)) => a.as_str().cmp(b.as_str()),

            // Mixed type comparisons: Number < Text < Boolean < Error
            (Self::Number(_), Self::Text(_)) => Ordering::Less,
            (Self::Text(_), Self::Number(_)) => Ordering::Greater,

            (Self::Number(_), Self::Boolean(_)) => Ordering::Less,
            (Self::Boolean(_), Self::Number(_)) => Ordering::Greater,

            (Self::Text(_), Self::Boolean(_)) => Ordering::Less,
            (Self::Boolean(_), Self::Text(_)) => Ordering::Greater,

            (Self::Error(_), Self::Number(_) | Self::Text(_) | Self::Boolean(_) | Self::Empty) => {
                Ordering::Greater
            }
            (Self::Number(_) | Self::Text(_) | Self::Boolean(_) | Self::Empty, Self::Error(_)) => {
                Ordering::Less
            }
        }
    }

    /// Tests equality conforming to Excel case-insensitive string and empty coercion rules.
    #[must_use]
    pub fn excel_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Error(a), Self::Error(b)) => a == b,
            (Self::Error(_), Self::Number(_) | Self::Text(_) | Self::Boolean(_) | Self::Empty)
            | (Self::Number(_) | Self::Text(_) | Self::Boolean(_) | Self::Empty, Self::Error(_)) => {
                false
            }
            (
                Self::Number(_) | Self::Text(_) | Self::Boolean(_) | Self::Empty,
                Self::Number(_) | Self::Text(_) | Self::Boolean(_) | Self::Empty,
            ) => self.excel_cmp(other) == Ordering::Equal,
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
    fn test_cell_value_coercions() {
        assert_eq!(CellValue::Number(12.5).coerce_to_number(), Ok(12.5));
        assert_eq!(CellValue::Boolean(true).coerce_to_number(), Ok(1.0));
        assert_eq!(CellValue::Boolean(false).coerce_to_number(), Ok(0.0));
        assert_eq!(CellValue::Empty.coerce_to_number(), Ok(0.0));
        assert_eq!(
            CellValue::Text("foo".into()).coerce_to_number(),
            Err(ExcelError::Value)
        );
        assert_eq!(
            CellValue::Error(ExcelError::Ref).coerce_to_number(),
            Err(ExcelError::Ref)
        );

        assert_eq!(CellValue::Boolean(true).coerce_to_bool(), Ok(true));
        assert_eq!(CellValue::Number(1.0).coerce_to_bool(), Ok(true));
        assert_eq!(CellValue::Number(0.0).coerce_to_bool(), Ok(false));
        assert_eq!(CellValue::Empty.coerce_to_bool(), Ok(false));
        assert_eq!(CellValue::Text("true".into()).coerce_to_bool(), Ok(true));
        assert_eq!(CellValue::Text("TRUE".into()).coerce_to_bool(), Ok(true));
        assert_eq!(CellValue::Text("false".into()).coerce_to_bool(), Ok(false));
        assert_eq!(CellValue::Text("FALSE".into()).coerce_to_bool(), Ok(false));
        assert_eq!(
            CellValue::Text("foo".into()).coerce_to_bool(),
            Err(ExcelError::Value)
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
    fn test_excel_cmp_and_excel_eq() {
        use std::cmp::Ordering;

        // Strings case-insensitivity
        assert_eq!(
            CellValue::Text("ABC".into()).excel_cmp(&CellValue::Text("abc".into())),
            Ordering::Equal
        );
        assert!(CellValue::Text("ABC".into()).excel_eq(&CellValue::Text("abc".into())));
        assert!(!CellValue::Text("ABC".into()).excel_eq(&CellValue::Text("def".into())));

        // Empty vs Number
        assert_eq!(
            CellValue::Empty.excel_cmp(&CellValue::Number(0.0)),
            Ordering::Equal
        );
        assert!(CellValue::Empty.excel_eq(&CellValue::Number(0.0)));

        // Empty vs Text
        assert_eq!(
            CellValue::Empty.excel_cmp(&CellValue::Text("".into())),
            Ordering::Equal
        );
        assert!(CellValue::Empty.excel_eq(&CellValue::Text("".into())));

        // Empty vs Boolean coerces Empty to false
        assert_eq!(
            CellValue::Empty.excel_cmp(&CellValue::Boolean(false)),
            Ordering::Equal
        );
        assert_eq!(
            CellValue::Boolean(false).excel_cmp(&CellValue::Empty),
            Ordering::Equal
        );
        assert_eq!(
            CellValue::Empty.excel_cmp(&CellValue::Boolean(true)),
            Ordering::Less
        );
        assert_eq!(
            CellValue::Boolean(true).excel_cmp(&CellValue::Empty),
            Ordering::Greater
        );
        assert!(CellValue::Empty.excel_eq(&CellValue::Boolean(false)));
        assert!(CellValue::Boolean(false).excel_eq(&CellValue::Empty));
        assert!(!CellValue::Empty.excel_eq(&CellValue::Boolean(true)));
        assert!(!CellValue::Boolean(true).excel_eq(&CellValue::Empty));

        // Type ranking: Number < Text < Boolean (including Text("") < Boolean(false))
        assert_eq!(
            CellValue::Number(100.0).excel_cmp(&CellValue::Text("1".into())),
            Ordering::Less
        );
        assert_eq!(
            CellValue::Text("".into()).excel_cmp(&CellValue::Boolean(false)),
            Ordering::Less
        );
        assert_eq!(
            CellValue::Boolean(false).excel_cmp(&CellValue::Text("".into())),
            Ordering::Greater
        );
        assert_eq!(
            CellValue::Text("xyz".into()).excel_cmp(&CellValue::Boolean(false)),
            Ordering::Less
        );
        assert!(!CellValue::Number(1.0).excel_eq(&CellValue::Text("1".into())));

        // Negative zero normalizes to positive zero in total-order comparison
        assert_eq!(
            CellValue::Number(-0.0).excel_cmp(&CellValue::Number(0.0)),
            Ordering::Equal
        );
        assert!(CellValue::Number(-0.0).excel_eq(&CellValue::Number(0.0)));
        assert_eq!(
            CellValue::Empty.excel_cmp(&CellValue::Number(-0.0)),
            Ordering::Equal
        );

        // Non-finite floats never compare equal to finite numbers or Empty
        assert_ne!(
            CellValue::Number(f64::NAN).excel_cmp(&CellValue::Number(42.0)),
            Ordering::Equal
        );
        assert!(!CellValue::Number(f64::NAN).excel_eq(&CellValue::Number(42.0)));
        assert_ne!(
            CellValue::Empty.excel_cmp(&CellValue::Number(f64::NAN)),
            Ordering::Equal
        );
        assert!(!CellValue::Empty.excel_eq(&CellValue::Number(f64::NAN)));
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
