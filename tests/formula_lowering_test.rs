#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use finx::compiler::ir::id::CellId;
use finx::compiler::layout::LayoutPlanner;
use finx::compiler::lowering::{FormulaLowerer, LoweringError};
use finx::compiler::{Compiler, ir};
use finx::expr::ast::{ColSelector, Expr, Literal, RowSelector, SpecialCol, TableRef};
use finx::model::id::{ColGroupId, ColId, RowGroupId, RowId, TableId};
use finx::model::manifest::{ProjectManifest, SheetSpec};
use finx::model::table::{
    CellInput, ColumnGroup, ColumnSpec, RowGroup, RowGroupChild, RowSpec, TableSpec,
};
use finx::model::theme::Theme;

fn create_comprehensive_model() -> (ProjectManifest, Vec<TableSpec>) {
    // 1. IncomeStatement table
    let is_cols = vec![
        ColumnSpec::new("2023A"),
        ColumnSpec::new("2024E"),
        ColumnSpec::new("2025E"),
        ColumnSpec::new("2026E"),
        ColumnSpec::new("2027E"),
    ];
    let is_col_groups = vec![ColumnGroup {
        id: ColGroupId::from("forecast"),
        label: "Forecast".to_string(),
        columns: vec![
            ColId::from("2024E"),
            ColId::from("2025E"),
            ColId::from("2026E"),
            ColId::from("2027E"),
        ],
    }];
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
            row.cells.insert(col.id.clone(), CellInput::number(100.0));
        }
    }
    // gross_profit = revenue - cogs
    for col in &["2024E", "2025E", "2026E", "2027E"] {
        is_rows[2].cells.insert(
            ColId::from(*col),
            CellInput::formula(
                "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
            ),
        );
    }
    let income_statement = TableSpec {
        id: TableId::from("IncomeStatement"),
        title: "Income Statement".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: is_cols,
        column_groups: is_col_groups,
        row_groups: vec![],
        rows: is_rows,
    };

    // 2. CapTable table
    let cap_cols = vec![
        ColumnSpec::new("shares"),
        ColumnSpec::new("share_price"),
        ColumnSpec::new("invested_capital"),
    ];
    let mut cap_rows = vec![RowSpec::new("seed"), RowSpec::new("series_a")];
    for row in &mut cap_rows {
        for col in &cap_cols {
            row.cells.insert(col.id.clone(), CellInput::number(10.0));
        }
    }
    // series_a invested = shares * share_price
    cap_rows[1].cells.insert(
        ColId::from("invested_capital"),
        CellInput::formula("CapTable[@row, \"shares\"] * CapTable[@row, \"share_price\"]"),
    );
    let cap_table = TableSpec {
        id: TableId::from("CapTable"),
        title: "Cap Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: cap_cols,
        column_groups: vec![],
        row_groups: vec![],
        rows: cap_rows,
    };

    // 3. BalanceSheet table
    let bs_cols = vec![
        ColumnSpec::new("2023A"),
        ColumnSpec::new("2024E"),
        ColumnSpec::new("2025E"),
    ];
    let bs_row_groups = vec![RowGroup {
        id: RowGroupId::from("current_assets"),
        label: "Current Assets".to_string(),
        children: vec![
            RowGroupChild::Row(RowId::from("cash")),
            RowGroupChild::Row(RowId::from("ar")),
            RowGroupChild::Row(RowId::from("inventory")),
        ],
    }];
    let mut bs_rows = vec![
        RowSpec::new("cash"),
        RowSpec::new("ar"),
        RowSpec::new("inventory"),
        RowSpec::new("total_current_assets"),
    ];
    for row in &mut bs_rows {
        for col in &bs_cols {
            row.cells.insert(col.id.clone(), CellInput::number(50.0));
        }
    }
    let balance_sheet = TableSpec {
        id: TableId::from("BalanceSheet"),
        title: "Balance Sheet".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: bs_cols,
        column_groups: vec![],
        row_groups: bs_row_groups,
        rows: bs_rows,
    };

    // 4. CashFlow table
    let cf_cols = vec![
        ColumnSpec::new("2023A"),
        ColumnSpec::new("2024E"),
        ColumnSpec::new("2025E"),
    ];
    let mut cf_rows = vec![RowSpec::new("net_change")];
    for col in &cf_cols {
        cf_rows[0]
            .cells
            .insert(col.id.clone(), CellInput::number(25.0));
    }
    let cash_flow = TableSpec {
        id: TableId::from("CashFlow"),
        title: "Cash Flow Statement".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: cf_cols,
        column_groups: vec![],
        row_groups: vec![],
        rows: cf_rows,
    };

    // 5. DebtSchedule table
    let ds_cols = vec![
        ColumnSpec::new("2023A"),
        ColumnSpec::new("2024E"),
        ColumnSpec::new("2025E"),
    ];
    let ds_col_groups = vec![ColumnGroup {
        id: ColGroupId::from("historical"),
        label: "Historical".to_string(),
        columns: vec![ColId::from("2023A")],
    }];
    let mut ds_rows = vec![
        RowSpec::new("beginning_balance"),
        RowSpec::new("end_balance"),
    ];
    for row in &mut ds_rows {
        for col in &ds_cols {
            row.cells.insert(col.id.clone(), CellInput::number(200.0));
        }
    }
    let debt_schedule = TableSpec {
        id: TableId::from("DebtSchedule"),
        title: "Debt Schedule".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: ds_cols,
        column_groups: ds_col_groups,
        row_groups: vec![],
        rows: ds_rows,
    };

    // 6. Assumptions table on separate sheet
    let assump_cols = vec![ColumnSpec::new("value")];
    let mut assump_rows = vec![RowSpec::new("tax_rate"), RowSpec::new("discount_rate")];
    assump_rows[0]
        .cells
        .insert(ColId::from("value"), CellInput::number(0.21));
    assump_rows[1]
        .cells
        .insert(ColId::from("value"), CellInput::number(0.10));
    let assumptions = TableSpec {
        id: TableId::from("Assumptions"),
        title: "Model Assumptions".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: assump_cols,
        column_groups: vec![],
        row_groups: vec![],
        rows: assump_rows,
    };

    let manifest = ProjectManifest {
        model_name: "FinxModel".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![
            SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![
                    PathBuf::from("tables/IncomeStatement.json"),
                    PathBuf::from("tables/CapTable.json"),
                    PathBuf::from("tables/BalanceSheet.json"),
                    PathBuf::from("tables/CashFlow.json"),
                    PathBuf::from("tables/DebtSchedule.json"),
                ],
            },
            SheetSpec {
                name: "Assumptions".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![PathBuf::from("tables/Assumptions.json")],
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
    let expr = Expr::parse("DebtSchedule[\"beginning_balance\", cols.historical.last]").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // cols.historical.last resolves to 2023A (col B, row 41), which is absolute ($B$41)
    assert_eq!(lowered, "=$B$41");
}

#[test]
fn test_scenario_7_horizontal_range_row_slice() {
    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");
    let expr = Expr::parse("SUM(IncomeStatement[\"revenue\", cols.forecast])").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // cols.forecast covers 2024E..2027E (cols C..F at revenue row 6).
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
    let expr = Expr::parse("SUM(BalanceSheet[rows.current_assets, @col])").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // rows.current_assets contains cash, ar, inventory (rows 25..27).
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
    let expr = Expr::parse("SUM(BalanceSheet[rows.current_assets, \"2024E\"])").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // Specific column 2024E (col C) with rows.current_assets (rows 25..27).
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
    let expr =
        Expr::parse("SUM(IncomeStatement[\"r_and_d\"..\"s_and_m\", cols.forecast])").unwrap();

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    // Rows r_and_d..s_and_m are rows 9..10. cols.forecast are cols C..F.
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
                table: TableId::from("IncomeStatement"),
                row: RowSelector::Key(RowId::from("revenue")),
                col: ColSelector::Special(SpecialCol::Group(ColGroupId::from("forecast"))),
            }),
            Expr::Reference(TableRef {
                table: TableId::from("IncomeStatement"),
                row: RowSelector::Key(RowId::from("cogs")),
                col: ColSelector::Special(SpecialCol::Group(ColGroupId::from("forecast"))),
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
    let theme = Theme::corporate_navy();

    // 1. Plan layout
    let layout_map = Compiler::plan_layout(&manifest, &tables, &theme, None).unwrap();

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
