#![allow(clippy::unwrap_used, clippy::expect_used)]

use f8_core::compiler::ir::{
    CellId, CellPayload, IrError, SymbolTable, expand_grid, expand_table, expand_workbook,
};
use f8_core::expr::ast::Expr;
use f8_core::model::format::{CurrencyCode, FormatPreset, SemanticStyle};
use f8_core::model::id::{ColId, RowId, SheetId, TableId};
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{
    CellInput, ColumnSpec, DetailedCellInput, RowSpec, TableSpec, ValueLiteral,
};
use indexmap::IndexMap;

fn create_simple_table(table_id: &str, cols: &[&str], rows: Vec<RowSpec>) -> TableSpec {
    TableSpec {
        id: TableId::from(table_id),
        title: format!("Title for {table_id}"),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: cols.iter().map(|&c| ColumnSpec::new(c)).collect(),
        rows,
    }
}

#[test]
fn test_scenario_1_expand_literal_row_across_columns() {
    let mut cells = IndexMap::new();
    cells.insert("2021A".into(), CellInput::number(100.0));
    cells.insert("2022A".into(), CellInput::number(110.0));
    cells.insert("2023A".into(), CellInput::number(120.0));

    let row = RowSpec::new("revenue").with_cells(cells);

    let table = create_simple_table("IncomeStatement", &["2021A", "2022A", "2023A"], vec![row]);

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .expect("registration succeeds");

    let cells = expand_table(&symbols, &table).expect("expansion succeeds");
    assert_eq!(cells.len(), 3);

    assert_eq!(
        cells[0].id,
        CellId::new("IncomeStatement", "revenue", "2021A")
    );
    assert_eq!(
        cells[0].payload,
        CellPayload::Literal(ValueLiteral::Number(100.0))
    );
    assert_eq!(cells[0].sheet, SheetId::from("Model"));

    assert_eq!(
        cells[1].id,
        CellId::new("IncomeStatement", "revenue", "2022A")
    );
    assert_eq!(
        cells[1].payload,
        CellPayload::Literal(ValueLiteral::Number(110.0))
    );

    assert_eq!(
        cells[2].id,
        CellId::new("IncomeStatement", "revenue", "2023A")
    );
    assert_eq!(
        cells[2].payload,
        CellPayload::Literal(ValueLiteral::Number(120.0))
    );
}

#[test]
fn test_scenario_2_expand_uniform_formula_row_across_columns() {
    let row = RowSpec::new("ebit")
        .with_format(Some(FormatPreset::Currency))
        .with_expr("IncomeStatement[\"revenue\", @col] * 0.1");

    let table = create_simple_table("IncomeStatement", &["2024E", "2025E"], vec![row]);

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .expect("registration succeeds");

    let cells = expand_table(&symbols, &table).expect("expansion succeeds");
    assert_eq!(cells.len(), 2);

    let expected_ast =
        Expr::parse("IncomeStatement[\"revenue\", @col] * 0.1").expect("valid formula");

    assert_eq!(cells[0].id, CellId::new("IncomeStatement", "ebit", "2024E"));
    assert_eq!(cells[0].payload, CellPayload::Formula(expected_ast.clone()));
    assert_eq!(cells[0].format, Some(FormatPreset::Currency));
    assert_eq!(cells[0].note, None);

    assert_eq!(cells[1].id, CellId::new("IncomeStatement", "ebit", "2025E"));
    assert_eq!(cells[1].payload, CellPayload::Formula(expected_ast));
    assert_eq!(cells[1].format, Some(FormatPreset::Currency));
    assert_eq!(cells[1].note, None);
}

#[test]
fn test_scenario_3_expand_vector_row_with_point_overrides() {
    let mut cells = IndexMap::new();
    cells.insert(
        "2023A".into(),
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::Number(100.0)),
            note: Some("Actual 10-K".to_string()),
            ..Default::default()
        }),
    );

    let row = RowSpec::new("revenue")
        .with_format(Some(FormatPreset::Currency))
        .with_expr("IncomeStatement[\"revenue\", col.prev] * 1.05")
        .with_cells(cells);

    let table = create_simple_table("IncomeStatement", &["2023A", "2024E", "2025E"], vec![row]);

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .expect("registration succeeds");

    let cells = expand_table(&symbols, &table).expect("expansion succeeds");
    assert_eq!(cells.len(), 3);

    // 2023A: Cell Override Literal
    assert_eq!(
        cells[0].id,
        CellId::new("IncomeStatement", "revenue", "2023A")
    );
    assert_eq!(
        cells[0].payload,
        CellPayload::Literal(ValueLiteral::Number(100.0))
    );
    assert_eq!(cells[0].note.as_deref(), Some("Actual 10-K"));

    // 2024E: Vector Formula
    assert_eq!(
        cells[1].id,
        CellId::new("IncomeStatement", "revenue", "2024E")
    );
    assert!(cells[1].payload.is_formula());
    assert_eq!(cells[1].note, None);

    // 2025E: Vector Formula
    assert_eq!(
        cells[2].id,
        CellId::new("IncomeStatement", "revenue", "2025E")
    );
    assert!(cells[2].payload.is_formula());
    assert_eq!(cells[2].note, None);
}

