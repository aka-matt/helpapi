//! Configuration data models for the mock API.
//!
//! These types are serializable with serde and are consumed by mock-core and mock-cli.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Body data variants for request/response bodies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum BodyData {
    #[default]
    Empty,
    Text(String),
    Json(serde_json::Value),
}

/// Top-level configuration structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Config {
    pub version: u32,
    pub server: ServerConfig,
    pub defaults: DefaultsConfig,
    pub routes: Vec<Route>,
    #[serde(default)]
    pub unmatched: Option<UnmatchedConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: 1,
            server: ServerConfig::default(),
            defaults: DefaultsConfig::default(),
            routes: Vec::new(),
            unmatched: None,
        }
    }
}

/// Server binding configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            host: "127.0.0.1".to_string(),
            port: 8080,
        }
    }
}

/// Default settings applied to all routes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DefaultsConfig {
    pub upstream_timeout_ms: u64,
    pub max_body_bytes: usize,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        DefaultsConfig {
            upstream_timeout_ms: 10_000,
            max_body_bytes: 1_048_576, // 1 MiB
        }
    }
}

/// Route definition linking a match rule to an action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Route {
    pub id: String,
    pub priority: i32,
    pub match_rule: MatchRule,
    pub action: Action,
}

/// Rule for matching incoming requests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MatchRule {
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub query: Option<HashMap<String, String>>,
    #[serde(default)]
    pub body: Option<BodyData>,
}

/// Action to take when a route matches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Action {
    Mock {
        response: Response,
    },
    Forward {
        upstream: String,
        #[serde(default)]
        request_transforms: Vec<Transform>,
        #[serde(default)]
        response_transforms: Vec<Transform>,
    },
}

/// Response returned for a mock action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Response {
    pub status: u16,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub json_body: Option<serde_json::Value>,
    #[serde(default)]
    pub text_body: Option<String>,
    #[serde(default)]
    pub delay_ms: Option<u64>,
}

/// Transform operations for request/response modification.
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

/// Configuration for requests that match no route.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UnmatchedConfig {
    pub action: Action,
}

/// Validation issue found during config validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ValidationIssue {
    pub code: String,
    pub path: String,
    pub message: String,
    pub severity: ValidationSeverity,
    #[serde(default)]
    pub suggestion: Option<String>,
}

/// Severity level of a validation issue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Error,
    Warning,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_json_roundtrip() {
        let config = Config {
            version: 1,
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 3000,
            },
            defaults: DefaultsConfig {
                upstream_timeout_ms: 5000,
                max_body_bytes: 512_000,
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

        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(config, parsed);
    }

    #[test]
    fn test_forward_action_roundtrip() {
        let action = Action::Forward {
            upstream: "https://api.example.com".to_string(),
            request_transforms: vec![
                Transform::SetHeader {
                    name: "X-Custom".to_string(),
                    value: "value".to_string(),
                },
                Transform::SetJsonPointer {
                    path: "/foo".to_string(),
                    value: serde_json::json!("bar"),
                },
            ],
            response_transforms: vec![Transform::RemoveHeader {
                name: "X-Internal".to_string(),
            }],
        };

        let json = serde_json::to_string_pretty(&action).unwrap();
        let parsed: Action = serde_json::from_str(&json).unwrap();
        assert_eq!(action, parsed);
    }

    #[test]
    fn test_body_data_variants() {
        let cases = vec![
            (BodyData::Empty, r#""empty""#),
            (BodyData::Text("hello".to_string()), r#""hello""#),
            (
                BodyData::Json(serde_json::json!({"a": 1})),
                r#"{"json":{"a":1}}"#,
            ),
        ];

        for (variant, _expected_pattern) in cases {
            let json = serde_json::to_string(&variant).unwrap();
            let parsed: BodyData = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn test_validation_issue_roundtrip() {
        let issue = ValidationIssue {
            code: "INVALID_METHOD".to_string(),
            path: "/routes/0/match_rule/method".to_string(),
            message: "HTTP method must be uppercase".to_string(),
            severity: ValidationSeverity::Warning,
            suggestion: Some("Use GET instead of get".to_string()),
        };

        let json = serde_json::to_string_pretty(&issue).unwrap();
        let parsed: ValidationIssue = serde_json::from_str(&json).unwrap();
        assert_eq!(issue, parsed);
    }
}
