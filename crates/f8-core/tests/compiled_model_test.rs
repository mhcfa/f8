#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use f8_core::compiler::emission::EmissionOptions;
use f8_core::compiler::ir::id::CellId;
use f8_core::model::format::{CurrencyCode, FormatPreset, SemanticStyle};
use f8_core::model::id::{ColId, RowId, SheetId, TableId};
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{ColumnSpec, RowSpec, TableSpec};
use f8_core::model::theme::{Theme, ThemeSpec};
use f8_core::value::{CellValue, ExcelError};
use f8_core::{
    CellError, CompiledModel, Compiler, InspectionError, ModelMarkdownDetail, ModelMarkdownOptions,
};

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

    // Verify emission via Compiler facade produces identical bytes
    let compiler_buffer = Compiler::emit_workbook(&manifest, &tables, &options)
        .expect("Compiler facade emission must succeed");
    assert_eq!(buffer, compiler_buffer);
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
    assert!(formula_cell.physical_address.starts_with("'Model'!"));
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

    let InspectionError::CellNotFound(id) = err else {
        panic!("Expected CellNotFound, got: {err:?}");
    };
    assert_eq!(id.row.as_str(), "nonexistent_row");
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

    // Verify column styles and groups: data columns have Some(SemanticStyle::Data), gutter has None
    assert_eq!(table_insp.columns[0].style, Some(SemanticStyle::Data));
    assert_eq!(table_insp.columns[0].group, None);
    assert_eq!(table_insp.columns[1].style, Some(SemanticStyle::Data));
    assert_eq!(
        table_insp.columns[1].group.as_deref(),
        Some("Historical Actuals")
    );
    assert_eq!(table_insp.columns[1].headers, vec!["2023A".to_string()]);
    assert_eq!(
        table_insp.columns[2].group.as_deref(),
        Some("Forecast Projections")
    );
    assert_eq!(table_insp.columns[5].style, None);
    assert_eq!(table_insp.columns[5].group, None);

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

    let InspectionError::TableNotFound(id) = err else {
        panic!("Expected TableNotFound, got: {err:?}");
    };
    assert_eq!(id.as_str(), "NonExistentTable");
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
    let direct_cols: Vec<&str> = direct_tree
        .nodes
        .iter()
        .map(|n| n.cell_id.col.as_str())
        .collect();
    assert_eq!(direct_cols, vec!["y2024e", "y2025e", "y2026e"]);
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

    // Verify ColumnHeader cells include both group banners (Tier 0) and metric headers (Tier 1)
    let header_texts: Vec<&str> = sheet
        .rows
        .iter()
        .flat_map(|r| &r.cells)
        .filter(|c| c.kind == f8_core::compiler::compiled::SheetCellKind::ColumnHeader)
        .filter_map(|c| c.formatted_value.as_deref())
        .collect();
    assert!(header_texts.contains(&"Historical Actuals"));
    assert!(header_texts.contains(&"Forecast Projections"));
    assert!(header_texts.contains(&"2023A"));
    assert!(header_texts.contains(&"2024E"));

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

