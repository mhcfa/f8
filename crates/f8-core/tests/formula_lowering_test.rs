#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::layout::LayoutPlanner;
use f8_core::compiler::lowering::{FormulaLowerer, LoweringError};
use f8_core::compiler::{Compiler, ir};
use f8_core::expr::ast::{
    CallArg, ColSelector, Expr, Literal, RowSelector, TableRef, TableSelector,
};
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
            CallArg::Expr(Expr::Literal(Literal::Number(100.0))),
            CallArg::Expr(Expr::Reference(TableRef {
                table: TableSelector::from("IncomeStatement"),
                row: RowSelector::Key(RowId::from("revenue")),
                col: ColSelector::Range("2024E".into(), "2027E".into()),
            })),
            CallArg::Expr(Expr::Reference(TableRef {
                table: TableSelector::from("IncomeStatement"),
                row: RowSelector::Key(RowId::from("cogs")),
                col: ColSelector::Range("2024E".into(), "2027E".into()),
            })),
            CallArg::Omitted,
            CallArg::Expr(Expr::Literal(Literal::Number(0.0))),
            CallArg::Expr(Expr::Literal(Literal::Number(1.0))),
        ],
    };

    let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map).unwrap();

    assert_eq!(lowered, "=XLOOKUP(100,$C$6:$F$6,$C$7:$F$7,,0,1)");

    // Parsed omitted optional slots round-trip and lower to empty positional slots
    let parsed_xlookup = Expr::parse(
        "XLOOKUP(100, IncomeStatement[\"revenue\", \"2024E\"..\"2027E\"], IncomeStatement[\"cogs\", \"2024E\"..\"2027E\"], _, 0, 1)",
    )
    .unwrap();
    assert_eq!(
        f8_core::compiler::graph::collect_references(&parsed_xlookup).len(),
        2
    );
    assert_eq!(
        FormulaLowerer::lower_cell(&origin, &parsed_xlookup, &symbol_table, &layout_map).unwrap(),
        "=XLOOKUP(100,$C$6:$F$6,$C$7:$F$7,,0,1)"
    );

    let parsed_xlookup_double_skip = Expr::parse(
        "XLOOKUP(100, IncomeStatement[\"revenue\", \"2024E\"..\"2027E\"], IncomeStatement[\"cogs\", \"2024E\"..\"2027E\"], _, _, -1)",
    )
    .unwrap();
    assert_eq!(
        FormulaLowerer::lower_cell(
            &origin,
            &parsed_xlookup_double_skip,
            &symbol_table,
            &layout_map
        )
        .unwrap(),
        "=XLOOKUP(100,$C$6:$F$6,$C$7:$F$7,,,-1)"
    );

    let parsed_pmt_skip = Expr::parse("PMT(0.05, 10, -1000, _, 1)").unwrap();
    assert_eq!(
        FormulaLowerer::lower_cell(&origin, &parsed_pmt_skip, &symbol_table, &layout_map).unwrap(),
        "=PMT(0.05,10,-1000,,1)"
    );

    let parsed_if_two_arg = Expr::parse("IF(FALSE, 10)").unwrap();
    assert_eq!(
        FormulaLowerer::lower_cell(&origin, &parsed_if_two_arg, &symbol_table, &layout_map)
            .unwrap(),
        "=IF(FALSE,10)"
    );

    // Unvalidated trailing CallArg::Omitted is rejected directly by FormulaLowerer (Scenario 6)
    for bad_trailing in [
        "IF(TRUE, 10, _)",
        "ABS(_)",
        "IF(TRUE, 10, XLOOKUP(100, IncomeStatement[\"revenue\", \"2024E\"..\"2027E\"], IncomeStatement[\"cogs\", \"2024E\"..\"2027E\"], _))",
        "PMT(0.05, 10, -1000, _, _)",
    ] {
        let parsed = Expr::parse(bad_trailing).unwrap();
        let err = FormulaLowerer::lower_cell(&origin, &parsed, &symbol_table, &layout_map)
            .expect_err("expected LoweringError::SyntaxError for trailing omitted arg");
        assert!(
            matches!(err, LoweringError::SyntaxError { .. }),
            "expected LoweringError::SyntaxError for '{bad_trailing}', got {err:?}"
        );
    }
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
            CallArg::Expr(Expr::Literal(Literal::Boolean(true))),
            CallArg::Expr(Expr::Literal(Literal::String(
                "Domestic \"Main\"".to_string(),
            ))),
            CallArg::Expr(Expr::Literal(Literal::String("Export".to_string()))),
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

    // 1. Both left-hand and right-hand parenthesized relational subexpressions preserve parentheses
    let rel_cases = [
        ("TRUE = (1 = 1)", "=TRUE=(1=1)", CellValue::Boolean(true)),
        ("(1 = 1) = TRUE", "=(1=1)=TRUE", CellValue::Boolean(true)),
        (
            "FALSE <> (2 < 3)",
            "=FALSE<>(2<3)",
            CellValue::Boolean(true),
        ),
        (
            "(2 >= 3) = (1 > 2)",
            "=(2>=3)=(1>2)",
            CellValue::Boolean(true),
        ),
        (
            "FALSE <= (2 >= 3)",
            "=FALSE<=(2>=3)",
            CellValue::Boolean(true),
        ),
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

    // 1b. ISBLANK and NA() lower directly to standard Excel functions
    let isblank_expr = Expr::parse("ISBLANK(IncomeStatement[\"revenue\", @col])").unwrap();
    assert_eq!(
        FormulaLowerer::lower_cell(&origin, &isblank_expr, &symbol_table, &layout_map).unwrap(),
        "=ISBLANK(C6)"
    );
    let na_expr = Expr::parse("IF(FALSE, 10, NA())").unwrap();
    assert_eq!(
        FormulaLowerer::lower_cell(&origin, &na_expr, &symbol_table, &layout_map).unwrap(),
        "=IF(FALSE,10,NA())"
    );

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
    assert_eq!(lowered_mul_div, "=1E300*(1E300/1E300)");

    let cancel_expr = Expr::parse("10000000000000000 + (-10000000000000000 + 1)").unwrap();
    assert_eq!(
        eval_expr(&cancel_expr, &origin, &memoized, &symbol_table, &registry).unwrap(),
        CellValue::Number(0.0)
    );
    assert_eq!(
        FormulaLowerer::lower_cell(&origin, &cancel_expr, &symbol_table, &layout_map).unwrap(),
        "=1E16+(-1E16+1)"
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

#[test]
fn test_scenario_21_cross_table_relative_offsets_lowering_and_diagnostics() {
    use f8_core::compiler::graph::{ResolveError, resolve_table_ref};
    use f8_core::compiler::lowering::lower_table_ref;

    let history = TableSpec {
        id: TableId::from("History"),
        title: "History".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("2022A"),
            ColumnSpec::new("2023A"),
            ColumnSpec::new("2024E"),
        ],
        rows: vec![
            RowSpec::new("rev")
                .with_cell("2022A", 100.0)
                .with_cell("2023A", 150.0)
                .with_cell("2024E", 200.0),
        ],
    };

    let detail = TableSpec {
        id: TableId::from("Detail"),
        title: "Detail".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("2022A"),
            ColumnSpec::descriptor("notes", "Notes"),
            ColumnSpec::gutter(),
            ColumnSpec::new("2023A"),
            ColumnSpec::new("2024E"),
        ],
        rows: vec![
            RowSpec::new("rev")
                .with_cell("2022A", 80.0)
                .with_cell("notes", "memo")
                .with_cell("2023A", 120.0)
                .with_cell("2024E", 160.0),
        ],
    };

    let schedule = TableSpec {
        id: TableId::from("Schedule"),
        title: "Schedule".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("2024E")],
        rows: vec![
            RowSpec::new("r0").with_cell("2024E", 10.0),
            RowSpec::section_header("Sec"),
            RowSpec::new("r1").with_cell("2024E", 20.0),
            RowSpec::gutter(),
            RowSpec::new("r2").with_cell("2024E", 30.0),
            RowSpec::new("r3").with_cell("2024E", 40.0),
        ],
    };

    let forecast = TableSpec {
        id: TableId::from("Forecast"),
        title: "Forecast".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("2024E"), ColumnSpec::new("2025E")],
        rows: vec![
            RowSpec::new("rev")
                .with_cell("2024E", 200.0)
                .with_cell("2025E", 300.0),
            RowSpec::new("ebitda")
                .with_cell("2024E", 50.0)
                .with_cell("2025E", 75.0),
        ],
    };

    let driver = TableSpec {
        id: TableId::from("DriverTable"),
        title: "Drivers".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("2024E")],
        rows: vec![
            RowSpec::new("r1").with_cell("2024E", 0.0),
            RowSpec::new("r3").with_cell("2024E", 0.0),
        ],
    };

    let assumptions = TableSpec {
        id: TableId::from("Assumptions"),
        title: "Assumptions".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("val")],
        rows: vec![RowSpec::new("tax").with_cell("val", 0.21)],
    };

    let target = TableSpec {
        id: TableId::from("Target"),
        title: "Target".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::descriptor("label", "Label"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
        ],
        rows: vec![
            RowSpec::new("r1")
                .with_cell("label", "Item")
                .with_cell("2024E", 10.0)
                .with_cell("2025E", 20.0),
        ],
    };

    let origin_tbl = TableSpec {
        id: TableId::from("Origin"),
        title: "Origin".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("2023A"),
            ColumnSpec::descriptor("label", "Label"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
        ],
        rows: vec![
            RowSpec::new("r0")
                .with_cell("2023A", 1.0)
                .with_cell("label", "Zero")
                .with_cell("2024E", 2.0)
                .with_cell("2025E", 3.0),
            RowSpec::new("r1")
                .with_cell("2023A", 4.0)
                .with_cell("label", "One")
                .with_cell("2024E", 5.0)
                .with_cell("2025E", 6.0),
        ],
    };

    let history_a = TableSpec {
        id: TableId::from("HistoryA"),
        title: "History A".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("2023A"),
            ColumnSpec::new("label"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
            ColumnSpec::new("2026E"),
        ],
        rows: vec![
            RowSpec::new("r0")
                .with_cell("2023A", 1.0)
                .with_cell("label", 2.0)
                .with_cell("2024E", 3.0)
                .with_cell("2025E", 4.0)
                .with_cell("2026E", 5.0),
            RowSpec::new("r1")
                .with_cell("2023A", 10.0)
                .with_cell("label", 20.0)
                .with_cell("2024E", 30.0)
                .with_cell("2025E", 40.0)
                .with_cell("2026E", 50.0),
            RowSpec::new("r2")
                .with_cell("2023A", 11.0)
                .with_cell("label", 21.0)
                .with_cell("2024E", 31.0)
                .with_cell("2025E", 41.0)
                .with_cell("2026E", 51.0),
        ],
    };

    let manifest = ProjectManifest {
        model_name: "CrossTableLowering".to_string(),
        theme: Default::default(),
        sheets: vec![
            SheetSpec {
                name: "HistSheet".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("History")],
            },
            SheetSpec {
                name: "Main".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![
                    TableId::from("Detail"),
                    TableId::from("Schedule"),
                    TableId::from("Forecast"),
                    TableId::from("DriverTable"),
                    TableId::from("Assumptions"),
                    TableId::from("Target"),
                    TableId::from("Origin"),
                    TableId::from("HistoryA"),
                ],
            },
        ],
    };

    let tables = vec![
        history,
        detail,
        schedule,
        forecast,
        driver,
        assumptions,
        target,
        origin_tbl,
        history_a,
    ];
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    // Scenario 1: Cross-sheet History <-> Forecast boundary offsets & range slices
    let fcst_origin = CellId::new("Forecast", "rev", "2024E");
    assert_eq!(
        FormulaLowerer::lower_cell(
            &fcst_origin,
            &Expr::parse("History[\"rev\", col.prev]").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap(),
        "='HistSheet'!C4"
    );
    assert_eq!(
        FormulaLowerer::lower_cell(
            &fcst_origin,
            &Expr::parse("SUM(History[\"rev\", \"2022A\"..col.prev])").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap(),
        "=SUM('HistSheet'!$B$4:C$4)"
    );

    let hist_origin = CellId::new("History", "rev", "2024E");
    // Forecast on Main: Detail (rows 2..5), gap (6..7), Schedule (rows 8..15), gap (16..17),
    // Forecast (Title 18, gutter 19, rev 20); Col C = 2025E
    assert_eq!(
        FormulaLowerer::lower_cell(
            &hist_origin,
            &Expr::parse("Forecast[\"rev\", col.next]").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap(),
        "='Main'!C20"
    );

    // Scenario 2: Detail target measure stepping skipping descriptor + gutter
    assert_eq!(
        FormulaLowerer::lower_cell(
            &fcst_origin,
            &Expr::parse("Detail[\"rev\", col.prev]").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap(),
        "=E5"
    );
    assert_eq!(
        FormulaLowerer::lower_cell(
            &fcst_origin,
            &Expr::parse("Detail[\"rev\", col.prev(2)]").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap(),
        "=B5"
    );

    // Scenario 3: Schedule target row stepping skipping section header + gutter
    // Schedule on Main: Title (8), gutter (9), r0 (10), Sec (11), r1 (12), gutter (13), r2 (14), r3 (15)
    let drv_r1 = CellId::new("DriverTable", "r1", "2024E");
    let drv_r3 = CellId::new("DriverTable", "r3", "2024E");
    assert_eq!(
        FormulaLowerer::lower_cell(
            &drv_r1,
            &Expr::parse("Schedule[row.prev, @col]").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap(),
        "=B10"
    );
    assert_eq!(
        FormulaLowerer::lower_cell(
            &drv_r3,
            &Expr::parse("Schedule[row.prev, @col]").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap(),
        "=B14"
    );
    assert_eq!(
        FormulaLowerer::lower_cell(
            &drv_r3,
            &Expr::parse("SUM(Schedule[\"r2\"..row.prev, @col])").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap(),
        "=SUM(B$14:B14)"
    );

    // Scenario 4: Active column/row mismatch on scalar and range endpoints during lowering
    let fcst_ebitda = CellId::new("Forecast", "ebitda", "2024E");
    for formula in [
        "Assumptions[\"tax\", col.prev]",
        "SUM(Assumptions[\"tax\", \"val\"..col.prev])",
    ] {
        let err = FormulaLowerer::lower_cell(
            &fcst_ebitda,
            &Expr::parse(formula).unwrap(),
            &symbol_table,
            &layout_map,
        )
        .unwrap_err();
        assert_eq!(
            err,
            LoweringError::Resolve(ResolveError::active_column_mismatch(
                &fcst_ebitda,
                "Assumptions",
                "2024E"
            ))
        );
    }

    for formula in [
        "Assumptions[row.prev, \"val\"]",
        "SUM(Assumptions[\"tax\"..row.prev, \"val\"])",
    ] {
        let err = FormulaLowerer::lower_cell(
            &fcst_ebitda,
            &Expr::parse(formula).unwrap(),
            &symbol_table,
            &layout_map,
        )
        .unwrap_err();
        assert_eq!(
            err,
            LoweringError::Resolve(ResolveError::active_row_mismatch(
                &fcst_ebitda,
                "Assumptions",
                "ebitda"
            ))
        );
    }

    // Scenario 5: Out-of-bounds in Target, descriptor anchor in Target, multi-table disambiguation,
    // and inverted range against leading descriptor
    let orig_2024 = CellId::new("Origin", "r1", "2024E");
    let orig_label = CellId::new("Origin", "r1", "label");
    let orig_2025 = CellId::new("Origin", "r1", "2025E");

    assert_eq!(
        FormulaLowerer::lower_cell(
            &orig_2024,
            &Expr::parse("Target[\"r1\", col.prev]").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap_err(),
        LoweringError::Resolve(ResolveError::temporal_offset_out_of_bounds(
            &orig_2024,
            TableId::from("Target"),
            -1
        ))
    );
    assert_eq!(
        FormulaLowerer::lower_cell(
            &orig_2024,
            &Expr::parse("Target[row.prev, \"2024E\"]").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap_err(),
        LoweringError::Resolve(ResolveError::temporal_offset_out_of_bounds(
            &orig_2024,
            TableId::from("Target"),
            -1
        ))
    );
    assert_eq!(
        FormulaLowerer::lower_cell(
            &orig_label,
            &Expr::parse("Target[\"r1\", col.prev]").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap_err(),
        LoweringError::Resolve(ResolveError::temporal_offset_out_of_bounds(
            &orig_label,
            TableId::from("Target"),
            -1
        ))
    );

    // Multi-table lowering disambiguation where HistoryA succeeds and Target fails
    for (origin_cell, formula, expected_offset) in [
        (
            &orig_2024,
            "HistoryA[\"r1\", col.prev] + Target[\"r1\", col.prev]",
            -1,
        ),
        (
            &orig_2025,
            "HistoryA[\"r1\", col.next] + Target[\"r1\", col.next]",
            1,
        ),
        (
            &orig_2024,
            "HistoryA[row.prev, \"2024E\"] + Target[row.prev, \"2024E\"]",
            -1,
        ),
        (
            &orig_2024,
            "HistoryA[row.next, \"2024E\"] + Target[row.next, \"2024E\"]",
            1,
        ),
        (
            &orig_label,
            "HistoryA[\"r1\", col.prev] + Target[\"r1\", col.prev]",
            -1,
        ),
    ] {
        assert_eq!(
            FormulaLowerer::lower_cell(
                origin_cell,
                &Expr::parse(formula).unwrap(),
                &symbol_table,
                &layout_map
            )
            .unwrap_err(),
            LoweringError::Resolve(ResolveError::temporal_offset_out_of_bounds(
                origin_cell,
                TableId::from("Target"),
                expected_offset
            ))
        );
    }

    assert_eq!(
        FormulaLowerer::lower_cell(
            &orig_2025,
            &Expr::parse("SUM(Target[\"r1\", col.prev..\"label\"])").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap_err(),
        LoweringError::Resolve(ResolveError::inverted_range(
            &orig_2025, "Target", "2024E", "label"
        ))
    );

    // Scenario 6: Unregistered origin table rejected identically in Pass 3 and Pass 6a
    let ghost_origin = CellId::new("GhostTable", "rev", "2024E");
    for target_sel in [TableSelector::from("History"), TableSelector::Active] {
        let table_ref = TableRef::new(
            target_sel,
            RowSelector::Key(RowId::from("rev")),
            ColSelector::Key(ColId::from("2024E")),
        );
        let pass3_err = resolve_table_ref(&ghost_origin, &table_ref, &symbol_table).unwrap_err();
        let pass6_err =
            lower_table_ref(&ghost_origin, &table_ref, &symbol_table, &layout_map).unwrap_err();
        let expected = ResolveError::unregistered_table(&ghost_origin, "GhostTable");
        assert_eq!(
            expected.diagnostic(),
            f8_core::compiler::ErrorDiagnostic::ReferenceResolutionError {
                message: expected.to_string(),
                coordinate: "GhostTable[\"rev\", \"2024E\"]".to_string(),
                target_table: "GhostTable".to_string(),
                row: None,
                col: None,
            }
        );
        assert_eq!(pass3_err, expected);
        assert_eq!(pass6_err, LoweringError::Resolve(expected));
    }
}

#[test]
fn test_cluster_c_typed_criteria_and_error_introspection_lowering() {
    use std::sync::Arc;

    use f8_core::expr::ast::{BlankCheck, CriteriaComparator, RowEndpoint};
    use f8_core::functions::{
        EvalContext, ExcelFunction, FunctionArg, FunctionCategory, FunctionError, FunctionRegistry,
        ParamKind, ParamType, ParameterDef,
    };
    use f8_core::value::{CellValue, ValueType};

    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();
    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");

    let cases = [
        (
            "SUMIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], IncomeStatement[\"revenue\"..\"cogs\", @col], > 15)",
            "=SUMIFS(C$6:C$7,C$6:C$7,\">15\")",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], > -10)",
            "=COUNTIFS(C$6:C$7,\">-10\")",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], <= 0.33333333333333337)",
            "=COUNTIFS(C$6:C$7,\"<=0.333333333333333\")",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], <= IncomeStatement[\"r_and_d\", @col])",
            "=COUNTIFS(C$6:C$7,\"<=\"&C9)",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], > IncomeStatement[\"r_and_d\", @col] + 5)",
            "=COUNTIFS(C$6:C$7,\">\"&(C9+5))",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], <= (1 / 3))",
            "=COUNTIFS(C$6:C$7,\"<=\"&(1/3))",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], > -IncomeStatement[\"r_and_d\", @col])",
            "=COUNTIFS(C$6:C$7,\">\"&(-C9))",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], 10 > 5)",
            "=COUNTIFS(C$6:C$7,10>5)",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], <> TRUE)",
            "=COUNTIFS(C$6:C$7,\"<>TRUE\")",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], <> \"Alpha\")",
            "=COUNTIFS(C$6:C$7,\"<>Alpha\")",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], \"€\")",
            "=COUNTIFS(C$6:C$7,\"€\")",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], \"España\")",
            "=COUNTIFS(C$6:C$7,\"España\")",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], \"東京\")",
            "=COUNTIFS(C$6:C$7,\"東京\")",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], \"café!\")",
            "=COUNTIFS(C$6:C$7,\"café!\")",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], <> \"Note: café\")",
            "=COUNTIFS(C$6:C$7,\"<>Note: café\")",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], BLANK)",
            "=COUNTIFS(C$6:C$7,\"=\")",
        ),
        (
            "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], NOT_BLANK)",
            "=COUNTIFS(C$6:C$7,\"<>\")",
        ),
        (
            "ISERROR(IncomeStatement[\"revenue\", @col] / 0)",
            "=ISERROR(C6/0)",
        ),
        ("ISNA(IncomeStatement[\"revenue\", @col])", "=ISNA(C6)"),
        (
            "ERROR.TYPE(IncomeStatement[\"revenue\", @col] / 0)",
            "=ERROR.TYPE(C6/0)",
        ),
    ];

    for (formula, expected_excel) in cases {
        let expr = Expr::parse(formula).unwrap();
        let lowered = FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map)
            .unwrap_or_else(|e| panic!("lowering failed on '{formula}': {e}"));
        assert_eq!(lowered, expected_excel, "mismatch on '{formula}'");
    }

    // Programmatic Literal::Number(1e-300) comparator formats with E notation ("<=1E-300")
    let tiny_expr = Expr::FunctionCall {
        name: "COUNTIFS".into(),
        args: vec![
            CallArg::Expr(Expr::Reference(TableRef::new(
                TableSelector::from("IncomeStatement"),
                RowSelector::Range(RowEndpoint::from("revenue"), RowEndpoint::from("cogs")),
                ColSelector::Active,
            ))),
            CallArg::Comparator {
                op: CriteriaComparator::Lte,
                expr: Expr::Literal(Literal::Number(1e-300)),
            },
        ],
    };
    assert_eq!(
        FormulaLowerer::lower_cell(&origin, &tiny_expr, &symbol_table, &layout_map).unwrap(),
        "=COUNTIFS(C$6:C$7,\"<=1E-300\")"
    );

    // Programmatic +/-f64::MAX comparator caps at finite 15-sig-digit IEEE-754 decimal ("1.79769313486231E308")
    for (op, val, expected_lit) in [
        (
            CriteriaComparator::Lte,
            f64::MAX,
            "=COUNTIFS(C$6:C$7,\"<=1.79769313486231E308\")",
        ),
        (
            CriteriaComparator::Gte,
            -f64::MAX,
            "=COUNTIFS(C$6:C$7,\">=-1.79769313486231E308\")",
        ),
    ] {
        let extreme_expr = Expr::FunctionCall {
            name: "COUNTIFS".into(),
            args: vec![
                CallArg::Expr(Expr::Reference(TableRef::new(
                    TableSelector::from("IncomeStatement"),
                    RowSelector::Range(RowEndpoint::from("revenue"), RowEndpoint::from("cogs")),
                    ColSelector::Active,
                ))),
                CallArg::Comparator {
                    op,
                    expr: Expr::Literal(Literal::Number(val)),
                },
            ],
        };
        assert_eq!(
            FormulaLowerer::lower_cell(&origin, &extreme_expr, &symbol_table, &layout_map).unwrap(),
            expected_lit
        );
    }

    // Surrogate-pair emoji 255-UTF-16-code-unit boundary in Pass 6a lowering
    let ok_emoji_exact = format!(
        "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], \"{}a\")",
        "😀".repeat(127)
    );
    assert!(
        FormulaLowerer::lower_cell(
            &origin,
            &Expr::parse(&ok_emoji_exact).unwrap(),
            &symbol_table,
            &layout_map
        )
        .is_ok()
    );
    let bad_emoji_exact = format!(
        "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], \"{}\")",
        "😀".repeat(128)
    );
    assert!(matches!(
        FormulaLowerer::lower_cell(
            &origin,
            &Expr::parse(&bad_emoji_exact).unwrap(),
            &symbol_table,
            &layout_map
        ),
        Err(LoweringError::SyntaxError { .. })
    ));

    // Standalone FormulaLowerer rejects criteria outside ParamKind::Criteria, ordering on non-Number literals, and invalid text criteria
    for bad in [
        "ABS(> 5)",
        "IF(> 5, 1, 2)",
        "SUM(BLANK)",
        "SUMIFS(> 5, IncomeStatement[\"revenue\"..\"cogs\", @col], 1)",
        "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], > \"Alpha\")",
        "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], >= TRUE)",
        "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], \"\")",
        "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], \"A*\")",
        "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], \">15\")",
        "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], \"10%\")",
        "COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], <> \"2024-01-01\")",
    ] {
        let expr = Expr::parse(bad).unwrap();
        assert!(
            matches!(
                FormulaLowerer::lower_cell(&origin, &expr, &symbol_table, &layout_map),
                Err(LoweringError::SyntaxError { .. })
            ),
            "expected LoweringError::SyntaxError on '{bad}'"
        );
    }

    // Custom FunctionRegistry with ParamKind::Criteria works in lower_cell_with_registry and compile_with_registry
    static CUSTOM_CRIT_PARAMS: [ParameterDef; 1] = [ParameterDef::required(
        "crit",
        "Custom criteria slot.",
        ParamKind::Criteria,
        ParamType::Any,
    )];
    #[derive(Debug)]
    struct CustomCritFn;
    impl ExcelFunction for CustomCritFn {
        fn name(&self) -> &'static str {
            "CUSTOM.CRIT"
        }
        fn category(&self) -> FunctionCategory {
            FunctionCategory::Math
        }
        fn parameters(&self) -> &'static [ParameterDef] {
            &CUSTOM_CRIT_PARAMS
        }
        fn return_type(&self) -> Option<ValueType> {
            Some(ValueType::Number)
        }
        fn evaluate(
            &self,
            args: &[FunctionArg<'_>],
            _ctx: &dyn EvalContext,
        ) -> Result<CellValue, FunctionError> {
            match args.first() {
                Some(FunctionArg::Criterion(_)) => Ok(CellValue::Number(1.0)),
                Some(
                    FunctionArg::Scalar(_)
                    | FunctionArg::TypedErrorScalar { .. }
                    | FunctionArg::Array { .. }
                    | FunctionArg::Lazy(_)
                    | FunctionArg::Omitted,
                )
                | None => Ok(CellValue::Number(0.0)),
            }
        }
    }

    let mut custom_reg = FunctionRegistry::standard();
    custom_reg.register(Arc::new(CustomCritFn));

    let custom_expr = Expr::FunctionCall {
        name: "CUSTOM.CRIT".into(),
        args: vec![CallArg::BlankCheck(BlankCheck::NotBlank)],
    };
    assert!(FormulaLowerer::lower_cell(&origin, &custom_expr, &symbol_table, &layout_map).is_err());
    assert_eq!(
        FormulaLowerer::lower_cell_with_registry(
            &origin,
            &custom_expr,
            &symbol_table,
            &layout_map,
            &custom_reg
        )
        .unwrap(),
        "=CUSTOM.CRIT(\"<>\")"
    );

    let custom_manifest = ProjectManifest {
        model_name: "CustomCritModel".into(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".into(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("T")],
        }],
    };
    let custom_table = TableSpec {
        id: TableId::from("T"),
        title: "Custom".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: Default::default(),
        columns: vec![ColumnSpec::new("c1").with_header("C1")],
        rows: vec![RowSpec::new("r1").with_expr("CUSTOM.CRIT(> 5)")],
    };
    let compiled =
        Compiler::compile_with_registry(&custom_manifest, &[custom_table], &custom_reg).unwrap();
    assert_eq!(
        compiled
            .cell_value(&TableId::from("T"), &RowId::from("r1"), &ColId::from("c1"))
            .unwrap(),
        CellValue::Number(1.0)
    );
    assert_eq!(
        compiled.lowered_map.get(&CellId::new("T", "r1", "c1")),
        Some("=CUSTOM.CRIT(\">5\")")
    );
}

