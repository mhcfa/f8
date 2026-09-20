//! Dedicated executable runner for exporting the Finx Styling & Formatting Showcase model.
//!
//! Usage:
//! ```bash
//! cargo run --example export_styling_showcase
//! cargo run --example export_styling_showcase -- ./output
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
    println!("║             Finx Styling & Formatting Showcase Export                ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝\n");
    println!("Output directory: {}\n", canonical_out.display());

    let theme_navy = Theme::corporate_navy();
    let start = Instant::now();

    println!("▶ Compiling Styling & Formatting Showcase Model (Default Theme)...");
    let (sc_manifest, sc_tables) = fixtures::styling_showcase::build_styling_showcase_model();
    let sc_file = canonical_out.join("styling_showcase.xlsx");

    let sc_options = EmissionOptions {
        title: Some("Finx Institutional Styling & Formatting Showcase".to_string()),
        author: Some("Finx Design System".to_string()),
        values_only: false,
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
        start.elapsed()
    );

    println!("\n────────────────────────────────────────────────────────────────────────");
    println!(
        "✨ Successfully exported Styling Showcase in {:.2?}!",
        start.elapsed()
    );
    println!("\nOpen in spreadsheet viewer:");
    println!("  • file://{}", sc_file.display());
    println!("────────────────────────────────────────────────────────────────────────\n");

    Ok(())
}
