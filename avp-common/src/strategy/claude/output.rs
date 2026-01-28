//! Claude Code hook output types.
//!
//! These types represent the JSON output format that Claude Code expects from hooks.
//! They are specific to Claude Code's hook system.

use serde::{Deserialize, Serialize};

use crate::types::HookType;

/// Common output structure for all hooks.
///
/// This follows the Claude Code hook output format:
/// - `continue`: Whether Claude should continue after hook execution (default: true)
/// - `stopReason`: Message shown when continue is false
/// - `decision`: For tool hooks, "block" to prevent the tool call
/// - `reason`: Explanation for the decision (shown to Claude when blocking)
/// - `hookSpecificOutput`: Hook-type-specific output fields
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    /// Whether to continue execution (default: true).
    #[serde(default = "default_continue", rename = "continue")]
    pub continue_execution: bool,

    /// Reason for stopping (only relevant if continue is false).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,

    /// Decision for tool hooks: "block" to prevent tool call.
    /// Used for PostToolUse to provide feedback to Claude.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,

    /// Reason for the decision (shown to Claude when blocking).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    /// Whether to suppress output display.
    #[serde(default, skip_serializing_if = "is_false")]
    pub suppress_output: bool,

    /// System message to inject into context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_message: Option<String>,

    /// Hook-specific output fields.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hook_specific_output: Option<HookSpecificOutput>,
}

fn default_continue() -> bool {
    true
}

fn is_false(b: &bool) -> bool {
    !*b
}

impl Default for HookOutput {
    fn default() -> Self {
        Self {
            continue_execution: true, // Default to allow continuation
            stop_reason: None,
            decision: None,
            reason: None,
            suppress_output: false,
            system_message: None,
            hook_specific_output: None,
        }
    }
}

impl HookOutput {
    /// Create a new success output that allows continuation.
    pub fn success() -> Self {
        Self {
            continue_execution: true,
            ..Default::default()
        }
    }

    /// Create a blocking error output that stops execution.
    ///
    /// This is used for hooks like Stop/SubagentStop where blocking
    /// prevents Claude from stopping.
    pub fn blocking_error(reason: impl Into<String>) -> Self {
        let reason_str = reason.into();
        Self {
            continue_execution: false,
            stop_reason: Some(reason_str.clone()),
            decision: Some("block".to_string()),
            reason: Some(reason_str),
            ..Default::default()
        }
    }

    /// Create a Stop hook blocking output.
    ///
    /// For Stop hooks, "blocking" means preventing Claude from stopping.
    /// This forces Claude to continue working and address the issues.
    /// - `continue: true` = Claude must continue, cannot stop
    /// - `decision: "block"` + `reason` = explains why stop was blocked
    pub fn stop_block(reason: impl Into<String>) -> Self {
        let reason_str = reason.into();
        Self {
            continue_execution: true, // MUST continue - can't stop
            stop_reason: Some(reason_str.clone()),
            decision: Some("block".to_string()),
            reason: Some(reason_str),
            ..Default::default()
        }
    }

    /// Create a PostToolUse blocking output.
    ///
    /// This uses the Claude Code format for PostToolUse blocking:
    /// - `decision: "block"` to indicate the tool result should be flagged
    /// - `reason` provides feedback to Claude about what went wrong
    /// - `continue: true` because the tool already ran, we're just flagging it
    ///
    /// Note: For PostToolUse, exit code 2 means "Shows stderr to Claude (tool already ran)"
    pub fn post_tool_use_block(reason: impl Into<String>) -> Self {
        Self {
            continue_execution: true, // Tool already ran, we're flagging the result
            decision: Some("block".to_string()),
            reason: Some(reason.into()),
            ..Default::default()
        }
    }

    /// Add a system message to the output.
    pub fn with_system_message(mut self, message: impl Into<String>) -> Self {
        self.system_message = Some(message.into());
        self
    }

    /// Suppress output display.
    pub fn with_suppress_output(mut self) -> Self {
        self.suppress_output = true;
        self
    }

    /// Add hook-specific output.
    pub fn with_hook_specific(mut self, output: HookSpecificOutput) -> Self {
        self.hook_specific_output = Some(output);
        self
    }

    /// Create a PreToolUse deny output (Claude Code format).
    ///
    /// Per Claude Code docs, PreToolUse blocking uses:
    /// - `hookSpecificOutput.permissionDecision: "deny"` to block the tool
    /// - `hookSpecificOutput.permissionDecisionReason` provides feedback to Claude
    /// - `continue: true` so JSON is parsed (exit code 0)
    pub fn pre_tool_use_deny(reason: impl Into<String>) -> Self {
        Self {
            continue_execution: true, // Required for JSON parsing
            hook_specific_output: Some(HookSpecificOutput::PreToolUse(PreToolUseOutput {
                permission_decision: Some(PermissionDecision::Deny),
                permission_decision_reason: Some(reason.into()),
                ..Default::default()
            })),
            ..Default::default()
        }
    }
}

