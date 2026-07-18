//! Header transform implementations.

use crate::{RequestData, ResponseData, Transform, TransformError};

use super::template::TemplateSubst;

/// Sets or updates a header value.
///
/// If the header already exists, its value is replaced.
/// If it does not exist, the header is appended.
#[derive(Debug, Clone)]
pub struct SetHeader {
    name: String,
    #[allow(dead_code)]
    value: String,
    template: TemplateSubst,
}

impl SetHeader {
    /// Creates a new SetHeader transform.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        let value_str = value.into();
        Self {
            name: name.into(),
            value: value_str.clone(),
            template: TemplateSubst::new(&value_str),
        }
    }

    /// Apply template substitution using path parameters.
    fn apply_template(&self, path_params: &std::collections::HashMap<String, String>) -> String {
        self.template.substitute(path_params)
    }
}

impl Transform for SetHeader {
    fn transform_request(&self, mut request: RequestData) -> Result<RequestData, TransformError> {
        let value = self.apply_template(&request.path_params);
        // Replace existing header or append new one
        if let Some((_, v)) = request.headers.iter_mut().find(|(k, _)| k == &self.name) {
            *v = value;
        } else {
            request.headers.push((self.name.clone(), value));
        }
        Ok(request)
    }

    fn transform_response(
        &self,
        mut response: ResponseData,
    ) -> Result<ResponseData, TransformError> {
        let value = self.apply_template(&response.path_params);
        if let Some((_, v)) = response.headers.iter_mut().find(|(k, _)| k == &self.name) {
            *v = value;
        } else {
            response.headers.push((self.name.clone(), value));
        }
        Ok(response)
    }

    fn name(&self) -> &str {
        "SetHeader"
    }
}

/// Removes a header by name (case-insensitive).
#[derive(Debug, Clone)]
pub struct RemoveHeader {
    name: String,
}

impl RemoveHeader {
    /// Creates a new RemoveHeader transform.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into().to_lowercase(),
        }
    }
}

impl Transform for RemoveHeader {
    fn transform_request(&self, mut request: RequestData) -> Result<RequestData, TransformError> {
        let name_lower = self.name.to_lowercase();
        request
            .headers
            .retain(|(k, _)| k.to_lowercase() != name_lower);
        Ok(request)
    }

    fn transform_response(
        &self,
        mut response: ResponseData,
    ) -> Result<ResponseData, TransformError> {
        let name_lower = self.name.to_lowercase();
        response
            .headers
            .retain(|(k, _)| k.to_lowercase() != name_lower);
        Ok(response)
    }

    fn name(&self) -> &str {
        "RemoveHeader"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request() -> RequestData {
        RequestData::new("GET", "/users/123")
            .with_headers(vec![
                ("content-type".to_string(), "application/json".to_string()),
                ("x-custom".to_string(), "value".to_string()),
            ])
            .with_body(crate::BodyData::Empty)
    }

    fn make_response() -> ResponseData {
        ResponseData::new(200)
            .with_headers(vec![
                ("Content-Type".to_string(), "application/json".to_string()),
                ("X-Custom".to_string(), "value".to_string()),
            ])
            .with_body(crate::BodyData::Empty)
    }

    fn with_path_params(mut req: RequestData) -> RequestData {
        req.path_params.insert("id".to_string(), "42".to_string());
        req
    }

    #[test]
    fn test_set_header_adds_new_header() {
        let t = SetHeader::new("x-new-header", "new-value");
        let req = make_request();
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.header("x-new-header"), Some("new-value"));
        assert_eq!(result.headers.len(), 3);
    }

    #[test]
    fn test_set_header_replaces_existing_header() {
        let t = SetHeader::new("content-type", "text/xml");
        let req = make_request();
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.header("content-type"), Some("text/xml"));
        assert_eq!(result.headers.len(), 2);
    }

    #[test]
    fn test_set_header_response() {
        let t = SetHeader::new("x-request-id", "req-123");
        let resp = make_response();
        let result = t.transform_response(resp).unwrap();
        assert_eq!(result.header("x-request-id"), Some("req-123"));
    }

    #[test]
    fn test_set_header_with_template() {
        let t = SetHeader::new("x-user-id", "{{path.id}}");
        let req = with_path_params(make_request());
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.header("x-user-id"), Some("42"));
    }

    #[test]
    fn test_set_header_missing_template_var() {
        // Missing template variables are left as-is (no error per spec)
        let t = SetHeader::new("x-user-id", "{{path.nonexistent}}");
        let req = with_path_params(make_request());
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.header("x-user-id"), Some("{{path.nonexistent}}"));
    }

    #[test]
    fn test_remove_header_request() {
        let t = RemoveHeader::new("x-custom");
        let req = make_request();
        let result = t.transform_request(req).unwrap();
        assert!(result.header("x-custom").is_none());
        assert_eq!(result.headers.len(), 1);
    }

    #[test]
    fn test_remove_header_case_insensitive() {
        let t = RemoveHeader::new("X-CUSTOM");
        let req = make_request();
        let result = t.transform_request(req).unwrap();
        assert!(result.header("x-custom").is_none());
    }

    #[test]
    fn test_remove_header_not_found() {
        let t = RemoveHeader::new("x-nonexistent");
        let req = make_request();
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.headers.len(), 2);
    }

    #[test]
    fn test_remove_header_response() {
        let t = RemoveHeader::new("x-custom");
        let resp = make_response();
        let result = t.transform_response(resp).unwrap();
        assert!(result.header("x-custom").is_none());
    }
}
