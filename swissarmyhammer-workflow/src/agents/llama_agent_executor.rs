//! LlamaAgent executor adapter for SwissArmyHammer workflows
//!
//! This module provides adapters that implement the agent-executor AgentExecutor trait
//! and delegate to the actual implementations from agent-executor crate.

use async_trait::async_trait;
use swissarmyhammer_config::agent::AgentExecutorType;
use swissarmyhammer_config::LlamaAgentConfig;

// Import types from agent-executor
use swissarmyhammer_agent_executor::llama::{
    LlamaAgentExecutor as AgentExecutorLlamaAgentExecutor,
    LlamaAgentExecutorWrapper as AgentExecutorLlamaAgentExecutorWrapper,
};
use swissarmyhammer_agent_executor::{AgentExecutor, AgentResponse};

// Re-export McpServerHandle for use by workflow factory
pub use swissarmyhammer_agent_executor::llama::executor::McpServerHandle;

/// Wrapper for LlamaAgentExecutor that implements the canonical AgentExecutor trait
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

/// Wrapper for LlamaAgentExecutorWrapper that implements the canonical AgentExecutor trait
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
    pub fn new_with_mcp(config: LlamaAgentConfig, mcp_server: Option<McpServerHandle>) -> Self {
        Self {
            inner: AgentExecutorLlamaAgentExecutorWrapper::new_with_mcp(config, mcp_server),
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
