#![allow(clippy::unwrap_used, clippy::expect_used)]

use finx::compiler::ir::{CellGrid, CellId, CellNode, CellPayload, IrError, SymbolTable};
use finx::expr::Expr;
use finx::model::format::FormatPreset;
use finx::model::id::{ColGroupId, ColId, RowGroupId, RowId, SheetId, TableId};
use finx::model::table::{
    AxisRole, ColumnGroup, ColumnSpec, RowGroup, RowGroupChild, RowSpec, TableSpec, ValueLiteral,
};
use indexmap::IndexMap;

#[test]
fn test_scenario_1_cell_id_display_and_formatting() {
    let cell_id = CellId::new("IncomeStatement", "revenue", "2024E");
    assert_eq!(
        cell_id.to_string(),
        "IncomeStatement[\"revenue\", \"2024E\"]"
    );

    // Display roundtrip formatting check
    let formatted = format!("{cell_id}");
    assert_eq!(formatted, "IncomeStatement[\"revenue\", \"2024E\"]");
}

#[test]
fn test_scenario_2_cell_grid_insertion_and_lookup() {
    let mut grid = CellGrid::new();
    assert_eq!(grid.len(), 0);
    assert!(grid.is_empty());

    let id = CellId::new("Assumptions", "tax_rate", "2024E");
    let cell = CellNode::new(
        id.clone(),
        SheetId::new("AssumptionsSheet"),
        CellPayload::Literal(ValueLiteral::Number(0.21)),
    )
    .with_format(Some(FormatPreset::Percentage))
    .with_note(Some("US federal statutory rate".to_string()));

    assert!(grid.insert(cell).is_ok());
    assert_eq!(grid.len(), 1);
    assert!(!grid.is_empty());
    assert!(grid.contains(&id));

    let retrieved = grid.get(&id).expect("Cell must be retrievable");
    assert_eq!(retrieved.id, id);
    assert_eq!(retrieved.sheet, SheetId::new("AssumptionsSheet"));
    assert_eq!(retrieved.format, Some(FormatPreset::Percentage));
    assert_eq!(retrieved.note.as_deref(), Some("US federal statutory rate"));
    assert_eq!(
        retrieved.payload.as_literal(),
        Some(&ValueLiteral::Number(0.21))
    );
}

#[test]
fn test_scenario_3_cell_grid_duplicate_rejection() {
    let mut grid = CellGrid::new();
    let id = CellId::new("IncomeStatement", "ebit", "2024E");

    let cell1 = CellNode::new(
        id.clone(),
        SheetId::new("Model"),
        CellPayload::Literal(ValueLiteral::Number(500.0)),
    );
    let cell2 = CellNode::new(
        id.clone(),
        SheetId::new("Model"),
        CellPayload::Formula(Expr::parse("IncomeStatement[\"revenue\", @col] * 0.2").unwrap()),
    );

    assert!(grid.insert(cell1).is_ok());
    let err = grid.insert(cell2).unwrap_err();
    assert_eq!(err, IrError::DuplicateCell { id });
}

#[test]
fn test_scenario_4_cell_grid_filtered_iteration() {
    let mut grid = CellGrid::new();
    let sheet = SheetId::new("Model");

    let cells = [
        CellNode::new(
            CellId::new("TableA", "r1", "2024E"),
            sheet.clone(),
            CellPayload::Literal(ValueLiteral::Number(1.0)),
        ),
        CellNode::new(
            CellId::new("TableA", "r1", "2025E"),
            sheet.clone(),
            CellPayload::Literal(ValueLiteral::Number(2.0)),
        ),
        CellNode::new(
            CellId::new("TableA", "r2", "2024E"),
            sheet.clone(),
            CellPayload::Literal(ValueLiteral::Number(3.0)),
        ),
        CellNode::new(
            CellId::new("TableB", "r1", "2024E"),
            sheet.clone(),
            CellPayload::Literal(ValueLiteral::Number(4.0)),
        ),
    ];

    for c in cells {
        grid.insert(c).unwrap();
    }

    // Filter by table
    let table_a_cells: Vec<_> = grid.cells_for_table(&TableId::new("TableA")).collect();
    assert_eq!(table_a_cells.len(), 3);

    // Filter by table and row
    let table_a_r1_cells: Vec<_> = grid
        .cells_for_row(&TableId::new("TableA"), &RowId::new("r1"))
        .collect();
    assert_eq!(table_a_r1_cells.len(), 2);

    // Filter by table and col
    let table_a_2024_cells: Vec<_> = grid
        .cells_for_col(&TableId::new("TableA"), &ColId::new("2024E"))
        .collect();
    assert_eq!(table_a_2024_cells.len(), 2);

    // Table B check
    let table_b_cells: Vec<_> = grid.cells_for_table(&TableId::new("TableB")).collect();
    assert_eq!(table_b_cells.len(), 1);

    // Keys in insertion order
    let keys: Vec<_> = grid.keys().cloned().collect();
    assert_eq!(
        keys,
        vec![
            CellId::new("TableA", "r1", "2024E"),
            CellId::new("TableA", "r1", "2025E"),
            CellId::new("TableA", "r2", "2024E"),
            CellId::new("TableB", "r1", "2024E"),
        ]
    );
}

