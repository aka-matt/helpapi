//! HTTP request handler — converts Axum requests to Engine decisions.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, Method, StatusCode, Uri},
    response::Response,
};
use bytes::Bytes;
use http_body::Body as HttpBody;
use http_body_util::BodyExt;
use mock_core::{BodyData, Decision, Engine, RequestData, ResponseData};
use tokio::time::timeout;
use tracing::{debug, error};

use crate::error::HttpError;

/// Maximum time to wait for request body in milliseconds.
const BODY_READ_TIMEOUT_MS: u64 = 5000;

/// Handles an incoming HTTP request by routing it through the mock engine.
///
/// Reads the body, converts to canonical `RequestData`, and asks the engine for
/// a `Decision`. Response building is the caller's responsibility: `Mock` and
/// `Reject` decisions are turned into `Response` values via [`build_mock_response`]
/// / [`build_reject_response`]; `Forward` decisions are executed by `handle_route`
/// via `UpstreamClient::send`.
pub async fn handle_request(
    request: Request,
    engine: Arc<tokio::sync::RwLock<Engine>>,
    max_body_bytes: usize,
) -> Result<(Decision, RequestData), HttpError> {
    let start_time = std::time::Instant::now();

    // Extract request components
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();

    debug!(%method, %uri, "handling request");

    // Read and validate body
    let body = read_body(request.into_body(), max_body_bytes).await?;
    let elapsed = start_time.elapsed();

    debug!(
        body_size = body.size_bytes(),
        read_time_ms = elapsed.as_millis() as u64,
        "body read complete"
    );

    // Convert to RequestData
    let request_data = convert_request(method.clone(), uri.clone(), headers.clone(), body)?;

    // Call engine decision (acquire read lock for the duration of decision)
    let decision = {
        let engine_guard = engine.read().await;
        engine_guard.decide(request_data.clone()).map_err(|e| {
            error!("engine.decide() failed: {}", e);
            HttpError::ServerStopped(e.to_string())
        })?
    };

    debug!(?decision, "engine decision made");
    debug!(
        decision_ms = start_time.elapsed().as_millis() as u64,
        "decision produced"
    );

    Ok((decision, request_data))
}

/// Reads the request body, enforcing the size limit.
async fn read_body(body: Body, max_body_bytes: usize) -> Result<BodyData, HttpError> {
    // For empty body, return early
    if body.is_end_stream() {
        return Ok(BodyData::Empty);
    }

    let mut accumulated: Vec<u8> = Vec::with_capacity(8192);
    let mut total_size = 0;

    // Stream body with timeout
    let mut body_stream = body;

    loop {
        let frame = match timeout(
            std::time::Duration::from_millis(BODY_READ_TIMEOUT_MS),
            body_stream.frame(),
        )
        .await
        {
            Ok(Some(frame)) => frame,
            Ok(None) => break, // End of stream
            Err(_) => {
                // Timeout, return what we have
                break;
            }
        };

        let frame = frame.map_err(|_| HttpError::BodyReadError)?;

        // Check if this is a data frame
        if let Some(data) = frame.data_ref() {
            let chunk = &data[..];
            total_size += chunk.len();

            if total_size > max_body_bytes {
                return Err(HttpError::BodyTooLarge {
                    size: total_size,
                    limit: max_body_bytes,
                });
            }

            accumulated.extend_from_slice(chunk);
        }
    }

    // Convert bytes to BodyData
    if accumulated.is_empty() {
        Ok(BodyData::Empty)
    } else if let Ok(text) = String::from_utf8(accumulated.clone()) {
        // Try to parse as JSON first
        if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&accumulated) {
            Ok(BodyData::Json(json))
        } else {
            Ok(BodyData::Text(text))
        }
    } else {
        // Binary content
        Ok(BodyData::Binary(accumulated))
    }
}

/// Converts Axum request components to a RequestData.
fn convert_request(
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: BodyData,
) -> Result<RequestData, HttpError> {
    let method_str = method.to_string();

    // Extract path and query
    let path = uri.path().to_string();
    let query: Vec<(String, String)> = uri
        .query()
        .map(|q| {
            let mut pairs: Vec<(String, String)> = form_urlencoded::parse(q.as_bytes())
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            pairs
        })
        .unwrap_or_default();

    // Convert headers (lowercase keys as per RequestData convention)
    let headers_vec: Vec<(String, String)> = headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_lowercase(),
                value.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect();

    Ok(RequestData::new(method_str, path)
        .with_query(query)
        .with_headers(headers_vec)
        .with_body(body))
}

/// Builds an HTTP response for a Mock decision or a Reject decision.
pub async fn build_mock_response(response: ResponseData) -> Result<Response, HttpError> {
    // Apply delay if specified
    if let Some(delay_ms) = response.delay_ms {
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
    }

    // Build response
    let mut builder = axum::response::Response::builder()
        .status(StatusCode::from_u16(response.status).unwrap_or(StatusCode::OK));

    // Add headers
    for (name, value) in &response.headers {
        builder = builder.header(name.as_str(), value.as_str());
    }

    // Add body
    let body_bytes = match &response.body {
        BodyData::Empty => Bytes::new(),
        BodyData::Text(s) => Bytes::from(s.clone()),
        BodyData::Json(v) => Bytes::from(v.to_string()),
        BodyData::Binary(b) => Bytes::from(b.clone()),
    };

    // Set content-type if not already set
    if !response
        .headers
        .iter()
        .any(|(k, _)| k.to_lowercase() == "content-type")
    {
        if let Some(ct) = response.body.content_type() {
            builder = builder.header("Content-Type", ct);
        }
    }

    let response = builder
        .body(axum::body::Body::from(body_bytes))
        .map_err(|_| HttpError::ServerStopped("failed to build response".to_string()))?;

    Ok(response)
}

