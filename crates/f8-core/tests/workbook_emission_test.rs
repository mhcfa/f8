#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use std::io::Read;

use f8_core::compiler::emission::{EmissionError, EmissionOptions, FormatCache, WorkbookEmitter};
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::layout::LayoutPlanner;
use f8_core::compiler::lowering::FormulaLowerer;
use f8_core::compiler::{Compiler, ir};
use f8_core::model::format::{SemanticStyle, Shade};
use f8_core::model::id::{SheetId, TableId};
use f8_core::model::manifest::{FreezePanes, ProjectManifest, SheetSpec};
use f8_core::model::table::{
    CellInput, ColumnSpec, DetailedCellInput, RowSpec, SectionHeaderRowSpec, TableSpec,
    ValueLiteral,
};
use f8_core::model::theme::Theme;

fn read_zip_entry(buffer: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(buffer)).unwrap();
    let mut file = archive
        .by_name(name)
        .unwrap_or_else(|_| panic!("Failed to find zip entry: {name}"));
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    content
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

// ----------------------------------------------------------------------------
// Scenario 1: End-to-End Workbook Binary Generation
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_1_end_to_end_workbook_binary_generation() {
    let mut is_table = TableSpec::new("IncomeStatement", "Income Statement");
    is_table
        .columns
        .push(ColumnSpec::descriptor("line_item", "Line Item"));
    is_table.columns.push(ColumnSpec::new("2024E"));
    is_table.rows.push(RowSpec::new("revenue"));
    populate_table_cells(&mut is_table);

    let mut sum_table = TableSpec::new("SummaryTable", "Executive Summary");
    sum_table
        .columns
        .push(ColumnSpec::descriptor("metric", "Metric"));
    sum_table.columns.push(ColumnSpec::new("value"));
    sum_table.rows.push(RowSpec::new("total_rev"));
    populate_table_cells(&mut sum_table);

    let manifest = ProjectManifest {
        model_name: "MultiSheetModel".to_string(),
        theme: Default::default(),
        sheets: vec![
            SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("IncomeStatement")],
            },
            SheetSpec {
                name: "Summary".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![TableId::from("SummaryTable")],
            },
        ],
    };

    let options = EmissionOptions {
        title: Some("Corporate Financial Model".to_string()),
        author: Some("Acme Corp".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let buffer = Compiler::emit_workbook(&manifest, &[is_table, sum_table], &options).unwrap();

    // 1. Non-empty byte buffer
    assert!(!buffer.is_empty());
    // 2. Valid Zip magic bytes (PK\x03\x04)
    assert_eq!(&buffer[0..4], &[0x50, 0x4B, 0x03, 0x04]);

    // 3. Valid OpenXML archive with declared worksheets
    let workbook_xml = read_zip_entry(&buffer, "xl/workbook.xml");
    assert!(workbook_xml.contains("name=\"Model\""));
    assert!(workbook_xml.contains("name=\"Summary\""));

    // 4. Document metadata properties
    let core_xml = read_zip_entry(&buffer, "docProps/core.xml");
    assert!(core_xml.contains("<dc:creator>Acme Corp</dc:creator>"));
    assert!(core_xml.contains("<dc:title>Corporate Financial Model</dc:title>"));
}

// ----------------------------------------------------------------------------
// Scenario 2: Sheet Layout Metrics and Column Widths Application
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_2_sheet_layout_metrics_and_column_widths_application() {
    let mut table = TableSpec::new("LayoutTable", "Layout Showcase");
    table
        .columns
        .push(ColumnSpec::descriptor("item", "Descriptor Column"));
    table.columns.push(ColumnSpec::new("col_long_header_title"));
    table.rows.push(RowSpec::new("row_1"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "LayoutModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "MetricsSheet".to_string(),
            tab_color: Some("#1B365D".to_string()),
            freeze_panes: Some(FreezePanes { row: 3, col: 2 }),
            show_gridlines: false,
            tables: vec![TableId::from("LayoutTable")],
        }],
    };

    let options = EmissionOptions::default();

    let buffer = Compiler::emit_workbook(&manifest, &[table], &options).unwrap();

    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");

    // Tab accent color
    assert!(sheet_xml.contains("tabColor") && sheet_xml.contains("1B365D"));

    // Gridline visibility disabled
    assert!(sheet_xml.contains("showGridLines=\"0\""));

    // Freeze panes configured at row 3, col 2
    assert!(
        sheet_xml.contains("pane")
            && sheet_xml.contains("ySplit=\"3\"")
            && sheet_xml.contains("xSplit=\"2\"")
    );

    // Sheet-wide column widths applied
    assert!(sheet_xml.contains("<cols>") && sheet_xml.contains("<col min=\""));
}

// ----------------------------------------------------------------------------
// Scenario 3: Structural Headers and Super-Headers Emission
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_3_structural_headers_and_super_headers_emission() {
    let mut table = TableSpec::new("HeaderTable", "Table Title Banner")
        .with_subtitle("($ in thousands, except per share data)");
    table
        .columns
        .push(ColumnSpec::descriptor("line_item", "Line Item"));
    table.columns.push(
        ColumnSpec::new("2024E")
            .with_group("Forecast Horizon")
            .with_header("2024E"),
    );
    table.columns.push(
        ColumnSpec::new("2025E")
            .with_group("Forecast Horizon")
            .with_header("2025E"),
    );
    table.columns.push(
        ColumnSpec::new("2026E")
            .with_group("Forecast Horizon")
            .with_header("2026E"),
    );

    table.rows.push(RowSpec::new("revenue"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "HeaderModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Headers".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("HeaderTable")],
        }],
    };

    let buffer = Compiler::emit_workbook(&manifest, &[table], &EmissionOptions::default()).unwrap();

    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
    let shared_strings = read_zip_entry(&buffer, "xl/sharedStrings.xml");

    // Title banner text
    assert!(shared_strings.contains("Table Title Banner"));

    // Subtitle text
    assert!(shared_strings.contains("($ in thousands, except per share data)"));

    // Super-header Center Across Selection: label is present, zero merged cells (FAST standard)
    assert!(shared_strings.contains("Forecast Horizon"));
    assert!(
        !sheet_xml.contains("<mergeCell"),
        "No merged cells allowed in financial models"
    );
    assert!(sheet_xml.contains("r=\"C5\""));
    assert!(sheet_xml.contains("r=\"D5\""));
    assert!(sheet_xml.contains("r=\"E5\""));

    let styles_xml = read_zip_entry(&buffer, "xl/styles.xml");
    assert!(styles_xml.contains("horizontal=\"centerContinuous\""));

    // Column key headers
    assert!(shared_strings.contains("Line Item"));
    assert!(shared_strings.contains("2024E"));
    assert!(shared_strings.contains("2025E"));
    assert!(shared_strings.contains("2026E"));
}

#[test]
fn test_scenario_3b_multi_row_subtitles_emission_and_inspection() {
    use f8_core::compiler::SheetCellKind;
    use f8_core::value::CellValue;

    let mut table = TableSpec::new("Assumptions", "Assumptions")
        .with_subtitles(["($ in Millions)", "Audited FY22-FY24; Projected FY25-FY28"]);
    table
        .columns
        .push(ColumnSpec::descriptor("param", "Parameter"));
    table
        .columns
        .push(ColumnSpec::new("val").with_header("Value"));
    table.rows.push(RowSpec::new("tax_rate"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "MultiSubtitleEmission".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Summary".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("Assumptions")],
        }],
    };

    let compiled = Compiler::compile(&manifest, std::slice::from_ref(&table)).unwrap();
    let placement = compiled
        .layout_map
        .get_table(&TableId::from("Assumptions"))
        .unwrap();
    assert_eq!(placement.title_row, Some(1));
    assert_eq!(placement.subtitle_row_range, Some((2, 3)));
    assert_eq!(placement.title_gutter_row, Some(4));
    assert_eq!(placement.header_row_range, Some((5, 5)));

    let table_insp = compiled
        .inspect_table(&TableId::from("Assumptions"))
        .unwrap();
    assert_eq!(
        table_insp.subtitles,
        vec![
            "($ in Millions)".to_string(),
            "Audited FY22-FY24; Projected FY25-FY28".to_string()
        ]
    );

    let sheet_insp = compiled.inspect_sheet(&SheetId::from("Summary")).unwrap();
    // Row 0 in sheet_insp.rows is physical_row 1 (Title), Row 1 is physical_row 2 (Subtitle 1), Row 2 is physical_row 3 (Subtitle 2)
    let sub_row_1 = &sheet_insp.rows[1];
    assert_eq!(sub_row_1.physical_row, 2);
    assert_eq!(sub_row_1.excel_row, 3);
    assert_eq!(sub_row_1.cells[0].physical_a1, "B3");
    assert_eq!(sub_row_1.cells[0].kind, SheetCellKind::TableSubtitle);
    assert_eq!(
        sub_row_1.cells[0].value,
        CellValue::Text("($ in Millions)".to_string())
    );
    assert_eq!(sub_row_1.cells[1].physical_a1, "C3");
    assert_eq!(sub_row_1.cells[1].kind, SheetCellKind::TableSubtitle);
    assert_eq!(sub_row_1.cells[1].value, CellValue::Empty);

    let sub_row_2 = &sheet_insp.rows[2];
    assert_eq!(sub_row_2.physical_row, 3);
    assert_eq!(sub_row_2.excel_row, 4);
    assert_eq!(sub_row_2.cells[0].physical_a1, "B4");
    assert_eq!(sub_row_2.cells[0].kind, SheetCellKind::TableSubtitle);
    assert_eq!(
        sub_row_2.cells[0].value,
        CellValue::Text("Audited FY22-FY24; Projected FY25-FY28".to_string())
    );
    assert_eq!(sub_row_2.cells[1].physical_a1, "C4");
    assert_eq!(sub_row_2.cells[1].kind, SheetCellKind::TableSubtitle);
    assert_eq!(sub_row_2.cells[1].value, CellValue::Empty);

    let buffer = compiled.emit_workbook(&EmissionOptions::default()).unwrap();
    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
    let shared_strings = read_zip_entry(&buffer, "xl/sharedStrings.xml");
    assert!(shared_strings.contains("($ in Millions)"));
    assert!(shared_strings.contains("Audited FY22-FY24; Projected FY25-FY28"));
    assert!(sheet_xml.contains("r=\"B3\""));
    assert!(sheet_xml.contains("r=\"B4\""));
    assert!(!sheet_xml.contains("r=\"B5\"")); // Blank title gutter row at physical row 4 (Excel row 5)
    assert!(sheet_xml.contains("r=\"B6\"")); // Column header at physical row 5 (Excel row 6)
}

