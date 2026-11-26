#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use std::io::Read;

use f8_core::compiler::emission::{EmissionError, EmissionOptions, FormatCache, WorkbookEmitter};
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::layout::LayoutPlanner;
use f8_core::compiler::lowering::FormulaLowerer;
use f8_core::compiler::{Compiler, ir};
use f8_core::model::format::{SemanticStyle, Shade};
use f8_core::model::id::{SheetId, TableId};
use f8_core::model::manifest::{BandSpec, FreezePanes, LaneSpec, ProjectManifest, SheetSpec};
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
                layout: Default::default(),
                print: None,
                bands: SheetSpec::stacked_bands([TableId::from("IncomeStatement")]),
            },
            SheetSpec {
                name: "Summary".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                layout: Default::default(),
                print: None,
                bands: SheetSpec::stacked_bands([TableId::from("SummaryTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("LayoutTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("HeaderTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("Assumptions")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("SingleGroupTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("FormulaTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("FreezeTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("OutlineTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("DiskTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("DiagTable")]),
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

    // 6. Cleared or truncated placement.col_spans fails fast with MissingTable instead of silently skipping columns
    let mut cleared_spans_layout =
        LayoutPlanner::plan(&manifest, std::slice::from_ref(&table), &grid, &theme, None).unwrap();
    cleared_spans_layout
        .tables
        .get_mut(&TableId::from("DiagTable"))
        .unwrap()
        .col_spans
        .clear();
    let emit_res8 = WorkbookEmitter::emit_to_buffer(
        &manifest,
        std::slice::from_ref(&table),
        &grid,
        &cleared_spans_layout,
        &lowered_formulas,
        &theme,
        None,
        &EmissionOptions::default(),
    );
    assert!(matches!(
        emit_res8,
        Err(EmissionError::MissingTable { table }) if table == TableId::from("DiagTable")
    ));

    let mut two_col_table = TableSpec::new("DiagTable", "Diagnostics");
    two_col_table
        .columns
        .push(ColumnSpec::descriptor("item", "Item").with_group("Group"));
    two_col_table
        .columns
        .push(ColumnSpec::new("c1").with_group("Group").with_header("C1"));
    two_col_table.rows.push(
        RowSpec::new("r1")
            .with_cell("item", "Row")
            .with_cell("c1", 1.0),
    );
    let (two_sym, two_grid) =
        ir::expand_workbook(&manifest, std::slice::from_ref(&two_col_table)).unwrap();
    let mut truncated_spans_layout = LayoutPlanner::plan(
        &manifest,
        std::slice::from_ref(&two_col_table),
        &two_grid,
        &theme,
        None,
    )
    .unwrap();
    let two_lowered =
        FormulaLowerer::lower_grid(&two_grid, &two_sym, &truncated_spans_layout).unwrap();
    truncated_spans_layout
        .tables
        .get_mut(&TableId::from("DiagTable"))
        .unwrap()
        .col_spans
        .truncate(1);
    let emit_res9 = WorkbookEmitter::emit_to_buffer(
        &manifest,
        std::slice::from_ref(&two_col_table),
        &two_grid,
        &truncated_spans_layout,
        &two_lowered,
        &theme,
        None,
        &EmissionOptions::default(),
    );
    assert!(matches!(
        emit_res9,
        Err(EmissionError::MissingTable { table }) if table == TableId::from("DiagTable")
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("NoteTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("NoteTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("SectionTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("LboSchedule")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("GutterFormatting")]),
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
// Scenario 15b: Row Gutters Remain Blank Across Styled Columns
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_15b_row_gutter_remains_blank_across_styled_columns() {
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

    let build_table = || {
        let mut table = TableSpec::new("RowGutterBlank", "Row Gutter Test");
        // Col B: Descriptor
        table
            .columns
            .push(ColumnSpec::descriptor("line_item", "Line Item"));
        // Col C: Plain data measure
        table
            .columns
            .push(ColumnSpec::new("c_data").with_header("Data"));
        // Col D: Column gutter
        table.columns.push(ColumnSpec::gutter());
        // Col E: Subtotal column (dashed left border)
        table.columns.push(
            ColumnSpec::new("c_sub")
                .with_header("Sub")
                .with_style(Some(SemanticStyle::Subtotal)),
        );
        // Col F: Total column (thin left border, no right border)
        table.columns.push(
            ColumnSpec::new("c_tot")
                .with_header("Total")
                .with_style(Some(SemanticStyle::Total)),
        );
        // Col G: Muted shade column
        table.columns.push(
            ColumnSpec::new("c_shade")
                .with_header("Forecast")
                .with_shade(Some(Shade::Muted)),
        );
        // Col H: Highlight column
        table.columns.push(
            ColumnSpec::new("c_hl")
                .with_header("Key")
                .with_highlight(Some(true)),
        );
        // Col I: Explicit default/none styling
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

    let assert_gutter_row_blank = |sheet_xml: &str| {
        for populated in ["B5", "E5", "F5", "G5", "H5", "B7", "E7", "F7", "G7", "H7"] {
            assert!(
                extract_cell_element(sheet_xml, populated).is_some(),
                "{populated} on adjacent data row must be emitted"
            );
        }
        assert!(
            !sheet_xml.contains("<row r=\"6\""),
            "Row gutter (row 6) must not be emitted in worksheet XML"
        );
        for cell in ["B6", "C6", "D6", "E6", "F6", "G6", "H6", "I6"] {
            assert!(
                extract_cell_element(sheet_xml, cell).is_none(),
                "{cell} on row gutter must remain blank without column formatting"
            );
        }
    };

    // Part 1: Default corporate_navy theme
    let manifest = ProjectManifest {
        model_name: "RowGutterDefaultTheme".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("RowGutterBlank")]),
        }],
    };
    let buffer =
        Compiler::emit_workbook(&manifest, &[build_table()], &EmissionOptions::default()).unwrap();
    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
    assert_gutter_row_blank(&sheet_xml);

    // Part 2: Custom theme with descriptor_border_right enabled
    let mut custom_theme = Theme::corporate_navy();
    custom_theme.borders.descriptor_border_right = BorderStyle::Thin;
    let custom_manifest = ProjectManifest {
        model_name: "RowGutterDescriptorBorderTheme".to_string(),
        theme: ThemeSpec::Custom(Box::new(custom_theme)),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("RowGutterBlank")]),
        }],
    };
    let custom_buf = Compiler::emit_workbook(
        &custom_manifest,
        &[build_table()],
        &EmissionOptions::default(),
    )
    .unwrap();
    let custom_sheet_xml = read_zip_entry(&custom_buf, "xl/worksheets/sheet1.xml");
    assert_gutter_row_blank(&custom_sheet_xml);
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("WrapEmission")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("BlankCoercion")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("NegZeroTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("TextCacheTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("NanTable")]),
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
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("PeerComps")]),
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

