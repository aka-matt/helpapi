use serde::{Deserialize, Serialize};

use super::forward::ForwardPlan;
use super::metadata::DecisionMetadata;
use super::response::ResponseData;

/// The result of the mock engine's decision for an incoming request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Decision {
    /// Return a locally-defined mock response.
    Mock {
        /// The mock response to return.
        response: ResponseData,
        /// Metadata about the decision.
        metadata: DecisionMetadata,
    },
    /// Forward the request to an upstream service.
    Forward {
        /// The forwarding plan describing the upstream request.
        plan: ForwardPlan,
        /// Metadata about the decision.
        metadata: DecisionMetadata,
    },
    /// Reject the request with an error response.
    Reject {
        /// The error response to return.
        response: ResponseData,
        /// Metadata about the decision.
        metadata: DecisionMetadata,
    },
}

impl Decision {
    /// Returns the metadata associated with this decision.
    pub fn metadata(&self) -> &DecisionMetadata {
        match self {
            Decision::Mock { metadata, .. } => metadata,
            Decision::Forward { metadata, .. } => metadata,
            Decision::Reject { metadata, .. } => metadata,
        }
    }

    /// Returns the route ID if a route was matched.
    pub fn route_id(&self) -> Option<&str> {
        self.metadata().route_id.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_mock_serialization() {
        let response = ResponseData::new(200)
            .with_body(super::super::body::BodyData::Json(serde_json::json!({"id": 1})));
        let metadata = DecisionMetadata::new()
            .with_route_id("get-user")
            .with_matcher_result("method: GET")
            .with_matcher_result("path: /users/:id");

        let decision = Decision::Mock { response, metadata };
        let json = serde_json::to_string(&decision).unwrap();

        assert!(json.contains("Mock"));
        assert!(json.contains("get-user"));
        assert!(json.contains("200"));

        // Verify round-trip
        let parsed: Decision = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Decision::Mock { .. }));
    }

    #[test]
    fn test_decision_forward_serialization() {
        let plan = ForwardPlan::new("https://api.example.com/users", "GET")
            .with_timeout(5000);
        let metadata = DecisionMetadata::new()
            .with_route_id("proxy-route")
            .with_matcher_result("method: GET")
            .with_transform_applied("jq: .user");

        let decision = Decision::Forward { plan, metadata };
        let json = serde_json::to_string(&decision).unwrap();

        assert!(json.contains("Forward"));
        assert!(json.contains("proxy-route"));
        assert!(json.contains("api.example.com"));

        // Verify round-trip
        let parsed: Decision = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Decision::Forward { .. }));
    }

    #[test]
    fn test_decision_reject_serialization() {
        let response = ResponseData::new(404)
            .with_body(super::super::body::BodyData::Text("Not found".to_string()));
        let metadata = DecisionMetadata::new()
            .with_route_id("get-user")
            .with_matcher_result("path: /users/:id");

        let decision = Decision::Reject { response, metadata };
        let json = serde_json::to_string(&decision).unwrap();

        assert!(json.contains("Reject"));
        assert!(json.contains("404"));
        assert!(json.contains("Not found"));

        // Verify round-trip
        let parsed: Decision = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, Decision::Reject { .. }));
    }

    #[test]
    fn test_decision_metadata_accessors() {
        let response = ResponseData::new(200);
        let metadata = DecisionMetadata::new()
            .with_route_id("test-route")
            .with_elapsed(500);

        let decision = Decision::Mock { response, metadata };
        assert_eq!(decision.route_id(), Some("test-route"));
        assert_eq!(decision.metadata().elapsed_us, 500);
    }

    #[test]
    fn test_decision_roundtrip_empty_response() {
        let decision = Decision::Mock {
            response: ResponseData::new(200),
            metadata: DecisionMetadata::new(),
        };

        let json = serde_json::to_string(&decision).unwrap();
        let parsed: Decision = serde_json::from_str(&json).unwrap();
        assert_eq!(decision, parsed);
    }
}
