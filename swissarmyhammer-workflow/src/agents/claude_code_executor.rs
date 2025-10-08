//! ClaudeCode executor adapter for SwissArmyHammer workflows
//!
//! This module provides adapters that bridge between the workflow-level
//! AgentExecutor trait and the agent-executor crate's ClaudeCodeExecutor.

use crate::actions::{
    ActionError, ActionResult, AgentExecutionContext, AgentExecutor, AgentResponse,
    AgentResponseType,
};
use async_trait::async_trait;
use swissarmyhammer_config::agent::AgentExecutorType;

// Import the agent-executor trait so we can call its methods
use swissarmyhammer_agent_executor::AgentExecutor as AgentExecutorTrait;

// Re-export the actual implementation from agent-executor crate
pub use swissarmyhammer_agent_executor::ClaudeCodeExecutor as AgentExecutorClaudeCodeExecutor;

/// Wrapper for ClaudeCodeExecutor that implements workflow's AgentExecutor trait
pub struct ClaudeCodeExecutor {
    inner: AgentExecutorClaudeCodeExecutor,
}

impl ClaudeCodeExecutor {
    /// Create a new ClaudeCode executor
    pub fn new() -> Self {
        Self {
            inner: AgentExecutorClaudeCodeExecutor::new(),
        }
    }
}

impl Default for ClaudeCodeExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentExecutor for ClaudeCodeExecutor {
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

/// Convert agent-executor error to workflow ActionError
fn convert_agent_executor_error(err: swissarmyhammer_agent_executor::ActionError) -> ActionError {
    match err {
        swissarmyhammer_agent_executor::ActionError::ExecutionError(msg) => {
            ActionError::ExecutionError(msg)
        }
        swissarmyhammer_agent_executor::ActionError::ClaudeError(msg) => {
            ActionError::ClaudeError(msg)
        }
        swissarmyhammer_agent_executor::ActionError::RateLimit { message, wait_time } => {
            ActionError::RateLimit { message, wait_time }
        }
        swissarmyhammer_agent_executor::ActionError::IoError(err) => ActionError::IoError(err),
        swissarmyhammer_agent_executor::ActionError::VariableError(msg) => {
            ActionError::ExecutionError(format!("Variable error: {}", msg))
        }
        swissarmyhammer_agent_executor::ActionError::ParseError(msg) => {
            ActionError::ExecutionError(format!("Parse error: {}", msg))
        }
        swissarmyhammer_agent_executor::ActionError::JsonError(err) => {
            ActionError::ExecutionError(format!("JSON error: {}", err))
        }
    }
}

/// Convert agent-executor response to workflow AgentResponse
fn convert_agent_response(
    response: swissarmyhammer_agent_executor::AgentResponse,
) -> AgentResponse {
    AgentResponse {
        content: response.content,
        metadata: response.metadata,
        response_type: convert_response_type(response.response_type),
    }
}

/// Convert agent-executor response type to workflow AgentResponseType
fn convert_response_type(
    response_type: swissarmyhammer_agent_executor::AgentResponseType,
) -> AgentResponseType {
    match response_type {
        swissarmyhammer_agent_executor::AgentResponseType::Success => AgentResponseType::Success,
        swissarmyhammer_agent_executor::AgentResponseType::Error => AgentResponseType::Error,
        swissarmyhammer_agent_executor::AgentResponseType::Partial => AgentResponseType::Partial,
    }
}
