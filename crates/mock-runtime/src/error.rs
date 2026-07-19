//! Error types for the runtime.

use thiserror::Error;

/// Errors that can occur during runtime operations.
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// Configuration is invalid or cannot be compiled.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),

    /// Configuration validation failed.
    #[error("validation error: {0}")]
    ValidationError(String),

    /// Failed to start the HTTP server.
    #[error("server error: {0}")]
    ServerError(String),

    /// Runtime is in an invalid state for the requested operation.
    #[error("invalid state: {0}")]
    InvalidState(String),

    /// Compilation of the core engine failed.
    #[error("engine error: {0}")]
    EngineError(String),
}
