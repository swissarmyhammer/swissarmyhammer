//! Claude Code-specific output transformation.
//!
//! This module transforms agent-agnostic AVP output types into
//! Claude Code-specific JSON formats per the Claude Code Hooks documentation.
//!
//! See: https://code.claude.com/docs/en/hooks

use crate::types::{
    AvpNotificationOutput, AvpPermissionRequestOutput, AvpPostToolUseFailureOutput,
    AvpPostToolUseOutput, AvpPreCompactOutput, AvpPreToolUseOutput, AvpSessionEndOutput,
    AvpSessionStartOutput, AvpSetupOutput, AvpStopOutput, AvpSubagentStartOutput,
    AvpSubagentStopOutput, AvpUserPromptSubmitOutput,
};

// Re-export output types with Claude-prefixed aliases for backwards compatibility
pub use super::output::{
    GenericOutput as ClaudeGenericOutput, HookOutput as ClaudeHookOutput,
    HookSpecificOutput as ClaudeHookSpecificOutput, PermissionBehavior as ClaudePermissionBehavior,
    PermissionDecision as ClaudePermissionDecision,
    PermissionRequestDecision as ClaudePermissionRequestDecision,
    PermissionRequestOutput as ClaudePermissionRequestOutput,
    PostToolUseOutput as ClaudePostToolUseOutput, PreToolUseOutput as ClaudePreToolUseOutput,
    SessionStartOutput as ClaudeSessionStartOutput, StopOutput as ClaudeStopOutput,
    UserPromptSubmitOutput as ClaudeUserPromptSubmitOutput,
};

// Also import the canonical names for use in this module
use super::output::{
    HookOutput, HookSpecificOutput, PermissionBehavior, PermissionDecision,
    PermissionRequestDecision, PermissionRequestOutput, PreToolUseOutput,
};

// ============================================================================
// Transformations from AVP outputs to Claude outputs
// ============================================================================

/// Transform AVP PreToolUse output to Claude format.
///
/// Claude Code PreToolUse blocking uses:
/// - Exit code 0 (so JSON is parsed)
/// - `hookSpecificOutput.permissionDecision: "deny"`
/// - `hookSpecificOutput.permissionDecisionReason` explains why
pub fn avp_pre_tool_use_to_claude(avp: AvpPreToolUseOutput) -> (HookOutput, i32) {
    if avp.allow {
        (HookOutput::success(), 0)
    } else {
        let output = HookOutput {
            continue_execution: true, // Exit code 0 so JSON is parsed
            hook_specific_output: Some(HookSpecificOutput::PreToolUse(PreToolUseOutput {
                permission_decision: Some(PermissionDecision::Deny),
                permission_decision_reason: avp.deny_reason,
                ..Default::default()
            })),
            system_message: avp.base.system_message,
            suppress_output: avp.base.suppress_output,
            ..Default::default()
        };
        (output, 0)
    }
}

/// Transform AVP PermissionRequest output to Claude format.
///
/// Claude Code PermissionRequest blocking uses:
/// - Exit code 0 (so JSON is parsed)
/// - `hookSpecificOutput.decision.behavior: "deny"`
/// - `hookSpecificOutput.decision.message` explains why
pub fn avp_permission_request_to_claude(avp: AvpPermissionRequestOutput) -> (HookOutput, i32) {
    if avp.grant {
        (HookOutput::success(), 0)
    } else {
        let output = HookOutput {
            continue_execution: true, // Exit code 0 so JSON is parsed
            hook_specific_output: Some(HookSpecificOutput::PermissionRequest(
                PermissionRequestOutput {
                    decision: Some(PermissionRequestDecision {
                        behavior: PermissionBehavior::Deny,
                        message: avp.deny_reason,
                        updated_input: None,
                        interrupt: false,
                    }),
                },
            )),
            system_message: avp.base.system_message,
            suppress_output: avp.base.suppress_output,
            ..Default::default()
        };
        (output, 0)
    }
}

/// Transform AVP PostToolUse output to Claude format.
///
/// Claude Code PostToolUse blocking uses:
/// - Exit code 0 (so JSON is parsed)
/// - `decision: "block"` to flag the tool result
/// - `reason` provides feedback to Claude
/// - `continue: true` because tool already ran
pub fn avp_post_tool_use_to_claude(avp: AvpPostToolUseOutput) -> (HookOutput, i32) {
    if !avp.flagged {
        (HookOutput::success(), 0)
    } else {
        let output = HookOutput {
            continue_execution: true, // Tool already ran, we're flagging the result
            decision: Some("block".to_string()),
            reason: avp.feedback,
            system_message: avp.base.system_message,
            suppress_output: avp.base.suppress_output,
            ..Default::default()
        };
        (output, 0)
    }
}

