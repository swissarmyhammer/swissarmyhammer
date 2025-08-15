//! Enhanced web search tool with comprehensive error handling and recovery
//!
//! This module provides an enhanced WebSearchTool that incorporates the comprehensive
//! error handling, circuit breaker, and graceful degradation features defined in
//! WEB_SEARCH_000007_error_recovery.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use crate::mcp::tools::web_search::content_fetcher::{ContentFetchConfig, ContentFetcher};
use crate::mcp::tools::web_search::error_recovery::{
    CircuitBreakerConfig, ContentFetchError, EnhancedSearchMetadata, 
    EnhancedWebSearchResponse, FailoverManager, RetryConfig, SearchResultBuilder,
    WebSearchError as RecoveryWebSearchError,
};
use crate::mcp::tools::web_search::instance_manager::{InstanceManager, InstanceManagerConfig};
use crate::mcp::tools::web_search::privacy::{PrivacyConfig, PrivacyManager};
use crate::mcp::tools::web_search::types::{
    ContentFetchStats, SearchCategory, SearchMetadata, SearchResult, 
    TimeRange, WebSearchRequest
};
use async_trait::async_trait;
use reqwest::Client;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::OnceCell;
use url::Url;

// Global instance manager - initialized lazily on first use
static INSTANCE_MANAGER: OnceCell<Arc<InstanceManager>> = OnceCell::const_new();

/// Enhanced web search tool with comprehensive error handling and recovery
#[derive(Default)]
pub struct EnhancedWebSearchTool {
    client: Option<Client>,
}

impl EnhancedWebSearchTool {
    /// Creates a new instance of the Enhanced WebSearchTool
    pub fn new() -> Self {
        Self {
            client: None,
        }
    }

    /// Gets or creates an HTTP client with appropriate configuration
    fn get_client(&mut self) -> &Client {
        if self.client.is_none() {
            self.client = Some(
                Client::builder()
                    .timeout(Duration::from_secs(30))
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
                let config = Self::load_instance_manager_config();
                let manager = InstanceManager::with_config(config).await;
                Arc::new(manager)
            })
            .await
    }

