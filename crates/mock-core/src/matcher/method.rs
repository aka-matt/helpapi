//! Method matcher — matches HTTP methods exactly or via wildcard.

use std::collections::HashMap;

use super::{Matcher, MatcherResult};
use crate::RequestData;

/// Matches HTTP request methods.
#[derive(Debug, Clone, PartialEq)]
pub struct MethodMatcher {
    /// The expected method (e.g., "GET", "POST"), or "*" for any method.
    method: String,
}

impl MethodMatcher {
    /// Creates a new method matcher.
    ///
    /// - Exact match (e.g., `"GET"`) scores 10.
    /// - Wildcard `"*"` scores 5.
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            method: method.into(),
        }
    }

    fn score(&self) -> u32 {
        if self.method == "*" { 5 } else { 10 }
    }
}

impl Matcher for MethodMatcher {
    fn match_request(&self, request: &RequestData) -> MatcherResult {
        if self.method == "*" || request.method.eq_ignore_ascii_case(&self.method) {
            MatcherResult::match_with(HashMap::new(), self.score())
        } else {
            MatcherResult::no_match()
        }
    }

    fn name(&self) -> &str {
        "MethodMatcher"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(method: &str) -> RequestData {
        RequestData::new(method, "/")
    }

    #[test]
    fn test_exact_method_match() {
        let matcher = MethodMatcher::new("GET");
        let result = matcher.match_request(&make_request("GET"));
        assert!(result.matches);
        assert_eq!(result.score, 10);
        assert!(result.path_params.is_empty());
    }

    #[test]
    fn test_exact_method_match_lowercase() {
        let matcher = MethodMatcher::new("get");
        let result = matcher.match_request(&make_request("GET"));
        assert!(result.matches);
        assert_eq!(result.score, 10);
    }

    #[test]
    fn test_exact_method_match_uppercase() {
        let matcher = MethodMatcher::new("POST");
        let result = matcher.match_request(&make_request("post"));
        assert!(result.matches);
        assert_eq!(result.score, 10);
    }

    #[test]
    fn test_method_mismatch() {
        let matcher = MethodMatcher::new("GET");
        let result = matcher.match_request(&make_request("POST"));
        assert!(!result.matches);
        assert_eq!(result.score, 0);
    }

    #[test]
    fn test_wildcard_match_any() {
        let matcher = MethodMatcher::new("*");
        let cases = ["GET", "POST", "PUT", "DELETE", "PATCH", "OPTIONS", "HEAD"];
        for method in cases {
            let result = matcher.match_request(&make_request(method));
            assert!(result.matches, "wildcard should match {method}");
            assert_eq!(result.score, 5);
        }
    }

    #[test]
    fn test_wildcard_has_lower_score_than_exact() {
        let wildcard = MethodMatcher::new("*");
        let exact = MethodMatcher::new("GET");
        let req = make_request("GET");
        assert!(wildcard.match_request(&req).score < exact.match_request(&req).score);
    }

    #[test]
    fn test_name() {
        let matcher = MethodMatcher::new("GET");
        assert_eq!(matcher.name(), "MethodMatcher");
    }
}
