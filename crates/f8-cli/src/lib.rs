pub mod cli;
pub mod commands;
pub mod error;
pub mod project;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::error::CliError;

/// Main entry point for CLI execution, returning the process exit code.
pub fn run() -> i32 {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(err) => {
            if err.use_stderr() {
                eprintln!("{err}");
                return 1;
            } else {
                print!("{err}");
                return 0;
            }
        }
    };

    let is_json = cli.json;
    if let Err(err) = dispatch(&cli) {
        if is_json {
            let json_err = serde_json::json!({
                "status": "error",
                "exit_code": err.exit_code(),
                "error": err.to_string(),
            });
            match serde_json::to_string_pretty(&json_err) {
                Ok(s) => eprintln!("{s}"),
                Err(_) => eprintln!("{err}"),
            }
        } else {
            eprintln!("Error: {err}");
        }
        return err.exit_code();
    }

    0
}

fn dispatch(cli: &Cli) -> Result<(), CliError> {
    match &cli.command {
        Command::Check(args) => commands::check::execute(args, cli.project.as_deref(), cli.json),
    }
}
