//! Type definitions for Claude Code hook inputs and outputs.

mod common;
mod input;
mod output;

pub use common::{CommonInput, HookType};
pub use input::{
    HookInput, NotificationInput, PermissionRequestInput, PostToolUseFailureInput,
    PostToolUseInput, PreCompactInput, PreToolUseInput, SessionEndInput, SessionStartInput,
    SetupInput, StopInput, SubagentStartInput, SubagentStopInput, UserPromptSubmitInput,
};
pub use output::{
    GenericOutput, HookOutput, HookSpecificOutput, LinkOutput, PermissionBehavior,
    PermissionDecision, PermissionRequestDecision, PermissionRequestOutput, PostToolUseOutput,
    PreToolUseOutput, SessionStartOutput, StopOutput, UserPromptSubmitOutput,
};
