#![allow(clippy::unwrap_used, clippy::expect_used)]

use f8_core::expr::ast::Expr;
use f8_core::functions::traits::{EvalContext, FunctionArg};
use f8_core::functions::{FunctionCategory, FunctionError, FunctionRegistry};
use f8_core::value::{CellValue, ExcelError};

/// Mock evaluation context for unit tests.
#[derive(Default)]
struct MockEvalContext;

impl EvalContext for MockEvalContext {
    fn eval_expr(&self, _expr: &Expr) -> Result<CellValue, FunctionError> {
        Ok(CellValue::Number(100.0))
    }
}

#[test]
fn test_scenario_1_category_folder_modularization_and_registration() {
    let registry = FunctionRegistry::standard();
    assert_eq!(registry.len(), 44);

    let expected_functions = [
        ("ABS", FunctionCategory::Math),
        ("SUM", FunctionCategory::Math),
        ("AVERAGE", FunctionCategory::Math),
        ("COUNT", FunctionCategory::Math),
        ("COUNTA", FunctionCategory::Math),
        ("MAX", FunctionCategory::Math),
        ("MIN", FunctionCategory::Math),
        ("MEDIAN", FunctionCategory::Math),
        ("ROUND", FunctionCategory::Math),
        ("ROUNDUP", FunctionCategory::Math),
        ("ROUNDDOWN", FunctionCategory::Math),
        ("SUMPRODUCT", FunctionCategory::Math),
        ("SUMIFS", FunctionCategory::Math),
        ("COUNTIFS", FunctionCategory::Math),
        ("AVERAGEIFS", FunctionCategory::Math),
        ("MINIFS", FunctionCategory::Math),
        ("MAXIFS", FunctionCategory::Math),
        ("IF", FunctionCategory::Logical),
        ("IFS", FunctionCategory::Logical),
        ("IFERROR", FunctionCategory::Logical),
        ("IFNA", FunctionCategory::Logical),
        ("AND", FunctionCategory::Logical),
        ("OR", FunctionCategory::Logical),
        ("NOT", FunctionCategory::Logical),
        ("CHOOSE", FunctionCategory::Logical),
        ("SWITCH", FunctionCategory::Logical),
        ("NPV", FunctionCategory::Financial),
        ("XNPV", FunctionCategory::Financial),
        ("IRR", FunctionCategory::Financial),
        ("XIRR", FunctionCategory::Financial),
        ("PMT", FunctionCategory::Financial),
        ("IPMT", FunctionCategory::Financial),
        ("PPMT", FunctionCategory::Financial),
        ("XLOOKUP", FunctionCategory::Lookup),
        ("XMATCH", FunctionCategory::Lookup),
        ("INDEX", FunctionCategory::Lookup),
    ];

    for (name, category) in expected_functions {
        let func = registry
            .lookup(name)
            .unwrap_or_else(|| panic!("Function {name} should be registered"));
        assert_eq!(func.name(), name);
        assert_eq!(func.category(), category);
    }
}

#[test]
fn test_scenario_2_average_calculation_and_empty_divisor() {
    let registry = FunctionRegistry::standard();
    let avg = registry.lookup("AVERAGE").unwrap();
    let ctx = MockEvalContext;

    // Range with numbers, ignored text, ignored booleans: [10.0, 20.0, "ignored", true, 30.0]
    let range_arg = FunctionArg::Array {
        rows: 5,
        cols: 1,
        values: vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Text("ignored".into()),
            CellValue::Boolean(true),
            CellValue::Number(30.0),
        ],
        is_range_ref: true,
    };

    let res = avg.evaluate(&[range_arg], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(20.0));

    // Empty range with no numbers returns #DIV/0!
    let empty_range = FunctionArg::Array {
        rows: 2,
        cols: 1,
        values: vec![CellValue::Empty, CellValue::Text("still no numbers".into())],
        is_range_ref: true,
    };
    let res = avg.evaluate(&[empty_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Div0));

    // Error in range bubbles immediately
    let err_range = FunctionArg::Array {
        rows: 2,
        cols: 1,
        values: vec![CellValue::Number(10.0), CellValue::Error(ExcelError::Ref)],
        is_range_ref: true,
    };
    let res = avg.evaluate(&[err_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Ref));

    // Direct scalar text returns #VALUE!
    let text_scalar = FunctionArg::Scalar(CellValue::Text("text".into()));
    let res = avg.evaluate(&[text_scalar], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Value));
}