// ----------------------------------------------------------------------------
// Scenario 4: Single-Cell Super-Header Safety
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_4_single_cell_super_header_safety() {
    let mut table = TableSpec::new("SingleGroupTable", "Single Group Table");
    table
        .columns
        .push(ColumnSpec::descriptor("line_item", "Line Item"));
    table
        .columns
        .push(ColumnSpec::new("2023A").with_header("2023A"));
    table.columns.push(
        ColumnSpec::new("2024E")
            .with_group("Single Column Tier")
            .with_header("2024E"),
    );

    table.rows.push(RowSpec::new("revenue"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "SingleGroupModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "SingleGroup".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("SingleGroupTable")],
        }],
    };

    // Must compile without error (specifically avoiding XlsxError::MergeRangeSingleCell)
    let buffer = Compiler::emit_workbook(&manifest, &[table], &EmissionOptions::default()).unwrap();

    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
    let shared_strings = read_zip_entry(&buffer, "xl/sharedStrings.xml");

    // Single-cell super-header label is present
    assert!(shared_strings.contains("Single Column Tier"));

    // Guard ensures no invalid 1x1 merge range is emitted
    assert!(!sheet_xml.contains("<mergeCell ref=\"D3:D3\"/>"));
    assert!(!sheet_xml.contains("<mergeCell ref=\"C3:C3\"/>"));
}

// ----------------------------------------------------------------------------
// Scenario 5: Live Formulas with Evaluated Result Caching
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_5_live_formulas_with_evaluated_result_caching() {
    let mut table = TableSpec::new("FormulaTable", "Formulas");
    table
        .columns
        .push(ColumnSpec::descriptor("line_item", "Line Item"));
    table.columns.push(ColumnSpec::new("val"));

    let row_rev = RowSpec::new("rev")
        .with_cell("val", 500.0)
        .with_cell("line_item", "Revenue");

    let row_gp = RowSpec::new("gp")
        .with_cell("val", CellInput::formula("FormulaTable[\"rev\", @col] * 2"))
        .with_cell("line_item", "Gross Profit");

    table.rows.push(row_rev);
    table.rows.push(row_gp);

    let manifest = ProjectManifest {
        model_name: "FormulaModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("FormulaTable")],
        }],
    };

    let buffer = Compiler::emit_workbook(
        &manifest,
        std::slice::from_ref(&table),
        &EmissionOptions::default(),
    )
    .unwrap();

    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");

    // 1. Live formula expression is emitted
    assert!(sheet_xml.contains("<f>") && sheet_xml.contains("*2</f>"));

    // 2. Precalculated evaluation result (500 * 2 = 1000) is pre-populated in cell cache <v>
    assert!(sheet_xml.contains("<v>1000</v>"));
}

// ----------------------------------------------------------------------------
// Scenario 6: Values-Only Freezing Mode
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_6_values_only_freezing_mode() {
    let mut table = TableSpec::new("FreezeTable", "Audit Freeze");
    table
        .columns
        .push(ColumnSpec::descriptor("line_item", "Line Item"));
    table.columns.push(ColumnSpec::new("val"));

    let row_rev = RowSpec::new("rev")
        .with_cell("val", 250.0)
        .with_cell("line_item", "Revenue");

    let row_margin = RowSpec::new("double_rev")
        .with_cell("val", CellInput::formula("FreezeTable[\"rev\", @col] * 2"))
        .with_cell("line_item", "Double Revenue");

    table.rows.push(row_rev);
    table.rows.push(row_margin);

    let manifest = ProjectManifest {
        model_name: "FreezeModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "FreezeSheet".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("FreezeTable")],
        }],
    };

    // 1. Test via Compiler::emit_workbook
    let options = EmissionOptions {
        values_only: true,
        omit_formula_cache: false,
        title: None,
        author: None,
    };
    let buffer =
        Compiler::emit_workbook(&manifest, std::slice::from_ref(&table), &options).unwrap();

    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");

    // In values-only mode, NO formula definitions (<f>) should exist in the sheet
    assert!(!sheet_xml.contains("<f>"));

    // Concrete frozen numbers (250 and 500) are emitted directly as <v>
    assert!(sheet_xml.contains("<v>250</v>"));
    assert!(sheet_xml.contains("<v>500</v>"));

    // 2. Test via CompiledModel::emit_workbook
    let compiled = Compiler::compile(&manifest, std::slice::from_ref(&table)).unwrap();
    let buffer2 = compiled.emit_workbook(&options).unwrap();
    assert_eq!(buffer, buffer2);

    let sheet2_xml = read_zip_entry(&buffer2, "xl/worksheets/sheet1.xml");
    assert!(!sheet2_xml.contains("<f>"));
    assert!(sheet2_xml.contains("<v>500</v>"));
}

// ----------------------------------------------------------------------------
// Scenario 7: Input Blue vs Formula Black Color Coding
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_7_input_blue_vs_formula_black_color_coding() {
    let theme = Theme::corporate_navy();
    let mut format_cache = FormatCache::new(&theme).unwrap();

    let role = f8_core::compiler::layout::StyleRole::data();

    // Measure input cell (is_input: true, is_descriptor: false)
    let input_key =
        f8_core::compiler::emission::CellFormatKey::new(role, None, None, None, true, false, 0);
    let _input_fmt = format_cache.get_cell_format(&input_key).unwrap();

    // Formula cell (is_input: false, is_descriptor: false)
    let formula_key =
        f8_core::compiler::emission::CellFormatKey::new(role, None, None, None, false, false, 0);
    let _formula_fmt = format_cache.get_cell_format(&formula_key).unwrap();

    // Descriptor cell (is_descriptor: true)
    let desc_key =
        f8_core::compiler::emission::CellFormatKey::new(role, None, None, None, false, true, 0);
    let _desc_fmt = format_cache.get_cell_format(&desc_key).unwrap();

    // Verify format cache distinguishes input vs formula styles
    assert_eq!(format_cache.len(), 3);
}

// ----------------------------------------------------------------------------
// Scenario 8: Collapsible Row Group Outlining
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_8_collapsible_row_group_outlining() {
    let mut table = TableSpec::new("OutlineTable", "Operating Expenses");
    table
        .columns
        .push(ColumnSpec::descriptor("line_item", "Expense Item"));
    table
        .columns
        .push(ColumnSpec::new("amount").with_header("Amount"));

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
    table.rows.push(
        SectionHeaderRowSpec::new("Marketing & Growth")
            .with_indent(Some(1))
            .into(),
    );
    table.rows.push(RowSpec::new("digital_ads").with_cell(
        "line_item",
        DetailedCellInput {
            value: Some(ValueLiteral::String("digital_ads".into())),
            indent: Some(2),
            ..Default::default()
        },
    ));
    table.rows.push(RowSpec::new("events").with_cell(
        "line_item",
        DetailedCellInput {
            value: Some(ValueLiteral::String("events".into())),
            indent: Some(2),
            ..Default::default()
        },
    ));
    table.rows.push(RowSpec::new("rent").with_cell(
        "line_item",
        DetailedCellInput {
            value: Some(ValueLiteral::String("rent".into())),
            indent: Some(1),
            ..Default::default()
        },
    ));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "OutlineModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Outlines".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("OutlineTable")],
        }],
    };

    let buffer =
        Compiler::emit_workbook(&manifest, &[table.clone()], &EmissionOptions::default()).unwrap();

    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
    let shared_strings = read_zip_entry(&buffer, "xl/sharedStrings.xml");
    let styles_xml = read_zip_entry(&buffer, "xl/styles.xml");

    // Interleaved section headers are written
    assert!(shared_strings.contains("Operating Expenses"));
    assert!(shared_strings.contains("Marketing &amp; Growth"));

    // Default table has collapsible: false -> no outline brackets on rows
    assert!(!sheet_xml.contains("outlineLevel="));

    // OpenXML styles define indentation levels for descriptor cells and sub-headers
    assert!(styles_xml.contains("indent=\"1\""));
    assert!(styles_xml.contains("indent=\"2\""));

    // Now emit with opt-in table-level collapsing enabled
    let mut collapsible_table = table;
    collapsible_table.collapsible = true;
    let collapsible_buffer =
        Compiler::emit_workbook(&manifest, &[collapsible_table], &EmissionOptions::default())
            .unwrap();

    let coll_sheet_xml = read_zip_entry(&collapsible_buffer, "xl/worksheets/sheet1.xml");
    // Table body rows are grouped into level 1 under the title banner
    assert!(coll_sheet_xml.contains("outlineLevel=\"1\""));
    // Outline symbols are positioned above the range (summaryBelow="0")
    assert!(coll_sheet_xml.contains("summaryBelow=\"0\""));
}

