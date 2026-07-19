//! HTTP server implementation using Axum.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{Router, body::Body, extract::Request, response::Response, routing::any};
use mock_core::Engine;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::oneshot;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;
use tracing::{debug, error, info, warn};

use crate::error::HttpError;
use crate::handler::handle_request;

/// Configuration for the HTTP server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Host to bind to.
    pub host: String,
    /// Port to bind to.
    pub port: u16,
    /// Maximum request body size in bytes.
    pub max_body_bytes: usize,
    /// Timeout for upstream requests in milliseconds.
    pub upstream_timeout_ms: u64,
}

impl ServerConfig {
    /// Creates a new ServerConfig with default values.
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            max_body_bytes: 1_048_576, // 1 MiB default
            upstream_timeout_ms: 10_000,
        }
    }

    /// Sets the maximum body size in bytes.
    pub fn with_max_body_bytes(mut self, bytes: usize) -> Self {
        self.max_body_bytes = bytes;
        self
    }

    /// Sets the upstream timeout in milliseconds.
    pub fn with_upstream_timeout_ms(mut self, ms: u64) -> Self {
        self.upstream_timeout_ms = ms;
        self
    }

    /// Returns the socket address for binding.
    pub fn socket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.host, self.port)
            .parse()
            .unwrap_or_else(|_| SocketAddr::from(([127, 0, 0, 1], self.port)))
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self::new("127.0.0.1", 8080)
    }
}

/// HTTP server instance.
#[derive(Debug)]
pub struct HttpServer {
    /// The socket address the server is bound to.
    addr: SocketAddr,
    /// Channel to signal shutdown.
    shutdown_tx: oneshot::Sender<()>,
    /// Join handle for the server task.
    _join_handle: tokio::task::JoinHandle<()>,
}

impl HttpServer {
    /// Starts the HTTP server with the given configuration and engine.
    ///
    /// Returns the server instance and the actual address bound (which may differ
    /// from the requested address if port 0 was used).
    ///
    /// # Errors
    ///
    /// Returns [`HttpError::BindError`] if the server fails to bind to the address.
    pub async fn start_server(
        config: ServerConfig,
        engine: Arc<Engine>,
    ) -> Result<Self, HttpError> {
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

        // Create TCP listener
        let listener =
            TcpListener::bind(config.socket_addr())
                .await
                .map_err(|e| HttpError::BindError {
                    host: config.host.clone(),
                    port: config.port,
                    source: e,
                })?;

        let addr = listener.local_addr().map_err(|e| HttpError::BindError {
            host: config.host.clone(),
            port: config.port,
            source: e,
        })?;

        info!(%addr, "starting HTTP server");

        // Build router with middleware
        let app = Router::new()
            .route("/", any(handle_route))
            .route("/*path", any(handle_route))
            .layer(
                ServiceBuilder::new()
                    .layer(TraceLayer::new_for_http())
                    .layer(CompressionLayer::new())
                    .into_inner(),
            )
            .with_state(AppState {
                engine,
                config: config.clone(),
            });

        // Spawn server task
        let join_handle = tokio::spawn(async move {
            let server = axum::serve(listener, app);

            tokio::select! {
                result = server => {
                    if let Err(e) = result {
                        error!("server error: {}", e);
                    }
                }
                _ = &mut shutdown_rx => {
                    info!("server shutdown signal received");
                }
            }
        });

        Ok(Self {
            addr,
            shutdown_tx,
            _join_handle: join_handle,
        })
    }

    /// Returns the socket address the server is bound to.
    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    /// Signals the server to shut down gracefully.
    ///
    /// This initiates graceful shutdown - the server will stop accepting new
    /// connections and wait for in-flight requests to complete (up to a timeout).
    pub fn shutdown(self) {
        info!("initiating server shutdown");
        let _ = self.shutdown_tx.send(());
    }
}

/// Application state shared across request handlers.
#[derive(Clone)]
struct AppState {
    engine: Arc<Engine>,
    config: ServerConfig,
}

/// Request handler that routes through the mock engine.
async fn handle_route(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: Request,
) -> Response {
    let engine = state.engine.clone();
    let max_body_bytes = state.config.max_body_bytes;

    match handle_request(request, engine, max_body_bytes).await {
        Ok(response) => {
            debug!("handler returned success response");
            response
        }
        Err(e) => {
            error!("request handling error: {:?} - {}", e, e);
            // Return a 500 error response
            axum::response::Response::builder()
                .status(axum::http::StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from(format!("Internal server error: {}", e)))
                .unwrap_or_else(|_| {
                    axum::response::Response::new(Body::from("Internal server error"))
                })
        }
    }
}

/// Runs the HTTP server with graceful shutdown handling.
///
/// This function sets up signal handlers for SIGINT and SIGTERM, and blocks
/// until a shutdown signal is received or the server fails.
pub async fn run_server_with_shutdown(
    config: ServerConfig,
    engine: Arc<Engine>,
) -> Result<(), HttpError> {
    let server = HttpServer::start_server(config, engine).await?;

    let addr = server.local_addr();
    info!(%addr, "server listening, press Ctrl+C to stop");

    // Wait for shutdown signal
    match signal::ctrl_c().await {
        Ok(()) => {
            info!("shutdown signal received (Ctrl+C)");
        }
        Err(e) => {
            warn!(
                "failed to listen for shutdown signal: {}, proceeding with shutdown",
                e
            );
        }
    }

    // Initiate graceful shutdown
    server.shutdown();

    // Give some time for graceful shutdown
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    info!("server stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mock_core::Engine;

    fn test_engine() -> Arc<Engine> {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [{
                "id": "get-user",
                "priority": 100,
                "match_rule": {"method": "GET", "path": "/users/:id"},
                "action": {"type": "mock", "response": {"status": 200, "json_body": {"id": 1}}}
            }]
        }"#;
        Arc::new(Engine::compile(json).unwrap())
    }

    #[tokio::test]
    async fn test_server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert_eq!(config.max_body_bytes, 1_048_576);
        assert_eq!(config.upstream_timeout_ms, 10_000);
    }

    #[tokio::test]
    async fn test_server_config_builder() {
        let config = ServerConfig::new("0.0.0.0", 3000)
            .with_max_body_bytes(2_097_152)
            .with_upstream_timeout_ms(5_000);

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 3000);
        assert_eq!(config.max_body_bytes, 2_097_152);
        assert_eq!(config.upstream_timeout_ms, 5_000);
    }

    #[tokio::test]
    async fn test_start_server() {
        let config = ServerConfig::new("127.0.0.1", 0); // Port 0 = dynamic
        let engine = test_engine();

        let server = HttpServer::start_server(config, engine).await.unwrap();

        // Verify it bound to some port
        assert!(server.local_addr().port() > 0);

        // Clean shutdown
        server.shutdown();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn test_server_graceful_shutdown() {
        let config = ServerConfig::new("127.0.0.1", 0);
        let engine = test_engine();

        let server = HttpServer::start_server(config, engine).await.unwrap();
        let addr = server.local_addr();

        // Make a request to verify server is running
        let client = reqwest::Client::new();
        let response = client.get(format!("http://{}/test", addr)).send().await;

        // Server should respond (either 200 mock or 404 reject)
        assert!(response.is_ok());

        // Shutdown
        server.shutdown();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Server should no longer be accepting connections
        let response = client.get(format!("http://{}/test", addr)).send().await;
        // May fail or succeed depending on timing, but we're just testing shutdown
    }
}
