#![allow(clippy::unwrap_used, clippy::expect_used)]

use indexmap::IndexMap;

use f8_core::compiler::eval::{EvalError, eval_expr};
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::ir::symbol::SymbolTable;
use f8_core::expr::ast::Expr;
use f8_core::functions::registry::FunctionRegistry;
use f8_core::model::id::{SheetId, TableId};
use f8_core::model::table::{ColumnSpec, RowSpec, TableSpec};
use f8_core::value::{CellValue, ExcelError};

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
fn test_scenario_1_formula_arithmetic_and_excel_parity_coercion() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let mut memo = IndexMap::new();

    let cell_empty = CellId::new("Assumptions", "empty_row", "val");
    memo.insert(cell_empty, CellValue::Empty);

    // 10 + 20 * 2 = 50.0
    let expr1 = Expr::parse("10 + 20 * 2").expect("valid expr");
    let val1 = eval_expr(&expr1, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val1, CellValue::Number(50.0));

    // Empty + 5 = 5.0
    let expr2 = Expr::parse("Assumptions[\"empty_row\", \"val\"] + 5").expect("valid expr");
    let val2 = eval_expr(&expr2, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val2, CellValue::Number(5.0));

    // TRUE * 10 = 10.0
    let expr3 = Expr::parse("TRUE * 10").expect("valid expr");
    let val3 = eval_expr(&expr3, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val3, CellValue::Number(10.0));

    // FALSE + 2 = 2.0
    let expr4 = Expr::parse("FALSE + 2").expect("valid expr");
    let val4 = eval_expr(&expr4, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val4, CellValue::Number(2.0));
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

    // 10 / Empty -> #DIV/0!
    let expr_div_empty =
        Expr::parse("10 / Assumptions[\"empty_row\", \"val\"]").expect("valid expr");
    let val = eval_expr(&expr_div_empty, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val, CellValue::Error(ExcelError::Div0));

    // 0 ^ -1 -> #DIV/0!
    let expr_pow_div0 = Expr::parse("0 ^ -1").expect("valid expr");
    let val = eval_expr(&expr_pow_div0, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val, CellValue::Error(ExcelError::Div0));

    // (-2) ^ 0.5 -> #NUM!
    let expr_complex_pow = Expr::parse("(-2) ^ 0.5").expect("valid expr");
    let val = eval_expr(&expr_complex_pow, &origin, &memo, &symbols, &registry).expect("eval ok");
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

    // Downstream cells bubble errors
    let cell_err = CellId::new("IncomeStatement", "cogs", "2024E");
    memo.insert(cell_err, CellValue::Error(ExcelError::Div0));
    let expr_bubble = Expr::parse("IncomeStatement[\"cogs\", @col] + 100").expect("valid expr");
    let val = eval_expr(&expr_bubble, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val, CellValue::Error(ExcelError::Div0));
}