#[test]
fn test_sheet_address_quoting_in_inspection_and_dependency_tree() {
    use f8_core::model::{ColumnSpec, ProjectManifest, RowSpec, SheetSpec, TableSpec, ThemeSpec};

    let manifest = ProjectManifest {
        model_name: "QuotingTest".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![
            SheetSpec {
                name: "Sheet1".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("SimpleTbl")],
            },
            SheetSpec {
                name: "Cash Flow".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("CashTbl")],
            },
            SheetSpec {
                name: "John's Data".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("QuoteTbl")],
            },
        ],
    };

    let mut simple_tbl = TableSpec::new("SimpleTbl", "Simple");
    simple_tbl.columns.push(ColumnSpec::new("c1"));
    simple_tbl
        .rows
        .push(RowSpec::new("r1").with_cell("c1", 10.0));

    let mut cash_tbl = TableSpec::new("CashTbl", "Cash");
    cash_tbl.columns.push(ColumnSpec::new("c1"));
    cash_tbl
        .rows
        .push(RowSpec::new("r1").with_expr("SimpleTbl[\"r1\", \"c1\"] * 2"));

    let mut quote_tbl = TableSpec::new("QuoteTbl", "Quote");
    quote_tbl.columns.push(ColumnSpec::new("c1"));
    quote_tbl
        .rows
        .push(RowSpec::new("r1").with_expr("CashTbl[\"r1\", \"c1\"] + 5"));

    let compiled = Compiler::compile(&manifest, &[simple_tbl, cash_tbl, quote_tbl]).unwrap();

    let simple_cell = compiled
        .inspect_cell(
            &TableId::from("SimpleTbl"),
            &RowId::from("r1"),
            &ColId::from("c1"),
        )
        .unwrap();
    assert_eq!(simple_cell.physical_address, "'Sheet1'!B4");
    assert_eq!(simple_cell.dependents[0].sheet_address, "'Cash Flow'!B4");

    let cash_cell = compiled
        .inspect_cell(
            &TableId::from("CashTbl"),
            &RowId::from("r1"),
            &ColId::from("c1"),
        )
        .unwrap();
    assert_eq!(cash_cell.physical_address, "'Cash Flow'!B4");
    assert_eq!(cash_cell.precedents[0].sheet_address, "'Sheet1'!B4");
    assert_eq!(cash_cell.dependents[0].sheet_address, "'John''s Data'!B4");

    let quote_cell = compiled
        .inspect_cell(
            &TableId::from("QuoteTbl"),
            &RowId::from("r1"),
            &ColId::from("c1"),
        )
        .unwrap();
    assert_eq!(quote_cell.physical_address, "'John''s Data'!B4");

    let tree = compiled
        .precedents_of(&CellId::new("QuoteTbl", "r1", "c1"), true)
        .unwrap();
    assert_eq!(tree.nodes[0].sheet_address, "'Cash Flow'!B4");
    assert_eq!(tree.nodes[0].children[0].sheet_address, "'Sheet1'!B4");
}

#[test]
fn test_scenario_15_coordinate_parsing_strictness_and_special_key_roundtrip() {
    use f8_core::model::{ColumnSpec, ProjectManifest, RowSpec, SheetSpec, TableSpec};

    let manifest = ProjectManifest {
        model_name: "CoordStrictModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("T1")],
        }],
    };

    let special_keys = [
        r#"C:\ops\"item""#,
        "[bracket, key]",
        "a]b",
        "it's",
        "  padded  ",
        "@row",
    ];

    let mut tbl = TableSpec::new("T1", "Special Keys");
    for key in special_keys {
        tbl.columns.push(ColumnSpec::new(key));
    }
    for (idx, r_key) in special_keys.iter().enumerate() {
        let mut row = RowSpec::new(*r_key);
        for (c_idx, c_key) in special_keys.iter().enumerate() {
            row = row.with_cell(*c_key, (idx * 10 + c_idx) as f64);
        }
        tbl.rows.push(row);
    }

    let compiled = Compiler::compile(&manifest, &[tbl]).unwrap();
    let insp = compiled.inspect_table(&TableId::from("T1")).unwrap();
    let roundtripped: f8_core::compiler::TableInspection =
        serde_json::from_str(&serde_json::to_string(&insp).unwrap()).unwrap();
    assert_eq!(roundtripped, insp);

    for r_key in special_keys {
        for c_key in special_keys {
            let expected = CellId::new("T1", r_key, c_key);
            assert_eq!(
                compiled
                    .parse_coordinate(&expected.to_string(), None, None)
                    .unwrap(),
                expected
            );
            assert_eq!(
                compiled
                    .parse_coordinate("T1", Some(r_key), Some(c_key))
                    .unwrap(),
                expected
            );
        }
    }

    // Dynamic/range TableRefs, unbalanced quotes, and invalid TableIds must fail with InvalidCoordinate
    for bad_coord in [
        "T1[@row, @col]",
        "T1[row.prev, col.next]",
        "@table[r1, c1]",
        "T1[r1..r2, c1]",
        "T1['r1', @col]",
        "T1['cogs, 'y2024e']",
        "T1['a'b', 'c']",
        "123[r1, c1]",
        "1e5[r1, c1]",
    ] {
        let err = compiled
            .parse_coordinate(bad_coord, None, None)
            .unwrap_err();
        assert!(
            matches!(err, InspectionError::InvalidCoordinate(_)),
            "expected InvalidCoordinate for {bad_coord:?}, got {err:?}"
        );
    }
    assert!(matches!(
        compiled
            .parse_coordinate("\"T1", Some("r1"), Some("c1"))
            .unwrap_err(),
        InspectionError::InvalidCoordinate(_)
    ));
    assert!(matches!(
        compiled
            .parse_coordinate("123", Some("r1"), Some("c1"))
            .unwrap_err(),
        InspectionError::InvalidCoordinate(_)
    ));
}

