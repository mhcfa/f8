#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use finx::compiler::ir::{
    CellId, CellPayload, IrError, SymbolTable, expand_grid, expand_table, expand_workbook,
};
use finx::expr::ast::Expr;
use finx::model::format::FormatPreset;
use finx::model::id::{ColGroupId, ColId, RowId, SheetId, TableId};
use finx::model::manifest::{ProjectManifest, SheetSpec};
use finx::model::table::{ColumnGroup, MatchArmSpec, RowSpec, TableSpec, ValueLiteral};
use indexmap::IndexMap;

fn create_simple_table(table_id: &str, cols: &[&str], rows: Vec<RowSpec>) -> TableSpec {
    TableSpec {
        id: TableId::from(table_id),
        title: format!("Title for {table_id}"),
        columns: cols.iter().map(|&c| ColId::from(c)).collect(),
        column_groups: vec![],
        row_groups: vec![],
        rows,
    }
}

#[test]
fn test_scenario_1_expand_literal_row_across_columns() {
    let row = RowSpec {
        id: RowId::from("revenue"),
        label: "Revenue".to_string(),
        format: None,
        note: None,
        source: None,
        value: Some(vec![
            ValueLiteral::Number(100.0),
            ValueLiteral::Number(110.0),
            ValueLiteral::Number(120.0),
        ]),
        expr: None,
        match_arms: None,
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
        label: "Operating Income".to_string(),
        format: Some(FormatPreset::Currency),
        note: Some("Operating margin formula".to_string()),
        source: None,
        value: None,
        expr: Some("IncomeStatement[\"revenue\", @] * 0.1".to_string()),
        match_arms: None,
    };

    let table = create_simple_table("IncomeStatement", &["2024E", "2025E"], vec![row]);

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .expect("registration succeeds");

    let cells = expand_table(&symbols, &table).expect("expansion succeeds");
    assert_eq!(cells.len(), 2);

    let expected_ast = Expr::parse("IncomeStatement[\"revenue\", @] * 0.1").expect("valid formula");

    assert_eq!(cells[0].id, CellId::new("IncomeStatement", "ebit", "2024E"));
    assert_eq!(cells[0].payload, CellPayload::Formula(expected_ast.clone()));
    assert_eq!(cells[0].format, Some(FormatPreset::Currency));
    assert_eq!(cells[0].note.as_deref(), Some("Operating margin formula"));

    assert_eq!(cells[1].id, CellId::new("IncomeStatement", "ebit", "2025E"));
    assert_eq!(cells[1].payload, CellPayload::Formula(expected_ast));
    assert_eq!(cells[1].format, Some(FormatPreset::Currency));
}

#[test]
fn test_scenario_3_expand_partitioned_row_with_match_arms() {
    let row = RowSpec {
        id: RowId::from("revenue"),
        label: "Total Revenue".to_string(),
        format: Some(FormatPreset::Currency),
        note: None,
        source: None,
        value: None,
        expr: None,
        match_arms: Some(IndexMap::from([
            (
                "2023A".to_string(),
                MatchArmSpec {
                    value: Some(vec![ValueLiteral::Number(100.0)]),
                    expr: None,
                    note: Some("Actual 10-K".to_string()),
                    source: None,
                },
            ),
            (
                "2024E..2025E".to_string(),
                MatchArmSpec {
                    value: None,
                    expr: Some("IncomeStatement[\"revenue\", col.prev] * 1.05".to_string()),
                    note: Some("5% growth projection".to_string()),
                    source: None,
                },
            ),
        ])),
    };

    let table = create_simple_table("IncomeStatement", &["2023A", "2024E", "2025E"], vec![row]);

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Model"), &table)
        .expect("registration succeeds");

    let cells = expand_table(&symbols, &table).expect("expansion succeeds");
    assert_eq!(cells.len(), 3);

    // 2023A: Literal
    assert_eq!(
        cells[0].id,
        CellId::new("IncomeStatement", "revenue", "2023A")
    );
    assert_eq!(
        cells[0].payload,
        CellPayload::Literal(ValueLiteral::Number(100.0))
    );
    assert_eq!(cells[0].note.as_deref(), Some("Actual 10-K"));

    // 2024E: Formula
    assert_eq!(
        cells[1].id,
        CellId::new("IncomeStatement", "revenue", "2024E")
    );
    assert!(cells[1].payload.is_formula());
    assert_eq!(cells[1].note.as_deref(), Some("5% growth projection"));

    // 2025E: Formula
    assert_eq!(
        cells[2].id,
        CellId::new("IncomeStatement", "revenue", "2025E")
    );
    assert!(cells[2].payload.is_formula());
    assert_eq!(cells[2].note.as_deref(), Some("5% growth projection"));
}

