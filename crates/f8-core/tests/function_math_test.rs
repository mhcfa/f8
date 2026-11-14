#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use indexmap::IndexMap;

use f8_core::compiler::eval::eval_expr;
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::ir::symbol::SymbolTable;
use f8_core::expr::ast::Expr;
use f8_core::functions::traits::{EvalContext, FunctionArg, ReferenceKind};
use f8_core::functions::{FunctionCategory, FunctionError, FunctionRegistry};
use f8_core::model::id::{SheetId, TableId};
use f8_core::model::table::{AxisRole, ColumnSpec, RowSpec, TableSpec};
use f8_core::value::{CellValue, ExcelError, ExpectedType, TypeError, ValueType};

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
    assert_eq!(registry.len(), 48);
    assert!(registry.lookup("SWITCH").is_none());

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
        ("ISBLANK", FunctionCategory::Information),
        ("ISERROR", FunctionCategory::Information),
        ("ISNA", FunctionCategory::Information),
        ("ERROR.TYPE", FunctionCategory::Information),
        ("NA", FunctionCategory::Information),
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

    // Sparse numeric slice: [10.0, Empty, 20.0, Empty, 30.0] -> 20.0
    let range_arg = FunctionArg::array(
        5,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Empty,
            CellValue::Number(20.0),
            CellValue::Empty,
            CellValue::Number(30.0),
        ],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );

    let res = avg.evaluate(&[range_arg], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(20.0));

    // All-empty numeric slice returns #DIV/0!
    let empty_range = FunctionArg::array(
        2,
        1,
        vec![CellValue::Empty, CellValue::Empty],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    let res = avg.evaluate(&[empty_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Div0));

    // Error in range bubbles immediately
    let err_range = FunctionArg::array(
        2,
        1,
        vec![CellValue::Number(10.0), CellValue::Error(ExcelError::Ref)],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    let res = avg.evaluate(&[err_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Ref));

    // Text, Boolean, Scalar Empty, and SingleCell Empty fail strictly with InvalidArgumentType
    for (bad_arg, found_type) in [
        (
            FunctionArg::Scalar(CellValue::Text("text".into())),
            ValueType::Text,
        ),
        (
            FunctionArg::Scalar(CellValue::Boolean(true)),
            ValueType::Boolean,
        ),
        (FunctionArg::Scalar(CellValue::Empty), ValueType::Empty),
        (
            FunctionArg::array(
                1,
                1,
                vec![CellValue::Empty],
                ReferenceKind::SingleCell,
                AxisRole::Measure,
            ),
            ValueType::Empty,
        ),
        (
            FunctionArg::array(
                2,
                1,
                vec![CellValue::Number(10.0), CellValue::Text("bad".into())],
                ReferenceKind::Slice,
                AxisRole::Measure,
            ),
            ValueType::Text,
        ),
    ] {
        assert_eq!(
            avg.evaluate(&[bad_arg], &ctx),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "AVERAGE",
                param: "number1",
                position: 1,
                expected: ExpectedType::Number,
                found: found_type,
            }))
        );
    }
}

