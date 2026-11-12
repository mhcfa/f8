#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use std::sync::Arc;

use indexmap::IndexMap;

use f8_core::compiler::eval::{EvalError, Evaluator};
use f8_core::compiler::graph::DependencyGraph;
use f8_core::compiler::ir::{CellGrid, CellId, CellNode, CellPayload, SymbolTable, expand_grid};
use f8_core::compiler::{Compiler, CompilerError};
use f8_core::functions::error::FunctionError;
use f8_core::functions::logical::IfFunction;
use f8_core::functions::registry::FunctionRegistry;
use f8_core::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParameterDef,
};
use f8_core::model::id::{ColId, RowId, SheetId, TableId};
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{CellInput, ColumnSpec, RowSpec, TableSpec, ValueLiteral};
use f8_core::model::theme::ThemeSpec;
use f8_core::value::{CellValue, ExcelError};

fn create_table(table_id: &str, cols: &[&str], rows: Vec<RowSpec>) -> TableSpec {
    TableSpec {
        id: TableId::from(table_id),
        title: table_id.into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: cols.iter().map(|&c| ColumnSpec::new(c)).collect(),
        rows,
    }
}

#[test]
fn test_scenario_1_topological_evaluation_of_pure_literal_grid() {
    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");

    let mut row1_cells = IndexMap::new();
    row1_cells.insert(ColId::from("c1"), CellInput::number(42.0));
    row1_cells.insert(ColId::from("c2"), CellInput::string("f8"));
    row1_cells.insert(ColId::from("c3"), CellInput::boolean(true));
    row1_cells.insert(ColId::from("c4"), CellInput::null());

    let row1 = RowSpec::new("r1").with_cells(row1_cells);

    let table = create_table("LiteralTable", &["c1", "c2", "c3", "c4"], vec![row1]);
    symbols
        .register_table(sheet, &table)
        .expect("register table");

    let grid = expand_grid(&symbols, &[table]).expect("expand grid");
    let graph = DependencyGraph::from_grid(&grid, &symbols).expect("build graph");
    let registry = FunctionRegistry::standard();

    let evaluated =
        Evaluator::evaluate(&grid, &graph, &symbols, &registry).expect("evaluate model");

    assert_eq!(evaluated.len(), 4);
    assert!(!evaluated.is_empty());
    assert_eq!(evaluated.find_errors().len(), 0);

    let t = TableId::from("LiteralTable");
    let r1 = RowId::from("r1");
    assert_eq!(
        evaluated.get_value(&t, &r1, &ColId::from("c1")),
        Some(&CellValue::Number(42.0))
    );
    assert_eq!(
        evaluated.get_value(&t, &r1, &ColId::from("c2")),
        Some(&CellValue::Text("f8".into()))
    );
    assert_eq!(
        evaluated.get_value(&t, &r1, &ColId::from("c3")),
        Some(&CellValue::Boolean(true))
    );
    assert_eq!(
        evaluated.get_value(&t, &r1, &ColId::from("c4")),
        Some(&CellValue::Empty)
    );
}

