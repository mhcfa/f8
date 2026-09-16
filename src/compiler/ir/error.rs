use thiserror::Error;

use super::id::CellId;
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
}
