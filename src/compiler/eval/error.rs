use thiserror::Error;

use crate::compiler::graph::ResolveError;
use crate::compiler::ir::id::CellId;
use crate::functions::error::FunctionError;

/// Structured compiler and evaluation errors encountered during formula calculation.
#[derive(Debug, Error)]
pub enum EvalError {
    /// The specified cell coordinate was not found in the cell grid.
    #[error("Cell '{0}' not found in cell grid")]
    CellNotFound(Box<CellId>),

    /// A referenced dependency cell has not been evaluated prior to this cell.
    #[error("Referenced cell '{0}' has not been evaluated (dependency ordering anomaly)")]
    UnevaluatedDependency(Box<CellId>),

    /// Table reference resolution failed against model metadata or symbol table.
    #[error("Table reference resolution error: {0}")]
    Resolve(#[from] ResolveError),

    /// A built-in or user-defined Excel function evaluation failed.
    #[error("Function error in cell '{cell}': {source}")]
    Function {
        /// Cell coordinate where the function failure occurred.
        cell: Box<CellId>,
        /// Underlying function error.
        #[source]
        source: Box<FunctionError>,
    },

    /// The omitted argument placeholder `_` appeared outside a function call.
    #[error("Unexpected omitted argument placeholder '_' in cell '{0}'")]
    UnexpectedOmitted(Box<CellId>),
}

impl EvalError {
    /// Creates a `CellNotFound` error.
    #[must_use]
    pub fn cell_not_found(cell: CellId) -> Self {
        Self::CellNotFound(Box::new(cell))
    }

    /// Creates an `UnevaluatedDependency` error.
    #[must_use]
    pub fn unevaluated_dependency(cell: CellId) -> Self {
        Self::UnevaluatedDependency(Box::new(cell))
    }

    /// Creates a `Function` evaluation error.
    #[must_use]
    pub fn function(cell: CellId, source: FunctionError) -> Self {
        Self::Function {
            cell: Box::new(cell),
            source: Box::new(source),
        }
    }

    /// Creates an `UnexpectedOmitted` error.
    #[must_use]
    pub fn unexpected_omitted(cell: CellId) -> Self {
        Self::UnexpectedOmitted(Box::new(cell))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eval_error_display() {
        let origin = CellId::new("Table", "r1", "c1");
        let err1 = EvalError::cell_not_found(origin.clone());
        assert_eq!(
            err1.to_string(),
            "Cell 'Table[\"r1\", \"c1\"]' not found in cell grid"
        );

        let err2 = EvalError::unevaluated_dependency(origin.clone());
        assert_eq!(
            err2.to_string(),
            "Referenced cell 'Table[\"r1\", \"c1\"]' has not been evaluated (dependency ordering anomaly)"
        );

        let err3 = EvalError::unexpected_omitted(origin.clone());
        assert_eq!(
            err3.to_string(),
            "Unexpected omitted argument placeholder '_' in cell 'Table[\"r1\", \"c1\"]'"
        );

        let func_err = FunctionError::UnknownFunction("FOO".into());
        let err4 = EvalError::function(origin, func_err);
        assert_eq!(
            err4.to_string(),
            "Function error in cell 'Table[\"r1\", \"c1\"]': unknown function 'FOO'"
        );
    }
}