#[test]
fn test_scenario_3_count_vs_counta_semantics() {
    let registry = FunctionRegistry::standard();
    let count_func = registry.lookup("COUNT").unwrap();
    let counta_func = registry.lookup("COUNTA").unwrap();
    let sum_func = registry.lookup("SUM").unwrap();
    let ctx = MockEvalContext;

    // COUNT on sparse numeric Measure slice skips both Empty and Error(_) (+0), counting finite numbers (+1)
    let numeric_sparse_err_slice = FunctionArg::array(
        4,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Empty,
            CellValue::Error(ExcelError::Div0),
            CellValue::Number(20.0),
        ],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    assert_eq!(
        count_func
            .evaluate(std::slice::from_ref(&numeric_sparse_err_slice), &ctx)
            .unwrap(),
        CellValue::Number(2.0)
    );
    // By contrast, SUM on the same slice bubbles ExcelError::Div0
    assert_eq!(
        sum_func
            .evaluate(std::slice::from_ref(&numeric_sparse_err_slice), &ctx)
            .unwrap(),
        CellValue::Error(ExcelError::Div0)
    );

    // COUNT rejects Text, Boolean, SingleCell Empty, and Descriptor role
    assert_eq!(
        count_func.evaluate(
            &[FunctionArg::array(
                2,
                1,
                vec![CellValue::Number(10.0), CellValue::Text("text".into())],
                ReferenceKind::Slice,
                AxisRole::Measure,
            )],
            &ctx,
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "COUNT",
            param: "value1",
            position: 1,
            expected: ExpectedType::Number,
            found: ValueType::Text,
        }))
    );
    assert_eq!(
        count_func.evaluate(
            &[FunctionArg::array(
                1,
                1,
                vec![CellValue::Empty],
                ReferenceKind::SingleCell,
                AxisRole::Measure,
            )],
            &ctx,
        ),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "COUNT",
            param: "value1",
            position: 1,
            expected: ExpectedType::Number,
            found: ValueType::Empty,
        }))
    );
    assert_eq!(
        count_func.evaluate(
            &[FunctionArg::array(
                2,
                1,
                vec![CellValue::Number(10.0), CellValue::Number(20.0)],
                ReferenceKind::Slice,
                AxisRole::Descriptor,
            )],
            &ctx,
        ),
        Err(FunctionError::DescriptorReferenceNotAllowed {
            function: "COUNT",
            param: "value1",
            position: 1,
        })
    );

    // COUNTA accepts mixed types, Empty, and Descriptor role: non-empty cells counted -> 4.0
    let mixed_descriptor_range = FunctionArg::array(
        5,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Text("text".into()),
            CellValue::Boolean(true),
            CellValue::Empty,
            CellValue::Error(ExcelError::Div0),
        ],
        ReferenceKind::Slice,
        AxisRole::Descriptor,
    );
    assert_eq!(
        counta_func
            .evaluate(std::slice::from_ref(&mixed_descriptor_range), &ctx)
            .unwrap(),
        CellValue::Number(4.0)
    );

    // Single-cell numeric/error references are accepted by COUNT; mixed single-cell references accepted by COUNTA
    let count_single_cells = [
        FunctionArg::array(
            1,
            1,
            vec![CellValue::Number(10.0)],
            ReferenceKind::SingleCell,
            AxisRole::Measure,
        ),
        FunctionArg::array(
            1,
            1,
            vec![CellValue::Error(ExcelError::Div0)],
            ReferenceKind::SingleCell,
            AxisRole::Measure,
        ),
    ];
    assert_eq!(
        count_func.evaluate(&count_single_cells, &ctx).unwrap(),
        CellValue::Number(1.0)
    );

    let counta_single_cells = [
        FunctionArg::array(
            1,
            1,
            vec![CellValue::Number(10.0)],
            ReferenceKind::SingleCell,
            AxisRole::Measure,
        ),
        FunctionArg::array(
            1,
            1,
            vec![CellValue::Text("hello".into())],
            ReferenceKind::SingleCell,
            AxisRole::Descriptor,
        ),
        FunctionArg::array(
            1,
            1,
            vec![CellValue::Empty],
            ReferenceKind::SingleCell,
            AxisRole::Measure,
        ),
        FunctionArg::array(
            1,
            1,
            vec![CellValue::Error(ExcelError::Div0)],
            ReferenceKind::SingleCell,
            AxisRole::Measure,
        ),
    ];
    assert_eq!(
        counta_func.evaluate(&counta_single_cells, &ctx).unwrap(),
        CellValue::Number(3.0)
    );

    // Non-reference scalars are rejected by ParamKind::Range
    assert_eq!(
        count_func.evaluate(&[FunctionArg::Scalar(CellValue::Number(10.0))], &ctx),
        Err(FunctionError::InvalidArgumentKind {
            function: "COUNT",
            expected: "range",
            found: "scalar",
        })
    );
    assert_eq!(
        counta_func.evaluate(
            &[FunctionArg::Scalar(CellValue::Text("hello".into()))],
            &ctx
        ),
        Err(FunctionError::InvalidArgumentKind {
            function: "COUNTA",
            expected: "range",
            found: "scalar",
        })
    );
}

