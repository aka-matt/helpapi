//! Template substitution for transform values.
//!
//! Supports `{{path.param}}` syntax for path parameter substitution.
//! Missing variables are left as-is (no error).

use std::collections::HashMap;
use std::string::ToString;

/// Template substitution helper.
///
/// Replaces `{{path.param}}` placeholders with values from path_params.
#[derive(Debug, Clone)]
pub struct TemplateSubst {
    /// Original template string.
    original: String,
    /// Replacements to apply: (placeholder, value).
    replacements: Vec<(String, String)>,
}

impl TemplateSubst {
    /// Creates a new TemplateSubst from a template string.
    pub fn new(template: &str) -> Self {
        let mut replacements = Vec::new();
        let mut search_start = 0;
        let prefix = "{{path.";
        let suffix = "}}";

        while let Some(start) = template[search_start..].find(prefix) {
            let abs_start = search_start + start;
            let key_start = abs_start + prefix.len();
            if let Some(end) = template[key_start..].find(suffix) {
                let key_end = key_start + end;
                let key = template[key_start..key_end].to_string();
                // Skip empty keys - they're not valid variables
                if !key.is_empty() {
                    let placeholder = format!("{}{}{}", prefix, key, suffix);
                    replacements.push((placeholder, key));
                }
                search_start = key_end + suffix.len();
            } else {
                break;
            }
        }

        Self {
            original: template.to_string(),
            replacements,
        }
    }

    /// Performs template substitution using the given path parameters.
    ///
    /// Missing variables are left as-is (no error).
    pub fn substitute(&self, path_params: &HashMap<String, String>) -> String {
        let mut result = self.original.clone();
        for (placeholder, key) in &self.replacements {
            if let Some(value) = path_params.get(key) {
                result = result.replace(placeholder.as_str(), value);
            }
            // Missing keys are left as-is (no error per spec)
        }
        result
    }

    /// Returns true if this template has any path variable placeholders.
    pub fn has_variables(&self) -> bool {
        !self.replacements.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_no_variables() {
        let t = TemplateSubst::new("static-value");
        assert_eq!(t.substitute(&HashMap::new()), "static-value");
        assert!(!t.has_variables());
    }

    #[test]
    fn test_template_single_variable() {
        let t = TemplateSubst::new("user-{{path.id}}");
        let mut params = HashMap::new();
        params.insert("id".to_string(), "42".to_string());
        assert_eq!(t.substitute(&params), "user-42");
    }

    #[test]
    fn test_template_multiple_variables() {
        let t = TemplateSubst::new("/users/{{path.user_id}}/posts/{{path.post_id}}");
        let mut params = HashMap::new();
        params.insert("user_id".to_string(), "abc".to_string());
        params.insert("post_id".to_string(), "123".to_string());
        assert_eq!(t.substitute(&params), "/users/abc/posts/123");
    }

    #[test]
    fn test_template_missing_variable() {
        // Missing variables are left as-is
        let t = TemplateSubst::new("user-{{path.id}}");
        let params = HashMap::new();
        assert_eq!(t.substitute(&params), "user-{{path.id}}");
    }

    #[test]
    fn test_template_partial_missing() {
        let t = TemplateSubst::new("/users/{{path.user_id}}/posts/{{path.post_id}}");
        let mut params = HashMap::new();
        params.insert("user_id".to_string(), "abc".to_string());
        assert_eq!(t.substitute(&params), "/users/abc/posts/{{path.post_id}}");
    }

    #[test]
    fn test_template_multiple_same_variable() {
        let t = TemplateSubst::new("{{path.id}}-{{path.id}}-{{path.id}}");
        let mut params = HashMap::new();
        params.insert("id".to_string(), "42".to_string());
        assert_eq!(t.substitute(&params), "42-42-42");
    }

    #[test]
    fn test_template_no_braces() {
        let t = TemplateSubst::new("no template syntax here");
        assert!(!t.has_variables());
        assert_eq!(t.substitute(&HashMap::new()), "no template syntax here");
    }

    #[test]
    fn test_template_malformed_braces() {
        // Malformed braces are left as-is
        let t = TemplateSubst::new("{{path.id");
        assert!(!t.has_variables());
        assert_eq!(t.substitute(&HashMap::new()), "{{path.id");
    }

    #[test]
    fn test_template_empty_key() {
        let t = TemplateSubst::new("{{path.}}");
        assert!(!t.has_variables()); // Empty key is not a valid variable
        assert_eq!(t.substitute(&HashMap::new()), "{{path.}}");
    }
}
