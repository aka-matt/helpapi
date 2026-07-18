use serde::{Deserialize, Serialize};

/// Metadata about a decision made by the mock engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionMetadata {
    /// The matched route ID, if any.
    pub route_id: Option<String>,
    /// Descriptions of which matchers matched.
    pub matcher_results: Vec<String>,
    /// Descriptions of transforms that were applied.
    pub transforms_applied: Vec<String>,
    /// Time elapsed processing the request in microseconds.
    pub elapsed_us: u64,
}

impl DecisionMetadata {
    /// Creates a new DecisionMetadata.
    pub fn new() -> Self {
        Self {
            route_id: None,
            matcher_results: Vec::new(),
            transforms_applied: Vec::new(),
            elapsed_us: 0,
        }
    }

    /// Sets the route ID.
    pub fn with_route_id(mut self, route_id: impl Into<String>) -> Self {
        self.route_id = Some(route_id.into());
        self
    }

    /// Adds a matcher result description.
    pub fn with_matcher_result(mut self, result: impl Into<String>) -> Self {
        self.matcher_results.push(result.into());
        self
    }

    /// Adds a transform description.
    pub fn with_transform_applied(mut self, transform: impl Into<String>) -> Self {
        self.transforms_applied.push(transform.into());
        self
    }

    /// Sets the elapsed time in microseconds.
    pub fn with_elapsed(mut self, elapsed_us: u64) -> Self {
        self.elapsed_us = elapsed_us;
        self
    }
}

impl Default for DecisionMetadata {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decision_metadata_new() {
        let meta = DecisionMetadata::new();
        assert!(meta.route_id.is_none());
        assert!(meta.matcher_results.is_empty());
        assert!(meta.transforms_applied.is_empty());
        assert_eq!(meta.elapsed_us, 0);
    }

    #[test]
    fn test_decision_metadata_with_route_id() {
        let meta = DecisionMetadata::new().with_route_id("get-user");
        assert_eq!(meta.route_id, Some("get-user".to_string()));
    }

    #[test]
    fn test_decision_metadata_with_matcher_result() {
        let meta = DecisionMetadata::new()
            .with_matcher_result("method: GET")
            .with_matcher_result("path: /users/:id");
        assert_eq!(meta.matcher_results.len(), 2);
    }

    #[test]
    fn test_decision_metadata_with_transform() {
        let meta = DecisionMetadata::new().with_transform_applied("jq: .user.name");
        assert_eq!(meta.transforms_applied.len(), 1);
    }

    #[test]
    fn test_decision_metadata_with_elapsed() {
        let meta = DecisionMetadata::new().with_elapsed(1500);
        assert_eq!(meta.elapsed_us, 1500);
    }
}