    /// Load retry configuration from settings
    fn load_retry_config() -> RetryConfig {
        let mut config = RetryConfig::default();

        if let Ok(Some(repo_config)) = swissarmyhammer::sah_config::load_repo_config_for_cli() {
            if let Some(swissarmyhammer::ConfigValue::Integer(max_retries)) =
                repo_config.get("web_search.error_handling.max_retries")
            {
                if *max_retries > 0 {
                    config.max_retries = *max_retries as u32;
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(base_delay)) =
                repo_config.get("web_search.error_handling.base_retry_delay")
            {
                if *base_delay > 0 {
                    config.base_delay = Duration::from_millis(*base_delay as u64);
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(max_delay)) =
                repo_config.get("web_search.error_handling.max_retry_delay")
            {
                if *max_delay > 0 {
                    config.max_delay = Duration::from_millis(*max_delay as u64);
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Float(multiplier)) =
                repo_config.get("web_search.error_handling.backoff_multiplier")
            {
                if *multiplier > 0.0 {
                    config.backoff_multiplier = *multiplier;
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Boolean(jitter)) =
                repo_config.get("web_search.error_handling.enable_jitter")
            {
                config.jitter = *jitter;
            }
        }

        config
    }

    /// Load circuit breaker configuration from settings
    fn load_circuit_breaker_config() -> CircuitBreakerConfig {
        let mut config = CircuitBreakerConfig::default();

        if let Ok(Some(repo_config)) = swissarmyhammer::sah_config::load_repo_config_for_cli() {
            if let Some(swissarmyhammer::ConfigValue::Integer(threshold)) =
                repo_config.get("web_search.error_handling.circuit_breaker_failure_threshold")
            {
                if *threshold > 0 {
                    config.failure_threshold = *threshold as u32;
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(timeout)) =
                repo_config.get("web_search.error_handling.circuit_breaker_recovery_timeout")
            {
                if *timeout > 0 {
                    config.recovery_timeout = Duration::from_millis(*timeout as u64);
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(max_calls)) =
                repo_config.get("web_search.error_handling.circuit_breaker_half_open_max_calls")
            {
                if *max_calls > 0 {
                    config.half_open_max_calls = *max_calls as u32;
                }
            }
        }

        config
    }

    /// Loads configuration for the instance manager
    fn load_instance_manager_config() -> InstanceManagerConfig {
        let mut config = InstanceManagerConfig::default();

        if let Ok(Some(repo_config)) = swissarmyhammer::sah_config::load_repo_config_for_cli() {
            if let Some(swissarmyhammer::ConfigValue::Boolean(enabled)) =
                repo_config.get("web_search.discovery.enabled")
            {
                config.discovery_enabled = *enabled;
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(interval)) =
                repo_config.get("web_search.discovery.refresh_interval_seconds")
            {
                if *interval > 0 {
                    config.discovery_refresh_interval = Duration::from_secs(*interval as u64);
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(interval)) =
                repo_config.get("web_search.discovery.health_check_interval_seconds")
            {
                if *interval > 0 {
                    config.health_check_interval = Duration::from_secs(*interval as u64);
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(failures)) =
                repo_config.get("web_search.discovery.max_consecutive_failures")
            {
                if *failures > 0 {
                    config.max_consecutive_failures = *failures as u32;
                }
            }
        }

        config
    }

    /// Loads configuration for content fetching
    fn load_content_fetch_config() -> ContentFetchConfig {
        let mut config = ContentFetchConfig::default();

        if let Ok(Some(repo_config)) = swissarmyhammer::sah_config::load_repo_config_for_cli() {
            if let Some(swissarmyhammer::ConfigValue::Integer(max_concurrent)) =
                repo_config.get("web_search.content_fetching.max_concurrent_fetches")
            {
                if *max_concurrent > 0 {
                    config.max_concurrent_fetches = *max_concurrent as usize;
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(timeout)) =
                repo_config.get("web_search.content_fetching.content_fetch_timeout")
            {
                if *timeout > 0 {
                    config.fetch_timeout = Duration::from_secs(*timeout as u64);
                }
            }

            if let Some(swissarmyhammer::ConfigValue::String(size_str)) =
                repo_config.get("web_search.content_fetching.max_content_size")
            {
                if let Ok(size) = Self::parse_size_string(size_str) {
                    config.max_content_size = size;
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(delay)) =
                repo_config.get("web_search.content_fetching.default_domain_delay")
            {
                if *delay > 0 {
                    config.default_domain_delay = Duration::from_millis(*delay as u64);
                }
            }

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
        }

        config
    }

    /// Loads configuration for privacy features
    fn load_privacy_config() -> PrivacyConfig {
        let mut config = PrivacyConfig::default();

        if let Ok(Some(repo_config)) = swissarmyhammer::sah_config::load_repo_config_for_cli() {
            if let Some(swissarmyhammer::ConfigValue::Boolean(rotate)) =
                repo_config.get("web_search.privacy.rotate_user_agents")
            {
                config.rotate_user_agents = *rotate;
            }

            if let Some(swissarmyhammer::ConfigValue::Boolean(randomize)) =
                repo_config.get("web_search.privacy.randomize_user_agents")
            {
                config.randomize_user_agents = *randomize;
            }

            if let Some(swissarmyhammer::ConfigValue::Array(agents)) =
                repo_config.get("web_search.privacy.custom_user_agents")
            {
                let custom_agents: Vec<String> = agents
                    .iter()
                    .filter_map(|v| {
                        if let swissarmyhammer::ConfigValue::String(s) = v {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .collect();
                if !custom_agents.is_empty() {
                    config.custom_user_agents = Some(custom_agents);
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Boolean(enable_dnt)) =
                repo_config.get("web_search.privacy.enable_dnt")
            {
                config.enable_dnt = *enable_dnt;
            }

            if let Some(swissarmyhammer::ConfigValue::Boolean(strip_referrer)) =
                repo_config.get("web_search.privacy.strip_referrer")
            {
                config.strip_referrer = *strip_referrer;
            }

            if let Some(swissarmyhammer::ConfigValue::Boolean(disable_cache)) =
                repo_config.get("web_search.privacy.disable_cache")
            {
                config.disable_cache = *disable_cache;
            }

            if let Some(swissarmyhammer::ConfigValue::Boolean(enable_jitter)) =
                repo_config.get("web_search.privacy.enable_request_jitter")
            {
                config.enable_request_jitter = *enable_jitter;
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(min_delay)) =
                repo_config.get("web_search.privacy.min_request_delay_ms")
            {
                if *min_delay > 0 {
                    config.min_request_delay_ms = *min_delay as u64;
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(max_delay)) =
                repo_config.get("web_search.privacy.max_request_delay_ms")
            {
                if *max_delay > 0 {
                    config.max_request_delay_ms = *max_delay as u64;
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Boolean(distribute)) =
                repo_config.get("web_search.privacy.distribute_requests")
            {
                config.distribute_requests = *distribute;
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(avoid_repeat)) =
                repo_config.get("web_search.privacy.avoid_repeat_instances")
            {
                if *avoid_repeat > 0 {
                    config.avoid_repeat_instances = *avoid_repeat as usize;
                }
            }

            if let Some(swissarmyhammer::ConfigValue::Boolean(anonymize_content)) =
                repo_config.get("web_search.privacy.anonymize_content_requests")
            {
                config.anonymize_content_requests = *anonymize_content;
            }

            if let Some(swissarmyhammer::ConfigValue::Integer(content_delay)) =
                repo_config.get("web_search.privacy.content_request_delay_ms")
            {
                if *content_delay > 0 {
                    config.content_request_delay_ms = *content_delay as u64;
                }
            }
        }

        config
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

    /// Performs a search operation with comprehensive error handling and recovery
    async fn perform_search_with_recovery(
        &mut self,
        request: &WebSearchRequest,
    ) -> Result<EnhancedWebSearchResponse, RecoveryWebSearchError> {
        let start_time = Instant::now();
        
        // Load configurations
        let privacy_config = Self::load_privacy_config();
        let privacy_manager = PrivacyManager::new(privacy_config);
        let content_config = Self::load_content_fetch_config();
        let content_fetcher = ContentFetcher::new(content_config);

        // Get instance manager
        let instance_manager = Self::get_instance_manager().await;

        // Create search result builder for graceful degradation
        let mut result_builder = SearchResultBuilder::new(request.query.clone());

        // Get available instances and try them with failover logic
        let available_instances: Vec<String> = instance_manager
            .get_instances()
            .await
            .into_iter()
            .map(|i| i.url)
            .collect();

        if available_instances.is_empty() {
            return Err(RecoveryWebSearchError::NoInstancesAvailable);
        }

        let max_attempts = std::cmp::min(available_instances.len(), 3);

        for _attempt in 0..max_attempts {
            result_builder.record_instance_attempt();

            // Select instance based on circuit breaker state
            let instance_url = if let Some(distributed_url) =
                privacy_manager.select_distributed_instance(&available_instances)
            {
                distributed_url
            } else if let Some(instance) = instance_manager.get_next_instance().await {
                instance.url.clone()
            } else {
                break;
            };

            // Create failover manager for this instance
            let retry_config = Self::load_retry_config();
            let circuit_breaker_config = Self::load_circuit_breaker_config();
            let failover_manager = FailoverManager::new(retry_config, circuit_breaker_config);

            // Check circuit breaker
            if !failover_manager.can_use_instance(&instance_url).await {
                result_builder.record_circuit_breaker_trip();
                continue;
            }

            // Attempt search with this instance
            match self
                .perform_single_search(&instance_url, request, &privacy_manager)
                .await
            {
                Ok(search_results) => {
                    tracing::info!(
                        "Search successful on instance {}: {} results",
                        instance_url,
                        search_results.len()
                    );

                    // Record success
                    failover_manager.record_success(&instance_url).await;
                    privacy_manager.record_instance_use(&instance_url);

                    // Fetch content if requested
                    let final_results = if request.fetch_content.unwrap_or(true) {
                        let (processed_results, stats) = content_fetcher
                            .fetch_search_results_with_privacy(search_results, &privacy_manager)
                            .await;

                        // Add content fetch failures to result builder  
                        if stats.failed > 0 {
                            for _ in 0..stats.failed {
                                result_builder.add_content_failure(ContentFetchError {
                                    url: "unknown".to_string(), // We don't have individual failure details
                                    error: "Content fetch failed".to_string(),
                                    retryable: true, // Most content fetch errors are retryable
                                });
                            }
                        }

                        processed_results
                    } else {
                        search_results
                    };

                    result_builder.add_results(final_results);
                    break;
                }
                Err(error) => {
                    tracing::warn!("Search failed on instance {}: {}", instance_url, error);

                    // Record failure
                    failover_manager.record_failure(&instance_url, &error).await;

                    // Mark instance as failed for health tracking
                    instance_manager.mark_instance_failed(&instance_url).await;

                    // Handle rate limiting
                    if let RecoveryWebSearchError::RateLimited { retry_after_secs, .. } = &error {
                        instance_manager
                            .mark_instance_rate_limited(
                                &instance_url,
                                Duration::from_secs(*retry_after_secs),
                            )
                            .await;
                    }

                    result_builder.add_search_error(error.clone());

                    // If this is a non-retryable error, don't try other instances
                    if !error.is_retryable() {
                        break;
                    }

                    result_builder.record_retry_attempt();
                }
            }
        }

        // Build final response with graceful degradation
        let search_time = start_time.elapsed();
        
        // Collect metadata before moving result_builder
        let stats = result_builder.stats.clone();
        let warnings = result_builder.warnings.clone();
        let search_errors = result_builder.search_errors.clone();
        let search_results_found = stats.search_results_found;
        let content_fetch_failures = stats.content_fetch_failures;
        
        let metadata = SearchMetadata {
            query: request.query.clone(),
            category: request.category.clone().unwrap_or_default(),
            language: request.language.clone().unwrap_or_else(|| "en".to_string()),
            results_count: search_results_found,
            search_time_ms: search_time.as_millis() as u64,
            instance_used: "multiple".to_string(), // We tried multiple instances
            total_results: search_results_found,
            engines_used: vec!["multiple".to_string()],
            content_fetch_stats: if content_fetch_failures > 0 {
                Some(ContentFetchStats {
                    attempted: search_results_found,
                    successful: search_results_found - content_fetch_failures,
                    failed: content_fetch_failures,
                    total_time_ms: search_time.as_millis() as u64,
                })
            } else {
                None
            },
            fetch_content: request.fetch_content.unwrap_or(true),
        };

        // Convert to enhanced response with graceful degradation support
        match result_builder.build_response(metadata) {
            Ok(response) => {
                let enhanced_metadata = EnhancedSearchMetadata {
                    query: response.metadata.query,
                    category: response.metadata.category,
                    language: response.metadata.language,
                    results_count: response.metadata.results_count,
                    search_time_ms: response.metadata.search_time_ms,
                    instance_used: response.metadata.instance_used,
                    total_results: response.metadata.total_results,
                    engines_used: response.metadata.engines_used,
                    content_fetch_stats: response.metadata.content_fetch_stats,
                    fetch_content: response.metadata.fetch_content,
                    warnings: if warnings.is_empty() {
                        None
                    } else {
                        Some(warnings)
                    },
                    success_stats: Some(stats),
                    degraded_service: !search_errors.is_empty() || content_fetch_failures > 0,
                };

                Ok(EnhancedWebSearchResponse {
                    results: response.results,
                    metadata: enhanced_metadata,
                    is_error: false,
                })
            }
            Err(error) => Err(error),
        }
    }

    /// Performs a single search request against a specific instance
    async fn perform_single_search(
        &mut self,
        instance: &str,
        request: &WebSearchRequest,
        privacy_manager: &PrivacyManager,
    ) -> Result<Vec<SearchResult>, RecoveryWebSearchError> {
        let client = self.get_client();

        // Validate the instance URL
        let instance_url = Url::parse(instance).map_err(|e| RecoveryWebSearchError::InvalidParameters {
            details: format!("Invalid SearXNG instance URL '{instance}': {e}"),
        })?;

        // Construct search URL
        let mut url = instance_url
            .join("search")
            .map_err(|e| RecoveryWebSearchError::InvalidParameters {
                details: format!("Failed to construct search URL for instance '{instance}': {e}"),
            })?;

        // Build query parameters
        {
            let mut query_pairs = url.query_pairs_mut();

            query_pairs
                .append_pair("q", &request.query)
                .append_pair("format", "json")
                .append_pair("pageno", "1");

            if let Some(category) = &request.category {
                let category_str = Self::category_to_string(category);
                query_pairs.append_pair("categories", category_str);
            }

            if let Some(language) = &request.language {
                query_pairs.append_pair("language", language);
            }

            if let Some(safe_search) = request.safe_search {
                query_pairs.append_pair("safesearch", &(safe_search as u8).to_string());
            }

            if let Some(time_range) = &request.time_range {
                if let Some(time_str) = Self::time_range_to_string(time_range) {
                    query_pairs.append_pair("time_range", time_str);
                }
            }
        }

        tracing::debug!("Making search request to: {}", url);

        // Apply privacy jitter delay
        privacy_manager.apply_jitter().await;

        // Build request with privacy features
        let mut request_builder = client.get(url).timeout(Duration::from_secs(15));

        // Apply User-Agent
        if let Some(user_agent) = privacy_manager.get_user_agent() {
            request_builder = request_builder.header("User-Agent", user_agent);
        } else {
            request_builder = request_builder.header(
                "User-Agent",
                "SwissArmyHammer/1.0 (Enhanced Privacy-Focused Web Search)",
            );
        }

        // Apply privacy headers
        request_builder = privacy_manager.apply_privacy_headers(request_builder);

        let response = request_builder.send().await.map_err(|e| {
            if e.is_timeout() {
                RecoveryWebSearchError::ConnectionTimeout {
                    timeout_ms: 15000,
                    instance: instance.to_string(),
                }
            } else if e.is_connect() {
                RecoveryWebSearchError::Network {
                    message: format!("Connection failed: {e}"),
                    instance: instance.to_string(),
                    source: None,
                }
            } else {
                RecoveryWebSearchError::Network {
                    message: format!("Network error: {e}"),
                    instance: instance.to_string(),
                    source: None,
                }
            }
        })?;

        if !response.status().is_success() {
            return Err(RecoveryWebSearchError::InstanceUnavailable {
                url: instance.to_string(),
            });
        }

        let json: Value = response.json().await.map_err(|e| RecoveryWebSearchError::ResponseParsing {
            details: format!("Failed to parse JSON response: {e}"),
            instance: instance.to_string(),
        })?;

        // Parse search results
        let results_array = json["results"].as_array();
        let mut results = Vec::new();

        if let Some(results_array) = results_array {
            let max_results = request.results_count.unwrap_or(10);

            for (index, result_json) in results_array.iter().enumerate() {
                if index >= max_results {
                    break;
                }

                let title = result_json["title"]
                    .as_str()
                    .unwrap_or("Untitled")
                    .to_string();

                let url = match result_json["url"].as_str() {
                    Some(url) if !url.is_empty() => url,
                    _ => {
                        tracing::warn!("Missing or empty URL in search result {}, skipping", index);
                        continue;
                    }
                };

                // Validate URL format
                if let Err(e) = Url::parse(url) {
                    tracing::warn!("Invalid URL in search result {}: {} - {}", index, url, e);
                    continue;
                }

                let description = result_json["content"]
                    .as_str()
                    .or_else(|| result_json["description"].as_str())
                    .unwrap_or("")
                    .to_string();

                let engine = result_json["engine"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();

                let score = result_json["score"]
                    .as_f64()
                    .unwrap_or(1.0)
                    .clamp(0.0, 1.0);

                results.push(SearchResult {
                    title,
                    url: url.to_string(),
                    description,
                    score,
                    engine,
                    content: None,
                });
            }
        }

        Ok(results)
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
    fn time_range_to_string(time_range: &TimeRange) -> Option<&'static str> {
        match time_range {
            TimeRange::All => None,
            TimeRange::Day => Some("day"),
            TimeRange::Week => Some("week"),
            TimeRange::Month => Some("month"),
            TimeRange::Year => Some("year"),
        }
    }

    /// Validates request parameters
    fn validate_request(request: &WebSearchRequest) -> Result<(), RecoveryWebSearchError> {
        if request.query.trim().is_empty() {
            return Err(RecoveryWebSearchError::QueryValidation {
                reason: "Search query cannot be empty".to_string(),
            });
        }

        if request.query.len() > 500 {
            return Err(RecoveryWebSearchError::QueryValidation {
                reason: format!(
                    "Search query is {} characters, maximum is 500",
                    request.query.len()
                ),
            });
        }

        if let Some(count) = request.results_count {
            if count == 0 {
                return Err(RecoveryWebSearchError::InvalidParameters {
                    details: "Results count must be at least 1".to_string(),
                });
            }
            if count > 50 {
                return Err(RecoveryWebSearchError::InvalidParameters {
                    details: format!("Results count is {count}, maximum is 50"),
                });
            }
        }

        Ok(())
    }
}

#[async_trait]
impl McpTool for EnhancedWebSearchTool {
    fn name(&self) -> &'static str {
        "enhanced_web_search"
    }

    fn description(&self) -> &'static str {
        "Perform web searches using SearXNG with comprehensive error handling, circuit breaker protection, and graceful degradation. Includes automatic failover, retry logic, and enhanced privacy features."
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
            "Starting enhanced web search: '{}', results_count: {:?}, fetch_content: {:?}",
            request.query,
            request.results_count,
            request.fetch_content
        );

        // Validate request
        if let Err(validation_error) = Self::validate_request(&request) {
            let error_message = validation_error.user_friendly_message();
            let suggestions = validation_error.recovery_suggestions();
            
            return Err(McpError::invalid_request(
                format!("{}\n\nSuggestions:\n{}", error_message, suggestions.join("\n- ")),
                None,
            ));
        }

        let mut search_tool = EnhancedWebSearchTool::new();

        match search_tool.perform_search_with_recovery(&request).await {
            Ok(response) => {
                tracing::info!(
                    "Enhanced web search completed: found {} results for '{}' in {}ms",
                    response.results.len(),
                    response.metadata.query,
                    response.metadata.search_time_ms
                );

                Ok(BaseToolImpl::create_success_response(
                    serde_json::to_string_pretty(&response).map_err(|e| {
                        McpError::internal_error(
                            format!("Failed to serialize response: {e}"),
                            None,
                        )
                    })?,
                ))
            }
            Err(error) => {
                let error_message = error.user_friendly_message();
                let suggestions = error.recovery_suggestions();

                // Create enhanced error response
                let error_response = serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": error_message
                    }],
                    "is_error": true,
                    "metadata": {
                        "query": request.query,
                        "error_type": match error {
                            RecoveryWebSearchError::NoInstancesAvailable => "no_instances_available",
                            RecoveryWebSearchError::RateLimited { .. } => "rate_limited", 
                            RecoveryWebSearchError::QueryValidation { .. } => "query_validation",
                            RecoveryWebSearchError::ConnectionTimeout { .. } => "connection_timeout",
                            RecoveryWebSearchError::ContentSizeLimit { .. } => "content_size_limit",
                            RecoveryWebSearchError::InstanceDiscovery { .. } => "instance_discovery",
                            RecoveryWebSearchError::DnsResolution { .. } => "dns_resolution",
                            RecoveryWebSearchError::ContentFetch { .. } => "content_fetch",
                            _ => "search_error"
                        },
                        "error_details": format!("{}", error),
                        "recovery_suggestions": suggestions,
                        "retry_after": error.retry_delay().map(|d| d.as_secs())
                    }
                });

                Err(McpError::internal_error(
                    serde_json::to_string_pretty(&error_response)
                        .unwrap_or_else(|_| "Enhanced search failed".to_string()),
                    None,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::web_search::types::SafeSearchLevel;
    use crate::test_utils::create_test_context;

    #[test]
    fn test_enhanced_web_search_tool_new() {
        let tool = EnhancedWebSearchTool::new();
        assert_eq!(tool.name(), "enhanced_web_search");
        assert!(!tool.description().is_empty());
        assert!(tool.description().contains("comprehensive error handling"));
    }

    #[test]
    fn test_enhanced_web_search_tool_schema() {
        let tool = EnhancedWebSearchTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert!(schema["properties"]["results_count"].is_object());
        assert!(schema["properties"]["category"].is_object());
        assert_eq!(schema["required"], serde_json::json!(["query"]));
    }

    #[tokio::test]
    async fn test_enhanced_web_search_tool_execute_empty_query() {
        let tool = EnhancedWebSearchTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "query".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("query cannot be empty"));
    }

    #[tokio::test]
    async fn test_enhanced_web_search_tool_execute_missing_query() {
        let tool = EnhancedWebSearchTool::new();
        let context = create_test_context().await;

        let arguments = serde_json::Map::new();

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_enhanced_validate_request_valid() {
        let request = WebSearchRequest {
            query: "test query".to_string(),
            category: Some(SearchCategory::General),
            language: Some("en".to_string()),
            results_count: Some(10),
            fetch_content: Some(true),
            safe_search: Some(SafeSearchLevel::Moderate),
            time_range: Some(TimeRange::Month),
        };
        assert!(EnhancedWebSearchTool::validate_request(&request).is_ok());
    }

    #[test]
    fn test_enhanced_validate_request_empty_query() {
        let request = WebSearchRequest {
            query: "".to_string(),
            category: None,
            language: None,
            results_count: None,
            fetch_content: None,
            safe_search: None,
            time_range: None,
        };
        let result = EnhancedWebSearchTool::validate_request(&request);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RecoveryWebSearchError::QueryValidation { .. }));
    }

    #[test]
    fn test_enhanced_validate_request_query_too_long() {
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
        let result = EnhancedWebSearchTool::validate_request(&request);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), RecoveryWebSearchError::QueryValidation { .. }));
    }

    #[test]
    fn test_load_retry_config_default() {
        let config = EnhancedWebSearchTool::load_retry_config();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.base_delay, Duration::from_millis(500));
        assert!(config.jitter);
    }

    #[test]
    fn test_load_circuit_breaker_config_default() {
        let config = EnhancedWebSearchTool::load_circuit_breaker_config();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.recovery_timeout, Duration::from_secs(60));
        assert_eq!(config.half_open_max_calls, 3);
    }
}