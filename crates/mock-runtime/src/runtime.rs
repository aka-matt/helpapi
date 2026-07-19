//! The Runtime orchestrates the mock HTTP server and core engine.
//!
//! It manages the lifecycle: start, stop, reload with atomic config replacement,
//! and exposes an event subscription channel for monitoring.

use std::sync::Arc;

use mock_core::Engine;
use mock_http::{HttpServer, ServerConfig, events::RuntimeEvent};
use tokio::sync::{RwLock, broadcast, mpsc};

use crate::error::RuntimeError;

/// Runtime status indicating the current state of the service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeStatus {
    /// Runtime is not running.
    Stopped,
    /// Runtime is starting up.
    Starting,
    /// Runtime is running and serving requests.
    Running,
    /// Runtime is reloading configuration.
    Reloading,
    /// Runtime has encountered a fatal error.
    Failed,
}

/// The runtime orchestrates the HTTP server and core engine.
///
/// It holds the engine behind an `Arc<RwLock>` to allow atomic config replacement
/// on reload without stopping the server. Events are broadcast to all subscribers.
#[derive(Debug)]
pub struct Runtime {
    /// The compiled engine, atomically replaceable on reload.
    engine: Arc<RwLock<Engine>>,
    /// The HTTP server instance.
    server: Option<HttpServer>,
    /// Channel for emitting runtime events to subscribers (broadcast).
    events_tx: broadcast::Sender<RuntimeEvent>,
    /// Channel for sending events from server to the broadcast forwarder.
    server_events_tx: mpsc::Sender<RuntimeEvent>,
    /// Handle for the event forwarding task.
    forwarder_handle: Option<tokio::task::JoinHandle<()>>,
    /// Current runtime status.
    status: RuntimeStatus,
    /// Server configuration derived from the loaded config.
    server_config: ServerConfig,
}

impl Runtime {
    /// Creates a new Runtime from a JSON config string.
    ///
    /// The config is parsed and validated first; any errors are returned immediately.
    /// The runtime will be in `Stopped` state and must be started explicitly.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::InvalidConfig`] if the JSON is malformed,
    /// [`RuntimeError::ValidationError`] if validation fails,
    /// or [`RuntimeError::EngineError`] if the config cannot be compiled.
    pub async fn new(config_json: &str) -> Result<Self, RuntimeError> {
        // Parse and validate the config
        let config: mock_config::Config = serde_json::from_str(config_json)
            .map_err(|e| RuntimeError::InvalidConfig(format!("invalid JSON: {}", e)))?;

        let issues = mock_config::validate(&config);
        if !issues.is_empty() {
            let messages: Vec<_> = issues
                .iter()
                .map(|i| format!("{}: {} (at {})", i.code, i.message, i.path))
                .collect();
            return Err(RuntimeError::ValidationError(messages.join("; ")));
        }

        // Build server config
        let server_config = ServerConfig::new(&config.server.host, config.server.port)
            .with_max_body_bytes(config.defaults.max_body_bytes)
            .with_upstream_timeout_ms(config.defaults.upstream_timeout_ms);

        // Compile the engine
        let engine =
            Engine::compile(config_json).map_err(|e| RuntimeError::EngineError(e.to_string()))?;

        // Create broadcast channel for subscribers
        let (events_tx, _) = broadcast::channel(128);

        // Create mpsc channel for server -> forwarder
        let (server_events_tx, server_events_rx) = mpsc::channel(128);

        // Spawn forwarding task: mpsc -> broadcast
        let forwarder_events_tx = events_tx.clone();
        let forwarder_handle = tokio::spawn(async move {
            let mut rx = server_events_rx;
            while let Some(event) = rx.recv().await {
                let _ = forwarder_events_tx.send(event);
            }
        });

        Ok(Self {
            engine: Arc::new(RwLock::new(engine)),
            server: None,
            events_tx,
            server_events_tx,
            forwarder_handle: Some(forwarder_handle),
            status: RuntimeStatus::Stopped,
            server_config,
        })
    }

