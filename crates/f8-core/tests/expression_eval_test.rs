#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use indexmap::IndexMap;

use f8_core::compiler::eval::{EvalError, eval_expr};
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::ir::symbol::SymbolTable;
use f8_core::expr::ast::Expr;
use f8_core::functions::FunctionError;
use f8_core::functions::registry::FunctionRegistry;
use f8_core::model::id::{SheetId, TableId};
use f8_core::model::table::{ColumnSpec, RowSpec, TableSpec};
use f8_core::value::{CellValue, ExcelError, TypeError, ValueType};

/// Helper to set up a standard test symbol table containing a 2-table model:
/// - `Assumptions` table: row `tax` (`rate`: 0.25), row `flag` (`active`: TRUE), row `empty_row` (`val`: Empty)
/// - `IncomeStatement` table: rows `rev`, `cogs`, `sga`, `ebit` across columns `2023A`, `2024E`, `2025E`
fn build_test_environment() -> (SymbolTable, FunctionRegistry) {
    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");

    let assumptions_table = TableSpec {
        id: TableId::from("Assumptions"),
        title: "Assumptions".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("rate"),
            ColumnSpec::new("active"),
            ColumnSpec::new("val"),
        ],
        rows: vec![
            RowSpec::new("tax"),
            RowSpec::new("flag"),
            RowSpec::new("empty_row"),
        ],
    };
    symbols
        .register_table(sheet.clone(), &assumptions_table)
        .expect("register assumptions");

    let is_table = TableSpec {
        id: TableId::from("IncomeStatement"),
        title: "Income Statement".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::new("2023A"),
            ColumnSpec::new("2024E"),
            ColumnSpec::new("2025E"),
        ],
        rows: vec![
            RowSpec::new("rev"),
            RowSpec::new("cogs"),
            RowSpec::new("sga"),
            RowSpec::new("ebit"),
        ],
    };
    symbols
        .register_table(sheet, &is_table)
        .expect("register is");

    let registry = FunctionRegistry::standard();
    (symbols, registry)
}

#[test]
fn test_scenario_1_formula_arithmetic_and_strict_scalar_rejection() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let mut memo = IndexMap::new();

    let cell_empty = CellId::new("Assumptions", "empty_row", "val");
    memo.insert(cell_empty, CellValue::Empty);

    // 10 + 20 * 2 = 50.0
    let expr1 = Expr::parse("10 + 20 * 2").expect("valid expr");
    let val1 = eval_expr(&expr1, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val1, CellValue::Number(50.0));

    // Dynamic Empty + 5 fails in Stage B with EmptyOperand
    let expr2 = Expr::parse("Assumptions[\"empty_row\", \"val\"] + 5").expect("valid expr");
    let err2 = eval_expr(&expr2, &origin, &memo, &symbols, &registry).unwrap_err();
    assert!(matches!(
        err2,
        EvalError::Type {
            ref cell,
            source: TypeError::EmptyOperand { operator: "+" },
        } if cell.as_ref() == &origin
    ));

    // Static Boolean literals in arithmetic fail in Stage A with EvalError::Type
    for (formula, op) in [("TRUE * 10", "*"), ("FALSE + 2", "+")] {
        let expr = Expr::parse(formula).expect("valid expr");
        let err = eval_expr(&expr, &origin, &memo, &symbols, &registry).unwrap_err();
        let EvalError::Type { cell, source } = err else {
            panic!("expected EvalError::Type for '{formula}', got {err:?}");
        };
        assert_eq!(*cell, origin);
        assert_eq!(
            source,
            TypeError::InvalidOperandType {
                operator: op,
                expected: ValueType::Number,
                found: ValueType::Boolean,
            }
        );
    }
}

