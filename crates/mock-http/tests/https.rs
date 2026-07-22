//! Integration tests for HTTPS (TLS) serving from JKS keystores.
//!
//! These tests start a real HTTPS server using the test keystores:
//!
//! - `examples/test-keystore.jks` — keystore password `changeit`,
//!   key password `changeit` (same as keystore password).
//! - `tests/fixtures/test-keystore-diffpass.jks` — keystore password
//!   `changeit`, key password `keypass123` (different from keystore password).
//!
//! Both hold a self-signed certificate for `*.localhost` / `localhost`.

use std::sync::Arc;

use mock_core::Engine;
use mock_http::{HttpServer, ServerConfig, TlsSettings};

/// Path to the shared example keystore (keypass == storepass == "changeit").
fn example_keystore() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/test-keystore.jks"
    )
    .to_string()
}

/// Path to the fixture keystore with a distinct key password ("keypass123").
fn diffpass_keystore() -> String {
    concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/test-keystore-diffpass.jks"
    )
    .to_string()
}

/// Creates an engine with a simple mock route.
fn test_engine() -> Arc<tokio::sync::RwLock<Engine>> {
    let json = r#"{
        "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
        "routes": [{
            "id": "get-user",
            "priority": 100,
            "match_rule": {"method": "GET", "path": "/users/:id"},
            "action": {"type": "mock", "response": {"status": 200, "json_body": {"id": 1, "name": "Alice"}}}
        }]
    }"#;
    Arc::new(tokio::sync::RwLock::new(Engine::compile(json).unwrap()))
}

/// HTTP client that accepts the self-signed test certificate.
fn insecure_client() -> reqwest::Client {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap()
}

#[tokio::test]
async fn test_https_server_serves_mock_route() {
    let config = ServerConfig::new("127.0.0.1", 0).with_tls(TlsSettings {
        keystore_file: example_keystore(),
        keystore_password: "changeit".to_string(),
        key_password: Some("changeit".to_string()),
    });
    let engine = test_engine();

    let server = HttpServer::start_server(config, engine, None)
        .await
        .unwrap();
    let addr = server.local_addr();

    let response = insecure_client()
        .get(format!("https://localhost:{}/users/42", addr.port()))
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
async fn test_https_key_password_falls_back_to_keystore_password() {
    // The example keystore uses "changeit" for both passwords, so omitting
    // key_password must work.
    let config = ServerConfig::new("127.0.0.1", 0).with_tls(TlsSettings {
        keystore_file: example_keystore(),
        keystore_password: "changeit".to_string(),
        key_password: None,
    });
    let engine = test_engine();

    let server = HttpServer::start_server(config, engine, None)
        .await
        .unwrap();
    let addr = server.local_addr();

    let response = insecure_client()
        .get(format!("https://localhost:{}/users/7", addr.port()))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    server.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
}

#[tokio::test]
async fn test_https_distinct_key_password() {
    let config = ServerConfig::new("127.0.0.1", 0).with_tls(TlsSettings {
        keystore_file: diffpass_keystore(),
        keystore_password: "changeit".to_string(),
        key_password: Some("keypass123".to_string()),
    });
    let engine = test_engine();

    let server = HttpServer::start_server(config, engine, None)
        .await
        .unwrap();
    let addr = server.local_addr();

    let response = insecure_client()
        .get(format!("https://localhost:{}/users/1", addr.port()))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    server.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
}

#[tokio::test]
async fn test_https_wrong_keystore_password_fails() {
    let config = ServerConfig::new("127.0.0.1", 0).with_tls(TlsSettings {
        keystore_file: example_keystore(),
        keystore_password: "wrong-password".to_string(),
        key_password: None,
    });
    let engine = test_engine();

    let result = HttpServer::start_server(config, engine, None).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, mock_http::HttpError::TlsError { .. }),
        "expected TlsError, got: {:?}",
        err
    );
}

#[tokio::test]
async fn test_https_wrong_key_password_fails() {
    let config = ServerConfig::new("127.0.0.1", 0).with_tls(TlsSettings {
        keystore_file: diffpass_keystore(),
        keystore_password: "changeit".to_string(),
        key_password: Some("wrong-key-password".to_string()),
    });
    let engine = test_engine();

    let result = HttpServer::start_server(config, engine, None).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, mock_http::HttpError::TlsError { .. }),
        "expected TlsError, got: {:?}",
        err
    );
}

#[tokio::test]
async fn test_https_missing_keystore_file_fails() {
    let config = ServerConfig::new("127.0.0.1", 0).with_tls(TlsSettings {
        keystore_file: "does-not-exist.jks".to_string(),
        keystore_password: "changeit".to_string(),
        key_password: None,
    });
    let engine = test_engine();

    let result = HttpServer::start_server(config, engine, None).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        matches!(err, mock_http::HttpError::TlsError { .. }),
        "expected TlsError, got: {:?}",
        err
    );
}

#[tokio::test]
async fn test_https_graceful_shutdown() {
    let config = ServerConfig::new("127.0.0.1", 0).with_tls(TlsSettings {
        keystore_file: example_keystore(),
        keystore_password: "changeit".to_string(),
        key_password: None,
    });
    let engine = test_engine();

    let server = HttpServer::start_server(config, engine, None)
        .await
        .unwrap();
    let addr = server.local_addr();

    // Verify the server is serving HTTPS.
    let response = insecure_client()
        .get(format!("https://localhost:{}/users/1", addr.port()))
        .send()
        .await;
    assert!(response.is_ok());

    // Shutdown should complete without hanging.
    server.shutdown();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
}
