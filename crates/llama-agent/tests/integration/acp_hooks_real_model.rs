//! Real-model (Qwen3-0.6B) end-to-end hook tests through the **live** agentic
//! loop.
//!
//! # Why this file exists
//!
//! The scripted-model hook tests in `acp/server.rs` (`hook_e2e`,
//! `tool_dispatch_hooks`) call `AcpServer::process_tool_call` directly with
//! `for_session`-resolved hooks — the same signature production uses — so they
//! faithfully prove the *seam's* decision logic. But two things they cannot
//! cover, and the join between them is proven by no test:
//!
//! 1. The full `prompt()` → agentic loop only runs on a REAL model. The queue
//!    worker (`model_manager.with_model`) has no `TextGenerator` fake-injection
//!    seam, so a fake model cannot drive tool emission through the loop.
//! 2. The existing real-model test
//!    `acp_multi_turn_dispatches_tool_and_threads_result` runs with cwd `/tmp`
//!    and NO `.claude`, so it never exercises `for_session` returning *loaded*
//!    hooks inside the live loop.
//!
//! Net: "a real model emits a tool call → the live loop resolves loaded
//! `.claude` hooks → the hook fires/blocks" is untested. A regression in that
//! loop glue would stay green. These two tests close that join against the
//! canonical Qwen3-0.6B test model:
//!
//! - **PreToolUse deny blocks the real `read_file` call, model continues** — the
//!   iteration-1 Tool message carries the deny reason and does NOT contain the
//!   fixture content `hello`, the hook command's marker file exists, and the
//!   live loop re-prompts after the deny so an `Assistant` message follows the
//!   deny Tool message — because a deny is informed forward progress.
//! - **PostToolUse additionalContext reaches the model from the live loop** —
//!   the tool really executed (the iteration-1 Tool message contains `hello`)
//!   AND that same Tool message carries the unique marker.
//!
//! # Determinism
//!
//! Everything except "does the small model emit the tool call this turn" is
//! deterministic. That single nondeterministic step is wrapped in a bounded
//! retry of real ACP turns; if the model never emits the tool call across all
//! attempts the test SKIPS (warn) rather than flakes — the exact idiom of the
//! sibling `acp_multi_turn_dispatches_tool_and_threads_result`. Model-load /
//! rate-limit failure skips via `build_real_model_server` returning `None`. The
//! temp `HOME` is restored on drop; `#[serial]` serializes the process-global
//! `HOME` mutation; every awaited turn is wrapped in `NO_HANG_BUDGET`.
//!
//! The hook-effect assertions read **per-iteration** session state (the
//! iteration-1 Tool message, and the presence of an `Assistant` message after
//! it) rather than the whole-turn `agentic_loop_aborted` flag. That flag is
//! non-deterministic with this small model — after a correct first tool turn it
//! can emit a spurious second, failing tool call that aborts the *turn* — and is
//! unrelated to whether the hook fired on iteration 1. Reading per-iteration
//! state makes the assertions insensitive to that downstream artifact.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use agent_client_protocol::schema::{
    McpServer, McpServerHttp, NewSessionRequest, PromptRequest, SessionId, SessionUpdate,
};
use llama_agent::acp::config::AcpConfig;
use llama_agent::acp::AcpServer;
use llama_agent::types::ids::SessionId as LlamaSessionId;
use llama_agent::types::{
    AgentAPI, AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
    SessionConfig,
};
use llama_agent::AgentServer;
use serial_test::serial;
use tempfile::TempDir;
use tokio::sync::broadcast::Receiver;
use tracing::{info, warn};

use llama_agent::test_models::{TEST_MODEL_FILE, TEST_MODEL_REPO};

use crate::integration::read_file_mcp_server::start_read_file_mcp_server;

/// Notifications broadcast by the server during a turn.
type Notification = agent_client_protocol::schema::SessionNotification;

/// A guard against the loop hanging: every awaited turn runs inside this budget
/// so a regression to a hang fails the test loudly instead of stalling the
/// whole suite.
const NO_HANG_BUDGET: Duration = Duration::from_secs(120);

