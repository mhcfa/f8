#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::layout::LayoutPlanner;
use f8_core::compiler::lowering::{FormulaLowerer, LoweringError};
use f8_core::compiler::{Compiler, ir};
use f8_core::expr::ast::{ColSelector, Expr, Literal, RowSelector, TableRef, TableSelector};
use f8_core::model::id::{ColId, RowId, TableId};
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{CellInput, ColumnSpec, RowSpec, TableSpec};
use f8_core::model::theme::Theme;

fn create_comprehensive_model() -> (ProjectManifest, Vec<TableSpec>) {
    // 1. IncomeStatement table
    let is_cols = vec![
        ColumnSpec::new("2023A").with_headers(["Historical", "2023A"]),
        ColumnSpec::new("2024E").with_headers(["Forecast", "2024E"]),
        ColumnSpec::new("2025E").with_headers(["Forecast", "2025E"]),
        ColumnSpec::new("2026E").with_headers(["Forecast", "2026E"]),
        ColumnSpec::new("2027E").with_headers(["Forecast", "2027E"]),
    ];
    let mut is_rows = vec![
        RowSpec::new("revenue"),
        RowSpec::new("cogs"),
        RowSpec::new("gross_profit"),
        RowSpec::new("r_and_d"),
        RowSpec::new("s_and_m"),
        RowSpec::new("ebit"),
    ];
    // Populate default cell values
    for row in &mut is_rows {
        for col in &is_cols {
            if let RowSpec::Data(d) = row
                && let Some(col_id) = col.id()
            {
                d.cells.insert(col_id.clone(), CellInput::number(100.0));
            }
        }
    }
    // gross_profit = revenue - cogs
    if let RowSpec::Data(d) = &mut is_rows[2] {
        for col in &["2024E", "2025E", "2026E", "2027E"] {
            d.cells.insert(
                ColId::from(*col),
                CellInput::formula(
                    "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
                ),
            );
        }
    }
    let income_statement = TableSpec {
        id: TableId::from("IncomeStatement"),
        title: "Income Statement".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: is_cols,
        rows: is_rows,
    };

    // 2. CapTable table
    let cap_cols = vec![
        ColumnSpec::new("shares").with_header("shares"),
        ColumnSpec::new("share_price").with_header("share_price"),
        ColumnSpec::new("invested_capital").with_header("invested_capital"),
    ];
    let mut cap_rows = vec![RowSpec::new("seed"), RowSpec::new("series_a")];
    for row in &mut cap_rows {
        for col in &cap_cols {
            if let RowSpec::Data(d) = row
                && let Some(col_id) = col.id()
            {
                d.cells.insert(col_id.clone(), CellInput::number(10.0));
            }
        }
    }
    // series_a invested = shares * share_price
    if let RowSpec::Data(d) = &mut cap_rows[1] {
        d.cells.insert(
            ColId::from("invested_capital"),
            CellInput::formula("CapTable[@row, \"shares\"] * CapTable[@row, \"share_price\"]"),
        );
    }
    let cap_table = TableSpec {
        id: TableId::from("CapTable"),
        title: "Cap Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: cap_cols,
        rows: cap_rows,
    };

    // 3. BalanceSheet table
    let bs_cols = vec![
        ColumnSpec::new("2023A").with_header("2023A"),
        ColumnSpec::new("2024E").with_header("2024E"),
        ColumnSpec::new("2025E").with_header("2025E"),
    ];
    let mut bs_rows = vec![
        RowSpec::section_header("Current Assets"),
        RowSpec::new("cash"),
        RowSpec::new("ar"),
        RowSpec::new("inventory"),
        RowSpec::new("total_current_assets"),
    ];
    for row in &mut bs_rows {
        for col in &bs_cols {
            if let RowSpec::Data(d) = row
                && let Some(col_id) = col.id()
            {
                d.cells.insert(col_id.clone(), CellInput::number(50.0));
            }
        }
    }
    let balance_sheet = TableSpec {
        id: TableId::from("BalanceSheet"),
        title: "Balance Sheet".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: bs_cols,
        rows: bs_rows,
    };

    // 4. CashFlow table
    let cf_cols = vec![
        ColumnSpec::new("2023A").with_header("2023A"),
        ColumnSpec::new("2024E").with_header("2024E"),
        ColumnSpec::new("2025E").with_header("2025E"),
    ];
    let mut cf_rows = vec![RowSpec::new("net_change")];
    for col in &cf_cols {
        if let RowSpec::Data(d) = &mut cf_rows[0]
            && let Some(col_id) = col.id()
        {
            d.cells.insert(col_id.clone(), CellInput::number(25.0));
        }
    }
    let cash_flow = TableSpec {
        id: TableId::from("CashFlow"),
        title: "Cash Flow Statement".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: cf_cols,
        rows: cf_rows,
    };

    // 5. DebtSchedule table
    let ds_cols = vec![
        ColumnSpec::new("2023A").with_headers(["Historical", "2023A"]),
        ColumnSpec::new("2024E").with_headers(["Forecast", "2024E"]),
        ColumnSpec::new("2025E").with_headers(["Forecast", "2025E"]),
    ];
    let mut ds_rows = vec![
        RowSpec::new("beginning_balance"),
        RowSpec::new("end_balance"),
    ];
    for row in &mut ds_rows {
        for col in &ds_cols {
            if let RowSpec::Data(d) = row
                && let Some(col_id) = col.id()
            {
                d.cells.insert(col_id.clone(), CellInput::number(200.0));
            }
        }
    }
    let debt_schedule = TableSpec {
        id: TableId::from("DebtSchedule"),
        title: "Debt Schedule".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: ds_cols,
        rows: ds_rows,
    };

    // 6. Assumptions table on separate sheet
    let assump_cols = vec![ColumnSpec::new("value").with_header("value")];
    let mut assump_rows = vec![RowSpec::new("tax_rate"), RowSpec::new("discount_rate")];
    if let RowSpec::Data(d) = &mut assump_rows[0] {
        d.cells
            .insert(ColId::from("value"), CellInput::number(0.21));
    }
    if let RowSpec::Data(d) = &mut assump_rows[1] {
        d.cells
            .insert(ColId::from("value"), CellInput::number(0.10));
    }
    let assumptions = TableSpec {
        id: TableId::from("Assumptions"),
        title: "Model Assumptions".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: assump_cols,
        rows: assump_rows,
    };

    let manifest = ProjectManifest {
        model_name: "f8Model".to_string(),
        theme: Default::default(),
        sheets: vec![
            SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![
                    TableId::from("IncomeStatement"),
                    TableId::from("CapTable"),
                    TableId::from("BalanceSheet"),
                    TableId::from("CashFlow"),
                    TableId::from("DebtSchedule"),
                ],
            },
            SheetSpec {
                name: "Assumptions".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("Assumptions")],
            },
        ],
    };

    let tables = vec![
        income_statement,
        cap_table,
        balance_sheet,
        cash_flow,
        debt_schedule,
        assumptions,
    ];

    (manifest, tables)
}

