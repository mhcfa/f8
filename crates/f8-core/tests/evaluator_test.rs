#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use std::sync::Arc;

use indexmap::IndexMap;

use f8_core::compiler::eval::{EvalError, Evaluator};
use f8_core::compiler::graph::DependencyGraph;
use f8_core::compiler::ir::{CellGrid, CellId, CellNode, CellPayload, SymbolTable, expand_grid};
use f8_core::compiler::{Compiler, CompilerError, ErrorDiagnostic};
use f8_core::functions::error::FunctionError;
use f8_core::functions::logical::IfFunction;
use f8_core::functions::registry::FunctionRegistry;
use f8_core::functions::traits::{
    EvalContext, ExcelFunction, FunctionArg, FunctionCategory, ParamKind, ParamType, ParameterDef,
};
use f8_core::model::id::{ColId, RowId, SheetId, TableId};
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{CellInput, ColumnSpec, RowSpec, TableSpec, ValueLiteral};
use f8_core::model::theme::ThemeSpec;
use f8_core::value::{CellValue, ExcelError, ExpectedType, TypeError, ValueType};

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

    let oob_idx_row = RowSpec::new("oob_idx").with_cells(IndexMap::from([(
        ColId::from("c1"),
        CellInput::number(99.0),
    )]));
    let div_zero_row = RowSpec::new("div_zero").with_expr("10 / 0");
    let dependent_row = RowSpec::new("dependent").with_expr("ErrModel[\"div_zero\", @col] + 100");
    let value_err_row =
        RowSpec::new("val_err").with_expr("CHOOSE(ErrModel[\"oob_idx\", @col], 10, 20)");
    let valid_row = RowSpec::new("valid").with_expr("50 * 2");

    let table = create_table(
        "ErrModel",
        &["c1"],
        vec![
            oob_idx_row,
            div_zero_row,
            dependent_row,
            value_err_row,
            valid_row,
        ],
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
    ParamType::Number,
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

    fn return_type(&self) -> Option<ValueType> {
        Some(ValueType::Number)
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
            "IF(TRUE, 10, _)",
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
            "SWITCH(1, 1, 10, 20)",
            FunctionError::UnknownFunction("SWITCH".into()),
        ),
        (
            "IF(FALSE, SWITCH(1, 1, 10, 20), 0)",
            FunctionError::UnknownFunction("SWITCH".into()),
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
            Evaluator::validate_grid(&grid, &symbols, &registry),
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
        Evaluator::validate_grid(&grid, &symbols, &registry),
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

        fn return_type(&self) -> Option<ValueType> {
            Some(ValueType::Number)
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
    let xmatch_2arg =
        RowSpec::new("xmatch_2arg").with_expr("XMATCH(\"B\", T[\"k1\"..\"k3\", \"c1\"])");
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
            xmatch_2arg,
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
            .get_value(&t, &RowId::from("xmatch_2arg"), &c1),
        Some(&CellValue::Number(2.0)),
    );
    assert_eq!(
        compiled
            .lowered_map
            .get(&CellId::new("T", "xmatch_2arg", "c1")),
        Some("=XMATCH(\"B\",$C$6:$C$8)"),
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

#[test]
fn test_scenario_12_empty_formula_result_rejected_and_null_literal_invariant() {
    use f8_core::model::format::FormatPreset;

    let manifest = manifest_for_table("T");

    // Reference-returning unguarded formulas resolving to CellValue::Empty fail in Pass 4 with EmptyFormulaResult (F8Error)
    for formula in [
        "T[\"a\", @col]",
        "INDEX(T[\"a\"..\"a\", @col], 1)",
        "XLOOKUP(\"match\", T[\"k\"..\"k\", @col], T[\"a\"..\"a\", @col])",
        "IF(TRUE, T[\"a\", @col], 1)",
        "IFS(TRUE, T[\"a\", @col])",
        "CHOOSE(1, T[\"a\", @col])",
    ] {
        let row_a = RowSpec::new("a")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1))
            .with_cells(IndexMap::from([(ColId::from("c1"), CellInput::null())]));
        let row_k = RowSpec::new("k").with_cells(IndexMap::from([(
            ColId::from("c1"),
            CellInput::string("match"),
        )]));
        let target = RowSpec::new("target").with_expr(formula);
        let table = create_table("T", &["c1"], vec![row_a, row_k, target]);

        let err = Compiler::compile(&manifest, &[table])
            .expect_err(&format!("expected EmptyFormulaResult on '{formula}'"));
        assert!(matches!(err.diagnostic(), ErrorDiagnostic::F8Error { .. }));
        let CompilerError::Eval(EvalError::Type { cell, source }) = err else {
            panic!("expected EvalError::Type on '{formula}', got {err:?}");
        };
        assert_eq!(*cell, CellId::new("T", "target", "c1"));
        assert_eq!(source, TypeError::EmptyFormulaResult);
    }

    // Value-returning IFERROR / IFNA reject evaluated CellValue::Empty arms with ExpectedType::NonEmpty
    for formula in [
        "IFERROR(T[\"a\", @col], 1)",
        "IFERROR(1 / 0, T[\"a\", @col])",
        "IFNA(T[\"a\", @col], 1)",
        "IFNA(XLOOKUP(\"miss\", T[\"k\"..\"k\", @col], T[\"a\"..\"a\", @col]), T[\"a\", @col])",
    ] {
        let row_a = RowSpec::new("a")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1))
            .with_cells(IndexMap::from([(ColId::from("c1"), CellInput::null())]));
        let row_k = RowSpec::new("k").with_cells(IndexMap::from([(
            ColId::from("c1"),
            CellInput::string("match"),
        )]));
        let target = RowSpec::new("target").with_expr(formula);
        let table = create_table("T", &["c1"], vec![row_a, row_k, target]);

        let err = Compiler::compile(&manifest, &[table]).expect_err(&format!(
            "expected InvalidArgumentType(NonEmpty) on '{formula}'"
        ));
        assert!(matches!(err.diagnostic(), ErrorDiagnostic::F8Error { .. }));
        let CompilerError::Eval(EvalError::Type { cell, source }) = err else {
            panic!("expected EvalError::Type on '{formula}', got {err:?}");
        };
        assert_eq!(*cell, CellId::new("T", "target", "c1"));
        assert!(
            matches!(
                source,
                TypeError::InvalidArgumentType {
                    expected: f8_core::value::ExpectedType::NonEmpty,
                    found: ValueType::Empty,
                    ..
                }
            ),
            "expected InvalidArgumentType(NonEmpty, Empty) on '{formula}', got {source:?}"
        );
    }

    // Explicitly guarded formulas succeed, and CellValue::Empty occurs iff CellPayload::Literal(ValueLiteral::Null)
    let row_a = RowSpec::new("a")
        .with_format(Some(FormatPreset::Currency))
        .with_decimals(Some(1))
        .with_cells(IndexMap::from([(ColId::from("c1"), CellInput::null())]));
    let row_b = RowSpec::new("b")
        .with_format(Some(FormatPreset::Currency))
        .with_decimals(Some(1))
        .with_expr("IF(ISBLANK(T[\"a\", @col]), 0, T[\"a\", @col])");
    let row_c = RowSpec::new("c")
        .with_format(Some(FormatPreset::Currency))
        .with_decimals(Some(1))
        .with_expr("IF(ISBLANK(T[\"a\", @col]), \"\", T[\"a\", @col])");
    let count_a = RowSpec::new("count_a").with_expr("COUNT(T[\"a\"..\"a\", @col])");
    let count_b = RowSpec::new("count_b").with_expr("COUNT(T[\"b\", @col])");
    let counta_a = RowSpec::new("counta_a").with_expr("COUNTA(T[\"a\", @col])");
    let counta_b = RowSpec::new("counta_b").with_expr("COUNTA(T[\"b\", @col])");

    let table = create_table(
        "T",
        &["c1"],
        vec![row_a, row_b, row_c, count_a, count_b, counta_a, counta_b],
    );
    let compiled = Compiler::compile(&manifest, &[table]).expect("compile guarded model");
    let t = TableId::from("T");
    let c1 = ColId::from("c1");

    for (cell_id, val) in compiled.evaluated.iter() {
        let node = compiled.grid.get(cell_id).expect("grid cell exists");
        let is_null_literal = matches!(node.payload, CellPayload::Literal(ValueLiteral::Null));
        assert_eq!(
            val.is_empty(),
            is_null_literal,
            "Empty invariant violated at {cell_id}: val={val:?}, payload={:?}",
            node.payload
        );
    }

    assert_eq!(
        compiled.cell_value(&t, &RowId::from("a"), &c1).unwrap(),
        CellValue::Empty
    );
    assert_eq!(
        compiled.cell_formatted(&t, &RowId::from("a"), &c1).unwrap(),
        ""
    );
    assert_eq!(
        compiled.cell_value(&t, &RowId::from("b"), &c1).unwrap(),
        CellValue::Number(0.0)
    );
    assert_eq!(
        compiled.cell_formatted(&t, &RowId::from("b"), &c1).unwrap(),
        "$0.0"
    );
    assert_eq!(
        compiled.cell_value(&t, &RowId::from("c"), &c1).unwrap(),
        CellValue::Text(String::new())
    );
    assert_eq!(
        compiled.cell_formatted(&t, &RowId::from("c"), &c1).unwrap(),
        ""
    );
    assert_eq!(
        compiled
            .cell_value(&t, &RowId::from("count_a"), &c1)
            .unwrap(),
        CellValue::Number(0.0)
    );
    assert_eq!(
        compiled
            .cell_value(&t, &RowId::from("count_b"), &c1)
            .unwrap(),
        CellValue::Number(1.0)
    );
    assert_eq!(
        compiled
            .cell_value(&t, &RowId::from("counta_a"), &c1)
            .unwrap(),
        CellValue::Number(0.0)
    );
    assert_eq!(
        compiled
            .cell_value(&t, &RowId::from("counta_b"), &c1)
            .unwrap(),
        CellValue::Number(1.0)
    );
}

