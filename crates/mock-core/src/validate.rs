//! Abbreviated validator for `mock_core::Engine::compile`.
//!
//! Duplicates a subset of `mock_config::validate` so that `Engine::compile`
//! can reject semantically-invalid configs without taking a `mock-config`
//! dependency (which would form a Cargo cycle). Both validators are exercised
//! in practice: `Runtime::new` runs the full `mock_config::parse_and_validate`,
//! while direct `Engine::compile` callers (tests, WASM bindings) run only
//! this one. A future task should hoist the canonical validator into
//! `mock-core` and have `mock-config` re-export it, eliminating this dup.

pub fn validate_for_engine(json: &str) -> Result<(), String> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| format!("invalid JSON: {}", e))?;

    // version: must be 1 if present; absent is treated as the default (1)
    // so that existing in-tree tests and configs that omit the field keep compiling.
    if let Some(version) = v.get("version") {
        if version.as_u64() != Some(1) {
            return Err(format!(
                "INVALID_VERSION: version must be 1, got {:?}",
                version
            ));
        }
    }

    // route IDs: must be unique
    let routes = v
        .get("routes")
        .and_then(|x| x.as_array())
        .cloned()
        .unwrap_or_default();
    let mut seen_ids = std::collections::HashSet::new();
    for (i, route) in routes.iter().enumerate() {
        let id = route.get("id").and_then(|x| x.as_str()).unwrap_or("");
        if !seen_ids.insert(id.to_string()) {
            return Err(format!(
                "DUPLICATE_ROUTE_ID: route id '{}' appears more than once (at index {})",
                id, i
            ));
        }

        // forward actions: upstream URL must have a scheme
        if let Some(action) = route.get("action") {
            if action.get("type").and_then(|x| x.as_str()) == Some("forward") {
                let upstream = action
                    .get("upstream")
                    .and_then(|x| x.as_str())
                    .unwrap_or("");
                if !(upstream.starts_with("http://") || upstream.starts_with("https://")) {
                    return Err(format!(
                        "INVALID_UPSTREAM_URL: must start with http:// or https:// (got '{}')",
                        upstream
                    ));
                }
            }
        }
    }
    Ok(())
}