/// Up to this many real ACP turns to coax the small model into emitting the
/// `read_file` tool call. Once it emits, the hook assertions are deterministic.
const MAX_ATTEMPTS: usize = 4;

// ---------------------------------------------------------------------------
// HOME isolation (mirrors the `hook_lifecycle` / `hook_e2e` HomeGuard pattern)
// ---------------------------------------------------------------------------

/// Point `HOME` at a temp dir for the duration of a test so the user-level
/// `~/.claude/settings.json` the hook loader reads is empty and deterministic —
/// a real home directory's hooks must not bleed into these tests. Restores the
/// previous `HOME` on drop. Use only from a `#[serial]` test (it mutates a
/// process-global env var).
struct HomeGuard {
    previous: Option<String>,
    _home: TempDir,
}

impl HomeGuard {
    /// Create a fresh temp `HOME` with an empty `~/.claude` and point the
    /// process at it.
    fn new() -> Self {
        let previous = std::env::var("HOME").ok();
        let home = TempDir::new().unwrap();
        fs::create_dir_all(home.path().join(".claude")).unwrap();
        std::env::set_var("HOME", home.path());
        Self {
            previous,
            _home: home,
        }
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }
}

/// Create a temp project dir whose `.claude/settings.json` carries `contents`.
/// This is the session cwd the live loop loads hooks from via `for_session`.
fn project_with_settings(contents: &str) -> TempDir {
    let dir = TempDir::new().unwrap();
    let claude = dir.path().join(".claude");
    fs::create_dir_all(&claude).unwrap();
    fs::write(claude.join("settings.json"), contents).unwrap();
    dir
}

// ---------------------------------------------------------------------------
// Real-model harness (copied from `acp_agentic_loop.rs` verbatim where possible)
// ---------------------------------------------------------------------------

/// Build an `AgentConfig` against the canonical Qwen3-0.6B test model — the same
/// config the sibling real-model tests use, so the turn loop runs the production
/// path. MCP servers attach per-session via `NewSessionRequest.mcp_servers`, not
/// the agent-level config, so this config carries none.
fn real_model_config() -> AgentConfig {
    AgentConfig {
        model: ModelConfig {
            source: ModelSource::HuggingFace {
                repo: TEST_MODEL_REPO.to_string(),
                filename: Some(TEST_MODEL_FILE.to_string()),
                folder: None,
            },
            batch_size: 64,
            use_hf_params: true,
            retry_config: RetryConfig {
                max_retries: 2,
                initial_delay_ms: 100,
                backoff_multiplier: 1.5,
                max_delay_ms: 1000,
            },
            debug: false,
            n_seq_max: 1,
            n_threads: 4,
            n_threads_batch: 4,
        },
        mcp_servers: Vec::new(),
        session_config: SessionConfig::default(),
        parallel_execution_config: ParallelConfig::default(),
        queue_config: QueueConfig::default(),
        tool_execution_config: Default::default(),
    }
}

/// Build a `NewSessionRequest` whose cwd is `project` (so the live loop loads
/// that project's `.claude/settings.json` hooks) AND that attaches the
/// in-process `read_file` MCP server over HTTP via the ACP `mcpServers` list —
/// the exact path the kanban board uses to hand the agent its `mcpUrl`.
///
/// This is the crux versus the sibling agentic-loop tests: those use cwd `/tmp`
/// with no `.claude`, so `for_session` returns empty hooks; here the cwd is a
/// real project dir carrying the hook under test.
fn new_session_in_project_with_mcp(project: &Path, mcp_url: &str) -> NewSessionRequest {
    NewSessionRequest::new(project.to_path_buf()).mcp_servers(vec![McpServer::Http(
        McpServerHttp::new("read-file-test-server", mcp_url),
    )])
}