#[test]
fn test_scenario_13_stage_a_and_stage_b_strict_type_and_iferror_non_masking() {
    let manifest = manifest_for_table("T");

    let seed_rows = || {
        vec![
            RowSpec::new("r1").with_cells(IndexMap::from([(
                ColId::from("c1"),
                CellInput::number(10.0),
            )])),
            RowSpec::new("r2").with_cells(IndexMap::from([(
                ColId::from("c1"),
                CellInput::number(20.0),
            )])),
            RowSpec::new("r3").with_cells(IndexMap::from([(
                ColId::from("c1"),
                CellInput::number(30.0),
            )])),
            RowSpec::new("r4").with_cells(IndexMap::from([(
                ColId::from("c1"),
                CellInput::number(40.0),
            )])),
            RowSpec::new("cat1").with_cells(IndexMap::from([(
                ColId::from("c1"),
                CellInput::string("Alpha"),
            )])),
            RowSpec::new("cat2").with_cells(IndexMap::from([(
                ColId::from("c1"),
                CellInput::string("Beta"),
            )])),
            RowSpec::new("txt_cell").with_cells(IndexMap::from([(
                ColId::from("c1"),
                CellInput::string("base"),
            )])),
            RowSpec::new("bool_true").with_cells(IndexMap::from([(
                ColId::from("c1"),
                CellInput::boolean(true),
            )])),
            RowSpec::new("txt_after_bool").with_cells(IndexMap::from([(
                ColId::from("c1"),
                CellInput::string("oops"),
            )])),
            RowSpec::new("null_cell")
                .with_cells(IndexMap::from([(ColId::from("c1"), CellInput::null())])),
            RowSpec::new("float_cell").with_cells(IndexMap::from([(
                ColId::from("c1"),
                CellInput::number(2.5),
            )])),
            RowSpec::new("huge_float_cell").with_cells(IndexMap::from([(
                ColId::from("c1"),
                CellInput::number(1e20),
            )])),
            RowSpec::new("date_cell").with_cells(IndexMap::from([(
                ColId::from("c1"),
                CellInput::number(45292.0),
            )])),
            RowSpec::new("div0_cell").with_expr("10 / 0"),
        ]
    };

    // Valid Scenario 2 compound / text formulas compile cleanly
    let mut valid_rows = seed_rows();
    valid_rows.push(RowSpec::new("ok_str").with_expr("\"str\""));
    valid_rows.push(RowSpec::new("ok_idx").with_expr(
        "INDEX(T[\"r1\"..\"r2\", @col], XMATCH(\"Beta\", T[\"cat1\"..\"cat2\", @col]))",
    ));
    valid_rows.push(RowSpec::new("ok_date").with_expr(
        "DATE(YEAR(T[\"date_cell\", @col]) + 1, MONTH(T[\"date_cell\", @col]), DAY(T[\"date_cell\", @col]))",
    ));
    let compiled = Compiler::compile(&manifest, &[create_table("T", &["c1"], valid_rows)])
        .expect("valid compound formulas compile");
    assert_eq!(
        compiled
            .cell_value(
                &TableId::from("T"),
                &RowId::from("ok_idx"),
                &ColId::from("c1")
            )
            .unwrap(),
        CellValue::Number(20.0)
    );

    // Stage A & Stage B invalid formulas fail compilation with F8Error (even under IFERROR)
    let failing_formulas = [
        // Stage A (Scenario 2)
        "\"5\" + 1",
        "TRUE * 10",
        "-FALSE",
        "ABS(\"invalid\")",
        "ROUND(TRUE, 2)",
        "ROUND(10, 2.5)",
        "ROUND(10, 100000000000000000000)",
        "INDEX(T[\"r1\"..\"r4\", @col], 1.5)",
        "DATE(2024, 1.5, 1)",
        "PMT(0.05, 10, -1000, 0, 0.5)",
        "DAYS(\"invalid\", DATE(2024, 1, 1))",
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
        // Stage B dynamic cell references (Scenario 3)
        "T[\"txt_cell\", @col] + 1",
        "T[\"null_cell\", @col] + 1",
        "-T[\"null_cell\", @col]",
        "ABS(T[\"txt_cell\", @col])",
        "ROUND(10, T[\"float_cell\", @col])",
        "ROUND(10, T[\"huge_float_cell\", @col])",
        "IF(T[\"r1\", @col], 1, 0)",
        "IF(T[\"null_cell\", @col], 1, 0)",
        "NOT(T[\"null_cell\", @col])",
        "AND(T[\"null_cell\", @col], TRUE)",
        "AND(T[\"bool_true\"..\"txt_after_bool\", @col])",
        "T[\"r1\", @col] < T[\"txt_cell\", @col]",
        "T[\"null_cell\", @col] = \"\"",
        "T[\"null_cell\", @col] = 0",
        "T[\"null_cell\", @col] = FALSE",
        "T[\"null_cell\", @col] = T[\"null_cell\", @col]",
        "T[\"div0_cell\", @col] + T[\"txt_cell\", @col]",
        "T[\"div0_cell\", @col] = T[\"null_cell\", @col]",
        "AND(T[\"div0_cell\", @col], T[\"txt_cell\", @col])",
        // Wrapped in IFERROR(..., 0) — must still fail compilation with F8Error!
        "IFERROR(T[\"txt_cell\", @col] + 1, 0)",
        "IFERROR(T[\"null_cell\", @col] = \"\", FALSE)",
        "IFERROR(AND(T[\"null_cell\", @col], TRUE), FALSE)",
        "IFERROR(ROUND(10, T[\"float_cell\", @col]), 0)",
    ];

    for formula in failing_formulas {
        let mut rows = seed_rows();
        rows.push(RowSpec::new("target").with_expr(formula));
        let err = Compiler::compile(&manifest, &[create_table("T", &["c1"], rows)])
            .expect_err(&format!("expected F8Error on '{formula}'"));
        assert!(
            matches!(err.diagnostic(), ErrorDiagnostic::F8Error { .. }),
            "expected ErrorDiagnostic::F8Error on '{formula}', got {err:?}"
        );
    }
}

