//! Header matcher — matches HTTP headers with case-insensitive key comparison.
//!
//! Supports exact value matching and existence checks (value `""` means "header must exist").

use std::collections::HashMap;

use super::{Matcher, MatcherResult};
use crate::RequestData;

/// Matches HTTP request headers.
///
/// Headers are compared case-insensitively. A header with an empty expected value
/// (`""`) acts as an existence check — the header must be present (with any value).
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderMatcher {
    /// Headers to match: key (lowercase) -> expected value (empty = existence only).
    headers: HashMap<String, String>,
}

impl HeaderMatcher {
    /// Creates a new header matcher from a `HashMap` of header key-value pairs.
    ///
    /// - Non-empty value: exact match (case-insensitive on both key and value).
    /// - Empty value `""`: header must exist (any value is acceptable).
    ///
    /// # Score
    ///
    /// 10 points per matched header. Existence checks score the same as exact matches.
    pub fn new(headers: HashMap<String, String>) -> Self {
        Self { headers }
    }

    /// Creates a new header matcher from an iterator of (key, value) pairs.
    pub fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        Self::new(iter.into_iter().collect())
    }
}

impl Matcher for HeaderMatcher {
    fn match_request(&self, request: &RequestData) -> MatcherResult {
        let mut score = 0u32;

        for (expected_key, expected_val) in &self.headers {
            // Find the header by lowercase key comparison
            let found = request
                .headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(expected_key));

            match found {
                Some((_, actual_val)) => {
                    // If expected value is empty, just check existence
                    if expected_val.is_empty() || actual_val.eq_ignore_ascii_case(expected_val) {
                        score += 10;
                    } else {
                        return MatcherResult::no_match();
                    }
                }
                None => {
                    // Header not found at all — no match
                    return MatcherResult::no_match();
                }
            }
        }

        MatcherResult::match_with(HashMap::new(), score)
    }

    fn name(&self) -> &str {
        "HeaderMatcher"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(headers: Vec<(&str, &str)>) -> RequestData {
        RequestData::new("GET", "/").with_headers(
            headers
                .into_iter()
                .map(|(k, v)| (k.to_lowercase(), v.to_string()))
                .collect(),
        )
    }

    // --- Exact value match ---

    #[test]
    fn test_exact_match() {
        let matcher = HeaderMatcher::new(HashMap::from_iter([(
            "content-type".to_string(),
            "application/json".to_string(),
        )]));
        let result =
            matcher.match_request(&make_request(vec![("content-type", "application/json")]));
        assert!(result.matches);
        assert_eq!(result.score, 10);
    }

    #[test]
    fn test_exact_match_case_insensitive_value() {
        let matcher = HeaderMatcher::new(HashMap::from_iter([(
            "content-type".to_string(),
            "APPLICATION/JSON".to_string(),
        )]));
        let result =
            matcher.match_request(&make_request(vec![("content-type", "application/json")]));
        assert!(result.matches);
    }

    #[test]
    fn test_exact_match_case_insensitive_key() {
        let matcher = HeaderMatcher::new(HashMap::from_iter([(
            "Content-Type".to_string(),
            "application/json".to_string(),
        )]));
        let result =
            matcher.match_request(&make_request(vec![("content-type", "application/json")]));
        assert!(result.matches);
    }

    #[test]
    fn test_value_mismatch() {
        let matcher = HeaderMatcher::new(HashMap::from_iter([(
            "content-type".to_string(),
            "text/plain".to_string(),
        )]));
        let result =
            matcher.match_request(&make_request(vec![("content-type", "application/json")]));
        assert!(!result.matches);
    }

    #[test]
    fn test_header_not_found() {
        let matcher = HeaderMatcher::new(HashMap::from_iter([(
            "x-custom".to_string(),
            "value".to_string(),
        )]));
        let result = matcher.match_request(&make_request(vec![]));
        assert!(!result.matches);
    }

    // --- Existence check ---

    #[test]
    fn test_existence_check() {
        let matcher = HeaderMatcher::new(HashMap::from_iter([(
            "authorization".to_string(),
            "".to_string(),
        )]));
        let result =
            matcher.match_request(&make_request(vec![("authorization", "Bearer token123")]));
        assert!(result.matches);
        assert_eq!(result.score, 10);
    }

    #[test]
    fn test_existence_check_missing() {
        let matcher = HeaderMatcher::new(HashMap::from_iter([(
            "x-required".to_string(),
            "".to_string(),
        )]));
        let result = matcher.match_request(&make_request(vec![]));
        assert!(!result.matches);
    }

    // --- Multiple headers ---

    #[test]
    fn test_multiple_headers_all_match() {
        let matcher = HeaderMatcher::new(HashMap::from_iter([
            ("content-type".to_string(), "application/json".to_string()),
            ("accept".to_string(), "application/json".to_string()),
        ]));
        let result = matcher.match_request(&make_request(vec![
            ("content-type", "application/json"),
            ("accept", "application/json"),
        ]));
        assert!(result.matches);
        assert_eq!(result.score, 20); // 2 headers × 10
    }

    #[test]
    fn test_multiple_headers_one_fails() {
        let matcher = HeaderMatcher::new(HashMap::from_iter([
            ("content-type".to_string(), "application/json".to_string()),
            ("accept".to_string(), "application/json".to_string()),
        ]));
        let result = matcher.match_request(&make_request(vec![
            ("content-type", "application/json"),
            ("accept", "text/html"),
        ]));
        assert!(!result.matches);
    }

    #[test]
    fn test_partial_headers_in_request() {
        // Request has more headers than matcher cares about — that's fine
        let matcher = HeaderMatcher::new(HashMap::from_iter([(
            "content-type".to_string(),
            "application/json".to_string(),
        )]));
        let result = matcher.match_request(&make_request(vec![
            ("content-type", "application/json"),
            ("x-extra", "should-be-ignored"),
        ]));
        assert!(result.matches);
    }

    // --- Empty matcher ---

    #[test]
    fn test_empty_headers_match_anything() {
        let matcher = HeaderMatcher::new(HashMap::new());
        let result = matcher.match_request(&make_request(vec![("anything", "anything")]));
        assert!(result.matches);
        assert_eq!(result.score, 0);
    }

    // --- Score ---

    #[test]
    fn test_score_per_header() {
        let matcher = HeaderMatcher::new(HashMap::from_iter([
            ("a".to_string(), "1".to_string()),
            ("b".to_string(), "2".to_string()),
            ("c".to_string(), "3".to_string()),
        ]));
        let result = matcher.match_request(&make_request(vec![("a", "1"), ("b", "2"), ("c", "3")]));
        assert_eq!(result.score, 30);
    }

    #[test]
    fn test_name() {
        let matcher = HeaderMatcher::new(HashMap::new());
        assert_eq!(matcher.name(), "HeaderMatcher");
    }
}
