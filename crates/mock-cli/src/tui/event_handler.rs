//! TUI event handling utilities.
//!
//! Provides helper functions for processing and displaying runtime events
//! in the TUI, including sensitive header masking.

/// List of sensitive header names that should be masked in the UI.
const SENSITIVE_HEADERS: &[&str] = &[
    "authorization",
    "cookie",
    "proxy-authorization",
];

/// Masked value used for sensitive headers.
const MASKED_VALUE: &str = "***MASKED***";

/// Checks if a header name is sensitive (should be masked in the UI).
pub fn is_sensitive_header(name: &str) -> bool {
    let name_lower = name.to_lowercase();
    SENSITIVE_HEADERS.iter().any(|h| *h == name_lower)
}

/// Returns a masked version of the header value if the header is sensitive,
/// otherwise returns the original value.
pub fn mask_header_value(name: &str, value: &str) -> String {
    if is_sensitive_header(name) {
        MASKED_VALUE.to_string()
    } else {
        value.to_string()
    }
}

/// Masks sensitive headers in a list of header tuples.
///
/// This function creates a new vector with sensitive header values replaced
/// with "***MASKED***" for display in the UI.
pub fn mask_sensitive_headers(headers: &[(String, String)]) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            let masked_name = name.to_lowercase();
            if SENSITIVE_HEADERS.iter().any(|h| *h == masked_name) {
                (name.clone(), MASKED_VALUE.to_string())
            } else {
                (name.clone(), value.clone())
            }
        })
        .collect()
}

/// Maximum body preview size (32 KiB) for UI display.
///
/// Bodies larger than this will be truncated to prevent memory issues
/// and to keep the UI responsive.
pub const MAX_BODY_PREVIEW: usize = 32 * 1024;

/// Truncates body bytes to the maximum preview size for display.
///
/// Returns the truncated body. The original body is returned unchanged
/// if it is smaller than the maximum size.
pub fn truncate_body(body: &[u8]) -> &[u8] {
    if body.len() > MAX_BODY_PREVIEW {
        &body[..MAX_BODY_PREVIEW]
    } else {
        body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_sensitive_header_authorization() {
        assert!(is_sensitive_header("authorization"));
        assert!(is_sensitive_header("Authorization"));
        assert!(is_sensitive_header("AUTHORIZATION"));
    }

    #[test]
    fn test_is_sensitive_header_cookie() {
        assert!(is_sensitive_header("cookie"));
        assert!(is_sensitive_header("Cookie"));
        assert!(is_sensitive_header("COOKIE"));
    }

    #[test]
    fn test_is_sensitive_header_proxy_authorization() {
        assert!(is_sensitive_header("proxy-authorization"));
        assert!(is_sensitive_header("Proxy-Authorization"));
    }

    #[test]
    fn test_is_sensitive_header_non_sensitive() {
        assert!(!is_sensitive_header("content-type"));
        assert!(!is_sensitive_header("accept"));
        assert!(!is_sensitive_header("x-custom-header"));
    }

    #[test]
    fn test_mask_header_value_sensitive() {
        assert_eq!(
            mask_header_value("authorization", "Bearer token123"),
            "***MASKED***"
        );
        assert_eq!(
            mask_header_value("cookie", "session=abc123"),
            "***MASKED***"
        );
    }

    #[test]
    fn test_mask_header_value_non_sensitive() {
        assert_eq!(
            mask_header_value("content-type", "application/json"),
            "application/json"
        );
    }

    #[test]
    fn test_mask_sensitive_headers() {
        let headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("authorization".to_string(), "Bearer token".to_string()),
            ("accept".to_string(), "*/*".to_string()),
            ("cookie".to_string(), "session=123".to_string()),
        ];

        let masked = mask_sensitive_headers(&headers);

        assert_eq!(masked[0].0, "content-type");
        assert_eq!(masked[0].1, "application/json"); // Not masked
        assert_eq!(masked[1].0, "authorization");
        assert_eq!(masked[1].1, "***MASKED***"); // Masked
        assert_eq!(masked[2].0, "accept");
        assert_eq!(masked[2].1, "*/*"); // Not masked
        assert_eq!(masked[3].0, "cookie");
        assert_eq!(masked[3].1, "***MASKED***"); // Masked
    }

    #[test]
    fn test_truncate_body_small() {
        let body = b"small body";
        assert_eq!(truncate_body(body), b"small body");
    }

    #[test]
    fn test_truncate_body_exact_max() {
        let body = vec![0u8; MAX_BODY_PREVIEW];
        assert_eq!(truncate_body(&body).len(), MAX_BODY_PREVIEW);
    }

    #[test]
    fn test_truncate_body_large() {
        let body = vec![0u8; MAX_BODY_PREVIEW + 1000];
        let truncated = truncate_body(&body);
        assert_eq!(truncated.len(), MAX_BODY_PREVIEW);
    }
}
