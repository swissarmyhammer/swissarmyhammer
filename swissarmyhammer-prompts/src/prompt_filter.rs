//! Prompt filtering functionality
//!
//! This module provides filtering capabilities to select prompts based
//! on various criteria like name patterns, tags, categories, and sources.

use crate::prompts::Prompt;
use crate::PromptSource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Filter criteria for selecting prompts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptFilter {
    /// Name pattern to match (supports glob patterns)
    pub name_pattern: Option<String>,
    /// Category to filter by
    pub category: Option<String>,
    /// Tags that prompts must have (any match)
    pub tags: Vec<String>,
    /// Sources to include
    pub sources: Vec<PromptSource>,
    /// Whether to include partial templates
    pub include_partials: bool,
}

impl PromptFilter {
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
    pub fn by_sources(sources: Vec<PromptSource>) -> Self {
        Self {
            sources,
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
    pub fn with_sources(mut self, sources: Vec<PromptSource>) -> Self {
        self.sources = sources;
        self
    }

    /// Apply the filter to a list of prompts
    pub fn apply(
        &self,
        prompts: Vec<&Prompt>,
        sources: &HashMap<String, PromptSource>,
    ) -> Vec<Prompt> {
        prompts
            .into_iter()
            .filter(|prompt| self.matches(prompt, sources))
            .cloned()
            .collect()
    }

    /// Check if a prompt matches the filter criteria
    pub fn matches(&self, prompt: &Prompt, sources: &HashMap<String, PromptSource>) -> bool {
        // Check name pattern
        if let Some(pattern) = &self.name_pattern {
            if !self.matches_pattern(&prompt.name, pattern) {
                return false;
            }
        }

        // Check category
        if let Some(category) = &self.category {
            match &prompt.category {
                Some(prompt_category) if prompt_category == category => {}
                _ => return false,
            }
        }

        // Check tags (any match)
        if !self.tags.is_empty() {
            let has_matching_tag = self.tags.iter().any(|filter_tag| {
                prompt
                    .tags
                    .iter()
                    .any(|prompt_tag| prompt_tag == filter_tag)
            });
            if !has_matching_tag {
                return false;
            }
        }

        // Check sources
        if !self.sources.is_empty() {
            if let Some(prompt_source) = sources.get(&prompt.name) {
                if !self.sources.contains(prompt_source) {
                    return false;
                }
            } else {
                // If source is unknown, exclude it
                return false;
            }
        }

        // Check if it's a partial template
        if !self.include_partials && self.is_partial(prompt) {
            return false;
        }

        true
    }

    /// Check if a prompt is a partial template
    fn is_partial(&self, prompt: &Prompt) -> bool {
        prompt
            .description
            .as_ref()
            .map(|desc| desc == "Partial template for reuse in other prompts")
            .unwrap_or(false)
            || prompt.name.to_lowercase().contains("partial")
            || prompt.name.starts_with('_')
            || prompt.template.trim_start().starts_with("{% partial %}")
    }

    /// Check if a string matches a pattern (supports basic glob patterns)
    fn matches_pattern(&self, text: &str, pattern: &str) -> bool {
        if pattern == "*" {
            return true;
        }

        if pattern.contains('*') || pattern.contains('?') {
            // Use glob matching
            if let Ok(glob) = glob::Pattern::new(pattern) {
                return glob.matches(text);
            }
        }

        // Exact match or contains match
        text.contains(pattern)
    }

    /// Check if the filter is empty (matches everything)
    pub fn is_empty(&self) -> bool {
        self.name_pattern.is_none()
            && self.category.is_none()
            && self.tags.is_empty()
            && self.sources.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_prompt(name: &str, category: Option<&str>, tags: Vec<&str>) -> Prompt {
        let mut prompt = Prompt::new(name, "Template content");
        if let Some(cat) = category {
            prompt.category = Some(cat.to_string());
        }
        prompt.tags = tags.iter().map(|s| s.to_string()).collect();
        prompt
    }

    #[test]
    fn test_empty_filter() {
        let filter = PromptFilter::new();
        let prompt = create_test_prompt("test", None, vec![]);
        let sources = HashMap::new();

        assert!(filter.matches(&prompt, &sources));
        assert!(filter.is_empty());
    }

    #[test]
    fn test_name_pattern_filter() {
        let filter = PromptFilter::by_name_pattern("test*");
        let sources = HashMap::new();

        let matching_prompt = create_test_prompt("test_prompt", None, vec![]);
        let non_matching_prompt = create_test_prompt("other_prompt", None, vec![]);

        assert!(filter.matches(&matching_prompt, &sources));
        assert!(!filter.matches(&non_matching_prompt, &sources));
    }

    #[test]
    fn test_category_filter() {
        let filter = PromptFilter::by_category("development");
        let sources = HashMap::new();

        let matching_prompt = create_test_prompt("test", Some("development"), vec![]);
        let non_matching_prompt = create_test_prompt("test", Some("other"), vec![]);
        let no_category_prompt = create_test_prompt("test", None, vec![]);

        assert!(filter.matches(&matching_prompt, &sources));
        assert!(!filter.matches(&non_matching_prompt, &sources));
        assert!(!filter.matches(&no_category_prompt, &sources));
    }

    #[test]
    fn test_tags_filter() {
        let filter = PromptFilter::by_tags(vec!["coding".to_string(), "review".to_string()]);
        let sources = HashMap::new();

        let matching_prompt = create_test_prompt("test", None, vec!["coding", "helper"]);
        let non_matching_prompt = create_test_prompt("test", None, vec!["other", "helper"]);
        let no_tags_prompt = create_test_prompt("test", None, vec![]);

        assert!(filter.matches(&matching_prompt, &sources));
        assert!(!filter.matches(&non_matching_prompt, &sources));
        assert!(!filter.matches(&no_tags_prompt, &sources));
    }

    #[test]
    fn test_source_filter() {
        let filter = PromptFilter::by_sources(vec![PromptSource::Builtin]);
        let mut sources = HashMap::new();
        sources.insert("builtin_prompt".to_string(), PromptSource::Builtin);
        sources.insert("user_prompt".to_string(), PromptSource::User);

        let builtin_prompt = create_test_prompt("builtin_prompt", None, vec![]);
        let user_prompt = create_test_prompt("user_prompt", None, vec![]);
        let unknown_prompt = create_test_prompt("unknown_prompt", None, vec![]);

        assert!(filter.matches(&builtin_prompt, &sources));
        assert!(!filter.matches(&user_prompt, &sources));
        assert!(!filter.matches(&unknown_prompt, &sources));
    }

    #[test]
    fn test_partial_detection() {
        let filter = PromptFilter::new().with_partials(false);
        let sources = HashMap::new();

        let partial_by_description = Prompt {
            name: "test".to_string(),
            template: "content".to_string(),
            description: Some("Partial template for reuse in other prompts".to_string()),
            category: None,
            tags: vec![],
            parameters: vec![],
            source: None,
            metadata: HashMap::new(),
        };

        let partial_by_name = create_test_prompt("_partial_test", None, vec![]);
        let regular_prompt = create_test_prompt("regular_prompt", None, vec![]);

        assert!(!filter.matches(&partial_by_description, &sources));
        assert!(!filter.matches(&partial_by_name, &sources));
        assert!(filter.matches(&regular_prompt, &sources));
    }

    #[test]
    fn test_apply_filter() {
        let filter = PromptFilter::by_category("development");
        let sources = HashMap::new();

        let prompt1 = create_test_prompt("test1", Some("development"), vec![]);
        let prompt2 = create_test_prompt("test2", Some("other"), vec![]);
        let prompt3 = create_test_prompt("test3", Some("development"), vec![]);

        let prompts = vec![&prompt1, &prompt2, &prompt3];
        let filtered = filter.apply(prompts, &sources);

        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].name, "test1");
        assert_eq!(filtered[1].name, "test3");
    }

    #[test]
    fn test_empty_filter_matches_all_prompts() {
        let filter = PromptFilter::new();
        let sources = HashMap::new();

        let prompts = [
            create_test_prompt("one", Some("cat1"), vec!["tag1"]),
            create_test_prompt("two", None, vec![]),
            create_test_prompt("three", Some("cat2"), vec!["tag2", "tag3"]),
        ];

        let refs: Vec<&Prompt> = prompts.iter().collect();
        let filtered = filter.apply(refs, &sources);
        // Empty filter excludes partials by default; all have no partial markers
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_wildcard_name_pattern() {
        let filter = PromptFilter::by_name_pattern("*");
        let sources = HashMap::new();

        let prompt = create_test_prompt("anything", None, vec![]);
        assert!(filter.matches(&prompt, &sources));
    }

    #[test]
    fn test_glob_name_pattern_with_question_mark() {
        let filter = PromptFilter::by_name_pattern("te?t");
        let sources = HashMap::new();

        let matching = create_test_prompt("test", None, vec![]);
        let non_matching = create_test_prompt("text-more", None, vec![]);

        assert!(filter.matches(&matching, &sources));
        assert!(!filter.matches(&non_matching, &sources));
    }

    #[test]
    fn test_substring_name_pattern() {
        // Pattern without glob characters should do substring match
        let filter = PromptFilter::by_name_pattern("debug");
        let sources = HashMap::new();

        let matching = create_test_prompt("my-debug-tool", None, vec![]);
        let non_matching = create_test_prompt("format-code", None, vec![]);

        assert!(filter.matches(&matching, &sources));
        assert!(!filter.matches(&non_matching, &sources));
    }

    #[test]
    fn test_multiple_tags_any_match() {
        // Tags filter uses OR logic - any matching tag should pass
        let filter = PromptFilter::by_tags(vec!["rust".to_string(), "python".to_string()]);
        let sources = HashMap::new();

        let has_rust = create_test_prompt("rust-prompt", None, vec!["rust"]);
        let has_python = create_test_prompt("python-prompt", None, vec!["python"]);
        let has_both = create_test_prompt("both-prompt", None, vec!["rust", "python"]);
        let has_neither = create_test_prompt("other-prompt", None, vec!["java"]);

        assert!(filter.matches(&has_rust, &sources));
        assert!(filter.matches(&has_python, &sources));
        assert!(filter.matches(&has_both, &sources));
        assert!(!filter.matches(&has_neither, &sources));
    }

    #[test]
    fn test_partial_detection_by_name_contains_partial() {
        let filter = PromptFilter::new().with_partials(false);
        let sources = HashMap::new();

        // "partial" in name should be detected as partial
        let partial_by_name = create_test_prompt("header-partial", None, vec![]);
        let partial_underscore = create_test_prompt("_sidebar", None, vec![]);
        let regular = create_test_prompt("sidebar-content", None, vec![]);

        assert!(!filter.matches(&partial_by_name, &sources));
        assert!(!filter.matches(&partial_underscore, &sources));
        assert!(filter.matches(&regular, &sources));
    }

    #[test]
    fn test_partial_detection_by_template_marker() {
        let filter = PromptFilter::new().with_partials(false);
        let sources = HashMap::new();

        let partial_by_marker = Prompt {
            name: "my-template".to_string(),
            template: "{% partial %}\nContent here".to_string(),
            description: None,
            category: None,
            tags: vec![],
            parameters: vec![],
            source: None,
            metadata: HashMap::new(),
        };

        assert!(!filter.matches(&partial_by_marker, &sources));
    }

    #[test]
    fn test_include_partials_true() {
        // When include_partials is true, partial templates should be included
        let filter = PromptFilter::new().with_partials(true);
        let sources = HashMap::new();

        let partial = create_test_prompt("_partial-name", None, vec![]);
        assert!(filter.matches(&partial, &sources));
    }

    #[test]
    fn test_is_empty_with_single_field() {
        let filter_with_name = PromptFilter::new().with_name_pattern("something");
        let filter_with_category = PromptFilter::new().with_category("cat");
        let filter_with_tags = PromptFilter::new().with_tags(vec!["tag".to_string()]);

        assert!(!filter_with_name.is_empty());
        assert!(!filter_with_category.is_empty());
        assert!(!filter_with_tags.is_empty());
    }

    #[test]
    fn test_sources_filter_with_multiple_sources() {
        use PromptSource::*;

        let filter = PromptFilter::by_sources(vec![Builtin, User]);
        let mut sources = HashMap::new();
        sources.insert("builtin_p".to_string(), Builtin);
        sources.insert("user_p".to_string(), User);
        sources.insert("local_p".to_string(), PromptSource::Local);

        let builtin_p = create_test_prompt("builtin_p", None, vec![]);
        let user_p = create_test_prompt("user_p", None, vec![]);
        let local_p = create_test_prompt("local_p", None, vec![]);

        assert!(filter.matches(&builtin_p, &sources));
        assert!(filter.matches(&user_p, &sources));
        assert!(!filter.matches(&local_p, &sources));
    }

    #[test]
    fn test_combined_filter_name_and_category() {
        let filter = PromptFilter::new()
            .with_name_pattern("rust*")
            .with_category("development");
        let sources = HashMap::new();

        let matching = create_test_prompt("rust-debug", Some("development"), vec![]);
        let wrong_category = create_test_prompt("rust-tool", Some("other"), vec![]);
        let wrong_name = create_test_prompt("python-debug", Some("development"), vec![]);

        assert!(filter.matches(&matching, &sources));
        assert!(!filter.matches(&wrong_category, &sources));
        assert!(!filter.matches(&wrong_name, &sources));
    }
}
