use thiserror::Error;

use crate::compiler::ir::id::CellId;
use crate::model::id::{ColGroupId, ColId, RowGroupId, RowId, TableId};

/// Errors encountered during formula reference resolution against table axes and the symbol table.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResolveError {
    #[error("Formula in cell '{origin}' references unknown table '{target_table}'")]
    UnknownTable {
        origin: Box<CellId>,
        target_table: TableId,
    },

    #[error("Formula in cell '{origin}' references unknown row '{row}' in table '{target_table}'")]
    UnknownRow {
        origin: Box<CellId>,
        target_table: TableId,
        row: RowId,
    },

    #[error(
        "Formula in cell '{origin}' references unknown column '{column}' in table '{target_table}'"
    )]
    UnknownColumn {
        origin: Box<CellId>,
        target_table: TableId,
        column: ColId,
    },

    #[error(
        "Formula in cell '{origin}' references unknown row group '{group}' in table '{target_table}'"
    )]
    UnknownRowGroup {
        origin: Box<CellId>,
        target_table: TableId,
        group: RowGroupId,
    },

    #[error(
        "Formula in cell '{origin}' references unknown column group '{group}' in table '{target_table}'"
    )]
    UnknownColumnGroup {
        origin: Box<CellId>,
        target_table: TableId,
        group: ColGroupId,
    },

    #[error(
        "Formula in cell '{origin}' references active column '{column}', but target table '{target_table}' does not declare this column"
    )]
    ActiveColumnMismatch {
        origin: Box<CellId>,
        target_table: TableId,
        column: ColId,
    },

    #[error(
        "Formula in cell '{origin}' references active row '{row}', but target table '{target_table}' does not declare this row"
    )]
    ActiveRowMismatch {
        origin: Box<CellId>,
        target_table: TableId,
        row: RowId,
    },

    #[error(
        "Formula in cell '{origin}' has temporal offset {offset:+} which exceeds axis boundaries"
    )]
    TemporalOffsetOutOfBounds { origin: Box<CellId>, offset: i32 },

    #[error(
        "Formula in cell '{origin}' specifies inverted range '{start}'..'{end}' in table '{target_table}'"
    )]
    InvertedRange {
        origin: Box<CellId>,
        target_table: TableId,
        start: String,
        end: String,
    },

    #[error("Table '{table}' containing cell '{origin}' is not registered in the symbol table")]
    UnregisteredTable { origin: Box<CellId>, table: TableId },
}

impl ResolveError {
    /// Creates an `UnknownTable` error.
    pub fn unknown_table(origin: &CellId, target_table: impl Into<TableId>) -> Self {
        Self::UnknownTable {
            origin: Box::new(origin.clone()),
            target_table: target_table.into(),
        }
    }

    /// Creates an `UnknownRow` error.
    pub fn unknown_row(
        origin: &CellId,
        target_table: impl Into<TableId>,
        row: impl Into<RowId>,
    ) -> Self {
        Self::UnknownRow {
            origin: Box::new(origin.clone()),
            target_table: target_table.into(),
            row: row.into(),
        }
    }

    /// Creates an `UnknownColumn` error.
    pub fn unknown_column(
        origin: &CellId,
        target_table: impl Into<TableId>,
        column: impl Into<ColId>,
    ) -> Self {
        Self::UnknownColumn {
            origin: Box::new(origin.clone()),
            target_table: target_table.into(),
            column: column.into(),
        }
    }

    /// Creates an `UnknownRowGroup` error.
    pub fn unknown_row_group(
        origin: &CellId,
        target_table: impl Into<TableId>,
        group: impl Into<RowGroupId>,
    ) -> Self {
        Self::UnknownRowGroup {
            origin: Box::new(origin.clone()),
            target_table: target_table.into(),
            group: group.into(),
        }
    }

    /// Creates an `UnknownColumnGroup` error.
    pub fn unknown_column_group(
        origin: &CellId,
        target_table: impl Into<TableId>,
        group: impl Into<ColGroupId>,
    ) -> Self {
        Self::UnknownColumnGroup {
            origin: Box::new(origin.clone()),
            target_table: target_table.into(),
            group: group.into(),
        }
    }

    /// Creates an `ActiveColumnMismatch` error.
    pub fn active_column_mismatch(
        origin: &CellId,
        target_table: impl Into<TableId>,
        column: impl Into<ColId>,
    ) -> Self {
        Self::ActiveColumnMismatch {
            origin: Box::new(origin.clone()),
            target_table: target_table.into(),
            column: column.into(),
        }
    }

    /// Creates an `ActiveRowMismatch` error.
    pub fn active_row_mismatch(
        origin: &CellId,
        target_table: impl Into<TableId>,
        row: impl Into<RowId>,
    ) -> Self {
        Self::ActiveRowMismatch {
            origin: Box::new(origin.clone()),
            target_table: target_table.into(),
            row: row.into(),
        }
    }

    /// Creates a `TemporalOffsetOutOfBounds` error.
    pub fn temporal_offset_out_of_bounds(origin: &CellId, offset: i32) -> Self {
        Self::TemporalOffsetOutOfBounds {
            origin: Box::new(origin.clone()),
            offset,
        }
    }

    /// Creates an `InvertedRange` error.
    pub fn inverted_range(
        origin: &CellId,
        target_table: impl Into<TableId>,
        start: impl Into<String>,
        end: impl Into<String>,
    ) -> Self {
        Self::InvertedRange {
            origin: Box::new(origin.clone()),
            target_table: target_table.into(),
            start: start.into(),
            end: end.into(),
        }
    }

    /// Creates an `UnregisteredTable` error.
    pub fn unregistered_table(origin: &CellId, table: impl Into<TableId>) -> Self {
        Self::UnregisteredTable {
            origin: Box::new(origin.clone()),
            table: table.into(),
        }
    }
}

/// Errors encountered during dependency graph construction, cycle validation, and topological sorting.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GraphError {
    #[error(transparent)]
    Resolve(#[from] ResolveError),

    #[error("Circular dependency detected in model.\nCycle trace:\n{trace}")]
    CircularDependency { cycle: Vec<CellId>, trace: String },
}

impl GraphError {
    /// Creates a `CircularDependency` error from an ordered cycle path.
    pub fn circular_dependency(cycle: Vec<CellId>) -> Self {
        let trace = format_cycle_trace(&cycle);
        Self::CircularDependency { cycle, trace }
    }
}

/// Formats an ordered list of cell coordinates into a numbered semantic cycle trace.
pub fn format_cycle_trace(cycle: &[CellId]) -> String {
    cycle
        .iter()
        .enumerate()
        .map(|(idx, cell)| format!("  {}. {cell}", idx + 1))
        .collect::<Vec<_>>()
        .join("\n")
}
