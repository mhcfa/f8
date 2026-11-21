#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use std::cell::RefCell;

use f8_core::expr::ast::{
    CallArg, ColEndpoint, ColSelector, Expr, Literal, RowEndpoint, RowSelector, TableRef, UnaryOp,
};
use f8_core::functions::{
    BranchPolicy, EmptyPolicy, EvalContext, FunctionArg, FunctionCategory, FunctionError,
    FunctionRegistry, LookupPolicy, ParamKind, ParamRequirement, ParamType, ParameterDef,
    RangeGrid, ReferenceKind, RolePolicy, ShapePolicy, StageAError, StaticRef, ValidationPolicy,
};
use f8_core::model::table::{AxisRole, ValueLiteral};
use f8_core::value::{CellValue, ExcelError, TypeError, ValueType};

/// Mock evaluation context tracking evaluated expressions.
#[derive(Default)]
struct MockEvalContext {
    evaluated_exprs: RefCell<Vec<String>>,
    range_data: RefCell<Option<(usize, usize, Vec<CellValue>)>>,
}

impl MockEvalContext {
    fn with_range(rows: usize, cols: usize, values: Vec<CellValue>) -> Self {
        Self {
            evaluated_exprs: RefCell::new(Vec::new()),
            range_data: RefCell::new(Some((rows, cols, values))),
        }
    }

    fn recorded(&self) -> Vec<String> {
        self.evaluated_exprs.borrow().clone()
    }
}

impl EvalContext for MockEvalContext {
    fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError> {
        self.evaluated_exprs.borrow_mut().push(expr.to_string());
        match expr {
            Expr::Literal(Literal::Number(n)) => Ok(CellValue::Number(*n)),
            Expr::Literal(Literal::Boolean(b)) => Ok(CellValue::Boolean(*b)),
            Expr::Literal(Literal::String(s)) => Ok(CellValue::Text(s.clone())),
            Expr::Unary {
                op: UnaryOp::Neg,
                expr: inner,
            } => match inner.as_ref() {
                Expr::Literal(Literal::Number(n)) => Ok(CellValue::Number(-n)),
                Expr::Literal(_)
                | Expr::Reference(_)
                | Expr::Unary { .. }
                | Expr::Binary { .. }
                | Expr::FunctionCall { .. } => Ok(CellValue::Number(-100.0)),
            },
            Expr::Reference(_) | Expr::Binary { .. } | Expr::FunctionCall { .. } => {
                Ok(CellValue::Number(100.0))
            }
        }
    }

    fn eval_range(&self, table_ref: &TableRef) -> Result<RangeGrid, FunctionError> {
        self.evaluated_exprs
            .borrow_mut()
            .push(table_ref.to_string());
        if let Some((rows, cols, values)) = self.range_data.borrow().clone() {
            Ok(RangeGrid { rows, cols, values })
        } else {
            let val = self.eval_expr(&Expr::Reference(table_ref.clone()))?;
            Ok(RangeGrid {
                rows: 1,
                cols: 1,
                values: vec![val],
            })
        }
    }
}

#[test]
fn test_scenario_1_cell_value_and_excel_error_display() {
    assert_eq!(ExcelError::Div0.to_string(), "#DIV/0!");
    assert_eq!(ExcelError::Value.to_string(), "#VALUE!");
    assert_eq!(ExcelError::Ref.to_string(), "#REF!");
    assert_eq!(ExcelError::Num.to_string(), "#NUM!");
    assert_eq!(ExcelError::Na.to_string(), "#N/A");
    assert_eq!(ExcelError::Name.to_string(), "#NAME?");

    assert_eq!(CellValue::Number(42.5).to_string(), "42.5");
    assert_eq!(CellValue::Number(-0.0).to_string(), "0");
    assert_eq!(CellValue::Text("Revenue".into()).to_string(), "Revenue");
    assert_eq!(CellValue::Boolean(true).to_string(), "TRUE");
    assert_eq!(CellValue::Boolean(false).to_string(), "FALSE");
    assert_eq!(CellValue::Empty.to_string(), "");
    assert_eq!(CellValue::Error(ExcelError::Div0).to_string(), "#DIV/0!");
    assert_eq!(CellValue::Error(ExcelError::Value).to_string(), "#VALUE!");

    // From<ValueLiteral>
    assert_eq!(
        CellValue::from(ValueLiteral::Number(100.5)),
        CellValue::Number(100.5)
    );
    assert_eq!(
        CellValue::from(ValueLiteral::String("Expenses".into())),
        CellValue::Text("Expenses".into())
    );
    assert_eq!(
        CellValue::from(ValueLiteral::Boolean(false)),
        CellValue::Boolean(false)
    );
    assert_eq!(CellValue::from(ValueLiteral::Null), CellValue::Empty);
}

#[test]
fn test_scenario_2_registry_signature_arity_and_omission_validation() {
    let registry = FunctionRegistry::standard();

    // Valid calls
    assert!(
        registry
            .validate_call("ABS", &[CallArg::from(Expr::number(10.0))])
            .is_ok()
    );
    assert!(
        registry
            .validate_call(
                "SUM",
                &[
                    CallArg::from(Expr::number(1.0)),
                    CallArg::from(Expr::number(2.0)),
                ]
            )
            .is_ok()
    );
    assert!(
        registry
            .validate_call(
                "IF",
                &[
                    CallArg::from(Expr::boolean(true)),
                    CallArg::from(Expr::number(1.0)),
                    CallArg::from(Expr::number(2.0)),
                ]
            )
            .is_ok()
    );
    // IF with 2 arguments is rejected (all 3 arguments are required)
    let err = registry
        .validate_call(
            "IF",
            &[
                CallArg::from(Expr::boolean(true)),
                CallArg::from(Expr::number(1.0)),
            ],
        )
        .unwrap_err();
    assert_eq!(
        err,
        FunctionError::TooFewArguments {
            function: "IF",
            expected: 3,
            found: 2,
        }
    );

    // Unknown function
    let err = registry.validate_call("NONEXISTENT", &[]).unwrap_err();
    assert_eq!(err, FunctionError::UnknownFunction("NONEXISTENT".into()));

    // Too few arguments for ABS (expected 1, found 0)
    let err = registry.validate_call("ABS", &[]).unwrap_err();
    assert_eq!(
        err,
        FunctionError::TooFewArguments {
            function: "ABS",
            expected: 1,
            found: 0,
        }
    );

    // Too many arguments for ABS (expected 1, found 2)
    let err = registry
        .validate_call(
            "ABS",
            &[
                CallArg::from(Expr::number(1.0)),
                CallArg::from(Expr::number(2.0)),
            ],
        )
        .unwrap_err();
    assert_eq!(
        err,
        FunctionError::TooManyArguments {
            function: "ABS",
            expected: 1,
            found: 2,
        }
    );

    // Cannot omit required argument in ABS(_) with 1-indexed position 1
    let err = registry
        .validate_call("ABS", &[CallArg::Omitted])
        .unwrap_err();
    assert_eq!(
        err,
        FunctionError::CannotOmitRequiredArgument {
            function: "ABS",
            param: "number",
            position: 1,
        }
    );

    // Cannot omit required first argument in IF(_, 10, 20)
    let err = registry
        .validate_call(
            "IF",
            &[
                CallArg::Omitted,
                CallArg::from(Expr::number(10.0)),
                CallArg::from(Expr::number(20.0)),
            ],
        )
        .unwrap_err();
    assert_eq!(
        err,
        FunctionError::CannotOmitRequiredArgument {
            function: "IF",
            param: "logical_test",
            position: 1,
        }
    );

    // Scenario 1: Valid intermediate optional positional skips
    for valid_intermediate in [
        "PMT(0.05, 10, -1000, _, 1)",
        "XMATCH(\"A\", R[\"k1\"..\"k3\", @col], _, -1)",
        "XLOOKUP(\"A\", R[\"k1\"..\"k3\", @col], R[\"v1\"..\"v3\", @col], _, 0, -1)",
        "XLOOKUP(\"A\", R[\"k1\"..\"k3\", @col], R[\"v1\"..\"v3\", @col], _, _, -1)",
    ] {
        let expr = Expr::parse(valid_intermediate).unwrap();
        assert!(
            registry.validate_expr(&expr).is_ok(),
            "expected valid intermediate omission for {valid_intermediate}"
        );
    }

    // Scenario 3: Trailing `_` on optional parameter fails validation statically
    let trailing_cases = [
        (
            "INDEX(R[\"r1\"..\"r3\", @col], 2, _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "INDEX",
                param: "col_num",
                position: 3,
            },
        ),
        (
            "YEARFRAC(45292, 45474, _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "YEARFRAC",
                param: "basis",
                position: 3,
            },
        ),
        (
            "IRR(R[\"r1\"..\"r3\", @col], _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "IRR",
                param: "guess",
                position: 2,
            },
        ),
        (
            "XIRR(R[\"v1\"..\"v3\", @col], R[\"d1\"..\"d3\", @col], _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "XIRR",
                param: "guess",
                position: 3,
            },
        ),
        (
            "PMT(0.05, 10, -1000, _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "PMT",
                param: "fv",
                position: 4,
            },
        ),
        (
            "PMT(0.05, 10, -1000, _, _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "PMT",
                param: "fv",
                position: 4,
            },
        ),
        (
            "XMATCH(1, R[\"r1\"..\"r3\", @col], _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "XMATCH",
                param: "match_mode",
                position: 3,
            },
        ),
        (
            "XLOOKUP(1, R[\"k1\"..\"k3\", @col], R[\"v1\"..\"v3\", @col], _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "XLOOKUP",
                param: "if_not_found",
                position: 4,
            },
        ),
        (
            "XLOOKUP(1, R[\"k1\"..\"k3\", @col], R[\"v1\"..\"v3\", @col], _, 0, _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "XLOOKUP",
                param: "search_mode",
                position: 6,
            },
        ),
    ];
    for (formula, expected_err) in trailing_cases {
        let Expr::FunctionCall { name, args } = Expr::parse(formula).unwrap() else {
            panic!("expected function call for {formula}");
        };
        assert_eq!(
            registry.validate_call(&name, &args).unwrap_err(),
            expected_err,
            "mismatch on trailing omission formula: {formula}"
        );
    }

    // Scenario 4: Required-parameter omission takes diagnostic precedence over trailing check
    let required_precedence_cases = [
        (
            "ABS(_)",
            FunctionError::CannotOmitRequiredArgument {
                function: "ABS",
                param: "number",
                position: 1,
            },
        ),
        (
            "ROUND(1.5, _)",
            FunctionError::CannotOmitRequiredArgument {
                function: "ROUND",
                param: "num_digits",
                position: 2,
            },
        ),
        (
            "IF(FALSE, 10, _)",
            FunctionError::CannotOmitRequiredArgument {
                function: "IF",
                param: "value_if_false",
                position: 3,
            },
        ),
        (
            "IF(TRUE, 10, _)",
            FunctionError::CannotOmitRequiredArgument {
                function: "IF",
                param: "value_if_false",
                position: 3,
            },
        ),
        (
            "IF(_, 10, _)",
            FunctionError::CannotOmitRequiredArgument {
                function: "IF",
                param: "logical_test",
                position: 1,
            },
        ),
        (
            "INDEX(R[\"r1\"..\"r3\", @col], _, 2)",
            FunctionError::CannotOmitRequiredArgument {
                function: "INDEX",
                param: "row_num",
                position: 2,
            },
        ),
    ];
    for (formula, expected_err) in required_precedence_cases {
        let Expr::FunctionCall { name, args } = Expr::parse(formula).unwrap() else {
            panic!("expected function call for {formula}");
        };
        assert_eq!(
            registry.validate_call(&name, &args).unwrap_err(),
            expected_err,
            "mismatch on required omission precedence formula: {formula}"
        );
    }
}

