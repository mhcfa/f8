use std::fmt;

use serde::{Deserialize, Serialize};

/// Excel domain calculation errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, thiserror::Error)]
pub enum ExcelError {
    /// Division by zero (`#DIV/0!`).
    #[error("#DIV/0!")]
    Div0,
    /// Invalid value type or conversion failure (`#VALUE!`).
    #[error("#VALUE!")]
    Value,
    /// Invalid cell or range reference (`#REF!`).
    #[error("#REF!")]
    Ref,
    /// Numeric overflow, underflow, or domain error (`#NUM!`).
    #[error("#NUM!")]
    Num,
    /// Value not available (`#N/A`).
    #[error("#N/A")]
    Na,
    /// Unrecognized function or range name (`#NAME?`).
    #[error("#NAME?")]
    Name,
}

impl ExcelError {
    /// Returns the canonical Excel error string literal (e.g. `"#DIV/0!"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Div0 => "#DIV/0!",
            Self::Value => "#VALUE!",
            Self::Ref => "#REF!",
            Self::Num => "#NUM!",
            Self::Na => "#N/A",
            Self::Name => "#NAME?",
        }
    }

    /// Returns the numeric `ERROR.TYPE` code for this Excel error (`2.0..=7.0`).
    #[must_use]
    pub const fn error_type_code(self) -> f64 {
        match self {
            Self::Div0 => 2.0,
            Self::Value => 3.0,
            Self::Ref => 4.0,
            Self::Name => 5.0,
            Self::Num => 6.0,
            Self::Na => 7.0,
        }
    }
}

/// Error returned when parsing an unrecognized Excel error literal.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unrecognized Excel error literal: {0}")]
pub struct ParseExcelError(pub String);

impl std::str::FromStr for ExcelError {
    type Err = ParseExcelError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("#div/0!") {
            Ok(Self::Div0)
        } else if s.eq_ignore_ascii_case("#value!") {
            Ok(Self::Value)
        } else if s.eq_ignore_ascii_case("#ref!") {
            Ok(Self::Ref)
        } else if s.eq_ignore_ascii_case("#num!") {
            Ok(Self::Num)
        } else if s.eq_ignore_ascii_case("#n/a") {
            Ok(Self::Na)
        } else if s.eq_ignore_ascii_case("#name?") {
            Ok(Self::Name)
        } else {
            Err(ParseExcelError(s.to_string()))
        }
    }
}

/// Non-error runtime value categories for static and dynamic type diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValueType {
    /// Numeric scalar (`f64`).
    Number,
    /// Text string scalar.
    Text,
    /// Boolean scalar (`TRUE` or `FALSE`).
    Boolean,
    /// Empty / null cell (`ValueLiteral::Null`).
    Empty,
}

impl ValueType {
    /// Returns the lowercase diagnostic label for this value type.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Number => "number",
            Self::Text => "text",
            Self::Boolean => "boolean",
            Self::Empty => "empty",
        }
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Expected type categories for operator operands and function parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExpectedType {
    /// Any non-empty scalar (`Number`, `Text`, or `Boolean`).
    NonEmpty,
    /// Finite numeric scalar.
    Number,
    /// Exact integer within the IEEE-754 safe integer range.
    Integer,
    /// Text string scalar.
    Text,
    /// Boolean scalar.
    Boolean,
}

impl ExpectedType {
    /// Returns the lowercase diagnostic label for this expected type.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NonEmpty => "non-empty value",
            Self::Number => "number",
            Self::Integer => "integer",
            Self::Text => "text",
            Self::Boolean => "boolean",
        }
    }

    /// Maps a concrete [`ValueType`] into its matching [`ExpectedType`], or `None` for [`ValueType::Empty`].
    #[must_use]
    pub const fn from_value_type(vt: ValueType) -> Option<Self> {
        match vt {
            ValueType::Number => Some(Self::Number),
            ValueType::Text => Some(Self::Text),
            ValueType::Boolean => Some(Self::Boolean),
            ValueType::Empty => None,
        }
    }
}

