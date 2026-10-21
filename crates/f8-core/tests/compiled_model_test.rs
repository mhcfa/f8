#![allow(clippy::unwrap_used, clippy::expect_used)]

use f8_core::compiler::emission::EmissionOptions;
use f8_core::compiler::ir::id::CellId;
use f8_core::model::format::{CurrencyCode, FormatPreset, SemanticStyle};
use f8_core::model::id::{ColId, RowId, TableId};
use f8_core::model::theme::{Theme, ThemeSpec};
use f8_core::value::CellValue;
use f8_core::{Compiler, InspectionError};

mod fixtures;

#[test]
fn test_scenario_1_single_pass_compilation() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();

    let compiled =
        Compiler::compile(&manifest, &tables).expect("Single-pass compilation must succeed");

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

    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

    let options = EmissionOptions::default();
    let buffer = compiled
        .emit_workbook(&options)
        .expect("Emission from compiled model must succeed");

    // Must be a valid zip archive starting with PK\x03\x04
    assert!(buffer.starts_with(b"PK\x03\x04"));
    assert!(buffer.len() > 1000);

    // Verify emission via Compiler facade produces matching buffer
    let compiler_buffer = Compiler::emit_workbook(&manifest, &tables, &options)
        .expect("Compiler facade emission must succeed");
    assert_eq!(buffer.len(), compiler_buffer.len());
}

#[test]
fn test_scenario_3_model_overview_summary() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();

    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

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

    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

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
    assert_eq!(formula_cell.formatted_value.as_deref(), Some("$115.0"));
    assert!(formula_cell.is_formula);
    assert!(formula_cell.formula_expr.is_some());
    assert!(formula_cell.excel_formula.is_some());
    assert_eq!(formula_cell.precedents.len(), 2);
    assert!(
        formula_cell
            .precedents
            .iter()
            .all(|p| p.formatted_value.is_some())
    );
    assert!(!formula_cell.dependents.is_empty());
    assert!(
        formula_cell
            .dependents
            .iter()
            .all(|d| d.formatted_value.is_some())
    );

    // 2. Literal input cell: revenue in y2023a
    let literal_cell = compiled
        .inspect_cell(&is_id, &RowId::from("revenue"), &ColId::from("y2023a"))
        .expect("Literal cell inspection must succeed");

    assert_eq!(literal_cell.evaluated_value, CellValue::Number(100.0));
    assert_eq!(literal_cell.formatted_value.as_deref(), Some("$100.0"));
    assert!(!literal_cell.is_formula);
    assert!(literal_cell.formula_expr.is_none());
    assert!(literal_cell.excel_formula.is_none());
    assert!(literal_cell.precedents.is_empty());
    assert!(!literal_cell.dependents.is_empty());

    // 3. Fast-path cell_value and cell_formatted
    let fast_val = compiled
        .cell_value(&is_id, &RowId::from("revenue"), &ColId::from("y2024e"))
        .expect("cell_value must succeed");
    assert!((fast_val.as_number().unwrap() - 115.0).abs() < 1e-6);

    let fast_fmt = compiled
        .cell_formatted(&is_id, &RowId::from("revenue"), &ColId::from("y2024e"))
        .expect("cell_formatted must succeed");
    assert_eq!(fast_fmt, "$115.0");

    assert!(
        compiled
            .cell_value(&is_id, &RowId::from("nonexistent"), &ColId::from("y2024e"))
            .is_err()
    );
    assert!(
        compiled
            .cell_formatted(&is_id, &RowId::from("nonexistent"), &ColId::from("y2024e"))
            .is_err()
    );
}

