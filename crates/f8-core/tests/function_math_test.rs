#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use indexmap::IndexMap;

use f8_core::compiler::eval::{EvalError, eval_expr};
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::ir::symbol::SymbolTable;
use f8_core::expr::ast::Expr;
use f8_core::functions::traits::{EvalContext, FunctionArg, ReferenceKind};
use f8_core::functions::{FunctionCategory, FunctionError, FunctionRegistry, ValidationPolicy};
use f8_core::model::id::{SheetId, TableId};
use f8_core::model::table::{AxisRole, ColumnSpec, RowSpec, TableSpec};
use f8_core::value::{CellValue, ExcelError, TypeError, ValueType};

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
    assert_eq!(registry.len(), 37);
    for retired in [
        "SWITCH",
        "SUMIFS",
        "COUNTIFS",
        "AVERAGEIFS",
        "MINIFS",
        "MAXIFS",
        "IPMT",
        "PPMT",
        "COUNTA",
        "IFS",
        "IFNA",
        "DAYS",
        "ISBLANK",
        "ISNA",
        "ERROR.TYPE",
    ] {
        assert!(
            registry.lookup(retired).is_none(),
            "{retired} must be absent"
        );
    }

    let expected_functions = [
        ("ABS", FunctionCategory::Math),
        ("LN", FunctionCategory::Math),
        ("EXP", FunctionCategory::Math),
        ("MOD", FunctionCategory::Math),
        ("SUM", FunctionCategory::Math),
        ("AVERAGE", FunctionCategory::Math),
        ("COUNT", FunctionCategory::Math),
        ("MAX", FunctionCategory::Math),
        ("MIN", FunctionCategory::Math),
        ("MEDIAN", FunctionCategory::Math),
        ("ROUND", FunctionCategory::Math),
        ("ROUNDUP", FunctionCategory::Math),
        ("ROUNDDOWN", FunctionCategory::Math),
        ("SUMPRODUCT", FunctionCategory::Math),
        ("IF", FunctionCategory::Logical),
        ("IFERROR", FunctionCategory::Logical),
        ("AND", FunctionCategory::Logical),
        ("OR", FunctionCategory::Logical),
        ("NOT", FunctionCategory::Logical),
        ("CHOOSE", FunctionCategory::Logical),
        ("NPV", FunctionCategory::Financial),
        ("XNPV", FunctionCategory::Financial),
        ("IRR", FunctionCategory::Financial),
        ("XIRR", FunctionCategory::Financial),
        ("PMT", FunctionCategory::Financial),
        ("XLOOKUP", FunctionCategory::Lookup),
        ("XMATCH", FunctionCategory::Lookup),
        ("INDEX", FunctionCategory::Lookup),
        ("ISERROR", FunctionCategory::Information),
        ("NA", FunctionCategory::Information),
    ];

    for (name, category) in expected_functions {
        let func = registry
            .lookup(name)
            .unwrap_or_else(|| panic!("Function {name} should be registered"));
        assert_eq!(func.name(), name);
        assert_eq!(func.category(), category);
    }

    // Invariant: Every FunctionCategory::Math function enforces ParamType::Number, return_type Some(Number),
    // ValidationPolicy::Standard (except SUMPRODUCT), and RolePolicy::MeasureOnly on range-accepting parameters
    for (name, func) in registry.iter() {
        if func.category() == FunctionCategory::Math {
            assert_eq!(
                func.return_type(),
                Some(ValueType::Number),
                "Math function {name} must return Some(ValueType::Number)"
            );
            if name != "SUMPRODUCT" {
                assert_eq!(
                    func.validation_policy(),
                    ValidationPolicy::Standard,
                    "Math function {name} must use ValidationPolicy::Standard"
                );
            }
            for param in func.parameters() {
                assert_eq!(
                    param.param_type,
                    f8_core::functions::ParamType::Number,
                    "Math function {name} param {} must enforce ParamType::Number",
                    param.name
                );
                if matches!(
                    param.kind,
                    f8_core::functions::ParamKind::Range
                        | f8_core::functions::ParamKind::ScalarOrRange
                ) {
                    assert_eq!(
                        param.role_policy,
                        f8_core::functions::RolePolicy::MeasureOnly,
                        "Math function {name} param {} must enforce RolePolicy::MeasureOnly",
                        param.name
                    );
                }
            }
        }
    }
}

#[test]
fn test_scenario_2_average_calculation_and_empty_divisor() {
    let registry = FunctionRegistry::standard();
    let avg = registry.lookup("AVERAGE").unwrap();
    let ctx = MockEvalContext;

    // Sparse numeric slice: [10.0, Empty, 20.0, Empty, 30.0] -> 20.0
    let range_arg = FunctionArg::range(
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
    );

    let res = avg.evaluate(&[range_arg], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(20.0));

    // All-empty numeric slice returns #DIV/0!
    let empty_range = FunctionArg::range(
        2,
        1,
        vec![CellValue::Empty, CellValue::Empty],
        ReferenceKind::Slice,
    );
    let res = avg.evaluate(&[empty_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Div0));

    // Error in range bubbles immediately
    let err_range = FunctionArg::range(
        2,
        1,
        vec![CellValue::Number(10.0), CellValue::Error(ExcelError::Ref)],
        ReferenceKind::Slice,
    );
    let res = avg.evaluate(&[err_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Ref));

    // Text and Boolean fail in Pass 4a with InvalidArgumentType; Empty fails in bind_arguments with EmptyArgument
    for (formula, found_type) in [
        ("AVERAGE(\"text\")", ValueType::Text),
        ("AVERAGE(TRUE)", ValueType::Boolean),
    ] {
        assert_eq!(
            registry.validate_expr(&Expr::parse(formula).unwrap()),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "AVERAGE",
                param: "number1",
                position: 1,
                expected: ValueType::Number,
                found: found_type,
            }))
        );
    }
    let avg_slice_expr = Expr::parse("AVERAGE(T[\"r1\"..\"r2\", \"val\"])").unwrap();
    let mut resolve_avg_bad = |_: &f8_core::expr::TableRef| -> Result<
        Option<f8_core::functions::StaticRef>,
        &'static str,
    > {
        Ok(Some(f8_core::functions::StaticRef {
            rows: 2,
            cols: 1,
            axis_role: AxisRole::Measure,
            elem_types: vec![Some(ValueType::Number), Some(ValueType::Text)],
        }))
    };
    assert_eq!(
        registry.validate_expr_with_shapes(&avg_slice_expr, &mut resolve_avg_bad),
        Err(f8_core::functions::StageAError::Function(
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "AVERAGE",
                param: "number1",
                position: 1,
                expected: ValueType::Number,
                found: ValueType::Text,
            })
        ))
    );
}

