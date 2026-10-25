use indexmap::{IndexMap, IndexSet};

use super::error::ResolveError;
use crate::compiler::ir::cell::{CellGrid, CellPayload};
use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::{SymbolTable, TableMetadata};
use crate::expr::ast::{
    ColEndpoint, ColSelector, Expr, RelativeOffset, RowEndpoint, RowSelector, TableRef,
};
use crate::model::id::{ColId, RowId};

/// Recursively collects all table references present within a formula expression AST.
pub fn collect_references(expr: &Expr) -> Vec<&TableRef> {
    let mut refs = Vec::new();
    collect_refs_recursive(expr, &mut refs);
    refs
}

fn collect_refs_recursive<'a>(expr: &'a Expr, out: &mut Vec<&'a TableRef>) {
    match expr {
        Expr::Literal(_) => {}
        Expr::Reference(r) => out.push(r),
        Expr::Unary { expr, .. } => collect_refs_recursive(expr, out),
        Expr::Binary { left, right, .. } => {
            collect_refs_recursive(left, out);
            collect_refs_recursive(right, out);
        }
        Expr::FunctionCall { args, .. } => {
            for arg in args {
                collect_refs_recursive(arg, out);
            }
        }
        Expr::Omitted => {}
    }
}

/// Resolves an explicit row key to its 0-based index and canonical [`RowId`] in `target_table.rows`.
fn resolve_row_key<'a>(
    origin: &CellId,
    target_table: &'a TableMetadata,
    row_id: &RowId,
) -> Result<(usize, &'a RowId), ResolveError> {
    target_table.rows.get_full(row_id).ok_or_else(|| {
        ResolveError::unknown_row(origin, target_table.table_id.clone(), row_id.clone())
    })
}

/// Resolves a relative row offset (`row.prev`, `row.next`) to its 0-based index and [`RowId`] in `target_table.rows`.
fn resolve_row_offset<'a>(
    origin: &CellId,
    target_table: &'a TableMetadata,
    origin_table: &TableMetadata,
    offset: RelativeOffset,
) -> Result<(usize, &'a RowId), ResolveError> {
    let origin_idx = origin_table.rows.get_index_of(&origin.row).ok_or_else(|| {
        ResolveError::unknown_row(origin, origin_table.table_id.clone(), origin.row.clone())
    })?;

    let signed_offset = match offset {
        RelativeOffset::Prev(k) => -(k as i32),
        RelativeOffset::Next(k) => k as i32,
    };
    let target_idx = match offset {
        RelativeOffset::Prev(k) => origin_idx
            .checked_sub(k as usize)
            .ok_or_else(|| ResolveError::temporal_offset_out_of_bounds(origin, signed_offset))?,
        RelativeOffset::Next(k) => origin_idx
            .checked_add(k as usize)
            .ok_or_else(|| ResolveError::temporal_offset_out_of_bounds(origin, signed_offset))?,
    };

    let target_row_id = origin_table
        .rows
        .get_index(target_idx)
        .ok_or_else(|| ResolveError::temporal_offset_out_of_bounds(origin, signed_offset))?;
    resolve_row_key(origin, target_table, target_row_id)
}

/// Resolves a single row endpoint to its 0-based index and canonical [`RowId`] in `target_table.rows`.
fn resolve_row_endpoint<'a>(
    origin: &CellId,
    target_table: &'a TableMetadata,
    origin_table: &TableMetadata,
    endpoint: &RowEndpoint,
) -> Result<(usize, &'a RowId), ResolveError> {
    match endpoint {
        RowEndpoint::Active => target_table.rows.get_full(&origin.row).ok_or_else(|| {
            ResolveError::active_row_mismatch(
                origin,
                target_table.table_id.clone(),
                origin.row.clone(),
            )
        }),
        RowEndpoint::Key(row_id) => resolve_row_key(origin, target_table, row_id),
        RowEndpoint::Offset(offset) => {
            resolve_row_offset(origin, target_table, origin_table, *offset)
        }
    }
}

