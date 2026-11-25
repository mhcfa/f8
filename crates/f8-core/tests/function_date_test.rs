#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]
use f8_core::compiler::eval::eval_expr;
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::ir::symbol::SymbolTable;
use f8_core::expr::ast::Expr;
use f8_core::functions::registry::FunctionRegistry;
use f8_core::functions::traits::{EvalContext, FunctionArg, FunctionCategory};
use f8_core::model::format::FormatPreset;
use f8_core::model::id::{SheetId, TableId};
use f8_core::model::table::{ColumnSpec, RowSpec, TableSpec};
use f8_core::value::format::format_cell_value;
use f8_core::value::{CellValue, DateSerial, ExcelError};
use indexmap::IndexMap;

/// Mock execution context for isolated function unit testing.
struct MockContext;

impl MockContext {
    fn new() -> Self {
        Self
    }
}

impl EvalContext for MockContext {
    fn eval_expr(&self, _expr: &Expr) -> Result<CellValue, f8_core::functions::FunctionError> {
        Ok(CellValue::Empty)
    }
}

/// Helper to parse and evaluate formula strings in an integration context.
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

// ============================================================================
// Registration and Metadata
// ============================================================================

#[test]
fn test_date_functions_registration_and_metadata() {
    let registry = FunctionRegistry::standard();
    assert_eq!(registry.len(), 37);
    assert!(registry.lookup("DAYS").is_none());

    let date_funcs = [
        ("DATE", 3, Some(3)),
        ("EDATE", 2, Some(2)),
        ("EOMONTH", 2, Some(2)),
        ("YEARFRAC", 2, Some(3)),
        ("YEAR", 1, Some(1)),
        ("MONTH", 1, Some(1)),
        ("DAY", 1, Some(1)),
    ];

    for (name, min_args, max_args) in date_funcs {
        let func = registry
            .lookup(name)
            .unwrap_or_else(|| panic!("missing {name}"));
        assert_eq!(func.name(), name);
        assert_eq!(func.category(), FunctionCategory::DateTime);
        assert_eq!(func.min_args(), min_args);
        assert_eq!(func.max_args(), max_args);
    }
}

// ============================================================================
// Scenario 1: DATE constructor and 1900 leap year bug
// ============================================================================

#[test]
fn test_scenario_1_date_constructor_and_1900_leap_year_bug() {
    let registry = FunctionRegistry::standard();
    let date_fn = registry.lookup("DATE").unwrap();
    let ctx = MockContext::new();

    // 1900-01-01 -> 1.0
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(1900.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(1.0));

    // 1900-02-28 -> 59.0
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(1900.0)),
                FunctionArg::Scalar(CellValue::Number(2.0)),
                FunctionArg::Scalar(CellValue::Number(28.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(59.0));

    // Lotus 1-2-3 / Excel leap year bug: 1900-02-29 -> 60.0
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(1900.0)),
                FunctionArg::Scalar(CellValue::Number(2.0)),
                FunctionArg::Scalar(CellValue::Number(29.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(60.0));

    // 1900-03-01 -> 61.0
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(1900.0)),
                FunctionArg::Scalar(CellValue::Number(3.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(61.0));

    // Unix epoch: 1970-01-01 -> 25569.0
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(1970.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(25569.0));

    // Modern leap day: 2024-02-29 -> 45351.0
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(2024.0)),
                FunctionArg::Scalar(CellValue::Number(2.0)),
                FunctionArg::Scalar(CellValue::Number(29.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(45351.0));

    // Maximum date: 9999-12-31 -> 2958465.0
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(9999.0)),
                FunctionArg::Scalar(CellValue::Number(12.0)),
                FunctionArg::Scalar(CellValue::Number(31.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(2958465.0));
}

// ============================================================================
// Scenario 2: DATE rejects out-of-range month and day rollover strictly
// ============================================================================

