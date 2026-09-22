use thiserror::Error;

/// Structured diagnostic errors produced during formula expression parsing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    /// Formula ended prematurely while expecting more input.
    #[error("Unexpected end of formula, expected {expected}")]
    UnexpectedEof {
        /// Expected syntactic construct.
        expected: &'static str,
    },

    /// An unexpected token was encountered.
    #[error("Unexpected token at offset {offset}: expected {expected}, found '{found}'")]
    UnexpectedToken {
        /// Expected syntactic construct.
        expected: &'static str,
        /// Actual token found.
        found: String,
        /// Byte offset in the formula string.
        offset: usize,
    },

    /// A numeric literal failed to parse as a valid number.
    #[error("Invalid number literal at offset {offset}: '{raw}'")]
    InvalidNumber {
        /// The raw unparsed text.
        raw: String,
        /// Byte offset in the formula string.
        offset: usize,
    },

    /// Function identifier was not uppercase.
    #[error("Invalid function name '{name}' at offset {offset}: must be uppercase [A-Z][A-Z0-9_]*")]
    InvalidFunctionName {
        /// The invalid function name.
        name: String,
        /// Byte offset in the formula string.
        offset: usize,
    },

    /// A coordinate key inside brackets was not quoted.
    #[error(
        "Coordinate key at offset {offset} must be an explicit double-quoted string: '{found}'"
    )]
    UnquotedCoordinateKey {
        /// The unquoted token.
        found: String,
        /// Byte offset in the formula string.
        offset: usize,
    },

    /// Trailing characters were left over after parsing a complete formula.
    #[error("Trailing characters at offset {offset}: '{trailing}'")]
    TrailingInput {
        /// The unparsed trailing slice.
        trailing: String,
        /// Byte offset in the formula string.
        offset: usize,
    },

    /// Bare '@' was encountered instead of '@col' or '@row'.
    #[error(
        "Bare '@' at offset {offset} is retired; specify active axis explicitly using '@col' or '@row'"
    )]
    RetiredBareAt {
        /// Byte offset in the formula string.
        offset: usize,
    },
}
