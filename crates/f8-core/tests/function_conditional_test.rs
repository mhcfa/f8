//! Integration tests for Excel conditional aggregation functions (*IFS family).
//!
//! Covers `SUMIFS`, `COUNTIFS`, `AVERAGEIFS`, `MINIFS`, and `MAXIFS` conforming to
//! Cluster C typed criteria semantics and Excel calculation parity.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use f8_core::expr::ast::{BlankCheck, CriteriaComparator, Expr};
use f8_core::functions::{
    BoundCriterion, EvalContext, FunctionArg, FunctionCategory, FunctionError, FunctionRegistry,
    ReferenceKind,
};
use f8_core::model::table::AxisRole;
use f8_core::value::{
    CellValue, ExcelError, ExpectedType, TextCriterionError, TypeError, ValueType,
};

/// Mock evaluation context for testing function execution.
struct TestEvalContext;

impl EvalContext for TestEvalContext {
    fn eval_expr(&self, _expr: &Expr) -> Result<CellValue, FunctionError> {
        Ok(CellValue::Empty)
    }

    fn eval_range(&self, _expr: &Expr) -> Result<(usize, usize, Vec<CellValue>), FunctionError> {
        Ok((0, 0, Vec::new()))
    }
}

fn make_array(rows: usize, cols: usize, values: Vec<CellValue>) -> FunctionArg<'static> {
    FunctionArg::array(rows, cols, values, ReferenceKind::Slice, AxisRole::Measure)
}

fn exact_crit(val: CellValue) -> FunctionArg<'static> {
    FunctionArg::Criterion(BoundCriterion::Exact(val))
}

fn cmp_crit(op: CriteriaComparator, val: CellValue) -> FunctionArg<'static> {
    FunctionArg::Criterion(BoundCriterion::Comparator { op, value: val })
}

fn blank_crit(check: BlankCheck) -> FunctionArg<'static> {
    FunctionArg::Criterion(BoundCriterion::BlankCheck(check))
}

// ============================================================================
// Scenario 1: SUMIFS multi-criteria evaluation with typed numeric and text criteria
// ============================================================================

#[test]
fn test_scenario_1_sumifs_multi_criteria_numeric_and_text() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let ctx = TestEvalContext;

    let sum_range = make_array(
        1,
        4,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
            CellValue::Number(40.0),
        ],
    );
    let cr1 = make_array(
        1,
        4,
        vec![
            CellValue::Text("A".into()),
            CellValue::Text("B".into()),
            CellValue::Text("A".into()),
            CellValue::Text("B".into()),
        ],
    );
    let crit1 = exact_crit(CellValue::Text("A".into()));
    let cr2 = make_array(
        1,
        4,
        vec![
            CellValue::Number(1.0),
            CellValue::Number(2.0),
            CellValue::Number(1.0),
            CellValue::Number(2.0),
        ],
    );
    let crit2 = cmp_crit(CriteriaComparator::Gt, CellValue::Number(0.0));

    let res = sumifs
        .evaluate(&[sum_range, cr1, crit1, cr2, crit2], &ctx)
        .unwrap();
    assert_eq!(res, CellValue::Number(40.0));
}