#[test]
fn test_scenario_4_metadata_and_format_preset_propagation() {
    let mut cells = IndexMap::new();
    cells.insert("2023A".into(), CellInput::number(100.0));
    cells.insert("2024E".into(), CellInput::number(105.0));
    cells.insert(
        "2025E".into(),
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::Number(110.0)),
            note: Some("Forecast update".to_string()),
            currency: None,
            source: Some(serde_json::json!({ "source_id": "Management Guidance" })),
            ..Default::default()
        }),
    );

    let row = RowSpec::new("revenue")
        .with_format(Some(FormatPreset::Currency))
        .with_source(Some(serde_json::json!({ "source_id": "10-K" })))
        .with_cells(cells);

    let table = create_simple_table("IncomeStatement", &["2023A", "2024E", "2025E"], vec![row]);

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .expect("registration succeeds");

    let cells = expand_table(&symbols, &table).expect("expansion succeeds");
    assert_eq!(cells.len(), 3);

    // 2023A: Inherits format, source, zero note
    assert_eq!(cells[0].format, Some(FormatPreset::Currency));
    assert_eq!(cells[0].note, None);
    assert_eq!(
        cells[0].source,
        Some(serde_json::json!({ "source_id": "10-K" }))
    );

    // 2024E: Inherits format, source, zero note
    assert_eq!(cells[1].format, Some(FormatPreset::Currency));
    assert_eq!(cells[1].note, None);
    assert_eq!(
        cells[1].source,
        Some(serde_json::json!({ "source_id": "10-K" }))
    );

    // 2025E: Inherits format, explicit cell note, overrides source
    assert_eq!(cells[2].format, Some(FormatPreset::Currency));
    assert_eq!(cells[2].note.as_deref(), Some("Forecast update"));
    assert_eq!(
        cells[2].source,
        Some(serde_json::json!({ "source_id": "Management Guidance" }))
    );
}

#[test]
fn test_scenario_5_actionable_formula_parse_error_diagnostic() {
    let row = RowSpec::new("bad_calc").with_expr("IncomeStatement[\"revenue\", @col] + * 5");

    let table = create_simple_table("IncomeStatement", &["2024E"], vec![row]);

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .expect("registration succeeds");

    let err = expand_table(&symbols, &table).unwrap_err();
    match err {
        IrError::FormulaParse { table, row, .. } => {
            assert_eq!(table, TableId::from("IncomeStatement"));
            assert_eq!(row, Some(RowId::from("bad_calc")));
        }
        other => panic!("expected IrError::FormulaParse, got {other:?}"),
    }
}

#[test]
fn test_scenario_6_end_to_end_workbook_orchestration() {
    let manifest = ProjectManifest {
        model_name: "LBO_Model".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![
            SheetSpec {
                name: "Summary".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("Assumptions")],
            },
            SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("IncomeStatement")],
            },
        ],
    };

    let mut assumptions_cells = IndexMap::new();
    assumptions_cells.insert("value".into(), CellInput::number(0.21));
    let assumptions_table = create_simple_table(
        "Assumptions",
        &["value"],
        vec![
            RowSpec::new("tax_rate")
                .with_format(Some(FormatPreset::Percentage))
                .with_cells(assumptions_cells),
        ],
    );

    let mut is_cells = IndexMap::new();
    is_cells.insert("2023A".into(), CellInput::number(1000.0));
    is_cells.insert("2024E".into(), CellInput::number(1100.0));
    let is_table = create_simple_table(
        "IncomeStatement",
        &["2023A", "2024E"],
        vec![
            RowSpec::new("revenue")
                .with_format(Some(FormatPreset::Currency))
                .with_cells(is_cells),
        ],
    );

    let (symbols, grid) = expand_workbook(&manifest, &[assumptions_table, is_table])
        .expect("workbook expansion succeeds");

    assert_eq!(symbols.sheet_ids().count(), 2);
    assert_eq!(symbols.tables_on_sheet(&SheetId::from("Summary")).len(), 1);
    assert_eq!(symbols.tables_on_sheet(&SheetId::from("Model")).len(), 1);

    assert_eq!(grid.len(), 3);
    assert!(grid.contains(&CellId::new("Assumptions", "tax_rate", "value")));
    assert!(grid.contains(&CellId::new("IncomeStatement", "revenue", "2023A")));
    assert!(grid.contains(&CellId::new("IncomeStatement", "revenue", "2024E")));

    // Verify sheet assignment on CellNodes
    let tax_node = grid
        .get(&CellId::new("Assumptions", "tax_rate", "value"))
        .unwrap();
    assert_eq!(tax_node.sheet, SheetId::from("Summary"));

    let is_node = grid
        .get(&CellId::new("IncomeStatement", "revenue", "2023A"))
        .unwrap();
    assert_eq!(is_node.sheet, SheetId::from("Model"));
}

