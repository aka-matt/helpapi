//! Semantic validation for configuration.

use crate::{Action, Config, Transform, ValidationIssue, ValidationSeverity};
use std::collections::HashSet;

/// Validates a parsed Config for semantic correctness.
///
/// Returns a vector of validation issues. An empty vector means the config is valid.
/// Issues are categorized by severity: Error (must fix) and Warning (should fix).
///
/// # Validation Rules
///
/// - `version` must be 1
/// - Route IDs must be unique
/// - Priority values should be positive (Warning if not)
/// - Forward action `upstream` must be a valid URL
/// - Transform paths should be valid JSON Pointers (RFC 6901)
/// - MatchRule should have at least one criterion
///
pub fn validate(config: &Config) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();

    validate_version(config, &mut issues);
    validate_tls(config, &mut issues);
    validate_route_ids(config, &mut issues);
    validate_priorities(config, &mut issues);
    validate_forward_urls(config, &mut issues);
    validate_transform_paths(config, &mut issues);
    validate_match_rules(config, &mut issues);

    issues
}

fn validate_version(config: &Config, issues: &mut Vec<ValidationIssue>) {
    if config.version != 1 {
        issues.push(ValidationIssue {
            code: "INVALID_VERSION".to_string(),
            path: "/version".to_string(),
            message: format!("version must be 1, got {}", config.version),
            severity: ValidationSeverity::Error,
            suggestion: Some("set version to 1".to_string()),
        });
    }
}

fn validate_tls(config: &Config, issues: &mut Vec<ValidationIssue>) {
    let server = &config.server;
    let has_keystore = server.keystore_file.is_some();
    let has_store_password = server.keystore_password.is_some();
    let has_key_password = server.key_password.is_some();

    if !has_keystore && (has_store_password || has_key_password) {
        issues.push(ValidationIssue {
            code: "TLS_PASSWORD_WITHOUT_KEYSTORE".to_string(),
            path: "/server".to_string(),
            message: "keystore_password/key_password are set but keystore_file is missing"
                .to_string(),
            severity: ValidationSeverity::Error,
            suggestion: Some(
                "set server.keystore_file to a JKS file, or remove the TLS passwords".to_string(),
            ),
        });
        return;
    }

    if has_keystore && !has_store_password {
        issues.push(ValidationIssue {
            code: "MISSING_KEYSTORE_PASSWORD".to_string(),
            path: "/server/keystore_password".to_string(),
            message: "keystore_password is required when keystore_file is set".to_string(),
            severity: ValidationSeverity::Error,
            suggestion: Some(
                "set server.keystore_password to the JKS keystore password".to_string(),
            ),
        });
    }
}

fn validate_route_ids(config: &Config, issues: &mut Vec<ValidationIssue>) {
    let mut seen = HashSet::new();
    for (i, route) in config.routes.iter().enumerate() {
        if !seen.insert(route.id.clone()) {
            issues.push(ValidationIssue {
                code: "DUPLICATE_ROUTE_ID".to_string(),
                path: format!("/routes/{}/id", i),
                message: format!("duplicate route id: '{}'", route.id),
                severity: ValidationSeverity::Error,
                suggestion: Some("use a unique id for each route".to_string()),
            });
        }
    }
}

fn validate_priorities(config: &Config, issues: &mut Vec<ValidationIssue>) {
    for (i, route) in config.routes.iter().enumerate() {
        if route.priority < 0 {
            issues.push(ValidationIssue {
                code: "NEGATIVE_PRIORITY".to_string(),
                path: format!("/routes/{}/priority", i),
                message: format!("priority should be positive, got {}", route.priority),
                severity: ValidationSeverity::Warning,
                suggestion: Some("use a positive priority value".to_string()),
            });
        }
    }
}

fn validate_forward_urls(config: &Config, issues: &mut Vec<ValidationIssue>) {
    for (i, route) in config.routes.iter().enumerate() {
        if let Action::Forward { upstream, .. } = &route.action {
            if let Err(e) = validate_url(upstream) {
                issues.push(ValidationIssue {
                    code: "INVALID_UPSTREAM_URL".to_string(),
                    path: format!("/routes/{}/action/upstream", i),
                    message: format!("invalid upstream URL '{}': {}", upstream, e),
                    severity: ValidationSeverity::Error,
                    suggestion: Some("provide a valid HTTP/HTTPS URL".to_string()),
                });
            }
        }
    }
}

fn validate_url(url: &str) -> Result<(), String> {
    // Must start with http:// or https://
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("must start with http:// or https://".to_string());
    }

    // Parse to check structure
    let parsed = url::Url::parse(url).map_err(|e| format!("parse error: {}", e))?;

    // Must have a host
    if parsed.host().is_none() {
        return Err("must have a host".to_string());
    }

    Ok(())
}