fn build_parity_workbook() -> CompiledModel {
    use f8_core::model::{
        CellInput, ColumnSpec, DetailedCellInput, ProjectManifest, RowSpec, SheetSpec, TableSpec,
        ValueLiteral,
    };

    let manifest = ProjectManifest {
        model_name: "ParityModel".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![
            SheetSpec {
                name: "Financials".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![
                    TableId::from("IncomeStatement"),
                    TableId::from("PureMeasures"),
                ],
            },
            SheetSpec {
                name: "Valuation".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("Comps")],
            },
            SheetSpec {
                name: "Notes".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![],
            },
        ],
    };

    let mut is_tbl = TableSpec::new("IncomeStatement", "Income Statement")
        .with_subtitles(["($ in Millions)", "Fiscal Year"])
        .with_currency(Some(CurrencyCode::Usd));
    is_tbl.columns = vec![
        ColumnSpec::descriptor("line_item", "Line Item"),
        ColumnSpec::new("y2023a")
            .with_group("Historical")
            .with_header("2023A")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1)),
        ColumnSpec::gutter(),
        ColumnSpec::new("2024E")
            .with_header("2024E")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1)),
    ];
    is_tbl.rows = vec![
        RowSpec::section_header("Revenue & Margin"),
        RowSpec::new("revenue")
            .with_cell("line_item", "Revenue")
            .with_cell("y2023a", 100.0)
            .with_cell(
                "2024E",
                CellInput::formula(r#"IncomeStatement["revenue", col.prev] * 1.15"#),
            ),
        RowSpec::new("cogs")
            .with_cell(
                "line_item",
                DetailedCellInput {
                    value: Some(ValueLiteral::String("Cost | Goods\nSold".to_string())),
                    indent: Some(1),
                    ..Default::default()
                },
            )
            .with_cell("y2023a", 40.0)
            .with_cell("2024E", 44.0),
        RowSpec::new("sparse")
            .with_cell("line_item", "Memo")
            .with_cell("y2023a", 5.0)
            .with_cell("2024E", CellInput::null()),
        RowSpec::gutter(),
        RowSpec::new("gross_profit")
            .with_style(Some(SemanticStyle::Subtotal))
            .with_cell("line_item", "Gross Profit")
            .with_expr(r#"IncomeStatement["revenue", @col] - IncomeStatement["cogs", @col]"#),
        RowSpec::section_header("Checks | Errors"),
        RowSpec::new("div_err")
            .with_style(Some(SemanticStyle::Total))
            .with_cell("line_item", "Net | Check")
            .with_cell("y2023a", 10.0)
            .with_cell("2024E", CellInput::formula("1 / 0")),
    ];

    let mut pure_tbl = TableSpec::new("PureMeasures", "PureMeasures");
    pure_tbl.columns = vec![
        ColumnSpec::new("c1").with_header("c1"),
        ColumnSpec::new("c2").with_header("Col Two"),
    ];
    pure_tbl.rows = vec![
        RowSpec::new("r1")
            .with_cell("c1", 1.0)
            .with_cell("c2", CellInput::formula("NA()")),
        RowSpec::new("r2")
            .with_style(Some(SemanticStyle::Total))
            .with_cell("c1", 2.0)
            .with_cell("c2", 3.0),
    ];

    let mut comps_tbl =
        TableSpec::new("Comps", "Trading Comps").with_currency(Some(CurrencyCode::Usd));
    comps_tbl.columns = vec![
        ColumnSpec::descriptor("ticker", "Ticker"),
        ColumnSpec::descriptor("company", "Company Name"),
        ColumnSpec::new("ev")
            .with_header("EV")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1)),
    ];
    comps_tbl.rows = vec![
        RowSpec::new("aapl")
            .with_cell("ticker", "AAPL")
            .with_cell("company", "Apple Inc.")
            .with_cell("ev", 3000.0),
        RowSpec::new("msft_row")
            .with_cell("ticker", "MSFT")
            .with_cell("company", "Microsoft")
            .with_cell("ev", CellInput::formula("LN(-1)")),
    ];

    Compiler::compile(&manifest, &[is_tbl, pure_tbl, comps_tbl])
        .expect("Parity workbook compilation must succeed")
}

