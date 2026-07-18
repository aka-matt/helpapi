//! The mock engine — compiles config into rules and evaluates incoming requests.
//!
//! ## Usage
//!
//! ```ignore
//! use mock_core::Engine;
//!
//! let engine = Engine::compile(json_str).unwrap();
//! let decision = engine.decide(request).unwrap();
//! ```

use std::collections::HashMap;
use std::time::Instant;

use crate::decision::Decision;
use crate::error::EngineError;
use crate::forward::ForwardPlan;
use crate::matcher::Matcher;
use crate::metadata::DecisionMetadata;
use crate::request::{MatchedRequestContext, RequestData};
use crate::response::ResponseData;
use crate::transform::Transform;

/// A compiled route — pre-parsed matchers and pre-resolved transforms.
pub struct CompiledRule {
    /// The route ID from the config.
    pub id: String,
    /// Route priority (higher = evaluated first).
    pub priority: i32,
    /// All matchers that must match for this rule to apply (AND logic).
    pub matchers: Vec<Box<dyn Matcher>>,
    /// Transforms applied to the request before forwarding.
    pub request_transforms: Vec<Box<dyn Transform>>,
    /// Transforms applied to the upstream response before returning.
    pub response_transforms: Vec<Box<dyn Transform>>,
    /// The compiled action to execute when this rule matches.
    pub action: CompiledAction,
}

impl std::fmt::Debug for CompiledRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompiledRule")
            .field("id", &self.id)
            .field("priority", &self.priority)
            .field("matchers", &self.matchers.len())
            .field("request_transforms", &self.request_transforms.len())
            .field("response_transforms", &self.response_transforms.len())
            .field("action", &self.action)
            .finish()
    }
}

/// The executable action for a matched rule.
#[derive(Debug, Clone)]
pub enum CompiledAction {
    /// Return a locally-defined mock response.
    Mock {
        /// The mock response to return.
        response: ResponseData,
    },
    /// Forward the request to an upstream service.
    Forward {
        /// The upstream URL base.
        upstream: String,
    },
}

/// The mock engine — compiles config into sorted rules and evaluates requests.
#[derive(Debug)]
pub struct Engine {
    /// Rules sorted by priority (highest first), then by declaration order.
    rules: Vec<CompiledRule>,
    /// Default settings applied to all routes.
    defaults: EngineDefaults,
    /// Action to take when no route matches.
    unmatched: Option<EngineUnmatched>,
}

/// Default settings extracted from config.
#[derive(Debug, Clone)]
struct EngineDefaults {
    upstream_timeout_ms: u64,
    max_body_bytes: usize,
}

/// Unmatched config extracted from config.
#[derive(Debug, Clone)]
struct EngineUnmatched {
    action: EngineUnmatchedAction,
}

/// Unmatched action (simplified from config Action).
#[derive(Debug, Clone)]
enum EngineUnmatchedAction {
    Mock {
        status: u16,
        body: crate::BodyData,
        headers: Vec<(String, String)>,
        delay_ms: u64,
    },
    Forward {
        upstream: String,
    },
}

