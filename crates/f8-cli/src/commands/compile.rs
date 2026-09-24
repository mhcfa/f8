use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;

use f8_core::compiler::Compiler;
use f8_core::compiler::emission::EmissionOptions;

use crate::cli::CompileArgs;
use crate::error::CliError;
use crate::project::ProjectLoader;

/// Summary metrics serialized when `--json` is supplied to `f8 compile`.
#[derive(Debug, Serialize)]
pub struct CompileJsonOutput {
    pub status: &'static str,
    pub path: String,
    pub size_bytes: usize,
    pub duration_ms: f64,
}

/// Executes the `f8 compile` command: loads the project, runs full compilation, emits `.xlsx`,
/// and writes to disk or stdout.
pub fn execute(
    args: &CompileArgs,
    project_path: Option<&Path>,
    json: bool,
) -> Result<(), CliError> {
    let loaded = ProjectLoader::load(project_path)?;

    let theme = match &args.theme {
        Some(theme_name) => ProjectLoader::resolve_theme(theme_name, &loaded.root_dir)?,
        None => loaded.theme,
    };

    let start = Instant::now();
    let compiled =
        Compiler::compile(&loaded.manifest, &loaded.tables, &theme).map_err(CliError::Compiler)?;

    let emission_options = EmissionOptions {
        values_only: args.values_only,
        omit_formula_cache: args.omit_cache,
        title: Some(loaded.manifest.model_name.clone()),
        author: None,
    };

    let bytes = compiled
        .emit_workbook(&emission_options)
        .map_err(CliError::Compiler)?;

    let elapsed = start.elapsed();
    let duration_ms = (elapsed.as_secs_f64() * 1000.0 * 10.0).round() / 10.0;

    let is_stdout = args
        .out
        .as_deref()
        .map(|p| p == Path::new("-"))
        .unwrap_or(false);

    if is_stdout {
        if std::io::stdout().is_terminal() {
            return Err(CliError::TtyOutputDisallowed);
        }
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(&bytes).map_err(|source| CliError::Io {
            path: PathBuf::from("<stdout>"),
            source,
        })?;
        stdout.flush().map_err(|source| CliError::Io {
            path: PathBuf::from("<stdout>"),
            source,
        })?;
        return Ok(());
    }

    let dest_path = match &args.out {
        Some(path) => path.clone(),
        None => {
            let file_name = format!("{}.xlsx", loaded.manifest.model_name);
            PathBuf::from(file_name)
        }
    };

    if let Some(parent) = dest_path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent).map_err(|source| CliError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    std::fs::write(&dest_path, &bytes).map_err(|source| CliError::Io {
        path: dest_path.clone(),
        source,
    })?;

    if json {
        let output = CompileJsonOutput {
            status: "ok",
            path: dest_path.display().to_string(),
            size_bytes: bytes.len(),
            duration_ms,
        };
        let json_str = serde_json::to_string_pretty(&output).map_err(|source| CliError::Json {
            path: dest_path.clone(),
            source,
        })?;
        println!("{json_str}");
    } else {
        let summary = compiled.summary();
        println!(
            "✓ Compiled {} sheets ({} cells) to {} in {:.1}ms",
            summary.total_sheets,
            summary.total_cells,
            dest_path.display(),
            duration_ms
        );
    }

    Ok(())
}
