//! End-to-end integration test suite for an institutional-grade Leveraged Buyout (LBO) model.
//!
//! Serves as Finx's canonical anchoring model benchmark across the compiler pipeline:
//! Pass 4 (Topological Evaluation), Pass 5 (Spatial Layout), and Pass 6 (Formula Lowering & Emission).
//!
//! Specifications defined in `specs/history/2026-09-19-lbo-model-anchoring-integration-test.md`
//! and `specs/living-specs/compiler/evaluator.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use finx::Compiler;
use finx::model::format::FormatPreset;
use finx::model::id::{ColId, RowId, TableId};
use finx::model::manifest::{ProjectManifest, SheetSpec};
use finx::model::table::{AxisRole, CellInput, ColumnSpec, RowSpec, TableSpec};
use finx::value::CellValue;
use indexmap::IndexMap;

fn build_measure_col(id: &str) -> ColumnSpec {
    ColumnSpec::new(id)
}

fn build_point_row(id: &str, values: &[(&str, CellInput)]) -> RowSpec {
    let mut cells = IndexMap::new();
    for &(col, ref input) in values {
        cells.insert(ColId::from(col), input.clone());
    }
    RowSpec {
        id: RowId::from(id),
        role: AxisRole::Measure,
        format: Some(FormatPreset::Number),
        expr: None,
        note: None,
        source: None,
        cells,
    }
}

fn build_vector_row(id: &str, expr: &str, overrides: &[(&str, CellInput)]) -> RowSpec {
    let mut cells = IndexMap::new();
    for &(col, ref input) in overrides {
        cells.insert(ColId::from(col), input.clone());
    }
    RowSpec {
        id: RowId::from(id),
        role: AxisRole::Measure,
        format: Some(FormatPreset::Number),
        expr: Some(expr.to_string()),
        note: None,
        source: None,
        cells,
    }
}

