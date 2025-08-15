//! Abort management tools for MCP operations
//!
//! This module provides tools for managing workflow and prompt abortion through file-based signaling.
//! It replaces the brittle string-based "ABORT ERROR" detection system with a robust file-based
//! approach that works across process boundaries.
//!
//! ## File-Based Abort System
//!
//! The abort system creates a `.swissarmyhammer/.abort` file containing the abort reason.
//! This approach provides several advantages:
//!
//! - **Robust**: Works across process boundaries and different execution contexts
//! - **Atomic**: File creation is an atomic operation
//! - **Persistent**: Abort state persists until explicitly cleared
//! - **Language Agnostic**: Can be detected by any process regardless of implementation language
//! - **Testable**: Easy to test by creating/checking files in tests
//!
//! ## Integration Points
//!
//! The abort file is checked by:
//! - Workflow execution loops for immediate termination
//! - CLI error handling for proper exit codes  
//! - Process cleanup routines for graceful shutdown
//!
//! ## Tool Implementation Pattern
//!
//! Abort tools follow the standard MCP pattern with file operations:
//! ```rust,no_run
//! use std::fs;
//! use std::path::Path;
//!
//! fn example() -> std::io::Result<()> {
//!     let reason = "User cancelled operation";
//!     
//!     // Create abort file
//!     fs::write(".swissarmyhammer/.abort", reason)?;
//!
//!     // Check for abort file
//!     if Path::new(".swissarmyhammer/.abort").exists() {
//!         let reason = fs::read_to_string(".swissarmyhammer/.abort")?;
//!         // Handle abort...
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## Available Tools
//!
//! - **create**: Create abort file with reason to signal termination

pub mod create;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all abort-related tools with the registry
pub fn register_abort_tools(registry: &mut ToolRegistry) {
    registry.register(create::AbortCreateTool::new());
}
