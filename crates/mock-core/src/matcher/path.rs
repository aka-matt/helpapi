//! Path matcher — matches URL paths with parameter extraction.
//!
//! Supports exact matching, `:param` segment capture (e.g., `/users/:id`),
//! and a trailing `*` wildcard.

use std::collections::HashMap;

use super::{Matcher, MatcherResult};
use crate::RequestData;

/// Matches HTTP request paths, extracting path parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct PathMatcher {
    /// The expected path pattern, e.g. `/users/:id` or `/users/*`.
    pattern: String,
    /// Pre-split pattern segments for efficient matching.
    segments: Vec<PathSegment>,
}

#[derive(Debug, Clone, PartialEq)]
enum PathSegment {
    /// A literal segment (e.g., "users").
    Literal(String),
    /// A parameter segment (e.g., "id" from ":id").
    Param(String),
    /// A trailing wildcard `*` — matches any remaining path.
    Wildcard,
}

impl PathMatcher {
    /// Creates a new path matcher from a pattern string.
    ///
    /// - `:param` segments are captured as path parameters.
    /// - A trailing `*` matches any suffix.
    ///
    /// # Score
    ///
    /// - 10 points per matched literal segment.
    /// - 15 points per matched parameter segment (10 + 5 bonus).
    /// - +20 bonus for highest specificity (most parameter segments).
    pub fn new(pattern: impl Into<String>) -> Self {
        let pattern = pattern.into();
        let segments = Self::parse_segments(&pattern);
        Self { pattern, segments }
    }

    fn parse_segments(pattern: &str) -> Vec<PathSegment> {
        let mut segments = Vec::new();
        let parts: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();

        // Handle the special case of "/" (single root segment, score 10)
        if parts.is_empty() && pattern == "/" {
            return segments;
        }

        for part in parts {
            if part == "*" {
                segments.push(PathSegment::Wildcard);
            } else if let Some(s) = part.strip_prefix(':') {
                if !s.is_empty() {
                    segments.push(PathSegment::Param(s.to_string()));
                }
            } else {
                segments.push(PathSegment::Literal(part.to_string()));
            }
        }

        segments
    }

    fn score_for_segments(&self, segments: &[PathSegment]) -> u32 {
        let mut score = 0u32;
        let mut param_count = 0usize;

        for seg in segments {
            match seg {
                PathSegment::Literal(_) => score += 10,
                PathSegment::Param(_) => {
                    param_count += 1;
                    score += 15; // 10 base + 5 param bonus
                }
                PathSegment::Wildcard => {
                    // Wildcard at end doesn't add to specificity score
                }
            }
        }

        // +20 for highest specificity (param segments contribute most)
        if param_count > 0 {
            score += 20;
        }

        score
    }
}

impl Matcher for PathMatcher {
    fn match_request(&self, request: &RequestData) -> MatcherResult {
        let path = &request.path;
        let path_parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        let pattern_parts = &self.segments;

        // Handle the special case of "/" root path matching
        if pattern_parts.is_empty() && (path == "/" || path_parts.is_empty()) {
            return MatcherResult::match_with(HashMap::new(), 10);
        }

        // Determine the non-wildcard segment count
        let has_wildcard = pattern_parts.last() == Some(&PathSegment::Wildcard);
        let non_wildcard_count = if has_wildcard {
            pattern_parts.len() - 1
        } else {
            pattern_parts.len()
        };

        // Length check: non-wildcard segments must match exactly;
        // with wildcard, path must have at least non_wildcard_count segments
        if !has_wildcard && path_parts.len() != non_wildcard_count {
            return MatcherResult::no_match();
        }
        if has_wildcard && path_parts.len() < non_wildcard_count {
            return MatcherResult::no_match();
        }

        let mut params = HashMap::new();

        for (i, pattern_seg) in pattern_parts.iter().enumerate() {
            if i >= non_wildcard_count {
                // This is past the last non-wildcard segment.
                // Any remaining path segments are consumed by wildcard.
                break;
            }

            if i >= path_parts.len() {
                return MatcherResult::no_match();
            }

            let path_seg = path_parts[i];

            match pattern_seg {
                PathSegment::Literal(lit) => {
                    if lit != path_seg {
                        return MatcherResult::no_match();
                    }
                }
                PathSegment::Param(name) => {
                    params.insert(name.clone(), path_seg.to_string());
                }
                PathSegment::Wildcard => {
                    // Wildcard consumes remaining path segments; nothing more to match.
                    break;
                }
            }
        }

        let score = self.score_for_segments(pattern_parts);
        MatcherResult::match_with(params, score)
    }

