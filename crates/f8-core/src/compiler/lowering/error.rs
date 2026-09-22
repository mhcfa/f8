//! Diagnostic errors for Pass 6a formula lowering.

use thiserror::Error;

use crate::compiler::graph::error::ResolveError;
use crate::compiler::ir::id::CellId;
use crate::model::id::TableId;

/// Errors encountered during AST formula lowering to Excel coordinate strings.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LoweringError {
    /// Failure during AST reference resolution against table axes or symbol table.
    #[error("reference resolution error: {0}")]
    Resolve(#[from] ResolveError),

    /// A required cell coordinate was missing from the spatial layout map.
    #[error("cell coordinate missing from spatial layout map: '{cell}'")]
    MissingCellCoordinate { cell: CellId },

    /// A referenced row or column range resolved to an empty slice.
    #[error("formula in cell '{origin}' references empty range in table '{target_table}'")]
    EmptyRange {
        origin: CellId,
        target_table: TableId,
    },

    /// Generic lowering syntax or formatting failure.
    #[error("formula lowering error: {message}")]
    SyntaxError { message: String },
}