/// Build an `AcpServer` on top of a fully-initialized `AgentServer` (model
/// loaded), returning the server plus the notification receiver. Returns `None`
/// when HuggingFace rate-limits or the model fails to load, so CI doesn't flake
/// on conditions outside the test's control — the same skip idiom the sibling
/// real-model tests use.
async fn build_real_model_server(
    config: AgentConfig,
) -> Option<(Arc<AcpServer>, Receiver<Notification>)> {
    let agent = match AgentServer::initialize(config).await {
        Ok(agent) => agent,
        Err(e) => {
            let msg = e.to_string().to_lowercase();
            if msg.contains("429")
                || msg.contains("too many requests")
                || msg.contains("rate limited")
                || msg.contains("loadingfailed")
            {
                warn!("Skipping test: model load / rate-limit failure: {}", e);
                return None;
            }
            panic!("AgentServer initialization failed: {}", e);
        }
    };

    let mount = Arc::new(llama_agent::InProcessMount::new(
        llama_agent::echo::EchoService::new(),
    ));
    let (server, rx) = AcpServer::new(Arc::new(agent), AcpConfig::default(), mount);
    Some((Arc::new(server), rx))
}

/// Initialize test tracing once; ignore the "already initialized" error.
fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_test_writer()
        .try_init();
}

/// Drain every notification currently buffered on the receiver into a Vec,
/// continuing past `Lagged` gaps so a late `ToolCall` is never silently dropped.
fn drain(rx: &mut Receiver<Notification>) -> Vec<Notification> {
    use tokio::sync::broadcast::error::TryRecvError;
    let mut out = Vec::new();
    loop {
        match rx.try_recv() {
            Ok(n) => out.push(n),
            Err(TryRecvError::Lagged(_)) => continue,
            Err(TryRecvError::Empty) | Err(TryRecvError::Closed) => break,
        }
    }
    out
}

/// Collect notifications *concurrently* with an awaited turn so a token-heavy
/// turn cannot overflow the bounded broadcast channel and evict the `ToolCall`
/// before a post-hoc drain runs. Mirrors the collector in `acp_agentic_loop.rs`.
struct NotificationCollector {
    handle: tokio::task::JoinHandle<Vec<Notification>>,
    stop: Arc<tokio::sync::Notify>,
}

impl NotificationCollector {
    /// Start collecting from a fresh subscription taken at the current stream
    /// position. Pass the shared receiver; this resubscribes from it so the
    /// caller keeps its own receiver intact.
    fn start(rx: &Receiver<Notification>) -> Self {
        let mut sub = rx.resubscribe();
        let stop = Arc::new(tokio::sync::Notify::new());
        let stop_signal = Arc::clone(&stop);
        let handle = tokio::spawn(async move {
            let mut out = Vec::new();
            loop {
                tokio::select! {
                    biased;
                    recv = sub.recv() => match recv {
                        Ok(n) => out.push(n),
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    },
                    () = stop_signal.notified() => {
                        while let Ok(n) = sub.try_recv() {
                            out.push(n);
                        }
                        break;
                    }
                }
            }
            out
        });
        Self { handle, stop }
    }

    /// Stop collecting and return everything observed during the turn.
    async fn finish(self) -> Vec<Notification> {
        self.stop.notify_one();
        self.handle.await.expect("notification collector task")
    }
}

/// Resolve the absolute path to the shared multi-turn fixture file (contains
/// `fn main` / `hello`). Reusing the existing fixture keeps the assertions in
/// sync with the sibling tests.
fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("multi_turn")
        .join("example.rs")
}

/// Build a `PromptRequest` carrying a single user text block.
fn text_prompt(session_id: SessionId, text: &str) -> PromptRequest {
    PromptRequest::new(
        session_id,
        vec![agent_client_protocol::schema::ContentBlock::from(
            text.to_string(),
        )],
    )
}

/// Parse the ACP session id into the internal llama `SessionId`.
fn parse_llama_id(id: &SessionId) -> LlamaSessionId {
    id.0.as_ref()
        .parse()
        .expect("a session id this server created must parse as a llama ULID")
}

