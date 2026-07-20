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
    long_about = "mock-api serves as a configurable HTTP mock server: load a JSON \
                  rule config, match incoming requests against the rules, and either \
                  return a local canned response or transparently forward to an \
                  upstream API with optional JSON-Pointer transforms.\n\n\
                  The same engine is also published as a WASM library for browser \
                  and Node.js reuse (see the `mock-wasm` crate).",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Validate a mock-api configuration file (parse + semantic checks).
    #[command(long_about = "Parse and validate a JSON configuration file without \
                            starting a server. Exits 0 on success, 1 on any error.\n\n\
                            Example:\n  \
                            mock-api validate --config examples/basic.json")]
    Validate {
        /// Path to the configuration file to validate.
        #[arg(short, long, value_name = "PATH")]
        config: PathBuf,
    },
    /// Run the mock API server (foreground, blocks until Ctrl-C).
    #[command(long_about = "Start the mock API server using the rules in the given \
                            JSON config file. The server watches the config file for \
                            changes and hot-reloads on every successful save.\n\n\
                            Examples:\n  \
                            mock-api run --config examples/basic.json\n  \
                            mock-api run -c examples/basic.json --mode tui\n  \
                            mock-api run -c examples/proxy.json --log-format json")]
    Run {
        /// Path to the configuration file (required).
        #[arg(short, long, value_name = "PATH")]
        config: Option<PathBuf>,
        /// Log format. `pretty` for human-readable colored output, `json` for
        /// line-delimited JSON suitable for log aggregators.
        #[arg(long, value_enum, default_value_t = commands::run::LogFormat::Pretty)]
        log_format: commands::run::LogFormat,
        /// Run mode. `standard` runs the server in the foreground with structured
        /// logs to stderr (production/CI). `tui` runs the same server under an
        /// interactive Ratatui interface showing live request history (local dev).
        #[arg(long, value_enum, default_value_t = commands::run::RunMode::Standard)]
        mode: commands::run::RunMode,
    },
    /// Generate a JSON schema for configuration files.
    #[command(
        long_about = "Print (or write to a file) a JSON Schema that describes \
                            the configuration file format. Useful for editor \
                            autocompletion and config validation in CI.\n\n\
                            Example:\n  \
                            mock-api schema --output mock-api.schema.json"
    )]
    Schema {
        /// Output file path. If omitted, the schema is written to stdout.
        #[arg(short, long, value_name = "PATH")]
        output: Option<PathBuf>,
    },
    /// Print the effective configuration after applying defaults.
    #[command(
        long_about = "Parse the given config, apply server-side defaults, and \
                            print the resolved configuration as pretty JSON. Useful \
                            for debugging \"what did the engine actually see?\".\n\n\
                            Example:\n  \
                            mock-api print-effective-config --config examples/basic.json"
    )]
    PrintEffectiveConfig {
        /// Path to the configuration file to read.
        #[arg(short, long, value_name = "PATH")]
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
        Commands::Run {
            config,
            log_format,
            mode,
        } => {
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