/// Resolves a row selector against target and origin table metadata.
pub(crate) fn resolve_row_selector(
    origin: &CellId,
    target_table: &TableMetadata,
    origin_table: &TableMetadata,
    row_selector: &RowSelector,
) -> Result<Vec<RowId>, ResolveError> {
    match row_selector {
        RowSelector::Active => {
            let (_, id) =
                resolve_row_endpoint(origin, target_table, origin_table, &RowEndpoint::Active)?;
            Ok(vec![id.clone()])
        }
        RowSelector::Key(row_id) => {
            let (_, id) = resolve_row_key(origin, target_table, row_id)?;
            Ok(vec![id.clone()])
        }
        RowSelector::Offset(offset) => {
            let (_, id) = resolve_row_offset(origin, target_table, origin_table, *offset)?;
            Ok(vec![id.clone()])
        }
        RowSelector::Range(start, end) => {
            let (start_idx, start_id) =
                resolve_row_endpoint(origin, target_table, origin_table, start)?;
            let (end_idx, end_id) = resolve_row_endpoint(origin, target_table, origin_table, end)?;

            if start_idx > end_idx {
                return Err(ResolveError::inverted_range(
                    origin,
                    target_table.table_id.clone(),
                    start_id.to_string(),
                    end_id.to_string(),
                ));
            }

            let slice = target_table
                .rows
                .get_range(start_idx..=end_idx)
                .ok_or_else(|| {
                    ResolveError::inverted_range(
                        origin,
                        target_table.table_id.clone(),
                        start_id.to_string(),
                        end_id.to_string(),
                    )
                })?;

            Ok(slice.iter().cloned().collect())
        }
    }
}

/// Resolves an explicit column key to its 0-based index and canonical [`ColId`] in `target_table.columns`.
fn resolve_col_key<'a>(
    origin: &CellId,
    target_table: &'a TableMetadata,
    col_id: &ColId,
) -> Result<(usize, &'a ColId), ResolveError> {
    let idx = target_table
        .column_index
        .get(col_id)
        .copied()
        .ok_or_else(|| {
            ResolveError::unknown_column(origin, target_table.table_id.clone(), col_id.clone())
        })?;
    let col = target_table.columns.get(idx).ok_or_else(|| {
        ResolveError::unknown_column(origin, target_table.table_id.clone(), col_id.clone())
    })?;
    Ok((idx, &col.id))
}

/// Resolves a relative column offset (`col.prev`, `col.next`) to its 0-based index and [`ColId`] in `target_table.columns`.
fn resolve_col_offset<'a>(
    origin: &CellId,
    target_table: &'a TableMetadata,
    origin_table: &TableMetadata,
    offset: RelativeOffset,
) -> Result<(usize, &'a ColId), ResolveError> {
    let signed_offset = match offset {
        RelativeOffset::Prev(k) => -(k as i32),
        RelativeOffset::Next(k) => k as i32,
    };
    let origin_measure_idx = origin_table
        .measure_columns
        .iter()
        .position(|c| c == &origin.col)
        .ok_or_else(|| ResolveError::temporal_offset_out_of_bounds(origin, signed_offset))?;

    let target_measure_idx = match offset {
        RelativeOffset::Prev(k) => origin_measure_idx
            .checked_sub(k as usize)
            .ok_or_else(|| ResolveError::temporal_offset_out_of_bounds(origin, signed_offset))?,
        RelativeOffset::Next(k) => origin_measure_idx
            .checked_add(k as usize)
            .ok_or_else(|| ResolveError::temporal_offset_out_of_bounds(origin, signed_offset))?,
    };

    let target_col_id = origin_table
        .measure_columns
        .get(target_measure_idx)
        .ok_or_else(|| ResolveError::temporal_offset_out_of_bounds(origin, signed_offset))?;
    resolve_col_key(origin, target_table, target_col_id)
}