// ----------------------------------------------------------------------------
// Scenario 20: Uniform Trailing Alignment Space OpenXML Emission
// ----------------------------------------------------------------------------
#[test]
fn test_scenario_20_uniform_trailing_alignment_space_openxml_emission() {
    use std::collections::BTreeMap;

    use f8_core::model::format::FormatPreset;
    use f8_core::model::theme::ThemeSpec;

    let build_table = || {
        let mut table = TableSpec::new("AlignTable", "Uniform Alignment Showcase");
        // Col B: Descriptor (multi-tier headers, format: None)
        table.columns.push(
            ColumnSpec::descriptor("item", "Line Item").with_headers(["Category", "Line Item"]),
        );
        // Col C: Currency (grouped, multi-tier headers)
        table.columns.push(
            ColumnSpec::new("curr")
                .with_group("Forecast")
                .with_headers(["FY25", "Currency"])
                .with_format(Some(FormatPreset::Currency)),
        );
        // Col D: Number (grouped, multi-tier headers)
        table.columns.push(
            ColumnSpec::new("num")
                .with_group("Forecast")
                .with_headers(["FY25", "Number"])
                .with_format(Some(FormatPreset::Number)),
        );
        // Col E: Percentage
        table.columns.push(
            ColumnSpec::new("pct")
                .with_headers(["Margin"])
                .with_format(Some(FormatPreset::Percentage)),
        );
        // Col F: Multiple
        table.columns.push(
            ColumnSpec::new("mult")
                .with_headers(["Multiple"])
                .with_format(Some(FormatPreset::Multiple)),
        );
        // Col G: BasisPoints
        table.columns.push(
            ColumnSpec::new("bps")
                .with_headers(["Spread"])
                .with_format(Some(FormatPreset::BasisPoints)),
        );
        // Col H: Integer
        table.columns.push(
            ColumnSpec::new("int_col")
                .with_headers(["Year"])
                .with_format(Some(FormatPreset::Integer)),
        );
        // Col I: Date (multi-tier headers)
        table.columns.push(
            ColumnSpec::new("dt")
                .with_headers(["Close", "Date"])
                .with_format(Some(FormatPreset::Date)),
        );
        // Col J: Text
        table.columns.push(
            ColumnSpec::new("txt")
                .with_headers(["Status"])
                .with_format(Some(FormatPreset::Text)),
        );
        // Col K: Unformatted measure (format: None)
        table
            .columns
            .push(ColumnSpec::new("raw").with_headers(["Unformatted"]));

        // Layout: Title=2, TitleGutter=3, Header Tier 0=4, Tier 1=5, Tier 2=6, Body r1=7, r2=8
        table.rows.push(
            RowSpec::new("r1")
                .with_cell("item", "Revenue")
                .with_cell("curr", 1000.0)
                .with_cell("num", 500.0)
                .with_cell("pct", 0.25)
                .with_cell("mult", 8.5)
                .with_cell("bps", 0.005)
                .with_cell("int_col", 2025.0)
                .with_cell("dt", 45657.0)
                .with_cell("txt", "Audited")
                .with_cell("raw", 1234.5),
        );
        table.rows.push(
            RowSpec::new("r2")
                .with_cell("item", "Memo Row")
                .with_cell("curr", "N/A")
                .with_cell("num", -50.0)
                .with_cell("pct", 0.0)
                .with_cell("mult", 0.0)
                .with_cell("bps", 0.0)
                .with_cell("int_col", -42.0)
                .with_cell("dt", 45658.0)
                .with_cell("txt", "Final")
                .with_cell("raw", "Memo"),
        );
        table
    };

    let parse_attr = |tag: &str, attr: &str| -> Option<String> {
        let needle = format!("{attr}=\"");
        let start = tag.find(&needle)? + needle.len();
        let end = tag[start..].find('"')? + start;
        Some(tag[start..end].to_string())
    };

    let parse_num_fmts = |styles_xml: &str| -> BTreeMap<usize, String> {
        let mut map = BTreeMap::new();
        for part in styles_xml.split("<numFmt ").skip(1) {
            let end = part.find("/>").unwrap_or(part.len());
            let tag = &part[..end];
            if let (Some(id_str), Some(code)) =
                (parse_attr(tag, "numFmtId"), parse_attr(tag, "formatCode"))
            {
                let id = id_str.parse::<usize>().unwrap();
                map.insert(id, code.replace("&quot;", "\""));
            }
        }
        map
    };

    let parse_cell_xf_num_fmt_ids = |styles_xml: &str| -> Vec<usize> {
        let cell_xfs_start = styles_xml.find("<cellXfs").expect("<cellXfs> not found");
        let cell_xfs_end =
            styles_xml[cell_xfs_start..].find("</cellXfs>").unwrap() + cell_xfs_start;
        let cell_xfs_xml = &styles_xml[cell_xfs_start..cell_xfs_end];

        cell_xfs_xml
            .split("<xf ")
            .skip(1)
            .map(|xf| {
                let end = xf.find('>').unwrap_or(xf.len());
                parse_attr(&xf[..end], "numFmtId").map_or(0, |s| s.parse::<usize>().unwrap())
            })
            .collect()
    };

    let cell_num_fmt = |sheet_xml: &str,
                        xf_num_fmt_ids: &[usize],
                        num_fmts: &BTreeMap<usize, String>,
                        cell_ref: &str|
     -> (usize, Option<String>) {
        let needle = format!("r=\"{cell_ref}\"");
        let start = sheet_xml
            .find(&needle)
            .unwrap_or_else(|| panic!("Cell {cell_ref} not found in sheet1.xml"));
        let tag_start = sheet_xml[..start].rfind("<c ").unwrap();
        let tag_end = sheet_xml[start..].find('>').unwrap() + start + 1;
        let tag = &sheet_xml[tag_start..tag_end];
        let style_idx = parse_attr(tag, "s")
            .unwrap_or_else(|| panic!("Cell {cell_ref} missing s attribute: {tag}"))
            .parse::<usize>()
            .unwrap();
        let num_fmt_id = xf_num_fmt_ids[style_idx];
        (num_fmt_id, num_fmts.get(&num_fmt_id).cloned())
    };

    // Part 1: Default corporate_navy theme
    let manifest = ProjectManifest {
        model_name: "AlignEmissionModel".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("AlignTable")]),
        }],
    };
    let table = build_table();
    let buffer = Compiler::emit_workbook(
        &manifest,
        std::slice::from_ref(&table),
        &EmissionOptions::default(),
    )
    .unwrap();
    let buffer2 = Compiler::emit_workbook(
        &manifest,
        std::slice::from_ref(&table),
        &EmissionOptions::default(),
    )
    .unwrap();
    assert_eq!(
        buffer, buffer2,
        "OpenXML emission must be bit-for-bit deterministic"
    );

    let styles_xml = read_zip_entry(&buffer, "xl/styles.xml");
    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
    let num_fmts = parse_num_fmts(&styles_xml);
    let xf_num_fmt_ids = parse_cell_xf_num_fmt_ids(&styles_xml);

    // Super-header ("Forecast" at C4, D4) -> native numFmtId="0"
    assert_eq!(
        cell_num_fmt(&sheet_xml, &xf_num_fmt_ids, &num_fmts, "C4"),
        (0, None)
    );
    assert_eq!(
        cell_num_fmt(&sheet_xml, &xf_num_fmt_ids, &num_fmts, "D4"),
        (0, None)
    );

    // Descriptor column headers (B5 "Category", B6 "Line Item") and unformatted descriptor cells (B7, B8) -> numFmtId="0"
    for desc_ref in ["B5", "B6", "B7", "B8"] {
        assert_eq!(
            cell_num_fmt(&sheet_xml, &xf_num_fmt_ids, &num_fmts, desc_ref),
            (0, None),
            "Descriptor cell/header {desc_ref} must use native numFmtId=0"
        );
    }

    // Measure column headers across upper and bottom tiers -> "@_)"
    for hdr_ref in [
        "C5", "C6", "D5", "D6", "E6", "F6", "G6", "H6", "I5", "I6", "J6", "K6",
    ] {
        let (fmt_id, code) = cell_num_fmt(&sheet_xml, &xf_num_fmt_ids, &num_fmts, hdr_ref);
        assert_ne!(
            fmt_id, 0,
            "Measure header {hdr_ref} must reference custom numFmt"
        );
        assert_eq!(
            code.as_deref(),
            Some("@_)"),
            "Measure header {hdr_ref} must use @_)"
        );
    }

    // All 6 numeric presets carry 4-section masks ending in ";@_)" (including text value in currency col C8)
    let expected_numeric_cells = [
        ("C7", "$#,##0_);($#,##0);\"-\"_);@_)"),
        ("C8", "$#,##0_);($#,##0);\"-\"_);@_)"),
        ("D7", "#,##0_);(#,##0);\"-\"_);@_)"),
        ("E7", "0.0%_);(0.0%);\"-\"_);@_)"),
        ("F7", "0.0\"x\"_);(0.0\"x\");\"-\"_);@_)"),
        ("G7", "#,##0\" bps\"_);(#,##0\" bps\");\"-\"_);@_)"),
        ("H7", "0_);(-0);\"-\"_);@_)"),
    ];
    for (cell_ref, expected_code) in expected_numeric_cells {
        let (_, code) = cell_num_fmt(&sheet_xml, &xf_num_fmt_ids, &num_fmts, cell_ref);
        assert_eq!(
            code.as_deref(),
            Some(expected_code),
            "Cell {cell_ref} must use 4-section numeric mask"
        );
        assert!(expected_code.ends_with(";@_)"));
    }

    // Date cell -> "yyyy-mm-dd_)"
    assert_eq!(
        cell_num_fmt(&sheet_xml, &xf_num_fmt_ids, &num_fmts, "I7")
            .1
            .as_deref(),
        Some("yyyy-mm-dd_)")
    );

    // Text measure cell -> "@_)"
    assert_eq!(
        cell_num_fmt(&sheet_xml, &xf_num_fmt_ids, &num_fmts, "J7")
            .1
            .as_deref(),
        Some("@_)")
    );

    // Unformatted measure cells (numeric K7 and text K8) -> "General_);(General);0_);@_)"
    for raw_ref in ["K7", "K8"] {
        assert_eq!(
            cell_num_fmt(&sheet_xml, &xf_num_fmt_ids, &num_fmts, raw_ref)
                .1
                .as_deref(),
            Some("General_);(General);0_);@_)"),
            "Unformatted measure cell {raw_ref} must use default general alignment mask"
        );
    }

    // Part 2: Custom theme opting out via general = "General" and text = "General"
    let mut opt_out_theme = Theme::corporate_navy();
    opt_out_theme.number_formats.general = "General".to_string();
    opt_out_theme.number_formats.text = "General".to_string();
    let opt_out_manifest = ProjectManifest {
        model_name: "AlignOptOutModel".to_string(),
        theme: ThemeSpec::Custom(Box::new(opt_out_theme)),
        sheets: manifest.sheets,
    };
    let opt_out_buf =
        Compiler::emit_workbook(&opt_out_manifest, &[table], &EmissionOptions::default()).unwrap();
    let opt_out_styles = read_zip_entry(&opt_out_buf, "xl/styles.xml");
    let opt_out_sheet = read_zip_entry(&opt_out_buf, "xl/worksheets/sheet1.xml");
    let opt_out_num_fmts = parse_num_fmts(&opt_out_styles);
    let opt_out_xf_ids = parse_cell_xf_num_fmt_ids(&opt_out_styles);

    for opt_out_ref in ["C5", "C6", "I5", "I6", "J6", "J7", "J8", "K6", "K7", "K8"] {
        assert_eq!(
            cell_num_fmt(
                &opt_out_sheet,
                &opt_out_xf_ids,
                &opt_out_num_fmts,
                opt_out_ref
            ),
            (0, None),
            "Opt-out theme cell/header {opt_out_ref} must use native numFmtId=0"
        );
    }
}

