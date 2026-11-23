#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

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
    let mut table =
        TableSpec::new("IncomeStatement", "Income Statement").with_subtitle("($ in thousands)");
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
    assert_eq!(placement.subtitle_row_range, Some((2, 2)));
    // Row 3: Title gutter
    assert_eq!(placement.title_gutter_row, Some(3));
    // Row 4: Column header
    assert_eq!(placement.header_row_range, Some((4, 4)));
    // Data rows: Rows 5 and 6
    assert_eq!(placement.body_row_range, (5, 6));

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
fn test_scenario_2b_multi_row_subtitles_and_subtitle_only_table_banner() {
    // 1. Multi-row subtitles with title banner
    let mut table_with_title = TableSpec::new("Assumptions", "Assumptions")
        .with_subtitles(["($ in Millions)", "Audited FY22-FY24; Projected FY25-FY28"]);
    table_with_title
        .columns
        .push(ColumnSpec::descriptor("param", "Parameter"));
    table_with_title
        .columns
        .push(ColumnSpec::new("val").with_header("Value"));
    table_with_title.rows.push(RowSpec::new("tax_rate"));
    populate_table_cells(&mut table_with_title);

    // 2. Subtitle-only table banner (empty title with multi-line subtitles)
    let mut subtitle_only = TableSpec::new("SubOnly", "").with_subtitles(["Line 1", "Line 2"]);
    subtitle_only
        .columns
        .push(ColumnSpec::new("c1").with_header("Col 1"));
    subtitle_only.rows.push(RowSpec::new("r1"));
    populate_table_cells(&mut subtitle_only);

    let manifest = ProjectManifest {
        model_name: "MultiSubtitleModel".to_string(),
        theme: Default::default(),
        sheets: vec![
            SheetSpec {
                name: "Sheet1".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("Assumptions")],
            },
            SheetSpec {
                name: "Sheet2".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("SubOnly")],
            },
        ],
    };

    let theme = create_test_theme();
    let tables = vec![table_with_title, subtitle_only];
    let (_sym, grid) = ir::expand_workbook(&manifest, &tables).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &tables, &grid, &theme, None).unwrap();

    let p1 = layout.get_table(&TableId::from("Assumptions")).unwrap();
    assert_eq!(p1.title_row, Some(1));
    assert_eq!(p1.subtitle_row_range, Some((2, 3)));
    assert_eq!(p1.title_gutter_row, Some(4));
    assert_eq!(p1.header_row_range, Some((5, 5)));
    assert_eq!(p1.body_row_range, (6, 6));

    let p2 = layout.get_table(&TableId::from("SubOnly")).unwrap();
    assert_eq!(p2.title_row, None);
    assert_eq!(p2.subtitle_row_range, Some((1, 2)));
    assert_eq!(p2.title_gutter_row, Some(3));
    assert_eq!(p2.header_row_range, Some((4, 4)));
    assert_eq!(p2.body_row_range, (5, 5));
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

    // Table 2 (after 2 inter-table gutter rows 6 and 7):
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

    // Col B (descriptor) has a 57-char label in Table1 vs 5-char in Table2 -> 57.0
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
fn test_scenario_9_multi_tier_column_header_row_range_allocation() {
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
    // Row 2: Title Gutter
    // Row 3: Col Header (Tier 0)
    // Row 4: Data row "rev"
    // Row 5: Explicit RowSpec::Gutter
    // Row 6: Section Header "Operating Expenses"
    // Row 7: Data row "salaries"
    // Row 8: Data row "ads"
    // Row 9: Data row "total_opex" (subtotal)
    // Row 10: Explicit RowSpec::Gutter
    // Row 11: Section Header "Capital Expenditures"
    // Row 12: Data row "equipment"

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
    // Physical col 1 (label, descriptor): "($ in millions)" (15 chars) remains unpadded -> 15.0
    assert_eq!(sheet.col_widths.get(&1), Some(&15.0));
    // Physical col 3 (mrq_1, measure): group banner (44 chars) is excluded from single-column width arbitration;
    // max_header_chars is "2012-06-30" (10 chars) + 1 trailing alignment pad -> width = 11 * 1.0 + 0.0 = 11.0
    assert_eq!(sheet.col_widths.get(&3), Some(&11.0));
    // Physical col 4 (standalone, measure): upper line "Standalone Financial" (20 chars) + 1 trailing pad -> 21.0
    assert_eq!(sheet.col_widths.get(&4), Some(&21.0));
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
fn test_scenario_18_2d_column_width_arbitration() {
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

    // Col 4 is arbitrated across T2 and T3 -> reflects wide_label from T3 (clamped to max_col_width)
    let expected_col4 = (wide_label.chars().count() as f64 * theme.metrics.char_width_multiplier
        + theme.metrics.col_padding)
        .clamp(theme.metrics.min_col_width, theme.metrics.max_col_width);
    assert_eq!(sheet.col_widths.get(&4), Some(&expected_col4));
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

#[test]
fn test_scenario_21_column_width_clamping_and_multiline_wrap_text() {
    let mut table = TableSpec::new("WrapTable", "Text Wrapping");
    table
        .columns
        .push(ColumnSpec::descriptor("desc", "Description"));
    table
        .columns
        .push(ColumnSpec::new("notes").with_header("Notes"));

    let long_180 = "A".repeat(180);
    let exact_75 = "B".repeat(75);
    let over_76 = "C".repeat(76);

    // Row 1: 180-char descriptor text -> 3 lines (wrap_text = true), short measure
    table.rows.push(
        RowSpec::new("r_long")
            .with_cell("desc", CellInput::string(long_180))
            .with_cell("notes", CellInput::string("Short note")),
    );
    // Row 2: short descriptor (<= max_col_width, no \n) -> unwrapped (wrap_text = false)
    table.rows.push(
        RowSpec::new("r_short")
            .with_cell("desc", CellInput::string("Brief description"))
            .with_cell("notes", CellInput::string("OK")),
    );
    // Row 3: exact 75-char boundary -> unwrapped (wrap_text = false)
    table.rows.push(
        RowSpec::new("r_exact_75")
            .with_cell("desc", CellInput::string(exact_75))
            .with_cell("notes", CellInput::string("OK")),
    );
    // Row 4: 76-char measure text -> 2 lines (wrap_text = true)
    table.rows.push(
        RowSpec::new("r_over_76")
            .with_cell("desc", CellInput::string("Item"))
            .with_cell("notes", CellInput::string(over_76)),
    );

    let manifest = ProjectManifest {
        model_name: "WrapModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("WrapTable")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, std::slice::from_ref(&table)).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let sheet = layout.get_sheet(&SheetId::from("Sheet1")).unwrap();
    // Both columns have cells > 75 chars -> clamped to max_col_width (75.0)
    assert_eq!(sheet.col_widths.get(&1), Some(&75.0));
    assert_eq!(sheet.col_widths.get(&2), Some(&75.0));

    // Row 1 (physical row 4): 180 chars -> ceil(180 / 75) = 3 lines -> wrap_text = true
    let long_desc = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "WrapTable",
            "r_long",
            "desc",
        ))
        .unwrap();
    let long_notes = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "WrapTable",
            "r_long",
            "notes",
        ))
        .unwrap();
    assert!(long_desc.wrap_text);
    assert!(!long_notes.wrap_text);

    // Row 2 (physical row 5): short text -> wrap_text = false
    let short_desc = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "WrapTable",
            "r_short",
            "desc",
        ))
        .unwrap();
    assert!(!short_desc.wrap_text);

    // Row 3 (physical row 6): exact 75 chars -> wrap_text = false
    let exact_desc = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "WrapTable",
            "r_exact_75",
            "desc",
        ))
        .unwrap();
    assert!(!exact_desc.wrap_text);

    // Row 4 (physical row 7): 76 chars in measure column -> wrap_text = true
    let over_notes = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "WrapTable",
            "r_over_76",
            "notes",
        ))
        .unwrap();
    assert!(over_notes.wrap_text);
}

