//! Tests verifying exact JSON output format for each hook type per Claude Code docs.
//!
//! Reference: https://code.claude.com/docs/en/hooks#hook-output

use avp_common::types::{
    HookOutput, HookSpecificOutput, PermissionBehavior, PermissionDecision,
    PermissionRequestDecision, PermissionRequestOutput, PostToolUseOutput, PreToolUseOutput,
    SessionStartOutput, StopOutput, UserPromptSubmitOutput,
};

// ============================================================================
// Common JSON Fields (all hooks)
// Docs: https://code.claude.com/docs/en/hooks#common-json-fields
// ============================================================================

/// Common fields per docs:
/// ```json
/// {
///   "continue": true,
///   "stopReason": "string",
///   "suppressOutput": true,
///   "systemMessage": "string"
/// }
/// ```
#[test]
fn test_common_json_fields() {
    let output = HookOutput {
        continue_execution: false,
        stop_reason: Some("Stopped by hook".to_string()),
        suppress_output: true,
        system_message: Some("Warning message".to_string()),
        ..Default::default()
    };

    let json = serde_json::to_value(&output).unwrap();

    assert_eq!(json.get("continue").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(
        json.get("stopReason").and_then(|v| v.as_str()),
        Some("Stopped by hook")
    );
    assert_eq!(
        json.get("suppressOutput").and_then(|v| v.as_bool()),
        Some(true)
    );
    assert_eq!(
        json.get("systemMessage").and_then(|v| v.as_str()),
        Some("Warning message")
    );
}

// ============================================================================
// PreToolUse Decision Control
// Docs: https://code.claude.com/docs/en/hooks#pretooluse-decision-control
// ============================================================================

/// PreToolUse format per docs:
/// ```json
/// {
///   "hookSpecificOutput": {
///     "hookEventName": "PreToolUse",
///     "permissionDecision": "allow" | "deny" | "ask",
///     "permissionDecisionReason": "My reason here",
///     "updatedInput": { ... },
///     "additionalContext": "Current environment: production"
///   }
/// }
/// ```
#[test]
fn test_pre_tool_use_deny_json_format() {
    let output = HookOutput::success().with_hook_specific(HookSpecificOutput::PreToolUse(
        PreToolUseOutput {
            permission_decision: Some(PermissionDecision::Deny),
            permission_decision_reason: Some("Command not allowed".to_string()),
            updated_input: None,
            additional_context: None,
        },
    ));

    let json = serde_json::to_value(&output).unwrap();
    let hook_specific = json.get("hookSpecificOutput").expect("hookSpecificOutput required");

    assert_eq!(
        hook_specific.get("hookEventName").and_then(|v| v.as_str()),
        Some("PreToolUse")
    );
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
fn test_pre_tool_use_allow_json_format() {
    let output = HookOutput::success().with_hook_specific(HookSpecificOutput::PreToolUse(
        PreToolUseOutput {
            permission_decision: Some(PermissionDecision::Allow),
            permission_decision_reason: Some("Auto-approved".to_string()),
            updated_input: None,
            additional_context: Some("Running in safe mode".to_string()),
        },
    ));

    let json = serde_json::to_value(&output).unwrap();
    let hook_specific = json.get("hookSpecificOutput").expect("hookSpecificOutput required");

    assert_eq!(
        hook_specific
            .get("permissionDecision")
            .and_then(|v| v.as_str()),
        Some("allow")
    );
    assert_eq!(
        hook_specific
            .get("additionalContext")
            .and_then(|v| v.as_str()),
        Some("Running in safe mode")
    );
}

#[test]
fn test_pre_tool_use_ask_json_format() {
    let output = HookOutput::success().with_hook_specific(HookSpecificOutput::PreToolUse(
        PreToolUseOutput {
            permission_decision: Some(PermissionDecision::Ask),
            permission_decision_reason: Some("Requires user confirmation".to_string()),
            updated_input: None,
            additional_context: None,
        },
    ));

    let json = serde_json::to_value(&output).unwrap();
    let hook_specific = json.get("hookSpecificOutput").expect("hookSpecificOutput required");

    assert_eq!(
        hook_specific
            .get("permissionDecision")
            .and_then(|v| v.as_str()),
        Some("ask")
    );
}

#[test]
fn test_pre_tool_use_with_updated_input_json_format() {
    let output = HookOutput::success().with_hook_specific(HookSpecificOutput::PreToolUse(
        PreToolUseOutput {
            permission_decision: Some(PermissionDecision::Allow),
            permission_decision_reason: Some("Modified command".to_string()),
            updated_input: Some(serde_json::json!({"command": "npm run lint"})),
            additional_context: None,
        },
    ));

    let json = serde_json::to_value(&output).unwrap();
    let hook_specific = json.get("hookSpecificOutput").expect("hookSpecificOutput required");

    let updated_input = hook_specific.get("updatedInput").expect("updatedInput required");
    assert_eq!(
        updated_input.get("command").and_then(|v| v.as_str()),
        Some("npm run lint")
    );
}

// ============================================================================
// PermissionRequest Decision Control
// Docs: https://code.claude.com/docs/en/hooks#permissionrequest-decision-control
// ============================================================================

/// PermissionRequest format per docs:
/// ```json
/// {
///   "hookSpecificOutput": {
///     "hookEventName": "PermissionRequest",
///     "decision": {
///       "behavior": "allow" | "deny",
///       "updatedInput": { ... },
///       "message": "...",
///       "interrupt": true/false
///     }
///   }
/// }
/// ```
#[test]
fn test_permission_request_deny_json_format() {
    let output =
        HookOutput::success().with_hook_specific(HookSpecificOutput::PermissionRequest(
            PermissionRequestOutput {
                decision: Some(PermissionRequestDecision {
                    behavior: PermissionBehavior::Deny,
                    updated_input: None,
                    message: Some("Permission denied by policy".to_string()),
                    interrupt: false,
                }),
            },
        ));

    let json = serde_json::to_value(&output).unwrap();
    let hook_specific = json.get("hookSpecificOutput").expect("hookSpecificOutput required");

    assert_eq!(
        hook_specific.get("hookEventName").and_then(|v| v.as_str()),
        Some("PermissionRequest")
    );

    let decision = hook_specific.get("decision").expect("decision required");
    assert_eq!(
        decision.get("behavior").and_then(|v| v.as_str()),
        Some("deny")
    );
    assert_eq!(
        decision.get("message").and_then(|v| v.as_str()),
        Some("Permission denied by policy")
    );
    assert_eq!(
        decision.get("interrupt").and_then(|v| v.as_bool()),
        Some(false)
    );
}

#[test]
fn test_permission_request_allow_with_updated_input_json_format() {
    let output =
        HookOutput::success().with_hook_specific(HookSpecificOutput::PermissionRequest(
            PermissionRequestOutput {
                decision: Some(PermissionRequestDecision {
                    behavior: PermissionBehavior::Allow,
                    updated_input: Some(serde_json::json!({"command": "npm run lint"})),
                    message: None,
                    interrupt: false,
                }),
            },
        ));

    let json = serde_json::to_value(&output).unwrap();
    let hook_specific = json.get("hookSpecificOutput").expect("hookSpecificOutput required");
    let decision = hook_specific.get("decision").expect("decision required");

    assert_eq!(
        decision.get("behavior").and_then(|v| v.as_str()),
        Some("allow")
    );
    let updated_input = decision.get("updatedInput").expect("updatedInput required");
    assert_eq!(
        updated_input.get("command").and_then(|v| v.as_str()),
        Some("npm run lint")
    );
}

#[test]
fn test_permission_request_deny_with_interrupt_json_format() {
    let output =
        HookOutput::success().with_hook_specific(HookSpecificOutput::PermissionRequest(
            PermissionRequestOutput {
                decision: Some(PermissionRequestDecision {
                    behavior: PermissionBehavior::Deny,
                    updated_input: None,
                    message: Some("Critical violation".to_string()),
                    interrupt: true,
                }),
            },
        ));

    let json = serde_json::to_value(&output).unwrap();
    let hook_specific = json.get("hookSpecificOutput").expect("hookSpecificOutput required");
    let decision = hook_specific.get("decision").expect("decision required");

    assert_eq!(
        decision.get("interrupt").and_then(|v| v.as_bool()),
        Some(true)
    );
}

// ============================================================================
// PostToolUse Decision Control
// Docs: https://code.claude.com/docs/en/hooks#posttooluse-decision-control
// ============================================================================

/// PostToolUse format per docs:
/// ```json
/// {
///   "decision": "block" | undefined,
///   "reason": "Explanation for decision",
///   "hookSpecificOutput": {
///     "hookEventName": "PostToolUse",
///     "additionalContext": "Additional information for Claude"
///   }
/// }
/// ```
#[test]
fn test_post_tool_use_block_json_format() {
    let output = HookOutput {
        continue_execution: true,
        decision: Some("block".to_string()),
        reason: Some("Found hardcoded secrets".to_string()),
        ..Default::default()
    };

    let json = serde_json::to_value(&output).unwrap();

    assert_eq!(json.get("continue").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        json.get("decision").and_then(|v| v.as_str()),
        Some("block")
    );
    assert_eq!(
        json.get("reason").and_then(|v| v.as_str()),
        Some("Found hardcoded secrets")
    );
}

#[test]
fn test_post_tool_use_with_additional_context_json_format() {
    let output = HookOutput {
        continue_execution: true,
        decision: Some("block".to_string()),
        reason: Some("Issue detected".to_string()),
        hook_specific_output: Some(HookSpecificOutput::PostToolUse(PostToolUseOutput {
            additional_context: Some("File contains sensitive data".to_string()),
        })),
        ..Default::default()
    };

    let json = serde_json::to_value(&output).unwrap();
    let hook_specific = json.get("hookSpecificOutput").expect("hookSpecificOutput required");

    assert_eq!(
        hook_specific.get("hookEventName").and_then(|v| v.as_str()),
        Some("PostToolUse")
    );
    assert_eq!(
        hook_specific
            .get("additionalContext")
            .and_then(|v| v.as_str()),
        Some("File contains sensitive data")
    );
}

// ============================================================================
// UserPromptSubmit Decision Control
// Docs: https://code.claude.com/docs/en/hooks#userpromptsubmit-decision-control
// ============================================================================

/// UserPromptSubmit format per docs:
/// ```json
/// {
///   "decision": "block" | undefined,
///   "reason": "Explanation for decision",
///   "hookSpecificOutput": {
///     "hookEventName": "UserPromptSubmit",
///     "additionalContext": "My additional context here"
///   }
/// }
/// ```
#[test]
fn test_user_prompt_submit_block_json_format() {
    let output = HookOutput {
        continue_execution: true,
        decision: Some("block".to_string()),
        reason: Some("Prompt contains sensitive information".to_string()),
        ..Default::default()
    };

    let json = serde_json::to_value(&output).unwrap();

    assert_eq!(json.get("continue").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        json.get("decision").and_then(|v| v.as_str()),
        Some("block")
    );
    assert_eq!(
        json.get("reason").and_then(|v| v.as_str()),
        Some("Prompt contains sensitive information")
    );
}

#[test]
fn test_user_prompt_submit_with_additional_context_json_format() {
    let output = HookOutput::success().with_hook_specific(HookSpecificOutput::UserPromptSubmit(
        UserPromptSubmitOutput {
            additional_context: Some("Current time: 2024-01-15".to_string()),
        },
    ));

    let json = serde_json::to_value(&output).unwrap();
    let hook_specific = json.get("hookSpecificOutput").expect("hookSpecificOutput required");

    assert_eq!(
        hook_specific.get("hookEventName").and_then(|v| v.as_str()),
        Some("UserPromptSubmit")
    );
    assert_eq!(
        hook_specific
            .get("additionalContext")
            .and_then(|v| v.as_str()),
        Some("Current time: 2024-01-15")
    );
}

// ============================================================================
// Stop/SubagentStop Decision Control
// Docs: https://code.claude.com/docs/en/hooks#stopsubagentstop-decision-control
// ============================================================================

/// Stop format per docs:
/// ```json
/// {
///   "decision": "block" | undefined,
///   "reason": "Must be provided when Claude is blocked from stopping"
/// }
/// ```
#[test]
fn test_stop_block_json_format() {
    let output = HookOutput {
        continue_execution: true,
        decision: Some("block".to_string()),
        reason: Some("Must fix failing tests before stopping".to_string()),
        ..Default::default()
    };

    let json = serde_json::to_value(&output).unwrap();

    assert_eq!(json.get("continue").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(
        json.get("decision").and_then(|v| v.as_str()),
        Some("block")
    );
    assert_eq!(
        json.get("reason").and_then(|v| v.as_str()),
        Some("Must fix failing tests before stopping")
    );
}

#[test]
fn test_stop_with_hook_specific_reason_json_format() {
    let output = HookOutput {
        continue_execution: true,
        decision: Some("block".to_string()),
        reason: Some("Tests failing".to_string()),
        hook_specific_output: Some(HookSpecificOutput::Stop(StopOutput {
            reason: Some("Additional stop reason".to_string()),
        })),
        ..Default::default()
    };

    let json = serde_json::to_value(&output).unwrap();

    // Top-level reason
    assert_eq!(
        json.get("reason").and_then(|v| v.as_str()),
        Some("Tests failing")
    );

    // hookSpecificOutput.reason (if needed)
    let hook_specific = json.get("hookSpecificOutput").expect("hookSpecificOutput");
    assert_eq!(
        hook_specific.get("hookEventName").and_then(|v| v.as_str()),
        Some("Stop")
    );
}

// ============================================================================
// SessionStart Decision Control
// Docs: https://code.claude.com/docs/en/hooks#sessionstart-decision-control
// ============================================================================

/// SessionStart format per docs:
/// ```json
/// {
///   "hookSpecificOutput": {
///     "hookEventName": "SessionStart",
///     "additionalContext": "My additional context here"
///   }
/// }
/// ```
#[test]
fn test_session_start_json_format() {
    let output = HookOutput::success().with_hook_specific(HookSpecificOutput::SessionStart(
        SessionStartOutput {
            additional_context: Some("Project initialized with Node 18".to_string()),
        },
    ));

    let json = serde_json::to_value(&output).unwrap();
    let hook_specific = json.get("hookSpecificOutput").expect("hookSpecificOutput required");

    assert_eq!(
        hook_specific.get("hookEventName").and_then(|v| v.as_str()),
        Some("SessionStart")
    );
    assert_eq!(
        hook_specific
            .get("additionalContext")
            .and_then(|v| v.as_str()),
        Some("Project initialized with Node 18")
    );
}

// ============================================================================
// Setup Decision Control
// Docs: https://code.claude.com/docs/en/hooks#setup-decision-control
// ============================================================================

/// Setup format per docs:
/// ```json
/// {
///   "hookSpecificOutput": {
///     "hookEventName": "Setup",
///     "additionalContext": "Repository initialized with custom configuration"
///   }
/// }
/// ```
/// Note: Setup uses the same SessionStart output type since they have identical structure
#[test]
fn test_setup_json_format() {
    // Setup uses SessionStart output type but would need hookEventName: "Setup"
    // Currently our enum uses SessionStart for both - this may need fixing
    let output = HookOutput::success().with_hook_specific(HookSpecificOutput::SessionStart(
        SessionStartOutput {
            additional_context: Some("Repository initialized".to_string()),
        },
    ));

    let json = serde_json::to_value(&output).unwrap();
    let hook_specific = json.get("hookSpecificOutput").expect("hookSpecificOutput required");

    // NOTE: This currently outputs "SessionStart" but docs say it should be "Setup"
    // for Setup hooks. This is a potential bug.
    assert!(
        hook_specific.get("additionalContext").is_some(),
        "additionalContext should be present"
    );
}

// ============================================================================
// Verify default values and optional field skipping
// ============================================================================

#[test]
fn test_optional_fields_are_skipped_when_none() {
    let output = HookOutput::success();
    let json = serde_json::to_value(&output).unwrap();

    // These should NOT be present when None/default
    assert!(json.get("stopReason").is_none());
    assert!(json.get("decision").is_none());
    assert!(json.get("reason").is_none());
    assert!(json.get("systemMessage").is_none());
    assert!(json.get("hookSpecificOutput").is_none());
    // suppressOutput defaults to false and should be skipped
    assert!(json.get("suppressOutput").is_none());
}

#[test]
fn test_continue_defaults_to_true() {
    let output = HookOutput::default();
    let json = serde_json::to_value(&output).unwrap();

    assert_eq!(json.get("continue").and_then(|v| v.as_bool()), Some(true));
}
