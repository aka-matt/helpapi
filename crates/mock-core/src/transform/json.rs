//! JSON Pointer transform implementations (RFC 6901).
//!
//! These transforms operate on JSON body types. Non-JSON body + JSON transform = error.

use jsonptr::Pointer;
use serde_json::Value;
use thiserror::Error;

use crate::{BodyData, RequestData, ResponseData, Transform};

/// Error type for transform failures.
#[derive(Debug, Clone, Error, PartialEq)]
pub enum TransformError {
    #[error("invalid JSON Pointer: {0}")]
    JsonPointer(String),
    #[error("body type mismatch: JSON transform requires JSON body")]
    BodyTypeMismatch,
    #[error("header not found: {0}")]
    HeaderNotFound(String),
    #[error("query parameter not found: {0}")]
    QueryNotFound(String),
    #[error("template error: {0}")]
    TemplateError(String),
}

impl TransformError {
    /// Returns true if this error indicates the transform is not applicable
    /// (e.g., body type mismatch for a JSON transform).
    pub fn is_not_applicable(&self) -> bool {
        matches!(self, TransformError::BodyTypeMismatch)
    }
}

/// Sets a value at a JSON Pointer path (RFC 6901).
///
/// Requires body to be JSON. If body is not JSON, returns BodyTypeMismatch error.
#[derive(Debug, Clone)]
pub struct SetJsonPointer {
    path: String,
    value: Value,
}

impl SetJsonPointer {
    /// Creates a new SetJsonPointer transform.
    ///
    /// # Arguments
    /// * `path` - RFC 6901 JSON Pointer path (e.g., "/foo/bar" or "/a/0")
    /// * `value` - JSON value to set at the path
    pub fn new(path: impl Into<String>, value: Value) -> Self {
        Self {
            path: path.into(),
            value,
        }
    }
}

impl Transform for SetJsonPointer {
    fn transform_request(&self, mut request: RequestData) -> Result<RequestData, TransformError> {
        match &request.body {
            BodyData::Json(json) => {
                // Handle root "/" specially - it replaces the entire document
                if self.path == "/" {
                    request.body = BodyData::Json(self.value.clone());
                    return Ok(request);
                }
                let pointer = Pointer::parse(self.path.as_str())
                    .map_err(|e| TransformError::JsonPointer(e.to_string()))?;
                let mut new_json = json.clone();
                pointer
                    .assign(&mut new_json, self.value.clone())
                    .map_err(|e| TransformError::JsonPointer(e.to_string()))?;
                request.body = BodyData::Json(new_json);
                Ok(request)
            }
            BodyData::Empty => Err(TransformError::BodyTypeMismatch),
            BodyData::Text(_) | BodyData::Binary(_) => Err(TransformError::BodyTypeMismatch),
        }
    }

    fn transform_response(
        &self,
        mut response: ResponseData,
    ) -> Result<ResponseData, TransformError> {
        match &response.body {
            BodyData::Json(json) => {
                // Handle root "/" specially - it replaces the entire document
                if self.path == "/" {
                    response.body = BodyData::Json(self.value.clone());
                    return Ok(response);
                }
                let pointer = Pointer::parse(self.path.as_str())
                    .map_err(|e| TransformError::JsonPointer(e.to_string()))?;
                let mut new_json = json.clone();
                pointer
                    .assign(&mut new_json, self.value.clone())
                    .map_err(|e| TransformError::JsonPointer(e.to_string()))?;
                response.body = BodyData::Json(new_json);
                Ok(response)
            }
            BodyData::Empty => Err(TransformError::BodyTypeMismatch),
            BodyData::Text(_) | BodyData::Binary(_) => Err(TransformError::BodyTypeMismatch),
        }
    }

    fn name(&self) -> &str {
        "SetJsonPointer"
    }
}

/// Removes a value at a JSON Pointer path (RFC 6901).
///
/// Requires body to be JSON. If body is not JSON, returns BodyTypeMismatch error.
#[derive(Debug, Clone)]
pub struct RemoveJsonPointer {
    path: String,
}