fn validate_transform_paths(config: &Config, issues: &mut Vec<ValidationIssue>) {
    for (ri, route) in config.routes.iter().enumerate() {
        // Check each transform for valid JSON Pointer syntax
        let transforms: Vec<(&Transform, usize)> = match &route.action {
            Action::Forward {
                request_transforms,
                response_transforms,
                ..
            } => request_transforms
                .iter()
                .enumerate()
                .chain(response_transforms.iter().enumerate())
                .map(|(i, t)| (t, i))
                .collect(),
            Action::Mock { .. } => Vec::new(),
        };

        for (transform, ti) in transforms {
            if let Transform::SetJsonPointer { path, .. } | Transform::RemoveJsonPointer { path } =
                transform
            {
                if let Err(e) = validate_json_pointer(path) {
                    issues.push(ValidationIssue {
                        code: "INVALID_JSON_POINTER".to_string(),
                        path: format!("/routes/{}/action/transforms/{}", ri, ti),
                        message: format!("invalid JSON Pointer '{}': {}", path, e),
                        severity: ValidationSeverity::Error,
                        suggestion: Some(
                            "use RFC 6901 JSON Pointer format (e.g., /foo/bar or /a/b/0)"
                                .to_string(),
                        ),
                    });
                }
            }
        }
    }
}

fn validate_json_pointer(path: &str) -> Result<(), String> {
    // JSON Pointer must start with /
    if !path.starts_with('/') {
        return Err("must start with /".to_string());
    }

    // Check for valid characters
    for (i, c) in path.char_indices() {
        if i == 0 && c == '/' {
            continue;
        }
        // Per RFC 6901: ~0 for ~, ~1 for /
        // Alphanumeric, _, -, ., /, ~, and percent-encoded
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == '~' || c == '/' {
            continue;
        }
        return Err(format!("invalid character '{}' at position {}", c, i));
    }

    Ok(())
}

