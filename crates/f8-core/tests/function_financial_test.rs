#![allow(clippy::unwrap_used, clippy::expect_used)]

use f8_core::compiler::eval::eval_expr;
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::ir::symbol::SymbolTable;
use f8_core::expr::ast::Expr;
use f8_core::functions::traits::{EvalContext, FunctionArg};
use f8_core::functions::{FunctionCategory, FunctionRegistry};
use f8_core::model::id::{SheetId, TableId};
use f8_core::model::table::{ColumnSpec, RowSpec, TableSpec};
use f8_core::value::{CellValue, ExcelError};
use indexmap::IndexMap;

fn eval_str(
    formula: &str,
    symbols: &SymbolTable,
    values: &IndexMap<CellId, CellValue>,
    registry: &FunctionRegistry,
    origin: &CellId,
) -> CellValue {
    let expr = Expr::parse(formula).expect("valid formula");
    eval_expr(&expr, origin, values, symbols, registry).expect("eval successfully")
}

/// Dummy mock evaluation context for direct unit testing of ExcelFunction::evaluate.
struct MockContext;

impl EvalContext for MockContext {
    fn eval_expr(
        &self,
        _expr: &f8_core::expr::ast::Expr,
    ) -> Result<CellValue, f8_core::functions::error::FunctionError> {
        Ok(CellValue::Empty)
    }
}

#[test]
fn test_scenario_1_registration_and_metadata() {
    let registry = FunctionRegistry::standard();
    assert_eq!(registry.len(), 44);

    let financial_names = ["NPV", "XNPV", "IRR", "XIRR", "PMT", "IPMT", "PPMT"];
    for name in financial_names {
        let func = registry
            .lookup(name)
            .unwrap_or_else(|| panic!("Function {name} must be registered"));
        assert_eq!(func.name(), name);
        assert_eq!(func.category(), FunctionCategory::Financial);
    }

    let npv = registry.lookup("NPV").unwrap();
    assert_eq!(npv.min_args(), 2);
    assert_eq!(npv.max_args(), None);

    let xnpv = registry.lookup("XNPV").unwrap();
    assert_eq!(xnpv.min_args(), 3);
    assert_eq!(xnpv.max_args(), Some(3));

    let irr = registry.lookup("IRR").unwrap();
    assert_eq!(irr.min_args(), 1);
    assert_eq!(irr.max_args(), Some(2));

    let xirr = registry.lookup("XIRR").unwrap();
    assert_eq!(xirr.min_args(), 2);
    assert_eq!(xirr.max_args(), Some(3));

    let pmt = registry.lookup("PMT").unwrap();
    assert_eq!(pmt.min_args(), 3);
    assert_eq!(pmt.max_args(), Some(5));

    let ipmt = registry.lookup("IPMT").unwrap();
    assert_eq!(ipmt.min_args(), 4);
    assert_eq!(ipmt.max_args(), Some(6));

    let ppmt = registry.lookup("PPMT").unwrap();
    assert_eq!(ppmt.min_args(), 4);
    assert_eq!(ppmt.max_args(), Some(6));
}

