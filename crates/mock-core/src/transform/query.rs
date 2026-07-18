//! Query parameter transform implementations.

use crate::{RequestData, ResponseData, Transform, TransformError};

use super::template::TemplateSubst;

/// Sets or updates a query parameter value.
#[derive(Debug, Clone)]
pub struct SetQuery {
    name: String,
    #[allow(dead_code)]
    value: String,
    template: TemplateSubst,
}

impl SetQuery {
    /// Creates a new SetQuery transform.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        let value_str = value.into();
        Self {
            name: name.into(),
            value: value_str.clone(),
            template: TemplateSubst::new(&value_str),
        }
    }

    fn apply_template(&self, path_params: &std::collections::HashMap<String, String>) -> String {
        self.template.substitute(path_params)
    }
}

impl Transform for SetQuery {
    fn transform_request(&self, mut request: RequestData) -> Result<RequestData, TransformError> {
        let value = self.apply_template(&request.path_params);
        if let Some((_, v)) = request.query.iter_mut().find(|(k, _)| k == &self.name) {
            *v = value;
        } else {
            request.query.push((self.name.clone(), value));
            request.query.sort_by(|a, b| a.0.cmp(&b.0));
        }
        Ok(request)
    }

    fn transform_response(&self, response: ResponseData) -> Result<ResponseData, TransformError> {
        // Query params don't make sense for responses, but we include it for interface completeness
        // In practice, response transforms might not use query manipulation
        let _ = response;
        Err(TransformError::QueryNotFound(self.name.clone()))
    }

    fn name(&self) -> &str {
        "SetQuery"
    }
}

/// Removes a query parameter by name.
#[derive(Debug, Clone)]
pub struct RemoveQuery {
    name: String,
}

impl RemoveQuery {
    /// Creates a new RemoveQuery transform.
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

impl Transform for RemoveQuery {
    fn transform_request(&self, mut request: RequestData) -> Result<RequestData, TransformError> {
        let original_len = request.query.len();
        request.query.retain(|(k, _)| k != &self.name);
        if request.query.len() == original_len {
            return Err(TransformError::QueryNotFound(self.name.clone()));
        }
        Ok(request)
    }

    fn transform_response(&self, response: ResponseData) -> Result<ResponseData, TransformError> {
        // Query params don't make sense for responses
        let _ = response;
        Err(TransformError::QueryNotFound(self.name.clone()))
    }

    fn name(&self) -> &str {
        "RemoveQuery"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request() -> RequestData {
        RequestData::new("GET", "/search")
            .with_query(vec![
                ("a".to_string(), "1".to_string()),
                ("b".to_string(), "2".to_string()),
            ])
            .with_headers(vec![("host".to_string(), "localhost".to_string())])
            .with_body(crate::BodyData::Empty)
    }

    fn with_path_params(mut req: RequestData) -> RequestData {
        req.path_params.insert("id".to_string(), "42".to_string());
        req
    }

    #[test]
    fn test_set_query_adds_new_param() {
        let t = SetQuery::new("c", "3");
        let req = make_request();
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.query_param("c"), Some("3"));
        assert_eq!(result.query.len(), 3);
    }

    #[test]
    fn test_set_query_updates_existing_param() {
        let t = SetQuery::new("a", "updated");
        let req = make_request();
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.query_param("a"), Some("updated"));
        assert_eq!(result.query.len(), 2);
    }

    #[test]
    fn test_set_query_sorted_after_add() {
        let t = SetQuery::new("z", "new");
        let req = make_request();
        let result = t.transform_request(req).unwrap();
        // Should still be sorted after adding new param
        assert_eq!(result.query.last().unwrap().0, "z");
    }

    #[test]
    fn test_set_query_with_template() {
        let t = SetQuery::new("user_id", "{{path.id}}");
        let req = with_path_params(make_request());
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.query_param("user_id"), Some("42"));
    }

    #[test]
    fn test_set_query_missing_template_var() {
        let t = SetQuery::new("user_id", "{{path.nonexistent}}");
        let req = with_path_params(make_request());
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.query_param("user_id"), Some("{{path.nonexistent}}"));
    }

    #[test]
    fn test_set_query_response_error() {
        let t = SetQuery::new("key", "value");
        let resp = ResponseData::new(200);
        let result = t.transform_response(resp);
        assert!(matches!(result, Err(TransformError::QueryNotFound(_))));
    }

    #[test]
    fn test_remove_query_found() {
        let t = RemoveQuery::new("a");
        let req = make_request();
        let result = t.transform_request(req).unwrap();
        assert!(result.query_param("a").is_none());
        assert_eq!(result.query.len(), 1);
    }

    #[test]
    fn test_remove_query_not_found() {
        let t = RemoveQuery::new("nonexistent");
        let req = make_request();
        let result = t.transform_request(req);
        assert!(matches!(result, Err(TransformError::QueryNotFound(_))));
    }

    #[test]
    fn test_remove_query_response_error() {
        let t = RemoveQuery::new("key");
        let resp = ResponseData::new(200);
        let result = t.transform_response(resp);
        assert!(matches!(result, Err(TransformError::QueryNotFound(_))));
    }
}