#[test]
fn test_scenario_3_count_vs_counta_semantics() {
    let registry = FunctionRegistry::standard();
    let count_func = registry.lookup("COUNT").unwrap();
    let counta_func = registry.lookup("COUNTA").unwrap();
    let ctx = MockEvalContext;

    // Range containing [10.0, "text", true, Empty, ExcelError::Div0]
    let mixed_range = FunctionArg::Array {
        rows: 5,
        cols: 1,
        values: vec![
            CellValue::Number(10.0),
            CellValue::Text("text".into()),
            CellValue::Boolean(true),
            CellValue::Empty,
            CellValue::Error(ExcelError::Div0),
        ],
        is_range_ref: true,
    };

    // COUNT: only numbers counted; text, booleans, empty, and errors ignored (+0) -> 1.0
    let res_count = count_func
        .evaluate(std::slice::from_ref(&mixed_range), &ctx)
        .unwrap();
    assert_eq!(res_count, CellValue::Number(1.0));

    // COUNTA: non-empty cells counted (10.0, text, true, error) -> 4.0
    let res_counta = counta_func
        .evaluate(std::slice::from_ref(&mixed_range), &ctx)
        .unwrap();
    assert_eq!(res_counta, CellValue::Number(4.0));

    // Direct scalar arguments: COUNT(10.0, "hello", true, ExcelError::Div0) -> 2.0 (10.0 and true)
    let scalar_args = [
        FunctionArg::Scalar(CellValue::Number(10.0)),
        FunctionArg::Scalar(CellValue::Text("hello".into())),
        FunctionArg::Scalar(CellValue::Boolean(true)),
        FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
    ];
    let res_count_scalars = count_func.evaluate(&scalar_args, &ctx).unwrap();
    assert_eq!(res_count_scalars, CellValue::Number(2.0));

    // COUNTA on direct scalar arguments -> 4.0
    let res_counta_scalars = counta_func.evaluate(&scalar_args, &ctx).unwrap();
    assert_eq!(res_counta_scalars, CellValue::Number(4.0));

    // Direct scalar text: COUNT("hello") -> 0.0 (no error!)
    let single_text = [FunctionArg::Scalar(CellValue::Text("hello".into()))];
    let res_single_text = count_func.evaluate(&single_text, &ctx).unwrap();
    assert_eq!(res_single_text, CellValue::Number(0.0));
}