#[test]
fn test_scenario_3_abs_scalar_evaluation() {
    let registry = FunctionRegistry::standard();
    let abs = registry.lookup("ABS").unwrap();
    let ctx = MockEvalContext::default();

    // Positive and negative numbers
    let res = abs
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(42.5))], &ctx)
        .unwrap();
    assert_eq!(res, CellValue::Number(42.5));

    let res = abs
        .evaluate(&[FunctionArg::Scalar(CellValue::Number(-42.5))], &ctx)
        .unwrap();
    assert_eq!(res, CellValue::Number(42.5));

    // Boolean and Text fail in Stage A with TypeError::InvalidArgumentType
    for (bad_formula, expected_found) in [
        ("ABS(TRUE)", ValueType::Boolean),
        ("ABS(FALSE)", ValueType::Boolean),
        ("ABS(\"invalid\")", ValueType::Text),
    ] {
        let expr = Expr::parse(bad_formula).unwrap();
        assert_eq!(
            registry.validate_expr(&expr).unwrap_err(),
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "ABS",
                param: "number",
                position: 1,
                expected: ValueType::Number,
                found: expected_found,
            })
        );
    }

    // Domain error propagates
    let res = abs
        .evaluate(
            &[FunctionArg::Scalar(CellValue::Error(ExcelError::Div0))],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Div0));
}

#[test]
fn test_scenario_4_sum_aggregation_and_range_filtering() {
    let registry = FunctionRegistry::standard();
    let sum = registry.lookup("SUM").unwrap();
    let ctx = MockEvalContext::default();

    assert_eq!(sum.parameters()[0].role_policy, RolePolicy::MeasureOnly);
    assert_eq!(sum.parameters()[0].empty_policy, EmptyPolicy::AllowInSlice);

    // Sparse numeric range slice containing numbers and Empty cells succeeds
    let range_arg = FunctionArg::range(
        2,
        2,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(25.5),
            CellValue::Empty, // Contributes 0.0 in Slice
            CellValue::Number(14.5),
        ],
        ReferenceKind::Slice,
    );

    let scalar_arg = FunctionArg::Scalar(CellValue::Number(50.0));
    let res = sum.evaluate(&[range_arg, scalar_arg], &ctx).unwrap();
    // 10.0 + 25.5 + 0.0 + 14.5 + 50.0 = 100.0
    assert_eq!(res, CellValue::Number(100.0));

    // Range reference with an error bubbles up immediately
    let err_range_arg = FunctionArg::range(
        1,
        2,
        vec![CellValue::Number(10.0), CellValue::Error(ExcelError::Ref)],
        ReferenceKind::Slice,
    );
    let res = sum.evaluate(&[err_range_arg], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Ref));

    // Descriptor range fails statically with DescriptorReferenceNotAllowed
    let sum_desc_expr = Expr::parse("SUM(T[\"r1\", \"desc\"])").unwrap();
    let mut resolve_sum_desc = |_: &TableRef| -> Result<Option<StaticRef>, &'static str> {
        Ok(Some(StaticRef {
            rows: 1,
            cols: 1,
            axis_role: AxisRole::Descriptor,
            elem_types: vec![Some(ValueType::Number)],
        }))
    };
    assert_eq!(
        registry.validate_expr_with_shapes(&sum_desc_expr, &mut resolve_sum_desc),
        Err(StageAError::Function(
            FunctionError::DescriptorReferenceNotAllowed {
                function: "SUM",
                param: "number1",
                position: 1,
            }
        ))
    );

    // Text or Boolean on SUM fail in Stage A with TypeError::InvalidArgumentType
    for (bad_formula, found_type) in [
        ("SUM(\"error\")", ValueType::Text),
        ("SUM(TRUE)", ValueType::Boolean),
    ] {
        let expr = Expr::parse(bad_formula).unwrap();
        assert_eq!(
            registry.validate_expr(&expr),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "SUM",
                param: "number1",
                position: 1,
                expected: ValueType::Number,
                found: found_type,
            }))
        );
    }
}

