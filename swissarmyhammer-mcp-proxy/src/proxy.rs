use crate::filter::ToolFilter;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use std::sync::Arc;
use swissarmyhammer_tools::mcp::McpServer;

/// Filtering proxy that wraps McpServer and filters tool discovery.
///
/// This proxy implements ServerHandler and forwards all requests to the wrapped
/// McpServer, except for list_tools() which filters the results based on
/// allow/deny regex patterns.
///
/// Note: Only tool discovery (list_tools) is filtered. Tool execution (call_tool)
/// is forwarded without validation, relying on the LLM not attempting to call
/// tools it cannot see.
#[derive(Clone)]
pub struct FilteringMcpProxy {
    wrapped_server: Arc<McpServer>,
    tool_filter: ToolFilter,
}

impl FilteringMcpProxy {
    pub fn new(wrapped_server: Arc<McpServer>, tool_filter: ToolFilter) -> Self {
        tracing::info!("Created FilteringMcpProxy wrapping McpServer");
        Self {
            wrapped_server,
            tool_filter,
        }
    }
}

impl ServerHandler for FilteringMcpProxy {
    /// Forward initialize request to wrapped server without modification.
    async fn initialize(
        &self,
        request: InitializeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<InitializeResult, McpError> {
        tracing::debug!("FilteringMcpProxy: Forwarding initialize request");
        <McpServer as ServerHandler>::initialize(&self.wrapped_server, request, context).await
    }

    /// Forward list_prompts request to wrapped server without modification.
    async fn list_prompts(
        &self,
        request: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListPromptsResult, McpError> {
        tracing::debug!("FilteringMcpProxy: Forwarding list_prompts request");
        <McpServer as ServerHandler>::list_prompts(&self.wrapped_server, request, context).await
    }

    /// Forward get_prompt request to wrapped server without modification.
    async fn get_prompt(
        &self,
        request: GetPromptRequestParam,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<GetPromptResult, McpError> {
        tracing::debug!(
            prompt_name = %request.name,
            "FilteringMcpProxy: Forwarding get_prompt request"
        );
        <McpServer as ServerHandler>::get_prompt(&self.wrapped_server, request, context).await
    }

    /// Filter list_tools to only return allowed tools.
    ///
    /// This is where tool filtering happens - tools are filtered during discovery
    /// based on the allow/deny regex patterns configured for this proxy.
    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParam>,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        tracing::debug!("FilteringMcpProxy: Filtering list_tools request");

        // Get all tools from wrapped server
        let result =
            <McpServer as ServerHandler>::list_tools(&self.wrapped_server, request, context)
                .await?;

        // Filter tools based on allow/deny patterns
        let total_tools = result.tools.len();
        let filtered_tools: Vec<Tool> = result
            .tools
            .into_iter()
            .filter(|tool| {
                let allowed = self.tool_filter.is_allowed(&tool.name);
                tracing::debug!(
                    tool_name = %tool.name,
                    allowed = allowed,
                    "FilteringMcpProxy: Tool filter evaluation"
                );
                allowed
            })
            .collect();

        let filtered_count = filtered_tools.len();
        tracing::info!(
            total_tools = total_tools,
            filtered_tools = filtered_count,
            removed_tools = total_tools - filtered_count,
            "FilteringMcpProxy: Filtered tool list"
        );

        Ok(ListToolsResult {
            tools: filtered_tools,
            next_cursor: result.next_cursor,
        })
    }

    /// Forward call_tool request to wrapped server without validation.
    ///
    /// Note: We do NOT validate tool names here. Tool filtering happens at
    /// discovery time (list_tools). The assumption is that the LLM will not
    /// attempt to call tools it cannot see in the list.
    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::debug!(
            tool_name = %request.name,
            "FilteringMcpProxy: Forwarding call_tool request"
        );
        <McpServer as ServerHandler>::call_tool(&self.wrapped_server, request, context).await
    }

    /// Forward get_info request to wrapped server without modification.
    fn get_info(&self) -> ServerInfo {
        tracing::debug!("FilteringMcpProxy: Forwarding get_info request");
        self.wrapped_server.get_info()
    }
}