/// Transform AVP PostToolUseFailure output to Claude format.
pub fn avp_post_tool_use_failure_to_claude(avp: AvpPostToolUseFailureOutput) -> (HookOutput, i32) {
    if !avp.flagged {
        (HookOutput::success(), 0)
    } else {
        let output = HookOutput {
            continue_execution: true,
            decision: Some("block".to_string()),
            reason: avp.feedback,
            system_message: avp.base.system_message,
            suppress_output: avp.base.suppress_output,
            ..Default::default()
        };
        (output, 0)
    }
}

/// Transform AVP Stop output to Claude format.
///
/// Claude Code Stop hook blocking:
/// - Exit code 0 (so JSON is parsed)
/// - `continue: true` (Claude must continue, cannot stop)
/// - `decision: "block"` + `reason` tells Claude why
pub fn avp_stop_to_claude(avp: AvpStopOutput) -> (HookOutput, i32) {
    if avp.allow_stop {
        (HookOutput::success(), 0)
    } else {
        let reason = avp.block_reason.clone();
        let output = HookOutput {
            continue_execution: true, // MUST continue, can't stop
            stop_reason: avp.block_reason,
            decision: Some("block".to_string()),
            reason,
            system_message: avp.base.system_message,
            suppress_output: avp.base.suppress_output,
            ..Default::default()
        };
        (output, 0)
    }
}

/// Transform AVP SubagentStop output to Claude format.
pub fn avp_subagent_stop_to_claude(avp: AvpSubagentStopOutput) -> (HookOutput, i32) {
    if avp.allow_stop {
        (HookOutput::success(), 0)
    } else {
        let reason = avp.block_reason.clone();
        let output = HookOutput {
            continue_execution: true,
            stop_reason: avp.block_reason,
            decision: Some("block".to_string()),
            reason,
            system_message: avp.base.system_message,
            suppress_output: avp.base.suppress_output,
            ..Default::default()
        };
        (output, 0)
    }
}

/// Transform AVP UserPromptSubmit output to Claude format.
///
/// Claude Code UserPromptSubmit blocking:
/// - Exit code 0 (so JSON is parsed)
/// - `decision: "block"` prevents the prompt
/// - `reason` is shown to user (not added to context)
pub fn avp_user_prompt_submit_to_claude(avp: AvpUserPromptSubmitOutput) -> (HookOutput, i32) {
    if avp.allow {
        (HookOutput::success(), 0)
    } else {
        let output = HookOutput {
            continue_execution: true, // Exit code 0 so JSON is parsed
            decision: Some("block".to_string()),
            reason: avp.block_reason,
            system_message: avp.base.system_message,
            suppress_output: avp.base.suppress_output,
            ..Default::default()
        };
        (output, 0)
    }
}

/// Transform AVP SessionStart output to Claude format.
pub fn avp_session_start_to_claude(avp: AvpSessionStartOutput) -> (HookOutput, i32) {
    let output = HookOutput {
        continue_execution: avp.base.should_continue,
        stop_reason: avp.base.stop_reason,
        system_message: avp.base.system_message,
        suppress_output: avp.base.suppress_output,
        ..Default::default()
    };
    let exit_code = if avp.base.should_continue { 0 } else { 2 };
    (output, exit_code)
}

/// Transform AVP SessionEnd output to Claude format.
pub fn avp_session_end_to_claude(avp: AvpSessionEndOutput) -> (HookOutput, i32) {
    let output = HookOutput {
        continue_execution: avp.base.should_continue,
        stop_reason: avp.base.stop_reason,
        system_message: avp.base.system_message,
        suppress_output: avp.base.suppress_output,
        ..Default::default()
    };
    let exit_code = if avp.base.should_continue { 0 } else { 2 };
    (output, exit_code)
}

/// Transform AVP Notification output to Claude format.
pub fn avp_notification_to_claude(avp: AvpNotificationOutput) -> (HookOutput, i32) {
    let output = HookOutput {
        continue_execution: avp.base.should_continue,
        stop_reason: avp.base.stop_reason,
        system_message: avp.base.system_message,
        suppress_output: avp.base.suppress_output,
        ..Default::default()
    };
    let exit_code = if avp.base.should_continue { 0 } else { 2 };
    (output, exit_code)
}

