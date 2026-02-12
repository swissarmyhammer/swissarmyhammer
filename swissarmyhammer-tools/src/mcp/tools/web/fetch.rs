//! FetchUrl operation — delegates to existing web_fetch pipeline

use crate::mcp::progress_notifications::generate_progress_token;
use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use crate::mcp::tools::web_fetch::fetch::WebFetchTool;
use crate::mcp::types::WebFetchRequest;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;
use serde_json::json;
use std::time::Instant;
use swissarmyhammer_operations::{Operation, ParamMeta, ParamType};

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

/// Execute a fetch operation using the existing web_fetch pipeline
pub async fn execute_fetch(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let request: WebFetchRequest = BaseToolImpl::parse_arguments(arguments)?;

    tracing::debug!("Fetching web content from URL: {}", request.url);

    let fetch_tool = WebFetchTool::new();

    // Validate request parameters
    let validated_url = fetch_tool.validate_request_parameters(&request).await?;

    // Create markdowndown configuration from request parameters
    let config = fetch_tool.create_markdowndown_config(&request);

    // Generate progress token and send start notification
    let progress_token = generate_progress_token();
    if let Some(sender) = &context.progress_sender {
        sender
            .send_progress_with_metadata(
                &progress_token,
                Some(0),
                format!("Web fetch: Fetching: {}", request.url),
                json!({
                    "url": request.url,
                    "timeout": config.http.timeout.as_secs()
                }),
            )
            .ok();
    }

    let start_time = Instant::now();

    match markdowndown::convert_url_with_config(&validated_url, config).await {
        Ok(markdown) => {
            let response_time_ms = start_time.elapsed().as_millis() as u64;
            let markdown_content = markdown.to_string();

            if let Some(sender) = &context.progress_sender {
                sender
                    .send_progress_with_metadata(
                        &progress_token,
                        Some(100),
                        format!(
                            "Web fetch: Complete - {} chars in {:.1}s",
                            markdown_content.len(),
                            response_time_ms as f64 / 1000.0
                        ),
                        json!({
                            "markdown_length": markdown_content.len(),
                            "duration_ms": response_time_ms
                        }),
                    )
                    .ok();
            }

            fetch_tool.build_success_response(&request, markdown_content, response_time_ms)
        }
        Err(e) => {
            let response_time_ms = start_time.elapsed().as_millis() as u64;

            if let Some(sender) = &context.progress_sender {
                sender
                    .send_progress_with_metadata(
                        &progress_token,
                        None,
                        format!("Web fetch: Failed - {}", e),
                        json!({
                            "error": e.to_string(),
                            "url": request.url,
                            "duration_ms": response_time_ms
                        }),
                    )
                    .ok();
            }

            fetch_tool.build_error_response(&e, response_time_ms, &request)
        }
    }
}