#[test]
fn test_scenario_5_if_short_circuiting_lazy_branches_and_required_false_branch() {
    let registry = FunctionRegistry::standard();
    let if_func = registry.lookup("IF").unwrap();

    let true_expr = Expr::number(111.0);
    let false_expr = Expr::number(222.0);

    // Case 1: IF(TRUE, 111, 222) -> only true branch evaluated
    let ctx = MockEvalContext::default();
    let args = [
        FunctionArg::Scalar(CellValue::Boolean(true)),
        FunctionArg::Lazy(&true_expr),
        FunctionArg::Lazy(&false_expr),
    ];
    let res = if_func.evaluate(&args, &ctx).unwrap();
    assert_eq!(res, CellValue::Number(111.0));
    assert_eq!(ctx.recorded(), vec!["111"]);

    // Case 2: IF(FALSE, 111, 222) -> only false branch evaluated
    let ctx = MockEvalContext::default();
    let args = [
        FunctionArg::Scalar(CellValue::Boolean(false)),
        FunctionArg::Lazy(&true_expr),
        FunctionArg::Lazy(&false_expr),
    ];
    let res = if_func.evaluate(&args, &ctx).unwrap();
    assert_eq!(res, CellValue::Number(222.0));
    assert_eq!(ctx.recorded(), vec!["222"]);

    // Case 3: IF(FALSE, 111, _) -> rejected with CannotOmitRequiredArgument in validate_expr
    assert_eq!(
        registry
            .validate_expr(&Expr::parse("IF(FALSE, 111, _)").unwrap())
            .unwrap_err(),
        FunctionError::CannotOmitRequiredArgument {
            function: "IF",
            param: "value_if_false",
            position: 3,
        }
    );

    // Case 4: IF(FALSE, 111) with only 2 arguments -> rejected with TooFewArguments
    let ctx = MockEvalContext::default();
    let args = [
        FunctionArg::Scalar(CellValue::Boolean(false)),
        FunctionArg::Lazy(&true_expr),
    ];
    let err_2arg = if_func.evaluate(&args, &ctx).unwrap_err();
    assert_eq!(
        err_2arg,
        FunctionError::TooFewArguments {
            function: "IF",
            expected: 3,
            found: 2,
        }
    );
    assert!(ctx.recorded().is_empty());

    // Condition error bubbles up without evaluating branches
    let ctx = MockEvalContext::default();
    let args = [
        FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
        FunctionArg::Lazy(&true_expr),
        FunctionArg::Lazy(&false_expr),
    ];
    let res = if_func.evaluate(&args, &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Div0));
    assert!(ctx.recorded().is_empty());
}

#[test]
fn test_bind_arguments_integration() {
    let registry = FunctionRegistry::standard();
    let if_func = registry.lookup("IF").unwrap();

    let table_ref = Expr::Reference(TableRef::new(
        "IncomeStatement",
        RowSelector::Range("r1".into(), "r3".into()),
        ColSelector::Active,
    ));

    // Scenario 2: 3-arg IF binds lazy arms 1-to-1
    let ctx = MockEvalContext::default();
    let true_val_expr = Expr::number(42.0);
    let false_val_expr = Expr::number(99.0);
    let exprs = [
        CallArg::from(Expr::boolean(false)),
        CallArg::from(true_val_expr.clone()),
        CallArg::from(false_val_expr.clone()),
    ];
    let bound = registry.bind_arguments(if_func, &exprs, &ctx).unwrap();

    assert_eq!(bound.len(), 3);
    assert_eq!(bound[0], FunctionArg::Scalar(CellValue::Boolean(false)));
    assert_eq!(bound[1], FunctionArg::Lazy(&true_val_expr));
    assert_eq!(bound[2], FunctionArg::Lazy(&false_val_expr));
    assert_eq!(
        if_func.evaluate(&bound, &ctx).unwrap(),
        CellValue::Number(99.0)
    );

    // Scenario 2: Binding 2-argument INDEX on a 1x3 row vector preserves bound.len() == 2
    let index_func = registry.lookup("INDEX").unwrap();
    let ctx_row_vec = MockEvalContext::with_range(
        1,
        3,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ],
    );
    let index_exprs = [
        CallArg::from(table_ref.clone()),
        CallArg::from(Expr::number(2.0)),
    ];
    let bound_index = registry
        .bind_arguments(index_func, &index_exprs, &ctx_row_vec)
        .unwrap();
    assert_eq!(bound_index.len(), 2);
    assert_eq!(
        index_func.evaluate(&bound_index, &ctx_row_vec).unwrap(),
        CellValue::Number(20.0)
    );

    // Scenario 1: Intermediate optional skips bind Scalar(default) or FunctionArg::Omitted 1-to-1
    let pmt_func = registry.lookup("PMT").unwrap();
    let Expr::FunctionCall { args: pmt_args, .. } =
        Expr::parse("PMT(0.05, 10, -1000, _, 1)").unwrap()
    else {
        panic!("expected PMT call");
    };
    let bound_pmt = registry.bind_arguments(pmt_func, &pmt_args, &ctx).unwrap();
    assert_eq!(bound_pmt.len(), 5);
    assert_eq!(bound_pmt[3], FunctionArg::Scalar(CellValue::Number(0.0)));
    assert_eq!(bound_pmt[4], FunctionArg::Scalar(CellValue::Number(1.0)));

    let xlookup_func = registry.lookup("XLOOKUP").unwrap();
    let Expr::FunctionCall {
        args: xlookup_args, ..
    } = Expr::parse("XLOOKUP(\"A\", R[\"k1\"..\"k3\", @col], R[\"v1\"..\"v3\", @col], _, _, -1)")
        .unwrap()
    else {
        panic!("expected XLOOKUP call");
    };
    let bound_xlookup = registry
        .bind_arguments(xlookup_func, &xlookup_args, &ctx_row_vec)
        .unwrap();
    assert_eq!(bound_xlookup.len(), 6);
    assert_eq!(bound_xlookup[3], FunctionArg::Omitted);
    assert_eq!(
        bound_xlookup[4],
        FunctionArg::Scalar(CellValue::Number(0.0))
    );
    assert_eq!(
        bound_xlookup[5],
        FunctionArg::Scalar(CellValue::Number(-1.0))
    );

    // Binding SUM with a range reference expression and scalar expression
    let sum_func = registry.lookup("SUM").unwrap();
    let ctx_with_range = MockEvalContext::with_range(
        3,
        1,
        vec![
            CellValue::Number(10.0),
            CellValue::Number(20.0),
            CellValue::Number(30.0),
        ],
    );

    let sum_exprs = [CallArg::from(table_ref), CallArg::from(Expr::number(40.0))];
    let bound_sum = registry
        .bind_arguments(sum_func, &sum_exprs, &ctx_with_range)
        .unwrap();

    assert_eq!(bound_sum.len(), 2);
    assert_eq!(
        bound_sum[0],
        FunctionArg::range(
            3,
            1,
            vec![
                CellValue::Number(10.0),
                CellValue::Number(20.0),
                CellValue::Number(30.0)
            ],
            ReferenceKind::Slice,
        )
    );
    assert_eq!(bound_sum[1], FunctionArg::Scalar(CellValue::Number(40.0)));

    let result = sum_func.evaluate(&bound_sum, &ctx_with_range).unwrap();
    assert_eq!(result, CellValue::Number(100.0));
}

