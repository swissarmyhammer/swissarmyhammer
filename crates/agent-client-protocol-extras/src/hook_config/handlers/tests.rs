//! Tests for [the two built-in hook handlers](super).

use super::*;

use super::super::output::HookDecisionValue;

// =====================================================================
// Event-aware exit-2 tests (interpret_exit_2_stderr is in this crate)
// =====================================================================

#[test]
fn test_exit_2_on_silent_events_allows() {
    let silent = vec![HookEventKind::Notification, HookEventKind::SessionStart];
    for kind in &silent {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg("echo 'should not block' >&2; exit 2")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .unwrap();
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
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg("echo 'blocked' >&2; exit 2")
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .unwrap();
        let decision = interpret_exit_2_stderr(&output, "test-cmd", *kind);
        assert!(
            matches!(decision, HookDecision::Block { .. }),
            "Expected Block for blockable {:?}, got {:?}",
            kind,
            decision
        );
    }
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