#[test]
fn test_scenario_2_divide_by_zero_power_anomalies_and_float_normalization() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let mut memo = IndexMap::new();

    let cell_empty = CellId::new("Assumptions", "empty_row", "val");
    memo.insert(cell_empty, CellValue::Empty);

    // 10 / 0 -> #DIV/0!
    let expr_div_zero = Expr::parse("10 / 0").expect("valid expr");
    let val = eval_expr(&expr_div_zero, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val, CellValue::Error(ExcelError::Div0));

    // 10 / Empty -> fails strictly with EmptyOperand rather than coercing Empty to 0
    let expr_div_empty =
        Expr::parse("10 / Assumptions[\"empty_row\", \"val\"]").expect("valid expr");
    let err = eval_expr(&expr_div_empty, &origin, &memo, &symbols, &registry).unwrap_err();
    assert!(matches!(
        err,
        EvalError::Type {
            source: TypeError::EmptyOperand { operator: "/" },
            ..
        }
    ));

    // 0 ^ -1 -> #DIV/0!
    let expr_pow_div0 = Expr::parse("0 ^ -1").expect("valid expr");
    let val = eval_expr(&expr_pow_div0, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val, CellValue::Error(ExcelError::Div0));

    // (-2) ^ 0.5 -> #NUM!
    let expr_complex_pow = Expr::parse("(-2) ^ 0.5").expect("valid expr");
    let val = eval_expr(&expr_complex_pow, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val, CellValue::Error(ExcelError::Num));

    // 10 ^ 309 -> #NUM! (float overflow)
    let expr_overflow = Expr::parse("10 ^ 309").expect("valid expr");
    let val = eval_expr(&expr_overflow, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val, CellValue::Error(ExcelError::Num));

    // 0 * -1 -> -0.0 normalized to 0.0
    let expr_neg_zero = Expr::parse("0 * -1").expect("valid expr");
    let val = eval_expr(&expr_neg_zero, &origin, &memo, &symbols, &registry).expect("eval ok");
    if let CellValue::Number(n) = val {
        assert_eq!(n, 0.0);
        assert!(
            !n.is_sign_negative(),
            "expected positive zero normalization"
        );
    } else {
        panic!("expected Number(0.0)");
    }

    // Downstream cells bubble errors when the other operand is typed validly
    let cell_err = CellId::new("IncomeStatement", "cogs", "2024E");
    memo.insert(cell_err, CellValue::Error(ExcelError::Div0));
    let expr_bubble = Expr::parse("IncomeStatement[\"cogs\", @col] + 100").expect("valid expr");
    let val = eval_expr(&expr_bubble, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val, CellValue::Error(ExcelError::Div0));

    // Type mismatch on RHS wins over LHS #DIV/0! error
    let cell_sga = CellId::new("IncomeStatement", "sga", "2024E");
    memo.insert(cell_sga, CellValue::Text("bad".into()));
    let expr_err_plus_text =
        Expr::parse("IncomeStatement[\"cogs\", @col] + IncomeStatement[\"sga\", @col]")
            .expect("valid expr");
    let err = eval_expr(&expr_err_plus_text, &origin, &memo, &symbols, &registry).unwrap_err();
    assert!(matches!(
        err,
        EvalError::Type {
            source: TypeError::InvalidOperandType {
                operator: "+",
                expected: ValueType::Number,
                found: ValueType::Text,
            },
            ..
        }
    ));
}

