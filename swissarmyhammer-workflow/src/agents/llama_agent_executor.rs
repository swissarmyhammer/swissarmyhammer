//! LlamaAgent executor adapter for SwissArmyHammer workflows
//!
//! This module provides adapters that bridge between the workflow-level
//! AgentExecutor trait and the agent-executor crate's implementations.

use crate::actions::{
    ActionError, ActionResult, AgentExecutionContext, AgentExecutor, AgentResponse,
    AgentResponseType,
};
use async_trait::async_trait;
use swissarmyhammer_config::agent::AgentExecutorType;
use swissarmyhammer_config::LlamaAgentConfig;

// Import the agent-executor trait so we can call its methods
use swissarmyhammer_agent_executor::AgentExecutor as AgentExecutorTrait;

// Re-export the actual implementations from agent-executor crate
pub use swissarmyhammer_agent_executor::llama::executor::McpServerHandle;
pub use swissarmyhammer_agent_executor::llama::{
    LlamaAgentExecutor as AgentExecutorLlamaAgentExecutor,
    LlamaAgentExecutorWrapper as AgentExecutorLlamaAgentExecutorWrapper,
};

/// Wrapper for LlamaAgentExecutor that implements workflow's AgentExecutor trait
pub struct LlamaAgentExecutor {
    inner: AgentExecutorLlamaAgentExecutor,
}

impl LlamaAgentExecutor {
    /// Create a new LlamaAgent executor with the given configuration and optional MCP server
    pub fn new(config: LlamaAgentConfig, mcp_server: Option<McpServerHandle>) -> Self {
        Self {
            inner: AgentExecutorLlamaAgentExecutor::new(config, mcp_server),
        }
    }
}

#[async_trait]
impl AgentExecutor for LlamaAgentExecutor {
    async fn initialize(&mut self) -> ActionResult<()> {
        self.inner
            .initialize()
            .await
            .map_err(convert_agent_executor_error)
    }

    async fn shutdown(&mut self) -> ActionResult<()> {
        self.inner
            .shutdown()
            .await
            .map_err(convert_agent_executor_error)
    }

    fn executor_type(&self) -> AgentExecutorType {
        self.inner.executor_type()
    }

    async fn execute_prompt(
        &self,
        system_prompt: String,
        rendered_prompt: String,
        context: &AgentExecutionContext<'_>,
    ) -> ActionResult<AgentResponse> {
        // Convert workflow context to agent-executor context
        let agent_config = context.agent_config();
        let agent_exec_context =
            swissarmyhammer_agent_executor::AgentExecutionContext::new(&agent_config);

        // Execute using the inner executor
        let response = self
            .inner
            .execute_prompt(system_prompt, rendered_prompt, &agent_exec_context)
            .await
            .map_err(convert_agent_executor_error)?;

        // Convert response back to workflow type
        Ok(convert_agent_response(response))
    }
}

/// Wrapper for LlamaAgentExecutorWrapper that implements workflow's AgentExecutor trait
pub struct LlamaAgentExecutorWrapper {
    inner: AgentExecutorLlamaAgentExecutorWrapper,
}

impl LlamaAgentExecutorWrapper {
    /// Create a new wrapper instance without MCP server (will fail on initialize)
    pub fn new(config: LlamaAgentConfig) -> Self {
        Self {
            inner: AgentExecutorLlamaAgentExecutorWrapper::new(config),
        }
    }

    /// Create a new wrapper instance with pre-started MCP server
    ///
    /// # Arguments
    ///
    /// * `config` - LlamaAgent configuration
    /// * `mcp_server` - Optional pre-started MCP server handle from the workflow layer
    ///
    /// # Architecture Note
    ///
    /// This constructor is used by the workflow layer after starting the MCP server.
    /// This ensures proper separation: workflow manages infrastructure, executor handles execution.
    pub fn new_with_mcp(config: LlamaAgentConfig, mcp_server: Option<McpServerHandle>) -> Self {
        Self {
            inner: AgentExecutorLlamaAgentExecutorWrapper::new_with_mcp(config, mcp_server),
        }
    }
}

#[async_trait]
impl AgentExecutor for LlamaAgentExecutorWrapper {
    async fn initialize(&mut self) -> ActionResult<()> {
        self.inner
            .initialize()
            .await
            .map_err(convert_agent_executor_error)
    }

    async fn shutdown(&mut self) -> ActionResult<()> {
        self.inner
            .shutdown()
            .await
            .map_err(convert_agent_executor_error)
    }

    fn executor_type(&self) -> AgentExecutorType {
        self.inner.executor_type()
    }

    async fn execute_prompt(
        &self,
        system_prompt: String,
        rendered_prompt: String,
        context: &AgentExecutionContext<'_>,
    ) -> ActionResult<AgentResponse> {
        // Convert workflow context to agent-executor context
        let agent_config = context.agent_config();
        let agent_exec_context =
            swissarmyhammer_agent_executor::AgentExecutionContext::new(&agent_config);

        // Execute using the inner executor
        let response = self
            .inner
            .execute_prompt(system_prompt, rendered_prompt, &agent_exec_context)
            .await
            .map_err(convert_agent_executor_error)?;

        // Convert response back to workflow type
        Ok(convert_agent_response(response))
    }
}

/// Convert agent-executor error to workflow error
fn convert_agent_executor_error(err: swissarmyhammer_agent_executor::ActionError) -> ActionError {
    use swissarmyhammer_agent_executor::ActionError as AEError;
    match err {
        AEError::ClaudeError(msg) => ActionError::ClaudeError(msg),
        AEError::VariableError(msg) => ActionError::VariableError(msg),
        AEError::ParseError(msg) => ActionError::ParseError(msg),
        AEError::ExecutionError(msg) => ActionError::ExecutionError(msg),
        AEError::IoError(err) => ActionError::IoError(err),
        AEError::JsonError(err) => ActionError::JsonError(err),
        AEError::RateLimit { message, wait_time } => ActionError::RateLimit { message, wait_time },
    }
}

/// Convert agent-executor response to workflow response
fn convert_agent_response(
    response: swissarmyhammer_agent_executor::AgentResponse,
) -> AgentResponse {
    use swissarmyhammer_agent_executor::AgentResponseType as AEType;
    let response_type = match response.response_type {
        AEType::Success => AgentResponseType::Success,
        AEType::Partial => AgentResponseType::Partial,
        AEType::Error => AgentResponseType::Error,
    };

    AgentResponse {
        content: response.content,
        metadata: response.metadata,
        response_type,
    }
}