#[test]
fn test_scenario_22_explicit_newline_text_wrapping_and_segment_measurement() {
    let mut table = TableSpec::new("NewlineTable", "Newline Wrapping");
    table
        .columns
        .push(ColumnSpec::descriptor("desc", "Description"));
    table
        .columns
        .push(ColumnSpec::new("val").with_header("Value"));

    // Row 1: "Line one\nLine two breakdown" (8 chars, 18 chars) -> longest segment is 18 chars, 2 lines
    table.rows.push(
        RowSpec::new("r_two_lines")
            .with_cell("desc", CellInput::string("Line one\nLine two breakdown"))
            .with_cell("val", CellInput::number(100.0)),
    );

    // Row 2: Explicit newline where second segment also wraps ("Header:\n" + 80 chars -> 1 + 2 = 3 lines)
    let mixed_multiline = format!("Header:\n{}", "X".repeat(80));
    table.rows.push(
        RowSpec::new("r_mixed")
            .with_cell("desc", CellInput::string("Normal"))
            .with_cell("val", CellInput::string(mixed_multiline)),
    );

    let manifest = ProjectManifest {
        model_name: "NewlineModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("NewlineTable")],
        }],
    };

    let theme = create_test_theme();
    let (_sym, grid) = ir::expand_workbook(&manifest, std::slice::from_ref(&table)).unwrap();
    let layout = LayoutPlanner::plan(&manifest, &[table], &grid, &theme, None).unwrap();

    let sheet = layout.get_sheet(&SheetId::from("Sheet1")).unwrap();
    // Col 1 ("desc"): longest segment across rows is "Line two breakdown" (18 chars), not full string length (27 chars)
    assert_eq!(sheet.col_widths.get(&1), Some(&18.0));
    // Col 2 ("val"): second segment is 80 chars -> clamped to 75.0
    assert_eq!(sheet.col_widths.get(&2), Some(&75.0));

    let two_line_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "NewlineTable",
            "r_two_lines",
            "desc",
        ))
        .unwrap();
    assert!(two_line_cell.wrap_text);

    let mixed_cell = layout
        .get_cell(&f8_core::compiler::ir::id::CellId::new(
            "NewlineTable",
            "r_mixed",
            "val",
        ))
        .unwrap();
    assert!(mixed_cell.wrap_text);
}

