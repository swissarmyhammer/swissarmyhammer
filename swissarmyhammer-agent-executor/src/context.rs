//! Agent execution context

use swissarmyhammer_config::agent::{AgentConfig, AgentExecutorType};

/// Agent execution context for prompt execution
///
/// This is a simplified context that can be used without depending on
/// WorkflowTemplateContext. The workflow crate will provide a richer
/// context type that wraps this.
///
/// The lifetime parameter 'a represents the lifetime of borrowed data
/// that may be held by the context.
#[derive(Debug)]
pub struct AgentExecutionContext<'a> {
    /// Agent configuration
    agent_config: &'a AgentConfig,
}

impl<'a> AgentExecutionContext<'a> {
    /// Create a new agent execution context with the given configuration
    pub fn new(agent_config: &'a AgentConfig) -> Self {
        Self { agent_config }
    }

    /// Get agent configuration
    pub fn agent_config(&self) -> &AgentConfig {
        self.agent_config
    }

    /// Get executor type
    pub fn executor_type(&self) -> AgentExecutorType {
        self.agent_config.executor_type()
    }

    /// Check if quiet mode is enabled
    pub fn quiet(&self) -> bool {
        self.agent_config.quiet
    }
}
