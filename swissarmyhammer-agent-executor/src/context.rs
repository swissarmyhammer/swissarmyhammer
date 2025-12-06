//! Agent execution context

use swissarmyhammer_config::model::{AgentExecutorType, ModelConfig};

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
    agent_config: &'a ModelConfig,
    /// Skip tool discovery for this execution (optimization for rule checking)
    skip_tools: bool,
}

impl<'a> AgentExecutionContext<'a> {
    /// Create a new agent execution context with the given configuration
    pub fn new(agent_config: &'a ModelConfig) -> Self {
        Self {
            agent_config,
            skip_tools: false,
        }
    }

    /// Create a new agent execution context optimized for rule checking (no tools)
    pub fn for_rule_checking(agent_config: &'a ModelConfig) -> Self {
        Self {
            agent_config,
            skip_tools: true,
        }
    }

    /// Get agent configuration
    pub fn agent_config(&self) -> &ModelConfig {
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

    /// Check if tools should be skipped for this execution
    pub fn skip_tools(&self) -> bool {
        self.skip_tools
    }
}