#[test]
fn test_scenario_2_date_rejects_out_of_range_month_and_day() {
    let registry = FunctionRegistry::standard();
    let date_fn = registry.lookup("DATE").unwrap();
    let ctx = MockContext::new();

    for bad_month in [0_i64, -1, 13, 14] {
        assert_eq!(
            date_fn.evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(2024.0)),
                    FunctionArg::Scalar(CellValue::Number(bad_month as f64)),
                    FunctionArg::Scalar(CellValue::Number(1.0)),
                ],
                &ctx,
            ),
            Ok(CellValue::Error(ExcelError::Num))
        );
    }

    for (y, m, bad_day) in [
        (2024.0, 1.0, 0_i64),
        (2024.0, 1.0, 32),
        (2024.0, 6.0, 31),
        (2025.0, 2.0, 29),
    ] {
        assert_eq!(
            date_fn.evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(y)),
                    FunctionArg::Scalar(CellValue::Number(m)),
                    FunctionArg::Scalar(CellValue::Number(bad_day as f64)),
                ],
                &ctx,
            ),
            Ok(CellValue::Error(ExcelError::Num))
        );
    }

    assert_eq!(
        date_fn
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
                    FunctionArg::Scalar(CellValue::Number(2.0)),
                    FunctionArg::Scalar(CellValue::Number(29.0)),
                ],
                &ctx,
            )
            .unwrap(),
        CellValue::Error(ExcelError::Div0)
    );
}

// ============================================================================
// Scenario 3: 4-digit year enforcement (1900..=9999) in DATE
// ============================================================================

#[test]
fn test_scenario_3_four_digit_year_enforcement_and_bounds() {
    let registry = FunctionRegistry::standard();
    let date_fn = registry.lookup("DATE").unwrap();
    let ctx = MockContext::new();

    for bad_year in [-1_i64, 0, 24, 100, 1899, 10000] {
        assert_eq!(
            date_fn.evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(bad_year as f64)),
                    FunctionArg::Scalar(CellValue::Number(1.0)),
                    FunctionArg::Scalar(CellValue::Number(1.0)),
                ],
                &ctx,
            ),
            Ok(CellValue::Error(ExcelError::Num))
        );
    }

    // Non-finite, fractional, and > MAX_SAFE_INTEGER float values return #NUM!
    for bad_float in [f64::INFINITY, f64::NEG_INFINITY, f64::NAN, 1e19, -1e19, 1.5] {
        let res = date_fn
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(bad_float)),
                    FunctionArg::Scalar(CellValue::Number(1.0)),
                    FunctionArg::Scalar(CellValue::Number(1.0)),
                ],
                &ctx,
            )
            .unwrap();
        assert_eq!(res, CellValue::Error(ExcelError::Num));
    }
}

// ============================================================================
// Scenario 4: Date component extraction (YEAR, MONTH, DAY)
// ============================================================================

#[test]
fn test_scenario_4_date_component_extraction() {
    let registry = FunctionRegistry::standard();
    let year_fn = registry.lookup("YEAR").unwrap();
    let month_fn = registry.lookup("MONTH").unwrap();
    let day_fn = registry.lookup("DAY").unwrap();
    let ctx = MockContext::new();

    // Unix epoch 25569.0 -> 1970-01-01
    let y = year_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(25569.0))], &ctx)
        .unwrap();
    let m = month_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(25569.0))], &ctx)
        .unwrap();
    let d = day_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(25569.0))], &ctx)
        .unwrap();
    assert_eq!(y, CellValue::Number(1970.0));
    assert_eq!(m, CellValue::Number(1.0));
    assert_eq!(d, CellValue::Number(1.0));

    // Lotus leap day 60.0 -> 1900-02-29
    let y = year_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(60.0))], &ctx)
        .unwrap();
    let m = month_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(60.0))], &ctx)
        .unwrap();
    let d = day_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(60.0))], &ctx)
        .unwrap();
    assert_eq!(y, CellValue::Number(1900.0));
    assert_eq!(m, CellValue::Number(2.0));
    assert_eq!(d, CellValue::Number(29.0));

    // Serial 0.0 (fictitious 1900-01-00) returns #NUM! as a date function input...
    for func in [year_fn, month_fn, day_fn] {
        assert_eq!(
            func.evaluate(&[FunctionArg::Scalar(CellValue::Number(0.0))], &ctx),
            Ok(CellValue::Error(ExcelError::Num))
        );
    }
    // ...while FormatPreset::Date display formatting of literal 0.0 retains "1900-01-00" parity
    assert_eq!(
        format_cell_value(
            &CellValue::Number(0.0),
            Some(FormatPreset::Date),
            None,
            None
        ),
        "1900-01-00"
    );

    // Fractional time: 25569.75 (6:00 PM) -> truncates to identical components
    let y = year_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(25569.75))], &ctx)
        .unwrap();
    let m = month_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(25569.75))], &ctx)
        .unwrap();
    let d = day_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(25569.75))], &ctx)
        .unwrap();
    assert_eq!(y, CellValue::Number(1970.0));
    assert_eq!(m, CellValue::Number(1.0));
    assert_eq!(d, CellValue::Number(1.0));

    // Out of bounds: -1.0 -> #NUM!
    assert_eq!(
        year_fn.evaluate(&[FunctionArg::Scalar(CellValue::Number(-1.0))], &ctx),
        Ok(CellValue::Error(ExcelError::Num))
    );
}