impl RemoveJsonPointer {
    /// Creates a new RemoveJsonPointer transform.
    ///
    /// # Arguments
    /// * `path` - RFC 6901 JSON Pointer path (e.g., "/foo/bar" or "/a/0")
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

impl Transform for RemoveJsonPointer {
    fn transform_request(&self, mut request: RequestData) -> Result<RequestData, TransformError> {
        match &request.body {
            BodyData::Json(json) => {
                // Handle root "/" specially - it clears the document
                if self.path == "/" {
                    request.body = BodyData::Json(serde_json::Value::Object(Default::default()));
                    return Ok(request);
                }
                let pointer = Pointer::parse(self.path.as_str())
                    .map_err(|e| TransformError::JsonPointer(e.to_string()))?;
                let mut new_json = json.clone();
                if pointer.delete(&mut new_json).is_none() {
                    return Err(TransformError::JsonPointer(format!(
                        "path not found: {}",
                        self.path
                    )));
                }
                request.body = BodyData::Json(new_json);
                Ok(request)
            }
            BodyData::Empty => Err(TransformError::BodyTypeMismatch),
            BodyData::Text(_) | BodyData::Binary(_) => Err(TransformError::BodyTypeMismatch),
        }
    }

    fn transform_response(
        &self,
        mut response: ResponseData,
    ) -> Result<ResponseData, TransformError> {
        match &response.body {
            BodyData::Json(json) => {
                // Handle root "/" specially - it clears the document
                if self.path == "/" {
                    response.body = BodyData::Json(serde_json::Value::Object(Default::default()));
                    return Ok(response);
                }
                let pointer = Pointer::parse(self.path.as_str())
                    .map_err(|e| TransformError::JsonPointer(e.to_string()))?;
                let mut new_json = json.clone();
                if pointer.delete(&mut new_json).is_none() {
                    return Err(TransformError::JsonPointer(format!(
                        "path not found: {}",
                        self.path
                    )));
                }
                response.body = BodyData::Json(new_json);
                Ok(response)
            }
            BodyData::Empty => Err(TransformError::BodyTypeMismatch),
            BodyData::Text(_) | BodyData::Binary(_) => Err(TransformError::BodyTypeMismatch),
        }
    }

    fn name(&self) -> &str {
        "RemoveJsonPointer"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ResponseData;

    fn json_body(value: Value) -> BodyData {
        BodyData::Json(value)
    }

    fn make_request_with_body(body: BodyData) -> RequestData {
        RequestData::new("GET", "/test")
            .with_headers(vec![])
            .with_body(body)
    }

    fn make_response_with_body(body: BodyData) -> ResponseData {
        ResponseData::new(200).with_body(body)
    }

    // === SetJsonPointer tests ===

    #[test]
    fn test_set_json_pointer_nested_path() {
        let t = SetJsonPointer::new("/foo/bar", serde_json::json!("new_value"));
        let req = make_request_with_body(json_body(serde_json::json!({
            "foo": {"bar": "old"}
        })));
        let result = t.transform_request(req).unwrap();
        assert_eq!(
            result.body,
            json_body(serde_json::json!({
                "foo": {"bar": "new_value"}
            }))
        );
    }

    #[test]
    fn test_set_json_pointer_array_index() {
        let t = SetJsonPointer::new("/items/0/name", serde_json::json!("first"));
        let req = make_request_with_body(json_body(serde_json::json!({
            "items": [{"name": "old"}]
        })));
        let result = t.transform_request(req).unwrap();
        assert_eq!(
            result.body,
            json_body(serde_json::json!({
                "items": [{"name": "first"}]
            }))
        );
    }

    #[test]
    fn test_set_json_pointer_creates_intermediate() {
        // jsonptr can create intermediate objects
        let t = SetJsonPointer::new("/a/b/c", serde_json::json!(42));
        let req = make_request_with_body(json_body(serde_json::json!({})));
        let result = t.transform_request(req).unwrap();
        assert_eq!(
            result.body,
            json_body(serde_json::json!({
                "a": {"b": {"c": 42}}
            }))
        );
    }

    #[test]
    fn test_set_json_pointer_root_replace() {
        let t = SetJsonPointer::new("/", serde_json::json!("replaced"));
        let req = make_request_with_body(json_body(serde_json::json!({"key": "value"})));
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.body, json_body(serde_json::json!("replaced")));
    }

    #[test]
    fn test_set_json_pointer_non_json_body_error() {
        let t = SetJsonPointer::new("/foo", serde_json::json!("bar"));
        let req = make_request_with_body(BodyData::Text("not json".to_string()));
        let result = t.transform_request(req);
        assert!(matches!(result, Err(TransformError::BodyTypeMismatch)));
    }

    #[test]
    fn test_set_json_pointer_empty_body_error() {
        let t = SetJsonPointer::new("/foo", serde_json::json!("bar"));
        let req = make_request_with_body(BodyData::Empty);
        let result = t.transform_request(req);
        assert!(matches!(result, Err(TransformError::BodyTypeMismatch)));
    }

    #[test]
    fn test_set_json_pointer_binary_body_error() {
        let t = SetJsonPointer::new("/foo", serde_json::json!("bar"));
        let req = make_request_with_body(BodyData::Binary(vec![0x00, 0xFF]));
        let result = t.transform_request(req);
        assert!(matches!(result, Err(TransformError::BodyTypeMismatch)));
    }

