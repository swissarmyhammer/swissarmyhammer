//! Tests for [the hook config shapes and the registration factory](super).

use super::*;

use super::super::event::{HookEvent, SessionSource};
use std::sync::atomic::{AtomicBool, Ordering};
use swissarmyhammer_common::test_utils::shell_escape_path;

// -- Mock evaluator (for prompt/agent handler config tests) --

struct MockEvaluator {
    response: String,
    is_agent_called: Arc<AtomicBool>,
}

impl MockEvaluator {
    fn allowing() -> Self {
        Self {
            response: r#"{"ok": true}"#.to_string(),
            is_agent_called: Arc::new(AtomicBool::new(false)),
        }
    }
}

#[async_trait::async_trait]
impl HookEvaluator for MockEvaluator {
    async fn evaluate(&self, _prompt: &str, is_agent: bool) -> Result<String, String> {
        if is_agent {
            self.is_agent_called.store(true, Ordering::SeqCst);
        }
        Ok(self.response.clone())
    }
}

// =====================================================================
// JSON deserialization tests (3-level nesting)
// =====================================================================

#[test]
fn test_json_command_hook() {
    let json = r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            { "type": "command", "command": "./check.sh" }
                        ]
                    }
                ]
            }
        }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.hooks.len(), 1);
    let groups = config.hooks.get(&HookEventKindConfig::PreToolUse).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].matcher.as_deref(), Some("Bash"));
    assert_eq!(groups[0].hooks.len(), 1);
    assert!(matches!(
        &groups[0].hooks[0],
        HookHandlerConfig::Command { command, .. } if command == "./check.sh"
    ));
}

#[test]
fn test_json_prompt_hook() {
    let json = r#"{
            "hooks": {
                "Stop": [
                    {
                        "hooks": [
                            { "type": "prompt", "prompt": "Check if all tasks are complete: $ARGUMENTS" }
                        ]
                    }
                ]
            }
        }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    let groups = config.hooks.get(&HookEventKindConfig::Stop).unwrap();
    assert!(groups[0].matcher.is_none());
    assert!(matches!(
        &groups[0].hooks[0],
        HookHandlerConfig::Prompt { prompt, .. }
            if prompt.contains("$ARGUMENTS")
    ));
}

#[test]
fn test_json_agent_hook() {
    let json = r#"{
            "hooks": {
                "Stop": [
                    {
                        "hooks": [
                            { "type": "agent", "prompt": "Verify tests pass: $ARGUMENTS", "timeout": 120 }
                        ]
                    }
                ]
            }
        }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    let groups = config.hooks.get(&HookEventKindConfig::Stop).unwrap();
    assert!(matches!(
        &groups[0].hooks[0],
        HookHandlerConfig::Agent { prompt, timeout, .. }
            if prompt.contains("Verify tests") && *timeout == 120
    ));
}

#[test]
fn test_json_multiple_events_with_matchers() {
    let json = r#"{
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [ { "type": "command", "command": "./bash-check.sh" } ] },
                    { "matcher": "Edit|Write", "hooks": [ { "type": "command", "command": "./lint.sh" } ] }
                ],
                "Stop": [
                    { "hooks": [ { "type": "prompt", "prompt": "All done? $ARGUMENTS" } ] }
                ]
            }
        }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.hooks.len(), 2);
    let pre_tool = config.hooks.get(&HookEventKindConfig::PreToolUse).unwrap();
    assert_eq!(pre_tool.len(), 2);
    assert_eq!(pre_tool[0].matcher.as_deref(), Some("Bash"));
    assert_eq!(pre_tool[1].matcher.as_deref(), Some("Edit|Write"));
}

#[test]
fn test_json_empty_config() {
    let config: HookConfig = serde_json::from_str("{}").unwrap();
    assert!(config.hooks.is_empty());
}