// ============================================================================
// Scenario 5: DAYS retired in favor of date serial subtraction
// ============================================================================

#[test]
fn test_scenario_5_days_retired() {
    let registry = FunctionRegistry::standard();
    assert!(registry.lookup("DAYS").is_none());
    let expr = Expr::parse("DAYS(25579, 25569)").unwrap();
    assert_eq!(
        registry.validate_expr(&expr),
        Err(f8_core::functions::FunctionError::UnknownFunction(
            "DAYS".into()
        ))
    );
}

// ============================================================================
// Scenario 6: EDATE month shifting and clamping
// ============================================================================

#[test]
fn test_scenario_6_edate_month_shifting_and_clamping() {
    let registry = FunctionRegistry::standard();
    let edate_fn = registry.lookup("EDATE").unwrap();
    let ctx = MockContext::new();

    let jan31_2024 = DateSerial::from_ymd(2024, 1, 31).unwrap();
    let jan31_2023 = DateSerial::from_ymd(2023, 1, 31).unwrap();
    let mar31_2024 = DateSerial::from_ymd(2024, 3, 31).unwrap();

    // 2024-01-31 + 1 month -> 2024-02-29 (leap year clamped)
    let res = edate_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(jan31_2024)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    let feb29_2024 = DateSerial::from_ymd(2024, 2, 29).unwrap();
    assert_eq!(res, CellValue::Number(feb29_2024));

    // 2023-01-31 + 1 month -> 2023-02-28 (non-leap year clamped)
    let res = edate_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(jan31_2023)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    let feb28_2023 = DateSerial::from_ymd(2023, 2, 28).unwrap();
    assert_eq!(res, CellValue::Number(feb28_2023));

    // 2024-03-31 - 1 month -> 2024-02-29 (leap year clamped backwards)
    let res = edate_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(mar31_2024)),
                FunctionArg::Scalar(CellValue::Number(-1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(feb29_2024));

    // Gregorian Feb 1900 clamping: EDATE clamps 1900-02 to serial 59 (1900-02-28), not serial 60
    let jan31_1900 = DateSerial::from_ymd(1900, 1, 31).unwrap();
    let feb29_1900 = DateSerial::from_ymd(1900, 2, 29).unwrap();
    let mar31_1900 = DateSerial::from_ymd(1900, 3, 31).unwrap();
    assert_eq!(feb29_1900, 60.0);
    for (start_serial, offset) in [(jan31_1900, 1.0), (mar31_1900, -1.0), (feb29_1900, 0.0)] {
        assert_eq!(
            edate_fn
                .evaluate(
                    &[
                        FunctionArg::Scalar(CellValue::Number(start_serial)),
                        FunctionArg::Scalar(CellValue::Number(offset)),
                    ],
                    &ctx,
                )
                .unwrap(),
            CellValue::Number(59.0)
        );
    }
}

// ============================================================================
// Scenario 7: EOMONTH month-end calculation
// ============================================================================

