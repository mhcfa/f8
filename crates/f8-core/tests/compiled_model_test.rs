#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use f8_core::compiler::emission::EmissionOptions;
use f8_core::compiler::ir::id::CellId;
use f8_core::model::format::{CurrencyCode, FormatPreset, SemanticStyle};
use f8_core::model::id::{ColId, RowId, SheetId, TableId};
use f8_core::model::manifest::{BandSpec, LaneSpec, ProjectManifest, SheetSpec};
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
                layout: Default::default(),
                bands: SheetSpec::stacked_bands([TableId::from("SimpleTbl")]),
            },
            SheetSpec {
                name: "Cash Flow".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                layout: Default::default(),
                bands: SheetSpec::stacked_bands([TableId::from("CashTbl")]),
            },
            SheetSpec {
                name: "John's Data".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                layout: Default::default(),
                bands: SheetSpec::stacked_bands([TableId::from("QuoteTbl")]),
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
            layout: Default::default(),
            bands: SheetSpec::stacked_bands([TableId::from("T1")]),
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
                show_gridlines: false,
                layout: Default::default(),
                bands: SheetSpec::stacked_bands([
                    TableId::from("IncomeStatement"),
                    TableId::from("PureMeasures"),
                ]),
            },
            SheetSpec {
                name: "Valuation".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: Default::default(),
                bands: SheetSpec::stacked_bands([TableId::from("Comps")]),
            },
            SheetSpec {
                name: "Notes".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: Default::default(),
                bands: vec![],
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

    let opts_layout: ModelMarkdownOptions =
        serde_json::from_value(serde_json::json!({ "detail": "layout" })).unwrap();
    assert_eq!(opts_layout.detail, ModelMarkdownDetail::Layout);

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

    // Layout mode: Scenario 5 cross-language byte-identical parity & determinism (ignores show_ids)
    let md_layout = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            show_ids: false,
            ..Default::default()
        })
        .unwrap();
    let expected_layout = "\
# ParityModel
Sheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3

## Runtime Errors

- `IncomeStatement[\"div_err\", \"2024E\"]` (`'Financials'!E15`): `#DIV/0!` (`=1/0`)
- `PureMeasures[\"r1\", \"c2\"]` (`'Financials'!C21`): `#N/A` (`=NA()`)
- `Comps[\"msft_row\", \"ev\"]` (`'Valuation'!D6`): `#NUM!` (`=LN(-1)`)

## Sheet: Financials
Layout: auto | Bounds: B2:E22 (21r x 4c, w:40)
Columns: A:2, B:14, C:12, D:2, E:12

- Band 1 (R2:R15, 14r x cols B..E, w:40):
  - `IncomeStatement` [B2:E15] 14r x 4c (w:40) | sub:2 hdr:R6:R7(grouped) body:R8:R15 | cols: desc B(1c), measure C(1c), gutter D(1c), measure E(1c) | wrap:1
- Band 2 (R18:R22, 5r x cols B..C, w:26, gap:2r, right_empty:D..E (2c, w:14)):
  - `PureMeasures` [B18:C22] 5r x 2c (w:26) | title->E (+2 empty) hdr:R20 body:R21:R22 | cols: measure B..C(2c)

## Sheet: Valuation
Layout: auto | Bounds: B2:D6 (5r x 3c, w:36)
Columns: A:2, B..D:12

- Band 1 (R2:R6, 5r x cols B..D, w:36):
  - `Comps` [B2:D6] 5r x 3c (w:36) | hdr:R4 body:R5:R6 | cols: desc B..C(2c), measure D(1c)

## Sheet: Notes

