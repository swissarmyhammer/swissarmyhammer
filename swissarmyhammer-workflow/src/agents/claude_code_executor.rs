//! ClaudeCode executor adapter for SwissArmyHammer workflows
//!
//! This module provides an adapter that implements the canonical AgentExecutor trait
//! and delegates to the agent-executor crate's implementation.

use async_trait::async_trait;
use swissarmyhammer_config::agent::AgentExecutorType;

// Import types from agent-executor
use swissarmyhammer_agent_executor::{
    AgentExecutor, AgentResponse, ClaudeCodeExecutor as AgentExecutorClaudeCodeExecutor,
};

/// Wrapper for ClaudeCodeExecutor that adapts contexts between workflow and agent-executor
pub struct ClaudeCodeExecutor {
    inner: AgentExecutorClaudeCodeExecutor,
}

impl ClaudeCodeExecutor {
    /// Create a new ClaudeCode executor with MCP server configuration
    ///
    /// # Arguments
    ///
    /// * `mcp_server` - MCP server configuration using agent-client-protocol types
    pub fn new(mcp_server: agent_client_protocol::McpServer) -> Self {
        Self {
            inner: AgentExecutorClaudeCodeExecutor::new(mcp_server),
        }
    }
}

#[async_trait]
impl AgentExecutor for ClaudeCodeExecutor {
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
