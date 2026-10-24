#![allow(clippy::unwrap_used, clippy::expect_used)]

use f8_core::compiler::graph::{DependencyGraph, GraphError};
use f8_core::compiler::ir::{CellId, SymbolTable, expand_grid, expand_workbook};
use f8_core::model::format::FormatPreset;
use f8_core::model::id::{ColId, SheetId, TableId};
use f8_core::model::manifest::{ProjectManifest, SheetSpec};
use f8_core::model::table::{CellInput, ColumnSpec, RowSpec, TableSpec};
use indexmap::IndexMap;

fn create_table(
    table_id: &str,
    cols: &[&str],
    rows: Vec<(&str, Option<&str>, Option<f64>)>,
) -> TableSpec {
    let columns = cols.iter().map(|&c| ColumnSpec::new(c)).collect();
    let row_specs = rows
        .into_iter()
        .map(|(row_id, expr_opt, lit_opt)| {
            let mut cells = IndexMap::new();
            if let Some(num) = lit_opt {
                for &col in cols {
                    cells.insert(ColId::from(col), CellInput::number(num));
                }
            }
            let mut row = RowSpec::new(row_id).with_cells(cells);
            if let Some(expr) = expr_opt {
                row = row.with_expr(expr);
            }
            row
        })
        .collect();

    TableSpec {
        id: TableId::from(table_id),
        title: table_id.into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns,
        rows: row_specs,
    }
}

#[test]
fn test_scenario_1_acyclic_dag_and_topological_sort() {
    let mut symbols = SymbolTable::new();
    let table = create_table(
        "Model",
        &["2024E"],
        vec![
            ("a", None, Some(10.0)),
            ("b", Some("Model[\"a\", @col] * 2"), None),
            ("c", Some("Model[\"b\", @col] + 5"), None),
        ],
    );

    symbols
        .register_table(SheetId::from("Sheet1"), &table)
        .unwrap();

    let grid = expand_grid(&symbols, &[table]).unwrap();
    let dep_graph = DependencyGraph::from_grid(&grid, &symbols).unwrap();

    assert_eq!(dep_graph.node_count(), 3);
    assert_eq!(dep_graph.edge_count(), 2);

    let cell_a = CellId::new("Model", "a", "2024E");
    let cell_b = CellId::new("Model", "b", "2024E");
    let cell_c = CellId::new("Model", "c", "2024E");

    let order = dep_graph.topological_order();
    let pos_a = order.iter().position(|c| c == &cell_a).unwrap();
    let pos_b = order.iter().position(|c| c == &cell_b).unwrap();
    let pos_c = order.iter().position(|c| c == &cell_c).unwrap();

    assert!(pos_a < pos_b);
    assert!(pos_b < pos_c);
}

#[test]
fn test_scenario_2_direct_two_cell_cycle_rejected() {
    let mut symbols = SymbolTable::new();
    let table = create_table(
        "Model",
        &["2024E"],
        vec![
            ("a", Some("Model[\"b\", @col] + 1"), None),
            ("b", Some("Model[\"a\", @col] * 2"), None),
        ],
    );

    symbols
        .register_table(SheetId::from("Sheet1"), &table)
        .unwrap();

    let grid = expand_grid(&symbols, &[table]).unwrap();
    let result = DependencyGraph::from_grid(&grid, &symbols);

    match result {
        Err(GraphError::CircularDependency { cycle, trace }) => {
            assert_eq!(cycle.len(), 3);
            assert_eq!(cycle.first(), cycle.last());
            assert!(trace.contains("Model[\"a\", \"2024E\"]"));
            assert!(trace.contains("Model[\"b\", \"2024E\"]"));
            assert!(trace.contains("1. "));
            assert!(trace.contains("2. "));
            assert!(trace.contains("3. "));
        }
        _ => panic!("expected CircularDependency error"),
    }
}

#[test]
fn test_scenario_3_single_cell_self_reference_cycle_rejected() {
    let mut symbols = SymbolTable::new();
    let table = create_table(
        "Model",
        &["2024E"],
        vec![("a", Some("Model[\"a\", @col] + 1"), None)],
    );

    symbols
        .register_table(SheetId::from("Sheet1"), &table)
        .unwrap();

    let grid = expand_grid(&symbols, &[table]).unwrap();
    let result = DependencyGraph::from_grid(&grid, &symbols);

    match result {
        Err(GraphError::CircularDependency { cycle, trace }) => {
            assert_eq!(cycle.len(), 2);
            assert_eq!(cycle[0], cycle[1]);
            assert_eq!(cycle[0], CellId::new("Model", "a", "2024E"));
            assert!(trace.contains("1. Model[\"a\", \"2024E\"]"));
            assert!(trace.contains("2. Model[\"a\", \"2024E\"]"));
        }
        _ => panic!("expected CircularDependency error"),
    }
}

