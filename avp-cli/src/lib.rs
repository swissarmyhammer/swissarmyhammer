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
//!
//! # Architecture
//!
//! ## Context
//!
//! The `AvpContext` manages the `.avp` directory structure and provides:
//! - Logging of hook events to `.avp/avp.log`
//! - Access to validator directories (builtin, user, project)
//!
//! ## Strategy Pattern
//!
//! The dispatcher routes hooks to agent-specific strategies:
//!
//! ```rust,no_run
//! use avp::context::AvpContext;
//! use avp::strategy::{HookDispatcher, ClaudeCodeHookStrategy};
//!
//! let context = AvpContext::init().unwrap();
//!
//! // Create dispatcher with Claude Code strategy
//! let dispatcher = HookDispatcher::with_defaults(context);
//!
//! // Or register custom strategies manually
//! // let mut dispatcher = HookDispatcher::new();
//! // dispatcher.register(ClaudeCodeHookStrategy::new(context));
//! ```

// Re-export everything from avp-common for backwards compatibility
pub use avp_common::*;
