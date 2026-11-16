#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use indexmap::IndexMap;

use f8_core::compiler::eval::{EvalError, eval_expr};
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::ir::symbol::SymbolTable;
use f8_core::expr::ast::{CallArg, Expr};
use f8_core::functions::traits::{EvalContext, FunctionArg, ReferenceKind};
use f8_core::functions::{FunctionCategory, FunctionError, FunctionRegistry};
use f8_core::model::id::{SheetId, TableId};
use f8_core::model::table::{ColumnSpec, RowSpec, TableSpec};
use f8_core::value::{CellValue, ExcelError, TypeError, ValueType};

/// Mock evaluation context for unit testing function evaluate directly.
struct MockContext {
    eval_count: std::cell::Cell<usize>,
}

impl MockContext {
    fn new() -> Self {
        Self {
            eval_count: std::cell::Cell::new(0),
        }
    }

    fn count(&self) -> usize {
        self.eval_count.get()
    }
}

impl EvalContext for MockContext {
    fn eval_expr(&self, expr: &Expr) -> Result<CellValue, FunctionError> {
        self.eval_count.set(self.eval_count.get() + 1);
        match expr {
            Expr::Literal(f8_core::expr::ast::Literal::Number(n)) => Ok(CellValue::Number(*n)),
            Expr::Literal(f8_core::expr::ast::Literal::String(s)) => Ok(CellValue::Text(s.clone())),
            Expr::Literal(f8_core::expr::ast::Literal::Boolean(b)) => Ok(CellValue::Boolean(*b)),
            Expr::Reference(_)
            | Expr::Unary { .. }
            | Expr::Binary { .. }
            | Expr::FunctionCall { .. } => Ok(CellValue::Number(999.0)),
        }
    }
}

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

#[test]
fn test_scenario_1_registration_and_metadata() {
    let registry = FunctionRegistry::standard();
    assert_eq!(registry.len(), 34);
    for retired in ["SWITCH", "switch", "IFS", "ifs", "IFNA", "ifna"] {
        assert!(registry.lookup(retired).is_none());
    }

    let logical_funcs = ["IF", "IFERROR", "AND", "OR", "NOT", "CHOOSE"];

    for name in logical_funcs {
        let func = registry
            .lookup(name)
            .unwrap_or_else(|| panic!("Missing {name}"));
        assert_eq!(func.name(), name);
        assert_eq!(func.category(), FunctionCategory::Logical);
    }
}