#[test]
fn test_cell_error_serde_roundtrip_all_variants() {
    let variants = [
        (ExcelError::Div0, "#DIV/0!"),
        (ExcelError::Value, "#VALUE!"),
        (ExcelError::Ref, "#REF!"),
        (ExcelError::Num, "#NUM!"),
        (ExcelError::Na, "#N/A"),
        (ExcelError::Name, "#NAME?"),
    ];

    for (variant, expected_code) in variants {
        let record = CellError {
            coordinate: CellId::new("T", "r1", "c1"),
            sheet: SheetId::from("Financials"),
            physical_address: "'Financials'!C14".to_string(),
            error: variant,
            formula_expr: Some("1 / 0".to_string()),
            excel_formula: Some("=1/0".to_string()),
        };
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["error"], expected_code);
        assert_eq!(json["physical_address"], "'Financials'!C14");

        let roundtripped: CellError = serde_json::from_value(json).unwrap();
        assert_eq!(roundtripped, record);
    }

    let invalid = serde_json::json!({
        "coordinate": { "table": "T", "row": "r1", "col": "c1" },
        "sheet": "Financials",
        "physical_address": "'Financials'!C14",
        "error": "#BOGUS!",
        "formula_expr": null,
        "excel_formula": null
    });
    assert!(serde_json::from_value::<CellError>(invalid).is_err());

    // ModelMarkdownOptions serde defaults and deny_unknown_fields enforcement
    let opts_default: ModelMarkdownOptions = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(opts_default, ModelMarkdownOptions::default());
    assert_eq!(opts_default.detail, ModelMarkdownDetail::Values);
    assert_eq!(opts_default.sheets, None);
    assert_eq!(opts_default.tables, None);
    assert!(opts_default.show_ids);

    let opts_unknown = serde_json::json!({ "unknown_field": true });
    assert!(serde_json::from_value::<ModelMarkdownOptions>(opts_unknown).is_err());
}

#[test]
fn test_errors_clean_model_and_multi_sheet_ordering() {
    // Scenario 1: Clean workbook returns empty list
    let (manifest, tables) = fixtures::three_statement::build_three_statement_model();
    let clean = Compiler::compile(&manifest, &tables).unwrap();
    assert!(clean.errors().is_empty());

    // Scenario 2: Multi-sheet errors returned in deterministic sheet -> table -> row -> col order
    let compiled = build_parity_workbook();
    let errors = compiled.errors();
    assert_eq!(errors.len(), 3);

    assert_eq!(
        errors[0],
        CellError {
            coordinate: CellId::new("IncomeStatement", "div_err", "2024E"),
            sheet: SheetId::from("Financials"),
            physical_address: "'Financials'!E15".to_string(),
            error: ExcelError::Div0,
            formula_expr: Some("1 / 0".to_string()),
            excel_formula: Some("=1/0".to_string()),
        }
    );
    assert_eq!(
        errors[1],
        CellError {
            coordinate: CellId::new("PureMeasures", "r1", "c2"),
            sheet: SheetId::from("Financials"),
            physical_address: "'Financials'!C21".to_string(),
            error: ExcelError::Na,
            formula_expr: Some("NA()".to_string()),
            excel_formula: Some("=NA()".to_string()),
        }
    );
    assert_eq!(
        errors[2],
        CellError {
            coordinate: CellId::new("Comps", "msft_row", "ev"),
            sheet: SheetId::from("Valuation"),
            physical_address: "'Valuation'!D6".to_string(),
            error: ExcelError::Num,
            formula_expr: Some("LN(-1)".to_string()),
            excel_formula: Some("=LN(-1)".to_string()),
        }
    );
}

