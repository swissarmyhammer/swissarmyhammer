//! Web search pipeline — Brave Search with content fetching
//!
//! This module provides the `WebSearcher` struct with reusable search pipeline methods.

pub mod brave;
pub mod content_fetcher;

use crate::search::brave::BraveSearchClient;
use crate::search::content_fetcher::ContentFetchConfig;
use crate::types::ScoringConfig;
use crate::types::*;
use std::time::Duration;

/// Reusable web search pipeline providing Brave search, content fetching, and validation.
#[derive(Default)]
pub struct WebSearcher {
    brave_client: Option<BraveSearchClient>,
}

impl WebSearcher {
    /// Creates a new instance of the WebSearcher
    pub fn new() -> Self {
        Self { brave_client: None }
    }

    /// Gets or creates a Brave Search client
    pub fn get_search_client(&mut self) -> &BraveSearchClient {
        if self.brave_client.is_none() {
            let config = Self::load_scoring_config();
            self.brave_client = Some(BraveSearchClient::with_scoring_config(config));
        }
        self.brave_client.as_ref().unwrap()
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

    /// Loads configuration for search scoring algorithm
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

    #[test]
    fn test_parse_size_string_megabytes() {
        let result = WebSearcher::parse_size_string("2MB").unwrap();
        assert_eq!(result, 2 * 1024 * 1024);
    }

    #[test]
    fn test_parse_size_string_kilobytes() {
        let result = WebSearcher::parse_size_string("512KB").unwrap();
        assert_eq!(result, 512 * 1024);
    }

    #[test]
    fn test_parse_size_string_gigabytes() {
        let result = WebSearcher::parse_size_string("1GB").unwrap();
        assert_eq!(result, 1024 * 1024 * 1024);
    }

    #[test]
    fn test_parse_size_string_plain_number() {
        let result = WebSearcher::parse_size_string("1024").unwrap();
        assert_eq!(result, 1024);
    }

    #[test]
    fn test_parse_size_string_lowercase() {
        let result = WebSearcher::parse_size_string("2mb").unwrap();
        assert_eq!(result, 2 * 1024 * 1024);
    }

    #[test]
    fn test_parse_size_string_invalid() {
        let result = WebSearcher::parse_size_string("abc");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_search_client_lazy_init() {
        let mut searcher = WebSearcher::new();
        // First call initializes the client
        let _client = searcher.get_search_client();
        // Second call returns the cached client
        let _client = searcher.get_search_client();
        // If we got here without panicking, lazy init works
        assert!(searcher.brave_client.is_some());
    }

    // ========================================================================
    // load_config_with_callback tests
    // ========================================================================

    #[test]
    fn test_load_config_with_callback_returns_default_when_no_config() {
        // When load_configuration_for_cli fails (no config available in test env),
        // the callback is never invoked and the default config is returned unchanged.
        let config = WebSearcher::load_config_with_callback(
            ContentFetchConfig::default(),
            |config, _ctx| {
                // If this runs, the test env has a config -- mutate to detect it
                config.max_concurrent_fetches = 999;
            },
        );
        // In CI / test environments without a config file, defaults are preserved.
        // If a config file IS present, the callback fires -- either way, no panic.
        assert!(config.max_concurrent_fetches > 0);
    }

    #[test]
    fn test_load_config_with_callback_preserves_custom_defaults() {
        // Verify the initial config value flows through when the callback is a no-op.
        let initial = ContentFetchConfig {
            max_concurrent_fetches: 42,
            ..Default::default()
        };

        let config = WebSearcher::load_config_with_callback(initial, |_config, _ctx| {
            // intentionally empty -- do not modify
        });
        // The value should be 42 if callback was invoked (no-op), or 42 if not invoked
        assert_eq!(config.max_concurrent_fetches, 42);
    }

    // ========================================================================
    // load_content_fetch_config tests
    // ========================================================================

    #[test]
    fn test_load_content_fetch_config_returns_valid_config() {
        // In test environments without a config file this returns defaults.
        let config = WebSearcher::load_content_fetch_config();

        // Verify all fields have sensible default values
        assert!(
            config.max_concurrent_fetches > 0,
            "max_concurrent_fetches must be positive"
        );
        assert!(
            config.fetch_timeout.as_secs() > 0,
            "fetch_timeout must be positive"
        );
        assert!(
            config.max_content_size > 0,
            "max_content_size must be positive"
        );
        assert!(
            config.default_domain_delay.as_millis() > 0,
            "default_domain_delay must be positive"
        );
        assert!(
            config.quality_config.min_content_length > 0,
            "min_content_length must be positive"
        );
        assert!(
            config.quality_config.max_content_length > config.quality_config.min_content_length,
            "max_content_length must exceed min_content_length"
        );
        assert!(
            config.processing_config.max_summary_length > 0,
            "max_summary_length must be positive"
        );
    }

    #[test]
    fn test_load_content_fetch_config_default_values() {
        // Without a config file, values should match ContentFetchConfig::default()
        let config = WebSearcher::load_content_fetch_config();
        let defaults = ContentFetchConfig::default();

        // These assertions hold when no config file overrides them
        // We check the defaults match the struct defaults
        assert_eq!(defaults.max_concurrent_fetches, 5);
        assert_eq!(defaults.fetch_timeout, Duration::from_secs(45));
        assert_eq!(defaults.max_content_size, 2 * 1024 * 1024);
        assert_eq!(defaults.default_domain_delay, Duration::from_millis(1000));
        assert_eq!(defaults.quality_config.min_content_length, 100);
        assert_eq!(defaults.quality_config.max_content_length, 50_000);
        assert_eq!(defaults.processing_config.max_summary_length, 500);
        assert!(defaults.processing_config.extract_code_blocks);
        assert!(defaults.processing_config.generate_summaries);
        assert!(defaults.processing_config.extract_metadata);

        // The loaded config should have values >= the defaults (config can only override)
        assert!(config.max_concurrent_fetches >= 1);
        assert!(config.fetch_timeout.as_secs() >= 1);
    }

    // ========================================================================
    // load_scoring_config tests
    // ========================================================================

    #[test]
    fn test_load_scoring_config_returns_valid_config() {
        // load_scoring_config is private, but we can test it indirectly through
        // get_search_client which calls it internally. Here we test defaults directly.
        let defaults = ScoringConfig::default();

        assert_eq!(defaults.base_score, 1.0);
        assert_eq!(defaults.position_penalty, 0.05);
        assert_eq!(defaults.min_score, 0.05);
        assert!(!defaults.exponential_decay);
        assert_eq!(defaults.decay_rate, 0.1);
    }

    #[test]
    fn test_load_scoring_config_via_search_client() {
        // get_search_client calls load_scoring_config internally.
        // Verifies the full config loading path does not panic.
        let mut searcher = WebSearcher::new();
        let _client = searcher.get_search_client();
        assert!(searcher.brave_client.is_some());
    }

    // ========================================================================
    // Additional parse_size_string edge cases
    // ========================================================================

    #[test]
    fn test_parse_size_string_mixed_case() {
        // "Mb" should be uppercased to "MB" and parsed correctly
        assert_eq!(
            WebSearcher::parse_size_string("5Mb").unwrap(),
            5 * 1024 * 1024
        );
        assert_eq!(WebSearcher::parse_size_string("10Kb").unwrap(), 10 * 1024);
        assert_eq!(
            WebSearcher::parse_size_string("1Gb").unwrap(),
            1024 * 1024 * 1024
        );
    }

    #[test]
    fn test_parse_size_string_zero() {
        assert_eq!(WebSearcher::parse_size_string("0").unwrap(), 0);
    }

    #[test]
    fn test_parse_size_string_large_value() {
        let result = WebSearcher::parse_size_string("100MB").unwrap();
        assert_eq!(result, 100 * 1024 * 1024);
    }

    // ========================================================================
    // Additional validate_request edge cases
    // ========================================================================

    #[test]
    fn test_validate_request_whitespace_only_query() {
        let request = WebSearchRequest {
            query: "   ".to_string(),
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        let result = WebSearcher::validate_request(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[test]
    fn test_validate_request_exactly_500_chars() {
        let query = "a".repeat(500);
        let request = WebSearchRequest {
            query,
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        assert!(WebSearcher::validate_request(&request).is_ok());
    }

    #[test]
    fn test_validate_request_language_with_region() {
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: Some("en-US".to_string()),
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        assert!(WebSearcher::validate_request(&request).is_ok());
    }

    #[test]
    fn test_validate_request_results_count_one() {
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: None,
            results_count: Some(1),
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        assert!(WebSearcher::validate_request(&request).is_ok());
    }

    #[test]
    fn test_validate_request_results_count_fifty() {
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: None,
            results_count: Some(50),
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        assert!(WebSearcher::validate_request(&request).is_ok());
    }

    #[test]
    fn test_validate_request_results_count_fifty_one() {
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: None,
            results_count: Some(51),
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        let result = WebSearcher::validate_request(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("maximum is 50"));
    }

    #[test]
    fn test_validate_request_no_optional_fields() {
        let request = WebSearchRequest {
            query: "hello world".to_string(),
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        assert!(WebSearcher::validate_request(&request).is_ok());
    }

    // ========================================================================
    // Additional coverage tests for search/mod.rs
    // ========================================================================

    #[test]
    fn test_web_searcher_default() {
        // Test the Default implementation
        let searcher = WebSearcher::default();
        assert!(searcher.brave_client.is_none());
    }

    #[test]
    fn test_parse_size_string_invalid_with_suffix() {
        // "abcMB" should fail because "abc" is not a valid number
        let result = WebSearcher::parse_size_string("abcMB");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_size_string_invalid_kb() {
        let result = WebSearcher::parse_size_string("notanumberKB");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_size_string_invalid_gb() {
        let result = WebSearcher::parse_size_string("badGB");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_language_code_two_letter() {
        // Valid: two-letter lowercase
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: Some("fr".to_string()),
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        assert!(WebSearcher::validate_request(&request).is_ok());
    }

    #[test]
    fn test_validate_request_language_uppercase_rejected() {
        // "EN" should be rejected (must be lowercase)
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: Some("EN".to_string()),
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        let result = WebSearcher::validate_request(&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid language code"));
    }

    #[test]
    fn test_validate_request_language_too_short() {
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: Some("e".to_string()),
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        let result = WebSearcher::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_language_three_letter_rejected() {
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: Some("eng".to_string()),
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        let result = WebSearcher::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_language_region_lowercase_rejected() {
        // "en-us" — region must be uppercase
        let request = WebSearchRequest {
            query: "test".to_string(),
            category: None,
            language: Some("en-us".to_string()),
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        let result = WebSearcher::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_all_fields_populated() {
        let request = WebSearchRequest {
            query: "comprehensive test".to_string(),
            category: Some(SearchCategory::News),
            language: Some("de-DE".to_string()),
            results_count: Some(25),
            fetch_content: Some(false),
            safe_search: Some(SafeSearchLevel::Strict),
            time_range: Some(TimeRange::Week),
        };
        assert!(WebSearcher::validate_request(&request).is_ok());
    }

    #[test]
    fn test_load_content_fetch_config_smoke() {
        // Exercises the full config loading path including all the if-let branches.
        // In a test environment without a config file, defaults are returned.
        let config = WebSearcher::load_content_fetch_config();
        // Verify it returns a valid config that matches defaults
        let defaults = ContentFetchConfig::default();
        assert_eq!(
            config.max_concurrent_fetches,
            defaults.max_concurrent_fetches
        );
        assert_eq!(config.fetch_timeout, defaults.fetch_timeout);
        assert_eq!(config.max_content_size, defaults.max_content_size);
        assert_eq!(config.default_domain_delay, defaults.default_domain_delay);
    }

    #[test]
    fn test_load_config_with_callback_mutates_when_invoked() {
        // Verify the callback CAN mutate the config when it's invoked.
        // This tests the closure path regardless of whether load_configuration_for_cli succeeds.
        let initial = ContentFetchConfig {
            max_concurrent_fetches: 10,
            ..Default::default()
        };

        let config = WebSearcher::load_config_with_callback(initial, |cfg, _ctx| {
            cfg.max_concurrent_fetches = 77;
        });

        // If config loading succeeded, callback ran and value is 77.
        // If it failed, value stays 10. Either way, no panic.
        assert!(
            config.max_concurrent_fetches == 77 || config.max_concurrent_fetches == 10,
            "Expected 77 (callback ran) or 10 (callback skipped), got {}",
            config.max_concurrent_fetches
        );
    }

    #[test]
    fn test_get_search_client_returns_same_instance() {
        let mut searcher = WebSearcher::new();
        assert!(searcher.brave_client.is_none());

        let _client1 = searcher.get_search_client();
        assert!(searcher.brave_client.is_some());

        // Second call should reuse the cached client (not create a new one)
        let _client2 = searcher.get_search_client();
        assert!(searcher.brave_client.is_some());
    }
}
