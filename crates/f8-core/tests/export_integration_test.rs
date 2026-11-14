//! Dedicated end-to-end integration test suite for OpenXML workbook emission and export.
//!
//! Validates that complete institutional financial models (Three-Statement and LBO)
//! compile through all six compiler passes into valid OpenXML `.xlsx` packages.
//!
//! - `test_models_emit_in_memory`: Runs during normal `cargo test` in RAM with zero disk writes.
//! - `test_export_xlsx_to_disk`: Marked `#[ignore]`, only executed when explicitly invoked.

#![allow(clippy::unwrap_used, clippy::expect_used, reason = "test code")]

use std::io::Read;
use std::path::Path;

use f8_core::Compiler;
use f8_core::compiler::emission::EmissionOptions;

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
    // 1. Three-Statement Financial Model
    let (ts_manifest, ts_tables) = fixtures::three_statement::build_three_statement_model();
    let ts_compiled = Compiler::compile(&ts_manifest, &ts_tables)
        .expect("Three-statement model must compile cleanly");

    let live_options = EmissionOptions {
        title: Some("Three-Statement Financial Model".to_string()),
        author: Some("f8 Institutional Engine".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let ts_buffer = ts_compiled
        .emit_workbook(&live_options)
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
    assert!(ts_core_xml.contains("f8 Institutional Engine"));

    // 2. Three-Statement Values-Only Freezing Mode
    let freeze_options = EmissionOptions {
        title: Some("Frozen Baseline".to_string()),
        author: Some("Audit Engine".to_string()),
        values_only: true,
        omit_formula_cache: false,
    };

    let ts_frozen = ts_compiled
        .emit_workbook(&freeze_options)
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

    let lbo_options = EmissionOptions {
        title: Some("Project Titan LBO Model".to_string()),
        author: Some("Sponsor Private Equity".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let lbo_buffer = Compiler::emit_workbook(&lbo_manifest, &lbo_tables, &lbo_options)
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
            && combined_sheets.contains("IFS")
            && combined_sheets.contains("PMT")
            && combined_sheets.contains("XIRR"),
        "Expected lowered financial and logical functions (XLOOKUP, IFS, PMT, XIRR) in formula tags"
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

    let showcase_options = EmissionOptions {
        title: Some("f8 Institutional Styling Showcase".to_string()),
        author: Some("f8 Design System".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let showcase_buffer =
        Compiler::emit_workbook(&showcase_manifest, &showcase_tables, &showcase_options)
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

    // 5. Excel Parity Verification Suite (All 48 Standard Functions + Edge & Error Cases)
    let (parity_manifest, parity_tables) = fixtures::parity_suite::build_parity_model();
    let parity_compiled = Compiler::compile(&parity_manifest, &parity_tables)
        .expect("Parity model must compile and evaluate cleanly");
    let parity_evaluated = &parity_compiled.evaluated;

    // Directly verify Pass 4 evaluated values and in-sheet parity_status against pinned Excel constants
    fixtures::parity_suite::assert_evaluated_parity(parity_evaluated);

    // Verify intentional domain error test cases cover all five canonical ExcelError variants
    let parity_errors = parity_evaluated.find_errors();
    assert!(
        !parity_errors.is_empty(),
        "Expected domain error test cases in parity suite"
    );
    for (cell_id, err) in &parity_errors {
        assert_eq!(
            cell_id.col.as_str(),
            "formula_val",
            "Only formula_val cells in error test cases may evaluate to an error, found error at {cell_id}"
        );
        let expected_recorded = parity_evaluated.get_value(
            &cell_id.table,
            &cell_id.row,
            &f8_core::model::id::ColId::from("expected_val"),
        );
        assert_eq!(
            expected_recorded,
            Some(&f8_core::value::CellValue::Text(err.as_str().to_string())),
            "Error cell {cell_id} must record its canonical error string in expected_val"
        );
    }

    for expected_variant in [
        f8_core::value::ExcelError::Div0,
        f8_core::value::ExcelError::Value,
        f8_core::value::ExcelError::Num,
        f8_core::value::ExcelError::Na,
        f8_core::value::ExcelError::Ref,
    ] {
        assert!(
            parity_errors
                .iter()
                .any(|(_, err)| **err == expected_variant),
            "Parity suite must include at least one graded test case for {expected_variant}"
        );
    }

    // Verify uncached formula emission (<f> present, <v> omitted for formulas)
    let uncached_options = EmissionOptions {
        title: Some("Excel Parity Verification (Uncached)".to_string()),
        author: Some("f8 Parity Engine".to_string()),
        values_only: false,
        omit_formula_cache: true,
    };

    let uncached_buffer = parity_compiled
        .emit_workbook(&uncached_options)
        .expect("Uncached parity model must emit to OpenXML buffer");

    assert_eq!(&uncached_buffer[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    let uncached_sheet_xml = read_zip_entry(&uncached_buffer, "xl/worksheets/sheet1.xml");
    assert!(
        uncached_sheet_xml.contains("<f>"),
        "Formulas must be emitted in uncached mode"
    );
    assert!(
        uncached_sheet_xml.contains("0.000000001*ABS("),
        "Parity check formulas must use 1e-9 (0.000000001) relative tolerance"
    );
    assert!(
        uncached_sheet_xml.contains("ERROR.TYPE("),
        "Parity check formulas must grade error cases via ERROR.TYPE"
    );
    assert!(
        !uncached_sheet_xml.contains("<f>ABS(-42.5)</f><v>42.5</v>"),
        "Uncached mode must not write evaluated value into formula result tag"
    );

    // Verify cached formula emission (<f> present with cached <v> tag)
    let cached_options = EmissionOptions {
        title: Some("Excel Parity Verification (Cached)".to_string()),
        author: Some("f8 Parity Engine".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let cached_buffer = parity_compiled
        .emit_workbook(&cached_options)
        .expect("Cached parity model must emit to OpenXML buffer");

    let cached_sheet_xml = read_zip_entry(&cached_buffer, "xl/worksheets/sheet1.xml");
    assert!(
        cached_sheet_xml.contains("<f>ABS(-42.5)</f><v>42.5</v>"),
        "Cached mode must write precalculated value in <v> tag following formula"
    );

    // 6. Institutional Public Trading Comparables Model
    let (tc_manifest, tc_tables) = fixtures::trading_comps::build_trading_comps_model();

    let tc_options = EmissionOptions {
        title: Some("Public Trading Comparables Model".to_string()),
        author: Some("f8 Valuation Engine".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let tc_buffer = Compiler::emit_workbook(&tc_manifest, &tc_tables, &tc_options)
        .expect("Trading comps model must emit to OpenXML buffer");

    assert_eq!(&tc_buffer[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    assert!(has_zip_entry(&tc_buffer, "xl/worksheets/sheet1.xml"));
    assert!(has_zip_entry(&tc_buffer, "xl/sharedStrings.xml"));

    let tc_workbook_xml = read_zip_entry(&tc_buffer, "xl/workbook.xml");
    assert!(tc_workbook_xml.contains("name=\"Trading_Comps\""));

    let tc_sheet_xml = read_zip_entry(&tc_buffer, "xl/worksheets/sheet1.xml");
    // Verify lowered formulas with @row lowered coordinates and range functions
    assert!(tc_sheet_xml.contains("<f>"));
    assert!(tc_sheet_xml.contains("MEDIAN(") && tc_sheet_xml.contains("AVERAGE("));
    assert!(tc_sheet_xml.contains("MAX(") && tc_sheet_xml.contains("MIN("));

    let tc_shared_strings = read_zip_entry(&tc_buffer, "xl/sharedStrings.xml");
    assert!(tc_shared_strings.contains("Public Trading Comparables"));
    assert!(tc_shared_strings.contains("Target Valuation Benchmarking"));
    assert!(tc_shared_strings.contains("Valuation Multiples"));
}

#[test]
fn test_migrated_parity_cases_rejected_with_f8_error() {
    use f8_core::compiler::ErrorDiagnostic;
    use f8_core::model::id::ColId;
    use f8_core::model::table::{CellInput, RowSpec};

    let migrated_compile_errors = [
        "ABS(\"invalid\")",
        "SUMIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r2\", \"category\"], \"Alpha\")",
        "DAYS(\"invalid\", DATE(2024, 1, 1))",
        "CHOOSE(5, 100, 200, 300)",
        "INDEX(SeedData[\"r1\"..\"r4\", \"num_a\"], 5)",
        "100 < \"1\"",
        "\"zzz\" < FALSE",
        "SeedData[\"r1\", \"sparse\"]",
        "SeedData[\"r1\", \"sparse\"] = \"\"",
        "SeedData[\"r1\", \"sparse\"] = FALSE",
        "INDEX(SeedData[\"r1\"..\"r4\", \"sparse\"], 1)",
        "XLOOKUP(\"Alpha\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"sparse\"])",
        // Cluster A2 migrated parity cases & retired SWITCH / taken-branch integer checks
        "IF(10 < 5, 100)",
        "IF(10 > 5, 100, \"NM\")",
        "IFERROR(10 / 0, \"N/A\")",
        "SWITCH(1, 1, 10, 20)",
        "IF(FALSE, SWITCH(1, 1, 10, 20), 0)",
        "ROUND(10, IF(FALSE, 2, 2.5))",
        "ISBLANK(IFERROR(SeedData[\"r1\", \"sparse\"], 1))",
        "ISBLANK(IFERROR(1 / 0, SeedData[\"r1\", \"sparse\"]))",
        "ISBLANK(IFNA(SeedData[\"r1\", \"sparse\"], 1))",
        "ISBLANK(IFNA(XLOOKUP(\"Missing\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"]), SeedData[\"r1\", \"sparse\"]))",
        "XLOOKUP(\"G*\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"], _, 2)",
        "XLOOKUP(\"Alpha\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"num_a\"], _, 99)",
        "XMATCH(\"Alpha\", SeedData[\"r1\"..\"r4\", \"category\"], 0, 2)",
        "XMATCH(20, SeedData[\"r1\"..\"r4\", \"num_a\"], 0, -2)",
        "INDEX(SeedData[\"r1\"..\"r4\", \"num_desc\"..\"category\"], 1, 1)",
        // Cluster B migrated parity cases & strict aggregation / descriptor / date-domain checks
        "SUM(SeedData[\"r1\"..\"r4\", \"category\"], TRUE, 5)",
        "COUNT(SeedData[\"r1\"..\"r4\", \"category\"])",
        "SUM(SeedData[\"r1\"..\"r4\", \"row_id\"])",
        "IF(FALSE, SUM(SeedData[\"r1\"..\"r4\", \"row_id\"]), 0)",
        "SUM(SeedData[\"r1\", \"sparse\"])",
        "IFERROR(SUM(SeedData[\"r1\", \"sparse\"]), 0)",
        "NPV(0.1, SeedData[\"r1\"..\"r4\", \"sparse\"])",
        "IRR(SeedData[\"r1\"..\"r4\", \"sparse\"])",
        "XNPV(0.1, SeedData[\"r1\"..\"r4\", \"date\"], SeedData[\"r1\"..\"r4\", \"cash_flow\"])",
        "DATE(24, 12, 31)",
        "DATE(2024, 14, 1)",
        "DATE(2024, 6, 31)",
        "DATE(2025, 2, 29)",
        "DATE(10000, 1, 1)",
        "YEARFRAC(DATE(2024, 1, 1), DATE(2024, 7, 1), 5)",
        "PMT(0.05, 10, -1000, 0, 2)",
        "YEAR(0)",
        "YEAR(SeedData[\"r2\", \"sparse\"])",
        "IF(FALSE, YEAR(SeedData[\"r2\", \"sparse\"]), 2024)",
        // Cluster C migrated parity cases & typed conditional aggregation criteria checks
        "SUMIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"category\"], \"A*\", SeedData[\"r1\"..\"r4\", \"num_a\"], \">15\")",
        "COUNTIFS(SeedData[\"r1\"..\"r4\", \"sparse\"], \"\")",
        "COUNTIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], \"NAN\")",
        "COUNTIFS(SeedData[\"r1\"..\"r4\", \"special_text\"], 42)",
        "COUNTIFS(SeedData[\"r1\"..\"r4\", \"special_text\"], \">0\")",
        "SUMIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"num_a\"], \"> 15 \")",
        "COUNTIFS(SeedData[\"r1\"..\"r4\", \"category\"], > \"Alpha\")",
        "COUNTIFS(SeedData[\"r1\"..\"r4\", \"category\"], > 10)",
        "COUNTIFS(SeedData[\"r1\"..\"r4\", \"category\"], \"10%\")",
        "COUNTIFS(SeedData[\"r1\"..\"r4\", \"category\"], \"1,000\")",
        "COUNTIFS(SeedData[\"r1\"..\"r4\", \"category\"], \"2024-01-01\")",
        "COUNTIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], <= (1 / 0))",
        "SUMIFS(SeedData[\"r1\"..\"r4\", \"num_a\"], SeedData[\"r1\"..\"r4\", \"date\"], <= XLOOKUP(\"Missing\", SeedData[\"r1\"..\"r4\", \"category\"], SeedData[\"r1\"..\"r4\", \"date\"]))",
        "ISERROR(\"5\" + 1)",
    ];

    for formula in migrated_compile_errors {
        let (manifest, mut tables) = fixtures::parity_suite::build_parity_model();
        // Inject the migrated invalid formula into MathParity["abs", "formula_val"]
        if let RowSpec::Data(abs_row) = &mut tables[1].rows[0] {
            abs_row
                .cells
                .insert(ColId::from("formula_val"), CellInput::formula(formula));
        }
        let err = Compiler::compile(&manifest, &tables)
            .expect_err(&format!("expected F8Error on migrated formula '{formula}'"));
        assert!(
            matches!(err.diagnostic(), ErrorDiagnostic::F8Error { .. }),
            "expected F8Error on '{formula}', got {err:?}"
        );
    }
}

#[test]
#[should_panic(expected = "Exact numeric mismatch for 'sum' in 'MathParity'")]
fn test_parity_oracle_detects_evaluation_divergence() {
    let (manifest, mut tables) = fixtures::parity_suite::build_parity_model();
    // Perturb SeedData["r1", "num_a"] from 10.0 to 11.0 while pinned expected_val constants stay fixed
    if let f8_core::model::table::RowSpec::Data(r1) = &mut tables[0].rows[0] {
        r1.cells.insert(
            f8_core::model::id::ColId::from("num_a"),
            f8_core::model::table::CellInput::number(11.0),
        );
    }
    let evaluated =
        Compiler::evaluate(&manifest, &tables).expect("Perturbed parity model must evaluate");
    fixtures::parity_suite::assert_evaluated_parity(&evaluated);
}

// ----------------------------------------------------------------------------
// Scenario 2: Physical Disk File Export (Ignored by Default in cargo test)
// Run with: cargo test --test export_integration_test -- --ignored
// ----------------------------------------------------------------------------
#[test]
#[ignore = "Generates physical .xlsx files on disk. Run with: cargo test --test export_integration_test -- --ignored"]
fn test_export_xlsx_to_disk() {
    let output_dir = Path::new("output");
    std::fs::create_dir_all(output_dir).expect("Failed to create output directory");

    let ts_file = output_dir.join("three_statement_model.xlsx");
    let lbo_file = output_dir.join("institutional_lbo_model.xlsx");

    // Clean up preexisting artifacts
    let _ = std::fs::remove_file(&ts_file);
    let _ = std::fs::remove_file(&lbo_file);

    // 1. Export Three-Statement Model
    let (ts_manifest, ts_tables) = fixtures::three_statement::build_three_statement_model();
    let ts_options = EmissionOptions {
        title: Some("Three-Statement Financial Model".to_string()),
        author: Some("f8 Institutional Engine".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let ts_buf = Compiler::emit_workbook(&ts_manifest, &ts_tables, &ts_options)
        .expect("Failed to emit Three-Statement model to buffer");
    std::fs::write(&ts_file, ts_buf).expect("Failed to write Three-Statement model to disk");

    assert!(ts_file.exists());
    let ts_bytes = std::fs::read(&ts_file).expect("Failed to read saved Three-Statement file");
    assert!(ts_bytes.len() > 4000, "Expected non-trivial file size");
    assert_eq!(&ts_bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    assert!(has_zip_entry(&ts_bytes, "xl/workbook.xml"));
    let ts_sheet1_xml = read_zip_entry(&ts_bytes, "xl/worksheets/sheet1.xml");
    assert!(ts_sheet1_xml.contains("<f>"), "Formulas must be emitted");
    assert!(
        ts_sheet1_xml.contains("<v>114.99") || ts_sheet1_xml.contains("<v>115</v>"),
        "Formulas must have precalculated values cached in <v> tag"
    );

    // 2. Export Institutional LBO Model
    let (lbo_manifest, lbo_tables) = fixtures::lbo::build_lbo_model("base");
    let lbo_options = EmissionOptions {
        title: Some("Project Titan LBO Model".to_string()),
        author: Some("Sponsor Private Equity".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let lbo_buf = Compiler::emit_workbook(&lbo_manifest, &lbo_tables, &lbo_options)
        .expect("Failed to emit LBO model to buffer");
    std::fs::write(&lbo_file, lbo_buf).expect("Failed to write LBO model to disk");

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
        title: Some("f8 Institutional Styling Showcase".to_string()),
        author: Some("f8 Design System".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let sc_buf = Compiler::emit_workbook(&showcase_manifest, &showcase_tables, &showcase_options)
        .expect("Failed to emit styling showcase model to buffer");
    std::fs::write(&sc_file, sc_buf).expect("Failed to write styling showcase model to disk");

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
        title: Some("Excel Parity Verification Suite (All 48 Functions)".to_string()),
        author: Some("f8 Parity Engine".to_string()),
        values_only: false,
        omit_formula_cache: true,
    };

    let parity_buf = Compiler::emit_workbook(&parity_manifest, &parity_tables, &parity_options)
        .expect("Failed to emit Excel parity verification model to buffer");
    std::fs::write(&parity_file, parity_buf)
        .expect("Failed to write Excel parity verification model to disk");

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

    // 5. Export Public Trading Comparables Model
    let tc_file = output_dir.join("trading_comps_model.xlsx");
    let _ = std::fs::remove_file(&tc_file);

    let (tc_manifest, tc_tables) = fixtures::trading_comps::build_trading_comps_model();
    let tc_options = EmissionOptions {
        title: Some("Public Trading Comparables Model".to_string()),
        author: Some("f8 Valuation Engine".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let tc_buf = Compiler::emit_workbook(&tc_manifest, &tc_tables, &tc_options)
        .expect("Failed to emit Trading Comps model to buffer");
    std::fs::write(&tc_file, tc_buf).expect("Failed to write Trading Comps model to disk");

    assert!(tc_file.exists());
    let tc_bytes = std::fs::read(&tc_file).expect("Failed to read saved Trading Comps file");
    assert!(tc_bytes.len() > 6000, "Expected non-trivial file size");
    assert_eq!(&tc_bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    assert!(has_zip_entry(&tc_bytes, "xl/workbook.xml"));
    assert!(has_zip_entry(&tc_bytes, "xl/worksheets/sheet1.xml"));

    let tc_sheet1_xml = read_zip_entry(&tc_bytes, "xl/worksheets/sheet1.xml");
    assert!(
        tc_sheet1_xml.contains("<f>"),
        "Formulas must be emitted in trading comps export"
    );
    assert!(
        tc_sheet1_xml.contains("MEDIAN("),
        "MEDIAN formula must be emitted"
    );

    println!("\nExported test workbooks successfully:");
    println!("  - Three-Statement Model: {}", ts_file.display());
    println!("  - Institutional LBO Model: {}", lbo_file.display());
    println!("  - Styling Showcase Model: {}", sc_file.display());
    println!(
        "  - Parity Verification Model (Uncached): {}",
        parity_file.display()
    );
    println!("  - Trading Comps Model: {}", tc_file.display());
}
