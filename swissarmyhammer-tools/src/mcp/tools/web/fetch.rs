//! FetchUrl operation — delegates to swissarmyhammer-web fetch pipeline

use crate::mcp::progress_notifications::generate_progress_token;
use crate::mcp::tool_registry::{BaseToolImpl, ToolContext};
use swissarmyhammer_web::{FetchError, WebFetcher, WebFetchRequest};
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

/// Execute a fetch operation using swissarmyhammer-web fetch pipeline
pub async fn execute_fetch(
    arguments: serde_json::Map<String, serde_json::Value>,
    context: &ToolContext,
) -> Result<CallToolResult, McpError> {
    let request: WebFetchRequest = BaseToolImpl::parse_arguments(arguments)?;

    tracing::debug!("Fetching web content from URL: {}", request.url);

    let fetcher = WebFetcher::new();

    // Validate URL via swissarmyhammer-web security checks
    let validated_url = fetcher.validate_url(&request).await.map_err(|e| match &e {
        FetchError::InvalidUrl(msg) => McpError::invalid_params(msg.clone(), None),
        FetchError::SecurityViolation(msg) => McpError::invalid_params(msg.clone(), None),
        FetchError::FetchFailed { message, .. } => {
            McpError::internal_error(message.clone(), None)
        }
    })?;

    // Create markdowndown configuration from request parameters
    let config = fetcher.create_markdowndown_config(&request);

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

            Ok(CallToolResult {
                content: vec![rmcp::model::Annotated::new(
                    rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                        text: markdown_content,
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
            let response_time_ms = start_time.elapsed().as_millis() as u64;
            let error_type = WebFetcher::categorize_error(&e);

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

            tracing::warn!(
                "Failed to fetch content from {} after {}ms: {} (category: {})",
                request.url,
                response_time_ms,
                e,
                error_type
            );

            let metadata = json!({
                "url": request.url,
                "error_type": error_type,
                "error_details": e.to_string(),
                "status_code": null,
                "response_time_ms": response_time_ms,
                "performance_impact": if response_time_ms > 10000 { "high" } else { "low" },
                "optimization_enabled": true
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
