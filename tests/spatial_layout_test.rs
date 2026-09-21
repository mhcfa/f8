#![allow(clippy::unwrap_used, clippy::expect_used)]

use finx::compiler::layout::{
    LayoutError, LayoutPlanner, MAX_EXCEL_COLUMNS, MAX_EXCEL_ROWS, SuperHeaderPlacement,
    col_to_excel_col, coordinate_to_a1,
};
use finx::compiler::{Compiler, ir};
use finx::model::format::SemanticStyle;
use finx::model::id::{ColGroupId, ColId, RowGroupId, RowId, SheetId, TableId};
use finx::model::manifest::{FreezePanes, ProjectManifest, SheetSpec};
use finx::model::table::{
    CellInput, ColumnGroup, ColumnSpec, RowGroup, RowGroupChild, RowSpec, TableSpec,
};
use finx::model::theme::Theme;
use std::path::PathBuf;

fn create_test_theme() -> Theme {
    Theme::corporate_navy()
}

fn populate_table_cells(table: &mut TableSpec) {
    for row in &mut table.rows {
        for col in &table.columns {
            if !row.cells.contains_key(&col.id) {
                if col.role.is_descriptor() {
                    row.cells
                        .insert(col.id.clone(), CellInput::string(row.id.as_str()));
                } else {
                    row.cells.insert(col.id.clone(), CellInput::number(100.0));
                }
            }
        }
    }
}

#[test]
fn test_scenario_1_default_sheet_margins_and_primary_table_origin_at_b2() {
    let mut table = TableSpec::new("SimpleTable", "");
    table.columns.push(ColumnSpec::new("col_1"));
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
            tables: vec![PathBuf::from("tables/SimpleTable.json")],
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

    let cell_id = finx::compiler::ir::id::CellId::new("SimpleTable", "row_1", "col_1");
    let cell_coord = layout.get_cell(&cell_id).unwrap();

    // Table has no title banner, but has col_header_row at row 1, data row at row 2
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
    table.columns.push(ColumnSpec::new("2023A"));
    table.columns.push(ColumnSpec::new("2024E"));

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
            tables: vec![PathBuf::from("tables/IncomeStatement.json")],
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
    assert_eq!(placement.col_header_row, 4);
    // Data rows: Rows 5 and 6
    assert_eq!(placement.data_row_range, (5, 6));

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
        .get_cell(&finx::compiler::ir::id::CellId::new(
            "IncomeStatement",
            "revenue",
            "line_item",
        ))
        .unwrap();
    assert_eq!(rev_cell.col, 1);
    assert_eq!(rev_cell.row, 5);
    assert_eq!(rev_cell.a1, "B6");

    let rev_2023a = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
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
    table.columns.push(ColumnSpec::new("2021A"));
    table.columns.push(ColumnSpec::new("2022A"));
    table.columns.push(ColumnSpec::new("2023A"));
    table.columns.push(ColumnSpec::new("2024E"));
    table.columns.push(ColumnSpec::new("2025E"));

    table.column_groups.push(ColumnGroup {
        id: ColGroupId::from("historical"),
        label: "Historical Periods".to_string(),
        columns: vec![
            ColId::from("2021A"),
            ColId::from("2022A"),
            ColId::from("2023A"),
        ],
    });

    table.column_groups.push(ColumnGroup {
        id: ColGroupId::from("forecast"),
        label: "Forecast Periods".to_string(),
        columns: vec![ColId::from("2024E"), ColId::from("2025E")],
    });

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
            tables: vec![PathBuf::from("tables/TimelineTable.json")],
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
    // Row 3: Super-header
    assert_eq!(placement.super_header_rows.len(), 2);
    assert_eq!(
        placement.super_header_rows[0],
        SuperHeaderPlacement {
            row: 3,
            group_id: "historical".to_string(),
            label: "Historical Periods".to_string(),
            start_col: 2, // 2021A is at index 1 -> physical col 2
            end_col: 4,   // 2023A is at index 3 -> physical col 4
        }
    );
    assert_eq!(
        placement.super_header_rows[1],
        SuperHeaderPlacement {
            row: 3,
            group_id: "forecast".to_string(),
            label: "Forecast Periods".to_string(),
            start_col: 5, // 2024E is at index 4 -> physical col 5
            end_col: 6,   // 2025E is at index 5 -> physical col 6
        }
    );
    // Row 4: Column Header
    assert_eq!(placement.col_header_row, 4);
    // Row 5: Data Row
    assert_eq!(placement.data_row_range, (5, 5));
}

