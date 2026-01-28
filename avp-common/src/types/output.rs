//! Hook output types - re-exported from strategy/claude for backwards compatibility.
//!
//! These types are Claude Code-specific and are now defined in strategy/claude/output.rs.
//! This module re-exports them for backwards compatibility with existing code.

pub use crate::strategy::claude::output::{
    GenericOutput, HookOutput, HookSpecificOutput, PermissionBehavior, PermissionDecision,
    PermissionRequestDecision, PermissionRequestOutput, PostToolUseOutput, PreToolUseOutput,
    SessionStartOutput, StopOutput, UserPromptSubmitOutput,
};
