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
    /// Create an executor based on agent configuration
    ///
    /// For LlamaAgent executors, an optional MCP server handle can be provided.
    /// If not provided, the executor will be created but may require MCP server
    /// to be started separately before initialization.
    ///
    /// # Arguments
    ///
    /// * `agent_config` - The agent configuration specifying which executor to use
    /// * `mcp_server` - Optional MCP server handle (required for LlamaAgent)
    ///
    /// # Returns
    ///
    /// Returns an initialized executor wrapped in a Box<dyn AgentExecutor>
    ///
    /// # Example
    ///
    /// ```no_run
    /// use swissarmyhammer_agent_executor::{AgentExecutorFactory, AgentExecutionContext};
    /// use swissarmyhammer_config::agent::AgentConfig;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let agent_config = AgentConfig::default();
    /// let mut executor = AgentExecutorFactory::create_executor(&agent_config, None).await?;
    ///
    /// // Execute a prompt
    /// let context = AgentExecutionContext::default();
    /// let response = executor.execute_prompt(
    ///     "You are a helpful assistant.".to_string(),
    ///     "What is 2+2?".to_string(),
    ///     &context
    /// ).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn create_executor(
        agent_config: &swissarmyhammer_config::agent::AgentConfig,
        mcp_server: agent_client_protocol::McpServer,
    ) -> ActionResult<Box<dyn AgentExecutor>> {
        match agent_config.executor_type() {
            AgentExecutorType::ClaudeCode => {
                tracing::info!("Creating ClaudeCode executor with MCP server");
                let mut executor = crate::claude::ClaudeCodeExecutor::new(mcp_server);
                executor.initialize().await?;
                Ok(Box::new(executor))
            }
            AgentExecutorType::LlamaAgent => {
                tracing::info!("Creating LlamaAgent executor with MCP server");

                // Extract LlamaAgent configuration from agent config
                let llama_config = match &agent_config.executor {
                    swissarmyhammer_config::agent::AgentExecutorConfig::LlamaAgent(config) => {
                        config.clone()
                    }
                    _ => {
                        return Err(crate::ActionError::ExecutionError(format!(
                            "Expected LlamaAgent configuration, but got {:?}",
                            agent_config.executor_type()
                        )))
                    }
                };

                // Create executor with MCP server
                let mut executor = crate::llama::LlamaAgentExecutor::new(llama_config, mcp_server);
                executor.initialize().await?;
                Ok(Box::new(executor))
            }
        }
    }
}
