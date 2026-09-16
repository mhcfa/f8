use indexmap::{IndexMap, IndexSet};

use super::error::IrError;
use crate::model::id::{ColGroupId, ColId, RowGroupId, RowId, SheetId, TableId};
use crate::model::table::{RowGroup, RowGroupChild, TableSpec};

/// Catalog metadata registered for a table within the symbol table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableMetadata {
    pub table_id: TableId,
    pub sheet_id: SheetId,
    pub columns: Vec<ColId>,
    pub column_index: IndexMap<ColId, usize>,
    pub rows: IndexSet<RowId>,
    pub column_groups: IndexMap<ColGroupId, Vec<ColId>>,
    pub row_groups: IndexMap<RowGroupId, Vec<RowId>>,
}

impl TableMetadata {
    /// Constructs metadata by analyzing a table specification and assigning it to a sheet.
    pub fn from_table_spec(sheet_id: SheetId, table: &TableSpec) -> Result<Self, IrError> {
        if table.columns.is_empty() {
            return Err(IrError::EmptyTable {
                table: table.id.clone(),
            });
        }

        let mut column_index = IndexMap::with_capacity(table.columns.len());
        for (idx, col) in table.columns.iter().enumerate() {
            column_index.insert(col.clone(), idx);
        }

        let mut rows = IndexSet::with_capacity(table.rows.len());
        for row in &table.rows {
            rows.insert(row.id.clone());
        }

        let mut column_groups = IndexMap::new();
        for cg in &table.column_groups {
            column_groups.insert(cg.id.clone(), cg.columns.clone());
        }

        let mut row_groups = IndexMap::new();
        for rg in &table.row_groups {
            Self::collect_row_group(rg, &mut row_groups);
        }

        Ok(Self {
            table_id: table.id.clone(),
            sheet_id,
            columns: table.columns.clone(),
            column_index,
            rows,
            column_groups,
            row_groups,
        })
    }

    fn collect_row_group(group: &RowGroup, out: &mut IndexMap<RowGroupId, Vec<RowId>>) {
        let mut row_ids = Vec::new();
        Self::collect_row_children(&group.children, &mut row_ids, out);
        out.insert(group.id.clone(), row_ids);
    }

    fn collect_row_children(
        children: &[RowGroupChild],
        row_ids: &mut Vec<RowId>,
        out: &mut IndexMap<RowGroupId, Vec<RowId>>,
    ) {
        for child in children {
            match child {
                RowGroupChild::Row(id) => {
                    row_ids.push(id.clone());
                }
                RowGroupChild::Group(nested) => {
                    Self::collect_row_group(nested, out);
                    if let Some(nested_rows) = out.get(&nested.id) {
                        row_ids.extend(nested_rows.iter().cloned());
                    }
                }
            }
        }
    }
}

/// Global symbol table managing table ownership, sheet placement, and coordinate indexing.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SymbolTable {
    tables: IndexMap<TableId, TableMetadata>,
    sheet_tables: IndexMap<SheetId, Vec<TableId>>,
}

impl SymbolTable {
    /// Creates a new, empty symbol table.
    pub fn new() -> Self {
        Self {
            tables: IndexMap::new(),
            sheet_tables: IndexMap::new(),
        }
    }

    /// Registers a table specification under the given sheet.
    ///
    /// Returns `IrError::DuplicateTable` if `table.id` is already registered on any sheet.
    pub fn register_table(&mut self, sheet_id: SheetId, table: &TableSpec) -> Result<(), IrError> {
        if let Some(existing) = self.tables.get(&table.id) {
            return Err(IrError::DuplicateTable {
                table: table.id.clone(),
                first_sheet: existing.sheet_id.clone(),
                second_sheet: sheet_id,
            });
        }

        let metadata = TableMetadata::from_table_spec(sheet_id.clone(), table)?;
        self.tables.insert(table.id.clone(), metadata);
        self.sheet_tables
            .entry(sheet_id)
            .or_default()
            .push(table.id.clone());
        Ok(())
    }

    /// Returns the sheet ID where the table is placed, if registered.
    pub fn get_sheet(&self, table: &TableId) -> Option<&SheetId> {
        self.tables.get(table).map(|m| &m.sheet_id)
    }

    /// Returns the registered metadata for a table, if present.
    pub fn get_table(&self, table: &TableId) -> Option<&TableMetadata> {
        self.tables.get(table)
    }

    /// Returns true if the table is registered.
    pub fn contains_table(&self, table: &TableId) -> bool {
        self.tables.contains_key(table)
    }