#[test]
fn test_scenario_3_count_semantics_and_retired_counta() {
    let registry = FunctionRegistry::standard();
    let count_func = registry.lookup("COUNT").unwrap();
    assert!(registry.lookup("COUNTA").is_none());
    let sum_func = registry.lookup("SUM").unwrap();
    let ctx = MockEvalContext;

    // COUNT on sparse numeric Measure slice skips both Empty and Error(_) (+0), counting finite numbers (+1)
    let numeric_sparse_err_slice = FunctionArg::range(
        4,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Empty,
            CellValue::Error(ExcelError::Div0),
            CellValue::Number(20.0),
        ],
        ReferenceKind::Slice,
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
    let count_slice_expr = Expr::parse("COUNT(T[\"r1\"..\"r2\", \"val\"])").unwrap();
    let mut resolve_count_text = |_: &f8_core::expr::TableRef| -> Result<
        Option<f8_core::functions::StaticRef>,
        &'static str,
    > {
        Ok(Some(f8_core::functions::StaticRef {
            rows: 2,
            cols: 1,
            axis_role: AxisRole::Measure,
            elem_types: vec![Some(ValueType::Number), Some(ValueType::Text)],
        }))
    };
    assert_eq!(
        registry.validate_expr_with_shapes(&count_slice_expr, &mut resolve_count_text),
        Err(f8_core::functions::StageAError::Function(
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "COUNT",
                param: "value1",
                position: 1,
                expected: ValueType::Number,
                found: ValueType::Text,
            })
        ))
    );
    let count_desc_expr = f8_core::expr::Expr::parse("COUNT(T[\"r1\"..\"r2\", \"desc\"])").unwrap();
    let mut resolve_count_desc = |_: &f8_core::expr::TableRef| -> Result<
        Option<f8_core::functions::StaticRef>,
        &'static str,
    > {
        Ok(Some(f8_core::functions::StaticRef {
            rows: 2,
            cols: 1,
            axis_role: AxisRole::Descriptor,
            elem_types: vec![Some(ValueType::Number); 2],
        }))
    };
    assert_eq!(
        registry.validate_expr_with_shapes(&count_desc_expr, &mut resolve_count_desc),
        Err(f8_core::functions::StageAError::Function(
            FunctionError::DescriptorReferenceNotAllowed {
                function: "COUNT",
                param: "value1",
                position: 1,
            }
        ))
    );

    // Single-cell numeric/error references are accepted by COUNT; single-cell Empty, Text, Boolean return #VALUE!; Scalar returns InvalidArgumentKind; 0 args returns TooFewArguments
    let count_single_cells = [
        FunctionArg::range(
            1,
            1,
            vec![CellValue::Number(10.0)],
            ReferenceKind::SingleCell,
        ),
        FunctionArg::range(
            1,
            1,
            vec![CellValue::Error(ExcelError::Div0)],
            ReferenceKind::SingleCell,
        ),
    ];
    assert_eq!(
        count_func.evaluate(&count_single_cells, &ctx).unwrap(),
        CellValue::Number(1.0)
    );
    assert_eq!(
        count_func.evaluate(&[], &ctx),
        Err(FunctionError::TooFewArguments {
            function: "COUNT",
            expected: 1,
            found: 0,
        })
    );
    assert_eq!(
        count_func.evaluate(&[FunctionArg::Scalar(CellValue::Number(10.0))], &ctx),
        Err(FunctionError::InvalidArgumentKind {
            function: "COUNT",
            expected: "range",
            found: "scalar",
        })
    );
    for bad_cell in [
        FunctionArg::range(1, 1, vec![CellValue::Empty], ReferenceKind::SingleCell),
        FunctionArg::range(
            1,
            1,
            vec![CellValue::Text("abc".into())],
            ReferenceKind::Slice,
        ),
        FunctionArg::range(1, 1, vec![CellValue::Boolean(true)], ReferenceKind::Slice),
    ] {
        assert_eq!(
            count_func.evaluate(&[bad_cell], &ctx),
            Ok(CellValue::Error(ExcelError::Value))
        );
    }

    // Non-reference scalars are rejected statically by ParamKind::Range; COUNTA is rejected as UnknownFunction
    assert_eq!(
        registry.validate_expr(&f8_core::expr::Expr::parse("COUNT(10)").unwrap()),
        Err(FunctionError::InvalidArgumentKind {
            function: "COUNT",
            expected: "range",
            found: "scalar",
        })
    );
    assert_eq!(
        registry.validate_expr(
            &f8_core::expr::Expr::parse("COUNTA(T[\"r1\"..\"r2\", \"val\"])").unwrap()
        ),
        Err(FunctionError::UnknownFunction("COUNTA".into()))
    );
}