/// Resolves a single column endpoint to its 0-based index and canonical [`ColId`] in `target_table.columns`.
fn resolve_col_endpoint<'a>(
    origin: &CellId,
    target_table: &'a TableMetadata,
    origin_table: &TableMetadata,
    endpoint: &ColEndpoint,
) -> Result<(usize, &'a ColId), ResolveError> {
    match endpoint {
        ColEndpoint::Active => {
            let idx = target_table
                .column_index
                .get(&origin.col)
                .copied()
                .ok_or_else(|| {
                    ResolveError::active_column_mismatch(
                        origin,
                        target_table.table_id.clone(),
                        origin.col.clone(),
                    )
                })?;
            let col = target_table.columns.get(idx).ok_or_else(|| {
                ResolveError::active_column_mismatch(
                    origin,
                    target_table.table_id.clone(),
                    origin.col.clone(),
                )
            })?;
            Ok((idx, &col.id))
        }
        ColEndpoint::Key(col_id) => resolve_col_key(origin, target_table, col_id),
        ColEndpoint::Offset(offset) => {
            resolve_col_offset(origin, target_table, origin_table, *offset)
        }
    }
}

/// Resolves a column selector against target and origin table metadata.
pub(crate) fn resolve_col_selector(
    origin: &CellId,
    target_table: &TableMetadata,
    origin_table: &TableMetadata,
    col_selector: &ColSelector,
) -> Result<Vec<ColId>, ResolveError> {
    match col_selector {
        ColSelector::Active => {
            let (_, id) =
                resolve_col_endpoint(origin, target_table, origin_table, &ColEndpoint::Active)?;
            Ok(vec![id.clone()])
        }
        ColSelector::Key(col_id) => {
            let (_, id) = resolve_col_key(origin, target_table, col_id)?;
            Ok(vec![id.clone()])
        }
        ColSelector::Offset(offset) => {
            let (_, id) = resolve_col_offset(origin, target_table, origin_table, *offset)?;
            Ok(vec![id.clone()])
        }
        ColSelector::Range(start, end) => {
            let (start_idx, start_id) =
                resolve_col_endpoint(origin, target_table, origin_table, start)?;
            let (end_idx, end_id) = resolve_col_endpoint(origin, target_table, origin_table, end)?;

            if start_idx > end_idx {
                return Err(ResolveError::inverted_range(
                    origin,
                    target_table.table_id.clone(),
                    start_id.to_string(),
                    end_id.to_string(),
                ));
            }

            let slice = target_table
                .columns
                .get(start_idx..=end_idx)
                .ok_or_else(|| {
                    ResolveError::inverted_range(
                        origin,
                        target_table.table_id.clone(),
                        start_id.to_string(),
                        end_id.to_string(),
                    )
                })?;

            Ok(slice.iter().map(|c| c.id.clone()).collect())
        }
    }
}

/// Two-dimensional slice of resolved cell coordinates preserving matrix dimensions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSlice {
    /// Number of rows in the resolved slice.
    pub rows: usize,
    /// Number of columns in the resolved slice.
    pub cols: usize,
    /// Flattened row-major list of cell coordinates.
    pub cells: Vec<CellId>,
}

impl ResolvedSlice {
    /// Creates a new `ResolvedSlice` with specified row and column dimensions.
    #[must_use]
    pub fn new(rows: usize, cols: usize, cells: Vec<CellId>) -> Self {
        Self { rows, cols, cells }
    }

    /// Returns `true` if the slice contains no cells.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Returns the total number of cells in the slice.
    #[must_use]
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    /// Borrows the resolved cells as a slice.
    #[must_use]
    pub fn cells(&self) -> &[CellId] {
        &self.cells
    }

    /// Consumes the slice and returns the underlying vector of cell coordinates.
    #[must_use]
    pub fn into_cells(self) -> Vec<CellId> {
        self.cells
    }
}

impl std::ops::Deref for ResolvedSlice {
    type Target = [CellId];

    fn deref(&self) -> &Self::Target {
        &self.cells
    }
}

impl IntoIterator for ResolvedSlice {
    type Item = CellId;
    type IntoIter = std::vec::IntoIter<CellId>;