#[test]
fn test_scenario_2_and_or_range_and_scalar_semantics() {
    let registry = FunctionRegistry::standard();
    let and_func = registry.lookup("AND").unwrap();
    let or_func = registry.lookup("OR").unwrap();
    let ctx = MockContext::new();

    // Pure boolean scalars: all true
    let args_true = vec![
        FunctionArg::Scalar(CellValue::Boolean(true)),
        FunctionArg::Scalar(CellValue::Boolean(true)),
    ];
    assert_eq!(
        and_func.evaluate(&args_true, &ctx).unwrap(),
        CellValue::Boolean(true)
    );
    assert_eq!(
        or_func.evaluate(&args_true, &ctx).unwrap(),
        CellValue::Boolean(true)
    );

    // Pure boolean scalars: mixed
    let args_mixed = vec![
        FunctionArg::Scalar(CellValue::Boolean(true)),
        FunctionArg::Scalar(CellValue::Boolean(false)),
    ];
    assert_eq!(
        and_func.evaluate(&args_mixed, &ctx).unwrap(),
        CellValue::Boolean(false)
    );
    assert_eq!(
        or_func.evaluate(&args_mixed, &ctx).unwrap(),
        CellValue::Boolean(true)
    );

    // Non-boolean scalars (Number, Text) fail in Pass 4a with InvalidArgumentType
    for (formula, expected_found) in [
        ("AND(1 = 0, 1)", ValueType::Number),
        ("AND(1 = 0, \"true\")", ValueType::Text),
    ] {
        assert_eq!(
            registry.validate_expr(&Expr::parse(formula).unwrap()),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "AND",
                param: "logical1",
                position: 2,
                expected: ValueType::Boolean,
                found: expected_found,
            }))
        );
    }

    // Syntactic Slice ignores Empty cells when Boolean cells are present
    let args_sparse_slice = vec![FunctionArg::range(
        2,
        1,
        vec![CellValue::Empty, CellValue::Boolean(true)],
        ReferenceKind::Slice,
    )];
    assert_eq!(
        and_func.evaluate(&args_sparse_slice, &ctx).unwrap(),
        CellValue::Boolean(true)
    );
    assert_eq!(
        or_func.evaluate(&args_sparse_slice, &ctx).unwrap(),
        CellValue::Boolean(true)
    );

    // Syntactic Slice with all Empty cells returns #VALUE!
    let args_all_empty_slice = vec![FunctionArg::range(
        2,
        1,
        vec![CellValue::Empty, CellValue::Empty],
        ReferenceKind::Slice,
    )];
    assert_eq!(
        and_func.evaluate(&args_all_empty_slice, &ctx).unwrap(),
        CellValue::Error(ExcelError::Value)
    );
    assert_eq!(
        or_func.evaluate(&args_all_empty_slice, &ctx).unwrap(),
        CellValue::Error(ExcelError::Value)
    );

    // Error in valid boolean slice bubbles immediately
    let args_err_range = vec![FunctionArg::range(
        2,
        1,
        vec![CellValue::Boolean(true), CellValue::Error(ExcelError::Div0)],
        ReferenceKind::Slice,
    )];
    assert_eq!(
        and_func.evaluate(&args_err_range, &ctx).unwrap(),
        CellValue::Error(ExcelError::Div0)
    );
    assert_eq!(
        or_func.evaluate(&args_err_range, &ctx).unwrap(),
        CellValue::Error(ExcelError::Div0)
    );

    // LogicalCollector::collect aggregates validated boolean arguments cleanly
    let col = f8_core::functions::common::LogicalCollector::collect(&[
        FunctionArg::Scalar(CellValue::Boolean(true)),
        FunctionArg::Scalar(CellValue::Boolean(false)),
    ])
    .unwrap();
    assert_eq!((col.count, col.all_true, col.any_true), (2, false, true));
}

#[test]
fn test_scenario_3_not_scalar_inversion() {
    let registry = FunctionRegistry::standard();
    let not_func = registry.lookup("NOT").unwrap();
    let ctx = MockContext::new();

    assert_eq!(
        not_func
            .evaluate(&[FunctionArg::Scalar(CellValue::Boolean(true))], &ctx)
            .unwrap(),
        CellValue::Boolean(false)
    );
    assert_eq!(
        not_func
            .evaluate(&[FunctionArg::Scalar(CellValue::Boolean(false))], &ctx)
            .unwrap(),
        CellValue::Boolean(true)
    );
    assert_eq!(
        not_func
            .evaluate(
                &[FunctionArg::Scalar(CellValue::Error(ExcelError::Ref))],
                &ctx
            )
            .unwrap(),
        CellValue::Error(ExcelError::Ref)
    );

    for (formula, expected_found) in [
        ("NOT(0)", ValueType::Number),
        ("NOT(10)", ValueType::Number),
        ("NOT(\"TRUE\")", ValueType::Text),
        ("NOT(\"false\")", ValueType::Text),
    ] {
        assert_eq!(
            registry.validate_expr(&Expr::parse(formula).unwrap()),
            Err(FunctionError::Type(TypeError::InvalidArgumentType {
                function: "NOT",
                param: "logical",
                position: 1,
                expected: ValueType::Boolean,
                found: expected_found,
            }))
        );
    }
}

#[test]
fn test_scenario_4_iferror_lazy_evaluation_and_retired_ifna() {
    let registry = FunctionRegistry::standard();
    let iferror_func = registry.lookup("IFERROR").unwrap();
    assert!(registry.lookup("IFNA").is_none());
    let ctx = MockContext::new();

    let dummy_expr = Expr::Literal(f8_core::expr::ast::Literal::Number(42.0));

    // IFERROR: valid value does NOT evaluate fallback
    let res = iferror_func
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(10.0)),
                FunctionArg::Lazy(&dummy_expr),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res, CellValue::Number(10.0));
    assert_eq!(ctx.count(), 0); // fallback expression was NOT evaluated

    // IFERROR: error value evaluates fallback
    let res_err = iferror_func
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
                FunctionArg::Lazy(&dummy_expr),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res_err, CellValue::Number(42.0));
    assert_eq!(ctx.count(), 1); // fallback expression WAS evaluated
}