#[test]
fn test_scenario_4_max_and_min_with_range_filtering_and_empty_fallback() {
    let registry = FunctionRegistry::standard();
    let max_func = registry.lookup("MAX").unwrap();
    let min_func = registry.lookup("MIN").unwrap();
    let ctx = MockEvalContext;

    // Sparse numeric slice containing [-15.0, Empty, 0.0, 42.5]
    let sparse_range = FunctionArg::range(
        4,
        1,
        vec![
            CellValue::Number(-15.0),
            CellValue::Empty,
            CellValue::Number(0.0),
            CellValue::Number(42.5),
        ],
        ReferenceKind::Slice,
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
    let negative_range = FunctionArg::range(
        3,
        1,
        vec![
            CellValue::Number(-20.0),
            CellValue::Number(-10.0),
            CellValue::Number(-5.0),
        ],
        ReferenceKind::Slice,
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
    let empty_range = FunctionArg::range(
        2,
        1,
        vec![CellValue::Empty, CellValue::Empty],
        ReferenceKind::Slice,
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

    // Text in slice fails in Pass 4a with InvalidArgumentType
    let min_text_expr = Expr::parse("MIN(T[\"r1\"..\"r2\", \"val\"])").unwrap();
    let mut resolve_min_text = |_: &f8_core::expr::TableRef| -> Result<
        Option<f8_core::functions::StaticRef>,
        &'static str,
    > {
        Ok(Some(f8_core::functions::StaticRef {
            rows: 1,
            cols: 1,
            axis_role: AxisRole::Measure,
            elem_types: vec![Some(ValueType::Text)],
        }))
    };
    assert_eq!(
        registry.validate_expr_with_shapes(&min_text_expr, &mut resolve_min_text),
        Err(f8_core::functions::StageAError::Function(
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "MIN",
                param: "number1",
                position: 1,
                expected: ValueType::Number,
                found: ValueType::Text,
            })
        ))
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

    // ROUND with non-integer or oversized digits returns #NUM!
    for bad_digits in [1.9, 1e20] {
        let args_non_int = [
            FunctionArg::Scalar(CellValue::Number(2.15)),
            FunctionArg::Scalar(CellValue::Number(bad_digits)),
        ];
        assert_eq!(
            round_func.evaluate(&args_non_int, &ctx),
            Ok(CellValue::Error(ExcelError::Num))
        );
    }

    // Exact base-10 15-significant-digit rounding (no binary float * 10^d drift)
    for (num, digits, expected) in [
        (1.005, 2.0, 1.01),
        (-1.005, 2.0, -1.01),
        (2.675, 2.0, 2.68),
        (-2.675, 2.0, -2.68),
        (100.0, 308.0, 100.0),
        (100.0, 1000.0, 100.0),
        (1.0 / 3.0, 16.0, 1.0 / 3.0),
        (1.0 / 3.0, 15.0, 0.333_333_333_333_333),
        (1.2345e-20, 22.0, 1.23e-20),
        (1.2345e-20, 23.0, 1.235e-20),
        (1.2345e-310, 312.0, 1.23e-310),
        (-1.0, -500.0, 0.0),
        (5e307, -308.0, 1e308),
    ] {
        let args = [
            FunctionArg::Scalar(CellValue::Number(num)),
            FunctionArg::Scalar(CellValue::Number(digits)),
        ];
        let actual = round_func.evaluate(&args, &ctx).unwrap();
        assert_eq!(
            actual,
            CellValue::Number(expected),
            "ROUND({num:?}, {digits}) mismatch"
        );
        if let CellValue::Number(n) = actual {
            assert_eq!(n.to_bits(), expected.to_bits());
        }
    }

    // Carry boundary monotonicity at 1 - eps/2
    let near_one = 1.0 - f64::EPSILON / 2.0;
    for (digits, expected) in [(16.0, near_one), (15.0, 1.0), (14.0, 1.0)] {
        let args = [
            FunctionArg::Scalar(CellValue::Number(near_one)),
            FunctionArg::Scalar(CellValue::Number(digits)),
        ];
        assert_eq!(
            round_func.evaluate(&args, &ctx).unwrap(),
            CellValue::Number(expected)
        );
    }

    // Exact decimal ROUNDUP / ROUNDDOWN without binary representation drift
    for (num, digits, expected) in [(0.07, 2.0, 0.07), (-0.07, 2.0, -0.07), (1.0, 500.0, 1.0)] {
        let args = [
            FunctionArg::Scalar(CellValue::Number(num)),
            FunctionArg::Scalar(CellValue::Number(digits)),
        ];
        assert_eq!(
            roundup_func.evaluate(&args, &ctx).unwrap(),
            CellValue::Number(expected)
        );
    }
    for (num, digits, expected) in [
        (0.29, 2.0, 0.29),
        (-0.29, 2.0, -0.29),
        (-1e308, -500.0, 0.0),
    ] {
        let args = [
            FunctionArg::Scalar(CellValue::Number(num)),
            FunctionArg::Scalar(CellValue::Number(digits)),
        ];
        let actual = rounddown_func.evaluate(&args, &ctx).unwrap();
        assert_eq!(actual, CellValue::Number(expected));
        if let CellValue::Number(n) = actual {
            assert_eq!(n.to_bits(), expected.to_bits());
        }
    }

    // Overflow beyond IEEE-754 f64::MAX returns #NUM!
    for (func, num, digits) in [
        (&roundup_func, 1.0, -309.0),
        (&roundup_func, -1.0, -309.0),
        (&roundup_func, 1.5e308, -308.0),
        (&roundup_func, 1.0, -9_007_199_254_740_991.0),
        (&round_func, 1.6e308, -308.0),
    ] {
        let args = [
            FunctionArg::Scalar(CellValue::Number(num)),
            FunctionArg::Scalar(CellValue::Number(digits)),
        ];
        assert_eq!(
            func.evaluate(&args, &ctx).unwrap(),
            CellValue::Error(ExcelError::Num),
            "expected #NUM! for {}({num:?}, {digits})",
            func.name()
        );
    }
}

#[test]
fn test_scenario_6_sumproduct_dimension_checking_and_calculation() {
    let registry = FunctionRegistry::standard();
    let sumproduct = registry.lookup("SUMPRODUCT").unwrap();
    let ctx = MockEvalContext;

    // Two ranges of dimensions 3x1: [1.0, 2.0, 3.0] and [4.0, 5.0, 6.0]
    let r1 = FunctionArg::range(
        3,
        1,
        vec![
            CellValue::Number(1.0),
            CellValue::Number(2.0),
            CellValue::Number(3.0),
        ],
        ReferenceKind::Slice,
    );
    let r2 = FunctionArg::range(
        3,
        1,
        vec![
            CellValue::Number(4.0),
            CellValue::Number(5.0),
            CellValue::Number(6.0),
        ],
        ReferenceKind::Slice,
    );

    // 1*4 + 2*5 + 3*6 = 4 + 10 + 18 = 32.0
    let res = sumproduct.evaluate(&[r1, r2], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(32.0));

    // Sparse Empty in syntactic Slice contributes 0.0: [Empty, 0.0, 20.0, Empty] * [10.0, 2.0, 4.0, 5.0] = 80.0
    let r_sparse = FunctionArg::range(
        4,
        1,
        vec![
            CellValue::Empty,
            CellValue::Number(0.0),
            CellValue::Number(20.0),
            CellValue::Empty,
        ],
        ReferenceKind::Slice,
    );
    let r_weights = FunctionArg::range(
        4,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(2.0),
            CellValue::Number(4.0),
            CellValue::Number(5.0),
        ],
        ReferenceKind::Slice,
    );
    assert_eq!(
        sumproduct.evaluate(&[r_sparse, r_weights], &ctx).unwrap(),
        CellValue::Number(80.0)
    );

    // Non-numeric text or boolean in range fails in Pass 4a with InvalidArgumentType
    for found_type in [ValueType::Text, ValueType::Boolean] {
        let sp_type_expr = f8_core::expr::Expr::parse(
            "SUMPRODUCT(T[\"a1\"..\"a3\", @col], T[\"b1\"..\"b3\", @col])",
        )
        .unwrap();
        let mut resolve_sp_type = |_: &f8_core::expr::TableRef| -> Result<
            Option<f8_core::functions::StaticRef>,
            &'static str,
        > {
            Ok(Some(f8_core::functions::StaticRef {
                rows: 3,
                cols: 1,
                axis_role: AxisRole::Measure,
                elem_types: vec![
                    Some(ValueType::Number),
                    Some(found_type),
                    Some(ValueType::Number),
                ],
            }))
        };
        assert_eq!(
            registry.validate_expr_with_shapes(&sp_type_expr, &mut resolve_sp_type),
            Err(f8_core::functions::StageAError::Function(
                FunctionError::Type(TypeError::InvalidArgumentType {
                    function: "SUMPRODUCT",
                    param: "array1",
                    position: 1,
                    expected: ValueType::Number,
                    found: found_type,
                })
            ))
        );
    }

    // Mismatched dimensions (3x1 and 2x1) -> FunctionError::DimensionMismatch statically
    let sp_dim_expr =
        f8_core::expr::Expr::parse("SUMPRODUCT(T[\"a1\"..\"a3\", @col], T[\"b1\"..\"b2\", @col])")
            .unwrap();
    let mut resolve_sp_dim =
        |r: &f8_core::expr::TableRef| -> Result<Option<f8_core::functions::StaticRef>, &'static str> {
            let rows = if let f8_core::expr::RowSelector::Range(
                f8_core::expr::RowEndpoint::Key(s),
                _,
            ) = &r.row
                && s.as_str() == "a1"
            {
                3
            } else {
                2
            };
            Ok(Some(f8_core::functions::StaticRef {
                rows,
                cols: 1,
                axis_role: AxisRole::Measure,
                elem_types: vec![Some(ValueType::Number); rows],
            }))
        };
    assert_eq!(
        registry.validate_expr_with_shapes(&sp_dim_expr, &mut resolve_sp_dim),
        Err(f8_core::functions::StageAError::Function(
            FunctionError::DimensionMismatch {
                function: "SUMPRODUCT",
                expected_rows: 3,
                expected_cols: 1,
                found_rows: 2,
                found_cols: 1,
            }
        ))
    );

    // Error in range bubbles immediately
    let r_err = FunctionArg::range(
        2,
        1,
        vec![CellValue::Number(1.0), CellValue::Error(ExcelError::Div0)],
        ReferenceKind::Slice,
    );
    let r_valid = FunctionArg::range(
        2,
        1,
        vec![CellValue::Number(1.0), CellValue::Number(2.0)],
        ReferenceKind::Slice,
    );
    let res_err = sumproduct
        .evaluate(&[r_err, r_valid.clone()], &ctx)
        .unwrap();
    assert_eq!(res_err, CellValue::Error(ExcelError::Div0));

    // Direct evaluate checks on SUMPRODUCT: TooFewArguments, DimensionMismatch, and #VALUE! on Text/Boolean/SingleCell Empty
    assert_eq!(
        sumproduct.evaluate(&[], &ctx),
        Err(FunctionError::TooFewArguments {
            function: "SUMPRODUCT",
            expected: 1,
            found: 0,
        })
    );
    let r_3x1 = FunctionArg::range(
        3,
        1,
        vec![
            CellValue::Number(1.0),
            CellValue::Number(2.0),
            CellValue::Number(3.0),
        ],
        ReferenceKind::Slice,
    );
    assert_eq!(
        sumproduct.evaluate(&[r_3x1, r_valid.clone()], &ctx),
        Err(FunctionError::DimensionMismatch {
            function: "SUMPRODUCT",
            expected_rows: 3,
            expected_cols: 1,
            found_rows: 2,
            found_cols: 1,
        })
    );
    for bad_elem in [
        FunctionArg::range(
            2,
            1,
            vec![CellValue::Text("a".into()), CellValue::Number(1.0)],
            ReferenceKind::Slice,
        ),
        FunctionArg::range(
            2,
            1,
            vec![CellValue::Boolean(true), CellValue::Number(1.0)],
            ReferenceKind::Slice,
        ),
    ] {
        assert_eq!(
            sumproduct.evaluate(&[bad_elem, r_valid.clone()], &ctx),
            Ok(CellValue::Error(ExcelError::Value))
        );
    }
    assert_eq!(
        sumproduct.evaluate(
            &[FunctionArg::range(
                1,
                1,
                vec![CellValue::Empty],
                ReferenceKind::SingleCell,
            )],
            &ctx,
        ),
        Ok(CellValue::Error(ExcelError::Value))
    );
}