#[test]
fn test_scenario_4_max_and_min_with_range_filtering_and_empty_fallback() {
    let registry = FunctionRegistry::standard();
    let max_func = registry.lookup("MAX").unwrap();
    let min_func = registry.lookup("MIN").unwrap();
    let ctx = MockEvalContext;

    // Sparse numeric slice containing [-15.0, Empty, 0.0, 42.5]
    let sparse_range = FunctionArg::array(
        4,
        1,
        vec![
            CellValue::Number(-15.0),
            CellValue::Empty,
            CellValue::Number(0.0),
            CellValue::Number(42.5),
        ],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    assert_eq!(
        max_func
            .evaluate(std::slice::from_ref(&sparse_range), &ctx)
            .unwrap(),
        CellValue::Number(42.5)
    );
    assert_eq!(
        min_func
            .evaluate(std::slice::from_ref(&sparse_range), &ctx)
            .unwrap(),
        CellValue::Number(-15.0)
    );

    // Range containing all negative numbers: [-20.0, -10.0, -5.0]
    let negative_range = FunctionArg::array(
        3,
        1,
        vec![
            CellValue::Number(-20.0),
            CellValue::Number(-10.0),
            CellValue::Number(-5.0),
        ],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
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

    // All-empty numeric slice defaults to 0.0
    let empty_range = FunctionArg::array(
        2,
        1,
        vec![CellValue::Empty, CellValue::Empty],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
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

    // Text in slice or Empty on SingleCell fails strictly with InvalidArgumentType
    let text_range = FunctionArg::array(
        1,
        1,
        vec![CellValue::Text("text".into())],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    assert_eq!(
        min_func.evaluate(std::slice::from_ref(&text_range), &ctx),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "MIN",
            param: "number1",
            position: 1,
            expected: ExpectedType::Number,
            found: ValueType::Text,
        }))
    );
    let single_empty = FunctionArg::array(
        1,
        1,
        vec![CellValue::Empty],
        ReferenceKind::SingleCell,
        AxisRole::Measure,
    );
    assert_eq!(
        max_func.evaluate(&[single_empty], &ctx),
        Err(FunctionError::Type(TypeError::InvalidArgumentType {
            function: "MAX",
            param: "number1",
            position: 1,
            expected: ExpectedType::Number,
            found: ValueType::Empty,
        }))
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

    // ROUND with non-integer or oversized digits fails strictly with NonIntegerArgument
    for bad_digits in [1.9, 1e20] {
        let args_non_int = [
            FunctionArg::Scalar(CellValue::Number(2.15)),
            FunctionArg::Scalar(CellValue::Number(bad_digits)),
        ];
        assert_eq!(
            round_func.evaluate(&args_non_int, &ctx),
            Err(FunctionError::Type(TypeError::NonIntegerArgument {
                function: "ROUND",
                param: "num_digits",
                position: 2,
                value: bad_digits.to_string(),
            }))
        );
    }
}

#[test]
fn test_scenario_6_sumproduct_dimension_checking_and_calculation() {
    let registry = FunctionRegistry::standard();
    let sumproduct = registry.lookup("SUMPRODUCT").unwrap();
    let ctx = MockEvalContext;

    // Two ranges of dimensions 3x1: [1.0, 2.0, 3.0] and [4.0, 5.0, 6.0]
    let r1 = FunctionArg::array(
        3,
        1,
        vec![
            CellValue::Number(1.0),
            CellValue::Number(2.0),
            CellValue::Number(3.0),
        ],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    let r2 = FunctionArg::array(
        3,
        1,
        vec![
            CellValue::Number(4.0),
            CellValue::Number(5.0),
            CellValue::Number(6.0),
        ],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );

    // 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32.0
    let res = sumproduct.evaluate(&[r1, r2], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(32.0));

    // Sparse Empty in syntactic Slice contributes 0.0: [Empty, 0.0, 20.0, Empty] * [10.0, 2.0, 4.0, 5.0] = 80.0
    let r_sparse = FunctionArg::array(
        4,
        1,
        vec![
            CellValue::Empty,
            CellValue::Number(0.0),
            CellValue::Number(20.0),
            CellValue::Empty,
        ],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    let r_weights = FunctionArg::array(
        4,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(2.0),
            CellValue::Number(4.0),
            CellValue::Number(5.0),
        ],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    assert_eq!(
        sumproduct.evaluate(&[r_sparse, r_weights], &ctx).unwrap(),
        CellValue::Number(80.0)
    );

    // Non-numeric text or boolean in range fails strictly with InvalidArgumentType
    for (bad_cell, found_type) in [
        (CellValue::Text("text".into()), ValueType::Text),
        (CellValue::Boolean(true), ValueType::Boolean),
    ] {
        let r_bad = FunctionArg::array(
            3,
            1,
            vec![CellValue::Number(1.0), bad_cell, CellValue::Number(3.0)],
            ReferenceKind::Slice,
            AxisRole::Measure,
        );
        let r_other = FunctionArg::array(
            3,
            1,
            vec![
                CellValue::Number(4.0),
                CellValue::Number(5.0),
                CellValue::Number(6.0),
            ],
            ReferenceKind::Slice,
            AxisRole::Measure,
        );
        assert_eq!(
            sumproduct.evaluate(&[r_bad, r_other], &ctx),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "SUMPRODUCT",
                param: "array1",
                position: 1,
                expected: ExpectedType::Number,
                found: found_type,
            }))
        );
    }

    // Mismatched dimensions (3x1 and 2x1) -> FunctionError::DimensionMismatch (precedes role & element checks)
    let r_mismatch = FunctionArg::array(
        2,
        1,
        vec![CellValue::Number(4.0), CellValue::Number(5.0)],
        ReferenceKind::Slice,
        AxisRole::Descriptor,
    );
    let r_base = FunctionArg::array(
        3,
        1,
        vec![
            CellValue::Error(ExcelError::Div0),
            CellValue::Number(2.0),
            CellValue::Number(3.0),
        ],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    assert_eq!(
        sumproduct.evaluate(&[r_base, r_mismatch], &ctx),
        Err(FunctionError::DimensionMismatch {
            function: "SUMPRODUCT",
            expected_rows: 3,
            expected_cols: 1,
            found_rows: 2,
            found_cols: 1,
        })
    );

    // Error in range bubbles immediately
    let r_err = FunctionArg::array(
        2,
        1,
        vec![CellValue::Number(1.0), CellValue::Error(ExcelError::Div0)],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    let r_valid = FunctionArg::array(
        2,
        1,
        vec![CellValue::Number(1.0), CellValue::Number(2.0)],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    let res_err = sumproduct.evaluate(&[r_err, r_valid], &ctx).unwrap();
    assert_eq!(res_err, CellValue::Error(ExcelError::Div0));

    // Malformed array element count mismatch returns evaluation failure error
    let r_malformed = FunctionArg::array(
        2,
        2,
        vec![CellValue::Number(1.0)],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
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
    let odd_range = FunctionArg::array(
        3,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(50.0),
            CellValue::Number(20.0),
        ],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    let res = median.evaluate(&[odd_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(20.0));

    // 2. Even count of numbers: [10, 40, 20, 30] -> sorted [10, 20, 30, 40] -> median is (20 + 30) / 2 = 25
    let even_range = FunctionArg::array(
        4,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(40.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    let res = median.evaluate(&[even_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(25.0));

    // 3. Sparse numeric slice skips Empty cells
    let sparse_range = FunctionArg::array(
        5,
        1,
        vec![
            CellValue::Number(100.0),
            CellValue::Empty,
            CellValue::Number(300.0),
            CellValue::Empty,
            CellValue::Number(200.0),
        ],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    let res = median.evaluate(&[sparse_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(200.0));

    // 4. All-empty numeric slice returns #NUM! in Excel
    let empty_range = FunctionArg::array(
        2,
        1,
        vec![CellValue::Empty, CellValue::Empty],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    let res = median.evaluate(&[empty_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Num));

    // 5. Direct scalar boolean or text or SingleCell Empty fails strictly with InvalidArgumentType
    for (bad_args, pos, found) in [
        (
            vec![
                FunctionArg::Scalar(CellValue::Number(10.0)),
                FunctionArg::Scalar(CellValue::Boolean(true)),
            ],
            2,
            ValueType::Boolean,
        ),
        (
            vec![
                FunctionArg::Scalar(CellValue::Number(10.0)),
                FunctionArg::Scalar(CellValue::Text("invalid".into())),
            ],
            2,
            ValueType::Text,
        ),
        (
            vec![FunctionArg::array(
                1,
                1,
                vec![CellValue::Empty],
                ReferenceKind::SingleCell,
                AxisRole::Measure,
            )],
            1,
            ValueType::Empty,
        ),
    ] {
        assert_eq!(
            median.evaluate(&bad_args, &ctx),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "MEDIAN",
                param: "number1",
                position: pos,
                expected: ExpectedType::Number,
                found,
            }))
        );
    }

    // 6. Error in range bubbles immediately
    let error_range = FunctionArg::array(
        2,
        1,
        vec![CellValue::Number(10.0), CellValue::Error(ExcelError::Ref)],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    let res = median.evaluate(&[error_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Ref));

    // 7. Too few arguments (0 args)
    let res_empty = median.evaluate(&[], &ctx);
    assert!(matches!(
        res_empty,
        Err(FunctionError::TooFewArguments { .. })
    ));
}

#[test]
fn test_cluster_b_aggregate_overflow_and_isblank_dormant_guard() {
    let registry = FunctionRegistry::standard();
    let ctx = MockEvalContext;
    let sum = registry.lookup("SUM").unwrap();
    let avg = registry.lookup("AVERAGE").unwrap();
    let max_fn = registry.lookup("MAX").unwrap();
    let min_fn = registry.lookup("MIN").unwrap();
    let median = registry.lookup("MEDIAN").unwrap();
    let sumproduct = registry.lookup("SUMPRODUCT").unwrap();

    // Overflow in SUM / AVERAGE / MEDIAN / SUMPRODUCT routes through CellValue::number -> #NUM!
    let extreme_slice = FunctionArg::array(
        2,
        1,
        vec![CellValue::Number(1e308), CellValue::Number(1e308)],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    assert_eq!(
        sum.evaluate(std::slice::from_ref(&extreme_slice), &ctx)
            .unwrap(),
        CellValue::Error(ExcelError::Num)
    );
    assert_eq!(
        avg.evaluate(std::slice::from_ref(&extreme_slice), &ctx)
            .unwrap(),
        CellValue::Error(ExcelError::Num)
    );
    assert_eq!(
        median
            .evaluate(std::slice::from_ref(&extreme_slice), &ctx)
            .unwrap(),
        CellValue::Error(ExcelError::Num)
    );
    // MAX and MIN on finite extreme floats remain finite 1e308
    assert_eq!(
        max_fn
            .evaluate(std::slice::from_ref(&extreme_slice), &ctx)
            .unwrap(),
        CellValue::Number(1e308)
    );
    assert_eq!(
        min_fn
            .evaluate(std::slice::from_ref(&extreme_slice), &ctx)
            .unwrap(),
        CellValue::Number(1e308)
    );

    let r_1e200 = FunctionArg::array(
        1,
        1,
        vec![CellValue::Number(1e200)],
        ReferenceKind::Slice,
        AxisRole::Measure,
    );
    assert_eq!(
        sumproduct
            .evaluate(&[r_1e200.clone(), r_1e200], &ctx)
            .unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // ISBLANK dormant guard on SingleCell Empty: IF(ISBLANK(T["r1", "val"]), 0, SUM(T["r1", "val"])) -> 0.0
    let mut symbols = SymbolTable::new();
    let sheet_id = SheetId::from("Sheet1");
    let spec = TableSpec {
        id: TableId::from("T"),
        title: "T".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("val")],
        rows: vec![RowSpec::new("r1"), RowSpec::new("r2")],
    };
    symbols.register_table(sheet_id, &spec).unwrap();

    let cell_r1 = CellId::new("T", "r1", "val");
    let origin = CellId::new("T", "r2", "val");
    let mut values = IndexMap::new();
    values.insert(cell_r1, CellValue::Empty);

    let guarded =
        Expr::parse("IF(ISBLANK(T[\"r1\", \"val\"]), 0, SUM(T[\"r1\", \"val\"]))").unwrap();
    assert_eq!(
        eval_expr(&guarded, &origin, &values, &symbols, &registry).unwrap(),
        CellValue::Number(0.0)
    );

    // Unguarded SUM(T["r1", "val"]) on SingleCell Empty fails even inside IFERROR
    let unguarded = Expr::parse("IFERROR(SUM(T[\"r1\", \"val\"]), 0)").unwrap();
    assert!(eval_expr(&unguarded, &origin, &values, &symbols, &registry).is_err());
}
