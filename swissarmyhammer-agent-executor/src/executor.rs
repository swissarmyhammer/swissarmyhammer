//! Agent executor trait and factory

use crate::{ActionResult, AgentExecutionContext, AgentResponse};
use async_trait::async_trait;
use swissarmyhammer_config::agent::AgentExecutorType;

/// Agent executor trait for abstracting prompt execution across different AI backends
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    /// Execute a rendered prompt and return the response
    async fn execute_prompt(
        &self,
        system_prompt: String,
        rendered_prompt: String,
        context: &AgentExecutionContext<'_>,
    ) -> ActionResult<AgentResponse>;

    /// Get the executor type enum
    fn executor_type(&self) -> AgentExecutorType;

    /// Initialize the executor with configuration
    async fn initialize(&mut self) -> ActionResult<()>;

    /// Shutdown the executor and cleanup resources
    async fn shutdown(&mut self) -> ActionResult<()>;

    /// Check if the executor supports streaming responses
    fn supports_streaming(&self) -> bool {
        false // Default implementation returns false
    }
}

/// Factory for creating agent executors
pub struct AgentExecutorFactory;

impl AgentExecutorFactory {
    /// Create an executor based on the execution context
    pub async fn create_executor(
        context: &AgentExecutionContext<'_>,
    ) -> ActionResult<Box<dyn AgentExecutor>> {
        match context.executor_type() {
            AgentExecutorType::ClaudeCode => {
                Err(crate::ActionError::ExecutionError(
                    "ClaudeCode executor not available in agent-executor crate. Use swissarmyhammer-workflow.".to_string(),
                ))
            }
            AgentExecutorType::LlamaAgent => {
                // LlamaAgent requires MCP server to be started first
                // This factory is low-level and doesn't start MCP server
                // Use swissarmyhammer_workflow::AgentExecutorFactory instead, which handles MCP server lifecycle
                Err(crate::ActionError::ExecutionError(
                    "LlamaAgent executor requires MCP server. Use swissarmyhammer_workflow::AgentExecutorFactory instead of agent-executor's factory.".to_string(),
                ))
            }
        }
    }
}