#[test]
fn test_scenario_11_median_odd_even_ranges_and_errors() {
    let registry = FunctionRegistry::standard();
    let median = registry.lookup("MEDIAN").unwrap();
    let ctx = MockEvalContext;

    // 1. Odd count of numbers: [10, 50, 20] -> sorted [10, 20, 50] -> median is 20
    let odd_range = FunctionArg::range(
        3,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(50.0),
            CellValue::Number(20.0),
        ],
        ReferenceKind::Slice,
    );
    let res = median.evaluate(&[odd_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(20.0));

    // 2. Even count of numbers: [10, 40, 20, 30] -> sorted [10, 20, 30, 40] -> median is (20 + 30) / 2 = 25
    let even_range = FunctionArg::range(
        4,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(40.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ],
        ReferenceKind::Slice,
    );
    let res = median.evaluate(&[even_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(25.0));

    // 3. Sparse numeric slice skips Empty cells
    let sparse_range = FunctionArg::range(
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
    );
    let res = median.evaluate(&[sparse_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(200.0));

    // 4. All-empty numeric slice returns #NUM! in Excel
    let empty_range = FunctionArg::range(
        2,
        1,
        vec![CellValue::Empty, CellValue::Empty],
        ReferenceKind::Slice,
    );
    let res = median.evaluate(&[empty_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Num));

    // 5. Direct scalar boolean or text fails in Pass 4a with InvalidArgumentType
    for (formula, pos, found) in [
        ("MEDIAN(10, TRUE)", 2, ValueType::Boolean),
        ("MEDIAN(10, \"invalid\")", 2, ValueType::Text),
    ] {
        assert_eq!(
            registry.validate_expr(&Expr::parse(formula).unwrap()),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "MEDIAN",
                param: "number1",
                position: pos,
                expected: ValueType::Number,
                found,
            }))
        );
    }

    // 6. Error in range bubbles immediately
    let error_range = FunctionArg::range(
        2,
        1,
        vec![CellValue::Number(10.0), CellValue::Error(ExcelError::Ref)],
        ReferenceKind::Slice,
    );
    let res = median.evaluate(&[error_range], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Ref));

    // 7. Too few arguments (0 args) fails in Pass 4a
    let res_empty = registry.validate_expr(&Expr::parse("MEDIAN()").unwrap());
    assert!(matches!(
        res_empty,
        Err(FunctionError::TooFewArguments { .. })
    ));
}

