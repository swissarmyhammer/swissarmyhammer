//! Web search pipeline — DuckDuckGo search with content fetching
//!
//! This module provides the `WebSearcher` struct with reusable search pipeline methods.

pub mod content_fetcher;
pub mod duckduckgo;

use crate::search::content_fetcher::ContentFetchConfig;
use crate::search::duckduckgo::DuckDuckGoClient;
use crate::types::ScoringConfig;
use crate::types::*;
use std::time::Duration;

/// Reusable web search pipeline providing DuckDuckGo search, content fetching, and validation.
#[derive(Default)]
pub struct WebSearcher {
    duckduckgo_client: Option<DuckDuckGoClient>,
}

impl WebSearcher {
    /// Creates a new instance of the WebSearcher
    pub fn new() -> Self {
        Self {
            duckduckgo_client: None,
        }
    }

    /// Gets or creates a DuckDuckGo web search client
    pub fn get_duckduckgo_client(&mut self) -> &mut DuckDuckGoClient {
        if self.duckduckgo_client.is_none() {
            let config = Self::load_scoring_config();
            self.duckduckgo_client = Some(DuckDuckGoClient::with_scoring_config(config));
        }
        self.duckduckgo_client.as_mut().unwrap()
    }

    /// Helper function to load configuration with a callback for setting values
    fn load_config_with_callback<T, F>(mut config: T, configure_fn: F) -> T
    where
        F: FnOnce(&mut T, &swissarmyhammer_config::TemplateContext),
    {
        if let Ok(template_context) = swissarmyhammer_config::load_configuration_for_cli() {
            configure_fn(&mut config, &template_context);
        }
        config
    }

    /// Loads configuration for content fetching
    pub fn load_content_fetch_config() -> ContentFetchConfig {
        Self::load_config_with_callback(
            ContentFetchConfig::default(),
            |config, template_context| {
                if let Some(serde_json::Value::Number(max_concurrent)) =
                    template_context.get("web_search.content_fetching.max_concurrent_fetches")
                {
                    if let Some(max_concurrent) = max_concurrent.as_i64() {
                        if max_concurrent > 0 {
                            config.max_concurrent_fetches = max_concurrent as usize;
                        }
                    }
                }

                if let Some(serde_json::Value::Number(timeout)) =
                    template_context.get("web_search.content_fetching.content_fetch_timeout")
                {
                    if let Some(timeout) = timeout.as_i64() {
                        if timeout > 0 {
                            config.fetch_timeout = Duration::from_secs(timeout as u64);
                        }
                    }
                }

                if let Some(serde_json::Value::String(size_str)) =
                    template_context.get("web_search.content_fetching.max_content_size")
                {
                    if let Ok(size) = Self::parse_size_string(size_str) {
                        config.max_content_size = size;
                    }
                }

                if let Some(serde_json::Value::Number(delay)) =
                    template_context.get("web_search.content_fetching.default_domain_delay")
                {
                    if let Some(delay) = delay.as_i64() {
                        if delay > 0 {
                            config.default_domain_delay = Duration::from_millis(delay as u64);
                        }
                    }
                }

                if let Some(serde_json::Value::Number(min_length)) =
                    template_context.get("web_search.content_fetching.min_content_length")
                {
                    if let Some(min_length) = min_length.as_i64() {
                        if min_length > 0 {
                            config.quality_config.min_content_length = min_length as usize;
                        }
                    }
                }

                if let Some(serde_json::Value::Number(max_length)) =
                    template_context.get("web_search.content_fetching.max_content_length")
                {
                    if let Some(max_length) = max_length.as_i64() {
                        if max_length > 0 {
                            config.quality_config.max_content_length = max_length as usize;
                        }
                    }
                }

                if let Some(serde_json::Value::Number(max_summary)) =
                    template_context.get("web_search.content_fetching.max_summary_length")
                {
                    if let Some(max_summary) = max_summary.as_i64() {
                        if max_summary > 0 {
                            config.processing_config.max_summary_length = max_summary as usize;
                        }
                    }
                }

                if let Some(serde_json::Value::Bool(extract_code)) =
                    template_context.get("web_search.content_fetching.extract_code_blocks")
                {
                    config.processing_config.extract_code_blocks = *extract_code;
                }

                if let Some(serde_json::Value::Bool(generate_summaries)) =
                    template_context.get("web_search.content_fetching.generate_summaries")
                {
                    config.processing_config.generate_summaries = *generate_summaries;
                }

                if let Some(serde_json::Value::Bool(extract_metadata)) =
                    template_context.get("web_search.content_fetching.extract_metadata")
                {
                    config.processing_config.extract_metadata = *extract_metadata;
                }
            },
        )
    }

