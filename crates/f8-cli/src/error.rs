use std::path::PathBuf;
use thiserror::Error;

use f8_core::compiler::CompilerError;

/// Structured CLI error type mapping cleanly to exit codes.
#[derive(Debug, Error)]
pub enum CliError {
    #[error("Project manifest 'f8.json' not found starting from '{start_dir}'")]
    ProjectNotFound { start_dir: PathBuf },

    #[error("I/O error at '{path}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("JSON error in '{path}': {source}")]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error(
        "Cannot write binary OpenXML workbook to an interactive terminal. Redirect output or specify a destination with '-o <path>'."
    )]
    TtyOutputDisallowed,

    #[error("Unknown template archetype '{name}'. Available templates: {available}")]
    UnknownTemplate { name: String, available: String },

    #[error("Project already exists at '{path}' (f8.json found)")]
    ProjectAlreadyExists { path: PathBuf },

    #[error(transparent)]
    Compiler(#[from] CompilerError),
}

impl CliError {
    /// Maps CLI error variants to standard exit codes:
    /// - `1`: Operational, I/O, or syntax error
    /// - `2`: Compiler diagnostic (Passes 1–6a)
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Compiler(_) => 2,
            _ => 1,
        }
    }
}
