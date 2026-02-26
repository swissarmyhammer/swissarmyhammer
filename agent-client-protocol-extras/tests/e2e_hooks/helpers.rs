//! Shared test infrastructure for e2e hook tests.

use agent_client_protocol::{
    ContentBlock, ContentChunk, InitializeRequest, NewSessionRequest, PromptRequest,
    ProtocolVersion, SessionId, SessionNotification, SessionUpdate, TextContent, ToolCall,
    ToolCallUpdate, ToolCallUpdateFields,
};
use agent_client_protocol_extras::{
    hookable_agent_from_config, HookCommandContext, HookConfig, HookEvaluator, HookEventKind,
    HookableAgent, PlaybackAgent,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// Exhaustive event kind list (compile-time enforced)
// ---------------------------------------------------------------------------

/// Returns all [`HookEventKind`] variants.
///
/// Uses an exhaustive match so the compiler forces an update here
/// whenever a new variant is added to the enum.
pub(crate) fn all_event_kinds() -> Vec<HookEventKind> {
    let mut kinds = Vec::new();
    for kind in [
        HookEventKind::SessionStart,
        HookEventKind::UserPromptSubmit,
        HookEventKind::PreToolUse,
        HookEventKind::PostToolUse,
        HookEventKind::PostToolUseFailure,
        HookEventKind::Stop,
        HookEventKind::Notification,
    ] {
        // Exhaustive match — compiler error if a new variant is added.
        match kind {
            HookEventKind::SessionStart
            | HookEventKind::UserPromptSubmit
            | HookEventKind::PreToolUse
            | HookEventKind::PostToolUse
            | HookEventKind::PostToolUseFailure
            | HookEventKind::Stop
            | HookEventKind::Notification => kinds.push(kind),
        }
    }
    kinds
}

// ---------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------

/// Path to the `.fixtures/hooks` directory under CARGO_MANIFEST_DIR.
pub(crate) fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".fixtures/hooks")
}

/// Load a [`PlaybackAgent`] from a named fixture in the hooks fixture directory.
pub(crate) fn load_playback_agent(fixture_name: &str) -> PlaybackAgent {
    let path = fixtures_dir().join(fixture_name);
    PlaybackAgent::new(path, "test")
}

// ---------------------------------------------------------------------------
// Hook script helpers
// ---------------------------------------------------------------------------

/// Write a shell script that captures stdin, prints `stderr_msg` to stderr,
/// and exits with `exit_code`.
pub(crate) fn write_exit_script(
    dir: &Path,
    name: &str,
    exit_code: i32,
    stderr_msg: &str,
) -> PathBuf {
    let script_path = dir.join(name);
    let capture_path = dir.join(format!("{}.stdin_capture", name));
    let content = format!(
        "#!/bin/sh\ncat > '{}'\necho '{}' >&2\nexit {}\n",
        capture_path.display(),
        stderr_msg,
        exit_code,
    );
    std::fs::write(&script_path, content).expect("Failed to write hook script");
    make_executable(&script_path);
    script_path
}

/// Write a shell script that captures stdin, prints `json_output` to stdout,
/// and exits with code 0.
pub(crate) fn write_json_output_script(dir: &Path, name: &str, json_output: &str) -> PathBuf {
    let script_path = dir.join(name);
    let capture_path = dir.join(format!("{}.stdin_capture", name));
    let content = format!(
        "#!/bin/sh\ncat > '{}'\nprintf '%s' '{}'\nexit 0\n",
        capture_path.display(),
        json_output.replace('\'', "'\\''"),
    );
    std::fs::write(&script_path, content).expect("Failed to write hook script");
    make_executable(&script_path);
    script_path
}

/// Write a shell script that sleeps for `seconds` then exits 0.
pub(crate) fn write_slow_script(dir: &Path, name: &str, seconds: u32) -> PathBuf {
    let script_path = dir.join(name);
    let content = format!("#!/bin/sh\nsleep {}\nexit 0\n", seconds);
    std::fs::write(&script_path, content).expect("Failed to write hook script");
    make_executable(&script_path);
    script_path
}