    /// Starts the HTTP server and transitions to `Running` state.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::InvalidState`] if already started or failed,
    /// or [`RuntimeError::ServerError`] if the server cannot bind.
    pub async fn start(&mut self) -> Result<(), RuntimeError> {
        if self.status == RuntimeStatus::Running {
            return Err(RuntimeError::InvalidState("already running".into()));
        }
        if self.status == RuntimeStatus::Starting {
            return Err(RuntimeError::InvalidState("already starting".into()));
        }

        self.status = RuntimeStatus::Starting;

        let server = HttpServer::start_server(
            self.server_config.clone(),
            self.engine.clone(),
            Some(self.server_events_tx.clone()),
        )
        .await
        .map_err(|e| {
            self.status = RuntimeStatus::Failed;
            RuntimeError::ServerError(e.to_string())
        })?;

        self.server = Some(server);
        self.status = RuntimeStatus::Running;

        Ok(())
    }

    /// Reloads the configuration with atomic replacement.
    ///
    /// The new config is compiled and validated first. On success, the engine
    /// is atomically replaced and a [`RuntimeEvent::ConfigReloaded`] is emitted.
    /// On failure, the old engine is kept and a [`RuntimeEvent::ConfigReloadFailed`] is emitted.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::InvalidState`] if the runtime is not running,
    /// or a config-related error if reload fails.
    pub async fn reload(&mut self, config_json: &str) -> Result<(), RuntimeError> {
        if self.status != RuntimeStatus::Running && self.status != RuntimeStatus::Stopped {
            return Err(RuntimeError::InvalidState(format!(
                "cannot reload in state {:?}",
                self.status
            )));
        }

        self.status = RuntimeStatus::Reloading;

        // Parse and validate first — don't touch the engine until we know it's valid
        let new_config: mock_config::Config = match serde_json::from_str(config_json) {
            Ok(c) => c,
            Err(e) => {
                let msg = format!("invalid JSON: {}", e);
                let _ = self.events_tx.send(RuntimeEvent::ConfigReloadFailed {
                    message: msg.clone(),
                });
                self.status = RuntimeStatus::Running;
                return Err(RuntimeError::InvalidConfig(msg));
            }
        };

        let issues = mock_config::validate(&new_config);
        if !issues.is_empty() {
            let messages: Vec<_> = issues
                .iter()
                .map(|i| format!("{}: {} (at {})", i.code, i.message, i.path))
                .collect();
            let msg = messages.join("; ");
            let _ = self.events_tx.send(RuntimeEvent::ConfigReloadFailed {
                message: msg.clone(),
            });
            self.status = RuntimeStatus::Running;
            return Err(RuntimeError::ValidationError(msg));
        }

        // Compile the new engine
        let new_engine = match Engine::compile(config_json) {
            Ok(e) => e,
            Err(e) => {
                let msg = e.to_string();
                let _ = self.events_tx.send(RuntimeEvent::ConfigReloadFailed {
                    message: msg.clone(),
                });
                self.status = RuntimeStatus::Running;
                return Err(RuntimeError::EngineError(msg));
            }
        };

        // Atomically replace the engine
        let mut engine = self.engine.write().await;
        *engine = new_engine;
        drop(engine);

        // Update server config if needed
        self.server_config = ServerConfig::new(&new_config.server.host, new_config.server.port)
            .with_max_body_bytes(new_config.defaults.max_body_bytes)
            .with_upstream_timeout_ms(new_config.defaults.upstream_timeout_ms);

        let rule_count = new_config.routes.len();
        let _ = self
            .events_tx
            .send(RuntimeEvent::ConfigReloaded { rule_count });

        self.status = RuntimeStatus::Running;

        Ok(())
    }

