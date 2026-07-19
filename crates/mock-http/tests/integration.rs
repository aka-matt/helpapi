//! Integration tests for the HTTP server.
//!
//! These tests start a real HTTP server and make actual HTTP requests to it.

use std::sync::Arc;

use mock_core::Engine;
use mock_http::{HttpServer, ServerConfig};

/// Creates an engine with a simple mock route.
fn test_engine() -> Arc<tokio::sync::RwLock<Engine>> {
    let json = r#"{
        "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
        "routes": [{
            "id": "get-user",
            "priority": 100,
            "match_rule": {"method": "GET", "path": "/users/:id"},
            "action": {"type": "mock", "response": {"status": 200, "json_body": {"id": 1, "name": "Alice"}}}
        }, {
            "id": "get-posts",
            "priority": 50,
            "match_rule": {"method": "GET", "path": "/posts"},
            "action": {"type": "mock", "response": {"status": 200, "json_body": [{"id": 1}, {"id": 2}]}}
        }]
    }"#;
    Arc::new(tokio::sync::RwLock::new(Engine::compile(json).unwrap()))
}

#[tokio::test]
async fn test_mock_route_returns_json_response() {
    let config = ServerConfig::new("127.0.0.1", 0);
    let engine = test_engine();

    let server = HttpServer::start_server(config, engine, None)
        .await
        .unwrap();
    let addr = server.local_addr();

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/users/42", addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);

    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["id"], 1);
    assert_eq!(body["name"], "Alice");

    server.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
}

#[tokio::test]
async fn test_unmatched_route_returns_404() {
    let config = ServerConfig::new("127.0.0.1", 0);
    let engine = test_engine();

    let server = HttpServer::start_server(config, engine, None)
        .await
        .unwrap();
    let addr = server.local_addr();

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/nonexistent", addr))
        .send()
        .await
        .unwrap();

    // Should return 404 Reject since no route matches
    assert_eq!(response.status(), 404);

    server.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
}

#[tokio::test]
async fn test_priority_ordering() {
    let json = r#"{
        "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
        "routes": [
            {"id": "low-priority", "priority": 10, "match_rule": {"method": "GET", "path": "/test"},
                "action": {"type": "mock", "response": {"status": 200, "json_body": {"priority": 10}}}},
            {"id": "high-priority", "priority": 100, "match_rule": {"method": "GET", "path": "/test"},
                "action": {"type": "mock", "response": {"status": 200, "json_body": {"priority": 100}}}}
        ]
    }"#;
    let engine = Arc::new(tokio::sync::RwLock::new(Engine::compile(json).unwrap()));

    let config = ServerConfig::new("127.0.0.1", 0);
    let server = HttpServer::start_server(config, engine, None)
        .await
        .unwrap();
    let addr = server.local_addr();

    let client = reqwest::Client::new();
    let response = client
        .get(format!("http://{}/test", addr))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    // High priority should win
    assert_eq!(body["priority"], 100);

    server.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
}

#[tokio::test]
async fn test_post_request_with_json_body() {
    let json = r#"{
        "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
        "routes": [{
            "id": "echo",
            "priority": 100,
            "match_rule": {"method": "POST", "path": "/echo"},
            "action": {"type": "mock", "response": {"status": 200, "json_body": {"received": true}}}
        }]
    }"#;
    let engine = Arc::new(tokio::sync::RwLock::new(Engine::compile(json).unwrap()));

    let config = ServerConfig::new("127.0.0.1", 0);
    let server = HttpServer::start_server(config, engine, None)
        .await
        .unwrap();
    let addr = server.local_addr();

    let client = reqwest::Client::new();
    let response = client
        .post(format!("http://{}/echo", addr))
        .json(&serde_json::json!({"key": "value"}))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["received"], true);

    server.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
}

// Note: Forward route testing is covered by the unit test
// handler::tests::test_handle_route_forward_returns_upstream_response_or_502.
// These integration tests drive a real round-trip through HttpServer::start_server.

#[tokio::test]
async fn test_forward_round_trip_via_handle_route() {
    // Bind a tiny upstream
    let upstream = axum::Router::new().route(
        "/",
        axum::routing::any(|| async {
            axum::http::Response::builder()
                .status(200)
                .body(axum::body::Body::from("ok-from-upstream"))
                .unwrap()
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = listener.local_addr().unwrap();
    let upstream_url = format!("http://{}/", upstream_addr);
    tokio::spawn(async move {
        let _ = axum::serve(listener, upstream).await;
    });

    let cfg = format!(
        r#"{{
            "defaults": {{ "upstream_timeout_ms": 5000, "max_body_bytes": 1048576 }},
            "routes": [{{
                "id": "proxy", "priority": 100,
                "match_rule": {{ "method": "GET", "path": "/proxy" }},
                "action": {{ "type": "forward", "upstream": "{}" }}
            }}]
        }}"#,
        upstream_url
    );

    let server = HttpServer::start_server(
        ServerConfig::new("127.0.0.1", 0),
        Arc::new(tokio::sync::RwLock::new(Engine::compile(&cfg).unwrap())),
        None,
    )
    .await
    .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/proxy", server.local_addr()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "ok-from-upstream");

    server.shutdown();
}

#[tokio::test]
async fn test_forward_returns_502_when_upstream_unreachable() {
    let cfg = r#"{
        "defaults": { "upstream_timeout_ms": 1000, "max_body_bytes": 1048576 },
        "routes": [{
            "id": "dead", "priority": 100,
            "match_rule": { "method": "GET", "path": "/x" },
            "action": { "type": "forward", "upstream": "http://127.0.0.1:1/" }
        }]
    }"#;
    let server = HttpServer::start_server(
        ServerConfig::new("127.0.0.1", 0),
        Arc::new(tokio::sync::RwLock::new(Engine::compile(cfg).unwrap())),
        None,
    )
    .await
    .unwrap();
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/x", server.local_addr()))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 502);
    server.shutdown();
}

#[tokio::test]
async fn test_forward_returns_502_when_upstream_body_exceeds_5_mi_b() {
    // Spawn an upstream that returns a 6 MiB body
    let upstream = axum::Router::new().route(
        "/*path",
        axum::routing::any(|| async {
            let big = "x".repeat(6 * 1024 * 1024);
            axum::http::Response::builder()
                .status(200)
                .header("Content-Type", "text/plain")
                .body(axum::body::Body::from(big))
                .unwrap()
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let upstream_addr = listener.local_addr().unwrap();
    let upstream_url = format!("http://{}/", upstream_addr);
    tokio::spawn(async move {
        let _ = axum::serve(listener, upstream).await;
    });

    let cfg = format!(
        r#"{{
            "defaults": {{ "upstream_timeout_ms": 5000, "max_body_bytes": 1048576 }},
            "routes": [{{
                "id": "proxy", "priority": 100,
                "match_rule": {{ "method": "GET", "path": "/proxy" }},
                "action": {{ "type": "forward", "upstream": "{}" }}
            }}]
        }}"#,
        upstream_url
    );

    let server = HttpServer::start_server(
        ServerConfig::new("127.0.0.1", 0),
        Arc::new(tokio::sync::RwLock::new(Engine::compile(&cfg).unwrap())),
        None,
    )
    .await
    .unwrap();

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}/proxy", server.local_addr()))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        502,
        "expected 502 when upstream body exceeds max_response_bytes"
    );
    server.shutdown();
}