#[test]
fn test_scenario_3_strict_same_type_comparisons_and_relational_rejection() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let mut memo = IndexMap::new();

    let cell_empty = CellId::new("Assumptions", "empty_row", "val");
    memo.insert(cell_empty, CellValue::Empty);
    memo.insert(
        CellId::new("Assumptions", "tax", "rate"),
        CellValue::Number(10.0),
    );
    memo.insert(
        CellId::new("Assumptions", "flag", "active"),
        CellValue::Text("2".into()),
    );

    // Same-type comparisons succeed (Number, case-insensitive Text, Boolean)
    for (formula, expected) in [
        ("10 < 20", true),
        ("10 = 10", true),
        ("\"Apple\" = \"apple\"", true),
        ("\"apple\" < \"Banana\"", true),
        ("FALSE < TRUE", true),
        ("(1 = 1) = TRUE", true),
        ("TRUE = (1 = 1)", true),
    ] {
        let expr = Expr::parse(formula).expect("valid expr");
        assert_eq!(
            eval_expr(&expr, &origin, &memo, &symbols, &registry).expect("eval ok"),
            CellValue::Boolean(expected),
            "mismatch on same-type comparison: {formula}"
        );
    }

    // Static cross-type literal comparisons fail in Stage A with IncompatibleComparison
    for (formula, operator, left, right) in [
        ("10 < \"2\"", "<", ValueType::Number, ValueType::Text),
        ("100 < TRUE", "<", ValueType::Number, ValueType::Boolean),
        ("\"\" < FALSE", "<", ValueType::Text, ValueType::Boolean),
        ("1 = \"1\"", "=", ValueType::Number, ValueType::Text),
    ] {
        let expr = Expr::parse(formula).expect("valid expr");
        let err = eval_expr(&expr, &origin, &memo, &symbols, &registry).unwrap_err();
        let EvalError::Type { cell, source } = err else {
            panic!("expected EvalError::Type on '{formula}', got {err:?}");
        };
        assert_eq!(*cell, origin);
        assert_eq!(
            source,
            TypeError::IncompatibleComparison {
                operator,
                left,
                right
            }
        );
    }

    // Dynamic cross-type comparisons fail in Stage B with IncompatibleComparison
    let cross_expr =
        Expr::parse("Assumptions[\"tax\", \"rate\"] < Assumptions[\"flag\", \"active\"]")
            .expect("valid expr");
    let cross_err = eval_expr(&cross_expr, &origin, &memo, &symbols, &registry).unwrap_err();
    assert!(matches!(
        cross_err,
        EvalError::Type {
            source: TypeError::IncompatibleComparison {
                operator: "<",
                left: ValueType::Number,
                right: ValueType::Text,
            },
            ..
        }
    ));

    // Dynamic Empty comparisons (including paired with #DIV/0!) fail in Stage B with EmptyOperand
    memo.insert(
        CellId::new("IncomeStatement", "cogs", "2024E"),
        CellValue::Error(ExcelError::Div0),
    );
    for formula in [
        "Assumptions[\"empty_row\", \"val\"] = 0",
        "Assumptions[\"empty_row\", \"val\"] = \"\"",
        "Assumptions[\"empty_row\", \"val\"] = FALSE",
        "Assumptions[\"empty_row\", \"val\"] <> FALSE",
        "Assumptions[\"empty_row\", \"val\"] < FALSE",
        "Assumptions[\"empty_row\", \"val\"] <= FALSE",
        "Assumptions[\"empty_row\", \"val\"] > FALSE",
        "Assumptions[\"empty_row\", \"val\"] >= FALSE",
        "Assumptions[\"empty_row\", \"val\"] < TRUE",
        "Assumptions[\"empty_row\", \"val\"] = TRUE",
        "FALSE = Assumptions[\"empty_row\", \"val\"]",
        "TRUE > Assumptions[\"empty_row\", \"val\"]",
        "Assumptions[\"empty_row\", \"val\"] = Assumptions[\"empty_row\", \"val\"]",
        "IncomeStatement[\"cogs\", @col] = Assumptions[\"empty_row\", \"val\"]",
    ] {
        let expr = Expr::parse(formula).expect("valid expr");
        let err = eval_expr(&expr, &origin, &memo, &symbols, &registry).unwrap_err();
        assert!(
            matches!(
                err,
                EvalError::Type {
                    source: TypeError::EmptyOperand { .. },
                    ..
                }
            ),
            "expected EmptyOperand on '{formula}', got {err:?}"
        );
    }
}

#[test]
fn test_scenario_4_short_circuit_lazy_evaluation_if() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let memo = IndexMap::new();

    // IF(TRUE, 100, 1 / 0) -> true branch executed, false branch (1/0) never executed
    let expr1 = Expr::parse("IF(TRUE, 100, 1 / 0)").expect("valid expr");
    let val1 = eval_expr(&expr1, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val1, CellValue::Number(100.0));

    // IF(FALSE, 1 / 0, 200) -> false branch executed, true branch (1/0) never executed
    let expr2 = Expr::parse("IF(FALSE, 1 / 0, 200)").expect("valid expr");
    let val2 = eval_expr(&expr2, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val2, CellValue::Number(200.0));
}