#[test]
fn test_json_default_timeouts() {
    let json = r#"{
            "hooks": {
                "PreToolUse": [{
                    "hooks": [
                        { "type": "command", "command": "true" },
                        { "type": "prompt", "prompt": "check" },
                        { "type": "agent", "prompt": "verify" }
                    ]
                }]
            }
        }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    let hooks = &config.hooks.get(&HookEventKindConfig::PreToolUse).unwrap()[0].hooks;
    assert!(matches!(&hooks[0], HookHandlerConfig::Command { timeout, .. } if *timeout == 600));
    assert!(matches!(&hooks[1], HookHandlerConfig::Prompt { timeout, .. } if *timeout == 30));
    assert!(matches!(&hooks[2], HookHandlerConfig::Agent { timeout, .. } if *timeout == 60));
}

// =====================================================================
// YAML deserialization tests
// =====================================================================

#[test]
fn test_yaml_command_hook() {
    let yaml = r#"
hooks:
  PreToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: "./check.sh"
"#;
    let config: HookConfig = serde_yaml_ng::from_str(yaml).unwrap();
    let groups = config.hooks.get(&HookEventKindConfig::PreToolUse).unwrap();
    assert_eq!(groups[0].matcher.as_deref(), Some("Bash"));
    assert!(matches!(
        &groups[0].hooks[0],
        HookHandlerConfig::Command { command, .. } if command == "./check.sh"
    ));
}

#[test]
fn test_yaml_prompt_hook() {
    let yaml = r#"
hooks:
  Stop:
    - hooks:
        - type: prompt
          prompt: "Check completion: $ARGUMENTS"
"#;
    let config: HookConfig = serde_yaml_ng::from_str(yaml).unwrap();
    let groups = config.hooks.get(&HookEventKindConfig::Stop).unwrap();
    assert!(matches!(
        &groups[0].hooks[0],
        HookHandlerConfig::Prompt { prompt, .. } if prompt.contains("$ARGUMENTS")
    ));
}

#[test]
fn test_yaml_multiple_events() {
    let yaml = r#"
hooks:
  PreToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: "./check.sh"
  Stop:
    - hooks:
        - type: prompt
          prompt: "Verify completion"
  SessionStart:
    - matcher: "startup"
      hooks:
        - type: command
          command: "./init.sh"
"#;
    let config: HookConfig = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.hooks.len(), 3);
}

#[test]
fn test_yaml_empty_config() {
    let config: HookConfig = serde_yaml_ng::from_str("{}").unwrap();
    assert!(config.hooks.is_empty());
}

// =====================================================================
// JSON ↔ YAML equivalence
// =====================================================================

#[test]
fn test_json_yaml_equivalence() {
    let json = r#"{
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [ { "type": "command", "command": "./check.sh" } ] }
                ],
                "Stop": [
                    { "hooks": [ { "type": "prompt", "prompt": "Done?" } ] }
                ]
            }
        }"#;

    let yaml = r#"
hooks:
  PreToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: "./check.sh"
  Stop:
    - hooks:
        - type: prompt
          prompt: "Done?"
"#;

    let from_json: HookConfig = serde_json::from_str(json).unwrap();
    let from_yaml: HookConfig = serde_yaml_ng::from_str(yaml).unwrap();

    assert_eq!(from_json.hooks.len(), from_yaml.hooks.len());
    assert!(from_json
        .hooks
        .contains_key(&HookEventKindConfig::PreToolUse));
    assert!(from_yaml
        .hooks
        .contains_key(&HookEventKindConfig::PreToolUse));
    assert!(from_json.hooks.contains_key(&HookEventKindConfig::Stop));
    assert!(from_yaml.hooks.contains_key(&HookEventKindConfig::Stop));
}

// =====================================================================
// Build registration tests
// =====================================================================

#[test]
fn test_build_registrations_command() {
    let json = r#"{
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [ { "type": "command", "command": "true" } ] }
                ]
            }
        }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    let regs = config.build_registrations(None).unwrap();
    assert_eq!(regs.len(), 1);
    assert_eq!(regs[0].events, vec![HookEventKind::PreToolUse]);
    assert!(matches!(regs[0].matcher, Matcher::Exact(_)));
}