#[test]
fn test_scenario_14_syntactic_slices_range_kinds_and_static_shape_bounds() {
    let manifest = manifest_for_table("T");

    let build_rows = |target_formula: &str| {
        let mk_row = |id: &str, v1: f64, v2: f64, v3: f64, v4: f64| {
            RowSpec::new(id).with_cells(IndexMap::from([
                (ColId::from("c1"), CellInput::number(v1)),
                (ColId::from("c2"), CellInput::number(v2)),
                (ColId::from("c3"), CellInput::number(v3)),
                (ColId::from("c4"), CellInput::number(v4)),
            ]))
        };
        vec![
            mk_row("r1", -1000.0, 20.0, 30.0, 40.0),
            mk_row("r2", 400.0, 20.0, 30.0, 40.0),
            mk_row("r3", 400.0, 20.0, 30.0, 40.0),
            mk_row("r4", 400.0, 20.0, 30.0, 40.0),
            mk_row("d1", 45292.0, 45292.0, 45292.0, 45292.0),
            mk_row("d2", 45658.0, 45658.0, 45658.0, 45658.0),
            mk_row("d3", 46023.0, 46023.0, 46023.0, 46023.0),
            mk_row("d4", 46388.0, 46388.0, 46388.0, 46388.0),
            mk_row("dyn_oob", 9.0, 9.0, 9.0, 9.0),
            RowSpec::new("target").with_expr(target_formula),
        ]
    };

    let failing_q07 = [
        // Syntactic slices in scalar contexts (including 1x1)
        "T[\"r1\"..\"r2\", @col]",
        "T[\"r1\"..\"r1\", @col]",
        "1 + T[\"r1\"..\"r1\", @col]",
        "1 + T[\"r1\"..\"r2\", @col]",
        "ABS(T[\"r1\"..\"r2\", @col])",
        "ISBLANK(T[\"r1\"..\"r2\", @col])",
        "ISBLANK(T[\"r1\"..\"r1\", @col])",
        "IF(TRUE, 1, T[\"r1\"..\"r2\", @col])",
        "IFS(T[\"r1\"..\"r2\", @col], 1, TRUE, 2)",
        // Non-reference scalars in ParamKind::Range slots
        "SUMIFS(10, T[\"r1\"..\"r4\", @col], \"Alpha\")",
        "SUMPRODUCT(\"abc\")",
        "COUNT(10)",
        "COUNTA(\"a\")",
        "XMATCH(10, 10)",
        "XLOOKUP(10, 10, 20)",
        "INDEX(10, 1)",
        "IRR(100)",
        "XNPV(0.1, 100, 45292)",
        // Slice dimension/length mismatches
        "SUMPRODUCT(T[\"r1\"..\"r4\", @col], T[\"r1\"..\"r2\", @col])",
        "SUMIFS(T[\"r1\"..\"r4\", @col], T[\"r1\"..\"r2\", @col], \"Alpha\")",
        "COUNTIFS(T[\"r1\"..\"r2\", \"c1\"..\"c2\"], 1, T[\"r1\", \"c1\"..\"c4\"], 1)",
        "XLOOKUP(10, T[\"r1\"..\"r4\", \"c1\"], T[\"r1\"..\"r3\", \"c1\"])",
        "XLOOKUP(10, T[\"r1\"..\"r4\", \"c1\"], T[\"r1\", \"c1\"..\"c4\"])",
        "XNPV(0.1, T[\"r1\"..\"r4\", \"c1\"], T[\"d1\"..\"d2\", \"c1\"])",
        "XIRR(T[\"r2\"..\"r4\", \"c1\"], T[\"d1\"..\"d4\", \"c1\"])",
        // 2D slices in 1D slots
        "XMATCH(10, T[\"r1\"..\"r2\", \"c1\"..\"c2\"])",
        "XLOOKUP(10, T[\"r1\"..\"r2\", \"c1\"..\"c2\"], T[\"r1\"..\"r2\", \"c1\"..\"c2\"])",
        "XNPV(-2.0, T[\"r1\"..\"r2\", \"c1\"..\"c2\"], T[\"d1\"..\"d2\", \"c1\"..\"c2\"])",
        "XIRR(T[\"r1\"..\"r2\", \"c1\"..\"c2\"], T[\"d1\"..\"d2\", \"c1\"..\"c2\"])",
        // 2-arg INDEX on 2D slice & constant out-of-bounds indices
        "INDEX(T[\"r1\"..\"r4\", \"c1\"..\"c2\"], 2)",
        "INDEX(T[\"r1\"..\"r4\", \"c1\"], 5)",
        "INDEX(T[\"r1\", \"c1\"..\"c4\"], 5)",
        "INDEX(T[\"r1\"..\"r4\", \"c1\"], 0)",
        "INDEX(T[\"r1\"..\"r4\", \"c1\"], -1)",
        "CHOOSE(5, 100, 200, 300)",
        "CHOOSE(0, 100)",
        // Inside IFERROR and dormant IF branches
        "IFERROR(INDEX(T[\"r1\"..\"r4\", \"c1\"], 5), 0)",
        "IF(TRUE, 1, CHOOSE(5, 100, 200, 300))",
    ];

    for formula in failing_q07 {
        let table = create_table("T", &["c1", "c2", "c3", "c4"], build_rows(formula));
        let err = Compiler::compile(&manifest, &[table])
            .expect_err(&format!("expected Q-07 F8Error on '{formula}'"));
        assert!(matches!(err.diagnostic(), ErrorDiagnostic::F8Error { .. }));
    }

    // Valid Q-07 cases succeed with Excel parity (1x1 SingleCell & Slice in Range/RangeOrArray, 2-arg horizontal INDEX, dynamic OOB)
    let mut valid_rows = build_rows("COUNT(T[\"r1\", @col])");
    valid_rows.push(RowSpec::new("sum_1x1").with_expr("SUM(T[\"r1\"..\"r1\", @col])"));
    valid_rows.push(RowSpec::new("idx_horiz").with_expr("INDEX(T[\"r1\", \"c1\"..\"c4\"], 3)"));
    valid_rows.push(
        RowSpec::new("idx_dyn_oob")
            .with_expr("INDEX(T[\"r1\"..\"r4\", \"c1\"], T[\"dyn_oob\", @col])"),
    );
    valid_rows
        .push(RowSpec::new("choose_dyn_oob").with_expr("CHOOSE(T[\"dyn_oob\", @col], 100, 200)"));
    let compiled = Compiler::compile(
        &manifest,
        &[create_table("T", &["c1", "c2", "c3", "c4"], valid_rows)],
    )
    .expect("compile valid Q-07 cases");
    let t = TableId::from("T");
    let c1 = ColId::from("c1");
    assert_eq!(
        compiled
            .cell_value(&t, &RowId::from("target"), &c1)
            .unwrap(),
        CellValue::Number(1.0)
    );
    assert_eq!(
        compiled
            .cell_value(&t, &RowId::from("sum_1x1"), &c1)
            .unwrap(),
        CellValue::Number(-1000.0)
    );
    assert_eq!(
        compiled
            .cell_value(&t, &RowId::from("idx_horiz"), &c1)
            .unwrap(),
        CellValue::Number(30.0)
    );
    assert_eq!(
        compiled
            .cell_value(&t, &RowId::from("idx_dyn_oob"), &c1)
            .unwrap(),
        CellValue::Error(ExcelError::Ref)
    );
    assert_eq!(
        compiled
            .cell_value(&t, &RowId::from("choose_dyn_oob"), &c1)
            .unwrap(),
        CellValue::Error(ExcelError::Value)
    );
}