#[test]
fn test_registry_metadata_and_iteration() {
    let registry = FunctionRegistry::standard();
    assert_eq!(registry.len(), 37);
    assert!(!registry.is_empty());

    let names: Vec<&str> = registry.iter().map(|(name, _)| name).collect();
    assert_eq!(
        names,
        vec![
            "ABS",
            "LN",
            "EXP",
            "MOD",
            "SUM",
            "AVERAGE",
            "COUNT",
            "MAX",
            "MIN",
            "MEDIAN",
            "ROUND",
            "ROUNDUP",
            "ROUNDDOWN",
            "SUMPRODUCT",
            "IF",
            "IFERROR",
            "AND",
            "OR",
            "NOT",
            "CHOOSE",
            "NPV",
            "XNPV",
            "IRR",
            "XIRR",
            "PMT",
            "XLOOKUP",
            "XMATCH",
            "INDEX",
            "DATE",
            "EDATE",
            "EOMONTH",
            "YEARFRAC",
            "YEAR",
            "MONTH",
            "DAY",
            "ISERROR",
            "NA",
        ]
    );

    // Case-insensitive lookup and metadata for LN, EXP, and MOD
    for (canonical, aliases, expected_params) in [
        ("LN", ["LN", "ln", "Ln"], vec!["number"]),
        ("EXP", ["EXP", "exp", "Exp"], vec!["number"]),
        ("MOD", ["MOD", "mod", "Mod"], vec!["number", "divisor"]),
    ] {
        for alias in aliases {
            let func = registry
                .lookup(alias)
                .unwrap_or_else(|| panic!("missing function {alias}"));
            assert_eq!(func.name(), canonical);
            assert_eq!(func.category(), FunctionCategory::Math);
            assert_eq!(func.min_args(), expected_params.len());
            assert_eq!(func.max_args(), Some(expected_params.len()));
            assert_eq!(func.return_type(), Some(ValueType::Number));
            assert_eq!(func.validation_policy(), ValidationPolicy::Standard);
            let param_names: Vec<&str> = func.parameters().iter().map(|p| p.name).collect();
            assert_eq!(param_names, expected_params);
            for param in func.parameters() {
                assert_eq!(param.kind, ParamKind::Scalar);
                assert_eq!(param.param_type, ParamType::Number);
                assert_eq!(param.requirement, ParamRequirement::Required);
            }
            assert!(registry.get(alias).is_some());
        }
    }

    let if_func = registry.lookup("if").unwrap();
    assert_eq!(if_func.name(), "IF");
    assert_eq!(if_func.category(), FunctionCategory::Logical);
    assert_eq!(if_func.min_args(), 3);
    assert_eq!(if_func.max_args(), Some(3));

    assert!(registry.lookup("IF").is_some());
    assert!(registry.lookup("If").is_some());
    for retired in [
        "COUNTA",
        "counta",
        "IFS",
        "ifs",
        "IFNA",
        "ifna",
        "DAYS",
        "days",
        "ISBLANK",
        "isblank",
        "ISNA",
        "isna",
        "ERROR.TYPE",
        "error.type",
        "SWITCH",
        "switch",
        "SUMIFS",
        "COUNTIFS",
        "AVERAGEIFS",
        "MINIFS",
        "MAXIFS",
        "IPMT",
        "ipmt",
        "PPMT",
        "ppmt",
        "UNKNOWN_FN",
        "unknown_fn",
    ] {
        assert!(
            registry.lookup(retired).is_none(),
            "{retired} lookup must be None"
        );
        assert!(
            registry.get(retired).is_none(),
            "{retired} get must be None"
        );
    }
    assert!(registry.get("SUM").is_some());
    assert!(registry.get("sum").is_some());

    // Exhaustive ValidationPolicy matrix across all 37 standard functions
    let mut non_standard_count = 0usize;
    for (name, func) in registry.iter() {
        let expected_policy = match name {
            "IF" => ValidationPolicy::Branch(BranchPolicy::If),
            "CHOOSE" => ValidationPolicy::Branch(BranchPolicy::Choose),
            "IFERROR" => ValidationPolicy::Branch(BranchPolicy::ErrorTrap),
            "INDEX" => ValidationPolicy::Lookup(LookupPolicy::Index),
            "XLOOKUP" => ValidationPolicy::Lookup(LookupPolicy::Xlookup),
            "XMATCH" => ValidationPolicy::Lookup(LookupPolicy::Xmatch),
            "SUMPRODUCT" => ValidationPolicy::ShapeCoupled(ShapePolicy::EqualAllRanges),
            "XNPV" => ValidationPolicy::ShapeCoupled(ShapePolicy::OneDimEqualLength {
                first_idx: 1,
                first_param: "values",
                second_idx: 2,
                second_param: "dates",
            }),
            "XIRR" => ValidationPolicy::ShapeCoupled(ShapePolicy::OneDimEqualLength {
                first_idx: 0,
                first_param: "values",
                second_idx: 1,
                second_param: "dates",
            }),
            _ => ValidationPolicy::Standard,
        };
        if expected_policy != ValidationPolicy::Standard {
            non_standard_count += 1;
        }
        assert_eq!(
            func.validation_policy(),
            expected_policy,
            "unexpected ValidationPolicy for {name}"
        );
    }
    assert_eq!(non_standard_count, 9);
}