#[test]
fn test_scenario_2_npv_periodic_discounting() {
    let registry = FunctionRegistry::standard();
    let npv_func = registry.lookup("NPV").unwrap();
    let ctx = MockContext;

    // Classic periodic cash flows starting at t=1:
    // Rate = 0.10, CFs = [-10000, 3000, 4200, 6800]
    // NPV = -10000/1.1 + 3000/1.1^2 + 4200/1.1^3 + 6800/1.1^4 = 1188.44341
    let args = vec![
        FunctionArg::Scalar(CellValue::Number(0.10)),
        FunctionArg::Scalar(CellValue::Number(-10000.0)),
        FunctionArg::Scalar(CellValue::Number(3000.0)),
        FunctionArg::Scalar(CellValue::Number(4200.0)),
        FunctionArg::Scalar(CellValue::Number(6800.0)),
    ];
    let res = npv_func.evaluate(&args, &ctx).unwrap();
    if let CellValue::Number(val) = res {
        assert!((val - 1188.44341).abs() < 1e-4, "NPV result was {val}");
    } else {
        panic!("Expected Number, got {res:?}");
    }

    // Range reference with ignored empty and text cells
    let range_args = vec![
        FunctionArg::Scalar(CellValue::Number(0.10)),
        FunctionArg::Array {
            rows: 6,
            cols: 1,
            values: vec![
                CellValue::Number(-10000.0),
                CellValue::Text("ignored text".into()),
                CellValue::Number(3000.0),
                CellValue::Empty,
                CellValue::Number(4200.0),
                CellValue::Number(6800.0),
            ],
            is_range_ref: true,
        },
    ];
    let range_res = npv_func.evaluate(&range_args, &ctx).unwrap();
    if let CellValue::Number(val) = range_res {
        assert!((val - 1188.44341).abs() < 1e-4);
    } else {
        panic!("Expected Number, got {range_res:?}");
    }

    // Direct scalar text produces #VALUE!
    let err_args = vec![
        FunctionArg::Scalar(CellValue::Number(0.10)),
        FunctionArg::Scalar(CellValue::Text("bad".into())),
    ];
    assert_eq!(
        npv_func.evaluate(&err_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Value)
    );

    // Rate = -1.0 with non-zero cash flows returns #DIV/0!
    let div0_args = vec![
        FunctionArg::Scalar(CellValue::Number(-1.0)),
        FunctionArg::Scalar(CellValue::Number(100.0)),
    ];
    assert_eq!(
        npv_func.evaluate(&div0_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Div0)
    );

    // Rate = -1.0 returns #DIV/0! even with zero cash flows
    let zero_args = vec![
        FunctionArg::Scalar(CellValue::Number(-1.0)),
        FunctionArg::Scalar(CellValue::Number(0.0)),
    ];
    assert_eq!(
        npv_func.evaluate(&zero_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Div0)
    );
}