#[test]
fn test_scenario_1_intra_sheet_active_column_formula() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");
    let expr = Expr::parse("IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]")
        .unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // In IncomeStatement on sheet Model:
    // Revenue is row 6, cogs is row 7, 2024E is col C
    assert_eq!(lowered, "=C6-C7");
}

#[test]
fn test_scenario_2_intra_sheet_active_row_formula() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("CapTable", "series_a", "invested_capital");
    let expr = Expr::parse("CapTable[@row, \"shares\"] * CapTable[@row, \"share_price\"]").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // CapTable is stacked below IncomeStatement on sheet Model.
    // At series_a (row 18): shares is col B, share_price is col C
    assert_eq!(lowered, "=B18*C18");
}

#[test]
fn test_scenario_3_relative_temporal_predecessor_offset() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("BalanceSheet", "cash", "2025E");
    let expr =
        Expr::parse("BalanceSheet[\"cash\", col.prev] + CashFlow[\"net_change\", @col]").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // col.prev in 2025E (col D) points to 2024E (col C).
    // cash is row 25, net_change is row 34.
    assert_eq!(lowered, "=C25+D34");
}

#[test]
fn test_scenario_4_cross_sheet_absolute_assumption_reference() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("IncomeStatement", "ebit", "2024E");
    let expr =
        Expr::parse("IncomeStatement[\"ebit\", @col] * Assumptions[\"tax_rate\", \"value\"]")
            .unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // ebit at 2024E is C11. Assumptions is on sheet Assumptions, tax_rate is cell B5 ($B$5).
    assert_eq!(lowered, "=C11*'Assumptions'!$B$5");
}

