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
}

/// Arguments for `f8 check`.
#[derive(Debug, Args, Default)]
pub struct CheckArgs {}
