//! Prompt filtering functionality
//!
//! This module provides filtering capabilities for prompts based on various criteria
//! such as source, category, and search terms.

use crate::{Prompt, PromptSource};
use std::collections::HashMap;

/// Filter options for prompt selection
#[derive(Debug, Clone, Default)]
pub struct PromptFilter {
    /// Filter by prompt source
    pub source: Option<PromptSource>,
    /// Filter by category
    pub category: Option<String>,
    /// Filter by search term (matches name, description, tags)
    pub search_term: Option<String>,
    /// Filter by required argument name
    pub has_arg: Option<String>,
    /// Filter to only show prompts with no arguments
    pub no_args: bool,
}

impl PromptFilter {
    /// Creates a new empty filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the source filter
    pub fn with_source(mut self, source: PromptSource) -> Self {
        self.source = Some(source);
        self
    }

    /// Sets the category filter
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    /// Sets the search term filter
    pub fn with_search_term(mut self, term: impl Into<String>) -> Self {
        self.search_term = Some(term.into());
        self
    }

    /// Sets the has_arg filter
    pub fn with_has_arg(mut self, arg_name: impl Into<String>) -> Self {
        self.has_arg = Some(arg_name.into());
        self
    }

    /// Sets the no_args filter
    pub fn with_no_args(mut self, no_args: bool) -> Self {
        self.no_args = no_args;
        self
    }

    /// Applies the filter to a list of prompts
    pub fn apply(
        &self,
        prompts: Vec<Prompt>,
        sources: &HashMap<String, PromptSource>,
    ) -> Vec<Prompt> {
        prompts
            .into_iter()
            .filter(|prompt| self.matches(prompt, sources))
            .collect()
    }

    /// Checks if a prompt matches the filter criteria
    pub fn matches(&self, prompt: &Prompt, sources: &HashMap<String, PromptSource>) -> bool {
        // Check source filter
        if let Some(ref filter_source) = self.source {
            let prompt_source = sources.get(&prompt.name);
            if prompt_source != Some(filter_source) {
                return false;
            }
        }

        // Check category filter
        if let Some(ref filter_category) = self.category {
            if prompt.category.as_deref() != Some(filter_category) {
                return false;
            }
        }

        // Check search term filter
        if let Some(ref search_term) = self.search_term {
            let search_lower = search_term.to_lowercase();
            let name_matches = prompt.name.to_lowercase().contains(&search_lower);
            let desc_matches = prompt
                .description
                .as_ref()
                .map(|d| d.to_lowercase().contains(&search_lower))
                .unwrap_or(false);
            let category_matches = prompt
                .category
                .as_ref()
                .map(|c| c.to_lowercase().contains(&search_lower))
                .unwrap_or(false);
            let tag_matches = prompt
                .tags
                .iter()
                .any(|t| t.to_lowercase().contains(&search_lower));

            if !(name_matches || desc_matches || category_matches || tag_matches) {
                return false;
            }
        }

        // Check has_arg filter
        if let Some(ref arg_name) = self.has_arg {
            if !prompt.parameters.iter().any(|param| param.name == *arg_name) {
                return false;
            }
        }

        // Check no_args filter
        if self.no_args && !prompt.parameters.is_empty() {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Prompt, PromptSource, common::Parameter, common::ParameterType};
    use std::collections::HashMap;

    fn create_test_prompt(name: &str, category: Option<&str>, tags: Vec<&str>) -> Prompt {
        let mut prompt = Prompt::new(name, format!("Template for {name}"))
            .with_description(format!("Description for {name}"));

        if let Some(cat) = category {
            prompt = prompt.with_category(cat);
        }

        if !tags.is_empty() {
            prompt = prompt.with_tags(tags.into_iter().map(|s| s.to_string()).collect());
        }

        prompt
    }

    fn create_test_sources() -> HashMap<String, PromptSource> {
        let mut sources = HashMap::new();
        sources.insert("builtin_prompt".to_string(), PromptSource::Builtin);
        sources.insert("user_prompt".to_string(), PromptSource::User);
        sources.insert("local_prompt".to_string(), PromptSource::Local);
        sources
    }

    #[test]
    fn test_filter_by_source() {
        let prompts = vec![
            create_test_prompt("builtin_prompt", Some("dev"), vec![]),
            create_test_prompt("user_prompt", Some("dev"), vec![]),
            create_test_prompt("local_prompt", Some("dev"), vec![]),
        ];
        let sources = create_test_sources();

        let filter = PromptFilter::new().with_source(PromptSource::Builtin);
        let filtered = filter.apply(prompts, &sources);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "builtin_prompt");
    }