#[test]
fn test_validate_expr_preorder_and_nested_ast() {
    let registry = FunctionRegistry::standard();

    // Valid nested expression with omitted optional slots passes
    let valid_expr = Expr::parse(
        "IF(TRUE, SUM(1, ABS(-2)), XLOOKUP(1, T[\"k1\"..\"k2\", @col], T[\"v1\"..\"v2\", @col], _, 0, 1))",
    )
    .unwrap();
    assert!(registry.validate_expr(&valid_expr).is_ok());

    // Dormant lazy branch signatures and unknown functions
    let cases = [
        (
            "IF(TRUE, 10, UNKNOWN_FN())",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        (
            "IF(FALSE, SUM(), 10)",
            FunctionError::TooFewArguments {
                function: "SUM",
                expected: 1,
                found: 0,
            },
        ),
        (
            "IF(TRUE, 10, ABS(1, 2))",
            FunctionError::TooManyArguments {
                function: "ABS",
                expected: 1,
                found: 2,
            },
        ),
        (
            "IF(TRUE, 10, IFS(TRUE, 1, FALSE, 2))",
            FunctionError::UnknownFunction("IFS".into()),
        ),
        (
            "IF(TRUE, 10, ABS(_))",
            FunctionError::CannotOmitRequiredArgument {
                function: "ABS",
                param: "number",
                position: 1,
            },
        ),
        (
            "IF(TRUE, 10, IF(FALSE, 1, _))",
            FunctionError::CannotOmitRequiredArgument {
                function: "IF",
                param: "value_if_false",
                position: 3,
            },
        ),
        (
            "IFS(TRUE, 1, FALSE, 2)",
            FunctionError::UnknownFunction("IFS".into()),
        ),
        (
            "IFERROR(10, UNKNOWN_FN())",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        ("IFNA(10, 0)", FunctionError::UnknownFunction("IFNA".into())),
        (
            "COUNTA(T[\"k1\"..\"k2\", @col])",
            FunctionError::UnknownFunction("COUNTA".into()),
        ),
        (
            "DAYS(45474, 45292)",
            FunctionError::UnknownFunction("DAYS".into()),
        ),
        (
            "ISBLANK(T[\"k1\", @col])",
            FunctionError::UnknownFunction("ISBLANK".into()),
        ),
        (
            "ISNA(T[\"k1\", @col])",
            FunctionError::UnknownFunction("ISNA".into()),
        ),
        (
            "CHOOSE(1, 10, SUM())",
            FunctionError::TooFewArguments {
                function: "SUM",
                expected: 1,
                found: 0,
            },
        ),
        (
            "SWITCH(1, 1, 10, 20)",
            FunctionError::UnknownFunction("SWITCH".into()),
        ),
        (
            "IF(FALSE, SWITCH(1, 1, 10, 20), 0)",
            FunctionError::UnknownFunction("SWITCH".into()),
        ),
        (
            "SUMIFS(T[\"v1\"..\"v2\", @col], T[\"k1\"..\"k2\", @col], 1)",
            FunctionError::UnknownFunction("SUMIFS".into()),
        ),
        (
            "COUNTIFS(T[\"k1\"..\"k2\", @col], 1)",
            FunctionError::UnknownFunction("COUNTIFS".into()),
        ),
        (
            "AVERAGEIFS(T[\"v1\"..\"v2\", @col], T[\"k1\"..\"k2\", @col], 1)",
            FunctionError::UnknownFunction("AVERAGEIFS".into()),
        ),
        (
            "MINIFS(T[\"v1\"..\"v2\", @col], T[\"k1\"..\"k2\", @col], 1)",
            FunctionError::UnknownFunction("MINIFS".into()),
        ),
        (
            "MAXIFS(T[\"v1\"..\"v2\", @col], T[\"k1\"..\"k2\", @col], 1)",
            FunctionError::UnknownFunction("MAXIFS".into()),
        ),
        (
            "IPMT(0.05, 1, 10, -1000)",
            FunctionError::UnknownFunction("IPMT".into()),
        ),
        (
            "PPMT(0.05, 1, 10, -1000)",
            FunctionError::UnknownFunction("PPMT".into()),
        ),
        (
            "XLOOKUP(1, R1[\"k\", @col], R2[\"v\", @col], UNKNOWN_FN())",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        (
            "IF((1 / 0) > 0, UNKNOWN_FN(), 10)",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        // Nested unary/binary inside dormant branch
        (
            "IF(TRUE, 10, -(1 + UNKNOWN_FN()))",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        // Pre-order precedence: outer validated before inner
        (
            "UNKNOWN_OUTER(UNKNOWN_INNER())",
            FunctionError::UnknownFunction("UNKNOWN_OUTER".into()),
        ),
        (
            "ABS(UNKNOWN_INNER(), 2)",
            FunctionError::TooManyArguments {
                function: "ABS",
                expected: 1,
                found: 2,
            },
        ),
        // Left-to-right argument traversal
        (
            "IF(UNKNOWN_FIRST(), UNKNOWN_SECOND(), 0)",
            FunctionError::UnknownFunction("UNKNOWN_FIRST".into()),
        ),
    ];

    for (formula, expected_err) in cases {
        let expr = Expr::parse(formula).unwrap();
        assert_eq!(
            registry.validate_expr(&expr).unwrap_err(),
            expected_err,
            "mismatch on formula: {formula}"
        );
    }
}

#[test]
fn test_bind_arguments_uses_resolved_function_without_duplicate_lookup() {
    let standard = FunctionRegistry::standard();
    let abs_func = standard.lookup("ABS").unwrap();
    let empty_registry = FunctionRegistry::new();
    let ctx = MockEvalContext::default();

    // Empty registry does not contain ABS, proving bind_arguments binds against `abs_func` directly
    let call_args = [CallArg::from(Expr::number(-5.0))];
    let bound = empty_registry
        .bind_arguments(abs_func, &call_args, &ctx)
        .unwrap();
    assert_eq!(bound, vec![FunctionArg::Scalar(CellValue::Number(-5.0))]);
}

#[test]
fn test_direct_evaluate_and_omission_invariants() {
    let req_required = ParameterDef::required("x", "required x", ParamKind::Scalar, ParamType::Any);
    assert_eq!(req_required.requirement, ParamRequirement::Required);
    assert!(!req_required.requirement.is_optional());
    assert_eq!(req_required.requirement.default_value(), None);

    let req_optional = ParameterDef::optional("y", "optional y", ParamKind::Lazy, ParamType::Any);
    assert_eq!(req_optional.requirement, ParamRequirement::Optional);
    assert!(req_optional.requirement.is_optional());
    assert_eq!(req_optional.requirement.default_value(), None);

    let req_default = ParameterDef::with_default(
        "z",
        "optional z with default",
        ParamKind::Scalar,
        ParamType::Number,
        CellValue::Number(0.0),
    );
    assert_eq!(
        req_default.requirement,
        ParamRequirement::OptionalWithDefault(CellValue::Number(0.0))
    );
    assert!(req_default.requirement.is_optional());
    assert_eq!(
        req_default.requirement.default_value(),
        Some(&CellValue::Number(0.0))
    );

    let registry = FunctionRegistry::standard();
    let ctx = MockEvalContext::default();
    let pmt_fn = registry.lookup("PMT").unwrap();

    // Direct intermediate optional FunctionArg::Omitted calls succeed using parameter defaults
    let pmt_omitted_fv = pmt_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(0.05)),
                FunctionArg::Scalar(CellValue::Number(10.0)),
                FunctionArg::Scalar(CellValue::Number(-1000.0)),
                FunctionArg::Omitted,
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    let pmt_explicit_fv = pmt_fn
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(0.05)),
                FunctionArg::Scalar(CellValue::Number(10.0)),
                FunctionArg::Scalar(CellValue::Number(-1000.0)),
                FunctionArg::Scalar(CellValue::Number(0.0)),
                FunctionArg::Scalar(CellValue::Number(1.0)),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(pmt_omitted_fv, pmt_explicit_fv);

    // eval_scalar rejects FunctionArg::Omitted
    assert_eq!(
        FunctionArg::Omitted.eval_scalar("IF", "value_if_true", 2, &ctx),
        Err(FunctionError::InvalidArgumentKind {
            function: "IF",
            expected: "scalar",
            found: "omitted",
        })
    );
}

#[test]
fn test_stage_a_static_scalar_slice_and_shape_strictness() {
    let registry = FunctionRegistry::standard();

    // Scenario 2: Static AST scalar, logical, and relational strictness across active and dormant branches
    for bad_formula in [
        "\"5\" + 1",
        "TRUE * 10",
        "-FALSE",
        "ABS(\"invalid\")",
        "ROUND(TRUE, 2)",
        "IF(1, 10, 20)",
        "IF(\"TRUE\", 10, 20)",
        "NA(1)",
        "NOT(0)",
        "AND(1, TRUE)",
        "100 < \"1\"",
        "\"zzz\" < FALSE",
        "1 = \"1\"",
        "IF(TRUE, 10, \"5\" + 1)",
        "IFERROR(\"5\" + 1, 0)",
    ] {
        let expr = Expr::parse(bad_formula).unwrap();
        assert!(
            registry.validate_expr(&expr).is_err(),
            "expected Stage A error for '{bad_formula}'"
        );
    }

    // Valid expressions pass Stage A (including numeric literals whose domain bounds are evaluated in Pass 4b)
    for ok_formula in [
        "\"str\"",
        "ROUND(10, 2.5)",
        "ROUND(10, 100000000000000000000)",
        "INDEX(T[\"r1\"..\"r4\", @col], 1.5)",
        "DATE(2024, 1.5, 1)",
        "PMT(0.05, 10, -1000, 0, 0.5)",
        "INDEX(T[\"r1\"..\"r4\", @col], XMATCH(\"Beta\", T[\"r1\"..\"r4\", @col]))",
        "DATE(YEAR(T[\"r1\", @col]) + 1, MONTH(T[\"r1\", @col]), DAY(T[\"r1\", @col]))",
        "ISERROR(T[\"r1\", @col])",
        "NA()",
    ] {
        let expr = Expr::parse(ok_formula).unwrap();
        assert!(
            registry.validate_expr(&expr).is_ok(),
            "expected Stage A ok for '{ok_formula}'"
        );
    }

    // Scenario 4: Shape-resolving Stage A validation (including 1x1 syntactic slices, ParamKind::Range, rank, and constant bounds)
    let resolve_shape = |tr: &TableRef| -> Result<Option<StaticRef>, &'static str> {
        let rows = match &tr.row {
            RowSelector::Range(s, e) => match (s, e) {
                (RowEndpoint::Key(sk), RowEndpoint::Key(ek)) => match (sk.as_str(), ek.as_str()) {
                    ("r1", "r1") => 1,
                    ("r1", "r2") => 2,
                    ("r2", "r4") => 3,
                    ("r1", "r4") => 4,
                    _ => 2,
                },
                (RowEndpoint::Active | RowEndpoint::Offset(_), _)
                | (_, RowEndpoint::Active | RowEndpoint::Offset(_)) => 1,
            },
            RowSelector::Active | RowSelector::Key(_) | RowSelector::Offset(_) => 1,
        };
        let cols = match &tr.col {
            ColSelector::Range(s, e) => match (s, e) {
                (ColEndpoint::Key(sk), ColEndpoint::Key(ek)) => match (sk.as_str(), ek.as_str()) {
                    ("c1", "c4") => 4,
                    _ => 2,
                },
                (ColEndpoint::Active | ColEndpoint::Offset(_), _)
                | (_, ColEndpoint::Active | ColEndpoint::Offset(_)) => 1,
            },
            ColSelector::Active | ColSelector::Key(_) | ColSelector::Offset(_) => 1,
        };
        let axis_role = match &tr.col {
            ColSelector::Key(k) if k.as_str() == "line_item" => AxisRole::Descriptor,
            ColSelector::Active
            | ColSelector::Key(_)
            | ColSelector::Offset(_)
            | ColSelector::Range(_, _) => AxisRole::Measure,
        };
        Ok(Some(StaticRef {
            rows,
            cols,
            axis_role,
            elem_types: vec![],
        }))
    };

    for slice_in_scalar in [
        "T[\"r1\"..\"r2\", @col]",
        "T[\"r1\"..\"r1\", @col]",
        "1 + T[\"r1\"..\"r1\", @col]",
        "1 + T[\"r1\"..\"r2\", @col]",
        "ABS(T[\"r1\"..\"r2\", @col])",
        "LN(T[\"r1\"..\"r2\", @col])",
        "EXP(T[\"r1\"..\"r2\", @col])",
        "MOD(T[\"r1\"..\"r2\", @col], 2)",
        "MOD(5, T[\"r1\"..\"r2\", @col])",
        "ISERROR(T[\"r1\"..\"r2\", @col])",
        "ISERROR(T[\"r1\"..\"r1\", @col])",
        "IF(TRUE, 1, T[\"r1\"..\"r2\", @col])",
        "IF(T[\"r1\"..\"r2\", @col], 1, 2)",
    ] {
        let expr = Expr::parse(slice_in_scalar).unwrap();
        let err = registry
            .validate_expr_with_shapes(&expr, resolve_shape)
            .unwrap_err();
        assert!(
            matches!(
                err,
                StageAError::Function(FunctionError::SliceInScalarContext { .. })
            ),
            "expected SliceInScalarContext for '{slice_in_scalar}', got {err:?}"
        );
    }

    for non_ref_range in [
        "SUMPRODUCT(\"abc\")",
        "COUNT(10)",
        "XMATCH(10, 10)",
        "XLOOKUP(10, 10, 20)",
        "INDEX(10, 1)",
        "IRR(100)",
        "XNPV(0.1, 100, 45292)",
    ] {
        let expr = Expr::parse(non_ref_range).unwrap();
        let err = registry
            .validate_expr_with_shapes(&expr, resolve_shape)
            .unwrap_err();
        assert!(
            matches!(
                err,
                StageAError::Function(FunctionError::InvalidArgumentKind { .. })
            ),
            "expected InvalidArgumentKind for '{non_ref_range}', got {err:?}"
        );
    }

    for shape_or_bounds in [
        "SUMPRODUCT(T[\"r1\"..\"r4\", @col], T[\"r1\"..\"r2\", @col])",
        "XLOOKUP(1, T[\"r1\"..\"r4\", @col], T[\"r2\"..\"r4\", @col])",
        "XLOOKUP(1, T[\"r1\"..\"r4\", @col], T[\"r1\", \"c1\"..\"c4\"])",
        "XNPV(0.1, T[\"r1\"..\"r4\", @col], T[\"r1\"..\"r2\", @col])",
        "XIRR(T[\"r2\"..\"r4\", @col], T[\"r1\"..\"r4\", @col])",
        "XMATCH(1, T[\"r1\"..\"r2\", \"c1\"..\"c2\"])",
        "XLOOKUP(1, T[\"r1\"..\"r2\", \"c1\"..\"c2\"], T[\"r1\"..\"r2\", \"c1\"..\"c2\"])",
        "XNPV(-2.0, T[\"r1\"..\"r2\", \"c1\"..\"c2\"], T[\"r1\"..\"r2\", \"c1\"..\"c2\"])",
        "XIRR(T[\"r1\"..\"r2\", \"c1\"..\"c2\"], T[\"r1\"..\"r2\", \"c1\"..\"c2\"])",
        "INDEX(T[\"r1\"..\"r4\", \"c1\"..\"c2\"], 2)",
    ] {
        let expr = Expr::parse(shape_or_bounds).unwrap();
        let err = registry
            .validate_expr_with_shapes(&expr, resolve_shape)
            .unwrap_err();
        assert!(
            matches!(
                err,
                StageAError::Function(
                    FunctionError::DimensionMismatch { .. }
                        | FunctionError::LengthMismatch { .. }
                        | FunctionError::ExpectedOneDimensionalSlice { .. }
                        | FunctionError::MissingColumnIndexFor2DSlice { .. }
                )
            ),
            "expected static shape/rank error for '{shape_or_bounds}', got {err:?}"
        );
    }

    // Valid 1x1 single-cell and 1x1 slice in Range/ScalarOrRange, 2-arg horizontal INDEX, and numeric indices (checked at runtime) succeed in Stage A
    for valid_shape_case in [
        "COUNT(T[\"r1\", @col])",
        "SUM(T[\"r1\"..\"r1\", @col])",
        "INDEX(T[\"r1\", \"c1\"..\"c4\"], 3)",
        "INDEX(T[\"r1\"..\"r4\", @col], 5)",
        "INDEX(T[\"r1\", \"c1\"..\"c4\"], 5)",
        "INDEX(T[\"r1\"..\"r4\", @col], 0)",
        "INDEX(T[\"r1\"..\"r4\", @col], -1)",
        "CHOOSE(5, 100, 200, 300)",
        "CHOOSE(0, 100)",
        "IFERROR(INDEX(T[\"r1\"..\"r4\", @col], 5), 0)",
        "IF(TRUE, 1, CHOOSE(0, 100))",
    ] {
        let expr = Expr::parse(valid_shape_case).unwrap();
        assert!(
            registry
                .validate_expr_with_shapes(&expr, resolve_shape)
                .is_ok(),
            "expected shape validation ok for '{valid_shape_case}'"
        );
    }
}

#[test]
fn test_cluster_a2_stage_a_monomorphic_branches_and_lookup_homogeneity() {
    let registry = FunctionRegistry::standard();

    let stage_a_cases: &[(&str, FunctionError)] = &[
        // 3-arg IF required
        (
            "IF(TRUE, 10)",
            FunctionError::TooFewArguments {
                function: "IF",
                expected: 3,
                found: 2,
            },
        ),
        // Monomorphic branch result unification across IF, CHOOSE, IFERROR
        (
            "IF(T[\"ebitda\", @col] > 0, T[\"ev\", @col] / T[\"ebitda\", @col], \"NM\")",
            FunctionError::Type(TypeError::IncompatibleBranchTypes {
                function: "IF",
                position: 3,
                expected: ValueType::Number,
                found: ValueType::Text,
            }),
        ),
        (
            "IFERROR(10 / 0, \"N/A\")",
            FunctionError::Type(TypeError::IncompatibleBranchTypes {
                function: "IFERROR",
                position: 2,
                expected: ValueType::Number,
                found: ValueType::Text,
            }),
        ),
        (
            "CHOOSE(1, 10, \"NM\")",
            FunctionError::Type(TypeError::IncompatibleBranchTypes {
                function: "CHOOSE",
                position: 3,
                expected: ValueType::Number,
                found: ValueType::Text,
            }),
        ),
        // Relational comparisons, parameter type mismatches, and XLOOKUP if_not_found
        (
            "IF(FALSE, IF(TRUE, \"a\", \"b\") = IF(FALSE, 1, 2), TRUE)",
            FunctionError::Type(TypeError::IncompatibleComparison {
                operator: "=",
                left: ValueType::Text,
                right: ValueType::Number,
            }),
        ),
        (
            "ROUND(10, IF(TRUE, \"a\", \"b\"))",
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "ROUND",
                param: "num_digits",
                position: 2,
                expected: ValueType::Number,
                found: ValueType::Text,
            }),
        ),
        (
            "ROUND(10, IF(TRUE, 2, \"bad\"))",
            FunctionError::Type(TypeError::IncompatibleBranchTypes {
                function: "IF",
                position: 3,
                expected: ValueType::Number,
                found: ValueType::Text,
            }),
        ),
        (
            "1 + XLOOKUP(\"Beta\", T[\"r1\"..\"r4\", \"cat\"], T[\"r1\"..\"r4\", \"num\"], \"NM\")",
            FunctionError::Type(TypeError::InvalidOperandType {
                operator: "+",
                expected: ValueType::Number,
                found: ValueType::Text,
            }),
        ),
        (
            "XMATCH(\"Beta\", T[\"r1\"..\"r4\", \"cat\"], \"abc\")",
            FunctionError::Type(TypeError::InvalidArgumentType {
                function: "XMATCH",
                param: "match_mode",
                position: 3,
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
            "mismatch on Cluster A2 Stage A formula: {formula}"
        );
    }

    // Valid monomorphic branches, NA() error bottom type, and numeric arguments succeed in Stage A
    for valid_formula in [
        "IF(T[\"c\", @col], \"PASS\", \"FAIL\")",
        "IF(T[\"ebitda\", @col] > 0, T[\"ev\", @col] / T[\"ebitda\", @col], NA())",
        "IF(T[\"ebitda\", @col] > 0, NA(), T[\"ev\", @col] / T[\"ebitda\", @col])",
        "XMATCH(30, T[\"r1\"..\"r4\", \"num\"], -1, 2)",
        "XLOOKUP(25, T[\"r1\"..\"r4\", \"num\"], T[\"r1\"..\"r4\", \"cat\"], \"Miss\", 1, -2)",
        "ROUND(10, -IF(TRUE, 2, 3))",
        "ROUND(10, IF(TRUE, 2, 2.5))",
        "ROUND(10, --2)",
        "ROUND(10, 2.5)",
        "ROUND(10, --2.5)",
        "XMATCH(\"Beta\", T[\"r1\"..\"r4\", \"cat\"], 1.5)",
        "XMATCH(\"Beta\", T[\"r1\"..\"r4\", \"cat\"], T[\"mode\", @col])",
        "XMATCH(\"Beta\", T[\"r1\"..\"r4\", \"cat\"], 0 + 0)",
        "XMATCH(\"Beta\", T[\"r1\"..\"r4\", \"cat\"], 2)",
    ] {
        let expr = Expr::parse(valid_formula).unwrap();
        assert!(
            registry.validate_expr(&expr).is_ok(),
            "expected Stage A ok on '{valid_formula}'"
        );
    }
}

#[test]
fn test_cluster_b_stage_a_descriptor_roles_and_date_annuity_domains() {
    let registry = FunctionRegistry::standard();
    let resolve_shape = |tr: &TableRef| -> Result<Option<StaticRef>, &'static str> {
        let rows = match &tr.row {
            RowSelector::Range(s, e) => match (s, e) {
                (RowEndpoint::Key(sk), RowEndpoint::Key(ek)) => match (sk.as_str(), ek.as_str()) {
                    ("r1", "r2") => 2,
                    ("r1", "r4") => 4,
                    _ => 1,
                },
                (RowEndpoint::Active | RowEndpoint::Offset(_), _)
                | (_, RowEndpoint::Active | RowEndpoint::Offset(_)) => 1,
            },
            RowSelector::Active | RowSelector::Key(_) | RowSelector::Offset(_) => 1,
        };
        let cols = match &tr.col {
            ColSelector::Range(_, _) => 2,
            ColSelector::Active | ColSelector::Key(_) | ColSelector::Offset(_) => 1,
        };
        let axis_role = match &tr.col {
            ColSelector::Key(k) if k.as_str() == "line_item" => AxisRole::Descriptor,
            ColSelector::Active
            | ColSelector::Key(_)
            | ColSelector::Offset(_)
            | ColSelector::Range(_, _) => AxisRole::Measure,
        };
        Ok(Some(StaticRef {
            rows,
            cols,
            axis_role,
            elem_types: vec![],
        }))
    };

    // Scenario 1: Descriptor references rejected in numeric and financial aggregations (active & dormant)
    for (formula, fn_name, param, position) in [
        ("SUM(T[\"r1\"..\"r4\", \"line_item\"])", "SUM", "number1", 1),
        ("SUM(T[\"r1\", \"line_item\"])", "SUM", "number1", 1),
        (
            "AVERAGE(T[\"r1\"..\"r4\", \"line_item\"])",
            "AVERAGE",
            "number1",
            1,
        ),
        ("MIN(T[\"r1\"..\"r4\", \"line_item\"])", "MIN", "number1", 1),
        ("MAX(T[\"r1\"..\"r4\", \"line_item\"])", "MAX", "number1", 1),
        (
            "MEDIAN(T[\"r1\"..\"r4\", \"line_item\"])",
            "MEDIAN",
            "number1",
            1,
        ),
        (
            "COUNT(T[\"r1\"..\"r4\", \"line_item\"])",
            "COUNT",
            "value1",
            1,
        ),
        (
            "SUMPRODUCT(T[\"r1\"..\"r4\", \"line_item\"], T[\"r1\"..\"r4\", \"amount\"])",
            "SUMPRODUCT",
            "array1",
            1,
        ),
        (
            "SUMPRODUCT(T[\"r1\"..\"r4\", \"amount\"], T[\"r1\"..\"r4\", \"line_item\"])",
            "SUMPRODUCT",
            "array1",
            2,
        ),
        (
            "NPV(0.1, T[\"r1\"..\"r4\", \"line_item\"])",
            "NPV",
            "value1",
            2,
        ),
        ("IRR(T[\"r1\"..\"r4\", \"line_item\"])", "IRR", "values", 1),
        (
            "XNPV(0.1, T[\"r1\"..\"r4\", \"line_item\"], T[\"r1\"..\"r4\", \"date\"])",
            "XNPV",
            "values",
            2,
        ),
        (
            "XIRR(T[\"r1\"..\"r4\", \"amount\"], T[\"r1\"..\"r4\", \"line_item\"])",
            "XIRR",
            "dates",
            2,
        ),
        (
            "IF(FALSE, SUM(T[\"r1\"..\"r4\", \"line_item\"]), 0)",
            "SUM",
            "number1",
            1,
        ),
    ] {
        let expr = Expr::parse(formula).unwrap();
        assert_eq!(
            registry
                .validate_expr_with_shapes(&expr, resolve_shape)
                .unwrap_err(),
            StageAError::Function(FunctionError::DescriptorReferenceNotAllowed {
                function: fn_name,
                param,
                position,
            }),
            "expected DescriptorReferenceNotAllowed on {formula}"
        );
    }

    // Precedence: ShapePolicy runs before RolePolicy on ShapeCoupled functions
    let shape_first =
        Expr::parse("XNPV(0.1, T[\"r1\"..\"r4\", \"line_item\"], T[\"r1\"..\"r2\", \"date\"])")
            .unwrap();
    assert!(matches!(
        registry
            .validate_expr_with_shapes(&shape_first, resolve_shape)
            .unwrap_err(),
        StageAError::Function(FunctionError::LengthMismatch { .. })
    ));

    // Descriptor references remain permitted on XLOOKUP, XMATCH, and INDEX
    for allowed_descriptor in [
        "XLOOKUP(\"Rev\", T[\"r1\"..\"r4\", \"line_item\"], T[\"r1\"..\"r4\", \"amount\"])",
        "XMATCH(\"Rev\", T[\"r1\"..\"r4\", \"line_item\"])",
        "INDEX(T[\"r1\"..\"r4\", \"line_item\"], 1)",
    ] {
        let expr = Expr::parse(allowed_descriptor).unwrap();
        assert!(
            registry
                .validate_expr_with_shapes(&expr, resolve_shape)
                .is_ok(),
            "expected Descriptor ok on {allowed_descriptor}"
        );
    }

    // Scenario 4 & 5: Non-numeric arguments fail Stage A, while numeric domain values pass Stage A and evaluate at runtime
    let expr = Expr::parse("DATE(\"2024\", 1, 1)").unwrap();
    assert_eq!(
        registry.validate_expr(&expr).unwrap_err(),
        FunctionError::Type(TypeError::InvalidArgumentType {
            function: "DATE",
            param: "year",
            position: 1,
            expected: ValueType::Number,
            found: ValueType::Text,
        })
    );

    // Valid calendar dates, nested expressions, and numeric literals pass Stage A
    for valid_formula in [
        "DATE(1900, 1, 1)",
        "DATE(1900, 2, 29)",
        "DATE(2024, 2, 29)",
        "DATE(T[\"y\", @col], 2, 29)",
        "DATE(9999, 12, 31)",
        "DATE(24, 12, 31)",
        "DATE(2024.5, 1, 1)",
        "YEAR(DATE(2024, 1, 15))",
        "YEAR(0)",
        "YEAR(1)",
        "YEAR(60)",
        "YEAR(45292.5)",
        "YEAR(2958465.9)",
        "PMT(0.05, 10, -1000, 0, 0)",
        "PMT(0.05, 10, -1000, 0, 1)",
        "PMT(0.05, 10, -1000, 0, 2)",
        "PMT(0.05, 10, -1000, 0, T[\"type\", @col])",
        "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 0)",
        "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 4)",
        "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 5)",
        "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), T[\"basis\", @col])",
    ] {
        let expr = Expr::parse(valid_formula).unwrap();
        assert!(
            registry.validate_expr(&expr).is_ok(),
            "expected Stage A ok on '{valid_formula}'"
        );
    }
}

#[test]
fn test_error_introspection_metadata_and_validation() {
    let registry = FunctionRegistry::standard();
    let ctx = MockEvalContext::default();

    // Error introspection metadata and evaluation
    let iserror = registry.lookup("ISERROR").unwrap();
    assert_eq!(iserror.name(), "ISERROR");
    assert_eq!(iserror.category(), FunctionCategory::Information);
    assert_eq!(iserror.min_args(), 1);
    assert_eq!(iserror.max_args(), Some(1));
    assert_eq!(iserror.return_type(), Some(ValueType::Boolean));
    assert_eq!(iserror.parameters()[0].kind, ParamKind::Scalar);
    assert_eq!(iserror.parameters()[0].param_type, ParamType::Any);
    assert_eq!(iserror.parameters()[0].empty_policy, EmptyPolicy::Forbid);

    let na = registry.lookup("NA").unwrap();
    assert_eq!(na.name(), "NA");
    assert_eq!(na.category(), FunctionCategory::Information);
    assert_eq!(na.min_args(), 0);
    assert_eq!(na.max_args(), Some(0));
    assert_eq!(na.return_type(), None);

    assert!(registry.lookup("ISBLANK").is_none());
    assert!(registry.lookup("ISNA").is_none());
    assert!(registry.lookup("ERROR.TYPE").is_none());

    for err in [
        ExcelError::Div0,
        ExcelError::Value,
        ExcelError::Ref,
        ExcelError::Name,
        ExcelError::Num,
        ExcelError::Na,
    ] {
        let scalar_err = FunctionArg::Scalar(CellValue::Error(err));
        assert_eq!(
            iserror.evaluate(std::slice::from_ref(&scalar_err), &ctx),
            Ok(CellValue::Boolean(true))
        );
    }

    for non_err in [
        CellValue::Number(42.0),
        CellValue::Text("#DIV/0!".into()),
        CellValue::Boolean(true),
    ] {
        let arg = FunctionArg::Scalar(non_err);
        assert_eq!(
            iserror.evaluate(std::slice::from_ref(&arg), &ctx),
            Ok(CellValue::Boolean(false))
        );
    }

    // ISERROR rejects Empty argument with TypeError::EmptyArgument
    assert_eq!(
        iserror.evaluate(&[FunctionArg::Scalar(CellValue::Empty)], &ctx),
        Err(FunctionError::Type(TypeError::EmptyArgument {
            function: "ISERROR",
            param: "value",
            position: 1,
        }))
    );

    // SliceInScalarContext on ISERROR
    let expr = Expr::parse("ISERROR(T[\"r1\"..\"r2\", @col])").unwrap();
    assert!(matches!(
        registry.validate_expr(&expr),
        Err(FunctionError::SliceInScalarContext { .. })
    ));

    // Compile-time type errors inside ISERROR are not masked
    let bad_iserror = Expr::parse("ISERROR(\"5\" + 1)").unwrap();
    assert!(matches!(
        registry.validate_expr(&bad_iserror),
        Err(FunctionError::Type(_))
    ));
}

#[test]
fn test_scalar_measure_only_role_policy_and_iferror_param_kind() {
    use f8_core::functions::ExcelFunction;
    use std::sync::Arc;

    #[derive(Debug)]
    struct ScalarMeasureFn;

    static SCALAR_MEASURE_PARAMS: &[ParameterDef] = &[ParameterDef::required(
        "metric",
        "A measure-only scalar parameter.",
        ParamKind::Scalar,
        ParamType::Number,
    )
    .measure_only()];

    impl ExcelFunction for ScalarMeasureFn {
        fn name(&self) -> &'static str {
            "SCALAR_MEASURE"
        }
        fn category(&self) -> FunctionCategory {
            FunctionCategory::Math
        }
        fn parameters(&self) -> &'static [ParameterDef] {
            SCALAR_MEASURE_PARAMS
        }
        fn return_type(&self) -> Option<ValueType> {
            Some(ValueType::Number)
        }
        fn evaluate(
            &self,
            args: &[FunctionArg<'_>],
            _ctx: &dyn EvalContext,
        ) -> Result<CellValue, FunctionError> {
            match &args[0] {
                FunctionArg::Scalar(val) => Ok(val.clone()),
                FunctionArg::Range { .. } | FunctionArg::Lazy(_) | FunctionArg::Omitted => {
                    unreachable!()
                }
            }
        }
    }

    let mut registry = FunctionRegistry::standard();
    registry.register(Arc::new(ScalarMeasureFn));

    // 1. Single-cell Descriptor reference on ParamKind::Scalar with .measure_only() fails with DescriptorReferenceNotAllowed
    let single_ref_expr = Expr::parse("SCALAR_MEASURE(T[\"r1\", \"desc\"])").unwrap();
    let res_desc = registry.validate_expr_with_shapes(&single_ref_expr, |_| {
        Ok::<_, std::convert::Infallible>(Some(StaticRef {
            rows: 1,
            cols: 1,
            axis_role: AxisRole::Descriptor,
            elem_types: vec![Some(ValueType::Number)],
        }))
    });
    assert_eq!(
        res_desc,
        Err(StageAError::Function(
            FunctionError::DescriptorReferenceNotAllowed {
                function: "SCALAR_MEASURE",
                param: "metric",
                position: 1,
            }
        ))
    );

    // 2. Slice reference on ParamKind::Scalar with .measure_only() still fails with SliceInScalarContext first
    let slice_ref_expr = Expr::parse("SCALAR_MEASURE(T[\"r1\"..\"r2\", \"desc\"])").unwrap();
    let res_slice = registry.validate_expr_with_shapes(&slice_ref_expr, |_| {
        Ok::<_, std::convert::Infallible>(Some(StaticRef {
            rows: 2,
            cols: 1,
            axis_role: AxisRole::Descriptor,
            elem_types: vec![Some(ValueType::Number), Some(ValueType::Number)],
        }))
    });
    assert_eq!(
        res_slice,
        Err(StageAError::Function(FunctionError::SliceInScalarContext {
            rows: 2,
            cols: 1,
        }))
    );

    // 3. Measure single-cell reference passes and resolves only once
    let mut resolve_calls = 0usize;
    let res_measure = registry.validate_expr_with_shapes(&single_ref_expr, |_| {
        resolve_calls += 1;
        Ok::<_, std::convert::Infallible>(Some(StaticRef {
            rows: 1,
            cols: 1,
            axis_role: AxisRole::Measure,
            elem_types: vec![Some(ValueType::Number)],
        }))
    });
    assert_eq!(res_measure, Ok(()));
    assert_eq!(resolve_calls, 1);

    // 4. IFERROR declares arg 0 (`value`) as ParamKind::Scalar and arg 1 (`value_if_error`) as ParamKind::Lazy
    let func = registry.lookup("IFERROR").unwrap();
    assert_eq!(func.parameters()[0].kind, ParamKind::Scalar);
    assert_eq!(func.parameters()[1].kind, ParamKind::Lazy);

    let ctx = MockEvalContext::default();
    let fallback_expr = Expr::number(99.0);
    let call_args = [
        CallArg::from(Expr::number(10.0)),
        CallArg::from(fallback_expr.clone()),
    ];
    let bound = registry.bind_arguments(func, &call_args, &ctx).unwrap();
    assert_eq!(
        bound,
        vec![
            FunctionArg::Scalar(CellValue::Number(10.0)),
            FunctionArg::Lazy(&fallback_expr),
        ]
    );
    assert_eq!(ctx.recorded(), vec!["10"]);
}

