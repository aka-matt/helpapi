//! Composite matcher — combines multiple matchers with AND logic.

use std::collections::HashMap;

use super::{Matcher, MatcherResult};
use crate::RequestData;

/// Combines multiple matchers, all of which must match for the composite to match.
///
/// Uses AND logic: if any sub-matcher fails, the composite fails.
/// Path parameters from all sub-matchers are merged into a single map.
pub struct CompositeMatcher {
    matchers: Vec<Box<dyn Matcher>>,
}

impl std::fmt::Debug for CompositeMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeMatcher")
            .field("matchers", &self.matchers.len())
            .finish()
    }
}

impl CompositeMatcher {
    /// Creates a new empty composite matcher.
    ///
    /// An empty composite always matches (vacuously true).
    pub fn new() -> Self {
        Self {
            matchers: Vec::new(),
        }
    }

    /// Creates a composite matcher from a list of matchers.
    pub fn from_matchers(matchers: Vec<Box<dyn Matcher>>) -> Self {
        Self { matchers }
    }

    /// Adds a matcher to the composite.
    pub fn add_matcher<M: Matcher + 'static>(mut self, matcher: M) -> Self {
        self.matchers.push(Box::new(matcher));
        self
    }

    /// Returns the number of sub-matchers.
    pub fn len(&self) -> usize {
        self.matchers.len()
    }

    /// Returns true if the composite has no sub-matchers.
    pub fn is_empty(&self) -> bool {
        self.matchers.is_empty()
    }
}

impl Default for CompositeMatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Matcher for CompositeMatcher {
    fn match_request(&self, request: &RequestData) -> MatcherResult {
        if self.matchers.is_empty() {
            return MatcherResult::match_with(HashMap::new(), 0);
        }

        let mut all_params = HashMap::new();
        let mut total_score = 0u32;

        for matcher in &self.matchers {
            let result = matcher.match_request(request);
            if !result.matches {
                return MatcherResult::no_match();
            }
            total_score += result.score;
            all_params.extend(result.path_params);
        }

        MatcherResult::match_with(all_params, total_score)
    }

    fn name(&self) -> &str {
        "CompositeMatcher"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::{HeaderMatcher, MethodMatcher, PathMatcher, QueryMatcher};

    fn make_request(
        method: &str,
        path: &str,
        headers: Vec<(&str, &str)>,
        query: Vec<(&str, &str)>,
    ) -> RequestData {
        RequestData::new(method, path)
            .with_headers(
                headers
                    .into_iter()
                    .map(|(k, v)| (k.to_lowercase(), v.to_string()))
                    .collect(),
            )
            .with_query(
                query
                    .into_iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
            )
    }

    #[test]
    fn test_empty_composite_always_matches() {
        let composite = CompositeMatcher::new();
        let req = make_request("GET", "/anything", vec![], vec![]);
        let result = composite.match_request(&req);
        assert!(result.matches);
        assert_eq!(result.score, 0);
    }

    #[test]
    fn test_all_match() {
        let composite = CompositeMatcher::new()
            .add_matcher(MethodMatcher::new("GET"))
            .add_matcher(PathMatcher::new("/users/:id"))
            .add_matcher(HeaderMatcher::new(HashMap::from_iter(vec![(
                "accept".to_string(),
                "application/json".to_string(),
            )])))
            .add_matcher(QueryMatcher::new(HashMap::from_iter(vec![(
                "expand".to_string(),
                "".to_string(),
            )])));

        let req = make_request(
            "GET",
            "/users/42",
            vec![("accept", "application/json")],
            vec![("expand", "true")],
        );
        let result = composite.match_request(&req);
        assert!(result.matches);
        assert_eq!(result.path_params.get("id"), Some(&"42".to_string()));
    }

    #[test]
    fn test_one_fails_causes_no_match() {
        let composite = CompositeMatcher::new()
            .add_matcher(MethodMatcher::new("GET"))
            .add_matcher(PathMatcher::new("/users/:id"));

        let req = make_request("POST", "/users/42", vec![], vec![]);
        let result = composite.match_request(&req);
        assert!(!result.matches);
    }

    #[test]
    fn test_params_merged() {
        // CompositeMatcher with a PathMatcher and a HeaderMatcher.
        // Both must match for the composite to match.
        let composite = CompositeMatcher::new()
            .add_matcher(PathMatcher::new("/users/:user_id/posts/:post_id"))
            .add_matcher(HeaderMatcher::new(HashMap::from_iter(vec![(
                "x-authenticated".to_string(),
                "true".to_string(),
            )])));

        let req = make_request(
            "GET",
            "/users/10/posts/20",
            vec![("x-authenticated", "true")],
            vec![],
        );
        let result = composite.match_request(&req);
        assert!(result.matches);
        assert_eq!(result.path_params.get("user_id"), Some(&"10".to_string()));
        assert_eq!(result.path_params.get("post_id"), Some(&"20".to_string()));
    }

    #[test]
    fn test_score_summed() {
        let composite = CompositeMatcher::new()
            .add_matcher(MethodMatcher::new("GET"))
            .add_matcher(PathMatcher::new("/users"));

        let req = make_request("GET", "/users", vec![], vec![]);
        let result = composite.match_request(&req);
        assert!(result.matches);
        // GET exact = 10, /users exact = 10
        assert_eq!(result.score, 20);
    }

    #[test]
    fn test_single_matcher_composite() {
        let composite = CompositeMatcher::new().add_matcher(MethodMatcher::new("POST"));

        let req = make_request("POST", "/", vec![], vec![]);
        let result = composite.match_request(&req);
        assert!(result.matches);
    }

    #[test]
    fn test_name() {
        let composite = CompositeMatcher::new();
        assert_eq!(composite.name(), "CompositeMatcher");
    }

    #[test]
    fn test_len_and_is_empty() {
        let empty: CompositeMatcher = CompositeMatcher::new();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);

        let composite = CompositeMatcher::new()
            .add_matcher(MethodMatcher::new("GET"))
            .add_matcher(PathMatcher::new("/"));
        assert!(!composite.is_empty());
        assert_eq!(composite.len(), 2);
    }
}
