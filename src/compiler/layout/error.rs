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

    /// Row group hierarchy exceeds Excel's maximum outline level limit of 7.
    #[error(
        "table '{table}' has row group '{group}' exceeding maximum Excel outline depth of 7 (depth: {depth})"
    )]
    OutlineDepthExceeded {
        table: TableId,
        group: String,
        depth: usize,
    },

    /// Layout placement exceeds physical Excel grid boundaries.
    #[error("layout placement exceeds Excel grid limit of {max} {dimension} (requested: {actual})")]
    GridLimitExceeded {
        dimension: &'static str,
        max: u32,
        actual: u32,
    },
}
