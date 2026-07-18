use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::body::BodyData;

/// Represents an incoming HTTP request, host-agnostic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestData {
    /// HTTP method (e.g., "GET", "POST").
    pub method: String,
    /// Request path (e.g., "/users/123").
    pub path: String,
    /// Query parameters, sorted by key.
    pub query: Vec<(String, String)>,
    /// Request headers with lowercase keys.
    pub headers: Vec<(String, String)>,
    /// Request body.
    pub body: BodyData,
}

impl RequestData {
    /// Creates a new RequestData with the given method and path.
    pub fn new(method: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            method: method.into(),
            path: path.into(),
            query: Vec::new(),
            headers: Vec::new(),
            body: BodyData::Empty,
        }
    }

    /// Sets the query parameters (sorted by key).
    pub fn with_query(mut self, query: Vec<(String, String)>) -> Self {
        let mut sorted = query;
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        self.query = sorted;
        self
    }

    /// Sets the headers.
    pub fn with_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.headers = headers;
        self
    }

    /// Sets the body.
    pub fn with_body(self, body: BodyData) -> Self {
        Self { body, ..self }
    }

    /// Returns the value of the first header matching the given lowercase key.
    pub fn header(&self, key: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Returns the value of the first query param matching the given key.
    pub fn query_param(&self, key: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Context from route matching, carried through the request lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchedRequestContext {
    /// The matched route ID.
    pub route_id: String,
    /// Path parameters extracted from the route pattern.
    pub path_params: HashMap<String, String>,
    /// Descriptions of which matchers matched.
    pub matched_rules: Vec<String>,
}

impl MatchedRequestContext {
    /// Creates a new MatchedRequestContext.
    pub fn new(route_id: impl Into<String>) -> Self {
        Self {
            route_id: route_id.into(),
            path_params: HashMap::new(),
            matched_rules: Vec::new(),
        }
    }

    /// Adds a path parameter.
    pub fn with_path_param(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.path_params.insert(name.into(), value.into());
        self
    }

    /// Adds a matched rule description.
    pub fn with_matched_rule(mut self, rule: impl Into<String>) -> Self {
        self.matched_rules.push(rule.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_data_new() {
        let req = RequestData::new("GET", "/users/123");
        assert_eq!(req.method, "GET");
        assert_eq!(req.path, "/users/123");
        assert!(req.query.is_empty());
        assert!(req.headers.is_empty());
        assert!(req.body.is_empty());
    }

    #[test]
    fn test_request_data_with_query() {
        let req = RequestData::new("GET", "/search")
            .with_query(vec![
                ("b".to_string(), "2".to_string()),
                ("a".to_string(), "1".to_string()),
            ]);
        assert_eq!(req.query[0].0, "a");
        assert_eq!(req.query[1].0, "b");
    }

    #[test]
    fn test_request_data_with_headers() {
        let req = RequestData::new("POST", "/api")
            .with_headers(vec![
                ("content-type".to_string(), "application/json".to_string()),
            ]);
        assert_eq!(req.header("content-type"), Some("application/json"));
    }

    #[test]
    fn test_request_data_header_not_found() {
        let req = RequestData::new("GET", "/");
        assert!(req.header("x-custom").is_none());
    }

    #[test]
    fn test_matched_request_context_new() {
        let ctx = MatchedRequestContext::new("get-user");
        assert_eq!(ctx.route_id, "get-user");
        assert!(ctx.path_params.is_empty());
        assert!(ctx.matched_rules.is_empty());
    }

    #[test]
    fn test_matched_request_context_with_path_param() {
        let ctx = MatchedRequestContext::new("get-user")
            .with_path_param("id", "42");
        assert_eq!(ctx.path_params.get("id"), Some(&"42".to_string()));
    }

    #[test]
    fn test_matched_request_context_with_matched_rule() {
        let ctx = MatchedRequestContext::new("get-user")
            .with_matched_rule("method: GET")
            .with_matched_rule("path: /users/:id");
        assert_eq!(ctx.matched_rules.len(), 2);
    }
}