    #[test]
    fn test_filter_by_category() {
        let prompts = vec![
            create_test_prompt("prompt1", Some("development"), vec![]),
            create_test_prompt("prompt2", Some("writing"), vec![]),
            create_test_prompt("prompt3", Some("development"), vec![]),
        ];
        let sources = HashMap::new();

        let filter = PromptFilter::new().with_category("development");
        let filtered = filter.apply(prompts, &sources);

        assert_eq!(filtered.len(), 2);
        assert!(filtered
            .iter()
            .all(|p| p.category.as_deref() == Some("development")));
    }

    #[test]
    fn test_filter_by_search_term() {
        let prompts = vec![
            create_test_prompt("debug_helper", Some("dev"), vec!["debugging", "code"]),
            create_test_prompt("write_essay", Some("writing"), vec!["essay", "text"]),
            create_test_prompt("code_review", Some("dev"), vec!["review", "code"]),
        ];
        let sources = HashMap::new();

        // Search by name
        let filter = PromptFilter::new().with_search_term("debug");
        let filtered = filter.apply(prompts.clone(), &sources);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "debug_helper");

        // Search by tag
        let filter = PromptFilter::new().with_search_term("code");
        let filtered = filter.apply(prompts.clone(), &sources);
        assert_eq!(filtered.len(), 2);

        // Search by description
        let filter = PromptFilter::new().with_search_term("Description for write");
        let filtered = filter.apply(prompts, &sources);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "write_essay");
    }

    #[test]
    fn test_filter_by_arguments() {
        let prompt_with_args = create_test_prompt("with_args", Some("dev"), vec![])
            .add_parameter(
                Parameter::new("input", "Input data", ParameterType::String)
                    .required(true)
            );

        let prompt_no_args = create_test_prompt("no_args", Some("dev"), vec![]);

        let prompts = vec![prompt_with_args.clone(), prompt_no_args.clone()];
        let sources = HashMap::new();

        // Filter by has_arg
        let filter = PromptFilter::new().with_has_arg("input");
        let filtered = filter.apply(prompts.clone(), &sources);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "with_args");

        // Filter by no_args
        let filter = PromptFilter::new().with_no_args(true);
        let filtered = filter.apply(prompts, &sources);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "no_args");
    }

    #[test]
    fn test_combined_filters() {
        let prompts = vec![
            create_test_prompt("builtin_debug", Some("development"), vec!["debug"]),
            create_test_prompt("user_write", Some("writing"), vec!["text"]),
            create_test_prompt("local_debug", Some("development"), vec!["debug"]),
        ];

        // Add source mappings
        let mut sources = HashMap::new();
        sources.insert("builtin_debug".to_string(), PromptSource::Builtin);
        sources.insert("user_write".to_string(), PromptSource::User);
        sources.insert("local_debug".to_string(), PromptSource::Local);

        // Combine source and category filters
        let filter = PromptFilter::new()
            .with_source(PromptSource::Builtin)
            .with_category("development");
        let filtered = filter.apply(prompts.clone(), &sources);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "builtin_debug");

        // Combine category and search term
        let filter = PromptFilter::new()
            .with_category("development")
            .with_search_term("debug");
        let filtered = filter.apply(prompts, &sources);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|p| p.name.contains("debug")));
    }
}
