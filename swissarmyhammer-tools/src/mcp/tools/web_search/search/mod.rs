//! Web search tool for MCP operations
//!
//! This module provides the WebSearchTool for performing web searches through the MCP protocol.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::web_search::instance_manager::{InstanceManager, InstanceManagerConfig};
use crate::mcp::tools::web_search::types::*;
use async_trait::async_trait;
use html2text::from_read;
use reqwest::Client;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::OnceCell;
use tracing::warn;
use url::Url;

/// Structured error types for web search operations
#[derive(Debug)]
enum WebSearchInternalError {
    /// Invalid or malformed request parameters
    InvalidRequest { message: String, parameter: Option<String> },
    /// Network or connectivity issues
    NetworkError { message: String, instance: String },
    /// SearXNG instance returned an error response
    InstanceError { message: String, instance: String, status_code: Option<u16> },
    /// Failed to parse response from SearXNG
    ParseError { message: String, instance: String },
    /// Content fetching failed for a specific URL
    ContentFetchError { message: String, url: String },
    /// All instances failed, no fallback available
    AllInstancesFailed { attempted_instances: Vec<String>, last_error: String },
}

impl std::fmt::Display for WebSearchInternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebSearchInternalError::InvalidRequest { message, parameter } => {
                if let Some(param) = parameter {
                    write!(f, "Invalid parameter '{param}': {message}")
                } else {
                    write!(f, "Invalid request: {message}")
                }
            }
            WebSearchInternalError::NetworkError { message, instance } => {
                write!(f, "Network error for instance '{instance}': {message}")
            }
            WebSearchInternalError::InstanceError { message, instance, status_code } => {
                if let Some(code) = status_code {
                    write!(f, "Instance '{instance}' returned error {code} : {message}")
                } else {
                    write!(f, "Instance '{instance}' error: {message}")
                }
            }
            WebSearchInternalError::ParseError { message, instance } => {
                write!(f, "Failed to parse response from '{instance}': {message}")
            }
            WebSearchInternalError::ContentFetchError { message, url } => {
                write!(f, "Failed to fetch content from '{url}': {message}")
            }
            WebSearchInternalError::AllInstancesFailed { attempted_instances, last_error } => {
                write!(
                    f,
                    "All {} instances failed. Last error: {}",
                    attempted_instances.len(),
                    last_error
                )
            }
        }
    }
}

impl std::error::Error for WebSearchInternalError {}

// Global instance manager - initialized lazily on first use
static INSTANCE_MANAGER: OnceCell<Arc<InstanceManager>> = OnceCell::const_new();

/// Tool for performing web searches using SearXNG
#[derive(Default)]
pub struct WebSearchTool {
    client: Option<Client>,
}

impl WebSearchTool {
    /// Creates a new instance of the WebSearchTool
    pub fn new() -> Self {
        Self { client: None }
    }

    /// Gets or creates an HTTP client with appropriate configuration
    fn get_client(&mut self) -> &Client {
        if self.client.is_none() {
            self.client = Some(
                Client::builder()
                    .timeout(Duration::from_secs(30))
                    .user_agent("SwissArmyHammer/1.0 (Privacy-Focused Web Search)")
                    .build()
                    .unwrap_or_else(|_| Client::new()),
            );
        }
        self.client.as_ref().unwrap()
    }