// ----------------------------------------------------------------------------
// Scenario 9: Save Workbook Directly to Disk File
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_9_save_workbook_directly_to_disk_file() {
    let mut table = TableSpec::new("DiskTable", "Disk Export");
    table.columns.push(ColumnSpec::new("val"));
    table.rows.push(RowSpec::new("r1"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "DiskModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("DiskTable")],
        }],
    };

    let temp_dir = std::env::temp_dir();
    let target_file = temp_dir.join(format!("f8_test_{}.xlsx", std::process::id()));

    // Clean up if left over
    if target_file.exists() {
        let _ = std::fs::remove_file(&target_file);
    }

    let buffer = Compiler::emit_workbook(&manifest, &[table], &EmissionOptions::default())
        .expect("failed to emit workbook");

    std::fs::write(&target_file, &buffer).expect("failed to write workbook to disk");
    assert!(target_file.exists());

    let file_bytes = std::fs::read(&target_file).unwrap();
    assert!(!file_bytes.is_empty());
    assert_eq!(&file_bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);

    // Inspect zip contents from file
    let workbook_xml = read_zip_entry(&file_bytes, "xl/workbook.xml");
    assert!(workbook_xml.contains("name=\"Sheet1\""));

    // Cleanup
    let _ = std::fs::remove_file(&target_file);
}

// ----------------------------------------------------------------------------
// Scenario 10: Missing Coordinate or Entity Diagnostics
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_10_missing_coordinate_or_entity_diagnostics() {
    let mut table = TableSpec::new("DiagTable", "Diagnostics");
    table.columns.push(ColumnSpec::new("c1"));
    table.rows.push(RowSpec::new("r1"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "DiagModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "ModelSheet".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("DiagTable")],
        }],
    };

    let theme = Theme::corporate_navy();
    let (_sym, grid) = ir::expand_workbook(&manifest, &[table.clone()]).unwrap();
    let mut layout_map =
        LayoutPlanner::plan(&manifest, &[table.clone()], &grid, &theme, None).unwrap();
    let lowered_formulas = FormulaLowerer::lower_grid(&grid, &_sym, &layout_map).unwrap();

    // 1. Missing cell coordinate returns MissingCell
    let removed_cell_id = CellId::new("DiagTable", "r1", "c1");
    layout_map.cells.shift_remove(&removed_cell_id);

    let emit_res = WorkbookEmitter::emit_to_buffer(
        &manifest,
        &[table.clone()],
        &grid,
        &layout_map,
        &lowered_formulas,
        &theme,
        None,
        &EmissionOptions::default(),
    );

    assert!(matches!(
        emit_res,
        Err(EmissionError::MissingCell { cell }) if cell == removed_cell_id
    ));

    // 2. Missing table placement returns MissingTable
    let mut layout_map2 =
        LayoutPlanner::plan(&manifest, &[table.clone()], &grid, &theme, None).unwrap();
    layout_map2.tables.shift_remove(&TableId::from("DiagTable"));

    let emit_res2 = WorkbookEmitter::emit_to_buffer(
        &manifest,
        &[table.clone()],
        &grid,
        &layout_map2,
        &lowered_formulas,
        &theme,
        None,
        &EmissionOptions::default(),
    );

    assert!(matches!(
        emit_res2,
        Err(EmissionError::MissingTable { table }) if table == TableId::from("DiagTable")
    ));

    // 3. Missing sheet layout returns MissingSheet
    let mut layout_map3 =
        LayoutPlanner::plan(&manifest, &[table.clone()], &grid, &theme, None).unwrap();
    layout_map3
        .sheets
        .shift_remove(&SheetId::from("ModelSheet"));

    let emit_res3 = WorkbookEmitter::emit_to_buffer(
        &manifest,
        std::slice::from_ref(&table),
        &grid,
        &layout_map3,
        &lowered_formulas,
        &theme,
        None,
        &EmissionOptions::default(),
    );

    assert!(matches!(
        emit_res3,
        Err(EmissionError::MissingSheet { sheet }) if sheet == SheetId::from("ModelSheet")
    ));

    // 4. Missing grid cell returns MissingCell
    let full_layout =
        LayoutPlanner::plan(&manifest, std::slice::from_ref(&table), &grid, &theme, None).unwrap();
    let empty_grid = f8_core::compiler::ir::cell::CellGrid::new();
    let emit_res4 = WorkbookEmitter::emit_to_buffer(
        &manifest,
        std::slice::from_ref(&table),
        &empty_grid,
        &full_layout,
        &lowered_formulas,
        &theme,
        None,
        &EmissionOptions::default(),
    );
    assert!(matches!(
        emit_res4,
        Err(EmissionError::MissingCell { cell }) if cell == removed_cell_id
    ));

    // 5. Missing lowered formula or evaluated entry for a formula cell returns EmissionError
    let mut formula_table = TableSpec::new("DiagTable", "Diagnostics");
    formula_table.columns.push(ColumnSpec::new("c1"));
    formula_table
        .rows
        .push(RowSpec::new("r1").with_cell("c1", CellInput::formula("1 + 1")));
    let (formula_sym, formula_grid) =
        ir::expand_workbook(&manifest, std::slice::from_ref(&formula_table)).unwrap();
    let formula_layout = LayoutPlanner::plan(
        &manifest,
        std::slice::from_ref(&formula_table),
        &formula_grid,
        &theme,
        None,
    )
    .unwrap();
    let formula_lowered =
        FormulaLowerer::lower_grid(&formula_grid, &formula_sym, &formula_layout).unwrap();
    let empty_lowered = f8_core::compiler::lowering::LoweredFormulaMap::new();
    let empty_eval = f8_core::compiler::eval::EvaluatedModel::new(
        indexmap::IndexMap::new(),
        formula_sym.clone(),
    );

    // Missing lowered formula in live mode
    let emit_res5 = WorkbookEmitter::emit_to_buffer(
        &manifest,
        std::slice::from_ref(&formula_table),
        &formula_grid,
        &formula_layout,
        &empty_lowered,
        &theme,
        None,
        &EmissionOptions::default(),
    );
    assert!(matches!(
        emit_res5,
        Err(EmissionError::MissingFormula { cell }) if cell == removed_cell_id
    ));

    // Missing evaluated cell in live cached mode when evaluated model is supplied
    let emit_res6 = WorkbookEmitter::emit_to_buffer(
        &manifest,
        std::slice::from_ref(&formula_table),
        &formula_grid,
        &formula_layout,
        &formula_lowered,
        &theme,
        Some(&empty_eval),
        &EmissionOptions::default(),
    );
    assert!(matches!(
        emit_res6,
        Err(EmissionError::MissingEvaluatedCell { cell }) if cell == removed_cell_id
    ));

    // Missing evaluated cell in values_only mode
    let emit_res7 = WorkbookEmitter::emit_to_buffer(
        &manifest,
        std::slice::from_ref(&formula_table),
        &formula_grid,
        &formula_layout,
        &formula_lowered,
        &theme,
        Some(&empty_eval),
        &EmissionOptions::default().with_values_only(true),
    );
    assert!(matches!(
        emit_res7,
        Err(EmissionError::MissingEvaluatedCell { cell }) if cell == removed_cell_id
    ));
}

// ----------------------------------------------------------------------------
// Scenario 11: Audit Note Attachment to Cells
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_11_audit_note_attachment() {
    let mut table = TableSpec::new("NoteTable", "Audit Note Showcase");
    table.columns.push(ColumnSpec::new("c1"));
    let r1 = RowSpec::new("r1").with_cell(
        "c1",
        CellInput::Detailed(DetailedCellInput {
            value: Some(ValueLiteral::Number(100.0)),
            note: Some("Audit checked by Senior Associate".to_string()),
            ..Default::default()
        }),
    );
    table.rows.push(r1);

    let manifest = ProjectManifest {
        model_name: "NoteModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "AuditSheet".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("NoteTable")],
        }],
    };

    let buffer = Compiler::emit_workbook(&manifest, &[table], &EmissionOptions::default()).unwrap();

    let comments_xml = read_zip_entry(&buffer, "xl/comments1.xml");
    assert!(comments_xml.contains("Audit checked by Senior Associate"));
}

