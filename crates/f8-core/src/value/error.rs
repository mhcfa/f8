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

/// Non-error runtime value categories for static and dynamic type diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValueType {
    /// Numeric scalar (`f64`).
    Number,
    /// Text string scalar.
    Text,
    /// Boolean scalar (`TRUE` or `FALSE`).
    Boolean,
}

impl ValueType {
    /// Returns the lowercase diagnostic label for this value type.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Number => "number",
            Self::Text => "text",
            Self::Boolean => "boolean",
        }
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Static and dynamic formula type errors raised prior to workbook emission.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TypeError {
    /// Formula cell evaluated to `CellValue::Empty` at the cell boundary.
    #[error(
        "formula cannot evaluate to an empty cell; guard nullable references explicitly with IF(ISBLANK(...), ...)"
    )]
    EmptyFormulaResult,
    /// Unary or binary operator received `CellValue::Empty`.
    #[error("operand for '{operator}' evaluated to an empty cell")]
    EmptyOperand {
        /// Operator token (`"-"`, `"+"`, `"="`, etc.).
        operator: &'static str,
    },
    /// Function parameter with `EmptyPolicy::Forbid` or `EmptyPolicy::AllowInSlice` received `CellValue::Empty`.
    #[error(
        "argument for parameter '{param}' at position {position} of '{function}' evaluated to an empty cell"
    )]
    EmptyArgument {
        /// Canonical uppercase function name.
        function: &'static str,
        /// Parameter name.
        param: &'static str,
        /// 1-indexed argument position.
        position: usize,
    },
    /// Unary or binary operator received an operand of an invalid type.
    #[error("invalid operand type for '{operator}': expected {expected}, found {found}")]
    InvalidOperandType {
        /// Operator token (`"-"`, `"+"`, etc.).
        operator: &'static str,
        /// Expected operand value type.
        expected: ValueType,
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
        /// Expected parameter value type.
        expected: ValueType,
        /// Actual argument value type.
        found: ValueType,
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
    fn test_excel_error_type_codes() {
        assert_eq!(ExcelError::Div0.error_type_code(), 2.0);
        assert_eq!(ExcelError::Value.error_type_code(), 3.0);
        assert_eq!(ExcelError::Ref.error_type_code(), 4.0);
        assert_eq!(ExcelError::Name.error_type_code(), 5.0);
        assert_eq!(ExcelError::Num.error_type_code(), 6.0);
        assert_eq!(ExcelError::Na.error_type_code(), 7.0);
    }
}