#[test]
fn test_scenario_2_multi_table_topological_evaluation_with_cross_references() {
    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");

    // Table 1: Assumptions
    let mut tax_cells = IndexMap::new();
    tax_cells.insert(ColId::from("val"), CellInput::number(0.20));
    let tax_row = RowSpec::new("tax_rate").with_cells(tax_cells);

    let mut growth_cells = IndexMap::new();
    growth_cells.insert(ColId::from("val"), CellInput::number(0.10));
    let growth_row = RowSpec::new("growth_rate").with_cells(growth_cells);

    let assumptions_table = create_table("Assumptions", &["val"], vec![tax_row, growth_row]);
    symbols
        .register_table(sheet.clone(), &assumptions_table)
        .expect("register assumptions");

    // Table 2: IncomeStatement
    let mut rev_cells = IndexMap::new();
    rev_cells.insert(ColId::from("2023A"), CellInput::number(1000.0));
    rev_cells.insert(
        ColId::from("2024E"),
        CellInput::formula("IncomeStatement[\"revenue\", col.prev(1)] * (1 + Assumptions[\"growth_rate\", \"val\"])"),
    );
    rev_cells.insert(
        ColId::from("2025E"),
        CellInput::formula("IncomeStatement[\"revenue\", col.prev(1)] * (1 + Assumptions[\"growth_rate\", \"val\"])"),
    );

    let rev_row = RowSpec::new("revenue").with_cells(rev_cells);

    let cogs_row = RowSpec::new("cogs").with_expr("IncomeStatement[\"revenue\", @col] * 0.4");

    let gp_row = RowSpec::new("gross_profit")
        .with_expr("IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col]");

    let tax_is_row = RowSpec::new("tax")
        .with_expr("IncomeStatement[\"gross_profit\", @col] * Assumptions[\"tax_rate\", \"val\"]");

    let ni_row = RowSpec::new("net_income")
        .with_expr("IncomeStatement[\"gross_profit\", @col] - IncomeStatement[\"tax\", @col]");

    let is_table = create_table(
        "IncomeStatement",
        &["2023A", "2024E", "2025E"],
        vec![rev_row, cogs_row, gp_row, tax_is_row, ni_row],
    );
    symbols
        .register_table(sheet, &is_table)
        .expect("register income statement");

    let grid = expand_grid(&symbols, &[assumptions_table, is_table]).expect("expand grid");
    let graph = DependencyGraph::from_grid(&grid, &symbols).expect("build graph");
    let registry = FunctionRegistry::standard();

    let evaluated =
        Evaluator::evaluate(&grid, &graph, &symbols, &registry).expect("evaluate model");

    let is = TableId::from("IncomeStatement");
    let rev = RowId::from("revenue");
    let cogs = RowId::from("cogs");
    let gp = RowId::from("gross_profit");
    let tax = RowId::from("tax");
    let ni = RowId::from("net_income");

    let y23 = ColId::from("2023A");
    let y24 = ColId::from("2024E");
    let y25 = ColId::from("2025E");

    // 2023A: 1000 rev, 400 cogs, 600 gp, 120 tax, 480 ni
    assert_eq!(
        evaluated.get_value(&is, &rev, &y23),
        Some(&CellValue::Number(1000.0))
    );
    assert_eq!(
        evaluated.get_value(&is, &cogs, &y23),
        Some(&CellValue::Number(400.0))
    );
    assert_eq!(
        evaluated.get_value(&is, &gp, &y23),
        Some(&CellValue::Number(600.0))
    );
    assert_eq!(
        evaluated.get_value(&is, &tax, &y23),
        Some(&CellValue::Number(120.0))
    );
    assert_eq!(
        evaluated.get_value(&is, &ni, &y23),
        Some(&CellValue::Number(480.0))
    );

    // 2024E: 1100 rev, 440 cogs, 660 gp, 132 tax, 528 ni
    assert_eq!(
        evaluated.get_value(&is, &rev, &y24),
        Some(&CellValue::Number(1100.0))
    );
    assert_eq!(
        evaluated.get_value(&is, &cogs, &y24),
        Some(&CellValue::Number(440.0))
    );
    assert_eq!(
        evaluated.get_value(&is, &gp, &y24),
        Some(&CellValue::Number(660.0))
    );
    assert_eq!(
        evaluated.get_value(&is, &tax, &y24),
        Some(&CellValue::Number(132.0))
    );
    assert_eq!(
        evaluated.get_value(&is, &ni, &y24),
        Some(&CellValue::Number(528.0))
    );

    // 2025E: 1210 rev, 484 cogs, 726 gp, 145.2 tax, 580.8 ni
    assert_eq!(
        evaluated.get_value(&is, &rev, &y25),
        Some(&CellValue::Number(1210.0))
    );
    assert_eq!(
        evaluated.get_value(&is, &cogs, &y25),
        Some(&CellValue::Number(484.0))
    );
    assert_eq!(
        evaluated.get_value(&is, &gp, &y25),
        Some(&CellValue::Number(726.0))
    );

    let tax_val = evaluated
        .get_value(&is, &tax, &y25)
        .and_then(|v| v.as_number())
        .expect("tax is number");
    assert!((tax_val - 145.2).abs() < 1e-9);

    let ni_val = evaluated
        .get_value(&is, &ni, &y25)
        .and_then(|v| v.as_number())
        .expect("ni is number");
    assert!((ni_val - 580.8).abs() < 1e-9);

    assert_eq!(evaluated.find_errors().len(), 0);
}

#[test]
fn test_scenario_3_model_query_apis_and_column_ordering() {
    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");

    let mut rev_cells = IndexMap::new();
    rev_cells.insert(ColId::from("2021A"), CellInput::number(100.0));
    rev_cells.insert(ColId::from("2022A"), CellInput::number(200.0));
    rev_cells.insert(ColId::from("2023A"), CellInput::number(300.0));

    let rev_row = RowSpec::new("rev").with_cells(rev_cells);

    let table = create_table("Report", &["2021A", "2022A", "2023A"], vec![rev_row]);
    symbols
        .register_table(sheet, &table)
        .expect("register table");

    let grid = expand_grid(&symbols, &[table]).expect("expand grid");
    let graph = DependencyGraph::from_grid(&grid, &symbols).expect("build graph");
    let registry = FunctionRegistry::standard();

    let evaluated =
        Evaluator::evaluate(&grid, &graph, &symbols, &registry).expect("evaluate model");

    let t = TableId::from("Report");
    let r = RowId::from("rev");

    // 1. Verify get_row_values preserves declared column order
    let row_values = evaluated.get_row_values(&t, &r).expect("row exists");
    let ordered_cols: Vec<_> = row_values.keys().cloned().collect();
    assert_eq!(
        ordered_cols,
        vec![
            ColId::from("2021A"),
            ColId::from("2022A"),
            ColId::from("2023A")
        ]
    );
    assert_eq!(
        row_values.values().cloned().collect::<Vec<_>>(),
        vec![
            CellValue::Number(100.0),
            CellValue::Number(200.0),
            CellValue::Number(300.0)
        ]
    );

    // 2. Query non-existent table -> TableNotFound
    let invalid_table = TableId::from("NonExistent");
    match evaluated.get_row_values(&invalid_table, &r) {
        Err(EvalError::TableNotFound(id)) => assert_eq!(id, invalid_table),
        other => panic!("Expected TableNotFound, got {other:?}"),
    }

    // 3. Query non-existent row -> RowNotFound
    let invalid_row = RowId::from("ghost_row");
    match evaluated.get_row_values(&t, &invalid_row) {
        Err(EvalError::RowNotFound { table: tbl, row }) => {
            assert_eq!(tbl, t);
            assert_eq!(row, invalid_row);
        }
        other => panic!("Expected RowNotFound, got {other:?}"),
    }

    // 4. Accessors & iteration
    assert_eq!(evaluated.len(), 3);
    assert!(!evaluated.is_empty());
    assert_eq!(evaluated.symbol_table().len(), 1);
    assert_eq!(evaluated.values().len(), 3);

    let iterated: Vec<_> = evaluated.iter().collect();
    assert_eq!(iterated.len(), 3);

    let into_iterated: Vec<_> = (&evaluated).into_iter().collect();
    assert_eq!(into_iterated.len(), 3);
}

