//! Tests for [the hook output shapes and how they become a decision](super).

use super::*;

// =====================================================================
// HookOutput interpretation tests
// =====================================================================

#[test]
fn test_interpret_output_continue_false() {
    let output = HookOutput {
        should_continue: false,
        stop_reason: Some("Build failed".into()),
        ..Default::default()
    };
    let decision = interpret_output(&output, HookEventKind::PreToolUse);
    assert!(matches!(
        decision,
        HookDecision::Cancel { reason } if reason == "Build failed"
    ));
}

#[test]
fn test_interpret_output_pre_tool_use_deny() {
    let output = HookOutputBuilder::new()
        .with_hook_specific_output(HookSpecificOutput::PreToolUse {
            permission_decision: Some("deny".into()),
            permission_decision_reason: Some("Not allowed".into()),
            updated_input: None,
            additional_context: None,
        })
        .build();
    let decision = interpret_output(&output, HookEventKind::PreToolUse);
    assert!(matches!(
        decision,
        HookDecision::Block { reason } if reason == "Not allowed"
    ));
}

#[test]
fn test_interpret_output_pre_tool_use_allow() {
    let output = HookOutputBuilder::new()
        .with_hook_specific_output(HookSpecificOutput::PreToolUse {
            permission_decision: Some("allow".into()),
            permission_decision_reason: None,
            updated_input: None,
            additional_context: None,
        })
        .build();
    let decision = interpret_output(&output, HookEventKind::PreToolUse);
    assert!(matches!(decision, HookDecision::Allow));
}

#[test]
fn test_interpret_output_stop_block_is_should_continue() {
    let output = HookOutputBuilder::new()
        .with_decision(HookDecisionValue::Block)
        .with_reason("Tests not passing")
        .build();
    let decision = interpret_output(&output, HookEventKind::Stop);
    assert!(matches!(
        decision,
        HookDecision::ShouldContinue { reason } if reason == "Tests not passing"
    ));
}

#[test]
fn test_interpret_output_user_prompt_block() {
    let output = HookOutputBuilder::new()
        .with_decision(HookDecisionValue::Block)
        .with_reason("Prompt rejected")
        .build();
    let decision = interpret_output(&output, HookEventKind::UserPromptSubmit);
    assert!(matches!(
        decision,
        HookDecision::Block { reason } if reason == "Prompt rejected"
    ));
}

#[test]
fn test_interpret_output_additional_context() {
    let output = HookOutputBuilder::new()
        .with_additional_context("Extra info")
        .build();
    let decision = interpret_output(&output, HookEventKind::SessionStart);
    assert!(matches!(
        decision,
        HookDecision::AllowWithContext { context } if context == "Extra info"
    ));
}

#[test]
fn test_interpret_output_empty_is_allow() {
    let output = HookOutput::default();
    let decision = interpret_output(&output, HookEventKind::PreToolUse);
    assert!(matches!(decision, HookDecision::Allow));
}

// =====================================================================
// HookOutputBuilder tests
// =====================================================================

#[test]
fn test_hook_output_builder_defaults_match_hook_output_default() {
    let built = HookOutputBuilder::new().build();
    assert!(built.should_continue);
    assert!(!built.suppress_output);
    assert!(built.stop_reason.is_none());
    assert!(built.system_message.is_none());
    assert!(built.decision.is_none());
    assert!(built.reason.is_none());
    assert!(built.hook_specific_output.is_none());
    assert!(built.additional_context.is_none());
}

#[test]
fn test_hook_output_builder_sets_every_optional_field() {
    let output = HookOutputBuilder::new()
        .with_stop_reason("Build failed")
        .with_system_message("careful")
        .with_decision(HookDecisionValue::Block)
        .with_reason("Blocked")
        .with_hook_specific_output(HookSpecificOutput::Stop {
            reason: Some("stop reason".into()),
        })
        .with_additional_context("extra")
        .build();

    assert_eq!(output.stop_reason.as_deref(), Some("Build failed"));
    assert_eq!(output.system_message.as_deref(), Some("careful"));
    assert_eq!(output.decision, Some(HookDecisionValue::Block));
    assert_eq!(output.reason.as_deref(), Some("Blocked"));
    assert!(matches!(
        output.hook_specific_output,
        Some(HookSpecificOutput::Stop { reason: Some(ref r) }) if r == "stop reason"
    ));
    assert_eq!(output.additional_context.as_deref(), Some("extra"));
}

// =====================================================================
// Prompt/agent response interpretation
// =====================================================================

#[test]
fn test_prompt_response_ok_true() {
    let response = PromptHookResponse {
        ok: true,
        reason: None,
    };
    let decision = interpret_prompt_response(&response, HookEventKind::PreToolUse);
    assert!(matches!(decision, HookDecision::Allow));
}

