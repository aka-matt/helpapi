//! Canonical typed representation for transforms. The actual transformation
//! logic lives in `crate::transform::headers`, `crate::transform::query`, etc.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The body data attached to a request or response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum BodyData {
    #[default]
    Empty,
    Text(String),
    Json(serde_json::Value),
    Binary(Vec<u8>),
}

impl BodyData {
    /// Returns true if the body is empty.
    pub fn is_empty(&self) -> bool {
        matches!(self, BodyData::Empty)
    }

    /// Returns the content type hint for this body type.
    pub fn content_type(&self) -> Option<&'static str> {
        match self {
            BodyData::Empty => None,
            BodyData::Text(_) => Some("text/plain"),
            BodyData::Json(_) => Some("application/json"),
            BodyData::Binary(_) => Some("application/octet-stream"),
        }
    }

    /// Returns the size in bytes, if known.
    pub fn size_bytes(&self) -> usize {
        match self {
            BodyData::Empty => 0,
            BodyData::Text(s) => s.len(),
            BodyData::Json(v) => v.to_string().len(),
            BodyData::Binary(b) => b.len(),
        }
    }
}

/// Transform kinds expressed as JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Transform {
    SetHeader {
        name: String,
        value: String,
    },
    RemoveHeader {
        name: String,
    },
    SetQuery {
        name: String,
        value: String,
    },
    RemoveQuery {
        name: String,
    },
    SetJsonPointer {
        path: String,
        value: serde_json::Value,
    },
    RemoveJsonPointer {
        path: String,
    },
    ReplaceBody {
        body: BodyData,
    },
    SetStatus {
        status: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_body_data_empty() {
        let body = BodyData::Empty;
        assert!(body.is_empty());
        assert_eq!(body.size_bytes(), 0);
        assert!(body.content_type().is_none());
    }

    #[test]
    fn test_body_data_text() {
        let body = BodyData::Text("hello".to_string());
        assert!(!body.is_empty());
        assert_eq!(body.size_bytes(), 5);
        assert_eq!(body.content_type(), Some("text/plain"));
    }

    #[test]
    fn test_body_data_json() {
        let body = BodyData::Json(serde_json::json!({"key": "value"}));
        assert!(!body.is_empty());
        assert_eq!(body.content_type(), Some("application/json"));
    }

    #[test]
    fn test_body_data_binary() {
        let body = BodyData::Binary(vec![0x00, 0xFF, 0x42]);
        assert!(!body.is_empty());
        assert_eq!(body.size_bytes(), 3);
        assert_eq!(body.content_type(), Some("application/octet-stream"));
    }

    #[test]
    fn test_body_data_default() {
        let body = BodyData::default();
        assert!(body.is_empty());
    }

    /// Verifies that the canonical `BodyData` uses snake_case serialization,
    /// matching the typed-deserialize path used by `compile_transforms`.
    #[test]
    fn test_body_data_serialization_snake_case() {
        // Test JSON serialization — snake_case variants
        let body = BodyData::Text("test".to_string());
        let json = serde_json::to_string(&body).unwrap();
        assert!(json.contains("text"));

        let body2 = BodyData::Json(serde_json::json!({"a": 1}));
        let json2 = serde_json::to_string(&body2).unwrap();
        assert!(json2.contains("json"));

        let body3 = BodyData::Binary(vec![1, 2, 3]);
        let json3 = serde_json::to_string(&body3).unwrap();
        assert!(json3.contains("binary"));

        // Test deserialization of snake_case variant
        let empty: BodyData = serde_json::from_str(r#""empty""#).unwrap();
        assert_eq!(empty, BodyData::Empty);
    }

    #[test]
    fn test_transform_set_header_deserialize() {
        let v: Transform = serde_json::from_value(serde_json::json!({
            "type": "set_header",
            "name": "X-Foo",
            "value": "bar"
        }))
        .unwrap();
        match v {
            Transform::SetHeader { name, value } => {
                assert_eq!(name, "X-Foo");
                assert_eq!(value, "bar");
            }
            _ => panic!("expected SetHeader"),
        }
    }
}