#[test]
fn test_scenario_4_error_propagation_and_domain_error_isolation() {
    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");

    let div_zero_row = RowSpec::new("div_zero").with_expr("10 / 0");
    let dependent_row = RowSpec::new("dependent").with_expr("ErrModel[\"div_zero\", @col] + 100");
    let value_err_row = RowSpec::new("val_err").with_expr("10 + \"not_a_num\"");
    let valid_row = RowSpec::new("valid").with_expr("50 * 2");

    let table = create_table(
        "ErrModel",
        &["c1"],
        vec![div_zero_row, dependent_row, value_err_row, valid_row],
    );
    symbols
        .register_table(sheet, &table)
        .expect("register table");

    let grid = expand_grid(&symbols, &[table]).expect("expand grid");
    let graph = DependencyGraph::from_grid(&grid, &symbols).expect("build graph");
    let registry = FunctionRegistry::standard();

    // Evaluation succeeds without halting; runtime Excel errors are stored as CellValue::Error
    let evaluated = Evaluator::evaluate(&grid, &graph, &symbols, &registry)
        .expect("evaluate model with errors");

    let t = TableId::from("ErrModel");
    let c1 = ColId::from("c1");

    assert_eq!(
        evaluated.get_value(&t, &RowId::from("div_zero"), &c1),
        Some(&CellValue::Error(ExcelError::Div0))
    );
    // Downstream cell bubbled the #DIV/0! error
    assert_eq!(
        evaluated.get_value(&t, &RowId::from("dependent"), &c1),
        Some(&CellValue::Error(ExcelError::Div0))
    );
    assert_eq!(
        evaluated.get_value(&t, &RowId::from("val_err"), &c1),
        Some(&CellValue::Error(ExcelError::Value))
    );
    assert_eq!(
        evaluated.get_value(&t, &RowId::from("valid"), &c1),
        Some(&CellValue::Number(100.0))
    );

    let errors = evaluated.find_errors();
    assert_eq!(errors.len(), 3);
    let error_cells: Vec<_> = errors.iter().map(|(cell, _)| cell.row.as_str()).collect();
    assert!(error_cells.contains(&"div_zero"));
    assert!(error_cells.contains(&"dependent"));
    assert!(error_cells.contains(&"val_err"));
}

#[test]
fn test_scenario_5_built_in_functions_in_model_context() {
    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");

    let mut row1_cells = IndexMap::new();
    row1_cells.insert(ColId::from("c1"), CellInput::number(10.0));
    row1_cells.insert(ColId::from("c2"), CellInput::number(20.0));
    let r1 = RowSpec::new("r1").with_cells(row1_cells);

    let mut row2_cells = IndexMap::new();
    row2_cells.insert(ColId::from("c1"), CellInput::number(-30.0));
    row2_cells.insert(ColId::from("c2"), CellInput::number(-40.0));
    let r2 = RowSpec::new("r2").with_cells(row2_cells);

    // Row using SUM across 2D range
    let sum_row = RowSpec::new("sum_row").with_expr("SUM(FuncTable[\"r1\"..\"r2\", @col])");

    // Row using ABS
    let abs_row = RowSpec::new("abs_row").with_expr("ABS(FuncTable[\"r2\", @col])");

    // Row using IF with short-circuit lazy branch
    let if_row = RowSpec::new("if_row").with_expr("IF(FuncTable[\"r1\", @col] > 15, 100, 1 / 0)");

    let table = create_table(
        "FuncTable",
        &["c1", "c2"],
        vec![r1, r2, sum_row, abs_row, if_row],
    );
    symbols
        .register_table(sheet, &table)
        .expect("register table");

    let grid = expand_grid(&symbols, &[table]).expect("expand grid");
    let graph = DependencyGraph::from_grid(&grid, &symbols).expect("build graph");
    let registry = FunctionRegistry::standard();

    let evaluated = Evaluator::evaluate(&grid, &graph, &symbols, &registry)
        .expect("evaluate model with functions");

    let t = TableId::from("FuncTable");
    let c1 = ColId::from("c1");
    let c2 = ColId::from("c2");

    // SUM(r1..r2): c1 = 10 + (-30) = -20; c2 = 20 + (-40) = -20
    assert_eq!(
        evaluated.get_value(&t, &RowId::from("sum_row"), &c1),
        Some(&CellValue::Number(-20.0))
    );
    assert_eq!(
        evaluated.get_value(&t, &RowId::from("sum_row"), &c2),
        Some(&CellValue::Number(-20.0))
    );

    // ABS(r2): c1 = 30; c2 = 40
    assert_eq!(
        evaluated.get_value(&t, &RowId::from("abs_row"), &c1),
        Some(&CellValue::Number(30.0))
    );
    assert_eq!(
        evaluated.get_value(&t, &RowId::from("abs_row"), &c2),
        Some(&CellValue::Number(40.0))
    );

    // IF(r1 > 15, 100, 1/0):
    // In c1: r1 is 10 (not > 15), so false branch 1/0 evaluates -> #DIV/0!
    assert_eq!(
        evaluated.get_value(&t, &RowId::from("if_row"), &c1),
        Some(&CellValue::Error(ExcelError::Div0))
    );
    // In c2: r1 is 20 (> 15), so true branch 100 evaluates, 1/0 is NEVER evaluated -> 100.0
    assert_eq!(
        evaluated.get_value(&t, &RowId::from("if_row"), &c2),
        Some(&CellValue::Number(100.0))
    );
}