#[test]
fn test_cluster_b_aggregate_overflow_and_dormant_empty_guard() {
    let registry = FunctionRegistry::standard();
    let ctx = MockEvalContext;
    let sum = registry.lookup("SUM").unwrap();
    let avg = registry.lookup("AVERAGE").unwrap();
    let max_fn = registry.lookup("MAX").unwrap();
    let min_fn = registry.lookup("MIN").unwrap();
    let median = registry.lookup("MEDIAN").unwrap();
    let sumproduct = registry.lookup("SUMPRODUCT").unwrap();

    // Overflow in SUM / AVERAGE / MEDIAN / SUMPRODUCT routes through CellValue::number -> #NUM!
    let extreme_slice = FunctionArg::range(
        2,
        1,
        vec![CellValue::Number(1e308), CellValue::Number(1e308)],
        ReferenceKind::Slice,
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

    let r_1e200 = FunctionArg::range(1, 1, vec![CellValue::Number(1e200)], ReferenceKind::Slice);
    assert_eq!(
        sumproduct
            .evaluate(&[r_1e200.clone(), r_1e200], &ctx)
            .unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // Dormant guard on SingleCell Empty: IF(FALSE, SUM(T["r1", "val"]), 0) -> 0.0
    let mut symbols = SymbolTable::new();
    let sheet_id = SheetId::from("Sheet1");
    let spec = TableSpec {
        id: TableId::from("T"),
        title: "T".into(),
        subtitles: Vec::new(),
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

    let guarded = Expr::parse("IF(FALSE, SUM(T[\"r1\", \"val\"]), 0)").unwrap();
    assert_eq!(
        eval_expr(&guarded, &origin, &values, &symbols, &registry).unwrap(),
        CellValue::Number(0.0)
    );

    // Unguarded SUM(T["r1", "val"]) on SingleCell Empty fails even inside IFERROR
    let unguarded = Expr::parse("IFERROR(SUM(T[\"r1\", \"val\"]), 0)").unwrap();
    assert!(eval_expr(&unguarded, &origin, &values, &symbols, &registry).is_err());
}

fn make_eval_fixture() -> (SymbolTable, CellId, IndexMap<CellId, CellValue>) {
    let mut symbols = SymbolTable::new();
    let sheet_id = SheetId::from("Sheet1");
    let spec = TableSpec {
        id: TableId::from("T"),
        title: "T".into(),
        subtitles: Vec::new(),
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("val")],
        rows: vec![
            RowSpec::new("r1"),
            RowSpec::new("r2"),
            RowSpec::new("empty"),
        ],
    };
    symbols.register_table(sheet_id, &spec).unwrap();

    let origin = CellId::new("T", "r1", "val");
    let mut values = IndexMap::new();
    values.insert(CellId::new("T", "r1", "val"), CellValue::Number(10.0));
    values.insert(CellId::new("T", "r2", "val"), CellValue::Number(20.0));
    values.insert(CellId::new("T", "empty", "val"), CellValue::Empty);
    (symbols, origin, values)
}

#[test]
fn test_ln_nominal_inverse_and_domain_errors() {
    let registry = FunctionRegistry::standard();
    let ln = registry.lookup("LN").unwrap();
    let ctx = MockEvalContext;
    let (symbols, origin, values) = make_eval_fixture();

    // LN(1) -> exact +0.0
    let ln_one = ln
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(1.0))], &ctx)
        .unwrap();
    assert_eq!(ln_one, CellValue::Number(0.0));
    if let CellValue::Number(n) = ln_one {
        assert_eq!(n.to_bits(), 0.0_f64.to_bits());
    }

    // LN(EXP(1)) -> 1.0, LN(10) -> 10_f64.ln(), LN(0.5) -> 0.5_f64.ln()
    for (formula, expected) in [
        ("LN(1)", 0.0_f64),
        ("LN(EXP(1))", 1.0_f64),
        ("LN(10)", 10.0_f64.ln()),
        ("LN(0.5)", 0.5_f64.ln()),
    ] {
        let expr = Expr::parse(formula).unwrap();
        let actual = eval_expr(&expr, &origin, &values, &symbols, &registry).unwrap();
        assert_eq!(
            actual,
            CellValue::Number(expected),
            "mismatch on '{formula}'"
        );
        if let CellValue::Number(n) = actual {
            assert_eq!(
                n.to_bits(),
                expected.to_bits(),
                "bit mismatch on '{formula}'"
            );
        }
    }

    // Domain errors: LN(0), LN(-0.0), LN(-5) -> #NUM!; non-finite -> #NUM!
    for bad in [0.0, -0.0, -5.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            ln.evaluate(&[FunctionArg::Scalar(CellValue::Number(bad))], &ctx)
                .unwrap(),
            CellValue::Error(ExcelError::Num),
            "expected #NUM! for LN({bad:?})"
        );
    }
    for formula in ["LN(0)", "LN(-0)", "LN(-5)"] {
        let expr = Expr::parse(formula).unwrap();
        assert_eq!(
            eval_expr(&expr, &origin, &values, &symbols, &registry).unwrap(),
            CellValue::Error(ExcelError::Num),
            "expected #NUM! for '{formula}'"
        );
    }

    // Upstream error bubbling: LN(1 / 0) -> #DIV/0!
    assert_eq!(
        ln.evaluate(
            &[FunctionArg::Scalar(CellValue::Error(ExcelError::Div0))],
            &ctx,
        )
        .unwrap(),
        CellValue::Error(ExcelError::Div0)
    );
    let div0_expr = Expr::parse("LN(1 / 0)").unwrap();
    assert_eq!(
        eval_expr(&div0_expr, &origin, &values, &symbols, &registry).unwrap(),
        CellValue::Error(ExcelError::Div0)
    );
}