#[test]
fn test_scenario_15_isblank_na_nullable_guards_sparse_and_and_domain_errors() {
    let manifest = manifest_for_table("T");

    let rows = vec![
        RowSpec::new("null_cell")
            .with_cells(IndexMap::from([(ColId::from("c1"), CellInput::null())])),
        RowSpec::new("empty_str")
            .with_cells(IndexMap::from([(ColId::from("c1"), CellInput::string(""))])),
        RowSpec::new("zero_cell")
            .with_cells(IndexMap::from([(ColId::from("c1"), CellInput::number(0.0))])),
        RowSpec::new("false_cell").with_cells(IndexMap::from([(
            ColId::from("c1"),
            CellInput::boolean(false),
        )])),
        RowSpec::new("cat1").with_cells(IndexMap::from([(
            ColId::from("c1"),
            CellInput::string("Alpha"),
        )])),
        RowSpec::new("cat2").with_cells(IndexMap::from([(
            ColId::from("c1"),
            CellInput::string("Beta"),
        )])),
        RowSpec::new("sp1")
            .with_cells(IndexMap::from([(ColId::from("c1"), CellInput::null())])),
        RowSpec::new("sp2").with_cells(IndexMap::from([(
            ColId::from("c1"),
            CellInput::number(42.0),
        )])),
        RowSpec::new("f1")
            .with_cells(IndexMap::from([(ColId::from("c1"), CellInput::null())])),
        RowSpec::new("f2").with_cells(IndexMap::from([(
            ColId::from("c1"),
            CellInput::boolean(true),
        )])),
        RowSpec::new("huge_cell").with_cells(IndexMap::from([(
            ColId::from("c1"),
            CellInput::number(1e308),
        )])),
        RowSpec::new("guarded_zero")
            .with_expr("IF(ISBLANK(T[\"null_cell\", @col]), 0, T[\"null_cell\", @col])"),
        // ISBLANK introspection cases
        RowSpec::new("ib_null").with_expr("ISBLANK(T[\"null_cell\", @col])"),
        RowSpec::new("ib_guarded").with_expr("ISBLANK(T[\"guarded_zero\", @col])"),
        RowSpec::new("ib_empty_str").with_expr("ISBLANK(T[\"empty_str\", @col])"),
        RowSpec::new("ib_zero").with_expr("ISBLANK(T[\"zero_cell\", @col])"),
        RowSpec::new("ib_false").with_expr("ISBLANK(T[\"false_cell\", @col])"),
        RowSpec::new("ib_div0").with_expr("ISBLANK(1 / 0)"),
        RowSpec::new("ib_if").with_expr("ISBLANK(IF(TRUE, T[\"null_cell\", @col], 0))"),
        RowSpec::new("ib_ifs").with_expr("ISBLANK(IFS(TRUE, T[\"null_cell\", @col]))"),
        RowSpec::new("ib_choose").with_expr("ISBLANK(CHOOSE(1, T[\"null_cell\", @col]))"),
        RowSpec::new("ib_index").with_expr("ISBLANK(INDEX(T[\"sp1\"..\"sp2\", @col], 1))"),
        RowSpec::new("ib_xlookup_hit").with_expr(
            "ISBLANK(XLOOKUP(\"Alpha\", T[\"cat1\"..\"cat2\", @col], T[\"sp1\"..\"sp2\", @col], 0))",
        ),
        RowSpec::new("ib_xlookup_miss").with_expr(
            "ISBLANK(XLOOKUP(\"Missing\", T[\"cat1\"..\"cat2\", @col], T[\"sp1\"..\"sp2\", @col], 0))",
        ),
        // NA() ratio suppression
        RowSpec::new("na_direct").with_expr("NA()"),
        RowSpec::new("na_prop").with_expr("1 + IF(FALSE, 10, NA())"),
        RowSpec::new("na_trapped").with_expr("IFNA(IF(FALSE, 10, NA()), 0)"),
        // Lazy nullable guards
        RowSpec::new("guard_str")
            .with_expr("IF(ISBLANK(T[\"null_cell\", @col]), \"\", T[\"null_cell\", @col])"),
        RowSpec::new("guard_arith")
            .with_expr("IF(ISBLANK(T[\"null_cell\", @col]), 0, T[\"null_cell\", @col] + 5)"),
        // Sparse ReferenceKind::Slice AND
        RowSpec::new("and_sparse_true").with_expr("AND(T[\"f1\"..\"f2\", @col])"),
        RowSpec::new("and_all_null").with_expr("AND(T[\"f1\"..\"f1\", @col])"),
        // Overflow/domain arithmetic & IFERROR
        RowSpec::new("pow_overflow").with_expr("10 ^ 309"),
        RowSpec::new("mul_overflow").with_expr("T[\"huge_cell\", @col] * 10"),
        RowSpec::new("neg_zero").with_expr("0 * -1"),
        RowSpec::new("iferror_div0").with_expr("IFERROR((10 / 0) + 1, 99)"),
    ];

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
        ],
        rows,
    };

    let compiled = Compiler::compile(&manifest, &[table]).expect("compile Scenario 5 model");
    let t = TableId::from("T");
    let c1 = ColId::from("c1");
    let get = |row: &str| compiled.cell_value(&t, &RowId::from(row), &c1).unwrap();

    assert_eq!(get("ib_null"), CellValue::Boolean(true));
    assert_eq!(get("ib_guarded"), CellValue::Boolean(false));
    assert_eq!(get("ib_empty_str"), CellValue::Boolean(false));
    assert_eq!(get("ib_zero"), CellValue::Boolean(false));
    assert_eq!(get("ib_false"), CellValue::Boolean(false));
    assert_eq!(get("ib_div0"), CellValue::Boolean(false));
    assert_eq!(get("ib_if"), CellValue::Boolean(true));
    assert_eq!(get("ib_ifs"), CellValue::Boolean(true));
    assert_eq!(get("ib_choose"), CellValue::Boolean(true));
    assert_eq!(get("ib_index"), CellValue::Boolean(true));
    assert_eq!(get("ib_xlookup_hit"), CellValue::Boolean(true));
    assert_eq!(get("ib_xlookup_miss"), CellValue::Boolean(false));

    assert_eq!(get("na_direct"), CellValue::Error(ExcelError::Na));
    assert_eq!(get("na_prop"), CellValue::Error(ExcelError::Na));
    assert_eq!(get("na_trapped"), CellValue::Number(0.0));

    assert_eq!(get("guard_str"), CellValue::Text(String::new()));
    assert_eq!(get("guard_arith"), CellValue::Number(0.0));

    assert_eq!(get("and_sparse_true"), CellValue::Boolean(true));
    assert_eq!(get("and_all_null"), CellValue::Error(ExcelError::Value));

    assert_eq!(get("pow_overflow"), CellValue::Error(ExcelError::Num));
    assert_eq!(get("mul_overflow"), CellValue::Error(ExcelError::Num));
    if let CellValue::Number(n) = get("neg_zero") {
        assert_eq!(n.to_bits(), 0.0_f64.to_bits());
    } else {
        panic!("expected +0.0 on neg_zero");
    }
    assert_eq!(get("iferror_div0"), CellValue::Number(99.0));

    assert_eq!(
        compiled.lowered_map.get(&CellId::new("T", "ib_null", "c1")),
        Some("=ISBLANK(C5)")
    );
    assert_eq!(
        compiled
            .lowered_map
            .get(&CellId::new("T", "na_direct", "c1")),
        Some("=NA()")
    );
}

