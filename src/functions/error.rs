/// Structured errors produced by function validation, binding, and execution.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FunctionError {
    /// Function name is not registered in the function catalog.
    #[error("unknown function '{0}'")]
    UnknownFunction(String),

    /// Too few arguments supplied for the function's minimum arity.
    #[error("too few arguments for '{function}': expected at least {expected}, found {found}")]
    TooFewArguments {
        /// Canonical uppercase function identifier.
        function: &'static str,
        /// Expected minimum argument count.
        expected: usize,
        /// Actual argument count found.
        found: usize,
    },

    /// Too many arguments supplied for a non-variadic function.
    #[error("too many arguments for '{function}': expected at most {expected}, found {found}")]
    TooManyArguments {
        /// Canonical uppercase function identifier.
        function: &'static str,
        /// Expected maximum argument count.
        expected: usize,
        /// Actual argument count found.
        found: usize,
    },

    /// An explicit omitted argument placeholder (`_`) was provided for a required parameter.
    #[error("argument '{param}' at position {position} of '{function}' cannot be omitted")]
    CannotOmitRequiredArgument {
        /// Canonical uppercase function identifier.
        function: &'static str,
        /// Parameter identifier.
        param: &'static str,
        /// 1-indexed argument position for user-facing diagnostics.
        position: usize,
    },

    /// The variadic tail of a paired function does not have an even number of arguments.
    #[error("invalid argument count for variadic pairs in '{function}'")]
    InvalidVariadicPairs {
        /// Canonical uppercase function identifier.
        function: &'static str,
        /// Total argument count received.
        count: usize,
    },

    /// An unexpected argument kind was passed to a parameter (e.g. array where scalar required).
    #[error("invalid argument kind for '{function}': expected {expected}, found {found}")]
    InvalidArgumentKind {
        /// Canonical uppercase function identifier.
        function: &'static str,
        /// Expected argument kind description.
        expected: &'static str,
        /// Actual argument kind description.
        found: &'static str,
    },

    /// Internal evaluation failure during expression resolution.
    #[error("internal evaluation error in '{function}': {message}")]
    EvaluationFailure {
        /// Canonical uppercase function identifier.
        function: &'static str,
        /// Descriptive failure diagnostic message.
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_error_display() {
        let err = FunctionError::UnknownFunction("FOOBAR".into());
        assert_eq!(err.to_string(), "unknown function 'FOOBAR'");

        let err = FunctionError::TooFewArguments {
            function: "ABS",
            expected: 1,
            found: 0,
        };
        assert_eq!(
            err.to_string(),
            "too few arguments for 'ABS': expected at least 1, found 0"
        );

        let err = FunctionError::TooManyArguments {
            function: "ABS",
            expected: 1,
            found: 2,
        };
        assert_eq!(
            err.to_string(),
            "too many arguments for 'ABS': expected at most 1, found 2"
        );

        let err = FunctionError::CannotOmitRequiredArgument {
            function: "ABS",
            param: "number",
            position: 1,
        };
        assert_eq!(
            err.to_string(),
            "argument 'number' at position 1 of 'ABS' cannot be omitted"
        );

        let err = FunctionError::InvalidVariadicPairs {
            function: "SUMIFS",
            count: 2,
        };
        assert_eq!(
            err.to_string(),
            "invalid argument count for variadic pairs in 'SUMIFS'"
        );

        let err = FunctionError::InvalidArgumentKind {
            function: "ABS",
            expected: "scalar",
            found: "array",
        };
        assert_eq!(
            err.to_string(),
            "invalid argument kind for 'ABS': expected scalar, found array"
        );

        let err = FunctionError::EvaluationFailure {
            function: "IF",
            message: "variable not found".into(),
        };
        assert_eq!(
            err.to_string(),
            "internal evaluation error in 'IF': variable not found"
        );
    }
}
