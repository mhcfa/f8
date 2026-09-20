use indexmap::{IndexMap, IndexSet};

use super::error::ResolveError;
use crate::compiler::ir::cell::{CellGrid, CellPayload};
use crate::compiler::ir::id::CellId;
use crate::compiler::ir::symbol::{SymbolTable, TableMetadata};
use crate::expr::ast::{ColSelector, Expr, RowSelector, SpecialCol, SpecialRow, TableRef};
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

/// Resolves a row selector against target and origin table metadata.
pub(crate) fn resolve_row_selector(
    origin: &CellId,
    target_table: &TableMetadata,
    origin_table: &TableMetadata,
    row_selector: &RowSelector,
) -> Result<Vec<RowId>, ResolveError> {
    match row_selector {
        RowSelector::Active => {
            if !target_table.rows.contains(&origin.row) {
                return Err(ResolveError::active_row_mismatch(
                    origin,
                    target_table.table_id.clone(),
                    origin.row.clone(),
                ));
            }
            Ok(vec![origin.row.clone()])
        }
        RowSelector::Key(row_id) => {
            if !target_table.rows.contains(row_id) {
                return Err(ResolveError::unknown_row(
                    origin,
                    target_table.table_id.clone(),
                    row_id.clone(),
                ));
            }
            Ok(vec![row_id.clone()])
        }
        RowSelector::Range(start, end) => {
            let start_idx = target_table.rows.get_index_of(start).ok_or_else(|| {
                ResolveError::unknown_row(origin, target_table.table_id.clone(), start.clone())
            })?;
            let end_idx = target_table.rows.get_index_of(end).ok_or_else(|| {
                ResolveError::unknown_row(origin, target_table.table_id.clone(), end.clone())
            })?;

            if start_idx > end_idx {
                return Err(ResolveError::inverted_range(
                    origin,
                    target_table.table_id.clone(),
                    start.to_string(),
                    end.to_string(),
                ));
            }

            Ok(target_table.rows[start_idx..=end_idx]
                .iter()
                .cloned()
                .collect())
        }
        RowSelector::Special(special) => match special {
            SpecialRow::Prev(k) => {
                let origin_idx = origin_table.rows.get_index_of(&origin.row).ok_or_else(|| {
                    ResolveError::unknown_row(
                        origin,
                        origin_table.table_id.clone(),
                        origin.row.clone(),
                    )
                })?;

                let k_usize = *k as usize;
                if origin_idx < k_usize {
                    return Err(ResolveError::temporal_offset_out_of_bounds(
                        origin,
                        -(*k as i32),
                    ));
                }

                let target_idx = origin_idx - k_usize;
                let target_row_id = origin_table.rows[target_idx].clone();

                if !target_table.rows.contains(&target_row_id) {
                    return Err(ResolveError::unknown_row(
                        origin,
                        target_table.table_id.clone(),
                        target_row_id,
                    ));
                }

                Ok(vec![target_row_id])
            }
            SpecialRow::Next(k) => {
                let origin_idx = origin_table.rows.get_index_of(&origin.row).ok_or_else(|| {
                    ResolveError::unknown_row(
                        origin,
                        origin_table.table_id.clone(),
                        origin.row.clone(),
                    )
                })?;

                let k_usize = *k as usize;
                let target_idx = match origin_idx.checked_add(k_usize) {
                    Some(idx) if idx < origin_table.rows.len() => idx,
                    _ => {
                        return Err(ResolveError::temporal_offset_out_of_bounds(
                            origin, *k as i32,
                        ));
                    }
                };

                let target_row_id = origin_table.rows[target_idx].clone();

                if !target_table.rows.contains(&target_row_id) {
                    return Err(ResolveError::unknown_row(
                        origin,
                        target_table.table_id.clone(),
                        target_row_id,
                    ));
                }

                Ok(vec![target_row_id])
            }
            SpecialRow::Group(group_id) => {
                let rows = target_table.row_groups.get(group_id).ok_or_else(|| {
                    ResolveError::unknown_row_group(
                        origin,
                        target_table.table_id.clone(),
                        group_id.clone(),
                    )
                })?;
                Ok(rows.clone())
            }
            SpecialRow::GroupFirst(group_id) => {
                let rows = target_table.row_groups.get(group_id).ok_or_else(|| {
                    ResolveError::unknown_row_group(
                        origin,
                        target_table.table_id.clone(),
                        group_id.clone(),
                    )
                })?;
                let first = rows.first().ok_or_else(|| {
                    ResolveError::unknown_row_group(
                        origin,
                        target_table.table_id.clone(),
                        group_id.clone(),
                    )
                })?;
                Ok(vec![first.clone()])
            }
            SpecialRow::GroupLast(group_id) => {
                let rows = target_table.row_groups.get(group_id).ok_or_else(|| {
                    ResolveError::unknown_row_group(
                        origin,
                        target_table.table_id.clone(),
                        group_id.clone(),
                    )
                })?;
                let last = rows.last().ok_or_else(|| {
                    ResolveError::unknown_row_group(
                        origin,
                        target_table.table_id.clone(),
                        group_id.clone(),
                    )
                })?;
                Ok(vec![last.clone()])
            }
        },
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
            if !target_table.column_index.contains_key(&origin.col) {
                return Err(ResolveError::active_column_mismatch(
                    origin,
                    target_table.table_id.clone(),
                    origin.col.clone(),
                ));
            }
            Ok(vec![origin.col.clone()])
        }
        ColSelector::Key(col_id) => {
            if !target_table.column_index.contains_key(col_id) {
                return Err(ResolveError::unknown_column(
                    origin,
                    target_table.table_id.clone(),
                    col_id.clone(),
                ));
            }
            Ok(vec![col_id.clone()])
        }
        ColSelector::Range(start, end) => {
            let start_idx = target_table
                .column_index
                .get(start)
                .copied()
                .ok_or_else(|| {
                    ResolveError::unknown_column(
                        origin,
                        target_table.table_id.clone(),
                        start.clone(),
                    )
                })?;
            let end_idx = target_table.column_index.get(end).copied().ok_or_else(|| {
                ResolveError::unknown_column(origin, target_table.table_id.clone(), end.clone())
            })?;

            if start_idx > end_idx {
                return Err(ResolveError::inverted_range(
                    origin,
                    target_table.table_id.clone(),
                    start.to_string(),
                    end.to_string(),
                ));
            }

            let cols = target_table.columns[start_idx..=end_idx]
                .iter()
                .map(|c| c.id.clone())
                .collect();
            Ok(cols)
        }
        ColSelector::Special(special) => match special {
            SpecialCol::Prev(k) => {
                let origin_idx = origin_table
                    .column_index
                    .get(&origin.col)
                    .copied()
                    .ok_or_else(|| {
                        ResolveError::unknown_column(
                            origin,
                            origin_table.table_id.clone(),
                            origin.col.clone(),
                        )
                    })?;

                let k_usize = *k as usize;
                if origin_idx < k_usize {
                    return Err(ResolveError::temporal_offset_out_of_bounds(
                        origin,
                        -(*k as i32),
                    ));
                }

                let target_idx = origin_idx - k_usize;
                let target_col_id = origin_table.columns[target_idx].id.clone();

                if !target_table.column_index.contains_key(&target_col_id) {
                    return Err(ResolveError::unknown_column(
                        origin,
                        target_table.table_id.clone(),
                        target_col_id,
                    ));
                }

                Ok(vec![target_col_id])
            }
            SpecialCol::Next(k) => {
                let origin_idx = origin_table
                    .column_index
                    .get(&origin.col)
                    .copied()
                    .ok_or_else(|| {
                        ResolveError::unknown_column(
                            origin,
                            origin_table.table_id.clone(),
                            origin.col.clone(),
                        )
                    })?;

                let k_usize = *k as usize;
                let target_idx = match origin_idx.checked_add(k_usize) {
                    Some(idx) if idx < origin_table.columns.len() => idx,
                    _ => {
                        return Err(ResolveError::temporal_offset_out_of_bounds(
                            origin, *k as i32,
                        ));
                    }
                };

                let target_col_id = origin_table.columns[target_idx].id.clone();

                if !target_table.column_index.contains_key(&target_col_id) {
                    return Err(ResolveError::unknown_column(
                        origin,
                        target_table.table_id.clone(),
                        target_col_id,
                    ));
                }

                Ok(vec![target_col_id])
            }
            SpecialCol::Group(group_id) => {
                let cols = target_table.column_groups.get(group_id).ok_or_else(|| {
                    ResolveError::unknown_column_group(
                        origin,
                        target_table.table_id.clone(),
                        group_id.clone(),
                    )
                })?;
                Ok(cols.clone())
            }
            SpecialCol::GroupFirst(group_id) => {
                let cols = target_table.column_groups.get(group_id).ok_or_else(|| {
                    ResolveError::unknown_column_group(
                        origin,
                        target_table.table_id.clone(),
                        group_id.clone(),
                    )
                })?;
                let first = cols.first().ok_or_else(|| {
                    ResolveError::unknown_column_group(
                        origin,
                        target_table.table_id.clone(),
                        group_id.clone(),
                    )
                })?;
                Ok(vec![first.clone()])
            }
            SpecialCol::GroupLast(group_id) => {
                let cols = target_table.column_groups.get(group_id).ok_or_else(|| {
                    ResolveError::unknown_column_group(
                        origin,
                        target_table.table_id.clone(),
                        group_id.clone(),
                    )
                })?;
                let last = cols.last().ok_or_else(|| {
                    ResolveError::unknown_column_group(
                        origin,
                        target_table.table_id.clone(),
                        group_id.clone(),
                    )
                })?;
                Ok(vec![last.clone()])
            }
        },
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
            columns: vec![ColumnSpec::new("2024E")],
            column_groups: vec![],
            row_groups: vec![],
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