/// Read the stdin capture file written by a hook script, if it exists.
pub(crate) fn read_stdin_capture(dir: &Path, script_name: &str) -> Option<String> {
    let capture_path = dir.join(format!("{}.stdin_capture", script_name));
    std::fs::read_to_string(capture_path).ok()
}

/// Poll for the stdin capture file to appear with non-empty content,
/// retrying with backoff.
///
/// Notification-pipeline hooks run in a spawned tokio task that may not be
/// scheduled immediately, especially under CI load.  This helper replaces
/// bare `read_stdin_capture` calls in notification-pipeline tests to avoid
/// flaky race conditions.
///
/// The shell scripts use `cat > file` which creates/truncates the file
/// before stdin has been fully written. We require non-empty content to
/// avoid reading a partially-written (empty) capture file.
pub(crate) async fn wait_for_stdin_capture(dir: &Path, script_name: &str) -> Option<String> {
    let capture_path = dir.join(format!("{}.stdin_capture", script_name));
    for _ in 0..40 {
        if let Ok(contents) = std::fs::read_to_string(&capture_path) {
            if !contents.is_empty() {
                return Some(contents);
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    None
}

/// Remove the stdin capture file so a subsequent non-firing hook can be verified.
///
/// Validates that `script_name` is a plain filename (no path separators).
pub(crate) fn clear_stdin_capture(dir: &Path, script_name: &str) {
    let safe_name = Path::new(script_name)
        .file_name()
        .expect("script_name must be a plain filename");
    let capture_path = dir.join(format!("{}.stdin_capture", safe_name.to_string_lossy()));
    let _ = std::fs::remove_file(capture_path);
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("Failed to chmod hook script");
    }
    #[cfg(not(unix))]
    let _ = path;
}

// ---------------------------------------------------------------------------
// Hook config builders
// ---------------------------------------------------------------------------

/// Build a JSON string configuring a single command hook for the given event.
pub(crate) fn hook_config_json(event_name: &str, command: &str, matcher: Option<&str>) -> String {
    let matcher_field = match matcher {
        Some(m) => format!(r#""matcher": "{m}","#),
        None => String::new(),
    };
    format!(
        r#"{{
            "hooks": {{
                "{event_name}": [
                    {{
                        {matcher_field}
                        "hooks": [
                            {{ "type": "command", "command": "{cmd}" }}
                        ]
                    }}
                ]
            }}
        }}"#,
        cmd = command.replace('\\', "\\\\").replace('"', "\\\""),
    )
}

/// Build a JSON string configuring a single command hook with a custom timeout.
pub(crate) fn hook_config_json_with_timeout(
    event_name: &str,
    command: &str,
    timeout_secs: u64,
) -> String {
    format!(
        r#"{{
            "hooks": {{
                "{event_name}": [
                    {{
                        "hooks": [
                            {{ "type": "command", "command": "{cmd}", "timeout": {timeout_secs} }}
                        ]
                    }}
                ]
            }}
        }}"#,
        cmd = command.replace('\\', "\\\\").replace('"', "\\\""),
    )
}

/// Build a JSON string configuring two command hooks for the given event.
pub(crate) fn hook_config_two_hooks_json(
    event_name: &str,
    command1: &str,
    command2: &str,
) -> String {
    format!(
        r#"{{
            "hooks": {{
                "{event_name}": [
                    {{
                        "hooks": [
                            {{ "type": "command", "command": "{cmd1}" }},
                            {{ "type": "command", "command": "{cmd2}" }}
                        ]
                    }}
                ]
            }}
        }}"#,
        cmd1 = command1.replace('\\', "\\\\").replace('"', "\\\""),
        cmd2 = command2.replace('\\', "\\\\").replace('"', "\\\""),
    )
}

/// Parse a JSON hook config and build a [`HookableAgent`] wrapping `inner`.
pub(crate) fn build_hookable_agent(
    inner: Arc<dyn agent_client_protocol::Agent + Send + Sync>,
    config_json: &str,
) -> HookableAgent {
    let config: HookConfig =
        serde_json::from_str(config_json).expect("Failed to parse hook config JSON");
    hookable_agent_from_config(inner, &config, None).expect("Failed to build HookableAgent")
}