#[test]
fn test_scenario_16_cluster_a2_monomorphic_branches_dormant_validation_and_lookup_strictness() {
    let manifest = manifest_for_table("T");

    let seed_rows = || {
        let cell = |id: &str, input: CellInput| {
            RowSpec::new(id).with_cells(IndexMap::from([(ColId::from("c1"), input)]))
        };
        vec![
            cell("ev", CellInput::number(100.0)),
            cell("ebitda", CellInput::number(10.0)),
            cell("num1", CellInput::number(10.0)),
            cell("num2", CellInput::number(20.0)),
            cell("float_cell", CellInput::number(2.5)),
            cell("dyn_idx", CellInput::number(1.0)),
            cell("dyn_oob_idx", CellInput::number(9.0)),
            cell("c1", CellInput::boolean(true)),
            cell("c2", CellInput::boolean(false)),
            cell("flag_false", CellInput::boolean(false)),
            cell("txt_cell", CellInput::string("base")),
            cell("txt1", CellInput::string("one")),
            cell("txt2", CellInput::string("two")),
            cell("cat1", CellInput::string("Alpha")),
            cell("cat2", CellInput::string("Beta")),
            cell("null_cell", CellInput::null()),
            RowSpec::new("is_null").with_expr("ISBLANK(T[\"null_cell\", @col])"),
            // Mixed slice [10.0, "two"]
            cell("mixed1", CellInput::number(10.0)),
            cell("mixed2", CellInput::string("two")),
            // Sparse numeric slice [null, 10.0, 30.0]
            cell("sp1", CellInput::null()),
            cell("sp2", CellInput::number(10.0)),
            cell("sp3", CellInput::number(30.0)),
            // Error in lookup_array [10.0, #DIV/0!, 30.0]
            cell("errk1", CellInput::number(10.0)),
            RowSpec::new("errk2").with_expr("10 / 0"),
            cell("errk3", CellInput::number(30.0)),
            // Linear sparse + error numeric slice [#DIV/0!, 10.0, null, 30.0]
            RowSpec::new("lin_sparse1").with_expr("10 / 0"),
            cell("lin_sparse2", CellInput::number(10.0)),
            cell("lin_sparse3", CellInput::null()),
            cell("lin_sparse4", CellInput::number(30.0)),
            // Unsorted [10.0, 30.0, 20.0]
            cell("unsort1", CellInput::number(10.0)),
            cell("unsort2", CellInput::number(30.0)),
            cell("unsort3", CellInput::number(20.0)),
            // Clean ascending [10.0, 20.0, 30.0, 40.0]
            cell("asc1", CellInput::number(10.0)),
            cell("asc2", CellInput::number(20.0)),
            cell("asc3", CellInput::number(30.0)),
            cell("asc4", CellInput::number(40.0)),
            // Clean descending [40.0, 30.0, 20.0, 10.0]
            cell("desc1", CellInput::number(40.0)),
            cell("desc2", CellInput::number(30.0)),
            cell("desc3", CellInput::number(20.0)),
            cell("desc4", CellInput::number(10.0)),
            // Matching 4-row category labels
            cell("lbl1", CellInput::string("Alpha")),
            cell("lbl2", CellInput::string("Beta")),
            cell("lbl3", CellInput::string("Gamma")),
            cell("lbl4", CellInput::string("Delta")),
        ]
    };

    // Scenarios 1, 2, & 3 compile-time rejections (all halt with F8Error)
    let failing_cluster_a2 = [
        // Scenario 1 (Stage A & retired SWITCH)
        "IF(TRUE, 10)",
        "SWITCH(1, 1, 10, 20)",
        "IF(FALSE, SWITCH(1, 1, 10, 20), 0)",
        "IF(T[\"ebitda\", @col] > 0, T[\"ev\", @col] / T[\"ebitda\", @col], \"NM\")",
        "IFERROR(10 / 0, \"N/A\")",
        "CHOOSE(1, 10, \"NM\")",
        "IFS(T[\"c1\", @col], 10, T[\"c2\", @col], \"NM\")",
        "IF(FALSE, IF(TRUE, \"a\", \"b\") = IF(FALSE, 1, 2), TRUE)",
        "ROUND(10, IF(FALSE, 2, 2.5))",
        "ROUND(10, IF(TRUE, \"a\", \"b\"))",
        "ROUND(10, IF(TRUE, 2, \"bad\"))",
        "1 + XLOOKUP(\"Beta\", T[\"cat1\"..\"cat2\", @col], T[\"num1\"..\"num2\", @col], \"NM\")",
        // Scenario 2 (Stage B: IFERROR/IFNA Empty, runtime unification, dormant memoized checks, taken-branch integer bounds)
        "IFERROR(T[\"null_cell\", @col], 1)",
        "IFERROR(1 / 0, T[\"null_cell\", @col])",
        "IFNA(T[\"null_cell\", @col], 1)",
        "ISBLANK(IFERROR(T[\"null_cell\", @col], 1))",
        "IF(TRUE, T[\"num1\", @col], T[\"txt_cell\", @col])",
        "IF(T[\"flag_false\", @col], T[\"txt_cell\", @col] + 5, 0)",
        "IF(T[\"flag_false\", @col], ROUND(10, T[\"float_cell\", @col]), 0)",
        "IF(T[\"flag_false\", @col], T[\"num1\", @col] < T[\"txt_cell\", @col], FALSE)",
        "IF(TRUE, 10, XLOOKUP(\"Beta\", T[\"cat1\"..\"cat2\", @col], T[\"txt1\"..\"txt2\", @col]))",
        "IF(TRUE, 10, INDEX(T[\"txt1\"..\"txt2\", @col], 1))",
        "IF(TRUE, 10, INDEX(T[\"txt1\"..\"txt2\", @col], T[\"dyn_idx\", @col]))",
        "IFERROR(INDEX(T[\"txt1\"..\"txt2\", @col], T[\"dyn_oob_idx\", @col]), 10)",
        "IFNA(XLOOKUP(\"Missing\", T[\"cat1\"..\"cat2\", @col], T[\"txt1\"..\"txt2\", @col]), 10)",
        "IFERROR(XLOOKUP(1 / 0, T[\"cat1\"..\"cat2\", @col], T[\"txt1\"..\"txt2\", @col]), 10)",
        "IF(ISBLANK(T[\"txt_cell\", @col]), 0, T[\"txt_cell\", @col])",
        "ROUND(10, IF(FALSE, 2, T[\"float_cell\", @col]))",
        "IF(FALSE, 0, T[\"null_cell\", @col] + 5)",
        // Scenario 3 (Lookup modes, Empty lookup_value, array homogeneity, binary search cleanliness & monotonicity)
        "XMATCH(\"Beta\", T[\"cat1\"..\"cat2\", @col], \"abc\")",
        "XMATCH(\"Beta\", T[\"cat1\"..\"cat2\", @col], 1.5)",
        "XMATCH(\"Beta\", T[\"cat1\"..\"cat2\", @col], T[\"num1\", @col])",
        "XMATCH(\"Beta\", T[\"cat1\"..\"cat2\", @col], 0 + 0)",
        "XMATCH(\"Beta\", T[\"cat1\"..\"cat2\", @col], 2)",
        "XLOOKUP(\"Beta\", T[\"cat1\"..\"cat2\", @col], T[\"num1\"..\"num2\", @col], _, 99)",
        "XLOOKUP(T[\"null_cell\", @col], T[\"cat1\"..\"cat2\", @col], T[\"num1\"..\"num2\", @col])",
        "XLOOKUP(1 / 0, T[\"mixed1\"..\"mixed2\", @col], T[\"num1\"..\"num2\", @col])",
        "XLOOKUP(\"Alpha\", T[\"cat1\"..\"cat2\", @col], T[\"mixed1\"..\"mixed2\", @col])",
        "XLOOKUP(\"Alpha\", T[\"cat1\"..\"cat2\", @col], T[\"num1\"..\"num2\", @col], T[\"txt_cell\", @col])",
        "INDEX(T[\"mixed1\"..\"mixed2\", @col], 1)",
        "XMATCH(10, T[\"sp1\"..\"sp3\", @col], 0, 2)",
        "XMATCH(10, T[\"errk1\"..\"errk3\", @col], 0, 2)",
        "XMATCH(20, T[\"unsort1\"..\"unsort3\", @col], 0, 2)",
        "XMATCH(20, T[\"asc1\"..\"asc3\", @col], 0, -2)",
        // Dormant binary search / mixed slice inside untaken IF branch also fails in Stage B!
        "IF(TRUE, 0, XMATCH(20, T[\"unsort1\"..\"unsort3\", @col], 0, 2))",
        "IF(TRUE, 0, INDEX(T[\"mixed1\"..\"mixed2\", @col], 1))",
        // Signed literal integer checks & lookup mode literal checks
        "ROUND(10, --2.5)",
        "ROUND(10, ---2.5)",
        "XLOOKUP(\"Alpha\", T[\"cat1\"..\"cat2\", @col], T[\"txt1\"..\"txt2\", @col], \"nf\", --1)",
    ];

    for formula in failing_cluster_a2 {
        let mut rows = seed_rows();
        rows.push(RowSpec::new("target").with_expr(formula));
        let err = Compiler::compile(&manifest, &[create_table("T", &["c1"], rows)])
            .expect_err(&format!("expected Cluster A2 F8Error on '{formula}'"));
        assert!(
            matches!(err.diagnostic(), ErrorDiagnostic::F8Error { .. }),
            "expected F8Error diagnostic on '{formula}', got {err:?}"
        );
        assert!(
            !err.to_string().contains("dormant"),
            "error diagnostic must never leak 'dormant' sentinel on '{formula}': {err}"
        );
    }

    // Exact Scenario 2 parameter-boundary integer diagnostics (never 'dormant')
    for (formula, expected_val) in [
        ("ROUND(10, 2.5)", "2.5"),
        ("ROUND(10, --2.5)", "2.5"),
        ("ROUND(10, ---2.5)", "-2.5"),
        ("ROUND(10, T[\"float_cell\", @col])", "2.5"),
        ("IF(FALSE, ROUND(10, 2.5), 0)", "2.5"),
        (
            "IF(T[\"flag_false\", @col], ROUND(10, T[\"float_cell\", @col]), 0)",
            "2.5",
        ),
        ("ROUND(10, IF(FALSE, 2, 2.5))", "2.5"),
        ("ROUND(10, IF(FALSE, 2, T[\"float_cell\", @col]))", "2.5"),
    ] {
        let mut rows = seed_rows();
        rows.push(RowSpec::new("target").with_expr(formula));
        let err = Compiler::compile(&manifest, &[create_table("T", &["c1"], rows)])
            .expect_err(&format!("expected NonIntegerArgument on '{formula}'"));
        let CompilerError::Eval(EvalError::Type { source, .. }) = err else {
            panic!("expected EvalError::Type on '{formula}', got {err:?}");
        };
        assert_eq!(
            source,
            TypeError::NonIntegerArgument {
                function: "ROUND",
                param: "num_digits",
                position: 2,
                value: expected_val.into(),
            },
            "unexpected TypeError on '{formula}'"
        );
    }

    // Stage B EvalError::Type regressions: active error-producing typed expressions
    let stage_b_type_errors = [
        "1 + INDEX(T[\"txt1\"..\"txt2\", @col], T[\"dyn_oob_idx\", @col])",
        "-XLOOKUP(\"Missing\", T[\"cat1\"..\"cat2\", @col], T[\"txt1\"..\"txt2\", @col])",
        "-IF(T[\"flag_false\", @col], T[\"txt_cell\", @col], NA())",
        "1 = XLOOKUP(\"Missing\", T[\"cat1\"..\"cat2\", @col], T[\"txt1\"..\"txt2\", @col])",
        "\"a\" < INDEX(T[\"num1\"..\"num2\", @col], T[\"dyn_oob_idx\", @col])",
        "ABS(XLOOKUP(\"Missing\", T[\"cat1\"..\"cat2\", @col], T[\"txt1\"..\"txt2\", @col]))",
        "AND(TRUE, XLOOKUP(\"Missing\", T[\"cat1\"..\"cat2\", @col], T[\"txt1\"..\"txt2\", @col]))",
        "IFS(XLOOKUP(\"Missing\", T[\"cat1\"..\"cat2\", @col], T[\"txt1\"..\"txt2\", @col]), 10, TRUE, 20)",
        "XMATCH(IF(FALSE, \"Alpha\", NA()), T[\"num1\"..\"num2\", @col])",
        "XMATCH(INDEX(T[\"txt1\"..\"txt2\", @col], T[\"dyn_oob_idx\", @col]), T[\"num1\"..\"num2\", @col])",
        "XLOOKUP(XLOOKUP(\"Missing\", T[\"cat1\"..\"cat2\", @col], T[\"txt1\"..\"txt2\", @col]), T[\"num1\"..\"num2\", @col], T[\"num1\"..\"num2\", @col])",
    ];

    for formula in stage_b_type_errors {
        let mut rows = seed_rows();
        rows.push(RowSpec::new("target").with_expr(formula));
        let err = Compiler::compile(&manifest, &[create_table("T", &["c1"], rows)])
            .expect_err(&format!("expected EvalError::Type on '{formula}'"));
        assert!(
            matches!(err, CompilerError::Eval(EvalError::Type { .. })),
            "expected CompilerError::Eval(EvalError::Type) on '{formula}', got {err:?}"
        );
    }

    // Valid expressions compile cleanly and match Excel 100%
    let mut ok_rows = seed_rows();
    ok_rows.extend([
        RowSpec::new("ok_if_txt").with_expr("IF(T[\"c1\", @col], \"PASS\", \"FAIL\")"),
        RowSpec::new("ok_if_na").with_expr(
            "IF(T[\"ebitda\", @col] > 0, T[\"ev\", @col] / T[\"ebitda\", @col], NA())",
        ),
        RowSpec::new("ok_round_taken_lit").with_expr("ROUND(10, IF(TRUE, 2, 2.5))"),
        RowSpec::new("ok_round_taken_cell")
            .with_expr("ROUND(10, IF(TRUE, 2, T[\"float_cell\", @col]))"),
        RowSpec::new("ok_guard_str")
            .with_expr("IF(ISBLANK(T[\"null_cell\", @col]), \"\", T[\"null_cell\", @col])"),
        RowSpec::new("ok_guard_arith")
            .with_expr("IF(ISBLANK(T[\"null_cell\", @col]), 0, T[\"null_cell\", @col] + 5)"),
        RowSpec::new("ok_guard_cmp")
            .with_expr("IF(ISBLANK(T[\"null_cell\", @col]), FALSE, T[\"null_cell\", @col] > 0)"),
        RowSpec::new("ok_helper_guard")
            .with_expr("IF(T[\"is_null\", @col], 0, T[\"null_cell\", @col] + 5)"),
        RowSpec::new("ok_dormant_empty").with_expr("IF(TRUE, 0, T[\"null_cell\", @col] + 5)"),
        RowSpec::new("ok_ib_if").with_expr("ISBLANK(IF(TRUE, T[\"null_cell\", @col], 0))"),
        RowSpec::new("ok_lin_sparse")
            .with_expr("XMATCH(30, T[\"lin_sparse1\"..\"lin_sparse4\", @col])"),
        RowSpec::new("ok_idx_sparse")
            .with_expr("INDEX(T[\"lin_sparse1\"..\"lin_sparse4\", @col], 2)"),
        RowSpec::new("ok_bin_asc").with_expr("XMATCH(30, T[\"asc1\"..\"asc4\", @col], 0, 2)"),
        RowSpec::new("ok_xlookup_asc").with_expr(
            "XLOOKUP(25, T[\"asc1\"..\"asc4\", @col], T[\"lbl1\"..\"lbl4\", @col], \"Miss\", -1, 2)",
        ),
        RowSpec::new("ok_bin_desc").with_expr("XMATCH(20, T[\"desc1\"..\"desc4\", @col], 0, -2)"),
        RowSpec::new("ok_xlookup_desc").with_expr(
            "XLOOKUP(25, T[\"desc1\"..\"desc4\", @col], T[\"lbl1\"..\"lbl4\", @col], \"Miss\", 1, -2)",
        ),
        RowSpec::new("ok_add_na").with_expr("1 + IF(FALSE, 10, NA())"),
        RowSpec::new("ok_eq_na").with_expr("1 = IF(FALSE, 10, NA())"),
        RowSpec::new("ok_abs_na").with_expr("ABS(IF(FALSE, 10, NA()))"),
        RowSpec::new("ok_ifs_err_cond").with_expr("IFS((1 / 0) > 0, 10, TRUE, 20)"),
        RowSpec::new("ok_xmatch_na").with_expr("XMATCH(NA(), T[\"num1\"..\"num2\", @col])"),
        RowSpec::new("ok_xlookup_na")
            .with_expr("XLOOKUP(NA(), T[\"cat1\"..\"cat2\", @col], T[\"num1\"..\"num2\", @col])"),
        RowSpec::new("ok_xmatch_oob_match_key").with_expr(
            "XMATCH(INDEX(T[\"txt1\"..\"txt2\", @col], T[\"dyn_oob_idx\", @col]), T[\"cat1\"..\"cat2\", @col])",
        ),
    ]);

    let compiled = Compiler::compile(&manifest, &[create_table("T", &["c1"], ok_rows)])
        .expect("valid Cluster A2 formulas must compile");
    let t = TableId::from("T");
    let c1 = ColId::from("c1");
    let get = |row: &str| compiled.cell_value(&t, &RowId::from(row), &c1).unwrap();

    assert_eq!(get("ok_if_txt"), CellValue::Text("PASS".into()));
    assert_eq!(get("ok_if_na"), CellValue::Number(10.0));
    assert_eq!(get("ok_round_taken_lit"), CellValue::Number(10.0));
    assert_eq!(get("ok_round_taken_cell"), CellValue::Number(10.0));
    assert_eq!(get("ok_guard_str"), CellValue::Text(String::new()));
    assert_eq!(get("ok_guard_arith"), CellValue::Number(0.0));
    assert_eq!(get("ok_guard_cmp"), CellValue::Boolean(false));
    assert_eq!(get("ok_helper_guard"), CellValue::Number(0.0));
    assert_eq!(get("ok_dormant_empty"), CellValue::Number(0.0));
    assert_eq!(get("ok_ib_if"), CellValue::Boolean(true));
    assert_eq!(get("ok_lin_sparse"), CellValue::Number(4.0));
    assert_eq!(get("ok_idx_sparse"), CellValue::Number(10.0));
    assert_eq!(get("ok_bin_asc"), CellValue::Number(3.0));
    assert_eq!(get("ok_xlookup_asc"), CellValue::Text("Beta".into()));
    assert_eq!(get("ok_bin_desc"), CellValue::Number(3.0));
    assert_eq!(get("ok_xlookup_desc"), CellValue::Text("Beta".into()));
    assert_eq!(get("ok_add_na"), CellValue::Error(ExcelError::Na));
    assert_eq!(get("ok_eq_na"), CellValue::Error(ExcelError::Na));
    assert_eq!(get("ok_abs_na"), CellValue::Error(ExcelError::Na));
    assert_eq!(get("ok_ifs_err_cond"), CellValue::Error(ExcelError::Div0));
    assert_eq!(get("ok_xmatch_na"), CellValue::Error(ExcelError::Na));
    assert_eq!(get("ok_xlookup_na"), CellValue::Error(ExcelError::Na));
    assert_eq!(
        get("ok_xmatch_oob_match_key"),
        CellValue::Error(ExcelError::Ref)
    );
}