#[test]
fn test_to_markdown_default_values_index_and_parity() {
    let compiled = build_parity_workbook();

    // Scenario 3, 5, 6, 10: Default values mode with show_ids=true
    let md_values = compiled
        .to_markdown(&ModelMarkdownOptions::default())
        .unwrap();
    let expected_values = "\
# ParityModel
Sheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3

## Runtime Errors

- `IncomeStatement[\"div_err\", \"2024E\"]` (`'Financials'!E15`): `#DIV/0!` (`=1/0`)
- `PureMeasures[\"r1\", \"c2\"]` (`'Financials'!C21`): `#N/A` (`=NA()`)
- `Comps[\"msft_row\", \"ev\"]` (`'Valuation'!D6`): `#NUM!` (`=LN(-1)`)

## Sheet: Financials

### `IncomeStatement` — Income Statement
_($ in Millions)_
_Fiscal Year_

| Line Item [line_item] | Historical: 2023A [y2023a] | 2024E |
| :--- | ---: | ---: |
| **Revenue & Margin** | | |
| Revenue | $100.0 | $115.0 |
|   Cost \\| Goods Sold [cogs] | $40.0 | $44.0 |
| Memo [sparse] | $5.0 | — |
| **Gross Profit [gross_profit]** | **$60.0** | **$71.0** |
| **Checks \\| Errors** | | |
| **Net \\| Check [div_err]** | **$10.0** | **#DIV/0!** |

### `PureMeasures`

| Row | c1 | Col Two [c2] |
| :--- | ---: | ---: |
| r1 | 1 | #N/A |
| **r2** | **2** | **3** |

## Sheet: Valuation

### `Comps` — Trading Comps

| Ticker | Company Name [company] | EV |
| :--- | :--- | ---: |
| AAPL | Apple Inc. | $3,000.0 |
| MSFT [msft_row] | Microsoft | #NUM! |

## Sheet: Notes

_(no tables)_
";
    assert_eq!(md_values, expected_values);
    // Determinism check
    assert_eq!(
        compiled
            .to_markdown(&ModelMarkdownOptions::default())
            .unwrap(),
        expected_values
    );

    // Scenario 5: show_ids=false omits synthetic Row column and [row_id]/[col_id] annotations
    let md_no_ids = compiled
        .to_markdown(&ModelMarkdownOptions {
            show_ids: false,
            ..Default::default()
        })
        .unwrap();
    assert!(!md_no_ids.contains("| Row |"));
    assert!(!md_no_ids.contains("[line_item]"));
    assert!(!md_no_ids.contains("[y2023a]"));
    assert!(!md_no_ids.contains("[gross_profit]"));
    assert!(!md_no_ids.contains("[msft_row]"));
    assert!(md_no_ids.contains("| c1 | Col Two |"));
    assert!(md_no_ids.contains("| MSFT | Microsoft | #NUM! |"));

    // Scenario 4: Index mode (ignores show_ids=false)
    let md_index = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Index,
            show_ids: false,
            ..Default::default()
        })
        .unwrap();
    let expected_index = "\
# ParityModel
Sheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3

## Runtime Errors

- `IncomeStatement[\"div_err\", \"2024E\"]` (`'Financials'!E15`): `#DIV/0!` (`=1/0`)
- `PureMeasures[\"r1\", \"c2\"]` (`'Financials'!C21`): `#N/A` (`=NA()`)
- `Comps[\"msft_row\", \"ev\"]` (`'Valuation'!D6`): `#NUM!` (`=LN(-1)`)

