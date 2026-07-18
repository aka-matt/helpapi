//! Validate command - checks a config file for errors.

use crate::commands::Command;
use std::path::PathBuf;
use std::process::ExitCode;

/// Validate command implementation.
#[allow(dead_code)]
pub struct ValidateCommand {
    /// Path to the config file to validate.
    config_path: PathBuf,
}

impl ValidateCommand {
    /// Creates a new validate command.
    #[allow(dead_code)]
    pub fn new(config_path: PathBuf) -> Self {
        Self { config_path }
    }
}

impl Command for ValidateCommand {
    fn run(&self) -> ExitCode {
        let content = match std::fs::read_to_string(&self.config_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "Error reading config file {}: {}",
                    self.config_path.display(),
                    e
                );
                return ExitCode::from(1);
            }
        };

        match mock_config::parse_and_validate(&content) {
            Ok(_) => {
                println!("Config is valid");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("Config is invalid: {}", e);
                ExitCode::from(1)
            }
        }
    }
}

/// Helper to create the validate subcommand for clap.
#[allow(dead_code)]
pub fn validate_subcommand() -> clap::Command {
    clap::Command::new("validate")
        .about("Validate a mock-api configuration file")
        .arg(
            clap::Arg::new("config")
                .short('c')
                .long("config")
                .help("Path to the configuration file")
                .required(true)
                .value_parser(clap::value_parser!(PathBuf)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn run_validate_on_content(content: &str) -> ExitCode {
        let mut file = NamedTempFile::with_suffix(".json").unwrap();
        file.write_all(content.as_bytes()).unwrap();
        let path = file.path().to_path_buf();
        let cmd = ValidateCommand::new(path);
        cmd.run()
    }

    #[test]
    fn test_validate_valid_config() {
        let valid_config = r#"{
            "version": 1,
            "server": {"host": "127.0.0.1", "port": 8080},
            "defaults": {"upstream_timeout_ms": 10000, "max_body_bytes": 1048576},
            "routes": []
        }"#;
        assert_eq!(run_validate_on_content(valid_config), ExitCode::SUCCESS);
    }

    #[test]
    fn test_validate_invalid_config() {
        let invalid_config = r#"{
            "version": 999,
            "server": {"host": "127.0.0.1", "port": 8080},
            "defaults": {"upstream_timeout_ms": 10000, "max_body_bytes": 1048576},
            "routes": []
        }"#;
        assert_eq!(run_validate_on_content(invalid_config), ExitCode::from(1));
    }

    #[test]
    fn test_validate_invalid_json() {
        let invalid_json = "not valid json at all";
        assert_eq!(run_validate_on_content(invalid_json), ExitCode::from(1));
    }

    #[test]
    fn test_validate_nonexistent_file() {
        let cmd = ValidateCommand::new(PathBuf::from("/nonexistent/path/config.json"));
        assert_eq!(cmd.run(), ExitCode::from(1));
    }
}
