#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use finx::compiler::ir::{
    CellId, CellPayload, IrError, SymbolTable, expand_grid, expand_table, expand_workbook,
};
use finx::expr::ast::Expr;
use finx::model::format::FormatPreset;
use finx::model::id::{ColGroupId, ColId, RowId, SheetId, TableId};
use finx::model::manifest::{ProjectManifest, SheetSpec};
use finx::model::table::{
    AxisRole, CellInput, ColumnGroup, ColumnSpec, DetailedCellInput, RowSpec, TableSpec,
    ValueLiteral,
};
use indexmap::IndexMap;

fn create_simple_table(table_id: &str, cols: &[&str], rows: Vec<RowSpec>) -> TableSpec {
    TableSpec {
        id: TableId::from(table_id),
        title: format!("Title for {table_id}"),
        columns: cols.iter().map(|&c| ColumnSpec::new(c)).collect(),
        column_groups: vec![],
        row_groups: vec![],
        rows,
    }
}

#[test]
fn test_scenario_1_expand_literal_row_across_columns() {
    let mut cells = IndexMap::new();
    cells.insert("2021A".into(), CellInput::number(100.0));
    cells.insert("2022A".into(), CellInput::number(110.0));
    cells.insert("2023A".into(), CellInput::number(120.0));

    let row = RowSpec {
        id: RowId::from("revenue"),
        role: AxisRole::Measure,
        format: None,
        note: None,
        source: None,
        expr: None,
        cells,
    };

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
    let row = RowSpec {
        id: RowId::from("ebit"),
        role: AxisRole::Measure,
        format: Some(FormatPreset::Currency),
        note: Some("Operating margin formula".to_string()),
        source: None,
        expr: Some("IncomeStatement[\"revenue\", @col] * 0.1".to_string()),
        cells: IndexMap::new(),
    };

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
    assert_eq!(cells[0].note.as_deref(), Some("Operating margin formula"));

    assert_eq!(cells[1].id, CellId::new("IncomeStatement", "ebit", "2025E"));
    assert_eq!(cells[1].payload, CellPayload::Formula(expected_ast));
    assert_eq!(cells[1].format, Some(FormatPreset::Currency));
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

    let row = RowSpec {
        id: RowId::from("revenue"),
        role: AxisRole::Measure,
        format: Some(FormatPreset::Currency),
        note: Some("5% growth projection".to_string()),
        source: None,
        expr: Some("IncomeStatement[\"revenue\", col.prev] * 1.05".to_string()),
        cells,
    };

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
    assert_eq!(cells[1].note.as_deref(), Some("5% growth projection"));

    // 2025E: Vector Formula
    assert_eq!(
        cells[2].id,
        CellId::new("IncomeStatement", "revenue", "2025E")
    );
    assert!(cells[2].payload.is_formula());
    assert_eq!(cells[2].note.as_deref(), Some("5% growth projection"));
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
            source: Some(serde_json::json!({ "source_id": "Management Guidance" })),
            ..Default::default()
        }),
    );

    let row = RowSpec {
        id: RowId::from("revenue"),
        role: AxisRole::Measure,
        format: Some(FormatPreset::Currency),
        note: Some("Base revenue note".to_string()),
        source: Some(serde_json::json!({ "source_id": "10-K" })),
        expr: None,
        cells,
    };

    let table = create_simple_table("IncomeStatement", &["2023A", "2024E", "2025E"], vec![row]);

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .expect("registration succeeds");

    let cells = expand_table(&symbols, &table).expect("expansion succeeds");
    assert_eq!(cells.len(), 3);

    // 2023A: Inherits format, note, source
    assert_eq!(cells[0].format, Some(FormatPreset::Currency));
    assert_eq!(cells[0].note.as_deref(), Some("Base revenue note"));
    assert_eq!(
        cells[0].source,
        Some(serde_json::json!({ "source_id": "10-K" }))
    );

    // 2024E: Inherits format, note, source
    assert_eq!(cells[1].format, Some(FormatPreset::Currency));
    assert_eq!(cells[1].note.as_deref(), Some("Base revenue note"));

    // 2025E: Inherits format, overrides note and source
    assert_eq!(cells[2].format, Some(FormatPreset::Currency));
    assert_eq!(cells[2].note.as_deref(), Some("Forecast update"));
    assert_eq!(
        cells[2].source,
        Some(serde_json::json!({ "source_id": "Management Guidance" }))
    );
}

