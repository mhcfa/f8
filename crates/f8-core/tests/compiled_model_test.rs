#![allow(clippy::unwrap_used, clippy::expect_used)]

use f8_core::compiler::emission::EmissionOptions;
use f8_core::compiler::ir::id::CellId;
use f8_core::model::id::{ColId, RowId, TableId};
use f8_core::model::theme::Theme;
use f8_core::value::CellValue;
use f8_core::{Compiler, InspectionError};

mod fixtures;

#[test]
fn test_scenario_1_single_pass_compilation() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let theme = Theme::resolve_builtin(Theme::DEFAULT_NAME).expect("Theme should resolve");

    let compiled = Compiler::compile(&manifest, &tables, &theme)
        .expect("Single-pass compilation must succeed");

    assert_eq!(compiled.manifest.model_name, "ThreeStatementFinancialModel");
    assert_eq!(compiled.tables.len(), 4);
    assert!(!compiled.grid.is_empty());
    assert!(compiled.graph.node_count() > 0);
    assert!(!compiled.layout_map.cells.is_empty());
    assert!(!compiled.lowered_map.is_empty());
}

#[test]
fn test_scenario_2_zero_redundancy_emission() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let theme = Theme::resolve_builtin(Theme::DEFAULT_NAME).expect("Theme should resolve");

    let compiled = Compiler::compile(&manifest, &tables, &theme).expect("Compilation must succeed");

    let options = EmissionOptions::default();
    let buffer = compiled
        .emit_workbook(&options)
        .expect("Emission from compiled model must succeed");

    // Must be a valid zip archive starting with PK\x03\x04
    assert!(buffer.starts_with(b"PK\x03\x04"));
    assert!(buffer.len() > 1000);

    // Verify emission via Compiler facade produces matching buffer
    let compiler_buffer = Compiler::emit_workbook(&manifest, &tables, &theme, None, &options)
        .expect("Compiler facade emission must succeed");
    assert_eq!(buffer.len(), compiler_buffer.len());
}

#[test]
fn test_scenario_3_model_overview_summary() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let theme = Theme::resolve_builtin(Theme::DEFAULT_NAME).expect("Theme should resolve");

    let compiled = Compiler::compile(&manifest, &tables, &theme).expect("Compilation must succeed");

    let summary = compiled.summary();
    assert_eq!(summary.model_name, "ThreeStatementFinancialModel");
    assert_eq!(summary.theme_name, "corporate_navy");
    assert_eq!(summary.total_sheets, 1);
    assert_eq!(summary.total_tables, 4);
    assert_eq!(summary.sheets.len(), 1);

    // Every cell must be either a driver (literal) or a formula
    assert_eq!(
        summary.total_cells,
        summary.total_drivers + summary.total_formulas
    );
    assert!(summary.total_drivers > 0);
    assert!(summary.total_formulas > 0);

    // Verify sheet details
    let model_sheet = &summary.sheets[0];
    assert_eq!(model_sheet.name.as_str(), "Model");
    assert_eq!(model_sheet.tables.len(), 4);

    let is_table = model_sheet
        .tables
        .iter()
        .find(|t| t.id.as_str() == "IncomeStatement")
        .expect("IncomeStatement table must exist");
    assert_eq!(is_table.row_count, 12);
    assert_eq!(is_table.col_count, 5);
}