    fn into_iter(self) -> Self::IntoIter {
        self.cells.into_iter()
    }
}

impl<'a> IntoIterator for &'a ResolvedSlice {
    type Item = &'a CellId;
    type IntoIter = std::slice::Iter<'a, CellId>;

    fn into_iter(self) -> Self::IntoIter {
        self.cells.iter()
    }
}

/// Resolves a single TableRef expression in the context of an evaluating cell coordinate.
pub fn resolve_table_ref(
    origin: &CellId,
    table_ref: &TableRef,
    symbol_table: &SymbolTable,
) -> Result<ResolvedSlice, ResolveError> {
    let origin_table = symbol_table
        .get_table(&origin.table)
        .ok_or_else(|| ResolveError::unregistered_table(origin, origin.table.clone()))?;

    let target_table = symbol_table
        .get_table(&table_ref.table)
        .ok_or_else(|| ResolveError::unknown_table(origin, table_ref.table.clone()))?;

    let rows = resolve_row_selector(origin, target_table, origin_table, &table_ref.row)?;
    let cols = resolve_col_selector(origin, target_table, origin_table, &table_ref.col)?;

    let num_rows = rows.len();
    let num_cols = cols.len();
    let mut resolved = Vec::with_capacity(num_rows * num_cols);
    for row in rows {
        for col in &cols {
            resolved.push(CellId::new(
                table_ref.table.clone(),
                row.clone(),
                col.clone(),
            ));
        }
    }
    Ok(ResolvedSlice::new(num_rows, num_cols, resolved))
}

/// Traverses all formulas in the cell grid, resolving each cell's references into discrete target CellIds.
pub fn resolve_cell_dependencies(
    grid: &CellGrid,
    symbol_table: &SymbolTable,
) -> Result<IndexMap<CellId, IndexSet<CellId>>, ResolveError> {
    let mut dependencies = IndexMap::with_capacity(grid.len());

    for cell in grid.values() {
        let mut cell_deps = IndexSet::new();

        if let CellPayload::Formula(ref expr) = cell.payload {
            let refs = collect_references(expr);
            for table_ref in refs {
                let targets = resolve_table_ref(&cell.id, table_ref, symbol_table)?;
                cell_deps.extend(targets);
            }
        }

        dependencies.insert(cell.id.clone(), cell_deps);
    }

    Ok(dependencies)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::model::id::{SheetId, TableId};
    use crate::model::table::{ColumnSpec, RowSpec, TableSpec};

    #[test]
    fn test_collect_references_nested_expression() {
        let formula = "SUM(TableA[\"rev\", @col] * 2, TableB[\"cogs\", col.prev] + -TableC[\"tax\", \"rate\"])";
        let expr = Expr::parse(formula).expect("valid formula");
        let refs = collect_references(&expr);
        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].table, TableId::from("TableA"));
        assert_eq!(refs[1].table, TableId::from("TableB"));
        assert_eq!(refs[2].table, TableId::from("TableC"));
    }

    #[test]
    fn test_unregistered_origin_table_rejected() {
        let mut symbols = SymbolTable::new();
        let target_table = TableSpec {
            id: TableId::from("Target"),
            title: "Target".into(),
            subtitle: None,
            currency: None,
            collapsible: false,
            placement: crate::model::table::PlacementSpec::default(),
            columns: vec![ColumnSpec::new("2024E")],
            rows: vec![RowSpec::new("rev")],
        };
        symbols
            .register_table(SheetId::from("Sheet1"), &target_table)
            .expect("register ok");

        let origin = CellId::new("GhostTable", "rev", "2024E");
        let table_ref = TableRef::new(
            "Target",
            RowSelector::Key(RowId::from("rev")),
            ColSelector::Key(ColId::from("2024E")),
        );

        let err = resolve_table_ref(&origin, &table_ref, &symbols).unwrap_err();
        assert_eq!(err, ResolveError::unregistered_table(&origin, "GhostTable"));
    }
}
