//! Runtime value representations and Excel parity calculation semantics.
//!
//! Defines `CellValue`, `ExcelError`, and coercion rules documented in
//! `docs/compiler/evaluator.md` and `docs/compiler/parity.md`.

pub mod error;

pub use error::ExcelError;

use crate::expr::ast::Literal;
use crate::model::table::ValueLiteral;
use serde::{Deserialize, Serialize};
use std::fmt;

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
    /// Returns the contained numeric value if this cell is `Number`.
    #[must_use]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Returns the contained boolean value if this cell is `Boolean`.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(b) => Some(*b),
            _ => None,
        }
    }

    /// Returns the contained text slice if this cell is `Text`.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s.as_str()),
            _ => None,
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
            _ => None,
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

    /// Compares two cell values conforming to Excel type ranking and equality rules:
    /// - Same types: standard comparison (strings compare case-insensitively using ASCII case folding).
    /// - Empty cell coercions: Empty vs Number coerces to 0.0; Empty vs Text coerces to ""; Empty is less than Boolean.
    /// - Mixed types: Number < Text < Boolean < Error.
    #[must_use]
    pub fn excel_cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self, other) {
            (Self::Empty, Self::Empty) => Ordering::Equal,

            // Empty vs Number coerces Empty to 0.0
            (Self::Empty, Self::Number(n)) => 0.0f64.partial_cmp(n).unwrap_or(Ordering::Equal),
            (Self::Number(n), Self::Empty) => n.partial_cmp(&0.0f64).unwrap_or(Ordering::Equal),

            // Empty vs Text coerces Empty to ""
            (Self::Empty, Self::Text(s)) => compare_ascii_case_insensitive("", s.as_str()),
            (Self::Text(s), Self::Empty) => compare_ascii_case_insensitive(s.as_str(), ""),

            // Empty vs Boolean: Empty ranks lower than any Boolean
            (Self::Empty, Self::Boolean(_)) => Ordering::Less,
            (Self::Boolean(_), Self::Empty) => Ordering::Greater,

            // Same type comparisons
            (Self::Number(a), Self::Number(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
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

            (Self::Error(_), _) => Ordering::Greater,
            (_, Self::Error(_)) => Ordering::Less,
        }
    }

    /// Tests equality conforming to Excel case-insensitive string and empty coercion rules.
    #[must_use]
    pub fn excel_eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Error(a), Self::Error(b)) => a == b,
            (Self::Error(_), _) | (_, Self::Error(_)) => false,
            _ => self.excel_cmp(other) == std::cmp::Ordering::Equal,
        }
    }
}

/// Compares two strings case-insensitively using ASCII case folding.
fn compare_ascii_case_insensitive(a: &str, b: &str) -> std::cmp::Ordering {
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
            ValueLiteral::Number(n) => Self::Number(n),
            ValueLiteral::String(s) => Self::Text(s),
            ValueLiteral::Boolean(b) => Self::Boolean(b),
            ValueLiteral::Null => Self::Empty,
        }
    }
}

impl From<&ValueLiteral> for CellValue {
    fn from(lit: &ValueLiteral) -> Self {
        match lit {
            ValueLiteral::Number(n) => Self::Number(*n),
            ValueLiteral::String(s) => Self::Text(s.clone()),
            ValueLiteral::Boolean(b) => Self::Boolean(*b),
            ValueLiteral::Null => Self::Empty,
        }
    }
}

impl From<Literal> for CellValue {
    fn from(lit: Literal) -> Self {
        match lit {
            Literal::Number(n) => Self::Number(n),
            Literal::String(s) => Self::Text(s),
            Literal::Boolean(b) => Self::Boolean(b),
        }
    }
}

impl From<&Literal> for CellValue {
    fn from(lit: &Literal) -> Self {
        match lit {
            Literal::Number(n) => Self::Number(*n),
            Literal::String(s) => Self::Text(s.clone()),
            Literal::Boolean(b) => Self::Boolean(*b),
        }
    }
}

impl From<f64> for CellValue {
    fn from(n: f64) -> Self {
        Self::Number(n)
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

        // Empty vs Boolean
        assert_eq!(
            CellValue::Empty.excel_cmp(&CellValue::Boolean(false)),
            Ordering::Less
        );
        assert!(!CellValue::Empty.excel_eq(&CellValue::Boolean(false)));

        // Type ranking: Number < Text < Boolean
        assert_eq!(
            CellValue::Number(100.0).excel_cmp(&CellValue::Text("1".into())),
            Ordering::Less
        );
        assert_eq!(
            CellValue::Text("xyz".into()).excel_cmp(&CellValue::Boolean(false)),
            Ordering::Less
        );
        assert!(!CellValue::Number(1.0).excel_eq(&CellValue::Text("1".into())));
    }
}