fn validate_match_rules(config: &Config, issues: &mut Vec<ValidationIssue>) {
    for (i, route) in config.routes.iter().enumerate() {
        let has_method = route.match_rule.method.is_some();
        let has_path = route.match_rule.path.is_some();
        let has_headers = route
            .match_rule
            .headers
            .as_ref()
            .is_some_and(|h| !h.is_empty());
        let has_query = route
            .match_rule
            .query
            .as_ref()
            .is_some_and(|q| !q.is_empty());
        let has_body = route.match_rule.body.is_some();

        if !has_method && !has_path && !has_headers && !has_query && !has_body {
            issues.push(ValidationIssue {
                code: "EMPTY_MATCH_RULE".to_string(),
                path: format!("/routes/{}/match_rule", i),
                message: "match rule has no criteria - at least one of method, path, headers, query, or body is required".to_string(),
                severity: ValidationSeverity::Warning,
                suggestion: Some("add at least one match criterion (method, path, headers, query, or body)".to_string()),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DefaultsConfig, MatchRule, Response, Route, ServerConfig};
    use std::collections::HashMap;

    fn valid_config() -> Config {
        Config {
            version: 1,
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 8080,
                ..Default::default()
            },
            defaults: DefaultsConfig {
                upstream_timeout_ms: 10_000,
                max_body_bytes: 1_048_576,
            },
            routes: vec![
                Route {
                    id: "route-1".to_string(),
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
                },
                Route {
                    id: "route-2".to_string(),
                    priority: 50,
                    match_rule: MatchRule {
                        method: Some("POST".to_string()),
                        path: Some("/api/users".to_string()),
                        headers: None,
                        query: None,
                        body: None,
                    },
                    action: Action::Forward {
                        upstream: "https://api.example.com".to_string(),
                        request_transforms: vec![],
                        response_transforms: vec![],
                    },
                },
            ],
            unmatched: None,
        }
    }

    #[test]
    fn test_validate_valid_config() {
        let config = valid_config();
        let issues = validate(&config);
        assert!(
            issues.is_empty(),
            "valid config should have no issues: {:?}",
            issues
        );
    }

    #[test]
    fn test_validate_invalid_version() {
        let mut config = valid_config();
        config.version = 999;
        let issues = validate(&config);
        assert!(!issues.is_empty());
        assert!(issues.iter().any(|i| i.code == "INVALID_VERSION"));
    }

    #[test]
    fn test_validate_duplicate_route_ids() {
        let mut config = valid_config();
        config.routes.push(Route {
            id: "route-1".to_string(), // Duplicate
            priority: 200,
            match_rule: MatchRule {
                method: Some("GET".to_string()),
                path: Some("/api/other".to_string()),
                headers: None,
                query: None,
                body: None,
            },
            action: Action::Mock {
                response: Response {
                    status: 200,
                    headers: HashMap::new(),
                    json_body: None,
                    text_body: None,
                    delay_ms: None,
                },
            },
        });
        let issues = validate(&config);
        assert!(issues.iter().any(|i| i.code == "DUPLICATE_ROUTE_ID"));
    }

    #[test]
    fn test_validate_negative_priority() {
        let mut config = valid_config();
        config.routes[0].priority = -1;
        let issues = validate(&config);
        assert!(issues.iter().any(|i| i.code == "NEGATIVE_PRIORITY"));
    }

    #[test]
    fn test_validate_invalid_upstream_url() {
        let mut config = valid_config();
        config.routes[1] = Route {
            id: "bad-forward".to_string(),
            priority: 50,
            match_rule: MatchRule {
                method: Some("POST".to_string()),
                path: Some("/api/bad".to_string()),
                headers: None,
                query: None,
                body: None,
            },
            action: Action::Forward {
                upstream: "not-a-valid-url".to_string(),
                request_transforms: vec![],
                response_transforms: vec![],
            },
        };
        let issues = validate(&config);
        assert!(issues.iter().any(|i| i.code == "INVALID_UPSTREAM_URL"));
    }

    #[test]
    fn test_validate_missing_upstream_scheme() {
        let mut config = valid_config();
        config.routes[1] = Route {
            id: "no-scheme".to_string(),
            priority: 50,
            match_rule: MatchRule {
                method: Some("POST".to_string()),
                path: Some("/api/no-scheme".to_string()),
                headers: None,
                query: None,
                body: None,
            },
            action: Action::Forward {
                upstream: "api.example.com".to_string(), // Missing http://
                request_transforms: vec![],
                response_transforms: vec![],
            },
        };
        let issues = validate(&config);
        assert!(issues.iter().any(|i| i.code == "INVALID_UPSTREAM_URL"));
    }

    #[test]
    fn test_validate_empty_match_rule() {
        let mut config = valid_config();
        config.routes[0].match_rule = MatchRule {
            method: None,
            path: None,
            headers: None,
            query: None,
            body: None,
        };
        let issues = validate(&config);
        assert!(issues.iter().any(|i| i.code == "EMPTY_MATCH_RULE"));
    }

    #[test]
    fn test_validate_invalid_json_pointer() {
        let mut config = valid_config();
        config.routes[1] = Route {
            id: "bad-pointer".to_string(),
            priority: 50,
            match_rule: MatchRule {
                method: Some("POST".to_string()),
                path: Some("/api/pointer".to_string()),
                headers: None,
                query: None,
                body: None,
            },
            action: Action::Forward {
                upstream: "https://api.example.com".to_string(),
                request_transforms: vec![Transform::SetJsonPointer {
                    path: "invalid".to_string(), // Missing leading /
                    value: serde_json::json!("test"),
                }],
                response_transforms: vec![],
            },
        };
        let issues = validate(&config);
        assert!(issues.iter().any(|i| i.code == "INVALID_JSON_POINTER"));
    }

    #[test]
    fn test_validate_valid_json_pointer() {
        let mut config = valid_config();
        config.routes[1] = Route {
            id: "good-pointer".to_string(),
            priority: 50,
            match_rule: MatchRule {
                method: Some("POST".to_string()),
                path: Some("/api/pointer".to_string()),
                headers: None,
                query: None,
                body: None,
            },
            action: Action::Forward {
                upstream: "https://api.example.com".to_string(),
                request_transforms: vec![Transform::SetJsonPointer {
                    path: "/data/attributes/name".to_string(),
                    value: serde_json::json!("test"),
                }],
                response_transforms: vec![],
            },
        };
        let issues = validate(&config);
        assert!(!issues.iter().any(|i| i.code == "INVALID_JSON_POINTER"));
    }

    #[test]
    fn test_validate_tls_valid_config() {
        let mut config = valid_config();
        config.server.keystore_file = Some("keystore.jks".to_string());
        config.server.keystore_password = Some("changeit".to_string());
        config.server.key_password = Some("keypass".to_string());
        let issues = validate(&config);
        assert!(
            issues.is_empty(),
            "valid TLS config should have no issues: {:?}",
            issues
        );
    }

    #[test]
    fn test_validate_tls_password_without_keystore() {
        let mut config = valid_config();
        config.server.keystore_password = Some("changeit".to_string());
        let issues = validate(&config);
        assert!(
            issues
                .iter()
                .any(|i| i.code == "TLS_PASSWORD_WITHOUT_KEYSTORE")
        );
    }

    #[test]
    fn test_validate_tls_missing_keystore_password() {
        let mut config = valid_config();
        config.server.keystore_file = Some("keystore.jks".to_string());
        let issues = validate(&config);
        assert!(issues.iter().any(|i| i.code == "MISSING_KEYSTORE_PASSWORD"));
    }
}