#[test]
fn test_scenario_4_max_and_min_with_range_filtering_and_empty_fallback() {
    let registry = FunctionRegistry::standard();
    let max_func = registry.lookup("MAX").unwrap();
    let min_func = registry.lookup("MIN").unwrap();
    let ctx = MockEvalContext;

    // Range containing [-15.0, 0.0, 42.5, "text"]
    let mixed_range = FunctionArg::Array {
        rows: 4,
        cols: 1,
        values: vec![
            CellValue::Number(-15.0),
            CellValue::Number(0.0),
            CellValue::Number(42.5),
            CellValue::Text("text".into()),
        ],
        is_range_ref: true,
    };
    assert_eq!(
        max_func
            .evaluate(std::slice::from_ref(&mixed_range), &ctx)
            .unwrap(),
        CellValue::Number(42.5)
    );
    assert_eq!(
        min_func
            .evaluate(std::slice::from_ref(&mixed_range), &ctx)
            .unwrap(),
        CellValue::Number(-15.0)
    );

    // Range containing all negative numbers: [-20.0, -10.0, -5.0]
    let negative_range = FunctionArg::Array {
        rows: 3,
        cols: 1,
        values: vec![
            CellValue::Number(-20.0),
            CellValue::Number(-10.0),
            CellValue::Number(-5.0),
        ],
        is_range_ref: true,
    };
    assert_eq!(
        max_func
            .evaluate(std::slice::from_ref(&negative_range), &ctx)
            .unwrap(),
        CellValue::Number(-5.0)
    );
    assert_eq!(
        min_func
            .evaluate(std::slice::from_ref(&negative_range), &ctx)
            .unwrap(),
        CellValue::Number(-20.0)
    );

    // Empty range defaults to 0.0
    let empty_range = FunctionArg::Array {
        rows: 2,
        cols: 1,
        values: vec![CellValue::Empty, CellValue::Text("ignored".into())],
        is_range_ref: true,
    };
    assert_eq!(
        max_func
            .evaluate(std::slice::from_ref(&empty_range), &ctx)
            .unwrap(),
        CellValue::Number(0.0)
    );
    assert_eq!(
        min_func
            .evaluate(std::slice::from_ref(&empty_range), &ctx)
            .unwrap(),
        CellValue::Number(0.0)
    );
}

#[test]
fn test_scenario_5_round_roundup_and_rounddown_variants() {
    let registry = FunctionRegistry::standard();
    let round_func = registry.lookup("ROUND").unwrap();
    let roundup_func = registry.lookup("ROUNDUP").unwrap();
    let rounddown_func = registry.lookup("ROUNDDOWN").unwrap();
    let ctx = MockEvalContext;

    // ROUND(2.15, 1) -> 2.2, ROUND(-2.15, 1) -> -2.2 (symmetric half away from zero)
    let args_pos = [
        FunctionArg::Scalar(CellValue::Number(2.15)),
        FunctionArg::Scalar(CellValue::Number(1.0)),
    ];
    let args_neg = [
        FunctionArg::Scalar(CellValue::Number(-2.15)),
        FunctionArg::Scalar(CellValue::Number(1.0)),
    ];
    assert_eq!(
        round_func.evaluate(&args_pos, &ctx).unwrap(),
        CellValue::Number(2.2)
    );
    assert_eq!(
        round_func.evaluate(&args_neg, &ctx).unwrap(),
        CellValue::Number(-2.2)
    );

    // ROUNDUP(2.141, 2) -> 2.15, ROUNDUP(-2.141, 2) -> -2.15
    let args_up = [
        FunctionArg::Scalar(CellValue::Number(2.141)),
        FunctionArg::Scalar(CellValue::Number(2.0)),
    ];
    let args_up_neg = [
        FunctionArg::Scalar(CellValue::Number(-2.141)),
        FunctionArg::Scalar(CellValue::Number(2.0)),
    ];
    assert_eq!(
        roundup_func.evaluate(&args_up, &ctx).unwrap(),
        CellValue::Number(2.15)
    );
    assert_eq!(
        roundup_func.evaluate(&args_up_neg, &ctx).unwrap(),
        CellValue::Number(-2.15)
    );

    // ROUNDDOWN(2.149, 2) -> 2.14, ROUNDDOWN(-2.149, 2) -> -2.14
    let args_down = [
        FunctionArg::Scalar(CellValue::Number(2.149)),
        FunctionArg::Scalar(CellValue::Number(2.0)),
    ];
    let args_down_neg = [
        FunctionArg::Scalar(CellValue::Number(-2.149)),
        FunctionArg::Scalar(CellValue::Number(2.0)),
    ];
    assert_eq!(
        rounddown_func.evaluate(&args_down, &ctx).unwrap(),
        CellValue::Number(2.14)
    );
    assert_eq!(
        rounddown_func.evaluate(&args_down_neg, &ctx).unwrap(),
        CellValue::Number(-2.14)
    );

    // ROUND(125.0, -1) -> 130.0
    let args_left = [
        FunctionArg::Scalar(CellValue::Number(125.0)),
        FunctionArg::Scalar(CellValue::Number(-1.0)),
    ];
    assert_eq!(
        round_func.evaluate(&args_left, &ctx).unwrap(),
        CellValue::Number(130.0)
    );

    // ROUND with non-integer digits truncates digits: ROUND(2.15, 1.9) == ROUND(2.15, 1) -> 2.2
    let args_trunc_digits = [
        FunctionArg::Scalar(CellValue::Number(2.15)),
        FunctionArg::Scalar(CellValue::Number(1.9)),
    ];
    assert_eq!(
        round_func.evaluate(&args_trunc_digits, &ctx).unwrap(),
        CellValue::Number(2.2)
    );
}