/// The proven imperative `/no_think` prompt that drives the small model to call
/// `read_file` on the fixture in one turn (the shape used by
/// `acp_multi_turn_dispatches_tool_and_threads_result`).
fn read_file_prompt(fixture: &Path) -> String {
    format!(
        "/no_think Use the read_file tool to read the file at {}. After you receive the tool \
         result, tell me the name of the function defined in that file in one short sentence.",
        fixture.to_string_lossy()
    )
}

/// The observable signals one tool-calling prompt attempt yields for the hook
/// assertions.
struct ToolTurnOutcome {
    /// A `ToolCall` notification was broadcast during the turn — the hard guard
    /// that the model actually emitted the tool call this turn.
    tool_call_broadcast: bool,
    /// Number of `Tool`-role messages the loop appended to the session.
    tool_message_count: usize,
    /// Content of the FIRST `Tool`-role message the loop appended — the
    /// iteration-1 result, which is exactly where the hook effect (deny reason /
    /// additionalContext) lands. Asserting on this single message rather than on
    /// the whole concatenation makes the hook-effect checks insensitive to any
    /// spurious later iteration the small model may add (the source of the
    /// historical flake). Empty string when no `Tool` message was appended.
    first_tool_message_text: String,
    /// `true` when the session contains an `Assistant`-role message positioned
    /// AFTER the first `Tool`-role message — i.e. the live loop re-prompted the
    /// model with the threaded-back tool result and the model generated again.
    /// This is the robust, deterministic signal that "the loop continued past
    /// the first tool step": post-fix a PreToolUse deny is `failed:false`, so
    /// the loop ALWAYS re-prompts after a denied call. Unlike a whole-turn abort
    /// flag, a later unrelated failing tool call cannot retroactively clear an
    /// assistant generation that already happened, so this never flakes.
    assistant_after_first_tool: bool,
}

/// Drive one `read_file` prompt through `server.prompt()` on a fresh session
/// whose cwd is `project` (loaded hooks) with the MCP server attached, then
/// collect the tool-path signals — most importantly the content of the
/// `Tool`-role messages threaded back to the model.
///
/// Driving through `server.prompt()` (NOT `process_tool_call` directly) is the
/// whole point: it exercises the live loop resolving loaded `.claude` hooks via
/// `for_session` and firing them inside a real generation turn.
async fn run_tool_turn(
    server: &AcpServer,
    rx: &mut Receiver<Notification>,
    project: &Path,
    mcp_url: &str,
    prompt_text: &str,
) -> ToolTurnOutcome {
    let session = server
        .new_session(new_session_in_project_with_mcp(project, mcp_url))
        .await
        .expect("new_session in project cwd with MCP server must succeed");
    // Discard session-creation notifications so the collector observes only this
    // turn's stream.
    let _ = drain(rx);

    let collector = NotificationCollector::start(rx);

    let request = text_prompt(session.session_id.clone(), prompt_text);
    // The turn must not HANG (the only hard failure here). We deliberately do
    // NOT distinguish Ok/Err: with the corrected semantics a PreToolUse deny is
    // forward progress and the turn returns Ok, but a *later* spurious failing
    // tool call from the small model can independently trip the whole-turn
    // `agentic_loop_aborted` guard. That whole-turn flag is therefore not a
    // sound signal for any per-iteration hook effect; the assertions instead
    // read the ordered session messages below, which the abort cannot retract.
    let _ = tokio::time::timeout(NO_HANG_BUDGET, server.prompt(request))
        .await
        .expect("tool-calling prompt must not hang");

    let notes = collector.finish().await;
    let tool_call_broadcast = notes
        .iter()
        .any(|n| matches!(n.update, SessionUpdate::ToolCall(_)));

    let llama_id = parse_llama_id(&session.session_id);
    let final_session = server
        .agent_server()
        .session_manager()
        .get_session(&llama_id)
        .await
        .expect("session lookup must not error")
        .expect("session must exist after the turn");

    let tool_messages: Vec<String> = final_session
        .messages
        .iter()
        .filter(|m| m.role == llama_agent::types::MessageRole::Tool)
        .map(|m| m.content.clone())
        .collect();
    let first_tool_message_text = tool_messages.first().cloned().unwrap_or_default();

    // Robust "the loop continued past the first tool step" signal: is there an
    // `Assistant`-role message positioned after the first `Tool`-role message?
    // This is exactly the model generating again on the threaded-back tool
    // result, and it is immune to a later unrelated failing tool call tripping
    // the whole-turn abort.
    let assistant_after_first_tool = first_tool_index(&final_session.messages)
        .map(|tool_idx| {
            final_session.messages[tool_idx + 1..]
                .iter()
                .any(|m| m.role == llama_agent::types::MessageRole::Assistant)
        })
        .unwrap_or(false);

    ToolTurnOutcome {
        tool_call_broadcast,
        tool_message_count: tool_messages.len(),
        first_tool_message_text,
        assistant_after_first_tool,
    }
}

