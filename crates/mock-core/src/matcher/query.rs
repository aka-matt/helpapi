//! Query matcher — matches URL query parameters.
//!
//! Supports exact key-value matching and key-only existence checks.

use std::collections::HashMap;

use super::{Matcher, MatcherResult};
use crate::RequestData;

/// Matches URL query parameters.
///
/// An expected value of `""` acts as an existence check — the query parameter
/// must be present with any value.
#[derive(Debug, Clone, PartialEq)]
pub struct QueryMatcher {
    /// Query params to match: key -> expected value (empty = existence only).
    query: HashMap<String, String>,
}

impl QueryMatcher {
    /// Creates a new query matcher from a `HashMap` of query key-value pairs.
    ///
    /// - Non-empty value: exact match.
    /// - Empty value `""`: parameter must exist (any value is acceptable).
    ///
    /// # Score
    ///
    /// 10 points per matched query parameter.
    pub fn new(query: HashMap<String, String>) -> Self {
        Self { query }
    }

    /// Creates a new query matcher from an iterator of (key, value) pairs.
    pub fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (String, String)>,
    {
        Self::new(iter.into_iter().collect())
    }
}

impl Matcher for QueryMatcher {
    fn match_request(&self, request: &RequestData) -> MatcherResult {
        // If no query params are required, it's an automatic match with score 0.
        if self.query.is_empty() {
            return MatcherResult::match_with(HashMap::new(), 0);
        }

        let mut score = 0u32;

        for (expected_key, expected_val) in &self.query {
            let found = request.query.iter().find(|(k, _)| k == expected_key);

            match found {
                Some((_, actual_val)) => {
                    // Empty expected value = existence check
                    if expected_val.is_empty() || actual_val == expected_val {
                        score += 10;
                    } else {
                        return MatcherResult::no_match();
                    }
                }
                None => {
                    return MatcherResult::no_match();
                }
            }
        }

        MatcherResult::match_with(HashMap::new(), score)
    }

    fn name(&self) -> &str {
        "QueryMatcher"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(query: Vec<(&str, &str)>) -> RequestData {
        RequestData::new("GET", "/").with_query(
            query
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    // --- Exact key-value match ---

    #[test]
    fn test_exact_match() {
        let matcher = QueryMatcher::new(HashMap::from_iter(vec![(
            "status".to_string(),
            "active".to_string(),
        )]));
        let result = matcher.match_request(&make_request(vec![("status", "active")]));
        assert!(result.matches);
        assert_eq!(result.score, 10);
    }

    #[test]
    fn test_exact_match_multiple() {
        let matcher = QueryMatcher::new(HashMap::from_iter(vec![
            ("status".to_string(), "active".to_string()),
            ("sort".to_string(), "name".to_string()),
        ]));
        let result =
            matcher.match_request(&make_request(vec![("status", "active"), ("sort", "name")]));
        assert!(result.matches);
        assert_eq!(result.score, 20);
    }

    #[test]
    fn test_value_mismatch() {
        let matcher = QueryMatcher::new(HashMap::from_iter(vec![(
            "status".to_string(),
            "active".to_string(),
        )]));
        let result = matcher.match_request(&make_request(vec![("status", "inactive")]));
        assert!(!result.matches);
    }

    #[test]
    fn test_key_not_found() {
        let matcher = QueryMatcher::new(HashMap::from_iter(vec![(
            "page".to_string(),
            "1".to_string(),
        )]));
        let result = matcher.match_request(&make_request(vec![]));
        assert!(!result.matches);
    }

    // --- Existence check ---

    #[test]
    fn test_existence_check() {
        let matcher = QueryMatcher::new(HashMap::from_iter(vec![(
            "token".to_string(),
            "".to_string(),
        )]));
        let result = matcher.match_request(&make_request(vec![("token", "abc123")]));
        assert!(result.matches);
        assert_eq!(result.score, 10);
    }

    #[test]
    fn test_existence_check_missing() {
        let matcher = QueryMatcher::new(HashMap::from_iter(vec![(
            "required".to_string(),
            "".to_string(),
        )]));
        let result = matcher.match_request(&make_request(vec![]));
        assert!(!result.matches);
    }

    // --- Extra params in request (not required to match) ---

    #[test]
    fn test_extra_params_in_request_ignored() {
        let matcher = QueryMatcher::new(HashMap::from_iter(vec![(
            "page".to_string(),
            "1".to_string(),
        )]));
        let result =
            matcher.match_request(&make_request(vec![("page", "1"), ("extra", "ignored")]));
        assert!(result.matches);
    }

    // --- Empty matcher ---

    #[test]
    fn test_empty_query_matches_anything() {
        let matcher = QueryMatcher::new(HashMap::new());
        let result = matcher.match_request(&make_request(vec![("any", "value")]));
        assert!(result.matches);
        assert_eq!(result.score, 0);
    }

    #[test]
    fn test_empty_query_on_request_with_no_params() {
        let matcher = QueryMatcher::new(HashMap::new());
        let result = matcher.match_request(&make_request(vec![]));
        assert!(result.matches);
    }

    // --- Score ---

    #[test]
    fn test_score_per_param() {
        let matcher = QueryMatcher::new(HashMap::from_iter(vec![
            ("a".to_string(), "1".to_string()),
            ("b".to_string(), "2".to_string()),
            ("c".to_string(), "3".to_string()),
        ]));
        let result = matcher.match_request(&make_request(vec![("a", "1"), ("b", "2"), ("c", "3")]));
        assert_eq!(result.score, 30);
    }

    #[test]
    fn test_name() {
        let matcher = QueryMatcher::new(HashMap::new());
        assert_eq!(matcher.name(), "QueryMatcher");
    }
}
