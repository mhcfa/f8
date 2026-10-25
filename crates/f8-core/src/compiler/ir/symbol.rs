use indexmap::{IndexMap, IndexSet};

use super::error::IrError;
use crate::model::id::{ColId, RowId, SheetId, TableId};
use crate::model::table::{AxisRole, ColumnSpec, DataColumnSpec, RowSpec, TableSpec};

/// Catalog metadata registered for a table within the symbol table.
#[derive(Debug, Clone, PartialEq)]
pub struct TableMetadata {
    pub table_id: TableId,
    pub sheet_id: SheetId,
    pub columns: Vec<DataColumnSpec>,
    pub column_index: IndexMap<ColId, usize>,
    pub measure_columns: Vec<ColId>,
    pub rows: IndexSet<RowId>,
}

impl TableMetadata {
    /// Constructs metadata by analyzing a table specification and assigning it to a sheet.
    pub fn from_table_spec(sheet_id: SheetId, table: &TableSpec) -> Result<Self, IrError> {
        if table.columns.is_empty() {
            return Err(IrError::EmptyTable {
                table: table.id.clone(),
            });
        }

        let mut columns = Vec::new();
        let mut column_index = IndexMap::new();
        let mut measure_columns = Vec::new();

        for col in &table.columns {
            if let ColumnSpec::Data(d) = col {
                let idx = columns.len();
                column_index.insert(d.id.clone(), idx);
                if d.role.is_measure() {
                    measure_columns.push(d.id.clone());
                }
                columns.push(d.clone());
            }
        }

        let mut rows = IndexSet::new();
        for row in &table.rows {
            if let RowSpec::Data(d) = row {
                rows.insert(d.id.clone());
            }
        }

        Ok(Self {
            table_id: table.id.clone(),
            sheet_id,
            columns,
            column_index,
            measure_columns,
            rows,
        })
    }

    /// Returns the column specification for a given column identifier.
    pub fn col_spec(&self, col: &ColId) -> Option<&DataColumnSpec> {
        self.column_index.get(col).map(|&idx| &self.columns[idx])
    }

    /// Returns the axis role of a column.
    pub fn col_role(&self, col: &ColId) -> Option<AxisRole> {
        self.col_spec(col).map(|c| c.role)
    }
}

/// Global symbol table managing table ownership, sheet placement, and coordinate indexing.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SymbolTable {
    tables: IndexMap<TableId, TableMetadata>,
    sheet_tables: IndexMap<SheetId, Vec<TableId>>,
}

impl SymbolTable {
    /// Creates an empty symbol table.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a table and assigns it to a sheet.
    pub fn register_table(&mut self, sheet_id: SheetId, table: &TableSpec) -> Result<(), IrError> {
        if let Some(existing) = self.tables.get(&table.id) {
            return Err(IrError::DuplicateTable {
                table: table.id.clone(),
                first_sheet: existing.sheet_id.clone(),
                second_sheet: sheet_id,
            });
        }
        let meta = TableMetadata::from_table_spec(sheet_id.clone(), table)?;
        let table_id = table.id.clone();
        self.tables.insert(table_id.clone(), meta);
        self.sheet_tables
            .entry(sheet_id)
            .or_default()
            .push(table_id);
        Ok(())
    }

    /// Returns the number of registered tables.
    pub fn len(&self) -> usize {
        self.tables.len()
    }

    /// Returns `true` if the symbol table contains no registered tables.
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    /// Returns `true` if a table with the given ID has been registered.
    pub fn contains_table(&self, table: &TableId) -> bool {
        self.tables.contains_key(table)
    }

    /// Returns `true` if a table exists and contains the given column.
    pub fn contains_col(&self, table: &TableId, col: &ColId) -> bool {
        self.tables
            .get(table)
            .is_some_and(|m| m.column_index.contains_key(col))
    }

    /// Returns `true` if a table exists and contains the given row.
    pub fn contains_row(&self, table: &TableId, row: &RowId) -> bool {
        self.tables.get(table).is_some_and(|m| m.rows.contains(row))
    }