impl fmt::Display for ExpectedType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Failure modes when extracting a strict integer from a [`crate::value::CellValue`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrictIntError {
    /// Value had a non-numeric [`ValueType`] (`Text`, `Boolean`, or `Empty`).
    WrongType(ValueType),
    /// Numeric value had a non-zero fractional part or exceeded safe integer bounds.
    NonInteger(String),
}

/// Static and dynamic formula type errors raised prior to workbook emission.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TypeError {
    /// Formula cell evaluated to `CellValue::Empty` at the cell boundary.
    #[error(
        "formula cannot evaluate to an empty cell; guard nullable references explicitly with IF(ISBLANK(...), ...)"
    )]
    EmptyFormulaResult,
    /// Unary or binary operator received an operand of an invalid type.
    #[error("invalid operand type for '{operator}': expected {expected}, found {found}")]
    InvalidOperandType {
        /// Operator token (`"-"`, `"+"`, `"="`, etc.).
        operator: &'static str,
        /// Expected operand type.
        expected: ExpectedType,
        /// Actual operand value type.
        found: ValueType,
    },
    /// Relational comparison received operands of incompatible types.
    #[error("cannot compare '{operator}' operands of types {left} and {right}")]
    IncompatibleComparison {
        /// Relational operator token (`"="`, `"<>"`, `"<"`, `"<="`, `">"`, `">="`).
        operator: &'static str,
        /// Left operand value type.
        left: ValueType,
        /// Right operand value type.
        right: ValueType,
    },
    /// Function parameter received an argument of an invalid type.
    #[error(
        "invalid argument type for parameter '{param}' at position {position} of '{function}': expected {expected}, found {found}"
    )]
    InvalidArgumentType {
        /// Canonical uppercase function name.
        function: &'static str,
        /// Parameter name.
        param: &'static str,
        /// 1-indexed argument position.
        position: usize,
        /// Expected parameter type.
        expected: ExpectedType,
        /// Actual argument value type.
        found: ValueType,
    },
    /// Integer parameter received a fractional or out-of-safe-range numeric value.
    #[error(
        "invalid argument for parameter '{param}' at position {position} of '{function}': expected integer, found {value}"
    )]
    NonIntegerArgument {
        /// Canonical uppercase function name.
        function: &'static str,
        /// Parameter name.
        param: &'static str,
        /// 1-indexed argument position.
        position: usize,
        /// Formatted offending numeric value.
        value: String,
    },
    /// Branching or error-handling function received result arms of incompatible types.
    #[error(
        "incompatible branch return types in '{function}' at position {position}: expected {expected}, found {found}"
    )]
    IncompatibleBranchTypes {
        /// Canonical uppercase function name.
        function: &'static str,
        /// 1-indexed arm position.
        position: usize,
        /// Expected unified branch return type.
        expected: ValueType,
        /// Actual arm value type.
        found: ValueType,
    },
}

/// Validation failure reasons for `Text` criteria in conditional aggregation (`*IFS`) functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TextCriterionError {
    /// Empty string `""` passed instead of `BLANK` or `NOT_BLANK`.
    #[error("empty string criterion \"\" is ambiguous in Excel *IFS; use BLANK or NOT_BLANK")]
    EmptyString,
    /// Contains Excel wildcard or escape characters (`*`, `?`, `~`).
    #[error("text criterion cannot contain Excel wildcard or escape characters ('*', '?', '~')")]
    ContainsWildcardOrEscape,
    /// Starts with a comparison operator character (`=`, `<`, `>`).
    #[error(
        "text criterion cannot start with comparison operator ('=', '<', '>'); use a typed comparator expression"
    )]
    LeadingOperatorPrefix,
    /// Criterion exceeds Excel's 255-UTF-16-code-unit `*IFS` limit.
    #[error("text criterion exceeds Excel's 255-character *IFS limit")]
    TooLong,
    /// Criterion trims/parses as a number, percentage, currency, grouped/parenthesized number, or mixed fraction in Excel `*IFS`.
    #[error("text criterion parses as a number, percentage, currency, or fraction in Excel *IFS")]
    CoercibleNumericString,
    /// Criterion trims/parses as a date or time in Excel `*IFS`.
    #[error(
        "text criterion parses as a date or time in Excel *IFS; use numeric DATE serials instead"
    )]
    CoercibleDateOrTimeString,
    /// Criterion trims/parses as a boolean in Excel `*IFS`.
    #[error("text criterion parses as a boolean in Excel *IFS and would fail to match text")]
    CoercibleBooleanString,
    /// Criterion trims/parses as an Excel error literal in `*IFS`.
    #[error(
        "text criterion parses as an Excel error literal in *IFS; use ISERROR, ISNA, or ERROR.TYPE for error checks"
    )]
    CoercibleErrorString,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_excel_error_display() {
        assert_eq!(ExcelError::Div0.to_string(), "#DIV/0!");
        assert_eq!(ExcelError::Value.to_string(), "#VALUE!");
        assert_eq!(ExcelError::Ref.to_string(), "#REF!");
        assert_eq!(ExcelError::Num.to_string(), "#NUM!");
        assert_eq!(ExcelError::Na.to_string(), "#N/A");
        assert_eq!(ExcelError::Name.to_string(), "#NAME?");
    }

    #[test]
    fn test_excel_error_from_str() {
        assert_eq!("#div/0!".parse::<ExcelError>(), Ok(ExcelError::Div0));
        assert_eq!("#VALUE!".parse::<ExcelError>(), Ok(ExcelError::Value));
        assert_eq!("#ref!".parse::<ExcelError>(), Ok(ExcelError::Ref));
        assert_eq!("#NUM!".parse::<ExcelError>(), Ok(ExcelError::Num));
        assert_eq!("#N/A".parse::<ExcelError>(), Ok(ExcelError::Na));
        assert_eq!("#NAME?".parse::<ExcelError>(), Ok(ExcelError::Name));
        assert!("#UNKNOWN!".parse::<ExcelError>().is_err());
    }

    #[test]
    fn test_excel_error_type_codes() {
        assert_eq!(ExcelError::Div0.error_type_code(), 2.0);
        assert_eq!(ExcelError::Value.error_type_code(), 3.0);
        assert_eq!(ExcelError::Ref.error_type_code(), 4.0);
        assert_eq!(ExcelError::Name.error_type_code(), 5.0);
        assert_eq!(ExcelError::Num.error_type_code(), 6.0);
        assert_eq!(ExcelError::Na.error_type_code(), 7.0);
    }
}