#[test]
fn test_exp_nominal_underflow_and_overflow_errors() {
    let registry = FunctionRegistry::standard();
    let exp = registry.lookup("EXP").unwrap();
    let ctx = MockEvalContext;
    let (symbols, origin, values) = make_eval_fixture();

    for (formula, input, expected) in [
        ("EXP(0)", 0.0_f64, 1.0_f64),
        ("EXP(1)", 1.0_f64, std::f64::consts::E),
        ("EXP(-1)", -1.0_f64, (-1.0_f64).exp()),
        ("EXP(709)", 709.0_f64, 709.0_f64.exp()),
        ("EXP(-1000)", -1000.0_f64, 0.0_f64),
    ] {
        let direct = exp
            .evaluate(&[FunctionArg::Scalar(CellValue::Number(input))], &ctx)
            .unwrap();
        assert_eq!(
            direct,
            CellValue::Number(expected),
            "direct mismatch for {formula}"
        );
        if let CellValue::Number(n) = direct {
            assert_eq!(
                n.to_bits(),
                expected.to_bits(),
                "bit mismatch for {formula}"
            );
        }

        let expr = Expr::parse(formula).unwrap();
        let evaluated = eval_expr(&expr, &origin, &values, &symbols, &registry).unwrap();
        assert_eq!(
            evaluated,
            CellValue::Number(expected),
            "eval mismatch for {formula}"
        );
        if let CellValue::Number(n) = evaluated {
            assert_eq!(
                n.to_bits(),
                expected.to_bits(),
                "eval bit mismatch for {formula}"
            );
        }
    }

    // Overflow -> #NUM!
    assert_eq!(
        exp.evaluate(&[FunctionArg::Scalar(CellValue::Number(710.0))], &ctx)
            .unwrap(),
        CellValue::Error(ExcelError::Num)
    );
    let overflow_expr = Expr::parse("EXP(710)").unwrap();
    assert_eq!(
        eval_expr(&overflow_expr, &origin, &values, &symbols, &registry).unwrap(),
        CellValue::Error(ExcelError::Num)
    );

    // Upstream error bubbling: EXP(NA()) -> #N/A
    assert_eq!(
        exp.evaluate(
            &[FunctionArg::Scalar(CellValue::Error(ExcelError::Na))],
            &ctx,
        )
        .unwrap(),
        CellValue::Error(ExcelError::Na)
    );
    let na_expr = Expr::parse("EXP(NA())").unwrap();
    assert_eq!(
        eval_expr(&na_expr, &origin, &values, &symbols, &registry).unwrap(),
        CellValue::Error(ExcelError::Na)
    );
}

#[test]
fn test_mod_sign_matrix_fractions_and_neg_zero_normalization() {
    let registry = FunctionRegistry::standard();
    let mod_fn = registry.lookup("MOD").unwrap();
    let ctx = MockEvalContext;
    let (symbols, origin, values) = make_eval_fixture();

    let cases = [
        ("MOD(7, 3)", 7.0, 3.0, 1.0),
        ("MOD(-7, 3)", -7.0, 3.0, 2.0),
        ("MOD(7, -3)", 7.0, -3.0, -2.0),
        ("MOD(-7, -3)", -7.0, -3.0, -1.0),
        ("MOD(6, 3)", 6.0, 3.0, 0.0),
        ("MOD(-6, 3)", -6.0, 3.0, 0.0),
        ("MOD(0, -3)", 0.0, -3.0, 0.0),
        ("MOD(5.5, 2)", 5.5, 2.0, 1.5),
        ("MOD(-5.5, 2)", -5.5, 2.0, 0.5),
        ("MOD(5.5, -2)", 5.5, -2.0, -0.5),
        ("MOD(3.25, 1)", 3.25, 1.0, 0.25),
        ("MOD(1, 0.1)", 1.0, 0.1, 0.0),
        ("MOD(10, 0.1)", 10.0, 0.1, 0.0),
    ];

    for (formula, num, div, expected) in cases {
        let direct = mod_fn
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(num)),
                    FunctionArg::Scalar(CellValue::Number(div)),
                ],
                &ctx,
            )
            .unwrap();
        assert_eq!(
            direct,
            CellValue::Number(expected),
            "direct mismatch for {formula}"
        );
        if let CellValue::Number(n) = direct {
            assert_eq!(
                n.to_bits(),
                expected.to_bits(),
                "direct bit mismatch for {formula}"
            );
        }

        let expr = Expr::parse(formula).unwrap();
        let evaluated = eval_expr(&expr, &origin, &values, &symbols, &registry).unwrap();
        assert_eq!(
            evaluated,
            CellValue::Number(expected),
            "eval mismatch for {formula}"
        );
        if let CellValue::Number(n) = evaluated {
            assert_eq!(
                n.to_bits(),
                expected.to_bits(),
                "eval bit mismatch for {formula}"
            );
        }
    }
}