#[test]
fn test_scenario_7_eomonth_calculation() {
    let registry = FunctionRegistry::standard();
    let eomonth_fn = registry.lookup("EOMONTH").unwrap();
    let ctx = MockContext::new();

    let jan15_2024 = DateSerial::from_ymd(2024, 1, 15).unwrap();

    // 2024-01-15 offset 0 -> 2024-01-31
    let res = eomonth_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(jan15_2024)),
                FunctionArg::Scalar(CellValue::Number(0.0)),
            ],
            &ctx,
        )
        .unwrap();
    let jan31_2024 = DateSerial::from_ymd(2024, 1, 31).unwrap();
    assert_eq!(res, CellValue::Number(jan31_2024));

    // 2024-01-15 offset 1 -> 2024-02-29 (leap year)
    let res = eomonth_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(jan15_2024)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    let feb29_2024 = DateSerial::from_ymd(2024, 2, 29).unwrap();
    assert_eq!(res, CellValue::Number(feb29_2024));

    // 2024-01-15 offset -1 -> 2023-12-31
    let res = eomonth_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(jan15_2024)),
                FunctionArg::Scalar(CellValue::Number(-1.0)),
            ],
            &ctx,
        )
        .unwrap();
    let dec31_2023 = DateSerial::from_ymd(2023, 12, 31).unwrap();
    assert_eq!(res, CellValue::Number(dec31_2023));

    // Gregorian Feb 1900 end-of-month: EOMONTH returns serial 59 (1900-02-28), including EOMONTH(60, 0)
    let jan15_1900 = DateSerial::from_ymd(1900, 1, 15).unwrap();
    let feb01_1900 = DateSerial::from_ymd(1900, 2, 1).unwrap();
    let feb29_1900 = DateSerial::from_ymd(1900, 2, 29).unwrap();
    for (start_serial, offset) in [(jan15_1900, 1.0), (feb01_1900, 0.0), (feb29_1900, 0.0)] {
        assert_eq!(
            eomonth_fn
                .evaluate(
                    &[
                        FunctionArg::Scalar(CellValue::Number(start_serial)),
                        FunctionArg::Scalar(CellValue::Number(offset)),
                    ],
                    &ctx,
                )
                .unwrap(),
            CellValue::Number(59.0)
        );
    }
}

// ============================================================================
// Scenario 8: YEARFRAC across all 5 basis conventions
// ============================================================================

#[test]
fn test_scenario_8_yearfrac_bases() {
    let registry = FunctionRegistry::standard();
    let yearfrac_fn = registry.lookup("YEARFRAC").unwrap();
    let ctx = MockContext::new();

    let start = DateSerial::from_ymd(2024, 1, 1).unwrap();
    let end = DateSerial::from_ymd(2024, 7, 1).unwrap();

    // Basis 0: US 30/360 -> 180 / 360 = 0.50
    let res = yearfrac_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(start)),
                FunctionArg::Scalar(CellValue::Number(end)),
                FunctionArg::Scalar(CellValue::Number(0.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(0.50));

    // Basis 0 default when omitted -> 0.50
    let res = yearfrac_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(start)),
                FunctionArg::Scalar(CellValue::Number(end)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(0.50));

    // Basis 1: Actual/Actual in leap year 2024 -> 182 / 366
    let res = yearfrac_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(start)),
                FunctionArg::Scalar(CellValue::Number(end)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    if let CellValue::Number(n) = res {
        assert!((n - (182.0 / 366.0)).abs() < 1e-12);
    } else {
        panic!("expected number");
    }

    // Basis 2: Actual/360 -> 182 / 360
    let res = yearfrac_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(start)),
                FunctionArg::Scalar(CellValue::Number(end)),
                FunctionArg::Scalar(CellValue::Number(2.0)),
            ],
            &ctx,
        )
        .unwrap();
    if let CellValue::Number(n) = res {
        assert!((n - (182.0 / 360.0)).abs() < 1e-12);
    } else {
        panic!("expected number");
    }

    // Basis 3: Actual/365 -> 182 / 365
    let res = yearfrac_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(start)),
                FunctionArg::Scalar(CellValue::Number(end)),
                FunctionArg::Scalar(CellValue::Number(3.0)),
            ],
            &ctx,
        )
        .unwrap();
    if let CellValue::Number(n) = res {
        assert!((n - (182.0 / 365.0)).abs() < 1e-12);
    } else {
        panic!("expected number");
    }

    // Basis 4: European 30/360 -> 180 / 360 = 0.50
    let res = yearfrac_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(start)),
                FunctionArg::Scalar(CellValue::Number(end)),
                FunctionArg::Scalar(CellValue::Number(4.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(0.50));

    // Basis 0 US (NASD) 30/360 rule ordering: d2=31 checks original d1 >= 30 before end-of-Feb adjustment
    for ((y1, m1, d1), (y2, m2, d2), expected_num_days) in [
        ((2023, 2, 28), (2023, 3, 31), 31.0),
        ((2024, 2, 29), (2024, 3, 31), 31.0),
        ((1900, 2, 29), (1900, 3, 31), 31.0),
        ((1900, 2, 28), (1900, 3, 31), 33.0),
        ((2023, 2, 28), (2024, 2, 29), 360.0),
        ((2023, 1, 31), (2023, 2, 28), 28.0),
        ((2023, 1, 31), (2023, 3, 31), 60.0),
        ((2023, 4, 30), (2023, 5, 31), 30.0),
    ] {
        let s = DateSerial::from_ymd(y1, m1, d1).unwrap();
        let e = DateSerial::from_ymd(y2, m2, d2).unwrap();
        let CellValue::Number(actual) = yearfrac_fn
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(s)),
                    FunctionArg::Scalar(CellValue::Number(e)),
                    FunctionArg::Scalar(CellValue::Number(0.0)),
                ],
                &ctx,
            )
            .unwrap()
        else {
            panic!("expected Number for YEARFRAC ({y1}-{m1}-{d1}..{y2}-{m2}-{d2})");
        };
        let expected = expected_num_days / 360.0;
        assert!(
            (actual - expected).abs() < 1e-12,
            "YEARFRAC Basis 0 ({y1}-{m1}-{d1}..{y2}-{m2}-{d2}): got {actual}, expected {expected}"
        );
    }

    // Order independence: start > end gives identical fraction
    let rev_res = yearfrac_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(end)),
                FunctionArg::Scalar(CellValue::Number(start)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(0.50));
    if let CellValue::Number(n) = rev_res {
        assert!((n - (182.0 / 366.0)).abs() < 1e-12);
    }

    // Invalid basis: 5 -> #NUM!
    assert_eq!(
        yearfrac_fn.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(start)),
                FunctionArg::Scalar(CellValue::Number(end)),
                FunctionArg::Scalar(CellValue::Number(5.0)),
            ],
            &ctx,
        ),
        Ok(CellValue::Error(ExcelError::Num))
    );
}