#[test]
fn test_scenario_4_multi_step_indirect_cycle_with_trace() {
    let mut symbols = SymbolTable::new();
    let sheet = SheetId::from("Sheet1");

    let t1 = create_table("T1", &["C"], vec![("r", Some("T2[\"r\", \"C\"]"), None)]);
    let t2 = create_table("T2", &["C"], vec![("r", Some("T3[\"r\", \"C\"]"), None)]);
    let t3 = create_table("T3", &["C"], vec![("r", Some("T1[\"r\", \"C\"]"), None)]);

    symbols.register_table(sheet.clone(), &t1).unwrap();
    symbols.register_table(sheet.clone(), &t2).unwrap();
    symbols.register_table(sheet, &t3).unwrap();

    let grid = expand_grid(&symbols, &[t1, t2, t3]).unwrap();
    let result = DependencyGraph::from_grid(&grid, &symbols);

    match result {
        Err(GraphError::CircularDependency { cycle, trace }) => {
            assert_eq!(cycle.len(), 4);
            assert_eq!(cycle.first(), cycle.last());
            assert!(trace.contains("1. "));
            assert!(trace.contains("2. "));
            assert!(trace.contains("3. "));
            assert!(trace.contains("4. "));
            assert!(trace.contains("T1[\"r\", \"C\"]"));
            assert!(trace.contains("T2[\"r\", \"C\"]"));
            assert!(trace.contains("T3[\"r\", \"C\"]"));
        }
        _ => panic!("expected CircularDependency error"),
    }
}

#[test]
fn test_scenario_5_query_dependencies_and_dependents() {
    let mut symbols = SymbolTable::new();
    let table = create_table(
        "Model",
        &["2024E"],
        vec![
            ("a1", None, Some(1.0)),
            ("a2", None, Some(2.0)),
            ("b", Some("Model[\"a1\", @col] + Model[\"a2\", @col]"), None),
            ("c", Some("Model[\"b\", @col] * 10"), None),
        ],
    );

    symbols
        .register_table(SheetId::from("Sheet1"), &table)
        .unwrap();

    let grid = expand_grid(&symbols, &[table]).unwrap();
    let dep_graph = DependencyGraph::from_grid(&grid, &symbols).unwrap();

    let cell_a1 = CellId::new("Model", "a1", "2024E");
    let cell_a2 = CellId::new("Model", "a2", "2024E");
    let cell_b = CellId::new("Model", "b", "2024E");
    let cell_c = CellId::new("Model", "c", "2024E");

    // b depends on a1 and a2
    let b_deps = dep_graph.dependencies_of(&cell_b);
    assert_eq!(b_deps.len(), 2);
    assert!(b_deps.contains(&&cell_a1));
    assert!(b_deps.contains(&&cell_a2));

    // b has dependent c
    let b_dependents = dep_graph.dependents_of(&cell_b);
    assert_eq!(b_dependents.len(), 1);
    assert_eq!(b_dependents[0], &cell_c);

    // a1 has no dependencies and 1 dependent (b)
    assert!(dep_graph.dependencies_of(&cell_a1).is_empty());
    assert_eq!(dep_graph.dependents_of(&cell_a1), vec![&cell_b]);

    // c has 1 dependency (b) and no dependents
    assert_eq!(dep_graph.dependencies_of(&cell_c), vec![&cell_b]);
    assert!(dep_graph.dependents_of(&cell_c).is_empty());
}

#[test]
fn test_scenario_6_disconnected_literal_cells_preserved() {
    let mut symbols = SymbolTable::new();
    let table = create_table(
        "Model",
        &["2024E"],
        vec![
            ("lit1", None, Some(1.0)),
            ("lit2", None, Some(2.0)),
            ("lit3", None, Some(3.0)),
            ("calc1", Some("Model[\"lit1\", @col] * 2"), None),
            ("calc2", Some("Model[\"calc1\", @col] + 1"), None),
        ],
    );

    symbols
        .register_table(SheetId::from("Sheet1"), &table)
        .unwrap();

    let grid = expand_grid(&symbols, &[table]).unwrap();
    let dep_graph = DependencyGraph::from_grid(&grid, &symbols).unwrap();

    assert_eq!(dep_graph.node_count(), 5);
    assert_eq!(dep_graph.topological_order().len(), 5);

    for row in ["lit1", "lit2", "lit3", "calc1", "calc2"] {
        let cell = CellId::new("Model", row, "2024E");
        assert!(dep_graph.topological_order().contains(&cell));
    }
}

