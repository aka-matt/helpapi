//! Run command - starts the mock API server.

use crate::commands::Command;
use anyhow::Result;
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Log format option for the `run` subcommand.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum LogFormat {
    /// Human-readable logs with colors and aligned fields (default).
    #[default]
    Pretty,
    /// Line-delimited JSON logs, suitable for log aggregators.
    Json,
}

impl std::str::FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "pretty" => Ok(LogFormat::Pretty),
            "json" => Ok(LogFormat::Json),
            _ => Err(format!("unknown log format: {}", s)),
        }
    }
}

/// Run mode for the `run` subcommand.
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum RunMode {
    /// Run the server in the foreground with structured logs to stderr.
    /// Use this for production, CI, and when piping logs to a file.
    #[default]
    Standard,
    /// Run the server with an interactive Ratatui TUI showing live request
    /// history. Use this for local development and debugging.
    Tui,
}

impl std::str::FromStr for RunMode {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "standard" => Ok(RunMode::Standard),
            "tui" => Ok(RunMode::Tui),
            _ => Err(format!("unknown run mode: {}", s)),
        }
    }
}

/// Run command implementation.
pub struct RunCommand {
    /// Path to the configuration file.
    config_path: PathBuf,
    /// Log format.
    log_format: LogFormat,
    /// Run mode (standard or TUI).
    run_mode: RunMode,
}

impl RunCommand {
    /// Creates a new run command.
    pub fn new(config_path: PathBuf, log_format: LogFormat) -> Self {
        Self {
            config_path,
            log_format,
            run_mode: RunMode::Standard,
        }
    }

    /// Sets the run mode.
    pub fn with_run_mode(mut self, mode: RunMode) -> Self {
        self.run_mode = mode;
        self
    }

    /// Initializes the tracing subscriber based on log format and RUST_LOG env var.
    fn init_tracing(&self) {
        use tracing_subscriber::{EnvFilter, fmt, prelude::*};

        let env_filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        let subscriber = tracing_subscriber::registry().with(env_filter);

        match self.log_format {
            LogFormat::Pretty => {
                subscriber
                    .with(fmt::layer().with_target(true).with_thread_ids(true))
                    .init();
            }
            LogFormat::Json => {
                subscriber
                    .with(fmt::layer().with_target(true).with_thread_ids(true).json())
                    .init();
            }
        }
    }

    /// Reads and validates the config file.
    fn read_config(&self) -> Result<String> {
        let content = std::fs::read_to_string(&self.config_path).map_err(|e| {
            anyhow::anyhow!(
                "failed to read config file {}: {}",
                self.config_path.display(),
                e
            )
        })?;
        Ok(content)
    }

