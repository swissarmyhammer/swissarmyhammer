//! FetchUrl operation — delegates to swissarmyhammer-web fetch pipeline

use crate::mcp::tool_registry::{send_mcp_log, BaseToolImpl, ToolContext};
use rmcp::model::{CallToolResult, LoggingLevel};
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use serde_json::json;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use swissarmyhammer_web::{FetchError, WebFetchRequest, WebFetcher};

/// Fetch web content and convert HTML to markdown
#[derive(Debug, Default, Deserialize)]
pub struct FetchUrl {
    /// The URL to fetch content from (must be a valid HTTP/HTTPS URL)
    pub url: Option<String>,
    /// Request timeout in seconds (1-120, default 30)
    pub timeout: Option<u32>,
    /// Whether to follow HTTP redirects (default true)
    pub follow_redirects: Option<bool>,
    /// Maximum content length in bytes (1KB-10MB, default 1MB)
    pub max_content_length: Option<u32>,
    /// Custom User-Agent header
    pub user_agent: Option<String>,
}

static FETCH_URL_PARAMS: &[ParamMeta] = &[
    ParamMeta::new("url")
        .description("The URL to fetch content from (must be a valid HTTP/HTTPS URL)")
        .param_type(ParamType::String)
        .required(),
    ParamMeta::new("timeout")
        .description("Request timeout in seconds (1-120, default 30)")
        .param_type(ParamType::Integer),
    ParamMeta::new("follow_redirects")
        .description("Whether to follow HTTP redirects (default true)")
        .param_type(ParamType::Boolean),
    ParamMeta::new("max_content_length")
        .description("Maximum content length in bytes (1KB-10MB, default 1MB)")
        .param_type(ParamType::Integer),
    ParamMeta::new("user_agent")
        .description("Custom User-Agent header (default SwissArmyHammer-Bot/1.0)")
        .param_type(ParamType::String),
];

impl Operation for FetchUrl {
    fn verb(&self) -> &'static str {
        "fetch"
    }
    fn noun(&self) -> &'static str {
        "url"
    }
    fn description(&self) -> &'static str {
        "Fetch web content and convert HTML to markdown"
    }
    fn parameters(&self) -> &'static [ParamMeta] {
        FETCH_URL_PARAMS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build a minimal arguments map with a given URL.
    fn fetch_args(url: &str) -> serde_json::Map<String, serde_json::Value> {
        let mut args = serde_json::Map::new();
        args.insert(
            "url".to_string(),
            serde_json::Value::String(url.to_string()),
        );
        args
    }

    async fn test_context() -> ToolContext {
        crate::test_utils::create_test_context().await
    }

    // ── URL validation / security paths ──────────────────────────────────────

    /// A completely invalid (non-URL) string must return an `invalid_params` error.
    #[tokio::test]
    async fn test_execute_fetch_invalid_url_returns_invalid_params() {
        let ctx = test_context().await;
        let result = execute_fetch(fetch_args("not-a-url"), &ctx).await;
        assert!(result.is_err(), "invalid URL should produce an error");
        // Just verify we get an error — the message should mention the URL or validation
        let err = result.unwrap_err();
        assert!(
            !err.message.is_empty(),
            "error should have a non-empty message"
        );
    }

    /// An unsupported scheme (ftp://) must return an `invalid_params` error.
    #[tokio::test]
    async fn test_execute_fetch_unsupported_scheme_returns_invalid_params() {
        let ctx = test_context().await;
        let result = execute_fetch(fetch_args("ftp://example.com/file.txt"), &ctx).await;
        assert!(result.is_err(), "ftp:// should be rejected");
        let err = result.unwrap_err();
        assert!(
            err.message.contains("protocol")
                || err.message.contains("scheme")
                || err.message.contains("Unsupported")
                || err.message.contains("Invalid"),
            "error should mention unsupported scheme, got: {}",
            err.message
        );
    }

    /// A loopback URL (SSRF) must return an `invalid_params` error.
    #[tokio::test]
    async fn test_execute_fetch_ssrf_loopback_returns_invalid_params() {
        let ctx = test_context().await;
        let result = execute_fetch(fetch_args("http://127.0.0.1/admin"), &ctx).await;
        assert!(result.is_err(), "loopback URL should be rejected as SSRF");
        let err = result.unwrap_err();
        assert!(
            err.message.contains("private")
                || err.message.contains("Security")
                || err.message.contains("SSRF")
                || err.message.contains("not allowed"),
            "error should mention security violation, got: {}",
            err.message
        );
    }

    /// Access to a private-network metadata endpoint must be blocked.
    #[tokio::test]
    async fn test_execute_fetch_blocked_metadata_domain_returns_invalid_params() {
        let ctx = test_context().await;
        let result = execute_fetch(
            fetch_args("https://metadata.google.internal/computeMetadata"),
            &ctx,
        )
        .await;
        assert!(result.is_err(), "cloud metadata endpoint should be blocked");
        let err = result.unwrap_err();
        assert!(
            !err.message.is_empty(),
            "error should have a non-empty message, got: {}",
            err.message
        );
    }

    /// An empty URL string must be rejected.
    #[tokio::test]
    async fn test_execute_fetch_empty_url_returns_invalid_params() {
        let ctx = test_context().await;
        let result = execute_fetch(fetch_args(""), &ctx).await;
        assert!(result.is_err(), "empty URL should be rejected");
        let err = result.unwrap_err();
        assert!(
            !err.message.is_empty(),
            "error should have a non-empty message, got: {}",
            err.message
        );
    }

    /// A fetch that reaches an unreachable host must return a non-error `CallToolResult`
    /// with `is_error: true` (the tool converts network failures to error results, not Err).
    ///
    /// Marked #[ignore] because waiting for the TCP timeout takes ~11 seconds;
    /// the underlying FetchError mapping is already exercised in
    /// swissarmyhammer-web/src/fetch.rs unit tests.
    #[tokio::test]
    #[ignore = "requires TCP timeout (~11s); FetchError mapping covered in swissarmyhammer-web tests"]
    async fn test_execute_fetch_unreachable_host_returns_error_result() {
        let ctx = test_context().await;
        // 192.0.2.1 is TEST-NET-1 (RFC 5737) — non-routable, so the fetch will time out
        // or be blocked by the SSRF guard.  Either an MCP Err or an Ok(is_error) is fine.
        let mut args = fetch_args("https://192.0.2.1/page");
        // Shorten the timeout so the test doesn't hang for 30s
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(1u64)),
        );
        let result = execute_fetch(args, &ctx).await;
        // Either the SSRF guard blocks it (Err) or the fetch fails (Ok with is_error)
        match result {
            Err(_) => { /* SSRF guard blocked — acceptable */ }
            Ok(call_result) => {
                assert_eq!(
                    call_result.is_error,
                    Some(true),
                    "unreachable host should produce an error result"
                );
            }
        }
    }

    // ── FetchUrl operation metadata ──────────────────────────────────────────

    #[test]
    fn test_fetch_url_operation_verb_and_noun() {
        let op = FetchUrl::default();
        assert_eq!(op.verb(), "fetch");
        assert_eq!(op.noun(), "url");
    }

    #[test]
    fn test_fetch_url_operation_description_not_empty() {
        let op = FetchUrl::default();
        assert!(!op.description().is_empty());
    }

    #[test]
    fn test_fetch_url_operation_parameters_not_empty() {
        let op = FetchUrl::default();
        assert!(!op.parameters().is_empty());
    }

    #[test]
    fn test_fetch_url_has_required_url_param() {
        use swissarmyhammer_operations::ParamMeta;
        let op = FetchUrl::default();
        let params: &[ParamMeta] = op.parameters();
        let url_param = params.iter().find(|p| p.name == "url");
        assert!(url_param.is_some(), "should have a 'url' parameter");
        assert!(
            url_param.unwrap().required,
            "'url' parameter should be required"
        );
    }

    #[test]
    fn test_fetch_url_op_string() {
        use swissarmyhammer_operations::Operation;
        let op = FetchUrl::default();
        assert_eq!(op.op_string(), "fetch url");
    }
}

