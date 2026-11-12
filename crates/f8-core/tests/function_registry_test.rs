#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use std::cell::RefCell;

use f8_core::expr::ast::{CallArg, ColSelector, Expr, Literal, RowSelector, TableRef};
use f8_core::functions::{
    EvalContext, FunctionArg, FunctionCategory, FunctionError, FunctionRegistry,
};
use f8_core::model::table::ValueLiteral;
use f8_core::value::{CellValue, ExcelError};

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
            Expr::Reference(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::FunctionCall { .. } => Ok(CellValue::Number(100.0)),
        }
    }

    fn eval_range(&self, expr: &Expr) -> Result<(usize, usize, Vec<CellValue>), FunctionError> {
        self.evaluated_exprs.borrow_mut().push(expr.to_string());
        if let Some(range) = self.range_data.borrow().clone() {
            Ok(range)
        } else {
            let val = self.eval_expr(expr)?;
            Ok((1, 1, vec![val]))
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
    // IF with 2 arguments is valid (3rd argument is optional)
    assert!(
        registry
            .validate_call(
                "IF",
                &[
                    CallArg::from(Expr::boolean(true)),
                    CallArg::from(Expr::number(1.0)),
                ]
            )
            .is_ok()
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

    // Omitting optional 3rd argument in IF(TRUE, 10, _) is valid
    assert!(
        registry
            .validate_call(
                "IF",
                &[
                    CallArg::from(Expr::boolean(true)),
                    CallArg::from(Expr::number(10.0)),
                    CallArg::Omitted,
                ]
            )
            .is_ok()
    );
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

    // Empty cell coerces to 0.0
    let res = abs
        .evaluate(&[FunctionArg::Scalar(CellValue::Empty)], &ctx)
        .unwrap();
    assert_eq!(res, CellValue::Number(0.0));

    // Booleans coerce to 1.0 / 0.0
    let res = abs
        .evaluate(&[FunctionArg::Scalar(CellValue::Boolean(true))], &ctx)
        .unwrap();
    assert_eq!(res, CellValue::Number(1.0));

    let res = abs
        .evaluate(&[FunctionArg::Scalar(CellValue::Boolean(false))], &ctx)
        .unwrap();
    assert_eq!(res, CellValue::Number(0.0));

    // Domain error propagates
    let res = abs
        .evaluate(
            &[FunctionArg::Scalar(CellValue::Error(ExcelError::Div0))],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Div0));

    // Text evaluates to #VALUE!
    let res = abs
        .evaluate(
            &[FunctionArg::Scalar(CellValue::Text("invalid".into()))],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Value));
}

#[test]
fn test_scenario_4_sum_aggregation_and_range_filtering() {
    let registry = FunctionRegistry::standard();
    let sum = registry.lookup("SUM").unwrap();
    let ctx = MockEvalContext::default();

    // Range slice containing numbers, text, booleans, and empty cells
    let range_arg = FunctionArg::Array {
        rows: 2,
        cols: 3,
        values: vec![
            CellValue::Number(10.0),
            CellValue::Text("N/A text".into()), // Ignored in range
            CellValue::Boolean(true),           // Ignored in range
            CellValue::Number(25.5),
            CellValue::Empty, // Contributes 0.0
            CellValue::Number(14.5),
        ],
        is_range_ref: true,
    };

    let scalar_arg = FunctionArg::Scalar(CellValue::Number(50.0));
    let empty_arg = FunctionArg::Scalar(CellValue::Empty);

    let res = sum
        .evaluate(&[range_arg, scalar_arg, empty_arg], &ctx)
        .unwrap();
    // 10.0 + 25.5 + 14.5 + 50.0 + 0.0 = 100.0
    assert_eq!(res, CellValue::Number(100.0));

    // Range reference with an error bubbles up immediately
    let err_range_arg = FunctionArg::Array {
        rows: 1,
        cols: 2,
        values: vec![CellValue::Number(10.0), CellValue::Error(ExcelError::Ref)],
        is_range_ref: true,
    };
    let res = sum.evaluate(&[err_range_arg], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Ref));

    // Scalar text argument in SUM produces #VALUE!
    let scalar_text_arg = FunctionArg::Scalar(CellValue::Text("error".into()));
    let res = sum.evaluate(&[scalar_text_arg], &ctx).unwrap();
    assert_eq!(res, CellValue::Error(ExcelError::Value));

    // Scalar boolean argument coerces to 1.0
    let scalar_bool_arg = FunctionArg::Scalar(CellValue::Boolean(true));
    let res = sum.evaluate(&[scalar_bool_arg], &ctx).unwrap();
    assert_eq!(res, CellValue::Number(1.0));
}