    /// Sets up file watching for auto-reload.
    async fn watch_config(&self, runtime: Arc<RwLock<mock_runtime::Runtime>>) -> Result<()> {
        let config_path = self.config_path.clone();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        let mut watcher = RecommendedWatcher::new(
            move |res: std::result::Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    if event.kind.is_modify() || event.kind.is_create() {
                        let _ = tx.blocking_send(());
                    }
                }
            },
            NotifyConfig::default().with_poll_interval(Duration::from_secs(1)),
        )
        .map_err(|e| anyhow::anyhow!("failed to create file watcher: {}", e))?;

        watcher
            .watch(&config_path, RecursiveMode::NonRecursive)
            .map_err(|e| anyhow::anyhow!("failed to watch config file: {}", e))?;

        info!(
            "watching config file for changes: {}",
            config_path.display()
        );

        loop {
            tokio::select! {
                _ = rx.recv() => {
                    info!("config file changed, reloading...");
                    match self.read_config() {
                        Ok(content) => {
                            let mut runtime = runtime.write().await;
                            match runtime.reload(&content).await {
                                Ok(()) => {
                                    info!("config reloaded successfully");
                                }
                                Err(e) => {
                                    error!("config reload failed, keeping old config: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            error!("failed to read config file, keeping old config: {}", e);
                        }
                    }
                }
                _ = signal::ctrl_c() => {
                    info!("received shutdown signal");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Runs the server until shutdown.
    async fn run_server(&self) -> Result<()> {
        self.init_tracing();

        let content = self.read_config()?;

        info!(
            "starting mock-api server with config: {}",
            self.config_path.display()
        );

        let runtime = Arc::new(RwLock::new(
            mock_runtime::Runtime::new(&content)
                .await
                .map_err(|e| anyhow::anyhow!("failed to create runtime: {}", e))?,
        ));

        {
            let mut runtime = runtime.write().await;
            runtime
                .start()
                .await
                .map_err(|e| anyhow::anyhow!("failed to start runtime: {}", e))?;
        }

        // Spawn file watcher
        let watcher_runtime = runtime.clone();
        let watcher_config = self.config_path.clone();
        let watcher_handle = tokio::spawn(async move {
            let watcher = RunCommand {
                config_path: watcher_config,
                log_format: LogFormat::Pretty, // Use pretty for watcher
                run_mode: RunMode::Standard,
            };
            if let Err(e) = watcher.watch_config(watcher_runtime).await {
                warn!("file watcher error: {}", e);
            }
        });

        // Wait for shutdown signal
        match signal::ctrl_c().await {
            Ok(()) => {
                info!("shutdown signal received");
            }
            Err(e) => {
                warn!(
                    "failed to listen for shutdown signal: {}, proceeding with shutdown",
                    e
                );
            }
        }

        // Stop the runtime
        {
            let mut runtime = runtime.write().await;
            if let Err(e) = runtime.stop().await {
                error!("error stopping runtime: {}", e);
            }
        }

        // Abort the watcher task
        watcher_handle.abort();

        info!("server stopped");
        Ok(())
    }

    /// Runs the server with TUI until shutdown.
    async fn run_server_tui(&self) -> Result<()> {
        // For TUI mode, we don't initialize standard tracing as the TUI takes over stderr
        let content = self.read_config()?;

        info!(
            "starting mock-api server with TUI and config: {}",
            self.config_path.display()
        );

        let runtime = Arc::new(RwLock::new(
            mock_runtime::Runtime::new(&content)
                .await
                .map_err(|e| anyhow::anyhow!("failed to create runtime: {}", e))?,
        ));

        {
            let mut runtime = runtime.write().await;
            runtime
                .start()
                .await
                .map_err(|e| anyhow::anyhow!("failed to start runtime: {}", e))?;
        }

        // Create and run the TUI app
        let tui_app = crate::tui::App::new(runtime.clone(), self.config_path.clone());

        // Spawn file watcher in background
        let watcher_runtime = runtime.clone();
        let watcher_config = self.config_path.clone();
        let _watcher_handle = tokio::spawn(async move {
            let watcher = RunCommand {
                config_path: watcher_config,
                log_format: LogFormat::Pretty,
                run_mode: RunMode::Tui,
            };
            if let Err(e) = watcher.watch_config(watcher_runtime).await {
                warn!("file watcher error: {}", e);
            }
        });

        // Run the TUI (this blocks until quit)
        if let Err(e) = tui_app.run().await {
            error!("TUI error: {}", e);
        }

        // Stop the runtime
        {
            let mut runtime = runtime.write().await;
            if let Err(e) = runtime.stop().await {
                error!("error stopping runtime: {}", e);
            }
        }

        info!("server stopped");
        Ok(())
    }
}

impl Command for RunCommand {
    fn run(&self) -> ExitCode {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| anyhow::anyhow!("failed to create runtime: {}", e));

        if let Err(e) = rt {
            eprintln!("Error: {}", e);
            return ExitCode::from(1);
        }

        let rt = rt.unwrap();

        let res = match self.run_mode {
            RunMode::Standard => rt.block_on(self.run_server()),
            RunMode::Tui => rt.block_on(self.run_server_tui()),
        };

        if let Err(e) = res {
            eprintln!("Error: {}", e);
            return ExitCode::from(1);
        }

        ExitCode::SUCCESS
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
            "server": {"host": "127.0.0.1", "port": 0},
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [{
                "id": "get-user",
                "priority": 100,
                "match_rule": {"method": "GET", "path": "/users/:id"},
                "action": {"type": "mock", "response": {"status": 200, "json_body": {"id": 1}}}
            }]
        }"#
    }

    #[test]
    fn test_log_format_parsing() {
        assert!(matches!(
            "pretty".parse::<LogFormat>(),
            Ok(LogFormat::Pretty)
        ));
        assert!(matches!("json".parse::<LogFormat>(), Ok(LogFormat::Json)));
        assert!("unknown".parse::<LogFormat>().is_err());
    }

    #[test]
    fn test_run_mode_parsing() {
        assert!(matches!(
            "standard".parse::<RunMode>(),
            Ok(RunMode::Standard)
        ));
        assert!(matches!("tui".parse::<RunMode>(), Ok(RunMode::Tui)));
        assert!("unknown".parse::<RunMode>().is_err());
    }

    #[test]
    fn test_run_command_with_valid_config() {
        let mut file = NamedTempFile::with_suffix(".json").unwrap();
        file.write_all(valid_config().as_bytes()).unwrap();
        let path = file.path().to_path_buf();

        // This test just verifies the command can be created and would parse
        let cmd = RunCommand::new(path, LogFormat::Pretty);
        assert!(cmd.config_path.exists());
    }

    #[test]
    fn test_run_command_with_nonexistent_config() {
        let path = PathBuf::from("/nonexistent/path/config.json");
        let cmd = RunCommand::new(path, LogFormat::Pretty);
        // Reading should fail
        assert!(cmd.read_config().is_err());
    }
}
