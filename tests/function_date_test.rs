#![allow(clippy::unwrap_used, clippy::expect_used)]
use finx::compiler::eval::eval_expr;
use finx::compiler::ir::id::CellId;
use finx::compiler::ir::symbol::SymbolTable;
use finx::expr::ast::Expr;
use finx::functions::date::DateSerial;
use finx::functions::registry::FunctionRegistry;
use finx::functions::traits::{EvalContext, FunctionArg, FunctionCategory};
use finx::model::id::{SheetId, TableId};
use finx::model::table::{ColumnSpec, RowSpec, TableSpec};
use finx::value::{CellValue, ExcelError};
use indexmap::IndexMap;

/// Mock execution context for isolated function unit testing.
struct MockContext;

impl MockContext {
    fn new() -> Self {
        Self
    }
}

impl EvalContext for MockContext {
    fn eval_expr(&self, _expr: &Expr) -> Result<CellValue, finx::functions::FunctionError> {
        Ok(CellValue::Empty)
    }

    fn eval_range(
        &self,
        _expr: &Expr,
    ) -> Result<(usize, usize, Vec<CellValue>), finx::functions::FunctionError> {
        Ok((1, 1, vec![CellValue::Empty]))
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
    assert_eq!(registry.len(), 43);

    let date_funcs = [
        ("DATE", 3, Some(3)),
        ("EDATE", 2, Some(2)),
        ("EOMONTH", 2, Some(2)),
        ("YEARFRAC", 2, Some(3)),
        ("YEAR", 1, Some(1)),
        ("MONTH", 1, Some(1)),
        ("DAY", 1, Some(1)),
        ("DAYS", 2, Some(2)),
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
// Scenario 2: DATE rollover with out-of-range month and day
// ============================================================================

#[test]
fn test_scenario_2_date_rollover_with_out_of_range_month_and_day() {
    let registry = FunctionRegistry::standard();
    let date_fn = registry.lookup("DATE").unwrap();
    let ctx = MockContext::new();

    // 2024-13-01 -> 2025-01-01 (45658.0)
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(2024.0)),
                FunctionArg::Scalar(CellValue::Number(13.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(45658.0));

    // 2024-01-00 -> 2023-12-31 (45291.0)
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(2024.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
                FunctionArg::Scalar(CellValue::Number(0.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(45291.0));

    // 2024-00-01 -> 2023-12-01 (45261.0)
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(2024.0)),
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(45261.0));

    // 2024-01-32 -> 2024-02-01 (45323.0)
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(2024.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
                FunctionArg::Scalar(CellValue::Number(32.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(45323.0));
}

// ============================================================================
// Scenario 3: 2-digit year expansion and bounds checking in DATE
// ============================================================================

#[test]
fn test_scenario_3_two_digit_year_expansion_and_bounds() {
    let registry = FunctionRegistry::standard();
    let date_fn = registry.lookup("DATE").unwrap();
    let ctx = MockContext::new();

    // 100 -> 2000 (2000-01-01 -> 36526.0)
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(100.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(36526.0));

    // 0 -> 1900 (1900-01-01 -> 1.0)
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(1.0));

    // Negative year -> #NUM!
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(-1.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Num));

    // Year >= 10000 -> #NUM!
    let res = date_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(10000.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Num));

    // Infinite and extreme float values -> #NUM!
    for extreme in [f64::INFINITY, f64::NEG_INFINITY, 1e19, -1e19] {
        let res = date_fn
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(extreme)),
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

    // Day zero 0.0 -> 1900-01-00
    let y = year_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(0.0))], &ctx)
        .unwrap();
    let m = month_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(0.0))], &ctx)
        .unwrap();
    let d = day_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(0.0))], &ctx)
        .unwrap();
    assert_eq!(y, CellValue::Number(1900.0));
    assert_eq!(m, CellValue::Number(1.0));
    assert_eq!(d, CellValue::Number(0.0));

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
    let err = year_fn
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(-1.0))], &ctx)
        .unwrap();
    assert_eq!(err, CellValue::Error(ExcelError::Num));
}

// ============================================================================
// Scenario 5: DAYS difference calculation
// ============================================================================

