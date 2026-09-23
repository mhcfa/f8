use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// Command-line interface definition for f8.
#[derive(Debug, Parser)]
#[command(
    name = "f8",
    about = "Next-generation declarative financial modeling compiler and CLI",
    version
)]
pub struct Cli {
    /// Path to project directory or f8.json manifest
    #[arg(short = 'p', long = "project", global = true)]
    pub project: Option<PathBuf>,

    /// Output results in machine-readable JSON format
    #[arg(long = "json", global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

/// Available f8 subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run headless compiler verification across Passes 1–6a
    Check(CheckArgs),

    /// Compile project specifications into an OpenXML .xlsx workbook
    Compile(CompileArgs),

    /// Initialize a new financial model project or unpack an archetype template
    Init(InitArgs),
}

/// Arguments for `f8 check`.
#[derive(Debug, Args, Default)]
pub struct CheckArgs {}

/// Arguments for `f8 compile`.
#[derive(Debug, Args, Default)]
pub struct CompileArgs {
    /// Output .xlsx file destination ('-' for stdout)
    #[arg(short = 'o', long = "out")]
    pub out: Option<PathBuf>,

    /// Override the visual theme configured in f8.json
    #[arg(long = "theme")]
    pub theme: Option<String>,

    /// Freeze calculated formulas into static calculation values
    #[arg(long = "values-only")]
    pub values_only: bool,

    /// Omit precalculated <v> calculation result tags in OpenXML cells
    #[arg(long = "omit-cache")]
    pub omit_cache: bool,
}

/// Arguments for `f8 init`.
#[derive(Debug, Args, Default)]
pub struct InitArgs {
    /// Destination directory for the new project (defaults to current directory)
    pub path: Option<PathBuf>,

    /// Template archetype to unpack (three-statement, lbo, trading-comps)
    #[arg(short = 't', long = "template")]
    pub template: Option<String>,

    /// Visual theme preset for the project
    #[arg(long = "theme", default_value = "corporate_navy")]
    pub theme: String,
}
