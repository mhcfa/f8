use std::path::PathBuf;

use serde::Serialize;

use f8_core::compiler::Compiler;

use crate::archetypes;
use crate::cli::InitArgs;
use crate::error::CliError;
use crate::project::{ProjectLoader, ProjectWriter};

/// JSON payload structure for successful `f8 init`.
#[derive(Debug, Serialize)]
pub struct InitJsonOutput {
    pub status: &'static str,
    pub path: String,
    pub template: String,
    pub theme: String,
}

/// Executes the `f8 init` command: scaffolds project structure from blank or archetype template.
pub fn execute(args: &InitArgs, json: bool) -> Result<(), CliError> {
    let dest_dir = args.path.clone().unwrap_or_else(|| PathBuf::from("."));

    let archetype = match &args.template {
        Some(tmpl) => archetypes::get_archetype(tmpl).ok_or_else(|| CliError::UnknownTemplate {
            name: tmpl.clone(),
            available: archetypes::available_names(),
        })?,
        None => &archetypes::BLANK_ARCHETYPE,
    };

    let theme = args.theme.trim();
    ProjectWriter::write_archetype(&dest_dir, archetype, Some(theme))?;

    // Self-verification: ensure initialized project parses and passes compilation
    let loaded = ProjectLoader::load(Some(&dest_dir))?;
    Compiler::compile(&loaded.manifest, &loaded.tables, &loaded.theme)
        .map_err(CliError::Compiler)?;

    if json {
        let output = InitJsonOutput {
            status: "ok",
            path: dest_dir.display().to_string(),
            template: archetype.name.to_string(),
            theme: theme.to_string(),
        };
        let json_str = serde_json::to_string_pretty(&output).map_err(|source| CliError::Json {
            path: dest_dir.clone(),
            source,
        })?;
        println!("{json_str}");
    } else {
        println!(
            "✓ Initialized new f8 project at '{}' using template '{}' (theme: {})",
            dest_dir.display(),
            archetype.name,
            theme
        );
    }

    Ok(())
}
