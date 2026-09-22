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

    // 1. Three-Statement Financial Model
    let (ts_manifest, ts_tables) = fixtures::three_statement::build_three_statement_model();
    let ts_evaluated = Compiler::evaluate(&ts_manifest, &ts_tables)
        .expect("Three-statement model must evaluate cleanly");

    let live_options = EmissionOptions {
        title: Some("Three-Statement Financial Model".to_string()),
        author: Some("Finx Institutional Engine".to_string()),
        values_only: false,
        omit_formula_cache: false,
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
    // Verify lowered formulas, cached evaluation results, and explicit row gutters
    assert!(ts_sheet_xml.contains("<f>"));
    assert!(ts_sheet_xml.contains("<v>100</v>") || ts_sheet_xml.contains("<v>115</v>"));
    // Explicit row gutters leave unstyled rows unpopulated in sheetData
    assert!(
        ts_sheet_xml.contains("r=\"3\"") && !ts_sheet_xml.contains("r=\"4\""),
        "Expected blank gutter row between subtitle and table data"
    );

    // Verify Core document properties metadata
    let ts_core_xml = read_zip_entry(&ts_buffer, "docProps/core.xml");
    assert!(ts_core_xml.contains("Three-Statement Financial Model"));
    assert!(ts_core_xml.contains("Finx Institutional Engine"));

    // 2. Three-Statement Values-Only Freezing Mode
    let freeze_options = EmissionOptions {
        title: Some("Frozen Baseline".to_string()),
        author: Some("Audit Engine".to_string()),
        values_only: true,
        omit_formula_cache: false,
    };

    let ts_frozen = Compiler::emit_workbook(
        &ts_manifest,
        &ts_tables,
        &theme_navy,
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
        omit_formula_cache: false,
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
            && combined_sheets.contains("SWITCH")
            && combined_sheets.contains("PMT")
            && combined_sheets.contains("XIRR"),
        "Expected lowered financial and logical functions (XLOOKUP, SWITCH, PMT, XIRR) in formula tags"
    );

    let lbo_shared_strings = read_zip_entry(&lbo_buffer, "xl/sharedStrings.xml");
    assert!(
        lbo_shared_strings.contains("Sources of Financing")
            || lbo_shared_strings.contains("Sources &amp; Uses"),
        "Expected 7-column Sources & Uses table in shared strings"
    );
    assert!(
        lbo_shared_strings.contains("Project Titan LBO"),
        "Expected 3-tier super-header banner on Model sheet in shared strings"
    );

    // 4. Institutional Styling & Formatting Showcase Model
    let (showcase_manifest, showcase_tables) =
        fixtures::styling_showcase::build_styling_showcase_model();
    let showcase_evaluated = Compiler::evaluate(&showcase_manifest, &showcase_tables)
        .expect("Styling showcase model must evaluate cleanly");

    let showcase_options = EmissionOptions {
        title: Some("Finx Institutional Styling Showcase".to_string()),
        author: Some("Finx Design System".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let showcase_buffer = Compiler::emit_workbook(
        &showcase_manifest,
        &showcase_tables,
        &theme_navy,
        Some(&showcase_evaluated),
        &showcase_options,
    )
    .expect("Styling showcase model must emit to OpenXML buffer");

    assert_eq!(&showcase_buffer[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    assert!(has_zip_entry(&showcase_buffer, "xl/worksheets/sheet1.xml"));
    assert!(has_zip_entry(&showcase_buffer, "xl/worksheets/sheet2.xml"));

    let showcase_workbook_xml = read_zip_entry(&showcase_buffer, "xl/workbook.xml");
    assert!(showcase_workbook_xml.contains("name=\"Styling_Showcase\""));
    assert!(showcase_workbook_xml.contains("name=\"Freeze_Panes_Demo\""));

    let sc_sheet1_xml = read_zip_entry(&showcase_buffer, "xl/worksheets/sheet1.xml");
    assert!(
        sc_sheet1_xml.contains("<f>"),
        "Showcase formulas must be emitted"
    );
    assert!(
        sc_sheet1_xml.contains("<v>"),
        "Showcase cached values must be emitted"
    );
    let sc_shared_strings = read_zip_entry(&showcase_buffer, "xl/sharedStrings.xml");
    assert!(
        sc_shared_strings.contains("Commercial Operations"),
        "Expected 3-tier super-header in styling showcase shared strings"
    );
    assert!(
        sc_sheet1_xml.contains("width=\"3.71") && sc_sheet1_xml.contains("min=\"1\" max=\"1\""),
        "Expected explicit margin column gutter in styling showcase"
    );
    // Explicit row gutters leave unstyled rows unpopulated in sheetData
    assert!(
        sc_sheet1_xml.contains("r=\"3\"") && !sc_sheet1_xml.contains("r=\"4\""),
        "Expected blank gutter row between subtitle and table headers"
    );

    let sc_styles_xml = read_zip_entry(&showcase_buffer, "xl/styles.xml");
    assert!(
        sc_styles_xml.contains("style=\"double\""),
        "Expected double border in styles.xml"
    );
    assert!(
        sc_styles_xml.contains("style=\"thin\""),
        "Expected thin border in styles.xml"
    );

    // 5. Excel Parity Verification Suite (All 43 Standard Functions)
    let (parity_manifest, parity_tables) = fixtures::parity_suite::build_parity_model();
    let parity_evaluated = Compiler::evaluate(&parity_manifest, &parity_tables)
        .expect("Parity model must compile and evaluate cleanly");

    let parity_errors = parity_evaluated.find_errors();
    assert!(
        parity_errors.is_empty(),
        "Expected zero evaluation errors across all 43 functions in parity suite, found: {parity_errors:?}"
    );

    for table in &parity_tables[1..] {
        for row in &table.rows {
            if let finx::model::table::RowSpec::Data(d) = row {
                let status = parity_evaluated.get_value(
                    &table.id,
                    &d.id,
                    &finx::model::id::ColId::from("parity_status"),
                );
                assert_eq!(
                    status,
                    Some(&finx::value::CellValue::Text("PASS".to_string())),
                    "Function '{}' in table '{}' failed parity check",
                    d.id,
                    table.id
                );
            }
        }
    }

    // Verify uncached formula emission (<f> present, <v> omitted for formulas)
    let uncached_options = EmissionOptions {
        title: Some("Excel Parity Verification (Uncached)".to_string()),
        author: Some("Finx Parity Engine".to_string()),
        values_only: false,
        omit_formula_cache: true,
    };

    let uncached_buffer = Compiler::emit_workbook(
        &parity_manifest,
        &parity_tables,
        &theme_navy,
        Some(&parity_evaluated),
        &uncached_options,
    )
    .expect("Uncached parity model must emit to OpenXML buffer");

    assert_eq!(&uncached_buffer[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    let uncached_sheet_xml = read_zip_entry(&uncached_buffer, "xl/worksheets/sheet1.xml");
    assert!(
        uncached_sheet_xml.contains("<f>"),
        "Formulas must be emitted in uncached mode"
    );
    assert!(
        !uncached_sheet_xml.contains("<f>ABS(-42.5)</f><v>42.5</v>"),
        "Uncached mode must not write evaluated value into formula result tag"
    );

    // Verify cached formula emission (<f> present with cached <v> tag)
    let cached_options = EmissionOptions {
        title: Some("Excel Parity Verification (Cached)".to_string()),
        author: Some("Finx Parity Engine".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let cached_buffer = Compiler::emit_workbook(
        &parity_manifest,
        &parity_tables,
        &theme_navy,
        Some(&parity_evaluated),
        &cached_options,
    )
    .expect("Cached parity model must emit to OpenXML buffer");

    let cached_sheet_xml = read_zip_entry(&cached_buffer, "xl/worksheets/sheet1.xml");
    assert!(
        cached_sheet_xml.contains("<f>ABS(-42.5)</f><v>42.5</v>"),
        "Cached mode must write precalculated value in <v> tag following formula"
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
        omit_formula_cache: false,
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
        omit_formula_cache: false,
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

    // 3. Export Styling Showcase Model
    let sc_file = output_dir.join("styling_showcase.xlsx");
    let _ = std::fs::remove_file(&sc_file);

    let (showcase_manifest, showcase_tables) =
        fixtures::styling_showcase::build_styling_showcase_model();
    let showcase_options = EmissionOptions {
        title: Some("Finx Institutional Styling Showcase".to_string()),
        author: Some("Finx Design System".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    Compiler::save_workbook(
        &showcase_manifest,
        &showcase_tables,
        &theme,
        None, // Automatically evaluates during compilation
        &showcase_options,
        &sc_file,
    )
    .expect("Failed to save styling showcase model to disk");

    assert!(sc_file.exists());
    let sc_bytes = std::fs::read(&sc_file).expect("Failed to read saved showcase file");
    assert!(sc_bytes.len() > 6000, "Expected non-trivial file size");
    assert_eq!(&sc_bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    assert!(has_zip_entry(&sc_bytes, "xl/worksheets/sheet1.xml"));
    assert!(has_zip_entry(&sc_bytes, "xl/worksheets/sheet2.xml"));

    // 4. Export Excel Parity Verification Model (Uncached for Web Excel Inspection)
    let parity_file = output_dir.join("excel_parity_verification.xlsx");
    let _ = std::fs::remove_file(&parity_file);

    let (parity_manifest, parity_tables) = fixtures::parity_suite::build_parity_model();
    let parity_options = EmissionOptions {
        title: Some("Excel Parity Verification Suite (All 43 Functions)".to_string()),
        author: Some("Finx Parity Engine".to_string()),
        values_only: false,
        omit_formula_cache: true,
    };

    Compiler::save_workbook(
        &parity_manifest,
        &parity_tables,
        &theme,
        None, // Automatically evaluates during compilation
        &parity_options,
        &parity_file,
    )
    .expect("Failed to save Excel parity verification model to disk");

    assert!(parity_file.exists());
    let parity_bytes = std::fs::read(&parity_file).expect("Failed to read saved parity file");
    assert!(parity_bytes.len() > 6000, "Expected non-trivial file size");
    assert_eq!(&parity_bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    assert!(has_zip_entry(&parity_bytes, "xl/workbook.xml"));
    assert!(has_zip_entry(&parity_bytes, "xl/worksheets/sheet1.xml"));

    let parity_sheet_xml = read_zip_entry(&parity_bytes, "xl/worksheets/sheet1.xml");
    assert!(
        parity_sheet_xml.contains("<f>"),
        "Formulas must be emitted in parity export"
    );
    assert!(
        !parity_sheet_xml.contains("<f>ABS(-42.5)</f><v>42.5</v>"),
        "Uncached mode must not cache formula results"
    );

    println!("\nExported test workbooks successfully:");
    println!("  - Three-Statement Model: {}", ts_file.display());
    println!("  - Institutional LBO Model: {}", lbo_file.display());
    println!("  - Styling Showcase Model: {}", sc_file.display());
    println!(
        "  - Parity Verification Model (Uncached): {}",
        parity_file.display()
    );
}