_(no tables)_
";
    assert_eq!(md_layout, expected_layout);
    assert_eq!(
        compiled
            .to_markdown(&ModelMarkdownOptions {
                detail: ModelMarkdownDetail::Layout,
                show_ids: true,
                ..Default::default()
            })
            .unwrap(),
        expected_layout
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
            layout: Default::default(),
            bands: SheetSpec::stacked_bands([TableId::from("Summary")]),
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
            layout: Default::default(),
            bands: SheetSpec::stacked_bands([TableId::from("GutterCols")]),
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
            layout: Default::default(),
            bands: SheetSpec::stacked_bands([TableId::from("WhitespaceDesc")]),
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
            layout: Default::default(),
            bands: SheetSpec::stacked_bands([TableId::from("Summary")]),
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

#[test]
fn test_inspect_sheet_aligned_title_banners_stacked_and_side_by_side() {
    use f8_core::compiler::compiled::SheetCellKind;
    use f8_core::model::table::CellInput;

    let make_table = |id: &str, title: &str, cols: usize, rows: usize| {
        let mut t = TableSpec::new(id, title);
        t.columns.push(ColumnSpec::descriptor("item", "Item"));
        for c in 1..cols {
            t.columns
                .push(ColumnSpec::new(format!("c{c}")).with_header(format!("C{c}")));
        }
        for r in 1..=rows {
            let mut row = RowSpec::new(format!("r{r}")).with_cell("item", format!("Row {r}"));
            for c in 1..cols {
                row = row.with_cell(format!("c{c}"), CellInput::number((r * 10 + c) as f64));
            }
            t.rows.push(row);
        }
        t
    };

    // Part 1: Stacked tables (2, 5, 3 cols across bands and within a lane)
    let stacked_tables = vec![
        make_table("T1", "Top Narrow Table", 2, 1),
        make_table("T2", "Middle Wide Table", 5, 1),
        make_table("T3", "Bottom Medium Table", 3, 1),
    ];
    let stacked_manifest = ProjectManifest {
        model_name: "InspectStackedBanners".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Main".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            layout: Default::default(),
            bands: vec![
                BandSpec::single("T1"),
                BandSpec::new([LaneSpec::new(["T2", "T3"])]),
            ],
        }],
    };
    let compiled = Compiler::compile(&stacked_manifest, &stacked_tables).unwrap();
    let sheet = compiled.inspect_sheet(&SheetId::from("Main")).unwrap();

    assert_eq!(sheet.min_col, 1);
    assert_eq!(sheet.max_col, 5);

    for (id, expected_title, end_col) in [
        ("T1", "Top Narrow Table", 2_u32),
        ("T2", "Middle Wide Table", 5_u32),
        ("T3", "Bottom Medium Table", 3_u32),
    ] {
        let p = compiled.layout_map.get_table(&TableId::from(id)).unwrap();
        let title_row_idx = (p.title_row.unwrap() - sheet.min_row) as usize;
        let title_row = &sheet.rows[title_row_idx];
        assert_eq!(title_row.cells.len(), 5);
        for (idx, cell) in title_row.cells.iter().enumerate() {
            let expected_col = 1 + idx as u32;
            assert_eq!(cell.physical_col, expected_col);
            assert_eq!(cell.kind, SheetCellKind::TableTitle);
            assert_eq!(cell.table_id.as_ref().map(TableId::as_str), Some(id));
            if expected_col == 1 {
                assert_eq!(cell.value, CellValue::Text(expected_title.to_string()));
                assert_eq!(cell.formatted_value.as_deref(), Some(expected_title));
            } else {
                assert_eq!(cell.value, CellValue::Empty);
                assert_eq!(cell.formatted_value, None);
            }
        }

        // Header and body rows remain strictly within 1..=end_col
        let hdr_row_idx = (p.header_row_range.unwrap().0 - sheet.min_row) as usize;
        let data_row_idx = (p.body_row_range.0 - sheet.min_row) as usize;
        for col in 1..=5_u32 {
            let hdr_cell = &sheet.rows[hdr_row_idx].cells[(col - 1) as usize];
            let data_cell = &sheet.rows[data_row_idx].cells[(col - 1) as usize];
            if col <= end_col {
                assert_eq!(hdr_cell.kind, SheetCellKind::ColumnHeader);
                assert_eq!(hdr_cell.table_id.as_ref().map(TableId::as_str), Some(id));
                assert_eq!(data_cell.kind, SheetCellKind::Data);
                assert_eq!(data_cell.table_id.as_ref().map(TableId::as_str), Some(id));
            } else {
                assert_eq!(hdr_cell.kind, SheetCellKind::Empty);
                assert_eq!(hdr_cell.table_id, None);
                assert_eq!(data_cell.kind, SheetCellKind::Empty);
                assert_eq!(data_cell.table_id, None);
            }
        }
    }

    // Part 2: Left-lane alignment (Left1 1..=2, Left2 1..=3 beside Right 5..=7)
    let lane_tables = vec![
        make_table("Left1", "Left Card 1", 2, 2),
        make_table("Right", "Right Panel", 3, 12),
        make_table("Left2", "Left Card 2", 3, 2),
    ];
    let lane_manifest = ProjectManifest {
        model_name: "InspectLeftLane".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Dash".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            layout: Default::default(),
            bands: vec![BandSpec::new([
                LaneSpec::new(["Left1", "Left2"]),
                LaneSpec::new(["Right"]),
            ])],
        }],
    };
    let lane_compiled = Compiler::compile(&lane_manifest, &lane_tables).unwrap();
    let dash = lane_compiled.inspect_sheet(&SheetId::from("Dash")).unwrap();

    assert_eq!(dash.min_col, 1);
    assert_eq!(dash.max_col, 7);

    // Row 0 (physical row 1): Left1 TableTitle across cols 1..=3, Empty at col 4, Right TableTitle across cols 5..=7
    let row0 = &dash.rows[0];
    for col in 1..=7_u32 {
        let cell = &row0.cells[(col - 1) as usize];
        match col {
            1..=3 => {
                assert_eq!(cell.kind, SheetCellKind::TableTitle);
                assert_eq!(cell.table_id.as_ref().map(TableId::as_str), Some("Left1"));
            }
            4 => {
                assert_eq!(cell.kind, SheetCellKind::Empty);
                assert_eq!(cell.table_id, None);
            }
            5..=7 => {
                assert_eq!(cell.kind, SheetCellKind::TableTitle);
                assert_eq!(cell.table_id.as_ref().map(TableId::as_str), Some("Right"));
            }
            _ => unreachable!(),
        }
    }

    // Left2's title row: Left2 TableTitle across cols 1..=3, Empty at col 4, Right Data across cols 5..=7
    let p_l2 = lane_compiled
        .layout_map
        .get_table(&TableId::from("Left2"))
        .unwrap();
    let l2_title_row = &dash.rows[(p_l2.title_row.unwrap() - dash.min_row) as usize];
    for col in 1..=7_u32 {
        let cell = &l2_title_row.cells[(col - 1) as usize];
        match col {
            1..=3 => {
                assert_eq!(cell.kind, SheetCellKind::TableTitle);
                assert_eq!(cell.table_id.as_ref().map(TableId::as_str), Some("Left2"));
            }
            4 => {
                assert_eq!(cell.kind, SheetCellKind::Empty);
                assert_eq!(cell.table_id, None);
            }
            5..=7 => {
                assert_eq!(cell.kind, SheetCellKind::Data);
                assert_eq!(cell.table_id.as_ref().map(TableId::as_str), Some("Right"));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn test_grid_layout_scenario_10_inspect_sheet_overflow_and_physical_spans() {
    use f8_core::SheetLayoutSpec;
    use f8_core::compiler::compiled::SheetCellKind;

    let mut left = TableSpec::new("Left", "Left Panel");
    left.columns
        .push(ColumnSpec::descriptor("item", "Line Item"));
    left.columns.push(ColumnSpec::gutter());
    left.columns
        .push(ColumnSpec::new("v1").with_header("2024E"));
    left.rows.push(RowSpec::section_header("Operating Section"));
    left.rows.push(
        RowSpec::new("r1")
            .with_cell("item", "Operating Expenses (Excl DA)") // 28+1=29 -> span 3 (B..D)
            .with_cell("v1", 100.0),
    );
    left.rows.push(RowSpec::gutter());
    left.rows.push(
        RowSpec::new("r2")
            .with_cell("item", "Total")
            .with_cell("v1", 100.0),
    );

    // Side-by-side table to also verify unowned inter-table gap is strictly SheetCellKind::Empty
    let mut right = TableSpec::new("Right", "Right Panel");
    right.columns.push(ColumnSpec::new("k").with_header("KPI"));
    right.rows.push(RowSpec::new("m1").with_cell("k", 2.5));

    let manifest = ProjectManifest {
        model_name: "InspectGrid".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Dash".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: false,
            layout: SheetLayoutSpec::grid_with_col_width(12.0),
            bands: vec![BandSpec::new([
                LaneSpec::new(["Left"]),
                LaneSpec::new(["Right"]),
            ])],
        }],
    };

    let compiled = Compiler::compile(&manifest, &[left, right]).unwrap();
    let sheet = compiled.inspect_sheet(&SheetId::from("Dash")).unwrap();

    // Left: item=(1..=3), gutter=(4..=4), v1=(5..=5); inter-table gap=(6); Right: k=(7..=7)
    assert_eq!(sheet.min_col, 1);
    assert_eq!(sheet.max_col, 7);
    assert_eq!(sheet.col_widths.get(&0), Some(&2.0));
    assert_eq!(sheet.col_widths.get(&1), Some(&12.0));
    assert_eq!(sheet.col_widths.get(&2), Some(&12.0));
    assert_eq!(sheet.col_widths.get(&3), Some(&12.0));
    assert_eq!(sheet.col_widths.get(&4), Some(&2.0)); // intra-table gutter
    assert_eq!(sheet.col_widths.get(&5), Some(&12.0));
    assert_eq!(sheet.col_widths.get(&6), Some(&2.0)); // inter-table gap
    assert_eq!(sheet.col_widths.get(&7), Some(&12.0));

    let row_at = |excel_row: u32| {
        sheet
            .rows
            .iter()
            .find(|r| r.excel_row == excel_row)
            .unwrap()
    };

    // Row 4: ColumnHeader across B4..D4 (anchor text at B4, empty ColumnHeader at C4..D4), Gutter at E4, ColumnHeader at F4, Empty at G4, ColumnHeader at H4
    let hdr = row_at(4);
    assert_eq!(hdr.cells[0].kind, SheetCellKind::ColumnHeader);
    assert_eq!(hdr.cells[0].formatted_value.as_deref(), Some("Line Item"));
    assert_eq!(
        hdr.cells[0].col_id.as_ref().map(ColId::as_str),
        Some("item")
    );
    for idx in [1_usize, 2] {
        assert_eq!(hdr.cells[idx].kind, SheetCellKind::ColumnHeader);
        assert_eq!(hdr.cells[idx].value, CellValue::Empty);
        assert_eq!(hdr.cells[idx].formatted_value, None);
        assert_eq!(
            hdr.cells[idx].col_id.as_ref().map(ColId::as_str),
            Some("item")
        );
    }
    assert_eq!(hdr.cells[3].kind, SheetCellKind::Gutter);
    assert_eq!(hdr.cells[4].kind, SheetCellKind::ColumnHeader);
    assert_eq!(hdr.cells[5].kind, SheetCellKind::Empty);
    assert_eq!(hdr.cells[5].table_id, None);
    assert_eq!(hdr.cells[5].row_id, None);
    assert_eq!(hdr.cells[5].col_id, None);
    assert!(!hdr.cells[5].is_descriptor);

    // Row 5: SectionHeader spans all physical cols of Left (B5..F5), Empty at G5, Right Data at H5
    let sec = row_at(5);
    assert_eq!(sec.cells[0].kind, SheetCellKind::SectionHeader);
    assert_eq!(
        sec.cells[0].formatted_value.as_deref(),
        Some("Operating Section")
    );
    for cell in &sec.cells[1..=4] {
        assert_eq!(cell.kind, SheetCellKind::SectionHeader);
        assert_eq!(cell.value, CellValue::Empty);
    }
    assert_eq!(sec.cells[5].kind, SheetCellKind::Empty);

    // Row 6: Data row r1 -> B6 is Data (descriptor), C6 & D6 are Overflow, E6 is Gutter, F6 is Data (measure), G6 is Empty
    let r1 = row_at(6);
    assert_eq!(r1.cells[0].kind, SheetCellKind::Data);
    assert!(r1.cells[0].is_descriptor);
    assert_eq!(r1.cells[0].physical_a1, "B6");
    for (idx, expected_a1) in [(1_usize, "C6"), (2_usize, "D6")] {
        let cell = &r1.cells[idx];
        assert_eq!(cell.physical_a1, expected_a1);
        assert_eq!(cell.kind, SheetCellKind::Overflow);
        assert_eq!(serde_json::to_string(&cell.kind).unwrap(), r#""overflow""#);
        assert_eq!(cell.table_id.as_ref().map(TableId::as_str), Some("Left"));
        assert_eq!(cell.row_id.as_ref().map(RowId::as_str), Some("r1"));
        assert_eq!(cell.col_id.as_ref().map(ColId::as_str), Some("item"));
        assert!(cell.is_descriptor);
        assert_eq!(cell.value, CellValue::Empty);
        assert_eq!(cell.formatted_value, None);
        assert_eq!(cell.formula_expr, None);
        assert_eq!(cell.excel_formula, None);
    }
    assert_eq!(r1.cells[3].kind, SheetCellKind::Gutter);
    assert_eq!(r1.cells[4].kind, SheetCellKind::Data);
    assert_eq!(r1.cells[4].physical_a1, "F6");
    assert_eq!(r1.cells[5].kind, SheetCellKind::Empty);
    assert_eq!(r1.cells[5].table_id, None);
    assert!(!r1.cells[5].is_descriptor);

    // Row 7: RowSpec::Gutter spans all physical cols of Left (B7..F7)
    let gutter_row = row_at(7);
    for cell in &gutter_row.cells[0..=4] {
        assert_eq!(cell.kind, SheetCellKind::Gutter);
        assert_eq!(cell.table_id.as_ref().map(TableId::as_str), Some("Left"));
    }
    assert_eq!(gutter_row.cells[5].kind, SheetCellKind::Empty);
}

#[test]
fn test_to_markdown_layout_scenario_1_grid_dashboard() {
    use f8_core::model::manifest::SheetLayoutSpec;
    use f8_core::model::table::CellInput;

    let build_grid_table = |id: &str,
                            title: &str,
                            measure_cols: usize,
                            gutter_after: Option<usize>,
                            rows: usize,
                            subtitle: Option<&str>,
                            grouped: bool| {
        let mut t = TableSpec::new(id, title);
        if let Some(sub) = subtitle {
            t = t.with_subtitle(sub);
        }
        t.columns.push(ColumnSpec::descriptor("item", "Line Item"));
        for c in 1..=measure_cols {
            let mut col = ColumnSpec::new(format!("m{c}")).with_header(format!("M{c}"));
            if grouped && c == 1 {
                col = col.with_group("Period");
            }
            t.columns.push(col);
            if gutter_after == Some(c) {
                t.columns.push(ColumnSpec::gutter());
            }
        }
        for r in 1..=rows {
            // 18-char descriptor triggers 2-column span at W=12 (19 > 12, ceil(19/12) = 2)
            let label = if r == 1 {
                "Eighteen Char Desc".to_string()
            } else {
                format!("Row {r}")
            };
            let mut row = RowSpec::new(format!("r{r}")).with_cell("item", label);
            for c in 1..=measure_cols {
                row = row.with_cell(format!("m{c}"), CellInput::number((r + c) as f64));
            }
            t.rows.push(row);
        }
        t
    };

    let top_left = build_grid_table("TopLeft", "Top Left", 4, None, 6, None, false);
    let top_right = build_grid_table(
        "TopRight",
        "Top Right",
        3,
        None,
        23,
        Some("($ in millions)"),
        false,
    );
    let middle = build_grid_table("Middle", "Middle", 8, Some(6), 25, None, false);
    let bottom = build_grid_table("Bottom", "Bottom", 15, None, 8, None, true);

    let manifest = ProjectManifest {
        model_name: "Dashboard".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Summary".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: false,
            layout: SheetLayoutSpec::grid_with_col_width(12.0),
            bands: vec![
                BandSpec::new([LaneSpec::new(["TopLeft"]), LaneSpec::new(["TopRight"])]),
                BandSpec::single("Middle"),
                BandSpec::single("Bottom"),
            ],
        }],
    };

    let compiled = Compiler::compile(&manifest, &[top_left, top_right, middle, bottom]).unwrap();
    let md = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            ..Default::default()
        })
        .unwrap();

    // Sheet metadata & RLE column compression
    assert!(md.contains(
        "## Sheet: Summary\nLayout: grid (W=12) | Bounds: B2:R72 (71r x 17c, w:204)\nColumns: A:2, B..R:12\n\n"
    ));

    // Band 1: side-by-side unbalanced lanes with right_empty, empty_below, and widened gap H
    assert!(
        md.contains("- Band 1 (R2:R28, 27r x cols B..M, w:144, right_empty:N..R (5c, w:60)):\n")
    );
    assert!(md.contains("  - Lane B..G (6c, w:72, R2:R10 = 9r, empty_below:R11:R28 (18r)):\n"));
    assert!(md.contains(
        "    - `TopLeft` [B2:G10] 9r x 6c (w:72) | hdr:R4 body:R5:R10 | cols: desc B..C(2c, req:19), measure D..G(4c)\n"
    ));
    assert!(md.contains("  - Lane I..M (5c, w:60, gap:1c (H, w:12), R2:R28 = 27r):\n"));
    assert!(md.contains(
        "    - `TopRight` [I2:M28] 27r x 5c (w:60) | title->R (+5 empty) sub:1 hdr:R5 body:R6:R28 | cols: desc I..J(2c, req:19), measure K..M(3c)\n"
    ));

    // Band 2: single-lane band (no Lane nesting), title overhang, and widened overlapped gutter J
    assert!(md.contains(
        "- Band 2 (R31:R58, 28r x cols B..L, w:132, gap:2r, right_empty:M..R (6c, w:72)):\n"
    ));
    assert!(md.contains(
        "  - `Middle` [B31:L58] 28r x 11c (w:132) | title->R (+6 empty) hdr:R33 body:R34:R58 | cols: desc B..C(2c, req:19), measure D..I(6c), gutter J(1c, w:12), measure K..L(2c)\n"
    ));

    // Band 3: full-width single-lane band with grouped headers and no right_empty or title overhang
    assert!(md.contains("- Band 3 (R61:R72, 12r x cols B..R, w:204, gap:2r):\n"));
    assert!(md.contains(
        "  - `Bottom` [B61:R72] 12r x 17c (w:204) | hdr:R63:R64(grouped) body:R65:R72 | cols: desc B..C(2c, req:19), measure D..R(15c)\n"
    ));
    assert_eq!(md.matches("  - Lane ").count(), 2);
    assert!(!md.contains("| stack"));
    assert!(!md.contains("right_of"));
}