// ----------------------------------------------------------------------------
// Scenario 12: Audit Note 0D Isolation and Section Header Notes
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_12_audit_note_isolation_and_section_header() {
    let mut table = TableSpec::new("NoteTable", "Audit Note Showcase");
    table
        .columns
        .push(ColumnSpec::descriptor("desc", "Descriptor"));
    table.columns.push(ColumnSpec::new("val"));

    table.rows.push(
        SectionHeaderRowSpec::new("Executive Summary")
            .with_note("Section level annotation")
            .into(),
    );

    let r1 = RowSpec::new("r1")
        .with_cell(
            "desc",
            CellInput::Detailed(DetailedCellInput {
                value: Some(ValueLiteral::String("Revenue Line Item".to_string())),
                note: Some("Line item audit sign-off".to_string()),
                ..Default::default()
            }),
        )
        .with_cell("val", CellInput::number(500.0));
    table.rows.push(r1);

    let manifest = ProjectManifest {
        model_name: "NoteModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "AuditSheet".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("NoteTable")],
        }],
    };

    let buffer = Compiler::emit_workbook(&manifest, &[table], &EmissionOptions::default()).unwrap();

    let comments_xml = read_zip_entry(&buffer, "xl/comments1.xml");
    assert!(comments_xml.contains("Section level annotation"));
    assert!(comments_xml.contains("Line item audit sign-off"));
    // Exactly 2 comments should be emitted
    let comment_count = comments_xml.matches("<comment ").count();
    assert_eq!(comment_count, 2);
}

#[test]
fn test_scenario_13_section_header_underline_workbook_emission() {
    let mut table = TableSpec::new("SectionTable", "Section Underline Showcase");
    table
        .columns
        .push(ColumnSpec::descriptor("desc", "Descriptor"));
    table.columns.push(ColumnSpec::new("val"));

    table
        .rows
        .push(SectionHeaderRowSpec::new("Operating Activities").into());

    let r1 = RowSpec::new("r1")
        .with_cell("desc", CellInput::string("Revenue"))
        .with_cell("val", CellInput::number(100.0));
    table.rows.push(r1);

    let manifest = ProjectManifest {
        model_name: "UnderlineModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("SectionTable")],
        }],
    };

    let buffer = Compiler::emit_workbook(&manifest, &[table], &EmissionOptions::default()).unwrap();

    let styles_xml = read_zip_entry(&buffer, "xl/styles.xml");
    assert!(
        styles_xml.contains("<u/>") || styles_xml.contains("<u val=\"single\"/>"),
        "styles.xml must contain font underline definition: {styles_xml}"
    );

    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains("r=\"B5\""),
        "Section header title must be written at B5: {sheet_xml}"
    );
    assert!(
        !sheet_xml.contains("r=\"C5\""),
        "Trailing measure column C5 must remain unwritten when section_header_fill is 'none': {sheet_xml}"
    );
}

#[test]
fn test_scenario_14_lbo_timeline_group_banners_and_stacked_headers_no_center_across_bleed() {
    let mut table = TableSpec::new("LboSchedule", "LBO Schedule");
    table
        .columns
        .push(ColumnSpec::descriptor("label", "($ in millions)"));
    table.columns.push(
        ColumnSpec::new("mrq")
            .with_group("Historical")
            .with_headers(["MRQ", "2012-03-31"]),
    );
    table.columns.push(
        ColumnSpec::new("mrq_plus_1")
            .with_group("Quarterly Rollforward")
            .with_headers(["MRQ+1", "2012-06-30"]),
    );
    table.columns.push(
        ColumnSpec::new("mrq_plus_2")
            .with_group("Quarterly Rollforward")
            .with_headers(["MRQ+2", "2012-09-30"]),
    );
    table.columns.push(
        ColumnSpec::new("mrq_plus_3")
            .with_group("Quarterly Rollforward")
            .with_headers(["MRQ+3", "2012-12-31"]),
    );
    table.columns.push(
        ColumnSpec::new("mrq_plus_4")
            .with_group("Quarterly Rollforward")
            .with_headers(["MRQ+4", "2013-03-31"]),
    );
    table
        .columns
        .push(ColumnSpec::new("standalone_close").with_headers([
            "Standalone",
            "at Close",
            "2012-12-31",
        ]));
    table
        .columns
        .push(ColumnSpec::new("deal_adjustments").with_headers(["Deal", "Adjustments"]));
    table
        .columns
        .push(ColumnSpec::new("pf_close").with_headers(["Pro Forma", "at Close", "2012-12-31"]));
    table
        .columns
        .push(ColumnSpec::new("pf_close_adj").with_headers(["Adjusted", "at Close", "2012-12-31"]));
    table
        .columns
        .push(ColumnSpec::new("stub_3q2013").with_headers(["3 Quarters", "Ending", "2013-09-30"]));
    table.columns.push(
        ColumnSpec::new("fy2014")
            .with_group("Annual Projections")
            .with_headers(["Year 2", "2014"]),
    );
    table.columns.push(
        ColumnSpec::new("fy2015")
            .with_group("Annual Projections")
            .with_headers(["Year 3", "2015"]),
    );
    table.columns.push(ColumnSpec::new("unlabeled"));

    table.rows.push(RowSpec::new("revenue"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "LboHeaderModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "LBO".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("LboSchedule")],
        }],
    };

    let buffer =
        Compiler::emit_workbook(&manifest, &[table.clone()], &EmissionOptions::default()).unwrap();
    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
    let shared_strings = read_zip_entry(&buffer, "xl/sharedStrings.xml");
    let styles_xml = read_zip_entry(&buffer, "xl/styles.xml");

    assert!(!sheet_xml.contains("<mergeCell"));
    assert!(styles_xml.contains("horizontal=\"centerContinuous\""));

    // Extract style attribute s="..." for cell C4 (1-col group "Historical" using super_header_format)
    let extract_cell_tag = |xml: &str, cell_ref: &str| -> Option<String> {
        let needle = format!("r=\"{cell_ref}\"");
        let start = xml.find(&needle)?;
        let tag_start = xml[..start].rfind("<c ")?;
        let tag_end = xml[start..].find('>')? + start + 1;
        Some(xml[tag_start..tag_end].to_string())
    };

    // Row 4 is Tier 0 (Title at Row 2, Title Gutter at Row 3)
    // C4 ("Historical") and D4..G4 ("Quarterly Rollforward") use super_header_format
    let c4_tag = extract_cell_tag(&sheet_xml, "C4").expect("C4 must be written");
    let d4_tag = extract_cell_tag(&sheet_xml, "D4").expect("D4 must be written");
    let e4_tag = extract_cell_tag(&sheet_xml, "E4").expect("E4 must be written as blank in span");
    assert!(c4_tag.contains("t=\"s\""));
    assert!(d4_tag.contains("t=\"s\""));
    assert!(!e4_tag.contains("t=\"s\""));

    // H4 ("Standalone") is an ungrouped 3-line column header -> uses col_header_upper_format, NOT super_header_format
    let h4_tag = extract_cell_tag(&sheet_xml, "H4").expect("H4 must be written");
    assert!(h4_tag.contains("t=\"s\""));
    // Extract style id s="..." from c4_tag and h4_tag and verify they differ
    let get_style_attr = |tag: &str| -> String {
        let s_idx = tag.find("s=\"").unwrap();
        let rest = &tag[s_idx + 3..];
        let end = rest.find('"').unwrap();
        rest[..end].to_string()
    };
    let super_style_id = get_style_attr(&c4_tag);
    let upper_col_style_id = get_style_attr(&h4_tag);
    assert_ne!(
        super_style_id, upper_col_style_id,
        "Ungrouped Tier 0 header H4 must not use super_header_format (CenterAcross)"
    );
    assert_eq!(get_style_attr(&d4_tag), super_style_id);
    assert_eq!(get_style_attr(&e4_tag), super_style_id);

    // I4 (deal_adjustments on Tier 0) must be completely absent when col_header_fill is "none",
    // preventing Standalone (H4) from bleeding across I4even if H4 had CenterAcross
    assert!(
        extract_cell_tag(&sheet_xml, "I4").is_none(),
        "I4 must not be emitted when ungrouped and col_header_fill is transparent"
    );

    // Row 5 is Tier 1: J5 ("at Close" in pf_close) and K5 ("at Close" in pf_close_adj)
    // Both must be emitted as string cells (t="s") with upper_col_style_id, never merged or blanked via CenterAcross!
    let j5_tag = extract_cell_tag(&sheet_xml, "J5").expect("J5 must be written");
    let k5_tag = extract_cell_tag(&sheet_xml, "K5").expect("K5 must be written");
    assert!(j5_tag.contains("t=\"s\""));
    assert!(k5_tag.contains("t=\"s\""));
    assert_eq!(get_style_attr(&j5_tag), upper_col_style_id);
    assert_eq!(get_style_attr(&k5_tag), upper_col_style_id);

    // Row 6 is Tier 2 (bottom tier): O6 ("unlabeled" with 0 headers) is written as blank with bottom col_header_format
    let o6_tag =
        extract_cell_tag(&sheet_xml, "O6").expect("O6 must be written for continuous underline");
    let h6_tag = extract_cell_tag(&sheet_xml, "H6").expect("H6 must be written");
    let bottom_col_style_id = get_style_attr(&h6_tag);
    assert_ne!(bottom_col_style_id, upper_col_style_id);
    assert_eq!(get_style_attr(&o6_tag), bottom_col_style_id);

    // Verify all labels exist in sharedStrings.xml
    for expected in [
        "Historical",
        "Quarterly Rollforward",
        "Annual Projections",
        "Standalone",
        "at Close",
        "Deal",
        "Adjustments",
    ] {
        assert!(
            shared_strings.contains(expected),
            "Missing {expected} in sharedStrings.xml"
        );
    }

    // Also verify custom theme with solid col_header_fill writes blank with upper_col_style_id on I4
    let mut custom_theme = Theme::corporate_navy();
    custom_theme.palette.col_header_fill = "#1B365D".to_string();
    let custom_manifest = ProjectManifest {
        model_name: "LboHeaderSolidFill".to_string(),
        theme: f8_core::model::theme::ThemeSpec::Custom(Box::new(custom_theme)),
        sheets: manifest.sheets,
    };
    let solid_buffer =
        Compiler::emit_workbook(&custom_manifest, &[table], &EmissionOptions::default()).unwrap();
    let solid_sheet_xml = read_zip_entry(&solid_buffer, "xl/worksheets/sheet1.xml");
    let solid_i4_tag = extract_cell_tag(&solid_sheet_xml, "I4")
        .expect("I4 must be written as styled blank when col_header_fill is solid");
    let solid_h4_tag = extract_cell_tag(&solid_sheet_xml, "H4").unwrap();
    assert_eq!(get_style_attr(&solid_i4_tag), get_style_attr(&solid_h4_tag));
}

