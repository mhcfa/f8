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
    CellInput, ColumnSpec, DetailedCellInput, RowSpec, SectionHeaderRowSpec, TableSpec,
    ValueLiteral,
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
        theme: "corporate_navy".to_string(),
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
        theme: "corporate_navy".to_string(),
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
    table
        .columns
        .push(ColumnSpec::new("2021A").with_headers(["Historical Periods", "2021A"]));
    table
        .columns
        .push(ColumnSpec::new("2022A").with_headers(["Historical Periods", "2022A"]));
    table
        .columns
        .push(ColumnSpec::new("2023A").with_headers(["Historical Periods", "2023A"]));
    table
        .columns
        .push(ColumnSpec::new("2024E").with_headers(["Forecast Periods", "2024E"]));
    table
        .columns
        .push(ColumnSpec::new("2025E").with_headers(["Forecast Periods", "2025E"]));

    table.rows.push(RowSpec::new("metric"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "TestModel".to_string(),
        theme: "corporate_navy".to_string(),
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
        theme: "corporate_navy".to_string(),
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
        theme: "corporate_navy".to_string(),
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
        theme: "corporate_navy".to_string(),
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

    // Col B (descriptor) must be wide enough for the long label (57 chars * 1.0 + 1.0 = 58.0)
    assert_eq!(col_b_width, 58.0);

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
        theme: "corporate_navy".to_string(),
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
        theme: "corporate_navy".to_string(),
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
        theme: "corporate_navy".to_string(),
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
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Summary".to_string(),
            tab_color: Some("#1B365D".to_string()),
            freeze_panes: Some(FreezePanes { row: 2, col: 2 }),
            show_gridlines: true,
            tables: vec![TableId::from("FacadeTable")],
        }],
    };

    let theme = create_test_theme();
    let layout = Compiler::plan_layout(&manifest, &[table], &theme, None).unwrap();

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
        theme: "corporate_navy".to_string(),
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
        theme: "corporate_navy".to_string(),
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
