//! Command modules for shelltool CLI.
//!
//! Each subcommand implementation lives in its own module:
//! - `serve`: MCP server over stdio
//! - `doctor`: Diagnostic checks
//! - `registry`: Init/deinit component registration
//! - `skill`: Skill resolution and deployment

pub mod doctor;
pub mod registry;
pub mod serve;
pub mod skill;