#[test]
fn test_scenario_3_xnpv_non_periodic_discounting() {
    let registry = FunctionRegistry::standard();
    let xnpv_func = registry.lookup("XNPV").unwrap();
    let ctx = MockContext;

    // Rate = 0.09, Values = [-10000, 2750, 4250, 5000], Dates = [45292, 45383, 45475, 45657] (2024-01-01 to 2024-12-31)
    let d0 = 45292.0;
    let d1 = 45383.0; // 91 days
    let d2 = 45475.0; // 183 days
    let d3 = 45657.0; // 365 days
    let expected = -10000.0
        + 2750.0 / (1.09_f64).powf(91.0 / 365.0)
        + 4250.0 / (1.09_f64).powf(183.0 / 365.0)
        + 5000.0 / (1.09_f64).powf(365.0 / 365.0);

    let args = vec![
        FunctionArg::Scalar(CellValue::Number(0.09)),
        FunctionArg::Array {
            rows: 4,
            cols: 1,
            values: vec![
                CellValue::Number(-10000.0),
                CellValue::Number(2750.0),
                CellValue::Number(4250.0),
                CellValue::Number(5000.0),
            ],
            is_range_ref: false,
        },
        FunctionArg::Array {
            rows: 4,
            cols: 1,
            values: vec![
                CellValue::Number(d0),
                CellValue::Number(d1),
                CellValue::Number(d2),
                CellValue::Number(d3),
            ],
            is_range_ref: false,
        },
    ];
    let res = xnpv_func.evaluate(&args, &ctx).unwrap();
    if let CellValue::Number(val) = res {
        assert!(
            (val - expected).abs() < 1e-4,
            "XNPV was {val}, expected {expected}"
        );
    } else {
        panic!("Expected Number, got {res:?}");
    }

    // Rate <= -1.0 returns #NUM!
    let bad_rate_args = vec![
        FunctionArg::Scalar(CellValue::Number(-1.0)),
        args[1].clone(),
        args[2].clone(),
    ];
    assert_eq!(
        xnpv_func.evaluate(&bad_rate_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // Mismatched vector lengths return #NUM!
    let mismatched_args = vec![
        FunctionArg::Scalar(CellValue::Number(0.09)),
        FunctionArg::Array {
            rows: 2,
            cols: 1,
            values: vec![CellValue::Number(-1000.0), CellValue::Number(1000.0)],
            is_range_ref: false,
        },
        FunctionArg::Array {
            rows: 1,
            cols: 1,
            values: vec![CellValue::Number(d0)],
            is_range_ref: false,
        },
    ];
    assert_eq!(
        xnpv_func.evaluate(&mismatched_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // Out-of-order date (di < d0) returns #NUM!
    let out_of_order_args = vec![
        FunctionArg::Scalar(CellValue::Number(0.09)),
        FunctionArg::Array {
            rows: 2,
            cols: 1,
            values: vec![CellValue::Number(-1000.0), CellValue::Number(1000.0)],
            is_range_ref: false,
        },
        FunctionArg::Array {
            rows: 2,
            cols: 1,
            values: vec![CellValue::Number(d1), CellValue::Number(d0)],
            is_range_ref: false,
        },
    ];
    assert_eq!(
        xnpv_func.evaluate(&out_of_order_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // Date serial truncation (fractional timestamps truncate to integer days)
    let trunc_dates = vec![
        FunctionArg::Scalar(CellValue::Number(0.09)),
        args[1].clone(),
        FunctionArg::Array {
            rows: 4,
            cols: 1,
            values: vec![
                CellValue::Number(d0 + 0.5),
                CellValue::Number(d1 + 0.25),
                CellValue::Number(d2 + 0.75),
                CellValue::Number(d3 + 0.9),
            ],
            is_range_ref: false,
        },
    ];
    let trunc_res = xnpv_func.evaluate(&trunc_dates, &ctx).unwrap();
    if let CellValue::Number(val) = trunc_res {
        assert!((val - expected).abs() < 1e-4);
    } else {
        panic!("Expected Number, got {trunc_res:?}");
    }
}

#[test]
fn test_scenario_4_irr_periodic_solver() {
    let registry = FunctionRegistry::standard();
    let irr_func = registry.lookup("IRR").unwrap();
    let ctx = MockContext;

    // Standard cash flows: [-10000, 3000, 4200, 6800]
    // Target rate is approximately 16.34% (0.1634)
    let args = vec![FunctionArg::Array {
        rows: 4,
        cols: 1,
        values: vec![
            CellValue::Number(-10000.0),
            CellValue::Number(3000.0),
            CellValue::Number(4200.0),
            CellValue::Number(6800.0),
        ],
        is_range_ref: true,
    }];
    let res = irr_func.evaluate(&args, &ctx).unwrap();
    if let CellValue::Number(rate) = res {
        assert!((rate - 0.1634).abs() < 1e-3, "IRR was {rate}");
        // Verify that NPV at this rate is zero:
        let npv = -10000.0
            + 3000.0 / (1.0 + rate)
            + 4200.0 / (1.0 + rate).powi(2)
            + 6800.0 / (1.0 + rate).powi(3);
        assert!(npv.abs() < 1e-4, "NPV at IRR was {npv}");
    } else {
        panic!("Expected Number, got {res:?}");
    }

    // All positive cash flows -> #NUM!
    let all_pos = vec![FunctionArg::Array {
        rows: 3,
        cols: 1,
        values: vec![
            CellValue::Number(100.0),
            CellValue::Number(200.0),
            CellValue::Number(300.0),
        ],
        is_range_ref: true,
    }];
    assert_eq!(
        irr_func.evaluate(&all_pos, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // All negative cash flows -> #NUM!
    let all_neg = vec![FunctionArg::Array {
        rows: 3,
        cols: 1,
        values: vec![
            CellValue::Number(-100.0),
            CellValue::Number(-200.0),
            CellValue::Number(-300.0),
        ],
        is_range_ref: true,
    }];
    assert_eq!(
        irr_func.evaluate(&all_neg, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // Fewer than 2 cash flows -> #NUM!
    let too_few = vec![FunctionArg::Array {
        rows: 1,
        cols: 1,
        values: vec![CellValue::Number(-100.0)],
        is_range_ref: true,
    }];
    assert_eq!(
        irr_func.evaluate(&too_few, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );
}

#[test]
fn test_scenario_5_xirr_non_periodic_solver() {
    let registry = FunctionRegistry::standard();
    let xirr_func = registry.lookup("XIRR").unwrap();
    let ctx = MockContext;

    // Exactly 1 year apart: [-1000, 1100] gives exactly 10%
    let args = vec![
        FunctionArg::Array {
            rows: 2,
            cols: 1,
            values: vec![CellValue::Number(-1000.0), CellValue::Number(1100.0)],
            is_range_ref: false,
        },
        FunctionArg::Array {
            rows: 2,
            cols: 1,
            values: vec![CellValue::Number(45292.0), CellValue::Number(45657.0)],
            is_range_ref: false,
        },
    ];
    let res = xirr_func.evaluate(&args, &ctx).unwrap();
    if let CellValue::Number(rate) = res {
        assert!((rate - 0.10).abs() < 1e-4, "XIRR was {rate}");
    } else {
        panic!("Expected Number, got {res:?}");
    }

    // Realistic multi-period non-periodic project
    let dates = vec![
        CellValue::Number(45292.0), // 2024-01-01
        CellValue::Number(45352.0), // 2024-03-01
        CellValue::Number(45657.0), // 2024-12-31
        CellValue::Number(46022.0), // 2025-12-31
    ];
    let values = vec![
        CellValue::Number(-10000.0),
        CellValue::Number(-2000.0),
        CellValue::Number(6000.0),
        CellValue::Number(9000.0),
    ];
    let multi_args = vec![
        FunctionArg::Array {
            rows: 4,
            cols: 1,
            values,
            is_range_ref: false,
        },
        FunctionArg::Array {
            rows: 4,
            cols: 1,
            values: dates,
            is_range_ref: false,
        },
    ];
    let multi_res = xirr_func.evaluate(&multi_args, &ctx).unwrap();
    if let CellValue::Number(rate) = multi_res {
        assert!(rate > 0.05 && rate < 0.25, "XIRR multi-period was {rate}");
    } else {
        panic!("Expected Number, got {multi_res:?}");
    }

    // Fractional timestamps in XIRR truncate to integer days
    let trunc_dates = vec![
        CellValue::Number(45292.6),
        CellValue::Number(45352.1),
        CellValue::Number(45657.9),
        CellValue::Number(46022.4),
    ];
    let trunc_args = vec![
        FunctionArg::Array {
            rows: 4,
            cols: 1,
            values: vec![
                CellValue::Number(-10000.0),
                CellValue::Number(-2000.0),
                CellValue::Number(6000.0),
                CellValue::Number(9000.0),
            ],
            is_range_ref: false,
        },
        FunctionArg::Array {
            rows: 4,
            cols: 1,
            values: trunc_dates,
            is_range_ref: false,
        },
    ];
    let trunc_res = xirr_func.evaluate(&trunc_args, &ctx).unwrap();
    if let CellValue::Number(rate) = trunc_res {
        let expected_rate = match multi_res {
            CellValue::Number(r) => r,
            _ => unreachable!(),
        };
        assert!(
            (rate - expected_rate).abs() < 1e-9,
            "Truncated dates should match integer dates"
        );
    } else {
        panic!("Expected Number, got {trunc_res:?}");
    }
}

#[test]
fn test_scenario_6_pmt_annuity_payments() {
    let registry = FunctionRegistry::standard();
    let pmt_func = registry.lookup("PMT").unwrap();
    let ctx = MockContext;

    // Loan: $10,000 at 8% annual rate paid monthly for 10 years (120 months)
    // Type 0 (period end): PMT(0.08/12, 120, 10000) = -121.32759
    let rate = 0.08 / 12.0;
    let nper = 120.0;
    let pv = 10000.0;

    let args_end = vec![
        FunctionArg::Scalar(CellValue::Number(rate)),
        FunctionArg::Scalar(CellValue::Number(nper)),
        FunctionArg::Scalar(CellValue::Number(pv)),
    ];
    let res_end = pmt_func.evaluate(&args_end, &ctx).unwrap();
    if let CellValue::Number(pmt) = res_end {
        assert!((pmt - (-121.32759)).abs() < 1e-4, "PMT end was {pmt}");
    } else {
        panic!("Expected Number, got {res_end:?}");
    }

    // Type 1 (period start): PMT(0.08/12, 120, 10000, 0, 1) = -120.52409
    let args_start = vec![
        FunctionArg::Scalar(CellValue::Number(rate)),
        FunctionArg::Scalar(CellValue::Number(nper)),
        FunctionArg::Scalar(CellValue::Number(pv)),
        FunctionArg::Scalar(CellValue::Number(0.0)),
        FunctionArg::Scalar(CellValue::Number(1.0)),
    ];
    let res_start = pmt_func.evaluate(&args_start, &ctx).unwrap();
    if let CellValue::Number(pmt) = res_start {
        assert!((pmt - (-120.52409)).abs() < 1e-4, "PMT start was {pmt}");
    } else {
        panic!("Expected Number, got {res_start:?}");
    }

    // Zero interest rate: PMT(0, 10, 1000) = -100.0
    let args_zero_rate = vec![
        FunctionArg::Scalar(CellValue::Number(0.0)),
        FunctionArg::Scalar(CellValue::Number(10.0)),
        FunctionArg::Scalar(CellValue::Number(1000.0)),
    ];
    let res_zero = pmt_func.evaluate(&args_zero_rate, &ctx).unwrap();
    assert_eq!(res_zero, CellValue::Number(-100.0));

    // nper = 0 returns #DIV/0!
    let args_zero_nper = vec![
        FunctionArg::Scalar(CellValue::Number(0.05)),
        FunctionArg::Scalar(CellValue::Number(0.0)),
        FunctionArg::Scalar(CellValue::Number(1000.0)),
    ];
    assert_eq!(
        pmt_func.evaluate(&args_zero_nper, &ctx).unwrap(),
        CellValue::Error(ExcelError::Div0)
    );
}

#[test]
fn test_scenario_7_ipmt_and_ppmt_amortization_identity() {
    let registry = FunctionRegistry::standard();
    let pmt_func = registry.lookup("PMT").unwrap();
    let ipmt_func = registry.lookup("IPMT").unwrap();
    let ppmt_func = registry.lookup("PPMT").unwrap();
    let ctx = MockContext;

    let rate = 0.08 / 12.0;
    let nper = 120.0;
    let pv = 10000.0;

    let pmt_val = match pmt_func
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(rate)),
                FunctionArg::Scalar(CellValue::Number(nper)),
                FunctionArg::Scalar(CellValue::Number(pv)),
            ],
            &ctx,
        )
        .unwrap()
    {
        CellValue::Number(n) => n,
        other => panic!("Expected Number, got {other:?}"),
    };

    // Amortization invariant for Type 0:
    // Across periods 1 to 120, IPMT + PPMT == PMT
    for period in [1, 2, 60, 119, 120] {
        let args = vec![
            FunctionArg::Scalar(CellValue::Number(rate)),
            FunctionArg::Scalar(CellValue::Number(period as f64)),
            FunctionArg::Scalar(CellValue::Number(nper)),
            FunctionArg::Scalar(CellValue::Number(pv)),
        ];

        let ipmt_res = ipmt_func.evaluate(&args, &ctx).unwrap();
        let ppmt_res = ppmt_func.evaluate(&args, &ctx).unwrap();

        match (ipmt_res, ppmt_res) {
            (CellValue::Number(ipmt), CellValue::Number(ppmt)) => {
                assert!(
                    (ipmt + ppmt - pmt_val).abs() < 1e-6,
                    "Period {period}: IPMT ({ipmt}) + PPMT ({ppmt}) != PMT ({pmt_val})"
                );
                // Period 1 interest check: exactly -10000 * (0.08/12) = -66.66667
                if period == 1 {
                    assert!((ipmt - (-66.66667)).abs() < 1e-4);
                }
            }
            other => panic!("Expected Numbers, got {other:?}"),
        }
    }

    // Type 1 (beginning of period): period 1 interest must be 0.0
    let args_type1_p1 = vec![
        FunctionArg::Scalar(CellValue::Number(rate)),
        FunctionArg::Scalar(CellValue::Number(1.0)),
        FunctionArg::Scalar(CellValue::Number(nper)),
        FunctionArg::Scalar(CellValue::Number(pv)),
        FunctionArg::Scalar(CellValue::Number(0.0)),
        FunctionArg::Scalar(CellValue::Number(1.0)),
    ];
    let ipmt_p1_res = ipmt_func.evaluate(&args_type1_p1, &ctx).unwrap();
    assert_eq!(ipmt_p1_res, CellValue::Number(0.0));

    // Out of bounds period (per > nper or per < 1) returns #NUM!
    let bad_per_args = vec![
        FunctionArg::Scalar(CellValue::Number(rate)),
        FunctionArg::Scalar(CellValue::Number(121.0)),
        FunctionArg::Scalar(CellValue::Number(nper)),
        FunctionArg::Scalar(CellValue::Number(pv)),
    ];
    assert_eq!(
        ipmt_func.evaluate(&bad_per_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );
    assert_eq!(
        ppmt_func.evaluate(&bad_per_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );
}

#[test]
fn test_scenario_8_end_to_end_formula_evaluation() {
    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");
    let registry = FunctionRegistry::standard();

    let table = TableSpec {
        id: TableId::from("DebtService"),
        title: "DebtService".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("value")],
        rows: vec![
            RowSpec::new("loan_amount"),
            RowSpec::new("interest_rate"),
            RowSpec::new("periods"),
            RowSpec::new("monthly_pmt"),
            RowSpec::new("first_month_interest"),
            RowSpec::new("first_month_principal"),
            RowSpec::new("cf0"),
            RowSpec::new("cf1"),
            RowSpec::new("cf2"),
            RowSpec::new("cf3"),
        ],
    };
    symbols.register_table(sheet, &table).unwrap();

    let origin = CellId::new("DebtService", "monthly_pmt", "value");
    let mut values = IndexMap::new();
    values.insert(
        CellId::new("DebtService", "loan_amount", "value"),
        CellValue::Number(10000.0),
    );
    values.insert(
        CellId::new("DebtService", "interest_rate", "value"),
        CellValue::Number(0.08 / 12.0),
    );
    values.insert(
        CellId::new("DebtService", "periods", "value"),
        CellValue::Number(120.0),
    );
    values.insert(
        CellId::new("DebtService", "cf0", "value"),
        CellValue::Number(-10000.0),
    );
    values.insert(
        CellId::new("DebtService", "cf1", "value"),
        CellValue::Number(3000.0),
    );
    values.insert(
        CellId::new("DebtService", "cf2", "value"),
        CellValue::Number(4200.0),
    );
    values.insert(
        CellId::new("DebtService", "cf3", "value"),
        CellValue::Number(6800.0),
    );

    // PMT formula
    let pmt_res = eval_str(
        "PMT(DebtService[\"interest_rate\", \"value\"], DebtService[\"periods\", \"value\"], DebtService[\"loan_amount\", \"value\"])",
        &symbols,
        &values,
        &registry,
        &origin,
    );
    if let CellValue::Number(pmt) = pmt_res {
        assert!((pmt - (-121.32759)).abs() < 1e-4);
    } else {
        panic!("Expected Number, got {pmt_res:?}");
    }

    // IPMT formula
    let ipmt_res = eval_str(
        "IPMT(DebtService[\"interest_rate\", \"value\"], 1, DebtService[\"periods\", \"value\"], DebtService[\"loan_amount\", \"value\"])",
        &symbols,
        &values,
        &registry,
        &origin,
    );
    if let CellValue::Number(ipmt) = ipmt_res {
        assert!((ipmt - (-66.66667)).abs() < 1e-4);
    } else {
        panic!("Expected Number, got {ipmt_res:?}");
    }

    // PPMT formula
    let ppmt_res = eval_str(
        "PPMT(DebtService[\"interest_rate\", \"value\"], 1, DebtService[\"periods\", \"value\"], DebtService[\"loan_amount\", \"value\"])",
        &symbols,
        &values,
        &registry,
        &origin,
    );
    if let CellValue::Number(ppmt) = ppmt_res {
        assert!((ppmt - (-54.66092)).abs() < 1e-4);
    } else {
        panic!("Expected Number, got {ppmt_res:?}");
    }

    // NPV formula referencing range slice
    let npv_res = eval_str(
        "NPV(0.10, DebtService[\"cf0\"..\"cf3\", \"value\"])",
        &symbols,
        &values,
        &registry,
        &origin,
    );
    if let CellValue::Number(npv) = npv_res {
        assert!((npv - 1188.44341).abs() < 1e-4);
    } else {
        panic!("Expected Number, got {npv_res:?}");
    }

    // IRR formula referencing range slice
    let irr_res = eval_str(
        "IRR(DebtService[\"cf0\"..\"cf3\", \"value\"])",
        &symbols,
        &values,
        &registry,
        &origin,
    );
    if let CellValue::Number(irr) = irr_res {
        assert!((irr - 0.1634).abs() < 1e-3);
    } else {
        panic!("Expected Number, got {irr_res:?}");
    }
}
