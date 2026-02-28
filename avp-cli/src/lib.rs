//! AVP (Agent Validator Protocol) - Claude Code hook processor.
//!
//! AVP provides a framework for processing Claude Code hooks with:
//! - Typed Input/Output structs for all 13 hook types
//! - Strategy pattern for pluggable hook processing
//! - Chain of Responsibility with Success/BlockingError starters
//! - JSON serialization and validation
//! - Validator execution via ACP agent
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use avp::context::AvpContext;
//! use avp::strategy::HookDispatcher;
//!
//! #[tokio::main]
//! async fn main() {
//!     // Initialize context (requires being in a git repository)
//!     let context = AvpContext::init().unwrap();
//!
//!     // Create dispatcher with the context
//!     let dispatcher = HookDispatcher::with_defaults(context);
//!
//!     let input = serde_json::json!({
//!         "session_id": "test123",
//!         "transcript_path": "/path/to/transcript.jsonl",
//!         "cwd": "/home/user",
//!         "permission_mode": "default",
//!         "hook_event_name": "PreToolUse",
//!         "tool_name": "Bash",
//!         "tool_input": {"command": "ls"}
//!     });
//!
//!     let (output, exit_code) = dispatcher.dispatch(input).await.unwrap();
//!     assert!(output.continue_execution);
//!     assert_eq!(exit_code, 0);
//! }
//! ```

use std::fmt;

mod cli;
pub use cli::{Cli, Commands, ModelAction};
pub mod doctor;
pub mod edit;
pub mod install;
pub mod logging;
pub mod model;
pub mod new;

/// Error type for AVP CLI operations.
#[derive(Debug)]
pub enum AvpCliError {
    /// File system error.
    Io(std::io::Error),
    /// Validation failure (e.g. invalid name, missing file).
    Validation(String),
}

impl fmt::Display for AvpCliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Validation(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for AvpCliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AvpCliError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// Re-export everything from avp-common for backwards compatibility
pub use avp_common::*;