#[test]
fn test_to_markdown_layout_scenario_2_multi_table_lane_width_inflation_and_table_gaps() {
    use f8_core::model::manifest::SheetLayoutSpec;
    use f8_core::model::table::CellInput;

    // Sheet 1: Lane 0 has T1 + T2 (default table_gap=2), Lane 1 has T3; wide measure inflates W above base 12
    let mut t1 = TableSpec::new("T1", "Top Left");
    t1.columns = vec![
        ColumnSpec::descriptor("item", "Item"),
        ColumnSpec::new("v")
            .with_header("Val")
            .with_format(Some(FormatPreset::Currency))
            .with_decimals(Some(1)),
    ];
    t1.rows = vec![
        RowSpec::new("r1")
            .with_cell("item", "Revenue")
            .with_cell("v", 123_456_789.0), // "$123,456,789.0" -> 14 chars + 1 pad = 15
    ];

    let mut t2 = TableSpec::new("T2", "Bottom Left");
    t2.columns = vec![
        ColumnSpec::descriptor("item", "Item"),
        ColumnSpec::new("v").with_header("Val"),
    ];
    t2.rows = vec![
        RowSpec::new("r1")
            .with_cell("item", "A")
            .with_cell("v", 1.0),
        RowSpec::new("r2")
            .with_cell("item", "B")
            .with_cell("v", 2.0),
    ];

    let mut t3 = TableSpec::new("T3", "Right Tall");
    t3.columns = vec![
        ColumnSpec::descriptor("item", "Item"),
        ColumnSpec::new("v").with_header("Val"),
    ];
    for r in 1..=12 {
        t3.rows.push(
            RowSpec::new(format!("r{r}"))
                .with_cell("item", format!("Row {r}"))
                .with_cell("v", CellInput::number(r as f64)),
        );
    }

    // Sheet 2: exercises non-default table_gap on both multi-lane Lane header and single-lane multi-table Band header
    let make_simple = |id: &str, rows: usize| {
        let mut t = TableSpec::new(id, id);
        t.columns = vec![ColumnSpec::new("v").with_header("V")];
        for r in 1..=rows {
            t.rows
                .push(RowSpec::new(format!("r{r}")).with_cell("v", CellInput::number(r as f64)));
        }
        t
    };
    let a1 = make_simple("A1", 1);
    let a2 = make_simple("A2", 1);
    let b1 = make_simple("B1", 1);
    let b2 = make_simple("B2", 1);
    let c1 = make_simple("C1", 1);
    let c2 = make_simple("C2", 1);

    let manifest = ProjectManifest {
        model_name: "BalancedLanes".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![
            SheetSpec {
                name: "Inflated".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: SheetLayoutSpec::grid_with_col_width(12.0),
                bands: vec![BandSpec::new([
                    LaneSpec::new(["T1", "T2"]),
                    LaneSpec::new(["T3"]),
                ])],
            },
            SheetSpec {
                name: "CustomTableGaps".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: SheetLayoutSpec::Auto,
                bands: vec![
                    BandSpec::new([
                        LaneSpec::new(["A1", "A2"]).with_table_gap(1),
                        LaneSpec::new(["B1", "B2"]).with_table_gap(4),
                    ]),
                    BandSpec::new([LaneSpec::new(["C1", "C2"]).with_table_gap(1)]).with_gap(3),
                ],
            },
        ],
    };

    let compiled = Compiler::compile(&manifest, &[t1, t2, t3, a1, a2, b1, b2, c1, c2]).unwrap();

    let md_inflated = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            sheets: Some(vec![SheetId::from("Inflated")]),
            ..Default::default()
        })
        .unwrap();

    // Uniform width inflated to 15 with base=12 reported explicitly
    assert!(
        md_inflated.contains("Layout: grid (W=15, base=12) | Bounds: B2:F16 (15r x 5c, w:62)\n")
    );
    assert!(md_inflated.contains("Columns: A:2, B..C:15, D:2, E..F:15\n"));
    // Single Band 1 with two Lanes: Lane 1 holds T1 (R2:R5) + T2 (R8:R12) -> empty_below R13:R16 (4r); default table_gap omitted
    assert_eq!(md_inflated.matches("- Band ").count(), 1);
    assert!(
        md_inflated.contains("  - Lane B..C (2c, w:30, R2:R12 = 11r, empty_below:R13:R16 (4r)):\n")
    );
    assert!(!md_inflated.contains("table_gap"));
    assert!(md_inflated.contains("    - `T1` [B2:C5]"));
    assert!(md_inflated.contains("    - `T2` [B8:C12] 5r x 2c (w:30) | hdr:R10"));
    assert!(md_inflated.contains("  - Lane E..F (2c, w:30, gap:1c (D, w:2), R2:R16 = 15r):\n"));
    assert!(md_inflated.contains("    - `T3` [E2:F16]"));
    assert!(!md_inflated.contains("below `T1`"));

    let md_gaps = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            sheets: Some(vec![SheetId::from("CustomTableGaps")]),
            ..Default::default()
        })
        .unwrap();

    // Multi-lane band reports non-default table_gap on each owning Lane header
    assert!(
        md_gaps.contains(
            "  - Lane B (1c, w:12, R2:R10 = 9r, empty_below:R11:R13 (3r), table_gap:1r):\n"
        )
    );
    assert!(
        md_gaps.contains("  - Lane D (1c, w:12, gap:1c (C, w:2), R2:R13 = 12r, table_gap:4r):\n")
    );
    // Single-lane multi-table band reports non-default table_gap on the owning Band header (no Lane nesting)
    assert!(md_gaps.contains(
        "- Band 2 (R17:R25, 9r x cols B, w:12, gap:3r, right_empty:C..D (2c, w:14), table_gap:1r):\n"
    ));
    assert!(!md_gaps.contains("empty_above"));
    assert!(!md_gaps.contains("left_empty"));
}