    /// Returns metadata for a table if registered.
    pub fn get_table(&self, table: &TableId) -> Option<&TableMetadata> {
        self.tables.get(table)
    }

    /// Returns the sheet ID where a table is placed.
    pub fn get_sheet(&self, table: &TableId) -> Option<&SheetId> {
        self.tables.get(table).map(|m| &m.sheet_id)
    }

    /// Returns the 0-based column index within the table if present.
    pub fn col_index(&self, table: &TableId, col: &ColId) -> Option<usize> {
        self.tables
            .get(table)
            .and_then(|m| m.column_index.get(col).copied())
    }

    /// Returns the 0-based row index within the table if present.
    pub fn row_index(&self, table: &TableId, row: &RowId) -> Option<usize> {
        self.tables
            .get(table)
            .and_then(|m| m.rows.get_index_of(row))
    }

    /// Returns the column specification for a column in a registered table.
    pub fn col_spec(&self, table: &TableId, col: &ColId) -> Option<&DataColumnSpec> {
        self.tables.get(table).and_then(|m| m.col_spec(col))
    }

    /// Returns the axis role of a column in a registered table.
    pub fn col_role(&self, table: &TableId, col: &ColId) -> Option<AxisRole> {
        self.tables.get(table).and_then(|m| m.col_role(col))
    }

    /// Returns the slice of table IDs placed on a specific sheet.
    pub fn tables_on_sheet(&self, sheet: &SheetId) -> &[TableId] {
        self.sheet_tables
            .get(sheet)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Returns all registered table IDs in registration order.
    pub fn table_ids(&self) -> impl Iterator<Item = &TableId> {
        self.tables.keys()
    }

    /// Returns all registered sheet IDs in registration order.
    pub fn sheet_ids(&self) -> impl Iterator<Item = &SheetId> {
        self.sheet_tables.keys()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_table_registration() {
        let mut symbols = SymbolTable::new();
        assert!(symbols.is_empty());

        let table = TableSpec {
            id: TableId::new("IncomeStatement"),
            title: "Income Statement".to_string(),
            subtitle: None,
            currency: None,
            collapsible: false,
            placement: crate::model::table::PlacementSpec::default(),
            columns: vec![
                ColumnSpec::descriptor("line_item", "Line Item"),
                ColumnSpec::new("2023A"),
                ColumnSpec::new("2024E"),
            ],
            rows: vec![
                RowSpec::new("revenue").with_expr("100"),
                RowSpec::new("cogs").with_expr("50"),
            ],
        };

        symbols
            .register_table(SheetId::from("Model"), &table)
            .unwrap();

        assert_eq!(symbols.len(), 1);
        assert!(!symbols.is_empty());
        assert!(symbols.contains_table(&TableId::new("IncomeStatement")));
        assert_eq!(
            symbols.get_sheet(&TableId::new("IncomeStatement")),
            Some(&SheetId::from("Model"))
        );
        assert_eq!(
            symbols.tables_on_sheet(&SheetId::from("Model")),
            &[TableId::new("IncomeStatement")]
        );

        assert!(symbols.contains_col(&TableId::new("IncomeStatement"), &ColId::new("2023A")));
        assert!(symbols.contains_row(&TableId::new("IncomeStatement"), &RowId::new("revenue")));
        assert_eq!(
            symbols.col_index(&TableId::new("IncomeStatement"), &ColId::new("line_item")),
            Some(0)
        );
        assert_eq!(
            symbols.col_index(&TableId::new("IncomeStatement"), &ColId::new("2023A")),
            Some(1)
        );
        assert_eq!(
            symbols.col_role(&TableId::new("IncomeStatement"), &ColId::new("line_item")),
            Some(AxisRole::Descriptor)
        );
        assert_eq!(
            symbols.col_role(&TableId::new("IncomeStatement"), &ColId::new("2023A")),
            Some(AxisRole::Measure)
        );
    }
}