/// Hook-specific output fields based on hook type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "hookEventName")]
pub enum HookSpecificOutput {
    /// PreToolUse hook-specific output.
    PreToolUse(PreToolUseOutput),
    /// PermissionRequest hook-specific output.
    PermissionRequest(PermissionRequestOutput),
    /// PostToolUse hook-specific output.
    PostToolUse(PostToolUseOutput),
    /// UserPromptSubmit hook-specific output.
    UserPromptSubmit(UserPromptSubmitOutput),
    /// Stop/SubagentStop hook-specific output.
    Stop(StopOutput),
    /// SessionStart/Setup hook-specific output.
    SessionStart(SessionStartOutput),
    /// Generic additional context output for other hooks.
    Generic(GenericOutput),
}

/// PreToolUse hook-specific output for permission decisions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PreToolUseOutput {
    /// Permission decision: "allow", "deny", or "ask".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision: Option<PermissionDecision>,

    /// Reason for the permission decision.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_decision_reason: Option<String>,

    /// Modified tool input (if allowed with changes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<serde_json::Value>,

    /// Additional context to provide to Claude.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

/// Permission decision options for PreToolUse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionDecision {
    /// Allow the tool call to proceed.
    Allow,
    /// Deny the tool call.
    Deny,
    /// Ask the user for permission.
    Ask,
}

/// PermissionRequest hook-specific output.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequestOutput {
    /// The decision for the permission request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<PermissionRequestDecision>,
}

/// Decision structure for PermissionRequest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequestDecision {
    /// Behavior: "allow" or "deny".
    pub behavior: PermissionBehavior,

    /// Modified input if allowing with changes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_input: Option<serde_json::Value>,

    /// Message to display.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Whether to interrupt execution.
    #[serde(default)]
    pub interrupt: bool,
}

/// Permission behavior options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PermissionBehavior {
    /// Allow the action.
    Allow,
    /// Deny the action.
    Deny,
}

/// PostToolUse hook-specific output.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PostToolUseOutput {
    /// Additional context to provide to Claude.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

/// UserPromptSubmit hook-specific output.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UserPromptSubmitOutput {
    /// Additional context to inject.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

/// Stop/SubagentStop hook-specific output.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct StopOutput {
    /// Reason for blocking (required when blocking stop).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// SessionStart/Setup hook-specific output.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionStartOutput {
    /// Additional context to add at session start.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

/// Generic output for hooks that only need additional context.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenericOutput {
    /// The hook event name.
    pub hook_event_name: HookType,

    /// Additional context to provide.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_context: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_output_success_serialization() {
        let output = HookOutput::success();
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"continue\":true"));
    }

    #[test]
    fn test_hook_output_blocking_error() {
        let output = HookOutput::blocking_error("Not allowed");
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"continue\":false"));
        assert!(json.contains("\"stopReason\":\"Not allowed\""));
        // Also includes Claude Code format fields
        assert!(json.contains("\"decision\":\"block\""));
        assert!(json.contains("\"reason\":\"Not allowed\""));
    }

    #[test]
    fn test_hook_output_post_tool_use_block() {
        let output = HookOutput::post_tool_use_block("Found hardcoded secrets");
        let json = serde_json::to_string(&output).unwrap();
        // PostToolUse blocking uses decision + reason (Claude Code format)
        assert!(json.contains("\"decision\":\"block\""));
        assert!(json.contains("\"reason\":\"Found hardcoded secrets\""));
        // Should NOT have continue:false (tool already ran, we're just flagging it)
        assert!(json.contains("\"continue\":true"));
    }

    #[test]
    fn test_pre_tool_use_output() {
        let output = HookOutput::success().with_hook_specific(HookSpecificOutput::PreToolUse(
            PreToolUseOutput {
                permission_decision: Some(PermissionDecision::Allow),
                permission_decision_reason: Some("Auto-approved".to_string()),
                ..Default::default()
            },
        ));
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"permissionDecision\":\"allow\""));
    }

    #[test]
    fn test_hook_output_stop_block() {
        let output = HookOutput::stop_block("Must fix issues before stopping");
        let json = serde_json::to_string(&output).unwrap();
        // Stop block: continue=true (Claude cannot stop), decision=block
        assert!(json.contains("\"continue\":true"));
        assert!(json.contains("\"decision\":\"block\""));
        assert!(json.contains("\"reason\":\"Must fix issues before stopping\""));
        assert!(json.contains("\"stopReason\":\"Must fix issues before stopping\""));
    }

    #[test]
    fn test_hook_output_with_system_message() {
        let output = HookOutput::success().with_system_message("Additional context for Claude");
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"systemMessage\":\"Additional context for Claude\""));
    }

    #[test]
    fn test_hook_output_with_suppress_output() {
        let output = HookOutput::success().with_suppress_output();
        assert!(output.suppress_output);
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"suppressOutput\":true"));
    }

    #[test]
    fn test_hook_output_with_hook_specific() {
        let output =
            HookOutput::success().with_hook_specific(HookSpecificOutput::Stop(StopOutput {
                reason: Some("Validator blocked stop".to_string()),
            }));
        assert!(output.hook_specific_output.is_some());
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"hookEventName\":\"Stop\""));
        assert!(json.contains("\"reason\":\"Validator blocked stop\""));
    }

}