#[test]
fn test_to_markdown_layout_scenario_3_auto_metadata_chrome_wrap_and_collapsible() {
    use std::io::Read;

    use f8_core::compiler::emission::EmissionOptions;
    use f8_core::model::manifest::FreezePanes;

    // Sheet 1: full auto chrome, freeze (6, 2), wrap_text, sub:2, grouped header, collapsible vs 1-row collapsible
    let mut main_tbl = TableSpec::new("SummaryTbl", "Summary Table")
        .with_subtitles(["($ in millions)", "Unaudited"])
        .with_collapsible(true);
    main_tbl.columns = vec![
        ColumnSpec::descriptor("item", "Line Item"),
        ColumnSpec::new("y1").with_group("FY2025").with_header("Q1"),
    ];
    main_tbl.rows = vec![
        RowSpec::new("r1")
            .with_cell("item", "Line One\nLine Two")
            .with_cell("y1", 10.0),
    ];

    let mut one_row_tbl = TableSpec::new("OneRow", "").with_collapsible(true);
    one_row_tbl.columns = vec![ColumnSpec::new("v")];
    one_row_tbl.rows = vec![RowSpec::new("r1").with_cell("v", 99.0)];

    // Auxiliary sheets exercising row-only, col-only, and (0, 0) freeze splits + custom lane_gap
    let make_tiny = |id: &str| {
        let mut t = TableSpec::new(id, id);
        t.columns = vec![ColumnSpec::new("v").with_header("V")];
        t.rows = vec![RowSpec::new("r1").with_cell("v", 1.0)];
        t
    };
    let fr_row = make_tiny("FrRow");
    let fr_col = make_tiny("FrCol");
    let fr_col_r = make_tiny("FrColR");
    let fr_zero = make_tiny("FrZero");

    let manifest = ProjectManifest {
        model_name: "AutoChrome".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![
            SheetSpec {
                name: "Main".to_string(),
                tab_color: Some("#1F4E79".to_string()),
                freeze_panes: Some(FreezePanes { row: 6, col: 2 }),
                show_gridlines: true,
                layout: Default::default(),
                bands: vec![
                    BandSpec::single("SummaryTbl"),
                    BandSpec::single("OneRow").with_gap(4),
                ],
            },
            SheetSpec {
                name: "FreezeRow".to_string(),
                tab_color: None,
                freeze_panes: Some(FreezePanes { row: 4, col: 0 }),
                show_gridlines: false,
                layout: Default::default(),
                bands: SheetSpec::stacked_bands([TableId::from("FrRow")]),
            },
            SheetSpec {
                name: "FreezeCol".to_string(),
                tab_color: None,
                freeze_panes: Some(FreezePanes { row: 0, col: 3 }),
                show_gridlines: false,
                layout: Default::default(),
                bands: vec![
                    BandSpec::new([LaneSpec::new(["FrCol"]), LaneSpec::new(["FrColR"])])
                        .with_lane_gap(2),
                ],
            },
            SheetSpec {
                name: "FreezeNone".to_string(),
                tab_color: None,
                freeze_panes: Some(FreezePanes { row: 0, col: 0 }),
                show_gridlines: false,
                layout: Default::default(),
                bands: SheetSpec::stacked_bands([TableId::from("FrZero")]),
            },
        ],
    };

    let compiled = Compiler::compile(
        &manifest,
        &[main_tbl, one_row_tbl, fr_row, fr_col, fr_col_r, fr_zero],
    )
    .unwrap();
    let md = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            ..Default::default()
        })
        .unwrap();

    // Auto mode with Freeze, Gridlines, Tab
    assert!(md.contains(
        "## Sheet: Main\nLayout: auto | Bounds: B2:C13 (12r x 2c, w:24) | Freeze: C7 (R1:R6, A..B) | Gridlines: on | Tab: #1F4E79\n"
    ));
    // Single-lane bands on Main omit Lane nesting
    let main_section = md.split("## Sheet: FreezeRow").next().unwrap();
    assert!(!main_section.contains("  - Lane "));
    // Multi-row table emits sub:2, grouped header, collapsible, and wrap:1 (no placement token)
    assert!(main_section.contains(
        "  - `SummaryTbl` [B2:C8] 7r x 2c (w:24) | sub:2 hdr:R6:R7(grouped) body:R8 collapsible | cols: desc B(1c), measure C(1c) | wrap:1\n"
    ));
    // Band 2 has gap:4r; 1-row untitled headerless table emits untitled hdr:none and omits collapsible
    assert!(
        main_section
            .contains("- Band 2 (R13, 1r x cols B, w:12, gap:4r, right_empty:C (1c, w:12)):\n")
    );
    assert!(main_section.contains(
        "  - `OneRow` [B13:B13] 1r x 1c (w:12) | untitled hdr:none body:R13 | cols: measure B(1c)\n"
    ));
    let one_row_line = main_section
        .lines()
        .find(|line| line.contains("`OneRow`"))
        .unwrap();
    assert!(!one_row_line.contains("collapsible"));

    // Verify Pass 6b OpenXML parity for 1-row collapsible: SummaryTbl rows (R3..R8) get outlineLevel="1", OneRow (R13) does not
    let xlsx_bytes = compiled.emit_workbook(&EmissionOptions::default()).unwrap();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(xlsx_bytes)).unwrap();
    let mut sheet1_xml = String::new();
    archive
        .by_name("xl/worksheets/sheet1.xml")
        .unwrap()
        .read_to_string(&mut sheet1_xml)
        .unwrap();
    for row_num in 3..=8 {
        let marker = format!(r#"<row r="{row_num}""#);
        let row_start = sheet1_xml.find(&marker).unwrap();
        let row_tag_end = row_start + sheet1_xml[row_start..].find('>').unwrap();
        assert!(
            sheet1_xml[row_start..row_tag_end].contains(r#"outlineLevel="1""#),
            "expected outlineLevel=\"1\" on row {row_num}, got: {}",
            &sheet1_xml[row_start..row_tag_end]
        );
    }
    let row13_start = sheet1_xml.find(r#"<row r="13""#).unwrap();
    let row13_tag_end = row13_start + sheet1_xml[row13_start..].find('>').unwrap();
    assert!(!sheet1_xml[row13_start..row13_tag_end].contains("outlineLevel"));

    // Freeze split variants and custom lane_gap=2
    assert!(md.contains("| Freeze: A5 (R1:R4)\n"));
    assert!(md.contains("| Freeze: D1 (A..C)\n"));
    assert!(md.contains("  - Lane E (1c, w:12, gap:2c (C..D, w:4), R2:R5 = 4r):\n"));
    assert!(md.contains("| Freeze: A1 (no split)\n"));
}

#[test]
fn test_to_markdown_layout_scenario_4_filters_context_empty_and_errors() {
    let compiled = build_parity_workbook();

    // Partial tables filter on Financials: renders full sheet 2D layout, tags unselected sibling (context), scopes Runtime Errors
    let md_partial = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            sheets: None,
            tables: Some(vec![TableId::from("PureMeasures")]),
            show_ids: true,
        })
        .unwrap();
    assert!(md_partial.contains("| Errors: 3\n"));
    assert!(md_partial.contains("## Runtime Errors"));
    assert!(md_partial.contains(r#"PureMeasures["r1", "c2"]"#));
    assert!(!md_partial.contains(r#"IncomeStatement["div_err", "2024E"]"#));
    assert!(!md_partial.contains(r#"Comps["msft_row", "ev"]"#));
    assert!(md_partial.contains("## Sheet: Financials\n"));
    assert!(!md_partial.contains("## Sheet: Valuation"));
    assert!(!md_partial.contains("## Sheet: Notes"));
    // Full sheet bounds preserved (B2:E22), IncomeStatement tagged (context), PureMeasures untagged
    assert!(md_partial.contains("Layout: auto | Bounds: B2:E22 (21r x 4c, w:40)\n"));
    assert!(md_partial.contains("  - `IncomeStatement` (context) [B2:E15]"));
    assert!(md_partial.contains("  - `PureMeasures` [B18:C22]"));
    assert!(!md_partial.contains("`PureMeasures` (context)"));

    // Union including entire Financials sheet: no table tagged (context)
    let md_full_sheet = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            sheets: Some(vec![SheetId::from("Financials")]),
            tables: Some(vec![TableId::from("PureMeasures")]),
            show_ids: true,
        })
        .unwrap();
    assert!(!md_full_sheet.contains("(context)"));

    // Selected empty sheet emits _(no tables)_ and omits Runtime Errors
    let md_empty_sheet = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            sheets: Some(vec![SheetId::from("Notes")]),
            ..Default::default()
        })
        .unwrap();
    assert!(!md_empty_sheet.contains("## Runtime Errors"));
    assert!(md_empty_sheet.contains("## Sheet: Notes\n\n_(no tables)_\n"));

    // Explicit empty allowlists emit only the 2-line model header
    let expected_header_only =
        "# ParityModel\nSheets: 3 | Tables: 3 | Cells: 25 (19 drivers, 6 formulas) | Errors: 3\n";
    for (sheets, tables) in [
        (Some(vec![]), None),
        (None, Some(vec![])),
        (Some(vec![]), Some(vec![])),
    ] {
        assert_eq!(
            compiled
                .to_markdown(&ModelMarkdownOptions {
                    detail: ModelMarkdownDetail::Layout,
                    sheets,
                    tables,
                    show_ids: true,
                })
                .unwrap(),
            expected_header_only
        );
    }

    // Unknown sheet / table fail with InspectionError
    assert_eq!(
        compiled
            .to_markdown(&ModelMarkdownOptions {
                detail: ModelMarkdownDetail::Layout,
                sheets: Some(vec![SheetId::from("NoSuchSheet")]),
                ..Default::default()
            })
            .unwrap_err(),
        InspectionError::SheetNotFound(SheetId::from("NoSuchSheet"))
    );
    assert_eq!(
        compiled
            .to_markdown(&ModelMarkdownOptions {
                detail: ModelMarkdownDetail::Layout,
                tables: Some(vec![TableId::from("NoSuchTable")]),
                ..Default::default()
            })
            .unwrap_err(),
        InspectionError::TableNotFound(TableId::from("NoSuchTable"))
    );
}

#[test]
fn test_to_markdown_layout_edge_cases() {
    use f8_core::model::manifest::SheetLayoutSpec;
    use f8_core::model::table::CellInput;

    let make_rect = |id: &str, cols: usize, rows: usize| {
        let mut t = TableSpec::new(id, id);
        for c in 1..=cols {
            t.columns
                .push(ColumnSpec::new(format!("c{c}")).with_header(format!("C{c}")));
        }
        for r in 1..=rows {
            let mut row = RowSpec::new(format!("r{r}"));
            for c in 1..=cols {
                row = row.with_cell(format!("c{c}"), CellInput::number((r + c) as f64));
            }
            t.rows.push(row);
        }
        t
    };

    // Edge case 1 (Spec Scenario 5): Band/Lane reordering decoupled from declaration order
    // Input tables slice order is [T1, T2, T3]; bands arrange [Lane([T3, T1]), Lane([T2])].
    let make_err_table = |id: &str| {
        let mut t = TableSpec::new(id, id);
        t.columns.push(ColumnSpec::new("c1").with_header("C1"));
        t.rows
            .push(RowSpec::new("r1").with_cell("c1", CellInput::formula("1 / 0")));
        t
    };
    let r_t1 = make_err_table("T1");
    let r_t2 = make_err_table("T2");
    let r_t3 = make_err_table("T3");

    // Edge case 2: gap=0 Band splitting (gap:0r) and lane_gap=0 Lane splitting (gap:0c)
    let g_left = make_rect("GLeft", 1, 1);
    let g_right = make_rect("GRight", 1, 1);
    let g_below = make_rect("GBelow", 2, 1);

    // Edge case 3: Gutter-only grid sheet actual_w fallback to base_w
    let mut gutter_only = TableSpec::new("GutterOnly", "Gutter Only");
    gutter_only.columns = vec![ColumnSpec::gutter()];
    gutter_only.rows = vec![RowSpec::section_header("Spacer")];

    let manifest = ProjectManifest {
        model_name: "LayoutEdgeCases".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![
            SheetSpec {
                name: "Reordered".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: Default::default(),
                bands: vec![BandSpec::new([
                    LaneSpec::new(["T3", "T1"]),
                    LaneSpec::new(["T2"]),
                ])],
            },
            SheetSpec {
                name: "ZeroGaps".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: Default::default(),
                bands: vec![
                    BandSpec::new([LaneSpec::new(["GLeft"]), LaneSpec::new(["GRight"])])
                        .with_lane_gap(0),
                    BandSpec::single("GBelow").with_gap(0),
                ],
            },
            SheetSpec {
                name: "GutterGrid".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: SheetLayoutSpec::grid_with_col_width(14.5),
                bands: SheetSpec::stacked_bands([TableId::from("GutterOnly")]),
            },
        ],
    };

    let compiled = Compiler::compile(
        &manifest,
        &[r_t1, r_t2, r_t3, g_left, g_right, g_below, gutter_only],
    )
    .unwrap();

    // Check Spec Scenario 5: canonical band -> lane -> table order (T3, T1, T2) across all surfaces
    let expected_order = vec!["T3", "T1", "T2"];
    let grid_reordered: Vec<&str> = compiled
        .grid
        .keys()
        .filter(|id| matches!(id.table.as_str(), "T1" | "T2" | "T3"))
        .map(|id| id.table.as_str())
        .collect();
    assert_eq!(grid_reordered, expected_order);

    let summary = compiled.summary();
    let summary_reordered: Vec<&str> = summary.sheets[0]
        .tables
        .iter()
        .map(|t| t.id.as_str())
        .collect();
    assert_eq!(summary_reordered, expected_order);

    let errors = compiled.errors();
    let errors_reordered: Vec<&str> = errors.iter().map(|e| e.coordinate.table.as_str()).collect();
    assert_eq!(errors_reordered, expected_order);

    let sheet_insp_reordered = compiled.inspect_sheet(&SheetId::from("Reordered")).unwrap();
    let insp_tables: Vec<&str> = sheet_insp_reordered
        .tables
        .iter()
        .map(TableId::as_str)
        .collect();
    assert_eq!(insp_tables, expected_order);

    // Physical coordinates reflect T3 at B2, T1 below T3 at B8, T2 in Lane 1 at D2
    let p_t3 = compiled.layout_map.get_table(&TableId::from("T3")).unwrap();
    let p_t1 = compiled.layout_map.get_table(&TableId::from("T1")).unwrap();
    let p_t2 = compiled.layout_map.get_table(&TableId::from("T2")).unwrap();
    assert_eq!((p_t3.start_row, p_t3.start_col), (1, 1));
    assert_eq!((p_t1.start_row, p_t1.start_col), (7, 1));
    assert_eq!((p_t2.start_row, p_t2.start_col), (1, 3));

    for detail in [
        ModelMarkdownDetail::Values,
        ModelMarkdownDetail::Index,
        ModelMarkdownDetail::Layout,
    ] {
        let md = compiled
            .to_markdown(&ModelMarkdownOptions {
                detail,
                sheets: Some(vec![SheetId::from("Reordered")]),
                ..Default::default()
            })
            .unwrap();
        let pos_t3 = md.find("`T3`").unwrap();
        let pos_t1 = md.find("`T1`").unwrap();
        let pos_t2 = md.find("`T2`").unwrap();
        assert!(
            pos_t3 < pos_t1 && pos_t1 < pos_t2,
            "detail {detail:?} must follow canonical order T3 < T1 < T2:\n{md}"
        );
    }

    // Check gap=0 Band and Lane splitting
    let md_zero = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            sheets: Some(vec![SheetId::from("ZeroGaps")]),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(md_zero.matches("- Band ").count(), 2);
    assert!(md_zero.contains("  - Lane B (1c, w:12, R2:R5 = 4r):\n"));
    assert!(md_zero.contains("  - Lane C (1c, w:12, gap:0c, R2:R5 = 4r):\n"));
    assert!(md_zero.contains("- Band 2 (R6:R9, 4r x cols B..C, w:24, gap:0r):\n"));
    assert!(!md_zero.contains("right_of"));
    assert!(!md_zero.contains("stack"));

    // Check gutter-only grid sheet actual_w fallback to base_w=14.5
    let md_gutter = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            sheets: Some(vec![SheetId::from("GutterGrid")]),
            ..Default::default()
        })
        .unwrap();
    assert!(md_gutter.contains("Layout: grid (W=14.5) | Bounds: B2:B4 (3r x 1c, w:2)\n"));
    assert!(!md_gutter.contains("base="));
    assert!(md_gutter.contains("Columns: A..B:2\n"));
    assert!(md_gutter.contains("| hdr:none body:R4 | cols: gutter B(1c)\n"));
}