#[test]
fn test_scenario_4_cell_inspection_formula_and_literal() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let theme = Theme::resolve_builtin(Theme::DEFAULT_NAME).expect("Theme should resolve");

    let compiled = Compiler::compile(&manifest, &tables, &theme).expect("Compilation must succeed");

    let is_id = TableId::from("IncomeStatement");

    // 1. Formula cell: revenue in y2024e
    let formula_cell = compiled
        .inspect_cell(&is_id, &RowId::from("revenue"), &ColId::from("y2024e"))
        .expect("Cell inspection must succeed");

    assert_eq!(formula_cell.coordinate.table.as_str(), "IncomeStatement");
    assert_eq!(formula_cell.coordinate.row.as_str(), "revenue");
    assert_eq!(formula_cell.coordinate.col.as_str(), "y2024e");
    assert!(formula_cell.physical_address.starts_with("Model!"));
    assert!(
        (formula_cell.evaluated_value.as_number().unwrap() - 115.0).abs() < 1e-6,
        "Expected evaluated number ~115.0, got: {:?}",
        formula_cell.evaluated_value
    );
    assert!(formula_cell.is_formula);
    assert!(formula_cell.formula_expr.is_some());
    assert!(formula_cell.excel_formula.is_some());
    assert_eq!(formula_cell.precedents.len(), 2);
    assert!(!formula_cell.dependents.is_empty());

    // 2. Literal input cell: revenue in y2023a
    let literal_cell = compiled
        .inspect_cell(&is_id, &RowId::from("revenue"), &ColId::from("y2023a"))
        .expect("Literal cell inspection must succeed");

    assert_eq!(literal_cell.evaluated_value, CellValue::Number(100.0));
    assert!(!literal_cell.is_formula);
    assert!(literal_cell.formula_expr.is_none());
    assert!(literal_cell.excel_formula.is_none());
    assert!(literal_cell.precedents.is_empty());
    assert!(!literal_cell.dependents.is_empty());
}

#[test]
fn test_scenario_5_missing_cell_inspection_error() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let theme = Theme::resolve_builtin(Theme::DEFAULT_NAME).expect("Theme should resolve");

    let compiled = Compiler::compile(&manifest, &tables, &theme).expect("Compilation must succeed");

    let err = compiled
        .inspect_cell(
            &TableId::from("IncomeStatement"),
            &RowId::from("nonexistent_row"),
            &ColId::from("y2024e"),
        )
        .expect_err("Missing cell must return an error");

    match err {
        InspectionError::CellNotFound(id) => {
            assert_eq!(id.row.as_str(), "nonexistent_row");
        }
        other => panic!("Expected CellNotFound, got: {other:?}"),
    }
}

#[test]
fn test_scenario_6_table_matrix_inspection() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let theme = Theme::resolve_builtin(Theme::DEFAULT_NAME).expect("Theme should resolve");

    let compiled = Compiler::compile(&manifest, &tables, &theme).expect("Compilation must succeed");

    let is_id = TableId::from("IncomeStatement");
    let table_insp = compiled
        .inspect_table(&is_id)
        .expect("Table inspection must succeed");

    assert_eq!(table_insp.table_id.as_str(), "IncomeStatement");
    assert_eq!(table_insp.title, "Income Statement");
    assert_eq!(table_insp.sheet.as_str(), "Model");
    assert_eq!(table_insp.columns.len(), 5);
    assert_eq!(table_insp.rows.len(), 12);
    assert!(!table_insp.cells.is_empty());

    // Verify cell content
    let rev_2024_id = CellId::new(is_id.clone(), RowId::from("revenue"), ColId::from("y2024e"));
    let cell_info = table_insp
        .cells
        .get(&rev_2024_id)
        .expect("revenue y2024e must exist in table inspection");
    assert!((cell_info.value.as_number().unwrap() - 115.0).abs() < 1e-6);
    assert!(cell_info.is_formula);
    assert!(cell_info.excel_formula.is_some());
}

#[test]
fn test_scenario_7_missing_table_inspection_error() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let theme = Theme::resolve_builtin(Theme::DEFAULT_NAME).expect("Theme should resolve");

    let compiled = Compiler::compile(&manifest, &tables, &theme).expect("Compilation must succeed");

    let err = compiled
        .inspect_table(&TableId::from("NonExistentTable"))
        .expect_err("Missing table must return an error");

    match err {
        InspectionError::TableNotFound(id) => {
            assert_eq!(id.as_str(), "NonExistentTable");
        }
        other => panic!("Expected TableNotFound, got: {other:?}"),
    }
}