// ---------------------------------------------------------------------------
// Agent lifecycle helpers
// ---------------------------------------------------------------------------

/// Initialize the agent and create a new session, returning the session ID.
pub(crate) async fn init_session(agent: &dyn agent_client_protocol::Agent) -> SessionId {
    let _init_resp = agent
        .initialize(InitializeRequest::new(ProtocolVersion::V1))
        .await
        .expect("initialize failed");

    let session_resp = agent
        .new_session(NewSessionRequest::new(PathBuf::from("/tmp/test-hooks")))
        .await
        .expect("new_session failed");

    session_resp.session_id
}

/// Build a [`PromptRequest`] with a single text content block.
pub(crate) fn make_prompt_request(session_id: SessionId, text: &str) -> PromptRequest {
    PromptRequest::new(session_id, vec![ContentBlock::Text(TextContent::new(text))])
}

/// Send a prompt and unwrap the response.
pub(crate) async fn run_prompt(
    agent: &dyn agent_client_protocol::Agent,
    session_id: &SessionId,
    text: &str,
) -> agent_client_protocol::PromptResponse {
    agent
        .prompt(make_prompt_request(session_id.clone(), text))
        .await
        .expect("prompt failed")
}

// ---------------------------------------------------------------------------
// Notification pipeline helpers
// ---------------------------------------------------------------------------

/// Delay between sending notifications so the spawned intercept task can process each one.
const NOTIFY_DELAY: std::time::Duration = std::time::Duration::from_millis(50);

/// Send a tool-call + completed-update through a broadcast channel to trigger
/// PreToolUse and PostToolUse hooks via `intercept_notifications`.
pub(crate) async fn send_tool_completed_notifications(
    tx: &broadcast::Sender<SessionNotification>,
    session_id: &str,
) {
    let tool_call = ToolCall::new("call-1", "Bash");
    let _ = tx.send(SessionNotification::new(
        SessionId::new(session_id),
        SessionUpdate::ToolCall(tool_call),
    ));
    tokio::time::sleep(NOTIFY_DELAY).await;

    let update = ToolCallUpdate::new(
        "call-1",
        ToolCallUpdateFields::new().status(agent_client_protocol::ToolCallStatus::Completed),
    );
    let _ = tx.send(SessionNotification::new(
        SessionId::new(session_id),
        SessionUpdate::ToolCallUpdate(update),
    ));
    tokio::time::sleep(NOTIFY_DELAY).await;
}

/// Send a tool-call + failed-update through a broadcast channel to trigger
/// PreToolUse and PostToolUseFailure hooks via `intercept_notifications`.
pub(crate) async fn send_tool_failed_notifications(
    tx: &broadcast::Sender<SessionNotification>,
    session_id: &str,
) {
    let tool_call = ToolCall::new("call-1", "Bash");
    let _ = tx.send(SessionNotification::new(
        SessionId::new(session_id),
        SessionUpdate::ToolCall(tool_call),
    ));
    tokio::time::sleep(NOTIFY_DELAY).await;

    let update = ToolCallUpdate::new(
        "call-1",
        ToolCallUpdateFields::new().status(agent_client_protocol::ToolCallStatus::Failed),
    );
    let _ = tx.send(SessionNotification::new(
        SessionId::new(session_id),
        SessionUpdate::ToolCallUpdate(update),
    ));
    tokio::time::sleep(NOTIFY_DELAY).await;
}

/// Send an AgentMessageChunk notification to trigger Notification hooks.
pub(crate) async fn send_agent_message_notification(
    tx: &broadcast::Sender<SessionNotification>,
    session_id: &str,
) {
    let content = ContentChunk::new(ContentBlock::Text(TextContent::new("agent says hello")));
    let _ = tx.send(SessionNotification::new(
        SessionId::new(session_id),
        SessionUpdate::AgentMessageChunk(content),
    ));
    tokio::time::sleep(NOTIFY_DELAY).await;
}

