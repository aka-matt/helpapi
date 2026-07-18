//! Error types for mock-config.

use thiserror::Error;

/// Errors that can occur when parsing or validating configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// JSON parsing error.
    #[error("failed to parse JSON: {0}")]
    Parse(#[from] serde_json::Error),

    /// I/O error when reading configuration files.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Validation error with a list of issues.
    #[error("validation failed: {0}")]
    Validation(String),
}

impl ConfigError {
    /// Creates a validation error from a list of issues.
    pub fn validation<S: Into<String>>(message: S) -> Self {
        ConfigError::Validation(message.into())
    }
}