#[test]
fn test_sumifs_sparse_empty_allowed_and_non_numeric_or_descriptor_rejected() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let avgifs = registry.lookup("AVERAGEIFS").unwrap();
    let minifs = registry.lookup("MINIFS").unwrap();
    let maxifs = registry.lookup("MAXIFS").unwrap();
    let ctx = TestEvalContext;

    // Sparse Empty in syntactic Slice target range is skipped cleanly
    let sparse_target = make_array(
        1,
        4,
        vec![
            CellValue::Number(100.0),
            CellValue::Empty,
            CellValue::Number(50.0),
            CellValue::Empty,
        ],
    );
    let cr_desc = FunctionArg::array(
        1,
        4,
        vec![
            CellValue::Text("Alpha".into()),
            CellValue::Text("Alpha".into()),
            CellValue::Text("Alpha".into()),
            CellValue::Text("Beta".into()),
        ],
        ReferenceKind::Slice,
        AxisRole::Descriptor,
    );
    let crit_alpha = exact_crit(CellValue::Text("Alpha".into()));

    assert_eq!(
        sumifs
            .evaluate(
                &[sparse_target.clone(), cr_desc.clone(), crit_alpha.clone()],
                &ctx
            )
            .unwrap(),
        CellValue::Number(150.0)
    );
    assert_eq!(
        avgifs
            .evaluate(
                &[sparse_target.clone(), cr_desc.clone(), crit_alpha.clone()],
                &ctx
            )
            .unwrap(),
        CellValue::Number(75.0)
    );
    assert_eq!(
        minifs
            .evaluate(
                &[sparse_target.clone(), cr_desc.clone(), crit_alpha.clone()],
                &ctx
            )
            .unwrap(),
        CellValue::Number(50.0)
    );
    assert_eq!(
        maxifs
            .evaluate(&[sparse_target, cr_desc.clone(), crit_alpha.clone()], &ctx)
            .unwrap(),
        CellValue::Number(100.0)
    );

    // Text or Boolean in target_range fails eagerly even when its row is unmatched
    for (bad_cell, found_type) in [
        (CellValue::Text("text".into()), ValueType::Text),
        (CellValue::Boolean(true), ValueType::Boolean),
    ] {
        let mixed_target = make_array(
            1,
            4,
            vec![
                CellValue::Number(100.0),
                CellValue::Number(50.0),
                CellValue::Empty,
                bad_cell,
            ],
        );
        for (fn_name, param_name) in [
            ("SUMIFS", "sum_range"),
            ("AVERAGEIFS", "average_range"),
            ("MINIFS", "min_range"),
            ("MAXIFS", "max_range"),
        ] {
            let func = registry.lookup(fn_name).unwrap();
            assert_eq!(
                func.evaluate(
                    &[mixed_target.clone(), cr_desc.clone(), crit_alpha.clone()],
                    &ctx,
                ),
                Err(FunctionError::Type(TypeError::InvalidArgumentType {
                    function: fn_name,
                    param: param_name,
                    position: 1,
                    expected: ExpectedType::Number,
                    found: found_type,
                }))
            );
        }
    }

    // SingleCell Empty in target_range is rejected, while SingleCell Empty in criteria_range is allowed with BLANK
    let single_empty_target = FunctionArg::array(
        1,
        1,
        vec![CellValue::Empty],
        ReferenceKind::SingleCell,
        AxisRole::Measure,
    );
    let single_crit_range = FunctionArg::array(
        1,
        1,
        vec![CellValue::Empty],
        ReferenceKind::SingleCell,
        AxisRole::Descriptor,
    );
    assert_eq!(
        sumifs.evaluate(
            &[
                single_empty_target,
                single_crit_range.clone(),
                blank_crit(BlankCheck::Blank),
            ],
            &ctx,
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "SUMIFS",
            param: "sum_range",
            position: 1,
            expected: ExpectedType::Number,
            found: ValueType::Empty,
        }))
    );
    let single_num_target = FunctionArg::array(
        1,
        1,
        vec![CellValue::Number(42.0)],
        ReferenceKind::SingleCell,
        AxisRole::Measure,
    );
    assert_eq!(
        sumifs
            .evaluate(
                &[
                    single_num_target,
                    single_crit_range,
                    blank_crit(BlankCheck::Blank),
                ],
                &ctx,
            )
            .unwrap(),
        CellValue::Number(42.0)
    );

    // Descriptor target_range fails with DescriptorReferenceNotAllowed
    let desc_target = FunctionArg::array(
        1,
        4,
        vec![CellValue::Number(10.0); 4],
        ReferenceKind::Slice,
        AxisRole::Descriptor,
    );
    assert_eq!(
        sumifs.evaluate(&[desc_target, cr_desc, crit_alpha.clone()], &ctx),
        Err(FunctionError::DescriptorReferenceNotAllowed {
            function: "SUMIFS",
            param: "sum_range",
            position: 1,
        })
    );

    // Overflow in SUMIFS / AVERAGEIFS routes through CellValue::number -> #NUM!
    let extreme_target = make_array(
        1,
        2,
        vec![CellValue::Number(1e308), CellValue::Number(1e308)],
    );
    let cr_two = make_array(
        1,
        2,
        vec![
            CellValue::Text("Alpha".into()),
            CellValue::Text("Alpha".into()),
        ],
    );
    assert_eq!(
        sumifs
            .evaluate(
                &[extreme_target.clone(), cr_two.clone(), crit_alpha.clone()],
                &ctx
            )
            .unwrap(),
        CellValue::Error(ExcelError::Num)
    );
    assert_eq!(
        avgifs
            .evaluate(&[extreme_target, cr_two, crit_alpha], &ctx)
            .unwrap(),
        CellValue::Error(ExcelError::Num)
    );
}

