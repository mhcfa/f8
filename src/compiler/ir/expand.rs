use std::path::Path;

use super::cell::{CellGrid, CellNode, CellPayload};
use super::error::IrError;
use super::id::CellId;
use super::symbol::SymbolTable;
use crate::model::id::{SheetId, TableId};
use crate::model::manifest::ProjectManifest;
use crate::model::table::{MatchSelector, TableSpec};

/// Expands a single table specification into an ordered vector of discrete cell calculation nodes.
///
/// Ensures strict declaration ordering: rows in `table.rows` order, and columns in `table.columns` order.
pub fn expand_table(
    symbol_table: &SymbolTable,
    table: &TableSpec,
) -> Result<Vec<CellNode>, IrError> {
    table.validate()?;

    let sheet_id =
        symbol_table
            .get_sheet(&table.id)
            .cloned()
            .ok_or_else(|| IrError::UnknownTable {
                table: table.id.clone(),
                context: "grid expansion".to_string(),
            })?;

    let mut cells = Vec::with_capacity(table.rows.len() * table.columns.len());

    for row in &table.rows {
        let row_format = row.format;
        let row_note = row.note.as_deref();
        let row_source = row.source.as_ref();

        if let Some(values) = &row.value {
            for (col_id, val) in table.columns.iter().zip(values) {
                let id = CellId::new(table.id.clone(), row.id.clone(), col_id.clone());
                let payload = CellPayload::Literal(val.clone());
                let mut node = CellNode::new(id, sheet_id.clone(), payload).with_format(row_format);
                if let Some(note) = row_note {
                    node = node.with_note(Some(note.to_string()));
                }
                if let Some(source) = row_source {
                    node = node.with_source(Some(source.clone()));
                }
                cells.push(node);
            }
        } else if let Some(expr_str) = &row.expr {
            let parsed_expr =
                crate::expr::ast::Expr::parse(expr_str).map_err(|err| IrError::FormulaParse {
                    table: table.id.clone(),
                    row: row.id.clone(),
                    column: None,
                    source: err,
                })?;

            for col_id in &table.columns {
                let id = CellId::new(table.id.clone(), row.id.clone(), col_id.clone());
                let payload = CellPayload::Formula(parsed_expr.clone());
                let mut node = CellNode::new(id, sheet_id.clone(), payload).with_format(row_format);
                if let Some(note) = row_note {
                    node = node.with_note(Some(note.to_string()));
                }
                if let Some(source) = row_source {
                    node = node.with_source(Some(source.clone()));
                }
                cells.push(node);
            }
        } else if let Some(arms) = &row.match_arms {
            let mut row_cells: Vec<Option<CellNode>> = vec![None; table.columns.len()];

            for (selector_str, arm) in arms {
                let selector = MatchSelector::parse(selector_str).map_err(|reason| {
                    crate::model::error::ModelError::InvalidMatchSelector {
                        table: table.id.clone(),
                        row: row.id.clone(),
                        selector: selector_str.clone(),
                        reason,
                    }
                })?;

                let target_columns = selector.resolve_columns(table)?;
                let arm_note = arm.note.as_deref().or(row_note);
                let arm_source = arm.source.as_ref().or(row_source);

                if let Some(arm_values) = &arm.value {
                    for (col_id, val) in target_columns.iter().zip(arm_values) {
                        let col_idx =
                            symbol_table.col_index(&table.id, col_id).ok_or_else(|| {
                                IrError::UnknownColumn {
                                    table: table.id.clone(),
                                    column: col_id.clone(),
                                    context: format!("match arm in row '{}'", row.id),
                                }
                            })?;

                        let id = CellId::new(table.id.clone(), row.id.clone(), col_id.clone());
                        let payload = CellPayload::Literal(val.clone());
                        let mut node =
                            CellNode::new(id, sheet_id.clone(), payload).with_format(row_format);
                        if let Some(note) = arm_note {
                            node = node.with_note(Some(note.to_string()));
                        }
                        if let Some(source) = arm_source {
                            node = node.with_source(Some(source.clone()));
                        }
                        let slot =
                            row_cells
                                .get_mut(col_idx)
                                .ok_or_else(|| IrError::UnknownColumn {
                                    table: table.id.clone(),
                                    column: col_id.clone(),
                                    context: format!(
                                        "column index out of bounds in row '{}'",
                                        row.id
                                    ),
                                })?;
                        *slot = Some(node);
                    }
                } else if let Some(arm_expr_str) = &arm.expr {
                    let parsed_expr =
                        crate::expr::ast::Expr::parse(arm_expr_str).map_err(|err| {
                            IrError::FormulaParse {
                                table: table.id.clone(),
                                row: row.id.clone(),
                                column: None,
                                source: err,
                            }
                        })?;

                    for col_id in &target_columns {
                        let col_idx =
                            symbol_table.col_index(&table.id, col_id).ok_or_else(|| {
                                IrError::UnknownColumn {
                                    table: table.id.clone(),
                                    column: col_id.clone(),
                                    context: format!("match arm in row '{}'", row.id),
                                }
                            })?;

                        let id = CellId::new(table.id.clone(), row.id.clone(), col_id.clone());
                        let payload = CellPayload::Formula(parsed_expr.clone());
                        let mut node =
                            CellNode::new(id, sheet_id.clone(), payload).with_format(row_format);
                        if let Some(note) = arm_note {
                            node = node.with_note(Some(note.to_string()));
                        }
                        if let Some(source) = arm_source {
                            node = node.with_source(Some(source.clone()));
                        }
                        let slot =
                            row_cells
                                .get_mut(col_idx)
                                .ok_or_else(|| IrError::UnknownColumn {
                                    table: table.id.clone(),
                                    column: col_id.clone(),
                                    context: format!(
                                        "column index out of bounds in row '{}'",
                                        row.id
                                    ),
                                })?;
                        *slot = Some(node);
                    }
                }
            }

            for (col_idx, cell_opt) in row_cells.into_iter().enumerate() {
                let cell = cell_opt.ok_or_else(|| IrError::UnknownColumn {
                    table: table.id.clone(),
                    column: table.columns[col_idx].clone(),
                    context: format!("uncovered column in row '{}'", row.id),
                })?;
                cells.push(cell);
            }
        }
    }

    Ok(cells)
}