#[test]
fn test_to_markdown_layout_spec_scenario_9_golden_parity() {
    use f8_core::model::manifest::SheetLayoutSpec;
    use f8_core::model::table::CellInput;

    let make_tbl = |id: &str, title: &str, desc_val: &str, measure_cols: usize, rows: usize| {
        let mut t = TableSpec::new(id, title);
        t.columns.push(ColumnSpec::descriptor("item", "Item"));
        for c in 1..=measure_cols {
            t.columns
                .push(ColumnSpec::new(format!("m{c}")).with_header(format!("M{c}")));
        }
        for r in 1..=rows {
            let mut row = RowSpec::new(format!("r{r}")).with_cell("item", desc_val);
            for c in 1..=measure_cols {
                row = row.with_cell(format!("m{c}"), CellInput::number((r * 10 + c) as f64));
            }
            t.rows.push(row);
        }
        t
    };

    let tl = make_tbl("TL", "Top Left", "Short", 1, 1);
    let bl = make_tbl("BL", "Bottom Left", "Short", 2, 2);
    let tr = make_tbl("TR", "Top Right", "Short", 1, 1);
    let s1 = make_tbl("S1", "Stack One", "Short", 1, 1);
    let s2 = make_tbl("S2", "Stack Two", "Short", 1, 1);
    let footer = make_tbl("Footer", "Footer", "Short", 3, 1);

    let gl = make_tbl("GL", "Grid Left", "Eighteen Char Desc", 1, 1);
    let gr = make_tbl("GR", "Grid Right", "Short", 1, 1);
    let gb = make_tbl("GB", "Grid Bottom", "Short", 2, 1);

    let manifest = ProjectManifest {
        model_name: "BandLaneParity".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![
            SheetSpec {
                name: "DashAuto".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: SheetLayoutSpec::Auto,
                bands: vec![
                    BandSpec::new([
                        LaneSpec::new(["TL", "BL"]).with_table_gap(1),
                        LaneSpec::new(["TR"]),
                    ])
                    .with_lane_gap(0),
                    BandSpec::new([LaneSpec::new(["S1", "S2"]).with_table_gap(1)]).with_gap(0),
                    BandSpec::single("Footer"),
                ],
            },
            SheetSpec {
                name: "DashGrid".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: SheetLayoutSpec::grid_with_col_width(12.0),
                bands: vec![
                    BandSpec::new([LaneSpec::new(["GL"]), LaneSpec::new(["GR"])]),
                    BandSpec::single("GB"),
                ],
            },
            SheetSpec {
                name: "EmptyNotes".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: SheetLayoutSpec::Auto,
                bands: vec![],
            },
        ],
    };

    let compiled = Compiler::compile(&manifest, &[tl, bl, tr, s1, s2, footer, gl, gr, gb]).unwrap();

    let full_md = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            ..Default::default()
        })
        .unwrap();

    let expected_full = "\
# BandLaneParity
Sheets: 3 | Tables: 9 | Cells: 25 (25 drivers, 0 formulas) | Errors: 0