#[test]
fn test_scenario_5_intra_sheet_specific_key_absolute_reference() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("DebtSchedule", "beginning_balance", "2024E");
    let expr = Expr::parse("DebtSchedule[\"beginning_balance\", \"2023A\"]").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // Specific keys for both row and col resolve to absolute coordinates ($B$41)
    assert_eq!(lowered, "=$B$41");
}

#[test]
fn test_scenario_6_group_milestone_anchor_reference() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("DebtSchedule", "beginning_balance", "2024E");
    let expr = Expr::parse("DebtSchedule[\"beginning_balance\", \"2023A\"]").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // 2023A resolves to col B, row 41, which is absolute ($B$41)
    assert_eq!(lowered, "=$B$41");
}

#[test]
fn test_scenario_7_horizontal_range_row_slice() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");
    let expr = Expr::parse("SUM(IncomeStatement[\"revenue\", \"2024E\"..\"2027E\"])").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // 2024E..2027E covers cols C..F at revenue row 6.
    // Neither dimension is dynamic, so bounds are pinned ($C$6:$F$6).
    assert_eq!(lowered, "=SUM($C$6:$F$6)");
}

#[test]
fn test_scenario_8_horizontal_range_with_active_row() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("CapTable", "series_a", "invested_capital");
    let expr = Expr::parse("SUM(CapTable[@row, \"shares\"..\"share_price\"])").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // CapTable row series_a is row 18, shares is B, share_price is C.
    // Active row emits relative row bounds without $ (B18:C18).
    assert_eq!(lowered, "=SUM(B18:C18)");
}

#[test]
fn test_scenario_9_vertical_range_column_slice_with_active_column() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("BalanceSheet", "total_current_assets", "2024E");
    let expr = Expr::parse("SUM(BalanceSheet[\"cash\"..\"inventory\", @col])").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // cash..inventory rows are rows 25..27.
    // @col is 2024E (col C).
    // Relative col, pinned row bounds: C$25:C$27.
    assert_eq!(lowered, "=SUM(C$25:C$27)");
}

#[test]
fn test_scenario_10_vertical_range_column_slice_with_specific_column() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("BalanceSheet", "total_current_assets", "2025E");
    let expr = Expr::parse("SUM(BalanceSheet[\"cash\"..\"inventory\", \"2024E\"])").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // Specific column 2024E (col C) with cash..inventory (rows 25..27).
    // Absolute column and row bounds: $C$25:$C$27.
    assert_eq!(lowered, "=SUM($C$25:$C$27)");
}

#[test]
fn test_scenario_11_2d_block_range() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");
    let expr = Expr::parse("SUM(IncomeStatement[\"r_and_d\"..\"s_and_m\", \"2024E\"..\"2027E\"])")
        .unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // Rows r_and_d..s_and_m are rows 9..10. cols 2024E..2027E are cols C..F.
    // 2D block range with absolute bounds: $C$9:$F$10.
    assert_eq!(lowered, "=SUM($C$9:$F$10)");
}

#[test]
fn test_scenario_12_binary_operator_precedence_and_parentheses() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");

    // (A + B) * C / (D - E)
    let expr = Expr::parse(
        "(IncomeStatement[\"revenue\", @col] + IncomeStatement[\"cogs\", @col]) * 2 / (IncomeStatement[\"ebit\", @col] - 10)"
    ).unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    assert_eq!(lowered, "=(C6+C7)*2/(C11-10)");
}