#[test]
fn test_bold_style_emits_bold_without_borders_and_skips_gutter_blanks() {
    let mut table = TableSpec::new("BoldEmit", "Bold Emission");
    table.columns = vec![
        ColumnSpec::descriptor("item", "Item"),
        ColumnSpec::new("normal").with_header("Normal"),
        ColumnSpec::gutter(),
        ColumnSpec::new("bold_col")
            .with_header("Bold Col")
            .with_style(Some(SemanticStyle::Bold)),
        ColumnSpec::new("sub_col")
            .with_header("Subtotal Col")
            .with_style(Some(SemanticStyle::Subtotal)),
    ];
    table.rows = vec![
        RowSpec::new("r_data")
            .with_cell("item", "Data")
            .with_cell(
                "normal",
                DetailedCellInput {
                    value: Some(ValueLiteral::Number(10.0)),
                    style: Some(SemanticStyle::Bold),
                    ..Default::default()
                },
            )
            .with_cell("bold_col", 20.0)
            .with_cell("sub_col", 30.0),
        RowSpec::new("r_bold")
            .with_style(Some(SemanticStyle::Bold))
            .with_cell("item", "Bold Row")
            .with_cell("normal", 11.0)
            .with_cell("bold_col", 21.0)
            .with_cell("sub_col", 31.0),
        RowSpec::new("r_bold_shade")
            .with_style(Some(SemanticStyle::Bold))
            .with_shade(Some(Shade::Muted))
            .with_cell("item", "Bold Shaded")
            .with_cell("normal", 12.0)
            .with_cell("bold_col", 22.0)
            .with_cell("sub_col", 32.0),
        RowSpec::new("r_sub")
            .with_style(Some(SemanticStyle::Subtotal))
            .with_cell("item", "Subtotal Row")
            .with_cell("normal", 13.0)
            .with_cell("bold_col", 23.0)
            .with_cell("sub_col", 33.0),
    ];

    let manifest = ProjectManifest {
        model_name: "BoldEmitModel".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Main".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("BoldEmit")]),
        }],
    };

    let buffer = Compiler::emit_workbook(&manifest, &[table], &EmissionOptions::default()).unwrap();
    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
    let styles_xml = read_zip_entry(&buffer, "xl/styles.xml");

    // Extract <fonts>, <borders>, and <cellXfs> entries from styles.xml
    let extract_tags = |xml: &str, container: &str, item: &str| -> Vec<String> {
        let open_container = format!("<{container}");
        let close_container = format!("</{container}>");
        let start = xml.find(&open_container).unwrap();
        let body_start = xml[start..].find('>').unwrap() + start + 1;
        let end = xml[body_start..].find(&close_container).unwrap() + body_start;
        let section = &xml[body_start..end];
        let open_item = format!("<{item}");
        let close_item = format!("</{item}>");
        let mut out = Vec::new();
        let mut rest = section;
        while let Some(pos) = rest.find(&open_item) {
            let tail = &rest[pos..];
            let end_self = tail.find("/>");
            let end_open = tail.find('>');
            let end_close = tail.find(&close_item);
            let len = match (end_self, end_open, end_close) {
                (Some(s), Some(o), _) if s + 1 == o => s + 2,
                (_, _, Some(c)) => c + close_item.len(),
                _ => break,
            };
            out.push(tail[..len].to_string());
            rest = &tail[len..];
        }
        out
    };

    let fonts = extract_tags(&styles_xml, "fonts", "font");
    let borders = extract_tags(&styles_xml, "borders", "border");
    let xfs = extract_tags(&styles_xml, "cellXfs", "xf");

    let parse_attr = |tag: &str, attr: &str| -> usize {
        let needle = format!(r#"{attr}=""#);
        let start = tag.find(&needle).unwrap() + needle.len();
        let end = tag[start..].find('"').unwrap() + start;
        tag[start..end].parse().unwrap()
    };

    let cell_style_info = |cell_ref: &str| -> (String, String) {
        let needle = format!(r#"<c r="{cell_ref}""#);
        let start = sheet_xml
            .find(&needle)
            .unwrap_or_else(|| panic!("Missing cell {cell_ref} in {sheet_xml}"));
        let end = sheet_xml[start..].find('>').unwrap() + start + 1;
        let c_tag = &sheet_xml[start..end];
        let xf_idx = parse_attr(c_tag, "s");
        let xf = &xfs[xf_idx];
        let font_id = parse_attr(xf, "fontId");
        let border_id = parse_attr(xf, "borderId");
        (fonts[font_id].clone(), borders[border_id].clone())
    };

    // Physical layout:
    // Title = row 2 (idx 1), title gutter = row 3 (idx 2), header = row 4 (idx 3)
    // Body rows: r_data = row 5, r_bold = row 6, r_bold_shade = row 7, r_sub = row 8
    // Columns: item = B, normal = C, gutter = D, bold_col = E, sub_col = F

    // 0D bold cell (C5), bold column cell (E5), and bold row cells (B6, C6, E6) -> bold font, NO borders
    for bold_no_border_ref in ["C5", "E5", "B6", "C6", "E6"] {
        let (font, border) = cell_style_info(bold_no_border_ref);
        assert!(
            font.contains("<b/>"),
            "Cell {bold_no_border_ref} must be bold: {font}"
        );
        assert!(
            !border.contains("style="),
            "Cell {bold_no_border_ref} must have zero borders: {border}"
        );
    }

    // Contrast: subtotal column cell (F5) and subtotal row x bold col (E8) carry accounting borders
    let (f5_font, f5_border) = cell_style_info("F5");
    assert!(f5_font.contains("<b/>"));
    assert!(
        f5_border.contains(r#"<left style="dashed">"#),
        "Subtotal col cell F5 must have dashed left border: {f5_border}"
    );
    assert!(!f5_border.contains("<top style="));

    let (e8_font, e8_border) = cell_style_info("E8");
    assert!(e8_font.contains("<b/>"));
    assert!(
        e8_border.contains(r#"<top style="thin">"#),
        "Subtotal row x bold col E8 must have thin top border: {e8_border}"
    );
    assert!(
        !e8_border.contains("<left style="),
        "Subtotal row x bold col E8 must NOT have left border: {e8_border}"
    );

    // Gutter column D: bold-only row (D6) skips emitting blank cell; bold+shade row (D7) emits shaded blank without border/bold
    assert!(
        !sheet_xml.contains(r#"<c r="D6""#),
        "Bold-only row must not emit blank gutter cell D6"
    );
    let (d7_font, d7_border) = cell_style_info("D7");
    assert!(
        !d7_font.contains("<b/>"),
        "Gutter cell D7 should reuse unbolded shade format: {d7_font}"
    );
    assert!(
        !d7_border.contains("style="),
        "Gutter cell D7 must have no borders: {d7_border}"
    );
}

#[test]
fn test_aligned_title_banners_openxml_stacked_and_side_by_side_emission() {
    let cell_style_idx = |sheet_xml: &str, cell_ref: &str| -> Option<String> {
        let needle = format!(r#"<c r="{cell_ref}""#);
        let start = sheet_xml.find(&needle)?;
        let end = sheet_xml[start..].find('>')? + start + 1;
        let tag = &sheet_xml[start..end];
        let s_needle = r#" s=""#;
        let s_start = tag.find(s_needle)? + s_needle.len();
        let s_end = tag[s_start..].find('"')? + s_start;
        Some(tag[s_start..s_end].to_string())
    };

    // Part 1: Stacked tables (2, 5, 3 cols across bands and within a lane) with subtitles and total rows
    let make_stacked = |id: &str, title: &str, subtitle: &str, cols: usize| {
        let mut t = TableSpec::new(id, title).with_subtitle(subtitle);
        t.columns.push(ColumnSpec::descriptor("item", "Item"));
        for c in 1..cols {
            t.columns
                .push(ColumnSpec::new(format!("m{c}")).with_header(format!("M{c}")));
        }
        t.rows.push(RowSpec::new("r1"));
        t.rows
            .push(RowSpec::new("tot").with_style(Some(SemanticStyle::Total)));
        populate_table_cells(&mut t);
        t
    };

    let t1 = make_stacked("T1", "Narrow Top", "Sub 1", 2);
    let t2 = make_stacked("T2", "Wide Middle", "Sub 2", 5);
    let t3 = make_stacked("T3", "Medium Bottom", "Sub 3", 3);

    let stacked_manifest = ProjectManifest {
        model_name: "AlignedBannersStacked".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Model".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            layout: Default::default(),
            print: None,
            bands: vec![
                BandSpec::single("T1"),
                BandSpec::new([LaneSpec::new(["T2", "T3"])]),
            ],
        }],
    };
    let stacked_tables = vec![t1, t2, t3];
    let compiled = Compiler::compile(&stacked_manifest, &stacked_tables).unwrap();

    let buf1 = compiled.emit_workbook(&EmissionOptions::default()).unwrap();
    let buf2 = compiled.emit_workbook(&EmissionOptions::default()).unwrap();
    assert_eq!(
        buf1, buf2,
        "Aligned banner emission must be bit-for-bit deterministic"
    );

    let sheet_xml = read_zip_entry(&buf1, "xl/worksheets/sheet1.xml");
    let shared_strings = read_zip_entry(&buf1, "xl/sharedStrings.xml");
    assert!(shared_strings.contains("Narrow Top"));
    assert!(shared_strings.contains("Wide Middle"));
    assert!(shared_strings.contains("Medium Bottom"));

    // For each table: title banner spans B..F (cols 1..=5) with identical title style index,
    // while subtitle, header, and total rows remain strictly within start_col..=end_col.
    let expected_tables = [("T1", 2_u32), ("T2", 5_u32), ("T3", 3_u32)];
    let mut expected_title_style: Option<String> = None;
    for (id, end_col) in expected_tables {
        let p = compiled.layout_map.get_table(&TableId::from(id)).unwrap();
        assert_eq!(p.end_col, end_col);
        assert_eq!(p.title_end_col, Some(5));

        let title_excel_row = p.title_row.unwrap() + 1;
        let b_ref = format!("B{title_excel_row}");
        let b_style = cell_style_idx(&sheet_xml, &b_ref)
            .unwrap_or_else(|| panic!("Missing title cell {b_ref}"));
        if let Some(ref prev) = expected_title_style {
            assert_eq!(
                &b_style, prev,
                "All title banners must reuse identical title style"
            );
        } else {
            expected_title_style = Some(b_style.clone());
        }
        // Only B carries shared-string type t="s"; C..F are styled blanks with same style index
        let b_tag_start = sheet_xml.find(&format!(r#"<c r="{b_ref}""#)).unwrap();
        let b_tag_end = sheet_xml[b_tag_start..].find('>').unwrap() + b_tag_start + 1;
        assert!(sheet_xml[b_tag_start..b_tag_end].contains(r#"t="s""#));

        for col_letter in ["C", "D", "E", "F"] {
            let cell_ref = format!("{col_letter}{title_excel_row}");
            assert_eq!(
                cell_style_idx(&sheet_xml, &cell_ref).as_deref(),
                Some(b_style.as_str()),
                "{id} title banner cell {cell_ref} must share title style"
            );
            let tag_start = sheet_xml.find(&format!(r#"<c r="{cell_ref}""#)).unwrap();
            let tag_end = sheet_xml[tag_start..].find('>').unwrap() + tag_start + 1;
            assert!(
                !sheet_xml[tag_start..tag_end].contains(r#"t="s""#),
                "{id} continuation title cell {cell_ref} must be styled blank"
            );
        }

        // Subtitle, header, and total rows stay strictly within B..=end_col
        let sub_excel_row = p.subtitle_row_range.unwrap().0 + 1;
        let hdr_excel_row = p.header_row_range.unwrap().0 + 1;
        let tot_excel_row = p.body_row_range.1 + 1;

        assert!(cell_style_idx(&sheet_xml, &format!("B{sub_excel_row}")).is_some());
        for col in (end_col + 1)..=5 {
            let col_letter = f8_core::compiler::layout::col_to_excel_col(col);
            assert!(
                !sheet_xml.contains(&format!(r#"<c r="{col_letter}{sub_excel_row}""#)),
                "{id} subtitle must not extend to {col_letter}{sub_excel_row}"
            );
            assert!(
                !sheet_xml.contains(&format!(r#"<c r="{col_letter}{hdr_excel_row}""#)),
                "{id} header must not extend to {col_letter}{hdr_excel_row}"
            );
            assert!(
                !sheet_xml.contains(&format!(r#"<c r="{col_letter}{tot_excel_row}""#)),
                "{id} total border must not extend to {col_letter}{tot_excel_row}"
            );
        }
    }

    // Part 2: Side-by-side non-collision (tall L B..C beside R1 E..G, R2 E..F, R3 E..J)
    let make_plain = |id: &str, title: &str, cols: usize, rows: usize| {
        let mut t = TableSpec::new(id, title);
        for c in 1..=cols {
            t.columns
                .push(ColumnSpec::new(format!("c{c}")).with_header(format!("C{c}")));
        }
        for r in 1..=rows {
            t.rows.push(RowSpec::new(format!("r{r}")));
        }
        populate_table_cells(&mut t);
        t
    };

    let side_tables = vec![
        make_plain("L", "Left Tall", 2, 27),
        make_plain("R1", "Right 1", 3, 2),
        make_plain("R2", "Right 2", 2, 2),
        make_plain("R3", "Right 3", 6, 2),
    ];
    let side_manifest = ProjectManifest {
        model_name: "AlignedBannersSideBySide".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Dashboard".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            layout: Default::default(),
            print: None,
            bands: vec![BandSpec::new([
                LaneSpec::new(["L"]),
                LaneSpec::new(["R1", "R2", "R3"]),
            ])],
        }],
    };
    let side_compiled = Compiler::compile(&side_manifest, &side_tables).unwrap();
    let side_buf = side_compiled
        .emit_workbook(&EmissionOptions::default())
        .unwrap();
    let side_xml = read_zip_entry(&side_buf, "xl/worksheets/sheet1.xml");

    // Row 2 (physical row 1): L occupies B2..C2, D2 is blank gutter, R1 occupies E2..J2
    let l_style = cell_style_idx(&side_xml, "B2").expect("B2 title cell");
    assert_eq!(
        cell_style_idx(&side_xml, "C2").as_deref(),
        Some(l_style.as_str())
    );
    assert!(
        !side_xml.contains(r#"<c r="D2""#),
        "Gutter column D2 between L and R1 must remain blank"
    );
    for col_letter in ["E", "F", "G", "H", "I", "J"] {
        assert_eq!(
            cell_style_idx(&side_xml, &format!("{col_letter}2")).as_deref(),
            Some(l_style.as_str()),
            "R1 title banner cell {col_letter}2 must be emitted with title style"
        );
    }

    // R2 and R3 title rows also extend across E..J while leaving gutter col D blank
    for right_id in ["R2", "R3"] {
        let pr = side_compiled
            .layout_map
            .get_table(&TableId::from(right_id))
            .unwrap();
        let row_num = pr.title_row.unwrap() + 1;
        assert!(
            !side_xml.contains(&format!(r#"<c r="D{row_num}""#)),
            "Gutter D{row_num} beside {right_id} title must remain blank"
        );
        for col_letter in ["E", "F", "G", "H", "I", "J"] {
            assert_eq!(
                cell_style_idx(&side_xml, &format!("{col_letter}{row_num}")).as_deref(),
                Some(l_style.as_str()),
                "{right_id} title cell {col_letter}{row_num} must share title style"
            );
        }
    }

    // Part 3: Explicit lane_gap = 0 side-by-side non-collision (T1 B..C, T2 D..E)
    let gap0_tables = vec![
        make_plain("T1", "Left Tight", 2, 1),
        make_plain("T2", "Right Tight", 2, 1),
    ];
    let gap0_manifest = ProjectManifest {
        model_name: "GapZeroEmission".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Tight".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            layout: Default::default(),
            print: None,
            bands: vec![
                BandSpec::new([LaneSpec::new(["T1"]), LaneSpec::new(["T2"])]).with_lane_gap(0),
            ],
        }],
    };
    let gap0_buf =
        Compiler::emit_workbook(&gap0_manifest, &gap0_tables, &EmissionOptions::default()).unwrap();
    let gap0_xml = read_zip_entry(&gap0_buf, "xl/worksheets/sheet1.xml");
    let gap0_strings = read_zip_entry(&gap0_buf, "xl/sharedStrings.xml");
    assert!(gap0_strings.contains("Left Tight"));
    assert!(gap0_strings.contains("Right Tight"));
    // Both B2 and D2 carry string values t="s" (no overwrite of D2 by T1's banner)
    for title_start_ref in ["B2", "D2"] {
        let pos = gap0_xml
            .find(&format!(r#"<c r="{title_start_ref}""#))
            .unwrap();
        let end = gap0_xml[pos..].find('>').unwrap() + pos + 1;
        assert!(
            gap0_xml[pos..end].contains(r#"t="s""#),
            "{title_start_ref} must remain a distinct title string cell"
        );
    }
    assert!(cell_style_idx(&gap0_xml, "C2").is_some());
    assert!(cell_style_idx(&gap0_xml, "E2").is_some());
    assert!(!gap0_xml.contains(r#"<c r="F2""#));
}

#[test]
fn test_sheet_gridlines_default_false_and_opt_in_true() {
    let manifest_json = r#"{
        "model_name": "GridlineDefaults",
        "sheets": [
            { "name": "DefaultHidden", "bands": [{ "lanes": [{ "tables": ["T1"] }] }] },
            { "name": "ExplicitVisible", "show_gridlines": true, "bands": [{ "lanes": [{ "tables": ["T2"] }] }] }
        ]
    }"#;
    let manifest: ProjectManifest = serde_json::from_str(manifest_json).unwrap();
    assert!(!manifest.sheets[0].show_gridlines);
    assert!(manifest.sheets[1].show_gridlines);

    let make_table = |id: &str| {
        let mut t = TableSpec::new(id, id);
        t.columns.push(ColumnSpec::new("c1").with_header("C1"));
        t.rows.push(RowSpec::new("r1").with_cell("c1", 1.0));
        t
    };

    let buffer = Compiler::emit_workbook(
        &manifest,
        &[make_table("T1"), make_table("T2")],
        &EmissionOptions::default(),
    )
    .unwrap();

    let hidden_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");
    let visible_xml = read_zip_entry(&buffer, "xl/worksheets/sheet2.xml");
    assert!(hidden_xml.contains(r#"showGridLines="0""#));
    assert!(!visible_xml.contains(r#"showGridLines="0""#));
}

#[test]
fn test_grid_layout_scenario_9_runway_styling_borders_headers_superheaders_gutters_and_determinism()
{
    use f8_core::SheetLayoutSpec;
    use f8_core::model::theme::{BorderStyle, ThemeSpec};

    let extract_tags = |xml: &str, container: &str, item: &str| -> Vec<String> {
        let open_container = format!("<{container}");
        let close_container = format!("</{container}>");
        let start = xml.find(&open_container).unwrap();
        let body_start = xml[start..].find('>').unwrap() + start + 1;
        let end = xml[body_start..].find(&close_container).unwrap() + body_start;
        let section = &xml[body_start..end];
        let open_item = format!("<{item}");
        let close_item = format!("</{item}>");
        let mut out = Vec::new();
        let mut rest = section;
        while let Some(pos) = rest.find(&open_item) {
            let tail = &rest[pos..];
            let end_self = tail.find("/>");
            let end_open = tail.find('>');
            let end_close = tail.find(&close_item);
            let len = match (end_self, end_open, end_close) {
                (Some(s), Some(o), _) if s + 1 == o => s + 2,
                (_, _, Some(c)) => c + close_item.len(),
                _ => break,
            };
            out.push(tail[..len].to_string());
            rest = &tail[len..];
        }
        out
    };

    let parse_attr = |tag: &str, attr: &str| -> usize {
        let needle = format!(r#"{attr}=""#);
        let start = tag.find(&needle).unwrap() + needle.len();
        let end = tag[start..].find('"').unwrap() + start;
        tag[start..end].parse().unwrap()
    };

    let cell_xf_info = |sheet_xml: &str,
                        xfs: &[String],
                        borders: &[String],
                        fills: &[String],
                        cell_ref: &str|
     -> (String, String, String) {
        let needle = format!(r#"<c r="{cell_ref}""#);
        let start = sheet_xml
            .find(&needle)
            .unwrap_or_else(|| panic!("Missing cell {cell_ref} in worksheet XML"));
        let end = sheet_xml[start..].find('>').unwrap() + start + 1;
        let c_tag = &sheet_xml[start..end];
        let xf_idx = parse_attr(c_tag, "s");
        let xf = &xfs[xf_idx];
        let border_id = parse_attr(xf, "borderId");
        let fill_id = parse_attr(xf, "fillId");
        (
            xf.clone(),
            borders[border_id].clone(),
            fills[fill_id].clone(),
        )
    };

    // Part A: Theme with descriptor_border_right: Thin + total_fill + vertical Subtotal & Total descriptors + SuperHeader + Gutter
    let mut bordered_theme = Theme::corporate_navy();
    bordered_theme.borders.descriptor_border_right = BorderStyle::Thin;
    bordered_theme.palette.total_fill = "#D9E1F2".to_string();

    let mut t1 = TableSpec::new("BorderedGrid", "Bordered Grid");
    // Descriptor col 0 (span 3: B..D, vertical Subtotal) with SuperHeader "Descriptors"
    t1.columns.push(
        ColumnSpec::descriptor("item", "Line Item")
            .with_group("Descriptors")
            .with_style(Some(SemanticStyle::Subtotal)),
    );
    // Intra-table gutter col (E = col 4 -> gutter_col_width 2.0)
    t1.columns.push(ColumnSpec::gutter());
    // Descriptor col 2 (span 3: F..H, vertical Total)
    t1.columns
        .push(ColumnSpec::descriptor("tot_desc", "Summary").with_style(Some(SemanticStyle::Total)));
    // Measure col 3 (I = col 8)
    t1.columns.push(
        ColumnSpec::new("val")
            .with_group("Metrics")
            .with_header("2024E"),
    );

    let label_28 = "Operating Expenses (Excl DA)";
    t1.rows.push(
        RowSpec::new("r_plain")
            .with_cell("item", label_28)
            .with_cell("tot_desc", label_28)
            .with_cell("val", 10.0),
    );
    t1.rows.push(
        RowSpec::new("r_shaded")
            .with_shade(Some(Shade::Muted))
            .with_cell("item", "Shaded Row")
            .with_cell("tot_desc", "Shaded Desc")
            .with_cell("val", 20.0),
    );
    t1.rows.push(
        RowSpec::new("r_hl")
            .with_cell(
                "item",
                CellInput::Detailed(DetailedCellInput {
                    value: Some(ValueLiteral::String("Highlighted Item".to_string())),
                    highlight: Some(true),
                    ..Default::default()
                }),
            )
            .with_cell("tot_desc", "Normal")
            .with_cell("val", 30.0),
    );
    t1.rows.push(
        RowSpec::new("r_total")
            .with_style(Some(SemanticStyle::Total))
            .with_cell("item", "Total Row")
            .with_cell("tot_desc", "Grand Total")
            .with_cell("val", 60.0),
    );

    let manifest_a = ProjectManifest {
        model_name: "GridEmissionA".to_string(),
        theme: ThemeSpec::Custom(Box::new(bordered_theme)),
        sheets: vec![SheetSpec {
            name: "GridA".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: false,
            layout: SheetLayoutSpec::grid_with_col_width(12.0),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("BorderedGrid")]),
        }],
    };

    let buf1 =
        Compiler::emit_workbook(&manifest_a, &[t1.clone()], &EmissionOptions::default()).unwrap();
    let buf2 = Compiler::emit_workbook(&manifest_a, &[t1], &EmissionOptions::default()).unwrap();
    assert_eq!(
        buf1, buf2,
        "Grid .xlsx emission must be bit-for-bit deterministic"
    );

    let sheet_xml = read_zip_entry(&buf1, "xl/worksheets/sheet1.xml");
    let styles_xml = read_zip_entry(&buf1, "xl/styles.xml");

    assert!(!sheet_xml.contains("<mergeCells"));
    assert!(!styles_xml.contains(r#"wrapText="1""#));

    let xfs = extract_tags(&styles_xml, "cellXfs", "xf");
    let borders = extract_tags(&styles_xml, "borders", "border");
    let fills = extract_tags(&styles_xml, "fills", "fill");

    // Layout rows: Title=2, title_gutter=3, SuperHeader=4, ColHeader=5,
    // r_plain=6, r_shaded=7, r_hl=8, r_total=9
    // Columns: item=B..D (1..=3), gutter=E (4), tot_desc=F..H (5..=7), val=I (8)

    // SuperHeader "Descriptors" spans B4..D4 with centerContinuous
    for ref_addr in ["B4", "C4", "D4"] {
        let (xf, _, _) = cell_xf_info(&sheet_xml, &xfs, &borders, &fills, ref_addr);
        assert!(
            xf.contains(r#"horizontal="centerContinuous""#),
            "SuperHeader {ref_addr} must carry CenterAcross: {xf}"
        );
    }

    // Bottom column header spans B5..D5 and F5..H5 with header bottom border
    for ref_addr in ["B5", "C5", "D5", "F5", "G5", "H5"] {
        let (_, border, _) = cell_xf_info(&sheet_xml, &xfs, &borders, &fills, ref_addr);
        assert!(
            border.contains(r#"<bottom style="thin""#),
            "Header {ref_addr} must carry bottom border: {border}"
        );
    }

    // r_plain (row 6), item (B6..D6, vertical Subtotal + descriptor_border_right=Thin):
    // B6 (Start): left dashed, no right; C6 (Middle): no-op skipped (no fill/no border); D6 (End): no left, right thin
    let (_, b6_border, _) = cell_xf_info(&sheet_xml, &xfs, &borders, &fills, "B6");
    assert!(b6_border.contains(r#"<left style="dashed""#));
    assert!(!b6_border.contains(r#"<right style="#));
    assert!(!sheet_xml.contains(r#"<c r="C6""#));
    let (_, d6_border, _) = cell_xf_info(&sheet_xml, &xfs, &borders, &fills, "D6");
    assert!(!d6_border.contains(r#"<left style="#));
    assert!(d6_border.contains(r#"<right style="thin""#));

    // r_plain (row 6), tot_desc (F6..H6, vertical Total with total_fill + descriptor_border_right=Thin, default col_total_border_right=None):
    // F6 (Start): left thin, no right; G6 (Middle): no left, no right, has total fill; H6 (End): no left, right thin (from descriptor_border_right)
    let (_, f6_border, _) = cell_xf_info(&sheet_xml, &xfs, &borders, &fills, "F6");
    assert!(f6_border.contains(r#"<left style="thin""#));
    assert!(!f6_border.contains(r#"<right style="#));
    let (_, g6_border, g6_fill) = cell_xf_info(&sheet_xml, &xfs, &borders, &fills, "G6");
    assert!(!g6_border.contains(r#"<left style="#));
    assert!(!g6_border.contains(r#"<right style="#));
    assert!(g6_fill.contains("patternFill"));
    let (_, h6_border, _) = cell_xf_info(&sheet_xml, &xfs, &borders, &fills, "H6");
    assert!(!h6_border.contains(r#"<left style="#));
    assert!(h6_border.contains(r#"<right style="thin""#));

    // r_shaded (row 7): C7 (Middle) carries muted fill with suppressed vertical borders
    let (_, c7_border, c7_fill) = cell_xf_info(&sheet_xml, &xfs, &borders, &fills, "C7");
    assert!(!c7_border.contains(r#"<left style="#));
    assert!(!c7_border.contains(r#"<right style="#));
    assert!(
        c7_fill.contains("F2F2F2") || c7_fill.contains("F7F7F7") || c7_fill.contains("fgColor")
    );

    // r_hl (row 8): highlighted descriptor anchor B8 + runway C8..D8 carry highlight fill
    let (_, _, b8_fill) = cell_xf_info(&sheet_xml, &xfs, &borders, &fills, "B8");
    let (_, _, c8_fill) = cell_xf_info(&sheet_xml, &xfs, &borders, &fills, "C8");
    let (_, _, d8_fill) = cell_xf_info(&sheet_xml, &xfs, &borders, &fills, "D8");
    assert_eq!(b8_fill, c8_fill);
    assert_eq!(b8_fill, d8_fill);

    // r_total (row 9): B9..D9 carry horizontal Total top thin + bottom double borders across entire runway
    for ref_addr in ["B9", "C9", "D9"] {
        let (_, border, _) = cell_xf_info(&sheet_xml, &xfs, &borders, &fills, ref_addr);
        assert!(border.contains(r#"<top style="thin""#));
        assert!(border.contains(r#"<bottom style="double""#));
    }

    // Gutter-only column E (1-indexed col 5) retains gutter_col_width (2.0 -> ~2.71 in XML), while data cols are wider
    assert!(
        sheet_xml.contains(r#"min="5" max="5" width="2.71"#),
        "Gutter column E (col 5) must retain gutter_col_width in XML: {sheet_xml}"
    );

    // Part B: Default corporate_navy theme (descriptor_border_right: None, col_total_border_right: None, data_fill: none)
    // Bold-only unshaded row skips no-op blank runway <c> elements; column subtotal uses dashed left border,
    // and column total uses thin left border with no right border.
    let mut t2 = TableSpec::new("BoldSkip", "Bold Skip");
    t2.columns.push(ColumnSpec::descriptor("item", "Item"));
    t2.columns.push(ColumnSpec::new("val").with_header("Val"));
    t2.columns.push(
        ColumnSpec::new("sub_col")
            .with_header("Sub")
            .with_style(Some(SemanticStyle::Subtotal)),
    );
    t2.columns.push(
        ColumnSpec::new("tot_col")
            .with_header("Total")
            .with_style(Some(SemanticStyle::Total)),
    );
    t2.rows.push(
        RowSpec::new("r_bold")
            .with_style(Some(SemanticStyle::Bold))
            .with_cell("item", label_28)
            .with_cell("val", 42.0)
            .with_cell("sub_col", 42.0)
            .with_cell("tot_col", 42.0),
    );
    let manifest_b = ProjectManifest {
        model_name: "BoldSkipModel".to_string(),
        theme: ThemeSpec::default(),
        sheets: vec![SheetSpec {
            name: "GridB".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: false,
            layout: SheetLayoutSpec::grid_with_col_width(12.0),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("BoldSkip")]),
        }],
    };
    let buf_b = Compiler::emit_workbook(&manifest_b, &[t2], &EmissionOptions::default()).unwrap();
    let xml_b = read_zip_entry(&buf_b, "xl/worksheets/sheet1.xml");
    let styles_b = read_zip_entry(&buf_b, "xl/styles.xml");
    let xfs_b = extract_tags(&styles_b, "cellXfs", "xf");
    let borders_b = extract_tags(&styles_b, "borders", "border");
    let fills_b = extract_tags(&styles_b, "fills", "fill");
    // Title=2, title_gutter=3, header=4, r_bold=5; item=B..D (1..=3), val=E (4), sub_col=F (5), tot_col=G (6)
    assert!(xml_b.contains(r#"<c r="B5""#));
    assert!(
        !xml_b.contains(r#"<c r="C5""#),
        "Bold-only unshaded runway C5 must be skipped"
    );
    assert!(
        !xml_b.contains(r#"<c r="D5""#),
        "Bold-only unshaded runway D5 must be skipped"
    );
    assert!(xml_b.contains(r#"<c r="E5""#));
    let (_, f5_border, _) = cell_xf_info(&xml_b, &xfs_b, &borders_b, &fills_b, "F5");
    assert!(f5_border.contains(r#"<left style="dashed""#));
    assert!(!f5_border.contains(r#"<right style="#));
    let (_, g5_border, _) = cell_xf_info(&xml_b, &xfs_b, &borders_b, &fills_b, "G5");
    assert!(g5_border.contains(r#"<left style="thin""#));
    assert!(!g5_border.contains(r#"<right style="#));
}

// ----------------------------------------------------------------------------
// Scenario: OpenXML quotePrefix="1" for literal strings starting with Excel
// formula or text-prefix trigger characters (=, +, -, @ after optional leading
// ASCII whitespace, or leading ').
// ----------------------------------------------------------------------------
#[test]
fn test_literal_string_quote_prefix_emission() {
    use f8_core::model::theme::ThemeSpec;

    let mut table =
        TableSpec::new("IntegrityChecks", "= Integrity Checks").with_subtitle("= Tolerance checks");
    table
        .columns
        .push(ColumnSpec::descriptor("item", "Check").with_group("+ Controls"));
    table
        .columns
        .push(ColumnSpec::descriptor("target", "'26E").with_group("+ Controls"));
    table
        .columns
        .push(ColumnSpec::new("measure_text").with_header("Measure"));
    table
        .columns
        .push(ColumnSpec::new("fy26").with_header("'26E"));

    // Section header starting with '-'
    table.rows.push(RowSpec::section_header("- Balance Checks"));

    let quoted_cases = [
        ("eq", "= $0.00"),
        ("plus", "+ Add: D&A"),
        ("minus", "- Less: CapEx"),
        ("at", "@memo"),
        ("apos", "'26E"),
        ("ws_eq", "  = $0.00"),
        ("tab_minus", "\t-100"),
        ("crlf_eq", "\r\n=1"),
        ("nl_plus", "\n+1"),
    ];
    for (row_id, text) in quoted_cases {
        table.rows.push(
            RowSpec::new(row_id)
                .with_cell("item", text)
                .with_cell("target", text)
                .with_cell("measure_text", text)
                .with_cell("fy26", text),
        );
    }

    let plain_cases = [
        ("plain_dollar", "$0.00"),
        ("plain_gte", ">= $0.00"),
        ("plain_unicode_gte", "≥ $0.00"),
        ("plain_emdash", "—"),
        ("plain_ws_apos", "  '26E"),
        ("plain_spaces", "   "),
    ];
    for (row_id, text) in plain_cases {
        table.rows.push(
            RowSpec::new(row_id)
                .with_cell("item", text)
                .with_cell("target", text)
                .with_cell("measure_text", text)
                .with_cell("fy26", text),
        );
    }

    // Formula returning a string starting with '=' and a runtime error cell
    table.rows.push(
        RowSpec::new("formula_eq")
            .with_cell("item", "Formula Text")
            .with_cell("target", "Safe")
            .with_cell(
                "measure_text",
                CellInput::formula(r#"IF(TRUE, "= $0.00", "NO")"#),
            )
            .with_cell("fy26", 0.0),
    );
    table.rows.push(
        RowSpec::new("formula_err")
            .with_cell("item", "Div Zero")
            .with_cell("target", "Safe")
            .with_cell("measure_text", CellInput::formula("1 / 0"))
            .with_cell("fy26", 0.0),
    );

    let mut theme = Theme::corporate_navy();
    theme.palette.section_header_fill = "#F0F0F0".to_string();

    let manifest = ProjectManifest {
        model_name: "QuotePrefixModel".to_string(),
        theme: ThemeSpec::Custom(Box::new(theme)),
        sheets: vec![SheetSpec {
            name: "Checks".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: false,
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("IntegrityChecks")]),
        }],
    };

    let buf_live_1 =
        Compiler::emit_workbook(&manifest, &[table.clone()], &EmissionOptions::default()).unwrap();
    let buf_live_2 =
        Compiler::emit_workbook(&manifest, &[table.clone()], &EmissionOptions::default()).unwrap();
    assert_eq!(buf_live_1, buf_live_2, "emission must remain deterministic");

    let buf_frozen = Compiler::emit_workbook(
        &manifest,
        &[table],
        &EmissionOptions::new().with_values_only(true),
    )
    .unwrap();

    let extract_tags = |xml: &str, container: &str, item: &str| -> Vec<String> {
        let open_container = format!("<{container}");
        let close_container = format!("</{container}>");
        let start = xml.find(&open_container).unwrap();
        let body_start = xml[start..].find('>').unwrap() + start + 1;
        let end = xml[body_start..].find(&close_container).unwrap() + body_start;
        let section = &xml[body_start..end];
        let open_item = format!("<{item}");
        let close_item = format!("</{item}>");
        let mut out = Vec::new();
        let mut rest = section;
        while let Some(pos) = rest.find(&open_item) {
            let tail = &rest[pos..];
            let end_self = tail.find("/>");
            let end_open = tail.find('>');
            let end_close = tail.find(&close_item);
            let len = match (end_self, end_open, end_close) {
                (Some(s), Some(o), _) if s + 1 == o => s + 2,
                (_, _, Some(c)) => c + close_item.len(),
                _ => break,
            };
            out.push(tail[..len].to_string());
            rest = &tail[len..];
        }
        out
    };

    let parse_attr = |tag: &str, attr: &str| -> usize {
        let needle = format!(r#"{attr}=""#);
        let start = tag.find(&needle).unwrap() + needle.len();
        let end = tag[start..].find('"').unwrap() + start;
        tag[start..end].parse().unwrap()
    };

    let cell_xf_idx = |sheet_xml: &str, cell_ref: &str| -> usize {
        let needle = format!(r#"<c r="{cell_ref}""#);
        let start = sheet_xml
            .find(&needle)
            .unwrap_or_else(|| panic!("Missing cell {cell_ref} in {sheet_xml}"));
        let end = sheet_xml[start..].find('>').unwrap() + start + 1;
        let c_tag = &sheet_xml[start..end];
        parse_attr(c_tag, "s")
    };

    let cell_xf = |sheet_xml: &str, xfs: &[String], cell_ref: &str| -> String {
        xfs[cell_xf_idx(sheet_xml, cell_ref)].clone()
    };

    let sheet_live = read_zip_entry(&buf_live_1, "xl/worksheets/sheet1.xml");
    let styles_live = read_zip_entry(&buf_live_1, "xl/styles.xml");
    let shared_live = read_zip_entry(&buf_live_1, "xl/sharedStrings.xml");
    let xfs_live = extract_tags(&styles_live, "cellXfs", "xf");

    // Raw text values in sharedStrings.xml must remain unmodified (no literal leading apostrophe injected)
    assert!(shared_live.contains("<t>= $0.00</t>"));
    assert!(!shared_live.contains("'= $0.00"));
    assert!(shared_live.contains("<t>'26E</t>"));
    assert!(!shared_live.contains("''26E"));

    // Physical rows:
    // Row 2: Title ("= Integrity Checks" at B2; blank banner cells at C2, D2, E2)
    // Row 3: Subtitle ("= Tolerance checks" at B3)
    // Row 4: Title gutter
    // Row 5: Super-header ("+ Controls" at B5; blank C5)
    // Row 6: Column headers ("Check" at B6, "'26E" at C6, "Measure" at D6, "'26E" measure at E6)
    // Row 7: Section header ("- Balance Checks" at B7; blank fill at C7, D7, E7)
    // Rows 8..=16: quoted_cases (9 rows; rows 15..=16 multiline with \r\n / \n)
    // Rows 17..=22: plain_cases (6 rows)
    // Row 23: formula_eq (D23 is formula returning "= $0.00", E23 is numeric literal 0.0)
    // Row 24: formula_err (D24 is formula returning #DIV/0!, E24 is numeric literal 0.0)

    // Structural anchor cells with trigger prefixes get quotePrefix="1";
    // non-triggered headers and adjacent blank continuation cells do NOT.
    for addr in ["B2", "B3", "B5", "C6", "E6", "B7"] {
        let xf = cell_xf(&sheet_live, &xfs_live, addr);
        assert!(
            xf.contains(r#"quotePrefix="1""#),
            "Structural cell {addr} must carry quotePrefix=\"1\": {xf}"
        );
    }
    // Measure header E6 ("'26E") keeps its custom "@_)" number format alongside quotePrefix="1",
    // while non-triggered measure header D6 ("Measure") keeps its format without quotePrefix.
    let d6_xf = cell_xf(&sheet_live, &xfs_live, "D6");
    let e6_xf = cell_xf(&sheet_live, &xfs_live, "E6");
    assert!(
        d6_xf.contains(r#"applyNumberFormat="1""#) && e6_xf.contains(r#"applyNumberFormat="1""#),
        "Measure headers D6 and E6 must both preserve applyNumberFormat: D6={d6_xf}, E6={e6_xf}"
    );
    assert_eq!(
        parse_attr(&d6_xf, "numFmtId"),
        parse_attr(&e6_xf, "numFmtId"),
        "Quoted measure header E6 must share numFmtId with D6: D6={d6_xf}, E6={e6_xf}"
    );

    for addr in ["C2", "D2", "E2", "C5", "B6", "D6", "C7", "D7", "E7"] {
        let xf = cell_xf(&sheet_live, &xfs_live, addr);
        assert!(
            !xf.contains("quotePrefix"),
            "Non-triggered/blank structural cell {addr} must omit quotePrefix: {xf}"
        );
    }

    // All quoted_cases (rows 8..=16) across descriptor (B, C) and measure (D, E) columns get quotePrefix="1"
    for row_num in 8..=16 {
        for col in ['B', 'C', 'D', 'E'] {
            let addr = format!("{col}{row_num}");
            let xf = cell_xf(&sheet_live, &xfs_live, &addr);
            assert!(
                xf.contains(r#"quotePrefix="1""#),
                "Literal cell {addr} must carry quotePrefix=\"1\": {xf}"
            );
            if row_num >= 15 {
                assert!(
                    xf.contains(r#"wrapText="1""#),
                    "Multiline quoted cell {addr} must also carry wrapText=\"1\": {xf}"
                );
            }
        }
    }

    // Verify <cellXfs> deduplication across single-line quoted rows (rows 8..=14)
    let b8_xf_idx = cell_xf_idx(&sheet_live, "B8");
    let d8_xf_idx = cell_xf_idx(&sheet_live, "D8");
    for row_num in 9..=14 {
        assert_eq!(
            cell_xf_idx(&sheet_live, &format!("B{row_num}")),
            b8_xf_idx,
            "Quoted descriptor rows should reuse one deduplicated <xf>"
        );
        assert_eq!(
            cell_xf_idx(&sheet_live, &format!("D{row_num}")),
            d8_xf_idx,
            "Quoted measure rows should reuse one deduplicated <xf>"
        );
    }

    // All plain_cases (rows 17..=22) omit quotePrefix
    for row_num in 17..=22 {
        for col in ['B', 'C', 'D', 'E'] {
            let addr = format!("{col}{row_num}");
            let xf = cell_xf(&sheet_live, &xfs_live, &addr);
            assert!(
                !xf.contains("quotePrefix"),
                "Plain literal cell {addr} must omit quotePrefix: {xf}"
            );
        }
    }

    // Live formula cell D23 returning "= $0.00" and numeric literal E23 must NOT carry quotePrefix
    for addr in ["D23", "E23"] {
        let xf = cell_xf(&sheet_live, &xfs_live, addr);
        assert!(
            !xf.contains("quotePrefix"),
            "Cell {addr} must omit quotePrefix in live mode: {xf}"
        );
    }

    // Frozen values_only mode: D23 becomes literal text "= $0.00" WITH quotePrefix="1";
    // literal quoted cells (B8, D8) keep quotePrefix="1"; D24 (#DIV/0!) and E23 (0.0) omit it.
    let sheet_frozen = read_zip_entry(&buf_frozen, "xl/worksheets/sheet1.xml");
    let styles_frozen = read_zip_entry(&buf_frozen, "xl/styles.xml");
    let xfs_frozen = extract_tags(&styles_frozen, "cellXfs", "xf");

    for addr in ["B8", "D8", "D23"] {
        let xf = cell_xf(&sheet_frozen, &xfs_frozen, addr);
        assert!(
            xf.contains(r#"quotePrefix="1""#),
            "values_only text cell {addr} must carry quotePrefix=\"1\": {xf}"
        );
    }
    for addr in ["D24", "E23"] {
        let xf = cell_xf(&sheet_frozen, &xfs_frozen, addr);
        assert!(
            !xf.contains("quotePrefix"),
            "values_only cell {addr} must omit quotePrefix: {xf}"
        );
    }
}

#[test]
fn test_sheet_print_settings_openxml_scenarios_1_to_5_and_8() {
    use f8_core::model::{
        HeaderFooterSpec, PageOrientation, PaperSize, PrintFitMode, SheetLayoutSpec, SheetPrintSpec,
    };

    let make_tbl = |id: &str, title: &str| -> TableSpec {
        let mut t = TableSpec::new(id, title);
        t.columns.push(ColumnSpec::descriptor("item", "Line Item"));
        t.columns.push(ColumnSpec::new("y1").with_header("2024E"));
        t.columns.push(ColumnSpec::new("y2").with_header("2025E"));
        t.rows.push(
            RowSpec::new("rev")
                .with_cell("item", "Revenue")
                .with_cell("y1", 100.0)
                .with_cell(
                    "y2",
                    CellInput::formula(format!(r#"{id}["rev", "y1"] * 1.1"#)),
                ),
        );
        t.rows.push(
            RowSpec::new("ebitda")
                .with_cell("item", "EBITDA")
                .with_cell(
                    "y1",
                    CellInput::formula(format!(r#"{id}["rev", @col] * 0.3"#)),
                )
                .with_cell(
                    "y2",
                    CellInput::formula(format!(r#"{id}["rev", @col] * 0.3"#)),
                ),
        );
        t
    };

    // Scenario 1: Default opt-in backward compatibility (no print configured)
    let t_default = make_tbl("TDefault", "Default Sheet");
    let manifest_s1 = ProjectManifest {
        model_name: "DefaultNoPrint".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "NoPrint".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: false,
            layout: SheetLayoutSpec::Auto,
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("TDefault")]),
        }],
    };
    let buf_s1 =
        Compiler::emit_workbook(&manifest_s1, &[t_default], &EmissionOptions::default()).unwrap();
    let wb_s1 = read_zip_entry(&buf_s1, "xl/workbook.xml");
    let sh_s1 = read_zip_entry(&buf_s1, "xl/worksheets/sheet1.xml");
    assert!(!wb_s1.contains("_xlnm.Print_Area"));
    assert!(!sh_s1.contains("<pageSetUpPr"));
    assert!(!sh_s1.contains("<pageSetup"));
    assert!(!sh_s1.contains("<printOptions"));
    assert!(!sh_s1.contains("<headerFooter"));
    assert!(!sh_s1.contains("<rowBreaks"));
    // Default Excel margins preserved unchanged
    assert!(sh_s1.contains(r#"<pageMargins left="0.7" right="0.7" top="0.75" bottom="0.75" header="0.3" footer="0.3"/>"#));

    // Scenario 2: Landscape A4 fit_width with 3 bands, page_break_before on Band 1 & Band 2,
    // trimmed Print_Area, tested across both Auto and Grid layout modes
    for layout_spec in [
        SheetLayoutSpec::Auto,
        SheetLayoutSpec::grid_with_col_width(12.0),
    ] {
        let t1 = make_tbl("S2B1", "Band 1");
        let t2 = make_tbl("S2B2", "Band 2");
        let t3 = make_tbl("S2B3", "Band 3");
        let manifest_s2 = ProjectManifest {
            model_name: "Scenario2".to_string(),
            theme: Default::default(),
            sheets: vec![SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: false,
                layout: layout_spec,
                print: Some(SheetPrintSpec::new(PaperSize::A4)),
                bands: vec![
                    BandSpec::single("S2B1"),
                    BandSpec::single("S2B2").with_page_break_before(true),
                    BandSpec::single("S2B3").with_page_break_before(true),
                ],
            }],
        };
        let compiled_s2 = Compiler::compile(&manifest_s2, &[t1, t2, t3]).unwrap();
        let sl_s2 = compiled_s2
            .layout_map
            .get_sheet(&SheetId::from("Model"))
            .unwrap();
        let print_s2 = sl_s2.print.as_ref().unwrap();
        let area_s2 = print_s2.print_area.unwrap();
        let r1 = print_s2.row_breaks[0];
        let r2 = print_s2.row_breaks[1];
        let last_col = f8_core::col_to_excel_col(area_s2.last_col);
        let last_row = area_s2.last_row + 1;

        let buf_s2 = compiled_s2
            .emit_workbook(&EmissionOptions::default())
            .unwrap();
        let wb_s2 = read_zip_entry(&buf_s2, "xl/workbook.xml");
        let sh_s2 = read_zip_entry(&buf_s2, "xl/worksheets/sheet1.xml");

        assert!(
            wb_s2.contains(&format!(
                r#"<definedName name="_xlnm.Print_Area" localSheetId="0">Model!$B$2:${last_col}${last_row}</definedName>"#
            )),
            "Unexpected workbook.xml Print_Area: {wb_s2}"
        );
        assert!(sh_s2.contains(r#"<pageSetUpPr fitToPage="1"/>"#));
        assert!(sh_s2.contains(r#"<printOptions horizontalCentered="1"/>"#));
        assert!(sh_s2.contains(
            r#"<pageMargins left="0.4" right="0.4" top="0.6" bottom="0.6" header="0.25" footer="0.25"/>"#
        ));
        assert!(
            sh_s2.contains(r#"<pageSetup paperSize="9" fitToHeight="0" orientation="landscape""#),
            "Unexpected pageSetup: {sh_s2}"
        );
        assert!(!sh_s2.contains("fitToWidth="));
        assert!(
            sh_s2.contains(&format!(
                r#"<rowBreaks count="2" manualBreakCount="2"><brk id="{r1}" max="16383" man="1"/><brk id="{r2}" max="16383" man="1"/></rowBreaks>"#
            )),
            "Unexpected rowBreaks: {sh_s2}"
        );
        assert!(!sh_s2.contains("view="));
        assert!(!sh_s2.contains("<headerFooter"));
    }

    // Scenario 3: Portrait Letter fit_page with center_horizontally: false
    let t_s3 = make_tbl("S3T1", "One Page Dashboard");
    let manifest_s3 = ProjectManifest {
        model_name: "Scenario3".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Dashboard".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: false,
            layout: SheetLayoutSpec::Auto,
            print: Some(SheetPrintSpec {
                paper_size: PaperSize::Letter,
                orientation: PageOrientation::Portrait,
                fit: PrintFitMode::FitPage,
                center_horizontally: false,
                header: None,
                footer: None,
            }),
            bands: SheetSpec::stacked_bands([TableId::from("S3T1")]),
        }],
    };
    let buf_s3 =
        Compiler::emit_workbook(&manifest_s3, &[t_s3], &EmissionOptions::default()).unwrap();
    let sh_s3 = read_zip_entry(&buf_s3, "xl/worksheets/sheet1.xml");
    assert!(sh_s3.contains(r#"<pageSetUpPr fitToPage="1"/>"#));
    assert!(
        sh_s3.contains(r#"<pageSetup paperSize="1" orientation="portrait""#),
        "Unexpected pageSetup in Scenario 3: {sh_s3}"
    );
    assert!(!sh_s3.contains("fitToWidth="));
    assert!(!sh_s3.contains("fitToHeight="));
    assert!(sh_s3.contains(
        r#"<pageMargins left="0.4" right="0.4" top="0.6" bottom="0.6" header="0.25" footer="0.25"/>"#
    ));
    assert!(!sh_s3.contains("<printOptions"));
    assert!(!sh_s3.contains("<headerFooter"));

    // Scenario 4 & Scenario 8 (Determinism + values_only parity):
    // Structured headers & footers with token expansion, escaping, and theme typography
    let t_s4 = make_tbl("S4T1", "Financial Summary");
    let t_s4_b2 = make_tbl("S4T2", "Supporting Schedule");
    let manifest_s4 = ProjectManifest {
        model_name: "Acme & Co".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Summary".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: false,
            layout: SheetLayoutSpec::Auto,
            print: Some(SheetPrintSpec {
                paper_size: PaperSize::A4,
                orientation: PageOrientation::Landscape,
                fit: PrintFitMode::FitWidth,
                center_horizontally: true,
                header: Some(HeaderFooterSpec {
                    left: Some("{model}".to_string()),
                    center: Some("2026 Forecast {{Rev}}".to_string()),
                    right: Some("{sheet}".to_string()),
                }),
                footer: Some(HeaderFooterSpec {
                    left: Some("M&A Confidential".to_string()),
                    center: None,
                    right: Some("Page {page} of {pages} ({date})".to_string()),
                }),
            }),
            bands: vec![
                BandSpec::single("S4T1"),
                BandSpec::single("S4T2").with_page_break_before(true),
            ],
        }],
    };
    let compiled_s4 = Compiler::compile(&manifest_s4, &[t_s4, t_s4_b2]).unwrap();
    let buf_s4_a = compiled_s4
        .emit_workbook(&EmissionOptions::default())
        .unwrap();
    let buf_s4_b = compiled_s4
        .emit_workbook(&EmissionOptions::default())
        .unwrap();
    assert_eq!(
        buf_s4_a, buf_s4_b,
        "Repeated emission must be byte-for-byte deterministic"
    );

    let buf_s4_frozen = compiled_s4
        .emit_workbook(&EmissionOptions::default().with_values_only(true))
        .unwrap();

    for (label, buf) in [("live", &buf_s4_a), ("values_only", &buf_s4_frozen)] {
        let sh = read_zip_entry(buf, "xl/worksheets/sheet1.xml");
        let wb = read_zip_entry(buf, "xl/workbook.xml");
        assert!(
            wb.contains("_xlnm.Print_Area"),
            "{label}: missing Print_Area"
        );
        assert!(
            sh.contains(r#"<pageSetUpPr fitToPage="1"/>"#),
            "{label}: missing pageSetUpPr"
        );
        assert!(
            sh.contains(r#"<printOptions horizontalCentered="1"/>"#),
            "{label}: missing printOptions"
        );
        assert!(
            sh.contains(r#"<pageSetup paperSize="9" fitToHeight="0" orientation="landscape""#),
            "{label}: missing pageSetup"
        );
        assert!(
            sh.contains(r#"<rowBreaks count="1" manualBreakCount="1">"#),
            "{label}: missing rowBreaks"
        );
        assert!(
            sh.contains(
                r#"<headerFooter scaleWithDoc="0"><oddHeader>&amp;L&amp;8&amp;K525252&amp;"Aptos,Regular"Acme &amp;&amp; Co&amp;C&amp;8&amp;K525252&amp;"Aptos,Regular"2026 Forecast {Rev}&amp;R&amp;8&amp;K525252&amp;"Aptos,Regular"&amp;A</oddHeader><oddFooter>&amp;L&amp;8&amp;K525252&amp;"Aptos,Regular"M&amp;&amp;A Confidential&amp;R&amp;8&amp;K525252&amp;"Aptos,Regular"Page &amp;P of &amp;N (&amp;D)</oddFooter></headerFooter>"#
            ),
            "{label}: unexpected headerFooter XML in {sh}"
        );
    }
    let sh_live = read_zip_entry(&buf_s4_a, "xl/worksheets/sheet1.xml");
    let sh_frozen = read_zip_entry(&buf_s4_frozen, "xl/worksheets/sheet1.xml");
    assert!(sh_live.contains("<f>"));
    assert!(!sh_frozen.contains("<f>"));

    // Scenario 5: Empty sheet with print configured
    let manifest_s5 = ProjectManifest {
        model_name: "EmptyPrintSheet".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Empty".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: false,
            layout: SheetLayoutSpec::Auto,
            print: Some(SheetPrintSpec::new(PaperSize::A4)),
            bands: Vec::new(),
        }],
    };
    let buf_s5 = Compiler::emit_workbook(&manifest_s5, &[], &EmissionOptions::default()).unwrap();
    let wb_s5 = read_zip_entry(&buf_s5, "xl/workbook.xml");
    let sh_s5 = read_zip_entry(&buf_s5, "xl/worksheets/sheet1.xml");
    assert!(!wb_s5.contains("_xlnm.Print_Area"));
    assert!(!sh_s5.contains("<rowBreaks"));
    assert!(!sh_s5.contains("<headerFooter"));
    assert!(sh_s5.contains(r#"<pageSetUpPr fitToPage="1"/>"#));
    assert!(sh_s5.contains(r#"<printOptions horizontalCentered="1"/>"#));
    assert!(sh_s5.contains(r#"<pageSetup paperSize="9" fitToHeight="0" orientation="landscape""#));
    assert!(sh_s5.contains(
        r#"<pageMargins left="0.4" right="0.4" top="0.6" bottom="0.6" header="0.25" footer="0.25"/>"#
    ));
}

#[test]
fn test_excel_2023_office_theme_defaults_emitted() {
    let manifest = ProjectManifest {
        model_name: "Theme2023Check".to_string(),
        theme: Default::default(),
        sheets: vec![SheetSpec {
            name: "Summary".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: false,
            layout: Default::default(),
            print: None,
            bands: SheetSpec::stacked_bands([TableId::from("T1")]),
        }],
    };
    let mut table = TableSpec::new("T1", "Summary Table");
    table.columns = vec![
        ColumnSpec::descriptor("item", "Line Item"),
        ColumnSpec::new("y1").with_header("2026E"),
    ];
    table.rows = vec![
        RowSpec::new("rev")
            .with_cell("item", "Revenue")
            .with_cell("y1", 100.0),
    ];

    let compiled = Compiler::compile(&manifest, &[table]).unwrap();
    let buf_a = compiled.emit_workbook(&EmissionOptions::default()).unwrap();
    let buf_b = compiled.emit_workbook(&EmissionOptions::default()).unwrap();
    assert_eq!(buf_a, buf_b, "Emission must remain byte-deterministic");

    let wb_xml = read_zip_entry(&buf_a, "xl/workbook.xml");
    assert!(
        wb_xml.contains(r#"defaultThemeVersion="202300""#),
        "Expected Excel 2023 defaultThemeVersion in workbook.xml: {wb_xml}"
    );

    let theme_xml = read_zip_entry(&buf_a, "xl/theme/theme1.xml");
    assert!(
        theme_xml.contains(r#"<a:latin typeface="Aptos Display""#),
        "Expected Aptos Display majorFont in theme1.xml"
    );
    assert!(
        theme_xml.contains(r#"<a:latin typeface="Aptos Narrow""#),
        "Expected Aptos Narrow minorFont in theme1.xml"
    );
    assert!(
        !theme_xml.contains(r#"typeface="Calibri""#),
        "Legacy Calibri theme font must not remain in theme1.xml"
    );

    let styles_xml = read_zip_entry(&buf_a, "xl/styles.xml");
    let first_font_start = styles_xml
        .find("<font>")
        .expect("styles.xml must contain Normal fontId=0");
    let first_font_end = styles_xml[first_font_start..]
        .find("</font>")
        .expect("styles.xml fontId=0 must close")
        + first_font_start;
    let normal_font = &styles_xml[first_font_start..first_font_end];
    assert!(
        normal_font.contains(r#"<name val="Aptos Narrow"/>"#)
            && normal_font.contains(r#"<sz val="11"/>"#)
            && normal_font.contains(r#"<scheme val="minor"/>"#),
        "Expected Normal style fontId=0 to be Aptos Narrow 11pt minor scheme, got: {normal_font}"
    );
    assert!(
        !styles_xml.contains(r#"<name val="Calibri"/>"#),
        "Legacy Calibri must not appear in default theme styles.xml: {styles_xml}"
    );

    let sheet_xml = read_zip_entry(&buf_a, "xl/worksheets/sheet1.xml");
    assert!(
        sheet_xml.contains(r#"<sheetFormatPr defaultRowHeight="15"/>"#),
        "Excel 2023 theme must preserve 15pt defaultRowHeight: {sheet_xml}"
    );
}
