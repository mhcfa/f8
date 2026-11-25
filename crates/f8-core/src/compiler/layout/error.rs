//! Layout diagnostic errors for Pass 5 spatial planning.

use thiserror::Error;

use crate::model::id::{ColId, RowId, SheetId, TableId};

/// Errors encountered during spatial layout planning.
#[derive(Debug, Error, PartialEq)]
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

    /// Uniform grid column width falls outside valid Excel column width bounds.
    #[error("sheet '{sheet}': grid column width {width} is out of bounds (expected {min}..={max})")]
    GridColWidthOutOfBounds {
        sheet: SheetId,
        width: f64,
        min: f64,
        max: f64,
    },

    /// Measure column header line exceeds the uniform grid base column width.
    #[error(
        "sheet '{sheet}', table '{table}', column '{col}': measure header '{header}' requires width {required_width} exceeding grid col_width {grid_col_width}"
    )]
    GridMeasureHeaderTooWide {
        sheet: SheetId,
        table: TableId,
        col: ColId,
        header: String,
        required_width: f64,
        grid_col_width: f64,
    },

    /// Data cell on a grid sheet contains an explicit line break (`\n` or `\r`).
    #[error(
        "sheet '{sheet}', table '{table}', row '{row}', column '{col}': multiline cell text is not allowed on a grid sheet"
    )]
    GridMultilineCell {
        sheet: SheetId,
        table: TableId,
        row: RowId,
        col: ColId,
    },

    /// Column header or group banner on a grid sheet contains an explicit line break (`\n` or `\r`).
    #[error(
        "sheet '{sheet}', table '{table}', column '{col}': multiline column header or group banner is not allowed on a grid sheet"
    )]
    GridMultilineHeader {
        sheet: SheetId,
        table: TableId,
        col: ColId,
    },
}