#[test]
fn test_scenario_13_function_calls_with_omitted_positional_arguments() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");

    // Function call with omitted 4th argument: XLOOKUP(lookup, array, return, _, 0, 1)
    let expr = Expr::FunctionCall {
        name: "xlookup".to_string(),
        args: vec![
            Expr::Literal(Literal::Number(100.0)),
            Expr::Reference(TableRef {
                table: TableSelector::from("IncomeStatement"),
                row: RowSelector::Key(RowId::from("revenue")),
                col: ColSelector::Range("2024E".into(), "2027E".into()),
            }),
            Expr::Reference(TableRef {
                table: TableSelector::from("IncomeStatement"),
                row: RowSelector::Key(RowId::from("cogs")),
                col: ColSelector::Range("2024E".into(), "2027E".into()),
            }),
            Expr::Omitted,
            Expr::Literal(Literal::Number(0.0)),
            Expr::Literal(Literal::Number(1.0)),
        ],
    };

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    assert_eq!(lowered, "=XLOOKUP(100,$C$6:$F$6,$C$7:$F$7,,0,1)");
}

#[test]
fn test_scenario_14_string_literal_escaping() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");

    // IF(cond, "Domestic \"Main\"", "Export")
    let expr = Expr::FunctionCall {
        name: "if".to_string(),
        args: vec![
            Expr::Literal(Literal::Boolean(true)),
            Expr::Literal(Literal::String("Domestic \"Main\"".to_string())),
            Expr::Literal(Literal::String("Export".to_string())),
        ],
    };

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    assert_eq!(lowered, "=IF(TRUE,\"Domestic \"\"Main\"\"\",\"Export\")");
}

#[test]
fn test_scenario_15_full_compiler_facade_integration() {
    let (manifest, tables) = create_comprehensive_model();

    // 1. Plan layout
    let layout_map = Compiler::plan_layout(&manifest, &tables, None).unwrap();

    // 2. Lower formulas
    let formula_map = Compiler::lower_formulas(&manifest, &tables, &layout_map).unwrap();

    // In IncomeStatement, we added formulas for gross_profit across 4 forecast columns.
    // In CapTable, we added 1 formula for series_a invested_capital.
    // Total formula cells = 4 + 1 = 5.
    assert_eq!(formula_map.len(), 5);

    let gp_2024 = CellId::new("IncomeStatement", "gross_profit", "2024E");
    assert_eq!(formula_map.get(&gp_2024), Some("=C6-C7"));

    let gp_2025 = CellId::new("IncomeStatement", "gross_profit", "2025E");
    assert_eq!(formula_map.get(&gp_2025), Some("=D6-D7"));

    let cap_series_a = CellId::new("CapTable", "series_a", "invested_capital");
    assert_eq!(formula_map.get(&cap_series_a), Some("=B18*C18"));
}

#[test]
fn test_lowering_error_diagnostics() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");

    // 1. Unknown table
    let bad_table_expr = Expr::parse("NonExistentTable[\"revenue\", @col]").unwrap();
    let err = FormulaLowerer::lower_cell(&origin, &bad_table_expr, &symbol_table, &layout_map)
        .unwrap_err();
    assert!(matches!(err, LoweringError::Resolve(_)));

    // 2. Unknown column
    let bad_col_expr = Expr::parse("IncomeStatement[\"revenue\", \"UnknownCol\"]").unwrap();
    let err =
        FormulaLowerer::lower_cell(&origin, &bad_col_expr, &symbol_table, &layout_map).unwrap_err();
    assert!(matches!(err, LoweringError::Resolve(_)));

    // 3. Temporal underflow
    let underflow_origin = CellId::new("BalanceSheet", "cash", "2023A");
    let underflow_expr = Expr::parse("BalanceSheet[\"cash\", col.prev]").unwrap();
    let err = FormulaLowerer::lower_cell(
        &underflow_origin,
        &underflow_expr,
        &symbol_table,
        &layout_map,
    )
    .unwrap_err();
    assert!(matches!(err, LoweringError::Resolve(_)));
}