#[test]
fn test_mod_error_semantics_and_extreme_float_guards() {
    let registry = FunctionRegistry::standard();
    let mod_fn = registry.lookup("MOD").unwrap();
    let ctx = MockEvalContext;
    let (symbols, origin, values) = make_eval_fixture();

    // Division by zero -> #DIV/0!
    for formula in ["MOD(5, 0)", "MOD(0, 0)"] {
        let expr = Expr::parse(formula).unwrap();
        assert_eq!(
            eval_expr(&expr, &origin, &values, &symbols, &registry).unwrap(),
            CellValue::Error(ExcelError::Div0),
            "expected #DIV/0! for '{formula}'"
        );
    }

    // Left-to-right upstream error bubbling
    for (formula, expected_err) in [
        ("MOD(NA(), 0)", ExcelError::Na),
        ("MOD(5, NA())", ExcelError::Na),
        ("MOD(NA(), 1 / 0)", ExcelError::Na),
    ] {
        let expr = Expr::parse(formula).unwrap();
        assert_eq!(
            eval_expr(&expr, &origin, &values, &symbols, &registry).unwrap(),
            CellValue::Error(expected_err),
            "error bubbling mismatch for '{formula}'"
        );
    }

    // Quotient exceeding MAX_SAFE_INTEGER or overflowing to infinity -> #NUM!
    for formula in [
        "MOD(1e20, 3)",
        "MOD(9007199254740992, 1)",
        "MOD(-9007199254740992, 1)",
        "MOD(1e308, 1e-308)",
    ] {
        let expr = Expr::parse(formula).unwrap();
        assert_eq!(
            eval_expr(&expr, &origin, &values, &symbols, &registry).unwrap(),
            CellValue::Error(ExcelError::Num),
            "expected #NUM! for '{formula}'"
        );
    }

    // Safe integer boundary, opposite-sign quotient underflow, and intermediate product overflow guards
    for (formula, num, div, expected) in [
        (
            "MOD(9007199254740991, 1)",
            9_007_199_254_740_991.0,
            1.0,
            0.0,
        ),
        ("MOD(-1e-200, 1e200)", -1e-200, 1e200, 1e200),
        ("MOD(1e-200, -1e200)", 1e-200, -1e200, -1e200),
        ("MOD(-1.5e308, 1.0e308)", -1.5e308, 1.0e308, 0.5e308),
        ("MOD(1.5e308, -1.0e308)", 1.5e308, -1.0e308, -0.5e308),
    ] {
        let direct = mod_fn
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(num)),
                    FunctionArg::Scalar(CellValue::Number(div)),
                ],
                &ctx,
            )
            .unwrap();
        assert_eq!(
            direct,
            CellValue::Number(expected),
            "direct mismatch for {formula}"
        );
        if let CellValue::Number(n) = direct {
            assert_eq!(
                n.to_bits(),
                expected.to_bits(),
                "direct bit mismatch for {formula}"
            );
        }

        let expr = Expr::parse(formula).unwrap();
        let evaluated = eval_expr(&expr, &origin, &values, &symbols, &registry).unwrap();
        assert_eq!(
            evaluated,
            CellValue::Number(expected),
            "eval mismatch for {formula}"
        );
        if let CellValue::Number(n) = evaluated {
            assert_eq!(
                n.to_bits(),
                expected.to_bits(),
                "eval bit mismatch for {formula}"
            );
        }
    }

    // Integer-rounded intermediate overflow boundaries across all four sign quadrants
    // (f64::MAX / (f64::MAX / 3.0) == 3.0, while 3.0 * (f64::MAX / 3.0) overflows to infinity)
    let max_div_3 = f64::MAX / 3.0;
    let neg_quad_expected = (-f64::MAX - max_div_3 * -2.0) + max_div_3;
    let pos_quad_expected = f64::MAX - max_div_3 * 2.0;
    assert_eq!(neg_quad_expected, 9.979_201_547_673_6e291);
    assert_eq!(pos_quad_expected, 5.992_310_449_541_052e307);

    for (label, num, div, expected) in [
        (
            "MOD(-f64::MAX, f64::MAX / 3)",
            -f64::MAX,
            max_div_3,
            neg_quad_expected,
        ),
        (
            "MOD(f64::MAX, -(f64::MAX / 3))",
            f64::MAX,
            -max_div_3,
            -neg_quad_expected,
        ),
        (
            "MOD(f64::MAX, f64::MAX / 3)",
            f64::MAX,
            max_div_3,
            pos_quad_expected,
        ),
        (
            "MOD(-f64::MAX, -(f64::MAX / 3))",
            -f64::MAX,
            -max_div_3,
            -pos_quad_expected,
        ),
    ] {
        let direct = mod_fn
            .evaluate(
                &[
                    FunctionArg::Scalar(CellValue::Number(num)),
                    FunctionArg::Scalar(CellValue::Number(div)),
                ],
                &ctx,
            )
            .unwrap();
        assert_eq!(
            direct,
            CellValue::Number(expected),
            "direct overflow boundary mismatch for {label}"
        );
        if let CellValue::Number(n) = direct {
            assert!(n.is_finite());
            assert_eq!(
                n.is_sign_negative(),
                div.is_sign_negative(),
                "divisor sign mismatch for {label}"
            );
            assert_eq!(n.to_bits(), expected.to_bits(), "bit mismatch for {label}");
        }
    }
}