#[test]
fn test_scenario_5_if_short_circuiting_and_retired_ifs() {
    let registry = FunctionRegistry::standard();
    let if_func = registry.lookup("IF").unwrap();
    assert!(registry.lookup("IFS").is_none());
    let ctx = MockContext::new();

    let expr_res1 = Expr::Literal(f8_core::expr::ast::Literal::String("first".into()));
    let expr_res2 = Expr::Literal(f8_core::expr::ast::Literal::String("second".into()));

    // IF(FALSE, "first", "second") evaluates only "second"
    let args = vec![
        FunctionArg::Scalar(CellValue::Boolean(false)),
        FunctionArg::Lazy(&expr_res1),
        FunctionArg::Lazy(&expr_res2),
    ];
    let res = if_func.evaluate(&args, &ctx).unwrap();
    assert_eq!(res, CellValue::Text("second".into()));
    assert_eq!(ctx.count(), 1);

    // IFS is rejected as UnknownFunction
    assert_eq!(
        registry.validate_expr(&Expr::parse("IFS(TRUE, \"first\", FALSE, \"second\")").unwrap()),
        Err(FunctionError::UnknownFunction("IFS".into()))
    );
}

#[test]
fn test_scenario_6_choose_index_selection() {
    let registry = FunctionRegistry::standard();
    let choose_func = registry.lookup("CHOOSE").unwrap();
    let ctx = MockContext::new();

    let choice1 = Expr::Literal(f8_core::expr::ast::Literal::String("A".into()));
    let choice2 = Expr::Literal(f8_core::expr::ast::Literal::String("B".into()));
    let choice3 = Expr::Literal(f8_core::expr::ast::Literal::String("C".into()));

    // Index 2 selects "B"
    let args = vec![
        FunctionArg::Scalar(CellValue::Number(2.0)),
        FunctionArg::Lazy(&choice1),
        FunctionArg::Lazy(&choice2),
        FunctionArg::Lazy(&choice3),
    ];
    let res = choose_func.evaluate(&args, &ctx).unwrap();
    assert_eq!(res, CellValue::Text("B".into()));

    // Floating index 2.9 returns #NUM!
    let args_non_int = vec![
        FunctionArg::Scalar(CellValue::Number(2.9)),
        FunctionArg::Lazy(&choice1),
        FunctionArg::Lazy(&choice2),
        FunctionArg::Lazy(&choice3),
    ];
    assert_eq!(
        choose_func.evaluate(&args_non_int, &ctx),
        Ok(CellValue::Error(ExcelError::Num))
    );

    // Index 0 or negative returns #VALUE!
    let args_zero = vec![
        FunctionArg::Scalar(CellValue::Number(0.0)),
        FunctionArg::Lazy(&choice1),
    ];
    assert_eq!(
        choose_func.evaluate(&args_zero, &ctx).unwrap(),
        CellValue::Error(ExcelError::Value)
    );

    // Index exceeding choice count returns #VALUE!; fewer than 2 args returns TooFewArguments
    let args_overflow = vec![
        FunctionArg::Scalar(CellValue::Number(4.0)),
        FunctionArg::Lazy(&choice1),
        FunctionArg::Lazy(&choice2),
    ];
    assert_eq!(
        choose_func.evaluate(&args_overflow, &ctx).unwrap(),
        CellValue::Error(ExcelError::Value)
    );
    assert_eq!(
        choose_func.evaluate(&[FunctionArg::Scalar(CellValue::Number(1.0))], &ctx),
        Err(FunctionError::TooFewArguments {
            function: "CHOOSE",
            expected: 2,
            found: 1,
        })
    );
}