#[test]
fn test_scenario_23_trailing_alignment_space_width_arbitration() {
    use f8_core::model::format::FormatPreset;
    use f8_core::model::id::RowId;
    use f8_core::model::theme::ThemeSpec;

    let mut table = TableSpec::new("AlignWidthTable", "Alignment Width Arbitration");
    // Col 1 (B): Unformatted descriptor driven by header ("DescriptorHeader", 16 chars) -> 16.0 (unpadded)
    table
        .columns
        .push(ColumnSpec::descriptor("desc_hdr", "DescriptorHeader"));
    // Col 2 (C): Unformatted descriptor driven by text cell ("UnformattedDescCell", 19 chars) -> 19.0 (unpadded)
    table
        .columns
        .push(ColumnSpec::descriptor("desc_txt", "Desc"));
    // Col 3 (D): Unformatted descriptor driven by number cell (123456789 -> "123,456,789", 11 chars) -> 11.0 (unpadded)
    table
        .columns
        .push(ColumnSpec::descriptor("desc_num", "Num"));
    // Col 4 (E): Multi-tier measure header (upper "Upper Measure Hdr", 17 chars) -> 17 + 1 = 18.0
    table
        .columns
        .push(ColumnSpec::new("meas_hdr").with_headers(["Upper Measure Hdr", "Short"]));
    // Col 5 (F): Empty-header measure column with null value -> min_col_width (0.5, no +1 pad on empty header)
    table.columns.push(ColumnSpec::new("meas_empty"));
    // Col 6 (G): Empty-header measure column with empty string "" -> min_col_width (0.5, no +1 pad on empty header or empty segment)
    table.columns.push(ColumnSpec::new("meas_empty_str"));
    // Col 7 (H): Date measure column (45657 -> "2024-12-31", 10 chars) -> 10 + 1 = 11.0
    table.columns.push(
        ColumnSpec::new("meas_date")
            .with_header("Date")
            .with_format(Some(FormatPreset::Date)),
    );
    // Col 8 (I): Date measure column with text cell ("LongDateTextNote", 16 chars) -> 16.0 (no +1 pad for text under Date)
    table.columns.push(
        ColumnSpec::new("meas_date_txt")
            .with_header("Date")
            .with_format(Some(FormatPreset::Date)),
    );
    // Col 9 (J): Unformatted measure column with positive number (12345678 -> "12,345,678", 10 chars) -> 10 + 1 = 11.0
    table
        .columns
        .push(ColumnSpec::new("meas_raw_pos").with_header("Val"));
    // Col 10 (K): Unformatted measure column with negative number (-12345678 -> "(12,345,678)", 12 chars, ends in ')') -> 12.0
    table
        .columns
        .push(ColumnSpec::new("meas_raw_neg").with_header("Val"));
    // Col 11 (L): Unformatted measure column with text ("MeasureTextLabel", 16 chars) -> 16 + 1 = 17.0
    table
        .columns
        .push(ColumnSpec::new("meas_raw_txt").with_header("Val"));
    // Col 12 (M): Currency-formatted measure column holding text ("NotApplicableText", 17 chars) -> 17 + 1 = 18.0
    table.columns.push(
        ColumnSpec::new("meas_curr_txt")
            .with_header("Rev")
            .with_format(Some(FormatPreset::Currency)),
    );

    table.rows.push(
        RowSpec::new("r1")
            .with_cell("desc_hdr", CellInput::string("Short"))
            .with_cell("desc_txt", CellInput::string("UnformattedDescCell"))
            .with_cell("desc_num", CellInput::number(123_456_789.0))
            .with_cell("meas_hdr", CellInput::number(1.0))
            .with_cell("meas_empty", CellInput::null())
            .with_cell("meas_empty_str", CellInput::string(""))
            .with_cell("meas_date", CellInput::number(45_657.0))
            .with_cell("meas_date_txt", CellInput::string("LongDateTextNote"))
            .with_cell("meas_raw_pos", CellInput::number(12_345_678.0))
            .with_cell("meas_raw_neg", CellInput::number(-12_345_678.0))
            .with_cell("meas_raw_txt", CellInput::string("MeasureTextLabel"))
            .with_cell("meas_curr_txt", CellInput::string("NotApplicableText")),
    );

    let mut theme = create_test_theme();
    theme.metrics.min_col_width = 0.5;
    theme.metrics.char_width_multiplier = 1.0;
    theme.metrics.col_padding = 0.0;

    let manifest = ProjectManifest {
        model_name: "AlignWidthModel".to_string(),
        theme: ThemeSpec::Custom(Box::new(theme.clone())),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("AlignWidthTable")],
        }],
    };

    let assert_widths = |sheet: &f8_core::compiler::layout::SheetLayout, label: &str| {
        let expected: &[(u32, f64, &str)] = &[
            (1, 16.0, "descriptor header unpadded"),
            (2, 19.0, "unformatted descriptor text unpadded"),
            (3, 11.0, "unformatted descriptor number unpadded"),
            (4, 18.0, "multi-tier measure header +1"),
            (5, 0.5, "empty-header measure column stays at min_col_width"),
            (6, 0.5, "empty-string measure column stays at min_col_width"),
            (7, 11.0, "date measure value +1"),
            (8, 16.0, "text in date column unpadded"),
            (9, 11.0, "unformatted measure positive number +1"),
            (
                10,
                12.0,
                "unformatted measure negative number already ends with ')'",
            ),
            (11, 17.0, "unformatted measure text +1"),
            (12, 18.0, "numeric-formatted measure text +1"),
        ];
        for &(col_idx, width, desc) in expected {
            assert_eq!(
                sheet.col_widths.get(&col_idx),
                Some(&width),
                "[{label}] col {col_idx} ({desc})"
            );
        }
    };

    // 1. Unevaluated path (measure_literal_value)
    let (_sym, grid) = ir::expand_workbook(&manifest, std::slice::from_ref(&table)).unwrap();
    let uneval_layout =
        LayoutPlanner::plan(&manifest, std::slice::from_ref(&table), &grid, &theme, None).unwrap();
    assert_widths(
        uneval_layout.get_sheet(&SheetId::from("Sheet1")).unwrap(),
        "unevaluated",
    );

    // 2. Full compiled / evaluated path (measure_cell_value)
    let compiled = Compiler::compile(&manifest, &[table]).unwrap();
    assert_widths(
        compiled
            .layout_map
            .get_sheet(&SheetId::from("Sheet1"))
            .unwrap(),
        "evaluated",
    );

    // 3. In-memory formatted values remain free of Excel _) spacing tokens
    let tbl_id = TableId::from("AlignWidthTable");
    let row_id = RowId::from("r1");
    assert_eq!(
        compiled
            .cell_formatted(&tbl_id, &row_id, &ColId::from("meas_date"))
            .unwrap(),
        "2024-12-31"
    );
    assert_eq!(
        compiled
            .cell_formatted(&tbl_id, &row_id, &ColId::from("meas_raw_pos"))
            .unwrap(),
        "12,345,678"
    );
    assert_eq!(
        compiled
            .cell_formatted(&tbl_id, &row_id, &ColId::from("meas_raw_neg"))
            .unwrap(),
        "(12,345,678)"
    );
    assert_eq!(
        compiled
            .cell_formatted(&tbl_id, &row_id, &ColId::from("meas_raw_txt"))
            .unwrap(),
        "MeasureTextLabel"
    );
}

