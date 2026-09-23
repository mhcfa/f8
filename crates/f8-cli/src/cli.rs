use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

use crate::render::table::{TableFormat, ViewMode};

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

    /// Inspect compiled model structure, tables, cells, or dependency trees
    Inspect(InspectArgs),
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

/// Arguments for `f8 inspect`.
#[derive(Debug, Args, Default)]
pub struct InspectArgs {
    #[command(subcommand)]
    pub command: Option<InspectSubcommand>,
}

/// Available inspection subcommands.
#[derive(Debug, Subcommand)]
pub enum InspectSubcommand {
    /// Inspect a 2D table grid in terminal table, markdown, or JSON format
    Table(InspectTableArgs),

    /// Inspect an individual cell coordinate with formulas, values, and dependencies
    Cell(InspectCellArgs),

    /// Inspect upstream calculation precedents tree
    Precedents(InspectPrecedentsArgs),

    /// Inspect downstream blast-radius dependents tree
    Dependents(InspectDependentsArgs),
}

/// Arguments for `f8 inspect table`.
#[derive(Debug, Args)]
pub struct InspectTableArgs {
    /// Programmatic identifier of the table to inspect
    pub table: String,

    /// Cell display mode (values, formulas, both)
    #[arg(long, default_value = "values")]
    pub view: ViewMode,

    /// Output presentation format (table, markdown, json)
    #[arg(long, default_value = "table")]
    pub format: TableFormat,
}

/// Arguments for `f8 inspect cell`.
#[derive(Debug, Args)]
pub struct InspectCellArgs {
    /// Target coordinate: table ID (when specifying 3 args) or canonical IR reference `Table["row", "col"]`
    pub target: String,

    /// Row identifier (when using 3 positional arguments: <table row col>)
    pub row: Option<String>,

    /// Column identifier (when using 3 positional arguments: <table row col>)
    pub col: Option<String>,
}

/// Arguments for `f8 inspect precedents`.
#[derive(Debug, Args)]
pub struct InspectPrecedentsArgs {
    /// Target coordinate: table ID (when specifying 3 args) or canonical IR reference `Table["row", "col"]`
    pub target: String,

    /// Row identifier (when using 3 positional arguments: <table row col>)
    pub row: Option<String>,

    /// Column identifier (when using 3 positional arguments: <table row col>)
    pub col: Option<String>,

    /// Recursively traverse all transitive upstream calculation dependencies
    #[arg(long)]
    pub recursive: bool,
}

/// Arguments for `f8 inspect dependents`.
#[derive(Debug, Args)]
pub struct InspectDependentsArgs {
    /// Target coordinate: table ID (when specifying 3 args) or canonical IR reference `Table["row", "col"]`
    pub target: String,

    /// Row identifier (when using 3 positional arguments: <table row col>)
    pub row: Option<String>,

    /// Column identifier (when using 3 positional arguments: <table row col>)
    pub col: Option<String>,

    /// Recursively traverse all transitive downstream affected calculations
    #[arg(long)]
    pub recursive: bool,
}