#[test]
fn test_scenario_7_full_pipeline_spec_ast_to_dependency_graph() {
    let manifest = ProjectManifest {
        model_name: "EndToEndPipelineModel".to_string(),
        theme: Default::default(),
        sheets: vec![
            SheetSpec {
                name: "Summary".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("Assumptions")],
            },
            SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("Forecast")],
            },
        ],
    };

    let mut assumptions_cells = IndexMap::new();
    assumptions_cells.insert(ColId::from("value"), CellInput::number(0.10));
    let assumptions_row_tax = RowSpec::new("tax_rate")
        .with_format(Some(FormatPreset::Percentage))
        .with_cells(assumptions_cells);

    let mut rev_base_cells = IndexMap::new();
    rev_base_cells.insert(ColId::from("value"), CellInput::number(1000.0));
    let assumptions_row_base = RowSpec::new("base_revenue")
        .with_format(Some(FormatPreset::Currency))
        .with_cells(rev_base_cells);

    let assumptions_table = TableSpec {
        id: TableId::from("Assumptions"),
        title: "Key Assumptions".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("value")],
        rows: vec![assumptions_row_tax, assumptions_row_base],
    };

    let mut rev_cells = IndexMap::new();
    rev_cells.insert(
        ColId::from("2023A"),
        CellInput::formula("Assumptions[\"base_revenue\", \"value\"]"),
    );
    rev_cells.insert(
        ColId::from("2024E"),
        CellInput::formula("Forecast[\"revenue\", col.prev] * 1.1"),
    );

    let forecast_row_rev = RowSpec::new("revenue")
        .with_format(Some(FormatPreset::Currency))
        .with_cells(rev_cells);

    let forecast_row_tax = RowSpec::new("tax")
        .with_format(Some(FormatPreset::Currency))
        .with_expr("Forecast[\"revenue\", @col] * Assumptions[\"tax_rate\", \"value\"]");

    let forecast_table = TableSpec {
        id: TableId::from("Forecast"),
        title: "Three Year Forecast".into(),
        subtitle: None,
        currency: None,
        collapsible: false,
        placement: f8_core::model::PlacementSpec::default(),
        columns: vec![ColumnSpec::new("2023A"), ColumnSpec::new("2024E")],
        rows: vec![forecast_row_rev, forecast_row_tax],
    };

    // Full Pipeline: Spec AST -> expand_workbook -> DependencyGraph::from_grid
    let (symbols, grid) = expand_workbook(&manifest, &[assumptions_table, forecast_table])
        .expect("workbook expands successfully");

    let dep_graph = DependencyGraph::from_grid(&grid, &symbols)
        .expect("dependency graph constructs successfully without circularity");

    assert_eq!(dep_graph.node_count(), 6);
    // 2 in Assumptions + 4 in Forecast (revenue: 2, tax: 2)

    let tax_rate = CellId::new("Assumptions", "tax_rate", "value");
    let base_rev = CellId::new("Assumptions", "base_revenue", "value");
    let rev_2023 = CellId::new("Forecast", "revenue", "2023A");
    let rev_2024 = CellId::new("Forecast", "revenue", "2024E");
    let tax_2023 = CellId::new("Forecast", "tax", "2023A");
    let tax_2024 = CellId::new("Forecast", "tax", "2024E");

    // Check topological evaluation ordering
    let order = dep_graph.topological_order();
    let pos_base_rev = order.iter().position(|c| c == &base_rev).unwrap();
    let pos_rev_2023 = order.iter().position(|c| c == &rev_2023).unwrap();
    let pos_rev_2024 = order.iter().position(|c| c == &rev_2024).unwrap();
    let pos_tax_2023 = order.iter().position(|c| c == &tax_2023).unwrap();
    let pos_tax_2024 = order.iter().position(|c| c == &tax_2024).unwrap();
    let pos_tax_rate = order.iter().position(|c| c == &tax_rate).unwrap();

    assert!(pos_base_rev < pos_rev_2023);
    assert!(pos_rev_2023 < pos_rev_2024);
    assert!(pos_rev_2023 < pos_tax_2023);
    assert!(pos_tax_rate < pos_tax_2023);
    assert!(pos_rev_2024 < pos_tax_2024);
    assert!(pos_tax_rate < pos_tax_2024);

    // Check dependencies of 2024E revenue
    let rev_2024_deps = dep_graph.dependencies_of(&rev_2024);
    assert_eq!(rev_2024_deps, vec![&rev_2023]);

    // Check dependents of Assumptions tax_rate
    let tax_rate_dependents = dep_graph.dependents_of(&tax_rate);
    assert_eq!(tax_rate_dependents.len(), 2);
    assert!(tax_rate_dependents.contains(&&tax_2023));
    assert!(tax_rate_dependents.contains(&&tax_2024));
}

#[test]
fn test_scenario_8_omitted_argument_contributes_zero_dependencies() {
    let table = create_table(
        "Forecast",
        &["2024E"],
        vec![
            ("rate", None, Some(0.05)),
            (
                "pmt",
                Some("PMT(Forecast[\"rate\", @col], 10, 1000, _, 1)"),
                None,
            ),
        ],
    );

    let mut symbols = SymbolTable::new();
    symbols
        .register_table(SheetId::from("Sheet1"), &table)
        .unwrap();
    let grid = expand_grid(&symbols, &[table]).unwrap();
    let dep_graph = DependencyGraph::from_grid(&grid, &symbols).unwrap();

    let pmt = CellId::new("Forecast", "pmt", "2024E");
    let rate = CellId::new("Forecast", "rate", "2024E");

    let pmt_deps = dep_graph.dependencies_of(&pmt);
    // Rate is the only dependency; _ contributes zero references
    assert_eq!(pmt_deps, vec![&rate]);
}