fn build_banner_test_table(
    id: &str,
    title: &str,
    num_cols: usize,
    num_rows: usize,
    placement: PlacementSpec,
) -> TableSpec {
    let mut table = TableSpec::new(id, title).with_placement(placement);
    for c in 1..=num_cols {
        table
            .columns
            .push(ColumnSpec::new(format!("c{c}")).with_header(format!("C{c}")));
    }
    for r in 1..=num_rows {
        table.rows.push(RowSpec::new(format!("r{r}")));
    }
    populate_table_cells(&mut table);
    table
}

fn single_sheet_manifest(model_name: &str, tables: &[TableSpec]) -> ProjectManifest {
    ProjectManifest {
        model_name: model_name.to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: tables.iter().map(|t| t.id.clone()).collect(),
        }],
    }
}

#[test]
fn test_aligned_title_banners_scenario_1_stacked_varying_widths_and_sheet_isolation() {
    let t1 = build_banner_test_table(
        "NarrowTop",
        "Very Long Narrow Schedule Banner Title That Must Not Widen Columns",
        2,
        1,
        PlacementSpec::stack(),
    );
    let t2 = build_banner_test_table("WideMid", "Wide Middle", 5, 1, PlacementSpec::stack());
    let t3 = build_banner_test_table(
        "MedBot",
        "Medium Bottom",
        3,
        1,
        PlacementSpec::below("WideMid"),
    );
    let other_sheet_wide = build_banner_test_table(
        "OtherWide",
        "Other Sheet Wide",
        10,
        1,
        PlacementSpec::stack(),
    );

    let manifest = ProjectManifest {
        model_name: "StackedBanners".to_string(),
        theme: Default::default(),
        sheets: vec![
            SheetSpec {
                name: "Sheet1".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![
                    TableId::from("NarrowTop"),
                    TableId::from("WideMid"),
                    TableId::from("MedBot"),
                ],
            },
            SheetSpec {
                name: "Sheet2".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("OtherWide")],
            },
        ],
    };

    let tables = vec![t1, t2, t3, other_sheet_wide];
    let compiled = Compiler::compile(&manifest, &tables).unwrap();

    let p1 = compiled
        .layout_map
        .get_table(&TableId::from("NarrowTop"))
        .unwrap();
    let p2 = compiled
        .layout_map
        .get_table(&TableId::from("WideMid"))
        .unwrap();
    let p3 = compiled
        .layout_map
        .get_table(&TableId::from("MedBot"))
        .unwrap();
    let p_other = compiled
        .layout_map
        .get_table(&TableId::from("OtherWide"))
        .unwrap();

    // Own bounding boxes remain unchanged (2, 5, 3 columns starting at col 1)
    assert_eq!((p1.start_col, p1.end_col), (1, 2));
    assert_eq!((p2.start_col, p2.end_col), (1, 5));
    assert_eq!((p3.start_col, p3.end_col), (1, 3));

    // All stacked titled tables on Sheet1 align title_end_col to Sheet1 max (col 5),
    // unaffected by Sheet2's 10-column table (col 10)
    assert_eq!(p1.title_row, Some(1));
    assert_eq!(p1.title_end_col, Some(5));
    assert_eq!(p2.title_row, Some(7));
    assert_eq!(p2.title_end_col, Some(5));
    assert_eq!(p3.title_row, Some(13));
    assert_eq!(p3.title_end_col, Some(5));
    assert_eq!(p_other.title_end_col, Some(10));

    // Column widths remain driven strictly by cell/header content, not by long title text
    let sheet1 = compiled
        .layout_map
        .get_sheet(&SheetId::from("Sheet1"))
        .unwrap();
    let theme = create_test_theme();
    for col in 1..=5 {
        assert_eq!(
            sheet1.col_widths.get(&col),
            Some(&theme.metrics.min_col_width)
        );
    }
}

