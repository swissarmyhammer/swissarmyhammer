//! Web search tool for MCP operations
//!
//! This module provides the WebSearchTool for performing web searches using DuckDuckGo's
//! official Instant Answer API through the MCP protocol. For comprehensive web search results,
//! consider configuring third-party APIs.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::web_search::content_fetcher::{ContentFetchConfig, ContentFetcher};
use crate::mcp::tools::web_search::duckduckgo_client::{DuckDuckGoClient, DuckDuckGoError};
use crate::mcp::tools::web_search::types::ScoringConfig;
use crate::mcp::tools::web_search::types::*;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use std::time::{Duration, Instant};

/// Tool for performing web searches using DuckDuckGo web scraping
#[derive(Default)]
pub struct WebSearchTool {
    duckduckgo_client: Option<DuckDuckGoClient>,
}

impl WebSearchTool {
    /// Creates a new instance of the WebSearchTool
    pub fn new() -> Self {
        Self {
            duckduckgo_client: None,
        }
    }

    /// Gets or creates a DuckDuckGo web search client
    fn get_duckduckgo_client(&mut self) -> &mut DuckDuckGoClient {
        if self.duckduckgo_client.is_none() {
            let config = Self::load_scoring_config();
            self.duckduckgo_client = Some(DuckDuckGoClient::with_scoring_config(config));
        }
        self.duckduckgo_client.as_mut().unwrap()
    }

    /// Helper function to load configuration with a callback for setting values
    fn load_config_with_callback<T, F>(mut config: T, configure_fn: F) -> T
    where
        F: FnOnce(&mut T, &swissarmyhammer::Configuration),
    {
        if let Ok(Some(repo_config)) = swissarmyhammer::sah_config::load_repo_config_for_cli() {
            configure_fn(&mut config, &repo_config);
        }
        config
    }