#[test]
fn test_scenario_8_precedents_of_direct_and_recursive() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let theme = Theme::resolve_builtin(Theme::DEFAULT_NAME).expect("Theme should resolve");

    let compiled = Compiler::compile(&manifest, &tables, &theme).expect("Compilation must succeed");

    let ni_id = CellId::new(
        TableId::from("IncomeStatement"),
        RowId::from("net_income"),
        ColId::from("y2024e"),
    );

    // 1. Direct precedents
    let direct_tree = compiled
        .precedents_of(&ni_id, false)
        .expect("Direct precedents query must succeed");

    assert_eq!(direct_tree.root, ni_id);
    assert_eq!(direct_tree.direct_count, 2);
    assert_eq!(direct_tree.transitive_count, 2);
    assert_eq!(direct_tree.nodes.len(), 2);
    for node in &direct_tree.nodes {
        assert!(
            node.children.is_empty(),
            "Direct query must not nest children"
        );
    }

    // 2. Recursive precedents
    let recursive_tree = compiled
        .precedents_of(&ni_id, true)
        .expect("Recursive precedents query must succeed");

    assert_eq!(recursive_tree.root, ni_id);
    assert_eq!(recursive_tree.direct_count, 2);
    assert!(
        recursive_tree.transitive_count > 2,
        "Transitive precedent count must be > 2, got: {}",
        recursive_tree.transitive_count
    );
    assert_eq!(recursive_tree.nodes.len(), 2);

    // Children of direct nodes must have nested sub-precedents
    let has_grandchildren = recursive_tree.nodes.iter().any(|n| !n.children.is_empty());
    assert!(
        has_grandchildren,
        "Recursive precedent tree must contain nested children"
    );
}

#[test]
fn test_scenario_9_dependents_of_direct_and_recursive() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let theme = Theme::resolve_builtin(Theme::DEFAULT_NAME).expect("Theme should resolve");

    let compiled = Compiler::compile(&manifest, &tables, &theme).expect("Compilation must succeed");

    let growth_id = CellId::new(
        TableId::from("Assumptions"),
        RowId::from("rev_growth"),
        ColId::from("val"),
    );

    // 1. Direct dependents
    let direct_tree = compiled
        .dependents_of(&growth_id, false)
        .expect("Direct dependents query must succeed");

    assert_eq!(direct_tree.root, growth_id);
    assert_eq!(direct_tree.direct_count, 3); // revenue in y2024e, y2025e, y2026e
    assert_eq!(direct_tree.transitive_count, 3);
    for node in &direct_tree.nodes {
        assert_eq!(node.cell_id.row.as_str(), "revenue");
        assert!(node.children.is_empty());
    }

    // 2. Recursive dependents (blast radius)
    let recursive_tree = compiled
        .dependents_of(&growth_id, true)
        .expect("Recursive dependents query must succeed");

    assert_eq!(recursive_tree.root, growth_id);
    assert_eq!(recursive_tree.direct_count, 3);
    // Downstream blast radius reaches: revenue, cogs, gross_profit, sga, opex, ebit, ebt, tax, net_income, balance sheet, cash flow across timeline
    assert!(
        recursive_tree.transitive_count >= 10,
        "Downstream blast radius must affect >= 10 cells, got: {}",
        recursive_tree.transitive_count
    );
    assert!(!recursive_tree.nodes[0].children.is_empty());
}

#[test]
fn test_scenario_10_missing_cell_precedents_and_dependents_error() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let theme = Theme::resolve_builtin(Theme::DEFAULT_NAME).expect("Theme should resolve");

    let compiled = Compiler::compile(&manifest, &tables, &theme).expect("Compilation must succeed");

    let bad_cell = CellId::new(
        TableId::from("IncomeStatement"),
        RowId::from("nonexistent"),
        ColId::from("y2024e"),
    );

    let err_prec = compiled
        .precedents_of(&bad_cell, false)
        .expect_err("Missing cell precedents must fail");
    assert_eq!(err_prec, InspectionError::CellNotFound(bad_cell.clone()));

    let err_dep = compiled
        .dependents_of(&bad_cell, false)
        .expect_err("Missing cell dependents must fail");
    assert_eq!(err_dep, InspectionError::CellNotFound(bad_cell));
}
