#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use indexmap::IndexMap;

use f8_core::compiler::eval::{EvalError, eval_expr};
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::ir::symbol::SymbolTable;
use f8_core::expr::ast::{CallArg, Expr};
use f8_core::functions::traits::{EvalContext, FunctionArg};
use f8_core::functions::{FunctionCategory, FunctionError, FunctionRegistry};
use f8_core::model::id::{SheetId, TableId};
use f8_core::model::table::{ColumnSpec, RowSpec, TableSpec};
use f8_core::value::{CellValue, ExcelError};

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

    let logical_funcs = [
        "IF", "IFS", "IFERROR", "IFNA", "AND", "OR", "NOT", "CHOOSE", "SWITCH",
    ];

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

    // Pure scalars: all true
    let args_true = vec![
        FunctionArg::Scalar(CellValue::Boolean(true)),
        FunctionArg::Scalar(CellValue::Number(1.0)),
        FunctionArg::Scalar(CellValue::Text("true".into())),
    ];
    assert_eq!(
        and_func.evaluate(&args_true, &ctx).unwrap(),
        CellValue::Boolean(true)
    );
    assert_eq!(
        or_func.evaluate(&args_true, &ctx).unwrap(),
        CellValue::Boolean(true)
    );

    // Pure scalars: mixed
    let args_mixed = vec![
        FunctionArg::Scalar(CellValue::Boolean(true)),
        FunctionArg::Scalar(CellValue::Number(0.0)),
        FunctionArg::Scalar(CellValue::Text("false".into())),
    ];
    assert_eq!(
        and_func.evaluate(&args_mixed, &ctx).unwrap(),
        CellValue::Boolean(false)
    );
    assert_eq!(
        or_func.evaluate(&args_mixed, &ctx).unwrap(),
        CellValue::Boolean(true)
    );

    // Range reference: ignores text and empty cells
    let args_range = vec![FunctionArg::Array {
        rows: 4,
        cols: 1,
        values: vec![
            CellValue::Boolean(true),
            CellValue::Text("some_text".into()),
            CellValue::Empty,
            CellValue::Number(2.5),
        ],
        is_range_ref: true,
    }];
    assert_eq!(
        and_func.evaluate(&args_range, &ctx).unwrap(),
        CellValue::Boolean(true)
    );
    assert_eq!(
        or_func.evaluate(&args_range, &ctx).unwrap(),
        CellValue::Boolean(true)
    );

    // Range with zero logical values returns #VALUE!
    let args_empty_range = vec![FunctionArg::Array {
        rows: 2,
        cols: 1,
        values: vec![CellValue::Text("abc".into()), CellValue::Empty],
        is_range_ref: true,
    }];
    assert_eq!(
        and_func.evaluate(&args_empty_range, &ctx).unwrap(),
        CellValue::Error(ExcelError::Value)
    );
    assert_eq!(
        or_func.evaluate(&args_empty_range, &ctx).unwrap(),
        CellValue::Error(ExcelError::Value)
    );

    // Error in range bubbles immediately
    let args_err_range = vec![FunctionArg::Array {
        rows: 2,
        cols: 1,
        values: vec![CellValue::Boolean(true), CellValue::Error(ExcelError::Div0)],
        is_range_ref: true,
    }];
    assert_eq!(
        and_func.evaluate(&args_err_range, &ctx).unwrap(),
        CellValue::Error(ExcelError::Div0)
    );
    assert_eq!(
        or_func.evaluate(&args_err_range, &ctx).unwrap(),
        CellValue::Error(ExcelError::Div0)
    );

    // Non-boolean text in scalar produces #VALUE!
    let args_bad_text = vec![
        FunctionArg::Scalar(CellValue::Boolean(true)),
        FunctionArg::Scalar(CellValue::Text("invalid".into())),
    ];
    assert_eq!(
        and_func.evaluate(&args_bad_text, &ctx).unwrap(),
        CellValue::Error(ExcelError::Value)
    );
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
            .evaluate(&[FunctionArg::Scalar(CellValue::Number(0.0))], &ctx)
            .unwrap(),
        CellValue::Boolean(true)
    );
    assert_eq!(
        not_func
            .evaluate(&[FunctionArg::Scalar(CellValue::Number(10.0))], &ctx)
            .unwrap(),
        CellValue::Boolean(false)
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
    assert_eq!(
        not_func
            .evaluate(&[FunctionArg::Scalar(CellValue::Text("TRUE".into()))], &ctx)
            .unwrap(),
        CellValue::Boolean(false)
    );
    assert_eq!(
        not_func
            .evaluate(
                &[FunctionArg::Scalar(CellValue::Text("false".into()))],
                &ctx
            )
            .unwrap(),
        CellValue::Boolean(true)
    );
    assert_eq!(
        not_func
            .evaluate(&[FunctionArg::Scalar(CellValue::Text("foo".into()))], &ctx)
            .unwrap(),
        CellValue::Error(ExcelError::Value)
    );
}

