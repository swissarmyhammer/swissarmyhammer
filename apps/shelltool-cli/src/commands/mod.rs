//! Command modules for shelltool CLI.
//!
//! Each subcommand implementation lives in its own module:
//! - `serve`: MCP server over stdio
//! - `doctor`: Diagnostic checks
//! - `registry`: Init/deinit profile + component registration
//! - `ops`: Schema-driven shell operation dispatch (run/list/grep/get/kill)

pub mod doctor;
pub mod ops;
pub mod registry;
pub mod serve;