#[test]
fn test_aligned_title_banners_scenario_2_untitled_widest_table_sets_sheet_edge() {
    let narrow = build_banner_test_table("NarrowTitled", "Summary", 2, 1, PlacementSpec::stack());
    let wide_untitled = build_banner_test_table("WideUntitled", "", 6, 1, PlacementSpec::stack());

    let tables = vec![narrow, wide_untitled];
    let manifest = single_sheet_manifest("UntitledWidest", &tables);
    let compiled = Compiler::compile(&manifest, &tables).unwrap();

    let p_narrow = compiled
        .layout_map
        .get_table(&TableId::from("NarrowTitled"))
        .unwrap();
    let p_wide = compiled
        .layout_map
        .get_table(&TableId::from("WideUntitled"))
        .unwrap();

    assert_eq!((p_narrow.start_col, p_narrow.end_col), (1, 2));
    assert_eq!(p_narrow.title_row, Some(1));
    assert_eq!(p_narrow.title_end_col, Some(6));

    assert_eq!((p_wide.start_col, p_wide.end_col), (1, 6));
    assert_eq!(p_wide.title_row, None);
    assert_eq!(p_wide.title_end_col, None);
}

#[test]
fn test_aligned_title_banners_scenario_3_side_by_side_left_lane_and_right_stack() {
    // Tall left table L: B..C (cols 1..=2), rows 1..=30 (title + gutter + header + 27 data rows)
    let left = build_banner_test_table("L", "Left Lane", 2, 27, PlacementSpec::stack());
    // Three stacked right-lane tables via right_of(L) and below: E..G (3 cols), E..F (2 cols), E..J (6 cols)
    let r1 = build_banner_test_table("R1", "Right 1", 3, 2, PlacementSpec::right_of("L"));
    let r2 = build_banner_test_table("R2", "Right 2", 2, 2, PlacementSpec::below("R1"));
    let r3 = build_banner_test_table("R3", "Right 3", 6, 2, PlacementSpec::below("R2"));

    let tables = vec![left, r1, r2, r3];
    let manifest = single_sheet_manifest("SideBySideStack", &tables);
    let compiled = Compiler::compile(&manifest, &tables).unwrap();

    let pl = compiled.layout_map.get_table(&TableId::from("L")).unwrap();
    let pr1 = compiled.layout_map.get_table(&TableId::from("R1")).unwrap();
    let pr2 = compiled.layout_map.get_table(&TableId::from("R2")).unwrap();
    let pr3 = compiled.layout_map.get_table(&TableId::from("R3")).unwrap();

    assert_eq!(
        (pl.start_col, pl.end_col, pl.start_row, pl.end_row),
        (1, 2, 1, 30)
    );
    assert_eq!(pl.title_end_col, Some(2));

    assert_eq!((pr1.start_col, pr1.end_col), (4, 6));
    assert_eq!((pr2.start_col, pr2.end_col), (4, 5));
    assert_eq!((pr3.start_col, pr3.end_col), (4, 9));
    assert_eq!(pr1.title_end_col, Some(9));
    assert_eq!(pr2.title_end_col, Some(9));
    assert_eq!(pr3.title_end_col, Some(9));
}

