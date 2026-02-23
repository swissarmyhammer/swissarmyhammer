//! SearchUrl operation — delegates to existing web_search pipeline

use crate::mcp::tool_registry::{send_mcp_log, BaseToolImpl, ToolContext};
use crate::mcp::tools::web_search::content_fetcher::ContentFetcher;
use crate::mcp::tools::web_search::duckduckgo_client::DuckDuckGoError;
use crate::mcp::tools::web_search::search::WebSearchTool;
use crate::mcp::tools::web_search::types::*;
use rmcp::model::{CallToolResult, LoggingLevel};
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use std::time::Instant;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

/// Search the web using DuckDuckGo with optional content fetching
#[derive(Debug, Default, Deserialize)]
pub struct SearchUrl {
    /// The search query string
    pub query: Option<String>,
    /// Search category
    pub category: Option<SearchCategory>,
    /// Search language code (e.g. "en" or "en-US")
    pub language: Option<String>,
    /// Number of search results to return (1-50, default 10)
    pub results_count: Option<usize>,
    /// Whether to fetch content from result URLs (default true)
    pub fetch_content: Option<bool>,
    /// Safe search level
    pub safe_search: Option<SafeSearchLevel>,
    /// Time range filter
    pub time_range: Option<TimeRange>,
}

static SEARCH_URL_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("query")
        .description("The search query string")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("category")
        .description(
            "Search category (general, images, videos, news, map, music, it, science, files)",
        )
        .param_type(ParamType::String),
    ParamMeta::new("language")
        .description("Search language code (e.g. 'en' or 'en-US')")
        .param_type(ParamType::String),
    ParamMeta::new("results_count")
        .description("Number of search results to return (1-50, default 10)")
        .param_type(ParamType::Integer),
    ParamMeta::new("fetch_content")
        .description("Whether to fetch content from result URLs (default true)")
        .param_type(ParamType::Boolean),
    ParamMeta::new("safe_search")
        .description("Safe search level (off, moderate, strict)")
        .param_type(ParamType::String),
    ParamMeta::new("time_range")
        .description("Time range filter (all, day, week, month, year)")
        .param_type(ParamType::String),
];

impl Operation for SearchUrl {
    fn verb(&self) -> &'static str {
        "search"
    }
    fn noun(&self) -> &'static str {
        "url"
    }
    fn description(&self) -> &'static str {
        "Search the web using DuckDuckGo with optional content fetching"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        SEARCH_URL_PARAMS
    }
}

/// Execute a search operation using the existing web_search pipeline
pub async fn execute_search(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let request: WebSearchRequest = BaseToolImpl::parse_arguments(arguments)?;

    tracing::info!(
        "Starting web search: '{}', results_count: {:?}, fetch_content: {:?}",
        request.query,
        request.results_count,
        request.fetch_content
    );

    // Comprehensive parameter validation
    if let Err(validation_error) = WebSearchTool::validate_request(&request) {
        return Err(McpError::invalid_request(validation_error, None));
    }

    let start_time = Instant::now();

    send_mcp_log(
        context,
        LoggingLevel::Info,
        "web_search",
        format!("Starting search: {}", request.query),
    )
    .await;

    // Create a fresh search tool instance for its DuckDuckGo client
    let mut search_tool = WebSearchTool::new();

    send_mcp_log(
        context,
        LoggingLevel::Info,
        "web_search",
        "Executing search...".into(),
    )
    .await;

    // Perform search using DuckDuckGo browser automation
    let duckduckgo_client = search_tool.get_duckduckgo_client();
    let mut results = match duckduckgo_client.search(&request).await {
        Ok(results) => results,
        Err(DuckDuckGoError::NoResults) => {
            send_mcp_log(
                context,
                LoggingLevel::Warning,
                "web_search",
                "No results found".into(),
            )
            .await;

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
            send_mcp_log(
                context,
                LoggingLevel::Error,
                "web_search",
                format!("Failed: {}", e),
            )
            .await;

            let error = WebSearchError {
                error_type: "search_failed".to_string(),
                error_details: format!("DuckDuckGo web search failed: {e}"),
                attempted_instances: vec!["https://duckduckgo.com".to_string()],
                retry_after: Some(10),
            };

            return Err(McpError::internal_error(
                serde_json::to_string_pretty(&error)
                    .unwrap_or_else(|_| "Search failed".to_string()),
                None,
            ));
        }
    };

    let search_time = start_time.elapsed();

    send_mcp_log(
        context,
        LoggingLevel::Info,
        "web_search",
        "Processing results...".into(),
    )
    .await;

    // Optionally fetch content from each result
    let mut content_fetch_stats = None;

    if request.fetch_content.unwrap_or(true) {
        let content_config = WebSearchTool::load_content_fetch_config();
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

    send_mcp_log(
        context,
        LoggingLevel::Info,
        "web_search",
        format!("Complete: {} results", response.results.len()),
    )
    .await;

    Ok(BaseToolImpl::create_success_response(
        serde_json::to_string_pretty(&response).map_err(|e| {
            McpError::internal_error(format!("Failed to serialize response: {e}"), None)
        })?,
    ))
}