/// Assembles the complete 7-table, 2-sheet institutional LBO model specification.
fn build_lbo_model(active_case: &str) -> (ProjectManifest, Vec<TableSpec>) {
    let timeline_cols = ["y0", "y1", "y2", "y3", "y4", "y5"];

    // -------------------------------------------------------------------------
    // 1. Scenarios Table (Sheet: summary)
    // -------------------------------------------------------------------------
    let scenarios_table = TableSpec {
        id: TableId::from("Scenarios"),
        title: "Operating & Exit Scenarios".to_string(),
        columns: vec![
            build_measure_col("case_id"),
            build_measure_col("rev_growth"),
            build_measure_col("ebitda_margin"),
            build_measure_col("exit_multiple"),
        ],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            build_point_row(
                "base",
                &[
                    ("case_id", CellInput::string("base")),
                    ("rev_growth", CellInput::number(0.08)),
                    ("ebitda_margin", CellInput::number(0.25)),
                    ("exit_multiple", CellInput::number(10.0)),
                ],
            ),
            build_point_row(
                "bull",
                &[
                    ("case_id", CellInput::string("bull")),
                    ("rev_growth", CellInput::number(0.12)),
                    ("ebitda_margin", CellInput::number(0.28)),
                    ("exit_multiple", CellInput::number(12.0)),
                ],
            ),
            build_point_row(
                "bear",
                &[
                    ("case_id", CellInput::string("bear")),
                    ("rev_growth", CellInput::number(0.03)),
                    ("ebitda_margin", CellInput::number(0.20)),
                    ("exit_multiple", CellInput::number(8.0)),
                ],
            ),
        ],
    };

    // -------------------------------------------------------------------------
    // 2. DealAssumptions Table (Sheet: summary)
    // -------------------------------------------------------------------------
    let deal_assumptions_table = TableSpec {
        id: TableId::from("DealAssumptions"),
        title: "Transaction & Financing Assumptions".to_string(),
        columns: vec![build_measure_col("val")],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            // Scenario Selection
            build_point_row("active_case", &[("val", CellInput::string(active_case))]),
            build_point_row(
                "rev_growth",
                &[(
                    "val",
                    CellInput::formula(
                        "XLOOKUP(DealAssumptions[\"active_case\", \"val\"], Scenarios[\"base\"..\"bear\", \"case_id\"], Scenarios[\"base\"..\"bear\", \"rev_growth\"])",
                    ),
                )],
            ),
            build_point_row(
                "ebitda_margin",
                &[(
                    "val",
                    CellInput::formula(
                        "XLOOKUP(DealAssumptions[\"active_case\", \"val\"], Scenarios[\"base\"..\"bear\", \"case_id\"], Scenarios[\"base\"..\"bear\", \"ebitda_margin\"])",
                    ),
                )],
            ),
            build_point_row(
                "exit_multiple",
                &[(
                    "val",
                    CellInput::formula(
                        "XLOOKUP(DealAssumptions[\"active_case\", \"val\"], Scenarios[\"base\"..\"bear\", \"case_id\"], Scenarios[\"base\"..\"bear\", \"exit_multiple\"])",
                    ),
                )],
            ),
            // Valuation & Leverage Drivers
            build_point_row("entry_ebitda", &[("val", CellInput::number(50.0))]),
            build_point_row("entry_multiple", &[("val", CellInput::number(10.0))]),
            build_point_row("senior_leverage", &[("val", CellInput::number(3.0))]),
            build_point_row("sub_leverage", &[("val", CellInput::number(1.5))]),
            build_point_row("senior_rate", &[("val", CellInput::number(0.08))]),
            build_point_row("sub_rate", &[("val", CellInput::number(0.12))]),
            build_point_row("tax_rate", &[("val", CellInput::number(0.25))]),
            build_point_row("capex_pct", &[("val", CellInput::number(0.05))]),
            build_point_row("discount_rate", &[("val", CellInput::number(0.10))]),
            // Sources & Uses Calculations
            build_point_row(
                "entry_ev",
                &[(
                    "val",
                    CellInput::formula(
                        "DealAssumptions[\"entry_ebitda\", \"val\"] * DealAssumptions[\"entry_multiple\", \"val\"]",
                    ),
                )],
            ),
            build_point_row(
                "senior_debt_entry",
                &[(
                    "val",
                    CellInput::formula(
                        "DealAssumptions[\"entry_ebitda\", \"val\"] * DealAssumptions[\"senior_leverage\", \"val\"]",
                    ),
                )],
            ),
            build_point_row(
                "sub_debt_entry",
                &[(
                    "val",
                    CellInput::formula(
                        "DealAssumptions[\"entry_ebitda\", \"val\"] * DealAssumptions[\"sub_leverage\", \"val\"]",
                    ),
                )],
            ),
            build_point_row(
                "total_debt_entry",
                &[(
                    "val",
                    CellInput::formula(
                        "DealAssumptions[\"senior_debt_entry\", \"val\"] + DealAssumptions[\"sub_debt_entry\", \"val\"]",
                    ),
                )],
            ),
            build_point_row(
                "sponsor_equity_entry",
                &[(
                    "val",
                    CellInput::formula(
                        "DealAssumptions[\"entry_ev\", \"val\"] - DealAssumptions[\"total_debt_entry\", \"val\"]",
                    ),
                )],
            ),
        ],
    };

    // -------------------------------------------------------------------------
    // 3. Timeline Table (Sheet: model)
    // -------------------------------------------------------------------------
    let timeline_table = TableSpec {
        id: TableId::from("Timeline"),
        title: "Investment Timeline".to_string(),
        columns: timeline_cols
            .iter()
            .map(|&c| build_measure_col(c))
            .collect(),
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            build_vector_row(
                "date",
                "EDATE(Timeline[\"date\", col.prev], 12)",
                &[("y0", CellInput::formula("DATE(2024, 12, 31)"))],
            ),
            build_vector_row("year", "YEAR(Timeline[\"date\", @col])", &[]),
            build_vector_row(
                "days",
                "DAYS(Timeline[\"date\", @col], Timeline[\"date\", col.prev])",
                &[("y0", CellInput::number(0.0))],
            ),
            build_vector_row(
                "year_frac",
                "YEARFRAC(Timeline[\"date\", col.prev], Timeline[\"date\", @col], 0)",
                &[("y0", CellInput::number(0.0))],
            ),
        ],
    };

    // -------------------------------------------------------------------------
    // 4. Operations Table (Sheet: model)
    // -------------------------------------------------------------------------
    let operations_table = TableSpec {
        id: TableId::from("Operations"),
        title: "Operating Forecast & Cash Flows".to_string(),
        columns: timeline_cols
            .iter()
            .map(|&c| build_measure_col(c))
            .collect(),
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            build_vector_row(
                "revenue",
                "Operations[\"revenue\", col.prev] * (1 + DealAssumptions[\"rev_growth\", \"val\"])",
                &[("y0", CellInput::number(200.0))],
            ),
            build_vector_row(
                "ebitda",
                "Operations[\"revenue\", @col] * DealAssumptions[\"ebitda_margin\", \"val\"]",
                &[("y0", CellInput::number(50.0))],
            ),
            build_vector_row(
                "depreciation",
                "Operations[\"revenue\", @col] * 0.04",
                &[("y0", CellInput::number(8.0))],
            ),
            build_vector_row(
                "ebit",
                "Operations[\"ebitda\", @col] - Operations[\"depreciation\", @col]",
                &[],
            ),
            build_vector_row(
                "interest_expense",
                "DebtSchedule[\"total_interest\", @col]",
                &[("y0", CellInput::number(0.0))],
            ),
            build_vector_row(
                "ebt",
                "Operations[\"ebit\", @col] - Operations[\"interest_expense\", @col]",
                &[],
            ),
            build_vector_row(
                "tax",
                "IF(Operations[\"ebt\", @col] > 0, Operations[\"ebt\", @col] * DealAssumptions[\"tax_rate\", \"val\"], 0)",
                &[("y0", CellInput::number(0.0))],
            ),
            build_vector_row(
                "net_income",
                "Operations[\"ebt\", @col] - Operations[\"tax\", @col]",
                &[],
            ),
            build_vector_row(
                "capex",
                "Operations[\"revenue\", @col] * DealAssumptions[\"capex_pct\", \"val\"]",
                &[("y0", CellInput::number(0.0))],
            ),
            build_vector_row(
                "fcf_pre_debt",
                "Operations[\"ebitda\", @col] - Operations[\"tax\", @col] - Operations[\"capex\", @col]",
                &[("y0", CellInput::number(0.0))],
            ),
        ],
    };

    // -------------------------------------------------------------------------
    // 5. DebtSchedule Table (Sheet: model)
    // -------------------------------------------------------------------------
    let debt_schedule_table = TableSpec {
        id: TableId::from("DebtSchedule"),
        title: "Debt Schedule & Cash Sweep".to_string(),
        columns: timeline_cols
            .iter()
            .map(|&c| build_measure_col(c))
            .collect(),
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            build_vector_row(
                "senior_begin",
                "DebtSchedule[\"senior_end\", col.prev]",
                &[("y0", CellInput::number(0.0))],
            ),
            build_vector_row(
                "senior_interest",
                "DebtSchedule[\"senior_begin\", @col] * DealAssumptions[\"senior_rate\", \"val\"] * Timeline[\"year_frac\", @col]",
                &[("y0", CellInput::number(0.0))],
            ),
            build_vector_row(
                "sub_begin",
                "DebtSchedule[\"sub_end\", col.prev]",
                &[("y0", CellInput::number(0.0))],
            ),
            build_vector_row(
                "sub_interest",
                "DebtSchedule[\"sub_begin\", @col] * DealAssumptions[\"sub_rate\", \"val\"] * Timeline[\"year_frac\", @col]",
                &[("y0", CellInput::number(0.0))],
            ),
            build_vector_row(
                "total_interest",
                "DebtSchedule[\"senior_interest\", @col] + DebtSchedule[\"sub_interest\", @col]",
                &[("y0", CellInput::number(0.0))],
            ),
            build_vector_row(
                "senior_mandatory",
                "MIN(DebtSchedule[\"senior_begin\", @col], DealAssumptions[\"senior_debt_entry\", \"val\"] * 0.05)",
                &[("y0", CellInput::number(0.0))],
            ),
            build_vector_row(
                "cash_available_sweep",
                "MAX(0, Operations[\"fcf_pre_debt\", @col] - DebtSchedule[\"total_interest\", @col] - DebtSchedule[\"senior_mandatory\", @col])",
                &[("y0", CellInput::number(0.0))],
            ),
            build_vector_row(
                "senior_sweep",
                "MIN(DebtSchedule[\"cash_available_sweep\", @col], DebtSchedule[\"senior_begin\", @col] - DebtSchedule[\"senior_mandatory\", @col])",
                &[("y0", CellInput::number(0.0))],
            ),
            build_vector_row(
                "senior_end",
                "DebtSchedule[\"senior_begin\", @col] - DebtSchedule[\"senior_mandatory\", @col] - DebtSchedule[\"senior_sweep\", @col]",
                &[(
                    "y0",
                    CellInput::formula("DealAssumptions[\"senior_debt_entry\", \"val\"]"),
                )],
            ),
            build_vector_row(
                "sub_end",
                "DebtSchedule[\"sub_begin\", @col]",
                &[(
                    "y0",
                    CellInput::formula("DealAssumptions[\"sub_debt_entry\", \"val\"]"),
                )],
            ),
            build_vector_row(
                "total_debt_end",
                "DebtSchedule[\"senior_end\", @col] + DebtSchedule[\"sub_end\", @col]",
                &[],
            ),
        ],
    };

    // -------------------------------------------------------------------------
    // 6. CashFlows Table (Sheet: model)
    // -------------------------------------------------------------------------
    let cash_flows_table = TableSpec {
        id: TableId::from("CashFlows"),
        title: "Sponsor Equity Cash Flows".to_string(),
        columns: timeline_cols
            .iter()
            .map(|&c| build_measure_col(c))
            .collect(),
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![build_point_row(
            "sponsor_cf",
            &[
                (
                    "y0",
                    CellInput::formula("-DealAssumptions[\"sponsor_equity_entry\", \"val\"]"),
                ),
                ("y1", CellInput::number(0.0)),
                ("y2", CellInput::number(0.0)),
                ("y3", CellInput::number(0.0)),
                ("y4", CellInput::number(0.0)),
                (
                    "y5",
                    CellInput::formula("ReturnsSummary[\"exit_equity_proceeds\", \"val\"]"),
                ),
            ],
        )],
    };

    // -------------------------------------------------------------------------
    // 7. ReturnsSummary Table (Sheet: summary)
    // -------------------------------------------------------------------------
    let returns_summary_table = TableSpec {
        id: TableId::from("ReturnsSummary"),
        title: "Exit Valuation & Sponsor Returns".to_string(),
        columns: vec![build_measure_col("val")],
        column_groups: vec![],
        row_groups: vec![],
        rows: vec![
            build_point_row(
                "exit_year",
                &[(
                    "val",
                    CellInput::formula("YEAR(Timeline[\"date\", \"y5\"])"),
                )],
            ),
            build_point_row(
                "exit_multiple",
                &[(
                    "val",
                    CellInput::formula("DealAssumptions[\"exit_multiple\", \"val\"]"),
                )],
            ),
            build_point_row(
                "exit_ebitda",
                &[("val", CellInput::formula("Operations[\"ebitda\", \"y5\"]"))],
            ),
            build_point_row(
                "exit_ev",
                &[(
                    "val",
                    CellInput::formula(
                        "ReturnsSummary[\"exit_ebitda\", \"val\"] * ReturnsSummary[\"exit_multiple\", \"val\"]",
                    ),
                )],
            ),
            build_point_row(
                "exit_net_debt",
                &[(
                    "val",
                    CellInput::formula("DebtSchedule[\"total_debt_end\", \"y5\"]"),
                )],
            ),
            build_point_row(
                "exit_equity_proceeds",
                &[(
                    "val",
                    CellInput::formula(
                        "ReturnsSummary[\"exit_ev\", \"val\"] - ReturnsSummary[\"exit_net_debt\", \"val\"]",
                    ),
                )],
            ),
            build_point_row(
                "initial_equity",
                &[(
                    "val",
                    CellInput::formula("DealAssumptions[\"sponsor_equity_entry\", \"val\"]"),
                )],
            ),
            build_point_row(
                "moic",
                &[(
                    "val",
                    CellInput::formula(
                        "ReturnsSummary[\"exit_equity_proceeds\", \"val\"] / ReturnsSummary[\"initial_equity\", \"val\"]",
                    ),
                )],
            ),
            build_point_row(
                "irr",
                &[(
                    "val",
                    CellInput::formula(
                        "XIRR(CashFlows[\"sponsor_cf\", \"y0\"..\"y5\"], Timeline[\"date\", \"y0\"..\"y5\"])",
                    ),
                )],
            ),
            build_point_row(
                "npv",
                &[(
                    "val",
                    CellInput::formula(
                        "XNPV(DealAssumptions[\"discount_rate\", \"val\"], CashFlows[\"sponsor_cf\", \"y0\"..\"y5\"], Timeline[\"date\", \"y0\"..\"y5\"])",
                    ),
                )],
            ),
        ],
    };

    // -------------------------------------------------------------------------
    // Project Manifest (Sheets: Summary and Model)
    // -------------------------------------------------------------------------
    let manifest = ProjectManifest {
        model_name: "ProjectTitanLBO".to_string(),
        theme: "classic".to_string(),
        sheets: vec![
            SheetSpec {
                name: "Summary".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![
                    std::path::PathBuf::from("tables/scenarios.json"),
                    std::path::PathBuf::from("tables/deal_assumptions.json"),
                    std::path::PathBuf::from("tables/returns_summary.json"),
                ],
            },
            SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![
                    std::path::PathBuf::from("tables/timeline.json"),
                    std::path::PathBuf::from("tables/operations.json"),
                    std::path::PathBuf::from("tables/debt_schedule.json"),
                    std::path::PathBuf::from("tables/cash_flows.json"),
                ],
            },
        ],
    };

    let tables = vec![
        scenarios_table,
        deal_assumptions_table,
        returns_summary_table,
        timeline_table,
        operations_table,
        debt_schedule_table,
        cash_flows_table,
    ];

    (manifest, tables)
}