#[test]
fn test_scenario_5_missing_cell_inspection_error() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();

    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

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
    let (manifest, mut tables) = fixtures::three_statement::build_three_statement_model();
    if let Some(table) = tables
        .iter_mut()
        .find(|t| t.id.as_str() == "IncomeStatement")
    {
        table
            .columns
            .push(f8_core::model::table::ColumnSpec::gutter());
    }

    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

    let is_id = TableId::from("IncomeStatement");
    let table_insp = compiled
        .inspect_table(&is_id)
        .expect("Table inspection must succeed");

    assert_eq!(table_insp.table_id.as_str(), "IncomeStatement");
    assert_eq!(table_insp.title, "Income Statement");
    assert_eq!(table_insp.sheet.as_str(), "Model");
    assert_eq!(table_insp.columns.len(), 6);
    assert_eq!(table_insp.rows.len(), 12);
    assert!(!table_insp.cells.is_empty());

    // Verify column styles: data columns have Some(SemanticStyle::Data), gutter has None
    assert_eq!(table_insp.columns[0].style, Some(SemanticStyle::Data));
    assert_eq!(table_insp.columns[1].style, Some(SemanticStyle::Data));
    assert_eq!(table_insp.columns[5].style, None);

    // Verify row styles: data, subtotal, total, gutter, and section_header
    let rev_row = table_insp
        .rows
        .iter()
        .find(|r| r.id.as_ref().is_some_and(|id| id.as_str() == "revenue"))
        .unwrap();
    assert_eq!(rev_row.style, Some(SemanticStyle::Data));

    let gp_row = table_insp
        .rows
        .iter()
        .find(|r| {
            r.id.as_ref()
                .is_some_and(|id| id.as_str() == "gross_profit")
        })
        .unwrap();
    assert_eq!(gp_row.style, Some(SemanticStyle::Subtotal));

    let ni_row = table_insp
        .rows
        .iter()
        .find(|r| r.id.as_ref().is_some_and(|id| id.as_str() == "net_income"))
        .unwrap();
    assert_eq!(ni_row.style, Some(SemanticStyle::Total));

    let sec_row = table_insp
        .rows
        .iter()
        .find(|r| r.is_section_header)
        .unwrap();
    assert_eq!(sec_row.style, None);

    let gutter_row = table_insp.rows.iter().find(|r| r.is_gutter).unwrap();
    assert_eq!(gutter_row.style, None);

    // Verify cell content
    let rev_2024_id = CellId::new(is_id.clone(), RowId::from("revenue"), ColId::from("y2024e"));
    let cell_info = table_insp
        .cells
        .get(&rev_2024_id)
        .expect("revenue y2024e must exist in table inspection");
    assert!((cell_info.value.as_number().unwrap() - 115.0).abs() < 1e-6);
    assert_eq!(cell_info.formatted_value.as_deref(), Some("$115.0"));
    assert!(cell_info.is_formula);
    assert!(cell_info.excel_formula.is_some());
    assert_eq!(cell_info.format, Some(FormatPreset::Currency));
    assert_eq!(cell_info.decimals, Some(1));
    assert_eq!(cell_info.currency, Some(CurrencyCode::Usd));
}