// ============================================================================
// Scenario 2: COUNTIFS multi-criteria counting with typed comparators
// ============================================================================

#[test]
fn test_scenario_2_countifs_multi_criteria_counting() {
    let registry = FunctionRegistry::standard();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    let cr1 = make_array(
        1,
        4,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
            CellValue::Number(40.0),
        ],
    );
    let crit1 = cmp_crit(CriteriaComparator::Gte, CellValue::Number(20.0));
    let cr2 = make_array(
        1,
        4,
        vec![
            CellValue::Text("East".into()),
            CellValue::Text("West".into()),
            CellValue::Text("East".into()),
            CellValue::Text("North".into()),
        ],
    );
    let crit2 = cmp_crit(CriteriaComparator::NotEq, CellValue::Text("North".into()));

    let res = countifs.evaluate(&[cr1, crit1, cr2, crit2], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(2.0)); // positions 2 and 3 match
}

// ============================================================================
// Scenario 3: AVERAGEIFS empty match #DIV/0! fallback & case-insensitive text
// ============================================================================

#[test]
fn test_scenario_3_averageifs_empty_match_div0_fallback() {
    let registry = FunctionRegistry::standard();
    let avgifs = registry.lookup("AVERAGEIFS").unwrap();
    let ctx = TestEvalContext;

    let avg_range = make_array(
        1,
        3,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ],
    );
    let cr1 = make_array(
        1,
        3,
        vec![
            CellValue::Text("X".into()),
            CellValue::Text("Y".into()),
            CellValue::Text("Z".into()),
        ],
    );
    let crit1 = exact_crit(CellValue::Text("W".into()));

    let res = avgifs.evaluate(&[avg_range, cr1, crit1], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Div0));
}

#[test]
fn test_averageifs_successful_mean() {
    let registry = FunctionRegistry::standard();
    let avgifs = registry.lookup("AVERAGEIFS").unwrap();
    let ctx = TestEvalContext;

    let avg_range = make_array(
        1,
        4,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
            CellValue::Number(40.0),
        ],
    );
    let cr1 = make_array(
        1,
        4,
        vec![
            CellValue::Text("Tech".into()),
            CellValue::Text("Sales".into()),
            CellValue::Text("Tech".into()),
            CellValue::Text("Ops".into()),
        ],
    );
    let crit1 = exact_crit(CellValue::Text("tech".into()));

    let res = avgifs.evaluate(&[avg_range, cr1, crit1], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(20.0));
}

// ============================================================================
// Scenario 4: MINIFS and MAXIFS zero fallback and calculation
// ============================================================================

#[test]
fn test_scenario_4_minifs_and_maxifs_zero_fallback_on_no_match() {
    let registry = FunctionRegistry::standard();
    let minifs = registry.lookup("MINIFS").unwrap();
    let maxifs = registry.lookup("MAXIFS").unwrap();
    let ctx = TestEvalContext;

    let data = make_array(
        1,
        3,
        vec![
            CellValue::Number(100.0),
            CellValue::Number(200.0),
            CellValue::Number(300.0),
        ],
    );
    let cr = make_array(
        1,
        3,
        vec![
            CellValue::Text("Red".into()),
            CellValue::Text("Blue".into()),
            CellValue::Text("Green".into()),
        ],
    );
    let crit = exact_crit(CellValue::Text("Yellow".into()));

    assert_eq!(
        minifs
            .evaluate(&[data.clone(), cr.clone(), crit.clone()], &ctx)
            .unwrap(),
        CellValue::Number(0.0)
    );
    assert_eq!(
        maxifs.evaluate(&[data, cr, crit], &ctx).unwrap(),
        CellValue::Number(0.0)
    );
}