    /// Returns true if the column exists in the registered table.
    pub fn contains_col(&self, table: &TableId, col: &ColId) -> bool {
        self.tables
            .get(table)
            .is_some_and(|m| m.column_index.contains_key(col))
    }

    /// Returns true if the row exists in the registered table.
    pub fn contains_row(&self, table: &TableId, row: &RowId) -> bool {
        self.tables.get(table).is_some_and(|m| m.rows.contains(row))
    }

    /// Returns the 0-based column index of a column within a table.
    pub fn col_index(&self, table: &TableId, col: &ColId) -> Option<usize> {
        self.tables
            .get(table)
            .and_then(|m| m.column_index.get(col).copied())
    }

    /// Returns the 0-based row index of a row within a table.
    pub fn row_index(&self, table: &TableId, row: &RowId) -> Option<usize> {
        self.tables
            .get(table)
            .and_then(|m| m.rows.get_index_of(row))
    }

    /// Returns the column IDs belonging to a column group in a table.
    pub fn cols_in_group(&self, table: &TableId, group: &ColGroupId) -> Option<&[ColId]> {
        self.tables
            .get(table)
            .and_then(|m| m.column_groups.get(group).map(|v| v.as_slice()))
    }

    /// Returns the row IDs belonging to a row group in a table.
    pub fn rows_in_group(&self, table: &TableId, group: &RowGroupId) -> Option<&[RowId]> {
        self.tables
            .get(table)
            .and_then(|m| m.row_groups.get(group).map(|v| v.as_slice()))
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

    /// Returns the total number of registered tables.
    pub fn len(&self) -> usize {
        self.tables.len()
    }

    /// Returns true if no tables are registered.
    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::model::table::RowSpec;

    #[test]
    fn test_symbol_table_registration_and_lookups() {
        let mut symbols = SymbolTable::new();
        assert!(symbols.is_empty());

        let table = TableSpec {
            id: TableId::new("IncomeStatement"),
            title: "Income Statement".to_string(),
            columns: vec![
                ColId::new("2023A"),
                ColId::new("2024E"),
                ColId::new("2025E"),
            ],
            column_groups: vec![],
            row_groups: vec![],
            rows: vec![
                RowSpec {
                    id: RowId::new("revenue"),
                    label: "Revenue".to_string(),
                    format: None,
                    note: None,
                    source: None,
                    value: None,
                    expr: Some("100".to_string()),
                    match_arms: None,
                },
                RowSpec {
                    id: RowId::new("cogs"),
                    label: "COGS".to_string(),
                    format: None,
                    note: None,
                    source: None,
                    value: None,
                    expr: Some("50".to_string()),
                    match_arms: None,
                },
            ],
        };

        symbols
            .register_table(SheetId::new("Model"), &table)
            .expect("Should register table");

        assert_eq!(symbols.len(), 1);
        assert_eq!(
            symbols.get_sheet(&TableId::new("IncomeStatement")),
            Some(&SheetId::new("Model"))
        );

        assert_eq!(
            symbols.col_index(&TableId::new("IncomeStatement"), &ColId::new("2023A")),
            Some(0)
        );
        assert_eq!(
            symbols.col_index(&TableId::new("IncomeStatement"), &ColId::new("2024E")),
            Some(1)
        );
        assert_eq!(
            symbols.col_index(&TableId::new("IncomeStatement"), &ColId::new("2025E")),
            Some(2)
        );
        assert_eq!(
            symbols.col_index(&TableId::new("IncomeStatement"), &ColId::new("unknown")),
            None
        );

        assert_eq!(
            symbols.row_index(&TableId::new("IncomeStatement"), &RowId::new("revenue")),
            Some(0)
        );
        assert_eq!(
            symbols.row_index(&TableId::new("IncomeStatement"), &RowId::new("cogs")),
            Some(1)
        );
        assert_eq!(
            symbols.row_index(&TableId::new("IncomeStatement"), &RowId::new("unknown")),
            None
        );

        assert_eq!(
            symbols.tables_on_sheet(&SheetId::new("Model")),
            &[TableId::new("IncomeStatement")]
        );
        assert_eq!(
            symbols.sheet_ids().collect::<Vec<_>>(),
            vec![&SheetId::new("Model")]
        );

        // Duplicate table rejection across sheets
        let err = symbols
            .register_table(SheetId::new("Summary"), &table)
            .unwrap_err();
        assert_eq!(
            err,
            IrError::DuplicateTable {
                table: TableId::new("IncomeStatement"),
                first_sheet: SheetId::new("Model"),
                second_sheet: SheetId::new("Summary")
            }
        );
    }
}