#[test]
fn test_scenario_5_days_difference() {
    let registry = FunctionRegistry::standard();
    let days_fn = registry.lookup("DAYS").unwrap();
    let ctx = MockContext::new();

    // DAYS(25579, 25569) -> 10.0
    let diff = days_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(25579.0)),
                FunctionArg::Scalar(CellValue::Number(25569.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(diff, CellValue::Number(10.0));

    // Reverse order: DAYS(25569, 25579) -> -10.0
    let diff = days_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(25569.0)),
                FunctionArg::Scalar(CellValue::Number(25579.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(diff, CellValue::Number(-10.0));

    // Fractional dates: truncates to integer days
    let diff = days_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(25579.8)),
                FunctionArg::Scalar(CellValue::Number(25569.2)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(diff, CellValue::Number(10.0));
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
    let err = yearfrac_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(start)),
                FunctionArg::Scalar(CellValue::Number(end)),
                FunctionArg::Scalar(CellValue::Number(5.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(err, CellValue::Error(ExcelError::Num));
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
        subtitle: None,
        currency: None,
        columns: vec![
            ColumnSpec::new("c1"),
            ColumnSpec::new("c2"),
            ColumnSpec::new("c3"),
        ],
        column_groups: vec![],
        row_groups: vec![],
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

    // 5. DAYS formula
    let days_eval = eval_str(
        r#"DAYS(Timeline["dates", "c2"], Timeline["dates", "c1"])"#,
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

    // Negative numbers in YEAR, MONTH, DAY must evaluate to #NUM!
    for neg in [-0.5, -0.0001, -1.0, -100.0] {
        let arg = [FunctionArg::Scalar(CellValue::Number(neg))];
        assert_eq!(
            year_fn.evaluate(&arg, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num)
        );
        assert_eq!(
            month_fn.evaluate(&arg, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num)
        );
        assert_eq!(
            day_fn.evaluate(&arg, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num)
        );
    }

    // Negative numbers in EDATE and EOMONTH start_date must evaluate to #NUM!
    let neg_args = [
        FunctionArg::Scalar(CellValue::Number(-0.5)),
        FunctionArg::Scalar(CellValue::Number(1.0)),
    ];
    assert_eq!(
        edate_fn.evaluate(&neg_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );
    assert_eq!(
        eomonth_fn.evaluate(&neg_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
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

    // Numbers strictly beyond 9999-12-31 must return #NUM!
    let beyond_max_args = [FunctionArg::Scalar(CellValue::Number(2958466.0))];
    assert_eq!(
        year_fn.evaluate(&beyond_max_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );
    assert_eq!(
        month_fn.evaluate(&beyond_max_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );
    assert_eq!(
        day_fn.evaluate(&beyond_max_args, &ctx).unwrap(),
        CellValue::Error(ExcelError::Num)
    );
}

#[test]
fn test_days_bounds_validation() {
    let registry = FunctionRegistry::standard();
    let days_fn = registry.lookup("DAYS").unwrap();
    let ctx = MockContext::new();

    // Invalid start or end date outside 0..=2958465 must return #NUM!
    let res = days_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(-10.0)),
                FunctionArg::Scalar(CellValue::Number(10.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Num));

    let res = days_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(10.0)),
                FunctionArg::Scalar(CellValue::Number(-10.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Num));

    let res = days_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(2958466.0)),
                FunctionArg::Scalar(CellValue::Number(10.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Num));

    // Valid fractional timestamps on bounds
    let res = days_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(2958465.75)),
                FunctionArg::Scalar(CellValue::Number(2958460.25)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(5.0));
}

#[test]
fn test_yearfrac_omitted_basis_and_fractional_boundaries() {
    let registry = FunctionRegistry::standard();
    let yearfrac_fn = registry.lookup("YEARFRAC").unwrap();
    let ctx = MockContext::new();

    let start = DateSerial::from_ymd(2024, 1, 1).unwrap();
    let end = DateSerial::from_ymd(2024, 7, 1).unwrap();

    // FunctionArg::Omitted for basis should gracefully default to 0.0 (US 30/360)
    let res = yearfrac_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(start)),
                FunctionArg::Scalar(CellValue::Number(end)),
                FunctionArg::Omitted,
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(0.50));

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

    // Negative dates in YEARFRAC must return #NUM!
    let res = yearfrac_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(-0.5)),
                FunctionArg::Scalar(CellValue::Number(start)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Num));

    // Invalid basis values: negative fractions, out-of-range, non-finite
    for bad_basis in [
        -0.5,
        -1.0,
        5.0,
        5.5,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
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

    // Enormous and non-finite month offsets must return #NUM! rather than wrapping/overflowing
    for extreme_months in [
        1e15,
        1e19,
        f64::INFINITY,
        -1e15,
        -1e19,
        f64::NEG_INFINITY,
        f64::NAN,
    ] {
        let args = [
            FunctionArg::Scalar(CellValue::Number(jan15_2024)),
            FunctionArg::Scalar(CellValue::Number(extreme_months)),
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
    let date_fn = registry.lookup("DATE").unwrap();
    let edate_fn = registry.lookup("EDATE").unwrap();
    let year_fn = registry.lookup("YEAR").unwrap();
    let days_fn = registry.lookup("DAYS").unwrap();
    let yearfrac_fn = registry.lookup("YEARFRAC").unwrap();
    let ctx = MockContext::new();

    // Calling evaluate with too few arguments returns FunctionError::TooFewArguments
    assert!(matches!(
        date_fn.evaluate(&[], &ctx),
        Err(finx::functions::FunctionError::TooFewArguments { .. })
    ));
    assert!(matches!(
        edate_fn.evaluate(&[FunctionArg::Scalar(CellValue::Number(1.0))], &ctx),
        Err(finx::functions::FunctionError::TooFewArguments { .. })
    ));
    assert!(matches!(
        year_fn.evaluate(&[], &ctx),
        Err(finx::functions::FunctionError::TooFewArguments { .. })
    ));
    assert!(matches!(
        days_fn.evaluate(&[FunctionArg::Scalar(CellValue::Number(1.0))], &ctx),
        Err(finx::functions::FunctionError::TooFewArguments { .. })
    ));
    assert!(matches!(
        yearfrac_fn.evaluate(&[FunctionArg::Scalar(CellValue::Number(1.0))], &ctx),
        Err(finx::functions::FunctionError::TooFewArguments { .. })
    ));
}