#[test]
fn test_scenario_7_reject_table_not_in_symbol_table() {
    let mut cells = IndexMap::new();
    cells.insert("2024E".into(), CellInput::number(100.0));
    let table = create_simple_table(
        "UnknownTable",
        &["2024E"],
        vec![RowSpec::new("revenue").with_cells(cells)],
    );
    let symbols = SymbolTable::new();

    let err = expand_table(&symbols, &table).unwrap_err();
    match err {
        IrError::UnknownTable { table, context } => {
            assert_eq!(table, TableId::from("UnknownTable"));
            assert_eq!(context, "grid expansion");
        }
        other => panic!("expected IrError::UnknownTable, got {other:?}"),
    }
}

#[test]
fn test_expand_multi_tier_column_with_vector_rules() {
    let mut table = TableSpec {
        id: TableId::from("BalanceSheet"),
        title: "Balance Sheet".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![
            ColumnSpec::new("2021A").with_headers(["Historical", "2021A"]),
            ColumnSpec::new("2022A").with_headers(["Historical", "2022A"]),
            ColumnSpec::new("2023A").with_headers(["Historical", "2023A"]),
            ColumnSpec::new("2024E").with_headers(["Forecast", "2024E"]),
            ColumnSpec::new("2025E").with_headers(["Forecast", "2025E"]),
        ],
        rows: vec![],
    };

    let mut cells = IndexMap::new();
    cells.insert("2021A".into(), CellInput::number(50.0));
    cells.insert("2022A".into(), CellInput::number(60.0));
    cells.insert("2023A".into(), CellInput::number(70.0));

    table.rows = vec![
        RowSpec::new("cash")
            .with_format(Some(FormatPreset::Currency))
            .with_expr("BalanceSheet[\"cash\", col.prev] + 10.0")
            .with_cells(cells),
    ];

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .expect("registration succeeds");

    let cells = expand_table(&symbols, &table).expect("expansion succeeds");
    assert_eq!(cells.len(), 5);

    // Columns are in exact order: 2021A, 2022A, 2023A, 2024E, 2025E
    assert_eq!(cells[0].id.col, ColId::from("2021A"));
    assert_eq!(
        cells[0].payload,
        CellPayload::Literal(ValueLiteral::Number(50.0))
    );
    assert_eq!(cells[1].id.col, ColId::from("2022A"));
    assert_eq!(cells[2].id.col, ColId::from("2023A"));
    assert_eq!(cells[3].id.col, ColId::from("2024E"));
    assert!(cells[3].payload.is_formula());
    assert_eq!(cells[4].id.col, ColId::from("2025E"));
    assert!(cells[4].payload.is_formula());
}

#[test]
fn test_expand_workbook_unassigned_table_rejected() {
    let manifest = ProjectManifest {
        model_name: "Test".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Summary".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("Assumptions")],
        }],
    };

    let mut t1_cells = IndexMap::new();
    t1_cells.insert("value".into(), CellInput::number(0.21));
    let table1 = create_simple_table(
        "Assumptions",
        &["value"],
        vec![RowSpec::new("tax_rate").with_cells(t1_cells)],
    );

    let mut t2_cells = IndexMap::new();
    t2_cells.insert("value".into(), CellInput::number(1.0));
    let table2 = create_simple_table(
        "OrphanTable",
        &["value"],
        vec![RowSpec::new("foo").with_cells(t2_cells)],
    );

    let err = expand_workbook(&manifest, &[table1, table2]).unwrap_err();
    match err {
        IrError::UnknownTable { table, context } => {
            assert_eq!(table, TableId::from("OrphanTable"));
            assert!(context.contains("unassigned table"));
        }
        other => panic!("expected IrError::UnknownTable, got {other:?}"),
    }
}

