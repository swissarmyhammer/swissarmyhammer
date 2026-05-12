//! EchoService - A simple MCP server implementation for testing.
//!
//! This service provides echo functionality through both tools and prompts,
//! following the rmcp SDK patterns for clean MCP server implementation.

use rmcp::{
    handler::server::{
        router::{prompt::PromptRouter, tool::ToolRouter},
        wrapper::Parameters,
    },
    model::*,
    prompt, prompt_handler, prompt_router, schemars,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};
// Note: serde_json::json removed as unused

/// Arguments accepted by the `echo` tool.
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct EchoToolArgs {
    /// The message to echo back
    pub message: String,
}

/// Arguments accepted by the `echo_prompt` prompt.
#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct EchoPromptArgs {
    /// The message to include in the echo prompt
    pub message: String,
}

/// EchoService provides simple echo functionality for testing MCP transports.
/// It implements both tool and prompt functionality using the rmcp SDK patterns.
#[derive(Clone)]
pub struct EchoService {
    tool_router: ToolRouter<Self>,
    prompt_router: PromptRouter<Self>,
}

#[tool_router]
impl EchoService {
    /// Construct a new `EchoService` with the tool and prompt routers populated
    /// from the macro-generated static methods.
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
            prompt_router: Self::prompt_router(),
        }
    }

    /// Echo back the provided message
    #[tool(description = "Echo back the input message")]
    async fn echo(
        &self,
        Parameters(args): Parameters<EchoToolArgs>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(format!(
            "Echo: {}",
            args.message
        ))]))
    }

    /// Get server status
    #[tool(description = "Get the current status of the echo server")]
    async fn status(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            "Echo server is running and ready",
        )]))
    }
}

#[prompt_router]
impl EchoService {
    /// Generate an echo prompt with the provided message
    #[prompt(name = "echo_prompt")]
    async fn echo_prompt(
        &self,
        Parameters(args): Parameters<EchoPromptArgs>,
    ) -> Result<GetPromptResult, McpError> {
        let messages = vec![
            PromptMessage::new_text(
                PromptMessageRole::User,
                format!("Please echo this message: {}", args.message),
            ),
            PromptMessage::new_text(
                PromptMessageRole::Assistant,
                format!("Echo: {}", args.message),
            ),
        ];

        Ok(GetPromptResult::new(messages)
            .with_description("Echo prompt for testing MCP functionality"))
    }
}

#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for EchoService {
    /// Return the static server info advertised during MCP initialization,
    /// including the supported protocol version, declared capabilities
    /// (prompts + tools), and a short instructions string.
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_prompts()
                .enable_tools()
                .build(),
        )
        .with_protocol_version(ProtocolVersion::V_2024_11_05)
        .with_server_info(Implementation::from_build_env())
        .with_instructions(
            "Echo server for testing MCP functionality. Tools: echo, status. Prompts: echo_prompt.",
        )
    }

    /// Handle the MCP `initialize` request. Logs the originating HTTP request
    /// (when the transport carries one) and returns the same `ServerInfo`
    /// produced by [`Self::get_info`].
    async fn initialize(
        &self,
        _request: InitializeRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<ServerInfo, McpError> {
        if let Some(http_request_part) = context.extensions.get::<http::request::Parts>() {
            let initialize_headers = &http_request_part.headers;
            let initialize_uri = &http_request_part.uri;
            tracing::info!(?initialize_headers, %initialize_uri, "initialize from http server");
        }
        Ok(self.get_info())
    }
}

impl Default for EchoService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_echo_service_creation() {
        let service = EchoService::new();
        let info = service.get_info();

        assert_eq!(info.protocol_version, ProtocolVersion::V_2024_11_05);
        assert!(info.capabilities.prompts.is_some());
        assert!(info.capabilities.tools.is_some());
    }

    #[tokio::test]
    async fn test_echo_tool() {
        let service = EchoService::new();
        let args = EchoToolArgs {
            message: "Hello, World!".to_string(),
        };

        let result = service.echo(Parameters(args)).await.unwrap();

        assert!(!result.content.is_empty(), "Expected content in result");
    }

    #[tokio::test]
    async fn test_echo_prompt() {
        let service = EchoService::new();
        let args = EchoPromptArgs {
            message: "Test message".to_string(),
        };

        let result = service.echo_prompt(Parameters(args)).await.unwrap();

        assert_eq!(
            result.description.as_deref(),
            Some("Echo prompt for testing MCP functionality")
        );
        assert_eq!(result.messages.len(), 2);
        assert_eq!(result.messages[0].role, PromptMessageRole::User);
        assert_eq!(result.messages[1].role, PromptMessageRole::Assistant);
        let PromptMessageContent::Text { text: user_text } = &result.messages[0].content else {
            panic!("expected text content for user message");
        };
        assert_eq!(user_text, "Please echo this message: Test message");
        let PromptMessageContent::Text {
            text: assistant_text,
        } = &result.messages[1].content
        else {
            panic!("expected text content for assistant message");
        };
        assert_eq!(assistant_text, "Echo: Test message");
    }
}
