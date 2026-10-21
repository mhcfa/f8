#![allow(clippy::unwrap_used, clippy::expect_used)]

use indexmap::IndexMap;

use f8_core::Compiler;
use f8_core::functions::FunctionRegistry;
use f8_core::model::id::{ColId, RowId, TableId};
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{CellInput, ColumnSpec, RowSpec, TableSpec};
use f8_core::value::CellValue;

mod fixtures;

#[test]
fn test_simplified_three_statement_financial_model() {
    let timeline_cols = ["y2023a", "y2024e", "y2025e", "y2026e"];
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();

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
        theme: Default::default(),
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
        matches!(err, f8_core::CompilerError::Model(_)),
        "Expected CompilerError::Model, got: {err:?}"
    );

    // 2. IrError propagation (manifest references non-existent table)
    let missing_table_manifest = ProjectManifest {
        model_name: "MissingTable".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("non_existent")],
        }],
    };

    let err = Compiler::evaluate(&missing_table_manifest, &[])
        .expect_err("Missing table must trigger IR error");
    assert!(
        matches!(err, f8_core::CompilerError::Ir(_)),
        "Expected CompilerError::Ir, got: {err:?}"
    );

    // 3. GraphError propagation (circular formula dependency)
    let cyclic_table = TableSpec {
        id: TableId::from("CyclicTable"),
        title: "Cyclic Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![ColumnSpec::new("c1")],
        rows: vec![
            RowSpec::new("r1").with_expr("CyclicTable[\"r2\", @col]"),
            RowSpec::new("r2").with_expr("CyclicTable[\"r1\", @col]"),
        ],
    };

    let cyclic_manifest = ProjectManifest {
        model_name: "CyclicModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("CyclicTable")],
        }],
    };

    let err = Compiler::evaluate(&cyclic_manifest, &[cyclic_table])
        .expect_err("Circular dependency must trigger Graph error");
    assert!(
        matches!(err, f8_core::CompilerError::Graph(_)),
        "Expected CompilerError::Graph, got: {err:?}"
    );

    // 4. EvalError propagation (custom registry missing required function)
    let sum_table = TableSpec {
        id: TableId::from("SumTable"),
        title: "Sum Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![ColumnSpec::new("c1")],
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
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("SumTable")],
        }],
    };

    let empty_registry = FunctionRegistry::new();
    let err = Compiler::evaluate_with_registry(&sum_manifest, &[sum_table], &empty_registry)
        .expect_err("Missing function in custom registry must trigger Eval error");
    assert!(
        matches!(err, f8_core::CompilerError::Eval(_)),
        "Expected CompilerError::Eval, got: {err:?}"
    );
}