#[test]
fn test_expand_grid_multi_table() {
    let mut t1_cells = IndexMap::new();
    t1_cells.insert("c1".into(), CellInput::number(1.0));
    t1_cells.insert("c2".into(), CellInput::number(2.0));
    let t1 = create_simple_table(
        "TableA",
        &["c1", "c2"],
        vec![RowSpec::new("r1").with_cells(t1_cells)],
    );

    let mut t2_cells = IndexMap::new();
    t2_cells.insert("c1".into(), CellInput::number(3.0));
    let t2 = create_simple_table(
        "TableB",
        &["c1"],
        vec![RowSpec::new("r2").with_cells(t2_cells)],
    );

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Sheet1"), &t1)
        .unwrap();
    symbols
        .register_table(SheetId::from("Sheet1"), &t2)
        .unwrap();

    let grid = expand_grid(&symbols, &[t1, t2]).unwrap();
    assert_eq!(grid.len(), 3);
    assert_eq!(
        grid.get(&CellId::new("TableA", "r1", "c1"))
            .unwrap()
            .payload,
        CellPayload::Literal(ValueLiteral::Number(1.0))
    );
    assert_eq!(
        grid.get(&CellId::new("TableA", "r1", "c2"))
            .unwrap()
            .payload,
        CellPayload::Literal(ValueLiteral::Number(2.0))
    );
    assert_eq!(
        grid.get(&CellId::new("TableB", "r2", "c1"))
            .unwrap()
            .payload,
        CellPayload::Literal(ValueLiteral::Number(3.0))
    );
}

#[test]
fn test_expand_column_vector_formula_broadcasts_downward() {
    let actual_col = ColumnSpec::new("actual").with_format(Some(FormatPreset::Currency));
    let forecast_col = ColumnSpec::new("forecast")
        .with_format(Some(FormatPreset::Currency))
        .with_expr("IncomeStatement[\"revenue\", \"actual\"] * 1.1");

    let mut rev_cells = IndexMap::new();
    rev_cells.insert("actual".into(), CellInput::number(100.0));

    let mut cogs_cells = IndexMap::new();
    cogs_cells.insert("actual".into(), CellInput::number(50.0));

    let mut opex_cells = IndexMap::new();
    opex_cells.insert("actual".into(), CellInput::number(30.0));
    // 0D Point override on forecast
    opex_cells.insert("forecast".into(), CellInput::number(35.0));

    let table = TableSpec {
        id: TableId::from("IncomeStatement"),
        title: "Income Statement".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![actual_col, forecast_col],
        rows: vec![
            RowSpec::new("revenue").with_cells(rev_cells),
            RowSpec::new("cogs").with_cells(cogs_cells),
            RowSpec::new("opex").with_cells(opex_cells),
        ],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 6);

    // Revenue: actual is literal, forecast is formula from column rule
    assert_eq!(
        cells[0].id,
        CellId::new("IncomeStatement", "revenue", "actual")
    );
    assert_eq!(
        cells[0].payload,
        CellPayload::Literal(ValueLiteral::Number(100.0))
    );
    assert_eq!(
        cells[1].id,
        CellId::new("IncomeStatement", "revenue", "forecast")
    );
    assert!(cells[1].payload.is_formula());

    // COGS: actual is literal, forecast is formula from column rule
    assert_eq!(
        cells[2].id,
        CellId::new("IncomeStatement", "cogs", "actual")
    );
    assert_eq!(
        cells[3].id,
        CellId::new("IncomeStatement", "cogs", "forecast")
    );
    assert!(cells[3].payload.is_formula());

    // Opex: actual is literal, forecast is point literal override
    assert_eq!(
        cells[4].id,
        CellId::new("IncomeStatement", "opex", "actual")
    );
    assert_eq!(
        cells[4].payload,
        CellPayload::Literal(ValueLiteral::Number(30.0))
    );
    assert_eq!(
        cells[5].id,
        CellId::new("IncomeStatement", "opex", "forecast")
    );
    assert_eq!(
        cells[5].payload,
        CellPayload::Literal(ValueLiteral::Number(35.0))
    );
}