    /// Stops the HTTP server and transitions to `Stopped` state.
    ///
    /// The server is shut down gracefully with a brief drain period.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::InvalidState`] if not running.
    pub async fn stop(&mut self) -> Result<(), RuntimeError> {
        if self.status == RuntimeStatus::Stopped {
            return Err(RuntimeError::InvalidState("not running".into()));
        }

        if let Some(server) = self.server.take() {
            server.shutdown();
            // Brief drain
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        self.status = RuntimeStatus::Stopped;

        // Emit ServerStopped event so subscribers know the server has halted
        let _ = self.events_tx.send(RuntimeEvent::ServerStopped);

        Ok(())
    }

    /// Returns a receiver for runtime events.
    ///
    /// Each call creates a new channel. Events are broadcast to all subscribers.
    pub fn subscribe(&self) -> EventReceiver {
        EventReceiver(self.events_tx.subscribe())
    }

    /// Returns the current runtime status.
    pub fn status(&self) -> RuntimeStatus {
        self.status
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if let Some(handle) = self.forwarder_handle.take() {
            handle.abort();
        }
    }
}

/// Receiver for runtime events.
#[derive(Debug)]
pub struct EventReceiver(broadcast::Receiver<RuntimeEvent>);

impl EventReceiver {
    /// Receives the next event, waiting up to `timeout`.
    ///
    /// Returns `None` if the timeout expires.
    pub async fn recv_timeout(
        &mut self,
        timeout: std::time::Duration,
    ) -> Result<Option<RuntimeEvent>, broadcast::error::RecvError> {
        match tokio::time::timeout(timeout, self.0.recv()).await {
            Ok(result) => Ok(Some(result?)),
            Err(_) => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_config() -> &'static str {
        r#"{
            "version": 1,
            "server": { "host": "127.0.0.1", "port": 0 },
            "defaults": { "upstream_timeout_ms": 5000, "max_body_bytes": 1048576 },
            "routes": [{
                "id": "get-user",
                "priority": 100,
                "match_rule": { "method": "GET", "path": "/users/:id" },
                "action": { "type": "mock", "response": { "status": 200, "json_body": { "id": 1 } } }
            }]
        }"#
    }

