//! Tests for [the two built-in hook handlers](super).

use super::*;

use super::super::output::HookDecisionValue;

// =====================================================================
// Event-aware exit-2 tests (interpret_exit_2_stderr is in this crate)
// =====================================================================

/// Run a hook command that writes `message` on stderr and exits with the
/// block code, which is how a command hook refuses.
fn refusing_hook_output(message: &str) -> std::process::Output {
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("echo '{message}' >&2; exit {EXIT_CODE_BLOCK}"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("the shell must run the hook command")
}

#[test]
fn test_exit_2_on_silent_events_allows() {
    let silent = vec![HookEventKind::Notification, HookEventKind::SessionStart];
    for kind in &silent {
        let output = refusing_hook_output("should not block");
        let decision = interpret_exit_2_stderr(&output, "test-cmd", *kind);
        assert!(
            matches!(decision, HookDecision::Allow),
            "Expected Allow for silent {:?}, got {:?}",
            kind,
            decision
        );
    }
}

#[test]
fn test_exit_2_on_blockable_event_blocks() {
    let blockable = vec![HookEventKind::PreToolUse, HookEventKind::UserPromptSubmit];
    for kind in &blockable {
        let output = refusing_hook_output("blocked");
        let decision = interpret_exit_2_stderr(&output, "test-cmd", *kind);
        assert!(
            matches!(decision, HookDecision::Block { .. }),
            "Expected Block for blockable {:?}, got {:?}",
            kind,
            decision
        );
    }
}

/// A `Stop` event has no action to stop, so a refusal there tells the agent to
/// go on. The stderr of the hook is the reason.
#[test]
fn exit_2_on_a_stop_event_asks_the_agent_to_continue() {
    let output = refusing_hook_output("tests are not complete");
    let decision = interpret_exit_2_stderr(&output, "test-cmd", HookEventKind::Stop);
    match decision {
        HookDecision::ShouldContinue { reason } => {
            assert_eq!(reason, "tests are not complete");
        }
        other => panic!("Expected ShouldContinue for Stop, got {:?}", other),
    }
}

/// A tool event happens after the tool ran, so a refusal cannot block it. The
/// stderr of the hook goes to the agent as context instead.
#[test]
fn exit_2_on_a_post_tool_event_gives_the_stderr_to_the_agent() {
    let post_tool = vec![
        HookEventKind::PostToolUse,
        HookEventKind::PostToolUseFailure,
    ];
    for kind in &post_tool {
        let output = refusing_hook_output("lint warning");
        let decision = interpret_exit_2_stderr(&output, "test-cmd", *kind);
        match decision {
            HookDecision::AllowWithContext { context } => {
                assert_eq!(context, "lint warning");
            }
            other => panic!("Expected AllowWithContext for {:?}, got {:?}", kind, other),
        }
    }
}

/// `Allow` is the one decision that drops the message of the hook, so the
/// fall back to it must be visible in the log.
#[test]
#[tracing_test::traced_test]
fn exit_2_on_a_silent_event_logs_the_fall_back_to_allow() {
    let output = refusing_hook_output("nobody reads this");
    let decision = interpret_exit_2_stderr(&output, "test-cmd", HookEventKind::Notification);
    assert!(
        matches!(decision, HookDecision::Allow),
        "Expected Allow, got {:?}",
        decision
    );
    assert!(
        logs_contain("Exit 2 on non-blockable event"),
        "a refusal that becomes Allow must log the fall back"
    );
}

/// A decision that keeps the message of the hook needs no log. Only the fall
/// back to `Allow` writes one.
#[test]
#[tracing_test::traced_test]
fn exit_2_on_a_blockable_event_logs_no_fall_back() {
    let output = refusing_hook_output("blocked");
    let decision = interpret_exit_2_stderr(&output, "test-cmd", HookEventKind::PreToolUse);
    assert!(
        matches!(decision, HookDecision::Block { .. }),
        "Expected Block, got {:?}",
        decision
    );
    assert!(
        !logs_contain("Exit 2 on non-blockable event"),
        "a block keeps the message of the hook, so it must not log the fall back"
    );
}

/// End to end: a hook command that exits 0 with a genuinely malformed
/// `hookSpecificOutput` falls back to `Allow`, but only after an
/// explicit, visible `warn`-level log — never a silent permit.
#[test]
#[tracing_test::traced_test]
fn malformed_hook_specific_output_allows_with_an_explicit_asserted_log() {
    let stdout = r#"{"hookSpecificOutput": {"hookEventName": "NotARealEvent"}}"#;
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("echo '{stdout}'"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .unwrap();

    let decision = interpret_exit_0_stdout(&output, "test-cmd", HookEventKind::PreToolUse);

    assert!(
        matches!(decision, HookDecision::Allow),
        "expected Allow, got {:?}",
        decision
    );
    assert!(
        logs_contain("Failed to parse hook command output"),
        "a malformed hookSpecificOutput must log a visible warning before falling back to Allow"
    );
}

// -- Hook command stdout parsing --

/// JSON is the documented hook protocol and must keep working unchanged.
#[test]
fn hook_stdout_parses_json() {
    let parsed = parse_hook_stdout(r#"{"decision":"block","reason":"keep going"}"#)
        .expect("JSON hook output must parse");

    assert_eq!(parsed.decision, Some(HookDecisionValue::Block));
    assert_eq!(parsed.reason.as_deref(), Some("keep going"));
}

/// A hook command that answers in YAML must be understood too. Before this
/// fallback, YAML stdout failed the JSON parse and the decision was thrown
/// away as `Allow` with only a warning — the hook silently did nothing.
#[test]
fn hook_stdout_falls_back_to_yaml() {
    let parsed = parse_hook_stdout("decision: block\nreason: keep going\n")
        .expect("YAML hook output must parse");

    assert_eq!(parsed.decision, Some(HookDecisionValue::Block));
    assert_eq!(parsed.reason.as_deref(), Some("keep going"));
}

/// On a Stop event `block` inverts to "do not stop", and that has to survive
/// the YAML path exactly as it does the JSON one.
#[test]
fn yaml_hook_stdout_still_blocks_the_stop() {
    let parsed = parse_hook_stdout("decision: block\nreason: keep going\n").expect("YAML parses");

    assert!(matches!(
        interpret_output(&parsed, HookEventKind::Stop),
        HookDecision::ShouldContinue { .. }
    ));
}

/// Output that is neither JSON nor YAML is still reported as an error, so
/// the caller can warn instead of inventing a decision.
#[test]
fn hook_stdout_that_is_neither_json_nor_yaml_is_an_error() {
    assert!(parse_hook_stdout("this is: not: valid: anything").is_err());
}
