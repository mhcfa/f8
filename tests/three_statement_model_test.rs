#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use indexmap::IndexMap;

use finx::Compiler;
use finx::functions::FunctionRegistry;
use finx::model::format::FormatPreset;
use finx::model::id::{ColId, RowId, TableId};
use finx::model::manifest::{ProjectManifest, SheetSpec};
use finx::model::table::{CellInput, ColumnSpec, RowSpec, TableSpec};
use finx::value::CellValue;

fn build_measure_col(id: &str) -> ColumnSpec {
    ColumnSpec::new(id)
}

fn build_point_row(id: &str, values: &[(&str, f64)]) -> RowSpec {
    let mut cells = IndexMap::new();
    for &(col, val) in values {
        cells.insert(ColId::from(col), CellInput::number(val));
    }
    RowSpec::new(id)
        .with_format(Some(FormatPreset::Number))
        .with_cells(cells)
}

fn build_vector_row(id: &str, expr: &str, overrides: &[(&str, f64)]) -> RowSpec {
    let mut cells = IndexMap::new();
    for &(col, val) in overrides {
        cells.insert(ColId::from(col), CellInput::number(val));
    }
    RowSpec::new(id)
        .with_format(Some(FormatPreset::Number))
        .with_expr(expr)
        .with_cells(cells)
}

