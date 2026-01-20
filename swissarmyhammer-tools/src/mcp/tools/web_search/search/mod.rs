//! Web search tool for MCP operations
//!
//! This module provides the WebSearchTool for performing web searches using DuckDuckGo's
//! official Instant Answer API through the MCP protocol. For comprehensive web search results,
//! consider configuring third-party APIs.

use crate::mcp::progress_notifications::generate_progress_token;
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::web_search::chrome_detection;
use crate::mcp::tools::web_search::content_fetcher::{ContentFetchConfig, ContentFetcher};
use crate::mcp::tools::web_search::duckduckgo_client::{DuckDuckGoClient, DuckDuckGoError};
use crate::mcp::tools::web_search::types::ScoringConfig;
use crate::mcp::tools::web_search::types::*;
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use std::time::{Duration, Instant};
use swissarmyhammer_common::health::{Doctorable, HealthCheck};

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
        F: FnOnce(&mut T, &swissarmyhammer_config::TemplateContext),
    {
        if let Ok(template_context) = swissarmyhammer_config::load_configuration_for_cli() {
            configure_fn(&mut config, &template_context);
        }
        config
    }

    /// Loads configuration for content fetching
    fn load_content_fetch_config() -> ContentFetchConfig {
        Self::load_config_with_callback(
            ContentFetchConfig::default(),
            |config, template_context| {
                // Concurrent processing settings
                if let Some(serde_json::Value::Number(max_concurrent)) =
                    template_context.get("web_search.content_fetching.max_concurrent_fetches")
                {
                    if let Some(max_concurrent) = max_concurrent.as_i64() {
                        if max_concurrent > 0 {
                            config.max_concurrent_fetches = max_concurrent as usize;
                        }
                    }
                }

                // Timeout settings
                if let Some(serde_json::Value::Number(timeout)) =
                    template_context.get("web_search.content_fetching.content_fetch_timeout")
                {
                    if let Some(timeout) = timeout.as_i64() {
                        if timeout > 0 {
                            config.fetch_timeout = Duration::from_secs(timeout as u64);
                        }
                    }
                }

                // Content size limit
                if let Some(serde_json::Value::String(size_str)) =
                    template_context.get("web_search.content_fetching.max_content_size")
                {
                    if let Ok(size) = Self::parse_size_string(size_str) {
                        config.max_content_size = size;
                    }
                }

                // Rate limiting settings
                if let Some(serde_json::Value::Number(delay)) =
                    template_context.get("web_search.content_fetching.default_domain_delay")
                {
                    if let Some(delay) = delay.as_i64() {
                        if delay > 0 {
                            config.default_domain_delay = Duration::from_millis(delay as u64);
                        }
                    }
                }

                // Content quality settings
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

                // Processing settings
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
            // Scoring algorithm configuration
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
        let progress_token = generate_progress_token();

        // Send start notification (0%)
        if let Some(sender) = &_context.progress_sender {
            sender
                .send_progress_with_metadata(
                    &progress_token,
                    Some(0),
                    format!("Web search: 0/3 - Searching for: {}", request.query),
                    json!({
                        "query": request.query,
                        "results_count": request.results_count,
                        "fetch_content": request.fetch_content,
                        "current": 0,
                        "total": 3
                    }),
                )
                .ok();
        }

        let mut search_tool = WebSearchTool::new();

        // Send search progress notification (25%)
        if let Some(sender) = &_context.progress_sender {
            sender
                .send_progress_with_metadata(
                    &progress_token,
                    Some(25),
                    "Web search: 1/3 - Performing search...",
                    json!({
                        "current": 1,
                        "total": 3
                    }),
                )
                .ok();
        }

        // Perform search using DuckDuckGo browser automation
        let duckduckgo_client = search_tool.get_duckduckgo_client();
        let mut results = match duckduckgo_client.search(&request).await {
            Ok(results) => results,
            Err(DuckDuckGoError::NoResults) => {
                // Send error notification
                if let Some(sender) = &_context.progress_sender {
                    sender
                        .send_progress_with_metadata(
                            &progress_token,
                            None,
                            "Web search: Failed - No results found",
                            json!({
                                "error": "no_results",
                                "query": request.query
                            }),
                        )
                        .ok();
                }

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
                // Send error notification
                if let Some(sender) = &_context.progress_sender {
                    sender
                        .send_progress_with_metadata(
                            &progress_token,
                            None,
                            format!("Web search: Failed - {}", e),
                            json!({
                                "error": "search_failed",
                                "details": e.to_string(),
                                "query": request.query
                            }),
                        )
                        .ok();
                }

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

        // Send results retrieved notification
        // If fetch_content is false, we're almost done (90%), otherwise we're at 40%
        let progress_after_search = if request.fetch_content.unwrap_or(true) {
            40
        } else {
            90
        };

        if let Some(sender) = &_context.progress_sender {
            sender
                .send_progress_with_metadata(
                    &progress_token,
                    Some(progress_after_search),
                    format!("Web search: 2/3 - Retrieved {} results", results.len()),
                    json!({
                        "results_count": results.len(),
                        "current": 2,
                        "total": 3
                    }),
                )
                .ok();
        }

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

        // Calculate with_content before moving content_fetch_stats
        let with_content = content_fetch_stats
            .as_ref()
            .map(|s| s.successful)
            .unwrap_or(0);

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

        // Send completion notification (100%)
        if let Some(sender) = &_context.progress_sender {
            sender
                .send_progress_with_metadata(
                    &progress_token,
                    Some(100),
                    format!(
                        "Web search: 3/3 - Complete ({} results)",
                        response.results.len()
                    ),
                    json!({
                        "total_results": response.results.len(),
                        "with_content": with_content,
                        "search_time_ms": search_time.as_millis() as u64,
                        "current": 3,
                        "total": 3
                    }),
                )
                .ok();
        }

        Ok(BaseToolImpl::create_success_response(
            serde_json::to_string_pretty(&response).map_err(|e| {
                McpError::internal_error(format!("Failed to serialize response: {e}"), None)
            })?,
        ))
    }
}

impl Doctorable for WebSearchTool {
    fn name(&self) -> &str {
        "Web Search"
    }

    fn category(&self) -> &str {
        "tools"
    }

    fn run_health_checks(&self) -> Vec<HealthCheck> {
        let mut checks = Vec::new();

        // Check Chrome/Chromium availability
        let chrome_result = chrome_detection::detect_chrome();

        if chrome_result.found {
            let path = chrome_result.path.as_ref().unwrap();
            let method = chrome_result.detection_method.as_ref().unwrap();

            checks.push(HealthCheck::ok(
                "Chrome/Chromium Browser",
                format!("Found at {} (via {})", path.display(), method),
                self.category(),
            ));
        } else {
            let instructions = chrome_result.installation_instructions();

            checks.push(HealthCheck::warning(
                "Chrome/Chromium Browser",
                format!(
                    "Not found (required for web_search)\nChecked {} locations",
                    chrome_result.paths_checked.len()
                ),
                Some(format!("Install Chrome/Chromium:\n{}", instructions)),
                self.category(),
            ));
        }

        checks
    }

    fn is_applicable(&self) -> bool {
        // Web search is always applicable - it's a core feature
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_context;

    #[test]
    fn test_web_search_tool_new() {
        let tool = WebSearchTool::new();
        assert_eq!(<WebSearchTool as crate::mcp::tool_registry::McpTool>::name(&tool), "web_search");
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

    #[tokio::test]
    async fn test_web_search_sends_progress_notifications() {
        use crate::mcp::progress_notifications::ProgressSender;
        use tokio::sync::mpsc;

        let (tx, mut rx) = mpsc::unbounded_channel();
        let progress_sender = ProgressSender::new(tx);

        let mut context = create_test_context().await;
        context.progress_sender = Some(progress_sender);

        let tool = WebSearchTool::new();
        let mut args = serde_json::Map::new();
        args.insert(
            "query".to_string(),
            serde_json::Value::String("rust".to_string()),
        );
        args.insert(
            "results_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(1)),
        );
        args.insert("fetch_content".to_string(), serde_json::Value::Bool(false));

        // Execute the search
        let result = tool.execute(args, &context).await;

        // Collect all notifications
        let mut notifications = Vec::new();
        while let Ok(notif) = rx.try_recv() {
            notifications.push(notif);
        }

        // If search failed due to missing Chrome, skip the test with clear message
        if let Err(ref e) = result {
            let error_text = e.to_string();
            if error_text.contains("Chrome/Chromium browser not found") {
                eprintln!("Skipping test: {}", error_text);
                return;
            }
        }

        // The test should verify notifications are sent
        if !notifications.is_empty() {
            // First notification should be the start notification with 0% progress
            assert_eq!(
                notifications[0].progress,
                Some(0),
                "First notification should be start with 0% progress"
            );
            assert!(
                notifications[0].message.contains("Searching for:"),
                "Start notification should contain query"
            );

            // Last notification should have higher progress (90-100%)
            let last = notifications.last().unwrap();
            assert!(
                last.progress.unwrap_or(0) >= 90,
                "Last notification should have progress >= 90%"
            );

            // Should have at least start and one other notification
            assert!(
                notifications.len() >= 2,
                "Expected at least 2 notifications (start and progress/completion), got {}",
                notifications.len()
            );
        }

        // Verify result structure
        let call_result = result.expect("Search should succeed when Chrome is available");
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }
}
