//! Build the runtime `Box<dyn Transform>` instances from typed `Transform` specs.

use crate::Transform;
use crate::transform::body::{ReplaceBody, SetStatus};
use crate::transform::headers::{RemoveHeader, SetHeader};
use crate::transform::json::{RemoveJsonPointer, SetJsonPointer};
use crate::transform::query::{RemoveQuery, SetQuery};
use crate::transform::spec::Transform as SpecTransform;

/// Builds a `Box<dyn Transform>` from a typed `Transform` spec.
pub fn transform_from_spec(spec: &SpecTransform) -> Box<dyn Transform> {
    match spec {
        SpecTransform::SetHeader { name, value } => {
            Box::new(SetHeader::new(name.clone(), value.clone()))
        }
        SpecTransform::RemoveHeader { name } => Box::new(RemoveHeader::new(name.clone())),
        SpecTransform::SetQuery { name, value } => {
            Box::new(SetQuery::new(name.clone(), value.clone()))
        }
        SpecTransform::RemoveQuery { name } => Box::new(RemoveQuery::new(name.clone())),
        SpecTransform::SetJsonPointer { path, value } => {
            Box::new(SetJsonPointer::new(path.clone(), value.clone()))
        }
        SpecTransform::RemoveJsonPointer { path } => Box::new(RemoveJsonPointer::new(path.clone())),
        SpecTransform::ReplaceBody { body } => Box::new(ReplaceBody::new(body.clone())),
        SpecTransform::SetStatus { status } => Box::new(SetStatus::new(*status)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BodyData, RequestData, ResponseData};

    // === SetHeader ===

    #[test]
    fn test_set_header_from_spec() {
        let spec = SpecTransform::SetHeader {
            name: "X-Foo".to_string(),
            value: "bar".to_string(),
        };
        let t = transform_from_spec(&spec);
        assert_eq!(t.name(), "SetHeader");
    }

    #[test]
    fn test_set_header_from_spec_applies_to_request() {
        let spec = SpecTransform::SetHeader {
            name: "X-Foo".to_string(),
            value: "bar".to_string(),
        };
        let t = transform_from_spec(&spec);
        let req = RequestData::new("GET", "/users/123").with_body(BodyData::Empty);
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.header("X-Foo"), Some("bar"));
    }

    // === RemoveHeader ===

    #[test]
    fn test_remove_header_from_spec() {
        let spec = SpecTransform::RemoveHeader {
            name: "x-custom".to_string(),
        };
        let t = transform_from_spec(&spec);
        assert_eq!(t.name(), "RemoveHeader");
    }

    #[test]
    fn test_remove_header_from_spec_applies_to_request() {
        let spec = SpecTransform::RemoveHeader {
            name: "x-custom".to_string(),
        };
        let t = transform_from_spec(&spec);
        let req = RequestData::new("GET", "/users/123")
            .with_headers(vec![
                ("content-type".to_string(), "application/json".to_string()),
                ("x-custom".to_string(), "value".to_string()),
            ])
            .with_body(BodyData::Empty);
        let result = t.transform_request(req).unwrap();
        assert!(result.header("x-custom").is_none());
        assert_eq!(result.headers.len(), 1);
    }

    // === SetQuery ===

    #[test]
    fn test_set_query_from_spec() {
        let spec = SpecTransform::SetQuery {
            name: "limit".to_string(),
            value: "10".to_string(),
        };
        let t = transform_from_spec(&spec);
        assert_eq!(t.name(), "SetQuery");
    }

    #[test]
    fn test_set_query_from_spec_applies_to_request() {
        let spec = SpecTransform::SetQuery {
            name: "limit".to_string(),
            value: "10".to_string(),
        };
        let t = transform_from_spec(&spec);
        let req = RequestData::new("GET", "/search").with_body(BodyData::Empty);
        let result = t.transform_request(req).unwrap();
        assert_eq!(result.query_param("limit"), Some("10"));
        assert_eq!(result.query.len(), 1);
    }

    // === RemoveQuery ===

    #[test]
    fn test_remove_query_from_spec() {
        let spec = SpecTransform::RemoveQuery {
            name: "limit".to_string(),
        };
        let t = transform_from_spec(&spec);
        assert_eq!(t.name(), "RemoveQuery");
    }

    #[test]
    fn test_remove_query_from_spec_applies_to_request() {
        let spec = SpecTransform::RemoveQuery {
            name: "limit".to_string(),
        };
        let t = transform_from_spec(&spec);
        let req = RequestData::new("GET", "/search")
            .with_query(vec![
                ("limit".to_string(), "10".to_string()),
                ("offset".to_string(), "0".to_string()),
            ])
            .with_body(BodyData::Empty);
        let result = t.transform_request(req).unwrap();
        assert!(result.query_param("limit").is_none());
        assert_eq!(result.query.len(), 1);
    }

    // === SetJsonPointer ===

    #[test]
    fn test_set_json_pointer_from_spec() {
        let spec = SpecTransform::SetJsonPointer {
            path: "/name".to_string(),
            value: serde_json::json!("alice"),
        };
        let t = transform_from_spec(&spec);
        assert_eq!(t.name(), "SetJsonPointer");
    }

    #[test]
    fn test_set_json_pointer_from_spec_applies_to_request() {
        let spec = SpecTransform::SetJsonPointer {
            path: "/name".to_string(),
            value: serde_json::json!("alice"),
        };
        let t = transform_from_spec(&spec);
        let req = RequestData::new("POST", "/users").with_body(BodyData::Json(serde_json::json!({
            "name": "bob",
            "age": 30
        })));
        let result = t.transform_request(req).unwrap();
        match result.body {
            BodyData::Json(v) => {
                assert_eq!(v["name"], serde_json::json!("alice"));
                assert_eq!(v["age"], serde_json::json!(30));
            }
            _ => panic!("expected Json body"),
        }
    }

    // === RemoveJsonPointer ===

    #[test]
    fn test_remove_json_pointer_from_spec() {
        let spec = SpecTransform::RemoveJsonPointer {
            path: "/secret".to_string(),
        };
        let t = transform_from_spec(&spec);
        assert_eq!(t.name(), "RemoveJsonPointer");
    }

    #[test]
    fn test_remove_json_pointer_from_spec_applies_to_request() {
        let spec = SpecTransform::RemoveJsonPointer {
            path: "/secret".to_string(),
        };
        let t = transform_from_spec(&spec);
        let req = RequestData::new("POST", "/users").with_body(BodyData::Json(serde_json::json!({
            "name": "bob",
            "secret": "hidden"
        })));
        let result = t.transform_request(req).unwrap();
        match result.body {
            BodyData::Json(v) => {
                assert_eq!(v["name"], serde_json::json!("bob"));
                assert!(v.get("secret").is_none());
            }
            _ => panic!("expected Json body"),
        }
    }

    // === ReplaceBody ===

    #[test]
    fn test_replace_body_from_spec() {
        // `ReplaceBody` accepts an externally-tagged `BodyData` (e.g.
        // `{"json": {...}}`, `{"text": "..."}`, `{"empty": null}`,
        // `{"binary": "..."}`).
        let spec = SpecTransform::ReplaceBody {
            body: BodyData::Json(serde_json::json!({"replaced": true})),
        };
        let t = transform_from_spec(&spec);
        assert_eq!(t.name(), "ReplaceBody");
    }

    #[test]
    fn test_replace_body_from_spec_applies_to_request() {
        let spec = SpecTransform::ReplaceBody {
            body: BodyData::Text("replacement body".to_string()),
        };
        let t = transform_from_spec(&spec);
        let req = RequestData::new("POST", "/users")
            .with_body(BodyData::Json(serde_json::json!({"original": true})));
        let result = t.transform_request(req).unwrap();
        match result.body {
            BodyData::Text(s) => assert_eq!(s, "replacement body"),
            _ => panic!("expected Text body"),
        }
    }

    #[test]
    fn test_replace_body_from_spec_applies_to_response() {
        let spec = SpecTransform::ReplaceBody {
            body: BodyData::Json(serde_json::json!({"status": "ok"})),
        };
        let t = transform_from_spec(&spec);
        let resp = ResponseData::new(200).with_body(BodyData::Empty);
        let result = t.transform_response(resp).unwrap();
        match result.body {
            BodyData::Json(v) => assert_eq!(v, serde_json::json!({"status": "ok"})),
            _ => panic!("expected Json body"),
        }
    }

    // === SetStatus ===

    #[test]
    fn test_set_status_from_spec() {
        let spec = SpecTransform::SetStatus { status: 201 };
        let t = transform_from_spec(&spec);
        assert_eq!(t.name(), "SetStatus");
    }

    #[test]
    fn test_set_status_from_spec_applies_to_response() {
        let spec = SpecTransform::SetStatus { status: 201 };
        let t = transform_from_spec(&spec);
        let resp = ResponseData::new(200);
        let result = t.transform_response(resp).unwrap();
        assert_eq!(result.status, 201);
    }
}