#[test]
fn test_simplified_three_statement_financial_model() {
    let timeline_cols = ["y2023a", "y2024e", "y2025e", "y2026e"];

    // 1. Assumptions Table
    let assumptions_table = TableSpec {
        id: TableId::from("Assumptions"),
        title: "Model Assumptions".to_string(),
        columns: vec![build_measure_col("val")],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            build_point_row("tax_rate", &[("val", 0.20)]),
            build_point_row("rev_growth", &[("val", 0.15)]),
            build_point_row("cogs_pct", &[("val", 0.40)]),
            build_point_row("opex_salaries", &[("val", 20.0)]),
            build_point_row("depreciation", &[("val", 10.0)]),
            build_point_row("capex", &[("val", 15.0)]),
            build_point_row("wc_pct_rev", &[("val", 0.10)]),
        ],
    };

    // 2. Income Statement Table
    let is_cols: Vec<ColumnSpec> = timeline_cols
        .iter()
        .map(|&c| build_measure_col(c))
        .collect();
    let income_statement_table = TableSpec {
        id: TableId::from("IncomeStatement"),
        title: "Income Statement".to_string(),
        columns: is_cols,
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            build_vector_row(
                "revenue",
                "IncomeStatement[\"revenue\", col.prev] * (1.0 + Assumptions[\"rev_growth\", \"val\"])",
                &[("y2023a", 100.0)],
            ),
            build_vector_row(
                "cogs",
                "IncomeStatement[\"revenue\", @col] * Assumptions[\"cogs_pct\", \"val\"]",
                &[],
            ),
            build_vector_row(
                "gross_profit",
                "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]",
                &[],
            ),
            build_vector_row("opex", "Assumptions[\"opex_salaries\", \"val\"]", &[]),
            build_vector_row(
                "depreciation",
                "Assumptions[\"depreciation\", \"val\"]",
                &[],
            ),
            build_vector_row(
                "operating_income",
                "IncomeStatement[\"gross_profit\", @col] - IncomeStatement[\"opex\", @col] - IncomeStatement[\"depreciation\", @col]",
                &[],
            ),
            build_vector_row(
                "tax",
                "IncomeStatement[\"operating_income\", @col] * Assumptions[\"tax_rate\", \"val\"]",
                &[],
            ),
            build_vector_row(
                "net_income",
                "IncomeStatement[\"operating_income\", @col] - IncomeStatement[\"tax\", @col]",
                &[],
            ),
        ],
    };

    // 3. Cash Flow Table
    let cf_cols: Vec<ColumnSpec> = timeline_cols
        .iter()
        .map(|&c| build_measure_col(c))
        .collect();
    let cash_flow_table = TableSpec {
        id: TableId::from("CashFlow"),
        title: "Cash Flow Statement".to_string(),
        columns: cf_cols,
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            build_vector_row("net_income", "IncomeStatement[\"net_income\", @col]", &[]),
            build_vector_row(
                "depreciation",
                "Assumptions[\"depreciation\", \"val\"]",
                &[],
            ),
            build_vector_row(
                "change_in_wc",
                "(IncomeStatement[\"revenue\", @col] - IncomeStatement[\"revenue\", col.prev]) * Assumptions[\"wc_pct_rev\", \"val\"]",
                &[("y2023a", 0.0)],
            ),
            build_vector_row(
                "operating_cf",
                "CashFlow[\"net_income\", @col] + CashFlow[\"depreciation\", @col] - CashFlow[\"change_in_wc\", @col]",
                &[],
            ),
            build_vector_row("capex", "Assumptions[\"capex\", \"val\"]", &[]),
            build_vector_row("investing_cf", "0.0 - CashFlow[\"capex\", @col]", &[]),
            build_vector_row(
                "net_cash_flow",
                "CashFlow[\"operating_cf\", @col] + CashFlow[\"investing_cf\", @col]",
                &[],
            ),
            build_vector_row(
                "ending_cash",
                "CashFlow[\"ending_cash\", col.prev] + CashFlow[\"net_cash_flow\", @col]",
                &[("y2023a", 50.0)],
            ),
        ],
    };

    // 4. Balance Sheet Table
    let bs_cols: Vec<ColumnSpec> = timeline_cols
        .iter()
        .map(|&c| build_measure_col(c))
        .collect();
    let balance_sheet_table = TableSpec {
        id: TableId::from("BalanceSheet"),
        title: "Balance Sheet".to_string(),
        columns: bs_cols,
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            // Assets
            build_vector_row("cash", "CashFlow[\"ending_cash\", @col]", &[]),
            build_vector_row(
                "working_capital",
                "IncomeStatement[\"revenue\", @col] * Assumptions[\"wc_pct_rev\", \"val\"]",
                &[],
            ),
            build_vector_row(
                "ppe",
                "BalanceSheet[\"ppe\", col.prev] + Assumptions[\"capex\", \"val\"] - Assumptions[\"depreciation\", \"val\"]",
                &[("y2023a", 40.0)],
            ),
            build_vector_row(
                "total_assets",
                "BalanceSheet[\"cash\", @col] + BalanceSheet[\"working_capital\", @col] + BalanceSheet[\"ppe\", @col]",
                &[],
            ),
            // Liabilities & Equity
            build_vector_row("debt", "0.0", &[]),
            build_vector_row(
                "equity_capital",
                "BalanceSheet[\"equity_capital\", col.prev]",
                &[("y2023a", 76.0)],
            ),
            build_vector_row(
                "retained_earnings",
                "BalanceSheet[\"retained_earnings\", col.prev] + IncomeStatement[\"net_income\", @col]",
                &[("y2023a", 24.0)],
            ),
            build_vector_row(
                "total_liabilities_and_equity",
                "BalanceSheet[\"debt\", @col] + BalanceSheet[\"equity_capital\", @col] + BalanceSheet[\"retained_earnings\", @col]",
                &[],
            ),
            // Balance Check: Total Assets - (Total Liabilities + Equity) == 0.0
            build_vector_row(
                "balance_check",
                "BalanceSheet[\"total_assets\", @col] - BalanceSheet[\"total_liabilities_and_equity\", @col]",
                &[],
            ),
        ],
    };

    let manifest = ProjectManifest {
        model_name: "ThreeStatementFinancialModel".to_string(),
        theme: "classic".to_string(),
        sheets: vec![SheetSpec {
            name: "Model".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![
                PathBuf::from("tables/assumptions.json"),
                PathBuf::from("tables/income_statement.json"),
                PathBuf::from("tables/cash_flow.json"),
                PathBuf::from("tables/balance_sheet.json"),
            ],
        }],
    };

    let tables = vec![
        assumptions_table,
        income_statement_table,
        cash_flow_table,
        balance_sheet_table,
    ];

    // Compile and evaluate via the unified Compiler facade
    let evaluated = Compiler::evaluate(&manifest, &tables)
        .expect("Three-statement financial model must compile and evaluate without errors");

    // Zero domain errors across the entire evaluated workbook
    let domain_errors = evaluated.find_errors();
    assert!(
        domain_errors.is_empty(),
        "Expected zero domain errors, found: {domain_errors:?}"
    );

    let is_id = TableId::from("IncomeStatement");
    let bs_id = TableId::from("BalanceSheet");
    let cf_id = TableId::from("CashFlow");

    // Helper closure to extract number
    let get_num = |t: &TableId, r: &str, c: &str| -> f64 {
        match evaluated.get_value(t, &RowId::from(r), &ColId::from(c)) {
            Some(CellValue::Number(n)) => *n,
            other => panic!("Expected number at {t:?}[{r}, {c}], got {other:?}"),
        }
    };

    // 1. Verify Income Statement revenue compounding
    let rev_2023 = get_num(&is_id, "revenue", "y2023a");
    let rev_2024 = get_num(&is_id, "revenue", "y2024e");
    let rev_2025 = get_num(&is_id, "revenue", "y2025e");
    let rev_2026 = get_num(&is_id, "revenue", "y2026e");

    assert!((rev_2023 - 100.0).abs() < 1e-6);
    assert!((rev_2024 - 115.0).abs() < 1e-6);
    assert!((rev_2025 - 132.25).abs() < 1e-6);
    assert!((rev_2026 - 152.0875).abs() < 1e-6);

    // Verify Net Income values
    let ni_2023 = get_num(&is_id, "net_income", "y2023a");
    let ni_2024 = get_num(&is_id, "net_income", "y2024e");
    let ni_2025 = get_num(&is_id, "net_income", "y2025e");
    let ni_2026 = get_num(&is_id, "net_income", "y2026e");

    assert!((ni_2023 - 24.0).abs() < 1e-6);
    assert!((ni_2024 - 31.2).abs() < 1e-6);
    assert!((ni_2025 - 39.48).abs() < 1e-6);
    assert!((ni_2026 - 49.002).abs() < 1e-6);

    // 2. Verify Cash Flow ending cash
    let cash_2023 = get_num(&cf_id, "ending_cash", "y2023a");
    let cash_2024 = get_num(&cf_id, "ending_cash", "y2024e");
    let cash_2025 = get_num(&cf_id, "ending_cash", "y2025e");
    let cash_2026 = get_num(&cf_id, "ending_cash", "y2026e");

    assert!((cash_2023 - 50.0).abs() < 1e-6);
    assert!((cash_2024 - 74.7).abs() < 1e-6);
    assert!((cash_2025 - 107.455).abs() < 1e-6);
    assert!((cash_2026 - 149.47325).abs() < 1e-6);

    // 3. Verify Balance Sheet balances (Assets == Liabilities + Equity) across ALL periods
    for col in timeline_cols {
        let assets = get_num(&bs_id, "total_assets", col);
        let liab_equity = get_num(&bs_id, "total_liabilities_and_equity", col);
        let balance_check = get_num(&bs_id, "balance_check", col);

        assert!(
            (assets - liab_equity).abs() < 1e-6,
            "Balance sheet out of balance in {col}: assets={assets}, liab_equity={liab_equity}"
        );
        assert!(
            balance_check.abs() < 1e-6,
            "Balance check formula non-zero in {col}: {balance_check}"
        );
    }
}

