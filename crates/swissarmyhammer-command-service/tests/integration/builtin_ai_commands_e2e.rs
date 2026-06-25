//! End-to-end test for the `ai-commands` builtin plugin's event-driven
//! palette availability gate on `ai.cancel`.
//!
//! This is the real-pipeline proof that "Stop AI Generation" (`ai.cancel`) is
//! gated in the REGISTRY palette — the `available command` operation — by an
//! event-driven cached flag the plugin maintains, NOT by a hand-built mock.
//! Nothing here is faked: the committed `builtin/plugins/ai-commands` bundle is
//! discovered from a builtin layer and loaded into a real [`PluginHost`] with a
//! real V8 isolate; the real `commands` and `ui_state` in-process MCP servers
//! are exposed; and the AI-streaming notification is published on the host's
//! real [`NotificationBridge`](swissarmyhammer_plugin::NotificationBridge) —
//! exactly what the production `ai_set_streaming` Tauri command does.
//!
//! The chain proven end to end:
//!
//! 1. the `ai-commands` plugin's `load()` subscribes via the SDK
//!    `this.ui_state.on("aiStreaming", …)` surface (resolved against the
//!    `ui_state` tool's declared `io.swissarmyhammer/notifications` `_meta`),
//!    caches the streaming flag, and registers `ai.cancel` with a SYNCHRONOUS
//!    `available` that reads the cached flag;
//! 2. while idle, `available command` for `ai.cancel` returns
//!    `{ ok: false, reason: "No AI generation is running" }`;
//! 3. publishing the `notifications/ui_state/ai_streaming` notification with
//!    `streaming: true` drives the plugin's `.on` callback through the host's
//!    event pump, flipping the cached flag, so `available command` then returns
//!    `{ ok: true }`;
//! 4. publishing `streaming: false` re-closes the gate.
//!
//! Every cross-thread interaction is bounded by a timeout so a wedged isolate
//! fails the test fast instead of hanging CI.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};
use swissarmyhammer_command_service::bootstrap::install_commands_module;
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_directory::KanbanConfig;
use swissarmyhammer_plugin::{CallerId, InProcessServer, McpServer as PluginMcpServer, PluginHost};
use swissarmyhammer_ui_state::{ai_streaming_notification, UiState, UiStateServer};
use tempfile::TempDir;

use super::support::{copy_dir_recursive, try_call_command};

/// A generous upper bound on any single host or isolate interaction.
const TIMEOUT: Duration = Duration::from_secs(60);

/// The directory name of the builtin bundle under test.
const BUNDLE: &str = "ai-commands";

/// The `plugins/` subdirectory of a layer root the host discovers bundles in.
const PLUGINS_SUBDIR: &str = "plugins";

/// Resolve the workspace root (two levels above this crate's manifest dir).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above the crate manifest dir")
        .to_path_buf()
}

/// Stage the committed `builtin/plugins/ai-commands` bundle into a temp
/// builtin-layer root so `discover_and_load_all` finds it at
/// `<layer_root>/plugins/ai-commands/`.
fn stage_ai_commands(layer_root: &Path) {
    let source = workspace_root().join("builtin/plugins").join(BUNDLE);
    assert!(
        source.is_dir(),
        "the committed ai-commands bundle must exist at {}",
        source.display()
    );
    let destination = layer_root.join(PLUGINS_SUBDIR).join(BUNDLE);
    copy_dir_recursive(&source, &destination);
}

/// Expose the real `ui_state` in-process MCP server (over a temp-file-backed
/// `UiState`) on `host` under its public id, kept alive by the returned guard.
async fn expose_ui_state(host: &PluginHost) -> (TempDir, Arc<UiState>) {
    let dir = TempDir::new().expect("ui_state substrate temp dir");
    let ui_state = Arc::new(UiState::load(dir.path().join("ui_state.yaml")));
    let ui_state_server = UiStateServer::new(Arc::clone(&ui_state));
    let ui_state_module = InProcessServer::new(ui_state_server)
        .await
        .expect("wrapping the ui_state server in an InProcessServer should succeed");
    host.expose_rust_module(
        "ui_state".to_string(),
        Arc::new(ui_state_module) as Arc<dyn PluginMcpServer>,
    )
    .await
    .expect("exposing the ui_state module should succeed");
    (dir, ui_state)
}

