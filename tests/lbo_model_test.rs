//! End-to-end integration test suite for an institutional-grade Leveraged Buyout (LBO) model.
//!
//! Serves as Finx's canonical anchoring model benchmark across the compiler pipeline:
//! Pass 4 (Topological Evaluation), Pass 5 (Spatial Layout), and Pass 6 (Formula Lowering & Emission).
//!
//! Specifications defined in `specs/history/2026-09-19-lbo-model-anchoring-integration-test.md`
//! and `specs/living-specs/compiler/evaluator.md`.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use finx::Compiler;
use finx::model::id::{ColId, RowId, TableId};
use finx::value::CellValue;

mod fixtures;

use fixtures::lbo::build_lbo_model;

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
    assert_eq!(get_val("DealAssumptions", "sponsor_hurdle", "val"), 0.20);

    // Verify Sources & Uses Irregular Table Linkages
    assert_eq!(
        get_val("SourcesAndUses", "senior_debt", "source_val"),
        150.0
    );
    assert_eq!(get_val("SourcesAndUses", "senior_debt", "use_val"), 500.0);
    assert_eq!(get_val("SourcesAndUses", "sub_debt", "source_val"), 75.0);
    assert_eq!(
        get_val("SourcesAndUses", "sponsor_equity", "source_val"),
        275.0
    );
    assert_eq!(get_val("SourcesAndUses", "total", "source_val"), 500.0);
    assert_eq!(get_val("SourcesAndUses", "total", "use_val"), 500.0);
    assert!((get_val("SourcesAndUses", "total", "source_pct") - 1.0).abs() < 1e-6);
    assert!((get_val("SourcesAndUses", "total", "use_pct") - 1.0).abs() < 1e-6);

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

    // Verify 5-Year Full Amortization PMT Benchmark
    for col in ["y1", "y2", "y3", "y4", "y5"] {
        let pmt_val = get_val("DebtSchedule", "term_pmt_benchmark", col);
        assert!((pmt_val - 37.56855).abs() < 1e-3);
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

    assert_eq!(exit_equity, exit_ev - exit_net_debt);

    let initial_equity = get_val("ReturnsSummary", "initial_equity", "val");
    assert_eq!(initial_equity, 275.0);

    let moic = get_val("ReturnsSummary", "moic", "val");
    assert!((moic - (exit_equity / initial_equity)).abs() < 1e-6);
    assert!(
        moic > 2.0 && moic < 3.0,
        "Expected base case MoIC around 2.2-2.5x, got {moic}"
    );

    let irr = get_val("ReturnsSummary", "irr", "val");
    assert!(
        irr > 0.15 && irr < 0.25,
        "Expected base case IRR around 18-22%, got {irr}"
    );

    let npv = get_val("ReturnsSummary", "npv", "val");
    assert!(
        npv > 0.0,
        "Base case NPV at 10% hurdle should be positive, got {npv}"
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
    assert_eq!(
        get_val_bull("DealAssumptions", "sponsor_hurdle", "val"),
        0.25
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
    assert_eq!(
        get_val_bear("DealAssumptions", "sponsor_hurdle", "val"),
        0.15
    );

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