    /// Loads configuration for content fetching
    fn load_content_fetch_config() -> ContentFetchConfig {
        Self::load_config_with_callback(ContentFetchConfig::default(), |config, repo_config| {
            // Concurrent processing settings
            if let Some(swissarmyhammer::ConfigValue::Integer(max_concurrent)) =
                repo_config.get("web_search.content_fetching.max_concurrent_fetches")
            {
                if *max_concurrent > 0 {
                    config.max_concurrent_fetches = *max_concurrent as usize;
                }
            }

            // Timeout settings
            if let Some(swissarmyhammer::ConfigValue::Integer(timeout)) =
                repo_config.get("web_search.content_fetching.content_fetch_timeout")
            {
                if *timeout > 0 {
                    config.fetch_timeout = Duration::from_secs(*timeout as u64);
                }
            }

            // Content size limit
            if let Some(swissarmyhammer::ConfigValue::String(size_str)) =
                repo_config.get("web_search.content_fetching.max_content_size")
            {
                if let Ok(size) = Self::parse_size_string(size_str) {
                    config.max_content_size = size;
                }
            }

            // Rate limiting settings
            if let Some(swissarmyhammer::ConfigValue::Integer(delay)) =
                repo_config.get("web_search.content_fetching.default_domain_delay")
            {
                if *delay > 0 {
                    config.default_domain_delay = Duration::from_millis(*delay as u64);
                }
            }

            // Content quality settings
            if let Some(swissarmyhammer::ConfigValue::Integer(min_length)) =
                repo_config.get("web_search.content_fetching.min_content_length")
            {
                if *min_length > 0 {
                    config.quality_config.min_content_length = *min_length as usize;
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(max_length)) =
                repo_config.get("web_search.content_fetching.max_content_length")
            {
                if *max_length > 0 {
                    config.quality_config.max_content_length = *max_length as usize;
                }
            }

            // Processing settings
            if let Some(swissarmyhammer::ConfigValue::Integer(max_summary)) =
                repo_config.get("web_search.content_fetching.max_summary_length")
            {
                if *max_summary > 0 {
                    config.processing_config.max_summary_length = *max_summary as usize;
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Boolean(extract_code)) =
                repo_config.get("web_search.content_fetching.extract_code_blocks")
            {
                config.processing_config.extract_code_blocks = *extract_code;
            }

            if let Some(swissarmyhammer::ConfigValue::Boolean(generate_summaries)) =
                repo_config.get("web_search.content_fetching.generate_summaries")
            {
                config.processing_config.generate_summaries = *generate_summaries;
            }

            if let Some(swissarmyhammer::ConfigValue::Boolean(extract_metadata)) =
                repo_config.get("web_search.content_fetching.extract_metadata")
            {
                config.processing_config.extract_metadata = *extract_metadata;
            }
        })
    }

    /// Loads configuration for DuckDuckGo scoring algorithm
    fn load_scoring_config() -> ScoringConfig {
        Self::load_config_with_callback(ScoringConfig::default(), |config, repo_config| {
            // Scoring algorithm configuration
            if let Some(swissarmyhammer::ConfigValue::Float(base_score)) =
                repo_config.get("web_search.scoring.base_score")
            {
                config.base_score = *base_score;
            }

            if let Some(swissarmyhammer::ConfigValue::Float(position_penalty)) =
                repo_config.get("web_search.scoring.position_penalty")
            {
                config.position_penalty = *position_penalty;
            }

            if let Some(swissarmyhammer::ConfigValue::Float(min_score)) =
                repo_config.get("web_search.scoring.min_score")
            {
                config.min_score = *min_score;
            }

            if let Some(swissarmyhammer::ConfigValue::Boolean(exponential_decay)) =
                repo_config.get("web_search.scoring.exponential_decay")
            {
                config.exponential_decay = *exponential_decay;
            }

            if let Some(swissarmyhammer::ConfigValue::Float(decay_rate)) =
                repo_config.get("web_search.scoring.decay_rate")
            {
                config.decay_rate = *decay_rate;
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
    fn validate_request(request: &WebSearchRequest) -> Result<(), String> {
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

#[async_trait]
impl McpTool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(WebSearchRequest))
            .expect("Failed to generate schema")
    }

    fn cli_category(&self) -> Option<&'static str> {
        Some("web-search")
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: WebSearchRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::info!(
            "Starting web search: '{}', results_count: {:?}, fetch_content: {:?}",
            request.query,
            request.results_count,
            request.fetch_content
        );

        // Comprehensive parameter validation
        if let Err(validation_error) = Self::validate_request(&request) {
            return Err(McpError::invalid_request(validation_error, None));
        }

        let start_time = Instant::now();
        let mut search_tool = WebSearchTool::new();

        // Perform search using DuckDuckGo browser automation
        let duckduckgo_client = search_tool.get_duckduckgo_client();
        let mut results = match duckduckgo_client.search(&request).await {
            Ok(results) => results,
            Err(DuckDuckGoError::NoResults) => {
                // No web results found - provide informative response
                let error = WebSearchError {
                    error_type: "no_results".to_string(),
                    error_details: format!(
                        "No web search results found for '{}'. The search may be too specific or the terms may not match any web pages.",
                        request.query
                    ),
                    attempted_instances: vec!["https://duckduckgo.com".to_string()],
                    retry_after: None,
                };

                return Err(McpError::invalid_request(
                    serde_json::to_string_pretty(&error)
                        .unwrap_or_else(|_| "No results found".to_string()),
                    None,
                ));
            }
            Err(e) => {
                let error = WebSearchError {
                    error_type: "search_failed".to_string(),
                    error_details: format!("DuckDuckGo web search failed: {e}"),
                    attempted_instances: vec!["https://duckduckgo.com".to_string()],
                    retry_after: Some(10), // Suggest retry after 10 seconds for general issues
                };

                return Err(McpError::internal_error(
                    serde_json::to_string_pretty(&error)
                        .unwrap_or_else(|_| "Search failed".to_string()),
                    None,
                ));
            }
        };

        let search_time = start_time.elapsed();

        // Optionally fetch content from each result using the ContentFetcher
        let mut content_fetch_stats = None;

        if request.fetch_content.unwrap_or(true) {
            let content_config = Self::load_content_fetch_config();
            let content_fetcher = ContentFetcher::new(content_config);

            let (processed_results, stats) = content_fetcher.fetch_search_results(results).await;

            results = processed_results;

            content_fetch_stats = Some(ContentFetchStats {
                attempted: stats.attempted,
                successful: stats.successful,
                failed: stats.failed,
                total_time_ms: stats.total_time_ms,
            });
        }

        let response = WebSearchResponse {
            results: results.clone(),
            metadata: SearchMetadata {
                query: request.query.clone(),
                category: request.category.unwrap_or_default(),
                language: request.language.unwrap_or_else(|| "en".to_string()),
                results_count: results.len(),
                search_time_ms: search_time.as_millis() as u64,
                instance_used: "https://duckduckgo.com".to_string(),
                total_results: results.len(),
                engines_used: vec!["duckduckgo".to_string()],
                content_fetch_stats,
                fetch_content: request.fetch_content.unwrap_or(true),
            },
        };

        tracing::info!(
            "Web search completed: found {} results for '{}' in {:?}",
            response.results.len(),
            response.metadata.query,
            search_time
        );

        Ok(BaseToolImpl::create_success_response(
            serde_json::to_string_pretty(&response).map_err(|e| {
                McpError::internal_error(format!("Failed to serialize response: {e}"), None)
            })?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;

    #[test]
    fn test_web_search_tool_new() {
        let tool = WebSearchTool::new();
        assert_eq!(tool.name(), "web_search");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_web_search_tool_schema() {
        let tool = WebSearchTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["properties"]["results_count"].is_object());
        assert!(schema["properties"]["category"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["query"]));
    }

    #[tokio::test]
    async fn test_web_search_tool_execute_empty_query() {
        let tool = WebSearchTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_web_search_tool_execute_missing_query() {
        let tool = WebSearchTool::new();
        let context = create_test_context().await;

        let arguments = serde_json::Map::new(); // Missing query field

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_web_search_request_parsing() {
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("test query".to_string()),
        );
        arguments.insert(
            "results_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(5)),
        );
        arguments.insert("fetch_content".to_string(), serde_json::Value::Bool(false));

        let request: WebSearchRequest = BaseToolImpl::parse_arguments(arguments).unwrap();
        assert_eq!(request.query, "test query");
        assert_eq!(request.results_count, Some(5));
        assert_eq!(request.fetch_content, Some(false));
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
        assert!(WebSearchTool::validate_request(&request).is_ok());
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
        let result = WebSearchTool::validate_request(&request);
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
        let result = WebSearchTool::validate_request(&request);
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
        let result = WebSearchTool::validate_request(&request);
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
        let result = WebSearchTool::validate_request(&request);
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
        let result_high = WebSearchTool::validate_request(&request_high);
        assert!(result_high.is_err());
        assert!(result_high
            .unwrap_err()
            .to_string()
            .contains("maximum is 50"));
    }
}
