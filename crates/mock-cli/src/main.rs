//! CLI entry point for mock-api.

mod commands;
mod tui;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

use commands::Command;

#[derive(Parser, Debug)]
#[command(
    name = "mock-api",
    about = "A mock API CLI tool with WASM export capability",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Validate a mock-api configuration file.
    Validate {
        /// Path to the configuration file to validate.
        #[arg(short, long)]
        config: PathBuf,
    },
    /// Run the mock API server.
    Run {
        /// Path to the configuration file.
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Log format (pretty or json).
        #[arg(long, default_value = "pretty")]
        log_format: commands::run::LogFormat,
        /// Run mode (standard or tui).
        #[arg(long, default_value = "standard")]
        mode: commands::run::RunMode,
    },
    /// Generate a JSON schema for configuration files.
    Schema {
        /// Output file path (default: stdout).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Print the effective configuration after applying defaults.
    PrintEffectiveConfig {
        /// Path to the configuration file.
        #[arg(short, long)]
        config: PathBuf,
    },
    /// Show version information.
    Version,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Commands::Validate { config } => {
            let content = match std::fs::read_to_string(&config) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Error reading config file {}: {}", config.display(), e);
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
        Commands::Run { config, log_format, mode } => {
            use commands::run::RunCommand;

            let config_path = match config {
                Some(p) => p,
                None => {
                    eprintln!("Error: --config is required for run command");
                    return ExitCode::from(1);
                }
            };

            let cmd = RunCommand::new(config_path, log_format).with_run_mode(mode);
            cmd.run()
        }
        Commands::Schema { output } => {
            use commands::schema::SchemaCommand;

            let cmd = SchemaCommand::new(output);
            cmd.run()
        }
        Commands::PrintEffectiveConfig { config } => {
            use commands::print_config::PrintEffectiveConfigCommand;

            let cmd = PrintEffectiveConfigCommand::new(config);
            cmd.run()
        }
        Commands::Version => {
            println!("mock-api version {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
    }
}
