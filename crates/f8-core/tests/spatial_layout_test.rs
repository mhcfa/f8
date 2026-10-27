#![allow(clippy::unwrap_used, clippy::expect_used)]

use f8_core::compiler::layout::{
    LayoutError, LayoutPlanner, MAX_EXCEL_COLUMNS, MAX_EXCEL_ROWS, col_to_excel_col,
    coordinate_to_a1,
};
use f8_core::compiler::{Compiler, ir};
use f8_core::model::format::SemanticStyle;
use f8_core::model::id::{ColId, SheetId, TableId};
use f8_core::model::manifest::{FreezePanes, ProjectManifest, SheetSpec};
use f8_core::model::table::{
    CellInput, ColumnSpec, DetailedCellInput, PlacementSpec, RowSpec, SectionHeaderRowSpec,
    TableSpec, ValueLiteral,
};
use f8_core::model::theme::Theme;

fn create_test_theme() -> Theme {
    Theme::corporate_navy()
}

fn populate_table_cells(table: &mut TableSpec) {
    for row in &mut table.rows {
        if let RowSpec::Data(d) = row {
            for col in &table.columns {
                if let ColumnSpec::Data(c) = col
                    && !d.cells.contains_key(&c.id)
                {
                    if c.role.is_descriptor() {
                        d.cells
                            .insert(c.id.clone(), CellInput::string(d.id.as_str()));
                    } else {
                        d.cells.insert(c.id.clone(), CellInput::number(100.0));
                    }
                }
            }
        }
    }
}

#[test]
fn test_scenario_1_default_sheet_margins_and_primary_table_origin_at_b2() {
    let mut table = TableSpec::new("SimpleTable", "");
    table
        .columns
        .push(ColumnSpec::new("col_1").with_header("Col 1"));
    table.rows.push(RowSpec::new("row_1"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "TestModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("SimpleTable")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let sheet_layout = layout.get_sheet(&SheetId::from("Sheet1")).unwrap();
    // Row 0 is top gutter
    assert_eq!(
        sheet_layout.row_heights.get(&0),
        Some(&theme.metrics.gutter_row_height)
    );
    // Col 0 is left gutter
    assert_eq!(
        sheet_layout.col_widths.get(&0),
        Some(&theme.metrics.gutter_col_width)
    );

    let cell_id = f8_core::compiler::ir::id::CellId::new("SimpleTable", "row_1", "col_1");
    let cell_coord = layout.get_cell(&cell_id).unwrap();

    // Table has no title banner, but has header_row at row 1, data row at row 2
    assert_eq!(cell_coord.col, 1);
    assert_eq!(cell_coord.row, 2);
    assert_eq!(cell_coord.a1, "B3");
}

#[test]
fn test_scenario_2_table_header_stack_with_title_subtitle_buffer_and_column_headers() {
    let mut table = TableSpec::new("IncomeStatement", "Income Statement");
    table.subtitle = Some("($ in thousands)".to_string());
    table
        .columns
        .push(ColumnSpec::descriptor("line_item", "Line Item"));
    table
        .columns
        .push(ColumnSpec::new("2023A").with_header("2023A"));
    table
        .columns
        .push(ColumnSpec::new("2024E").with_header("2024E"));

    table.rows.push(RowSpec::new("revenue"));
    table.rows.push(RowSpec::new("cogs"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "TestModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Model".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("IncomeStatement")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let placement = layout.get_table(&TableId::from("IncomeStatement")).unwrap();

    // Row 1: Title
    assert_eq!(placement.title_row, Some(1));
    // Row 2: Subtitle
    assert_eq!(placement.subtitle_row, Some(2));
    // Row 3: Title gutter
    assert_eq!(placement.title_gutter_row, Some(3));
    // Row 4: Column header
    assert_eq!(placement.header_row_range, Some((4, 4)));
    // Data rows: Rows 5 and 6
    assert_eq!(placement.body_row_range, (5, 6));

    let sheet = layout.get_sheet(&SheetId::from("Model")).unwrap();
    assert_eq!(
        sheet.row_heights.get(&1),
        Some(&theme.metrics.title_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&2),
        Some(&theme.metrics.data_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&3),
        Some(&theme.metrics.title_gutter_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&4),
        Some(&theme.metrics.header_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&5),
        Some(&theme.metrics.data_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&6),
        Some(&theme.metrics.data_row_height)
    );

    // Verify cell coordinates
    let rev_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "IncomeStatement",
            "revenue",
            "line_item",
        ))
        .unwrap();
    assert_eq!(rev_cell.col, 1);
    assert_eq!(rev_cell.row, 5);
    assert_eq!(rev_cell.a1, "B6");

    let rev_2023a = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "IncomeStatement",
            "revenue",
            "2023A",
        ))
        .unwrap();
    assert_eq!(rev_2023a.col, 2);
    assert_eq!(rev_2023a.row, 5);
    assert_eq!(rev_2023a.a1, "C6");
}