#[test]
fn test_scenario_6_sumproduct_dimension_checking_and_calculation() {
    let registry = FunctionRegistry::standard();
    let sumproduct = registry.lookup("SUMPRODUCT").unwrap();
    let ctx = MockEvalContext;

    // Two ranges of dimensions 3x1: [1.0, 2.0, 3.0] and [4.0, 5.0, 6.0]
    let r1 = FunctionArg::Array {
        rows: 3,
        cols: 1,
        values: vec![
            CellValue::Number(1.0),
            CellValue::Number(2.0),
            CellValue::Number(3.0),
        ],
        is_range_ref: true,
    };
    let r2 = FunctionArg::Array {
        rows: 3,
        cols: 1,
        values: vec![
            CellValue::Number(4.0),
            CellValue::Number(5.0),
            CellValue::Number(6.0),
        ],
        is_range_ref: true,
    };

    // 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32.0
    let res = sumproduct.evaluate(&[r1, r2], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(32.0));

    // Ranges with non-numeric text: [1.0, "text", 3.0] and [4.0, 5.0, 6.0]
    let r_text = FunctionArg::Array {
        rows: 3,
        cols: 1,
        values: vec![
            CellValue::Number(1.0),
            CellValue::Text("text".into()),
            CellValue::Number(3.0),
        ],
        is_range_ref: true,
    };
    let r_other = FunctionArg::Array {
        rows: 3,
        cols: 1,
        values: vec![
            CellValue::Number(4.0),
            CellValue::Number(5.0),
            CellValue::Number(6.0),
        ],
        is_range_ref: true,
    };
    // 1*4 + 0*5 + 3*6 = 22.0
    let res_text = sumproduct.evaluate(&[r_text, r_other], &ctx).unwrap();
    assert_eq!(res_text, CellValue::Number(22.0));

    // Mismatched dimensions (3x1 and 2x1) -> #VALUE!
    let r_mismatch = FunctionArg::Array {
        rows: 2,
        cols: 1,
        values: vec![CellValue::Number(4.0), CellValue::Number(5.0)],
        is_range_ref: true,
    };
    let r_base = FunctionArg::Array {
        rows: 3,
        cols: 1,
        values: vec![
            CellValue::Number(1.0),
            CellValue::Number(2.0),
            CellValue::Number(3.0),
        ],
        is_range_ref: true,
    };
    let res_mismatch = sumproduct.evaluate(&[r_base, r_mismatch], &ctx).unwrap();
    assert_eq!(res_mismatch, CellValue::Error(ExcelError::Value));

    // Error in range bubbles immediately
    let r_err = FunctionArg::Array {
        rows: 2,
        cols: 1,
        values: vec![CellValue::Number(1.0), CellValue::Error(ExcelError::Div0)],
        is_range_ref: true,
    };
    let r_valid = FunctionArg::Array {
        rows: 2,
        cols: 1,
        values: vec![CellValue::Number(1.0), CellValue::Number(2.0)],
        is_range_ref: true,
    };
    let res_err = sumproduct.evaluate(&[r_err, r_valid], &ctx).unwrap();
    assert_eq!(res_err, CellValue::Error(ExcelError::Div0));

    // Malformed array element count mismatch returns evaluation failure error
    let r_malformed = FunctionArg::Array {
        rows: 2,
        cols: 2,
        values: vec![CellValue::Number(1.0)],
        is_range_ref: true,
    };
    let res_malformed = sumproduct.evaluate(&[r_malformed], &ctx);
    assert!(matches!(
        res_malformed,
        Err(FunctionError::EvaluationFailure { .. })
    ));
}

