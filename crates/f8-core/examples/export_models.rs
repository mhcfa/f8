//! Dedicated executable runner for exporting production-grade Excel models to disk.
//!
//! Usage:
//! ```bash
//! cargo run --example export_models
//! cargo run --example export_models -- ./output
//! ```

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use f8_core::Compiler;
use f8_core::compiler::emission::EmissionOptions;
use f8_core::model::theme::Theme;

#[path = "../tests/fixtures/mod.rs"]
mod fixtures;

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let out_dir_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("output")
    };

    fs::create_dir_all(&out_dir_path)?;
    let canonical_out = fs::canonicalize(&out_dir_path).unwrap_or(out_dir_path);

    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║                    f8 Excel Model Export Runner                      ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝\n");
    println!("Output directory: {}\n", canonical_out.display());

    let theme_navy = Theme::corporate_navy();
    let start_all = Instant::now();

    // -------------------------------------------------------------------------
    // 1. Three-Statement Financial Model
    // -------------------------------------------------------------------------
    println!("▶ Compiling Three-Statement Financial Model...");
    let ts_start = Instant::now();
    let (ts_manifest, ts_tables) = fixtures::three_statement::build_three_statement_model();
    let ts_file = canonical_out.join("three_statement_model.xlsx");
    let ts_compiled = Compiler::compile(&ts_manifest, &ts_tables, &theme_navy)?;

    let ts_options = EmissionOptions {
        title: Some("Three-Statement Financial Model".to_string()),
        author: Some("f8 Institutional Engine".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let ts_buf = ts_compiled.emit_workbook(&ts_options)?;
    fs::write(&ts_file, ts_buf)?;

    let ts_meta = fs::metadata(&ts_file)?;
    println!(
        "  ✔ Saved to: {} ({}, {} tables, 1 sheet) in {:.2?}",
        ts_file.display(),
        format_bytes(ts_meta.len()),
        ts_tables.len(),
        ts_start.elapsed()
    );

    // -------------------------------------------------------------------------
    // 2. Three-Statement Model (Values-Only Freeze Baseline)
    // -------------------------------------------------------------------------
    println!("\n▶ Compiling Three-Statement Model (Values-Only Freeze)...");
    let freeze_start = Instant::now();
    let ts_frozen_file = canonical_out.join("three_statement_model_frozen.xlsx");
    let freeze_options = EmissionOptions {
        title: Some("Three-Statement Financial Model (Frozen)".to_string()),
        author: Some("f8 Audit Freeze Engine".to_string()),
        values_only: true,
        omit_formula_cache: false,
    };

    let freeze_buf = ts_compiled.emit_workbook(&freeze_options)?;
    fs::write(&ts_frozen_file, freeze_buf)?;

    let freeze_meta = fs::metadata(&ts_frozen_file)?;
    println!(
        "  ✔ Saved to: {} ({}) in {:.2?}",
        ts_frozen_file.display(),
        format_bytes(freeze_meta.len()),
        freeze_start.elapsed()
    );

    // -------------------------------------------------------------------------
    // 3. Institutional Leveraged Buyout (LBO) Model
    // -------------------------------------------------------------------------
    println!("\n▶ Compiling Institutional LBO Model (Base Case)...");
    let lbo_start = Instant::now();
    let (lbo_manifest, lbo_tables) = fixtures::lbo::build_lbo_model("base");
    let lbo_file = canonical_out.join("institutional_lbo_model.xlsx");

    let lbo_options = EmissionOptions {
        title: Some("Project Titan Institutional LBO Model".to_string()),
        author: Some("Sponsor Private Equity".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let lbo_buf = Compiler::emit_workbook(&lbo_manifest, &lbo_tables, &theme_navy, &lbo_options)?;
    fs::write(&lbo_file, lbo_buf)?;

    let lbo_meta = fs::metadata(&lbo_file)?;
    println!(
        "  ✔ Saved to: {} ({}, {} tables, 2 sheets) in {:.2?}",
        lbo_file.display(),
        format_bytes(lbo_meta.len()),
        lbo_tables.len(),
        lbo_start.elapsed()
    );

    // -------------------------------------------------------------------------
    // 4. Styling & Formatting Showcase Model
    // -------------------------------------------------------------------------
    println!("\n▶ Compiling Styling & Formatting Showcase Model...");
    let sc_start = Instant::now();
    let (sc_manifest, sc_tables) = fixtures::styling_showcase::build_styling_showcase_model();
    let sc_file = canonical_out.join("styling_showcase.xlsx");

    let sc_options = EmissionOptions {
        title: Some("f8 Institutional Styling & Formatting Showcase".to_string()),
        author: Some("f8 Design System".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let sc_buf = Compiler::emit_workbook(&sc_manifest, &sc_tables, &theme_navy, &sc_options)?;
    fs::write(&sc_file, sc_buf)?;

    let sc_meta = fs::metadata(&sc_file)?;
    println!(
        "  ✔ Saved to: {} ({}, {} tables, 2 sheets) in {:.2?}",
        sc_file.display(),
        format_bytes(sc_meta.len()),
        sc_tables.len(),
        sc_start.elapsed()
    );

    // -------------------------------------------------------------------------
    // 5. Excel Parity Verification Suite (All 44 Functions, Uncached)
    // -------------------------------------------------------------------------
    println!("\n▶ Compiling Excel Parity Verification Model (All 44 Functions, Uncached)...");
    let parity_start = Instant::now();
    let (parity_manifest, parity_tables) = fixtures::parity_suite::build_parity_model();
    let parity_file = canonical_out.join("excel_parity_verification.xlsx");

    let parity_options = EmissionOptions {
        title: Some("Excel Parity Verification Suite (All 44 Functions)".to_string()),
        author: Some("f8 Parity Engine".to_string()),
        values_only: false,
        omit_formula_cache: true,
    };

    let parity_buf = Compiler::emit_workbook(
        &parity_manifest,
        &parity_tables,
        &theme_navy,
        &parity_options,
    )?;
    fs::write(&parity_file, parity_buf)?;

    let parity_meta = fs::metadata(&parity_file)?;
    println!(
        "  ✔ Saved to: {} ({}, {} tables, 1 sheet) in {:.2?}",
        parity_file.display(),
        format_bytes(parity_meta.len()),
        parity_tables.len(),
        parity_start.elapsed()
    );

    // -------------------------------------------------------------------------
    // 6. Public Trading Comparables Model
    // -------------------------------------------------------------------------
    println!("\n▶ Compiling Public Trading Comparables Model...");
    let tc_start = Instant::now();
    let (tc_manifest, tc_tables) = fixtures::trading_comps::build_trading_comps_model();
    let tc_file = canonical_out.join("trading_comps_model.xlsx");

    let tc_options = EmissionOptions {
        title: Some("Public Trading Comparables Model".to_string()),
        author: Some("f8 Valuation Engine".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    let tc_buf = Compiler::emit_workbook(&tc_manifest, &tc_tables, &theme_navy, &tc_options)?;
    fs::write(&tc_file, tc_buf)?;

    let tc_meta = fs::metadata(&tc_file)?;
    println!(
        "  ✔ Saved to: {} ({}, {} tables, 1 sheet) in {:.2?}",
        tc_file.display(),
        format_bytes(tc_meta.len()),
        tc_tables.len(),
        tc_start.elapsed()
    );

    // -------------------------------------------------------------------------
    // Summary
    // -------------------------------------------------------------------------
    println!("\n────────────────────────────────────────────────────────────────────────");
    println!(
        "✨ Successfully compiled and exported all models in {:.2?}!",
        start_all.elapsed()
    );
    println!("\nOpen in spreadsheet viewer:");
    println!("  • file://{}", ts_file.display());
    println!("  • file://{}", ts_frozen_file.display());
    println!("  • file://{}", lbo_file.display());
    println!("  • file://{}", sc_file.display());
    println!("  • file://{}", parity_file.display());
    println!("  • file://{}", tc_file.display());
    println!("────────────────────────────────────────────────────────────────────────\n");

    Ok(())
}
