#![allow(clippy::unwrap_used, clippy::expect_used)]

use indexmap::IndexMap;

use finx::compiler::eval::{EvalError, Evaluator};
use finx::compiler::graph::DependencyGraph;
use finx::compiler::ir::{CellGrid, CellId, CellNode, CellPayload, SymbolTable, expand_grid};
use finx::functions::registry::FunctionRegistry;
use finx::model::id::{ColId, RowId, SheetId, TableId};
use finx::model::table::{CellInput, ColumnSpec, RowSpec, TableSpec, ValueLiteral};
use finx::value::{CellValue, ExcelError};

fn create_table(table_id: &str, cols: &[&str], rows: Vec<RowSpec>) -> TableSpec {
    TableSpec {
        id: TableId::from(table_id),
        title: table_id.into(),
        subtitle: None,
        currency: None,
        columns: cols.iter().map(|&c| ColumnSpec::new(c)).collect(),
        column_groups: Vec::new(),
        row_groups: Vec::new(),
        rows,
    }
}

#[test]
fn test_scenario_1_topological_evaluation_of_pure_literal_grid() {
    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");

    let mut row1_cells = IndexMap::new();
    row1_cells.insert(ColId::from("c1"), CellInput::number(42.0));
    row1_cells.insert(ColId::from("c2"), CellInput::string("FinX"));
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
        Some(&CellValue::Text("FinX".into()))
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