#[test]
fn test_scenario_16_unary_parenthesization_edge_cases() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");

    // 1. Unary on right-hand side of binary multiplication: A * (-B)
    let expr_mul_neg =
        Expr::parse("IncomeStatement[\"revenue\", @col] * (-IncomeStatement[\"cogs\", @col])")
            .unwrap();
    let lowered =
        FormulaLowerer::lower_cell(&origin, &expr_mul_neg, &symbol_table, &layout_map).unwrap();
    assert_eq!(lowered, "=C6*(-C7)");

    // 2. Unary on right-hand side of division: A / (-10)
    let expr_div_neg = Expr::parse("IncomeStatement[\"revenue\", @col] / (-10)").unwrap();
    let lowered =
        FormulaLowerer::lower_cell(&origin, &expr_div_neg, &symbol_table, &layout_map).unwrap();
    assert_eq!(lowered, "=C6/(-10)");

    // 3. Unary negation wrapping binary power: -(A ^ 2)
    let expr_neg_pow = Expr::parse("-(IncomeStatement[\"revenue\", @col] ^ 2)").unwrap();
    let lowered =
        FormulaLowerer::lower_cell(&origin, &expr_neg_pow, &symbol_table, &layout_map).unwrap();
    assert_eq!(lowered, "=-(C6^2)");

    // 4. Unary negation wrapping binary addition: -(A + B)
    let expr_neg_add =
        Expr::parse("-(IncomeStatement[\"revenue\", @col] + IncomeStatement[\"cogs\", @col])")
            .unwrap();
    let lowered =
        FormulaLowerer::lower_cell(&origin, &expr_neg_add, &symbol_table, &layout_map).unwrap();
    assert_eq!(lowered, "=-(C6+C7)");
}

#[test]
fn test_scenario_17_power_operator_associativity_parenthesization() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");

    // 1. A ^ (B ^ C): Right-hand power operand must wrap in parentheses to prevent Excel's left-to-right evaluation
    let expr_right_pow =
        Expr::parse("IncomeStatement[\"revenue\", @col] ^ (IncomeStatement[\"cogs\", @col] ^ 2)")
            .unwrap();
    let lowered =
        FormulaLowerer::lower_cell(&origin, &expr_right_pow, &symbol_table, &layout_map).unwrap();
    assert_eq!(lowered, "=C6^(C7^2)");

    // 2. (A ^ B) ^ C: Left-hand power operand must also wrap
    let expr_left_pow =
        Expr::parse("(IncomeStatement[\"revenue\", @col] ^ IncomeStatement[\"cogs\", @col]) ^ 2")
            .unwrap();
    let lowered =
        FormulaLowerer::lower_cell(&origin, &expr_left_pow, &symbol_table, &layout_map).unwrap();
    assert_eq!(lowered, "=(C6^C7)^2");
}

#[test]
fn test_scenario_18_dynamic_expanding_range_lowering() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    // IncomeStatement on sheet Model:
    // "revenue" is row 6, "gross_profit" is row 8; "2024E" is col C, "2025E" is col D.
    let origin_2024 = CellId::new("IncomeStatement", "gross_profit", "2024E");
    let origin_2025 = CellId::new("IncomeStatement", "gross_profit", "2025E");

    // 1. Expanding horizontal range "2024E"..@col at 2024E (1-cell anchor range) -> =SUM($C$6:C$6)
    let expr_expand_col =
        Expr::parse("SUM(IncomeStatement[\"revenue\", \"2024E\"..@col])").unwrap();
    assert_eq!(
        FormulaLowerer::lower_cell(&origin_2024, &expr_expand_col, &symbol_table, &layout_map)
            .unwrap(),
        "=SUM($C$6:C$6)"
    );

    // 2. Expanding horizontal range "2024E"..@col at 2025E -> =SUM($C$6:D$6)
    assert_eq!(
        FormulaLowerer::lower_cell(&origin_2025, &expr_expand_col, &symbol_table, &layout_map)
            .unwrap(),
        "=SUM($C$6:D$6)"
    );

    // 3. Prior-period cumulative range "2024E"..col.prev at 2025E -> =SUM($C$6:C$6)
    let expr_prior_col =
        Expr::parse("SUM(IncomeStatement[\"revenue\", \"2024E\"..col.prev])").unwrap();
    assert_eq!(
        FormulaLowerer::lower_cell(&origin_2025, &expr_prior_col, &symbol_table, &layout_map)
            .unwrap(),
        "=SUM($C$6:C$6)"
    );

    // 4. Expanding vertical range "revenue"..@row at ("gross_profit", "2024E") -> =SUM(C$6:C8)
    let expr_expand_row = Expr::parse("SUM(IncomeStatement[\"revenue\"..@row, @col])").unwrap();
    assert_eq!(
        FormulaLowerer::lower_cell(&origin_2024, &expr_expand_row, &symbol_table, &layout_map)
            .unwrap(),
        "=SUM(C$6:C8)"
    );

    // 5. Sliding horizontal window col.prev..@col at 2025E -> =SUM(C$6:D$6)
    let expr_sliding_col =
        Expr::parse("SUM(IncomeStatement[\"revenue\", col.prev..@col])").unwrap();
    assert_eq!(
        FormulaLowerer::lower_cell(&origin_2025, &expr_sliding_col, &symbol_table, &layout_map)
            .unwrap(),
        "=SUM(C$6:D$6)"
    );
}