    /// Gets or initializes the global instance manager
    async fn get_instance_manager() -> &'static Arc<InstanceManager> {
        INSTANCE_MANAGER
            .get_or_init(|| async {
                // Load configuration for instance discovery
                let config = Self::load_instance_manager_config();
                let manager = InstanceManager::with_config(config).await;
                Arc::new(manager)
            })
            .await
    }

    /// Loads configuration for the instance manager
    fn load_instance_manager_config() -> InstanceManagerConfig {
        let mut config = InstanceManagerConfig::default();
        
        // Try to load from configuration
        if let Ok(Some(repo_config)) = swissarmyhammer::sah_config::load_repo_config_for_cli() {
            // Discovery settings
            if let Some(swissarmyhammer::ConfigValue::Boolean(enabled)) = 
                repo_config.get("web_search.discovery.enabled") {
                config.discovery_enabled = *enabled;
            }
            
            // Discovery refresh interval
            if let Some(swissarmyhammer::ConfigValue::Integer(interval)) = 
                repo_config.get("web_search.discovery.refresh_interval_seconds") {
                if *interval > 0 {
                    config.discovery_refresh_interval = Duration::from_secs(*interval as u64);
                }
            }
            
            // Health check interval
            if let Some(swissarmyhammer::ConfigValue::Integer(interval)) = 
                repo_config.get("web_search.discovery.health_check_interval_seconds") {
                if *interval > 0 {
                    config.health_check_interval = Duration::from_secs(*interval as u64);
                }
            }
            
            // Max consecutive failures
            if let Some(swissarmyhammer::ConfigValue::Integer(failures)) = 
                repo_config.get("web_search.discovery.max_consecutive_failures") {
                if *failures > 0 {
                    config.max_consecutive_failures = *failures as u32;
                }
            }
        }
        
        config
    }

    /// Performs a search using a SearXNG instance with comprehensive parameter support
    async fn perform_search(
        &mut self,
        instance: &str,
        request: &WebSearchRequest,
    ) -> Result<SearXngResponse, WebSearchInternalError> {
        // Validate the instance URL first
        let instance_url = Url::parse(instance).map_err(|e| WebSearchInternalError::InvalidRequest {
            message: format!("Invalid SearXNG instance URL '{instance}': {e}"),
            parameter: Some("instance_url".to_string()),
        })?;

        // Validate search query
        if request.query.trim().is_empty() {
            return Err(WebSearchInternalError::InvalidRequest {
                message: "Search query cannot be empty".to_string(),
                parameter: Some("query".to_string()),
            });
        }

        if request.query.len() > 500 {
            return Err(WebSearchInternalError::InvalidRequest {
                message: "Search query exceeds maximum length of 500 characters".to_string(),
                parameter: Some("query".to_string()),
            });
        }

        let client = self.get_client();

        // Construct search URL
        let mut url = instance_url.join("search").map_err(|e| WebSearchInternalError::InvalidRequest {
            message: format!("Failed to construct search URL for instance '{instance}': {e}"),
            parameter: Some("instance_url".to_string()),
        })?;

        // Build query parameters systematically
        {
            let mut query_pairs = url.query_pairs_mut();
            
            // Required parameters
            query_pairs
                .append_pair("q", &request.query)
                .append_pair("format", "json")
                .append_pair("pageno", "1");

            // Category parameter
            if let Some(category) = &request.category {
                let category_str = Self::category_to_string(category);
                query_pairs.append_pair("categories", category_str);
            }

            // Language parameter with validation
            if let Some(language) = &request.language {
                Self::validate_language_code(language)?;
                query_pairs.append_pair("language", language);
            }

            // Safe search parameter
            if let Some(safe_search) = request.safe_search {
                query_pairs.append_pair("safesearch", &(safe_search as u8).to_string());
            }

            // Time range parameter
            if let Some(time_range) = &request.time_range {
                if let Some(time_str) = Self::time_range_to_string(time_range) {
                    query_pairs.append_pair("time_range", time_str);
                }
            }

            // Results per page (SearXNG uses 'engines' for this in some configurations)
            // We'll handle result limiting during response parsing for consistency
        }

        tracing::debug!("Making search request to: {}", url);

        let response = client
            .get(url)
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    WebSearchInternalError::NetworkError {
                        message: "Request timeout (15 seconds)".to_string(),
                        instance: instance.to_string(),
                    }
                } else if e.is_connect() {
                    WebSearchInternalError::NetworkError {
                        message: format!("Connection failed: {e}"),
                        instance: instance.to_string(),
                    }
                } else {
                    WebSearchInternalError::NetworkError {
                        message: format!("Network error: {e}"),
                        instance: instance.to_string(),
                    }
                }
            })?;

        if !response.status().is_success() {
            return Err(WebSearchInternalError::InstanceError {
                message: "SearXNG instance returned non-success status".to_string(),
                instance: instance.to_string(),
                status_code: Some(response.status().as_u16()),
            });
        }

        let json: Value = response.json().await.map_err(|e| WebSearchInternalError::ParseError {
            message: format!("Failed to parse JSON response: {e}"),
            instance: instance.to_string(),
        })?;

        // Validate response structure
        if json.is_null() || !json.is_object() {
            return Err(WebSearchInternalError::ParseError {
                message: "Response is not a valid JSON object".to_string(),
                instance: instance.to_string(),
            });
        }

        // Parse search results with robust error handling
        let results_array = json["results"].as_array();
        let mut results = Vec::new();
        let mut engines_set = std::collections::HashSet::new();

        if let Some(results_array) = results_array {
            let max_results = request.results_count.unwrap_or(10);
            
            for (index, result_json) in results_array.iter().enumerate() {
                if index >= max_results {
                    break;
                }

                // Extract required fields with validation
                let title = result_json["title"].as_str()
                    .unwrap_or_else(|| {
                        tracing::warn!("Missing or invalid title in search result {}", index);
                        "Untitled"
                    });

                let url = match result_json["url"].as_str() {
                    Some(url) if !url.is_empty() => url,
                    _ => {
                        tracing::warn!("Missing or empty URL in search result {}, skipping", index);
                        continue; // Skip results without valid URLs
                    }
                };

                // Validate URL format
                if let Err(e) = Url::parse(url) {
                    tracing::warn!("Invalid URL in search result {}: {} - {}", index, url, e);
                    continue; // Skip results with invalid URLs
                }

                let description = result_json["content"].as_str()
                    .or_else(|| result_json["description"].as_str()) // Try alternate field name
                    .unwrap_or("")
                    .to_string();

                let engine = result_json["engine"].as_str()
                    .unwrap_or("unknown")
                    .to_string();

                // Collect engines for metadata
                engines_set.insert(engine.clone());

                // Extract score if available (some SearXNG instances provide it)
                let score = result_json["score"].as_f64()
                    .unwrap_or(1.0) // Default score when not provided
                    .clamp(0.0, 1.0); // Clamp between 0 and 1

                results.push(SearchResult {
                    title: title.to_string(),
                    url: url.to_string(),
                    description,
                    score,
                    engine,
                    content: None, // Will be populated later if fetch_content is true
                });
            }
        } else {
            tracing::warn!("No results array found in SearXNG response");
        }

        // Extract total results count with fallback
        let total_results = json["number_of_results"]
            .as_u64()
            .or_else(|| json["total_results"].as_u64()) // Try alternate field name
            .unwrap_or(results.len() as u64) as usize;

        let engines_used: Vec<String> = engines_set.into_iter().collect();

        tracing::debug!(
            "Parsed SearXNG response: {} results, {} engines used",
            results.len(),
            engines_used.len()
        );

        Ok(SearXngResponse {
            results,
            engines_used,
            total_results,
        })
    }

    /// Converts SearchCategory enum to SearXNG category string
    fn category_to_string(category: &SearchCategory) -> &'static str {
        match category {
            SearchCategory::General => "general",
            SearchCategory::Images => "images",
            SearchCategory::Videos => "videos", 
            SearchCategory::News => "news",
            SearchCategory::Map => "map",
            SearchCategory::Music => "music",
            SearchCategory::It => "it",
            SearchCategory::Science => "science",
            SearchCategory::Files => "files",
        }
    }

    /// Converts TimeRange enum to SearXNG time range string
    /// Returns None for TimeRange::All as it should not be included in the query
    fn time_range_to_string(time_range: &TimeRange) -> Option<&'static str> {
        match time_range {
            TimeRange::All => None, // Don't include parameter for all time
            TimeRange::Day => Some("day"),
            TimeRange::Week => Some("week"),
            TimeRange::Month => Some("month"),
            TimeRange::Year => Some("year"),
        }
    }

    /// Validates language code format (ISO 639-1 with optional country code)
    fn validate_language_code(language: &str) -> Result<(), WebSearchInternalError> {
        let re = regex::Regex::new(r"^[a-z]{2}(-[A-Z]{2})?$").map_err(|e| {
            WebSearchInternalError::InvalidRequest {
                message: format!("Failed to compile language regex: {e}"),
                parameter: Some("language".to_string()),
            }
        })?;
        
        if !re.is_match(language) {
            return Err(WebSearchInternalError::InvalidRequest {
                message: format!(
                    "Invalid language code '{language}'. Expected format: 'en' or 'en-US'"
                ),
                parameter: Some("language".to_string()),
            });
        }
        
        Ok(())
    }

    /// Validates all request parameters comprehensively
    fn validate_request(request: &WebSearchRequest) -> Result<(), WebSearchInternalError> {
        // Query validation
        if request.query.trim().is_empty() {
            return Err(WebSearchInternalError::InvalidRequest {
                message: "Search query cannot be empty".to_string(),
                parameter: Some("query".to_string()),
            });
        }

        if request.query.len() > 500 {
            return Err(WebSearchInternalError::InvalidRequest {
                message: format!("Search query is {} characters, maximum is 500", request.query.len()),
                parameter: Some("query".to_string()),
            });
        }

        // Language validation if provided
        if let Some(language) = &request.language {
            Self::validate_language_code(language)?;
        }

        // Results count validation
        if let Some(count) = request.results_count {
            if count == 0 {
                return Err(WebSearchInternalError::InvalidRequest {
                    message: "Results count must be at least 1".to_string(),
                    parameter: Some("results_count".to_string()),
                });
            }
            if count > 50 {
                return Err(WebSearchInternalError::InvalidRequest {
                    message: format!("Results count is {count}, maximum is 50"),
                    parameter: Some("results_count".to_string()),
                });
            }
        }

        // Safe search validation (enum ensures valid values, but let's be explicit)
        if let Some(safe_search) = request.safe_search {
            let level = safe_search as u8;
            if level > 2 {
                return Err(WebSearchInternalError::InvalidRequest {
                    message: format!("Safe search level {level} is invalid, must be 0, 1, or 2"),
                    parameter: Some("safe_search".to_string()),
                });
            }
        }

        Ok(())
    }

    /// Fetches content from a URL and converts it to markdown
    async fn fetch_content(
        &mut self,
        url: &str,
    ) -> Result<SearchResultContent, WebSearchInternalError> {
        // Validate URL first
        let _parsed_url = Url::parse(url).map_err(|e| WebSearchInternalError::ContentFetchError {
            message: format!("Invalid URL format: {e}"),
            url: url.to_string(),
        })?;

        let client = self.get_client();
        let start_time = Instant::now();

        // Perform content fetch with proper error handling
        let response = client
            .get(url)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    WebSearchInternalError::ContentFetchError {
                        message: "Request timeout (10 seconds)".to_string(),
                        url: url.to_string(),
                    }
                } else if e.is_connect() {
                    WebSearchInternalError::ContentFetchError {
                        message: format!("Connection failed: {e}"),
                        url: url.to_string(),
                    }
                } else {
                    WebSearchInternalError::ContentFetchError {
                        message: format!("Network error: {e}"),
                        url: url.to_string(),
                    }
                }
            })?;

        if !response.status().is_success() {
            return Err(WebSearchInternalError::ContentFetchError {
                message: format!("HTTP error {}: {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("Unknown")),
                url: url.to_string(),
            });
        }

        let html = response.text().await.map_err(|e| WebSearchInternalError::ContentFetchError {
            message: format!("Failed to read response body: {e}"),
            url: url.to_string(),
        })?;
        let fetch_time = start_time.elapsed();

        // Convert HTML to text using html2text for proper formatting
        let text = from_read(html.as_bytes(), 80); // 80 character line width

        let word_count = text.split_whitespace().count();
        let summary = if word_count > 50 {
            text.split_whitespace()
                .take(50)
                .collect::<Vec<_>>()
                .join(" ")
                + "..."
        } else {
            text.clone()
        };

        Ok(SearchResultContent {
            markdown: text,
            word_count,
            fetch_time_ms: fetch_time.as_millis() as u64,
            summary,
        })
    }
}