#[test]
fn test_compiler_facade_error_propagation() {
    // 1. ModelError propagation (duplicate sheet name)
    let duplicate_sheet_manifest = ProjectManifest {
        model_name: "InvalidModel".to_string(),
        theme: "classic".to_string(),
        sheets: vec![
            SheetSpec {
                name: "SameSheet".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![],
            },
            SheetSpec {
                name: "SameSheet".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![],
            },
        ],
    };

    let err = Compiler::evaluate(&duplicate_sheet_manifest, &[])
        .expect_err("Duplicate sheets must trigger Model error");
    assert!(
        matches!(err, finx::CompilerError::Model(_)),
        "Expected CompilerError::Model, got: {err:?}"
    );

    // 2. IrError propagation (manifest references non-existent table)
    let missing_table_manifest = ProjectManifest {
        model_name: "MissingTable".to_string(),
        theme: "classic".to_string(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/non_existent.json")],
        }],
    };

    let err = Compiler::evaluate(&missing_table_manifest, &[])
        .expect_err("Missing table must trigger IR error");
    assert!(
        matches!(err, finx::CompilerError::Ir(_)),
        "Expected CompilerError::Ir, got: {err:?}"
    );

    // 3. GraphError propagation (circular formula dependency)
    let cyclic_table = TableSpec {
        id: TableId::from("CyclicTable"),
        title: "Cyclic Table".to_string(),
        columns: vec![ColumnSpec::new("c1")],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            RowSpec::new("r1").with_expr("CyclicTable[\"r2\", @col]"),
            RowSpec::new("r2").with_expr("CyclicTable[\"r1\", @col]"),
        ],
    };

    let cyclic_manifest = ProjectManifest {
        model_name: "CyclicModel".to_string(),
        theme: "classic".to_string(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/cyclic_table.json")],
        }],
    };

    let err = Compiler::evaluate(&cyclic_manifest, &[cyclic_table])
        .expect_err("Circular dependency must trigger Graph error");
    assert!(
        matches!(err, finx::CompilerError::Graph(_)),
        "Expected CompilerError::Graph, got: {err:?}"
    );

    // 4. EvalError propagation (custom registry missing required function)
    let sum_table = TableSpec {
        id: TableId::from("SumTable"),
        title: "Sum Table".to_string(),
        columns: vec![ColumnSpec::new("c1")],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            RowSpec::new("r1").with_cells({
                let mut cells = IndexMap::new();
                cells.insert(ColId::from("c1"), CellInput::number(42.0));
                cells
            }),
            RowSpec::new("r2").with_expr("SUM(SumTable[\"r1\", @col])"),
        ],
    };

    let sum_manifest = ProjectManifest {
        model_name: "SumModel".to_string(),
        theme: "classic".to_string(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/sum_table.json")],
        }],
    };

    let empty_registry = FunctionRegistry::new();
    let err = Compiler::evaluate_with_registry(&sum_manifest, &[sum_table], &empty_registry)
        .expect_err("Missing function in custom registry must trigger Eval error");
    assert!(
        matches!(err, finx::CompilerError::Eval(_)),
        "Expected CompilerError::Eval, got: {err:?}"
    );
}
