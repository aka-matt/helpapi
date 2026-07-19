//! WasmMockEngine — WASM-safe wrapper around mock_core::Engine.

use wasm_bindgen::prelude::*;

use crate::error::WasmError;
use mock_config::parse_and_validate;
use mock_core::{Engine, MatchedRequestContext, RequestData, ResponseData};

/// A compiled mock engine for use in WASM environments.
///
/// This struct wraps `mock_core::Engine` and provides a WASM-safe API
/// that accepts and returns JSON strings.
#[wasm_bindgen]
pub struct WasmMockEngine {
    engine: Engine,
}

#[wasm_bindgen]
impl WasmMockEngine {
    /// Creates a new WasmMockEngine from a JSON config string.
    ///
    /// # Errors
    ///
    /// Returns a JsValue error if the config is invalid JSON or fails validation.
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Result<WasmMockEngine, JsValue> {
        let config = parse_and_validate(config_json).map_err(WasmError::from)?;
        let config_str = serde_json::to_string(&config).map_err(WasmError::from)?;
        let engine = Engine::compile(&config_str).map_err(WasmError::from)?;
        Ok(WasmMockEngine { engine })
    }

    /// Evaluates a request against the compiled rules and returns a decision.
    ///
    /// # Arguments
    ///
    /// * `request_json` - JSON string representing a `RequestData` object.
    ///
    /// # Errors
    ///
    /// Returns a JsValue error if the request JSON is invalid or the engine
    /// fails to produce a decision.
    #[wasm_bindgen]
    pub fn decide(&self, request_json: &str) -> Result<String, JsValue> {
        let request: RequestData = serde_json::from_str(request_json).map_err(WasmError::from)?;
        let decision = self.engine.decide(request).map_err(WasmError::from)?;
        let json = serde_json::to_string(&decision).map_err(WasmError::from)?;
        Ok(json)
    }

    /// Transforms an upstream response using the response transforms from
    /// the matched route.
    ///
    /// # Arguments
    ///
    /// * `context_json` - JSON string representing a `MatchedRequestContext`.
    /// * `response_json` - JSON string representing a `ResponseData`.
    ///
    /// # Errors
    ///
    /// Returns a JsValue error if the JSON is invalid or the transform fails.
    #[wasm_bindgen]
    pub fn transform_response(
        &self,
        context_json: &str,
        response_json: &str,
    ) -> Result<String, JsValue> {
        let context: MatchedRequestContext =
            serde_json::from_str(context_json).map_err(WasmError::from)?;
        let response: ResponseData =
            serde_json::from_str(response_json).map_err(WasmError::from)?;
        let transformed = self
            .engine
            .transform_upstream_response(&context, response)
            .map_err(WasmError::from)?;
        let json = serde_json::to_string(&transformed).map_err(WasmError::from)?;
        Ok(json)
    }
}

// --- Unit tests for the Engine wrapper (run on host, not WASM) ---
// Note: These tests call the underlying mock_core Engine directly,
// since WasmMockEngine requires WASM runtime.

#[cfg(test)]
mod tests {
    use super::*;
    use mock_core::{Decision, ResponseData};

    const VALID_CONFIG: &str = r#"{
        "version": 1,
        "server": { "host": "127.0.0.1", "port": 8080 },
        "defaults": { "upstream_timeout_ms": 10000, "max_body_bytes": 1048576 },
        "routes": [
            {
                "id": "get-user",
                "priority": 100,
                "match_rule": { "method": "GET", "path": "/users/:id" },
                "action": {
                    "type": "mock",
                    "response": { "status": 200, "json_body": { "id": 1, "name": "Alice" } }
                }
            }
        ]
    }"#;

    const VALID_REQUEST: &str = r#"{
        "method": "GET",
        "path": "/users/42",
        "query": [],
        "headers": [],
        "body": "empty"
    }"#;

    fn compile_engine(config_json: &str) -> Engine {
        // parse_and_validate ensures the config is valid
        let _ = parse_and_validate(config_json).unwrap();
        // Engine::compile handles JSON parsing directly
        Engine::compile(config_json).unwrap()
    }

    #[test]
    fn test_engine_decide_mock_response() {
        let engine = compile_engine(VALID_CONFIG);
        let request: RequestData = serde_json::from_str(VALID_REQUEST).unwrap();
        let decision = engine.decide(request).unwrap();

        match decision {
            Decision::Mock { response, metadata } => {
                assert_eq!(response.status, 200);
                assert_eq!(metadata.route_id, Some("get-user".to_string()));
            }
            other => panic!("Expected Mock decision, got {:?}", other),
        }
    }

    #[test]
    fn test_engine_decide_unmatched_request() {
        let engine = compile_engine(VALID_CONFIG);
        // Request to a path that doesn't match any route
        let request_json = r#"{
            "method": "GET",
            "path": "/unknown",
            "query": [],
            "headers": [],
            "body": "empty"
        }"#;
        let request: RequestData = serde_json::from_str(request_json).unwrap();
        let decision = engine.decide(request).unwrap();

        // Should get a reject or unmatched response
        match decision {
            Decision::Reject { .. } | Decision::Mock { .. } => {}
            Decision::Forward { .. } => panic!("Expected Reject or Mock, got Forward"),
        }
    }

    #[test]
    fn test_engine_transform_response() {
        let engine = compile_engine(VALID_CONFIG);
        let context = MatchedRequestContext::new("get-user");
        let response = ResponseData::new(200);
        let result = engine.transform_upstream_response(&context, response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().status, 200);
    }

    #[test]
    fn test_engine_transform_response_unknown_route() {
        let engine = compile_engine(VALID_CONFIG);
        let context = MatchedRequestContext::new("unknown-route");
        let response = ResponseData::new(200);
        let result = engine.transform_upstream_response(&context, response);
        assert!(result.is_err());
    }
}