#[test]
fn test_minifs_and_maxifs_calculation() {
    let registry = FunctionRegistry::standard();
    let minifs = registry.lookup("MINIFS").unwrap();
    let maxifs = registry.lookup("MAXIFS").unwrap();
    let ctx = TestEvalContext;

    let data = make_array(
        1,
        4,
        vec![
            CellValue::Number(50.0),
            CellValue::Number(10.0),
            CellValue::Number(80.0),
            CellValue::Number(30.0),
        ],
    );
    let cr = make_array(
        1,
        4,
        vec![
            CellValue::Text("DeptA".into()),
            CellValue::Text("DeptB".into()),
            CellValue::Text("DeptA".into()),
            CellValue::Text("DeptA".into()),
        ],
    );
    let crit = exact_crit(CellValue::Text("DeptA".into()));

    assert_eq!(
        minifs
            .evaluate(&[data.clone(), cr.clone(), crit.clone()], &ctx)
            .unwrap(),
        CellValue::Number(30.0)
    );
    assert_eq!(
        maxifs.evaluate(&[data, cr, crit], &ctx).unwrap(),
        CellValue::Number(80.0)
    );
}

// ============================================================================
// Scenario 5: BLANK / NOT_BLANK, 15-sig-digit precision, and <> Empty/Error parity
// ============================================================================

#[test]
fn test_blank_and_not_blank_strict_empty_vs_empty_string_parity() {
    let registry = FunctionRegistry::standard();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    // Homogeneous text range with Empty, "Alpha", "", and Error(Div0)
    let cr = make_array(
        1,
        4,
        vec![
            CellValue::Empty,
            CellValue::Text("Alpha".into()),
            CellValue::Text(String::new()),
            CellValue::Error(ExcelError::Div0),
        ],
    );

    // BLANK ("=") matches strictly CellValue::Empty (not "")
    assert_eq!(
        countifs
            .evaluate(&[cr.clone(), blank_crit(BlankCheck::Blank)], &ctx)
            .unwrap(),
        CellValue::Number(1.0)
    );

    // NOT_BLANK ("<>") matches "Alpha", "", and Error(Div0)
    assert_eq!(
        countifs
            .evaluate(&[cr, blank_crit(BlankCheck::NotBlank)], &ctx)
            .unwrap(),
        CellValue::Number(3.0)
    );
}

#[test]
fn test_number_exact_vs_15sig_comparator_and_not_eq_empty_error_matching() {
    let registry = FunctionRegistry::standard();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    let one_third_range = make_array(1, 1, vec![CellValue::Number(1.0 / 3.0)]);
    assert_eq!(
        countifs
            .evaluate(
                &[
                    one_third_range.clone(),
                    exact_crit(CellValue::Number(1.0 / 3.0))
                ],
                &ctx
            )
            .unwrap(),
        CellValue::Number(1.0)
    );
    assert_eq!(
        countifs
            .evaluate(
                &[
                    one_third_range,
                    cmp_crit(CriteriaComparator::Lte, CellValue::Number(1.0 / 3.0)),
                ],
                &ctx
            )
            .unwrap(),
        CellValue::Number(0.0)
    );

    // NotEq matches non-20 numbers, Empty, and Error(Div0)
    let sparse_err_nums = make_array(
        1,
        4,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Empty,
            CellValue::Error(ExcelError::Div0),
        ],
    );
    assert_eq!(
        countifs
            .evaluate(
                &[
                    sparse_err_nums.clone(),
                    cmp_crit(CriteriaComparator::NotEq, CellValue::Number(20.0)),
                ],
                &ctx
            )
            .unwrap(),
        CellValue::Number(3.0)
    );
    assert_eq!(
        countifs
            .evaluate(
                &[
                    sparse_err_nums.clone(),
                    blank_crit(BlankCheck::NotBlank),
                    sparse_err_nums,
                    cmp_crit(CriteriaComparator::NotEq, CellValue::Number(20.0)),
                ],
                &ctx
            )
            .unwrap(),
        CellValue::Number(2.0) // 10.0 and Error(Div0)
    );
}

// ============================================================================
// Scenario 6: Stage B strict homogeneity, ordering, ErrorInCriteria, and text validation
// ============================================================================

