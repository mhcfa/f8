//! Dedicated end-to-end integration test suite for OpenXML workbook emission and export.
//!
//! Validates that complete institutional financial models (Three-Statement and LBO)
//! compile through all six compiler passes into valid OpenXML `.xlsx` packages.
//!
//! - `test_models_emit_in_memory`: Runs during normal `cargo test` in RAM with zero disk writes.
//! - `test_export_xlsx_to_disk`: Marked `#[ignore]`, only executed when explicitly invoked.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::io::Read;
use std::path::Path;

use finx::Compiler;
use finx::compiler::emission::EmissionOptions;
use finx::model::theme::Theme;

mod fixtures;

fn read_zip_entry(buffer: &[u8], name: &str) -> String {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(buffer)).unwrap();
    let mut file = archive
        .by_name(name)
        .unwrap_or_else(|_| panic!("Failed to find zip entry: {name}"));
    let mut content = String::new();
    file.read_to_string(&mut content).unwrap();
    content
}

fn has_zip_entry(buffer: &[u8], name: &str) -> bool {
    let mut archive = match zip::ZipArchive::new(std::io::Cursor::new(buffer)) {
        Ok(a) => a,
        Err(_) => return false,
    };
    archive.by_name(name).is_ok()
}

// ----------------------------------------------------------------------------
// Scenario 1: In-Memory End-to-End Workbook Emission (Default CI Test)
// ----------------------------------------------------------------------------
#[test]
fn test_models_emit_in_memory() {
    let theme_navy = Theme::corporate_navy();
    let theme_green = Theme::classic_green();

    // 1. Three-Statement Financial Model
    let (ts_manifest, ts_tables) = fixtures::three_statement::build_three_statement_model();
    let ts_evaluated = Compiler::evaluate(&ts_manifest, &ts_tables)
        .expect("Three-statement model must evaluate cleanly");

    let live_options = EmissionOptions {
        title: Some("Three-Statement Financial Model".to_string()),
        author: Some("Finx Institutional Engine".to_string()),
        values_only: false,
    };

    let ts_buffer = Compiler::emit_workbook(
        &ts_manifest,
        &ts_tables,
        &theme_navy,
        Some(&ts_evaluated),
        &live_options,
    )
    .expect("Three-statement model must emit to OpenXML buffer");

    // Verify ZIP magic bytes
    assert!(!ts_buffer.is_empty());
    assert_eq!(&ts_buffer[0..4], &[0x50, 0x4B, 0x03, 0x04]);

    // Verify OpenXML package structure
    assert!(has_zip_entry(&ts_buffer, "[Content_Types].xml"));
    assert!(has_zip_entry(&ts_buffer, "xl/styles.xml"));
    assert!(has_zip_entry(&ts_buffer, "xl/workbook.xml"));
    assert!(has_zip_entry(&ts_buffer, "xl/worksheets/sheet1.xml"));
    assert!(has_zip_entry(&ts_buffer, "docProps/core.xml"));

    let ts_workbook_xml = read_zip_entry(&ts_buffer, "xl/workbook.xml");
    assert!(ts_workbook_xml.contains("name=\"Model\""));

    let ts_sheet_xml = read_zip_entry(&ts_buffer, "xl/worksheets/sheet1.xml");
    // Verify lowered formulas and cached evaluation results
    assert!(ts_sheet_xml.contains("<f>"));
    assert!(ts_sheet_xml.contains("<v>100</v>") || ts_sheet_xml.contains("<v>115</v>"));

    // Verify Core document properties metadata
    let ts_core_xml = read_zip_entry(&ts_buffer, "docProps/core.xml");
    assert!(ts_core_xml.contains("Three-Statement Financial Model"));
    assert!(ts_core_xml.contains("Finx Institutional Engine"));

    // 2. Three-Statement Values-Only Freezing Mode
    let freeze_options = EmissionOptions {
        title: Some("Frozen Baseline".to_string()),
        author: Some("Audit Engine".to_string()),
        values_only: true,
    };

    let ts_frozen = Compiler::emit_workbook(
        &ts_manifest,
        &ts_tables,
        &theme_green,
        Some(&ts_evaluated),
        &freeze_options,
    )
    .expect("Frozen model must emit to buffer");

    let ts_frozen_sheet_xml = read_zip_entry(&ts_frozen, "xl/worksheets/sheet1.xml");
    assert!(
        !ts_frozen_sheet_xml.contains("<f>"),
        "Values-only mode must not contain formula tags <f>"
    );
    assert!(
        ts_frozen_sheet_xml.contains("<v>"),
        "Values-only mode must preserve cached concrete values <v>"
    );

    // 3. Institutional LBO Model (Multi-sheet, cross-sheet formulas, day-count dates, returns)
    let (lbo_manifest, lbo_tables) = fixtures::lbo::build_lbo_model("base");
    let lbo_evaluated =
        Compiler::evaluate(&lbo_manifest, &lbo_tables).expect("LBO model must evaluate cleanly");

    let lbo_options = EmissionOptions {
        title: Some("Project Titan LBO Model".to_string()),
        author: Some("Sponsor Private Equity".to_string()),
        values_only: false,
    };

    let lbo_buffer = Compiler::emit_workbook(
        &lbo_manifest,
        &lbo_tables,
        &theme_navy,
        Some(&lbo_evaluated),
        &lbo_options,
    )
    .expect("LBO model must emit to OpenXML buffer");

    assert_eq!(&lbo_buffer[0..4], &[0x50, 0x4B, 0x03, 0x04]);

    // Verify both sheets exist
    assert!(has_zip_entry(&lbo_buffer, "xl/worksheets/sheet1.xml"));
    assert!(has_zip_entry(&lbo_buffer, "xl/worksheets/sheet2.xml"));

    let lbo_workbook_xml = read_zip_entry(&lbo_buffer, "xl/workbook.xml");
    assert!(lbo_workbook_xml.contains("name=\"Summary\""));
    assert!(lbo_workbook_xml.contains("name=\"Model\""));

    let lbo_sheet1_xml = read_zip_entry(&lbo_buffer, "xl/worksheets/sheet1.xml");
    let lbo_sheet2_xml = read_zip_entry(&lbo_buffer, "xl/worksheets/sheet2.xml");

    // Verify cross-sheet references and financial functions in formulas
    let combined_sheets = format!("{lbo_sheet1_xml}\n{lbo_sheet2_xml}");
    assert!(
        combined_sheets.contains("&apos;Summary&apos;!")
            || combined_sheets.contains("'Summary'!")
            || combined_sheets.contains("Summary!"),
        "Expected cross-sheet reference to Summary sheet"
    );
    assert!(
        combined_sheets.contains("XLOOKUP")
            || combined_sheets.contains("EDATE")
            || combined_sheets.contains("XIRR"),
        "Expected lowered financial functions in formula tags"
    );
}

