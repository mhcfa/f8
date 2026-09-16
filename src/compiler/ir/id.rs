use serde::{Deserialize, Serialize};
use std::fmt;

use crate::model::id::{ColId, RowId, TableId};

/// Discrete cell coordinate identifier across the entire workbook.
///
/// Every discrete cell in the model is uniquely addressed by the 3-tuple
/// `(TableId, RowId, ColId)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CellId {
    pub table: TableId,
    pub row: RowId,
    pub col: ColId,
}

impl CellId {
    /// Creates a new cell coordinate identifier.
    pub fn new(table: impl Into<TableId>, row: impl Into<RowId>, col: impl Into<ColId>) -> Self {
        Self {
            table: table.into(),
            row: row.into(),
            col: col.into(),
        }
    }
}

impl fmt::Display for CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}[\"{}\"]", self.table, self.row, self.col)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_id_display() {
        let cell = CellId::new("IncomeStatement", "revenue", "2024E");
        assert_eq!(cell.to_string(), "IncomeStatement.revenue[\"2024E\"]");
    }

    #[test]
    fn test_cell_id_equality_and_order() {
        let c1 = CellId::new("A", "r1", "c1");
        let c2 = CellId::new("A", "r1", "c1");
        let c3 = CellId::new("A", "r1", "c2");
        let c4 = CellId::new("B", "r1", "c1");

        assert_eq!(c1, c2);
        assert_ne!(c1, c3);
        assert!(c1 < c3);
        assert!(c3 < c4);
    }
}
