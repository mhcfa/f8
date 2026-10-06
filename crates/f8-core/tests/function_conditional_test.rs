//! Integration tests for Excel conditional aggregation functions (*IFS family).
//!
//! Covers `SUMIFS`, `COUNTIFS`, `AVERAGEIFS`, `MINIFS`, and `MAXIFS` conforming to
//! Excel calculation parity and criteria semantics.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use f8_core::expr::ast::Expr;
use f8_core::functions::{
    EvalContext, FunctionArg, FunctionCategory, FunctionError, FunctionRegistry,
};
use f8_core::value::{CellValue, ExcelError};

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
    FunctionArg::Array {
        rows,
        cols,
        values,
        is_range_ref: true,
    }
}

// ============================================================================
// Scenario 1: SUMIFS multi-criteria evaluation with numeric and text criteria
// ============================================================================

#[test]
fn test_scenario_1_sumifs_multi_criteria_numeric_and_text() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let ctx = TestEvalContext;

    // sum_range: [10, 20, 30, 40]
    // cr1: ["A", "B", "A", "B"], crit1: "A"
    // cr2: [1, 2, 1, 2], crit2: ">0"
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
    let crit1 = FunctionArg::Scalar(CellValue::Text("A".into()));
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
    let crit2 = FunctionArg::Scalar(CellValue::Text(">0".into()));

    let res = sumifs
        .evaluate(&[sum_range, cr1, crit1, cr2, crit2], &ctx)
        .unwrap();
    assert_eq!(res, CellValue::Number(40.0)); // 10 + 30 = 40.0
}

#[test]
fn test_sumifs_text_empty_and_boolean_ignored_in_sum_range() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let ctx = TestEvalContext;

    // sum_range with mixed types
    let sum_range = make_array(
        1,
        4,
        vec![
            CellValue::Number(100.0),
            CellValue::Text("text".into()),
            CellValue::Empty,
            CellValue::Boolean(true),
        ],
    );
    let cr1 = make_array(
        1,
        4,
        vec![
            CellValue::Number(1.0),
            CellValue::Number(1.0),
            CellValue::Number(1.0),
            CellValue::Number(1.0),
        ],
    );
    let crit1 = FunctionArg::Scalar(CellValue::Number(1.0));

    let res = sumifs.evaluate(&[sum_range, cr1, crit1], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(100.0));
}

// ============================================================================
// Scenario 2: COUNTIFS multi-criteria counting
// ============================================================================

#[test]
fn test_scenario_2_countifs_multi_criteria_counting() {
    let registry = FunctionRegistry::standard();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    // cr1: [10, 20, 30, 40], crit1: ">=20"
    // cr2: ["East", "West", "East", "West"], crit2: "*st"
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
    let crit1 = FunctionArg::Scalar(CellValue::Text(">=20".into()));
    let cr2 = make_array(
        1,
        4,
        vec![
            CellValue::Text("East".into()),
            CellValue::Text("West".into()),
            CellValue::Text("East".into()),
            CellValue::Text("West".into()),
        ],
    );
    let crit2 = FunctionArg::Scalar(CellValue::Text("*st".into()));

    let res = countifs.evaluate(&[cr1, crit1, cr2, crit2], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(3.0)); // positions 2, 3, 4 match
}

// ============================================================================
// Scenario 3: AVERAGEIFS empty match #DIV/0! fallback
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
    let crit1 = FunctionArg::Scalar(CellValue::Text("W".into()));

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
    let crit1 = FunctionArg::Scalar(CellValue::Text("tech".into())); // case-insensitive

    let res = avgifs.evaluate(&[avg_range, cr1, crit1], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(20.0)); // (10 + 30) / 2 = 20.0
}

// ============================================================================
// Scenario 4: MINIFS and MAXIFS zero fallback on no match
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
    let crit = FunctionArg::Scalar(CellValue::Text("Yellow".into()));

    let min_res = minifs
        .evaluate(&[data.clone(), cr.clone(), crit.clone()], &ctx)
        .unwrap();
    assert_eq!(min_res, CellValue::Number(0.0));

    let max_res = maxifs.evaluate(&[data, cr, crit], &ctx).unwrap();
    assert_eq!(max_res, CellValue::Number(0.0));
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
    let crit = FunctionArg::Scalar(CellValue::Text("DeptA".into()));

    let min_res = minifs
        .evaluate(&[data.clone(), cr.clone(), crit.clone()], &ctx)
        .unwrap();
    assert_eq!(min_res, CellValue::Number(30.0)); // min of 50, 80, 30

    let max_res = maxifs.evaluate(&[data, cr, crit], &ctx).unwrap();
    assert_eq!(max_res, CellValue::Number(80.0)); // max of 50, 80, 30
}

// ============================================================================
// Scenario 5: Wildcard matching with escape sequences
// ============================================================================