#[test]
fn test_institutional_lbo_base_case() {
    let timeline_cols = ["y0", "y1", "y2", "y3", "y4", "y5"];
    let (manifest, tables) = build_lbo_model("base");

    let evaluated = Compiler::evaluate(&manifest, &tables)
        .expect("LBO Base Case model must compile and evaluate cleanly without errors");

    // 1. Verify zero evaluation errors across the entire model
    let errors = evaluated.find_errors();
    assert!(
        errors.is_empty(),
        "Model should evaluate with zero errors, found: {errors:?}"
    );

    // 2. Scenario & Deal Assumptions Verification
    let get_val = |table: &str, row: &str, col: &str| -> f64 {
        let t_id = TableId::from(table);
        let r_id = RowId::from(row);
        let c_id = ColId::from(col);
        match evaluated.get_value(&t_id, &r_id, &c_id) {
            Some(CellValue::Number(n)) => *n,
            other => panic!("Expected number for {table}[{row}, {col}], got {other:?}"),
        }
    };

    assert_eq!(get_val("DealAssumptions", "rev_growth", "val"), 0.08);
    assert_eq!(get_val("DealAssumptions", "ebitda_margin", "val"), 0.25);
    assert_eq!(get_val("DealAssumptions", "exit_multiple", "val"), 10.0);
    assert_eq!(get_val("DealAssumptions", "entry_ev", "val"), 500.0);
    assert_eq!(
        get_val("DealAssumptions", "senior_debt_entry", "val"),
        150.0
    );
    assert_eq!(get_val("DealAssumptions", "sub_debt_entry", "val"), 75.0);
    assert_eq!(get_val("DealAssumptions", "total_debt_entry", "val"), 225.0);
    assert_eq!(
        get_val("DealAssumptions", "sponsor_equity_entry", "val"),
        275.0
    );

    // 3. Timeline & Calendar Day-Count Fidelity
    // 2024-12-31 is serial 45657.0
    assert_eq!(get_val("Timeline", "date", "y0"), 45657.0);
    // EDATE +12 months each period
    assert_eq!(get_val("Timeline", "date", "y1"), 46022.0); // 2025-12-31
    assert_eq!(get_val("Timeline", "date", "y2"), 46387.0); // 2026-12-31
    assert_eq!(get_val("Timeline", "date", "y3"), 46752.0); // 2027-12-31
    assert_eq!(get_val("Timeline", "date", "y4"), 47118.0); // 2028-12-31 (leap year!)
    assert_eq!(get_val("Timeline", "date", "y5"), 47483.0); // 2029-12-31

    assert_eq!(get_val("Timeline", "year", "y0"), 2024.0);
    assert_eq!(get_val("Timeline", "year", "y5"), 2029.0);

    // Days difference (leap year 2028 has 366 days)
    assert_eq!(get_val("Timeline", "days", "y1"), 365.0);
    assert_eq!(get_val("Timeline", "days", "y2"), 365.0);
    assert_eq!(get_val("Timeline", "days", "y3"), 365.0);
    assert_eq!(get_val("Timeline", "days", "y4"), 366.0);
    assert_eq!(get_val("Timeline", "days", "y5"), 365.0);

    // Contractual year fraction under US 30/360 NASD is 1.0 for each full year
    for col in ["y1", "y2", "y3", "y4", "y5"] {
        assert_eq!(get_val("Timeline", "year_frac", col), 1.0);
    }

    // 4. Operating Forecast Verification
    let rev_y0 = get_val("Operations", "revenue", "y0");
    assert_eq!(rev_y0, 200.0);
    let rev_y1 = get_val("Operations", "revenue", "y1");
    assert!((rev_y1 - 216.0).abs() < 1e-6);
    let rev_y5 = get_val("Operations", "revenue", "y5");
    assert!((rev_y5 - (200.0 * 1.08_f64.powi(5))).abs() < 1e-6);

    let ebitda_y1 = get_val("Operations", "ebitda", "y1");
    assert!((ebitda_y1 - 54.0).abs() < 1e-6);

    // Positive taxes generated every forecast year
    for col in ["y1", "y2", "y3", "y4", "y5"] {
        assert!(get_val("Operations", "tax", col) > 0.0);
        assert!(get_val("Operations", "net_income", col) > 0.0);
        assert!(get_val("Operations", "fcf_pre_debt", col) > 0.0);
    }

    // 5. Debt Schedule & Cash Sweep Verification
    assert_eq!(get_val("DebtSchedule", "senior_begin", "y1"), 150.0);
    assert_eq!(get_val("DebtSchedule", "senior_interest", "y1"), 12.0); // 150 * 0.08 * 1.0
    assert_eq!(get_val("DebtSchedule", "sub_begin", "y1"), 75.0);
    assert_eq!(get_val("DebtSchedule", "sub_interest", "y1"), 9.0); // 75 * 0.12 * 1.0
    assert_eq!(get_val("DebtSchedule", "total_interest", "y1"), 21.0);

    // 5% mandatory senior debt repayment = 7.5
    assert_eq!(get_val("DebtSchedule", "senior_mandatory", "y1"), 7.5);
    // Cash sweep voluntarily prepays senior debt
    let sweep_y1 = get_val("DebtSchedule", "senior_sweep", "y1");
    assert!(sweep_y1 > 0.0);
    assert_eq!(
        get_val("DebtSchedule", "senior_end", "y1"),
        150.0 - 7.5 - sweep_y1
    );
    // Sub debt remains constant bullet balance of 75.0
    for col in ["y0", "y1", "y2", "y3", "y4", "y5"] {
        assert_eq!(get_val("DebtSchedule", "sub_end", col), 75.0);
    }

    // Ending debt decreases monotonically
    for i in 1..timeline_cols.len() {
        let prev_debt = get_val("DebtSchedule", "total_debt_end", timeline_cols[i - 1]);
        let curr_debt = get_val("DebtSchedule", "total_debt_end", timeline_cols[i]);
        assert!(
            curr_debt < prev_debt,
            "Total debt must pay down over time: {curr_debt} < {prev_debt}"
        );
    }

    // 6. Cash Flows & Returns Verification
    assert_eq!(get_val("CashFlows", "sponsor_cf", "y0"), -275.0);
    for col in ["y1", "y2", "y3", "y4"] {
        assert_eq!(get_val("CashFlows", "sponsor_cf", col), 0.0);
    }
    let exit_equity = get_val("ReturnsSummary", "exit_equity_proceeds", "val");
    assert_eq!(get_val("CashFlows", "sponsor_cf", "y5"), exit_equity);

    assert_eq!(get_val("ReturnsSummary", "exit_year", "val"), 2029.0);
    assert_eq!(get_val("ReturnsSummary", "exit_multiple", "val"), 10.0);
    let exit_ebitda = get_val("ReturnsSummary", "exit_ebitda", "val");
    assert!((exit_ebitda - get_val("Operations", "ebitda", "y5")).abs() < 1e-6);

    let exit_ev = get_val("ReturnsSummary", "exit_ev", "val");
    assert!((exit_ev - (exit_ebitda * 10.0)).abs() < 1e-6);

    let exit_net_debt = get_val("ReturnsSummary", "exit_net_debt", "val");
    assert_eq!(
        exit_net_debt,
        get_val("DebtSchedule", "total_debt_end", "y5")
    );
    assert!((exit_equity - (exit_ev - exit_net_debt)).abs() < 1e-6);

    // MoIC check (expected ~2.2x to 2.4x)
    let moic = get_val("ReturnsSummary", "moic", "val");
    assert!(
        moic > 2.0 && moic < 2.5,
        "Base Case MoIC should be between 2.0x and 2.5x, got {moic}"
    );

    // IRR check via XIRR (expected ~17% to 19% annualized)
    let irr = get_val("ReturnsSummary", "irr", "val");
    assert!(
        irr > 0.16 && irr < 0.20,
        "Base Case XIRR should be between 16% and 20%, got {irr}"
    );

    // NPV check via XNPV at 10% discount rate (expected positive value)
    let npv = get_val("ReturnsSummary", "npv", "val");
    assert!(
        npv > 50.0,
        "Base Case XNPV at 10% discount rate should be strongly positive, got {npv}"
    );
}

