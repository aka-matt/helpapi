//! Integration tests for the mock-api server.
//!
//! These tests spawn the HTTP server with various configurations and make
//! real HTTP requests to verify the full request/response cycle.

use std::time::Duration;

/// Test configuration for basic mock routes.
fn basic_config() -> &'static str {
    r#"{
        "version": 1,
        "server": { "host": "127.0.0.1", "port": 0 },
        "defaults": { "upstream_timeout_ms": 5000, "max_body_bytes": 1048576 },
        "routes": [
            {
                "id": "get-user",
                "priority": 100,
                "match_rule": { "method": "GET", "path": "/users/:id" },
                "action": {
                    "type": "mock",
                    "response": {
                        "status": 200,
                        "headers": { "Content-Type": "application/json" },
                        "json_body": { "id": 1, "name": "Alice", "email": "alice@example.com" }
                    }
                }
            },
            {
                "id": "list-users",
                "priority": 50,
                "match_rule": { "method": "GET", "path": "/users" },
                "action": {
                    "type": "mock",
                    "response": {
                        "status": 200,
                        "headers": { "Content-Type": "application/json" },
                        "json_body": [
                            { "id": 1, "name": "Alice" },
                            { "id": 2, "name": "Bob" }
                        ]
                    }
                }
            },
            {
                "id": "create-user",
                "priority": 100,
                "match_rule": { "method": "POST", "path": "/users" },
                "action": {
                    "type": "mock",
                    "response": {
                        "status": 201,
                        "headers": { "Content-Type": "application/json" },
                        "json_body": { "id": 4, "name": "New User", "created": true }
                    }
                }
            },
            {
                "id": "delayed",
                "priority": 100,
                "match_rule": { "method": "GET", "path": "/delayed" },
                "action": {
                    "type": "mock",
                    "response": {
                        "status": 200,
                        "headers": { "Content-Type": "application/json" },
                        "json_body": { "delayed": true },
                        "delay_ms": 200
                    }
                }
            }
        ],
        "unmatched": {
            "action": {
                "type": "mock",
                "response": {
                    "status": 404,
                    "headers": { "Content-Type": "application/json" },
                    "json_body": { "error": "Not Found", "message": "No route matched" }
                }
            }
        }
    }"#
}

/// Test configuration for text body responses.
fn text_config() -> &'static str {
    r#"{
        "version": 1,
        "server": { "host": "127.0.0.1", "port": 0 },
        "defaults": { "upstream_timeout_ms": 5000, "max_body_bytes": 1048576 },
        "routes": [
            {
                "id": "text-endpoint",
                "priority": 100,
                "match_rule": { "method": "GET", "path": "/text" },
                "action": {
                    "type": "mock",
                    "response": {
                        "status": 200,
                        "headers": { "Content-Type": "text/plain" },
                        "text_body": "Hello, this is plain text!"
                    }
                }
            }
        ]
    }"#
}

/// Test configuration for rejecting requests.
fn reject_config() -> &'static str {
    r#"{
        "version": 1,
        "server": { "host": "127.0.0.1", "port": 0 },
        "defaults": { "upstream_timeout_ms": 5000, "max_body_bytes": 1048576 },
        "routes": [
            {
                "id": "reject-admin",
                "priority": 100,
                "match_rule": { "method": "GET", "path": "/admin" },
                "action": {
                    "type": "mock",
                    "response": {
                        "status": 403,
                        "headers": { "Content-Type": "application/json" },
                        "json_body": { "error": "Forbidden", "message": "Admin access denied" }
                    }
                }
            }
        ]
    }"#
}