#[test]
fn test_scenario_3_super_headers_spanning_grouped_column_ranges() {
    let mut table = TableSpec::new("TimelineTable", "Timeline");
    table
        .columns
        .push(ColumnSpec::descriptor("line_item", "Line Item"));
    table.columns.push(
        ColumnSpec::new("2021A")
            .with_group("Historical Periods")
            .with_header("2021A"),
    );
    table.columns.push(
        ColumnSpec::new("2022A")
            .with_group("Historical Periods")
            .with_header("2022A"),
    );
    table.columns.push(
        ColumnSpec::new("2023A")
            .with_group("Historical Periods")
            .with_header("2023A"),
    );
    table.columns.push(
        ColumnSpec::new("2024E")
            .with_group("Forecast Periods")
            .with_header("2024E"),
    );
    table.columns.push(
        ColumnSpec::new("2025E")
            .with_group("Forecast Periods")
            .with_header("2025E"),
    );

    table.rows.push(RowSpec::new("metric"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "TestModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Summary".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("TimelineTable")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let placement = layout.get_table(&TableId::from("TimelineTable")).unwrap();

    // Row 1: Title
    assert_eq!(placement.title_row, Some(1));
    // Row 2: Title Gutter
    assert_eq!(placement.title_gutter_row, Some(2));
    // Rows 3..4: Multi-tier headers (Tier 0 super-header, Tier 1 metric header)
    assert_eq!(placement.header_row_range, Some((3, 4)));
    // Row 5: Data Row
    assert_eq!(placement.body_row_range, (5, 5));
}

#[test]
fn test_scenario_4_interleaved_section_headers_and_outline_levels() {
    let mut table = TableSpec::new("HierarchyTable", "Hierarchy");
    table.columns.push(ColumnSpec::descriptor("item", "Item"));
    table
        .columns
        .push(ColumnSpec::new("val").with_header("Value"));

    table
        .rows
        .push(RowSpec::section_header("Operating Expenses"));
    table.rows.push(RowSpec::new("r_and_d"));
    table.rows.push(
        SectionHeaderRowSpec::new("Sales & Marketing")
            .with_indent(Some(1))
            .into(),
    );
    table.rows.push(RowSpec::new("ad_spend"));
    table.rows.push(RowSpec::new("direct_sales"));
    table.rows.push(RowSpec::new("g_and_a"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "TestModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Model".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("HierarchyTable")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let sheet = layout.get_sheet(&SheetId::from("Model")).unwrap();

    // Verify row sequencing:
    // Row 1: Title
    // Row 2: Title Gutter
    // Row 3: Col Header
    // Row 4: Section Header "Operating Expenses"
    // Row 5: Data Row "r_and_d"
    // Row 6: Section Header "Sales & Marketing"
    // Row 7: Data Row "ad_spend"
    // Row 8: Data Row "direct_sales"
    // Row 9: Data Row "g_and_a"
    assert_eq!(
        sheet.row_heights.get(&4),
        Some(&theme.metrics.section_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&6),
        Some(&theme.metrics.section_row_height)
    );

    let rd_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "HierarchyTable",
            "r_and_d",
            "val",
        ))
        .unwrap();
    assert_eq!(rd_cell.row, 5);

    let ad_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "HierarchyTable",
            "ad_spend",
            "val",
        ))
        .unwrap();
    assert_eq!(ad_cell.row, 7);

    let ds_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "HierarchyTable",
            "direct_sales",
            "val",
        ))
        .unwrap();
    assert_eq!(ds_cell.row, 8);

    let ga_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "HierarchyTable",
            "g_and_a",
            "val",
        ))
        .unwrap();
    assert_eq!(ga_cell.row, 9);
}

#[test]
fn test_scenario_5_multi_table_vertical_stacking_and_inter_table_gutters() {
    let mut table1 = TableSpec::new("Table1", "First Table");
    table1.columns.push(ColumnSpec::new("c1").with_header("C1"));
    table1.rows.push(RowSpec::new("r1"));
    table1.rows.push(RowSpec::new("r2"));

    let mut table2 = TableSpec::new("Table2", "Second Table");
    table2.columns.push(ColumnSpec::new("c1").with_header("C1"));
    table2.rows.push(RowSpec::new("r1"));

    let manifest = ProjectManifest {
        model_name: "MultiTable".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "StackedSheet".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("Table1"), TableId::from("Table2")],
        }],
    };

    let theme = create_test_theme();
    let mut tables = vec![table1, table2];
    populate_table_cells(&mut tables[0]);
    populate_table_cells(&mut tables[1]);
    let (_sym, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let placement1 = layout.get_table(&TableId::from("Table1")).unwrap();
    let placement2 = layout.get_table(&TableId::from("Table2")).unwrap();

    // Table 1:
    // Row 1: Title
    // Row 2: Title Gutter
    // Row 3: Col Header
    // Row 4: Data r1
    // Row 5: Data r2
    assert_eq!(placement1.start_row, 1);
    assert_eq!(placement1.end_row, 5);

    // Inter-table gutters: Rows 6 and 7 (2 blank rows)
    let sheet = layout.get_sheet(&SheetId::from("StackedSheet")).unwrap();
    assert_eq!(
        sheet.row_heights.get(&6),
        Some(&theme.metrics.gutter_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&7),
        Some(&theme.metrics.gutter_row_height)
    );

    // Table 2:
    // Row 8: Title
    // Row 9: Title Gutter
    // Row 10: Col Header
    // Row 11: Data r1
    assert_eq!(placement2.start_row, 8);
    assert_eq!(placement2.end_row, 11);
}

#[test]
fn test_scenario_6_sheet_wide_column_width_arbitration_across_stacked_tables() {
    let mut table1 = TableSpec::new("Table1", "T1");
    table1.columns.push(ColumnSpec::descriptor("desc", "Desc"));
    table1
        .columns
        .push(ColumnSpec::new("amount").with_header("Amount"));
    let mut row1 = RowSpec::new("long_desc_row");
    if let RowSpec::Data(d) = &mut row1 {
        d.cells.insert(
            ColId::from("desc"),
            CellInput::string("Very Long Descriptive Account Name Exceeding Normal Width"),
        );
    }
    table1.rows.push(row1);

    let mut table2 = TableSpec::new("Table2", "T2");
    table2.columns.push(ColumnSpec::descriptor("desc", "Desc"));
    table2
        .columns
        .push(ColumnSpec::new("amount").with_header("Amount"));
    let mut row2 = RowSpec::new("short_row");
    if let RowSpec::Data(d) = &mut row2 {
        d.cells
            .insert(ColId::from("desc"), CellInput::string("Short"));
    }
    table2.rows.push(row2);

    let manifest = ProjectManifest {
        model_name: "WidthTest".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("Table1"), TableId::from("Table2")],
        }],
    };

    let theme = create_test_theme();
    let mut tables = vec![table1, table2];
    populate_table_cells(&mut tables[0]);
    populate_table_cells(&mut tables[1]);
    let (_sym, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let sheet = layout.get_sheet(&SheetId::from("Sheet1")).unwrap();
    let col_b_width = *sheet.col_widths.get(&1).unwrap();
    let col_c_width = *sheet.col_widths.get(&2).unwrap();

    // Col B (descriptor) must be wide enough for the long label (57 chars * 1.0 + 0.0 = 57.0)
    assert_eq!(col_b_width, 57.0);

    // Col C (measure) must be at least min_col_width (12.0)
    assert!(col_c_width >= theme.metrics.min_col_width);
}