// ----------------------------------------------------------------------------
// Scenario 15: Column Gutter Row Formatting and Header Border Continuity
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_15_column_gutter_row_formatting_and_header_border_continuity() {
    let mut table = TableSpec::new("GutterFormatting", "Gutter Test");
    table
        .columns
        .push(ColumnSpec::descriptor("line_item", "Line Item"));
    table.columns.push(ColumnSpec::gutter());
    table
        .columns
        .push(ColumnSpec::new("c1").with_header("2024"));
    table
        .columns
        .push(ColumnSpec::new("c2").with_header("2025"));

    table.rows.push(RowSpec::new("rev"));
    table
        .rows
        .push(RowSpec::new("total").with_style(Some(SemanticStyle::Total)));
    table
        .rows
        .push(RowSpec::new("subtotal").with_style(Some(SemanticStyle::Subtotal)));
    table
        .rows
        .push(RowSpec::new("shaded").with_shade(Some(Shade::Muted)));

    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "GutterFormattingModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("GutterFormatting")],
        }],
    };

    let buffer = Compiler::emit_workbook(&manifest, &[table], &EmissionOptions::default()).unwrap();
    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
    let styles_xml = read_zip_entry(&buffer, "xl/styles.xml");

    let extract_cell_tag = |xml: &str, cell_ref: &str| -> Option<String> {
        let needle = format!("r=\"{cell_ref}\"");
        let start = xml.find(&needle)?;
        let tag_start = xml[..start].rfind("<c ")?;
        let tag_end = xml[start..].find('>')? + start + 1;
        Some(xml[tag_start..tag_end].to_string())
    };

    let get_style_attr = |tag: &str| -> usize {
        let s_idx = tag.find("s=\"").unwrap();
        let rest = &tag[s_idx + 3..];
        let end = rest.find('"').unwrap();
        rest[..end].parse::<usize>().unwrap()
    };

    // Header bottom tier is row 4:
    // C4 is the column gutter cell on the bottom header tier -> must be emitted with bottom border
    let c4_tag = extract_cell_tag(&sheet_xml, "C4")
        .expect("C4 (gutter on header tier) must be emitted with header bottom border");
    assert!(
        extract_cell_tag(&sheet_xml, "D4").is_some(),
        "D4 must be written"
    );
    assert!(!c4_tag.contains("t=\"s\""), "C4 gutter must be blank");

    // Row 5 is unstyled data row ("rev"):
    // C5 is the column gutter on standard data row -> must be omitted to keep XLSX compact
    assert!(
        extract_cell_tag(&sheet_xml, "C5").is_none(),
        "C5 (gutter on standard data row) must not be emitted"
    );

    // Row 6 is Total row:
    // C6 is the column gutter on Total row -> must be emitted as blank with Total accounting borders
    let c6_tag = extract_cell_tag(&sheet_xml, "C6")
        .expect("C6 (gutter on Total row) must be emitted with accounting borders");
    assert!(!c6_tag.contains("t=\"s\""), "C6 gutter must be blank");

    // Row 7 is Subtotal row:
    // C7 is the column gutter on Subtotal row -> must be emitted as blank with Subtotal accounting borders
    let c7_tag = extract_cell_tag(&sheet_xml, "C7")
        .expect("C7 (gutter on Subtotal row) must be emitted with Subtotal border");
    assert!(!c7_tag.contains("t=\"s\""), "C7 gutter must be blank");

    // Row 8 is Shaded row:
    // C8 is the column gutter on Muted shade row -> must be emitted as blank with background fill
    let c8_tag = extract_cell_tag(&sheet_xml, "C8")
        .expect("C8 (gutter on Shaded row) must be emitted with background shade");
    assert!(!c8_tag.contains("t=\"s\""), "C8 gutter must be blank");

    // Verify borderId on C6 style in styles.xml points to a border with double bottom
    let c6_style_id = get_style_attr(&c6_tag);
    // Find <xf ...> at index c6_style_id within <cellXfs>
    let cell_xfs_start = styles_xml.find("<cellXfs").expect("<cellXfs> not found");
    let cell_xfs_end = styles_xml[cell_xfs_start..].find("</cellXfs>").unwrap() + cell_xfs_start;
    let cell_xfs_xml = &styles_xml[cell_xfs_start..cell_xfs_end];

    let mut target_border_id: Option<usize> = None;
    for (current_idx, xf) in cell_xfs_xml.split("<xf ").skip(1).enumerate() {
        if current_idx == c6_style_id {
            if let Some(b_idx) = xf.find("borderId=\"") {
                let rest = &xf[b_idx + 10..];
                let end = rest.find('"').unwrap();
                target_border_id = Some(rest[..end].parse::<usize>().unwrap());
            }
            break;
        }
    }

    let border_id =
        target_border_id.expect("borderId attribute must exist on Total gutter cell xf");
    // Verify that the border at border_id in <borders> contains double bottom
    let borders_start = styles_xml.find("<borders").expect("<borders> not found");
    let borders_end = styles_xml[borders_start..].find("</borders>").unwrap() + borders_start;
    let borders_xml = &styles_xml[borders_start..borders_end];

    let mut found_double_bottom = false;
    for (b_idx, b) in borders_xml
        .split("<border>")
        .skip(1)
        .chain(borders_xml.split("<border ").skip(1))
        .enumerate()
    {
        if b_idx == border_id {
            found_double_bottom =
                b.contains("<bottom style=\"double\">") || b.contains("style=\"double\"");
            break;
        }
    }
    assert!(
        found_double_bottom,
        "Total gutter cell border must contain double bottom style"
    );
}