#[test]
fn test_build_registrations_invalid_regex() {
    let json = r#"{
            "hooks": {
                "PreToolUse": [{
                    "matcher": "[invalid",
                    "hooks": [{ "type": "command", "command": "true" }]
                }]
            }
        }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    let result = config.build_registrations(None);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        HookConfigError::InvalidRegex(_)
    ));
}

#[test]
fn test_build_registrations_missing_evaluator() {
    let json = r#"{
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "prompt", "prompt": "check" }] }]
            }
        }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    let result = config.build_registrations(None);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        HookConfigError::MissingEvaluator
    ));
}

#[test]
fn test_build_registrations_prompt_with_evaluator() {
    let json = r#"{
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "prompt", "prompt": "check" }] }]
            }
        }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    let evaluator: Arc<dyn HookEvaluator> = Arc::new(MockEvaluator::allowing());
    let regs = config.build_registrations(Some(evaluator)).unwrap();
    assert_eq!(regs.len(), 1);
    assert_eq!(regs[0].events, vec![HookEventKind::Stop]);
}

/// A command hook built via `build_registrations_with_context` receives the
/// context's `transcript_path` (and `permission_mode`) in its JSON stdin —
/// the field is captured into the handler at build time, not left at the
/// empty default.
#[tokio::test]
async fn command_handler_stdin_carries_context_transcript_path() {
    let dir = tempfile::TempDir::new().unwrap();
    let marker = dir.path().join("stdin.json");
    // The hook copies its JSON stdin to the marker so the test can inspect it.
    let json = format!(
        r#"{{ "hooks": {{ "SessionStart": [ {{ "hooks": [ {{ "type": "command", "command": "cat > {}" }} ] }} ] }} }}"#,
        shell_escape_path(&marker)
    );
    let config: HookConfig = serde_json::from_str(&json).unwrap();

    let ctx = HookCommandContext {
        transcript_path: "/state/acp/SESSION/raw.jsonl".to_string(),
        permission_mode: "acceptEdits".to_string(),
    };
    let regs = config.build_registrations_with_context(None, &ctx).unwrap();

    let event = HookEvent::SessionStart {
        session_id: "SESSION".to_string(),
        source: SessionSource::Startup,
        cwd: std::path::PathBuf::from("/project"),
    };
    let _ = regs[0].handler.handle(&event).await;

    let recorded = std::fs::read_to_string(&marker).unwrap();
    let value: serde_json::Value = serde_json::from_str(&recorded).unwrap();
    assert_eq!(value["transcript_path"], "/state/acp/SESSION/raw.jsonl");
    assert_eq!(value["permission_mode"], "acceptEdits");
}

/// Regression test for the command-injection finding: a hook command
/// string built by embedding a tempfile path must escape that path for
/// shell interpolation. Without escaping, a path containing shell
/// metacharacters — here a single quote, `$(...)` command substitution,
/// and a space, combined in one filename — breaks out of the intended
/// `cat > <path>` command and executes the injected command instead of
/// being treated as a literal filename.
///
/// The injected `$(touch ...)` payload targets a bare, no-slash relative
/// filename rather than an absolute path: a real filesystem path cannot
/// embed `/` characters within a single path component, so an absolute
/// sentinel path spliced into the marker's own filename text would make
/// `marker` itself unopenable — a filesystem-validity problem, not a
/// shell-escaping one. `run_command` does not set a child `current_dir`,
/// so if the exploit ever fires it lands in the test process's cwd; the
/// sentinel is looked up and removed there.
#[tokio::test]
async fn command_handler_command_string_escapes_shell_metacharacters_in_path() {
    let dir = tempfile::TempDir::new().unwrap();
    let sentinel_name = format!(
        "acp_extras_shell_injection_sentinel_{}.tmp",
        std::process::id()
    );
    let sentinel = std::env::current_dir().unwrap().join(&sentinel_name);
    let _ = std::fs::remove_file(&sentinel); // best-effort cleanup from a prior failed run

    let marker = dir
        .path()
        .join(format!("it's a $(touch {sentinel_name}) marker.json"));

    // The hook copies its JSON stdin to the marker so the test can inspect it.
    // Built via `serde_json::json!` (not a hand-templated JSON string) so
    // the backslash `shell_escape_path` introduces for the embedded
    // single quote is itself correctly JSON-escaped.
    let command = format!("cat > {}", shell_escape_path(&marker));
    let config_json = serde_json::json!({
        "hooks": {
            "SessionStart": [
                { "hooks": [ { "type": "command", "command": command } ] }
            ]
        }
    });
    let config: HookConfig = serde_json::from_value(config_json).unwrap();
    let regs = config.build_registrations(None).unwrap();

    let event = HookEvent::SessionStart {
        session_id: "SESSION".to_string(),
        source: SessionSource::Startup,
        cwd: std::path::PathBuf::from("/project"),
    };
    let _ = regs[0].handler.handle(&event).await;

    let sentinel_created = sentinel.exists();
    let _ = std::fs::remove_file(&sentinel);
    assert!(
        !sentinel_created,
        "shell metacharacters embedded in the path must not execute an injected command"
    );

    let recorded = std::fs::read_to_string(&marker).unwrap_or_else(|e| {
        panic!("expected the hook to write to the literal marker path {marker:?}, got error: {e}")
    });
    let value: serde_json::Value = serde_json::from_str(&recorded).unwrap();
    assert_eq!(value["cwd"], "/project");
}

