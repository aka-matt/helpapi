//! PrintEffectiveConfig command - prints the resolved configuration.

use crate::commands::Command;
use std::path::PathBuf;
use std::process::ExitCode;

/// PrintEffectiveConfig command implementation.
pub struct PrintEffectiveConfigCommand {
    /// Path to the configuration file.
    config_path: PathBuf,
}

impl PrintEffectiveConfigCommand {
    /// Creates a new print effective config command.
    pub fn new(config_path: PathBuf) -> Self {
        Self { config_path }
    }
}

impl Command for PrintEffectiveConfigCommand {
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

        // Parse and validate
        match mock_config::parse_and_validate(&content) {
            Ok(config) => {
                // Print the resolved config (which is the parsed Config struct)
                let json = serde_json::to_string_pretty(&config)
                    .map_err(|e| anyhow::anyhow!("failed to serialize config: {}", e));
                match json {
                    Ok(json) => {
                        println!("{}", json);
                        ExitCode::SUCCESS
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        ExitCode::from(1)
                    }
                }
            }
            Err(e) => {
                eprintln!("Config is invalid: {}", e);
                ExitCode::from(1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn valid_config() -> &'static str {
        r#"{
            "version": 1,
            "server": {"host": "127.0.0.1", "port": 8080},
            "defaults": {"upstream_timeout_ms": 10000, "max_body_bytes": 1048576},
            "routes": []
        }"#
    }

    #[test]
    fn test_print_effective_config_valid() {
        let mut file = NamedTempFile::with_suffix(".json").unwrap();
        file.write_all(valid_config().as_bytes()).unwrap();
        let path = file.path().to_path_buf();

        let cmd = PrintEffectiveConfigCommand::new(path);
        assert_eq!(cmd.run(), ExitCode::SUCCESS);
    }

    #[test]
    fn test_print_effective_config_invalid() {
        let mut file = NamedTempFile::with_suffix(".json").unwrap();
        file.write_all(b"not valid json").unwrap();
        let path = file.path().to_path_buf();

        let cmd = PrintEffectiveConfigCommand::new(path);
        assert_eq!(cmd.run(), ExitCode::from(1));
    }

    #[test]
    fn test_print_effective_config_nonexistent() {
        let path = PathBuf::from("/nonexistent/path/config.json");
        let cmd = PrintEffectiveConfigCommand::new(path);
        assert_eq!(cmd.run(), ExitCode::from(1));
    }
}