#[test]
fn test_scenario_7_full_style_role_propagation_to_physical_coordinates() {
    let mut table = TableSpec::new("SummaryTable", "Summary");
    table
        .columns
        .push(ColumnSpec::new("metric").with_header("Metric"));
    table.columns.push(
        ColumnSpec::new("col_total")
            .with_header("Total")
            .with_style(Some(SemanticStyle::Total))
            .with_highlight(Some(true)),
    );

    let row = RowSpec::new("row_total").with_style(Some(SemanticStyle::Total));
    table.rows.push(row);
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "StyleTest".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("SummaryTable")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let intersection_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "SummaryTable",
            "row_total",
            "col_total",
        ))
        .unwrap();

    assert_eq!(
        intersection_cell.style_role.horizontal,
        SemanticStyle::Total
    );
    assert_eq!(intersection_cell.style_role.vertical, SemanticStyle::Total);
    assert!(intersection_cell.style_role.is_highlight);
}

#[test]
fn test_scenario_8_exact_excel_a1_coordinate_bijection_and_boundary_checks() {
    assert_eq!(col_to_excel_col(0), "A");
    assert_eq!(col_to_excel_col(1), "B");
    assert_eq!(col_to_excel_col(25), "Z");
    assert_eq!(col_to_excel_col(26), "AA");
    assert_eq!(col_to_excel_col(27), "AB");
    assert_eq!(col_to_excel_col(701), "ZZ");
    assert_eq!(col_to_excel_col(702), "AAA");

    assert_eq!(coordinate_to_a1(0, 0).unwrap(), "A1");
    assert_eq!(coordinate_to_a1(1, 1).unwrap(), "B2");
    assert_eq!(coordinate_to_a1(25, 9).unwrap(), "Z10");
    assert_eq!(coordinate_to_a1(26, 99).unwrap(), "AA100");

    assert!(matches!(
        coordinate_to_a1(MAX_EXCEL_COLUMNS, 0),
        Err(LayoutError::GridLimitExceeded {
            dimension: "columns",
            ..
        })
    ));

    assert!(matches!(
        coordinate_to_a1(0, MAX_EXCEL_ROWS),
        Err(LayoutError::GridLimitExceeded {
            dimension: "rows",
            ..
        })
    ));
}

#[test]
fn test_scenario_9_multi_tier_column_header_row_height_allocation() {
    let mut table = TableSpec::new("MultiTierTable", "Multi Tier");
    table
        .columns
        .push(ColumnSpec::new("c1").with_headers(["Tier1"]));
    table
        .columns
        .push(ColumnSpec::new("c2").with_headers(["Tier1", "Tier2"]));
    table
        .columns
        .push(ColumnSpec::new("c3").with_headers(["Top", "Mid", "Tier3"]));
    table.rows.push(RowSpec::new("r1"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "MultiTierModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("MultiTierTable")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let placement = layout.get_table(&TableId::from("MultiTierTable")).unwrap();

    // Title at row 1, gutter at row 2, 3 header rows at rows 3, 4, 5
    assert_eq!(placement.header_row_range, Some((3, 5)));
    assert_eq!(placement.body_row_range, (6, 6));

    let sheet = layout.get_sheet(&SheetId::from("Sheet1")).unwrap();
    assert_eq!(
        sheet.row_heights.get(&3),
        Some(&theme.metrics.header_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&4),
        Some(&theme.metrics.header_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&5),
        Some(&theme.metrics.header_row_height)
    );
}

#[test]
fn test_scenario_10_rejection_of_unassigned_tables() {
    let mut table1 = TableSpec::new("AssignedTable", "A");
    table1.columns.push(ColumnSpec::new("c").with_header("C"));
    table1.rows.push(RowSpec::new("r"));

    let mut table2 = TableSpec::new("UnassignedTable", "U");
    table2.columns.push(ColumnSpec::new("c").with_header("C"));
    table2.rows.push(RowSpec::new("r"));

    let manifest = ProjectManifest {
        model_name: "TestModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("AssignedTable")],
        }],
    };

    let theme = create_test_theme();
    let err = LayoutPlanner::plan(
        &manifest,
        &[table1, table2],
        &ir::cell::CellGrid::new(),
        &theme,
        None,
    )
    .unwrap_err();

    assert!(matches!(
        err,
        LayoutError::UnassignedTable { table } if table.as_str() == "UnassignedTable"
    ));
}

#[test]
fn test_scenario_11_end_to_end_compiler_facade_integration() {
    let mut table = TableSpec::new("FacadeTable", "Facade");
    table
        .columns
        .push(ColumnSpec::new("col_a").with_header("A"));
    table
        .columns
        .push(ColumnSpec::new("col_b").with_header("B"));
    table.rows.push(RowSpec::new("row_1"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "FacadeModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Summary".to_string(),
            tab_color: Some("#1B365D".to_string()),
            freeze_panes: Some(FreezePanes { row: 2, col: 2 }),
            show_gridlines: true,
            tables: vec![TableId::from("FacadeTable")],
        }],
    };

    let layout = Compiler::plan_layout(&manifest, &[table], None).unwrap();

    let sheet = layout.get_sheet(&SheetId::from("Summary")).unwrap();
    assert_eq!(sheet.tab_color.as_deref(), Some("#1B365D"));
    assert_eq!(sheet.freeze_panes, Some(FreezePanes { row: 2, col: 2 }));
    assert!(sheet.show_gridlines);

    assert_eq!(layout.tables.len(), 1);
    assert_eq!(layout.cells.len(), 2);
}