#[test]
fn test_lbo_scenario_sensitivity_bull_and_bear() {
    // 1. Evaluate Bull Case
    let (manifest_bull, tables_bull) = build_lbo_model("bull");
    let eval_bull =
        Compiler::evaluate(&manifest_bull, &tables_bull).expect("Bull Case must compile cleanly");
    assert!(eval_bull.find_errors().is_empty());

    let get_val_bull = |table: &str, row: &str, col: &str| -> f64 {
        let t_id = TableId::from(table);
        let r_id = RowId::from(row);
        let c_id = ColId::from(col);
        match eval_bull.get_value(&t_id, &r_id, &c_id) {
            Some(CellValue::Number(n)) => *n,
            other => panic!("Expected number, got {other:?}"),
        }
    };

    assert_eq!(get_val_bull("DealAssumptions", "rev_growth", "val"), 0.12);
    assert_eq!(
        get_val_bull("DealAssumptions", "ebitda_margin", "val"),
        0.28
    );
    assert_eq!(
        get_val_bull("DealAssumptions", "exit_multiple", "val"),
        12.0
    );

    let moic_bull = get_val_bull("ReturnsSummary", "moic", "val");
    let irr_bull = get_val_bull("ReturnsSummary", "irr", "val");
    assert!(
        moic_bull > 3.0,
        "Bull Case MoIC should exceed 3.0x, got {moic_bull}"
    );
    assert!(
        irr_bull > 0.25,
        "Bull Case IRR should exceed 25%, got {irr_bull}"
    );

    // 2. Evaluate Bear Case
    let (manifest_bear, tables_bear) = build_lbo_model("bear");
    let eval_bear =
        Compiler::evaluate(&manifest_bear, &tables_bear).expect("Bear Case must compile cleanly");
    assert!(eval_bear.find_errors().is_empty());

    let get_val_bear = |table: &str, row: &str, col: &str| -> f64 {
        let t_id = TableId::from(table);
        let r_id = RowId::from(row);
        let c_id = ColId::from(col);
        match eval_bear.get_value(&t_id, &r_id, &c_id) {
            Some(CellValue::Number(n)) => *n,
            other => panic!("Expected number, got {other:?}"),
        }
    };

    assert_eq!(get_val_bear("DealAssumptions", "rev_growth", "val"), 0.03);
    assert_eq!(
        get_val_bear("DealAssumptions", "ebitda_margin", "val"),
        0.20
    );
    assert_eq!(get_val_bear("DealAssumptions", "exit_multiple", "val"), 8.0);

    let moic_bear = get_val_bear("ReturnsSummary", "moic", "val");
    let irr_bear = get_val_bear("ReturnsSummary", "irr", "val");
    assert!(
        moic_bear < 1.6,
        "Bear Case MoIC should be compressed under 1.6x, got {moic_bear}"
    );
    assert!(
        irr_bear < 0.10,
        "Bear Case IRR should be below 10%, got {irr_bear}"
    );

    // 3. Evaluate Base Case for three-way monotonic ordering verification
    let (manifest_base, tables_base) = build_lbo_model("base");
    let eval_base =
        Compiler::evaluate(&manifest_base, &tables_base).expect("Base Case must compile cleanly");
    assert!(eval_base.find_errors().is_empty());

    let get_val_base = |table: &str, row: &str, col: &str| -> f64 {
        let t_id = TableId::from(table);
        let r_id = RowId::from(row);
        let c_id = ColId::from(col);
        match eval_base.get_value(&t_id, &r_id, &c_id) {
            Some(CellValue::Number(n)) => *n,
            other => panic!("Expected number, got {other:?}"),
        }
    };

    let moic_base = get_val_base("ReturnsSummary", "moic", "val");
    let irr_base = get_val_base("ReturnsSummary", "irr", "val");

    // Relative ordering assertion: Bull > Base > Bear
    assert!(
        moic_bull > moic_base && moic_base > moic_bear,
        "MoIC must follow Bull ({moic_bull}) > Base ({moic_base}) > Bear ({moic_bear})"
    );
    assert!(
        irr_bull > irr_base && irr_base > irr_bear,
        "IRR must follow Bull ({irr_bull}) > Base ({irr_base}) > Bear ({irr_bear})"
    );
}
