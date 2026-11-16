use crate::model::id::{ColId, RowId, TableId};
use crate::value::TypeError;

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

    /// An explicit omitted argument placeholder (`_`) was provided in a trailing optional position.
    #[error(
        "trailing argument '{param}' at position {position} of '{function}' cannot use '_'; omit trailing optional arguments instead"
    )]
    CannotOmitTrailingArgument {
        /// Canonical uppercase function identifier.
        function: &'static str,
        /// Parameter identifier.
        param: &'static str,
        /// 1-indexed argument position for user-facing diagnostics.
        position: usize,
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

    /// Static or dynamic formula type violation.
    #[error(transparent)]
    Type(#[from] TypeError),

    /// Range slice evaluated in a scalar parameter or expression context.
    #[error("range slice ({rows}x{cols}) cannot be evaluated as a scalar")]
    SliceInScalarContext {
        /// Resolved row count of the slice.
        rows: usize,
        /// Resolved column count of the slice.
        cols: usize,
    },

    /// Parallel range arguments have incompatible `(rows, cols)` dimensions.
    #[error(
        "dimension mismatch in '{function}': expected {expected_rows}x{expected_cols}, found {found_rows}x{found_cols}"
    )]
    DimensionMismatch {
        /// Canonical uppercase function identifier.
        function: &'static str,
        /// Expected row count.
        expected_rows: usize,
        /// Expected column count.
        expected_cols: usize,
        /// Actual row count.
        found_rows: usize,
        /// Actual column count.
        found_cols: usize,
    },

    /// Parallel 1D slice arguments have unequal element counts.
    #[error("length mismatch in '{function}': expected {expected}, found {found}")]
    LengthMismatch {
        /// Canonical uppercase function identifier.
        function: &'static str,
        /// Expected element count.
        expected: usize,
        /// Actual element count.
        found: usize,
    },

    /// A 1D slice parameter received a 2D slice (`rows > 1 && cols > 1`).
    #[error("parameter '{param}' of '{function}' requires a 1D slice, found {rows}x{cols}")]
    ExpectedOneDimensionalSlice {
        /// Canonical uppercase function identifier.
        function: &'static str,
        /// Parameter identifier.
        param: &'static str,
        /// Actual row count.
        rows: usize,
        /// Actual column count.
        cols: usize,
    },

    /// 2-argument `INDEX` called on a 2D slice (`rows > 1 && cols > 1`) without `col_num`.
    #[error("'{function}' on a 2D slice ({rows}x{cols}) requires 'col_num'")]
    MissingColumnIndexFor2DSlice {
        /// Canonical uppercase function identifier.
        function: &'static str,
        /// Slice row count.
        rows: usize,
        /// Slice column count.
        cols: usize,
    },

    /// A `RolePolicy::MeasureOnly` parameter received a reference to an `AxisRole::Descriptor` column.
    #[error(
        "parameter '{param}' at position {position} of '{function}' cannot reference descriptor column"
    )]
    DescriptorReferenceNotAllowed {
        /// Canonical uppercase function identifier.
        function: &'static str,
        /// Parameter identifier.
        param: &'static str,
        /// 1-indexed argument position.
        position: usize,
    },

    /// A referenced cell coordinate has not yet been evaluated in the current evaluation context.
    #[error("referenced cell '{table}[\"{row}\", \"{col}\"]' has not been evaluated")]
    UnevaluatedCell {
        /// Referenced table identifier.
        table: TableId,
        /// Referenced row identifier.
        row: RowId,
        /// Referenced column identifier.
        col: ColId,
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

        let err = FunctionError::CannotOmitTrailingArgument {
            function: "IF",
            param: "value_if_false",
            position: 3,
        };
        assert_eq!(
            err.to_string(),
            "trailing argument 'value_if_false' at position 3 of 'IF' cannot use '_'; omit trailing optional arguments instead"
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