// ----------------------------------------------------------------------------
// Scenario 15b: Row Gutter Column Formatting Continuity
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_15b_row_gutter_column_formatting_continuity() {
    use f8_core::model::theme::{BorderStyle, ThemeSpec};

    let extract_cell_element = |xml: &str, cell_ref: &str| -> Option<String> {
        let needle = format!("r=\"{cell_ref}\"");
        let start = xml.find(&needle)?;
        let tag_start = xml[..start].rfind("<c ")?;
        let open_end = xml[tag_start..].find('>')? + tag_start;
        if xml[..open_end].ends_with('/') {
            Some(xml[tag_start..=open_end].to_string())
        } else {
            let close_end = xml[open_end..].find("</c>")? + open_end + "</c>".len();
            Some(xml[tag_start..close_end].to_string())
        }
    };

    let get_attr_usize = |tag: &str, attr: &str| -> Option<usize> {
        let needle = format!("{attr}=\"");
        let idx = tag.find(&needle)?;
        let rest = &tag[idx + needle.len()..];
        let end = rest.find('"')?;
        rest[..end].parse::<usize>().ok()
    };

    let get_xf_xml = |styles_xml: &str, style_idx: usize| -> String {
        let start = styles_xml.find("<cellXfs").expect("<cellXfs> not found");
        let end = styles_xml[start..].find("</cellXfs>").unwrap() + start;
        styles_xml[start..end]
            .split("<xf ")
            .skip(1)
            .nth(style_idx)
            .unwrap_or_else(|| panic!("xf index {style_idx} out of bounds"))
            .to_string()
    };

    let get_border_xml = |styles_xml: &str, border_idx: usize| -> String {
        let start = styles_xml.find("<borders").expect("<borders> not found");
        let end = styles_xml[start..].find("</borders>").unwrap() + start;
        let inner = &styles_xml[start..end];
        let mut cursor = 0;
        let mut borders = Vec::new();
        while let Some(rel) = inner[cursor..].find("<border") {
            let pos = cursor + rel;
            let after = inner.as_bytes().get(pos + 7).copied();
            if matches!(after, Some(b'>' | b' ' | b'/')) {
                let close = inner[pos..].find("</border>").map(|c| pos + c + 9);
                let self_close = inner[pos..].find("/>").map(|c| pos + c + 2);
                let tag_end = match (close, self_close) {
                    (Some(_), Some(sc))
                        if sc < inner[pos..].find('>').map_or(usize::MAX, |g| pos + g + 1) =>
                    {
                        sc
                    }
                    (Some(c), _) => c,
                    (None, Some(sc)) => sc,
                    (None, None) => break,
                };
                borders.push(inner[pos..tag_end].to_string());
                cursor = tag_end;
            } else {
                cursor = pos + 7;
            }
        }
        borders
            .into_iter()
            .nth(border_idx)
            .unwrap_or_else(|| panic!("border index {border_idx} out of bounds"))
    };

    let get_fill_xml = |styles_xml: &str, fill_idx: usize| -> String {
        let start = styles_xml.find("<fills").expect("<fills> not found");
        let end = styles_xml[start..].find("</fills>").unwrap() + start;
        styles_xml[start..end]
            .split("<fill>")
            .skip(1)
            .nth(fill_idx)
            .unwrap_or_else(|| panic!("fill index {fill_idx} out of bounds"))
            .to_string()
    };

    let build_table = || {
        let mut table = TableSpec::new("RowGutterFormatting", "Row Gutter Test");
        // Col B: Descriptor
        table
            .columns
            .push(ColumnSpec::descriptor("line_item", "Line Item"));
        // Col C: Plain data measure
        table
            .columns
            .push(ColumnSpec::new("c_data").with_header("Data"));
        // Col D: Column gutter (intersection with row gutter must stay omitted)
        table.columns.push(ColumnSpec::gutter());
        // Col E: Subtotal column (thin left border)
        table.columns.push(
            ColumnSpec::new("c_sub")
                .with_header("Sub")
                .with_style(Some(SemanticStyle::Subtotal)),
        );
        // Col F: Total column (thin left + double right border)
        table.columns.push(
            ColumnSpec::new("c_tot")
                .with_header("Total")
                .with_style(Some(SemanticStyle::Total)),
        );
        // Col G: Muted shade column (#F8FAFC)
        table.columns.push(
            ColumnSpec::new("c_shade")
                .with_header("Forecast")
                .with_shade(Some(Shade::Muted)),
        );
        // Col H: Highlight column (#FEF3C7)
        table.columns.push(
            ColumnSpec::new("c_hl")
                .with_header("Key")
                .with_highlight(Some(true)),
        );
        // Col I: Explicit default/none styling (must stay omitted)
        table.columns.push(
            ColumnSpec::new("c_none")
                .with_header("None")
                .with_style(Some(SemanticStyle::Data))
                .with_shade(Some(Shade::None))
                .with_highlight(Some(false)),
        );

        // Layout: Title=2, TitleGutter=3, Header=4, r1=5, RowGutter=6, r2=7
        table.rows.push(RowSpec::new("r1"));
        table.rows.push(RowSpec::gutter());
        table.rows.push(RowSpec::new("r2"));
        populate_table_cells(&mut table);
        table
    };

    // Part 1: Default corporate_navy theme
    let table = build_table();
    let manifest = ProjectManifest {
        model_name: "RowGutterDefaultTheme".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("RowGutterFormatting")],
        }],
    };

    let buffer = Compiler::emit_workbook(&manifest, &[table], &EmissionOptions::default()).unwrap();
    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
    let styles_xml = read_zip_entry(&buffer, "xl/styles.xml");

    // Unstyled columns and column-gutter intersection on row 6 must be omitted
    for omitted in ["B6", "C6", "D6", "I6"] {
        assert!(
            extract_cell_element(&sheet_xml, omitted).is_none(),
            "{omitted} on unstyled/gutter column must not be emitted"
        );
    }

    // E6: Subtotal column -> blank with thin left border
    let e6 = extract_cell_element(&sheet_xml, "E6")
        .expect("E6 (row gutter on Subtotal column) must be emitted");
    assert!(!e6.contains("<v>") && !e6.contains("<f>") && !e6.contains("t=\"s\""));
    let e6_xf = get_xf_xml(&styles_xml, get_attr_usize(&e6, "s").unwrap());
    let e6_border = get_border_xml(&styles_xml, get_attr_usize(&e6_xf, "borderId").unwrap());
    assert!(
        e6_border.contains("<left style=\"thin\">"),
        "E6 must preserve col_subtotal_border_left: {e6_border}"
    );

    // F6: Total column -> blank with thin left + double right border
    let f6 = extract_cell_element(&sheet_xml, "F6")
        .expect("F6 (row gutter on Total column) must be emitted");
    assert!(!f6.contains("<v>") && !f6.contains("<f>") && !f6.contains("t=\"s\""));
    let f6_xf = get_xf_xml(&styles_xml, get_attr_usize(&f6, "s").unwrap());
    let f6_border = get_border_xml(&styles_xml, get_attr_usize(&f6_xf, "borderId").unwrap());
    assert!(
        f6_border.contains("<left style=\"thin\">")
            && f6_border.contains("<right style=\"double\">"),
        "F6 must preserve col_total left/right borders: {f6_border}"
    );

    // G6: Shaded column -> blank with shade_muted_fill (#F8FAFC)
    let g6 = extract_cell_element(&sheet_xml, "G6")
        .expect("G6 (row gutter on Muted shade column) must be emitted");
    assert!(!g6.contains("<v>") && !g6.contains("<f>") && !g6.contains("t=\"s\""));
    let g6_xf = get_xf_xml(&styles_xml, get_attr_usize(&g6, "s").unwrap());
    let g6_fill = get_fill_xml(&styles_xml, get_attr_usize(&g6_xf, "fillId").unwrap());
    assert!(
        g6_fill.contains("F8FAFC"),
        "G6 must preserve shade_muted_fill: {g6_fill}"
    );

    // H6: Highlighted column -> blank with highlight_fill (#FEF3C7)
    let h6 = extract_cell_element(&sheet_xml, "H6")
        .expect("H6 (row gutter on highlighted column) must be emitted");
    assert!(!h6.contains("<v>") && !h6.contains("<f>") && !h6.contains("t=\"s\""));
    let h6_xf = get_xf_xml(&styles_xml, get_attr_usize(&h6, "s").unwrap());
    let h6_fill = get_fill_xml(&styles_xml, get_attr_usize(&h6_xf, "fillId").unwrap());
    assert!(
        h6_fill.contains("FEF3C7"),
        "H6 must preserve highlight_fill: {h6_fill}"
    );

    // Part 2: Custom theme with descriptor_border_right enabled
    let mut custom_theme = Theme::corporate_navy();
    custom_theme.borders.descriptor_border_right = BorderStyle::Thin;
    let custom_table = build_table();
    let custom_manifest = ProjectManifest {
        model_name: "RowGutterDescriptorBorderTheme".to_string(),
        theme: ThemeSpec::Custom(Box::new(custom_theme)),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("RowGutterFormatting")],
        }],
    };

    let custom_buf = Compiler::emit_workbook(
        &custom_manifest,
        &[custom_table],
        &EmissionOptions::default(),
    )
    .unwrap();
    let custom_sheet_xml = read_zip_entry(&custom_buf, "xl/worksheets/sheet1.xml");
    let custom_styles_xml = read_zip_entry(&custom_buf, "xl/styles.xml");

    // B6 (descriptor) must now emit blank with thin right border; C6 stays omitted
    let b6 = extract_cell_element(&custom_sheet_xml, "B6").expect(
        "B6 (row gutter on descriptor column with descriptor_border_right) must be emitted",
    );
    assert!(!b6.contains("<v>") && !b6.contains("<f>") && !b6.contains("t=\"s\""));
    let b6_xf = get_xf_xml(&custom_styles_xml, get_attr_usize(&b6, "s").unwrap());
    let b6_border = get_border_xml(
        &custom_styles_xml,
        get_attr_usize(&b6_xf, "borderId").unwrap(),
    );
    assert!(
        b6_border.contains("<right style=\"thin\">"),
        "B6 must preserve descriptor_border_right: {b6_border}"
    );
    assert!(
        extract_cell_element(&custom_sheet_xml, "C6").is_none(),
        "C6 (unstyled measure column) must remain omitted"
    );
}