#[test]
fn test_scenario_12_descriptor_indentation_tiers() {
    let mut table = TableSpec::new("NestedHierarchyTable", "Nested");
    table.columns.push(ColumnSpec::descriptor("item", "Item"));
    table
        .columns
        .push(ColumnSpec::new("val").with_header("Value"));

    table.rows.push(RowSpec::new("ad_spend").with_cell(
        "item",
        DetailedCellInput {
            value: Some(ValueLiteral::String("ad_spend".into())),
            indent: Some(2),
            ..Default::default()
        },
    ));
    table.rows.push(RowSpec::new("direct_sales").with_cell(
        "item",
        DetailedCellInput {
            value: Some(ValueLiteral::String("direct_sales".into())),
            indent: Some(1),
            ..Default::default()
        },
    ));
    table.rows.push(RowSpec::new("g_and_a"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "TestModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Model".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("NestedHierarchyTable")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let ad_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "NestedHierarchyTable",
            "ad_spend",
            "item",
        ))
        .unwrap();
    assert_eq!(ad_cell.indent, 2);

    let ds_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "NestedHierarchyTable",
            "direct_sales",
            "item",
        ))
        .unwrap();
    assert_eq!(ds_cell.indent, 1);

    let ga_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "NestedHierarchyTable",
            "g_and_a",
            "item",
        ))
        .unwrap();
    assert_eq!(ga_cell.indent, 0);

    // Measure cells are never indented
    let ad_val = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "NestedHierarchyTable",
            "ad_spend",
            "val",
        ))
        .unwrap();
    assert_eq!(ad_val.indent, 0);
}

#[test]
fn test_scenario_13_explicit_gutters_and_section_headers() {
    let mut table = TableSpec::new("GutterTable", "Financial Statement");
    table
        .columns
        .push(ColumnSpec::descriptor("line_item", "Line Item"));
    table.columns.push(ColumnSpec::gutter());
    table
        .columns
        .push(ColumnSpec::new("val").with_header("Value"));

    table.rows.push(RowSpec::new("rev"));
    table.rows.push(RowSpec::gutter());
    table
        .rows
        .push(RowSpec::section_header("Operating Expenses"));
    table.rows.push(RowSpec::new("salaries").with_cell(
        "line_item",
        DetailedCellInput {
            value: Some(ValueLiteral::String("salaries".into())),
            indent: Some(1),
            ..Default::default()
        },
    ));
    table.rows.push(RowSpec::new("ads").with_cell(
        "line_item",
        DetailedCellInput {
            value: Some(ValueLiteral::String("ads".into())),
            indent: Some(2),
            ..Default::default()
        },
    ));
    table
        .rows
        .push(RowSpec::new("total_opex").with_style(Some(SemanticStyle::Subtotal)));
    table.rows.push(RowSpec::gutter());
    table
        .rows
        .push(RowSpec::section_header("Capital Expenditures"));
    table.rows.push(RowSpec::new("equipment").with_cell(
        "line_item",
        DetailedCellInput {
            value: Some(ValueLiteral::String("equipment".into())),
            indent: Some(1),
            ..Default::default()
        },
    ));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "GutterModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Model".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("GutterTable")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();
    let sheet = layout.get_sheet(&SheetId::from("Model")).unwrap();

    // Column 2 is ColumnSpec::Gutter -> width should be gutter_col_width (3.0 chars)
    assert_eq!(
        sheet.col_widths.get(&2),
        Some(&theme.metrics.gutter_col_width)
    );

    // Row sequence:
    // Row 1: Title
    // Row 2: Title Gutter (8pt)
    // Row 3: Col Header (Tier 0)
    // Row 4: Data row "rev"
    // Row 5: Explicit RowSpec::Gutter (9pt)
    // Row 6: Section Header "Operating Expenses" (20pt)
    // Row 7: Data row "salaries"
    // Row 8: Data row "ads"
    // Row 9: Data row "total_opex" (subtotal)
    // Row 10: Explicit RowSpec::Gutter (9pt)
    // Row 11: Section Header "Capital Expenditures" (20pt)
    // Row 12: Data row "equipment"

    // Verify gutter rows
    assert_eq!(
        sheet.row_heights.get(&5),
        Some(&theme.metrics.gutter_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&10),
        Some(&theme.metrics.gutter_row_height)
    );

    // Verify section header row heights
    assert_eq!(
        sheet.row_heights.get(&6),
        Some(&theme.metrics.section_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&11),
        Some(&theme.metrics.section_row_height)
    );

    // Verify physical rows
    let rev_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "GutterTable",
            "rev",
            "line_item",
        ))
        .unwrap();
    assert_eq!(rev_cell.row, 4);
    assert_eq!(rev_cell.indent, 0);

    let salaries_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "GutterTable",
            "salaries",
            "line_item",
        ))
        .unwrap();
    assert_eq!(salaries_cell.row, 7);
    assert_eq!(salaries_cell.indent, 1);

    let ads_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "GutterTable",
            "ads",
            "line_item",
        ))
        .unwrap();
    assert_eq!(ads_cell.row, 8);
    assert_eq!(ads_cell.indent, 2);

    let total_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "GutterTable",
            "total_opex",
            "line_item",
        ))
        .unwrap();
    assert_eq!(total_cell.row, 9);
    assert_eq!(total_cell.indent, 0);

    let equip_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "GutterTable",
            "equipment",
            "line_item",
        ))
        .unwrap();
    assert_eq!(equip_cell.row, 12);
    assert_eq!(equip_cell.indent, 1);

    // Verify measure cells are NEVER indented
    let salaries_val = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "GutterTable",
            "salaries",
            "val",
        ))
        .unwrap();
    assert_eq!(salaries_val.indent, 0);
}