#[test]
fn test_ln_exp_mod_strict_typing_arity_omission_slice_and_empty_rejection() {
    let registry = FunctionRegistry::standard();
    let ctx = MockEvalContext;
    let (symbols, origin, values) = make_eval_fixture();

    // Stage A static typing, arity, omission, and dormant-branch checks
    let stage_a_cases: &[(&str, FunctionError)] = &[
        (
            "LN(\"1\")",
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "LN",
                param: "number",
                position: 1,
                expected: ValueType::Number,
                found: ValueType::Text,
            }),
        ),
        (
            "LN(TRUE)",
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "LN",
                param: "number",
                position: 1,
                expected: ValueType::Number,
                found: ValueType::Boolean,
            }),
        ),
        (
            "LN()",
            FunctionError::TooFewArguments {
                function: "LN",
                expected: 1,
                found: 0,
            },
        ),
        (
            "LN(1, 2)",
            FunctionError::TooManyArguments {
                function: "LN",
                expected: 1,
                found: 2,
            },
        ),
        (
            "LN(_)",
            FunctionError::CannotOmitRequiredArgument {
                function: "LN",
                param: "number",
                position: 1,
            },
        ),
        (
            "EXP(\"1\")",
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "EXP",
                param: "number",
                position: 1,
                expected: ValueType::Number,
                found: ValueType::Text,
            }),
        ),
        (
            "EXP(FALSE)",
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "EXP",
                param: "number",
                position: 1,
                expected: ValueType::Number,
                found: ValueType::Boolean,
            }),
        ),
        (
            "EXP()",
            FunctionError::TooFewArguments {
                function: "EXP",
                expected: 1,
                found: 0,
            },
        ),
        (
            "EXP(1, 2)",
            FunctionError::TooManyArguments {
                function: "EXP",
                expected: 1,
                found: 2,
            },
        ),
        (
            "EXP(_)",
            FunctionError::CannotOmitRequiredArgument {
                function: "EXP",
                param: "number",
                position: 1,
            },
        ),
        (
            "MOD(\"5\", 2)",
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "MOD",
                param: "number",
                position: 1,
                expected: ValueType::Number,
                found: ValueType::Text,
            }),
        ),
        (
            "MOD(5, TRUE)",
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "MOD",
                param: "divisor",
                position: 2,
                expected: ValueType::Number,
                found: ValueType::Boolean,
            }),
        ),
        (
            "MOD(5)",
            FunctionError::TooFewArguments {
                function: "MOD",
                expected: 2,
                found: 1,
            },
        ),
        (
            "MOD(5, 2, 1)",
            FunctionError::TooManyArguments {
                function: "MOD",
                expected: 2,
                found: 3,
            },
        ),
        (
            "MOD(5, _)",
            FunctionError::CannotOmitRequiredArgument {
                function: "MOD",
                param: "divisor",
                position: 2,
            },
        ),
        (
            "MOD(_, 2)",
            FunctionError::CannotOmitRequiredArgument {
                function: "MOD",
                param: "number",
                position: 1,
            },
        ),
        (
            "IF(TRUE, 1, LN(\"1\"))",
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "LN",
                param: "number",
                position: 1,
                expected: ValueType::Number,
                found: ValueType::Text,
            }),
        ),
    ];

    for (formula, expected_err) in stage_a_cases {
        let expr = Expr::parse(formula).unwrap();
        assert_eq!(
            registry.validate_expr(&expr).unwrap_err(),
            *expected_err,
            "Stage A mismatch for '{formula}'"
        );
    }

    // Range slices rejected with SliceInScalarContext
    let resolve_slice = |_: &f8_core::expr::TableRef| -> Result<
        Option<f8_core::functions::StaticRef>,
        &'static str,
    > {
        Ok(Some(f8_core::functions::StaticRef {
            rows: 2,
            cols: 1,
            axis_role: AxisRole::Measure,
            elem_types: vec![Some(ValueType::Number), Some(ValueType::Number)],
        }))
    };
    for slice_formula in [
        "LN(T[\"r1\"..\"r2\", @col])",
        "EXP(T[\"r1\"..\"r2\", @col])",
        "MOD(T[\"r1\"..\"r2\", @col], 2)",
        "MOD(5, T[\"r1\"..\"r2\", @col])",
    ] {
        let expr = Expr::parse(slice_formula).unwrap();
        assert_eq!(
            registry
                .validate_expr_with_shapes(&expr, resolve_slice)
                .unwrap_err(),
            f8_core::functions::StageAError::Function(FunctionError::SliceInScalarContext {
                rows: 2,
                cols: 1,
            }),
            "expected SliceInScalarContext for '{slice_formula}'"
        );
    }

    // Empty cell reference passed to any LN/EXP/MOD argument fails with EmptyArgument
    for (formula, expected_fn, expected_param, expected_pos) in [
        ("LN(T[\"empty\", \"val\"])", "LN", "number", 1),
        ("EXP(T[\"empty\", \"val\"])", "EXP", "number", 1),
        ("MOD(T[\"empty\", \"val\"], 2)", "MOD", "number", 1),
        ("MOD(5, T[\"empty\", \"val\"])", "MOD", "divisor", 2),
    ] {
        let expr = Expr::parse(formula).unwrap();
        let err = eval_expr(&expr, &origin, &values, &symbols, &registry).unwrap_err();
        let EvalError::Type { cell, source } = err else {
            panic!("expected EvalError::Type for '{formula}', got {err:?}");
        };
        assert_eq!(*cell, origin, "cell mismatch for '{formula}'");
        assert_eq!(
            source,
            TypeError::EmptyArgument {
                function: expected_fn,
                param: expected_param,
                position: expected_pos,
            },
            "expected EmptyArgument for '{formula}'"
        );
    }

    // Direct evaluate arity enforcement
    let ln = registry.lookup("LN").unwrap();
    let exp = registry.lookup("EXP").unwrap();
    let mod_fn = registry.lookup("MOD").unwrap();
    let one = FunctionArg::Scalar(CellValue::Number(1.0));
    assert_eq!(
        ln.evaluate(&[], &ctx),
        Err(FunctionError::TooFewArguments {
            function: "LN",
            expected: 1,
            found: 0,
        })
    );
    assert_eq!(
        exp.evaluate(&[one.clone(), one.clone()], &ctx),
        Err(FunctionError::TooManyArguments {
            function: "EXP",
            expected: 1,
            found: 2,
        })
    );
    assert_eq!(
        mod_fn.evaluate(std::slice::from_ref(&one), &ctx),
        Err(FunctionError::TooFewArguments {
            function: "MOD",
            expected: 2,
            found: 1,
        })
    );
    assert_eq!(
        mod_fn.evaluate(&[one.clone(), one.clone(), one], &ctx),
        Err(FunctionError::TooManyArguments {
            function: "MOD",
            expected: 2,
            found: 3,
        })
    );
}
