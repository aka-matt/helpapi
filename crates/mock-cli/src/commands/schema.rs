//! Schema command - generates a JSON schema for configuration files.

use crate::commands::Command;
use std::path::PathBuf;
use std::process::ExitCode;

/// Schema command implementation.
pub struct SchemaCommand {
    /// Output file path (None for stdout).
    output: Option<PathBuf>,
}

impl SchemaCommand {
    /// Creates a new schema command.
    pub fn new(output: Option<PathBuf>) -> Self {
        Self { output }
    }
}

impl Command for SchemaCommand {
    fn run(&self) -> ExitCode {
        let schema = mock_config::generate_schema();

        if let Some(path) = &self.output {
            if let Err(e) = std::fs::write(path, &schema) {
                eprintln!("Error writing schema to {}: {}", path.display(), e);
                return ExitCode::from(1);
            }
            println!("Schema written to {}", path.display());
        } else {
            println!("{}", schema);
        }

        ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_command_to_stdout() {
        let _cmd = SchemaCommand::new(None);
        assert_eq!(_cmd.run(), ExitCode::SUCCESS);
    }

    #[test]
    fn test_schema_generates_valid_json() {
        // This should generate valid JSON schema
        let schema = mock_config::generate_schema();
        let parsed: serde_json::Value = serde_json::from_str(&schema).unwrap();
        assert!(parsed.is_object());
        assert!(parsed.get("$schema").is_some());
    }
}
