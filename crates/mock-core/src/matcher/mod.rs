//! Matcher system for request matching against rules.
//!
//! The matcher system evaluates incoming [`RequestData`] against route rules
//! and produces a [`MatcherResult`] indicating whether the request matches,
//! any extracted path parameters, and a specificity score.

mod composite;
mod header;
mod method;
mod path;
mod query;

pub use composite::CompositeMatcher;
pub use header::HeaderMatcher;
pub use method::MethodMatcher;
pub use path::PathMatcher;
pub use query::QueryMatcher;

use std::collections::HashMap;

/// The result of a matcher evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct MatcherResult {
    /// Whether the request matched the rule.
    pub matches: bool,
    /// Path parameters extracted from the request URL.
    pub path_params: HashMap<String, String>,
    /// Specificity score — higher means more specific.
    pub score: u32,
}

impl MatcherResult {
    /// Creates a non-matching result with no path parameters.
    pub fn no_match() -> Self {
        Self {
            matches: false,
            path_params: HashMap::new(),
            score: 0,
        }
    }

    /// Creates a matching result with the given path parameters and score.
    pub fn match_with(path_params: HashMap<String, String>, score: u32) -> Self {
        Self {
            matches: true,
            path_params,
            score,
        }
    }
}

/// Trait for matchers that evaluate incoming requests.
///
/// Implementors must be thread-safe (`Send + Sync`) since matchers may be
/// shared across async tasks.
pub trait Matcher: Send + Sync {
    /// Evaluates whether the given request matches this matcher's rule.
    fn match_request(&self, request: &crate::RequestData) -> MatcherResult;

    /// Returns a human-readable name for this matcher, used in diagnostics.
    fn name(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matcher_result_no_match() {
        let result = MatcherResult::no_match();
        assert!(!result.matches);
        assert!(result.path_params.is_empty());
        assert_eq!(result.score, 0);
    }

    #[test]
    fn test_matcher_result_match_with() {
        let mut params = HashMap::new();
        params.insert("id".to_string(), "42".to_string());
        let result = MatcherResult::match_with(params, 100);
        assert!(result.matches);
        assert_eq!(result.path_params.get("id"), Some(&"42".to_string()));
        assert_eq!(result.score, 100);
    }
}
