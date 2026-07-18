//! Body and status transform implementations.

use crate::{BodyData, RequestData, ResponseData, Transform, TransformError};

/// Replaces the entire request or response body.
#[derive(Debug, Clone)]
pub struct ReplaceBody {
    body: BodyData,
}

impl ReplaceBody {
    /// Creates a new ReplaceBody transform with the given body.
    pub fn new(body: BodyData) -> Self {
        Self { body }
    }

    /// Creates a ReplaceBody with a text body.
    pub fn text(content: impl Into<String>) -> Self {
        Self {
            body: BodyData::Text(content.into()),
        }
    }

    /// Creates a ReplaceBody with a JSON body.
    pub fn json(value: serde_json::Value) -> Self {
        Self {
            body: BodyData::Json(value),
        }
    }
}

impl Transform for ReplaceBody {
    fn transform_request(&self, mut request: RequestData) -> Result<RequestData, TransformError> {
        request.body = self.body.clone();
        Ok(request)
    }

    fn transform_response(
        &self,
        mut response: ResponseData,
    ) -> Result<ResponseData, TransformError> {
        response.body = self.body.clone();
        Ok(response)
    }

    fn name(&self) -> &str {
        "ReplaceBody"
    }
}

/// Sets the response status code (response-only transform).
///
/// Returns an error if applied to a request.
#[derive(Debug, Clone)]
pub struct SetStatus {
    status: u16,
}

impl SetStatus {
    /// Creates a new SetStatus transform with the given status code.
    pub fn new(status: u16) -> Self {
        Self { status }
    }
}

impl Transform for SetStatus {
    fn transform_request(&self, _request: RequestData) -> Result<RequestData, TransformError> {
        // Status codes don't make sense for requests
        Err(TransformError::TemplateError(
            "SetStatus cannot be applied to requests".to_string(),
        ))
    }

    fn transform_response(
        &self,
        mut response: ResponseData,
    ) -> Result<ResponseData, TransformError> {
        response.status = self.status;
        Ok(response)
    }

    fn name(&self) -> &str {
        "SetStatus"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request() -> RequestData {
        RequestData::new("GET", "/test")
            .with_headers(vec![])
            .with_body(BodyData::Empty)
    }

    fn make_response() -> ResponseData {
        ResponseData::new(200)
            .with_headers(vec![])
            .with_body(BodyData::Empty)
    }

    // === ReplaceBody tests ===

    #[test]
    fn test_replace_body_text() {
        let t = ReplaceBody::text("Hello, World!");
        let req = make_request();
        let result = t.transform_request(req).unwrap();
        match result.body {
            BodyData::Text(s) => assert_eq!(s, "Hello, World!"),
            _ => panic!("expected Text body"),
        }
    }

    #[test]
    fn test_replace_body_json() {
        let t = ReplaceBody::json(serde_json::json!({"message": "hello"}));
        let req = make_request();
        let result = t.transform_request(req).unwrap();
        match result.body {
            BodyData::Json(v) => assert_eq!(v, serde_json::json!({"message": "hello"})),
            _ => panic!("expected Json body"),
        }
    }

    #[test]
    fn test_replace_body_response() {
        let t = ReplaceBody::text("Custom response body");
        let resp = make_response();
        let result = t.transform_response(resp).unwrap();
        match result.body {
            BodyData::Text(s) => assert_eq!(s, "Custom response body"),
            _ => panic!("expected Text body"),
        }
    }

    #[test]
    fn test_replace_body_binary() {
        let t = ReplaceBody::new(BodyData::Binary(vec![0x00, 0x01, 0x02]));
        let req = make_request();
        let result = t.transform_request(req).unwrap();
        match result.body {
            BodyData::Binary(b) => assert_eq!(b, vec![0x00, 0x01, 0x02]),
            _ => panic!("expected Binary body"),
        }
    }

    // === SetStatus tests ===

    #[test]
    fn test_set_status_response() {
        let t = SetStatus::new(201);
        let resp = make_response();
        let result = t.transform_response(resp).unwrap();
        assert_eq!(result.status, 201);
    }

    #[test]
    fn test_set_status_request_error() {
        let t = SetStatus::new(200);
        let req = make_request();
        let result = t.transform_request(req);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_status_various_codes() {
        let resp = make_response();
        for status in [200, 201, 301, 400, 404, 500, 503] {
            let t = SetStatus::new(status);
            let result = t.transform_response(resp.clone()).unwrap();
            assert_eq!(result.status, status);
        }
    }
}
