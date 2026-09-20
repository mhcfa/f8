use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::error::IrError;
use super::id::CellId;
use crate::expr::ast::Expr;
use crate::model::format::{CurrencyCode, FormatPreset, SemanticStyle};
use crate::model::id::{ColId, RowId, SheetId, TableId};
use crate::model::table::ValueLiteral;

/// Calculation or constant payload associated with a cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CellPayload {
    /// Constant literal value (number, text string, or null).
    Literal(ValueLiteral),
    /// Parsed formula expression tree.
    Formula(Expr),
}

impl CellPayload {
    /// Returns true if this payload is a constant literal.
    pub fn is_literal(&self) -> bool {
        matches!(self, Self::Literal(_))
    }

    /// Returns true if this payload is a parsed formula expression.
    pub fn is_formula(&self) -> bool {
        matches!(self, Self::Formula(_))
    }

    /// Borrows the literal value if present.
    pub fn as_literal(&self) -> Option<&ValueLiteral> {
        match self {
            Self::Literal(lit) => Some(lit),
            Self::Formula(_) => None,
        }
    }

    /// Borrows the formula expression if present.
    pub fn as_formula(&self) -> Option<&Expr> {
        match self {
            Self::Formula(expr) => Some(expr),
            Self::Literal(_) => None,
        }
    }
}

/// Discrete cell calculation node within the intermediate representation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellNode {
    /// Unique 3-tuple coordinate identifier `(TableId, RowId, ColId)`.
    pub id: CellId,
    /// Parent worksheet identifier.
    pub sheet: SheetId,
    /// Optional number formatting preset.
    pub format: Option<FormatPreset>,
    /// Optional display precision override.
    pub decimals: Option<u32>,
    /// Optional currency denomination code.
    pub currency: Option<CurrencyCode>,
    /// Resolved horizontal structural style (top/bottom borders).
    pub horizontal: SemanticStyle,
    /// Resolved vertical structural style (left/right borders).
    pub vertical: SemanticStyle,
    /// Visual highlight emphasis flag.
    pub is_highlight: bool,
    /// Evaluatable payload: literal value or formula AST.
    pub payload: CellPayload,
    /// Optional developer or audit note.
    pub note: Option<String>,
    /// Optional origin provenance metadata.
    pub source: Option<serde_json::Value>,
}

impl CellNode {
    /// Creates a new cell calculation node.
    pub fn new(id: CellId, sheet: SheetId, payload: CellPayload) -> Self {
        Self {
            id,
            sheet,
            format: None,
            decimals: None,
            currency: None,
            horizontal: SemanticStyle::Data,
            vertical: SemanticStyle::Data,
            is_highlight: false,
            payload,
            note: None,
            source: None,
        }
    }

    /// Builder method attaching a formatting preset.
    pub fn with_format(mut self, format: Option<FormatPreset>) -> Self {
        self.format = format;
        self
    }

    /// Builder method attaching display precision.
    pub fn with_decimals(mut self, decimals: Option<u32>) -> Self {
        self.decimals = decimals;
        self
    }

    /// Builder method attaching a currency denomination.
    pub fn with_currency(mut self, currency: Option<CurrencyCode>) -> Self {
        self.currency = currency;
        self
    }

    /// Builder method attaching horizontal and vertical structural styles.
    pub fn with_style(mut self, horizontal: SemanticStyle, vertical: SemanticStyle) -> Self {
        self.horizontal = horizontal;
        self.vertical = vertical;
        self
    }

    /// Builder method attaching visual highlight emphasis.
    pub fn with_highlight(mut self, is_highlight: bool) -> Self {
        self.is_highlight = is_highlight;
        self
    }

    /// Returns true if this cell renders with bold typography according to total style composition.
    pub fn is_bold(&self) -> bool {
        self.horizontal.is_total() || self.vertical.is_total()
    }

    /// Builder method attaching an audit note.
    pub fn with_note(mut self, note: Option<String>) -> Self {
        self.note = note;
        self
    }

    /// Builder method attaching provenance source metadata.
    pub fn with_source(mut self, source: Option<serde_json::Value>) -> Self {
        self.source = source;
        self
    }
}

/// Deterministic, ordered collection of discrete cell nodes across the model.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
pub struct CellGrid {
    cells: IndexMap<CellId, CellNode>,
}

impl CellGrid {
    /// Creates a new, empty cell grid.
    pub fn new() -> Self {
        Self {
            cells: IndexMap::new(),
        }
    }

    /// Inserts a cell node into the grid, returning an error if a cell with the same `CellId` already exists.
    pub fn insert(&mut self, cell: CellNode) -> Result<(), IrError> {
        match self.cells.entry(cell.id.clone()) {
            indexmap::map::Entry::Occupied(_) => Err(IrError::DuplicateCell { id: cell.id }),
            indexmap::map::Entry::Vacant(entry) => {
                entry.insert(cell);
                Ok(())
            }
        }
    }

    /// Returns a reference to the cell node with the specified ID, if present.
    pub fn get(&self, id: &CellId) -> Option<&CellNode> {
        self.cells.get(id)
    }

    /// Returns a mutable reference to the cell node with the specified ID, if present.
    pub fn get_mut(&mut self, id: &CellId) -> Option<&mut CellNode> {
        self.cells.get_mut(id)
    }

    /// Returns true if the grid contains a cell with the specified ID.
    pub fn contains(&self, id: &CellId) -> bool {
        self.cells.contains_key(id)
    }