#[test]
fn test_scenario_5_syntactic_slice_in_scalar_position_rejection() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let memo = IndexMap::new();

    for (formula, expected_rows, expected_cols) in [
        ("IncomeStatement[\"cogs\"..\"sga\", @col] + 10", 2, 1),
        ("IncomeStatement[\"cogs\"..\"cogs\", @col] + 10", 1, 1),
        ("IncomeStatement[\"cogs\"..\"sga\", @col]", 2, 1),
        ("IncomeStatement[\"cogs\"..\"cogs\", @col]", 1, 1),
    ] {
        let expr = Expr::parse(formula).expect("valid expr");
        let err = eval_expr(&expr, &origin, &memo, &symbols, &registry).unwrap_err();
        let EvalError::SliceInScalarContext { cell, rows, cols } = err else {
            panic!("expected SliceInScalarContext for '{formula}', got {err:?}");
        };
        assert_eq!(*cell, origin);
        assert_eq!(
            (rows, cols),
            (expected_rows, expected_cols),
            "mismatch on '{formula}'"
        );
    }
}

#[test]
fn test_scenario_6_2d_range_resolution_and_aggregation_sum() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "ebit", "2024E");
    let mut memo = IndexMap::new();

    // Sparse numeric range: rev = 100.0, cogs = Empty, sga = Empty -> 100.0
    memo.insert(
        CellId::new("IncomeStatement", "rev", "2024E"),
        CellValue::Number(100.0),
    );
    memo.insert(
        CellId::new("IncomeStatement", "cogs", "2024E"),
        CellValue::Empty,
    );
    memo.insert(
        CellId::new("IncomeStatement", "sga", "2024E"),
        CellValue::Empty,
    );

    let expr = Expr::parse("SUM(IncomeStatement[\"rev\"..\"sga\", @col])").expect("valid expr");
    let val = eval_expr(&expr, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val, CellValue::Number(100.0));

    // Also test adding another numeric cell -> 130.0
    memo.insert(
        CellId::new("IncomeStatement", "sga", "2024E"),
        CellValue::Number(30.0),
    );
    let val2 = eval_expr(&expr, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val2, CellValue::Number(130.0));

    // Non-numeric text cell inside SUM slice fails strictly with EvalError::Type
    memo.insert(
        CellId::new("IncomeStatement", "cogs", "2024E"),
        CellValue::Text("CostNote".into()),
    );
    let err = eval_expr(&expr, &origin, &memo, &symbols, &registry).unwrap_err();
    assert!(matches!(err, EvalError::Type { .. }));
}

