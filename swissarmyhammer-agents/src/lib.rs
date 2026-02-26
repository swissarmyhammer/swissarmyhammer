//! SwissArmyHammer Agents
//!
//! Core crate for parsing, validating, resolving, and storing agent definitions.
//! Agents define system prompts and execution contexts for subagent delegation.
//!
//! ## Overview
//!
//! Agents are defined as AGENT.md files with YAML frontmatter + Liquid markdown body.
//! The body IS the system prompt, with template support for shared partials.
//!
//! ## Resolution Precedence
//!
//! Agents are resolved from multiple sources with later sources overriding earlier:
//! 1. **Builtin** — embedded in the binary from `builtin/agents/`
//! 2. **Local** — `.agents/` or `.swissarmyhammer/agents/` in the project
//! 3. **User** — `~/.agents/` or `~/.swissarmyhammer/agents/`

pub mod agent;
pub mod agent_library;
pub mod agent_loader;
pub mod agent_resolver;
pub mod context;
pub mod error;
pub mod operations;
pub mod parse;
pub mod schema;
pub mod validation;

// Re-export key types
pub use agent::{Agent, AgentName, AgentResources, AgentSource};
pub use agent_library::AgentLibrary;
pub use agent_resolver::AgentResolver;
pub use context::AgentContext;
pub use error::AgentError;
pub use operations::{ListAgents, SearchAgent, UseAgent};
pub use parse::{parse_input, AgentOperation};
pub use schema::generate_agent_mcp_schema;

// Re-export Execute trait from operations crate
pub use swissarmyhammer_operations::{async_trait, Execute, ExecutionResult, Operation};
