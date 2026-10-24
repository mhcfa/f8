#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Read;

use f8_core::compiler::emission::{EmissionError, EmissionOptions, FormatCache, WorkbookEmitter};
use f8_core::compiler::ir::id::CellId;
use f8_core::compiler::layout::LayoutPlanner;
use f8_core::compiler::lowering::FormulaLowerer;
use f8_core::compiler::{Compiler, ir};
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
    let mut table = TableSpec::new("HeaderTable", "Table Title Banner");
    table.subtitle = Some("($ in thousands, except per share data)".to_string());
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