#[test]
fn test_scenario_5_wildcard_matching_with_escape_sequences() {
    let registry = FunctionRegistry::standard();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    let cr = make_array(
        1,
        4,
        vec![
            CellValue::Text("abc".into()),
            CellValue::Text("a*c".into()),
            CellValue::Text("axc".into()),
            CellValue::Text("a?c".into()),
        ],
    );

    // Literal escaped star: "a~*c" should match only "a*c"
    let crit_escaped_star = FunctionArg::Scalar(CellValue::Text("a~*c".into()));
    let res = countifs
        .evaluate(&[cr.clone(), crit_escaped_star], &ctx)
        .unwrap();
    assert_eq!(res, CellValue::Number(1.0));

    // Literal escaped question: "a~?c" should match only "a?c"
    let crit_escaped_q = FunctionArg::Scalar(CellValue::Text("a~?c".into()));
    let res_q = countifs
        .evaluate(&[cr.clone(), crit_escaped_q], &ctx)
        .unwrap();
    assert_eq!(res_q, CellValue::Number(1.0));

    // Unescaped wildcard "a?c" should match "abc", "a*c", "axc", "a?c"
    let crit_wildcard = FunctionArg::Scalar(CellValue::Text("a?c".into()));
    let res_all = countifs.evaluate(&[cr, crit_wildcard], &ctx).unwrap();
    assert_eq!(res_all, CellValue::Number(4.0));
}

// ============================================================================
// Scenario 6: Blank and non-blank criteria sentinels
// ============================================================================

#[test]
fn test_scenario_6_blank_and_non_blank_sentinels() {
    let registry = FunctionRegistry::standard();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    let cr = make_array(
        1,
        4,
        vec![
            CellValue::Empty,
            CellValue::Text("Text".into()),
            CellValue::Number(10.0),
            CellValue::Text("".into()),
        ],
    );

    // Empty sentinel "" matches Empty and ""
    let crit_empty = FunctionArg::Scalar(CellValue::Text("".into()));
    let res_empty = countifs.evaluate(&[cr.clone(), crit_empty], &ctx).unwrap();
    assert_eq!(res_empty, CellValue::Number(2.0));

    // Non-blank sentinel "<>" matches "Text" and 10.0
    let crit_non_blank = FunctionArg::Scalar(CellValue::Text("<>".into()));
    let res_non_blank = countifs.evaluate(&[cr, crit_non_blank], &ctx).unwrap();
    assert_eq!(res_non_blank, CellValue::Number(2.0));
}

// ============================================================================
// Scenario 7: Dimension conformity validation
// ============================================================================

#[test]
fn test_scenario_7_dimension_conformity_validation() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    // Target (1, 4) vs criteria range (1, 3)
    let target = make_array(1, 4, vec![CellValue::Number(10.0); 4]);
    let mismatched = make_array(1, 3, vec![CellValue::Text("A".into()); 3]);
    let crit = FunctionArg::Scalar(CellValue::Text("A".into()));

    let res = sumifs.evaluate(&[target, mismatched, crit], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Value));

    // COUNTIFS dimension mismatch: cr1 (2, 2) vs cr2 (1, 4)
    let cr1 = make_array(2, 2, vec![CellValue::Number(1.0); 4]);
    let cr2 = make_array(1, 4, vec![CellValue::Number(1.0); 4]);
    let crit1 = FunctionArg::Scalar(CellValue::Number(1.0));
    let crit2 = FunctionArg::Scalar(CellValue::Number(1.0));

    let res_count = countifs.evaluate(&[cr1, crit1, cr2, crit2], &ctx).unwrap();
    assert_eq!(res_count, CellValue::Error(ExcelError::Value));
}

// ============================================================================
// Error Bubbling and Domain Error Handling
// ============================================================================

#[test]
fn test_error_bubbling_in_target_range() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let ctx = TestEvalContext;

    // Row 1 matches and has #DIV/0! -> must bubble
    // Row 2 does not match and has #VALUE! -> must not bubble
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
    let crit = FunctionArg::Scalar(CellValue::Text("Yes".into()));

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
    let crit = FunctionArg::Scalar(CellValue::Text("Yes".into()));

    let res = sumifs.evaluate(&[target, cr, crit], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(100.0));
}

#[test]
fn test_2d_range_slices() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let ctx = TestEvalContext;

    // 2x2 grid
    // target: [1, 2; 3, 4]
    // cr: ["X", "Y"; "X", "Y"]
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
    let crit = FunctionArg::Scalar(CellValue::Text("X".into()));

    let res = sumifs.evaluate(&[target, cr, crit], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(4.0)); // 1.0 + 3.0 = 4.0
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

// ============================================================================
// Whitespace Preservation and Operator Spacing Integration Tests
// ============================================================================