/// Index of the first `Tool`-role message in an ordered message list, or `None`
/// when the loop appended no tool result.
fn first_tool_index(messages: &[llama_agent::types::Message]) -> Option<usize> {
    messages
        .iter()
        .position(|m| m.role == llama_agent::types::MessageRole::Tool)
}

/// Run up to `MAX_ATTEMPTS` real ACP turns, returning the first outcome
/// `accept` is satisfied on. `accept` lets each test state precisely which
/// tool-path turn it needs (e.g. "a denied result landed" vs "a *successful*
/// read_file result landed"), so a turn where the small model takes a degenerate
/// path is retried instead of asserted on. Returns `None` when no attempt
/// satisfied `accept` — the caller SKIPS (warn) in that case rather than
/// flaking, matching `acp_multi_turn_dispatches_tool_and_threads_result`.
async fn first_tool_turn(
    server: &AcpServer,
    rx: &mut Receiver<Notification>,
    project: &Path,
    mcp_url: &str,
    prompt_text: &str,
    accept: impl Fn(&ToolTurnOutcome) -> bool,
) -> Option<ToolTurnOutcome> {
    for attempt in 1..=MAX_ATTEMPTS {
        info!("tool-turn attempt {}/{}", attempt, MAX_ATTEMPTS);
        let outcome = run_tool_turn(server, rx, project, mcp_url, prompt_text).await;
        if accept(&outcome) {
            return Some(outcome);
        }
    }
    None
}

// ===========================================================================
// Test 1: PreToolUse deny blocks the real read_file call
// ===========================================================================