    /// Returns the total number of cells in the grid.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Returns true if the grid contains no cells.
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Iterates over all cell IDs and their associated cell nodes in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&CellId, &CellNode)> {
        self.cells.iter()
    }

    /// Iterates mutably over all cell IDs and their associated cell nodes in insertion order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&CellId, &mut CellNode)> {
        self.cells.iter_mut()
    }

    /// Iterates over all cell nodes in insertion order.
    pub fn values(&self) -> impl Iterator<Item = &CellNode> {
        self.cells.values()
    }

    /// Iterates over all cell IDs in insertion order.
    pub fn keys(&self) -> impl Iterator<Item = &CellId> {
        self.cells.keys()
    }

    /// Yields all cells belonging to a specific table in insertion order.
    pub fn cells_for_table<'a, 't>(
        &'a self,
        table: &'t TableId,
    ) -> impl Iterator<Item = &'a CellNode> + use<'a, 't> {
        self.cells
            .values()
            .filter(move |cell| &cell.id.table == table)
    }

    /// Yields all cells belonging to a specific row within a table in insertion order.
    pub fn cells_for_row<'a, 't, 'r>(
        &'a self,
        table: &'t TableId,
        row: &'r RowId,
    ) -> impl Iterator<Item = &'a CellNode> + use<'a, 't, 'r> {
        self.cells
            .values()
            .filter(move |cell| &cell.id.table == table && &cell.id.row == row)
    }

    /// Yields all cells belonging to a specific column within a table in insertion order.
    pub fn cells_for_col<'a, 't, 'c>(
        &'a self,
        table: &'t TableId,
        col: &'c ColId,
    ) -> impl Iterator<Item = &'a CellNode> + use<'a, 't, 'c> {
        self.cells
            .values()
            .filter(move |cell| &cell.id.table == table && &cell.id.col == col)
    }
}

impl IntoIterator for CellGrid {
    type Item = (CellId, CellNode);
    type IntoIter = indexmap::map::IntoIter<CellId, CellNode>;

    fn into_iter(self) -> Self::IntoIter {
        self.cells.into_iter()
    }
}

impl<'a> IntoIterator for &'a CellGrid {
    type Item = (&'a CellId, &'a CellNode);
    type IntoIter = indexmap::map::Iter<'a, CellId, CellNode>;

    fn into_iter(self) -> Self::IntoIter {
        self.cells.iter()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_grid_basic_operations() {
        let mut grid = CellGrid::new();
        assert!(grid.is_empty());

        let id = CellId::new("IncomeStatement", "revenue", "2024E");
        let cell = CellNode::new(
            id.clone(),
            SheetId::new("Model"),
            CellPayload::Literal(ValueLiteral::Number(100.0)),
        );

        assert!(grid.insert(cell.clone()).is_ok());
        assert_eq!(grid.len(), 1);
        assert!(grid.contains(&id));

        // Duplicate rejection
        assert!(matches!(
            grid.insert(cell),
            Err(IrError::DuplicateCell { .. })
        ));

        let retrieved = grid.get(&id).expect("Should find cell");
        assert_eq!(retrieved.sheet, SheetId::new("Model"));
        assert!(retrieved.payload.is_literal());
        assert_eq!(
            retrieved.payload.as_literal(),
            Some(&ValueLiteral::Number(100.0))
        );
    }

    #[test]
    fn test_cell_grid_filtered_iterations() {
        let mut grid = CellGrid::new();
        let sheet = SheetId::new("Model");

        let c1 = CellNode::new(
            CellId::new("T1", "r1", "c1"),
            sheet.clone(),
            CellPayload::Literal(ValueLiteral::Number(1.0)),
        );
        let c2 = CellNode::new(
            CellId::new("T1", "r1", "c2"),
            sheet.clone(),
            CellPayload::Literal(ValueLiteral::Number(2.0)),
        );
        let c3 = CellNode::new(
            CellId::new("T1", "r2", "c1"),
            sheet.clone(),
            CellPayload::Literal(ValueLiteral::Number(3.0)),
        );
        let c4 = CellNode::new(
            CellId::new("T2", "r1", "c1"),
            sheet.clone(),
            CellPayload::Literal(ValueLiteral::Number(4.0)),
        );

        grid.insert(c1).unwrap();
        grid.insert(c2).unwrap();
        grid.insert(c3).unwrap();
        grid.insert(c4).unwrap();

        assert_eq!(grid.len(), 4);

        let keys: Vec<_> = grid.keys().collect();
        assert_eq!(keys.len(), 4);
        assert_eq!(keys[0], &CellId::new("T1", "r1", "c1"));
        assert_eq!(keys[3], &CellId::new("T2", "r1", "c1"));

        let t1_cells: Vec<_> = grid.cells_for_table(&TableId::new("T1")).collect();
        assert_eq!(t1_cells.len(), 3);

        let t1_r1_cells: Vec<_> = grid
            .cells_for_row(&TableId::new("T1"), &RowId::new("r1"))
            .collect();
        assert_eq!(t1_r1_cells.len(), 2);

        let t1_c1_cells: Vec<_> = grid
            .cells_for_col(&TableId::new("T1"), &ColId::new("c1"))
            .collect();
        assert_eq!(t1_c1_cells.len(), 2);

        let t2_cells: Vec<_> = grid.cells_for_table(&TableId::new("T2")).collect();
        assert_eq!(t2_cells.len(), 1);
    }
}
