//! JSON parsing for configuration files.

use crate::{Config, ConfigError};

/// Parses a JSON string into a Config value.
///
/// Maps serde JSON errors to ConfigError::Parse with path context.
pub fn parse_json(input: &str) -> Result<Config, ConfigError> {
    let config: Config = serde_json::from_str(input)?;
    Ok(config)
}

/// Parses a JSON string and validates the resulting Config.
///
/// This is a convenience function that combines parsing and validation.
/// Use [`validate`] separately if you need access to all validation issues
/// or to distinguish parse errors from validation errors.
pub fn parse_and_validate(input: &str) -> Result<Config, ConfigError> {
    let config = parse_json(input)?;
    let issues = crate::validate::validate(&config);
    if !issues.is_empty() {
        let messages: Vec<String> = issues
            .iter()
            .map(|issue| format!("{}: {}", issue.path, issue.message))
            .collect();
        return Err(ConfigError::Validation(messages.join("; ")));
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Action, DefaultsConfig, MatchRule, Response, Route, ServerConfig};
    use std::collections::HashMap;

    fn minimal_config() -> Config {
        Config {
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
                id: "get-user".to_string(),
                priority: 100,
                match_rule: MatchRule {
                    method: Some("GET".to_string()),
                    path: Some("/users/:id".to_string()),
                    headers: None,
                    query: None,
                    body: None,
                },
                action: Action::Mock {
                    response: Response {
                        status: 200,
                        headers: HashMap::new(),
                        json_body: Some(serde_json::json!({"id": 1})),
                        text_body: None,
                        delay_ms: None,
                    },
                },
            }],
            unmatched: None,
        }
    }

    #[test]
    fn test_parse_json_valid() {
        let config = minimal_config();
        let json = serde_json::to_string(&config).unwrap();
        let parsed = parse_json(&json).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.routes.len(), 1);
        assert_eq!(parsed.routes[0].id, "get-user");
    }

    #[test]
    fn test_parse_json_invalid() {
        let result = parse_json("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_json_missing_required_field() {
        // Missing 'server' field
        let json = r#"{"version": 1, "defaults": {}, "routes": []}"#;
        let result = parse_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_and_validate_valid() {
        let config = minimal_config();
        let json = serde_json::to_string(&config).unwrap();
        let parsed = parse_and_validate(&json).unwrap();
        assert_eq!(parsed.version, 1);
    }

    #[test]
    fn test_parse_and_validate_invalid_version() {
        let json = r#"{
            "version": 999,
            "server": {"host": "127.0.0.1", "port": 8080},
            "defaults": {"upstream_timeout_ms": 10000, "max_body_bytes": 1048576},
            "routes": []
        }"#;
        let result = parse_and_validate(json);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, crate::ConfigError::Validation(_)));
    }

    #[test]
    fn test_parse_and_validate_empty_routes() {
        let json = r#"{
            "version": 1,
            "server": {"host": "127.0.0.1", "port": 8080},
            "defaults": {"upstream_timeout_ms": 10000, "max_body_bytes": 1048576},
            "routes": []
        }"#;
        // Empty routes is valid
        let result = parse_and_validate(json);
        assert!(result.is_ok());
    }
}
