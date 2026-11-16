#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use f8_core::compiler::eval::eval_expr;
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::ir::symbol::SymbolTable;
use f8_core::expr::ast::Expr;
use f8_core::functions::traits::{EvalContext, FunctionArg, ReferenceKind};
use f8_core::functions::{FunctionCategory, FunctionError, FunctionRegistry};
use f8_core::model::id::{SheetId, TableId};
use f8_core::model::table::{AxisRole, ColumnSpec, RowSpec, TableSpec};
use f8_core::value::{CellValue, ExcelError, TypeError, ValueType};
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
    assert_eq!(registry.len(), 43);

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

    // Clean numeric Measure slice succeeds
    let range_args = vec![
        FunctionArg::Scalar(CellValue::Number(0.10)),
        FunctionArg::range(
            4,
            1,
            vec![
                CellValue::Number(-10000.0),
                CellValue::Number(3000.0),
                CellValue::Number(4200.0),
                CellValue::Number(6800.0),
            ],
            ReferenceKind::Slice,
        ),
    ];
    let range_res = npv_func.evaluate(&range_args, &ctx).unwrap();
    if let CellValue::Number(val) = range_res {
        assert!((val - 1188.44341).abs() < 1e-4);
    } else {
        panic!("Expected Number, got {range_res:?}");
    }

    // Text or Boolean in range or scalar fails in Pass 4a with InvalidArgumentType
    for (formula, found_type) in [
        ("NPV(0.10, \"bad\")", ValueType::Text),
        ("NPV(0.10, TRUE)", ValueType::Boolean),
    ] {
        assert_eq!(
            registry.validate_expr(&Expr::parse(formula).unwrap()),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "NPV",
                param: "value1",
                position: 2,
                expected: ValueType::Number,
                found: found_type,
            }))
        );
    }
    let npv_slice_expr = Expr::parse("NPV(0.10, T[\"r1\"..\"r2\", \"val\"])").unwrap();
    let mut resolve_npv_bad =
        |_: &f8_core::expr::TableRef| -> Result<Option<f8_core::functions::StaticRef>, &'static str> {
            Ok(Some(f8_core::functions::StaticRef {
                rows: 2,
                cols: 1,
                axis_role: AxisRole::Measure,
                elem_types: vec![Some(ValueType::Number), Some(ValueType::Text)],
            }))
        };
    assert_eq!(
        registry.validate_expr_with_shapes(&npv_slice_expr, &mut resolve_npv_bad),
        Err(f8_core::functions::StageAError::Function(
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "NPV",
                param: "value1",
                position: 2,
                expected: ValueType::Number,
                found: ValueType::Text,
            })
        ))
    );

    // Descriptor range fails statically with DescriptorReferenceNotAllowed
    let npv_desc_expr =
        f8_core::expr::Expr::parse("NPV(0.10, T[\"r1\"..\"r2\", \"desc\"])").unwrap();
    let mut resolve_npv_desc =
        |_: &f8_core::expr::TableRef| -> Result<Option<f8_core::functions::StaticRef>, &'static str> {
            Ok(Some(f8_core::functions::StaticRef {
                rows: 2,
                cols: 1,
                axis_role: AxisRole::Descriptor,
                elem_types: vec![Some(ValueType::Number); 2],
            }))
        };
    assert_eq!(
        registry.validate_expr_with_shapes(&npv_desc_expr, &mut resolve_npv_desc),
        Err(f8_core::functions::StageAError::Function(
            FunctionError::DescriptorReferenceNotAllowed {
                function: "NPV",
                param: "value1",
                position: 2,
            }
        ))
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
        FunctionArg::range(
            4,
            1,
            vec![
                CellValue::Number(-10000.0),
                CellValue::Number(2750.0),
                CellValue::Number(4250.0),
                CellValue::Number(5000.0),
            ],
            ReferenceKind::Slice,
        ),
        FunctionArg::range(
            4,
            1,
            vec![
                CellValue::Number(d0),
                CellValue::Number(d1),
                CellValue::Number(d2),
                CellValue::Number(d3),
            ],
            ReferenceKind::Slice,
        ),
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

    // Rate <= 0.0 returns #NUM! (matching Excel's XNPV non-positive rate guard)
    for non_positive_rate in [0.0, -1e-9, -0.05, -1.0, -2.0] {
        let bad_rate_args = vec![
            FunctionArg::Scalar(CellValue::Number(non_positive_rate)),
            args[1].clone(),
            args[2].clone(),
        ];
        assert_eq!(
            xnpv_func.evaluate(&bad_rate_args, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num),
            "expected #NUM! for XNPV rate = {non_positive_rate}"
        );
    }

    // Mismatched vector lengths and 2D slices fail statically in validate_expr_with_shapes
    let xnpv_shape_expr =
        f8_core::expr::Expr::parse("XNPV(0.09, T[\"v1\"..\"v2\", @col], T[\"d1\", @col])").unwrap();
    let mut resolve_xnpv_len =
        |r: &f8_core::expr::TableRef| -> Result<Option<f8_core::functions::StaticRef>, &'static str> {
            let rows = if matches!(r.row, f8_core::expr::RowSelector::Range(_, _)) {
                2
            } else {
                1
            };
            Ok(Some(f8_core::functions::StaticRef {
                rows,
                cols: 1,
                axis_role: AxisRole::Measure,
                elem_types: vec![Some(ValueType::Number); rows],
            }))
        };
    assert_eq!(
        registry.validate_expr_with_shapes(&xnpv_shape_expr, &mut resolve_xnpv_len),
        Err(f8_core::functions::StageAError::Function(
            FunctionError::LengthMismatch {
                function: "XNPV",
                expected: 2,
                found: 1,
            }
        ))
    );

    let mut resolve_xnpv_2d =
        |_: &f8_core::expr::TableRef| -> Result<Option<f8_core::functions::StaticRef>, &'static str> {
            Ok(Some(f8_core::functions::StaticRef {
                rows: 2,
                cols: 2,
                axis_role: AxisRole::Measure,
                elem_types: vec![Some(ValueType::Number); 4],
            }))
        };
    assert_eq!(
        registry.validate_expr_with_shapes(&xnpv_shape_expr, &mut resolve_xnpv_2d),
        Err(f8_core::functions::StageAError::Function(
            FunctionError::ExpectedOneDimensionalSlice {
                function: "XNPV",
                param: "values",
                rows: 2,
                cols: 2,
            }
        ))
    );

    // OOB dates return #NUM!
    assert_eq!(
        xnpv_func.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(0.09)),
                FunctionArg::range(
                    2,
                    1,
                    vec![CellValue::Number(d0), CellValue::Number(d1)],
                    ReferenceKind::Slice,
                ),
                FunctionArg::range(
                    2,
                    1,
                    vec![CellValue::Number(-1000.0), CellValue::Number(1100.0)],
                    ReferenceKind::Slice,
                ),
            ],
            &ctx,
        ),
        Ok(CellValue::Error(ExcelError::Num))
    );

    // Out-of-order date (di < d0) returns #NUM!
    let out_of_order_args = vec![
        FunctionArg::Scalar(CellValue::Number(0.09)),
        FunctionArg::range(
            2,
            1,
            vec![CellValue::Number(-1000.0), CellValue::Number(1000.0)],
            ReferenceKind::Slice,
        ),
        FunctionArg::range(
            2,
            1,
            vec![CellValue::Number(d1), CellValue::Number(d0)],
            ReferenceKind::Slice,
        ),
    ];
    assert_eq!(
        xnpv_func.evaluate(&out_of_order_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // Date serial truncation (fractional timestamps truncate to integer days)
    let trunc_dates = vec![
        FunctionArg::Scalar(CellValue::Number(0.09)),
        args[1].clone(),
        FunctionArg::range(
            4,
            1,
            vec![
                CellValue::Number(d0 + 0.5),
                CellValue::Number(d1 + 0.25),
                CellValue::Number(d2 + 0.75),
                CellValue::Number(d3 + 0.9),
            ],
            ReferenceKind::Slice,
        ),
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
    let args = vec![FunctionArg::range(
        4,
        1,
        vec![
            CellValue::Number(-10000.0),
            CellValue::Number(3000.0),
            CellValue::Number(4200.0),
            CellValue::Number(6800.0),
        ],
        ReferenceKind::Slice,
    )];
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
    let all_pos = vec![FunctionArg::range(
        3,
        1,
        vec![
            CellValue::Number(100.0),
            CellValue::Number(200.0),
            CellValue::Number(300.0),
        ],
        ReferenceKind::Slice,
    )];
    assert_eq!(
        irr_func.evaluate(&all_pos, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // All negative cash flows -> #NUM!
    let all_neg = vec![FunctionArg::range(
        3,
        1,
        vec![
            CellValue::Number(-1000.0 / 10.0),
            CellValue::Number(-200.0),
            CellValue::Number(-300.0),
        ],
        ReferenceKind::Slice,
    )];
    assert_eq!(
        irr_func.evaluate(&all_neg, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // Fewer than 2 cash flows -> #NUM!
    let too_few = vec![FunctionArg::range(
        1,
        1,
        vec![CellValue::Number(-1000.0)],
        ReferenceKind::Slice,
    )];
    assert_eq!(
        irr_func.evaluate(&too_few, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // guess <= -1.0 returns #NUM! (matching live Excel and PMT/XNPV rate domain handling)
    for bad_guess in [-1.0, -1.5, -100.0] {
        let bad_guess_args = vec![
            args[0].clone(),
            FunctionArg::Scalar(CellValue::Number(bad_guess)),
        ];
        assert_eq!(
            irr_func.evaluate(&bad_guess_args, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num),
            "expected #NUM! for IRR guess = {bad_guess}"
        );
    }

    // Runtime ExcelError in `guess` bubbles before sign-alternation `#NUM!`
    let guess_err_args = vec![
        all_pos[0].clone(),
        FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
    ];
    assert_eq!(
        irr_func.evaluate(&guess_err_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Div0)
    );

    // 30-year monthly mortgage ([-100_000] + [600; 360]) converges from default guess = 0.10
    let mut mortgage_cfs = vec![CellValue::Number(-100_000.0)];
    mortgage_cfs.extend(std::iter::repeat_n(CellValue::Number(600.0), 360));
    let mortgage_args = vec![FunctionArg::range(
        361,
        1,
        mortgage_cfs,
        ReferenceKind::Slice,
    )];
    let mortgage_res = irr_func.evaluate(&mortgage_args, &ctx).unwrap();
    if let CellValue::Number(rate) = mortgage_res {
        assert!(
            (rate - 0.005_005_825_006_762_407).abs() < 1e-12,
            "30y monthly mortgage IRR was {rate}"
        );
    } else {
        panic!("Expected Number, got {mortgage_res:?}");
    }

    // Leading-zero cash flows with extreme guess = 1e160 find true root 0.10 rather than overflow 0.0
    let lead_zero_args = vec![
        FunctionArg::range(
            4,
            1,
            vec![
                CellValue::Number(0.0),
                CellValue::Number(0.0),
                CellValue::Number(-100.0),
                CellValue::Number(110.0),
            ],
            ReferenceKind::Slice,
        ),
        FunctionArg::Scalar(CellValue::Number(1e160)),
    ];
    if let CellValue::Number(rate) = irr_func.evaluate(&lead_zero_args, &ctx).unwrap() {
        assert!((rate - 0.10).abs() < 1e-12, "Leading-zero IRR was {rate}");
    } else {
        panic!("Expected Number for leading-zero IRR");
    }

    // Multi-root cash flows [-1600, 10000, -10000] select root nearest guess (0.25 vs 4.0)
    let multi_root_slice = FunctionArg::range(
        3,
        1,
        vec![
            CellValue::Number(-1600.0),
            CellValue::Number(10_000.0),
            CellValue::Number(-10_000.0),
        ],
        ReferenceKind::Slice,
    );
    if let CellValue::Number(low_root) = irr_func
        .evaluate(std::slice::from_ref(&multi_root_slice), &ctx)
        .unwrap()
    {
        assert!((low_root - 0.25).abs() < 1e-12);
    } else {
        panic!("Expected low IRR root 0.25");
    }
    if let CellValue::Number(high_root) = irr_func
        .evaluate(
            &[
                multi_root_slice,
                FunctionArg::Scalar(CellValue::Number(3.0)),
            ],
            &ctx,
        )
        .unwrap()
    {
        assert!((high_root - 4.0).abs() < 1e-12);
    } else {
        panic!("Expected high IRR root 4.0");
    }

    // Rootless sign-alternating cash flows return #NUM!
    for rootless in [
        vec![
            CellValue::Number(100.0),
            CellValue::Number(-10.0),
            CellValue::Number(100.0),
        ],
        vec![
            CellValue::Number(-100.0),
            CellValue::Number(50.0),
            CellValue::Number(-100.0),
        ],
    ] {
        let rootless_args = vec![FunctionArg::range(3, 1, rootless, ReferenceKind::Slice)];
        assert_eq!(
            irr_func.evaluate(&rootless_args, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num)
        );
    }

    // Severe-loss salvage schedule [-1_000_000, 1] converges to -0.999999
    let salvage_slice = FunctionArg::range(
        2,
        1,
        vec![CellValue::Number(-1_000_000.0), CellValue::Number(1.0)],
        ReferenceKind::Slice,
    );
    if let CellValue::Number(rate) = irr_func
        .evaluate(std::slice::from_ref(&salvage_slice), &ctx)
        .unwrap()
    {
        assert!(
            (rate - (-0.999_999)).abs() < 1e-14,
            "Salvage IRR was {rate}"
        );
    } else {
        panic!("Expected Number for salvage IRR");
    }
}

#[test]
fn test_scenario_5_xirr_non_periodic_solver() {
    let registry = FunctionRegistry::standard();
    let xirr_func = registry.lookup("XIRR").unwrap();
    let ctx = MockContext;

    // Exactly 1 year apart: [-1000, 1100] gives exactly 10%
    let args = vec![
        FunctionArg::range(
            2,
            1,
            vec![CellValue::Number(-1000.0), CellValue::Number(1100.0)],
            ReferenceKind::Slice,
        ),
        FunctionArg::range(
            2,
            1,
            vec![CellValue::Number(45292.0), CellValue::Number(45657.0)],
            ReferenceKind::Slice,
        ),
    ];
    let res = xirr_func.evaluate(&args, &ctx).unwrap();
    if let CellValue::Number(rate) = res {
        assert!((rate - 0.10).abs() < 1e-4, "XIRR was {rate}");
    } else {
        panic!("Expected Number, got {res:?}");
    }

    // OOB serial in XIRR dates returns #NUM!
    assert_eq!(
        xirr_func.evaluate(
            &[
                args[0].clone(),
                FunctionArg::range(
                    2,
                    1,
                    vec![CellValue::Number(0.0), CellValue::Number(45657.0)],
                    ReferenceKind::Slice,
                ),
            ],
            &ctx,
        ),
        Ok(CellValue::Error(ExcelError::Num))
    );

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
        FunctionArg::range(4, 1, values, ReferenceKind::Slice),
        FunctionArg::range(4, 1, dates, ReferenceKind::Slice),
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
        FunctionArg::range(
            4,
            1,
            vec![
                CellValue::Number(-10000.0),
                CellValue::Number(-2000.0),
                CellValue::Number(6000.0),
                CellValue::Number(9000.0),
            ],
            ReferenceKind::Slice,
        ),
        FunctionArg::range(4, 1, trunc_dates, ReferenceKind::Slice),
    ];
    let trunc_res = xirr_func.evaluate(&trunc_args, &ctx).unwrap();
    if let CellValue::Number(rate) = trunc_res {
        let CellValue::Number(expected_rate) = multi_res else {
            unreachable!();
        };
        assert!(
            (rate - expected_rate).abs() < 1e-9,
            "Truncated dates should match integer dates"
        );
    } else {
        panic!("Expected Number, got {trunc_res:?}");
    }

    // guess <= -1.0 in XIRR returns #NUM!
    for bad_guess in [-1.0, -1.5, -100.0] {
        let bad_guess_args = vec![
            args[0].clone(),
            args[1].clone(),
            FunctionArg::Scalar(CellValue::Number(bad_guess)),
        ];
        assert_eq!(
            xirr_func.evaluate(&bad_guess_args, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num),
            "expected #NUM! for XIRR guess = {bad_guess}"
        );
    }

    // Runtime ExcelError in `guess` bubbles before sign-alternation `#NUM!`
    let xirr_guess_err_args = vec![
        FunctionArg::range(
            2,
            1,
            vec![CellValue::Number(100.0), CellValue::Number(200.0)],
            ReferenceKind::Slice,
        ),
        args[1].clone(),
        FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
    ];
    assert_eq!(
        xirr_func.evaluate(&xirr_guess_err_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Div0)
    );

    // Leading-zero XIRR cash flows with extreme guess = 1e160 converge to true root 0.10
    let xirr_lead_zero = vec![
        FunctionArg::range(
            3,
            1,
            vec![
                CellValue::Number(0.0),
                CellValue::Number(-1000.0),
                CellValue::Number(1100.0),
            ],
            ReferenceKind::Slice,
        ),
        FunctionArg::range(
            3,
            1,
            vec![
                CellValue::Number(45292.0),
                CellValue::Number(45657.0),
                CellValue::Number(46022.0),
            ],
            ReferenceKind::Slice,
        ),
        FunctionArg::Scalar(CellValue::Number(1e160)),
    ];
    if let CellValue::Number(rate) = xirr_func.evaluate(&xirr_lead_zero, &ctx).unwrap() {
        assert!((rate - 0.10).abs() < 1e-12, "Leading-zero XIRR was {rate}");
    } else {
        panic!("Expected Number for leading-zero XIRR");
    }

    // Rootless and same-day cancelling XIRR schedules return #NUM!
    let xirr_same_day_cancel = vec![
        FunctionArg::range(
            2,
            1,
            vec![CellValue::Number(-100.0), CellValue::Number(100.0)],
            ReferenceKind::Slice,
        ),
        FunctionArg::range(
            2,
            1,
            vec![CellValue::Number(45292.0), CellValue::Number(45292.0)],
            ReferenceKind::Slice,
        ),
    ];
    assert_eq!(
        xirr_func.evaluate(&xirr_same_day_cancel, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // Severe-loss salvage XIRR and same-day huge-notional cancellation preserve precision
    let xirr_salvage = vec![
        FunctionArg::range(
            2,
            1,
            vec![CellValue::Number(-1_000_000.0), CellValue::Number(1.0)],
            ReferenceKind::Slice,
        ),
        args[1].clone(),
    ];
    if let CellValue::Number(rate) = xirr_func.evaluate(&xirr_salvage, &ctx).unwrap() {
        assert!(
            (rate - (-0.999_999)).abs() < 1e-14,
            "Salvage XIRR was {rate}"
        );
    } else {
        panic!("Expected Number for salvage XIRR");
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

    // nper = 0 and rate <= -1.0 return #NUM!
    for bad_pmt_args in [
        vec![
            FunctionArg::Scalar(CellValue::Number(0.05)),
            FunctionArg::Scalar(CellValue::Number(0.0)),
            FunctionArg::Scalar(CellValue::Number(1000.0)),
        ],
        vec![
            FunctionArg::Scalar(CellValue::Number(0.0)),
            FunctionArg::Scalar(CellValue::Number(0.0)),
            FunctionArg::Scalar(CellValue::Number(1000.0)),
        ],
        vec![
            FunctionArg::Scalar(CellValue::Number(-1.0)),
            FunctionArg::Scalar(CellValue::Number(5.0)),
            FunctionArg::Scalar(CellValue::Number(1000.0)),
        ],
        vec![
            FunctionArg::Scalar(CellValue::Number(-1.5)),
            FunctionArg::Scalar(CellValue::Number(4.0)),
            FunctionArg::Scalar(CellValue::Number(1000.0)),
        ],
        vec![
            FunctionArg::Scalar(CellValue::Number(0.0)),
            FunctionArg::Scalar(CellValue::Number(1.0)),
            FunctionArg::Scalar(CellValue::Number(1e308)),
            FunctionArg::Scalar(CellValue::Number(1e308)),
        ],
    ] {
        assert_eq!(
            pmt_func.evaluate(&bad_pmt_args, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num)
        );
    }

    // Non-integer `type` argument in PMT returns #NUM!
    let args_fractional_type = vec![
        FunctionArg::Scalar(CellValue::Number(0.05)),
        FunctionArg::Scalar(CellValue::Number(10.0)),
        FunctionArg::Scalar(CellValue::Number(-1000.0)),
        FunctionArg::Scalar(CellValue::Number(0.0)),
        FunctionArg::Scalar(CellValue::Number(0.5)),
    ];
    assert_eq!(
        pmt_func.evaluate(&args_fractional_type, &ctx),
        Ok(CellValue::Error(ExcelError::Num))
    );

    // Integer `type` outside {0, 1} returns #NUM!
    for bad_type in [2_i64, -1, 5] {
        assert_eq!(
            pmt_func.evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(0.05)),
                    FunctionArg::Scalar(CellValue::Number(10.0)),
                    FunctionArg::Scalar(CellValue::Number(-1000.0)),
                    FunctionArg::Scalar(CellValue::Number(0.0)),
                    FunctionArg::Scalar(CellValue::Number(bad_type as f64)),
                ],
                &ctx,
            ),
            Ok(CellValue::Error(ExcelError::Num))
        );
    }
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

    let res = pmt_func
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(rate)),
                FunctionArg::Scalar(CellValue::Number(nper)),
                FunctionArg::Scalar(CellValue::Number(pv)),
            ],
            &ctx,
        )
        .unwrap();
    let CellValue::Number(pmt_val) = res else {
        panic!("Expected Number, got {res:?}");
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

    // Out of bounds period (per > nper or per < 1), nper = 0, and rate <= -1.0 return #NUM!
    for bad_amort_args in [
        vec![
            FunctionArg::Scalar(CellValue::Number(rate)),
            FunctionArg::Scalar(CellValue::Number(121.0)),
            FunctionArg::Scalar(CellValue::Number(nper)),
            FunctionArg::Scalar(CellValue::Number(pv)),
        ],
        // Fractional per > nper (5.5 > 5.0) must not be masked by truncation
        vec![
            FunctionArg::Scalar(CellValue::Number(0.1)),
            FunctionArg::Scalar(CellValue::Number(5.5)),
            FunctionArg::Scalar(CellValue::Number(5.0)),
            FunctionArg::Scalar(CellValue::Number(-1000.0)),
        ],
        // nper = 0 returns #NUM!
        vec![
            FunctionArg::Scalar(CellValue::Number(0.05)),
            FunctionArg::Scalar(CellValue::Number(1.0)),
            FunctionArg::Scalar(CellValue::Number(0.0)),
            FunctionArg::Scalar(CellValue::Number(-1000.0)),
        ],
        // rate <= -1.0 returns #NUM! even when type = 1 and per = 1
        vec![
            FunctionArg::Scalar(CellValue::Number(-1.0)),
            FunctionArg::Scalar(CellValue::Number(1.0)),
            FunctionArg::Scalar(CellValue::Number(5.0)),
            FunctionArg::Scalar(CellValue::Number(1000.0)),
            FunctionArg::Scalar(CellValue::Number(0.0)),
            FunctionArg::Scalar(CellValue::Number(1.0)),
        ],
    ] {
        assert_eq!(
            ipmt_func.evaluate(&bad_amort_args, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num)
        );
        assert_eq!(
            ppmt_func.evaluate(&bad_amort_args, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num)
        );
    }

    // Fractional per & nper preserve IPMT + PPMT == PMT across type 0 and type 1
    for annuity_type in [0.0, 1.0] {
        let pmt_frac = pmt_func
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(0.1)),
                    FunctionArg::Scalar(CellValue::Number(5.5)),
                    FunctionArg::Scalar(CellValue::Number(-1000.0)),
                    FunctionArg::Scalar(CellValue::Number(0.0)),
                    FunctionArg::Scalar(CellValue::Number(annuity_type)),
                ],
                &ctx,
            )
            .unwrap();
        let amort_frac_args = [
            FunctionArg::Scalar(CellValue::Number(0.1)),
            FunctionArg::Scalar(CellValue::Number(2.5)),
            FunctionArg::Scalar(CellValue::Number(5.5)),
            FunctionArg::Scalar(CellValue::Number(-1000.0)),
            FunctionArg::Scalar(CellValue::Number(0.0)),
            FunctionArg::Scalar(CellValue::Number(annuity_type)),
        ];
        let ipmt_frac = ipmt_func.evaluate(&amort_frac_args, &ctx).unwrap();
        let ppmt_frac = ppmt_func.evaluate(&amort_frac_args, &ctx).unwrap();
        match (pmt_frac, ipmt_frac, ppmt_frac) {
            (CellValue::Number(p), CellValue::Number(i), CellValue::Number(pr)) => {
                assert!(
                    (i + pr - p).abs() < 1e-12,
                    "fractional type {annuity_type}: IPMT ({i}) + PPMT ({pr}) != PMT ({p})"
                );
            }
            other => panic!("Expected Numbers, got {other:?}"),
        }
    }

    // Invalid integer `type` in IPMT and PPMT returns #NUM!
    assert_eq!(
        ipmt_func.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(rate)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
                FunctionArg::Scalar(CellValue::Number(nper)),
                FunctionArg::Scalar(CellValue::Number(pv)),
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(-1.0)),
            ],
            &ctx,
        ),
        Ok(CellValue::Error(ExcelError::Num))
    );
    assert_eq!(
        ppmt_func.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(rate)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
                FunctionArg::Scalar(CellValue::Number(nper)),
                FunctionArg::Scalar(CellValue::Number(pv)),
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(3.0)),
            ],
            &ctx,
        ),
        Ok(CellValue::Error(ExcelError::Num))
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