#[test]
fn test_descriptor_column_isolates_formulas_and_enforces_assignment() {
    let desc_col = ColumnSpec::descriptor("desc", "Description");
    let col1 = ColumnSpec::new("c1");

    // Unassigned descriptor column halts with UnassignedCell even if row has expr
    let unassigned_table = TableSpec {
        id: TableId::from("T_Unassigned"),
        title: "Test".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![desc_col.clone(), col1.clone()],
        rows: vec![RowSpec::new("r1").with_expr("100")],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &unassigned_table)
        .unwrap();

    let err = expand_table(&symbols, &unassigned_table).unwrap_err();
    assert!(matches!(
        err,
        IrError::UnassignedCell { table, row, column }
            if table.as_str() == "T_Unassigned" && row.as_str() == "r1" && column.as_str() == "desc"
    ));

    // Assigned descriptor column emits literal and isolates row formula
    let mut r1_cells = IndexMap::new();
    r1_cells.insert("desc".into(), CellInput::string("Revenue Label"));

    let assigned_table = TableSpec {
        id: TableId::from("T_Assigned"),
        title: "Test".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![desc_col, col1],
        rows: vec![RowSpec::new("r1").with_expr("100").with_cells(r1_cells)],
    };

    let mut symbols2 = SymbolTable::new();
    symbols2
        .register_table(SheetId::from("Model"), &assigned_table)
        .unwrap();

    let cells = expand_table(&symbols2, &assigned_table).unwrap();
    assert_eq!(cells.len(), 2);

    // Descriptor cell is string literal
    assert_eq!(
        cells[0].payload,
        CellPayload::Literal(ValueLiteral::String("Revenue Label".to_string()))
    );

    // Measure cell receives row formula broadcast
    assert!(cells[1].payload.is_formula());
}

#[test]
fn test_column_formula_parse_error_diagnostic() {
    let bad_col = ColumnSpec::new("bad_col").with_expr("1 + * 2");

    let table = TableSpec {
        id: TableId::from("BadColTable"),
        title: "Bad Col".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![bad_col],
        rows: vec![RowSpec::new("r1")],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let err = expand_table(&symbols, &table).unwrap_err();
    match &err {
        IrError::FormulaParse {
            table, row, column, ..
        } => {
            assert_eq!(table.as_str(), "BadColTable");
            assert_eq!(*row, None);
            assert_eq!(column.as_ref().map(|c| c.as_str()), Some("bad_col"));
        }
        other => panic!("expected FormulaParse error, got {other:?}"),
    }

    let err_msg = err.to_string();
    assert!(err_msg.contains("col 'bad_col'"));
    assert!(!err_msg.contains("row ''"));
}

#[test]
fn test_descriptor_column_format_isolation() {
    let desc_col = ColumnSpec::descriptor("d_col", "Desc Col");
    let mut row_cells = IndexMap::new();
    row_cells.insert("d_col".into(), CellInput::string("Label"));

    let row = RowSpec::new("r1")
        .with_format(Some(FormatPreset::Currency))
        .with_decimals(Some(2))
        .with_cells(row_cells);

    let table = TableSpec {
        id: TableId::from("DualDesc"),
        title: "Descriptor Isolation Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![desc_col],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 1);
    // Descriptor column must NOT inherit row's Currency format or decimals
    assert_eq!(cells[0].format, None);
    assert_eq!(cells[0].decimals, None);
}

#[test]
fn test_scenario_1_symmetrical_2d_border_composition_at_matrix_intersection() {
    let col = ColumnSpec::new("fy").with_style(Some(SemanticStyle::Total));
    let row = RowSpec::new("net_income")
        .with_style(Some(SemanticStyle::Total))
        .with_expr("100");

    let table = TableSpec {
        id: TableId::from("IncomeStatement"),
        title: "Income Statement".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![col],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 1);
    let cell = &cells[0];
    assert_eq!(cell.horizontal, SemanticStyle::Total);
    assert_eq!(cell.vertical, SemanticStyle::Total);
    assert!(cell.is_bold());
}

#[test]
fn test_scenario_2_cell_reset_suppresses_inherited_summary_borders() {
    let col = ColumnSpec::descriptor("line_item", "Line Item");
    let mut row_cells = IndexMap::new();
    row_cells.insert(
        ColId::from("line_item"),
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::String("Gross Profit".to_string())),
            expr: None,
            style: Some(SemanticStyle::Data),
            highlight: None,
            format: None,
            decimals: None,
            note: None,
            source: None,
            currency: None,
            indent: None,
        }),
    );

    let row = RowSpec::new("gross_profit")
        .with_style(Some(SemanticStyle::Subtotal))
        .with_cells(row_cells);

    let table = TableSpec {
        id: TableId::from("IncomeStatement"),
        title: "Income Statement".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![col],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 1);
    let cell = &cells[0];
    // Explicit cell reset to Data suppresses both horizontal and vertical summary rules
    assert_eq!(cell.horizontal, SemanticStyle::Data);
    assert_eq!(cell.vertical, SemanticStyle::Data);
    assert!(!cell.is_bold());
}

