//! Hook input types - re-exported from strategy/claude for backwards compatibility.
//!
//! These types are Claude Code-specific and are now defined in strategy/claude/input.rs.
//! This module re-exports them for backwards compatibility with existing code.

pub use crate::strategy::claude::input::{
    ConfigChangeInput, ElicitationInput, ElicitationResultInput, HookInput,
    InstructionsLoadedInput, NotificationInput, PermissionRequestInput, PostCompactInput,
    PostToolUseFailureInput, PostToolUseInput, PreCompactInput, PreToolUseInput, SessionEndInput,
    SessionStartInput, SetupInput, StopInput, SubagentStartInput, SubagentStopInput,
    TaskCompletedInput, TeammateIdleInput, UserPromptSubmitInput, WorktreeCreateInput,
    WorktreeRemoveInput,
};