#[test]
fn test_scenario_4_metadata_and_format_preset_propagation() {
    let row = RowSpec {
        id: RowId::from("revenue"),
        label: "Revenue".to_string(),
        format: Some(FormatPreset::Currency),
        note: Some("Base revenue note".to_string()),
        source: Some(serde_json::json!({ "source_id": "10-K" })),
        value: None,
        expr: None,
        match_arms: Some(IndexMap::from([
            (
                "2023A..2024E".to_string(),
                MatchArmSpec {
                    value: Some(vec![
                        ValueLiteral::Number(100.0),
                        ValueLiteral::Number(105.0),
                    ]),
                    expr: None,
                    note: None, // Should inherit row note
                    source: None,
                },
            ),
            (
                "2025E".to_string(),
                MatchArmSpec {
                    value: Some(vec![ValueLiteral::Number(110.0)]),
                    expr: None,
                    note: Some("Forecast update".to_string()), // Should override row note
                    source: Some(serde_json::json!({ "source_id": "Management Guidance" })),
                },
            ),
        ])),
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
        label: "Broken Formula".to_string(),
        format: None,
        note: None,
        source: None,
        value: None,
        expr: Some("IncomeStatement[\"revenue\", @] + * 5".to_string()),
        match_arms: None,
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
            assert_eq!(row, RowId::from("bad_calc"));
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

    let assumptions_table = create_simple_table(
        "Assumptions",
        &["value"],
        vec![RowSpec {
            id: RowId::from("tax_rate"),
            label: "Tax Rate".to_string(),
            format: Some(FormatPreset::Percentage),
            note: None,
            source: None,
            value: Some(vec![ValueLiteral::Number(0.21)]),
            expr: None,
            match_arms: None,
        }],
    );

    let is_table = create_simple_table(
        "IncomeStatement",
        &["2023A", "2024E"],
        vec![RowSpec {
            id: RowId::from("revenue"),
            label: "Revenue".to_string(),
            format: Some(FormatPreset::Currency),
            note: None,
            source: None,
            value: Some(vec![
                ValueLiteral::Number(1000.0),
                ValueLiteral::Number(1100.0),
            ]),
            expr: None,
            match_arms: None,
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
    let table = create_simple_table(
        "UnknownTable",
        &["2024E"],
        vec![RowSpec {
            id: RowId::from("revenue"),
            label: "Revenue".to_string(),
            format: None,
            note: None,
            source: None,
            value: Some(vec![ValueLiteral::Number(100.0)]),
            expr: None,
            match_arms: None,
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
fn test_expand_column_group_in_match_arms() {
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
    table.rows = vec![RowSpec {
        id: RowId::from("cash"),
        label: "Cash and Equivalents".to_string(),
        format: Some(FormatPreset::Currency),
        note: None,
        source: None,
        value: None,
        expr: None,
        match_arms: Some(IndexMap::from([
            (
                "cols.historical".to_string(),
                MatchArmSpec {
                    value: Some(vec![
                        ValueLiteral::Number(50.0),
                        ValueLiteral::Number(60.0),
                        ValueLiteral::Number(70.0),
                    ]),
                    expr: None,
                    note: None,
                    source: None,
                },
            ),
            (
                "cols.forecast".to_string(),
                MatchArmSpec {
                    value: None,
                    expr: Some("BalanceSheet[\"cash\", col.prev] + 10.0".to_string()),
                    note: None,
                    source: None,
                },
            ),
        ])),
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

    let table1 = create_simple_table(
        "Assumptions",
        &["value"],
        vec![RowSpec {
            id: RowId::from("tax_rate"),
            label: "Tax Rate".to_string(),
            format: None,
            note: None,
            source: None,
            value: Some(vec![ValueLiteral::Number(0.21)]),
            expr: None,
            match_arms: None,
        }],
    );

    let table2 = create_simple_table(
        "OrphanTable",
        &["value"],
        vec![RowSpec {
            id: RowId::from("foo"),
            label: "Foo".to_string(),
            format: None,
            note: None,
            source: None,
            value: Some(vec![ValueLiteral::Number(1.0)]),
            expr: None,
            match_arms: None,
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
    let t1 = create_simple_table(
        "TableA",
        &["c1", "c2"],
        vec![RowSpec {
            id: RowId::from("r1"),
            label: "R1".to_string(),
            format: None,
            note: None,
            source: None,
            value: Some(vec![ValueLiteral::Number(1.0), ValueLiteral::Number(2.0)]),
            expr: None,
            match_arms: None,
        }],
    );

    let t2 = create_simple_table(
        "TableB",
        &["c1"],
        vec![RowSpec {
            id: RowId::from("r2"),
            label: "R2".to_string(),
            format: None,
            note: None,
            source: None,
            value: Some(vec![ValueLiteral::Number(3.0)]),
            expr: None,
            match_arms: None,
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