// ============================================================================
// End-to-End Formula Integration
// ============================================================================

#[test]
fn test_end_to_end_date_formula_evaluation() {
    let mut symbols = SymbolTable::new();
    let sheet_id = SheetId::from("sheet1");

    let dates_table = TableSpec {
        id: TableId::from("Timeline"),
        title: "Timeline".into(),
        subtitles: Vec::new(),
        currency: None,
        collapsible: false,
        columns: vec![
            ColumnSpec::new("c1"),
            ColumnSpec::new("c2"),
            ColumnSpec::new("c3"),
        ],
        rows: vec![RowSpec::new("dates"), RowSpec::new("fractions")],
    };
    symbols
        .register_table(sheet_id, &dates_table)
        .expect("registered");

    let registry = FunctionRegistry::standard();
    let mut values = IndexMap::new();

    let c_d1 = CellId::new("Timeline", "dates", "c1");
    let c_d2 = CellId::new("Timeline", "dates", "c2");
    let c_d3 = CellId::new("Timeline", "dates", "c3");

    // c1: 2024-01-01 (45292)
    // c2: 2024-07-01 (45474)
    // c3: 2025-01-01 (45658)
    values.insert(c_d1, CellValue::Number(45292.0));
    values.insert(c_d2, CellValue::Number(45474.0));
    values.insert(c_d3, CellValue::Number(45658.0));

    let origin = CellId::new("Timeline", "fractions", "c1");

    // 1. DATE constructor formula
    let date_eval = eval_str("DATE(2024, 1, 1)", &symbols, &values, &registry, &origin);
    assert_eq!(date_eval, CellValue::Number(45292.0));

    // 2. EDATE formula
    let edate_eval = eval_str(
        r#"EDATE(Timeline["dates", "c1"], 6)"#,
        &symbols,
        &values,
        &registry,
        &origin,
    );
    assert_eq!(edate_eval, CellValue::Number(45474.0)); // 2024-07-01

    // 3. EOMONTH formula
    let eomonth_eval = eval_str(
        r#"EOMONTH(Timeline["dates", "c1"], 1)"#,
        &symbols,
        &values,
        &registry,
        &origin,
    );
    assert_eq!(eomonth_eval, CellValue::Number(45351.0)); // 2024-02-29

    // 4. YEARFRAC formula
    let yf_eval = eval_str(
        r#"YEARFRAC(Timeline["dates", "c1"], Timeline["dates", "c2"], 0)"#,
        &symbols,
        &values,
        &registry,
        &origin,
    );
    assert_eq!(yf_eval, CellValue::Number(0.50));

    // 5. Date serial subtraction formula
    let days_eval = eval_str(
        r#"Timeline["dates", "c2"] - Timeline["dates", "c1"]"#,
        &symbols,
        &values,
        &registry,
        &origin,
    );
    assert_eq!(days_eval, CellValue::Number(182.0));

    // 6. Component extractors
    let year_eval = eval_str(
        r#"YEAR(Timeline["dates", "c1"])"#,
        &symbols,
        &values,
        &registry,
        &origin,
    );
    assert_eq!(year_eval, CellValue::Number(2024.0));

    let month_eval = eval_str(
        r#"MONTH(Timeline["dates", "c2"])"#,
        &symbols,
        &values,
        &registry,
        &origin,
    );
    assert_eq!(month_eval, CellValue::Number(7.0));

    let day_eval = eval_str(
        r#"DAY(Timeline["dates", "c1"])"#,
        &symbols,
        &values,
        &registry,
        &origin,
    );
    assert_eq!(day_eval, CellValue::Number(1.0));

    // 7. Upstream #NUM! from EDATE/EOMONTH stepping outside 1900..=9999 bubbles as CellValue::Error(Num)
    assert_eq!(
        eval_str(
            "EDATE(DATE(1900, 1, 15), -2)",
            &symbols,
            &values,
            &registry,
            &origin
        ),
        CellValue::Error(ExcelError::Num)
    );
    assert_eq!(
        eval_str(
            "EOMONTH(DATE(9999, 12, 1), 2)",
            &symbols,
            &values,
            &registry,
            &origin
        ),
        CellValue::Error(ExcelError::Num)
    );
    assert_eq!(
        eval_str(
            "YEAR(EDATE(DATE(1900, 1, 15), -2))",
            &symbols,
            &values,
            &registry,
            &origin
        ),
        CellValue::Error(ExcelError::Num)
    );

    // 8. Dynamic cell references and constant literals evaluate out-of-range CalendarDate and DateSerial to #NUM!
    values.insert(
        CellId::new("Timeline", "fractions", "c2"),
        CellValue::Number(24.0),
    );
    values.insert(
        CellId::new("Timeline", "fractions", "c3"),
        CellValue::Number(0.0),
    );
    assert_eq!(
        eval_str(
            r#"IFERROR(DATE(Timeline["fractions", "c2"], 1, 1), 0)"#,
            &symbols,
            &values,
            &registry,
            &origin,
        ),
        CellValue::Number(0.0)
    );
    for (formula, expected_val) in [
        (
            r#"IF(TRUE, DATE(Timeline["fractions", "c2"], 1, 1), Timeline["dates", "c1"])"#,
            CellValue::Error(ExcelError::Num),
        ),
        (
            r#"IF(FALSE, DATE(24, 1, 1), Timeline["dates", "c1"])"#,
            CellValue::Number(45292.0),
        ),
        (
            r#"DATE(Timeline["fractions", "c2"], 1 / 0, 1)"#,
            CellValue::Error(ExcelError::Div0),
        ),
        (r#"DATE(1 / 0, 2, 30)"#, CellValue::Error(ExcelError::Div0)),
        (
            r#"IF(TRUE, YEAR(Timeline["fractions", "c3"]), 2024)"#,
            CellValue::Error(ExcelError::Num),
        ),
        (r#"IF(FALSE, YEAR(0), 2024)"#, CellValue::Number(2024.0)),
    ] {
        assert_eq!(
            eval_str(formula, &symbols, &values, &registry, &origin),
            expected_val,
            "mismatch on {formula}"
        );
    }

    assert_eq!(
        eval_str(
            r#"IF(FALSE, DATE(Timeline["fractions", "c2"], 1, 1), Timeline["dates", "c1"])"#,
            &symbols,
            &values,
            &registry,
            &origin,
        ),
        CellValue::Number(45292.0)
    );
    assert_eq!(
        eval_str(
            r#"IF(Timeline["fractions", "c3"] >= 1, YEAR(Timeline["fractions", "c3"]), 2024)"#,
            &symbols,
            &values,
            &registry,
            &origin,
        ),
        CellValue::Number(2024.0)
    );

    // Dynamic day=29 with year=2025, month=2 returns #NUM!, while year=2024 succeeds
    values.insert(
        CellId::new("Timeline", "fractions", "c2"),
        CellValue::Number(29.0),
    );
    assert_eq!(
        eval_str(
            r#"DATE(2025, 2, Timeline["fractions", "c2"])"#,
            &symbols,
            &values,
            &registry,
            &origin,
        ),
        CellValue::Error(ExcelError::Num)
    );
    assert_eq!(
        eval_str(
            r#"DATE(2024, 2, Timeline["fractions", "c2"])"#,
            &symbols,
            &values,
            &registry,
            &origin,
        ),
        CellValue::Number(45351.0)
    );
    assert_eq!(
        eval_str("DATE(1 / 0, 2, 29)", &symbols, &values, &registry, &origin),
        CellValue::Error(ExcelError::Div0)
    );
}

// ============================================================================
// Edge Cases & Robustness Verification
// ============================================================================

#[test]
fn test_negative_dates_and_fractional_boundaries() {
    let registry = FunctionRegistry::standard();
    let year_fn = registry.lookup("YEAR").unwrap();
    let month_fn = registry.lookup("MONTH").unwrap();
    let day_fn = registry.lookup("DAY").unwrap();
    let edate_fn = registry.lookup("EDATE").unwrap();
    let eomonth_fn = registry.lookup("EOMONTH").unwrap();
    let ctx = MockContext::new();

    // Sub-1.0 and negative numbers in YEAR, MONTH, DAY return #NUM!
    for bad_serial in [0.0, 0.5, -0.5, -0.0001, -1.0, -100.0] {
        let arg = [FunctionArg::Scalar(CellValue::Number(bad_serial))];
        for func in [year_fn, month_fn, day_fn] {
            assert_eq!(
                func.evaluate(&arg, &ctx),
                Ok(CellValue::Error(ExcelError::Num))
            );
        }
    }

    // Sub-1.0 or negative numbers in EDATE and EOMONTH start_date return #NUM!
    let neg_args = [
        FunctionArg::Scalar(CellValue::Number(-0.5)),
        FunctionArg::Scalar(CellValue::Number(1.0)),
    ];
    assert_eq!(
        edate_fn.evaluate(&neg_args, &ctx),
        Ok(CellValue::Error(ExcelError::Num))
    );
    assert_eq!(
        eomonth_fn.evaluate(&neg_args, &ctx),
        Ok(CellValue::Error(ExcelError::Num))
    );

    // Fractional timestamps on maximum date (9999-12-31 at 6:00 PM) must be accepted
    let max_time_args = [FunctionArg::Scalar(CellValue::Number(2958465.75))];
    assert_eq!(
        year_fn.evaluate(&max_time_args, &ctx).unwrap(),
        CellValue::Number(9999.0)
    );
    assert_eq!(
        month_fn.evaluate(&max_time_args, &ctx).unwrap(),
        CellValue::Number(12.0)
    );
    assert_eq!(
        day_fn.evaluate(&max_time_args, &ctx).unwrap(),
        CellValue::Number(31.0)
    );

    // Numbers strictly beyond 9999-12-31 return #NUM!
    let beyond_max_args = [FunctionArg::Scalar(CellValue::Number(2958466.0))];
    for func in [year_fn, month_fn, day_fn] {
        assert_eq!(
            func.evaluate(&beyond_max_args, &ctx),
            Ok(CellValue::Error(ExcelError::Num))
        );
    }
}

#[test]
fn test_yearfrac_omitted_basis_and_fractional_boundaries() {
    let registry = FunctionRegistry::standard();
    let yearfrac_fn = registry.lookup("YEARFRAC").unwrap();
    let ctx = MockContext::new();

    let start = DateSerial::from_ymd(2024, 1, 1).unwrap();
    let end = DateSerial::from_ymd(2024, 7, 1).unwrap();

    // 2-argument YEARFRAC defaults basis to 0.0 (US 30/360)
    let res = yearfrac_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(start)),
                FunctionArg::Scalar(CellValue::Number(end)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(0.50));

    // Trailing omission for final optional basis is rejected statically
    let expr = f8_core::expr::Expr::parse("YEARFRAC(45292, 45474, _)").unwrap();
    let err = registry.validate_expr(&expr).unwrap_err();
    assert_eq!(
        err,
        f8_core::functions::FunctionError::CannotOmitTrailingArgument {
            function: "YEARFRAC",
            param: "basis",
            position: 3,
        }
    );

    // Fractional timestamps on max date (9999-12-31 at noon) must be valid
    let res = yearfrac_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(2958465.5)),
                FunctionArg::Scalar(CellValue::Number(2958465.0)),
                FunctionArg::Scalar(CellValue::Number(0.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(0.0));

    // Sub-1.0 or negative dates in YEARFRAC return #NUM!
    assert_eq!(
        yearfrac_fn.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(-0.5)),
                FunctionArg::Scalar(CellValue::Number(start)),
            ],
            &ctx,
        ),
        Ok(CellValue::Error(ExcelError::Num))
    );

    // Out-of-domain, non-finite, fractional, or > MAX_SAFE_INTEGER basis values return #NUM!
    for bad_basis in [
        -1.0,
        5.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        -0.5,
        5.5,
        1e19,
    ] {
        let res = yearfrac_fn
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(start)),
                    FunctionArg::Scalar(CellValue::Number(end)),
                    FunctionArg::Scalar(CellValue::Number(bad_basis)),
                ],
                &ctx,
            )
            .unwrap();
        assert_eq!(res, CellValue::Error(ExcelError::Num));
    }
}