/// Send a single ToolCall notification with a specific tool name and call ID.
///
/// Unlike [`send_tool_completed_notifications`], this sends only the ToolCall
/// (triggering PreToolUse) without a follow-up ToolCallUpdate.
pub(crate) async fn send_named_tool_notification(
    tx: &broadcast::Sender<SessionNotification>,
    session_id: &str,
    tool_name: &str,
    call_id: &str,
) {
    let tool_call = ToolCall::new(call_id.to_string(), tool_name.to_string());
    let _ = tx.send(SessionNotification::new(
        SessionId::new(session_id),
        SessionUpdate::ToolCall(tool_call),
    ));
    tokio::time::sleep(NOTIFY_DELAY).await;
}

// ---------------------------------------------------------------------------
// AVP schema validation helpers
// ---------------------------------------------------------------------------

/// Build a HookCommandContext with typical test values for AVP validation.
pub(crate) fn avp_test_context() -> HookCommandContext {
    HookCommandContext {
        transcript_path: "/tmp/test-transcript.jsonl".to_string(),
        permission_mode: "default".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Script helpers (additional variants)
// ---------------------------------------------------------------------------

/// Write a shell script that captures stdin, prints non-JSON to stdout,
/// and exits 0. Used to test malformed output fallback.
pub(crate) fn write_malformed_output_script(dir: &Path, name: &str) -> PathBuf {
    let script_path = dir.join(name);
    let capture_path = dir.join(format!("{}.stdin_capture", name));
    let content = format!(
        "#!/bin/sh\ncat > '{}'\nprintf '%s' 'not valid json {{{{'\nexit 0\n",
        capture_path.display(),
    );
    std::fs::write(&script_path, content).expect("Failed to write hook script");
    make_executable(&script_path);
    script_path
}

/// Write a shell script that captures stdin and exits with the given code
/// (no stderr message). Used for testing unexpected exit codes.
pub(crate) fn write_exit_code_script(dir: &Path, name: &str, exit_code: i32) -> PathBuf {
    let script_path = dir.join(name);
    let capture_path = dir.join(format!("{}.stdin_capture", name));
    let content = format!(
        "#!/bin/sh\ncat > '{}'\nexit {}\n",
        capture_path.display(),
        exit_code,
    );
    std::fs::write(&script_path, content).expect("Failed to write hook script");
    make_executable(&script_path);
    script_path
}

// ---------------------------------------------------------------------------
// Mock evaluator for prompt/agent hooks
// ---------------------------------------------------------------------------

/// Test evaluator for prompt-based and agent-based hooks.
pub(crate) struct MockEvaluator {
    response: String,
    is_agent_called: Arc<AtomicBool>,
}

impl MockEvaluator {
    /// Returns an evaluator that always responds `{"ok": true}`.
    pub(crate) fn allowing() -> Self {
        Self {
            response: r#"{"ok": true}"#.to_string(),
            is_agent_called: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns an evaluator that responds `{"ok": false, "reason": "..."}`.
    pub(crate) fn blocking(reason: &str) -> Self {
        Self {
            response: format!(r#"{{"ok": false, "reason": "{}"}}"#, reason),
            is_agent_called: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns an evaluator (ok=true) plus a shared flag tracking whether
    /// it was called with `is_agent=true`.
    pub(crate) fn with_agent_tracking() -> (Self, Arc<AtomicBool>) {
        let flag = Arc::new(AtomicBool::new(false));
        (
            Self {
                response: r#"{"ok": true}"#.to_string(),
                is_agent_called: flag.clone(),
            },
            flag,
        )
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

// ---------------------------------------------------------------------------
// Agent builder with evaluator
// ---------------------------------------------------------------------------

/// Like [`build_hookable_agent`] but passes an evaluator for prompt/agent hooks.
pub(crate) fn build_hookable_agent_with_evaluator(
    inner: Arc<dyn agent_client_protocol::Agent + Send + Sync>,
    config_json: &str,
    evaluator: Arc<dyn HookEvaluator>,
) -> HookableAgent {
    let config: HookConfig =
        serde_json::from_str(config_json).expect("Failed to parse hook config JSON");
    hookable_agent_from_config(inner, &config, Some(evaluator))
        .expect("Failed to build HookableAgent")
}