## Sheet: DashAuto
Layout: auto | Bounds: B2:F26 (25r x 5c, w:60)
Columns: A:2, B..F:12

- Band 1 (R2:R11, 10r x cols B..F, w:60):
  - Lane B..D (3c, w:36, R2:R11 = 10r, table_gap:1r):
    - `TL` [B2:C5] 4r x 2c (w:24) | title->D (+1 empty) hdr:R4 body:R5 | cols: desc B(1c), measure C(1c)
    - `BL` [B7:D11] 5r x 3c (w:36) | hdr:R9 body:R10:R11 | cols: desc B(1c), measure C..D(2c)
  - Lane E..F (2c, w:24, gap:0c, R2:R5 = 4r, empty_below:R6:R11 (6r)):
    - `TR` [E2:F5] 4r x 2c (w:24) | hdr:R4 body:R5 | cols: desc E(1c), measure F(1c)
- Band 2 (R12:R20, 9r x cols B..C, w:24, gap:0r, right_empty:D..F (3c, w:36), table_gap:1r):
  - `S1` [B12:C15] 4r x 2c (w:24) | title->F (+3 empty) hdr:R14 body:R15 | cols: desc B(1c), measure C(1c)
  - `S2` [B17:C20] 4r x 2c (w:24) | title->F (+3 empty) hdr:R19 body:R20 | cols: desc B(1c), measure C(1c)
- Band 3 (R23:R26, 4r x cols B..E, w:48, gap:2r, right_empty:F (1c, w:12)):
  - `Footer` [B23:E26] 4r x 4c (w:48) | title->F (+1 empty) hdr:R25 body:R26 | cols: desc B(1c), measure C..E(3c)