#[test]
fn test_scenario_3_cell_precision_override_wins_over_vector_decimals() {
    let col = ColumnSpec::new("q1").with_decimals(Some(2));
    let mut row_cells = IndexMap::new();
    row_cells.insert(
        ColId::from("q1"),
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::Number(1.2345)),
            expr: None,
            style: None,
            highlight: None,
            format: None,
            decimals: Some(4),
            note: None,
            source: None,
            currency: None,
            indent: None,
        }),
    );

    let row = RowSpec::new("eps")
        .with_decimals(Some(2))
        .with_cells(row_cells);

    let table = TableSpec {
        id: TableId::from("EPS"),
        title: "EPS Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![col],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].decimals, Some(4));
}

#[test]
fn test_scenario_4_measure_vector_precision_conflict_rejected() {
    let col = ColumnSpec::new("c1").with_decimals(Some(0));
    let row = RowSpec::new("r1").with_decimals(Some(2)).with_expr("100");

    let table = TableSpec {
        id: TableId::from("ConflictTable"),
        title: "Conflict Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![col],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let err = expand_table(&symbols, &table).unwrap_err();
    match err {
        IrError::DecimalsConflict {
            table,
            row,
            column,
            row_decimals,
            col_decimals,
        } => {
            assert_eq!(table.as_str(), "ConflictTable");
            assert_eq!(row.as_str(), "r1");
            assert_eq!(column.as_str(), "c1");
            assert_eq!(row_decimals, 2);
            assert_eq!(col_decimals, 0);
        }
        other => panic!("expected DecimalsConflict, got {other:?}"),
    }
}

#[test]
fn test_scenario_6_point_highlight_override_suppresses_vector_highlight() {
    let col = ColumnSpec::new("c1");
    let mut row_cells = IndexMap::new();
    row_cells.insert(
        ColId::from("c1"),
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::Number(50.0)),
            expr: None,
            style: None,
            highlight: Some(false),
            format: None,
            decimals: None,
            note: None,
            source: None,
            currency: None,
            indent: None,
        }),
    );

    let row = RowSpec::new("r1")
        .with_highlight(Some(true))
        .with_cells(row_cells);

    let table = TableSpec {
        id: TableId::from("HighlightTable"),
        title: "Highlight Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![col],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 1);
    assert!(!cells[0].is_highlight);
}

#[test]
fn test_scenario_7_highlight_inheritance_from_row_or_column() {
    let col_highlight = ColumnSpec::new("c_hi").with_highlight(Some(true));
    let col_normal = ColumnSpec::new("c_norm");

    let row_hi = RowSpec::new("r_hi")
        .with_highlight(Some(true))
        .with_expr("1");
    let row_norm = RowSpec::new("r_norm").with_expr("2");

    let table = TableSpec {
        id: TableId::from("InheritanceTable"),
        title: "Inheritance Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![col_highlight, col_normal],
        rows: vec![row_hi, row_norm],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 4);

    // ("r_hi", "c_hi") -> row: true, col: true -> true
    assert!(cells[0].is_highlight);
    // ("r_hi", "c_norm") -> row: true, col: false -> true
    assert!(cells[1].is_highlight);
    // ("r_norm", "c_hi") -> row: false, col: true -> true
    assert!(cells[2].is_highlight);
    // ("r_norm", "c_norm") -> row: false, col: false -> false
    assert!(!cells[3].is_highlight);
}

#[test]
fn test_scenario_1_point_currency_override_wins_over_vectors_and_table() {
    let col = ColumnSpec::new("c1")
        .with_format(Some(FormatPreset::Currency))
        .with_currency(Some(CurrencyCode::Gbp));

    let mut row_cells = IndexMap::new();
    row_cells.insert(
        ColId::from("c1"),
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::Number(100.0)),
            currency: Some(CurrencyCode::Chf),
            ..Default::default()
        }),
    );

    let row = RowSpec::new("r1")
        .with_format(Some(FormatPreset::Currency))
        .with_currency(Some(CurrencyCode::Jpy))
        .with_cells(row_cells);

    let table = TableSpec {
        id: TableId::from("Tier1Table"),
        title: "Tier 1 Table".to_string(),
        subtitle: None,
        currency: Some(CurrencyCode::Eur),
        collapsible: false,
        columns: vec![col],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].currency, Some(CurrencyCode::Chf));
}

