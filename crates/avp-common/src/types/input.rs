//! Hook input types - re-exported from strategy/claude for backwards compatibility.
//!
//! These types are Claude Code-specific and are now defined in strategy/claude/input.rs.
//! This module re-exports them for backwards compatibility with existing code.

pub use crate::strategy::claude::input::{
    HookInput, NotificationInput, PermissionRequestInput, PostToolUseFailureInput,
    PostToolUseInput, PreCompactInput, PreToolUseInput, SessionEndInput, SessionStartInput,
    SetupInput, StopInput, SubagentStartInput, SubagentStopInput, UserPromptSubmitInput,
};