#[test]
fn test_aligned_title_banners_scenario_4_mixed_dashboard() {
    let top_left = build_banner_test_table("TopLeft", "Top Left", 3, 2, PlacementSpec::stack());
    let top_right = build_banner_test_table(
        "TopRight",
        "Top Right",
        5,
        2,
        PlacementSpec::right_of("TopLeft"),
    );
    let bottom_1 = build_banner_test_table("Bottom1", "Bottom 1", 7, 2, PlacementSpec::stack());
    let bottom_2 = build_banner_test_table("Bottom2", "Bottom 2", 5, 2, PlacementSpec::stack());

    let tables = vec![top_left, top_right, bottom_1, bottom_2];
    let manifest = single_sheet_manifest("MixedDashboard", &tables);
    let compiled = Compiler::compile(&manifest, &tables).unwrap();

    let p_tl = compiled
        .layout_map
        .get_table(&TableId::from("TopLeft"))
        .unwrap();
    let p_tr = compiled
        .layout_map
        .get_table(&TableId::from("TopRight"))
        .unwrap();
    let p_b1 = compiled
        .layout_map
        .get_table(&TableId::from("Bottom1"))
        .unwrap();
    let p_b2 = compiled
        .layout_map
        .get_table(&TableId::from("Bottom2"))
        .unwrap();

    assert_eq!((p_tl.start_col, p_tl.end_col), (1, 3));
    assert_eq!((p_tr.start_col, p_tr.end_col), (5, 9));
    assert_eq!((p_b1.start_col, p_b1.end_col), (1, 7));
    assert_eq!((p_b2.start_col, p_b2.end_col), (1, 5));

    assert_eq!(p_tl.title_end_col, Some(3));
    assert_eq!(p_tr.title_end_col, Some(9));
    assert_eq!(p_b1.title_end_col, Some(9));
    assert_eq!(p_b2.title_end_col, Some(9));
}