#[test]
fn test_scenario_6_evaluator_missing_cell_guard() {
    let symbols = SymbolTable::new();
    let registry = FunctionRegistry::standard();

    let cell_present = CellId::new("T", "r1", "c1");
    let cell_missing = CellId::new("T", "r2", "c1");

    // Construct a grid containing both cells to build the graph
    let mut grid_full = CellGrid::new();
    grid_full
        .insert(CellNode::new(
            cell_present.clone(),
            SheetId::from("S"),
            CellPayload::Literal(ValueLiteral::Number(10.0)),
        ))
        .expect("insert cell 1");
    grid_full
        .insert(CellNode::new(
            cell_missing.clone(),
            SheetId::from("S"),
            CellPayload::Literal(ValueLiteral::Number(20.0)),
        ))
        .expect("insert cell 2");

    let graph =
        DependencyGraph::from_dependencies(&grid_full, &IndexMap::new()).expect("build graph");

    // Construct a grid that is missing the second cell
    let mut grid_partial = CellGrid::new();
    grid_partial
        .insert(CellNode::new(
            cell_present,
            SheetId::from("S"),
            CellPayload::Literal(ValueLiteral::Number(10.0)),
        ))
        .expect("insert cell 1");

    let result = Evaluator::evaluate(&grid_partial, &graph, &symbols, &registry);
    match result {
        Err(EvalError::CellNotFound(id)) => assert_eq!(*id, cell_missing),
        other => panic!("Expected CellNotFound, got {other:?}"),
    }
}

fn manifest_for_table(table_id: &str) -> ProjectManifest {
    ProjectManifest {
        model_name: "StaticValidationModel".into(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".into(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from(table_id)],
        }],
    }
}

fn assert_eval_function_err<T: std::fmt::Debug>(
    result: Result<T, EvalError>,
    expected_cell: &CellId,
    expected_err: &FunctionError,
) {
    if let Err(EvalError::Function { cell, source }) = result {
        assert_eq!(cell.as_ref(), expected_cell);
        assert_eq!(source.as_ref(), expected_err);
    } else {
        panic!("Expected EvalError::Function({expected_cell}, {expected_err:?}), got {result:?}");
    }
}

fn assert_compiler_function_err<T: std::fmt::Debug>(
    result: Result<T, CompilerError>,
    expected_cell: &CellId,
    expected_err: &FunctionError,
) {
    if let Err(CompilerError::Eval(eval_err)) = result {
        assert_eval_function_err::<()>(Err(eval_err), expected_cell, expected_err);
    } else {
        panic!(
            "Expected CompilerError::Eval(EvalError::Function({expected_cell}, {expected_err:?})), got {result:?}"
        );
    }
}

#[derive(Debug)]
struct CustomDoubleFunction;

static CUSTOM_DOUBLE_PARAMS: [ParameterDef; 1] = [ParameterDef::required(
    "value",
    "Numeric value to double",
    ParamKind::Scalar,
)];

impl ExcelFunction for CustomDoubleFunction {
    fn name(&self) -> &'static str {
        "CUSTOM_FN"
    }

    fn category(&self) -> FunctionCategory {
        FunctionCategory::Math
    }

    fn parameters(&self) -> &'static [ParameterDef] {
        &CUSTOM_DOUBLE_PARAMS
    }

    fn evaluate(
        &self,
        args: &[FunctionArg<'_>],
        ctx: &dyn EvalContext,
    ) -> Result<CellValue, FunctionError> {
        let val = args[0].eval_scalar(self.name(), ctx)?;
        match val {
            CellValue::Number(n) => Ok(CellValue::Number(n * 2.0)),
            CellValue::Error(e) => Ok(CellValue::Error(e)),
            CellValue::Text(_) | CellValue::Boolean(_) | CellValue::Empty => {
                Ok(CellValue::Error(ExcelError::Value))
            }
        }
    }
}

