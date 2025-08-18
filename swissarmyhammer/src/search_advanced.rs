//! Advanced search functionality for prompts
//!
//! This module extends the basic search functionality with additional features
//! like regex search, case sensitivity options, excerpt generation, and more.

use crate::prompt_search::{SearchEngine, SearchResult};
use crate::{Prompt, PromptFilter, Result};
use regex::Regex;
use std::collections::HashMap;

/// Advanced search options
#[derive(Debug, Clone, Default)]
pub struct AdvancedSearchOptions {
    /// Use regex pattern matching
    pub regex: bool,
    /// Use fuzzy matching
    pub fuzzy: bool,
    /// Case sensitive search
    pub case_sensitive: bool,
    /// Generate excerpts with highlights
    pub highlight: bool,
    /// Maximum number of results
    pub limit: Option<usize>,
}

/// Advanced search result with additional metadata
#[derive(Debug, Clone)]
pub struct AdvancedSearchResult {
    /// The prompt
    pub prompt: Prompt,
    /// Relevance score
    pub score: f32,
    /// Excerpt with highlighted matches
    pub excerpt: Option<String>,
}

impl From<SearchResult> for AdvancedSearchResult {
    fn from(result: SearchResult) -> Self {
        Self {
            prompt: result.prompt,
            score: result.score,
            excerpt: None,
        }
    }
}

/// Enhanced search engine with advanced features
pub struct AdvancedSearchEngine {
    base_engine: SearchEngine,
}

impl AdvancedSearchEngine {
    /// Create a new advanced search engine
    pub fn new() -> Result<Self> {
        Ok(Self {
            base_engine: SearchEngine::new()?,
        })
    }

    /// Search with advanced options
    pub fn search(
        &self,
        query: &str,
        prompts: &[Prompt],
        options: &AdvancedSearchOptions,
        filter: Option<&PromptFilter>,
        sources: &HashMap<String, crate::PromptSource>,
    ) -> Result<Vec<AdvancedSearchResult>> {
        // Apply filter first if provided
        let filtered_prompts = if let Some(f) = filter {
            f.apply(prompts.to_vec(), sources)
        } else {
            prompts.to_vec()
        };

        let mut results = if options.regex {
            self.regex_search(query, &filtered_prompts, options.case_sensitive)?
        } else if options.fuzzy {
            self.fuzzy_search(query, &filtered_prompts)
                .into_iter()
                .map(AdvancedSearchResult::from)
                .collect()
        } else {
            self.simple_search(query, &filtered_prompts, options.case_sensitive)
        };

        // Sort by score
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        // Apply limit
        if let Some(limit) = options.limit {
            results.truncate(limit);
        }

        // Generate excerpts if requested
        if options.highlight {
            for result in &mut results {
                result.excerpt = generate_excerpt(&result.prompt.template, query, true);
            }
        }

        Ok(results)
    }

    /// Simple substring search
    fn simple_search(
        &self,
        query: &str,
        prompts: &[Prompt],
        case_sensitive: bool,
    ) -> Vec<AdvancedSearchResult> {
        let query_lower = if case_sensitive {
            query.to_string()
        } else {
            query.to_lowercase()
        };

        let mut results = Vec::new();

        for prompt in prompts {
            let name_check = if case_sensitive {
                &prompt.name
            } else {
                &prompt.name.to_lowercase()
            };

            let mut matched = name_check.contains(&query_lower);

            if !matched {
                if let Some(desc) = &prompt.description {
                    matched = if case_sensitive {
                        desc.contains(query)
                    } else {
                        desc.to_lowercase().contains(&query_lower)
                    };
                }
            }

            if !matched {
                matched = prompt.template.contains(query);
            }

            if matched {
                results.push(AdvancedSearchResult {
                    prompt: prompt.clone(),
                    score: 100.0,
                    excerpt: None,
                });
            }
        }

        results
    }

    /// Regex-based search
    fn regex_search(
        &self,
        pattern: &str,
        prompts: &[Prompt],
        case_sensitive: bool,
    ) -> Result<Vec<AdvancedSearchResult>> {
        let re = if case_sensitive {
            Regex::new(pattern)
        } else {
            Regex::new(&format!("(?i){pattern}"))
        }
        .map_err(|e| crate::SwissArmyHammerError::Other(format!("Invalid regex: {e}")))?;

        let mut results = Vec::new();

        for prompt in prompts {
            let matched = re.is_match(&prompt.name)
                || prompt
                    .description
                    .as_ref()
                    .map(|d| re.is_match(d))
                    .unwrap_or(false)
                || re.is_match(&prompt.template);

            if matched {
                results.push(AdvancedSearchResult {
                    prompt: prompt.clone(),
                    score: 100.0,
                    excerpt: None,
                });
            }
        }

        Ok(results)
    }

    /// Fuzzy search using the base engine
    fn fuzzy_search(&self, query: &str, prompts: &[Prompt]) -> Vec<SearchResult> {
        self.base_engine.fuzzy_search(query, prompts)
    }
}

