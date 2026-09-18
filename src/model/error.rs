use thiserror::Error;

use super::id::{ColGroupId, ColId, RowGroupId, RowId, TableId};

/// Structured diagnostic errors for Finx specifications and model validation.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModelError {
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

    #[error("Table '{table}' has no columns declared")]
    EmptyColumns { table: TableId },

    #[error("Table '{table}' has no rows declared")]
    EmptyRows { table: TableId },

    #[error("Duplicate sheet name '{sheet}' in project manifest")]
    DuplicateSheetName { sheet: String },

    #[error("Duplicate table id '{table}' across project")]
    DuplicateTableId { table: TableId },

    #[error("Serialization / Deserialization error: {0}")]
    Json(String),
}

impl From<serde_json::Error> for ModelError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json(err.to_string())
    }
}