/// Spawns a test server with the given config and returns the bound address.
async fn spawn_server(config_json: &str) -> (mock_runtime::Runtime, std::net::SocketAddr) {
    let mut runtime = mock_runtime::Runtime::new(config_json)
        .await
        .expect("failed to create runtime");
    runtime.start().await.expect("failed to start runtime");

    // Subscribe to get the server address
    let mut receiver = runtime.subscribe();
    let event = receiver
        .recv_timeout(Duration::from_secs(5))
        .await
        .expect("timeout waiting for server event")
        .expect("failed to receive event");

    let addr = match event {
        mock_http::events::RuntimeEvent::ServerStarted { address } => {
            address.parse().expect("invalid socket address")
        }
        _ => panic!("expected ServerStarted event, got {:?}", event),
    };

    (runtime, addr)
}

/// Helper to make an HTTP request and return the response.
async fn get(addr: std::net::SocketAddr, path: &str) -> reqwest::Response {
    let client = reqwest::Client::new();
    client
        .get(format!("http://{}/", addr).to_string() + path.trim_start_matches('/'))
        .send()
        .await
        .expect("request failed")
}

/// Helper to make a POST request with a JSON body.
async fn post_json(addr: std::net::SocketAddr, path: &str, body: String) -> reqwest::Response {
    let client = reqwest::Client::new();
    client
        .post(format!("http://{}/", addr).to_string() + path.trim_start_matches('/'))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
        .expect("request failed")
}

// --- Basic Mock Route Tests ---

#[tokio::test]
async fn test_mock_get_user() {
    let (mut runtime, addr) = spawn_server(basic_config()).await;

    let resp = get(addr, "/users/1").await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.expect("failed to parse JSON");
    assert_eq!(body["id"], 1);
    assert_eq!(body["name"], "Alice");
    assert_eq!(body["email"], "alice@example.com");

    runtime.stop().await.expect("failed to stop runtime");
}

#[tokio::test]
async fn test_mock_list_users() {
    let (mut runtime, addr) = spawn_server(basic_config()).await;

    let resp = get(addr, "/users").await;
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.expect("failed to parse JSON");
    assert!(body.is_array());
    assert_eq!(body.as_array().unwrap().len(), 2);

    runtime.stop().await.expect("failed to stop runtime");
}

#[tokio::test]
async fn test_mock_post_creates_resource() {
    let (mut runtime, addr) = spawn_server(basic_config()).await;

    let resp = post_json(addr, "/users", r#"{"name": "New User"}"#.to_string()).await;
    assert_eq!(resp.status(), 201);

    let body: serde_json::Value = resp.json().await.expect("failed to parse JSON");
    assert_eq!(body["name"], "New User");
    assert_eq!(body["created"], true);

    runtime.stop().await.expect("failed to stop runtime");
}

#[tokio::test]
async fn test_mock_unmatched_returns_404() {
    let (mut runtime, addr) = spawn_server(basic_config()).await;

    let resp = get(addr, "/nonexistent/path").await;
    assert_eq!(resp.status(), 404);

    let body: serde_json::Value = resp.json().await.expect("failed to parse JSON");
    assert_eq!(body["error"], "Not Found");

    runtime.stop().await.expect("failed to stop runtime");
}

#[tokio::test]
async fn test_mock_delayed_response() {
    let (mut runtime, addr) = spawn_server(basic_config()).await;

    let start = std::time::Instant::now();
    let resp = get(addr, "/delayed").await;
    let elapsed = start.elapsed();

    assert_eq!(resp.status(), 200);
    // Allow some tolerance for timing
    assert!(
        elapsed >= Duration::from_millis(150),
        "response should be delayed"
    );

    runtime.stop().await.expect("failed to stop runtime");
}

// --- Text Body Tests ---

#[tokio::test]
async fn test_mock_text_response() {
    let (mut runtime, addr) = spawn_server(text_config()).await;

    let resp = get(addr, "/text").await;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap()),
        Some("text/plain")
    );

    let text = resp.text().await.expect("failed to read text");
    assert_eq!(text, "Hello, this is plain text!");

    runtime.stop().await.expect("failed to stop runtime");
}

// --- Reject/Error Response Tests ---