#[test]
fn test_prompt_response_ok_false_blocks() {
    let response = PromptHookResponse {
        ok: false,
        reason: Some("Forbidden".into()),
    };
    let decision = interpret_prompt_response(&response, HookEventKind::PreToolUse);
    assert!(matches!(
        decision,
        HookDecision::Block { reason } if reason == "Forbidden"
    ));
}

#[test]
fn test_prompt_response_ok_false_stop_is_should_continue() {
    let response = PromptHookResponse {
        ok: false,
        reason: Some("Tests not complete".into()),
    };
    let decision = interpret_prompt_response(&response, HookEventKind::Stop);
    assert!(matches!(
        decision,
        HookDecision::ShouldContinue { reason } if reason == "Tests not complete"
    ));
}

#[test]
fn test_prompt_response_ok_false_post_tool_feeds_context() {
    let response = PromptHookResponse {
        ok: false,
        reason: Some("Lint warning detected".into()),
    };
    let decision = interpret_prompt_response(&response, HookEventKind::PostToolUse);
    assert!(matches!(
        decision,
        HookDecision::AllowWithContext { context } if context == "Lint warning detected"
    ));
}

/// An event kind that can neither block nor give its reason to the agent
/// loses the answer of the hook, so the refusal becomes a plain `Allow`.
#[test]
fn test_prompt_response_ok_false_on_a_silent_event_allows() {
    let response = PromptHookResponse {
        ok: false,
        reason: Some("Nobody reads this".into()),
    };
    let decision = interpret_prompt_response(&response, HookEventKind::Notification);
    assert!(matches!(decision, HookDecision::Allow));
}

#[test]
fn test_prompt_response_ok_false_post_tool_failure_feeds_context() {
    let response = PromptHookResponse {
        ok: false,
        reason: Some("Failure noted".into()),
    };
    let decision = interpret_prompt_response(&response, HookEventKind::PostToolUseFailure);
    assert!(matches!(
        decision,
        HookDecision::AllowWithContext { context } if context == "Failure noted"
    ));
}

// =====================================================================
// HookDecisionValue enum
// =====================================================================

#[test]
fn test_hook_decision_value_serialization() {
    assert_eq!(
        serde_json::to_string(&HookDecisionValue::Allow).unwrap(),
        "\"allow\""
    );
    assert_eq!(
        serde_json::to_string(&HookDecisionValue::Block).unwrap(),
        "\"block\""
    );
    assert_eq!(
        serde_json::to_string(&HookDecisionValue::Ask).unwrap(),
        "\"ask\""
    );
}

#[test]
fn test_hook_decision_value_deserialization() {
    assert_eq!(
        serde_json::from_str::<HookDecisionValue>("\"allow\"").unwrap(),
        HookDecisionValue::Allow
    );
    assert_eq!(
        serde_json::from_str::<HookDecisionValue>("\"block\"").unwrap(),
        HookDecisionValue::Block
    );
    assert_eq!(
        serde_json::from_str::<HookDecisionValue>("\"ask\"").unwrap(),
        HookDecisionValue::Ask
    );
}

#[test]
fn test_hook_output_with_decision_value() {
    let json = r#"{ "continue": true, "decision": "block", "reason": "Blocked by hook" }"#;
    let output: HookOutput = serde_json::from_str(json).unwrap();
    assert_eq!(output.decision, Some(HookDecisionValue::Block));
    assert_eq!(output.reason, Some("Blocked by hook".to_string()));
}

#[test]
fn test_pre_tool_use_deny_decision_parses_with_reason() {
    let json = r#"{
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": "Too risky"
        }"#;
    let output: HookSpecificOutput = serde_json::from_str(json).unwrap();
    match output {
        HookSpecificOutput::PreToolUse {
            permission_decision,
            permission_decision_reason,
            ..
        } => {
            assert_eq!(permission_decision, Some("deny".to_string()));
            assert_eq!(permission_decision_reason, Some("Too risky".to_string()));
        }
        other => panic!("Expected PreToolUse, got {:?}", other),
    }
}

