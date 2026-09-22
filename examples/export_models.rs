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

use finx::Compiler;
use finx::compiler::emission::EmissionOptions;
use finx::model::theme::Theme;

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
    println!("║                   Finx Excel Model Export Runner                     ║");
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

    let ts_options = EmissionOptions {
        title: Some("Three-Statement Financial Model".to_string()),
        author: Some("Finx Institutional Engine".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    Compiler::save_workbook(
        &ts_manifest,
        &ts_tables,
        &theme_navy,
        None, // Automatic DAG evaluation and formula caching
        &ts_options,
        &ts_file,
    )?;

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
        author: Some("Finx Audit Freeze Engine".to_string()),
        values_only: true,
        omit_formula_cache: false,
    };

    Compiler::save_workbook(
        &ts_manifest,
        &ts_tables,
        &theme_navy,
        None,
        &freeze_options,
        &ts_frozen_file,
    )?;

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

    Compiler::save_workbook(
        &lbo_manifest,
        &lbo_tables,
        &theme_navy,
        None, // Automatic DAG evaluation and formula caching
        &lbo_options,
        &lbo_file,
    )?;

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
        title: Some("Finx Institutional Styling & Formatting Showcase".to_string()),
        author: Some("Finx Design System".to_string()),
        values_only: false,
        omit_formula_cache: false,
    };

    Compiler::save_workbook(
        &sc_manifest,
        &sc_tables,
        &theme_navy,
        None, // Automatic DAG evaluation and formula caching
        &sc_options,
        &sc_file,
    )?;

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
        author: Some("Finx Parity Engine".to_string()),
        values_only: false,
        omit_formula_cache: true,
    };

    Compiler::save_workbook(
        &parity_manifest,
        &parity_tables,
        &theme_navy,
        None,
        &parity_options,
        &parity_file,
    )?;

    let parity_meta = fs::metadata(&parity_file)?;
    println!(
        "  ✔ Saved to: {} ({}, {} tables, 1 sheet) in {:.2?}",
        parity_file.display(),
        format_bytes(parity_meta.len()),
        parity_tables.len(),
        parity_start.elapsed()
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
    println!("────────────────────────────────────────────────────────────────────────\n");

    Ok(())
}