#[test]
fn test_scenario_table_cell_formatted_value_synthesis_and_negative_numbers() {
    let (manifest, mut tables) = fixtures::three_statement::build_three_statement_model();
    if let Some(table) = tables
        .iter_mut()
        .find(|t| t.id.as_str() == "IncomeStatement")
    {
        table.rows.push(
            fixtures::RowBuilder::line_item("interest_expense", "Interest Expense")
                .format(FormatPreset::Currency)
                .decimals(1)
                .expr("-50.0")
                .build(),
        );
        table.rows.push(
            fixtures::RowBuilder::line_item("unformatted_fee", "Advisory Fee")
                .format(FormatPreset::Currency)
                .expr("100.0")
                .build(),
        );
    }
    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

    let assumptions_insp = compiled
        .inspect_table(&TableId::from("Assumptions"))
        .expect("Assumptions inspection must succeed");
    let rev_growth_cell = assumptions_insp
        .cells
        .get(&CellId::new("Assumptions", "rev_growth", "val"))
        .expect("rev_growth cell must exist");
    assert_eq!(rev_growth_cell.formatted_value.as_deref(), Some("15.0%"));

    let is_insp = compiled
        .inspect_table(&TableId::from("IncomeStatement"))
        .expect("IncomeStatement inspection must succeed");
    let rev_cell = is_insp
        .cells
        .get(&CellId::new("IncomeStatement", "revenue", "y2023a"))
        .expect("revenue cell must exist");
    assert_eq!(rev_cell.formatted_value.as_deref(), Some("$100.0"));

    let interest_cell = is_insp
        .cells
        .get(&CellId::new(
            "IncomeStatement",
            "interest_expense",
            "y2023a",
        ))
        .expect("interest_expense cell must exist");
    assert_eq!(interest_cell.formatted_value.as_deref(), Some("($50.0)"));

    // Verify theme default currency_decimals (0 in corporate_navy, 2 in custom theme)
    assert_eq!(
        compiled
            .cell_formatted(
                &TableId::from("IncomeStatement"),
                &RowId::from("unformatted_fee"),
                &ColId::from("y2023a")
            )
            .unwrap(),
        "$100"
    );
    let mut custom_theme = Theme::corporate_navy();
    custom_theme.number_formats.defaults.currency_decimals = 2;
    let mut custom_manifest = manifest.clone();
    custom_manifest.theme = ThemeSpec::Custom(Box::new(custom_theme));
    let custom_compiled = Compiler::compile(&custom_manifest, &tables)
        .expect("Compilation with custom theme must succeed");
    assert_eq!(
        custom_compiled
            .cell_formatted(
                &TableId::from("IncomeStatement"),
                &RowId::from("unformatted_fee"),
                &ColId::from("y2023a")
            )
            .unwrap(),
        "$100.00"
    );

    // Verify LBO Timeline date & integer formatting in CompiledModel
    let (lbo_manifest, lbo_tables) = fixtures::lbo::build_lbo_model("base");
    let lbo_compiled = Compiler::compile(&lbo_manifest, &lbo_tables).expect("LBO must compile");
    assert_eq!(
        lbo_compiled
            .cell_formatted(
                &TableId::from("Timeline"),
                &RowId::from("date"),
                &ColId::from("y0")
            )
            .unwrap(),
        "2024-12-31"
    );
    assert_eq!(
        lbo_compiled
            .cell_formatted(
                &TableId::from("Timeline"),
                &RowId::from("year"),
                &ColId::from("y5")
            )
            .unwrap(),
        "2029"
    );
}

#[test]
fn test_scenario_7_missing_table_inspection_error() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();

    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

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

    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

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
            node.formatted_value.is_some(),
            "DependencyNode must have formatted_value"
        );
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

    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

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

    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

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

#[test]
fn test_scenario_11_dag_pruning_on_reconvergent_paths() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();

    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

    // Net income depends directly on revenue, cogs, opex, tax, etc.
    // COGS also depends on revenue:
    // NetIncome -> Revenue
    // NetIncome -> COGS -> Revenue
    let net_income_id = CellId::new(
        TableId::from("IncomeStatement"),
        RowId::from("net_income"),
        ColId::from("y2024e"),
    );

    let tree = compiled
        .precedents_of(&net_income_id, true)
        .expect("Recursive precedents of net_income must succeed");

    // Recursively count nodes and find pruned occurrences
    fn find_pruned_nodes(nodes: &[f8_core::compiler::compiled::DependencyNode]) -> usize {
        let mut count = 0;
        for node in nodes {
            if node.is_pruned {
                count += 1;
                assert!(
                    node.children.is_empty(),
                    "Pruned node must have empty children"
                );
            }
            count += find_pruned_nodes(&node.children);
        }
        count
    }

    let pruned_count = find_pruned_nodes(&tree.nodes);
    assert!(
        pruned_count > 0,
        "Reconvergent calculation DAG must prune duplicate subtree expansions"
    );
}