#[test]
fn test_scenario_4_interleaved_section_headers_and_outline_levels() {
    let mut table = TableSpec::new("HierarchyTable", "Hierarchy");
    table.columns.push(ColumnSpec::descriptor("item", "Item"));
    table.columns.push(ColumnSpec::new("val"));

    table.rows.push(RowSpec::new("r_and_d"));
    table.rows.push(RowSpec::new("ad_spend"));
    table.rows.push(RowSpec::new("direct_sales"));
    table.rows.push(RowSpec::new("g_and_a"));
    populate_table_cells(&mut table);

    // Opex (depth 1) contains r_and_d, SalesMarketing sub-group, and g_and_a
    // SalesMarketing (depth 2) contains ad_spend and direct_sales
    table.row_groups.push(RowGroup {
        id: RowGroupId::from("opex"),
        label: "Operating Expenses".to_string(),
        children: vec![
            RowGroupChild::Row(RowId::from("r_and_d")),
            RowGroupChild::Group(RowGroup {
                id: RowGroupId::from("sales_marketing"),
                label: "Sales & Marketing".to_string(),
                children: vec![
                    RowGroupChild::Row(RowId::from("ad_spend")),
                    RowGroupChild::Row(RowId::from("direct_sales")),
                ],
            }),
            RowGroupChild::Row(RowId::from("g_and_a")),
        ],
    });

    let manifest = ProjectManifest {
        model_name: "TestModel".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Model".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/HierarchyTable.json")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let sheet = layout.get_sheet(&SheetId::from("Model")).unwrap();
    assert_eq!(sheet.section_header_rows.len(), 2);

    // Section 1: opex starts at r_and_d
    assert_eq!(sheet.section_header_rows[0].group_id, "opex");
    assert_eq!(sheet.section_header_rows[0].label, "Operating Expenses");
    assert_eq!(sheet.section_header_rows[0].group_level, 1);
    assert_eq!(sheet.section_header_rows[0].descriptor_col, 1);

    // Section 2: sales_marketing starts at ad_spend
    assert_eq!(sheet.section_header_rows[1].group_id, "sales_marketing");
    assert_eq!(sheet.section_header_rows[1].label, "Sales & Marketing");
    assert_eq!(sheet.section_header_rows[1].group_level, 2);

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
    assert_eq!(sheet.section_header_rows[0].row, 4);
    assert_eq!(sheet.section_header_rows[1].row, 6);

    let rd_cell = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
            "HierarchyTable",
            "r_and_d",
            "val",
        ))
        .unwrap();
    assert_eq!(rd_cell.row, 5);

    let ad_cell = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
            "HierarchyTable",
            "ad_spend",
            "val",
        ))
        .unwrap();
    assert_eq!(ad_cell.row, 7);

    let ds_cell = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
            "HierarchyTable",
            "direct_sales",
            "val",
        ))
        .unwrap();
    assert_eq!(ds_cell.row, 8);

    let ga_cell = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
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
    table1.columns.push(ColumnSpec::new("c1"));
    table1.rows.push(RowSpec::new("r1"));
    table1.rows.push(RowSpec::new("r2"));

    let mut table2 = TableSpec::new("Table2", "Second Table");
    table2.columns.push(ColumnSpec::new("c1"));
    table2.rows.push(RowSpec::new("r1"));

    let manifest = ProjectManifest {
        model_name: "MultiTable".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "StackedSheet".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![
                PathBuf::from("tables/Table1.json"),
                PathBuf::from("tables/Table2.json"),
            ],
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
    table1.columns.push(ColumnSpec::new("amount"));
    let mut row1 = RowSpec::new("long_desc_row");
    row1.cells.insert(
        ColId::from("desc"),
        CellInput::string("Very Long Descriptive Account Name Exceeding Normal Width"),
    );
    table1.rows.push(row1);

    let mut table2 = TableSpec::new("Table2", "T2");
    table2.columns.push(ColumnSpec::descriptor("desc", "Desc"));
    table2.columns.push(ColumnSpec::new("amount"));
    let mut row2 = RowSpec::new("short_row");
    row2.cells
        .insert(ColId::from("desc"), CellInput::string("Short"));
    table2.rows.push(row2);

    let manifest = ProjectManifest {
        model_name: "WidthTest".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![
                PathBuf::from("tables/Table1.json"),
                PathBuf::from("tables/Table2.json"),
            ],
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

    // Col B (descriptor) must be wide enough for the long label (58 chars * 1.15 + 2.0 = ~68.7)
    assert!(col_b_width > 60.0);

    // Col C (measure) must be at least min_col_width (12.0)
    assert!(col_c_width >= theme.metrics.min_col_width);
}

#[test]
fn test_scenario_7_full_style_role_propagation_to_physical_coordinates() {
    let mut table = TableSpec::new("SummaryTable", "Summary");
    table.columns.push(ColumnSpec::new("metric"));
    table.columns.push(
        ColumnSpec::new("col_total")
            .with_style(Some(SemanticStyle::Total))
            .with_highlight(Some(true)),
    );

    let mut row = RowSpec::new("row_total");
    row.style = Some(SemanticStyle::Total);
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
            tables: vec![PathBuf::from("tables/SummaryTable.json")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let intersection_cell = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
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
fn test_scenario_9_rejection_of_outline_levels_exceeding_maximum_depth_of_7() {
    let mut table = TableSpec::new("DeepTable", "Deep");
    table.columns.push(ColumnSpec::new("c"));
    table.rows.push(RowSpec::new("r"));

    // Build 8-level deeply nested row groups
    let mut current_group = RowGroup {
        id: RowGroupId::from("g8"),
        label: "L8".to_string(),
        children: vec![RowGroupChild::Row(RowId::from("r"))],
    };

    for i in (1..=7).rev() {
        current_group = RowGroup {
            id: RowGroupId::from(format!("g{i}")),
            label: format!("L{i}"),
            children: vec![RowGroupChild::Group(current_group)],
        };
    }

    table.row_groups.push(current_group);
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "DeepModel".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/DeepTable.json")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let err = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap_err();

    assert!(matches!(
        err,
        LayoutError::OutlineDepthExceeded { depth: 8, .. }
    ));
}

#[test]
fn test_scenario_10_rejection_of_unassigned_tables() {
    let mut table1 = TableSpec::new("AssignedTable", "A");
    table1.columns.push(ColumnSpec::new("c"));
    table1.rows.push(RowSpec::new("r"));

    let mut table2 = TableSpec::new("UnassignedTable", "U");
    table2.columns.push(ColumnSpec::new("c"));
    table2.rows.push(RowSpec::new("r"));

    let manifest = ProjectManifest {
        model_name: "TestModel".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/AssignedTable.json")],
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
    table.columns.push(ColumnSpec::new("col_a"));
    table.columns.push(ColumnSpec::new("col_b"));
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
            tables: vec![PathBuf::from("tables/FacadeTable.json")],
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
fn test_scenario_12_nested_row_group_section_header_ascending_depth_order() {
    let mut table = TableSpec::new("NestedHierarchyTable", "Nested");
    table.columns.push(ColumnSpec::descriptor("item", "Item"));
    table.columns.push(ColumnSpec::new("val"));

    table.rows.push(RowSpec::new("ad_spend"));
    table.rows.push(RowSpec::new("direct_sales"));
    table.rows.push(RowSpec::new("g_and_a"));
    populate_table_cells(&mut table);

    // Parent group "opex" (depth 1) whose first child is subgroup "sales_marketing" (depth 2)
    table.row_groups.push(RowGroup {
        id: RowGroupId::from("opex"),
        label: "Operating Expenses".to_string(),
        children: vec![
            RowGroupChild::Group(RowGroup {
                id: RowGroupId::from("sales_marketing"),
                label: "Sales & Marketing".to_string(),
                children: vec![
                    RowGroupChild::Row(RowId::from("ad_spend")),
                    RowGroupChild::Row(RowId::from("direct_sales")),
                ],
            }),
            RowGroupChild::Row(RowId::from("g_and_a")),
        ],
    });

    let manifest = ProjectManifest {
        model_name: "TestModel".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Model".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/NestedHierarchyTable.json")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let sheet = layout.get_sheet(&SheetId::from("Model")).unwrap();
    assert_eq!(sheet.section_header_rows.len(), 2);

    // opex (depth 1) MUST come before sales_marketing (depth 2)
    assert_eq!(sheet.section_header_rows[0].group_id, "opex");
    assert_eq!(sheet.section_header_rows[0].label, "Operating Expenses");
    assert_eq!(sheet.section_header_rows[0].group_level, 1);
    assert_eq!(sheet.section_header_rows[0].row, 4);

    assert_eq!(sheet.section_header_rows[1].group_id, "sales_marketing");
    assert_eq!(sheet.section_header_rows[1].label, "Sales & Marketing");
    assert_eq!(sheet.section_header_rows[1].group_level, 2);
    assert_eq!(sheet.section_header_rows[1].row, 5);

    // First data row (ad_spend) must follow at row 6
    let ad_cell = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
            "NestedHierarchyTable",
            "ad_spend",
            "val",
        ))
        .unwrap();
    assert_eq!(ad_cell.row, 6);
}

#[test]
fn test_scenario_13_section_gutter_and_descriptor_indentation() {
    let mut table = TableSpec::new("GutterTable", "Financial Statement");
    table
        .columns
        .push(ColumnSpec::descriptor("line_item", "Line Item"));
    table.columns.push(ColumnSpec::new("val"));

    table.rows.push(RowSpec::new("rev"));
    table.rows.push(RowSpec::new("salaries"));
    table.rows.push(RowSpec::new("ads"));
    table
        .rows
        .push(RowSpec::new("total_opex").with_style(Some(SemanticStyle::Subtotal)));
    table.rows.push(RowSpec::new("equipment"));
    populate_table_cells(&mut table);

    // Group 1: opex (depth 1) containing salaries and nested marketing (depth 2) containing ads
    table.row_groups.push(RowGroup {
        id: RowGroupId::from("opex"),
        label: "Operating Expenses".to_string(),
        children: vec![
            RowGroupChild::Row(RowId::from("salaries")),
            RowGroupChild::Group(RowGroup {
                id: RowGroupId::from("marketing"),
                label: "Marketing".to_string(),
                children: vec![RowGroupChild::Row(RowId::from("ads"))],
            }),
        ],
    });

    // Group 2: capex (depth 1) containing equipment
    table.row_groups.push(RowGroup {
        id: RowGroupId::from("capex"),
        label: "Capital Expenditures".to_string(),
        children: vec![RowGroupChild::Row(RowId::from("equipment"))],
    });

    let manifest = ProjectManifest {
        model_name: "GutterModel".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Model".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/GutterTable.json")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();
    let sheet = layout.get_sheet(&SheetId::from("Model")).unwrap();

    // Row sequence:
    // Row 1: Title
    // Row 2: Title Gutter (8pt)
    // Row 3: Col Header
    // Row 4: Data row "rev" (data_start_row = 4)
    // Row 5: Section Gutter (9pt) inserted before opex because current_row > data_start_row
    // Row 6: Section Header "Operating Expenses" (depth 1)
    // Row 7: Data row "salaries"
    // Row 8: Section Header "Marketing" (depth 2, NO gutter because depth > 1)
    // Row 9: Data row "ads"
    // Row 10: Data row "total_opex" (subtotal)
    // Row 11: Section Gutter (9pt) inserted before capex because current_row > data_start_row
    // Row 12: Section Header "Capital Expenditures" (depth 1)
    // Row 13: Data row "equipment"

    // Verify gutter rows
    assert_eq!(
        sheet.row_heights.get(&5),
        Some(&theme.metrics.gutter_row_height)
    );
    assert_eq!(
        sheet.row_heights.get(&11),
        Some(&theme.metrics.gutter_row_height)
    );

    // Verify section header rows
    assert_eq!(sheet.section_header_rows.len(), 3);
    assert_eq!(sheet.section_header_rows[0].group_id, "opex");
    assert_eq!(sheet.section_header_rows[0].row, 6);
    assert_eq!(sheet.section_header_rows[1].group_id, "marketing");
    assert_eq!(sheet.section_header_rows[1].row, 8);
    assert_eq!(sheet.section_header_rows[2].group_id, "capex");
    assert_eq!(sheet.section_header_rows[2].row, 12);

    // Verify physical rows
    let rev_cell = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
            "GutterTable",
            "rev",
            "line_item",
        ))
        .unwrap();
    assert_eq!(rev_cell.row, 4);
    assert_eq!(rev_cell.indent, 0); // Root row: indent 0

    let salaries_cell = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
            "GutterTable",
            "salaries",
            "line_item",
        ))
        .unwrap();
    assert_eq!(salaries_cell.row, 7);
    assert_eq!(salaries_cell.indent, 1); // Depth 1: indent 1

    let ads_cell = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
            "GutterTable",
            "ads",
            "line_item",
        ))
        .unwrap();
    assert_eq!(ads_cell.row, 9);
    assert_eq!(ads_cell.indent, 2); // Depth 2: indent 2

    let total_cell = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
            "GutterTable",
            "total_opex",
            "line_item",
        ))
        .unwrap();
    assert_eq!(total_cell.row, 10);
    assert_eq!(total_cell.indent, 0); // Subtotal outside group: indent 0

    let equip_cell = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
            "GutterTable",
            "equipment",
            "line_item",
        ))
        .unwrap();
    assert_eq!(equip_cell.row, 13);
    assert_eq!(equip_cell.indent, 1); // Depth 1: indent 1

    // Verify measure cells are NEVER indented
    let salaries_val = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
            "GutterTable",
            "salaries",
            "val",
        ))
        .unwrap();
    assert_eq!(salaries_val.indent, 0);
    let ads_val = layout
        .get_cell(&finx::compiler::ir::id::CellId::new(
            "GutterTable",
            "ads",
            "val",
        ))
        .unwrap();
    assert_eq!(ads_val.indent, 0);
}
