//! Tests for [the matcher rules and the registration](super).

use super::*;

use std::path::PathBuf;

// =====================================================================
// Matcher fidelity (exact-string vs anchored regex)
// =====================================================================

/// Build a PreToolUse event with the given tool name for matcher tests.
fn pre_tool_use(tool_name: &str) -> HookEvent {
    HookEvent::PreToolUse {
        session_id: "s".into(),
        tool_name: tool_name.into(),
        tool_input: None,
        tool_use_id: None,
        cwd: PathBuf::from("/tmp"),
    }
}

/// Build a registration whose matcher is parsed from `matcher`.
fn reg_with_matcher(matcher: &str) -> HookRegistration {
    HookRegistration::new(
        vec![HookEventKind::PreToolUse],
        Matcher::try_parse(matcher).unwrap(),
        Arc::new(MockEvaluatorHook),
    )
}

struct MockEvaluatorHook;

#[async_trait::async_trait]
impl HookHandler for MockEvaluatorHook {
    async fn handle(&self, _event: &HookEvent) -> HookDecision {
        HookDecision::Allow
    }
}

#[test]
fn test_exact_matcher_is_full_string_not_substring() {
    let reg = reg_with_matcher("Bash");
    assert!(reg.matches(&pre_tool_use("Bash")));
    assert!(!reg.matches(&pre_tool_use("Bash2")));
    assert!(!reg.matches(&pre_tool_use("xBash")));
    assert!(!reg.matches(&pre_tool_use("run-Bash")));
}

#[test]
fn test_exact_alternation_matches_each_member_exactly() {
    let reg = reg_with_matcher("Edit|Write");
    assert!(reg.matches(&pre_tool_use("Edit")));
    assert!(reg.matches(&pre_tool_use("Write")));
    assert!(!reg.matches(&pre_tool_use("Editor")));
    assert!(!reg.matches(&pre_tool_use("Writer")));
}

#[test]
fn test_regex_matcher_uses_js_test_semantics() {
    // Regex matchers are evaluated like RegExp(pattern).test(value):
    // unanchored, with anchoring controlled by the pattern author.
    let reg = reg_with_matcher("mcp__memory__.*");
    assert!(reg.matches(&pre_tool_use("mcp__memory__create_entities")));

    // A leading `^` anchors at the start, so a prefix match succeeds but a
    // string that does not start with the pattern does not.
    let notebook = reg_with_matcher("^Notebook");
    assert!(notebook.matches(&pre_tool_use("NotebookEdit")));
    assert!(!notebook.matches(&pre_tool_use("xNotebookEdit")));
}

#[test]
fn test_wildcard_and_empty_match_everything() {
    for pat in ["*", ""] {
        let reg = reg_with_matcher(pat);
        assert!(reg.matches(&pre_tool_use("Bash")));
        assert!(reg.matches(&pre_tool_use("anything-at-all")));
    }
}

#[test]
fn test_matcher_parse_classifies_kinds() {
    assert!(matches!(Matcher::try_parse("*").unwrap(), Matcher::All));
    assert!(matches!(Matcher::try_parse("").unwrap(), Matcher::All));
    assert!(matches!(
        Matcher::try_parse("Bash").unwrap(),
        Matcher::Exact(_)
    ));
    assert!(matches!(
        Matcher::try_parse("Edit|Write").unwrap(),
        Matcher::Exact(_)
    ));
    assert!(matches!(
        Matcher::try_parse("mcp__memory__.*").unwrap(),
        Matcher::Regex(_)
    ));
    assert!(matches!(
        Matcher::try_parse("^Notebook").unwrap(),
        Matcher::Regex(_)
    ));
}

#[test]
fn test_invalid_regex_matcher_is_an_error() {
    // A pattern with regex metacharacters that fails to compile is
    // reported as an error, not silently treated as match-all.
    assert!(Matcher::try_parse("[invalid").is_err());
}