#[test]
fn test_scenario_11_median_odd_even_ranges_and_errors() {
    let registry = FunctionRegistry::standard();
    let median = registry.lookup("MEDIAN").unwrap();
    let ctx = MockEvalContext;

    // 1. Odd count of numbers: [10, 50, 20] -> sorted [10, 20, 50] -> median is 20
    let odd_range = FunctionArg::Array {
        rows: 3,
        cols: 1,
        values: vec![
            CellValue::Number(10.0),
            CellValue::Number(50.0),
            CellValue::Number(20.0),
        ],
        is_range_ref: true,
    };
    let res = median.evaluate(&[odd_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(20.0));

    // 2. Even count of numbers: [10, 40, 20, 30] -> sorted [10, 20, 30, 40] -> median is (20 + 30) / 2 = 25
    let even_range = FunctionArg::Array {
        rows: 4,
        cols: 1,
        values: vec![
            CellValue::Number(10.0),
            CellValue::Number(40.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ],
        is_range_ref: true,
    };
    let res = median.evaluate(&[even_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(25.0));

    // 3. Range with ignored non-numeric cells (text, booleans, empty)
    let mixed_range = FunctionArg::Array {
        rows: 6,
        cols: 1,
        values: vec![
            CellValue::Number(100.0),
            CellValue::Text("ignore me".into()),
            CellValue::Boolean(true),
            CellValue::Empty,
            CellValue::Number(300.0),
            CellValue::Number(200.0),
        ],
        is_range_ref: true,
    };
    let res = median.evaluate(&[mixed_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(200.0));

    // 4. Empty range with no numbers returns #NUM! in Excel
    let empty_range = FunctionArg::Array {
        rows: 2,
        cols: 1,
        values: vec![CellValue::Empty, CellValue::Text("no numbers".into())],
        is_range_ref: true,
    };
    let res = median.evaluate(&[empty_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Num));

    // 5. Direct scalars with boolean coercion: MEDIAN(10, TRUE, FALSE) -> sorted [0, 1, 10] -> median is 1
    let scalar_args = [
        FunctionArg::Scalar(CellValue::Number(10.0)),
        FunctionArg::Scalar(CellValue::Boolean(true)),
        FunctionArg::Scalar(CellValue::Boolean(false)),
    ];
    let res = median.evaluate(&scalar_args, &ctx).unwrap();
    assert_eq!(res, CellValue::Number(1.0));

    // 6. Direct scalar text returns #VALUE!
    let scalar_text_args = [
        FunctionArg::Scalar(CellValue::Number(10.0)),
        FunctionArg::Scalar(CellValue::Text("invalid".into())),
    ];
    let res = median.evaluate(&scalar_text_args, &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Value));

    // 7. Error in range bubbles immediately
    let error_range = FunctionArg::Array {
        rows: 2,
        cols: 1,
        values: vec![CellValue::Number(10.0), CellValue::Error(ExcelError::Ref)],
        is_range_ref: true,
    };
    let res = median.evaluate(&[error_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Ref));

    // 8. Too few arguments (0 args)
    let res_empty = median.evaluate(&[], &ctx);
    assert!(matches!(
        res_empty,
        Err(FunctionError::TooFewArguments { .. })
    ));
}
