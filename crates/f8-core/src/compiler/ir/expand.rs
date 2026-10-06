use indexmap::IndexMap;

use super::cell::{CellGrid, CellNode, CellPayload};
use super::error::IrError;
use super::id::CellId;
use super::symbol::SymbolTable;
use crate::model::format::{CurrencyCode, FormatPreset, SemanticStyle};
use crate::model::id::{ColId, RowId, SheetId, TableId};
use crate::model::manifest::ProjectManifest;
use crate::model::table::{CellInput, ColumnSpec, DataColumnSpec, DataRowSpec, RowSpec, TableSpec};

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
        let data_row = match row {
            RowSpec::Data(d) => d,
            RowSpec::SectionHeader(_) | RowSpec::Gutter => continue,
        };
        if let Some(expr_str) = &data_row.expr {
            let expr = crate::expr::ast::Expr::parse(expr_str).map_err(|source| {
                IrError::FormulaParse {
                    table: table.id.clone(),
                    row: Some(data_row.id.clone()),
                    column: None,
                    source,
                }
            })?;
            parsed_row_exprs.insert(&data_row.id, expr);
        }
    }

    // Pre-parse column vector formulas once per column
    let mut parsed_col_exprs: IndexMap<&ColId, crate::expr::ast::Expr> = IndexMap::new();
    for col in &table.columns {
        let data_col = match col {
            ColumnSpec::Data(d) => d,
            ColumnSpec::Gutter => continue,
        };
        if let Some(expr_str) = &data_col.expr {
            let expr = crate::expr::ast::Expr::parse(expr_str).map_err(|source| {
                IrError::FormulaParse {
                    table: table.id.clone(),
                    row: None,
                    column: Some(data_col.id.clone()),
                    source,
                }
            })?;
            parsed_col_exprs.insert(&data_col.id, expr);
        }
    }

    let mut cells = Vec::with_capacity(table.rows.len() * table.columns.len());

    for row in &table.rows {
        let row_data = match row {
            RowSpec::Data(d) => d,
            RowSpec::SectionHeader(_) | RowSpec::Gutter => continue,
        };

        for col in &table.columns {
            let col_data = match col {
                ColumnSpec::Data(d) => d,
                ColumnSpec::Gutter => continue,
            };

            let cell_id = CellId::new(table.id.clone(), row_data.id.clone(), col_data.id.clone());

            // Tier 1: Cell Override (0D Point)
            if let Some(cell_input) = row_data.cells.get(&col_data.id) {
                let cell_style = cell_input.style();
                let cell_highlight = cell_input.highlight();
                let cell_format = cell_input.format();
                let cell_decimals = cell_input.decimals();
                let cell_currency = cell_input.currency();
                let cell_indent = cell_input.indent().unwrap_or(0);
                let cell_note = cell_input.note();

                let payload = match cell_input {
                    CellInput::Literal(lit) => CellPayload::Literal(lit.clone()),
                    CellInput::Detailed(d) => {
                        if let Some(val) = &d.value {
                            CellPayload::Literal(val.clone())
                        } else if let Some(expr_str) = &d.expr {
                            let parsed =
                                crate::expr::ast::Expr::parse(expr_str).map_err(|source| {
                                    IrError::FormulaParse {
                                        table: table.id.clone(),
                                        row: Some(row_data.id.clone()),
                                        column: Some(col_data.id.clone()),
                                        source,
                                    }
                                })?;
                            CellPayload::Formula(parsed)
                        } else {
                            return Err(IrError::Model(format!(
                                "Cell '{cell_id}' specifies neither 'value' nor 'expr'"
                            )));
                        }
                    }
                };

                let effective_format = resolve_format(cell_format, row_data, col_data, &table.id)?;
                let effective_decimals = resolve_decimals(
                    cell_decimals,
                    effective_format,
                    row_data,
                    col_data,
                    &table.id,
                )?;
                let effective_currency = resolve_currency(
                    cell_currency,
                    effective_format,
                    row_data,
                    col_data,
                    table.currency,
                    &table.id,
                )?;
                let (horizontal, vertical) =
                    resolve_style(cell_style, row_data.style, col_data.style);
                let is_highlight =
                    resolve_highlight(cell_highlight, row_data.highlight, col_data.highlight);

                let effective_note = cell_note.map(String::from);

                cells.push(
                    CellNode::new(cell_id, sheet_id.clone(), payload)
                        .with_format(effective_format)
                        .with_decimals(effective_decimals)
                        .with_currency(effective_currency)
                        .with_style(horizontal, vertical)
                        .with_highlight(is_highlight)
                        .with_indent(cell_indent)
                        .with_note(effective_note),
                );
                continue;
            }

            // Tier 2: Descriptor boundary check (formulas never broadcast across descriptor axes)
            if col_data.role.is_descriptor() {
                return Err(IrError::UnassignedCell {
                    table: table.id.clone(),
                    row: row_data.id.clone(),
                    column: col_data.id.clone(),
                });
            }

            // Tier 3: Measure-on-measure vector rules
            let row_has_expr = parsed_row_exprs.contains_key(&row_data.id);
            let col_has_expr = parsed_col_exprs.contains_key(&col_data.id);

            if row_has_expr && col_has_expr {
                return Err(IrError::AmbiguousExpressionConflict {
                    table: table.id.clone(),
                    row: row_data.id.clone(),
                    column: col_data.id.clone(),
                });
            }

            let payload = if let Some(expr) = parsed_row_exprs.get(&row_data.id) {
                CellPayload::Formula(expr.clone())
            } else if let Some(expr) = parsed_col_exprs.get(&col_data.id) {
                CellPayload::Formula(expr.clone())
            } else {
                return Err(IrError::UnassignedCell {
                    table: table.id.clone(),
                    row: row_data.id.clone(),
                    column: col_data.id.clone(),
                });
            };

            let effective_format = resolve_format(None, row_data, col_data, &table.id)?;
            let effective_decimals =
                resolve_decimals(None, effective_format, row_data, col_data, &table.id)?;
            let effective_currency = resolve_currency(
                None,
                effective_format,
                row_data,
                col_data,
                table.currency,
                &table.id,
            )?;
            let (horizontal, vertical) = resolve_style(None, row_data.style, col_data.style);
            let is_highlight = resolve_highlight(None, row_data.highlight, col_data.highlight);

            cells.push(
                CellNode::new(cell_id, sheet_id.clone(), payload)
                    .with_format(effective_format)
                    .with_decimals(effective_decimals)
                    .with_currency(effective_currency)
                    .with_style(horizontal, vertical)
                    .with_highlight(is_highlight),
            );
        }
    }

    Ok(cells)
}