#[test]
fn test_stage_b_homogeneity_ordering_error_in_criteria_and_text_validation() {
    let registry = FunctionRegistry::standard();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let ctx = TestEvalContext;

    // Mixed criteria_range [10.0, "Alpha"] fails homogeneity check
    let mixed_cr = make_array(
        1,
        2,
        vec![CellValue::Number(10.0), CellValue::Text("Alpha".into())],
    );
    assert_eq!(
        countifs.evaluate(&[mixed_cr, blank_crit(BlankCheck::NotBlank)], &ctx),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "COUNTIFS",
            param: "criteria_range1",
            position: 1,
            expected: ExpectedType::Number,
            found: ValueType::Text,
        }))
    );

    // Ordering comparator on Text criteria_range fails on criteria_range1
    let text_cr = make_array(
        1,
        2,
        vec![
            CellValue::Text("Alpha".into()),
            CellValue::Text("Beta".into()),
        ],
    );
    assert_eq!(
        countifs.evaluate(
            &[
                text_cr.clone(),
                cmp_crit(CriteriaComparator::Gt, CellValue::Number(10.0)),
            ],
            &ctx
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "COUNTIFS",
            param: "criteria_range1",
            position: 1,
            expected: ExpectedType::Number,
            found: ValueType::Text,
        }))
    );

    // Cross-type exact/NotEq criterion vs criteria_range fails with InvalidArgumentType
    let num_cr = make_array(1, 2, vec![CellValue::Number(10.0), CellValue::Number(20.0)]);
    assert_eq!(
        countifs.evaluate(
            &[num_cr.clone(), exact_crit(CellValue::Text("Alpha".into()))],
            &ctx
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "COUNTIFS",
            param: "criteria1",
            position: 2,
            expected: ExpectedType::Number,
            found: ValueType::Text,
        }))
    );

    // Empty criterion fails with InvalidArgumentType
    assert_eq!(
        countifs.evaluate(&[num_cr.clone(), exact_crit(CellValue::Empty)], &ctx),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "COUNTIFS",
            param: "criteria1",
            position: 2,
            expected: ExpectedType::Number,
            found: ValueType::Empty,
        }))
    );

    // Errored criterion checks inferred_type provenance before ErrorInCriteria
    assert_eq!(
        countifs.evaluate(
            &[
                num_cr.clone(),
                FunctionArg::Criterion(BoundCriterion::Errored {
                    op: None,
                    error: ExcelError::Na,
                    inferred_type: Some(ValueType::Text),
                }),
            ],
            &ctx
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "COUNTIFS",
            param: "criteria1",
            position: 2,
            expected: ExpectedType::Number,
            found: ValueType::Text,
        }))
    );

    // Type-compatible errored criterion fails with ErrorInCriteria
    assert_eq!(
        countifs.evaluate(
            &[
                num_cr.clone(),
                FunctionArg::Criterion(BoundCriterion::Errored {
                    op: Some(CriteriaComparator::Lte),
                    error: ExcelError::Na,
                    inferred_type: Some(ValueType::Number),
                }),
            ],
            &ctx
        ),
        Err(FunctionError::ErrorInCriteria {
            function: "COUNTIFS",
            param: "criteria1",
            position: 2,
            error: ExcelError::Na,
        })
    );

    // Second pair validation runs BEFORE bubbling matched target_range ExcelError
    let target_with_err = make_array(
        1,
        2,
        vec![CellValue::Error(ExcelError::Div0), CellValue::Number(10.0)],
    );
    assert_eq!(
        sumifs.evaluate(
            &[
                target_with_err,
                num_cr,
                exact_crit(CellValue::Number(10.0)),
                text_cr.clone(),
                exact_crit(CellValue::Text("A*".into())),
            ],
            &ctx
        ),
        Err(FunctionError::InvalidTextCriterion {
            function: "SUMIFS",
            param: "criteria1",
            position: 5,
            reason: TextCriterionError::ContainsWildcardOrEscape,
        })
    );

    // Invalid dynamic text criterion variants fail with InvalidTextCriterion
    for (bad_str, expected_reason) in [
        ("", TextCriterionError::EmptyString),
        ("a?b", TextCriterionError::ContainsWildcardOrEscape),
        (">=10", TextCriterionError::LeadingOperatorPrefix),
        ("10%", TextCriterionError::CoercibleNumericString),
        ("2024-01-01", TextCriterionError::CoercibleDateOrTimeString),
        ("false", TextCriterionError::CoercibleBooleanString),
        ("#N/A", TextCriterionError::CoercibleErrorString),
    ] {
        assert_eq!(
            countifs.evaluate(
                &[text_cr.clone(), exact_crit(CellValue::Text(bad_str.into()))],
                &ctx
            ),
            Err(FunctionError::InvalidTextCriterion {
                function: "COUNTIFS",
                param: "criteria1",
                position: 2,
                reason: expected_reason,
            }),
            "mismatch on {bad_str:?}"
        );
    }
}