#[test]
fn test_scenario_2_vector_currency_wins_over_table_default() {
    let col = ColumnSpec::new("c1")
        .with_format(Some(FormatPreset::Currency))
        .with_currency(Some(CurrencyCode::Gbp));

    let row = RowSpec::new("r1")
        .with_format(Some(FormatPreset::Currency))
        .with_expr("100");

    let table = TableSpec {
        id: TableId::from("Tier2Table"),
        title: "Tier 2 Table".to_string(),
        subtitle: None,
        currency: Some(CurrencyCode::Eur),
        collapsible: false,
        columns: vec![col],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].currency, Some(CurrencyCode::Gbp));
}

#[test]
fn test_scenario_3_measure_vector_currency_conflict_rejected() {
    let col = ColumnSpec::new("c1")
        .with_format(Some(FormatPreset::Currency))
        .with_currency(Some(CurrencyCode::Usd));

    let row = RowSpec::new("r1")
        .with_format(Some(FormatPreset::Currency))
        .with_currency(Some(CurrencyCode::Eur))
        .with_expr("100");

    let table = TableSpec {
        id: TableId::from("ConflictTable"),
        title: "Conflict Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![col],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let err = expand_table(&symbols, &table).unwrap_err();
    assert!(matches!(
        err,
        IrError::CurrencyConflict {
            row_currency: CurrencyCode::Eur,
            col_currency: CurrencyCode::Usd,
            ..
        }
    ));
}

#[test]
fn test_scenario_4_harmonious_measure_vector_currencies_accepted() {
    let col = ColumnSpec::new("c1")
        .with_format(Some(FormatPreset::Currency))
        .with_currency(Some(CurrencyCode::Eur));

    let row = RowSpec::new("r1")
        .with_format(Some(FormatPreset::Currency))
        .with_currency(Some(CurrencyCode::Eur))
        .with_expr("100");

    let table = TableSpec {
        id: TableId::from("HarmoniousTable"),
        title: "Harmonious Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![col],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].currency, Some(CurrencyCode::Eur));
}

#[test]
fn test_scenario_5_table_default_applies_when_vectors_omit_currency() {
    let col = ColumnSpec::new("c1").with_format(Some(FormatPreset::Currency));

    let row = RowSpec::new("r1")
        .with_format(Some(FormatPreset::Currency))
        .with_expr("100");

    let table = TableSpec {
        id: TableId::from("TableDefault"),
        title: "Table Default".to_string(),
        subtitle: None,
        currency: Some(CurrencyCode::Eur),
        collapsible: false,
        columns: vec![col],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].currency, Some(CurrencyCode::Eur));
}

#[test]
fn test_scenario_6_fallback_to_usd_when_omitted_across_all_tiers() {
    let col = ColumnSpec::new("c1").with_format(Some(FormatPreset::Currency));

    let row = RowSpec::new("r1")
        .with_format(Some(FormatPreset::Currency))
        .with_expr("100");

    let table = TableSpec {
        id: TableId::from("UsdFallback"),
        title: "USD Fallback Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![col],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].currency, Some(CurrencyCode::Usd));
}

#[test]
fn test_scenario_7_non_currency_format_presets_resolve_none() {
    let col_num = ColumnSpec::new("c_num").with_format(Some(FormatPreset::Number));
    let col_pct = ColumnSpec::new("c_pct").with_format(Some(FormatPreset::Percentage));
    let col_date = ColumnSpec::new("c_date").with_format(Some(FormatPreset::Date));
    let col_txt = ColumnSpec::new("c_txt").with_format(Some(FormatPreset::Text));

    let row = RowSpec::new("r1").with_expr("1");

    let table = TableSpec {
        id: TableId::from("NonCurrency"),
        title: "Non Currency Table".to_string(),
        subtitle: None,
        currency: Some(CurrencyCode::Eur),
        collapsible: false,
        columns: vec![col_num, col_pct, col_date, col_txt],
        rows: vec![row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 4);
    for cell in &cells {
        assert_eq!(cell.currency, None);
    }
}

#[test]
fn test_scenario_10_descriptor_axis_isolation_suppresses_currency_and_conflicts() {
    let table_desc_only = TableSpec {
        id: TableId::from("DescOnly"),
        title: "Descriptor Only Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![ColumnSpec::descriptor("desc", "Description")],
        rows: vec![
            RowSpec::new("r1")
                .with_format(Some(FormatPreset::Currency))
                .with_currency(Some(CurrencyCode::Eur))
                .with_cells({
                    let mut map = IndexMap::new();
                    map.insert(ColId::from("desc"), CellInput::string("Label"));
                    map
                }),
        ],
    };

    let mut symbols_desc = SymbolTable::new();
    symbols_desc
        .register_table(SheetId::from("Model"), &table_desc_only)
        .unwrap();

    let cells = expand_table(&symbols_desc, &table_desc_only).unwrap();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].currency, None);
}

#[test]
fn test_scenario_11_cell_level_indentation_expansion_and_defaults() {
    let table = TableSpec {
        id: TableId::from("IndentedTable"),
        title: "Indented Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![
            ColumnSpec::descriptor("line_item", "Line Item"),
            ColumnSpec::new("val"),
        ],
        rows: vec![
            RowSpec::new("r1").with_cells({
                let mut map = IndexMap::new();
                map.insert(
                    ColId::from("line_item"),
                    CellInput::Detailed(DetailedCellInput {
                        value: Some(ValueLiteral::String("Salaries".into())),
                        indent: Some(2),
                        ..Default::default()
                    }),
                );
                map.insert(ColId::from("val"), CellInput::number(100.0));
                map
            }),
            RowSpec::new("r2").with_cells({
                let mut map = IndexMap::new();
                map.insert(ColId::from("line_item"), CellInput::string("Revenue"));
                map.insert(ColId::from("val"), CellInput::number(200.0));
                map
            }),
        ],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 4);

    // r1 line_item: indent 2
    let r1_item = cells
        .iter()
        .find(|c| c.id.row.as_str() == "r1" && c.id.col.as_str() == "line_item")
        .unwrap();
    assert_eq!(r1_item.indent, 2);
    assert_eq!(
        r1_item.payload.as_literal(),
        Some(&ValueLiteral::String("Salaries".into()))
    );

    // r1 val: indent 0
    let r1_val = cells
        .iter()
        .find(|c| c.id.row.as_str() == "r1" && c.id.col.as_str() == "val")
        .unwrap();
    assert_eq!(r1_val.indent, 0);

    // r2 line_item: unindented literal defaults to 0
    let r2_item = cells
        .iter()
        .find(|c| c.id.row.as_str() == "r2" && c.id.col.as_str() == "line_item")
        .unwrap();
    assert_eq!(r2_item.indent, 0);
    assert_eq!(
        r2_item.payload.as_literal(),
        Some(&ValueLiteral::String("Revenue".into()))
    );

    // r2 val: indent 0
    let r2_val = cells
        .iter()
        .find(|c| c.id.row.as_str() == "r2" && c.id.col.as_str() == "val")
        .unwrap();
    assert_eq!(r2_val.indent, 0);
}