#[test]
fn test_scenario_7_static_validation_of_dormant_branches_and_iferror() {
    let cases: &[(&str, FunctionError)] = &[
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
            "IF(FALSE, 10, _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "IF",
                param: "value_if_false",
                position: 3,
            },
        ),
        (
            "IF(TRUE, 10, _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "IF",
                param: "value_if_false",
                position: 3,
            },
        ),
        (
            "IF(TRUE, 10, IF(FALSE, 1, _))",
            FunctionError::CannotOmitTrailingArgument {
                function: "IF",
                param: "value_if_false",
                position: 3,
            },
        ),
        (
            "INDEX(T[\"k1\"..\"k2\", @col], 2, _)",
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
            "IRR(T[\"k1\"..\"k2\", @col], _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "IRR",
                param: "guess",
                position: 2,
            },
        ),
        (
            "XIRR(T[\"v1\"..\"v2\", @col], T[\"k1\"..\"k2\", @col], _)",
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
            "XMATCH(1, T[\"k1\"..\"k2\", @col], _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "XMATCH",
                param: "match_mode",
                position: 3,
            },
        ),
        (
            "XLOOKUP(1, T[\"k1\"..\"k2\", @col], T[\"v1\"..\"v2\", @col], _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "XLOOKUP",
                param: "if_not_found",
                position: 4,
            },
        ),
        (
            "XLOOKUP(1, T[\"k1\"..\"k2\", @col], T[\"v1\"..\"v2\", @col], _, 0, _)",
            FunctionError::CannotOmitTrailingArgument {
                function: "XLOOKUP",
                param: "search_mode",
                position: 6,
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
            "IFS(TRUE, 1, FALSE, _)",
            FunctionError::CannotOmitRequiredArgument {
                function: "IFS",
                param: "value_if_true1",
                position: 4,
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
            "INDEX(T[\"k1\"..\"k2\", @col], _, 1)",
            FunctionError::CannotOmitRequiredArgument {
                function: "INDEX",
                param: "row_num",
                position: 2,
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
            "IFERROR(ABS(1, 2, 3), 0)",
            FunctionError::TooManyArguments {
                function: "ABS",
                expected: 1,
                found: 3,
            },
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
            "XLOOKUP(1, T[\"k1\"..\"k2\", @col], T[\"v1\"..\"v2\", @col], UNKNOWN_FN())",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        (
            "IF(1 / 0, UNKNOWN_FN(), 10)",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
        (
            "IF(TRUE, 10, -(1 + UNKNOWN_FN()))",
            FunctionError::UnknownFunction("UNKNOWN_FN".into()),
        ),
    ];

    let manifest = manifest_for_table("T");
    let registry = FunctionRegistry::standard();
    let target_cell = CellId::new("T", "target", "c1");

    for (formula, expected_err) in cases {
        let k1 = RowSpec::new("k1").with_cells(IndexMap::from([(
            ColId::from("c1"),
            CellInput::number(1.0),
        )]));
        let k2 = RowSpec::new("k2").with_cells(IndexMap::from([(
            ColId::from("c1"),
            CellInput::number(2.0),
        )]));
        let v1 = RowSpec::new("v1").with_cells(IndexMap::from([(
            ColId::from("c1"),
            CellInput::number(100.0),
        )]));
        let v2 = RowSpec::new("v2").with_cells(IndexMap::from([(
            ColId::from("c1"),
            CellInput::number(200.0),
        )]));
        let target = RowSpec::new("target").with_expr(*formula);

        let table = create_table("T", &["c1"], vec![k1, k2, v1, v2, target]);
        let tables = [table.clone()];

        let mut symbols = SymbolTable::new();
        symbols
            .register_table(SheetId::from("Sheet1"), &table)
            .expect("register table");
        let grid = expand_grid(&symbols, &tables).expect("expand grid");
        let graph = DependencyGraph::from_grid(&grid, &symbols).expect("build graph");

        assert_eval_function_err(
            Evaluator::validate_grid(&grid, &registry),
            &target_cell,
            expected_err,
        );
        assert_eval_function_err(
            Evaluator::evaluate(&grid, &graph, &symbols, &registry),
            &target_cell,
            expected_err,
        );
        assert_compiler_function_err(
            Compiler::evaluate(&manifest, &tables),
            &target_cell,
            expected_err,
        );
        assert_compiler_function_err(
            Compiler::compile(&manifest, &tables),
            &target_cell,
            expected_err,
        );
    }

    // Valid IFERROR trapping an Excel domain error still succeeds and evaluates to fallback
    let ok_table = create_table(
        "T",
        &["c1"],
        vec![RowSpec::new("target").with_expr("IFERROR(10 / 0, 42)")],
    );
    let evaluated = Compiler::evaluate(&manifest, &[ok_table]).expect("evaluate valid IFERROR");
    assert_eq!(
        evaluated.get_value(
            &TableId::from("T"),
            &RowId::from("target"),
            &ColId::from("c1"),
        ),
        Some(&CellValue::Number(42.0))
    );
}

#[test]
fn test_scenario_8_validate_grid_reports_in_declaration_order() {
    // r1 is declared first in CellGrid, but depends on r2 so topological order evaluates r2 before r1.
    let r1 = RowSpec::new("r1").with_expr("T[\"r2\", @col] + IF(TRUE, 1, UNKNOWN_FIRST())");
    let r2 = RowSpec::new("r2").with_expr("IF(TRUE, 1, UNKNOWN_SECOND())");
    let table = create_table("T", &["c1"], vec![r1, r2]);
    let tables = [table.clone()];
    let manifest = manifest_for_table("T");

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Sheet1"), &table)
        .expect("register table");
    let grid = expand_grid(&symbols, &tables).expect("expand grid");
    let graph = DependencyGraph::from_grid(&grid, &symbols).expect("build graph");

    // Confirm topological order puts r2 before r1, while grid declaration order puts r1 before r2
    let r1_cell = CellId::new("T", "r1", "c1");
    let r2_cell = CellId::new("T", "r2", "c1");
    let grid_order: Vec<CellId> = grid.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(grid_order, vec![r1_cell.clone(), r2_cell.clone()]);
    assert_eq!(graph.topological_order(), &[r2_cell, r1_cell.clone()]);

    let registry = FunctionRegistry::standard();
    let expected_err = FunctionError::UnknownFunction("UNKNOWN_FIRST".into());

    assert_eval_function_err(
        Evaluator::validate_grid(&grid, &registry),
        &r1_cell,
        &expected_err,
    );
    assert_eval_function_err(
        Evaluator::evaluate(&grid, &graph, &symbols, &registry),
        &r1_cell,
        &expected_err,
    );
    assert_compiler_function_err(
        Compiler::compile(&manifest, &tables),
        &r1_cell,
        &expected_err,
    );
}

#[test]
fn test_scenario_9_custom_function_registry_across_active_and_dormant_branches() {
    let manifest = manifest_for_table("T");
    let r1_cell = CellId::new("T", "r1", "c1");

    // 1. Empty registry rejects even built-in IF
    let empty_registry = FunctionRegistry::new();
    let if_table = create_table(
        "T",
        &["c1"],
        vec![RowSpec::new("r1").with_expr("IF(TRUE, 10, 20)")],
    );
    let expected_missing_if = FunctionError::UnknownFunction("IF".into());
    assert_compiler_function_err(
        Compiler::evaluate_with_registry(
            &manifest,
            std::slice::from_ref(&if_table),
            &empty_registry,
        ),
        &r1_cell,
        &expected_missing_if,
    );

    // 2. Custom registry with IF + CUSTOM_FN (without SUM)
    let mut custom_registry = FunctionRegistry::new();
    custom_registry.register(Arc::new(IfFunction));
    custom_registry.register(Arc::new(CustomDoubleFunction));

    // Dormant CUSTOM_FN(1) and active CUSTOM_FN(21) succeed
    let custom_ok_table = create_table(
        "T",
        &["c1"],
        vec![
            RowSpec::new("r1").with_expr("IF(TRUE, 10, CUSTOM_FN(1))"),
            RowSpec::new("r2").with_expr("CUSTOM_FN(21)"),
        ],
    );
    let evaluated = Compiler::evaluate_with_registry(
        &manifest,
        std::slice::from_ref(&custom_ok_table),
        &custom_registry,
    )
    .expect("evaluate with custom registry");
    assert_eq!(
        evaluated.get_value(&TableId::from("T"), &RowId::from("r1"), &ColId::from("c1"),),
        Some(&CellValue::Number(10.0))
    );
    assert_eq!(
        evaluated.get_value(&TableId::from("T"), &RowId::from("r2"), &ColId::from("c1"),),
        Some(&CellValue::Number(42.0))
    );

    let compiled = Compiler::compile_with_registry(
        &manifest,
        std::slice::from_ref(&custom_ok_table),
        &custom_registry,
    )
    .expect("compile with custom registry");
    assert_eq!(
        compiled.lowered_map.get(&r1_cell),
        Some("=IF(TRUE,10,CUSTOM_FN(1))")
    );

    // Unregistered SUM(1) in dormant branch fails with UnknownFunction("SUM")
    let unregistered_sum_table = create_table(
        "T",
        &["c1"],
        vec![RowSpec::new("r1").with_expr("IF(TRUE, 10, SUM(1))")],
    );
    let expected_missing_sum = FunctionError::UnknownFunction("SUM".into());
    assert_compiler_function_err(
        Compiler::evaluate_with_registry(
            &manifest,
            std::slice::from_ref(&unregistered_sum_table),
            &custom_registry,
        ),
        &r1_cell,
        &expected_missing_sum,
    );
    assert_compiler_function_err(
        Compiler::compile_with_registry(
            &manifest,
            std::slice::from_ref(&unregistered_sum_table),
            &custom_registry,
        ),
        &r1_cell,
        &expected_missing_sum,
    );
}

#[test]
fn test_scenario_10_nested_function_calls_validate_ast_once() {
    #[derive(Debug, Default)]
    struct ValidationCountFunction {
        min_args_checks: std::sync::atomic::AtomicUsize,
    }

    impl ExcelFunction for ValidationCountFunction {
        fn name(&self) -> &'static str {
            "COUNTED_FN"
        }

        fn category(&self) -> FunctionCategory {
            FunctionCategory::Math
        }

        fn parameters(&self) -> &'static [ParameterDef] {
            &CUSTOM_DOUBLE_PARAMS
        }

        fn min_args(&self) -> usize {
            self.min_args_checks
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            1
        }

        fn evaluate(
            &self,
            args: &[FunctionArg<'_>],
            ctx: &dyn EvalContext,
        ) -> Result<CellValue, FunctionError> {
            CustomDoubleFunction.evaluate(args, ctx)
        }
    }

    let counted = Arc::new(ValidationCountFunction::default());
    let mut registry = FunctionRegistry::new();
    registry.register(Arc::new(IfFunction));
    registry.register(counted.clone());

    let manifest = manifest_for_table("T");
    let nested_formula = "IF(TRUE, IF(TRUE, IF(TRUE, 10, COUNTED_FN(1)), 0), 0)";
    let table = create_table(
        "T",
        &["c1"],
        vec![RowSpec::new("r1").with_expr(nested_formula)],
    );

    let evaluated = Compiler::evaluate_with_registry(&manifest, &[table], &registry)
        .expect("evaluate nested IF with dormant COUNTED_FN");
    assert_eq!(
        evaluated.get_value(&TableId::from("T"), &RowId::from("r1"), &ColId::from("c1")),
        Some(&CellValue::Number(10.0))
    );
    // Dormant COUNTED_FN(1) is never evaluated; static validation must inspect it once at entry,
    // not re-walk subtrees at each outer IF level.
    assert_eq!(
        counted
            .min_args_checks
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );

    counted
        .min_args_checks
        .store(0, std::sync::atomic::Ordering::Relaxed);
    let expr = f8_core::expr::Expr::parse(nested_formula).expect("parse nested formula");
    let symbols = SymbolTable::new();
    let memo = IndexMap::new();
    let origin = CellId::new("T", "r1", "c1");
    let val = f8_core::compiler::eval_expr(&expr, &origin, &memo, &symbols, &registry)
        .expect("eval_expr nested IF");
    assert_eq!(val, CellValue::Number(10.0));
    assert_eq!(
        counted
            .min_args_checks
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
}

#[test]
fn test_scenario_11_positional_omission_end_to_end_evaluation_and_lowering() {
    let manifest = manifest_for_table("T");

    let row_vec = RowSpec::new("row_vec").with_cells(IndexMap::from([
        (ColId::from("c1"), CellInput::number(10.0)),
        (ColId::from("c2"), CellInput::number(20.0)),
        (ColId::from("c3"), CellInput::number(30.0)),
    ]));
    let k1 = RowSpec::new("k1").with_cells(IndexMap::from([
        (ColId::from("c1"), CellInput::string("A")),
        (ColId::from("c2"), CellInput::number(100.0)),
        (ColId::from("c3"), CellInput::number(0.0)),
    ]));
    let k2 = RowSpec::new("k2").with_cells(IndexMap::from([
        (ColId::from("c1"), CellInput::string("B")),
        (ColId::from("c2"), CellInput::number(200.0)),
        (ColId::from("c3"), CellInput::number(0.0)),
    ]));
    let k3 = RowSpec::new("k3").with_cells(IndexMap::from([
        (ColId::from("c1"), CellInput::string("A")),
        (ColId::from("c2"), CellInput::number(300.0)),
        (ColId::from("c3"), CellInput::number(0.0)),
    ]));

    let pmt_skip = RowSpec::new("pmt_skip").with_expr("PMT(0.05, 10, -1000, _, 1)");
    let pmt_explicit = RowSpec::new("pmt_explicit").with_expr("PMT(0.05, 10, -1000, 0, 1)");
    let ipmt_skip = RowSpec::new("ipmt_skip").with_expr("IPMT(0.05, 1, 10, -1000, _, 1)");
    let ipmt_explicit = RowSpec::new("ipmt_explicit").with_expr("IPMT(0.05, 1, 10, -1000, 0, 1)");
    let ppmt_skip = RowSpec::new("ppmt_skip").with_expr("PPMT(0.05, 1, 10, -1000, _, 1)");
    let ppmt_explicit = RowSpec::new("ppmt_explicit").with_expr("PPMT(0.05, 1, 10, -1000, 0, 1)");
    let xmatch_skip =
        RowSpec::new("xmatch_skip").with_expr("XMATCH(\"A\", T[\"k1\"..\"k3\", \"c1\"], _, -1)");
    let xlookup_1skip = RowSpec::new("xlookup_1skip").with_expr(
        "XLOOKUP(\"A\", T[\"k1\"..\"k3\", \"c1\"], T[\"k1\"..\"k3\", \"c2\"], _, 0, -1)",
    );
    let xlookup_2skip = RowSpec::new("xlookup_2skip").with_expr(
        "XLOOKUP(\"A\", T[\"k1\"..\"k3\", \"c1\"], T[\"k1\"..\"k3\", \"c2\"], _, _, -1)",
    );
    let if_2arg = RowSpec::new("if_2arg").with_expr("IF(FALSE, 10)");
    let index_2arg =
        RowSpec::new("index_2arg").with_expr("INDEX(T[\"row_vec\", \"c1\"..\"c3\"], 2)");

    let table = TableSpec {
        id: TableId::from("T"),
        title: "T".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![
            ColumnSpec::gutter(),
            ColumnSpec::new("c1").with_header("C1"),
            ColumnSpec::new("c2").with_header("C2"),
            ColumnSpec::new("c3").with_header("C3"),
        ],
        rows: vec![
            row_vec,
            k1,
            k2,
            k3,
            pmt_skip,
            pmt_explicit,
            ipmt_skip,
            ipmt_explicit,
            ppmt_skip,
            ppmt_explicit,
            xmatch_skip,
            xlookup_1skip,
            xlookup_2skip,
            if_2arg,
            index_2arg,
        ],
    };

    let compiled = Compiler::compile(&manifest, &[table]).expect("compile omission scenarios");
    let t = TableId::from("T");
    let c1 = ColId::from("c1");

    // Scenario 1: Pass 4 values match explicit defaults
    assert_eq!(
        compiled
            .evaluated
            .get_value(&t, &RowId::from("pmt_skip"), &c1),
        compiled
            .evaluated
            .get_value(&t, &RowId::from("pmt_explicit"), &c1),
    );
    assert_eq!(
        compiled
            .evaluated
            .get_value(&t, &RowId::from("ipmt_skip"), &c1),
        compiled
            .evaluated
            .get_value(&t, &RowId::from("ipmt_explicit"), &c1),
    );
    assert_eq!(
        compiled
            .evaluated
            .get_value(&t, &RowId::from("ppmt_skip"), &c1),
        compiled
            .evaluated
            .get_value(&t, &RowId::from("ppmt_explicit"), &c1),
    );
    assert_eq!(
        compiled
            .evaluated
            .get_value(&t, &RowId::from("xmatch_skip"), &c1),
        Some(&CellValue::Number(3.0)),
    );
    assert_eq!(
        compiled
            .evaluated
            .get_value(&t, &RowId::from("xlookup_1skip"), &c1),
        Some(&CellValue::Number(300.0)),
    );
    assert_eq!(
        compiled
            .evaluated
            .get_value(&t, &RowId::from("xlookup_2skip"), &c1),
        Some(&CellValue::Number(300.0)),
    );

    // Scenario 1: Pass 6a lowered formulas emit intermediate `,,` / `,,,` slots
    assert_eq!(
        compiled
            .lowered_map
            .get(&CellId::new("T", "pmt_skip", "c1")),
        Some("=PMT(0.05,10,-1000,,1)"),
    );
    assert_eq!(
        compiled
            .lowered_map
            .get(&CellId::new("T", "ipmt_skip", "c1")),
        Some("=IPMT(0.05,1,10,-1000,,1)"),
    );
    assert_eq!(
        compiled
            .lowered_map
            .get(&CellId::new("T", "ppmt_skip", "c1")),
        Some("=PPMT(0.05,1,10,-1000,,1)"),
    );
    assert_eq!(
        compiled
            .lowered_map
            .get(&CellId::new("T", "xmatch_skip", "c1")),
        Some("=XMATCH(\"A\",$C$6:$C$8,,-1)"),
    );
    assert_eq!(
        compiled
            .lowered_map
            .get(&CellId::new("T", "xlookup_1skip", "c1")),
        Some("=XLOOKUP(\"A\",$C$6:$C$8,$D$6:$D$8,,0,-1)"),
    );
    assert_eq!(
        compiled
            .lowered_map
            .get(&CellId::new("T", "xlookup_2skip", "c1")),
        Some("=XLOOKUP(\"A\",$C$6:$C$8,$D$6:$D$8,,,-1)"),
    );

    // Scenario 2: Absent trailing optional arguments evaluate and lower without synthetic padding
    assert_eq!(
        compiled
            .evaluated
            .get_value(&t, &RowId::from("if_2arg"), &c1),
        Some(&CellValue::Boolean(false)),
    );
    assert_eq!(
        compiled.lowered_map.get(&CellId::new("T", "if_2arg", "c1")),
        Some("=IF(FALSE,10)"),
    );
    assert_eq!(
        compiled
            .evaluated
            .get_value(&t, &RowId::from("index_2arg"), &c1),
        Some(&CellValue::Number(20.0)),
    );
    assert_eq!(
        compiled
            .lowered_map
            .get(&CellId::new("T", "index_2arg", "c1")),
        Some("=INDEX($C$5:$E$5,2)"),
    );
}
