//! AVP (Agent Validator Protocol) - Claude Code hook processor.
//!
//! AVP provides a framework for processing Claude Code hooks with:
//! - Typed Input/Output structs for all 13 hook types
//! - Strategy pattern for pluggable hook processing
//! - Chain of Responsibility with Success/BlockingError starters
//! - JSON serialization and validation
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use avp::strategy::HookDispatcher;
//!
//! let dispatcher = HookDispatcher::with_defaults();
//! let input = serde_json::json!({
//!     "session_id": "test123",
//!     "transcript_path": "/path/to/transcript.jsonl",
//!     "cwd": "/home/user",
//!     "permission_mode": "default",
//!     "hook_event_name": "PreToolUse",
//!     "tool_name": "Bash",
//!     "tool_input": {"command": "ls"}
//! });
//!
//! let (output, exit_code) = dispatcher.dispatch(input).unwrap();
//! assert!(output.continue_execution);
//! assert_eq!(exit_code, 0);
//! ```
//!
//! # Architecture
//!
//! ## Strategy Pattern
//!
//! The dispatcher routes hooks to agent-specific strategies:
//!
//! ```rust,no_run
//! use avp::strategy::{HookDispatcher, ClaudeCodeHookStrategy};
//!
//! // Create dispatcher with Claude Code strategy
//! let dispatcher = HookDispatcher::with_defaults();
//!
//! // Or register custom strategies
//! let mut dispatcher = HookDispatcher::new();
//! dispatcher.register(ClaudeCodeHookStrategy::new());
//! ```

pub mod chain;
pub mod error;
pub mod hooks;
pub mod strategy;
pub mod types;

// Re-export commonly used types at the crate root
pub use error::{AvpError, ChainError, ValidationError};
pub use strategy::HookDispatcher;
pub use types::{HookInput, HookOutput, HookType};