#[test]
fn test_scenario_7_retired_functions_unknown_function() {
    let registry = FunctionRegistry::standard();
    for retired in ["SWITCH", "COUNTA", "IFS", "IFNA", "DAYS", "ISBLANK", "ISNA"] {
        assert!(registry.lookup(retired).is_none());
    }

    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");
    let table = TableSpec {
        id: TableId::from("Data"),
        title: "Data".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("c1")],
        rows: vec![RowSpec::new("r1")],
    };
    symbols.register_table(sheet, &table).unwrap();
    let origin = CellId::new("Data", "r1", "c1");
    let values = IndexMap::new();

    for (formula, fn_name) in [
        ("SWITCH(1, 1, 10, 20)", "SWITCH"),
        ("IF(FALSE, SWITCH(1, 1, 10, 20), 0)", "SWITCH"),
        ("IFS(TRUE, 10, FALSE, 20)", "IFS"),
        ("IF(FALSE, IFS(TRUE, 10), 0)", "IFS"),
        ("IFNA(1 / 0, 0)", "IFNA"),
        ("IF(FALSE, IFNA(1 / 0, 0), 0)", "IFNA"),
        ("ISBLANK(Data[\"r1\", \"c1\"])", "ISBLANK"),
        ("IF(FALSE, ISBLANK(Data[\"r1\", \"c1\"]), FALSE)", "ISBLANK"),
        ("ISNA(1 / 0)", "ISNA"),
        ("IF(FALSE, ISNA(1 / 0), FALSE)", "ISNA"),
        ("COUNTA(Data[\"r1\", \"c1\"])", "COUNTA"),
        ("DAYS(45292, 45000)", "DAYS"),
    ] {
        let expr = Expr::parse(formula).unwrap();
        assert_eq!(
            registry.validate_expr(&expr),
            Err(FunctionError::UnknownFunction(fn_name.into())),
            "expected UnknownFunction({fn_name:?}) in Stage A for '{formula}'"
        );
        let err = eval_expr(&expr, &origin, &values, &symbols, &registry).unwrap_err();
        let EvalError::Function { cell, source } = err else {
            panic!("expected EvalError::Function for '{formula}', got {err:?}");
        };
        assert_eq!(*cell, origin);
        assert_eq!(*source, FunctionError::UnknownFunction(fn_name.into()));
    }
}

#[test]
fn test_scenario_8_end_to_end_formula_evaluation() {
    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");
    let registry = FunctionRegistry::standard();

    let table = TableSpec {
        id: TableId::from("Data"),
        title: "Data".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("c1"), ColumnSpec::new("c2")],
        rows: vec![RowSpec::new("r1"), RowSpec::new("r2")],
    };
    symbols.register_table(sheet.clone(), &table).unwrap();

    let origin = CellId::new("Data", "r1", "c1");
    let values = IndexMap::new();

    // AND formula
    assert_eq!(
        eval_str(
            "AND(10 > 5, 20 = 20)",
            &symbols,
            &values,
            &registry,
            &origin
        ),
        CellValue::Boolean(true)
    );

    // OR formula
    assert_eq!(
        eval_str("OR(10 < 5, 20 = 20)", &symbols, &values, &registry, &origin),
        CellValue::Boolean(true)
    );

    // NOT formula
    assert_eq!(
        eval_str("NOT(10 > 5)", &symbols, &values, &registry, &origin),
        CellValue::Boolean(false)
    );

    // IFERROR formula catching division by zero
    assert_eq!(
        eval_str("IFERROR(10 / 0, 42)", &symbols, &values, &registry, &origin),
        CellValue::Number(42.0)
    );

    // Nested IF formula short-circuiting
    assert_eq!(
        eval_str(
            "IF(1 = 2, 100, IF(3 = 3, 200, 300))",
            &symbols,
            &values,
            &registry,
            &origin
        ),
        CellValue::Number(200.0)
    );

    // CHOOSE formula
    assert_eq!(
        eval_str(
            "CHOOSE(2, \"A\", \"B\", \"C\")",
            &symbols,
            &values,
            &registry,
            &origin
        ),
        CellValue::Text("B".into())
    );

    // Boolean text literals in IF and NOT formulas are rejected strictly
    for bad_bool_formula in [
        "IF(\"TRUE\", 10, 20)",
        "IF(\"false\", 10, 20)",
        "NOT(\"true\")",
    ] {
        let parsed = Expr::parse(bad_bool_formula).unwrap();
        let err = eval_expr(&parsed, &origin, &values, &symbols, &registry).unwrap_err();
        assert!(
            matches!(err, EvalError::Type { .. }),
            "expected EvalError::Type for '{bad_bool_formula}', got {err:?}"
        );
    }

    // ISERROR and NA() end-to-end formulas
    assert_eq!(
        eval_str("ISERROR(\"\")", &symbols, &values, &registry, &origin),
        CellValue::Boolean(false)
    );
    assert_eq!(
        eval_str("ISERROR(1 / 0)", &symbols, &values, &registry, &origin),
        CellValue::Boolean(true)
    );
    assert_eq!(
        eval_str("NA()", &symbols, &values, &registry, &origin),
        CellValue::Error(ExcelError::Na)
    );
    assert_eq!(
        eval_str(
            "IFERROR(IF(FALSE, 10, NA()), 0)",
            &symbols,
            &values,
            &registry,
            &origin
        ),
        CellValue::Number(0.0)
    );
}