#[test]
fn test_scientific_notation_and_neg_zero_lowering_and_roundtrip() {
    use indexmap::IndexMap;

    use f8_core::compiler::eval::eval_expr;
    use f8_core::expr::ast::BinaryOp;
    use f8_core::functions::registry::FunctionRegistry;
    use f8_core::value::CellValue;

    let (manifest, tables) = create_comprehensive_model();
    let theme = Theme::corporate_navy();
    let (symbol_table, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout_map = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();
    let registry = FunctionRegistry::standard();
    let memoized = IndexMap::new();
    let origin = CellId::new("IncomeStatement", "gross_profit", "2024E");

    // Scenario 3: Programmatic negative and -0.0 Literal::Number parenthesization & normalization
    let programmatic_cases = [
        (
            Expr::Binary {
                op: BinaryOp::Pow,
                left: Box::new(Expr::number(-2.0)),
                right: Box::new(Expr::number(2.0)),
            },
            "(-2) ^ 2",
            "=(-2)^2",
            4.0,
        ),
        (
            Expr::Binary {
                op: BinaryOp::Sub,
                left: Box::new(Expr::number(1.0)),
                right: Box::new(Expr::number(-2.0)),
            },
            "1 - (-2)",
            "=1-(-2)",
            3.0,
        ),
        (
            Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::number(-2.0)),
                right: Box::new(Expr::number(1.0)),
            },
            "-2 + 1",
            "=-2+1",
            -1.0,
        ),
        (Expr::number(-0.0), "0", "=0", 0.0),
        (
            Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::number(1.0)),
                right: Box::new(Expr::number(-0.0)),
            },
            "1 + 0",
            "=1+0",
            1.0,
        ),
        (
            Expr::Binary {
                op: BinaryOp::Pow,
                left: Box::new(Expr::number(-0.0)),
                right: Box::new(Expr::number(2.0)),
            },
            "0 ^ 2",
            "=0^2",
            0.0,
        ),
        (
            Expr::Binary {
                op: BinaryOp::Sub,
                left: Box::new(Expr::number(1.0)),
                right: Box::new(Expr::number(-0.0)),
            },
            "1 - 0",
            "=1-0",
            1.0,
        ),
    ];

    for (prog_expr, expected_display, expected_excel, expected_num) in programmatic_cases {
        let displayed = prog_expr.to_string();
        assert_eq!(displayed, expected_display);
        let lowered_direct =
            FormulaLowerer::lower_cell(&origin, &prog_expr, &symbol_table, &layout_map).unwrap();
        assert_eq!(lowered_direct, expected_excel);

        let reparsed = Expr::parse(&displayed).unwrap();
        assert_eq!(
            eval_expr(&reparsed, &origin, &memoized, &symbol_table, &registry).unwrap(),
            CellValue::Number(expected_num)
        );
        assert_eq!(
            FormulaLowerer::lower_cell(&origin, &reparsed, &symbol_table, &layout_map).unwrap(),
            expected_excel
        );
    }

    // Scenario 6: [1e-6, 1e15) plain decimal vs < 1e-6 / >= 1e15 lossless uppercase E notation
    let lowering_cases = [
        (0.0_f64, "=0"),
        (-0.0_f64, "=0"),
        (1e-6_f64, "=0.000001"),
        (42.5_f64, "=42.5"),
        (999_999_999_999_999.0_f64, "=999999999999999"),
        (0.333_333_333_333_333_37_f64, "=0.33333333333333337"),
        (1e-9_f64, "=1E-9"),
        (9.999_999_999_999_997e-7_f64, "=9.999999999999997E-7"),
        (1e15_f64, "=1E15"),
        (1e16_f64, "=1E16"),
        (1e300_f64, "=1E300"),
        (1e-300_f64, "=1E-300"),
        (f64::MAX, "=1.7976931348623157E308"),
        (-f64::MAX, "=-1.7976931348623157E308"),
        (f64::MIN_POSITIVE, "=2.2250738585072014E-308"),
        (5e-324_f64, "=5E-324"),
    ];

    for (val, expected_excel) in lowering_cases {
        let num_expr = Expr::number(val);
        let lowered =
            FormulaLowerer::lower_cell(&origin, &num_expr, &symbol_table, &layout_map).unwrap();
        assert_eq!(lowered, expected_excel, "lowering mismatch for {val:?}");

        let reparsed = Expr::parse(&num_expr.to_string()).unwrap();
        let CellValue::Number(eval_num) =
            eval_expr(&reparsed, &origin, &memoized, &symbol_table, &registry).unwrap()
        else {
            panic!("expected Number evaluation for {val:?}");
        };
        let expected_bits = if val == 0.0 {
            0.0_f64.to_bits()
        } else {
            val.to_bits()
        };
        assert_eq!(eval_num.to_bits(), expected_bits);
    }

    // Parsed scientific formulas lower cleanly
    assert_eq!(
        FormulaLowerer::lower_cell(
            &origin,
            &Expr::parse("1e-9 - 2e-9").unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap(),
        "=1E-9-2E-9"
    );
    assert_eq!(
        FormulaLowerer::lower_cell(
            &origin,
            &Expr::parse("COUNTIFS(IncomeStatement[\"revenue\"..\"cogs\", @col], <= 1e-9)")
                .unwrap(),
            &symbol_table,
            &layout_map
        )
        .unwrap(),
        "=COUNTIFS(C$6:C$7,\"<=1E-9\")"
    );
}