// ----------------------------------------------------------------------------
// Scenario 16: OpenXML wrapText and VerticalCenter Emission
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_16_wrapped_text_and_vertical_center_emission() {
    let mut table = TableSpec::new("WrapEmission", "Text Wrapping Emission");
    table
        .columns
        .push(ColumnSpec::descriptor("desc", "Description"));
    table
        .columns
        .push(ColumnSpec::new("val").with_header("Value"));

    let long_text = "W".repeat(120);
    table.rows.push(
        RowSpec::new("r_wrapped")
            .with_cell("desc", CellInput::string(long_text))
            .with_cell("val", CellInput::number(42.0)),
    );
    table.rows.push(
        RowSpec::new("r_unwrapped")
            .with_cell("desc", CellInput::string("Short label"))
            .with_cell("val", CellInput::number(99.0)),
    );

    let manifest = ProjectManifest {
        model_name: "WrapEmissionModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("WrapEmission")],
        }],
    };

    let buffer = Compiler::emit_workbook(&manifest, &[table], &EmissionOptions::default()).unwrap();
    let styles_xml = read_zip_entry(&buffer, "xl/styles.xml");
    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");

    // Data cell alignments include vertical="center" and wrapped cell format includes wrapText="1"
    assert!(
        styles_xml.contains("vertical=\"center\""),
        "styles.xml must include vertical=\"center\" on data cells: {styles_xml}"
    );
    assert!(
        styles_xml.contains("wrapText=\"1\""),
        "styles.xml must include wrapText=\"1\" on wrapped data cells: {styles_xml}"
    );

    // Row heights remain automatic in worksheet XML (no customHeight="1")
    assert!(
        !sheet_xml.contains("customHeight=\"1\""),
        "sheet1.xml must leave row heights automatic: {sheet_xml}"
    );
}

// ----------------------------------------------------------------------------
// Scenario 17: EmptyFormulaResult Rejection & Sparse Slice / Null Cell Emission
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_17_empty_formula_rejected_and_sparse_slice_emission() {
    use f8_core::compiler::eval::EvalError;
    use f8_core::compiler::{CompilerError, ErrorDiagnostic};
    use f8_core::model::id::{ColId, RowId};
    use f8_core::value::TypeError;

    let manifest = ProjectManifest {
        model_name: "BlankGuardEmission".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("BlankCoercion")],
        }],
    };

    // 1. Unguarded formula evaluating to Empty fails in Pass 4 prior to Pass 5/6 emission
    let mut unguarded = TableSpec::new("BlankCoercion", "Blank Coercion");
    unguarded
        .columns
        .push(ColumnSpec::new("val").with_header("Value"));
    unguarded
        .rows
        .push(RowSpec::new("a").with_cell("val", CellInput::null()));
    unguarded
        .rows
        .push(RowSpec::new("b").with_cell("val", CellInput::formula("BlankCoercion[\"a\", @col]")));

    let err = Compiler::emit_workbook(&manifest, &[unguarded], &EmissionOptions::default())
        .expect_err("unguarded formula evaluating to Empty must halt before workbook emission");
    assert!(matches!(err.diagnostic(), ErrorDiagnostic::F8Error { .. }));
    assert!(matches!(
        err,
        CompilerError::Eval(EvalError::Type {
            source: TypeError::EmptyFormulaResult,
            ..
        })
    ));

    // 2. Sparse slice SUM(a..a) over Null cell `a` emits cleanly in cached and values_only modes
    let mut table = TableSpec::new("BlankCoercion", "Blank Coercion");
    table
        .columns
        .push(ColumnSpec::new("val").with_header("Value"));
    table
        .rows
        .push(RowSpec::new("a").with_cell("val", CellInput::null()));
    table.rows.push(RowSpec::new("b").with_cell(
        "val",
        CellInput::formula("SUM(BlankCoercion[\"a\"..\"a\", @col])"),
    ));

    let compiled = Compiler::compile(&manifest, std::slice::from_ref(&table)).unwrap();
    let t = TableId::from("BlankCoercion");
    let col = ColId::from("val");

    let a_addr = compiled
        .inspect_cell(&t, &RowId::from("a"), &col)
        .unwrap()
        .physical_address;
    let b_addr = compiled
        .inspect_cell(&t, &RowId::from("b"), &col)
        .unwrap()
        .physical_address;
    let a_ref = a_addr.strip_prefix("'Sheet1'!").unwrap_or(&a_addr);
    let b_ref = b_addr.strip_prefix("'Sheet1'!").unwrap_or(&b_addr);

    let extract_cell_element = |xml: &str, cell_ref: &str| -> Option<String> {
        let needle = format!("r=\"{cell_ref}\"");
        let start = xml.find(&needle)?;
        let tag_start = xml[..start].rfind("<c ")?;
        let open_end = xml[tag_start..].find('>')? + tag_start;
        if xml[..open_end].ends_with('/') {
            Some(xml[tag_start..=open_end].to_string())
        } else {
            let close_end = xml[open_end..].find("</c>")? + open_end + "</c>".len();
            Some(xml[tag_start..close_end].to_string())
        }
    };

    // Cached formula emission: `b` writes `<f>` with `SUM(` and `<v>0</v>` while `a` has no `<v>` tag
    let cached_buf = compiled.emit_workbook(&EmissionOptions::default()).unwrap();
    let cached_xml = read_zip_entry(&cached_buf, "xl/worksheets/sheet1.xml");
    let cached_a = extract_cell_element(&cached_xml, a_ref).expect("a blank cell written");
    let cached_b = extract_cell_element(&cached_xml, b_ref).expect("b formula cell written");

    assert!(
        !cached_a.contains("<v>") && !cached_a.contains("<f>"),
        "null literal cell A must remain blank without <v> or <f>: {cached_a}"
    );
    assert!(
        cached_b.contains("<f>") && cached_b.contains("SUM(") && cached_b.contains("<v>0</v>"),
        "formula cell B must emit SUM <f> and cached <v>0</v>: {cached_b}"
    );

    // values_only emission: `b` writes numeric `0` while `a` remains blank
    let values_only_buf = compiled
        .emit_workbook(&EmissionOptions {
            values_only: true,
            omit_formula_cache: false,
            title: None,
            author: None,
        })
        .unwrap();
    let values_only_xml = read_zip_entry(&values_only_buf, "xl/worksheets/sheet1.xml");
    let frozen_a = extract_cell_element(&values_only_xml, a_ref).expect("a blank cell written");
    let frozen_b = extract_cell_element(&values_only_xml, b_ref).expect("b value cell written");

    assert!(
        !frozen_a.contains("<v>") && !frozen_a.contains("<f>"),
        "null literal cell A must remain blank in values_only mode: {frozen_a}"
    );
    assert!(
        !frozen_b.contains("<f>") && !frozen_b.contains("t=\"s\"") && frozen_b.contains("<v>0</v>"),
        "formula cell B must emit numeric <v>0</v> in values_only mode: {frozen_b}"
    );
}

// ----------------------------------------------------------------------------
// Scenario 18: Negative Zero (-0.0) Literal and Evaluated Value Normalization
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_18_negative_zero_literal_and_evaluated_normalization() {
    let mut table = TableSpec::new("NegZeroTable", "Negative Zero Normalization");
    table
        .columns
        .push(ColumnSpec::new("val").with_header("Value"));
    table
        .rows
        .push(RowSpec::new("lit_neg_zero").with_cell("val", CellInput::number(-0.0)));
    table.rows.push(RowSpec::new("expr_neg_zero").with_cell(
        "val",
        CellInput::formula("-1 * NegZeroTable[\"lit_neg_zero\", @col]"),
    ));

    let manifest = ProjectManifest {
        model_name: "NegZeroModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("NegZeroTable")],
        }],
    };

    let compiled = Compiler::compile(&manifest, std::slice::from_ref(&table)).unwrap();

    // 1. Live formula mode normalizes -0.0 literal to <v>0</v>
    let live_buf = compiled.emit_workbook(&EmissionOptions::default()).unwrap();
    let live_xml = read_zip_entry(&live_buf, "xl/worksheets/sheet1.xml");
    assert!(
        !live_xml.contains("<v>-0</v>"),
        "live worksheet XML must normalize -0.0 to 0: {live_xml}"
    );
    assert!(live_xml.contains("<v>0</v>"));

    // 2. Values-only mode normalizes -0.0 CellValue/ValueLiteral to <v>0</v>
    let mut raw_values = indexmap::IndexMap::new();
    raw_values.insert(
        CellId::new("NegZeroTable", "lit_neg_zero", "val"),
        f8_core::value::CellValue::Number(-0.0),
    );
    raw_values.insert(
        CellId::new("NegZeroTable", "expr_neg_zero", "val"),
        f8_core::value::CellValue::Number(-0.0),
    );
    let raw_eval =
        f8_core::compiler::eval::EvaluatedModel::new(raw_values, compiled.symbol_table.clone());
    let values_buf = WorkbookEmitter::emit_to_buffer(
        &manifest,
        &[table],
        &compiled.grid,
        &compiled.layout_map,
        &compiled.lowered_map,
        &compiled.theme,
        Some(&raw_eval),
        &EmissionOptions::default().with_values_only(true),
    )
    .unwrap();
    let values_xml = read_zip_entry(&values_buf, "xl/worksheets/sheet1.xml");
    assert!(
        !values_xml.contains("<v>-0</v>"),
        "values_only worksheet XML must normalize -0.0 to 0: {values_xml}"
    );
    assert!(values_xml.contains("<v>0</v>"));
}