#[test]
fn test_cross_cell_error_value_type_provenance_referential_transparency() {
    let manifest = manifest_for_table("T");
    let cell = |id: &str, val: CellInput| {
        RowSpec::new(id).with_cells(IndexMap::from([(ColId::from("c1"), val)]))
    };

    let helper_rows = |bool_adjacent_to_err_txt: bool| {
        let mut rows = vec![
            cell("cat1", CellInput::string("Alpha")),
            cell("cat2", CellInput::string("Beta")),
            cell("txt1", CellInput::string("one")),
            cell("txt2", CellInput::string("two")),
            RowSpec::new("dyn_oob_idx").with_expr("5 + 0"),
        ];
        if bool_adjacent_to_err_txt {
            rows.push(cell("num1", CellInput::number(10.0)));
            rows.push(cell("c1", CellInput::boolean(true)));
        } else {
            rows.push(cell("c1", CellInput::boolean(true)));
            rows.push(cell("num1", CellInput::number(10.0)));
        }
        rows.extend([
            RowSpec::new("err_txt").with_expr(
                "XLOOKUP(\"Missing\", T[\"cat1\"..\"cat2\", @col], T[\"txt1\"..\"txt2\", @col])",
            ),
            RowSpec::new("err_oob_txt")
                .with_expr("INDEX(T[\"txt1\"..\"txt2\", @col], T[\"dyn_oob_idx\", @col])"),
            RowSpec::new("err_div0").with_expr("1 / 0"),
            RowSpec::new("err_div0_2").with_expr("2 / 0"),
            RowSpec::new("idx_div0").with_expr("INDEX(T[\"err_div0\"..\"err_div0_2\", @col], 1)"),
            RowSpec::new("err_na").with_expr("NA()"),
        ]);
        rows
    };

    let rejected_cases = [
        (
            false,
            "IFNA(T[\"err_txt\", @col], 10)",
            TypeError::IncompatibleBranchTypes {
                function: "IFNA",
                position: 2,
                expected: ValueType::Text,
                found: ValueType::Number,
            },
        ),
        (
            false,
            "IFERROR(T[\"err_oob_txt\", @col], 10)",
            TypeError::IncompatibleBranchTypes {
                function: "IFERROR",
                position: 2,
                expected: ValueType::Text,
                found: ValueType::Number,
            },
        ),
        (
            false,
            "1 + T[\"err_txt\", @col]",
            TypeError::InvalidOperandType {
                operator: "+",
                expected: ExpectedType::Number,
                found: ValueType::Text,
            },
        ),
        (
            false,
            "T[\"err_txt\", @col] = 10",
            TypeError::IncompatibleComparison {
                operator: "=",
                left: ValueType::Text,
                right: ValueType::Number,
            },
        ),
        (
            false,
            "IFERROR(T[\"err_div0\", @col], \"fallback\")",
            TypeError::IncompatibleBranchTypes {
                function: "IFERROR",
                position: 2,
                expected: ValueType::Number,
                found: ValueType::Text,
            },
        ),
        (
            false,
            "IF(FALSE, 1 + T[\"err_txt\", @col], 0)",
            TypeError::InvalidOperandType {
                operator: "+",
                expected: ExpectedType::Number,
                found: ValueType::Text,
            },
        ),
        (
            false,
            "AND(T[\"err_txt\", @col], TRUE)",
            TypeError::InvalidArgumentType {
                function: "AND",
                param: "logical1",
                position: 1,
                expected: ExpectedType::Boolean,
                found: ValueType::Text,
            },
        ),
        (
            true,
            "AND(T[\"c1\"..\"err_txt\", @col])",
            TypeError::InvalidArgumentType {
                function: "AND",
                param: "logical1",
                position: 1,
                expected: ExpectedType::Boolean,
                found: ValueType::Text,
            },
        ),
        (
            true,
            "AND(T[\"err_txt\"..\"err_txt\", @col])",
            TypeError::InvalidArgumentType {
                function: "AND",
                param: "logical1",
                position: 1,
                expected: ExpectedType::Boolean,
                found: ValueType::Text,
            },
        ),
        (
            true,
            "IF(FALSE, AND(T[\"c1\"..\"err_txt\", @col]), FALSE)",
            TypeError::InvalidArgumentType {
                function: "AND",
                param: "logical1",
                position: 1,
                expected: ExpectedType::Boolean,
                found: ValueType::Text,
            },
        ),
        (
            true,
            "IF(FALSE, AND(T[\"err_txt\"..\"err_txt\", @col]), FALSE)",
            TypeError::InvalidArgumentType {
                function: "AND",
                param: "logical1",
                position: 1,
                expected: ExpectedType::Boolean,
                found: ValueType::Text,
            },
        ),
        (
            false,
            "XLOOKUP(10, T[\"num1\"..\"err_txt\", @col], T[\"txt1\"..\"txt2\", @col])",
            TypeError::InvalidArgumentType {
                function: "XLOOKUP",
                param: "lookup_array",
                position: 2,
                expected: ExpectedType::Number,
                found: ValueType::Text,
            },
        ),
        (
            false,
            "XMATCH(10, T[\"err_txt\"..\"err_txt\", @col])",
            TypeError::InvalidArgumentType {
                function: "XMATCH",
                param: "lookup_array",
                position: 2,
                expected: ExpectedType::Number,
                found: ValueType::Text,
            },
        ),
        (
            false,
            "XMATCH(10, T[\"err_txt\"..\"err_txt\", @col], 0, 2)",
            TypeError::InvalidArgumentType {
                function: "XMATCH",
                param: "lookup_array",
                position: 2,
                expected: ExpectedType::Number,
                found: ValueType::Text,
            },
        ),
        (
            false,
            "INDEX(T[\"num1\"..\"err_txt\", @col], 1)",
            TypeError::InvalidArgumentType {
                function: "INDEX",
                param: "array",
                position: 1,
                expected: ExpectedType::Number,
                found: ValueType::Text,
            },
        ),
        (
            false,
            "IF(FALSE, INDEX(T[\"num1\"..\"err_txt\", @col], 1), 0)",
            TypeError::InvalidArgumentType {
                function: "INDEX",
                param: "array",
                position: 1,
                expected: ExpectedType::Number,
                found: ValueType::Text,
            },
        ),
        (
            false,
            "XLOOKUP(\"Alpha\", T[\"cat1\"..\"cat2\", @col], T[\"num1\"..\"err_txt\", @col])",
            TypeError::InvalidArgumentType {
                function: "XLOOKUP",
                param: "return_array",
                position: 3,
                expected: ExpectedType::Number,
                found: ValueType::Text,
            },
        ),
        (
            false,
            "IFERROR(INDEX(T[\"err_div0\"..\"err_div0_2\", @col], 1), \"fallback\")",
            TypeError::IncompatibleBranchTypes {
                function: "IFERROR",
                position: 2,
                expected: ValueType::Number,
                found: ValueType::Text,
            },
        ),
        (
            false,
            "IFERROR(T[\"idx_div0\", @col], \"fallback\")",
            TypeError::IncompatibleBranchTypes {
                function: "IFERROR",
                position: 2,
                expected: ValueType::Number,
                found: ValueType::Text,
            },
        ),
    ];

    for (bool_adj, formula, expected_type_err) in rejected_cases {
        let mut rows = helper_rows(bool_adj);
        rows.push(RowSpec::new("target").with_expr(formula));
        let err = Compiler::compile(&manifest, &[create_table("T", &["c1"], rows)])
            .expect_err(&format!("expected EvalError::Type on '{formula}'"));
        let CompilerError::Eval(EvalError::Type { cell, source }) = err else {
            panic!("expected CompilerError::Eval(EvalError::Type) on '{formula}', got {err:?}");
        };
        assert_eq!(*cell, CellId::new("T", "target", "c1"));
        assert_eq!(source, expected_type_err, "mismatch on formula '{formula}'");
    }

    let valid_rows = vec![
        cell("cat1", CellInput::string("Alpha")),
        cell("cat2", CellInput::string("Beta")),
        cell("txt1", CellInput::string("one")),
        cell("txt2", CellInput::string("two")),
        RowSpec::new("dyn_oob_idx").with_expr("5 + 0"),
        RowSpec::new("err_txt").with_expr(
            "XLOOKUP(\"Missing\", T[\"cat1\"..\"cat2\", @col], T[\"txt1\"..\"txt2\", @col])",
        ),
        RowSpec::new("err_oob_txt")
            .with_expr("INDEX(T[\"txt1\"..\"txt2\", @col], T[\"dyn_oob_idx\", @col])"),
        RowSpec::new("err_div0").with_expr("1 / 0"),
        RowSpec::new("err_div0_2").with_expr("2 / 0"),
        cell("c1", CellInput::boolean(true)),
        RowSpec::new("err_na").with_expr("NA()"),
        RowSpec::new("ok_ifna_txt").with_expr("IFNA(T[\"err_txt\", @col], \"fallback\")"),
        RowSpec::new("ok_iferr_div0").with_expr("IFERROR(T[\"err_div0\", @col], 0)"),
        RowSpec::new("ok_iferr_idx_div0")
            .with_expr("IFERROR(INDEX(T[\"err_div0\"..\"err_div0_2\", @col], 1), 0)"),
        RowSpec::new("ok_add_div0").with_expr("1 + T[\"err_div0\", @col]"),
        RowSpec::new("ok_ifna_na_num").with_expr("IFNA(T[\"err_na\", @col], 10)"),
        RowSpec::new("ok_ifna_na_txt").with_expr("IFNA(T[\"err_na\", @col], \"fallback\")"),
        RowSpec::new("ok_and_na_slice").with_expr("AND(T[\"c1\"..\"err_na\", @col])"),
        RowSpec::new("ok_dormant_and_na_slice")
            .with_expr("IF(TRUE, TRUE, AND(T[\"c1\"..\"err_na\", @col]))"),
    ];

    let compiled = Compiler::compile(&manifest, &[create_table("T", &["c1"], valid_rows)])
        .expect("valid cross-cell error provenance formulas must compile");
    let t = TableId::from("T");
    let c1 = ColId::from("c1");
    let get = |row: &str| compiled.cell_value(&t, &RowId::from(row), &c1).unwrap();

    assert_eq!(get("err_txt"), CellValue::Error(ExcelError::Na));
    assert_eq!(get("err_oob_txt"), CellValue::Error(ExcelError::Ref));
    assert_eq!(get("err_div0"), CellValue::Error(ExcelError::Div0));
    assert_eq!(get("err_div0_2"), CellValue::Error(ExcelError::Div0));
    assert_eq!(get("err_na"), CellValue::Error(ExcelError::Na));
    assert_eq!(get("ok_ifna_txt"), CellValue::Text("fallback".into()));
    assert_eq!(get("ok_iferr_div0"), CellValue::Number(0.0));
    assert_eq!(get("ok_iferr_idx_div0"), CellValue::Number(0.0));
    assert_eq!(get("ok_add_div0"), CellValue::Error(ExcelError::Div0));
    assert_eq!(get("ok_ifna_na_num"), CellValue::Number(10.0));
    assert_eq!(get("ok_ifna_na_txt"), CellValue::Text("fallback".into()));
    assert_eq!(get("ok_and_na_slice"), CellValue::Error(ExcelError::Na));
    assert_eq!(get("ok_dormant_and_na_slice"), CellValue::Boolean(true));
}