#[test]
fn test_scenario_4_iferror_and_ifna_lazy_evaluation() {
    let registry = FunctionRegistry::standard();
    let iferror_func = registry.lookup("IFERROR").unwrap();
    let ifna_func = registry.lookup("IFNA").unwrap();
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

    // IFNA: valid value does not evaluate fallback
    let res_na_val = ifna_func
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Number(99.0)),
                FunctionArg::Lazy(&dummy_expr),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res_na_val, CellValue::Number(99.0));

    // IFNA: other errors (e.g. #DIV/0!) are NOT caught; fallback not evaluated
    let res_div0 = ifna_func
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Error(ExcelError::Div0)),
                FunctionArg::Lazy(&dummy_expr),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res_div0, CellValue::Error(ExcelError::Div0));

    // IFNA: #N/A is caught; fallback evaluated
    let res_caught = ifna_func
        .evaluate(
            &[
                FunctionArg::Scalar(CellValue::Error(ExcelError::Na)),
                FunctionArg::Lazy(&dummy_expr),
            ],
            &ctx,
        )
        .unwrap();
    assert_eq!(res_caught, CellValue::Number(42.0));
}

#[test]
fn test_scenario_5_ifs_short_circuiting_and_na_fallback() {
    let registry = FunctionRegistry::standard();
    let ifs_func = registry.lookup("IFS").unwrap();
    let ctx = MockContext::new();

    let expr_true = Expr::Literal(f8_core::expr::ast::Literal::Boolean(true));
    let expr_false = Expr::Literal(f8_core::expr::ast::Literal::Boolean(false));
    let expr_res1 = Expr::Literal(f8_core::expr::ast::Literal::String("first".into()));
    let expr_res2 = Expr::Literal(f8_core::expr::ast::Literal::String("second".into()));
    let expr_unreachable = Expr::Literal(f8_core::expr::ast::Literal::String("unreachable".into()));

    // IFS(FALSE, "first", TRUE, "second", ...)
    let args = vec![
        FunctionArg::Lazy(&expr_false),
        FunctionArg::Lazy(&expr_res1),
        FunctionArg::Lazy(&expr_true),
        FunctionArg::Lazy(&expr_res2),
        FunctionArg::Lazy(&expr_unreachable),
        FunctionArg::Lazy(&expr_unreachable),
    ];
    let res = ifs_func.evaluate(&args, &ctx).unwrap();
    assert_eq!(res, CellValue::Text("second".into()));

    // All false conditions return #N/A
    let args_all_false = vec![
        FunctionArg::Lazy(&expr_false),
        FunctionArg::Lazy(&expr_res1),
        FunctionArg::Lazy(&expr_false),
        FunctionArg::Lazy(&expr_res2),
    ];
    let res_na = ifs_func.evaluate(&args_all_false, &ctx).unwrap();
    assert_eq!(res_na, CellValue::Error(ExcelError::Na));

    // Validation rejects odd argument counts
    let single_arg = vec![CallArg::from(Expr::Literal(
        f8_core::expr::ast::Literal::Boolean(true),
    ))];
    assert!(matches!(
        registry.validate_call("IFS", &single_arg),
        Err(FunctionError::TooFewArguments { .. })
    ));

    let odd_args = vec![
        CallArg::from(Expr::Literal(f8_core::expr::ast::Literal::Boolean(true))),
        CallArg::from(Expr::Literal(f8_core::expr::ast::Literal::Number(1.0))),
        CallArg::from(Expr::Literal(f8_core::expr::ast::Literal::Boolean(false))),
    ];
    assert!(matches!(
        registry.validate_call("IFS", &odd_args),
        Err(FunctionError::InvalidVariadicPairs { .. })
    ));

    // Validation correctly associates cyclical parameter names for omitted arguments in pairs
    let omitted_cond = vec![
        CallArg::from(Expr::Literal(f8_core::expr::ast::Literal::Boolean(true))),
        CallArg::from(Expr::Literal(f8_core::expr::ast::Literal::Number(1.0))),
        CallArg::Omitted,
        CallArg::from(Expr::Literal(f8_core::expr::ast::Literal::Number(2.0))),
    ];
    match registry.validate_call("IFS", &omitted_cond) {
        Err(FunctionError::CannotOmitRequiredArgument {
            param, position, ..
        }) => {
            assert_eq!(param, "logical_test1");
            assert_eq!(position, 3);
        }
        other => panic!("expected CannotOmitRequiredArgument for position 3, got {other:?}"),
    }

    let omitted_val = vec![
        CallArg::from(Expr::Literal(f8_core::expr::ast::Literal::Boolean(true))),
        CallArg::from(Expr::Literal(f8_core::expr::ast::Literal::Number(1.0))),
        CallArg::from(Expr::Literal(f8_core::expr::ast::Literal::Boolean(false))),
        CallArg::Omitted,
    ];
    match registry.validate_call("IFS", &omitted_val) {
        Err(FunctionError::CannotOmitRequiredArgument {
            param, position, ..
        }) => {
            assert_eq!(param, "value_if_true1");
            assert_eq!(position, 4);
        }
        other => panic!("expected CannotOmitRequiredArgument for position 4, got {other:?}"),
    }

    // Text boolean coercion in IFS condition
    let expr_text_true = Expr::Literal(f8_core::expr::ast::Literal::String("true".into()));
    let args_text_cond = vec![
        FunctionArg::Lazy(&expr_text_true),
        FunctionArg::Lazy(&expr_res1),
    ];
    assert_eq!(
        ifs_func.evaluate(&args_text_cond, &ctx).unwrap(),
        CellValue::Text("first".into())
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

    // Floating index 2.9 truncates to 2
    let args_trunc = vec![
        FunctionArg::Scalar(CellValue::Number(2.9)),
        FunctionArg::Lazy(&choice1),
        FunctionArg::Lazy(&choice2),
        FunctionArg::Lazy(&choice3),
    ];
    let res_trunc = choose_func.evaluate(&args_trunc, &ctx).unwrap();
    assert_eq!(res_trunc, CellValue::Text("B".into()));

    // Index 0 or negative returns #VALUE!
    let args_zero = vec![
        FunctionArg::Scalar(CellValue::Number(0.0)),
        FunctionArg::Lazy(&choice1),
    ];
    assert_eq!(
        choose_func.evaluate(&args_zero, &ctx).unwrap(),
        CellValue::Error(ExcelError::Value)
    );

    // Index exceeding choice count returns #VALUE!
    let args_overflow = vec![
        FunctionArg::Scalar(CellValue::Number(4.0)),
        FunctionArg::Lazy(&choice1),
        FunctionArg::Lazy(&choice2),
    ];
    assert_eq!(
        choose_func.evaluate(&args_overflow, &ctx).unwrap(),
        CellValue::Error(ExcelError::Value)
    );
}

#[test]
fn test_scenario_7_switch_matching_and_default() {
    let registry = FunctionRegistry::standard();
    let switch_func = registry.lookup("SWITCH").unwrap();
    let ctx = MockContext::new();

    let val1 = Expr::Literal(f8_core::expr::ast::Literal::String("apple".into()));
    let res1 = Expr::Literal(f8_core::expr::ast::Literal::Number(1.0));
    let val2 = Expr::Literal(f8_core::expr::ast::Literal::String("banana".into()));
    let res2 = Expr::Literal(f8_core::expr::ast::Literal::Number(2.0));
    let def_expr = Expr::Literal(f8_core::expr::ast::Literal::Number(99.0));

    // Case-insensitive match on "BANANA"
    let args_match = vec![
        FunctionArg::Scalar(CellValue::Text("BANANA".into())),
        FunctionArg::Lazy(&val1),
        FunctionArg::Lazy(&res1),
        FunctionArg::Lazy(&val2),
        FunctionArg::Lazy(&res2),
        FunctionArg::Lazy(&def_expr),
    ];
    let res = switch_func.evaluate(&args_match, &ctx).unwrap();
    assert_eq!(res, CellValue::Number(2.0));

    // No match with default
    let args_default = vec![
        FunctionArg::Scalar(CellValue::Text("orange".into())),
        FunctionArg::Lazy(&val1),
        FunctionArg::Lazy(&res1),
        FunctionArg::Lazy(&val2),
        FunctionArg::Lazy(&res2),
        FunctionArg::Lazy(&def_expr),
    ];
    let res_def = switch_func.evaluate(&args_default, &ctx).unwrap();
    assert_eq!(res_def, CellValue::Number(99.0));

    // No match without default returns #N/A
    let args_no_match = vec![
        FunctionArg::Scalar(CellValue::Text("orange".into())),
        FunctionArg::Lazy(&val1),
        FunctionArg::Lazy(&res1),
        FunctionArg::Lazy(&val2),
        FunctionArg::Lazy(&res2),
    ];
    let res_na = switch_func.evaluate(&args_no_match, &ctx).unwrap();
    assert_eq!(res_na, CellValue::Error(ExcelError::Na));

    // Empty coerces to Number(0.0) on expression and value_i (skips "" and FALSE, matches 0.0 and Empty)
    let empty_str_val = Expr::Literal(f8_core::expr::ast::Literal::String(String::new()));
    let false_val = Expr::Literal(f8_core::expr::ast::Literal::Boolean(false));
    let zero_val = Expr::Literal(f8_core::expr::ast::Literal::Number(0.0));
    let res3 = Expr::Literal(f8_core::expr::ast::Literal::Number(3.0));
    let args_empty_target = vec![
        FunctionArg::Scalar(CellValue::Empty),
        FunctionArg::Lazy(&empty_str_val),
        FunctionArg::Lazy(&res1),
        FunctionArg::Lazy(&false_val),
        FunctionArg::Lazy(&res2),
        FunctionArg::Lazy(&zero_val),
        FunctionArg::Lazy(&res3),
        FunctionArg::Lazy(&def_expr),
    ];
    assert_eq!(
        switch_func.evaluate(&args_empty_target, &ctx).unwrap(),
        CellValue::Number(3.0)
    );

    // Numeric 0.0 matches Empty case_val, while "" and FALSE do not match Empty case_val
    let args_zero_vs_empty = vec![
        FunctionArg::Scalar(CellValue::Number(0.0)),
        FunctionArg::Scalar(CellValue::Empty),
        FunctionArg::Lazy(&res1),
        FunctionArg::Lazy(&def_expr),
    ];
    assert_eq!(
        switch_func.evaluate(&args_zero_vs_empty, &ctx).unwrap(),
        CellValue::Number(1.0)
    );

    for non_matching_target in [CellValue::Text(String::new()), CellValue::Boolean(false)] {
        let args_non_match_empty = vec![
            FunctionArg::Scalar(non_matching_target),
            FunctionArg::Scalar(CellValue::Empty),
            FunctionArg::Lazy(&res1),
            FunctionArg::Lazy(&def_expr),
        ];
        assert_eq!(
            switch_func.evaluate(&args_non_match_empty, &ctx).unwrap(),
            CellValue::Number(99.0)
        );
    }

    // Empty in result_i / default remains uncoerced CellValue::Empty mid-expression
    let args_empty_result = vec![
        FunctionArg::Scalar(CellValue::Empty),
        FunctionArg::Scalar(CellValue::Empty),
        FunctionArg::Scalar(CellValue::Empty),
    ];
    assert_eq!(
        switch_func.evaluate(&args_empty_result, &ctx).unwrap(),
        CellValue::Empty
    );
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

    // IFNA formula ignoring division by zero
    assert_eq!(
        eval_str("IFNA(10 / 0, 42)", &symbols, &values, &registry, &origin),
        CellValue::Error(ExcelError::Div0)
    );

    // IFS formula short-circuiting
    assert_eq!(
        eval_str(
            "IFS(1 = 2, 100, 3 = 3, 200, 4 = 4, 300)",
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

    // SWITCH formula
    assert_eq!(
        eval_str(
            "SWITCH(2, 1, \"one\", 2, \"two\", \"other\")",
            &symbols,
            &values,
            &registry,
            &origin
        ),
        CellValue::Text("two".into())
    );

    // Boolean text literals in IF and NOT formulas
    assert_eq!(
        eval_str(
            "IF(\"TRUE\", 10, 20)",
            &symbols,
            &values,
            &registry,
            &origin
        ),
        CellValue::Number(10.0)
    );
    assert_eq!(
        eval_str(
            "IF(\"false\", 10, 20)",
            &symbols,
            &values,
            &registry,
            &origin
        ),
        CellValue::Number(20.0)
    );
    assert_eq!(
        eval_str("NOT(\"true\")", &symbols, &values, &registry, &origin),
        CellValue::Boolean(false)
    );
}

#[test]
fn test_scenario_9_iferror_and_ifna_propagate_function_error_and_trap_domain_errors_only() {
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

    // 4. Runtime FunctionError (InvalidArgumentKind and EvaluationFailure) on args[0] in IFERROR and IFNA propagates
    let iferror_func = registry.lookup("IFERROR").unwrap();
    let ifna_func = registry.lookup("IFNA").unwrap();
    let fallback_expr = Expr::number(42.0);
    let primary_expr = Expr::number(1.0);

    let mock_ctx = MockContext::new();
    let array_arg = FunctionArg::Array {
        rows: 1,
        cols: 1,
        values: vec![CellValue::Number(1.0)],
        is_range_ref: true,
    };

    let err_kind_iferror = iferror_func
        .evaluate(
            &[array_arg.clone(), FunctionArg::Lazy(&fallback_expr)],
            &mock_ctx,
        )
        .unwrap_err();
    assert_eq!(
        err_kind_iferror,
        FunctionError::InvalidArgumentKind {
            function: "IFERROR",
            expected: "scalar or expression",
            found: "array",
        }
    );
    assert_eq!(mock_ctx.count(), 0);

    let err_kind_ifna = ifna_func
        .evaluate(&[array_arg, FunctionArg::Lazy(&fallback_expr)], &mock_ctx)
        .unwrap_err();
    assert_eq!(
        err_kind_ifna,
        FunctionError::InvalidArgumentKind {
            function: "IFNA",
            expected: "scalar or expression",
            found: "array",
        }
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

    let fail_ctx_na = FailingFirstEvalContext {
        eval_count: std::cell::Cell::new(0),
    };
    let err_eval_ifna = ifna_func
        .evaluate(
            &[
                FunctionArg::Lazy(&primary_expr),
                FunctionArg::Lazy(&fallback_expr),
            ],
            &fail_ctx_na,
        )
        .unwrap_err();
    assert!(matches!(
        err_eval_ifna,
        FunctionError::EvaluationFailure { .. }
    ));
    assert_eq!(fail_ctx_na.eval_count.get(), 1);
}