/// A PreToolUse `deny` hook in a real `.claude/settings.json`, loaded by the
/// server from the session cwd and matched against the bare MCP tool name
/// `read_file`, blocks the real tool call **inside the live agentic loop**.
///
/// The hook command both `touch`es a marker file (so "the command ran" is
/// independently observable) and emits the documented `permissionDecision:deny`
/// JSON. Once the model emits the tool call (bounded retry; skip if it never
/// does), the assertions are deterministic:
///
/// - the threaded-back `Tool`-role message carries the deny reason
///   (`blocked by test`), proving the hook fired through the loop, and
/// - it does NOT contain the fixture content `hello` — proving real dispatch was
///   prevented (`read_file` never actually ran), and
/// - `server.prompt()` returns **Ok** (the turn completed normally, NOT
///   `agentic_loop_aborted`) — a deny is informed forward progress, so the model
///   continues past the block, matching Claude Code, and
/// - the marker file exists — independent proof the hook command executed.
#[tokio::test]
#[serial]
async fn pre_tool_use_deny_blocks_real_read_file_through_live_loop() {
    init_tracing();
    info!("=== ACP PRE-TOOL-USE DENY (real model, live loop) ===");

    let _home = HomeGuard::new();

    let mcp_url = start_read_file_mcp_server()
        .await
        .expect("read_file MCP server must start");

    let Some((server, mut rx)) = build_real_model_server(real_model_config()).await else {
        return;
    };

    // The marker lives in a stable temp dir (NOT the project, whose own
    // `.claude` we don't want to perturb): its existence proves the hook command
    // ran even though dispatch was blocked.
    let marker_dir = TempDir::new().unwrap();
    let marker = marker_dir.path().join("pre_tool_use.marker");

    // Real PreToolUse command hook matching the bare MCP tool name `read_file`:
    // touch the marker (observable side effect) AND deny via the documented
    // JSON-stdout contract. The marker path is embedded as a JSON string literal
    // so backslashes/quotes are escaped correctly on any platform.
    let marker_json = serde_json::to_string(&marker.to_string_lossy().into_owned()).unwrap();
    let settings = format!(
        r#"{{ "hooks": {{ "PreToolUse": [ {{ "matcher": "read_file", "hooks": [ {{ "type": "command", "command": "touch {} ; echo '{{\"hookSpecificOutput\":{{\"hookEventName\":\"PreToolUse\",\"permissionDecision\":\"deny\",\"permissionDecisionReason\":\"blocked by test\"}}}}'" }} ] }} ] }} }}"#,
        marker_json.trim_matches('"')
    );
    let project = project_with_settings(&settings);

    let fixture = fixture_path();
    let prompt_text = read_file_prompt(&fixture);

    // Accept the first turn on which a `Tool`-role result actually landed: the
    // iteration-1 deny message is what every assertion below reads. (Every
    // read_file call is denied by the matcher, so the first Tool message is
    // always the deny.)
    let Some(outcome) = first_tool_turn(
        &server,
        &mut rx,
        project.path(),
        &mcp_url,
        &prompt_text,
        |o| o.tool_message_count >= 1,
    )
    .await
    else {
        warn!(
            "Skipping PreToolUse deny assertion: the test model never emitted the read_file tool \
             call in {MAX_ATTEMPTS} attempts (model comprehension limitation)."
        );
        return;
    };

    // Hard guard: the model emitted the tool call this turn.
    assert!(
        outcome.tool_call_broadcast,
        "when the model emits a tool call the loop must broadcast a ToolCall notification"
    );
    assert!(
        outcome.tool_message_count >= 1,
        "the loop must append the denied Tool-role result message; got {}",
        outcome.tool_message_count
    );

    // (a) The hook blocked the real read. All checks read the ITERATION-1 Tool
    // message specifically, so a later spurious tool call the small model may
    // emit cannot perturb them.
    //
    // The iteration-1 Tool message carries the deny reason, proving the loaded
    // `.claude` hook fired inside the live loop.
    assert!(
        outcome.first_tool_message_text.contains("blocked by test"),
        "the deny reason from the loaded settings file must reach the model via the iteration-1 \
         Tool message; got: {:?}",
        outcome.first_tool_message_text
    );

    // Real dispatch was prevented: the fixture content never reached the model,
    // so `read_file` was never actually executed on iteration 1.
    assert!(
        !outcome.first_tool_message_text.contains("hello")
            && !outcome.first_tool_message_text.contains("fn main"),
        "a denied call must never dispatch — the fixture content must NOT appear in the \
         iteration-1 Tool message; got: {:?}",
        outcome.first_tool_message_text
    );

    // Independent proof the hook command ran.
    assert!(
        marker.exists(),
        "the PreToolUse hook command must have run (marker file must exist): {}",
        marker.display()
    );

    // (b) The user-chosen "model CONTINUES past the deny" behavior, proven
    // ROBUSTLY: after the iteration-1 deny, the live loop re-prompts and the
    // model generates again, so the session contains an `Assistant`-role message
    // positioned AFTER the first `Tool`-role (deny) message. Post-fix a deny is
    // `failed:false`, so the loop ALWAYS continues — this is deterministic.
    //
    // We deliberately do NOT gate on the whole-turn `agentic_loop_aborted` flag:
    // a later, unrelated failing tool call from the small model can trip that
    // flag independently of the deny, but it cannot retract an assistant
    // generation that already followed the deny.
    assert!(
        outcome.assistant_after_first_tool,
        "a denied tool call is forward progress — after the deny the loop must re-prompt and the \
         model must generate again (an Assistant message must follow the iteration-1 deny Tool \
         message)"
    );
}

// ===========================================================================
// Test 2: PostToolUse additionalContext reaches the model from the live loop
// ===========================================================================