#[test]
fn test_scenario_17_cluster_b_aggregation_descriptor_and_date_domain_strictness() {
    let manifest = ProjectManifest {
        model_name: "ClusterBTest".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("T")],
        }],
    };

    let build_model_with_target = |target_formula: &str| -> TableSpec {
        let line_item_col = ColumnSpec::descriptor("line_item", "Line Item");
        let c1_col = ColumnSpec::new("c1");

        let make_row = |id: &str, label: &str, val: CellInput| -> RowSpec {
            RowSpec::new(id).with_cells(IndexMap::from([
                (ColId::from("line_item"), CellInput::string(label)),
                (ColId::from("c1"), val),
            ]))
        };

        TableSpec {
            id: TableId::from("T"),
            title: "T".into(),
            subtitle: None,
            currency: None,
            collapsible: false,
            placement: f8_core::model::PlacementSpec::default(),
            columns: vec![line_item_col, c1_col],
            rows: vec![
                make_row("n1", "Rev", CellInput::number(10.0)),
                make_row("n2", "COGS", CellInput::number(20.0)),
                make_row("txt", "Note", CellInput::string("N/A")),
                make_row("flag", "Active", CellInput::boolean(true)),
                make_row("hole", "Missing", CellInput::null()),
                make_row("cf0", "CF0", CellInput::number(-1000.0)),
                make_row("cf1", "CF1", CellInput::null()),
                make_row("cf2", "CF2", CellInput::number(600.0)),
                make_row("cf3", "CF3", CellInput::number(600.0)),
                make_row("d0", "D0", CellInput::number(45292.0)),
                make_row("d1", "D1", CellInput::number(45474.0)),
                make_row("d2", "D2", CellInput::number(45658.0)),
                make_row("d3", "D3", CellInput::number(46023.0)),
                make_row("yr24", "Yr24", CellInput::number(24.0)),
                make_row("zero", "Zero", CellInput::number(0.0)),
                make_row("type2", "Type2", CellInput::number(2.0)),
                make_row("basis5", "Basis5", CellInput::number(5.0)),
                RowSpec::new("target").with_cells(IndexMap::from([
                    (ColId::from("line_item"), CellInput::string("Target")),
                    (ColId::from("c1"), CellInput::formula(target_formula)),
                ])),
            ],
        }
    };

    // Scenario 1: Descriptor references rejected in Stage A (active and dormant)
    for desc_formula in [
        "SUM(T[\"n1\"..\"n2\", \"line_item\"])",
        "SUM(T[\"n1\", \"line_item\"])",
        "AVERAGE(T[\"n1\"..\"n2\", \"line_item\"])",
        "MIN(T[\"n1\"..\"n2\", \"line_item\"])",
        "MAX(T[\"n1\"..\"n2\", \"line_item\"])",
        "MEDIAN(T[\"n1\"..\"n2\", \"line_item\"])",
        "COUNT(T[\"n1\"..\"n2\", \"line_item\"])",
        "SUMPRODUCT(T[\"n1\"..\"n2\", \"line_item\"], T[\"n1\"..\"n2\", \"c1\"])",
        "SUMIFS(T[\"n1\"..\"n2\", \"line_item\"], T[\"n1\"..\"n2\", \"c1\"], 10)",
        "NPV(0.1, T[\"n1\"..\"n2\", \"line_item\"])",
        "IRR(T[\"n1\"..\"n2\", \"line_item\"])",
        "XNPV(0.1, T[\"d0\"..\"d3\", \"line_item\"], T[\"d0\"..\"d3\", \"c1\"])",
        "XIRR(T[\"cf0\"..\"cf3\", \"c1\"], T[\"cf0\"..\"cf3\", \"line_item\"])",
        "IF(FALSE, SUM(T[\"n1\"..\"n2\", \"line_item\"]), 0)",
    ] {
        let err = Compiler::compile(&manifest, &[build_model_with_target(desc_formula)])
            .expect_err(&format!(
                "expected DescriptorReferenceNotAllowed on {desc_formula}"
            ));
        assert!(matches!(err.diagnostic(), ErrorDiagnostic::F8Error { .. }));
        assert!(
            matches!(
                &err,
                CompilerError::Eval(EvalError::Function {
                    source,
                    ..
                }) if matches!(source.as_ref(), FunctionError::DescriptorReferenceNotAllowed { .. })
            ),
            "expected DescriptorReferenceNotAllowed on {desc_formula}, got {err:?}"
        );
    }

    // Scenario 2: Non-numeric cells, SingleCell Empty, and NPV/IRR/XNPV/XIRR Empty holes rejected (even inside IFERROR)
    for type_err_formula in [
        "SUM(T[\"n1\"..\"txt\", \"c1\"])",
        "SUM(T[\"txt\"..\"flag\", \"c1\"])",
        "SUM(TRUE, 5)",
        "SUM(T[\"hole\", \"c1\"])",
        "IFERROR(SUM(T[\"hole\", \"c1\"]), 0)",
        "AVERAGE(T[\"n1\", \"c1\"], T[\"hole\", \"c1\"])",
        "MIN(T[\"txt\"..\"txt\", \"c1\"])",
        "MAX(T[\"hole\", \"c1\"])",
        "MEDIAN(T[\"flag\"..\"flag\", \"c1\"])",
        "COUNT(T[\"txt\"..\"txt\", \"c1\"])",
        "COUNT(T[\"hole\", \"c1\"])",
        "SUMPRODUCT(T[\"n2\"..\"txt\", \"c1\"], T[\"n1\"..\"n2\", \"c1\"])",
        "SUMIFS(T[\"n1\"..\"txt\", \"c1\"], T[\"n1\"..\"txt\", \"line_item\"], \"Rev\")",
        "NPV(0.1, T[\"cf0\"..\"cf3\", \"c1\"])",
        "IFERROR(NPV(0.1, T[\"cf0\"..\"cf3\", \"c1\"]), 0)",
        "IRR(T[\"cf0\"..\"cf3\", \"c1\"])",
        "XNPV(0.1, T[\"cf0\"..\"cf3\", \"c1\"], T[\"d0\"..\"d3\", \"c1\"])",
        "XIRR(T[\"cf0\"..\"cf3\", \"c1\"], T[\"d0\"..\"d3\", \"c1\"])",
        "XIRR(T[\"n1\"..\"flag\", \"c1\"], T[\"hole\"..\"cf2\", \"c1\"])",
    ] {
        let err = Compiler::compile(&manifest, &[build_model_with_target(type_err_formula)])
            .expect_err(&format!("expected EvalError::Type on {type_err_formula}"));
        assert!(matches!(err.diagnostic(), ErrorDiagnostic::F8Error { .. }));
        assert!(
            matches!(err, CompilerError::Eval(EvalError::Type { .. })),
            "expected EvalError::Type on {type_err_formula}, got {err:?}"
        );
    }

    // Scenario 4 & 5: Date, serial, annuity type, and day-count basis domain errors (Stage A & Stage B)
    for domain_err_formula in [
        "DATE(24, 12, 31)",
        "DATE(2024, 6, 31)",
        "DATE(2025, 2, 29)",
        "IF(FALSE, DATE(2024, 6, 31), DATE(2024, 1, 1))",
        "DATE(T[\"yr24\", \"c1\"], 14, 1)",
        "IFERROR(DATE(T[\"yr24\", \"c1\"], 1, 1), 0)",
        "IF(FALSE, DATE(T[\"yr24\", \"c1\"], 1, 1), DATE(2024, 1, 1))",
        "PMT(0.05, 10, -1000, 0, T[\"type2\", \"c1\"])",
        "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), T[\"basis5\", \"c1\"])",
        "YEAR(0)",
        "YEAR(T[\"zero\", \"c1\"])",
        "IF(FALSE, YEAR(T[\"zero\", \"c1\"]), 2024)",
        "XNPV(0.1, T[\"n1\"..\"n2\", \"c1\"], T[\"cf0\"..\"cf0\", \"c1\"])",
        "XNPV(0.1, T[\"d0\"..\"d1\", \"c1\"], T[\"yr24\"..\"zero\", \"c1\"])",
        "XIRR(T[\"d0\"..\"d3\", \"c1\"], T[\"cf0\"..\"cf3\", \"c1\"])",
    ] {
        let err = Compiler::compile(&manifest, &[build_model_with_target(domain_err_formula)])
            .expect_err(&format!("expected FunctionError on {domain_err_formula}"));
        assert!(matches!(err.diagnostic(), ErrorDiagnostic::F8Error { .. }));
        assert!(
            matches!(err, CompilerError::Eval(EvalError::Function { .. })),
            "expected EvalError::Function on {domain_err_formula}, got {err:?}"
        );
    }

    // Valid Cluster B scenarios compile and evaluate cleanly
    for (ok_formula, expected_val) in [
        (
            "COUNTA(T[\"n1\"..\"n2\", \"line_item\"])",
            CellValue::Number(2.0),
        ),
        (
            "XLOOKUP(\"COGS\", T[\"n1\"..\"n2\", \"line_item\"], T[\"n1\"..\"n2\", \"c1\"])",
            CellValue::Number(20.0),
        ),
        (
            "SUMIFS(T[\"n1\"..\"n2\", \"c1\"], T[\"n1\"..\"n2\", \"line_item\"], \"COGS\")",
            CellValue::Number(20.0),
        ),
        (
            "IF(ISBLANK(T[\"hole\", \"c1\"]), 0, SUM(T[\"hole\", \"c1\"]))",
            CellValue::Number(0.0),
        ),
        ("DATE(1900, 2, 29)", CellValue::Number(60.0)),
    ] {
        let compiled = Compiler::compile(&manifest, &[build_model_with_target(ok_formula)])
            .unwrap_or_else(|e| panic!("expected ok on {ok_formula}, got {e:?}"));
        assert_eq!(
            compiled
                .cell_value(
                    &TableId::from("T"),
                    &RowId::from("target"),
                    &ColId::from("c1")
                )
                .unwrap(),
            expected_val,
            "mismatch on {ok_formula}"
        );
    }
}
