//! Command modules for code-context CLI.
//!
//! Each command is organized in its own module:
//! - `serve`: MCP server over stdio
//! - `doctor`: Diagnostic checks for setup and configuration
//! - `registry`: Init/deinit component registration
//! - `skill`: Skill resolution and deployment
//! - `ops`: CLI-to-MCP operation dispatch

pub mod doctor;
pub mod ops;
pub mod registry;
pub mod serve;
pub mod skill;