#[test]
fn test_edate_eomonth_overflow_protection() {
    let registry = FunctionRegistry::standard();
    let edate_fn = registry.lookup("EDATE").unwrap();
    let eomonth_fn = registry.lookup("EOMONTH").unwrap();
    let ctx = MockContext::new();

    let jan15_2024 = DateSerial::from_ymd(2024, 1, 15).unwrap();

    // Large safe-integer, non-finite, fractional, and > MAX_SAFE_INTEGER month offsets return #NUM!
    for num_err_months in [
        1e15,
        -1e15,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        1e19,
        -1e19,
        1.5,
    ] {
        let args = [
            FunctionArg::Scalar(CellValue::Number(jan15_2024)),
            FunctionArg::Scalar(CellValue::Number(num_err_months)),
        ];
        assert_eq!(
            edate_fn.evaluate(&args, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num)
        );
        assert_eq!(
            eomonth_fn.evaluate(&args, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num)
        );
    }
}

#[test]
fn test_date_functions_arity_bounds() {
    let registry = FunctionRegistry::standard();

    // Calling validate_expr with too few arguments returns FunctionError::TooFewArguments
    for formula in ["DATE()", "EDATE(1)", "YEAR()", "YEARFRAC(1)"] {
        let expr = Expr::parse(formula).unwrap();
        assert!(matches!(
            registry.validate_expr(&expr),
            Err(f8_core::functions::FunctionError::TooFewArguments { .. })
        ));
    }

    // Eager ExcelError in later arguments bubbles before domain #NUM! in DATE, EDATE, EOMONTH
    let ctx = MockContext::new();
    let date_fn = registry.lookup("DATE").unwrap();
    let edate_fn = registry.lookup("EDATE").unwrap();
    let eomonth_fn = registry.lookup("EOMONTH").unwrap();

    assert_eq!(
        date_fn
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(1.5)),
                    FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
                    FunctionArg::Scalar(CellValue::Number(1.0)),
                ],
                &ctx,
            )
            .unwrap(),
        CellValue::Error(ExcelError::Div0)
    );
    assert_eq!(
        edate_fn
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(-1.0)),
                    FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
                ],
                &ctx,
            )
            .unwrap(),
        CellValue::Error(ExcelError::Div0)
    );
    assert_eq!(
        eomonth_fn
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(0.0)),
                    FunctionArg::Scalar(CellValue::Error(ExcelError::Na)),
                ],
                &ctx,
            )
            .unwrap(),
        CellValue::Error(ExcelError::Na)
    );
}