#[test]
fn test_active_table_selector_lowering() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::default();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin_2024 = CellId::new("IncomeStatement", "gross_profit", "2024E");
    let origin_2025 = CellId::new("IncomeStatement", "gross_profit", "2025E");

    // Scenario 1: Active table scalar, offset, and range lowering matches explicit OriginTable identically
    let comparisons = [
        (
            "@table[\"revenue\", @col] - @table[\"cogs\", @col]",
            "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
            &origin_2024,
        ),
        (
            "@table[\"revenue\", col.prev]",
            "IncomeStatement[\"revenue\", col.prev]",
            &origin_2025,
        ),
        (
            "SUM(@table[\"revenue\"..\"cogs\", @col])",
            "SUM(IncomeStatement[\"revenue\"..\"cogs\", @col])",
            &origin_2024,
        ),
        (
            "SUM(@table[\"revenue\", \"2024E\"..@col])",
            "SUM(IncomeStatement[\"revenue\", \"2024E\"..@col])",
            &origin_2025,
        ),
        (
            "SUM(@table[\"revenue\"..@row, @col])",
            "SUM(IncomeStatement[\"revenue\"..@row, @col])",
            &origin_2024,
        ),
    ];

    for (active_formula, explicit_formula, origin) in comparisons {
        let active_expr = Expr::parse(active_formula).unwrap();
        let explicit_expr = Expr::parse(explicit_formula).unwrap();

        let lowered_active =
            FormulaLowerer::lower_cell(origin, &active_expr, &symbol_table, &layout_map).unwrap();
        let lowered_explicit =
            FormulaLowerer::lower_cell(origin, &explicit_expr, &symbol_table, &layout_map).unwrap();

        assert_eq!(
            lowered_active, lowered_explicit,
            "Active formula '{active_formula}' did not lower identically to '{explicit_formula}'"
        );
    }
}

