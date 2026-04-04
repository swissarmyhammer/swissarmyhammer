//! SearchUrl operation — delegates to swissarmyhammer-web search pipeline

use crate::mcp::tool_registry::{send_mcp_log, BaseToolImpl, ToolContext};
use rmcp::model::{CallToolResult, LoggingLevel};
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use std::time::Instant;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use swissarmyhammer_web::search::brave::BraveSearchError;
use swissarmyhammer_web::search::content_fetcher::ContentFetcher;
use swissarmyhammer_web::types::*;
use swissarmyhammer_web::WebSearcher;

/// Search the web using Brave Search with optional content fetching
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
        "Search the web using Brave Search with optional content fetching"
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
    if let Err(validation_error) = WebSearcher::validate_request(&request) {
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

    // Create a fresh search tool instance
    let mut search_tool = WebSearcher::new();

    send_mcp_log(
        context,
        LoggingLevel::Info,
        "web_search",
        "Executing search...".into(),
    )
    .await;

    // Perform search using Brave Search (direct HTTP, no browser needed)
    let search_client = search_tool.get_search_client();
    let mut results = match search_client.search(&request).await {
        Ok(results) => results,
        Err(BraveSearchError::NoResults) => {
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
                attempted_instances: vec!["https://search.brave.com".to_string()],
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
                error_details: format!("Brave web search failed: {e}"),
                attempted_instances: vec!["https://search.brave.com".to_string()],
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
        let content_config = WebSearcher::load_content_fetch_config();
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
            instance_used: "https://search.brave.com".to_string(),
            total_results: results.len(),
            engines_used: vec!["brave".to_string()],
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper that builds a minimal serde_json::Map with the given query.
    fn search_args(query: &str) -> serde_json::Map<String, serde_json::Value> {
        let mut args = serde_json::Map::new();
        args.insert(
            "query".to_string(),
            serde_json::Value::String(query.to_string()),
        );
        args
    }

    async fn test_context() -> ToolContext {
        crate::test_utils::create_test_context().await
    }

    // ── Validation error paths ────────────────────────────────────────────────

    /// An empty query is rejected before any network call is made.
    #[tokio::test]
    async fn test_execute_search_empty_query_returns_invalid_request() {
        let ctx = test_context().await;
        let result = execute_search(search_args(""), &ctx).await;
        assert!(result.is_err(), "empty query should fail");
        let err = result.unwrap_err();
        assert!(
            err.message.contains("empty") || err.message.contains("cannot be empty"),
            "error should mention empty query, got: {}",
            err.message
        );
    }

    /// A whitespace-only query is treated the same as an empty query.
    #[tokio::test]
    async fn test_execute_search_whitespace_query_returns_invalid_request() {
        let ctx = test_context().await;
        let result = execute_search(search_args("   "), &ctx).await;
        assert!(result.is_err(), "whitespace-only query should fail");
    }

    /// A query that exceeds 500 characters is rejected.
    #[tokio::test]
    async fn test_execute_search_query_too_long_returns_invalid_request() {
        let ctx = test_context().await;
        let long_query = "a".repeat(501);
        let result = execute_search(search_args(&long_query), &ctx).await;
        assert!(result.is_err(), "501-char query should fail");
        let err = result.unwrap_err();
        assert!(
            err.message.contains("501") || err.message.contains("maximum"),
            "error should mention query length, got: {}",
            err.message
        );
    }

    /// An invalid language code is rejected before any network call.
    #[tokio::test]
    async fn test_execute_search_invalid_language_returns_invalid_request() {
        let ctx = test_context().await;
        let mut args = search_args("rust programming");
        args.insert(
            "language".to_string(),
            serde_json::Value::String("not-a-lang".to_string()),
        );
        let result = execute_search(args, &ctx).await;
        assert!(result.is_err(), "invalid language should fail");
        let err = result.unwrap_err();
        assert!(
            err.message.contains("language") || err.message.contains("Invalid"),
            "error should mention language, got: {}",
            err.message
        );
    }

    /// Zero results_count is rejected.
    #[tokio::test]
    async fn test_execute_search_zero_results_count_returns_invalid_request() {
        let ctx = test_context().await;
        let mut args = search_args("rust");
        args.insert(
            "results_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(0u64)),
        );
        let result = execute_search(args, &ctx).await;
        assert!(result.is_err(), "results_count=0 should fail");
        let err = result.unwrap_err();
        assert!(
            err.message.contains("least 1") || err.message.contains("results"),
            "error should mention results count, got: {}",
            err.message
        );
    }

    /// results_count above the 50-item ceiling is rejected.
    #[tokio::test]
    async fn test_execute_search_excess_results_count_returns_invalid_request() {
        let ctx = test_context().await;
        let mut args = search_args("rust");
        args.insert(
            "results_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(51u64)),
        );
        let result = execute_search(args, &ctx).await;
        assert!(result.is_err(), "results_count=51 should fail");
        let err = result.unwrap_err();
        assert!(
            err.message.contains("50") || err.message.contains("maximum"),
            "error should mention maximum results, got: {}",
            err.message
        );
    }

    // ── Parameter parsing ────────────────────────────────────────────────────

    /// Deserialization of unknown/extra fields should not panic execute_search.
    /// This test may hit the real Brave endpoint; we only care that it does not
    /// crash with a deserialization error on the extra field.
    #[tokio::test]
    async fn test_execute_search_ignores_unknown_fields_in_args() {
        let ctx = test_context().await;
        // "unknown_param" is not part of WebSearchRequest — the tool should
        // either silently ignore it or fail gracefully (not panic).
        let mut args = search_args("");
        args.insert(
            "unknown_param".to_string(),
            serde_json::Value::String("ignored".to_string()),
        );
        // An empty query means validation fails before any network call.
        let result = execute_search(args, &ctx).await;
        assert!(
            result.is_err(),
            "empty query + unknown param should fail on validation"
        );
    }

    // ── SearchUrl operation metadata ─────────────────────────────────────────

    #[test]
    fn test_search_url_operation_verb_and_noun() {
        let op = SearchUrl::default();
        assert_eq!(op.verb(), "search");
        assert_eq!(op.noun(), "url");
    }

    #[test]
    fn test_search_url_operation_description_not_empty() {
        let op = SearchUrl::default();
        assert!(!op.description().is_empty());
    }

    #[test]
    fn test_search_url_operation_parameters_not_empty() {
        let op = SearchUrl::default();
        assert!(!op.parameters().is_empty());
    }

    #[test]
    fn test_search_url_has_required_query_param() {
        use swissarmyhammer_operations::ParamMeta;
        let op = SearchUrl::default();
        let params: &[ParamMeta] = op.parameters();
        let query_param = params.iter().find(|p| p.name == "query");
        assert!(query_param.is_some(), "should have a 'query' parameter");
        assert!(
            query_param.unwrap().required,
            "'query' parameter should be required"
        );
    }

    #[test]
    fn test_search_url_op_string() {
        use swissarmyhammer_operations::Operation;
        let op = SearchUrl::default();
        assert_eq!(op.op_string(), "search url");
    }
}
