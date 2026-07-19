//! CLI commands module.

pub mod print_config;
pub mod run;
pub mod schema;
pub mod validate;

use std::process::ExitCode;

/// Trait for CLI commands.
#[allow(dead_code)]
pub trait Command {
    /// Runs the command and returns the exit code.
    fn run(&self) -> ExitCode;
}