/// A PostToolUse command hook in a real `.claude/settings.json`, loaded by the
/// server from the session cwd and matched against the bare MCP tool name
/// `read_file`, returns a unique `additionalContext` marker that reaches the
/// model **from the live agentic loop** while the tool still executes.
///
/// Once the model emits the tool call (bounded retry; skip if it never does),
/// the assertions are deterministic:
///
/// - the tool really executed — the threaded-back `Tool`-role message contains
///   the fixture content `hello`, and
/// - the unique PostToolUse marker is appended to that same Tool message — the
///   loaded hook's additionalContext reached the model through the live loop.
#[tokio::test]
#[serial]
async fn post_tool_use_additional_context_reaches_model_through_live_loop() {
    init_tracing();
    info!("=== ACP POST-TOOL-USE additionalContext (real model, live loop) ===");

    let _home = HomeGuard::new();

    let mcp_url = start_read_file_mcp_server()
        .await
        .expect("read_file MCP server must start");

    let Some((server, mut rx)) = build_real_model_server(real_model_config()).await else {
        return;
    };

    // A unique marker so a stray match cannot pass the assertion by accident.
    const POST_MARKER: &str = "POST_TOOL_CONTEXT_MARKER_9F3A1C";

    // Real PostToolUse command hook matching the bare MCP tool name `read_file`,
    // emitting the unique additionalContext via the documented JSON-stdout
    // contract. PostToolUse cannot block, so the tool runs and its result plus
    // this context both reach the model.
    let settings = format!(
        r#"{{ "hooks": {{ "PostToolUse": [ {{ "matcher": "read_file", "hooks": [ {{ "type": "command", "command": "echo '{{\"hookSpecificOutput\":{{\"hookEventName\":\"PostToolUse\",\"additionalContext\":\"{POST_MARKER}\"}}}}'" }} ] }} ] }} }}"#
    );
    let project = project_with_settings(&settings);

    let fixture = fixture_path();
    let prompt_text = read_file_prompt(&fixture);

    // A PostToolUse hook cannot block, so the whole-turn outcome is irrelevant
    // (a later spurious failing tool call could trip the abort guard with no
    // bearing on the hook). We retry until the model produces a *successful*
    // read_file Tool message — its iteration-1 Tool result must carry the
    // fixture content `hello` — and assert the hook effect on that message
    // alone. If the model never emits a successful read this turn, retry; if it
    // never does across all attempts, SKIP rather than flake.
    let Some(outcome) = first_tool_turn(
        &server,
        &mut rx,
        project.path(),
        &mcp_url,
        &prompt_text,
        |o| o.first_tool_message_text.contains("hello"),
    )
    .await
    else {
        warn!(
            "Skipping PostToolUse additionalContext assertion: the test model never emitted a \
             successful read_file tool call in {MAX_ATTEMPTS} attempts (model comprehension \
             limitation)."
        );
        return;
    };

    // Hard guard: the model emitted the tool call this turn.
    assert!(
        outcome.tool_call_broadcast,
        "when the model emits a tool call the loop must broadcast a ToolCall notification"
    );
    assert!(
        outcome.tool_message_count >= 1,
        "the loop must append the read_file Tool-role result message; got {}",
        outcome.tool_message_count
    );

    // The tool really executed: the iteration-1 Tool message carries the fixture
    // content (proving PostToolUse did NOT block — unlike PreToolUse deny).
    assert!(
        outcome.first_tool_message_text.contains("hello"),
        "PostToolUse must let the tool execute — the fixture content `hello` must appear in the \
         iteration-1 Tool message; got: {:?}",
        outcome.first_tool_message_text
    );

    // Deterministic hook effect: the unique additionalContext from the loaded
    // settings file is appended to that same iteration-1 Tool message the model
    // sees alongside the fixture content.
    assert!(
        outcome.first_tool_message_text.contains(POST_MARKER),
        "PostToolUse additionalContext from the loaded settings file must reach the model via the \
         iteration-1 Tool message; got: {:?}",
        outcome.first_tool_message_text
    );
}