    /// Loads configuration for DuckDuckGo scoring algorithm
    fn load_scoring_config() -> ScoringConfig {
        Self::load_config_with_callback(ScoringConfig::default(), |config, template_context| {
            if let Some(serde_json::Value::Number(base_score)) =
                template_context.get("web_search.scoring.base_score")
            {
                if let Some(base_score) = base_score.as_f64() {
                    config.base_score = base_score;
                }
            }

            if let Some(serde_json::Value::Number(position_penalty)) =
                template_context.get("web_search.scoring.position_penalty")
            {
                if let Some(position_penalty) = position_penalty.as_f64() {
                    config.position_penalty = position_penalty;
                }
            }

            if let Some(serde_json::Value::Number(min_score)) =
                template_context.get("web_search.scoring.min_score")
            {
                if let Some(min_score) = min_score.as_f64() {
                    config.min_score = min_score;
                }
            }

            if let Some(serde_json::Value::Bool(exponential_decay)) =
                template_context.get("web_search.scoring.exponential_decay")
            {
                config.exponential_decay = *exponential_decay;
            }

            if let Some(serde_json::Value::Number(decay_rate)) =
                template_context.get("web_search.scoring.decay_rate")
            {
                if let Some(decay_rate) = decay_rate.as_f64() {
                    config.decay_rate = decay_rate;
                }
            }
        })
    }

    /// Parse size string like "2MB" into bytes
    fn parse_size_string(size_str: &str) -> Result<usize, std::num::ParseIntError> {
        let size_str = size_str.to_uppercase();
        if let Some(stripped) = size_str.strip_suffix("MB") {
            Ok(stripped.parse::<usize>()? * 1024 * 1024)
        } else if let Some(stripped) = size_str.strip_suffix("KB") {
            Ok(stripped.parse::<usize>()? * 1024)
        } else if let Some(stripped) = size_str.strip_suffix("GB") {
            Ok(stripped.parse::<usize>()? * 1024 * 1024 * 1024)
        } else {
            size_str.parse()
        }
    }

    /// Validates all request parameters comprehensively
    pub fn validate_request(request: &WebSearchRequest) -> Result<(), String> {
        // Query validation
        if request.query.trim().is_empty() {
            return Err("Search query cannot be empty".to_string());
        }

        if request.query.len() > 500 {
            return Err(format!(
                "Search query is {} characters, maximum is 500",
                request.query.len()
            ));
        }

        // Language validation if provided
        if let Some(language) = &request.language {
            let re = regex::Regex::new(r"^[a-z]{2}(-[A-Z]{2})?$")
                .map_err(|e| format!("Failed to compile language regex: {e}"))?;

            if !re.is_match(language) {
                return Err(format!(
                    "Invalid language code '{language}'. Expected format: 'en' or 'en-US'"
                ));
            }
        }

        // Results count validation
        if let Some(count) = request.results_count {
            if count == 0 {
                return Err("Results count must be at least 1".to_string());
            }
            if count > 50 {
                return Err(format!("Results count is {count}, maximum is 50"));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_searcher_new() {
        let _tool = WebSearcher::new();
    }

    #[test]
    fn test_validate_request_valid() {
        let request = WebSearchRequest {
            query: "test query".to_string(),
            category: Some(SearchCategory::General),
            language: Some("en".to_string()),
            results_count: Some(10),
            fetch_content: Some(true),
            safe_search: Some(SafeSearchLevel::Moderate),
            time_range: Some(TimeRange::Month),
        };
        assert!(WebSearcher::validate_request(&request).is_ok());
    }

    #[test]
    fn test_validate_request_empty_query() {
        let request = WebSearchRequest {
            query: "".to_string(),
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        let result = WebSearcher::validate_request(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_validate_request_query_too_long() {
        let long_query = "a".repeat(501);
        let request = WebSearchRequest {
            query: long_query,
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        let result = WebSearcher::validate_request(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("501 characters"));
    }

    #[test]
    fn test_validate_request_invalid_language() {
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: Some("invalid".to_string()),
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        let result = WebSearcher::validate_request(&request);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid language code"));
    }

    #[test]
    fn test_validate_request_invalid_results_count() {
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: None,
            results_count: Some(0),
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        let result = WebSearcher::validate_request(&request);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("must be at least 1"));

        let request_high = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: None,
            results_count: Some(100),
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        let result_high = WebSearcher::validate_request(&request_high);
        assert!(result_high.is_err());
        assert!(result_high
            .unwrap_err()
            .to_string()
            .contains("maximum is 50"));
    }
}
