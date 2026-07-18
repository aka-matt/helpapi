use serde::{Deserialize, Serialize};

use super::body::BodyData;
use super::request::MatchedRequestContext;

/// Plan for forwarding a request to an upstream service.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForwardPlan {
    /// The full URL to forward to.
    pub url: String,
    /// HTTP method to use for the upstream request.
    pub method: String,
    /// Headers to send to the upstream service.
    pub headers: Vec<(String, String)>,
    /// Body to send to the upstream service.
    pub body: BodyData,
    /// Timeout in milliseconds for the upstream request.
    pub timeout_ms: Option<u64>,
    /// Context from route matching.
    pub context: MatchedRequestContext,
}

impl ForwardPlan {
    /// Creates a new ForwardPlan.
    pub fn new(url: impl Into<String>, method: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: method.into(),
            headers: Vec::new(),
            body: BodyData::Empty,
            timeout_ms: None,
            context: MatchedRequestContext::new(""),
        }
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

    /// Sets the timeout in milliseconds.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Sets the matched request context.
    pub fn with_context(mut self, context: MatchedRequestContext) -> Self {
        self.context = context;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_plan_new() {
        let plan = ForwardPlan::new("https://api.example.com/users", "GET");
        assert_eq!(plan.url, "https://api.example.com/users");
        assert_eq!(plan.method, "GET");
        assert!(plan.headers.is_empty());
        assert!(plan.body.is_empty());
        assert!(plan.timeout_ms.is_none());
    }

    #[test]
    fn test_forward_plan_with_headers() {
        let plan = ForwardPlan::new("https://api.example.com/users", "POST")
            .with_headers(vec![("Authorization".to_string(), "Bearer token".to_string())]);
        assert_eq!(plan.headers.len(), 1);
    }

    #[test]
    fn test_forward_plan_with_body() {
        let plan = ForwardPlan::new("https://api.example.com/users", "POST")
            .with_body(BodyData::Text("request body".to_string()));
        assert!(!plan.body.is_empty());
    }

    #[test]
    fn test_forward_plan_with_timeout() {
        let plan = ForwardPlan::new("https://api.example.com/users", "GET")
            .with_timeout(5000);
        assert_eq!(plan.timeout_ms, Some(5000));
    }

    #[test]
    fn test_forward_plan_with_context() {
        let ctx = MatchedRequestContext::new("proxy-route")
            .with_path_param("id", "123");
        let plan = ForwardPlan::new("https://api.example.com/users", "GET")
            .with_context(ctx);
        assert_eq!(plan.context.route_id, "proxy-route");
        assert_eq!(plan.context.path_params.get("id"), Some(&"123".to_string()));
    }
}
