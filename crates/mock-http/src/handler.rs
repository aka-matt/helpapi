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
use mock_core::{BodyData, Decision, Engine, ForwardPlan, RequestData, ResponseData};
use tokio::time::timeout;
use tracing::{debug, error};

use crate::error::HttpError;

/// Maximum time to wait for request body in milliseconds.
const BODY_READ_TIMEOUT_MS: u64 = 5000;

/// Handles an incoming HTTP request by routing it through the mock engine.
pub async fn handle_request(
    request: Request,
    engine: Arc<Engine>,
    max_body_bytes: usize,
) -> Result<Response, HttpError> {
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
    let request_data = convert_request(method, uri, headers, body)?;

    // Call engine decision
    let decision = engine.decide(request_data).map_err(|e| {
        error!("engine.decide() failed: {}", e);
        HttpError::ServerStopped(e.to_string())
    })?;

    debug!(?decision, "engine decision made");

    // Build response based on decision type
    debug!(?decision, "building response for decision");
    let response = match &decision {
        Decision::Mock { response, .. } => {
            debug!("building mock response");
            build_mock_response(response.clone()).await
        }
        Decision::Reject { response, .. } => {
            debug!("building reject response");
            build_reject_response(response.clone()).await
        }
        Decision::Forward { plan, .. } => {
            debug!("building forward response with plan");
            Ok(build_forward_response(plan.clone()))
        }
    };
    debug!("response built, returning");
    response
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

/// Builds an HTTP response for a Mock decision.
async fn build_mock_response(response: ResponseData) -> Result<Response, HttpError> {
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
async fn build_reject_response(response: ResponseData) -> Result<Response, HttpError> {
    build_mock_response(response).await
}

/// Builds a special response that carries the ForwardPlan.
/// This uses a custom header to encode the forward information.
/// In practice, this would be handled by returning a special type that
/// the server can detect and process.
fn build_forward_response(plan: ForwardPlan) -> Response {
    // Serialize the ForwardPlan to JSON and embed it
    let plan_json = serde_json::to_string(&plan).unwrap_or_default();

    axum::response::Response::builder()
        .status(StatusCode::PROCESSING) // 102 Processing
        .header("X-Forward-Plan", plan_json)
        .body(axum::body::Body::empty())
        .expect("failed to build forward response")
}

/// Extracts a ForwardPlan from a response (if present).
pub fn extract_forward_plan(response: &Response) -> Option<ForwardPlan> {
    let plan_header = response.headers().get("X-Forward-Plan")?;

    let plan_str = plan_header.to_str().ok()?;
    serde_json::from_str(plan_str).ok()
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
        let engine = Arc::new(Engine::compile(json).unwrap());

        // Create a simple request
        let request = Request::builder()
            .uri("/proxy")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = handle_request(request, engine, 1024).await.unwrap();

        // Check that we got a forward response
        let status = response.status();
        assert_eq!(status, 102, "Expected 102 Processing, got {}", status);

        // Check that X-Forward-Plan header is present
        assert!(
            response.headers().contains_key("x-forward-plan"),
            "Missing X-Forward-Plan header"
        );
    }
}
