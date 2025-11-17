//! Rule command - now dynamically generated from MCP tools
//!
//! The rule command (sah rule check, etc.) is now dynamically generated from
//! the rules_check and other rules_* MCP tools, not hand-coded here.
//!
//! This module exists only to provide the DESCRIPTION constant for the
//! dynamically generated command's help text.

/// Help text for the rule command
pub const DESCRIPTION: &str = include_str!("description.md");
