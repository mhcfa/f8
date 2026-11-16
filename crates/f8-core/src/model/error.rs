use thiserror::Error;

use super::format::{CurrencyCode, FormatPreset};
use super::id::{ColId, RowId, TableId};

/// Structured diagnostic errors for f8 specifications and model validation.
#[derive(Debug, Error)]
pub enum ModelError {
    #[error(
        "Table '{table}': {context} specifies decimals ({decimals}) for preset '{preset}' which does not support decimals"
    )]
    InvalidPrecisionForPreset {
        table: TableId,
        context: String,
        preset: FormatPreset,
        decimals: u32,
    },

    #[error(
        "Table '{table}': {context} specifies currency ('{currency}') for non-currency preset '{preset}'"
    )]
    InvalidCurrencyForPreset {
        table: TableId,
        context: String,
        preset: FormatPreset,
        currency: CurrencyCode,
    },

    #[error("Table '{table}': subtitle must not be empty or whitespace")]
    EmptyTableSubtitle { table: TableId },

    #[error("Table '{table}': column '{column}' group must not be empty or whitespace")]
    EmptyColumnGroup { table: TableId, column: ColId },

    #[error(
        "Table '{table}': {context} specifies decimals ({decimals}) exceeding maximum supported precision ({max})"
    )]
    PrecisionOutOfBounds {
        table: TableId,
        context: String,
        decimals: u32,
        max: u32,
    },

    #[error("Table '{table}': row '{row}' specifies unknown column '{column}' in cells")]
    UnknownCellColumn {
        table: TableId,
        row: RowId,
        column: ColId,
    },

    #[error(
        "Table '{table}': row '{row}', column '{column}' cell override cannot specify both 'value' and 'expr'"
    )]
    MutuallyExclusiveCellPayload {
        table: TableId,
        row: RowId,
        column: ColId,
    },

    #[error(
        "Table '{table}': row '{row}', column '{column}' detailed cell override specifies neither 'value' nor 'expr'"
    )]
    EmptyCellPayload {
        table: TableId,
        row: RowId,
        column: ColId,
    },

    #[error(
        "Table '{table}': cell at row '{row}', column '{column}' specifies indent ({indent}), but column is a measure axis"
    )]
    InvalidIndentOnMeasureCell {
        table: TableId,
        row: RowId,
        column: ColId,
        indent: u8,
    },

    #[error("Table '{table}': row id must not be empty or whitespace")]
    EmptyRowId { table: TableId },

    #[error("Table '{table}': column id must not be empty or whitespace")]
    EmptyColumnId { table: TableId },

    #[error("Table '{table}': duplicate row id '{row}'")]
    DuplicateRowId { table: TableId, row: RowId },

    #[error("Table '{table}': duplicate column id '{column}'")]
    DuplicateColumnId { table: TableId, column: ColId },

    #[error("Table '{table}': duplicate table id '{table}' across project")]
    DuplicateTableId { table: TableId },

    #[error("Table '{table}' has no columns declared")]
    EmptyColumns { table: TableId },

    #[error("Table '{table}' has no rows declared")]
    EmptyRows { table: TableId },

    #[error("Project manifest must declare at least one worksheet")]
    EmptySheets,

    #[error("Duplicate sheet name '{sheet}' in project manifest")]
    DuplicateSheetName { sheet: String },

    #[error("Invalid sheet name '{sheet}': {reason}")]
    InvalidSheetName { sheet: String, reason: &'static str },

    #[error("Sheet '{sheet}': invalid hex color code '{color}' for 'tab_color'")]
    InvalidSheetTabColor { sheet: String, color: String },

    #[error(
        "Sheet '{sheet}': freeze_panes ({row}, {col}) exceeds Excel grid bounds ({max_rows} rows, {max_cols} columns)"
    )]
    InvalidFreezePanes {
        sheet: String,
        row: u32,
        col: u32,
        max_rows: u32,
        max_cols: u32,
    },

    #[error(
        "Table '{table}': table id must be a valid f8 identifier ([A-Za-z0-9_]+ with at least one letter or '_', excluding 'TRUE', 'FALSE', '_', and scientific notation)"
    )]
    InvalidTableId { table: TableId },

    #[error(
        "Table '{table}': row '{row}', column '{column}' cell specifies non-finite numeric value ({value})"
    )]
    NonFiniteCellNumber {
        table: TableId,
        row: RowId,
        column: ColId,
        value: f64,
    },

    #[error("Theme '{name}' not found in built-in catalog")]
    ThemeNotFound { name: String },

    #[error("Invalid hex color code '{color}' in theme palette for field '{field}'")]
    InvalidThemeColor { field: String, color: String },

    #[error("Invalid theme metric '{metric}': expected positive finite value, found {value}")]
    InvalidThemeMetric { metric: String, value: f64 },

    #[error(
        "Invalid theme column width bounds: max_col_width ({max_col_width}) must be greater than or equal to min_col_width ({min_col_width})"
    )]
    InvalidColWidthBounds {
        min_col_width: f64,
        max_col_width: f64,
    },

    #[error("Theme format template for '{preset}' must contain '{{dec}}' placeholder")]
    MissingDecimalPlaceholder { preset: FormatPreset },

    #[error("Theme format template for 'currency' must contain '{{sym}}' placeholder")]
    MissingCurrencySymbolPlaceholder,

    #[error(
        "Theme format for '{preset}' specifies precision ({decimals}) exceeding maximum supported precision ({max})"
    )]
    ThemePrecisionOutOfBounds {
        preset: FormatPreset,
        decimals: u32,
        max: u32,
    },
}