#[test]
fn test_aligned_title_banners_scenario_5_stacked_obstructed_left_lane_cards() {
    let left_1 = build_banner_test_table("Left1", "Left Card 1", 2, 2, PlacementSpec::stack());
    let right = build_banner_test_table(
        "Right",
        "Tall Right Panel",
        3,
        12,
        PlacementSpec::right_of_with_gap("Left1", 2),
    );
    let left_2 =
        build_banner_test_table("Left2", "Left Card 2", 3, 2, PlacementSpec::below("Left1"));

    let tables = vec![left_1, right, left_2];
    let manifest = single_sheet_manifest("LeftLaneCards", &tables);
    let compiled = Compiler::compile(&manifest, &tables).unwrap();

    let p_l1 = compiled
        .layout_map
        .get_table(&TableId::from("Left1"))
        .unwrap();
    let p_r = compiled
        .layout_map
        .get_table(&TableId::from("Right"))
        .unwrap();
    let p_l2 = compiled
        .layout_map
        .get_table(&TableId::from("Left2"))
        .unwrap();

    assert_eq!((p_l1.start_col, p_l1.end_col), (1, 2));
    assert_eq!((p_l2.start_col, p_l2.end_col), (1, 3));
    assert_eq!((p_r.start_col, p_r.end_col), (5, 7));
    // Right overlaps both Left1 and Left2 vertically
    assert!(p_r.start_row <= p_l1.end_row && p_l1.start_row <= p_r.end_row);
    assert!(p_r.start_row <= p_l2.end_row && p_l2.start_row <= p_r.end_row);

    // Both left-lane cards align to col 3 (D), leaving gutter col 4 (E) blank before Right (col 5)
    assert_eq!(p_l1.title_end_col, Some(3));
    assert_eq!(p_l2.title_end_col, Some(3));
    assert_eq!(p_r.title_end_col, Some(7));
}

#[test]
fn test_aligned_title_banners_scenario_6_explicit_gap_zero_and_gap_gte_two() {
    // Part (a): explicit gap = 0
    let t1 = build_banner_test_table("T1", "Left Zero Gap", 2, 1, PlacementSpec::stack());
    let t2 = build_banner_test_table(
        "T2",
        "Right Zero Gap",
        2,
        1,
        PlacementSpec::right_of_with_gap("T1", 0),
    );
    let tables_a = vec![t1, t2];
    let manifest_a = single_sheet_manifest("GapZero", &tables_a);
    let compiled_a = Compiler::compile(&manifest_a, &tables_a).unwrap();

    let pt1 = compiled_a
        .layout_map
        .get_table(&TableId::from("T1"))
        .unwrap();
    let pt2 = compiled_a
        .layout_map
        .get_table(&TableId::from("T2"))
        .unwrap();
    assert_eq!((pt1.start_col, pt1.end_col), (1, 2));
    assert_eq!((pt2.start_col, pt2.end_col), (3, 4));
    assert_eq!(pt1.title_end_col, Some(2));
    assert_eq!(pt2.title_end_col, Some(4));

    // Part (b): explicit gap = 2 above an unobstructed stacked wider table C
    let a = build_banner_test_table("A", "Top Left", 2, 1, PlacementSpec::stack());
    let b = build_banner_test_table(
        "B",
        "Top Right",
        3,
        1,
        PlacementSpec::right_of_with_gap("A", 2),
    );
    let c = build_banner_test_table("C", "Bottom Stack", 3, 1, PlacementSpec::stack());
    let tables_b = vec![a, b, c];
    let manifest_b = single_sheet_manifest("GapTwoPreserved", &tables_b);
    let compiled_b = Compiler::compile(&manifest_b, &tables_b).unwrap();

    let pa = compiled_b
        .layout_map
        .get_table(&TableId::from("A"))
        .unwrap();
    let pb = compiled_b
        .layout_map
        .get_table(&TableId::from("B"))
        .unwrap();
    let pc = compiled_b
        .layout_map
        .get_table(&TableId::from("C"))
        .unwrap();
    assert_eq!((pa.start_col, pa.end_col), (1, 2));
    assert_eq!((pb.start_col, pb.end_col), (5, 7));
    assert_eq!((pc.start_col, pc.end_col), (1, 3));
    // A preserves both explicit gap columns (3..=4); C is unobstructed and does not pull A right
    assert_eq!(pa.title_end_col, Some(2));
    assert_eq!(pb.title_end_col, Some(7));
    assert_eq!(pc.title_end_col, Some(7));
}

