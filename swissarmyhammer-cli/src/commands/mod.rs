//! Command modules for swissarmyhammer CLI
//!
//! Each command is organized in its own module with:
//! - Command logic implementation
//! - Help text from description.md files
//! - Following MCP tool patterns for documentation

pub mod agent;
pub mod doctor;
pub mod flow;
pub mod prompt;
pub mod rule;
pub mod serve;
pub mod validate;
