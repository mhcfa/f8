#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Read;
use std::path::PathBuf;

use finx::compiler::emission::{EmissionError, EmissionOptions, FormatCache, WorkbookEmitter};
use finx::compiler::ir::id::CellId;
use finx::compiler::layout::LayoutPlanner;
use finx::compiler::lowering::FormulaLowerer;
use finx::compiler::{Compiler, ir};
use finx::model::id::{SheetId, TableId};
use finx::model::manifest::{FreezePanes, ProjectManifest, SheetSpec};
use finx::model::table::{
    CellInput, ColumnSpec, DetailedCellInput, RowSpec, SectionHeaderRowSpec, TableSpec,
    ValueLiteral,
};
use finx::model::theme::Theme;

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
        theme: "corporate_navy".to_string(),
        sheets: vec![
            SheetSpec {
                name: "Model".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![PathBuf::from("tables/IncomeStatement.json")],
            },
            SheetSpec {
                name: "Summary".to_string(),
                tab_color: None,
                freeze_panes: None,
                show_gridlines: true,
                tables: vec![PathBuf::from("tables/SummaryTable.json")],
            },
        ],
    };

    let theme = Theme::corporate_navy();
    let options = EmissionOptions {
        title: Some("Corporate Financial Model".to_string()),
        author: Some("Acme Corp".to_string()),
        values_only: false,
    };

    let buffer =
        Compiler::emit_workbook(&manifest, &[is_table, sum_table], &theme, None, &options).unwrap();

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
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "MetricsSheet".to_string(),
            tab_color: Some("#1B365D".to_string()),
            freeze_panes: Some(FreezePanes { row: 3, col: 2 }),
            show_gridlines: false,
            tables: vec![PathBuf::from("tables/LayoutTable.json")],
        }],
    };

    let theme = Theme::corporate_navy();
    let options = EmissionOptions::default();

    let buffer = Compiler::emit_workbook(&manifest, &[table], &theme, None, &options).unwrap();

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
    let mut table = TableSpec::new("HeaderTable", "Table Title Banner");
    table.subtitle = Some("($ in thousands, except per share data)".to_string());
    table
        .columns
        .push(ColumnSpec::descriptor("line_item", "Line Item"));
    table
        .columns
        .push(ColumnSpec::new("2024E").with_headers(["Forecast Horizon", "2024E"]));
    table
        .columns
        .push(ColumnSpec::new("2025E").with_headers(["Forecast Horizon", "2025E"]));
    table
        .columns
        .push(ColumnSpec::new("2026E").with_headers(["Forecast Horizon", "2026E"]));

    table.rows.push(RowSpec::new("revenue"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "HeaderModel".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Headers".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/HeaderTable.json")],
        }],
    };

    let theme = Theme::corporate_navy();
    let buffer = Compiler::emit_workbook(
        &manifest,
        &[table],
        &theme,
        None,
        &EmissionOptions::default(),
    )
    .unwrap();

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
    table
        .columns
        .push(ColumnSpec::new("2024E").with_headers(["Single Column Tier", "2024E"]));

    table.rows.push(RowSpec::new("revenue"));
    populate_table_cells(&mut table);

    let manifest = ProjectManifest {
        model_name: "SingleGroupModel".to_string(),
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "SingleGroup".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/SingleGroupTable.json")],
        }],
    };

    let theme = Theme::corporate_navy();

    // Must compile without error (specifically avoiding XlsxError::MergeRangeSingleCell)
    let buffer = Compiler::emit_workbook(
        &manifest,
        &[table],
        &theme,
        None,
        &EmissionOptions::default(),
    )
    .unwrap();

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
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/FormulaTable.json")],
        }],
    };

    let theme = Theme::corporate_navy();
    let evaluated = Compiler::evaluate(&manifest, std::slice::from_ref(&table)).unwrap();

    let buffer = Compiler::emit_workbook(
        &manifest,
        std::slice::from_ref(&table),
        &theme,
        Some(&evaluated),
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
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "FreezeSheet".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/FreezeTable.json")],
        }],
    };

    let theme = Theme::corporate_navy();

    // 1. Test with automatic fallback evaluation (evaluated: None)
    let options = EmissionOptions {
        values_only: true,
        title: None,
        author: None,
    };
    let buffer = Compiler::emit_workbook(
        &manifest,
        std::slice::from_ref(&table),
        &theme,
        None,
        &options,
    )
    .unwrap();

    let sheet_xml = read_zip_entry(&buffer, "xl/worksheets/sheet1.xml");

    // In values-only mode, NO formula definitions (<f>) should exist in the sheet
    assert!(!sheet_xml.contains("<f>"));

    // Concrete frozen numbers (250 and 500) are emitted directly as <v>
    assert!(sheet_xml.contains("<v>250</v>"));
    assert!(sheet_xml.contains("<v>500</v>"));

    // 2. Test with explicitly supplied evaluated model
    let evaluated = Compiler::evaluate(&manifest, std::slice::from_ref(&table)).unwrap();
    let buffer2 = Compiler::emit_workbook(
        &manifest,
        std::slice::from_ref(&table),
        &theme,
        Some(&evaluated),
        &options,
    )
    .unwrap();

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

    let role = finx::compiler::layout::StyleRole::data();

    // Measure input cell (is_input: true, is_descriptor: false)
    let input_key =
        finx::compiler::emission::CellFormatKey::new(role, None, None, None, true, false, 0);
    let _input_fmt = format_cache.get_cell_format(&input_key).unwrap();

    // Formula cell (is_input: false, is_descriptor: false)
    let formula_key =
        finx::compiler::emission::CellFormatKey::new(role, None, None, None, false, false, 0);
    let _formula_fmt = format_cache.get_cell_format(&formula_key).unwrap();

    // Descriptor cell (is_descriptor: true)
    let desc_key =
        finx::compiler::emission::CellFormatKey::new(role, None, None, None, false, true, 0);
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
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Outlines".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/OutlineTable.json")],
        }],
    };

    let theme = Theme::corporate_navy();
    let buffer = Compiler::emit_workbook(
        &manifest,
        &[table.clone()],
        &theme,
        None,
        &EmissionOptions::default(),
    )
    .unwrap();

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
    let collapsible_buffer = Compiler::emit_workbook(
        &manifest,
        &[collapsible_table],
        &theme,
        None,
        &EmissionOptions::default(),
    )
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
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "Sheet1".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/DiskTable.json")],
        }],
    };

    let theme = Theme::corporate_navy();
    let temp_dir = std::env::temp_dir();
    let target_file = temp_dir.join(format!("finx_test_{}.xlsx", std::process::id()));

    // Clean up if left over
    if target_file.exists() {
        let _ = std::fs::remove_file(&target_file);
    }

    let save_result = Compiler::save_workbook(
        &manifest,
        &[table],
        &theme,
        None,
        &EmissionOptions::default(),
        &target_file,
    );

    assert!(save_result.is_ok());
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
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "ModelSheet".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/DiagTable.json")],
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
        &[table],
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
        theme: "corporate_navy".to_string(),
        sheets: vec![SheetSpec {
            name: "AuditSheet".to_string(),
            tab_color: None,
            freeze_panes: None,
            show_gridlines: true,
            tables: vec![PathBuf::from("tables/NoteTable.json")],
        }],
    };

    let theme = Theme::corporate_navy();
    let buffer = Compiler::emit_workbook(
        &manifest,
        &[table],
        &theme,
        None,
        &EmissionOptions::default(),
    )
    .unwrap();

    let comments_xml = read_zip_entry(&buffer, "xl/comments1.xml");
    assert!(comments_xml.contains("Audit checked by Senior Associate"));
}