// ============================================================================
// Scenario 7: Dimension conformity and FunctionArg kind validation
// ============================================================================

#[test]
fn test_scenario_7_dimension_conformity_validation() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    let target = make_array(1, 4, vec![CellValue::Number(10.0); 4]);
    let mismatched = make_array(1, 3, vec![CellValue::Text("A".into()); 3]);
    let crit = exact_crit(CellValue::Text("A".into()));

    assert_eq!(
        sumifs.evaluate(&[target.clone(), mismatched.clone(), crit.clone()], &ctx),
        Err(FunctionError::DimensionMismatch {
            function: "SUMIFS",
            expected_rows: 1,
            expected_cols: 4,
            found_rows: 1,
            found_cols: 3,
        })
    );

    // Scalar in ParamKind::Range slot -> InvalidArgumentKind
    assert_eq!(
        sumifs.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(10.0)),
                mismatched,
                crit
            ],
            &ctx
        ),
        Err(FunctionError::InvalidArgumentKind {
            function: "SUMIFS",
            expected: "range",
            found: "scalar",
        })
    );

    // Scalar in ParamKind::Criteria slot -> InvalidArgumentKind
    let cr_four = make_array(1, 4, vec![CellValue::Text("A".into()); 4]);
    assert_eq!(
        sumifs.evaluate(
            &[
                target,
                cr_four,
                FunctionArg::Scalar(CellValue::Text("A".into())),
            ],
            &ctx
        ),
        Err(FunctionError::InvalidArgumentKind {
            function: "SUMIFS",
            expected: "criterion",
            found: "scalar",
        })
    );

    // COUNTIFS dimension mismatch: cr1 (2, 2) vs cr2 (1, 4) -> DimensionMismatch
    let cr1 = make_array(2, 2, vec![CellValue::Number(1.0); 4]);
    let cr2 = make_array(1, 4, vec![CellValue::Number(1.0); 4]);
    let crit1 = exact_crit(CellValue::Number(1.0));
    let crit2 = exact_crit(CellValue::Number(1.0));

    assert_eq!(
        countifs.evaluate(&[cr1, crit1, cr2, crit2], &ctx),
        Err(FunctionError::DimensionMismatch {
            function: "COUNTIFS",
            expected_rows: 2,
            expected_cols: 2,
            found_rows: 1,
            found_cols: 4,
        })
    );
}

// ============================================================================
// Error Bubbling, 2D Slices, Leading Whitespace, and Arity Guards
// ============================================================================

#[test]
fn test_error_bubbling_in_target_range() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let ctx = TestEvalContext;

    let target = make_array(
        1,
        2,
        vec![
            CellValue::Error(ExcelError::Div0),
            CellValue::Error(ExcelError::Value),
        ],
    );
    let cr = make_array(
        1,
        2,
        vec![CellValue::Text("Yes".into()), CellValue::Text("No".into())],
    );
    let crit = exact_crit(CellValue::Text("Yes".into()));

    let res = sumifs.evaluate(&[target, cr, crit], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Div0));
}

#[test]
fn test_error_in_unmatched_target_is_ignored() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let ctx = TestEvalContext;

    let target = make_array(
        1,
        2,
        vec![CellValue::Number(100.0), CellValue::Error(ExcelError::Div0)],
    );
    let cr = make_array(
        1,
        2,
        vec![CellValue::Text("Yes".into()), CellValue::Text("No".into())],
    );
    let crit = exact_crit(CellValue::Text("Yes".into()));

    let res = sumifs.evaluate(&[target, cr, crit], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(100.0));
}