/// Generate an excerpt with optional highlighting
pub fn generate_excerpt(content: &str, query: &str, highlight: bool) -> Option<String> {
    let query_lower = query.to_lowercase();
    let content_lower = content.to_lowercase();

    if let Some(pos) = content_lower.find(&query_lower) {
        let start = pos.saturating_sub(30);
        let end = (pos + query.len() + 30).min(content.len());

        let excerpt = &content[start..end];

        if highlight {
            // In a terminal context, we might use ANSI codes, but for library usage,
            // we'll use a simple marker
            let highlighted = excerpt.replace(query, &format!("**{query}**"));
            Some(format!("...{highlighted}..."))
        } else {
            Some(format!("...{excerpt}..."))
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{common::Parameter, common::ParameterType, Prompt};

    fn create_test_prompts() -> Vec<Prompt> {
        vec![
            Prompt::new("debug_helper", "Debug this error: {{error}}")
                .with_description("Helps debug programming errors")
                .with_tags(vec!["debug".to_string(), "error".to_string()]),
            Prompt::new("code_review", "Review this code: {{code}}")
                .with_description("Performs code review")
                .with_tags(vec!["review".to_string(), "code".to_string()]),
            Prompt::new("test_writer", "Write tests for: {{function}}")
                .with_description("Generates unit tests")
                .with_tags(vec!["test".to_string(), "tdd".to_string()]),
        ]
    }

    #[test]
    fn test_simple_search_case_insensitive() {
        let engine = AdvancedSearchEngine::new().unwrap();
        let prompts = create_test_prompts();
        let options = AdvancedSearchOptions {
            case_sensitive: false,
            ..Default::default()
        };

        let results = engine
            .search("DEBUG", &prompts, &options, None, &HashMap::new())
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].prompt.name, "debug_helper");
    }

    #[test]
    fn test_simple_search_case_sensitive() {
        let engine = AdvancedSearchEngine::new().unwrap();
        let prompts = create_test_prompts();
        let options = AdvancedSearchOptions {
            case_sensitive: true,
            ..Default::default()
        };

        // Should not find anything with uppercase
        let results = engine
            .search("DEBUG", &prompts, &options, None, &HashMap::new())
            .unwrap();
        assert_eq!(results.len(), 0);

        // Should find with correct case
        let results = engine
            .search("debug", &prompts, &options, None, &HashMap::new())
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_regex_search() {
        let engine = AdvancedSearchEngine::new().unwrap();
        let prompts = create_test_prompts();
        let options = AdvancedSearchOptions {
            regex: true,
            ..Default::default()
        };

        let results = engine
            .search("code.*review", &prompts, &options, None, &HashMap::new())
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].prompt.name, "code_review");
    }

    #[test]
    fn test_search_with_filter() {
        let engine = AdvancedSearchEngine::new().unwrap();
        let mut prompts = create_test_prompts();

        // Add a prompt with a parameter
        let prompt_with_arg = Prompt::new("arg_test", "Test {{input}}")
            .with_description("Has parameters")
            .add_parameter(
                Parameter::new("input", "Input value", ParameterType::String).required(true),
            );
        prompts.push(prompt_with_arg);

        // Search for prompts with no arguments
        let filter = PromptFilter::new().with_no_args(true);
        let options = AdvancedSearchOptions::default();

        let results = engine
            .search("test", &prompts, &options, Some(&filter), &HashMap::new())
            .unwrap();

        // Should find test_writer but not arg_test
        assert!(results.iter().any(|r| r.prompt.name == "test_writer"));
        assert!(!results.iter().any(|r| r.prompt.name == "arg_test"));
    }

    #[test]
    fn test_search_with_limit() {
        let engine = AdvancedSearchEngine::new().unwrap();
        let prompts = create_test_prompts();
        let options = AdvancedSearchOptions {
            limit: Some(2),
            ..Default::default()
        };

        let results = engine
            .search("e", &prompts, &options, None, &HashMap::new())
            .unwrap();

        assert!(results.len() <= 2);
    }

    #[test]
    fn test_excerpt_generation() {
        let content = "This is a long text with the keyword somewhere in the middle of it";
        let excerpt = generate_excerpt(content, "keyword", false);

        assert!(excerpt.is_some());
        let excerpt_text = excerpt.unwrap();
        assert!(excerpt_text.contains("keyword"));
        assert!(excerpt_text.starts_with("..."));
        assert!(excerpt_text.ends_with("..."));
    }

    #[test]
    fn test_excerpt_generation_with_highlight() {
        let content = "This is a long text with the keyword somewhere in the middle of it";
        let excerpt = generate_excerpt(content, "keyword", true);

        assert!(excerpt.is_some());
        let excerpt_text = excerpt.unwrap();
        assert!(excerpt_text.contains("**keyword**"));
    }
}