/// Response from SearXNG API
struct SearXngResponse {
    results: Vec<SearchResult>,
    engines_used: Vec<String>,
    total_results: usize,
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
            return Err(McpError::invalid_request(
                validation_error.to_string(),
                None,
            ));
        }

        let start_time = Instant::now();
        let mut search_tool = WebSearchTool::new();

        // Get instance manager and try instances until one works
        let instance_manager = Self::get_instance_manager().await;
        let max_attempts = 3; // Try up to 3 different instances
        let mut attempted_instances = Vec::new();
        let mut last_error = None;

        for attempt in 0..max_attempts {
            if let Some(instance) = instance_manager.get_next_instance().await {
                attempted_instances.push(instance.url.clone());
                
                match search_tool.perform_search(&instance.url, &request).await {
                    Ok(mut searxng_response) => {
                        let search_time = start_time.elapsed();

                        // Optionally fetch content from each result
                        let mut content_fetch_stats = None;

                        if request.fetch_content.unwrap_or(true) {
                            let content_start = Instant::now();
                            let mut successful = 0;
                            let mut failed = 0;

                            for result in &mut searxng_response.results {
                                match search_tool.fetch_content(&result.url).await {
                                    Ok(content) => {
                                        result.content = Some(content);
                                        successful += 1;
                                    }
                                    Err(e) => {
                                        tracing::warn!(
                                            "Failed to fetch content from {}: {}",
                                            result.url,
                                            e
                                        );
                                        failed += 1;
                                    }
                                }
                            }

                            content_fetch_stats = Some(ContentFetchStats {
                                attempted: searxng_response.results.len(),
                                successful,
                                failed,
                                total_time_ms: content_start.elapsed().as_millis() as u64,
                            });
                        }

                        let response = WebSearchResponse {
                            results: searxng_response.results,
                            metadata: SearchMetadata {
                                query: request.query.clone(),
                                category: request.category.unwrap_or_default(),
                                language: request.language.unwrap_or_else(|| "en".to_string()),
                                results_count: request.results_count.unwrap_or(10),
                                search_time_ms: search_time.as_millis() as u64,
                                instance_used: instance.url.clone(),
                                total_results: searxng_response.total_results,
                                engines_used: searxng_response.engines_used,
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

                        return Ok(BaseToolImpl::create_success_response(
                            serde_json::to_string_pretty(&response).map_err(|e| {
                                McpError::internal_error(
                                    format!("Failed to serialize response: {e}"),
                                    None,
                                )
                            })?,
                        ));
                    }
                    Err(e) => {
                        tracing::warn!("Search failed on instance {}: {}", instance.url, e);
                        
                        // Mark instance as failed for health tracking
                        instance_manager.mark_instance_failed(&instance.url).await;
                        
                        // Check if it's a rate limit error and handle appropriately
                        if e.to_string().contains("rate limit") || e.to_string().contains("429") {
                            instance_manager.mark_instance_rate_limited(&instance.url, Duration::from_secs(300)).await;
                        }
                        
                        last_error = Some(e.to_string());
                        continue;
                    }
                }
            } else {
                // No healthy instances available
                warn!("No healthy instances available on attempt {}", attempt + 1);
                break;
            }
        }

        // All instances failed - create structured error
        let all_failed_error = WebSearchInternalError::AllInstancesFailed {
            attempted_instances: attempted_instances.clone(),
            last_error: last_error.unwrap_or_else(|| "Unknown error".to_string()),
        };

        let error = WebSearchError {
            error_type: "no_instances_available".to_string(),
            error_details: all_failed_error.to_string(),
            attempted_instances,
            retry_after: Some(300), // Suggest retry after 5 minutes
        };

        Err(McpError::internal_error(
            serde_json::to_string_pretty(&error).unwrap_or_else(|_| "Search failed".to_string()),
            None,
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
    fn test_load_instance_manager_config() {
        let config = WebSearchTool::load_instance_manager_config();
        
        // Should have default values when no config is present
        assert!(config.discovery_enabled);
        assert_eq!(config.discovery_refresh_interval, Duration::from_secs(3600));
        assert_eq!(config.health_check_interval, Duration::from_secs(300));
        assert_eq!(config.max_consecutive_failures, 3);
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
    fn test_category_to_string() {
        assert_eq!(WebSearchTool::category_to_string(&SearchCategory::General), "general");
        assert_eq!(WebSearchTool::category_to_string(&SearchCategory::Images), "images");
        assert_eq!(WebSearchTool::category_to_string(&SearchCategory::Videos), "videos");
        assert_eq!(WebSearchTool::category_to_string(&SearchCategory::News), "news");
        assert_eq!(WebSearchTool::category_to_string(&SearchCategory::Map), "map");
        assert_eq!(WebSearchTool::category_to_string(&SearchCategory::Music), "music");
        assert_eq!(WebSearchTool::category_to_string(&SearchCategory::It), "it");
        assert_eq!(WebSearchTool::category_to_string(&SearchCategory::Science), "science");
        assert_eq!(WebSearchTool::category_to_string(&SearchCategory::Files), "files");
    }

    #[test]
    fn test_time_range_to_string() {
        assert_eq!(WebSearchTool::time_range_to_string(&TimeRange::All), None);
        assert_eq!(WebSearchTool::time_range_to_string(&TimeRange::Day), Some("day"));
        assert_eq!(WebSearchTool::time_range_to_string(&TimeRange::Week), Some("week"));
        assert_eq!(WebSearchTool::time_range_to_string(&TimeRange::Month), Some("month"));
        assert_eq!(WebSearchTool::time_range_to_string(&TimeRange::Year), Some("year"));
    }

    #[test]
    fn test_validate_language_code_success() {
        assert!(WebSearchTool::validate_language_code("en").is_ok());
        assert!(WebSearchTool::validate_language_code("fr").is_ok());
        assert!(WebSearchTool::validate_language_code("en-US").is_ok());
        assert!(WebSearchTool::validate_language_code("fr-CA").is_ok());
    }

    #[test]
    fn test_validate_language_code_failure() {
        assert!(WebSearchTool::validate_language_code("e").is_err());
        assert!(WebSearchTool::validate_language_code("english").is_err());
        assert!(WebSearchTool::validate_language_code("en-us").is_err()); // lowercase country
        assert!(WebSearchTool::validate_language_code("EN").is_err()); // uppercase language
        assert!(WebSearchTool::validate_language_code("123").is_err());
        assert!(WebSearchTool::validate_language_code("").is_err());
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
        assert!(result.unwrap_err().to_string().contains("Invalid language code"));
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
        assert!(result.unwrap_err().to_string().contains("must be at least 1"));

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
        assert!(result_high.unwrap_err().to_string().contains("maximum is 50"));
    }

    #[test]
    fn test_web_search_internal_error_display() {
        let error = WebSearchInternalError::InvalidRequest {
            message: "Test error".to_string(),
            parameter: Some("test_param".to_string()),
        };
        assert_eq!(error.to_string(), "Invalid parameter 'test_param': Test error");

        let error2 = WebSearchInternalError::NetworkError {
            message: "Connection failed".to_string(),
            instance: "https://example.com".to_string(),
        };
        assert_eq!(error2.to_string(), "Network error for instance 'https://example.com': Connection failed");
    }
}