#[test]
fn test_scenario_3_comparison_operators_and_excel_type_ranking() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let mut memo = IndexMap::new();

    let cell_empty = CellId::new("Assumptions", "empty_row", "val");
    memo.insert(cell_empty, CellValue::Empty);

    // 10 < "2" -> Number < Text -> TRUE
    let expr1 = Expr::parse("10 < \"2\"").expect("valid expr");
    assert_eq!(
        eval_expr(&expr1, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Boolean(true)
    );

    // "Apple" = "apple" -> Case-insensitive -> TRUE
    let expr2 = Expr::parse("\"Apple\" = \"apple\"").expect("valid expr");
    assert_eq!(
        eval_expr(&expr2, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Boolean(true)
    );

    // 100 < TRUE -> Number < Boolean -> TRUE
    let expr3 = Expr::parse("100 < TRUE").expect("valid expr");
    assert_eq!(
        eval_expr(&expr3, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Boolean(true)
    );

    // Empty = 0 -> Empty coerces to 0.0 -> TRUE
    let expr4 = Expr::parse("Assumptions[\"empty_row\", \"val\"] = 0").expect("valid expr");
    assert_eq!(
        eval_expr(&expr4, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Boolean(true)
    );

    // Empty = "" -> Empty coerces to "" -> TRUE
    let expr5 = Expr::parse("Assumptions[\"empty_row\", \"val\"] = \"\"").expect("valid expr");
    assert_eq!(
        eval_expr(&expr5, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Boolean(true)
    );

    // Empty = FALSE -> FALSE (Empty ranks lower than Boolean)
    let expr6 = Expr::parse("Assumptions[\"empty_row\", \"val\"] = FALSE").expect("valid expr");
    assert_eq!(
        eval_expr(&expr6, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Boolean(false)
    );

    // Empty < FALSE -> TRUE
    let expr7 = Expr::parse("Assumptions[\"empty_row\", \"val\"] < FALSE").expect("valid expr");
    assert_eq!(
        eval_expr(&expr7, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Boolean(true)
    );

    // Empty <> FALSE -> TRUE
    let expr8 = Expr::parse("Assumptions[\"empty_row\", \"val\"] <> FALSE").expect("valid expr");
    assert_eq!(
        eval_expr(&expr8, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Boolean(true)
    );
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
fn test_scenario_5_multi_cell_reference_in_scalar_position_rejection() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let memo = IndexMap::new();

    // Multi-cell row range in scalar arithmetic: IncomeStatement["cogs".."sga", @col] + 10 -> #VALUE!
    let expr = Expr::parse("IncomeStatement[\"cogs\"..\"sga\", @col] + 10").expect("valid expr");
    let val = eval_expr(&expr, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val, CellValue::Error(ExcelError::Value));
}

#[test]
fn test_scenario_6_2d_range_resolution_and_aggregation_sum() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "ebit", "2024E");
    let mut memo = IndexMap::new();

    // Cells for SUM range:
    // rev = 100.0
    // cogs = "TextString" (should be ignored by SUM range reference)
    // sga = Empty (should be ignored by SUM range reference)
    memo.insert(
        CellId::new("IncomeStatement", "rev", "2024E"),
        CellValue::Number(100.0),
    );
    memo.insert(
        CellId::new("IncomeStatement", "cogs", "2024E"),
        CellValue::Text("CostNote".into()),
    );
    memo.insert(
        CellId::new("IncomeStatement", "sga", "2024E"),
        CellValue::Empty,
    );

    let expr = Expr::parse("SUM(IncomeStatement[\"rev\"..\"sga\", @col])").expect("valid expr");
    let val = eval_expr(&expr, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val, CellValue::Number(100.0));

    // Also test adding another numeric cell
    memo.insert(
        CellId::new("IncomeStatement", "sga", "2024E"),
        CellValue::Number(30.0),
    );
    let val2 = eval_expr(&expr, &origin, &memo, &symbols, &registry).expect("eval ok");
    assert_eq!(val2, CellValue::Number(130.0));
}

#[test]
fn test_scenario_7_standalone_omitted_argument_rejection() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let memo = IndexMap::new();

    // Standalone _ in AST arithmetic: 1 + _
    let expr1 = Expr::Binary {
        op: f8_core::expr::ast::BinaryOp::Add,
        left: Box::new(Expr::number(1.0)),
        right: Box::new(Expr::Omitted),
    };
    let err1 = eval_expr(&expr1, &origin, &memo, &symbols, &registry).unwrap_err();
    match err1 {
        EvalError::UnexpectedOmitted(cell) => assert_eq!(*cell, origin),
        other => panic!("expected UnexpectedOmitted, got {other:?}"),
    }

    // Standalone _
    let expr2 = Expr::Omitted;
    let err2 = eval_expr(&expr2, &origin, &memo, &symbols, &registry).unwrap_err();
    match err2 {
        EvalError::UnexpectedOmitted(cell) => assert_eq!(*cell, origin),
        other => panic!("expected UnexpectedOmitted, got {other:?}"),
    }
}

#[test]
fn test_unevaluated_dependency_error() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "ebit", "2024E");
    let memo = IndexMap::new(); // Empty memo: referenced cell is missing

    let expr = Expr::parse("IncomeStatement[\"rev\", @col] + 10").expect("valid expr");
    let err = eval_expr(&expr, &origin, &memo, &symbols, &registry).unwrap_err();
    match err {
        EvalError::UnevaluatedDependency(target) => {
            assert_eq!(*target, CellId::new("IncomeStatement", "rev", "2024E"));
        }
        other => panic!("expected UnevaluatedDependency, got {other:?}"),
    }
}

#[test]
fn test_unary_operator_arithmetic_and_parity() {
    let (symbols, registry) = build_test_environment();
    let origin = CellId::new("IncomeStatement", "rev", "2024E");
    let memo = IndexMap::new();

    // -(-5) = 5.0
    let expr1 = Expr::parse("-(-5)").expect("valid expr");
    assert_eq!(
        eval_expr(&expr1, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Number(5.0)
    );

    // +10 = 10.0
    let expr2 = Expr::parse("+10").expect("valid expr");
    assert_eq!(
        eval_expr(&expr2, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Number(10.0)
    );

    // -TRUE = -1.0
    let expr3 = Expr::parse("-TRUE").expect("valid expr");
    assert_eq!(
        eval_expr(&expr3, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Number(-1.0)
    );

    // -"text" = #VALUE!
    let expr4 = Expr::parse("-\"hello\"").expect("valid expr");
    assert_eq!(
        eval_expr(&expr4, &origin, &memo, &symbols, &registry).expect("eval ok"),
        CellValue::Error(ExcelError::Value)
    );
}