#[tokio::test]
async fn test_mock_reject_status() {
    let (mut runtime, addr) = spawn_server(reject_config()).await;

    let resp = get(addr, "/admin").await;
    assert_eq!(resp.status(), 403);

    let body: serde_json::Value = resp.json().await.expect("failed to parse JSON");
    assert_eq!(body["error"], "Forbidden");

    runtime.stop().await.expect("failed to stop runtime");
}

// --- Header Tests ---

#[tokio::test]
async fn test_mock_preserves_response_headers() {
    let (mut runtime, addr) = spawn_server(basic_config()).await;

    let resp = get(addr, "/users/1").await;
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers()
            .get("content-type")
            .map(|v| v.to_str().unwrap()),
        Some("application/json")
    );

    runtime.stop().await.expect("failed to stop runtime");
}

// --- Method Matching Tests ---

#[tokio::test]
async fn test_method_mismatch_returns_404() {
    let (mut runtime, addr) = spawn_server(basic_config()).await;

    let client = reqwest::Client::new();

    // GET /users should match list-users route (status 200)
    let resp = client
        .get(format!("http://{}/users", addr))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), 200);

    // POST to /admin is not configured, should return unmatched (404)
    let resp = client
        .post(format!("http://{}/admin", addr))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), 404);

    runtime.stop().await.expect("failed to stop runtime");
}

#[tokio::test]
async fn test_post_to_get_only_route_returns_unmatched() {
    let (mut runtime, addr) = spawn_server(basic_config()).await;

    let client = reqwest::Client::new();

    // GET /users/1 returns user (200)
    let resp = client
        .get(format!("http://{}/users/1", addr))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), 200);

    // DELETE /users/1 is not configured, should return unmatched (404)
    let resp = client
        .request(reqwest::Method::DELETE, format!("http://{}/users/1", addr))
        .send()
        .await
        .expect("request failed");
    assert_eq!(resp.status(), 404);

    runtime.stop().await.expect("failed to stop runtime");
}

// --- Forward Route Tests ---

/// Test configuration for forward routes.
/// Uses mock responses to verify forward routes are properly configured
/// and recognized by the engine (actual HTTP forwarding requires Phase 3).
fn forward_config() -> &'static str {
    r#"{
        "version": 1,
        "server": { "host": "127.0.0.1", "port": 0 },
        "defaults": { "upstream_timeout_ms": 10000, "max_body_bytes": 1048576 },
        "routes": [
            {
                "id": "forward-route",
                "priority": 100,
                "match_rule": { "method": "GET", "path": "/forward" },
                "action": {
                    "type": "forward",
                    "upstream": "http://httpbin.org",
                    "request_transforms": [],
                    "response_transforms": []
                }
            }
        ],
        "unmatched": {
            "action": {
                "type": "mock",
                "response": {
                    "status": 404,
                    "headers": { "Content-Type": "application/json" },
                    "json_body": { "error": "Not Found" }
                }
            }
        }
    }"#
}

#[tokio::test]
async fn test_forward_route_is_recognized() {
    // This test verifies that forward routes are properly recognized by the engine.
    // The actual HTTP forwarding is not yet implemented in the handler layer,
    // so we verify through config parsing and engine decision.
    let config = forward_config();
    let engine = mock_core::Engine::compile(config).expect("config should compile");

    let request = mock_core::RequestData::new("GET", "/forward");
    let decision = engine.decide(request).expect("should decide");

    // Verify it's a Forward decision, not a Mock or Reject
    match decision {
        mock_core::Decision::Forward { .. } => {
            // Forward route was recognized - test passes
        }
        mock_core::Decision::Mock { response, .. } => {
            panic!("Expected Forward decision, got Mock({})", response.status);
        }
        mock_core::Decision::Reject { response, .. } => {
            panic!("Expected Forward decision, got Reject({})", response.status);
        }
    }
}

// --- Transform Tests ---