#[test]
fn test_scenario_19_formula_cache_text_collisions_and_non_finite_guard() {
    let mut table = TableSpec::new("TextCacheTable", "Text Formula Cache");
    table
        .columns
        .push(ColumnSpec::new("val").with_header("Value"));
    table.rows.push(RowSpec::new("r_nan").with_expr("\"NaN\""));
    table.rows.push(RowSpec::new("r_inf").with_expr("\"inf\""));
    table
        .rows
        .push(RowSpec::new("r_num_str").with_expr("\"42\""));
    table
        .rows
        .push(RowSpec::new("r_bool_str").with_expr("\"TRUE\""));
    table
        .rows
        .push(RowSpec::new("r_err_str").with_expr("\"#N/A\""));
    table
        .rows
        .push(RowSpec::new("r_empty_str").with_expr("\"\""));
    table
        .rows
        .push(RowSpec::new("r_plain_str").with_expr("\"Hello\""));

    let manifest = ProjectManifest {
        model_name: "TextCacheModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("TextCacheTable")],
        }],
    };

    let compiled = Compiler::compile(&manifest, std::slice::from_ref(&table)).unwrap();
    let buf = compiled.emit_workbook(&EmissionOptions::default()).unwrap();
    let xml = read_zip_entry(&buf, "xl/worksheets/sheet1.xml");

    assert!(
        !xml.contains("<v>NaN</v>") && !xml.contains("<v>inf</v>"),
        "worksheet XML must never emit invalid numeric <v>NaN</v> or <v>inf</v>: {xml}"
    );
    assert!(
        xml.contains("t=\"str\"><f>\"\"</f><v></v></c>"),
        "empty string formula must emit t=\"str\" and empty <v></v>: {xml}"
    );
    assert!(
        xml.contains("t=\"str\"><f>\"NaN\"</f>")
            && xml.contains("t=\"str\"><f>\"42\"</f>")
            && xml.contains("t=\"str\"><f>\"TRUE\"</f>")
            && xml.contains("t=\"str\"><f>\"#N/A\"</f>")
            && xml.contains("t=\"str\"><f>\"Hello\"</f><v>Hello</v></c>"),
        "all text formulas must carry t=\"str\" in worksheet XML: {xml}"
    );

    // Direct WorkbookEmitter call with non-finite CellValue::Number fails in cached live mode
    let mut num_table = TableSpec::new("NanTable", "NaN");
    num_table.columns.push(ColumnSpec::new("val"));
    num_table.rows.push(RowSpec::new("r1").with_expr("1 + 1"));
    let nan_manifest = ProjectManifest {
        model_name: "NanModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("NanTable")],
        }],
    };
    let nan_compiled = Compiler::compile(&nan_manifest, std::slice::from_ref(&num_table)).unwrap();
    let mut bad_values = indexmap::IndexMap::new();
    bad_values.insert(
        CellId::new("NanTable", "r1", "val"),
        f8_core::value::CellValue::Number(f64::NAN),
    );
    let bad_eval =
        f8_core::compiler::eval::EvaluatedModel::new(bad_values, nan_compiled.symbol_table.clone());
    for opts in [
        EmissionOptions::default(),
        EmissionOptions::default().with_values_only(true),
        EmissionOptions::default().with_omit_formula_cache(true),
    ] {
        assert!(
            WorkbookEmitter::emit_to_buffer(
                &nan_manifest,
                std::slice::from_ref(&num_table),
                &nan_compiled.grid,
                &nan_compiled.layout_map,
                &nan_compiled.lowered_map,
                &nan_compiled.theme,
                Some(&bad_eval),
                &opts,
            )
            .is_err()
        );
    }

    // Unevaluated literal fallback with non-finite ValueLiteral::Number also fails
    let mut nan_lit_grid = nan_compiled.grid.clone();
    nan_lit_grid
        .get_mut(&CellId::new("NanTable", "r1", "val"))
        .unwrap()
        .payload =
        f8_core::compiler::ir::CellPayload::Literal(f8_core::model::ValueLiteral::Number(f64::NAN));
    assert!(
        WorkbookEmitter::emit_to_buffer(
            &nan_manifest,
            &[num_table],
            &nan_lit_grid,
            &nan_compiled.layout_map,
            &nan_compiled.lowered_map,
            &nan_compiled.theme,
            None,
            &EmissionOptions::default(),
        )
        .is_err()
    );
}

#[test]
fn test_detailed_null_cell_with_note_and_metadata_emission() {
    use f8_core::model::id::{ColId, RowId};
    use f8_core::value::CellValue;

    let table: TableSpec = serde_json::from_str(
        r#"{
            "id": "PeerComps",
            "title": "Peer Comps",
            "columns": [
                { "kind": "data", "id": "company", "role": "descriptor", "headers": ["Company"] },
                { "kind": "data", "id": "ev", "headers": ["EV"], "format": "currency", "decimals": 1 },
                { "kind": "data", "id": "fcf", "headers": ["FCF"], "format": "currency", "decimals": 1 },
                {
                    "kind": "data",
                    "id": "ev_fcf",
                    "headers": ["EV / FCF"],
                    "format": "multiple",
                    "decimals": 1,
                    "expr": "PeerComps[@row, \"ev\"] / PeerComps[@row, \"fcf\"]"
                }
            ],
            "rows": [
                {
                    "kind": "data",
                    "id": "crm",
                    "cells": { "company": "Salesforce", "ev": 185.0, "fcf": 10.0 }
                },
                {
                    "kind": "data",
                    "id": "orcl",
                    "shade": "muted",
                    "highlight": true,
                    "cells": {
                        "company": "Oracle",
                        "ev": 400.0,
                        "fcf": -5.0,
                        "ev_fcf": {
                            "value": null,
                            "note": "Excluded: negative FCF due to AI capex",
                            "shade": "none",
                            "highlight": false
                        }
                    }
                },
                {
                    "kind": "data",
                    "id": "avg",
                    "cells": {
                        "company": "Average",
                        "ev": null,
                        "fcf": null,
                        "ev_fcf": { "expr": "AVERAGE(PeerComps[\"crm\"..\"orcl\", @col])" }
                    }
                }
            ]
        }"#,
    )
    .unwrap();

    let manifest = ProjectManifest {
        model_name: "PeerCompsModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Comps".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![TableId::from("PeerComps")],
        }],
    };

    let compiled = Compiler::compile(&manifest, std::slice::from_ref(&table)).unwrap();
    let t = TableId::from("PeerComps");
    let ev_fcf = ColId::from("ev_fcf");

    let crm = compiled
        .inspect_cell(&t, &RowId::from("crm"), &ev_fcf)
        .unwrap();
    assert_eq!(crm.evaluated_value, CellValue::Number(18.5));
    assert!(crm.is_formula);

    let orcl = compiled
        .inspect_cell(&t, &RowId::from("orcl"), &ev_fcf)
        .unwrap();
    assert_eq!(orcl.evaluated_value, CellValue::Empty);
    assert!(!orcl.is_formula);
    assert_eq!(orcl.formula_expr, None);
    assert_eq!(
        orcl.note.as_deref(),
        Some("Excluded: negative FCF due to AI capex")
    );
    assert_eq!(orcl.shade, None);
    assert!(!orcl.is_highlight);

    let avg = compiled
        .inspect_cell(&t, &RowId::from("avg"), &ev_fcf)
        .unwrap();
    assert_eq!(avg.evaluated_value, CellValue::Number(18.5));

    let orcl_ref = orcl
        .physical_address
        .strip_prefix("'Comps'!")
        .expect("orcl physical address must include sheet qualifier");

    let extract_cell_element = |xml: &str, cell_ref: &str| -> Option<String> {
        let needle = format!("r=\"{cell_ref}\"");
        let start = xml.find(&needle)?;
        let tag_start = xml[..start].rfind("<c ")?;
        let open_end = xml[tag_start..].find('>')? + tag_start;
        if xml[..open_end].ends_with('/') {
            Some(xml[tag_start..=open_end].to_string())
        } else {
            let close_end = xml[open_end..].find("</c>")? + open_end + "</c>".len();
            Some(xml[tag_start..close_end].to_string())
        }
    };

    for opts in [
        EmissionOptions::default(),
        EmissionOptions::default().with_values_only(true),
    ] {
        let buffer = compiled.emit_workbook(&opts).unwrap();
        let comments_xml = read_zip_entry(&buffer, "xl/comments1.xml");
        assert!(
            comments_xml.contains(&format!("ref=\"{orcl_ref}\"")),
            "comment must anchor to {orcl_ref}: {comments_xml}"
        );
        assert!(comments_xml.contains("Excluded: negative FCF due to AI capex"));

        let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
        let orcl_cell_xml =
            extract_cell_element(&sheet_xml, orcl_ref).expect("orcl blank cell must be emitted");
        assert!(
            !orcl_cell_xml.contains("<v>") && !orcl_cell_xml.contains("<f>"),
            "explicit null override must remain blank without <v> or <f>: {orcl_cell_xml}"
        );
    }
}