/// Expands a slice of table specifications into a populated `CellGrid`.
///
/// Insertion preserves deterministic declaration order: tables, then rows, then columns.
pub fn expand_grid(symbol_table: &SymbolTable, tables: &[TableSpec]) -> Result<CellGrid, IrError> {
    let mut grid = CellGrid::new();

    for table in tables {
        let nodes = expand_table(symbol_table, table)?;
        for node in nodes {
            grid.insert(node)?;
        }
    }

    Ok(grid)
}

/// Helper to match a path string from `SheetSpec.tables` to a `TableId`.
fn matches_table_id(path: &Path, table_id: &TableId) -> bool {
    let id_str = table_id.as_str();
    let norm_id = id_str.replace(['_', '-'], "");

    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
        if stem.eq_ignore_ascii_case(id_str) {
            return true;
        }
        let norm_stem = stem.replace(['_', '-'], "");
        if norm_stem.eq_ignore_ascii_case(&norm_id) {
            return true;
        }
    }

    false
}

/// Orchestrates workbook symbol registration and grid expansion from a manifest and table specs.
pub fn expand_workbook(
    manifest: &ProjectManifest,
    tables: &[TableSpec],
) -> Result<(SymbolTable, CellGrid), IrError> {
    manifest.validate()?;

    let mut symbol_table = SymbolTable::new();

    for sheet in &manifest.sheets {
        let sheet_id = SheetId::from(sheet.name.as_str());

        for table_path in &sheet.tables {
            let matched_table = tables
                .iter()
                .find(|t| matches_table_id(table_path, &t.id))
                .ok_or_else(|| IrError::UnknownTable {
                    table: TableId::from(
                        table_path
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or_default(),
                    ),
                    context: format!("sheet '{}'", sheet.name),
                })?;

            symbol_table.register_table(sheet_id.clone(), matched_table)?;
        }
    }

    // Verify all input tables belong to a sheet in the manifest
    for table in tables {
        if !symbol_table.contains_table(&table.id) {
            return Err(IrError::UnknownTable {
                table: table.id.clone(),
                context: "unassigned table not present in manifest sheets".to_string(),
            });
        }
    }

    let grid = expand_grid(&symbol_table, tables)?;
    Ok((symbol_table, grid))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::model::format::FormatPreset;
    use crate::model::id::{ColGroupId, ColId, RowId};
    use crate::model::table::{ColumnGroup, MatchArmSpec, RowSpec, ValueLiteral};
    use indexmap::IndexMap;

    fn sample_table() -> TableSpec {
        TableSpec {
            id: TableId::from("IncomeStatement"),
            title: "Income Statement".to_string(),
            columns: vec![
                ColId::from("2023A"),
                ColId::from("2024E"),
                ColId::from("2025E"),
            ],
            column_groups: vec![
                ColumnGroup {
                    id: ColGroupId::from("historical"),
                    label: "Historical".to_string(),
                    columns: vec![ColId::from("2023A")],
                },
                ColumnGroup {
                    id: ColGroupId::from("forecast"),
                    label: "Forecast".to_string(),
                    columns: vec![ColId::from("2024E"), ColId::from("2025E")],
                },
            ],
            row_groups: vec![],
            rows: vec![
                RowSpec {
                    id: RowId::from("revenue"),
                    label: "Revenue".to_string(),
                    format: Some(FormatPreset::Currency),
                    note: Some("Base revenue".to_string()),
                    source: None,
                    value: None,
                    expr: None,
                    match_arms: Some(IndexMap::from([
                        (
                            "2023A".to_string(),
                            MatchArmSpec {
                                value: Some(vec![ValueLiteral::Number(100.0)]),
                                expr: None,
                                note: None,
                                source: None,
                            },
                        ),
                        (
                            "cols.forecast".to_string(),
                            MatchArmSpec {
                                value: None,
                                expr: Some("IncomeStatement.revenue[col.prev] * 1.05".to_string()),
                                note: Some("5% growth".to_string()),
                                source: None,
                            },
                        ),
                    ])),
                },
                RowSpec {
                    id: RowId::from("tax_rate"),
                    label: "Tax Rate".to_string(),
                    format: Some(FormatPreset::Percentage),
                    note: None,
                    source: None,
                    value: Some(vec![
                        ValueLiteral::Number(0.21),
                        ValueLiteral::Number(0.21),
                        ValueLiteral::Number(0.21),
                    ]),
                    expr: None,
                    match_arms: None,
                },
            ],
        }
    }

    #[test]
    fn test_expand_table_ordering_and_payloads() {
        let table = sample_table();
        let mut symbols = SymbolTable::new();
        symbols
            .register_table(SheetId::from("Model"), &table)
            .unwrap();

        let cells = expand_table(&symbols, &table).unwrap();
        assert_eq!(cells.len(), 6);

        // Row 1: revenue
        assert_eq!(
            cells[0].id,
            CellId::new("IncomeStatement", "revenue", "2023A")
        );
        assert_eq!(
            cells[0].payload,
            CellPayload::Literal(ValueLiteral::Number(100.0))
        );
        assert_eq!(cells[0].format, Some(FormatPreset::Currency));
        assert_eq!(cells[0].note.as_deref(), Some("Base revenue"));

        assert_eq!(
            cells[1].id,
            CellId::new("IncomeStatement", "revenue", "2024E")
        );
        assert!(cells[1].payload.is_formula());
        assert_eq!(cells[1].note.as_deref(), Some("5% growth"));

        assert_eq!(
            cells[2].id,
            CellId::new("IncomeStatement", "revenue", "2025E")
        );
        assert!(cells[2].payload.is_formula());
        assert_eq!(cells[2].note.as_deref(), Some("5% growth"));

        // Row 2: tax_rate
        assert_eq!(
            cells[3].id,
            CellId::new("IncomeStatement", "tax_rate", "2023A")
        );
        assert_eq!(
            cells[3].payload,
            CellPayload::Literal(ValueLiteral::Number(0.21))
        );
        assert_eq!(cells[3].format, Some(FormatPreset::Percentage));
    }

    #[test]
    fn test_expand_grid_inserts_into_grid() {
        let table = sample_table();
        let mut symbols = SymbolTable::new();
        symbols
            .register_table(SheetId::from("Model"), &table)
            .unwrap();

        let grid = expand_grid(&symbols, &[table]).unwrap();
        assert_eq!(grid.len(), 6);
        assert!(grid.contains(&CellId::new("IncomeStatement", "revenue", "2023A")));
        assert!(grid.contains(&CellId::new("IncomeStatement", "tax_rate", "2025E")));
    }

    #[test]
    fn test_expand_table_rejects_unregistered() {
        let table = sample_table();
        let symbols = SymbolTable::new();
        let err = expand_table(&symbols, &table).unwrap_err();
        assert!(matches!(err, IrError::UnknownTable { .. }));
    }

    #[test]
    fn test_expand_table_syntax_error() {
        let mut table = sample_table();
        table.rows[1] = RowSpec {
            id: RowId::from("bad_formula"),
            label: "Bad Formula".to_string(),
            format: None,
            note: None,
            source: None,
            value: None,
            expr: Some("IncomeStatement.revenue[@] + * 2".to_string()),
            match_arms: None,
        };

        let mut symbols = SymbolTable::new();
        symbols
            .register_table(SheetId::from("Model"), &table)
            .unwrap();

        let err = expand_table(&symbols, &table).unwrap_err();
        assert!(matches!(err, IrError::FormulaParse { .. }));
    }
}
