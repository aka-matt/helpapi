//! Upstream HTTP client for forwarding requests.

use std::time::Duration;

use mock_core::{BodyData, ForwardPlan, RequestData, ResponseData};
use reqwest::Client;
use tracing::{debug, warn};

use crate::error::UpstreamError;

/// Client for forwarding requests to upstream services.
#[derive(Clone)]
pub struct UpstreamClient {
    client: Client,
}

impl UpstreamClient {
    /// Creates a new UpstreamClient with default settings.
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to create upstream HTTP client");

        Self { client }
    }

    /// Creates a new UpstreamClient with a custom timeout.
    pub fn with_timeout(timeout: Duration) -> Self {
        let client = Client::builder()
            .timeout(timeout)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("failed to create upstream HTTP client");

        Self { client }
    }

    /// Sends a ForwardPlan to the upstream service.
    ///
    /// First transforms the request using the engine's transform_request,
    /// then sends it upstream, and finally transforms the response.
    pub async fn send(
        &self,
        plan: ForwardPlan,
        request: RequestData,
        engine: &mock_core::Engine,
    ) -> Result<ResponseData, UpstreamError> {
        debug!(url = %plan.url, method = %plan.method, "forwarding request to upstream");

        // Transform request before forwarding
        let transformed_request = engine.transform_request(request, &plan).map_err(|e| {
            UpstreamError::InvalidResponse(format!("request transform failed: {}", e))
        })?;

        // Build the upstream request
        let mut req_builder = self.client.request(
            reqwest::Method::from_bytes(plan.method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &plan.url,
        );

        // Add headers (filtering out hop-by-hop headers)
        for (name, value) in &transformed_request.headers {
            let name_lower = name.to_lowercase();
            // Skip hop-by-hop headers
            if !is_hop_by_hop_header(&name_lower) {
                req_builder = req_builder.header(name.as_str(), value.as_str());
            }
        }

        // Add body
        match &transformed_request.body {
            BodyData::Empty => {}
            BodyData::Text(s) => {
                req_builder = req_builder.body(s.clone());
            }
            BodyData::Json(v) => {
                req_builder = req_builder.json(&v);
            }
            BodyData::Binary(b) => {
                req_builder = req_builder.body(b.clone());
            }
        }

        // Apply timeout from plan
        let timeout_ms = plan.timeout_ms.unwrap_or(30000);
        let response = req_builder
            .timeout(Duration::from_millis(timeout_ms))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    UpstreamError::Timeout { timeout_ms }
                } else {
                    UpstreamError::ConnectionError(e)
                }
            })?;

        // Check status
        let status = response.status();

        // Get headers before consuming body
        let response_headers: Vec<(String, String)> = response
            .headers()
            .iter()
            .filter(|(name, _)| !is_hop_by_hop_header(&name.as_str().to_lowercase()))
            .map(|(name, value)| {
                (
                    name.as_str().to_string(),
                    value.to_str().unwrap_or_default().to_string(),
                )
            })
            .collect();

        if !status.is_success() {
            let reason = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            warn!(status = %status, "upstream returned error status");
            return Err(UpstreamError::UpstreamStatus {
                status: status.as_u16(),
                reason,
            });
        }

        // Read response body
        let body_bytes = response
            .bytes()
            .await
            .map_err(UpstreamError::ConnectionError)?;

        // Parse response body
        let body = if body_bytes.is_empty() {
            BodyData::Empty
        } else if let Ok(text) = String::from_utf8(body_bytes.to_vec()) {
            // Try to parse as JSON
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
                BodyData::Json(json)
            } else {
                BodyData::Text(text)
            }
        } else {
            BodyData::Binary(body_bytes.to_vec())
        };

        // Build response
        let mut response_data = ResponseData::new(status.as_u16()).with_body(body);
        response_data.headers = response_headers;

        debug!(status = %status, "upstream response received");

        Ok(response_data)
    }

    /// Sends a simple GET request to the specified URL.
    pub async fn get(&self, url: &str) -> Result<ResponseData, UpstreamError> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(UpstreamError::ConnectionError)?;

        let status = response.status();
        let body_bytes = response
            .bytes()
            .await
            .map_err(UpstreamError::ConnectionError)?;

        let body = if body_bytes.is_empty() {
            BodyData::Empty
        } else if let Ok(text) = String::from_utf8(body_bytes.to_vec()) {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
                BodyData::Json(json)
            } else {
                BodyData::Text(text)
            }
        } else {
            BodyData::Binary(body_bytes.to_vec())
        };

        Ok(ResponseData::new(status.as_u16()).with_body(body))
    }
}

impl Default for UpstreamClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Headers that should not be forwarded to upstream servers.
fn is_hop_by_hop_header(name: &str) -> bool {
    matches!(
        name,
        "connection"
            | "keep-alive"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailers"
            | "transfer-encoding"
            | "upgrade"
            | "host"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hop_by_hop_headers() {
        let hop_by_hop = vec![
            "connection",
            "keep-alive",
            "proxy-authenticate",
            "proxy-authorization",
            "te",
            "trailers",
            "transfer-encoding",
            "upgrade",
            "host",
        ];

        for header in hop_by_hop {
            assert!(
                is_hop_by_hop_header(header),
                "should be hop-by-hop: {}",
                header
            );
        }

        assert!(!is_hop_by_hop_header("content-type"));
        assert!(!is_hop_by_hop_header("authorization"));
        assert!(!is_hop_by_hop_header("x-custom-header"));
    }

    #[test]
    fn test_upstream_client_default() {
        let client = UpstreamClient::default();
        // Basic creation test - actual network tests would need a server
        drop(client);
    }

    #[test]
    fn test_upstream_client_with_timeout() {
        let client = UpstreamClient::with_timeout(Duration::from_secs(10));
        drop(client);
    }

    #[tokio::test]
    async fn test_upstream_does_not_follow_redirects() {
        // Spawn a server that responds 301 + Location: http://127.0.0.1:1/
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let (mut sock, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 1024];
            let _ = sock.read(&mut buf).await;
            let response = "HTTP/1.1 301 Moved Permanently\r\nLocation: http://127.0.0.1:1/\r\nContent-Length: 0\r\n\r\n";
            let _ = sock.write_all(response.as_bytes()).await;
        });

        let client = UpstreamClient::with_timeout(Duration::from_secs(2));
        let response = client.get(&format!("http://{}/", addr)).await.unwrap();
        assert_eq!(
            response.status, 301,
            "expected the 301 to be returned, not chased"
        );

        task.abort();
    }
}