#[test]
fn test_scenario_9_iferror_propagates_function_error_and_traps_domain_errors_only() {
    struct FailingFirstEvalContext {
        eval_count: std::cell::Cell<usize>,
    }

    impl EvalContext for FailingFirstEvalContext {
        fn eval_expr(&self, _expr: &Expr) -> Result<CellValue, FunctionError> {
            self.eval_count.set(self.eval_count.get() + 1);
            Err(FunctionError::EvaluationFailure {
                function: "mock",
                message: "simulated internal failure".into(),
            })
        }
    }

    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");
    let registry = FunctionRegistry::standard();
    let table = TableSpec {
        id: TableId::from("Data"),
        title: "Data".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("c1")],
        rows: vec![RowSpec::new("r1")],
    };
    symbols.register_table(sheet, &table).unwrap();

    let origin = CellId::new("Data", "r1", "c1");
    let values = IndexMap::new();

    // 1. Domain error in args[0] is trapped by IFERROR
    assert_eq!(
        eval_str("IFERROR(10 / 0, 42)", &symbols, &values, &registry, &origin),
        CellValue::Number(42.0)
    );

    // 2. Active invalid call in args[0] propagates FunctionError instead of falling through
    let active_invalid = Expr::parse("IFERROR(ABS(1, 2, 3), 0)").unwrap();
    let err_active = eval_expr(&active_invalid, &origin, &values, &symbols, &registry).unwrap_err();
    let EvalError::Function { cell, source } = err_active else {
        panic!("expected EvalError::Function, got {err_active:?}");
    };
    assert_eq!(*cell, origin);
    assert_eq!(
        *source,
        FunctionError::TooManyArguments {
            function: "ABS",
            expected: 1,
            found: 3,
        }
    );

    // 3. Dormant invalid call in args[1] fails statically
    let dormant_invalid = Expr::parse("IFERROR(10, UNKNOWN_FN())").unwrap();
    let err_dormant =
        eval_expr(&dormant_invalid, &origin, &values, &symbols, &registry).unwrap_err();
    let EvalError::Function { cell, source } = err_dormant else {
        panic!("expected EvalError::Function, got {err_dormant:?}");
    };
    assert_eq!(*cell, origin);
    assert_eq!(*source, FunctionError::UnknownFunction("UNKNOWN_FN".into()));

    // 4. Runtime FunctionError (InvalidArgumentKind and EvaluationFailure) on args[0] in IFERROR propagates
    let iferror_func = registry.lookup("IFERROR").unwrap();
    let fallback_expr = Expr::number(42.0);
    let primary_expr = Expr::number(1.0);

    let mock_ctx = MockContext::new();
    let array_arg = FunctionArg::range(1, 1, vec![CellValue::Number(1.0)], ReferenceKind::Slice);

    let err_kind_iferror = iferror_func
        .evaluate(&[array_arg, FunctionArg::Lazy(&fallback_expr)], &mock_ctx)
        .unwrap_err();
    assert_eq!(
        err_kind_iferror,
        FunctionError::SliceInScalarContext { rows: 1, cols: 1 }
    );
    assert_eq!(mock_ctx.count(), 0);

    let fail_ctx = FailingFirstEvalContext {
        eval_count: std::cell::Cell::new(0),
    };
    let err_eval_iferror = iferror_func
        .evaluate(
            &[
                FunctionArg::Lazy(&primary_expr),
                FunctionArg::Lazy(&fallback_expr),
            ],
            &fail_ctx,
        )
        .unwrap_err();
    assert!(matches!(
        err_eval_iferror,
        FunctionError::EvaluationFailure { .. }
    ));
    assert_eq!(fail_ctx.eval_count.get(), 1);
}

