//! Domain errors occurring during model inspection queries.

use thiserror::Error;

use crate::compiler::ir::id::CellId;
use crate::model::id::{SheetId, TableId};

/// Domain errors occurring during model inspection queries.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InspectionError {
    /// Referenced table was not found in the compiled model.
    #[error("Table '{0}' not found in compiled model")]
    TableNotFound(TableId),

    /// Referenced cell was not found in the compiled model grid.
    #[error("Cell '{0}' not found in compiled model")]
    CellNotFound(CellId),

    /// Referenced worksheet was not found in the compiled model.
    #[error("Sheet '{0}' not found in compiled model")]
    SheetNotFound(SheetId),
}