#[test]
fn test_scenario_5_if_short_circuiting_lazy_branches_and_omitted_false() {
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

    // Case 3: IF(FALSE, 111, _) -> returns FALSE without executing true branch
    let ctx = MockEvalContext::default();
    let args = [
        FunctionArg::Scalar(CellValue::Boolean(false)),
        FunctionArg::Lazy(&true_expr),
        FunctionArg::Omitted,
    ];
    let res = if_func.evaluate(&args, &ctx).unwrap();
    assert_eq!(res, CellValue::Boolean(false));
    assert!(ctx.recorded().is_empty());

    // Case 4: IF(FALSE, 111) with only 2 arguments -> returns FALSE
    let ctx = MockEvalContext::default();
    let args = [
        FunctionArg::Scalar(CellValue::Boolean(false)),
        FunctionArg::Lazy(&true_expr),
    ];
    let res = if_func.evaluate(&args, &ctx).unwrap();
    assert_eq!(res, CellValue::Boolean(false));
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

    // Binding IF with 2 arguments: [condition, true_expr]
    // 3rd argument is missing, so it should be bound to the default CellValue::Boolean(false)
    let ctx = MockEvalContext::default();
    let true_val_expr = Expr::number(42.0);
    let exprs = [
        CallArg::from(Expr::boolean(true)),
        CallArg::from(true_val_expr.clone()),
    ];
    let bound = registry.bind_arguments(if_func, &exprs, &ctx).unwrap();

    assert_eq!(bound.len(), 3);
    assert_eq!(bound[0], FunctionArg::Scalar(CellValue::Boolean(true)));
    assert_eq!(bound[1], FunctionArg::Lazy(&true_val_expr));
    assert_eq!(bound[2], FunctionArg::Scalar(CellValue::Boolean(false)));

    // Binding IF with explicit omitted 3rd argument: [condition, true_expr, _]
    let exprs_omitted = [
        CallArg::from(Expr::boolean(false)),
        CallArg::from(Expr::number(42.0)),
        CallArg::Omitted,
    ];
    let bound_omitted = registry
        .bind_arguments(if_func, &exprs_omitted, &ctx)
        .unwrap();

    assert_eq!(bound_omitted.len(), 3);
    assert_eq!(
        bound_omitted[2],
        FunctionArg::Scalar(CellValue::Boolean(false))
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
        FunctionArg::Array {
            rows: 3,
            cols: 1,
            values: vec![
                CellValue::Number(10.0),
                CellValue::Number(20.0),
                CellValue::Number(30.0)
            ],
            is_range_ref: true,
        }
    );
    assert_eq!(bound_sum[1], FunctionArg::Scalar(CellValue::Number(40.0)));

    let result = sum_func.evaluate(&bound_sum, &ctx_with_range).unwrap();
    assert_eq!(result, CellValue::Number(100.0));
}

#[test]
fn test_registry_metadata_and_iteration() {
    let registry = FunctionRegistry::standard();
    assert_eq!(registry.len(), 44);
    assert!(!registry.is_empty());

    let names: Vec<&str> = registry.iter().map(|(name, _)| name).collect();
    assert_eq!(
        names,
        vec![
            "ABS",
            "SUM",
            "AVERAGE",
            "COUNT",
            "COUNTA",
            "MAX",
            "MIN",
            "MEDIAN",
            "ROUND",
            "ROUNDUP",
            "ROUNDDOWN",
            "SUMPRODUCT",
            "SUMIFS",
            "COUNTIFS",
            "AVERAGEIFS",
            "MINIFS",
            "MAXIFS",
            "IF",
            "IFS",
            "IFERROR",
            "IFNA",
            "AND",
            "OR",
            "NOT",
            "CHOOSE",
            "SWITCH",
            "NPV",
            "XNPV",
            "IRR",
            "XIRR",
            "PMT",
            "IPMT",
            "PPMT",
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
            "DAYS"
        ]
    );

    let if_func = registry.lookup("if").unwrap();
    assert_eq!(if_func.name(), "IF");
    assert_eq!(if_func.category(), FunctionCategory::Logical);
    assert_eq!(if_func.min_args(), 2);
    assert_eq!(if_func.max_args(), Some(3));

    assert!(registry.lookup("IF").is_some());
    assert!(registry.lookup("If").is_some());
    assert!(registry.lookup("UNKNOWN_FN").is_none());
    assert!(registry.lookup("unknown_fn").is_none());
    assert!(registry.get("SUM").is_some());
    assert!(registry.get("sum").is_some());
    assert!(registry.get("UNKNOWN_FN").is_none());
    assert!(registry.get("unknown_fn").is_none());
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
            "IF(TRUE, 10, IFS(TRUE, 1, FALSE))",
            FunctionError::InvalidVariadicPairs {
                function: "IFS",
                count: 3,
            },
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
            "IFS(TRUE, 1, FALSE, UNKNOWN_FN())",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        (
            "IFERROR(10, UNKNOWN_FN())",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        (
            "IFNA(10, UNKNOWN_FN())",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
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
            "SWITCH(1, 1, 10, ABS())",
            FunctionError::TooFewArguments {
                function: "ABS",
                expected: 1,
                found: 0,
            },
        ),
        (
            "XLOOKUP(1, R1[\"k\", @col], R2[\"v\", @col], UNKNOWN_FN())",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        (
            "IF(1 / 0, UNKNOWN_FN(), 10)",
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
            "IF(UNKNOWN_FIRST(), UNKNOWN_SECOND())",
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

    // Empty registry does not contain ABS, proving bind_arguments validates against `abs_func` directly
    let call_args = [CallArg::from(Expr::number(-5.0))];
    let bound = empty_registry
        .bind_arguments(abs_func, &call_args, &ctx)
        .unwrap();
    assert_eq!(bound, vec![FunctionArg::Scalar(CellValue::Number(-5.0))]);

    let err = empty_registry
        .bind_arguments(abs_func, &[], &ctx)
        .unwrap_err();
    assert_eq!(
        err,
        FunctionError::TooFewArguments {
            function: "ABS",
            expected: 1,
            found: 0,
        }
    );
}