#[test]
fn test_build_registrations_wildcard_matcher_treated_as_all() {
    let json = r#"{
            "hooks": {
                "PreToolUse": [{
                    "matcher": "*",
                    "hooks": [{ "type": "command", "command": "true" }]
                }]
            }
        }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    let regs = config.build_registrations(None).unwrap();
    assert!(matches!(regs[0].matcher, Matcher::All));
}

// =====================================================================
// Forward-compatible event kinds
// =====================================================================

#[test]
fn test_unsupported_event_kinds_return_error() {
    let unsupported_kinds = vec![
        HookEventKindConfig::PermissionRequest,
        HookEventKindConfig::SubagentStart,
        HookEventKindConfig::SubagentStop,
        HookEventKindConfig::PreCompact,
        HookEventKindConfig::Setup,
        HookEventKindConfig::SessionEnd,
    ];
    for kind in &unsupported_kinds {
        let result: Result<HookEventKind, _> = kind.clone().try_into();
        assert!(result.is_err(), "Expected {:?} to be unsupported", kind);
    }
}

#[test]
fn test_supported_event_kinds_succeed() {
    let supported_kinds = vec![
        (
            HookEventKindConfig::SessionStart,
            HookEventKind::SessionStart,
        ),
        (
            HookEventKindConfig::UserPromptSubmit,
            HookEventKind::UserPromptSubmit,
        ),
        (HookEventKindConfig::PreToolUse, HookEventKind::PreToolUse),
        (HookEventKindConfig::PostToolUse, HookEventKind::PostToolUse),
        (
            HookEventKindConfig::PostToolUseFailure,
            HookEventKind::PostToolUseFailure,
        ),
        (HookEventKindConfig::Stop, HookEventKind::Stop),
        (
            HookEventKindConfig::Notification,
            HookEventKind::Notification,
        ),
        (HookEventKindConfig::PostCompact, HookEventKind::PostCompact),
        (
            HookEventKindConfig::TeammateIdle,
            HookEventKind::TeammateIdle,
        ),
        (
            HookEventKindConfig::TaskCompleted,
            HookEventKind::TaskCompleted,
        ),
        (HookEventKindConfig::Elicitation, HookEventKind::Elicitation),
        (
            HookEventKindConfig::ElicitationResult,
            HookEventKind::ElicitationResult,
        ),
        (
            HookEventKindConfig::InstructionsLoaded,
            HookEventKind::InstructionsLoaded,
        ),
        (
            HookEventKindConfig::ConfigChange,
            HookEventKind::ConfigChange,
        ),
        (
            HookEventKindConfig::WorktreeCreate,
            HookEventKind::WorktreeCreate,
        ),
        (
            HookEventKindConfig::WorktreeRemove,
            HookEventKind::WorktreeRemove,
        ),
    ];
    for (config_kind, expected_kind) in &supported_kinds {
        let result: Result<HookEventKind, _> = config_kind.clone().try_into();
        assert_eq!(
            result.unwrap(),
            *expected_kind,
            "Expected {:?} to convert successfully",
            config_kind
        );
    }
}