#[test]
fn test_2d_range_slices() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let ctx = TestEvalContext;

    let target = make_array(
        2,
        2,
        vec![
            CellValue::Number(1.0),
            CellValue::Number(2.0),
            CellValue::Number(3.0),
            CellValue::Number(4.0),
        ],
    );
    let cr = make_array(
        2,
        2,
        vec![
            CellValue::Text("X".into()),
            CellValue::Text("Y".into()),
            CellValue::Text("X".into()),
            CellValue::Text("Y".into()),
        ],
    );
    let crit = exact_crit(CellValue::Text("X".into()));

    let res = sumifs.evaluate(&[target, cr, crit], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(4.0));
}

#[test]
fn test_metadata_and_categories() {
    let registry = FunctionRegistry::standard();
    let functions = ["SUMIFS", "COUNTIFS", "AVERAGEIFS", "MINIFS", "MAXIFS"];

    for name in functions {
        let func = registry.lookup(name).unwrap();
        assert_eq!(func.name(), name);
        assert_eq!(func.category(), FunctionCategory::Math);
    }
}

#[test]
fn test_criteria_leading_whitespace_space_only_and_nan_text_labels() {
    let registry = FunctionRegistry::standard();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    let cr = make_array(
        1,
        5,
        vec![
            CellValue::Text("  DeptA".into()),
            CellValue::Text("DeptA".into()),
            CellValue::Text(" ".into()),
            CellValue::Text("NAN".into()),
            CellValue::Empty,
        ],
    );

    assert_eq!(
        countifs
            .evaluate(
                &[cr.clone(), exact_crit(CellValue::Text("  DeptA".into()))],
                &ctx
            )
            .unwrap(),
        CellValue::Number(1.0)
    );
    assert_eq!(
        countifs
            .evaluate(&[cr.clone(), exact_crit(CellValue::Text(" ".into()))], &ctx)
            .unwrap(),
        CellValue::Number(1.0)
    );
    assert_eq!(
        countifs
            .evaluate(
                &[cr.clone(), exact_crit(CellValue::Text("nan".into()))],
                &ctx
            )
            .unwrap(),
        CellValue::Number(1.0)
    );
    assert_eq!(
        countifs
            .evaluate(
                &[
                    cr,
                    cmp_crit(CriteriaComparator::NotEq, CellValue::Text(" ".into()))
                ],
                &ctx
            )
            .unwrap(),
        CellValue::Number(4.0)
    );
}

#[test]
fn test_evaluate_arity_and_variadic_pairs_guards() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    assert_eq!(
        sumifs.evaluate(&[], &ctx).unwrap_err(),
        FunctionError::TooFewArguments {
            function: "SUMIFS",
            expected: 3,
            found: 0,
        }
    );

    let dummy = FunctionArg::Scalar(CellValue::Number(1.0));
    assert_eq!(
        sumifs
            .evaluate(&[dummy.clone(), dummy.clone()], &ctx)
            .unwrap_err(),
        FunctionError::TooFewArguments {
            function: "SUMIFS",
            expected: 3,
            found: 2,
        }
    );
    assert_eq!(
        sumifs
            .evaluate(
                &[dummy.clone(), dummy.clone(), dummy.clone(), dummy.clone()],
                &ctx,
            )
            .unwrap_err(),
        FunctionError::InvalidVariadicPairs {
            function: "SUMIFS",
            count: 4,
        }
    );
    assert_eq!(
        countifs.evaluate(&[], &ctx).unwrap_err(),
        FunctionError::TooFewArguments {
            function: "COUNTIFS",
            expected: 2,
            found: 0,
        }
    );
    assert_eq!(
        countifs
            .evaluate(std::slice::from_ref(&dummy), &ctx)
            .unwrap_err(),
        FunctionError::TooFewArguments {
            function: "COUNTIFS",
            expected: 2,
            found: 1,
        }
    );
    assert_eq!(
        countifs
            .evaluate(&[dummy.clone(), dummy.clone(), dummy], &ctx)
            .unwrap_err(),
        FunctionError::InvalidVariadicPairs {
            function: "COUNTIFS",
            count: 3,
        }
    );
}
