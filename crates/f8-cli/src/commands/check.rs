use std::path::Path;
use std::time::Instant;

use serde::Serialize;

use f8_core::compiler::Compiler;

use crate::cli::CheckArgs;
use crate::error::CliError;
use crate::project::ProjectLoader;

/// Summary metrics serialized when `--json` is supplied to `f8 check`.
#[derive(Debug, Serialize)]
pub struct CheckJsonSummary {
    pub sheets: usize,
    pub tables: usize,
    pub cells: usize,
    pub duration_ms: f64,
}

/// JSON payload structure for successful `f8 check`.
#[derive(Debug, Serialize)]
pub struct CheckJsonOutput {
    pub status: &'static str,
    pub summary: CheckJsonSummary,
}

/// Executes the `f8 check` command: loads the project, verifies Passes 1–6a, and formats output.
pub fn execute(_args: &CheckArgs, project_path: Option<&Path>, json: bool) -> Result<(), CliError> {
    let loaded = ProjectLoader::load(project_path)?;

    let start = Instant::now();
    let compiled = Compiler::compile(&loaded.manifest, &loaded.tables, &loaded.theme)
        .map_err(CliError::Compiler)?;
    let elapsed = start.elapsed();
    let duration_ms = (elapsed.as_secs_f64() * 1000.0 * 10.0).round() / 10.0;

    let summary = compiled.summary();

    if json {
        let output = CheckJsonOutput {
            status: "ok",
            summary: CheckJsonSummary {
                sheets: summary.total_sheets,
                tables: summary.total_tables,
                cells: summary.total_cells,
                duration_ms,
            },
        };
        let json_str = serde_json::to_string_pretty(&output).map_err(|source| CliError::Json {
            path: loaded.manifest_path.clone(),
            source,
        })?;
        println!("{json_str}");
    } else {
        println!(
            "✓ Validated {} sheets, {} tables, {} cells",
            summary.total_sheets, summary.total_tables, summary.total_cells
        );
        println!("✓ DAG acyclicity verified (0 cycles)");
        println!(
            "✓ Evaluated {} cells in {:.1}ms",
            summary.total_cells, duration_ms
        );
        println!(
            "✓ Planned spatial layouts and verified formula lowering across {} sheets",
            summary.total_sheets
        );
        println!("Status: OK");
    }

    Ok(())
}