/// Execute a fetch operation using swissarmyhammer-web fetch pipeline
pub async fn execute_fetch(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let request: WebFetchRequest = BaseToolImpl::parse_arguments(arguments)?;

    tracing::debug!("Fetching web content from URL: {}", request.url);

    send_mcp_log(
        context,
        LoggingLevel::Info,
        "web_fetch",
        format!("Fetching: {}", request.url),
    )
    .await;

    let fetcher = WebFetcher::new();

    match fetcher.fetch_url(&request).await {
        Ok(result) => {
            send_mcp_log(
                context,
                LoggingLevel::Info,
                "web_fetch",
                format!("Complete: {} chars", result.markdown.len()),
            )
            .await;

            Ok(CallToolResult::success(vec![rmcp::model::Content::text(
                result.markdown,
            )]))
        }
        Err(e) => {
            let (error_type, response_time_ms) = match &e {
                FetchError::InvalidUrl(_) => {
                    return Err(McpError::invalid_params(e.to_string(), None))
                }
                FetchError::SecurityViolation(_) => {
                    return Err(McpError::invalid_params(e.to_string(), None))
                }
                FetchError::FetchFailed {
                    error_type,
                    response_time_ms,
                    ..
                } => (error_type.clone(), *response_time_ms),
            };

            send_mcp_log(
                context,
                LoggingLevel::Error,
                "web_fetch",
                format!("Failed: {}", e),
            )
            .await;

            let metadata = json!({
                "url": request.url,
                "error_type": error_type,
                "error_details": e.to_string(),
                "response_time_ms": response_time_ms,
                "performance_impact": if response_time_ms > 10000 { "high" } else { "low" },
            });

            let response = json!({
                "content": [{
                    "type": "text",
                    "text": format!("Failed to fetch content: {e}")
                }],
                "is_error": true,
                "metadata": metadata
            });

            Ok(CallToolResult::error(vec![rmcp::model::Content::text(
                serde_json::to_string_pretty(&response).unwrap_or_default(),
            )]))
        }
    }
}