/// A hook only ever sets one field of `hookSpecificOutput` — the rest are
/// absent, not `null`. Before `#[serde(default)]` was added to the sibling
/// `Option` fields, `PreToolUse` required every key to be present, so a
/// hook that emitted only `permissionDecision` failed the whole
/// `HookOutput` parse and the caller fell back to `HookDecision::Allow`.
/// A hook that meant to block ran, reported success, and did nothing.
#[test]
fn partial_pre_tool_use_hook_specific_output_deserializes() {
    let json = r#"{
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny"
        }"#;
    let output: HookSpecificOutput = serde_json::from_str(json)
        .expect("a hookSpecificOutput with only permissionDecision set must deserialize");
    match output {
        HookSpecificOutput::PreToolUse {
            permission_decision,
            permission_decision_reason,
            updated_input,
            additional_context,
        } => {
            assert_eq!(permission_decision, Some("deny".to_string()));
            assert_eq!(permission_decision_reason, None);
            assert_eq!(updated_input, None);
            assert_eq!(additional_context, None);
        }
        other => panic!("Expected PreToolUse, got {:?}", other),
    }
}

/// The partial `hookSpecificOutput` must also be honored end to end: its
/// one present field decides the outcome, not `Allow`.
#[test]
fn partial_pre_tool_use_hook_specific_output_decision_is_honored() {
    let json = r#"{
            "continue": true,
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "deny"
            }
        }"#;
    let hook_output: HookOutput =
        serde_json::from_str(json).expect("partial hookSpecificOutput must deserialize");
    let decision = interpret_output(&hook_output, HookEventKind::PreToolUse);
    assert!(
        matches!(decision, HookDecision::Block { .. }),
        "expected Block, got {:?}",
        decision
    );
}

/// Every non-PreToolUse variant carries a single optional field. A hook
/// that emits the bare tag with none of that field set must still
/// deserialize as that variant with the field `None`, not fail the parse.
#[test]
fn sibling_variants_with_no_fields_set_deserialize() {
    let cases = [
        "PostToolUse",
        "PostToolUseFailure",
        "UserPromptSubmit",
        "SessionStart",
        "Notification",
        "Stop",
    ];
    for name in cases {
        let json = format!(r#"{{"hookEventName": "{name}"}}"#);
        let output: HookSpecificOutput = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("bare {name} must deserialize: {e}"));
        assert!(
            extract_specific_context(&Some(output)).is_none(),
            "bare {name} must carry no context"
        );
    }
}

/// A `hookSpecificOutput` that is genuinely unparseable — an unknown
/// `hookEventName` tag — must not be silently swallowed into a permissive
/// decision without a visible trace. The whole `HookOutput` parse fails
/// (checked here), and the caller (`interpret_exit_0_stdout`) logs that
/// failure at `warn` before falling back to `Allow`, which is the
/// documented, deliberate behavior for a genuinely malformed payload.
#[test]
fn malformed_hook_specific_output_tag_is_a_parse_error_not_a_silent_allow() {
    let json = r#"{
            "continue": true,
            "hookSpecificOutput": { "hookEventName": "NotARealEvent" }
        }"#;
    let result: Result<HookOutput, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "an unknown hookEventName tag must be a deserialize error, not a silent Allow"
    );
}

#[test]
fn yaml_partial_pre_tool_use_hook_specific_output_deserializes() {
    let yaml = "hookEventName: PreToolUse\npermissionDecision: deny\n";
    let output: HookSpecificOutput = serde_yaml_ng::from_str(yaml)
        .expect("a hookSpecificOutput with only permissionDecision set must deserialize");
    match output {
        HookSpecificOutput::PreToolUse {
            permission_decision,
            ..
        } => {
            assert_eq!(permission_decision, Some("deny".to_string()));
        }
        other => panic!("Expected PreToolUse, got {:?}", other),
    }
}

#[test]
fn bare_pre_tool_use_tag_with_no_other_fields_deserializes() {
    let json = r#"{"hookEventName": "PreToolUse"}"#;
    let output: HookSpecificOutput = serde_json::from_str(json)
        .expect("a bare PreToolUse tag with no other fields must deserialize");
    assert!(matches!(
        output,
        HookSpecificOutput::PreToolUse {
            permission_decision: None,
            permission_decision_reason: None,
            updated_input: None,
            additional_context: None,
        }
    ));
}

#[test]
fn test_interpret_output_with_enum_block_decision() {
    let output = HookOutputBuilder::new()
        .with_decision(HookDecisionValue::Block)
        .with_reason("Blocked")
        .build();
    let decision = interpret_output(&output, HookEventKind::UserPromptSubmit);
    assert!(matches!(decision, HookDecision::Block { .. }));
}

#[test]
fn test_interpret_output_with_enum_permission_decision() {
    let output = HookOutputBuilder::new()
        .with_hook_specific_output(HookSpecificOutput::PreToolUse {
            permission_decision: Some("deny".into()),
            permission_decision_reason: Some("Denied".into()),
            updated_input: None,
            additional_context: None,
        })
        .build();
    let decision = interpret_output(&output, HookEventKind::PreToolUse);
    assert!(matches!(decision, HookDecision::Block { .. }));
}