#[test]
fn test_scenario_14_group_header_depth_and_multi_line_header_width_arbitration() {
    let mut table = TableSpec::new("LboTimeline", "LBO Schedule");
    table
        .columns
        .push(ColumnSpec::descriptor("label", "($ in millions)"));
    // 1-col group ("Historical", 10 chars) + 2-line headers ("MRQ", "2012-03-31" -> max 10 chars) -> depth 3
    table.columns.push(
        ColumnSpec::new("mrq")
            .with_group("Historical")
            .with_headers(["MRQ", "2012-03-31"]),
    );
    // Multi-col group ("Quarterly Rollforward Very Long Banner Title", 44 chars) + 2-line headers ("MRQ+1", "2012-06-30" -> max 10 chars)
    table.columns.push(
        ColumnSpec::new("mrq_1")
            .with_group("Quarterly Rollforward Very Long Banner Title")
            .with_headers(["MRQ+1", "2012-06-30"]),
    );
    // Ungrouped 3-line header where top line is longest ("Standalone Financial", 20 chars) -> depth 3
    table
        .columns
        .push(ColumnSpec::new("standalone").with_headers([
            "Standalone Financial",
            "at Close",
            "2012-12-31",
        ]));

    table.rows.push(RowSpec::new("rev"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "LboWidthTest".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("LboTimeline")],
        }],
    };

    let mut theme = create_test_theme();
    theme.metrics.min_col_width = 8.0;
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let placement = layout.get_table(&TableId::from("LboTimeline")).unwrap();
    // Title at row 1, gutter at row 2, 3 header tiers at rows 3..=5
    assert_eq!(placement.header_row_range, Some((3, 5)));
    assert_eq!(placement.body_row_range, (6, 6));

    let sheet = layout.get_sheet(&SheetId::from("Sheet1")).unwrap();
    // Physical col 3 (mrq_1): group banner (44 chars) is excluded from single-column width arbitration;
    // max_header_chars is "2012-06-30" (10 chars) -> width = 10 * 1.0 + 0.0 = 10.0
    assert_eq!(sheet.col_widths.get(&3), Some(&10.0));
    // Physical col 4 (standalone): upper line "Standalone Financial" (20 chars) is included -> width = 20 * 1.0 + 0.0 = 20.0
    assert_eq!(sheet.col_widths.get(&4), Some(&20.0));
}

