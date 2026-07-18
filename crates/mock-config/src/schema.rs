//! JSON Schema generation for configuration validation.

use crate::Config;
use schemars::schema_for;

/// Generates a JSON Schema for the Config type.
///
/// The generated schema can be used by editors and tools for
/// configuration validation and auto-completion support.
pub fn generate_schema() -> String {
    let schema = schema_for!(Config);
    serde_json::to_string_pretty(&schema).expect("schema should be valid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, DefaultsConfig, MatchRule, Response, Route};
    use crate::{Config, ServerConfig};
    use std::collections::HashMap;

    #[test]
    fn test_generate_schema_returns_valid_json() {
        let schema = generate_schema();
        // Should be valid JSON
        let parsed: serde_json::Value =
            serde_json::from_str(&schema).expect("schema should be valid JSON");
        assert!(parsed.is_object());
    }

    #[test]
    fn test_generate_schema_contains_config_properties() {
        let schema = generate_schema();
        let parsed: serde_json::Value = serde_json::from_str(&schema).unwrap();
        // Root should be an object schema
        let schema_obj = parsed.as_object().unwrap();
        assert!(schema_obj.contains_key("$schema"));
        assert!(schema_obj.contains_key("properties"));
    }

    #[test]
    fn test_generate_schema_includes_version_field() {
        let schema = generate_schema();
        let parsed: serde_json::Value = serde_json::from_str(&schema).unwrap();
        let props = parsed.pointer("/properties").unwrap();
        assert!(props.as_object().unwrap().contains_key("version"));
    }

    #[test]
    fn test_generate_schema_includes_routes_array() {
        let schema = generate_schema();
        let parsed: serde_json::Value = serde_json::from_str(&schema).unwrap();
        let props = parsed.pointer("/properties").unwrap();
        assert!(props.as_object().unwrap().contains_key("routes"));
    }

    #[test]
    fn test_schema_can_roundtrip_config() {
        // Verify that a valid config serializes to JSON that can be parsed
        let config = Config {
            version: 1,
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
            },
            defaults: DefaultsConfig {
                upstream_timeout_ms: 10_000,
                max_body_bytes: 1_048_576,
            },
            routes: vec![Route {
                id: "test-route".to_string(),
                priority: 100,
                match_rule: MatchRule {
                    method: Some("GET".to_string()),
                    path: Some("/api/test".to_string()),
                    headers: None,
                    query: None,
                    body: None,
                },
                action: Action::Mock {
                    response: Response {
                        status: 200,
                        headers: HashMap::new(),
                        json_body: Some(serde_json::json!({"ok": true})),
                        text_body: None,
                        delay_ms: None,
                    },
                },
            }],
            unmatched: None,
        };

        // Config should serialize and deserialize
        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }
}
