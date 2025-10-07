//! Agent executor trait and factory

use crate::{ActionResult, AgentExecutionContext, AgentResponse};
use async_trait::async_trait;
use swissarmyhammer_config::agent::{AgentExecutorConfig, AgentExecutorType};

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
                tracing::info!("Using LlamaAgent with singleton pattern");
                let agent_config = context.agent_config();
                let llama_config = match &agent_config.executor {
                    AgentExecutorConfig::LlamaAgent(config) => config.clone(),
                    _ => {
                        return Err(crate::ActionError::ExecutionError(
                            "Expected LlamaAgent configuration".to_string(),
                        ))
                    }
                };
                let mut executor = crate::llama::LlamaAgentExecutorWrapper::new(llama_config);
                executor.initialize().await?;
                Ok(Box::new(executor))
            }
        }
    }
}