## Sheet: Financials

### `IncomeStatement` — Income Statement
_($ in Millions)_
_Fiscal Year_

Columns: `line_item` (Line Item, descriptor), `y2023a` (Historical: 2023A), `2024E`
Rows:
- **Revenue & Margin**
  - `revenue`
  - `cogs`: Cost | Goods Sold
  - `sparse`: Memo
  - **`gross_profit`**: Gross Profit (subtotal)
- **Checks | Errors**
  - **`div_err`**: Net | Check (total)

### `PureMeasures`

Columns: `c1`, `c2` (Col Two)
Rows:
- `r1`
- **`r2`** (total)

## Sheet: Valuation

### `Comps` — Trading Comps

Columns: `ticker` (descriptor), `company` (Company Name, descriptor), `ev`
Rows:
- `aapl`: AAPL | Apple Inc.
- `msft_row`: MSFT | Microsoft

## Sheet: Notes

_(no tables)_
";
    assert_eq!(md_index, expected_index);
    assert_eq!(
        compiled
            .to_markdown(&ModelMarkdownOptions {
                detail: ModelMarkdownDetail::Index,
                show_ids: true,
                ..Default::default()
            })
            .unwrap(),
        expected_index
    );
}

#[test]
fn test_to_markdown_filtering_errors_clean_and_blank_model_name() {
    use f8_core::model::{ColumnSpec, ProjectManifest, RowSpec, SheetSpec, TableSpec};

    let compiled = build_parity_workbook();

    // Scenario 6: Filtered to Valuation shows total Errors: 3 in header but only Comps error in Runtime Errors
    let md_val = compiled
        .to_markdown(&ModelMarkdownOptions {
            sheets: Some(vec![SheetId::from("Valuation")]),
            ..Default::default()
        })
        .unwrap();
    assert!(md_val.contains("| Errors: 3\n"));
    assert!(md_val.contains("## Runtime Errors"));
    assert!(md_val.contains(r#"Comps["msft_row", "ev"]"#));
    assert!(!md_val.contains("IncomeStatement"));
    assert!(!md_val.contains("PureMeasures"));

    // Filtered to Notes (empty sheet): 0 errors in scope -> omits ## Runtime Errors, emits _(no tables)_
    let md_notes = compiled
        .to_markdown(&ModelMarkdownOptions {
            sheets: Some(vec![SheetId::from("Notes")]),
            ..Default::default()
        })
        .unwrap();
    assert!(md_notes.contains("| Errors: 3\n"));
    assert!(!md_notes.contains("## Runtime Errors"));
    assert!(md_notes.contains("## Sheet: Notes\n\n_(no tables)_\n"));

    // Standalone non-empty tables-only filter (sheets: None, tables: Some(...))
    let md_tables_only = compiled
        .to_markdown(&ModelMarkdownOptions {
            sheets: None,
            tables: Some(vec![TableId::from("PureMeasures")]),
            ..Default::default()
        })
        .unwrap();
    assert!(md_tables_only.contains("| Errors: 3\n"));
    assert!(md_tables_only.contains("## Runtime Errors"));
    assert!(md_tables_only.contains(r#"PureMeasures["r1", "c2"]"#));
    assert!(!md_tables_only.contains("IncomeStatement"));
    assert!(!md_tables_only.contains("Comps"));
    assert!(md_tables_only.contains("## Sheet: Financials"));
    assert!(!md_tables_only.contains("## Sheet: Valuation"));
    assert!(!md_tables_only.contains("## Sheet: Notes"));

    // Scenario 7: Union of sheets + tables with overlap and duplicates preserves declaration order
    let md_union = compiled
        .to_markdown(&ModelMarkdownOptions {
            sheets: Some(vec![SheetId::from("Valuation"), SheetId::from("Valuation")]),
            tables: Some(vec![
                TableId::from("Comps"),
                TableId::from("PureMeasures"),
                TableId::from("PureMeasures"),
            ]),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(md_union.matches("### `Comps`").count(), 1);
    assert_eq!(md_union.matches("### `PureMeasures`").count(), 1);
    assert!(!md_union.contains("### `IncomeStatement`"));
    // Financials precedes Valuation in manifest order even though Valuation was listed first in filter
    let pos_fin = md_union.find("## Sheet: Financials").unwrap();
    let pos_val = md_union.find("## Sheet: Valuation").unwrap();
    assert!(pos_fin < pos_val);

    // Explicit empty allowlists emit only the model header
    let expected_header_only =
        "# ParityModel\nSheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3\n";
    assert_eq!(
        compiled
            .to_markdown(&ModelMarkdownOptions {
                sheets: Some(vec![]),
                tables: None,
                ..Default::default()
            })
            .unwrap(),
        expected_header_only
    );
    assert_eq!(
        compiled
            .to_markdown(&ModelMarkdownOptions {
                sheets: None,
                tables: Some(vec![]),
                ..Default::default()
            })
            .unwrap(),
        expected_header_only
    );
    assert_eq!(
        compiled
            .to_markdown(&ModelMarkdownOptions {
                sheets: Some(vec![]),
                tables: Some(vec![]),
                ..Default::default()
            })
            .unwrap(),
        expected_header_only
    );

    // Unknown sheet / unknown table fail strictly with InspectionError
    assert_eq!(
        compiled
            .to_markdown(&ModelMarkdownOptions {
                sheets: Some(vec![SheetId::from("MissingSheet")]),
                ..Default::default()
            })
            .unwrap_err(),
        InspectionError::SheetNotFound(SheetId::from("MissingSheet"))
    );
    assert_eq!(
        compiled
            .to_markdown(&ModelMarkdownOptions {
                tables: Some(vec![TableId::from("MissingTable")]),
                ..Default::default()
            })
            .unwrap_err(),
        InspectionError::TableNotFound(TableId::from("MissingTable"))
    );

    // Clean model with blank model_name falls back to "# Model" and omits ## Runtime Errors
    let manifest = ProjectManifest {
        model_name: "  \r\n  ".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Main".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("Summary")],
        }],
    };
    let mut tbl = TableSpec::new("Summary", "");
    tbl.columns = vec![ColumnSpec::new("v")];
    tbl.rows = vec![RowSpec::new("x").with_cell("v", 42.0)];
    let clean = Compiler::compile(&manifest, &[tbl]).unwrap();
    let clean_md = clean.to_markdown(&ModelMarkdownOptions::default()).unwrap();
    assert!(clean_md.starts_with(
        "# Model\nSheets: 1 | Tables: 1 | Cells: 1 (1 drivers, 0 formulas) | Errors: 0\n"
    ));
    assert!(!clean_md.contains("## Runtime Errors"));
    assert!(clean_md.contains("### `Summary`\n"));

    // Degenerate gutter-only-columns table emits "Columns:" without trailing space in Index mode
    let mut gutter_tbl = TableSpec::new("GutterCols", "Gutter Cols");
    gutter_tbl.columns = vec![ColumnSpec::gutter()];
    gutter_tbl.rows = vec![RowSpec::section_header("Only Section")];
    let gutter_manifest = ProjectManifest {
        model_name: "GutterModel".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Main".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("GutterCols")],
        }],
    };
    let gutter_compiled = Compiler::compile(&gutter_manifest, &[gutter_tbl]).unwrap();
    let gutter_index_md = gutter_compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Index,
            ..Default::default()
        })
        .unwrap();
    assert!(gutter_index_md.contains("\nColumns:\nRows:\n- **Only Section**\n"));
    assert!(!gutter_index_md.contains("Columns: \n"));

    // Whitespace-only descriptor cells are treated as empty symmetrically across Values and Index
    let mut ws_tbl = TableSpec::new("WhitespaceDesc", "WhitespaceDesc");
    ws_tbl.columns = vec![
        ColumnSpec::descriptor("d1", "D1"),
        ColumnSpec::descriptor("d2", "D2"),
        ColumnSpec::new("val").with_header("Val"),
    ];
    ws_tbl.rows = vec![
        RowSpec::new("r_blank")
            .with_cell("d1", "  \n  ")
            .with_cell("d2", " \r\n ")
            .with_cell("val", 7.0),
    ];
    let ws_manifest = ProjectManifest {
        model_name: "WsModel".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Main".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("WhitespaceDesc")],
        }],
    };
    let ws_compiled = Compiler::compile(&ws_manifest, &[ws_tbl]).unwrap();
    let ws_values_md = ws_compiled
        .to_markdown(&ModelMarkdownOptions::default())
        .unwrap();
    assert!(ws_values_md.contains("| r_blank | | 7 |\n"));
    let ws_index_md = ws_compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Index,
            ..Default::default()
        })
        .unwrap();
    assert!(ws_index_md.contains("Rows:\n- `r_blank`\n"));
}

