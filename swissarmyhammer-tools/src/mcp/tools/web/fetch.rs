//! FetchUrl operation — delegates to swissarmyhammer-web fetch pipeline

use crate::mcp::tool_registry::{send_mcp_log, BaseToolImpl, ToolContext};
use rmcp::model::{CallToolResult, LoggingLevel};
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use serde_json::json;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};
use swissarmyhammer_web::{FetchError, WebFetcher, WebFetchRequest};

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

            Ok(CallToolResult {
                content: vec![rmcp::model::Annotated::new(
                    rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                        text: result.markdown,
                        meta: None,
                    }),
                    None,
                )],
                structured_content: None,
                meta: None,
                is_error: Some(false),
            })
        }
        Err(e) => {
            let (error_type, response_time_ms) = match &e {
                FetchError::InvalidUrl(_) => return Err(McpError::invalid_params(e.to_string(), None)),
                FetchError::SecurityViolation(_) => return Err(McpError::invalid_params(e.to_string(), None)),
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

            Ok(CallToolResult {
                content: vec![rmcp::model::Annotated::new(
                    rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                        text: serde_json::to_string_pretty(&response).unwrap_or_default(),
                        meta: None,
                    }),
                    None,
                )],
                meta: None,
                structured_content: None,
                is_error: Some(true),
            })
        }
    }
}
