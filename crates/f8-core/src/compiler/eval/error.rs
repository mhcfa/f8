use thiserror::Error;

use crate::compiler::graph::ResolveError;
use crate::compiler::ir::id::CellId;
use crate::functions::error::FunctionError;
use crate::model::id::{RowId, TableId};
use crate::value::TypeError;

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

    /// Static or dynamic formula type error.
    #[error("Type error in cell '{cell}': {source}")]
    Type {
        /// Cell coordinate where the type failure occurred.
        cell: Box<CellId>,
        /// Underlying type error.
        #[source]
        source: TypeError,
    },

    /// Range slice evaluated in a scalar expression or parameter context.
    #[error("Range slice ({rows}x{cols}) cannot be evaluated as a scalar in cell '{cell}'")]
    SliceInScalarContext {
        /// Cell coordinate where the scalar slice violation occurred.
        cell: Box<CellId>,
        /// Resolved row count of the slice.
        rows: usize,
        /// Resolved column count of the slice.
        cols: usize,
    },

    /// The specified table was not found in the symbol table.
    #[error("Table '{0}' not found in symbol table")]
    TableNotFound(TableId),

    /// The specified row was not found in the table metadata.
    #[error("Row '{row}' not found in table '{table}'")]
    RowNotFound {
        /// Table identifier.
        table: TableId,
        /// Row identifier.
        row: RowId,
    },
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

    /// Creates a normalized `EvalError` from a `FunctionError`, promoting `FunctionError::Type`
    /// and `FunctionError::SliceInScalarContext` into their canonical `EvalError` variants.
    #[must_use]
    pub fn function(cell: CellId, source: FunctionError) -> Self {
        match source {
            FunctionError::Type(source) => Self::Type {
                cell: Box::new(cell),
                source,
            },
            FunctionError::SliceInScalarContext { rows, cols } => Self::SliceInScalarContext {
                cell: Box::new(cell),
                rows,
                cols,
            },
            FunctionError::UnevaluatedCell { table, row, col } => {
                Self::UnevaluatedDependency(Box::new(CellId::new(table, row, col)))
            }
            other @ (FunctionError::UnknownFunction(_)
            | FunctionError::TooFewArguments { .. }
            | FunctionError::TooManyArguments { .. }
            | FunctionError::CannotOmitRequiredArgument { .. }
            | FunctionError::CannotOmitTrailingArgument { .. }
            | FunctionError::InvalidArgumentKind { .. }
            | FunctionError::EvaluationFailure { .. }
            | FunctionError::DimensionMismatch { .. }
            | FunctionError::LengthMismatch { .. }
            | FunctionError::ExpectedOneDimensionalSlice { .. }
            | FunctionError::MissingColumnIndexFor2DSlice { .. }
            | FunctionError::DescriptorReferenceNotAllowed { .. }) => Self::Function {
                cell: Box::new(cell),
                source: Box::new(other),
            },
        }
    }

    /// Creates a `TableNotFound` error.
    #[must_use]
    pub fn table_not_found(table: TableId) -> Self {
        Self::TableNotFound(table)
    }

    /// Creates a `RowNotFound` error.
    #[must_use]
    pub fn row_not_found(table: TableId, row: RowId) -> Self {
        Self::RowNotFound { table, row }
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

        let func_err = FunctionError::UnknownFunction("FOO".into());
        let err4 = EvalError::function(origin.clone(), func_err);
        assert_eq!(
            err4.to_string(),
            "Function error in cell 'Table[\"r1\", \"c1\"]': unknown function 'FOO'"
        );

        let type_err = EvalError::function(
            origin.clone(),
            FunctionError::Type(TypeError::EmptyFormulaResult),
        );
        assert!(matches!(type_err, EvalError::Type { .. }));

        let slice_err = EvalError::function(
            origin,
            FunctionError::SliceInScalarContext { rows: 2, cols: 1 },
        );
        assert!(matches!(
            slice_err,
            EvalError::SliceInScalarContext {
                rows: 2,
                cols: 1,
                ..
            }
        ));

        let err5 = EvalError::table_not_found(TableId::from("NonExistentTable"));
        assert_eq!(
            err5.to_string(),
            "Table 'NonExistentTable' not found in symbol table"
        );

        let err6 = EvalError::row_not_found(TableId::from("MyTable"), RowId::from("missing_row"));
        assert_eq!(
            err6.to_string(),
            "Row 'missing_row' not found in table 'MyTable'"
        );
    }
}
