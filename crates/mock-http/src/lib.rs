//! HTTP server layer using Axum for the mock API.
//!
//! This crate provides the HTTP server that receives incoming requests,
//! routes them through the mock-core Engine, and returns mock responses
//! or forwards requests to upstream services.
//!
//! ## Architecture
//!
//! - [`server::HttpServer`] - Main server instance with lifecycle management
//! - [`server::ServerConfig`] - Server configuration (host, port, limits)
//! - [`handler`] - Request handling, body reading, and response building
//! - [`client::UpstreamClient`] - HTTP client for forwarding requests to upstream
//! - [`error::HttpError`] - Error types for the HTTP layer
//!
//! ## Usage
//!
//! ```ignore
//! use mock_http::{HttpServer, ServerConfig};
//! use mock_core::Engine;
//! use std::sync::Arc;
//!
//! let config = ServerConfig::new("127.0.0.1", 8080);
//! let engine = Arc::new(Engine::compile(config_json)?);
//!
//! let server = HttpServer::start_server(config, engine).await?;
//! println!("Server listening on {}", server.local_addr());
//!
//! // When ready to shut down:
//! server.shutdown();
//! ```

pub mod client;
pub mod error;
pub mod events;
pub mod handler;
pub mod server;

// Re-exports for convenience
pub use client::UpstreamClient;
pub use error::{HttpError, UpstreamError};
pub use events::{DecisionType, RequestResult, RequestSummary, RuntimeEvent};
pub use server::{HttpServer, ServerConfig};
