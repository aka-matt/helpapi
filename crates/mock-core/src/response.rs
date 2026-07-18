use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::body::BodyData;

/// Represents an HTTP response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseData {
    /// HTTP status code.
    pub status: u16,
    /// Response headers.
    pub headers: Vec<(String, String)>,
    /// Response body.
    pub body: BodyData,
    /// Optional delay in milliseconds before sending the response.
    pub delay_ms: Option<u64>,
    /// Path parameters extracted from route matching (for template substitution).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub path_params: HashMap<String, String>,
}

impl ResponseData {
    /// Creates a new ResponseData with the given status and empty body.
    pub fn new(status: u16) -> Self {
        Self {
            status,
            headers: Vec::new(),
            body: BodyData::Empty,
            delay_ms: None,
            path_params: HashMap::new(),
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

    /// Sets the delay in milliseconds.
    pub fn with_delay(mut self, delay_ms: u64) -> Self {
        self.delay_ms = Some(delay_ms);
        self
    }

    /// Sets the path parameters (for template substitution).
    pub fn with_path_params(mut self, path_params: HashMap<String, String>) -> Self {
        self.path_params = path_params;
        self
    }

    /// Returns the value of the first header matching the given case-insensitive key.
    pub fn header(&self, key: &str) -> Option<&str> {
        let key_lower = key.to_lowercase();
        self.headers
            .iter()
            .find(|(k, _)| k.to_lowercase() == key_lower)
            .map(|(_, v)| v.as_str())
    }
}

impl Default for ResponseData {
    fn default() -> Self {
        Self::new(200)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_response_data_new() {
        let resp = ResponseData::new(200);
        assert_eq!(resp.status, 200);
        assert!(resp.headers.is_empty());
        assert!(resp.body.is_empty());
        assert!(resp.delay_ms.is_none());
    }

    #[test]
    fn test_response_data_with_headers() {
        let resp = ResponseData::new(200).with_headers(vec![(
            "Content-Type".to_string(),
            "application/json".to_string(),
        )]);
        assert_eq!(resp.header("content-type"), Some("application/json"));
    }

    #[test]
    fn test_response_data_with_body() {
        let resp = ResponseData::new(201).with_body(BodyData::Json(serde_json::json!({"id": 1})));
        assert!(!resp.body.is_empty());
    }

    #[test]
    fn test_response_data_with_delay() {
        let resp = ResponseData::new(200).with_delay(500);
        assert_eq!(resp.delay_ms, Some(500));
    }

    #[test]
    fn test_response_data_default() {
        let resp = ResponseData::default();
        assert_eq!(resp.status, 200);
    }
}