    fn name(&self) -> &str {
        "PathMatcher"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(path: &str) -> RequestData {
        RequestData::new("GET", path)
    }

    // --- Exact match ---

    #[test]
    fn test_exact_match() {
        let matcher = PathMatcher::new("/users");
        let result = matcher.match_request(&make_request("/users"));
        assert!(result.matches);
        assert_eq!(result.score, 10);
        assert!(result.path_params.is_empty());
    }

    #[test]
    fn test_exact_match_deep() {
        let matcher = PathMatcher::new("/api/v1/users");
        let result = matcher.match_request(&make_request("/api/v1/users"));
        assert!(result.matches);
        // 3 segments × 10 = 30 (api, v1, users)
        assert_eq!(result.score, 30);
        assert!(result.path_params.is_empty());
    }

    #[test]
    fn test_exact_match_no_leading_slash() {
        let matcher = PathMatcher::new("users");
        let result = matcher.match_request(&make_request("users"));
        assert!(result.matches);
    }

    #[test]
    fn test_exact_mismatch() {
        let matcher = PathMatcher::new("/users");
        let result = matcher.match_request(&make_request("/products"));
        assert!(!result.matches);
    }

    // --- Parameter extraction ---

    #[test]
    fn test_single_param() {
        let matcher = PathMatcher::new("/users/:id");
        let result = matcher.match_request(&make_request("/users/42"));
        assert!(result.matches);
        assert_eq!(result.path_params.get("id"), Some(&"42".to_string()));
        // 2 segments: literal(10) + param(15) + 20 specificity = 45
        assert_eq!(result.score, 45);
    }

    #[test]
    fn test_multiple_params() {
        let matcher = PathMatcher::new("/users/:user_id/posts/:post_id");
        let result = matcher.match_request(&make_request("/users/10/posts/20"));
        assert!(result.matches);
        assert_eq!(result.path_params.get("user_id"), Some(&"10".to_string()));
        assert_eq!(result.path_params.get("post_id"), Some(&"20".to_string()));
        // 4 segments: 2×literal(20) + 2×param(30) + 20 specificity = 70
        assert_eq!(result.score, 70);
    }

    #[test]
    fn test_param_with_exact_segments() {
        let matcher = PathMatcher::new("/api/users/:id/comments");
        let result = matcher.match_request(&make_request("/api/users/99/comments"));
        assert!(result.matches);
        assert_eq!(result.path_params.get("id"), Some(&"99".to_string()));
    }

    #[test]
    fn test_mixed_params_mismatch() {
        let matcher = PathMatcher::new("/users/:id/posts");
        let result = matcher.match_request(&make_request("/users/42/comments"));
        assert!(!result.matches);
    }

    #[test]
    fn test_path_param_not_found_when_expected() {
        let matcher = PathMatcher::new("/users/:id");
        let result = matcher.match_request(&make_request("/users"));
        assert!(!result.matches);
    }

    // --- Wildcard ---

    #[test]
    fn test_wildcard_suffix() {
        let matcher = PathMatcher::new("/static/*");
        let result = matcher.match_request(&make_request("/static/js/app.js"));
        assert!(result.matches);
        assert!(result.path_params.is_empty());
    }

    #[test]
    fn test_wildcard_exact_suffix() {
        let matcher = PathMatcher::new("/static/*");
        let result = matcher.match_request(&make_request("/static"));
        assert!(result.matches);
    }

    #[test]
    fn test_wildcard_at_root() {
        let matcher = PathMatcher::new("/*");
        let result = matcher.match_request(&make_request("/anything/goes/here"));
        assert!(result.matches);
    }

    #[test]
    fn test_wildcard_mismatch() {
        let matcher = PathMatcher::new("/api/*");
        let result = matcher.match_request(&make_request("/other/path"));
        assert!(!result.matches);
    }

    // --- Score ordering ---

    #[test]
    fn test_more_params_higher_score() {
        let single = PathMatcher::new("/users/:id");
        let multi = PathMatcher::new("/users/:user_id/posts/:post_id");
        let req = make_request("/users/1/posts/2");
        let single_score = single.match_request(&req).score;
        let multi_score = multi.match_request(&req).score;
        assert!(
            multi_score > single_score,
            "more params should score higher"
        );
    }

    #[test]
    fn test_exact_segments_higher_than_wildcard() {
        let wildcard = PathMatcher::new("/api/*");
        let exact = PathMatcher::new("/api/users");
        let req = make_request("/api/users");
        let wildcard_score = wildcard.match_request(&req).score;
        let exact_score = exact.match_request(&req).score;
        // Wildcard gives 0 per segment, exact gives 10 per segment
        assert!(
            exact_score > wildcard_score,
            "exact should score higher than wildcard"
        );
    }

    // --- Edge cases ---

    #[test]
    fn test_empty_path() {
        let matcher = PathMatcher::new("/");
        let result = matcher.match_request(&make_request("/"));
        assert!(result.matches);
        assert_eq!(result.score, 10);
    }

    #[test]
    fn test_root_literal() {
        let matcher = PathMatcher::new("/");
        let result = matcher.match_request(&make_request("/"));
        assert!(result.matches);
    }

    #[test]
    fn test_double_slash_normalized() {
        let matcher = PathMatcher::new("/users");
        let result = matcher.match_request(&make_request("//users"));
        assert!(result.matches);
    }

    #[test]
    fn test_trailing_slash_in_pattern_and_request() {
        let matcher = PathMatcher::new("/users/");
        let result = matcher.match_request(&make_request("/users/"));
        assert!(result.matches);
    }

    #[test]
    fn test_name() {
        let matcher = PathMatcher::new("/users/:id");
        assert_eq!(matcher.name(), "PathMatcher");
    }
}
