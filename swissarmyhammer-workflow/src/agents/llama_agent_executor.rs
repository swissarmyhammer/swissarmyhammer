//! LlamaAgent executor adapter for SwissArmyHammer workflows
//!
//! This module provides adapters that implement the agent-executor AgentExecutor trait
//! and delegate to the actual implementations from agent-executor crate.

use async_trait::async_trait;
use swissarmyhammer_config::model::AgentExecutorType;
use swissarmyhammer_config::LlamaAgentConfig;

// Import types from agent-executor
use swissarmyhammer_agent_executor::llama::{
    LlamaAgentExecutor as AgentExecutorLlamaAgentExecutor,
    LlamaAgentExecutorWrapper as AgentExecutorLlamaAgentExecutorWrapper,
};
use swissarmyhammer_agent_executor::{AgentExecutor, AgentResponse};

/// Wrapper for LlamaAgentExecutor that implements the canonical AgentExecutor trait
pub struct LlamaAgentExecutor {
    inner: AgentExecutorLlamaAgentExecutor,
}

impl LlamaAgentExecutor {
    /// Create a new LlamaAgent executor with the given configuration and MCP server
    ///
    /// # Arguments
    ///
    /// * `config` - LlamaAgent configuration
    /// * `mcp_server` - MCP server configuration using agent-client-protocol types
    pub fn new(config: LlamaAgentConfig, mcp_server: agent_client_protocol::McpServer) -> Self {
        Self {
            inner: AgentExecutorLlamaAgentExecutor::new(config, mcp_server),
        }
    }
}

#[async_trait]
impl AgentExecutor for LlamaAgentExecutor {
    async fn initialize(&mut self) -> swissarmyhammer_agent_executor::ActionResult<()> {
        self.inner.initialize().await
    }

    async fn shutdown(&mut self) -> swissarmyhammer_agent_executor::ActionResult<()> {
        self.inner.shutdown().await
    }

    fn executor_type(&self) -> AgentExecutorType {
        self.inner.executor_type()
    }

    async fn execute_prompt(
        &self,
        system_prompt: String,
        rendered_prompt: String,
        context: &swissarmyhammer_agent_executor::AgentExecutionContext<'_>,
    ) -> swissarmyhammer_agent_executor::ActionResult<AgentResponse> {
        self.inner
            .execute_prompt(system_prompt, rendered_prompt, context)
            .await
    }
}

/// Wrapper for LlamaAgentExecutorWrapper that implements the canonical AgentExecutor trait.
///
/// # MCP Server Lifecycle
///
/// LlamaAgent requires an MCP (Model Context Protocol) server to function. The MCP server
/// provides tools and prompts that the agent can use during execution.
///
/// ## Who Starts the MCP Server?
///
/// The caller (typically the CLI layer) is responsible for starting the MCP server
/// **before** creating the executor. The workflow layer does NOT start infrastructure.
///
/// ## Constructor Usage
///
/// - [`new()`](Self::new): Creates executor without MCP server handle. The executor will
///   attempt to connect to an MCP server during `initialize()`. If no server is available,
///   initialization will fail with an error indicating the MCP server is required.
///
/// - [`new_with_mcp()`](Self::new_with_mcp): Creates executor with a pre-started MCP server handle.
///   This is the preferred method when you control the MCP server lifecycle (e.g., in CLI or tests).
///
/// ## Example Usage
///
/// ```rust,no_run
/// use swissarmyhammer_workflow::agents::{LlamaAgentExecutorWrapper, McpServerHandle};
/// use swissarmyhammer_agent_executor::llama::config::LlamaAgentConfig;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Start MCP server first (typically done by CLI)
/// // let mcp_handle = start_mcp_server(...).await?;
///
/// let config = LlamaAgentConfig::default();
///
/// // Option 1: With pre-started server (recommended)
/// // let mut executor = LlamaAgentExecutorWrapper::new_with_mcp(config.clone(), Some(mcp_handle));
///
/// // Option 2: Without server handle (will fail if server not running)
/// let mut executor = LlamaAgentExecutorWrapper::new(config);
///
/// // Initialize will fail if MCP server is not available
/// executor.initialize().await?;
/// # Ok(())
/// # }
/// ```
pub struct LlamaAgentExecutorWrapper {
    inner: AgentExecutorLlamaAgentExecutorWrapper,
}

impl LlamaAgentExecutorWrapper {
    /// Creates a new executor without an MCP server handle.
    ///
    /// The executor will attempt to connect to an MCP server during `initialize()`.
    /// If no MCP server is available, initialization will fail.
    ///
    /// # When to Use
    ///
    /// Use this constructor when you don't have direct access to the MCP server handle
    /// but know that a server is running (e.g., started elsewhere in your application).
    ///
    /// # Errors
    ///
    /// Calling `initialize()` will fail with an error if:
    /// - No MCP server is running
    /// - The MCP server is not accessible at the configured address
    /// - The MCP server connection fails
    ///
    /// # Arguments
    ///
    /// * `config` - LlamaAgent configuration
    /// * `mcp_server` - MCP server configuration using agent-client-protocol types
    pub fn new(config: LlamaAgentConfig, mcp_server: agent_client_protocol::McpServer) -> Self {
        Self {
            inner: AgentExecutorLlamaAgentExecutorWrapper::new(config, mcp_server),
        }
    }
}

#[async_trait]
impl AgentExecutor for LlamaAgentExecutorWrapper {
    async fn initialize(&mut self) -> swissarmyhammer_agent_executor::ActionResult<()> {
        self.inner.initialize().await
    }

    async fn shutdown(&mut self) -> swissarmyhammer_agent_executor::ActionResult<()> {
        self.inner.shutdown().await
    }

    fn executor_type(&self) -> AgentExecutorType {
        self.inner.executor_type()
    }

    async fn execute_prompt(
        &self,
        system_prompt: String,
        rendered_prompt: String,
        context: &swissarmyhammer_agent_executor::AgentExecutionContext<'_>,
    ) -> swissarmyhammer_agent_executor::ActionResult<AgentResponse> {
        self.inner
            .execute_prompt(system_prompt, rendered_prompt, context)
            .await
    }
}