#[test]
fn test_unsupported_event_kind_display() {
    let err = UnsupportedEventKind;
    assert_eq!(err.to_string(), "event kind is not supported by ACP");
}

#[test]
fn test_unsupported_event_kind_is_error() {
    let err = UnsupportedEventKind;
    let _: &dyn std::error::Error = &err;
}

#[test]
fn test_post_tool_use_failure_deserialization() {
    let json = r#"{
            "hooks": {
                "PostToolUseFailure": [{
                    "hooks": [{ "type": "command", "command": "echo 'Tool failed'" }]
                }]
            }
        }"#;
    let config: HookConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.hooks.len(), 1);
    assert!(config
        .hooks
        .contains_key(&HookEventKindConfig::PostToolUseFailure));
}

// -- New hook event config variants --

#[test]
fn test_new_config_variants_serde_round_trip() {
    let variants = vec![
        HookEventKindConfig::Elicitation,
        HookEventKindConfig::ElicitationResult,
        HookEventKindConfig::InstructionsLoaded,
        HookEventKindConfig::ConfigChange,
        HookEventKindConfig::WorktreeCreate,
        HookEventKindConfig::WorktreeRemove,
        HookEventKindConfig::PostCompact,
    ];
    for variant in &variants {
        let json = serde_json::to_string(variant).unwrap();
        let deserialized: HookEventKindConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(
            std::mem::discriminant(&deserialized),
            std::mem::discriminant(variant),
            "round-trip failed for {:?}",
            variant
        );
    }
}

#[test]
fn test_new_config_variants_in_hook_config() {
    let names = [
        "Elicitation",
        "ElicitationResult",
        "InstructionsLoaded",
        "ConfigChange",
        "WorktreeCreate",
        "WorktreeRemove",
        "PostCompact",
    ];
    for name in &names {
        let json = format!(
            r#"{{"hooks":{{"{}":[{{"hooks":[{{"type":"command","command":"./check.sh"}}]}}]}}}}"#,
            name
        );
        let config: HookConfig = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("Failed to parse HookConfig with {}: {}", name, e));
        assert_eq!(config.hooks.len(), 1, "Expected 1 entry for {}", name);
    }
}

#[test]
fn test_try_from_new_active_variants() {
    let result: Result<HookEventKind, _> = HookEventKindConfig::PostCompact.try_into();
    assert!(matches!(result.unwrap(), HookEventKind::PostCompact));

    let result: Result<HookEventKind, _> = HookEventKindConfig::TeammateIdle.try_into();
    assert!(matches!(result.unwrap(), HookEventKind::TeammateIdle));

    let result: Result<HookEventKind, _> = HookEventKindConfig::TaskCompleted.try_into();
    assert!(matches!(result.unwrap(), HookEventKind::TaskCompleted));
}

#[test]
fn test_try_from_new_event_kinds_succeed() {
    assert!(matches!(
        HookEventKind::try_from(HookEventKindConfig::Elicitation),
        Ok(HookEventKind::Elicitation)
    ));
    assert!(matches!(
        HookEventKind::try_from(HookEventKindConfig::ElicitationResult),
        Ok(HookEventKind::ElicitationResult)
    ));
    assert!(matches!(
        HookEventKind::try_from(HookEventKindConfig::InstructionsLoaded),
        Ok(HookEventKind::InstructionsLoaded)
    ));
    assert!(matches!(
        HookEventKind::try_from(HookEventKindConfig::ConfigChange),
        Ok(HookEventKind::ConfigChange)
    ));
    assert!(matches!(
        HookEventKind::try_from(HookEventKindConfig::WorktreeCreate),
        Ok(HookEventKind::WorktreeCreate)
    ));
    assert!(matches!(
        HookEventKind::try_from(HookEventKindConfig::WorktreeRemove),
        Ok(HookEventKind::WorktreeRemove)
    ));
}
