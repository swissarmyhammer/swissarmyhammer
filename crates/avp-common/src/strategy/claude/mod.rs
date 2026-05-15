//! Claude Code specific strategy implementation.
//!
//! This module contains all Claude Code-specific types and logic:
//! - Input types that match Claude Code's hook JSON format
//! - Output types that match Claude Code's expected response format
//! - Transformation functions from AVP outputs to Claude format
//! - The ClaudeCodeHookStrategy implementation

pub mod input;
pub mod output;
pub mod strategy;
pub mod transform;

pub use input::{
    HookInput, NotificationInput, PermissionRequestInput, PostToolUseFailureInput,
    PostToolUseInput, PreCompactInput, PreToolUseInput, SessionEndInput, SessionStartInput,
    SetupInput, StopInput, SubagentStartInput, SubagentStopInput, UserPromptSubmitInput,
};
pub use output::{
    GenericOutput, HookOutput, HookSpecificOutput, PermissionBehavior, PermissionDecision,
    PermissionRequestDecision, PermissionRequestOutput, PostToolUseOutput, PreToolUseOutput,
    SessionStartOutput, StopOutput, UserPromptSubmitOutput,
};
pub use transform::{
    avp_permission_request_to_claude, avp_post_tool_use_failure_to_claude,
    avp_post_tool_use_to_claude, avp_pre_compact_to_claude, avp_pre_tool_use_to_claude,
    avp_session_end_to_claude, avp_session_start_to_claude, avp_setup_to_claude,
    avp_stop_to_claude, avp_subagent_start_to_claude, avp_subagent_stop_to_claude,
    avp_user_prompt_submit_to_claude, ClaudeGenericOutput, ClaudeHookOutput,
    ClaudeHookSpecificOutput, ClaudePermissionBehavior, ClaudePermissionDecision,
    ClaudePermissionRequestDecision, ClaudePermissionRequestOutput, ClaudePostToolUseOutput,
    ClaudePreToolUseOutput, ClaudeSessionStartOutput, ClaudeStopOutput,
    ClaudeUserPromptSubmitOutput,
};
