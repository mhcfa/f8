use indexmap::IndexMap;
use std::path::Path;

use super::cell::{CellGrid, CellNode, CellPayload};
use super::error::IrError;
use super::id::CellId;
use super::symbol::SymbolTable;
use crate::model::id::{ColId, RowId, SheetId, TableId};
use crate::model::manifest::ProjectManifest;
use crate::model::table::{CellInput, TableSpec};

/// Expands a single table specification into an ordered vector of discrete cell calculation nodes.
///
/// Iterates over rows and columns in strict declaration order, applying the 3-Tier Specificity Lattice:
/// 1. 0D Point Cell Override (`row.cells.get(col_id)`)
/// 2. 1D Vector Rule (`row.expr` XOR `col.expr`)
/// 3. Explicit Null or Unassigned Error
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

    // Pre-parse row vector formulas once per row
    let mut parsed_row_exprs: IndexMap<&RowId, crate::expr::ast::Expr> = IndexMap::new();
    for row in &table.rows {
        if let Some(expr_str) = &row.expr {
            let expr = crate::expr::ast::Expr::parse(expr_str).map_err(|source| {
                IrError::FormulaParse {
                    table: table.id.clone(),
                    row: Some(row.id.clone()),
                    column: None,
                    source,
                }
            })?;
            parsed_row_exprs.insert(&row.id, expr);
        }
    }

    // Pre-parse column vector formulas once per column
    let mut parsed_col_exprs: IndexMap<&ColId, crate::expr::ast::Expr> = IndexMap::new();
    for col in &table.columns {
        if let Some(expr_str) = &col.expr {
            let expr = crate::expr::ast::Expr::parse(expr_str).map_err(|source| {
                IrError::FormulaParse {
                    table: table.id.clone(),
                    row: None,
                    column: Some(col.id.clone()),
                    source,
                }
            })?;
            parsed_col_exprs.insert(&col.id, expr);
        }
    }

    let mut cells = Vec::with_capacity(table.rows.len() * table.columns.len());

    for row in &table.rows {
        for col in &table.columns {
            let cell_id = CellId::new(table.id.clone(), row.id.clone(), col.id.clone());

            // Tier 1: Cell Override (0D Point)
            if let Some(cell_input) = row.cells.get(&col.id) {
                let (payload, cell_format, cell_note, cell_source) = match cell_input {
                    CellInput::Literal(lit) => {
                        (CellPayload::Literal(lit.clone()), None, None, None)
                    }
                    CellInput::Detailed(d) => {
                        let payload = if let Some(val) = &d.value {
                            CellPayload::Literal(val.clone())
                        } else if let Some(expr_str) = &d.expr {
                            let parsed =
                                crate::expr::ast::Expr::parse(expr_str).map_err(|source| {
                                    IrError::FormulaParse {
                                        table: table.id.clone(),
                                        row: Some(row.id.clone()),
                                        column: Some(col.id.clone()),
                                        source,
                                    }
                                })?;
                            CellPayload::Formula(parsed)
                        } else {
                            return Err(IrError::Model(format!(
                                "Cell '{cell_id}' specifies neither 'value' nor 'expr'"
                            )));
                        };
                        (payload, d.format, d.note.clone(), d.source.clone())
                    }
                };

                // Format resolution: Cell override format > Row format / Col format
                let effective_format = if cell_format.is_some() {
                    cell_format
                } else if col.role.is_descriptor() && row.role.is_descriptor() {
                    col.format.or(row.format)
                } else if col.role.is_descriptor() {
                    col.format
                } else if row.role.is_descriptor() {
                    row.format
                } else {
                    match (row.format, col.format) {
                        (Some(rf), Some(cf)) if rf != cf => {
                            return Err(IrError::FormatConflict {
                                table: table.id.clone(),
                                row: row.id.clone(),
                                column: col.id.clone(),
                                row_format: rf,
                                col_format: cf,
                            });
                        }
                        (Some(rf), _) => Some(rf),
                        (_, Some(cf)) => Some(cf),
                        (None, None) => None,
                    }
                };

                let effective_note = cell_note
                    .or_else(|| row.note.clone())
                    .or_else(|| col.note.clone());
                let effective_source = cell_source
                    .or_else(|| row.source.clone())
                    .or_else(|| col.source.clone());

                cells.push(
                    CellNode::new(cell_id, sheet_id.clone(), payload)
                        .with_format(effective_format)
                        .with_note(effective_note)
                        .with_source(effective_source),
                );
                continue;
            }

            // Tier 2: Descriptor boundary check (formulas never broadcast across descriptor axes)
            if row.role.is_descriptor() || col.role.is_descriptor() {
                return Err(IrError::UnassignedCell {
                    table: table.id.clone(),
                    row: row.id.clone(),
                    column: col.id.clone(),
                });
            }

            // Tier 3: Measure-on-measure vector rules
            let row_has_expr = parsed_row_exprs.contains_key(&row.id);
            let col_has_expr = parsed_col_exprs.contains_key(&col.id);

            if row_has_expr && col_has_expr {
                return Err(IrError::AmbiguousExpressionConflict {
                    table: table.id.clone(),
                    row: row.id.clone(),
                    column: col.id.clone(),
                });
            }

            let payload = if let Some(expr) = parsed_row_exprs.get(&row.id) {
                CellPayload::Formula(expr.clone())
            } else if let Some(expr) = parsed_col_exprs.get(&col.id) {
                CellPayload::Formula(expr.clone())
            } else {
                return Err(IrError::UnassignedCell {
                    table: table.id.clone(),
                    row: row.id.clone(),
                    column: col.id.clone(),
                });
            };

            let effective_format = match (row.format, col.format) {
                (Some(rf), Some(cf)) if rf != cf => {
                    return Err(IrError::FormatConflict {
                        table: table.id.clone(),
                        row: row.id.clone(),
                        column: col.id.clone(),
                        row_format: rf,
                        col_format: cf,
                    });
                }
                (Some(rf), _) => Some(rf),
                (_, Some(cf)) => Some(cf),
                (None, None) => None,
            };

            let effective_note = row.note.clone().or_else(|| col.note.clone());
            let effective_source = row.source.clone().or_else(|| col.source.clone());

            cells.push(
                CellNode::new(cell_id, sheet_id.clone(), payload)
                    .with_format(effective_format)
                    .with_note(effective_note)
                    .with_source(effective_source),
            );
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
    use crate::model::id::{RowId, SheetId, TableId};
    use crate::model::table::{AxisRole, ColumnSpec, DetailedCellInput, RowSpec, ValueLiteral};

    fn make_test_symbol_table(table: &TableSpec) -> SymbolTable {
        let mut st = SymbolTable::new();
        st.register_table(SheetId::from("Model"), table).unwrap();
        st
    }

    #[test]
    fn test_expand_table_ordering_and_payloads() {
        let table = TableSpec {
            id: TableId::new("Assumptions"),
            title: "Model Assumptions".to_string(),
            columns: vec![
                ColumnSpec::descriptor("param", "Parameter"),
                ColumnSpec::new("val"),
            ],
            column_groups: vec![],
            row_groups: vec![],
            rows: vec![
                RowSpec {
                    id: RowId::new("tax_rate"),
                    role: AxisRole::Measure,
                    format: Some(FormatPreset::Percentage),
                    expr: None,
                    note: Some("Federal tax rate".to_string()),
                    source: None,
                    cells: {
                        let mut map = IndexMap::new();
                        map.insert("param".into(), CellInput::string("Corporate Tax Rate"));
                        map.insert("val".into(), CellInput::number(0.25));
                        map
                    },
                },
                RowSpec {
                    id: RowId::new("exit_multiple"),
                    role: AxisRole::Measure,
                    format: Some(FormatPreset::Multiple),
                    expr: None,
                    note: None,
                    source: None,
                    cells: {
                        let mut map = IndexMap::new();
                        map.insert("param".into(), CellInput::string("Exit Multiple"));
                        map.insert("val".into(), CellInput::number(10.0));
                        map
                    },
                },
            ],
        };

        let symbols = make_test_symbol_table(&table);
        let cells = expand_table(&symbols, &table).unwrap();

        assert_eq!(cells.len(), 4);
        assert_eq!(cells[0].id, CellId::new("Assumptions", "tax_rate", "param"));
        assert_eq!(
            cells[0].payload,
            CellPayload::Literal(ValueLiteral::String("Corporate Tax Rate".to_string()))
        );
        assert_eq!(cells[0].format, None); // Descriptor column isolates measure format

        assert_eq!(cells[1].id, CellId::new("Assumptions", "tax_rate", "val"));
        assert_eq!(
            cells[1].payload,
            CellPayload::Literal(ValueLiteral::Number(0.25))
        );
        assert_eq!(cells[1].format, Some(FormatPreset::Percentage));
        assert_eq!(cells[1].note.as_deref(), Some("Federal tax rate"));

        assert_eq!(
            cells[2].id,
            CellId::new("Assumptions", "exit_multiple", "param")
        );
        assert_eq!(
            cells[3].id,
            CellId::new("Assumptions", "exit_multiple", "val")
        );
        assert_eq!(cells[3].format, Some(FormatPreset::Multiple));
    }

    #[test]
    fn test_expand_table_rejects_unregistered() {
        let table = TableSpec {
            id: TableId::new("Unregistered"),
            title: "Unregistered Table".to_string(),
            columns: vec![ColumnSpec::new("c1")],
            column_groups: vec![],
            row_groups: vec![],
            rows: vec![RowSpec {
                id: RowId::new("r1"),
                role: AxisRole::Measure,
                format: None,
                expr: Some("1".to_string()),
                note: None,
                source: None,
                cells: IndexMap::new(),
            }],
        };

        let symbols = SymbolTable::new();
        let err = expand_table(&symbols, &table).unwrap_err();
        assert!(
            matches!(err, IrError::UnknownTable { table, .. } if table.as_str() == "Unregistered")
        );
    }

    #[test]
    fn test_expand_table_syntax_error() {
        let table = TableSpec {
            id: TableId::new("BadExpr"),
            title: "Bad Expr Table".to_string(),
            columns: vec![ColumnSpec::new("c1")],
            column_groups: vec![],
            row_groups: vec![],
            rows: vec![RowSpec {
                id: RowId::new("r1"),
                role: AxisRole::Measure,
                format: None,
                expr: Some("1 + * 2".to_string()),
                note: None,
                source: None,
                cells: IndexMap::new(),
            }],
        };

        let symbols = make_test_symbol_table(&table);
        let err = expand_table(&symbols, &table).unwrap_err();
        assert!(matches!(err, IrError::FormulaParse { .. }));
    }

    #[test]
    fn test_expand_grid_inserts_into_grid() {
        let table = TableSpec {
            id: TableId::new("T1"),
            title: "Table 1".to_string(),
            columns: vec![ColumnSpec::new("c1")],
            column_groups: vec![],
            row_groups: vec![],
            rows: vec![RowSpec {
                id: RowId::new("r1"),
                role: AxisRole::Measure,
                format: None,
                expr: Some("10".to_string()),
                note: None,
                source: None,
                cells: IndexMap::new(),
            }],
        };

        let symbols = make_test_symbol_table(&table);
        let grid = expand_grid(&symbols, &[table]).unwrap();
        assert_eq!(grid.len(), 1);
        let cell = grid.get(&CellId::new("T1", "r1", "c1")).unwrap();
        assert!(cell.payload.is_formula());
    }

    #[test]
    fn test_ambiguous_expression_conflict_rejected() {
        let mut col = ColumnSpec::new("c1");
        col.expr = Some("100".to_string());

        let table = TableSpec {
            id: TableId::new("ConflictTable"),
            title: "Conflict Table".to_string(),
            columns: vec![col],
            column_groups: vec![],
            row_groups: vec![],
            rows: vec![RowSpec {
                id: RowId::new("r1"),
                role: AxisRole::Measure,
                format: None,
                expr: Some("200".to_string()),
                note: None,
                source: None,
                cells: IndexMap::new(),
            }],
        };

        let symbols = make_test_symbol_table(&table);
        let err = expand_table(&symbols, &table).unwrap_err();
        assert!(matches!(
            err,
            IrError::AmbiguousExpressionConflict { table, row, column }
                if table.as_str() == "ConflictTable" && row.as_str() == "r1" && column.as_str() == "c1"
        ));
    }

    #[test]
    fn test_cell_override_resolves_ambiguity() {
        let mut col = ColumnSpec::new("c1");
        col.expr = Some("100".to_string());

        let mut cells = IndexMap::new();
        cells.insert("c1".into(), CellInput::number(999.0));

        let table = TableSpec {
            id: TableId::new("ConflictResolved"),
            title: "Resolved Table".to_string(),
            columns: vec![col],
            column_groups: vec![],
            row_groups: vec![],
            rows: vec![RowSpec {
                id: RowId::new("r1"),
                role: AxisRole::Measure,
                format: None,
                expr: Some("200".to_string()),
                note: None,
                source: None,
                cells,
            }],
        };

        let symbols = make_test_symbol_table(&table);
        let expanded = expand_table(&symbols, &table).unwrap();
        assert_eq!(expanded.len(), 1);
        assert_eq!(
            expanded[0].payload,
            CellPayload::Literal(ValueLiteral::Number(999.0))
        );
    }

    #[test]
    fn test_unassigned_measure_cell_rejected() {
        let table = TableSpec {
            id: TableId::new("UnassignedTable"),
            title: "Unassigned Table".to_string(),
            columns: vec![ColumnSpec::new("c1")],
            column_groups: vec![],
            row_groups: vec![],
            rows: vec![RowSpec {
                id: RowId::new("r1"),
                role: AxisRole::Measure,
                format: None,
                expr: None,
                note: None,
                source: None,
                cells: IndexMap::new(),
            }],
        };

        let symbols = make_test_symbol_table(&table);
        let err = expand_table(&symbols, &table).unwrap_err();
        assert!(matches!(
            err,
            IrError::UnassignedCell { table, row, column }
                if table.as_str() == "UnassignedTable" && row.as_str() == "r1" && column.as_str() == "c1"
        ));
    }

    #[test]
    fn test_format_conflict_rejected() {
        let mut col = ColumnSpec::new("c1");
        col.format = Some(FormatPreset::Percentage);

        let table = TableSpec {
            id: TableId::new("FmtConflict"),
            title: "Format Conflict Table".to_string(),
            columns: vec![col],
            column_groups: vec![],
            row_groups: vec![],
            rows: vec![RowSpec {
                id: RowId::new("r1"),
                role: AxisRole::Measure,
                format: Some(FormatPreset::Currency),
                expr: Some("10".to_string()),
                note: None,
                source: None,
                cells: IndexMap::new(),
            }],
        };

        let symbols = make_test_symbol_table(&table);
        let err = expand_table(&symbols, &table).unwrap_err();
        assert!(matches!(
            err,
            IrError::FormatConflict {
                row_format: FormatPreset::Currency,
                col_format: FormatPreset::Percentage,
                ..
            }
        ));
    }

    #[test]
    fn test_cell_format_override_resolves_format_conflict() {
        let mut col = ColumnSpec::new("c1");
        col.format = Some(FormatPreset::Percentage);

        let mut cells = IndexMap::new();
        cells.insert(
            "c1".into(),
            CellInput::Detailed(DetailedCellInput {
                value: Some(ValueLiteral::Number(42.0)),
                format: Some(FormatPreset::Number),
                ..Default::default()
            }),
        );

        let table = TableSpec {
            id: TableId::new("FmtResolved"),
            title: "Format Resolved Table".to_string(),
            columns: vec![col],
            column_groups: vec![],
            row_groups: vec![],
            rows: vec![RowSpec {
                id: RowId::new("r1"),
                role: AxisRole::Measure,
                format: Some(FormatPreset::Currency),
                expr: Some("10".to_string()),
                note: None,
                source: None,
                cells,
            }],
        };

        let symbols = make_test_symbol_table(&table);
        let expanded = expand_table(&symbols, &table).unwrap();
        assert_eq!(expanded[0].format, Some(FormatPreset::Number));
    }
}
