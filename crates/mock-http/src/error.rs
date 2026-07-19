//! HTTP server error types.

use thiserror::Error;

/// Errors that can occur in the HTTP server.
#[derive(Debug, Error)]
pub enum HttpError {
    /// Failed to bind to the specified address.
    #[error("failed to bind to {host}:{port}: {source}")]
    BindError {
        host: String,
        port: u16,
        #[source]
        source: std::io::Error,
    },

    /// The request body exceeded the configured limit.
    #[error("request body too large: {size} bytes exceeds limit of {limit} bytes")]
    BodyTooLarge { size: usize, limit: usize },

    /// Failed to read the request body.
    #[error("failed to read request body")]
    BodyReadError,

    /// Failed to parse the request body as UTF-8 text.
    #[error("request body is not valid UTF-8: {0}")]
    BodyEncodingError(#[from] std::string::FromUtf8Error),

    /// Failed to parse the request body as JSON.
    #[error("request body is not valid JSON: {0}")]
    BodyJsonError(#[from] serde_json::Error),

    /// The server was stopped.
    #[error("server stopped: {0}")]
    ServerStopped(String),

    /// Shutdown timeout expired while draining in-flight requests.
    #[error("shutdown timed out waiting for in-flight requests to complete")]
    ShutdownTimeout,
}

/// Errors from upstream forwarding.
#[derive(Debug, Error)]
pub enum UpstreamError {
    /// Connection failed.
    #[error("connection failed: {0}")]
    ConnectionError(#[source] reqwest::Error),

    /// Request timeout.
    #[error("upstream request timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    /// Invalid URL.
    #[error("invalid upstream URL: {0}")]
    InvalidUrl(String),

    /// Request failed with a non-success status code.
    #[error("upstream returned error status {status}: {reason}")]
    UpstreamStatus { status: u16, reason: String },

    /// Invalid response from upstream.
    #[error("invalid upstream response: {0}")]
    InvalidResponse(String),
}

impl UpstreamError {
    /// Returns a 502 Bad Gateway response for this error.
    pub fn to_502_response(&self) -> mock_core::ResponseData {
        mock_core::ResponseData::new(502).with_body(mock_core::BodyData::Text(self.to_string()))
    }
}
