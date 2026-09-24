//! Layout diagnostic errors for Pass 5 spatial planning.

use thiserror::Error;

use crate::model::id::{SheetId, TableId};

/// Errors encountered during spatial layout planning.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum LayoutError {
    /// Table is not assigned to any worksheet in the manifest.
    #[error("table '{table}' is not assigned to any sheet in the manifest")]
    UnassignedTable { table: TableId },

    /// Worksheet references a table specification that was not provided.
    #[error("table '{table}' referenced by sheet '{sheet}' was not found")]
    UnknownTable { table: TableId, sheet: SheetId },

    /// Layout placement exceeds physical Excel grid boundaries.
    #[error("layout placement exceeds Excel grid limit of {max} {dimension} (requested: {actual})")]
    GridLimitExceeded {
        dimension: &'static str,
        max: u32,
        actual: u32,
    },
}