/// Builds an HTTP response for a Reject decision.
pub async fn build_reject_response(response: ResponseData) -> Result<Response, HttpError> {
    build_mock_response(response).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use mock_core::Engine;

    fn test_engine() -> Arc<Engine> {
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [{
                "id": "get-user",
                "priority": 100,
                "match_rule": {"method": "GET", "path": "/users/:id"},
                "action": {"type": "mock", "response": {"status": 200, "json_body": {"id": 1}}}
            }]
        }"#;
        Arc::new(Engine::compile(json).unwrap())
    }

    #[tokio::test]
    async fn test_convert_request() {
        let method = Method::GET;
        let uri = Uri::from_static("/users/123?expand=true&sort=name");
        let headers = HeaderMap::new();

        let request_data = convert_request(method, uri, headers, BodyData::Empty).unwrap();

        assert_eq!(request_data.method, "GET");
        assert_eq!(request_data.path, "/users/123");
        assert_eq!(request_data.query.len(), 2);
        assert_eq!(request_data.query[0].0, "expand");
        assert_eq!(request_data.query[1].0, "sort");
    }

    #[tokio::test]
    async fn test_read_body_empty() {
        let body = Body::empty();
        let result = read_body(body, 1024).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_read_body_json() {
        let json = br#"{"key": "value"}"#;
        let body = Body::from(json.as_slice());
        let result = read_body(body, 1024).await.unwrap();

        match result {
            BodyData::Json(v) => assert_eq!(v["key"], "value"),
            _ => panic!("expected JSON body"),
        }
    }

    #[tokio::test]
    async fn test_read_body_text() {
        let text = b"plain text content";
        let body = Body::from(text.as_slice());
        let result = read_body(body, 1024).await.unwrap();

        match result {
            BodyData::Text(s) => assert_eq!(s, "plain text content"),
            _ => panic!("expected text body"),
        }
    }

    #[tokio::test]
    async fn test_read_body_size_limit() {
        let body = Body::from(b"too much content".as_slice());
        let result = read_body(body, 5).await;

        assert!(matches!(result, Err(HttpError::BodyTooLarge { .. })));
    }

    #[tokio::test]
    async fn test_handle_request_forward_decision() {
        // Create engine with a forward route
        let json = r#"{
            "defaults": {"upstream_timeout_ms": 5000, "max_body_bytes": 1048576},
            "routes": [{
                "id": "proxy",
                "priority": 100,
                "match_rule": {"method": "GET", "path": "/proxy"},
                "action": {"type": "forward", "upstream": "https://api.example.com"}
            }]
        }"#;
        let engine = Arc::new(tokio::sync::RwLock::new(Engine::compile(json).unwrap()));

        // Create a simple request
        let request = Request::builder()
            .uri("/proxy")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let (decision, request_data) = handle_request(request, engine, 1024).await.unwrap();

        // Forward decisions surface the original request data and a Forward plan;
        // the actual upstream HTTP call is executed by handle_route.
        assert!(
            matches!(decision, Decision::Forward { .. }),
            "Expected Forward decision"
        );
        assert_eq!(request_data.method, "GET");
        assert_eq!(request_data.path, "/proxy");
    }

    #[tokio::test]
    async fn test_handle_route_forward_returns_upstream_response_or_502() {
        // Spin a tiny upstream
        let upstream = axum::Router::new().route(
            "/",
            axum::routing::any(|| async {
                axum::http::Response::builder()
                    .status(200)
                    .body(axum::body::Body::from("upstream-ok"))
                    .unwrap()
            }),
        );
        let upstream_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let upstream_addr = upstream_listener.local_addr().unwrap();
        let upstream_url = format!("http://{}/", upstream_addr);
        let _upstream_task = tokio::spawn(async move {
            let _ = axum::serve(upstream_listener, upstream).await;
        });

        // Engine with a forward route pointing at that upstream
        let json = format!(
            r#"{{
                "defaults": {{"upstream_timeout_ms": 5000, "max_body_bytes": 1048576}},
                "routes": [{{
                    "id": "proxy",
                    "priority": 100,
                    "match_rule": {{"method": "GET", "path": "/proxy"}},
                    "action": {{"type": "forward", "upstream": "{}"}}
                }}]
            }}"#,
            upstream_url
        );
        let engine = Arc::new(tokio::sync::RwLock::new(
            mock_core::Engine::compile(&json).unwrap(),
        ));

        // Bind a mock-api server on a random port
        use crate::server::{HttpServer, ServerConfig};
        let server = HttpServer::start_server(ServerConfig::new("127.0.0.1", 0), engine, None)
            .await
            .unwrap();
        let addr = server.local_addr();

        let client = reqwest::Client::new();
        let response = client
            .get(format!("http://{}/proxy", addr))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        let body = response.text().await.unwrap();
        assert_eq!(body, "upstream-ok");

        server.shutdown();
    }
}
