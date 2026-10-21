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

    #[error("Table '{table}': unknown column '{column}' in {context}")]
    UnknownColumn {
        table: TableId,
        column: ColId,
        context: String,
    },

    #[error("Table '{table}': unknown row '{row}' in {context}")]
    UnknownRow {
        table: TableId,
        row: RowId,
        context: String,
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

    #[error("Duplicate sheet name '{sheet}' in project manifest")]
    DuplicateSheetName { sheet: String },

    #[error("Theme '{name}' not found in built-in catalog")]
    ThemeNotFound { name: String },

    #[error("Invalid hex color code '{color}' in theme palette for field '{field}'")]
    InvalidThemeColor { field: String, color: String },

    #[error("Invalid theme metric '{metric}': expected positive finite value, found {value}")]
    InvalidThemeMetric { metric: String, value: f64 },

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