#[test]
fn test_criteria_leading_whitespace_and_space_only_matching() {
    let registry = FunctionRegistry::standard();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    let cr = make_array(
        1,
        4,
        vec![
            CellValue::Text("  DeptA".into()),
            CellValue::Text("DeptA".into()),
            CellValue::Text(" ".into()),
            CellValue::Empty,
        ],
    );

    // Criterion with leading spaces matches "  DeptA", not "DeptA"
    let crit_spaces = FunctionArg::Scalar(CellValue::Text("  DeptA".into()));
    let res = countifs.evaluate(&[cr.clone(), crit_spaces], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(1.0));

    // Criterion with only a space matches " ", not Empty
    let crit_space_only = FunctionArg::Scalar(CellValue::Text(" ".into()));
    let res_space = countifs.evaluate(&[cr, crit_space_only], &ctx).unwrap();
    assert_eq!(res_space, CellValue::Number(1.0));
}

#[test]
fn test_criteria_operator_with_spaces() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let ctx = TestEvalContext;

    let sum_range = make_array(
        1,
        3,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ],
    );
    let cr = make_array(
        1,
        3,
        vec![
            CellValue::Number(5.0),
            CellValue::Number(15.0),
            CellValue::Number(25.0),
        ],
    );

    // Criteria with space after operator "> 10"
    let crit = FunctionArg::Scalar(CellValue::Text("> 10".into()));
    let res = sumifs.evaluate(&[sum_range, cr, crit], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(50.0)); // 20.0 + 30.0
}

#[test]
fn test_criteria_not_equal_to_space_vs_non_blank_sentinel() {
    let registry = FunctionRegistry::standard();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    // Range with Empty, "Text", 10.0, and a single space " "
    let cr = make_array(
        1,
        4,
        vec![
            CellValue::Empty,
            CellValue::Text("Text".into()),
            CellValue::Number(10.0),
            CellValue::Text(" ".into()),
        ],
    );

    // Non-blank sentinel "<>" matches non-empty cells ("Text", 10.0, " ") -> 3
    let crit_non_blank = FunctionArg::Scalar(CellValue::Text("<>".into()));
    let res_non_blank = countifs
        .evaluate(&[cr.clone(), crit_non_blank], &ctx)
        .unwrap();
    assert_eq!(res_non_blank, CellValue::Number(3.0));

    // Not-equal-to-space sentinel "<> " matches all cells EXCEPT " " (Empty, "Text", 10.0) -> 3
    let crit_not_space = FunctionArg::Scalar(CellValue::Text("<> ".into()));
    let res_not_space = countifs.evaluate(&[cr, crit_not_space], &ctx).unwrap();
    assert_eq!(res_not_space, CellValue::Number(3.0));
}

// ============================================================================
// Arity and Variadic Pairs Validation in evaluate()
// ============================================================================

#[test]
fn test_evaluate_arity_and_variadic_pairs_guards() {
    let registry = FunctionRegistry::standard();
    let sumifs = registry.lookup("SUMIFS").unwrap();
    let countifs = registry.lookup("COUNTIFS").unwrap();
    let ctx = TestEvalContext;

    // SUMIFS with 0 args
    let err_empty = sumifs.evaluate(&[], &ctx).unwrap_err();
    assert_eq!(
        err_empty,
        FunctionError::TooFewArguments {
            function: "SUMIFS",
            expected: 3,
            found: 0,
        }
    );

    // SUMIFS with 2 args (too few)
    let dummy = FunctionArg::Scalar(CellValue::Number(1.0));
    let err_two = sumifs
        .evaluate(&[dummy.clone(), dummy.clone()], &ctx)
        .unwrap_err();
    assert_eq!(
        err_two,
        FunctionError::TooFewArguments {
            function: "SUMIFS",
            expected: 3,
            found: 2,
        }
    );

    // SUMIFS with 4 args (invalid variadic pairs)
    let err_four = sumifs
        .evaluate(
            &[dummy.clone(), dummy.clone(), dummy.clone(), dummy.clone()],
            &ctx,
        )
        .unwrap_err();
    assert_eq!(
        err_four,
        FunctionError::InvalidVariadicPairs {
            function: "SUMIFS",
            count: 4,
        }
    );

    // COUNTIFS with 0 args
    let err_count_empty = countifs.evaluate(&[], &ctx).unwrap_err();
    assert_eq!(
        err_count_empty,
        FunctionError::TooFewArguments {
            function: "COUNTIFS",
            expected: 2,
            found: 0,
        }
    );

    // COUNTIFS with 1 arg
    let err_count_one = countifs
        .evaluate(std::slice::from_ref(&dummy), &ctx)
        .unwrap_err();
    assert_eq!(
        err_count_one,
        FunctionError::TooFewArguments {
            function: "COUNTIFS",
            expected: 2,
            found: 1,
        }
    );

    // COUNTIFS with 3 args (invalid variadic pairs)
    let err_count_three = countifs
        .evaluate(&[dummy.clone(), dummy.clone(), dummy], &ctx)
        .unwrap_err();
    assert_eq!(
        err_count_three,
        FunctionError::InvalidVariadicPairs {
            function: "COUNTIFS",
            count: 3,
        }
    );
}