#[test]
fn test_scenario_parse_coordinate() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

    // 1. 3 positional tokens
    let cell_1 = compiled
        .parse_coordinate("IncomeStatement", Some("cogs"), Some("y2024e"))
        .expect("3-arg coordinate parse must succeed");
    assert_eq!(cell_1.table.as_str(), "IncomeStatement");
    assert_eq!(cell_1.row.as_str(), "cogs");
    assert_eq!(cell_1.col.as_str(), "y2024e");

    // 2. Canonical IR string
    let cell_2 = compiled
        .parse_coordinate("IncomeStatement[\"cogs\", \"y2024e\"]", None, None)
        .expect("Canonical IR coordinate parse must succeed");
    assert_eq!(cell_1, cell_2);
}

#[test]
fn test_scenario_inspect_sheet_success() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

    let sheet_id = f8_core::model::id::SheetId::from("Model");
    let sheet = compiled
        .inspect_sheet(&sheet_id)
        .expect("Sheet inspection must succeed");

    assert_eq!(sheet.sheet_id.as_str(), "Model");
    assert_eq!(sheet.title, "Model");
    assert_eq!(sheet.tables.len(), 4);
    assert!(sheet.max_row > sheet.min_row);
    assert!(sheet.max_col > sheet.min_col);
    assert_eq!(
        sheet.rows.len(),
        (sheet.max_row - sheet.min_row + 1) as usize
    );

    // Verify cell kinds exist: TableTitle, ColumnHeader, Data
    let has_title = sheet.rows.iter().any(|r| {
        r.cells
            .iter()
            .any(|c| c.kind == f8_core::compiler::compiled::SheetCellKind::TableTitle)
    });
    let has_header = sheet.rows.iter().any(|r| {
        r.cells
            .iter()
            .any(|c| c.kind == f8_core::compiler::compiled::SheetCellKind::ColumnHeader)
    });
    let has_data = sheet.rows.iter().any(|r| {
        r.cells
            .iter()
            .any(|c| c.kind == f8_core::compiler::compiled::SheetCellKind::Data)
    });

    assert!(has_title, "Sheet inspection must include TableTitle cells");
    assert!(
        has_header,
        "Sheet inspection must include ColumnHeader cells"
    );
    assert!(has_data, "Sheet inspection must include Data cells");

    // Verify data cell contains formula and physical address
    let formula_cell = sheet
        .rows
        .iter()
        .flat_map(|r| &r.cells)
        .find(|c| c.excel_formula.is_some())
        .expect("Sheet must contain at least one formula cell");
    assert!(
        formula_cell.physical_a1.starts_with('B')
            || formula_cell.physical_a1.starts_with('C')
            || formula_cell.physical_a1.starts_with('D')
            || formula_cell.physical_a1.starts_with('E')
            || formula_cell.physical_a1.starts_with('F')
    );
    assert!(
        formula_cell
            .excel_formula
            .as_ref()
            .unwrap()
            .starts_with('=')
    );
    assert!(
        !formula_cell.is_descriptor,
        "Formula calculation cells must not be descriptors"
    );

    // Verify descriptor cell exists and has is_descriptor = true
    let descriptor_cell = sheet
        .rows
        .iter()
        .flat_map(|r| &r.cells)
        .find(|c| c.is_descriptor)
        .expect("Sheet must contain at least one descriptor cell");
    assert_eq!(
        descriptor_cell.kind,
        f8_core::compiler::compiled::SheetCellKind::Data
    );
    assert!(descriptor_cell.excel_formula.is_none());
}

#[test]
fn test_scenario_inspect_sheet_not_found() {
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let compiled = Compiler::compile(&manifest, &tables).expect("Compilation must succeed");

    let err = compiled
        .inspect_sheet(&f8_core::model::id::SheetId::from("NonExistent"))
        .unwrap_err();
    assert_eq!(
        err,
        InspectionError::SheetNotFound(f8_core::model::id::SheetId::from("NonExistent"))
    );
}