/// Resolves the effective format preset according to the 3-Tier Specificity Lattice.
fn resolve_format(
    cell_format: Option<FormatPreset>,
    row: &DataRowSpec,
    col: &DataColumnSpec,
    table_id: &TableId,
) -> Result<Option<FormatPreset>, IrError> {
    if cell_format.is_some() {
        Ok(cell_format)
    } else if col.role.is_descriptor() {
        Ok(col.format)
    } else {
        match (row.format, col.format) {
            (Some(rf), Some(cf)) if rf != cf => Err(IrError::FormatConflict {
                table: table_id.clone(),
                row: row.id.clone(),
                column: col.id.clone(),
                row_format: rf,
                col_format: cf,
            }),
            (Some(rf), _) => Ok(Some(rf)),
            (_, Some(cf)) => Ok(Some(cf)),
            (None, None) => Ok(None),
        }
    }
}

/// Resolves display precision (decimals) according to preset applicability and the 3-Tier Specificity Lattice.
fn resolve_decimals(
    cell_decimals: Option<u32>,
    effective_format: Option<FormatPreset>,
    row: &DataRowSpec,
    col: &DataColumnSpec,
    table_id: &TableId,
) -> Result<Option<u32>, IrError> {
    // Non-numeric presets have no precision concept and always resolve to None
    if let Some(format) = effective_format
        && !format.is_numeric()
    {
        return Ok(None);
    }

    if cell_decimals.is_some() {
        Ok(cell_decimals)
    } else if col.role.is_descriptor() {
        Ok(col.decimals)
    } else {
        match (row.decimals, col.decimals) {
            (Some(rd), Some(cd)) if rd != cd => Err(IrError::DecimalsConflict {
                table: table_id.clone(),
                row: row.id.clone(),
                column: col.id.clone(),
                row_decimals: rd,
                col_decimals: cd,
            }),
            (Some(rd), _) => Ok(Some(rd)),
            (_, Some(cd)) => Ok(Some(cd)),
            (None, None) => Ok(None),
        }
    }
}

