//! Runtime event types for the mock HTTP server.
//!
//! These events are emitted to the mock-runtime event channel and can be
//! used for logging, metrics, or UI updates.

use serde::{Deserialize, Serialize};

/// Runtime events emitted by the mock HTTP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeEvent {
    /// Server has started listening.
    ServerStarted {
        /// The address the server is bound to.
        address: String,
    },
    /// Server has stopped.
    ServerStopped,
    /// Configuration was successfully reloaded.
    ConfigReloaded {
        /// Number of rules in the new configuration.
        rule_count: usize,
    },
    /// Configuration reload failed.
    ConfigReloadFailed {
        /// Error message describing the failure.
        message: String,
    },
    /// A request has started processing.
    RequestStarted {
        /// Unique identifier for this request.
        request_id: String,
        /// Summary of the incoming request.
        summary: RequestSummary,
    },
    /// A request has completed successfully.
    RequestCompleted {
        /// Unique identifier for this request.
        request_id: String,
        /// Result of the request processing.
        result: RequestResult,
    },
    /// A request failed to process.
    RequestFailed {
        /// Unique identifier for this request.
        request_id: String,
        /// Error message describing the failure.
        message: String,
    },
}

/// Summary information about an incoming request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestSummary {
    /// HTTP method (GET, POST, etc.).
    pub method: String,
    /// Request path.
    pub path: String,
    /// ID of the matched rule, if any.
    pub rule_id: Option<String>,
    /// Upstream URL for forward decisions, if known.
    pub upstream_url: Option<String>,
}

/// Result of a completed request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestResult {
    /// HTTP status code of the response.
    pub status: u16,
    /// Time taken to process the request in milliseconds.
    pub elapsed_ms: u64,
    /// Type of decision that was made.
    pub decision_type: DecisionType,
}

/// The type of decision made by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionType {
    /// Request was handled by a mock response.
    Mock,
    /// Request was forwarded to an upstream service.
    Forward,
    /// Request was rejected.
    Reject,
}

impl DecisionType {
    /// Returns the decision type name.
    pub fn as_str(&self) -> &'static str {
        match self {
            DecisionType::Mock => "mock",
            DecisionType::Forward => "forward",
            DecisionType::Reject => "reject",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_type_as_str() {
        assert_eq!(DecisionType::Mock.as_str(), "mock");
        assert_eq!(DecisionType::Forward.as_str(), "forward");
        assert_eq!(DecisionType::Reject.as_str(), "reject");
    }

    #[test]
    fn test_runtime_event_serialization() {
        let event = RuntimeEvent::ServerStarted {
            address: "127.0.0.1:8080".to_string(),
        };
        let json = serde_json::to_string(&event).unwrap();
        // serde uses PascalCase by default for enum variants
        assert!(json.contains("ServerStarted"));
        assert!(json.contains("127.0.0.1:8080"));
    }

    #[test]
    fn test_request_summary_serialization() {
        let summary = RequestSummary {
            method: "GET".to_string(),
            path: "/users/123".to_string(),
            rule_id: Some("get-user".to_string()),
            upstream_url: None,
        };
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("GET"));
        assert!(json.contains("/users/123"));
        assert!(json.contains("get-user"));
    }

    #[test]
    fn test_request_result_serialization() {
        let result = RequestResult {
            status: 200,
            elapsed_ms: 42,
            decision_type: DecisionType::Mock,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("200"));
        assert!(json.contains("42"));
        assert!(json.contains("mock"));
    }
}