#[test]
fn test_scenario_19_right_hand_relational_and_float_associativity_parenthesization() {
    use f8_core::compiler::eval::eval_expr;
    use f8_core::expr::ast::BinaryOp;
    use f8_core::functions::registry::FunctionRegistry;
    use f8_core::value::CellValue;
    use indexmap::IndexMap;

    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();
    let registry = FunctionRegistry::standard();
    let memoized = IndexMap::new();

    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");

    // 1. Right-hand relational comparisons wrap in parentheses; left-associated chains do not
    let rel_cases = [
        ("TRUE = (1 = 1)", "=TRUE=(1=1)", CellValue::Boolean(true)),
        ("TRUE = 1 = 1", "=TRUE=1=1", CellValue::Boolean(false)),
        ("1 < (2 < 3)", "=1<(2<3)", CellValue::Boolean(true)),
        ("1 < 2 < 3", "=1<2<3", CellValue::Boolean(false)),
        ("1 <> (2 = 2)", "=1<>(2=2)", CellValue::Boolean(true)),
        ("1 >= (2 >= 3)", "=1>=(2>=3)", CellValue::Boolean(false)),
        ("1 <= (2 >= 3)", "=1<=(2>=3)", CellValue::Boolean(true)),
    ];

    for (formula, expected_excel, expected_val) in rel_cases {
        let expr = Expr::parse(formula).unwrap();
        let eval_res = eval_expr(&expr, &origin, &memoized, &symbol_table, &registry).unwrap();
        assert_eq!(eval_res, expected_val, "Pass 4 eval mismatch on {formula}");

        let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map)
            .expect("lowering must succeed");
        assert_eq!(
            lowered, expected_excel,
            "Pass 6a lowering mismatch on {formula}"
        );
    }

    // 2. Right-hand same-precedence arithmetic preserves grouping; left-heavy chains stay minimal
    let arith_cases = [
        (
            "IncomeStatement[\"revenue\", @col] + IncomeStatement[\"cogs\", @col] + IncomeStatement[\"r_and_d\", @col]",
            "=C6+C7+C9",
        ),
        (
            "IncomeStatement[\"revenue\", @col] + (IncomeStatement[\"cogs\", @col] + IncomeStatement[\"r_and_d\", @col])",
            "=C6+(C7+C9)",
        ),
        (
            "IncomeStatement[\"revenue\", @col] + (IncomeStatement[\"cogs\", @col] - IncomeStatement[\"r_and_d\", @col])",
            "=C6+(C7-C9)",
        ),
        (
            "IncomeStatement[\"revenue\", @col] * IncomeStatement[\"cogs\", @col] * IncomeStatement[\"r_and_d\", @col]",
            "=C6*C7*C9",
        ),
        (
            "IncomeStatement[\"revenue\", @col] * (IncomeStatement[\"cogs\", @col] * IncomeStatement[\"r_and_d\", @col])",
            "=C6*(C7*C9)",
        ),
        (
            "IncomeStatement[\"revenue\", @col] * (IncomeStatement[\"cogs\", @col] / IncomeStatement[\"r_and_d\", @col])",
            "=C6*(C7/C9)",
        ),
    ];

    for (formula, expected_excel) in arith_cases {
        let expr = Expr::parse(formula).unwrap();
        let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map)
            .expect("lowering must succeed");
        assert_eq!(
            lowered, expected_excel,
            "Pass 6a arithmetic grouping mismatch on {formula}"
        );
    }

    // 3. Float overflow & cancellation cases: 1e300 * (1e300 / 1e300) and 1e16 + (-1e16 + 1)
    let big = Expr::number(1e300);
    let mul_div = Expr::Binary {
        op: BinaryOp::Mul,
        left: Box::new(big.clone()),
        right: Box::new(Expr::Binary {
            op: BinaryOp::Div,
            left: Box::new(big.clone()),
            right: Box::new(big),
        }),
    };
    assert_eq!(
        eval_expr(&mul_div, &origin, &memoized, &symbol_table, &registry).unwrap(),
        CellValue::Number(1e300)
    );
    let lowered_mul_div =
        FormulaLowerer::lower_cell(&origin, &mul_div, &symbol_table, &layout_map).unwrap();
    assert!(
        lowered_mul_div.contains("*("),
        "Expected parenthesized right-hand division, got {lowered_mul_div}"
    );

    let cancel_expr = Expr::parse("10000000000000000 + (-10000000000000000 + 1)").unwrap();
    assert_eq!(
        eval_expr(&cancel_expr, &origin, &memoized, &symbol_table, &registry).unwrap(),
        CellValue::Number(0.0)
    );
    assert_eq!(
        FormulaLowerer::lower_cell(&origin, &cancel_expr, &symbol_table, &layout_map).unwrap(),
        "=10000000000000000+(-10000000000000000+1)"
    );

    // 4. Programmatic negative numeric literals respect binary precedence identically to Unary::Neg
    let neg_pow = Expr::Binary {
        op: BinaryOp::Pow,
        left: Box::new(Expr::number(-2.0)),
        right: Box::new(Expr::number(2.0)),
    };
    assert_eq!(
        FormulaLowerer::lower_cell(&origin, &neg_pow, &symbol_table, &layout_map).unwrap(),
        "=(-2)^2"
    );
    let sub_neg = Expr::Binary {
        op: BinaryOp::Sub,
        left: Box::new(Expr::number(1.0)),
        right: Box::new(Expr::number(-2.0)),
    };
    assert_eq!(
        FormulaLowerer::lower_cell(&origin, &sub_neg, &symbol_table, &layout_map).unwrap(),
        "=1-(-2)"
    );
}