## Sheet: DashGrid
Layout: grid (W=12) | Bounds: B2:G11 (10r x 6c, w:72)
Columns: A:2, B..G:12

- Band 1 (R2:R5, 4r x cols B..G, w:72):
  - Lane B..D (3c, w:36, R2:R5 = 4r):
    - `GL` [B2:D5] 4r x 3c (w:36) | hdr:R4 body:R5 | cols: desc B..C(2c, req:19), measure D(1c)
  - Lane F..G (2c, w:24, gap:1c (E, w:12), R2:R5 = 4r):
    - `GR` [F2:G5] 4r x 2c (w:24) | hdr:R4 body:R5 | cols: desc F(1c), measure G(1c)
- Band 2 (R8:R11, 4r x cols B..E, w:48, gap:2r, right_empty:F..G (2c, w:24)):
  - `GB` [B8:E11] 4r x 4c (w:48) | title->G (+2 empty) hdr:R10 body:R11 | cols: desc B..C(2c, req:6), measure D..E(2c)

## Sheet: EmptyNotes

_(no tables)_
";
    assert_eq!(full_md, expected_full);
    assert!(!full_md.contains("| stack"));
    assert!(!full_md.contains("right_of"));
    assert!(!full_md.contains("below `"));

    let partial_md = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            tables: Some(vec![TableId::from("BL"), TableId::from("GR")]),
            ..Default::default()
        })
        .unwrap();
    let expected_partial = "\
# BandLaneParity
Sheets: 3 | Tables: 9 | Cells: 25 (25 drivers, 0 formulas) | Errors: 0

## Sheet: DashAuto
Layout: auto | Bounds: B2:F26 (25r x 5c, w:60)
Columns: A:2, B..F:12

- Band 1 (R2:R11, 10r x cols B..F, w:60):
  - Lane B..D (3c, w:36, R2:R11 = 10r, table_gap:1r):
    - `TL` (context) [B2:C5] 4r x 2c (w:24) | title->D (+1 empty) hdr:R4 body:R5 | cols: desc B(1c), measure C(1c)
    - `BL` [B7:D11] 5r x 3c (w:36) | hdr:R9 body:R10:R11 | cols: desc B(1c), measure C..D(2c)
  - Lane E..F (2c, w:24, gap:0c, R2:R5 = 4r, empty_below:R6:R11 (6r)):
    - `TR` (context) [E2:F5] 4r x 2c (w:24) | hdr:R4 body:R5 | cols: desc E(1c), measure F(1c)
- Band 2 (R12:R20, 9r x cols B..C, w:24, gap:0r, right_empty:D..F (3c, w:36), table_gap:1r):
  - `S1` (context) [B12:C15] 4r x 2c (w:24) | title->F (+3 empty) hdr:R14 body:R15 | cols: desc B(1c), measure C(1c)
  - `S2` (context) [B17:C20] 4r x 2c (w:24) | title->F (+3 empty) hdr:R19 body:R20 | cols: desc B(1c), measure C(1c)
- Band 3 (R23:R26, 4r x cols B..E, w:48, gap:2r, right_empty:F (1c, w:12)):
  - `Footer` (context) [B23:E26] 4r x 4c (w:48) | title->F (+1 empty) hdr:R25 body:R26 | cols: desc B(1c), measure C..E(3c)

## Sheet: DashGrid
Layout: grid (W=12) | Bounds: B2:G11 (10r x 6c, w:72)
Columns: A:2, B..G:12

- Band 1 (R2:R5, 4r x cols B..G, w:72):
  - Lane B..D (3c, w:36, R2:R5 = 4r):
    - `GL` (context) [B2:D5] 4r x 3c (w:36) | hdr:R4 body:R5 | cols: desc B..C(2c, req:19), measure D(1c)
  - Lane F..G (2c, w:24, gap:1c (E, w:12), R2:R5 = 4r):
    - `GR` [F2:G5] 4r x 2c (w:24) | hdr:R4 body:R5 | cols: desc F(1c), measure G(1c)
- Band 2 (R8:R11, 4r x cols B..E, w:48, gap:2r, right_empty:F..G (2c, w:24)):
  - `GB` (context) [B8:E11] 4r x 4c (w:48) | title->G (+2 empty) hdr:R10 body:R11 | cols: desc B..C(2c, req:6), measure D..E(2c)
";
    assert_eq!(partial_md, expected_partial);
}

