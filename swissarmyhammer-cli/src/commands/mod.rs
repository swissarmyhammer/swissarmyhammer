//! Command modules for swissarmyhammer CLI
//!
//! Each command is organized in its own module with:
//! - Command logic implementation
//! - Help text from description.md files
//! - Following MCP tool patterns for documentation

pub mod doctor;
pub mod flow;
pub mod model;
pub mod prompt;
pub mod serve;
pub mod validate;