#[test]
fn test_scenario_7_static_function_validation_in_eval_expr() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let memo = IndexMap::new();

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
            "IFS(TRUE, 1, FALSE, 2)",
            FunctionError::UnknownFunction("IFS".into()),
        ),
        (
            "IFERROR(10, UNKNOWN_FN())",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        (
            "IFERROR(ABS(1, 2, 3), 0)",
            FunctionError::TooManyArguments {
                function: "ABS",
                expected: 1,
                found: 3,
            },
        ),
        ("IFNA(10, 0)", FunctionError::UnknownFunction("IFNA".into())),
        (
            "COUNTA(Assumptions[\"tax\"..\"flag\", \"rate\"])",
            FunctionError::UnknownFunction("COUNTA".into()),
        ),
        (
            "DAYS(45474, 45292)",
            FunctionError::UnknownFunction("DAYS".into()),
        ),
        (
            "ISBLANK(Assumptions[\"tax\", \"rate\"])",
            FunctionError::UnknownFunction("ISBLANK".into()),
        ),
        (
            "ISNA(Assumptions[\"tax\", \"rate\"])",
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
        // Selector-error branch: fails statically with UnknownFunction rather than returning #DIV/0!
        (
            "IF((1 / 0) > 0, UNKNOWN_FN(), 10)",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        // Nested invalid expression inside dormant branch
        (
            "IF(TRUE, 10, -(1 + UNKNOWN_FN()))",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        // Scenario 3: 2-arg IF and required `_` rejected; trailing optional `_` rejected on optional parameters
        (
            "IF(FALSE, 10)",
            FunctionError::TooFewArguments {
                function: "IF",
                expected: 3,
                found: 2,
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
            "IF(TRUE, 10, IF(FALSE, 1, _))",
            FunctionError::CannotOmitRequiredArgument {
                function: "IF",
                param: "value_if_false",
                position: 3,
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
            "YEARFRAC(45292, 45474, _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "YEARFRAC",
                param: "basis",
                position: 3,
            },
        ),
        // Scenario 4: Required-parameter omission takes diagnostic precedence
        (
            "IF(_, 10, _)",
            FunctionError::CannotOmitRequiredArgument {
                function: "IF",
                param: "logical_test",
                position: 1,
            },
        ),
    ];

    for (formula, expected_fn_err) in cases {
        let expr = Expr::parse(formula).expect("valid syntax");
        let err = eval_expr(&expr, &origin, &memo, &symbols, &registry).unwrap_err();
        let EvalError::Function { cell, source } = err else {
            panic!("expected EvalError::Function for '{formula}', got {err:?}");
        };
        assert_eq!(*cell, origin);
        assert_eq!(*source, expected_fn_err, "mismatch on formula: {formula}");
    }

    // Valid absent trailing optional and intermediate optional skip succeed in eval_expr
    let pmt_3arg = Expr::parse("PMT(0.05, 10, -1000)").expect("valid PMT(0.05, 10, -1000)");
    let pmt_default =
        Expr::parse("PMT(0.05, 10, -1000, 0, 0)").expect("valid PMT(0.05, 10, -1000, 0, 0)");
    assert_eq!(
        eval_expr(&pmt_3arg, &origin, &memo, &symbols, &registry).expect("eval PMT 3-arg"),
        eval_expr(&pmt_default, &origin, &memo, &symbols, &registry).expect("eval PMT default")
    );
    let pmt_skip = Expr::parse("PMT(0.05, 10, -1000, _, 1)").expect("valid PMT skip");
    let pmt_explicit = Expr::parse("PMT(0.05, 10, -1000, 0, 1)").expect("valid PMT explicit");
    assert_eq!(
        eval_expr(&pmt_skip, &origin, &memo, &symbols, &registry).expect("eval PMT skip"),
        eval_expr(&pmt_explicit, &origin, &memo, &symbols, &registry).expect("eval PMT explicit")
    );
}

#[test]
fn test_unevaluated_dependency_error() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "ebit", "2024E");
    let memo = IndexMap::new(); // Empty memo: referenced cell is missing

    let expr = Expr::parse("IncomeStatement[\"rev\", @col] + 10").expect("valid expr");
    let err = eval_expr(&expr, &origin, &memo, &symbols, &registry).unwrap_err();
    let EvalError::UnevaluatedDependency(target) = err else {
        panic!("expected UnevaluatedDependency, got {err:?}");
    };
    assert_eq!(*target, CellId::new("IncomeStatement", "rev", "2024E"));
}

#[test]
fn test_unary_operator_arithmetic_and_strict_rejection() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let mut memo = IndexMap::new();
    memo.insert(
        CellId::new("Assumptions", "flag", "active"),
        CellValue::Number(5.0),
    );
    memo.insert(
        CellId::new("Assumptions", "empty_row", "val"),
        CellValue::Empty,
    );

    // -(-5) = 5.0
    let expr1 = Expr::parse("-(-5)").expect("valid expr");
    assert_eq!(
        eval_expr(&expr1, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Number(5.0)
    );

    // Unary `+` is rejected at parse time
    assert!(Expr::parse("+10").is_err());

    // -TRUE and -"hello" fail in Stage A with InvalidOperandType
    for (formula, found) in [
        ("-TRUE", ValueType::Boolean),
        ("-\"hello\"", ValueType::Text),
    ] {
        let expr = Expr::parse(formula).expect("valid expr");
        let err = eval_expr(&expr, &origin, &memo, &symbols, &registry).unwrap_err();
        let EvalError::Type { cell, source } = err else {
            panic!("expected EvalError::Type for '{formula}', got {err:?}");
        };
        assert_eq!(*cell, origin);
        assert_eq!(
            source,
            TypeError::InvalidOperandType {
                operator: "-",
                expected: ValueType::Number,
                found,
            }
        );
    }

    // -Empty fails in Stage B with EmptyOperand
    let expr_empty = Expr::parse("-Assumptions[\"empty_row\", \"val\"]").expect("valid expr");
    let err = eval_expr(&expr_empty, &origin, &memo, &symbols, &registry).unwrap_err();
    assert!(matches!(
        err,
        EvalError::Type {
            source: TypeError::EmptyOperand { operator: "-" },
            ..
        }
    ));
}

#[test]
fn test_scenario_8_standalone_eval_expr_rejects_empty_root_and_empty_arguments() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let mut memo = IndexMap::new();

    memo.insert(
        CellId::new("Assumptions", "tax", "rate"),
        CellValue::Text("k_tax".into()),
    );
    memo.insert(
        CellId::new("Assumptions", "flag", "rate"),
        CellValue::Text("k_flag".into()),
    );
    memo.insert(
        CellId::new("Assumptions", "empty_row", "rate"),
        CellValue::Text("k_empty".into()),
    );
    memo.insert(
        CellId::new("Assumptions", "tax", "val"),
        CellValue::Number(0.25),
    );
    memo.insert(
        CellId::new("Assumptions", "flag", "val"),
        CellValue::Number(0.5),
    );
    memo.insert(
        CellId::new("Assumptions", "flag", "active"),
        CellValue::Number(5.0),
    );
    memo.insert(
        CellId::new("Assumptions", "empty_row", "val"),
        CellValue::Empty,
    );

    // Top-level bare reference to a Null cell is rejected with TypeError::EmptyFormulaResult in standalone eval_expr
    let root_empty = Expr::parse("Assumptions[\"empty_row\", \"val\"]").expect("valid expr");
    let root_err = eval_expr(&root_empty, &origin, &memo, &symbols, &registry).unwrap_err();
    assert!(
        root_err
            .to_string()
            .contains("formula cannot evaluate to an empty cell")
    );
    let EvalError::Type { cell, source } = root_err else {
        panic!("expected EvalError::Type on bare Null reference, got {root_err:?}");
    };
    assert_eq!(*cell, origin);
    assert_eq!(source, TypeError::EmptyFormulaResult);

    // Scalar, active branch, and selected lookup/index elements resolving to CellValue::Empty fail with TypeError::EmptyArgument
    for (formula, expected_fn, expected_param, expected_pos) in [
        (
            "ISERROR(Assumptions[\"empty_row\", \"val\"])",
            "ISERROR",
            "value",
            1,
        ),
        (
            "IF(TRUE, Assumptions[\"empty_row\", \"val\"], 1)",
            "IF",
            "value_if_true",
            2,
        ),
        (
            "IF(FALSE, 1, Assumptions[\"empty_row\", \"val\"])",
            "IF",
            "value_if_false",
            3,
        ),
        (
            "CHOOSE(1, Assumptions[\"empty_row\", \"val\"], 10)",
            "CHOOSE",
            "value1",
            2,
        ),
        (
            "IFERROR(Assumptions[\"empty_row\", \"val\"], 1)",
            "IFERROR",
            "value",
            1,
        ),
        (
            "IFERROR(1 / 0, Assumptions[\"empty_row\", \"val\"])",
            "IFERROR",
            "value_if_error",
            2,
        ),
        (
            "INDEX(Assumptions[\"tax\"..\"empty_row\", \"val\"], 3)",
            "INDEX",
            "array",
            1,
        ),
        (
            "INDEX(Assumptions[\"empty_row\", \"val\"], 1)",
            "INDEX",
            "array",
            1,
        ),
        (
            "XLOOKUP(\"k_empty\", Assumptions[\"tax\"..\"empty_row\", \"rate\"], Assumptions[\"tax\"..\"empty_row\", \"val\"])",
            "XLOOKUP",
            "return_array",
            3,
        ),
        (
            "XLOOKUP(\"missing\", Assumptions[\"tax\"..\"empty_row\", \"rate\"], Assumptions[\"tax\"..\"empty_row\", \"val\"], Assumptions[\"empty_row\", \"val\"])",
            "XLOOKUP",
            "if_not_found",
            4,
        ),
        (
            "XLOOKUP(\"k_empty\", Assumptions[\"empty_row\", \"val\"], Assumptions[\"empty_row\", \"rate\"])",
            "XLOOKUP",
            "lookup_array",
            2,
        ),
        (
            "XMATCH(\"k_empty\", Assumptions[\"empty_row\", \"val\"])",
            "XMATCH",
            "lookup_array",
            2,
        ),
    ] {
        let expr = Expr::parse(formula).expect("valid expr");
        let err = eval_expr(&expr, &origin, &memo, &symbols, &registry).unwrap_err();
        let EvalError::Type { cell, source } = err else {
            panic!("expected EvalError::Type on '{formula}', got {err:?}");
        };
        assert_eq!(*cell, origin);
        assert_eq!(
            source,
            TypeError::EmptyArgument {
                function: expected_fn,
                param: expected_param,
                position: expected_pos,
            },
            "expected EmptyArgument on '{formula}'"
        );
    }

    // Dormant branches referencing Null cells and non-Empty selections from sparse slices succeed
    for (formula, expected) in [
        (
            "IF(FALSE, Assumptions[\"empty_row\", \"val\"], 1)",
            CellValue::Number(1.0),
        ),
        (
            "CHOOSE(2, Assumptions[\"empty_row\", \"val\"], 10)",
            CellValue::Number(10.0),
        ),
        (
            "IFERROR(10, Assumptions[\"empty_row\", \"val\"])",
            CellValue::Number(10.0),
        ),
        (
            "XLOOKUP(\"k_tax\", Assumptions[\"tax\"..\"empty_row\", \"rate\"], Assumptions[\"tax\"..\"empty_row\", \"val\"], Assumptions[\"empty_row\", \"val\"])",
            CellValue::Number(0.25),
        ),
        (
            "INDEX(Assumptions[\"tax\"..\"empty_row\", \"val\"], 1)",
            CellValue::Number(0.25),
        ),
    ] {
        let expr = Expr::parse(formula).expect("valid expr");
        assert_eq!(
            eval_expr(&expr, &origin, &memo, &symbols, &registry).expect("eval ok"),
            expected,
            "unexpected result on '{formula}'"
        );
    }
}