#[test]
fn test_to_markdown_layout_subtitle_overflow_and_descriptor_req() {
    use f8_core::model::manifest::SheetLayoutSpec;
    use f8_core::model::table::{CellInput, DetailedCellInput, ValueLiteral};
    use f8_core::model::theme::Theme;

    // Sheet 1 (auto): subtitle boundary fit vs overflow (single & multi-line); auto descriptors omit req:
    let make_auto_sub = |id: &str, subs: &[&str], desc_label: &str| {
        let mut t = TableSpec::new(id, id).with_subtitles(subs.iter().copied());
        t.columns = vec![
            ColumnSpec::descriptor("item", "Item"),
            ColumnSpec::new("v").with_header("Val"),
        ];
        t.rows = vec![
            RowSpec::new("r1")
                .with_cell("item", desc_label)
                .with_cell("v", 1.0),
        ];
        t
    };
    // B=12, C=12 -> table w=24
    let auto_exact = make_auto_sub("AutoExact", &["x".repeat(24).as_str()], "Short");
    let auto_over = make_auto_sub("AutoOver", &["x".repeat(25).as_str()], "Short");
    let auto_multi_fit = make_auto_sub(
        "AutoMultiFit",
        &["($ in M)", "x".repeat(24).as_str()],
        "Short",
    );
    let auto_multi_over = make_auto_sub(
        "AutoMultiOver",
        &["($ in M)", "x".repeat(30).as_str()],
        "Short",
    );

    // Sheet 2 (auto wide descriptor): verifies auto descriptors never emit req:
    let mut auto_wide_desc = TableSpec::new("AutoWideDesc", "Auto Wide");
    auto_wide_desc.columns = vec![
        ColumnSpec::descriptor("item", "Line Item"),
        ColumnSpec::new("v").with_header("Val"),
    ];
    auto_wide_desc.rows = vec![
        RowSpec::new("r1")
            .with_cell("item", "A".repeat(30))
            .with_cell("v", 1.0),
    ];

    // Sheet 3 (grid W=12): cliff spills, indent demand, header demand, lane-coupling victim/culprit,
    // empty-descriptor victim, adjacent 1c merge vs spanned split, and side-by-side subtitle overflow.
    let mut culprit = TableSpec::new("Culprit", "Culprit");
    culprit.columns = vec![
        ColumnSpec::descriptor("item", "Item"),
        ColumnSpec::new("m1").with_header("M1"),
        ColumnSpec::descriptor("notes", "Notes"),
    ];
    culprit.rows = vec![
        RowSpec::new("r1")
            .with_cell("item", "A".repeat(36)) // 36+1=37 -> 4c (B..E)
            .with_cell("m1", 10.0)
            .with_cell("notes", "N".repeat(24)), // 24+1=25 -> 3c (G..I), 1 char past 2c cliff
    ];

    let mut victim = TableSpec::new("Victim", "Victim");
    victim.columns = vec![
        ColumnSpec::descriptor("item", "Item"),
        ColumnSpec::new("m1").with_header("M1"),
        ColumnSpec::descriptor("notes", "Notes"),
    ];
    victim.rows = vec![
        RowSpec::new("r1")
            .with_cell("item", "ShortItem") // 9+1=10 -> raw 1c, stretched to 4c by Culprit
            .with_cell("m1", 20.0)
            .with_cell("notes", "N".repeat(23)), // 23+1=24 -> exact 2c cliff (G..H)
    ];

    let mut empty_victim = TableSpec::new("EmptyVictim", "Empty Victim");
    empty_victim.columns = vec![
        ColumnSpec::descriptor("item", ""),
        ColumnSpec::new("m1").with_header("M1"),
    ];
    empty_victim.rows = vec![
        RowSpec::new("r1")
            .with_cell("item", CellInput::Literal(ValueLiteral::Null)) // req:0, stretched to 4c
            .with_cell("m1", 30.0),
    ];

    let mut indent_and_hdr = TableSpec::new("IndentAndHdr", "Indent And Header");
    indent_and_hdr.columns = vec![
        ColumnSpec::descriptor("ticker", "Tkr"),
        ColumnSpec::descriptor("item", "Item"),
        ColumnSpec::new("m1").with_header("M1"),
        ColumnSpec::descriptor("hdr_col", "H".repeat(24)), // header 24+1=25 -> 3c
    ];
    indent_and_hdr.rows = vec![
        RowSpec::new("r1")
            .with_cell("ticker", "AAPL") // 1c
            .with_cell(
                "item",
                DetailedCellInput {
                    value: Some(ValueLiteral::String("I".repeat(20))), // 20+1+2*2=25 -> 3c
                    indent: Some(2),
                    ..Default::default()
                },
            )
            .with_cell("m1", 40.0)
            .with_cell("hdr_col", "OK"),
    ];

    // Right of Victim (lane 1): two adjacent 1c descriptors merge; 40ch subtitle overflows w:36
    let mut adj_single = TableSpec::new("AdjSingle", "Adj Single").with_subtitle("S".repeat(40));
    adj_single.columns = vec![
        ColumnSpec::descriptor("d1", "D1"),
        ColumnSpec::descriptor("d2", "D2"),
        ColumnSpec::new("m1").with_header("M1"),
    ];
    adj_single.rows = vec![
        RowSpec::new("r1")
            .with_cell("d1", "AA")
            .with_cell("d2", "BB")
            .with_cell("m1", 5.0),
    ];

    let manifest = ProjectManifest {
        model_name: "SubAndReq".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![
            SheetSpec {
                name: "AutoSubs".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: SheetLayoutSpec::Auto,
                bands: SheetSpec::stacked_bands([
                    TableId::from("AutoExact"),
                    TableId::from("AutoOver"),
                    TableId::from("AutoMultiFit"),
                    TableId::from("AutoMultiOver"),
                ]),
            },
            SheetSpec {
                name: "AutoWide".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: SheetLayoutSpec::Auto,
                bands: SheetSpec::stacked_bands([TableId::from("AutoWideDesc")]),
            },
            SheetSpec {
                name: "GridReq".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: SheetLayoutSpec::grid_with_col_width(12.0),
                bands: vec![
                    BandSpec::single("Culprit"),
                    BandSpec::new([LaneSpec::new(["Victim"]), LaneSpec::new(["AdjSingle"])]),
                    BandSpec::single("EmptyVictim"),
                    BandSpec::single("IndentAndHdr"),
                ],
            },
        ],
    };

    let compiled = Compiler::compile(
        &manifest,
        &[
            auto_exact,
            auto_over,
            auto_multi_fit,
            auto_multi_over,
            auto_wide_desc,
            culprit,
            victim,
            empty_victim,
            indent_and_hdr,
            adj_single,
        ],
    )
    .unwrap();

    let md = compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            ..Default::default()
        })
        .unwrap();

    let line_for = |id: &str| {
        md.lines()
            .find(|line| line.contains(&format!("`{id}`")))
            .unwrap_or_else(|| panic!("missing line for `{id}` in:\n{md}"))
    };

    // Auto subtitle exact boundary vs overflow
    assert!(line_for("AutoExact").contains("| sub:1 hdr:"));
    assert!(!line_for("AutoExact").contains("ch>w"));
    assert!(line_for("AutoOver").contains("| sub:1(25ch>w) hdr:"));
    assert!(line_for("AutoMultiFit").contains("| sub:2 hdr:"));
    assert!(!line_for("AutoMultiFit").contains("ch>w"));
    assert!(line_for("AutoMultiOver").contains("| sub:2(30ch>w) hdr:"));

    // Auto wide descriptor never emits req:
    assert!(line_for("AutoWideDesc").contains("| cols: desc B(1c), measure C(1c)"));
    assert!(!line_for("AutoWideDesc").contains("req:"));

    // Grid cliff spills, lane-coupling culprit vs victim, empty victim, indent/header, adjacent split/merge
    assert!(
        line_for("Culprit")
            .contains("| cols: desc B..E(4c, req:37), measure F(1c), desc G..I(3c, req:25)")
    );
    assert!(
        line_for("Victim")
            .contains("| cols: desc B..E(4c, req:10), measure F(1c), desc G..H(2c, req:24)")
    );
    assert!(line_for("EmptyVictim").contains("| cols: desc B..E(4c, req:0), measure F(1c)"));
    assert!(line_for("IndentAndHdr").contains(
        "| cols: desc B(1c), desc C..E(3c, req:25), measure F(1c), desc G..I(3c, req:25)"
    ));
    assert!(line_for("AdjSingle").contains("sub:1(40ch>w) hdr:"));
    assert!(line_for("AdjSingle").contains("| cols: desc J..K(2c), measure L(1c)"));
    assert!(!line_for("AdjSingle").contains("req:"));

    // Custom theme: char_width_multiplier=1.2, col_padding=0.5 -> fractional req:, formula-evaluated text,
    // adjacent spanned descriptors stay split, and subtitle overflow respects char_width_multiplier
    let mut custom_theme = Theme::corporate_navy();
    custom_theme.metrics.char_width_multiplier = 1.2;
    custom_theme.metrics.col_padding = 0.5;

    let mut custom_sub_fit = TableSpec::new("SubFit", "Sub Fit").with_subtitle("S".repeat(20)); // 20*1.2=24 <= 24
    custom_sub_fit.columns = vec![
        ColumnSpec::new("a").with_header("A"),
        ColumnSpec::new("b").with_header("B"),
    ];
    custom_sub_fit.rows = vec![RowSpec::new("r1").with_cell("a", 1.0).with_cell("b", 2.0)];

    let mut custom_sub_over = TableSpec::new("SubOver", "Sub Over").with_subtitle("S".repeat(21)); // 21*1.2=25.2 > 24
    custom_sub_over.columns = vec![
        ColumnSpec::new("a").with_header("A"),
        ColumnSpec::new("b").with_header("B"),
    ];
    custom_sub_over.rows = vec![RowSpec::new("r1").with_cell("a", 1.0).with_cell("b", 2.0)];

    let mut formula_desc = TableSpec::new("FormulaDesc", "Formula Desc");
    formula_desc.columns = vec![
        ColumnSpec::descriptor("d1", "D1"),
        ColumnSpec::descriptor("d2", "D2"),
        ColumnSpec::new("m1").with_header("M1"),
    ];
    formula_desc.rows = vec![
        RowSpec::new("r1")
            // 20-char evaluated formula text -> (20+1)*1.2 + 0.5 = 25.7 -> 3c (B..D)
            .with_cell(
                "d1",
                CellInput::formula(r#"IF(TRUE, "Twenty Chars Formula", "X")"#),
            )
            // 16-char literal -> (16+1)*1.2 + 0.5 = 20.9 -> 2c (E..F)
            .with_cell("d2", "Sixteen Char Txt")
            .with_cell("m1", 1.0),
    ];

    let custom_manifest = ProjectManifest {
        model_name: "CustomMetrics".to_string(),
        theme: ThemeSpec::Custom(Box::new(custom_theme)),
        sheets: vec![
            SheetSpec {
                name: "SubCheck".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: SheetLayoutSpec::grid_with_col_width(12.0),
                bands: SheetSpec::stacked_bands([
                    TableId::from("SubFit"),
                    TableId::from("SubOver"),
                ]),
            },
            SheetSpec {
                name: "FormulaGrid".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: SheetLayoutSpec::grid_with_col_width(12.0),
                bands: SheetSpec::stacked_bands([TableId::from("FormulaDesc")]),
            },
        ],
    };

    let custom_compiled = Compiler::compile(
        &custom_manifest,
        &[custom_sub_fit, custom_sub_over, formula_desc],
    )
    .unwrap();
    let custom_md = custom_compiled
        .to_markdown(&ModelMarkdownOptions {
            detail: ModelMarkdownDetail::Layout,
            ..Default::default()
        })
        .unwrap();

    let custom_line = |id: &str| {
        custom_md
            .lines()
            .find(|line| line.contains(&format!("`{id}`")))
            .unwrap_or_else(|| panic!("missing line for `{id}` in:\n{custom_md}"))
    };

    assert!(custom_line("SubFit").contains("| sub:1 hdr:"));
    assert!(!custom_line("SubFit").contains("ch>w"));
    assert!(custom_line("SubOver").contains("| sub:1(21ch>w) hdr:"));
    assert!(
        custom_line("FormulaDesc")
            .contains("| cols: desc B..D(3c, req:25.7), desc E..F(2c, req:20.9), measure G(1c)")
    );
}