#[test]
fn test_scenario_12_independent_indentation_across_multiple_descriptor_columns() {
    let table = TableSpec {
        id: TableId::from("MultiDescTable"),
        title: "Multi Descriptor Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![
            ColumnSpec::descriptor("category", "Category"),
            ColumnSpec::descriptor("item", "Item"),
            ColumnSpec::new("amount"),
        ],
        rows: vec![RowSpec::new("r1").with_cells({
            let mut map = IndexMap::new();
            map.insert(
                ColId::from("category"),
                CellInput::Detailed(DetailedCellInput {
                    value: Some(ValueLiteral::String("Opex".into())),
                    indent: Some(1),
                    ..Default::default()
                }),
            );
            map.insert(
                ColId::from("item"),
                CellInput::Detailed(DetailedCellInput {
                    value: Some(ValueLiteral::String("Rent".into())),
                    indent: Some(2),
                    ..Default::default()
                }),
            );
            map.insert(ColId::from("amount"), CellInput::number(500.0));
            map
        })],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 3);

    let cat_cell = cells
        .iter()
        .find(|c| c.id.col.as_str() == "category")
        .unwrap();
    assert_eq!(cat_cell.indent, 1);

    let item_cell = cells.iter().find(|c| c.id.col.as_str() == "item").unwrap();
    assert_eq!(item_cell.indent, 2);

    let amount_cell = cells
        .iter()
        .find(|c| c.id.col.as_str() == "amount")
        .unwrap();
    assert_eq!(amount_cell.indent, 0);
}
