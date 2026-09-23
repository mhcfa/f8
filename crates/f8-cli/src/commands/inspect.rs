use std::path::Path;
use std::time::Instant;

use f8_core::compiler::{Compiler, parse_coordinate_tokens};
use f8_core::model::id::TableId;

use crate::cli::{InspectArgs, InspectSubcommand};
use crate::error::CliError;
use crate::project::ProjectLoader;
use crate::render::{
    TableFormat, render_cell, render_dependents, render_precedents, render_summary, render_table,
};

/// Executes the `f8 inspect` family of subcommands.
pub fn execute(
    args: &InspectArgs,
    project_path: Option<&Path>,
    json: bool,
) -> Result<(), CliError> {
    let loaded = ProjectLoader::load(project_path)?;

    let start = Instant::now();
    let compiled = Compiler::compile(&loaded.manifest, &loaded.tables, &loaded.theme)
        .map_err(CliError::Compiler)?;
    let elapsed = start.elapsed();
    let duration_ms = (elapsed.as_secs_f64() * 1000.0 * 10.0).round() / 10.0;

    match &args.command {
        None => {
            let summary = compiled.summary();
            if json {
                let json_str =
                    serde_json::to_string_pretty(&summary).map_err(|source| CliError::Json {
                        path: loaded.manifest_path.clone(),
                        source,
                    })?;
                println!("{json_str}");
            } else {
                let rendered = render_summary(&summary, Some(duration_ms));
                println!("{rendered}");
            }
        }
        Some(InspectSubcommand::Table(table_args)) => {
            let table_id = TableId::from(table_args.table.as_str());
            let inspection = compiled.inspect_table(&table_id)?;
            let format = if json {
                TableFormat::Json
            } else {
                table_args.format
            };
            let rendered = render_table(&inspection, table_args.view, format);
            if rendered.ends_with('\n') {
                print!("{rendered}");
            } else {
                println!("{rendered}");
            }
        }
        Some(InspectSubcommand::Cell(cell_args)) => {
            let (table_id, row_id, col_id) = parse_coordinate_tokens(
                &cell_args.target,
                cell_args.row.as_deref(),
                cell_args.col.as_deref(),
            )?;
            let inspection = compiled.inspect_cell(&table_id, &row_id, &col_id)?;
            if json {
                let json_str =
                    serde_json::to_string_pretty(&inspection).map_err(|source| CliError::Json {
                        path: loaded.manifest_path.clone(),
                        source,
                    })?;
                println!("{json_str}");
            } else {
                let rendered = render_cell(&inspection);
                println!("{rendered}");
            }
        }
        Some(InspectSubcommand::Precedents(prec_args)) => {
            let cell_id = compiled.parse_coordinate(
                &prec_args.target,
                prec_args.row.as_deref(),
                prec_args.col.as_deref(),
            )?;
            let tree = compiled.precedents_of(&cell_id, prec_args.recursive)?;
            if json {
                let json_str =
                    serde_json::to_string_pretty(&tree).map_err(|source| CliError::Json {
                        path: loaded.manifest_path.clone(),
                        source,
                    })?;
                println!("{json_str}");
            } else {
                let rendered = render_precedents(&tree, prec_args.recursive);
                if rendered.ends_with('\n') {
                    print!("{rendered}");
                } else {
                    println!("{rendered}");
                }
            }
        }
        Some(InspectSubcommand::Dependents(dep_args)) => {
            let cell_id = compiled.parse_coordinate(
                &dep_args.target,
                dep_args.row.as_deref(),
                dep_args.col.as_deref(),
            )?;
            let tree = compiled.dependents_of(&cell_id, dep_args.recursive)?;
            if json {
                let json_str =
                    serde_json::to_string_pretty(&tree).map_err(|source| CliError::Json {
                        path: loaded.manifest_path.clone(),
                        source,
                    })?;
                println!("{json_str}");
            } else {
                let rendered = render_dependents(&tree, dep_args.recursive);
                if rendered.ends_with('\n') {
                    print!("{rendered}");
                } else {
                    println!("{rendered}");
                }
            }
        }
    }

    Ok(())
}