#[test]
fn test_bold_style_inspection_and_markdown_rendering() {
    let mut tbl = TableSpec::new("Summary", "Summary");
    tbl.columns = vec![
        ColumnSpec::descriptor("item", "Metric"),
        ColumnSpec::new("base")
            .with_header("Base Case")
            .with_style(Some(SemanticStyle::Bold)),
        ColumnSpec::new("upside").with_header("Upside"),
    ];
    tbl.rows = vec![
        RowSpec::new("rev")
            .with_format(Some(FormatPreset::Currency))
            .with_cell("item", "Revenue")
            .with_cell("base", 100.0)
            .with_cell("upside", 120.0),
        RowSpec::new("ebitda_margin")
            .with_style(Some(SemanticStyle::Bold))
            .with_format(Some(FormatPreset::Percentage))
            .with_cell("item", "EBITDA Margin")
            .with_cell("base", 0.25)
            .with_cell("upside", 0.28),
        RowSpec::new("ebitda")
            .with_style(Some(SemanticStyle::Subtotal))
            .with_format(Some(FormatPreset::Currency))
            .with_cell("item", "EBITDA")
            .with_cell("base", 25.0)
            .with_cell("upside", 33.6),
        RowSpec::new("net_income")
            .with_style(Some(SemanticStyle::Total))
            .with_format(Some(FormatPreset::Currency))
            .with_cell("item", "Net Income")
            .with_cell("base", 18.0)
            .with_cell("upside", 24.0),
    ];

    let manifest = ProjectManifest {
        model_name: "BoldMdModel".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Main".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("Summary")],
        }],
    };
    let compiled = Compiler::compile(&manifest, &[tbl]).unwrap();

    // Inspect table exposes SemanticStyle::Bold on both column and row
    let inspected = compiled.inspect_table(&TableId::from("Summary")).unwrap();
    assert_eq!(inspected.columns[1].style, Some(SemanticStyle::Bold));
    assert_eq!(inspected.rows[1].style, Some(SemanticStyle::Bold));

    // Values Markdown: bolds bold/subtotal/total rows; keeps bold-column cells on data rows unbolded
    let md_values = compiled
        .to_markdown(&ModelMarkdownOptions {
            show_ids: false,
            ..Default::default()
        })
        .unwrap();
    assert!(md_values.contains("| Revenue | $100 | $120 |\n"));
    assert!(md_values.contains("| **EBITDA Margin** | **25.0%** | **28.0%** |\n"));
    assert!(md_values.contains("| **EBITDA** | **$25** | **$34** |\n"));
    assert!(md_values.contains("| **Net Income** | **$18** | **$24** |\n"));

    // Index Markdown: keeps bolding and style tags strictly for subtotal/total summaries
    let md_index = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Index,
            ..Default::default()
        })
        .unwrap();
    assert!(md_index.contains("- `rev`: Revenue\n"));
    assert!(md_index.contains("- `ebitda_margin`: EBITDA Margin\n"));
    assert!(!md_index.contains("(bold)"));
    assert!(!md_index.contains("**`ebitda_margin`**"));
    assert!(md_index.contains("- **`ebitda`** (subtotal)\n"));
    assert!(md_index.contains("- **`net_income`**: Net Income (total)\n"));
}