#[test]
fn test_pass4_acceptance_criteria() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let mut memo = IndexMap::new();

    memo.insert(
        CellId::new("Assumptions", "tax", "rate"),
        CellValue::Number(20.0),
    );
    memo.insert(
        CellId::new("Assumptions", "flag", "rate"),
        CellValue::Number(10.0),
    );
    memo.insert(
        CellId::new("Assumptions", "empty_row", "rate"),
        CellValue::Number(30.0),
    );
    memo.insert(
        CellId::new("Assumptions", "tax", "val"),
        CellValue::Number(10.0),
    );
    memo.insert(
        CellId::new("Assumptions", "flag", "val"),
        CellValue::Number(2.0),
    );
    memo.insert(
        CellId::new("Assumptions", "flag", "active"),
        CellValue::Number(5.0),
    );
    memo.insert(
        CellId::new("Assumptions", "empty_row", "val"),
        CellValue::Empty,
    );

    // Scenario 1: Empty cell policy
    for (formula, expected_msg) in [
        (
            "Assumptions[\"empty_row\", \"val\"] + 1",
            "operand for '+' evaluated to an empty cell",
        ),
        (
            "-Assumptions[\"empty_row\", \"val\"]",
            "operand for '-' evaluated to an empty cell",
        ),
        (
            "Assumptions[\"empty_row\", \"val\"] = 0",
            "operand for '=' evaluated to an empty cell",
        ),
        (
            "ABS(Assumptions[\"empty_row\", \"val\"])",
            "argument for parameter 'number' at position 1 of 'ABS' evaluated to an empty cell",
        ),
        (
            "IF(Assumptions[\"empty_row\", \"val\"], 1, 0)",
            "argument for parameter 'logical_test' at position 1 of 'IF' evaluated to an empty cell",
        ),
        (
            "IFERROR(Assumptions[\"empty_row\", \"val\"], 1)",
            "argument for parameter 'value' at position 1 of 'IFERROR' evaluated to an empty cell",
        ),
        (
            "XMATCH(Assumptions[\"empty_row\", \"val\"], Assumptions[\"tax\"..\"flag\", \"rate\"])",
            "argument for parameter 'lookup_value' at position 1 of 'XMATCH' evaluated to an empty cell",
        ),
        (
            "SUM(Assumptions[\"empty_row\", \"val\"])",
            "argument for parameter 'number1' at position 1 of 'SUM' evaluated to an empty cell",
        ),
    ] {
        let expr = Expr::parse(formula).expect("valid expr");
        let err = eval_expr(&expr, &origin, &memo, &symbols, &registry).unwrap_err();
        assert!(
            err.to_string().contains(expected_msg),
            "expected '{expected_msg}' in error for '{formula}', got: {err}"
        );
    }

    assert_eq!(
        eval_expr(
            &Expr::parse("COUNT(Assumptions[\"tax\"..\"empty_row\", \"val\"])").unwrap(),
            &origin,
            &memo,
            &symbols,
            &registry
        )
        .unwrap(),
        CellValue::Number(2.0)
    );
    assert_eq!(
        eval_expr(
            &Expr::parse("SUM(Assumptions[\"tax\"..\"empty_row\", \"val\"])").unwrap(),
            &origin,
            &memo,
            &symbols,
            &registry
        )
        .unwrap(),
        CellValue::Number(12.0)
    );

    // Scenario 2: Simplified value types
    for (formula, expected_msg) in [
        (
            "\"x\" + 1",
            "invalid operand type for '+': expected number, found text",
        ),
        (
            "ABS(\"x\")",
            "invalid argument type for parameter 'number' at position 1 of 'ABS': expected number, found text",
        ),
        (
            "NOT(1)",
            "invalid argument type for parameter 'logical' at position 1 of 'NOT': expected boolean, found number",
        ),
        (
            "XMATCH(\"a\", Assumptions[\"tax\"..\"flag\", \"rate\"])",
            "invalid argument type for parameter 'lookup_array' at position 2 of 'XMATCH': expected text, found number",
        ),
    ] {
        let expr = Expr::parse(formula).expect("valid expr");
        let err = eval_expr(&expr, &origin, &memo, &symbols, &registry).unwrap_err();
        assert!(
            err.to_string().contains(expected_msg),
            "expected '{expected_msg}' in error for '{formula}', got: {err}"
        );
    }

    // Scenarios 3, 4, 5: Value-domain ExcelErrors
    for (formula, expected_val) in [
        ("ROUND(1.234, 1.5)", CellValue::Error(ExcelError::Num)),
        ("CHOOSE(1.5, 10, 20)", CellValue::Error(ExcelError::Num)),
        (
            "INDEX(Assumptions[\"tax\"..\"flag\", \"rate\"], 3)",
            CellValue::Error(ExcelError::Ref),
        ),
        ("CHOOSE(3, 10, 20)", CellValue::Error(ExcelError::Value)),
        (
            "XMATCH(10, Assumptions[\"tax\"..\"flag\", \"rate\"], 2)",
            CellValue::Error(ExcelError::Value),
        ),
        (
            "XLOOKUP(10, Assumptions[\"tax\"..\"flag\", \"rate\"], Assumptions[\"tax\"..\"flag\", \"val\"], 0, 0, Assumptions[\"flag\", \"active\"])",
            CellValue::Error(ExcelError::Value),
        ),
        (
            "XLOOKUP(10, Assumptions[\"tax\"..\"flag\", \"rate\"], Assumptions[\"tax\"..\"flag\", \"val\"], 0, 0, Assumptions[\"flag\", \"val\"])",
            CellValue::Error(ExcelError::Value),
        ),
        (
            "XMATCH(20, Assumptions[\"tax\"..\"empty_row\", \"rate\"], 0, 2)",
            CellValue::Error(ExcelError::Value),
        ),
        ("DATE(2026, 2, 30)", CellValue::Error(ExcelError::Num)),
        ("DATE(1899, 1, 1)", CellValue::Error(ExcelError::Num)),
        ("DATE(2026, 0, 15)", CellValue::Error(ExcelError::Num)),
        ("YEAR(-1)", CellValue::Error(ExcelError::Num)),
        ("EDATE(0, 1)", CellValue::Error(ExcelError::Num)),
        (
            "YEARFRAC(45000, 45365, 5)",
            CellValue::Error(ExcelError::Num),
        ),
        (
            "PMT(0.05, 10, -1000, 0, 2)",
            CellValue::Error(ExcelError::Num),
        ),
        ("IFERROR(DATE(2026, 2, 30), 0)", CellValue::Number(0.0)),
        (
            "IFERROR(INDEX(Assumptions[\"tax\"..\"flag\", \"rate\"], 3), -1)",
            CellValue::Number(-1.0),
        ),
    ] {
        let expr = Expr::parse(formula).expect("valid expr");
        assert_eq!(
            eval_expr(&expr, &origin, &memo, &symbols, &registry).expect("eval ok"),
            expected_val,
            "unexpected result for '{formula}'"
        );
    }
}