#[test]
fn test_scenario_20_range_contiguity_validation_during_lowering() {
    use f8_core::compiler::graph::{RangeDiscontinuity, ResolveError};

    let table = TableSpec {
        id: TableId::from("T"),
        title: "Lowering Contiguity".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::descriptor("line_item", "Line Item"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
            ColumnSpec::gutter(),
            ColumnSpec::descriptor("notes", "Notes"),
            ColumnSpec::new("2026E"),
        ],
        rows: vec![
            RowSpec::new("r1")
                .with_cell("line_item", "A")
                .with_cell("2024E", 10.0)
                .with_cell("2025E", 20.0)
                .with_cell("notes", "n1")
                .with_cell("2026E", 30.0),
            RowSpec::new("r2")
                .with_cell("line_item", "B")
                .with_cell("2024E", 40.0)
                .with_cell("2025E", 50.0)
                .with_cell("notes", "n2")
                .with_cell("2026E", 60.0),
            RowSpec::gutter(),
            RowSpec::new("r3")
                .with_cell("line_item", "C")
                .with_cell("2024E", 70.0)
                .with_cell("2025E", 80.0)
                .with_cell("notes", "n3")
                .with_cell("2026E", 90.0),
            RowSpec::section_header("OpEx"),
            RowSpec::new("r4")
                .with_cell("line_item", "D")
                .with_cell("2024E", 100.0)
                .with_cell("2025E", 110.0)
                .with_cell("notes", "n4")
                .with_cell("2026E", 120.0),
        ],
    };

    let manifest = ProjectManifest {
        model_name: "LoweringContiguityModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Model".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("T")],
        }],
    };

    let theme = Theme::corporate_navy();
    let tables = [table];
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("T", "r1", "2026E");

    // 1. Contiguous measure columns & data rows succeed
    let ok_expr = Expr::parse("SUM(T[\"r1\"..\"r2\", \"2024E\"..\"2025E\"])").unwrap();
    assert_eq!(
        FormulaLowerer::lower_cell(&origin, &ok_expr, &symbol_table, &layout_map).unwrap(),
        "=SUM($C$5:$D$6)"
    );

    // 2. Vertical range on a single descriptor column succeeds
    let ok_desc_vertical = Expr::parse("COUNTA(T[\"r1\"..\"r2\", \"line_item\"])").unwrap();
    assert_eq!(
        FormulaLowerer::lower_cell(&origin, &ok_desc_vertical, &symbol_table, &layout_map).unwrap(),
        "=COUNTA($B$5:$B$6)"
    );

    // 3. Discontinuous row range across gutter
    let bad_row_gutter = Expr::parse("SUM(T[\"r2\"..\"r3\", @col])").unwrap();
    assert!(matches!(
        FormulaLowerer::lower_cell(&origin, &bad_row_gutter, &symbol_table, &layout_map),
        Err(LoweringError::Resolve(ResolveError::DiscontinuousRange {
            discontinuity: RangeDiscontinuity::RowGutter,
            ..
        }))
    ));

    // 4. Discontinuous row range across section header
    let bad_section = Expr::parse("SUM(T[\"r3\"..\"r4\", @col])").unwrap();
    assert!(matches!(
        FormulaLowerer::lower_cell(&origin, &bad_section, &symbol_table, &layout_map),
        Err(LoweringError::Resolve(ResolveError::DiscontinuousRange {
            discontinuity: RangeDiscontinuity::SectionHeader { ref title },
            ..
        })) if title == "OpEx"
    ));

    // 5. Discontinuous col range across column gutter
    let bad_col_gutter = Expr::parse("SUM(T[\"r1\", \"2025E\"..\"2026E\"])").unwrap();
    assert!(matches!(
        FormulaLowerer::lower_cell(&origin, &bad_col_gutter, &symbol_table, &layout_map),
        Err(LoweringError::Resolve(ResolveError::DiscontinuousRange {
            discontinuity: RangeDiscontinuity::ColumnGutter,
            ..
        }))
    ));

    // 6. Discontinuous col range across descriptor endpoint
    let bad_col_desc = Expr::parse("COUNTA(T[\"r1\", \"line_item\"..\"2024E\"])").unwrap();
    assert!(matches!(
        FormulaLowerer::lower_cell(&origin, &bad_col_desc, &symbol_table, &layout_map),
        Err(LoweringError::Resolve(ResolveError::DiscontinuousRange {
            discontinuity: RangeDiscontinuity::DescriptorColumn { ref column },
            ..
        })) if column.as_str() == "line_item"
    ));
}