/// Resolves currency denomination according to preset applicability and the 3-Tier Specificity Lattice.
fn resolve_currency(
    cell_currency: Option<CurrencyCode>,
    effective_format: Option<FormatPreset>,
    row: &DataRowSpec,
    col: &DataColumnSpec,
    table_currency: Option<CurrencyCode>,
    table_id: &TableId,
) -> Result<Option<CurrencyCode>, IrError> {
    // Non-currency presets receive no currency denomination
    if effective_format != Some(FormatPreset::Currency) {
        return Ok(None);
    }

    // Descriptor axis isolates currency and prevents vector conflicts
    if col.role.is_descriptor() {
        return Ok(None);
    }

    // Tier 1: Cell point override wins
    if cell_currency.is_some() {
        return Ok(cell_currency);
    }

    // Tier 2: Vector rules (row and column)
    match (row.currency, col.currency) {
        (Some(rc), Some(cc)) if rc != cc => Err(IrError::CurrencyConflict {
            table: table_id.clone(),
            row: row.id.clone(),
            column: col.id.clone(),
            row_currency: rc,
            col_currency: cc,
        }),
        (Some(rc), _) => Ok(Some(rc)),
        (_, Some(cc)) => Ok(Some(cc)),
        // Tier 3: Table default (defaults to USD)
        (None, None) => Ok(Some(table_currency.unwrap_or(CurrencyCode::Usd))),
    }
}

/// Resolves the orthogonal (horizontal, vertical) structural style pair.
fn resolve_style(
    cell_style: Option<SemanticStyle>,
    row_style: Option<SemanticStyle>,
    col_style: Option<SemanticStyle>,
) -> (SemanticStyle, SemanticStyle) {
    match cell_style {
        Some(SemanticStyle::Data) => (SemanticStyle::Data, SemanticStyle::Data),
        Some(s) => (s, col_style.unwrap_or(SemanticStyle::Data)),
        None => (
            row_style.unwrap_or(SemanticStyle::Data),
            col_style.unwrap_or(SemanticStyle::Data),
        ),
    }
}