#[test]
fn test_scenario_15_horizontal_side_by_side_placement_and_formula_lowering() {
    let mut t1 = TableSpec::new("T1", "Left Table");
    t1.columns.push(ColumnSpec::descriptor("item", "Item"));
    t1.columns
        .push(ColumnSpec::new("2023A").with_header("2023A"));
    t1.columns
        .push(ColumnSpec::new("2024E").with_header("2024E"));
    t1.rows.push(RowSpec::new("rev"));
    populate_table_cells(&mut t1);

    let mut t2 = TableSpec::new("T2", "Right Table").with_placement(PlacementSpec::right_of("T1"));
    t2.columns.push(ColumnSpec::descriptor("metric", "Metric"));
    t2.columns.push(ColumnSpec::new("val").with_header("Value"));
    t2.rows.push(
        RowSpec::new("doubled")
            .with_cell("metric", CellInput::string("Doubled 2024E"))
            .with_cell("val", CellInput::formula(r#"T1["rev", "2024E"] * 2"#)),
    );
    t2.rows.push(
        RowSpec::new("tripled")
            .with_cell("metric", CellInput::string("From T2"))
            .with_cell("val", CellInput::formula(r#"T2["doubled", "val"] * 1.5"#)),
    );

    let manifest = ProjectManifest {
        model_name: "SideBySideModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Dashboard".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("T1"), TableId::from("T2")],
        }],
    };

    let compiled = Compiler::compile(&manifest, &[t1, t2]).unwrap();
    let theme = create_test_theme();

    let p1 = compiled.layout_map.get_table(&TableId::from("T1")).unwrap();
    assert_eq!(p1.start_col, 1);
    assert_eq!(p1.end_col, 3);
    assert_eq!(p1.start_row, 1);

    let p2 = compiled.layout_map.get_table(&TableId::from("T2")).unwrap();
    assert_eq!(p2.start_col, 5);
    assert_eq!(p2.end_col, 6);
    assert_eq!(p2.start_row, 1);

    // Physical column 4 is the 1-col horizontal gap -> receives gutter_col_width
    let sheet_layout = compiled
        .layout_map
        .get_sheet(&SheetId::from("Dashboard"))
        .unwrap();
    assert_eq!(
        sheet_layout.col_widths.get(&4),
        Some(&theme.metrics.gutter_col_width)
    );

    // T2["doubled", "val"] is at col 6 (G), row 4 (Excel row 5 -> G5)
    let doubled_cell_id = f8_core::compiler::ir::id::CellId::new("T2", "doubled", "val");
    let doubled_coord = compiled.layout_map.get_cell(&doubled_cell_id).unwrap();
    assert_eq!(doubled_coord.col, 6);
    assert_eq!(doubled_coord.row, 4);
    assert_eq!(doubled_coord.a1, "G5");

    // T2["tripled", "val"] references T2["doubled", "val"] -> lowers to G5
    let tripled_cell_id = f8_core::compiler::ir::id::CellId::new("T2", "tripled", "val");
    let tripled_coord = compiled.layout_map.get_cell(&tripled_cell_id).unwrap();
    assert_eq!(tripled_coord.a1, "G6");
    assert_eq!(
        compiled.lowered_map.get(&tripled_cell_id),
        Some("=$G$5*1.5")
    );
    assert_eq!(compiled.lowered_map.get(&doubled_cell_id), Some("=$D$5*2"));
}

#[test]
fn test_scenario_16_asymmetric_dashboard_placement_and_sheet_inspection() {
    // Tall T1: rows 1..=12 (title + title_gutter + 1 header + 9 data rows = 12 rows), cols 1..=3
    let mut t1 = TableSpec::new("T1", "Tall Left Table");
    t1.columns.push(ColumnSpec::descriptor("item", "Item"));
    t1.columns.push(ColumnSpec::new("c1").with_header("C1"));
    t1.columns.push(ColumnSpec::new("c2").with_header("C2"));
    for i in 1..=9 {
        t1.rows.push(RowSpec::new(format!("r{i}")));
    }
    populate_table_cells(&mut t1);

    // Short T2: right_of("T1"), rows 1..=4 (title + title_gutter + 1 header + 1 data row = 4 rows), cols 5..=6
    let mut t2 = TableSpec::new("T2", "Top Right").with_placement(PlacementSpec::right_of("T1"));
    t2.columns.push(ColumnSpec::descriptor("k", "Key"));
    t2.columns.push(ColumnSpec::new("v").with_header("Val"));
    t2.rows.push(RowSpec::new("r1"));
    populate_table_cells(&mut t2);

    // Short T3: below("T2"), rows 7..=10, cols 5..=6
    let mut t3 = TableSpec::new("T3", "Bottom Right").with_placement(PlacementSpec::below("T2"));
    t3.columns.push(ColumnSpec::descriptor("k", "Key"));
    t3.columns.push(ColumnSpec::new("v").with_header("Val"));
    t3.rows.push(RowSpec::new("r1"));
    populate_table_cells(&mut t3);

    // Full-width T4: stack(), placed at start_col = 1, start_row = 15 (2 gutter rows below max(12, 10) = 12)
    let mut t4 = TableSpec::new("T4", "Bottom Full Width").with_placement(PlacementSpec::stack());
    for i in 1..=6 {
        t4.columns
            .push(ColumnSpec::new(format!("c{i}")).with_header(format!("C{i}")));
    }
    t4.rows.push(RowSpec::new("r1"));
    populate_table_cells(&mut t4);

    let manifest = ProjectManifest {
        model_name: "DashboardModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Summary".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![
                TableId::from("T1"),
                TableId::from("T2"),
                TableId::from("T3"),
                TableId::from("T4"),
            ],
        }],
    };

    let compiled = Compiler::compile(&manifest, &[t1, t2, t3, t4]).unwrap();

    let p1 = compiled.layout_map.get_table(&TableId::from("T1")).unwrap();
    assert_eq!(
        (p1.start_col, p1.end_col, p1.start_row, p1.end_row),
        (1, 3, 1, 12)
    );

    let p2 = compiled.layout_map.get_table(&TableId::from("T2")).unwrap();
    assert_eq!(
        (p2.start_col, p2.end_col, p2.start_row, p2.end_row),
        (5, 6, 1, 4)
    );

    let p3 = compiled.layout_map.get_table(&TableId::from("T3")).unwrap();
    assert_eq!(
        (p3.start_col, p3.end_col, p3.start_row, p3.end_row),
        (5, 6, 7, 10)
    );

    let p4 = compiled.layout_map.get_table(&TableId::from("T4")).unwrap();
    assert_eq!(
        (p4.start_col, p4.end_col, p4.start_row, p4.end_row),
        (1, 6, 15, 18)
    );

    let sheet_insp = compiled.inspect_sheet(&SheetId::from("Summary")).unwrap();
    assert_eq!(sheet_insp.min_col, 1);
    assert_eq!(sheet_insp.max_col, 6);
    assert_eq!(sheet_insp.min_row, 1);
    assert_eq!(sheet_insp.max_row, 18);

    let xlsx_bytes = compiled
        .emit_workbook(&f8_core::compiler::EmissionOptions::default())
        .unwrap();
    assert!(xlsx_bytes.starts_with(b"PK\x03\x04"));
}

#[test]
fn test_scenario_17_custom_gap_overrides() {
    let mut t1 = TableSpec::new("T1", "Base");
    t1.columns.push(ColumnSpec::new("c1").with_header("C1"));
    t1.columns.push(ColumnSpec::new("c2").with_header("C2"));
    t1.rows.push(RowSpec::new("r1"));
    populate_table_cells(&mut t1);

    // T2: right_of("T1") with gap = 0 -> start_col = 2 + 1 + 0 = 3
    let mut t2 = TableSpec::new("T2", "Adjacent Right")
        .with_placement(PlacementSpec::right_of_with_gap("T1", 0));
    t2.columns.push(ColumnSpec::new("c1").with_header("C1"));
    t2.rows.push(RowSpec::new("r1"));
    populate_table_cells(&mut t2);

    // T3: right_of("T2") with gap = 3 -> start_col = 3 + 1 + 3 = 7 (cols 4, 5, 6 are gap columns)
    let mut t3 =
        TableSpec::new("T3", "Far Right").with_placement(PlacementSpec::right_of_with_gap("T2", 3));
    t3.columns.push(ColumnSpec::new("c1").with_header("C1"));
    t3.rows.push(RowSpec::new("r1"));
    populate_table_cells(&mut t3);

    // T4: below("T1") with gap = 4 -> T1 ends at row 4, so T4 starts at 4 + 1 + 4 = 9
    let mut t4 =
        TableSpec::new("T4", "Below Gap 4").with_placement(PlacementSpec::below_with_gap("T1", 4));
    t4.columns.push(ColumnSpec::new("c1").with_header("C1"));
    t4.rows.push(RowSpec::new("r1"));
    populate_table_cells(&mut t4);

    // T5: stack_with_gap(1) -> T4 ends at row 12, so T5 starts at 12 + 1 + 1 = 14
    let mut t5 =
        TableSpec::new("T5", "Stack Gap 1").with_placement(PlacementSpec::stack_with_gap(1));
    t5.columns.push(ColumnSpec::new("c1").with_header("C1"));
    t5.rows.push(RowSpec::new("r1"));
    populate_table_cells(&mut t5);

    let manifest = ProjectManifest {
        model_name: "GapModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![
                TableId::from("T1"),
                TableId::from("T2"),
                TableId::from("T3"),
                TableId::from("T4"),
                TableId::from("T5"),
            ],
        }],
    };

    let theme = create_test_theme();
    let tables = vec![t1, t2, t3, t4, t5];
    let (_sym, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    assert_eq!(layout.get_table(&TableId::from("T2")).unwrap().start_col, 3);
    assert_eq!(layout.get_table(&TableId::from("T3")).unwrap().start_col, 7);
    assert_eq!(layout.get_table(&TableId::from("T4")).unwrap().start_row, 9);
    assert_eq!(
        layout.get_table(&TableId::from("T5")).unwrap().start_row,
        14
    );

    let sheet_layout = layout.get_sheet(&SheetId::from("Sheet1")).unwrap();
    for gap_col in 4..=6 {
        assert_eq!(
            sheet_layout.col_widths.get(&gap_col),
            Some(&theme.metrics.gutter_col_width)
        );
    }
}