// ----------------------------------------------------------------------------
// Scenario 2: Physical Disk File Export (Ignored by Default in cargo test)
// Run with: cargo test --test export_integration_test -- --ignored
// ----------------------------------------------------------------------------
#[test]
#[ignore = "Generates physical .xlsx files on disk. Run with: cargo test --test export_integration_test -- --ignored"]
fn test_export_xlsx_to_disk() {
    let output_dir = Path::new("target/test-output");
    std::fs::create_dir_all(output_dir).expect("Failed to create target/test-output directory");

    let ts_file = output_dir.join("three_statement_model.xlsx");
    let lbo_file = output_dir.join("institutional_lbo_model.xlsx");

    // Clean up preexisting artifacts
    let _ = std::fs::remove_file(&ts_file);
    let _ = std::fs::remove_file(&lbo_file);

    let theme = Theme::corporate_navy();

    // 1. Export Three-Statement Model
    let (ts_manifest, ts_tables) = fixtures::three_statement::build_three_statement_model();
    let ts_options = EmissionOptions {
        title: Some("Three-Statement Financial Model".to_string()),
        author: Some("Finx Institutional Engine".to_string()),
        values_only: false,
    };

    Compiler::save_workbook(
        &ts_manifest,
        &ts_tables,
        &theme,
        None, // Automatically evaluates during compilation
        &ts_options,
        &ts_file,
    )
    .expect("Failed to save Three-Statement model to disk");

    assert!(ts_file.exists());
    let ts_bytes = std::fs::read(&ts_file).expect("Failed to read saved Three-Statement file");
    assert!(ts_bytes.len() > 4000, "Expected non-trivial file size");
    assert_eq!(&ts_bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    assert!(has_zip_entry(&ts_bytes, "xl/workbook.xml"));
    let ts_sheet1_xml = read_zip_entry(&ts_bytes, "xl/worksheets/sheet1.xml");
    assert!(ts_sheet1_xml.contains("<f>"), "Formulas must be emitted");
    assert!(
        ts_sheet1_xml.contains("<v>114.99") || ts_sheet1_xml.contains("<v>115</v>"),
        "Formulas must have precalculated values cached in <v> tag when evaluated is None"
    );

    // 2. Export Institutional LBO Model
    let (lbo_manifest, lbo_tables) = fixtures::lbo::build_lbo_model("base");
    let lbo_options = EmissionOptions {
        title: Some("Project Titan LBO Model".to_string()),
        author: Some("Sponsor Private Equity".to_string()),
        values_only: false,
    };

    Compiler::save_workbook(
        &lbo_manifest,
        &lbo_tables,
        &theme,
        None, // Automatically evaluates during compilation
        &lbo_options,
        &lbo_file,
    )
    .expect("Failed to save LBO model to disk");

    assert!(lbo_file.exists());
    let lbo_bytes = std::fs::read(&lbo_file).expect("Failed to read saved LBO file");
    assert!(lbo_bytes.len() > 6000, "Expected non-trivial file size");
    assert_eq!(&lbo_bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    assert!(has_zip_entry(&lbo_bytes, "xl/workbook.xml"));
    assert!(has_zip_entry(&lbo_bytes, "xl/worksheets/sheet2.xml"));

    let lbo_sheet1_xml = read_zip_entry(&lbo_bytes, "xl/worksheets/sheet1.xml");
    assert!(
        lbo_sheet1_xml.contains("<f>"),
        "LBO formulas must be emitted"
    );
    assert!(
        lbo_sheet1_xml.contains("<v>500</v>"),
        "LBO entry EV must have precalculated 500 cached in <v> tag"
    );

    println!("\nExported test workbooks successfully:");
    println!("  - Three-Statement Model: {}", ts_file.display());
    println!("  - Institutional LBO Model: {}", lbo_file.display());
}