    #[test]
    fn test_set_json_pointer_invalid_path() {
        let t = SetJsonPointer::new("not-a-pointer", serde_json::json!("value"));
        let req = make_request_with_body(json_body(serde_json::json!({"key": "value"})));
        let result = t.transform_request(req);
        assert!(matches!(result, Err(TransformError::JsonPointer(_))));
    }

    #[test]
    fn test_set_json_pointer_response() {
        let t = SetJsonPointer::new("/status", serde_json::json!("ok"));
        let resp = make_response_with_body(json_body(serde_json::json!({
            "status": "old"
        })));
        let result = t.transform_response(resp).unwrap();
        assert_eq!(
            result.body,
            json_body(serde_json::json!({
                "status": "ok"
            }))
        );
    }

    // === RemoveJsonPointer tests ===

    #[test]
    fn test_remove_json_pointer_nested() {
        let t = RemoveJsonPointer::new("/foo/bar");
        let req = make_request_with_body(json_body(serde_json::json!({
            "foo": {"bar": "value", "baz": "keep"}
        })));
        let result = t.transform_request(req).unwrap();
        assert_eq!(
            result.body,
            json_body(serde_json::json!({
                "foo": {"baz": "keep"}
            }))
        );
    }

    #[test]
    fn test_remove_json_pointer_array_element() {
        let t = RemoveJsonPointer::new("/items/0");
        let req = make_request_with_body(json_body(serde_json::json!({
            "items": ["a", "b", "c"]
        })));
        let result = t.transform_request(req).unwrap();
        assert_eq!(
            result.body,
            json_body(serde_json::json!({
                "items": ["b", "c"]
            }))
        );
    }

    #[test]
    fn test_remove_json_pointer_root() {
        let t = RemoveJsonPointer::new("/");
        let req = make_request_with_body(json_body(serde_json::json!({"key": "value"})));
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.body, json_body(serde_json::json!({})));
    }

    #[test]
    fn test_remove_json_pointer_non_json_body_error() {
        let t = RemoveJsonPointer::new("/foo");
        let req = make_request_with_body(BodyData::Text("not json".to_string()));
        let result = t.transform_request(req);
        assert!(matches!(result, Err(TransformError::BodyTypeMismatch)));
    }

    #[test]
    fn test_remove_json_pointer_empty_body_error() {
        let t = RemoveJsonPointer::new("/foo");
        let req = make_request_with_body(BodyData::Empty);
        let result = t.transform_request(req);
        assert!(matches!(result, Err(TransformError::BodyTypeMismatch)));
    }

    #[test]
    fn test_remove_json_pointer_invalid_path() {
        let t = RemoveJsonPointer::new("invalid");
        let req = make_request_with_body(json_body(serde_json::json!({"key": "value"})));
        let result = t.transform_request(req);
        assert!(matches!(result, Err(TransformError::JsonPointer(_))));
    }

    #[test]
    fn test_remove_json_pointer_nonexistent_path() {
        let t = RemoveJsonPointer::new("/nonexistent/path");
        let req = make_request_with_body(json_body(serde_json::json!({"key": "value"})));
        let result = t.transform_request(req);
        assert!(matches!(result, Err(TransformError::JsonPointer(_))));
    }

    #[test]
    fn test_remove_json_pointer_response() {
        let t = RemoveJsonPointer::new("/data");
        let resp = make_response_with_body(json_body(serde_json::json!({
            "data": "sensitive",
            "public": "value"
        })));
        let result = t.transform_response(resp).unwrap();
        assert_eq!(
            result.body,
            json_body(serde_json::json!({
                "public": "value"
            }))
        );
    }

    // === TransformError tests ===

    #[test]
    fn test_transform_error_is_not_applicable() {
        assert!(TransformError::BodyTypeMismatch.is_not_applicable());
        assert!(!TransformError::JsonPointer("test".to_string()).is_not_applicable());
        assert!(!TransformError::HeaderNotFound("h".to_string()).is_not_applicable());
        assert!(!TransformError::QueryNotFound("q".to_string()).is_not_applicable());
        assert!(!TransformError::TemplateError("t".to_string()).is_not_applicable());
    }

    #[test]
    fn test_transform_error_display() {
        assert_eq!(
            TransformError::BodyTypeMismatch.to_string(),
            "body type mismatch: JSON transform requires JSON body"
        );
        assert_eq!(
            TransformError::JsonPointer("bad path".to_string()).to_string(),
            "invalid JSON Pointer: bad path"
        );
    }
}
