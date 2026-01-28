//! Type definitions for Claude Code hook inputs and outputs.

mod avp_output;
mod common;
mod input;
mod output;

pub use avp_output::{
    AvpNotificationOutput, AvpOutputBase, AvpPermissionRequestOutput, AvpPostToolUseFailureOutput,
    AvpPostToolUseOutput, AvpPreCompactOutput, AvpPreToolUseOutput, AvpSessionEndOutput,
    AvpSessionStartOutput, AvpSetupOutput, AvpStopOutput, AvpSubagentStartOutput,
    AvpSubagentStopOutput, AvpUserPromptSubmitOutput, ValidatorBlock,
};
pub use common::{CommonInput, HookType};
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