#[test]
fn test_scenario_18_2d_column_width_and_row_height_arbitration() {
    // T1 at cols 1..=2, rows 1..=10
    let mut t1 = TableSpec::new("T1", "Left Table");
    t1.columns.push(ColumnSpec::descriptor("item", "Item"));
    t1.columns.push(ColumnSpec::new("val").with_header("Val"));
    for i in 1..=7 {
        t1.rows.push(RowSpec::new(format!("r{i}")));
    }
    populate_table_cells(&mut t1);

    // T2 right_of("T1") at cols 4..=5, rows 1..=4 with short text in col 4
    let mut t2 = TableSpec::new("T2", "Right Top").with_placement(PlacementSpec::right_of("T1"));
    t2.columns.push(ColumnSpec::descriptor("desc", "Short"));
    t2.columns.push(ColumnSpec::new("val").with_header("Val"));
    t2.rows
        .push(RowSpec::new("r1").with_cell("desc", CellInput::string("Brief")));
    populate_table_cells(&mut t2);

    // T3 below("T2") at cols 4..=5, rows 7..=10 with much wider text in col 4
    let mut t3 = TableSpec::new("T3", "Right Bottom").with_placement(PlacementSpec::below("T2"));
    t3.columns.push(ColumnSpec::descriptor("desc", "Short"));
    t3.columns.push(ColumnSpec::new("val").with_header("Val"));
    let wide_label = "Extremely Wide Descriptor Label In Right Column Four";
    t3.rows
        .push(RowSpec::new("r1").with_cell("desc", CellInput::string(wide_label)));
    populate_table_cells(&mut t3);

    let manifest = ProjectManifest {
        model_name: "Arbitration2D".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![
                TableId::from("T1"),
                TableId::from("T2"),
                TableId::from("T3"),
            ],
        }],
    };

    let theme = create_test_theme();
    let tables = vec![t1, t2, t3];
    let (_sym, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let sheet = layout.get_sheet(&SheetId::from("Sheet1")).unwrap();

    // Col 3 is the horizontal gap between T1 (1..=2) and T2/T3 (4..=5)
    assert_eq!(
        sheet.col_widths.get(&3),
        Some(&theme.metrics.gutter_col_width)
    );

    // Col 4 is arbitrated across T2 and T3 -> reflects wide_label from T3
    let expected_col4 = (wide_label.chars().count() as f64 * theme.metrics.char_width_multiplier
        + theme.metrics.col_padding)
        .max(theme.metrics.min_col_width);
    assert_eq!(sheet.col_widths.get(&4), Some(&expected_col4));

    // Row 5 and 6: T1 has Data rows (data_row_height = 16.0) while T3's inter-table gutter has gutter_row_height (9.0)
    // Must retain max(16.0, 9.0) = 16.0
    assert_eq!(
        sheet.row_heights.get(&5),
        Some(&theme.metrics.data_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&6),
        Some(&theme.metrics.data_row_height)
    );

    // Row 7: T1 has Data row (16.0) while T3 has Title row (title_row_height = 26.0)
    // Must retain max(16.0, 26.0) = 26.0
    assert_eq!(
        sheet.row_heights.get(&7),
        Some(&theme.metrics.title_row_height)
    );

    // Verify row_heights keys are ordered ascending 0..=10
    let row_keys: Vec<u32> = sheet.row_heights.keys().copied().collect();
    let expected_keys: Vec<u32> = (0..=10).collect();
    assert_eq!(row_keys, expected_keys);
}

