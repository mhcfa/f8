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
}
