//! Evaluated model representation and symbol-integrated query APIs.

use indexmap::IndexMap;

use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::SymbolTable;
use crate::model::id::{ColId, RowId, TableId};
use crate::value::{CellValue, ExcelError};

use super::error::EvalError;

/// Evaluated model containing computed cell values and symbol table metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedModel {
    values: IndexMap<CellId, CellValue>,
    symbol_table: SymbolTable,
}

impl EvaluatedModel {
    /// Creates a new evaluated model wrapping calculated cell values and model symbol metadata.
    #[must_use]
    pub fn new(values: IndexMap<CellId, CellValue>, symbol_table: SymbolTable) -> Self {
        Self {
            values,
            symbol_table,
        }
    }

    /// Retrieves an evaluated cell value by coordinate components `(table, row, col)`.
    #[must_use]
    pub fn get_value(&self, table: &TableId, row: &RowId, col: &ColId) -> Option<&CellValue> {
        let cell_id = CellId::new(table.clone(), row.clone(), col.clone());
        self.values.get(&cell_id)
    }

    /// Retrieves an evaluated cell value by its 3-tuple `CellId`.
    #[must_use]
    pub fn get_cell(&self, id: &CellId) -> Option<&CellValue> {
        self.values.get(id)
    }

    /// Retrieves all evaluated cell values for a given table row, preserving declared column order.
    ///
    /// # Errors
    /// Returns `EvalError::TableNotFound` if the table does not exist in the symbol table.
    /// Returns `EvalError::RowNotFound` if the row does not exist in the registered table metadata.
    pub fn get_row_values(
        &self,
        table: &TableId,
        row: &RowId,
    ) -> Result<IndexMap<ColId, CellValue>, EvalError> {
        let table_meta = self
            .symbol_table
            .get_table(table)
            .ok_or_else(|| EvalError::table_not_found(table.clone()))?;

        if !table_meta.rows.contains(row) {
            return Err(EvalError::row_not_found(table.clone(), row.clone()));
        }

        let mut row_values = IndexMap::with_capacity(table_meta.columns.len());
        let mut lookup_id = CellId::new(table.clone(), row.clone(), ColId::from(""));
        for col_spec in &table_meta.columns {
            lookup_id.col.clone_from(&col_spec.id);
            let val = match self.values.get(&lookup_id) {
                Some(v) => v.clone(),
                None => CellValue::Empty,
            };
            row_values.insert(col_spec.id.clone(), val);
        }

        Ok(row_values)
    }

    /// Finds all cells that evaluated to an Excel domain error (`#DIV/0!`, `#VALUE!`, `#NUM!`, etc.).
    #[must_use]
    pub fn find_errors(&self) -> Vec<(&CellId, &ExcelError)> {
        self.values
            .iter()
            .filter_map(|(id, val)| match val {
                CellValue::Error(err) => Some((id, err)),
                _ => None,
            })
            .collect()
    }

    /// Returns a reference to the underlying symbol table.
    #[must_use]
    pub fn symbol_table(&self) -> &SymbolTable {
        &self.symbol_table
    }

    /// Returns a reference to the map of all evaluated cell values.
    #[must_use]
    pub fn values(&self) -> &IndexMap<CellId, CellValue> {
        &self.values
    }

    /// Consumes the model and returns the map of evaluated cell values.
    #[must_use]
    pub fn into_values(self) -> IndexMap<CellId, CellValue> {
        self.values
    }

    /// Iterates over all evaluated cell IDs and their values.
    pub fn iter(&self) -> impl Iterator<Item = (&CellId, &CellValue)> {
        self.values.iter()
    }

    /// Returns the total number of evaluated cells.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` if the evaluated model contains no cells.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl<'a> IntoIterator for &'a EvaluatedModel {
    type Item = (&'a CellId, &'a CellValue);
    type IntoIter = indexmap::map::Iter<'a, CellId, CellValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.iter()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::model::id::SheetId;
    use crate::model::table::{ColumnSpec, RowSpec, TableSpec};

    fn make_test_symbol_table() -> SymbolTable {
        let mut symbols = SymbolTable::new();
        let table = TableSpec {
            id: TableId::from("T"),
            title: "T".into(),
            subtitle: None,
            currency: None,
            columns: vec![
                ColumnSpec::new("c1"),
                ColumnSpec::new("c2"),
                ColumnSpec::new("c3"),
            ],
            column_groups: vec![],
            row_groups: vec![],
            rows: vec![RowSpec::new("r1"), RowSpec::new("r2")],
        };
        symbols
            .register_table(SheetId::from("Sheet1"), &table)
            .expect("registration succeeds");
        symbols
    }

    #[test]
    fn test_evaluated_model_queries() {
        let symbols = make_test_symbol_table();
        let mut values = IndexMap::new();
        let t = TableId::from("T");
        let r1 = RowId::from("r1");
        let c1 = ColId::from("c1");
        let c2 = ColId::from("c2");
        let c3 = ColId::from("c3");

        let cell1 = CellId::new(t.clone(), r1.clone(), c1.clone());
        let cell2 = CellId::new(t.clone(), r1.clone(), c2.clone());
        let cell3 = CellId::new(t.clone(), r1.clone(), c3.clone());

        values.insert(cell1.clone(), CellValue::Number(100.0));
        values.insert(cell2.clone(), CellValue::Text("hello".into()));
        values.insert(cell3.clone(), CellValue::Error(ExcelError::Div0));

        let model = EvaluatedModel::new(values, symbols);

        assert_eq!(model.len(), 3);
        assert!(!model.is_empty());
        assert_eq!(model.get_cell(&cell1), Some(&CellValue::Number(100.0)));
        assert_eq!(
            model.get_value(&t, &r1, &c2),
            Some(&CellValue::Text("hello".into()))
        );

        let row_vals = model.get_row_values(&t, &r1).expect("row exists");
        assert_eq!(row_vals.len(), 3);
        let col_keys: Vec<_> = row_vals.keys().cloned().collect();
        assert_eq!(col_keys, vec![c1, c2, c3]);

        let errors = model.find_errors();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].0, &cell3);
        assert_eq!(errors[0].1, &ExcelError::Div0);

        // Test error when querying non-existent table or row
        let missing_table = TableId::from("Missing");
        assert!(matches!(
            model.get_row_values(&missing_table, &r1),
            Err(EvalError::TableNotFound(_))
        ));

        let missing_row = RowId::from("missing_row");
        assert!(matches!(
            model.get_row_values(&t, &missing_row),
            Err(EvalError::RowNotFound { .. })
        ));
    }
}