/// Resolves visual highlight emphasis according to point override and vector rules.
fn resolve_highlight(
    cell_highlight: Option<bool>,
    row_highlight: Option<bool>,
    col_highlight: Option<bool>,
) -> bool {
    cell_highlight
        .unwrap_or_else(|| row_highlight.unwrap_or(false) || col_highlight.unwrap_or(false))
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

/// Orchestrates workbook symbol registration and grid expansion from a manifest and table specs.
pub fn expand_workbook(
    manifest: &ProjectManifest,
    tables: &[TableSpec],
) -> Result<(SymbolTable, CellGrid), IrError> {
    manifest.validate()?;

    let mut symbol_table = SymbolTable::new();

    for sheet in &manifest.sheets {
        let sheet_id = SheetId::from(sheet.name.as_str());

        for table_id in &sheet.tables {
            let matched_table =
                tables
                    .iter()
                    .find(|t| t.id == *table_id)
                    .ok_or_else(|| IrError::UnknownTable {
                        table: table_id.clone(),
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
    use crate::model::id::{SheetId, TableId};
    use crate::model::table::{ColumnSpec, DetailedCellInput, RowSpec, ValueLiteral};

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
            subtitle: None,
            currency: None,
            collapsible: false,
            columns: vec![
                ColumnSpec::descriptor("param", "Parameter"),
                ColumnSpec::new("val"),
            ],
            rows: vec![
                RowSpec::new("tax_rate")
                    .with_format(Some(FormatPreset::Percentage))
                    .with_cells({
                        let mut map = IndexMap::new();
                        map.insert("param".into(), CellInput::string("Corporate Tax Rate"));
                        map.insert(
                            "val".into(),
                            CellInput::Detailed(crate::model::table::DetailedCellInput {
                                value: Some(ValueLiteral::Number(0.25)),
                                note: Some("Federal tax rate".to_string()),
                                ..Default::default()
                            }),
                        );
                        map
                    }),
                RowSpec::new("exit_multiple")
                    .with_format(Some(FormatPreset::Multiple))
                    .with_cells({
                        let mut map = IndexMap::new();
                        map.insert("param".into(), CellInput::string("Exit Multiple"));
                        map.insert("val".into(), CellInput::number(10.0));
                        map
                    }),
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
        assert_eq!(cells[0].note, None);

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
            subtitle: None,
            currency: None,
            collapsible: false,
            columns: vec![ColumnSpec::new("c1")],
            rows: vec![RowSpec::new("r1").with_expr("1")],
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
            subtitle: None,
            currency: None,
            collapsible: false,
            columns: vec![ColumnSpec::new("c1")],
            rows: vec![RowSpec::new("r1").with_expr("1 + * 2")],
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
            subtitle: None,
            currency: None,
            collapsible: false,
            columns: vec![ColumnSpec::new("c1")],
            rows: vec![RowSpec::new("r1").with_expr("10")],
        };

        let symbols = make_test_symbol_table(&table);
        let grid = expand_grid(&symbols, &[table]).unwrap();
        assert_eq!(grid.len(), 1);
        let cell = grid.get(&CellId::new("T1", "r1", "c1")).unwrap();
        assert!(cell.payload.is_formula());
    }

    #[test]
    fn test_ambiguous_expression_conflict_rejected() {
        let col = ColumnSpec::new("c1").with_expr("100");

        let table = TableSpec {
            id: TableId::new("ConflictTable"),
            title: "Conflict Table".to_string(),
            subtitle: None,
            currency: None,
            collapsible: false,
            columns: vec![col],
            rows: vec![RowSpec::new("r1").with_expr("200")],
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
        let col = ColumnSpec::new("c1").with_expr("100");

        let mut cells = IndexMap::new();
        cells.insert("c1".into(), CellInput::number(999.0));

        let table = TableSpec {
            id: TableId::new("ConflictResolved"),
            title: "Resolved Table".to_string(),
            subtitle: None,
            currency: None,
            collapsible: false,
            columns: vec![col],
            rows: vec![RowSpec::new("r1").with_expr("200").with_cells(cells)],
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
            subtitle: None,
            currency: None,
            collapsible: false,
            columns: vec![ColumnSpec::new("c1")],
            rows: vec![RowSpec::new("r1")],
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
        let col = ColumnSpec::new("c1").with_format(Some(FormatPreset::Percentage));

        let table = TableSpec {
            id: TableId::new("FmtConflict"),
            title: "Format Conflict Table".to_string(),
            subtitle: None,
            currency: None,
            collapsible: false,
            columns: vec![col],
            rows: vec![
                RowSpec::new("r1")
                    .with_format(Some(FormatPreset::Currency))
                    .with_expr("10"),
            ],
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
        let col = ColumnSpec::new("c1").with_format(Some(FormatPreset::Percentage));

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
            subtitle: None,
            currency: None,
            collapsible: false,
            columns: vec![col],
            rows: vec![
                RowSpec::new("r1")
                    .with_format(Some(FormatPreset::Currency))
                    .with_expr("10")
                    .with_cells(cells),
            ],
        };

        let symbols = make_test_symbol_table(&table);
        let expanded = expand_table(&symbols, &table).unwrap();
        assert_eq!(expanded[0].format, Some(FormatPreset::Number));
    }
}
