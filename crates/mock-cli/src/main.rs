//! CLI entry point for mock-api.

mod commands;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(
    name = "mock-api",
    about = "A mock API CLI tool with WASM export capability",
    version = "0.1.0"
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
        Commands::Run { config: _ } => {
            println!("Run command not yet implemented");
            ExitCode::SUCCESS
        }
        Commands::Schema { output } => {
            let schema = mock_config::generate_schema();
            let json = serde_json::to_string_pretty(&schema).unwrap();
            if let Some(path) = output {
                std::fs::write(&path, &json).unwrap();
            } else {
                println!("{}", json);
            }
            ExitCode::SUCCESS
        }
        Commands::PrintEffectiveConfig { config: _ } => {
            println!("PrintEffectiveConfig command not yet implemented");
            ExitCode::SUCCESS
        }
        Commands::Version => {
            println!("mock-api version 0.1.0");
            ExitCode::SUCCESS
        }
    }
}