/// Transform AVP SubagentStart output to Claude format.
pub fn avp_subagent_start_to_claude(avp: AvpSubagentStartOutput) -> (HookOutput, i32) {
    let output = HookOutput {
        continue_execution: avp.base.should_continue,
        stop_reason: avp.base.stop_reason,
        system_message: avp.base.system_message,
        suppress_output: avp.base.suppress_output,
        ..Default::default()
    };
    let exit_code = if avp.base.should_continue { 0 } else { 2 };
    (output, exit_code)
}

/// Transform AVP PreCompact output to Claude format.
pub fn avp_pre_compact_to_claude(avp: AvpPreCompactOutput) -> (HookOutput, i32) {
    let output = HookOutput {
        continue_execution: avp.base.should_continue,
        stop_reason: avp.base.stop_reason,
        system_message: avp.base.system_message,
        suppress_output: avp.base.suppress_output,
        ..Default::default()
    };
    let exit_code = if avp.base.should_continue { 0 } else { 2 };
    (output, exit_code)
}

/// Transform AVP Setup output to Claude format.
pub fn avp_setup_to_claude(avp: AvpSetupOutput) -> (HookOutput, i32) {
    let output = HookOutput {
        continue_execution: avp.base.should_continue,
        stop_reason: avp.base.stop_reason,
        system_message: avp.base.system_message,
        suppress_output: avp.base.suppress_output,
        ..Default::default()
    };
    let exit_code = if avp.base.should_continue { 0 } else { 2 };
    (output, exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AvpPostToolUseOutput, AvpPreToolUseOutput, AvpStopOutput};

    #[test]
    fn test_pre_tool_use_allow_to_claude() {
        let avp = AvpPreToolUseOutput::allow();
        let (output, exit_code) = avp_pre_tool_use_to_claude(avp);

        assert!(output.continue_execution);
        assert!(output.hook_specific_output.is_none());
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_pre_tool_use_deny_to_claude() {
        let avp = AvpPreToolUseOutput::deny("Command not allowed");
        let (output, exit_code) = avp_pre_tool_use_to_claude(avp);

        assert!(output.continue_execution); // Exit 0 so JSON parsed
        assert_eq!(exit_code, 0);

        let json = serde_json::to_value(&output).unwrap();
        let hook_specific = json.get("hookSpecificOutput").unwrap();
        assert_eq!(
            hook_specific
                .get("permissionDecision")
                .and_then(|v| v.as_str()),
            Some("deny")
        );
        assert_eq!(
            hook_specific
                .get("permissionDecisionReason")
                .and_then(|v| v.as_str()),
            Some("Command not allowed")
        );
    }

    #[test]
    fn test_post_tool_use_accept_to_claude() {
        let avp = AvpPostToolUseOutput::accept();
        let (output, exit_code) = avp_post_tool_use_to_claude(avp);

        assert!(output.continue_execution);
        assert!(output.decision.is_none());
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_post_tool_use_flag_to_claude() {
        let avp = AvpPostToolUseOutput::flag("Found hardcoded secrets");
        let (output, exit_code) = avp_post_tool_use_to_claude(avp);

        assert!(output.continue_execution);
        assert_eq!(output.decision.as_deref(), Some("block"));
        assert_eq!(output.reason.as_deref(), Some("Found hardcoded secrets"));
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_stop_allow_to_claude() {
        let avp = AvpStopOutput::allow();
        let (output, exit_code) = avp_stop_to_claude(avp);

        assert!(output.continue_execution);
        assert!(output.decision.is_none());
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_stop_block_to_claude() {
        let avp = AvpStopOutput::block("Must fix tests before stopping");
        let (output, exit_code) = avp_stop_to_claude(avp);

        assert!(output.continue_execution); // Force continuation
        assert_eq!(output.decision.as_deref(), Some("block"));
        assert_eq!(
            output.reason.as_deref(),
            Some("Must fix tests before stopping")
        );
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_claude_hook_output_json_format() {
        let avp = AvpPreToolUseOutput::deny("Denied by validator");
        let (output, _) = avp_pre_tool_use_to_claude(avp);
        let json = serde_json::to_string(&output).unwrap();

        // Verify Claude Code expected format
        assert!(json.contains("\"continue\":true"));
        assert!(json.contains("\"hookSpecificOutput\""));
        assert!(json.contains("\"permissionDecision\":\"deny\""));
    }
}