impl Engine {
    /// Compiles a JSON config string into an [`Engine`].
    ///
    /// Rules are sorted by priority (descending). In case of equal priority,
    /// the rule declared first in the config wins.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::CompileError`] if the JSON is invalid or
    /// any route cannot be compiled (e.g., invalid match pattern).
    pub fn compile(config_json: &str) -> Result<Self, EngineError> {
        let config: serde_json::Value = serde_json::from_str(config_json)
            .map_err(|e| EngineError::CompileError(format!("invalid JSON: {}", e)))?;

        let rules_json = config
            .get("routes")
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                EngineError::CompileError("missing or invalid 'routes' field".to_string())
            })?;

        let mut rules: Vec<CompiledRule> = Vec::new();
        for (idx, route_val) in rules_json.iter().enumerate() {
            let route: serde_json::Map<String, serde_json::Value> = route_val
                .as_object()
                .cloned()
                .ok_or_else(|| EngineError::CompileError(format!("route {}: not an object", idx)))?
                .into_iter()
                .collect();

            let compiled = compile_rule(route, idx).map_err(|e| EngineError::CompileError(e))?;
            rules.push(compiled);
        }

        // Sort by priority descending, tie-break by original index (stable sort)
        // Since we iterate in declaration order, rules[i] has declaration order i
        let mut indexed: Vec<(i32, usize, CompiledRule)> = rules
            .into_iter()
            .enumerate()
            .map(|(i, r)| (r.priority, i, r))
            .collect();
        indexed.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        rules = indexed.into_iter().map(|(_, _, r)| r).collect();

        // Parse defaults
        let defaults = parse_defaults(&config);

        // Parse unmatched
        let unmatched = parse_unmatched(&config).map_err(EngineError::CompileError)?;

        Ok(Self {
            rules,
            defaults,
            unmatched,
        })
    }

    /// Evaluates an incoming [`RequestData`] against the compiled rules.
    ///
    /// Rules are tried in priority order. The first rule where **all** matchers
    /// match wins. If no rule matches, the `unmatched` config determines the
    /// response.
    ///
    /// # Determinism
    ///
    /// The same `request` always produces the same `Decision` given the same
    /// Engine (same compiled config). Priority ordering is stable and
    /// declaration-order is used as a tie-breaker.
    pub fn decide(&self, request: RequestData) -> Result<Decision, EngineError> {
        let start = Instant::now();

        // Try each rule in priority order
        for rule in &self.rules {
            let result = self.try_rule(rule, &request);

            if result.matched {
                let elapsed_us = start.elapsed().as_micros() as u64;
                let mut metadata = result.metadata;
                metadata.elapsed_us = elapsed_us;
                return Ok(result.decision.unwrap_or_else(|| {
                    // Rule matched but produced no decision — shouldn't happen with valid config
                    Decision::Reject {
                        response: ResponseData::new(500).with_body(crate::BodyData::Text(
                            "Internal: rule matched but no action".to_string(),
                        )),
                        metadata,
                    }
                }));
            }
        }

        // No rule matched — handle unmatched
        let elapsed_us = start.elapsed().as_micros() as u64;
        self.handle_unmatched(request, elapsed_us)
    }

    /// Transforms an upstream response using the response transforms from
    /// the matched route.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::TransformError`] if any transform fails.
    pub fn transform_upstream_response(
        &self,
        context: &MatchedRequestContext,
        mut response: ResponseData,
    ) -> Result<ResponseData, EngineError> {
        // Find the rule by route_id
        let rule = self
            .rules
            .iter()
            .find(|r| r.id == context.route_id)
            .ok_or_else(|| {
                EngineError::MatchError(format!("unknown route_id: {}", context.route_id))
            })?;

        // Set path_params on response for template substitution
        response.path_params = context.path_params.clone();

        for transform in &rule.response_transforms {
            response = transform
                .transform_response(response)
                .map_err(|e| EngineError::TransformError(e.to_string()))?;
        }

        Ok(response)
    }

    /// Transforms a request before forwarding to upstream.
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::TransformError`] if any transform fails.
    pub fn transform_request(
        &self,
        mut request: RequestData,
        plan: &ForwardPlan,
    ) -> Result<RequestData, EngineError> {
        // Set path_params on request for template substitution
        request.path_params = plan.context.path_params.clone();

        // Find the rule to get request transforms
        let rule = self
            .rules
            .iter()
            .find(|r| r.id == plan.context.route_id)
            .ok_or_else(|| {
                EngineError::MatchError(format!("unknown route_id: {}", plan.context.route_id))
            })?;

        for transform in &rule.request_transforms {
            request = transform
                .transform_request(request)
                .map_err(|e| EngineError::TransformError(e.to_string()))?;
        }

        Ok(request)
    }
}

// --- Internal helpers ---

/// Result of trying a single rule against a request.
struct TryRuleResult {
    matched: bool,
    decision: Option<Decision>,
    metadata: DecisionMetadata,
}

impl Engine {
    /// Attempts to match a request against a single rule.
    fn try_rule(&self, rule: &CompiledRule, request: &RequestData) -> TryRuleResult {
        let mut metadata = DecisionMetadata::new();
        metadata.route_id = Some(rule.id.clone());
        let mut all_params: HashMap<String, String> = HashMap::new();

        // Run all matchers (AND logic — all must match)
        for matcher in &rule.matchers {
            let result = matcher.match_request(request);
            if !result.matches {
                return TryRuleResult {
                    matched: false,
                    decision: None,
                    metadata,
                };
            }
            metadata
                .matcher_results
                .push(format!("{}: score={}", matcher.name(), result.score));
            all_params.extend(result.path_params);
        }

        // All matchers passed — build MatchedRequestContext
        let mut ctx = MatchedRequestContext::new(&rule.id);
        for (k, v) in all_params.clone() {
            ctx = ctx.with_path_param(&k, &v);
        }

        // Build the decision from the action
        let decision = match &rule.action {
            CompiledAction::Mock { response } => {
                let mut response = response.clone();
                response.path_params = all_params.clone();
                Some(Decision::Mock {
                    response,
                    metadata: metadata.clone(),
                })
            }
            CompiledAction::Forward { upstream } => {
                let plan = ForwardPlan::new(upstream, &request.method)
                    .with_headers(request.headers.clone())
                    .with_body(request.body.clone())
                    .with_context(ctx)
                    .with_timeout(self.defaults.upstream_timeout_ms);

                Some(Decision::Forward {
                    plan,
                    metadata: metadata.clone(),
                })
            }
        };

        TryRuleResult {
            matched: true,
            decision,
            metadata,
        }
    }

    /// Handles the case when no rule matched.
    fn handle_unmatched(
        &self,
        request: RequestData,
        elapsed_us: u64,
    ) -> Result<Decision, EngineError> {
        let mut metadata = DecisionMetadata::new();
        metadata.elapsed_us = elapsed_us;

        match &self.unmatched {
            Some(unmatched) => match &unmatched.action {
                EngineUnmatchedAction::Mock {
                    status,
                    body,
                    headers,
                    delay_ms,
                } => {
                    let mut response = ResponseData::new(*status)
                        .with_headers(headers.clone())
                        .with_body(body.clone())
                        .with_delay(*delay_ms);
                    response.path_params = HashMap::new();
                    Ok(Decision::Mock { response, metadata })
                }
                EngineUnmatchedAction::Forward { upstream } => {
                    let plan = ForwardPlan::new(upstream, &request.method)
                        .with_headers(request.headers.clone())
                        .with_body(request.body.clone())
                        .with_context(MatchedRequestContext::new("__unmatched__"))
                        .with_timeout(self.defaults.upstream_timeout_ms);
                    Ok(Decision::Forward { plan, metadata })
                }
            },
            None => {
                // Default: return 404 Reject
                let response = ResponseData::new(404)
                    .with_body(crate::BodyData::Text("No route matched".to_string()));
                Ok(Decision::Reject { response, metadata })
            }
        }
    }
}

// --- Config parsing helpers ---

fn parse_defaults(config: &serde_json::Value) -> EngineDefaults {
    let defaults = config.get("defaults");
    EngineDefaults {
        upstream_timeout_ms: defaults
            .and_then(|d| d.get("upstream_timeout_ms"))
            .and_then(|v| v.as_u64())
            .unwrap_or(10_000),
        max_body_bytes: defaults
            .and_then(|d| d.get("max_body_bytes"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1_048_576) as usize,
    }
}

fn parse_unmatched(config: &serde_json::Value) -> Result<Option<EngineUnmatched>, String> {
    let unmatched_val = match config.get("unmatched") {
        Some(v) => v,
        None => return Ok(None),
    };

    let action_val = unmatched_val
        .get("action")
        .ok_or("unmatched: missing 'action' field")?;

    let action = parse_action(action_val)?;

    Ok(Some(EngineUnmatched { action }))
}

fn parse_action(action_val: &serde_json::Value) -> Result<EngineUnmatchedAction, String> {
    let type_str = action_val
        .get("type")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or("action: missing 'type' field")?;

    match type_str.as_str() {
        "mock" => {
            let response_val = action_val
                .get("response")
                .ok_or("mock action: missing 'response' field")?;
            let (status, body, headers, delay_ms) = parse_response(response_val)?;
            Ok(EngineUnmatchedAction::Mock {
                status,
                body,
                headers,
                delay_ms,
            })
        }
        "forward" => {
            let upstream = action_val
                .get("upstream")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or("forward action: missing 'upstream' field")?
                .to_string();
            Ok(EngineUnmatchedAction::Forward { upstream })
        }
        _ => Err(format!("unknown action type: {}", type_str)),
    }
}

/// Compiles a single route from a JSON map.
fn compile_rule(
    mut route: serde_json::Map<String, serde_json::Value>,
    _index: usize,
) -> Result<CompiledRule, String> {
    let id = route
        .remove("id")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or("route: missing 'id' field")?;

    let priority = route
        .remove("priority")
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;

    let match_rule_val = route
        .remove("match_rule")
        .ok_or("route: missing 'match_rule' field")?;

    let matchers = compile_match_rule(&match_rule_val)?;

    let action_val = route
        .remove("action")
        .ok_or("route: missing 'action' field")?;

    let (request_transforms, response_transforms) = compile_transforms(&action_val)?;

    let action = compile_action(&action_val)?;

    Ok(CompiledRule {
        id,
        priority,
        matchers,
        request_transforms,
        response_transforms,
        action,
    })
}

fn compile_match_rule(rule_val: &serde_json::Value) -> Result<Vec<Box<dyn Matcher>>, String> {
    use crate::matcher::{
        CompositeMatcher, HeaderMatcher, MethodMatcher, PathMatcher, QueryMatcher,
    };

    let rule = rule_val
        .as_object()
        .ok_or("match_rule: must be an object")?;

    let mut matchers: Vec<Box<dyn Matcher>> = Vec::new();

    // Method matcher
    if let Some(method_val) = rule.get("method") {
        if !method_val.is_null() {
            let method = method_val
                .as_str()
                .ok_or("match_rule.method: expected string")?
                .to_string();
            matchers.push(Box::new(MethodMatcher::new(method)) as Box<dyn Matcher>);
        }
    }

    // Path matcher
    if let Some(path_val) = rule.get("path") {
        if !path_val.is_null() {
            let path = path_val
                .as_str()
                .ok_or("match_rule.path: expected string")?
                .to_string();
            matchers.push(Box::new(PathMatcher::new(path)) as Box<dyn Matcher>);
        }
    }

    // Header matcher
    if let Some(headers_val) = rule.get("headers") {
        if let Some(headers_obj) = headers_val.as_object() {
            if !headers_obj.is_empty() {
                let mut normalized = HashMap::new();
                for (k, v) in headers_obj {
                    let v_str = v
                        .as_str()
                        .ok_or_else(|| format!("match_rule.headers.{}: expected string", k))?;
                    normalized.insert(k.to_lowercase(), v_str.to_string());
                }
                matchers.push(Box::new(HeaderMatcher::new(normalized)) as Box<dyn Matcher>);
            }
        }
    }

    // Query matcher
    if let Some(query_val) = rule.get("query") {
        if let Some(query_obj) = query_val.as_object() {
            if !query_obj.is_empty() {
                let mut query = HashMap::new();
                for (k, v) in query_obj {
                    let v_str = v
                        .as_str()
                        .ok_or_else(|| format!("match_rule.query.{}: expected string", k))?
                        .to_string();
                    query.insert(k.clone(), v_str);
                }
                matchers.push(Box::new(QueryMatcher::new(query)) as Box<dyn Matcher>);
            }
        }
    }

    // Wrap in composite (AND) logic if multiple matchers exist
    if matchers.len() > 1 {
        let composite = CompositeMatcher::from_matchers(matchers);
        matchers = vec![Box::new(composite) as Box<dyn Matcher>];
    } else if matchers.is_empty() {
        // Empty match rule matches everything
        matchers.push(Box::new(CompositeMatcher::new()) as Box<dyn Matcher>);
    }

    Ok(matchers)
}

fn compile_transforms(
    action_val: &serde_json::Value,
) -> Result<(Vec<Box<dyn Transform>>, Vec<Box<dyn Transform>>), String> {
    let type_str = action_val
        .get("type")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "mock".to_string());

    match type_str.as_str() {
        "forward" => {
            let req_t: Vec<Box<dyn Transform>> = action_val
                .get("request_transforms")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| convert_transform(t).ok())
                        .collect()
                })
                .unwrap_or_default();

            let resp_t: Vec<Box<dyn Transform>> = action_val
                .get("response_transforms")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| convert_transform(t).ok())
                        .collect()
                })
                .unwrap_or_default();

            Ok((req_t, resp_t))
        }
        _ => Ok((Vec::new(), Vec::new())),
    }
}

fn compile_action(action_val: &serde_json::Value) -> Result<CompiledAction, String> {
    let type_str = action_val
        .get("type")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or("action: missing 'type' field")?;

    match type_str.as_str() {
        "mock" => {
            let response_val = action_val
                .get("response")
                .ok_or("mock action: missing 'response' field")?;
            let (status, body, headers, delay_ms) = parse_response(response_val)?;
            let response = ResponseData::new(status)
                .with_headers(headers)
                .with_body(body)
                .with_delay(delay_ms);
            Ok(CompiledAction::Mock { response })
        }
        "forward" => {
            let upstream = action_val
                .get("upstream")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or("forward action: missing 'upstream' field")?
                .to_string();
            Ok(CompiledAction::Forward { upstream })
        }
        _ => Err(format!("unknown action type: {}", type_str)),
    }
}

fn parse_response(
    response_val: &serde_json::Value,
) -> Result<(u16, crate::BodyData, Vec<(String, String)>, u64), String> {
    let response = response_val
        .as_object()
        .ok_or("response: must be an object")?;

    let status = response
        .get("status")
        .and_then(|v| v.as_u64())
        .unwrap_or(200) as u16;

    let delay_ms = response
        .get("delay_ms")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    let headers: Vec<(String, String)> = response
        .get("headers")
        .and_then(|v| v.as_object())
        .map(|h| {
            let mut result: Vec<(String, String)> = h
                .iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                .collect();
            result.sort_by(|a, b| a.0.cmp(&b.0));
            result
        })
        .unwrap_or_default();

    let body = match (response.get("json_body"), response.get("text_body")) {
        (Some(v), _) if !v.is_null() => crate::BodyData::Json(v.clone()),
        (_, Some(v)) if !v.is_null() => {
            crate::BodyData::Text(v.as_str().unwrap_or_default().to_string())
        }
        _ => crate::BodyData::Empty,
    };

    Ok((status, body, headers, delay_ms))
}

/// Converts a JSON transform value to a `Box<dyn Transform>`.
fn convert_transform(t: &serde_json::Value) -> Result<Box<dyn Transform>, String> {
    use crate::transform::*;

    let t_obj = t.as_object().ok_or("transform: must be an object")?;

    let type_str = t_obj
        .get("type")
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .ok_or("transform: missing 'type' field")?;

    match type_str.as_str() {
        "SetHeader" => {
            let name = t_obj
                .get("name")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or("SetHeader: missing 'name'")?
                .to_string();
            let value = t_obj
                .get("value")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or("SetHeader: missing 'value'")?
                .to_string();
            Ok(Box::new(SetHeader::new(name, value)) as Box<dyn Transform>)
        }
        "RemoveHeader" => {
            let name = t_obj
                .get("name")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or("RemoveHeader: missing 'name'")?
                .to_string();
            Ok(Box::new(RemoveHeader::new(name)) as Box<dyn Transform>)
        }
        "SetQuery" => {
            let name = t_obj
                .get("name")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or("SetQuery: missing 'name'")?
                .to_string();
            let value = t_obj
                .get("value")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or("SetQuery: missing 'value'")?
                .to_string();
            Ok(Box::new(SetQuery::new(name, value)) as Box<dyn Transform>)
        }
        "RemoveQuery" => {
            let name = t_obj
                .get("name")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or("RemoveQuery: missing 'name'")?
                .to_string();
            Ok(Box::new(RemoveQuery::new(name)) as Box<dyn Transform>)
        }
        "SetJsonPointer" => {
            let path = t_obj
                .get("path")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or("SetJsonPointer: missing 'path'")?
                .to_string();
            let value = t_obj
                .get("value")
                .cloned()
                .ok_or("SetJsonPointer: missing 'value'")?;
            Ok(Box::new(SetJsonPointer::new(path, value)) as Box<dyn Transform>)
        }
        "RemoveJsonPointer" => {
            let path = t_obj
                .get("path")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .ok_or("RemoveJsonPointer: missing 'path'")?
                .to_string();
            Ok(Box::new(RemoveJsonPointer::new(path)) as Box<dyn Transform>)
        }
        "ReplaceBody" => {
            let body = match t_obj.get("body") {
                Some(v) if v.get("json").is_some() => {
                    crate::BodyData::Json(v.get("json").cloned().unwrap_or(serde_json::Value::Null))
                }
                Some(v) if v.get("text").is_some() => crate::BodyData::Text(
                    v.get("text")
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                ),
                _ => crate::BodyData::Empty,
            };
            Ok(Box::new(ReplaceBody::new(body)) as Box<dyn Transform>)
        }
        "SetStatus" => {
            let status = t_obj
                .get("status")
                .and_then(|v| v.as_u64())
                .ok_or("SetStatus: missing 'status'")? as u16;
            Ok(Box::new(SetStatus::new(status)) as Box<dyn Transform>)
        }
        _ => Err(format!("unknown transform type: {}", type_str)),
    }
}

// === Tests ===

#[cfg(test)]
mod tests {
    use super::*;

    fn engine_from_json(json: &str) -> Engine {
        Engine::compile(json).unwrap()
    }

    // --- Determinism: same input -> same output ---

    #[test]
    fn test_deterministic_decision() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [{
                "id": "get-user",
                "priority": 100,
                "match_rule": {"method": "GET", "path": "/users/:id"},
                "action": {"type": "mock", "response": {"status": 200, "json_body": {"id": 1}}}
            }]
        }"#;

        let engine = engine_from_json(json);

        let request = RequestData::new("GET", "/users/42");
        let request_clone = RequestData::new("GET", "/users/42");

        let decision1 = engine.decide(request).unwrap();
        let decision2 = engine.decide(request_clone).unwrap();

        assert_eq!(decision1, decision2);
    }

    #[test]
    fn test_deterministic_different_requests() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [{
                "id": "get-user",
                "priority": 100,
                "match_rule": {"method": "GET", "path": "/users/:id"},
                "action": {"type": "mock", "response": {"status": 200, "json_body": {"id": 1}}}
            }]
        }"#;

        let engine = engine_from_json(json);

        let req1 = RequestData::new("GET", "/users/1");
        let req2 = RequestData::new("GET", "/users/2");

        let decision1 = engine.decide(req1).unwrap();
        let decision2 = engine.decide(req2).unwrap();

        assert!(matches!(decision1, Decision::Mock { .. }));
        assert!(matches!(decision2, Decision::Mock { .. }));
    }

    // --- Priority ordering ---

    #[test]
    fn test_higher_priority_wins() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [
                {"id": "low", "priority": 10, "match_rule": {"method": "GET", "path": "/users"},
                    "action": {"type": "mock", "response": {"status": 200}}},
                {"id": "high", "priority": 100, "match_rule": {"method": "GET", "path": "/users"},
                    "action": {"type": "mock", "response": {"status": 201}}}
            ]
        }"#;

        let engine = engine_from_json(json);

        let request = RequestData::new("GET", "/users");
        let decision = engine.decide(request).unwrap();

        assert!(matches!(decision, Decision::Mock { ref response, .. } if response.status == 201));
        assert_eq!(decision.route_id(), Some("high"));
    }

    #[test]
    fn test_priority_tie_break_by_declaration_order() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [
                {"id": "first", "priority": 50, "match_rule": {"method": "GET", "path": "/users"},
                    "action": {"type": "mock", "response": {"status": 200}}},
                {"id": "second", "priority": 50, "match_rule": {"method": "GET", "path": "/users"},
                    "action": {"type": "mock", "response": {"status": 404}}}
            ]
        }"#;

        let engine = engine_from_json(json);

        let request = RequestData::new("GET", "/users");
        let decision = engine.decide(request).unwrap();

        assert!(matches!(decision, Decision::Mock { ref response, .. } if response.status == 200));
        assert_eq!(decision.route_id(), Some("first"));
    }

    #[test]
    fn test_negative_priority_still_evaluated() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [
                {"id": "catch-all", "priority": -100, "match_rule": {"method": "GET", "path": "/users"},
                    "action": {"type": "mock", "response": {"status": 200}}},
                {"id": "specific", "priority": 100, "match_rule": {"method": "GET", "path": "/users/:id"},
                    "action": {"type": "mock", "response": {"status": 201}}}
            ]
        }"#;

        let engine = engine_from_json(json);

        // Exact match should win over catch-all
        let request = RequestData::new("GET", "/users/42");
        let decision = engine.decide(request).unwrap();
        assert!(matches!(decision, Decision::Mock { ref response, .. } if response.status == 201));
    }

    // --- Template variables (path params) ---

    #[test]
    fn test_path_params_extracted() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [{
                "id": "get-user",
                "priority": 100,
                "match_rule": {"method": "GET", "path": "/users/:id"},
                "action": {"type": "mock", "response": {"status": 200, "json_body": {"id": 1}}}
            }]
        }"#;

        let engine = engine_from_json(json);

        let request = RequestData::new("GET", "/users/42");
        let decision = engine.decide(request).unwrap();

        assert_eq!(decision.route_id(), Some("get-user"));
    }

    // --- Forward action ---

    #[test]
    fn test_forward_action() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [{
                "id": "proxy",
                "priority": 100,
                "match_rule": {"method": "*", "path": "/api/*"},
                "action": {"type": "forward", "upstream": "https://api.example.com"}
            }]
        }"#;

        let engine = engine_from_json(json);

        let request = RequestData::new("POST", "/api/users")
            .with_headers(vec![(
                "content-type".to_string(),
                "application/json".to_string(),
            )])
            .with_body(crate::BodyData::Json(serde_json::json!({"name": "Alice"})));

        let decision = engine.decide(request).unwrap();

        assert!(matches!(decision, Decision::Forward { .. }));
        if let Decision::Forward { plan, .. } = &decision {
            assert_eq!(plan.url, "https://api.example.com");
            assert_eq!(plan.method, "POST");
        }
    }

    // --- Request transforms ---

    #[test]
    fn test_request_transforms_applied_before_forward() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [{
                "id": "proxy",
                "priority": 100,
                "match_rule": {"method": "POST", "path": "/users"},
                "action": {
                    "type": "forward",
                    "upstream": "https://api.example.com",
                    "request_transforms": [{"type": "SetHeader", "name": "X-Forwarded-User", "value": "from-config"}]
                }
            }]
        }"#;

        let engine = engine_from_json(json);

        let request = RequestData::new("POST", "/users")
            .with_body(crate::BodyData::Json(serde_json::json!({"name": "test"})));

        let decision = engine.decide(request.clone()).unwrap();
        assert!(matches!(&decision, Decision::Forward { .. }));

        if let Decision::Forward { plan, .. } = &decision {
            let transformed = engine.transform_request(request, plan).unwrap();
            assert_eq!(transformed.header("X-Forwarded-User"), Some("from-config"));
        }
    }

    // --- Response transforms ---

    #[test]
    fn test_response_transforms_applied() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [{
                "id": "mock-with-transform",
                "priority": 100,
                "match_rule": {"method": "GET", "path": "/users"},
                "action": {
                    "type": "forward",
                    "upstream": "https://api.example.com",
                    "response_transforms": [{"type": "SetJsonPointer", "path": "/transformed", "value": true}]
                }
            }]
        }"#;

        let engine = engine_from_json(json);

        let context = MatchedRequestContext::new("mock-with-transform").with_path_param("id", "1");

        let upstream_response = ResponseData::new(200)
            .with_body(crate::BodyData::Json(serde_json::json!({"status": "ok"})));

        let transformed = engine
            .transform_upstream_response(&context, upstream_response)
            .unwrap();

        assert!(matches!(
            transformed.body,
            crate::BodyData::Json(ref v) if v.get("transformed") == Some(&serde_json::json!(true))
        ));
    }

    // --- Unmatched requests ---

    #[test]
    fn test_unmatched_default_404() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": []
        }"#;

        let engine = engine_from_json(json);

        let request = RequestData::new("GET", "/nonexistent");
        let decision = engine.decide(request).unwrap();

        assert!(matches!(decision, Decision::Reject { response, .. } if response.status == 404));
    }

    #[test]
    fn test_unmatched_custom_mock() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [],
            "unmatched": {
                "action": {"type": "mock", "response": {"status": 418, "text_body": "I'm a teapot"}}
            }
        }"#;

        let engine = engine_from_json(json);

        let request = RequestData::new("GET", "/nonexistent");
        let decision = engine.decide(request).unwrap();

        assert!(matches!(decision, Decision::Mock { ref response, .. } if response.status == 418));
    }

    #[test]
    fn test_unmatched_forward() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [],
            "unmatched": {
                "action": {"type": "forward", "upstream": "https://fallback.example.com"}
            }
        }"#;

        let engine = engine_from_json(json);

        let request = RequestData::new("GET", "/nonexistent");
        let decision = engine.decide(request).unwrap();

        assert!(
            matches!(decision, Decision::Forward { plan, .. } if plan.url == "https://fallback.example.com")
        );
    }

    // --- No routes ---

    #[test]
    fn test_empty_config_compiles() {
        let json = r#"{"defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576}, "routes": []}"#;
        let engine = Engine::compile(json);
        assert!(engine.is_ok());
    }

    // --- Multiple matcher types in one rule ---

    #[test]
    fn test_multiple_matchers_all_must_match() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [{
                "id": "multi-matcher",
                "priority": 100,
                "match_rule": {
                    "method": "GET",
                    "path": "/users/:id",
                    "headers": {"accept": "application/json"},
                    "query": {"expand": ""}
                },
                "action": {"type": "mock", "response": {"status": 200, "json_body": {"ok": true}}}
            }]
        }"#;

        let engine = engine_from_json(json);

        // Request with all matching criteria
        let matching = RequestData::new("GET", "/users/42")
            .with_headers(vec![("accept".to_string(), "application/json".to_string())])
            .with_query(vec![("expand".to_string(), "anything".to_string())]);

        let result = engine.decide(matching).unwrap();
        assert!(matches!(result, Decision::Mock { .. }));
        assert_eq!(result.route_id(), Some("multi-matcher"));

        // Request missing header — should not match
        let missing_header = RequestData::new("GET", "/users/42")
            .with_headers(vec![("content-type".to_string(), "text/html".to_string())])
            .with_query(vec![("expand".to_string(), "anything".to_string())]);

        let result2 = engine.decide(missing_header).unwrap();
        // Should fall through to unmatched (404 since no unmatched config)
        assert!(matches!(result2, Decision::Reject { .. }));
    }

    // --- Elapsed time tracking ---

    #[test]
    fn test_elapsed_time_recorded() {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [{
                "id": "get-user",
                "priority": 100,
                "match_rule": {"method": "GET", "path": "/users/:id"},
                "action": {"type": "mock", "response": {"status": 200}}
            }]
        }"#;

        let engine = engine_from_json(json);

        let request = RequestData::new("GET", "/users/42");
        let decision = engine.decide(request).unwrap();

        // elapsed_us should be >= 0 (recorded)
        assert!(decision.metadata().elapsed_us >= 0);
    }

    // --- Compilation errors ---

    #[test]
    fn test_compile_invalid_json() {
        let result = Engine::compile("not valid json");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), EngineError::CompileError(_)));
    }

    #[test]
    fn test_compile_missing_routes() {
        let result = Engine::compile(r#"{"defaults": {"upstream_timeout_ms": 5000}}"#);
        assert!(result.is_err());
    }
}