/// Call `available command` for `id` and return the `{ ok, reason? }`
/// structured-content object.
async fn available(service: &CommandService, id: &str) -> Value {
    let resp = try_call_command(
        service,
        CallerId::HostInternal,
        json!({ "op": "available command", "id": id, "ctx": {} }),
    )
    .await
    .unwrap_or_else(|e| panic!("available command for {id} raised: {e:?}"));
    resp.get("structuredContent")
        .cloned()
        .unwrap_or_else(|| panic!("available response must carry structuredContent, got {resp}"))
}

/// Poll `available command` for `id` until its `ok` matches `want`, or panic on
/// timeout. The event pump delivers a published notification to the plugin's
/// `.on` callback asynchronously, so the cached flag flips a beat after the
/// publish — poll rather than asserting on the first read.
async fn wait_for_available(service: &CommandService, id: &str, want: bool) -> Value {
    let deadline = tokio::time::Instant::now() + TIMEOUT;
    loop {
        let got = available(service, id).await;
        if got.get("ok").and_then(Value::as_bool) == Some(want) {
            return got;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("timed out waiting for {id} available.ok == {want}; last response: {got}");
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// A built `ai-commands` host: the real [`PluginHost`] with the committed bundle
/// loaded, its [`CommandService`], and the live guards that must outlive the
/// test (the staged builtin/user layer temp dirs and the `ui_state` substrate).
struct AiCommandsHost {
    host: PluginHost,
    service: Arc<CommandService>,
    /// Keeps the staged builtin layer, user root, and `ui_state` substrate dirs
    /// alive for the host's lifetime — dropping them would pull the rug out from
    /// under the running isolate.
    _guards: (TempDir, TempDir, TempDir, Arc<UiState>),
}

/// Build a fresh `ai-commands` host: stage the committed bundle into a temp
/// builtin layer, build a real [`PluginHost`], install the `commands` module,
/// expose the real `ui_state` server, and discover/load the bundle.
///
/// This is the per-host setup shared by the single-host and multi-host tests —
/// each call yields a fully independent host with its OWN `ai-commands` isolate
/// and OWN cached streaming flag, exactly as the global host and each per-board
/// host are independent in production.
async fn build_ai_commands_host() -> AiCommandsHost {
    let user_root = TempDir::new().expect("user root temp dir");
    let builtin_root = TempDir::new().expect("builtin layer root temp dir");

    stage_ai_commands(builtin_root.path());

    let host = PluginHost::new(
        Some(builtin_root.path().to_path_buf()),
        user_root.path().to_path_buf(),
        None,
        user_root.path().to_path_buf(),
        false,
        user_root.path().to_path_buf(),
    );

    let service = install_commands_module(&host)
        .await
        .expect("install_commands_module must succeed");

    // Expose `ui_state` BEFORE discovery so the plugin's
    // `ensureServices(this, ["commands", "ui_state"])` finds it already exposed
    // and `this.ui_state.on("aiStreaming", …)` resolves against its `_meta`.
    let (ui_state_dir, ui_state) = tokio::time::timeout(TIMEOUT, expose_ui_state(&host))
        .await
        .expect("exposing ui_state should not hang");

    let loaded = tokio::time::timeout(TIMEOUT, host.discover_and_load_all::<KanbanConfig>())
        .await
        .expect("discovery should not hang")
        .expect("discovering the ai-commands builtin plugin should succeed");
    assert_eq!(
        loaded.len(),
        1,
        "exactly the one ai-commands builtin plugin should be discovered, got {loaded:?}"
    );

    AiCommandsHost {
        host,
        service,
        _guards: (builtin_root, user_root, ui_state_dir, ui_state),
    }
}

/// The committed `ai-commands` builtin plugin gates `ai.cancel` in the registry
/// palette via an event-driven cached flag: disabled while idle, enabled while
/// the conversation streams, disabled again when the turn ends.
#[tokio::test]
async fn ai_cancel_palette_availability_tracks_streaming_notification() {
    let AiCommandsHost { host, service, .. } = build_ai_commands_host().await;

    // Idle (default): the registry palette gate is closed.
    let idle = available(&service, "ai.cancel").await;
    assert_eq!(
        idle["ok"],
        json!(false),
        "ai.cancel must be unavailable while idle, got {idle}"
    );
    assert_eq!(
        idle["reason"], "No AI generation is running",
        "the idle reason must name why ai.cancel is gated, got {idle}"
    );

    // Streaming starts: publish the AI-streaming notification on the host's
    // bridge, exactly as the production `ai_set_streaming` Tauri command does.
    let reached = host
        .notification_bridge()
        .publish(ai_streaming_notification(true));
    assert!(
        reached >= 1,
        "the host event pump should be a live bridge subscriber, got {reached}"
    );

    let streaming = wait_for_available(&service, "ai.cancel", true).await;
    assert_eq!(
        streaming["ok"],
        json!(true),
        "ai.cancel must be available mid-stream, got {streaming}"
    );

    // The turn ends: the gate re-closes.
    host.notification_bridge()
        .publish(ai_streaming_notification(false));
    let idle_again = wait_for_available(&service, "ai.cancel", false).await;
    assert_eq!(
        idle_again["ok"],
        json!(false),
        "ai.cancel must be unavailable once the turn ends, got {idle_again}"
    );
}

/// MULTI-HOST TOPOLOGY: each `ai-commands` host owns an INDEPENDENT streaming
/// flag — a publish on one host's bridge flips ONLY that host's `ai.cancel`,
/// never another host's.
///
/// This is the topology property the single-host test above structurally cannot
/// observe (it has one host, so it can never tell whether a publish that reaches
/// the subscriber reached the RIGHT one). Production runs two host kinds — the
/// global fallback host AND a per-board host for each open board — each loading
/// its OWN `ai-commands` isolate with its OWN module-level cached flag and its
/// OWN `aiStreaming` subscription bound to its OWN bridge. The AI panel mounts
/// inside a board window, whose palette routes `available command` for
/// `ai.cancel` to the PER-BOARD host. Because the per-board and global flags are
/// thus independent, a publish aimed at the wrong host is invisible to the host
/// that answers the board window's palette — which is exactly the bug the prior
/// global-only `ai_set_streaming` publish caused: "Stop AI Generation" stuck
/// disabled mid-stream on every board window.
///
/// This test pins that independence: it builds two hosts, publishes on ONE
/// host's bridge, and asserts the OTHER host's `ai.cancel` does not flip. The
/// end-to-end proof that production `ai_set_streaming` actually RESOLVES the
/// streaming window to its per-board host's bridge (so the publish lands on the
/// host that answers the palette) lives in the kanban-app crate, which owns the
/// Tauri window→board routing this crate cannot reach:
/// `kanban_app::plugins::tests::ai_set_streaming_reaches_per_board_host_for_a_board_window`.
#[tokio::test]
async fn ai_streaming_flag_is_independent_per_host() {
    // A global fallback host and a per-board host — two independent isolates,
    // exactly as production builds them.
    let global = build_ai_commands_host().await;
    let per_board = build_ai_commands_host().await;

    // Both start idle: each isolate's cached flag defaults false.
    for (svc, which) in [
        (&global.service, "global"),
        (&per_board.service, "per-board"),
    ] {
        let idle = available(svc, "ai.cancel").await;
        assert_eq!(
            idle["ok"],
            json!(false),
            "ai.cancel must be unavailable while idle on the {which} host, got {idle}"
        );
    }

    // Publish on the PER-BOARD host's bridge — standing in for the host the
    // fixed `ai_set_streaming` resolves a board window to. (The window→host
    // resolution itself is proven by the kanban-app test named in the doc
    // comment; here we only need a publish on one of the two hosts.)
    let reached = per_board
        .host
        .notification_bridge()
        .publish(ai_streaming_notification(true));
    assert!(
        reached >= 1,
        "the per-board host's event pump should be a live subscriber, got {reached}"
    );

    // The per-board isolate's gate opens…
    let streaming = wait_for_available(&per_board.service, "ai.cancel", true).await;
    assert_eq!(
        streaming["ok"],
        json!(true),
        "ai.cancel must be available mid-stream on the per-board host, got {streaming}"
    );

    // …while the GLOBAL isolate — a different bridge, a different cached flag —
    // is untouched. This is the crux: a publish aimed at the global host (the
    // pre-fix behaviour) would have flipped THIS isolate and left the per-board
    // one (which answers the board window's palette) stuck disabled.
    let global_still_idle = available(&global.service, "ai.cancel").await;
    assert_eq!(
        global_still_idle["ok"],
        json!(false),
        "publishing on the per-board bridge must NOT flip the global isolate's \
         ai.cancel — the two hosts' streaming flags are isolated, got {global_still_idle}"
    );

    // Ending the turn on the per-board bridge re-closes only the per-board gate.
    per_board
        .host
        .notification_bridge()
        .publish(ai_streaming_notification(false));
    let per_board_idle = wait_for_available(&per_board.service, "ai.cancel", false).await;
    assert_eq!(
        per_board_idle["ok"],
        json!(false),
        "ai.cancel must re-close on the per-board host when the turn ends, got {per_board_idle}"
    );
}
