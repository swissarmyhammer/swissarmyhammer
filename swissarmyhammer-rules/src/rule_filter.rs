//! Rule filtering functionality
//!
//! This module provides filtering capabilities to select rules based
//! on various criteria like name patterns, tags, categories, severity, and sources.

use crate::rules::Rule;
use crate::{RuleSource, Severity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Filter criteria for selecting rules
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleFilter {
    /// Name pattern to match (supports glob patterns)
    pub name_pattern: Option<String>,
    /// Category to filter by
    pub category: Option<String>,
    /// Tags that rules must have (any match)
    pub tags: Vec<String>,
    /// Sources to include
    pub sources: Vec<RuleSource>,
    /// Severity levels to include
    pub severities: Vec<Severity>,
    /// Whether to include partial templates
    pub include_partials: bool,
}

impl RuleFilter {
    /// Create a new empty filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a filter for a specific name pattern
    pub fn by_name_pattern(pattern: impl Into<String>) -> Self {
        Self {
            name_pattern: Some(pattern.into()),
            ..Self::default()
        }
    }

    /// Create a filter for a specific category
    pub fn by_category(category: impl Into<String>) -> Self {
        Self {
            category: Some(category.into()),
            ..Self::default()
        }
    }

    /// Create a filter for specific tags
    pub fn by_tags(tags: Vec<String>) -> Self {
        Self {
            tags,
            ..Self::default()
        }
    }

    /// Create a filter for specific sources
    pub fn by_sources(sources: Vec<RuleSource>) -> Self {
        Self {
            sources,
            ..Self::default()
        }
    }

    /// Create a filter for specific severity levels
    pub fn by_severities(severities: Vec<Severity>) -> Self {
        Self {
            severities,
            ..Self::default()
        }
    }

    /// Set whether to include partial templates
    pub fn with_partials(mut self, include_partials: bool) -> Self {
        self.include_partials = include_partials;
        self
    }

    /// Set the name pattern
    pub fn with_name_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.name_pattern = Some(pattern.into());
        self
    }

    /// Set the category filter
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Add tags to filter by
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Set the source filter
    pub fn with_sources(mut self, sources: Vec<RuleSource>) -> Self {
        self.sources = sources;
        self
    }

    /// Set the severity filter
    pub fn with_severities(mut self, severities: Vec<Severity>) -> Self {
        self.severities = severities;
        self
    }

    /// Apply the filter to a list of rules
    pub fn apply(&self, rules: Vec<&Rule>, sources: &HashMap<String, RuleSource>) -> Vec<Rule> {
        rules
            .into_iter()
            .filter(|rule| self.matches(rule, sources))
            .cloned()
            .collect()
    }

    /// Check if a rule matches the filter criteria
    pub fn matches(&self, rule: &Rule, sources: &HashMap<String, RuleSource>) -> bool {
        // Check name pattern
        if let Some(pattern) = &self.name_pattern {
            if !self.matches_pattern(&rule.name, pattern) {
                return false;
            }
        }

        // Check category
        if let Some(category) = &self.category {
            match &rule.category {
                Some(rule_category) if rule_category == category => {}
                _ => return false,
            }
        }

        // Check tags (any match)
        if !self.tags.is_empty() {
            let has_matching_tag = self
                .tags
                .iter()
                .any(|filter_tag| rule.tags.iter().any(|rule_tag| rule_tag == filter_tag));
            if !has_matching_tag {
                return false;
            }
        }

        // Check sources
        if !self.sources.is_empty() {
            if let Some(rule_source) = sources.get(&rule.name) {
                if !self.sources.contains(rule_source) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check severity
        if !self.severities.is_empty() && !self.severities.contains(&rule.severity) {
            return false;
        }

        // Check if it's a partial template
        if !self.include_partials && rule.is_partial() {
            return false;
        }

        true
    }

    /// Check if a string matches a pattern (supports basic glob patterns)
    fn matches_pattern(&self, text: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.contains('*') || pattern.contains('?') {
            if let Ok(glob) = glob::Pattern::new(pattern) {
                return glob.matches(text);
            }
        }

        text.contains(pattern)
    }

    /// Check if the filter is empty (matches everything)
    pub fn is_empty(&self) -> bool {
        self.name_pattern.is_none()
            && self.category.is_none()
            && self.tags.is_empty()
            && self.sources.is_empty()
            && self.severities.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_rule(
        name: &str,
        category: Option<&str>,
        tags: Vec<&str>,
        severity: Severity,
    ) -> Rule {
        let mut rule = Rule::new(name.to_string(), "Template content".to_string(), severity);
        if let Some(cat) = category {
            rule.category = Some(cat.to_string());
        }
        rule.tags = tags.iter().map(|s| s.to_string()).collect();
        rule
    }

    #[test]
    fn test_empty_filter() {
        let filter = RuleFilter::new();
        let rule = create_test_rule("test", None, vec![], Severity::Error);
        let sources = HashMap::new();

        assert!(filter.matches(&rule, &sources));
        assert!(filter.is_empty());
    }

    #[test]
    fn test_name_pattern_filter() {
        let filter = RuleFilter::by_name_pattern("test*");
        let sources = HashMap::new();

        let matching_rule = create_test_rule("test_rule", None, vec![], Severity::Error);
        let non_matching_rule = create_test_rule("other_rule", None, vec![], Severity::Error);

        assert!(filter.matches(&matching_rule, &sources));
        assert!(!filter.matches(&non_matching_rule, &sources));
    }

    #[test]
    fn test_category_filter() {
        let filter = RuleFilter::by_category("security");
        let sources = HashMap::new();

        let matching_rule = create_test_rule("test", Some("security"), vec![], Severity::Error);
        let non_matching_rule = create_test_rule("test", Some("other"), vec![], Severity::Error);
        let no_category_rule = create_test_rule("test", None, vec![], Severity::Error);

        assert!(filter.matches(&matching_rule, &sources));
        assert!(!filter.matches(&non_matching_rule, &sources));
        assert!(!filter.matches(&no_category_rule, &sources));
    }

    #[test]
    fn test_tags_filter() {
        let filter = RuleFilter::by_tags(vec!["security".to_string(), "critical".to_string()]);
        let sources = HashMap::new();

        let matching_rule =
            create_test_rule("test", None, vec!["security", "check"], Severity::Error);
        let non_matching_rule =
            create_test_rule("test", None, vec!["other", "check"], Severity::Error);
        let no_tags_rule = create_test_rule("test", None, vec![], Severity::Error);

        assert!(filter.matches(&matching_rule, &sources));
        assert!(!filter.matches(&non_matching_rule, &sources));
        assert!(!filter.matches(&no_tags_rule, &sources));
    }

    #[test]
    fn test_source_filter() {
        let filter = RuleFilter::by_sources(vec![RuleSource::Builtin]);
        let mut sources = HashMap::new();
        sources.insert("builtin_rule".to_string(), RuleSource::Builtin);
        sources.insert("user_rule".to_string(), RuleSource::User);

        let builtin_rule = create_test_rule("builtin_rule", None, vec![], Severity::Error);
        let user_rule = create_test_rule("user_rule", None, vec![], Severity::Error);
        let unknown_rule = create_test_rule("unknown_rule", None, vec![], Severity::Error);

        assert!(filter.matches(&builtin_rule, &sources));
        assert!(!filter.matches(&user_rule, &sources));
        assert!(!filter.matches(&unknown_rule, &sources));
    }

    #[test]
    fn test_severity_filter() {
        let filter = RuleFilter::by_severities(vec![Severity::Error, Severity::Warning]);
        let sources = HashMap::new();

        let error_rule = create_test_rule("test1", None, vec![], Severity::Error);
        let warning_rule = create_test_rule("test2", None, vec![], Severity::Warning);
        let info_rule = create_test_rule("test3", None, vec![], Severity::Info);

        assert!(filter.matches(&error_rule, &sources));
        assert!(filter.matches(&warning_rule, &sources));
        assert!(!filter.matches(&info_rule, &sources));
    }

    #[test]
    fn test_partial_detection() {
        let filter = RuleFilter::new().with_partials(false);
        let sources = HashMap::new();

        let partial_rule = Rule::new(
            "test".to_string(),
            "{% partial %}\nContent".to_string(),
            Severity::Info,
        );
        let regular_rule = create_test_rule("regular", None, vec![], Severity::Error);

        assert!(!filter.matches(&partial_rule, &sources));
        assert!(filter.matches(&regular_rule, &sources));
    }

    #[test]
    fn test_apply_filter() {
        let filter = RuleFilter::by_category("security");
        let sources = HashMap::new();

        let rule1 = create_test_rule("test1", Some("security"), vec![], Severity::Error);
        let rule2 = create_test_rule("test2", Some("other"), vec![], Severity::Error);
        let rule3 = create_test_rule("test3", Some("security"), vec![], Severity::Warning);

        let rules = vec![&rule1, &rule2, &rule3];
        let filtered = filter.apply(rules, &sources);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "test1");
        assert_eq!(filtered[1].name, "test3");
    }

    #[test]
    fn test_combined_filters() {
        let filter = RuleFilter::by_category("security")
            .with_severities(vec![Severity::Error])
            .with_tags(vec!["critical".to_string()]);

        let sources = HashMap::new();

        let matching_rule =
            create_test_rule("test1", Some("security"), vec!["critical"], Severity::Error);
        let wrong_severity = create_test_rule(
            "test2",
            Some("security"),
            vec!["critical"],
            Severity::Warning,
        );
        let wrong_category =
            create_test_rule("test3", Some("other"), vec!["critical"], Severity::Error);
        let wrong_tags =
            create_test_rule("test4", Some("security"), vec!["other"], Severity::Error);

        assert!(filter.matches(&matching_rule, &sources));
        assert!(!filter.matches(&wrong_severity, &sources));
        assert!(!filter.matches(&wrong_category, &sources));
        assert!(!filter.matches(&wrong_tags, &sources));
    }
}