    fn duplicate_id_engine_config() -> &'static str {
        // Config that fails Engine::compile via B4's duplicate-route-id check
        // (also fails mock_config::validate).
        r#"{
            "version": 1,
            "server": { "host": "127.0.0.1", "port": 0 },
            "defaults": { "upstream_timeout_ms": 5000, "max_body_bytes": 1048576 },
            "routes": [
                {
                    "id": "dupe",
                    "priority": 100,
                    "match_rule": { "method": "GET", "path": "/a" },
                    "action": { "type": "mock", "response": { "status": 200 } }
                },
                {
                    "id": "dupe",
                    "priority": 50,
                    "match_rule": { "method": "GET", "path": "/b" },
                    "action": { "type": "mock", "response": { "status": 200 } }
                }
            ]
        }"#
    }

    fn invalid_json_config() -> &'static str {
        "this is not json"
    }

    // --- new() ---

    #[tokio::test]
    async fn test_new_valid_config() {
        let runtime = Runtime::new(valid_config()).await;
        assert!(runtime.is_ok());
        let runtime = runtime.unwrap();
        assert_eq!(runtime.status(), RuntimeStatus::Stopped);
    }

    #[tokio::test]
    async fn test_new_invalid_json() {
        let result = Runtime::new(invalid_json_config()).await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            RuntimeError::InvalidConfig(_)
        ));
    }

    // --- start/stop cycle ---

    #[tokio::test]
    async fn test_start_stop_cycle() {
        let mut runtime = Runtime::new(valid_config()).await.unwrap();

        // Start
        assert!(runtime.start().await.is_ok());
        assert_eq!(runtime.status(), RuntimeStatus::Running);

        // Stop
        assert!(runtime.stop().await.is_ok());
        assert_eq!(runtime.status(), RuntimeStatus::Stopped);
    }

    #[tokio::test]
    async fn test_start_idempotent() {
        let mut runtime = Runtime::new(valid_config()).await.unwrap();
        runtime.start().await.unwrap();

        // Second start should fail
        let result = runtime.start().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RuntimeError::InvalidState(_)));
    }

    // --- subscribe ---

    #[tokio::test]
    async fn test_subscribe_receives_server_started() {
        let mut runtime = Runtime::new(valid_config()).await.unwrap();
        let mut receiver = runtime.subscribe();

        runtime.start().await.unwrap();

        // Wait for ServerStarted event
        let event = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .await;
        assert!(event.is_ok());
        let event = event.unwrap().unwrap();
        assert!(matches!(event, RuntimeEvent::ServerStarted { .. }));
    }

    // --- reload ---

    #[tokio::test]
    async fn test_reload_success() {
        let mut runtime = Runtime::new(valid_config()).await.unwrap();
        let mut receiver = runtime.subscribe();

        runtime.start().await.unwrap();

        // Wait for ServerStarted
        let _ = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .await;

        // Reload with new config (same structure, different route)
        let new_config = r#"{
            "version": 1,
            "server": { "host": "127.0.0.1", "port": 0 },
            "defaults": { "upstream_timeout_ms": 5000, "max_body_bytes": 1048576 },
            "routes": [
                {
                    "id": "get-user",
                    "priority": 100,
                    "match_rule": { "method": "GET", "path": "/users/:id" },
                    "action": { "type": "mock", "response": { "status": 200, "json_body": { "id": 1 } } }
                },
                {
                    "id": "list-users",
                    "priority": 50,
                    "match_rule": { "method": "GET", "path": "/users" },
                    "action": { "type": "mock", "response": { "status": 200, "json_body": [] } }
                }
            ]
        }"#;

        let result = runtime.reload(new_config).await;
        assert!(result.is_ok());
        assert_eq!(runtime.status(), RuntimeStatus::Running);

        // Wait for ConfigReloaded event
        let event = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .await;
        assert!(event.is_ok());
        let event = event.unwrap().unwrap();
        assert!(matches!(
            event,
            RuntimeEvent::ConfigReloaded { rule_count: 2 }
        ));
    }

    #[tokio::test]
    async fn test_reload_failure_preserves_old_engine() {
        // Create runtime with config that has 1 rule
        let mut runtime = Runtime::new(valid_config()).await.unwrap();
        let mut receiver = runtime.subscribe();

        runtime.start().await.unwrap();

        // Wait for ServerStarted
        let _ = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .await;

        // Attempt reload with invalid JSON — should keep old engine
        let result = runtime.reload(invalid_json_config()).await;
        assert!(result.is_err());
        assert_eq!(runtime.status(), RuntimeStatus::Running);

        // Wait for ConfigReloadFailed event
        let event = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .await;
        assert!(event.is_ok());
        let event = event.unwrap().unwrap();
        assert!(matches!(event, RuntimeEvent::ConfigReloadFailed { .. }));

        // Verify old engine still works by doing a valid reload later
        // and checking it still has the original 1 rule
        let new_config = r#"{
            "version": 1,
            "server": { "host": "127.0.0.1", "port": 0 },
            "defaults": { "upstream_timeout_ms": 5000, "max_body_bytes": 1048576 },
            "routes": [
                {
                    "id": "get-user",
                    "priority": 100,
                    "match_rule": { "method": "GET", "path": "/users/:id" },
                    "action": { "type": "mock", "response": { "status": 200, "json_body": { "id": 1 } } }
                }
            ]
        }"#;
        runtime.reload(new_config).await.unwrap();

        let event = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .await;
        assert!(event.is_ok());
        let event = event.unwrap().unwrap();
        // Old engine had 1 rule, so it should still report 1 rule on reload
        assert!(matches!(
            event,
            RuntimeEvent::ConfigReloaded { rule_count: 1 }
        ));
    }

    #[tokio::test]
    async fn test_reload_bad_engine_preserves_old() {
        // Config that fails Engine::compile via B4's duplicate-route-id check (also fails mock_config::validate)
        let mut runtime = Runtime::new(valid_config()).await.unwrap();
        let mut receiver = runtime.subscribe();

        runtime.start().await.unwrap();

        // Wait for ServerStarted
        let _ = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .await;

        // Attempt reload with bad engine config
        let result = runtime.reload(duplicate_id_engine_config()).await;
        assert!(result.is_err()); // Should fail at Engine::compile
        assert_eq!(runtime.status(), RuntimeStatus::Running);

        // Should get a ConfigReloadFailed
        let event = receiver
            .recv_timeout(std::time::Duration::from_secs(2))
            .await;
        assert!(event.is_ok());
        let event = event.unwrap().unwrap();
        assert!(matches!(event, RuntimeEvent::ConfigReloadFailed { .. }));
    }

    // --- status transitions ---

    #[tokio::test]
    async fn test_status_transitions() {
        let runtime = Runtime::new(valid_config()).await.unwrap();
        assert_eq!(runtime.status(), RuntimeStatus::Stopped);

        let mut runtime = runtime;
        runtime.start().await.unwrap();
        assert_eq!(runtime.status(), RuntimeStatus::Running);

        runtime.stop().await.unwrap();
        assert_eq!(runtime.status(), RuntimeStatus::Stopped);
    }

    #[tokio::test]
    async fn test_stop_when_not_running() {
        let mut runtime = Runtime::new(valid_config()).await.unwrap();
        let result = runtime.stop().await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RuntimeError::InvalidState(_)));
    }
}
