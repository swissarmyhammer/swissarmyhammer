//! ACP (Agent Client Protocol) integration for workflows
//!
//! This module re-exports the unified agent interface from `swissarmyhammer_agent`.
//! See that crate for the full API documentation.
//!
//! # Example
//!
//! ```ignore
//! use swissarmyhammer_workflow::acp::{create_agent, execute_prompt, McpServerConfig};
//!
//! let config = ModelConfig::load("model.yaml")?;
//! let mut handle = create_agent(&config, None).await?;
//! let response = execute_prompt(&mut handle, None, "Hello!".to_string()).await?;
//! ```

pub use swissarmyhammer_agent::{
    create_agent, execute_prompt, AcpAgentHandle, AcpError, AcpResult, AgentResponse,
    AgentResponseType, McpServerConfig,
};
