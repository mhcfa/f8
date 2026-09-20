use std::path::PathBuf;
use thiserror::Error;

use super::format::{CurrencyCode, FormatPreset};
use super::id::{ColGroupId, ColId, RowGroupId, RowId, TableId};

/// Structured diagnostic errors for Finx specifications and model validation.
#[derive(Debug, Error)]
pub enum ModelError {
    #[error(
        "Table '{table}': {context} specifies decimals ({decimals}) for non-numeric preset '{preset}'"
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
        "Table '{table}': column group '{group}' columns are not a contiguous sub-slice of table columns"
    )]
    NonContiguousColumnGroup { table: TableId, group: ColGroupId },

    #[error(
        "Table '{table}': column '{column}' belongs to multiple column groups ('{first_group}' and '{second_group}')"
    )]
    OverlappingColumnGroup {
        table: TableId,
        column: ColId,
        first_group: ColGroupId,
        second_group: ColGroupId,
    },

    #[error(
        "Table '{table}': row group '{group}' descendant rows are not a contiguous sub-slice of table rows"
    )]
    NonContiguousRowGroup { table: TableId, group: RowGroupId },

    #[error(
        "Table '{table}': row '{row}' belongs to multiple row groups ('{first_group}' and '{second_group}')"
    )]
    OverlappingRowGroup {
        table: TableId,
        row: RowId,
        first_group: RowGroupId,
        second_group: RowGroupId,
    },

    #[error("Table '{table}': cyclic row group hierarchy detected at group '{group}'")]
    CyclicRowGroup { table: TableId, group: RowGroupId },

    #[error("Table '{table}': duplicate row id '{row}'")]
    DuplicateRowId { table: TableId, row: RowId },

    #[error("Table '{table}': duplicate column id '{column}'")]
    DuplicateColumnId { table: TableId, column: ColId },

    #[error("Table '{table}': duplicate column group id '{group}'")]
    DuplicateColumnGroupId { table: TableId, group: ColGroupId },

    #[error("Table '{table}': duplicate row group id '{group}'")]
    DuplicateRowGroupId { table: TableId, group: RowGroupId },

    #[error("Table '{table}': duplicate table id '{table}' across project")]
    DuplicateTableId { table: TableId },

    #[error("Table '{table}' has no columns declared")]
    EmptyColumns { table: TableId },

    #[error("Table '{table}' has no rows declared")]
    EmptyRows { table: TableId },

    #[error("Duplicate sheet name '{sheet}' in project manifest")]
    DuplicateSheetName { sheet: String },

    #[error("Theme '{name}' not found in built-in catalog and file does not exist: {path}")]
    ThemeNotFound { name: String, path: PathBuf },

    #[error("Invalid hex color code '{color}' in theme palette for field '{field}'")]
    InvalidThemeColor { field: String, color: String },

    #[error("Invalid theme metric '{metric}': expected positive finite value, found {value}")]
    InvalidThemeMetric { metric: String, value: f64 },

    #[error("Theme format template for '{preset}' must contain '{{dec}}' placeholder")]
    MissingDecimalPlaceholder { preset: FormatPreset },

    #[error("Theme format template for 'currency' must contain '{{sym}}' placeholder")]
    MissingCurrencySymbolPlaceholder,

    #[error("Failed to read theme file '{path}': {source}")]
    ThemeIoError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to parse theme file '{path}': {source}")]
    ThemeParseError {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "Theme format for '{preset}' specifies precision ({decimals}) exceeding maximum supported precision ({max})"
    )]
    ThemePrecisionOutOfBounds {
        preset: FormatPreset,
        decimals: u32,
        max: u32,
    },

    #[error("Serialization / Deserialization error: {0}")]
    Json(String),
}

impl From<serde_json::Error> for ModelError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err.to_string())
    }
}