#[test]
fn test_scenario_5_actionable_formula_parse_error_diagnostic() {
    let row = RowSpec {
        id: RowId::from("bad_calc"),
        role: AxisRole::Measure,
        format: None,
        note: None,
        source: None,
        expr: Some("IncomeStatement[\"revenue\", @col] + * 5".to_string()),
        cells: IndexMap::new(),
    };

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
                tables: vec![PathBuf::from("tables/assumptions.json")],
            },
            SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![PathBuf::from("tables/income_statement.json")],
            },
        ],
    };

    let mut assumptions_cells = IndexMap::new();
    assumptions_cells.insert("value".into(), CellInput::number(0.21));
    let assumptions_table = create_simple_table(
        "Assumptions",
        &["value"],
        vec![RowSpec {
            id: RowId::from("tax_rate"),
            role: AxisRole::Measure,
            format: Some(FormatPreset::Percentage),
            note: None,
            source: None,
            expr: None,
            cells: assumptions_cells,
        }],
    );

    let mut is_cells = IndexMap::new();
    is_cells.insert("2023A".into(), CellInput::number(1000.0));
    is_cells.insert("2024E".into(), CellInput::number(1100.0));
    let is_table = create_simple_table(
        "IncomeStatement",
        &["2023A", "2024E"],
        vec![RowSpec {
            id: RowId::from("revenue"),
            role: AxisRole::Measure,
            format: Some(FormatPreset::Currency),
            note: None,
            source: None,
            expr: None,
            cells: is_cells,
        }],
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
        vec![RowSpec {
            id: RowId::from("revenue"),
            role: AxisRole::Measure,
            format: None,
            note: None,
            source: None,
            expr: None,
            cells,
        }],
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
fn test_expand_column_group_with_vector_rules() {
    let mut table = create_simple_table(
        "BalanceSheet",
        &["2021A", "2022A", "2023A", "2024E", "2025E"],
        vec![],
    );
    table.column_groups = vec![
        ColumnGroup {
            id: ColGroupId::from("historical"),
            label: "Historical".to_string(),
            columns: vec![
                ColId::from("2021A"),
                ColId::from("2022A"),
                ColId::from("2023A"),
            ],
        },
        ColumnGroup {
            id: ColGroupId::from("forecast"),
            label: "Forecast".to_string(),
            columns: vec![ColId::from("2024E"), ColId::from("2025E")],
        },
    ];

    let mut cells = IndexMap::new();
    cells.insert("2021A".into(), CellInput::number(50.0));
    cells.insert("2022A".into(), CellInput::number(60.0));
    cells.insert("2023A".into(), CellInput::number(70.0));

    table.rows = vec![RowSpec {
        id: RowId::from("cash"),
        role: AxisRole::Measure,
        format: Some(FormatPreset::Currency),
        note: None,
        source: None,
        expr: Some("BalanceSheet[\"cash\", col.prev] + 10.0".to_string()),
        cells,
    }];

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
            tables: vec![PathBuf::from("tables/assumptions.json")],
        }],
    };

    let mut t1_cells = IndexMap::new();
    t1_cells.insert("value".into(), CellInput::number(0.21));
    let table1 = create_simple_table(
        "Assumptions",
        &["value"],
        vec![RowSpec {
            id: RowId::from("tax_rate"),
            role: AxisRole::Measure,
            format: None,
            note: None,
            source: None,
            expr: None,
            cells: t1_cells,
        }],
    );

    let mut t2_cells = IndexMap::new();
    t2_cells.insert("value".into(), CellInput::number(1.0));
    let table2 = create_simple_table(
        "OrphanTable",
        &["value"],
        vec![RowSpec {
            id: RowId::from("foo"),
            role: AxisRole::Measure,
            format: None,
            note: None,
            source: None,
            expr: None,
            cells: t2_cells,
        }],
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
        vec![RowSpec {
            id: RowId::from("r1"),
            role: AxisRole::Measure,
            format: None,
            note: None,
            source: None,
            expr: None,
            cells: t1_cells,
        }],
    );

    let mut t2_cells = IndexMap::new();
    t2_cells.insert("c1".into(), CellInput::number(3.0));
    let t2 = create_simple_table(
        "TableB",
        &["c1"],
        vec![RowSpec {
            id: RowId::from("r2"),
            role: AxisRole::Measure,
            format: None,
            note: None,
            source: None,
            expr: None,
            cells: t2_cells,
        }],
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
    let mut actual_col = ColumnSpec::new("actual");
    actual_col.format = Some(FormatPreset::Currency);

    let mut forecast_col = ColumnSpec::new("forecast");
    forecast_col.format = Some(FormatPreset::Currency);
    forecast_col.expr = Some("IncomeStatement[\"revenue\", \"actual\"] * 1.1".to_string());

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
        columns: vec![actual_col, forecast_col],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            RowSpec {
                id: RowId::from("revenue"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: None,
                cells: rev_cells,
            },
            RowSpec {
                id: RowId::from("cogs"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: None,
                cells: cogs_cells,
            },
            RowSpec {
                id: RowId::from("opex"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: None,
                cells: opex_cells,
            },
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

    // Opex: actual is literal, forecast has 0D cell override literal (35.0)
    assert_eq!(
        cells[4].id,
        CellId::new("IncomeStatement", "opex", "actual")
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
fn test_descriptor_row_isolates_formulas_and_enforces_assignment() {
    let mut col1 = ColumnSpec::new("c1");
    col1.expr = Some("10".to_string());

    let mut col2 = ColumnSpec::new("c2");
    col2.expr = Some("20".to_string());

    // Unassigned descriptor row halts with UnassignedCell
    let unassigned_table = TableSpec {
        id: TableId::from("T_Unassigned"),
        title: "Test".to_string(),
        columns: vec![col1.clone(), col2.clone()],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![RowSpec {
            id: RowId::from("header"),
            role: AxisRole::Descriptor,
            format: None,
            note: None,
            source: None,
            expr: None,
            cells: IndexMap::new(),
        }],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &unassigned_table)
        .unwrap();

    let err = expand_table(&symbols, &unassigned_table).unwrap_err();
    assert!(matches!(
        err,
        IrError::UnassignedCell { table, row, column }
            if table.as_str() == "T_Unassigned" && row.as_str() == "header" && column.as_str() == "c1"
    ));

    // Assigned descriptor row emits literals and isolates column formulas
    let mut header_cells = IndexMap::new();
    header_cells.insert("c1".into(), CellInput::string("Header 1"));
    header_cells.insert("c2".into(), CellInput::string("Header 2"));

    let assigned_table = TableSpec {
        id: TableId::from("T_Assigned"),
        title: "Test".to_string(),
        columns: vec![col1, col2],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            RowSpec {
                id: RowId::from("header"),
                role: AxisRole::Descriptor,
                format: None,
                note: None,
                source: None,
                expr: None,
                cells: header_cells,
            },
            RowSpec {
                id: RowId::from("data"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: None,
                cells: IndexMap::new(),
            },
        ],
    };

    let mut symbols2 = SymbolTable::new();
    symbols2
        .register_table(SheetId::from("Model"), &assigned_table)
        .unwrap();

    let cells = expand_table(&symbols2, &assigned_table).unwrap();
    assert_eq!(cells.len(), 4);

    // Header cells are string literals
    assert_eq!(
        cells[0].payload,
        CellPayload::Literal(ValueLiteral::String("Header 1".to_string()))
    );
    assert_eq!(
        cells[1].payload,
        CellPayload::Literal(ValueLiteral::String("Header 2".to_string()))
    );

    // Measure row receives column formula broadcasts
    assert!(cells[2].payload.is_formula());
    assert!(cells[3].payload.is_formula());
}

#[test]
fn test_column_formula_parse_error_diagnostic() {
    let mut bad_col = ColumnSpec::new("bad_col");
    bad_col.expr = Some("1 + * 2".to_string());

    let table = TableSpec {
        id: TableId::from("BadColTable"),
        title: "Bad Col".to_string(),
        columns: vec![bad_col],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![RowSpec {
            id: RowId::from("r1"),
            role: AxisRole::Measure,
            format: None,
            note: None,
            source: None,
            expr: None,
            cells: IndexMap::new(),
        }],
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
fn test_dual_descriptor_format_fallback() {
    let mut desc_col = ColumnSpec::descriptor("d_col", "Desc Col");
    desc_col.format = None;

    let mut desc_row = RowSpec::descriptor("d_row");
    desc_row.format = Some(FormatPreset::Text);
    desc_row
        .cells
        .insert("d_col".into(), CellInput::string("Label"));

    let table = TableSpec {
        id: TableId::from("DualDesc"),
        title: "Dual Descriptor Table".to_string(),
        columns: vec![desc_col],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![desc_row],
    };

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .unwrap();

    let cells = expand_table(&symbols, &table).unwrap();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].format, Some(FormatPreset::Text));
}