#[test]
fn test_standard_function_registry_shares_cached_arc_instances() {
    let r1 = FunctionRegistry::standard();
    let r2 = FunctionRegistry::standard();
    let sum1 = r1.get("SUM").unwrap();
    let sum2 = r2.get("SUM").unwrap();
    assert!(
        std::sync::Arc::ptr_eq(&sum1, &sum2),
        "FunctionRegistry::standard() must share LazyLock-cached Arc<dyn ExcelFunction> instances"
    );
}

#[test]
fn test_bind_arguments_and_fixed_arity_evaluate_guards() {
    let registry = FunctionRegistry::standard();
    let abs_fn = registry.lookup("ABS").unwrap();
    let na_fn = registry.lookup("NA").unwrap();
    let pmt_fn = registry.lookup("PMT").unwrap();
    let ctx = MockEvalContext::default();

    // 1. bind_arguments enforces arity and omission rules directly
    assert_eq!(
        registry.bind_arguments(abs_fn, &[], &ctx),
        Err(FunctionError::TooFewArguments {
            function: "ABS",
            expected: 1,
            found: 0,
        })
    );
    assert_eq!(
        registry.bind_arguments(
            abs_fn,
            &[
                CallArg::from(Expr::number(1.0)),
                CallArg::from(Expr::number(2.0))
            ],
            &ctx
        ),
        Err(FunctionError::TooManyArguments {
            function: "ABS",
            expected: 1,
            found: 2,
        })
    );
    assert_eq!(
        registry.bind_arguments(na_fn, &[CallArg::from(Expr::number(1.0))], &ctx),
        Err(FunctionError::TooManyArguments {
            function: "NA",
            expected: 0,
            found: 1,
        })
    );
    assert_eq!(
        registry.bind_arguments(
            pmt_fn,
            &[
                CallArg::from(Expr::number(0.05)),
                CallArg::from(Expr::number(10.0)),
                CallArg::from(Expr::number(-1000.0)),
                CallArg::Omitted,
            ],
            &ctx
        ),
        Err(FunctionError::CannotOmitTrailingArgument {
            function: "PMT",
            param: "fv",
            position: 4,
        })
    );

    // 2. bind_arguments rejects slice reference on ParamKind::Scalar
    let slice_arg = [CallArg::from(
        Expr::parse("T[\"r1\"..\"r2\", @col]").unwrap(),
    )];
    assert!(matches!(
        registry.bind_arguments(abs_fn, &slice_arg, &ctx),
        Err(FunctionError::SliceInScalarContext { .. })
    ));

    // 3. Direct fixed-arity .evaluate() rejects excess trailing arguments
    assert_eq!(
        abs_fn.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(1.0)),
                FunctionArg::Scalar(CellValue::Number(2.0)),
            ],
            &ctx
        ),
        Err(FunctionError::TooManyArguments {
            function: "ABS",
            expected: 1,
            found: 2,
        })
    );
    assert_eq!(
        na_fn.evaluate(&[FunctionArg::Scalar(CellValue::Number(1.0))], &ctx),
        Err(FunctionError::TooManyArguments {
            function: "NA",
            expected: 0,
            found: 1,
        })
    );
}