/// Test configuration for transforms.
/// Verifies that transform configs are properly parsed by the engine.
fn transform_config() -> &'static str {
    r#"{
        "version": 1,
        "server": { "host": "127.0.0.1", "port": 0 },
        "defaults": { "upstream_timeout_ms": 10000, "max_body_bytes": 1048576 },
        "routes": [
            {
                "id": "transform-route",
                "priority": 100,
                "match_rule": { "method": "POST", "path": "/api/transform" },
                "action": {
                    "type": "forward",
                    "upstream": "http://httpbin.org",
                    "request_transforms": [
                        {"type": "SetHeader", "name": "X-Request-Id", "value": "test-123"}
                    ],
                    "response_transforms": [
                        {"type": "SetJsonPointer", "path": "/transformed", "value": true}
                    ]
                }
            }
        ],
        "unmatched": {
            "action": {
                "type": "mock",
                "response": {
                    "status": 404,
                    "headers": { "Content-Type": "application/json" },
                    "json_body": { "error": "Not Found" }
                }
            }
        }
    }"#
}

#[tokio::test]
async fn test_transform_config_is_parsed() {
    // This test verifies that transform configurations are properly parsed
    // and attached to the compiled rule.
    let config = transform_config();
    let engine = mock_core::Engine::compile(config).expect("config should compile");

    // Verify transforms are stored in the compiled rule
    let routes = engine.routes();
    let transform_route = routes
        .iter()
        .find(|r| r.id == "transform-route")
        .expect("transform-route should exist");

    // Verify request transforms are non-empty
    assert!(
        !transform_route.request_transforms.is_empty(),
        "request_transforms should not be empty"
    );
    assert_eq!(
        transform_route.request_transforms.len(),
        1,
        "should have exactly 1 request transform"
    );

    // Verify response transforms are non-empty
    assert!(
        !transform_route.response_transforms.is_empty(),
        "response_transforms should not be empty"
    );
    assert_eq!(
        transform_route.response_transforms.len(),
        1,
        "should have exactly 1 response transform"
    );

    // Verify the decision is Forward
    let request = mock_core::RequestData::new("POST", "/api/transform").with_body(
        mock_core::BodyData::Json(serde_json::json!({"input": "value"})),
    );
    let decision = engine.decide(request).expect("should decide");

    match decision {
        mock_core::Decision::Forward { .. } => {
            // Forward route was recognized with transforms - test passes
        }
        mock_core::Decision::Mock { response, .. } => {
            panic!("Expected Forward decision, got Mock({})", response.status);
        }
        mock_core::Decision::Reject { response, .. } => {
            panic!("Expected Forward decision, got Reject({})", response.status);
        }
    }
}

// --- Timeout Tests ---

/// Test configuration with a short timeout value to verify timeout config is parsed.
fn timeout_config() -> &'static str {
    r#"{
        "version": 1,
        "server": { "host": "127.0.0.1", "port": 0 },
        "defaults": { "upstream_timeout_ms": 500, "max_body_bytes": 1048576 },
        "routes": [
            {
                "id": "timeout-route",
                "priority": 100,
                "match_rule": { "method": "GET", "path": "/timeout" },
                "action": {
                    "type": "mock",
                    "response": {
                        "status": 200,
                        "headers": { "Content-Type": "application/json" },
                        "json_body": { "ok": true }
                    }
                }
            }
        ],
        "unmatched": {
            "action": {
                "type": "mock",
                "response": {
                    "status": 404,
                    "headers": { "Content-Type": "application/json" },
                    "json_body": { "error": "Not Found" }
                }
            }
        }
    }"#
}

#[tokio::test]
async fn test_timeout_config_is_parsed() {
    // This test verifies that timeout configurations are properly parsed.
    // The actual timeout enforcement requires upstream forwarding to be implemented.
    let config = timeout_config();
    let engine = mock_core::Engine::compile(config).expect("config should compile");

    let request = mock_core::RequestData::new("GET", "/timeout");
    let decision = engine.decide(request).expect("should decide");

    match decision {
        mock_core::Decision::Mock { response, .. } => {
            assert_eq!(response.status, 200);
        }
        _ => panic!("Expected Mock decision for timeout test route"),
    }
}
