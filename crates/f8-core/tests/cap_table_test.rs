#![allow(clippy::unwrap_used, clippy::expect_used)]

use indexmap::IndexMap;

use f8_core::Compiler;
use f8_core::model::format::FormatPreset;
use f8_core::model::id::{ColId, RowId, TableId};
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{CellInput, ColumnSpec, RowSpec, TableSpec};
use f8_core::value::CellValue;

#[test]
fn test_orthogonal_column_based_cap_table_and_waterfall() {
    let columns = vec![
        ColumnSpec::new("shares").with_format(Some(FormatPreset::Number)),
        ColumnSpec::new("pps").with_format(Some(FormatPreset::Currency)),
        ColumnSpec::new("capital")
            .with_format(Some(FormatPreset::Currency))
            .with_expr("CapTable[@row, \"shares\"] * CapTable[@row, \"pps\"]"),
        ColumnSpec::new("ownership_pct")
            .with_format(Some(FormatPreset::Percentage))
            .with_expr("CapTable[@row, \"shares\"] / CapTable[\"total\", \"shares\"]"),
        ColumnSpec::new("exit_proceeds")
            .with_format(Some(FormatPreset::Currency))
            .with_expr("CapTable[@row, \"shares\"] * Valuation[\"exit_share_price\", \"val\"]"),
    ];

    // Entity rows with point literals for shares and initial PPS
    let make_shareholder_row = |id: &str, shares: f64, pps: f64| -> RowSpec {
        let mut cells = IndexMap::new();
        cells.insert(ColId::from("shares"), CellInput::number(shares));
        cells.insert(ColId::from("pps"), CellInput::number(pps));
        RowSpec::new(id).with_cells(cells)
    };

    // Summary "total" row with point overrides for vertical SUM aggregations
    let mut total_cells = IndexMap::new();
    total_cells.insert(
        ColId::from("shares"),
        CellInput::formula("SUM(CapTable[\"founders\"..\"option_pool\", \"shares\"])"),
    );
    total_cells.insert(ColId::from("pps"), CellInput::number(0.0));
    total_cells.insert(
        ColId::from("capital"),
        CellInput::formula("SUM(CapTable[\"founders\"..\"option_pool\", \"capital\"])"),
    );
    // ownership_pct is handled automatically by column vector rule (total.shares / total.shares = 1.0)
    total_cells.insert(
        ColId::from("exit_proceeds"),
        CellInput::formula("SUM(CapTable[\"founders\"..\"option_pool\", \"exit_proceeds\"])"),
    );

    let total_row = RowSpec::new("total").with_cells(total_cells);

    let cap_table = TableSpec {
        id: TableId::from("CapTable"),
        title: "Capitalization Table".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns,
        rows: vec![
            make_shareholder_row("founders", 4_000_000.0, 0.001),
            make_shareholder_row("seed", 1_000_000.0, 1.00),
            make_shareholder_row("series_a", 3_000_000.0, 2.50),
            make_shareholder_row("option_pool", 2_000_000.0, 0.0),
            total_row,
        ],
    };

    // 2. Valuation Table: Exit valuation scenario
    let mut exit_val_cells = IndexMap::new();
    exit_val_cells.insert(ColId::from("val"), CellInput::number(50_000_000.0));

    let valuation_table = TableSpec {
        id: TableId::from("Valuation"),
        title: "Valuation Scenario".to_string(),
        subtitle: None,
        currency: None,
        collapsible: false,
        columns: vec![ColumnSpec::new("val")],
        rows: vec![
            RowSpec::new("exit_valuation")
                .with_format(Some(FormatPreset::Currency))
                .with_cells(exit_val_cells),
            RowSpec::new("exit_share_price")
                .with_format(Some(FormatPreset::Currency))
                .with_expr(
                    "Valuation[\"exit_valuation\", \"val\"] / CapTable[\"total\", \"shares\"]",
                ),
        ],
    };

    let manifest = ProjectManifest {
        model_name: "CapTableAndExitWaterfall".to_string(),
        theme: "classic".to_string(),
        sheets: vec![SheetSpec {
            name: "Equity".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("CapTable"), TableId::from("Valuation")],
        }],
    };

    let tables = vec![cap_table, valuation_table];

    // Compile and evaluate via the unified Compiler facade
    let evaluated = Compiler::evaluate(&manifest, &tables)
        .expect("Cap table and waterfall model must compile and evaluate without errors");

    assert!(
        evaluated.find_errors().is_empty(),
        "Expected zero domain errors in cap table model"
    );

    let cap_id = TableId::from("CapTable");
    let val_id = TableId::from("Valuation");

    let get_num = |t: &TableId, r: &str, c: &str| -> f64 {
        match evaluated.get_value(t, &RowId::from(r), &ColId::from(c)) {
            Some(CellValue::Number(n)) => *n,
            other => panic!("Expected number at {t:?}[{r}, {c}], got {other:?}"),
        }
    };

    // 1. Verify Total Shares from vertical SUM aggregation
    let total_shares = get_num(&cap_id, "total", "shares");
    assert_eq!(total_shares, 10_000_000.0);

    // 2. Verify Invested Capital (column vector rule: shares * pps)
    let founders_cap = get_num(&cap_id, "founders", "capital");
    let seed_cap = get_num(&cap_id, "seed", "capital");
    let series_a_cap = get_num(&cap_id, "series_a", "capital");
    let option_cap = get_num(&cap_id, "option_pool", "capital");
    let total_cap = get_num(&cap_id, "total", "capital");

    assert!((founders_cap - 4_000.0).abs() < 1e-6);
    assert!((seed_cap - 1_000_000.0).abs() < 1e-6);
    assert!((series_a_cap - 7_500_000.0).abs() < 1e-6);
    assert!((option_cap - 0.0).abs() < 1e-6);
    assert!((total_cap - 8_504_000.0).abs() < 1e-6);

    // 3. Verify Ownership Percentages (column vector rule: shares / total.shares)
    let founders_pct = get_num(&cap_id, "founders", "ownership_pct");
    let seed_pct = get_num(&cap_id, "seed", "ownership_pct");
    let series_a_pct = get_num(&cap_id, "series_a", "ownership_pct");
    let option_pct = get_num(&cap_id, "option_pool", "ownership_pct");
    let total_pct = get_num(&cap_id, "total", "ownership_pct");

    assert!((founders_pct - 0.40).abs() < 1e-6);
    assert!((seed_pct - 0.10).abs() < 1e-6);
    assert!((series_a_pct - 0.30).abs() < 1e-6);
    assert!((option_pct - 0.20).abs() < 1e-6);
    assert!((total_pct - 1.00).abs() < 1e-6);

    // 4. Verify Valuation Table Exit Share Price ($50M / 10M shares = $5.00)
    let exit_share_price = get_num(&val_id, "exit_share_price", "val");
    assert!((exit_share_price - 5.0).abs() < 1e-6);

    // 5. Verify Exit Proceeds (cross-table column vector formula: shares * exit_share_price)
    let founders_proceeds = get_num(&cap_id, "founders", "exit_proceeds");
    let seed_proceeds = get_num(&cap_id, "seed", "exit_proceeds");
    let series_a_proceeds = get_num(&cap_id, "series_a", "exit_proceeds");
    let option_proceeds = get_num(&cap_id, "option_pool", "exit_proceeds");
    let total_proceeds = get_num(&cap_id, "total", "exit_proceeds");

    assert!((founders_proceeds - 20_000_000.0).abs() < 1e-6);
    assert!((seed_proceeds - 5_000_000.0).abs() < 1e-6);
    assert!((series_a_proceeds - 15_000_000.0).abs() < 1e-6);
    assert!((option_proceeds - 10_000_000.0).abs() < 1e-6);
    assert!((total_proceeds - 50_000_000.0).abs() < 1e-6);
}