#[test]
fn test_scenario_5_symbol_table_table_registration() {
    let mut symbols = SymbolTable::new();

    let table = TableSpec {
        id: TableId::new("IncomeStatement"),
        title: "Income Statement".to_string(),
        columns: vec![
            ColumnSpec::new("2023A"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
        ],
        column_groups: vec![ColumnGroup {
            id: ColGroupId::new("forecast"),
            label: "Forecast".to_string(),
            columns: vec![ColId::new("2024E"), ColId::new("2025E")],
        }],
        row_groups: vec![RowGroup {
            id: RowGroupId::new("opex"),
            label: "Operating Expenses".to_string(),
            children: vec![
                RowGroupChild::Row(RowId::new("r_and_d")),
                RowGroupChild::Row(RowId::new("s_and_m")),
            ],
        }],
        rows: vec![
            RowSpec {
                id: RowId::new("revenue"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: Some("100".to_string()),
                cells: IndexMap::new(),
            },
            RowSpec {
                id: RowId::new("r_and_d"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: Some("20".to_string()),
                cells: IndexMap::new(),
            },
            RowSpec {
                id: RowId::new("s_and_m"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: Some("30".to_string()),
                cells: IndexMap::new(),
            },
        ],
    };

    symbols
        .register_table(SheetId::new("Financials"), &table)
        .expect("Table should register successfully");

    assert_eq!(
        symbols.get_sheet(&TableId::new("IncomeStatement")),
        Some(&SheetId::new("Financials"))
    );
    assert_eq!(
        symbols.col_index(&TableId::new("IncomeStatement"), &ColId::new("2024E")),
        Some(1)
    );
    assert_eq!(
        symbols.row_index(&TableId::new("IncomeStatement"), &RowId::new("r_and_d")),
        Some(1)
    );

    // Sheet IDs query
    let sheet_ids: Vec<_> = symbols.sheet_ids().collect();
    assert_eq!(sheet_ids, vec![&SheetId::new("Financials")]);

    // Group queries
    let forecast_cols = symbols
        .cols_in_group(
            &TableId::new("IncomeStatement"),
            &ColGroupId::new("forecast"),
        )
        .expect("Forecast group should exist");
    assert_eq!(forecast_cols, &[ColId::new("2024E"), ColId::new("2025E")]);

    let opex_rows = symbols
        .rows_in_group(&TableId::new("IncomeStatement"), &RowGroupId::new("opex"))
        .expect("Opex group should exist");
    assert_eq!(opex_rows, &[RowId::new("r_and_d"), RowId::new("s_and_m")]);
}

#[test]
fn test_scenario_6_symbol_table_duplicate_table_rejection() {
    let mut symbols = SymbolTable::new();

    let table = TableSpec {
        id: TableId::new("CashFlow"),
        title: "Cash Flow".to_string(),
        columns: vec![ColumnSpec::new("2024E")],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![RowSpec {
            id: RowId::new("fcf"),
            role: AxisRole::Measure,
            format: None,
            note: None,
            source: None,
            expr: Some("100".to_string()),
            cells: IndexMap::new(),
        }],
    };

    symbols
        .register_table(SheetId::new("Sheet1"), &table)
        .expect("First registration should succeed");

    let err = symbols
        .register_table(SheetId::new("Sheet2"), &table)
        .unwrap_err();

    assert_eq!(
        err,
        IrError::DuplicateTable {
            table: TableId::new("CashFlow"),
            first_sheet: SheetId::new("Sheet1"),
            second_sheet: SheetId::new("Sheet2")
        }
    );
}

#[test]
fn test_cell_payload_formula_and_literal_inspection() {
    let lit_payload = CellPayload::Literal(ValueLiteral::String("abc".to_string()));
    assert!(lit_payload.is_literal());
    assert!(!lit_payload.is_formula());
    assert_eq!(
        lit_payload.as_literal(),
        Some(&ValueLiteral::String("abc".to_string()))
    );
    assert_eq!(lit_payload.as_formula(), None);

    let parsed_expr = Expr::parse("SUM(A[\"b\", @col])").unwrap();
    let formula_payload = CellPayload::Formula(parsed_expr.clone());
    assert!(formula_payload.is_formula());
    assert!(!formula_payload.is_literal());
    assert_eq!(formula_payload.as_formula(), Some(&parsed_expr));
    assert_eq!(formula_payload.as_literal(), None);
}

#[test]
fn test_scenario_7_symbol_table_nested_row_groups_flattening() {
    let mut symbols = SymbolTable::new();

    let table = TableSpec {
        id: TableId::new("BalanceSheet"),
        title: "Balance Sheet".to_string(),
        columns: vec![ColumnSpec::new("2024E"), ColumnSpec::new("2025E")],
        column_groups: vec![],
        row_groups: vec![RowGroup {
            id: RowGroupId::new("assets"),
            label: "Total Assets".to_string(),
            children: vec![
                RowGroupChild::Row(RowId::new("cash")),
                RowGroupChild::Group(RowGroup {
                    id: RowGroupId::new("current_assets"),
                    label: "Other Current Assets".to_string(),
                    children: vec![
                        RowGroupChild::Row(RowId::new("ar")),
                        RowGroupChild::Row(RowId::new("inventory")),
                    ],
                }),
                RowGroupChild::Row(RowId::new("ppe")),
            ],
        }],
        rows: vec![
            RowSpec {
                id: RowId::new("cash"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: Some("100".to_string()),
                cells: IndexMap::new(),
            },
            RowSpec {
                id: RowId::new("ar"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: Some("50".to_string()),
                cells: IndexMap::new(),
            },
            RowSpec {
                id: RowId::new("inventory"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: Some("30".to_string()),
                cells: IndexMap::new(),
            },
            RowSpec {
                id: RowId::new("ppe"),
                role: AxisRole::Measure,
                format: None,
                note: None,
                source: None,
                expr: Some("200".to_string()),
                cells: IndexMap::new(),
            },
        ],
    };

    symbols
        .register_table(SheetId::new("BS"), &table)
        .expect("Table should register successfully");

    // Check nested child group
    let current_assets = symbols
        .rows_in_group(
            &TableId::new("BalanceSheet"),
            &RowGroupId::new("current_assets"),
        )
        .expect("Sub-group should exist");
    assert_eq!(current_assets, &[RowId::new("ar"), RowId::new("inventory")]);

    // Check parent group with flattened descendants in traversal order
    let total_assets = symbols
        .rows_in_group(&TableId::new("BalanceSheet"), &RowGroupId::new("assets"))
        .expect("Parent group should exist");
    assert_eq!(
        total_assets,
        &[
            RowId::new("cash"),
            RowId::new("ar"),
            RowId::new("inventory"),
            RowId::new("ppe")
        ]
    );
}

#[test]
fn test_scenario_8_symbol_table_empty_table_rejection() {
    let mut symbols = SymbolTable::new();

    let table = TableSpec {
        id: TableId::new("EmptyTable"),
        title: "Empty Table".to_string(),
        columns: vec![],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![],
    };

    let err = symbols
        .register_table(SheetId::new("Sheet1"), &table)
        .unwrap_err();

    assert_eq!(
        err,
        IrError::EmptyTable {
            table: TableId::new("EmptyTable")
        }
    );
}