#[test]
fn test_scenario_10_static_monomorphic_branch_unification_and_non_empty_lazy_arms() {
    let registry = FunctionRegistry::standard();
    let if_fn = registry.lookup("IF").unwrap();
    let choose_fn = registry.lookup("CHOOSE").unwrap();
    let iferror_fn = registry.lookup("IFERROR").unwrap();

    // Static IncompatibleBranchTypes across IF, CHOOSE, IFERROR
    for (formula, fn_name, pos) in [
        ("IF(TRUE, 10, \"NM\")", "IF", 3),
        ("CHOOSE(1, 10, \"NM\")", "CHOOSE", 3),
        ("IFERROR(10, \"N/A\")", "IFERROR", 2),
    ] {
        let expr = Expr::parse(formula).unwrap();
        assert_eq!(
            registry.validate_expr(&expr),
            Err(FunctionError::Type(TypeError::IncompatibleBranchTypes {
                function: fn_name,
                position: pos,
                expected: ValueType::Number,
                found: ValueType::Text,
            }))
        );
    }

    struct EmptyEvalContext;
    impl EvalContext for EmptyEvalContext {
        fn eval_expr(&self, _expr: &Expr) -> Result<CellValue, FunctionError> {
            Ok(CellValue::Empty)
        }
    }

    // IF, CHOOSE, and IFERROR reject CellValue::Empty on evaluated arms with EmptyArgument
    let call_args = [
        CallArg::Expr(Expr::parse("1").unwrap()),
        CallArg::Expr(Expr::parse("1").unwrap()),
    ];
    assert_eq!(
        registry.bind_arguments(iferror_fn, &call_args, &EmptyEvalContext),
        Err(FunctionError::Type(TypeError::EmptyArgument {
            function: "IFERROR",
            param: "value",
            position: 1,
        }))
    );
    let dummy = Expr::number(1.0);
    assert_eq!(
        iferror_fn.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
                FunctionArg::Lazy(&dummy),
            ],
            &EmptyEvalContext,
        ),
        Err(FunctionError::Type(TypeError::EmptyArgument {
            function: "IFERROR",
            param: "value_if_error",
            position: 2,
        }))
    );
    assert_eq!(
        if_fn.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Boolean(true)),
                FunctionArg::Lazy(&dummy),
                FunctionArg::Lazy(&dummy),
            ],
            &EmptyEvalContext,
        ),
        Err(FunctionError::Type(TypeError::EmptyArgument {
            function: "IF",
            param: "value_if_true",
            position: 2,
        }))
    );
    assert_eq!(
        if_fn.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Boolean(false)),
                FunctionArg::Lazy(&dummy),
                FunctionArg::Lazy(&dummy),
            ],
            &EmptyEvalContext,
        ),
        Err(FunctionError::Type(TypeError::EmptyArgument {
            function: "IF",
            param: "value_if_false",
            position: 3,
        }))
    );
    assert_eq!(
        choose_fn.evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(2.0)),
                FunctionArg::Lazy(&dummy),
                FunctionArg::Lazy(&dummy),
            ],
            &EmptyEvalContext,
        ),
        Err(FunctionError::Type(TypeError::EmptyArgument {
            function: "CHOOSE",
            param: "value1",
            position: 3,
        }))
    );
}
