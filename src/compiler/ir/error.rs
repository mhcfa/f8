use thiserror::Error;

use super::id::CellId;
use crate::model::format::FormatPreset;
use crate::model::id::{ColId, RowId, SheetId, TableId};

/// Errors encountered during intermediate representation construction and symbol registration.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IrError {
    #[error("duplicate cell definition: '{id}'")]
    DuplicateCell { id: CellId },

    #[error(
        "table '{table}' is already registered on sheet '{first_sheet}', cannot register on sheet '{second_sheet}'"
    )]
    DuplicateTable {
        table: TableId,
        first_sheet: SheetId,
        second_sheet: SheetId,
    },

    #[error("unknown table '{table}' in {context}")]
    UnknownTable { table: TableId, context: String },

    #[error("unknown column '{column}' in table '{table}' ({context})")]
    UnknownColumn {
        table: TableId,
        column: ColId,
        context: String,
    },

    #[error("unknown row '{row}' in table '{table}' ({context})")]
    UnknownRow {
        table: TableId,
        row: RowId,
        context: String,
    },

    #[error("unknown sheet '{sheet}'")]
    UnknownSheet { sheet: SheetId },

    #[error("table '{table}' contains no columns")]
    EmptyTable { table: TableId },

    #[error(
        "formula parse error in table '{table}'{}: {source}",
        format_parse_coords(row, column)
    )]
    FormulaParse {
        table: TableId,
        row: Option<RowId>,
        column: Option<ColId>,
        source: crate::expr::error::ParseError,
    },

    #[error(
        "Table '{table}': ambiguous expression conflict at row '{row}', col '{column}' (both row and column define vector expressions without a cell override)"
    )]
    AmbiguousExpressionConflict {
        table: TableId,
        row: RowId,
        column: ColId,
    },

    #[error("Table '{table}': unassigned cell at row '{row}', col '{column}'")]
    UnassignedCell {
        table: TableId,
        row: RowId,
        column: ColId,
    },

    #[error(
        "Table '{table}': format conflict at row '{row}' ({row_format}) and column '{column}' ({col_format})"
    )]
    FormatConflict {
        table: TableId,
        row: RowId,
        column: ColId,
        row_format: FormatPreset,
        col_format: FormatPreset,
    },

    #[error(
        "Table '{table}': decimals conflict at row '{row}' ({row_decimals}) and column '{column}' ({col_decimals})"
    )]
    DecimalsConflict {
        table: TableId,
        row: RowId,
        column: ColId,
        row_decimals: u32,
        col_decimals: u32,
    },

    #[error("model validation error: {0}")]
    Model(String),
}

impl From<crate::model::error::ModelError> for IrError {
    fn from(err: crate::model::error::ModelError) -> Self {
        Self::Model(err.to_string())
    }
}

fn format_parse_coords(row: &Option<RowId>, column: &Option<ColId>) -> String {
    match (row, column) {
        (Some(r), Some(c)) => format!(", row '{r}', col '{c}'"),
        (Some(r), None) => format!(", row '{r}'"),
        (None, Some(c)) => format!(", col '{c}'"),
        (None, None) => String::new(),
    }
}