#[test]
fn test_scenario_19_invalid_placement_target_rejected() {
    let theme = create_test_theme();

    let make_simple_table = |id: &str, placement: PlacementSpec| -> TableSpec {
        let mut t = TableSpec::new(id, id).with_placement(placement);
        t.columns.push(ColumnSpec::new("c1").with_header("C1"));
        t.rows.push(RowSpec::new("r1"));
        populate_table_cells(&mut t);
        t
    };

    // 1. Non-existent target
    {
        let t1 = make_simple_table("T1", PlacementSpec::right_of("Ghost"));
        let manifest = ProjectManifest {
            model_name: "M".to_string(),
            theme: Default::default(),
            sheets: vec![SheetSpec {
                name: "S1".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("T1")],
            }],
        };
        let err = LayoutPlanner::plan(&manifest, &[t1], &ir::cell::CellGrid::new(), &theme, None)
            .unwrap_err();
        assert!(matches!(
            err,
            LayoutError::InvalidPlacementTarget { ref table, ref target, ref sheet, .. }
                if table.as_str() == "T1" && target.as_str() == "Ghost" && sheet.as_str() == "S1"
        ));
    }

    // 2. Target on a different worksheet
    {
        let t1 = make_simple_table("T1", PlacementSpec::stack());
        let t2 = make_simple_table("T2", PlacementSpec::below("T1"));
        let manifest = ProjectManifest {
            model_name: "M".to_string(),
            theme: Default::default(),
            sheets: vec![
                SheetSpec {
                    name: "S1".to_string(),
                    tab_color: None,
                    freeze_panes: None,
                    show_gridlines: true,
                    tables: vec![TableId::from("T1")],
                },
                SheetSpec {
                    name: "S2".to_string(),
                    tab_color: None,
                    freeze_panes: None,
                    show_gridlines: true,
                    tables: vec![TableId::from("T2")],
                },
            ],
        };
        let err = LayoutPlanner::plan(
            &manifest,
            &[t1, t2],
            &ir::cell::CellGrid::new(),
            &theme,
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            LayoutError::InvalidPlacementTarget { ref table, ref target, ref sheet, .. }
                if table.as_str() == "T2" && target.as_str() == "T1" && sheet.as_str() == "S2"
        ));
    }

    // 3. Target declared later on the same worksheet
    {
        let t1 = make_simple_table("T1", PlacementSpec::right_of("T2"));
        let t2 = make_simple_table("T2", PlacementSpec::stack());
        let manifest = ProjectManifest {
            model_name: "M".to_string(),
            theme: Default::default(),
            sheets: vec![SheetSpec {
                name: "S1".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("T1"), TableId::from("T2")],
            }],
        };
        let err = LayoutPlanner::plan(
            &manifest,
            &[t1, t2],
            &ir::cell::CellGrid::new(),
            &theme,
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            LayoutError::InvalidPlacementTarget { ref table, ref target, ref sheet, .. }
                if table.as_str() == "T1" && target.as_str() == "T2" && sheet.as_str() == "S1"
        ));
    }

    // 4. Self-reference (target == table.id)
    {
        let t1 = make_simple_table("T1", PlacementSpec::below("T1"));
        let manifest = ProjectManifest {
            model_name: "M".to_string(),
            theme: Default::default(),
            sheets: vec![SheetSpec {
                name: "S1".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("T1")],
            }],
        };
        let err = LayoutPlanner::plan(&manifest, &[t1], &ir::cell::CellGrid::new(), &theme, None)
            .unwrap_err();
        assert!(matches!(
            err,
            LayoutError::InvalidPlacementTarget { ref table, ref target, ref sheet, .. }
                if table.as_str() == "T1" && target.as_str() == "T1" && sheet.as_str() == "S1"
        ));
    }
}

#[test]
fn test_scenario_20_overlapping_table_bounding_boxes_rejected() {
    let theme = create_test_theme();

    // Case 1: Two tables both anchored right_of("T1") with identical gap
    {
        let mut t1 = TableSpec::new("T1", "T1");
        t1.columns.push(ColumnSpec::new("c1").with_header("C1"));
        t1.rows.push(RowSpec::new("r1"));

        let mut t2 = TableSpec::new("T2", "T2").with_placement(PlacementSpec::right_of("T1"));
        t2.columns.push(ColumnSpec::new("c1").with_header("C1"));
        t2.rows.push(RowSpec::new("r1"));

        let mut t3 = TableSpec::new("T3", "T3").with_placement(PlacementSpec::right_of("T1"));
        t3.columns.push(ColumnSpec::new("c1").with_header("C1"));
        t3.rows.push(RowSpec::new("r1"));

        let manifest = ProjectManifest {
            model_name: "OverlapModel".to_string(),
            theme: Default::default(),
            sheets: vec![SheetSpec {
                name: "S1".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![
                    TableId::from("T1"),
                    TableId::from("T2"),
                    TableId::from("T3"),
                ],
            }],
        };

        let err = LayoutPlanner::plan(
            &manifest,
            &[t1, t2, t3],
            &ir::cell::CellGrid::new(),
            &theme,
            None,
        )
        .unwrap_err();

        assert!(matches!(
            err,
            LayoutError::TableOverlap {
                ref sheet,
                ref first_table,
                ref second_table,
            } if sheet.as_str() == "S1" && first_table.as_str() == "T2" && second_table.as_str() == "T3"
        ));
    }

    // Case 2: Wide table placed below("T1") colliding with taller T2 to the right of T1
    {
        // T1: 1 col, 1 data row (rows 1..=4, col 1..=1)
        let mut t1 = TableSpec::new("T1", "T1");
        t1.columns.push(ColumnSpec::new("c1").with_header("C1"));
        t1.rows.push(RowSpec::new("r1"));

        // T2: right_of("T1"), 1 col, 6 data rows (rows 1..=9, col 3..=3)
        let mut t2 = TableSpec::new("T2", "T2").with_placement(PlacementSpec::right_of("T1"));
        t2.columns.push(ColumnSpec::new("c1").with_header("C1"));
        for i in 1..=6 {
            t2.rows.push(RowSpec::new(format!("r{i}")));
        }

        // T3: below("T1"), 4 cols (cols 1..=4, rows 7..=10) -> intersects T2 at col 3, rows 7..=9
        let mut t3 = TableSpec::new("T3", "T3").with_placement(PlacementSpec::below("T1"));
        for i in 1..=4 {
            t3.columns
                .push(ColumnSpec::new(format!("c{i}")).with_header(format!("C{i}")));
        }
        t3.rows.push(RowSpec::new("r1"));

        let manifest = ProjectManifest {
            model_name: "OverlapModel2".to_string(),
            theme: Default::default(),
            sheets: vec![SheetSpec {
                name: "S1".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![
                    TableId::from("T1"),
                    TableId::from("T2"),
                    TableId::from("T3"),
                ],
            }],
        };

        let err = LayoutPlanner::plan(
            &manifest,
            &[t1, t2, t3],
            &ir::cell::CellGrid::new(),
            &theme,
            None,
        )
        .unwrap_err();

        assert!(matches!(
            err,
            LayoutError::TableOverlap {
                ref sheet,
                ref first_table,
                ref second_table,
            } if sheet.as_str() == "S1" && first_table.as_str() == "T2" && second_table.as_str() == "T3"
        ));
    }
}