#[test]
fn test_aligned_title_banners_scenario_7_gap_row_vertical_overlap_guard() {
    // L1: rows 1..=4 (1 data row)
    let l1 = build_banner_test_table("L1", "Left 1", 2, 1, PlacementSpec::stack());
    // L2: below(L1), gap=2 -> rows 7..=15 (6 data rows; title_row == 7)
    let l2 = build_banner_test_table("L2", "Left 2", 2, 6, PlacementSpec::below("L1"));
    // R1: right_of(L1) -> rows 1..=5 (2 data rows)
    let r1 = build_banner_test_table("R1", "Right 1", 3, 2, PlacementSpec::right_of("L1"));
    // R2: below(R1), gap=2 -> rows 8..=15 (5 data rows)
    let r2 = build_banner_test_table("R2", "Right 2", 3, 5, PlacementSpec::below("R1"));

    let tables = vec![l1, l2, r1, r2];
    let manifest = single_sheet_manifest("GapRowGuard", &tables);
    let compiled = Compiler::compile(&manifest, &tables).unwrap();

    let pl1 = compiled.layout_map.get_table(&TableId::from("L1")).unwrap();
    let pl2 = compiled.layout_map.get_table(&TableId::from("L2")).unwrap();
    let pr1 = compiled.layout_map.get_table(&TableId::from("R1")).unwrap();
    let pr2 = compiled.layout_map.get_table(&TableId::from("R2")).unwrap();

    assert_eq!((pl1.start_row, pl1.end_row), (1, 4));
    assert_eq!((pl2.start_row, pl2.end_row), (7, 15));
    assert_eq!(pl2.title_row, Some(7));
    assert_eq!((pr1.start_row, pr1.end_row), (1, 5));
    assert_eq!((pr2.start_row, pr2.end_row), (8, 15));

    // L2.title_row (7) sits in the blank vertical gap between R1 (end 5) and R2 (start 8),
    // but L2's vertical band (7..=15) overlaps R2 (8..=15), blocking L2 at its own end_col.
    assert_eq!(pl1.title_end_col, Some(pl1.end_col));
    assert_eq!(pl2.title_end_col, Some(pl2.end_col));
    assert_eq!(pr1.title_end_col, Some(6));
    assert_eq!(pr2.title_end_col, Some(6));
}

#[test]
fn test_aligned_title_banners_scenario_8_untitled_right_neighbor_blocks_left_banner() {
    let left = build_banner_test_table("LeftTitled", "Left Titled", 2, 1, PlacementSpec::stack());
    let right = build_banner_test_table(
        "RightUntitled",
        "",
        3,
        1,
        PlacementSpec::right_of("LeftTitled"),
    );

    let tables = vec![left, right];
    let manifest = single_sheet_manifest("UntitledBlocker", &tables);
    let compiled = Compiler::compile(&manifest, &tables).unwrap();

    let p_left = compiled
        .layout_map
        .get_table(&TableId::from("LeftTitled"))
        .unwrap();
    let p_right = compiled
        .layout_map
        .get_table(&TableId::from("RightUntitled"))
        .unwrap();

    assert_eq!((p_left.start_col, p_left.end_col), (1, 2));
    assert_eq!(p_left.title_row, Some(1));
    assert_eq!(p_left.title_end_col, Some(2));

    assert_eq!((p_right.start_col, p_right.end_col), (4, 6));
    assert_eq!(p_right.title_row, None);
    assert_eq!(p_right.title_end_col, None);
}
