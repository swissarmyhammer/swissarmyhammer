use regex::Regex;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FilterError {
    #[error("Invalid regex pattern: {0}")]
    InvalidRegex(#[from] regex::Error),
}

#[derive(Clone)]
pub struct ToolFilter {
    allowed_patterns: Vec<Regex>,
    denied_patterns: Vec<Regex>,
}

impl ToolFilter {
    pub fn new(allowed: Vec<String>, denied: Vec<String>) -> Result<Self, FilterError> {
        let allowed_patterns = allowed
            .iter()
            .map(|s| Regex::new(s))
            .collect::<Result<Vec<_>, _>>()?;

        let denied_patterns = denied
            .iter()
            .map(|s| Regex::new(s))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            allowed_patterns,
            denied_patterns,
        })
    }

    pub fn is_allowed(&self, tool_name: &str) -> bool {
        // Allow patterns take precedence - if any allow pattern matches, allow immediately
        for pattern in &self.allowed_patterns {
            if pattern.is_match(tool_name) {
                tracing::debug!(
                    tool_name = %tool_name,
                    pattern = %pattern.as_str(),
                    result = "allowed",
                    "Tool matched allow pattern"
                );
                return true;
            }
        }

        // Check deny patterns
        for pattern in &self.denied_patterns {
            if pattern.is_match(tool_name) {
                tracing::debug!(
                    tool_name = %tool_name,
                    pattern = %pattern.as_str(),
                    result = "denied",
                    "Tool matched deny pattern"
                );
                return false;
            }
        }

        // No patterns matched
        // If no allow patterns specified, allow by default
        // If allow patterns specified but didn't match, deny (whitelist mode)
        let result = self.allowed_patterns.is_empty();
        tracing::debug!(
            tool_name = %tool_name,
            result = if result { "allowed" } else { "denied" },
            reason = if self.allowed_patterns.is_empty() { "no_allow_patterns" } else { "not_in_allowlist" },
            "Tool filter default evaluation"
        );
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allow_precedence_over_deny() {
        let filter = ToolFilter::new(
            vec!["^files$".to_string()],
            vec!["^files$".to_string()],
        )
        .unwrap();

        // files matches both allow and deny, but allow wins
        assert!(filter.is_allowed("files"));
    }

    #[test]
    fn test_empty_allow_list_allows_all() {
        let filter = ToolFilter::new(
            vec![], // Empty allow list
            vec!["^shell_.*".to_string()],
        )
        .unwrap();

        assert!(filter.is_allowed("files"));
        assert!(filter.is_allowed("web"));
        assert!(!filter.is_allowed("shell_execute")); // Denied
    }

    #[test]
    fn test_specific_patterns() {
        let filter = ToolFilter::new(
            vec!["^files$".to_string(), "^treesitter_search$".to_string()],
            vec![],
        )
        .unwrap();

        assert!(filter.is_allowed("files"));
        assert!(filter.is_allowed("treesitter_search"));
        assert!(!filter.is_allowed("kanban"));
        assert!(!filter.is_allowed("web"));
        assert!(!filter.is_allowed("shell_execute"));
    }

    #[test]
    fn test_invalid_regex() {
        let result = ToolFilter::new(vec!["[invalid".to_string()], vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_complex_patterns() {
        let filter = ToolFilter::new(
            vec!["^(files|web)$".to_string()], // Allow files and web
            vec!["^shell_.*".to_string()],      // Deny shell tools
        )
        .unwrap();

        assert!(filter.is_allowed("files"));
        assert!(filter.is_allowed("web"));
        // Allow wins over deny, so these are allowed
        assert!(!filter.is_allowed("kanban")); // Not in allow list
        assert!(!filter.is_allowed("shell_execute")); // Not in allow list
    }

    #[test]
    fn test_no_patterns_allows_all() {
        let filter = ToolFilter::new(vec![], vec![]).unwrap();

        assert!(filter.is_allowed("files"));
        assert!(filter.is_allowed("shell_execute"));
        assert!(filter.is_allowed("anything"));
    }

    #[test]
    fn test_whitelist_mode() {
        let filter = ToolFilter::new(
            vec!["^files$".to_string()],
            vec![], // No deny patterns
        )
        .unwrap();

        assert!(filter.is_allowed("files")); // In allow list
        assert!(!filter.is_allowed("kanban")); // Not in allow list = denied
        assert!(!filter.is_allowed("shell_execute")); // Not in allow list = denied
    }
}
